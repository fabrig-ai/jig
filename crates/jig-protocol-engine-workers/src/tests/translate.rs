use super::fixtures;
use crate::translate;
use skein_lib::{Duration, Token};
fn translated(record: translate::Up) -> (jig_domain_host::LinkUp, translate::Parts) {
    let mut parts = translate::Parts::new(4);
    let link = translate::up(record, &mut parts);
    (link, parts)
}
fn up(record: translate::Up) -> jig_domain_host::LinkUp {
    let (link, parts) = translated(record);
    assert!(parts.is_empty(), "this record has no application parts");
    link
}
fn answer(record: jig_protocol_worker_wire::Answer) -> (jig_domain_host::Answer, translate::Parts) {
    let (link, parts) = translated(translate::Up::Answer(record));
    let jig_domain_host::LinkUp::Answer { run, attempt, answer } = link else {
        panic!("an answer");
    };
    assert_eq!((run, attempt), (Token::new(7), Token::new(2)));
    (answer, parts)
}
#[test]
fn a_hello_record_becomes_the_hubs_hello_with_its_bounds() {
    let hello = translate::hello(fixtures::hello_record()).expect("valid hello");
    assert_eq!(hello.slots, 1);
    assert_eq!(hello.stop_bound, Duration::from_secs(20));
    assert_eq!(hello.workstreams.as_ref(), &[9]);
    assert_eq!(
        hello.hosting.as_ref(),
        &[jig_domain_host::Hosting {
            run: Token::new(7),
            attempt: Token::new(2),
            phase: jig_domain_host::Phase::Waiting
        }]
    );
    let model = hello.bounds.models.first().expect("one model");
    assert_eq!(model.name.as_ref(), b"model");
    assert_eq!(model.window, 8192);
    assert_eq!(model.output, 1024);
    assert_eq!(hello.bounds.turns, 8);
    assert_eq!(hello.bounds.spend, 100);
    assert_eq!(hello.bounds.time, Duration::from_secs(60));
    assert_eq!(hello.bounds.waiting, Duration::from_secs(10));
    assert_eq!(hello.bounds.checks, Duration::from_secs(2));
    assert_eq!(hello.bounds.message_bytes, 16);
    assert_eq!(hello.bounds.inbox, 1);
    assert_eq!(hello.bounds.unacknowledged, 1);
}

#[test]
fn a_turn_record_becomes_the_hubs_turn_with_its_refusals() {
    let link = up(translate::Up::Turn(fixtures::turn_record()));
    let jig_domain_host::LinkUp::Turn { run, attempt, turn } = link else {
        panic!("a turn");
    };
    assert_eq!((run, attempt), (Token::new(7), Token::new(2)));
    assert!(turn.opens_window, "window marker kept");
    assert_eq!(
        turn.refused.as_ref(),
        &[jig_domain_host::Refused { name: Token::new(8), reason: jig_domain_host::MessageRefusal::Full }]
    );
    assert_eq!(turn.read, Some(Token::new(9)));
    assert_eq!(turn.body.as_ref(), b"opaque-turn");
}
#[test]
fn a_fact_record_becomes_the_hubs_activity() {
    assert_eq!(
        up(translate::Up::Fact(fixtures::fact_record())),
        jig_domain_host::LinkUp::Activity {
            run: Token::new(7),
            attempt: Token::new(2),
            activity: jig_domain_host::Activity::Count {
                kind: jig_domain_host::CountKind::LlmStarted,
                elapsed: Duration::from_secs(2),
                count: 3
            }
        }
    );
}
#[test]
fn each_account_notice_record_becomes_its_link_notice() {
    let (run, attempt) = (Token::new(7), Token::new(2));
    assert_eq!(
        up(translate::Up::Unauthorized(fixtures::unauthorized_record())),
        jig_domain_host::LinkUp::Unauthorized { run, attempt, account: 1, generation: 4 }
    );
    for reset in [Some(Duration::from_secs(5)), None] {
        assert_eq!(
            up(translate::Up::Limited(fixtures::limited_record(reset))),
            jig_domain_host::LinkUp::Limited { run, attempt, account: 1, reset }
        );
    }
    assert_eq!(
        up(translate::Up::AccountRefused(fixtures::account_refused_record())),
        jig_domain_host::LinkUp::AccountRefused {
            run,
            attempt,
            account: 1,
            code: Box::from(b"forbidden".as_slice()),
            reason: Box::from(b"suspended".as_slice()),
        }
    );
}

#[test]
fn a_failed_answer_keeps_the_limited_or_refused_account_it_turned_on() {
    use jig_domain_host::RunFailure;
    for (record, failure) in [
        (
            fixtures::limited_account(Some(Duration::from_secs(5))),
            RunFailure::Limited { account: 1, reset: Some(Duration::from_secs(5)) },
        ),
        (fixtures::limited_account(None), RunFailure::Limited { account: 1, reset: None }),
        (fixtures::refused_account(), RunFailure::Refused { account: 1 }),
    ] {
        let (answer, _parts) = answer(fixtures::answer_failed_on(record));
        let jig_domain_host::Ending::Failed { failure: found, .. } = answer.ending else {
            panic!("failed");
        };
        assert_eq!(found, jig_domain_host::Failure::Run(failure));
    }
}

#[test]
fn an_answer_record_keeps_its_unread_fence() {
    let (answer, parts) = answer(fixtures::answer_record());
    drop(parts);
    assert_eq!(answer.read, Some(Token::new(9)));
    assert_eq!(
        answer.refused.as_ref(),
        &[jig_domain_host::Refused { name: Token::new(8), reason: jig_domain_host::MessageRefusal::Full }]
    );
}
#[test]
fn an_answer_hands_left_and_saved_work_up_as_parts() {
    let (answer, parts) = answer(fixtures::answer_record());
    let jig_domain_host::Ending::Ended { work, .. } = answer.ending else {
        panic!("ended");
    };
    assert_eq!(work.left, Some(Token::new(4)));
    assert_eq!(work.saved, Some(Token::new(5)));
    assert!(!parts.is_empty(), "application bytes extracted");
    assert_eq!(parts.next(), 6);
    let parts = parts.finish();
    assert_eq!(parts.first().expect("left part").bytes.as_ref(), b"left");
    assert_eq!(parts.get(1).expect("saved part").bytes.as_ref(), b"saved");
}
#[test]
fn a_call_record_keeps_its_name_bytes() {
    let link = up(translate::Up::Call(fixtures::call_record()));
    let jig_domain_host::LinkUp::Call { run, attempt, call, tool, writes, input, deadline } = link else {
        panic!("call");
    };
    assert_eq!(call.as_ref(), &[0, 2, 7]);
    assert_eq!((run, attempt), (Token::new(7), Token::new(2)));
    assert_eq!(tool.as_ref(), b"read");
    assert!(!writes, "write bit kept");
    assert_eq!(input.as_ref(), b"input");
    assert_eq!(deadline, Duration::from_secs(4));
}

#[test]
fn a_failed_answers_resource_precedes_its_left_and_saved_parts() {
    let (answer, parts) = answer(fixtures::answer_with_resource());
    let jig_domain_host::Ending::Failed { failure, detail, work } = answer.ending else {
        panic!("failed");
    };
    assert_eq!(
        failure,
        jig_domain_host::Failure::Unprepared(jig_domain_host::Preparation::Permanent { resource: Some(Token::new(4)) })
    );
    assert_eq!(detail.as_ref(), b"detail");
    assert_eq!(work.left, Some(Token::new(5)));
    assert_eq!(work.saved, Some(Token::new(6)));
    assert_eq!(parts.next(), 7);
    let parts = parts.finish();
    assert_eq!(parts[0].bytes.as_ref(), b"resource");
    assert_eq!(parts[1].bytes.as_ref(), b"left");
    assert_eq!(parts[2].bytes.as_ref(), b"saved");
}

fn down(record: jig_domain_host::LinkDown) -> translate::Down {
    use jig_domain_host::LinkDown;
    let translated = match record {
        LinkDown::Assign { .. } | LinkDown::Grant { .. } => {
            translate::down(record, &fixtures::parts(), fixtures::values(), &codec())
        }
        LinkDown::Sources { .. }
        | LinkDown::Check { .. }
        | LinkDown::SignOut { .. }
        | LinkDown::Message { .. }
        | LinkDown::Relayed { .. }
        | LinkDown::TurnKept { .. }
        | LinkDown::Acknowledge { .. }
        | LinkDown::Cancel { .. }
                    | LinkDown::Park { .. } => translate::down(record, &[], Box::new([]), &codec()),
    };
    translated.expect("complete output")
}
fn codec() -> jig_protocol_worker_wire::v9::Limits {
    jig_protocol_worker_wire::sizes::limits(&fixtures::SIZES).expect("small codec")
}
fn assignment_output() -> jig_domain_host::LinkDown {
    jig_domain_host::LinkDown::Assign { assignment: fixtures::assignment() }
}
fn assigned() -> jig_protocol_worker_wire::Assignment {
    assignment_record(down(assignment_output()))
}
fn assignment_record(record: translate::Down) -> jig_protocol_worker_wire::Assignment {
    let translate::Down::Assign(record) = record else {
        panic!("assignment");
    };
    record
}
#[test]
fn an_assignment_keeps_its_start_messages_and_identity() {
    let fields = assigned().into_parts();
    assert_eq!((fields.task, fields.attempt), (7, 2));
    assert_eq!(fields.identity, fixtures::IDENTITY);
    assert_eq!(fields.messages.len(), 1);
    let message = &fields.messages.get(0).expect("message");
    assert_eq!(message.name(), 8);
    assert_eq!(message.sender(), b"sender");
    assert_eq!(message.words(), b"message");
    assert!(message.finish_requested());
}
#[test]
fn an_assignments_items_come_from_their_part() {
    let fields = assigned().into_parts();
    let workspace = fields.workspace.expect("workspace");
    assert_eq!(workspace.workstream(), 9);
    assert_eq!(workspace.items(), b"items");
    assert!(fields.save);
}
#[test]
fn a_missing_workspace_part_drops_the_assignment() {
    assert!(translate::down(assignment_output(), &[], fixtures::values(), &codec(),).is_none());
}
#[test]
fn an_assignments_window_is_one_piece_with_each_turns_length() {
    assert_eq!(assigned().transcript(), &[0, 0, 0, 2, 0, 127, 0, 0, 0, 3, 255, 1, 2]);
}
#[test]
fn a_transcript_past_its_bound_including_lengths_drops_the_assignment() {
    let mut limits = codec();
    limits.assignment_transcript = 12;
    assert!(translate::down(assignment_output(), &fixtures::parts(), fixtures::values(), &limits,).is_none());
}
#[test]
fn a_fresh_assignment_has_an_empty_transcript_piece() {
    let mut assignment = fixtures::assignment();
    assignment.turns = Box::new([]);
    let record = assignment_record(down(jig_domain_host::LinkDown::Assign { assignment }));
    assert!(record.transcript().is_empty());
}
#[test]
fn a_grant_without_a_value_is_left_out() {
    let record = assignment_record(
        translate::down(
            assignment_output(),
            &fixtures::parts(),
            Box::new([None, fixtures::values()[1].take()]),
            &codec(),
        )
        .expect("assignment"),
    );
    assert_eq!(record.grants().len(), 1);
    let jig_protocol_worker_wire::GrantValue::Application(value) = assignment_grant(&record).value() else {
        panic!("application value")
    };
    assert_eq!(value.secret().expose(), fixtures::TOKEN_TWO);
}
fn granted(form: jig_domain_host::Form, value: crate::GrantValue) -> Option<translate::Down> {
    translate::down(
        jig_domain_host::LinkDown::Grant { run: Token::new(7), attempt: Token::new(2), grant: fixtures::grant(form) },
        &[],
        Box::new([Some(value)]),
        &codec(),
    )
}
fn granted_value(down: Option<translate::Down>) -> jig_protocol_worker_wire::Grant {
    let Some(translate::Down::Grant(record)) = down else { panic!("granted") };
    assert_eq!((record.task(), record.attempt()), (7, 2));
    assert_eq!((record.grant().number(), record.grant().generation()), (1, 4));
    record.grant().clone()
}
#[test]
fn each_credential_scheme_reaches_the_wire_with_its_token() {
    use jig_domain_host::{Form, Scheme};
    use jig_protocol_worker_wire as wire;
    for (form, scheme, expected) in [
        (Scheme::Bearer, crate::Scheme::Bearer, wire::Scheme::Bearer),
        (Scheme::Key, crate::Scheme::Key, wire::Scheme::Key),
    ] {
        let grant = granted_value(granted(Form::Credential(form), fixtures::credential_value(scheme)));
        let wire::GrantValue::Credential(value) = grant.value() else { panic!("credential value") };
        assert_eq!(value.scheme(), &expected);
        assert_eq!(value.bytes().expose(), fixtures::TOKEN_ONE);
    }
}
#[test]
fn an_application_grant_reaches_the_wire_with_its_secret() {
    let value = fixtures::values()[1].take().expect("application value");
    let grant = granted_value(granted(jig_domain_host::Form::Application, value));
    let jig_protocol_worker_wire::GrantValue::Application(value) = grant.value() else { panic!("application value") };
    assert_eq!(value.secret().expose(), fixtures::TOKEN_TWO);
}
#[test]
fn a_value_unlike_its_grants_form_is_not_sent() {
    use jig_domain_host::{Form, Scheme};
    let bearer = Form::Credential(Scheme::Bearer);
    let key = Form::Credential(Scheme::Key);
    for (form, value) in [
        (key, fixtures::credential_value(crate::Scheme::Bearer)),
        (bearer, fixtures::credential_value(crate::Scheme::Key)),
        (bearer, fixtures::values()[1].take().expect("application")),
        (key, fixtures::values()[1].take().expect("application")),
        (Form::Application, fixtures::credential_value(crate::Scheme::Bearer)),
        (Form::Application, fixtures::credential_value(crate::Scheme::Key)),
    ] {
        assert!(granted(form, value).is_none(), "value unlike {form:?}");
    }
}
#[test]
fn a_standalone_grant_without_a_value_is_not_sent() {
    assert!(
        translate::down(
            jig_domain_host::LinkDown::Grant {
                run: Token::new(7),
                attempt: Token::new(2),
                grant: fixtures::grant(jig_domain_host::Form::Credential(jig_domain_host::Scheme::Bearer)),
            },
            &[],
            Box::new([None]),
            &codec(),
        )
        .is_none()
    );
}
#[test]
fn a_down_event_debug_shows_value_lengths_only() {
    let text = alloc::fmt::format(format_args!("{:?}", fixtures::assignment_down(Token::new(1))));
    for secret in [fixtures::TOKEN_ONE, fixtures::TOKEN_TWO] {
        for window in text.as_bytes().windows(secret.len()) {
            assert_ne!(window, secret);
        }
        assert!(!text.contains(&alloc::fmt::format(format_args!("{secret:?}"))));
    }
    assert!(text.contains("Secret"));
}
#[test]
fn a_relayed_busy_answer_carries_no_text() {
    let translate::Down::Relayed(record) = down(jig_domain_host::LinkDown::Relayed {
        run: Token::new(7),
        attempt: Token::new(2),
        call: Box::from([0, 2, 7]),
        answer: jig_domain_host::Relayed::Busy,
    }) else {
        panic!("relayed");
    };
    assert_eq!((record.task(), record.attempt()), (7, 2));
    assert_eq!(record.name(), &[0, 2, 7]);
    assert_eq!(record.answer(), &jig_protocol_worker_wire::RelayAnswer::Busy);
}
#[test]
fn a_turn_kept_record_names_the_turn() {
    let translate::Down::TurnKept(record) =
        down(jig_domain_host::LinkDown::TurnKept { run: Token::new(7), attempt: Token::new(2), turn: 3 })
    else {
        panic!("kept");
    };
    assert_eq!((record.task(), record.attempt(), record.number()), (7, 2, 3));
}
#[test]
fn an_assignments_committed_call_keeps_its_answer() {
    let fields = assigned().into_parts();
    assert_eq!(fields.answered.len(), 1);
    let call = answered_call(&fields.answered);
    assert_eq!(call.name(), &[0, 2, 7]);
    assert_eq!(call.tool(), b"read");
    let jig_protocol_worker_wire::SettledAnswer::Host(answer) = call.answer() else {
        panic!("host answer");
    };
    assert!(answer.error());
    assert_eq!(answer.body(), b"answer");
}

#[test]
fn a_message_record_keeps_its_name_sender_and_words() {
    for finish_requested in [false, true] {
        let translate::Down::Message(record) = down(jig_domain_host::LinkDown::Message {
            run: Token::new(7),
            attempt: Token::new(2),
            name: Token::new(8),
            sender: Box::from(b"sender".as_slice()),
            words: Box::from(b"message".as_slice()),
            finish_requested,
        }) else {
            panic!("message");
        };
        assert_eq!((record.task(), record.attempt(), record.name()), (7, 2, 8));
        assert_eq!(record.sender(), b"sender");
        assert_eq!(record.words(), b"message");
        assert_eq!(record.finish_requested(), finish_requested);
    }
}
#[test]
fn a_relayed_answer_keeps_its_text_and_error() {
    let translate::Down::Relayed(record) = down(jig_domain_host::LinkDown::Relayed {
        run: Token::new(7),
        attempt: Token::new(2),
        call: Box::from([0, 2, 7]),
        answer: jig_domain_host::Relayed::Answer { error: true, text: Box::from(b"error".as_slice()) },
    }) else {
        panic!("relayed");
    };
    let jig_protocol_worker_wire::RelayAnswer::Text(answer) = record.answer() else {
        panic!("text");
    };
    assert!(answer.error());
    assert_eq!(answer.text(), b"error");
}
#[test]
fn an_acknowledgement_names_its_attempt() {
    let translate::Down::Acknowledge(record) =
        down(jig_domain_host::LinkDown::Acknowledge { run: Token::new(7), attempt: Token::new(2) })
    else {
        panic!("acknowledge");
    };
    assert_eq!((record.task(), record.attempt()), (7, 2));
}
#[test]
fn a_cancel_names_its_attempt() {
    let translate::Down::Cancel(record) =
        down(jig_domain_host::LinkDown::Cancel { run: Token::new(7), attempt: Token::new(2) })
    else {
        panic!("cancel");
    };
    assert_eq!((record.task(), record.attempt()), (7, 2));
}
#[test]
fn each_committed_delivery_outcome_keeps_its_evidence() {
    use jig_domain_host as host;
    use jig_protocol_worker_wire as wire;
    for (input, expected) in [
        (host::DeliveryOutcome::Delivered, wire::DeliveryOutcome::Delivered),
        (host::DeliveryOutcome::Nothing, wire::DeliveryOutcome::Nothing),
        (host::DeliveryOutcome::Stale, wire::DeliveryOutcome::Stale),
        (host::DeliveryOutcome::Refused, wire::DeliveryOutcome::Refused),
        (host::DeliveryOutcome::Failed, wire::DeliveryOutcome::Failed),
    ] {
        let mut assignment = fixtures::assignment();
        assignment.answered[0].answer =
            host::SettledAnswer::Delivery { outcome: input, evidence: Box::from([255, 0, 127]) };
        let record = assignment_record(down(host::LinkDown::Assign { assignment }));
        let call = answered_call(record.answered());
        let wire::SettledAnswer::Delivery(answer) = call.answer() else {
            panic!("delivery");
        };
        assert_eq!(answer.outcome(), &expected);
        assert_eq!(answer.evidence(), &[255, 0, 127]);
    }
}

fn assignment_grant(record: &jig_protocol_worker_wire::Assignment) -> &jig_protocol_worker_wire::Grant {
    record.grants().get(0).expect("grant")
}
#[test]
fn a_turn_busy_record_names_the_turn() {
    let translate::Down::TurnBusy(record) = translate::turn_busy(Token::new(7), Token::new(2), 3, &codec()) else {
        panic!("busy");
    };
    assert_eq!((record.task(), record.attempt(), record.number()), (7, 2, 3));
}

fn answered_call(
    calls: &skein_lib::List<jig_protocol_worker_wire::AnsweredCall>,
) -> &jig_protocol_worker_wire::AnsweredCall {
    calls.get(0).expect("call")
}

#[test]
fn an_invalid_start_answer_keeps_its_words_in_the_hub() {
    use jig_protocol_worker_wire as wire;
    let limits = codec();
    let start = wire::InvalidStart::new(
        &limits,
        wire::InvalidStartParts { cause: wire::StartCause::Grants, why: b"budget too small".as_slice().into() },
    )
    .expect("bounded reason");
    let invalid = wire::InvalidValue::new(&limits, wire::InvalidValueParts { value: wire::Invalid::Start(start) })
        .expect("invalid start");
    let refusal = wire::RefusalValue::new(&limits, wire::RefusalValueParts { value: wire::Refusal::Invalid(invalid) })
        .expect("refusal");
    let mut fields = fixtures::answer_record().into_parts();
    fields.ending = wire::Ending::Refused(refusal);
    let record = wire::Answer::new(&limits, fields).expect("answer");
    let (answer, _) = answer(record);
    assert_eq!(
        answer.ending,
        jig_domain_host::Ending::Refused(jig_domain_host::Refusal::Invalid(jig_domain_host::Invalid::Start {
            cause: jig_domain_host::StartCause::Grants,
            why: b"budget too small".as_slice().into()
        },))
    );
}

fn program_name() -> jig_domain_host::SourceName {
    jig_domain_host::SourceName::new(b"claude").expect("source name")
}
#[test]
fn a_program_grant_crosses_as_its_name() {
    let name = program_name();
    let grant = granted_value(granted(jig_domain_host::Form::Program(name), crate::GrantValue::Program { name }));
    let jig_protocol_worker_wire::GrantValue::Program(value) = grant.value() else { panic!("program value") };
    assert_eq!(value.name(), b"claude");
}
#[test]
fn another_value_under_a_program_form_is_not_sent() {
    for value in [
        fixtures::credential_value(crate::Scheme::Bearer),
        fixtures::credential_value(crate::Scheme::Key),
        fixtures::values()[1].take().expect("application"),
    ] {
        assert!(granted(jig_domain_host::Form::Program(program_name()), value).is_none());
    }
}
#[test]
fn a_program_name_unlike_its_form_s_is_not_sent() {
    let name = jig_domain_host::SourceName::new(b"other").expect("source name");
    assert!(granted(jig_domain_host::Form::Program(program_name()), crate::GrantValue::Program { name }).is_none());
}
#[test]
fn a_program_value_under_another_form_is_not_sent() {
    use jig_domain_host::{Form, Scheme};
    for form in [Form::Credential(Scheme::Bearer), Form::Credential(Scheme::Key), Form::Application] {
        assert!(granted(form, crate::GrantValue::Program { name: program_name() }).is_none());
    }
}

#[test]
fn a_duplicate_program_endpoint_hello_is_refused_before_delivery() {
    let limits = jig_protocol_worker_wire::sizes::limits(&fixtures::SIZES).unwrap();
    let mut hello = fixtures::hello_record().into_parts();
    let mut bounds = hello.bounds.into_parts();
    let mut endpoints = skein_lib::List::with_capacity(2);
    endpoints.push(7).unwrap();
    endpoints.push(7).unwrap();
    bounds.program_endpoints = endpoints;
    hello.bounds = jig_protocol_worker_wire::Bounds::new(&limits, bounds).unwrap();
    let hello = jig_protocol_worker_wire::Hello::new(&limits, hello).unwrap();
    assert!(translate::hello(hello).is_none());
}

#[test]
fn an_assignment_encodes_each_live_ask_without_changing_it() {
    use jig_domain_host as host;
    for ask in [
        host::LiveAsk::NONE,
        host::LiveAsk { text: true, reasoning: false, input: false },
        host::LiveAsk { text: false, reasoning: true, input: false },
        host::LiveAsk { text: false, reasoning: false, input: true },
        host::LiveAsk { text: true, reasoning: true, input: true },
    ] {
        let mut assignment = fixtures::assignment();
        assignment.assignment.live = ask;
        let record = assignment_record(down(host::LinkDown::Assign { assignment }));
        let mut bytes = skein_lib::Writer::new(usize::try_from(record.measure()).unwrap());
        record.encode(&mut bytes).unwrap();
        let bytes = bytes.finish();
        let record =
            jig_protocol_worker_wire::Assignment::decode(&codec(), &mut skein_lib::Reader::new(&bytes)).unwrap();
        assert_eq!(
            (record.live().text(), record.live().reasoning(), record.live().input()),
            (ask.text, ask.reasoning, ask.input)
        );
    }
}
#[test]
fn every_piece_kind_decodes_whole_including_offsets_past_u32() {
    use jig_domain_host as host;
    use jig_protocol_worker_wire as wire;
    let limits = codec();
    let call = wire::InputCall::new(
        &limits,
        wire::InputCallParts { id: Box::from(&b"call-id"[..]), tool: Box::from(&b"tool"[..]) },
    )
    .unwrap();
    for (kind, expected) in [
        (wire::LiveKind::Text, host::LiveKind::Text),
        (wire::LiveKind::Reasoning, host::LiveKind::Reasoning),
        (
            wire::LiveKind::Input(call),
            host::LiveKind::Input { call: Box::from(&b"call-id"[..]), tool: Box::from(&b"tool"[..]) },
        ),
    ] {
        let piece = wire::Piece::new(
            &limits,
            wire::PieceParts {
                turn: 17,
                retry: 23,
                kind,
                offset: u64::from(u32::MAX) + 19,
                bytes: Box::from([0xff, 0, 0x80]),
            },
        )
        .unwrap();
        let fact =
            wire::Fact::new(&limits, wire::FactParts { task: 83, attempt: 97, value: wire::FactValue::Piece(piece) })
                .unwrap();
        let mut bytes = skein_lib::Writer::new(usize::try_from(fact.measure()).unwrap());
        fact.encode(&mut bytes).unwrap();
        let decoded = translate::decode(wire::Kind::Fact.number(), &bytes.finish(), &limits).unwrap();
        assert_eq!(
            up(decoded),
            host::LinkUp::Piece {
                run: Token::new(83),
                attempt: Token::new(97),
                piece: host::Piece {
                    turn: 17,
                    retry: 23,
                    kind: expected,
                    offset: u64::from(u32::MAX) + 19,
                    bytes: Box::from([0xff, 0, 0x80])
                }
            }
        );
    }
}
