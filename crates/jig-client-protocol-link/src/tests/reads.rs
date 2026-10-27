use super::*;
#[test]
fn a_read_keeps_its_long_cursor_as_opaque_bytes() {
    let actual = translate::reads::read(&fixtures::history_read(), &fixtures::limits().wire).unwrap();
    let w::Read::History(record) = actual else { panic!("history record") };
    assert_eq!(record.task(), 7);
    assert!(record.subtree());
    assert_eq!(record.after().as_ref().unwrap().value(), fixtures::READ_CURSOR);
}
#[test]
fn a_read_cursor_past_its_wire_bound_is_refused() {
    let mut limits = fixtures::limits().wire;
    limits.cursor_value = 47;
    let problem = translate::reads::read(&fixtures::history_read(), &limits).unwrap_err();
    assert_eq!(problem.path, w::Path::CursorValue);
    assert_eq!(problem.reason, skein_codec::Reason::Bound);
}
#[test]
fn a_notes_read_keeps_its_scope_and_filter_with_its_cursor() {
    let actual = translate::reads::read(&fixtures::notes_read(), &fixtures::limits().wire).unwrap();
    let w::Read::Notes(record) = actual else { panic!("notes record") };
    assert_eq!(record.scope(), &fixtures::notes_scope_record());
    assert_eq!(record.description().as_deref(), Some(b"filter".as_slice()));
    assert_eq!(record.after().as_ref().unwrap().value(), fixtures::READ_CURSOR);
}

#[test]
fn a_read_body_names_jigs_wire_and_keeps_its_read() {
    let record = translate::reads::owned(Readable::Jig(fixtures::history_read()), &fixtures::limits().wire).unwrap();
    assert_eq!(record.owner(), &w::Owner::Jig);
    assert_eq!(record.version(), jig_client_protocol_wire::VERSION);
    let read = w::Read::decode(&fixtures::limits().wire, &mut Reader::new(record.record())).unwrap();
    let w::Read::History(read) = read else { panic!("history read") };
    assert_eq!(read.task(), 7);
    assert_eq!(read.after().as_ref().unwrap().value(), fixtures::READ_CURSOR);
}
#[test]
fn a_family_read_body_keeps_its_opaque_owner_record() {
    let record = translate::reads::owned(
        Readable::Family {
            scope: jig_client_domain::contract::Scope::Project(1),
            window: jig_client_domain::contract::Window { rows: 3, after: None },
            record: fixtures::family_read(),
        },
        &fixtures::limits().wire,
    )
    .unwrap();
    let w::Owner::Family(owner) = record.owner() else { panic!("family owner") };
    assert_eq!(owner.number(), 19);
    assert_eq!(record.version(), 23);
    let family = w::FamilyRecord::decode(&fixtures::limits().wire, &mut Reader::new(record.record())).unwrap();
    assert_eq!(family.record(), &[255, 0, 1]);
}
#[test]
fn each_wire_none_keeps_its_reason() {
    for (wire, client) in [
        (w::ReadNone::Malformed, c::ReadNone::Malformed),
        (
            w::ReadNone::TooLinked(
                w::ReadTooLinked::new(&fixtures::limits().wire, w::ReadTooLinkedParts { joins: 4 }).expect("bound"),
            ),
            c::ReadNone::TooLinked { joins: 4 },
        ),
        (
            w::ReadNone::TooLarge(
                w::TurnTooLarge::new(&fixtures::limits().wire, w::TurnTooLargeParts { bytes: 8192 }).expect("size"),
            ),
            c::ReadNone::TooLarge { bytes: 8192 },
        ),
        (w::ReadNone::Unknown, c::ReadNone::Unknown),
        (w::ReadNone::Refused, c::ReadNone::Refused),
        (w::ReadNone::Gone, c::ReadNone::Gone),
        (w::ReadNone::Busy, c::ReadNone::Busy),
        (w::ReadNone::SignedOut, c::ReadNone::SignedOut),
    ] {
        assert_eq!(translate::reads::none(&wire), client);
    }
}

#[test]
fn a_read_becomes_one_post_with_its_owner_and_cursor() {
    let (mut h, _) = fixtures::Harness::reading();
    assert_eq!(h.out.len(), 1);
    let Request::Exchange { message: Message::Read(body), .. } = h.out.pop().unwrap() else {
        panic!("expected read exchange")
    };
    let record = w::Owned::decode(&fixtures::limits().wire, &mut Reader::new(&body)).unwrap();
    assert_eq!(record.owner(), &w::Owner::Jig);
    assert_eq!(record.version(), jig_client_protocol_wire::VERSION);
    let w::Read::History(value) = w::Read::decode(&fixtures::limits().wire, &mut Reader::new(record.record())).unwrap()
    else {
        panic!("history read")
    };
    assert_eq!(value.after().as_ref().unwrap().value(), fixtures::READ_CURSOR);
}
#[test]
fn a_read_signed_out_is_answered_signed_out() {
    let (mut h, exchange) = fixtures::Harness::reading();
    h.answer(exchange, &fixtures::read_none_answer(w::ReadNone::SignedOut));
    assert_eq!(h.read_answered(), Some(&Readout::Jig(c::ReadResult::None(c::ReadNone::SignedOut))));
    assert_eq!(h.link.next_deadline(), None);
}
#[test]
fn a_read_with_no_answer_is_unreachable() {
    let (mut h, exchange) = fixtures::Harness::reading();
    h.link.step(Time::ZERO, Event::Lost { exchange }, &mut h.out);
    h.read_unreachable();
    h.answer(exchange, &fixtures::read_none_answer(w::ReadNone::Unknown));
    assert_eq!(h.read_terminals(), 1);
    assert_eq!(h.link.next_deadline(), None);
}
#[test]
fn a_lost_read_is_answered_once() {
    let (mut h, exchange) = fixtures::Harness::reading();
    h.link.step(Time::ZERO, Event::Lost { exchange }, &mut h.out);
    h.read_unreachable();
    h.answer(exchange, &fixtures::read_none_answer(w::ReadNone::Busy));
    assert_eq!(h.read_terminals(), 1);
    assert_eq!(h.link.next_deadline(), None);
}
#[test]
fn a_read_none_answer_keeps_each_wire_reason_once() {
    for (wire, client) in [
        (w::ReadNone::Malformed, c::ReadNone::Malformed),
        (
            w::ReadNone::TooLinked(
                w::ReadTooLinked::new(&fixtures::limits().wire, w::ReadTooLinkedParts { joins: 4 }).expect("bound"),
            ),
            c::ReadNone::TooLinked { joins: 4 },
        ),
        (
            w::ReadNone::TooLarge(
                w::TurnTooLarge::new(&fixtures::limits().wire, w::TurnTooLargeParts { bytes: 8192 }).expect("size"),
            ),
            c::ReadNone::TooLarge { bytes: 8192 },
        ),
        (w::ReadNone::Unknown, c::ReadNone::Unknown),
        (w::ReadNone::Refused, c::ReadNone::Refused),
        (w::ReadNone::Gone, c::ReadNone::Gone),
        (w::ReadNone::Busy, c::ReadNone::Busy),
    ] {
        let (mut h, exchange) = fixtures::Harness::reading();
        h.answer(exchange, &fixtures::read_none_answer(wire));
        h.answer(exchange, &fixtures::read_none_answer(w::ReadNone::SignedOut));
        assert_eq!(h.read_answered(), Some(&Readout::Jig(c::ReadResult::None(client))));
        assert_eq!(h.read_terminals(), 1);
    }
}
#[test]
fn a_family_read_is_answered_once_with_its_opaque_value() {
    let (mut h, exchange) = fixtures::Harness::reading_family();
    h.answer(exchange, &fixtures::family_read_answer(19, 23));
    h.answer(exchange, &fixtures::family_read_answer(19, 23));
    assert_eq!(h.read_answered(), Some(&Readout::Family { family: 19, record: Some(Box::from([255, 0, 1])) }));
    assert_eq!(h.read_terminals(), 1);
}
#[test]
fn a_family_read_none_stays_under_its_owner() {
    let (mut h, exchange) = fixtures::Harness::reading_family();
    h.answer(exchange, &fixtures::read_none_answer(w::ReadNone::Unknown));
    assert_eq!(h.read_answered(), Some(&Readout::Family { family: 19, record: None }));
}
#[test]
fn a_read_value_from_another_owner_or_version_is_reported() {
    for (owner, version) in [(20, 23), (19, 24)] {
        let (mut h, exchange) = fixtures::Harness::reading_family();
        h.answer(exchange, &fixtures::family_read_answer(owner, version));
        assert_eq!(h.read_answered(), Some(&Readout::Family { family: 19, record: None }));
        assert!(h.has(&Request::Fact { fact: Fact::UndecodableDocument }));
    }
}
#[test]
fn a_malformed_read_body_is_unreachable() {
    let (mut h, exchange) = fixtures::Harness::reading();
    h.answer(exchange, &[255]);
    h.read_unreachable();
    assert!(h.has(&Request::Fact { fact: Fact::UndecodableDocument }));
}
#[test]
fn reads_and_attempts_share_the_exchange_admission_bound() {
    let mut limits = fixtures::limits();
    limits.exchanges = 1;
    let mut h = fixtures::Harness::new(&limits);
    h.send(fixtures::ATTEMPT, fixtures::KEY, fixtures::message());
    h.link.step(
        Time::ZERO,
        Event::Read { read: fixtures::READ, what: Readable::Jig(fixtures::history_read()) },
        &mut h.out,
    );
    h.read_unreachable();
    assert_eq!(h.read_terminals(), 1);
    assert_eq!(h.out.len(), 2);
}

#[test]
fn a_read_is_answered_once_with_its_value() {
    let (mut h, exchange) = fixtures::Harness::reading();
    h.answer(exchange, &fixtures::jig_read_value_answer(&fixtures::frame_read_value()));
    h.answer(exchange, &fixtures::read_none_answer(w::ReadNone::Unknown));
    assert_eq!(h.read_answered(), Some(&fixtures::expected_frame_read()));
    assert_eq!(h.read_terminals(), 1);
    assert_eq!(h.link.next_deadline(), None);
}
#[test]
fn a_value_past_the_client_s_limits_is_unreachable_and_reported() {
    for value in fixtures::values_past_client_limits() {
        let (mut h, exchange) = fixtures::Harness::reading();
        h.answer(exchange, &fixtures::jig_read_value_answer(&value));
        h.read_unreachable();
        assert!(h.has(&Request::Fact { fact: Fact::UndecodableDocument }));
        assert_eq!(h.read_terminals(), 1);
    }
}
#[test]
fn each_discriminated_read_value_reaches_its_contract_variant() {
    for (wire, expected) in fixtures::read_value_cases() {
        let (mut h, exchange) = fixtures::Harness::reading();
        h.answer(exchange, &fixtures::jig_read_value_answer(&wire));
        assert_eq!(h.read_answered(), Some(&Readout::Jig(c::ReadResult::Value(expected))));
        assert!(!h.has(&Request::Fact { fact: Fact::UndecodableDocument }));
    }
}
#[test]
fn a_malformed_read_value_is_unreachable_and_reported() {
    for value in fixtures::malformed_read_values() {
        let (mut h, exchange) = fixtures::Harness::reading();
        h.answer(exchange, &fixtures::jig_read_record_answer(value, jig_client_protocol_wire::VERSION));
        h.read_unreachable();
        assert!(h.has(&Request::Fact { fact: Fact::UndecodableDocument }));
    }
}
#[test]
fn a_read_value_of_another_wire_version_is_unreachable_and_reported() {
    let (mut h, exchange) = fixtures::Harness::reading();
    let value = fixtures::encoded_read_value(&fixtures::frame_read_value());
    h.answer(exchange, &fixtures::jig_read_record_answer(value, 2));
    h.read_unreachable();
    assert!(h.has(&Request::Fact { fact: Fact::UndecodableDocument }));
}
#[test]
fn a_read_value_counts_its_discriminator_against_the_document_byte_bound() {
    let value = fixtures::encoded_read_value(&fixtures::frame_read_value());
    let bytes = u64::try_from(value.len()).unwrap();
    let limits = fixtures::read_byte_limits(bytes);
    assert_eq!(translate::reads::value(&value, &limits), Ok(c::ReadValue::Me(Box::new(fixtures::frame()))));
    let short = fixtures::read_byte_limits(bytes.checked_sub(1).unwrap());
    assert_eq!(translate::reads::value(&value, &short), Err(Undecodable::TooLarge));
}
#[test]
fn a_read_value_rejects_mutated_original_limits() {
    let value = fixtures::encoded_read_value(&fixtures::frame_read_value());
    let mut limits = fixtures::limits();
    limits.wire.cursor_value = 1;
    assert_eq!(translate::reads::value(&value, &limits), Err(Undecodable::TooLarge));
}
#[test]
fn a_read_page_keeps_both_long_cursors_without_parsing_them() {
    let (mut h, exchange) = fixtures::Harness::reading();
    let value = w::ReadValue::Notes(fixtures::note_page_with_cursors());
    h.answer(exchange, &fixtures::jig_read_value_answer(&value));
    let expected = c::ReadValue::Notes(Box::new(fixtures::expected_note_page_with_cursors()));
    assert_eq!(h.read_answered(), Some(&Readout::Jig(c::ReadResult::Value(expected))));
}
#[test]
fn a_read_activity_keeps_each_undecodable_turn_in_its_page() {
    for (record, expected) in fixtures::unread_pages() {
        let (mut h, exchange) = fixtures::Harness::reading();
        h.answer(exchange, &fixtures::jig_read_value_answer(&w::ReadValue::Activity(record)));
        let expected = c::ReadValue::Activity(Box::new(expected));
        assert_eq!(h.read_answered(), Some(&Readout::Jig(c::ReadResult::Value(expected))));
    }
}

#[test]
fn me_s_projects_past_the_document_bytes_are_refused_as_too_large() {
    let value = jig_client_test_records::frame_two_projects_record();
    let mut limits = fixtures::limits();
    limits.client.document_bytes = u64::from(value.measure()).checked_sub(1).unwrap();
    assert_eq!(translate::documents::frame(value, &limits), Err(Undecodable::TooLarge));
}

#[test]
fn a_turn_read_keeps_all_three_numbers() {
    let value = translate::reads::read(&fixtures::turn_read(), &fixtures::limits().wire).unwrap();
    let w::Read::Turn(value) = value else { panic!("turn read") };
    assert_eq!((value.task(), value.attempt(), value.turn()), (7, 44, 3));
}
#[test]
fn a_turn_read_keeps_its_archive_metadata() {
    let (mut harness, exchange) = fixtures::reading_turn(&fixtures::limits());
    let answer = fixtures::jig_read_value_answer(&fixtures::single_turn_value());
    harness.answer(exchange, &answer);
    harness.answer(exchange, &answer);
    assert_eq!(
        harness.read_answered(),
        Some(&Readout::Jig(c::ReadResult::Value(c::ReadValue::Turn(Box::new(fixtures::expected_single_turn())))))
    );
    assert_eq!(harness.read_terminals(), 1);
    assert_eq!(harness.link.next_deadline(), None);
}
#[test]
fn a_turn_at_the_document_bound_decodes() {
    let limits = fixtures::read_byte_limits(fixtures::TURN_DOCUMENT_BYTES);
    let (mut harness, exchange) = fixtures::reading_turn(&limits);
    harness.answer(exchange, &fixtures::jig_read_value_answer(&fixtures::single_turn_value()));
    assert_eq!(
        harness.read_answered(),
        Some(&Readout::Jig(c::ReadResult::Value(c::ReadValue::Turn(Box::new(fixtures::expected_single_turn())))))
    );
}
#[test]
fn a_turn_past_the_document_bound_keeps_its_size() {
    let limits = fixtures::read_byte_limits(fixtures::TURN_DOCUMENT_BYTES - 1);
    let (mut harness, exchange) = fixtures::reading_turn(&limits);
    let answer = fixtures::jig_read_value_answer(&fixtures::single_turn_value());
    harness.answer(exchange, &answer);
    harness.answer(exchange, &answer);
    assert_eq!(
        harness.read_answered(),
        Some(&Readout::Jig(c::ReadResult::None(c::ReadNone::TooLarge { bytes: fixtures::TURN_PAYLOAD_BYTES })))
    );
    assert_eq!(harness.read_terminals(), 1);
    assert_eq!(harness.link.next_deadline(), None);
}
#[test]
fn an_undecodable_single_turn_is_malformed() {
    for body in [&[0, 4][..], &[0, 6][..], &[255][..]] {
        let value = w::ReadValue::Turn(fixtures::single_turn_record(body));
        let harness = fixtures::answered_turn(&fixtures::limits(), &fixtures::jig_read_value_answer(&value));
        assert_eq!(
            harness.read_answered(),
            Some(&Readout::Jig(c::ReadResult::None(c::ReadNone::Malformed))),
            "{body:?}"
        );
    }
}
#[test]
fn a_turn_answer_for_another_identity_is_rejected() {
    for (task, attempt, turn) in [(8, 44, 3), (7, 45, 3), (7, 44, 4)] {
        let mut record = fixtures::single_turn_record(fixtures::FULL_TURN).into_parts();
        record.task = task;
        record.name = w::TurnName::new(&w::CEILINGS, w::TurnNameParts { attempt, number: turn }).unwrap();
        let value = w::ReadValue::Turn(w::ArchivedTurn::new(&w::CEILINGS, record).unwrap());
        let harness = fixtures::answered_turn(&fixtures::limits(), &fixtures::jig_read_value_answer(&value));
        assert_eq!(
            harness.read_answered(),
            Some(&Readout::Jig(c::ReadResult::None(c::ReadNone::Malformed))),
            "{task}/{attempt}/{turn}"
        );
    }
}
#[test]
fn the_turn_inspector_rejects_incomplete_or_false_envelopes() {
    let limits = fixtures::read_byte_limits(fixtures::TURN_DOCUMENT_BYTES - 1);
    for (label, body) in fixtures::malformed_turn_answers() {
        let harness = fixtures::answered_turn(&limits, &body);
        assert_eq!(
            harness.read_answered(),
            Some(&Readout::Jig(c::ReadResult::None(c::ReadNone::Malformed))),
            "{label}"
        );
        assert_eq!(harness.read_terminals(), 1, "{label}");
    }
}

#[test]
fn the_single_turn_decoder_uses_the_archive_bound() {
    // The archive decoder applies its TurnCarriedBytes bound.
    let mut archive_limits = fixtures::limits();
    archive_limits.wire.activity_turn_turn = 1;
    assert_eq!(
        translate::turn::archive(fixtures::single_turn_record(fixtures::FULL_TURN), &archive_limits),
        Ok(fixtures::expected_single_turn())
    );
}

#[test]
fn a_too_linked_read_keeps_its_bound_after_decoding() {
    let (mut h, exchange) = fixtures::Harness::reading();
    let none = w::ReadNone::TooLinked(
        w::ReadTooLinked::new(&fixtures::limits().wire, w::ReadTooLinkedParts { joins: 4 }).expect("bound"),
    );
    h.answer(exchange, &fixtures::read_none_answer(none));
    assert_eq!(h.read_answered(), Some(&Readout::Jig(c::ReadResult::None(c::ReadNone::TooLinked { joins: 4 }))));
}
