use jig_domain_host as host;
use jig_protocol_worker_link::{Event, Request};
use jig_worker_link_world::{World, fixtures};
use skein_lib::{Duration, Reader};
fn world() -> World {
    open_world(&fixtures::limits())
}
fn open_world(limits: &jig_protocol_worker_link::Limits) -> World {
    let mut world = World::new(limits, 1);
    world.engine(fixtures::engine_accepting());
    world.run();
    world.down(Event::Hello { hello: fixtures::hello() });
    world
}
fn call(world: &mut World, name: &[u8]) {
    world.down(Event::Up { up: fixtures::call(name), parts: Box::new([]) });
}
fn cancel(world: &mut World, name: &[u8]) {
    world.down(Event::CancelRelay { run: fixtures::RUN_SEVEN, attempt: fixtures::ATTEMPT_TWO, call: name.into() });
}
fn redial(world: &mut World) {
    world.engine(fixtures::engine_accepting());
    world.advance(Duration::from_millis(200));
    world.down(Event::Hello { hello: fixtures::hello() });
}
fn kinds(world: &World, channel: usize) -> Vec<u16> {
    world.records()[channel].iter().map(|record| record.kind).collect()
}
fn decoded_call(record: &skein_fake_channel::Observed) -> jig_protocol_worker_wire::Call {
    jig_protocol_worker_wire::Call::decode(&fixtures::codec(), &mut Reader::new(&record.body)).expect("call")
}
fn answers(world: &World) -> Vec<&host::Relayed> {
    world
        .root()
        .iter()
        .filter_map(|request| match request {
            Request::Down { down: host::LinkDown::Relayed { answer, .. }, .. } => Some(answer),
            Request::Ready
            | Request::Resend { .. }
            | Request::GracePassed
            | Request::Closed
            | Request::Lost
            | Request::Down { .. }
            | Request::Value { .. }
            | Request::Dial
            | Request::Lower { .. }
            | Request::Close { .. }
            | Request::Abort { .. }
            | Request::Fact { .. }
            | Request::RelayCancelled { .. } => None,
        })
        .collect()
}
#[test]
fn a_relay_answered_on_a_later_channel_reaches_its_run_once() {
    let mut world = world();
    call(&mut world, b"first");
    world.lose_channel();
    redial(&mut world);
    world.play(fixtures::relayed(b"first", false));
    world.play(fixtures::relayed(b"first", false));
    cancel(&mut world, b"first");
    assert_eq!(answers(&world), [&host::Relayed::Answer { error: false, text: b"answer".as_slice().into() }]);
    assert!(!world.root().iter().any(|request| matches!(request, Request::RelayCancelled { .. })));
}
#[test]
fn a_relay_cancelled_while_the_link_is_down_is_never_sent_again() {
    let mut world = world();
    call(&mut world, b"first");
    world.lose_channel();
    cancel(&mut world, b"first");
    cancel(&mut world, b"first");
    redial(&mut world);
    assert_eq!(kinds(&world, 1), [0x0100]);
    let cancelled: Vec<_> = world
        .root()
        .iter()
        .filter_map(|request| match request {
            Request::RelayCancelled { run, attempt, call } => Some((*run, *attempt, call.as_ref())),
            Request::Ready
            | Request::Resend { .. }
            | Request::GracePassed
            | Request::Closed
            | Request::Lost
            | Request::Down { .. }
            | Request::Value { .. }
            | Request::Dial
            | Request::Lower { .. }
            | Request::Close { .. }
            | Request::Abort { .. }
            | Request::Fact { .. } => None,
        })
        .collect();
    assert_eq!(cancelled, [(fixtures::RUN_SEVEN, fixtures::ATTEMPT_TWO, b"first".as_slice())]);
}
#[test]
fn a_busy_call_goes_up_as_busy_and_ends_its_relay() {
    let mut world = world();
    call(&mut world, b"first");
    world.play(fixtures::relayed(b"first", true));
    assert_eq!(answers(&world), [&host::Relayed::Busy]);
    world.lose_channel();
    redial(&mut world);
    assert_eq!(kinds(&world, 1), [0x0100]);
}
#[test]
fn relays_go_again_after_the_hello_before_the_hubs_resends() {
    let mut limits = fixtures::limits();
    limits.sizes.calls = 3;
    let mut world = open_world(&limits);
    for name in [b"third".as_slice(), b"first", b"second"] {
        call(&mut world, name);
    }
    world.lose_channel();
    redial(&mut world);
    world.down(Event::Up { up: fixtures::turn(), parts: Box::new([]) });
    let channels = world.records();
    let records = &channels[1];
    assert_eq!(records.iter().map(|record| record.kind).collect::<Vec<_>>(), [0x0100, 0x0102, 0x0102, 0x0102, 0x0101]);
    for (record, name) in records[1..4].iter().zip([b"third".as_slice(), b"first", b"second"]) {
        let call = decoded_call(record);
        assert_eq!(call.name(), name);
    }
}
#[test]
fn an_answer_after_cancellation_never_reaches_the_hub() {
    let mut world = world();
    call(&mut world, b"first");
    cancel(&mut world, b"first");
    world.play(fixtures::relayed(b"first", false));
    assert!(answers(&world).is_empty());
}

#[test]
fn a_full_relay_replay_keeps_one_hello_steps_outputs_bounded() {
    let mut limits = fixtures::limits();
    limits.sizes.calls = 40;
    let mut world = open_world(&limits);
    for name in 0_u32..40 {
        call(&mut world, &name.to_be_bytes());
    }
    world.lose_channel();
    redial(&mut world);
    let channels = world.records();
    assert_eq!(channels[1].len(), 41);
    assert_eq!(channels[1][0].kind, 0x0100);
    for (record, name) in channels[1][1..].iter().zip(0_u32..40) {
        let call = decoded_call(record);
        assert_eq!(call.name(), name.to_be_bytes());
    }
}

#[test]
fn call_input_at_the_bound_crosses_and_one_more_byte_is_answered_locally() {
    for (input, sent) in [(16, true), (17, false)] {
        let mut limits = fixtures::limits();
        limits.sizes.call_input = 16;
        let mut world = open_world(&limits);
        let host::LinkUp::Call { run, attempt, call, tool, writes, deadline, .. } = fixtures::call(b"opaque") else {
            panic!("call")
        };
        world.down(Event::Up {
            up: host::LinkUp::Call {
                run,
                attempt,
                call,
                tool,
                writes,
                deadline,
                input: vec![b'x'; input].into_boxed_slice(),
            },
            parts: Box::new([]),
        });
        assert_eq!(kinds(&world, 0).contains(&0x0102), sent);
        if sent {
            assert!(answers(&world).is_empty());
        } else {
            let terminals: Vec<_> = world
                .root()
                .iter()
                .filter_map(|request| {
                    if let Request::Down { down: host::LinkDown::Relayed { run, attempt, call, answer }, parts } =
                        request
                    {
                        assert!(parts.is_empty());
                        Some((*run, *attempt, call.as_ref(), answer))
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(
                terminals,
                [(
                    fixtures::RUN_SEVEN,
                    fixtures::ATTEMPT_TWO,
                    b"opaque".as_slice(),
                    &host::Relayed::Answer { error: true, text: b"call too large".as_slice().into() }
                )]
            );
        }
        world.lose_channel();
        redial(&mut world);
        assert_eq!(kinds(&world, 1).contains(&0x0102), sent, "only accepted calls replay");
    }
}

#[test]
fn encoding_errors_are_bounded_and_local_refusal_has_one_terminal() {
    for (answer, expected) in [(16, b"call refused".as_slice()), (4, b"call"), (0, b"")] {
        let mut limits = fixtures::limits();
        limits.sizes.answer = answer;
        let mut world = open_world(&limits);
        let host::LinkUp::Call { run, attempt, call, writes, input, deadline, .. } = fixtures::call(b"opaque") else {
            panic!("call")
        };
        world.down(Event::Up {
            up: host::LinkUp::Call {
                run,
                attempt,
                call,
                writes,
                input,
                deadline,
                tool: vec![b'x'; usize::try_from(limits.sizes.tool_name + 1).expect("small tool bound")]
                    .into_boxed_slice(),
            },
            parts: Box::new([]),
        });
        assert_eq!(answers(&world), [&host::Relayed::Answer { error: true, text: expected.into() }]);
        cancel(&mut world, b"opaque");
        assert!(!world.root().iter().any(|request| matches!(request, Request::RelayCancelled { .. })));
        assert!(!kinds(&world, 0).contains(&0x0102));
        world.lose_channel();
        redial(&mut world);
        assert_eq!(kinds(&world, 1), [0x0100]);
    }
}

#[test]
fn calls_during_an_outage_are_refused_or_retained_before_the_next_hello() {
    for input in [16, 17] {
        let mut limits = fixtures::limits();
        limits.sizes.call_input = 16;
        let mut world = open_world(&limits);
        world.lose_channel();
        let host::LinkUp::Call { run, attempt, call, tool, writes, deadline, .. } = fixtures::call(b"outage") else {
            panic!("call")
        };
        world.down(Event::Up {
            up: host::LinkUp::Call {
                run,
                attempt,
                call,
                tool,
                writes,
                deadline,
                input: vec![b'x'; input].into_boxed_slice(),
            },
            parts: Box::new([]),
        });
        assert_eq!(answers(&world).len(), usize::from(input > 16));
        redial(&mut world);
        assert_eq!(kinds(&world, 1).contains(&0x0102), input == 16);
        if input == 16 {
            cancel(&mut world, b"outage");
            assert_eq!(
                world.root().iter().filter(|request| matches!(request, Request::RelayCancelled { .. })).count(),
                1
            );
            world.lose_channel();
            redial(&mut world);
            assert_eq!(kinds(&world, 2), [0x0100]);
        }
    }
}

#[test]
fn a_call_before_hello_replays_once_and_duplicate_cancel_has_no_second_terminal() {
    let mut world = World::new(&fixtures::limits(), 1);
    world.engine(fixtures::engine_accepting());
    world.run();
    call(&mut world, b"before-hello");
    assert!(!kinds(&world, 0).contains(&0x0102));
    world.down(Event::Hello { hello: fixtures::hello() });
    assert_eq!(kinds(&world, 0).iter().filter(|kind| **kind == 0x0102).count(), 1);
    cancel(&mut world, b"before-hello");
    cancel(&mut world, b"before-hello");
    assert_eq!(world.root().iter().filter(|request| matches!(request, Request::RelayCancelled { .. })).count(), 1);
    world.down(Event::Stop);
    world.advance(Duration::from_secs(30));
    let count = world.root().len();
    cancel(&mut world, b"before-hello");
    assert_eq!(world.root().len(), count, "the closed guard stays in force");
    jig_worker_link_world::referee::judge(&world).expect("one terminal and closed last");
}

#[test]
fn replay_capacity_is_retryable_without_losing_the_retained_call() {
    let mut limits = fixtures::limits();
    limits.sizes.calls = 1;
    let mut world = open_world(&limits);
    call(&mut world, b"retained");
    call(&mut world, b"retry");
    assert_eq!(answers(&world), [&host::Relayed::Busy]);
    world.lose_channel();
    redial(&mut world);
    let records = world.records();
    let replayed: Vec<_> = records[1].iter().filter(|record| record.kind == 0x0102).map(decoded_call).collect();
    assert_eq!(replayed.len(), 1);
    assert_eq!(replayed[0].name(), b"retained");
    cancel(&mut world, b"retained");
    call(&mut world, b"retry");
    assert_eq!(kinds(&world, 1).iter().filter(|kind| **kind == 0x0102).count(), 2);
}
