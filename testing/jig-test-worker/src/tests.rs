mod fixtures;
use crate::{Encodable, Encoded, Event, Invalid, Request};
use alloc::boxed::Box;
use fixtures::Worker;
use jig_domain_host as host;
use smith_host_domain as smith;

#[test]
fn a_grants_form_maps_one_to_one_into_smiths() {
    for (form, expected) in [
        (host::Form::Credential(host::Scheme::Bearer), Some(smith::Form::Credential(smith::Scheme::Bearer))),
        (host::Form::Credential(host::Scheme::Key), Some(smith::Form::Credential(smith::Scheme::Key))),
        (host::Form::Application, None),
        (
            host::Form::Program(host::SourceName::new(b"claude").expect("source name")),
            Some(smith::Form::Program(smith::SourceName::new(b"claude").expect("smith source name"))),
        ),
    ] {
        let grant = host::Grant { form, account: 7, generation: 4, valid: skein_lib::Duration::from_secs(30) };
        let actual = crate::translate::grant(grant);
        match expected {
            Some(form) => {
                assert_eq!(actual, Some(smith::Grant { account: 7, generation: 4, valid: grant.valid, form }));
            }
            None => assert_eq!(actual, None),
        }
    }
}

#[test]
fn a_start_waits_for_its_charter_to_come_back_decoded() {
    let mut worker = Worker::new(fixtures::limits());
    worker.assign();
    let (owner, source) = worker.take_decode();
    assert_eq!(owner, fixtures::AGENT);
    assert_eq!(source.charter, fixtures::charter_bytes());
    assert_eq!(worker.spawns(), 0);
    worker.step(Event::Decoded { agent: owner, start: Ok(Box::new(fixtures::start(source))) });
    assert_eq!(worker.spawns(), 1);
}

#[test]
fn an_undecodable_charter_fails_the_start_as_invalid() {
    let mut worker = Worker::new(fixtures::limits());
    worker.assign();
    let (owner, _) = worker.take_decode();
    worker.step(Event::Decoded { agent: owner, start: Err(Invalid::Charter) });
    assert_eq!(worker.spawns(), 0);
    assert_eq!(
        worker.take_answer().ending,
        host::Ending::Refused(host::Refusal::Invalid(host::Invalid::Start {
            cause: jig_domain_host::StartCause::Other,
            why: Box::from(&b"charter"[..])
        }))
    );
}

#[test]
fn a_turn_reaches_the_hub_only_after_it_is_encoded() {
    let mut worker = Worker::started();
    worker.receive(smith::Up::Turn { turn: fixtures::turn() });
    let (owner, value) = worker.take_encode();
    assert_eq!(value, Encodable::Turn(fixtures::turn()));
    let turn = fixtures::encoded_turn();
    worker.step(Event::Encoded { owner, encoded: Encoded::Turn(fixtures::encoded_turn()) });
    let mut actual = None;
    for _ in 0..worker.out.len() {
        if let Some(Request::Link(host::LinkUp::Turn { turn, .. })) = worker.out.pop() {
            actual = Some(turn);
        }
    }
    assert_eq!(actual, Some(turn));
}

#[test]
fn a_result_is_handed_out_once_and_reaches_the_hub_as_bytes() {
    let mut worker = Worker::started();
    worker.receive(smith::Up::Answer {
        answer: smith::Answer { read: None, turns: 0, spent: 0, result: fixtures::result() },
    });
    let (owner, value) = worker.take_encode();
    assert_eq!(value, Encodable::Result(fixtures::result()));
    assert_eq!(worker.hand_outs(), 0, "one hand-out");
    worker.step(Event::Encoded { owner, encoded: Encoded::Result(Box::from(&b"encoded result"[..])) });
    worker.gone();
    assert_eq!(
        worker.take_answer().ending,
        host::Ending::Ended {
            outcome: Box::from(&b"encoded result"[..]),
            work: host::Work { left: None, saved: None }
        }
    );
}

#[test]
fn a_channel_fact_reaches_the_hub_as_activity() {
    let mut worker = Worker::started();
    worker.receive(smith::Up::Fact { body: Box::from(&b"a fact"[..]) });
    assert_eq!(worker.out.pop(), Some(Request::Fact { owner: fixtures::AGENT, body: Box::from(&b"a fact"[..]) }));
    worker.clear();
    let activity =
        host::Activity::Count { kind: host::CountKind::TextArrived, elapsed: skein_lib::Duration::ZERO, count: 1 };
    worker.step(Event::Activity { owner: fixtures::AGENT, activity: Some(activity) });
    assert_eq!(
        worker.out.pop(),
        Some(Request::Link(host::LinkUp::Activity { run: fixtures::RUN, attempt: fixtures::ATTEMPT, activity }))
    );
}

fn live_piece() -> host::Piece {
    host::Piece { turn: 1, retry: 2, kind: host::LiveKind::Text, offset: 7, bytes: Box::from(&b"reply"[..]) }
}

#[test]
fn a_live_piece_reaches_the_hub_and_refreshes_progress() {
    let mut worker = Worker::asking(Box::new([]), host::LiveAsk { text: true, reasoning: false, input: false });
    let before = worker.domain.next_deadline().expect("watchdog armed");
    worker.env.now = skein_lib::Time::ZERO.saturating_add(skein_lib::Duration::from_secs(1));
    worker.step(Event::Live { agent: worker.process, piece: live_piece() });
    assert_eq!(
        link_notice(&mut worker),
        Some(host::LinkUp::Piece { run: fixtures::RUN, attempt: fixtures::ATTEMPT, piece: live_piece() })
    );
    assert_eq!(worker.domain.next_deadline(), Some(before.saturating_add(skein_lib::Duration::from_secs(1))));
}

#[test]
fn a_live_piece_for_an_unknown_agent_goes_nowhere() {
    let mut worker = Worker::started();
    let before = worker.domain.next_deadline();
    worker.env.now = skein_lib::Time::ZERO.saturating_add(skein_lib::Duration::from_secs(1));
    worker.step(Event::Live { agent: skein_lib::Token::new(u64::MAX), piece: live_piece() });
    assert!(worker.out.is_empty());
    assert_eq!(worker.domain.next_deadline(), before);
}

#[test]
fn an_unasked_run_gives_no_live_piece() {
    let mut worker = Worker::started();
    worker.step(Event::Live { agent: worker.process, piece: live_piece() });
    assert_eq!(link_notice(&mut worker), None);
}

#[test]
fn a_deliver_is_answered_with_its_title_and_body() {
    let mut worker = Worker::started();
    worker.receive(smith::Up::Call {
        call: fixtures::CALL,
        name: smith::CallName { activation: 7, completion: 1, position: 0 },
        deadline: skein_lib::Time::ZERO.saturating_add(skein_lib::Duration::from_secs(30)),
        ask: fixtures::delivery(),
    });
    let (owner, value) = worker.take_encode();
    assert_eq!(value, Encodable::CallName(smith::CallName { activation: 7, completion: 1, position: 0 }));
    worker.step(Event::Encoded { owner, encoded: Encoded::CallName(Box::from(&b"delivery call"[..])) });
    let content = worker.domain.delivery(fixtures::AGENT).expect("delivered workspace");
    assert_eq!(content.title.as_ref(), b"a title");
    assert_eq!(content.body.as_ref(), b"a body");
    let mut actual = None;
    for _ in 0..worker.out.len() {
        if let Some(Request::Process(request)) = worker.out.pop()
            && let smith::process::Request::Send { message: smith::Down::Answer { call, reply }, .. } = *request
        {
            actual = Some((call, reply));
        }
    }
    let receipt = smith::Receipt::new(0, Box::new(0_u64.to_be_bytes())).expect("fixture receipt");
    assert_eq!(
        actual,
        Some((
            fixtures::CALL,
            smith::Reply::Delivery(smith::Delivery::Delivered(
                smith::Delivered::new(Box::new([receipt])).expect("fixture delivered")
            ))
        ))
    );
}

#[test]
fn a_stopping_worker_waits_on_its_agent_and_emits_stop_when_it_is_gone() {
    let mut worker = Worker::started();
    worker.step(Event::Shutdown);
    worker.step(Event::Timer);
    worker.sent();
    worker.clear();
    assert!(!worker.domain.work_pending(), "the live agent owes IO or its stop deadline");
    assert!(worker.domain.next_deadline().is_some(), "the agent's stop has a deadline");
    worker.gone();
    worker.step(Event::Link(host::LinkDown::Acknowledge { run: fixtures::RUN, attempt: fixtures::ATTEMPT }));
    worker.domain.reclaim();
    assert!(worker.domain.work_pending(), "the last agent makes the final stop ready");
    worker.step(Event::Timer);
    let mut stopped = false;
    for request in &worker.out {
        if let Request::Stop = request {
            stopped = true;
        }
    }
    assert!(stopped, "the last agent releases the final stop");
    assert!(!worker.domain.work_pending(), "the one stop was emitted");
}

/// The one link notice an agent's account notice became.
fn link_notice(worker: &mut Worker) -> Option<host::LinkUp> {
    let mut found = None;
    for _ in 0..worker.out.len() {
        if let Some(Request::Link(up)) = worker.out.pop() {
            assert!(found.is_none(), "one link notice");
            found = Some(up);
        }
    }
    found
}

#[test]
fn each_account_notice_reaches_the_link_as_its_own_notice() {
    let grant = smith::Grant {
        account: 0,
        generation: 1,
        valid: skein_lib::Duration::from_secs(60),
        form: smith::Form::Credential(smith::Scheme::Bearer),
    };
    let (run, attempt) = (fixtures::RUN, fixtures::ATTEMPT);
    let reset = Some(skein_lib::Duration::from_secs(30));
    let (code, reason) = (Box::<[u8]>::from(&b"forbidden"[..]), Box::<[u8]>::from(&b"suspended"[..]));
    let notices = [
        (
            smith::Up::Unauthorized { account: 0, generation: 1 },
            host::LinkUp::Unauthorized { run, attempt, account: 0, generation: 1 },
        ),
        (smith::Up::Limited { account: 0, reset }, host::LinkUp::Limited { run, attempt, account: 0, reset }),
        (
            smith::Up::Limited { account: 0, reset: None },
            host::LinkUp::Limited { run, attempt, account: 0, reset: None },
        ),
        (
            smith::Up::Refused { account: 0, code: code.clone(), reason: reason.clone() },
            host::LinkUp::AccountRefused { run, attempt, account: 0, code, reason },
        ),
    ];
    for (notice, expected) in notices {
        let mut worker = Worker::lending(Box::new([grant]));
        worker.receive(notice);
        assert_eq!(link_notice(&mut worker), Some(expected));
    }
}

#[test]
fn a_run_failed_on_its_limited_or_refused_account_keeps_the_account() {
    let reset = Some(skein_lib::Duration::from_secs(30));
    let model = |fault| crate::translate::failure(&smith::RunFailure::Model(fault));
    assert_eq!(
        model(smith::ModelFault::Limited { account: 0, reset }),
        host::RunFailure::Limited { account: 0, reset }
    );
    assert_eq!(
        model(smith::ModelFault::Limited { account: 0, reset: None }),
        host::RunFailure::Limited { account: 0, reset: None }
    );
    assert_eq!(
        model(smith::ModelFault::Refused { account: 0, code: Box::from(&b"forbidden"[..]) }),
        host::RunFailure::Refused { account: 0 }
    );
    assert_eq!(model(smith::ModelFault::Unauthorized { account: 0 }), host::RunFailure::Model);
}

#[test]
fn only_an_agents_grants_refusal_carries_the_grants_start_cause() {
    for (why, expected) in [
        (smith::RunInvalid::Grants, host::StartCause::Grants),
        (smith::RunInvalid::Messages, host::StartCause::Other),
        (smith::RunInvalid::Outcome, host::StartCause::Other),
        (smith::RunInvalid::Workspace, host::StartCause::Other),
    ] {
        let result = smith::RunResult::Refused { refusal: smith::Refusal::Invalid(why) };
        let crate::hand_out::Finish::Refused(cause) = crate::hand_out::Finish::of(&result) else {
            panic!("start refusal")
        };
        assert_eq!(cause, expected);
    }
}

#[test]
fn hello_endpoints_keep_uniqueness_and_count_checks() {
    let mut limits = fixtures::limits();
    assert!(crate::valid(&limits));
    let mut endpoints = skein_lib::List::with_capacity(host::PROGRAM_ENDPOINTS);
    for endpoint in 0..host::PROGRAM_ENDPOINTS {
        endpoints.push(endpoint).expect("bounded endpoint list");
    }
    limits.bounds.program_endpoints = endpoints.into_boxed();
    assert!(crate::valid(&limits));
    limits.bounds.program_endpoints = Box::new([0, 0]);
    assert!(!crate::valid(&limits));
    let past_cap = host::PROGRAM_ENDPOINTS + 1;
    let mut endpoints = skein_lib::List::with_capacity(past_cap);
    for endpoint in 0..past_cap {
        endpoints.push(endpoint).expect("one slot per endpoint");
    }
    limits.bounds.program_endpoints = endpoints.into_boxed();
    assert!(!crate::valid(&limits));
}
