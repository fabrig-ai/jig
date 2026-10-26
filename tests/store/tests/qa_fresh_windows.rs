//! Independent QA from store.md 8.6 and 10.2–10.4.
use jig_domain_core as core;
use jig_protocol_store::{Event, Owner, PatchSet, PatchSets, Request, TranscriptRead, Write, codec};
use jig_store_world::{World, patches, referee::Referee, root, world::Change};
use skein_lib::Token;
use std::collections::BTreeMap;

const SEED: u64 = 719;
const TURNS_PER_TASK: usize = 2; // patches::fresh_committed writes two turns.
const COMMITS_PER_TASK: u64 = 4; // live row, two turns, final proof.
const CLOSING_PAGES: usize = 2; // Six proofs at the fixture's four writes per page.
const PENDING_PATCHES: usize = 2; // The two header-only patches below.
const SEAL_COMMITS: u64 = 1; // store.md 10.2, step 4.

const FRAME_KEY_BYTES: u32 = 18; // store.md 8.6: the places key and an eight-byte offset.
const FRAMING_BYTES: usize = 12; // store.md 8.1: length, checksum, length.
const MARK: &str = "patch 1\nowner fixture-smith 1\nmark fresh-windows\n";
const FOLLOW: &str = "patch 1\nowner fixture-smith 2\nmark owner-private\n";
const SMITH: PatchSet = patches::FRESH_BEFORE.sets[1];
const MARKED: PatchSets = PatchSets { sets: &[patches::FRESH_BEFORE.sets[0], PatchSet { patches: &[MARK], ..SMITH }] };
const FOLLOWED: PatchSets =
    PatchSets { sets: &[patches::FRESH_BEFORE.sets[0], PatchSet { patches: &[MARK, FOLLOW], ..SMITH }] };
const WITHOUT_WORD: PatchSets =
    PatchSets { sets: &[patches::FRESH_BEFORE.sets[0], PatchSet { fresh: None, patches: &[MARK, FOLLOW], ..SMITH }] };
const ANOTHER_WORD: PatchSets = PatchSets {
    sets: &[patches::FRESH_BEFORE.sets[0], PatchSet { fresh: Some("another-word"), patches: &[MARK, FOLLOW], ..SMITH }],
};
const OTHER_OWNER: PatchSets = PatchSets {
    sets: &[
        PatchSet { fresh: Some(jig_test_patches_smith::FRESH), ..patches::FRESH_BEFORE.sets[0] },
        PatchSet { fresh: None, patches: &[MARK, FOLLOW], ..SMITH },
    ],
};

const LAST_MARKED: PatchSets = PatchSets {
    sets: &[
        patches::FRESH_BEFORE.sets[0],
        PatchSet {
            patches: &["patch 1\nowner fixture-smith 1\n", "patch 1\nowner fixture-smith 2\nmark fresh-windows\n"],
            ..SMITH
        },
    ],
};
const CUSTOM_MARKED: PatchSets = PatchSets {
    sets: &[
        patches::FRESH_BEFORE.sets[0],
        PatchSet {
            fresh: Some("review-window"),
            patches: &["patch 1\nowner fixture-smith 1\nmark review-window\n", FOLLOW],
            ..SMITH
        },
    ],
};

fn proof(world: &mut World, task: u64) -> core::Record {
    world.get(core::Key::Core(core::CoreKey::RunProof(task)))
}
fn proofs(world: &mut World, crossed: bool) {
    for (index, window) in patches::FRESH_WINDOWS.iter().enumerate() {
        let task = root::LIVE + u64::try_from(index).expect("task index");
        assert_eq!(proof(world, task), patches::fresh_proof(task, *window, crossed), "task {task}");
    }
}
fn transcripts(world: &mut World) -> BTreeMap<Vec<u8>, Option<Vec<u8>>> {
    world.contents().into_iter().filter(|(path, _)| path.starts_with(b"transcripts/")).collect()
}
fn open(world: &mut World, sets: PatchSets) -> u64 {
    world.patch_sets(sets);
    world.open_switched().unwrap_or_else(|| panic!("open refused: {:?}", world.refusal()))
}

#[test]
fn pending_marks_close_windows_without_family_changes() {
    for sets in [FOLLOWED, LAST_MARKED, CUSTOM_MARKED] {
        let mut world = patches::fresh_committed(SEED, patches::FRESH_WINDOWS, false);
        open(&mut world, sets);
        proofs(&mut world, true);
        assert_eq!(world.patched().len(), PENDING_PATCHES, "the closing walk answers no Patched");
        assert!(
            world
                .patched()
                .iter()
                .all(|(owner, _, counts)| { *owner == Owner::Smith && counts.records == 0 && counts.commits == 0 })
        );
    }
}

#[test]
fn absent_other_and_other_owners_words_leave_windows_open() {
    for sets in [WITHOUT_WORD, ANOTHER_WORD, OTHER_OWNER] {
        let mut world = patches::fresh_committed(SEED, patches::FRESH_WINDOWS, false);
        open(&mut world, sets);
        proofs(&mut world, false);
    }
}

#[test]
fn applied_mark_does_not_close_a_later_window() {
    let mut world = patches::fresh_committed(SEED, patches::FRESH_WINDOWS, false);
    let last = open(&mut world, MARKED);
    world
        .commit(Box::new([
            Write::Save(root::header(last + 1)),
            Write::Save(patches::fresh_proof(root::LIVE, true, false)),
        ]))
        .expect("subsequent open window");
    world.crash(SEED);
    open(&mut world, FOLLOWED);
    assert_eq!(proof(&mut world, root::LIVE), patches::fresh_proof(root::LIVE, true, false));
    world.crash(SEED);
    open(&mut world, FOLLOWED);
    assert_eq!(proof(&mut world, root::LIVE), patches::fresh_proof(root::LIVE, true, false));
}

#[test]
fn closing_preserves_proofs_transcripts_and_the_generation_before() {
    let mut world = patches::fresh_committed(SEED, patches::FRESH_WINDOWS, false);
    let source = patches::source(&mut world);
    let frames = transcripts(&mut world);
    open(&mut world, patches::FRESH_AFTER);
    proofs(&mut world, true);
    patches::kept(&source, &patches::source(&mut world), &"closing");
    assert_eq!(transcripts(&mut world), frames);
    patches::whole(&mut world);
    world.crash(SEED);
    let restored = world.reopen();
    Referee::fresh_windows(&restored).expect("restart retains closed windows");
    proofs(&mut world, true);
    assert_eq!(transcripts(&mut world), frames);
}

#[test]
fn rollback_and_forward_repeat_the_close_from_untouched_proofs() {
    let mut world = patches::fresh_committed(SEED, patches::FRESH_WINDOWS, false);
    let frames = transcripts(&mut world);
    open(&mut world, patches::FRESH_AFTER);
    world.rewrite(b"active", b"1\n");
    world.crash(SEED);
    open(&mut world, patches::FRESH_BEFORE);
    proofs(&mut world, false);
    assert!(!world.paths().iter().any(|path| path.starts_with(b"generations/2")));
    world.crash(SEED);
    open(&mut world, patches::FRESH_AFTER);
    proofs(&mut world, true);
    assert_eq!(transcripts(&mut world), frames);
}

fn encoded(record: &core::Record) -> Box<[u8]> {
    let limits = codec::limits(&patches::fresh_limits().core).expect("binary codec limits");
    codec::encode(record, &limits, &[]).expect("binary record")
}
fn contains(bytes: &[u8], record: &[u8]) -> bool {
    skein_lib::bytes::find(bytes, record).is_some()
}

#[test]
fn closing_pages_have_own_commits_before_the_seal() {
    let mut world = patches::fresh_committed(SEED, patches::FRESH_WINDOWS, false);
    world.trace_writes();
    let last = open(&mut world, FOLLOWED);
    let mut closing = Vec::new();
    for (index, has_window) in patches::FRESH_WINDOWS.iter().enumerate() {
        let task = root::LIVE + u64::try_from(index).expect("task index");
        let bytes = encoded(&patches::fresh_proof(task, *has_window, true));
        let writes: Vec<_> = world.written().iter().filter(|(_, write)| contains(write, &bytes)).collect();
        assert_eq!(writes.len(), 1, "task {task} written once by the close");
        closing.push(writes[0].0);
    }
    closing.sort_unstable();
    closing.dedup();
    // Six proofs at fresh_limits' four writes per page require two commits.
    assert_eq!(closing.len(), CLOSING_PAGES);
    let header = encoded(&root::header(last));
    let seal: Vec<_> = world.written().iter().filter(|(_, write)| contains(write, &header)).collect();
    assert_eq!(seal.len(), 1);
    assert!(closing.iter().all(|operation| *operation < seal[0].0));
    let source_last = u64::try_from(patches::FRESH_WINDOWS.len()).expect("tasks") * COMMITS_PER_TASK;
    assert_eq!(
        last,
        source_last + u64::try_from(CLOSING_PAGES).expect("page count") + SEAL_COMMITS,
        "two closing commits, then the seal"
    );
}

#[test]
fn uncrossed_closed_and_absent_windows_are_written_crossed() {
    const WINDOWS: &[bool] = &[false, false, false, false, true, true];
    let mut world = patches::fresh_committed(SEED, WINDOWS, false);
    let mut last = open(&mut world, patches::FRESH_BEFORE);
    for (index, window) in WINDOWS.iter().enumerate() {
        let task = root::LIVE + u64::try_from(index).expect("task index");
        last += 1;
        let core::Record::Core(core::CoreRecord::RunProof(mut kept)) = patches::fresh_proof(task, *window, true) else {
            panic!("proof fixture");
        };
        kept.crossed = false;
        world
            .commit(Box::new([
                Write::Save(root::header(last)),
                Write::Save(core::Record::Core(core::CoreRecord::RunProof(kept))),
            ]))
            .expect("uncrossed, already closed fixture");
    }
    world.crash(SEED);
    world.trace_writes();
    assert_eq!(
        open(&mut world, FOLLOWED),
        last + u64::try_from(CLOSING_PAGES).expect("closing pages") + SEAL_COMMITS,
        "uncrossed closed and windowless proofs are written, then the seal"
    );
    for (index, window) in WINDOWS.iter().enumerate() {
        let task = root::LIVE + u64::try_from(index).expect("task index");
        let expected = patches::fresh_proof(task, *window, true);
        assert_eq!(proof(&mut world, task), expected);
        let bytes = encoded(&expected);
        assert_eq!(
            world.written().iter().filter(|(_, write)| contains(write, &bytes)).count(),
            1,
            "task {task} crossed once"
        );
    }
}

#[test]
fn power_loss_at_each_closing_commit_and_seal_recovers_whole() {
    let mut traced = patches::fresh_committed(SEED, patches::FRESH_WINDOWS, false);
    traced.trace_writes();
    let last = open(&mut traced, patches::FRESH_AFTER);
    let closed: Vec<_> = patches::FRESH_WINDOWS
        .iter()
        .enumerate()
        .map(|(index, window)| {
            encoded(&patches::fresh_proof(root::LIVE + u64::try_from(index).expect("task"), *window, true))
        })
        .chain(std::iter::once(encoded(&root::header(last))))
        .collect();
    let writes: Vec<_> = traced
        .written()
        .iter()
        .filter(|(_, bytes)| closed.iter().any(|record| contains(bytes, record)))
        .map(|(operation, _)| *operation)
        .collect();
    assert_eq!(
        writes.len(),
        CLOSING_PAGES + usize::try_from(SEAL_COMMITS).expect("seal"),
        "two closing pages and the sealing commit"
    );
    let mut cuts = Vec::new();
    for write in writes {
        let sync = traced
            .changes()
            .iter()
            .find(|(operation, change)| *operation > write && *change == Change::Sync)
            .expect("commit sync")
            .0;
        cuts.extend([write, write + 1, sync, sync + 1]);
    }
    cuts.sort_unstable();
    cuts.dedup();
    for cut in cuts {
        let mut world = patches::fresh_committed(SEED, patches::FRESH_WINDOWS, false);
        let frames = transcripts(&mut world);
        let source = patches::source(&mut world);
        world.trace_writes();
        world.cut_at(cut);
        world.patch_sets(patches::FRESH_AFTER);
        assert_eq!(world.open_switched(), None, "cut {cut} reached");
        world.crash(SEED);
        assert_eq!(world.active(), b"1\n", "cut {cut} precedes switch");
        // The source binary must still open every window as it stood.
        open(&mut world, patches::FRESH_BEFORE);
        proofs(&mut world, false);
        patches::kept(&source, &patches::source(&mut world), &cut);
        world.crash(SEED);
        open(&mut world, patches::FRESH_AFTER);
        proofs(&mut world, true);
        patches::whole(&mut world);
        assert_eq!(transcripts(&mut world), frames, "cut {cut}");
    }
}

#[test]
fn serving_shell_answers_sites_for_closed_window_reads() {
    let mut world = patches::fresh_committed(SEED, patches::FRESH_WINDOWS, false);
    open(&mut world, patches::FRESH_AFTER);
    let frames = transcripts(&mut world);
    let mut reads = vec![
        (TranscriptRead::Window { task: root::LIVE, attempt: 1, turn: 1, after: None }, TURNS_PER_TASK),
        (TranscriptRead::Turn { task: root::LIVE, attempt: 1, turn: 1 }, 1),
        (TranscriptRead::Page { task: root::LIVE, backwards: false, after: None }, TURNS_PER_TASK),
        (TranscriptRead::Page { task: root::LIVE, backwards: true, after: None }, TURNS_PER_TASK),
    ];
    // Reverse order reads cannot depend on another frame's context.
    reads.reverse();
    for (read, count) in reads {
        let turns = world.transcript(read);
        assert_eq!(turns.len(), count, "each written site is answered while serving");
        for turn in turns {
            let key_tokens = u32::try_from(patches::FRESH_REPLAY.len()).expect("replay") + FRAME_KEY_BYTES;
            assert_eq!(turn.transcript, patches::fixture_turn(2, patches::FRESH_SAID, &[], key_tokens));
        }
    }
    proofs(&mut world, true);
    assert_eq!(transcripts(&mut world), frames);
}

#[test]
fn patched_frame_charge_holds_at_bound_and_refuses_one_below() {
    let mut world = patches::fresh_committed(SEED, patches::FRESH_WINDOWS, false);
    open(&mut world, patches::FRESH_AFTER);
    let patched = patches::fixture_turn(
        2,
        patches::FRESH_SAID,
        &[],
        FRAME_KEY_BYTES + u32::try_from(patches::FRESH_REPLAY.len()).expect("replay size"),
    );
    let size =
        u64::try_from(jig_test_patches_jig::frames::body(6, &patched).len() + FRAMING_BYTES).expect("frame size");
    for (bytes, expected) in [(size, 1), (size - 1, 0)] {
        let events = world.answered(Request::Transcript {
            owner: Token::new(719),
            read: TranscriptRead::Window { task: root::LIVE, attempt: 1, turn: 1, after: None },
            rows: 1,
            bytes,
        });
        if expected == 0 {
            assert!(
                matches!(events.as_slice(), [Event::Unread { owner }] if *owner == Token::new(719)),
                "patched frame charge {size}, budget {bytes}: {events:?}"
            );
        } else {
            let [Event::Turns { turns, .. }] = events.as_slice() else {
                panic!("bounded read: {events:?}");
            };
            assert_eq!(turns.len(), expected, "patched frame charge {size}, budget {bytes}");
        }
    }
}

#[test]
fn closing_changes_only_the_two_flags_in_a_populated_proof() {
    let mut world = patches::fresh_committed(SEED, patches::FRESH_WINDOWS, false);
    let last = open(&mut world, patches::FRESH_BEFORE);
    let core::Record::Core(core::CoreRecord::RunProof(mut kept)) = patches::fresh_proof(root::LIVE, true, false) else {
        panic!("proof fixture");
    };
    kept.conversation = 3;
    kept.attempt = 5;
    kept.window = Some(core::CurrentWindow { attempt: 4, turn: 7, closed: false });
    kept.offered = Some(17);
    kept.outstanding = Box::new([13, 17]);
    kept.turn = Some(core::TurnProof { turn: 9, cumulative: 11, read: Some(13) });
    world
        .commit(Box::new([
            Write::Save(root::header(last + 1)),
            Write::Save(core::Record::Core(core::CoreRecord::RunProof(kept.clone()))),
        ]))
        .expect("populated proof");
    world.crash(SEED);
    open(&mut world, FOLLOWED);
    kept.window.as_mut().expect("kept window").closed = true;
    kept.crossed = true;
    assert_eq!(proof(&mut world, root::LIVE), core::Record::Core(core::CoreRecord::RunProof(kept)));
}

#[test]
fn closing_follows_every_owners_last_patch_page() {
    const BEFORE: PatchSets = PatchSets { sets: &[patches::BASE.sets[0], patches::BASE.sets[1], SMITH] };
    const AFTER: PatchSets =
        PatchSets { sets: &[patches::SETS.sets[0], patches::SETS.sets[1], PatchSet { patches: &[MARK], ..SMITH }] };
    let records = patches::Records::story();
    let mut world = records.committed_with(SEED, BEFORE, 1, patches::limits(), &[]);
    let mut number = open(&mut world, BEFORE);
    for (index, window) in patches::FRESH_WINDOWS.iter().enumerate() {
        let task = root::LIVE + u64::try_from(index).expect("task index");
        number += 1;
        world
            .commit(Box::new([
                Write::Save(root::header(number)),
                Write::Save(root::live_task(task)),
                Write::Save(patches::fresh_proof(task, *window, false)),
            ]))
            .expect("one live task and proof fit a commit");
    }
    world.restart_with_limits(SEED, patches::fresh_limits());
    world.trace_writes();
    let last = open(&mut world, AFTER);
    let patch_rows: Vec<_> = records.rows(2).into_values().collect();
    let last_patch_write = world
        .written()
        .iter()
        .filter(|(_, bytes)| patch_rows.iter().any(|row| contains(bytes, row)))
        .map(|(operation, _)| *operation)
        .max()
        .expect("patch pages written");
    let closed_rows: Vec<_> = patches::FRESH_WINDOWS
        .iter()
        .enumerate()
        .map(|(index, window)| {
            encoded(&patches::fresh_proof(root::LIVE + u64::try_from(index).expect("task"), *window, true))
        })
        .collect();
    let first_close = world
        .written()
        .iter()
        .filter(|(_, bytes)| closed_rows.iter().any(|row| contains(bytes, row)))
        .map(|(operation, _)| *operation)
        .min()
        .expect("closing pages written");
    assert!(last_patch_write < first_close, "closing runs after the final owner's patched page");
    let seal = encoded(&root::header(last));
    let seal_write = world.written().iter().find(|(_, bytes)| contains(bytes, &seal)).expect("seal").0;
    assert!(first_close < seal_write);
    proofs(&mut world, true);
    records.check(&mut world, 2, &"after closing");
}

#[test]
fn power_loss_around_switch_recovers_all_windows_closed() {
    let mut traced = patches::fresh_committed(SEED, patches::FRESH_WINDOWS, false);
    traced.trace_writes();
    open(&mut traced, patches::FRESH_AFTER);
    let rename = traced.changes().iter().rfind(|(_, change)| *change == Change::Rename).expect("switch rename").0;
    let sync = traced
        .changes()
        .iter()
        .find(|(operation, change)| *operation > rename && *change == Change::Sync)
        .expect("switch sync")
        .0;
    for cut in [rename, rename + 1, sync, sync + 1] {
        let mut world = patches::fresh_committed(SEED, patches::FRESH_WINDOWS, false);
        let frames = transcripts(&mut world);
        world.trace_writes();
        world.cut_at(cut);
        world.patch_sets(patches::FRESH_AFTER);
        assert_eq!(world.open_switched(), None, "switch cut {cut} reached");
        world.crash(SEED);
        let active = world.active();
        assert!([b"1\n".as_slice(), b"2\n".as_slice()].contains(&active.as_slice()), "whole active: {cut}");
        open(&mut world, patches::FRESH_AFTER);
        proofs(&mut world, true);
        patches::whole(&mut world);
        assert_eq!(transcripts(&mut world), frames, "switch cut {cut}");
    }
}
