//! Reading releases tasks of the forgotten-once-read retention class (people.md, 6).
use crate::{Core, Limits};
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_lib::{Env, Queue, Wall};

/// The requesting party's reading and the task's live references.
#[derive(Clone, Copy, Debug)]
pub struct Reading {
    pub results: u64,
    pub turns: Option<u64>,
    pub named: bool,
}

/// What an ended task's forget erases, from its record or cached result.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Owner {
    pub task: u64,
    pub party: u64,
    pub project: u32,
    pub label: u32,
    pub ended_at: Wall,
    pub result_position: u64,
    pub listed: bool,
    pub last_reply: Option<u64>,
}

impl Owner {
    /// A forgettable ended record requested by a party, with its result.
    #[must_use]
    pub fn of_row(row: &tasks::TaskRecord) -> Option<Self> {
        let tasks::Party::Person(party) = row.requester else { return None };
        if row.flags.retention != tasks::Retention::ForgottenOnceRead
            || match row.phase {
                tasks::Phase::Ended(_) => false,
                tasks::Phase::Waiting
                | tasks::Phase::Active(_)
                | tasks::Phase::Closing(_)
                | tasks::Phase::Held { .. } => true,
            }
            || row.result_position == 0
        {
            return None;
        }
        Some(Self {
            task: row.number,
            party,
            project: row.project,
            label: row.label,
            ended_at: row.ended_at?,
            result_position: row.result_position,
            listed: row.flags.listed,
            last_reply: row.flags.conversation.then_some(row.last_reply),
        })
    }

    /// The same owner carried by an unread result, without loading its record.
    #[must_use]
    pub fn of_result(party: u64, result: &people::ResultRef) -> Option<Self> {
        let forgettable = result.forgettable?;
        Some(Self {
            task: result.task,
            party,
            project: result.project,
            label: forgettable.label,
            ended_at: result.at,
            result_position: result.position,
            listed: forgettable.listed,
            last_reply: forgettable.last_reply,
        })
    }
}

/// The requesting party whose reading releases this ended task, if any.
#[must_use]
pub fn forgets(owner: &Owner, reading: Reading) -> Option<u64> {
    if reading.named || reading.results < owner.result_position {
        return None;
    }
    match (owner.last_reply, reading.turns) {
        (Some(last), Some(seen)) if seen >= last => Some(owner.party),
        (Some(_), _) => None,
        (None, _) => Some(owner.party),
    }
}

/// Erase the owner and its result/index rows in the mark's decision.
pub(crate) fn forget(
    core: &mut Core,
    _env: &Env<Limits>,
    work: &mut Queue<crate::Event>,
    owner: &Owner,
) -> crate::Requests {
    let mut out = Queue::with_capacity(core.room_max.writes);
    if core.people.ready() && owner.result_position <= core.counters.deployment().messages {
        crate::projection_failures::forgotten(core, owner.task, &mut out);
        let party = owner.party;
        work.push(crate::Event::People(people::Event::Forget {
            project: owner.project,
            task: owner.task,
            party: Some(party),
            last_reply: owner.last_reply.unwrap_or(0),
        }));
        let ended_at = owner.ended_at;
        let update = tasks::Update { party, at: ended_at, task: owner.task, object: tasks::UpdateObject::Result };
        let index = tasks::EndedByParty { party, label: owner.label, ended_at, task: owner.task };
        for key in [
            tasks::Key::Ended(owner.task),
            update.key(),
            index.key(),
            tasks::Key::Result { party, position: owner.result_position },
        ] {
            out.push(crate::Request::Write(crate::Write::Erase(crate::Key::Tasks(key))));
        }
        if owner.listed {
            let index =
                tasks::EndedByProject { project: owner.project, label: owner.label, ended_at, task: owner.task };
            out.push(crate::Request::Write(crate::Write::Erase(crate::Key::Tasks(index.key()))));
        }
    }
    out.push(crate::Request::Decided);
    crate::Requests::Out(out)
}
