//! The connector's retained core facts, translated by value.
use crate::v2 as w;
use jig_test_connector as d;
use skein_lib::Wall;
pub(super) fn encode_refusal(value: &d::application_core::Refusal) -> w::Refusal {
    match value {
        d::application_core::Refusal::Role { held } => w::Refusal::Role(
            w::RefusalRole::new(&w::CEILINGS, w::RefusalRoleParts { held: *held }).expect("admitted fields fit schema"),
        ),
        d::application_core::Refusal::Authority { lacking, proposable } => w::Refusal::Authority(
            w::RefusalAuthority::new(
                &w::CEILINGS,
                w::RefusalAuthorityParts { lacking: encode_lack(lacking), proposable: *proposable },
            )
            .expect("admitted fields fit schema"),
        ),
        d::application_core::Refusal::Limit { which } => w::Refusal::Limit(
            w::RefusalLimit::new(&w::CEILINGS, w::RefusalLimitParts { which: encode_limit_kind(*which) })
                .expect("admitted fields fit schema"),
        ),
        d::application_core::Refusal::Moved { now } => w::Refusal::Moved(
            w::RefusalMoved::new(&w::CEILINGS, w::RefusalMovedParts { now: encode_moved(now) })
                .expect("admitted fields fit schema"),
        ),
        d::application_core::Refusal::Ended { how } => w::Refusal::Ended(
            w::RefusalEnded::new(&w::CEILINGS, w::RefusalEndedParts { how: encode_ending(how) })
                .expect("admitted fields fit schema"),
        ),
        d::application_core::Refusal::Taken { by } => w::Refusal::Taken(
            w::RefusalTaken::new(&w::CEILINGS, w::RefusalTakenParts { by: *by }).expect("admitted fields fit schema"),
        ),
        d::application_core::Refusal::NotWaiting => w::Refusal::NotWaiting,
        d::application_core::Refusal::NoFurther => w::Refusal::NoFurther,
        d::application_core::Refusal::Unknown => w::Refusal::Unknown,
        d::application_core::Refusal::NotOffered => w::Refusal::NotOffered,
        d::application_core::Refusal::KeyConflict => w::Refusal::KeyConflict,
    }
}
pub(super) fn decode_refusal(value: w::Refusal) -> d::application_core::Refusal {
    match value {
        w::Refusal::Role(value) => {
            let w::RefusalRoleParts { held } = value.into_parts();
            d::application_core::Refusal::Role { held }
        }
        w::Refusal::Authority(value) => {
            let w::RefusalAuthorityParts { lacking, proposable } = value.into_parts();
            d::application_core::Refusal::Authority { lacking: decode_lack(lacking), proposable }
        }
        w::Refusal::Limit(value) => {
            let w::RefusalLimitParts { which } = value.into_parts();
            d::application_core::Refusal::Limit { which: decode_limit_kind(which) }
        }
        w::Refusal::Moved(value) => {
            let w::RefusalMovedParts { now } = value.into_parts();
            d::application_core::Refusal::Moved { now: decode_moved(now) }
        }
        w::Refusal::Ended(value) => {
            let w::RefusalEndedParts { how } = value.into_parts();
            d::application_core::Refusal::Ended { how: decode_ending(how) }
        }
        w::Refusal::Taken(value) => {
            let w::RefusalTakenParts { by } = value.into_parts();
            d::application_core::Refusal::Taken { by }
        }
        w::Refusal::NotWaiting => d::application_core::Refusal::NotWaiting,
        w::Refusal::NoFurther => d::application_core::Refusal::NoFurther,
        w::Refusal::Unknown => d::application_core::Refusal::Unknown,
        w::Refusal::NotOffered => d::application_core::Refusal::NotOffered,
        w::Refusal::KeyConflict => d::application_core::Refusal::KeyConflict,
    }
}
fn encode_lack(value: &d::application_core::Lack) -> w::Lack {
    match value {
        d::application_core::Lack::Approver => w::Lack::Approver,
        d::application_core::Lack::Again { required } => w::Lack::Again(
            w::LackAgain::new(&w::CEILINGS, w::LackAgainParts { required: *required })
                .expect("admitted fields fit schema"),
        ),
        d::application_core::Lack::Budget { needs, left } => w::Lack::Budget(
            w::LackBudget::new(&w::CEILINGS, w::LackBudgetParts { needs: *needs, left: *left })
                .expect("admitted fields fit schema"),
        ),
        d::application_core::Lack::Grant { kind } => w::Lack::Grant(
            w::LackGrant::new(&w::CEILINGS, w::LackGrantParts { kind: *kind }).expect("admitted fields fit schema"),
        ),
        d::application_core::Lack::Funding => w::Lack::Funding,
        d::application_core::Lack::TaskKind => w::Lack::TaskKind,
        d::application_core::Lack::Depth { most } => w::Lack::Depth(
            w::LackDepth::new(&w::CEILINGS, w::LackDepthParts { most: *most }).expect("admitted fields fit schema"),
        ),
        d::application_core::Lack::Deadline => w::Lack::Deadline,
        d::application_core::Lack::Tasks { left } => w::Lack::Tasks(
            w::LackTasks::new(&w::CEILINGS, w::LackTasksParts { left: *left }).expect("admitted fields fit schema"),
        ),
        d::application_core::Lack::Tool => w::Lack::Tool,
        d::application_core::Lack::Scope => w::Lack::Scope,
        d::application_core::Lack::Models => w::Lack::Models,
        d::application_core::Lack::Reference => w::Lack::Reference,
        d::application_core::Lack::ResourceAccess => w::Lack::ResourceAccess,
        d::application_core::Lack::ResourceUnavailable => w::Lack::ResourceUnavailable,
        d::application_core::Lack::Shape => w::Lack::Shape,
        d::application_core::Lack::Account => w::Lack::Account,
        d::application_core::Lack::Writer => w::Lack::Writer,
        d::application_core::Lack::Required { connector, kind } => w::Lack::Required(
            w::LackRequired::new(&w::CEILINGS, w::LackRequiredParts { connector: *connector, kind: *kind })
                .expect("admitted fields fit schema"),
        ),
        d::application_core::Lack::Failed { connector, kind } => w::Lack::Failed(
            w::LackFailed::new(&w::CEILINGS, w::LackFailedParts { connector: *connector, kind: *kind })
                .expect("admitted fields fit schema"),
        ),
        d::application_core::Lack::Unguarded { connector, kind } => w::Lack::Unguarded(
            w::LackUnguarded::new(&w::CEILINGS, w::LackUnguardedParts { connector: *connector, kind: *kind })
                .expect("admitted fields fit schema"),
        ),
    }
}
fn decode_lack(value: w::Lack) -> d::application_core::Lack {
    match value {
        w::Lack::Approver => d::application_core::Lack::Approver,
        w::Lack::Again(value) => {
            let w::LackAgainParts { required } = value.into_parts();
            d::application_core::Lack::Again { required }
        }
        w::Lack::Budget(value) => {
            let w::LackBudgetParts { needs, left } = value.into_parts();
            d::application_core::Lack::Budget { needs, left }
        }
        w::Lack::Grant(value) => {
            let w::LackGrantParts { kind } = value.into_parts();
            d::application_core::Lack::Grant { kind }
        }
        w::Lack::Funding => d::application_core::Lack::Funding,
        w::Lack::TaskKind => d::application_core::Lack::TaskKind,
        w::Lack::Depth(value) => {
            let w::LackDepthParts { most } = value.into_parts();
            d::application_core::Lack::Depth { most }
        }
        w::Lack::Deadline => d::application_core::Lack::Deadline,
        w::Lack::Tasks(value) => {
            let w::LackTasksParts { left } = value.into_parts();
            d::application_core::Lack::Tasks { left }
        }
        w::Lack::Tool => d::application_core::Lack::Tool,
        w::Lack::Scope => d::application_core::Lack::Scope,
        w::Lack::Models => d::application_core::Lack::Models,
        w::Lack::Reference => d::application_core::Lack::Reference,
        w::Lack::ResourceAccess => d::application_core::Lack::ResourceAccess,
        w::Lack::ResourceUnavailable => d::application_core::Lack::ResourceUnavailable,
        w::Lack::Shape => d::application_core::Lack::Shape,
        w::Lack::Account => d::application_core::Lack::Account,
        w::Lack::Writer => d::application_core::Lack::Writer,
        w::Lack::Required(value) => {
            let w::LackRequiredParts { connector, kind } = value.into_parts();
            d::application_core::Lack::Required { connector, kind }
        }
        w::Lack::Failed(value) => {
            let w::LackFailedParts { connector, kind } = value.into_parts();
            d::application_core::Lack::Failed { connector, kind }
        }
        w::Lack::Unguarded(value) => {
            let w::LackUnguardedParts { connector, kind } = value.into_parts();
            d::application_core::Lack::Unguarded { connector, kind }
        }
    }
}
fn encode_limit_kind(value: d::application_core::LimitKind) -> w::LimitKind {
    match value {
        d::application_core::LimitKind::Words => w::LimitKind::Words,
        d::application_core::LimitKind::Identity => w::LimitKind::Identity,
        d::application_core::LimitKind::Roles => w::LimitKind::Roles,
        d::application_core::LimitKind::Arithmetic => w::LimitKind::Arithmetic,
        d::application_core::LimitKind::Duplicate => w::LimitKind::Duplicate,
        d::application_core::LimitKind::Empty => w::LimitKind::Empty,
        d::application_core::LimitKind::Batch => w::LimitKind::Batch,
        d::application_core::LimitKind::ProjectLiveTasks => w::LimitKind::ProjectLiveTasks,
        d::application_core::LimitKind::LiveTasks => w::LimitKind::LiveTasks,
        d::application_core::LimitKind::Tree => w::LimitKind::Tree,
        d::application_core::LimitKind::Depth => w::LimitKind::Depth,
        d::application_core::LimitKind::Delegates => w::LimitKind::Delegates,
        d::application_core::LimitKind::Subscription => w::LimitKind::Subscription,
        d::application_core::LimitKind::Dependencies => w::LimitKind::Dependencies,
        d::application_core::LimitKind::Cycle => w::LimitKind::Cycle,
        d::application_core::LimitKind::Spec => w::LimitKind::Spec,
        d::application_core::LimitKind::Contract => w::LimitKind::Contract,
        d::application_core::LimitKind::Authority => w::LimitKind::Authority,
        d::application_core::LimitKind::Inputs => w::LimitKind::Inputs,
        d::application_core::LimitKind::Attempts => w::LimitKind::Attempts,
        d::application_core::LimitKind::LiveDelegates => w::LimitKind::LiveDelegates,
        d::application_core::LimitKind::Restore => w::LimitKind::Restore,
        d::application_core::LimitKind::Read => w::LimitKind::Read,
        d::application_core::LimitKind::Turn => w::LimitKind::Turn,
        d::application_core::LimitKind::HoldKind => w::LimitKind::HoldKind,
        d::application_core::LimitKind::HoldTaken => w::LimitKind::HoldTaken,
        d::application_core::LimitKind::Holds => w::LimitKind::Holds,
        d::application_core::LimitKind::InboxFull => w::LimitKind::InboxFull,
        d::application_core::LimitKind::Notes => w::LimitKind::Notes,
        d::application_core::LimitKind::Revision => w::LimitKind::Revision,
        d::application_core::LimitKind::Question => w::LimitKind::Question,
        d::application_core::LimitKind::Priority => w::LimitKind::Priority,
        d::application_core::LimitKind::Policy => w::LimitKind::Policy,
        d::application_core::LimitKind::Resource => w::LimitKind::Resource,
        d::application_core::LimitKind::Service => w::LimitKind::Service,
        d::application_core::LimitKind::PlacedHolds => w::LimitKind::PlacedHolds,
        d::application_core::LimitKind::ProjectPlacedHolds => w::LimitKind::ProjectPlacedHolds,
        d::application_core::LimitKind::PartyPlacedHolds => w::LimitKind::PartyPlacedHolds,
        d::application_core::LimitKind::Exemptions => w::LimitKind::Exemptions,
    }
}
fn decode_limit_kind(value: w::LimitKind) -> d::application_core::LimitKind {
    match value {
        w::LimitKind::Words => d::application_core::LimitKind::Words,
        w::LimitKind::Identity => d::application_core::LimitKind::Identity,
        w::LimitKind::Roles => d::application_core::LimitKind::Roles,
        w::LimitKind::Arithmetic => d::application_core::LimitKind::Arithmetic,
        w::LimitKind::Duplicate => d::application_core::LimitKind::Duplicate,
        w::LimitKind::Empty => d::application_core::LimitKind::Empty,
        w::LimitKind::Batch => d::application_core::LimitKind::Batch,
        w::LimitKind::ProjectLiveTasks => d::application_core::LimitKind::ProjectLiveTasks,
        w::LimitKind::LiveTasks => d::application_core::LimitKind::LiveTasks,
        w::LimitKind::Tree => d::application_core::LimitKind::Tree,
        w::LimitKind::Depth => d::application_core::LimitKind::Depth,
        w::LimitKind::Delegates => d::application_core::LimitKind::Delegates,
        w::LimitKind::Subscription => d::application_core::LimitKind::Subscription,
        w::LimitKind::Dependencies => d::application_core::LimitKind::Dependencies,
        w::LimitKind::Cycle => d::application_core::LimitKind::Cycle,
        w::LimitKind::Spec => d::application_core::LimitKind::Spec,
        w::LimitKind::Contract => d::application_core::LimitKind::Contract,
        w::LimitKind::Authority => d::application_core::LimitKind::Authority,
        w::LimitKind::Inputs => d::application_core::LimitKind::Inputs,
        w::LimitKind::Attempts => d::application_core::LimitKind::Attempts,
        w::LimitKind::LiveDelegates => d::application_core::LimitKind::LiveDelegates,
        w::LimitKind::Restore => d::application_core::LimitKind::Restore,
        w::LimitKind::Read => d::application_core::LimitKind::Read,
        w::LimitKind::Turn => d::application_core::LimitKind::Turn,
        w::LimitKind::HoldKind => d::application_core::LimitKind::HoldKind,
        w::LimitKind::HoldTaken => d::application_core::LimitKind::HoldTaken,
        w::LimitKind::Holds => d::application_core::LimitKind::Holds,
        w::LimitKind::InboxFull => d::application_core::LimitKind::InboxFull,
        w::LimitKind::Notes => d::application_core::LimitKind::Notes,
        w::LimitKind::Revision => d::application_core::LimitKind::Revision,
        w::LimitKind::Question => d::application_core::LimitKind::Question,
        w::LimitKind::Priority => d::application_core::LimitKind::Priority,
        w::LimitKind::Policy => d::application_core::LimitKind::Policy,
        w::LimitKind::Resource => d::application_core::LimitKind::Resource,
        w::LimitKind::Service => d::application_core::LimitKind::Service,
        w::LimitKind::PlacedHolds => d::application_core::LimitKind::PlacedHolds,
        w::LimitKind::ProjectPlacedHolds => d::application_core::LimitKind::ProjectPlacedHolds,
        w::LimitKind::PartyPlacedHolds => d::application_core::LimitKind::PartyPlacedHolds,
        w::LimitKind::Exemptions => d::application_core::LimitKind::Exemptions,
    }
}
fn encode_party(value: &d::application_core::Party) -> w::Party {
    match value {
        d::application_core::Party::Person(value) => w::Party::Person(
            w::PartyPerson::new(&w::CEILINGS, w::PartyPersonParts { value: *value })
                .expect("admitted fields fit schema"),
        ),
        d::application_core::Party::Task(value) => w::Party::Task(
            w::PartyTask::new(&w::CEILINGS, w::PartyTaskParts { value: *value }).expect("admitted fields fit schema"),
        ),
        d::application_core::Party::Deployment { project } => w::Party::Deployment(
            w::PartyDeployment::new(&w::CEILINGS, w::PartyDeploymentParts { project: *project })
                .expect("admitted fields fit schema"),
        ),
    }
}
fn decode_party(value: w::Party) -> d::application_core::Party {
    match value {
        w::Party::Person(value) => {
            let w::PartyPersonParts { value } = value.into_parts();
            d::application_core::Party::Person(value)
        }
        w::Party::Task(value) => {
            let w::PartyTaskParts { value } = value.into_parts();
            d::application_core::Party::Task(value)
        }
        w::Party::Deployment(value) => {
            let w::PartyDeploymentParts { project } = value.into_parts();
            d::application_core::Party::Deployment { project }
        }
    }
}
fn encode_ending(value: &d::application_core::Ending) -> w::Ending {
    match value {
        d::application_core::Ending::Done => w::Ending::Done,
        d::application_core::Ending::Failed => w::Ending::Failed,
        d::application_core::Ending::Cancelled { by, at } => w::Ending::Cancelled(
            w::EndingCancelled::new(&w::CEILINGS, w::EndingCancelledParts { by: encode_party(by), at: at.as_nanos() })
                .expect("admitted fields fit schema"),
        ),
        d::application_core::Ending::Decided { by, at } => w::Ending::Decided(
            w::EndingDecided::new(&w::CEILINGS, w::EndingDecidedParts { by: *by, at: at.as_nanos() })
                .expect("admitted fields fit schema"),
        ),
        d::application_core::Ending::Deleted => w::Ending::Deleted,
    }
}
fn decode_ending(value: w::Ending) -> d::application_core::Ending {
    match value {
        w::Ending::Done => d::application_core::Ending::Done,
        w::Ending::Failed => d::application_core::Ending::Failed,
        w::Ending::Cancelled(value) => {
            let w::EndingCancelledParts { by, at } = value.into_parts();
            d::application_core::Ending::Cancelled { by: decode_party(by), at: Wall::from_nanos(at) }
        }
        w::Ending::Decided(value) => {
            let w::EndingDecidedParts { by, at } = value.into_parts();
            d::application_core::Ending::Decided { by, at: Wall::from_nanos(at) }
        }
        w::Ending::Deleted => d::application_core::Ending::Deleted,
    }
}
fn encode_moved(value: &d::application_core::Moved) -> w::Moved {
    match value {
        d::application_core::Moved::Hold { revision, reason } => w::Moved::Hold(
            w::MovedHold::new(
                &w::CEILINGS,
                w::MovedHoldParts { revision: *revision, reason: encode_hold_reason(reason) },
            )
            .expect("admitted fields fit schema"),
        ),
        d::application_core::Moved::Spec { revision } => w::Moved::Spec(
            w::MovedSpec::new(&w::CEILINGS, w::MovedSpecParts { revision: *revision })
                .expect("admitted fields fit schema"),
        ),
        d::application_core::Moved::Person { revision } => w::Moved::Person(
            w::MovedPerson::new(&w::CEILINGS, w::MovedPersonParts { revision: *revision })
                .expect("admitted fields fit schema"),
        ),
        d::application_core::Moved::Note { revision } => w::Moved::Note(
            w::MovedNote::new(&w::CEILINGS, w::MovedNoteParts { revision: *revision })
                .expect("admitted fields fit schema"),
        ),
    }
}
fn decode_moved(value: w::Moved) -> d::application_core::Moved {
    match value {
        w::Moved::Hold(value) => {
            let w::MovedHoldParts { revision, reason } = value.into_parts();
            d::application_core::Moved::Hold { revision, reason: decode_hold_reason(reason) }
        }
        w::Moved::Spec(value) => {
            let w::MovedSpecParts { revision } = value.into_parts();
            d::application_core::Moved::Spec { revision }
        }
        w::Moved::Person(value) => {
            let w::MovedPersonParts { revision } = value.into_parts();
            d::application_core::Moved::Person { revision }
        }
        w::Moved::Note(value) => {
            let w::MovedNoteParts { revision } = value.into_parts();
            d::application_core::Moved::Note { revision }
        }
    }
}
fn encode_hold_reason(value: &d::application_core::HoldReason) -> w::HoldReason {
    match value {
        d::application_core::HoldReason::Exceeded(value) => w::HoldReason::Exceeded(
            w::Exceeded::new(
                &w::CEILINGS,
                w::ExceededParts {
                    bound: match value.bound {
                        d::application_core::Bound::Reasoning => w::Bound::Reasoning,
                        d::application_core::Bound::Metadata => w::Bound::Metadata,
                        d::application_core::Bound::Items => w::Bound::Items,
                        d::application_core::Bound::Answer => w::Bound::Answer,
                        d::application_core::Bound::Tools => w::Bound::Tools,
                        d::application_core::Bound::HistoryItems => w::Bound::HistoryItems,
                        d::application_core::Bound::History => w::Bound::History,
                        d::application_core::Bound::Window => w::Bound::Window,
                        d::application_core::Bound::Output => w::Bound::Output,
                        d::application_core::Bound::HostPart => w::Bound::HostPart,
                    },
                    value: value.value,
                },
            )
            .expect("admitted fields fit schema"),
        ),
        d::application_core::HoldReason::Failures { class } => w::HoldReason::Failures(
            w::HoldReasonFailures::new(
                &w::CEILINGS,
                w::HoldReasonFailuresParts { class: encode_failure_class(*class) },
            )
            .expect("admitted fields fit schema"),
        ),
        d::application_core::HoldReason::Stalled => w::HoldReason::Stalled,
        d::application_core::HoldReason::Dependency { task } => w::HoldReason::Dependency(
            w::HoldReasonDependency::new(&w::CEILINGS, w::HoldReasonDependencyParts { task: *task })
                .expect("admitted fields fit schema"),
        ),
        d::application_core::HoldReason::Stopped => w::HoldReason::Stopped,
        d::application_core::HoldReason::Placed { hold } => w::HoldReason::Placed(
            w::HoldReasonPlaced::new(&w::CEILINGS, w::HoldReasonPlacedParts { hold: *hold })
                .expect("admitted fields fit schema"),
        ),
        d::application_core::HoldReason::Drift => w::HoldReason::Drift,
        d::application_core::HoldReason::Effects => w::HoldReason::Effects,
        d::application_core::HoldReason::EffectFailed => w::HoldReason::EffectFailed,
        d::application_core::HoldReason::Uncertain { entry } => w::HoldReason::Uncertain(
            w::HoldReasonUncertain::new(&w::CEILINGS, w::HoldReasonUncertainParts { entry: *entry })
                .expect("admitted fields fit schema"),
        ),
        d::application_core::HoldReason::Budget => w::HoldReason::Budget,
        d::application_core::HoldReason::Deadline => w::HoldReason::Deadline,
        d::application_core::HoldReason::HoldsWaited => w::HoldReason::HoldsWaited,
        d::application_core::HoldReason::ResourceUnavailable => w::HoldReason::ResourceUnavailable,
        d::application_core::HoldReason::ResourceAccess => w::HoldReason::ResourceAccess,
        d::application_core::HoldReason::PoolLost { pool } => w::HoldReason::PoolLost(
            w::HoldReasonPoolLost::new(&w::CEILINGS, w::HoldReasonPoolLostParts { pool: *pool })
                .expect("admitted fields fit schema"),
        ),
        d::application_core::HoldReason::Procedure => w::HoldReason::Procedure,
        d::application_core::HoldReason::Circle => w::HoldReason::Circle,
    }
}
fn decode_hold_reason(value: w::HoldReason) -> d::application_core::HoldReason {
    match value {
        w::HoldReason::Exceeded(value) => d::application_core::HoldReason::Exceeded(d::application_core::Exceeded {
            bound: match value.bound() {
                w::Bound::Reasoning => d::application_core::Bound::Reasoning,
                w::Bound::Metadata => d::application_core::Bound::Metadata,
                w::Bound::Items => d::application_core::Bound::Items,
                w::Bound::Answer => d::application_core::Bound::Answer,
                w::Bound::Tools => d::application_core::Bound::Tools,
                w::Bound::HistoryItems => d::application_core::Bound::HistoryItems,
                w::Bound::History => d::application_core::Bound::History,
                w::Bound::Window => d::application_core::Bound::Window,
                w::Bound::Output => d::application_core::Bound::Output,
                w::Bound::HostPart => d::application_core::Bound::HostPart,
            },
            value: value.value(),
        }),
        w::HoldReason::Failures(value) => {
            let w::HoldReasonFailuresParts { class } = value.into_parts();
            d::application_core::HoldReason::Failures { class: decode_failure_class(class) }
        }
        w::HoldReason::Stalled => d::application_core::HoldReason::Stalled,
        w::HoldReason::Dependency(value) => {
            let w::HoldReasonDependencyParts { task } = value.into_parts();
            d::application_core::HoldReason::Dependency { task }
        }
        w::HoldReason::Stopped => d::application_core::HoldReason::Stopped,
        w::HoldReason::Placed(value) => {
            let w::HoldReasonPlacedParts { hold } = value.into_parts();
            d::application_core::HoldReason::Placed { hold }
        }
        w::HoldReason::Drift => d::application_core::HoldReason::Drift,
        w::HoldReason::Effects => d::application_core::HoldReason::Effects,
        w::HoldReason::EffectFailed => d::application_core::HoldReason::EffectFailed,
        w::HoldReason::Uncertain(value) => {
            let w::HoldReasonUncertainParts { entry } = value.into_parts();
            d::application_core::HoldReason::Uncertain { entry }
        }
        w::HoldReason::Budget => d::application_core::HoldReason::Budget,
        w::HoldReason::Deadline => d::application_core::HoldReason::Deadline,
        w::HoldReason::HoldsWaited => d::application_core::HoldReason::HoldsWaited,
        w::HoldReason::ResourceUnavailable => d::application_core::HoldReason::ResourceUnavailable,
        w::HoldReason::ResourceAccess => d::application_core::HoldReason::ResourceAccess,
        w::HoldReason::PoolLost(value) => {
            let w::HoldReasonPoolLostParts { pool } = value.into_parts();
            d::application_core::HoldReason::PoolLost { pool }
        }
        w::HoldReason::Procedure => d::application_core::HoldReason::Procedure,
        w::HoldReason::Circle => d::application_core::HoldReason::Circle,
    }
}
fn encode_failure_class(value: d::application_core::FailureClass) -> w::FailureClass {
    match value {
        d::application_core::FailureClass::Transient => w::FailureClass::Transient,
        d::application_core::FailureClass::Permanent => w::FailureClass::Permanent,
        d::application_core::FailureClass::Run => w::FailureClass::Run,
        d::application_core::FailureClass::Agent => w::FailureClass::Agent,
        d::application_core::FailureClass::Lost => w::FailureClass::Lost,
        d::application_core::FailureClass::Invalid => w::FailureClass::Invalid,
    }
}
fn decode_failure_class(value: w::FailureClass) -> d::application_core::FailureClass {
    match value {
        w::FailureClass::Transient => d::application_core::FailureClass::Transient,
        w::FailureClass::Permanent => d::application_core::FailureClass::Permanent,
        w::FailureClass::Run => d::application_core::FailureClass::Run,
        w::FailureClass::Agent => d::application_core::FailureClass::Agent,
        w::FailureClass::Lost => d::application_core::FailureClass::Lost,
        w::FailureClass::Invalid => d::application_core::FailureClass::Invalid,
    }
}
