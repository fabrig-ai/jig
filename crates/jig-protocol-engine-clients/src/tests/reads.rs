use super::super::*;
use super::fixtures as f;
use jig_domain_core::reads;
#[test]
fn a_read_is_answered_only_once_held_is_released() {
    let cases = f::held_reads();
    for (label, value, expected_read, answer, expected_answer) in cases {
        let mut exchange = f::read_exchange();
        let (owner, reply, read) = exchange.read(&value);
        assert_eq!(read, expected_read, "{label}");
        assert!(exchange.read_unanswered(owner), "{label}");
        let name = reply.into_token();
        exchange.release_read(ReplyTo::new(name), answer.clone());
        assert_eq!(exchange.read_answer(owner), expected_answer, "{label}");
        exchange.release_read(ReplyTo::new(name), reads::Answer::None(reads::Why::Unknown));
        assert_eq!(exchange.read_answer(owner), expected_answer, "{label}");
        assert!(exchange.component.pending.exchanges.is_empty(), "{label}");
        assert!(exchange.out.is_empty(), "{label}");
    }
}
#[test]
fn a_cursor_round_trips_to_the_same_position() {
    for (label, position) in f::cursors() {
        let encoded = crate::translate::cursor::encode(&position, 1024).expect(label);
        assert_eq!(crate::translate::cursor::decode(&encoded), Some(position.clone()), "{label}");
        let size = u32::try_from(encoded.len()).expect("small cursor");
        assert_eq!(crate::translate::cursor::encode(&position, size), Some(encoded.clone()), "{label}");
        assert_eq!(
            crate::translate::cursor::encode(&position, size.checked_sub(1).expect("cursor header")),
            None,
            "{label}"
        );
        for cut in 0..encoded.len() {
            assert_eq!(crate::translate::cursor::decode(&encoded[..cut]), None, "{label}, cut {cut}");
        }
        let mut trailing = skein_lib::Writer::new(encoded.len().checked_add(1).expect("small cursor"));
        trailing.put(&encoded).expect("cursor");
        trailing.put(&[0]).expect("extra byte");
        assert_eq!(crate::translate::cursor::decode(&trailing.finish()), None, "{label}, trailing");
    }
}
#[test]
fn a_cursor_that_does_not_decode_is_answered_unknown() {
    let limits = f::wire();
    for (label, bytes) in
        [("unknown format", &[255][..]), ("unknown variant", &[1, 255][..]), ("truncated scope", &[1, 1, 10][..])]
    {
        let mut exchange = f::read_exchange();
        let after = w::Cursor::new(&limits, w::CursorParts { value: bytes.into() }).expect(label);
        let value = w::Read::Spend(
            w::ReadSpend::new(&limits, w::ReadSpendParts { project: 3, after: Some(after) }).expect(label),
        );
        let owner = exchange.send(crate::Message::Read(f::owned_read(w::Owner::Jig, 19, f::encoded_read(&value))));
        exchange.authenticate();
        assert_eq!(exchange.read_answer(owner), f::read_none(w::ReadNone::Unknown), "{label}");
        assert!(exchange.out.is_empty(), "{label}");
        assert!(exchange.component.pending.exchanges.is_empty(), "{label}");
    }
}

#[test]
fn a_read_signed_out_is_answered_signed_out() {
    for label in ["core answer", "ended pending sign-in", "absent digest"] {
        let mut exchange = f::read_exchange();
        let owner = if label == "absent digest" {
            let owner =
                exchange.send(crate::Message::Read(f::owned_read(w::Owner::Jig, 19, f::encoded_read(&w::Read::Me))));
            let Some(Request::Secrets(SecretsRequest::Get { owner: who })) = exchange.out.pop() else {
                panic!("digest get")
            };
            exchange.component.down(
                &exchange.env,
                Event::Secrets(SecretsAnswer::Got { owner: who, value: None }),
                &mut exchange.out,
            );
            owner
        } else {
            let (owner, reply, _) = exchange.read(&w::Read::Me);
            if label == "core answer" {
                exchange.release_read(reply, reads::Answer::None(reads::Why::SignedOut));
            } else {
                exchange.component.down(&exchange.env, Event::SignInEnded { sign_in: f::NUMBER }, &mut exchange.out);
                let Some(Request::Secrets(SecretsRequest::Erase { owner: secret_owner })) = exchange.out.pop() else {
                    panic!("digest erase")
                };
                assert_eq!(secret_owner, SecretOwner::SignIn(f::NUMBER), "{label}");
            }
            owner
        };
        assert_eq!(exchange.read_answer(owner), f::read_none(w::ReadNone::SignedOut), "{label}");
        assert!(exchange.component.pending.exchanges.is_empty(), "{label}");
        assert!(exchange.out.is_empty(), "{label}");
    }
}
#[test]
fn an_application_read_is_routed_and_answered_under_its_family() {
    for label in ["document", "unknown"] {
        let mut exchange = f::read_exchange();
        let (owner, reply) = exchange.application_read();
        assert!(exchange.read_unanswered(owner), "{label}");
        let name = reply.into_token();
        exchange.release_application_read(ReplyTo::new(name), 8, None);
        assert!(exchange.read_unanswered(owner), "{label}");
        let record = if label == "document" { Some(Box::from([255, 0, 2])) } else { None };
        exchange.release_application_read(ReplyTo::new(name), 9, record);
        let expected = if label == "document" {
            let family = w::Family::new(&f::wire(), w::FamilyParts { number: 9 }).expect(label);
            w::ReadAnswer::Value(
                w::Owned::new(
                    &f::wire(),
                    w::OwnedParts { owner: w::Owner::Family(family), version: 2, record: Box::from([255, 0, 2]) },
                )
                .expect(label),
            )
        } else {
            f::read_none(w::ReadNone::Unknown)
        };
        assert_eq!(exchange.read_answer(owner), expected, "{label}");
        exchange.release_application_read(ReplyTo::new(name), 9, None);
        assert_eq!(exchange.read_answer(owner), expected, "{label}, late");
        assert!(exchange.component.pending.exchanges.is_empty(), "{label}");
    }
}

const TASK_QUERY: jig_domain_core::documents::TaskQuery = jig_domain_core::documents::TaskQuery {
    project: Some(3),
    requester: Some(jig_domain_tasks::Party::Person(5)),
    root: Some(7),
    parent: Some(11),
    label: Some(13),
    phase: None,
    executor: None,
    tracked: None,
    order: jig_domain_core::documents::QueryOrder::Number,
};
const QUERY_ORDERS: [jig_domain_core::documents::QueryOrder; 5] = {
    use jig_domain_core::documents::QueryOrder as O;
    [O::Number, O::Changed, O::Activity, O::Priority, O::Ended]
};
const QUERY_PHASES: [Option<jig_domain_core::documents::PhaseClass>; 6] = {
    use jig_domain_core::documents::PhaseClass as P;
    [None, Some(P::Working), Some(P::Waiting), Some(P::Held), Some(P::Closing), Some(P::Ended)]
};
const QUERY_EXECUTORS: [Option<jig_domain_core::documents::ExecutorKind>; 4] = {
    use jig_domain_core::documents::ExecutorKind as E;
    [None, Some(E::Agent), Some(E::Procedure), Some(E::Person)]
};
#[test]
fn task_query_cursors_preserve_their_keys_orders_and_phases() {
    for order in QUERY_ORDERS {
        for phase in QUERY_PHASES {
            for executor in QUERY_EXECUTORS {
                let query = jig_domain_core::documents::TaskQuery { order, phase, executor, ..TASK_QUERY };
                for query in [
                    query,
                    jig_domain_core::documents::TaskQuery { requester: None, tracked: Some(true), ..query },
                    jig_domain_core::documents::TaskQuery {
                        requester: Some(jig_domain_tasks::Party::Task(19)),
                        tracked: Some(false),
                        ..query
                    },
                ] {
                    let scope = reads::Scope::Tasks { query };
                    let cursors = [
                        reads::Cursor::Store { scope: scope.clone(), after: None },
                        reads::Cursor::Live {
                            scope,
                            after: jig_domain_core::documents::Cursor::Number { number: 17 },
                            store_after: None,
                        },
                    ];
                    for cursor in cursors {
                        let bytes = crate::translate::cursor::encode(&cursor, 1024).unwrap();
                        assert_eq!(crate::translate::cursor::decode(&bytes), Some(cursor));
                    }
                }
            }
        }
    }
}

#[test]
fn turn_failure_terminals_keep_their_reason_after_release() {
    for (why, expected) in [
        (reads::Why::Malformed, w::ReadNone::Malformed),
        (
            reads::Why::TooLarge { bytes: 8192 },
            w::ReadNone::TooLarge(
                w::TurnTooLarge::new(&f::wire(), w::TurnTooLargeParts { bytes: 8192 }).expect("size"),
            ),
        ),
    ] {
        let mut exchange = f::read_exchange();
        let (owner, reply, _) = exchange.read(&w::Read::Me);
        assert!(exchange.read_unanswered(owner));
        exchange.release_read(reply, reads::Answer::None(why));
        assert_eq!(exchange.read_answer(owner), f::read_none(expected));
    }
}

#[test]
fn a_single_turn_read_carries_the_same_archive_as_activity() {
    let page = f::activity_page();
    let reads::PageRow::Activity { turn: Some(reads::PageTurn::Carried(record)), .. } = page.rows.get(0).unwrap()
    else {
        panic!("carried fixture turn")
    };
    let limits = f::wire();
    let value = w::Read::Turn(
        w::ReadTurn::new(&limits, w::ReadTurnParts { task: record.task, attempt: record.attempt, turn: record.turn })
            .unwrap(),
    );
    let mut exchange = f::read_exchange();
    let (owner, reply, read) = exchange.read(&value);
    assert_eq!(read, reads::Read::Turn { task: record.task, attempt: record.attempt, turn: record.turn });
    exchange.release_read(
        reply,
        reads::Answer::Historical(Box::new(reads::Historical {
            value: reads::Past::Turn(record.clone()),
            at: None,
            by: None,
        })),
    );
    let w::ReadAnswer::Value(envelope) = exchange.read_answer(owner) else { panic!("turn answer") };
    let w::ReadValue::Turn(archive) =
        w::ReadValue::decode(&limits, &mut skein_lib::Reader::new(envelope.record())).unwrap()
    else {
        panic!("archive")
    };
    assert_eq!(
        (archive.task(), archive.name().attempt(), archive.name().number()),
        (record.task, record.attempt, record.turn)
    );
    assert_eq!(
        (archive.cumulative(), *archive.read(), archive.at()),
        (record.spent, record.read, record.at.as_nanos())
    );
    let w::TurnBytes::Carried(turn) = archive.turn() else { panic!("carried") };
    assert_eq!(turn.bytes(), record.transcript.as_ref());
}

#[test]
fn a_client_reading_a_too_linked_task_is_told_its_join_bound() {
    let mut exchange = f::read_exchange();
    let limits = f::wire();
    let task = w::Read::Task(w::ReadTask::new(&limits, w::ReadTaskParts { task: 1 }).expect("task"));
    let (owner, reply, _) = exchange.read(&task);
    exchange.release_read(reply, reads::Answer::None(reads::Why::TooLinked { joins: 4 }));
    // read_answer decodes the real engine response bytes with the client codec.
    let w::ReadAnswer::None(reason) = exchange.read_answer(owner) else { panic!("read refusal") };
    assert_eq!(
        jig_client_protocol_link::translate::reads::none(reason.value()),
        jig_client_domain::contract::ReadNone::TooLinked { joins: 4 },
    );
}
