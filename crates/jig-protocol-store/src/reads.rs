//! Parties' bounded durable reads and joins (protocol/store.md, 7.3).
mod indexed;
mod joins;
mod paged;
mod point;
mod settings;
use crate::{Below, Event, Phase, Store, loads};
use alloc::boxed::Box;
use jig_domain_core::reads::{Loaded, Range, Scope};
use skein_lib::{Queue, Token};

pub(crate) struct Spent {
    pub rows: u32,
    pub bytes: u64,
    pub joins: u32,
}

#[derive(Debug)]
pub(crate) enum Failure {
    Cut,
    Joins,
    Corrupt,
}

pub(crate) fn empty() -> Loaded {
    Loaded::Page { rows: Box::new([]), next: None }
}
pub(crate) use point::{finish, proposal_key};

pub(crate) fn read(store: &mut Store, owner: Token, range: Range, above: &mut Queue<Event>, below: &mut Below) {
    if store.phase == Phase::Closed {
        loads::corrupt(store, owner, above);
        return;
    }
    let mut spent = Spent { rows: range.rows, bytes: range.bytes, joins: store.limits.joins };
    let result = match range.scope {
        Scope::SettingsHistory { .. } => settings::read(store, &range),
        Scope::Task { task } | Scope::PersonTask { task } | Scope::Result { task } => {
            point::task(store, &range, task, &mut spent)
        }
        Scope::Proposal { proposer, proposal } => point::proposal(store, &range, proposer, proposal, &mut spent),
        Scope::Escalation { task } => point::escalation(store, &range, task, &mut spent),
        Scope::Question { task, message } => point::question(store, &range, task, message, &mut spent),
        Scope::Reply { .. } => {
            let result = point::reply(store, owner, &range, &mut spent, above, below);
            return answer(store, owner, result, above);
        }
        Scope::Turn { .. } => {
            let result = point::turn(store, owner, &range, above, below);
            return answer(store, owner, result, above);
        }
        Scope::Activity { task } => {
            let result = paged::activity(store, &range, task, &mut spent);
            let result = match result {
                Ok(Loaded::Page { rows, next }) => {
                    crate::transcripts::read::activity(store, owner, rows, next, &range, above, below)
                }
                Ok(value) => Ok(Some(value)),
                Err(error) => Err(error),
            };
            return answer(store, owner, result, above);
        }
        Scope::History { task, subtree: false } => paged::history(store, &range, task, &mut spent),
        Scope::Spend { .. } => paged::spend(store, &range, &mut spent),
        Scope::Notes { .. } => paged::notes(store, &range, &mut spent),
        Scope::ScopedNotes { .. } => paged::scoped_notes(store, &range, &mut spent),
        Scope::EndedDelegates { .. } => indexed::ended_delegates(store, &range, &mut spent),
        Scope::History { subtree: true, .. } => indexed::subtree_history(store, &range, &mut spent),
        Scope::Inbox { .. } => indexed::inbox(store, &range, &mut spent),
        Scope::Tasks { .. } => indexed::ended_tasks(store, &range, &mut spent),
    };
    answer(store, owner, ready(result), above);
}

pub(crate) fn ready(result: Result<Loaded, Failure>) -> Result<Option<Loaded>, Failure> {
    match result {
        Ok(result) => Ok(Some(result)),
        Err(error) => Err(error),
    }
}

pub(crate) fn answer(
    store: &mut Store,
    owner: Token,
    result: Result<Option<Loaded>, Failure>,
    above: &mut Queue<Event>,
) {
    match result {
        Ok(Some(result)) => above.push(Event::Read { owner, result }),
        Ok(None) => {}
        Err(Failure::Joins) => {
            above.push(Event::Read { owner, result: Loaded::TooLinked { joins: store.limits.joins } });
        }
        Err(Failure::Cut) => above.push(Event::Read { owner, result: empty() }),
        Err(Failure::Corrupt) => loads::corrupt(store, owner, above),
    }
}
