#![expect(clippy::disallowed_macros, reason = "unit tests read the committed settings golden fixtures")]
//! Settings translation is reversible for every record and populated whole values.
use super::*;
use crate::translate::settings as t;
use jig_client_test_records::settings as values;
use skein_lib::{List, Writer};
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one table checks every new record and enum arm against committed wire bytes"
)]
fn every_new_record_translates_in_both_directions() {
    let limits = Limits::new(jig_client_domain::Limits { rows: 8, ..fixtures::limits().client }, 4).unwrap();
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/record_setting_number_smallest.bin");
        let wire = w::SettingNumber::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_setting_number(&wire, &limits).unwrap();
        assert_eq!(t::wire_setting_number(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/enum_setting_charter_smallest.bin");
        let wire = w::Setting::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_setting(&wire, &limits).unwrap();
        assert_eq!(t::wire_setting(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/enum_setting_template_smallest.bin");
        let wire = w::Setting::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_setting(&wire, &limits).unwrap();
        assert_eq!(t::wire_setting(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/enum_setting_policy_smallest.bin");
        let wire = w::Setting::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_setting(&wire, &limits).unwrap();
        assert_eq!(t::wire_setting(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/enum_setting_rules_smallest.bin");
        let wire = w::Setting::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_setting(&wire, &limits).unwrap();
        assert_eq!(t::wire_setting(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/record_setting_model_smallest.bin");
        let wire = w::SettingModel::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_setting_model(&wire, &limits).unwrap();
        assert_eq!(t::wire_setting_model(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/enum_setting_hosts_workers_smallest.bin");
        let wire = w::SettingHosts::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_setting_hosts(&wire, &limits).unwrap();
        assert_eq!(t::wire_setting_hosts(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/enum_setting_hosts_engine_smallest.bin");
        let wire = w::SettingHosts::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_setting_hosts(&wire, &limits).unwrap();
        assert_eq!(t::wire_setting_hosts(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/enum_setting_hosts_both_smallest.bin");
        let wire = w::SettingHosts::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_setting_hosts(&wire, &limits).unwrap();
        assert_eq!(t::wire_setting_hosts(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/record_charter_smallest.bin");
        let wire = w::Charter::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_charter(&wire, &limits).unwrap();
        assert_eq!(t::wire_charter(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/record_setting_policy_smallest.bin");
        let wire = w::SettingPolicy::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_setting_policy(&wire, &limits).unwrap();
        assert_eq!(t::wire_setting_policy(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/record_rules_smallest.bin");
        let wire = w::Rules::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_rules(&wire, &limits).unwrap();
        assert_eq!(t::wire_rules(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/enum_setting_value_charter_smallest.bin");
        let wire = w::SettingValue::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_setting_value(&wire, &limits).unwrap();
        assert_eq!(t::wire_setting_value(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/enum_setting_value_template_smallest.bin");
        let wire = w::SettingValue::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_setting_value(&wire, &limits).unwrap();
        assert_eq!(t::wire_setting_value(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/enum_setting_value_policy_smallest.bin");
        let wire = w::SettingValue::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_setting_value(&wire, &limits).unwrap();
        assert_eq!(t::wire_setting_value(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/enum_setting_value_rules_smallest.bin");
        let wire = w::SettingValue::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_setting_value(&wire, &limits).unwrap();
        assert_eq!(t::wire_setting_value(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/record_setting_set_smallest.bin");
        let wire = w::SettingSet::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_setting_set(&wire, &limits).unwrap();
        assert_eq!(t::wire_setting_set(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/enum_setting_edit_set_smallest.bin");
        let wire = w::SettingEdit::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_setting_edit(&wire, &limits).unwrap();
        assert_eq!(t::wire_setting_edit(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/enum_setting_edit_removed_smallest.bin");
        let wire = w::SettingEdit::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_setting_edit(&wire, &limits).unwrap();
        assert_eq!(t::wire_setting_edit(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/enum_setting_edit_reset_smallest.bin");
        let wire = w::SettingEdit::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_setting_edit(&wire, &limits).unwrap();
        assert_eq!(t::wire_setting_edit(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/record_setting_item_smallest.bin");
        let wire = w::SettingItem::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_setting_item(&wire, &limits).unwrap();
        assert_eq!(t::wire_setting_item(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/enum_running_stays_smallest.bin");
        let wire = w::Running::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_running(&wire, &limits).unwrap();
        assert_eq!(t::wire_running(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/enum_running_moves_smallest.bin");
        let wire = w::Running::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_running(&wire, &limits).unwrap();
        assert_eq!(t::wire_running(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/record_change_settings_smallest.bin");
        let wire = w::ChangeSettings::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_change_settings(&wire, &limits).unwrap();
        assert_eq!(t::wire_change_settings(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes =
            include_bytes!("../../../jig-client-protocol-wire/golden/record_outcome_settings_changed_smallest.bin");
        let wire = w::OutcomeSettingsChanged::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_outcome_settings_changed(&wire, &limits).unwrap();
        assert_eq!(t::wire_outcome_settings_changed(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/record_refusal_stale_smallest.bin");
        let wire = w::RefusalStale::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_refusal_stale(&wire, &limits).unwrap();
        assert_eq!(t::wire_refusal_stale(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/record_refusal_full_smallest.bin");
        let wire = w::RefusalFull::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_refusal_full(&wire, &limits).unwrap();
        assert_eq!(t::wire_refusal_full(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/enum_setting_fault_shape_smallest.bin");
        let wire = w::SettingFault::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_setting_fault(&wire, &limits).unwrap();
        assert_eq!(t::wire_setting_fault(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/enum_setting_fault_count_smallest.bin");
        let wire = w::SettingFault::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_setting_fault(&wire, &limits).unwrap();
        assert_eq!(t::wire_setting_fault(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes =
            include_bytes!("../../../jig-client-protocol-wire/golden/enum_setting_fault_reference_smallest.bin");
        let wire = w::SettingFault::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_setting_fault(&wire, &limits).unwrap();
        assert_eq!(t::wire_setting_fault(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/enum_setting_fault_rules_smallest.bin");
        let wire = w::SettingFault::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_setting_fault(&wire, &limits).unwrap();
        assert_eq!(t::wire_setting_fault(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/enum_setting_fault_budget_smallest.bin");
        let wire = w::SettingFault::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_setting_fault(&wire, &limits).unwrap();
        assert_eq!(t::wire_setting_fault(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/enum_setting_fault_grace_smallest.bin");
        let wire = w::SettingFault::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_setting_fault(&wire, &limits).unwrap();
        assert_eq!(t::wire_setting_fault(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/enum_setting_fault_owner_smallest.bin");
        let wire = w::SettingFault::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_setting_fault(&wire, &limits).unwrap();
        assert_eq!(t::wire_setting_fault(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/record_malformed_settings_smallest.bin");
        let wire = w::MalformedSettings::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_malformed_settings(&wire, &limits).unwrap();
        assert_eq!(t::wire_malformed_settings(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/record_read_setting_smallest.bin");
        let wire = w::ReadSetting::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_read_setting(&wire, &limits).unwrap();
        assert_eq!(t::wire_read_setting(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/record_read_revision_smallest.bin");
        let wire = w::ReadRevision::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_read_revision(&wire, &limits).unwrap();
        assert_eq!(t::wire_read_revision(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/record_read_revisions_smallest.bin");
        let wire = w::ReadRevisions::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_read_revisions(&wire, &limits).unwrap();
        assert_eq!(t::wire_read_revisions(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes =
            include_bytes!("../../../jig-client-protocol-wire/golden/enum_setting_follows_default_smallest.bin");
        let wire = w::SettingFollows::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_setting_follows(&wire, &limits).unwrap();
        assert_eq!(t::wire_setting_follows(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/enum_setting_follows_change_smallest.bin");
        let wire = w::SettingFollows::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_setting_follows(&wire, &limits).unwrap();
        assert_eq!(t::wire_setting_follows(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/record_setting_line_smallest.bin");
        let wire = w::SettingLine::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_setting_line(&wire, &limits).unwrap();
        assert_eq!(t::wire_setting_line(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/record_revision_party_smallest.bin");
        let wire = w::RevisionParty::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_revision_party(&wire, &limits).unwrap();
        assert_eq!(t::wire_revision_party(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes =
            include_bytes!("../../../jig-client-protocol-wire/golden/enum_revision_decider_first_start_smallest.bin");
        let wire = w::RevisionDecider::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_revision_decider(&wire, &limits).unwrap();
        assert_eq!(t::wire_revision_decider(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes =
            include_bytes!("../../../jig-client-protocol-wire/golden/enum_revision_decider_release_smallest.bin");
        let wire = w::RevisionDecider::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_revision_decider(&wire, &limits).unwrap();
        assert_eq!(t::wire_revision_decider(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/enum_revision_decider_party_smallest.bin");
        let wire = w::RevisionDecider::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_revision_decider(&wire, &limits).unwrap();
        assert_eq!(t::wire_revision_decider(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/record_revision_head_smallest.bin");
        let wire = w::RevisionHead::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_revision_head(&wire, &limits).unwrap();
        assert_eq!(t::wire_revision_head(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/record_held_revision_smallest.bin");
        let wire = w::HeldRevision::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_held_revision(&wire, &limits).unwrap();
        assert_eq!(t::wire_held_revision(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/record_settings_proposal_smallest.bin");
        let wire = w::SettingsProposal::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_settings_proposal(&wire, &limits).unwrap();
        assert_eq!(t::wire_settings_proposal(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/record_settings_document_smallest.bin");
        let wire = w::SettingsDocument::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_settings_document(&wire, &limits).unwrap();
        assert_eq!(t::wire_settings_document(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/record_setting_document_smallest.bin");
        let wire = w::SettingDocument::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_setting_document(&wire, &limits).unwrap();
        assert_eq!(t::wire_setting_document(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/record_size_smallest.bin");
        let wire = w::Size::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_size(&wire, &limits).unwrap();
        assert_eq!(t::wire_size(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/enum_revision_edit_set_smallest.bin");
        let wire = w::RevisionEdit::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_revision_edit(&wire, &limits).unwrap();
        assert_eq!(t::wire_revision_edit(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes =
            include_bytes!("../../../jig-client-protocol-wire/golden/enum_revision_edit_too_large_smallest.bin");
        let wire = w::RevisionEdit::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_revision_edit(&wire, &limits).unwrap();
        assert_eq!(t::wire_revision_edit(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/enum_revision_edit_removed_smallest.bin");
        let wire = w::RevisionEdit::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_revision_edit(&wire, &limits).unwrap();
        assert_eq!(t::wire_revision_edit(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/enum_revision_edit_reset_smallest.bin");
        let wire = w::RevisionEdit::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_revision_edit(&wire, &limits).unwrap();
        assert_eq!(t::wire_revision_edit(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/record_revision_item_smallest.bin");
        let wire = w::RevisionItem::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_revision_item(&wire, &limits).unwrap();
        assert_eq!(t::wire_revision_item(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/record_revision_document_smallest.bin");
        let wire = w::RevisionDocument::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_revision_document(&wire, &limits).unwrap();
        assert_eq!(t::wire_revision_document(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/record_revision_forgotten_smallest.bin");
        let wire = w::RevisionForgotten::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_revision_forgotten(&wire, &limits).unwrap();
        assert_eq!(t::wire_revision_forgotten(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes =
            include_bytes!("../../../jig-client-protocol-wire/golden/enum_revision_answer_revision_smallest.bin");
        let wire = w::RevisionAnswer::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_revision_answer(&wire, &limits).unwrap();
        assert_eq!(t::wire_revision_answer(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes =
            include_bytes!("../../../jig-client-protocol-wire/golden/enum_revision_answer_forgotten_smallest.bin");
        let wire = w::RevisionAnswer::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_revision_answer(&wire, &limits).unwrap();
        assert_eq!(t::wire_revision_answer(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes =
            include_bytes!("../../../jig-client-protocol-wire/golden/enum_revision_answer_unknown_smallest.bin");
        let wire = w::RevisionAnswer::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_revision_answer(&wire, &limits).unwrap();
        assert_eq!(t::wire_revision_answer(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/record_read_revision_answer_smallest.bin");
        let wire = w::ReadRevisionAnswer::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_read_revision_answer(&wire, &limits).unwrap();
        assert_eq!(t::wire_read_revision_answer(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/enum_revision_item_edit_set_smallest.bin");
        let wire = w::RevisionItemEdit::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_revision_item_edit(&wire, &limits).unwrap();
        assert_eq!(t::wire_revision_item_edit(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes =
            include_bytes!("../../../jig-client-protocol-wire/golden/enum_revision_item_edit_removed_smallest.bin");
        let wire = w::RevisionItemEdit::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_revision_item_edit(&wire, &limits).unwrap();
        assert_eq!(t::wire_revision_item_edit(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes =
            include_bytes!("../../../jig-client-protocol-wire/golden/enum_revision_item_edit_reset_smallest.bin");
        let wire = w::RevisionItemEdit::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_revision_item_edit(&wire, &limits).unwrap();
        assert_eq!(t::wire_revision_item_edit(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/record_revision_item_head_smallest.bin");
        let wire = w::RevisionItemHead::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_revision_item_head(&wire, &limits).unwrap();
        assert_eq!(t::wire_revision_item_head(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/record_revision_row_smallest.bin");
        let wire = w::RevisionRow::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_revision_row(&wire, &limits).unwrap();
        assert_eq!(t::wire_revision_row(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/record_revisions_page_smallest.bin");
        let wire = w::RevisionsPage::decode(&limits.wire, &mut Reader::new(bytes)).unwrap();
        let client = t::restore_revisions_page(&wire, &limits).unwrap();
        assert_eq!(t::wire_revisions_page(&client, &limits.wire), Ok(wire));
    }
}

#[test]
fn whole_values_changes_and_history_keep_every_nonempty_field() {
    let limits = Limits::new(jig_client_domain::Limits { rows: 8, ..fixtures::limits().client }, 4).unwrap();
    for value in values::settings_values().into_boxed() {
        let wire = t::wire_setting_value(&value, &limits.wire).unwrap();
        assert_eq!(t::restore_setting_value(&wire, &limits), Ok(value));
    }
    let change = values::settings_change();
    let wire = t::wire_change_settings(&change, &limits.wire).unwrap();
    assert_eq!(t::restore_change_settings(&wire, &limits), Ok(change.clone()));
    let document = values::settings_document();
    let wire = t::wire_settings_document(&document, &limits.wire).unwrap();
    assert_eq!(t::restore_settings_document(&wire, &limits), Ok(document));
    let revision = values::settings_revision();
    let wire = t::wire_revision_document(&revision, &limits.wire).unwrap();
    assert_eq!(t::restore_revision_document(&wire, &limits), Ok(revision));
    let page = values::settings_page();
    let wire = t::wire_revisions_page(&page, &limits.wire).unwrap();
    assert_eq!(t::restore_revisions_page(&wire, &limits), Ok(page));
    for running in [c::Running::Moves, c::Running::Stays] {
        for edit in [c::SettingEdit::Removed, c::SettingEdit::Reset] {
            let mut change = change.clone();
            change.running = running.clone();
            change.items.get_mut(0).unwrap().edit = edit;
            let ask = c::Ask::ChangeSettings(change);
            let wire = translate::asks::ask(&ask, &limits.wire).unwrap();
            assert_eq!(translate::store::restore_ask(wire, &limits), Ok(ask));
        }
    }
}
#[test]
fn settings_change_survives_the_small_store_and_carried_transport() {
    let ask = c::Ask::ChangeSettings(values::settings_change());
    let saved = fixtures::saved_table(ask.clone());
    let mut h = fixtures::Harness::new(&fixtures::limits());
    h.start_store(fixtures::one_store_read(fixtures::store_read(saved.clone())));
    assert!(h.restored(&saved));
    for _ in 0..h.out.len() {
        h.out.pop();
    }
    h.link.step(
        Time::ZERO,
        Event::Send { attempt: fixtures::ATTEMPT, key: fixtures::KEY, ask: Ask::Jig(Box::new(ask)), value: None },
        &mut h.out,
    );
    assert_eq!(h.exchanges(), 1);
}
#[test]
fn malformed_pieces_counts_and_byte_bounds_are_refused() {
    let limits = Limits::new(jig_client_domain::Limits { rows: 8, ..fixtures::limits().client }, 4).unwrap();
    let mut change = values::settings_change();
    let mut wire = t::wire_change_settings(&change, &limits.wire).unwrap().into_parts();
    wire.items = Box::new([0, 0, 0, 1, 255]);
    let wire = w::ChangeSettings::new(&limits.wire, wire).unwrap();
    t::restore_change_settings(&wire, &limits).expect_err("invalid settings value is refused");
    let mut items = List::with_capacity(1025);
    for _ in 0_u32..1025 {
        items.push(c::SettingItem { setting: c::Setting::Rules, edit: c::SettingEdit::Reset }).unwrap();
    }
    change.items = items;
    t::wire_change_settings(&change, &limits.wire).expect_err("invalid settings value is refused");
    let mut tight = limits.wire;
    tight.change_settings_items = 1;
    t::wire_change_settings(&values::settings_change(), &tight).expect_err("invalid settings value is refused");
    let wire = translate::asks::ask(&c::Ask::ChangeSettings(values::settings_change()), &limits.wire).unwrap();
    let mut out = Writer::new(usize::try_from(wire.measure()).unwrap());
    wire.encode(&mut out).unwrap();
    let record = out.finish();
    let mut pieces = Writer::new(record.len().checked_add(4).unwrap());
    pieces.put(&u32::try_from(record.len()).unwrap().to_be_bytes()).unwrap();
    pieces.put(&record).unwrap();
    let compound =
        w::Compound::new(&limits.wire, w::CompoundParts { project: 1, operations: pieces.finish() }).unwrap();
    translate::store::restore_ask(w::Ask::Compound(compound), &limits).expect_err("invalid settings value is refused");
}

#[test]
fn settings_items_fit_at_the_count_and_byte_bounds() {
    const MAX_ITEMS: u32 = 1024; // The wire change/revision item count.
    const RESET_BYTES: u32 = 6; // Four-byte piece length, one setting tag, one edit tag.
    let limits = fixtures::limits();
    for (count, accepted) in [(MAX_ITEMS, true), (MAX_ITEMS.checked_add(1).unwrap(), false)] {
        let mut items = List::with_capacity(count);
        for _ in 0..count {
            items.push(c::SettingItem { setting: c::Setting::Rules, edit: c::SettingEdit::Reset }).unwrap();
        }
        let change = c::ChangeSettings { against: 1, items, running: c::Running::Stays };
        assert_eq!(t::wire_change_settings(&change, &limits.wire).is_ok(), accepted);
    }
    let mut items = List::with_capacity(1);
    items.push(c::SettingItem { setting: c::Setting::Rules, edit: c::SettingEdit::Reset }).unwrap();
    let change = c::ChangeSettings { against: 1, items, running: c::Running::Stays };
    for (bytes, accepted) in [(RESET_BYTES, true), (RESET_BYTES.checked_sub(1).unwrap(), false)] {
        let mut wire = limits.wire;
        wire.change_settings_items = bytes;
        assert_eq!(t::wire_change_settings(&change, &wire).is_ok(), accepted);
    }
    let mut malformed = t::wire_change_settings(&change, &limits.wire).unwrap().into_parts();
    // A valid reset row with an extra trailing byte inside its piece.
    malformed.items = Box::new([0, 0, 0, 3, 3, 2, 0]);
    let malformed = w::ChangeSettings::new(&limits.wire, malformed).unwrap();
    t::restore_change_settings(&malformed, &limits).expect_err("invalid settings value is refused");
}

// The standard inventory has 8 charters + 16 templates + 9 policies + the rules.
const STANDARD_SETTINGS: u32 = 34;
const MAX_SETTINGS: u32 = 256; // SettingsDocument.settings on client wire 19.

fn settings_inventory(count: u32) -> c::SettingsDocument {
    let mut settings = List::with_capacity(count);
    for number in 0..count {
        settings
            .push(c::SettingLine {
                setting: c::Setting::Charter(c::SettingNumber { number }),
                name: None,
                follows: c::SettingFollows::Default,
                last: 0,
            })
            .unwrap();
    }
    c::SettingsDocument { newest: 0, settings, held: List::with_capacity(0), pending: List::with_capacity(0) }
}

#[test]
fn settings_inventory_round_trips_beyond_a_page_and_at_its_ceiling() {
    let limits = fixtures::limits();
    for count in [STANDARD_SETTINGS, MAX_SETTINGS] {
        let document = settings_inventory(count);
        let wire = t::wire_settings_document(&document, &limits.wire).unwrap();
        let mut encoded = Writer::new(usize::try_from(wire.measure()).unwrap());
        wire.encode(&mut encoded).unwrap();
        let decoded = w::SettingsDocument::decode(&limits.wire, &mut Reader::new(&encoded.finish())).unwrap();
        assert_eq!(t::restore_settings_document(&decoded, &limits), Ok(document));
    }
}

#[test]
fn settings_inventory_refuses_one_line_past_the_wire_and_receiving_limits() {
    let limits = fixtures::limits();
    let problem = w::Problem { path: w::Path::SettingsDocumentSettings, reason: skein_codec::Reason::Bound };
    assert_eq!(t::wire_settings_document(&settings_inventory(MAX_SETTINGS + 1), &limits.wire), Err(problem));
    let mut narrow = limits.wire;
    narrow.settings_document_settings = STANDARD_SETTINGS - 1;
    let wire = t::wire_settings_document(&settings_inventory(STANDARD_SETTINGS), &limits.wire).unwrap();
    let mut encoded = Writer::new(usize::try_from(wire.measure()).unwrap());
    wire.encode(&mut encoded).unwrap();
    assert_eq!(w::SettingsDocument::decode(&narrow, &mut Reader::new(&encoded.finish())), Err(problem));
}

#[test]
fn move_to_newest_retains_its_revision_and_hears_both_counts() {
    let limits = Limits::new(fixtures::limits().client, 4).expect("limits");
    let ask = c::Ask::MoveToNewest(c::MoveToNewest { revision: 7 });
    let wire = translate::asks::ask(&ask, &limits.wire).expect("ask");
    assert_eq!(translate::store::restore_ask(wire, &limits), Ok(ask));
    let outcome = w::Outcome::MovedToNewest(
        w::OutcomeMovedToNewest::new(&limits.wire, w::OutcomeMovedToNewestParts { newest: 9, moved: 3, stayed: 1 })
            .expect("counts"),
    );
    let answer = w::Final::Done(w::Done::new(&limits.wire, w::DoneParts { outcome }).expect("done"));
    let mut bytes = Writer::new(usize::try_from(answer.measure()).expect("measure"));
    answer.encode(&mut bytes).expect("measured answer");
    let answer = w::KeyedAnswer::FinalAnswer(
        w::Owned::new(&limits.wire, w::OwnedParts { owner: w::Owner::Jig, version: 19, record: bytes.finish() })
            .expect("answer"),
    );
    assert_eq!(
        translate::answers::answer(answer, &limits),
        Ok(c::Answer::Done(c::Outcome::MovedToNewest(c::OutcomeMovedToNewest { newest: 9, moved: 3, stayed: 1 })))
    );
}

#[test]
fn restore_retains_the_reason_and_hears_proposal_revision_and_omissions() {
    let limits = Limits::new(fixtures::limits().client, 4).expect("limits");
    let ask = c::Ask::Restore(c::Restore {
        revision: 7,
        reason: c::Text::new(b"earlier choices".as_slice().into(), 64).expect("text"),
    });
    let wire = translate::asks::ask(&ask, &limits.wire).expect("ask");
    assert_eq!(translate::store::restore_ask(wire, &limits), Ok(ask));
    let mut left_out = List::with_capacity(1);
    left_out
        .push(
            w::ApplicationSetting::new(&limits.wire, w::ApplicationSettingParts { kind: 8, scope: 9, number: 10 })
                .expect("identity"),
        )
        .expect("one omission");
    let outcome = w::Outcome::Restored(
        w::OutcomeRestored::new(&limits.wire, w::OutcomeRestoredParts { proposal: 11, to: 7, left_out })
            .expect("restored"),
    );
    let answer = w::Final::Done(w::Done::new(&limits.wire, w::DoneParts { outcome }).expect("done"));
    let mut bytes = Writer::new(usize::try_from(answer.measure()).expect("measure"));
    answer.encode(&mut bytes).expect("measured answer");
    let answer = w::KeyedAnswer::FinalAnswer(
        w::Owned::new(&limits.wire, w::OwnedParts { owner: w::Owner::Jig, version: 19, record: bytes.finish() })
            .expect("answer"),
    );
    let mut left_out = List::with_capacity(1);
    left_out.push(c::ApplicationSetting { kind: 8, scope: 9, number: 10 }).expect("one omission");
    assert_eq!(
        translate::answers::answer(answer, &limits),
        Ok(c::Answer::Done(c::Outcome::Restored(c::OutcomeRestored { proposal: 11, to: 7, left_out })))
    );
    let refusal = w::Refusal::Forgotten(
        w::RefusalForgotten::new(&limits.wire, w::RefusalForgottenParts { oldest: 3 }).expect("forgotten"),
    );
    assert_eq!(
        translate::answers::refusal(&refusal, &limits),
        Ok(c::Refusal::Forgotten(c::RefusalForgotten { oldest: 3 }))
    );
}
