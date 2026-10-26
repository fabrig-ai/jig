//! Adversarial promises at open, through the existing store world only.
use jig_protocol_store::{Generations, Older, OpenRefusal, Owner, PatchSet, PatchSets, Write};
use jig_store_world::{
    World,
    patches::{self, BASE, Records, SETS},
};
use skein_fake_machine::Item;

const SEED: u64 = 0x5141;
const UNPATCHED: u16 = 1;
const PATCHED: u16 = 2;
// The fixture has two application patches, two jig patches (the second patches fixture-frame alone, which the world's
// sets leave out), and two-put pages where a test lowers the journal's writes.
const APPLICATION_LEVEL: u32 = 2;
const JIG_LEVEL: u32 = 2;
const PAGE_ROWS: u64 = 2;
const APP_ONLY: PatchSets = PatchSets { sets: &[SETS.sets[0]] };
const JIG_ONLY: PatchSets = PatchSets { sets: &[BASE.sets[1]] };
const JIG_APPLIED: PatchSets = PatchSets { sets: &[BASE.sets[0], SETS.sets[1]] };
const MID: PatchSets = PatchSets {
    sets: &[
        PatchSet {
            families: &[SETS.sets[0].families[0], BASE.sets[0].families[1]],
            patches: &[jig_test_patches::patches::PATCHES[0]],
            ..SETS.sets[0]
        },
        BASE.sets[1],
    ],
};
const WRONG_OWNER: PatchSets = PatchSets {
    sets: &[
        PatchSet {
            patches: &[
                "patch 1\nowner stranger 1 after fixture-jig 0\n\nfamily fixture-label 1 to 2\nadd Label.color option u8 none\n",
            ],
            ..SETS.sets[0]
        },
        SETS.sets[1],
    ],
};
const PIN_TOO_FAR: PatchSets = PatchSets {
    sets: &[
        PatchSet {
            patches: &[
                "patch 1\nowner fixture 1 after fixture-jig 3\n\nfamily fixture-label 1 to 2\nadd Label.color option u8 none\n",
            ],
            ..SETS.sets[0]
        },
        SETS.sets[1],
    ],
};

fn pending(records: &Records) -> World {
    let mut world = records.committed(SEED, BASE);
    world.patch_sets(SETS);
    world
}

fn second_generation() -> World {
    let mut world = Records::story().committed(SEED, BASE);
    world.patch_sets(MID);
    world.open().expect("intermediate upgrade opens");
    world.crash(SEED);
    world.patch_sets(SETS);
    world
}

fn stray(world: &mut World) {
    world.lay(&[Item::directory(b"generations/9"), Item::file(b"generations/9/sentinel", b"never discard on refusal")]);
}

#[test]
fn pinned_order_reports_exact_walk_counts() {
    // store.md 10.2 step 4 and 10.3: each page is one commit within skein-kv's bounds (5.4), in the plan's pinned order.
    let mut limits = patches::limits();
    limits.journal.writes = u32::try_from(PAGE_ROWS).expect("two-put page");
    let mut records = Records::story();
    records.tokens.clear();
    let mut world = records.committed_with(SEED, BASE, UNPATCHED, limits, &[]);
    world.patch_sets(SETS);
    world.open().expect("upgrade opens");
    let progress: Vec<_> =
        world.patched().iter().map(|(owner, patch, count)| (*owner, *patch, count.records, count.commits)).collect();
    let app_first = u64::from(patches::STORY_LABELS + patches::STORY_SHELVES);
    let jig = u64::from(patches::STORY_NOTES);
    let app_last = u64::from(patches::STORY_SHELVES);
    assert_eq!(
        progress,
        [
            (Owner::Application, 1, app_first, app_first.div_ceil(PAGE_ROWS)),
            (Owner::Jig, 1, jig, jig.div_ceil(PAGE_ROWS)),
            (Owner::Application, 2, app_last, app_last.div_ceil(PAGE_ROWS)),
            // Jig's second patch walks fixture-frame alone, which the world's sets leave out.
            (Owner::Jig, 2, 0, 0),
        ]
    );
}

#[test]
fn written_keys_and_held_values_survive_paging() {
    // store.md 10.3: the shell sets the record key in Context, and held bytes are patched by their own family.
    let mut records = Records::story();
    records.shelves[0].key = b"s/a".to_vec();
    records.shelves[1].key = b"s/a-much-longer-key".to_vec();
    records.shelves[2].key = b"s/z".to_vec();
    records.shelves[1].caption = b"held across a page".to_vec();
    let mut world = pending(&records);
    world.open().expect("upgrade opens");
    records.check(&mut world, PATCHED, &"key context and nested label");
}

#[test]
fn wrong_record_version_refuses_with_its_key() {
    // store.md 10.2 step 4: a record at another version refuses Patch with its key.
    let records = Records::story();
    let key = b"l/1";
    let replacement = records.rows(PATCHED).get(key.as_slice()).expect("modeled label").clone();
    let mut writes = records.writes(UNPATCHED);
    for write in &mut writes {
        if let Write::Put { key: found, value } = write
            && found.as_ref() == key
        {
            *value = replacement.clone();
        }
    }
    let mut world = World::unopened_observed(SEED, patches::limits(), None);
    world.patch_sets(BASE);
    world.open().expect("first start");
    world.commit(writes).expect("source commit");
    world.crash(SEED);
    world.patch_sets(SETS);
    let before = patches::source(&mut world);
    assert_eq!(world.open(), None);
    assert!(
        matches!(world.refusal(), Some(OpenRefusal::Patch { owner: Owner::Application, patch: 1, key: refused, .. }) if refused.as_ref() == key),
        "{:?}",
        world.refusal()
    );
    assert_eq!(world.active(), b"1\n");
    patches::kept(&before, &patches::source(&mut world), &"refused record version");
}

#[test]
fn written_refusal_repeats_and_old_binary_reopens() {
    // store.md 10.2: every start refuses the same record, and the build before opens the source as it was.
    let mut records = Records::story();
    records.shelves[2].books = 0;
    let mut world = pending(&records);
    let original = patches::source(&mut world);
    assert_eq!(world.open(), None);
    let refusal = world.refusal().expect("written refusal").clone();
    assert!(
        matches!(&refusal, OpenRefusal::Patch { owner: Owner::Application, patch: 2, key, .. } if key.as_ref() == b"s/2"),
        "{refusal:?}"
    );
    assert_eq!(world.active(), b"1\n");
    let after_refusal = patches::source(&mut world);
    world.crash(SEED);
    assert_eq!(world.open(), None);
    assert_eq!(world.refusal(), Some(&refusal));
    world.crash(SEED);
    world.patch_sets(BASE);
    assert_eq!(world.open(), Some(1));
    records.check(&mut world, UNPATCHED, &"old binary rollback");
    assert!(!world.paths().iter().any(|path| path.starts_with(b"generations/2")));
    patches::kept(&original, &after_refusal, &"written refusal");
}

#[test]
fn ahead_chooses_upstream_before_downstream() {
    // store.md 5.3: any level past the binary refuses Ahead, the first in Owner order, upstream first.
    let mut world = Records::story().committed_at(SEED, SETS, PATCHED);
    world.patch_sets(BASE);
    stray(&mut world);
    let before = world.contents();
    assert_eq!(world.open(), None);
    assert_eq!(
        world.refusal(),
        Some(&OpenRefusal::Ahead { owner: Owner::Jig, found: Some(JIG_LEVEL), wanted: Some(0) })
    );
    patches::kept(&before, &world.contents(), &"ahead before cleanup");
}

#[test]
fn ahead_precedes_another_owners_missing_baseline() {
    // store.md 5.3: first any owner's level past the binary's, then any owner's missing level.
    let mut world = Records::story().committed_at(SEED, APP_ONLY, PATCHED);
    world.patch_sets(BASE);
    assert_eq!(world.open(), None);
    assert_eq!(
        world.refusal(),
        Some(&OpenRefusal::Ahead { owner: Owner::Application, found: Some(APPLICATION_LEVEL), wanted: Some(0) })
    );
}

#[test]
fn jig_baseline_does_not_hide_unleveled_application() {
    // store.md 5.3: Unleveled includes another owner's none in a row whose jig level is set.
    let mut world = Records::story().committed(SEED, JIG_ONLY);
    world.patch_sets(BASE);
    let before = world.contents();
    assert_eq!(world.open(), None);
    assert_eq!(world.refusal(), Some(&OpenRefusal::OlderBuild { why: Older::Unleveled }));
    patches::kept(&before, &world.contents(), &"unleveled application");
}

#[test]
fn manifest_is_opaque_and_unchanged_at_open() {
    // store.md 2: nothing at open reads the manifest; it is the operator's record.
    let mut world = World::unopened(SEED);
    world.lay(&[
        Item::directory(b"generations"),
        Item::directory(b"generations/1"),
        Item::directory(b"generations/1/store"),
        Item::directory(b"generations/1/secrets").mode(0o700),
        Item::file(b"generations/1/manifest", b"deliberately not a manifest"),
        Item::file(b"active", b"1\n"),
    ]);
    assert_eq!(world.open(), Some(0));
    assert_eq!(
        world.contents().get(b"generations/1/manifest".as_slice()).and_then(Option::as_deref),
        Some(b"deliberately not a manifest".as_slice())
    );
}

#[test]
fn stray_cleanup_restarts_listing_and_keeps_previous() {
    // store.md 10.1 step 5: directories are listed again from their first page until empty; active and previous stay.
    const STRAY_FILES: u32 = 35; // More than the world's 32-entry directory page.
    let mut world = pending(&Records::story());
    world.open().expect("upgrade opens");
    world.crash(SEED);
    let previous = patches::source(&mut world);
    world.lay(&[Item::directory(b"generations/9"), Item::directory(b"generations/9/deep")]);
    for n in 0..STRAY_FILES {
        world.lay(&[Item::file(format!("generations/9/deep/{n:02}").as_bytes(), b"stray")]);
    }
    world.lay(&[
        Item::file(b"generations/dropped", b"stray file"),
        Item::file(b"active.skein-0123456789abcdef", b"2\n"),
    ]);
    world.open().expect("equal-level restart");
    patches::whole(&mut world);
    assert_eq!(patches::source(&mut world), previous);
    assert!(world.patched().is_empty());
}

#[test]
fn active_rejects_noncanonical_and_oversize_numbers() {
    // store.md 10.1 step 2: active is decimal without a leading zero and one newline, at most 21 bytes; anything else refuses.
    let invalid: &[&[u8]] = &[
        b"",
        b"00\n",
        b"01\n",
        b"1",
        b"1\n\n",
        b"+1\n",
        b" 1\n",
        b"1\r\n",
        b"1\0\n",
        b"18446744073709551616\n",
        b"18446744073709551615\nX",
    ];
    for bytes in invalid {
        let mut world = World::unopened(SEED);
        world.lay(&[Item::file(b"active", bytes)]);
        let before = world.contents();
        assert_eq!(world.open(), None, "{bytes:?}");
        assert_eq!(world.refusal(), Some(&OpenRefusal::Generations { why: Generations::Unreadable }), "{bytes:?}");
        assert_eq!(world.contents(), before, "{bytes:?}");
    }
}

#[test]
fn maximum_active_number_reaches_missing_generation_check() {
    // store.md 10.1 steps 2–3: a 21-byte canonical active is accepted before missing maps refuse.
    let mut world = World::unopened(SEED);
    world.lay(&[Item::file(b"active", b"18446744073709551615\n"), Item::directory(b"generations")]);
    let before = world.contents();
    assert_eq!(world.open(), None);
    assert_eq!(world.refusal(), Some(&OpenRefusal::Generations { why: Generations::Missing { generation: u64::MAX } }));
    assert_eq!(world.contents(), before);
}

#[test]
fn top_level_stores_without_active_refuse_without_writes() {
    // store.md 2: store/, secrets/ or transcripts/ at the top without active refuses Directory before any write.
    for directory in [b"store".as_slice(), b"secrets", b"transcripts"] {
        let mut world = World::unopened(SEED);
        world.lay(&[Item::directory(directory)]);
        let before = world.contents();
        assert_eq!(world.open(), None);
        assert_eq!(world.refusal(), Some(&OpenRefusal::OlderBuild { why: Older::Directory }));
        assert_eq!(world.contents(), before, "{directory:?}");
    }
}

#[test]
fn unnamed_generation_with_store_file_is_never_first_start() {
    // store.md 2: generations holding anything beyond an empty interrupted first start refuse Unnamed and stay unchanged.
    let mut world = World::unopened(SEED);
    world.lay(&[
        Item::directory(b"generations"),
        Item::directory(b"generations/1"),
        Item::directory(b"generations/1/store"),
        Item::directory(b"generations/1/secrets"),
        Item::file(b"generations/1/store/data", b"operator must recover me"),
    ]);
    let before = world.contents();
    assert_eq!(world.open(), None);
    assert_eq!(world.refusal(), Some(&OpenRefusal::Generations { why: Generations::Unnamed }));
    assert_eq!(world.contents(), before);
}

#[test]
fn interrupted_first_start_discards_only_empty_generation() {
    // store.md 2: an empty generation 1 with manifest temporaries is removed with active's temporary and started afresh.
    let mut world = World::unopened(SEED);
    world.lay(&[
        Item::directory(b"generations"),
        Item::directory(b"generations/1"),
        Item::directory(b"generations/1/store"),
        Item::directory(b"generations/1/secrets"),
        Item::file(b"generations/1/manifest.skein-0123456789abcdef", b"partial"),
        Item::file(b"active.skein-0123456789abcdef", b"1\n"),
    ]);
    assert_eq!(world.open(), Some(0));
    assert_eq!(world.active(), b"1\n");
    let paths = world.paths();
    assert!(paths.iter().any(|path| path == b"generations/1/manifest"));
    assert!(!paths.iter().any(|path| path.windows(b".skein-".len()).any(|part| part == b".skein-")));
}

#[test]
fn wrong_owner_set_refuses_before_stray_cleanup() {
    // store.md 10.1: nothing is removed or made until the active generation has opened and nothing refuses.
    let mut world = pending(&Records::story());
    world.patch_sets(WRONG_OWNER);
    stray(&mut world);
    let before = world.contents();
    assert_eq!(world.open(), None);
    assert_eq!(world.refusal(), Some(&OpenRefusal::Set { owner: Owner::Application, patch: 1 }));
    patches::kept(&before, &world.contents(), &"set refusal before removal");
}

#[test]
fn pin_past_binary_refuses_before_copy() {
    // store.md 10.3: after naming a level past the binary's refuses the set.
    let mut world = pending(&Records::story());
    world.patch_sets(PIN_TOO_FAR);
    let before = world.contents();
    assert_eq!(world.open(), None);
    assert_eq!(world.refusal(), Some(&OpenRefusal::Set { owner: Owner::Application, patch: 1 }));
    patches::kept(&before, &world.contents(), &"impossible pin");
}

#[test]
fn pin_already_passed_in_store_refuses_before_copy() {
    // store.md 10.3: after naming an upstream level already passed in the store refuses the set.
    let mut records = Records::story();
    records.notes.clear(); // The upstream is at level 1 with an empty family; application rows remain at version 1.
    let mut world = records.committed(SEED, JIG_APPLIED);
    world.patch_sets(SETS);
    let before = world.contents();
    assert_eq!(world.open(), None);
    assert_eq!(world.refusal(), Some(&OpenRefusal::Set { owner: Owner::Application, patch: 1 }));
    patches::kept(&before, &world.contents(), &"already passed pin");
}

#[test]
fn duplicate_owner_sets_refuse_before_copy() {
    // store.md 10.3: PatchSets contains at most one set per owner.
    const DUPLICATE: PatchSets = PatchSets { sets: &[SETS.sets[0], SETS.sets[0], SETS.sets[1]] };
    let mut world = pending(&Records::story());
    world.patch_sets(DUPLICATE);
    let before = world.contents();
    assert_eq!(world.open(), None);
    assert!(
        matches!(world.refusal(), Some(OpenRefusal::Set { owner: Owner::Application, .. })),
        "{:?}",
        world.refusal()
    );
    patches::kept(&before, &world.contents(), &"duplicate owner");
}

#[test]
fn first_start_recovers_after_every_disk_mutation() {
    // store.md 10.4: after each first-start write, sync and rename the next start opens one whole generation.
    let mut trace = World::unopened(SEED);
    trace.trace_writes();
    trace.open().expect("traced first start");
    for &(operation, change) in trace.changes() {
        let mut world = World::unopened(SEED);
        world.trace_writes();
        world.cut_at(operation);
        let _ = world.open();
        world.crash(SEED);
        assert_eq!(world.open(), Some(0), "cut {operation} {change:?}: {:?}", world.refusal());
        assert_eq!(world.active(), b"1\n", "cut {operation} {change:?}");
        let paths = world.paths();
        assert!(paths.iter().any(|path| path == b"generations/1/manifest"));
        assert!(paths.iter().any(|path| path == b"transcripts"));
        assert!(!paths.iter().any(|path| path.windows(b".skein-".len()).any(|part| part == b".skein-")));
    }
}

#[test]
fn drop_copy_and_switch_recover_after_every_mutation() {
    // store.md 10.4: a cut during the previous drop, copy, patch or switch opens the old whole generation or the new.
    let mut trace = second_generation();
    trace.trace_writes();
    let expected_last = trace.open().expect("traced second upgrade");
    for &(operation, change) in trace.changes() {
        let mut world = second_generation();
        let source: std::collections::BTreeMap<_, _> =
            world.contents().into_iter().filter(|(path, _)| path.starts_with(b"generations/2/")).collect();
        world.trace_writes();
        world.cut_at(operation);
        let _ = world.open();
        world.crash(SEED);
        assert_eq!(world.open(), Some(expected_last), "cut {operation} {change:?}: {:?}", world.refusal());
        assert_eq!(world.active(), b"3\n", "cut {operation} {change:?}");
        Records::story().check(&mut world, PATCHED, &(operation, change));
        let after = world.contents();
        for (path, bytes) in &source {
            assert_eq!(after.get(path), Some(bytes), "source changed at {operation}: {path:?}");
        }
        assert!(
            !after.keys().any(|path| path.starts_with(b"generations/1/") || path.starts_with(b"generations/dropped"))
        );
    }
}
