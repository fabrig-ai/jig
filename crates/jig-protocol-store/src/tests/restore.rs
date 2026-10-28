//! Ordered restart pages and census joins (protocol/store.md, 7.2).
use super::fixtures::{self, ALICE, ENDED, LIVE};
use crate::{Cursor, Event, Phase, Request, Restore, Row, Store, Write, keys};
use jig_domain_core::{Census, ReadResults, Record, RestoreRange};
use skein_lib::{List, Token};

const OWNER: Token = Token::new(5);

fn live_store() -> Store {
    let mut store = fixtures::opened();
    fixtures::committed(
        &mut store,
        Box::new([
            fixtures::header(1),
            fixtures::save(fixtures::live_task(LIVE)),
            fixtures::save(fixtures::person(ALICE)),
            fixtures::save(fixtures::ended_task(ENDED)),
        ]),
    );
    store
}

fn restore(store: &mut Store, range: Restore, after: Option<Cursor>, rows: u32, bytes: u64) -> Box<[Event]> {
    fixtures::run(store, Request::Restore { owner: OWNER, range, after, rows, bytes })
}

fn live_page(store: &mut Store, after: Option<Cursor>, rows: u32, bytes: u64) -> Box<[Event]> {
    restore(store, Restore::Jig(Box::new(RestoreRange::LIVE)), after, rows, bytes)
}

fn page(events: Box<[Event]>) -> (Box<[Row]>, Option<Cursor>) {
    let mut events = events.into_iter();
    let Some(Event::Restored { owner, rows, next }) = events.next() else {
        panic!("one restored page");
    };
    assert_eq!(owner, OWNER);
    assert!(events.next().is_none());
    (rows, next)
}

fn jig_rows(rows: Box<[Row]>) -> Box<[Record]> {
    let mut found = List::with_capacity(u32::try_from(rows.len()).expect("fixture page count"));
    for row in rows {
        let Row::Jig(record) = row else { panic!("typed jig row") };
        found.push(record).expect("counted rows");
    }
    found.into_boxed()
}

fn assert_corrupt(store: &mut Store, events: &[Event]) {
    let [Event::Corrupt { owner }] = events else { panic!("one corruption terminal: {events:?}") };
    assert_eq!(*owner, OWNER);
    assert_eq!(store.phase, Phase::Stopped);
}

fn census() -> Restore {
    Restore::Jig(Box::new([RestoreRange::Census(Census {
        questions: Box::new([]),
        results: Box::new([ReadResults { party: ALICE, after: 0 }]),
    })]))
}

fn census_store(include_ended: bool) -> Store {
    let mut store = fixtures::opened();
    fixtures::committed(
        &mut store,
        Box::new([fixtures::header(1), fixtures::save(fixtures::result_index(ALICE, 1, ENDED))]),
    );
    if include_ended {
        fixtures::committed(&mut store, Box::new([fixtures::header(2), fixtures::save(fixtures::ended_task(ENDED))]));
    }
    store
}

#[test]
fn the_core_restore_hands_up_header_people_then_live_tasks() {
    let mut store = live_store();
    let (rows, next) = page(live_page(&mut store, None, 8, 65_536));
    assert_eq!(next, None);
    assert_eq!(
        jig_rows(rows).as_ref(),
        &[fixtures::header_record(1), fixtures::person(ALICE), fixtures::live_task(LIVE)],
    );
}

#[test]
fn run_proofs_follow_live_tasks_and_calls_follow_run_proofs() {
    let mut store = live_store();
    fixtures::committed(
        &mut store,
        Box::new([
            fixtures::header(2),
            fixtures::save(fixtures::call(LIVE, 3)),
            fixtures::save(fixtures::run_proof(LIVE)),
        ]),
    );
    let (rows, next) = page(live_page(&mut store, None, 8, 65_536));
    assert_eq!(next, None);
    assert_eq!(
        jig_rows(rows).as_ref(),
        &[
            fixtures::header_record(2),
            fixtures::person(ALICE),
            fixtures::live_task(LIVE),
            fixtures::run_proof(LIVE),
            fixtures::call(LIVE, 3),
        ],
    );
}

#[test]
fn ended_tasks_history_and_notes_are_never_read_at_start() {
    let mut store = live_store();
    fixtures::committed(
        &mut store,
        Box::new([
            fixtures::header(2),
            fixtures::save(fixtures::history(ENDED, 3)),
            fixtures::save(fixtures::notes_entry(17)),
        ]),
    );
    let (rows, next) = page(live_page(&mut store, None, 8, 65_536));
    assert_eq!(next, None);
    assert_eq!(
        jig_rows(rows).as_ref(),
        &[fixtures::header_record(2), fixtures::person(ALICE), fixtures::live_task(LIVE)],
    );
}

fn accounts_rows() -> [Record; 3] {
    use jig_domain_accounts::{Held, Holder, Kind, Stored};
    [
        Record::Accounts(Stored::Next { number: 3 }),
        Record::Accounts(Stored::Account {
            place: 0,
            uses: None,
            number: 1,
            provider: 1,
            holder: Holder::Party(ALICE),
            subject: Box::from(&b"alice@example"[..]),
            display: Box::from(&b"Alice"[..]),
        }),
        Record::Accounts(Stored::Source {
            account: 1,
            number: 2,
            place: 0,
            kind: Kind::SignIn,
            name: Box::new([]),
            held: Some(Held::Limited { reset: Some(skein_lib::Wall::from_nanos(5)) }),
        }),
    ]
}

#[test]
fn the_accounts_rows_follow_the_header_without_the_installations_row() {
    let mut store = live_store();
    let [next, account, source] = accounts_rows();
    fixtures::committed(
        &mut store,
        Box::new([
            fixtures::header(2),
            fixtures::save(source.clone()),
            fixtures::save(account.clone()),
            fixtures::save(next.clone()),
        ]),
    );
    let (rows, after) = page(live_page(&mut store, None, 8, 65_536));
    assert_eq!(after, None);
    assert_eq!(
        jig_rows(rows).as_ref(),
        &[fixtures::header_record(2), next, account, source, fixtures::person(ALICE), fixtures::live_task(LIVE)],
    );
}

#[test]
fn an_erased_accounts_row_is_not_restored() {
    let mut store = live_store();
    let [next, account, source] = accounts_rows();
    fixtures::committed(
        &mut store,
        Box::new([fixtures::header(2), fixtures::save(next.clone()), fixtures::save(account), fixtures::save(source)]),
    );
    fixtures::committed(
        &mut store,
        Box::new([
            fixtures::header(3),
            Write::Erase(jig_domain_core::Key::Accounts(jig_domain_accounts::Key::Source { account: 1, number: 2 })),
            Write::Erase(jig_domain_core::Key::Accounts(jig_domain_accounts::Key::Account { number: 1 })),
        ]),
    );
    let (rows, _) = page(live_page(&mut store, None, 8, 65_536));
    assert_eq!(
        jig_rows(rows).as_ref(),
        &[fixtures::header_record(3), next, fixtures::person(ALICE), fixtures::live_task(LIVE)],
    );
}

#[test]
fn a_restore_page_goes_on_into_the_next_interval() {
    let mut store = live_store();
    let (rows, next) = page(live_page(&mut store, None, 2, 65_536));
    assert_eq!(jig_rows(rows).as_ref(), &[fixtures::header_record(1), fixtures::person(ALICE)]);
    assert_eq!(
        next,
        Some(Cursor {
            interval: 4,
            after: keys::encode(&fixtures::person(ALICE).key(), fixtures::KEY_LIMIT).expect("key fits")
        }),
    );
}

#[test]
fn the_cursor_resumes_just_past_the_last_row() {
    let mut store = live_store();
    let (_, next) = page(live_page(&mut store, None, 2, 65_536));
    let (rows, next) = page(live_page(&mut store, next, 2, 65_536));
    assert_eq!(next, None);
    assert_eq!(jig_rows(rows).as_ref(), &[fixtures::live_task(LIVE)]);
}

#[test]
fn the_census_hands_up_the_ended_task_its_row_names() {
    let mut store = census_store(true);
    let (rows, next) = page(restore(&mut store, census(), None, 2, 65_536));
    assert_eq!(next, None);
    assert_eq!(jig_rows(rows).as_ref(), &[fixtures::ended_task(ENDED)]);
}

#[test]
#[should_panic(expected = "links to Ended(22), which is absent")]
fn a_census_row_without_its_ended_task_panics_at_the_link() {
    let mut store = census_store(false);
    let _events = restore(&mut store, census(), None, 2, 65_536);
}

#[test]
fn a_connectors_interval_hands_up_rows_without_the_tag() {
    let mut store = fixtures::opened();
    fixtures::committed(
        &mut store,
        Box::new([
            fixtures::header(1),
            Write::Put { key: Box::from(b"a1".as_slice()), value: Box::new([7]) },
            Write::Put { key: Box::from(b"b1".as_slice()), value: Box::new([9]) },
        ]),
    );
    let ranges = Box::new([skein_kv::Range::prefix(b"a")]);
    let (rows, next) = page(restore(&mut store, Restore::Application(ranges), None, 2, 64));
    assert_eq!(next, None);
    let [Row::Application { key, value }] = rows.as_ref() else { panic!("one application row") };
    assert_eq!(key.as_ref(), b"a1");
    assert_eq!(value.as_ref(), &[7]);
}

fn corrupt_people_store() -> Store {
    let mut stores = fixtures::stores();
    fixtures::plant(
        &mut stores,
        &[
            fixtures::format_put(fixtures::DEPLOYMENT),
            fixtures::layouts_put(),
            fixtures::header_put(fixtures::DEPLOYMENT, 1),
            fixtures::installation_put(fixtures::INSTALLATION),
            skein_kv::Op::Put {
                key: keys::encode(&fixtures::person(ALICE).key(), fixtures::KEY_LIMIT).expect("person key fits"),
                value: Box::new([255]),
            },
        ],
    );
    let mut store = fixtures::over(stores);
    let events = fixtures::run(
        &mut store,
        Request::Open { owner: OWNER, root: fixtures::ROOT, installation: fixtures::INSTALLATION },
    );
    let [Event::Opened { .. }] = &*events else { panic!("fixture store opens: {events:?}") };
    store
}

#[test]
fn an_undecodable_restored_row_is_corrupt() {
    let mut store = corrupt_people_store();
    let events = live_page(&mut store, None, 2, 65_536);
    assert_corrupt(&mut store, &events);
}

#[test]
fn a_restore_page_keeps_the_prefix_within_the_decoded_bytes() {
    let mut store = live_store();
    // The deployment fits 100 measured bytes; adding the person exceeds it.
    let (rows, next) = page(live_page(&mut store, None, 2, 100));
    assert_eq!(jig_rows(rows).as_ref(), &[fixtures::header_record(1)]);
    let (rows, _) = page(live_page(&mut store, next, 2, 65_536));
    assert_eq!(jig_rows(rows).as_ref(), &[fixtures::person(ALICE), fixtures::live_task(LIVE)]);
}

#[test]
fn a_restore_page_keeps_the_prefix_within_the_encoded_bytes() {
    let mut store = live_store();
    store.limits.store.page.bytes = 105;
    let (rows, next) = page(live_page(&mut store, None, 2, 65_536));
    assert_eq!(jig_rows(rows).as_ref(), &[fixtures::header_record(1)]);
    assert!(next.is_some());
}

#[test]
fn nothing_is_answered_after_a_corrupt_restore() {
    let mut store = corrupt_people_store();
    let events = live_page(&mut store, None, 2, 65_536);
    assert_corrupt(&mut store, &events);
    assert!(live_page(&mut store, None, 2, 65_536).is_empty());
}

#[test]
fn the_census_hands_up_conversations_before_result_rows() {
    let mut store = census_store(true);
    fixtures::committed(
        &mut store,
        Box::new([
            fixtures::header(3),
            fixtures::save(ended_position(LIVE)),
            fixtures::save(fixtures::ended_task(LIVE)),
        ]),
    );
    let (rows, next) = page(restore(&mut store, census(), None, 3, 65_536));
    assert_eq!(next, None);
    assert_eq!(
        jig_rows(rows).as_ref(),
        &[ended_position(LIVE), fixtures::ended_task(LIVE), fixtures::ended_task(ENDED)]
    );
}

#[test]
fn the_census_counts_the_joined_task_against_decoded_bytes() {
    let mut store = census_store(true);
    let (rows, next) = page(restore(&mut store, census(), None, 2, 100));
    assert!(rows.is_empty());
    assert!(next.is_some());
    let (rows, next) = page(restore(&mut store, census(), next, 2, 65_536));
    assert_eq!(next, None);
    assert_eq!(jig_rows(rows).as_ref(), &[fixtures::ended_task(ENDED)]);
}

fn application_store() -> Store {
    let mut store = fixtures::opened();
    fixtures::committed(
        &mut store,
        Box::new([
            fixtures::header(1),
            Write::Put { key: Box::from(b"a1".as_slice()), value: Box::new([7]) },
            Write::Put { key: Box::from(b"a2".as_slice()), value: Box::new([9]) },
        ]),
    );
    store
}

fn application_page(store: &mut Store, after: Option<Cursor>, bytes: u64) -> Box<[Event]> {
    let ranges = Box::new([skein_kv::Range::prefix(b"a"), skein_kv::Range::prefix(b"a")]);
    restore(store, Restore::Application(ranges), after, 8, bytes)
}

fn application_keys(rows: Box<[Row]>) -> Box<[Box<[u8]>]> {
    let mut keys = List::with_capacity(u32::try_from(rows.len()).expect("fixture row count"));
    for row in rows {
        let Row::Application { key, .. } = row else { panic!("application row") };
        keys.push(key).expect("counted rows");
    }
    keys.into_boxed()
}

#[test]
fn a_decoded_cut_keeps_the_cursor_in_its_duplicate_interval() {
    let mut store = application_store();
    let (rows, next) = page(application_page(&mut store, None, 9));
    assert_eq!(
        application_keys(rows).as_ref(),
        &[Box::from(b"a1".as_slice()), Box::from(b"a2".as_slice()), Box::from(b"a1".as_slice())]
    );
    assert_eq!(next.as_ref().expect("fourth row remains").interval, 1);
    let (rows, next) = page(application_page(&mut store, next, 9));
    assert_eq!(next, None);
    assert_eq!(application_keys(rows).as_ref(), &[Box::from(b"a2".as_slice())]);
}

#[test]
fn a_maximum_length_keys_cursor_resumes_without_refusal() {
    let mut store = application_store();
    store.limits.store.key = 3;
    let ranges = Box::new([skein_kv::Range::prefix(b"a")]);
    let (_, next) = page(restore(&mut store, Restore::Application(ranges.clone()), None, 1, 64));
    let (rows, next) = page(restore(&mut store, Restore::Application(ranges), next, 1, 64));
    assert_eq!(next, None);
    assert_eq!(application_keys(rows).as_ref(), &[Box::from(b"a2".as_slice())]);
}

const SECOND_ENDED: u64 = ENDED + 1;
fn ended_position(task: u64) -> Record {
    Record::People(jig_domain_people::Stored::EndedRead { task, party: ALICE, through: 1 })
}
fn positions_store() -> Store {
    let mut store = fixtures::opened();
    fixtures::committed(
        &mut store,
        Box::new([
            fixtures::header(1),
            fixtures::save(ended_position(ENDED)),
            fixtures::save(fixtures::ended_task(ENDED)),
        ]),
    );
    fixtures::committed(
        &mut store,
        Box::new([
            fixtures::header(2),
            fixtures::save(ended_position(SECOND_ENDED)),
            fixtures::save(fixtures::ended_task(SECOND_ENDED)),
        ]),
    );
    store
}
#[test]
fn the_census_hands_up_an_ended_read_position_then_its_task() {
    let mut store = positions_store();
    let (rows, next) = page(restore(&mut store, census(), None, 4, 65_536));
    assert_eq!(next, None);
    assert_eq!(
        jig_rows(rows).as_ref(),
        &[
            ended_position(ENDED),
            fixtures::ended_task(ENDED),
            ended_position(SECOND_ENDED),
            fixtures::ended_task(SECOND_ENDED)
        ]
    );
}
#[test]
fn the_census_never_parts_an_ended_read_position_from_its_task() {
    let mut store = positions_store();
    let (rows, next) = page(restore(&mut store, census(), None, 3, 65_536));
    assert_eq!(jig_rows(rows).as_ref(), &[ended_position(ENDED), fixtures::ended_task(ENDED)]);
    assert_eq!(
        next,
        Some(Cursor {
            interval: 0,
            after: keys::encode(&ended_position(ENDED).key(), fixtures::KEY_LIMIT).expect("position key")
        })
    );
    let (rows, next) = page(restore(&mut store, census(), None, 1, 65_536));
    assert!(rows.is_empty());
    assert!(next.is_some());
}
