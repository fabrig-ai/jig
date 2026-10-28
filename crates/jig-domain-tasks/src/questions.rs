//! Question credits, their person answers and their exits (domain/tasks.md, 5.7 and 7).
use crate::domain::{Domain, publish, record, refused, task_mut};
use crate::{Event, Limits, MessageKind, Party, Refusal, Request, Word};
use alloc::boxed::Box;
use skein_lib::{Env, List, Queue, ReplyTo};

impl Domain {
    /// The credit owner of one question offered in this person's inbox.
    #[must_use]
    pub fn question_asker(&self, asked: u64, person: u64, question: u64) -> Option<u64> {
        let recipient = record(self, asked)?;
        if recipient.requester != Party::Person(person) {
            return None;
        }
        for (number, _) in &self.names {
            let row = record(self, *number).expect("live asker");
            for credit in &row.questions {
                if credit.number == question
                    && credit.answerer == Party::Person(person)
                    && (credit.asked == Party::Task(asked) || *number == asked && credit.asked == Party::Person(person))
                {
                    return Some(*number);
                }
            }
        }
        None
    }
}

pub(crate) fn person_answer(domain: &mut Domain, env: &Env<Limits>, event: Event, out: &mut Queue<Request>) {
    let Event::PersonQuestionAnswer { reply_to, project, asked, person, question, mut word, notice } = event else {
        unreachable!("person answer event")
    };
    let Some(asker) = domain.question_asker(asked, person, question) else {
        return refused(reply_to, Some(asked), Refusal::Reference, out);
    };
    let row = record(domain, asker).expect("credit owner live");
    if row.project != project
        || !crate::moving::mutable(&row.phase)
        || word.from != Party::Person(person)
        || word.kind != (MessageKind::Answer { question })
        || word.words.is_empty()
        || word.words.len() > usize::try_from(env.limits.message_bytes).expect("byte bound")
    {
        return refused(reply_to, Some(asked), Refusal::State, out);
    }
    let mut selected = None;
    for credit in &row.questions {
        if credit.number == question {
            selected = Some(credit.clone());
        }
    }
    let credit = selected.expect("selected credit");
    word.eligible = credit.asked == row.requester;
    word.from = credit.asked;
    let Some(prepared) = crate::inbox::prepare(domain, &env.limits, asker, word, crate::inbox::Admission::RoomKept)
    else {
        return refused(reply_to, Some(asker), Refusal::Read, out);
    };
    let (task, word) = crate::inbox::apply_message(
        domain,
        env,
        crate::inbox::MessagePlan { number: asker, prepared, answer: Some(question) },
        out,
    );
    if asked != asker {
        notice_word(domain, env, asked, notice, MessageKind::QuestionAnswered { question }, out);
    }
    out.push(Request::Sent { reply_to, task, word });
}

pub(crate) fn cancel(
    domain: &mut Domain,
    env: &Env<Limits>,
    to: ReplyTo,
    task: u64,
    question: u64,
    out: &mut Queue<Request>,
) {
    let Some(row) = record(domain, task) else {
        return refused(to, Some(task), Refusal::Unknown, out);
    };
    let mut selected = None;
    for credit in &row.questions {
        if credit.number == question {
            selected = Some(credit.clone());
        }
    }
    let Some(credit) = selected else {
        return refused(to, Some(task), Refusal::Reference, out);
    };
    forget_credit(domain, env, task, question, out);
    match credit.asked {
        Party::Task(asked) => {
            forget_received(domain, env, asked, question, out);
        }
        Party::Person(_) => forget_received(domain, env, task, question, out),
        Party::Deployment { .. } => unreachable!("question recipient is a task or person"),
    }
    owe(domain, env, task, question);
    out.push(Request::Done { reply_to: to });
}

pub(crate) fn forget_credit(
    domain: &mut Domain,
    env: &Env<Limits>,
    task: u64,
    question: u64,
    _out: &mut Queue<Request>,
) {
    let row = task_mut(domain, task).expect("credit owner live");
    let mut credits = List::with_capacity(env.limits.inbox_messages);
    for credit in &row.record.questions {
        if credit.number != question {
            credits.push(credit.clone()).expect("credit subset");
        }
    }
    row.record.questions = credits.into_boxed();
    changed(domain, task);
}

pub(crate) fn forget_received(
    domain: &mut Domain,
    env: &Env<Limits>,
    task: u64,
    question: u64,
    _out: &mut Queue<Request>,
) {
    let Some(row) = task_mut(domain, task) else {
        return;
    };
    let mut lines = List::with_capacity(env.limits.inbox_messages);
    for line in &row.record.question_lines {
        if line.message != question {
            lines.push(*line).expect("question subset");
        }
    }
    row.record.question_lines = lines.into_boxed();
    changed(domain, task);
}

pub(crate) fn ended(
    domain: &mut Domain,
    env: &Env<Limits>,
    task: u64,
    notices: &[crate::QuestionNotice],
    out: &mut Queue<Request>,
) {
    assert!(
        notices.len() <= usize::try_from(env.limits.inbox_messages).expect("question bound"),
        "bounded notice batch"
    );
    let mut last = None;
    for notice in notices {
        last = append_notice(
            domain,
            env,
            task,
            notice.message,
            MessageKind::QuestionEnded { question: notice.question },
            out,
        );
    }
    if let Some((previous, word)) = last {
        publish(domain, env, task, out);
        crate::wake::after_message(domain, env, task, previous, word, out);
    }
}

fn notice_word(
    domain: &mut Domain,
    env: &Env<Limits>,
    task: u64,
    message: u64,
    kind: MessageKind,
    out: &mut Queue<Request>,
) {
    if let Some((previous, word)) = append_notice(domain, env, task, message, kind, out) {
        publish(domain, env, task, out);
        crate::wake::after_message(domain, env, task, previous, word, out);
    }
}

fn append_notice(
    domain: &mut Domain,
    env: &Env<Limits>,
    task: u64,
    message: u64,
    kind: MessageKind,
    out: &mut Queue<Request>,
) -> Option<(Option<u64>, Word)> {
    let row = record(domain, task)?;
    if !crate::moving::mutable(&row.phase) {
        return None;
    }
    let word = Word {
        number: message,
        from: Party::Task(task),
        kind,
        words: Box::new([]),
        at: env.wall,
        hits: 1,
        eligible: true,
    };
    let prepared = crate::inbox::prepare(domain, &env.limits, task, word, crate::inbox::Admission::RoomKept)
        .expect("question reserved its terminal notice");
    let row = task_mut(domain, task).expect("notice owner live");
    row.record.last_message = message;
    row.record.inbox = prepared.inbox;
    crate::activity::message(&mut row.record, &prepared.activity_word, env.limits.inbox_messages, out);
    Some((prepared.previous, prepared.word))
}

/// Move's question decisions stay with the child that owns their credits.
pub(crate) fn moved(domain: &mut Domain, env: &Env<Limits>, number: u64, person: u64, out: &mut Queue<Request>) {
    let credits = record(domain, number).expect("moving asker live").questions.clone();
    for credit in &mut task_mut(domain, number).expect("moving asker live").record.questions {
        credit.answerer = Party::Person(person);
        credit.asked = Party::Person(person);
    }
    // Received questions stay with this task; their answering person follows its requester.
    for source in crate::domain::snapshot(domain, env.limits.tasks).into_boxed() {
        let row = task_mut(domain, source).expect("live question source");
        let mut updated = false;
        for credit in &mut row.record.questions {
            if credit.asked == Party::Task(number) {
                credit.answerer = Party::Person(person);
                updated = true;
            }
        }
        if updated {
            changed(domain, source);
        }
    }
    for credit in credits {
        match credit.asked {
            Party::Task(asked) => {
                forget_received(domain, env, asked, credit.number, out);
                let row = task_mut(domain, number).expect("moved asker live");
                row.record.last_message = row.record.last_message.max(credit.number);
                crate::positions::question(&mut row.record, credit.number, credit.line, env.limits.inbox_messages);
                changed(domain, number);
            }
            Party::Person(_) | Party::Deployment { .. } => {}
        }
    }
}

/// Coalesce all credit-only publications and terminal routing in one task step.
pub(crate) fn changed(domain: &mut Domain, task: u64) {
    for old in &domain.question_changes {
        if *old == task {
            return;
        }
    }
    domain.question_changes.push(task).expect("one dirty entry per live task");
}

pub(crate) fn owe(domain: &mut Domain, env: &Env<Limits>, task: u64, question: u64) {
    if !domain.question_ends.contains_key(&task) {
        domain.question_ends.insert(task, List::with_capacity(env.limits.inbox_messages)).expect("one batch per task");
    }
    domain.question_ends.get_mut(&task).expect("question batch").push(question).expect("retired credits were bounded");
}

pub(crate) fn flush(domain: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    let changed = core::mem::replace(&mut domain.question_changes, List::with_capacity(env.limits.tasks));
    for task in changed.into_boxed() {
        if record(domain, task).is_some() {
            publish(domain, env, task, out);
        }
    }
    let mut names = List::with_capacity(env.limits.tasks);
    for (task, _) in &domain.question_ends {
        names.push(*task).expect("bounded batches");
    }
    for task in names.into_boxed() {
        let questions = domain.question_ends.remove(&task).expect("named batch");
        if let Some(row) = record(domain, task)
            && crate::moving::mutable(&row.phase)
        {
            out.push(Request::QuestionEndedDue { task, questions: questions.into_boxed() });
        }
    }
}
