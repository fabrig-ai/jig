//! Pure borrowed durable-row heap measurement for root journal/load admission
//! (domain/tasks.md, section 2; domain/engine.md, sections 5.3 and 5.6).
//! Measures existing ownership without allocating or cloning; shape admission
//! remains with tasks and authority decisions remain with root policy checks.
use crate::{
    Authority, Contract, End, Ending, Holding, Last, Parameter, PersonProposalState, Phase, Proposal, ProposalAction,
    ProposalState, ResultFollowups, Spec, Stored, TaskRecord, TaskResult, Was,
};
use alloc::boxed::Box;
use core::mem::{size_of, size_of_val};

/// Pure borrowed measurement for root journal/load byte admission: counts existing boxed records,
/// slice backing arrays and nested bytes, excluding the inline `Stored` slot. `Ledger` own no heap.
/// Returns `None` on sum overflow; allocates/copies nothing and admits neither shape nor authority.
/// Root separately counts inline slots and queues.
#[must_use]
pub fn stored_bytes(record: &Stored) -> Option<u64> {
    match record {
        Stored::PlacedHold(hold) => bytes(size_of::<crate::PlacedHold>())?
            .checked_add(bytes(hold.reason.len())?)?
            .checked_add(bytes(size_of_val(&*hold.exempt))?),
        Stored::Activity(row) => match &row.line {
            crate::Line::Words { words, kind, .. } => bytes(words.len())?.checked_add(kind_bytes(kind)?),
            crate::Line::Heard(word) | crate::Line::Notified { word, .. } => word_bytes(word),
            crate::Line::Kept { pending, .. } => bytes(size_of_val(&**pending)),
            crate::Line::Decided { .. }
            | crate::Line::Milestone { .. }
            | crate::Line::RunBegan { .. }
            | crate::Line::RunEnded { .. }
            | crate::Line::Turn { .. } => Some(0),
        },
        Stored::Milestone(row) => phase_bytes(&row.phase),
        Stored::PersonProposal(row) | Stored::DecidedProposal(row) => {
            let action = match &row.action {
                crate::PersonAction::Settings { items, .. } => bytes(size_of_val(&**items))?,
                crate::PersonAction::Batch(batch) => batch_bytes(batch)?,
                crate::PersonAction::Amend { amendment, .. } => {
                    amendment_bytes(amendment)?.checked_add(bytes(size_of::<crate::Amendment>())?)?
                }
            };
            let mut total = bytes(size_of::<crate::PersonProposal>())?
                .checked_add(bytes(row.reason.len())?)?
                .checked_add(action)?;
            match &row.state {
                PersonProposalState::Rejected { reason, .. } => {
                    total = total.checked_add(bytes(reason.len())?)?;
                }
                PersonProposalState::Pending { .. }
                | PersonProposalState::Accepted { .. }
                | PersonProposalState::SettingsAccepted { .. }
                | PersonProposalState::Withdrawn { .. } => {}
            }
            Some(total)
        }
        Stored::History(row) => {
            let mut total = bytes(row.reason.len())?;
            if let Some(proposal) = &row.proposal {
                total = total.checked_add(proposal_bytes(proposal)?)?;
            }
            if let Some(amendment) = &row.amendment {
                total = total
                    .checked_add(bytes(size_of::<crate::AmendmentHistory>())?)?
                    .checked_add(settings_bytes(&amendment.before)?)?
                    .checked_add(settings_bytes(&amendment.after)?)?;
            }
            Some(total)
        }
        Stored::Live(task) | Stored::Ended(task) => task_bytes(task),
        Stored::Result(_)
        | Stored::EndedByParty(_)
        | Stored::EndedByProject(_)
        | Stored::EndedDelegate(_)
        | Stored::Delegate(_)
        | Stored::Update(_)
        | Stored::Stub(_)
        | Stored::Ledger(_)
        | Stored::ClosedLedger(_) => Some(0),
        Stored::Writer(slot) => {
            let mut total = bytes(size_of_val(&*slot.resource.path))?;
            for segment in &slot.resource.path {
                total = total.checked_add(bytes(segment.len())?)?;
            }
            Some(total)
        }
        Stored::Pool(row) => {
            let mut total = bytes(size_of_val(&*row.pool.path))?;
            for segment in &row.pool.path {
                total = total.checked_add(bytes(segment.len())?)?;
            }
            Some(total)
        }
    }
}

/// Borrowed heap bytes retained by one fenced terminal proof.
#[must_use]
pub fn terminal_bytes(end: &End) -> Option<u64> {
    match end {
        End::Finished { result, .. } => result_bytes(result),
        End::FinishedWithFollowups { result, followups } => {
            let result = result_bytes(result)?;
            let followups = match followups {
                ResultFollowups::Delegates(batch) => batch_bytes(batch)?,
                ResultFollowups::Proposal(proposal) => proposal_bytes(proposal)?,
            };
            result.checked_add(followups)
        }
        End::Exceeded(_) | End::Parked | End::Failed(_) | End::Refused | End::Recheck => Some(0),
    }
}

fn batch_bytes(batch: &[crate::New]) -> Option<u64> {
    let mut total = bytes(size_of_val(batch))?;
    for member in batch {
        total = total
            .checked_add(spec_bytes(&member.spec)?)?
            .checked_add(authority_bytes(&member.authority)?)?
            .checked_add(contract_bytes(&member.contract)?)?
            .checked_add(bytes(size_of_val(&*member.dependencies))?)?
            .checked_add(holdings_bytes(&member.holdings)?)?
            .checked_add(llm_bytes(member.llm.as_ref())?)?;
    }
    Some(total)
}

/// The sub-agents' array an LLM choice owns.
fn llm_bytes(llm: Option<&crate::LlmChoice>) -> Option<u64> {
    match llm {
        Some(choice) => bytes(size_of_val(&*choice.subagents)),
        None => Some(0),
    }
}

fn bytes(length: usize) -> Option<u64> {
    u64::try_from(length).ok()
}

fn holdings_bytes(holdings: &[Holding]) -> Option<u64> {
    let mut total = bytes(size_of_val(holdings))?;
    for holding in holdings {
        let path = match holding {
            Holding::Write { resource, .. } => &resource.path,
            Holding::Slot { pool, .. } => &pool.path,
        };
        total = total.checked_add(bytes(size_of_val(&**path))?)?;
        for segment in path {
            total = total.checked_add(bytes(segment.len())?)?;
        }
    }
    Some(total)
}

fn spec_bytes(spec: &Spec) -> Option<u64> {
    let mut total = bytes(spec.words.len())?
        .checked_add(bytes(size_of_val(&*spec.parameters))?)?
        .checked_add(bytes(size_of_val(&*spec.inputs))?)?;
    for parameter in &spec.parameters {
        match parameter {
            Parameter::Bytes { value, .. } => total = total.checked_add(bytes(value.len())?)?,
            Parameter::Number { .. } | Parameter::Resource { .. } => {}
        }
    }
    Some(total)
}

fn authority_bytes(authority: &Authority) -> Option<u64> {
    let mut total = bytes(size_of_val(&*authority.grants))?
        .checked_add(bytes(size_of_val(&*authority.note_resources))?)?
        .checked_add(bytes(size_of_val(&*authority.delegation.kinds))?)?;
    for grant in &authority.grants {
        total = total.checked_add(bytes(size_of_val(&*grant.pattern.segments))?)?;
        for segment in &grant.pattern.segments {
            total = total.checked_add(bytes(segment.len())?)?;
        }
        match &grant.pattern.last {
            Last::Exact(word) | Last::Open(word) => total = total.checked_add(bytes(word.len())?)?,
        }
    }
    for scope in &authority.note_resources {
        total = total.checked_add(bytes(size_of_val(&*scope.pattern.segments))?)?;
        for segment in &scope.pattern.segments {
            total = total.checked_add(bytes(segment.len())?)?;
        }
        match &scope.pattern.last {
            Last::Exact(word) | Last::Open(word) => total = total.checked_add(bytes(word.len())?)?,
        }
    }
    Some(total)
}

fn contract_bytes(contract: &Contract) -> Option<u64> {
    match contract {
        Contract::Approval { state, part } => {
            let mut total = bytes(state.len())?;
            match &part.key {
                crate::PartKey::Number(_) => {}
                crate::PartKey::Path(segments) => {
                    total = total.checked_add(skein_lib::List::<Box<[u8]>>::worst_case(segments.capacity())?)?;
                    for segment in segments {
                        total = total.checked_add(bytes(segment.len())?)?;
                    }
                }
            }
            Some(total)
        }
        Contract::Verdict { choices } => {
            let mut total = bytes(size_of_val(&**choices))?;
            for choice in choices {
                total = total.checked_add(bytes(size_of_val(&*choice.fields))?)?;
                for field in &choice.fields {
                    total = total.checked_add(bytes(field.name.len())?)?;
                }
            }
            Some(total)
        }
        Contract::Report { .. } | Contract::Change { .. } => Some(0),
    }
}

fn kind_bytes(kind: &crate::MessageKind) -> Option<u64> {
    match kind {
        crate::MessageKind::Result(crate::ResultKind::Verdict { fields, .. }) => {
            bytes(crate::verdict::fields_bytes(fields)?)?.checked_add(bytes(size_of_val(&**fields))?)
        }
        crate::MessageKind::Result(crate::ResultKind::Approved { state, .. }) => bytes(state.len()),
        crate::MessageKind::Result(
            crate::ResultKind::Report
            | crate::ResultKind::Change { .. }
            | crate::ResultKind::Failed
            | crate::ResultKind::Cancelled,
        )
        | crate::MessageKind::Escalation { .. }
        | crate::MessageKind::Proposal { .. }
        | crate::MessageKind::PersonStalled { .. }
        | crate::MessageKind::SettingsDecision { .. }
        | crate::MessageKind::ProposalDecision { .. }
        | crate::MessageKind::Words
        | crate::MessageKind::EffectNotMade { .. }
        | crate::MessageKind::FinishRequested { .. }
        | crate::MessageKind::Amendment { .. }
        | crate::MessageKind::Met { .. }
        | crate::MessageKind::QuestionAnswered { .. }
        | crate::MessageKind::QuestionOwed
        | crate::MessageKind::QuestionEnded { .. }
        | crate::MessageKind::Question
        | crate::MessageKind::Answer { .. }
        | crate::MessageKind::Notice { .. }
        | crate::MessageKind::Timer { .. }
        | crate::MessageKind::News { .. } => Some(0),
    }
}

fn word_bytes(word: &crate::Word) -> Option<u64> {
    let payload = bytes(word.payload_bytes()?)?;
    match &word.kind {
        crate::MessageKind::Result(crate::ResultKind::Verdict { fields, .. }) => {
            payload.checked_add(bytes(size_of_val(&**fields))?)
        }
        crate::MessageKind::Result(
            crate::ResultKind::Approved { .. }
            | crate::ResultKind::Report
            | crate::ResultKind::Change { .. }
            | crate::ResultKind::Failed
            | crate::ResultKind::Cancelled,
        )
        | crate::MessageKind::Escalation { .. }
        | crate::MessageKind::Proposal { .. }
        | crate::MessageKind::PersonStalled { .. }
        | crate::MessageKind::SettingsDecision { .. }
        | crate::MessageKind::ProposalDecision { .. }
        | crate::MessageKind::Words
        | crate::MessageKind::EffectNotMade { .. }
        | crate::MessageKind::FinishRequested { .. }
        | crate::MessageKind::Amendment { .. }
        | crate::MessageKind::Met { .. }
        | crate::MessageKind::QuestionAnswered { .. }
        | crate::MessageKind::QuestionOwed
        | crate::MessageKind::QuestionEnded { .. }
        | crate::MessageKind::Question
        | crate::MessageKind::Answer { .. }
        | crate::MessageKind::Notice { .. }
        | crate::MessageKind::Timer { .. }
        | crate::MessageKind::News { .. } => Some(payload),
    }
}

fn result_bytes(result: &TaskResult) -> Option<u64> {
    match result {
        TaskResult::Report { words } | TaskResult::Change { words, .. } => bytes(words.len()),
        TaskResult::Verdict { fields, words, .. } => bytes(words.len())?
            .checked_add(bytes(crate::verdict::fields_bytes(fields)?)?)?
            .checked_add(bytes(size_of_val(&**fields))?),
        TaskResult::Approved { state, words } => bytes(state.len())?.checked_add(match words {
            Some(words) => bytes(words.len())?,
            None => 0,
        }),
        TaskResult::Failure { reason } => bytes(reason.len()),
    }
}

fn ending_bytes(ending: &Ending) -> Option<u64> {
    match ending {
        Ending::Done(result) => result_bytes(result),
        Ending::Failed { reason } => bytes(reason.len()),
        Ending::Cancelled { reason, result, .. } => {
            let reason = bytes(reason.len())?;
            match result {
                Some(result) => reason.checked_add(result_bytes(result)?),
                None => Some(reason),
            }
        }
    }
}

/// Deep owned bytes of a task phase, excluding its inline slot.
#[must_use]
pub fn phase_bytes(phase: &Phase) -> Option<u64> {
    match phase {
        Phase::Closing(closing) => ending_bytes(&closing.ending),
        Phase::Ended(ending) => ending_bytes(ending),
        Phase::Held { was, source, .. } => {
            let mut total = match was {
                Was::Closing(closing) => ending_bytes(&closing.ending)?,
                Was::Waiting | Was::Active(_) => 0,
            };
            if let Some(source) = source {
                total = total.checked_add(bytes(size_of::<crate::HoldSource>())?)?;
                match &**source {
                    crate::HoldSource::Resource(name) => {
                        total = total.checked_add(bytes(size_of_val(&*name.path))?)?;
                        for segment in &name.path {
                            total = total.checked_add(bytes(segment.len())?)?;
                        }
                    }
                    crate::HoldSource::Effect { .. } => {}
                    crate::HoldSource::Circle(circle) => {
                        total = total.checked_add(bytes(size_of_val(&*circle.tasks))?)?;
                    }
                }
            }
            Some(total)
        }
        Phase::Waiting | Phase::Active(_) => Some(0),
    }
}

fn proposal_bytes(proposal: &Proposal) -> Option<u64> {
    let mut total = bytes(size_of::<Proposal>())?.checked_add(bytes(proposal.reason.len())?)?;
    match &proposal.state {
        ProposalState::Rejected { reason, .. } => {
            total = total.checked_add(bytes(reason.len())?)?;
        }
        ProposalState::Pending { .. } | ProposalState::Accepted { .. } | ProposalState::Withdrawn => {}
    }
    match &proposal.action {
        ProposalAction::Settings { items, .. } => {
            total = total.checked_add(bytes(size_of_val(items.as_ref()))?)?;
        }
        ProposalAction::Batch(batch) => {
            total = total.checked_add(bytes(size_of_val(&**batch))?)?;
            for member in batch {
                total = total
                    .checked_add(spec_bytes(&member.spec)?)?
                    .checked_add(authority_bytes(&member.authority)?)?
                    .checked_add(contract_bytes(&member.contract)?)?
                    .checked_add(bytes(size_of_val(&*member.dependencies))?)?
                    .checked_add(holdings_bytes(&member.holdings)?)?;
            }
        }
        ProposalAction::Amend { amendment, .. } => {
            total = total.checked_add(amendment_bytes(amendment)?)?;
        }
        ProposalAction::Effect { authority, .. }
        | ProposalAction::ProcedureEffect { authority, .. }
        | ProposalAction::Widen { authority, .. } => {
            total = total.checked_add(authority_bytes(authority)?)?;
        }
        ProposalAction::Release { .. } | ProposalAction::ReleasePlaced { .. } => {}
    }
    Some(total)
}

/// Borrowed complete heap footprint of one task record, including its boxed carrier.
#[must_use]
pub fn task_bytes(task: &TaskRecord) -> Option<u64> {
    let mut inbox_bytes = bytes(size_of_val(&*task.inbox))?.checked_add(bytes(size_of_val(&*task.refused))?)?;
    for word in &task.inbox {
        inbox_bytes = inbox_bytes.checked_add(word_bytes(word)?)?;
    }
    let mut saved_bytes = bytes(size_of_val(&*task.saved))?;
    for resource in &task.saved {
        saved_bytes = saved_bytes.checked_add(bytes(size_of_val(&*resource.path))?)?;
        for segment in &resource.path {
            saved_bytes = saved_bytes.checked_add(bytes(segment.len())?)?;
        }
    }
    // The inline record includes Spec::asks and the durable asked_requester latch.
    bytes(size_of::<TaskRecord>())?
        .checked_add(match &task.grace {
            Some(grace) => bytes(grace.reason.len())?,
            None => 0,
        })?
        .checked_add(match &task.recurring {
            Some(state) => {
                let mut total = bytes(size_of::<crate::RecurringState>())?
                    .checked_add(bytes(size_of_val(&*state.template.batch))?)?
                    .checked_add(bytes(size_of_val(&*state.last_batch))?)?;
                for member in &state.template.batch {
                    total = total
                        .checked_add(spec_bytes(&member.spec)?)?
                        .checked_add(authority_bytes(&member.authority)?)?
                        .checked_add(contract_bytes(&member.contract)?)?
                        .checked_add(bytes(size_of_val(&*member.dependencies))?)?
                        .checked_add(holdings_bytes(&member.holdings)?)?
                        .checked_add(llm_bytes(member.llm.as_ref())?)?;
                }
                total
            }
            None => 0,
        })?
        .checked_add(spec_bytes(&task.spec)?)?
        .checked_add(match &task.instructions {
            Some(instructions) => bytes(instructions.len())?,
            None => 0,
        })?
        .checked_add(llm_bytes(task.llm.as_ref())?)?
        .checked_add(authority_bytes(&task.authority)?)?
        .checked_add(contract_bytes(&task.contract)?)?
        .checked_add(phase_bytes(&task.phase)?)?
        .checked_add(match &task.escalation {
            crate::Escalation::Rejected { reason, .. } => bytes(reason.len())?,
            crate::Escalation::Unheld { .. }
            | crate::Escalation::Routing { .. }
            | crate::Escalation::Waiting { .. } => 0,
        })?
        .checked_add(match &task.proposal {
            Some(proposal) => proposal_bytes(proposal)?,
            None => 0,
        })?
        .checked_add(bytes(size_of_val(&*task.dependencies))?)?
        .checked_add(bytes(size_of_val(&*task.added))?)?
        .checked_add(holdings_bytes(&task.holdings)?)?
        .checked_add(bytes(size_of_val(&*task.delegates))?)?
        .checked_add(bytes(size_of_val(&*task.result_followups))?)?
        .checked_add(bytes(size_of_val(&*task.references))?)?
        .checked_add(bytes(size_of_val(&*task.questions))?)?
        .checked_add(bytes(size_of_val(&*task.subscriptions))?)?
        .checked_add(bytes(size_of_val(&*task.waiting_on))?)?
        .checked_add(bytes(size_of_val(&*task.replies))?)?
        .checked_add(bytes(size_of_val(&*task.question_lines))?)?
        .checked_add(bytes(size_of_val(&*task.proposal_rows))?)?
        .checked_add(inbox_bytes)?
        .checked_add(saved_bytes)
}

fn settings_bytes(value: &crate::Settings) -> Option<u64> {
    spec_bytes(&value.spec)?
        .checked_add(match &value.instructions {
            Some(instructions) => bytes(instructions.len())?,
            None => 0,
        })?
        .checked_add(authority_bytes(&value.authority)?)?
        .checked_add(bytes(size_of_val(&*value.dependencies))?)?
        .checked_add(llm_bytes(value.llm.as_ref())?)
}

fn amendment_bytes(amendment: &crate::Amendment) -> Option<u64> {
    let mut total = bytes(amendment.reason.len())?;
    if let Some(instructions) = &amendment.instructions {
        total = total.checked_add(bytes(instructions.len())?)?;
    }
    if let Some(spec) = &amendment.spec {
        total = total.checked_add(spec_bytes(spec)?)?;
    }
    if let Some(authority) = &amendment.authority {
        total = total.checked_add(authority_bytes(authority)?)?;
    }
    if let Some(dependencies) = &amendment.dependencies {
        total = total.checked_add(bytes(size_of_val(&**dependencies))?)?;
    }
    total = total.checked_add(llm_bytes(amendment.llm.as_ref())?)?;
    Some(total)
}
