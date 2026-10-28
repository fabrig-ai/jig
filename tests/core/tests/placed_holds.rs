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
    configured_story(seed, config, &limits)
}

fn configured_story(seed: u64, config: root::Config, limits: &root::Limits) -> Story {
    let mut world = World::configured(seed, false, config, *limits);
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

fn placed_hold(s: &Story, number: u64) -> tasks::PlacedHold {
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
        proposable: Some(people::Accepter::HoldOwner),
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
    let revision = placed_hold(s, number).revision;
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
    refusal(
        &s.world,
        people::Refusal::Authority {
            lacking: people::Lack::HoldOwner { hold: number, owner: people::HoldOwner::Party { person: s.owner } },
            proposable: None,
        },
    );
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
    assert_eq!(placed_hold(&s, number).owner, tasks::Owner::Party { person: s.owner });
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
    assert!(!placed_hold(&s, number).lifted);
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

// A party places a hold explicitly, independently of the task's Stop control.
fn hold(
    s: &mut Story,
    sign_in: u64,
    key: u8,
    mode: people::HoldMode,
    scope: people::HoldScope,
    reason: &[u8],
) -> people::Outcome {
    ask_as(&mut s.world, sign_in, key, people::Ask::Hold { project: 1, mode, scope, reason: reason.into() });
    match s.world.people_answers.last() {
        Some(people::Reply::Outcome(outcome)) => outcome.clone(),
        Some(people::Reply::Refused(why)) => people::Outcome::Refused(why.clone()),
        reply => panic!("Hold has a permanent answer: {reply:?}"),
    }
}

fn placed_number(outcome: &people::Outcome) -> u64 {
    let people::Outcome::Placed { hold, revision } = outcome else { panic!("placed answer: {outcome:?}") };
    assert_ne!(*hold, 0);
    assert_eq!(*revision, 1);
    *hold
}

fn parked(s: &mut Story) {
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
}

fn holds_count(s: &Story) -> usize {
    s.world
        .store
        .rows
        .values()
        .filter(|row| matches!(row, root::Record::Core(core::Record::Tasks(tasks::Stored::PlacedHold(_)))))
        .count()
}

fn message_number(s: &Story) -> u64 {
    s.world
        .store
        .rows
        .values()
        .find_map(|row| {
            if let root::Record::Core(core::Record::Core(core::CoreRecord::Deployment(header))) = row {
                Some(header.messages)
            } else {
                None
            }
        })
        .expect("deployment high-water marks")
}

#[test]
fn a_hold_over_a_goal_places_the_owners_stop_with_its_reason() {
    let mut s = story(970);
    let sign_in = s.owner_sign_in;
    let scope = people::HoldScope::Subtree { task: s.goal };
    let number = placed_number(&hold(&mut s, sign_in, 71, people::HoldMode::Stop, scope, b"pause for inspection"));
    let row = placed_hold(&s, number);
    assert_eq!(row.owner, tasks::Owner::Party { person: s.owner });
    assert_eq!(
        (row.project, row.mode, row.scope, row.revision),
        (1, tasks::Mode::Stop, tasks::Scope::Subtree { task: s.goal }, 1)
    );
    assert_eq!(row.reason.as_ref(), b"pause for inspection");
    assert!(!row.lifted && row.exempt.is_empty());
    for task in [s.goal, s.child] {
        assert_eq!(live_row(&s.world, task).placed, Some(number));
        assert!(
            matches!(live_row(&s.world, task).phase, tasks::Phase::Held { why: tasks::Hold::Placed { hold }, .. } if hold == number)
        );
        assert!(s.world.cancellations.iter().any(|run| run.0 == task));
    }
    assert_eq!(holds_count(&s), 1);
}

#[test]
fn a_project_hold_by_the_owner_holds_every_task_but_their_chat() {
    let mut s = story(971);
    let sign_in = s.owner_sign_in;
    let chat = live_row(&s.world, s.coordinator.0).clone();
    assert!(chat.flags.conversation);
    assert_eq!(chat.requester, tasks::Party::Person(s.owner));
    let number = placed_number(&hold(
        &mut s,
        sign_in,
        71,
        people::HoldMode::Stop,
        people::HoldScope::Project,
        b"freeze this project",
    ));
    assert_eq!(placed_hold(&s, number).scope, tasks::Scope::Project);
    assert_eq!(live_row(&s.world, s.coordinator.0), &chat);
    assert!(!s.world.cancellations.iter().any(|run| run.0 == s.coordinator.0));
    for task in [s.goal, s.child] {
        assert_eq!(live_row(&s.world, task).placed, Some(number));
        assert!(s.world.cancellations.iter().any(|run| run.0 == task));
    }
    parked(&mut s);
    let ask = jig_core_world::fixtures::goal(
        &s.world,
        1,
        b"a new goal under the hold".as_slice().into(),
        people::TaskExecutor::Agent { charter: 1 },
        jig_core_world::fixtures::Goal { budget: 5, priority: 1, deadline: None },
    );
    ask_as(&mut s.world, sign_in, 72, ask);
    let Some(people::Reply::Outcome(people::Outcome::Created { first: later, .. })) = s.world.people_answers.last()
    else {
        panic!("goal created while project held: {:?}", s.world.people_answers.last())
    };
    let later = *later;
    assert_eq!(live_row(&s.world, later).placed, Some(number));
    assert!(!s.world.assignments.iter().any(|run| run.0 == later));
    ask_as(&mut s.world, sign_in, 73, people::Ask::Lift { hold: number, revision: 1, with_tasks: true });
    assert_eq!(
        s.world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::Lifted { hold: number, released: 3 }))
    );
    for task in [s.goal, s.child, later] {
        assert_eq!(live_row(&s.world, task).placed, None);
    }
}

#[test]
fn a_member_may_hold_a_task_in_their_tree_but_not_a_project() {
    let mut s = story(972);
    let member_sign_in = second_party(&mut s.world, b"member", people::Role::Member);
    assert_eq!(
        hold(&mut s, member_sign_in, 71, people::HoldMode::Stop, people::HoldScope::Project, b"pause"),
        people::Outcome::Refused(people::Refusal::Role { held: Some(people::Role::Member) })
    );
    let scope = people::HoldScope::Subtree { task: s.goal };
    let refused = hold(&mut s, member_sign_in, 72, people::HoldMode::Stop, scope, b"outside my tree");
    let goal = s.goal;
    ask_as(&mut s.world, member_sign_in, 73, people::Ask::Stop { project: 1, task: goal });
    assert_eq!(refused, people::Outcome::Refused(people::Refusal::Unknown));
    assert_eq!(s.world.people_answers.last(), Some(&people::Reply::Outcome(refused)));
    assert_eq!(holds_count(&s), 0);
    let ask = jig_core_world::fixtures::goal(
        &s.world,
        1,
        b"member's own goal".as_slice().into(),
        people::TaskExecutor::Agent { charter: 1 },
        jig_core_world::fixtures::Goal { budget: 5, priority: 1, deadline: None },
    );
    ask_as(&mut s.world, member_sign_in, 74, ask);
    let Some(people::Reply::Outcome(people::Outcome::Created { first: own, .. })) = s.world.people_answers.last()
    else {
        panic!("member's goal: {:?}", s.world.people_answers.last())
    };
    let own = *own;
    let scope = people::HoldScope::Subtree { task: own };
    let number = placed_number(&hold(&mut s, member_sign_in, 75, people::HoldMode::Stop, scope, b"my own task"));
    assert_eq!(live_row(&s.world, own).placed, Some(number));
    assert_eq!(live_row(&s.world, s.goal).placed, None);
}

#[test]
fn a_hold_without_a_reason_or_past_the_words_bound_is_refused() {
    let mut s = story(973);
    let sign_in = s.owner_sign_in;
    let bound = configuration(s.seed).1.core.people.words;
    let messages = message_number(&s);
    for (key, reason, which) in [
        (71, Vec::new(), people::LimitKind::Empty),
        (72, vec![b'x'; usize::try_from(bound).expect("words bound") + 1], people::LimitKind::Words),
    ] {
        assert_eq!(
            hold(&mut s, sign_in, key, people::HoldMode::Stop, people::HoldScope::Project, &reason),
            people::Outcome::Refused(people::Refusal::Limit { which })
        );
    }
    assert_eq!(holds_count(&s), 0);
    assert_eq!(message_number(&s), messages);
    let reason = vec![b'x'; usize::try_from(bound).expect("words bound")];
    let number = placed_number(&hold(&mut s, sign_in, 73, people::HoldMode::Stop, people::HoldScope::Project, &reason));
    assert_eq!(placed_hold(&s, number).reason.as_ref(), reason);
}

#[test]
fn a_drain_is_placed_where_t5b_refused_it() {
    for scope in [people::HoldScope::Project, people::HoldScope::Subtree { task: 2 }] {
        let mut s = story(974);
        let sign_in = s.owner_sign_in;
        let scope = match scope {
            people::HoldScope::Project => people::HoldScope::Project,
            people::HoldScope::Subtree { .. } => people::HoldScope::Subtree { task: s.goal },
        };
        let outcome = hold(&mut s, sign_in, 71, people::HoldMode::Drain, scope, b"finish existing work");
        let number = placed_number(&outcome);
        assert_eq!(holds_count(&s), 1);
        assert_eq!(placed_hold(&s, number).mode, tasks::Mode::Drain);
        for task in [s.goal, s.child] {
            assert_eq!(live_row(&s.world, task).placed, Some(number));
            assert_eq!(
                live_row(&s.world, task).phase,
                tasks::Phase::Active(tasks::Active::Running { attempt: live_row(&s.world, task).attempt })
            );
            assert!(s.world.parks.iter().any(|run| run.0 == task));
        }
        assert!(s.world.cancellations.is_empty());
    }
}

#[test]
fn a_party_drains_a_project_and_a_run_mid_turn_is_parked_at_its_turn() {
    let mut s = story(989);
    let sign_in = s.owner_sign_in;
    let chat = live_row(&s.world, s.coordinator.0).clone();
    let run = (s.goal, live_row(&s.world, s.goal).attempt);
    s.world.store.fault(jig_test_store_files::Fault::Hold { commits: 1 });
    ask_as(
        &mut s.world,
        sign_in,
        71,
        people::Ask::Hold {
            project: 1,
            mode: people::HoldMode::Drain,
            scope: people::HoldScope::Project,
            reason: b"finish the current turns".as_slice().into(),
        },
    );
    assert!(s.world.parks.is_empty(), "park is held behind placement's commit");
    s.world.release_commits();
    let Some(people::Reply::Outcome(outcome)) = s.world.people_answers.last() else { panic!("placed answer") };
    let number = placed_number(outcome);
    assert_eq!(s.world.parks, vec![run, (s.child, live_row(&s.world, s.child).attempt)]);
    assert_eq!(live_row(&s.world, s.coordinator.0), &chat);
    // The park command leaves the current attempt able to commit its selected turn.
    s.world.send(root::Event::Turn {
        opens_window: false,
        refused: Box::new([]),
        channel: Token::new(7),
        task: run.0,
        attempt: run.1,
        turn: 2,
        cumulative: 0,
        read: None,
        transcript: b"selected drain turn".as_slice().into(),
    });
    assert_eq!(live_row(&s.world, s.goal).turn, 2);
    parked(&mut s);
    for task in [s.goal, s.child] {
        let row = live_row(&s.world, task);
        assert_eq!(row.placed, Some(number));
        assert!(matches!(row.phase, tasks::Phase::Held { was: tasks::Was::Active(tasks::Active::Due),
            why: tasks::Hold::Placed { hold }, .. } if hold == number));
        assert_eq!(row.tries, tasks::Tries::NONE);
    }
    let assigned = s.world.assignments.len();
    ask_as(&mut s.world, sign_in, 72, people::Ask::Lift { hold: number, revision: 1, with_tasks: true });
    assert_eq!(
        s.world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::Lifted { hold: number, released: 2 }))
    );
    assert!(s.world.assignments.len() > assigned);
    assert!(s.world.assignments[assigned..].iter().any(|run| run.0 == s.goal));
    assert_eq!(live_row(&s.world, s.goal).placed, None);
}

#[test]
fn a_hold_replayed_under_its_key_answers_the_same_hold() {
    let mut s = story(975);
    let sign_in = s.owner_sign_in;
    let outcome = hold(&mut s, sign_in, 71, people::HoldMode::Stop, people::HoldScope::Project, b"pause once");
    let number = placed_number(&outcome);
    let messages = message_number(&s);
    assert_eq!(hold(&mut s, sign_in, 71, people::HoldMode::Stop, people::HoldScope::Project, b"pause once"), outcome);
    assert_eq!(
        hold(&mut s, sign_in, 71, people::HoldMode::Stop, people::HoldScope::Project, b"a different reason"),
        people::Outcome::Refused(people::Refusal::KeyConflict)
    );
    assert_eq!(message_number(&s), messages);
    assert_eq!(holds_count(&s), 1);
    assert_eq!(placed_hold(&s, number).reason.as_ref(), b"pause once");
}

#[test]
fn a_retained_hold_answer_survives_a_cold_restart() {
    let mut s = story(976);
    let sign_in = s.owner_sign_in;
    let reason = b"pause across restart";
    let scope = people::HoldScope::Subtree { task: s.goal };
    let outcome = hold(&mut s, sign_in, 71, people::HoldMode::Stop, scope, reason);
    let number = placed_number(&outcome);
    let before = placed_hold(&s, number);
    parked(&mut s);
    s.world.restart_with(configuration(s.seed).0);
    assert!(s.world.domain.ready(), "cold engine ready");
    assert_eq!(s.world.domain.restore_failure(), None);
    assert_eq!(placed_hold(&s, number), before);
    let key = people::RequestKey { person: s.owner, key: [71; 16] };
    let Some(root::Record::Core(core::Record::People(people::Stored::Answer { ask, outcome: saved, .. }))) =
        s.world.store.rows.get(&root::Key::Core(core::Key::People(people::Key::Answer(key))))
    else {
        panic!("retained Hold ask and Placed answer restored")
    };
    assert_eq!(
        ask.as_ref(),
        &people::Ask::Hold { project: 1, mode: people::HoldMode::Stop, scope, reason: reason.as_slice().into() }
    );
    assert_eq!(saved, &outcome);
    assert_eq!(hold(&mut s, sign_in, 71, people::HoldMode::Stop, scope, reason), outcome);
    for task in [s.goal, s.child] {
        assert_eq!(live_row(&s.world, task).placed, Some(number));
        assert!(matches!(live_row(&s.world, task).phase, tasks::Phase::Held { .. }));
    }
    assert_eq!(holds_count(&s), 1);
}

#[test]
fn a_maintainer_may_hold_a_project_but_observer_and_policy_roles_may_not() {
    for (seed, role) in
        [(978, people::Role::Maintainer), (979, people::Role::Observer), (980, people::Role::Policy { role: 4 })]
    {
        let (mut config, limits) = configuration(seed);
        if let people::Role::Policy { role } = role {
            config.core.seed_projects[0].policy.policy.roles[3].number = role;
        }
        let mut s = configured_story(seed, config, &limits);
        let sign_in = second_party(&mut s.world, b"project holder", role);
        let outcome = hold(&mut s, sign_in, 71, people::HoldMode::Stop, people::HoldScope::Project, b"project review");
        if role == people::Role::Maintainer {
            let number = placed_number(&outcome);
            assert_eq!(live_row(&s.world, s.goal).placed, Some(number));
            assert_eq!(live_row(&s.world, s.child).placed, Some(number));
        } else {
            assert_eq!(outcome, people::Outcome::Refused(people::Refusal::Role { held: Some(role) }));
            assert_eq!(holds_count(&s), 0);
        }
    }
}

#[test]
fn a_full_placed_hold_table_returns_the_existing_capacity_refusal() {
    let mut s = story(981);
    let sign_in = s.owner_sign_in;
    let scope = people::HoldScope::Subtree { task: s.goal };
    let number = placed_number(&hold(&mut s, sign_in, 71, people::HoldMode::Stop, scope, b"first hold"));
    let expected = people::Outcome::Refused(people::Refusal::Limit { which: people::LimitKind::PlacedHolds });
    for _ in 0..2 {
        assert_eq!(
            hold(&mut s, sign_in, 72, people::HoldMode::Stop, people::HoldScope::Project, b"table is full"),
            expected
        );
    }
    assert_eq!(holds_count(&s), 1);
    assert_eq!(live_row(&s.world, s.goal).placed, Some(number));
    assert_eq!(placed_hold(&s, number).reason.as_ref(), b"first hold");
}

#[test]
fn a_hold_rejects_a_zero_task_and_a_task_named_in_another_project() {
    let mut s = story(982);
    let sign_in = s.owner_sign_in;
    assert_eq!(
        hold(&mut s, sign_in, 71, people::HoldMode::Stop, people::HoldScope::Subtree { task: 0 }, b"invalid task"),
        people::Outcome::Refused(people::Refusal::Unknown)
    );
    ask_as(
        &mut s.world,
        sign_in,
        72,
        people::Ask::Hold {
            project: u32::MAX,
            mode: people::HoldMode::Stop,
            scope: people::HoldScope::Subtree { task: s.goal },
            reason: b"wrong project".as_slice().into(),
        },
    );
    assert_eq!(
        s.world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::Refused(people::Refusal::Role { held: None })))
    );
    ask_as(
        &mut s.world,
        sign_in,
        73,
        people::Ask::Hold {
            project: u32::MAX,
            mode: people::HoldMode::Stop,
            scope: people::HoldScope::Project,
            reason: b"unknown project".as_slice().into(),
        },
    );
    assert!(matches!(
        s.world.people_answers.last(),
        Some(people::Reply::Outcome(people::Outcome::Refused(people::Refusal::Unknown)))
    ));
    assert_eq!(holds_count(&s), 0);
}

fn read_story(seed: u64, rows: u32) -> Story {
    let (config, mut limits) = configuration(seed);
    limits.core.documents.rows = rows;
    limits.core.tasks.placed_holds = 4;
    limits.core.tasks.project_placed_holds = 4;
    limits.core.tasks.party_placed_holds = 4;
    jig_core_world::peers::journal(&mut limits);
    let mut s = configured_story(seed, config, &limits);
    s.world.reads = Some(jig_core_world::reads::Parties::new(s.world.store.applied));
    s
}
fn read_holds(s: &mut Story, sign_in: u64, from: Option<core::reads::Cursor>) -> core::reads::Answer {
    s.world.sign_in = Some(sign_in);
    jig_core_world::reads::ask(&mut s.world, 92, core::reads::Read::Holds { project: 1, from });
    jig_core_world::reads::latest(&s.world).clone()
}
fn holds_page(answer: core::reads::Answer) -> Box<core::documents::ProjectHolds> {
    let core::reads::Answer::Holds(page) = answer else { panic!("holds page: {answer:?}") };
    assert_eq!(page.project, 1);
    page
}

#[test]
fn a_projects_holds_read_lists_each_hold_with_its_tasks_and_exemptions() {
    let mut s = read_story(983, 4);
    let sign_in = s.owner_sign_in;
    let scope = people::HoldScope::Subtree { task: s.child };
    let first = placed_number(&hold(&mut s, sign_in, 71, people::HoldMode::Stop, scope, b"inspect delegate"));
    let second = placed_number(&hold(
        &mut s,
        sign_in,
        72,
        people::HoldMode::Stop,
        people::HoldScope::Project,
        b"inspect project",
    ));
    let revision = live_row(&s.world, s.goal).revisions.hold;
    ask_as(&mut s.world, sign_in, 73, people::Ask::Release { project: 1, task: s.goal, revision, again: None });
    let page = holds_page(read_holds(&mut s, sign_in, None));
    assert_eq!(page.rows.len(), 2);
    for (row, number, held) in [(&page.rows[0], first, 1), (&page.rows[1], second, 0)] {
        let source = placed_hold(&s, number);
        assert_eq!(row.hold.number, number);
        assert_eq!(row.hold.owner, people::HoldOwner::Party { person: s.owner });
        assert_eq!(row.hold.mode, source.mode);
        assert_eq!(row.hold.scope, source.scope);
        assert_eq!(row.hold.reason, source.reason);
        assert_eq!(row.hold.placed_at.as_nanos(), source.placed_at * 1_000_000);
        assert_eq!(row.hold.revision, source.revision);
        assert!(!row.hold.lifted);
        assert!(row.hold.may_lift);
        assert_eq!(row.held, held);
        assert_eq!(row.exempt, source.exempt);
    }
    assert_eq!(page.rows[1].exempt.as_ref(), &[s.goal]);
    assert!(page.after.is_none());
}
#[test]
fn a_holds_read_says_which_holds_the_reader_may_lift() {
    let mut s = read_story(984, 4);
    let sign_in = s.owner_sign_in;
    let maintainer = second_party(&mut s.world, b"maintainer", people::Role::Maintainer);
    stop(&mut s, sign_in);
    assert!(holds_page(read_holds(&mut s, sign_in, None)).rows[0].hold.may_lift);
    assert!(!holds_page(read_holds(&mut s, maintainer, None)).rows[0].hold.may_lift);
    s.world.send(root::Event::Core(core::Event::People(people::Event::Roles {
        project: 1,
        holdings: Box::new([
            people::Holding { person: s.owner, role: people::Role::Owner },
            people::Holding { person: 2, role: people::Role::Observer },
        ]),
    })));
    assert!(!holds_page(read_holds(&mut s, maintainer, None)).rows[0].hold.may_lift);
}
#[test]
fn a_lifted_hold_is_read_while_it_holds_a_task() {
    let mut s = read_story(985, 4);
    let sign_in = s.owner_sign_in;
    let number = stop(&mut s, sign_in);
    ask_as(&mut s.world, sign_in, 72, people::Ask::Lift { hold: number, revision: 1, with_tasks: false });
    let page = holds_page(read_holds(&mut s, sign_in, None));
    assert_eq!(page.rows[0].held, 2);
    assert!(page.rows[0].hold.lifted);
    release(&mut s, sign_in, 73);
    assert_eq!(holds_page(read_holds(&mut s, sign_in, None)).rows[0].held, 1);
    let revision = live_row(&s.world, s.child).revisions.hold;
    ask_as(&mut s.world, sign_in, 74, people::Ask::Release { project: 1, task: s.child, revision, again: None });
    assert!(holds_page(read_holds(&mut s, sign_in, None)).rows.is_empty());
}
#[test]
fn a_holds_read_pages_past_its_row_bound() {
    let mut s = read_story(986, 1);
    let sign_in = s.owner_sign_in;
    let mut numbers = Vec::new();
    for key in 71..74 {
        numbers.push(placed_number(&hold(
            &mut s,
            sign_in,
            key,
            people::HoldMode::Stop,
            people::HoldScope::Project,
            b"paged project hold",
        )));
    }
    let mut from = None;
    let mut seen = Vec::new();
    loop {
        let page = holds_page(read_holds(&mut s, sign_in, from));
        assert_eq!(page.rows.len(), 1);
        seen.push(page.rows[0].hold.number);
        if let Some(core::reads::Cursor::Live { after: core::documents::Cursor::Number { number }, .. }) = &page.after {
            assert_eq!(*number, page.rows[0].hold.number);
        }
        from = page.after;
        if from.is_none() {
            break;
        }
    }
    assert_eq!(seen, numbers);
    let wrong = core::reads::Cursor::Live {
        scope: core::reads::Scope::Holds { project: 2 },
        after: core::documents::Cursor::Number { number: 1 },
        store_after: None,
    };
    assert_eq!(read_holds(&mut s, sign_in, Some(wrong)), core::reads::Answer::None(core::reads::Why::Unknown));
}
#[test]
fn a_holds_read_of_a_project_without_a_role_is_unknown() {
    let mut s = read_story(987, 4);
    let sign_in = s.owner_sign_in;
    stop(&mut s, sign_in);
    s.world.send(root::Event::Core(core::Event::People(people::Event::Roles { project: 1, holdings: Box::new([]) })));
    assert_eq!(read_holds(&mut s, sign_in, None), core::reads::Answer::None(core::reads::Why::Unknown));
}

#[test]
fn a_holds_pages_price_includes_whole_reasons_and_exemptions() {
    use jig_protocol_config::profile::{Profile, profile};
    for profile_name in [Profile::Small, Profile::Standard] {
        let limits = profile(profile_name).core;
        let before = core::documents::document_worst_case(&limits.documents).expect("document price");
        let holds = core::documents::project_holds_worst_case(&limits).expect("holds price");
        assert!(holds <= before, "the existing largest document covers this page");
        let context = core::documents::document_with_context_worst_case(&limits).expect("context price");
        eprintln!(
            "{profile_name:?}: document before={before} after={before}; holds={holds}; context before={context} after={context}"
        );
    }
}

#[test]
fn a_party_can_exempt_a_live_draining_run_before_it_parks() {
    let mut s = story(990);
    let sign_in = s.owner_sign_in;
    let outcome = hold(&mut s, sign_in, 71, people::HoldMode::Drain, people::HoldScope::Project, b"finish turns");
    let number = placed_number(&outcome);
    let before = live_row(&s.world, s.goal).phase.clone();
    release(&mut s, sign_in, 72);
    assert_eq!(
        s.world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::Released { task: s.goal }))
    );
    assert_eq!(live_row(&s.world, s.goal).placed, None);
    assert_eq!(live_row(&s.world, s.goal).phase, before);
    assert_eq!(placed_hold(&s, number).exempt.as_ref(), &[s.goal]);
    let assigned = s.world.assignments.len();
    parked(&mut s);
    assert!(s.world.assignments[assigned..].iter().any(|run| run.0 == s.goal));
    assert!(matches!(live_row(&s.world, s.child).phase, tasks::Phase::Held { .. }));
}

#[test]
fn a_project_drain_reasserts_its_parks_after_the_core_adopts_restored_runs() {
    use jig_domain_fleet as fleet;
    let mut s = story(991);
    let sign_in = s.owner_sign_in;
    let outcome = hold(&mut s, sign_in, 71, people::HoldMode::Drain, people::HoldScope::Project, b"finish turns");
    let number = placed_number(&outcome);
    let hosting = [s.coordinator.0, s.goal, s.child].map(|task| fleet::Hosted {
        run: Token::new(task),
        attempt: Token::new(live_row(&s.world, task).attempt),
        phase: fleet::Phase::Active,
    });
    s.world.parks.clear();
    s.world.restart_with(configuration(s.seed).0);
    s.world.send(root::Event::Core(core::Event::Fleet(fleet::Event::Hello {
        worker: 0,
        channel: Token::new(7),
        hello: fleet::Hello {
            bounds: jig_core_world::fixtures::bounds(),
            stop_bound: skein_lib::Duration::from_millis(100),
            slots: 3,
            workstreams: Box::new([]),
            hosting: Box::new(hosting),
        },
    })));
    assert_eq!(s.world.parks.len(), 2);
    for task in [s.goal, s.child] {
        assert!(s.world.parks.iter().any(|run| run.0 == task));
        assert_eq!(live_row(&s.world, task).placed, Some(number));
    }
    assert_eq!(live_row(&s.world, s.coordinator.0).placed, None);
    parked(&mut s);
    for task in [s.goal, s.child] {
        assert!(matches!(live_row(&s.world, task).phase, tasks::Phase::Held { .. }));
    }
}

fn maintainer(s: &mut Story) -> (u64, u64) {
    let sign_in = second_party(&mut s.world, b"proposing maintainer", people::Role::Maintainer);
    let Some(people::Reply::SignedIn { person, .. }) = s.world.people_answers.last() else {
        panic!("maintainer signed in")
    };
    (sign_in, *person)
}
fn party_propose(s: &mut Story, sign_in: u64, task: u64, hold: u64, key: u8) -> u64 {
    let revision = placed_hold(s, hold).revision;
    ask_as(
        &mut s.world,
        sign_in,
        key,
        people::Ask::Propose {
            project: 1,
            action: people::Action::ReleasePlaced { task, hold, revision },
            reason: b"resume this task".as_slice().into(),
        },
    );
    let Some(people::Reply::Outcome(people::Outcome::ReleaseProposed { proposal })) = s.world.people_answers.last()
    else {
        panic!("release proposed: {:?}", s.world.people_answers.last())
    };
    *proposal
}
fn party_decide(
    s: &mut Story,
    sign_in: u64,
    proposer: u64,
    proposal: u64,
    decision: people::ProposalDecision,
    key: u8,
) {
    ask_as(
        &mut s.world,
        sign_in,
        key,
        people::Ask::DecideProposal {
            project: 1,
            proposer: people::Proposer::Party(proposer),
            proposal,
            decision,
            running: None,
        },
    );
}
fn party_proposal(s: &Story, number: u64) -> tasks::PersonProposal {
    s.world
        .store
        .rows
        .values()
        .find_map(|row| match row {
            root::Record::Core(core::Record::Tasks(
                tasks::Stored::PersonProposal(row) | tasks::Stored::DecidedProposal(row),
            )) if row.number == number => Some(*row.clone()),
            _ => None,
        })
        .expect("durable party proposal")
}

#[test]
fn a_maintainers_refused_release_says_the_owner_may_accept_a_proposal() {
    let mut s = story(1010);
    let (sign_in, _) = maintainer(&mut s);
    let owner = s.owner_sign_in;
    let number = stop(&mut s, owner);
    release(&mut s, sign_in, 80);
    refusal(&s.world, owner_refusal(number, s.owner));
}

#[test]
fn a_maintainer_proposes_the_release_and_the_owner_accepts_it_in_their_inbox() {
    let mut s = story(1011);
    let (sign_in, person) = maintainer(&mut s);
    let owner = s.owner_sign_in;
    let number = stop(&mut s, owner);
    let goal = s.goal;
    let proposal = party_propose(&mut s, sign_in, goal, number, 80);
    assert_eq!(party_proposal(&s, proposal).first(), goal);
    s.world.reads = Some(jig_core_world::reads::Parties::new(s.world.store.applied));
    let object = core::documents::InboxObject::Proposal { proposer: people::Proposer::Party(person), number: proposal };
    assert!(inbox_objects(&mut s.world, owner, core::reads::Group::Proposals).contains(&object));
    assert!(!inbox_objects(&mut s.world, sign_in, core::reads::Group::Proposals).contains(&object));
    // The maintainer cannot accept their own offer, even with policy authority over its tree.
    party_decide(&mut s, sign_in, person, proposal, people::ProposalDecision::Accept, 81);
    refusal(&s.world, owner_refusal(number, s.owner));
    party_decide(&mut s, owner, person, proposal, people::ProposalDecision::Pass, 82);
    refusal(&s.world, people::Refusal::NoFurther);
    party_decide(&mut s, owner, person, proposal, people::ProposalDecision::Accept, 83);
    assert_eq!(
        party_proposal(&s, proposal).state,
        tasks::PersonProposalState::Accepted { by: tasks::Party::Person(s.owner) }
    );
    resumed(&s, number);
    transcripts(&s);
}

#[test]
fn a_party_release_proposal_of_a_delegate_exempts_it_alone_when_accepted() {
    let mut s = story(1012);
    let (sign_in, person) = maintainer(&mut s);
    let owner = s.owner_sign_in;
    let number = stop(&mut s, owner);
    let child = s.child;
    let proposal = party_propose(&mut s, sign_in, child, number, 80);
    party_decide(&mut s, owner, person, proposal, people::ProposalDecision::Accept, 81);
    assert_eq!(live_row(&s.world, child).placed, None);
    assert_eq!(live_row(&s.world, s.goal).placed, Some(number));
    let hold = placed_hold(&s, number);
    assert!(!hold.lifted);
    assert_eq!(hold.exempt.as_ref(), &[child]);
    assert_eq!(
        party_proposal(&s, proposal).state,
        tasks::PersonProposalState::Accepted { by: tasks::Party::Person(s.owner) }
    );
}

#[test]
fn a_party_release_proposal_is_withdrawn_when_the_hold_is_lifted_or_moves() {
    for with_tasks in [false, true] {
        let mut s = story(1013);
        let (sign_in, _) = maintainer(&mut s);
        let owner = s.owner_sign_in;
        let number = stop(&mut s, owner);
        let goal = s.goal;
        let proposal = party_propose(&mut s, sign_in, goal, number, 80);
        let task_proposal = propose(&mut s, number);
        ask_as(&mut s.world, owner, 81, people::Ask::Lift { hold: number, revision: 1, with_tasks });
        assert!(matches!(party_proposal(&s, proposal).state, tasks::PersonProposalState::Withdrawn { .. }));
        assert!(s.world.store.rows.values().any(|row| matches!(row,
            root::Record::Core(core::Record::Tasks(tasks::Stored::History(history))) if history.proposal.as_ref().is_some_and(|row| row.number == task_proposal && row.state == tasks::ProposalState::Withdrawn))));
        if !with_tasks {
            assert_eq!(placed_hold(&s, number).revision, 2)
        }
    }
}

#[test]
fn a_party_release_proposal_to_a_task_owned_hold_waits_among_its_entries_and_the_task_accepts_it() {
    let mut s = story(1014);
    let (sign_in, person) = maintainer(&mut s);
    let number = message_number(&s).max(1);
    s.world.send(root::Event::Core(core::Event::Tasks(tasks::Event::Place {
        reply_to: ReplyTo::new(Token::new(10900)),
        hold: tasks::PlacedHold {
            number,
            project: 1,
            owner: tasks::Owner::Task { task: s.coordinator.0 },
            mode: tasks::Mode::Stop,
            scope: tasks::Scope::Subtree { task: s.goal },
            reason: b"coordinator's hold".as_slice().into(),
            placed_at: 1,
            revision: 1,
            exempt: Box::new([]),
            lifted: false,
        },
    })));
    parked(&mut s);
    let goal = s.goal;
    let proposal = party_propose(&mut s, sign_in, goal, number, 80);
    // A new run reads the virtual proposal entry as a real holder entry.
    let coordinator = s.coordinator;
    s.world.send(root::Event::Answer {
        read: None,
        refused: Box::new([]),
        channel: Token::new(7),
        task: coordinator.0,
        attempt: coordinator.1,
        cumulative: 0,
        end: tasks::End::Parked,
    });
    let run = s.world.assignments.iter().rev().find(|run| run.0 == coordinator.0).copied().expect("holder woken");
    s.coordinator = run;
    assert!(s.world.assigned.as_ref().expect("holder assignment").messages.iter().any(|word|
        word.from == tasks::Party::Person(person) && matches!(word.kind, tasks::MessageKind::Proposal { proposer, proposal: named, kind: tasks::ProposalKind::Release } if proposer == person && named == proposal)));
    action(
        &mut s,
        20,
        core::NamedAction::DecideProposal { proposer: person, proposal, choice: core::ProposalChoice::Accept },
    );
    assert_eq!(
        party_proposal(&s, proposal).state,
        tasks::PersonProposalState::Accepted { by: tasks::Party::Task(coordinator.0) }
    );
    resumed(&s, number);
}

#[test]
fn a_party_who_may_lift_the_hold_is_refused_a_proposal_and_releases_instead() {
    let mut s = story(1015);
    let owner = s.owner_sign_in;
    let number = stop(&mut s, owner);
    let goal = s.goal;
    ask_as(
        &mut s.world,
        owner,
        80,
        people::Ask::Propose {
            project: 1,
            action: people::Action::ReleasePlaced { task: goal, hold: number, revision: 1 },
            reason: b"resume".as_slice().into(),
        },
    );
    refusal(&s.world, people::Refusal::NotOffered);
    release(&mut s, owner, 81);
    resumed(&s, number);
}

#[test]
fn a_rejected_party_release_proposal_leaves_the_task_held() {
    let mut s = story(1016);
    let (sign_in, person) = maintainer(&mut s);
    let owner = s.owner_sign_in;
    let number = stop(&mut s, owner);
    let goal = s.goal;
    let proposal = party_propose(&mut s, sign_in, goal, number, 80);
    let before = live_row(&s.world, goal).clone();
    let reason = b"keep inspecting".as_slice();
    party_decide(&mut s, owner, person, proposal, people::ProposalDecision::Reject { reason: reason.into() }, 81);
    assert_eq!(live_row(&s.world, goal), &before);
    assert!(
        matches!(party_proposal(&s, proposal).state, tasks::PersonProposalState::Rejected { by: tasks::Party::Person(by), reason: words, .. } if by == s.owner && words.as_ref() == reason)
    );
}

#[test]
fn a_pending_party_release_proposal_survives_a_cold_restart() {
    let mut s = story(1017);
    let (sign_in, person) = maintainer(&mut s);
    let owner = s.owner_sign_in;
    let number = stop(&mut s, owner);
    let goal = s.goal;
    release(&mut s, sign_in, 79);
    refusal(&s.world, owner_refusal(number, s.owner));
    let proposal = party_propose(&mut s, sign_in, goal, number, 80);
    let before = party_proposal(&s, proposal);
    s.world.restart_with(configuration(s.seed).0);
    assert_eq!(party_proposal(&s, proposal), before);
    release(&mut s, sign_in, 79);
    refusal(&s.world, owner_refusal(number, s.owner));
    // Replaying the retained ask reads back both its action and number-only successful outcome.
    assert_eq!(party_propose(&mut s, sign_in, goal, number, 80), proposal);
    s.world.reads = Some(jig_core_world::reads::Parties::new(s.world.store.applied));
    assert!(inbox_objects(&mut s.world, owner, core::reads::Group::Proposals).contains(
        &core::documents::InboxObject::Proposal { proposer: people::Proposer::Party(person), number: proposal }
    ));
    party_decide(&mut s, owner, person, proposal, people::ProposalDecision::Accept, 81);
    resumed(&s, number);
    assert_eq!(
        party_proposal(&s, proposal).state,
        tasks::PersonProposalState::Accepted { by: tasks::Party::Person(s.owner) }
    );
}

#[test]
fn a_refused_party_release_acceptance_changes_neither_the_hold_nor_the_proposal() {
    let seed = 1018;
    let (config, mut limits) = configuration(seed);
    limits.core.tasks.exemptions = 1;
    jig_core_world::peers::journal(&mut limits);
    let mut s = configured_story(seed, config, &limits);
    let (sign_in, person) = maintainer(&mut s);
    let owner = s.owner_sign_in;
    let number =
        placed_number(&hold(&mut s, owner, 80, people::HoldMode::Stop, people::HoldScope::Project, b"pause project"));
    parked(&mut s);
    let child = s.child;
    let revision = live_row(&s.world, child).revisions.hold;
    ask_as(&mut s.world, owner, 81, people::Ask::Release { project: 1, task: child, revision, again: None });
    let goal = s.goal;
    let proposal = party_propose(&mut s, sign_in, goal, number, 82);
    let before = (placed_hold(&s, number), live_row(&s.world, goal).clone(), party_proposal(&s, proposal));
    party_decide(&mut s, owner, person, proposal, people::ProposalDecision::Accept, 83);
    assert_eq!((placed_hold(&s, number), live_row(&s.world, goal).clone(), party_proposal(&s, proposal)), before);
    assert!(matches!(s.world.people_answers.last(), Some(people::Reply::Again(people::Again::Busy))));
}

#[test]
fn party_release_proposals_for_one_task_keep_both_inbox_entries_and_acceptance_withdraws_the_other() {
    let mut s = story(1019);
    let (sign_in, first) = maintainer(&mut s);
    let second_sign_in = second_party(&mut s.world, b"another maintainer", people::Role::Maintainer);
    let Some(people::Reply::SignedIn { person: second, .. }) = s.world.people_answers.last() else {
        panic!("second maintainer")
    };
    let second = *second;
    let owner = s.owner_sign_in;
    let number = stop(&mut s, owner);
    let goal = s.goal;
    let a = party_propose(&mut s, sign_in, goal, number, 80);
    let b = party_propose(&mut s, second_sign_in, goal, number, 81);
    s.world.reads = Some(jig_core_world::reads::Parties::new(s.world.store.applied));
    let inbox = inbox_objects(&mut s.world, owner, core::reads::Group::Proposals);
    for (person, number) in [(first, a), (second, b)] {
        assert!(
            inbox.contains(&core::documents::InboxObject::Proposal {
                proposer: people::Proposer::Party(person),
                number
            })
        );
    }
    party_decide(&mut s, owner, first, a, people::ProposalDecision::Accept, 82);
    assert!(matches!(party_proposal(&s, b).state, tasks::PersonProposalState::Withdrawn { .. }));
    resumed(&s, number);
}

#[test]
fn party_release_admission_checks_the_reason_standing_hold_and_revision() {
    let mut s = story(1021);
    let (sign_in, _) = maintainer(&mut s);
    let member_sign_in = second_party(&mut s.world, b"outside member", people::Role::Member);
    let owner = s.owner_sign_in;
    let number = stop(&mut s, owner);
    let goal = s.goal;
    let bound = configuration(s.seed).1.core.people.words;
    for (key, reason, which) in
        [(80, Vec::new(), people::LimitKind::Empty), (81, vec![b'x'; bound as usize + 1], people::LimitKind::Words)]
    {
        ask_as(
            &mut s.world,
            sign_in,
            key,
            people::Ask::Propose {
                project: 1,
                action: people::Action::ReleasePlaced { task: goal, hold: number, revision: 1 },
                reason: reason.into(),
            },
        );
        refusal(&s.world, people::Refusal::Limit { which });
    }
    ask_as(
        &mut s.world,
        member_sign_in,
        82,
        people::Ask::Propose {
            project: 1,
            action: people::Action::ReleasePlaced { task: goal, hold: number, revision: 1 },
            reason: b"outside my tree".as_slice().into(),
        },
    );
    refusal(&s.world, people::Refusal::Unknown);
    ask_as(
        &mut s.world,
        sign_in,
        83,
        people::Ask::Propose {
            project: 1,
            action: people::Action::ReleasePlaced { task: goal, hold: number, revision: 0 },
            reason: b"old revision".as_slice().into(),
        },
    );
    refusal(&s.world, people::Refusal::Moved { now: people::Moved::PlacedHold { revision: 1 } });
    ask_as(
        &mut s.world,
        sign_in,
        84,
        people::Ask::Propose {
            project: 1,
            action: people::Action::ReleasePlaced { task: goal, hold: number + 1, revision: 1 },
            reason: b"different hold".as_slice().into(),
        },
    );
    refusal(&s.world, people::Refusal::NotWaiting);
    let reason = vec![b'x'; bound as usize];
    ask_as(
        &mut s.world,
        sign_in,
        85,
        people::Ask::Propose {
            project: 1,
            action: people::Action::ReleasePlaced { task: goal, hold: number, revision: 1 },
            reason: reason.clone().into(),
        },
    );
    let Some(people::Reply::Outcome(people::Outcome::ReleaseProposed { proposal })) = s.world.people_answers.last()
    else {
        panic!("at-bound reason admitted")
    };
    assert_eq!(party_proposal(&s, *proposal).reason.as_ref(), reason.as_slice());
}
