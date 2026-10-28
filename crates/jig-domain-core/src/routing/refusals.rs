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
    let tasks::Problem { task, why, blocked_by, capacity, limit } = problem;
    if let Some(capacity) = capacity
        && let Some(which) = placed_limit(capacity.cause)
    {
        return people::Outcome::Refused(core.limit_refusal(which));
    }
    let names = match blocked_by {
        Some(names) => names,
        None => Box::new([]),
    };
    let refusal = match why {
        tasks::Refusal::Unproposable => {
            people::Refusal::Authority { lacking: people::Lack::Approver, proposable: Some(people::Accepter::Nobody) }
        }
        tasks::Refusal::HoldRevision { revision } => {
            people::Refusal::Moved { now: people::Moved::PlacedHold { revision } }
        }
        tasks::Refusal::HoldOwner { hold } => match core.tasks.placed_hold(hold) {
            Some(row) => people::Refusal::Authority {
                proposable: None,
                lacking: people::Lack::HoldOwner { hold, owner: crate::roles::hold_owner(row.owner) },
            },
            None => people::Refusal::Unknown,
        },
        tasks::Refusal::PersonRevision { revision } => {
            people::Refusal::Moved { now: people::Moved::Person { revision } }
        }
        tasks::Refusal::Busy => return people::Outcome::Again(people::Again::Busy),
        tasks::Refusal::NotReady => return people::Outcome::Again(people::Again::NotReady),
        tasks::Refusal::ResourceUnavailable => {
            people::Refusal::Authority { proposable: None, lacking: people::Lack::ResourceUnavailable }
        }
        tasks::Refusal::Unknown => people::Refusal::Unknown,
        tasks::Refusal::State | tasks::Refusal::Placed { .. } => match current_task(core, task) {
            Some(row) => match &row.phase {
                tasks::Phase::Closing(closing) => ended(&closing.ending),
                tasks::Phase::Ended(ending) => ended(ending),
                tasks::Phase::Waiting | tasks::Phase::Active(_) | tasks::Phase::Held { .. } => {
                    people::Refusal::NotWaiting
                }
            },
            None => people::Refusal::Unknown,
        },
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
        tasks::Refusal::Duplicate => core.limit_refusal(people::LimitKind::Duplicate),
        tasks::Refusal::Empty => core.limit_refusal(people::LimitKind::Empty),
        tasks::Refusal::Batch => core.limit_refusal(people::LimitKind::Batch),
        tasks::Refusal::Live => core.limit_refusal(people::LimitKind::LiveTasks),
        tasks::Refusal::Project => core.limit_refusal(people::LimitKind::ProjectLiveTasks),
        tasks::Refusal::Tree => core.limit_refusal(people::LimitKind::Tree),
        tasks::Refusal::Depth => core.limit_refusal(people::LimitKind::Depth),
        tasks::Refusal::Delegates => core.limit_refusal(people::LimitKind::Delegates),
        tasks::Refusal::Subscription => core.limit_refusal(people::LimitKind::Subscription),
        tasks::Refusal::Dependencies => core.limit_refusal(people::LimitKind::Dependencies),
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
        tasks::Refusal::Grace => core.limit_refusal(people::LimitKind::Grace),
        tasks::Refusal::Goals => core.limit_refusal(people::LimitKind::Goals),
        tasks::Refusal::Spec => core.limit_refusal(people::LimitKind::Spec),
        tasks::Refusal::Contract => core.limit_refusal(people::LimitKind::Contract),
        tasks::Refusal::Inputs => core.limit_refusal(people::LimitKind::Inputs),
        tasks::Refusal::Attempt => core.limit_refusal(people::LimitKind::Attempts),
        tasks::Refusal::LiveDelegates => core.limit_refusal(people::LimitKind::LiveDelegates),
        tasks::Refusal::Restore => core.limit_refusal(people::LimitKind::Restore),
        tasks::Refusal::Read => core.limit_refusal(people::LimitKind::Read),
        tasks::Refusal::Turn => core.limit_refusal(people::LimitKind::Turn),
        tasks::Refusal::HoldKind => core.limit_refusal(people::LimitKind::HoldKind),
        tasks::Refusal::HoldTaken => core.limit_refusal(people::LimitKind::HoldTaken),
        tasks::Refusal::Holds => core.limit_refusal(people::LimitKind::Holds),
    };
    people::Outcome::Refused(with_limit(refusal, capacity, limit))
}

fn with_limit(
    mut refusal: people::Refusal,
    capacity: Option<tasks::Capacity>,
    limit: Option<tasks::NumericLimit>,
) -> people::Refusal {
    if capacity.is_none()
        && limit.is_none()
        && let people::Refusal::Limit { bound, which } = &mut refusal
        && *which != people::LimitKind::Goals
    {
        *bound = None;
    }
    if let Some(value) = match limit {
        Some(limit) => Some(numeric_limit(limit)),
        None => match capacity {
            Some(capacity) => capacity_limit(capacity.cause),
            None => None,
        },
    } {
        if let people::Refusal::Limit { bound, .. } = &mut refusal {
            *bound = Some(value);
        }
        if let people::Refusal::Authority { lacking: people::Lack::Shape, .. } = &refusal {
            refusal = people::Refusal::Limit { which: people::LimitKind::Authority, bound: Some(value) };
        }
    }
    refusal
}

pub(crate) fn numeric_limit(limit: tasks::NumericLimit) -> people::LimitBound {
    let (key, unit) = match limit.key {
        tasks::LimitKey::ContractWords => return people::LimitBound::fixed(limit.value, people::LimitUnit::Bytes),
        tasks::LimitKey::Batch => (people::LimitKey::TasksBatch, people::LimitUnit::Items),
        tasks::LimitKey::Tasks => (people::LimitKey::TasksTasks, people::LimitUnit::Items),
        tasks::LimitKey::ProjectTasks => (people::LimitKey::TasksProjectTasks, people::LimitUnit::Items),
        tasks::LimitKey::ProjectGoals => (people::LimitKey::TasksProjectGoals, people::LimitUnit::Items),
        tasks::LimitKey::TreeTasks => (people::LimitKey::TasksTreeTasks, people::LimitUnit::Items),
        tasks::LimitKey::Depth => (people::LimitKey::TasksDepth, people::LimitUnit::Items),
        tasks::LimitKey::Delegates => (people::LimitKey::TasksDelegates, people::LimitUnit::Items),
        tasks::LimitKey::References => (people::LimitKey::TasksReferences, people::LimitUnit::Items),
        tasks::LimitKey::Subscriptions => (people::LimitKey::TasksSubscriptions, people::LimitUnit::Items),
        tasks::LimitKey::Dependencies => (people::LimitKey::TasksDependencies, people::LimitUnit::Items),
        tasks::LimitKey::SpecBytes => (people::LimitKey::TasksSpecBytes, people::LimitUnit::Bytes),
        tasks::LimitKey::Parameters => (people::LimitKey::TasksParameters, people::LimitUnit::Items),
        tasks::LimitKey::ContractChoices => (people::LimitKey::TasksContractChoices, people::LimitUnit::Items),
        tasks::LimitKey::ResultBytes => (people::LimitKey::TasksResultBytes, people::LimitUnit::Bytes),
        tasks::LimitKey::Inputs => (people::LimitKey::TasksInputs, people::LimitUnit::Items),
        tasks::LimitKey::Holdings => (people::LimitKey::TasksHoldings, people::LimitUnit::Items),
        tasks::LimitKey::HoldWaiters => (people::LimitKey::TasksHoldWaiters, people::LimitUnit::Items),
        tasks::LimitKey::InboxMessages => (people::LimitKey::TasksInboxMessages, people::LimitUnit::Items),
        tasks::LimitKey::InboxBytes => (people::LimitKey::TasksInboxBytes, people::LimitUnit::Bytes),
        tasks::LimitKey::MessageBytes => (people::LimitKey::TasksMessageBytes, people::LimitUnit::Bytes),
        tasks::LimitKey::Grace => (people::LimitKey::TasksGrace, people::LimitUnit::Milliseconds),
        tasks::LimitKey::PlacedHolds => (people::LimitKey::TasksPlacedHolds, people::LimitUnit::Items),
        tasks::LimitKey::ProjectPlacedHolds => (people::LimitKey::TasksProjectPlacedHolds, people::LimitUnit::Items),
        tasks::LimitKey::PartyPlacedHolds => (people::LimitKey::TasksPartyPlacedHolds, people::LimitUnit::Items),
        tasks::LimitKey::Exemptions => (people::LimitKey::TasksExemptions, people::LimitUnit::Items),
        tasks::LimitKey::AuthorityGrants => (people::LimitKey::TasksAuthorityGrants, people::LimitUnit::Items),
        tasks::LimitKey::AuthorityBytes => (people::LimitKey::TasksAuthorityBytes, people::LimitUnit::Bytes),
        tasks::LimitKey::AuthoritySegments => (people::LimitKey::TasksAuthoritySegments, people::LimitUnit::Items),
        tasks::LimitKey::HoldSegments => (people::LimitKey::TasksHoldSegments, people::LimitUnit::Items),
        tasks::LimitKey::HoldBytes => (people::LimitKey::TasksHoldBytes, people::LimitUnit::Bytes),
        tasks::LimitKey::Funders => (people::LimitKey::TasksFunders, people::LimitUnit::Items),
        tasks::LimitKey::ExecutorKinds => (people::LimitKey::TasksExecutorKinds, people::LimitUnit::Items),
        tasks::LimitKey::ResourceReports => (people::LimitKey::TasksResourceReports, people::LimitUnit::Items),
    };
    people::LimitBound::configured(limit.value, key, unit)
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

pub(super) fn note_person_refusal(why: notes::Refusal, limit: Option<notes::NumericLimit>) -> people::Outcome {
    let mut refusal = match why {
        notes::Refusal::Busy => return people::Outcome::Again(people::Again::Busy),
        notes::Refusal::Oversized | notes::Refusal::Full => {
            people::Refusal::Limit { which: people::LimitKind::Notes, bound: None }
        }
        notes::Refusal::RevisionExhausted => people::Refusal::Limit { which: people::LimitKind::Revision, bound: None },
        notes::Refusal::Missing => people::Refusal::Ended { how: people::Ending::Deleted },
        notes::Refusal::Moved { revision } => people::Refusal::Moved { now: people::Moved::Note { revision } },
        notes::Refusal::Exists => people::Refusal::Limit { which: people::LimitKind::Duplicate, bound: None },
    };
    if let Some(limit) = limit
        && let people::Refusal::Limit { bound, .. } = &mut refusal
    {
        let (key, unit) = match limit.key {
            notes::LimitKey::PatternBytes => (people::LimitKey::NotesPatternBytes, people::LimitUnit::Bytes),
            notes::LimitKey::DescriptionBytes => (people::LimitKey::NotesDescriptionBytes, people::LimitUnit::Bytes),
            notes::LimitKey::BodyBytes => (people::LimitKey::NotesBodyBytes, people::LimitUnit::Bytes),
            notes::LimitKey::References => (people::LimitKey::NotesReferences, people::LimitUnit::Items),
            notes::LimitKey::EntriesPerScope => (people::LimitKey::NotesEntriesPerScope, people::LimitUnit::Items),
            notes::LimitKey::Scopes => (people::LimitKey::NotesScopes, people::LimitUnit::Items),
        };
        *bound = Some(people::LimitBound::configured(u64::from(limit.value), key, unit));
    }
    people::Outcome::Refused(refusal)
}

fn capacity_limit(cause: tasks::CapacityCause) -> Option<people::LimitBound> {
    use people::{LimitBound as B, LimitKey as K, LimitUnit as U};
    Some(match cause {
        tasks::CapacityCause::PlacedHolds { limit } => B::configured(u64::from(limit), K::TasksPlacedHolds, U::Items),
        tasks::CapacityCause::ProjectPlacedHolds { limit } => {
            B::configured(u64::from(limit), K::TasksProjectPlacedHolds, U::Items)
        }
        tasks::CapacityCause::PartyPlacedHolds { limit } => {
            B::configured(u64::from(limit), K::TasksPartyPlacedHolds, U::Items)
        }
        tasks::CapacityCause::Exemptions { limit } => B::configured(u64::from(limit), K::TasksExemptions, U::Items),
        tasks::CapacityCause::Tasks { limit } => B::configured(u64::from(limit), K::TasksTasks, U::Items),
        tasks::CapacityCause::ResourceReports { limit } => {
            B::configured(u64::from(limit), K::TasksResourceReports, U::Items)
        }
        tasks::CapacityCause::ProjectTasks { limit } => B::configured(u64::from(limit), K::TasksProjectTasks, U::Items),
        tasks::CapacityCause::Tree { limit } => B::configured(u64::from(limit), K::TasksTreeTasks, U::Items),
        tasks::CapacityCause::Delegates { limit } => B::configured(u64::from(limit), K::TasksDelegates, U::Items),
        tasks::CapacityCause::Holdings { limit } => B::configured(u64::from(limit), K::TasksHoldings, U::Items),
        tasks::CapacityCause::HoldWaiters { limit } => B::configured(u64::from(limit), K::TasksHoldWaiters, U::Items),
        tasks::CapacityCause::References { limit } => B::configured(u64::from(limit), K::TasksReferences, U::Items),
        tasks::CapacityCause::Subscriptions { limit } => {
            B::configured(u64::from(limit), K::TasksSubscriptions, U::Items)
        }
        tasks::CapacityCause::Funders { limit } => B::configured(u64::from(limit), K::TasksFunders, U::Items),
        tasks::CapacityCause::CreationFlights { limit }
        | tasks::CapacityCause::Proposals { limit }
        | tasks::CapacityCause::Writers { limit } => B::fixed(u64::from(limit), U::Items),
        tasks::CapacityCause::Inbox { .. } => return None,
    })
}
