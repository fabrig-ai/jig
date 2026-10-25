//! Bounded task messages and atomic turn reads (domain/tasks.md, section 8.2).
#![expect(clippy::result_large_err, reason = "bounded owner refusal carries exact capacity diagnostics inline")]
use crate::domain::{Domain, publish, record, task_mut};
use crate::{Limits, MessageKind, NewsClass, Party, Phase, QuestionCredit, Refusal, Request, Word};
use alloc::boxed::Box;
use skein_lib::{Env, List, Queue, ReplyTo};

/// The three ways a whole message enters a bounded inbox (domain/tasks.md, 8.2).
#[derive(Clone, Copy)]
pub(crate) enum Admission {
    /// The sender may be refused before its decision is committed.
    Refusable,
    /// Its sender already kept one slot and its maximum bytes.
    RoomKept,
    /// A subscription or amendment replaces its earlier unread hint.
    Merged,
}

pub(crate) struct Prepared {
    pub previous: Option<u64>,
    pub word: Word,
    /// Admission before unread hint merging changes its time, kind or hit count.
    pub activity_word: Word,
    pub inbox: Box<[Word]>,
}

fn merge_key(kind: &MessageKind) -> Option<(u8, u64)> {
    match kind {
        MessageKind::Notice { subscription, .. }
        | MessageKind::Timer { subscription }
        | MessageKind::News { subscription, .. } => Some((1, *subscription)),
        MessageKind::Amendment { .. } => Some((0, 0)),
        MessageKind::Met { .. } => Some((2, 0)),
        MessageKind::FinishRequested { .. }
        | MessageKind::EffectNotMade { .. }
        | MessageKind::Words
        | MessageKind::Question
        | MessageKind::Answer { .. }
        | MessageKind::Proposal { .. }
        | MessageKind::Escalation { .. }
        | MessageKind::ProposalDecision { .. }
        | MessageKind::PersonStalled { .. }
        | MessageKind::Result(_) => None,
    }
}

/// Build one admitted inbox update before its owner commits any state. A
/// reserved or merged message is never refused after its sender committed.
pub(crate) fn prepare(
    domain: &Domain,
    limits: &Limits,
    number: u64,
    word: Word,
    admission: Admission,
) -> Option<Prepared> {
    prepare_staged(domain, &crate::compound::Staged::new(0), limits, number, word, admission)
}

pub(crate) fn prepare_staged(
    domain: &Domain,
    staged: &crate::compound::Staged,
    limits: &Limits,
    number: u64,
    mut word: Word,
    admission: Admission,
) -> Option<Prepared> {
    let task = staged.record(domain, number)?;
    if word.number == 0 || word.number <= task.last_message {
        return None;
    }
    match admission {
        Admission::Refusable => {
            let mut unanswered = 0_usize;
            for line in &task.question_lines {
                if line.answer.is_none() {
                    unanswered = unanswered.checked_add(1).expect("bounded question positions");
                }
            }
            if (word.kind == MessageKind::Question
                && unanswered >= usize::try_from(limits.inbox_messages).expect("u32 fits usize"))
                || !room_staged(domain, staged, limits, number, 1, word.words.len(), false)
            {
                return None;
            }
        }
        Admission::RoomKept | Admission::Merged => {}
    }
    let activity_word = word.clone();
    let previous = if task.last_message == 0 { None } else { Some(task.last_message) };
    let key = match admission {
        Admission::Merged => merge_key(&word.kind),
        Admission::Refusable | Admission::RoomKept => None,
    };
    let mut inbox = List::with_capacity(limits.inbox_messages.saturating_add(3));
    for old in task.inbox {
        if key.is_some()
            && key == merge_key(&old.kind)
            && !match &task.grace {
                Some(grace) => grace.message == old.number,
                None => false,
            }
        {
            // The hint offered to a procedure belongs to that step. A later
            // hint stays unread and begins a fresh batch after its fence.
            match domain.procedure_offered(number) {
                Some(offered) if old.number <= offered => continue,
                Some(_) | None => {}
            }
            word.at = old.at;
            word.hits = old.hits.saturating_add(1);
            word.eligible = old.eligible;
            // A later quiet hint cannot erase an earlier unread wake.
            match old.kind {
                MessageKind::News { subscription, class: NewsClass::Wakes } => {
                    word.kind = MessageKind::News { subscription, class: NewsClass::Wakes };
                }
                MessageKind::EffectNotMade { .. }
                | MessageKind::Words
                | MessageKind::Question
                | MessageKind::Answer { .. }
                | MessageKind::Proposal { .. }
                | MessageKind::Escalation { .. }
                | MessageKind::ProposalDecision { .. }
                | MessageKind::PersonStalled { .. }
                | MessageKind::Result(_)
                | MessageKind::Notice { .. }
                | MessageKind::Timer { .. }
                | MessageKind::FinishRequested { .. }
                | MessageKind::Amendment { .. }
                | MessageKind::Met { .. }
                | MessageKind::News { class: NewsClass::Kept | NewsClass::Dropped, .. } => {}
            }
        } else {
            inbox.push(old.clone()).expect("admitted inbox subset");
        }
    }
    inbox.push(word.clone()).expect("admitted inbox room");
    Some(Prepared { previous, word, activity_word, inbox: inbox.into_boxed() })
}

/// Account for messages already waiting and the result credit reserved for
/// every live direct delegate (domain/tasks.md, section 8.2).
pub(crate) fn room(domain: &Domain, limits: &Limits, task: u64, count: u32, bytes: usize) -> bool {
    room_staged(domain, &crate::compound::Staged::new(0), limits, task, count, bytes, false)
}

pub(crate) fn projection_room(domain: &Domain, limits: &Limits, task: u64, bytes: u32) -> bool {
    let Some(row) = record(domain, task) else { return false };
    room_staged(
        domain,
        &crate::compound::Staged::new(0),
        limits,
        task,
        1,
        usize::try_from(bytes).expect("u32 fits usize"),
        row.tracked.is_some(),
    )
}

#[expect(clippy::too_many_arguments, reason = "the shared credit calculation admits the reserved core hint")]
fn room_staged(
    domain: &Domain,
    staged: &crate::compound::Staged,
    limits: &Limits,
    task: u64,
    count: u32,
    bytes: usize,
    projection: bool,
) -> bool {
    let Some(record) = staged.record(domain, task) else {
        return false;
    };
    let mut stall_credits = 0_usize;
    let mut delegates = 0_usize;
    for child in &record.delegates {
        if staged.detached(*child) {
            continue;
        }
        delegates = delegates.checked_add(1).expect("delegate bound");
        if crate::person::credit(crate::domain::record(domain, *child).expect("live delegate")) {
            stall_credits = stall_credits.checked_add(1).expect("bounded person delegates");
        }
    }
    let Some(reserved) = delegates.checked_add(stall_credits) else { return false };
    let Some(reserved) = reserved.checked_add(record.questions.len()) else { return false };
    let Some(reserved) = reserved.checked_add(usize::from(record.proposal.is_some())) else { return false };
    let Some((mut subscription_count, mut subscription_bytes)) = crate::subscriptions::credit(&record, limits) else {
        return false;
    };
    let mut core_count = 0_usize;
    let mut core_bytes = 0_usize;
    if projection {
        for word in record.inbox {
            if merge_key(&word.kind) == Some((1, 0)) {
                let Some(next) = core_count.checked_add(1) else { return false };
                core_count = next;
                let Some(bytes) = word.payload_bytes() else { return false };
                let Some(next) = core_bytes.checked_add(bytes) else { return false };
                core_bytes = next;
            }
        }
        if core_count == 0 {
            let Some(next) = subscription_count.checked_sub(1) else { return false };
            subscription_count = next;
            let Some(next) =
                subscription_bytes.checked_sub(usize::try_from(limits.message_bytes).expect("u32 fits usize"))
            else {
                return false;
            };
            subscription_bytes = next;
        }
    }
    let Some(total_count) = record.inbox.len().saturating_sub(core_count).checked_add(reserved) else {
        return false;
    };
    // Amendment, grace and Met each have a separately priced slot. A notice
    // that the task may go on cannot consume a sender's room (domain/tasks.md, 5.7 and 8.2).
    let mut extra_count = 0_usize;
    let mut extra_bytes = 0_usize;
    for word in record.inbox {
        match word.kind {
            MessageKind::Amendment { .. } | MessageKind::FinishRequested { .. } | MessageKind::Met { .. } => {
                extra_count = extra_count.checked_add(1).expect("bounded merged slots");
                extra_bytes = extra_bytes.checked_add(word.words.len()).expect("bounded amendment bytes");
            }
            MessageKind::EffectNotMade { .. }
            | MessageKind::Words
            | MessageKind::Question
            | MessageKind::Answer { .. }
            | MessageKind::Proposal { .. }
            | MessageKind::Escalation { .. }
            | MessageKind::ProposalDecision { .. }
            | MessageKind::PersonStalled { .. }
            | MessageKind::Result(_)
            | MessageKind::Notice { .. }
            | MessageKind::Timer { .. }
            | MessageKind::News { .. } => {}
        }
    }
    let total_count = total_count.saturating_sub(extra_count);
    let Some(total_count) = total_count.checked_add(subscription_count) else { return false };
    let Some(total_count) = total_count.checked_add(usize::try_from(count).expect("u32 fits usize")) else {
        return false;
    };
    let mut total_bytes = bytes;
    for message in record.inbox {
        let Some(sum) = total_bytes.checked_add(match message.payload_bytes() {
            Some(bytes) => bytes,
            None => return false,
        }) else {
            return false;
        };
        total_bytes = sum;
    }
    let Some(result_bytes) = delegates.checked_mul(usize::try_from(limits.result_bytes).expect("u32 fits usize"))
    else {
        return false;
    };
    let Some(answer_bytes) =
        record.questions.len().checked_mul(usize::try_from(limits.message_bytes).expect("u32 fits usize"))
    else {
        return false;
    };
    let total_bytes = total_bytes
        .checked_sub(extra_bytes)
        .expect("separate slots included")
        .checked_sub(core_bytes)
        .expect("core hint counted");
    let Some(total_bytes) = total_bytes.checked_add(result_bytes) else { return false };
    let Some(total_bytes) = total_bytes.checked_add(answer_bytes) else {
        return false;
    };
    let proposal_bytes =
        if record.proposal.is_some() { usize::try_from(limits.message_bytes).expect("u32 fits usize") } else { 0 };
    let Some(total_bytes) = total_bytes.checked_add(proposal_bytes) else { return false };
    let Some(total_bytes) = total_bytes.checked_add(subscription_bytes) else { return false };
    total_count <= usize::try_from(limits.inbox_messages).expect("u32 fits usize")
        && total_bytes <= usize::try_from(limits.inbox_bytes).expect("u32 fits usize")
}

#[expect(clippy::too_many_arguments, reason = "one message entrance carries its addressed word and authorization")]
pub(crate) fn message(
    domain: &mut Domain,
    env: &Env<Limits>,
    to: ReplyTo,
    project: u32,
    number: u64,
    word: Word,
    authorized: bool,
    out: &mut Queue<Request>,
) {
    match check_message(domain, &crate::compound::Staged::new(0), env, Message { project, number, word, authorized }) {
        Ok(plan) => {
            let (task, word) = apply_message(domain, env, plan, out);
            out.push(Request::Sent { reply_to: to, task, word });
        }
        Err(problem) => crate::capacity::refused(domain, env, to, *problem, out),
    }
}

pub(crate) struct Message {
    pub project: u32,
    pub number: u64,
    pub word: Word,
    pub authorized: bool,
}

pub(crate) struct MessagePlan {
    pub(crate) number: u64,
    pub(crate) prepared: Prepared,
    pub(crate) answer: Option<u64>,
}

#[expect(clippy::too_many_lines, reason = "one inbox entrance preflights message and answer credits before mutation")]
pub(crate) fn check_message(
    domain: &Domain,
    staged: &crate::compound::Staged,
    env: &Env<Limits>,
    message: Message,
) -> Result<MessagePlan, Box<crate::Problem>> {
    let Message { project, number, word, authorized } = message;
    if !domain.ready() {
        return Err(Box::new(crate::Problem {
            task: Some(number),
            why: Refusal::NotReady,
            blocked_by: None,
            capacity: None,
        }));
    }
    let Some(task) = staged.record(domain, number) else {
        return Err(Box::new(crate::Problem {
            task: Some(number),
            why: Refusal::Unknown,
            blocked_by: None,
            capacity: None,
        }));
    };
    let requester = match word.from {
        Party::Person(person) => task.requester == Party::Person(person) || authorized && person != 0,
        Party::Task(source) => crate::refs::allows(domain, source, number),
        Party::Deployment { .. } => false,
    };
    let sendable = match word.kind {
        MessageKind::Words | MessageKind::Question | MessageKind::Answer { .. } => true,
        MessageKind::EffectNotMade { .. }
        | MessageKind::Proposal { .. }
        | MessageKind::Escalation { .. }
        | MessageKind::FinishRequested { .. }
        | MessageKind::Amendment { .. }
        | MessageKind::Met { .. }
        | MessageKind::ProposalDecision { .. }
        | MessageKind::PersonStalled { .. }
        | MessageKind::Result(_)
        | MessageKind::Notice { .. }
        | MessageKind::Timer { .. }
        | MessageKind::News { .. } => false,
    };
    if task.project != project || !requester || !sendable {
        return Err(Box::new(crate::Problem {
            task: Some(number),
            why: Refusal::State,
            blocked_by: None,
            capacity: None,
        }));
    }
    let available = match task.phase {
        Phase::Active(_) | Phase::Waiting | Phase::Held { was: crate::Was::Waiting | crate::Was::Active(_), .. } => {
            true
        }
        Phase::Closing(_) | Phase::Held { was: crate::Was::Closing(_), .. } | Phase::Ended(_) => false,
    };
    let person_waiting = match word.from {
        Party::Person(_) => match task.phase {
            Phase::Active(_) | Phase::Held { was: crate::Was::Active(_), .. } => false,
            Phase::Waiting
            | Phase::Closing(_)
            | Phase::Held { was: crate::Was::Waiting | crate::Was::Closing(_), .. }
            | Phase::Ended(_) => true,
        },
        Party::Task(_) | Party::Deployment { .. } => false,
    };
    if !available || person_waiting {
        return Err(Box::new(crate::Problem {
            task: Some(number),
            why: Refusal::State,
            blocked_by: None,
            capacity: None,
        }));
    }
    if word.number == 0
        || word.number <= task.last_message
        || word.words.is_empty()
        || word.words.len() > usize::try_from(env.limits.message_bytes).expect("u32 fits usize")
    {
        return Err(Box::new(crate::Problem {
            task: Some(number),
            why: Refusal::Read,
            blocked_by: None,
            capacity: None,
        }));
    }
    let answer = match word.kind {
        MessageKind::Words | MessageKind::Question => None,
        MessageKind::Answer { question } => {
            let valid = match word.from {
                Party::Task(answerer) => {
                    let mut found = false;
                    for credit in &task.questions {
                        if credit.number == question && answerer == credit.answerer {
                            found = true;
                        }
                    }
                    found
                }
                Party::Person(person) => {
                    let mut found = false;
                    if task.requester == Party::Person(person) {
                        for line in &task.question_lines {
                            if line.message == question && line.answer.is_none() {
                                found = true;
                            }
                        }
                        if staged.answered_question(number, question) {
                            found = false;
                        }
                    }
                    found
                }
                Party::Deployment { .. } => false,
            };
            if !valid {
                return Err(Box::new(crate::Problem {
                    task: Some(number),
                    why: Refusal::Reference,
                    blocked_by: None,
                    capacity: None,
                }));
            }
            match word.from {
                Party::Task(_) => Some(question),
                Party::Person(_) | Party::Deployment { .. } => None,
            }
        }
        MessageKind::EffectNotMade { .. }
        | MessageKind::Proposal { .. }
        | MessageKind::Escalation { .. }
        | MessageKind::FinishRequested { .. }
        | MessageKind::Amendment { .. }
        | MessageKind::Met { .. }
        | MessageKind::ProposalDecision { .. }
        | MessageKind::PersonStalled { .. }
        | MessageKind::Result(_)
        | MessageKind::Notice { .. }
        | MessageKind::Timer { .. }
        | MessageKind::News { .. } => {
            unreachable!("root hints have reserved entrances")
        }
    };
    let admission = match answer {
        Some(_) => Admission::RoomKept,
        None => Admission::Refusable,
    };
    let capacity = crate::Capacity {
        project: Some(project),
        by: word.from,
        action: crate::CapacityAction::Message { task: number, number: word.number },
        cause: crate::capacity::inbox(number, &env.limits),
    };
    let Some(prepared) = prepare_staged(domain, staged, &env.limits, number, word, admission) else {
        return Err(Box::new(crate::Problem {
            task: Some(number),
            why: Refusal::Busy,
            blocked_by: None,
            capacity: Some(capacity),
        }));
    };
    if prepared.word.kind == MessageKind::Question {
        let source = match prepared.word.from {
            Party::Task(source) => source,
            Party::Person(_) | Party::Deployment { .. } => {
                return Err(Box::new(crate::Problem {
                    task: Some(number),
                    why: Refusal::Reference,
                    blocked_by: None,
                    capacity: None,
                }));
            }
        };
        if !room(domain, &env.limits, source, 1, usize::try_from(env.limits.message_bytes).expect("u32 fits usize")) {
            return Err(Box::new(crate::Problem {
                task: Some(source),
                why: Refusal::Busy,
                blocked_by: None,
                capacity: Some(crate::Capacity { cause: crate::capacity::inbox(source, &env.limits), ..capacity }),
            }));
        }
    }
    Ok(MessagePlan { number, prepared, answer })
}

pub(crate) fn apply_message(
    domain: &mut Domain,
    env: &Env<Limits>,
    plan: MessagePlan,
    out: &mut Queue<Request>,
) -> (u64, Word) {
    let MessagePlan { number, prepared, answer } = plan;
    let sender = prepared.word.from;
    let task = task_mut(domain, number).expect("admitted task still live");
    task.record.last_activity = env.wall;
    task.record.last_message = prepared.word.number;
    task.record.inbox = prepared.inbox;
    crate::activity::message(&mut task.record, &prepared.activity_word, env.limits.inbox_messages, out);
    if let Some(question) = answer {
        let mut credits = List::with_capacity(env.limits.inbox_messages);
        for credit in &task.record.questions {
            if credit.number != question {
                credits.push(*credit).expect("credit subset bounded");
            }
        }
        task.record.questions = credits.into_boxed();
        let heard = crate::AnswerLine { task: number, position: task.record.activity };
        crate::positions::sent_answer(domain, env, sender, question, heard, out);
    }
    publish(domain, env, number, out);
    if prepared.word.kind == MessageKind::Question {
        let source = match prepared.word.from {
            Party::Task(source) => source,
            Party::Person(_) | Party::Deployment { .. } => unreachable!("question source checked"),
        };
        let source_row = task_mut(domain, source).expect("referenced questioner remains live");
        let mut credits = List::with_capacity(env.limits.inbox_messages);
        for credit in &source_row.record.questions {
            credits.push(*credit).expect("question credits bounded");
        }
        credits.push(QuestionCredit { number: prepared.word.number, answerer: number }).expect("answer room reserved");
        source_row.record.questions = credits.into_boxed();
        publish(domain, env, source, out);
    }
    crate::wake::after_message(domain, env, number, prepared.previous, prepared.word.clone(), out);
    (number, prepared.word)
}

/// Root's same-decision result handoff after a delegate settled. Its reserved
/// inbox credit cannot be consumed by ordinary words.
pub(crate) fn delegate_result(
    domain: &mut Domain,
    env: &Env<Limits>,
    number: u64,
    word: Word,
    out: &mut Queue<Request>,
) {
    let Some(task) = record(domain, number) else {
        return;
    };
    match word.kind {
        MessageKind::Result(_) => {}
        MessageKind::EffectNotMade { .. }
        | MessageKind::Words
        | MessageKind::Proposal { .. }
        | MessageKind::Escalation { .. }
        | MessageKind::ProposalDecision { .. }
        | MessageKind::FinishRequested { .. }
        | MessageKind::Amendment { .. }
        | MessageKind::Met { .. }
        | MessageKind::Question
        | MessageKind::Answer { .. }
        | MessageKind::Notice { .. }
        | MessageKind::Timer { .. }
        | MessageKind::PersonStalled { .. }
        | MessageKind::News { .. } => return,
    }
    let Some(payload_bytes) = word.payload_bytes() else { return };
    if !match word.from {
        Party::Task(_) => true,
        Party::Person(_) | Party::Deployment { .. } => false,
    } || word.number == 0
        || word.number <= task.last_message
        || payload_bytes > usize::try_from(env.limits.result_bytes).expect("u32 fits usize")
        || !room(domain, &env.limits, number, 1, payload_bytes)
    {
        return;
    }
    let prepared = prepare(domain, &env.limits, number, word, Admission::RoomKept)
        .expect("committed delegate result keeps a slot");
    let task = task_mut(domain, number).expect("requester remains live");
    task.record.last_activity = env.wall;
    task.record.last_message = prepared.word.number;
    task.record.inbox = prepared.inbox;
    crate::activity::message(&mut task.record, &prepared.activity_word, env.limits.inbox_messages, out);
    publish(domain, env, number, out);
    crate::wake::after_message(domain, env, number, prepared.previous, prepared.word, out);
}

/// Root-authenticated proposal decision consumes the pending proposal's
/// reserved inbox slot and wakes or relays the proposer after commitment.
pub(crate) fn proposal_decision(
    domain: &mut Domain,
    env: &Env<Limits>,
    number: u64,
    word: Word,
    out: &mut Queue<Request>,
) {
    let old = record(domain, number).expect("proposal's live proposer");
    assert!(old.proposal.is_none() && word.number > old.last_message, "decision follows proposal and is fresh");
    assert!(room(domain, &env.limits, number, 1, word.words.len()), "reserved proposal slot remains");
    let prepared = prepare(domain, &env.limits, number, word, Admission::RoomKept)
        .expect("committed proposal decision keeps a slot");
    let task = task_mut(domain, number).expect("proposal's live proposer");
    task.record.last_activity = env.wall;
    task.record.last_message = prepared.word.number;
    task.record.inbox = prepared.inbox;
    crate::activity::message(&mut task.record, &prepared.activity_word, env.limits.inbox_messages, out);
    publish(domain, env, number, out);
    crate::wake::after_message(domain, env, number, prepared.previous, prepared.word, out);
}

pub(crate) fn readable(domain: &Domain, task: u64, read: Option<u64>, outstanding: &[u64]) -> bool {
    match read {
        None => true,
        Some(number) => {
            if number == 0 {
                return false;
            }
            if outstanding.contains(&number) {
                return true;
            }
            let task_record = record(domain, task).expect("readable task live");
            for word in &task_record.inbox {
                if word.number == number {
                    return true;
                }
            }
            for word in crate::proposals::waiting_for(domain, task) {
                if word.number == number {
                    return true;
                }
            }
            for word in crate::escalation::waiting_for(domain, task) {
                if word.number == number {
                    return true;
                }
            }
            false
        }
    }
}

pub(crate) fn valid_refusals(task: &crate::TaskRecord, offered: Option<u64>, refusals: &[u64]) -> bool {
    for number in refusals {
        if offered.is_none() || Some(*number) > offered {
            return false;
        }
        let mut found = false;
        for word in &task.inbox {
            found |= word.number == *number;
        }
        if !found {
            return false;
        }
    }
    true
}

pub(crate) fn keep_refusals(task: &mut crate::TaskRecord, refused: &[u64]) {
    if task.refused.is_empty() && refused.is_empty() {
        return;
    }
    let mut names = List::with_capacity(u32::try_from(task.inbox.len()).expect("bounded inbox"));
    for word in &task.inbox {
        if task.refused.contains(&word.number) || refused.contains(&word.number) {
            names.push(word.number).expect("refused subset fits");
        }
    }
    task.refused = names.into_boxed();
}

pub(crate) fn take(task: &mut crate::TaskRecord, read: Option<u64>, refused: &[u64]) {
    keep_refusals(task, refused);
    if let Some(number) = read {
        let mut kept = List::with_capacity(u32::try_from(task.inbox.len()).expect("bounded inbox count"));
        for word in &task.inbox {
            if word.number > number || task.refused.contains(&word.number) {
                kept.push(word.clone()).expect("retained subset fits");
            }
        }
        task.inbox = kept.into_boxed();
    }
}

/// The one finish word has its own reserved room beside the merged amendment.
pub(crate) fn finish_word(task: &crate::TaskRecord, message: u64) -> bool {
    match &task.grace {
        Some(grace) => grace.message == message,
        None => false,
    }
}

/// Project inbox admission and answer credits without publishing a step part.
pub(crate) fn preview_message(
    domain: &mut Domain,
    env: &Env<Limits>,
    project: u32,
    number: u64,
    word: Word,
) -> Result<(), crate::Problem> {
    let checked = match check_message(
        domain,
        &crate::compound::Staged::new(0),
        env,
        Message { project, number, word, authorized: false },
    ) {
        Ok(plan) => plan,
        Err(problem) => return Err(*problem),
    };
    let row = &mut task_mut(domain, number).expect("message checked").record;
    row.last_message = checked.prepared.word.number;
    row.inbox = checked.prepared.inbox;
    if let Some(question) = checked.answer {
        let mut credits = List::with_capacity(env.limits.inbox_messages);
        for credit in &row.questions {
            if credit.number != question {
                credits.push(*credit).expect("credit subset");
            }
        }
        row.questions = credits.into_boxed();
    }
    if checked.prepared.word.kind == MessageKind::Question {
        let Party::Task(source) = checked.prepared.word.from else { unreachable!("question source checked") };
        let row = &mut task_mut(domain, source).expect("referenced source").record;
        let mut credits = List::with_capacity(env.limits.inbox_messages);
        for credit in &row.questions {
            credits.push(*credit).expect("bounded credits");
        }
        credits
            .push(QuestionCredit { number: checked.prepared.word.number, answerer: number })
            .expect("question checked");
        row.questions = credits.into_boxed();
    }
    Ok(())
}
