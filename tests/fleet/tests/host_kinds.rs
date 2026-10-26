//! The engine's local slots and worker-only charters at the fleet boundary.

use jig_domain_fleet::{self as fleet, Answer, Domain, Event, Hello, HostKind, Kinds, Limits, Request};
use skein_lib::{Duration, Env, Queue, ReplyTo, Time, Token, Wall};

fn step(domain: &mut Domain, limits: Limits, event: Event) -> Vec<Request> {
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits };
    let mut out = Queue::with_capacity(fleet::max_out(&limits));
    fleet::step(domain, &env, event, &mut out);
    let mut requests = Vec::new();
    while let Some(request) = out.pop() {
        if let Request::Sources { channel, sources } = request {
            assert!(sources.is_empty());
            fleet::step(domain, &env, Event::Logins { channel, logins: Box::new([]) }, &mut out);
        } else {
            requests.push(request);
        }
    }
    domain.reclaim();
    requests
}

fn place(domain: &mut Domain, limits: Limits) -> Vec<Request> {
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits };
    let mut out = Queue::with_capacity(fleet::max_out(&limits));
    fleet::resume(domain, &env, &mut out);
    let mut requests = Vec::new();
    while let Some(request) = out.pop() {
        if let Request::Sources { channel, sources } = request {
            assert!(sources.is_empty());
            fleet::step(domain, &env, Event::Logins { channel, logins: Box::new([]) }, &mut out);
        } else {
            requests.push(request);
        }
    }
    domain.reclaim();
    requests
}

#[test]
fn a_run_placed_on_the_engine_slots_is_settled_as_not_started_by_a_restart_before_its_first_turn() {
    let limits = Limits { workers: 0, engine_slots: 1, ..jig_fleet_world::LIMITS };
    let mut domain = jig_fleet_world::fixtures::domain(&limits, jig_fleet_world::fixtures::needs());
    let task = Token::new(1);
    let attempt = Token::new(2);
    assert!(
        step(
            &mut domain,
            limits,
            Event::Start {
                placement: None,
                version: jig_domain_fleet::Version { charter: 1, revision: 1 },
                grants: Box::new([]),
                reply_to: ReplyTo::new(attempt),
                run: task,
                attempt,
                workstream: 1,
                kinds: Kinds::Engine,
                assignment: refs()
            }
        )
        .is_empty()
    );
    assert_eq!(
        place(&mut domain, limits),
        vec![
            Request::Assign {
                channel: Token::new(0),
                kind: HostKind::Engine,
                run: task,
                attempt,
                activation: attempt.raw(),
                assignment: refs()
            },
            Request::Placed { run: task, attempt },
        ]
    );
    assert!(step(&mut domain, limits, Event::Lost { channel: Token::new(0) }).is_empty());
    assert_eq!(domain.attempts(), 1, "the local host stays until restart");
    let mut restarted = jig_fleet_world::fixtures::domain(&limits, jig_fleet_world::fixtures::needs());
    assert_eq!(
        step(
            &mut restarted,
            limits,
            Event::Adopt {
                holds: Box::new([]),
                grants: Box::new([]),
                reply_to: ReplyTo::new(attempt),
                run: task,
                attempt,
                kept: 0,
                kind: HostKind::Engine,
                worked: false
            }
        ),
        vec![Request::NotStarted { to: ReplyTo::new(attempt), run: task, attempt }]
    );
    let mut restarted_with_work = jig_fleet_world::fixtures::domain(&limits, jig_fleet_world::fixtures::needs());
    assert_eq!(
        step(
            &mut restarted_with_work,
            limits,
            Event::Adopt {
                holds: Box::new([]),
                grants: Box::new([]),
                reply_to: ReplyTo::new(attempt),
                run: task,
                attempt,
                kept: 0,
                kind: HostKind::Engine,
                worked: true,
            }
        ),
        vec![Request::Lost { to: ReplyTo::new(attempt), run: task, attempt }]
    );
}

#[test]
fn a_charter_for_workers_only_waits_while_only_engine_slots_are_free() {
    let limits = Limits { workers: 1, engine_slots: 1, ..jig_fleet_world::LIMITS };
    let mut domain = jig_fleet_world::fixtures::domain(&limits, jig_fleet_world::fixtures::needs());
    let task = Token::new(3);
    let attempt = Token::new(4);
    assert!(
        step(
            &mut domain,
            limits,
            Event::Start {
                placement: None,
                version: jig_domain_fleet::Version { charter: 1, revision: 1 },
                grants: Box::new([]),
                reply_to: ReplyTo::new(attempt),
                run: task,
                attempt,
                workstream: 3,
                kinds: Kinds::Workers,
                assignment: refs()
            }
        )
        .is_empty()
    );
    assert!(place(&mut domain, limits).is_empty());
    assert_eq!(domain.waiting(), 1);
    assert!(
        step(
            &mut domain,
            limits,
            Event::Hello {
                worker: 0,
                channel: Token::new(5),
                hello: Hello {
                    bounds: jig_fleet_world::fixtures::bounds(),
                    slots: 1,
                    stop_bound: Duration::from_secs(1),
                    workstreams: Box::new([]),
                    hosting: Box::new([]),
                }
            }
        )
        .is_empty()
    );
    assert_eq!(
        place(&mut domain, limits),
        vec![
            Request::Assign {
                channel: Token::new(5),
                kind: HostKind::Worker,
                run: task,
                attempt,
                activation: attempt.raw(),
                assignment: refs()
            },
            Request::Placed { run: task, attempt },
        ]
    );
}

#[test]
fn a_worker_holding_the_workstream_is_preferred_over_a_free_engine_slot() {
    let limits = Limits { workers: 1, engine_slots: 1, ..jig_fleet_world::LIMITS };
    let mut domain = jig_fleet_world::fixtures::domain(&limits, jig_fleet_world::fixtures::needs());
    let task = Token::new(9);
    let attempt = Token::new(10);
    assert!(
        step(
            &mut domain,
            limits,
            Event::Hello {
                worker: 0,
                channel: Token::new(11),
                hello: Hello {
                    bounds: jig_fleet_world::fixtures::bounds(),
                    slots: 1,
                    stop_bound: Duration::from_secs(1),
                    workstreams: Box::new([9]),
                    hosting: Box::new([]),
                }
            }
        )
        .is_empty()
    );
    assert!(
        step(
            &mut domain,
            limits,
            Event::Start {
                placement: None,
                version: jig_domain_fleet::Version { charter: 1, revision: 1 },
                grants: Box::new([]),
                reply_to: ReplyTo::new(attempt),
                run: task,
                attempt,
                workstream: 9,
                kinds: Kinds::Both,
                assignment: refs()
            }
        )
        .is_empty()
    );
    assert_eq!(
        place(&mut domain, limits),
        vec![
            Request::Assign {
                channel: Token::new(11),
                kind: HostKind::Worker,
                run: task,
                attempt,
                activation: attempt.raw(),
                assignment: refs()
            },
            Request::Placed { run: task, attempt },
        ]
    );
    assert_eq!(
        step(
            &mut domain,
            limits,
            Event::Answer {
                channel: Token::new(11),
                run: task,
                attempt,
                answer: Answer::Busy,
                payload: Token::new(12),
            }
        ),
        vec![Request::Drop { payload: Token::new(12) }]
    );
    assert_eq!(
        place(&mut domain, limits),
        vec![
            Request::Assign {
                channel: Token::new(0),
                kind: HostKind::Engine,
                run: task,
                attempt,
                activation: attempt.raw(),
                assignment: refs()
            },
            Request::Placed { run: task, attempt },
        ]
    );
}

fn refs() -> jig_domain_fleet::Assignment {
    jig_domain_fleet::Assignment { turns: Token::new(0), answered: Token::new(0) }
}

#[test]
fn a_worker_refused_for_its_agent_hosts_nothing() {
    let limits = jig_fleet_world::LIMITS;
    let mut domain = waiting_worker(limits);
    let bounds = fleet::Bounds { turns: 7, ..jig_fleet_world::fixtures::bounds() };
    assert_eq!(
        step(
            &mut domain,
            limits,
            Event::Hello {
                worker: 0,
                channel: Token::new(5),
                hello: Hello {
                    bounds,
                    slots: 1,
                    stop_bound: Duration::ZERO,
                    workstreams: Box::new([]),
                    hosting: Box::new([]),
                },
            }
        ),
        vec![Request::Refuse { channel: Token::new(5) }]
    );
    assert_eq!(
        domain.hello_refusals().as_slice()[0].why,
        fleet::HelloRefusal::Agent { why: fleet::AgentShortfall::Turns { charter: 1 } }
    );
    assert!(place(&mut domain, limits).is_empty());
    assert_eq!(domain.workers(), 0);
    assert_eq!(domain.waiting(), 1);
}

fn worker_needs() -> fleet::Needs {
    use fleet::{CharterNeeds, ModelNeed, Needs};
    Needs {
        charters: Box::new([CharterNeeds {
            version: jig_domain_fleet::Version { charter: 1, revision: 1 },
            kinds: Kinds::Workers,
            models: Box::new([ModelNeed { name: b"model".as_slice().into(), window: 8192, output: 1024 }]),
            turns: 8,
            time: Duration::from_secs(60),
            waiting: Duration::from_secs(10),
            change_reserve: None,
        }]),
        spend: 100,
        message_bytes: 64,
        start_messages: 4,
        unacknowledged: 2,
    }
}

fn waiting_worker(limits: Limits) -> Domain {
    let mut domain = jig_fleet_world::fixtures::domain(&limits, worker_needs());
    assert!(
        step(
            &mut domain,
            limits,
            Event::Start {
                placement: None,
                version: jig_domain_fleet::Version { charter: 1, revision: 1 },
                grants: Box::new([]),
                reply_to: ReplyTo::new(Token::new(2)),
                run: Token::new(1),
                attempt: Token::new(2),
                workstream: 1,
                kinds: Kinds::Workers,
                assignment: refs(),
            }
        )
        .is_empty()
    );
    domain
}

#[test]
fn startup_and_hello_check_only_the_charters_for_the_host_kind() {
    for (kinds, kind, applies) in [
        (Kinds::Workers, HostKind::Worker, true),
        (Kinds::Workers, HostKind::Engine, false),
        (Kinds::Engine, HostKind::Worker, false),
        (Kinds::Engine, HostKind::Engine, true),
        (Kinds::Both, HostKind::Worker, true),
        (Kinds::Both, HostKind::Engine, true),
    ] {
        let mut needs = worker_needs();
        needs.charters[0].kinds = kinds;
        let bounds = fleet::Bounds { turns: 7, ..jig_fleet_world::fixtures::bounds() };
        let expected = applies.then_some(fleet::AgentShortfall::Turns { charter: 1 });
        assert_eq!(fleet::shortfall(&needs, &bounds, kind), expected);
        let limits = Limits { engine_slots: 1, ..jig_fleet_world::LIMITS };
        let mut domain = jig_fleet_world::fixtures::domain(&limits, needs);
        let channel = match kind {
            HostKind::Engine => Token::new(0),
            HostKind::Worker => Token::new(5),
        };
        let requests = step(
            &mut domain,
            limits,
            Event::Hello {
                worker: 0,
                channel,
                hello: Hello {
                    bounds,
                    slots: 1,
                    stop_bound: Duration::ZERO,
                    workstreams: Box::new([]),
                    hosting: Box::new([]),
                },
            },
        );
        if let Some(why) = expected {
            assert_eq!(requests, vec![Request::Refuse { channel }]);
            assert_eq!(domain.hello_refusals().as_slice()[0].why, fleet::HelloRefusal::Agent { why });
        } else {
            assert!(requests.is_empty());
            assert!(domain.hello_refusals().is_empty());
        }
    }
}

#[test]
fn the_global_message_bound_applies_only_when_a_charter_does() {
    let needs = worker_needs();
    let adequate = fleet::Bounds { message_bytes: 64, ..jig_fleet_world::fixtures::bounds() };
    assert_eq!(fleet::shortfall(&needs, &adequate, HostKind::Worker), None);
    let short = fleet::Bounds { message_bytes: 63, ..adequate };
    assert_eq!(fleet::shortfall(&needs, &short, HostKind::Worker), Some(fleet::AgentShortfall::Message));
    assert_eq!(fleet::shortfall(&needs, &short, HostKind::Engine), None);
}
