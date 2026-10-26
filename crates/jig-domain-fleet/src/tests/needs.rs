use super::*;
use crate::{AgentShortfall, Bounds, CharterNeeds, ModelBound, ModelNeed, Needs};

fn needs_window(window: u32) -> Needs {
    Needs {
        charters: Box::new([CharterNeeds {
            version: crate::Version { charter: 1, revision: 1 },
            kinds: Kinds::Workers,
            models: Box::new([ModelNeed { name: b"model".as_slice().into(), window, output: 1024 }]),
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
fn bounds_window(window: u32) -> Bounds {
    Bounds {
        program_endpoints: Box::new([]),
        models: Box::new([ModelBound { name: b"model".as_slice().into(), window, output: 1024 }]),
        turns: 8,
        spend: 100,
        time: Duration::from_secs(60),
        waiting: Duration::from_secs(10),
        checks: Duration::from_secs(5),
        message_bytes: 64,
        inbox: 4,
        unacknowledged: 2,
    }
}
fn hello(bounds: Bounds) -> Hello {
    Hello { stop_bound: Duration::from_nanos(1), slots: 2, workstreams: Box::new([]), hosting: Box::new([]), bounds }
}
fn rejected(needs: Needs, bounds: Bounds) -> HelloRefusal {
    let expected = crate::shortfall(&needs, &bounds, HostKind::Worker).expect("agent shortfall");
    let mut h = Harness::with_needs(LIMITS, needs);
    assert_eq!(
        &*h.step(Event::Hello { worker: 0, channel: C1, hello: hello(bounds) }),
        &[Request::Refuse { channel: C1 }]
    );
    let actual = refusal(&h);
    assert_eq!(actual, HelloRefusal::Agent { why: expected });
    actual
}
#[test]
fn a_worker_whose_model_window_is_smaller_is_refused_naming_it() {
    assert_eq!(
        rejected(needs_window(8192), bounds_window(4096)),
        HelloRefusal::Agent { why: AgentShortfall::Window { charter: 1, model: 0 } }
    );
}
#[test]
fn a_worker_lacking_a_charters_model_is_admitted() {
    let mut bounds = bounds_window(8192);
    bounds.models = Box::new([]);
    let mut h = Harness::with_needs(LIMITS, needs_window(8192));
    assert!(h.step(Event::Hello { worker: 0, channel: C1, hello: hello(bounds) }).is_empty());
    assert_eq!(h.domain.workers(), 1);
}
#[test]
fn a_worker_whose_inbox_holds_fewer_than_a_start_is_refused() {
    let mut bounds = bounds_window(8192);
    bounds.inbox = 3;
    assert_eq!(rejected(needs_window(8192), bounds), HelloRefusal::Agent { why: AgentShortfall::Inbox });
}
#[test]
fn a_worker_whose_checks_outlast_a_change_reserve_is_refused() {
    let mut needs = needs_window(8192);
    needs.charters[0].change_reserve = Some(Duration::from_secs(4));
    assert_eq!(
        rejected(needs, bounds_window(8192)),
        HelloRefusal::Agent { why: AgentShortfall::Checks { charter: 1 } }
    );
}
#[test]
fn a_charter_for_the_engine_only_needs_nothing_of_workers() {
    let mut needs = needs_window(8192);
    needs.charters[0].kinds = Kinds::Engine;
    let mut h = Harness::with_needs(LIMITS, needs);
    let mut bounds = bounds_window(0);
    bounds.models = Box::new([]);
    assert!(h.step(Event::Hello { worker: 0, channel: C1, hello: hello(bounds) }).is_empty());
    assert_eq!(h.domain.workers(), 1);
}
#[test]
fn a_worker_holding_every_charter_is_admitted() {
    let mut h = Harness::with_needs(LIMITS, needs_window(8192));
    assert!(h.step(Event::Hello { worker: 0, channel: C1, hello: hello(bounds_window(8192)) }).is_empty());
    assert_eq!(h.domain.workers(), 1);
}
#[test]
fn the_engines_first_hello_is_checked_against_its_charters() {
    let mut needs = needs_window(8192);
    needs.charters[0].kinds = Kinds::Engine;
    let mut h = Harness::with_needs(LIMITS, needs);
    assert_eq!(
        &*h.step(Event::Hello { worker: 0, channel: Token::new(0), hello: hello(bounds_window(4096)) }),
        &[Request::Refuse { channel: Token::new(0) }]
    );
    assert_eq!(refusal(&h), HelloRefusal::Agent { why: AgentShortfall::Window { charter: 1, model: 0 } });
}
#[test]
fn a_second_hello_on_the_engine_channel_is_refused() {
    let mut h = Harness::new(LIMITS);
    assert!(h.step(Event::Hello { worker: 0, channel: Token::new(0), hello: hello(bounds_window(8192)) }).is_empty());
    assert_eq!(
        &*h.step(Event::Hello { worker: 0, channel: Token::new(0), hello: hello(bounds_window(8192)) }),
        &[Request::Refuse { channel: Token::new(0) }]
    );
    assert_eq!(refusal(&h), HelloRefusal::ReservedChannel);
}

fn refusal(h: &Harness) -> HelloRefusal {
    h.domain.hello_refusals().get(0).unwrap().why
}

#[test]
#[should_panic(expected = "configured agent needs fit their byte bound")]
fn needs_past_their_byte_bound_fail_construction() {
    core::mem::drop(Domain::new(&Limits { needs_bytes: 0, ..LIMITS }, needs_window(8192), Box::new([])));
}

#[test]
fn a_worker_whose_model_output_is_smaller_is_refused() {
    let mut bounds = bounds_window(8192);
    bounds.models[0].output = 1023;
    assert_eq!(
        rejected(needs_window(8192), bounds),
        HelloRefusal::Agent { why: AgentShortfall::Output { charter: 1, model: 0 } }
    );
}
#[test]
fn a_worker_with_fewer_turns_than_a_charter_is_refused() {
    let mut bounds = bounds_window(8192);
    bounds.turns = 7;
    assert_eq!(rejected(needs_window(8192), bounds), HelloRefusal::Agent { why: AgentShortfall::Turns { charter: 1 } });
}
#[test]
fn a_worker_with_less_spend_than_the_deployment_is_refused() {
    let mut bounds = bounds_window(8192);
    bounds.spend = 99;
    assert_eq!(rejected(needs_window(8192), bounds), HelloRefusal::Agent { why: AgentShortfall::Spend });
}
#[test]
fn a_worker_with_less_time_than_a_charter_is_refused() {
    let mut bounds = bounds_window(8192);
    bounds.time = Duration::from_secs(59);
    assert_eq!(rejected(needs_window(8192), bounds), HelloRefusal::Agent { why: AgentShortfall::Time { charter: 1 } });
}
#[test]
fn a_worker_with_less_waiting_than_a_charter_is_refused() {
    let mut bounds = bounds_window(8192);
    bounds.waiting = Duration::from_secs(9);
    assert_eq!(
        rejected(needs_window(8192), bounds),
        HelloRefusal::Agent { why: AgentShortfall::Waiting { charter: 1 } }
    );
}
#[test]
fn a_worker_with_a_smaller_message_bound_is_refused() {
    let mut bounds = bounds_window(8192);
    bounds.message_bytes = 63;
    assert_eq!(rejected(needs_window(8192), bounds), HelloRefusal::Agent { why: AgentShortfall::Message });
}
#[test]
fn a_worker_keeping_fewer_unacknowledged_turns_is_refused() {
    let mut bounds = bounds_window(8192);
    bounds.unacknowledged = 1;
    assert_eq!(rejected(needs_window(8192), bounds), HelloRefusal::Agent { why: AgentShortfall::Unacknowledged });
}
#[test]
fn a_smaller_alternative_names_its_position() {
    let mut needs = needs_window(8192);
    needs.charters[0].models = Box::new([
        needs.charters[0].models[0].clone(),
        ModelNeed { name: b"alternative".as_slice().into(), window: 8192, output: 1024 },
    ]);
    let mut bounds = bounds_window(8192);
    bounds.models = Box::new([
        bounds.models[0].clone(),
        ModelBound { name: b"alternative".as_slice().into(), window: 4096, output: 1024 },
    ]);
    assert_eq!(rejected(needs, bounds), HelloRefusal::Agent { why: AgentShortfall::Window { charter: 1, model: 1 } });
}

#[test]
fn each_per_charter_shortfall_names_the_second_applicable_charter() {
    let mut needs = needs_window(8192);
    let mut first = needs.charters[0].clone();
    first.version.charter = 17;
    first.turns = 7;
    first.time = Duration::from_secs(59);
    first.waiting = Duration::from_secs(9);
    first.change_reserve = Some(Duration::from_secs(6));
    let mut second = needs.charters[0].clone();
    second.version.charter = 73;
    second.change_reserve = Some(Duration::from_secs(5));
    let mut unrelated = second.clone();
    unrelated.version.charter = 99;
    unrelated.kinds = Kinds::Engine;
    unrelated.turns = 100;
    needs.charters = Box::new([unrelated, first, second]);

    let adequate = bounds_window(8192);
    assert_eq!(crate::shortfall(&needs, &adequate, HostKind::Worker), None);
    let mut h = Harness::with_needs(LIMITS, needs.clone());
    assert!(h.step(Event::Hello { worker: 0, channel: C1, hello: hello(adequate.clone()) }).is_empty());
    assert_eq!(h.domain.workers(), 1);
    for (bounds, expected) in [
        (Bounds { turns: 7, ..adequate.clone() }, AgentShortfall::Turns { charter: 73 }),
        (Bounds { time: Duration::from_secs(59), ..adequate.clone() }, AgentShortfall::Time { charter: 73 }),
        (Bounds { waiting: Duration::from_secs(9), ..adequate.clone() }, AgentShortfall::Waiting { charter: 73 }),
        (Bounds { checks: Duration::from_secs(6), ..adequate.clone() }, AgentShortfall::Checks { charter: 73 }),
    ] {
        assert_eq!(rejected(needs.clone(), bounds), HelloRefusal::Agent { why: expected });
    }
    // The canonical decision remains first-shortfall only when several bounds fail.
    let several = Bounds {
        turns: 7,
        time: Duration::from_secs(59),
        waiting: Duration::from_secs(9),
        checks: Duration::from_secs(6),
        message_bytes: 63,
        ..adequate
    };
    assert_eq!(rejected(needs, several), HelloRefusal::Agent { why: AgentShortfall::Turns { charter: 73 } });
}

#[test]
fn endpoint_evidence_is_admitted_once_for_workers_and_engine() {
    for channel in [C1, Token::new(0)] {
        let mut limits = LIMITS;
        limits.engine_slots = 1;
        let mut h = Harness::with_needs(limits, needs_window(8192));
        assert_eq!(h.domain.program_endpoint(channel, 7), None);
        let mut bounds = bounds_window(8192);
        bounds.program_endpoints = endpoint_numbers(crate::PROGRAM_ENDPOINTS);
        h.step(Event::Hello { worker: 0, channel, hello: hello(bounds) });
        assert_eq!(h.domain.program_endpoint(channel, 7), Some(true));
        assert_eq!(h.domain.program_endpoint(channel, crate::PROGRAM_ENDPOINTS), Some(false));
        h.step(Event::Hello { worker: 0, channel, hello: hello(bounds_window(8192)) });
        assert_eq!(h.domain.program_endpoint(channel, 7), Some(true));
    }
}

#[test]
fn ambiguous_or_oversized_endpoint_metadata_is_never_retained() {
    let repeated: Box<[u32]> = Box::new([7, 7]);
    for endpoints in [repeated, endpoint_numbers(crate::PROGRAM_ENDPOINTS + 1)] {
        let mut h = Harness::with_needs(LIMITS, needs_window(8192));
        let mut bounds = bounds_window(8192);
        bounds.program_endpoints = endpoints;
        assert_eq!(
            &*h.step(Event::Hello { worker: 0, channel: C1, hello: hello(bounds) }),
            &[Request::Refuse { channel: C1 }]
        );
        assert_eq!(h.domain.program_endpoint(C1, 7), None);
        assert_eq!(h.domain.workers(), 0);
    }
}

fn endpoint_numbers(count: u32) -> Box<[u32]> {
    let mut endpoints = List::with_capacity(count);
    for endpoint in 0..count {
        endpoints.push(endpoint).expect("bounded test endpoints");
    }
    endpoints.into_boxed()
}

#[test]
fn replacing_needs_keeps_running_attempts_and_rebuilds_placement_from_the_catalogue() {
    let catalogue = Box::new([b"model".as_slice().into(), b"future".as_slice().into()]);
    let mut h = Harness::with_catalogue(LIMITS, needs_window(4096), catalogue);
    let mut bounds = bounds_window(8192);
    bounds.models = Box::new([
        bounds.models[0].clone(),
        ModelBound { name: b"future".as_slice().into(), window: 8192, output: 1024 },
        ModelBound { name: b"unconfigured".as_slice().into(), window: 8192, output: 1024 },
    ]);
    assert!(h.step(Event::Hello { worker: 0, channel: C1, hello: hello(bounds) }).is_empty());
    assert_eq!(h.place(R1, A1, 1), C1);
    let before = h.domain.attempts.len();
    assert!(h.step(Event::Needs(needs_window(16384))).is_empty());
    assert_eq!(h.domain.workers(), 1);
    assert_eq!(h.domain.attempts.len(), before);
    let id = *h.domain.tokens.get(&C1).unwrap();
    assert_eq!(h.domain.channels.get(id).unwrap().evidence.models.len(), 2, "only catalogue evidence retained");
    assert!(!h.domain.channels.get(id).unwrap().eligible[0]);
    assert!(h.start(R2, A2, 2).is_empty());
    assert!(h.settle().is_empty(), "new work waits while needs exceed the hello");
    let mut needs = needs_window(4096);
    needs.charters[0].models[0].name = b"future".as_slice().into();
    assert!(h.step(Event::Needs(needs)).is_empty());
    assert!(
        h.domain.channels.get(id).unwrap().eligible[0],
        "evidence for a previously unused catalogue model survives"
    );
    assert!(!h.settle().is_empty(), "the waiting attempt can now be placed");
}

fn versioned_needs() -> Needs {
    let mut needs = needs_window(8192);
    let mut old = needs.charters[0].clone();
    old.kinds = Kinds::Engine;
    let mut newest = old.clone();
    newest.version.revision = 2;
    newest.kinds = Kinds::Workers;
    needs.charters = Box::new([old, newest]);
    needs
}

fn start_version(run: Token, attempt: Token, revision: u32) -> Event {
    Event::Start {
        reply_to: to(attempt),
        run,
        attempt,
        workstream: run.raw(),
        kinds: Kinds::Both,
        version: crate::Version { charter: 1, revision },
        placement: None,
        assignment: references(),
        grants: Box::new([]),
    }
}

#[test]
fn a_run_of_an_older_version_is_placed_by_its_own_needs() {
    let limits = Limits { engine_slots: 1, versions: 1, ..LIMITS };
    let mut h = Harness::with_needs(limits, versioned_needs());
    let engine = Token::new(0);
    assert!(h.step(Event::Hello { worker: 0, channel: engine, hello: hello(bounds_window(8192)) }).is_empty());
    assert!(h.step(Event::Hello { worker: 0, channel: C1, hello: hello(bounds_window(8192)) }).is_empty());
    for (run, attempt, revision, channel, kind) in
        [(R1, A1, 1, engine, HostKind::Engine), (R2, A2, 2, C1, HostKind::Worker)]
    {
        let version = crate::Version { charter: 1, revision };
        assert_eq!(
            h.domain.probe(Kinds::Both, 1, version, &[], &[]),
            Some(crate::PlacementConstraint { channel, kind })
        );
        assert!(h.step(start_version(run, attempt, revision)).is_empty());
        let placed = h.settle();
        let [Request::Assign { channel: actual, run: assigned, .. }, Request::Placed { .. }] = &placed[..] else {
            panic!("placed: {placed:?}");
        };
        assert_eq!((*actual, *assigned), (channel, run));
    }
}

#[test]
fn a_dropped_version_leaves_the_table_and_live_attempts_keep_theirs() {
    let limits = Limits { engine_slots: 1, versions: 1, ..LIMITS };
    let mut h = Harness::with_needs(limits, versioned_needs());
    let engine = Token::new(0);
    h.step(Event::Hello { worker: 0, channel: engine, hello: hello(bounds_window(8192)) });
    h.step(start_version(R1, A1, 1));
    assert!(!h.settle().is_empty());
    let mut needs = versioned_needs();
    needs.charters = Box::new([needs.charters[1].clone()]);
    assert!(h.step(Event::Needs(needs)).is_empty());
    assert_eq!(h.domain.needs.charters.len(), 1);
    assert_eq!(h.domain.needs.charters[0].version, crate::Version { charter: 1, revision: 2 });
    let id = h.domain.names.get(&(R1, A1)).expect("live attempt");
    let crate::attempt::State::Claimed { version, .. } = &h.domain.attempts.get(*id).unwrap().state else {
        panic!("live attempt stays claimed");
    };
    assert_eq!(*version, crate::Version { charter: 1, revision: 1 });
    h.step(Event::Hello { worker: 0, channel: C1, hello: hello(bounds_window(8192)) });
    h.step(start_version(R2, A2, 2));
    let placed = h.settle();
    let [Request::Assign { channel, .. }, Request::Placed { .. }] = &placed[..] else {
        panic!("placed: {placed:?}");
    };
    assert_eq!(*channel, C1);
}

#[test]
fn probes_check_the_selected_versions_turn_time_and_waiting_bounds() {
    let mut old = needs_window(8192).charters[0].clone();
    let mut newest = old.clone();
    newest.version.revision = 2;
    for dimension in 0_u32..3 {
        old.turns = 8;
        old.time = Duration::from_secs(60);
        old.waiting = Duration::from_secs(10);
        let mut h = Harness::with_needs(LIMITS, Needs { charters: Box::new([old.clone()]), ..needs_window(8192) });
        h.step(Event::Hello { worker: 0, channel: C1, hello: hello(bounds_window(8192)) });
        match dimension {
            0 => newest.turns = 9,
            1 => newest.time = Duration::from_secs(61),
            2 => newest.waiting = Duration::from_secs(11),
            _ => unreachable!(),
        }
        h.step(Event::Needs(Needs { charters: Box::new([old.clone(), newest.clone()]), ..needs_window(8192) }));
        assert!(h.domain.probe(Kinds::Workers, 1, old.version, &[], &[]).is_some());
        assert!(h.domain.probe(Kinds::Workers, 1, newest.version, &[], &[]).is_none());
        newest = old.clone();
        newest.version.revision = 2;
    }
}

#[test]
fn a_queued_old_version_keeps_its_models_when_the_newest_changes() {
    let mut old = needs_window(8192).charters[0].clone();
    old.models[0].name = b"old".as_slice().into();
    let needs = Needs { charters: Box::new([old.clone()]), ..needs_window(8192) };
    let mut h = Harness::with_catalogue(LIMITS, needs, Box::new([b"old".as_slice().into(), b"new".as_slice().into()]));
    h.step(start_version(R1, A1, 1));
    assert!(h.settle().is_empty());
    let mut newest = old.clone();
    newest.version.revision = 2;
    newest.models[0].name = b"new".as_slice().into();
    h.step(Event::Needs(Needs { charters: Box::new([old, newest]), ..needs_window(8192) }));
    let mut bounds = bounds_window(8192);
    bounds.models[0].name = b"old".as_slice().into();
    h.step(Event::Hello { worker: 0, channel: C1, hello: hello(bounds) });
    let placed = h.settle();
    let [Request::Assign { channel, run, .. }, Request::Placed { .. }] = &placed[..] else {
        panic!("placed: {placed:?}");
    };
    assert_eq!((*channel, *run), (C1, R1));
    h.step(start_version(R2, A2, 2));
    assert!(h.settle().is_empty(), "the old host cannot run the new version");
}
