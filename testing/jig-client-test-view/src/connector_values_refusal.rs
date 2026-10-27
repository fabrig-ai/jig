//! Display translation of sparse owner refusals (contract.md, 4; view.md, 7).
//! Absent facts remain absent. The exact terminal remains in `ConnectorAnswer`.
use jig_client_domain::contract as client;
use jig_client_test_domain::connector_values::application_core as owner;

pub(super) fn refusal(value: owner::Refusal) -> Option<client::Refusal> {
    Some(match value {
        // A raw connector role supplies no client policy-role category.
        // The owner renderer shows this complete fact directly.
        owner::Refusal::Role { .. } => return None,
        owner::Refusal::Authority { lacking, proposable } => {
            client::Refusal::Authority { lacking: lack(lacking), proposable }
        }
        owner::Refusal::Limit { which } => {
            client::Refusal::Limit { which: client::Limit::Admission { which: limit(which) } }
        }
        owner::Refusal::Moved { now } => {
            let (revision, what) = match now {
                owner::Moved::Hold { revision, reason } => {
                    (revision, client::Moved::Hold { reason: Some(client::CurrentHold::Reason(hold(reason))) })
                }
                owner::Moved::Spec { revision } => (revision, client::Moved::Spec),
                owner::Moved::Person { revision } => (revision, client::Moved::PersonTask),
                owner::Moved::Note { revision } => (u64::from(revision), client::Moved::Note),
            };
            client::Refusal::Moved { now: client::Revision { value: revision }, what }
        }
        owner::Refusal::Ended { how } => client::Refusal::Ended { how: ending(how) },
        owner::Refusal::Taken { by } => client::Refusal::Taken { by },
        owner::Refusal::NotWaiting => client::Refusal::NotWaiting,
        owner::Refusal::NoFurther => client::Refusal::NoFurther,
        owner::Refusal::Unknown => client::Refusal::Unknown,
        owner::Refusal::NotOffered => client::Refusal::NotOffered,
        owner::Refusal::KeyConflict => client::Refusal::KeyConflict,
    })
}
fn lack(value: owner::Lack) -> client::Lacking {
    match value {
        owner::Lack::Budget { needs, left } => client::Lacking::Budget { needed: needs, left },
        owner::Lack::Grant { kind } => client::Lacking::GrantKind { kind },
        owner::Lack::TaskKind => client::Lacking::TaskKind { executor: None },
        owner::Lack::Depth { most } => client::Lacking::Depth { needed: None, allowed: most },
        owner::Lack::Deadline => client::Lacking::Deadline { asked: None, latest: None },
        owner::Lack::Tasks { left } => client::Lacking::Tasks { needed: None, left },
        owner::Lack::Approver => client::Lacking::Approver,
        owner::Lack::Funding => client::Lacking::Funding,
        owner::Lack::Tool => client::Lacking::Tool,
        owner::Lack::Scope => client::Lacking::Scope,
        owner::Lack::Models => client::Lacking::Models,
        owner::Lack::Reference => client::Lacking::Reference,
        owner::Lack::ResourceAccess => client::Lacking::ResourceAccess,
        owner::Lack::ResourceUnavailable => client::Lacking::ResourceUnavailable,
        owner::Lack::Shape => client::Lacking::Shape,
        owner::Lack::Account => client::Lacking::Account,
        owner::Lack::Writer => client::Lacking::Writer,
        owner::Lack::Again { required } => client::Lacking::Again { required },
        owner::Lack::Required { connector, kind } => client::Lacking::Required { connector, kind },
        owner::Lack::Failed { connector, kind } => client::Lacking::Failed { connector, kind },
        owner::Lack::Unguarded { connector, kind } => client::Lacking::Unguarded { connector, kind },
    }
}
fn limit(value: owner::LimitKind) -> client::AdmissionLimit {
    match value {
        owner::LimitKind::Words => client::AdmissionLimit::Words,
        owner::LimitKind::Identity => client::AdmissionLimit::Identity,
        owner::LimitKind::Roles => client::AdmissionLimit::Roles,
        owner::LimitKind::Arithmetic => client::AdmissionLimit::Arithmetic,
        owner::LimitKind::Duplicate => client::AdmissionLimit::Duplicate,
        owner::LimitKind::Empty => client::AdmissionLimit::Empty,
        owner::LimitKind::Batch => client::AdmissionLimit::Batch,
        owner::LimitKind::ProjectLiveTasks => client::AdmissionLimit::ProjectLiveTasks,
        owner::LimitKind::LiveTasks => client::AdmissionLimit::LiveTasks,
        owner::LimitKind::Tree => client::AdmissionLimit::Tree,
        owner::LimitKind::Depth => client::AdmissionLimit::Depth,
        owner::LimitKind::Delegates => client::AdmissionLimit::Delegates,
        owner::LimitKind::Subscription => client::AdmissionLimit::Subscription,
        owner::LimitKind::Dependencies => client::AdmissionLimit::Dependencies,
        owner::LimitKind::Cycle => client::AdmissionLimit::Cycle,
        owner::LimitKind::Spec => client::AdmissionLimit::Spec,
        owner::LimitKind::Contract => client::AdmissionLimit::Contract,
        owner::LimitKind::Authority => client::AdmissionLimit::Authority,
        owner::LimitKind::Inputs => client::AdmissionLimit::Inputs,
        owner::LimitKind::Attempts => client::AdmissionLimit::Attempts,
        owner::LimitKind::LiveDelegates => client::AdmissionLimit::LiveDelegates,
        owner::LimitKind::Restore => client::AdmissionLimit::Restore,
        owner::LimitKind::Read => client::AdmissionLimit::Read,
        owner::LimitKind::Turn => client::AdmissionLimit::Turn,
        owner::LimitKind::HoldKind => client::AdmissionLimit::HoldKind,
        owner::LimitKind::HoldTaken => client::AdmissionLimit::HoldTaken,
        owner::LimitKind::Holds => client::AdmissionLimit::Holds,
        owner::LimitKind::InboxFull => client::AdmissionLimit::InboxFull,
        owner::LimitKind::Notes => client::AdmissionLimit::Notes,
        owner::LimitKind::Revision => client::AdmissionLimit::Revision,
        owner::LimitKind::Question => client::AdmissionLimit::Question,
        owner::LimitKind::Priority => client::AdmissionLimit::Priority,
        owner::LimitKind::Policy => client::AdmissionLimit::Policy,
        owner::LimitKind::Resource => client::AdmissionLimit::Resource,
        owner::LimitKind::Service => client::AdmissionLimit::Service,
    }
}
fn hold(value: owner::HoldReason) -> client::HoldCategory {
    match value {
        owner::HoldReason::Exceeded(value) => client::HoldCategory::Exceeded(client::Exceeded {
            bound: match value.bound {
                owner::Bound::Reasoning => client::Bound::Reasoning,
                owner::Bound::Metadata => client::Bound::Metadata,
                owner::Bound::Items => client::Bound::Items,
                owner::Bound::Answer => client::Bound::Answer,
                owner::Bound::Tools => client::Bound::Tools,
                owner::Bound::HistoryItems => client::Bound::HistoryItems,
                owner::Bound::History => client::Bound::History,
                owner::Bound::Window => client::Bound::Window,
                owner::Bound::Output => client::Bound::Output,
            },
            value: value.value,
        }),
        owner::HoldReason::Failures { class } => client::HoldCategory::Failures {
            class: match class {
                owner::FailureClass::Transient => client::HoldFailure::Transient,
                owner::FailureClass::Permanent => client::HoldFailure::Permanent,
                owner::FailureClass::Run => client::HoldFailure::Run,
                owner::FailureClass::Agent => client::HoldFailure::Agent,
                owner::FailureClass::Lost => client::HoldFailure::Lost,
                owner::FailureClass::Invalid => client::HoldFailure::Invalid,
            },
        },
        owner::HoldReason::Dependency { task } => client::HoldCategory::Dependency { task },
        owner::HoldReason::Placed { hold: party } => client::HoldCategory::Placed { hold: party },
        owner::HoldReason::Uncertain { entry } => client::HoldCategory::Uncertain { entry },
        owner::HoldReason::PoolLost { pool } => client::HoldCategory::PoolLost { pool },
        owner::HoldReason::Stalled => client::HoldCategory::Stalled,
        owner::HoldReason::Stopped => client::HoldCategory::Stopped,
        owner::HoldReason::Drift => client::HoldCategory::Drift,
        owner::HoldReason::Effects => client::HoldCategory::Effects,
        owner::HoldReason::EffectFailed => client::HoldCategory::EffectFailed,
        owner::HoldReason::Budget => client::HoldCategory::Budget,
        owner::HoldReason::Deadline => client::HoldCategory::Deadline,
        owner::HoldReason::HoldsWaited => client::HoldCategory::HoldsWaited,
        owner::HoldReason::ResourceUnavailable => client::HoldCategory::ResourceUnavailable,
        owner::HoldReason::ResourceAccess => client::HoldCategory::ResourceAccess,
        owner::HoldReason::Procedure => client::HoldCategory::Procedure,
        owner::HoldReason::Circle => client::HoldCategory::Circle,
    }
}
fn ending(value: owner::Ending) -> client::Ending {
    match value {
        owner::Ending::Done => client::Ending::Done,
        owner::Ending::Failed => client::Ending::Failed { reason: None },
        owner::Ending::Cancelled { by, at } => client::Ending::Cancelled {
            by: match by {
                owner::Party::Person(number) => client::Actor::Party(number),
                owner::Party::Task(number) => client::Actor::Task(number),
                owner::Party::Deployment { project } => client::Actor::ProjectDeployment { project },
            },
            reason: None,
            at: Some(at),
        },
        owner::Ending::Decided { by, at } => client::Ending::Decided { by, at, party: None, how: None },
        owner::Ending::Deleted => client::Ending::Deleted,
    }
}
