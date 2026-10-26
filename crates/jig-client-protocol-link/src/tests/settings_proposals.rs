#![expect(clippy::disallowed_macros, reason = "unit tests read committed golden records")]
//! New settings records translate in both directions, including framed reviews.
use super::*;
use crate::translate::settings_proposals as t;
use jig_client_test_records::settings_proposals as values;
#[test]
fn every_settings_proposal_record_translates_both_ways() {
    let limits = Limits::new(jig_client_domain::Limits { rows: 8, ..fixtures::limits().client }, 4).expect("limits");
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/record_propose_settings_smallest.bin");
        let wire = w::ProposeSettings::decode(&limits.wire, &mut Reader::new(bytes)).expect("wire fixture");
        let client = t::restore_propose_settings(&wire, &limits).expect("client record");
        assert_eq!(t::wire_propose_settings(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/record_origin_revision_smallest.bin");
        let wire = w::OriginRevision::decode(&limits.wire, &mut Reader::new(bytes)).expect("wire fixture");
        let client = t::restore_origin_revision(&wire, &limits).expect("client record");
        assert_eq!(t::wire_origin_revision(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/record_review_item_smallest.bin");
        let wire = w::ReviewItem::decode(&limits.wire, &mut Reader::new(bytes)).expect("wire fixture");
        let client = t::restore_review_item(&wire, &limits).expect("client record");
        assert_eq!(t::wire_review_item(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/record_settings_review_smallest.bin");
        let wire = w::SettingsReview::decode(&limits.wire, &mut Reader::new(bytes)).expect("wire fixture");
        let client = t::restore_settings_review(&wire, &limits).expect("client record");
        assert_eq!(t::wire_settings_review(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!(
            "../../../jig-client-protocol-wire/golden/record_owner_proposal_action_settings_smallest.bin"
        );
        let wire = w::OwnerProposalActionSettings::decode(&limits.wire, &mut Reader::new(bytes)).expect("wire fixture");
        let client = t::restore_owner_proposal_action_settings(&wire, &limits).expect("client record");
        assert_eq!(t::wire_owner_proposal_action_settings(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!(
            "../../../jig-client-protocol-wire/golden/record_owner_retained_action_settings_smallest.bin"
        );
        let wire = w::OwnerRetainedActionSettings::decode(&limits.wire, &mut Reader::new(bytes)).expect("wire fixture");
        let client = t::restore_owner_retained_action_settings(&wire, &limits).expect("client record");
        assert_eq!(t::wire_owner_retained_action_settings(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/enum_setting_origin_default_smallest.bin");
        let wire = w::SettingOrigin::decode(&limits.wire, &mut Reader::new(bytes)).expect("wire fixture");
        let client = t::restore_setting_origin(&wire, &limits).expect("client record");
        assert_eq!(t::wire_setting_origin(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/enum_setting_origin_change_smallest.bin");
        let wire = w::SettingOrigin::decode(&limits.wire, &mut Reader::new(bytes)).expect("wire fixture");
        let client = t::restore_setting_origin(&wire, &limits).expect("client record");
        assert_eq!(t::wire_setting_origin(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes =
            include_bytes!("../../../jig-client-protocol-wire/golden/enum_setting_origin_not_offered_smallest.bin");
        let wire = w::SettingOrigin::decode(&limits.wire, &mut Reader::new(bytes)).expect("wire fixture");
        let client = t::restore_setting_origin(&wire, &limits).expect("client record");
        assert_eq!(t::wire_setting_origin(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/enum_review_value_value_smallest.bin");
        let wire = w::ReviewValue::decode(&limits.wire, &mut Reader::new(bytes)).expect("wire fixture");
        let client = t::restore_review_value(&wire, &limits).expect("client record");
        assert_eq!(t::wire_review_value(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/enum_review_value_too_large_smallest.bin");
        let wire = w::ReviewValue::decode(&limits.wire, &mut Reader::new(bytes)).expect("wire fixture");
        let client = t::restore_review_value(&wire, &limits).expect("client record");
        assert_eq!(t::wire_review_value(&client, &limits.wire), Ok(wire));
    }
    {
        let bytes = include_bytes!("../../../jig-client-protocol-wire/golden/enum_review_value_none_smallest.bin");
        let wire = w::ReviewValue::decode(&limits.wire, &mut Reader::new(bytes)).expect("wire fixture");
        let client = t::restore_review_value(&wire, &limits).expect("client record");
        assert_eq!(t::wire_review_value(&client, &limits.wire), Ok(wire));
    }
}
#[test]
fn populated_reviews_history_and_carried_proposals_keep_every_fact() {
    let limits = Limits::new(jig_client_domain::Limits { rows: 8, ..fixtures::limits().client }, 4).expect("limits");
    let proposal = values::proposal();
    let pending = values::pending();
    let mut accepted = pending.clone();
    accepted.review = None;
    accepted.made = Some(7);
    let retained = values::retained();
    assert_eq!(
        t::restore_propose_settings(&t::wire_propose_settings(&proposal, &limits.wire).expect("proposal"), &limits),
        Ok(proposal.clone())
    );
    assert_eq!(
        t::restore_owner_proposal_action_settings(
            &t::wire_owner_proposal_action_settings(&pending, &limits.wire).expect("pending"),
            &limits
        ),
        Ok(pending)
    );
    assert_eq!(
        t::restore_owner_retained_action_settings(
            &t::wire_owner_retained_action_settings(&retained, &limits.wire).expect("retained"),
            &limits
        ),
        Ok(retained)
    );
    assert_eq!(
        t::restore_owner_proposal_action_settings(
            &t::wire_owner_proposal_action_settings(&accepted, &limits.wire).expect("accepted action"),
            &limits
        ),
        Ok(accepted)
    );
    let ask = c::Ask::Propose {
        project: 0,
        action: c::ProposeAction::Settings(proposal),
        reason: c::Text::new(skein_lib::bytes::copy_of(b"review these settings"), limits.client.text).expect("reason"),
    };
    let wire = translate::asks::ask(&ask, &limits.wire).expect("ask");
    assert_eq!(translate::store::restore_ask(wire, &limits), Ok(ask));
    for running in [None, Some(c::Running::Stays), Some(c::Running::Moves)] {
        let ask = c::Ask::Decide {
            project: 0,
            on: c::DecideOn::Proposal {
                proposal: c::ProposalKey { proposer: c::Proposer::Party(3), number: 7 },
                decision: c::Decision::Accepted,
                running,
            },
        };
        let wire = translate::asks::ask(&ask, &limits.wire).expect("decision");
        assert_eq!(translate::store::restore_ask(wire, &limits), Ok(ask));
    }
}
// settings-v1 admits 128 proposal items; a reset of the rules is two tags plus a four-byte piece length.
const PROPOSAL_ITEMS: u32 = 128;
const RESET_PIECE: u32 = 6;
// Five unit tags and the four-byte piece length.
const REVIEW_PIECE: u32 = 9;
fn resets(count: u32) -> c::ProposeSettings {
    let mut items = skein_lib::List::with_capacity(count);
    for _ in 0..count {
        items.push(c::SettingItem { setting: c::Setting::Rules, edit: c::SettingEdit::Reset }).expect("item slots");
    }
    c::ProposeSettings { against: 6, items }
}
#[test]
fn proposed_items_fit_at_the_count_and_byte_ceilings() {
    let limits = fixtures::limits();
    let proposal = resets(PROPOSAL_ITEMS);
    let mut wire = limits.wire;
    wire.propose_settings_items = PROPOSAL_ITEMS * RESET_PIECE;
    let value = t::wire_propose_settings(&proposal, &wire).expect("at both ceilings");
    assert_eq!(t::restore_propose_settings(&value, &limits), Ok(proposal));
}
#[test]
fn proposed_items_refuse_one_past_either_ceiling() {
    let limits = fixtures::limits();
    t::wire_propose_settings(&resets(PROPOSAL_ITEMS + 1), &limits.wire).expect_err("one past the proposal item count");
    let mut wire = limits.wire;
    wire.propose_settings_items = PROPOSAL_ITEMS * RESET_PIECE - 1;
    t::wire_propose_settings(&resets(PROPOSAL_ITEMS), &wire).expect_err("one past the proposal byte bound");
    let mut writer = skein_lib::Writer::new(usize::try_from((PROPOSAL_ITEMS + 1) * RESET_PIECE).expect("piece bytes"));
    for _ in 0..=PROPOSAL_ITEMS {
        writer.put(&[0, 0, 0, 2, 3, 2]).expect("rules reset piece");
    }
    let value = w::ProposeSettings::new(&limits.wire, w::ProposeSettingsParts { against: 6, items: writer.finish() })
        .expect("outer record");
    t::restore_propose_settings(&value, &limits).expect_err("one past the decoded proposal item count");
}
#[test]
fn reviews_keep_their_byte_bound_and_refuse_malformed_pieces() {
    let mut limits = fixtures::limits();
    let mut items = skein_lib::List::with_capacity(1);
    items
        .push(c::ReviewItem {
            setting: c::Setting::Rules,
            edit: c::RevisionItemEdit::Reset,
            now: c::ReviewValue::None,
            origin: c::SettingOrigin::NotOffered,
            would_be: c::ReviewValue::None,
        })
        .expect("one item");
    let review = c::SettingsReview { newest: 6, items, cut: false };
    limits.wire.settings_review_items = REVIEW_PIECE;
    let wire = t::wire_settings_review(&review, &limits.wire).expect("exact review bound");
    assert_eq!(t::restore_settings_review(&wire, &limits), Ok(review.clone()));
    limits.wire.settings_review_items = REVIEW_PIECE - 1;
    t::wire_settings_review(&review, &limits.wire).expect_err("one past the review byte bound");
    t::restore_settings_review(&wire, &limits).expect_err("the receiving review byte bound is enforced");
    limits.wire.settings_review_items = w::CEILINGS.settings_review_items;
    for bytes in [b"\0".as_slice(), b"\0\0\0\x06\x03\x02\x02\x02\x02\0".as_slice()] {
        let wire = w::SettingsReview::new(
            &limits.wire,
            w::SettingsReviewParts { newest: 6, cut: false, items: skein_lib::bytes::copy_of(bytes) },
        )
        .expect("outer review");
        t::restore_settings_review(&wire, &limits).expect_err("malformed review framing is refused");
    }
}

#[test]
fn proposed_settings_and_running_choices_survive_the_small_store() {
    let proposal = c::ProposalKey { proposer: c::Proposer::Party(3), number: 7 };
    for ask in [
        c::Ask::Propose {
            project: 0,
            action: c::ProposeAction::Settings(values::proposal()),
            reason: c::Text::new(skein_lib::bytes::copy_of(b"review"), fixtures::limits().client.text).expect("reason"),
        },
        c::Ask::Decide {
            project: 0,
            on: c::DecideOn::Proposal { proposal, decision: c::Decision::Accepted, running: Some(c::Running::Moves) },
        },
    ] {
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
}

fn action_heads(count: u32) -> skein_lib::List<c::RevisionItemHead> {
    let mut items = skein_lib::List::with_capacity(count);
    for _ in 0..count {
        items
            .push(c::RevisionItemHead { setting: c::Setting::Rules, edit: c::RevisionItemEdit::Reset })
            .expect("head slots");
    }
    items
}
#[test]
fn settings_action_heads_fit_their_own_receiving_bounds() {
    let mut limits = fixtures::limits();
    let pending =
        c::OwnerProposalActionSettings { against: 6, items: action_heads(PROPOSAL_ITEMS), review: None, made: None };
    let retained = c::OwnerRetainedActionSettings { against: 6, items: action_heads(PROPOSAL_ITEMS), made: Some(7) };
    let pending_wire = t::wire_owner_proposal_action_settings(&pending, &limits.wire).expect("128 pending heads");
    let retained_wire = t::wire_owner_retained_action_settings(&retained, &limits.wire).expect("128 retained heads");
    assert_eq!(t::restore_owner_proposal_action_settings(&pending_wire, &limits), Ok(pending));
    assert_eq!(t::restore_owner_retained_action_settings(&retained_wire, &limits), Ok(retained.clone()));
    limits.wire.owner_proposal_action_settings_items = PROPOSAL_ITEMS - 1;
    t::restore_owner_proposal_action_settings(&pending_wire, &limits)
        .expect_err("the pending head receiving limit is enforced");
    assert_eq!(t::wire_owner_retained_action_settings(&retained, &limits.wire), Ok(retained_wire.clone()));
    limits.wire.owner_retained_action_settings_items = PROPOSAL_ITEMS - 1;
    t::restore_owner_retained_action_settings(&retained_wire, &limits)
        .expect_err("the retained head receiving limit is enforced");
}
#[test]
fn settings_actions_refuse_one_head_past_the_ceiling() {
    let limits = fixtures::limits();
    let pending = c::OwnerProposalActionSettings {
        against: 6,
        items: action_heads(PROPOSAL_ITEMS + 1),
        review: None,
        made: None,
    };
    let retained =
        c::OwnerRetainedActionSettings { against: 6, items: action_heads(PROPOSAL_ITEMS + 1), made: Some(7) };
    t::wire_owner_proposal_action_settings(&pending, &limits.wire).expect_err("one past the pending head bound");
    t::wire_owner_retained_action_settings(&retained, &limits.wire).expect_err("one past the retained head bound");
}

#[test]
fn a_settings_decision_keeps_its_three_outcomes_through_the_wire_and_link() {
    let limits = fixtures::limits();
    for (outcome, expected) in [
        (
            w::SettingsOutcome::Accepted(
                w::SettingsAccepted::new(&limits.wire, w::SettingsAcceptedParts { revision: 7 }).unwrap(),
            ),
            c::SettingsOutcome::Accepted { revision: 7 },
        ),
        (w::SettingsOutcome::Rejected, c::SettingsOutcome::Rejected),
        (
            w::SettingsOutcome::Stale(
                w::SettingsStale::new(&limits.wire, w::SettingsStaleParts { newest: 9 }).unwrap(),
            ),
            c::SettingsOutcome::Stale { newest: 9 },
        ),
    ] {
        let value = w::OwnerMessageKind::SettingsDecision(
            w::OwnerMessageKindSettingsDecision::new(
                &limits.wire,
                w::OwnerMessageKindSettingsDecisionParts { proposal: 41, outcome },
            )
            .unwrap(),
        );
        let mut writer = skein_lib::Writer::new(usize::try_from(value.measure()).expect("measured settings decision"));
        value.encode(&mut writer).unwrap();
        let value = w::OwnerMessageKind::decode(&limits.wire, &mut Reader::new(&writer.finish())).unwrap();
        assert_eq!(
            translate::pages::owner_message_kind_fields(&value, &limits).unwrap(),
            c::OwnerMessageKind::SettingsDecision { proposal: 41, outcome: expected }
        );
    }
}
