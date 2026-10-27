//! Durable people refusal facts (protocol/store.md, 5.1).
use super::{domain, wire};
use skein_lib::Wall;
pub(super) fn encode_again(value: domain::Again, _limits: &wire::Limits) -> wire::Again {
    match value {
        domain::Again::Busy => wire::Again::Busy,
        domain::Again::NotReady => wire::Again::NotReady,
    }
}
pub(super) fn decode_again(value: wire::Again) -> domain::Again {
    match value {
        wire::Again::Busy => domain::Again::Busy,
        wire::Again::NotReady => domain::Again::NotReady,
    }
}
pub(super) fn encode_failure_class(value: domain::FailureClass, _limits: &wire::Limits) -> wire::FailureClass {
    match value {
        domain::FailureClass::Transient => wire::FailureClass::Transient,
        domain::FailureClass::Permanent => wire::FailureClass::Permanent,
        domain::FailureClass::Run => wire::FailureClass::Run,
        domain::FailureClass::Agent => wire::FailureClass::Agent,
        domain::FailureClass::Lost => wire::FailureClass::Lost,
        domain::FailureClass::Invalid => wire::FailureClass::Invalid,
    }
}
pub(super) fn decode_failure_class(value: wire::FailureClass) -> domain::FailureClass {
    match value {
        wire::FailureClass::Transient => domain::FailureClass::Transient,
        wire::FailureClass::Permanent => domain::FailureClass::Permanent,
        wire::FailureClass::Run => domain::FailureClass::Run,
        wire::FailureClass::Agent => domain::FailureClass::Agent,
        wire::FailureClass::Lost => domain::FailureClass::Lost,
        wire::FailureClass::Invalid => domain::FailureClass::Invalid,
    }
}
pub(super) fn encode_hold_reason(
    value: &domain::HoldReason,
    limits: &wire::Limits,
) -> Result<wire::HoldReason, wire::Problem> {
    Ok(match value {
        domain::HoldReason::Exceeded(value) => wire::HoldReason::Exceeded(wire::Exceeded::new(
            limits,
            wire::ExceededParts {
                bound: match value.bound {
                    domain::Bound::Reasoning => wire::Bound::Reasoning,
                    domain::Bound::Metadata => wire::Bound::Metadata,
                    domain::Bound::Items => wire::Bound::Items,
                    domain::Bound::Answer => wire::Bound::Answer,
                    domain::Bound::Tools => wire::Bound::Tools,
                    domain::Bound::HistoryItems => wire::Bound::HistoryItems,
                    domain::Bound::History => wire::Bound::History,
                    domain::Bound::Window => wire::Bound::Window,
                    domain::Bound::Output => wire::Bound::Output,
                },
                value: value.value,
            },
        )?),
        domain::HoldReason::Failures { class } => wire::HoldReason::Failures(wire::HoldReasonFailures::new(
            limits,
            wire::HoldReasonFailuresParts { class: encode_failure_class(*class, limits) },
        )?),
        domain::HoldReason::Stalled => wire::HoldReason::Stalled,
        domain::HoldReason::Dependency { task } => wire::HoldReason::Dependency(wire::HoldReasonDependency::new(
            limits,
            wire::HoldReasonDependencyParts { task: *task },
        )?),
        domain::HoldReason::Stopped => wire::HoldReason::Stopped,
        domain::HoldReason::StoppedBy { party } => wire::HoldReason::StoppedBy(wire::HoldReasonStoppedBy::new(
            limits,
            wire::HoldReasonStoppedByParts { party: *party },
        )?),
        domain::HoldReason::Drift => wire::HoldReason::Drift,
        domain::HoldReason::Effects => wire::HoldReason::Effects,
        domain::HoldReason::EffectFailed => wire::HoldReason::EffectFailed,
        domain::HoldReason::Uncertain { entry } => wire::HoldReason::Uncertain(wire::HoldReasonUncertain::new(
            limits,
            wire::HoldReasonUncertainParts { entry: *entry },
        )?),
        domain::HoldReason::Budget => wire::HoldReason::Budget,
        domain::HoldReason::Deadline => wire::HoldReason::Deadline,
        domain::HoldReason::HoldsWaited => wire::HoldReason::HoldsWaited,
        domain::HoldReason::ResourceUnavailable => wire::HoldReason::ResourceUnavailable,
        domain::HoldReason::ResourceAccess => wire::HoldReason::ResourceAccess,
        domain::HoldReason::PoolLost { pool } => wire::HoldReason::PoolLost(wire::HoldReasonPoolLost::new(
            limits,
            wire::HoldReasonPoolLostParts { pool: *pool },
        )?),
        domain::HoldReason::Procedure => wire::HoldReason::Procedure,
        domain::HoldReason::Circle => wire::HoldReason::Circle,
    })
}
pub(super) fn decode_hold_reason(value: wire::HoldReason) -> domain::HoldReason {
    match value {
        wire::HoldReason::Exceeded(value) => domain::HoldReason::Exceeded(domain::Exceeded {
            bound: match value.bound() {
                wire::Bound::Reasoning => domain::Bound::Reasoning,
                wire::Bound::Metadata => domain::Bound::Metadata,
                wire::Bound::Items => domain::Bound::Items,
                wire::Bound::Answer => domain::Bound::Answer,
                wire::Bound::Tools => domain::Bound::Tools,
                wire::Bound::HistoryItems => domain::Bound::HistoryItems,
                wire::Bound::History => domain::Bound::History,
                wire::Bound::Window => domain::Bound::Window,
                wire::Bound::Output => domain::Bound::Output,
            },
            value: value.value(),
        }),
        wire::HoldReason::Failures(value) => {
            let wire::HoldReasonFailuresParts { class } = value.into_parts();
            domain::HoldReason::Failures { class: decode_failure_class(class) }
        }
        wire::HoldReason::Stalled => domain::HoldReason::Stalled,
        wire::HoldReason::Dependency(value) => {
            let wire::HoldReasonDependencyParts { task } = value.into_parts();
            domain::HoldReason::Dependency { task }
        }
        wire::HoldReason::Stopped => domain::HoldReason::Stopped,
        wire::HoldReason::StoppedBy(value) => {
            let wire::HoldReasonStoppedByParts { party } = value.into_parts();
            domain::HoldReason::StoppedBy { party }
        }
        wire::HoldReason::Drift => domain::HoldReason::Drift,
        wire::HoldReason::Effects => domain::HoldReason::Effects,
        wire::HoldReason::EffectFailed => domain::HoldReason::EffectFailed,
        wire::HoldReason::Uncertain(value) => {
            let wire::HoldReasonUncertainParts { entry } = value.into_parts();
            domain::HoldReason::Uncertain { entry }
        }
        wire::HoldReason::Budget => domain::HoldReason::Budget,
        wire::HoldReason::Deadline => domain::HoldReason::Deadline,
        wire::HoldReason::HoldsWaited => domain::HoldReason::HoldsWaited,
        wire::HoldReason::ResourceUnavailable => domain::HoldReason::ResourceUnavailable,
        wire::HoldReason::ResourceAccess => domain::HoldReason::ResourceAccess,
        wire::HoldReason::PoolLost(value) => {
            let wire::HoldReasonPoolLostParts { pool } = value.into_parts();
            domain::HoldReason::PoolLost { pool }
        }
        wire::HoldReason::Procedure => domain::HoldReason::Procedure,
        wire::HoldReason::Circle => domain::HoldReason::Circle,
    }
}
pub(super) fn encode_moved(value: &domain::Moved, limits: &wire::Limits) -> Result<wire::Moved, wire::Problem> {
    Ok(match value {
        domain::Moved::Hold { revision, reason } => wire::Moved::Hold(wire::MovedHold::new(
            limits,
            wire::MovedHoldParts { revision: *revision, reason: encode_hold_reason(reason, limits)? },
        )?),
        domain::Moved::Spec { revision } => {
            wire::Moved::Spec(wire::MovedSpec::new(limits, wire::MovedSpecParts { revision: *revision })?)
        }
        domain::Moved::Person { revision } => {
            wire::Moved::Person(wire::MovedPerson::new(limits, wire::MovedPersonParts { revision: *revision })?)
        }
        domain::Moved::Note { revision } => {
            wire::Moved::Note(wire::MovedNote::new(limits, wire::MovedNoteParts { revision: *revision })?)
        }
    })
}
pub(super) fn decode_moved(value: wire::Moved) -> domain::Moved {
    match value {
        wire::Moved::Hold(value) => {
            let wire::MovedHoldParts { revision, reason } = value.into_parts();
            domain::Moved::Hold { revision, reason: decode_hold_reason(reason) }
        }
        wire::Moved::Spec(value) => {
            let wire::MovedSpecParts { revision } = value.into_parts();
            domain::Moved::Spec { revision }
        }
        wire::Moved::Person(value) => {
            let wire::MovedPersonParts { revision } = value.into_parts();
            domain::Moved::Person { revision }
        }
        wire::Moved::Note(value) => {
            let wire::MovedNoteParts { revision } = value.into_parts();
            domain::Moved::Note { revision }
        }
    }
}
pub(super) fn encode_party(value: &domain::Party, limits: &wire::Limits) -> Result<wire::Party, wire::Problem> {
    Ok(match value {
        domain::Party::Person(value) => {
            wire::Party::Person(wire::PartyPerson::new(limits, wire::PartyPersonParts { value: *value })?)
        }
        domain::Party::Task(value) => {
            wire::Party::Task(wire::PartyTask::new(limits, wire::PartyTaskParts { value: *value })?)
        }
        domain::Party::Deployment { project } => wire::Party::Deployment(wire::PartyDeployment::new(
            limits,
            wire::PartyDeploymentParts { project: *project },
        )?),
    })
}
pub(super) fn decode_party(value: wire::Party) -> domain::Party {
    match value {
        wire::Party::Person(value) => {
            let wire::PartyPersonParts { value } = value.into_parts();
            domain::Party::Person(value)
        }
        wire::Party::Task(value) => {
            let wire::PartyTaskParts { value } = value.into_parts();
            domain::Party::Task(value)
        }
        wire::Party::Deployment(value) => {
            let wire::PartyDeploymentParts { project } = value.into_parts();
            domain::Party::Deployment { project }
        }
    }
}
pub(super) fn encode_ending(value: &domain::Ending, limits: &wire::Limits) -> Result<wire::Ending, wire::Problem> {
    Ok(match value {
        domain::Ending::Done => wire::Ending::Done,
        domain::Ending::Failed => wire::Ending::Failed,
        domain::Ending::Cancelled { by, at } => wire::Ending::Cancelled(wire::EndingCancelled::new(
            limits,
            wire::EndingCancelledParts { by: encode_party(by, limits)?, at: at.as_nanos() },
        )?),
        domain::Ending::Decided { by, at } => wire::Ending::Decided(wire::EndingDecided::new(
            limits,
            wire::EndingDecidedParts { by: *by, at: at.as_nanos() },
        )?),
        domain::Ending::Deleted => wire::Ending::Deleted,
    })
}
pub(super) fn decode_ending(value: wire::Ending) -> domain::Ending {
    match value {
        wire::Ending::Done => domain::Ending::Done,
        wire::Ending::Failed => domain::Ending::Failed,
        wire::Ending::Cancelled(value) => {
            let wire::EndingCancelledParts { by, at } = value.into_parts();
            domain::Ending::Cancelled { by: decode_party(by), at: Wall::from_nanos(at) }
        }
        wire::Ending::Decided(value) => {
            let wire::EndingDecidedParts { by, at } = value.into_parts();
            domain::Ending::Decided { by, at: Wall::from_nanos(at) }
        }
        wire::Ending::Deleted => domain::Ending::Deleted,
    }
}
pub(super) fn encode_lack(value: &domain::Lack, limits: &wire::Limits) -> Result<wire::Lack, wire::Problem> {
    Ok(match value {
        domain::Lack::Approver => wire::Lack::Approver,
        domain::Lack::Budget { needs, left } => {
            wire::Lack::Budget(wire::LackBudget::new(limits, wire::LackBudgetParts { needs: *needs, left: *left })?)
        }
        domain::Lack::Grant { kind } => {
            wire::Lack::Grant(wire::LackGrant::new(limits, wire::LackGrantParts { kind: *kind })?)
        }
        domain::Lack::Funding => wire::Lack::Funding,
        domain::Lack::TaskKind { executor } => wire::Lack::TaskKind(wire::LackTaskKind::new(
            limits,
            wire::LackTaskKindParts {
                executor: match executor {
                    Some(executor) => Some(super::super::batch::parts::encode_executor(*executor, limits)?),
                    None => None,
                },
            },
        )?),
        domain::Lack::Depth { most } => {
            wire::Lack::Depth(wire::LackDepth::new(limits, wire::LackDepthParts { most: *most })?)
        }
        domain::Lack::Deadline => wire::Lack::Deadline,
        domain::Lack::Tasks { left } => {
            wire::Lack::Tasks(wire::LackTasks::new(limits, wire::LackTasksParts { left: *left })?)
        }
        domain::Lack::Tool => wire::Lack::Tool,
        domain::Lack::Scope => wire::Lack::Scope,
        domain::Lack::Models => wire::Lack::Models,
        domain::Lack::Reference => wire::Lack::Reference,
        domain::Lack::ResourceAccess => wire::Lack::ResourceAccess,
        domain::Lack::ResourceUnavailable => wire::Lack::ResourceUnavailable,
        domain::Lack::Shape => wire::Lack::Shape,
        domain::Lack::Again { required } => {
            wire::Lack::Again(wire::LackAgain::new(limits, wire::LackAgainParts { required: *required })?)
        }
        domain::Lack::Account => wire::Lack::Account,
        domain::Lack::Writer => wire::Lack::Writer,
        domain::Lack::Required { connector, kind } => wire::Lack::Required(wire::LackRequired::new(
            limits,
            wire::LackRequiredParts { connector: *connector, kind: *kind },
        )?),
        domain::Lack::Failed { connector, kind } => wire::Lack::Failed(wire::LackFailed::new(
            limits,
            wire::LackFailedParts { connector: *connector, kind: *kind },
        )?),
        domain::Lack::Unguarded { connector, kind } => wire::Lack::Unguarded(wire::LackUnguarded::new(
            limits,
            wire::LackUnguardedParts { connector: *connector, kind: *kind },
        )?),
    })
}
pub(super) fn decode_lack(value: wire::Lack) -> domain::Lack {
    match value {
        wire::Lack::Approver => domain::Lack::Approver,
        wire::Lack::Budget(value) => {
            let wire::LackBudgetParts { needs, left } = value.into_parts();
            domain::Lack::Budget { needs, left }
        }
        wire::Lack::Grant(value) => {
            let wire::LackGrantParts { kind } = value.into_parts();
            domain::Lack::Grant { kind }
        }
        wire::Lack::Funding => domain::Lack::Funding,
        wire::Lack::TaskKind(value) => {
            let mut executor = None;
            if let Some(value) = value.into_parts().executor {
                executor = Some(super::super::batch::parts::decode_executor(value));
            }
            domain::Lack::TaskKind { executor }
        }
        wire::Lack::Depth(value) => {
            let wire::LackDepthParts { most } = value.into_parts();
            domain::Lack::Depth { most }
        }
        wire::Lack::Deadline => domain::Lack::Deadline,
        wire::Lack::Tasks(value) => {
            let wire::LackTasksParts { left } = value.into_parts();
            domain::Lack::Tasks { left }
        }
        wire::Lack::Tool => domain::Lack::Tool,
        wire::Lack::Scope => domain::Lack::Scope,
        wire::Lack::Models => domain::Lack::Models,
        wire::Lack::Reference => domain::Lack::Reference,
        wire::Lack::ResourceAccess => domain::Lack::ResourceAccess,
        wire::Lack::ResourceUnavailable => domain::Lack::ResourceUnavailable,
        wire::Lack::Shape => domain::Lack::Shape,
        wire::Lack::Again(value) => {
            let wire::LackAgainParts { required } = value.into_parts();
            domain::Lack::Again { required }
        }
        wire::Lack::Account => domain::Lack::Account,
        wire::Lack::Writer => domain::Lack::Writer,
        wire::Lack::Required(value) => {
            let wire::LackRequiredParts { connector, kind } = value.into_parts();
            domain::Lack::Required { connector, kind }
        }
        wire::Lack::Failed(value) => {
            let wire::LackFailedParts { connector, kind } = value.into_parts();
            domain::Lack::Failed { connector, kind }
        }
        wire::Lack::Unguarded(value) => {
            let wire::LackUnguardedParts { connector, kind } = value.into_parts();
            domain::Lack::Unguarded { connector, kind }
        }
    }
}
pub(super) fn encode_limit_kind(value: domain::LimitKind, _limits: &wire::Limits) -> wire::LimitKind {
    match value {
        domain::LimitKind::Projects => wire::LimitKind::Projects,
        domain::LimitKind::Words => wire::LimitKind::Words,
        domain::LimitKind::Identity => wire::LimitKind::Identity,
        domain::LimitKind::Roles => wire::LimitKind::Roles,
        domain::LimitKind::Arithmetic => wire::LimitKind::Arithmetic,
        domain::LimitKind::Duplicate => wire::LimitKind::Duplicate,
        domain::LimitKind::Empty => wire::LimitKind::Empty,
        domain::LimitKind::Batch => wire::LimitKind::Batch,
        domain::LimitKind::ProjectLiveTasks => wire::LimitKind::ProjectLiveTasks,
        domain::LimitKind::LiveTasks => wire::LimitKind::LiveTasks,
        domain::LimitKind::Tree => wire::LimitKind::Tree,
        domain::LimitKind::Depth => wire::LimitKind::Depth,
        domain::LimitKind::Delegates => wire::LimitKind::Delegates,
        domain::LimitKind::Subscription => wire::LimitKind::Subscription,
        domain::LimitKind::Dependencies => wire::LimitKind::Dependencies,
        domain::LimitKind::Cycle => wire::LimitKind::Cycle,
        domain::LimitKind::Spec => wire::LimitKind::Spec,
        domain::LimitKind::Contract => wire::LimitKind::Contract,
        domain::LimitKind::Authority => wire::LimitKind::Authority,
        domain::LimitKind::Inputs => wire::LimitKind::Inputs,
        domain::LimitKind::Attempts => wire::LimitKind::Attempts,
        domain::LimitKind::LiveDelegates => wire::LimitKind::LiveDelegates,
        domain::LimitKind::Restore => wire::LimitKind::Restore,
        domain::LimitKind::Read => wire::LimitKind::Read,
        domain::LimitKind::Turn => wire::LimitKind::Turn,
        domain::LimitKind::HoldKind => wire::LimitKind::HoldKind,
        domain::LimitKind::HoldTaken => wire::LimitKind::HoldTaken,
        domain::LimitKind::Holds => wire::LimitKind::Holds,
        domain::LimitKind::InboxFull => wire::LimitKind::InboxFull,
        domain::LimitKind::Notes => wire::LimitKind::Notes,
        domain::LimitKind::Revision => wire::LimitKind::Revision,
        domain::LimitKind::Question => wire::LimitKind::Question,
        domain::LimitKind::Priority => wire::LimitKind::Priority,
        domain::LimitKind::Policy => wire::LimitKind::Policy,
        domain::LimitKind::Resource => wire::LimitKind::Resource,
        domain::LimitKind::Service => wire::LimitKind::Service,
        domain::LimitKind::Accounts => wire::LimitKind::Accounts,
        domain::LimitKind::HolderAccounts => wire::LimitKind::HolderAccounts,
        domain::LimitKind::Sources => wire::LimitKind::Sources,
        domain::LimitKind::SettingsProposals => wire::LimitKind::SettingsProposals,
        domain::LimitKind::SignIns => wire::LimitKind::SignIns,
        domain::LimitKind::Operations => wire::LimitKind::Operations,
        domain::LimitKind::Grace => wire::LimitKind::Grace,
        domain::LimitKind::Goals => wire::LimitKind::Goals,
    }
}
pub(super) fn decode_limit_kind(value: wire::LimitKind) -> domain::LimitKind {
    match value {
        wire::LimitKind::Projects => domain::LimitKind::Projects,
        wire::LimitKind::Words => domain::LimitKind::Words,
        wire::LimitKind::Identity => domain::LimitKind::Identity,
        wire::LimitKind::Roles => domain::LimitKind::Roles,
        wire::LimitKind::Arithmetic => domain::LimitKind::Arithmetic,
        wire::LimitKind::Duplicate => domain::LimitKind::Duplicate,
        wire::LimitKind::Empty => domain::LimitKind::Empty,
        wire::LimitKind::Batch => domain::LimitKind::Batch,
        wire::LimitKind::ProjectLiveTasks => domain::LimitKind::ProjectLiveTasks,
        wire::LimitKind::LiveTasks => domain::LimitKind::LiveTasks,
        wire::LimitKind::Tree => domain::LimitKind::Tree,
        wire::LimitKind::Depth => domain::LimitKind::Depth,
        wire::LimitKind::Delegates => domain::LimitKind::Delegates,
        wire::LimitKind::Subscription => domain::LimitKind::Subscription,
        wire::LimitKind::Dependencies => domain::LimitKind::Dependencies,
        wire::LimitKind::Cycle => domain::LimitKind::Cycle,
        wire::LimitKind::Spec => domain::LimitKind::Spec,
        wire::LimitKind::Contract => domain::LimitKind::Contract,
        wire::LimitKind::Authority => domain::LimitKind::Authority,
        wire::LimitKind::Inputs => domain::LimitKind::Inputs,
        wire::LimitKind::Attempts => domain::LimitKind::Attempts,
        wire::LimitKind::LiveDelegates => domain::LimitKind::LiveDelegates,
        wire::LimitKind::Restore => domain::LimitKind::Restore,
        wire::LimitKind::Read => domain::LimitKind::Read,
        wire::LimitKind::Turn => domain::LimitKind::Turn,
        wire::LimitKind::HoldKind => domain::LimitKind::HoldKind,
        wire::LimitKind::HoldTaken => domain::LimitKind::HoldTaken,
        wire::LimitKind::Holds => domain::LimitKind::Holds,
        wire::LimitKind::InboxFull => domain::LimitKind::InboxFull,
        wire::LimitKind::Notes => domain::LimitKind::Notes,
        wire::LimitKind::Revision => domain::LimitKind::Revision,
        wire::LimitKind::Question => domain::LimitKind::Question,
        wire::LimitKind::Priority => domain::LimitKind::Priority,
        wire::LimitKind::Policy => domain::LimitKind::Policy,
        wire::LimitKind::Resource => domain::LimitKind::Resource,
        wire::LimitKind::Service => domain::LimitKind::Service,
        wire::LimitKind::Accounts => domain::LimitKind::Accounts,
        wire::LimitKind::HolderAccounts => domain::LimitKind::HolderAccounts,
        wire::LimitKind::Sources => domain::LimitKind::Sources,
        wire::LimitKind::SettingsProposals => domain::LimitKind::SettingsProposals,
        wire::LimitKind::SignIns => domain::LimitKind::SignIns,
        wire::LimitKind::Operations => domain::LimitKind::Operations,
        wire::LimitKind::Grace => domain::LimitKind::Grace,
        wire::LimitKind::Goals => domain::LimitKind::Goals,
    }
}
#[expect(clippy::too_many_lines, reason = "one exhaustive translation preserves every stored refusal variant")]
pub(super) fn encode_refusal(value: &domain::Refusal, limits: &wire::Limits) -> Result<wire::Refusal, wire::Problem> {
    Ok(match value {
        domain::Refusal::Forgotten { oldest } => wire::Refusal::Forgotten(wire::RefusalForgotten::new(
            limits,
            wire::RefusalForgottenParts { oldest: *oldest },
        )?),
        domain::Refusal::Stale { newest } => {
            wire::Refusal::Stale(wire::RefusalStale::new(limits, wire::RefusalStaleParts { newest: *newest })?)
        }
        domain::Refusal::Full { revision, tasks, more } => wire::Refusal::Full(wire::RefusalFull::new(
            limits,
            wire::RefusalFullParts {
                revision: *revision,
                tasks: names(tasks, limits.refusal_full_tasks, wire::Path::RefusalFullTasks)?,
                more: *more,
            },
        )?),
        domain::Refusal::NotWellFormed { at, why } => wire::Refusal::NotWellFormed(wire::RefusalNotWellFormed::new(
            limits,
            wire::RefusalNotWellFormedParts {
                at: *at,
                why: match why {
                    domain::Malformed::Settings { setting, fault } => {
                        wire::Malformed::Settings(wire::MalformedSettings::new(
                            limits,
                            wire::MalformedSettingsParts {
                                setting: encode_setting(*setting, limits)?,
                                fault: encode_setting_fault(*fault),
                            },
                        )?)
                    }
                    domain::Malformed::SettingsUnchanged => wire::Malformed::SettingsUnchanged,
                    domain::Malformed::Running => wire::Malformed::Running,
                    domain::Malformed::ProposedAsAllowed => wire::Malformed::ProposedAsAllowed,
                    domain::Malformed::Cycle => wire::Malformed::Cycle,
                    domain::Malformed::Name(rule) => wire::Malformed::Name(wire::MalformedName::new(
                        limits,
                        wire::MalformedNameParts { rule: encode_name_rule(*rule) },
                    )?),
                    domain::Malformed::AsAllowedTwice => wire::Malformed::AsAllowedTwice,
                    domain::Malformed::Conversation => wire::Malformed::Conversation,

                    domain::Malformed::EndedBy => wire::Malformed::EndedBy,
                },
            },
        )?),
        domain::Refusal::Role { held } => wire::Refusal::Role(wire::RefusalRole::new(
            limits,
            wire::RefusalRoleParts {
                held: match held {
                    Some(value) => Some(super::super::encode_role(*value, limits)?),
                    None => None,
                },
            },
        )?),
        domain::Refusal::Authority { lacking, proposable } => wire::Refusal::Authority(wire::RefusalAuthority::new(
            limits,
            wire::RefusalAuthorityParts {
                lacking: encode_lack(lacking, limits)?,
                proposable: match proposable {
                    None => None,
                    Some(domain::Accepter::Nobody) => Some(wire::Accepter::Nobody),
                    Some(domain::Accepter::Role(role)) => Some(wire::Accepter::Role(wire::AccepterRole::new(
                        limits,
                        wire::AccepterRoleParts { role: *role },
                    )?)),
                },
            },
        )?),
        domain::Refusal::Limit { which } => wire::Refusal::Limit(wire::RefusalLimit::new(
            limits,
            wire::RefusalLimitParts { which: encode_limit_kind(*which, limits) },
        )?),
        domain::Refusal::Moved { now } => wire::Refusal::Moved(wire::RefusalMoved::new(
            limits,
            wire::RefusalMovedParts { now: encode_moved(now, limits)? },
        )?),
        domain::Refusal::Ended { how } => wire::Refusal::Ended(wire::RefusalEnded::new(
            limits,
            wire::RefusalEndedParts { how: encode_ending(how, limits)? },
        )?),
        domain::Refusal::Taken { by } => {
            wire::Refusal::Taken(wire::RefusalTaken::new(limits, wire::RefusalTakenParts { by: *by })?)
        }
        domain::Refusal::Circle { tasks, cut } => wire::Refusal::Circle(wire::RefusalCircle::new(
            limits,
            wire::RefusalCircleParts {
                tasks: names(tasks, limits.refusal_circle_tasks, wire::Path::RefusalCircleTasks)?,
                cut: *cut,
            },
        )?),
        domain::Refusal::Kept { pending } => wire::Refusal::Kept(wire::RefusalKept::new(
            limits,
            wire::RefusalKeptParts {
                pending: names(pending, limits.refusal_kept_pending, wire::Path::RefusalKeptPending)?,
            },
        )?),
        domain::Refusal::Dependency { dependency, how } => wire::Refusal::Dependency(wire::RefusalDependency::new(
            limits,
            wire::RefusalDependencyParts {
                dependency: *dependency,
                how: match how {
                    domain::DependencyStatus::Done => wire::DependencyStatus::Done,
                    domain::DependencyStatus::Failed => wire::DependencyStatus::Failed,
                    domain::DependencyStatus::Cancelled => wire::DependencyStatus::Cancelled,
                },
            },
        )?),
        domain::Refusal::NotWaiting => wire::Refusal::NotWaiting,
        domain::Refusal::NoFurther => wire::Refusal::NoFurther,
        domain::Refusal::SignInRedirect { why } => wire::Refusal::SignInRedirect(wire::RefusalSignInRedirect::new(
            limits,
            wire::RefusalSignInRedirectParts {
                why: match why {
                    domain::SignInRedirectWhy::NotWaiting => wire::SignInRedirectWhy::NotWaiting,
                    domain::SignInRedirectWhy::Address => wire::SignInRedirectWhy::Address,
                    domain::SignInRedirectWhy::State => wire::SignInRedirectWhy::State,
                    domain::SignInRedirectWhy::TooMany => wire::SignInRedirectWhy::TooMany,
                },
            },
        )?),
        domain::Refusal::AccountMove { why } => wire::Refusal::AccountMove(wire::RefusalAccountMove::new(
            limits,
            wire::RefusalAccountMoveParts { why: encode_account_move_why(*why) },
        )?),
        domain::Refusal::NameTaken => wire::Refusal::NameTaken,
        domain::Refusal::Unknown => wire::Refusal::Unknown,
        domain::Refusal::Provider => wire::Refusal::Provider,
        domain::Refusal::NotOffered => wire::Refusal::NotOffered,
        domain::Refusal::KeyConflict => wire::Refusal::KeyConflict,
        domain::Refusal::NoEscalationHolder => wire::Refusal::NoEscalationHolder,
        domain::Refusal::Account { account, why } => wire::Refusal::Account(wire::RefusalAccount::new(
            limits,
            wire::RefusalAccountParts { account: *account, why: encode_account_why(*why) },
        )?),
        domain::Refusal::Llm { model, why } => wire::Refusal::Llm(wire::RefusalLlm::new(
            limits,
            wire::RefusalLlmParts { model: *model, why: super::super::llm::encode_why(*why) },
        )?),
    })
}
#[expect(clippy::too_many_lines, reason = "one exhaustive translation preserves every stored refusal variant")]
pub(super) fn decode_refusal(value: wire::Refusal) -> domain::Refusal {
    match value {
        wire::Refusal::Forgotten(value) => domain::Refusal::Forgotten { oldest: value.oldest() },
        wire::Refusal::Stale(value) => domain::Refusal::Stale { newest: value.newest() },
        wire::Refusal::Full(value) => {
            let v = value.into_parts();
            domain::Refusal::Full { revision: v.revision, tasks: v.tasks.into_boxed(), more: v.more }
        }
        wire::Refusal::NotWellFormed(value) => {
            let wire::RefusalNotWellFormedParts { at, why } = value.into_parts();
            domain::Refusal::NotWellFormed {
                at,
                why: match why {
                    wire::Malformed::Settings(value) => domain::Malformed::Settings {
                        setting: decode_setting(value.setting()),
                        fault: decode_setting_fault(value.fault()),
                    },
                    wire::Malformed::SettingsUnchanged => domain::Malformed::SettingsUnchanged,
                    wire::Malformed::Running => domain::Malformed::Running,
                    wire::Malformed::ProposedAsAllowed => domain::Malformed::ProposedAsAllowed,
                    wire::Malformed::Cycle => domain::Malformed::Cycle,
                    wire::Malformed::Name(value) => domain::Malformed::Name(decode_name_rule(value.rule().clone())),
                    wire::Malformed::AsAllowedTwice => domain::Malformed::AsAllowedTwice,
                    wire::Malformed::Conversation => domain::Malformed::Conversation,

                    wire::Malformed::EndedBy => domain::Malformed::EndedBy,
                },
            }
        }
        wire::Refusal::Role(value) => {
            let wire::RefusalRoleParts { held } = value.into_parts();
            domain::Refusal::Role {
                held: match held {
                    Some(value) => {
                        let result = super::super::decode_role(value);
                        Some(result)
                    }
                    None => None,
                },
            }
        }
        wire::Refusal::Authority(value) => {
            let wire::RefusalAuthorityParts { lacking, proposable } = value.into_parts();
            domain::Refusal::Authority {
                lacking: decode_lack(lacking),
                proposable: match proposable {
                    None => None,
                    Some(wire::Accepter::Nobody) => Some(domain::Accepter::Nobody),
                    Some(wire::Accepter::Role(role)) => Some(domain::Accepter::Role(role.into_parts().role)),
                },
            }
        }
        wire::Refusal::Limit(value) => {
            let wire::RefusalLimitParts { which } = value.into_parts();
            domain::Refusal::Limit { which: decode_limit_kind(which) }
        }
        wire::Refusal::Moved(value) => {
            let wire::RefusalMovedParts { now } = value.into_parts();
            domain::Refusal::Moved { now: decode_moved(now) }
        }
        wire::Refusal::Ended(value) => {
            let wire::RefusalEndedParts { how } = value.into_parts();
            domain::Refusal::Ended { how: decode_ending(how) }
        }
        wire::Refusal::Taken(value) => {
            let wire::RefusalTakenParts { by } = value.into_parts();
            domain::Refusal::Taken { by }
        }
        wire::Refusal::Circle(value) => {
            let wire::RefusalCircleParts { tasks, cut } = value.into_parts();
            domain::Refusal::Circle { tasks: tasks.into_boxed(), cut }
        }
        wire::Refusal::Kept(value) => domain::Refusal::Kept { pending: value.into_parts().pending.into_boxed() },
        wire::Refusal::Dependency(value) => {
            let wire::RefusalDependencyParts { dependency, how } = value.into_parts();
            domain::Refusal::Dependency {
                dependency,
                how: match how {
                    wire::DependencyStatus::Done => domain::DependencyStatus::Done,
                    wire::DependencyStatus::Failed => domain::DependencyStatus::Failed,
                    wire::DependencyStatus::Cancelled => domain::DependencyStatus::Cancelled,
                },
            }
        }
        wire::Refusal::NotWaiting => domain::Refusal::NotWaiting,
        wire::Refusal::NoFurther => domain::Refusal::NoFurther,
        wire::Refusal::SignInRedirect(row) => {
            domain::Refusal::SignInRedirect { why: decode_sign_in_redirect_why(row.why()) }
        }
        wire::Refusal::AccountMove(value) => {
            domain::Refusal::AccountMove { why: decode_account_move_why(value.into_parts().why) }
        }
        wire::Refusal::NameTaken => domain::Refusal::NameTaken,
        wire::Refusal::Unknown => domain::Refusal::Unknown,
        wire::Refusal::Provider => domain::Refusal::Provider,
        wire::Refusal::NotOffered => domain::Refusal::NotOffered,
        wire::Refusal::KeyConflict => domain::Refusal::KeyConflict,
        wire::Refusal::NoEscalationHolder => domain::Refusal::NoEscalationHolder,
        wire::Refusal::Account(value) => {
            let wire::RefusalAccountParts { account, why } = value.into_parts();
            domain::Refusal::Account { account, why: decode_account_why(why) }
        }
        wire::Refusal::Llm(value) => {
            let wire::RefusalLlmParts { model, why } = value.into_parts();
            domain::Refusal::Llm { model, why: super::super::llm::decode_why(why) }
        }
    }
}
pub(super) fn encode_proposal_choice(value: domain::ProposalChoice, _limits: &wire::Limits) -> wire::ProposalChoice {
    match value {
        domain::ProposalChoice::Accepted => wire::ProposalChoice::Accepted,
        domain::ProposalChoice::Rejected => wire::ProposalChoice::Rejected,
        domain::ProposalChoice::Passed => wire::ProposalChoice::Passed,
        domain::ProposalChoice::Withdrawn => wire::ProposalChoice::Withdrawn,
        domain::ProposalChoice::Stale => wire::ProposalChoice::Stale,
    }
}
pub(super) fn decode_proposal_choice(value: wire::ProposalChoice) -> domain::ProposalChoice {
    match value {
        wire::ProposalChoice::Accepted => domain::ProposalChoice::Accepted,
        wire::ProposalChoice::Rejected => domain::ProposalChoice::Rejected,
        wire::ProposalChoice::Passed => domain::ProposalChoice::Passed,
        wire::ProposalChoice::Withdrawn => domain::ProposalChoice::Withdrawn,
        wire::ProposalChoice::Stale => domain::ProposalChoice::Stale,
    }
}
pub(super) fn encode_escalation_choice(
    value: domain::EscalationChoice,
    _limits: &wire::Limits,
) -> wire::EscalationChoice {
    match value {
        domain::EscalationChoice::Released => wire::EscalationChoice::Released,
        domain::EscalationChoice::Rejected => wire::EscalationChoice::Rejected,
        domain::EscalationChoice::Passed => wire::EscalationChoice::Passed,
    }
}
pub(super) fn decode_escalation_choice(value: wire::EscalationChoice) -> domain::EscalationChoice {
    match value {
        wire::EscalationChoice::Released => domain::EscalationChoice::Released,
        wire::EscalationChoice::Rejected => domain::EscalationChoice::Rejected,
        wire::EscalationChoice::Passed => domain::EscalationChoice::Passed,
    }
}

fn encode_account_why(value: domain::AccountWhy) -> wire::AccountWhy {
    match value {
        domain::AccountWhy::Unknown => wire::AccountWhy::Unknown,
        domain::AccountWhy::Provider => wire::AccountWhy::Provider,
        domain::AccountWhy::Uses => wire::AccountWhy::Uses,
        domain::AccountWhy::Limited => wire::AccountWhy::Limited,
    }
}

fn decode_account_why(value: wire::AccountWhy) -> domain::AccountWhy {
    match value {
        wire::AccountWhy::Unknown => domain::AccountWhy::Unknown,
        wire::AccountWhy::Provider => domain::AccountWhy::Provider,
        wire::AccountWhy::Uses => domain::AccountWhy::Uses,
        wire::AccountWhy::Limited => domain::AccountWhy::Limited,
    }
}

fn names(values: &[u64], bound: u32, path: wire::Path) -> Result<skein_lib::List<u64>, wire::Problem> {
    let mut rows = skein_lib::List::with_capacity(bound);
    for number in values {
        if rows.push(*number).is_err() {
            return Err(wire::Problem { path, reason: skein_codec::Reason::Bound });
        }
    }
    Ok(rows)
}

fn encode_name_rule(rule: domain::NameRule) -> wire::NameRule {
    match rule {
        domain::NameRule::Empty => wire::NameRule::Empty,
        domain::NameRule::Long => wire::NameRule::Long,
        domain::NameRule::Control => wire::NameRule::Control,
        domain::NameRule::Space => wire::NameRule::Space,
        domain::NameRule::Digits => wire::NameRule::Digits,
    }
}

fn decode_name_rule(rule: wire::NameRule) -> domain::NameRule {
    match rule {
        wire::NameRule::Empty => domain::NameRule::Empty,
        wire::NameRule::Long => domain::NameRule::Long,
        wire::NameRule::Control => domain::NameRule::Control,
        wire::NameRule::Space => domain::NameRule::Space,
        wire::NameRule::Digits => domain::NameRule::Digits,
    }
}

const fn encode_account_move_why(value: domain::AccountMoveWhy) -> wire::AccountMoveWhy {
    match value {
        domain::AccountMoveWhy::NotSignedIn => wire::AccountMoveWhy::NotSignedIn,
        domain::AccountMoveWhy::NotOurs => wire::AccountMoveWhy::NotOurs,
        domain::AccountMoveWhy::Held => wire::AccountMoveWhy::Held,
        domain::AccountMoveWhy::Cancelled => wire::AccountMoveWhy::Cancelled,
        domain::AccountMoveWhy::Moving => wire::AccountMoveWhy::Moving,
    }
}

const fn decode_account_move_why(value: wire::AccountMoveWhy) -> domain::AccountMoveWhy {
    match value {
        wire::AccountMoveWhy::NotSignedIn => domain::AccountMoveWhy::NotSignedIn,
        wire::AccountMoveWhy::NotOurs => domain::AccountMoveWhy::NotOurs,
        wire::AccountMoveWhy::Held => domain::AccountMoveWhy::Held,
        wire::AccountMoveWhy::Cancelled => domain::AccountMoveWhy::Cancelled,
        wire::AccountMoveWhy::Moving => domain::AccountMoveWhy::Moving,
    }
}

fn encode_setting(value: domain::Setting, limits: &wire::Limits) -> Result<wire::Setting, wire::Problem> {
    Ok(match value {
        domain::Setting::Charter(number) => {
            wire::Setting::Charter(wire::SettingNumber::new(limits, wire::SettingNumberParts { number })?)
        }
        domain::Setting::Template(number) => {
            wire::Setting::Template(wire::SettingNumber::new(limits, wire::SettingNumberParts { number })?)
        }
        domain::Setting::Policy(number) => {
            wire::Setting::Policy(wire::SettingNumber::new(limits, wire::SettingNumberParts { number })?)
        }
        domain::Setting::Rules => wire::Setting::Rules,
    })
}
fn encode_setting_fault(value: domain::SettingFault) -> wire::SettingFault {
    match value {
        domain::SettingFault::Shape => wire::SettingFault::Shape,
        domain::SettingFault::Count => wire::SettingFault::Count,
        domain::SettingFault::Reference => wire::SettingFault::Reference,
        domain::SettingFault::Rules => wire::SettingFault::Rules,
        domain::SettingFault::Budget => wire::SettingFault::Budget,
        domain::SettingFault::Grace => wire::SettingFault::Grace,
        domain::SettingFault::Owner => wire::SettingFault::Owner,
    }
}

fn decode_setting(value: &wire::Setting) -> domain::Setting {
    match value {
        wire::Setting::Charter(value) => domain::Setting::Charter(value.number()),
        wire::Setting::Template(value) => domain::Setting::Template(value.number()),
        wire::Setting::Policy(value) => domain::Setting::Policy(value.number()),
        wire::Setting::Rules => domain::Setting::Rules,
    }
}
fn decode_setting_fault(value: &wire::SettingFault) -> domain::SettingFault {
    match value {
        wire::SettingFault::Shape => domain::SettingFault::Shape,
        wire::SettingFault::Count => domain::SettingFault::Count,
        wire::SettingFault::Reference => domain::SettingFault::Reference,
        wire::SettingFault::Rules => domain::SettingFault::Rules,
        wire::SettingFault::Budget => domain::SettingFault::Budget,
        wire::SettingFault::Grace => domain::SettingFault::Grace,
        wire::SettingFault::Owner => domain::SettingFault::Owner,
    }
}

const fn decode_sign_in_redirect_why(value: &wire::SignInRedirectWhy) -> domain::SignInRedirectWhy {
    match value {
        wire::SignInRedirectWhy::NotWaiting => domain::SignInRedirectWhy::NotWaiting,
        wire::SignInRedirectWhy::Address => domain::SignInRedirectWhy::Address,
        wire::SignInRedirectWhy::State => domain::SignInRedirectWhy::State,
        wire::SignInRedirectWhy::TooMany => domain::SignInRedirectWhy::TooMany,
    }
}
