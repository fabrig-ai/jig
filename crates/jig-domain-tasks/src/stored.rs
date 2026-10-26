//! Cold admission of bounded live rows and complete link validation
//! (domain/tasks.md, sections 2 and 5). Live/Ledger/Stub enter startup;
//! historical Ended rows stay in root storage. Successful restoration
//! reconstructs activations, claims, closing gates and projected retry timers;
//! failed restoration never becomes ready through more input.
use crate::domain::{Domain, Startup, Task, activate, publish, record, snapshot, task_mut};
use crate::{
    Active, Closing, Ending, Executor, Limits, Party, Phase, Problem, Refusal, Request, Stage, Stored, TaskRecord, Was,
};
use skein_lib::{Env, Queue};

fn valid_ending(limits: &Limits, ending: &Ending) -> bool {
    let cap = usize::try_from(limits.result_bytes).expect("u32 fits usize");
    match ending {
        Ending::Done(result) => crate::run::result_bytes(result) <= cap,
        Ending::Failed { reason } => reason.len() <= cap,
        Ending::Cancelled { reason, result, by, .. } => {
            let valid_party = match by {
                Party::Person(number) | Party::Task(number) => *number != 0,
                Party::Deployment { project } => *project != 0,
            };
            if !valid_party || reason.len() > cap {
                return false;
            }
            match result {
                Some(result) => crate::run::result_bytes(result) <= cap,
                None => true,
            }
        }
    }
}

fn valid_stage(task: &TaskRecord, closing: &Closing, limits: &Limits) -> bool {
    valid_ending(limits, &closing.ending)
        && match &closing.ending {
            Ending::Done(result) | Ending::Cancelled { result: Some(result), .. } => {
                crate::run::valid_result(&task.contract, result, limits)
            }
            Ending::Failed { .. } | Ending::Cancelled { result: None, .. } => true,
        }
        && match closing.stage {
            Stage::Run { attempt } => attempt != 0 && attempt == task.attempt && task.last_answer != Some(attempt),
            Stage::Delegates => true,
            Stage::Effects | Stage::Releases | Stage::Settled => task.delegates.is_empty(),
        }
}

fn valid_phase(task: &TaskRecord, limits: &Limits) -> bool {
    if let Phase::Held { why, .. } = task.phase
        && !why.valid()
    {
        return false;
    }
    if let Phase::Held { why, source, .. } = &task.phase
        && !crate::hold_origin::valid(limits, *why, source.as_deref())
    {
        return false;
    }
    if !crate::flags::valid(task.flags, task.executor, task.requester) {
        return false;
    }
    match task.executor {
        Executor::Person(_) if !person_phase(&task.phase) => return false,
        Executor::Procedure { .. } | Executor::Person(_) if task.flags.conversation => return false,
        Executor::Person(_) | Executor::Agent { .. } | Executor::Procedure { .. } => {}
    }
    match &task.phase {
        Phase::Active(Active::Claimed { attempt } | Active::Running { attempt })
        | Phase::Held { was: Was::Active(Active::Claimed { attempt } | Active::Running { attempt }), .. } => {
            *attempt != 0 && *attempt == task.attempt && task.last_answer != Some(*attempt)
        }
        Phase::Closing(closing) => closing.stage != Stage::Settled && valid_stage(task, closing, limits),
        Phase::Held { was: Was::Closing(closing), .. } => valid_stage(task, closing, limits),
        Phase::Waiting
        | Phase::Active(Active::Idle | Active::Due | Active::Preparing | Active::BackingOff { .. })
        | Phase::Held {
            was: Was::Waiting | Was::Active(Active::Idle | Active::Due | Active::Preparing | Active::BackingOff { .. }),
            ..
        } => true,
        Phase::Ended(_) => false,
    }
}

fn person_phase(phase: &Phase) -> bool {
    match phase {
        Phase::Waiting
        | Phase::Active(Active::Due | Active::Idle)
        | Phase::Held { was: Was::Waiting | Was::Active(Active::Due | Active::Idle), .. }
        | Phase::Closing(Closing {
            stage: Stage::Delegates | Stage::Effects | Stage::Releases | Stage::Settled, ..
        })
        | Phase::Held {
            was:
                Was::Closing(Closing {
                    stage: Stage::Delegates | Stage::Effects | Stage::Releases | Stage::Settled, ..
                }),
            ..
        } => true,
        Phase::Active(
            Active::Preparing | Active::Claimed { .. } | Active::Running { .. } | Active::BackingOff { .. },
        )
        | Phase::Closing(Closing { stage: Stage::Run { .. }, .. })
        | Phase::Held {
            was:
                Was::Active(Active::Preparing | Active::Claimed { .. } | Active::Running { .. } | Active::BackingOff { .. }),
            ..
        }
        | Phase::Held { was: Was::Closing(Closing { stage: Stage::Run { .. }, .. }), .. }
        | Phase::Ended(_) => false,
    }
}

#[expect(clippy::too_many_lines, reason = "one restored proposal validates its whole action and pending holder")]
fn valid_proposal(task: &TaskRecord, limits: &Limits) -> bool {
    if task.result_proposal {
        let waiting = match &task.phase {
            Phase::Closing(Closing { stage: Stage::Delegates, .. })
            | Phase::Held { was: Was::Closing(Closing { stage: Stage::Delegates, .. }), .. } => true,
            Phase::Waiting | Phase::Active(_) | Phase::Closing(_) | Phase::Held { .. } | Phase::Ended(_) => false,
        };
        if !waiting {
            return false;
        }
    }
    let Some(proposal) = &task.proposal else { return !task.result_proposal };
    let result_batch = match proposal.action {
        crate::ProposalAction::Batch(_) => true,
        crate::ProposalAction::Settings { .. }
        | crate::ProposalAction::Effect { .. }
        | crate::ProposalAction::ProcedureEffect { .. }
        | crate::ProposalAction::Amend { .. }
        | crate::ProposalAction::Widen { .. }
        | crate::ProposalAction::Release { .. }
        | crate::ProposalAction::ReleasePlaced { .. } => false,
    };
    if task.result_proposal && (proposal.as_holder || !result_batch) {
        return false;
    }
    if proposal.number == 0
        || proposal.proposer != task.number
        || !crate::proposals::proposal_scope(task.project, proposal)
        || proposal.reason.len() > usize::try_from(limits.message_bytes).expect("u32 fits usize")
        || task.revision == 0
    {
        return false;
    }
    let holder = match proposal.state {
        crate::ProposalState::Pending { holder, .. } => holder,
        crate::ProposalState::Accepted { .. }
        | crate::ProposalState::Rejected { .. }
        | crate::ProposalState::Withdrawn => return false,
    };
    let holder_valid = match holder {
        crate::ProposalHolder::Task(number) => number != 0 && number != task.number,
        crate::ProposalHolder::Person(number) => number != 0,
        crate::ProposalHolder::Policy { project, kind } => {
            project == proposal.project
                && kind
                    == match proposal.action {
                        crate::ProposalAction::Settings { .. } => crate::ProposalKind::Settings,
                        crate::ProposalAction::Batch(_) => crate::ProposalKind::Batch,
                        crate::ProposalAction::Effect { .. } | crate::ProposalAction::ProcedureEffect { .. } => {
                            crate::ProposalKind::Effect
                        }
                        crate::ProposalAction::Amend { .. } => crate::ProposalKind::Amend,
                        crate::ProposalAction::Widen { .. } => crate::ProposalKind::Widen,
                        crate::ProposalAction::Release { .. } | crate::ProposalAction::ReleasePlaced { .. } => {
                            crate::ProposalKind::Release
                        }
                    }
        }
    };
    if !holder_valid {
        return false;
    }
    let action_valid = match &proposal.action {
        crate::ProposalAction::Settings { .. } => true,
        crate::ProposalAction::Batch(batch) => {
            if batch.is_empty() || batch.len() > usize::try_from(limits.batch).expect("u32 fits usize") {
                return false;
            }
            for (at, member) in batch.iter().enumerate() {
                if member.number == 0
                    || !crate::flags::shaped(member.flags, member.executor)
                    || member.project != task.project
                    || member.funder != crate::Funder::Task(task.number)
                    || !crate::batch::valid_spec(limits, &member.spec)
                    || !member.spec.inputs.is_empty()
                    || !crate::batch::valid_contract(limits, &member.contract)
                    || !crate::batch::valid_authority(limits, &member.authority)
                    || !crate::wake::valid(&member.wake)
                    || member.dependencies.len() > usize::try_from(limits.dependencies).expect("u32 fits usize")
                {
                    return false;
                }
                for earlier in batch.iter().take(at) {
                    if earlier.number == member.number {
                        return false;
                    }
                }
            }
            true
        }
        crate::ProposalAction::Effect { authority, attempt, completion, .. } => {
            *attempt != 0 && *completion != 0 && crate::batch::valid_authority(limits, authority)
        }
        crate::ProposalAction::ProcedureEffect { authority, step, .. } => {
            *step != 0 && crate::batch::valid_authority(limits, authority)
        }
        crate::ProposalAction::Amend { task: target, amendment, .. } => {
            *target != 0 && amendment.authority.is_some() && valid_amendment(limits, amendment)
        }
        crate::ProposalAction::Widen { task: target, authority } => {
            *target != 0 && crate::batch::valid_authority(limits, authority)
        }
        crate::ProposalAction::Release { task: target } => *target != 0,
        crate::ProposalAction::ReleasePlaced { task, hold, revision } => *task != 0 && *hold != 0 && *revision != 0,
    };
    if !action_valid {
        return false;
    }
    match (&proposal.state, &task.phase) {
        (
            crate::ProposalState::Pending { .. },
            Phase::Waiting | Phase::Active(_) | Phase::Held { was: Was::Waiting | Was::Active(_), .. },
        ) => true,
        (crate::ProposalState::Pending { .. }, Phase::Closing(_) | Phase::Held { was: Was::Closing(_), .. })
            if task.result_proposal =>
        {
            true
        }
        (
            crate::ProposalState::Pending { .. },
            Phase::Closing(_) | Phase::Held { was: Was::Closing(_), .. } | Phase::Ended(_),
        )
        | (
            crate::ProposalState::Accepted { .. }
            | crate::ProposalState::Rejected { .. }
            | crate::ProposalState::Withdrawn,
            _,
        ) => false,
    }
}

fn valid_escalation(task: &TaskRecord, limits: &Limits) -> bool {
    if task.revisions.hold != task.escalation.revision() {
        return false;
    }
    let held = match task.phase {
        Phase::Held { why: crate::Hold::Placed { .. }, .. }
        | Phase::Waiting
        | Phase::Active(_)
        | Phase::Closing(_)
        | Phase::Ended(_) => false,
        Phase::Held { .. } => true,
    };
    match &task.escalation {
        crate::Escalation::Unheld { revision } => *revision != u64::MAX && !held,
        crate::Escalation::Routing { .. } => false,
        crate::Escalation::Waiting { revision, holder, entry, .. } => {
            *revision != 0
                && *entry != 0
                && held
                && match holder {
                    crate::EscalationHolder::Task(parent) => *parent != 0 && *parent != task.number,
                    crate::EscalationHolder::Person(person) => {
                        *person != 0
                            && match task.requester {
                                Party::Person(requester) => *person == requester,
                                Party::Task(_) | Party::Deployment { .. } => true,
                            }
                    }
                    crate::EscalationHolder::Role { project, .. } => *project == task.project,
                }
        }
        crate::Escalation::Rejected { revision, by, reason } => {
            *revision != 0
                && *by != 0
                && held
                && reason.len() <= usize::try_from(limits.result_bytes).expect("u32 fits usize")
        }
    }
}

fn valid_followups(task: &TaskRecord) -> bool {
    if task.result_followups.is_empty() {
        return true;
    }
    let ending = match &task.phase {
        Phase::Closing(closing) | Phase::Held { was: Was::Closing(closing), .. } => &closing.ending,
        Phase::Waiting
        | Phase::Active(_)
        | Phase::Held { was: Was::Waiting | Was::Active(_), .. }
        | Phase::Ended(_) => {
            return false;
        }
    };
    let code = match ending {
        Ending::Done(crate::TaskResult::Verdict { code, .. })
        | Ending::Cancelled { result: Some(crate::TaskResult::Verdict { code, .. }), .. } => *code,
        Ending::Done(
            crate::TaskResult::Approved { .. }
            | crate::TaskResult::Report { .. }
            | crate::TaskResult::Change { .. }
            | crate::TaskResult::Failure { .. },
        )
        | Ending::Failed { .. }
        | Ending::Cancelled { .. } => return false,
    };
    match &task.contract {
        crate::Contract::Verdict { choices } => {
            for choice in choices {
                if choice.code == code {
                    return task.result_followups.len() <= usize::try_from(choice.followups).expect("u32 fits usize");
                }
            }
            false
        }
        crate::Contract::Approval { .. } | crate::Contract::Report { .. } | crate::Contract::Change { .. } => false,
    }
}

#[expect(clippy::too_many_lines, reason = "one complete restored task shape is checked before retention")]
fn valid_record(domain: &Domain, limits: &Limits, task: &TaskRecord) -> bool {
    if task.last_activity < task.created_at
        || task.run_reserved > task.numbers.reserved
        || (task.run_reserved != 0 && crate::run::run_attempt(&task.phase).is_none())
        || task.ended_at.is_some()
        || crate::funders::total(task.numbers).is_none()
        || crate::funders::available(task.numbers).is_none()
        || task.depth > limits.depth
        || task.made == 0
        || task.made > limits.tree_tasks
        || task.delegates.len() > usize::try_from(limits.delegates).expect("u32 fits usize")
        || task.result_followups.len() > usize::try_from(limits.batch).expect("u32 fits usize")
        || task.references.len() > usize::try_from(limits.references).expect("u32 fits usize")
        || task.subscriptions.len() > usize::try_from(limits.subscriptions).expect("u32 fits usize")
        || task.waiting_on.len() > usize::try_from(limits.dependencies).expect("u32 fits usize")
        || task.dependencies.len() > usize::try_from(limits.dependencies).expect("u32 fits usize")
    {
        return false;
    }
    if !crate::batch::valid_spec(limits, &task.spec)
        || crate::control::instruction_admit(limits, task.executor, &task.instructions).is_err()
        || !crate::wake::valid(&task.wake)
        || !crate::batch::valid_contract(limits, &task.contract)
        || !crate::batch::valid_authority(limits, &task.authority)
        || !crate::holds::valid_record(domain, limits, task)
        || !valid_phase(task, limits)
        || !crate::runtime::valid(task)
        || !valid_escalation(task, limits)
        || !valid_proposal(task, limits)
        || !valid_followups(task)
        || !valid_grace(task, limits)
        || !valid_person_answer(task)
        || !crate::positions::valid(task, limits.inbox_messages)
    {
        return false;
    }
    if ((task.turn != 0 || !task.refused.is_empty()) && task.attempt == 0)
        || (task.narrowing && crate::run::run_attempt(&task.phase).is_none())
    {
        return false;
    }
    let mut previous = 0;
    for &number in &task.refused {
        if number <= previous {
            return false;
        }
        previous = number;
    }
    let Some((subscription_count, subscription_bytes)) = crate::subscriptions::credit(task, limits) else {
        return false;
    };
    let mut amendments = 0_usize;
    let mut amendment_bytes = 0_usize;
    let mut ordinary_amendment = false;
    let mut met = false;
    for word in &task.inbox {
        if crate::inbox::finish_word(task, word.number) != word.finish_requested() {
            return false;
        }
        match word.kind {
            // One merged notice per task, from the ended dependency or, for a
            // removal, from its amender (domain/tasks.md, 5.7 and 8.1).
            crate::MessageKind::Met { dependency, removed } => {
                if met || dependency == 0 || !removed && word.from != Party::Task(dependency) {
                    return false;
                }
                met = true;
            }
            crate::MessageKind::Amendment { revision } | crate::MessageKind::FinishRequested { revision } => {
                let finish = crate::inbox::finish_word(task, word.number);
                if !finish && ordinary_amendment {
                    return false;
                }
                ordinary_amendment |= !finish;
                amendments = amendments.checked_add(1).expect("bounded amendments");
                amendment_bytes = amendment_bytes.checked_add(word.words.len()).expect("bounded amendment bytes");
                if revision == 0 || revision > task.revision {
                    return false;
                }
            }
            crate::MessageKind::Escalation { .. }
            | crate::MessageKind::Proposal { .. }
            | crate::MessageKind::SettingsDecision { .. }
            | crate::MessageKind::ProposalDecision { .. }
            | crate::MessageKind::EffectNotMade { .. }
            | crate::MessageKind::Words
            | crate::MessageKind::Question
            | crate::MessageKind::Answer { .. }
            | crate::MessageKind::Notice { .. }
            | crate::MessageKind::Timer { .. }
            | crate::MessageKind::News { .. }
            | crate::MessageKind::PersonStalled { .. }
            | crate::MessageKind::Result(_) => {}
        }
    }
    if task
        .inbox
        .len()
        .saturating_sub(amendments)
        .saturating_sub(usize::from(met))
        .saturating_add(task.delegates.len())
        .saturating_add(task.questions.len())
        .saturating_add(usize::from(task.proposal.is_some()))
        .saturating_add(subscription_count)
        > usize::try_from(limits.inbox_messages).expect("u32 fits usize")
    {
        return false;
    }
    if task.last_reply > task.activity
        || task.replies.len() > usize::try_from(limits.inbox_messages).expect("reply bound fits")
    {
        return false;
    }
    let mut previous_reply = None;
    for (index, reply) in task.replies.iter().enumerate() {
        if reply.number == 0
            || reply.number > task.last_reply
            || reply.activity == 0
            || reply.activity > task.activity
            || reply.attempt == 0
            || reply.attempt > task.attempt
            || reply.turn == 0
            || (reply.attempt == task.attempt && reply.turn > task.turn)
        {
            return false;
        }
        if let Some(key) = previous_reply
            && key <= (reply.at, reply.number)
        {
            return false;
        }
        for old in task.replies.iter().take(index) {
            if old.number == reply.number || old.activity == reply.activity {
                return false;
            }
        }
        previous_reply = Some((reply.at, reply.number));
    }
    if task.last_reply != 0 && task.replies.is_empty() {
        return false;
    }
    let mut previous = 0;
    let mut bytes = 0_usize;
    for word in &task.inbox {
        let Some(payload_bytes) = word.payload_bytes() else { return false };
        let Some(total) = bytes.checked_add(payload_bytes) else { return false };
        bytes = total;
        let requester = match word.kind {
            crate::MessageKind::Proposal { .. } | crate::MessageKind::Escalation { .. } => false,
            crate::MessageKind::Words | crate::MessageKind::Question | crate::MessageKind::Answer { .. } => {
                match word.from {
                    Party::Person(person) => task.requester == Party::Person(person),
                    Party::Task(_) => true,
                    Party::Deployment { .. } => false,
                }
            }
            crate::MessageKind::EffectNotMade { .. }
            | crate::MessageKind::Amendment { .. }
            | crate::MessageKind::Met { .. } => match word.from {
                Party::Task(_) | Party::Person(_) => true,
                Party::Deployment { .. } => false,
            },
            // The deployment's close asks its own project's conversation to finish (people.md, 5.3).
            crate::MessageKind::FinishRequested { .. } => match word.from {
                Party::Task(_) | Party::Person(_) => true,
                Party::Deployment { project } => project == task.project,
            },
            crate::MessageKind::Result(_) => match word.from {
                Party::Task(_) => true,
                Party::Person(_) | Party::Deployment { .. } => false,
            },
            crate::MessageKind::SettingsDecision { proposal, outcome } => {
                let owner = match word.from {
                    Party::Person(_) => true,
                    Party::Task(_) | Party::Deployment { .. } => false,
                };
                proposal > 0
                    && match outcome {
                        crate::SettingsOutcome::Accepted { revision } => revision > 0 && owner,
                        crate::SettingsOutcome::Rejected => owner,
                        crate::SettingsOutcome::Stale { newest } => {
                            newest > 0 && word.from == Party::Deployment { project: 0 }
                        }
                    }
            }
            crate::MessageKind::ProposalDecision { .. } => match word.from {
                Party::Task(_) | Party::Person(_) => true,
                Party::Deployment { .. } => false,
            },
            crate::MessageKind::Notice { .. } | crate::MessageKind::News { .. } => match word.from {
                Party::Task(_) => true,
                Party::Person(_) | Party::Deployment { .. } => false,
            },
            crate::MessageKind::PersonStalled { task: child } => child != 0 && word.from == Party::Task(child),
            crate::MessageKind::Timer { .. } => word.from == Party::Task(task.number),
        };
        let bound = if crate::inbox::finish_word(task, word.number) {
            limits.result_bytes
        } else {
            match &word.kind {
                crate::MessageKind::Words
                | crate::MessageKind::SettingsDecision { .. }
                | crate::MessageKind::ProposalDecision { .. }
                | crate::MessageKind::FinishRequested { .. }
                | crate::MessageKind::Amendment { .. }
                | crate::MessageKind::Question
                | crate::MessageKind::Answer { .. } => limits.message_bytes,
                crate::MessageKind::Result(_) => limits.result_bytes,
                crate::MessageKind::Notice { .. } | crate::MessageKind::News { .. } => {
                    limits.result_bytes.max(limits.message_bytes)
                }
                crate::MessageKind::EffectNotMade { .. }
                | crate::MessageKind::Proposal { .. }
                | crate::MessageKind::Escalation { .. }
                | crate::MessageKind::PersonStalled { .. }
                | crate::MessageKind::Met { .. }
                | crate::MessageKind::Timer { .. } => 0,
            }
        };
        if word.number <= previous
            || word.number > task.last_message
            || (match &word.kind {
                crate::MessageKind::Words | crate::MessageKind::Question | crate::MessageKind::Answer { .. } => {
                    word.words.is_empty()
                }
                crate::MessageKind::EffectNotMade { entry } => *entry == 0,
                crate::MessageKind::Proposal { .. }
                | crate::MessageKind::Escalation { .. }
                | crate::MessageKind::PersonStalled { .. }
                | crate::MessageKind::Result(_)
                | crate::MessageKind::SettingsDecision { .. }
                | crate::MessageKind::ProposalDecision { .. }
                | crate::MessageKind::FinishRequested { .. }
                | crate::MessageKind::Amendment { .. }
                | crate::MessageKind::Met { .. }
                | crate::MessageKind::Notice { .. }
                | crate::MessageKind::News { .. }
                | crate::MessageKind::Timer { .. } => false,
            })
            || word.hits == 0
            || payload_bytes > usize::try_from(bound).expect("u32 fits usize")
            || !requester
        {
            return false;
        }
        previous = word.number;
    }
    let Some(reserved) =
        task.delegates.len().checked_mul(usize::try_from(limits.result_bytes).expect("u32 fits usize"))
    else {
        return false;
    };
    let Some(ordinary_bytes) = bytes.checked_sub(amendment_bytes) else { return false };
    let Some(total) = ordinary_bytes.checked_add(reserved) else {
        return false;
    };
    let Some(answer_reserved) =
        task.questions.len().checked_mul(usize::try_from(limits.message_bytes).expect("u32 fits usize"))
    else {
        return false;
    };
    let Some(total) = total.checked_add(answer_reserved) else { return false };
    let Some(total) = total.checked_add(if task.proposal.is_some() {
        usize::try_from(limits.message_bytes).expect("u32 fits usize")
    } else {
        0
    }) else {
        return false;
    };
    let Some(total) = total.checked_add(subscription_bytes) else { return false };
    if total > usize::try_from(limits.inbox_bytes).expect("u32 fits usize") {
        return false;
    }
    if !crate::run::saved_within(Some(&task.saved), limits) {
        return false;
    }
    if let Some(attempt) = task.last_answer
        && (attempt == 0 || attempt > task.attempt)
    {
        return false;
    }
    for followup in &task.result_followups {
        if *followup == 0 || *followup == task.number {
            return false;
        }
    }
    for numbers in [
        &*task.delegates,
        &*task.result_followups,
        &*task.references,
        &*task.dependencies,
        &*task.spec.inputs,
        &*task.waiting_on,
    ] {
        for (at, number) in numbers.iter().enumerate() {
            for earlier in numbers.iter().take(at) {
                if earlier == number {
                    return false;
                }
            }
        }
    }
    for (at, credit) in task.questions.iter().enumerate() {
        if credit.number == 0 || credit.answerer == 0 {
            return false;
        }
        for earlier in task.questions.iter().take(at) {
            if earlier.number == credit.number {
                return false;
            }
        }
    }
    for (at, subscription) in task.subscriptions.iter().enumerate() {
        if subscription.number == 0 {
            return false;
        }
        for earlier in task.subscriptions.iter().take(at) {
            if earlier.number == subscription.number {
                return false;
            }
        }
        match subscription.kind {
            crate::SubscriptionKind::Timer { period: Some(period), .. } if period == skein_lib::Duration::ZERO => {
                return false;
            }
            crate::SubscriptionKind::Task { .. }
            | crate::SubscriptionKind::Timer { .. }
            | crate::SubscriptionKind::Topic { .. } => {}
        }
    }
    // Amendment provenance is a subsequence of the current whole list
    // (domain/tasks.md, 5.7 and section 7).
    let mut remaining = task.added.iter();
    let mut entry = remaining.next();
    for dependency in &task.dependencies {
        if let Some(added) = entry
            && added.task == *dependency
        {
            entry = remaining.next();
        }
    }
    if entry.is_some() {
        return false;
    }
    for number in &task.waiting_on {
        if !crate::batch::contains(&task.dependencies, *number) {
            return false;
        }
    }
    if task.kept_account.is_some() {
        if task.attempt == 0 {
            return false;
        }
        match task.executor {
            Executor::Agent { .. } => {}
            Executor::Procedure { .. } | Executor::Person(_) => return false,
        }
    }
    if let Some(choice) = &task.llm {
        // Restore checks the choice's shape; configuration is checked as a choice is given.
        let agent = match task.executor {
            Executor::Agent { .. } => true,
            Executor::Procedure { .. } | Executor::Person(_) => false,
        };
        if !agent || !crate::llm::valid_shape(choice) {
            return false;
        }
    }
    match task.executor {
        Executor::Agent { charter } => {
            if task.recurring.is_some() || task.taken_by.is_some() || task.person_wait.is_some() {
                return false;
            }
            domain.charter_models(charter, task.settings.task).is_some()
        }
        Executor::Procedure { code, .. } if code != 0 && task.recurring.is_some() => match &task.recurring {
            Some(state) => {
                task.taken_by.is_none()
                    && task.person_wait.is_none()
                    && state.template.key != 0
                    && crate::recurring::valid_last_batch(limits, task.number, state)
                    && crate::recurring::valid_template(
                        limits,
                        task.project,
                        task.authority.budget.spend,
                        &state.template.batch,
                    )
                    && task.numbers.budget == 0
                    && match state.pending_period {
                        Some(period) => period == state.last_period,
                        None => true,
                    }
            }
            None => false,
        },
        Executor::Procedure { code, .. } => {
            code != 0 && task.recurring.is_none() && task.taken_by.is_none() && task.person_wait.is_none()
        }
        Executor::Person(crate::PersonAddress::Person(person)) => {
            person != 0 && task.taken_by.is_none() && task.recurring.is_none() && person_counters(task)
        }
        Executor::Person(crate::PersonAddress::Role(_)) => {
            task.taken_by != Some(0) && task.recurring.is_none() && person_counters(task)
        }
    }
}

fn valid_person_answer(task: &TaskRecord) -> bool {
    let Some(answer) = task.person_answer else { return true };
    let addressed = match task.executor {
        Executor::Person(crate::PersonAddress::Person(person)) => answer.by == person,
        Executor::Person(crate::PersonAddress::Role(_)) => true,
        Executor::Agent { .. } | Executor::Procedure { .. } => false,
    };
    let answered = match task.phase {
        Phase::Closing(_) | Phase::Held { was: Was::Closing(_), .. } | Phase::Ended(_) => true,
        Phase::Waiting | Phase::Active(_) | Phase::Held { was: Was::Waiting | Was::Active(_), .. } => false,
    };
    answer.by != 0 && task.taken_by.is_none() && addressed && answered
}

fn person_counters(task: &TaskRecord) -> bool {
    let waiting = match task.person_wait {
        Some(wait) => {
            wait.stall != skein_lib::Duration::ZERO
                && wait.deadline.as_nanos() == wait.since.as_nanos().saturating_add(wait.stall.as_nanos())
                && match wait.notice {
                    Some(notice) => notice.message != 0 && notice.activity != 0 && notice.activity <= task.activity,
                    None => true,
                }
        }
        None => match task.phase {
            Phase::Active(_) | Phase::Held { was: Was::Active(_), .. } => false,
            Phase::Waiting
            | Phase::Held { was: Was::Waiting | Was::Closing(_), .. }
            | Phase::Closing(_)
            | Phase::Ended(_) => true,
        },
    };
    if !waiting {
        return false;
    }

    task.attempt == 0
        && task.run_spent == 0
        && task.run_reserved == 0
        && task.turn == 0
        && task.last_answer.is_none()
        && !task.ever_turned
        && task.saved.is_empty()
}

pub(crate) fn restore(domain: &mut Domain, env: &Env<Limits>, stored: Stored, out: &mut Queue<Request>) {
    match domain.startup {
        Startup::Checked | Startup::Ready => return failed(domain, None, Refusal::NotReady, out),
        Startup::Failed => return,
        Startup::Restoring => {}
    }
    match stored {
        Stored::PlacedHold(hold) => {
            if !crate::placed::shaped(&hold, &env.limits)
                || domain.placed.contains_key(&hold.number)
                || domain.placed.len() >= env.limits.placed_holds
            {
                return failed(domain, None, Refusal::Restore, out);
            }
            domain.placed.insert(hold.number, *hold).expect("restored hold room");
        }
        Stored::PersonProposal(row) => restore_person_proposal(domain, env, *row, out),
        Stored::Live(task) => {
            let number = task.number;
            if domain.names.contains_key(&number) || domain.tasks.is_full() || !valid_record(domain, &env.limits, &task)
            {
                return failed(domain, Some(number), Refusal::Restore, out);
            }
            let observed_hold = match task.phase {
                Phase::Held { why, .. } => Some(why),
                Phase::Waiting | Phase::Active(_) | Phase::Closing(_) | Phase::Ended(_) => None,
            };
            match task.executor {
                Executor::Agent { .. } => {
                    domain.live_agents = domain.live_agents.checked_add(1).expect("bounded restored agents");
                }
                Executor::Person(_) | Executor::Procedure { .. } => {}
            }
            crate::settings::hold(domain, &task);
            let id = domain
                .tasks
                .insert(Task {
                    published: Some(task.clone()),
                    procedure_offered: None,
                    observed_phase: Some(task.phase.clone()),
                    record: *task,
                    alarm: None,
                    grace_alarm: None,
                    observed_hold,
                })
                .expect("restored task admitted");
            let indexed = domain.names.insert(number, id);
            assert!(indexed == Ok(None), "restored name admitted");
            crate::waits::refresh(domain, number);
        }
        Stored::Stub(stub) => {
            if stub.task == 0
                || stub.result.raw() != stub.task
                || domain.stubs.contains_key(&stub.task)
                || domain.stubs.len() == domain.stubs.capacity()
            {
                return failed(domain, Some(stub.task), Refusal::Restore, out);
            }
            domain.stubs.insert(stub.task, stub).expect("bounded restored stub");
        }
        Stored::Writer(slot) => {
            if !crate::writers::restore(domain, &env.limits, slot) {
                failed(domain, None, Refusal::Restore, out);
            }
        }
        Stored::Pool(row) => {
            if !crate::holds::restore_pool(domain, &env.limits, row) {
                failed(domain, None, Refusal::Restore, out);
            }
        }
        // Historical ended rows stay outside the live arena;
        // they cannot accidentally return an ended task to the live arena.
        Stored::Update(row) => failed(domain, Some(row.task), Refusal::Restore, out),
        Stored::EndedByParty(row) => failed(domain, Some(row.task), Refusal::Restore, out),
        Stored::EndedByProject(row) => failed(domain, Some(row.task), Refusal::Restore, out),
        Stored::EndedDelegate(row) | Stored::Delegate(row) => failed(domain, Some(row.task), Refusal::Restore, out),
        Stored::Result(row) => failed(domain, Some(row.task), Refusal::Restore, out),
        Stored::DecidedProposal(_) | Stored::ClosedLedger(_) => failed(domain, None, Refusal::Restore, out),
        Stored::Ended(task) => failed(domain, Some(task.number), Refusal::Restore, out),
        Stored::Milestone(row) => failed(domain, Some(row.task), Refusal::Restore, out),
        Stored::History(row) => failed(domain, Some(row.task), Refusal::Restore, out),
        Stored::Activity(row) => failed(domain, Some(row.task), Refusal::Restore, out),
        Stored::Ledger(record) => {
            if record.made > env.limits.tree_tasks || !crate::funders::restore_funding(domain, record) {
                failed(domain, None, Refusal::Restore, out);
            }
        }
    }
}

fn restore_person_proposal(
    domain: &mut Domain,
    env: &Env<Limits>,
    row: crate::PersonProposal,
    out: &mut Queue<Request>,
) {
    if !crate::proposals::valid_person_proposal(&row, &env.limits) {
        return failed(domain, None, Refusal::Restore, out);
    }
    for member in row.batch() {
        if !crate::flags::shaped(member.flags, member.executor) {
            return failed(domain, None, Refusal::Restore, out);
        }
    }
    match row.state {
        crate::PersonProposalState::Pending { .. } => {
            if domain.person_proposals.len() >= env.limits.tasks || domain.person_proposals.contains_key(&row.number) {
                return failed(domain, None, Refusal::Restore, out);
            }
            for (_, old) in &domain.person_proposals {
                if old.proposer == row.proposer {
                    return failed(domain, None, Refusal::Restore, out);
                }
            }
            let inserted = domain.person_proposals.insert(row.number, row);
            assert!(inserted == Ok(None), "restored person proposal admitted");
        }
        crate::PersonProposalState::Accepted { .. }
        | crate::PersonProposalState::SettingsAccepted { .. }
        | crate::PersonProposalState::Rejected { .. }
        | crate::PersonProposalState::Withdrawn { .. } => {
            failed(domain, None, Refusal::Restore, out);
        }
    }
}

fn failed(domain: &mut Domain, task: Option<u64>, why: Refusal, out: &mut Queue<Request>) {
    domain.startup = Startup::Failed;
    out.push(Request::RestoreRefused { problem: Problem { task, why, blocked_by: None, capacity: None } });
}

#[expect(clippy::too_many_lines, reason = "restore validates complete task topology and period allotments together")]
fn links(domain: &Domain, env: &Env<Limits>, task: &TaskRecord) -> bool {
    // Virtual entries live on other rows, so validate after every row arrived.
    if !crate::inbox::valid_refusals(domain, task.number, domain.run_message(task.number), &task.refused) {
        return false;
    }
    match task.requester {
        Party::Task(number) => {
            let Some(parent) = record(domain, number) else {
                return false;
            };
            if task.project != parent.project
                || task.root != parent.root
                || parent.depth.checked_add(1) != Some(task.depth)
                || !crate::batch::contains(&parent.delegates, task.number)
            {
                return false;
            }
        }
        Party::Person(_) | Party::Deployment { .. } => {
            if task.root != task.number || task.depth != 0 {
                return false;
            }
        }
    }
    match task.requester {
        Party::Deployment { project } if project != task.project => return false,
        Party::Deployment { .. } | Party::Task(_) | Party::Person(_) => {}
    }
    let Some(root) = record(domain, task.root) else {
        return false;
    };
    let mut archived = false;
    for (funder, _) in &domain.funding {
        match *funder {
            crate::Funder::Recurring { task: source, .. } if source == root.number => archived = true,
            crate::Funder::Task(_)
            | crate::Funder::Pool { .. }
            | crate::Funder::Period { .. }
            | crate::Funder::Recurring { .. } => {}
        }
    }
    let standing = root.recurring.is_none()
        && match root.executor {
            Executor::Procedure { .. } => !root.subscriptions.is_empty() || archived,
            Executor::Agent { .. } | Executor::Person(_) => false,
        };
    let period = crate::funders::original_period(domain, task.number);
    let made = if standing { crate::funders::tree_made(domain, task.number, task.root) } else { Some(root.made) };
    if root.depth != 0
        || match made {
            Some(made) => made < task.made,
            None => true,
        }
    {
        return false;
    }
    let mut project_count = 0_u32;
    let mut subtree = 0_u32;
    for (_, id) in &domain.names {
        let other = &domain.tasks.get(*id).expect("name indexes task").record;
        if other.project == task.project {
            project_count = project_count.saturating_add(1);
        }
        if other.root == task.root && (!standing || crate::funders::original_period(domain, other.number) == period) {
            subtree = subtree.saturating_add(1);
        }
    }
    if project_count > env.limits.project_tasks || subtree > made.expect("checked tree allotment") {
        return false;
    }
    let mut descendants_made = 1_u32;
    for delegate in &task.delegates {
        let Some(child) = record(domain, *delegate) else {
            return false;
        };
        if !standing || crate::funders::original_period(domain, child.number) == period {
            let Some(total) = descendants_made.checked_add(child.made) else {
                return false;
            };
            descendants_made = total;
        }
        if child.requester != Party::Task(task.number) {
            return false;
        }
    }
    if descendants_made > task.made {
        return false;
    }
    let mut stalls = 0_usize;
    for child in &task.delegates {
        if crate::person::credit(record(domain, *child).expect("validated child")) {
            stalls = stalls.checked_add(1).expect("bounded person delegates");
        }
    }
    let mut amendments = 0_usize;
    for word in &task.inbox {
        match word.kind {
            crate::MessageKind::Amendment { .. }
            | crate::MessageKind::FinishRequested { .. }
            | crate::MessageKind::Met { .. } => {
                amendments = amendments.checked_add(1).expect("bounded amendments");
            }
            crate::MessageKind::EffectNotMade { .. }
            | crate::MessageKind::Words
            | crate::MessageKind::PersonStalled { .. }
            | crate::MessageKind::Proposal { .. }
            | crate::MessageKind::Escalation { .. }
            | crate::MessageKind::SettingsDecision { .. }
            | crate::MessageKind::ProposalDecision { .. }
            | crate::MessageKind::Question
            | crate::MessageKind::Answer { .. }
            | crate::MessageKind::Notice { .. }
            | crate::MessageKind::Timer { .. }
            | crate::MessageKind::News { .. }
            | crate::MessageKind::Result(_) => {}
        }
    }
    let Some((subscriptions, _bytes)) = crate::subscriptions::credit(task, &env.limits) else { return false };
    let messages = task
        .inbox
        .len()
        .saturating_sub(amendments)
        .saturating_add(task.delegates.len())
        .saturating_add(task.questions.len())
        .saturating_add(usize::from(task.proposal.is_some()))
        .saturating_add(subscriptions)
        .saturating_add(stalls);
    if messages > usize::try_from(env.limits.inbox_messages).expect("message limit fits") {
        return false;
    }
    for reference in &task.references {
        if let Some(peer) = record(domain, *reference) {
            if peer.project != task.project || !crate::batch::contains(&peer.references, task.number) {
                return false;
            }
        } else if !domain.stubs.contains_key(reference) {
            return false;
        }
    }
    for dependency in &task.waiting_on {
        if !crate::batch::contains(&task.dependencies, *dependency) {
            return false;
        }
        let Some(other) = record(domain, *dependency) else {
            return false;
        };
        if other.project != task.project {
            return false;
        }
    }
    for dependency in &task.dependencies {
        if let Some(other) = record(domain, *dependency)
            && (other.project != task.project || !crate::batch::contains(&task.waiting_on, *dependency))
        {
            return false;
        }
    }
    true
}

pub(crate) fn check(domain: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    match domain.startup {
        Startup::Checked | Startup::Ready | Startup::Failed => return,
        Startup::Restoring => {}
    }
    for (number, _) in &domain.names {
        if !links(domain, env, record(domain, *number).expect("name indexes task"))
            || !crate::funders::representable(domain, *number, 0)
        {
            return failed(domain, Some(*number), Refusal::Restore, out);
        }
    }
    if !crate::placed::check(domain, &env.limits) {
        return failed(domain, None, Refusal::Restore, out);
    }
    for (number, _) in &domain.stubs {
        if domain.names.contains_key(number) || !crate::closing::named_by_live(domain, *number) {
            return failed(domain, Some(*number), Refusal::Restore, out);
        }
    }
    if !crate::writers::valid_restored(domain, env.limits.depth.saturating_add(1))
        || !crate::waits::restore(domain, env, out)
        || !crate::funders::links(domain, env.limits.tasks)
    {
        return failed(domain, None, Refusal::Restore, out);
    }
    domain.startup = Startup::Checked;
}

pub(crate) fn restored(domain: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    match domain.startup {
        Startup::Restoring => return failed(domain, None, Refusal::NotReady, out),
        Startup::Ready | Startup::Failed => return,
        Startup::Checked => {}
    }
    domain.startup = Startup::Ready;
    crate::holds::restore(domain, env);
    crate::subscriptions::restore(domain, env);
    crate::wake::restore(domain, env);
    let numbers = snapshot(domain, env.limits.tasks);
    for number in numbers.into_boxed() {
        crate::grace::schedule(domain, env, number);
        let phase = record(domain, number).expect("restored name exists").phase.clone();
        match record(domain, number).expect("restored name exists").escalation {
            crate::Escalation::Waiting { .. } => out.push(Request::EscalationNeeded {
                context: crate::escalation::context(domain, number).expect("valid held person context"),
            }),
            crate::Escalation::Unheld { .. }
            | crate::Escalation::Routing { .. }
            | crate::Escalation::Rejected { .. } => {}
        }
        match phase {
            Phase::Active(Active::Due | Active::Preparing) => {
                let row = &mut task_mut(domain, number).expect("restored name exists").record;
                let next = crate::run::ends::for_record(row).amended(row.executor, Phase::Active(Active::Due));
                let changed = row.phase != next;
                row.phase = next;
                let due = row.phase == Phase::Active(Active::Due);
                if changed {
                    publish(domain, env, number, out);
                }
                if due {
                    activate(domain, number, out);
                }
            }
            Phase::Active(Active::Claimed { attempt } | Active::Running { attempt }) => {
                out.push(Request::Adopt {
                    task: number,
                    attempt,
                    kept: record(domain, number).expect("restored live task").turn,
                });
            }
            Phase::Active(Active::Idle | Active::BackingOff { .. }) => publish(domain, env, number, out),
            Phase::Closing(closing) => match closing.stage {
                Stage::Run { attempt } => {
                    out.push(Request::Adopt {
                        task: number,
                        attempt,
                        kept: record(domain, number).expect("restored live task").turn,
                    });
                    out.push(Request::Stop { task: number, attempt });
                }
                Stage::Effects => out.push(Request::Close { task: number, ending: closing.ending }),
                Stage::Releases => out.push(Request::Release { task: number, ending: closing.ending }),
                Stage::Delegates | Stage::Settled => {}
            },
            Phase::Held { was, .. } => match was {
                Was::Active(Active::Claimed { attempt } | Active::Running { attempt })
                | Was::Closing(Closing { stage: Stage::Run { attempt }, .. }) => {
                    out.push(Request::Adopt {
                        task: number,
                        attempt,
                        kept: record(domain, number).expect("restored live task").turn,
                    });
                    out.push(Request::Stop { task: number, attempt });
                }
                Was::Closing(Closing { stage: Stage::Effects, ending }) => {
                    out.push(Request::Close { task: number, ending });
                }
                Was::Waiting
                | Was::Active(Active::Idle | Active::Due | Active::Preparing | Active::BackingOff { .. })
                | Was::Closing(Closing { stage: Stage::Delegates | Stage::Releases | Stage::Settled, .. }) => {}
            },
            Phase::Waiting => {}
            Phase::Ended(_) => unreachable!("ended record not restored into live arena"),
        }
    }
}

fn valid_grace(task: &TaskRecord, limits: &Limits) -> bool {
    let Some(grace) = &task.grace else { return true };
    if grace.message == 0
        || grace.message > task.last_message
        || grace.reason.len() > usize::try_from(limits.result_bytes).expect("reason bound")
    {
        return false;
    }
    match grace.by {
        Party::Person(person) => person != 0,
        Party::Task(task) => task != 0,
        Party::Deployment { project } => project == task.project,
    }
}

/// Bounded durable amendment payload shared by both proposal origins.
pub(crate) fn valid_amendment(limits: &Limits, amendment: &crate::Amendment) -> bool {
    (match amendment.addressed {
        Some(change) => crate::control::valid_address(change.address),
        None => true,
    }) && match &amendment.instructions {
        Some(instructions) => instructions.len() <= usize::try_from(limits.message_bytes).expect("u32 fits usize"),
        None => true,
    } && amendment.reason.len() <= usize::try_from(limits.message_bytes).expect("u32 fits usize")
        && match &amendment.spec {
            Some(spec) => crate::batch::valid_spec(limits, spec) && spec.inputs.is_empty(),
            None => true,
        }
        && match &amendment.authority {
            Some(authority) => crate::batch::valid_authority(limits, authority),
            None => true,
        }
        && match &amendment.dependencies {
            Some(dependencies) => dependencies.len() <= usize::try_from(limits.dependencies).expect("u32 fits usize"),
            None => true,
        }
        && match &amendment.wake {
            Some(wake) => crate::wake::valid(wake),
            None => true,
        }
}
