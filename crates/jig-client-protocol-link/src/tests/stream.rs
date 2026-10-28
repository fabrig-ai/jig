use super::*;
#[test]
fn the_hello_s_jig_range_reaches_the_domain() {
    let harness = fixtures::Harness::streaming(fixtures::frame_subject());
    assert!(harness.has(&Request::Hello { family: None, versions: Some(c::VersionRange { first: 19, last: 19 }) }));
}
#[test]
fn the_hello_s_family_range_reaches_its_owner() {
    let harness = fixtures::Harness::streaming(fixtures::family_subject());
    assert!(harness.has(&Request::Hello { family: Some(19), versions: Some(c::VersionRange { first: 23, last: 23 }) }));
}

#[test]
fn a_hello_reopens_no_watch_of_an_owner_it_does_not_serve() {
    let mut harness = fixtures::Harness::new(&fixtures::limits());
    harness.open(fixtures::WATCH, fixtures::family_subject());
    harness.clear();
    let bytes = fixtures::hello_event(fixtures::HEARTBEAT);
    let w::StreamEvent::Hello(hello) = w::StreamEvent::decode(&w::CEILINGS, &mut Reader::new(&bytes)).unwrap() else {
        panic!("fixture hello");
    };
    let mut hello = hello.into_parts();
    hello.served.clear();
    hello
        .served
        .push(w::Served::new(&w::CEILINGS, w::ServedParts { owner: w::Owner::Jig, first: 19, last: 19 }).unwrap())
        .unwrap();
    harness.stream_record(&fixtures::event_bytes(&w::StreamEvent::Hello(w::Hello::new(&w::CEILINGS, hello).unwrap())));
    assert!(harness.has(&Request::Hello { family: Some(19), versions: None }));
    for request in &harness.out {
        if let Request::Exchange { .. } = request {
            panic!("no opening exchange outside its owner range");
        }
    }
    harness.link.step(Time::ZERO, Event::Close { watch: fixtures::WATCH }, &mut harness.out);
    assert_eq!(harness.ended(fixtures::WATCH), Some(c::WatchEnd::Closed));
    for request in &harness.out {
        if let Request::Exchange { .. } = request {
            panic!("no opening exchange outside its owner range");
        }
    }
}
#[test]
fn a_delivery_before_its_opening_s_answer_admits_it() {
    let mut harness = fixtures::Harness::streaming(fixtures::family_subject());
    let (exchange, name) = harness.opening();
    harness.stream_record(&fixtures::family_delivery(name));
    harness.answer(exchange, &fixtures::open_answer(&w::OpenAnswer::Admitted));
    assert!(harness.has(&Request::Watch { watch: fixtures::WATCH, event: c::WatchEvent::Opened }));
    harness.expected_family();
}
#[test]
fn silence_past_twice_the_heartbeat_drops_each_watch() {
    let mut harness = fixtures::Harness::new(&fixtures::limits());
    let watch = harness.open(fixtures::WATCH, fixtures::frame_subject());
    harness.stream_record(&fixtures::hello_event(fixtures::HEARTBEAT));
    harness.advance(fixtures::past_two_heartbeats());
    assert_eq!(harness.ended(watch), Some(c::WatchEnd::Dropped));
}
#[test]
fn a_second_watch_uses_the_stream_already_opened() {
    let mut harness = fixtures::Harness::streaming(fixtures::frame_subject());
    harness.clear();
    harness.open(Token::new(42), fixtures::family_subject());
    assert!(!harness.has(&Request::OpenStream { credential: None }));
}
#[test]
fn an_opening_waits_for_the_stream_s_name() {
    let mut harness = fixtures::Harness::new(&fixtures::limits());
    harness.open(fixtures::WATCH, fixtures::frame_subject());
    assert_eq!(harness.out.len(), 1);
    assert!(harness.has(&Request::OpenStream { credential: None }));
}
#[test]
fn a_watch_s_opening_answer_is_handed_up() {
    let mut harness = fixtures::Harness::streaming(fixtures::frame_subject());
    let (exchange, _) = harness.opening();
    harness.answer(exchange, &fixtures::open_answer(&w::OpenAnswer::Admitted));
    assert!(harness.has(&Request::Watch { watch: fixtures::WATCH, event: c::WatchEvent::Opened }));
}
#[test]
fn each_watch_refusal_is_handed_up() {
    for (why, expected) in [
        (w::WatchRefusal::Unknown, c::WatchEnd::Unknown),
        (w::WatchRefusal::NotAllowed, c::WatchEnd::NotAllowed),
        (w::WatchRefusal::Busy, c::WatchEnd::Busy),
        (w::WatchRefusal::SignedOut, c::WatchEnd::SignedOut),
    ] {
        let mut harness = fixtures::Harness::streaming(fixtures::frame_subject());
        let (exchange, _) = harness.opening();
        let value = w::OpenAnswer::Refused(
            w::OpenAnswerRefused::new(&fixtures::limits().wire, w::OpenAnswerRefusedParts { why }).unwrap(),
        );
        harness.answer(exchange, &fixtures::open_answer(&value));
        assert_eq!(harness.ended(fixtures::WATCH), Some(expected));
    }
}
#[test]
fn a_close_s_answer_and_stream_ending_yield_one_terminal() {
    let mut harness = fixtures::Harness::streaming(fixtures::frame_subject());
    let (_, name) = harness.opening();
    let close = harness.close();
    harness.answer(close, &fixtures::close_answer());
    harness.stream_record(&fixtures::ending(name, w::WatchEnd::Closed));
    assert_eq!(harness.ended(fixtures::WATCH), Some(c::WatchEnd::Closed));
    assert_eq!(harness.closed_count(), 1);
}
#[test]
fn a_stream_ending_before_the_close_answer_yields_one_terminal() {
    let mut harness = fixtures::Harness::streaming(fixtures::frame_subject());
    let (_, name) = harness.opening();
    let close = harness.close();
    harness.stream_record(&fixtures::ending(name, w::WatchEnd::Closed));
    harness.answer(close, &fixtures::close_answer());
    assert_eq!(harness.closed_count(), 1);
}
#[test]
fn a_heartbeat_keeps_each_watch_alive() {
    let mut harness = fixtures::Harness::streaming(fixtures::frame_subject());
    harness.clear();
    harness.link.step(Time::from_nanos(150), Event::Streamed { message: Streamed::Heartbeat }, &mut harness.out);
    harness.advance(fixtures::past_two_heartbeats());
    assert_eq!(harness.ended(fixtures::WATCH), None);
    assert!(harness.has(&Request::Watch { watch: fixtures::WATCH, event: c::WatchEvent::Alive }));
}
#[test]
fn the_stream_is_live_at_exactly_twice_the_heartbeat() {
    let mut harness = fixtures::Harness::streaming(fixtures::frame_subject());
    harness.advance(Time::from_nanos(200));
    assert_eq!(harness.ended(fixtures::WATCH), None);
}
#[test]
fn a_duplicate_hello_drops_the_stream() {
    let mut harness = fixtures::Harness::streaming(fixtures::frame_subject());
    harness.stream_record(&fixtures::hello_event(fixtures::HEARTBEAT));
    assert_eq!(harness.ended(fixtures::WATCH), Some(c::WatchEnd::Dropped));
}
#[test]
fn a_delivery_before_the_hello_drops_the_stream() {
    let mut harness = fixtures::Harness::new(&fixtures::limits());
    harness.open(fixtures::WATCH, fixtures::family_subject());
    harness.stream_record(&fixtures::family_delivery(1));
    assert_eq!(harness.ended(fixtures::WATCH), Some(c::WatchEnd::Dropped));
}
#[test]
fn a_new_opening_never_takes_the_old_wire_name() {
    let mut harness = fixtures::Harness::streaming(fixtures::family_subject());
    let (_, old) = harness.opening();
    harness.stream_record(&fixtures::ending(old, w::WatchEnd::Closed));
    harness.clear();
    harness.open(fixtures::WATCH, fixtures::family_subject());
    let (_, new) = harness.opening();
    assert_ne!(old, new);
    harness.stream_record(&fixtures::family_delivery(old));
    assert!(!harness.has(&Request::FamilyDelivery { watch: fixtures::WATCH, record: fixtures::family_read() }));
}
#[test]
fn a_stream_end_drops_all_watches_once() {
    let mut harness = fixtures::Harness::streaming(fixtures::frame_subject());
    harness.open(Token::new(42), fixtures::family_subject());
    harness.link.step(Time::ZERO, Event::StreamGone, &mut harness.out);
    harness.link.step(Time::ZERO, Event::StreamGone, &mut harness.out);
    assert_eq!(harness.ended(fixtures::WATCH), Some(c::WatchEnd::Dropped));
    assert_eq!(harness.ended(Token::new(42)), Some(c::WatchEnd::Dropped));
    assert_eq!(harness.closed_count(), 2);
}
#[test]
fn a_stream_refused_signed_out_ends_the_sign_in() {
    let mut harness = fixtures::Harness::new(&fixtures::limits());
    harness.open(fixtures::WATCH, fixtures::family_subject());
    harness.link.step(Time::ZERO, Event::Streamed { message: Streamed::SignedOut }, &mut harness.out);
    assert_eq!(harness.ended(fixtures::WATCH), Some(c::WatchEnd::SignedOut));
    assert!(harness.has(&Request::Signing { signing: Signing::SignedOut }));
    assert!(harness.has(&Request::CloseStream));
    assert!(!harness.has(&Request::Fact { fact: Fact::StreamDropped }));
}
#[test]
fn a_stream_drop_tells_the_engine_which_stream_is_gone() {
    let mut harness = fixtures::Harness::streaming(fixtures::frame_subject());
    harness.link.step(Time::ZERO, Event::StreamGone, &mut harness.out);
    assert_eq!(harness.gone_name(), fixtures::STREAM);
}
#[test]
fn a_close_before_the_hello_ends_without_an_exchange() {
    let mut harness = fixtures::Harness::new(&fixtures::limits());
    harness.open(fixtures::WATCH, fixtures::frame_subject());
    harness.link.step(Time::ZERO, Event::Close { watch: fixtures::WATCH }, &mut harness.out);
    harness.stream_record(&fixtures::hello_event(fixtures::HEARTBEAT));
    assert_eq!(harness.closed_count(), 1);
    assert_eq!(harness.ended(fixtures::WATCH), Some(c::WatchEnd::Closed));
}
#[test]
fn a_continued_watch_has_a_fresh_name_and_retains_its_run_offset() {
    let mut harness = fixtures::Harness::streaming(fixtures::frame_subject());
    let (_, old) = harness.opening();
    harness.clear();
    harness.link.step(
        Time::ZERO,
        Event::Continue {
            watch: fixtures::WATCH,
            subject: c::Subject::Run {
                task: 7,
                turn: Some(c::ProvisionalTurnName { attempt: 9, number: 2, retry: 3 }),
                offset: 123,
            },
        },
        &mut harness.out,
    );
    let (_, new) = harness.opening();
    assert_ne!(old, new);
    harness.assert_run_subject();
    harness.stream_record(&fixtures::ending(old, w::WatchEnd::Closed));
    assert_eq!(harness.ended(fixtures::WATCH), None);
}
#[test]
fn a_lost_opening_drops_its_watch() {
    let mut harness = fixtures::Harness::streaming(fixtures::frame_subject());
    let (exchange, _) = harness.opening();
    harness.link.step(Time::ZERO, Event::Lost { exchange }, &mut harness.out);
    harness.answer(exchange, &fixtures::open_answer(&w::OpenAnswer::Admitted));
    assert_eq!(harness.closed_count(), 1);
    assert_eq!(harness.ended(fixtures::WATCH), Some(c::WatchEnd::Dropped));
}
#[test]
fn a_delivery_whose_owner_differs_drops_the_stream() {
    let mut harness = fixtures::Harness::streaming(fixtures::frame_subject());
    let (_, name) = harness.opening();
    harness.stream_record(&fixtures::family_delivery(name));
    assert_eq!(harness.ended(fixtures::WATCH), Some(c::WatchEnd::Dropped));
    assert!(harness.has(&Request::Fact { fact: Fact::UndecodableEvent }));
}
#[test]
fn a_run_s_text_is_handed_up_with_its_offsets() {
    let mut harness = fixtures::Harness::streaming(fixtures::frame_subject());
    let (_, name) = harness.opening();
    harness.stream_record(&fixtures::run_event(name));
    assert_eq!(harness.delivered(), Some(&c::Delivery::Run(Box::new(fixtures::run_text_expected()))));
}
#[test]
fn each_jig_delivery_reaches_the_domain_with_its_variant() {
    for (record, expected) in fixtures::delivery_cases().as_ref() {
        let bytes = fixtures::encoded_delivery(record);
        assert_eq!(translate::watches::delivery(&bytes, &fixtures::limits()), Ok(expected.clone()));
    }
}
#[test]
fn a_jig_delivery_before_the_opening_answer_admits_it() {
    let mut harness = fixtures::Harness::streaming(fixtures::frame_subject());
    let (exchange, name) = harness.opening();
    harness.stream_record(&fixtures::jig_delivery(&w::DeliveryValue::Me(fixtures::frame_record()), name));
    harness.answer(exchange, &fixtures::open_answer(&w::OpenAnswer::Admitted));
    assert_eq!(harness.delivered(), Some(&c::Delivery::Me(Box::new(fixtures::frame()))));
}
#[test]
fn a_delivery_over_the_original_document_bound_is_refused() {
    let value = w::DeliveryValue::Me(fixtures::frame_record());
    let bytes = fixtures::encoded_delivery(&value);
    let limits = Limits::new(fixtures::limits().client, 4).unwrap();
    let mut client = limits.client;
    client.document_bytes = u64::from(value.measure()).checked_sub(1).unwrap();
    let limits = Limits::new(client, 4).unwrap();
    assert_eq!(translate::watches::delivery(&bytes, &limits), Err(Undecodable::TooLarge));
}
#[test]
fn a_delivery_with_mutated_original_limits_is_refused() {
    let mut limits = fixtures::limits();
    limits.wire.run_text_text = 1;
    let bytes = fixtures::encoded_delivery(&w::DeliveryValue::Run(fixtures::run_text_record()));
    assert_eq!(translate::watches::delivery(&bytes, &limits), Err(Undecodable::TooLarge));
}
#[test]
fn an_owner_inbox_delivery_preserves_both_groups_and_its_next_position() {
    let bytes = fixtures::encoded_delivery(&fixtures::owner_inbox_with_rows(1));
    assert_eq!(
        translate::watches::delivery(&bytes, &fixtures::limits()),
        Ok(fixtures::owner_inbox_with_rows_expected())
    );
}
#[test]
fn an_owner_tree_delivery_preserves_its_whole_descendant() {
    let bytes = fixtures::encoded_delivery(&fixtures::owner_tree_with_rows(1));
    assert_eq!(
        translate::watches::delivery(&bytes, &fixtures::limits()),
        Ok(fixtures::owner_tree_with_rows_expected())
    );
}
#[test]
fn a_delivery_piece_that_does_not_split_is_refused() {
    for record in &fixtures::malformed_delivery_pieces() {
        let bytes = fixtures::encoded_delivery(record);
        assert_eq!(translate::watches::delivery(&bytes, &fixtures::limits()), Err(Undecodable::Malformed));
    }
}
#[test]
fn an_owner_inbox_delivery_one_row_over_the_limit_is_refused() {
    let bytes = fixtures::encoded_delivery(&fixtures::owner_inbox_with_rows(3));
    assert_eq!(translate::watches::delivery(&bytes, &fixtures::limits()), Err(Undecodable::TooLarge));
}
#[test]
fn an_owner_tree_delivery_one_row_over_the_limit_is_refused() {
    let bytes = fixtures::encoded_delivery(&fixtures::owner_tree_with_rows(3));
    assert_eq!(translate::watches::delivery(&bytes, &fixtures::limits()), Err(Undecodable::TooLarge));
}

#[test]
fn a_malformed_stream_record_drops_the_stream_and_is_reported() {
    let mut harness = fixtures::Harness::streaming(fixtures::frame_subject());
    harness.stream_record(&[255]);
    assert_eq!(harness.ended(fixtures::WATCH), Some(c::WatchEnd::Dropped));
    assert!(harness.has(&Request::Fact { fact: Fact::UndecodableEvent }));
    assert!(harness.has(&Request::CloseStream));
}

#[test]
fn a_watch_ended_not_served_ends_its_handle_naming_why() {
    let mut h = fixtures::Harness::streaming(fixtures::family_subject());
    let (_, name) = h.opening();
    h.stream_record(&fixtures::ending(name, w::WatchEnd::NotServed(fixtures::not_served())));
    assert_eq!(h.ended(fixtures::WATCH), Some(c::WatchEnd::NotServed));
    assert!(h.has(&Request::Hello { family: Some(19), versions: Some(c::VersionRange { first: 24, last: 24 }) }));
}

#[test]
fn the_stream_s_name_is_saved_at_each_hello() {
    const HELLOS: u32 = 2;
    let mut h = fixtures::Harness::new(&fixtures::limits());
    for _ in 0..HELLOS {
        h.clear();
        h.open(fixtures::WATCH, fixtures::frame_subject());
        h.stream_record(&fixtures::hello_event(fixtures::HEARTBEAT));
        assert!(h.has(&Request::Save { record: fixtures::kept_stream() }));
        h.link.step(Time::ZERO, Event::StreamGone, &mut h.out);
    }
}
const GONE_TERMINALS: [bool; 2] = [false, true];
#[test]
fn a_kept_stream_name_is_sent_gone_before_the_first_open() {
    for lost in GONE_TERMINALS {
        let mut h = fixtures::Harness::new(&fixtures::limits());
        h.start_store(fixtures::one_store_read(fixtures::kept_stream()));
        h.open(fixtures::WATCH, fixtures::frame_subject());
        assert_eq!(h.gone_name(), fixtures::STREAM, "lost={lost}");
        let Some(Request::Exchange { exchange, .. }) = h.out.iter().next() else { panic!("gone first") };
        let exchange = *exchange;
        h.clear();
        h.open(fixtures::ATTEMPT, fixtures::family_subject());
        assert_eq!(h.out.len(), 0, "lost={lost}");
        let event = if lost { Event::Lost { exchange } } else { Event::Reply { exchange, message: Box::new([]) } };
        h.link.step(Time::ZERO, event, &mut h.out);
        assert_eq!(h.out.len(), 1, "lost={lost}");
        assert!(h.has(&Request::OpenStream { credential: None }), "lost={lost}");
    }
}

#[test]
fn a_too_linked_watch_ending_decodes_its_join_bound() {
    for joins in [4, u32::MAX] {
        let mut harness = fixtures::Harness::streaming(fixtures::frame_subject());
        let (_, name) = harness.opening();
        let why = w::WatchEnd::TooLinked(
            w::WatchTooLinked::new(&fixtures::limits().wire, w::WatchTooLinkedParts { joins }).expect("join bound"),
        );
        harness.stream_record(&fixtures::ending(name, why));
        assert_eq!(harness.ended(fixtures::WATCH), Some(c::WatchEnd::TooLinked { joins }));
    }
}
