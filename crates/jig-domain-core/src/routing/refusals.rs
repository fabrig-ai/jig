//! Total sibling translations into party answer facts
//! (client/domain/contract.md, section 4; domain/people.md, section 5.1).
//! The core reads the current live record for an ending; no historical archive
//! is retained here. Unknown names disclose no object. Retry answers are unsaved.
use crate::{Core, Event};
use alloc::boxed::Box;
use jig_domain_notes as notes;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_lib::{Queue, Token};

pub(crate) fn person_problem_refusal(core: &Core, problem: tasks::Problem) -> people::Outcome {
    task_answer(core, problem)
}

pub(crate) fn person_task_refusal(core: &Core, task: Option<u64>, why: tasks::Refusal) -> people::Outcome {
    task_answer(core, tasks::Problem::naming(task, why, &[]))
}

pub(super) fn person_move_refusal(core: &Core, problem: tasks::Problem) -> people::Outcome {
    task_answer(core, problem)
}

pub(super) fn person_message_refusal(core: &Core, problem: tasks::Problem) -> people::Outcome {
    task_answer(core, problem)
}

fn task_answer(core: &Core, problem: tasks::Problem) -> people::Outcome {
    let tasks::Problem { task, why, blocked_by, capacity } = problem;
    if let Some(capacity) = capacity
        && let Some(which) = placed_limit(capacity.cause)
    {
        return people::Outcome::Refused(people::Refusal::Limit { which });
    }
    let names = match blocked_by {
        Some(names) => names,
        None => Box::new([]),
    };
    let refusal = match why {
        tasks::Refusal::Unproposable => {
            people::Refusal::Authority { lacking: people::Lack::Approver, proposable: Some(people::Accepter::Nobody) }
        }
        tasks::Refusal::HoldRevision { revision } | tasks::Refusal::PersonRevision { revision } => {
            people::Refusal::Moved { now: people::Moved::Person { revision } }
        }
        tasks::Refusal::Busy => return people::Outcome::Again(people::Again::Busy),
        tasks::Refusal::NotReady => return people::Outcome::Again(people::Again::NotReady),
        tasks::Refusal::ResourceUnavailable => {
            people::Refusal::Authority { proposable: None, lacking: people::Lack::ResourceUnavailable }
        }
        tasks::Refusal::Unknown => people::Refusal::Unknown,
        tasks::Refusal::State | tasks::Refusal::Placed { .. } | tasks::Refusal::HoldOwner { .. } => {
            match current_task(core, task) {
                Some(row) => match &row.phase {
                    tasks::Phase::Closing(closing) => ended(&closing.ending),
                    tasks::Phase::Ended(ending) => ended(ending),
                    tasks::Phase::Waiting | tasks::Phase::Active(_) | tasks::Phase::Held { .. } => {
                        people::Refusal::NotWaiting
                    }
                },
                None => people::Refusal::Unknown,
            }
        }
        tasks::Refusal::Executor => {
            people::Refusal::Authority { proposable: None, lacking: people::Lack::TaskKind { executor: None } }
        }
        // A refused model reaches the party by its index, which the client's engine names (tasks.md, 3).
        tasks::Refusal::Llm { model, why } => people::Refusal::Llm { model, why: llm_why(why) },
        tasks::Refusal::Account { account, why } => people::Refusal::Account {
            account,
            why: match why {
                tasks::AccountWhy::Unknown => people::AccountWhy::Unknown,
                tasks::AccountWhy::Provider => people::AccountWhy::Provider,
                tasks::AccountWhy::Uses => people::AccountWhy::Uses,
                tasks::AccountWhy::Limited => people::AccountWhy::Limited,
            },
        },
        tasks::Refusal::Reference => people::Refusal::Authority { proposable: None, lacking: people::Lack::Reference },
        tasks::Refusal::Funding => people::Refusal::Authority { proposable: None, lacking: people::Lack::Funding },
        tasks::Refusal::AuthorityShape => people::Refusal::Authority { proposable: None, lacking: people::Lack::Shape },
        tasks::Refusal::Duplicate => people::Refusal::Limit { which: people::LimitKind::Duplicate },
        tasks::Refusal::Empty => people::Refusal::Limit { which: people::LimitKind::Empty },
        tasks::Refusal::Batch => people::Refusal::Limit { which: people::LimitKind::Batch },
        tasks::Refusal::Live => people::Refusal::Limit { which: people::LimitKind::LiveTasks },
        tasks::Refusal::Project => people::Refusal::Limit { which: people::LimitKind::ProjectLiveTasks },
        tasks::Refusal::Tree => people::Refusal::Limit { which: people::LimitKind::Tree },
        tasks::Refusal::Depth => people::Refusal::Limit { which: people::LimitKind::Depth },
        tasks::Refusal::Delegates => people::Refusal::Limit { which: people::LimitKind::Delegates },
        tasks::Refusal::Subscription => people::Refusal::Limit { which: people::LimitKind::Subscription },
        tasks::Refusal::Dependencies => people::Refusal::Limit { which: people::LimitKind::Dependencies },
        tasks::Refusal::Circle { cut } => people::Refusal::Circle { tasks: names, cut },
        tasks::Refusal::DependencyEnded { how } => people::Refusal::Dependency {
            dependency: match names.first() {
                Some(number) => *number,
                None => return people::Outcome::Refused(people::Refusal::Unknown),
            },
            how: match how {
                tasks::Status::Done => people::DependencyStatus::Done,
                tasks::Status::Failed => people::DependencyStatus::Failed,
                tasks::Status::Cancelled => people::DependencyStatus::Cancelled,
            },
        },
        tasks::Refusal::Kept => people::Refusal::Kept { pending: names },
        tasks::Refusal::Grace => people::Refusal::Limit { which: people::LimitKind::Grace },
        tasks::Refusal::Goals => people::Refusal::Limit { which: people::LimitKind::Goals },
        tasks::Refusal::Spec => people::Refusal::Limit { which: people::LimitKind::Spec },
        tasks::Refusal::Contract => people::Refusal::Limit { which: people::LimitKind::Contract },
        tasks::Refusal::Inputs => people::Refusal::Limit { which: people::LimitKind::Inputs },
        tasks::Refusal::Attempt => people::Refusal::Limit { which: people::LimitKind::Attempts },
        tasks::Refusal::LiveDelegates => people::Refusal::Limit { which: people::LimitKind::LiveDelegates },
        tasks::Refusal::Restore => people::Refusal::Limit { which: people::LimitKind::Restore },
        tasks::Refusal::Read => people::Refusal::Limit { which: people::LimitKind::Read },
        tasks::Refusal::Turn => people::Refusal::Limit { which: people::LimitKind::Turn },
        tasks::Refusal::HoldKind => people::Refusal::Limit { which: people::LimitKind::HoldKind },
        tasks::Refusal::HoldTaken => people::Refusal::Limit { which: people::LimitKind::HoldTaken },
        tasks::Refusal::Holds => people::Refusal::Limit { which: people::LimitKind::Holds },
    };
    people::Outcome::Refused(refusal)
}

fn placed_limit(cause: tasks::CapacityCause) -> Option<people::LimitKind> {
    match cause {
        tasks::CapacityCause::PlacedHolds { .. } => Some(people::LimitKind::PlacedHolds),
        tasks::CapacityCause::ProjectPlacedHolds { .. } => Some(people::LimitKind::ProjectPlacedHolds),
        tasks::CapacityCause::PartyPlacedHolds { .. } => Some(people::LimitKind::PartyPlacedHolds),
        tasks::CapacityCause::Exemptions { .. } => Some(people::LimitKind::Exemptions),
        tasks::CapacityCause::Tasks { .. }
        | tasks::CapacityCause::CreationFlights { .. }
        | tasks::CapacityCause::ResourceReports { .. }
        | tasks::CapacityCause::ProjectTasks { .. }
        | tasks::CapacityCause::Tree { .. }
        | tasks::CapacityCause::Delegates { .. }
        | tasks::CapacityCause::Inbox { .. }
        | tasks::CapacityCause::Holdings { .. }
        | tasks::CapacityCause::HoldWaiters { .. }
        | tasks::CapacityCause::Proposals { .. }
        | tasks::CapacityCause::References { .. }
        | tasks::CapacityCause::Subscriptions { .. }
        | tasks::CapacityCause::Funders { .. }
        | tasks::CapacityCause::Writers { .. } => None,
    }
}

fn llm_why(why: tasks::LlmFault) -> people::LlmWhy {
    match why {
        tasks::LlmFault::Unconfigured => people::LlmWhy::Unconfigured,
        tasks::LlmFault::Effort => people::LlmWhy::Effort,
        tasks::LlmFault::Charter => people::LlmWhy::Charter,
        tasks::LlmFault::Authority => people::LlmWhy::Authority,
        tasks::LlmFault::Procedure => people::LlmWhy::Procedure,
        tasks::LlmFault::Repeated => people::LlmWhy::Repeated,
    }
}

fn current_task(core: &Core, task: Option<u64>) -> Option<&tasks::TaskRecord> {
    match task {
        Some(task) => core.tasks.task(task),
        None => None,
    }
}

pub(crate) fn ended(ending: &tasks::Ending) -> people::Refusal {
    let how = match ending {
        tasks::Ending::Done(_) => people::Ending::Done,
        tasks::Ending::Failed { .. } => people::Ending::Failed,
        tasks::Ending::Cancelled { by, at, .. } => people::Ending::Cancelled { by: party(*by), at: *at },
    };
    people::Refusal::Ended { how }
}

fn party(by: tasks::Party) -> people::Party {
    match by {
        tasks::Party::Person(person) => people::Party::Person(person),
        tasks::Party::Task(task) => people::Party::Task(task),
        tasks::Party::Deployment { project } => people::Party::Deployment { project },
    }
}

pub(super) fn pool_refused(work: &mut Queue<Event>, request: Token, why: people::Refusal) {
    work.push(Event::People(people::Event::Decided { request, outcome: people::Outcome::Refused(why) }));
}

pub(super) fn pool_again(work: &mut Queue<Event>, request: Token, why: people::Again) {
    work.push(Event::People(people::Event::Decided { request, outcome: people::Outcome::Again(why) }));
}

pub(super) fn note_person_refusal(why: notes::Refusal) -> people::Outcome {
    let refusal = match why {
        notes::Refusal::Busy => return people::Outcome::Again(people::Again::Busy),
        notes::Refusal::Oversized => people::Refusal::Limit { which: people::LimitKind::Words },
        notes::Refusal::Full => people::Refusal::Limit { which: people::LimitKind::Notes },
        notes::Refusal::RevisionExhausted => people::Refusal::Limit { which: people::LimitKind::Revision },
        notes::Refusal::Missing => people::Refusal::Ended { how: people::Ending::Deleted },
        notes::Refusal::Moved { revision } => people::Refusal::Moved { now: people::Moved::Note { revision } },
        notes::Refusal::Exists => people::Refusal::Limit { which: people::LimitKind::Duplicate },
    };
    people::Outcome::Refused(refusal)
}
