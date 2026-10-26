//! Design-only QA of store.md 2, 9, 10.1–10.4 and 16.
use jig_domain_core as core;
use jig_domain_tasks as tasks;
use jig_protocol_store::{Check, Event, KeptFamily, OpenRefusal, Owner, PatchSet, PatchSets, SecretOwner, Secrets};
use jig_store_world::{World, patches, root};
use skein_fake_machine::Item;
use skein_lib::Secret;

const SEED: u64 = 401;
// fixture-token's schema bounds its payload at 64 bytes.
const TOKEN_BOUND: usize = 64;
// Manifest's family and name bounds (store.md 10.3).
const FAMILY_BOUND: usize = 64;
const NAME_BOUND: usize = 64;
const CARRIED_FIRST: u64 = 1_u64 << 63;

fn empty() -> patches::Records {
    patches::Records { labels: vec![], shelves: vec![], notes: vec![], tokens: vec![] }
}

fn manifest(world: &mut World, generation: u64) -> jig_protocol_store::codec::component::Manifest {
    let contents = world.contents();
    let path = format!("generations/{generation}/manifest").into_bytes();
    let bytes = contents.get(&path).expect("generation manifest").as_ref().expect("manifest file");
    jig_protocol_store::codec::component::Manifest::decode(bytes).expect("manifest decodes")
}

fn token_key(number: u64) -> Box<[u8]> {
    // Application keys at the patch boundary are relative to their map's prefix (10.3).
    Box::from(number.to_be_bytes())
}

fn load_key(record: &core::Record) -> Box<[u8]> {
    jig_protocol_store::keys::encode(&record.key(), patches::limits().store.key).expect("fixture key fits")
}

fn refuses_check(world: &mut World, expected: &Check) {
    let before = patches::source(world);
    world.patch_sets(patches::SETS);
    for attempt in 0..2 {
        assert_eq!(world.open(), None, "check attempt {attempt}");
        assert!(
            matches!(world.refusal(), Some(OpenRefusal::Check { why }) if why == expected),
            "expected {expected:?}, got {:?}",
            world.refusal()
        );
        assert_eq!(world.active(), b"1\n");
        patches::kept(&before, &patches::source(world), &attempt);
        assert!(world.paths().iter().any(|path| path == b"generations/2/store"));
        assert!(!world.paths().iter().any(|path| path == b"generations/2/manifest"));
        // A marker proves that the next start removes the refused copy.
        assert!(!world.paths().iter().any(|path| path == b"generations/2/refused-copy"));
        world.lay(&[Item::file(b"generations/2/refused-copy", b"stray")]);
        world.crash(SEED);
    }
    world.patch_sets(patches::BASE);
    assert_eq!(world.open(), Some(1), "the build before opens its source");
    assert!(!world.paths().iter().any(|path| path.starts_with(b"generations/2")));
    patches::kept(&before, &patches::source(world), &"build before");
}

#[test]
fn tokens_at_bounds_keep_numbers_across_many_pages() {
    // Two-row pages, seventeen tokens, including both ends of the drawn-number exclusion.
    const TOKENS: u64 = 17;
    let mut records = patches::Records::story();
    records.tokens = (0..TOKENS - 1).map(|n| (n, vec![b'x'; TOKEN_BOUND])).collect();
    records.tokens.push((CARRIED_FIRST - 1, vec![]));
    let mut limits = patches::limits();
    // Only the secrets map has one operation and room for a single token per page.
    limits.secrets.ops = 1;
    limits.secrets.value = 128;
    limits.secrets.page.bytes = 128;
    let mut world = records.committed_with(SEED, patches::BASE, 1, limits, &[]);
    let before = patches::source(&mut world);
    world.patch_sets(patches::SETS);
    let last = world.open().expect("tokens patch with their map's bounds");
    patches::whole(&mut world);
    patches::kept(&before, &patches::source(&mut world), &"tokens");
    records.check(&mut world, 2, &"both token bounds and number bounds");
    assert_eq!(
        patches::counts(&mut world),
        vec![
            (b"fixture-label".to_vec(), 2, 2),
            (b"fixture-shelf".to_vec(), 3, 3),
            (b"fixture-token".to_vec(), TOKENS, TOKENS),
            (b"fixture-note".to_vec(), 2, 2),
        ]
    );
    let seal = manifest(&mut world, 2);
    assert_eq!(seal.store, last);
    assert!(seal.secrets.is_some_and(|number| number > TOKENS));
    let core::Record::Core(core::CoreRecord::Deployment(header)) = world.get(root::header(last).key()) else {
        panic!("deployment header");
    };
    assert_eq!(header.commits, last);
    assert_eq!(world.commit(Box::new([jig_protocol_store::Write::Save(root::header(last + 1))])), Some(last + 1));
    world.crash(SEED);
    world.reopen();
    records.check(&mut world, 2, &"restart at patched levels");
    assert!(world.patched().is_empty());
}

#[test]
fn empty_families_have_zero_manifest_counts() {
    let records = empty();
    let mut world = records.committed(SEED, patches::BASE);
    world.patch_sets(patches::SETS);
    world.open().expect("empty families patch");
    records.check(&mut world, 2, &"no tokens or records");
    assert_eq!(
        patches::counts(&mut world),
        vec![
            (b"fixture-label".to_vec(), 0, 0),
            (b"fixture-shelf".to_vec(), 0, 0),
            (b"fixture-token".to_vec(), 0, 0),
            (b"fixture-note".to_vec(), 0, 0),
        ]
    );
}

#[test]
fn sparse_counts_exclude_unkept_opaque_application_rows() {
    let mut records = empty();
    records.labels.push((b"l/0".to_vec(), b"kept".to_vec()));
    records.tokens.push((9, b"credential".to_vec()));
    let mut world = records.committed(SEED, patches::BASE);
    assert_eq!(world.open(), Some(1));
    let key: Box<[u8]> = Box::from(b"z/opaque".as_slice());
    let value: Box<[u8]> = Box::from(b"not a versioned record".as_slice());
    let writes = Box::new([
        jig_protocol_store::Write::Save(root::header(2)),
        jig_protocol_store::Write::Put { key: key.clone(), value: value.clone() },
    ]);
    assert_eq!(world.commit(writes), Some(2));
    world.crash(SEED);
    world.patch_sets(patches::SETS);
    world.open().expect("unkept rows are copied as bytes");
    let mut expected = records.rows(2);
    expected.insert(key, value);
    assert_eq!(world.application(false), expected);
    assert_eq!(
        patches::counts(&mut world),
        vec![
            (b"fixture-label".to_vec(), 1, 1),
            (b"fixture-shelf".to_vec(), 0, 0),
            (b"fixture-token".to_vec(), 1, 1),
            (b"fixture-note".to_vec(), 0, 0),
        ]
    );
}

#[test]
fn first_manifest_counts_each_empty_kept_family() {
    let mut world = World::unopened_observed(SEED, patches::limits(), None);
    world.patch_sets(patches::SETS);
    assert_eq!(world.open(), Some(0));
    let first = manifest(&mut world, 1);
    let counts: Vec<_> = first.counts.iter().map(|row| (row.family().to_vec(), row.before(), row.after())).collect();
    assert_eq!(
        counts,
        vec![
            (b"fixture-label".to_vec(), 0, 0),
            (b"fixture-shelf".to_vec(), 0, 0),
            (b"fixture-token".to_vec(), 0, 0),
            (b"fixture-note".to_vec(), 0, 0),
        ]
    );
    assert_eq!(first.store, 0);
    assert_eq!(first.secrets, None);
}

#[test]
fn first_manifest_without_sets_has_no_counts() {
    let mut world = World::unopened_observed(SEED, patches::limits(), None);
    assert_eq!(world.open(), Some(0));
    assert!(manifest(&mut world, 1).counts.is_empty());
}

#[test]
fn carried_counter_and_unwalked_values_survive_switch() {
    let records = empty();
    let mut world = records.committed(SEED, patches::BASE);
    assert_eq!(world.open(), Some(1));
    let carried = world.secret(Secrets::Carry {
        kind: jig_protocol_store::CarriedKind::Value,
        value: Secret::new(Box::from(b"carried".as_slice())),
    });
    assert!(matches!(carried.as_slice(), [Event::Carried { number, .. }] if *number == CARRIED_FIRST));
    world.crash(SEED);
    world.patch_sets(patches::SETS);
    world.open().expect("patch with carried row");
    let found = world.secret(Secrets::Get { owner: SecretOwner::Application, number: CARRIED_FIRST });
    assert!(
        matches!(found.as_slice(), [Event::Secret { value: Some(jig_protocol_store::SecretValue::Carried { kind: jig_protocol_store::CarriedKind::Value, value }), .. }] if value.expose() == b"carried")
    );
    let carried = world.secret(Secrets::Carry {
        kind: jig_protocol_store::CarriedKind::Value,
        value: Secret::new(Box::from(b"next".as_slice())),
    });
    assert!(matches!(carried.as_slice(), [Event::Carried { number, .. }] if *number == CARRIED_FIRST + 1));
    assert_eq!(patches::counts(&mut world)[2], (b"fixture-token".to_vec(), 0, 0));
}

#[test]
fn erased_carried_number_is_never_drawn_after_switch() {
    let mut world = empty().committed(SEED, patches::BASE);
    assert_eq!(world.open(), Some(1));
    let carried = world.secret(Secrets::Carry {
        kind: jig_protocol_store::CarriedKind::Value,
        value: Secret::new(Box::from(b"erased".as_slice())),
    });
    assert!(matches!(carried.as_slice(), [Event::Carried { number, .. }] if *number == CARRIED_FIRST));
    let erased = world.secret(Secrets::Erase { owner: SecretOwner::Application, number: CARRIED_FIRST });
    assert!(matches!(erased.as_slice(), [Event::Erased { .. }]));
    world.crash(SEED);
    world.patch_sets(patches::SETS);
    world.open().expect("patch with an erased carried value");
    let carried = world.secret(Secrets::Carry {
        kind: jig_protocol_store::CarriedKind::Value,
        value: Secret::new(Box::from(b"next".as_slice())),
    });
    assert!(matches!(carried.as_slice(), [Event::Carried { number, .. }] if *number == CARRIED_FIRST + 1));
}

#[test]
fn stray_secrets_file_is_copied_before_privacy_check() {
    let records = patches::Records::story();
    let mut world = records.committed(SEED, patches::BASE);
    // The source file is public; the copy must make it private.
    let mut stray = Item::file(b"generations/1/secrets/stray", b"fixture stray");
    stray.mode = 0o644;
    world.lay(&[stray]);
    let before = patches::source(&mut world);
    world.patch_sets(patches::SETS);
    world.open().expect("copy makes even a stray regular secrets file private");
    patches::kept(&before, &patches::source(&mut world), &"secrets stray");
    assert_eq!(world.contents().get(b"generations/2/secrets/stray".as_slice()), Some(&Some(b"fixture stray".to_vec())));
    records.check(&mut world, 2, &"stray beside secrets map");
}

#[test]
fn token_past_schema_bound_refuses_without_switch() {
    let mut records = empty();
    records.tokens.push((0, vec![b'x'; TOKEN_BOUND + 1]));
    let mut world = records.committed(SEED, patches::BASE);
    let before = patches::source(&mut world);
    world.patch_sets(patches::SETS);
    for attempt in 0..2 {
        assert_eq!(world.open(), None);
        assert!(
            matches!(world.refusal(), Some(OpenRefusal::Patch { owner: Owner::Application, key, .. })
            if *key == token_key(0)),
            "{:?}",
            world.refusal()
        );
        assert_eq!(world.active(), b"1\n");
        patches::kept(&before, &patches::source(&mut world), &attempt);
        world.crash(SEED);
    }
    world.patch_sets(patches::BASE);
    assert_eq!(world.open(), Some(1));
}

#[test]
fn secrets_record_one_byte_past_commit_room_refuses_large() {
    // kv.md 3: frame 26 + put framing 9 + key 10 + Token before 78 = 123.
    // The patch adds the absent-option tag, making this record alone cost 124.
    const BEFORE_COMMIT: u32 = 123;
    let mut records = empty();
    records.tokens.push((0, vec![b'x'; TOKEN_BOUND]));
    let mut world = records.committed(SEED, patches::BASE);
    let mut limits = patches::limits();
    limits.secrets.commit = BEFORE_COMMIT;
    world.restart_with_limits(SEED, limits);
    refuses_check(&mut world, &Check::Large { key: token_key(0) });
}

#[test]
fn secrets_record_exactly_at_commit_room_patches() {
    const PATCHED_COMMIT: u32 = 124; // 26-byte frame + 9-byte put + 10-byte key + 79-byte Token.
    let mut records = empty();
    records.tokens.push((0, vec![b'x'; TOKEN_BOUND]));
    let mut world = records.committed(SEED, patches::BASE);
    let mut limits = patches::limits();
    limits.secrets.commit = PATCHED_COMMIT;
    world.restart_with_limits(SEED, limits);
    world.patch_sets(patches::SETS);
    world.open().expect("single token fits exactly");
    records.check(&mut world, 2, &"commit bound");
}

#[test]
fn oversized_patch_record_precedes_restart_load_check() {
    const BEFORE_COMMIT: u32 = 123; // The Token's 124-byte patched frame exceeds it.
    let mut records = empty();
    records.tokens.push((0, vec![b'x'; TOKEN_BOUND]));
    let mut world = records.committed_with(SEED, patches::BASE, 1, patches::limits(), &[root::live_task(root::LIVE)]);
    let mut limits = patches::limits();
    limits.secrets.commit = BEFORE_COMMIT;
    limits.core.tasks.spec_bytes = 1;
    world.restart_with_limits(SEED, limits);
    refuses_check(&mut world, &Check::Large { key: token_key(0) });
}

#[test]
fn live_record_over_decode_limit_refuses_load() {
    let records = empty();
    let row = root::live_task(root::LIVE);
    let mut world = records.committed_with(SEED, patches::BASE, 1, patches::limits(), std::slice::from_ref(&row));
    let mut limits = patches::limits();
    limits.core.tasks.spec_bytes = 1;
    world.restart_with_limits(SEED, limits);
    refuses_check(&mut world, &Check::Load { key: load_key(&row) });
}

fn task_with_words(number: u64, bytes: usize) -> core::Record {
    let mut row = root::live_task(number);
    let core::Record::Tasks(tasks::Stored::Live(task)) = &mut row else {
        panic!("task fixture");
    };
    task.spec.words = vec![b'w'; bytes].into_boxed_slice();
    row
}

#[test]
fn live_record_exactly_at_decode_bound_switches() {
    let bound = usize::try_from(root::TASKS_LIMITS.spec_bytes).expect("spec bound fits");
    let row = task_with_words(root::LIVE, bound);
    let mut world = empty().committed_with(SEED, patches::BASE, 1, patches::limits(), std::slice::from_ref(&row));
    world.patch_sets(patches::SETS);
    world.open().expect("codec limit is inclusive");
    assert_eq!(world.get(row.key()), row);
}

#[test]
fn live_record_one_byte_past_decode_bound_refuses_load() {
    let mut initial = patches::limits();
    initial.core.tasks.spec_bytes += 1;
    let bytes = usize::try_from(initial.core.tasks.spec_bytes).expect("spec bound fits");
    let row = task_with_words(root::LIVE, bytes);
    let mut world = empty().committed_with(SEED, patches::BASE, 1, initial, std::slice::from_ref(&row));
    world.restart_with_limits(SEED, patches::limits());
    refuses_check(&mut world, &Check::Load { key: load_key(&row) });
}

#[test]
fn first_bad_live_key_wins_over_commit_insertion_order() {
    let rows = [root::live_task(root::LIVE + 1), root::live_task(root::LIVE)];
    let mut world = empty().committed_with(SEED, patches::BASE, 1, patches::limits(), &rows);
    let mut limits = patches::limits();
    limits.core.tasks.spec_bytes = 1;
    world.restart_with_limits(SEED, limits);
    refuses_check(&mut world, &Check::Load { key: load_key(&rows[1]) });
}

#[test]
fn ended_position_without_its_task_refuses_join() {
    let row = core::Record::People(jig_domain_people::Stored::EndedRead { task: root::LIVE, party: 1, through: 0 });
    let mut world = empty().committed_with(SEED, patches::BASE, 1, patches::limits(), std::slice::from_ref(&row));
    refuses_check(&mut world, &Check::Join { key: load_key(&row) });
}

#[test]
fn live_decode_failure_precedes_missing_ended_join() {
    let row = root::live_task(root::LIVE);
    let position =
        core::Record::People(jig_domain_people::Stored::EndedRead { task: root::LIVE + 1, party: 1, through: 0 });
    let rows = [position, row.clone()];
    let mut world = empty().committed_with(SEED, patches::BASE, 1, patches::limits(), &rows);
    let mut limits = patches::limits();
    limits.core.tasks.spec_bytes = 1;
    world.restart_with_limits(SEED, limits);
    refuses_check(&mut world, &Check::Load { key: load_key(&row) });
}

#[test]
fn ended_position_with_its_task_passes_join() {
    let core::Record::Tasks(tasks::Stored::Live(mut task)) = root::live_task(root::LIVE) else {
        panic!("task fixture");
    };
    task.ended_at = Some(skein_lib::Wall::EPOCH);
    let ended = core::Record::Tasks(tasks::Stored::Ended(task));
    let position =
        core::Record::People(jig_domain_people::Stored::EndedRead { task: root::LIVE, party: 1, through: 0 });
    let mut world = empty().committed_with(SEED, patches::BASE, 1, patches::limits(), &[ended, position]);
    world.patch_sets(patches::SETS);
    world.open().expect("ended read position's task is held");
    assert_eq!(world.active(), b"2\n");
}

#[test]
fn store_record_one_byte_past_commit_room_refuses_large() {
    // fixture-shelf and its held label each allow 32 bytes of text.
    const TEXT_BOUND: usize = 32;
    // 26-byte frame + 9-byte put + 4-byte tagged key + 89-byte patched Shelf = 128.
    const PATCHED_COMMIT: u32 = 128;
    let mut records = empty();
    records.shelves.push(patches::Shelf {
        key: b"s/0".to_vec(),
        name: vec![b's'; TEXT_BOUND],
        books: 1,
        caption: vec![b'l'; TEXT_BOUND],
    });
    let mut initial = patches::limits();
    // Recover from a snapshot: the initial commit also contains the format and header,
    // and is larger than the single-record commit room under test.
    initial.store.snapshot_after = 1;
    let mut world = records.committed_with(SEED, patches::BASE, 1, initial, &[]);
    let mut limits = patches::limits();
    limits.store.commit = PATCHED_COMMIT - 1;
    world.restart_with_limits(SEED, limits);
    refuses_check(&mut world, &Check::Large { key: Box::from(b"s/0".as_slice()) });
}

#[test]
fn live_sets_past_arena_limits_switch_before_restart() {
    let records = empty();
    // The check decodes each row, but does not enforce the child's live-set bound (store.md 16).
    let rows = [root::live_task(root::LIVE), root::live_task(root::LIVE + 1), root::live_task(root::LIVE + 2)];
    let mut world = records.committed_with(SEED, patches::BASE, 1, patches::limits(), &rows);
    let before = patches::source(&mut world);
    world.patch_sets(patches::SETS);
    world.open().expect("live set's arena bound is outside the check");
    assert_eq!(world.active(), b"2\n");
    world.crash(SEED);
    let restored = world.reopen();
    for row in &rows {
        assert_eq!(restored.rows.get(&row.key()), Some(row));
    }
    patches::kept(&before, &patches::source(&mut world), &"set past child bound");
    world.crash(SEED);
    world.rewrite(b"active", b"1\n");
    world.patch_sets(patches::BASE);
    assert_eq!(world.open(), Some(1));
    assert!(!world.paths().iter().any(|path| path.starts_with(b"generations/2")));
}

#[test]
fn child_semantic_rule_is_outside_pre_switch_check() {
    let mut row = root::live_task(root::LIVE);
    let core::Record::Tasks(tasks::Stored::Live(task)) = &mut row else {
        panic!("task fixture");
    };
    task.number = 0;
    let mut world = empty().committed_with(SEED, patches::BASE, 1, patches::limits(), &[row]);
    world.patch_sets(patches::SETS);
    world.open().expect("codec-valid child rule is not checked before switch");
    assert_eq!(world.active(), b"2\n");
}

fn bounded_sets(families: usize, name_bytes: usize) -> PatchSets {
    let application = &patches::SETS.sets[0];
    let upstream = &patches::SETS.sets[1];
    let mut kept = application.families.to_vec();
    // One family is in the upstream set; each additional family has an empty key range.
    for index in kept.len()..families - upstream.families.len() {
        let name: &'static str = Box::leak(format!("f{index:0width$}", width = name_bytes - 1).into_boxed_str());
        let text: &'static str =
            Box::leak(format!("family {name} 1\nversioned record Empty {{ n: u64 }}\n").into_boxed_str());
        let texts = Box::leak(vec![text].into_boxed_slice());
        kept.push(KeptFamily { frames: None, name, texts, map: jig_protocol_store::KeptMap::Store, keys: &[] });
    }
    let kept = Box::leak(kept.into_boxed_slice());
    let sets = Box::leak(vec![PatchSet { families: kept, ..*application }, *upstream].into_boxed_slice());
    PatchSets { sets }
}

#[test]
fn manifest_family_and_name_bounds_hold_at_first_start() {
    let sets = bounded_sets(FAMILY_BOUND, NAME_BOUND);
    let mut world = World::unopened_observed(SEED, patches::limits(), None);
    world.patch_sets(sets);
    assert_eq!(world.open(), Some(0), "{:?}", world.refusal());
    let first = manifest(&mut world, 1);
    assert_eq!(usize::try_from(first.counts.len()).expect("count fits"), FAMILY_BOUND);
    assert!(first.counts.iter().all(|row| row.before() == 0 && row.after() == 0));
}

#[test]
fn each_manifest_bound_fits_without_the_other_at_bound() {
    // Each axis is legal independently, including the two endpoints of the combined case.
    const SHORT_NAME: usize = 8;
    const SPARSE_FAMILIES: usize = 5; // Four fixture families and one empty family.
    for (families, name_bytes) in [(FAMILY_BOUND, SHORT_NAME), (SPARSE_FAMILIES, NAME_BOUND)] {
        let mut world = World::unopened_observed(SEED, patches::limits(), None);
        world.patch_sets(bounded_sets(families, name_bytes));
        assert_eq!(world.open(), Some(0), "{families} families, {name_bytes} bytes: {:?}", world.refusal());
        let first = manifest(&mut world, 1);
        assert_eq!(usize::try_from(first.counts.len()).expect("count fits"), families);
    }
}

#[test]
fn combined_manifest_bounds_fit_with_larger_file_chunks() {
    // The manifest at both bounds exceeds the fixture's 4096-byte file chunk.
    const MANIFEST_CHUNK: u32 = 8192;
    let mut limits = patches::limits();
    limits.store.chunk = MANIFEST_CHUNK;
    let mut world = World::unopened_observed(SEED, limits, None);
    world.patch_sets(bounded_sets(FAMILY_BOUND, NAME_BOUND));
    assert_eq!(world.open(), Some(0), "{:?}", world.refusal());
    let first = manifest(&mut world, 1);
    assert_eq!(usize::try_from(first.counts.len()).expect("count fits"), FAMILY_BOUND);
}

#[test]
fn combined_manifest_bounds_patch_with_larger_file_chunks() {
    const MANIFEST_CHUNK: u32 = 8192; // Both manifest bounds together exceed a 4096-byte chunk.
    let mut limits = patches::limits();
    limits.store.chunk = MANIFEST_CHUNK;
    let mut world = empty().committed(SEED, patches::BASE);
    world.restart_with_limits(SEED, limits);
    world.patch_sets(bounded_sets(FAMILY_BOUND, NAME_BOUND));
    assert!(world.open().is_some(), "{:?}", world.refusal());
    assert_eq!(patches::counts(&mut world).len(), FAMILY_BOUND);
}

#[test]
fn manifest_family_and_name_bounds_hold_behind() {
    let sets = bounded_sets(FAMILY_BOUND, NAME_BOUND);
    let mut world = empty().committed(SEED, patches::BASE);
    world.patch_sets(sets);
    assert!(world.open().is_some(), "bounds accepted on the patch path: {:?}", world.refusal());
    let counts = patches::counts(&mut world);
    assert_eq!(counts.len(), FAMILY_BOUND);
    assert!(counts.iter().all(|(_, before, after)| *before == 0 && *after == 0));
}

#[test]
fn manifest_bounds_refuse_sets_before_any_first_start_write() {
    for (families, name_bytes, owner) in
        [(FAMILY_BOUND + 1, NAME_BOUND, Owner::Jig), (FAMILY_BOUND, NAME_BOUND + 1, Owner::Application)]
    {
        let mut world = World::unopened_observed(SEED, patches::limits(), None);
        world.patch_sets(bounded_sets(families, name_bytes));
        let before = world.contents();
        world.trace_writes();
        assert_eq!(world.open(), None);
        assert!(
            matches!(world.refusal(), Some(OpenRefusal::Set { owner: found, patch: 0 }) if *found == owner),
            "{:?}",
            world.refusal()
        );
        assert_eq!(world.contents(), before);
        assert!(world.changes().is_empty());
    }
}

#[test]
fn manifest_bounds_refuse_sets_behind_without_removing_strays() {
    for (families, name_bytes, owner) in
        [(FAMILY_BOUND + 1, NAME_BOUND, Owner::Jig), (FAMILY_BOUND, NAME_BOUND + 1, Owner::Application)]
    {
        let mut world = empty().committed(SEED, patches::BASE);
        world.lay(&[Item::directory(b"generations/9"), Item::file(b"generations/9/stray", b"untouched")]);
        let before = world.contents();
        world.patch_sets(bounded_sets(families, name_bytes));
        assert_eq!(world.open(), None);
        assert!(
            matches!(world.refusal(), Some(OpenRefusal::Set { owner: found, patch: 0 }) if *found == owner),
            "{:?}",
            world.refusal()
        );
        patches::kept(&before, &world.contents(), &"invalid manifest bounds");
        assert_eq!(world.active(), b"1\n");
    }
}

#[test]
fn cuts_across_check_and_switch_recover_whole_maps() {
    let records = patches::Records::story();
    let mut complete = records.committed(SEED, patches::BASE);
    complete.patch_sets(patches::SETS);
    complete.trace_writes();
    complete.open().expect("complete patch to enumerate its cuts");
    let cuts = patches::cuts(&complete);
    let first = *cuts.first().expect("copy has cut points");
    let after_switch = cuts.last().expect("switch has cut points") + 1;
    let mut source_opened = false;
    let mut switched = false;
    // Include the reads between mutations: the check itself has no writes at which to cut.
    for cut in first..=after_switch {
        let mut world = records.committed(SEED, patches::BASE);
        let before = patches::source(&mut world);
        world.patch_sets(patches::SETS);
        world.trace_writes();
        world.cut_at(cut);
        world.open();
        world.crash(SEED);
        // A build at the source levels either opens the untouched source or refuses the switched copy as ahead.
        world.patch_sets(patches::BASE);
        if world.active() == b"1\n" {
            source_opened = true;
            assert_eq!(world.open(), Some(1), "cut {cut}");
            records.check(&mut world, 1, &cut);
            assert!(!world.paths().iter().any(|path| path.starts_with(b"generations/2")));
        } else {
            switched = true;
            assert_eq!(world.open(), None, "cut {cut}");
            assert!(matches!(world.refusal(), Some(OpenRefusal::Ahead { .. })), "cut {cut}: {:?}", world.refusal());
        }
        patches::kept(&before, &patches::source(&mut world), &cut);
        world.crash(SEED);
        world.patch_sets(patches::SETS);
        world.open().expect("retry after cut");
        patches::whole(&mut world);
        records.check(&mut world, 2, &cut);
    }
    assert!(source_opened && switched, "cuts reached each whole generation");
}
