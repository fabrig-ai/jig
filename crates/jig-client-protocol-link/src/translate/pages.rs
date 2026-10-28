//! Whole pages and single documents (protocol/client.md, 3 and 6).
use super::documents::{
    budget_fields, chip_fields, classify, cursor_fields, ending_fields, field_rule_fields, hold_fields,
    inbox_entry_piece, object_key_fields, offered_piece, owner_authority_document_fields, owner_authority_fields,
    owner_chip_ending_fields, owner_task_link_fields, part_piece, passed_piece, proposed_task_piece, recurring_piece,
    result_fields, specification_document_fields, task_contract_document_fields, task_fields,
};
use super::{Undecodable, piece_problem, too_large};
use crate::Limits;
use alloc::boxed::Box;
use jig_client_domain::contract as c;
use jig_client_protocol_wire::v19 as w;
use skein_lib::{List, Reader, Wall, bytes};
fn opaque(value: &[u8], limits: &Limits) -> Result<c::Bytes, w::Problem> {
    match c::Bytes::new(bytes::copy_of(value), limits.client.text) {
        Ok(value) => Ok(value),
        Err(_) => Err(too_large()),
    }
}
fn text(value: &[u8], limits: &Limits) -> Result<c::Text, w::Problem> {
    match c::Text::new(bytes::copy_of(value), limits.client.text) {
        Ok(value) => Ok(value),
        Err(_) => Err(too_large()),
    }
}

pub(crate) fn receiving(limits: &Limits) -> Result<Limits, Undecodable> {
    let mut checked = super::documents::receiving(limits)?;
    checked.wire.project_holds_rows = limits.client.rows;
    checked.wire.hold_row_exempt = match u32::try_from(limits.client.document_bytes / 8) {
        Ok(value) => value.min(w::CEILINGS.hold_row_exempt),
        Err(_) => return Err(Undecodable::TooLarge),
    };
    receiving_part0(&mut checked, limits);
    receiving_part1(&mut checked, limits);
    receiving_part2(&mut checked, limits);
    match w::check_limits(&checked.wire) {
        Ok(()) => Ok(checked),
        Err(problem) => Err(classify(problem)),
    }
}
fn receiving_part0(checked: &mut Limits, limits: &Limits) {
    checked.wire.task_page_live = u32::try_from(limits.client.document_bytes).expect("validated bytes");
    checked.wire.ended_rows_rows = u32::try_from(limits.client.document_bytes).expect("validated bytes");
    checked.wire.activity_page_rows = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.amendment_dependencies_some = limits.client.rows;
    checked.wire.amendment_instructions_some = limits.client.text;
    checked.wire.authority_grants = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.authority_task_kinds = limits.client.authority;
    checked.wire.authority_tools = limits.client.rows;
    checked.wire.authority_note_scopes =
        u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.call_identity_id = limits.client.text;
    checked.wire.chip_rows = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.closing_rows = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.document_specification_parameters = limits.client.rows;
    checked.wire.document_specification_inputs = limits.client.rows;
    checked.wire.document_text_bytes = limits.client.text;
    checked.wire.ending_rows = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.escalation_trail = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.escalation_acts = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.held_runtime_rows = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.history_page_rows = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.hold_spend = limits.client.rows;
    checked.wire.hold_reason_rows = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.inbox_page_rows = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.note_page_rows = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.object_page_rows = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.owner_activity_words_words = limits.client.text;
    checked.wire.owner_activity_kept_pending = limits.client.rows.min(w::CEILINGS.owner_activity_kept_pending);
    checked.wire.owner_amendment_state_dependencies = limits.client.rows;
    checked.wire.owner_authority_grants =
        u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.owner_authority_note_resources =
        u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.owner_chip_ending_rows =
        u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.owner_delegation_kinds = limits.client.authority;
    checked.wire.owner_ended_page_rows =
        u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.owner_escalation_acts =
        u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.owner_escalation_decision_reject_reason = limits.client.text;
    checked.wire.owner_inbox_page_selection_some =
        u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.owner_inbox_page_rows =
        u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.owner_part_key_path_value = limits.client.segments;
    checked.wire.owner_part_key_path_value_item = limits.client.text;
    checked.wire.owner_person_proposal_goal_dependencies = limits.client.rows;
    checked.wire.owner_person_proposal_goal_holdings =
        u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
}
fn receiving_part1(checked: &mut Limits, limits: &Limits) {
    checked.wire.owner_person_proposal_state_rejected_reason = limits.client.text;
    checked.wire.owner_person_task_results =
        u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.owner_person_task_acts =
        u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.owner_proposal_funding = limits.client.rows;
    checked.wire.owner_proposal_acts = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.owner_proposal_action_batch_value =
        u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.owner_retained_action_batch_value =
        u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.owner_retained_amendment_dependencies_some = limits.client.rows;
    checked.wire.owner_spend_page_rows = limits.client.rows;
    checked.wire.owner_word_words = limits.client.text;
    checked.wire.owner_word_result_approved_state = limits.client.text;
    checked.wire.owner_word_result_verdict_fields = limits.client.rows;
    checked.wire.parameter_bytes_value = limits.client.text;
    checked.wire.part_resource_path = limits.client.segments;
    checked.wire.part_resource_path_item = limits.client.text;
    checked.wire.party_name = limits.client.text;
    checked.wire.person_contract_approval_state = limits.client.text;
    checked.wire.person_contract_choice_options =
        u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.person_task_inputs = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.person_task_acts = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.policy_roles = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.policy_requirements = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.policy_deciders = limits.client.rows;
    checked.wire.project_spend_lines = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.project_name = limits.client.text;
    checked.wire.project_roles = limits.client.rows;
    checked.wire.project_pools = limits.client.rows;
    checked.wire.project_resources = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.project_services = limits.client.rows;
    checked.wire.project_recurring = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.project_parts = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.project_acts = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.proposal_why = limits.client.text;
    checked.wire.proposal_trail = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.proposal_acts = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.proposal_action_amend_before = limits.client.text;
    checked.wire.proposal_action_batch_tasks =
        u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.question_acts = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.recipient_roles_roles = limits.client.rows;
}
fn receiving_part2(checked: &mut Limits, limits: &Limits) {
    checked.wire.result_details_delegates =
        u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.result_details_followups_some =
        u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.result_document_acts = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.result_field_name = limits.client.text;
    checked.wire.result_field_value = limits.client.text;
    checked.wire.result_value_rows = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.run_work_tool_tool = limits.client.text;
    checked.wire.runtime_backing_off_reason = limits.client.text;
    checked.wire.runtime_running_tool_some = limits.client.text;
    checked.wire.runtime_waiting_dependencies = limits.client.rows;
    checked.wire.specification_words = limits.client.text;
    checked.wire.specification_parameters = limits.client.rows;
    checked.wire.specification_inputs = limits.client.rows;
    checked.wire.spend_page_rows = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
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
}
#[path = "pages_part0.rs"]
mod part0;
pub(crate) use part0::*;
#[path = "pages_part1.rs"]
mod part1;
pub(crate) use part1::*;
#[path = "pages_part2.rs"]
mod part2;
pub(crate) use part2::*;
#[path = "pages_part3.rs"]
mod part3;
pub(crate) use part3::*;
#[path = "pages_part4.rs"]
mod part4;
pub(crate) use part4::*;
#[path = "pages_part5.rs"]
mod part5;
pub(crate) use part5::*;
#[path = "pages_part6.rs"]
mod part6;
pub(crate) use part6::*;
#[path = "pages_part7.rs"]
mod part7;
pub(crate) use part7::*;
#[path = "pages_part8.rs"]
mod part8;
pub(crate) use part8::*;

pub fn history_page(record: w::HistoryPage, limits: &Limits) -> Result<c::HistoryPage, Undecodable> {
    if crate::limits::worst_case(limits).is_none() || u64::from(record.measure()) > limits.client.document_bytes {
        return Err(Undecodable::TooLarge);
    }
    let checked = receiving(limits)?;
    let limits = &checked;
    let record = match w::HistoryPage::new(&limits.wire, record.into_parts()) {
        Ok(value) => value,
        Err(problem) => return Err(classify(problem)),
    };
    match history_page_fields(&record, limits) {
        Ok(value) => Ok(value),
        Err(problem) => Err(classify(problem)),
    }
}

pub fn inbox_page(record: w::InboxPage, limits: &Limits) -> Result<c::InboxPage, Undecodable> {
    if crate::limits::worst_case(limits).is_none() || u64::from(record.measure()) > limits.client.document_bytes {
        return Err(Undecodable::TooLarge);
    }
    let checked = receiving(limits)?;
    let limits = &checked;
    let record = match w::InboxPage::new(&limits.wire, record.into_parts()) {
        Ok(value) => value,
        Err(problem) => return Err(classify(problem)),
    };
    match inbox_page_fields(&record, limits) {
        Ok(value) => Ok(value),
        Err(problem) => Err(classify(problem)),
    }
}

pub fn spend_page(record: w::SpendPage, limits: &Limits) -> Result<c::SpendPage, Undecodable> {
    if crate::limits::worst_case(limits).is_none() || u64::from(record.measure()) > limits.client.document_bytes {
        return Err(Undecodable::TooLarge);
    }
    let checked = receiving(limits)?;
    let limits = &checked;
    let record = match w::SpendPage::new(&limits.wire, record.into_parts()) {
        Ok(value) => value,
        Err(problem) => return Err(classify(problem)),
    };
    match spend_page_fields(&record, limits) {
        Ok(value) => Ok(value),
        Err(problem) => Err(classify(problem)),
    }
}

pub fn note_page(record: w::NotePage, limits: &Limits) -> Result<c::NotePage, Undecodable> {
    if crate::limits::worst_case(limits).is_none() || u64::from(record.measure()) > limits.client.document_bytes {
        return Err(Undecodable::TooLarge);
    }
    let checked = receiving(limits)?;
    let limits = &checked;
    let record = match w::NotePage::new(&limits.wire, record.into_parts()) {
        Ok(value) => value,
        Err(problem) => return Err(classify(problem)),
    };
    match note_page_fields(&record, limits) {
        Ok(value) => Ok(value),
        Err(problem) => Err(classify(problem)),
    }
}

pub fn object_page(record: w::ObjectPage, limits: &Limits) -> Result<c::ObjectPage, Undecodable> {
    if crate::limits::worst_case(limits).is_none() || u64::from(record.measure()) > limits.client.document_bytes {
        return Err(Undecodable::TooLarge);
    }
    let checked = receiving(limits)?;
    let limits = &checked;
    let record = match w::ObjectPage::new(&limits.wire, record.into_parts()) {
        Ok(value) => value,
        Err(problem) => return Err(classify(problem)),
    };
    match object_page_fields(&record, limits) {
        Ok(value) => Ok(value),
        Err(problem) => Err(classify(problem)),
    }
}

pub fn owner_ended_page(record: w::OwnerEndedPage, limits: &Limits) -> Result<c::OwnerEndedPage, Undecodable> {
    if crate::limits::worst_case(limits).is_none() || u64::from(record.measure()) > limits.client.document_bytes {
        return Err(Undecodable::TooLarge);
    }
    let checked = receiving(limits)?;
    let limits = &checked;
    let record = match w::OwnerEndedPage::new(&limits.wire, record.into_parts()) {
        Ok(value) => value,
        Err(problem) => return Err(classify(problem)),
    };
    match owner_ended_page_fields(&record, limits) {
        Ok(value) => Ok(value),
        Err(problem) => Err(classify(problem)),
    }
}

pub fn owner_inbox_page(record: w::OwnerInboxPage, limits: &Limits) -> Result<c::OwnerInboxPage, Undecodable> {
    if crate::limits::worst_case(limits).is_none() || u64::from(record.measure()) > limits.client.document_bytes {
        return Err(Undecodable::TooLarge);
    }
    let checked = receiving(limits)?;
    let limits = &checked;
    let record = match w::OwnerInboxPage::new(&limits.wire, record.into_parts()) {
        Ok(value) => value,
        Err(problem) => return Err(classify(problem)),
    };
    match owner_inbox_page_fields(&record, limits) {
        Ok(value) => Ok(value),
        Err(problem) => Err(classify(problem)),
    }
}

pub fn owner_spend_page(record: w::OwnerSpendPage, limits: &Limits) -> Result<c::OwnerSpendPage, Undecodable> {
    if crate::limits::worst_case(limits).is_none() || u64::from(record.measure()) > limits.client.document_bytes {
        return Err(Undecodable::TooLarge);
    }
    let checked = receiving(limits)?;
    let limits = &checked;
    let record = match w::OwnerSpendPage::new(&limits.wire, record.into_parts()) {
        Ok(value) => value,
        Err(problem) => return Err(classify(problem)),
    };
    match owner_spend_page_fields(&record, limits) {
        Ok(value) => Ok(value),
        Err(problem) => Err(classify(problem)),
    }
}

pub fn activity_page(record: w::ActivityPage, limits: &Limits) -> Result<c::ActivityPage, Undecodable> {
    if crate::limits::worst_case(limits).is_none() || u64::from(record.measure()) > limits.client.document_bytes {
        return Err(Undecodable::TooLarge);
    }
    let checked = receiving(limits)?;
    let limits = &checked;
    let record = match w::ActivityPage::new(&limits.wire, record.into_parts()) {
        Ok(value) => value,
        Err(problem) => return Err(classify(problem)),
    };
    match activity_page_fields(&record, limits) {
        Ok(value) => Ok(value),
        Err(problem) => Err(classify(problem)),
    }
}

pub(crate) fn turn_fields(value: &[u8], limits: &Limits) -> c::TurnRead {
    let bytes = u32::try_from(limits.client.document_bytes)
        .expect("validated document bytes")
        .min(limits.wire.activity_turn_turn);
    super::turn::decode(value, limits, bytes)
}

/// An archived turn its page carries, within the archive's bound (`turn::archive`).
pub(crate) fn carried_turn_fields(value: &[u8], limits: &Limits) -> c::TurnRead {
    let bytes = u32::try_from(limits.client.document_bytes)
        .expect("validated document bytes")
        .min(limits.wire.turn_carried_bytes);
    super::turn::decode(value, limits, bytes)
}

pub(crate) fn task_page(value: w::TaskPage, limits: &Limits) -> Result<c::TaskPage, Undecodable> {
    if u64::from(value.measure()) > limits.client.document_bytes {
        return Err(Undecodable::TooLarge);
    }
    let value = match w::TaskPage::new(&limits.wire, value.into_parts()) {
        Ok(value) => value,
        Err(problem) => return Err(classify(problem)),
    };
    match task_page_fields(&value, limits) {
        Ok(value) => Ok(value),
        Err(problem) => Err(classify(problem)),
    }
}
fn task_page_fields(value: &w::TaskPage, limits: &Limits) -> Result<c::TaskPage, w::Problem> {
    let live = super::documents::task_line_piece(value.live(), limits, limits.client.rows)?;
    let live_after = match value.live_after() {
        Some(v) => Some(cursor_fields(v, limits)?),
        None => None,
    };
    let ended = match value.ended() {
        w::EndedGroup::None => c::EndedGroup::None,
        w::EndedGroup::Rows(v) => c::EndedGroup::Rows {
            rows: super::documents::task_line_piece(v.rows(), limits, limits.client.rows)?,
            after: match v.after() {
                Some(v) => Some(cursor_fields(v, limits)?),
                None => None,
            },
            cut: v.cut(),
        },
    };
    Ok(c::TaskPage { live, live_after, ended })
}

pub fn holds(record: w::ProjectHolds, limits: &Limits) -> Result<c::ProjectHolds, Undecodable> {
    if crate::limits::worst_case(limits).is_none() || u64::from(record.measure()) > limits.client.document_bytes {
        return Err(Undecodable::TooLarge);
    }
    let limits = receiving(limits)?;
    let record = match w::ProjectHolds::new(&limits.wire, record.into_parts()) {
        Ok(value) => value,
        Err(problem) => return Err(classify(problem)),
    };
    let mut rows = List::with_capacity(record.rows().len());
    for row in record.rows() {
        let hold = match super::answers::placed(Some(row.hold()), &limits) {
            Ok(value) => value.expect("supplied hold"),
            Err(problem) => return Err(classify(problem)),
        };
        let mut exempt = List::with_capacity(row.exempt().len());
        for task in row.exempt() {
            exempt.push(*task).expect("same exemptions");
        }
        rows.push(c::HoldRow { hold, held: row.held_tasks(), exempt: exempt.into_boxed() }).expect("same rows");
    }
    Ok(c::ProjectHolds {
        project: record.project(),
        rows: rows.into_boxed(),
        after: match record.after() {
            Some(value) => match cursor_fields(value, &limits) {
                Ok(value) => Some(value),
                Err(problem) => return Err(classify(problem)),
            },
            None => None,
        },
    })
}
