//! Core rows carried by their generated family (protocol/store.md, 5.1).
use crate::core_v6 as wire;
use alloc::boxed::Box;
use jig_domain_authority as authority;
use jig_domain_core as domain;
use jig_domain_fleet as fleet;
use jig_domain_notes as notes;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_lib::{List, Reader, Wall, Writer};
mod calls;
mod projections;
mod runs;
/// Family bounds used by the core and its carried task and note values.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub core: wire::Limits,
    pub tasks: crate::tasks_v9::Limits,
    pub notes: crate::notes_v1::Limits,
}
#[must_use]
pub fn limits(domain: &domain::Limits) -> Option<Limits> {
    let tasks = crate::codec::tasks::limits(&domain.tasks)?;
    let notes = crate::codec::notes::limits(&domain.notes)?;
    let core = wire::Limits {
        phase_bytes_value: u32::try_from(crate::tasks_v9::PhaseRecord::worst_case_bytes(&tasks)?).ok()?,
        change_bytes_value: u32::try_from(crate::tasks_v9::ChangeRecord::worst_case_bytes(&tasks)?).ok()?,
        end_bytes_value: u32::try_from(crate::tasks_v9::EndRecord::worst_case_bytes(&tasks)?).ok()?,
        projection_goal_words: domain.tasks.spec_bytes,
        projection_task_words: domain.tasks.spec_bytes,
        projection_milestone_words: domain.tasks.message_bytes.max(domain.tasks.result_bytes),
        projection_goal_row_pending: domain.connectors,
        run_proof_accounts: tasks::MODELS,
        run_proof_credentials: domain.accounts.credentials,
        run_proof_outstanding: domain.tasks.run_inbox,
        turn_record_transcript: wire::CEILINGS.turn_record_transcript,

        escalation_decision_reject_reason: domain.escalation_reason_bytes.max(domain.people.words),
        settled_answer_host_body: domain.call_answer_bytes,
        settled_call_tool: domain.call_answer_bytes,
        call_part_effect_denied_findings: jig_domain_authority::max_out(&domain.authority)?,
        call_part_tool_denied_findings: jig_domain_authority::max_out(&domain.authority)?,
        call_part_delegated_value: domain.tasks.batch,
        call_part_delegation_denied_findings: jig_domain_authority::max_out(&domain.authority)?,
        call_part_note_recalled_entries: domain.notes.recalled,
        task_bytes_value: u32::try_from(crate::tasks_v9::TaskProblem::worst_case_bytes(&tasks)?).ok()?,
        note_bytes_value: u32::try_from(crate::notes_v1::Row::worst_case_bytes(&notes)?).ok()?,
    };
    wire::check_limits(&core).ok()?;
    Some(Limits { core, tasks, notes })
}
pub fn encode(row: &domain::CoreRecord, limits: &Limits) -> Result<Box<[u8]>, wire::Problem> {
    let value = match row {
        domain::CoreRecord::Call(call) => wire::Value::Call(calls::encode_call_record(call, limits)?),
        domain::CoreRecord::Settings(_) => {
            return Err(wire::Problem { path: wire::Path::ValueTag, reason: skein_codec::Reason::Bound });
        }
        domain::CoreRecord::Deployment(value) => wire::Value::Deployment(runs::encode_deployment(value, limits)?),
        domain::CoreRecord::Projection(value) => wire::Value::Projection(wire::ProjectionValue::new(
            &limits.core,
            wire::ProjectionValueParts { value: projections::encode_projection_record(value, limits)? },
        )?),
        domain::CoreRecord::RunProof(value) => wire::Value::RunProof(runs::encode_run_proof(value, limits)?),
        domain::CoreRecord::Turn(value) => wire::Value::Turn(runs::encode_turn_record(value, limits)?),
        domain::CoreRecord::Terminal(value) => wire::Value::Terminal(runs::encode_terminal_record(value, limits)?),
        domain::CoreRecord::ProposalDecision(value) => {
            wire::Value::ProposalDecision(runs::encode_proposal_decision_record(value, limits)?)
        }
        domain::CoreRecord::EscalationDecision(value) => {
            wire::Value::EscalationDecision(runs::encode_escalation_decision_record(value, limits)?)
        }
    };
    let row = wire::Row::new(&limits.core, wire::RowParts { value })?;
    let mut writer = Writer::new(usize::try_from(row.measure()).expect("measured length"));
    row.encode(&mut writer).expect("measured room");
    Ok(writer.finish())
}
pub fn decode(bytes: &[u8], limits: &Limits) -> Result<domain::CoreRecord, wire::Problem> {
    let wire::RowParts { value } = wire::Row::decode(&limits.core, &mut Reader::new(bytes))?.into_parts();
    Ok(match value {
        wire::Value::Call(value) => domain::CoreRecord::Call(calls::decode_call_record(value, limits)?),
        wire::Value::Deployment(value) => domain::CoreRecord::Deployment(runs::decode_deployment(value, limits)),
        wire::Value::Projection(value) => {
            domain::CoreRecord::Projection(projections::decode_projection_record(value.into_parts().value, limits)?)
        }
        wire::Value::RunProof(value) => domain::CoreRecord::RunProof(runs::decode_run_proof(value, limits)?),
        wire::Value::Turn(value) => domain::CoreRecord::Turn(runs::decode_turn_record(value, limits)),
        wire::Value::Terminal(value) => domain::CoreRecord::Terminal(runs::decode_terminal_record(value, limits)?),
        wire::Value::ProposalDecision(value) => {
            domain::CoreRecord::ProposalDecision(runs::decode_proposal_decision_record(value, limits))
        }
        wire::Value::EscalationDecision(value) => {
            domain::CoreRecord::EscalationDecision(runs::decode_escalation_decision_record(value, limits))
        }
    })
}
fn problem(reason: skein_codec::Reason) -> wire::Problem {
    wire::Problem { path: wire::Path::RowValue, reason }
}
fn task_problem(value: crate::tasks_v9::Problem) -> wire::Problem {
    wire::Problem { path: wire::Path::TaskBytesValue, reason: value.reason }
}
fn note_problem(value: crate::notes_v1::Problem) -> wire::Problem {
    wire::Problem { path: wire::Path::NoteBytesValue, reason: value.reason }
}
fn too_large(_: core::num::TryFromIntError) -> wire::Problem {
    problem(skein_codec::Reason::Bound)
}
pub fn encode_proposer(value: &people::Proposer, limits: &Limits) -> Result<wire::Proposer, wire::Problem> {
    Ok(match value {
        people::Proposer::Task(value) => {
            wire::Proposer::Task(wire::ProposerTask::new(&limits.core, wire::ProposerTaskParts { value: *value })?)
        }
        people::Proposer::Party(value) => {
            wire::Proposer::Party(wire::ProposerParty::new(&limits.core, wire::ProposerPartyParts { value: *value })?)
        }
    })
}
#[must_use]
pub fn decode_proposer(value: wire::Proposer, _limits: &Limits) -> people::Proposer {
    match value {
        wire::Proposer::Task(value) => {
            let wire::ProposerTaskParts { value } = value.into_parts();
            people::Proposer::Task(value)
        }
        wire::Proposer::Party(value) => {
            let wire::ProposerPartyParts { value } = value.into_parts();
            people::Proposer::Party(value)
        }
    }
}

#[must_use]
pub fn encode_proposal_choice(value: &people::ProposalChoice, _limits: &Limits) -> wire::ProposalChoice {
    match value {
        people::ProposalChoice::Accepted => wire::ProposalChoice::Accepted,
        people::ProposalChoice::Rejected => wire::ProposalChoice::Rejected,
        people::ProposalChoice::Passed => wire::ProposalChoice::Passed,
        people::ProposalChoice::Withdrawn => wire::ProposalChoice::Withdrawn,
        people::ProposalChoice::Stale => wire::ProposalChoice::Stale,
    }
}
#[must_use]
pub fn decode_proposal_choice(value: wire::ProposalChoice, _limits: &Limits) -> people::ProposalChoice {
    match value {
        wire::ProposalChoice::Accepted => people::ProposalChoice::Accepted,
        wire::ProposalChoice::Rejected => people::ProposalChoice::Rejected,
        wire::ProposalChoice::Passed => people::ProposalChoice::Passed,
        wire::ProposalChoice::Withdrawn => people::ProposalChoice::Withdrawn,
        wire::ProposalChoice::Stale => people::ProposalChoice::Stale,
    }
}

pub fn encode_escalation_decision(
    value: &people::EscalationDecision,
    limits: &Limits,
) -> Result<wire::EscalationDecision, wire::Problem> {
    Ok(match value {
        people::EscalationDecision::Release => wire::EscalationDecision::Release,
        people::EscalationDecision::Reject { reason } => {
            wire::EscalationDecision::Reject(wire::EscalationDecisionReject::new(
                &limits.core,
                wire::EscalationDecisionRejectParts { reason: reason.clone() },
            )?)
        }
        people::EscalationDecision::Pass => wire::EscalationDecision::Pass,
    })
}
#[must_use]
pub fn decode_escalation_decision(value: wire::EscalationDecision, _limits: &Limits) -> people::EscalationDecision {
    match value {
        wire::EscalationDecision::Release => people::EscalationDecision::Release,
        wire::EscalationDecision::Reject(value) => {
            let wire::EscalationDecisionRejectParts { reason } = value.into_parts();
            people::EscalationDecision::Reject { reason }
        }
        wire::EscalationDecision::Pass => people::EscalationDecision::Pass,
    }
}

#[must_use]
pub fn encode_proposal_kind(value: &tasks::ProposalKind, _limits: &Limits) -> wire::ProposalKind {
    match value {
        tasks::ProposalKind::Settings => wire::ProposalKind::Settings,
        tasks::ProposalKind::Batch => wire::ProposalKind::Batch,
        tasks::ProposalKind::Effect => wire::ProposalKind::Effect,
        tasks::ProposalKind::Amend => wire::ProposalKind::Amend,
        tasks::ProposalKind::Widen => wire::ProposalKind::Widen,
        tasks::ProposalKind::Release => wire::ProposalKind::Release,
    }
}
#[must_use]
pub fn decode_proposal_kind(value: wire::ProposalKind, _limits: &Limits) -> tasks::ProposalKind {
    match value {
        wire::ProposalKind::Settings => tasks::ProposalKind::Settings,
        wire::ProposalKind::Batch => tasks::ProposalKind::Batch,
        wire::ProposalKind::Effect => tasks::ProposalKind::Effect,
        wire::ProposalKind::Amend => tasks::ProposalKind::Amend,
        wire::ProposalKind::Widen => tasks::ProposalKind::Widen,
        wire::ProposalKind::Release => tasks::ProposalKind::Release,
    }
}

pub fn encode_escalation_outcome(
    value: &tasks::EscalationOutcome,
    limits: &Limits,
) -> Result<wire::EscalationOutcome, wire::Problem> {
    Ok(match value {
        tasks::EscalationOutcome::Released => wire::EscalationOutcome::Released,
        tasks::EscalationOutcome::Rejected => wire::EscalationOutcome::Rejected,
        tasks::EscalationOutcome::Passed { revision } => {
            wire::EscalationOutcome::Passed(wire::EscalationOutcomePassed::new(
                &limits.core,
                wire::EscalationOutcomePassedParts { revision: *revision },
            )?)
        }
        tasks::EscalationOutcome::Stale => wire::EscalationOutcome::Stale,
        tasks::EscalationOutcome::NoFurther => wire::EscalationOutcome::NoFurther,
        tasks::EscalationOutcome::Limit => wire::EscalationOutcome::Limit,
    })
}
#[must_use]
pub fn decode_escalation_outcome(value: wire::EscalationOutcome, _limits: &Limits) -> tasks::EscalationOutcome {
    match value {
        wire::EscalationOutcome::Released => tasks::EscalationOutcome::Released,
        wire::EscalationOutcome::Rejected => tasks::EscalationOutcome::Rejected,
        wire::EscalationOutcome::Passed(value) => {
            let wire::EscalationOutcomePassedParts { revision } = value.into_parts();
            tasks::EscalationOutcome::Passed { revision }
        }
        wire::EscalationOutcome::Stale => tasks::EscalationOutcome::Stale,
        wire::EscalationOutcome::NoFurther => tasks::EscalationOutcome::NoFurther,
        wire::EscalationOutcome::Limit => tasks::EscalationOutcome::Limit,
    }
}

#[must_use]
pub fn encode_proposal_outcome(value: &tasks::ProposalOutcome, _limits: &Limits) -> wire::ProposalOutcome {
    match value {
        tasks::ProposalOutcome::Accepted => wire::ProposalOutcome::Accepted,
        tasks::ProposalOutcome::Rejected => wire::ProposalOutcome::Rejected,
        tasks::ProposalOutcome::Passed => wire::ProposalOutcome::Passed,
        tasks::ProposalOutcome::Withdrawn => wire::ProposalOutcome::Withdrawn,
        tasks::ProposalOutcome::Stale => wire::ProposalOutcome::Stale,
    }
}
#[must_use]
pub fn decode_proposal_outcome(value: wire::ProposalOutcome, _limits: &Limits) -> tasks::ProposalOutcome {
    match value {
        wire::ProposalOutcome::Accepted => tasks::ProposalOutcome::Accepted,
        wire::ProposalOutcome::Rejected => tasks::ProposalOutcome::Rejected,
        wire::ProposalOutcome::Passed => tasks::ProposalOutcome::Passed,
        wire::ProposalOutcome::Withdrawn => tasks::ProposalOutcome::Withdrawn,
        wire::ProposalOutcome::Stale => tasks::ProposalOutcome::Stale,
    }
}

#[must_use]
pub fn encode_source(value: &authority::Source, _limits: &Limits) -> wire::Source {
    match value {
        authority::Source::Task => wire::Source::Task,
        authority::Source::Role => wire::Source::Role,
        authority::Source::Project => wire::Source::Project,
        authority::Source::Deployment => wire::Source::Deployment,
    }
}
#[must_use]
pub fn decode_source(value: wire::Source, _limits: &Limits) -> authority::Source {
    match value {
        wire::Source::Task => authority::Source::Task,
        wire::Source::Role => authority::Source::Role,
        wire::Source::Project => authority::Source::Project,
        wire::Source::Deployment => authority::Source::Deployment,
    }
}

pub fn encode_lacks(value: &authority::Lacks, limits: &Limits) -> Result<wire::Lacks, wire::Problem> {
    let authority::Lacks { tools, grants, executors, tasks, depth, spend, deadline, notes, models } = value;
    wire::Lacks::new(
        &limits.core,
        wire::LacksParts {
            skein_bools: [*tools, *grants, *executors, *tasks, *depth, *spend, *deadline, *notes, *models],
        },
    )
}
#[must_use]
pub fn decode_lacks(value: wire::Lacks, _limits: &Limits) -> authority::Lacks {
    let wire::LacksParts { skein_bools: [tools, grants, executors, tasks, depth, spend, deadline, notes, models] } =
        value.into_parts();
    authority::Lacks { tools, grants, executors, tasks, depth, spend, deadline, notes, models }
}

pub fn encode_judge(value: &authority::Judge, limits: &Limits) -> Result<wire::Judge, wire::Problem> {
    let authority::Judge { connector, requirement, parameters } = value;
    wire::Judge::new(
        &limits.core,
        wire::JudgeParts { connector: *connector, requirement: *requirement, parameters: *parameters },
    )
}
#[must_use]
pub fn decode_judge(value: wire::Judge, _limits: &Limits) -> authority::Judge {
    let wire::JudgeParts { connector, requirement, parameters } = value.into_parts();
    authority::Judge { connector, requirement, parameters }
}

#[must_use]
pub fn encode_answer(value: &authority::Answer, _limits: &Limits) -> wire::Answer {
    match value {
        authority::Answer::Allow => wire::Answer::Allow,
        authority::Answer::Wait => wire::Answer::Wait,
        authority::Answer::Propose => wire::Answer::Propose,
        authority::Answer::Refuse => wire::Answer::Refuse,
    }
}
#[must_use]
pub fn decode_answer(value: wire::Answer, _limits: &Limits) -> authority::Answer {
    match value {
        wire::Answer::Allow => authority::Answer::Allow,
        wire::Answer::Wait => authority::Answer::Wait,
        wire::Answer::Propose => authority::Answer::Propose,
        wire::Answer::Refuse => authority::Answer::Refuse,
    }
}

pub fn encode_finding(value: &authority::Finding, limits: &Limits) -> Result<wire::Finding, wire::Problem> {
    Ok(match value {
        authority::Finding::Oversized => wire::Finding::Oversized,
        authority::Finding::UnknownProject => wire::Finding::UnknownProject,
        authority::Finding::UnknownRole => wire::Finding::UnknownRole,
        authority::Finding::Authority { source, lacks } => wire::Finding::Authority(wire::FindingAuthority::new(
            &limits.core,
            wire::FindingAuthorityParts { source: encode_source(source, limits), lacks: encode_lacks(lacks, limits)? },
        )?),
        authority::Finding::Executor { source } => wire::Finding::Executor(wire::FindingExecutor::new(
            &limits.core,
            wire::FindingExecutorParts { source: encode_source(source, limits) },
        )?),
        authority::Finding::Tasks { source } => wire::Finding::Tasks(wire::FindingTasks::new(
            &limits.core,
            wire::FindingTasksParts { source: encode_source(source, limits) },
        )?),
        authority::Finding::Spend { source } => wire::Finding::Spend(wire::FindingSpend::new(
            &limits.core,
            wire::FindingSpendParts { source: encode_source(source, limits) },
        )?),
        authority::Finding::Arithmetic => wire::Finding::Arithmetic,
        authority::Finding::RunBudget => wire::Finding::RunBudget,
        authority::Finding::RunCap => wire::Finding::RunCap,
        authority::Finding::Deadline => wire::Finding::Deadline,
        authority::Finding::Account => wire::Finding::Account,
        authority::Finding::Writer => wire::Finding::Writer,
        authority::Finding::Tool => wire::Finding::Tool,
        authority::Finding::Grant { source } => wire::Finding::Grant(wire::FindingGrant::new(
            &limits.core,
            wire::FindingGrantParts { source: encode_source(source, limits) },
        )?),
        authority::Finding::Price { source } => wire::Finding::Price(wire::FindingPrice::new(
            &limits.core,
            wire::FindingPriceParts { source: encode_source(source, limits) },
        )?),
        authority::Finding::ResourceAccess => wire::Finding::ResourceAccess,
        authority::Finding::ResourceUnavailable => wire::Finding::ResourceUnavailable,
        authority::Finding::Reference => wire::Finding::Reference,
        authority::Finding::Scope { source } => wire::Finding::Scope(wire::FindingScope::new(
            &limits.core,
            wire::FindingScopeParts { source: encode_source(source, limits) },
        )?),
        authority::Finding::Required { judge } => wire::Finding::Required(wire::FindingRequired::new(
            &limits.core,
            wire::FindingRequiredParts { judge: encode_judge(judge, limits)? },
        )?),
        authority::Finding::Failed { judge } => wire::Finding::Failed(wire::FindingFailed::new(
            &limits.core,
            wire::FindingFailedParts { judge: encode_judge(judge, limits)? },
        )?),
        authority::Finding::Unguarded { judge } => wire::Finding::Unguarded(wire::FindingUnguarded::new(
            &limits.core,
            wire::FindingUnguardedParts { judge: encode_judge(judge, limits)? },
        )?),
        authority::Finding::Unpermitted => wire::Finding::Unpermitted,
        authority::Finding::Undecidable => wire::Finding::Undecidable,
        authority::Finding::PeriodSpend => wire::Finding::PeriodSpend,
    })
}
#[must_use]
pub fn decode_finding(value: wire::Finding, limits: &Limits) -> authority::Finding {
    match value {
        wire::Finding::Oversized => authority::Finding::Oversized,
        wire::Finding::UnknownProject => authority::Finding::UnknownProject,
        wire::Finding::UnknownRole => authority::Finding::UnknownRole,
        wire::Finding::Authority(value) => {
            let wire::FindingAuthorityParts { source, lacks } = value.into_parts();
            authority::Finding::Authority { source: decode_source(source, limits), lacks: decode_lacks(lacks, limits) }
        }
        wire::Finding::Executor(value) => {
            let wire::FindingExecutorParts { source } = value.into_parts();
            authority::Finding::Executor { source: decode_source(source, limits) }
        }
        wire::Finding::Tasks(value) => {
            let wire::FindingTasksParts { source } = value.into_parts();
            authority::Finding::Tasks { source: decode_source(source, limits) }
        }
        wire::Finding::Spend(value) => {
            let wire::FindingSpendParts { source } = value.into_parts();
            authority::Finding::Spend { source: decode_source(source, limits) }
        }
        wire::Finding::Arithmetic => authority::Finding::Arithmetic,
        wire::Finding::RunBudget => authority::Finding::RunBudget,
        wire::Finding::RunCap => authority::Finding::RunCap,
        wire::Finding::Deadline => authority::Finding::Deadline,
        wire::Finding::Account => authority::Finding::Account,
        wire::Finding::Writer => authority::Finding::Writer,
        wire::Finding::Tool => authority::Finding::Tool,
        wire::Finding::Grant(value) => {
            let wire::FindingGrantParts { source } = value.into_parts();
            authority::Finding::Grant { source: decode_source(source, limits) }
        }
        wire::Finding::Price(value) => {
            let wire::FindingPriceParts { source } = value.into_parts();
            authority::Finding::Price { source: decode_source(source, limits) }
        }
        wire::Finding::ResourceAccess => authority::Finding::ResourceAccess,
        wire::Finding::ResourceUnavailable => authority::Finding::ResourceUnavailable,
        wire::Finding::Reference => authority::Finding::Reference,
        wire::Finding::Scope(value) => {
            let wire::FindingScopeParts { source } = value.into_parts();
            authority::Finding::Scope { source: decode_source(source, limits) }
        }
        wire::Finding::Required(value) => {
            let wire::FindingRequiredParts { judge } = value.into_parts();
            authority::Finding::Required { judge: decode_judge(judge, limits) }
        }
        wire::Finding::Failed(value) => {
            let wire::FindingFailedParts { judge } = value.into_parts();
            authority::Finding::Failed { judge: decode_judge(judge, limits) }
        }
        wire::Finding::Unguarded(value) => {
            let wire::FindingUnguardedParts { judge } = value.into_parts();
            authority::Finding::Unguarded { judge: decode_judge(judge, limits) }
        }
        wire::Finding::Unpermitted => authority::Finding::Unpermitted,
        wire::Finding::Undecidable => authority::Finding::Undecidable,
        wire::Finding::PeriodSpend => authority::Finding::PeriodSpend,
    }
}

#[must_use]
pub fn encode_host_kind(value: &fleet::HostKind, _limits: &Limits) -> wire::HostKind {
    match value {
        fleet::HostKind::Worker => wire::HostKind::Worker,
        fleet::HostKind::Engine => wire::HostKind::Engine,
    }
}
#[must_use]
pub fn decode_host_kind(value: wire::HostKind, _limits: &Limits) -> fleet::HostKind {
    match value {
        wire::HostKind::Worker => fleet::HostKind::Worker,
        wire::HostKind::Engine => fleet::HostKind::Engine,
    }
}

pub fn encode_outbox_outcome(
    value: &domain::connector::OutboxOutcome,
    limits: &Limits,
) -> Result<wire::OutboxOutcome, wire::Problem> {
    Ok(match value {
        domain::connector::OutboxOutcome::Made => wire::OutboxOutcome::Made,
        domain::connector::OutboxOutcome::Failed => wire::OutboxOutcome::Failed,
        domain::connector::OutboxOutcome::Withdrawn => wire::OutboxOutcome::Withdrawn,
        domain::connector::OutboxOutcome::NotMade => wire::OutboxOutcome::NotMade,
        domain::connector::OutboxOutcome::Uncertain => wire::OutboxOutcome::Uncertain,
        domain::connector::OutboxOutcome::Held { entry } => wire::OutboxOutcome::Held(wire::OutboxOutcomeHeld::new(
            &limits.core,
            wire::OutboxOutcomeHeldParts { entry: *entry },
        )?),
    })
}
#[must_use]
pub fn decode_outbox_outcome(value: wire::OutboxOutcome, _limits: &Limits) -> domain::connector::OutboxOutcome {
    match value {
        wire::OutboxOutcome::Made => domain::connector::OutboxOutcome::Made,
        wire::OutboxOutcome::Failed => domain::connector::OutboxOutcome::Failed,
        wire::OutboxOutcome::Withdrawn => domain::connector::OutboxOutcome::Withdrawn,
        wire::OutboxOutcome::NotMade => domain::connector::OutboxOutcome::NotMade,
        wire::OutboxOutcome::Uncertain => domain::connector::OutboxOutcome::Uncertain,
        wire::OutboxOutcome::Held(value) => {
            let wire::OutboxOutcomeHeldParts { entry } = value.into_parts();
            domain::connector::OutboxOutcome::Held { entry }
        }
    }
}

#[must_use]
pub fn encode_delivery_outcome(value: &domain::DeliveryOutcome, _limits: &Limits) -> wire::DeliveryOutcome {
    match value {
        domain::DeliveryOutcome::Delivered => wire::DeliveryOutcome::Delivered,
        domain::DeliveryOutcome::Nothing => wire::DeliveryOutcome::Nothing,
        domain::DeliveryOutcome::Stale => wire::DeliveryOutcome::Stale,
        domain::DeliveryOutcome::Refused => wire::DeliveryOutcome::Refused,
        domain::DeliveryOutcome::Invalid => wire::DeliveryOutcome::Invalid,
        domain::DeliveryOutcome::Failed => wire::DeliveryOutcome::Failed,
    }
}
#[must_use]
pub fn decode_delivery_outcome(value: wire::DeliveryOutcome, _limits: &Limits) -> domain::DeliveryOutcome {
    match value {
        wire::DeliveryOutcome::Delivered => domain::DeliveryOutcome::Delivered,
        wire::DeliveryOutcome::Nothing => domain::DeliveryOutcome::Nothing,
        wire::DeliveryOutcome::Stale => domain::DeliveryOutcome::Stale,
        wire::DeliveryOutcome::Refused => domain::DeliveryOutcome::Refused,
        wire::DeliveryOutcome::Invalid => domain::DeliveryOutcome::Invalid,
        wire::DeliveryOutcome::Failed => domain::DeliveryOutcome::Failed,
    }
}

/// The versioned body stored in a transcript frame (protocol/store.md, 8.3).
pub fn turn(value: &domain::TurnRecord, limits: &Limits) -> Result<Box<[u8]>, wire::Problem> {
    let value = runs::encode_turn_record(value, limits)?;
    let mut writer = Writer::new(usize::try_from(value.measure()).expect("measured length"));
    value.encode(&mut writer).expect("measured room");
    Ok(writer.finish())
}
/// Decode a transcript frame's versioned core turn.
pub fn read_turn(bytes: &[u8], limits: &Limits) -> Result<domain::TurnRecord, wire::Problem> {
    Ok(runs::decode_turn_record(wire::TurnRecord::decode(&limits.core, &mut Reader::new(bytes))?, limits))
}
