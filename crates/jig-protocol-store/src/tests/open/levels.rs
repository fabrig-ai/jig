//! Levels decide before the version and layout checks, with no file operation.
use super::{assert_opened, assert_refusal, fixture_format, replacing, rows};
use crate::patching::plan::{Levels, Planned};
use crate::tests::{
    fixtures,
    patching::{BASE, ROOM, SETS, levels},
};
use crate::{Event, Family, KeptFamily, Older, OpenRefusal, Owner, PatchSet, PatchSets, Store, format};
use skein_lib::{Queue, Token};

const FOUR: PatchSets = PatchSets {
    sets: &[
        PatchSet {
            owner: Owner::Skein,
            families: &[KeptFamily { keys: &[], ..BASE.sets[1].families[0] }],
            ..BASE.sets[1]
        },
        PatchSet {
            owner: Owner::Smith,
            families: &[KeptFamily { keys: &[], ..BASE.sets[1].families[0] }],
            ..BASE.sets[1]
        },
        BASE.sets[1],
        BASE.sets[0],
    ],
};
const ZERO: Levels = Levels { skein: Some(0), smith: Some(0), jig: Some(0), application: Some(0) };

fn store(found: Levels, sets: PatchSets) -> Store {
    let (key, mut row) = fixture_format();
    row.levels = format::recorded(found);
    let mut store = replacing(skein_kv::Op::Put { key, value: row.encode().expect("format fits") });
    store.patch_sets(sets);
    store.limits.store.value = ROOM.bytes;
    store
}
fn open(store: &mut Store) -> Box<[Event]> {
    let mut above = Queue::with_capacity(crate::MAX_OUT_DOWN);
    let mut below = fixtures::below();
    crate::open::open(store, Token::new(1), &mut above, &mut below);
    assert!(below.store.is_empty() && below.secrets.is_empty() && below.transcripts.is_empty(), "no file operation");
    fixtures::events(&mut above)
}
fn refused(store: &mut Store, why: OpenRefusal) {
    let before = rows(store);
    let last = store.stores.store.last();
    let used = store.stores.store.used();
    assert_refusal(&open(store), why);
    assert_eq!(rows(store), before);
    assert_eq!(store.stores.store.last(), last);
    assert_eq!(store.stores.store.used(), used);
    assert_eq!(store.stores.secrets.last(), 0);
}

#[test]
fn each_owner_past_the_binary_refuses_before_any_file_operation() {
    for owner in [Owner::Skein, Owner::Smith, Owner::Jig, Owner::Application] {
        let mut found = ZERO;
        found.set(owner, Some(1));
        refused(&mut store(found, FOUR), OpenRefusal::Ahead { owner, found: Some(1), wanted: Some(0) });
        let mut found = format::NONE;
        found.set(owner, Some(0));
        refused(&mut store(found, PatchSets::NONE), OpenRefusal::Ahead { owner, found: Some(0), wanted: None });
    }
}
#[test]
fn ahead_wins_over_unleveled_and_behind_in_upstream_order() {
    for (found, sets, owner, wanted) in [
        (Levels { jig: Some(1), ..format::NONE }, FOUR, Owner::Jig, Some(0)),
        (Levels { skein: Some(1), smith: Some(1), ..ZERO }, FOUR, Owner::Skein, Some(0)),
        (Levels { jig: Some(0), application: Some(3), ..format::NONE }, SETS, Owner::Application, Some(2)),
    ] {
        refused(&mut store(found, sets), OpenRefusal::Ahead { owner, found: found.of(owner), wanted });
    }
}
#[test]
fn any_unleveled_owner_refuses_even_when_jig_is_leveled() {
    for owner in [Owner::Skein, Owner::Smith, Owner::Jig, Owner::Application] {
        let mut found = ZERO;
        found.set(owner, None);
        refused(&mut store(found, FOUR), OpenRefusal::OlderBuild { why: Older::Unleveled });
    }
    refused(&mut store(format::NONE, BASE), OpenRefusal::OlderBuild { why: Older::Unleveled });
    for found in [Levels { jig: Some(0), ..format::NONE }, Levels { application: Some(0), ..format::NONE }] {
        refused(&mut store(found, SETS), OpenRefusal::OlderBuild { why: Older::Unleveled });
    }
}
#[test]
fn levels_refuse_before_versions_and_layouts_are_considered() {
    let (key, mut row) = fixture_format();
    row.levels = format::recorded(Levels { jig: Some(3), ..format::NONE });
    row.tasks = 1;
    let mut store = replacing(skein_kv::Op::Put { key, value: row.encode().expect("format") });
    store.patch_sets(SETS);
    store.limits.store.value = ROOM.bytes;
    refused(&mut store, OpenRefusal::Ahead { owner: Owner::Jig, found: Some(3), wanted: Some(2) });
}
#[test]
fn a_later_component_version_refuses_ahead_without_reading_its_levels() {
    for sets in [PatchSets::NONE, SETS] {
        let (key, _) = fixture_format();
        // Only the version prefix is readable; none of the later row is needed.
        let mut store = replacing(skein_kv::Op::Put { key, value: Box::new([0, 4]) });
        store.patch_sets(sets);
        store.limits.store.value = ROOM.bytes;
        refused(
            &mut store,
            OpenRefusal::Ahead { owner: Owner::Jig, found: None, wanted: crate::patching::plan::binary(&sets).jig },
        );
    }
}
#[test]
fn a_row_from_ls_build_refuses_as_the_older_component_layout() {
    // L's complete version-3 row, before Levels was added in place: the
    // family's prefix, its seven versions, application 1 and deployment 7.
    const L_ROW: &[u8] =
        &[0, 3, 0, 3, 0, 6, 0, 9, 0, 13, 0, 1, 0, 2, 0, 1, 0, 0, 0, 1, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7];
    let (key, _) = fixture_format();
    refused(
        &mut replacing(skein_kv::Op::Put { key, value: Box::from(L_ROW) }),
        OpenRefusal::OlderBuild { why: Older::Layout(Family::Component) },
    );
}
#[test]
fn equal_levels_still_check_every_version_and_layout() {
    for found in [format::NONE, levels(0, 0)] {
        let sets = if found == format::NONE { PatchSets::NONE } else { BASE };
        assert_opened(&open(&mut store(found, sets)), 1);
        let (key, mut row) = fixture_format();
        row.levels = format::recorded(found);
        row.application = 2;
        let mut changed = replacing(skein_kv::Op::Put { key, value: row.encode().expect("format") });
        changed.patch_sets(sets);
        refused(&mut changed, OpenRefusal::Format { family: Family::Application, found: 2, wanted: 1 });
        let mut changed = store(found, sets);
        changed.layouts.core[0] ^= 1;
        refused(&mut changed, OpenRefusal::OlderBuild { why: Older::Layout(Family::Core) });
    }
}
#[test]
fn behind_plans_before_ready_and_ignores_only_the_application_version() {
    let (key, mut row) = fixture_format();
    row.levels = format::recorded(levels(0, 0));
    row.application = 0;
    let mut store = replacing(skein_kv::Op::Put { key, value: row.encode().expect("format") });
    store.patch_sets(SETS);
    store.limits.store.value = ROOM.bytes;
    assert_opened(&open(&mut store), 1);
    let pending = store.directory.patch.as_ref().expect("open made the plan");
    assert_eq!(pending.found, levels(0, 0));
    assert_eq!(
        pending.plan.steps(),
        &[
            Planned { owner: Owner::Application, patch: 1 },
            Planned { owner: Owner::Jig, patch: 1 },
            Planned { owner: Owner::Application, patch: 2 },
            Planned { owner: Owner::Jig, patch: 2 },
        ]
    );
    assert_eq!(pending.machines.len(), 4);
}
#[test]
fn behind_keeps_jigs_versions_layouts_deployment_and_numbers_checks() {
    let (key, mut row) = fixture_format();
    row.levels = format::recorded(levels(0, 0));
    row.core = 5;
    let mut changed = replacing(skein_kv::Op::Put { key, value: row.encode().expect("format") });
    changed.patch_sets(SETS);
    refused(&mut changed, OpenRefusal::Format { family: Family::Core, found: 5, wanted: 6 });
    let mut changed = store(levels(0, 0), SETS);
    changed.layouts.core[0] ^= 1;
    refused(&mut changed, OpenRefusal::OlderBuild { why: Older::Layout(Family::Core) });
    for (op, why) in [
        (fixtures::header_put(fixtures::OTHER_DEPLOYMENT, 2), OpenRefusal::Deployment),
        (fixtures::header_put(fixtures::DEPLOYMENT, 0), OpenRefusal::Numbers { store: 2, header: 0 }),
    ] {
        let mut changed = store(levels(0, 0), SETS);
        fixtures::plant(&mut changed.stores, &[op]);
        refused(&mut changed, why);
    }
}
#[test]
fn unplannable_headers_and_unchained_blocks_refuse_set_before_file_operations() {
    const BAD_HEADER: PatchSets =
        PatchSets { sets: &[PatchSet { patches: &["patch 1\nowner other 1\n"], ..SETS.sets[0] }, SETS.sets[1]] };
    const UNCHAINED: PatchSets = PatchSets {
        sets: &[
            PatchSet {
                patches: &[
                    "patch 1\nowner fixture 1\nfamily fixture-label 1 to 2\nadd Label.color option u8 none\n",
                    "patch 1\nowner fixture 2\nfamily fixture-label 1 to 2\nadd Label.color option u8 none\n",
                ],
                ..SETS.sets[0]
            },
            SETS.sets[1],
        ],
    };
    for (sets, patch) in [(BAD_HEADER, 1), (UNCHAINED, 2)] {
        refused(&mut store(levels(0, 0), sets), OpenRefusal::Set { owner: Owner::Application, patch });
    }
}
#[test]
fn a_family_past_the_manifests_bounds_refuses_set_before_any_write() {
    // The manifest counts each kept family by name: at most 64, each at most 64 bytes.
    const BOUND: &str = "fixture-label-at-the-manifests-sixty-four-byte-family-name-bound";
    const PAST: &str = "fixture-label-at-the-manifests-sixty-four-byte-family-name-bounds";
    const AT_BOUND: PatchSets = PatchSets {
        sets: &[
            PatchSet { families: &[KeptFamily { name: BOUND, ..SETS.sets[0].families[0] }], ..SETS.sets[0] },
            SETS.sets[1],
        ],
    };
    const LONG: PatchSets = PatchSets {
        sets: &[
            PatchSet { families: &[KeptFamily { name: PAST, ..SETS.sets[0].families[0] }], ..SETS.sets[0] },
            SETS.sets[1],
        ],
    };
    // Sixty-four of the application's, so jig's one family is the 65th.
    const MANY: PatchSets =
        PatchSets { sets: &[PatchSet { families: &[SETS.sets[0].families[0]; 64], ..SETS.sets[0] }, SETS.sets[1]] };
    assert_eq!((BOUND.len(), PAST.len()), (64, 65));
    for (sets, owner) in [(LONG, Owner::Application), (MANY, Owner::Jig)] {
        let why = OpenRefusal::Set { owner, patch: 0 };
        // Behind: the plan refuses before any file operation.
        refused(&mut store(levels(0, 0), sets), why.clone());
        // A first start: nothing is made in the empty deployment.
        let mut disk = fixtures::disk::Disk::new(&[]);
        let before = disk.contents();
        let mut first = disk.store();
        first.patch_sets(sets);
        assert_refusal(&disk.open(&mut first), why);
        assert_eq!(disk.contents(), before);
    }
    let mut disk = fixtures::disk::Disk::new(&[]);
    let mut first = disk.store();
    first.patch_sets(AT_BOUND);
    let events = disk.open(&mut first);
    let [Event::Opened { .. }] = &*events else { panic!("a first start at the bound opens: {events:?}") };
    let manifest = crate::codec::component::Manifest::decode(&disk.bytes(b"generations/1/manifest"));
    assert_eq!(
        manifest.expect("first manifest").counts,
        fixtures::manifest_counts(&[(BOUND, 0, 0), ("fixture-note", 0, 0)])
    );
}

#[test]
fn every_open_checks_ranges_before_comparing_levels() {
    for &(label, sets, owner) in crate::tests::patching::RANGE_FAULTS {
        for found in [crate::patching::plan::binary(&sets), ZERO] {
            let mut store = store(found, sets);
            let events = open(&mut store);
            let [Event::OpenRefused { why, .. }] = events.as_ref() else { panic!("{label}: {events:?}") };
            assert_eq!(why, &OpenRefusal::Set { owner, patch: 0 }, "{label}");
        }
    }
}

#[test]
fn valid_sets_serve_stores_already_at_the_binary_levels() {
    for sets in [BASE, SETS] {
        assert_opened(&open(&mut store(crate::patching::plan::binary(&sets), sets)), 1);
    }
}
