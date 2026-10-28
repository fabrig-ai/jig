//! Whole compact task facts (contract.md, 5.1; objects.md, 3).
//! Metadata remains mandatory and independent of phase, observation and ending.
use super::{owner, results};
use jig_client::DomainLimits as Limits;
use jig_client_domain::contract as client;
use jig_domain_core::documents;
use jig_domain_tasks as tasks;

#[must_use]
pub fn class(value: tasks::Class) -> client::HoldFailure {
    match value {
        tasks::Class::Transient => client::HoldFailure::Transient,
        tasks::Class::Permanent => client::HoldFailure::Permanent,
        tasks::Class::Run => client::HoldFailure::Run,
        tasks::Class::Agent => client::HoldFailure::Agent,
        tasks::Class::Lost => client::HoldFailure::Lost,
        tasks::Class::Invalid => client::HoldFailure::Invalid,
    }
}

#[must_use]
pub fn retry(value: tasks::RetryNotice) -> client::RetryNotice {
    match value {
        tasks::RetryNotice::Failed { class: value, count, allowance } => {
            client::RetryNotice::OwnerFailed { class: class(value), count, allowance }
        }
        tasks::RetryNotice::Refused { count } => client::RetryNotice::Refused { count },
    }
}

#[must_use]
pub fn hold(value: tasks::Hold) -> client::HoldCategory {
    match value {
        tasks::Hold::Exceeded(value) => client::HoldCategory::Exceeded(client::Exceeded {
            bound: match value.bound {
                tasks::Bound::Reasoning => client::Bound::Reasoning,
                tasks::Bound::Metadata => client::Bound::Metadata,
                tasks::Bound::Items => client::Bound::Items,
                tasks::Bound::Answer => client::Bound::Answer,
                tasks::Bound::Tools => client::Bound::Tools,
                tasks::Bound::HistoryItems => client::Bound::HistoryItems,
                tasks::Bound::History => client::Bound::History,
                tasks::Bound::Window => client::Bound::Window,
                tasks::Bound::Output => client::Bound::Output,
            },
            value: value.value,
        }),
        tasks::Hold::Failures(value) => client::HoldCategory::Failures { class: class(value) },
        tasks::Hold::Stalled => client::HoldCategory::Stalled,
        tasks::Hold::Dependency(task) => client::HoldCategory::Dependency { task },
        tasks::Hold::Stopped => client::HoldCategory::Stopped,
        tasks::Hold::Placed { hold } => client::HoldCategory::Placed { hold },
        tasks::Hold::Drift => client::HoldCategory::Drift,
        tasks::Hold::Effects => client::HoldCategory::Effects,
        tasks::Hold::EffectFailed => client::HoldCategory::EffectFailed,
        tasks::Hold::Uncertain { entry } => client::HoldCategory::Uncertain { entry },
        tasks::Hold::Budget => client::HoldCategory::Budget,
        tasks::Hold::Deadline => client::HoldCategory::Deadline,
        tasks::Hold::HoldsWaited => client::HoldCategory::HoldsWaited,
        tasks::Hold::ResourceUnavailable => client::HoldCategory::ResourceUnavailable,
        tasks::Hold::ResourceAccess => client::HoldCategory::ResourceAccess,
        tasks::Hold::PoolLost { pool } => client::HoldCategory::PoolLost { pool },
        tasks::Hold::Procedure => client::HoldCategory::Procedure,
        tasks::Hold::Circle => client::HoldCategory::Circle,
    }
}

#[must_use]
pub fn active(value: tasks::Active) -> client::RunStage {
    match value {
        tasks::Active::Idle => client::RunStage::Idle,
        tasks::Active::Due => client::RunStage::Due,
        tasks::Active::Preparing => client::RunStage::Preparing,
        tasks::Active::Claimed { attempt } => client::RunStage::Claimed { attempt },
        tasks::Active::Running { attempt } => client::RunStage::Running { attempt },
        tasks::Active::BackingOff { until } => client::RunStage::BackingOff { until },
    }
}

#[must_use]
pub fn stage(value: tasks::Stage) -> client::ClosingStage {
    match value {
        tasks::Stage::Run { attempt } => client::ClosingStage::Run { attempt },
        tasks::Stage::Delegates => client::ClosingStage::Delegates,
        tasks::Stage::Effects => client::ClosingStage::Effects,
        tasks::Stage::Releases => client::ClosingStage::Releases,
        tasks::Stage::Settled => client::ClosingStage::Settled,
    }
}

#[must_use]
pub fn address(value: tasks::PersonAddress) -> client::PersonAddress {
    match value {
        tasks::PersonAddress::Person(number) => client::PersonAddress::Party(number),
        tasks::PersonAddress::Role(number) => client::PersonAddress::Role(number),
    }
}

/// # Errors
/// Refuses inconsistent executor facts or any whole nested chip allocation above the receiving bounds.
pub fn phase(value: &documents::PhaseLine, limits: &Limits) -> Result<client::OwnerChipPhase, owner::Invalid> {
    Ok(match value {
        documents::PhaseLine::Waiting { dependencies, more, holds } => client::OwnerChipPhase::Waiting {
            dependencies: owner::list(dependencies, limits, |number| Ok(*number))?,
            more: *more,
            holds: *holds,
        },
        documents::PhaseLine::Active(value) => client::OwnerChipPhase::Active(active(*value)),
        documents::PhaseLine::Person { address: value, taken_by, since } => {
            client::OwnerChipPhase::Person { address: address(*value), taken_by: *taken_by, since: *since }
        }
        documents::PhaseLine::Closing(value) => client::OwnerChipPhase::Closing(stage(*value)),
        documents::PhaseLine::Held { reason, revision } => {
            client::OwnerChipPhase::Held { reason: hold(*reason), revision: *revision }
        }
        documents::PhaseLine::Ended => client::OwnerChipPhase::Ended,
    })
}

/// # Errors
/// Refuses inconsistent executor facts or any whole nested chip allocation above the receiving bounds.
pub fn held(value: &documents::HoldLine, limits: &Limits) -> Result<client::OwnerChipHold, owner::Invalid> {
    Ok(client::OwnerChipHold {
        placed: value.placed.as_ref().map(placed),
        circle: value.circle.as_ref().map(|circle| client::Circle { tasks: numbers(&circle.tasks), cut: circle.cut }),
        account: value.account.as_ref().map(|value| owner::part(value, limits)).transpose()?,
        source: value.source.as_ref().map(|value| owner::part(value, limits)).transpose()?,
        reason: hold(value.reason),
        revision: value.revision,
        before: match value.before {
            documents::PriorPhase::Waiting => client::OwnerPriorPhase::Waiting,
            documents::PriorPhase::Active(value) => client::OwnerPriorPhase::Active(active(value)),
            documents::PriorPhase::Closing(value) => client::OwnerPriorPhase::Closing(stage(value)),
        },
        escalation: value.escalation.as_ref().map(|value| escalation(value, limits)).transpose()?,
        tries: client::OwnerFailureTries {
            transient: value.tries.transient,
            permanent: value.tries.permanent,
            run: value.tries.run,
            agent: value.tries.agent,
            lost: value.tries.lost,
            invalid: value.tries.invalid,
        },
        hold_wait_since: value.hold_wait_since,
    })
}

fn escalation(
    value: &documents::EscalationState,
    limits: &Limits,
) -> Result<client::OwnerHoldEscalation, owner::Invalid> {
    Ok(match value {
        documents::EscalationState::Routing => client::OwnerHoldEscalation::Routing,
        documents::EscalationState::Waiting { holder, entry, since } => client::OwnerHoldEscalation::Waiting {
            holder: match holder {
                tasks::EscalationHolder::Task(number) => client::OwnerEscalationHolder::Task(*number),
                tasks::EscalationHolder::Person(number) => client::OwnerEscalationHolder::Person(*number),
                tasks::EscalationHolder::Role { project, role } => {
                    client::OwnerEscalationHolder::Role { project: *project, role: *role }
                }
            },
            entry: *entry,
            since: *since,
        },
        documents::EscalationState::Rejected { by, reason } => {
            client::OwnerHoldEscalation::Rejected { by: *by, reason: owner::text(reason, limits)? }
        }
    })
}

/// # Errors
/// Refuses the supplied raw part's whole allocation above the receiving bounds.
pub fn resource(value: &documents::ResourceLine, limits: &Limits) -> Result<client::OwnerResourceLine, owner::Invalid> {
    Ok(client::OwnerResourceLine {
        part: owner::part(&value.part, limits)?,
        kind: value.kind,
        taken: value.taken,
        holder: value.holder,
        writer: value.writer.map(|value| match value {
            tasks::Writer::Run { task, attempt } => client::OwnerResourceWriter::Run { task, attempt },
            tasks::Writer::Effect { entry } => client::OwnerResourceWriter::Effect { entry },
        }),
        writer_lost: value.writer_lost,
    })
}

/// # Errors
/// Refuses inconsistent executor facts or any whole nested chip allocation above the receiving bounds.
pub fn details(value: &documents::ChipDetails, limits: &Limits) -> Result<client::OwnerChipDetails, owner::Invalid> {
    Ok(client::OwnerChipDetails {
        dependencies: dependencies(&value.dependencies, limits)?,
        project: value.project,
        executor: owner::executor(value.executor),
        phase_since: value.phase_since,
        run_since: value.run_since,
        attempt: value.attempt,
        retry: value.retry.map(retry),
        wake: owner::wake(value.wake),
        person_wait: value.person_wait.map(|value| client::OwnerPersonWait {
            since: value.since,
            stall: value.stall,
            deadline: value.deadline,
            notice: value.notice.map(|value| client::OwnerPersonStallNotice {
                message: value.message,
                at: value.at,
                activity: value.activity,
            }),
        }),
        hold: value.hold.as_ref().map(|value| held(value, limits)).transpose()?,
        budget: owner::budget(&value.budget),
        activity: value.activity,
        resources: owner::list(&value.resources, limits, |value| resource(value, limits))?,
        resources_cut: value.resources_cut,
        resources_observed: value.resources_observed,
        procedure: value.procedure.as_ref().map(|value| owner::part(value, limits)).transpose()?,
        live_delegates: value.live_delegates,
        closing_connectors: value
            .closing_connectors
            .as_ref()
            .map(|value| owner::list(value, limits, |number| Ok(*number)))
            .transpose()?,
        ending: value.ending.as_ref().map(|value| results::ending(value, limits)).transpose()?,
        ended_at: value.ended_at,
        observation: value.observation.as_ref().map(|value| observation(value, limits)).transpose()?,
    })
}

fn observation(
    value: &documents::ChipObservation,
    limits: &Limits,
) -> Result<client::OwnerChipObservation, owner::Invalid> {
    Ok(client::OwnerChipObservation {
        channel: value.channel,
        work: match &value.work {
            documents::ChipWork::Preparing => client::OwnerChipWork::Preparing,
            documents::ChipWork::Starting => client::OwnerChipWork::Starting,
            documents::ChipWork::Active => client::OwnerChipWork::Active,
            documents::ChipWork::Writing => client::OwnerChipWork::Writing,
            documents::ChipWork::Tool { call, tool } => {
                client::OwnerChipWork::Tool { call: owner::bytes(call, limits)?, tool: owner::bytes(tool, limits)? }
            }
            documents::ChipWork::Waiting => client::OwnerChipWork::Waiting,
            documents::ChipWork::Ending => client::OwnerChipWork::Ending,
        },
        since: value.since,
        reported_at: value.reported_at,
        started: value.started,
    })
}

/// # Errors
/// Refuses inconsistent executor facts or any whole nested chip allocation above the receiving bounds.
pub fn chip(value: &documents::Chip, limits: &Limits) -> Result<client::Chip, owner::Invalid> {
    let executor = match value.details.executor {
        tasks::Executor::Agent { charter } => {
            if value.executor != documents::ExecutorKind::Agent {
                return Err(owner::Invalid::Executor);
            }
            client::Executor::Agent { charter }
        }
        tasks::Executor::Procedure { connector, code } => {
            if value.executor != documents::ExecutorKind::Procedure {
                return Err(owner::Invalid::Executor);
            }
            client::Executor::Procedure { connector, kind: code }
        }
        tasks::Executor::Person(address) => {
            if value.executor != documents::ExecutorKind::Person {
                return Err(owner::Invalid::Executor);
            }
            match address {
                tasks::PersonAddress::Person(party) => client::Executor::Party { party },
                tasks::PersonAddress::Role(role) => client::Executor::Role { project: value.details.project, role },
            }
        }
    };
    let phase = phase(&value.phase, limits)?;
    Ok(client::Chip {
        task: value.number,
        title: owner::text(&value.title, limits)?,
        executor,
        phase: phase.compact(),
        details: Box::new(client::ChipPhase::Owner { phase, details: Box::new(details(&value.details, limits)?) }),
    })
}

/// # Errors
/// Rejects dependency allocations above the receiver's existing row bound.
pub fn dependencies(
    values: &skein_lib::List<documents::Dependency>,
    limits: &Limits,
) -> Result<skein_lib::List<client::Dependency>, owner::Invalid> {
    owner::list(values, limits, |value| {
        Ok(client::Dependency {
            number: value.number,
            pending: value.pending,
            keeps: value.keeps,
            added: value
                .added
                .as_ref()
                .map(|added| client::DependencyAdded { at: added.at, by: results::requester(added.by) }),
            status: value.status.map(|status| match status {
                tasks::Status::Done => client::OwnerTaskStatus::Done,
                tasks::Status::Failed => client::OwnerTaskStatus::Failed,
                tasks::Status::Cancelled => client::OwnerTaskStatus::Cancelled,
            }),
        })
    })
}

fn numbers(values: &[u64]) -> skein_lib::List<u64> {
    let mut rows = skein_lib::List::with_capacity(u32::try_from(values.len()).expect("bounded numbers"));
    for number in values {
        rows.push(*number).expect("same number count");
    }
    rows
}

#[cfg(test)]
pub(super) mod tests {
    const UTF8_TEXTS: [&[u8]; 1] = [b"~/"];

    use super::*;
    use skein_lib::{Duration, List, Token, Wall};

    pub(crate) fn source_chip() -> documents::Chip {
        documents::Chip {
            number: u64::MAX,
            title: documents::Text { bytes: Box::from(UTF8_TEXTS[0]), cut: true },
            executor: documents::ExecutorKind::Person,
            phase: documents::PhaseLine::Ended,
            details: Box::new(documents::ChipDetails {
                dependencies: List::with_capacity(0),
                project: u32::MAX,
                executor: tasks::Executor::Person(tasks::PersonAddress::Role(u32::MAX)),
                phase_since: Wall::from_nanos(3),
                run_since: None,
                attempt: Some(u64::MAX),
                retry: Some(tasks::RetryNotice::Failed { class: tasks::Class::Invalid, count: u32::MAX, allowance: 0 }),
                wake: tasks::WakePolicy {
                    words: tasks::WakeRule::Immediate,
                    notices: tasks::WakeRule::Never,
                    news: tasks::WakeRule::Batch { count: u32::MAX, age: Duration::ZERO },
                    results: tasks::ResultsWake::Each,
                    questions: false,
                    answers: true,
                    timers: false,
                },
                person_wait: Some(tasks::PersonWait {
                    since: Wall::from_nanos(5),
                    stall: Duration::from_nanos(u64::MAX),
                    deadline: Wall::from_nanos(7),
                    notice: Some(tasks::PersonStallNotice {
                        message: u64::MAX,
                        at: Wall::from_nanos(11),
                        activity: 17,
                    }),
                }),
                hold: None,
                budget: documents::BudgetLine {
                    numbers: tasks::Numbers { budget: 2, spent: 3, spent_below: 5, reserved: u64::MAX },
                    left: 13,
                    funder: tasks::Funder::Period { project: u32::MAX, period: u64::MAX },
                    deadline: None,
                },
                activity: 19,
                resources: List::with_capacity(4),
                resources_cut: true,
                resources_observed: false,
                procedure: Some(tasks::Part {
                    connector: u16::MAX,
                    kind: tasks::PartKind::Outbox,
                    key: tasks::PartKey::Number(u64::MAX),
                }),
                live_delegates: u32::MAX,
                closing_connectors: None,
                ending: None,
                ended_at: Some(Wall::EPOCH),
                observation: Some(documents::ChipObservation {
                    channel: Token::new(u64::MAX),
                    work: documents::ChipWork::Tool { call: Box::from([255, 0]), tool: Box::from([b'/', 255]) },
                    since: Wall::from_nanos(23),
                    reported_at: Wall::from_nanos(29),
                    started: None,
                }),
            }),
        }
    }

    #[test]
    fn supplied_hold_account_and_resource_or_outbox_origin_survive_independently() {
        let limits = Limits { rows: 2, text: 4, turn_text: 4, ..crate::test::limits().client.domain };
        let account =
            tasks::Part { connector: u16::MAX, kind: tasks::PartKind::Hold, key: tasks::PartKey::Number(u64::MAX) };
        let mut path = List::with_capacity(2);
        path.push(Box::from(&b"a/b"[..])).expect("first whole origin segment");
        path.push(Box::from(&[255_u8, 0][..])).expect("second whole origin segment");
        for origin in [
            None,
            Some(tasks::Part { connector: 7, kind: tasks::PartKind::Resource, key: tasks::PartKey::Path(path) }),
            Some(tasks::Part { connector: 9, kind: tasks::PartKind::Outbox, key: tasks::PartKey::Number(8) }),
        ] {
            for supplied in [None, Some(account.clone())] {
                let source = documents::HoldLine {
                    placed: None,
                    circle: None,
                    account: supplied.clone(),
                    source: origin.clone(),
                    reason: tasks::Hold::Drift,
                    revision: 29,
                    before: documents::PriorPhase::Active(tasks::Active::Running { attempt: 31 }),
                    escalation: None,
                    tries: tasks::Tries::NONE,
                    hold_wait_since: Some(Wall::from_nanos(37)),
                };
                let native = held(&source, &limits).expect("whole supplied hold");
                assert_eq!(
                    native.account,
                    supplied.as_ref().map(|_| client::OwnerPart {
                        connector: u16::MAX,
                        kind: client::OwnerPartKind::Hold,
                        key: client::OwnerPartKey::Number(u64::MAX),
                    })
                );
                assert_eq!(
                    native.source,
                    origin.as_ref().map(|part| owner::part(part, &limits).expect("whole unchanged origin"))
                );
                assert_eq!(native.revision, 29);
                assert_eq!(native.before, client::OwnerPriorPhase::Active(client::RunStage::Running { attempt: 31 }));
                assert_eq!(native.hold_wait_since, Some(Wall::from_nanos(37)));
                let mut chip = source_chip();
                chip.details.hold = Some(source);
                let metadata = details(&chip.details, &Limits { rows: 4, text: 4, turn_text: 4, ..limits })
                    .expect("whole chip details");
                assert_eq!(metadata.hold, Some(native));
            }
        }
    }

    #[test]
    fn whole_chip_keeps_mandatory_metadata_unknown_ending_title_loss_and_sparse_observation_independent() {
        let mut source = source_chip();
        let limits = Limits { rows: 4, text: 3, turn_text: 3, ..crate::test::limits().client.domain };
        for connectors in [None, Some(List::with_capacity(4))] {
            source.details.closing_connectors = connectors;
            let before = source.clone();
            let actual = chip(&source, &limits).expect("complete owner chip fits");
            assert_eq!(source, before);
            assert_eq!(actual.task, u64::MAX);
            assert_eq!(actual.title.bytes.bytes(), UTF8_TEXTS[0]);
            assert!(actual.title.cut);
            assert_eq!(actual.executor, client::Executor::Role { project: u32::MAX, role: u32::MAX });
            assert_eq!(actual.phase, client::Phase::Ended, "generic ending never becomes a guessed outcome");
            let client::ChipPhase::Owner { phase, details } = actual.details.as_ref() else {
                panic!("mandatory whole metadata")
            };
            assert_eq!(phase, &client::OwnerChipPhase::Ended);
            assert_eq!(details.phase_since, Wall::from_nanos(3));
            assert_eq!(details.run_since, None);
            assert_eq!(details.attempt, Some(u64::MAX));
            assert_eq!(
                details.retry,
                Some(client::RetryNotice::OwnerFailed {
                    class: client::HoldFailure::Invalid,
                    count: u32::MAX,
                    allowance: 0
                })
            );
            assert_eq!(details.budget.numbers.reserved, u64::MAX);
            assert_eq!(details.budget.left, 13);
            assert_eq!(details.budget.deadline, None);
            assert_eq!(details.activity, 19);
            assert_eq!(details.resources.capacity(), 4);
            assert!(details.resources_cut);
            assert!(!details.resources_observed);
            assert_eq!(
                details.closing_connectors.as_ref().map(List::capacity),
                source.details.closing_connectors.as_ref().map(List::capacity)
            );
            assert!(details.ending.is_none());
            assert_eq!(details.ended_at, Some(Wall::EPOCH));
            assert_eq!(details.procedure.as_ref().expect("actual raw part").kind, client::OwnerPartKind::Outbox);
            assert_eq!(
                details.person_wait.as_ref().expect("actual person wait").notice.expect("actual notice").activity,
                17
            );
            let observation = details.observation.as_ref().expect("supplied observation");
            assert_eq!(observation.channel, Token::new(u64::MAX));
            assert_eq!(observation.since, Wall::from_nanos(23));
            assert_eq!(observation.reported_at, Wall::from_nanos(29));
            assert_eq!(observation.started, None, "an observation is not an attested start");
            assert_eq!(
                observation.work,
                client::OwnerChipWork::Tool { call: Box::from([255, 0]), tool: Box::from([b'/', 255]) }
            );
        }
    }

    #[test]
    fn exact_owner_retry_keeps_all_six_classes_and_independent_refusal_count_without_a_reason_or_policy() {
        for (source, expected) in [
            (tasks::Class::Transient, client::HoldFailure::Transient),
            (tasks::Class::Permanent, client::HoldFailure::Permanent),
            (tasks::Class::Run, client::HoldFailure::Run),
            (tasks::Class::Agent, client::HoldFailure::Agent),
            (tasks::Class::Lost, client::HoldFailure::Lost),
            (tasks::Class::Invalid, client::HoldFailure::Invalid),
        ] {
            assert_eq!(
                retry(tasks::RetryNotice::Failed { class: source, count: u32::MAX, allowance: 0 }),
                client::RetryNotice::OwnerFailed { class: expected, count: u32::MAX, allowance: 0 }
            );
            assert_eq!(hold(tasks::Hold::Failures(source)), client::HoldCategory::Failures { class: expected });
        }
        assert_eq!(
            retry(tasks::RetryNotice::Refused { count: u32::MAX }),
            client::RetryNotice::Refused { count: u32::MAX }
        );
        let mut source = source_chip();
        source.details.retry = None;
        let limits = Limits { rows: 4, text: 3, turn_text: 3, ..crate::test::limits().client.domain };
        assert!(details(&source.details, &limits).expect("complete supplied metadata").retry.is_none());
    }

    #[test]
    fn chip_phase_and_preserved_hold_keep_named_lifecycle_address_revision_and_separate_failure_counters() {
        let limits = Limits { rows: 4, text: 3, turn_text: 3, ..crate::test::limits().client.domain };
        let mut dependencies = List::with_capacity(4);
        dependencies.push(u64::MAX).expect("declared room");
        assert_eq!(
            phase(
                &documents::PhaseLine::Waiting { dependencies: dependencies.clone(), more: true, holds: false },
                &limits
            ),
            Ok(client::OwnerChipPhase::Waiting { dependencies, more: true, holds: false })
        );
        assert_eq!(
            phase(
                &documents::PhaseLine::Person {
                    address: tasks::PersonAddress::Person(u64::MAX),
                    taken_by: Some(0),
                    since: None
                },
                &limits
            ),
            Ok(client::OwnerChipPhase::Person {
                address: client::PersonAddress::Party(u64::MAX),
                taken_by: Some(0),
                since: None
            })
        );
        assert_eq!(
            phase(&documents::PhaseLine::Active(tasks::Active::Running { attempt: u64::MAX }), &limits),
            Ok(client::OwnerChipPhase::Active(client::RunStage::Running { attempt: u64::MAX }))
        );
        assert_eq!(
            phase(&documents::PhaseLine::Closing(tasks::Stage::Run { attempt: u64::MAX }), &limits),
            Ok(client::OwnerChipPhase::Closing(client::ClosingStage::Run { attempt: u64::MAX }))
        );
        let source = documents::HoldLine {
            placed: None,
            circle: None,
            account: None,
            source: None,
            reason: tasks::Hold::PoolLost { pool: u64::MAX },
            revision: u64::MAX,
            before: documents::PriorPhase::Active(tasks::Active::BackingOff { until: Wall::EPOCH }),
            escalation: Some(documents::EscalationState::Rejected {
                by: u64::MAX,
                reason: documents::Text { bytes: Box::new([]), cut: true },
            }),
            tries: tasks::Tries { transient: 2, permanent: 3, run: 5, agent: 7, lost: 11, invalid: u32::MAX },
            hold_wait_since: None,
        };
        let actual = held(&source, &limits).expect("whole preserved hold");
        assert_eq!(actual.reason, client::HoldCategory::PoolLost { pool: u64::MAX });
        assert_eq!(actual.revision, u64::MAX);
        assert_eq!(actual.before, client::OwnerPriorPhase::Active(client::RunStage::BackingOff { until: Wall::EPOCH }));
        assert_eq!(
            actual.tries,
            client::OwnerFailureTries { transient: 2, permanent: 3, run: 5, agent: 7, lost: 11, invalid: u32::MAX }
        );
        assert!(actual.source.is_none());
        assert!(actual.hold_wait_since.is_none());
        let Some(client::OwnerHoldEscalation::Rejected { by, reason }) = actual.escalation else {
            panic!("exact rejected escalation")
        };
        assert_eq!(by, u64::MAX);
        assert!(reason.bytes.bytes().is_empty());
        assert!(reason.cut);
    }

    #[test]
    fn chip_receiver_refuses_reserved_rows_raw_tool_bytes_and_inconsistent_executor_facts_without_trimming() {
        let source = source_chip();
        let before = source.clone();
        let limits = Limits { rows: 4, text: 3, turn_text: 3, ..crate::test::limits().client.domain };
        assert_eq!(
            chip(&source, &Limits { rows: 3, turn_text: limits.turn_text, ..limits }),
            Err(owner::Invalid::TooLarge)
        );
        assert_eq!(chip(&source, &Limits { text: 1, turn_text: 1, ..limits }), Err(owner::Invalid::TooLarge));
        let mut mismatch = source.clone();
        mismatch.executor = documents::ExecutorKind::Agent;
        assert_eq!(chip(&mismatch, &limits), Err(owner::Invalid::Executor));
        assert_eq!(source, before);
    }
    #[test]
    fn named_chip_collection_admits_the_whole_allocated_envelope_before_delivery() {
        let mut source = List::with_capacity(4);
        source.push(source_chip()).expect("supplied whole Chip fits reserved row");
        let before = source.clone();
        let limits = Limits {
            rows: 4,
            turn_text: crate::test::limits().client.domain.turn_text,
            ..crate::test::limits().client.domain
        };
        let value = super::super::documents::chips_delivery(&source, &limits).expect("whole envelope fits");
        let client::Delivery::Chips(value) = value else { panic!("named Chip envelope") };
        assert_eq!(value.capacity(), 4);
        assert_eq!(value.len(), 1);
        assert_eq!(value.as_slice()[0], chip(&source.as_slice()[0], &limits).expect("whole exact Chip"));
        assert_eq!(
            super::super::documents::chips_delivery(
                &source,
                &Limits { document_bytes: 0, turn_text: limits.turn_text, ..limits }
            ),
            Err(super::super::documents::Invalid::Payload)
        );
        assert_eq!(
            super::super::documents::chips_delivery(
                &source,
                &Limits { rows: 3, turn_text: limits.turn_text, ..limits }
            ),
            Err(super::super::documents::Invalid::Owner(owner::Invalid::TooLarge))
        );
        assert_eq!(source, before);
    }
}

#[must_use]
pub fn placed(hold: &documents::PlacedLine) -> client::PlacedHold {
    client::PlacedHold {
            number: hold.number,
            owner: super::refusals::hold_owner(hold.owner),
            mode: match hold.mode {
                tasks::Mode::Stop => client::HoldMode::Stop,
                tasks::Mode::Drain => client::HoldMode::Drain,
            },
            scope: match hold.scope {
                tasks::Scope::Subtree { task } => client::HoldScope::Subtree { task },
                tasks::Scope::Project => client::HoldScope::Project,
            },
            reason: hold.reason.clone(),
            placed_at: hold.placed_at,
            revision: client::Revision { value: hold.revision },
            lifted: hold.lifted,
            may_lift: hold.may_lift,
            }
}
