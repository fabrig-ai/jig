//! Wire 19 settings proposals keep their committed records and tags.
use jig_client_protocol_wire::v19 as w;
use skein_lib::{Reader, Writer};
#[test]
#[expect(clippy::too_many_lines, reason = "one table checks each new record and arm against generated goldens")]
fn settings_proposal_records_and_arms_round_trip() {
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_propose_settings_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::ProposeSettings::decode(&w::CEILINGS, &mut input).expect("golden record");
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).expect("wire size"));
        value.encode(&mut out).expect("measured");
        assert_eq!(out.finish().as_ref(), bytes.as_slice(), "record_propose_settings_smallest");
    }
    {
        let bytes = include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_propose_settings_full.bin");
        let mut input = Reader::new(bytes);
        let value = w::ProposeSettings::decode(&w::CEILINGS, &mut input).expect("golden record");
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).expect("wire size"));
        value.encode(&mut out).expect("measured");
        assert_eq!(out.finish().as_ref(), bytes.as_slice(), "record_propose_settings_full");
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_origin_revision_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::OriginRevision::decode(&w::CEILINGS, &mut input).expect("golden record");
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).expect("wire size"));
        value.encode(&mut out).expect("measured");
        assert_eq!(out.finish().as_ref(), bytes.as_slice(), "record_origin_revision_smallest");
    }
    {
        let bytes = include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_origin_revision_full.bin");
        let mut input = Reader::new(bytes);
        let value = w::OriginRevision::decode(&w::CEILINGS, &mut input).expect("golden record");
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).expect("wire size"));
        value.encode(&mut out).expect("measured");
        assert_eq!(out.finish().as_ref(), bytes.as_slice(), "record_origin_revision_full");
    }
    {
        let bytes = include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_review_item_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::ReviewItem::decode(&w::CEILINGS, &mut input).expect("golden record");
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).expect("wire size"));
        value.encode(&mut out).expect("measured");
        assert_eq!(out.finish().as_ref(), bytes.as_slice(), "record_review_item_smallest");
    }
    {
        let bytes = include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_review_item_full.bin");
        let mut input = Reader::new(bytes);
        let value = w::ReviewItem::decode(&w::CEILINGS, &mut input).expect("golden record");
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).expect("wire size"));
        value.encode(&mut out).expect("measured");
        assert_eq!(out.finish().as_ref(), bytes.as_slice(), "record_review_item_full");
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_settings_review_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::SettingsReview::decode(&w::CEILINGS, &mut input).expect("golden record");
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).expect("wire size"));
        value.encode(&mut out).expect("measured");
        assert_eq!(out.finish().as_ref(), bytes.as_slice(), "record_settings_review_smallest");
    }
    {
        let bytes = include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_settings_review_full.bin");
        let mut input = Reader::new(bytes);
        let value = w::SettingsReview::decode(&w::CEILINGS, &mut input).expect("golden record");
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).expect("wire size"));
        value.encode(&mut out).expect("measured");
        assert_eq!(out.finish().as_ref(), bytes.as_slice(), "record_settings_review_full");
    }
    {
        let bytes = include_bytes!(
            "../../../crates/jig-client-protocol-wire/golden/record_owner_proposal_action_settings_smallest.bin"
        );
        let mut input = Reader::new(bytes);
        let value = w::OwnerProposalActionSettings::decode(&w::CEILINGS, &mut input).expect("golden record");
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).expect("wire size"));
        value.encode(&mut out).expect("measured");
        assert_eq!(out.finish().as_ref(), bytes.as_slice(), "record_owner_proposal_action_settings_smallest");
    }
    {
        let bytes = include_bytes!(
            "../../../crates/jig-client-protocol-wire/golden/record_owner_proposal_action_settings_full.bin"
        );
        let mut input = Reader::new(bytes);
        let value = w::OwnerProposalActionSettings::decode(&w::CEILINGS, &mut input).expect("golden record");
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).expect("wire size"));
        value.encode(&mut out).expect("measured");
        assert_eq!(out.finish().as_ref(), bytes.as_slice(), "record_owner_proposal_action_settings_full");
    }
    {
        let bytes = include_bytes!(
            "../../../crates/jig-client-protocol-wire/golden/record_owner_retained_action_settings_smallest.bin"
        );
        let mut input = Reader::new(bytes);
        let value = w::OwnerRetainedActionSettings::decode(&w::CEILINGS, &mut input).expect("golden record");
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).expect("wire size"));
        value.encode(&mut out).expect("measured");
        assert_eq!(out.finish().as_ref(), bytes.as_slice(), "record_owner_retained_action_settings_smallest");
    }
    {
        let bytes = include_bytes!(
            "../../../crates/jig-client-protocol-wire/golden/record_owner_retained_action_settings_full.bin"
        );
        let mut input = Reader::new(bytes);
        let value = w::OwnerRetainedActionSettings::decode(&w::CEILINGS, &mut input).expect("golden record");
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).expect("wire size"));
        value.encode(&mut out).expect("measured");
        assert_eq!(out.finish().as_ref(), bytes.as_slice(), "record_owner_retained_action_settings_full");
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_setting_origin_default_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::SettingOrigin::decode(&w::CEILINGS, &mut input).expect("golden record");
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).expect("wire size"));
        value.encode(&mut out).expect("measured");
        assert_eq!(out.finish().as_ref(), bytes.as_slice(), "enum_setting_origin_default_smallest");
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_setting_origin_default_full.bin");
        let mut input = Reader::new(bytes);
        let value = w::SettingOrigin::decode(&w::CEILINGS, &mut input).expect("golden record");
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).expect("wire size"));
        value.encode(&mut out).expect("measured");
        assert_eq!(out.finish().as_ref(), bytes.as_slice(), "enum_setting_origin_default_full");
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_setting_origin_change_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::SettingOrigin::decode(&w::CEILINGS, &mut input).expect("golden record");
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).expect("wire size"));
        value.encode(&mut out).expect("measured");
        assert_eq!(out.finish().as_ref(), bytes.as_slice(), "enum_setting_origin_change_smallest");
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_setting_origin_change_full.bin");
        let mut input = Reader::new(bytes);
        let value = w::SettingOrigin::decode(&w::CEILINGS, &mut input).expect("golden record");
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).expect("wire size"));
        value.encode(&mut out).expect("measured");
        assert_eq!(out.finish().as_ref(), bytes.as_slice(), "enum_setting_origin_change_full");
    }
    {
        let bytes = include_bytes!(
            "../../../crates/jig-client-protocol-wire/golden/enum_setting_origin_not_offered_smallest.bin"
        );
        let mut input = Reader::new(bytes);
        let value = w::SettingOrigin::decode(&w::CEILINGS, &mut input).expect("golden record");
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).expect("wire size"));
        value.encode(&mut out).expect("measured");
        assert_eq!(out.finish().as_ref(), bytes.as_slice(), "enum_setting_origin_not_offered_smallest");
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_setting_origin_not_offered_full.bin");
        let mut input = Reader::new(bytes);
        let value = w::SettingOrigin::decode(&w::CEILINGS, &mut input).expect("golden record");
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).expect("wire size"));
        value.encode(&mut out).expect("measured");
        assert_eq!(out.finish().as_ref(), bytes.as_slice(), "enum_setting_origin_not_offered_full");
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_review_value_value_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::ReviewValue::decode(&w::CEILINGS, &mut input).expect("golden record");
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).expect("wire size"));
        value.encode(&mut out).expect("measured");
        assert_eq!(out.finish().as_ref(), bytes.as_slice(), "enum_review_value_value_smallest");
    }
    {
        let bytes = include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_review_value_value_full.bin");
        let mut input = Reader::new(bytes);
        let value = w::ReviewValue::decode(&w::CEILINGS, &mut input).expect("golden record");
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).expect("wire size"));
        value.encode(&mut out).expect("measured");
        assert_eq!(out.finish().as_ref(), bytes.as_slice(), "enum_review_value_value_full");
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_review_value_too_large_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::ReviewValue::decode(&w::CEILINGS, &mut input).expect("golden record");
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).expect("wire size"));
        value.encode(&mut out).expect("measured");
        assert_eq!(out.finish().as_ref(), bytes.as_slice(), "enum_review_value_too_large_smallest");
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_review_value_too_large_full.bin");
        let mut input = Reader::new(bytes);
        let value = w::ReviewValue::decode(&w::CEILINGS, &mut input).expect("golden record");
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).expect("wire size"));
        value.encode(&mut out).expect("measured");
        assert_eq!(out.finish().as_ref(), bytes.as_slice(), "enum_review_value_too_large_full");
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_review_value_none_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::ReviewValue::decode(&w::CEILINGS, &mut input).expect("golden record");
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).expect("wire size"));
        value.encode(&mut out).expect("measured");
        assert_eq!(out.finish().as_ref(), bytes.as_slice(), "enum_review_value_none_smallest");
    }
    {
        let bytes = include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_review_value_none_full.bin");
        let mut input = Reader::new(bytes);
        let value = w::ReviewValue::decode(&w::CEILINGS, &mut input).expect("golden record");
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).expect("wire size"));
        value.encode(&mut out).expect("measured");
        assert_eq!(out.finish().as_ref(), bytes.as_slice(), "enum_review_value_none_full");
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_propose_action_settings_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::ProposeAction::decode(&w::CEILINGS, &mut input).expect("golden record");
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).expect("wire size"));
        value.encode(&mut out).expect("measured");
        assert_eq!(out.finish().as_ref(), bytes.as_slice(), "enum_propose_action_settings_smallest");
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_propose_action_settings_full.bin");
        let mut input = Reader::new(bytes);
        let value = w::ProposeAction::decode(&w::CEILINGS, &mut input).expect("golden record");
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).expect("wire size"));
        value.encode(&mut out).expect("measured");
        assert_eq!(out.finish().as_ref(), bytes.as_slice(), "enum_propose_action_settings_full");
    }
    {
        let bytes = include_bytes!(
            "../../../crates/jig-client-protocol-wire/golden/enum_owner_proposal_action_settings_smallest.bin"
        );
        let mut input = Reader::new(bytes);
        let value = w::OwnerProposalAction::decode(&w::CEILINGS, &mut input).expect("golden record");
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).expect("wire size"));
        value.encode(&mut out).expect("measured");
        assert_eq!(out.finish().as_ref(), bytes.as_slice(), "enum_owner_proposal_action_settings_smallest");
    }
    {
        let bytes = include_bytes!(
            "../../../crates/jig-client-protocol-wire/golden/enum_owner_proposal_action_settings_full.bin"
        );
        let mut input = Reader::new(bytes);
        let value = w::OwnerProposalAction::decode(&w::CEILINGS, &mut input).expect("golden record");
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).expect("wire size"));
        value.encode(&mut out).expect("measured");
        assert_eq!(out.finish().as_ref(), bytes.as_slice(), "enum_owner_proposal_action_settings_full");
    }
    {
        let bytes = include_bytes!(
            "../../../crates/jig-client-protocol-wire/golden/enum_owner_retained_action_settings_smallest.bin"
        );
        let mut input = Reader::new(bytes);
        let value = w::OwnerRetainedAction::decode(&w::CEILINGS, &mut input).expect("golden record");
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).expect("wire size"));
        value.encode(&mut out).expect("measured");
        assert_eq!(out.finish().as_ref(), bytes.as_slice(), "enum_owner_retained_action_settings_smallest");
    }
    {
        let bytes = include_bytes!(
            "../../../crates/jig-client-protocol-wire/golden/enum_owner_retained_action_settings_full.bin"
        );
        let mut input = Reader::new(bytes);
        let value = w::OwnerRetainedAction::decode(&w::CEILINGS, &mut input).expect("golden record");
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).expect("wire size"));
        value.encode(&mut out).expect("measured");
        assert_eq!(out.finish().as_ref(), bytes.as_slice(), "enum_owner_retained_action_settings_full");
    }
    {
        let bytes = include_bytes!(
            "../../../crates/jig-client-protocol-wire/golden/enum_owner_message_proposal_kind_settings_smallest.bin"
        );
        let mut input = Reader::new(bytes);
        let value = w::OwnerMessageProposalKind::decode(&w::CEILINGS, &mut input).expect("golden record");
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).expect("wire size"));
        value.encode(&mut out).expect("measured");
        assert_eq!(out.finish().as_ref(), bytes.as_slice(), "enum_owner_message_proposal_kind_settings_smallest");
    }
    {
        let bytes = include_bytes!(
            "../../../crates/jig-client-protocol-wire/golden/enum_owner_message_proposal_kind_settings_full.bin"
        );
        let mut input = Reader::new(bytes);
        let value = w::OwnerMessageProposalKind::decode(&w::CEILINGS, &mut input).expect("golden record");
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).expect("wire size"));
        value.encode(&mut out).expect("measured");
        assert_eq!(out.finish().as_ref(), bytes.as_slice(), "enum_owner_message_proposal_kind_settings_full");
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_malformed_running_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::Malformed::decode(&w::CEILINGS, &mut input).expect("golden record");
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).expect("wire size"));
        value.encode(&mut out).expect("measured");
        assert_eq!(out.finish().as_ref(), bytes.as_slice(), "enum_malformed_running_smallest");
    }
    {
        let bytes = include_bytes!("../../../crates/jig-client-protocol-wire/golden/enum_malformed_running_full.bin");
        let mut input = Reader::new(bytes);
        let value = w::Malformed::decode(&w::CEILINGS, &mut input).expect("golden record");
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).expect("wire size"));
        value.encode(&mut out).expect("measured");
        assert_eq!(out.finish().as_ref(), bytes.as_slice(), "enum_malformed_running_full");
    }
    {
        let bytes =
            include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_decide_proposal_smallest.bin");
        let mut input = Reader::new(bytes);
        let value = w::DecideProposal::decode(&w::CEILINGS, &mut input).expect("golden record");
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).expect("wire size"));
        value.encode(&mut out).expect("measured");
        assert_eq!(out.finish().as_ref(), bytes.as_slice(), "record_decide_proposal_smallest");
    }
    {
        let bytes = include_bytes!("../../../crates/jig-client-protocol-wire/golden/record_decide_proposal_full.bin");
        let mut input = Reader::new(bytes);
        let value = w::DecideProposal::decode(&w::CEILINGS, &mut input).expect("golden record");
        assert!(input.is_empty());
        let mut out = Writer::new(usize::try_from(value.measure()).expect("wire size"));
        value.encode(&mut out).expect("measured");
        assert_eq!(out.finish().as_ref(), bytes.as_slice(), "record_decide_proposal_full");
    }
}

#[test]
fn original_proposal_recipient_round_trips_and_old_short_bodies_are_rejected() {
    for to in [0, 7, 91, u32::MAX] {
        let value = w::OwnerOutcomeProposed::new(&w::CEILINGS, w::OwnerOutcomeProposedParts { proposal: 31, to })
            .expect("scalar role");
        assert_eq!(value.measure(), 12);
        let mut out = Writer::new(12);
        value.encode(&mut out).expect("exact room");
        let bytes = out.finish();
        let mut reader = Reader::new(&bytes);
        let read = w::OwnerOutcomeProposed::decode(&w::CEILINGS, &mut reader).expect("complete new record");
        assert_eq!((read.proposal(), read.to()), (31, to));
        assert!(reader.is_empty());
        for length in 8..12 {
            let mut reader = Reader::new(&bytes[..length]);
            let found = w::OwnerOutcomeProposed::decode(&w::CEILINGS, &mut reader).expect_err("old/truncated to");
            assert_eq!(found.path, w::Path::OwnerOutcomeProposedTo);
        }
        let mut extra = bytes.into_vec();
        extra.push(0);
        let mut reader = Reader::new(&extra);
        let found = w::OwnerOutcomeProposed::decode(&w::CEILINGS, &mut reader).expect_err("trailing byte");
        assert_eq!(found.path, w::Path::OwnerOutcomeProposedTail);
    }
}
