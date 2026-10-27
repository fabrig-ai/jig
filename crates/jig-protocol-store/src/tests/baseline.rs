//! Admission and ordinary opening of the kept owner sets (store.md 10.3).
use super::fixtures;
use crate::patching::plan;
use crate::{Event, KeptFamily, KeptMap, Owner, PatchSet, PatchSets, Request, Store, codec, keys};
use alloc::boxed::Box;
use skein_lib::Token;

pub(super) const SETS: PatchSets = PatchSets {
    sets: &[
        PatchSet {
            owner: Owner::Skein,
            name: skein_oauth::OWNER,
            patches: skein_oauth::patches::PATCHES,
            fresh: None,
            families: &[KeptFamily {
                name: skein_oauth::FAMILY,
                texts: skein_oauth::patches::kept::skein_oauth::TEXTS,
                map: KeptMap::Secrets,
                keys: &[],
                frames: None,
                flights: &[],
            }],
        },
        PatchSet {
            owner: Owner::Smith,
            name: smith_transcript::OWNER,
            patches: smith_transcript::patches::PATCHES,
            fresh: Some(smith_transcript::FRESH_WINDOWS),
            families: &[KeptFamily {
                name: smith_transcript::FAMILY,
                texts: smith_transcript::patches::kept::smith_transcript::TEXTS,
                map: KeptMap::Store,
                keys: &[],
                frames: Some(smith_transcript::ROOT),
                flights: &[],
            }],
        },
        crate::JIG,
        // The fixture supplies the fourth owner's baseline.
        super::patching::BASE.sets[0],
    ],
};
const ZERO: codec::component::Levels =
    codec::component::Levels { skein: Some(0), smith: Some(0), jig: Some(0), application: Some(0) };

#[test]
fn real_owner_sets_pass_each_admission_check() {
    // bounds also checks held-family acyclicity.
    assert_eq!(plan::bounds(&SETS), Ok(()));
    assert_eq!(plan::ranges(&SETS), Ok(()));
    assert_eq!(plan::frames(&SETS), Ok(()));
}

fn open(store: &mut Store) -> Box<[Event]> {
    fixtures::run(
        store,
        Request::Open { owner: Token::new(1), root: fixtures::ROOT, installation: fixtures::INSTALLATION },
    )
}

fn recorded(store: &mut Store) -> codec::component::Levels {
    let key = keys::encode_own(&keys::Own::Format, fixtures::KEY_LIMIT).expect("format key");
    let row = fixtures::get_kv(store, &key).expect("first commit records format");
    codec::component::Format::decode(&row).expect("current format").levels
}

#[test]
fn real_owner_sets_start_and_reopen_at_level_zero() {
    let mut store = fixtures::over(fixtures::stores());
    store.patch_sets(SETS);
    let opened = open(&mut store);
    let [Event::Opened { last: 0, .. }] = &*opened else { panic!("first open: {opened:?}") };
    let committed = fixtures::run(&mut store, fixtures::commit(1, Box::new([fixtures::header(1)])));
    let [Event::Committed { number: 1, .. }] = &*committed else { panic!("first commit: {committed:?}") };
    assert_eq!(recorded(&mut store), ZERO);
    let mut reopened = fixtures::over(store.into_stores());
    reopened.patch_sets(SETS);
    let opened = open(&mut reopened);
    let [Event::Opened { last: 1, .. }] = &*opened else { panic!("equal-level open: {opened:?}") };
    assert_eq!(recorded(&mut reopened), ZERO);
}
