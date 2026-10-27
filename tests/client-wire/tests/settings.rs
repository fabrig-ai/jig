//! Every new settings record and enum arm survives its committed bytes.
use jig_client_protocol_wire::v19 as w;
use skein_lib::{Reader, Writer};
#[expect(
    clippy::unwrap_used,
    clippy::too_many_lines,
    reason = "the record table checks every new wire type and enum arm"
)]
#[test]
fn settings_records_and_all_enum_arms_round_trip() {
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_setting_number_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::SettingNumber::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes = include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_setting_charter_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::Setting::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_setting_template_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::Setting::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes = include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_setting_policy_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::Setting::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes = include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_setting_rules_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::Setting::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes = include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_setting_model_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::SettingModel::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_setting_hosts_workers_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::SettingHosts::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_setting_hosts_engine_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::SettingHosts::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_setting_hosts_both_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::SettingHosts::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes = include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_charter_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::Charter::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_setting_policy_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::SettingPolicy::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes = include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_rules_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::Rules::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_setting_value_charter_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::SettingValue::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_setting_value_template_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::SettingValue::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_setting_value_policy_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::SettingValue::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_setting_value_rules_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::SettingValue::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes = include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_setting_set_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::SettingSet::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_setting_edit_set_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::SettingEdit::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_setting_edit_removed_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::SettingEdit::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_setting_edit_reset_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::SettingEdit::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes = include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_setting_item_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::SettingItem::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes = include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_running_stays_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::Running::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes = include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_running_moves_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::Running::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_change_settings_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::ChangeSettings::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes = include_bytes!(
            "../../../crates/jig-client-protocol-wire/golden/record_outcome_settings_changed_smallest.bin"
        );
        let mut input = Reader::new(bytes);
        let value = w::OutcomeSettingsChanged::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes = include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_refusal_stale_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::RefusalStale::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes = include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_refusal_full_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::RefusalFull::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_setting_fault_shape_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::SettingFault::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_setting_fault_count_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::SettingFault::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_setting_fault_reference_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::SettingFault::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_setting_fault_rules_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::SettingFault::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_setting_fault_budget_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::SettingFault::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_setting_fault_grace_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::SettingFault::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_setting_fault_owner_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::SettingFault::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_malformed_settings_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::MalformedSettings::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes = include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_read_setting_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::ReadSetting::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes = include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_read_revision_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::ReadRevision::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_read_revisions_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::ReadRevisions::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_setting_follows_default_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::SettingFollows::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_setting_follows_change_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::SettingFollows::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes = include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_setting_line_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::SettingLine::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_revision_party_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::RevisionParty::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes = include_bytes!(
            "../../../crates/jig-client-protocol-wire/golden/enum_revision_decider_first_start_smallest.bin"
        );
        let mut input = Reader::new(bytes);
        let value = w::RevisionDecider::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes = include_bytes!(
            "../../../crates/jig-client-protocol-wire/golden/enum_revision_decider_release_smallest.bin"
        );
        let mut input = Reader::new(bytes);
        let value = w::RevisionDecider::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_revision_decider_party_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::RevisionDecider::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes = include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_revision_head_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::RevisionHead::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes = include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_held_revision_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::HeldRevision::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_settings_proposal_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::SettingsProposal::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_settings_document_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::SettingsDocument::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_setting_document_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::SettingDocument::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes = include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_size_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::Size::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_revision_edit_set_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::RevisionEdit::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_revision_edit_too_large_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::RevisionEdit::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_revision_edit_removed_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::RevisionEdit::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_revision_edit_reset_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::RevisionEdit::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes = include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_revision_item_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::RevisionItem::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_revision_document_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::RevisionDocument::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_revision_forgotten_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::RevisionForgotten::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes = include_bytes!(
            "../../../crates/jig-client-protocol-wire/golden/enum_revision_answer_revision_smallest.bin"
        );
        let mut input = Reader::new(bytes);
        let value = w::RevisionAnswer::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes = include_bytes!(
            "../../../crates/jig-client-protocol-wire/golden/enum_revision_answer_forgotten_smallest.bin"
        );
        let mut input = Reader::new(bytes);
        let value = w::RevisionAnswer::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_revision_answer_unknown_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::RevisionAnswer::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_read_revision_answer_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::ReadRevisionAnswer::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_revision_item_edit_set_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::RevisionItemEdit::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes = include_bytes!(
            "../../../crates/jig-client-protocol-wire/golden/enum_revision_item_edit_removed_smallest.bin"
        );
        let mut input = Reader::new(bytes);
        let value = w::RevisionItemEdit::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes = include_bytes!(
            "../../../crates/jig-client-protocol-wire/golden/enum_revision_item_edit_reset_smallest.bin"
        );
        let mut input = Reader::new(bytes);
        let value = w::RevisionItemEdit::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_revision_item_head_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::RevisionItemHead::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes = include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_revision_row_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::RevisionRow::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_revisions_page_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::RevisionsPage::decode(&w::CEILINGS, &mut input).unwrap();
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut out).unwrap();
        assert_eq!(out.finish().as_ref(), bytes.as_slice());
    }
}

#[test]
fn move_to_newest_records_round_trip() {
    let ask =
        w::Ask::MoveToNewest(w::MoveToNewest::new(&w::CEILINGS, w::MoveToNewestParts { revision: 7 }).expect("move"));
    let mut out = Writer::new(usize::try_from(ask.measure()).expect("measure"));
    ask.encode(&mut out).expect("measured ask");
    let bytes = out.finish();
    let mut input = Reader::new(&bytes);
    assert_eq!(w::Ask::decode(&w::CEILINGS, &mut input), Ok(ask));
    assert!(input.is_empty());
    let outcome = w::Outcome::MovedToNewest(
        w::OutcomeMovedToNewest::new(&w::CEILINGS, w::OutcomeMovedToNewestParts { newest: 9, moved: 3, stayed: 1 })
            .expect("counts"),
    );
    let mut out = Writer::new(usize::try_from(outcome.measure()).expect("measure"));
    outcome.encode(&mut out).expect("measured outcome");
    let bytes = out.finish();
    let mut input = Reader::new(&bytes);
    assert_eq!(w::Outcome::decode(&w::CEILINGS, &mut input), Ok(outcome));
    assert!(input.is_empty());
}

#[test]
fn replaced_values_and_sizes_round_trip_their_wire_bytes() {
    for bytes in [
        include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_revision_replaced_value_full.bin")
            .as_slice(),
        include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_revision_replaced_too_large_full.bin")
            .as_slice(),
        include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_revision_replaced_default_smallest.bin")
            .as_slice(),
        include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_revision_replaced_removed_smallest.bin")
            .as_slice(),
        include_bytes!(
            "../../../crates/jig-client-protocol-wire/golden/enum_revision_replaced_not_offered_smallest.bin"
        )
        .as_slice(),
    ] {
        let mut input = Reader::new(bytes);
        let value = w::RevisionReplaced::decode(&w::CEILINGS, &mut input).expect("before-state decodes");
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).expect("measured bytes"));
        value.encode(&mut out).expect("measured before-state");
        assert_eq!(out.finish().as_ref(), bytes);
    }
}

#[test]
fn restore_records_round_trip_with_omitted_application_settings() {
    let limits = &w::CEILINGS;
    let ask = w::Ask::Restore(
        w::Restore::new(limits, w::RestoreParts { revision: 7, reason: b"earlier choices".as_slice().into() })
            .expect("restore"),
    );
    let mut out = Writer::new(usize::try_from(ask.measure()).expect("measure"));
    ask.encode(&mut out).expect("measured ask");
    assert_eq!(w::Ask::decode(limits, &mut Reader::new(&out.finish())), Ok(ask));
    let mut left_out = skein_lib::List::with_capacity(1);
    left_out
        .push(
            w::ApplicationSetting::new(limits, w::ApplicationSettingParts { kind: 8, scope: 9, number: 10 })
                .expect("identity"),
        )
        .expect("one omission");
    let outcome = w::Outcome::Restored(
        w::OutcomeRestored::new(limits, w::OutcomeRestoredParts { proposal: 11, to: 7, left_out }).expect("restored"),
    );
    let mut out = Writer::new(usize::try_from(outcome.measure()).expect("measure"));
    outcome.encode(&mut out).expect("measured outcome");
    assert_eq!(w::Outcome::decode(limits, &mut Reader::new(&out.finish())), Ok(outcome));
    let refusal = w::Refusal::Forgotten(
        w::RefusalForgotten::new(limits, w::RefusalForgottenParts { oldest: 3 }).expect("forgotten"),
    );
    let mut out = Writer::new(usize::try_from(refusal.measure()).expect("measure"));
    refusal.encode(&mut out).expect("measured refusal");
    assert_eq!(w::Refusal::decode(limits, &mut Reader::new(&out.finish())), Ok(refusal));
}
