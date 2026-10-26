use jig_core_world::{effects::World, world};
use jig_domain_core as core;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_test_domain as root;
use skein_lib::{ReplyTo, Token};

#[test]
fn a_delegate_on_another_charter_runs_its_instructions_and_account() {
    let (config, limits) = world::two_charters(141);
    let mut world = World::configured(141, false, config, limits);
    assert_eq!(world.assigned.as_ref().expect("chat").charter, 1);
    world.grant_account(2);
    world.delegate_agent(2, b"find the regression");
    let assignment = world.assigned.as_ref().expect("delegate assigned");
    assert_eq!(assignment.charter, 2);
    assert_eq!(assignment.run.policy.instructions.as_ref(), b"Research the question.");
    assert_eq!(assignment.grants[0].account, 2);
}

#[test]
fn a_task_waits_while_its_charters_account_is_unusable() {
    let (config, limits) = world::two_charters(142);
    let mut world = World::configured(142, false, config, limits);
    let chat = world.assigned.as_ref().expect("chat").task;
    world.delegate_agent(2, b"wait for the account");
    assert_eq!(world.assigned.as_ref().expect("chat stays assigned").task, chat);
    assert!(world.store.rows.values().any(|row| matches!(row,
        root::Record::Core(core::Record::Tasks(tasks::Stored::Live(task)))
        if task.executor == tasks::Executor::Agent { charter: 2 } && task.attempt == 0)));
    world.grant_account(2);
    assert_eq!(world.assigned.as_ref().expect("delegate after grant").charter, 2);
}

#[test]
fn a_delegate_waits_for_a_host_declaring_its_charters_model() {
    use jig_domain_fleet as fleet;
    let (mut config, mut limits) = world::two_charters(151);
    limits.core.fleet.workers = 2;
    config.core.defaults.charters[1].policy.model.name = b"specialist".as_slice().into();
    config.core.settings.llm_providers.providers[1].models = Box::new([b"specialist".as_slice().into()]);
    let specialist = config.core.defaults.charters[1].policy.model.clone();
    config.core.settings.models = Box::new([config.core.settings.models[0].clone(), specialist]);
    let mut world = World::configured(151, false, config, limits);
    let chat = world.assigned.as_ref().expect("chat").task;
    world.grant_account(2);
    world.delegate_agent(2, b"wait for a specialist host");
    assert_eq!(world.assigned.as_ref().expect("chat stays assigned").task, chat);
    let mut bounds = jig_core_world::fixtures::bounds();
    bounds.models[1].name = b"specialist".as_slice().into();
    world.send(root::Event::Core(core::Event::Fleet(fleet::Event::Hello {
        worker: 1,
        channel: Token::new(8),
        hello: fleet::Hello {
            bounds,
            stop_bound: skein_lib::Duration::from_millis(100),
            slots: 1,
            workstreams: Box::new([]),
            hosting: Box::new([]),
        },
    })));
    let assignment = world.assigned.as_ref().expect("delegate after suitable hello");
    assert_eq!(assignment.charter, 2);
    assert_eq!(assignment.run.policy.model.name.as_ref(), b"specialist");
}

#[test]
fn a_restart_restores_a_task_on_the_second_charter() {
    let (config, limits) = world::two_charters(143);
    let mut world = World::configured(143, false, config, limits);
    world.delegate_agent(2, b"research across restart");
    let task = world
        .store
        .rows
        .values()
        .find_map(|row| match row {
            root::Record::Core(core::Record::Tasks(tasks::Stored::Live(task)))
                if task.executor == tasks::Executor::Agent { charter: 2 } =>
            {
                Some(task.number)
            }
            root::Record::Core(_) | root::Record::Connector { .. } => None,
        })
        .expect("durable delegate");
    world.restart_with(world::two_charters(143).0);
    world.grant_account(2);
    rejoin(&mut world);
    let assignment = world.assigned.as_ref().expect("restored delegate assigned");
    assert_eq!(assignment.task, task);
    assert_eq!(assignment.charter, 2);
    assert_eq!(assignment.run.policy.instructions.as_ref(), b"Research the question.");
    assert_eq!(assignment.grants[0].account, 2);
}

fn rejoin(world: &mut World) {
    world.send(root::Event::Core(core::Event::Fleet(jig_domain_fleet::Event::Hello {
        worker: 0,
        channel: skein_lib::Token::new(7),
        hello: jig_domain_fleet::Hello {
            bounds: jig_core_world::fixtures::bounds(),
            stop_bound: skein_lib::Duration::from_millis(100),
            slots: 2,
            workstreams: Box::new([]),
            hosting: Box::new([]),
        },
    })));
}

fn set_goal(world: &mut World, charter: u32) {
    world.send(root::Event::Core(core::Event::People(people::Event::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(Token::new(9000)),
        sign_in: world.sign_in.expect("signed in party"),
        key: [31; 16],
        ask: jig_core_world::fixtures::goal(
            world,
            1,
            b"Research this goal".as_slice().into(),
            people::TaskExecutor::Agent { charter },
            jig_core_world::fixtures::Goal { budget: 20, priority: 1, ..Default::default() },
        ),
    })));
}

#[test]
fn a_goal_on_the_second_charter_is_admitted_and_runs_it() {
    let (config, limits) = world::two_charters(144);
    let mut world = World::configured(144, false, config, limits);
    world.grant_account(2);
    set_goal(&mut world, 2);
    assert!(
        world
            .people_answers
            .iter()
            .any(|reply| matches!(reply, people::Reply::Outcome(people::Outcome::Created { .. })))
    );
    let assignment = world.assigned.as_ref().expect("goal assigned");
    assert_eq!(assignment.charter, 2);
    assert_eq!(assignment.run.policy.instructions.as_ref(), b"Research the question.");
    assert_eq!(assignment.grants[0].account, 2);
}

#[test]
fn a_goal_naming_an_unoffered_charter_is_refused_not_offered() {
    let (config, limits) = world::two_charters(145);
    let mut world = World::configured(145, false, config, limits);
    set_goal(&mut world, 9);
    assert!(
        world.people_answers.iter().any(|reply| matches!(
            reply,
            people::Reply::Outcome(people::Outcome::Refused(people::Refusal::NotOffered))
        ))
    );
}

fn host_world(seed: u64, kinds: jig_domain_fleet::Kinds) -> World {
    use jig_core_world::peers::{Peers, fixture};
    use jig_fake_parties::{Act, Ask, Party, Role};
    use jig_fake_workers::{Script, Worker};
    let (mut config, limits) = fixture(seed, 1);
    config.core.defaults.charters[0].hosts = kinds;
    let workers = [0, 7].into_iter().map(|channel| Worker::new(channel, 1, vec![Box::new([Script::Wait])])).collect();
    let party = Party::new(
        1,
        Role::Owner,
        10_000,
        Box::new([Act::SignIn { provider: 0, subject: 7_u64.to_be_bytes().into() }]),
    );
    let mut world = World::scripted(config, limits, Peers::new(workers, vec![party]));
    world.peers.as_mut().expect("peers").parties[0]
        .extend(Box::new([Act::Request { key: [1; 16], ask: Ask::Chat { words: Box::from(&b"hello"[..]) } }]));
    world.drain();
    world
}

#[test]
fn a_charter_for_workers_never_runs_in_the_engines_slots() {
    let world = host_world(146, jig_domain_fleet::Kinds::Workers);
    let peers = world.peers.as_ref().expect("peers");
    assert!(peers.workers[0].seen.is_empty(), "engine slot stays free");
    assert_eq!(peers.workers[1].seen.len(), 1, "worker received the assignment");
}

#[test]
fn a_charter_for_the_engine_never_runs_on_a_worker() {
    let world = host_world(147, jig_domain_fleet::Kinds::Engine);
    let peers = world.peers.as_ref().expect("peers");
    assert_eq!(peers.workers[0].seen.len(), 1, "engine received the assignment");
    assert!(peers.workers[1].seen.is_empty(), "worker slot stays free");
}

#[test]
fn a_restart_settles_an_engine_charters_claim_at_once() {
    let (mut config, mut limits) = jig_core_world::effects::fixture(148, false);
    config.core.defaults.charters[0].hosts = jig_domain_fleet::Kinds::Engine;
    limits.core.fleet.engine_slots = 1;
    let mut world = World::configured(148, false, config, limits);
    let assignment = world.assigned.as_ref().expect("engine assigned");
    let task = assignment.task;
    let attempt = assignment.attempt;
    world.send(root::Event::Turn {
        opens_window: false,
        refused: Box::new([]),
        channel: Token::new(0),
        task,
        attempt,
        turn: 1,
        cumulative: 2,
        read: None,
        transcript: b"engine turn".as_slice().into(),
    });
    assert!(world.store.rows.values().any(|record| matches!(record,
        root::Record::Core(core::Record::Core(core::CoreRecord::RunProof(proof)))
        if proof.task == task && proof.host == jig_domain_fleet::HostKind::Engine)));
    let mut config = jig_core_world::effects::fixture(148, false).0;
    config.core.defaults.charters[0].hosts = jig_domain_fleet::Kinds::Engine;
    world.restart_with(config);
    assert!(world.domain.ready(), "restart does not wait for a worker hello");
    assert!(world.store.rows.values().any(|record| matches!(record,
        root::Record::Core(core::Record::Tasks(tasks::Stored::Live(row)))
        if row.number == task && row.tries.lost == 1)));
}

fn worker_with_affinity(world: &mut World, task: u64) {
    use jig_domain_fleet as fleet;
    world.send(root::Event::Core(core::Event::Fleet(fleet::Event::Hello {
        worker: 0,
        channel: Token::new(8),
        hello: fleet::Hello {
            bounds: jig_core_world::fixtures::bounds(),
            stop_bound: skein_lib::Duration::from_millis(100),
            slots: 1,
            workstreams: Box::new([task]),
            hosting: Box::new([]),
        },
    })));
}

fn both_host_world(seed: u64) -> World {
    let (mut config, mut limits) = world::two_charters(seed);
    config.core.defaults.charters[1].hosts = jig_domain_fleet::Kinds::Both;
    limits.core.fleet.engine_slots = 1;
    limits.core.fleet.slots = 1;
    let mut world = World::configured(seed, false, config, limits);
    world.grant_account(2);
    world
}

fn delegate_task(world: &World) -> u64 {
    world
        .store
        .rows
        .values()
        .find_map(|row| match row {
            root::Record::Core(core::Record::Tasks(tasks::Stored::Live(task)))
                if task.executor == tasks::Executor::Agent { charter: 2 } =>
            {
                Some(task.number)
            }
            root::Record::Core(_) | root::Record::Connector { .. } => None,
        })
        .expect("delegate in store")
}

fn first_claim_host(world: &World) -> jig_domain_fleet::HostKind {
    let task = delegate_task(world);
    world
        .commits
        .iter()
        .flatten()
        .find_map(|write| match write {
            root::Write::Save(root::Record::Core(core::Record::Core(core::CoreRecord::RunProof(proof))))
                if proof.task == task =>
            {
                Some(proof.host)
            }
            root::Write::Save(_)
            | root::Write::Wires
            | root::Write::Erase(_)
            | root::Write::CopySettingsRevision(_) => None,
        })
        .expect("first claim submitted")
}

fn current_claim_host(world: &World) -> jig_domain_fleet::HostKind {
    let task = delegate_task(world);
    let key = root::Key::Core(core::Key::Core(core::CoreKey::RunProof(task)));
    let root::Record::Core(core::Record::Core(core::CoreRecord::RunProof(proof))) = &world.store.rows[&key] else {
        panic!("durable claim proof")
    };
    proof.host
}

#[test]
fn a_charter_both_kinds_may_run_claims_the_engine_when_only_its_slot_is_free() {
    let mut world = both_host_world(149);
    world.delegate_agent(2, b"use the free engine slot");
    assert_eq!(first_claim_host(&world), jig_domain_fleet::HostKind::Engine);
}

#[test]
fn a_claim_for_the_engine_is_placed_only_on_the_engine() {
    let mut world = both_host_world(150);
    let assigned = world.assignments.len();
    world.store.fault(jig_core_world::Fault::Hold { commits: 2 });
    world.delegate_agent(2, b"keep the claimed kind");
    world.release_commits();
    assert_eq!(world.assignments.len(), assigned, "claim waits for its completion");
    let task = delegate_task(&world);
    worker_with_affinity(&mut world, task);
    world.release_commits();
    assert_eq!(world.assigned.as_ref().expect("delegate assigned").task, task);
    assert_eq!(current_claim_host(&world), jig_domain_fleet::HostKind::Engine);
}

#[test]
fn a_task_created_without_an_llm_choice_records_its_default_and_its_run_uses_it() {
    let world = World::new(153, false);
    let assignment = world.assigned.as_ref().expect("chat assigned");
    let chat = world
        .store
        .rows
        .values()
        .find_map(|row| match row {
            root::Record::Core(core::Record::Tasks(tasks::Stored::Live(task))) if task.number == assignment.task => {
                Some(task)
            }
            root::Record::Core(_) | root::Record::Connector { .. } => None,
        })
        .expect("durable chat");
    let default = tasks::ModelEffort { model: 0, effort: Some(0) };
    assert_eq!(
        chat.llm,
        Some(tasks::LlmChoice { account: None, main: default, subagents: Box::new([]) }),
        "recorded as given"
    );
    assert_eq!(assignment.run.policy.model.name.as_ref(), b"scripted");
    assert_eq!(assignment.run.policy.model.effort.as_deref(), Some(b"low".as_slice()), "its first declared effort");
}
