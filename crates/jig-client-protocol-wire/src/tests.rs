//! Client wire refusals and round trips (protocol/client.md, section 3).
mod fixtures;
mod task_refusal;
use crate::*;
use skein_lib::Reader;

fn a_keyed_request_past_its_bound_is_refused_naming_it() {
    let limits = fixtures::limits();
    let request = fixtures::bytes(limits.keyed_request.checked_add(1).expect("small bound"));
    let refused = Keyed::new(&limits, KeyedParts { key: [7; 16], owner: Owner::Jig, version: 1, request, value: None });
    assert_eq!(refused, Err(Problem { path: Path::KeyedRequest, reason: skein_codec::Reason::Bound }));
}

fn an_owned_record_keeps_its_owners_version() {
    let limits = fixtures::limits();
    let owned = fixtures::owned();
    let bytes = fixtures::encoded(&owned, Owned::measure, Owned::encode);
    assert_eq!(Owned::decode(&limits, &mut Reader::new(&bytes)), Ok(owned));
}

fn a_refusal_with_its_facts_round_trips_unchanged() {
    let limits = fixtures::limits();
    let refused = fixtures::refused_moved();
    let bytes = fixtures::encoded(&refused, Refused::measure, Refused::encode);
    assert_eq!(Refused::decode(&limits, &mut Reader::new(&bytes)), Ok(refused));
}

fn each_answer_kind_round_trips_unchanged() {
    let limits = fixtures::limits();
    for answer in [
        KeyedAnswer::NotServed(fixtures::not_served()),
        KeyedAnswer::FinalAnswer(fixtures::owned()),
        KeyedAnswer::Busy,
        KeyedAnswer::AnswersFull(
            AnswersFull::new(&limits, AnswersFullParts { held: u32::MAX, room_at: u64::MAX }).expect("full facts"),
        ),
        KeyedAnswer::NotReady,
        KeyedAnswer::SignedOut,
    ] {
        let bytes = fixtures::encoded(&answer, KeyedAnswer::measure, KeyedAnswer::encode);
        assert_eq!(KeyedAnswer::decode(&limits, &mut Reader::new(&bytes)), Ok(answer));
    }
}

fn a_bearer_token_past_its_bound_is_refused() {
    let limits = fixtures::limits();
    let token = fixtures::bytes(limits.signed_in_token_some.checked_add(1).expect("small bound"));
    assert_eq!(
        SignedIn::new(&limits, SignedInParts { token: Some(skein_lib::Secret::new(token)), party: 17, expires: 29 }),
        Err(Problem { path: Path::SignedInToken, reason: skein_codec::Reason::Bound })
    );
}

fn a_cursor_past_its_bound_is_refused() {
    let limits = fixtures::limits();
    let value = fixtures::bytes(limits.cursor_value.checked_add(1).expect("small bound"));
    assert_eq!(
        Cursor::new(&limits, CursorParts { value }),
        Err(Problem { path: Path::CursorValue, reason: skein_codec::Reason::Bound })
    );
}

fn each_read_none_round_trips_unchanged() {
    let limits = fixtures::limits();
    for none in [
        ReadNone::Unknown,
        ReadNone::Refused,
        ReadNone::Gone,
        ReadNone::Busy,
        ReadNone::SignedOut,
        ReadNone::Malformed,
        ReadNone::TooLinked(ReadTooLinked::new(&limits, ReadTooLinkedParts { joins: 4 }).expect("join bound")),
        ReadNone::TooLarge(TurnTooLarge::new(&limits, TurnTooLargeParts { bytes: 8192 }).expect("size")),
    ] {
        let answer = ReadAnswer::None(fixtures::read_none(none));
        let bytes = fixtures::encoded(&answer, ReadAnswer::measure, ReadAnswer::encode);
        assert_eq!(ReadAnswer::decode(&limits, &mut Reader::new(&bytes)), Ok(answer));
    }
}

fn a_read_value_round_trips_with_its_owner_version() {
    let limits = fixtures::limits();
    let answer = ReadAnswer::Value(fixtures::owned());
    let bytes = fixtures::encoded(&answer, ReadAnswer::measure, ReadAnswer::encode);
    assert_eq!(ReadAnswer::decode(&limits, &mut Reader::new(&bytes)), Ok(answer));
}

#[test]
fn a_page_one_row_past_its_limit_is_refused() {
    let limits = fixtures::limits();
    let rows = fixtures::funding_rows(limits.owner_spend_page_rows.checked_add(1).expect("small bound"));
    assert_eq!(
        OwnerSpendPage::new(&limits, OwnerSpendPageParts { rows, before: None, after: None, cut: false }),
        Err(Problem { path: Path::OwnerSpendPageRows, reason: skein_codec::Reason::Bound })
    );
}

fn an_activity_line_carries_its_turn_bytes_unchanged() {
    let limits = fixtures::limits();
    let line = fixtures::activity_line(Box::from([0, 255, 1]));
    let bytes = fixtures::encoded(&line, ActivityLine::measure, ActivityLine::encode);
    assert_eq!(ActivityLine::decode(&limits, &mut Reader::new(&bytes)), Ok(line));
}

#[test]
fn a_page_whose_rows_do_not_split_to_its_end_is_refused_as_malformed() {
    let limits = fixtures::limits_with_activity_bytes(4096);
    for rows in fixtures::malformed_activity_pieces() {
        let page = fixtures::activity_page(&limits, rows);
        let bytes = fixtures::encoded(&page, ActivityPage::measure, ActivityPage::encode);
        assert_eq!(
            fixtures::decode_activity_page(&limits, &bytes, 8),
            Err(Problem { path: Path::ActivityPageRows, reason: skein_codec::Reason::Malformed })
        );
    }
}

#[test]
fn a_page_of_many_small_rows_and_one_of_a_few_large_rows_fit_one_bound() {
    let limits = fixtures::limits_with_activity_bytes(4096);
    for (count, width) in [(4, 16), (2, 512)] {
        let rows = fixtures::activity_rows(count, width);
        let bytes = fixtures::encoded_activity_page(&limits, &rows);
        assert_eq!(fixtures::decode_activity_page(&limits, &bytes, 8), Ok(rows));
    }
}

fn me_at_its_ceilings_round_trips_unchanged() {
    let limits = fixtures::document_limits();
    let frame = fixtures::frame_at_ceilings(&limits);
    let bytes = fixtures::encoded(&frame, Me::measure, Me::encode);
    assert_eq!(Me::decode(&limits, &mut Reader::new(&bytes)), Ok(frame));
}

#[test]
fn a_task_one_byte_past_its_text_bound_is_refused() {
    let mut limits = fixtures::document_limits();
    let task = fixtures::task(&limits);
    let bytes = fixtures::encoded(&task, Task::measure, Task::encode);
    limits.document_text_bytes -= 1;
    let expected = Err(Problem { path: Path::DocumentTextBytes, reason: skein_codec::Reason::Bound });
    assert_eq!(Task::decode(&limits, &mut Reader::new(&bytes)), expected);
    assert_eq!(Task::new(&limits, task.into_parts()), expected);
}

fn a_run_state_without_writing_or_call_round_trips() {
    let limits = fixtures::document_limits();
    let run = fixtures::run_state(&limits);
    let bytes = fixtures::encoded(&run, RunState::measure, RunState::encode);
    assert_eq!(RunState::decode(&limits, &mut Reader::new(&bytes)), Ok(run));
}

fn a_chip_with_each_phase_round_trips_unchanged() {
    let limits = fixtures::document_limits();
    for phase in fixtures::chip_phases(&limits) {
        let rows = fixtures::chip_facts(&limits, phase);
        let chip = fixtures::chip(&limits, &rows);
        let bytes = fixtures::encoded(&chip, Chip::measure, Chip::encode);
        assert_eq!(fixtures::decode_chip_facts(&limits, &bytes), rows);
    }
}

fn a_hello_naming_two_owners_round_trips_unchanged() {
    let limits = fixtures::stream_limits();
    let hello = fixtures::hello(&limits);
    let bytes = fixtures::encoded(&hello, Hello::measure, Hello::encode);
    assert_eq!(Hello::decode(&limits, &mut Reader::new(&bytes)), Ok(hello));
}

fn a_stream_event_at_its_ceiling_round_trips_unchanged() {
    let limits = fixtures::stream_limits();
    for event in fixtures::stream_events(&limits) {
        let bytes = fixtures::encoded(&event, StreamEvent::measure, StreamEvent::encode);
        assert_eq!(StreamEvent::decode(&limits, &mut Reader::new(&bytes)), Ok(event));
    }
}

#[test]
fn an_open_for_a_tree_carries_its_window() {
    let limits = fixtures::stream_limits();
    let open = fixtures::open_tree(&limits);
    let bytes = fixtures::encoded(&open, Open::measure, Open::encode);
    let decoded = Open::decode(&limits, &mut Reader::new(&bytes)).expect("whole open exchange");
    assert_eq!(
        Subject::decode(&limits, &mut Reader::new(decoded.subject().record())),
        Ok(fixtures::tree_subject(&limits))
    );
    assert_eq!(decoded, open);
}

fn each_watch_end_but_dropped_round_trips() {
    let limits = fixtures::stream_limits();
    for why in [
        WatchEnd::Closed,
        WatchEnd::Unknown,
        WatchEnd::NotAllowed,
        WatchEnd::Busy,
        WatchEnd::SubjectEnded,
        WatchEnd::SignedOut,
        WatchEnd::Stopping,
    ] {
        let ended = fixtures::ended(&limits, why);
        let bytes = fixtures::encoded(&ended, Ended::measure, Ended::encode);
        assert_eq!(Ended::decode(&limits, &mut Reader::new(&bytes)), Ok(ended));
    }
}

#[test]
fn a_deployment_document_past_its_rows_is_refused() {
    let limits = fixtures::stream_limits();
    let parts =
        fixtures::deployment_parts(&limits, limits.deployment_capacity.checked_add(1).expect("small row bound"));
    assert_eq!(
        Deployment::new(&limits, parts),
        Err(Problem { path: Path::DeploymentCapacity, reason: skein_codec::Reason::Bound })
    );
}

#[test]
fn an_account_waiting_to_ask_again_carries_the_wall_it_asks_at() {
    let limits = fixtures::stream_limits();
    for until in [None, Some(0), Some(u64::MAX)] {
        let state = fixtures::waiting_account_state(&limits, until);
        let account = fixtures::owner_account(&limits, state.clone());
        let bytes = fixtures::encoded(&account, OwnerAccount::measure, OwnerAccount::encode);
        let decoded = OwnerAccount::decode(&limits, &mut Reader::new(&bytes)).expect("whole account document");
        assert_eq!(decoded.state(), &state);
        assert_eq!(decoded, account);
    }
}

#[test]
fn a_historical_goal_holding_keeps_its_opaque_segments() {
    let limits = fixtures::stream_limits();
    let holding = fixtures::historical_goal_holding(&limits);
    let bytes = fixtures::encoded(&holding, OwnerGoalHolding::measure, OwnerGoalHolding::encode);
    assert_eq!(OwnerGoalHolding::decode(&limits, &mut Reader::new(&bytes)), Ok(holding));
}

fn small_store_family(limits: &Limits) -> Family {
    Family::new(limits, FamilyParts { number: u16::MAX }).expect("whole small-store owner number")
}

#[test]
fn a_kept_record_of_another_family_version_is_refused() {
    let limits = fixtures::limits();
    let kept = Kept::new(&limits, KeptParts { store_version: u16::MAX, record: Box::from([255, 0]) })
        .expect("bounded kept record");
    let mut bytes = fixtures::encoded(&kept, Kept::measure, Kept::encode);
    bytes[0] = 2;
    bytes[1] = 0;
    assert_eq!(
        Kept::decode(&limits, &mut Reader::new(&bytes)),
        Err(Problem { path: Path::KeptVersion, reason: skein_codec::Reason::Version })
    );
}

fn a_table_entry_never_sent_round_trips_with_no_wall() {
    let limits = fixtures::limits();
    let family = small_store_family(&limits);
    let entry = TableEntry::new(
        &limits,
        TableEntryParts {
            asker: 65536,
            number: 1,
            key: [7; 16],
            scope: u64::MAX,
            first_sent: None,
            owner: Owner::Family(family),
            version: u32::MAX,
            request: Box::from([255, 0]),
        },
    )
    .expect("unsent whole request");
    let bytes = fixtures::encoded(&entry, TableEntry::measure, TableEntry::encode);
    let decoded = TableEntry::decode(&limits, &mut Reader::new(&bytes)).expect("whole table entry");
    assert_eq!(decoded.first_sent(), &None);
    assert_eq!(decoded, entry);
}

fn each_read_value_variant_round_trips_unchanged() {
    let limits = fixtures::limits();
    for (name, value) in &fixtures::read_values(&limits) {
        let bytes = fixtures::encoded(value, ReadValue::measure, ReadValue::encode);
        assert_eq!(ReadValue::decode(&limits, &mut Reader::new(&bytes)), Ok(value.clone()), "{name}");
    }
}

fn a_stream_text_event_round_trips_with_its_watch() {
    let limits = fixtures::stream_limits();
    let event = StreamEvent::Text(fixtures::stream_text(&limits, fixtures::raw_live_call(&limits)));
    let bytes = fixtures::encoded(&event, StreamEvent::measure, StreamEvent::encode);
    assert_eq!(StreamEvent::decode(&limits, &mut Reader::new(&bytes)), Ok(event));
}

fn each_delivery_value_variant_round_trips_unchanged() {
    let limits = fixtures::limits();
    for (name, value) in &fixtures::delivery_values(&limits) {
        let bytes = fixtures::encoded(value, DeliveryValue::measure, DeliveryValue::encode);
        assert_eq!(DeliveryValue::decode(&limits, &mut Reader::new(&bytes)), Ok(value.clone()), "{name}");
    }
}

#[test]
fn each_turn_bytes_variant_round_trips_with_the_carried_ceiling() {
    let limits = fixtures::limits();
    let rows = [
        (
            "carried at 8 MiB",
            TurnBytes::Carried(
                TurnCarried::new(&limits, TurnCarriedParts { bytes: fixtures::bytes(8_388_608) })
                    .expect("store and worker turn ceiling"),
            ),
        ),
        (
            "too large",
            TurnBytes::TooLarge(TurnTooLarge::new(&limits, TurnTooLargeParts { bytes: u64::MAX }).expect("turn size")),
        ),
    ];
    for (name, value) in rows {
        let bytes = fixtures::encoded(&value, TurnBytes::measure, TurnBytes::encode);
        assert_eq!(TurnBytes::decode(&limits, &mut Reader::new(&bytes)), Ok(value), "{name}");
    }
}

fn a_sign_in_not_ready_round_trips_unchanged() {
    let limits = fixtures::limits();
    let answer = SignInAnswer::NotReady;
    let bytes = fixtures::encoded(&answer, SignInAnswer::measure, SignInAnswer::encode);
    assert_eq!(SignInAnswer::decode(&limits, &mut Reader::new(&bytes)), Ok(answer), "sign-in not_ready");
}

fn a_sign_out_not_ready_round_trips_unchanged() {
    let limits = fixtures::limits();
    let answer = SignOutAnswer::NotReady;
    let bytes = fixtures::encoded(&answer, SignOutAnswer::measure, SignOutAnswer::encode);
    assert_eq!(SignOutAnswer::decode(&limits, &mut Reader::new(&bytes)), Ok(answer), "sign-out not_ready");
}

#[test]
fn wire_values_round_trip_unchanged() {
    table([
        ("a_compound_s_operations_round_trip_as_pieces", a_compound_s_operations_round_trip_as_pieces),
        ("version_not_served_is_tag_zero_of_each_answer", version_not_served_is_tag_zero_of_each_answer),
        (
            "a_family_record_round_trips_with_its_scope_and_window",
            a_family_record_round_trips_with_its_scope_and_window,
        ),
        ("an_owned_record_keeps_its_owners_version", an_owned_record_keeps_its_owners_version),
        ("a_refusal_with_its_facts_round_trips_unchanged", a_refusal_with_its_facts_round_trips_unchanged),
        ("each_answer_kind_round_trips_unchanged", each_answer_kind_round_trips_unchanged),
        ("each_read_none_round_trips_unchanged", each_read_none_round_trips_unchanged),
        ("a_read_value_round_trips_with_its_owner_version", a_read_value_round_trips_with_its_owner_version),
        ("an_activity_line_carries_its_turn_bytes_unchanged", an_activity_line_carries_its_turn_bytes_unchanged),
        ("me_at_its_ceilings_round_trips_unchanged", me_at_its_ceilings_round_trips_unchanged),
        ("a_run_state_without_writing_or_call_round_trips", a_run_state_without_writing_or_call_round_trips),
        ("a_chip_with_each_phase_round_trips_unchanged", a_chip_with_each_phase_round_trips_unchanged),
        ("a_hello_naming_two_owners_round_trips_unchanged", a_hello_naming_two_owners_round_trips_unchanged),
        ("a_stream_event_at_its_ceiling_round_trips_unchanged", a_stream_event_at_its_ceiling_round_trips_unchanged),
        ("each_watch_end_but_dropped_round_trips", each_watch_end_but_dropped_round_trips),
        ("a_table_entry_never_sent_round_trips_with_no_wall", a_table_entry_never_sent_round_trips_with_no_wall),
        ("each_read_value_variant_round_trips_unchanged", each_read_value_variant_round_trips_unchanged),
        ("a_stream_text_event_round_trips_with_its_watch", a_stream_text_event_round_trips_with_its_watch),
        ("each_delivery_value_variant_round_trips_unchanged", each_delivery_value_variant_round_trips_unchanged),
        ("a_sign_in_not_ready_round_trips_unchanged", a_sign_in_not_ready_round_trips_unchanged),
        ("a_sign_out_not_ready_round_trips_unchanged", a_sign_out_not_ready_round_trips_unchanged),
    ]);
}

#[test]
fn wire_fields_past_their_bounds_are_refused() {
    table([
        ("a_keyed_request_past_its_bound_is_refused_naming_it", a_keyed_request_past_its_bound_is_refused_naming_it),
        ("a_bearer_token_past_its_bound_is_refused", a_bearer_token_past_its_bound_is_refused),
        ("a_cursor_past_its_bound_is_refused", a_cursor_past_its_bound_is_refused),
    ]);
}

type Case = (&'static str, fn());

fn table<const N: usize>(cases: [Case; N]) {
    let mut failures = [None; N];
    let mut passed = true;
    for ((name, case), failure) in cases.into_iter().zip(&mut failures) {
        if std::panic::catch_unwind(case).is_err() {
            *failure = Some(name);
            passed = false;
        }
    }
    assert!(passed, "{failures:?}");
}

fn version_not_served_is_tag_zero_of_each_answer() {
    let refusal = fixtures::not_served();
    let values = [
        fixtures::encoded(&KeyedAnswer::NotServed(refusal.clone()), KeyedAnswer::measure, KeyedAnswer::encode),
        fixtures::encoded(&ReadAnswer::NotServed(refusal.clone()), ReadAnswer::measure, ReadAnswer::encode),
        fixtures::encoded(&OpenAnswer::NotServed(refusal.clone()), OpenAnswer::measure, OpenAnswer::encode),
        fixtures::encoded(&WatchEnd::NotServed(refusal), WatchEnd::measure, WatchEnd::encode),
    ];
    for bytes in values {
        assert_eq!(bytes.first(), Some(&0));
    }
}
const FAMILY_PROJECT: u32 = 7;
fn a_family_record_round_trips_with_its_scope_and_window() {
    let limits = fixtures::limits();
    for scope in [
        Scope::Own,
        Scope::Deployment,
        Scope::Project(ScopeProject::new(&limits, ScopeProjectParts { value: FAMILY_PROJECT }).expect("project")),
    ] {
        let after = Some(Cursor::new(&limits, CursorParts { value: Box::from([1, 2, 3]) }).expect("cursor"));
        let window = Window::new(&limits, WindowParts { rows: 3, after }).expect("window");
        let record = FamilyRecord::new(&limits, FamilyRecordParts { scope, window, record: Box::from([255, 0]) })
            .expect("record");
        let bytes = fixtures::encoded(&record, FamilyRecord::measure, FamilyRecord::encode);
        assert_eq!(FamilyRecord::decode(&limits, &mut Reader::new(&bytes)), Ok(record));
    }
}

#[test]
fn each_merged_operation_target_round_trips_unchanged() {
    let limits = fixtures::limits();
    for (name, value) in fixtures::merged_targets() {
        let bytes = fixtures::encoded(&value, Ask::measure, Ask::encode);
        assert_eq!(Ask::decode(&limits, &mut Reader::new(&bytes)), Ok(value), "{name}");
    }
}

fn a_compound_s_operations_round_trip_as_pieces() {
    use skein_lib::Writer;
    let limits = fixtures::limits();
    let values = fixtures::compound_operations();
    let mut size = 0_usize;
    for value in &values {
        size = size.checked_add(usize::try_from(value.measure()).unwrap()).unwrap().checked_add(4).unwrap();
    }
    let mut bytes = Writer::new(size);
    for value in &values {
        bytes.put(&value.measure().to_be_bytes()).unwrap();
        value.encode(&mut bytes).unwrap();
    }
    let record =
        Compound::new(&limits, CompoundParts { project: fixtures::COMPOUND_PROJECT, operations: bytes.finish() })
            .unwrap();
    let bytes = fixtures::encoded(&record, Compound::measure, Compound::encode);
    let restored = Compound::decode(&limits, &mut Reader::new(&bytes)).unwrap();
    assert_eq!(restored, record);
    let mut reader = Reader::new(restored.operations());
    for expected in values {
        let bytes = skein_codec::piece::split(&mut reader).unwrap().unwrap();
        assert_eq!(Operation::decode(&limits, &mut Reader::new(&bytes)), Ok(expected));
    }
    assert!(reader.is_empty());
}

#[test]
fn a_me_project_without_its_name_cannot_decode() {
    let limits = fixtures::limits();
    let project = u32::MAX.to_be_bytes();
    assert_eq!(
        MeProject::decode(&limits, &mut Reader::new(&project)),
        Err(Problem { path: Path::DocumentTextBytes, reason: skein_codec::Reason::Short })
    );
}

// protocol/client.md, 5: Keyed.value holds at most 128 KiB of a moved file.
const MOVED_FILE_BYTES: usize = 131_072;
#[test]
fn a_keyed_value_at_the_move_bound_round_trips() {
    let limits = CEILINGS;
    let value = skein_lib::Secret::new(skein_lib::bytes::zeroed(MOVED_FILE_BYTES));
    let keyed = Keyed::new(
        &limits,
        KeyedParts { key: [7; 16], owner: Owner::Jig, version: VERSION, request: Box::new([]), value: Some(value) },
    )
    .expect("128 KiB value fits");
    let bytes = fixtures::encoded(&keyed, Keyed::measure, Keyed::encode);
    assert_eq!(Keyed::decode(&limits, &mut Reader::new(&bytes)), Ok(keyed));
}
#[test]
fn a_keyed_value_one_byte_past_the_move_bound_is_refused() {
    let value = skein_lib::Secret::new(skein_lib::bytes::zeroed(MOVED_FILE_BYTES + 1));
    assert_eq!(
        Keyed::new(
            &CEILINGS,
            KeyedParts { key: [7; 16], owner: Owner::Jig, version: VERSION, request: Box::new([]), value: Some(value) }
        ),
        Err(Problem { path: Path::KeyedValue, reason: skein_codec::Reason::Bound })
    );
}

#[test]
fn authorization_query_and_each_answer_round_trip_at_their_bounds() {
    let limits = CEILINGS;
    let record = Authorize::new(&limits, AuthorizeParts { query: fixtures::bytes(8192) }).unwrap();
    let bytes = fixtures::encoded(&record, Authorize::measure, Authorize::encode);
    assert_eq!(Authorize::decode(&limits, &mut Reader::new(&bytes)), Ok(record));
    assert_eq!(
        Authorize::new(&limits, AuthorizeParts { query: fixtures::bytes(8193) }),
        Err(Problem { path: Path::AuthorizeQuery, reason: skein_codec::Reason::Bound })
    );
    assert_eq!(
        AuthorizeGo::new(&limits, AuthorizeGoParts { location: fixtures::bytes(16385) }),
        Err(Problem { path: Path::AuthorizeGoLocation, reason: skein_codec::Reason::Bound })
    );
    let go = AuthorizeGo::new(&limits, AuthorizeGoParts { location: fixtures::bytes(16384) }).unwrap();
    for answer in
        [AuthorizeAnswer::Go(go), AuthorizeAnswer::SignedOut, AuthorizeAnswer::NotReady, AuthorizeAnswer::Busy]
    {
        let bytes = fixtures::encoded(&answer, AuthorizeAnswer::measure, AuthorizeAnswer::encode);
        assert_eq!(AuthorizeAnswer::decode(&limits, &mut Reader::new(&bytes)), Ok(answer));
    }
    for why in [
        AuthorizeWhy::Malformed,
        AuthorizeWhy::UnknownClient,
        AuthorizeWhy::UnknownRedirect,
        AuthorizeWhy::NoProvider,
        AuthorizeWhy::NotAPerson,
    ] {
        let answer = AuthorizeAnswer::Refused(AuthorizeRefused::new(&limits, AuthorizeRefusedParts { why }).unwrap());
        let bytes = fixtures::encoded(&answer, AuthorizeAnswer::measure, AuthorizeAnswer::encode);
        assert_eq!(AuthorizeAnswer::decode(&limits, &mut Reader::new(&bytes)), Ok(answer));
    }
}
