//! Commits answered only after the same skein-kv number (protocol/store.md, 6).
use super::fixtures;
use crate::{Event, Phase, Request, Store, Write, codec, keys};
use alloc::boxed::Box;
use jig_domain_core::{CoreKey, CoreRecord, Key, Record};
use skein_lib::Token;

fn assert_committed(events: &[Event], number: u64) -> u64 {
    let [Event::Committed { number: found, fill }] = events else {
        panic!("one committed terminal: {events:?}");
    };
    assert_eq!(*found, number);
    *fill
}

fn assert_failed(events: &[Event], number: u64) {
    let [Event::Failed { number: found }] = events else {
        panic!("one failed terminal: {events:?}");
    };
    assert_eq!(*found, number);
}

fn commit(store: &mut Store, number: u64, writes: Box<[Write]>) -> Box<[Event]> {
    fixtures::run(store, fixtures::commit(number, writes))
}

fn jig_row(store: &mut Store, key: &Key) -> Option<Record> {
    let encoded = keys::encode(key, fixtures::KEY_LIMIT).expect("fixture key fits");
    let bytes = fixtures::get_kv(store, &encoded)?;
    codec::decode(key, &bytes, &store.codec, &store.names)
}

fn format_row(store: &mut Store) -> codec::component::Format {
    let bytes = fixtures::get_kv(store, &[0, 0]).expect("format row exists");
    codec::component::Format::decode(&bytes).expect("format decodes")
}

fn note() -> Record {
    Record::Notes(jig_domain_notes::Record::Entry(fixtures::note_entry()))
}

fn other_header(commits: u64) -> Write {
    let mut header = fixtures::core_rows::deployment();
    header.id = fixtures::OTHER_DEPLOYMENT;
    header.commits = commits;
    fixtures::save(Record::Core(CoreRecord::Deployment(header)))
}

fn application_put() -> Write {
    Write::Put { key: Box::new([41]), value: Box::new([42, 43]) }
}

fn application_row(store: &mut Store) -> Option<Box<[u8]>> {
    fixtures::get_kv(store, &[2, 41])
}

fn save_note(store: &mut Store) -> Box<[Event]> {
    commit(store, 1, Box::new([fixtures::header(1), fixtures::save(note())]))
}

fn failed_number() -> Store {
    let mut store = fixtures::opened();
    assert_failed(&commit(&mut store, 2, Box::new([fixtures::header(2)])), 2);
    store
}

fn committed_header() -> Store {
    let mut store = fixtures::opened();
    assert_committed(&commit(&mut store, 1, Box::new([fixtures::header(1)])), 1);
    store
}

fn committed_note() -> Store {
    let mut store = fixtures::opened();
    assert_committed(&save_note(&mut store), 1);
    store
}

#[test]
fn a_first_commit_puts_format_layouts_and_installation_with_its_header() {
    let mut store = committed_header();
    assert_eq!(
        format_row(&mut store),
        codec::component::Format {
            component: 3,
            core: 6,
            tasks: 9,
            people: 13,
            notes: 1,
            accounts: 2,
            settings: 1,
            application: 1,
            deployment: fixtures::DEPLOYMENT,
            levels: crate::format::recorded(crate::format::NONE),
        },
    );
    let layouts = fixtures::get_kv(&mut store, &[0, 3]).expect("layouts row exists");
    assert_eq!(codec::component::Layouts::decode(&layouts), Ok(store.layouts));
    let installation = fixtures::get_kv(&mut store, &[1, 4, 3]).expect("installation row exists");
    assert_eq!(
        codec::accounts::Installation::decode(&installation),
        Ok(codec::accounts::Installation { id: fixtures::INSTALLATION })
    );
    let Some(Record::Core(CoreRecord::Deployment(header))) = jig_row(&mut store, &Key::Core(CoreKey::Deployment))
    else {
        panic!("header exists under its key");
    };
    assert_eq!(header.id, fixtures::DEPLOYMENT);
    assert_eq!(header.commits, 1);
}

#[test]
fn a_save_puts_the_encoded_record_under_its_key() {
    let mut store = committed_note();
    assert_eq!(jig_row(&mut store, &note().key()), Some(note()));
}

#[test]
fn an_erase_removes_the_key_from_the_store() {
    let mut store = committed_note();
    assert_committed(&commit(&mut store, 2, Box::new([fixtures::header(2), Write::Erase(note().key())])), 2);
    assert_eq!(jig_row(&mut store, &note().key()), None);
}

#[test]
fn application_writes_land_under_the_application_tag() {
    let mut store = fixtures::opened();
    let writes = Box::new([fixtures::header(1), application_put()]);
    assert_committed(&commit(&mut store, 1, writes), 1);
    assert_eq!(application_row(&mut store), Some(Box::from([42, 43])));
    assert_eq!(fixtures::get_kv(&mut store, &[41]), None);
}

#[test]
fn application_removes_erase_the_tagged_key() {
    let mut store = fixtures::opened();
    let writes = Box::new([fixtures::header(1), application_put(), Write::Remove { key: Box::new([41]) }]);
    assert_committed(&commit(&mut store, 1, writes), 1);
    assert_eq!(application_row(&mut store), None);
}

#[test]
fn a_committed_answer_carries_the_stores_used_bytes() {
    let mut store = fixtures::opened();
    let events = save_note(&mut store);
    let fill = assert_committed(&events, 1);
    assert_eq!(fill, store.stores.store.used());
}

#[test]
fn a_commit_numbered_past_the_next_fails() {
    let store = failed_number();
    assert_eq!(store.stores.store.last(), 0);
}

#[test]
fn a_commit_numbered_behind_the_next_fails() {
    let mut store = committed_header();
    assert_failed(&commit(&mut store, 1, Box::new([fixtures::header(1)])), 1);
    assert_eq!(store.stores.store.last(), 1);
}

#[test]
fn a_refused_skein_commit_is_answered_failed() {
    let mut store = fixtures::opened_with_budget(1);
    assert_failed(&commit(&mut store, 1, Box::new([fixtures::header(1)])), 1);
    assert_eq!(store.stores.store.last(), 0);
    assert!(store.stores.store.is_empty());
}

#[test]
fn nothing_is_answered_after_a_failure() {
    let mut store = failed_number();
    assert!(commit(&mut store, 1, Box::new([fixtures::header(1)])).is_empty());
    assert_eq!(store.stores.store.last(), 0);
}

#[test]
fn consecutive_commits_keep_the_skein_numbers_equal() {
    let mut store = committed_header();
    assert_committed(&commit(&mut store, 2, Box::new([fixtures::header(2)])), 2);
    assert_eq!(store.stores.store.last(), 2);
    assert_eq!(store.phase, Phase::Ready { next: 3, format_due: false });
}

#[test]
fn writes_to_the_same_key_keep_their_order() {
    let mut store = fixtures::opened();
    let writes =
        Box::new([fixtures::header(1), Write::Erase(note().key()), fixtures::save(note()), Write::Erase(note().key())]);
    assert_committed(&commit(&mut store, 1, writes), 1);
    assert_eq!(jig_row(&mut store, &note().key()), None);
}

#[test]
fn the_first_format_uses_the_last_saved_header_id() {
    let mut store = fixtures::opened();
    let writes = Box::new([fixtures::header(1), other_header(1)]);
    assert_committed(&commit(&mut store, 1, writes), 1);
    assert_eq!(format_row(&mut store).deployment, fixtures::OTHER_DEPLOYMENT);
}

#[test]
fn later_commits_do_not_replace_the_format_row() {
    let mut store = committed_header();
    assert_committed(&commit(&mut store, 2, Box::new([other_header(2)])), 2);
    assert_eq!(format_row(&mut store).deployment, fixtures::DEPLOYMENT);
}

#[test]
#[should_panic(expected = "the first root commit saves its deployment header")]
fn the_first_commit_without_a_header_is_an_owners_bug() {
    let mut store = fixtures::opened();
    commit(&mut store, 1, Box::new([fixtures::save(note())]));
}

#[test]
fn a_committed_store_reopens_at_its_last_number_with_its_kept_installation() {
    let store = committed_header();
    let mut restarted = fixtures::over(store.into_stores());
    let drawn = [6; 16];
    let events = fixtures::run(
        &mut restarted,
        Request::Open { owner: Token::new(1), root: fixtures::ROOT, installation: drawn },
    );
    let [Event::Opened { owner, last, installation, pending: false }] = &*events else {
        panic!("committed store opens: {events:?}");
    };
    assert_eq!(*owner, Token::new(1));
    assert_eq!(*last, 1);
    assert_eq!(*installation, fixtures::INSTALLATION);
}

fn pending_operations(writes: Box<[Write]>) -> Box<[skein_kv::Op]> {
    let mut store = fixtures::opened();
    store.limits.store.commit = 64;
    crate::commits::operations(&mut store, 1, writes, false).expect("final operations fit the commit").ops
}

struct PutCase {
    label: &'static str,
    writes: Box<[Write]>,
    expected: Box<[skein_kv::Op]>,
}

fn put_cases() -> [PutCase; 2] {
    use skein_kv::Op;
    [
        PutCase {
            label: "last_put_keeps_first_key_order",
            writes: Box::new([
                application_put(),
                Write::Put { key: Box::new([42]), value: Box::new([2]) },
                Write::Put { key: Box::new([41]), value: Box::new([3]) },
            ]),
            expected: Box::new([
                Op::Put { key: Box::new([2, 41]), value: Box::new([3]) },
                Op::Put { key: Box::new([2, 42]), value: Box::new([2]) },
            ]),
        },
        PutCase {
            label: "only_final_put_is_priced",
            writes: Box::new([
                Write::Put { key: Box::new([41]), value: Box::new([0; 128]) },
                Write::Put { key: Box::new([41]), value: Box::new([3]) },
            ]),
            expected: Box::new([Op::Put { key: Box::new([2, 41]), value: Box::new([3]) }]),
        },
    ]
}

#[test]
fn a_key_written_twice_in_a_commit_is_one_put_the_last() {
    for case in put_cases() {
        assert_eq!(pending_operations(case.writes), case.expected, "{}", case.label);
    }
}

#[test]
fn a_put_then_an_erase_of_one_key_is_one_erase() {
    let ops = pending_operations(Box::new([application_put(), Write::Remove { key: Box::new([41]) }]));
    assert_eq!(ops.as_ref(), &[skein_kv::Op::Erase { key: Box::new([2, 41]) }]);
}

#[test]
fn archiving_a_settings_revision_copies_its_items_before_erasing_the_held_row() {
    use jig_domain_core::settings as s;
    let mut store = fixtures::opened();
    let mut template = super::settings::template();
    template.number = 7;
    template.llm = None;
    let revision = s::Revision {
        head: s::Head {
            number: 1,
            against: 0,
            at: skein_lib::Wall::EPOCH,
            by: s::Decider::FirstStart,
            proposal: None,
            moved: false,
        },
        items: Box::new([s::RevisionItem {
            setting: s::Setting::Template(7),
            edit: s::Edit::Removed,
            replaced: s::Replaced::Value(Box::new(s::Value::Template(Box::new(template)))),
        }]),
    };
    assert_committed(
        &commit(
            &mut store,
            1,
            Box::new([
                fixtures::header(1),
                fixtures::save(Record::Core(CoreRecord::Settings(s::Record::Revision {
                    history: false,
                    revision: revision.clone(),
                }))),
            ]),
        ),
        1,
    );
    let held = Key::Core(CoreKey::Settings(s::Key::Held(1)));
    assert_committed(
        &commit(
            &mut store,
            2,
            Box::new([fixtures::header(2), Write::CopySettingsRevision(1), Write::Erase(held.clone())]),
        ),
        2,
    );
    assert_eq!(jig_row(&mut store, &held), None);
    assert_eq!(
        jig_row(&mut store, &Key::Core(CoreKey::Settings(s::Key::History(1)))),
        Some(Record::Core(CoreRecord::Settings(s::Record::Revision { history: true, revision })))
    );
}

#[test]
fn later_commits_write_none_of_the_three_creation_rows() {
    let mut store = committed_header();
    let pending =
        crate::commits::operations(&mut store, 2, Box::new([fixtures::header(2)]), false).expect("later commit fits");
    assert_eq!(pending.ops.len(), 1);
    let header_key = keys::encode(&Key::Core(CoreKey::Deployment), fixtures::KEY_LIMIT).expect("header key fits");
    assert_eq!(pending.ops[0].key(), header_key.as_ref());
}

#[test]
fn only_the_first_commit_records_the_binarys_levels_and_layouts() {
    use super::patching::{BASE, SETS, levels};
    for (first_sets, later_sets, expected) in [(BASE, SETS, levels(0, 0)), (SETS, BASE, levels(2, 2))] {
        let mut store = fixtures::opened();
        store.patch_sets(first_sets);
        assert_committed(&commit(&mut store, 1, Box::new([fixtures::header(1)])), 1);
        let first = format_row(&mut store);
        assert_eq!(crate::format::levels(first.levels), expected);
        let key = keys::encode_own(&keys::Own::Layouts, store.limits.store.key).expect("layouts key");
        let layouts = fixtures::get_kv(&mut store, &key).expect("layouts");
        store.patch_sets(later_sets);
        store.layouts.core[0] ^= 1;
        assert_committed(&commit(&mut store, 2, Box::new([fixtures::header(2)])), 2);
        assert_eq!(format_row(&mut store), first);
        assert_eq!(fixtures::get_kv(&mut store, &key), Some(layouts));
    }
}
