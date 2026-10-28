use super::fixtures;
use crate::{End, Stop};
use jig_domain_core as core;
use jig_protocol_accounts::table::{Scheme, Value};
use jig_protocol_engine_workers::{self as engine, GrantValue};
use jig_protocol_store::Event as StoreEvent;
use skein_lib::Secret;

#[test]
fn the_restart_asks_one_load_at_a_time() {
    let mut service = fixtures::started(&fixtures::engine_document());
    fixtures::turn(&mut service);
    let engine = fixtures::engine(&mut service);
    assert_eq!(engine.store.above.len(), 1);
    let Some(StoreEvent::Restored { owner, rows, next }) = engine.store.above.pop() else {
        panic!("restart load completed by the real store");
    };
    assert_eq!(rows.len(), 0);
    assert_eq!(next, None);
    engine.store.above.push(StoreEvent::Restored { owner, rows, next });
    fixtures::turn(&mut service);
    let engine = fixtures::engine(&mut service);
    let Some(StoreEvent::Committed { .. }) = engine.store.above.pop() else {
        panic!("restart writes commit first");
    };
    assert_eq!(engine.store.above.len(), 0);
}

fn period(driven: &mut fixtures::Driven, number: u64) {
    driven.party(core::Event::Period { project: 1, period: number, budget: 100, next: None });
}

// Offer real work with commit answers withheld until admission returns the
// probe. One writing period contributes at least one backlog entry.
fn fill_to_watermark(driven: &mut fixtures::Driven) -> u64 {
    let watermark = fixtures::engine(&mut driven.service).env.limits.watermark;
    let first_period = 2; // Driven::started has already committed period 1.
    let last_probe = u64::from(watermark).checked_add(first_period).expect("fixture periods fit");
    for number in first_period..=last_probe {
        period(driven, number);
        if let Some(event) = driven.busy.pop() {
            let jig_test_domain::Event::Core(core::Event::Period { period, .. }) = *event else {
                panic!("the period probe is returned at the watermark");
            };
            return period;
        }
    }
    panic!("admission returns a probe at the watermark");
}

#[test]
fn a_store_answer_is_taken_at_the_watermark() {
    // Small profile: 3 commits + 256 door outputs; answering one and
    // admitting its retry leaves the same 259 commits pending.
    const PENDING_COMMITS: u32 = 259;
    let mut driven = fixtures::Driven::started(&fixtures::engine_document());
    let next_period = fill_to_watermark(&mut driven);
    let first = driven.commit_asked();
    driven.commit(first);
    assert_eq!(fixtures::engine(&mut driven.service).store.above.len(), 0);
    period(&mut driven, next_period);
    assert_eq!(driven.commits.len(), PENDING_COMMITS);
}

#[test]
fn a_party_request_is_returned_busy_at_the_watermark() {
    let mut driven = fixtures::Driven::started(&fixtures::engine_document());
    let _next_period = fill_to_watermark(&mut driven);
    driven.chat(101, b"retained");
    let event = driven.busy.pop().expect("unadmitted party input");
    let jig_test_domain::Event::Core(core::Event::People(jig_domain_people::Event::Ask { ask, .. })) = *event else {
        panic!("busy returns the same party request");
    };
    let jig_domain_people::Ask::Create { batch, .. } = ask else {
        panic!("same chat request");
    };
    assert_eq!(batch[0].words.as_ref(), b"retained");
}

#[test]
fn a_commit_leaves_in_the_iteration_that_made_it() {
    let mut driven = fixtures::Driven::started(&fixtures::engine_document());
    period(&mut driven, 2);
    assert_eq!(driven.commits.len(), 1);
}

#[test]
fn a_failed_commit_releases_nothing_after_it() {
    let mut driven = fixtures::Driven::started(&fixtures::engine_document());
    driven.chat(101, b"one");
    let first = driven.commit_asked();
    driven.chat(102, b"two");
    let second = driven.commit_asked();
    driven.fail(first);
    driven.commit(second);
    assert_eq!(driven.released_to_parties(), 0);
    assert_eq!(fixtures::engine(&mut driven.service).store.above.len(), 0);
    assert_eq!(driven.end(), Some(&End::Stopped(Stop::CommitFailed { number: first })));
}

#[test]
fn a_frame_refusal_keeps_the_reference_uncommitted_stop_barrier() {
    let mut driven = fixtures::Driven::started(&fixtures::engine_document());
    driven.chat(101, b"one");
    let first = driven.commit_asked();
    driven.chat(102, b"two");
    let second = driven.commit_asked();
    fixtures::engine(&mut driven.service).store.above.push(jig_protocol_store::Event::FrameTooLarge {
        number: first,
        task: 11,
        bytes: 79,
        limit: 78,
    });
    driven.commit(second);
    assert_eq!(driven.released_to_parties(), 0);
    assert_eq!(fixtures::engine(&mut driven.service).store.above.len(), 0);
    assert_eq!(driven.end(), Some(&End::Stopped(Stop::CommitFailed { number: first })));
}

#[test]
fn the_service_waits_while_its_load_is_outstanding() {
    let mut service = fixtures::started(&fixtures::engine_document());
    fixtures::turn(&mut service);
    let _held = fixtures::engine(&mut service).store.above.pop().expect("hold the actual load terminal");
    assert!(!service.work_pending(skein_lib::Time::from_nanos(1)), "waiting for the store is not work");
}

#[test]
fn a_store_answer_is_work_when_party_outputs_are_full() {
    let mut driven = fixtures::Driven::started(&fixtures::engine_document());
    period(&mut driven, 2);
    let number = driven.commit_asked();
    let terminal = driven.terminal(number);
    let engine = fixtures::engine(&mut driven.service);
    for _ in 0..engine.parties_out().room() {
        engine.parties_out().push(crate::PartyOutput::Busy(Box::new(jig_test_domain::Event::Core(
            core::Event::Period { project: 1, period: 3, budget: 100, next: None },
        ))));
    }
    engine.store.above.push(terminal);
    assert!(driven.service.work_pending(skein_lib::Time::from_nanos(1)));
    fixtures::turn(&mut driven.service);
    assert_eq!(fixtures::engine(&mut driven.service).store.above.len(), 0);
}

fn path() -> jig_test_connector::Path {
    let mut segments = skein_lib::List::with_capacity(1);
    segments.push(b"one".as_slice().into()).expect("one segment");
    jig_test_connector::Path::new(segments, 8).expect("fixture path")
}
fn effect(target: u64) -> jig_test_connector::Effect {
    jig_test_connector::Effect {
        kind: 1,
        resources: Box::new([path()]),
        purpose: 1,
        condition: None,
        target,
        state: target,
    }
}
fn key() -> jig_test_connector::Key {
    jig_test_connector::Key { deployment: [1; 16], task: 1, purpose: 1, attempt: 1, completion: 1, position: 1 }
}
#[test]
fn the_system_applies_an_effect_and_reads_it_back() {
    use jig_test_connector::{ApplyResult, Fact, Form, Origin, Recovery, SystemEvent, SystemRequest};
    let mut system = crate::system::System::new(2, 2);
    assert_eq!(
        system.answer(SystemRequest::Apply {
            entry: 1,
            attempt: 1,
            key: key(),
            effect: effect(7),
            form: Form::Set,
            recovery: Recovery::Keyed
        }),
        SystemEvent::Applied { entry: 1, attempt: 1, result: ApplyResult::Made { state: 7 } }
    );
    assert_eq!(
        system.answer(SystemRequest::ReadFact { resource: path(), observed: skein_lib::Wall::EPOCH }),
        SystemEvent::Fact {
            resource: path(),
            fact: Fact { state: Some(7), observed: skein_lib::Wall::EPOCH, pending: false },
            origin: Origin::Own
        }
    );
}

#[test]
fn a_keyed_retry_keeps_the_first_result() {
    use jig_test_connector::{ApplyResult, Form, Recovery, SystemEvent, SystemRequest};
    let mut system = crate::system::System::new(2, 2);
    let _first = system.answer(SystemRequest::Apply {
        entry: 1,
        attempt: 1,
        key: key(),
        effect: effect(7),
        form: Form::Set,
        recovery: Recovery::Keyed,
    });
    assert_eq!(
        system.answer(SystemRequest::Apply {
            entry: 1,
            attempt: 2,
            key: key(),
            effect: effect(9),
            form: Form::Set,
            recovery: Recovery::Keyed
        }),
        SystemEvent::Applied { entry: 1, attempt: 2, result: ApplyResult::Made { state: 7 } }
    );
}

#[test]
fn a_call_is_decoded_and_one_that_does_not_is_answered_an_error() {
    let config = fixtures::config();
    let mut tools = tools(&config);
    let rows: [(&[u8], Option<&[u8]>); 3] = [
        (br#"{"target":3,"reason":"no longer needed"}"#, None),
        (br#"{"target":3}"#, Some(b"cancel: the field reason is missing")),
        (b"broken", Some(b"cancel: the input is malformed")),
    ];
    for (input, error) in rows {
        let event = decode_call(&mut tools, input);
        match error {
            None => {
                let jig_test_domain::Event::Core(core::Event::NamedAction { key, action, .. }) = event else {
                    panic!("named action");
                };
                assert_eq!(key, core::CallKey { task: 2, attempt: 3, completion: 7, position: 2 });
                let core::NamedAction::Control { target, control: jig_domain_tasks::Control::Cancel { .. } } = action
                else {
                    panic!("cancel action");
                };
                assert_eq!(target, 3);
            }
            Some(expected) => {
                let jig_test_domain::Event::CallAnswered { call, .. } = event else {
                    panic!("call error");
                };
                assert_eq!(call.answer, core::SettledAnswer::Host { error: true, body: expected.into() });
            }
        }
    }
}

fn encoded_assignment(
    attempt: u64,
    answered: Box<[core::Settled]>,
) -> (jig_domain_host::Assignment, Box<[Option<GrantValue>]>) {
    let config = fixtures::config();
    let workers = fixtures::workers(&config);
    let values = fixtures::values();
    let mut assignment = fixtures::assignment(&config);
    assignment.attempt = attempt;
    assignment.answered = answered;
    let event = workers
        .encoded_assignment(skein_lib::Time::ZERO, skein_lib::Token::new(17), assignment, &values)
        .expect("assignment encoded");
    let jig_protocol_engine_workers::Event::Down {
        channel,
        down: jig_domain_host::LinkDown::Assign { assignment },
        values,
        ..
    } = event
    else {
        panic!("worker assignment");
    };
    assert_eq!(channel, skein_lib::Token::new(17));
    (assignment, values)
}
#[test]
fn an_assignment_leaves_with_its_charter_encoded_and_grants_filled() {
    let (assignment, grants) = encoded_assignment(3, Box::default());
    let decoded = smith_protocol_channel::decode_charter(
        &assignment.assignment.charter,
        &jig_protocol_charter::limits(),
        &fixtures::endpoints(),
    )
    .expect("charter decodes");
    assert_eq!(decoded.instructions.as_ref(), b"fixture contents");
    assert_eq!(decoded.brief.sections.len(), 2);
    assert_eq!(decoded.brief.sections[0].title.as_ref(), b"Task");
    assert_eq!(decoded.brief.sections[0].text.as_ref(), b"Make a report");
    assert_eq!(decoded.brief.sections[1].title.as_ref(), b"Results");
    assert_eq!(decoded.brief.sections[1].text.as_ref(), b"Prior result");
    assert_eq!(grants.len(), 2);
    let Some(GrantValue::Credential { scheme: engine::Scheme::Bearer, token }) = &grants[0] else {
        panic!("a bearer account grant");
    };
    assert_eq!(token.expose(), fixtures::TOKEN);
    let Some(GrantValue::Application { secret }) = &grants[1] else { panic!("an application grant") };
    assert_eq!(secret.expose(), b"application-token");
}
#[test]
fn each_scheme_reaches_the_engine_s_grant_value() {
    use jig_domain_host::{Form, Grant};
    use skein_lib::{Duration, Time};
    for (scheme, expected) in [(Scheme::Bearer, ("bearer", b"".as_slice())), (Scheme::Key, ("key", b"".as_slice()))] {
        let mut values = fixtures::values();
        let value = Value { token: Secret::new(fixtures::TOKEN.into()), scheme };
        values.accounts.granted(1, 2, value, Time::from_nanos(1)).expect("room");
        let grant = Grant {
            form: Form::Credential(jig_domain_host::Scheme::Bearer),
            account: 1,
            generation: 2,
            valid: Duration::from_secs(1),
        };
        let grants = values.grants(&[grant], Time::ZERO);
        let found = match &grants[0] {
            Some(GrantValue::Credential { scheme: engine::Scheme::Bearer, token }) => ("bearer", token, b"".as_slice()),
            Some(GrantValue::Credential { scheme: engine::Scheme::Key, token }) => ("key", token, b"".as_slice()),
            Some(GrantValue::Application { .. } | GrantValue::Program { .. }) | None => panic!("an account grant"),
        };
        assert_eq!((found.0, found.1.expose(), found.2), (expected.0, fixtures::TOKEN, expected.1));
    }
}
#[test]
fn a_grant_whose_generation_the_table_lacks_is_left_out() {
    use jig_domain_host::{Form, Grant};
    use skein_lib::{Duration, Time};
    let values = fixtures::values();
    for (form, generation, time) in [
        (Form::Credential(jig_domain_host::Scheme::Bearer), 2, Time::ZERO),
        (Form::Credential(jig_domain_host::Scheme::Bearer), 1, Time::from_nanos(120_000_000_000)),
        (Form::Application, 2, Time::ZERO),
    ] {
        let grants = values.grants(&[Grant { form, account: 1, generation, valid: Duration::from_secs(1) }], time);
        assert_eq!(grants.len(), 1);
        assert!(grants[0].is_none());
    }
}
#[test]
fn application_grants_keep_the_two_newest_generations_per_number() {
    use jig_domain_host::{Form, Grant};
    use skein_lib::{Duration, Secret, Time};
    let mut values = fixtures::values();
    for (number, generation, value) in [
        (2, 1, b"other".as_slice()),
        (1, 3, b"third".as_slice()),
        (1, 2, b"second".as_slice()),
        (1, 1, b"stale".as_slice()),
        (1, 3, b"replacement".as_slice()),
    ] {
        values.put_application(number, generation, Secret::new(value.into()));
    }
    for (number, generation, expected) in [
        (1, 1, None),
        (1, 2, Some(b"second".as_slice())),
        (1, 3, Some(b"replacement".as_slice())),
        (1, 4, None),
        (2, 1, Some(b"other".as_slice())),
    ] {
        let grants = values.grants(
            &[Grant { form: Form::Application, account: number, generation, valid: Duration::from_secs(1) }],
            Time::ZERO,
        );
        match &grants[0] {
            Some(GrantValue::Application { secret }) => assert_eq!(Some(secret.expose()), expected),
            Some(GrantValue::Credential { .. } | GrantValue::Program { .. }) => {
                panic!("an application grant")
            }
            None => assert_eq!(expected, None),
        }
    }
}

#[test]
fn the_run_s_identity_is_shared_by_a_conversation_s_attempts() {
    for attempt in [3, 4, 9] {
        let (assignment, _) = encoded_assignment(attempt, Box::default());
        assert_eq!(assignment.assignment.attempt, skein_lib::Token::new(attempt));
        assert_eq!(assignment.assignment.identity, IDENTITY);
    }
}
#[test]
fn the_workers_listener_opens_before_the_restart() {
    let mut service = fixtures::started(&fixtures::workers_document());
    fixtures::turn(&mut service);
    assert!(fixtures::engine(&mut service).store.above.is_empty());
    assert!(!fixtures::engine(&mut service).workers.as_ref().expect("workers").opened());
    fixtures::turn(&mut service);
    assert!(fixtures::engine(&mut service).workers.as_ref().expect("workers").opened());
    // The handed-in record's `Kept` may come in the same pass.
    let mut restored = false;
    for event in &fixtures::engine(&mut service).store.above {
        if let StoreEvent::Restored { .. } = event {
            restored = true;
        }
    }
    assert!(restored, "restart follows listener");
}
#[test]
fn a_result_that_does_not_decode_fails_its_attempt_as_invalid() {
    for bytes in [b"".as_slice(), b"broken", &[0, 2]] {
        let event = crate::tools::result(skein_lib::Token::new(7), bytes);
        let jig_test_domain::Event::ResultRead { payload, end } = event else {
            panic!("result read");
        };
        assert_eq!(payload, skein_lib::Token::new(7));
        assert_eq!(end, jig_domain_tasks::End::Failed(jig_domain_tasks::Class::Invalid));
    }
}
#[test]
fn a_provider_registration_without_runtime_is_refused() {
    let configured = fixtures::with(
        &fixtures::accounts_document(),
        br#""name":"fake-provider""#,
        br#""name":"fake-provider","signed_in_plan":false"#,
    );
    let document = fixtures::with(&configured, br#""keys":true"#, br#""keys":true,"sign_in":{"issuer":"https://issuer.example","client":"client","scopes":["openid"],"authorization":"https://issuer.example/authorize","token":"https://issuer.example/token","keys":"https://issuer.example/keys","transport":"tls","server_name":"issuer.example","must_grant":["openid"],"redirect_path":"callback"}"#);
    let found = crate::startup(&document, &fixtures::files(&document), None, &fixtures::seeds(), None, None)
        .err()
        .expect("runtime absent");
    assert_eq!(
        found.problems.get(0).unwrap().path,
        jig_protocol_config::Path::root().member(b"engine").member(b"llm").member(b"providers")
    );
    assert_eq!(found.problems.get(0).unwrap().what, jig_protocol_config::What::Refers);
}

const IDENTITY: [u8; 16] = [227, 220, 92, 103, 122, 9, 131, 161, 162, 47, 205, 195, 6, 132, 110, 28];

#[test]
fn kernel_operations_are_routed_back_to_their_owner() {
    use skein_io::kernel::{Complete, Done, Error, Fd, Op, Submit};
    use skein_lib::{Queue, Token};
    let mut kernel = crate::accounts::Kernel::new(4);
    let mut sockets = Queue::with_capacity(2);
    sockets.push(Submit { op: Token::new(1), kind: Op::Close { fd: Fd::new(6) } });
    let mut output = Queue::with_capacity(4);
    kernel.publish(&mut sockets, &mut output);
    let request = output.pop().expect("socket operation");
    sockets.push(Submit { op: Token::new(2), kind: Op::Cancel { target: Token::new(1) } });
    let complete =
        kernel.complete(Complete { op: request.op, kind: request.kind, result: Ok(Done::Nothing) }, &sockets);
    assert_eq!(complete.op, Token::new(1));
    kernel.publish(&mut sockets, &mut output);
    let cancel = output.pop().expect("cancel after target completion");
    assert_eq!(cancel.kind, Op::Cancel { target: request.op });
    let complete =
        kernel.complete(Complete { op: cancel.op, kind: cancel.kind, result: Err(Error::TooLate) }, &sockets);
    assert_eq!(complete.op, Token::new(2));
    assert_eq!(complete.kind, Op::Cancel { target: Token::new(1) });
}

fn tools(config: &jig_protocol_config::Engine) -> crate::tools::Tools {
    crate::tools::Tools::new(&config.limits.core)
}
fn decode_call(tools: &mut crate::tools::Tools, input: &[u8]) -> jig_test_domain::Event {
    tools.decode(
        skein_lib::ReplyTo::new(skein_lib::Token::new(11)),
        skein_lib::Token::new(2),
        skein_lib::Token::new(3),
        &fixtures::call(input),
    )
}
#[test]
fn a_calls_terminal_keeps_its_live_address_only_for_settlement() {
    let config = fixtures::config();
    let mut tools = tools(&config);
    let key = core::CallKey { task: 2, attempt: 3, completion: 7, position: 2 };
    for durable in [false, true] {
        let _decoded = decode_call(&mut tools, br#"{"target":3,"reason":"no longer needed"}"#);
        let event = tools
            .render(skein_lib::ReplyTo::new(skein_lib::Token::new(11)), key, &core::CallPart::Controlled, durable)
            .expect("rendered terminal");
        let (call, seen) = if let jig_test_domain::Event::CallAnswered { to, call } = event {
            assert_eq!(to.into_token(), skein_lib::Token::new(11));
            (call, false)
        } else {
            let jig_test_domain::Event::Core(core::Event::SettledCall { to, key: named, name, call }) = event else {
                panic!("rendered call route");
            };
            assert_eq!(to.into_token(), skein_lib::Token::new(11));
            assert_eq!(named, key);
            assert_eq!(name, fixtures::call(b"").name);
            (call, true)
        };
        assert_eq!(seen, durable);
        assert_eq!(call.tool.as_ref(), b"cancel");
        assert_eq!(call.answer, core::SettledAnswer::Host { error: false, body: b"Done".as_slice().into() });
    }
}
#[test]
fn a_store_refusal_at_open_ends_the_start() {
    let mut service = fixtures::started(&fixtures::engine_document());
    let engine = fixtures::engine(&mut service);
    let _opened = engine.store.above.pop();
    engine
        .store
        .above
        .push(StoreEvent::OpenRefused { owner: fixtures::OWNER, why: jig_protocol_store::OpenRefusal::Failed });
    fixtures::turn(&mut service);
    let Some(End::Stopped(Stop::Start(problems))) = service.end() else {
        panic!("store refused startup");
    };
    assert_eq!(problems.problems.len(), 1);
    assert_eq!(
        problems.problems.get(0).expect("open refusal").path,
        jig_protocol_config::Path::root().member(b"deployment")
    );
}
#[test]
fn a_commit_reaches_the_store_and_its_answer_the_journal() {
    let mut driven = fixtures::Driven::started(&fixtures::engine_document());
    driven.chat(101, b"one");
    let number = driven.commit_asked();
    assert_eq!(driven.released_to_parties(), 0);
    driven.commit(number);
    assert_eq!(
        driven.reply,
        Some(jig_domain_people::Reply::Outcome(jig_domain_people::Outcome::Created {
            first: 1,
            last: 1,
            given: Some(jig_domain_people::Given { task: 1, amount: 100 })
        }))
    );
}

#[test]
fn the_store_waits_for_other_drivers_before_closing_its_root() {
    let mut service = fixtures::started(&fixtures::engine_document());
    let engine = fixtures::engine(&mut service);
    engine.store.adopt(skein_io::kernel::Fd::new(9), 0).expect("fixture deployment root");
    engine.store.iterate();
    assert_eq!(engine.store.submitted.len(), 2, "other drivers are opening their directory handles");
    while engine.store.submitted.pop().is_some() {}
    engine.store.close();
    assert!(!engine.store.work_pending(skein_lib::Time::ZERO), "root close waits for outstanding drivers");
    assert!(!engine.store.closed(), "the owned root is still held");
}

#[test]
fn a_program_grant_needs_no_table_entry() {
    use jig_domain_host::{Form, Grant, SourceName};
    let values = fixtures::values();
    let name = SourceName::new(b"claude").expect("source name");
    let grants = values.grants(
        &[Grant { form: Form::Program(name), account: 99, generation: 7, valid: skein_lib::Duration::from_secs(1) }],
        skein_lib::Time::ZERO,
    );
    let Some(GrantValue::Program { name }) = &grants[0] else { panic!("program name") };
    assert_eq!(name.as_bytes(), b"claude");
}

#[test]
fn program_names_share_the_wire_and_domain_bounds() {
    use jig_protocol_worker_wire::v9;
    // Schema source names and grant names both admit exactly 32 bytes.
    let bound = usize::try_from(v9::CEILINGS.program_value_name).expect("name bound fits usize");
    assert_eq!(v9::CEILINGS.program_value_name, v9::CEILINGS.program_source_name);
    let past = bound.checked_add(1).expect("small name bound");
    let name = [b'a'; 33]; // The schema's 32-byte name plus one byte.
    for (length, admitted) in [(bound, true), (past, false)] {
        let value = name.get(..length).expect("fixture spans both sides of the bound");
        assert_eq!(jig_domain_host::SourceName::new(value).is_some(), admitted);
        assert_eq!(jig_domain_accounts::SourceName::new(value).is_some(), admitted);
    }
}

#[test]
fn a_replayed_call_uses_its_original_key_for_the_name() {
    let key = core::CallKey { task: 2, attempt: 1, completion: 7, position: 2 };
    let answered = Box::new([core::Settled {
        key,
        call: core::SettledCall {
            serial: 1,
            tool: b"cancel".as_slice().into(),
            answer: core::SettledAnswer::Host { error: false, body: b"Done".as_slice().into() },
        },
    }]);
    let (assignment, _) = encoded_assignment(3, answered);
    assert_eq!(
        assignment.answered.as_ref(),
        [jig_domain_host::AnsweredCall {
            name: jig_protocol_charter::call_name(&jig_protocol_charter::CallKey {
                activation: key.attempt,
                completion: key.completion,
                position: key.position,
            }),
            tool: b"cancel".as_slice().into(),
            answer: jig_domain_host::SettledAnswer::Host { error: false, body: b"Done".as_slice().into() },
        }]
    );
}
