//! Field-for-field client contract translation (protocol/client.md, 3 and 5).
use super::{Child, take_chip_phase};
use crate::Limits;
use crate::translate::{piece_problem, text_problem, too_large};
use alloc::boxed::Box;
use jig_client_domain::contract as c;
use jig_client_protocol_wire::v19 as w;
use skein_lib::Map;
use skein_lib::{List, Reader, Wall, bytes};
pub(crate) fn document_text(value: &w::DocumentText, limits: &Limits) -> Result<c::DocumentText, w::Problem> {
    Ok(c::DocumentText {
        bytes: match c::Text::new(bytes::copy_of(value.bytes()), limits.client.text) {
            Ok(value) => value,
            Err(problem) => return Err(text_problem(problem)),
        },
        cut: value.cut(),
    })
}

pub(crate) fn executor(value: &w::Executor, _limits: &Limits) -> c::Executor {
    match value {
        w::Executor::Agent(row) => c::Executor::Agent { charter: row.charter() },
        w::Executor::Procedure(row) => c::Executor::Procedure { connector: row.connector(), kind: row.kind() },
        w::Executor::Party(row) => c::Executor::Party { party: row.party() },
        w::Executor::Role(row) => c::Executor::Role { project: row.project(), role: row.role() },
    }
}

pub(crate) fn phase(value: &w::Phase, _limits: &Limits) -> c::Phase {
    match value {
        w::Phase::Waiting => c::Phase::Waiting,
        w::Phase::Idle => c::Phase::Idle,
        w::Phase::Due => c::Phase::Due,
        w::Phase::Preparing => c::Phase::Preparing,
        w::Phase::Claimed => c::Phase::Claimed,
        w::Phase::Running => c::Phase::Running,
        w::Phase::BackingOff => c::Phase::BackingOff,
        w::Phase::Active => c::Phase::Active,
        w::Phase::Closing => c::Phase::Closing,
        w::Phase::Held => c::Phase::Held,
        w::Phase::Ended => c::Phase::Ended,
        w::Phase::Done => c::Phase::Done,
        w::Phase::Failed => c::Phase::Failed,
        w::Phase::Cancelled => c::Phase::Cancelled,
    }
}

pub(crate) fn chip_fields(
    value: &w::ChipFields,
    limits: &Limits,
    index: u32,
    children: &mut Map<u32, Child>,
) -> Result<c::Chip, w::Problem> {
    Ok(c::Chip {
        task: value.task(),
        title: document_text(value.title(), limits)?,
        executor: executor(value.executor(), limits),
        phase: phase(value.phase(), limits),
        details: Box::new(take_chip_phase(value.details(), index, children)?),
    })
}

pub(crate) fn run_stage(value: &w::RunStage, _limits: &Limits) -> c::RunStage {
    match value {
        w::RunStage::Idle => c::RunStage::Idle,
        w::RunStage::Due => c::RunStage::Due,
        w::RunStage::Preparing => c::RunStage::Preparing,
        w::RunStage::Claimed(row) => c::RunStage::Claimed { attempt: row.attempt() },
        w::RunStage::Running(row) => c::RunStage::Running { attempt: row.attempt() },
        w::RunStage::BackingOff(row) => c::RunStage::BackingOff { until: Wall::from_nanos(row.until()) },
    }
}

pub(crate) fn person_address(value: &w::PersonAddress, _limits: &Limits) -> c::PersonAddress {
    match value {
        w::PersonAddress::Party(row) => c::PersonAddress::Party(row.value()),
        w::PersonAddress::Role(row) => c::PersonAddress::Role(row.value()),
    }
}

pub(crate) fn closing_stage(value: &w::ClosingStage, _limits: &Limits) -> c::ClosingStage {
    match value {
        w::ClosingStage::Run(row) => c::ClosingStage::Run { attempt: row.attempt() },
        w::ClosingStage::Delegates => c::ClosingStage::Delegates,
        w::ClosingStage::Effects => c::ClosingStage::Effects,
        w::ClosingStage::Releases => c::ClosingStage::Releases,
        w::ClosingStage::Settled => c::ClosingStage::Settled,
    }
}

pub(crate) fn hold_failure(value: &w::HoldFailure, _limits: &Limits) -> c::HoldFailure {
    match value {
        w::HoldFailure::Transient => c::HoldFailure::Transient,
        w::HoldFailure::Permanent => c::HoldFailure::Permanent,
        w::HoldFailure::Run => c::HoldFailure::Run,
        w::HoldFailure::Agent => c::HoldFailure::Agent,
        w::HoldFailure::Lost => c::HoldFailure::Lost,
        w::HoldFailure::Invalid => c::HoldFailure::Invalid,
    }
}

pub(crate) fn hold_category(value: &w::HoldCategory, limits: &Limits) -> c::HoldCategory {
    match value {
        w::HoldCategory::Exceeded(value) => c::HoldCategory::Exceeded(c::Exceeded {
            bound: match value.bound() {
                w::Bound::Reasoning => c::Bound::Reasoning,
                w::Bound::Metadata => c::Bound::Metadata,
                w::Bound::Items => c::Bound::Items,
                w::Bound::Answer => c::Bound::Answer,
                w::Bound::Tools => c::Bound::Tools,
                w::Bound::HistoryItems => c::Bound::HistoryItems,
                w::Bound::History => c::Bound::History,
                w::Bound::Window => c::Bound::Window,
                w::Bound::Output => c::Bound::Output,
            },
            value: value.value(),
        }),
        w::HoldCategory::Failures(row) => c::HoldCategory::Failures { class: hold_failure(row.class(), limits) },
        w::HoldCategory::Stalled => c::HoldCategory::Stalled,
        w::HoldCategory::Dependency(row) => c::HoldCategory::Dependency { task: row.task() },
        w::HoldCategory::Stopped => c::HoldCategory::Stopped,
        w::HoldCategory::Placed(row) => c::HoldCategory::Placed { hold: row.hold() },
        w::HoldCategory::Drift => c::HoldCategory::Drift,
        w::HoldCategory::Effects => c::HoldCategory::Effects,
        w::HoldCategory::EffectFailed => c::HoldCategory::EffectFailed,
        w::HoldCategory::Uncertain(row) => c::HoldCategory::Uncertain { entry: row.entry() },
        w::HoldCategory::Budget => c::HoldCategory::Budget,
        w::HoldCategory::Deadline => c::HoldCategory::Deadline,
        w::HoldCategory::HoldsWaited => c::HoldCategory::HoldsWaited,
        w::HoldCategory::ResourceUnavailable => c::HoldCategory::ResourceUnavailable,
        w::HoldCategory::ResourceAccess => c::HoldCategory::ResourceAccess,
        w::HoldCategory::PoolLost(row) => c::HoldCategory::PoolLost { pool: row.pool() },
        w::HoldCategory::Procedure => c::HoldCategory::Procedure,
        w::HoldCategory::Circle => c::HoldCategory::Circle,
    }
}

#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn owner_chip_phase(value: &w::OwnerChipPhase, limits: &Limits) -> c::OwnerChipPhase {
    match value {
        w::OwnerChipPhase::Waiting(row) => {
            let mut list0 = List::with_capacity(row.dependencies().len());
            for item in row.dependencies() {
                list0.push(*item).expect("reserved supplied list");
            }
            c::OwnerChipPhase::Waiting { dependencies: list0, more: row.more(), holds: row.holds() }
        }
        w::OwnerChipPhase::Active(row) => c::OwnerChipPhase::Active(run_stage(row.value(), limits)),
        w::OwnerChipPhase::Person(row) => {
            let opt0 = match row.taken_by() {
                Some(item) => Some(*item),
                None => None,
            };
            let opt1 = match row.since() {
                Some(item) => Some(Wall::from_nanos(*item)),
                None => None,
            };
            c::OwnerChipPhase::Person { address: person_address(row.address(), limits), taken_by: opt0, since: opt1 }
        }
        w::OwnerChipPhase::Closing(row) => c::OwnerChipPhase::Closing(closing_stage(row.value(), limits)),
        w::OwnerChipPhase::Held(row) => {
            c::OwnerChipPhase::Held { reason: hold_category(row.reason(), limits), revision: row.revision() }
        }
        w::OwnerChipPhase::Ended => c::OwnerChipPhase::Ended,
    }
}

pub(crate) fn failure_class(value: &w::FailureClass, _limits: &Limits) -> c::FailureClass {
    match value {
        w::FailureClass::HostLost => c::FailureClass::HostLost,
        w::FailureClass::Temporary => c::FailureClass::Temporary,
        w::FailureClass::Permanent => c::FailureClass::Permanent,
        w::FailureClass::Stall => c::FailureClass::Stall,
    }
}

pub(crate) fn retry_notice(value: &w::RetryNotice, limits: &Limits) -> c::RetryNotice {
    match value {
        w::RetryNotice::OwnerFailed(row) => c::RetryNotice::OwnerFailed {
            class: hold_failure(row.class(), limits),
            count: row.count(),
            allowance: row.allowance(),
        },
        w::RetryNotice::Failed(row) => c::RetryNotice::Failed {
            class: failure_class(row.class(), limits),
            count: row.count(),
            allowance: row.allowance(),
        },
        w::RetryNotice::Refused(row) => c::RetryNotice::Refused { count: row.count() },
    }
}

pub(crate) fn call_identity(value: &w::CallIdentity, limits: &Limits) -> Result<c::CallIdentity, w::Problem> {
    Ok(c::CallIdentity {
        owner: skein_lib::Token::new(value.owner()),
        sequence: value.sequence(),
        retry: value.retry(),
        id: match c::Text::new(bytes::copy_of(value.id()), limits.client.text) {
            Ok(value) => value,
            Err(problem) => return Err(text_problem(problem)),
        },
    })
}

pub(crate) fn run_work(value: &w::RunWork, limits: &Limits) -> Result<c::RunWork, w::Problem> {
    Ok(match value {
        w::RunWork::Preparing => c::RunWork::Preparing,
        w::RunWork::Starting => c::RunWork::Starting,
        w::RunWork::Active => c::RunWork::Active,
        w::RunWork::Writing => c::RunWork::Writing,
        w::RunWork::Tool(row) => c::RunWork::Tool {
            call: call_identity(row.call(), limits)?,
            tool: match c::Text::new(bytes::copy_of(row.tool()), limits.client.text) {
                Ok(value) => value,
                Err(problem) => return Err(text_problem(problem)),
            },
        },
        w::RunWork::Waiting => c::RunWork::Waiting,
        w::RunWork::Ending => c::RunWork::Ending,
    })
}

#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn run_observation(value: &w::RunObservation, limits: &Limits) -> Result<c::RunObservation, w::Problem> {
    let opt0 = match value.started() {
        Some(item) => Some(Wall::from_nanos(*item)),
        None => None,
    };
    Ok(c::RunObservation {
        channel: skein_lib::Token::new(value.channel()),
        work: run_work(value.work(), limits)?,
        since: Wall::from_nanos(value.since()),
        reported_at: Wall::from_nanos(value.reported_at()),
        started: opt0,
    })
}

#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn active_run(value: &w::ActiveRun, limits: &Limits) -> Result<c::ActiveRun, w::Problem> {
    let opt0 = match value.attempt() {
        Some(item) => Some(*item),
        None => None,
    };
    let opt1 = match value.placement() {
        Some(item) => Some(Wall::from_nanos(*item)),
        None => None,
    };
    let opt2 = match &value.retry() {
        Some(item) => Some(retry_notice(item, limits)),
        None => None,
    };
    let opt3 = match &value.observation() {
        Some(item) => Some(run_observation(item, limits)?),
        None => None,
    };
    Ok(c::ActiveRun {
        stage: run_stage(value.stage(), limits),
        phase_since: Wall::from_nanos(value.phase_since()),
        attempt: opt0,
        placement: opt1,
        retry: opt2,
        observation: opt3,
    })
}

pub(crate) fn proposer(value: &w::Proposer, _limits: &Limits) -> c::Proposer {
    match value {
        w::Proposer::Task(row) => c::Proposer::Task(row.value()),
        w::Proposer::Party(row) => c::Proposer::Party(row.value()),
    }
}

pub(crate) fn proposal_key(value: &w::ProposalKey, limits: &Limits) -> c::ProposalKey {
    c::ProposalKey { proposer: proposer(value.proposer(), limits), number: value.number() }
}

pub(crate) fn part_scope(value: &w::PartScope, _limits: &Limits) -> c::PartScope {
    match value {
        w::PartScope::Project(row) => c::PartScope::Project(row.value()),
        w::PartScope::Deployment => c::PartScope::Deployment,
    }
}

pub(crate) fn part(value: &w::Part, limits: &Limits) -> c::Part {
    match value {
        w::Part::Connector(row) => c::Part::Connector { connector: row.connector() },
        w::Part::Resource(row) => {
            let mut list0 = List::with_capacity(row.path().len());
            for item in row.path() {
                list0.push(bytes::copy_of(item)).expect("reserved supplied list");
            }
            c::Part::Resource { connector: row.connector(), path: list0 }
        }
        w::Part::Kind(row) => c::Part::Kind { connector: row.connector(), kind: row.kind() },
        w::Part::Panel(row) => c::Part::Panel { connector: row.connector(), task: row.task() },
        w::Part::Result(row) => c::Part::Result { connector: row.connector(), task: row.task() },
        w::Part::Effect(row) => {
            c::Part::Effect { connector: row.connector(), proposal: proposal_key(row.proposal(), limits) }
        }
        w::Part::OwnerEffect(row) => c::Part::OwnerEffect { connector: row.connector(), proposal: row.proposal() },
        w::Part::Outbox(row) => {
            c::Part::Outbox { connector: row.connector(), scope: part_scope(row.scope(), limits), entry: row.entry() }
        }
        w::Part::Hold(row) => c::Part::Hold { connector: row.connector(), task: row.task() },
        w::Part::Project(row) => c::Part::Project { connector: row.connector(), project: row.project() },
        w::Part::Deployment(row) => c::Part::Deployment { connector: row.connector() },
    }
}

pub(crate) fn part_piece(bytes: &[u8], limits: &Limits) -> Result<List<c::Part>, w::Problem> {
    let mut reader = Reader::new(bytes);
    let mut rows = List::with_capacity(limits.client.rows);
    for _ in 0..=limits.client.rows {
        let item = match skein_codec::piece::split(&mut reader) {
            Ok(value) => value,
            Err(problem) => return Err(piece_problem(problem)),
        };
        let Some(item) = item else { return Ok(rows) };
        let row = w::Part::decode(&limits.wire, &mut Reader::new(&item))?;
        let row = part(&row, limits);
        if rows.push(row).is_err() {
            return Err(too_large());
        }
    }
    Err(too_large())
}

pub(crate) fn wake_policy(value: &w::WakePolicy, limits: &Limits) -> Result<c::WakePolicy, w::Problem> {
    let mut list0 = List::with_capacity(value.topics().len());
    for item in value.topics() {
        list0.push(*item).expect("reserved supplied list");
    }
    Ok(c::WakePolicy {
        words: value.words(),
        results: value.results(),
        topics: list0,
        subscriptions: part_piece(value.subscriptions(), limits)?,
    })
}

pub(crate) fn recipient(value: &w::Recipient, _limits: &Limits) -> c::Recipient {
    match value {
        w::Recipient::Task(row) => c::Recipient::Task(row.value()),
        w::Recipient::Party(row) => c::Recipient::Party(row.value()),
        w::Recipient::Role(row) => c::Recipient::Role { project: row.project(), role: row.role() },
        w::Recipient::Roles(row) => {
            let mut list0 = List::with_capacity(row.roles().len());
            for item in row.roles() {
                list0.push(*item).expect("reserved supplied list");
            }
            c::Recipient::Roles { project: row.project(), roles: list0 }
        }
    }
}

pub(crate) fn taken_by(value: &w::TakenBy, limits: &Limits) -> Result<c::TakenBy, w::Problem> {
    let opt0 = match &value.name() {
        Some(item) => Some(match c::Text::new(bytes::copy_of(item), limits.client.text) {
            Ok(value) => value,
            Err(problem) => return Err(text_problem(problem)),
        }),
        None => None,
    };
    Ok(c::TakenBy { number: value.number(), name: opt0 })
}

pub(crate) fn revision(value: &w::Revision, _limits: &Limits) -> c::Revision {
    c::Revision { value: value.value() }
}
