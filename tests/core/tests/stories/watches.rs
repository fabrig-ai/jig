use jig_core_world::{
    effects::Cut,
    watches::{latest, open, world},
};
use jig_domain_core::{
    self as core,
    documents::{Count, Cursor, Document, PhaseCounts, PhaseLine, Watched, Window},
};
use jig_domain_tasks as tasks;
use jig_domain_views as views;
use jig_test_domain as root;
use skein_lib::Token;

#[test]
fn a_party_watching_a_chat_sees_each_phase_after_its_commit() {
    let mut world = world(701, true);
    let Document::Task(task) = latest(&world, 2) else { panic!("task page") };
    assert!(matches!(task.chip.phase, PhaseLine::Active(tasks::Active::Running { .. })));
    world.store_delay = 4;
    world.end(tasks::End::Parked, 0);
    let Document::Task(task) = latest(&world, 2) else { panic!("idle page") };
    assert_eq!(task.chip.phase, PhaseLine::Active(tasks::Active::Idle));
    world.say(70);
    world.retry_due();
    let Document::Task(task) = latest(&world, 2) else { panic!("running page") };
    assert!(matches!(task.chip.phase, PhaseLine::Active(tasks::Active::Running { .. })));
    world.watches.as_ref().expect("watches").finish();
}

#[test]
fn a_slow_watcher_of_a_tree_sees_fewer_deliveries_and_the_last_state() {
    let mut world = world(702, true);
    world.watches.as_mut().expect("watches").manual(3);
    world.turn(1, 1, None, b"first");
    // This first replacement reaches the party, which leaves it in flight.
    for turn in 2..=4 {
        world.turn(turn, u64::from(turn), None, b"later");
    }
    world.end(tasks::End::Parked, 4);
    world.send(root::Event::Core(core::Event::Delivered { watcher: Token::new(3) }));
    let watches = world.watches.as_ref().expect("watches");
    let slow = watches.deliveries.iter().filter(|(watcher, _, _)| *watcher == 3).count();
    let fast = watches.deliveries.iter().filter(|(watcher, _, _)| *watcher == 2).count();
    assert!(slow < fast, "one replacement coalesces intervening commits: slow={slow} fast={fast}");
    let Document::Tree(tree) = latest(&world, 3) else { panic!("tree page") };
    assert_eq!(tree.root.phase, PhaseLine::Active(tasks::Active::Idle));
    watches.finish();
    inbox_across_projects_stories();
}

#[test]
fn a_tree_window_past_the_rows_is_clamped_and_cut() {
    let seed = 706;
    let (configuration, mut limits) = jig_core_world::faults::plan_fixture_with_document_rows(seed, 2);
    limits.core.tasks.delegates = 4;
    jig_core_world::peers::journal(&mut limits);
    let mut world = jig_core_world::effects::World::configured(seed, false, configuration, limits);
    let parent = world.assignments[0];
    let mut delegates = Vec::new();
    for completion in 1..=4 {
        let mut child =
            jig_core_world::fixtures::member(&world, tasks::Executor::Agent { charter: 1 }, b"live delegate");
        child.authority.delegation = tasks::Delegation { kinds: Box::new([]), tasks: 0, depth: 0 };
        delegates.push(jig_core_world::fixtures::delegate(&mut world, parent, completion, child));
    }
    world.watches =
        Some(jig_core_world::watches::Parties::new(seed, world.store.applied, true, world.sign_in.expect("signed in")));
    world.send(root::Event::Core(core::Event::Watch {
        watcher: Token::new(3),
        sign_in: world.sign_in.expect("signed in"),
        subject: Watched::Tree { task: parent.0 },
        window: Window { rows: 32, after: None },
    }));
    let watches = world.watches.as_ref().expect("party's tree watch");
    assert_eq!(watches.deliveries.len(), 1, "first delivery without reopening the watch");
    let Document::Tree(tree) = &watches.deliveries[0].2 else { panic!("tree document") };
    assert_eq!(tree.root.number, parent.0);
    assert_eq!(tree.rows.len(), 2);
    assert_eq!(tree.rows.get(0).expect("first row").chip.number, delegates[0]);
    assert_eq!(tree.rows.get(1).expect("second row").chip.number, delegates[1]);
    assert_eq!(tree.next, Some(Cursor::Number { number: delegates[1] }));
    assert_eq!(tree.folded.len(), 1);
    let folded = tree.folded.get(0).expect("omitted delegates folded at root");
    assert_eq!(folded.parent, parent.0);
    assert_eq!(folded.phases, PhaseCounts { starting: 2, ..PhaseCounts::ZERO });
    assert_eq!(folded.held_below, 0);
    assert!(watches.endings.is_empty());
    assert!(world.busy_events.is_empty(), "no Busy or retry");
    watches.finish();
}

#[test]
fn a_delivery_never_shows_what_a_crash_took_back() {
    let mut world = world(703, true);
    let task = world.assigned.as_ref().expect("assignment").task;
    open(&mut world, 5, Watched::Run { task });
    let before = world.watches.as_ref().expect("watches").deliveries.len();
    let next = world.store.applied + 1;
    world.cut = Some(Cut::Submitted(next));
    world.turn(1, 1, None, b"uncommitted turn");
    assert_eq!(world.reached_cut, Some(Cut::Submitted(next)));
    assert_eq!(world.watches.as_ref().expect("watches").deliveries.len(), before);
    world.restart(703, false);
    open(&mut world, 6, Watched::Run { task });
    let Document::Run(run) = latest(&world, 6) else { panic!("reopened run") };
    assert_eq!(run.committed_turn, 0);
    assert!(world.store.rows.values().all(|row| !matches!(row, root::Record::Core(core::Record::Core(core::CoreRecord::Turn(turn))) if turn.task == task && turn.turn == 1)));
    world.watches.as_ref().expect("watches").finish();
}

#[test]
fn a_run_s_text_streams_by_turn_and_is_replaced_by_the_committed_turn() {
    let mut world = world(704, true);
    let assignment = world.assigned.as_ref().expect("assignment");
    let (task, attempt) = (assignment.task, assignment.attempt);
    open(&mut world, 5, Watched::Run { task });
    let committed = world.store.applied;
    for (offset, text) in [(0, b"hello ".as_slice()), (6, b"world".as_slice())] {
        world.send(root::Event::Text { task, attempt, turn: 1, offset, text: text.into() });
        world.send(root::Event::Core(core::Event::StreamDelivered { watcher: Token::new(5) }));
    }
    assert_eq!(world.store.applied, committed, "provisional text writes nothing");
    let streams = &world.watches.as_ref().expect("watches").streams;
    assert_eq!(streams.len(), 2);
    assert_eq!(
        (streams[0].turn, streams[0].from, streams[0].to),
        (views::TurnName { attempt, turn: 1, retry: 0 }, 0, 6)
    );
    assert_eq!((streams[1].from, streams[1].to, streams[1].text.as_ref()), (6, 11, b"world".as_slice()));
    world.store_delay = 3;
    world.turn(1, 1, None, b"whole committed turn");
    let Document::Run(run) = latest(&world, 5) else { panic!("committed run") };
    assert_eq!(run.committed_turn, 1);
    assert!(run.writing.is_none());
    world.watches.as_ref().expect("watches").finish();
}

#[test]
fn a_watch_on_a_task_that_ends_ends_saying_so() {
    let mut world = world(705, true);
    world.store_delay = 3;
    world.end(
        tasks::End::Finished {
            result: tasks::TaskResult::Report { words: b"done".as_slice().into() },
            cancel_delegates: false,
        },
        0,
    );
    let watches = world.watches.as_ref().expect("watches");
    assert!(watches.endings.contains(&(2, views::End::Gone)));
    assert!(watches.endings.contains(&(3, views::End::Gone)));
    let Document::Frame(frame) = latest(&world, 1) else { panic!("updated frame") };
    assert_eq!(frame.updates, Count::Known(1));
    let Document::Inbox(inbox) = latest(&world, 4) else { panic!("updated inbox") };
    assert_eq!(inbox.cached_updates, Count::Known(1));
    assert_eq!(inbox.updates.len(), 1);
    watches.finish();
}

fn inbox_across_projects_stories() {
    use jig_core_world::watches::Parties;
    use jig_domain_authority as authority;
    use jig_domain_people as people;
    let (mut config, mut limits) = super::amendment_world_fixture(788);
    limits.core.tasks.tasks = 4;
    // Both configured projects and the deployment's roles.
    limits.core.people.projects = 3;
    limits.core.people.initial_owners = 2;
    limits.core.authority.projects = 2;
    let policy = config.core.seed_projects[0].policy.policy.clone();
    let rules = config.core.defaults.rules.clone();
    config.core.defaults.rules =
        (authority::Domain::new(rules, limits.core.authority).expect("two policies")).rules().clone();
    let first = config.core.owners[0].clone();
    config.core.owners = Box::new([first.clone(), people::InitialOwner { project: 2, identity: first.identity }]);
    config.core.seed_projects = [1, 2]
        .into_iter()
        .map(|number| core::SeedProject {
            number,
            name: format!("Project {number}").into_boxed_str(),
            policy: core::ConfiguredPolicy {
                policy: policy.clone(),
                permissions: config.core.seed_projects[0].policy.permissions.clone(),
                templates: Box::new([]),
            },
        })
        .collect();
    limits.core.people.amendment_bytes = 256;
    limits.core.people.inbox_recipients = 16;
    jig_core_world::peers::journal(&mut limits);
    let mut world = jig_core_world::effects::World::configured(788, false, config, limits);
    world.watches = Some(Parties::new(788, world.store.applied, true, world.sign_in.expect("signed in")));
    open(&mut world, 1, Watched::Frame);
    open(&mut world, 4, Watched::Inbox);
    let first_sign_in = world.sign_in.expect("first party");
    world.send(root::Event::Core(core::Event::SignIn {
        held: None,
        reply_to: skein_lib::ReplyTo::new(Token::new(6090)),
        identity: people::Identity {
            key: people::IdentityKey { provider: 0, subject: 8_u64.to_be_bytes().into() },
            login: b"other".as_slice().into(),
            name: b"Other".as_slice().into(),
        },
    }));
    let second_sign_in = world.sign_in.expect("second party");
    world.sign_in = Some(first_sign_in);
    super::ask(
        &mut world,
        94,
        people::Ask::SetRoles {
            project: 2,
            holdings: Box::new([
                people::Holding { person: 1, role: people::Role::Owner },
                people::Holding { person: 2, role: people::Role::Owner },
            ]),
        },
    );
    let mut proposals = Vec::new();
    for project in [1, 2] {
        let proposed_goal = jig_core_world::fixtures::goal(
            &world,
            project,
            b"shared goal".as_slice().into(),
            people::TaskExecutor::Agent { charter: 1 },
            jig_core_world::fixtures::Goal { budget: 600, ..Default::default() },
        );
        super::ask_as(
            &mut world,
            if project == 1 { first_sign_in } else { second_sign_in },
            u8::try_from(90 + project).expect("request"),
            jig_core_world::fixtures::propose_goal(proposed_goal),
        );
        let Some(people::Reply::Outcome(people::Outcome::Proposed { proposal, to: 0 })) = world.people_answers.last()
        else {
            panic!(
                "goal proposal: {:?}; {:?}",
                world.people_answers,
                &world.trace[world.trace.len().saturating_sub(30)..]
            );
        };
        proposals.push(*proposal);
    }
    a_party_with_roles_in_two_projects_sees_both_in_one_inbox(&world);
    super::ask(
        &mut world,
        93,
        people::Ask::DecideProposal {
            project: 1,
            proposer: people::Proposer::Party(1),
            proposal: proposals[0],
            decision: people::ProposalDecision::Reject { reason: b"later".as_slice().into() },
            running: None,
        },
    );
    a_proposal_decided_in_one_project_leaves_the_inbox_across_projects(&world, proposals[1]);
    world.watches.as_ref().expect("parties").finish();
}

fn a_party_with_roles_in_two_projects_sees_both_in_one_inbox(world: &jig_core_world::effects::World) {
    let Document::Inbox(inbox) = latest(world, 4) else { panic!("inbox") };
    assert_eq!(inbox.cached_needs, 2);
    assert_eq!(inbox.needs.len(), 2);
    assert_eq!(inbox.needs.get(0).expect("first").project, Some(1));
    assert_eq!(inbox.needs.get(1).expect("second").project, Some(2));
    let Document::Frame(frame) = latest(world, 1) else { panic!("frame") };
    assert_eq!(frame.needs, 2);
}

fn a_proposal_decided_in_one_project_leaves_the_inbox_across_projects(
    world: &jig_core_world::effects::World,
    other: u64,
) {
    let Document::Inbox(inbox) = latest(world, 4) else { panic!("inbox") };
    assert_eq!(inbox.cached_needs, 1);
    assert_eq!(inbox.needs.len(), 1);
    assert_eq!(inbox.needs.get(0).expect("other proposal").project, Some(2));
    assert_eq!(
        inbox.needs.get(0).expect("other proposal").object,
        core::documents::InboxObject::Proposal { proposer: core::documents::Proposer::Party(2), number: other }
    );
    let Document::Frame(frame) = latest(world, 1) else { panic!("frame") };
    assert_eq!(frame.needs, 1);
}
