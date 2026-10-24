use super::*;

#[test]
fn a_second_authenticated_decider_reads_the_first_task_proposal_choice_and_clock() {
    let mut world = scripted(
        817,
        vec![
            Box::new([
                Script::Call(Call::ProposeGoal {
                    words: b"decide only once".as_slice().into(),
                    budget: 150,
                    priority: 7,
                }),
                Script::Park,
            ]),
            Box::new([Script::Finish { report: b"one goal".as_slice().into() }]),
        ],
        false,
    );
    acts(
        &mut world,
        2,
        Box::new([Act::Request { key: [81; 16], ask: Ask::Chat { words: b"propose once".as_slice().into() } }]),
    );
    acts(&mut world, 1, Box::new([Act::Decide { accept: true }]));
    let original = world
        .store
        .rows
        .values()
        .find_map(|row| match row {
            root::Record::Core(core::Record::Core(core::CoreRecord::ProposalDecision(row))) => Some(*row),
            root::Record::Core(_) | root::Record::Connector { .. } => None,
        })
        .expect("actual durable task proposal decision");
    assert!(matches!(original.proposer, people::Proposer::Task(_)));
    assert_eq!(original.choice, people::ProposalChoice::Accepted);
    let peers = world.peers.as_ref().expect("actual parties");
    let owner = peers.parties[0].person.expect("owner");
    let maintainer = peers.parties[1].person.expect("maintainer");
    let owner_sign_in = peers.parties[0].sign_in.expect("owner sign-in");
    assert_ne!(owner, maintainer);
    assert_eq!(original.by, maintainer);
    let assignments: Vec<_> = peers.workers[0].seen.iter().map(|row| (row.task, row.attempt)).collect();
    let mut tasks: Vec<_> = assignments.iter().map(|row| row.0).collect();
    tasks.sort_unstable();
    tasks.dedup();
    assert_eq!(tasks.len(), 2, "only the proposing chat and its accepted goal");
    world.send(root::Event::Core(core::Event::People(people::Event::Ask {
        models: Box::new([]),
        reply_to: skein_lib::ReplyTo::new(skein_lib::Token::new(81_001)),
        sign_in: owner_sign_in,
        key: [82; 16],
        ask: people::Ask::DecideProposal {
            project: original.project,
            proposer: original.proposer,
            proposal: original.proposal,
            decision: people::ProposalDecision::Reject { reason: b"second choice".as_slice().into() },
            running: None,
        },
    })));
    assert_eq!(
        world
            .store
            .rows
            .values()
            .filter(|row| matches!(row,
                root::Record::Core(core::Record::People(people::Stored::Answer { outcome, .. }))
                if matches!(outcome, people::Outcome::ProposalDecided { proposer, proposal, by, at, choice }
                    if *proposer == original.proposer && *proposal == original.proposal && *by == maintainer
                        && *at == original.at && *choice == original.choice)
            ))
            .count(),
        2
    );
    assert_eq!(
        world
            .store
            .rows
            .values()
            .filter(|row| matches!(row,
                root::Record::Core(core::Record::Tasks(tasks::Stored::Ended(task)))
                if task.tracked == Some(7) && task.numbers.budget == 150
            ))
            .count(),
        1
    );
    assert_eq!(
        world.peers.as_ref().expect("actual worker").workers[0]
            .seen
            .iter()
            .map(|row| (row.task, row.attempt))
            .collect::<Vec<_>>(),
        assignments,
        "the second decision starts no further attempt"
    );
}
