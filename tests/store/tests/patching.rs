use jig_protocol_store::{Check, Counts, Older, OpenRefusal, Owner, PatchSet, PatchSets, Write};
use jig_store_world::{
    patches::{self, BASE, Records, SETS},
    root,
};

#[test]
fn old_records_patch_in_order_and_the_root_continues() {
    let records = Records::story();
    let mut world = records.committed(901, BASE);
    let original = patches::source(&mut world);
    world.patch_sets(SETS);
    let last = world.open_switched().expect("patched open");
    assert_eq!(
        world.patched(),
        [
            (Owner::Application, 1, Counts { records: 5, commits: 2 }),
            (Owner::Jig, 1, Counts { records: 2, commits: 1 }),
            (Owner::Application, 2, Counts { records: 6, commits: 3 }),
            (Owner::Jig, 2, Counts { records: 0, commits: 0 }),
        ]
    );
    assert_eq!(last, 6); // Initial commit, four store patch pages and the seal; two secrets pages have their own numbers.
    patches::whole(&mut world);
    assert_eq!(
        patches::counts(&mut world),
        [
            (b"fixture-label".to_vec(), 2, 2),
            (b"fixture-shelf".to_vec(), 3, 3),
            (b"fixture-token".to_vec(), 3, 3),
            (b"fixture-note".to_vec(), 2, 2)
        ]
    );
    patches::kept(&original, &patches::source(&mut world), &"story");
    records.check(&mut world, 2, &"story");
    world.crash(902);
    // The fixture check committed this start's mark after the patch seal.
    assert_eq!(world.open_switched(), Some(last + 1));
    assert!(world.patched().is_empty());
    assert_eq!(world.active(), b"2\n");
    assert_eq!(world.commit(Box::new([Write::Save(root::header(last + 2)), Write::Wires])), Some(last + 2));
    world.crash(903);
    assert_eq!(world.reopen().last, last + 2);
    records.check(&mut world, 2, &"story");
}

#[test]
fn level_and_set_refusals_leave_every_file_as_it_was() {
    const BAD: PatchSets =
        PatchSets { sets: &[PatchSet { patches: &["patch 1\nowner other 1\n"], ..SETS.sets[0] }, SETS.sets[1]] };
    let records = Records::story();
    for (made, version, opened, expected) in [
        (SETS, 2, BASE, OpenRefusal::Ahead { owner: Owner::Jig, found: Some(2), wanted: Some(0) }),
        (PatchSets::NONE, 1, SETS, OpenRefusal::OlderBuild { why: Older::Unleveled }),
        (BASE, 1, BAD, OpenRefusal::Set { owner: Owner::Application, patch: 1 }),
    ] {
        let mut world = records.committed_at(904, made, version);
        world.patch_sets(opened);
        let before = world.contents();
        assert_eq!(world.open_switched(), None);
        assert_eq!(world.refusal(), Some(&expected));
        patches::kept(&before, &world.contents(), &expected);
    }
}

#[test]
fn an_empty_shelf_refuses_again_from_an_unchanged_source() {
    let mut records = Records::story();
    records.shelves[1].books = 0;
    let mut world = records.committed(905, BASE);
    let before = patches::source(&mut world);
    world.patch_sets(SETS);
    for seed in [906, 907] {
        assert_eq!(world.open_switched(), None);
        let Some(OpenRefusal::Patch { owner, patch, key, halt }) = world.refusal() else {
            panic!("typed patch refusal: {:?}", world.refusal())
        };
        assert_eq!((*owner, *patch, key.as_ref()), (Owner::Application, 2, b"s/1".as_slice()));
        assert_eq!(*halt, skein_codec::patch::Halt::Refused(jig_test_patches::patches::refusal::p0002::EMPTY));
        assert_eq!(world.active(), b"1\n");
        patches::kept(&before, &patches::source(&mut world), &seed);
        assert!(!world.paths().iter().any(|path| path == b"generations/2/failed-copy"));
        // Cleanup must remove this marker with the old failed copy.
        world.lay(&[skein_fake_machine::Item::file(b"generations/2/failed-copy", b"remove on retry")]);
        world.crash(seed);
    }
    // The second attempt removed the first marker before making a fresh copy.
}

#[test]
fn every_patch_write_cut_recovers_all_records_and_no_stray() {
    let records = Records::story();
    let mut completed = records.committed(908, BASE);
    completed.patch_sets(SETS);
    completed.trace_writes();
    assert!(completed.open_switched().is_some());
    let cuts = patches::cuts(&completed);
    eprintln!("patch-phase disk-changing cuts: {}", cuts.len());
    for cut in cuts {
        let mut world = records.committed(908, BASE);
        world.patch_sets(SETS);
        world.trace_writes();
        world.cut_at(cut);
        assert_eq!(world.open_switched(), None, "cut {cut} reached");
        world.crash(u64::from(cut));
        assert!(world.open_switched().is_some(), "clean start after cut {cut}: {:?}", world.refusal());
        patches::whole(&mut world);
        records.check(&mut world, 2, &(908, cut));
    }
}

#[test]
fn each_check_refusal_leaves_the_generation_before_active() {
    const SEED: u64 = 912;
    const ENDED: u64 = root::LIVE + 1;
    // The base's token commits have become a snapshot, so lowering the commit
    // limit tests the patched put rather than refusing recovery of the log.
    let mut token_limits = patches::limits();
    token_limits.secrets.snapshot_after = 1;
    let mut token_open = token_limits;
    // 26 frame + 9 put + 10 key + 32 v1 token; v2 adds one byte.
    token_open.secrets.commit = 77;
    let mut load_limits = patches::limits();
    load_limits.core.tasks.dependencies = 0;
    let mut live = root::live_task(root::LIVE);
    let jig_domain_core::Record::Tasks(jig_domain_tasks::Stored::Live(task)) = &mut live else { panic!("live task") };
    task.dependencies = Box::new([ENDED]);
    let key = |row: &jig_domain_core::Record| jig_protocol_store::keys::encode(&row.key(), 64).expect("fixture key");
    for (base_limits, open_limits, rows, why) in [
        (token_limits, token_open, vec![], Check::Large { key: Box::new(0_u64.to_be_bytes()) }),
        (patches::limits(), load_limits, vec![live.clone()], Check::Load { key: key(&live) }),
    ] {
        let records = Records::story();
        let mut world = records.committed_with(SEED, BASE, 1, base_limits, &rows);
        let before = patches::source(&mut world);
        world.patch_sets(SETS);
        world.restart_with_limits(SEED, open_limits);
        let expected = OpenRefusal::Check { why };
        for seed in [SEED + 1, SEED + 2] {
            assert_eq!(world.open_switched(), None, "{expected:?}");
            assert_eq!(world.refusal(), Some(&expected));
            assert_eq!(world.active(), b"1\n");
            patches::kept(&before, &patches::source(&mut world), &expected);
            assert!(!world.paths().iter().any(|path| path == b"generations/2/check-copy"));
            world.lay(&[skein_fake_machine::Item::file(b"generations/2/check-copy", b"remove on retry")]);
            world.crash(seed);
        }
    }
}
