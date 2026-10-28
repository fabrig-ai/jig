use super::fixtures;
use crate::{Bounds, Hello, Reason};
use alloc::boxed::Box;
use skein_channel::{Direction, Role, Term};
use skein_lib::{Duration, List, Reader};

// Existing hello 178 + list count 4 + the independent 64 endpoint identities.
const HELLO_MAX: u32 = 178 + 4 + crate::v9::CEILINGS.bounds_program_endpoints * 4;
fn hello_schema() -> skein_channel::Schema {
    crate::schema(&fixtures::LIMITS).expect("small schema")
}

fn hello_terms(largest: u32) -> List<Term> {
    let mut terms = List::with_capacity(11);
    terms.push(Term { kind: 0x0100, largest }).expect("hello term slot");
    // Hand-counted bounds for Turn, Call, Answer, Fact and the three notices:
    // a refusal's code and reason each take a length and sixteen bytes.
    for (kind, largest) in [
        (0x0101, 96),
        (0x0102, 101),
        (0x0103, 145),
        (0x0104, 398),
        (0x0105, 28),
        (0x0106, 29),
        (0x0107, 60),
        (0x0108, 150),
        (0x0109, 58),
    ] {
        terms.push(Term { kind, largest }).expect("one term per up kind");
    }
    terms
}

fn down_terms() -> List<Term> {
    let mut terms = List::with_capacity(12);
    // Independently summed from field widths and the fixture limits.
    for (kind, largest) in [
        (0x0180, 567),
        (0x0181, 81),
        (0x0182, 74),
        (0x0183, 20),
        (0x0184, 20),
        (0x0185, 16),
        (0x0186, 16),
        (0x0187, 74),
        (0x0188, 76),
        (0x0189, 36),
        (0x018a, 36),
        (0x018b, 16),
    ] {
        terms.push(Term { kind, largest }).expect("one term per down kind");
    }
    terms
}

fn no_terms() -> List<Term> {
    List::with_capacity(0)
}

fn hello_through_bytes(hello: &Hello) -> Hello {
    let bytes = fixtures::encode(hello);
    Hello::decode(&fixtures::LIMITS, &mut Reader::new(&bytes)).expect("a hello within limits")
}

#[test]
fn a_hello_keeps_its_agent_bounds_through_bytes() {
    let hello = fixtures::hello_fullest();
    let decoded = hello_through_bytes(&hello);
    let bounds = decoded.bounds();
    let small = bounds.models().get(0).expect("the first model");
    assert_eq!(bounds.models().len(), 2);
    assert_eq!(small.name(), b"small");
    assert_eq!(small.window(), 8192);
    assert_eq!(small.output(), 1024);
    assert_eq!(bounds.turns(), 40);
    assert_eq!(bounds.spend(), 100);
    assert_eq!(bounds.time(), Duration::from_secs(300));
    assert_eq!(bounds.waiting(), Duration::from_secs(60));
    assert_eq!(bounds.checks(), Duration::from_secs(20));
    assert_eq!(bounds.message_bytes(), 32);
    assert_eq!(bounds.inbox(), 2);
    assert_eq!(bounds.unacknowledged(), 2);
    assert_eq!(decoded.stop_bound(), Duration::from_secs(95));
    assert_eq!(decoded, hello);
}

#[test]
fn a_smallest_hello_crosses_bytes() {
    let hello = fixtures::hello_smallest();
    assert_eq!(hello_through_bytes(&hello), hello);
}

#[test]
fn a_credential_debug_shows_the_secret_length_only() {
    let credential = fixtures::credential(fixtures::WORKER_A, b"hidden-key");
    assert_eq!(alloc::fmt::format(format_args!("{:?}", credential.secret())), "Secret { len: 10 }");
    assert!(!alloc::fmt::format(format_args!("{credential:?}")).contains("hidden-key"));
}

#[test]
fn a_credential_keeps_its_secret_through_bytes() {
    let credential = fixtures::credential(fixtures::WORKER_B, b"private");
    let mut writer = skein_lib::Writer::new(usize::try_from(credential.measure()).expect("small credential"));
    credential.encode(&mut writer).expect("measured room");
    let bytes = writer.finish();
    let decoded = crate::Credential::decode(&fixtures::LIMITS, &mut Reader::new(&bytes)).expect("credential");
    assert_eq!(decoded, credential);
}

#[test]
fn a_hello_one_model_past_its_list_bound_is_refused() {
    let mut parts = fixtures::hello_fullest().into_parts();
    let mut bounds = parts.bounds.into_parts();
    let mut models = List::with_capacity(3);
    for _ in 0_u32..3 {
        models.push(fixtures::model()).expect("three fixture slots");
    }
    bounds.models = models;
    parts.bounds = Bounds::new(&crate::CEILINGS, bounds).expect("three models fit the ceilings");
    assert_eq!(
        Hello::new(&fixtures::LIMITS, parts).expect_err("third model refused").reason,
        skein_codec::Reason::Bound
    );
}

fn append_hello_program_endpoint(hello: &Hello, bytes: &[u8], endpoint: u32) -> Box<[u8]> {
    // The endpoint list is the hello's final record: a u32 count followed by u32 identities.
    const U32_BYTES: u32 = 4;
    let endpoints = hello.bounds().program_endpoints();
    let identities_bytes = endpoints.len().checked_mul(U32_BYTES).expect("small endpoint list");
    let record_bytes = U32_BYTES.checked_add(identities_bytes).expect("count and identities");
    let prefix_bytes = hello.measure().checked_sub(record_bytes).expect("trailing endpoint list");
    let mut reader = Reader::new(bytes);
    let prefix = reader.bytes(prefix_bytes).expect("hello before its endpoint list");
    let count = reader.u32().expect("endpoint count");
    assert_eq!(count, endpoints.len(), "edit exactly the hello's endpoint count");
    let extra_bytes = usize::try_from(U32_BYTES).expect("u32 width");
    let edited_bytes = bytes.len().checked_add(extra_bytes).expect("one extra identity");
    let mut writer = skein_lib::Writer::new(edited_bytes);
    writer.put(prefix).expect("unchanged hello prefix");
    let raised_count = count.checked_add(1).expect("one extra endpoint");
    writer.put(&raised_count.to_be_bytes()).expect("raised endpoint count");
    for _ in 0..count {
        let identity = reader.u32().expect("recorded endpoint identity");
        writer.put(&identity.to_be_bytes()).expect("unchanged endpoint identity");
    }
    assert!(reader.is_empty(), "the endpoint list is the hello's final record");
    writer.put(&endpoint.to_be_bytes()).expect("one appended endpoint");
    writer.finish()
}

#[test]
fn a_hello_past_the_endpoint_ceiling_is_refused_at_its_bound() {
    // hosts.md, 8.1: 64 distinct endpoint identities per hello.
    const ENDPOINT_CAP: u32 = 64;
    let limits = crate::sizes::limits(&fixtures::SIZES_ONE_SLOT).expect("real one-slot codec limits");
    let mut hello = fixtures::hello_smallest().into_parts();
    let mut bounds = hello.bounds.into_parts();
    let mut endpoints = List::with_capacity(ENDPOINT_CAP);
    for endpoint in 0..ENDPOINT_CAP {
        endpoints.push(endpoint).expect("one slot per endpoint");
    }
    bounds.program_endpoints = endpoints;
    hello.bounds = Bounds::new(&fixtures::LIMITS, bounds).expect("endpoint cap fits");
    let hello = Hello::new(&fixtures::LIMITS, hello).expect("hello at the endpoint cap");
    let at_cap = fixtures::encode(&hello);
    let past_cap = append_hello_program_endpoint(&hello, &at_cap, ENDPOINT_CAP);
    for (bytes, expected) in [
        (at_cap, Ok(ENDPOINT_CAP)),
        (
            past_cap,
            Err(crate::v9::Problem {
                path: crate::v9::Path::BoundsProgramEndpoints,
                reason: skein_codec::Reason::Bound,
            }),
        ),
    ] {
        let decoded = match Hello::decode(&limits, &mut Reader::new(&bytes)) {
            Ok(hello) => Ok(hello.bounds().program_endpoints().len()),
            Err(problem) => Err(problem),
        };
        assert_eq!(decoded, expected);
    }
}

#[test]
fn every_reason_reads_back_from_its_code() {
    for (reason, code) in [
        (Reason::Unauthorized, 256),
        (Reason::Application, 257),
        (Reason::Busy, 258),
        (Reason::Replaced, 259),
        (Reason::NotAdmitted, 260),
        (Reason::Stopping, 261),
    ] {
        assert_eq!(reason.code(), code);
        assert_eq!(Reason::from_code(code), Some(reason));
    }
}

#[test]
fn a_reason_below_256_reads_as_none() {
    for code in 0..256 {
        assert_eq!(Reason::from_code(code), None);
    }
}

#[test]
fn the_schema_lists_the_hello_from_the_initiator() {
    let schema = hello_schema();
    let version = schema.version(9).expect("one version");
    assert_eq!(schema.magic, *b"jigw");
    assert_eq!(version.kinds.len(), 22);
    assert_eq!(
        version.kind(0x0100),
        Some(skein_channel::Kind { kind: 0x0100, direction: Direction::FromInitiator, largest: HELLO_MAX })
    );
}

#[test]
fn terms_that_take_every_kind_leave_no_gap() {
    assert_eq!(crate::terms_gap(&hello_schema(), Role::Initiator, &hello_terms(HELLO_MAX)), None);
}

#[test]
fn terms_below_the_hello_bound_name_the_hello_kind() {
    assert_eq!(crate::terms_gap(&hello_schema(), Role::Initiator, &hello_terms(HELLO_MAX - 1)), Some(0x0100));
}

#[test]
fn terms_without_the_hello_name_the_hello_kind() {
    assert_eq!(crate::terms_gap(&hello_schema(), Role::Initiator, &no_terms()), Some(0x0100));
}

#[test]
fn the_responder_needs_no_sending_hello_term() {
    assert_eq!(crate::terms_gap(&hello_schema(), Role::Responder, &down_terms()), None);
}

fn answer_through_bytes(answer: &crate::Answer) -> crate::Answer {
    let mut writer = skein_lib::Writer::new(usize::try_from(answer.measure()).expect("small answer"));
    answer.encode(&mut writer).expect("measured room");
    crate::Answer::decode(&fixtures::LIMITS, &mut Reader::new(&writer.finish())).expect("answer within limits")
}

fn fact_through_bytes(fact: &crate::Fact) -> crate::Fact {
    let mut writer = skein_lib::Writer::new(usize::try_from(fact.measure()).expect("small fact"));
    fact.encode(&mut writer).expect("measured room");
    crate::Fact::decode(&fixtures::LIMITS, &mut Reader::new(&writer.finish())).expect("fact within limits")
}

#[test]
fn a_turn_keeps_its_refusals_and_window_flag_through_bytes() {
    let turn = fixtures::turn_with_refusals();
    let mut writer = skein_lib::Writer::new(usize::try_from(turn.measure()).expect("small turn"));
    turn.encode(&mut writer).expect("measured room");
    let decoded = crate::Turn::decode(&fixtures::LIMITS, &mut Reader::new(&writer.finish())).expect("turn");
    assert!(decoded.opens_window());
    assert_eq!(decoded.refused().len(), 2);
    let first = decoded.refused().get(0).expect("first refusal");
    let second = decoded.refused().get(1).expect("second refusal");
    assert_eq!(first.name(), fixtures::MESSAGE_FIRST);
    assert_eq!(first.reason(), &crate::MessageRefusal::TooLarge);
    assert_eq!(second.name(), fixtures::MESSAGE_SECOND);
    assert_eq!(second.reason(), &crate::MessageRefusal::Ending);
    assert_eq!(decoded, turn);
}

#[test]
fn a_turn_past_the_inbox_refusal_bound_is_refused() {
    let mut parts = fixtures::turn_with_refusals().into_parts();
    let mut refused = List::with_capacity(3);
    for name in [41, 42, 43] {
        refused.push(fixtures::refusal(name, crate::MessageRefusal::Full)).expect("three fixture slots");
    }
    parts.refused = refused;
    assert_eq!(
        crate::Turn::new(&fixtures::LIMITS, parts).expect_err("third refusal refused").reason,
        skein_codec::Reason::Bound
    );
}

#[test]
fn an_answer_keeps_its_unread_fence_through_bytes() {
    let answer = fixtures::answer_ended();
    let decoded = answer_through_bytes(&answer);
    assert_eq!(decoded.read(), &Some(fixtures::MESSAGE_FIRST));
    assert_eq!(decoded.refused().len(), 2);
    assert_eq!(decoded, answer);
}

#[test]
fn a_failed_answer_keeps_its_failure_kind_and_detail() {
    let answer = fixtures::answer_failed();
    let decoded = answer_through_bytes(&answer);
    match decoded.ending() {
        crate::Ending::Failed(failed) => {
            assert_eq!(failed.detail(), b"bounded-detail");
            match failed.failure() {
                crate::Failure::Run(run) => assert_eq!(run.value(), &crate::RunFailure::Model),
                other @ (crate::Failure::Unprepared(_) | crate::Failure::Agent(_) | crate::Failure::Cancelled(_)) => {
                    panic!("unexpected failure {other:?}")
                }
            }
        }
        other @ (crate::Ending::Refused(_) | crate::Ending::Ended(_) | crate::Ending::Parked(_)) => {
            panic!("unexpected ending {other:?}")
        }
    }
    assert_eq!(decoded, answer);
}

fn failed_through_bytes(failure: crate::RunFailure) -> crate::RunFailure {
    let mut parts = fixtures::answer_failed().into_parts();
    let run = crate::RunFailureValue::new(&fixtures::LIMITS, crate::RunFailureValueParts { value: failure })
        .expect("typed run failure");
    let crate::Ending::Failed(failed) = parts.ending else { panic!("the fixture answer failed") };
    let mut failed = failed.into_parts();
    failed.failure = crate::Failure::Run(run);
    parts.ending = crate::Ending::Failed(crate::Failed::new(&fixtures::LIMITS, failed).expect("failed answer"));
    let answer = crate::Answer::new(&fixtures::LIMITS, parts).expect("answer within limits");
    let decoded = answer_through_bytes(&answer);
    assert_eq!(decoded, answer);
    let crate::Ending::Failed(failed) = decoded.into_parts().ending else { panic!("a failed answer stays failed") };
    let crate::Failure::Run(run) = failed.into_parts().failure else { panic!("a run failure stays one") };
    run.into_parts().value
}

#[test]
fn a_failed_answer_keeps_the_limited_or_refused_account_it_turned_on() {
    for reset in [Some(Duration::from_secs(30)), None] {
        let limited = crate::LimitedAccount::new(&fixtures::LIMITS, crate::LimitedAccountParts { account: 7, reset })
            .expect("limited account");
        let decoded = failed_through_bytes(crate::RunFailure::Limited(limited));
        let crate::RunFailure::Limited(limited) = decoded else { panic!("limited stays limited: {decoded:?}") };
        assert_eq!((limited.account(), limited.reset()), (7, &reset));
    }
    let refused = crate::RefusedAccount::new(&fixtures::LIMITS, crate::RefusedAccountParts { account: 7 })
        .expect("refused account");
    let decoded = failed_through_bytes(crate::RunFailure::Refused(refused));
    let crate::RunFailure::Refused(refused) = decoded else { panic!("refused stays refused: {decoded:?}") };
    assert_eq!(refused.account(), 7);
}

fn through_bytes<T>(
    value: &T,
    measure: fn(&T) -> u32,
    encode: fn(&T, &mut skein_lib::Writer) -> Result<(), skein_lib::Overflow>,
    decode: fn(&crate::v9::Limits, &mut Reader<'_>) -> Result<T, crate::v9::Problem>,
) -> T {
    let mut writer = skein_lib::Writer::new(usize::try_from(measure(value)).expect("small notice"));
    encode(value, &mut writer).expect("measured room");
    decode(&fixtures::LIMITS, &mut Reader::new(&writer.finish())).expect("notice within limits")
}

#[test]
fn an_unauthorized_notice_keeps_its_account_and_generation_through_bytes() {
    let parts = crate::UnauthorizedParts { task: 3, attempt: 4, account: 7, generation: 2 };
    let notice = crate::Unauthorized::new(&fixtures::LIMITS, parts).expect("unauthorized notice");
    let decoded =
        through_bytes(&notice, crate::Unauthorized::measure, crate::Unauthorized::encode, crate::Unauthorized::decode);
    assert_eq!((decoded.task(), decoded.attempt(), decoded.account(), decoded.generation()), (3, 4, 7, 2));
    assert_eq!(decoded, notice);
}

#[test]
fn a_limited_notice_keeps_its_reset_or_none_through_bytes() {
    for reset in [Some(Duration::from_secs(30)), None] {
        let parts = crate::LimitedParts { task: 3, attempt: 4, account: 7, reset };
        let notice = crate::Limited::new(&fixtures::LIMITS, parts).expect("limited notice");
        let decoded = through_bytes(&notice, crate::Limited::measure, crate::Limited::encode, crate::Limited::decode);
        assert_eq!((decoded.account(), decoded.reset()), (7, &reset));
        assert_eq!(decoded, notice);
    }
}

fn refused_parts(code: &[u8], reason: &[u8]) -> crate::AccountRefusedParts {
    crate::AccountRefusedParts { task: 3, attempt: 4, account: 7, code: code.into(), reason: reason.into() }
}

#[test]
fn an_account_refused_notice_keeps_its_code_and_reason_through_bytes() {
    let notice = crate::AccountRefused::new(&fixtures::LIMITS, refused_parts(b"forbidden", b"suspended"))
        .expect("refused notice");
    let decoded = through_bytes(
        &notice,
        crate::AccountRefused::measure,
        crate::AccountRefused::encode,
        crate::AccountRefused::decode,
    );
    assert_eq!((decoded.account(), decoded.code(), decoded.reason()), (7, &b"forbidden"[..], &b"suspended"[..]));
    assert_eq!(decoded, notice);
}

#[test]
fn an_account_refused_code_or_reason_past_its_bound_or_not_text_is_refused() {
    let full = [b'x'; 16];
    let past = [b'x'; 17];
    for (code, reason, problem) in [
        (&past[..], &full[..], skein_codec::Reason::Bound),
        (&full[..], &past[..], skein_codec::Reason::Bound),
        (&b"\xff"[..], &full[..], skein_codec::Reason::Utf8),
        (&full[..], &b"\xff"[..], skein_codec::Reason::Utf8),
    ] {
        let refused = crate::AccountRefused::new(&fixtures::LIMITS, refused_parts(code, reason));
        assert_eq!(refused.expect_err("refused").reason, problem);
    }
    // The decoder holds the same bound: a seventeen-byte reason written under
    // the ceilings reads back refused under the fixture's sixteen.
    let wide = crate::AccountRefused::new(&crate::CEILINGS, refused_parts(&full, &past)).expect("within the ceilings");
    let mut writer = skein_lib::Writer::new(usize::try_from(wide.measure()).expect("small notice"));
    wide.encode(&mut writer).expect("measured room");
    let decoded = crate::AccountRefused::decode(&fixtures::LIMITS, &mut Reader::new(&writer.finish()));
    assert_eq!(decoded.expect_err("past the fixture bound").reason, skein_codec::Reason::Bound);
}

#[test]
fn a_count_fact_carries_kind_time_and_count() {
    let fact = fixtures::fact_count();
    let decoded = fact_through_bytes(&fact);
    match decoded.value() {
        crate::FactValue::Count(count) => {
            assert_eq!(count.kind(), &crate::CountKind::LlmRetried);
            assert_eq!(count.elapsed(), Duration::from_secs(2));
            assert_eq!(count.value(), 3);
        }
        other @ (crate::FactValue::Phase(_)
        | crate::FactValue::Long(_)
        | crate::FactValue::LongDone
        | crate::FactValue::Piece(_)) => {
            panic!("unexpected activity {other:?}")
        }
    }
    assert_eq!(decoded, fact);
}

#[test]
fn a_long_operation_fact_keeps_its_span() {
    let fact = fixtures::fact_long();
    let decoded = fact_through_bytes(&fact);
    match decoded.value() {
        crate::FactValue::Long(long) => {
            assert_eq!(long.operation(), &crate::Operation::Checks);
            assert_eq!(long.span(), Duration::from_secs(6));
        }
        other @ (crate::FactValue::Phase(_)
        | crate::FactValue::Count(_)
        | crate::FactValue::LongDone
        | crate::FactValue::Piece(_)) => {
            panic!("unexpected activity {other:?}")
        }
    }
    assert_eq!(decoded, fact);
}

#[test]
fn the_schema_lists_every_up_kind_from_the_initiator() {
    let schema = hello_schema();
    assert_eq!(schema.versions.len(), 1);
    for version in &schema.versions {
        assert_eq!(version.kinds.len(), 22);
        for number in [0x0100, 0x0101, 0x0102, 0x0103, 0x0104, 0x0105, 0x0106, 0x0107, 0x0108, 0x0109] {
            assert_eq!(version.kind(number).expect("every up kind").direction, Direction::FromInitiator);
        }
    }
}

fn assignment_through_bytes(assignment: &crate::Assignment) -> crate::Assignment {
    let mut writer = skein_lib::Writer::new(usize::try_from(assignment.measure()).expect("small assignment"));
    assignment.encode(&mut writer).expect("measured room");
    crate::Assignment::decode(&fixtures::LIMITS, &mut Reader::new(&writer.finish())).expect("assignment within limits")
}

fn grant_through_bytes(grant: &crate::Grant) -> crate::Grant {
    let mut writer = skein_lib::Writer::new(usize::try_from(grant.measure()).expect("small grant"));
    grant.encode(&mut writer).expect("measured room");
    crate::Grant::decode(&fixtures::LIMITS, &mut Reader::new(&writer.finish())).expect("grant within limits")
}

#[test]
fn an_assignment_keeps_its_start_messages_in_order() {
    let assignment = fixtures::assignment_resumed();
    let decoded = assignment_through_bytes(&assignment);
    assert_eq!(decoded.messages().len(), 2);
    assert_eq!(decoded.messages().get(0).expect("first start message").name(), fixtures::MESSAGE_FIRST);
    assert_eq!(decoded.messages().get(1).expect("second start message").name(), fixtures::MESSAGE_SECOND);
    assert!(!decoded.messages().get(0).expect("ordinary").finish_requested());
    assert!(decoded.messages().get(1).expect("finish").finish_requested());
    assert_eq!(decoded, assignment);
}

#[test]
fn an_assignment_keeps_its_sixteen_byte_identity() {
    let decoded = assignment_through_bytes(&fixtures::assignment_resumed());
    assert_eq!(decoded.identity(), &fixtures::IDENTITY_FIRST);
}

fn every_grant_value() -> [crate::GrantValue; 4] {
    [
        fixtures::credential_value(crate::Scheme::Bearer, fixtures::TOKEN_ONE),
        fixtures::credential_value(crate::Scheme::Key, fixtures::KEY_ONE),
        fixtures::application_value(),
        crate::GrantValue::Program(
            crate::ProgramValue::new(&fixtures::LIMITS, crate::ProgramValueParts { name: b"claude".as_slice().into() })
                .expect("source name"),
        ),
    ]
}

#[test]
fn each_grant_value_round_trips() {
    for value in every_grant_value() {
        let grant = fixtures::grant(value.clone(), 5, 9);
        let decoded = grant_through_bytes(&grant);
        assert_eq!(decoded.value(), &value);
        assert_eq!(decoded, grant);
    }
}

#[test]
fn a_credential_grant_keeps_its_scheme_with_its_bytes() {
    for (scheme, bytes) in [(crate::Scheme::Bearer, fixtures::TOKEN_ONE), (crate::Scheme::Key, fixtures::KEY_ONE)] {
        let decoded = grant_through_bytes(&fixtures::grant(fixtures::credential_value(scheme.clone(), bytes), 5, 9));
        let crate::GrantValue::Credential(value) = decoded.value() else { panic!("a credential: {decoded:?}") };
        assert_eq!((value.scheme(), value.bytes().expose()), (&scheme, bytes));
    }
}

fn granted_debug(value: crate::GrantValue) -> Box<[u8]> {
    let record = crate::Granted::new(
        &fixtures::LIMITS,
        crate::GrantedParts { task: 3, attempt: 4, grant: fixtures::grant(value, 5, 9) },
    )
    .expect("granted record");
    alloc::fmt::format(format_args!("{record:?}")).into_bytes().into_boxed_slice()
}

#[test]
fn a_granted_record_debug_shows_the_credentials_length_only() {
    let debug = granted_debug(fixtures::credential_value(crate::Scheme::Key, fixtures::KEY_ONE));
    assert!(skein_lib::bytes::find(&debug, b"len: 7").is_some());
    assert_eq!(skein_lib::bytes::find(&debug, fixtures::KEY_ONE), None);
    assert_eq!(skein_lib::bytes::find(&debug, b"107, 101, 121, 45, 111, 110, 101"), None);
}

#[test]
fn a_relayed_busy_answer_carries_no_text() {
    let record = crate::Relayed::new(
        &fixtures::LIMITS,
        crate::RelayedParts {
            task: 3,
            attempt: 4,
            name: b"call-one".as_slice().into(),
            answer: crate::RelayAnswer::Busy,
        },
    )
    .expect("busy answer");
    let mut writer = skein_lib::Writer::new(usize::try_from(record.measure()).expect("small relayed"));
    record.encode(&mut writer).expect("measured room");
    let bytes = writer.finish();
    assert_eq!(bytes.len(), 29);
    let decoded = crate::Relayed::decode(&fixtures::LIMITS, &mut Reader::new(&bytes)).expect("relayed within limits");
    assert_eq!(decoded.answer(), &crate::RelayAnswer::Busy);
}

#[test]
fn an_assignment_past_its_grant_bound_is_refused() {
    let mut parts = fixtures::assignment_resumed().into_parts();
    let mut grants = List::with_capacity(3);
    for number in [5, 6, 7] {
        grants.push(fixtures::grant(fixtures::application_value(), number, 9)).expect("three fixture slots");
    }
    parts.grants = grants;
    assert_eq!(
        crate::Assignment::new(&fixtures::LIMITS, parts).expect_err("third grant refused").reason,
        skein_codec::Reason::Bound
    );
}

#[test]
fn the_schema_lists_every_down_kind_from_the_responder() {
    let schema = hello_schema();
    let version = schema.version(9).expect("one version");
    assert_eq!(version.kinds.len(), 22);
    for term in &down_terms() {
        assert_eq!(
            version.kind(term.kind),
            Some(skein_channel::Kind { kind: term.kind, direction: Direction::FromResponder, largest: term.largest })
        );
    }
}

#[test]
fn an_assignment_keeps_its_transcript_piece_opaque() {
    let decoded = assignment_through_bytes(&fixtures::assignment_resumed());
    assert_eq!(decoded.transcript(), b"\0\0\0\x08turn-one\0\0\0\x08turn-two");
}
