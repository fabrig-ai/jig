use super::*;

#[test]
fn a_second_party_proposal_request_reads_the_original_committed_decision_after_restart() {
    let (configuration, limits) = fixture(816, false);
    let mut world = World::configured(816, false, configuration, limits);
    let proposed_goal = jig_core_world::fixtures::goal(
        &world,
        1,
        b"retain the goal decision".as_slice().into(),
        people::TaskExecutor::Agent { charter: 1 },
        jig_core_world::fixtures::Goal { budget: 600, ..Default::default() },
    );
    ask(&mut world, 80, jig_core_world::fixtures::propose_goal(proposed_goal));
    let Some(people::Reply::Outcome(people::Outcome::Proposed { proposal, to: 0 })) =
        world.people_answers.last().cloned()
    else {
        panic!("goal proposal: {:?}", world.people_answers)
    };
    let proposer = people::Proposer::Party(1);
    ask(
        &mut world,
        81,
        people::Ask::DecideProposal {
            project: 1,
            proposer,
            proposal,
            decision: people::ProposalDecision::Reject { reason: b"first choice".as_slice().into() },
            running: None,
        },
    );
    let key = root::Key::Core(core::Key::Core(core::CoreKey::ProposalDecision { proposer, proposal }));
    let original = *match world.store.rows.get(&key) {
        Some(root::Record::Core(core::Record::Core(core::CoreRecord::ProposalDecision(row)))) => row,
        other => panic!("durable decision: {other:?}"),
    };
    assert_eq!(original.choice, people::ProposalChoice::Rejected);
    assert!(original.at.as_nanos() > 0);
    let sign_in = world.sign_in.expect("original authenticated request");
    world.restart_with(fixture(816, false).0);
    ask_as(
        &mut world,
        sign_in,
        82,
        people::Ask::DecideProposal {
            project: 1,
            proposer,
            proposal,
            decision: people::ProposalDecision::Accept,
            running: None,
        },
    );
    assert_eq!(
        world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::ProposalDecided {
            proposer,
            proposal,
            by: original.by,
            at: original.at,
            choice: original.choice,
        }))
    );
    assert_eq!(
        world.store.rows.get(&key),
        Some(&root::Record::Core(core::Record::Core(core::CoreRecord::ProposalDecision(original))))
    );
    assert_eq!(world.store.rows.values().filter(|row| matches!(row,
        root::Record::Core(core::Record::People(people::Stored::Answer { outcome, .. }))
        if matches!(outcome, people::Outcome::ProposalDecided { proposer: maker, proposal: number, by, at, choice }
            if *maker == proposer && *number == proposal && *by == original.by && *at == original.at && *choice == original.choice)
    )).count(), 2);
    for (request, maker, number) in [(83, people::Proposer::Task(1), proposal), (84, proposer, proposal + 1)] {
        ask_as(
            &mut world,
            sign_in,
            request,
            people::Ask::DecideProposal {
                project: 1,
                proposer: maker,
                proposal: number,
                decision: people::ProposalDecision::Accept,
                running: None,
            },
        );
        assert_eq!(
            world.people_answers.last(),
            Some(&people::Reply::Outcome(people::Outcome::Refused(people::Refusal::Unknown)))
        );
    }
}
