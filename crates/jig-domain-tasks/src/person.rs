//! Person-task lifecycle at the tasks hub (domain/tasks.md, sections 5.4 and 5.7;
//! domain/people.md, section 8). Root authenticates people and role membership;
//! this child retains the durable role claim, active waiting interval and one
//! numbered stall notice. The task alarm projects its durable wall deadline;
//! restart projects it anew. Claims and held intervals preserve the wait.
//!
//! | State | Event | Next | Emits |
//! |---|---|---|---|
//! | dependency/resource wait | becomes active | answerable wait | saved start and deadline |
//! | answerable role task | take/hand back | claimed/unclaimed | save, reply |
//! | unnotified wait | timer expires | awaiting notice number | request number |
//! | unnotified wait | numbered notice | notified wait | saves, requester news |
//! | answerable task | valid answer | closing | save, reply |
//! | held task | release | answerable wait | prior deadline rearmed |
use crate::domain::{Domain, entrance, publish, record, refused, task_mut};
use crate::{Active, Closing, Ending, Executor, Limits, PersonAddress, Phase, Refusal, Request, Stage, TaskResult};
use alloc::boxed::Box;
use skein_lib::{Env, Queue, ReplyTo, Wall};

pub(crate) fn take(
    domain: &mut Domain,
    env: &Env<Limits>,
    to: ReplyTo,
    number: u64,
    person: u64,
    out: &mut Queue<Request>,
) {
    let to = match entrance(domain, to, number) {
        Ok(to) => to,
        Err((to, why)) => return refused(to, Some(number), why, out),
    };
    let row = record(domain, number).expect("entrance names task");
    match row.executor {
        Executor::Person(PersonAddress::Role(_)) => {}
        Executor::Person(PersonAddress::Person(_)) | Executor::Agent { .. } | Executor::Procedure { .. } => {
            return refused(to, Some(number), Refusal::Executor, out);
        }
    }
    if !active(&row.phase) || row.taken_by.is_some() || person == 0 {
        return refused(to, Some(number), Refusal::State, out);
    }
    task_mut(domain, number).expect("live person task").record.taken_by = Some(person);
    publish(domain, env, number, out);
    out.push(Request::Done { reply_to: to });
}

pub(crate) fn hand_back(
    domain: &mut Domain,
    env: &Env<Limits>,
    to: ReplyTo,
    number: u64,
    person: u64,
    out: &mut Queue<Request>,
) {
    let to = match entrance(domain, to, number) {
        Ok(to) => to,
        Err((to, why)) => return refused(to, Some(number), why, out),
    };
    let row = record(domain, number).expect("entrance names task");
    match row.executor {
        Executor::Person(PersonAddress::Role(_)) => {}
        Executor::Person(PersonAddress::Person(_)) | Executor::Agent { .. } | Executor::Procedure { .. } => {
            return refused(to, Some(number), Refusal::Executor, out);
        }
    }
    if !active(&row.phase) || row.taken_by != Some(person) {
        return refused(to, Some(number), Refusal::State, out);
    }
    task_mut(domain, number).expect("live person task").record.taken_by = None;
    publish(domain, env, number, out);
    out.push(Request::Done { reply_to: to });
}

pub(crate) fn answer(
    domain: &mut Domain,
    env: &Env<Limits>,
    to: ReplyTo,
    number: u64,
    person: u64,
    result: TaskResult,
    out: &mut Queue<Request>,
) {
    let to = match entrance(domain, to, number) {
        Ok(to) => to,
        Err((to, why)) => return refused(to, Some(number), why, out),
    };
    let row = record(domain, number).expect("entrance names task");
    let addressed = match row.executor {
        Executor::Person(PersonAddress::Person(address)) => address == person && address != 0,
        Executor::Person(PersonAddress::Role(_)) => row.taken_by == Some(person) && person != 0,
        Executor::Agent { .. } | Executor::Procedure { .. } => false,
    };
    if !addressed || !active(&row.phase) {
        return refused(to, Some(number), Refusal::State, out);
    }
    let failed = match &result {
        TaskResult::Failure { .. } => true,
        TaskResult::Approved { .. }
        | TaskResult::Report { .. }
        | TaskResult::Verdict { .. }
        | TaskResult::Change { .. } => false,
    };
    if !failed && crate::run::ends::kept(domain, number).refuses_done() {
        return out.push(Request::Refused {
            reply_to: to,
            problem: domain.waiting_effect_problem(number).expect("kept task names its reason"),
        });
    }
    if !row.delegates.is_empty() {
        return out
            .push(Request::Refused { reply_to: to, problem: crate::Problem::live_delegates(number, &row.delegates) });
    }
    if !crate::run::valid_result(&row.contract, &result, &env.limits) {
        return refused(to, Some(number), Refusal::Contract, out);
    }
    let ending = match result {
        TaskResult::Failure { reason } => Ending::Failed { reason },
        TaskResult::Approved { .. }
        | TaskResult::Report { .. }
        | TaskResult::Verdict { .. }
        | TaskResult::Change { .. } => Ending::Done(result),
    };
    let row = &mut task_mut(domain, number).expect("live person task").record;
    row.taken_by = None;
    row.person_answer = Some(crate::PersonAnswer { by: person, at: env.wall });
    row.phase = Phase::Closing(Closing { stage: Stage::Delegates, ending });
    publish(domain, env, number, out);
    out.push(Request::Done { reply_to: to });
}

fn active(phase: &Phase) -> bool {
    match phase {
        Phase::Active(Active::Due | Active::Idle) => true,
        Phase::Waiting
        | Phase::Active(
            Active::Preparing | Active::Claimed { .. } | Active::Running { .. } | Active::BackingOff { .. },
        )
        | Phase::Closing(_)
        | Phase::Held { .. }
        | Phase::Ended(_) => false,
    }
}

/// Retain the start of the active wait; taking, handing back and held intervals
/// do not restart the question. A dependency wait has not reached its recipient.
pub(crate) fn waiting(row: &mut crate::TaskRecord, env: &Env<Limits>) {
    match row.executor {
        Executor::Person(_) => {
            if active(&row.phase) && row.person_wait.is_none() {
                row.person_wait = Some(crate::PersonWait {
                    since: env.wall,
                    stall: env.limits.person_stall,
                    deadline: Wall::from_nanos(env.wall.as_nanos().saturating_add(env.limits.person_stall.as_nanos())),
                    notice: None,
                });
            }
        }
        Executor::Agent { .. } | Executor::Procedure { .. } => row.person_wait = None,
    }
}

pub(crate) fn pending(row: &crate::TaskRecord) -> bool {
    match row.executor {
        Executor::Person(_) => match row.person_wait {
            Some(wait) => wait.notice.is_none(),
            None => false,
        },
        Executor::Agent { .. } | Executor::Procedure { .. } => false,
    }
}

/// One credit per person delegate whose stall has not yet been reported.
pub(crate) fn credit(row: &crate::TaskRecord) -> bool {
    match row.executor {
        Executor::Person(_) => match row.person_wait {
            Some(wait) => wait.notice.is_none(),
            None => true,
        },
        Executor::Agent { .. } | Executor::Procedure { .. } => false,
    }
}

pub(crate) fn stalled(domain: &mut Domain, env: &Env<Limits>, number: u64, message: u64, out: &mut Queue<Request>) {
    let Some(row) = record(domain, number) else { return };
    if !active(&row.phase) || !pending(row) || message == 0 {
        return;
    }
    let Some(alarm) = task_mut(domain, number).expect("waiting person task").alarm else { return };
    if env.now < alarm.due {
        return;
    }
    let requester = record(domain, number).expect("waiting person task").requester;
    let word = crate::Word {
        number: message,
        from: crate::Party::Task(number),
        kind: crate::MessageKind::PersonStalled { task: number },
        words: Box::new([]),
        at: env.wall,
        hits: 1,
        eligible: false,
    };
    let row = &mut task_mut(domain, number).expect("waiting person task").record;
    crate::activity::append(row, env.wall, crate::Line::Notified { recipient: requester, word: word.clone() }, out);
    row.person_wait.as_mut().expect("active wait").notice =
        Some(crate::PersonStallNotice { message, at: env.wall, activity: row.activity });
    publish(domain, env, number, out);
    match requester {
        crate::Party::Task(parent) => {
            let prepared = crate::inbox::prepare(domain, &env.limits, parent, word, crate::inbox::Admission::RoomKept)
                .expect("person delegate kept requester news room");
            let parent_row = task_mut(domain, parent).expect("requester remains live");
            parent_row.record.last_message = prepared.word.number;
            parent_row.record.inbox = prepared.inbox;
            crate::activity::message(&mut parent_row.record, &prepared.activity_word, env.limits.inbox_messages, out);
            publish(domain, env, parent, out);
            crate::wake::after_message(domain, env, parent, prepared.previous, prepared.word, out);
        }
        crate::Party::Person(_) | crate::Party::Deployment { .. } => {}
    }
}
