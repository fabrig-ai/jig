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
    h.domain = Domain::new(&LIMITS, 91, 1, super::charters(), Box::new([0]));
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
fn loading_keeps_two_committed_proposals_from_one_party() {
    let mut h = Harness::new();
    h.domain = Domain::new(&LIMITS, 91, 1, super::charters(), Box::new([0]));
    h.send(Event::Restore { record: Stored::PersonProposal(Box::new(proposal(1))) });
    let mut second = proposal(1);
    second.number += 1;
    let out = h.send(Event::Restore { record: Stored::PersonProposal(Box::new(second)) });
    assert!(out.is_empty());
    assert_eq!(h.domain.person_proposals().len(), 2);
    assert!(!h.domain.ready());
}

fn restored_pending() -> Harness {
    let mut h = Harness::new();
    h.domain = Domain::new(&LIMITS, 91, 1, super::charters(), Box::new([0]));
    h.send(Event::Restore { record: Stored::PersonProposal(Box::new(proposal(1))) });
    h
}

#[test]
fn a_release_withdraws_restored_settings_before_task_adoption() {
    let mut h = restored_pending();
    h.send(Event::Check);
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

#[test]
fn a_tasks_settings_proposal_waits_for_deployment_owners_and_its_outcome_wakes_it() {
    for outcome in
        [SettingsOutcome::Accepted { revision: 2 }, SettingsOutcome::Rejected, SettingsOutcome::Stale { newest: 2 }]
    {
        let mut h = Harness::new();
        h.make([super::revisions::task(1, &[])].into());
        let offered = Proposal {
            number: PROPOSAL,
            proposer: 1,
            project: 0,
            action: ProposalAction::Settings {
                against: 1,
                items: Box::new([SettingHead { setting: Setting::Charter(1), edit: SettingEdit::Set }]),
            },
            reason: b"please review".as_slice().into(),
            as_holder: false,
            state: ProposalState::Pending {
                holder: ProposalHolder::Policy { project: 0, kind: ProposalKind::Settings },
                since: Wall::EPOCH,
            },
        };
        let out = h.send(Event::Propose { reply_to: to(), proposal: offered.clone() });
        assert!(
            out.iter().any(|r| match r {
                Request::Done { .. } => true,
                _ => false,
            }),
            "{out:?}"
        );
        assert_eq!(h.domain.proposal(1, PROPOSAL), Some(offered.clone()));
        let refused = h.send(Event::Propose { reply_to: to(), proposal: Proposal { number: PROPOSAL + 1, ..offered } });
        assert!(refused.iter().any(|r| match r {
            Request::Refused { .. } => true,
            _ => false,
        }));
        domain::task_mut(&mut h.domain, 1).unwrap().record.phase = Phase::Active(Active::Idle);
        let reason: Box<[u8]> =
            if outcome == SettingsOutcome::Rejected { b"later".as_slice().into() } else { Box::new([]) };
        let by = if let SettingsOutcome::Stale { .. } = outcome {
            Party::Deployment { project: 0 }
        } else {
            Party::Person(OWNER)
        };
        let out = h.send(Event::DecideSettingsProposal {
            reply_to: None,
            decision: SettingsDecision {
                proposer: 1,
                proposal: PROPOSAL,
                message: PROPOSAL + 1,
                by,
                outcome,
                reason: reason.clone(),
            },
        });
        let row = h.domain.task(1).unwrap();
        assert!(row.proposal.is_none());
        assert_eq!(row.phase, Phase::Active(Active::Due));
        assert!(
            !out.iter().any(|r| match r {
                Request::Refused { .. } => true,
                _ => false,
            }),
            "{out:?}"
        );
        let word = row
            .inbox
            .iter()
            .find(|w| match w.kind {
                MessageKind::SettingsDecision { .. } => true,
                _ => false,
            })
            .unwrap();
        assert_eq!(word.kind, MessageKind::SettingsDecision { proposal: PROPOSAL, outcome });
        assert_eq!(word.words, reason);
        h.restart();
        assert!(
            h.domain
                .task(1)
                .unwrap()
                .inbox
                .iter()
                .any(|w| w.kind == MessageKind::SettingsDecision { proposal: PROPOSAL, outcome })
        );
    }
}
