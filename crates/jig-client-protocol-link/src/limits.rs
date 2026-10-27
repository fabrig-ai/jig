//! Fixed attempt admission and receiving bounds (protocol/client.md, 13).
use crate::component::Attempt;
use jig_client_protocol_wire::v19 as w;
use skein_lib::{Duration, Map};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub client: jig_client_domain::Limits,
    pub exchanges: u32,
    pub wire: w::Limits,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unusable {
    Limits,
}
impl Limits {
    pub fn new(client: jig_client_domain::Limits, exchanges: u32) -> Result<Self, Unusable> {
        if client.owners.len() >= usize::try_from(w::CEILINGS.hello_served).expect("u32 fits") {
            return Err(Unusable::Limits);
        }
        for (i, owner) in client.owners.iter().enumerate() {
            for earlier in client.owners.get(..i).expect("declared prefix") {
                if earlier.family == owner.family {
                    return Err(Unusable::Limits);
                }
            }
        }
        if client.accounts.accounts == 0 || client.accounts.sources == 0 || client.accounts.sign_ins == 0 {
            return Err(Unusable::Limits);
        }
        let wire = wire(&client).ok_or(Unusable::Limits)?;
        let value = Self { client, exchanges, wire };
        worst_case(&value).ok_or(Unusable::Limits)?;
        Ok(value)
    }
}
#[expect(clippy::too_many_lines, reason = "the generated field bounds are assigned together at construction")]
fn wire(client: &jig_client_domain::Limits) -> Option<w::Limits> {
    let mut wire = w::CEILINGS;
    wire.owned_record = u32::try_from(client.document_bytes).ok()?;
    wire.document_text_bytes = client.text;
    wire.me_providers = wire.owned_record;
    wire.owner_accounts_readiness = wire.owned_record;
    wire.owner_deployment_providers = wire.owned_record;
    wire.owner_deployment_readiness = wire.owned_record;
    wire.provider_offer_programs = client.rows;
    wire.model_set_names_names_item = client.text.min(w::CEILINGS.model_set_names_names_item);
    wire.owner_chip_phase_waiting_dependencies = client.rows;
    wire.owner_chip_details_fields_dependencies = client.rows.min(w::CEILINGS.owner_chip_details_fields_dependencies);
    wire.task_dependency_facts = client.rows.min(w::CEILINGS.task_dependency_facts);
    wire.circle_tasks = client.rows.min(w::CEILINGS.circle_tasks);
    wire.refusal_circle_tasks = client.rows.min(w::CEILINGS.refusal_circle_tasks);
    wire.refusal_kept_pending = client.rows.min(w::CEILINGS.refusal_kept_pending);
    wire.owner_activity_kept_pending = client.rows.min(w::CEILINGS.owner_activity_kept_pending);
    wire.waits_on_dependencies_pending = client.rows.min(w::CEILINGS.waits_on_dependencies_pending);

    wire.call_identity_id = client.text;
    wire.run_work_tool_tool = client.text;
    wire.part_resource_path = client.segments;
    wire.part_resource_path_item = client.text;
    wire.wake_policy_topics = client.rows;
    wire.wake_policy_subscriptions = u32::try_from(client.document_bytes).ok()?;
    wire.recipient_roles_roles = client.rows;
    wire.taken_by_name_some = client.text;
    wire.party_name = client.text;
    wire.decision_rejected_reason = client.text;
    wire.restore_reason = client.text;
    wire.decision_left_held_reason = client.text;
    wire.attempt_failure_reason = client.text;
    wire.owner_part_key_path_value = client.segments;
    wire.owner_part_key_path_value_item = client.text;
    wire.result_field_name = client.text;
    wire.result_field_value = client.text;
    wire.chip_phase_waiting_fields_dependencies = client.rows;
    wire.chip_phase_running_fields_tool_some = client.text;
    wire.chip_phase_backing_off_fields_reason = client.text;
    wire.closing_fields_connectors_some = client.rows;
    wire.ending_failed_fields_reason_some = client.text;
    wire.ending_cancelled_fields_reason_some = client.text;
    wire.ending_withdrawn_fields_reason = client.text;
    wire.hold_reason_tries_fields_failures = client.rows;
    wire.hold_reason_stalled_fields_waiting = client.text;
    wire.owner_chip_details_fields_resources = u32::try_from(client.document_bytes).ok()?;
    wire.owner_chip_details_fields_closing_connectors_some = client.rows;
    wire.result_value_owner_verdict_fields_fields = client.rows;
    wire.result_value_verdict_fields_fields = client.rows;
    wire.result_value_verdict_fields_tasks = client.rows;
    wire.result_value_approved_fields_state = client.text;
    wire.chip_rows = u32::try_from(client.document_bytes).ok()?;
    wire.chip_phase_rows = u32::try_from(client.document_bytes).ok()?;
    wire.closing_rows = u32::try_from(client.document_bytes).ok()?;
    wire.ending_rows = u32::try_from(client.document_bytes).ok()?;
    wire.held_runtime_rows = u32::try_from(client.document_bytes).ok()?;
    wire.hold_reason_rows = u32::try_from(client.document_bytes).ok()?;
    wire.owner_chip_details_rows = u32::try_from(client.document_bytes).ok()?;
    wire.owner_chip_ending_rows = u32::try_from(client.document_bytes).ok()?;
    wire.result_value_rows = u32::try_from(client.document_bytes).ok()?;
    wire.person_answer_choice_reason = client.text;
    wire.person_answer_approved_words_some = client.text;
    wire.person_answer_words_value = client.text;
    wire.person_answer_cannot_reason = client.text;
    wire.parameter_bytes_value = client.text;
    wire.create_tasks = u32::try_from(client.document_bytes).ok()?.min(w::CEILINGS.create_tasks);
    wire.new_task_named = u32::try_from(client.document_bytes).ok()?.min(w::CEILINGS.new_task_named);
    wire.new_task_dependencies = client.rows.min(w::CEILINGS.new_task_dependencies);
    wire.contract_verdict_choices = client.rows.min(w::CEILINGS.contract_verdict_choices);
    wire.specification_words = client.text;
    wire.specification_parameters = client.rows;
    wire.specification_inputs = client.rows;
    wire.owner_pattern_end_exact_value = client.text;
    wire.owner_pattern_end_open_value = client.text;
    wire.owner_pattern_segments = client.segments;
    wire.owner_pattern_segments_item = client.text;
    wire.owner_delegation_kinds = client.authority;
    wire.owner_authority_grants = u32::try_from(client.document_bytes).ok()?;
    wire.owner_authority_note_resources = u32::try_from(client.document_bytes).ok()?;
    wire.amendment_dependencies_some = client.rows;
    wire.amendment_instructions_some = client.text;
    wire.policy_edit_requirements_requirements = u32::try_from(client.document_bytes).ok()?;
    wire.policy_edit_projections_grants = u32::try_from(client.document_bytes).ok()?;
    wire.policy_edit_permissions_mappings = client.rows;
    wire.policy_edit_templates_overrides = client.rows.min(w::CEILINGS.policy_edit_templates_overrides);
    wire.message_words = client.text;
    wire.answer_question_words = client.text;
    wire.amend_reason = client.text;
    wire.cancel_reason = client.text;
    wire.move_reason = client.text;
    wire.edit_note_description = client.text;
    wire.edit_note_body = client.text;
    wire.edit_note_references = client.rows;
    wire.set_roles_holdings = client.rows;
    wire.make_service_name = client.text;
    wire.made_tasks_tasks = client.rows;
    wire.read_notes_description_some = client.text;
    wire.owner_spend_page_rows = client.rows;
    wire.owner_word_result_approved_state = client.text;
    wire.owner_word_result_verdict_fields = client.rows;
    wire.owner_word_words = client.text;
    wire.owner_activity_words_words = client.text;
    wire.run_ending_failed_reason = client.text;
    wire.field_rule_name = client.text;
    wire.verdict_choice_fields = client.rows;
    wire.task_contract_document_approval_state = client.text;
    wire.task_contract_document_verdict_choices = u32::try_from(client.document_bytes).ok()?;
    wire.result_details_delegates = u32::try_from(client.document_bytes).ok()?;
    wire.result_details_followups_some = u32::try_from(client.document_bytes).ok()?;
    wire.bounds_create_charters = client.rows;
    wire.bounds_create_procedures = client.rows;
    wire.bounds_amend_grants = u32::try_from(client.document_bytes).ok()?;
    wire.bounds_amend_implications = client.authority;
    wire.result_document_acts = u32::try_from(client.document_bytes).ok()?;
    wire.activity_words_words = client.text;
    wire.activity_run_began_why = client.text;
    wire.activity_news_summary = client.text;
    wire.activity_page_rows = u32::try_from(client.document_bytes).ok()?;
    wire.hold_spend = client.rows;
    wire.history_change_amended_before = client.text;
    wire.history_change_amended_after = client.text;
    wire.history_change_cancelled_reason = client.text;
    wire.history_change_moved_reason = client.text;
    wire.document_specification_parameters = client.rows;
    wire.document_specification_inputs = client.rows;
    wire.proposed_task_dependencies = client.rows;
    wire.proposed_task_resources = u32::try_from(client.document_bytes).ok()?;
    wire.owner_retained_amendment_dependencies_some = client.rows;
    wire.owner_retained_action_batch_value = u32::try_from(client.document_bytes).ok()?;
    wire.owner_amendment_state_dependencies = client.rows;
    wire.history_page_rows = u32::try_from(client.document_bytes).ok()?;
    wire.runtime_waiting_dependencies = client.rows;
    wire.runtime_running_tool_some = client.text;
    wire.runtime_backing_off_reason = client.text;
    wire.inbox_entry_lineage = u32::try_from(client.document_bytes).ok()?;
    wire.inbox_page_rows = u32::try_from(client.document_bytes).ok()?;
    wire.spend_period_pools = client.rows;
    wire.spend_period_tasks = u32::try_from(client.document_bytes).ok()?;
    wire.spend_page_rows = u32::try_from(client.document_bytes).ok()?;
    wire.note_tasks = client.rows;
    wire.note_acts = u32::try_from(client.document_bytes).ok()?;
    wire.note_page_rows = u32::try_from(client.document_bytes).ok()?;
    wire.object_page_rows = u32::try_from(client.document_bytes).ok()?;
    wire.owner_ended_page_rows = u32::try_from(client.document_bytes).ok()?;
    wire.owner_inbox_selection_goals = client.rows;
    wire.owner_inbox_page_selection_some = u32::try_from(client.document_bytes).ok()?;
    wire.owner_inbox_page_rows = u32::try_from(client.document_bytes).ok()?;
    wire.subject_chips_tasks = client.rows;
    wire.propose_settings_items = u32::try_from(client.ask_bytes).ok()?.min(w::CEILINGS.propose_settings_items);
    wire.settings_review_items = u32::try_from(client.document_bytes).ok()?.min(w::CEILINGS.settings_review_items);
    wire.owner_proposal_action_settings_items = w::CEILINGS.owner_proposal_action_settings_items;
    wire.owner_retained_action_settings_items = w::CEILINGS.owner_retained_action_settings_items;
    // Whole settings share the existing receiving text, rows and message envelopes.
    wire.charter_instructions = u32::try_from(client.document_bytes).ok()?.min(w::CEILINGS.charter_instructions);
    wire.change_settings_items = u32::try_from(client.ask_bytes).ok()?.min(w::CEILINGS.change_settings_items);
    wire.revision_document_items = u32::try_from(client.document_bytes).ok()?.min(w::CEILINGS.revision_document_items);
    wire.setting_model_name = client.text.min(w::CEILINGS.setting_model_name);
    wire.setting_model_program_some = client.text.min(w::CEILINGS.setting_model_program_some);
    wire.setting_model_effort_some = client.text.min(w::CEILINGS.setting_model_effort_some);
    wire.setting_model_efforts_item = client.text.min(w::CEILINGS.setting_model_efforts_item);
    wire.charter_alternatives = client.rows.min(w::CEILINGS.charter_alternatives);
    wire.setting_model_efforts = client.rows.min(w::CEILINGS.setting_model_efforts);
    wire.setting_policy_projections = client.rows.min(w::CEILINGS.setting_policy_projections);
    wire.setting_policy_roles = client.rows.min(w::CEILINGS.setting_policy_roles);
    wire.setting_policy_requirements = client.rows.min(w::CEILINGS.setting_policy_requirements);
    wire.setting_policy_permissions = client.rows.min(w::CEILINGS.setting_policy_permissions);
    wire.setting_policy_templates = client.rows.min(w::CEILINGS.setting_policy_templates);
    wire.rules_implies = client.rows.min(w::CEILINGS.rules_implies);
    wire.rules_requirements = client.rows.min(w::CEILINGS.rules_requirements);
    // Settings are a whole inventory, independent of a page's row limit.
    wire.settings_document_settings =
        u32::try_from(client.document_bytes).ok()?.min(w::CEILINGS.settings_document_settings);
    wire.settings_document_held = client.rows.min(w::CEILINGS.settings_document_held);
    wire.settings_document_pending = client.rows.min(w::CEILINGS.settings_document_pending);
    wire.revision_row_items = client.rows.min(w::CEILINGS.revision_row_items);
    wire.revisions_page_rows = client.rows.min(w::CEILINGS.revisions_page_rows);
    wire.refusal_full_tasks = client.rows.min(w::CEILINGS.refusal_full_tasks);
    w::check_limits(&wire).ok()?;
    Some(wire)
}
#[must_use]
pub fn worst_case(limits: &Limits) -> Option<u64> {
    if limits.exchanges == 0 {
        return None;
    }
    if wire(&limits.client)? != limits.wire {
        return None;
    }
    max_out(limits)?;
    if limits.client.watches == 0 || limits.client.heartbeat == Duration::ZERO {
        return None;
    }
    let map = Map::<skein_lib::Token, Attempt>::worst_case(limits.exchanges)?;
    map.checked_add(u64::from(limits.exchanges).checked_mul(40)?)?
        .checked_add(skein_lib::List::<skein_lib::Token>::worst_case(limits.exchanges)?)?
        .checked_add(crate::watches::worst_case(limits)?)?
        .checked_add(skein_lib::List::<skein_lib::Token>::worst_case(limits.client.watches)?)?
        .checked_add(crate::store::worst_case(limits)?)?
        // Authorization owns its encoded query and decoded redirect only for this step.
        .checked_add(w::Authorize::worst_case_bytes(&limits.wire)?.checked_mul(2)?)?
        .checked_add(w::AuthorizeAnswer::worst_case_bytes(&limits.wire)?.checked_mul(2)?)?
        .checked_add(40)
}

#[must_use]
pub fn max_out(limits: &Limits) -> Option<u32> {
    let per_event = limits.client.watches.checked_mul(3)?.checked_add(limits.exchanges)?.checked_add(20)?;
    per_event
        .checked_add(limits.exchanges.checked_mul(2)?)?
        .checked_add(limits.client.requests.requests.checked_mul(2)?)
}
