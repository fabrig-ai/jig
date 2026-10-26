//! Stopped-engine tools over fixture owners' levels (store.md, 10.5 and 14).
use jig_protocol_store::{
    BackRefusal, Counts, Event, KeptWire, Levels, OpenRefusal, Owner, Request, Secrets, Wire, Write, levels,
};
use jig_store_world::{
    World,
    patches::{self, BASE, FRESH_AFTER, FRESH_BEFORE, Records, SETS},
    root,
};
use skein_lib::Token;

const SEED: u64 = 970;
const TOOL: Token = Token::new(971);
const WIRES: &[Wire] = &[Wire { name: "channel", version: 3 }, Wire { name: "payload", version: 7 }];
// Records::story: one initial commit, four public patch pages and the seal.
const PATCHED_LAST: u64 = 6;
const RESTORED_LAST: u64 = PATCHED_LAST + 1;
const RESTORED_KEY: &[u8] = b"rehearsed";
// fresh_committed keeps two fixture turns per task; a single-turn read returns one.
const KEPT_TURNS: usize = 2;
const SINGLE_TURN: usize = 1;

fn source(wires: bool) -> World {
    let records = Records::story();
    let mut world = World::unopened_observed(SEED, patches::limits(), None);
    world.patch_sets(BASE);
    world.wires(WIRES);
    world.open().expect("first open");
    let writes = records
        .writes(1)
        .into_vec()
        .into_iter()
        .filter(|write| wires || !matches!(write, Write::Wires))
        .collect::<Vec<_>>();
    world.commit(writes.into_boxed_slice()).expect("source commit");
    world.crash(SEED);
    world.patch_sets(SETS);
    world
}
fn rehearsed() -> World {
    let mut world = source(true);
    world.rehearse().expect("scratch opens");
    world
}
fn patched() -> World {
    let mut world = source(true);
    world.open().expect("pending open");
    world.switch().expect("switched without mark");
    world.crash(SEED);
    world
}
fn closed(world: &mut World) {
    assert!(matches!(world.close().as_slice(), [Event::Closed { removed: true, .. }]));
    assert!(world.scratch_paths().is_empty(), "scratch deployment removed");
}

#[test]
fn a_rehearsal_reports_each_patch_and_leaves_the_deployment_unchanged() {
    let mut world = source(true);
    let before = world.contents();
    let last = world.rehearse();
    let kept: Box<[KeptWire]> =
        Box::new([KeptWire { name: "channel".into(), version: 3 }, KeptWire { name: "payload".into(), version: 7 }]);
    assert_eq!(world.rehearsing, Some((Some(kept), false)));
    assert_eq!(
        world.patched(),
        [
            (Owner::Application, 1, Counts { records: 5, commits: 2 }),
            (Owner::Jig, 1, Counts { records: 2, commits: 1 }),
            (Owner::Application, 2, Counts { records: 3, commits: 1 }),
            (Owner::Jig, 2, Counts { records: 0, commits: 0 }),
        ]
    );
    assert_eq!(last, Some(PATCHED_LAST));
    assert!(world.pending);
    let next = last.expect("opened") + 1;
    world.commit(Box::new([
        Write::Save(root::header(next)),
        Write::Put { key: RESTORED_KEY.into(), value: b"scratch".as_slice().into() },
    ]));
    assert!(world.application(false).contains_key(RESTORED_KEY));
    let events = world.request(Request::Switch { owner: TOOL });
    assert!(matches!(events.as_slice(), [Event::OpenRefused { owner: TOOL, why: OpenRefusal::Failed }]));
    closed(&mut world);
    assert_eq!(world.contents(), before);
}

#[test]
fn a_rehearsal_over_a_store_that_keeps_no_wires_says_so() {
    let mut world = source(false);
    world.rehearse();
    assert_eq!(world.rehearsing, Some((None, false)));
    closed(&mut world);
}

#[test]
fn a_rehearsal_reports_the_plans_fresh_mark_and_never_cuts_transcripts() {
    let mut world = patches::fresh_committed(SEED, &[true], false);
    world.patch_sets(FRESH_AFTER);
    let before = world.contents();
    world.rehearse().expect("fresh rehearsal");
    assert_eq!(world.rehearsing, Some((None, true)));
    assert_eq!(
        world.transcript(jig_protocol_store::TranscriptRead::Turn { task: root::LIVE, attempt: 1, turn: 1 }).len(),
        SINGLE_TURN
    );
    closed(&mut world);
    assert_eq!(world.contents(), before);
}

#[test]
fn a_rehearsal_meets_the_refusal_a_start_would() {
    let mut records = Records::story();
    records.shelves[1].books = 0;
    let mut world = records.committed(SEED, BASE);
    world.patch_sets(SETS);
    world.open();
    let refusal = world.refusal().cloned();
    world.crash(SEED);
    let before = world.contents();
    assert_eq!(world.rehearse(), None);
    assert_eq!(world.refusal(), refusal.as_ref());
    closed(&mut world);
    assert_eq!(world.contents(), before);
}

#[test]
fn the_next_rehearsal_removes_a_crashed_rehearsals_scratch() {
    let mut world = rehearsed();
    world.commit(Box::new([
        Write::Save(root::header(RESTORED_LAST)),
        Write::Put { key: RESTORED_KEY.into(), value: b"lost".as_slice().into() },
    ]));
    world.crash(SEED);
    world.rehearse().expect("fresh scratch");
    assert!(!world.application(false).contains_key(RESTORED_KEY));
    closed(&mut world);
}

#[test]
fn a_failed_removal_leaves_the_rest_to_the_next_rehearsal() {
    let mut world = rehearsed();
    world.fail_next_removal();
    assert!(matches!(world.close().as_slice(), [Event::Closed { removed: false, .. }]));
    assert!(!world.scratch_paths().is_empty());
    world.crash(SEED);
    world.rehearse().expect("partial scratch removed first");
    closed(&mut world);
}

#[test]
fn a_rehearsal_with_nothing_pending_opens_the_scratchs_active_generation() {
    let mut world = source(true);
    world.patch_sets(BASE);
    let before = world.contents();
    assert_eq!(world.rehearse(), Some(1));
    assert!(!world.pending);
    assert!(world.patched().is_empty());
    let events = world.secret(Secrets::Get { owner: jig_protocol_store::SecretOwner::Application, number: 1 });
    assert!(matches!(events.as_slice(), [Event::SecretsNotReady { .. }]));
    closed(&mut world);
    assert_eq!(world.contents(), before);
}

#[test]
fn going_back_before_the_first_commit_opens_the_old_generation_as_it_was() {
    let mut world = patched();
    assert!(matches!(world.back().as_slice(), [Event::Back { to: 1, .. }]));
    assert_eq!(world.active(), b"1\n");
    world.crash(SEED);
    world.patch_sets(BASE);
    assert_eq!(world.open(), Some(1));
    assert!(!world.pending);
    assert_eq!(world.application(false), Records::story().rows(1).into_iter().collect());
}

#[test]
fn going_back_is_refused_once_the_patched_generation_has_committed() {
    let mut world = patched();
    world.open().expect("active patched generation");
    world.mark().expect("first own commit");
    // Normalize recovery's empty next segment before the byte comparison.
    world.crash(SEED);
    world.open().expect("recover committed generation");
    world.crash(SEED);
    let before = world.contents();
    assert!(matches!(
        world.back().as_slice(),
        [Event::BackRefused { why: BackRefusal::Committed { manifest: PATCHED_LAST, last: RESTORED_LAST }, .. }]
    ));
    assert_eq!(world.contents(), before);
}

#[test]
fn going_back_is_refused_with_no_previous_generation() {
    let mut world = source(true);
    let before = world.contents();
    assert!(matches!(world.back().as_slice(), [Event::BackRefused { why: BackRefusal::NoPrevious, .. }]));
    assert_eq!(world.contents(), before);
}

#[test]
fn going_back_is_refused_while_another_process_holds_the_lock() {
    let mut world = patched();
    world.hold_deployment_lock();
    let before = world.contents();
    assert!(matches!(world.back().as_slice(), [Event::BackRefused { why: BackRefusal::Busy, .. }]));
    assert_eq!(world.contents(), before);
}

#[test]
fn the_binarys_levels_are_the_fixture_owners_levels() {
    for (sets, expected) in [
        (jig_protocol_store::PatchSets::NONE, Levels { skein: None, smith: None, jig: None, application: None }),
        (BASE, Levels { skein: None, smith: None, jig: Some(0), application: Some(0) }),
        (SETS, Levels { skein: None, smith: None, jig: Some(2), application: Some(2) }),
        (FRESH_BEFORE, Levels { skein: None, smith: Some(0), jig: Some(2), application: None }),
        (FRESH_AFTER, Levels { skein: None, smith: Some(1), jig: Some(2), application: None }),
    ] {
        assert_eq!(levels(&sets), expected);
    }
}

#[test]
fn a_close_while_the_rehearsal_is_opening_removes_its_scratch() {
    let mut world = source(true);
    let before = world.contents();
    world.begin_rehearsal();
    world.enqueue(Request::Close { owner: TOOL });
    let events = world.settle();
    assert!(events.iter().any(|event| matches!(event, Event::Closed { owner: TOOL, removed: true })));
    assert!(world.scratch_paths().is_empty());
    assert_eq!(world.contents(), before);
}

fn fresh_patched() -> World {
    let mut world = patches::fresh_committed(SEED, &[true], false);
    world.patch_sets(FRESH_AFTER);
    world.open().expect("fresh pending copy");
    world.switch().expect("switch before mark");
    world
}
fn tail_commit(number: u64) -> Box<[Write]> {
    let bytes = patches::fixture_turn(2, patches::FRESH_SAID, patches::FRESH_REPLAY, 0);
    root::turn_commit(number, root::LIVE, 3, &bytes)
}
fn tail_cut() -> u32 {
    let mut world = fresh_patched();
    let last = world.open_position();
    world.commit(tail_commit(last + 1));
    world.transcript_sync().expect("new frame synced before map commit")
}

#[test]
fn going_back_cuts_an_uncommitted_transcript_tail_to_the_old_generations_lengths() {
    let mut world = fresh_patched();
    let before: Vec<_> = world.contents().into_iter().filter(|(path, _)| path.starts_with(b"transcripts/")).collect();
    let last = world.open_position();
    world.cut_at(tail_cut());
    world.commit(tail_commit(last + 1));
    world.crash(SEED);
    assert!(matches!(world.back().as_slice(), [Event::Back { to: 1, .. }]));
    world.crash(SEED);
    world.patch_sets(FRESH_BEFORE);
    let restored = world.reopen();
    assert_eq!(restored.turns(root::LIVE).len(), KEPT_TURNS, "old generation's window restored");
    let after: Vec<_> = world.contents().into_iter().filter(|(path, _)| path.starts_with(b"transcripts/")).collect();
    assert_eq!(after, before);
}

#[test]
fn a_rehearsal_preserves_an_uncommitted_transcript_tail() {
    let mut world = fresh_patched();
    let last = world.open_position();
    world.cut_at(tail_cut());
    world.commit(tail_commit(last + 1));
    world.crash(SEED);
    let before = world.contents();
    world.rehearse().expect("rehearsal over active patched generation");
    closed(&mut world);
    assert_eq!(world.contents(), before);
}

#[test]
fn a_close_at_a_written_site_needs_no_answer_from_the_patcher() {
    let mut world = source(true);
    let before = world.contents();
    world.begin_rehearsal();
    let events = world.settle();
    assert!(events.iter().any(|event| matches!(event, Event::Site { .. })));
    let events = world.close();
    assert!(matches!(
        events.as_slice(),
        [Event::OpenRefused { why: OpenRefusal::Failed, .. }, Event::Closed { removed: true, .. }]
    ));
    assert!(world.scratch_paths().is_empty());
    assert_eq!(world.contents(), before);
}

#[test]
fn a_rehearsals_close_drains_its_restore_commit_first() {
    let mut world = rehearsed();
    let before = world.contents();
    world.enqueue(Request::Commit {
        number: RESTORED_LAST,
        writes: Box::new([
            Write::Save(root::header(RESTORED_LAST)),
            Write::Put { key: RESTORED_KEY.into(), value: b"scratch".as_slice().into() },
        ]),
    });
    world.enqueue(Request::Close { owner: TOOL });
    let events = world.settle();
    assert!(matches!(
        events.as_slice(),
        [Event::Committed { number: RESTORED_LAST, .. }, Event::Closed { owner: TOOL, removed: true }]
    ));
    assert!(world.scratch_paths().is_empty());
    assert_eq!(world.contents(), before);
}

#[test]
fn a_rehearsal_cannot_commit_a_mark_with_nothing_pending() {
    let mut world = source(true);
    world.patch_sets(BASE);
    let before = world.contents();
    world.rehearse().expect("no patches pending");
    let events =
        world.request(Request::Commit { number: 2, writes: Box::new([Write::Save(root::header(2)), Write::Wires]) });
    assert!(matches!(events.as_slice(), [Event::Failed { number: 2 }]));
    closed(&mut world);
    assert_eq!(world.contents(), before);
}

#[test]
fn going_back_refuses_unreadable_active_or_manifest_bytes() {
    for path in [b"active".as_slice(), b"generations/2/manifest".as_slice()] {
        let mut world = patched();
        world.rewrite(path, b"unreadable fixture");
        let before = world.contents();
        assert!(matches!(world.back().as_slice(), [Event::BackRefused { why: BackRefusal::Unreadable, .. }]));
        assert_eq!(world.contents(), before);
    }
}

#[test]
fn closing_an_engine_store_is_out_of_order() {
    let mut world = source(true);
    world.open().expect("ordinary pending open");
    let before = world.contents();
    assert!(matches!(world.close().as_slice(), [Event::OpenRefused { why: OpenRefusal::Failed, .. }]));
    assert_eq!(world.contents(), before);
}

#[test]
fn a_rehearsal_removes_its_child_and_preserves_the_scratch_roots_other_entries() {
    let mut world = source(true);
    world.lay_scratch(&[skein_fake_machine::Item::file(b"other-cache", b"keep this")]);
    let before = world.scratch_contents();
    world.rehearse().expect("scratch child opens");
    assert!(world.scratch_paths().iter().any(|path| path == b"rehearsal/active"));
    let events = world.close();
    assert!(
        matches!(events.as_slice(), [Event::Closed { removed: true, .. }]),
        "{events:?}; scratch paths {:?}",
        world.scratch_paths()
    );
    assert_eq!(world.scratch_contents(), before);
}

#[test]
fn a_close_after_an_early_open_refusal_removes_the_crashed_child() {
    let mut world = rehearsed();
    world.crash(SEED);
    world.rewrite(b"active", b"not a generation");
    let before = world.contents();
    assert_eq!(world.rehearse(), None);
    assert_eq!(world.refusal(), Some(&OpenRefusal::Generations { why: jig_protocol_store::Generations::Unreadable }));
    closed(&mut world);
    assert_eq!(world.contents(), before);
}
