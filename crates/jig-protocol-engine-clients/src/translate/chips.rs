#![expect(clippy::manual_map, reason = "step code matches optional owner facts without closures")]
//! Whole owner facts encoded without clipping (protocol/client.md, 3).
use super::owner;
use super::{pieces, problem, w};
use alloc::boxed::Box;
use jig_domain_core::documents as d;
use jig_domain_tasks as t;
use skein_lib::{List, bytes};
pub fn class(value: t::Class, _limits: &w::Limits) -> Result<w::HoldFailure, w::Problem> {
    Ok(match value {
        t::Class::Transient => w::HoldFailure::Transient,
        t::Class::Permanent => w::HoldFailure::Permanent,
        t::Class::Run => w::HoldFailure::Run,
        t::Class::Agent => w::HoldFailure::Agent,
        t::Class::Lost => w::HoldFailure::Lost,
        t::Class::Invalid => w::HoldFailure::Invalid,
    })
}
pub fn retry(value: t::RetryNotice, limits: &w::Limits) -> Result<w::RetryNotice, w::Problem> {
    Ok(match value {
        t::RetryNotice::Failed { class: value, count, allowance } => {
            w::RetryNotice::OwnerFailed(w::RetryNoticeOwnerFailed::new(
                limits,
                w::RetryNoticeOwnerFailedParts { class: class(value, limits)?, count, allowance },
            )?)
        }
        t::RetryNotice::Refused { count } => {
            w::RetryNotice::Refused(w::RetryNoticeRefused::new(limits, w::RetryNoticeRefusedParts { count })?)
        }
    })
}
pub fn hold(value: t::Hold, limits: &w::Limits) -> Result<w::HoldCategory, w::Problem> {
    Ok(match value {
        t::Hold::Exceeded(value) => w::HoldCategory::Exceeded(w::Exceeded::new(
            limits,
            w::ExceededParts {
                bound: match value.bound {
                    t::Bound::Reasoning => w::Bound::Reasoning,
                    t::Bound::Metadata => w::Bound::Metadata,
                    t::Bound::Items => w::Bound::Items,
                    t::Bound::Answer => w::Bound::Answer,
                    t::Bound::Tools => w::Bound::Tools,
                    t::Bound::HistoryItems => w::Bound::HistoryItems,
                    t::Bound::History => w::Bound::History,
                    t::Bound::Window => w::Bound::Window,
                    t::Bound::Output => w::Bound::Output,
                },
                value: value.value,
            },
        )?),
        t::Hold::Failures(value) => w::HoldCategory::Failures(w::HoldCategoryFailures::new(
            limits,
            w::HoldCategoryFailuresParts { class: class(value, limits)? },
        )?),
        t::Hold::Dependency(task) => w::HoldCategory::Dependency(w::HoldCategoryDependency::new(
            limits,
            w::HoldCategoryDependencyParts { task },
        )?),
        t::Hold::Placed { hold } => {
            w::HoldCategory::Placed(w::HoldCategoryPlaced::new(limits, w::HoldCategoryPlacedParts { hold })?)
        }
        t::Hold::Uncertain { entry } => {
            w::HoldCategory::Uncertain(w::HoldCategoryUncertain::new(limits, w::HoldCategoryUncertainParts { entry })?)
        }
        t::Hold::PoolLost { pool } => {
            w::HoldCategory::PoolLost(w::HoldCategoryPoolLost::new(limits, w::HoldCategoryPoolLostParts { pool })?)
        }
        t::Hold::Stalled => w::HoldCategory::Stalled,
        t::Hold::Stopped => w::HoldCategory::Stopped,
        t::Hold::Drift => w::HoldCategory::Drift,
        t::Hold::Effects => w::HoldCategory::Effects,
        t::Hold::EffectFailed => w::HoldCategory::EffectFailed,
        t::Hold::Budget => w::HoldCategory::Budget,
        t::Hold::Deadline => w::HoldCategory::Deadline,
        t::Hold::HoldsWaited => w::HoldCategory::HoldsWaited,
        t::Hold::ResourceUnavailable => w::HoldCategory::ResourceUnavailable,
        t::Hold::ResourceAccess => w::HoldCategory::ResourceAccess,
        t::Hold::Procedure => w::HoldCategory::Procedure,
        t::Hold::Circle => w::HoldCategory::Circle,
    })
}
pub fn active(value: t::Active, limits: &w::Limits) -> Result<w::RunStage, w::Problem> {
    Ok(match value {
        t::Active::Idle => w::RunStage::Idle,
        t::Active::Due => w::RunStage::Due,
        t::Active::Preparing => w::RunStage::Preparing,
        t::Active::Claimed { attempt } => {
            w::RunStage::Claimed(w::RunStageClaimed::new(limits, w::RunStageClaimedParts { attempt })?)
        }
        t::Active::Running { attempt } => {
            w::RunStage::Running(w::RunStageRunning::new(limits, w::RunStageRunningParts { attempt })?)
        }
        t::Active::BackingOff { until } => w::RunStage::BackingOff(w::RunStageBackingOff::new(
            limits,
            w::RunStageBackingOffParts { until: until.as_nanos() },
        )?),
    })
}
pub fn stage(value: t::Stage, limits: &w::Limits) -> Result<w::ClosingStage, w::Problem> {
    Ok(match value {
        t::Stage::Run { attempt } => {
            w::ClosingStage::Run(w::ClosingStageRun::new(limits, w::ClosingStageRunParts { attempt })?)
        }
        t::Stage::Delegates => w::ClosingStage::Delegates,
        t::Stage::Effects => w::ClosingStage::Effects,
        t::Stage::Releases => w::ClosingStage::Releases,
        t::Stage::Settled => w::ClosingStage::Settled,
    })
}
pub fn address(value: t::PersonAddress, limits: &w::Limits) -> Result<w::PersonAddress, w::Problem> {
    Ok(match value {
        t::PersonAddress::Person(number) => {
            w::PersonAddress::Party(w::PersonAddressParty::new(limits, w::PersonAddressPartyParts { value: number })?)
        }
        t::PersonAddress::Role(number) => {
            w::PersonAddress::Role(w::PersonAddressRole::new(limits, w::PersonAddressRoleParts { value: number })?)
        }
    })
}
pub fn phase(value: &d::PhaseLine, limits: &w::Limits) -> Result<w::OwnerChipPhase, w::Problem> {
    Ok(match value {
        d::PhaseLine::Waiting { dependencies, more, holds } => {
            w::OwnerChipPhase::Waiting(w::OwnerChipPhaseWaiting::new(
                limits,
                w::OwnerChipPhaseWaitingParts { dependencies: dependencies.clone(), more: *more, holds: *holds },
            )?)
        }
        d::PhaseLine::Active(value) => w::OwnerChipPhase::Active(w::OwnerChipPhaseActive::new(
            limits,
            w::OwnerChipPhaseActiveParts { value: active(*value, limits)? },
        )?),
        d::PhaseLine::Person { address: value, taken_by, since } => {
            w::OwnerChipPhase::Person(w::OwnerChipPhasePerson::new(
                limits,
                w::OwnerChipPhasePersonParts {
                    address: address(*value, limits)?,
                    taken_by: *taken_by,
                    since: match *since {
                        Some(item) => Some(item.as_nanos()),
                        None => None,
                    },
                },
            )?)
        }
        d::PhaseLine::Closing(value) => w::OwnerChipPhase::Closing(w::OwnerChipPhaseClosing::new(
            limits,
            w::OwnerChipPhaseClosingParts { value: stage(*value, limits)? },
        )?),
        d::PhaseLine::Held { reason, revision } => w::OwnerChipPhase::Held(w::OwnerChipPhaseHeld::new(
            limits,
            w::OwnerChipPhaseHeldParts { reason: hold(*reason, limits)?, revision: *revision },
        )?),
        d::PhaseLine::Ended => w::OwnerChipPhase::Ended,
    })
}
pub fn prior(value: d::PriorPhase, limits: &w::Limits) -> Result<w::OwnerPriorPhase, w::Problem> {
    Ok(match value {
        d::PriorPhase::Waiting => w::OwnerPriorPhase::Waiting,
        d::PriorPhase::Active(value) => w::OwnerPriorPhase::Active(w::OwnerPriorPhaseActive::new(
            limits,
            w::OwnerPriorPhaseActiveParts { value: active(value, limits)? },
        )?),
        d::PriorPhase::Closing(value) => w::OwnerPriorPhase::Closing(w::OwnerPriorPhaseClosing::new(
            limits,
            w::OwnerPriorPhaseClosingParts { value: stage(value, limits)? },
        )?),
    })
}
pub fn holder(value: t::EscalationHolder, limits: &w::Limits) -> Result<w::OwnerEscalationHolder, w::Problem> {
    Ok(match value {
        t::EscalationHolder::Task(number) => w::OwnerEscalationHolder::Task(w::OwnerEscalationHolderTask::new(
            limits,
            w::OwnerEscalationHolderTaskParts { value: number },
        )?),
        t::EscalationHolder::Person(number) => w::OwnerEscalationHolder::Person(w::OwnerEscalationHolderPerson::new(
            limits,
            w::OwnerEscalationHolderPersonParts { value: number },
        )?),
        t::EscalationHolder::Role { project, role } => w::OwnerEscalationHolder::Role(
            w::OwnerEscalationHolderRole::new(limits, w::OwnerEscalationHolderRoleParts { project, role })?,
        ),
    })
}
pub fn escalation(value: &d::EscalationState, limits: &w::Limits) -> Result<w::OwnerHoldEscalation, w::Problem> {
    Ok(match value {
        d::EscalationState::Routing => w::OwnerHoldEscalation::Routing,
        d::EscalationState::Waiting { holder: value, entry, since } => {
            w::OwnerHoldEscalation::Waiting(w::OwnerHoldEscalationWaiting::new(
                limits,
                w::OwnerHoldEscalationWaitingParts {
                    holder: holder(*value, limits)?,
                    entry: *entry,
                    since: since.as_nanos(),
                },
            )?)
        }
        d::EscalationState::Rejected { by, reason } => {
            w::OwnerHoldEscalation::Rejected(w::OwnerHoldEscalationRejected::new(
                limits,
                w::OwnerHoldEscalationRejectedParts { by: *by, reason: owner::text(reason, limits)? },
            )?)
        }
    })
}
pub fn tries(value: t::Tries, limits: &w::Limits) -> Result<w::OwnerFailureTries, w::Problem> {
    w::OwnerFailureTries::new(
        limits,
        w::OwnerFailureTriesParts {
            transient: value.transient,
            permanent: value.permanent,
            run: value.run,
            agent: value.agent,
            lost: value.lost,
            invalid: value.invalid,
        },
    )
}
pub fn held(value: &d::HoldLine, limits: &w::Limits) -> Result<w::OwnerChipHold, w::Problem> {
    w::OwnerChipHold::new(
        limits,
        w::OwnerChipHoldParts {
            words: match &value.words {
                Some(text) => Some(owner::text(text, limits)?),
                None => None,
            },
            placed: match &value.placed {
                Some(value) => Some(placed(value, limits)?),
                None => None,
            },
            circle: circle(value.circle.as_ref(), limits)?,
            account: match &value.account {
                Some(item) => Some(owner::part(item, limits)?),
                None => None,
            },
            source: match &value.source {
                Some(item) => Some(owner::part(item, limits)?),
                None => None,
            },
            reason: hold(value.reason, limits)?,
            revision: value.revision,
            before: prior(value.before, limits)?,
            escalation: match &value.escalation {
                Some(item) => Some(escalation(item, limits)?),
                None => None,
            },
            tries: tries(value.tries, limits)?,
            hold_wait_since: match value.hold_wait_since {
                Some(item) => Some(item.as_nanos()),
                None => None,
            },
        },
    )
}
fn placed(value: &d::PlacedLine, limits: &w::Limits) -> Result<w::PlacedLine, w::Problem> {
    w::PlacedLine::new(
        limits,
        w::PlacedLineParts {
            number: value.number,
            owner: super::refusals::hold_owner(value.owner, limits)?,
            mode: match value.mode {
                t::Mode::Stop => w::HoldMode::Stop,
                t::Mode::Drain => w::HoldMode::Drain,
            },
            scope: match value.scope {
                t::Scope::Subtree { task } => {
                    w::HoldScope::Subtree(w::HoldScopeSubtree::new(limits, w::HoldScopeSubtreeParts { task })?)
                }
                t::Scope::Project => w::HoldScope::Project,
            },
            reason: value.reason.clone(),
            placed_at: value.placed_at.as_nanos(),
            revision: value.revision,
            lifted: value.lifted,
            may_lift: value.may_lift,
        },
    )
}
pub fn writer(value: t::Writer, limits: &w::Limits) -> Result<w::OwnerResourceWriter, w::Problem> {
    Ok(match value {
        t::Writer::Run { task, attempt } => w::OwnerResourceWriter::Run(w::OwnerResourceWriterRun::new(
            limits,
            w::OwnerResourceWriterRunParts { task, attempt },
        )?),
        t::Writer::Effect { entry } => w::OwnerResourceWriter::Effect(w::OwnerResourceWriterEffect::new(
            limits,
            w::OwnerResourceWriterEffectParts { entry },
        )?),
    })
}
pub fn resource(value: &d::ResourceLine, limits: &w::Limits) -> Result<w::OwnerResourceLine, w::Problem> {
    w::OwnerResourceLine::new(
        limits,
        w::OwnerResourceLineParts {
            part: owner::part(&value.part, limits)?,
            kind: value.kind,
            taken: value.taken,
            holder: value.holder,
            writer: match value.writer {
                Some(item) => Some(writer(item, limits)?),
                None => None,
            },
            writer_lost: value.writer_lost,
        },
    )
}
pub fn resources_pieces(rows: &List<w::OwnerResourceLine>, bound: u32, path: w::Path) -> Result<Box<[u8]>, w::Problem> {
    let mut encoded = List::with_capacity(rows.len());
    for row in rows {
        let mut out = skein_lib::Writer::new(usize::try_from(row.measure()).expect("u32 record measure fits usize"));
        match row.encode(&mut out) {
            Ok(()) => (),
            Err(_) => return Err(problem(skein_codec::Reason::Bound)),
        }
        encoded.push(out.finish()).expect("supplied piece count");
    }
    pieces(&encoded, bound, path)
}
pub fn work(value: &d::ChipWork, limits: &w::Limits) -> Result<w::OwnerChipWork, w::Problem> {
    Ok(match value {
        d::ChipWork::Preparing => w::OwnerChipWork::Preparing,
        d::ChipWork::Starting => w::OwnerChipWork::Starting,
        d::ChipWork::Active => w::OwnerChipWork::Active,
        d::ChipWork::Writing => w::OwnerChipWork::Writing,
        d::ChipWork::Waiting => w::OwnerChipWork::Waiting,
        d::ChipWork::Ending => w::OwnerChipWork::Ending,
        d::ChipWork::Tool { call, tool } => w::OwnerChipWork::Tool(w::OwnerChipWorkTool::new(
            limits,
            w::OwnerChipWorkToolParts { call: bytes::copy_of(call), tool: bytes::copy_of(tool) },
        )?),
    })
}
pub fn observation(value: &d::ChipObservation, limits: &w::Limits) -> Result<w::OwnerChipObservation, w::Problem> {
    w::OwnerChipObservation::new(
        limits,
        w::OwnerChipObservationParts {
            channel: value.channel.raw(),
            work: work(&value.work, limits)?,
            since: value.since.as_nanos(),
            reported_at: value.reported_at.as_nanos(),
            started: match value.started {
                Some(item) => Some(item.as_nanos()),
                None => None,
            },
        },
    )
}
pub fn person_notice(value: t::PersonStallNotice, limits: &w::Limits) -> Result<w::OwnerPersonStallNotice, w::Problem> {
    w::OwnerPersonStallNotice::new(
        limits,
        w::OwnerPersonStallNoticeParts { message: value.message, at: value.at.as_nanos(), activity: value.activity },
    )
}
pub fn person_wait(value: t::PersonWait, limits: &w::Limits) -> Result<w::OwnerPersonWait, w::Problem> {
    w::OwnerPersonWait::new(
        limits,
        w::OwnerPersonWaitParts {
            since: value.since.as_nanos(),
            stall: value.stall,
            deadline: value.deadline.as_nanos(),
            notice: match value.notice {
                Some(item) => Some(person_notice(item, limits)?),
                None => None,
            },
        },
    )
}
fn compact(value: &d::PhaseLine) -> w::Phase {
    match value {
        d::PhaseLine::Waiting { .. } => w::Phase::Waiting,
        d::PhaseLine::Active(value) => match value {
            t::Active::Idle => w::Phase::Idle,
            t::Active::Due => w::Phase::Due,
            t::Active::Preparing => w::Phase::Preparing,
            t::Active::Claimed { .. } => w::Phase::Claimed,
            t::Active::Running { .. } => w::Phase::Running,
            t::Active::BackingOff { .. } => w::Phase::BackingOff,
        },
        d::PhaseLine::Person { .. } => w::Phase::Active,
        d::PhaseLine::Closing(_) => w::Phase::Closing,
        d::PhaseLine::Held { .. } => w::Phase::Held,
        d::PhaseLine::Ended => w::Phase::Ended,
    }
}
fn chip_executor(value: &d::Chip, limits: &w::Limits) -> Result<w::Executor, w::Problem> {
    Ok(match value.details.executor {
        t::Executor::Agent { charter } => {
            if value.executor != d::ExecutorKind::Agent {
                return Err(problem(skein_codec::Reason::Malformed));
            }
            w::Executor::Agent(w::ExecutorAgent::new(limits, w::ExecutorAgentParts { charter })?)
        }
        t::Executor::Procedure { connector, code } => {
            if value.executor != d::ExecutorKind::Procedure {
                return Err(problem(skein_codec::Reason::Malformed));
            }
            w::Executor::Procedure(w::ExecutorProcedure::new(
                limits,
                w::ExecutorProcedureParts { connector, kind: code },
            )?)
        }
        t::Executor::Person(address) => {
            if value.executor != d::ExecutorKind::Person {
                return Err(problem(skein_codec::Reason::Malformed));
            }
            match address {
                t::PersonAddress::Person(party) => {
                    w::Executor::Party(w::ExecutorParty::new(limits, w::ExecutorPartyParts { party })?)
                }
                t::PersonAddress::Role(role) => w::Executor::Role(w::ExecutorRole::new(
                    limits,
                    w::ExecutorRoleParts { project: value.details.project, role },
                )?),
            }
        }
    })
}
pub fn chip(value: &d::Chip, limits: &w::Limits) -> Result<w::Chip, w::Problem> {
    let mut facts = List::with_capacity(5);
    facts
        .push(w::Fact::Chip(w::ChipRow::new(
            limits,
            w::ChipRowParts {
                parent: None,
                value: w::ChipFields::new(
                    limits,
                    w::ChipFieldsParts {
                        task: value.number,
                        title: owner::text(&value.title, limits)?,
                        executor: chip_executor(value, limits)?,
                        phase: compact(&value.phase),
                        details: 1,
                    },
                )?,
            },
        )?))
        .expect("chip root");
    facts
        .push(w::Fact::ChipPhase(w::ChipPhaseRow::new(
            limits,
            w::ChipPhaseRowParts {
                parent: Some(0),
                value: w::ChipPhaseFields::Owner(w::ChipPhaseOwnerFields::new(
                    limits,
                    w::ChipPhaseOwnerFieldsParts { phase: phase(&value.phase, limits)?, details: 2 },
                )?),
            },
        )?))
        .expect("chip phase");
    let ending = match &value.details.ending {
        Some(_) => Some(3),
        None => None,
    };
    facts
        .push(w::Fact::OwnerChipDetails(Box::new(w::OwnerChipDetailsRow::new(
            limits,
            w::OwnerChipDetailsRowParts { parent: Some(1), value: details_fields(&value.details, ending, limits)? },
        )?)))
        .expect("chip details");
    if let Some(ending) = &value.details.ending {
        append_ending(ending, Some(2), &mut facts, limits)?;
    }
    w::Chip::new(
        limits,
        w::ChipParts { rows: fact_pieces(&facts, limits.chip_rows.min(w::CEILINGS.chip_rows), w::Path::ChipRows)? },
    )
}
fn details_fields(
    value: &d::ChipDetails,
    ending: Option<u32>,
    limits: &w::Limits,
) -> Result<w::OwnerChipDetailsFields, w::Problem> {
    let mut resources = List::with_capacity(value.resources.len());
    for item in &value.resources {
        resources.push(resource(item, limits)?).expect("supplied resource rows");
    }
    w::OwnerChipDetailsFields::new(
        limits,
        w::OwnerChipDetailsFieldsParts {
            dependencies: dependencies(&value.dependencies, limits)?,
            project: value.project,
            executor: owner::executor(value.executor, limits)?,
            phase_since: value.phase_since.as_nanos(),
            run_since: match value.run_since {
                Some(item) => Some(item.as_nanos()),
                None => None,
            },
            attempt: value.attempt,
            retry: match value.retry {
                Some(item) => Some(retry(item, limits)?),
                None => None,
            },
            wake: owner::wake(value.wake, limits)?,
            person_wait: match value.person_wait {
                Some(item) => Some(person_wait(item, limits)?),
                None => None,
            },
            hold: match &value.hold {
                Some(item) => Some(held(item, limits)?),
                None => None,
            },
            budget: owner::budget(&value.budget, limits)?,
            activity: value.activity,
            resources: resources_pieces(
                &resources,
                limits.owner_chip_details_fields_resources.min(w::CEILINGS.owner_chip_details_fields_resources),
                w::Path::OwnerChipDetailsFieldsResources,
            )?,
            resources_cut: value.resources_cut,
            resources_observed: value.resources_observed,
            procedure: match &value.procedure {
                Some(item) => Some(owner::part(item, limits)?),
                None => None,
            },
            live_delegates: value.live_delegates,
            closing_connectors: value.closing_connectors.clone(),
            ending,
            ended_at: match value.ended_at {
                Some(item) => Some(item.as_nanos()),
                None => None,
            },
            observation: match &value.observation {
                Some(item) => Some(observation(item, limits)?),
                None => None,
            },
        },
    )
}

pub fn ending(value: &d::Ending, limits: &w::Limits) -> Result<w::OwnerChipEnding, w::Problem> {
    let mut facts = List::with_capacity(2);
    append_ending(value, None, &mut facts, limits)?;
    w::OwnerChipEnding::new(
        limits,
        w::OwnerChipEndingParts {
            rows: fact_pieces(
                &facts,
                limits.owner_chip_ending_rows.min(w::CEILINGS.owner_chip_ending_rows),
                w::Path::OwnerChipEndingRows,
            )?,
        },
    )
}
pub(super) fn append_ending(
    value: &d::Ending,
    parent: Option<u32>,
    facts: &mut List<w::Fact>,
    limits: &w::Limits,
) -> Result<(), w::Problem> {
    let index = facts.len();
    let child = index.checked_add(1).ok_or(problem(skein_codec::Reason::Bound))?;
    let fields = match value {
        d::Ending::Done(_) => w::OwnerChipEndingFields::Done(w::OwnerChipEndingDoneFields::new(
            limits,
            w::OwnerChipEndingDoneFieldsParts { value: child },
        )?),
        d::Ending::Failed(value) => w::OwnerChipEndingFields::Failed(w::OwnerChipEndingFailedFields::new(
            limits,
            w::OwnerChipEndingFailedFieldsParts { value: owner::text(value, limits)? },
        )?),
        d::Ending::Cancelled { reason, result, by, at } => {
            w::OwnerChipEndingFields::Cancelled(w::OwnerChipEndingCancelledFields::new(
                limits,
                w::OwnerChipEndingCancelledFieldsParts {
                    reason: owner::text(reason, limits)?,
                    result: match result {
                        Some(_) => Some(child),
                        None => None,
                    },
                    by: owner::requester(*by, limits)?,
                    at: at.as_nanos(),
                },
            )?)
        }
    };
    facts
        .push(w::Fact::OwnerChipEnding(w::OwnerChipEndingRow::new(
            limits,
            w::OwnerChipEndingRowParts { parent, value: fields },
        )?))
        .expect("bounded ending facts");
    let result = match value {
        d::Ending::Done(value) => Some(value),
        d::Ending::Failed(_) => None,
        d::Ending::Cancelled { result, .. } => result.as_ref(),
    };
    if let Some(value) = result {
        facts
            .push(w::Fact::ResultValue(w::ResultValueRow::new(
                limits,
                w::ResultValueRowParts { parent: Some(index), value: result_fields(value, limits)? },
            )?))
            .expect("bounded result facts");
    }
    Ok(())
}
pub(super) fn result_fields(value: &d::ResultValue, limits: &w::Limits) -> Result<w::ResultValueFields, w::Problem> {
    Ok(match value {
        d::ResultValue::Approved { state, words } => w::ResultValueFields::Approved(w::ResultValueApprovedFields::new(
            limits,
            w::ResultValueApprovedFieldsParts {
                state: bytes::copy_of(state),
                words: match words {
                    Some(item) => Some(owner::text(item, limits)?),
                    None => None,
                },
            },
        )?),
        d::ResultValue::Report(value) => w::ResultValueFields::Report(w::ResultValueReportFields::new(
            limits,
            w::ResultValueReportFieldsParts { value: owner::text(value, limits)? },
        )?),
        d::ResultValue::Failure(value) => w::ResultValueFields::Failure(w::ResultValueFailureFields::new(
            limits,
            w::ResultValueFailureFieldsParts { value: owner::text(value, limits)? },
        )?),
        d::ResultValue::Change { kind, part, words } => w::ResultValueFields::Change(w::ResultValueChangeFields::new(
            limits,
            w::ResultValueChangeFieldsParts {
                kind: *kind,
                part: owner::part(part, limits)?,
                words: owner::text(words, limits)?,
            },
        )?),
        d::ResultValue::Verdict { code, fields, words } => {
            let mut copied = List::with_capacity(super::count(fields.len())?);
            for item in fields {
                copied
                    .push(w::ResultField::new(
                        limits,
                        w::ResultFieldParts { name: bytes::copy_of(&item.name), value: bytes::copy_of(&item.value) },
                    )?)
                    .expect("supplied result fields");
            }
            w::ResultValueFields::OwnerVerdict(w::ResultValueOwnerVerdictFields::new(
                limits,
                w::ResultValueOwnerVerdictFieldsParts {
                    verdict: *code,
                    words: owner::text(words, limits)?,
                    fields: copied,
                },
            )?)
        }
    })
}

pub fn fact_pieces(rows: &List<w::Fact>, bound: u32, path: w::Path) -> Result<Box<[u8]>, w::Problem> {
    let mut encoded = List::with_capacity(rows.len());
    for row in rows {
        let mut out = skein_lib::Writer::new(usize::try_from(row.measure()).expect("u32 record measure fits usize"));
        match row.encode(&mut out) {
            Ok(()) => (),
            Err(_) => return Err(problem(skein_codec::Reason::Bound)),
        }
        encoded.push(out.finish()).expect("supplied piece count");
    }
    pieces(&encoded, bound, path)
}

fn circle(value: Option<&t::Circle>, limits: &w::Limits) -> Result<Option<w::Circle>, w::Problem> {
    let Some(value) = value else { return Ok(None) };
    let mut tasks = List::with_capacity(u32::try_from(value.tasks.len()).expect("bounded circle"));
    for task in &value.tasks {
        tasks.push(*task).expect("same circle");
    }
    Ok(Some(w::Circle::new(limits, w::CircleParts { tasks, cut: value.cut })?))
}
fn status(value: t::Status) -> w::OwnerTaskStatus {
    match value {
        t::Status::Done => w::OwnerTaskStatus::Done,
        t::Status::Failed => w::OwnerTaskStatus::Failed,
        t::Status::Cancelled => w::OwnerTaskStatus::Cancelled,
    }
}
pub(crate) fn dependencies(
    values: &List<d::Dependency>,
    limits: &w::Limits,
) -> Result<List<w::Dependency>, w::Problem> {
    let mut rows = List::with_capacity(values.len());
    for value in values {
        let added = match &value.added {
            Some(added) => Some(w::DependencyAdded::new(
                limits,
                w::DependencyAddedParts { at: added.at.as_nanos(), by: owner::requester(added.by, limits)? },
            )?),
            None => None,
        };
        rows.push(w::Dependency::new(
            limits,
            w::DependencyParts {
                number: value.number,
                pending: value.pending,
                added,
                keeps: value.keeps,
                status: match value.status {
                    Some(value) => Some(status(value)),
                    None => None,
                },
            },
        )?)
        .expect("same dependency count");
    }
    Ok(rows)
}
