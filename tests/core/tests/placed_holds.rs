use jig_core_world::effects::World;
use jig_core_world::fixtures::{ask_as, delegate, inbox_objects, live_row, member, refusal, second_party};
use jig_domain_core as core;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_test_domain as root;
use skein_lib::{List, ReplyTo, Token};

struct Story {
    world: World,
    seed: u64,
    owner: u64,
    owner_sign_in: u64,
    coordinator: (u64, u64),
    goal: u64,
    child: u64,
}

fn configuration(seed: u64) -> (root::Config, root::Limits) {
    let (mut config, mut limits) = jig_core_world::peers::fixture(seed, 0);
    config.core.seed_projects[0].policy.policy.roles[0].authority.budget.spend = 100;
    limits.core.fleet.slots = 3;
    limits.core.tasks.inbox_messages = 8;
    limits.core.tasks.inbox_bytes = 512;
    limits.core.people.inbox_recipients = 24;
    jig_core_world::peers::journal(&mut limits);
    (config, limits)
}

fn story(seed: u64) -> Story {
    let (config, limits) = configuration(seed);
    let mut world = World::configured(seed, false, config, limits);
    let owner_sign_in = world.sign_in.expect("owner signed in");
    let coordinator = world.assignments[0];
    let tasks::Party::Person(owner) = live_row(&world, coordinator.0).requester else { panic!("owner's coordinator") };
    let mut goal = member(&world, tasks::Executor::Agent { charter: 1 }, b"a goal to pause");
    goal.tracked = Some(1);
    goal.authority.budget.spend = 40;
    goal.authority.delegation.tasks = 2;
    let goal = delegate(&mut world, coordinator, 1, goal);
    world.turn(1, 0, None, b"committed goal turn");
    let goal_run = (goal, live_row(&world, goal).attempt);
    let mut child = member(&world, tasks::Executor::Agent { charter: 1 }, b"the goal's delegate");
    child.authority.delegation = tasks::Delegation { kinds: Box::new([]), tasks: 0, depth: 0 };
    let child = delegate(&mut world, goal_run, 1, child);
    world.turn(1, 0, None, b"committed delegate turn");
    Story { world, seed, owner, owner_sign_in, coordinator, goal, child }
}

fn stop(s: &mut Story, sign_in: u64) -> u64 {
    ask_as(&mut s.world, sign_in, 51, people::Ask::Stop { project: 1, task: s.goal });
    let hold = live_row(&s.world, s.goal).placed.expect("placed stop");
    for task in [s.goal, s.child] {
        assert_eq!(live_row(&s.world, task).placed, Some(hold));
        assert!(s.world.cancellations.iter().any(|run| run.0 == task));
    }
    for task in [s.goal, s.child] {
        let attempt = live_row(&s.world, task).attempt;
        s.world.send(root::Event::Answer {
            read: None,
            refused: Box::new([]),
            channel: Token::new(7),
            task,
            attempt,
            cumulative: 0,
            end: tasks::End::Parked,
        });
    }
    hold
}

fn hold(s: &Story, number: u64) -> tasks::PlacedHold {
    let key = root::Key::Core(core::Key::Tasks(tasks::Key::PlacedHold(number)));
    let Some(root::Record::Core(core::Record::Tasks(tasks::Stored::PlacedHold(row)))) = s.world.store.rows.get(&key)
    else {
        panic!("placed hold retained")
    };
    row.as_ref().clone()
}

fn release(s: &mut Story, sign_in: u64, key: u8) {
    let revision = live_row(&s.world, s.goal).revisions.hold;
    ask_as(&mut s.world, sign_in, key, people::Ask::Release { project: 1, task: s.goal, revision, again: None });
}

fn owner_refusal(hold: u64, person: u64) -> people::Refusal {
    people::Refusal::Authority {
        proposable: None,
        lacking: people::Lack::HoldOwner { hold, owner: people::HoldOwner::Party { person } },
    }
}

fn action(s: &mut Story, completion: u32, action: core::NamedAction) {
    s.world.send(root::Event::Core(core::Event::NamedAction {
        to: ReplyTo::new(Token::new(9400 + u64::from(completion))),
        key: core::CallKey { task: s.coordinator.0, attempt: s.coordinator.1, completion, position: 0 },
        action,
    }));
}

fn propose(s: &mut Story, number: u64) -> u64 {
    let revision = hold(s, number).revision;
    action(
        s,
        3,
        core::NamedAction::Propose {
            action: tasks::ProposalAction::ReleasePlaced { task: s.goal, hold: number, revision },
            reason: b"resume the goal".as_slice().into(),
            as_holder: false,
        },
    );
    let Some((_, _, core::CallPart::Proposed { proposal, holder })) = s.world.answers.last() else {
        panic!("release proposal admitted: {:?}", s.world.answers.last())
    };
    assert_eq!(*holder, tasks::ProposalHolder::Person(s.owner));
    *proposal
}

fn accept(s: &mut Story, proposal: u64) {
    ask_as(
        &mut s.world,
        s.owner_sign_in,
        54,
        people::Ask::DecideProposal {
            project: 1,
            proposer: people::Proposer::Task(s.coordinator.0),
            proposal,
            decision: people::ProposalDecision::Accept,
            running: None,
        },
    );
    assert!(matches!(
        s.world.people_answers.last(),
        Some(people::Reply::Outcome(people::Outcome::ProposalDecided { choice: people::ProposalChoice::Accepted, .. }))
    ));
}

fn resumed(s: &Story, number: u64) {
    for task in [s.goal, s.child] {
        assert_eq!(live_row(&s.world, task).placed, None);
    }
    assert!(!s.world.store.rows.contains_key(&root::Key::Core(core::Key::Tasks(tasks::Key::PlacedHold(number)))));
}

fn transcripts(s: &Story) {
    for (task, turn) in [(s.goal, b"committed goal turn".as_slice()), (s.child, b"committed delegate turn".as_slice())]
    {
        let transcript = s
            .world
            .assignment_transcripts
            .iter()
            .rev()
            .find(|transcript| transcript.task == task)
            .expect("resumed task");
        assert_eq!(transcript.turns.as_ref(), [Box::<[u8]>::from(turn)], "resumed committed conversation");
    }
}

#[test]
fn a_maintainers_release_of_a_goal_the_owner_stopped_is_refused_naming_the_owner() {
    let mut s = story(941);
    let maintainer = second_party(&mut s.world, b"maintainer", people::Role::Maintainer);
    let sign_in = s.owner_sign_in;
    let number = stop(&mut s, sign_in);
    let before = live_row(&s.world, s.goal).clone();
    release(&mut s, maintainer, 52);
    refusal(&s.world, owner_refusal(number, s.owner));
    assert_eq!(live_row(&s.world, s.goal), &before);
    release(&mut s, sign_in, 53);
    resumed(&s, number);
}

#[test]
fn the_coordinator_proposes_the_release_and_the_owner_accepts_it_in_their_inbox() {
    let mut s = story(942);
    let sign_in = s.owner_sign_in;
    let number = stop(&mut s, sign_in);
    let revision = live_row(&s.world, s.goal).revisions.hold;
    let goal = s.goal;
    action(&mut s, 2, core::NamedAction::Release { target: goal, revision, again: None });
    assert!(matches!(s.world.answers.last(), Some((_, _, core::CallPart::HoldOwner {
        hold, owner: people::HoldOwner::Party { person }, .. })) if *hold == number && *person == s.owner));
    let proposal = propose(&mut s, number);
    s.world.reads = Some(jig_core_world::reads::Parties::new(s.world.store.applied));
    assert!(inbox_objects(&mut s.world, sign_in, core::reads::Group::Proposals).contains(
        &core::documents::InboxObject::Proposal { proposer: people::Proposer::Task(s.coordinator.0), number: proposal }
    ));
    accept(&mut s, proposal);
    resumed(&s, number);
    transcripts(&s);
}

#[test]
fn a_proposal_of_a_release_is_withdrawn_when_the_hold_is_lifted() {
    let mut s = story(943);
    let sign_in = s.owner_sign_in;
    let number = stop(&mut s, sign_in);
    let proposal = propose(&mut s, number);
    ask_as(&mut s.world, sign_in, 55, people::Ask::Lift { hold: number, revision: 1, with_tasks: true });
    assert_eq!(
        s.world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::Lifted { hold: number, released: 2 }))
    );
    resumed(&s, number);
    assert!(s.world.store.rows.values().any(|r| matches!(r,
        root::Record::Core(core::Record::Tasks(tasks::Stored::History(h))) if h.proposal.as_ref().is_some_and(|p|
            p.number == proposal && p.state == tasks::ProposalState::Withdrawn))));
}

#[test]
fn a_lift_naming_an_old_revision_or_by_a_maintainer_is_refused() {
    let mut s = story(944);
    let maintainer = second_party(&mut s.world, b"maintainer", people::Role::Maintainer);
    let sign_in = s.owner_sign_in;
    let number = stop(&mut s, sign_in);
    ask_as(&mut s.world, sign_in, 56, people::Ask::Lift { hold: number, revision: 0, with_tasks: true });
    refusal(&s.world, people::Refusal::Moved { now: people::Moved::PlacedHold { revision: 1 } });
    ask_as(&mut s.world, maintainer, 57, people::Ask::Lift { hold: number, revision: 1, with_tasks: true });
    refusal(&s.world, owner_refusal(number, s.owner));
    ask_as(&mut s.world, sign_in, 58, people::Ask::Lift { hold: number, revision: 1, with_tasks: true });
    resumed(&s, number);
}

#[test]
fn an_owner_outranks_a_maintainers_stop() {
    let mut s = story(945);
    let maintainer = second_party(&mut s.world, b"maintainer", people::Role::Maintainer);
    s.world.send(root::Event::Core(core::Event::People(people::Event::Roles {
        project: 1,
        holdings: Box::new([
            people::Holding { person: s.owner, role: people::Role::Member },
            people::Holding { person: 2, role: people::Role::Maintainer },
        ]),
    })));
    let owner = second_party(&mut s.world, b"owner", people::Role::Owner);
    let number = stop(&mut s, maintainer);
    let member = s.owner_sign_in;
    release(&mut s, member, 59);
    refusal(&s.world, owner_refusal(number, 2));
    release(&mut s, owner, 60);
    resumed(&s, number);
}

#[test]
fn a_compound_release_is_refused_whole_by_the_owners_rule() {
    let mut s = story(946);
    let maintainer = second_party(&mut s.world, b"maintainer", people::Role::Maintainer);
    s.world.send(root::Event::Core(core::Event::People(people::Event::Roles {
        project: 1,
        holdings: Box::new([
            people::Holding { person: s.owner, role: people::Role::Member },
            people::Holding { person: 2, role: people::Role::Maintainer },
        ]),
    })));
    let number = stop(&mut s, maintainer);
    let before = live_row(&s.world, s.goal).clone();
    let mut operations = List::with_capacity(2);
    operations
        .push(people::Operation::Message { task: s.goal, words: b"resume".as_slice().into() })
        .expect("two operations");
    operations
        .push(people::Operation::Release { task: s.goal, revision: before.revisions.hold, again: None })
        .expect("two operations");
    ask_as(&mut s.world, s.owner_sign_in, 61, people::Ask::Compound { project: 1, operations });
    assert_eq!(
        s.world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::CompoundRefused { at: 1, why: owner_refusal(number, 2) }))
    );
    assert_eq!(live_row(&s.world, s.goal), &before);
}

#[test]
fn pending_release_proposals_and_placed_holds_survive_a_restart() {
    let mut s = story(947);
    let sign_in = s.owner_sign_in;
    let number = stop(&mut s, sign_in);
    let proposal = propose(&mut s, number);
    s.world.restart_with(configuration(s.seed).0);
    s.world.send(root::Event::Core(core::Event::Fleet(jig_domain_fleet::Event::Hello {
        worker: 0,
        channel: Token::new(7),
        hello: jig_domain_fleet::Hello {
            bounds: jig_core_world::fixtures::bounds(),
            stop_bound: skein_lib::Duration::from_millis(100),
            slots: 3,
            workstreams: Box::new([]),
            hosting: Box::new([jig_domain_fleet::Hosted {
                run: Token::new(s.coordinator.0),
                attempt: Token::new(s.coordinator.1),
                phase: jig_domain_fleet::Phase::Active,
            }]),
        },
    })));
    s.world.send(root::Event::Core(core::Event::Account(jig_domain_accounts::Event::Component(
        jig_domain_accounts::FromComponent::Granted {
            form: jig_domain_accounts::Form::Credential(jig_domain_accounts::Scheme::Bearer),
            account: 1,
            generation: 2,
            valid: skein_lib::Duration::from_secs(60),
        },
    ))));
    assert_eq!(hold(&s, number).owner, tasks::Owner::Party { person: s.owner });
    accept(&mut s, proposal);
    resumed(&s, number);
    transcripts(&s);
}

#[test]
fn a_roleless_hold_owner_is_unknown_and_a_member_outranks_their_former_role() {
    let mut s = story(948);
    let member = second_party(&mut s.world, b"member", people::Role::Member);
    let sign_in = s.owner_sign_in;
    let number = stop(&mut s, sign_in);
    s.world.send(root::Event::Core(core::Event::People(people::Event::Roles {
        project: 1,
        holdings: Box::new([people::Holding { person: 2, role: people::Role::Member }]),
    })));
    ask_as(&mut s.world, sign_in, 62, people::Ask::Lift { hold: number, revision: 0, with_tasks: true });
    refusal(&s.world, people::Refusal::Unknown);
    ask_as(&mut s.world, member, 63, people::Ask::Lift { hold: number, revision: 1, with_tasks: true });
    assert_eq!(
        s.world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::Lifted { hold: number, released: 2 }))
    );
    resumed(&s, number);
}

#[test]
fn rejecting_a_release_proposal_keeps_the_goal_and_delegate_held() {
    let mut s = story(949);
    let sign_in = s.owner_sign_in;
    let number = stop(&mut s, sign_in);
    let proposal = propose(&mut s, number);
    ask_as(
        &mut s.world,
        sign_in,
        64,
        people::Ask::DecideProposal {
            project: 1,
            proposer: people::Proposer::Task(s.coordinator.0),
            proposal,
            decision: people::ProposalDecision::Reject { reason: b"keep it stopped".as_slice().into() },
            running: None,
        },
    );
    for task in [s.goal, s.child] {
        assert_eq!(live_row(&s.world, task).placed, Some(number));
    }
    assert!(!hold(&s, number).lifted);
}

#[test]
fn a_maintainer_cannot_release_an_owners_hold_as_part_of_an_amendment() {
    let mut s = story(950);
    let maintainer = second_party(&mut s.world, b"maintainer", people::Role::Maintainer);
    let sign_in = s.owner_sign_in;
    let number = stop(&mut s, sign_in);
    let before = live_row(&s.world, s.goal).clone();
    let amendment = jig_core_world::fixtures::person_unchanged(b"resume");
    ask_as(
        &mut s.world,
        maintainer,
        65,
        people::Ask::Amend {
            project: 1,
            task: s.goal,
            revision: before.revisions.spec,
            amendment: amendment.clone(),
            release: Some(before.revisions.hold),
        },
    );
    refusal(&s.world, owner_refusal(number, s.owner));
    let mut operations = List::with_capacity(1);
    operations
        .push(people::Operation::Amend {
            task: s.goal,
            revision: before.revisions.spec,
            amendment: Box::new(amendment),
            release: Some(before.revisions.hold),
        })
        .expect("one amendment");
    ask_as(&mut s.world, maintainer, 66, people::Ask::Compound { project: 1, operations });
    assert_eq!(
        s.world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::CompoundRefused { at: 0, why: owner_refusal(number, s.owner) }))
    );
    assert_eq!(live_row(&s.world, s.goal), &before);
}
