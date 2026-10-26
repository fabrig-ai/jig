//! Settings heads share the party proposal lifecycle (tasks.md, section 9).
#![expect(clippy::disallowed_methods, reason = "step tests are ordinary Rust (programming-model.md, 10.2)")]
#![allow(
    clippy::wildcard_enum_match_arm,
    clippy::match_like_matches_macro,
    reason = "step tests inspect selected boundary records (programming-model.md, 10.2)"
)]
use super::revisions::{Harness, LIMITS, to};
use crate::*;
use skein_lib::Wall;
const PROPOSER: u64 = 9;
const OWNER: u64 = 10;
const PROPOSAL: u64 = 100;
fn proposal(items: u32) -> PersonProposal {
    PersonProposal {
        number: PROPOSAL,
        proposer: PROPOSER,
        project: 0,
        action: PersonAction::Settings {
            against: 1,
            items: (1..=items)
                .map(|number| SettingHead { setting: Setting::Template(number), edit: SettingEdit::Removed })
                .collect(),
        },
        reason: b"please review".as_slice().into(),
        state: PersonProposalState::Pending { since: Wall::EPOCH },
    }
}
fn pending() -> Harness {
    let mut h = Harness::new();
    h.send(Event::ProposePerson { reply_to: to(), proposal: proposal(1) });
    h
}
#[test]
fn settings_heads_restore_without_a_task_or_funding() {
    let mut h = pending();
    let row = h.stored[&Key::PersonProposal(PROPOSAL)].clone();
    h.domain = Domain::new(&LIMITS, 91, super::charters(), Box::new([0]));
    h.send(Event::Restore { record: row });
    h.send(Event::Check);
    h.send(Event::Restored);
    assert_eq!(h.domain.person_proposal(PROPOSER, PROPOSAL), Some(proposal(1)));
    assert_eq!(h.domain.person_proposal(PROPOSER, PROPOSAL).unwrap().first(), 0);
}
#[test]
fn settings_acceptance_retains_the_revision_it_made() {
    let mut h = pending();
    h.send(Event::DecidePersonProposal {
        reply_to: to(),
        proposer: PROPOSER,
        proposal: PROPOSAL,
        by: Party::Person(OWNER),
        message: None,
        decision: ProposalDecision::Accept,
        revision: Some(2),
    });
    let Stored::DecidedProposal(row) = &h.stored[&Key::DecidedProposal(PROPOSAL)] else { panic!("terminal proposal") };
    assert_eq!(row.action, proposal(1).action);
    assert_eq!(row.state, PersonProposalState::SettingsAccepted { by: Party::Person(OWNER), revision: 2 });
    assert!(h.domain.person_proposal(PROPOSER, PROPOSAL).is_none());
}
#[test]
fn settings_rejection_retains_heads_and_a_proposer_update() {
    let mut h = pending();
    h.send(Event::DecidePersonProposal {
        reply_to: to(),
        proposer: PROPOSER,
        proposal: PROPOSAL,
        by: Party::Person(OWNER),
        message: Some(101),
        decision: ProposalDecision::Reject { reason: b"later".as_slice().into() },
        revision: None,
    });
    let Stored::DecidedProposal(row) = &h.stored[&Key::DecidedProposal(PROPOSAL)] else { panic!("terminal proposal") };
    assert_eq!(row.action, proposal(1).action);
    assert!(match &row.state {
        PersonProposalState::Rejected { by: Party::Person(OWNER), reason, .. } if reason.as_ref() == b"later" => true,
        _ => false,
    });
    assert!(h.stored.values().any(|row| match row {
        Stored::Update(update)
            if update.party == PROPOSER
                && update.object == UpdateObject::Proposal { proposal: Newest::of(PROPOSAL) } =>
            true,
        _ => false,
    }));
}
#[test]
fn automatic_settings_withdrawal_notifies_without_a_reply_target() {
    let mut h = pending();
    let out = h.send(Event::WithdrawPersonProposal { reply_to: None, proposer: PROPOSER, proposal: PROPOSAL });
    assert!(out.iter().any(|row| match row {
        Request::PersonProposalDecided {
            reply_to: None,
            proposer: PROPOSER,
            number: PROPOSAL,
            outcome: ProposalOutcome::Withdrawn,
        } => true,
        _ => false,
    }));
    assert!(h.stored.values().any(|row| match row {
        Stored::Update(update) if update.party == PROPOSER => true,
        _ => false,
    }));
    assert!(!h.stored.contains_key(&Key::PersonProposal(PROPOSAL)));
}
#[test]
fn settings_heads_hold_at_128_and_refuse_one_more() {
    for count in [SETTINGS_ITEMS, SETTINGS_ITEMS + 1] {
        let mut h = Harness::new();
        let out = h.send(Event::ProposePerson { reply_to: to(), proposal: proposal(count) });
        assert_eq!(h.domain.person_proposal(PROPOSER, PROPOSAL).is_some(), count == SETTINGS_ITEMS, "{out:?}");
    }
}
#[test]
fn settings_proposals_keep_the_one_pending_proposal_per_party_rule() {
    let mut h = pending();
    let mut second = proposal(1);
    second.number += 1;
    let out = h.send(Event::ProposePerson { reply_to: to(), proposal: second });
    assert!(out.iter().any(|row| match row {
        Request::Refused { problem, .. } if problem.why == Refusal::Busy => true,
        _ => false,
    }));
    assert_eq!(h.domain.person_proposals().len(), 1);
}

#[test]
fn restore_refuses_two_pending_settings_proposals_from_one_party() {
    let mut h = Harness::new();
    h.domain = Domain::new(&LIMITS, 91, super::charters(), Box::new([0]));
    h.send(Event::Restore { record: Stored::PersonProposal(Box::new(proposal(1))) });
    let mut second = proposal(1);
    second.number += 1;
    let out = h.send(Event::Restore { record: Stored::PersonProposal(Box::new(second)) });
    assert!(out.iter().any(|row| match row {
        Request::RestoreRefused { .. } => true,
        _ => false,
    }));
    assert!(!h.domain.ready());
}

fn restored_pending() -> Harness {
    let mut h = Harness::new();
    h.domain = Domain::new(&LIMITS, 91, super::charters(), Box::new([0]));
    h.send(Event::Restore { record: Stored::PersonProposal(Box::new(proposal(1))) });
    h
}

#[test]
fn a_release_withdraws_restored_settings_before_task_adoption() {
    let mut h = restored_pending();
    let out = h.send(Event::WithdrawPersonProposal { reply_to: None, proposer: PROPOSER, proposal: PROPOSAL });
    assert!(h.domain.person_proposal(PROPOSER, PROPOSAL).is_none());
    assert!(out.iter().any(|row| match row {
        Request::PersonProposalDecided { reply_to: None, outcome: ProposalOutcome::Withdrawn, .. } => true,
        _ => false,
    }));
    assert!(match h.stored.get(&Key::DecidedProposal(PROPOSAL)) {
        Some(Stored::DecidedProposal(row))
            if match row.state {
                PersonProposalState::Withdrawn { .. } => true,
                _ => false,
            } =>
            true,
        _ => false,
    });
}

#[test]
fn explicit_withdrawal_waits_until_task_restoration_finishes() {
    let mut h = restored_pending();
    let out = h.send(Event::WithdrawPersonProposal { reply_to: Some(to()), proposer: PROPOSER, proposal: PROPOSAL });
    assert!(out.iter().any(|row| match row {
        Request::Refused { problem, .. } if problem.why == Refusal::NotReady => true,
        _ => false,
    }));
    assert!(h.domain.person_proposal(PROPOSER, PROPOSAL).is_some());
}
