//! Independent contract checks for protocol/store.md 6, 8.1, 8.2, 8.6 and 13.
#[global_allocator]
static HEAP: skein_heap::Counting = skein_heap::Counting;

use jig_domain_core as core;
use jig_protocol_store::{Event, Limits, Request, TranscriptRead, Write, sizes};
use jig_store_world::{World, root, world::Restored};
use skein_lib::Token;
use std::collections::BTreeMap;

const SEED: u64 = 0x0051_414d_3162;
const READER: Token = Token::new(714);
const TASK: u64 = root::LIVE;
const ATTEMPT: u64 = 1;
const PRIOR: &[u8] = b"durable before refusal";
// store.md 8.1 declares transcript: bytes 8388608 in the current codec.
const CODEC_TRANSCRIPT_CEILING: u32 = 8_388_608;
const FIRST: u64 = 1;
const DURABLE: u64 = 2;
const REFUSED: u64 = 3;
const KV_CHUNK_OVERHEAD: u32 = 20;

fn checked(limits: &Limits) -> Result<sizes::Sizes, sizes::SizeRefusal> {
    sizes::check(&limits.core, &root::APPLICATION, &limits.journal, &limits.store, limits.orphan_erases)
}

fn row_floor() -> u32 {
    let bound = checked(&root::limits()).expect("world fixture passes the shell's check");
    bound.key + bound.value
}

fn payload() -> Box<[u8]> {
    // The root fixture's row floor is about 323 KiB. Its frame is larger
    // still, allowing both boundary pages to pass the shell's size check.
    vec![b'x'; usize::try_from(row_floor()).expect("fixture row floor")].into_boxed_slice()
}

fn opened(page: u32) -> World {
    let mut limits = root::limits();
    limits.store.page.bytes = page;
    // The world's drivers must read a full frame in one chunk.
    limits.store.chunk = limits.store.chunk.max(page + KV_CHUNK_OVERHEAD);
    checked(&limits).expect("every opened world passes the shell's size check");
    let mut world = World::unopened_observed(SEED, limits, None);
    assert_eq!(world.open_switched(), Some(0), "open: {:?}", world.refusal());
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
    let page = row_floor();
    let mut world = opened(page);
    let events =
        world.request(Request::Commit { number: FIRST, writes: root::turn_commit(FIRST, TASK, 1, transcript) });
    refused(&events, FIRST, TASK, page)
}

fn baseline(page: u32) -> World {
    let mut world = opened(page);
    assert_eq!(world.commit(root::first()), Some(FIRST));
    assert_eq!(world.commit(root::turn_commit(DURABLE, TASK, 1, PRIOR)), Some(DURABLE));
    world
}

fn starts() -> [TranscriptRead; 3] {
    [
        TranscriptRead::Window { task: TASK, attempt: ATTEMPT, turn: 1, after: None },
        TranscriptRead::Page { task: TASK, backwards: false, after: None },
        TranscriptRead::Page { task: TASK, backwards: true, after: None },
    ]
}

fn kinds() -> [TranscriptRead; 4] {
    let [window, forwards, backwards] = starts();
    [TranscriptRead::Turn { task: TASK, attempt: ATTEMPT, turn: 1 }, window, forwards, backwards]
}

fn read(world: &mut World, read: TranscriptRead, bytes: u32) -> Vec<Event> {
    let events = world.request(Request::Transcript { owner: READER, read, rows: 1, bytes: u64::from(bytes) });
    for event in &events {
        if let Event::Turns { turns, next, .. } = event {
            eprintln!("{read:?}, room {bytes}: {} whole turns, next {next:?}", turns.len());
        } else {
            eprintln!("{read:?}, room {bytes}: {event:?}");
        }
    }
    events
}

fn whole(events: &[Event], expected: &core::TurnRecord) {
    let [Event::Turns { owner, turns, next: None }] = events else {
        panic!("one complete turn: {events:?}");
    };
    assert_eq!(*owner, READER);
    assert_eq!(turns.as_ref(), std::slice::from_ref(expected));
}

fn snapshot_rows(world: &mut World) -> Restored {
    // These are all three jig rows saved by this fixture's two commits.
    let rows = [root::header(DURABLE), root::live_task(TASK), root::run_proof(TASK)]
        .into_iter()
        .map(|record| {
            let record = world.get(record.key());
            (record.key(), record)
        })
        .collect();

    Restored { last: DURABLE, rows, application: world.application(false), transcripts: BTreeMap::new() }
}

fn snapshot(world: &mut World, room: u32) -> Restored {
    let mut before = snapshot_rows(world);
    let events = read(world, TranscriptRead::Page { task: TASK, backwards: false, after: None }, room);
    let [Event::Turns { owner, turns, next: None }] = events.as_slice() else {
        panic!("complete snapshot with ample decoded room: {events:?}");
    };
    assert_eq!(*owner, READER);
    before.transcripts.insert(TASK, turns.to_vec());
    before
}

fn recovered(world: &mut World, before: &Restored, room: u32) {
    // A reopen may add its own empty log: compare records and reads here.
    let restored = world.reopen();
    assert_eq!(restored.last, before.last);
    assert_eq!(restored.rows, before.rows);
    assert_eq!(restored.application, before.application);
    // Verify turns with explicit decoded room; the aggregate world's
    // convenience transcript snapshot has no room argument.
    let [expected] = before.turns(TASK) else { panic!("fixture keeps exactly one earlier turn") };
    let answers = kinds().map(|kind| read(world, kind, room));
    for events in answers {
        whole(&events, expected);
    }
    eprintln!("recovered commit {} with unchanged rows, application and turns", restored.last);
}

#[test]
fn a_frame_exactly_the_page_commits_and_reads_back_whole() {
    let payload = payload();
    let bytes = frame_bytes(&payload);
    let mut world = opened(bytes);
    assert_eq!(world.commit(root::first()), Some(FIRST));
    let events =
        world.request(Request::Commit { number: DURABLE, writes: root::turn_commit(DURABLE, TASK, 1, &payload) });
    assert!(matches!(events.as_slice(), [Event::Committed { number: DURABLE, .. }]), "{events:?}");
    let expected = root::turn(TASK, ATTEMPT, 1, &payload);
    eprintln!("terminal: {events:?}");
    // store.md 7.1: the page bounds encoded bytes, read room decoded bytes.
    let room = bytes * 2;
    eprintln!("before crash, decoded room {room}");
    let answers = kinds().map(|kind| read(&mut world, kind, room));
    let before = snapshot_rows(&mut world);
    world.crash(SEED + 1);
    let restored = world.reopen();
    assert_eq!(restored.last, before.last);
    assert_eq!(restored.rows, before.rows);
    assert_eq!(restored.application, before.application);
    eprintln!("after crash and reopen, unchanged records, decoded room {room}");
    let reopened = kinds().map(|kind| read(&mut world, kind, room));
    // Collect both phases before asserting so a read failure cannot hide
    // the crash/reopen half of this boundary story.
    for events in answers.into_iter().chain(reopened) {
        whole(&events, &expected);
    }
}

#[test]
fn a_frame_one_byte_past_the_page_is_refused_by_name() {
    let payload = payload();
    let bytes = frame_bytes(&payload);
    let mut world = baseline(bytes - 1);
    let records = snapshot(&mut world, bytes * 2);
    let files = world.contents();
    world.trace_writes();
    let events =
        world.request(Request::Commit { number: REFUSED, writes: root::turn_commit(REFUSED, TASK, 2, &payload) });
    assert_eq!(refused(&events, REFUSED, TASK, bytes - 1), bytes);
    assert!(world.written().is_empty(), "refused commit wrote data: {:?}", world.written());
    assert!(world.changes().is_empty(), "refused commit changed files: {:?}", world.changes());
    assert_eq!(world.contents(), files);
    world.crash(SEED + 1);
    assert_eq!(world.contents(), files, "refusal and crash preserve every file byte before reopen");
    recovered(&mut world, &records, bytes * 2);
}

#[test]
fn a_later_turn_past_the_page_refuses_its_whole_commit() {
    let payload = payload();
    let bytes = frame_bytes(&payload);
    let mut world = baseline(bytes - 1);
    let records = snapshot(&mut world, bytes * 2);
    let files = world.contents();
    let mut writes = root::turn_commit(REFUSED, TASK, 2, PRIOR).into_vec();
    writes.push(Write::Put { key: Box::from(b"refused".as_slice()), value: Box::from(b"never stored".as_slice()) });
    writes.push(Write::Save(core::Record::Core(core::CoreRecord::Turn(root::turn(TASK + 1, ATTEMPT, 1, &payload)))));
    world.trace_writes();
    let events = world.request(Request::Commit { number: REFUSED, writes: writes.into_boxed_slice() });
    assert_eq!(refused(&events, REFUSED, TASK + 1, bytes - 1), bytes);
    assert!(world.written().is_empty(), "earlier fitting data written: {:?}", world.written());
    assert!(world.changes().is_empty(), "refused batch changed files: {:?}", world.changes());
    assert_eq!(world.contents(), files);
    world.crash(SEED + 1);
    assert_eq!(world.contents(), files, "none of the refused batch survives the crash");
    recovered(&mut world, &records, bytes * 2);
    assert!(world.transcript(TranscriptRead::Page { task: TASK + 1, backwards: false, after: None }).is_empty());
}

#[test]
fn a_pending_commit_fails_once_when_a_later_frame_is_refused() {
    let payload = payload();
    let bytes = frame_bytes(&payload);
    let mut world = baseline(bytes - 1);
    world.trace_writes();
    world.cut_at(world.operation_count());
    let pending =
        world.request(Request::Commit { number: REFUSED, writes: root::turn_commit(REFUSED, TASK, 2, PRIOR) });
    assert!(pending.is_empty(), "commit must be pending: {pending:?}");
    let number = REFUSED + 1;
    let events = world.request(Request::Commit { number, writes: root::turn_commit(number, TASK + 1, 1, &payload) });
    let [Event::FrameTooLarge { number: actual, task, bytes: size, limit }, Event::Failed { number: pending }] =
        events.as_slice()
    else {
        panic!("refusal first, then exactly one failed terminal for the pending commit: {events:?}");
    };
    assert_eq!((*actual, *task, *size, *limit), (number, TASK + 1, bytes, bytes - 1));
    assert_eq!(*pending, REFUSED);
    eprintln!("terminals: {events:?}");
}

#[test]
fn the_frame_bound_covers_each_transcript_size() {
    let core = root::CORE_LIMITS;
    let floor = row_floor();
    let ceiling = CODEC_TRANSCRIPT_CEILING;
    // No ceiling-sized allocation. Below the row floor, a valid page cannot
    // refuse these small frames. Check round trips with ample decoded room;
    // their actual encoded-size comparison remains a public-surface gap.
    let mut sizes = vec![0, 1, 127, 128, 16_383, 16_384, core.resume_bytes, floor];
    sizes.retain(|size| *size <= ceiling);
    sizes.sort_unstable();
    sizes.dedup();
    let mut previous = None;
    for size in sizes {
        let bound = sizes::frame(&core, size).unwrap_or_else(|| panic!("no frame bound for admitted size {size}"));
        let transcript = vec![b'x'; usize::try_from(size).expect("small fixture")];
        if size >= floor {
            let actual = frame_bytes(&transcript);
            assert!(actual <= bound, "transcript {size}: refused frame {actual} exceeds bound {bound}");
            eprintln!("transcript {size}: refused frame {actual}, bound {bound}");
        } else {
            let mut world = opened(floor);
            assert_eq!(world.commit(root::first()), Some(FIRST));
            assert_eq!(world.commit(root::turn_commit(DURABLE, TASK, 1, &transcript)), Some(DURABLE));
            let expected = root::turn(TASK, ATTEMPT, 1, &transcript);
            for kind in starts() {
                whole(&read(&mut world, kind, bound * 2), &expected);
            }
            eprintln!("transcript {size}: encoded bound {bound}, whole read with decoded room {}", bound * 2);
        }
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
fn a_page_below_the_store_rows_fails_the_size_check() {
    let mut limits = root::limits();
    let bound = checked(&limits).expect("fixture limits pass the shell's check");
    let floor = bound.key + bound.value;
    eprintln!("store row floor: {floor}, key {}, value {}", bound.key, bound.value);
    limits.store.page.bytes = floor - 1;
    let result = checked(&limits);
    assert!(
        matches!(result, Err(sizes::SizeRefusal::Page { needs, has }) if needs == floor && has == floor - 1),
        "expected row floor {floor} refusal: {result:?}"
    );
    limits.store.page.bytes = floor;
    checked(&limits).expect("page exactly at row floor passes");
}
