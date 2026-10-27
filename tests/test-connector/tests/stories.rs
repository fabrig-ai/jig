use std::collections::VecDeque;

use jig_test_connector::{
    Class, Effect, Event, Fact, Hold, HoldMode, Origin, Outcome, ProcedureSignal, Read, Record, RecordKey, Request,
    ResourceRole, StepDecision, SystemEvent, SystemRequest, Verdict,
};
use jig_test_connector_world::{World, kind, path};
use jig_test_system::Fault;
use skein_lib::{Token, Wall};

fn effect(seed: u8, local: u16, purpose: u64, target: u64, condition: Option<u64>) -> Effect {
    Effect { kind: kind(seed, local), resources: Box::from([path(seed, 1)]), purpose, condition, target, state: target }
}

fn keep(world: &mut World, entry: u64, local: u16, purpose: u64, target: u64, condition: Option<u64>) -> Vec<Request> {
    let token = Token::new(entry);
    let described =
        world.event(Event::Describe { token, effect: effect(world.seed, local, purpose, target, condition) });
    assert!(matches!(described.as_slice(), [Request::Described { .. }]));
    let requests = world.event(Event::Keep { token, entry, task: 4, key: world.key(4, purpose) });
    assert!(matches!(requests.as_slice(), [Request::Save { .. }, Request::Make { .. }]));
    requests
}

fn made(world: &World, entry: u64) -> usize {
    world
        .trace()
        .iter()
        .filter(|request| matches!(request, Request::Outcome { entry: number, outcome: Outcome::Made { .. } } if *number == entry))
        .count()
}

#[test]
fn keyed_effects_make_once_across_three_durable_restart_cuts() {
    for seed in 1..=12 {
        let mut world = World::new(seed);
        let kept = keep(&mut world, 1, 1, 10, 7, None);
        match seed % 3 {
            0 => {
                world.cold_restart();
                world.settle_restart(&mut VecDeque::new());
            }
            1 => {
                let prepared = world.event(Event::Make { entry: 1 });
                assert!(matches!(
                    prepared.as_slice(),
                    [Request::Save { .. }, Request::System(SystemRequest::Apply { .. })]
                ));
                world.wall = 7;
                world.cold_restart();
                world.settle_restart(&mut VecDeque::new());
            }
            2 => {
                world.release(&kept, &mut VecDeque::from([Fault::AfterApply]));
                world.cold_restart();
                world.settle_restart(&mut VecDeque::new());
            }
            _ => unreachable!(),
        }
        assert_eq!(made(&world, 1), 1, "seed {seed}");
        assert_eq!(world.system.state(&path(seed, 1)), Some(7));
        assert_eq!(world.system.observed().iter().filter(|effect| effect.applied).count(), 1);
        assert!(world.records.contains_key(&RecordKey::Made(world.key(4, 10))));
    }
}

#[test]
fn late_copies_retries_and_each_recovery_class_keep_their_promise() {
    let mut keyed = World::new(13);
    let kept = keep(&mut keyed, 1, 1, 10, 7, None);
    keyed.release(&kept, &mut VecDeque::from([Fault::Late]));
    keyed.wall = 7;
    let due = keyed.fire();
    keyed.release(&due, &mut VecDeque::new());
    let late = keyed.system.deliver_late().expect("late copy exists");
    keyed.event(Event::System(late));
    assert_eq!(made(&keyed, 1), 1);
    assert_eq!(keyed.system.observed().iter().filter(|effect| effect.applied).count(), 1);

    let mut conditional = World::new(14);
    conditional.system.other_hand(&path(14, 1), 1);
    let kept = keep(&mut conditional, 1, 2, 11, 3, Some(1));
    conditional.release(&kept, &mut VecDeque::from([Fault::AfterApply]));
    conditional.cold_restart();
    conditional.settle_restart(&mut VecDeque::new());
    assert_eq!(made(&conditional, 1), 1);
    assert_eq!(conditional.system.state(&path(14, 1)), Some(3));

    let mut another = World::new(15);
    another.system.other_hand(&path(15, 1), 1);
    let kept = keep(&mut another, 1, 2, 12, 3, Some(1));
    let prepared = another.event(Event::Make { entry: 1 });
    assert!(matches!(kept.as_slice(), [Request::Save { .. }, Request::Make { .. }]));
    another.system.other_hand(&path(15, 1), 3);
    another.release(&prepared, &mut VecDeque::new());
    assert_eq!(made(&another, 1), 0);
    assert!(another.trace().iter().any(|request| matches!(request, Request::Outcome { outcome: Outcome::Failed, .. })));

    let mut idempotent = World::new(16);
    let kept = keep(&mut idempotent, 1, 3, 13, 9, None);
    idempotent.release(&kept, &mut VecDeque::from([Fault::Late]));
    idempotent.wall = 7;
    let due = idempotent.fire();
    idempotent.release(&due, &mut VecDeque::new());
    let late = idempotent.system.deliver_late().expect("late idempotent copy exists");
    idempotent.event(Event::System(late));
    assert_eq!(made(&idempotent, 1), 1);
    assert_eq!(idempotent.system.state(&path(16, 1)), Some(9));

    let mut unrecoverable = World::new(17);
    let kept = keep(&mut unrecoverable, 1, 4, 14, 8, None);
    unrecoverable.release(&kept, &mut VecDeque::from([Fault::Late]));
    unrecoverable.wall = 7;
    assert!(unrecoverable.fire().is_empty());
    assert_eq!(unrecoverable.system.observed().len(), 0);
    assert!(
        unrecoverable.trace().iter().any(|request| matches!(request, Request::Outcome { outcome: Outcome::Held, .. }))
    );
}

#[test]
fn a_committed_deadline_survives_restart_and_is_obeyed_when_due() {
    let mut world = World::new(18);
    drop(keep(&mut world, 1, 1, 10, 7, None));
    let prepared = world.event(Event::Make { entry: 1 });
    let Some(Request::Save { record: Record::Outbox(entry) }) = prepared.first() else {
        panic!("attempt saved before system call");
    };
    let deadline = entry.attempt.expect("attempt prepared").deadline;
    assert_eq!(deadline, Wall::from_nanos(6));
    world.wall = deadline.as_nanos() - 1;
    world.cold_restart();
    world.settle_restart(&mut VecDeque::new());
    assert_eq!(made(&world, 1), 0);
    world.wall = deadline.as_nanos();
    let due = world.fire();
    world.release(&due, &mut VecDeque::new());
    assert_eq!(made(&world, 1), 1);
}

#[test]
fn procedures_require_answers_and_current_facts_and_replay_the_same_decision() {
    let mut world = World::new(19);
    let task = 7;
    let step = |world: &mut World, signal| {
        world.event(Event::Procedure {
            pending: Box::new([]),
            added: Box::new([]),
            project: 1,
            task,
            number: kind(19, 1),
            resource: path(19, 1),
            signal,
        })
    };
    let first = step(&mut world, ProcedureSignal::Activate);
    assert!(matches!(first.last(), Some(Request::Step { decision: StepDecision::Effect(..), .. })));
    assert!(step(&mut world, ProcedureSignal::Message).is_empty());
    world.cold_restart();
    assert!(step(&mut world, ProcedureSignal::Message).is_empty());
    assert!(matches!(
        step(&mut world, ProcedureSignal::Settled).last(),
        Some(Request::Step { decision: StepDecision::Delegate { .. }, .. })
    ));
    assert!(matches!(
        step(&mut world, ProcedureSignal::DelegateDone).last(),
        Some(Request::Step { decision: StepDecision::Propose { .. }, .. })
    ));
    let waiting = step(&mut world, ProcedureSignal::ProposalDone);
    assert!(matches!(waiting.last(), Some(Request::Step { decision: StepDecision::Wait { .. }, .. })));
    let repeated = step(&mut world, ProcedureSignal::Message);
    assert_eq!(repeated.last(), waiting.last());
    let fact = Fact { state: Some(5), observed: Wall::from_nanos(world.wall), pending: false };
    world.event(Event::System(SystemEvent::Fact { resource: path(19, 1), fact, origin: Origin::Own }));
    assert!(matches!(
        step(&mut world, ProcedureSignal::Message).last(),
        Some(Request::Step { decision: StepDecision::Finish { .. }, .. })
    ));
    assert!(step(&mut world, ProcedureSignal::Message).is_empty());
}

#[test]
fn verdicts_respect_freshness_and_a_fact_can_change_before_an_effect() {
    let mut world = World::new(20);
    let resource = path(20, 1);
    let fact = Fact { state: Some(1), observed: Wall::from_nanos(1), pending: false };
    world.event(Event::System(SystemEvent::Fact { resource: resource.clone(), fact, origin: Origin::Other }));
    world.wall = 10;
    let token = Token::new(1);
    let judged = world.event(Event::Judge {
        token,
        requirement: kind(20, 2),
        resources: Box::from([resource.clone()]),
        state: 1,
    });
    assert!(matches!(judged.first(), Some(Request::Verdict { verdict: Verdict::Wait, .. })));
    world.system.other_hand(&resource, 1);
    world.release(&judged, &mut VecDeque::new());
    assert!(world.trace().iter().any(|request| matches!(request, Request::Verdict { verdict: Verdict::Met { observed, .. }, .. } if *observed == Wall::from_nanos(10))));
    let kept = keep(&mut world, 1, 2, 10, 2, Some(1));
    let prepared = world.event(Event::Make { entry: 1 });
    assert!(matches!(kept.last(), Some(Request::Make { .. })));
    world.system.other_hand(&resource, 2);
    world.release(&prepared, &mut VecDeque::new());
    assert_eq!(made(&world, 1), 0);
    assert!(world.trace().iter().any(|request| matches!(request, Request::Outcome { outcome: Outcome::Failed, .. })));
}

#[test]
fn shrinking_pools_news_and_values_cross_the_root_once() {
    let mut world = World::new(21);
    let resource = path(21, 2);
    world.event(Event::Adopt { project: 1, resource: resource.clone(), role: ResourceRole::Owned });
    let mut held = Vec::new();
    let mut waiting = Vec::new();
    for task in [1, 2, 3] {
        let named = world.event(Event::Names { task, project: 1, resources: Box::from([resource.clone()]) });
        assert!(
            matches!(named.last(), Some(Request::Named { resources, .. }) if resources[0].hold == Hold::Pooled { mode: HoldMode::Wait })
        );
        if held.len() < 2 {
            held.push(task);
        } else {
            waiting.push(task);
        }
        world.event(Event::Subscribe {
            markers: 0,
            task,
            topic: kind(21, 1),
            wake_at: if task == 3 { 7 } else { 5 },
            keep_at: if task == 3 { 4 } else { 2 },
        });
    }
    let shrunk =
        world.event(Event::System(SystemEvent::Pool { path: resource.clone(), slots: 0, lost: Box::from([1]) }));
    assert!(matches!(
        shrunk.as_slice(),
        [Request::Save { .. }, Request::Slots { slots: 0, .. }, Request::Drift { .. }]
    ));
    assert_eq!(held, [1, 2], "shrinking the pool does not retract existing holds");
    assert_eq!(waiting, [3], "a waiting task cannot take a vanished slot");
    let news = world.event(Event::System(SystemEvent::News {
        markers: 0,
        topic: kind(21, 1),
        importance: 5,
        origin: Origin::Other,
    }));
    let Some(Request::News { subscribers, .. }) = news.first() else { panic!("news reaches subscribers") };
    assert_eq!(subscribers.len(), 3);
    assert_eq!(subscribers.iter().filter(|subscriber| subscriber.class == Class::Wake).count(), 2);
    assert_eq!(subscribers.iter().filter(|subscriber| subscriber.class == Class::Keep).count(), 1);
    assert!(
        world
            .event(Event::System(SystemEvent::News {
                markers: 0,
                topic: kind(21, 1),
                importance: 1,
                origin: Origin::Other
            }))
            .is_empty()
    );
    assert!(
        world
            .event(Event::System(SystemEvent::News {
                markers: 0,
                topic: kind(21, 1),
                importance: 5,
                origin: Origin::Own
            }))
            .is_empty()
    );
    let token = Token::new(9);
    assert!(
        matches!(world.event(Event::Read { token, read: Read { resource: resource.clone(), size: 8 } }).as_slice(), [Request::Answer { bytes, .. }] if bytes.len() == 8)
    );
    assert!(matches!(
        world.event(Event::Gather { token, task: 1, budget: 18 }).as_slice(),
        [Request::Ready { size: 18, .. }]
    ));
    assert!(matches!(world.event(Event::Cut { token, size: 9 }).as_slice(), [Request::Ready { size: 9, .. }]));
    assert!(
        matches!(world.event(Event::HandOver { token }).as_slice(), [Request::Section { bytes, .. }] if bytes.len() == 9)
    );
    assert!(world.event(Event::HandOver { token }).is_empty());
    assert!(matches!(world.event(Event::Items { token, task: 1 }).as_slice(), [Request::Ready { size: 1, .. }]));
    assert!(
        matches!(world.event(Event::HandOver { token }).as_slice(), [Request::Workspace { items, .. }] if items.len() == 1)
    );
    assert!(world.event(Event::HandOver { token }).is_empty());
    world.event(Event::Gather { token, task: 1, budget: 8 });
    world.event(Event::Drop { token });
    assert!(world.event(Event::HandOver { token }).is_empty());
}

#[test]
fn idle_system_calls_do_not_grow_with_preloaded_history() {
    let mut small = World::new(22);
    let mut large = World::new(22);
    small.system.preload_history(100);
    large.system.preload_history(1000);
    for _ in 0..100 {
        small.fire();
        large.fire();
    }
    assert_eq!(small.system.calls(), large.system.calls());
    assert_eq!(small.system.calls(), 0);
    assert_eq!(small.trace(), large.trace());
}

#[test]
fn a_persons_retry_preserves_the_previous_attempt_deadline_across_restart() {
    let mut world = World::new(35);
    drop(keep(&mut world, 1, 4, 10, 7, None));
    let prepared = world.event(Event::Make { entry: 1 });
    let Request::Save { record: Record::Outbox(entry) } = &prepared[0] else {
        panic!("attempt saved before write");
    };
    let mut deadline = entry.attempt.expect("saved attempt").deadline;
    for attempt in 1..=3 {
        world.event(Event::System(SystemEvent::Applied {
            entry: 1,
            attempt,
            result: if attempt == 3 {
                jig_test_connector::ApplyResult::Uncertain
            } else {
                jig_test_connector::ApplyResult::Transient
            },
        }));
        if attempt < 3 {
            world.wall = deadline.as_nanos();
            let next = world.fire();
            let Request::Save { record: Record::Outbox(entry) } = &next[0] else {
                panic!("automatic attempt committed");
            };
            deadline = entry.attempt.expect("saved next attempt").deadline;
        }
    }
    let retry = world.event(Event::Resolve { entry: 1, repeat: true });
    assert!(
        matches!(retry.as_slice(), [Request::Save { record: Record::Outbox(entry) }] if entry.attempt.expect("saved retry attempt").deadline == deadline)
    );
    assert!(
        world
            .event(Event::System(SystemEvent::Applied {
                entry: 1,
                attempt: 3,
                result: jig_test_connector::ApplyResult::Uncertain,
            }))
            .is_empty(),
        "a late copy does not revoke the committed retry choice"
    );
    world.wall = deadline.as_nanos() - 1;
    world.cold_restart();
    world.settle_restart(&mut VecDeque::new());
    assert!(world.system.observed().is_empty());
    world.wall = deadline.as_nanos();
    let due = world.fire();
    assert!(matches!(due.as_slice(), [Request::Save { .. }, Request::System(SystemRequest::Apply { attempt: 4, .. })]));
    world.release(&due, &mut VecDeque::new());
    assert_eq!(made(&world, 1), 1);
}

#[test]
fn procedure_delegate_outcomes_are_bounded_replayed_updated_and_erased() {
    let mut world = World::new(3);
    for task in 1..=8 {
        let requests =
            world.event(Event::StepMade { task, step: task + 20, first: Some(task + 100), revisions: Box::new([]) });
        assert!(matches!(requests.as_slice(), [Request::Save { .. }]));
    }
    assert_eq!(
        world.event(Event::StepMade { task: 9, step: 29, first: None, revisions: Box::new([]) }),
        vec![Request::Refused { task: 9 }]
    );
    world.cold_restart();
    assert_eq!(
        world.event(Event::StepMade { task: 9, step: 30, first: None, revisions: Box::new([]) }),
        vec![Request::Refused { task: 9 }]
    );
    let record = Record::ProcedureDelegateOutcome { task: 1, step: 99, child: None, revisions: Box::new([]) };
    assert_eq!(
        world.event(Event::StepMade { task: 1, step: 99, first: None, revisions: Box::new([]) }),
        vec![Request::Save { record: record.clone() }]
    );
    assert_eq!(world.records.get(&record.key()), Some(&record));
    assert_eq!(
        world.event(Event::Unnamed { task: 1 }),
        vec![Request::Erase { key: RecordKey::ProcedureDelegateOutcome(1) }]
    );
    assert!(!world.records.contains_key(&record.key()));
    assert!(matches!(
        world.event(Event::StepMade { task: 9, step: 30, first: None, revisions: Box::new([]) }).as_slice(),
        [Request::Save { .. }]
    ));
}

#[test]
fn an_effect_description_carries_whether_its_kind_waits_for_dependencies() {
    const SEED: u8 = 43;
    for waits_for_dependencies in [false, true] {
        let mut configuration = jig_test_connector_world::config(SEED);
        configuration.kinds[0].waits_for_dependencies = waits_for_dependencies;
        let mut world = World::new(SEED);
        world.domain = jig_test_connector::Domain::new(configuration, &jig_test_connector_world::LIMITS);
        let requests = world.event(Event::Describe { token: Token::new(1), effect: effect(SEED, 1, 10, 7, None) });
        let [Request::Described { waits_for_dependencies: waits, .. }] = requests.as_slice() else {
            panic!("described effect")
        };
        assert_eq!(*waits, waits_for_dependencies);
    }
}
