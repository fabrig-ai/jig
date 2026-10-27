//! Task lifecycle values and bounded creation templates (protocol/store.md, 5.1).
use super::value;
use crate::tasks_v9 as wire;
use alloc::boxed::Box;
use jig_domain_tasks as domain;
use skein_lib::{List, Wall};

/// Translate `Holding` to its store value.
pub fn holding(source: &domain::Holding, limits: &wire::Limits) -> Result<wire::Holding, wire::Problem> {
    Ok(match source {
        domain::Holding::Write { resource, kind } => wire::Holding::Write(wire::WriteHolding::new(
            limits,
            wire::WriteHoldingParts { resource: value::name(resource, limits)?, kind: *kind },
        )?),
        domain::Holding::Slot { pool, kind } => wire::Holding::Slot(wire::SlotHolding::new(
            limits,
            wire::SlotHoldingParts { pool: value::name(pool, limits)?, kind: *kind },
        )?),
    })
}
/// Read `Holding` from its store value.
pub fn read_holding(source: wire::Holding, limits: &wire::Limits) -> Result<domain::Holding, wire::Problem> {
    Ok(match source {
        wire::Holding::Write(value) => {
            let wire::WriteHoldingParts { resource, kind } = value.into_parts();
            domain::Holding::Write { resource: value::read_name(resource, limits)?, kind }
        }
        wire::Holding::Slot(value) => {
            let wire::SlotHoldingParts { pool, kind } = value.into_parts();
            domain::Holding::Slot { pool: value::read_name(pool, limits)?, kind }
        }
    })
}

/// Translate `WakeRule` to its store value.
pub fn wake_rule(source: &domain::WakeRule, limits: &wire::Limits) -> Result<wire::WakeRule, wire::Problem> {
    Ok(match source {
        domain::WakeRule::Never => wire::WakeRule::Never,
        domain::WakeRule::Immediate => wire::WakeRule::Immediate,
        domain::WakeRule::Batch { count, age } => {
            wire::WakeRule::Batch(wire::WakeBatch::new(limits, wire::WakeBatchParts { count: *count, age: *age })?)
        }
    })
}
/// Read `WakeRule` from its store value.
pub fn read_wake_rule(source: wire::WakeRule, _limits: &wire::Limits) -> Result<domain::WakeRule, wire::Problem> {
    Ok(match source {
        wire::WakeRule::Never => domain::WakeRule::Never,
        wire::WakeRule::Immediate => domain::WakeRule::Immediate,
        wire::WakeRule::Batch(value) => {
            let wire::WakeBatchParts { count, age } = value.into_parts();
            domain::WakeRule::Batch { count, age }
        }
    })
}

/// Translate `ResultsWake` to its store value.
pub fn results_wake(source: &domain::ResultsWake, _limits: &wire::Limits) -> Result<wire::ResultsWake, wire::Problem> {
    Ok(match source {
        domain::ResultsWake::Never => wire::ResultsWake::Never,
        domain::ResultsWake::Each => wire::ResultsWake::Each,
        domain::ResultsWake::LastOrFailure => wire::ResultsWake::LastOrFailure,
    })
}
/// Read `ResultsWake` from its store value.
pub fn read_results_wake(
    source: wire::ResultsWake,
    _limits: &wire::Limits,
) -> Result<domain::ResultsWake, wire::Problem> {
    Ok(match source {
        wire::ResultsWake::Never => domain::ResultsWake::Never,
        wire::ResultsWake::Each => domain::ResultsWake::Each,
        wire::ResultsWake::LastOrFailure => domain::ResultsWake::LastOrFailure,
    })
}

/// Translate `WakePolicy` to its store record.
pub fn wake(source: &domain::WakePolicy, limits: &wire::Limits) -> Result<wire::WakePolicy, wire::Problem> {
    let domain::WakePolicy { words, notices, news, results, questions, answers, timers } = source;
    wire::WakePolicy::new(
        limits,
        wire::WakePolicyParts {
            words: wake_rule(words, limits)?,
            notices: wake_rule(notices, limits)?,
            news: wake_rule(news, limits)?,
            results: results_wake(results, limits)?,
            questions: *questions,
            answers: *answers,
            timers: *timers,
        },
    )
}
/// Read `WakePolicy` from its store record.
pub fn read_wake(source: wire::WakePolicy, limits: &wire::Limits) -> Result<domain::WakePolicy, wire::Problem> {
    let wire::WakePolicyParts { words, notices, news, results, questions, answers, timers } = source.into_parts();
    Ok(domain::WakePolicy {
        words: read_wake_rule(words, limits)?,
        notices: read_wake_rule(notices, limits)?,
        news: read_wake_rule(news, limits)?,
        results: read_results_wake(results, limits)?,
        questions,
        answers,
        timers,
    })
}

/// Translate `RecurringOverlap` to its store value.
pub fn overlap(
    source: &domain::RecurringOverlap,
    _limits: &wire::Limits,
) -> Result<wire::RecurringOverlap, wire::Problem> {
    Ok(match source {
        domain::RecurringOverlap::Skip => wire::RecurringOverlap::Skip,
        domain::RecurringOverlap::Wait => wire::RecurringOverlap::Wait,
    })
}
/// Read `RecurringOverlap` from its store value.
pub fn read_overlap(
    source: wire::RecurringOverlap,
    _limits: &wire::Limits,
) -> Result<domain::RecurringOverlap, wire::Problem> {
    Ok(match source {
        wire::RecurringOverlap::Skip => domain::RecurringOverlap::Skip,
        wire::RecurringOverlap::Wait => domain::RecurringOverlap::Wait,
    })
}

/// Translate `Hold` to its store value.
pub fn hold(source: &domain::Hold, limits: &wire::Limits) -> Result<wire::Hold, wire::Problem> {
    Ok(match source {
        domain::Hold::Exceeded(value) => wire::Hold::Exceeded(wire::Exceeded::new(
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
        domain::Hold::Failures(class) => wire::Hold::Failures(wire::FailedClass::new(
            limits,
            wire::FailedClassParts { class: value::class(*class) },
        )?),
        domain::Hold::Stalled => wire::Hold::Stalled,
        domain::Hold::Dependency(task) => {
            wire::Hold::Dependency(wire::Number::new(limits, wire::NumberParts { value: *task })?)
        }
        domain::Hold::Stopped => wire::Hold::Stopped,
        domain::Hold::Placed { hold } => {
            wire::Hold::Placed(wire::Number::new(limits, wire::NumberParts { value: *hold })?)
        }
        domain::Hold::Drift => wire::Hold::Drift,
        domain::Hold::Effects => wire::Hold::Effects,
        domain::Hold::EffectFailed => wire::Hold::EffectFailed,
        domain::Hold::Uncertain { entry } => {
            wire::Hold::Uncertain(wire::Effect::new(limits, wire::EffectParts { entry: *entry })?)
        }
        domain::Hold::Budget => wire::Hold::Budget,
        domain::Hold::Deadline => wire::Hold::Deadline,
        domain::Hold::HoldsWaited => wire::Hold::HoldsWaited,
        domain::Hold::ResourceUnavailable => wire::Hold::ResourceUnavailable,
        domain::Hold::ResourceAccess => wire::Hold::ResourceAccess,
        domain::Hold::PoolLost { pool } => {
            wire::Hold::PoolLost(wire::Number::new(limits, wire::NumberParts { value: *pool })?)
        }
        domain::Hold::Procedure => wire::Hold::Procedure,
        domain::Hold::Circle => wire::Hold::Circle,
    })
}
/// Read `Hold` from its store value.
pub fn read_hold(source: wire::Hold, _limits: &wire::Limits) -> Result<domain::Hold, wire::Problem> {
    Ok(match source {
        wire::Hold::Exceeded(value) => domain::Hold::Exceeded(domain::Exceeded {
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
        wire::Hold::Failures(value) => {
            let wire::FailedClassParts { class } = value.into_parts();
            domain::Hold::Failures(value::read_class(class))
        }
        wire::Hold::Stalled => domain::Hold::Stalled,
        wire::Hold::Dependency(value) => {
            let wire::NumberParts { value } = value.into_parts();
            domain::Hold::Dependency(value)
        }
        wire::Hold::Stopped => domain::Hold::Stopped,
        wire::Hold::Placed(value) => {
            let wire::NumberParts { value: hold } = value.into_parts();
            domain::Hold::Placed { hold }
        }
        wire::Hold::Drift => domain::Hold::Drift,
        wire::Hold::Effects => domain::Hold::Effects,
        wire::Hold::EffectFailed => domain::Hold::EffectFailed,
        wire::Hold::Uncertain(value) => {
            let wire::EffectParts { entry } = value.into_parts();
            domain::Hold::Uncertain { entry }
        }
        wire::Hold::Budget => domain::Hold::Budget,
        wire::Hold::Deadline => domain::Hold::Deadline,
        wire::Hold::HoldsWaited => domain::Hold::HoldsWaited,
        wire::Hold::ResourceUnavailable => domain::Hold::ResourceUnavailable,
        wire::Hold::ResourceAccess => domain::Hold::ResourceAccess,
        wire::Hold::PoolLost(value) => {
            let wire::NumberParts { value } = value.into_parts();
            domain::Hold::PoolLost { pool: value }
        }
        wire::Hold::Procedure => domain::Hold::Procedure,
        wire::Hold::Circle => domain::Hold::Circle,
    })
}

/// Translate `Active` to its store value.
pub fn active(source: &domain::Active, limits: &wire::Limits) -> Result<wire::Active, wire::Problem> {
    Ok(match source {
        domain::Active::Idle => wire::Active::Idle,
        domain::Active::Due => wire::Active::Due,
        domain::Active::Preparing => wire::Active::Preparing,
        domain::Active::Claimed { attempt } => {
            wire::Active::Claimed(wire::Number::new(limits, wire::NumberParts { value: *attempt })?)
        }
        domain::Active::Running { attempt } => {
            wire::Active::Running(wire::Number::new(limits, wire::NumberParts { value: *attempt })?)
        }
        domain::Active::BackingOff { until } => {
            wire::Active::BackingOff(wire::Number::new(limits, wire::NumberParts { value: until.as_nanos() })?)
        }
    })
}
/// Read `Active` from its store value.
pub fn read_active(source: wire::Active, _limits: &wire::Limits) -> Result<domain::Active, wire::Problem> {
    Ok(match source {
        wire::Active::Idle => domain::Active::Idle,
        wire::Active::Due => domain::Active::Due,
        wire::Active::Preparing => domain::Active::Preparing,
        wire::Active::Claimed(value) => {
            let wire::NumberParts { value } = value.into_parts();
            domain::Active::Claimed { attempt: value }
        }
        wire::Active::Running(value) => {
            let wire::NumberParts { value } = value.into_parts();
            domain::Active::Running { attempt: value }
        }
        wire::Active::BackingOff(value) => {
            let wire::NumberParts { value } = value.into_parts();
            domain::Active::BackingOff { until: Wall::from_nanos(value) }
        }
    })
}

/// Translate `Stage` to its store value.
pub fn stage(source: &domain::Stage, limits: &wire::Limits) -> Result<wire::Stage, wire::Problem> {
    Ok(match source {
        domain::Stage::Run { attempt } => {
            wire::Stage::Run(wire::Number::new(limits, wire::NumberParts { value: *attempt })?)
        }
        domain::Stage::Delegates => wire::Stage::Delegates,
        domain::Stage::Effects => wire::Stage::Effects,
        domain::Stage::Releases => wire::Stage::Releases,
        domain::Stage::Settled => wire::Stage::Settled,
    })
}
/// Read `Stage` from its store value.
pub fn read_stage(source: wire::Stage, _limits: &wire::Limits) -> Result<domain::Stage, wire::Problem> {
    Ok(match source {
        wire::Stage::Run(value) => {
            let wire::NumberParts { value } = value.into_parts();
            domain::Stage::Run { attempt: value }
        }
        wire::Stage::Delegates => domain::Stage::Delegates,
        wire::Stage::Effects => domain::Stage::Effects,
        wire::Stage::Releases => domain::Stage::Releases,
        wire::Stage::Settled => domain::Stage::Settled,
    })
}

/// Translate `Ending` to its store value.
pub fn ending(source: &domain::Ending, limits: &wire::Limits) -> Result<wire::Ending, wire::Problem> {
    Ok(match source {
        domain::Ending::Done(result) => {
            wire::Ending::Done(wire::Done::new(limits, wire::DoneParts { result: self::result(result, limits)? })?)
        }
        domain::Ending::Failed { reason } => {
            wire::Ending::Failed(wire::FailureResult::new(limits, wire::FailureResultParts { reason: reason.clone() })?)
        }
        domain::Ending::Cancelled { by, at, reason, result } => wire::Ending::Cancelled(wire::Cancelled::new(
            limits,
            wire::CancelledParts {
                by: value::party(by, limits)?,
                at: at.as_nanos(),
                reason: reason.clone(),
                result: optional_result(result.as_ref(), limits)?,
            },
        )?),
    })
}
/// Read `Ending` from its store value.
pub fn read_ending(source: wire::Ending, limits: &wire::Limits) -> Result<domain::Ending, wire::Problem> {
    Ok(match source {
        wire::Ending::Done(value) => {
            let wire::DoneParts { result } = value.into_parts();
            domain::Ending::Done(read_result(result, limits)?)
        }
        wire::Ending::Failed(value) => {
            let wire::FailureResultParts { reason } = value.into_parts();
            domain::Ending::Failed { reason }
        }
        wire::Ending::Cancelled(value) => {
            let wire::CancelledParts { by, at, reason, result } = value.into_parts();
            domain::Ending::Cancelled {
                by: value::read_party(by),
                at: Wall::from_nanos(at),
                reason,
                result: read_optional_result(result, limits)?,
            }
        }
    })
}

/// Translate `Closing` to its store record.
pub fn closing(source: &domain::Closing, limits: &wire::Limits) -> Result<wire::Closing, wire::Problem> {
    let domain::Closing { stage, ending } = source;
    wire::Closing::new(
        limits,
        wire::ClosingParts { stage: self::stage(stage, limits)?, ending: self::ending(ending, limits)? },
    )
}
/// Read `Closing` from its store record.
pub fn read_closing(source: wire::Closing, limits: &wire::Limits) -> Result<domain::Closing, wire::Problem> {
    let wire::ClosingParts { stage, ending } = source.into_parts();
    Ok(domain::Closing { stage: read_stage(stage, limits)?, ending: read_ending(ending, limits)? })
}

/// Translate `Was` to its store value.
pub fn was(source: &domain::Was, limits: &wire::Limits) -> Result<wire::Was, wire::Problem> {
    Ok(match source {
        domain::Was::Waiting => wire::Was::Waiting,
        domain::Was::Active(value) => {
            wire::Was::Active(wire::Activation::new(limits, wire::ActivationParts { value: active(value, limits)? })?)
        }
        domain::Was::Closing(value) => wire::Was::Closing(closing(value, limits)?),
    })
}
/// Read `Was` from its store value.
pub fn read_was(source: wire::Was, limits: &wire::Limits) -> Result<domain::Was, wire::Problem> {
    Ok(match source {
        wire::Was::Waiting => domain::Was::Waiting,
        wire::Was::Active(value) => {
            let wire::ActivationParts { value } = value.into_parts();
            domain::Was::Active(read_active(value, limits)?)
        }
        wire::Was::Closing(value) => domain::Was::Closing(read_closing(value, limits)?),
    })
}

/// Translate `HoldSource` to its store value.
pub fn hold_source(source: &domain::HoldSource, limits: &wire::Limits) -> Result<wire::HoldSource, wire::Problem> {
    Ok(match source {
        domain::HoldSource::Circle(circle) => wire::HoldSource::Circle(wire::Circle::new(
            limits,
            wire::CircleParts {
                tasks: references(&circle.tasks, limits.circle_tasks, wire::Path::CircleTasks, limits)?,
                cut: circle.cut,
            },
        )?),
        domain::HoldSource::Resource(value) => wire::HoldSource::Resource(value::name(value, limits)?),
        domain::HoldSource::Effect { connector, entry } => wire::HoldSource::Effect(wire::HoldEffect::new(
            limits,
            wire::HoldEffectParts { connector: *connector, entry: *entry },
        )?),
    })
}
/// Read `HoldSource` from its store value.
pub fn read_hold_source(source: wire::HoldSource, limits: &wire::Limits) -> Result<domain::HoldSource, wire::Problem> {
    Ok(match source {
        wire::HoldSource::Circle(value) => {
            let wire::CircleParts { tasks, cut } = value.into_parts();
            domain::HoldSource::Circle(domain::Circle { tasks: read_references(tasks), cut })
        }
        wire::HoldSource::Resource(value) => domain::HoldSource::Resource(value::read_name(value, limits)?),
        wire::HoldSource::Effect(value) => {
            let wire::HoldEffectParts { connector, entry } = value.into_parts();
            domain::HoldSource::Effect { connector, entry }
        }
    })
}

/// Translate `Phase` to its store value.
pub fn phase(source: &domain::Phase, limits: &wire::Limits) -> Result<wire::Phase, wire::Problem> {
    Ok(match source {
        domain::Phase::Waiting => wire::Phase::Waiting,
        domain::Phase::Active(value) => {
            wire::Phase::Active(wire::Activation::new(limits, wire::ActivationParts { value: active(value, limits)? })?)
        }
        domain::Phase::Closing(value) => wire::Phase::Closing(closing(value, limits)?),
        domain::Phase::Held { was, why, source } => wire::Phase::Held(wire::Held::new(
            limits,
            wire::HeldParts {
                was: self::was(was, limits)?,
                why: hold(why, limits)?,
                source: optional_source(source.as_deref(), limits)?,
            },
        )?),
        domain::Phase::Ended(value) => {
            wire::Phase::Ended(wire::Final::new(limits, wire::FinalParts { ending: ending(value, limits)? })?)
        }
    })
}
/// Read `Phase` from its store value.
pub fn read_phase(source: wire::Phase, limits: &wire::Limits) -> Result<domain::Phase, wire::Problem> {
    Ok(match source {
        wire::Phase::Waiting => domain::Phase::Waiting,
        wire::Phase::Active(value) => {
            let wire::ActivationParts { value } = value.into_parts();
            domain::Phase::Active(read_active(value, limits)?)
        }
        wire::Phase::Closing(value) => domain::Phase::Closing(read_closing(value, limits)?),
        wire::Phase::Held(value) => {
            let wire::HeldParts { was, why, source } = value.into_parts();
            domain::Phase::Held {
                was: read_was(was, limits)?,
                why: read_hold(why, limits)?,
                source: read_optional_source(source, limits)?,
            }
        }
        wire::Phase::Ended(value) => {
            let wire::FinalParts { ending } = value.into_parts();
            domain::Phase::Ended(read_ending(ending, limits)?)
        }
    })
}

/// Translate `Grace` to its store record.
pub fn grace(source: &domain::Grace, limits: &wire::Limits) -> Result<wire::Grace, wire::Problem> {
    let domain::Grace { by, until, reason, message } = source;
    wire::Grace::new(
        limits,
        wire::GraceParts {
            by: value::party(by, limits)?,
            until: until.as_nanos(),
            reason: reason.clone(),
            message: *message,
        },
    )
}
/// Read `Grace` from its store record.
pub fn read_grace(source: wire::Grace, _limits: &wire::Limits) -> Result<domain::Grace, wire::Problem> {
    let wire::GraceParts { by, until, reason, message } = source.into_parts();
    Ok(domain::Grace { by: value::read_party(by), until: Wall::from_nanos(until), reason, message })
}

/// Translate `Revisions` to its store record.
pub fn revisions(source: &domain::Revisions, limits: &wire::Limits) -> Result<wire::Revisions, wire::Problem> {
    let domain::Revisions { hold, spec, person } = source;
    wire::Revisions::new(limits, wire::RevisionsParts { hold: *hold, spec: *spec, person: *person })
}
/// Read `Revisions` from its store record.
pub fn read_revisions(source: wire::Revisions, _limits: &wire::Limits) -> Result<domain::Revisions, wire::Problem> {
    let wire::RevisionsParts { hold, spec, person } = source.into_parts();
    Ok(domain::Revisions { hold, spec, person })
}

/// Translate `RetryNotice` to its store value.
pub fn retry_notice(source: &domain::RetryNotice, limits: &wire::Limits) -> Result<wire::RetryNotice, wire::Problem> {
    Ok(match source {
        domain::RetryNotice::Failed { class, count, allowance } => wire::RetryNotice::Failed(wire::FailedRetry::new(
            limits,
            wire::FailedRetryParts { class: value::class(*class), count: *count, allowance: *allowance },
        )?),
        domain::RetryNotice::Refused { count } => {
            wire::RetryNotice::Refused(wire::RefusedRetry::new(limits, wire::RefusedRetryParts { count: *count })?)
        }
    })
}
/// Read `RetryNotice` from its store value.
pub fn read_retry_notice(
    source: wire::RetryNotice,
    _limits: &wire::Limits,
) -> Result<domain::RetryNotice, wire::Problem> {
    Ok(match source {
        wire::RetryNotice::Failed(value) => {
            let wire::FailedRetryParts { class, count, allowance } = value.into_parts();
            domain::RetryNotice::Failed { class: value::read_class(class), count, allowance }
        }
        wire::RetryNotice::Refused(value) => {
            let wire::RefusedRetryParts { count } = value.into_parts();
            domain::RetryNotice::Refused { count }
        }
    })
}

pub(super) fn numbers(source: &domain::Numbers, limits: &wire::Limits) -> Result<wire::Numbers, wire::Problem> {
    let domain::Numbers { budget, spent, spent_below, reserved } = source;
    wire::Numbers::new(
        limits,
        wire::NumbersParts { budget: *budget, spent: *spent, spent_below: *spent_below, reserved: *reserved },
    )
}
pub(super) fn read_numbers(source: wire::Numbers) -> domain::Numbers {
    let wire::NumbersParts { budget, spent, spent_below, reserved } = source.into_parts();
    domain::Numbers { budget, spent, spent_below, reserved }
}
pub(super) fn references(
    source: &[u64],
    cap: u32,
    path: wire::Path,
    limits: &wire::Limits,
) -> Result<List<wire::Number>, wire::Problem> {
    let mut members = List::with_capacity(cap);
    for number in source {
        let number = wire::Number::new(limits, wire::NumberParts { value: *number })?;
        if members.push(number).is_err() {
            return Err(problem(path));
        }
    }
    Ok(members)
}
pub(super) fn read_references(source: List<wire::Number>) -> Box<[u64]> {
    let mut members = List::with_capacity(source.len());
    for number in source.into_boxed() {
        members.push(number.into_parts().value).expect("same number of references");
    }
    members.into_boxed()
}
fn member(
    source: &domain::New,
    limits: &wire::Limits,
    moved: Option<&[u64; 64]>,
) -> Result<wire::Member, wire::Problem> {
    let domain::New {
        label,
        flags,
        number,
        project,
        executor,
        llm,
        spec,
        contract,
        authority,
        deadline,
        numbers: allotment,
        funder,
        dependencies,
        holdings,
        wake: policy,
        recurring: _,
        tracked,
    } = source;
    let mut resources = List::with_capacity(limits.member_holdings);
    for resource in holdings {
        let resource = holding(resource, limits)?;
        if resources.push(resource).is_err() {
            return Err(problem(wire::Path::MemberHoldings));
        }
    }
    wire::Member::new(
        limits,
        wire::MemberParts {
            label: *label,
            flags: self::flags(*flags, limits)?,
            number: *number,
            project: *project,
            executor: value::executor(*executor, limits)?,
            llm: value::llm(llm.as_ref(), limits)?,
            spec: value::spec(spec, limits)?,
            contract: value::contract(contract, limits)?,
            authority: value::authority(authority, limits, moved)?,
            deadline: match deadline {
                domain::Deadline::Named => wire::Deadline::Named,
                domain::Deadline::Span(value) => {
                    wire::Deadline::Span(wire::DeadlineSpan::new(limits, wire::DeadlineSpanParts { value: *value })?)
                }
            },
            numbers: numbers(allotment, limits)?,
            funder: value::funder(*funder, limits)?,
            dependencies: references(dependencies, limits.member_dependencies, wire::Path::MemberDependencies, limits)?,
            holdings: resources,
            wake: wake(policy, limits)?,
            tracked: *tracked,
        },
    )
}
fn read_member(source: wire::Member, limits: &wire::Limits) -> Result<domain::New, wire::Problem> {
    let wire::MemberParts {
        label,
        flags,
        number,
        project,
        executor,
        llm,
        spec,
        contract,
        authority,
        deadline,
        numbers,
        funder,
        dependencies,
        holdings,
        wake: policy,
        tracked,
    } = source.into_parts();
    let mut resources = List::with_capacity(holdings.len());
    for resource in holdings.into_boxed() {
        resources.push(read_holding(resource, limits)?).expect("same number of holdings");
    }
    Ok(domain::New {
        deadline: match deadline {
            wire::Deadline::Named => domain::Deadline::Named,
            wire::Deadline::Span(value) => domain::Deadline::Span(value.value()),
        },
        label,
        flags: read_flags(flags),
        number,
        project,
        executor: value::read_executor(executor),
        llm: value::read_llm(llm),
        spec: value::read_spec(spec, limits)?,
        contract: value::read_contract(contract, limits)?,
        authority: value::read_authority(authority, limits)?,
        numbers: read_numbers(numbers),
        funder: value::read_funder(funder),
        dependencies: read_references(dependencies),
        holdings: resources.into_boxed(),
        wake: read_wake(policy, limits)?,
        recurring: None,
        tracked,
    })
}
/// Translate one creation, including its optional one-level template.
pub fn new(
    source: &domain::New,
    limits: &wire::Limits,
    moved: Option<&[u64; 64]>,
) -> Result<wire::Creation, wire::Problem> {
    let recurring = match &source.recurring {
        Some(value) => Some(template(value, limits, moved)?),
        None => None,
    };
    wire::Creation::new(limits, wire::CreationParts { member: member(source, limits, moved)?, recurring })
}
/// Read one creation, including its optional one-level template.
pub fn read_new(source: wire::Creation, limits: &wire::Limits) -> Result<domain::New, wire::Problem> {
    let wire::CreationParts { member, recurring } = source.into_parts();
    let mut source = read_member(member, limits)?;
    source.recurring = match recurring {
        Some(value) => Some(Box::new(read_template(value, limits)?)),
        None => None,
    };
    Ok(source)
}
/// Translate a recurring batch, refusing a recurring member.
pub fn template(
    source: &domain::RecurringTemplate,
    limits: &wire::Limits,
    moved: Option<&[u64; 64]>,
) -> Result<wire::RecurringTemplate, wire::Problem> {
    let domain::RecurringTemplate { key, batch, overlap: rule } = source;
    let mut members = List::with_capacity(limits.recurring_template_batch);
    for item in batch {
        if item.recurring.is_some() {
            return Err(problem(wire::Path::CreationRecurring));
        }
        let item = member(item, limits, moved)?;
        if members.push(item).is_err() {
            return Err(problem(wire::Path::RecurringTemplateBatch));
        }
    }
    wire::RecurringTemplate::new(
        limits,
        wire::RecurringTemplateParts { key: *key, batch: members, overlap: overlap(rule, limits)? },
    )
}
/// Read a recurring batch whose schema members cannot recur.
pub fn read_template(
    source: wire::RecurringTemplate,
    limits: &wire::Limits,
) -> Result<domain::RecurringTemplate, wire::Problem> {
    let wire::RecurringTemplateParts { key, batch, overlap } = source.into_parts();
    let mut members = List::with_capacity(batch.len());
    for item in batch.into_boxed() {
        members.push(read_member(item, limits)?).expect("same number of members");
    }
    Ok(domain::RecurringTemplate { key, batch: members.into_boxed(), overlap: read_overlap(overlap, limits)? })
}
/// Translate a result within its combined byte bound.
pub fn result(source: &domain::TaskResult, limits: &wire::Limits) -> Result<wire::TaskResult, wire::Problem> {
    check_result(source, limits)?;
    Ok(match source {
        domain::TaskResult::Approved { state, words } => wire::TaskResult::Approved(wire::Approved::new(
            limits,
            wire::ApprovedParts { state: state.clone(), words: words.clone() },
        )?),
        domain::TaskResult::Report { words } => {
            wire::TaskResult::Report(wire::ReportResult::new(limits, wire::ReportResultParts { words: words.clone() })?)
        }
        domain::TaskResult::Verdict { code, fields, words } => {
            let members =
                super::pieces::fields(fields, limits, limits.verdict_result_fields, wire::Path::VerdictResultFields)?;
            wire::TaskResult::Verdict(wire::VerdictResult::new(
                limits,
                wire::VerdictResultParts { code: *code, fields: members, words: words.clone() },
            )?)
        }
        domain::TaskResult::Change { connector, kind, resource, words } => {
            wire::TaskResult::Change(wire::ChangeResult::new(
                limits,
                wire::ChangeResultParts {
                    connector: *connector,
                    kind: *kind,
                    resource: *resource,
                    words: words.clone(),
                },
            )?)
        }
        domain::TaskResult::Failure { reason } => wire::TaskResult::Failure(wire::FailureResult::new(
            limits,
            wire::FailureResultParts { reason: reason.clone() },
        )?),
    })
}
/// Read a result within its combined byte bound.
pub fn read_result(source: wire::TaskResult, limits: &wire::Limits) -> Result<domain::TaskResult, wire::Problem> {
    let source = match source {
        wire::TaskResult::Approved(value) => {
            let wire::ApprovedParts { state, words } = value.into_parts();
            domain::TaskResult::Approved { state, words }
        }
        wire::TaskResult::Report(value) => domain::TaskResult::Report { words: value.into_parts().words },
        wire::TaskResult::Verdict(value) => {
            let wire::VerdictResultParts { code, fields, words } = value.into_parts();
            let members = super::pieces::read_fields(fields, limits, wire::Path::VerdictResultFields)?;
            domain::TaskResult::Verdict { code, fields: members, words }
        }
        wire::TaskResult::Change(value) => {
            let wire::ChangeResultParts { connector, kind, resource, words } = value.into_parts();
            domain::TaskResult::Change { connector, kind, resource, words }
        }
        wire::TaskResult::Failure(value) => domain::TaskResult::Failure { reason: value.into_parts().reason },
    };
    check_result(&source, limits)?;
    Ok(source)
}
fn check_result(source: &domain::TaskResult, limits: &wire::Limits) -> Result<(), wire::Problem> {
    let bytes = match source {
        domain::TaskResult::Approved { state, words } => state.len().saturating_add(match words {
            Some(value) => value.len(),
            None => 0,
        }),
        domain::TaskResult::Verdict { words, fields, .. } => {
            let mut bytes = words.len();
            for field in fields {
                bytes = bytes.saturating_add(field.name.len()).saturating_add(field.value.len());
            }
            bytes
        }
        domain::TaskResult::Report { words } | domain::TaskResult::Change { words, .. } => words.len(),
        domain::TaskResult::Failure { reason } => reason.len(),
    };
    if bytes > usize::try_from(limits.result_field_value).expect("limit fits usize") {
        return Err(problem(wire::Path::TaskResultTag));
    }
    Ok(())
}
fn optional_result(
    source: Option<&domain::TaskResult>,
    limits: &wire::Limits,
) -> Result<Option<wire::TaskResult>, wire::Problem> {
    match source {
        Some(value) => Ok(Some(result(value, limits)?)),
        None => Ok(None),
    }
}
fn read_optional_result(
    source: Option<wire::TaskResult>,
    limits: &wire::Limits,
) -> Result<Option<domain::TaskResult>, wire::Problem> {
    match source {
        Some(value) => Ok(Some(read_result(value, limits)?)),
        None => Ok(None),
    }
}
fn optional_source(
    source: Option<&domain::HoldSource>,
    limits: &wire::Limits,
) -> Result<Option<wire::HoldSource>, wire::Problem> {
    match source {
        Some(value) => Ok(Some(hold_source(value, limits)?)),
        None => Ok(None),
    }
}
fn read_optional_source(
    source: Option<wire::HoldSource>,
    limits: &wire::Limits,
) -> Result<Option<Box<domain::HoldSource>>, wire::Problem> {
    match source {
        Some(value) => Ok(Some(Box::new(read_hold_source(value, limits)?))),
        None => Ok(None),
    }
}
fn problem(path: wire::Path) -> wire::Problem {
    wire::Problem { path, reason: skein_codec::Reason::Bound }
}

/// Translate immutable task flags to their durable form.
pub(super) fn flags(source: domain::Flags, limits: &wire::Limits) -> Result<wire::Flags, wire::Problem> {
    let retention = match source.retention {
        domain::Retention::Kept => wire::Retention::Kept,
        domain::Retention::ForgottenOnceRead => wire::Retention::ForgottenOnceRead,
    };
    wire::Flags::new(
        limits,
        wire::FlagsParts {
            ended_by: match source.ended_by {
                domain::EndedBy::Result => wire::EndedBy::Result,
                domain::EndedBy::Requester => wire::EndedBy::Requester,
            },
            conversation: source.conversation,
            listed: source.listed,
            retention,
        },
    )
}
/// Read immutable task flags.
pub(super) fn read_flags(source: wire::Flags) -> domain::Flags {
    let source = source.into_parts();
    let retention = match source.retention {
        wire::Retention::Kept => domain::Retention::Kept,
        wire::Retention::ForgottenOnceRead => domain::Retention::ForgottenOnceRead,
    };
    domain::Flags {
        ended_by: match source.ended_by {
            wire::EndedBy::Result => domain::EndedBy::Result,
            wire::EndedBy::Requester => domain::EndedBy::Requester,
        },
        conversation: source.conversation,
        listed: source.listed,
        retention,
    }
}
