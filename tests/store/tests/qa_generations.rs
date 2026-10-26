use jig_protocol_store::{Generations, Older, OpenRefusal};
use jig_store_world::World;
use skein_fake_machine::Item;

fn directory(path: &str) -> Item {
    Item::directory(path.as_bytes()).mode(0o700)
}

fn file(path: &str, bytes: &[u8]) -> Item {
    Item::file(path.as_bytes(), bytes).mode(0o600)
}

fn generation(world: &mut World, number: u64) {
    world.lay(&[
        directory(&format!("generations/{number}")),
        directory(&format!("generations/{number}/store")),
        directory(&format!("generations/{number}/secrets")),
    ]);
}

fn active_generation(number: u64) -> World {
    let mut world = World::unopened(801);
    world.lay(&[directory("generations")]);
    generation(&mut world, number);
    world.lay(&[file("active", format!("{number}\n").as_bytes())]);
    world
}

fn contains(paths: &[Vec<u8>], path: &str) -> bool {
    paths.iter().any(|found| found == path.as_bytes())
}

fn absent_tree(paths: &[Vec<u8>], path: &str) {
    let prefix = format!("{path}/");
    assert!(
        !paths.iter().any(|found| found == path.as_bytes() || found.starts_with(prefix.as_bytes())),
        "stray {path} remains: {paths:?}"
    );
}

fn refused_unchanged(world: &mut World, why: &OpenRefusal) {
    let before = world.paths();
    assert_eq!(world.open_switched(), None, "open must refuse");
    assert_eq!(world.refusal(), Some(why));
    assert_eq!(world.paths(), before, "refusal changes no paths");
}

fn first_start_defects(world: &mut World) -> Vec<String> {
    let mut defects = Vec::new();
    let active = world.active();
    if active != b"1\n" {
        defects.push(format!("active is {active:?}, wanted 1\\n"));
    }
    let paths = world.paths();
    for path in ["generations/1/store", "generations/1/secrets", "generations/1/manifest", "transcripts"] {
        if !contains(&paths, path) {
            defects.push(format!("missing {path}"));
        }
    }
    if paths.iter().any(|path| path.starts_with(b"active.skein-")) {
        defects.push("durable-replace temporary survives".to_owned());
    }
    defects
}

fn clean_first_start(world: &mut World) {
    let defects = first_start_defects(world);
    assert!(defects.is_empty(), "first-start defects: {defects:?}");
}

#[test]
fn first_start_is_empty_and_durable() {
    // store.md 2: The first start makes generation 1's manifest; the maps are empty until the root's first commit.
    let mut world = World::unopened(802);
    assert_eq!(world.open_switched(), Some(0));
    clean_first_start(&mut world);
    world.crash(803);
    let restored = world.reopen();
    assert_eq!(restored.last, 0);
    assert!(restored.rows.is_empty());
    assert!(restored.application.is_empty());
    assert!(restored.transcripts.is_empty());
    clean_first_start(&mut world);
}

#[test]
fn empty_interrupted_generation_restarts() {
    // store.md 2: A lone 1/ whose store/ and secrets/ hold no file is a cut first start, removed and made afresh.
    let mut world = World::unopened(804);
    world.lay(&[directory("generations")]);
    generation(&mut world, 1);
    world.lay(&[file("active.skein-0123456789abcdef", b"unfinished")]);
    assert_eq!(world.open_switched(), Some(0));
    clean_first_start(&mut world);
}

#[test]
fn interrupted_manifest_is_replaced() {
    // store.md 2: A lone 1/ with file-free maps is removed before the first start makes generation 1's manifest.
    let mut world = World::unopened(805);
    world.lay(&[directory("generations")]);
    generation(&mut world, 1);
    world.lay(&[file("generations/1/manifest", b"partial manifest")]);
    assert_eq!(world.open_switched(), Some(0));
    clean_first_start(&mut world);
}

#[test]
fn each_directory_of_todays_layout_refuses_untouched() {
    // store.md 2: store/, secrets/ or transcripts/ at the top with no active refuses Older::Directory before writes.
    for name in ["store", "secrets", "transcripts"] {
        let mut world = World::unopened(806);
        world.lay(&[directory(name), file(&format!("{name}/kept"), b"operator data")]);
        refused_unchanged(&mut world, &OpenRefusal::OlderBuild { why: Older::Directory });
    }
}

#[test]
fn todays_layout_precedes_an_interrupted_first_start() {
    // store.md 2: A first start has no store/, secrets/ or transcripts/ at its top.
    let mut world = World::unopened(807);
    world.lay(&[directory("generations"), directory("transcripts"), file("transcripts/kept", b"turns")]);
    generation(&mut world, 1);
    world.lay(&[file("active.skein-0123456789abcdef", b"1\n")]);
    refused_unchanged(&mut world, &OpenRefusal::OlderBuild { why: Older::Directory });
}

#[test]
fn nonempty_unnamed_maps_are_never_a_first_start() {
    // store.md 2: Anything beyond a cut first start with no active refuses Generations::Unnamed and is left as is.
    for map in ["store", "secrets"] {
        let mut world = World::unopened(808);
        world.lay(&[directory("generations")]);
        generation(&mut world, 1);
        world.lay(&[file(&format!("generations/1/{map}/kept"), b"durable data")]);
        refused_unchanged(&mut world, &OpenRefusal::Generations { why: Generations::Unnamed });
    }
}

#[test]
fn unnamed_other_generations_and_entries_refuse() {
    // store.md 2: A generations/ that holds anything else, with no active, is refused Unnamed and left as it is.
    for items in [
        vec![directory("generations/2")],
        vec![file("generations/unknown", b"keep")],
        vec![directory("generations/1"), directory("generations/2")],
    ] {
        let mut world = World::unopened(809);
        world.lay(&[directory("generations")]);
        world.lay(&items);
        refused_unchanged(&mut world, &OpenRefusal::Generations { why: Generations::Unnamed });
    }
}

#[test]
fn malformed_active_refuses_before_cleanup() {
    // store.md 10.1.2: active is decimal with no leading zero and a newline, at most 21 bytes; anything else refuses Unreadable.
    for bytes in [
        b"".as_slice(),
        b"1",
        b"01\n",
        b"+1\n",
        b"-1\n",
        b" 1\n",
        b"1 \n",
        b"1\r\n",
        b"1\n\n",
        b"1\ntrailing",
        b"\xff\n",
        b"18446744073709551616\n",
        b"184467440737095516150\n",
    ] {
        let mut world = World::unopened(817);
        world.lay(&[directory("generations")]);
        generation(&mut world, 1);
        world.lay(&[
            file("active", bytes),
            directory("generations/stray"),
            file("active.skein-0123456789abcdef", b"keep"),
        ]);
        refused_unchanged(&mut world, &OpenRefusal::Generations { why: Generations::Unreadable });
    }
}

#[test]
fn largest_generation_fits_the_active_limit() {
    // store.md 10.1.2: active loads up to 21 bytes of decimal digits and a newline.
    let mut world = active_generation(u64::MAX);
    assert_eq!(world.open_switched(), Some(0));
    assert_eq!(world.active(), b"18446744073709551615\n");
}

#[test]
fn missing_or_file_maps_refuse_before_removing_strays() {
    // store.md 10.1.3: The active generation's store/ and secrets/ must stand or Missing { generation } refuses.
    for items in [
        vec![],
        vec![directory("generations/7/store")],
        vec![directory("generations/7/secrets")],
        vec![file("generations/7/store", b"file"), directory("generations/7/secrets")],
        vec![directory("generations/7/store"), file("generations/7/secrets", b"file")],
    ] {
        let mut world = World::unopened(810);
        world.lay(&[
            directory("generations"),
            directory("generations/7"),
            directory("generations/8"),
            file("generations/8/kept", b"stray"),
            file("active", b"7\n"),
            file("active.skein-0123456789abcdef", b"keep"),
        ]);
        world.lay(&items);
        refused_unchanged(&mut world, &OpenRefusal::Generations { why: Generations::Missing { generation: 7 } });
    }
}

#[test]
fn previous_is_opaque_and_only_other_generations_are_removed() {
    // store.md 2 and 10.1.5: Nothing reads the previous generation; a start never removes the active or its previous.
    let mut world = active_generation(10);
    world.lay(&[
        directory("generations/9"),
        file("generations/9/store", b"not a map"),
        file("generations/9/secrets", b"not a map either"),
        file("generations/9/manifest", b"opaque"),
        directory("generations/8"),
        file("generations/8/drop", b"old"),
        directory("generations/11"),
        file("generations/11/drop", b"unfinished"),
    ]);
    assert_eq!(world.open_switched(), Some(0));
    assert_eq!(world.active(), b"10\n");
    let paths = world.paths();
    for path in [
        "generations/9/store",
        "generations/9/secrets",
        "generations/9/manifest",
        "generations/10/store",
        "generations/10/secrets",
    ] {
        assert!(contains(&paths, path), "missing {path}");
    }
    absent_tree(&paths, "generations/8");
    absent_tree(&paths, "generations/11");
}

#[test]
fn noncanonical_generation_names_are_strays() {
    // store.md 10.1.5: Every entry but the active generation and its previous is a stray.
    let mut world = active_generation(7);
    world.lay(&[
        directory("generations/6"),
        file("generations/6/kept", b"previous"),
        directory("generations/06"),
        directory("generations/007"),
        directory("generations/7-old"),
    ]);
    assert_eq!(world.open_switched(), Some(0));
    let paths = world.paths();
    assert!(contains(&paths, "generations/6/kept"));
    for path in ["generations/06", "generations/007", "generations/7-old"] {
        absent_tree(&paths, path);
    }
}

fn stray_fixture() -> World {
    let mut world = active_generation(3);
    world.lay(&[
        directory("generations/2"),
        file("generations/2/kept", b"rollback"),
        directory("generations/4"),
        directory("generations/4/deep"),
        directory("generations/4/deep/nested"),
        file("generations/4/deep/nested/data", b"unfinished"),
        directory("generations/dropped"),
        file("generations/dropped/partial", b"old"),
        file("generations/stray-file", b"stray"),
        file("active.skein-0123456789abcdef", b"4\n"),
    ]);
    world
}

fn strays_gone(world: &mut World) {
    assert_eq!(world.active(), b"3\n");
    let paths = world.paths();
    assert!(contains(&paths, "generations/2/kept"));
    assert!(contains(&paths, "generations/3/store"));
    assert!(contains(&paths, "generations/3/secrets"));
    assert!(contains(&paths, "transcripts"));
    for path in ["generations/4", "generations/dropped", "generations/stray-file", "active.skein-0123456789abcdef"] {
        absent_tree(&paths, path);
    }
}

#[test]
fn recursive_cleanup_restarts_directory_pages() {
    // store.md 10.1.5: List each directory from its first page until empty, then remove it deepest first.
    let mut world = stray_fixture();
    for number in 0..160 {
        world.lay(&[file(&format!("generations/4/deep/file-{number:03}"), b"partial")]);
    }
    assert_eq!(world.open_switched(), Some(0));
    strays_gone(&mut world);
}

#[test]
fn only_durable_replace_temporaries_are_removed() {
    // store.md 2 and 10.1.5: Remove temporaries active.skein-<16 hex> left beside active.
    let mut world = active_generation(1);
    let kept = [
        "active.skein-0123456789abcde",
        "active.skein-0123456789abcdef0",
        "active.skein-zzzzzzzzzzzzzzzz",
        "active.notes",
    ];
    for name in kept {
        world.lay(&[file(name, b"unrelated")]);
    }
    for name in ["active.skein-0123456789abcdef", "active.skein-fedcba9876543210"] {
        world.lay(&[file(name, b"unfinished")]);
    }
    assert_eq!(world.open_switched(), Some(0));
    let paths = world.paths();
    for name in kept {
        assert!(contains(&paths, name), "unrelated entry removed: {name}");
    }
    absent_tree(&paths, "active.skein-0123456789abcdef");
    absent_tree(&paths, "active.skein-fedcba9876543210");
}

#[test]
fn active_layout_keeps_shared_transcripts() {
    // store.md 2: Transcripts are shared by every generation; today's layout refuses only with no active.
    let mut world = active_generation(2);
    world.lay(&[directory("transcripts"), directory("transcripts/42"), file("transcripts/42/kept", b"turns")]);
    assert_eq!(world.open_switched(), Some(0));
    assert!(contains(&world.paths(), "transcripts/42/kept"));
}

#[test]
fn every_first_start_cut_recovers_one_whole_generation() {
    // store.md 10.4: Before active's rename remove the partial first start; after it open 1 and make transcripts/.
    let mut baseline = World::unopened(811);
    assert_eq!(baseline.open_switched(), Some(0));
    let operations = baseline.operation_count();
    assert!(operations > 0);
    let mut failures = Vec::new();
    for cut in 1..=operations {
        for crash_seed in [812, 813] {
            let mut world = World::unopened(811);
            world.cut_at(cut);
            let _ = world.open_switched();
            world.crash(crash_seed);
            assert_eq!(world.open_switched(), Some(0), "cut {cut}, crash seed {crash_seed}");
            let defects = first_start_defects(&mut world);
            if !defects.is_empty() {
                failures.push((cut, crash_seed, "first restart", defects));
            }
            world.crash(814);
            assert_eq!(world.open_switched(), Some(0), "second restart after cut {cut}");
            let defects = first_start_defects(&mut world);
            if !defects.is_empty() {
                failures.push((cut, crash_seed, "second restart", defects));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} defective restarts across {operations} cut points, two crash seeds and two restarts; first failures: {:?}",
        failures.len(),
        failures.iter().take(4).collect::<Vec<_>>()
    );
}

#[test]
fn every_stray_removal_cut_finishes_cleanup() {
    // store.md 10.4: A cut removing strays leaves part of a stray; the next start removes the rest.
    let mut baseline = stray_fixture();
    assert_eq!(baseline.open_switched(), Some(0));
    let operations = baseline.operation_count();
    assert!(operations > 0);
    for cut in 1..=operations {
        for crash_seed in [815, 816] {
            let mut world = stray_fixture();
            world.cut_at(cut);
            let _ = world.open_switched();
            world.crash(crash_seed);
            assert_eq!(world.open_switched(), Some(0), "cut {cut}, crash seed {crash_seed}");
            strays_gone(&mut world);
        }
    }
}

#[test]
fn generation_refusal_words_match_operator_instructions() {
    // store.md 10.1: Each Generations refusal's text, whole, is what OpenRefusal::words gives.
    for (why, expected) in [
        (
            Generations::Busy,
            "Another process has <deployment> open, and this start has changed nothing in it. Stop that process first, then start again.",
        ),
        (
            Generations::Unreadable,
            "The `active` file in <deployment> does not name a generation, and this start has changed nothing in it. Restore <deployment> from a backup, or write `active` again to name a generation under `generations/` that stands, and start again.",
        ),
        (
            Generations::Missing { generation: 17 },
            "The generation `active` names in <deployment> has lost its `store/` or `secrets/`, and this start has changed nothing in it. Restore <deployment> from a backup, or write `active` again to name a generation under `generations/` that stands, and start again.",
        ),
        (
            Generations::Unnamed,
            "<deployment> holds generations that no `active` names, and this start has changed nothing in it. Name one again from a backup of <deployment>, or write `active` to name the generation under `generations/` to open, and start again.",
        ),
    ] {
        assert_eq!(OpenRefusal::Generations { why }.words().join("<deployment>"), expected);
    }
}

#[test]
fn an_older_builds_refusal_names_the_whole_layout_to_move() {
    // store.md 2: Older::Directory carries the whole instruction to move store/, secrets/ and transcripts/ together.
    let words = OpenRefusal::OlderBuild { why: Older::Directory }.words().join("<deployment>");
    assert_eq!(
        words,
        "<deployment> holds a store from an older build, in the layout before generations (`store/`, `secrets/` or `transcripts/` at its top), and this start has changed nothing in it. Before the first release a store is not translated, so every start refuses this one until it is moved. To start afresh, stop the service, move `store/`, `secrets/` and `transcripts/` out of <deployment> together, and start again: nothing is deleted, and the next start makes a new, empty store. The new store has no tasks, sign-ins, accounts or keys, and every client signs in again."
    );
}
