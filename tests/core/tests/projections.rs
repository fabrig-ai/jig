use jig_core_world::effects::{World, fixture};
use jig_core_world::fixtures::finish_chat;
use jig_domain_authority as authority;
use jig_domain_core as core;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_test_connector as connector;
use jig_test_domain as root;
use skein_lib::{ReplyTo, Token};

fn projections_use_project_grants_require_verdicts_and_observe_a_policy_narrowing_after_restart() {
    let (mut configuration, limits) = fixture(170, true);
    let mut policy = configuration.core.seed_projects[0].policy.policy.clone();
    policy.projections.clone_from(&policy.ceiling.grants);
    policy.roles[0].authority.grants = Box::new([]);

    configuration.core.seed_projects[0].policy.policy = policy;
    let mut world = World::configured(170, true, configuration, limits);
    let goal = tracked_goal(&mut world);
    let record = world
        .store
        .rows
        .values()
        .find_map(|row| match row {
            root::Record::Core(core::Record::Tasks(tasks::Stored::Live(row))) if row.number == goal => Some(row),
            root::Record::Core(_) | root::Record::Connector { .. } => None,
        })
        .expect("durable goal");
    assert!(record.authority.grants.is_empty());
    assert_eq!(record.numbers.spent, 0);
    for number in 40..48 {
        world.send(root::Event::Core(core::Event::People(people::Event::Ask {
            models: Box::new([]),
            reply_to: ReplyTo::new(Token::new(9100 + u64::from(number))),
            sign_in: world.sign_in.expect("signed in party"),
            key: [number; 16],
            ask: people::Ask::Say { project: 1, task: goal, words: Box::from(&b"waiting words"[..]) },
        })));
    }
    world.send(root::Event::ProjectionEffect { goal, number: 1, effect: World::effect() });
    assert!(world.systems[0].observed().is_empty(), "a missing verdict keeps no projection effect");
    world.observed(13);
    world.send(root::Event::ProjectionEffect { goal, number: 1, effect: World::effect() });
    assert_eq!(world.systems[0].observed().iter().filter(|effect| effect.applied).count(), 1);
    let saved = world
        .store
        .rows
        .values()
        .find_map(|row| match row {
            root::Record::Core(core::Record::Tasks(tasks::Stored::Live(row))) if row.number == goal => Some(row),
            root::Record::Core(_) | root::Record::Connector { .. } => None,
        })
        .expect("durable goal");
    assert_eq!(saved.numbers.spent, 0, "the priced projection charges no task budget");
    world.send(root::Event::Core(core::Event::People(people::Event::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(Token::new(9001)),
        sign_in: world.sign_in.expect("signed in party"),
        key: [32; 16],
        ask: people::Ask::ChangePolicy {
            project: 1,
            change: people::PolicyChange::Projections { grants: Box::new([]) },
        },
    })));
    assert!(
        world
            .people_answers
            .iter()
            .any(|reply| matches!(reply, people::Reply::Outcome(people::Outcome::PolicyChanged { project: 1 })))
    );
    world.restart(170, true);
    let mut next = World::effect();
    next.purpose = 20;
    next.target = 14;
    world.send(root::Event::ProjectionEffect { goal, number: 1, effect: next });
    assert_eq!(
        world.systems[0].observed().iter().filter(|effect| effect.applied).count(),
        1,
        "a narrowed policy refuses the next projection"
    );
    assert!(
        world.store.rows.values().any(|row| matches!(row,
            root::Record::Core(core::Record::Tasks(tasks::Stored::Live(row))) if row.number == goal
                && row.inbox.iter().any(|word| word.words.as_ref() == b"Projection write refused")
        )),
        "the goal hears the refusal as durable news"
    );
}

fn delegate_agent(world: &mut World, words: &[u8], spend: u64, tasks_left: u32, depth: u32) -> (u64, u64) {
    let mut authority = world.assigned.as_ref().expect("assigned parent").run.authority.clone();
    authority.grants = Box::new([]);
    authority.budget.spend = spend;
    authority.delegation = tasks::Delegation {
        kinds: if tasks_left == 0 { Box::new([]) } else { Box::new([tasks::AuthorityExecutor::Charter(1)]) },
        tasks: tasks_left,
        depth,
    };
    world.send(root::Event::Core(core::Event::DelegateValidated {
        to: ReplyTo::new(Token::new(9200 + spend)),
        key: world.call_key(1),
        batch: Box::new([core::Delegate {
            deadline: jig_domain_tasks::Deadline::Named,
            label: 0,
            flags: jig_domain_tasks::Flags::PLAIN,
            tracked: None,
            budget: jig_domain_people::Budget::Named,
            executor: tasks::Executor::Agent { charter: 1 },
            spec: tasks::Spec { words: Box::from(words), parameters: Box::new([]), inputs: Box::new([]) },
            contract: tasks::Contract::Report { words: 128 },
            authority,
            llm: None,
            symbolic_grants: Box::new([]),
            dependencies: Box::new([]),
            wake: tasks::WakePolicy::DEFAULT,
        }]),
        stubs: Box::new([]),
    }));
    let assigned = world.assigned.as_ref().expect("assigned delegate");
    (assigned.task, assigned.attempt)
}

fn projection_rows(world: &World, goal: u64) -> usize {
    world
        .store
        .rows
        .keys()
        .filter(|key| {
            matches!(key,
                root::Key::Core(core::Key::Core(core::CoreKey::Projection(
                    core::ProjectionKey::Goal(number)
                    | core::ProjectionKey::Task { goal: number, .. }
                    | core::ProjectionKey::Lifecycle { goal: number, .. }
                    | core::ProjectionKey::Revision { goal: number, .. }
                ))) if *number == goal
            )
        })
        .count()
}

fn tree_world() -> (World, root::Config) {
    let (mut configuration, mut limits) = fixture(171, false);
    limits.core.tasks.tasks = 4;
    limits.core.tasks.project_tasks = 4;
    limits.core.tasks.tree_tasks = 3;
    limits.core.tasks.depth = 2;
    limits.core.fleet.slots = 3;
    limits.core.fleet.attempts = 4;
    limits.core.fleet.calls = 3;
    limits.core.call_records = 4;
    limits.core.views.runs = 4;
    limits.core.brief.briefs = 4;
    limits.routes = 500;
    limits.journal.commits = 8;
    jig_core_world::peers::journal(&mut limits);
    let mut policy = configuration.core.seed_projects[0].policy.policy.clone();
    policy.projections.clone_from(&policy.ceiling.grants);
    policy.ceiling.delegation.depth = 3;
    policy.ceiling.delegation.tasks = 4;
    policy.roles[0].authority.delegation.depth = 3;
    policy.roles[0].authority.delegation.tasks = 4;
    let mut rules = configuration.core.defaults.rules.clone();
    rules.ceiling.delegation.depth = 3;
    rules.ceiling.delegation.tasks = 4;
    configuration.core.defaults.rules =
        (authority::Domain::new(rules, limits.core.authority).expect("tree policy")).rules().clone();

    configuration.core.seed_projects[0].policy.policy = policy;
    jig_core_world::world::edit_role(&mut configuration.core, |role| {
        role.authority.delegation =
            authority::Delegation { kinds: Box::new([authority::Executor::Charter(1)]), tasks: 3, depth: 3 };
    });
    let mut restart_configuration = fixture(171, false).0;
    restart_configuration.core.defaults.rules = configuration.core.defaults.rules.clone();
    restart_configuration.core.seed_projects[0].policy.policy =
        configuration.core.seed_projects[0].policy.policy.clone();
    (World::configured(171, false, configuration, limits), restart_configuration)
}

fn a_delegate_of_a_delegate_ending_feeds_the_whole_plan_once_and_closing_waits_for_its_write() {
    let (mut world, restart_configuration) = tree_world();
    world.end(
        tasks::End::Finished {
            result: tasks::TaskResult::Report { words: Box::from(&b"chat done"[..]) },
            cancel_delegates: false,
        },
        0,
    );
    world.send(root::Event::Core(core::Event::People(people::Event::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(Token::new(9200)),
        sign_in: world.sign_in.expect("signed in party"),
        key: [33; 16],
        ask: jig_core_world::fixtures::goal(
            &world,
            1,
            Box::from(&b"Whole tree"[..]),
            people::TaskExecutor::Agent { charter: 1 },
            jig_core_world::fixtures::Goal { budget: 300, priority: 1, ..Default::default() },
        ),
    })));
    let goal = {
        let row = world.assigned.as_ref().expect("goal assigned");
        (row.task, row.attempt)
    };
    let child = delegate_agent(&mut world, b"Delegate", 150, 1, 1);
    assert_ne!(child.0, goal.0, "child admitted: {:?}", world.answers);
    let grandchild = delegate_agent(&mut world, b"Delegate's delegate", 20, 0, 0);
    assert_ne!(grandchild.0, child.0, "grandchild admitted: {:?}", world.answers);
    world.projections.clear();
    world.end(
        tasks::End::Finished {
            result: tasks::TaskResult::Report { words: Box::from(&b"leaf done"[..]) },
            cancel_delegates: false,
        },
        0,
    );
    let terminal = world
        .projections
        .iter()
        .filter(|(_, _, feed)| {
            feed.plan.iter().any(|task| task.number == grandchild.0 && matches!(task.phase, tasks::Phase::Ended(_)))
        })
        .collect::<Vec<_>>();
    assert_eq!(terminal.len(), 2, "one terminal feed to each connector");
    assert_eq!(terminal[0].0, terminal[1].0, "one decision");
    for (_, _, feed) in terminal {
        assert_eq!(feed.plan.len(), 3, "goal, delegate, and ended grandchild");
        let ended = feed
            .milestones
            .iter()
            .filter(|milestone| {
                matches!(milestone.identity, core::MilestoneId::Lifecycle { task, .. } if task == grandchild.0)
                    && matches!(milestone.phase, Some(tasks::Phase::Ended(_)))
            })
            .collect::<Vec<_>>();
        assert_eq!(ended.len(), 1, "one immutable terminal milestone");
        assert!(world.store.rows.values().any(|row| matches!(row, root::Record::Core(core::Record::Tasks(tasks::Stored::Milestone(row))) if matches!(ended[0].identity, core::MilestoneId::Lifecycle { task, position } if task == row.task && position == row.position))));
    }
    settle_tree(&mut world, goal, child, restart_configuration);
}

fn settle_tree(world: &mut World, goal: (u64, u64), child: (u64, u64), restart_configuration: root::Config) {
    let read = world
        .store
        .rows
        .values()
        .find_map(|row| match row {
            root::Record::Core(core::Record::Tasks(tasks::Stored::Live(row))) if row.number == child.0 => {
                Some(row.last_message)
            }
            root::Record::Core(_) | root::Record::Connector { .. } => None,
        })
        .expect("live parent");
    world.send(root::Event::Turn {
        opens_window: false,
        refused: Box::new([]),
        channel: Token::new(7),
        task: child.0,
        attempt: child.1,
        turn: 1,
        cumulative: 0,
        read: Some(read),
        transcript: Box::from(&b"Read delegate result"[..]),
    });
    world.send(root::Event::Answer {
        read: None,
        refused: Box::new([]),
        channel: Token::new(7),
        task: child.0,
        attempt: child.1,
        cumulative: 0,
        end: tasks::End::Finished {
            result: tasks::TaskResult::Report { words: Box::from(&b"delegate done"[..]) },
            cancel_delegates: false,
        },
    });
    world.hold_projection_settlements = true;
    world.projections.clear();
    world.send(root::Event::Answer {
        read: None,
        refused: Box::new([]),
        channel: Token::new(7),
        task: goal.0,
        attempt: goal.1,
        cumulative: 0,
        end: tasks::End::Finished {
            result: tasks::TaskResult::Report { words: Box::from(&b"goal done"[..]) },
            cancel_delegates: false,
        },
    });
    assert_eq!(world.projections.iter().filter(|(_, _, feed)| feed.closing).count(), 2);
    assert_eq!(projection_rows(world, goal.0), 4, "header and full ended plan retained");
    assert!(world.store.rows.contains_key(&root::Key::Core(core::Key::Tasks(tasks::Key::Ended(goal.0)))));
    assert!(!world.store.rows.contains_key(&root::Key::Core(core::Key::Tasks(tasks::Key::Live(goal.0)))));
    world.hold_writes = true;
    world.send(root::Event::ProjectionEffect { goal: goal.0, number: 1, effect: World::effect() });
    assert_eq!(world.pending_writes.len(), 1, "closing write admitted after the task ended");
    assert_eq!(projection_rows(world, goal.0), 4);
    let (number, call) = world.pending_writes.pop().expect("committed projection attempt");
    let entry = projection_outbox(world, goal.0).expect("persisted projection");
    let procedures = world.procedure_inputs.len();
    let answers = world.answers.len();
    world.restart_with(restart_configuration);
    assert_eq!(projection_rows(world, goal.0), 4, "closing retention survives restart");
    assert_eq!(projection_outbox(world, goal.0).expect("restored projection").number, entry.number);
    let event = world.systems[0].answer(call, jig_test_system::Fault::None);
    world.send(root::Event::Connector { number, event: connector::Event::System(event) });
    assert!(projection_outbox(world, goal.0).is_none(), "late terminal settles the restored entry");
    assert_eq!(world.procedure_inputs.len(), procedures, "a projection does not step a procedure");
    assert_eq!(world.answers.len(), answers, "a projection does not answer calls");
    assert!(!world.store.rows.contains_key(&root::Key::Core(core::Key::Tasks(tasks::Key::Live(goal.0)))));
    assert_eq!(world.systems[0].observed().iter().filter(|effect| effect.applied).count(), 1);
    world.send(root::Event::Core(core::Event::ProjectionSettled { goal: goal.0, connector: 1 }));
    assert_eq!(projection_rows(world, goal.0), 4, "other connector still owns its last feed");
    world.send(root::Event::Core(core::Event::ProjectionSettled { goal: goal.0, connector: 2 }));
    assert_eq!(projection_rows(world, goal.0), 0, "last settlement releases projection state");
}

// Each named scenario preserves its own fixture and original assertions.
#[test]
fn projections_scenario_family() {
    projections_use_project_grants_require_verdicts_and_observe_a_policy_narrowing_after_restart();
    a_delegate_of_a_delegate_ending_feeds_the_whole_plan_once_and_closing_waits_for_its_write();
}

fn untracked_configuration(seed: u64) -> (root::Config, root::Limits) {
    let (mut configuration, limits) = fixture(seed, false);
    let mut policy = configuration.core.seed_projects[0].policy.policy.clone();
    policy.projections.clone_from(&policy.ceiling.grants);
    configuration.core.seed_projects[0].policy.policy = policy;
    (configuration, limits)
}

fn untracked_world(seed: u64) -> (World, root::Config, u64) {
    let (configuration, limits) = untracked_configuration(seed);
    let restart = untracked_configuration(seed).0;
    let mut world = World::configured(seed, false, configuration, limits);
    finish_chat(&mut world, 0, b"");
    let tracked_goal_request = jig_core_world::fixtures::goal(
        &world,
        1,
        b"untracked goal".as_slice().into(),
        people::TaskExecutor::Agent { charter: 1 },
        jig_core_world::fixtures::Goal { budget: 20, priority: 1, ..Default::default() },
    );
    jig_core_world::stories::reads::request(&mut world, 91, tracked_goal_request);
    let goal = world.assigned.as_ref().expect("goal assigned").task;
    world.hold_projection_settlements = true;
    world.projections.clear();
    jig_core_world::stories::reads::request(
        &mut world,
        92,
        people::Ask::Amend {
            project: 1,
            task: goal,
            revision: 0,
            release: None,
            amendment: people::Amendment {
                tracked: Some(people::Tracking::Untracked),
                spec: Some(people::Spec {
                    words: b"last goal state".as_slice().into(),
                    parameters: Box::new([]),
                    inputs: Box::new([]),
                }),
                ..jig_core_world::fixtures::person_unchanged(b"untracked")
            },
        },
    );
    assert_eq!(
        world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::Amended { task: goal, released: false }))
    );
    (world, restart, goal)
}

#[test]
fn an_untracked_projection_restores_before_its_live_task() {
    const SEED: u64 = 916;
    let (world, configuration, goal) = untracked_world(SEED);
    let limits = untracked_configuration(SEED).1;
    let env = skein_lib::Env { now: skein_lib::Time::ZERO, wall: skein_lib::Wall::EPOCH, limits };
    let mut restored = root::Domain::new(configuration, &limits);
    root::step(&mut restored, &env, root::Event::RestartBegin { pending: false });
    let mut requests = skein_lib::Queue::with_capacity(limits.routes);
    root::release(&mut restored, &env, &mut requests);
    // A store can hand up the core's projection family before the task family.
    // This is the order used by the consuming application's combined range.
    let ranges = [
        core::RestoreRange::Deployment,
        core::RestoreRange::Settings,
        core::RestoreRange::Accounts,
        core::RestoreRange::People,
        core::RestoreRange::Core,
        core::RestoreRange::Tasks,
    ];
    for range in ranges {
        for (key, row) in &world.store.rows {
            if let root::Key::Core(key) = key
                && range.admits(key)
            {
                assert!(
                    root::restore_record(&mut restored, &env, row.clone()),
                    "committed row refused before its live task arrives: {key:?}"
                );
            }
        }
    }
    // Finish this same cold start, then read and settle its restored pending cohort.
    let mut world = world;
    world.domain = restored;
    world.send(root::Event::RestartDone(core::RestartStep::LoadCore));
    assert!(world.domain.ready());
    world.send(root::Event::Core(core::Event::ProjectionSettled { goal, connector: 1 }));
    world.send(root::Event::Core(core::Event::ProjectionSettled { goal, connector: 2 }));
    assert_eq!(projection_rows(&world, goal), 0);
    assert_eq!(jig_core_world::fixtures::live_row(&world, goal).tracked, None);
}

#[test]
fn an_untracked_goal_is_fed_its_state_once_as_its_last_feed() {
    let (mut world, configuration, goal) = untracked_world(172);
    assert_eq!(jig_core_world::fixtures::live_row(&world, goal).tracked, None);
    let feeds = world.projections.iter().map(|(_, connector, feed)| (*connector, feed.clone())).collect::<Vec<_>>();
    assert_eq!(feeds.len(), 2);
    for (connector, feed) in &feeds {
        assert!([1, 2].contains(connector));
        assert_eq!(feed.goal.number, goal);
        assert_eq!(feed.goal.words.as_ref(), b"last goal state");
        assert!(feed.closing);
    }
    assert_ne!(feeds[0].0, feeds[1].0);
    world.restart_with(configuration);
    jig_core_world::stories::reads::request(
        &mut world,
        93,
        people::Ask::Say { project: 1, task: goal, words: b"still live".as_slice().into() },
    );
    assert_eq!(world.projections.len(), 2, "later decisions and restart send no further feed");
}

#[test]
fn an_untracked_goals_projection_is_forgotten_once_its_write_settles() {
    let (mut world, configuration, goal) = untracked_world(173);
    assert_eq!(projection_rows(&world, goal), 2, "header and task retained");
    world.restart_with(configuration);
    world.hold_writes = true;
    world.send(root::Event::ProjectionEffect { goal, number: 1, effect: World::effect() });
    assert_eq!(world.pending_writes.len(), 1, "the closing projection still writes");
    assert_eq!(projection_rows(&world, goal), 2);
    let before = jig_core_world::fixtures::live_row(&world, goal).phase.clone();
    let entry = projection_outbox(&world, goal).expect("pending projection");
    world.restart_with(untracked_configuration(173).0);
    let procedures = world.procedure_inputs.len();
    world.send(root::Event::Connector {
        number: 1,
        event: connector::Event::System(connector::SystemEvent::Applied {
            entry: entry.number,
            attempt: entry.attempt.expect("sent attempt").number,
            result: connector::ApplyResult::Conflict,
        }),
    });
    assert!(projection_outbox(&world, goal).is_none());
    assert_eq!(
        jig_core_world::fixtures::live_row(&world, goal).phase,
        before,
        "failed projection never holds its goal"
    );
    assert_eq!(world.procedure_inputs.len(), procedures);
    world.send(root::Event::Core(core::Event::ProjectionSettled { goal, connector: 1 }));
    assert_eq!(projection_rows(&world, goal), 2, "second connector still owns its feed");
    world.send(root::Event::Core(core::Event::ProjectionSettled { goal, connector: 2 }));
    assert_eq!(projection_rows(&world, goal), 0);
    assert_eq!(jig_core_world::fixtures::live_row(&world, goal).tracked, None, "the task stays live");
}

fn tracked_goal(world: &mut World) -> u64 {
    world.send(root::Event::Core(core::Event::People(people::Event::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(Token::new(9000)),
        sign_in: world.sign_in.expect("signed in party"),
        key: [31; 16],
        ask: jig_core_world::fixtures::goal(
            world,
            1,
            Box::from(&b"Project this goal"[..]),
            people::TaskExecutor::Agent { charter: 1 },
            jig_core_world::fixtures::Goal { budget: 20, priority: 1, ..Default::default() },
        ),
    })));
    world
        .people_answers
        .iter()
        .rev()
        .find_map(|reply| match reply {
            people::Reply::Outcome(people::Outcome::Created { first: task, .. }) => Some(*task),
            people::Reply::Signer(_)
            | people::Reply::Full { .. }
            | people::Reply::SignedIn { .. }
            | people::Reply::SignedOut
            | people::Reply::Outcome(_)
            | people::Reply::Refused(_)
            | people::Reply::Again(_) => None,
        })
        .expect("tracked goal admitted")
}

#[test]
fn a_conversation_asks_for_text_and_its_delegate_asks_for_nothing() {
    let (mut world, _) = tree_world();
    let parent = world.assigned.as_ref().expect("conversation assigned");
    let task = parent.task;
    assert_eq!(parent.live, core::LiveAsk { text: true, reasoning: false, input: true });
    let child = delegate_agent(&mut world, b"Delegate", 20, 0, 0);
    assert_ne!(child.0, task, "delegate admitted: {:?}", world.answers);
    assert_eq!(world.assigned.as_ref().expect("delegate assigned").live, core::LiveAsk::NONE);
}

fn projection_outbox(world: &World, goal: u64) -> Option<connector::OutboxEntry> {
    world.store.rows.values().find_map(|row| match row {
        root::Record::Connector { record: connector::Record::Outbox(entry), .. } if entry.task == goal => {
            Some(entry.clone())
        }
        root::Record::Core(_) | root::Record::Connector { .. } => None,
    })
}

#[test]
fn unresolved_projection_outcomes_keep_their_entry_across_restart_until_connector_recovery() {
    for (seed, recovery) in [(174, connector::Recovery::Keyed), (175, connector::Recovery::Unrecoverable)] {
        let configuration = || {
            let (mut config, limits) = untracked_configuration(seed);
            config.first.kinds.iter_mut().find(|kind| kind.kind == 5).expect("configured effect kind").recovery =
                recovery;
            (config, limits)
        };
        let (config, limits) = configuration();
        let mut world = World::configured(seed, false, config, limits);
        let goal = tracked_goal(&mut world);
        world.hold_projection_settlements = true;
        world.hold_writes = true;
        world.hold_lookups = true;
        world.send(root::Event::ProjectionEffect { goal, number: 1, effect: World::effect() });
        let entry = projection_outbox(&world, goal).expect("pending projection");
        let before = jig_core_world::fixtures::live_row(&world, goal).phase.clone();
        world.send(root::Event::Connector {
            number: 1,
            event: connector::Event::System(connector::SystemEvent::Applied {
                entry: entry.number,
                attempt: entry.attempt.expect("sent attempt").number,
                result: connector::ApplyResult::Uncertain,
            }),
        });
        assert!(projection_outbox(&world, goal).is_some(), "unresolved entry remains durable");
        assert_eq!(jig_core_world::fixtures::live_row(&world, goal).phase, before);
        world.hold_lookups = false;
        world.restart_with(configuration().0);
        assert!(projection_outbox(&world, goal).is_some(), "routing ownership survives restart");
        let procedures = world.procedure_inputs.len();
        match recovery {
            connector::Recovery::Unrecoverable => world.send(root::Event::Connector {
                number: 1,
                event: connector::Event::Resolve { entry: entry.number, repeat: false },
            }),
            connector::Recovery::Keyed => world.send(root::Event::Connector {
                number: 1,
                event: connector::Event::System(connector::SystemEvent::Applied {
                    entry: entry.number,
                    attempt: entry.attempt.expect("sent attempt").number,
                    result: connector::ApplyResult::Conflict,
                }),
            }),
            connector::Recovery::Conditional | connector::Recovery::Idempotent => unreachable!(),
        }
        assert!(projection_outbox(&world, goal).is_none(), "terminal recovery releases routing ownership");
        assert_eq!(jig_core_world::fixtures::live_row(&world, goal).phase, before);
        assert_eq!(world.procedure_inputs.len(), procedures);
        jig_core_world::fixtures::finish_chat(&mut world, 0, b"projection recovered");
    }
}
