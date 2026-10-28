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
fn a_drain_is_refused_before_anything_is_placed() {
    let mut s = story(974);
    let sign_in = s.owner_sign_in;
    let messages = message_number(&s);
    for (key, scope) in [(71, people::HoldScope::Project), (72, people::HoldScope::Subtree { task: s.goal })] {
        assert_eq!(
            hold(&mut s, sign_in, key, people::HoldMode::Drain, scope, b"drain later"),
            people::Outcome::Refused(people::Refusal::Authority { proposable: None, lacking: people::Lack::Shape })
        );
    }
    assert_eq!(holds_count(&s), 0);
    assert_eq!(message_number(&s), messages);
    assert_eq!(live_row(&s.world, s.goal).placed, None);
    assert_eq!(live_row(&s.world, s.child).placed, None);
    assert!(s.world.cancellations.is_empty());
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
    let second = placed_number(&hold(&mut s, sign_in, 72, people::HoldMode::Stop, people::HoldScope::Project, b"inspect project"));
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
    s.world.send(root::Event::Core(core::Event::People(people::Event::Roles { project: 1, holdings: Box::new([
        people::Holding { person: s.owner, role: people::Role::Owner },
        people::Holding { person: 2, role: people::Role::Observer },
    ]) })));
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
        numbers.push(placed_number(&hold(&mut s, sign_in, key, people::HoldMode::Stop, people::HoldScope::Project, b"paged project hold")));
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
        if from.is_none() { break; }
    }
    assert_eq!(seen, numbers);
    let wrong = core::reads::Cursor::Live { scope: core::reads::Scope::Holds { project: 2 }, after: core::documents::Cursor::Number { number: 1 }, store_after: None };
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
