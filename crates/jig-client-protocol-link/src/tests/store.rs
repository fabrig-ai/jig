use super::*;
fn restored(expected: &Saved) -> fixtures::Harness {
    let mut harness = fixtures::Harness::new(&fixtures::limits());
    harness.start_store(fixtures::one_store_read(fixtures::store_read(expected.clone())));
    harness
}
fn pending_write(record: Saved) -> fixtures::Harness {
    let mut harness = fixtures::Harness::new(&fixtures::limits());
    harness.save(record);
    harness.send_kept(fixtures::ATTEMPT);
    harness
}
#[test]
fn a_table_entry_restores_its_key_and_ask() {
    for ask in fixtures::store_asks().into_boxed() {
        let expected = fixtures::saved_table(ask);
        assert!(restored(&expected).restored(&expected));
    }
}
#[test]
fn a_record_of_another_store_version_is_dropped_and_told() {
    let mut harness = fixtures::Harness::new(&fixtures::limits());
    harness.start_store(fixtures::one_store_read(fixtures::other_store_version(fixtures::store_read(
        fixtures::saved_table(fixtures::message()),
    ))));
    assert!(harness.has(&Request::Dropped));
    assert_eq!(harness.out.len(), 1);
}
#[test]
fn a_step_s_writes_leave_before_its_exchanges() {
    let mut harness = pending_write(fixtures::saved_table(fixtures::message()));
    assert_eq!(harness.exchanges(), 0);
    harness.written();
    let Some(Request::Save { .. }) = harness.out.iter().next() else { panic!("write leaves first") };
    assert_eq!(harness.exchanges(), 1);
}
#[test]
fn a_write_gathers_exchanges_already_in_the_step_s_output_queue() {
    let mut harness = fixtures::Harness::new(&fixtures::limits());
    harness.send_kept(fixtures::ATTEMPT);
    harness.save(fixtures::saved_table(fixtures::message()));
    assert_eq!(harness.exchanges(), 0);
    harness.written();
    let Some(Request::Save { .. }) = harness.out.iter().next() else { panic!("write leaves first") };
    assert_eq!(harness.exchanges(), 1);
}
#[test]
fn a_failed_write_withholds_the_step_s_exchanges() {
    let mut harness = pending_write(fixtures::saved_table(fixtures::message()));
    harness.write_failed();
    harness.written();
    assert_eq!(harness.exchanges(), 0);
    assert!(harness.has(&Request::NotKept { attempt: fixtures::ATTEMPT }));
    assert_eq!(harness.terminals(), 1);
}
#[test]
fn a_table_entry_restores_family_bytes_and_their_wire_version() {
    let expected = Saved {
        version: fixtures::TABLE_VERSION,
        record: skein_lib::RequestRecord {
            key: fixtures::KEY,
            scope: u64::MAX,
            first_sent: None,
            request: jig_client_domain::Held {
                key: fixtures::APPLICATION_KEY,
                request: Ask::Family {
                    scope: jig_client_domain::contract::Scope::Project(1),
                    record: fixtures::family_read(),
                },
            },
        },
    };
    assert!(restored(&expected).restored(&expected));
}
#[test]
fn an_erased_key_waits_for_durability_before_the_step_s_exchange() {
    let mut harness = fixtures::Harness::new(&fixtures::limits());
    harness.link.step(Time::ZERO, Event::Erase { key: fixtures::KEY }, &mut harness.out);
    harness.send_kept(fixtures::ATTEMPT);
    assert_eq!(harness.exchanges(), 0);
    harness.written();
    let Some(Request::Erase { .. }) = harness.out.iter().next() else { panic!("erase leaves first") };
    assert_eq!(harness.exchanges(), 1);
}
#[test]
fn malformed_kept_bytes_drop_the_owner_and_are_reported() {
    let mut entry = fixtures::store_read(fixtures::saved_table(fixtures::message()));
    entry.bytes = Box::from([255]);
    let mut harness = fixtures::Harness::new(&fixtures::limits());
    harness.start_store(fixtures::one_store_read(entry));
    assert!(harness.has(&Request::Dropped));
    assert!(harness.has(&Request::Fact { fact: Fact::UndecodableStore }));
    assert_eq!(harness.out.len(), 2);
}
#[test]
fn a_mismatched_table_storage_key_drops_its_record() {
    let mut entry = fixtures::store_read(fixtures::saved_table(fixtures::message()));
    let length = entry.key.len();
    *entry.key.get_mut(length - 1).unwrap() = 0;
    let mut harness = fixtures::Harness::new(&fixtures::limits());
    harness.start_store(fixtures::one_store_read(entry));
    assert!(harness.has(&Request::Dropped));
    assert!(harness.has(&Request::Fact { fact: Fact::UndecodableStore }));
}
#[test]
fn a_cancelled_opening_already_in_the_step_queue_is_never_sent_after_writing() {
    let mut harness = fixtures::Harness::streaming(fixtures::family_subject());
    let (exchange, _) = harness.opening();
    harness.link.step(Time::ZERO, Event::Lost { exchange }, &mut harness.out);
    harness.save(fixtures::saved_table(fixtures::message()));
    harness.written();
    assert_eq!(harness.exchanges(), 0);
    assert_eq!(harness.ended(fixtures::WATCH), Some(c::WatchEnd::Dropped));
}

#[test]
fn a_full_table_and_the_stream_s_name_start() {
    const TABLE_RECORDS: u32 = 4;
    const KEPT_RECORDS: u32 = TABLE_RECORDS + 1;
    let mut h = fixtures::Harness::new(&fixtures::limits());
    let mut kept = skein_lib::List::with_capacity(KEPT_RECORDS);
    assert_eq!(fixtures::limits().client.requests.requests, TABLE_RECORDS);
    for number in 0..TABLE_RECORDS {
        let mut record = fixtures::saved_table(fixtures::message());
        record.record.key = [u8::try_from(number).unwrap(); 16];
        kept.push(fixtures::store_read(record)).unwrap();
    }
    kept.push(fixtures::kept_stream()).unwrap();
    h.start_store(kept);
    assert_eq!(h.out.len(), TABLE_RECORDS);
    for request in &h.out {
        let Request::Kept { .. } = request else { panic!("restored table entry: {request:?}") };
    }
    h.clear();
    h.open(fixtures::WATCH, fixtures::frame_subject());
    assert_eq!(h.gone_name(), fixtures::STREAM);
    assert!(!h.has(&Request::OpenStream { credential: None }));
}

#[test]
fn valid_refuses_each_unrepresentable_limit() {
    let base = fixtures::limits();
    assert!(crate::store::valid(&base));
    for limits in [
        Limits { exchanges: u32::MAX, ..base },
        Limits { wire: w::Limits { owned_record: u32::MAX, ..base.wire }, ..base },
    ] {
        assert!(!crate::store::valid(&limits));
    }
}

/// O4: a completed write releases its admitted exchanges in arrival order.
#[test]
fn held_exchanges_leave_in_order_after_the_write() {
    let mut harness = fixtures::Harness::new(&fixtures::limits());
    harness.save(fixtures::saved_table(fixtures::message()));
    for number in [1, 2, 3] {
        harness.link.step(
            Time::ZERO,
            Event::Send {
                attempt: Token::new(number),
                key: [u8::try_from(number).unwrap(); 16],
                ask: Ask::Jig(Box::new(fixtures::message())),
                value: None,
            },
            &mut harness.out,
        );
    }
    assert_eq!(harness.exchanges(), 0, "all exchanges await the write");
    harness.clear();
    harness.written();
    let mut keys = skein_lib::List::with_capacity(3);
    while let Some(request) = harness.out.pop() {
        let Request::Exchange { message: Message::Request(bytes), .. } = request else {
            panic!("only held keyed exchanges leave: {request:?}")
        };
        let keyed = w::Keyed::decode(&fixtures::limits().wire, &mut Reader::new(&bytes)).unwrap();
        keys.push(*keyed.key()).unwrap();
    }
    assert_eq!(keys.as_slice(), &[[1; 16], [2; 16], [3; 16]]);
    harness.written();
    assert_eq!(harness.exchanges(), 0, "a repeated answer releases nothing twice");
}
