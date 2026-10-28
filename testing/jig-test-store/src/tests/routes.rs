use super::fixtures;
use crate::{StoreOut, Up, route};
use jig_domain_core as core;
use jig_domain_people as people;
use jig_test_domain as root;
use skein_lib::Token;

#[test]
fn a_commit_routes_connector_records_as_bytes() {
    let mut routes = fixtures::routes();
    let record = jig_test_connector::Record::Pool { path: fixtures::path(), slots: 1 };
    let request = route::down(
        &mut routes,
        StoreOut::Commit { number: 1, writes: Box::new([root::Write::Save(fixtures::root_record(record))]) },
    )
    .expect("commit request");
    let writes = fixtures::commit_writes(request);
    match writes.as_ref() {
        [jig_protocol_store::Write::Put { key, value }] => {
            assert_eq!(key.as_ref(), &[0, 0, 0, 0, 0, 0, 0, 1, 8, 1, 1, 0, 0, 1, 1, 0, 0, 0]);
            assert_eq!(value.as_ref(), &[0, 2, 9, 0, 0, 0, 2, 0, 0, 0, 1, 1, 0, 0, 0, 1, 1, 0, 0, 0, 1]);
        }
        _ => unreachable!("one application's put"),
    }
}
#[test]
fn load_core_pages_reach_the_root_then_people_restored() {
    let mut component = fixtures::component();
    let mut routes = fixtures::routes();
    fixtures::routed(&mut component, &mut routes, fixtures::core_commit());
    let outputs = fixtures::routed(
        &mut component,
        &mut routes,
        StoreOut::Restart { step: core::RestartStep::LoadCore, census: None },
    );
    let mut rows = 0_u32;
    let mut restored = false;
    for output in outputs {
        match output {
            Up::Restore(_) => {
                assert!(!restored);
                rows = rows.checked_add(1).expect("bounded rows");
            }
            Up::Root(root::Event::Core(core::Event::People(people::Event::Restored))) => {
                assert_eq!(rows, 3);
                restored = true;
            }
            Up::Root(root::Event::RestartDone(step)) => {
                assert!(restored);
                assert_eq!(step, core::RestartStep::LoadCore);
            }
            Up::Root(_) | Up::Ask(_) => unreachable!("restart rows and terminals only"),
        }
    }
}
#[test]
fn a_connectors_step_restores_its_records() {
    let mut component = fixtures::component();
    let mut routes = fixtures::routes();
    let record = jig_test_connector::Record::Pool { path: fixtures::path(), slots: 1 };
    fixtures::routed(
        &mut component,
        &mut routes,
        StoreOut::Commit {
            number: 1,
            writes: Box::new([fixtures::header(1), root::Write::Save(fixtures::root_record(record.clone()))]),
        },
    );
    let outputs = fixtures::routed(
        &mut component,
        &mut routes,
        StoreOut::Restart { step: core::RestartStep::RestoreConnector { connector: fixtures::FIRST }, census: None },
    );
    match outputs.as_ref() {
        [
            Up::Restore(row),
            Up::Root(root::Event::RestartDone(core::RestartStep::RestoreConnector { connector: 1 })),
        ] => {
            assert_eq!(*row, fixtures::root_record(record));
        }
        _ => unreachable!("the typed connector row then its terminal"),
    }
}
#[test]
fn a_seen_conversation_is_answered_from_the_store() {
    let mut component = fixtures::component();
    let mut routes = fixtures::routes();
    let row = fixtures::ended_chat();
    let task = row.number;
    fixtures::routed(
        &mut component,
        &mut routes,
        StoreOut::Commit {
            number: 1,
            writes: Box::new([
                fixtures::header(1),
                root::Write::Save(root::Record::Core(core::Record::Tasks(jig_domain_tasks::Stored::Ended(
                    row.clone(),
                )))),
            ]),
        },
    );
    let outputs = fixtures::routed(
        &mut component,
        &mut routes,
        StoreOut::Now(core::Now::SeenConversation { request: Token::new(17), task }),
    );
    match outputs.as_ref() {
        [
            Up::Root(root::Event::Core(core::Event::SeenConversationLoaded { request, row: Some(found), busy: false })),
        ] => {
            assert_eq!(*request, Token::new(17));
            assert_eq!(*found, row);
        }
        _ => unreachable!("the retained chat answers its request"),
    }
}
#[test]
fn a_window_read_reaches_the_root_as_transcript_pages() {
    let mut component = fixtures::component();
    let mut routes = fixtures::routes();
    fixtures::routed(
        &mut component,
        &mut routes,
        StoreOut::Commit {
            number: 1,
            writes: Box::new([fixtures::header(1), fixtures::turn(1), fixtures::turn(2), fixtures::turn(3)]),
        },
    );
    let outputs = fixtures::routed(
        &mut component,
        &mut routes,
        StoreOut::Now(core::Now::StartPreparation { task: 21, transcript_waiter: None, from: Some((1, 1)) }),
    );
    let mut expected_turn = 1_u32;
    let mut pages = 0_u32;
    for output in outputs {
        match output {
            Up::Root(root::Event::TranscriptLoaded { task, rows, done }) => {
                assert_eq!(task, 21);
                pages = pages.checked_add(1).expect("bounded pages");
                for (row, text) in rows {
                    assert!(text.is_none());
                    assert_eq!(row.turn, expected_turn);
                    assert_eq!(row.transcript.as_ref(), b"opaque");
                    expected_turn = expected_turn.checked_add(1).expect("bounded turns");
                }
                assert_eq!(done, expected_turn == 4);
            }
            Up::Root(_) | Up::Restore(_) | Up::Ask(_) => unreachable!("only transcript pages"),
        }
    }
    assert_eq!(pages, 2);
}

#[test]
#[should_panic(expected = "store: restore range LoadCore did not advance cursor")]
fn a_restore_page_cannot_repeat_its_empty_cursor() {
    let mut routes = fixtures::routes();
    let request = route::down(&mut routes, StoreOut::Restart { step: core::RestartStep::LoadCore, census: None })
        .expect("restore request");
    let jig_protocol_store::Request::Restore { owner, .. } = request else { panic!("restore") };
    let mut into = skein_lib::Queue::with_capacity(8);
    let cursor = jig_protocol_store::Cursor { interval: 1, after: Box::from(&b"last row"[..]) };
    route::up(
        &mut routes,
        jig_protocol_store::Event::Restored { owner, rows: Box::new([]), next: Some(cursor.clone()) },
        &mut into,
    );
    route::up(
        &mut routes,
        jig_protocol_store::Event::Restored { owner, rows: Box::new([]), next: Some(cursor) },
        &mut into,
    );
}

#[test]
fn restore_pages_use_store_capacity_instead_of_read_allowance() {
    const STORE_ROW_PAIR: u64 = 33_556_480; // 2 * (fixture key 1024 + value 16777216).
    let mut routes = fixtures::routes();
    let request = route::down(&mut routes, StoreOut::Restart { step: core::RestartStep::LoadCore, census: None })
        .expect("restore request");
    let jig_protocol_store::Request::Restore { bytes, .. } = request else { panic!("restore") };
    assert_eq!(bytes, STORE_ROW_PAIR);
}
