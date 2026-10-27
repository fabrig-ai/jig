//! Values carried by core rows in their own family (protocol/store.md, 5.1).
use crate::tasks_v9 as wire;
use alloc::boxed::Box;
use jig_domain_tasks as tasks;
use skein_lib::{List, Reader, Writer};
type Limits = wire::Limits;
pub(super) fn encode_capacity_action(
    value: &tasks::CapacityAction,
    limits: &Limits,
) -> Result<wire::CapacityAction, wire::Problem> {
    Ok(match value {
        tasks::CapacityAction::Batch { first, members } => wire::CapacityAction::Batch(wire::CapacityActionBatch::new(
            limits,
            wire::CapacityActionBatchParts { first: *first, members: *members },
        )?),
        tasks::CapacityAction::Proposal { number } => wire::CapacityAction::Proposal(
            wire::CapacityActionProposal::new(limits, wire::CapacityActionProposalParts { number: *number })?,
        ),
        tasks::CapacityAction::PersonProposal { number } => {
            wire::CapacityAction::PersonProposal(wire::CapacityActionPersonProposal::new(
                limits,
                wire::CapacityActionPersonProposalParts { number: *number },
            )?)
        }
        tasks::CapacityAction::Message { task, number } => {
            wire::CapacityAction::Message(wire::CapacityActionMessage::new(
                limits,
                wire::CapacityActionMessageParts { task: *task, number: *number },
            )?)
        }
        tasks::CapacityAction::Source { funder } => wire::CapacityAction::Source(wire::CapacityActionSource::new(
            limits,
            wire::CapacityActionSourceParts { funder: super::value::funder(*funder, limits)? },
        )?),
        tasks::CapacityAction::Introduce { left, right } => {
            wire::CapacityAction::Introduce(wire::CapacityActionIntroduce::new(
                limits,
                wire::CapacityActionIntroduceParts { left: *left, right: *right },
            )?)
        }
        tasks::CapacityAction::Subscription { number } => wire::CapacityAction::Subscription(
            wire::CapacityActionSubscription::new(limits, wire::CapacityActionSubscriptionParts { number: *number })?,
        ),
        tasks::CapacityAction::Effect { entry } => wire::CapacityAction::Effect(wire::CapacityActionEffect::new(
            limits,
            wire::CapacityActionEffectParts { entry: *entry },
        )?),
        tasks::CapacityAction::Claim { attempt } => wire::CapacityAction::Claim(wire::CapacityActionClaim::new(
            limits,
            wire::CapacityActionClaimParts { attempt: *attempt },
        )?),
        tasks::CapacityAction::Place { hold } => wire::CapacityAction::Place(wire::CapacityActionPlace::new(
            limits,
            wire::CapacityActionPlaceParts { hold: *hold },
        )?),
        tasks::CapacityAction::Release => wire::CapacityAction::Release,
        tasks::CapacityAction::Move => wire::CapacityAction::MoveTask,
    })
}
pub(super) fn decode_capacity_action(value: wire::CapacityAction, _limits: &Limits) -> tasks::CapacityAction {
    match value {
        wire::CapacityAction::Batch(value) => {
            let wire::CapacityActionBatchParts { first, members } = value.into_parts();
            tasks::CapacityAction::Batch { first, members }
        }
        wire::CapacityAction::Proposal(value) => {
            let wire::CapacityActionProposalParts { number } = value.into_parts();
            tasks::CapacityAction::Proposal { number }
        }
        wire::CapacityAction::PersonProposal(value) => {
            let wire::CapacityActionPersonProposalParts { number } = value.into_parts();
            tasks::CapacityAction::PersonProposal { number }
        }
        wire::CapacityAction::Message(value) => {
            let wire::CapacityActionMessageParts { task, number } = value.into_parts();
            tasks::CapacityAction::Message { task, number }
        }
        wire::CapacityAction::Source(value) => {
            let wire::CapacityActionSourceParts { funder } = value.into_parts();
            tasks::CapacityAction::Source { funder: super::value::read_funder(funder) }
        }
        wire::CapacityAction::Introduce(value) => {
            let wire::CapacityActionIntroduceParts { left, right } = value.into_parts();
            tasks::CapacityAction::Introduce { left, right }
        }
        wire::CapacityAction::Subscription(value) => {
            let wire::CapacityActionSubscriptionParts { number } = value.into_parts();
            tasks::CapacityAction::Subscription { number }
        }
        wire::CapacityAction::Effect(value) => {
            let wire::CapacityActionEffectParts { entry } = value.into_parts();
            tasks::CapacityAction::Effect { entry }
        }
        wire::CapacityAction::Claim(value) => {
            let wire::CapacityActionClaimParts { attempt } = value.into_parts();
            tasks::CapacityAction::Claim { attempt }
        }
        wire::CapacityAction::Place(value) => tasks::CapacityAction::Place { hold: value.hold() },
        wire::CapacityAction::Release => tasks::CapacityAction::Release,
        wire::CapacityAction::MoveTask => tasks::CapacityAction::Move,
    }
}

pub(super) fn encode_capacity_cause(
    value: &tasks::CapacityCause,
    limits: &Limits,
) -> Result<wire::CapacityCause, wire::Problem> {
    Ok(match value {
        tasks::CapacityCause::PlacedHolds { limit } => wire::CapacityCause::PlacedHolds(
            wire::CapacityCausePlacedHolds::new(limits, wire::CapacityCausePlacedHoldsParts { limit: *limit })?,
        ),
        tasks::CapacityCause::ProjectPlacedHolds { limit } => {
            wire::CapacityCause::ProjectPlacedHolds(wire::CapacityCauseProjectPlacedHolds::new(
                limits,
                wire::CapacityCauseProjectPlacedHoldsParts { limit: *limit },
            )?)
        }
        tasks::CapacityCause::PartyPlacedHolds { limit } => {
            wire::CapacityCause::PartyPlacedHolds(wire::CapacityCausePartyPlacedHolds::new(
                limits,
                wire::CapacityCausePartyPlacedHoldsParts { limit: *limit },
            )?)
        }
        tasks::CapacityCause::Exemptions { limit } => wire::CapacityCause::Exemptions(
            wire::CapacityCauseExemptions::new(limits, wire::CapacityCauseExemptionsParts { limit: *limit })?,
        ),
        tasks::CapacityCause::Tasks { limit } => wire::CapacityCause::Tasks(wire::CapacityCauseTasks::new(
            limits,
            wire::CapacityCauseTasksParts { limit: *limit },
        )?),
        tasks::CapacityCause::CreationFlights { limit } => wire::CapacityCause::CreationFlights(
            wire::CapacityCauseCreationFlights::new(limits, wire::CapacityCauseCreationFlightsParts { limit: *limit })?,
        ),
        tasks::CapacityCause::ResourceReports { limit } => wire::CapacityCause::ResourceReports(
            wire::CapacityCauseResourceReports::new(limits, wire::CapacityCauseResourceReportsParts { limit: *limit })?,
        ),
        tasks::CapacityCause::ProjectTasks { limit } => wire::CapacityCause::ProjectTasks(
            wire::CapacityCauseProjectTasks::new(limits, wire::CapacityCauseProjectTasksParts { limit: *limit })?,
        ),
        tasks::CapacityCause::Tree { limit } => wire::CapacityCause::Tree(wire::CapacityCauseTree::new(
            limits,
            wire::CapacityCauseTreeParts { limit: *limit },
        )?),
        tasks::CapacityCause::Delegates { limit } => wire::CapacityCause::Delegates(wire::CapacityCauseDelegates::new(
            limits,
            wire::CapacityCauseDelegatesParts { limit: *limit },
        )?),
        tasks::CapacityCause::Inbox { task, messages, bytes } => {
            wire::CapacityCause::Inbox(wire::CapacityCauseInbox::new(
                limits,
                wire::CapacityCauseInboxParts { task: *task, messages: *messages, bytes: *bytes },
            )?)
        }
        tasks::CapacityCause::Holdings { limit } => wire::CapacityCause::Holdings(wire::CapacityCauseHoldings::new(
            limits,
            wire::CapacityCauseHoldingsParts { limit: *limit },
        )?),
        tasks::CapacityCause::HoldWaiters { limit } => wire::CapacityCause::HoldWaiters(
            wire::CapacityCauseHoldWaiters::new(limits, wire::CapacityCauseHoldWaitersParts { limit: *limit })?,
        ),
        tasks::CapacityCause::Proposals { limit } => wire::CapacityCause::Proposals(wire::CapacityCauseProposals::new(
            limits,
            wire::CapacityCauseProposalsParts { limit: *limit },
        )?),
        tasks::CapacityCause::References { limit } => wire::CapacityCause::References(
            wire::CapacityCauseReferences::new(limits, wire::CapacityCauseReferencesParts { limit: *limit })?,
        ),
        tasks::CapacityCause::Subscriptions { limit } => wire::CapacityCause::Subscriptions(
            wire::CapacityCauseSubscriptions::new(limits, wire::CapacityCauseSubscriptionsParts { limit: *limit })?,
        ),
        tasks::CapacityCause::Funders { limit } => wire::CapacityCause::Funders(wire::CapacityCauseFunders::new(
            limits,
            wire::CapacityCauseFundersParts { limit: *limit },
        )?),
        tasks::CapacityCause::Writers { limit } => wire::CapacityCause::Writers(wire::CapacityCauseWriters::new(
            limits,
            wire::CapacityCauseWritersParts { limit: *limit },
        )?),
    })
}
pub(super) fn decode_capacity_cause(value: wire::CapacityCause, _limits: &Limits) -> tasks::CapacityCause {
    match value {
        wire::CapacityCause::PlacedHolds(value) => tasks::CapacityCause::PlacedHolds { limit: value.limit() },
        wire::CapacityCause::ProjectPlacedHolds(value) => {
            tasks::CapacityCause::ProjectPlacedHolds { limit: value.limit() }
        }
        wire::CapacityCause::PartyPlacedHolds(value) => tasks::CapacityCause::PartyPlacedHolds { limit: value.limit() },
        wire::CapacityCause::Exemptions(value) => tasks::CapacityCause::Exemptions { limit: value.limit() },
        wire::CapacityCause::Tasks(value) => {
            let wire::CapacityCauseTasksParts { limit } = value.into_parts();
            tasks::CapacityCause::Tasks { limit }
        }
        wire::CapacityCause::CreationFlights(value) => {
            let wire::CapacityCauseCreationFlightsParts { limit } = value.into_parts();
            tasks::CapacityCause::CreationFlights { limit }
        }
        wire::CapacityCause::ResourceReports(value) => {
            let wire::CapacityCauseResourceReportsParts { limit } = value.into_parts();
            tasks::CapacityCause::ResourceReports { limit }
        }
        wire::CapacityCause::ProjectTasks(value) => {
            let wire::CapacityCauseProjectTasksParts { limit } = value.into_parts();
            tasks::CapacityCause::ProjectTasks { limit }
        }
        wire::CapacityCause::Tree(value) => {
            let wire::CapacityCauseTreeParts { limit } = value.into_parts();
            tasks::CapacityCause::Tree { limit }
        }
        wire::CapacityCause::Delegates(value) => {
            let wire::CapacityCauseDelegatesParts { limit } = value.into_parts();
            tasks::CapacityCause::Delegates { limit }
        }
        wire::CapacityCause::Inbox(value) => {
            let wire::CapacityCauseInboxParts { task, messages, bytes } = value.into_parts();
            tasks::CapacityCause::Inbox { task, messages, bytes }
        }
        wire::CapacityCause::Holdings(value) => {
            let wire::CapacityCauseHoldingsParts { limit } = value.into_parts();
            tasks::CapacityCause::Holdings { limit }
        }
        wire::CapacityCause::HoldWaiters(value) => {
            let wire::CapacityCauseHoldWaitersParts { limit } = value.into_parts();
            tasks::CapacityCause::HoldWaiters { limit }
        }
        wire::CapacityCause::Proposals(value) => {
            let wire::CapacityCauseProposalsParts { limit } = value.into_parts();
            tasks::CapacityCause::Proposals { limit }
        }
        wire::CapacityCause::References(value) => {
            let wire::CapacityCauseReferencesParts { limit } = value.into_parts();
            tasks::CapacityCause::References { limit }
        }
        wire::CapacityCause::Subscriptions(value) => {
            let wire::CapacityCauseSubscriptionsParts { limit } = value.into_parts();
            tasks::CapacityCause::Subscriptions { limit }
        }
        wire::CapacityCause::Funders(value) => {
            let wire::CapacityCauseFundersParts { limit } = value.into_parts();
            tasks::CapacityCause::Funders { limit }
        }
        wire::CapacityCause::Writers(value) => {
            let wire::CapacityCauseWritersParts { limit } = value.into_parts();
            tasks::CapacityCause::Writers { limit }
        }
    }
}

pub(super) fn encode_capacity(value: &tasks::Capacity, limits: &Limits) -> Result<wire::Capacity, wire::Problem> {
    let tasks::Capacity { project, by, action, cause } = value;
    wire::Capacity::new(
        limits,
        wire::CapacityParts {
            project: *project,
            by: super::value::party(by, limits)?,
            action: encode_capacity_action(action, limits)?,
            cause: encode_capacity_cause(cause, limits)?,
        },
    )
}
pub(super) fn decode_capacity(value: wire::Capacity, limits: &Limits) -> tasks::Capacity {
    let wire::CapacityParts { project, by, action, cause } = value.into_parts();
    tasks::Capacity {
        project,
        by: super::value::read_party(by),
        action: decode_capacity_action(action, limits),
        cause: decode_capacity_cause(cause, limits),
    }
}

pub(super) fn encode_refusal(value: &tasks::Refusal, limits: &Limits) -> Result<wire::Refusal, wire::Problem> {
    Ok(match value {
        tasks::Refusal::Placed { hold } => {
            wire::Refusal::Placed(wire::RefusalPlaced::new(limits, wire::RefusalPlacedParts { hold: *hold })?)
        }
        tasks::Refusal::HoldOwner { hold } => {
            wire::Refusal::HoldOwner(wire::RefusalHoldOwner::new(limits, wire::RefusalHoldOwnerParts { hold: *hold })?)
        }
        tasks::Refusal::HoldRevision { revision } => wire::Refusal::HoldRevision(wire::RefusalHoldRevision::new(
            limits,
            wire::RefusalHoldRevisionParts { revision: *revision },
        )?),
        tasks::Refusal::Unproposable => wire::Refusal::Unproposable,
        tasks::Refusal::PersonRevision { revision } => wire::Refusal::PersonRevision(wire::RefusalPersonRevision::new(
            limits,
            wire::RefusalPersonRevisionParts { revision: *revision },
        )?),
        tasks::Refusal::ResourceUnavailable => wire::Refusal::ResourceUnavailable,
        tasks::Refusal::NotReady => wire::Refusal::NotReady,
        tasks::Refusal::Unknown => wire::Refusal::Unknown,
        tasks::Refusal::Busy => wire::Refusal::Busy,
        tasks::Refusal::Duplicate => wire::Refusal::Duplicate,
        tasks::Refusal::Empty => wire::Refusal::Empty,
        tasks::Refusal::Batch => wire::Refusal::Batch,
        tasks::Refusal::Live => wire::Refusal::Live,
        tasks::Refusal::Project => wire::Refusal::Project,
        tasks::Refusal::Tree => wire::Refusal::Tree,
        tasks::Refusal::Depth => wire::Refusal::Depth,
        tasks::Refusal::Delegates => wire::Refusal::Delegates,
        tasks::Refusal::Reference => wire::Refusal::Reference,
        tasks::Refusal::Subscription => wire::Refusal::Subscription,
        tasks::Refusal::Dependencies => wire::Refusal::Dependencies,
        tasks::Refusal::Circle { cut } => {
            wire::Refusal::Circle(wire::RefusalCircle::new(limits, wire::RefusalCircleParts { cut: *cut })?)
        }
        tasks::Refusal::DependencyEnded { how } => wire::Refusal::DependencyEnded(wire::RefusalDependencyEnded::new(
            limits,
            wire::RefusalDependencyEndedParts { how: super::state::status(*how) },
        )?),
        tasks::Refusal::Kept => wire::Refusal::Kept,
        tasks::Refusal::Executor => wire::Refusal::Executor,
        tasks::Refusal::Spec => wire::Refusal::Spec,
        tasks::Refusal::Contract => wire::Refusal::Contract,
        tasks::Refusal::AuthorityShape => wire::Refusal::AuthorityShape,
        tasks::Refusal::Inputs => wire::Refusal::Inputs,
        tasks::Refusal::State => wire::Refusal::State,
        tasks::Refusal::Goals => wire::Refusal::Goals,
        tasks::Refusal::Grace => wire::Refusal::Grace,
        tasks::Refusal::Attempt => wire::Refusal::Attempt,
        tasks::Refusal::LiveDelegates => wire::Refusal::LiveDelegates,
        tasks::Refusal::Restore => wire::Refusal::Restore,
        tasks::Refusal::Read => wire::Refusal::Read,
        tasks::Refusal::Turn => wire::Refusal::Turn,
        tasks::Refusal::Funding => wire::Refusal::Funding,
        tasks::Refusal::HoldKind => wire::Refusal::HoldKind,
        tasks::Refusal::HoldTaken => wire::Refusal::HoldTaken,
        tasks::Refusal::Holds => wire::Refusal::Holds,
        tasks::Refusal::Account { account, why } => wire::Refusal::Account(wire::RefusalAccount::new(
            limits,
            wire::RefusalAccountParts { account: *account, why: encode_account_why(*why) },
        )?),
        tasks::Refusal::Llm { model, why } => wire::Refusal::Llm(wire::RefusalLlm::new(
            limits,
            wire::RefusalLlmParts { model: *model, why: encode_llm_fault(*why) },
        )?),
    })
}
pub(super) fn decode_refusal(value: wire::Refusal, _limits: &Limits) -> tasks::Refusal {
    match value {
        wire::Refusal::Placed(value) => tasks::Refusal::Placed { hold: value.hold() },
        wire::Refusal::HoldOwner(value) => tasks::Refusal::HoldOwner { hold: value.hold() },
        wire::Refusal::HoldRevision(value) => tasks::Refusal::HoldRevision { revision: value.revision() },
        wire::Refusal::Unproposable => tasks::Refusal::Unproposable,
        wire::Refusal::PersonRevision(value) => {
            let wire::RefusalPersonRevisionParts { revision } = value.into_parts();
            tasks::Refusal::PersonRevision { revision }
        }
        wire::Refusal::ResourceUnavailable => tasks::Refusal::ResourceUnavailable,
        wire::Refusal::NotReady => tasks::Refusal::NotReady,
        wire::Refusal::Unknown => tasks::Refusal::Unknown,
        wire::Refusal::Busy => tasks::Refusal::Busy,
        wire::Refusal::Duplicate => tasks::Refusal::Duplicate,
        wire::Refusal::Empty => tasks::Refusal::Empty,
        wire::Refusal::Batch => tasks::Refusal::Batch,
        wire::Refusal::Live => tasks::Refusal::Live,
        wire::Refusal::Project => tasks::Refusal::Project,
        wire::Refusal::Tree => tasks::Refusal::Tree,
        wire::Refusal::Depth => tasks::Refusal::Depth,
        wire::Refusal::Delegates => tasks::Refusal::Delegates,
        wire::Refusal::Reference => tasks::Refusal::Reference,
        wire::Refusal::Subscription => tasks::Refusal::Subscription,
        wire::Refusal::Dependencies => tasks::Refusal::Dependencies,
        wire::Refusal::Circle(value) => tasks::Refusal::Circle { cut: value.into_parts().cut },
        wire::Refusal::DependencyEnded(value) => {
            tasks::Refusal::DependencyEnded { how: super::state::read_status(value.into_parts().how) }
        }
        wire::Refusal::Kept => tasks::Refusal::Kept,
        wire::Refusal::Executor => tasks::Refusal::Executor,
        wire::Refusal::Spec => tasks::Refusal::Spec,
        wire::Refusal::Contract => tasks::Refusal::Contract,
        wire::Refusal::AuthorityShape => tasks::Refusal::AuthorityShape,
        wire::Refusal::Inputs => tasks::Refusal::Inputs,
        wire::Refusal::State => tasks::Refusal::State,
        wire::Refusal::Goals => tasks::Refusal::Goals,
        wire::Refusal::Grace => tasks::Refusal::Grace,
        wire::Refusal::Attempt => tasks::Refusal::Attempt,
        wire::Refusal::LiveDelegates => tasks::Refusal::LiveDelegates,
        wire::Refusal::Restore => tasks::Refusal::Restore,
        wire::Refusal::Read => tasks::Refusal::Read,
        wire::Refusal::Turn => tasks::Refusal::Turn,
        wire::Refusal::Funding => tasks::Refusal::Funding,
        wire::Refusal::HoldKind => tasks::Refusal::HoldKind,
        wire::Refusal::HoldTaken => tasks::Refusal::HoldTaken,
        wire::Refusal::Holds => tasks::Refusal::Holds,
        wire::Refusal::Account(value) => {
            let wire::RefusalAccountParts { account, why } = value.into_parts();
            tasks::Refusal::Account { account, why: decode_account_why(why) }
        }
        wire::Refusal::Llm(value) => {
            let wire::RefusalLlmParts { model, why } = value.into_parts();
            tasks::Refusal::Llm { model, why: decode_llm_fault(why) }
        }
    }
}
fn encode_llm_fault(value: tasks::LlmFault) -> wire::LlmFault {
    match value {
        tasks::LlmFault::Unconfigured => wire::LlmFault::Unconfigured,
        tasks::LlmFault::Effort => wire::LlmFault::Effort,
        tasks::LlmFault::Charter => wire::LlmFault::Charter,
        tasks::LlmFault::Authority => wire::LlmFault::Authority,
        tasks::LlmFault::Procedure => wire::LlmFault::Procedure,
        tasks::LlmFault::Repeated => wire::LlmFault::Repeated,
    }
}
fn decode_llm_fault(value: wire::LlmFault) -> tasks::LlmFault {
    match value {
        wire::LlmFault::Unconfigured => tasks::LlmFault::Unconfigured,
        wire::LlmFault::Effort => tasks::LlmFault::Effort,
        wire::LlmFault::Charter => tasks::LlmFault::Charter,
        wire::LlmFault::Authority => tasks::LlmFault::Authority,
        wire::LlmFault::Procedure => tasks::LlmFault::Procedure,
        wire::LlmFault::Repeated => tasks::LlmFault::Repeated,
    }
}

pub(super) fn encode_problem(value: &tasks::Problem, limits: &Limits) -> Result<wire::TaskProblem, wire::Problem> {
    let tasks::Problem { task, why, blocked_by, capacity } = value;
    wire::TaskProblem::new(
        limits,
        wire::TaskProblemParts {
            task: *task,
            why: encode_refusal(why, limits)?,
            blocked_by: match blocked_by {
                Some(value) => Some(encode_problem_blocked_by(value, limits)?),
                None => None,
            },
            capacity: match capacity {
                Some(value) => Some(encode_capacity(value, limits)?),
                None => None,
            },
        },
    )
}
#[expect(clippy::manual_map, reason = "step code uses concrete matches (programming-model.md, 10)")]
pub(super) fn decode_problem(value: wire::TaskProblem, limits: &Limits) -> tasks::Problem {
    let wire::TaskProblemParts { task, why, blocked_by, capacity } = value.into_parts();
    tasks::Problem {
        task,
        why: decode_refusal(why, limits),
        blocked_by: match blocked_by {
            Some(value) => Some(decode_problem_blocked_by(value, limits)),
            None => None,
        },
        capacity: match capacity {
            Some(value) => Some(decode_capacity(value, limits)),
            None => None,
        },
    }
}

pub(super) fn encode_result_followups(
    value: &tasks::ResultFollowups,
    limits: &Limits,
) -> Result<wire::ResultFollowups, wire::Problem> {
    Ok(match value {
        tasks::ResultFollowups::Delegates(value) => {
            wire::ResultFollowups::Delegates(wire::ResultFollowupsDelegates::new(
                limits,
                wire::ResultFollowupsDelegatesParts { value: encode_result_followups_delegates_value(value, limits)? },
            )?)
        }
        tasks::ResultFollowups::Proposal(value) => {
            wire::ResultFollowups::Proposal(Box::new(wire::ResultFollowupsProposal::new(
                limits,
                wire::ResultFollowupsProposalParts { value: super::proposals::proposal(value, limits, None)? },
            )?))
        }
    })
}
pub(super) fn decode_result_followups(
    value: wire::ResultFollowups,
    limits: &Limits,
) -> Result<tasks::ResultFollowups, wire::Problem> {
    Ok(match value {
        wire::ResultFollowups::Delegates(value) => {
            let wire::ResultFollowupsDelegatesParts { value } = value.into_parts();
            tasks::ResultFollowups::Delegates(decode_result_followups_delegates_value(value, limits)?)
        }
        wire::ResultFollowups::Proposal(value) => {
            let wire::ResultFollowupsProposalParts { value } = (*value).into_parts();
            tasks::ResultFollowups::Proposal(Box::new(super::proposals::read_proposal(value, limits)?))
        }
    })
}

pub(super) fn encode_end(value: &tasks::End, limits: &Limits) -> Result<wire::End, wire::Problem> {
    Ok(match value {
        tasks::End::Finished { result, cancel_delegates } => wire::End::Finished(wire::EndFinished::new(
            limits,
            wire::EndFinishedParts {
                result: super::phase::result(result, limits)?,
                cancel_delegates: *cancel_delegates,
            },
        )?),
        tasks::End::FinishedWithFollowups { result, followups } => {
            wire::End::FinishedWithFollowups(Box::new(wire::EndFinishedWithFollowups::new(
                limits,
                wire::EndFinishedWithFollowupsParts {
                    result: super::phase::result(result, limits)?,
                    followups: encode_result_followups(followups, limits)?,
                },
            )?))
        }
        tasks::End::Exceeded(value) => wire::End::Exceeded(wire::Exceeded::new(
            limits,
            wire::ExceededParts {
                bound: match value.bound {
                    tasks::Bound::Reasoning => wire::Bound::Reasoning,
                    tasks::Bound::Metadata => wire::Bound::Metadata,
                    tasks::Bound::Items => wire::Bound::Items,
                    tasks::Bound::Answer => wire::Bound::Answer,
                    tasks::Bound::Tools => wire::Bound::Tools,
                    tasks::Bound::HistoryItems => wire::Bound::HistoryItems,
                    tasks::Bound::History => wire::Bound::History,
                    tasks::Bound::Window => wire::Bound::Window,
                    tasks::Bound::Output => wire::Bound::Output,
                },
                value: value.value,
            },
        )?),
        tasks::End::Parked => wire::End::Parked,
        tasks::End::Failed(value) => wire::End::Failed(wire::EndFailed::new(
            limits,
            wire::EndFailedParts { value: super::value::class(*value) },
        )?),
        tasks::End::Refused => wire::End::Refused,
        tasks::End::Recheck => wire::End::Recheck,
    })
}
pub(super) fn decode_end(value: wire::End, limits: &Limits) -> Result<tasks::End, wire::Problem> {
    Ok(match value {
        wire::End::Finished(value) => {
            let wire::EndFinishedParts { result, cancel_delegates } = value.into_parts();
            tasks::End::Finished { result: super::phase::read_result(result, limits)?, cancel_delegates }
        }
        wire::End::FinishedWithFollowups(value) => {
            let wire::EndFinishedWithFollowupsParts { result, followups } = (*value).into_parts();
            tasks::End::FinishedWithFollowups {
                result: super::phase::read_result(result, limits)?,
                followups: decode_result_followups(followups, limits)?,
            }
        }
        wire::End::Exceeded(value) => tasks::End::Exceeded(tasks::Exceeded {
            bound: match value.bound() {
                wire::Bound::Reasoning => tasks::Bound::Reasoning,
                wire::Bound::Metadata => tasks::Bound::Metadata,
                wire::Bound::Items => tasks::Bound::Items,
                wire::Bound::Answer => tasks::Bound::Answer,
                wire::Bound::Tools => tasks::Bound::Tools,
                wire::Bound::HistoryItems => tasks::Bound::HistoryItems,
                wire::Bound::History => tasks::Bound::History,
                wire::Bound::Window => tasks::Bound::Window,
                wire::Bound::Output => tasks::Bound::Output,
            },
            value: value.value(),
        }),
        wire::End::Parked => tasks::End::Parked,
        wire::End::Failed(value) => {
            let wire::EndFailedParts { value } = value.into_parts();
            tasks::End::Failed(super::value::read_class(value))
        }
        wire::End::Refused => tasks::End::Refused,
        wire::End::Recheck => tasks::End::Recheck,
    })
}

fn encode_problem_blocked_by(values: &[u64], limits: &Limits) -> Result<List<wire::Number>, wire::Problem> {
    let mut result = List::with_capacity(match u32::try_from(values.len()) {
        Ok(value) => value,
        Err(error) => return Err(too_large(error)),
    });
    for value in values {
        result.push(wire::Number::new(limits, wire::NumberParts { value: *value })?).expect("same number of members");
    }
    Ok(result)
}
fn decode_problem_blocked_by(values: List<wire::Number>, _limits: &Limits) -> Box<[u64]> {
    let mut result = List::with_capacity(values.len());
    for value in values.into_boxed() {
        result.push(value.into_parts().value).expect("same number of members");
    }
    result.into_boxed()
}

fn encode_result_followups_delegates_value(
    values: &[tasks::New],
    limits: &Limits,
) -> Result<List<wire::Creation>, wire::Problem> {
    let mut result = List::with_capacity(match u32::try_from(values.len()) {
        Ok(value) => value,
        Err(error) => return Err(too_large(error)),
    });
    for value in values {
        result.push(super::phase::new(value, limits, None)?).expect("same number of members");
    }
    Ok(result)
}
fn decode_result_followups_delegates_value(
    values: List<wire::Creation>,
    limits: &Limits,
) -> Result<Box<[tasks::New]>, wire::Problem> {
    let mut result = List::with_capacity(values.len());
    for value in values.into_boxed() {
        result.push(super::phase::read_new(value, limits)?).expect("same number of members");
    }
    Ok(result.into_boxed())
}
pub fn problem(value: &tasks::Problem, limits: &Limits) -> Result<Box<[u8]>, wire::Problem> {
    let value = encode_problem(value, limits)?;
    let mut writer = Writer::new(usize::try_from(value.measure()).expect("measured length"));
    value.encode(&mut writer).expect("measured room");
    Ok(writer.finish())
}
pub fn read_problem(bytes: &[u8], limits: &Limits) -> Result<tasks::Problem, wire::Problem> {
    let value = wire::TaskProblem::decode(limits, &mut Reader::new(bytes))?;
    Ok(decode_problem(value, limits))
}
pub fn end(value: &tasks::End, limits: &Limits) -> Result<Box<[u8]>, wire::Problem> {
    let value = wire::EndRecord::new(limits, wire::EndRecordParts { value: encode_end(value, limits)? })?;
    let mut writer = Writer::new(usize::try_from(value.measure()).expect("measured length"));
    value.encode(&mut writer).expect("measured room");
    Ok(writer.finish())
}
pub fn read_end(bytes: &[u8], limits: &Limits) -> Result<tasks::End, wire::Problem> {
    let value = wire::EndRecord::decode(limits, &mut Reader::new(bytes))?;
    decode_end(value.into_parts().value, limits)
}
pub fn phase(value: &tasks::Phase, limits: &Limits) -> Result<Box<[u8]>, wire::Problem> {
    let value = wire::PhaseRecord::new(limits, wire::PhaseRecordParts { value: super::phase::phase(value, limits)? })?;
    let mut writer = Writer::new(usize::try_from(value.measure()).expect("measured length"));
    value.encode(&mut writer).expect("measured room");
    Ok(writer.finish())
}
pub fn read_phase(bytes: &[u8], limits: &Limits) -> Result<tasks::Phase, wire::Problem> {
    let value = wire::PhaseRecord::decode(limits, &mut Reader::new(bytes))?;
    super::phase::read_phase(value.into_parts().value, limits)
}
pub fn change(value: &tasks::Change, limits: &Limits) -> Result<Box<[u8]>, wire::Problem> {
    let value =
        wire::ChangeRecord::new(limits, wire::ChangeRecordParts { value: super::proposals::change(value, limits)? })?;
    let mut writer = Writer::new(usize::try_from(value.measure()).expect("measured length"));
    value.encode(&mut writer).expect("measured room");
    Ok(writer.finish())
}
pub fn read_change(bytes: &[u8], limits: &Limits) -> Result<tasks::Change, wire::Problem> {
    let value = wire::ChangeRecord::decode(limits, &mut Reader::new(bytes))?;
    super::proposals::read_change(value.into_parts().value, limits)
}
fn too_large(_: core::num::TryFromIntError) -> wire::Problem {
    wire::Problem { path: wire::Path::TaskProblemBlockedBy, reason: skein_codec::Reason::Bound }
}

fn encode_account_why(value: tasks::AccountWhy) -> wire::AccountWhy {
    match value {
        tasks::AccountWhy::Unknown => wire::AccountWhy::Unknown,
        tasks::AccountWhy::Provider => wire::AccountWhy::Provider,
        tasks::AccountWhy::Uses => wire::AccountWhy::Uses,
        tasks::AccountWhy::Limited => wire::AccountWhy::Limited,
    }
}

fn decode_account_why(value: wire::AccountWhy) -> tasks::AccountWhy {
    match value {
        wire::AccountWhy::Unknown => tasks::AccountWhy::Unknown,
        wire::AccountWhy::Provider => tasks::AccountWhy::Provider,
        wire::AccountWhy::Uses => tasks::AccountWhy::Uses,
        wire::AccountWhy::Limited => tasks::AccountWhy::Limited,
    }
}
