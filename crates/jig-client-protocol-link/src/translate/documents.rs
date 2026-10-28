//! Whole document decoding (protocol/client.md, 3; client/domain/contract.md, 5–7).
use super::{Undecodable, piece_problem, too_large};
use crate::Limits;
use alloc::boxed::Box;
use jig_client_domain::contract as c;
use jig_client_protocol_wire::v19 as w;
use skein_lib::{List, Reader, Wall, bytes};
#[path = "accounts.rs"]
mod accounts;
#[path = "catalogue.rs"]
mod catalogue;
#[path = "documents_part0.rs"]
mod part0;
pub(crate) use accounts::*;
pub(crate) use part0::*;
#[path = "documents_part1.rs"]
mod part1;
pub(crate) use part1::*;
#[path = "documents_part2.rs"]
mod part2;
pub(crate) use part2::*;
#[path = "documents_part3.rs"]
mod part3;
pub(crate) use part3::*;
#[path = "documents_part4.rs"]
mod part4;
pub(crate) use part4::*;
#[path = "documents_part5.rs"]
mod part5;
pub(crate) use part5::*;
#[path = "documents_part6.rs"]
mod part6;
pub(crate) use part6::*;
#[path = "documents_part7.rs"]
mod part7;
pub(crate) use part7::*;
#[path = "documents_part8.rs"]
mod part8;
pub(crate) use part8::*;

fn text(value: &[u8], limits: &Limits) -> Result<c::Text, w::Problem> {
    match c::Text::new(bytes::copy_of(value), limits.client.text) {
        Ok(value) => Ok(value),
        Err(_) => Err(too_large()),
    }
}
fn raw(value: &[u8], limits: &Limits) -> Result<Box<[u8]>, w::Problem> {
    if u64::try_from(value.len()).expect("usize fits u64") > u64::from(limits.client.text) {
        return Err(too_large());
    }
    Ok(bytes::copy_of(value))
}
pub(crate) fn classify(problem: w::Problem) -> Undecodable {
    match problem.reason {
        skein_codec::Reason::Bound => Undecodable::TooLarge,
        skein_codec::Reason::Malformed
        | skein_codec::Reason::Tag
        | skein_codec::Reason::Utf8
        | skein_codec::Reason::Short
        | skein_codec::Reason::Bool
        | skein_codec::Reason::Version
        | skein_codec::Reason::Trailing => Undecodable::Malformed,
    }
}

pub fn frame(record: w::Me, limits: &Limits) -> Result<c::Me, Undecodable> {
    if !crate::limits::valid(limits) || u64::from(record.measure()) > limits.client.document_bytes {
        return Err(Undecodable::TooLarge);
    }
    let checked = receiving(limits)?;
    let limits = &checked;
    let record = match w::Me::new(&limits.wire, record.into_parts()) {
        Ok(value) => value,
        Err(problem) => return Err(classify(problem)),
    };
    match frame_fields(&record, limits) {
        Ok(value) => Ok(value),
        Err(problem) => Err(classify(problem)),
    }
}

pub fn task(record: w::Task, limits: &Limits) -> Result<c::Task, Undecodable> {
    if !crate::limits::valid(limits) || u64::from(record.measure()) > limits.client.document_bytes {
        return Err(Undecodable::TooLarge);
    }
    let checked = receiving(limits)?;
    let limits = &checked;
    let record = match w::Task::new(&limits.wire, record.into_parts()) {
        Ok(value) => value,
        Err(problem) => return Err(classify(problem)),
    };
    match task_fields(&record, limits) {
        Ok(value) => Ok(value),
        Err(problem) => Err(classify(problem)),
    }
}

pub fn tree(record: w::Tree, limits: &Limits) -> Result<c::Tree, Undecodable> {
    if !crate::limits::valid(limits) || u64::from(record.measure()) > limits.client.document_bytes {
        return Err(Undecodable::TooLarge);
    }
    let checked = receiving(limits)?;
    let limits = &checked;
    let record = match w::Tree::new(&limits.wire, record.into_parts()) {
        Ok(value) => value,
        Err(problem) => return Err(classify(problem)),
    };
    match tree_fields(&record, limits) {
        Ok(value) => Ok(value),
        Err(problem) => Err(classify(problem)),
    }
}

pub fn inbox(record: w::Inbox, limits: &Limits) -> Result<c::Inbox, Undecodable> {
    if !crate::limits::valid(limits) || u64::from(record.measure()) > limits.client.document_bytes {
        return Err(Undecodable::TooLarge);
    }
    let checked = receiving(limits)?;
    let limits = &checked;
    let record = match w::Inbox::new(&limits.wire, record.into_parts()) {
        Ok(value) => value,
        Err(problem) => return Err(classify(problem)),
    };
    match inbox_fields(&record, limits) {
        Ok(value) => Ok(value),
        Err(problem) => Err(classify(problem)),
    }
}

pub fn tasks(record: w::Tasks, limits: &Limits) -> Result<c::Tasks, Undecodable> {
    if !crate::limits::valid(limits) || u64::from(record.measure()) > limits.client.document_bytes {
        return Err(Undecodable::TooLarge);
    }
    let checked = receiving(limits)?;
    let limits = &checked;
    let record = match w::Tasks::new(&limits.wire, record.into_parts()) {
        Ok(value) => value,
        Err(problem) => return Err(classify(problem)),
    };
    match tasks_fields(&record, limits) {
        Ok(value) => Ok(value),
        Err(problem) => Err(classify(problem)),
    }
}

pub fn deployment(record: w::Deployment, limits: &Limits) -> Result<c::Deployment, Undecodable> {
    if !crate::limits::valid(limits) || u64::from(record.measure()) > limits.client.document_bytes {
        return Err(Undecodable::TooLarge);
    }
    let checked = receiving(limits)?;
    let limits = &checked;
    let record = match w::Deployment::new(&limits.wire, record.into_parts()) {
        Ok(value) => value,
        Err(problem) => return Err(classify(problem)),
    };
    match deployment_fields(&record, limits) {
        Ok(value) => Ok(value),
        Err(problem) => Err(classify(problem)),
    }
}

pub fn owner_deployment(record: w::OwnerDeployment, limits: &Limits) -> Result<c::OwnerDeployment, Undecodable> {
    if !crate::limits::valid(limits) || u64::from(record.measure()) > limits.client.document_bytes {
        return Err(Undecodable::TooLarge);
    }
    let checked = receiving(limits)?;
    let limits = &checked;
    let record = match w::OwnerDeployment::new(&limits.wire, record.into_parts()) {
        Ok(value) => value,
        Err(problem) => return Err(classify(problem)),
    };
    match owner_deployment_fields(&record, limits) {
        Ok(value) => Ok(value),
        Err(problem) => Err(classify(problem)),
    }
}

pub fn owner_project(record: w::OwnerProject, limits: &Limits) -> Result<c::OwnerProject, Undecodable> {
    if !crate::limits::valid(limits) || u64::from(record.measure()) > limits.client.document_bytes {
        return Err(Undecodable::TooLarge);
    }
    let checked = receiving(limits)?;
    let limits = &checked;
    let record = match w::OwnerProject::new(&limits.wire, record.into_parts()) {
        Ok(value) => value,
        Err(problem) => return Err(classify(problem)),
    };
    match owner_project_fields(&record, limits) {
        Ok(value) => Ok(value),
        Err(problem) => Err(classify(problem)),
    }
}

pub fn chip(record: w::Chip, limits: &Limits) -> Result<c::Chip, Undecodable> {
    if !crate::limits::valid(limits) || u64::from(record.measure()) > limits.client.document_bytes {
        return Err(Undecodable::TooLarge);
    }
    let checked = receiving(limits)?;
    match chip_fields(&record, &checked) {
        Ok(value) => Ok(value),
        Err(problem) => Err(classify(problem)),
    }
}

pub(crate) fn receiving(limits: &Limits) -> Result<Limits, Undecodable> {
    let mut checked = *limits;
    checked.wire.task_dependency_facts = limits.client.rows.min(w::CEILINGS.task_dependency_facts);
    checked.wire.owner_chip_details_fields_dependencies =
        limits.client.rows.min(w::CEILINGS.owner_chip_details_fields_dependencies);
    checked.wire.circle_tasks = limits.client.rows.min(w::CEILINGS.circle_tasks);
    checked.wire.waits_on_dependencies_pending = limits.client.rows.min(w::CEILINGS.waits_on_dependencies_pending);
    receiving_part0(&mut checked, limits);
    receiving_part1(&mut checked, limits);
    receiving_part2(&mut checked, limits);
    match w::check_limits(&checked.wire) {
        Ok(()) => Ok(checked),
        Err(problem) => Err(classify(problem)),
    }
}

fn receiving_part0(checked: &mut Limits, limits: &Limits) {
    checked.wire.attempt_failure_reason = limits.client.text;
    checked.wire.call_tool = limits.client.text;
    checked.wire.call_input = limits.client.text;
    checked.wire.call_outcome_some = limits.client.text;
    checked.wire.call_objects = limits.client.rows;
    checked.wire.call_parts = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.call_identity_id = limits.client.text;
    checked.wire.chip_rows = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.chip_phase_backing_off_fields_reason = limits.client.text;
    checked.wire.chip_phase_running_fields_tool_some = limits.client.text;
    checked.wire.chip_phase_waiting_fields_dependencies = limits.client.rows;
    checked.wire.closing_rows = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.closing_fields_connectors_some = limits.client.rows;
    checked.wire.current_call_tool = limits.client.text;
    checked.wire.current_call_input = limits.client.text;
    checked.wire.decision_left_held_reason = limits.client.text;
    checked.wire.decision_rejected_reason = limits.client.text;
    checked.wire.deployment_hosts = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.deployment_accounts = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.deployment_capacity = limits.client.rows;
    checked.wire.deployment_refusals = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.deployment_parts = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.document_specification_parameters = limits.client.rows;
    checked.wire.document_specification_inputs = limits.client.rows;
    checked.wire.document_text_bytes = limits.client.text;
    checked.wire.ending_rows = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.ending_cancelled_fields_reason_some = limits.client.text;
    checked.wire.ending_failed_fields_reason_some = limits.client.text;
    checked.wire.ending_withdrawn_fields_reason = limits.client.text;
    checked.wire.escalation_trail = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.escalation_acts = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.held_runtime_rows = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.hold_spend = limits.client.rows;
    checked.wire.hold_reason_rows = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.hold_reason_stalled_fields_waiting = limits.client.text;
    checked.wire.hold_reason_tries_fields_failures = limits.client.rows;
}
fn receiving_part1(checked: &mut Limits, limits: &Limits) {
    checked.wire.inbox_entries = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.inbox_replies_seen = limits.client.rows;
    checked.wire.owner_authority_grants =
        u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.owner_authority_note_resources =
        u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.owner_chip_details_fields_resources =
        u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.owner_chip_details_fields_closing_connectors_some = limits.client.rows;
    checked.wire.owner_chip_ending_rows =
        u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.owner_chip_phase_waiting_dependencies = limits.client.rows;
    checked.wire.owner_chip_work_tool_call = limits.client.text;
    checked.wire.owner_chip_work_tool_tool = limits.client.text;
    checked.wire.owner_delegation_kinds = limits.client.authority;
    checked.wire.owner_deployment_hosts =
        u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.owner_deployment_attempts =
        u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.owner_deployment_accounts =
        u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.owner_accounts_accounts =
        u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.owner_accounts_signing =
        u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.owner_account_line_sources =
        u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.owner_account_sources =
        u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.owner_account_line_uses_some = w::CEILINGS.owner_account_line_uses_some;
    checked.wire.owner_account_uses_some = w::CEILINGS.owner_account_uses_some;
    checked.wire.owner_deployment_projects = limits.client.rows;
    checked.wire.owner_deployment_hello_refusals = limits.client.rows;
    checked.wire.owner_deployment_person_room_refusals = limits.client.rows;
    checked.wire.owner_deployment_task_room_refusals = limits.client.rows;
    checked.wire.owner_deployment_connectors = limits.client.rows;
    checked.wire.owner_part_key_path_value = limits.client.segments;
    checked.wire.owner_part_key_path_value_item = limits.client.text;
    checked.wire.owner_project_acts = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.owner_project_resources =
        u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.owner_project_members = limits.client.rows;
    checked.wire.owner_project_pools = limits.client.rows;
    checked.wire.owner_project_configured_pools = limits.client.rows;
    checked.wire.owner_project_spend = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.owner_project_recurring =
        u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.owner_project_policy_projections =
        u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.owner_project_policy_roles =
        u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.owner_project_policy_requirements =
        u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.owner_project_policy_permissions_some = limits.client.rows;
    checked.wire.part_resource_path = limits.client.segments;
    checked.wire.part_resource_path_item = limits.client.text;
    checked.wire.party_name = limits.client.text;
    checked.wire.recipient_roles_roles = limits.client.rows;
    checked.wire.result_details_delegates =
        u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.result_details_followups_some =
        u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.result_document_acts = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.result_field_name = limits.client.text;
}
fn receiving_part2(checked: &mut Limits, limits: &Limits) {
    checked.wire.result_field_value = limits.client.text;
    checked.wire.result_value_rows = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.result_value_approved_fields_state = limits.client.text;
    checked.wire.result_value_owner_verdict_fields_fields = limits.client.rows;
    checked.wire.result_value_verdict_fields_fields = limits.client.rows;
    checked.wire.result_value_verdict_fields_tasks = limits.client.rows;
    checked.wire.run_text_text = limits.client.text;
    checked.wire.run_work_tool_tool = limits.client.text;
    checked.wire.runtime_backing_off_reason = limits.client.text;
    checked.wire.runtime_running_tool_some = limits.client.text;
    checked.wire.runtime_waiting_dependencies = limits.client.rows;
    checked.wire.steering_dependencies = limits.client.rows;
    checked.wire.subscriptions_entries = limits.client.rows;
    checked.wire.taken_by_name_some = limits.client.text;
    checked.wire.task_lineage_chips = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.task_inputs = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.task_lineage = limits.client.rows;
    checked.wire.task_dependencies = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.task_references = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.task_resources = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.task_proposals = limits.client.rows;
    checked.wire.task_acts = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.task_delegates = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.task_context_parts = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.task_context_spend = limits.client.rows;
    checked.wire.task_context_writer_slots =
        u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.task_contract_document_approval_state = limits.client.text;
    checked.wire.task_contract_document_verdict_choices =
        u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.tasks_rows = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.tree_nodes = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.wake_policy_topics = limits.client.rows;
    checked.wire.wake_policy_subscriptions =
        u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
}

#[path = "me.rs"]
pub(crate) mod me;
