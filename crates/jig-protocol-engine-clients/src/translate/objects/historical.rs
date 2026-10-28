#![expect(clippy::manual_map, reason = "step code matches optional owner facts without closures")]
//! Whole single-read owner documents (protocol/client.md, 3 and 6).
use super::super::{chips, owner, task, w};
use alloc::boxed::Box;
use jig_domain_core::documents as d;
use jig_domain_people as p;
use jig_domain_tasks as t;
use skein_lib::{List, bytes};
pub fn retained_amendment(
    models: &[crate::Model],
    value: &d::AmendmentChange,
    limits: &w::Limits,
) -> Result<w::OwnerRetainedAmendment, w::Problem> {
    let dependencies = match &value.dependencies {
        Some(rows) => {
            let mut copied = List::with_capacity(rows.len());
            for row in rows {
                copied.push(*row).expect("supplied rows fit");
            }
            Some(copied)
        }
        None => None,
    };
    w::OwnerRetainedAmendment::new(
        limits,
        w::OwnerRetainedAmendmentParts {
            addressed: match value.addressed {
                Some(item) => Some(w::AddressChange::new(
                    limits,
                    w::AddressChangeParts {
                        revision: w::Revision::new(limits, w::RevisionParts { value: item.revision })?,
                        address: chips::address(item.address, limits)?,
                    },
                )?),
                None => None,
            },
            instructions: match &value.instructions {
                Some(item) => Some(owner::text(item, limits)?),
                None => None,
            },
            release: value.release,
            spec: match &value.spec {
                Some(item) => Some(owner::specification(item, limits)?),
                None => None,
            },
            authority: match &value.authority {
                Some(item) => Some(owner::authority(models, item, limits)?),
                None => None,
            },
            wake: match value.wake {
                Some(item) => Some(owner::wake(item, limits)?),
                None => None,
            },
            dependencies,
            reason: owner::text(&value.reason, limits)?,
            cut: value.cut,
        },
    )
}
pub fn retained_action(
    models: &[crate::Model],
    value: &d::RetainedAction,
    limits: &w::Limits,
) -> Result<w::OwnerRetainedAction, w::Problem> {
    Ok(match value {
        d::RetainedAction::Settings { against, items, made } => {
            w::OwnerRetainedAction::Settings(w::OwnerRetainedActionSettings::new(
                limits,
                w::OwnerRetainedActionSettingsParts {
                    against: *against,
                    items: super::super::settings::item_heads(items, limits)?,
                    made: *made,
                },
            )?)
        }
        d::RetainedAction::Batch(rows) => {
            let mut copied = List::with_capacity(rows.len());
            for row in rows {
                copied.push(super::proposals::proposed(models, row, limits)?).expect("supplied rows fit");
            }
            let copied = super::proposals::proposed_task_pieces(
                &copied,
                limits.owner_retained_action_batch_value,
                w::Path::OwnerRetainedActionBatchValue,
            )?;
            w::OwnerRetainedAction::Batch(w::OwnerRetainedActionBatch::new(
                limits,
                w::OwnerRetainedActionBatchParts { value: copied },
            )?)
        }
        d::RetainedAction::Effect { part, authority, attempt, completion, position, purpose } => {
            w::OwnerRetainedAction::Effect(w::OwnerRetainedActionEffect::new(
                limits,
                w::OwnerRetainedActionEffectParts {
                    part: owner::part(part, limits)?,
                    authority: owner::authority(models, authority, limits)?,
                    attempt: *attempt,
                    completion: *completion,
                    position: *position,
                    purpose: *purpose,
                },
            )?)
        }
        d::RetainedAction::ProcedureEffect { part, authority, step, purpose } => {
            w::OwnerRetainedAction::ProcedureEffect(w::OwnerRetainedActionProcedureEffect::new(
                limits,
                w::OwnerRetainedActionProcedureEffectParts {
                    part: owner::part(part, limits)?,
                    authority: owner::authority(models, authority, limits)?,
                    step: *step,
                    purpose: *purpose,
                },
            )?)
        }
        d::RetainedAction::Amend { task, change } => {
            w::OwnerRetainedAction::Amend(Box::new(w::OwnerRetainedActionAmend::new(
                limits,
                w::OwnerRetainedActionAmendParts { task: *task, change: retained_amendment(models, change, limits)? },
            )?))
        }
        d::RetainedAction::Widen { task, authority } => {
            w::OwnerRetainedAction::Widen(w::OwnerRetainedActionWiden::new(
                limits,
                w::OwnerRetainedActionWidenParts {
                    task: *task,
                    authority: owner::authority(models, authority, limits)?,
                },
            )?)
        }
        d::RetainedAction::Release { task } => w::OwnerRetainedAction::Release(w::OwnerRetainedActionRelease::new(
            limits,
            w::OwnerRetainedActionReleaseParts { task: *task },
        )?),
    })
}
pub fn proposal_history(
    models: &[crate::Model],
    value: &d::ProposalHistory,
    limits: &w::Limits,
) -> Result<w::OwnerProposalHistory, w::Problem> {
    w::OwnerProposalHistory::new(
        limits,
        w::OwnerProposalHistoryParts {
            action: retained_action(models, &value.action, limits)?,
            number: value.number,
            proposer: value.proposer,
            as_holder: value.as_holder,
            state: match &value.state {
                d::ProposalHistoryState::Pending(value) => {
                    w::OwnerProposalHistoryState::Pending(w::OwnerProposalHistoryStatePending::new(
                        limits,
                        w::OwnerProposalHistoryStatePendingParts {
                            holder: super::proposals::holder(value.holder, limits)?,
                            since: value.since.as_nanos(),
                        },
                    )?)
                }
                d::ProposalHistoryState::Accepted { by } => {
                    w::OwnerProposalHistoryState::Accepted(w::OwnerProposalHistoryStateAccepted::new(
                        limits,
                        w::OwnerProposalHistoryStateAcceptedParts { by: owner::requester(*by, limits)? },
                    )?)
                }
                d::ProposalHistoryState::Rejected { by, reason } => {
                    w::OwnerProposalHistoryState::Rejected(w::OwnerProposalHistoryStateRejected::new(
                        limits,
                        w::OwnerProposalHistoryStateRejectedParts {
                            by: owner::requester(*by, limits)?,
                            reason: owner::text(reason, limits)?,
                        },
                    )?)
                }
                d::ProposalHistoryState::Withdrawn => w::OwnerProposalHistoryState::Withdrawn,
            },
        },
    )
}
pub fn milestone(value: &d::MilestoneLine, limits: &w::Limits) -> Result<w::OwnerMilestoneLine, w::Problem> {
    w::OwnerMilestoneLine::new(
        limits,
        w::OwnerMilestoneLineParts {
            task: value.task,
            position: value.position,
            at: match value.at {
                Some(item) => Some(item.as_nanos()),
                None => None,
            },
            phase: match &value.phase {
                d::MilestonePhase::Waiting => w::OwnerMilestonePhase::Waiting,
                d::MilestonePhase::Active(value) => w::OwnerMilestonePhase::Active(w::OwnerMilestonePhaseActive::new(
                    limits,
                    w::OwnerMilestonePhaseActiveParts { value: chips::active(*value, limits)? },
                )?),
                d::MilestonePhase::Closing { stage, ending } => {
                    w::OwnerMilestonePhase::Closing(w::OwnerMilestonePhaseClosing::new(
                        limits,
                        w::OwnerMilestonePhaseClosingParts {
                            stage: chips::stage(*stage, limits)?,
                            ending: chips::ending(ending, limits)?,
                        },
                    )?)
                }
                d::MilestonePhase::Held { before, why, source, ending } => {
                    w::OwnerMilestonePhase::Held(w::OwnerMilestonePhaseHeld::new(
                        limits,
                        w::OwnerMilestonePhaseHeldParts {
                            before: chips::prior(*before, limits)?,
                            why: chips::hold(*why, limits)?,
                            source: match source {
                                Some(item) => Some(owner::part(item, limits)?),
                                None => None,
                            },
                            ending: match ending {
                                Some(item) => Some(chips::ending(item, limits)?),
                                None => None,
                            },
                        },
                    )?)
                }
                d::MilestonePhase::Ended(value) => w::OwnerMilestonePhase::Ended(w::OwnerMilestonePhaseEnded::new(
                    limits,
                    w::OwnerMilestonePhaseEndedParts { value: chips::ending(value, limits)? },
                )?),
            },
        },
    )
}
pub fn escalation_decision(
    value: &jig_domain_core::EscalationDecisionRecord,
    limits: &w::Limits,
) -> Result<w::OwnerEscalationDecisionRecord, w::Problem> {
    w::OwnerEscalationDecisionRecord::new(
        limits,
        w::OwnerEscalationDecisionRecordParts {
            project: value.project,
            requester: value.requester,
            task: value.task,
            revision: value.revision,
            by: value.by,
            at: value.at.as_nanos(),
            decision: match &value.decision {
                p::EscalationDecision::Release => w::OwnerEscalationDecision::Release,
                p::EscalationDecision::Pass => w::OwnerEscalationDecision::Pass,
                p::EscalationDecision::Reject { reason } => {
                    w::OwnerEscalationDecision::Reject(w::OwnerEscalationDecisionReject::new(
                        limits,
                        w::OwnerEscalationDecisionRejectParts { reason: bytes::copy_of(reason) },
                    )?)
                }
            },
        },
    )
}
pub fn proposal_decision(
    value: jig_domain_core::ProposalDecisionRecord,
    limits: &w::Limits,
) -> Result<w::OwnerProposalDecision, w::Problem> {
    w::OwnerProposalDecision::new(
        limits,
        w::OwnerProposalDecisionParts {
            project: value.project,
            proposer: task::proposal(&d::ProposalRef { proposer: value.proposer, number: value.proposal }, limits)?
                .proposer()
                .clone(),
            proposal: value.proposal,
            kind: super::proposals::kind(value.kind),
            by: value.by,
            at: value.at.as_nanos(),
            choice: match value.choice {
                p::ProposalChoice::Accepted => w::OwnerProposalChoice::Accepted,
                p::ProposalChoice::Rejected => w::OwnerProposalChoice::Rejected,
                p::ProposalChoice::Passed => w::OwnerProposalChoice::Passed,
                p::ProposalChoice::Withdrawn => w::OwnerProposalChoice::Withdrawn,
                p::ProposalChoice::Stale => w::OwnerProposalChoice::Stale,
            },
            created: match value.created {
                Some(item) => Some(w::OwnerProposalDecisionCreated::new(
                    limits,
                    w::OwnerProposalDecisionCreatedParts { first: item.0, last: item.1 },
                )?),
                None => None,
            },
        },
    )
}
pub fn person_proposal(
    models: &[crate::Model],
    value: &t::PersonProposal,
    limits: &w::Limits,
) -> Result<w::OwnerPersonProposal, w::Problem> {
    let [goal] = value.batch() else { return Err(super::super::problem(skein_codec::Reason::Malformed)) };
    w::OwnerPersonProposal::new(
        limits,
        w::OwnerPersonProposalParts {
            number: value.number,
            proposer: value.proposer,
            project: value.project,
            goal: super::goal::goal(models, goal, limits)?,
            state: match &value.state {
                t::PersonProposalState::Withdrawn { .. } => {
                    return Err(super::super::problem(skein_codec::Reason::Malformed));
                }
                t::PersonProposalState::Pending { since } => {
                    w::OwnerPersonProposalState::Pending(w::OwnerPersonProposalStatePending::new(
                        limits,
                        w::OwnerPersonProposalStatePendingParts { since: since.as_nanos() },
                    )?)
                }
                t::PersonProposalState::Accepted { by } | t::PersonProposalState::SettingsAccepted { by, .. } => {
                    w::OwnerPersonProposalState::Accepted(w::OwnerPersonProposalStateAccepted::new(
                        limits,
                        w::OwnerPersonProposalStateAcceptedParts { by: owner::requester(*by, limits)? },
                    )?)
                }
                t::PersonProposalState::Rejected { by, reason, message, at } => {
                    w::OwnerPersonProposalState::Rejected(w::OwnerPersonProposalStateRejected::new(
                        limits,
                        w::OwnerPersonProposalStateRejectedParts {
                            by: owner::requester(*by, limits)?,
                            reason: bytes::copy_of(reason),
                            message: *message,
                            at: at.as_nanos(),
                        },
                    )?)
                }
            },
        },
    )
}

pub(crate) fn settings_history(
    models: &[crate::Model],
    proposal: &t::PersonProposal,
    limits: &w::Limits,
) -> Result<w::OwnerProposalHistory, w::Problem> {
    let (against, items) = match &proposal.action {
        t::PersonAction::Settings { against, items } => (*against, d::settings_heads(items)),
        t::PersonAction::Batch(_) | t::PersonAction::Amend { .. } | t::PersonAction::ReleasePlaced { .. } => {
            unreachable!("settings history selected")
        }
    };
    let (state, made) = match &proposal.state {
        t::PersonProposalState::Pending { since } => (
            d::ProposalHistoryState::Pending(d::ProposalWaiting {
                holder: t::ProposalHolder::Policy { project: 0, kind: t::ProposalKind::Settings },
                since: *since,
            }),
            None,
        ),
        t::PersonProposalState::SettingsAccepted { by, revision } => {
            (d::ProposalHistoryState::Accepted { by: *by }, Some(*revision))
        }
        t::PersonProposalState::Accepted { .. } => unreachable!("settings acceptance records its revision"),
        t::PersonProposalState::Rejected { by, reason, .. } => {
            (d::ProposalHistoryState::Rejected { by: *by, reason: d::Text { bytes: reason.clone(), cut: false } }, None)
        }
        t::PersonProposalState::Withdrawn { .. } => (d::ProposalHistoryState::Withdrawn, None),
    };
    proposal_history(
        models,
        &d::ProposalHistory {
            number: proposal.number,
            proposer: proposal.proposer,
            as_holder: false,
            action: d::RetainedAction::Settings { against, items, made },
            state,
        },
        limits,
    )
}
