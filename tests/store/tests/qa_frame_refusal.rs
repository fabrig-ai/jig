//! Independent contract checks for protocol/store.md 6, 8.1, 8.2, 8.6 and 13.
#[global_allocator]
static HEAP: skein_heap::Counting = skein_heap::Counting;

use jig_domain_core as core;
use jig_protocol_store::{Event, Request, TranscriptRead, Write, sizes};
use jig_store_world::{World, patches, root};
use skein_lib::Token;

const SEED: u64 = 0x0051_414d_3162;
const READER: Token = Token::new(714);
const TASK: u64 = root::LIVE;
const ATTEMPT: u64 = 1;
const PRIOR: &[u8] = b"durable before refusal";
const PAYLOAD: [u8; 192] = [b'x'; 192];
// store.md 8.1 declares transcript: bytes 8388608 in the current codec.
const CODEC_TRANSCRIPT_CEILING: u32 = 8_388_608;
const PROBE_PAGE: u32 = 1;
const FIRST: u64 = 1;
const DURABLE: u64 = 2;
const REFUSED: u64 = 3;

fn opened(page: u32) -> World {
    let mut limits = patches::limits();
    limits.store.page.bytes = page;
    let mut world = World::unopened_observed(SEED, limits, None);
    assert_eq!(world.open(), Some(0), "open: {:?}", world.refusal());
    world
}

fn refused(events: &[Event], number: u64, task: u64, limit: u32) -> u32 {
    let [Event::FrameTooLarge { number: actual, task: actual_task, bytes, limit: actual_limit }] = events else {
        panic!("one named refusal, without Failed or Committed: {events:?}");
    };
    assert_eq!((*actual, *actual_task, *actual_limit), (number, task, limit), "{events:?}");
    assert!(*bytes > limit, "refusal must name the full oversized frame: {events:?}");
    eprintln!("terminal: {events:?}");
    *bytes
}

fn frame_bytes(transcript: &[u8]) -> u32 {
    let mut world = opened(PROBE_PAGE);
    let events =
        world.request(Request::Commit { number: FIRST, writes: root::turn_commit(FIRST, TASK, 1, transcript) });
    refused(&events, FIRST, TASK, PROBE_PAGE)
}

fn baseline(page: u32) -> World {
    let mut world = opened(page);
    assert_eq!(world.commit(root::first()), Some(FIRST));
    assert_eq!(world.commit(root::turn_commit(DURABLE, TASK, 1, PRIOR)), Some(DURABLE));
    world
}

fn recovered(world: &mut World) -> jig_store_world::world::Restored {
    let before = world.contents();
    world.crash(SEED + 1);
    assert!(world.contents() == before, "power loss changed durable files");
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| world.reopen()));
    let after = world.contents();
    for (path, bytes) in &after {
        if before.get(path) != Some(bytes) {
            eprintln!(
                "reopen changed {}: previous length {:?}, current length {:?}",
                String::from_utf8_lossy(path),
                before.get(path).and_then(Option::as_ref).map(Vec::len),
                bytes.as_ref().map(Vec::len)
            );
        }
    }
    for path in before.keys().filter(|path| !after.contains_key(*path)) {
        eprintln!("reopen removed {}", String::from_utf8_lossy(path));
    }
    eprintln!(
        "all preexisting paths and bytes unchanged: {}",
        before.iter().all(|(path, bytes)| after.get(path) == Some(bytes))
    );
    let restored = result.unwrap_or_else(|panic| std::panic::resume_unwind(panic));
    eprintln!("recovered commit {}, task {TASK} turns: {:?}", restored.last, restored.turns(TASK));
    assert!(after == before, "reopen changed physical contents; see path diagnostics");
    restored
}

fn starts() -> [TranscriptRead; 3] {
    [
        TranscriptRead::Window { task: TASK, attempt: ATTEMPT, turn: 1, after: None },
        TranscriptRead::Page { task: TASK, backwards: false, after: None },
        TranscriptRead::Page { task: TASK, backwards: true, after: None },
    ]
}

fn read(world: &mut World, read: TranscriptRead, bytes: u32) -> Vec<Event> {
    world.request(Request::Transcript { owner: READER, read, rows: 1, bytes: u64::from(bytes) })
}

fn whole(events: &[Event], expected: &core::TurnRecord) {
    let [Event::Turns { owner, turns, next: None }] = events else {
        panic!("one complete turn: {events:?}");
    };
    assert_eq!(*owner, READER);
    assert_eq!(turns.as_ref(), std::slice::from_ref(expected));
}

#[test]
fn a_frame_exactly_the_page_commits_and_reads_back_whole() {
    let bytes = frame_bytes(&PAYLOAD);
    let mut world = opened(bytes);
    assert_eq!(world.commit(root::first()), Some(FIRST));
    let events =
        world.request(Request::Commit { number: DURABLE, writes: root::turn_commit(DURABLE, TASK, 1, &PAYLOAD) });
    assert!(matches!(events.as_slice(), [Event::Committed { number: DURABLE, .. }]), "{events:?}");
    eprintln!("terminal: {events:?}; exact frame reads whole in all four read kinds");
    let expected = root::turn(TASK, ATTEMPT, 1, &PAYLOAD);
    for kind in starts().into_iter().chain([TranscriptRead::Turn { task: TASK, attempt: ATTEMPT, turn: 1 }]) {
        whole(&read(&mut world, kind, bytes), &expected);
    }
    let restored = recovered(&mut world);
    assert_eq!(restored.last, DURABLE);
    assert_eq!(restored.turns(TASK), &[expected]);
}

#[test]
fn a_frame_one_byte_past_the_page_is_refused_by_name() {
    let bytes = frame_bytes(&PAYLOAD);
    let mut world = baseline(bytes - 1);
    let before = world.contents();
    world.trace_writes();
    let events =
        world.request(Request::Commit { number: REFUSED, writes: root::turn_commit(REFUSED, TASK, 2, &PAYLOAD) });
    assert_eq!(refused(&events, REFUSED, TASK, bytes - 1), bytes);
    assert!(world.written().is_empty(), "refused commit wrote data: {:?}", world.written());
    assert!(world.changes().is_empty(), "refused commit changed files: {:?}", world.changes());
    assert_eq!(world.contents(), before);
    let restored = recovered(&mut world);
    assert_eq!(restored.last, DURABLE);
    assert_eq!(restored.turns(TASK), &[root::turn(TASK, ATTEMPT, 1, PRIOR)]);
    assert_eq!(world.contents(), before, "crash and recovery preserve every file byte");
}

#[test]
fn a_later_turn_past_the_page_refuses_its_whole_commit() {
    let bytes = frame_bytes(&PAYLOAD);
    let mut world = baseline(bytes - 1);
    let before = world.contents();
    let mut writes = root::turn_commit(REFUSED, TASK, 2, PRIOR).into_vec();
    writes.push(Write::Put { key: Box::from(b"refused".as_slice()), value: Box::from(b"never stored".as_slice()) });
    writes.push(Write::Save(core::Record::Core(core::CoreRecord::Turn(root::turn(TASK + 1, ATTEMPT, 1, &PAYLOAD)))));
    world.trace_writes();
    let events = world.request(Request::Commit { number: REFUSED, writes: writes.into_boxed_slice() });
    assert_eq!(refused(&events, REFUSED, TASK + 1, bytes - 1), bytes);
    assert!(world.written().is_empty(), "earlier fitting data written: {:?}", world.written());
    assert!(world.changes().is_empty(), "refused batch changed files: {:?}", world.changes());
    assert_eq!(world.contents(), before);
    let restored = recovered(&mut world);
    assert_eq!(restored.last, DURABLE);
    assert_eq!(restored.turns(TASK), &[root::turn(TASK, ATTEMPT, 1, PRIOR)]);
    assert!(restored.application.is_empty());
    assert!(world.transcript(TranscriptRead::Page { task: TASK + 1, backwards: false, after: None }).is_empty());
    assert_eq!(world.contents(), before);
}

#[test]
fn a_pending_commit_fails_once_when_a_later_frame_is_refused() {
    let bytes = frame_bytes(&PAYLOAD);
    let mut world = baseline(bytes - 1);
    world.trace_writes();
    world.cut_at(world.operation_count());
    let pending =
        world.request(Request::Commit { number: REFUSED, writes: root::turn_commit(REFUSED, TASK, 2, PRIOR) });
    assert!(pending.is_empty(), "commit must be pending: {pending:?}");
    let number = REFUSED + 1;
    let events = world.request(Request::Commit { number, writes: root::turn_commit(number, TASK + 1, 1, &PAYLOAD) });
    let [Event::FrameTooLarge { number: actual, task, bytes: size, limit }, Event::Failed { number: pending }] =
        events.as_slice()
    else {
        panic!("refusal first, then exactly one failed terminal for the pending commit: {events:?}");
    };
    assert_eq!((*actual, *task, *size, *limit), (number, TASK + 1, bytes, bytes - 1));
    assert_eq!(*pending, REFUSED);
}

#[test]
fn the_frame_bound_covers_each_transcript_size() {
    let core = root::CORE_LIMITS;
    let ceiling = CODEC_TRANSCRIPT_CEILING;
    // The ceiling is checked without allocating its transcript: this QA
    // task forbids megabyte-scale fixtures. The actual-frame table stays small.
    let mut sizes = vec![0, 1, 127, 128, 16_383, 16_384, core.resume_bytes];
    sizes.retain(|size| *size <= ceiling);
    sizes.sort_unstable();
    sizes.dedup();
    let mut previous = None;
    for size in sizes {
        let bound = sizes::frame(&core, size).unwrap_or_else(|| panic!("no frame bound for admitted size {size}"));
        let transcript = vec![b'x'; usize::try_from(size).expect("small fixture")];
        let actual = frame_bytes(&transcript);
        assert!(actual <= bound, "transcript {size}: refused frame {actual} exceeds bound {bound}");
        if let Some(previous) = previous {
            assert!(bound > previous, "frame bound did not grow at transcript {size}: {previous} -> {bound}");
        }
        previous = Some(bound);
    }
    let ceiling_bound = sizes::frame(&core, ceiling).expect("codec ceiling has a frame bound");
    assert!(ceiling_bound > previous.expect("nonempty transcript table"));
    assert_eq!(sizes::frame(&core, ceiling + 1), None, "past the codec's transcript ceiling");
    assert_eq!(sizes::frame(&core, u32::MAX), None, "no clamping or overflow");
}

#[test]
fn a_read_one_byte_short_is_unread_and_an_exact_read_recovers() {
    let bytes = frame_bytes(&PAYLOAD);
    let mut world = opened(bytes);
    assert_eq!(world.commit(root::first()), Some(FIRST));
    assert_eq!(world.commit(root::turn_commit(DURABLE, TASK, 1, &PAYLOAD)), Some(DURABLE));
    for kind in starts() {
        let events = read(&mut world, kind, bytes - 1);
        assert!(matches!(events.as_slice(), [Event::Unread { owner }] if *owner == READER), "{kind:?}: {events:?}");
        whole(&read(&mut world, kind, bytes), &root::turn(TASK, ATTEMPT, 1, &PAYLOAD));
    }
}

#[test]
fn the_durable_baseline_reopens_without_any_refused_commit() {
    // A control for the recovery stories: the same page and rows, no refusal.
    let bytes = frame_bytes(&PAYLOAD);
    let mut world = baseline(bytes - 1);
    let before = world.contents();
    let restored = recovered(&mut world);
    assert_eq!(restored.last, DURABLE);
    assert_eq!(restored.turns(TASK), &[root::turn(TASK, ATTEMPT, 1, PRIOR)]);
    assert_eq!(world.contents(), before);
}

#[test]
fn a_refusal_preserves_a_baseline_without_the_live_task_row() {
    // Diagnostic companion to the unchanged root::first() recovery stories.
    // A header and proof suffice to name and recover the earlier transcript.
    let bytes = frame_bytes(&PAYLOAD);
    let mut world = opened(bytes - 1);
    assert_eq!(
        world.commit(Box::new([Write::Save(root::header(FIRST)), Write::Save(root::run_proof(TASK))])),
        Some(FIRST)
    );
    assert_eq!(world.commit(root::turn_commit(DURABLE, TASK, 1, PRIOR)), Some(DURABLE));
    let before = world.contents();
    world.trace_writes();
    let events =
        world.request(Request::Commit { number: REFUSED, writes: root::turn_commit(REFUSED, TASK, 2, &PAYLOAD) });
    assert_eq!(refused(&events, REFUSED, TASK, bytes - 1), bytes);
    assert!(world.written().is_empty());
    assert!(world.changes().is_empty());
    assert_eq!(world.contents(), before);
    let restored = recovered(&mut world);
    assert_eq!(restored.last, DURABLE);
    assert_eq!(restored.turns(TASK), &[root::turn(TASK, ATTEMPT, 1, PRIOR)]);
    assert_eq!(world.contents(), before);
}
