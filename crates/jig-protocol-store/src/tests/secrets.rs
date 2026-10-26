//! Secrets' durability, paging and restart sweep (store.md, section 9).
use super::fixtures;
use crate::{CARRIED, Event, Phase, Request, Restore, SecretOwner, Secrets, Store};
use alloc::boxed::Box;
use skein_lib::{Secret, Token, Wall};
const OWNER: Token = Token::new(9);
fn ask(store: &mut Store, request: Secrets) -> Box<[Event]> {
    fixtures::run(store, Request::Secrets { owner: OWNER, request })
}
fn marked() -> Store {
    let mut store = fixtures::opened();
    fixtures::committed(&mut store, Box::new([fixtures::header(1), crate::Write::Wires]));
    store
}
fn kept() -> Store {
    let mut store = marked();
    ask(&mut store, fixtures::sign_in_secret(4, 41));
    store
}
fn get(store: &mut Store, kind: SecretOwner, number: u64) -> Option<Secret> {
    let mut events = ask(store, Secrets::Get { owner: kind, number }).into_iter();
    let Some(Event::Secret { owner, value }) = events.next() else {
        panic!("one secret answer");
    };
    assert_eq!(owner, OWNER);
    assert!(events.next().is_none());
    value
}
fn restore_people(store: &mut Store) {
    let events = fixtures::run(
        store,
        Request::Restore {
            owner: OWNER,
            range: Restore::Jig(Box::new([jig_domain_core::RestoreRange::People])),
            after: None,
            rows: 20,
            bytes: u64::MAX,
        },
    );
    let [Event::Restored { next: None, .. }] = &*events else {
        panic!("people restored");
    };
}
#[test]
fn a_kept_secret_is_answered_once_committed() {
    let mut store = marked();
    let events = ask(&mut store, fixtures::sign_in_secret(4, 41));
    let [Event::Kept { owner: OWNER }] = &*events else {
        panic!("kept once committed");
    };
    assert_eq!(store.stores.secrets.last(), 1);
    assert_eq!(store.stores.store.last(), 1, "only the start mark in the public map");
}
#[test]
fn a_secret_is_got_by_owner_and_number() {
    let mut store = kept();
    assert_eq!(get(&mut store, SecretOwner::SignIn, 4), Some(Secret::new(Box::new([41; 32]))));
    assert_eq!(get(&mut store, SecretOwner::Service, 4), None);
}
#[test]
fn an_erased_secret_gets_none() {
    let mut store = kept();
    let events = ask(&mut store, Secrets::Erase { owner: SecretOwner::SignIn, number: 4 });
    let [Event::Erased { owner: OWNER }] = &*events else {
        panic!("erased terminal");
    };
    assert_eq!(get(&mut store, SecretOwner::SignIn, 4), None);
}
#[test]
fn a_load_pages_an_owners_secrets_in_number_order() {
    let mut store = kept();
    ask(&mut store, fixtures::sign_in_secret(9, 42));
    ask(&mut store, fixtures::sign_in_secret(6, 43));
    let first = ask(&mut store, Secrets::Load { owner: SecretOwner::SignIn, after: None });
    let [Event::SecretsLoaded { rows, next: Some(6), .. }] = &*first else {
        panic!("first page: {first:?}");
    };
    let [first, second] = &**rows else {
        panic!("two secret rows");
    };
    assert_eq!((first.number, second.number), (4, 6));
    let second = ask(&mut store, Secrets::Load { owner: SecretOwner::SignIn, after: Some(6) });
    let [Event::SecretsLoaded { rows, next: None, .. }] = &*second else {
        panic!("last page: {second:?}");
    };
    let [last] = &**rows else {
        panic!("one last row");
    };
    assert_eq!(last.number, 9);
}
#[test]
fn the_sweep_erases_a_digest_whose_sign_in_is_gone() {
    let mut store = kept();
    ask(&mut store, fixtures::sign_in_secret(5, 42));
    let row = jig_domain_people::Stored::SignIn { number: 4, person: 11, expires: Wall::EPOCH };
    fixtures::committed(
        &mut store,
        Box::new([fixtures::header(2), fixtures::save(jig_domain_core::Record::People(row))]),
    );
    restore_people(&mut store);
    assert_eq!(get(&mut store, SecretOwner::SignIn, 4), Some(Secret::new(Box::new([41; 32]))));
    assert_eq!(get(&mut store, SecretOwner::SignIn, 5), None);
}
#[test]
fn the_sweep_keeps_a_digest_whose_service_remains() {
    let mut store = marked();
    let secret = Secret::new(Box::new([42; 32]));
    ask(&mut store, Secrets::Keep { owner: SecretOwner::Service, number: 11, value: secret.clone() });
    fixtures::committed(&mut store, Box::new([fixtures::header(2), fixtures::save(fixtures::person(11))]));
    restore_people(&mut store);
    assert_eq!(get(&mut store, SecretOwner::Service, 11), Some(secret));
}
#[test]
fn a_secrets_request_debug_shows_lengths_not_bytes() {
    let request = fixtures::sign_in_secret(4, 41);
    let debug = alloc::fmt::format(format_args!("{request:?}"));
    assert!(debug.contains("Secret { len: 32 }"));
    assert!(!debug.contains("41"));
}
#[test]
fn a_refused_secrets_commit_stops_without_answering() {
    let mut store = marked();
    let request = Secrets::Keep { owner: SecretOwner::SignIn, number: 1, value: Secret::new(Box::new([])) };
    store.limits.secrets.ops = 0;
    assert!(ask(&mut store, request).is_empty());
    assert_eq!(store.phase, Phase::Stopped);
}
#[test]
fn a_secret_past_its_value_bound_stops_before_submission() {
    let mut store = marked();
    store.limits.secrets.value = 31;
    assert!(ask(&mut store, fixtures::sign_in_secret(4, 41)).is_empty());
    assert_eq!(store.stores.secrets.last(), 0);
    assert_eq!(store.phase, Phase::Stopped);
}
#[test]
fn the_sweep_continues_one_bounded_page_per_fire() {
    let mut store = marked();
    for number in 0_u64..6 {
        ask(&mut store, fixtures::sign_in_secret(number, 41));
    }
    restore_people(&mut store);
    assert_eq!(get(&mut store, SecretOwner::SignIn, 0), None);
    assert!(get(&mut store, SecretOwner::SignIn, 2).is_some());
    for _ in 0_u8..2 {
        let mut above = skein_lib::Queue::with_capacity(crate::MAX_OUT_DOWN);
        let mut below = fixtures::below();
        crate::fire(&mut store, &fixtures::env(), &mut above, &mut below);
        assert!(above.is_empty());
        assert!(below.secrets.is_empty());
    }
    for number in 0_u64..6 {
        assert_eq!(get(&mut store, SecretOwner::SignIn, number), None);
    }
}
fn record(byte: u8) -> Secret {
    Secret::new(Box::new([byte; 48]))
}
fn digest(byte: u8) -> [u8; 32] {
    skein_lib::sha256::digest(record(byte).expose())
}
fn replace(store: &mut Store, value: u8, replaces: Option<[u8; 32]>) -> Box<[Event]> {
    ask(store, Secrets::Replace { owner: SecretOwner::Source, number: 4, value: record(value), replaces })
}
#[test]
fn a_record_replaces_the_one_whose_digest_it_states() {
    let mut store = marked();
    let [Event::Kept { owner: OWNER }] = &*replace(&mut store, 1, None) else {
        panic!("the first record kept");
    };
    let [Event::Kept { owner: OWNER }] = &*replace(&mut store, 2, Some(digest(1))) else {
        panic!("its successor kept");
    };
    assert_eq!(get(&mut store, SecretOwner::Source, 4), Some(record(2)));
    assert_eq!(get(&mut store, SecretOwner::SignIn, 4), None);
}
#[test]
fn a_replace_stating_another_digest_is_stale_and_writes_nothing() {
    let mut store = marked();
    replace(&mut store, 1, None);
    for (label, stated) in [("another record", Some(digest(3))), ("no record", None)] {
        let [Event::Stale { owner: OWNER }] = &*replace(&mut store, 2, stated) else {
            panic!("stale: {label}");
        };
        assert_eq!(get(&mut store, SecretOwner::Source, 4), Some(record(1)), "{label}");
    }
    assert_eq!(store.stores.secrets.last(), 1);
    let [Event::Stale { owner: OWNER }] = &*replace(&mut marked(), 2, Some(digest(1))) else {
        panic!("stale: nothing held");
    };
}
#[test]
fn a_replace_past_its_value_bound_stops_before_its_digest_is_read() {
    let mut store = marked();
    store.limits.secrets.value = 47;
    assert!(replace(&mut store, 2, Some(digest(1))).is_empty());
    assert_eq!(store.phase, Phase::Stopped);
}
#[test]
fn a_sources_records_page_apart_from_digests_and_outlive_the_sweep() {
    let mut store = kept();
    replace(&mut store, 1, None);
    let events = ask(&mut store, Secrets::Load { owner: SecretOwner::Source, after: None });
    let [Event::SecretsLoaded { rows, next: None, .. }] = &*events else {
        panic!("one page: {events:?}");
    };
    let [row] = &**rows else {
        panic!("the source's record alone");
    };
    assert_eq!((row.owner, row.number, &row.value), (SecretOwner::Source, 4, &record(1)));
    restore_people(&mut store);
    assert_eq!(get(&mut store, SecretOwner::SignIn, 4), None);
    assert_eq!(get(&mut store, SecretOwner::Source, 4), Some(record(1)));
}
fn carry(store: &mut Store, value: u8) -> Box<[Event]> {
    ask(store, Secrets::Carry { value: record(value) })
}
#[test]
fn a_carried_value_is_numbered_by_jig_and_kept_with_its_counter_in_one_commit() {
    let mut store = marked();
    for (value, number) in [(1, CARRIED), (2, CARRIED + 1)] {
        let events = carry(&mut store, value);
        let [Event::Carried { owner: OWNER, number: carried }] = &*events else {
            panic!("carried once committed: {events:?}");
        };
        assert_eq!(*carried, number);
        assert_eq!(get(&mut store, SecretOwner::Application, number), Some(record(value)));
    }
    assert_eq!(store.stores.secrets.last(), 2, "one commit each, the counter in it");
    let events = ask(&mut store, Secrets::Load { owner: SecretOwner::Application, after: None });
    let [Event::SecretsLoaded { rows, next: None, .. }] = &*events else {
        panic!("one page: {events:?}");
    };
    let [first, second] = &**rows else {
        panic!("the two values alone, the counter row no value: {rows:?}");
    };
    assert_eq!((first.number, second.number), (CARRIED, CARRIED + 1));
}
#[test]
fn a_carry_whose_next_number_holds_a_value_is_stale_and_writes_nothing() {
    let mut store = marked();
    ask(&mut store, Secrets::Keep { owner: SecretOwner::Application, number: CARRIED, value: record(7) });
    let [Event::Stale { owner: OWNER }] = &*carry(&mut store, 1) else {
        panic!("never overwritten");
    };
    assert_eq!(store.stores.secrets.last(), 1, "nothing written");
    assert_eq!(get(&mut store, SecretOwner::Application, CARRIED), Some(record(7)));
}
#[test]
fn a_carry_past_its_value_bound_stops_before_its_number_is_drawn() {
    let mut store = marked();
    store.limits.secrets.value = 47;
    assert!(carry(&mut store, 1).is_empty());
    assert_eq!(store.phase, Phase::Stopped);
}

fn carry_llm_key(store: &mut Store) -> u64 {
    let events = ask(store, Secrets::Carry { value: Secret::new(Box::new([73; 32])) });
    let [Event::Carried { number, .. }] = &*events else { panic!("carried LLM key") };
    *number
}
fn adopt_llm_key(store: &mut Store, carried: u64, replaces: Option<[u8; 32]>) -> Box<[Event]> {
    ask(store, Secrets::Adopt { carried, owner: SecretOwner::Source, number: 7, replaces })
}
#[test]
fn adopt_moves_a_carried_value_in_one_commit() {
    let mut store = marked();
    let carried = carry_llm_key(&mut store);
    let before = store.stores.secrets.last();
    let [Event::Kept { owner: OWNER }] = &*adopt_llm_key(&mut store, carried, None) else {
        panic!("durable LLM key adoption")
    };
    assert_eq!(store.stores.secrets.last(), before + 1);
    assert_eq!(get(&mut store, SecretOwner::Application, carried), None);
    assert_eq!(get(&mut store, SecretOwner::Source, 7), Some(Secret::new(Box::new([73; 32]))));
}
#[test]
fn adopt_is_stale_for_another_digest() {
    let mut store = marked();
    ask(&mut store, Secrets::Keep { owner: SecretOwner::Source, number: 7, value: Secret::new(Box::new([19; 32])) });
    let carried = carry_llm_key(&mut store);
    let before = store.stores.secrets.last();
    let [Event::Stale { .. }] = &*adopt_llm_key(&mut store, carried, Some([0; 32])) else {
        panic!("stale LLM key adoption")
    };
    assert_eq!(store.stores.secrets.last(), before);
    assert_eq!(get(&mut store, SecretOwner::Application, carried), Some(Secret::new(Box::new([73; 32]))));
    assert_eq!(get(&mut store, SecretOwner::Source, 7), Some(Secret::new(Box::new([19; 32]))));
    let digest = skein_lib::sha256::digest(&[19; 32]);
    let [Event::Kept { .. }] = &*adopt_llm_key(&mut store, carried, Some(digest)) else {
        panic!("matching digest permits LLM key adoption")
    };
}
#[test]
fn adopt_is_stale_for_a_missing_carried_row() {
    let mut store = marked();
    let [Event::Stale { .. }] = &*adopt_llm_key(&mut store, CARRIED, None) else {
        panic!("missing carried LLM key is stale")
    };
    assert_eq!(store.stores.secrets.last(), 0);
    assert_eq!(get(&mut store, SecretOwner::Source, 7), None);
}
#[test]
fn an_erase_after_an_adopt_is_a_no_op() {
    let mut store = marked();
    let carried = carry_llm_key(&mut store);
    adopt_llm_key(&mut store, carried, None);
    let [Event::Erased { .. }] = &*ask(&mut store, Secrets::Erase { owner: SecretOwner::Application, number: carried })
    else {
        panic!("absent carried row is erased idempotently")
    };
    assert_eq!(get(&mut store, SecretOwner::Source, 7), Some(Secret::new(Box::new([73; 32]))));
    restore_people(&mut store);
    assert_eq!(get(&mut store, SecretOwner::Source, 7), Some(Secret::new(Box::new([73; 32]))));
}
#[test]
fn the_restart_spares_kept_and_orphaned_carried_values() {
    let mut store = marked();
    let carried = carry_llm_key(&mut store);
    restore_people(&mut store);
    assert_eq!(get(&mut store, SecretOwner::Application, carried), Some(Secret::new(Box::new([73; 32]))));
}

fn adopt_decoded(store: &mut Store, carried: u64, replaces: Option<[u8; 32]>) -> Box<[Event]> {
    ask(
        store,
        Secrets::AdoptValue {
            carried,
            holds: skein_lib::sha256::digest(&[73; 32]),
            owner: SecretOwner::Source,
            number: 7,
            value: record(81),
            replaces,
        },
    )
}
#[test]
fn adopt_value_keeps_the_decoded_record_and_erases_the_file_in_one_commit() {
    let mut store = marked();
    let carried = carry_llm_key(&mut store);
    let before = store.stores.secrets.last();
    let [Event::Kept { owner: OWNER }] = &*adopt_decoded(&mut store, carried, None) else {
        panic!("durable decoded record adoption")
    };
    assert_eq!(store.stores.secrets.last(), before + 1);
    assert_eq!(get(&mut store, SecretOwner::Application, carried), None);
    assert_eq!(get(&mut store, SecretOwner::Source, 7), Some(record(81)));
}
#[test]
fn adopt_value_is_stale_when_the_carried_file_changed() {
    let mut store = marked();
    let carried = carry_llm_key(&mut store);
    ask(&mut store, Secrets::Keep { owner: SecretOwner::Application, number: carried, value: record(19) });
    let before = store.stores.secrets.last();
    let [Event::Stale { .. }] = &*adopt_decoded(&mut store, carried, None) else {
        panic!("changed carried file is stale")
    };
    assert_eq!(store.stores.secrets.last(), before);
    assert_eq!(get(&mut store, SecretOwner::Application, carried), Some(record(19)));
    assert_eq!(get(&mut store, SecretOwner::Source, 7), None);
}
#[test]
fn adopt_value_is_stale_when_the_destination_is_held() {
    let mut store = marked();
    let carried = carry_llm_key(&mut store);
    ask(&mut store, Secrets::Keep { owner: SecretOwner::Source, number: 7, value: record(19) });
    let before = store.stores.secrets.last();
    let [Event::Stale { .. }] = &*adopt_decoded(&mut store, carried, None) else { panic!("held destination is stale") };
    assert_eq!(store.stores.secrets.last(), before);
    assert_eq!(get(&mut store, SecretOwner::Application, carried), Some(Secret::new(Box::new([73; 32]))));
    assert_eq!(get(&mut store, SecretOwner::Source, 7), Some(record(19)));
    let [Event::Kept { .. }] =
        &*adopt_decoded(&mut store, carried, Some(skein_lib::sha256::digest(record(19).expose())))
    else {
        panic!("matching destination digest permits adoption")
    };
}

const ADDRESS: &[u8] = b"http://127.0.0.1:1455/auth/callback?code=c&state=s";
fn hold(store: &mut Store) -> u64 {
    let events = ask(store, Secrets::Hold { value: Secret::new(Box::from(ADDRESS)) });
    let [Event::Carried { owner: OWNER, number }] = &*events else { panic!("held at once") };
    *number
}
#[test]
fn a_held_address_is_read_and_erased_without_any_kv_write() {
    let mut store = marked();
    let number = hold(&mut store);
    assert_eq!(number, crate::HELD);
    assert_eq!(get(&mut store, SecretOwner::Application, number), Some(Secret::new(Box::from(ADDRESS))));
    let events = ask(&mut store, Secrets::Erase { owner: SecretOwner::Application, number });
    let [Event::Erased { owner: OWNER }] = &*events else { panic!("erased at once") };
    assert_eq!(get(&mut store, SecretOwner::Application, number), None);
    assert_eq!(store.stores.secrets.last(), 0);
    assert!(store.stores.secrets.is_empty());
    assert_eq!(store.stores.store.last(), 1, "only the start mark in the public map");
}
#[test]
fn held_values_at_capacity_refuse_one_more_without_writing() {
    // The shared store fixture holds two values at once.
    const HELD_CAPACITY: u64 = 2;
    let mut store = marked();
    for offset in 0..HELD_CAPACITY {
        assert_eq!(hold(&mut store), crate::HELD + offset);
    }
    let events = ask(&mut store, Secrets::Hold { value: Secret::new(Box::from(ADDRESS)) });
    let [Event::Stale { owner: OWNER }] = &*events else { panic!("held bound") };
    assert_eq!(store.stores.secrets.last(), 0);
    assert!(store.stores.secrets.is_empty());
}
#[test]
fn reopening_the_store_loses_held_addresses() {
    let mut store = marked();
    let number = hold(&mut store);
    let limits = fixtures::store_limits();
    let mut store = Store::over(&limits, fixtures::APPLICATION, &[], store.into_stores());
    fixtures::run(
        &mut store,
        Request::Open { owner: OWNER, root: fixtures::ROOT, installation: fixtures::INSTALLATION },
    );
    fixtures::committed(&mut store, Box::new([fixtures::header(2), crate::Write::Wires]));
    assert_eq!(get(&mut store, SecretOwner::Application, number), None);
    assert!(store.stores.secrets.is_empty());
}
#[test]
fn erased_held_numbers_are_not_reused_during_the_run() {
    let mut store = marked();
    let first = hold(&mut store);
    ask(&mut store, Secrets::Erase { owner: SecretOwner::Application, number: first });
    assert_eq!(hold(&mut store), crate::HELD + 1);
}

#[test]
fn a_held_address_does_not_appear_in_debug() {
    let mut store = marked();
    hold(&mut store);
    let debug = alloc::fmt::format(format_args!("{store:?}"));
    assert!(debug.contains("Secret { len:"));
    assert!(!debug.contains("code=c"));
    assert!(!debug.contains("state=s"));
}
