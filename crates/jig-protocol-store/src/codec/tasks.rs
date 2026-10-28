//! Tasks rows translated to their generated family (protocol/store.md, 5.1).
pub mod activity;
pub mod carried;
pub mod index;
mod models;
pub mod phase;
mod pieces;
mod placed;
pub mod proposals;
pub mod record;
pub mod state;
pub mod value;
use crate::tasks_v9 as wire;
use alloc::boxed::Box;
use jig_domain_tasks as domain;
use skein_lib::{Reader, Writer};

/// Translate the tasks child's bounds, refusing any schema ceiling.
#[must_use]
pub fn limits(domain: &domain::Limits) -> Option<wire::Limits> {
    let empty = wire::Limits { field_rule_name: 0, result_field_name: 0, result_field_value: 0, ..wire::CEILINGS };
    let rules = domain.result_bytes.checked_mul(
        u32::try_from(
            wire::FieldRule::worst_case_bytes(&empty)?
                .checked_add(u64::try_from(size_of::<u32>()).expect("length size"))?
                .checked_add(1)?,
        )
        .ok()?,
    )?;
    let fields = domain.result_bytes.checked_mul(
        u32::try_from(
            wire::ResultField::worst_case_bytes(&empty)?
                .checked_add(u64::try_from(size_of::<u32>()).expect("length size"))?
                .checked_add(1)?,
        )
        .ok()?,
    )?;
    let limits = wire::Limits {
        placed_hold_reason: domain.message_bytes,
        placed_hold_exempt: domain.exemptions,
        proposed_settings_items: domain::SETTINGS_ITEMS,
        row_models: jig_domain_tasks::MODELS,
        model_name_value: jig_domain_core::MODEL_NAME_BYTES,
        person_proposal_reason: domain.message_bytes,
        grace_reason: domain.result_bytes,
        task_problem_blocked_by_some: domain.delegates.max(domain.dependencies).max(domain.circle),
        result_followups_delegates_value: domain.batch,
        segment_bytes: domain.hold_bytes,
        name_path: domain.hold_segments,
        bytes_parameter_value: domain.spec_bytes,
        spec_words: domain.spec_bytes,
        spec_parameters: domain.parameters,
        spec_inputs: domain.inputs,
        part_segment_bytes: domain.hold_bytes,
        part_path_segments: domain.hold_segments,
        field_rule_name: domain.result_bytes,
        verdict_fields: rules,
        approval_state: domain.result_bytes,
        choices_choices: domain.contract_choices,
        llm_choice_subagents: domain::MODELS,
        authority_segment_bytes: domain.authority_bytes,
        pattern_segments: domain.authority_segments,
        authority_grants: domain.authority_grants,
        authority_note_resources: domain.authority_grants,
        delegation_kinds: domain.executor_kinds,
        member_dependencies: domain.dependencies,
        member_holdings: domain.holdings,
        recurring_template_batch: domain.batch,
        result_field_name: domain.result_bytes,
        result_field_value: domain.result_bytes,
        approved_state: domain.result_bytes,
        approved_words_some: domain.result_bytes,
        report_result_words: domain.result_bytes,
        verdict_result_fields: fields,
        verdict_result_words: domain.result_bytes,
        change_result_words: domain.result_bytes,
        failure_result_reason: domain.result_bytes,
        cancelled_reason: domain.result_bytes,
        rejected_proposal_reason: domain.message_bytes,
        amendment_instructions_some: domain.message_bytes,
        amendment_dependencies_some: domain.dependencies,
        amendment_reason: domain.message_bytes,
        proposed_batch_members: domain.batch,
        proposal_reason: domain.message_bytes,
        settings_instructions_some: domain.message_bytes,
        settings_dependencies: domain.dependencies,
        history_reason: domain.result_bytes.max(domain.message_bytes),
        rejected_person_proposal_reason: domain.message_bytes,
        approval_message_state: domain.result_bytes,
        verdict_message_fields: fields,
        word_words: domain.message_bytes.max(domain.result_bytes),
        activity_words_words: domain.message_bytes,
        rejected_escalation_reason: domain.result_bytes,
        recurring_state_last_batch: domain.batch,
        saved_resource_path: domain.authority_segments,
        task_history_question_lines: domain.inbox_messages,
        task_history_proposal_rows: domain.inbox_messages,
        task_history_instructions_some: domain.message_bytes,
        task_history_refused: domain.inbox_messages,
        task_run_replies: domain.inbox_messages,
        task_definition_dependencies: domain.dependencies,
        task_definition_waiting_on: domain.dependencies,
        task_definition_added: domain.dependencies,
        circle_tasks: domain.circle,
        kept_line_pending: domain.dependencies,
        task_interests_holdings: domain.holdings,
        task_interests_delegates: domain.delegates,
        task_interests_result_followups: domain.batch,
        task_interests_references: domain.references,
        task_interests_subscriptions: domain.subscriptions,
        task_interests_inbox: domain.inbox_messages.checked_add(2)?,
        task_interests_saved: domain.saved_resources,
    };
    wire::check_limits(&limits).ok()?;
    Some(limits)
}

/// Encode one tasks row within the configured bounds.
pub fn encode(row: &domain::Stored, limits: &wire::Limits, names: &[Box<[u8]>]) -> Result<Box<[u8]>, wire::Problem> {
    let (models, moved) = models::encode(row, names, limits)?;
    let value = match row {
        domain::Stored::PlacedHold(row) => wire::Value::PlacedHold(placed::encode(row, limits)?),
        domain::Stored::Ledger(row) => wire::Value::Ledger(value::funding(row, limits)?),
        domain::Stored::ClosedLedger(row) => wire::Value::ClosedLedger(value::funding(row, limits)?),
        domain::Stored::Writer(domain::WriterSlot { number, resource, writer, lost }) => {
            let writer = match writer {
                domain::Writer::Run { task, attempt } => {
                    wire::Writer::Run(wire::Run::new(limits, wire::RunParts { task: *task, attempt: *attempt })?)
                }
                domain::Writer::Effect { entry } => {
                    wire::Writer::Effect(wire::Effect::new(limits, wire::EffectParts { entry: *entry })?)
                }
            };
            wire::Value::Writer(wire::WriterSlot::new(
                limits,
                wire::WriterSlotParts {
                    number: *number,
                    resource: value::name(resource, limits)?,
                    writer,
                    lost: *lost,
                },
            )?)
        }
        domain::Stored::Pool(domain::PoolSlots { number, pool, slots }) => wire::Value::Pool(wire::PoolSlots::new(
            limits,
            wire::PoolSlotsParts { number: *number, pool: value::name(pool, limits)?, slots: *slots },
        )?),
        domain::Stored::History(row) => wire::Value::History(Box::new(proposals::history(row, limits, Some(&moved))?)),
        domain::Stored::Milestone(row) => wire::Value::Milestone(proposals::milestone(row, limits)?),
        domain::Stored::PersonProposal(row) => {
            wire::Value::PersonProposal(proposals::person_proposal(row, limits, Some(&moved))?)
        }
        domain::Stored::DecidedProposal(row) => {
            wire::Value::DecidedProposal(proposals::person_proposal(row, limits, Some(&moved))?)
        }
        domain::Stored::Update(row) => wire::Value::Update(index::update(row, limits)?),
        domain::Stored::Result(row) => wire::Value::Result(index::result_row(row, limits)?),
        domain::Stored::EndedDelegate(row) => wire::Value::EndedDelegate(index::edge(row, limits)?),
        domain::Stored::EndedByParty(row) => wire::Value::EndedByParty(index::ended_by_party(row, limits)?),
        domain::Stored::EndedByProject(row) => wire::Value::EndedByProject(index::ended_by_project(row, limits)?),
        domain::Stored::Delegate(row) => wire::Value::Delegate(index::edge(row, limits)?),
        domain::Stored::Activity(row) => wire::Value::Activity(activity::activity_line(row, limits)?),
        domain::Stored::Live(row) => wire::Value::Live(record::task_record(row, limits, Some(&moved))?),
        domain::Stored::Ended(row) => wire::Value::Ended(record::task_record(row, limits, Some(&moved))?),
        domain::Stored::Stub(row) => wire::Value::Stub(state::stub(row, limits)?),
    };
    let row = wire::Row::new(limits, wire::RowParts { value, models })?;
    let mut writer = Writer::new(usize::try_from(row.measure()).expect("encoded row fits usize"));
    row.encode(&mut writer).expect("measured room");
    Ok(writer.finish())
}

/// Decode one tasks row, refusing another version or trailing bytes.
pub fn decode(bytes: &[u8], limits: &wire::Limits, names: &[Box<[u8]>]) -> Result<domain::Stored, wire::Problem> {
    let wire::RowParts { value, models: kept } = wire::Row::decode(limits, &mut Reader::new(bytes))?.into_parts();
    let mut row = match value {
        wire::Value::PlacedHold(row) => domain::Stored::PlacedHold(Box::new(placed::decode(row))),
        wire::Value::Update(row) => domain::Stored::Update(index::read_update(row, limits)?),
        wire::Value::Result(row) => domain::Stored::Result(index::read_result_row(row, limits)?),
        wire::Value::EndedDelegate(row) => domain::Stored::EndedDelegate(index::read_edge(row, limits)?),
        wire::Value::EndedByParty(row) => domain::Stored::EndedByParty(index::read_ended_by_party(row)),
        wire::Value::EndedByProject(row) => domain::Stored::EndedByProject(index::read_ended_by_project(row)),
        wire::Value::Delegate(row) => domain::Stored::Delegate(index::read_edge(row, limits)?),
        wire::Value::Activity(row) => domain::Stored::Activity(activity::read_activity_line(row, limits)?),
        wire::Value::Live(row) => domain::Stored::Live(Box::new(record::read_task_record(row, limits)?)),
        wire::Value::Ended(row) => domain::Stored::Ended(Box::new(record::read_task_record(row, limits)?)),
        wire::Value::Stub(row) => domain::Stored::Stub(state::read_stub(row, limits)?),
        wire::Value::Ledger(value) => domain::Stored::Ledger(value::read_funding(value)),
        wire::Value::History(value) => domain::Stored::History(proposals::read_history(*value, limits)?),
        wire::Value::Milestone(value) => domain::Stored::Milestone(proposals::read_milestone(value, limits)?),
        wire::Value::PersonProposal(value) => {
            domain::Stored::PersonProposal(Box::new(proposals::read_person_proposal(value, limits)?))
        }
        wire::Value::DecidedProposal(value) => {
            domain::Stored::DecidedProposal(Box::new(proposals::read_person_proposal(value, limits)?))
        }
        wire::Value::ClosedLedger(value) => domain::Stored::ClosedLedger(value::read_funding(value)),
        wire::Value::Writer(value) => {
            let wire::WriterSlotParts { number, resource, writer, lost } = value.into_parts();
            let writer = match writer {
                wire::Writer::Run(value) => {
                    let wire::RunParts { task, attempt } = value.into_parts();
                    domain::Writer::Run { task, attempt }
                }
                wire::Writer::Effect(value) => domain::Writer::Effect { entry: value.into_parts().entry },
            };
            domain::Stored::Writer(domain::WriterSlot {
                number,
                resource: value::read_name(resource, limits)?,
                writer,
                lost,
            })
        }
        wire::Value::Pool(value) => {
            let wire::PoolSlotsParts { number, pool, slots } = value.into_parts();
            domain::Stored::Pool(domain::PoolSlots { number, pool: value::read_name(pool, limits)?, slots })
        }
    };
    models::remap(&mut row, &models::moves(&kept, names));
    Ok(row)
}
