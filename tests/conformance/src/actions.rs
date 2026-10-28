//! Scenario actions at outside boundaries; no action reads engine state.
use crate::{Config, Testing};
use jig_conformance::Harness;
use jig_fake_parties::{Act, Ask, Task};
use skein_lib::{ReplyTo, Time, Token, Wall};

/// The testing engine and its independent neighbours.
pub type World = Harness<Testing>;

/// Start a seeded scenario with a permanent engine slot or remote hosts.
#[must_use]
pub fn world(seed: u64, engine: bool) -> World {
    Harness::new(Config::new(seed, engine, 2, None), seed)
}

/// Ask the assigned task to make one named, permitted keyed write.
pub fn effect(world: &mut World, completion: u32) {
    let (task, attempt) = *world.peers.assignments.last().expect("scripted caller assigned");
    let mut effect = jig_core_world::effects::World::effect();
    effect.resources =
        Box::new([jig_test_connector_world::path(1, u8::try_from(completion).expect("tiny call number"))]);
    world.send(jig_test_domain::Event::EffectCall {
        to: ReplyTo::new(Token::new(77 + u64::from(completion))),
        key: jig_domain_core::CallKey { task, attempt, completion, position: 0 },
        number: 1,
        effect,
        deadline: Wall::from_nanos(world.clock().now + 1_000_000_000),
        proposal: None,
    });
}

/// The same party repeats its last request with the same durable request key.
pub fn duplicate(world: &mut World) {
    world.peers.peers.parties[0].extend(Box::new([Act::Twice]));
}

/// Send words from the independently signed-in party to its last task.
pub fn say(world: &mut World, key: u8) {
    world.peers.peers.parties[0].extend(Box::new([Act::Request {
        key: [key; 16],
        ask: Ask::Say { task: Task::Last, words: b"continue".as_slice().into(), release: None },
    }]));
}

/// Apply a host-link fault and feed the host's independently generated report.
pub fn host_fault(world: &mut World, index: usize, fault: jig_fake_workers::Fault) {
    let now = Time::from_nanos(world.clock().now);
    let peers = &mut world.peers.peers;
    let reports = peers.workers[index].fault(fault, now);
    let events: Vec<_> = reports.into_iter().map(|report| peers.worker_event(report)).collect();
    for event in events {
        world.send(event);
    }
}

/// Tell the engine to drain before its components close.
pub fn stop(world: &mut World) {
    world.send(jig_test_domain::Event::Stop);
}

pub(crate) fn prepare_racing_decisions(world: &mut World) {
    use jig_domain_core as core;
    use jig_domain_tasks as tasks;
    let (task, attempt) = world.peers.assignments[0];
    let mut authority = match &world.store.rows[&jig_test_domain::Key::Core(core::Key::Tasks(tasks::Key::Live(task)))] {
        jig_test_domain::Record::Core(core::Record::Tasks(tasks::Stored::Live(row))) => row.authority.clone(),
        row @ (jig_test_domain::Record::Core(_) | jig_test_domain::Record::Connector { .. }) => {
            panic!("parent row {row:?}")
        }
    };
    authority.budget.spend = 10;
    authority.delegation = tasks::Delegation { kinds: Box::new([]), tasks: 0, depth: 0 };
    world.send(jig_test_domain::Event::Core(core::Event::DelegateValidated {
        to: ReplyTo::new(Token::new(8001)),
        key: core::CallKey { task, attempt, completion: 1, position: 0 },
        batch: Box::new([core::Delegate {
            deadline: jig_domain_tasks::Deadline::Named,
            label: 0,
            flags: jig_domain_tasks::Flags::PLAIN,
            tracked: None,
            budget: jig_domain_people::Budget::Named,
            executor: tasks::Executor::Agent { charter: 1 },
            spec: tasks::Spec {
                asks: false,
                words: b"race target".as_slice().into(),
                parameters: Box::new([]),
                inputs: Box::new([]),
            },
            contract: tasks::Contract::Report { words: 128 },
            authority,
            llm: None,
            symbolic_grants: Box::new([]),
            dependencies: Box::new([]),
            wake: tasks::WakePolicy::DEFAULT,
        }]),
        stubs: Box::new([]),
    }));
}
pub(crate) fn run_racing_amendment(world: &mut World) {
    use jig_domain_core as core;
    use jig_domain_tasks as tasks;
    let (task, attempt) = world.peers.assignments[0];
    world.send(jig_test_domain::Event::Core(core::Event::NamedAction {
        to: ReplyTo::new(Token::new(8002)),
        key: core::CallKey { task, attempt, completion: 2, position: 0 },
        action: core::NamedAction::Amend {
            target: world.peers.children[0],
            revision: 0,
            amendment: tasks::Amendment {
                tracked: None,
                addressed: None,
                release: None,
                instructions: None,
                wake: None,
                dependencies: None,
                authority: None,
                llm: None,
                reason: b"run evidence".as_slice().into(),
                spec: Some(tasks::Spec {
                    asks: false,
                    words: b"run choice".as_slice().into(),
                    parameters: Box::new([]),
                    inputs: Box::new([]),
                }),
            },
        },
    }));
}
pub(crate) fn check_racing_decisions(world: &World) -> Result<(), jig_conformance::referee::Violation> {
    use jig_domain_core as core;
    use jig_domain_tasks as tasks;
    let target = world.peers.children[0];
    let row = &world.store.rows[&jig_test_domain::Key::Core(core::Key::Tasks(tasks::Key::Live(target)))];
    let jig_test_domain::Record::Core(core::Record::Tasks(tasks::Stored::Live(row))) = row else {
        panic!("live target")
    };
    let (task, attempt) = world.peers.assignments[0];
    let key = core::CallKey { task, attempt, completion: 2, position: 0 };
    let call = &world.store.rows[&jig_test_domain::Key::Core(core::Key::Core(core::CoreKey::Call(key)))];
    let jig_test_domain::Record::Core(core::Record::Core(core::CoreRecord::Call(call))) = call else {
        panic!("call row")
    };
    if call.part == (core::CallPart::Moved { task: target, now: jig_domain_people::Moved::Spec { revision: 1 } })
        && row.revisions.spec == 1
        && row.spec.words.as_ref() == b"party choice"
    {
        Ok(())
    } else {
        Err(jig_conformance::referee::Violation {
            promise: jig_conformance::referee::Promise::Ownership,
            why: format!("racing revision decisions: {:?}; spec {:?}", call.part, row.spec),
        })
    }
}
