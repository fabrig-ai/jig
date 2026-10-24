//! Settings asks carry admission facts, never the held set (engine.md, 17.6).
#![allow(
    clippy::wildcard_enum_match_arm,
    clippy::match_like_matches_macro,
    reason = "step tests inspect selected boundary records (programming-model.md, 10.2)"
)]
use super::*;
const PARTY: u64 = 1;
const SIGN_IN: u64 = 10;
fn proposed() -> Ask {
    Ask::Propose {
        project: DEPLOYMENT,
        action: Action::Settings { against: 1, digest: [7; 32] },
        reason: b"because".as_slice().into(),
    }
}
#[test]
fn settings_proposals_admit_any_project_or_deployment_role() {
    for (project, role) in [(DEPLOYMENT, Role::Observer), (1, Role::Member), (1, Role::Owner)] {
        let mut test = Test::new(LIMITS);
        test.signin(PARTY, SIGN_IN, identity(0, 1));
        test.send(Event::Roles { project, holdings: Box::new([Holding { person: PARTY, role }]) });
        let out = test.request(SIGN_IN, 1, proposed());
        assert!(
            out.iter().any(|row| match row {
                Request::Route { person: PARTY, project: DEPLOYMENT, ask, .. } if **ask == proposed() => true,
                _ => false,
            }),
            "{out:?}"
        );
    }
}
#[test]
fn settings_proposals_refuse_parties_without_a_role() {
    let mut test = Test::new(LIMITS);
    test.signin(PARTY, SIGN_IN, identity(0, 1));
    assert_eq!(
        reply(&test.request(SIGN_IN, 1, proposed())),
        Reply::Outcome(Outcome::Refused(Refusal::Role { held: None }))
    );
}
#[test]
fn running_is_malformed_on_rejection_and_passing() {
    for decision in [ProposalDecision::Reject { reason: b"later".as_slice().into() }, ProposalDecision::Pass] {
        let mut test = Test::new(LIMITS);
        test.member();
        let ask = Ask::DecideProposal {
            project: DEPLOYMENT,
            proposer: Proposer::Party(PARTY),
            proposal: 7,
            decision,
            running: Some(Running::Moves),
        };
        assert_eq!(
            reply(&test.request(SIGN_IN, 1, ask)),
            Reply::Refused(Refusal::NotWellFormed { at: 0, why: Malformed::Running })
        );
    }
}
#[test]
fn accepting_carries_running_for_the_core_to_check_after_lookup() {
    for running in [None, Some(Running::Stays), Some(Running::Moves)] {
        let mut test = Test::new(LIMITS);
        test.member();
        let ask = Ask::DecideProposal {
            project: DEPLOYMENT,
            proposer: Proposer::Party(PARTY),
            proposal: 7,
            decision: ProposalDecision::Accept,
            running,
        };
        let out = test.request(SIGN_IN, 1, ask.clone());
        assert!(out.iter().any(|row| match row {
            Request::Route { ask: routed, .. } if **routed == ask => true,
            _ => false,
        }));
    }
}
