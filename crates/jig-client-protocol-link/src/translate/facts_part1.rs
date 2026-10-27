//! Field-for-field client contract translation (protocol/client.md, 3 and 5).
use super::{
    Child, active_run, closing_stage, document_text, failure_class, hold_category, owner_chip_phase, part,
    person_address, recipient, revision, run_stage, take_closing, take_ending, take_held_runtime, take_hold_reason,
    take_owner_chip_details, take_owner_chip_ending, taken_by, wake_policy,
};
use crate::Limits;
use crate::translate::text_problem;
use alloc::boxed::Box;
use jig_client_domain::contract as c;
use jig_client_protocol_wire::v19 as w;
use skein_lib::Map;
use skein_lib::{List, Wall, bytes};
#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn chip_phase_fields(
    value: &w::ChipPhaseFields,
    limits: &Limits,
    index: u32,
    children: &mut Map<u32, Child>,
) -> Result<c::ChipPhase, w::Problem> {
    Ok(match value {
        w::ChipPhaseFields::Owner(row) => c::ChipPhase::Owner {
            phase: owner_chip_phase(row.phase(), limits),
            details: Box::new(take_owner_chip_details(row.details(), index, children)?),
        },
        w::ChipPhaseFields::Active(row) => c::ChipPhase::Active(active_run(row.value(), limits)?),
        w::ChipPhaseFields::Waiting(row) => {
            let mut list0 = List::with_capacity(row.dependencies().len());
            for item in row.dependencies() {
                list0.push(*item).expect("reserved supplied list");
            }
            let opt1 = match &row.hold() {
                Some(item) => Some(part(item, limits)),
                None => None,
            };
            c::ChipPhase::Waiting { dependencies: list0, hold: opt1 }
        }
        w::ChipPhaseFields::Idle(row) => c::ChipPhase::Idle { wake: wake_policy(row.wake(), limits)? },
        w::ChipPhaseFields::Starting => c::ChipPhase::Starting,
        w::ChipPhaseFields::Running(row) => {
            let opt0 = match &row.tool() {
                Some(item) => Some(match c::Text::new(bytes::copy_of(item), limits.client.text) {
                    Ok(value) => value,
                    Err(problem) => return Err(text_problem(problem)),
                }),
                None => None,
            };
            c::ChipPhase::Running { attempt: row.attempt(), since: Wall::from_nanos(row.since()), tool: opt0 }
        }
        w::ChipPhaseFields::WaitingForWords(row) => {
            c::ChipPhase::WaitingForWords { attempt: row.attempt(), since: Wall::from_nanos(row.since()) }
        }
        w::ChipPhaseFields::BackingOff(row) => c::ChipPhase::BackingOff {
            at: Wall::from_nanos(row.at()),
            attempt: row.attempt(),
            tries: row.tries(),
            class: failure_class(row.class(), limits),
            reason: match c::Text::new(bytes::copy_of(row.reason()), limits.client.text) {
                Ok(value) => value,
                Err(problem) => return Err(text_problem(problem)),
            },
        },
        w::ChipPhaseFields::Procedure(row) => c::ChipPhase::Procedure { panel: part(row.panel(), limits) },
        w::ChipPhaseFields::Person(row) => {
            let opt0 = match &row.taken() {
                Some(item) => Some(taken_by(item, limits)?),
                None => None,
            };
            let opt1 = match row.since() {
                Some(item) => Some(Wall::from_nanos(*item)),
                None => None,
            };
            c::ChipPhase::Person { addressed: recipient(row.addressed(), limits), taken: opt0, since: opt1 }
        }
        w::ChipPhaseFields::Closing(row) => c::ChipPhase::Closing(take_closing(row.value(), index, children)?),
        w::ChipPhaseFields::Held(row) => {
            let opt0 = match &row.was() {
                Some(item) => Some(take_held_runtime(*item, index, children)?),
                None => None,
            };
            c::ChipPhase::Held {
                reason: take_hold_reason(row.reason(), index, children)?,
                since: Wall::from_nanos(row.since()),
                revision: revision(row.revision(), limits),
                was: opt0,
            }
        }
        w::ChipPhaseFields::Ended(row) => {
            c::ChipPhase::Ended { how: take_ending(row.how(), index, children)?, at: Wall::from_nanos(row.at()) }
        }
    })
}

#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn closing_fields(
    value: &w::ClosingFields,
    limits: &Limits,
    index: u32,
    children: &mut Map<u32, Child>,
) -> Result<c::Closing, w::Problem> {
    let opt0 = match &value.connectors() {
        Some(item) => {
            let mut list0 = List::with_capacity((item).len());
            for item in item {
                list0.push(*item).expect("reserved supplied list");
            }
            Some(list0)
        }
        None => None,
    };
    let opt1 = match &value.stage() {
        Some(item) => Some(closing_stage(item, limits)),
        None => None,
    };
    let opt2 = match &value.ending() {
        Some(item) => Some(take_ending(*item, index, children)?),
        None => None,
    };
    Ok(c::Closing { delegates: value.delegates(), connectors: opt0, stage: opt1, ending: opt2 })
}

pub(crate) fn actor(value: &w::Actor, _limits: &Limits) -> c::Actor {
    match value {
        w::Actor::Party(row) => c::Actor::Party(row.value()),
        w::Actor::Task(row) => c::Actor::Task(row.value()),
        w::Actor::Deployment => c::Actor::Deployment,
        w::Actor::ProjectDeployment(row) => c::Actor::ProjectDeployment { project: row.project() },
    }
}

pub(crate) fn party_kind(value: &w::PartyKind, _limits: &Limits) -> c::PartyKind {
    match value {
        w::PartyKind::Person => c::PartyKind::Person,
        w::PartyKind::Service => c::PartyKind::Service,
    }
}

pub(crate) fn party(value: &w::Party, limits: &Limits) -> Result<c::Party, w::Problem> {
    Ok(c::Party {
        number: value.number(),
        kind: party_kind(value.kind(), limits),
        name: match c::Text::new(bytes::copy_of(value.name()), limits.client.text) {
            Ok(value) => value,
            Err(problem) => return Err(text_problem(problem)),
        },
    })
}

#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn decision(value: &w::Decision, limits: &Limits) -> Result<c::Decision, w::Problem> {
    Ok(match value {
        w::Decision::Accepted => c::Decision::Accepted,
        w::Decision::Rejected(row) => c::Decision::Rejected {
            reason: match c::Text::new(bytes::copy_of(row.reason()), limits.client.text) {
                Ok(value) => value,
                Err(problem) => return Err(text_problem(problem)),
            },
        },
        w::Decision::PassedUp => c::Decision::PassedUp,
        w::Decision::Released(row) => {
            let opt0 = match row.again() {
                Some(item) => Some(*item),
                None => None,
            };
            c::Decision::Released { again: opt0 }
        }
        w::Decision::LeftHeld(row) => c::Decision::LeftHeld {
            reason: match c::Text::new(bytes::copy_of(row.reason()), limits.client.text) {
                Ok(value) => value,
                Err(problem) => return Err(text_problem(problem)),
            },
        },
    })
}

#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn ending_fields(
    value: &w::EndingFields,
    limits: &Limits,
    index: u32,
    children: &mut Map<u32, Child>,
) -> Result<c::Ending, w::Problem> {
    Ok(match value {
        w::EndingFields::Task(row) => c::Ending::Task(Box::new(take_owner_chip_ending(row.value(), index, children)?)),
        w::EndingFields::Done => c::Ending::Done,
        w::EndingFields::Failed(row) => {
            let opt0 = match &row.reason() {
                Some(item) => Some(match c::Text::new(bytes::copy_of(item), limits.client.text) {
                    Ok(value) => value,
                    Err(problem) => return Err(text_problem(problem)),
                }),
                None => None,
            };
            c::Ending::Failed { reason: opt0 }
        }
        w::EndingFields::Cancelled(row) => {
            let opt0 = match &row.reason() {
                Some(item) => Some(match c::Text::new(bytes::copy_of(item), limits.client.text) {
                    Ok(value) => value,
                    Err(problem) => return Err(text_problem(problem)),
                }),
                None => None,
            };
            let opt1 = match row.at() {
                Some(item) => Some(Wall::from_nanos(*item)),
                None => None,
            };
            c::Ending::Cancelled { by: actor(row.by(), limits), reason: opt0, at: opt1 }
        }
        w::EndingFields::Withdrawn(row) => c::Ending::Withdrawn {
            reason: match c::Text::new(bytes::copy_of(row.reason()), limits.client.text) {
                Ok(value) => value,
                Err(problem) => return Err(text_problem(problem)),
            },
        },
        w::EndingFields::Decided(row) => {
            let opt0 = match &row.party() {
                Some(item) => Some(party(item, limits)?),
                None => None,
            };
            let opt1 = match &row.how() {
                Some(item) => Some(decision(item, limits)?),
                None => None,
            };
            c::Ending::Decided { by: row.by(), party: opt0, how: opt1, at: Wall::from_nanos(row.at()) }
        }
        w::EndingFields::Deleted => c::Ending::Deleted,
    })
}

pub(crate) fn held_runtime_fields(
    value: &w::HeldRuntimeFields,
    limits: &Limits,
    index: u32,
    children: &mut Map<u32, Child>,
) -> Result<c::HeldRuntime, w::Problem> {
    Ok(match value {
        w::HeldRuntimeFields::Waiting => c::HeldRuntime::Waiting,
        w::HeldRuntimeFields::Active(row) => c::HeldRuntime::Active(active_run(row.value(), limits)?),
        w::HeldRuntimeFields::Closing(row) => c::HeldRuntime::Closing(take_closing(row.value(), index, children)?),
    })
}

pub(crate) fn attempt_failure(value: &w::AttemptFailure, limits: &Limits) -> Result<c::AttemptFailure, w::Problem> {
    Ok(c::AttemptFailure {
        attempt: value.attempt(),
        class: failure_class(value.class(), limits),
        reason: match c::Text::new(bytes::copy_of(value.reason()), limits.client.text) {
            Ok(value) => value,
            Err(problem) => return Err(text_problem(problem)),
        },
        at: Wall::from_nanos(value.at()),
    })
}

pub(crate) fn hold_reason_fields(
    value: &w::HoldReasonFields,
    limits: &Limits,
    index: u32,
    children: &mut Map<u32, Child>,
) -> Result<c::HoldReason, w::Problem> {
    Ok(match value {
        w::HoldReasonFields::Tries(row) => {
            let mut list0 = List::with_capacity(row.failures().len());
            for item in row.failures() {
                list0.push(attempt_failure(item, limits)?).expect("reserved supplied list");
            }
            c::HoldReason::Tries { failures: list0 }
        }
        w::HoldReasonFields::Stalled(row) => c::HoldReason::Stalled {
            since: Wall::from_nanos(row.since()),
            waiting: match c::Text::new(bytes::copy_of(row.waiting()), limits.client.text) {
                Ok(value) => value,
                Err(problem) => return Err(text_problem(problem)),
            },
        },
        w::HoldReasonFields::Dependency(row) => {
            c::HoldReason::Dependency { task: row.task(), how: take_ending(row.how(), index, children)? }
        }
        w::HoldReasonFields::Drift(row) => c::HoldReason::Drift { part: part(row.part(), limits) },
        w::HoldReasonFields::EffectFailed(row) => c::HoldReason::EffectFailed { part: part(row.part(), limits) },
        w::HoldReasonFields::EffectUncertain(row) => c::HoldReason::EffectUncertain { part: part(row.part(), limits) },
        w::HoldReasonFields::Budget(row) => c::HoldReason::Budget { spent: row.spent(), limit: row.limit() },
        w::HoldReasonFields::Deadline(row) => c::HoldReason::Deadline { at: Wall::from_nanos(row.at()) },
        w::HoldReasonFields::Stopped(row) => {
            c::HoldReason::Stopped { by: party(row.by(), limits)?, at: Wall::from_nanos(row.at()) }
        }
        w::HoldReasonFields::Resource(row) => {
            c::HoldReason::Resource { resource: part(row.resource(), limits), holder: actor(row.holder(), limits) }
        }
        w::HoldReasonFields::Pool(row) => c::HoldReason::Pool { part: part(row.part(), limits) },
        w::HoldReasonFields::Procedure(row) => c::HoldReason::Procedure { part: part(row.part(), limits) },
    })
}

pub(crate) fn owner_executor(value: &w::OwnerExecutor, limits: &Limits) -> c::OwnerExecutor {
    match value {
        w::OwnerExecutor::Agent(row) => c::OwnerExecutor::Agent { charter: row.charter() },
        w::OwnerExecutor::Procedure(row) => {
            c::OwnerExecutor::Procedure { connector: row.connector(), code: row.code() }
        }
        w::OwnerExecutor::Person(row) => c::OwnerExecutor::Person(person_address(row.value(), limits)),
    }
}

pub(crate) fn wake_rule(value: &w::WakeRule, _limits: &Limits) -> c::WakeRule {
    match value {
        w::WakeRule::Never => c::WakeRule::Never,
        w::WakeRule::Immediate => c::WakeRule::Immediate,
        w::WakeRule::Batch(row) => c::WakeRule::Batch { count: row.count(), age: row.age() },
    }
}

pub(crate) fn results_wake(value: &w::ResultsWake, _limits: &Limits) -> c::ResultsWake {
    match value {
        w::ResultsWake::Never => c::ResultsWake::Never,
        w::ResultsWake::Each => c::ResultsWake::Each,
        w::ResultsWake::LastOrFailure => c::ResultsWake::LastOrFailure,
    }
}

pub(crate) fn owner_wake_policy(value: &w::OwnerWakePolicy, limits: &Limits) -> c::OwnerWakePolicy {
    c::OwnerWakePolicy {
        words: wake_rule(value.words(), limits),
        notices: wake_rule(value.notices(), limits),
        news: wake_rule(value.news(), limits),
        results: results_wake(value.results(), limits),
        questions: value.questions(),
        answers: value.answers(),
        timers: value.timers(),
    }
}

pub(crate) fn owner_person_stall_notice(
    value: &w::OwnerPersonStallNotice,
    _limits: &Limits,
) -> c::OwnerPersonStallNotice {
    c::OwnerPersonStallNotice { message: value.message(), at: Wall::from_nanos(value.at()), activity: value.activity() }
}

#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn owner_person_wait(value: &w::OwnerPersonWait, limits: &Limits) -> c::OwnerPersonWait {
    let opt0 = match &value.notice() {
        Some(item) => Some(owner_person_stall_notice(item, limits)),
        None => None,
    };
    c::OwnerPersonWait {
        since: Wall::from_nanos(value.since()),
        stall: value.stall(),
        deadline: Wall::from_nanos(value.deadline()),
        notice: opt0,
    }
}

pub(crate) fn owner_part_kind(value: &w::OwnerPartKind, _limits: &Limits) -> c::OwnerPartKind {
    match value {
        w::OwnerPartKind::Resource => c::OwnerPartKind::Resource,
        w::OwnerPartKind::Pool => c::OwnerPartKind::Pool,
        w::OwnerPartKind::Result => c::OwnerPartKind::Result,
        w::OwnerPartKind::Procedure => c::OwnerPartKind::Procedure,
        w::OwnerPartKind::Effect => c::OwnerPartKind::Effect,
        w::OwnerPartKind::Outbox => c::OwnerPartKind::Outbox,
        w::OwnerPartKind::Hold => c::OwnerPartKind::Hold,
    }
}

pub(crate) fn owner_part_key(value: &w::OwnerPartKey, _limits: &Limits) -> c::OwnerPartKey {
    match value {
        w::OwnerPartKey::Path(row) => {
            let mut list0 = List::with_capacity(row.value().len());
            for item in row.value() {
                list0.push(bytes::copy_of(item)).expect("reserved supplied list");
            }
            c::OwnerPartKey::Path(list0)
        }
        w::OwnerPartKey::Number(row) => c::OwnerPartKey::Number(row.value()),
    }
}

pub(crate) fn owner_part(value: &w::OwnerPart, limits: &Limits) -> c::OwnerPart {
    c::OwnerPart {
        connector: value.connector(),
        kind: owner_part_kind(value.kind(), limits),
        key: owner_part_key(value.key(), limits),
    }
}

pub(crate) fn owner_prior_phase(value: &w::OwnerPriorPhase, limits: &Limits) -> c::OwnerPriorPhase {
    match value {
        w::OwnerPriorPhase::Waiting => c::OwnerPriorPhase::Waiting,
        w::OwnerPriorPhase::Active(row) => c::OwnerPriorPhase::Active(run_stage(row.value(), limits)),
        w::OwnerPriorPhase::Closing(row) => c::OwnerPriorPhase::Closing(closing_stage(row.value(), limits)),
    }
}

pub(crate) fn owner_escalation_holder(value: &w::OwnerEscalationHolder, _limits: &Limits) -> c::OwnerEscalationHolder {
    match value {
        w::OwnerEscalationHolder::Task(row) => c::OwnerEscalationHolder::Task(row.value()),
        w::OwnerEscalationHolder::Person(row) => c::OwnerEscalationHolder::Person(row.value()),
        w::OwnerEscalationHolder::Role(row) => {
            c::OwnerEscalationHolder::Role { project: row.project(), role: row.role() }
        }
    }
}

pub(crate) fn owner_hold_escalation(
    value: &w::OwnerHoldEscalation,
    limits: &Limits,
) -> Result<c::OwnerHoldEscalation, w::Problem> {
    Ok(match value {
        w::OwnerHoldEscalation::Routing => c::OwnerHoldEscalation::Routing,
        w::OwnerHoldEscalation::Waiting(row) => c::OwnerHoldEscalation::Waiting {
            holder: owner_escalation_holder(row.holder(), limits),
            entry: row.entry(),
            since: Wall::from_nanos(row.since()),
        },
        w::OwnerHoldEscalation::Rejected(row) => {
            c::OwnerHoldEscalation::Rejected { by: row.by(), reason: document_text(row.reason(), limits)? }
        }
    })
}

pub(crate) fn owner_failure_tries(value: &w::OwnerFailureTries, _limits: &Limits) -> c::OwnerFailureTries {
    c::OwnerFailureTries {
        transient: value.transient(),
        permanent: value.permanent(),
        run: value.run(),
        agent: value.agent(),
        lost: value.lost(),
        invalid: value.invalid(),
    }
}

#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn owner_chip_hold(value: &w::OwnerChipHold, limits: &Limits) -> Result<c::OwnerChipHold, w::Problem> {
    let opt0 = match &value.account() {
        Some(item) => Some(owner_part(item, limits)),
        None => None,
    };
    let opt1 = match &value.source() {
        Some(item) => Some(owner_part(item, limits)),
        None => None,
    };
    let opt2 = match &value.escalation() {
        Some(item) => Some(owner_hold_escalation(item, limits)?),
        None => None,
    };
    let opt3 = match value.hold_wait_since() {
        Some(item) => Some(Wall::from_nanos(*item)),
        None => None,
    };
    Ok(c::OwnerChipHold {
        placed: crate::translate::answers::placed(value.placed().as_ref(), limits)?,
        circle: match value.circle() {
            Some(circle) => Some(c::Circle { tasks: circle.tasks().clone(), cut: circle.cut() }),
            None => None,
        },
        account: opt0,
        source: opt1,
        reason: hold_category(value.reason(), limits),
        revision: value.revision(),
        before: owner_prior_phase(value.before(), limits),
        escalation: opt2,
        tries: owner_failure_tries(value.tries(), limits),
        hold_wait_since: opt3,
    })
}

pub(crate) fn owner_numbers(value: &w::OwnerNumbers, _limits: &Limits) -> c::OwnerNumbers {
    c::OwnerNumbers {
        budget: value.budget(),
        spent: value.spent(),
        spent_below: value.spent_below(),
        reserved: value.reserved(),
    }
}
