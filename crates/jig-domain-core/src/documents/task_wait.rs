//! A task's next-run wait, separate from its creator preference and its committed account
//! (client/domain/contract.md, 5.1; accounts.md, 4.3).
use crate::{Core, RunWait};
use jig_domain_accounts as accounts;
use jig_domain_tasks as tasks;
use skein_lib::Wall;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TaskRunWait {
    Account { account: Option<u32>, source: Option<u32>, why: TaskWaitWhy },
    Credential,
    Host,
    Refused(TaskWaitRefused),
}
/// A derived explanation, never a durable task state (domain/engine.md, section 8).
/// Hello keeps the fleet's observed receipt time and cause until admission or restart.
/// Program is retained with no producer: Open admits program paths (hosts.md, 8.1).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TaskWaitRefused {
    Hello { kind: jig_domain_fleet::HostKind, at: Wall, why: jig_domain_fleet::HelloRefusal },
    Program(OpenProgram),
}

/// The incompatible model path or configured account/source of an Open run.
/// Retained with no producer since Open admits program paths (hosts.md, 8.1).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum OpenProgram {
    Model,
    Source { account: u32, source: u32 },
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TaskWaitWhy {
    Source(TaskSourceState),
    Refreshing,
    Host,
    Empty,
    Invalid(tasks::AccountWhy),
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TaskSourceState {
    Moving,
    Asking,
    Granted,
    Later { until: Option<Wall> },
    Limited { until: Option<Wall> },
    Refused,
    NeedsSignIn,
    Misconfigured,
    Unloaded { at: accounts::Place, why: accounts::Unread },
    Cancelled,
    Closing,
}

pub(super) fn waiting(core: &Core, row: &tasks::TaskRecord) -> Option<TaskRunWait> {
    match row.executor {
        tasks::Executor::Procedure { .. } | tasks::Executor::Person(_) => return None,
        tasks::Executor::Agent { .. } => {}
    }
    match row.phase {
        tasks::Phase::Active(tasks::Active::Due | tasks::Active::Preparing) => {}
        tasks::Phase::Active(_)
        | tasks::Phase::Waiting
        | tasks::Phase::Held { .. }
        | tasks::Phase::Closing(_)
        | tasks::Phase::Ended(_) => return None,
    }
    match core.run_choice(row.number) {
        Ok(_) if !core.credentials_ready(row.number) => Some(TaskRunWait::Credential),
        Ok(_) => None,
        Err(RunWait::InvalidAccount { account, why }) => {
            Some(TaskRunWait::Account { account: Some(account), source: None, why: TaskWaitWhy::Invalid(why) })
        }
        Err(RunWait::Credential) => Some(TaskRunWait::Credential),
        Err(RunWait::Host) => Some(match core.fleet.standing_refusal(core.charter_of(row.number).hosts) {
            Some((kind, record)) => {
                TaskRunWait::Refused(TaskWaitRefused::Hello { kind, at: record.at, why: record.why })
            }
            None => TaskRunWait::Host,
        }),
        Err(RunWait::Account { account, source, why }) => Some(TaskRunWait::Account {
            account,
            source,
            why: match why {
                accounts::WaitWhy::Refreshing => TaskWaitWhy::Refreshing,
                accounts::WaitWhy::Host => TaskWaitWhy::Host,
                accounts::WaitWhy::Empty => TaskWaitWhy::Empty,
                accounts::WaitWhy::Source(state) => TaskWaitWhy::Source(source_state(state)),
            },
        }),
    }
}

#[expect(clippy::manual_map, reason = "step code has no closures (programming-model.md, section 10)")]
fn source_state(state: accounts::State) -> TaskSourceState {
    match state {
        accounts::State::Moving => TaskSourceState::Moving,
        accounts::State::Asking => TaskSourceState::Asking,
        accounts::State::Granted => TaskSourceState::Granted,
        accounts::State::Later { until } => TaskSourceState::Later { until: Some(until.wall) },
        accounts::State::Limited { until } => TaskSourceState::Limited {
            until: match until {
                Some(until) => Some(until.wall),
                None => None,
            },
        },
        accounts::State::Refused => TaskSourceState::Refused,
        accounts::State::NeedsSignIn => TaskSourceState::NeedsSignIn,
        accounts::State::Misconfigured => TaskSourceState::Misconfigured,
        accounts::State::Unloaded { at, why } => TaskSourceState::Unloaded { at, why },
        accounts::State::Cancelled => TaskSourceState::Cancelled,
        accounts::State::Closing => TaskSourceState::Closing,
    }
}
