//! Durable finish graces for every task (domain/tasks.md, sections 7 and 8.2).
use crate::domain::{Domain, publish, record, task_mut};
use crate::{Change, Limits, MessageKind, Party, Phase, Refusal, Request, Word};
use alloc::boxed::Box;
use skein_lib::{Env, Queue, Wall};

/// Who asked a task to finish, when its grace ends, and the cancellation reason.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Grace {
    pub by: Party,
    pub until: Wall,
    pub reason: Box<[u8]>,
    /// The single finish word, kept apart from ordinary merged amendments.
    pub message: u64,
}

pub(crate) struct Finish {
    pub number: u64,
    pub by: Party,
    pub until: Wall,
    pub message: u64,
}

pub(crate) enum FinishPlan {
    Cancel { finish: Finish, reason: Box<[u8]>, selected: Box<[u64]> },
    Sooner { finish: Finish, reason: Box<[u8]> },
    Word { finish: Finish, reason: Box<[u8]>, prepared: crate::inbox::Prepared },
    Unchanged,
}

pub(crate) fn check_finish(
    domain: &Domain,
    staged: &crate::compound::Staged,
    env: &Env<Limits>,
    finish: Finish,
    words: &[u8],
    reason: &[u8],
) -> Result<FinishPlan, Refusal> {
    let Finish { number, by, until, message } = finish;
    let old = staged.record(domain, number).ok_or(Refusal::Unknown)?;
    if message == 0
        || message <= old.last_message
        || words.len() > usize::try_from(env.limits.result_bytes).expect("reason bound")
    {
        return Err(Refusal::Read);
    }
    let cancel = match old.phase {
        Phase::Waiting | Phase::Held { .. } => true,
        Phase::Active(_) => {
            !crate::run::ends::pending(old.waiting_on, old.flags.ended_by).with_question(old.question).finishes_alone()
                && crate::run::run_attempt(&old.phase).is_none()
        }
        Phase::Closing(_) | Phase::Ended(_) => return Err(Refusal::State),
    };
    if cancel {
        let mut selected = skein_lib::List::with_capacity(env.limits.tasks);
        for (child, _) in &domain.names {
            if staged.descendant(domain, *child, number, env.limits.tasks) {
                selected.push(*child).expect("live subtree bound");
            }
        }
        return Ok(FinishPlan::Cancel { finish, reason: reason.into(), selected: selected.into_boxed() });
    }
    if let Some(grace) = &old.grace {
        return Ok(if until < grace.until {
            FinishPlan::Sooner { finish, reason: reason.into() }
        } else {
            FinishPlan::Unchanged
        });
    }
    let revision = old.revision.checked_add(1).ok_or(Refusal::State)?;
    let word = Word {
        number: message,
        from: by,
        kind: MessageKind::FinishRequested { revision },
        words: words.into(),
        at: env.wall,
        hits: 1,
        eligible: true,
    };
    let prepared =
        crate::inbox::prepare_staged(domain, staged, &env.limits, number, word, crate::inbox::Admission::RoomKept)
            .expect("grace has its reserved inbox slot");
    Ok(FinishPlan::Word { finish, reason: reason.into(), prepared })
}

pub(crate) fn apply_finish(domain: &mut Domain, env: &Env<Limits>, plan: FinishPlan, out: &mut Queue<Request>) {
    let (finish, reason, prepared) = match plan {
        FinishPlan::Unchanged => return,
        FinishPlan::Cancel { finish, reason, selected } => {
            cancel_selected(domain, env, finish.number, finish.by, &reason, selected, out);
            return;
        }
        FinishPlan::Sooner { finish, reason } => {
            let grace =
                task_mut(domain, finish.number).expect("live grace").record.grace.as_mut().expect("saved grace");
            grace.by = finish.by;
            grace.until = finish.until;
            grace.reason = reason;
            publish(domain, env, finish.number, out);
            return;
        }
        FinishPlan::Word { finish, reason, prepared } => (finish, reason, prepared),
    };
    let Finish { number, by, until, message } = finish;
    let task = task_mut(domain, number).expect("live grace task");
    task.record.grace = Some(Grace { by, until, reason: reason.clone(), message });
    task.record.last_activity = env.wall;
    task.record.last_message = message;
    task.record.inbox = prepared.inbox;
    crate::activity::message(&mut task.record, &prepared.activity_word, env.limits.inbox_messages, out);
    crate::control::history(domain, number, env.wall, by, Change::Amended, &reason, out);
    publish(domain, env, number, out);
    crate::wake::after_message(domain, env, number, prepared.previous, prepared.word, out);
}

pub(crate) fn cancel_selected(
    domain: &mut Domain,
    env: &Env<Limits>,
    number: u64,
    by: Party,
    reason: &[u8],
    selected: Box<[u64]>,
    out: &mut Queue<Request>,
) {
    for child in selected {
        crate::control::history(domain, child, env.wall, by, Change::Cancelled, reason, out);
    }
    crate::closing::cancel_tree(domain, env, number, by, reason, out);
}

pub(crate) fn schedule(domain: &mut Domain, env: &Env<Limits>, number: u64) {
    let task = task_mut(domain, number).expect("live chat schedule");
    let until = match &task.record.grace {
        Some(grace) if mutable(&task.record.phase) => Some(grace.until),
        Some(_) | None => None,
    };
    let alarm = match until {
        Some(until) => match task.grace_alarm {
            Some(old) if old.until == until => Some(old),
            Some(_) | None => Some(crate::domain::Alarm {
                until,
                due: env.now.saturating_add(skein_lib::Duration::from_nanos(
                    until.as_nanos().saturating_sub(env.wall.as_nanos()),
                )),
            }),
        },
        None => None,
    };
    task.grace_alarm = alarm;
    match alarm {
        Some(alarm) => assert!(domain.grace_alarms.arm(number, alarm.due).is_ok(), "one grace per live task"),
        None => {
            domain.grace_alarms.cancel(number);
        }
    }
}

pub(crate) fn fire(domain: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    let Some(number) = domain.grace_alarms.expire(env.now) else { return };
    let Some(row) = record(domain, number) else { return };
    let Some(grace) = row.grace.clone() else { return };
    if !mutable(&row.phase) {
        return;
    }
    crate::closing::cancel_tree(domain, env, number, grace.by, &grace.reason, out);
}

pub(crate) fn mutable(phase: &Phase) -> bool {
    match phase {
        Phase::Waiting | Phase::Active(_) | Phase::Held { was: crate::Was::Waiting | crate::Was::Active(_), .. } => {
            true
        }
        Phase::Closing(_) | Phase::Held { was: crate::Was::Closing(_), .. } | Phase::Ended(_) => false,
    }
}
