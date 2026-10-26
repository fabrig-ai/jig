//! Checks before a memory restart (protocol/store.md, 5.3, 6 and 10).
mod levels;
use super::fixtures;
use crate::{Event, Family, Older, OpenRefusal, Phase, Request, Store, Stores, codec, keys};
use alloc::boxed::Box;
use jig_domain_core::{CoreKey, Key};
use skein_lib::Token;

fn open(store: &mut Store) -> Box<[Event]> {
    open_drawing(store, fixtures::INSTALLATION)
}

fn open_drawing(store: &mut Store, installation: [u8; 16]) -> Box<[Event]> {
    fixtures::run(store, Request::Open { owner: Token::new(1), root: fixtures::ROOT, installation })
}

fn planted(ops: &[skein_kv::Op]) -> Store {
    let mut stores = fixtures::stores();
    fixtures::plant(&mut stores, ops);
    fixtures::over(stores)
}

// Format, Layouts, deployment header and installation identifier.
const OPENED_ROWS: usize = 4;

fn opened_rows() -> [skein_kv::Op; OPENED_ROWS] {
    [
        fixtures::format_put(fixtures::DEPLOYMENT),
        fixtures::layouts_put(),
        fixtures::header_put(fixtures::DEPLOYMENT, 1),
        fixtures::installation_put(fixtures::INSTALLATION),
    ]
}

fn fixture_format() -> (Box<[u8]>, codec::component::Format) {
    let skein_kv::Op::Put { key, value } = fixtures::format_put(fixtures::DEPLOYMENT) else {
        panic!("format is a put");
    };
    (key, codec::component::Format::decode(&value).expect("format fixture"))
}

fn stores_with_header() -> Stores {
    let mut stores = fixtures::stores();
    fixtures::plant(&mut stores, &opened_rows());
    stores
}

fn header_without_format() -> Store {
    planted(&[fixtures::header_put(fixtures::DEPLOYMENT, 1)])
}

fn version_put(tasks: u16, people: u16, core: u16) -> skein_kv::Op {
    let (key, mut format) = fixture_format();
    format.tasks = tasks;
    format.people = people;
    format.core = core;
    skein_kv::Op::Put { key, value: format.encode().expect("format fits") }
}

fn accounts_put(accounts: u16) -> skein_kv::Op {
    let (key, mut format) = fixture_format();
    format.accounts = accounts;
    skein_kv::Op::Put { key, value: format.encode().expect("format fits") }
}

fn assert_refusal(events: &[Event], why: OpenRefusal) {
    let [Event::OpenRefused { owner, why: found }] = events else {
        panic!("one refusal: {events:?}");
    };
    assert_eq!(*owner, Token::new(1));
    assert_eq!(*found, why);
}

fn assert_opened(events: &[Event], last: u64) {
    assert_opened_with(events, last, fixtures::INSTALLATION);
}

fn assert_opened_with(events: &[Event], last: u64, installation: [u8; 16]) {
    let [Event::Opened { owner, last: found, installation: answered }] = events else {
        panic!("one opened terminal: {events:?}");
    };
    assert_eq!(*owner, Token::new(1));
    assert_eq!(*found, last);
    assert_eq!(*answered, installation);
}

fn assert_corrupt(events: &[Event]) {
    let [Event::Corrupt { owner }] = events else {
        panic!("one corrupt terminal: {events:?}");
    };
    assert_eq!(*owner, Token::new(1));
}

fn corrupt_header() -> skein_kv::Op {
    skein_kv::Op::Put {
        key: keys::encode(&Key::Core(CoreKey::Deployment), fixtures::KEY_LIMIT).expect("header key fits"),
        value: Box::new([]),
    }
}

#[test]
fn an_empty_store_opens_with_its_format_row_due() {
    let store = fixtures::opened();
    assert_eq!(store.phase, Phase::Ready { next: 1, format_due: true });
    let stores = store.into_stores();
    assert_eq!(stores.store.last(), 0);
    assert!(stores.store.is_empty());
}

#[test]
fn an_empty_store_answers_the_drawn_installation() {
    let mut store = fixtures::over(fixtures::stores());
    assert_opened_with(&open_drawing(&mut store, [6; 16]), 0, [6; 16]);
}

#[test]
fn a_store_answers_its_kept_installation_not_the_drawn_one() {
    let mut store = planted(&opened_rows());
    assert_opened_with(&open_drawing(&mut store, [6; 16]), 1, fixtures::INSTALLATION);
}

#[test]
fn a_checked_store_without_its_installation_is_corrupt() {
    let mut store = planted(&[
        fixtures::format_put(fixtures::DEPLOYMENT),
        fixtures::layouts_put(),
        fixtures::header_put(fixtures::DEPLOYMENT, 1),
    ]);
    assert_corrupt(&open(&mut store));
    assert_eq!(store.phase, Phase::Stopped);
}

#[test]
fn an_undecodable_installation_is_corrupt() {
    let mut store = planted(&[
        fixtures::format_put(fixtures::DEPLOYMENT),
        fixtures::layouts_put(),
        fixtures::header_put(fixtures::DEPLOYMENT, 1),
        skein_kv::Op::Put { key: keys::installation(fixtures::KEY_LIMIT).expect("key fits"), value: Box::new([0, 1]) },
    ]);
    assert_corrupt(&open(&mut store));
}

#[test]
fn a_format_row_of_another_component_version_refuses_naming_it() {
    let skein_kv::Op::Put { key, mut value } = fixtures::format_put(fixtures::DEPLOYMENT) else {
        panic!("format is a put");
    };
    let [high, low, ..] = &mut *value else { panic!("a version prefix") };
    (*high, *low) = (0, 2);
    let mut store = planted(&[skein_kv::Op::Put { key, value }, fixtures::header_put(fixtures::DEPLOYMENT, 1)]);
    let events = open(&mut store);
    assert_refusal(&events, OpenRefusal::Format { family: Family::Component, found: 2, wanted: 3 });
    super::words::has_deployment_slots(&events, super::words::FORMAT);
}

#[test]
fn a_store_of_another_accounts_version_refuses_naming_accounts() {
    let mut store = planted(&[accounts_put(1), fixtures::header_put(fixtures::DEPLOYMENT, 1)]);
    assert_refusal(&open(&mut store), OpenRefusal::Format { family: Family::Accounts, found: 1, wanted: 2 });
}

#[test]
fn a_store_of_another_deployment_refuses_to_open() {
    let mut store = planted(&[
        fixtures::format_put(fixtures::OTHER_DEPLOYMENT),
        fixtures::layouts_put(),
        fixtures::header_put(fixtures::DEPLOYMENT, 1),
    ]);
    let events = open(&mut store);
    assert_refusal(&events, OpenRefusal::Deployment);
    super::words::has_deployment_slots(&events, super::words::DEPLOYMENT);
}

#[test]
fn a_store_of_another_tasks_version_refuses_naming_tasks() {
    let mut store = planted(&[version_put(6, 8, 6), fixtures::header_put(fixtures::DEPLOYMENT, 1)]);
    assert_refusal(&open(&mut store), OpenRefusal::Format { family: Family::Tasks, found: 6, wanted: 9 });
}

#[test]
fn a_store_of_another_people_version_refuses_naming_people() {
    let mut store = planted(&[version_put(9, 10, 6), fixtures::header_put(fixtures::DEPLOYMENT, 1)]);
    assert_refusal(&open(&mut store), OpenRefusal::Format { family: Family::People, found: 10, wanted: 13 });
}

#[test]
fn a_header_behind_the_last_commit_refuses_naming_both_numbers() {
    let mut stores = stores_with_header();
    fixtures::plant(&mut stores, &[]);
    let mut store = fixtures::over(stores);
    let events = open(&mut store);
    assert_refusal(&events, OpenRefusal::Numbers { store: 2, header: 1 });
    super::words::has_deployment_slots(&events, super::words::NUMBERS);
}

#[test]
fn a_header_without_its_format_row_refuses_as_format() {
    let mut store = header_without_format();
    assert_refusal(&open(&mut store), OpenRefusal::Format { family: Family::Component, found: 0, wanted: 3 });
}

#[test]
fn a_header_equal_to_the_last_commit_opens_with_it() {
    let mut store = planted(&opened_rows());
    assert_opened(&open(&mut store), 1);
    assert_eq!(store.phase, Phase::Ready { next: 2, format_due: false });
}

#[test]
fn a_format_row_without_its_header_refuses_as_numbers() {
    let mut store = planted(&[fixtures::format_put(fixtures::DEPLOYMENT), fixtures::layouts_put()]);
    assert_refusal(&open(&mut store), OpenRefusal::Numbers { store: 1, header: 0 });
}

#[test]
fn an_unknown_core_version_is_refused_before_decoding_its_header() {
    let mut store = planted(&[version_put(9, 8, 5), corrupt_header()]);
    assert_refusal(&open(&mut store), OpenRefusal::Format { family: Family::Core, found: 5, wanted: 6 });
}

#[test]
fn a_corrupt_header_stops_the_open() {
    let mut store = planted(&[fixtures::format_put(fixtures::DEPLOYMENT), fixtures::layouts_put(), corrupt_header()]);
    assert_corrupt(&open(&mut store));
    assert_eq!(store.phase, Phase::Stopped);
}

#[test]
fn a_corrupt_format_stops_the_open() {
    let mut store = planted(&[skein_kv::Op::Put {
        key: keys::encode_own(&keys::Own::Format, fixtures::KEY_LIMIT).expect("format key fits"),
        value: Box::new([]),
    }]);
    assert_corrupt(&open(&mut store));
    assert_eq!(store.phase, Phase::Stopped);
}

#[test]
fn a_stopped_open_answers_nothing_further() {
    let mut store = header_without_format();
    assert_refusal(&open(&mut store), OpenRefusal::Format { family: Family::Component, found: 0, wanted: 3 });
    assert!(open(&mut store).is_empty());
}

#[test]
fn opening_checks_no_row_beyond_format_layouts_header_and_installation() {
    let mut stores = stores_with_header();
    fixtures::plant(&mut stores, &[skein_kv::Op::Put { key: Box::new([1, 0, 255]), value: Box::new([]) }]);
    fixtures::plant(&mut stores, &[fixtures::header_put(fixtures::DEPLOYMENT, 3)]);
    let mut store = fixtures::over(stores);
    assert_opened(&open(&mut store), 3);
}

#[test]
fn the_callers_two_stores_are_handed_back_without_changes() {
    let stores = stores_with_header();
    let expected = stores.store.used();
    let stores: Stores = fixtures::over(stores).into_stores();
    assert_eq!(stores.store.last(), 1);
    assert_eq!(stores.store.used(), expected);
    assert_eq!(stores.secrets.last(), 0);
    assert!(stores.secrets.is_empty());
}

#[test]
fn a_store_without_recorded_layouts_refuses_before_decoding_its_header() {
    let mut store = planted(&[fixtures::format_put(fixtures::DEPLOYMENT), corrupt_header()]);
    assert_refusal(&open(&mut store), OpenRefusal::OlderBuild { why: Older::Unrecorded });
}

fn layouts_put(layouts: &codec::component::Layouts) -> skein_kv::Op {
    skein_kv::Op::Put {
        key: keys::encode_own(&keys::Own::Layouts, fixtures::KEY_LIMIT).expect("layouts key fits"),
        value: layouts.encode(),
    }
}

#[test]
fn each_different_family_layout_refuses_before_decoding_its_header() {
    let layouts = crate::format::layouts();
    let mut component = layouts;
    component.component[0] ^= 1;
    let mut core = layouts;
    core.core[0] ^= 1;
    let mut tasks = layouts;
    tasks.tasks[0] ^= 1;
    let mut people = layouts;
    people.people[0] ^= 1;
    let mut notes = layouts;
    notes.notes[0] ^= 1;
    let mut accounts = layouts;
    accounts.accounts[0] ^= 1;
    let mut settings = layouts;
    settings.settings[0] ^= 1;
    let mut secrets = layouts;
    secrets.secrets[0] ^= 1;
    let mut skein_oauth = layouts;
    skein_oauth.skein_oauth[0] ^= 1;
    let mut smith_transcript = layouts;
    smith_transcript.smith_transcript[0] ^= 1;
    for (family, changed) in [
        (Family::Settings, settings),
        (Family::Component, component),
        (Family::Core, core),
        (Family::Tasks, tasks),
        (Family::People, people),
        (Family::Notes, notes),
        (Family::Accounts, accounts),
        (Family::Secrets, secrets),
        (Family::SkeinOauth, skein_oauth),
        (Family::SmithTranscript, smith_transcript),
    ] {
        let mut store = planted(&[fixtures::format_put(fixtures::DEPLOYMENT), layouts_put(&changed), corrupt_header()]);
        assert_refusal(&open(&mut store), OpenRefusal::OlderBuild { why: Older::Layout(family) });
    }
}

#[test]
fn undecodable_component_rows_refuse_as_an_older_component_layout() {
    for (key, value) in [
        (keys::Own::Layouts, Box::from([])),
        (keys::Own::Layouts, Box::from([0, 3])),
        (keys::Own::Format, Box::from([0, 3])),
    ] {
        let mut store = replacing(skein_kv::Op::Put {
            key: keys::encode_own(&key, fixtures::KEY_LIMIT).expect("own key fits"),
            value,
        });
        assert_refusal(&open(&mut store), OpenRefusal::OlderBuild { why: Older::Layout(Family::Component) });
    }
}

fn replacing(replacement: skein_kv::Op) -> Store {
    let capacity = u32::try_from(OPENED_ROWS).expect("fixture row count").checked_add(1).expect("one replacement");
    let mut ops = skein_lib::List::with_capacity(capacity);
    for row in opened_rows() {
        ops.push(row).expect("fixture row fits");
    }
    ops.push(replacement).expect("replacement fits");
    planted(ops.as_slice())
}

fn rows(store: &mut Store) -> Box<[skein_kv::Op]> {
    let mut rows = skein_lib::List::with_capacity(u32::try_from(OPENED_ROWS).expect("fixture row count"));
    let mut start = Box::from([]);
    for _ in 0..=OPENED_ROWS {
        let page = crate::pages::load(store, skein_kv::Range { start, end: None }).expect("memory page");
        if page.rows.is_empty() {
            return rows.into_boxed();
        }
        start = crate::pages::past(&page.rows.last().expect("nonempty page").key).expect("past key");
        for row in page.rows {
            rows.push(skein_kv::Op::Put { key: row.key, value: row.value }).expect("fixture row fits");
        }
    }
    panic!("fixture rows fit");
}

#[test]
fn older_build_refusals_leave_every_row_and_commit_unchanged() {
    let mut changed = crate::format::layouts();
    changed.core[0] ^= 1;
    let key = keys::encode_own(&keys::Own::Layouts, fixtures::KEY_LIMIT).expect("own key fits");
    for replacement in [
        skein_kv::Op::Erase { key: key.clone() },
        skein_kv::Op::Put { key, value: Box::from([]) },
        layouts_put(&changed),
        skein_kv::Op::Put {
            key: keys::encode_own(&keys::Own::Format, fixtures::KEY_LIMIT).expect("own key fits"),
            value: Box::from([0, 3]),
        },
    ] {
        let mut store = replacing(replacement);
        let before = rows(&mut store);
        let last = store.stores.store.last();
        let used = store.stores.store.used();
        let events = open(&mut store);
        let [Event::OpenRefused { why: OpenRefusal::OlderBuild { .. }, .. }] = &*events else {
            panic!("older build refusal: {events:?}");
        };
        assert_eq!(rows(&mut store), before);
        assert_eq!(store.stores.store.last(), last);
        assert_eq!(store.stores.store.used(), used);
        assert!(store.stores.secrets.is_empty());
    }
}

#[test]
fn a_build_before_held_turns_and_closed_windows_is_refused() {
    // core-v6 at ae4ec55e0, before the two in-place words of store.md 8.6.
    const BEFORE: [u8; 32] = [
        0xd3, 0xe9, 0x3c, 0xe9, 0x13, 0xdd, 0x3c, 0x94, 0x90, 0x47, 0x43, 0xea, 0xe3, 0xf0, 0x87, 0x70, 0x07, 0x52,
        0xbe, 0x24, 0x99, 0xc3, 0xff, 0xfb, 0x3b, 0x82, 0x65, 0x31, 0x2e, 0xe5, 0xd5, 0x40,
    ];
    let mut layouts = crate::layouts::LAYOUTS;
    layouts.core = BEFORE;
    let key = keys::encode_own(&keys::Own::Layouts, fixtures::KEY_LIMIT).expect("layouts key");
    let mut store = replacing(skein_kv::Op::Put { key, value: layouts.encode() });
    assert_refusal(&open(&mut store), OpenRefusal::OlderBuild { why: Older::Layout(Family::Core) });
}

#[test]
fn seven_field_layouts_refuse_before_a_secret_read() {
    // The former row was version u16 followed by seven fixed 32-byte digests.
    const BEFORE_BYTES: usize = 226;
    let current = crate::format::layouts();
    let mut bytes = skein_lib::Writer::new(BEFORE_BYTES);
    bytes.put(&3_u16.to_be_bytes()).expect("component version");
    for digest in [
        current.component,
        current.core,
        current.tasks,
        current.people,
        current.notes,
        current.accounts,
        current.settings,
    ] {
        bytes.put(&digest).expect("former digest");
    }
    let mut store = replacing(skein_kv::Op::Put {
        key: keys::encode_own(&keys::Own::Layouts, fixtures::KEY_LIMIT).expect("layouts key"),
        value: bytes.finish(),
    });
    // A secret request would fail key admission if it reached the reader.
    store.limits.secrets.key = 0;
    assert_refusal(&open(&mut store), OpenRefusal::OlderBuild { why: Older::Layout(Family::Component) });
    assert!(
        fixtures::run(
            &mut store,
            Request::Secrets {
                owner: Token::new(1),
                request: crate::Secrets::Get { owner: crate::SecretOwner::SignIn, number: 1 }
            }
        )
        .is_empty()
    );
}
