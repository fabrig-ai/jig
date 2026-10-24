//! Root translations preserve the party's core refusal facts (domain/connectors.md, 5.5).
use jig_domain_people as people;
use jig_test_connector::application_core as application;

#[expect(clippy::manual_map, reason = "step code translates optional role numbers by matching")]
pub(super) fn refusal(value: people::Refusal) -> application::Refusal {
    match value {
        people::Refusal::NotWellFormed { why: people::Malformed::Name(_), .. }
        | people::Refusal::Stale { .. }
        | people::Refusal::Full { .. }
        | people::Refusal::NotWellFormed {
            why: people::Malformed::Settings { .. } | people::Malformed::SettingsUnchanged | people::Malformed::Running,
            ..
        }
        | people::Refusal::NameTaken
        | people::Refusal::Circle { .. }
        | people::Refusal::Dependency { .. }
        | people::Refusal::Kept { .. }
        | people::Refusal::SignInRedirect { .. }
        | people::Refusal::AccountMove { .. } => {
            unreachable!("the application routes only Release, never Create, Amend or Answer")
        }
        people::Refusal::Role { held } => application::Refusal::Role {
            held: match held {
                Some(role) => Some(role.number()),
                None => None,
            },
        },
        people::Refusal::Authority { proposable, lacking } => application::Refusal::Authority {
            proposable: match proposable {
                Some(people::Accepter::Role(_)) => Some(true),
                Some(people::Accepter::Nobody) => Some(false),
                None => None,
            },
            lacking: lack(lacking),
        },
        people::Refusal::Limit { which } => application::Refusal::Limit { which: limit_kind(which) },
        people::Refusal::Moved { now } => application::Refusal::Moved { now: moved(now) },
        people::Refusal::Ended { how } => application::Refusal::Ended { how: ending(how) },
        people::Refusal::Taken { by } => application::Refusal::Taken { by },
        people::Refusal::NotWellFormed { why: people::Malformed::Cycle, .. } => {
            application::Refusal::Limit { which: application::LimitKind::Cycle }
        }
        people::Refusal::NotWellFormed {
            why: people::Malformed::AsAllowedTwice | people::Malformed::ProposedAsAllowed,
            ..
        } => application::Refusal::Limit { which: application::LimitKind::Batch },
        people::Refusal::NotWellFormed {
            why: people::Malformed::Conversation | people::Malformed::EndedBy, ..
        } => application::Refusal::Limit { which: application::LimitKind::Spec },
        people::Refusal::NotWaiting => application::Refusal::NotWaiting,
        people::Refusal::NoFurther => application::Refusal::NoFurther,
        people::Refusal::Unknown => application::Refusal::Unknown,
        people::Refusal::Provider | people::Refusal::NotOffered | people::Refusal::NoEscalationHolder => {
            application::Refusal::NotOffered
        }
        people::Refusal::KeyConflict => application::Refusal::KeyConflict,
        // The application connector names no model: a refused choice is its models' lack.
        people::Refusal::Account { .. } => {
            application::Refusal::Authority { lacking: application::Lack::Account, proposable: None }
        }
        people::Refusal::Llm { .. } => {
            application::Refusal::Authority { lacking: application::Lack::Models, proposable: None }
        }
    }
}

pub(super) fn lack(value: people::Lack) -> application::Lack {
    match value {
        people::Lack::Approver => application::Lack::Approver,
        people::Lack::Again { required } => application::Lack::Again { required },
        people::Lack::Budget { needs, left } => application::Lack::Budget { needs, left },
        people::Lack::Grant { kind } => application::Lack::Grant { kind },
        people::Lack::Funding => application::Lack::Funding,
        people::Lack::TaskKind { .. } => application::Lack::TaskKind,
        people::Lack::Depth { most } => application::Lack::Depth { most },
        people::Lack::Deadline => application::Lack::Deadline,
        people::Lack::Tasks { left } => application::Lack::Tasks { left },
        people::Lack::Tool => application::Lack::Tool,
        people::Lack::Scope => application::Lack::Scope,
        people::Lack::Models => application::Lack::Models,
        people::Lack::Reference => application::Lack::Reference,
        people::Lack::ResourceAccess => application::Lack::ResourceAccess,
        people::Lack::ResourceUnavailable => application::Lack::ResourceUnavailable,
        people::Lack::Shape => application::Lack::Shape,
        people::Lack::Account => application::Lack::Account,
        people::Lack::Writer => application::Lack::Writer,
        people::Lack::Required { connector, kind } => application::Lack::Required { connector, kind },
        people::Lack::Failed { connector, kind } => application::Lack::Failed { connector, kind },
        people::Lack::Unguarded { connector, kind } => application::Lack::Unguarded { connector, kind },
    }
}

pub(super) fn limit_kind(value: people::LimitKind) -> application::LimitKind {
    match value {
        people::LimitKind::Words => application::LimitKind::Words,
        people::LimitKind::Identity => application::LimitKind::Identity,
        people::LimitKind::Roles => application::LimitKind::Roles,
        people::LimitKind::Arithmetic => application::LimitKind::Arithmetic,
        people::LimitKind::Duplicate => application::LimitKind::Duplicate,
        people::LimitKind::Empty => application::LimitKind::Empty,
        people::LimitKind::Batch => application::LimitKind::Batch,
        people::LimitKind::ProjectLiveTasks | people::LimitKind::Goals => application::LimitKind::ProjectLiveTasks,
        people::LimitKind::LiveTasks => application::LimitKind::LiveTasks,
        people::LimitKind::Tree => application::LimitKind::Tree,
        people::LimitKind::Depth => application::LimitKind::Depth,
        people::LimitKind::Delegates => application::LimitKind::Delegates,
        people::LimitKind::Subscription => application::LimitKind::Subscription,
        people::LimitKind::Dependencies => application::LimitKind::Dependencies,
        people::LimitKind::Cycle => application::LimitKind::Cycle,
        people::LimitKind::Operations | people::LimitKind::Spec | people::LimitKind::Grace => {
            application::LimitKind::Spec
        }
        people::LimitKind::Contract => application::LimitKind::Contract,
        people::LimitKind::Authority => application::LimitKind::Authority,
        people::LimitKind::Inputs => application::LimitKind::Inputs,
        people::LimitKind::Attempts => application::LimitKind::Attempts,
        people::LimitKind::LiveDelegates => application::LimitKind::LiveDelegates,
        people::LimitKind::Restore => application::LimitKind::Restore,
        people::LimitKind::Read => application::LimitKind::Read,
        people::LimitKind::Turn => application::LimitKind::Turn,
        people::LimitKind::HoldKind => application::LimitKind::HoldKind,
        people::LimitKind::HoldTaken => application::LimitKind::HoldTaken,
        people::LimitKind::Holds => application::LimitKind::Holds,
        people::LimitKind::InboxFull => application::LimitKind::InboxFull,
        people::LimitKind::Notes => application::LimitKind::Notes,
        people::LimitKind::Revision => application::LimitKind::Revision,
        people::LimitKind::Question => application::LimitKind::Question,
        people::LimitKind::Priority => application::LimitKind::Priority,
        people::LimitKind::Policy => application::LimitKind::Policy,
        people::LimitKind::Resource => application::LimitKind::Resource,
        people::LimitKind::Service => application::LimitKind::Service,
        people::LimitKind::Projects
        | people::LimitKind::Accounts
        | people::LimitKind::HolderAccounts
        | people::LimitKind::Sources
        | people::LimitKind::SettingsProposals
        | people::LimitKind::SignIns => unreachable!("an application asks for no account"),
    }
}

pub(super) fn moved(value: people::Moved) -> application::Moved {
    match value {
        people::Moved::Hold { revision, reason } => application::Moved::Hold { revision, reason: hold_reason(reason) },
        people::Moved::Spec { revision } => application::Moved::Spec { revision },
        people::Moved::Person { revision } => application::Moved::Person { revision },
        people::Moved::Note { revision } => application::Moved::Note { revision },
    }
}

pub(super) fn hold_reason(value: people::HoldReason) -> application::HoldReason {
    match value {
        people::HoldReason::Exceeded(value) => application::HoldReason::Exceeded(application::Exceeded {
            bound: match value.bound {
                people::Bound::Reasoning => application::Bound::Reasoning,
                people::Bound::Metadata => application::Bound::Metadata,
                people::Bound::Items => application::Bound::Items,
                people::Bound::Answer => application::Bound::Answer,
                people::Bound::Tools => application::Bound::Tools,
                people::Bound::HistoryItems => application::Bound::HistoryItems,
                people::Bound::History => application::Bound::History,
                people::Bound::Window => application::Bound::Window,
                people::Bound::Output => application::Bound::Output,
            },
            value: value.value,
        }),
        people::HoldReason::Failures { class } => application::HoldReason::Failures { class: failure_class(class) },
        people::HoldReason::Stalled => application::HoldReason::Stalled,
        people::HoldReason::Dependency { task } => application::HoldReason::Dependency { task },
        people::HoldReason::Stopped => application::HoldReason::Stopped,
        people::HoldReason::StoppedBy { party } => application::HoldReason::StoppedBy { party },
        people::HoldReason::Drift => application::HoldReason::Drift,
        people::HoldReason::Effects => application::HoldReason::Effects,
        people::HoldReason::EffectFailed => application::HoldReason::EffectFailed,
        people::HoldReason::Uncertain { entry } => application::HoldReason::Uncertain { entry },
        people::HoldReason::Budget => application::HoldReason::Budget,
        people::HoldReason::Deadline => application::HoldReason::Deadline,
        people::HoldReason::HoldsWaited => application::HoldReason::HoldsWaited,
        people::HoldReason::ResourceUnavailable => application::HoldReason::ResourceUnavailable,
        people::HoldReason::ResourceAccess => application::HoldReason::ResourceAccess,
        people::HoldReason::PoolLost { pool } => application::HoldReason::PoolLost { pool },
        people::HoldReason::Procedure => application::HoldReason::Procedure,
        people::HoldReason::Circle => application::HoldReason::Circle,
    }
}

pub(super) fn failure_class(value: people::FailureClass) -> application::FailureClass {
    match value {
        people::FailureClass::Transient => application::FailureClass::Transient,
        people::FailureClass::Permanent => application::FailureClass::Permanent,
        people::FailureClass::Run => application::FailureClass::Run,
        people::FailureClass::Agent => application::FailureClass::Agent,
        people::FailureClass::Lost => application::FailureClass::Lost,
        people::FailureClass::Invalid => application::FailureClass::Invalid,
    }
}

pub(super) fn party(value: people::Party) -> application::Party {
    match value {
        people::Party::Person(number) => application::Party::Person(number),
        people::Party::Task(number) => application::Party::Task(number),
        people::Party::Deployment { project } => application::Party::Deployment { project },
    }
}

pub(super) fn ending(value: people::Ending) -> application::Ending {
    match value {
        people::Ending::Done => application::Ending::Done,
        people::Ending::Failed => application::Ending::Failed,
        people::Ending::Cancelled { by, at } => application::Ending::Cancelled { by: party(by), at },
        people::Ending::Decided { by, at } => application::Ending::Decided { by, at },
        people::Ending::Deleted => application::Ending::Deleted,
    }
}

pub(super) fn again(value: people::Again) -> application::Again {
    match value {
        people::Again::Busy => application::Again::Busy,
        people::Again::NotReady => application::Again::NotReady,
    }
}
