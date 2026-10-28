//! The current families change in place, with settings heads kept after decisions.
use super::{ask_round_trip, core_round_trip, fixtures, people_round_trip, saved_tasks};
use jig_domain_core as c;
use jig_domain_people as p;
use jig_domain_tasks as t;
use skein_lib::Wall;
fn settings_action(count: u32) -> t::PersonAction {
    let mut items = skein_lib::List::with_capacity(count);
    for number in 1..=count {
        items
            .push(t::SettingHead { setting: t::Setting::Template(number), edit: t::SettingEdit::Removed })
            .expect("bounded settings heads");
    }
    t::PersonAction::Settings { against: 1, items: items.into_boxed() }
}
#[test]
fn settings_asks_running_choices_and_refusals_round_trip_in_people_13() {
    let proposed = p::Ask::Propose {
        project: 0,
        action: p::Action::Settings { against: 3, digest: [9; 32] },
        reason: b"review".as_slice().into(),
    };
    ask_round_trip(proposed.clone());
    for running in [None, Some(p::Running::Stays), Some(p::Running::Moves)] {
        ask_round_trip(p::Ask::DecideProposal {
            project: 0,
            proposer: p::Proposer::Party(9),
            proposal: 17,
            decision: p::ProposalDecision::Accept,
            running,
        });
    }
    for why in [
        p::Refusal::Limit { which: p::LimitKind::SettingsProposals, bound: None },
        p::Refusal::NotWellFormed { at: 0, why: p::Malformed::Running },
    ] {
        people_round_trip(p::Stored::Answer {
            key: p::RequestKey { person: 9, key: [7; 16] },
            ask: Box::new(proposed.clone()),
            models: Box::new([]),
            outcome: p::Outcome::Refused(why),
            at: Wall::EPOCH,
        });
    }
}
#[test]
fn settings_heads_and_each_terminal_round_trip_in_tasks_9() {
    for state in [
        t::PersonProposalState::Pending { since: Wall::EPOCH },
        t::PersonProposalState::SettingsAccepted { by: t::Party::Person(10), revision: 2 },
        t::PersonProposalState::Rejected {
            by: t::Party::Person(10),
            reason: b"later".as_slice().into(),
            message: 18,
            at: Wall::EPOCH,
        },
        t::PersonProposalState::Withdrawn { at: Wall::EPOCH },
    ] {
        let row = t::PersonProposal {
            number: 17,
            proposer: 9,
            project: 0,
            action: settings_action(t::SETTINGS_ITEMS),
            reason: b"review".as_slice().into(),
            state: state.clone(),
        };
        let row = match state {
            t::PersonProposalState::Pending { .. } => t::Stored::PersonProposal(Box::new(row)),
            t::PersonProposalState::SettingsAccepted { .. }
            | t::PersonProposalState::Accepted { .. }
            | t::PersonProposalState::Rejected { .. }
            | t::PersonProposalState::Withdrawn { .. } => t::Stored::DecidedProposal(Box::new(row)),
        };
        assert_eq!(saved_tasks(&row), Ok(row));
    }
    let mut row = fixtures::task_person_proposal();
    row.action = settings_action(t::SETTINGS_ITEMS + 1);
    let limits = super::tasks_limits();
    crate::codec::tasks::proposals::person_proposal(&row, &limits, None)
        .expect_err("129 heads exceed the schema bound");
}
#[test]
fn settings_decision_kind_round_trips_in_core_6() {
    core_round_trip(c::CoreRecord::ProposalDecision(c::ProposalDecisionRecord {
        project: 0,
        proposer: p::Proposer::Party(9),
        proposal: 17,
        kind: t::ProposalKind::Settings,
        by: 10,
        at: Wall::EPOCH,
        choice: p::ProposalChoice::Accepted,
        created: None,
    }));
}

#[test]
fn hold_owner_call_parts_and_placed_hold_revisions_round_trip_in_core_6() {
    for owner in [p::HoldOwner::Party { person: 9 }, p::HoldOwner::Task { task: 7 }] {
        core_round_trip(c::CoreRecord::Call(c::CallRecord {
            key: c::CallKey { task: 3, attempt: 1, completion: 2, position: 0 },
            part: c::CallPart::HoldOwner { task: 7, hold: 31, revision: 2, owner },
            settled: None,
        }));
    }
    core_round_trip(c::CoreRecord::Call(c::CallRecord {
        key: c::CallKey { task: 3, attempt: 1, completion: 2, position: 0 },
        part: c::CallPart::Moved { task: 7, now: p::Moved::PlacedHold { revision: 2 } },
        settled: None,
    }));
}
