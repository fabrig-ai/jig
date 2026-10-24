use jig_domain_tasks::{
    Event, Funder, MessageKind, Party, Proposal, ProposalAction, ProposalDecision, ProposalHolder, ProposalKind,
    ProposalState,
};
use jig_tasks_world::{LIMITS, Reply, World, task};

#[test]
fn a_proposal_reroute_only_applies_to_the_holder_and_revision_it_checked() {
    let mut world = World::new(801, LIMITS);
    assert_eq!(world.make(Party::Person(9), vec![task(1, &[])]), Reply::Made(vec![1]));
    assert_eq!(world.make(Party::Task(1), vec![task(2, &[])]), Reply::Made(vec![2]));
    let mut member = task(3, &[]);
    member.funder = Funder::Task(2);
    let proposal = Proposal {
        number: 100,
        proposer: 2,
        project: 1,
        action: ProposalAction::Batch(Box::new([member])),
        reason: Box::from(&b"need one delegate"[..]),
        as_holder: false,
        state: ProposalState::Pending { holder: ProposalHolder::Task(1), since: world.env.wall },
    };
    let reply_to = world.to();
    world.send(Event::Propose { reply_to, proposal });
    let revision = world.record(2).revision;
    let original = world.record(2).proposal.clone();
    world.send(Event::StalledProposal {
        proposer: 2,
        proposal: 100,
        from: ProposalHolder::Task(1),
        revision: revision - 1,
        holder: ProposalHolder::Person(9),
    });
    assert_eq!(world.record(2).proposal, original);
    world.send(Event::StalledProposal {
        proposer: 2,
        proposal: 100,
        from: ProposalHolder::Task(1),
        revision,
        holder: ProposalHolder::Person(9),
    });
    assert!(matches!(
        world.record(2).proposal.as_ref().expect("pending").state,
        ProposalState::Pending { holder: ProposalHolder::Person(9), .. }
    ));
    world.send(Event::StalledProposal {
        proposer: 2,
        proposal: 100,
        from: ProposalHolder::Task(1),
        revision,
        holder: ProposalHolder::Policy { project: 1, kind: ProposalKind::Batch },
    });
    assert!(matches!(
        world.record(2).proposal.as_ref().expect("pending").state,
        ProposalState::Pending { holder: ProposalHolder::Person(9), .. }
    ));
    let reply_to = world.to();
    world.send(Event::DecideProposal {
        reply_to,
        proposer: 2,
        proposal: 100,
        message: Some(101),
        by: Party::Person(9),
        decision: ProposalDecision::Reject { reason: Box::from(&b"not now"[..]) },
    });
    assert!(world.record(2).proposal.is_none());
    assert!(
        world
            .record(2)
            .inbox
            .iter()
            .any(|word| word.kind == MessageKind::ProposalDecision { proposal: 100, accepted: false })
    );
    world.restart();
}

fn propose_batch(world: &mut World, proposer: u64, number: u64, batch: Vec<jig_domain_tasks::New>) {
    let mut batch = batch.into_boxed_slice();
    for member in &mut batch {
        member.funder = Funder::Pool { project: 1, person: proposer, period: 0 };
    }
    let reply_to = world.to();
    world.send(Event::ProposePerson {
        reply_to,
        proposal: jig_domain_tasks::PersonProposal {
            number,
            proposer,
            project: 1,
            action: jig_domain_tasks::PersonAction::Batch(batch),
            reason: Box::from(b"please fund this work".as_slice()),
            state: jig_domain_tasks::PersonProposalState::Pending { since: world.env.wall },
        },
    });
}
#[test]
fn a_partys_batch_proposal_is_kept_until_decided() {
    let mut world = World::new(340, LIMITS);
    propose_batch(&mut world, 9, 100, vec![task(1, &[]), task(2, &[1])]);
    let before = world.records[&jig_domain_tasks::Key::PersonProposal(100)].clone();
    world.restart();
    assert_eq!(world.records[&jig_domain_tasks::Key::PersonProposal(100)], before);
    assert!(!world.records.contains_key(&jig_domain_tasks::Key::Live(1)));
}
#[test]
fn a_partys_withdrawn_proposal_is_decided_withdrawn() {
    let mut world = World::new(341, LIMITS);
    propose_batch(&mut world, 9, 100, vec![task(1, &[])]);
    let reply_to = world.to();
    world.send(Event::WithdrawPersonProposal { reply_to: Some(reply_to), proposer: 9, proposal: 100 });
    let jig_domain_tasks::Stored::DecidedProposal(row) = &world.records[&jig_domain_tasks::Key::DecidedProposal(100)]
    else {
        panic!("decided proposal")
    };
    assert_eq!(row.state, jig_domain_tasks::PersonProposalState::Withdrawn { at: world.env.wall });
    assert!(!world.records.contains_key(&jig_domain_tasks::Key::PersonProposal(100)));
    world.restart();
}
#[test]
fn withdrawing_another_partys_proposal_is_unknown() {
    let mut world = World::new(342, LIMITS);
    propose_batch(&mut world, 9, 100, vec![task(1, &[])]);
    let reply_to = world.to();
    world.send(Event::WithdrawPersonProposal { reply_to: Some(reply_to), proposer: 8, proposal: 100 });
    assert_eq!(
        world.replies.last_key_value().expect("withdrawal reply").1.clone(),
        Reply::Refused(jig_domain_tasks::Problem {
            task: None,
            why: jig_domain_tasks::Refusal::Unknown,
            blocked_by: None,
            capacity: None
        })
    );
    assert!(world.records.contains_key(&jig_domain_tasks::Key::PersonProposal(100)));
}
#[test]
fn a_batch_proposal_naming_a_task_since_ended_is_withdrawn() {
    let mut world = World::new(343, LIMITS);
    assert_eq!(world.make(Party::Person(9), vec![task(1, &[])]), Reply::Made(vec![1]));
    propose_batch(&mut world, 9, 100, vec![task(2, &[1])]);
    let reply_to = world.to();
    world.send(Event::Control {
        reply_to,
        by: Party::Person(9),
        task: 1,
        control: jig_domain_tasks::Control::Cancel { reason: Box::new([]), message: None, grace: None },
    });
    world.settle(1);
    let jig_domain_tasks::Stored::DecidedProposal(row) = &world.records[&jig_domain_tasks::Key::DecidedProposal(100)]
    else {
        panic!("decided proposal")
    };
    assert_eq!(row.state, jig_domain_tasks::PersonProposalState::Withdrawn { at: world.env.wall });
}

fn propose_amendment(world: &mut World) {
    let reply_to = world.to();
    let mut authority = task(1, &[]).authority;
    authority.budget.spend = 150;
    world.send(Event::ProposePerson {
        reply_to,
        proposal: jig_domain_tasks::PersonProposal {
            number: 100,
            proposer: 9,
            project: 1,
            action: jig_domain_tasks::PersonAction::Amend {
                task: 1,
                revision: 0,
                amendment: Box::new(jig_domain_tasks::Amendment {
                    tracked: None,
                    addressed: None,
                    instructions: None,
                    release: None,
                    spec: None,
                    wake: None,
                    dependencies: None,
                    authority: Some(authority),
                    llm: None,
                    reason: b"more work".as_slice().into(),
                }),
            },
            reason: b"please widen".as_slice().into(),
            state: jig_domain_tasks::PersonProposalState::Pending { since: world.env.wall },
        },
    });
}
#[test]
fn a_partys_amendment_proposal_names_the_party_as_proposer() {
    let mut world = World::new(344, LIMITS);
    assert_eq!(world.make(Party::Person(9), vec![task(1, &[])]), Reply::Made(vec![1]));
    propose_amendment(&mut world);
    let jig_domain_tasks::Stored::PersonProposal(row) = &world.records[&jig_domain_tasks::Key::PersonProposal(100)]
    else {
        panic!("pending proposal")
    };
    assert_eq!(row.proposer, 9);
    assert_eq!(row.first(), 1);
    assert_eq!(world.record(1).proposal, None);
    world.restart();
    assert_eq!(world.record(1).proposal, None);
}
#[test]
fn an_amendment_proposal_whose_task_ends_is_withdrawn() {
    let mut world = World::new(345, LIMITS);
    assert_eq!(world.make(Party::Person(9), vec![task(1, &[])]), Reply::Made(vec![1]));
    propose_amendment(&mut world);
    let reply_to = world.to();
    world.send(Event::Control {
        reply_to,
        by: Party::Person(9),
        task: 1,
        control: jig_domain_tasks::Control::Cancel { reason: Box::new([]), message: None, grace: None },
    });
    world.settle(1);
    let jig_domain_tasks::Stored::DecidedProposal(row) = &world.records[&jig_domain_tasks::Key::DecidedProposal(100)]
    else {
        panic!("withdrawn proposal")
    };
    assert_eq!(row.state, jig_domain_tasks::PersonProposalState::Withdrawn { at: world.env.wall });
    assert!(!world.records.contains_key(&jig_domain_tasks::Key::PersonProposal(100)));
    world.restart();
}
