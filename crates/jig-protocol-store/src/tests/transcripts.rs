//! Files synced before turns become durable (protocol/store.md, 8.1 and 8.2).
use super::fixtures::{self, Answer, LIVE};
use crate::{Arrived, Below, Event, Io, Phase, Store};
use alloc::boxed::Box;
use jig_domain_core::{CoreKey, CoreRecord, Key, Record, TurnRecord};
use skein_io::{file, kernel::Error};
use skein_lib::{Queue, Token};
fn submit(store: &mut Store, number: u64, turns: &[TurnRecord]) -> (Box<[Event]>, Below) {
    let mut writes =
        skein_lib::List::with_capacity(u32::try_from(turns.len()).expect("test turns").checked_add(1).expect("header"));
    writes.push(fixtures::header(number)).expect("header slot");
    for turn in turns {
        writes.push(fixtures::save(Record::Core(CoreRecord::Turn(turn.clone())))).expect("turn slot");
    }
    let mut above = Queue::with_capacity(crate::MAX_OUT_DOWN);
    let mut below = fixtures::below();
    crate::down(store, &fixtures::env(), fixtures::commit(number, writes.into_boxed()), &mut above, &mut below);
    (fixtures::events(&mut above), below)
}
fn writing() -> (Store, Below) {
    let (mut store, _) = fixtures::opened_with_transcripts();
    let (events, below) = submit(&mut store, 2, &[fixtures::turn(LIVE, 1, 1, b"one")]);
    assert!(events.is_empty());
    (store, below)
}
fn turn_bytes(store: &mut Store, mut below: Below) -> (Box<[u8]>, Below) {
    let Some(file::Request::OpenWriteNoFollow { owner, .. }) = below.transcripts.pop() else {
        panic!("open transcript");
    };
    let (_, below) = fixtures::answer(
        store,
        Io::Transcripts(Arrived { event: file::Event::Opened { owner, file: Token::new(700), len: 0 }, fd: None }),
    );
    let (bytes, offset, below) = written_frame(store, below);
    assert_eq!(offset, 0);
    (bytes, below)
}
fn written_frame(store: &mut Store, mut below: Below) -> (Box<[u8]>, u64, Below) {
    let Some(file::Request::WriteAt { owner, offset, bytes, .. }) = below.transcripts.pop() else {
        panic!("frame write");
    };
    let (events, below) =
        fixtures::answer(store, Io::Transcripts(Arrived { event: file::Event::Written { owner }, fd: None }));
    assert!(events.is_empty());
    (bytes, offset, below)
}
fn place(store: &mut Store) -> crate::codec::component::Transcript {
    let key =
        crate::keys::encode_own(&crate::keys::Own::Transcript { task: LIVE }, fixtures::KEY_LIMIT).expect("place key");
    let bytes = fixtures::get_kv(store, &key).expect("durable place");
    crate::codec::component::Transcript::decode(&bytes).expect("place decodes")
}
fn finish(store: &mut Store, below: Below) {
    let events = fixtures::script(store, below, &[Answer::Opened { len: 0 }, Answer::Written, Answer::Synced]);
    let [Event::Committed { number: 2, .. }] = &*events else {
        panic!("durable second commit: {events:?}");
    };
}
#[test]
fn a_turn_is_written_and_synced_before_its_commit() {
    let (mut store, below) = writing();
    assert_eq!(store.stores.store.last(), 1);
    let (bytes, below) = turn_bytes(&mut store, below);
    assert_eq!(store.stores.store.last(), 1);
    let events = fixtures::script(&mut store, below, &[Answer::Synced]);
    let [Event::Committed { number: 2, .. }] = &*events else {
        panic!("commit after frame sync");
    };
    assert_eq!(store.stores.store.last(), 2);
    assert_eq!(place(&mut store).length, u64::try_from(bytes.len()).expect("frame length"));
}
#[test]
fn a_turns_save_puts_no_turn_row_in_the_map() {
    let (mut store, below) = writing();
    finish(&mut store, below);
    let key = crate::keys::encode(&Key::Core(CoreKey::Turn { task: LIVE, attempt: 1, turn: 1 }), fixtures::KEY_LIMIT)
        .expect("turn key");
    assert_eq!(fixtures::get_kv(&mut store, &key), None);
}
#[test]
fn a_turn_is_saved_as_a_row_until_transcript_files_land() {
    let (mut store, below) = writing();
    let (bytes, below) = turn_bytes(&mut store, below);
    let expected = fixtures::turn(LIVE, 1, 1, b"one");
    assert_eq!(crate::transcripts::frame::decode(&bytes, &fixtures::codec_limits()), Ok(expected));
    let events = fixtures::script(&mut store, below, &[Answer::Synced]);
    let [Event::Committed { number: 2, .. }] = &*events else {
        panic!("framed turn durable");
    };
}
#[test]
fn a_missing_file_is_created_and_its_directory_synced() {
    let (mut store, below) = writing();
    let events = fixtures::script(
        &mut store,
        below,
        &[
            Answer::Missing,
            Answer::Made,
            Answer::Synced,
            Answer::Opened { len: 0 },
            Answer::Opened { len: 0 },
            Answer::Written,
            Answer::Synced,
            Answer::Synced,
            Answer::Closed,
        ],
    );
    let [Event::Committed { number: 2, .. }] = &*events else {
        panic!("created frame durable");
    };
    assert!(place(&mut store).length > 0);
}
#[test]
fn a_second_turn_writes_past_the_first_still_queued() {
    let (mut store, below) = writing();
    let (bytes, mut below) = turn_bytes(&mut store, below);
    assert!(submit(&mut store, 3, &[fixtures::turn(LIVE, 1, 2, b"two")]).0.is_empty());
    let Some(file::Request::Sync { owner, .. }) = below.transcripts.pop() else {
        panic!("first sync");
    };
    let (_, mut below) =
        fixtures::answer(&mut store, Io::Transcripts(Arrived { event: file::Event::Synced { owner }, fd: None }));
    let Some(file::Request::WriteAt { offset, .. }) = below.transcripts.pop() else {
        panic!("second frame");
    };
    assert_eq!(offset, u64::try_from(bytes.len()).expect("first frame length"));
}
#[test]
fn a_window_turn_moves_the_tasks_window_in_the_commit() {
    let (mut store, below) = writing();
    let (bytes, below) = turn_bytes(&mut store, below);
    fixtures::script(&mut store, below, &[Answer::Synced]);
    let (_, below) = submit(&mut store, 3, &[fixtures::window_turn(LIVE, 2, 1, b"window")]);
    let (_, _, below) = written_frame(&mut store, below);
    assert_eq!(place(&mut store).window, 0);
    fixtures::script(&mut store, below, &[Answer::Synced]);
    assert_eq!(place(&mut store).window, u64::try_from(bytes.len()).expect("first frame length"));
}
#[test]
fn a_commit_behind_a_turn_waits_its_turn() {
    let (mut store, below) = writing();
    assert!(submit(&mut store, 3, &[]).0.is_empty());
    assert_eq!(store.stores.store.last(), 1);
    let events = fixtures::script(&mut store, below, &[Answer::Opened { len: 0 }, Answer::Written, Answer::Synced]);
    let [Event::Committed { number: 2, .. }, Event::Committed { number: 3, .. }] = &*events else {
        panic!("commits retain their order: {events:?}");
    };
}
#[test]
fn a_failed_sync_fails_the_commit_and_those_behind() {
    let (mut store, below) = writing();
    assert!(submit(&mut store, 3, &[]).0.is_empty());
    let (_, mut below) = turn_bytes(&mut store, below);
    let Some(file::Request::Sync { owner, .. }) = below.transcripts.pop() else {
        panic!("sync frame");
    };
    let (events, _) = fixtures::answer(
        &mut store,
        Io::Transcripts(Arrived {
            event: file::Event::Failed { owner, error: Error::Permission, committed: false, residue: None },
            fd: None,
        }),
    );
    let [Event::Failed { number: 2 }, Event::Failed { number: 3 }] = &*events else {
        panic!("failed ordered commits: {events:?}");
    };
    assert_eq!(store.stores.store.last(), 1);
    assert_eq!(store.phase, Phase::Stopped);
}
fn two_open_transcripts() -> Store {
    let (mut store, _) = fixtures::opened_with_transcripts();
    let (_, below) = submit(&mut store, 2, &[fixtures::turn(LIVE, 1, 1, b"one"), fixtures::turn(22, 1, 1, b"two")]);
    fixtures::script(
        &mut store,
        below,
        &[
            Answer::Opened { len: 0 },
            Answer::Written,
            Answer::Synced,
            Answer::Opened { len: 0 },
            Answer::Written,
            Answer::Synced,
        ],
    );
    store
}
#[test]
fn a_full_file_set_closes_the_file_written_longest_ago() {
    let mut store = two_open_transcripts();
    let (_, below) = submit(&mut store, 3, &[fixtures::turn(LIVE, 1, 2, b"recent")]);
    fixtures::script(&mut store, below, &[Answer::Written, Answer::Synced]);
    let (_, mut below) = submit(&mut store, 4, &[fixtures::turn(23, 1, 1, b"third")]);
    let Some(file::Request::Close { file, .. }) = below.transcripts.pop() else {
        panic!("close before another open");
    };
    assert_eq!(file, Token::new(1_003));
    assert_eq!(store.transcripts.files.len(), 2);
}
#[test]
fn a_new_attempt_keeps_its_first_turns_offset() {
    let (mut store, below) = writing();
    let (bytes, below) = turn_bytes(&mut store, below);
    fixtures::script(&mut store, below, &[Answer::Synced]);
    let (_, below) = submit(&mut store, 3, &[fixtures::turn(LIVE, 2, 1, b"new"), fixtures::turn(LIVE, 2, 2, b"next")]);
    fixtures::script(&mut store, below, &[Answer::Written, Answer::Synced, Answer::Written, Answer::Synced]);
    let key = crate::keys::encode_own(&crate::keys::Own::Attempt { task: LIVE, attempt: 2 }, fixtures::KEY_LIMIT)
        .expect("attempt key");
    let value = fixtures::get_kv(&mut store, &key).expect("new attempt place");
    assert_eq!(
        crate::codec::component::Attempt::decode(&value).expect("attempt decodes").first,
        u64::try_from(bytes.len()).expect("first frame length")
    );
}
#[test]
fn the_transcript_queue_refuses_before_a_later_commit_is_submitted() {
    let (mut store, below) = writing();
    assert!(submit(&mut store, 3, &[]).0.is_empty());
    assert!(submit(&mut store, 4, &[]).0.is_empty());
    let (events, _) = submit(&mut store, 5, &[]);
    let [
        Event::Failed { number: 5 },
        Event::Failed { number: 2 },
        Event::Failed { number: 3 },
        Event::Failed { number: 4 },
    ] = &*events
    else {
        panic!("all unsubmitted commits fail: {events:?}");
    };
    assert!(!below.transcripts.is_empty());
    assert_eq!(store.stores.store.last(), 1);
    assert_eq!(store.phase, Phase::Stopped);
}

#[test]
fn a_frame_exactly_the_page_bound_commits_and_reads_back() {
    let (mut store, _) = fixtures::opened_with_transcripts();
    let turn = fixtures::turn(LIVE, 1, 1, b"one");
    let encoded = crate::transcripts::frame::encode(&turn, &store.codec).expect("tiny frame");
    store.limits.store.page.bytes = u32::try_from(encoded.len()).expect("bounded tiny frame fixture");
    let (events, below) = submit(&mut store, 2, core::slice::from_ref(&turn));
    assert!(events.is_empty());
    let disk = fixtures::written(&mut store, below, core::slice::from_ref(&turn));
    assert_eq!(disk, encoded);
    assert_eq!(store.stores.store.last(), 2);
    let (turns, next) =
        read_turns(&mut store, &disk, crate::TranscriptRead::Page { task: LIVE, backwards: false, after: None }, 1);
    assert_eq!(turns.as_ref(), &[turn]);
    assert_eq!(next, None);
}

#[test]
fn a_frame_one_past_the_page_bound_refuses_before_any_data_io() {
    let (mut store, _) = fixtures::opened_with_transcripts();
    let turn = fixtures::turn(LIVE, 1, 1, b"one");
    let bytes = u32::try_from(
        crate::transcripts::frame::encode(&turn, &store.codec).expect("bounded tiny frame fixture").len(),
    )
    .expect("bounded tiny frame fixture");
    let limit = bytes.checked_sub(1).expect("bounded tiny frame fixture");
    store.limits.store.page.bytes = limit;
    let (events, below) = submit(&mut store, 2, &[turn]);
    let [Event::FrameTooLarge { number: 2, task: LIVE, bytes: found, limit: bound }] = &*events else {
        panic!("one named refusal, no generic failure: {events:?}");
    };
    assert_eq!((*found, *bound), (bytes, limit));
    assert!(below.store.is_empty() && below.secrets.is_empty() && below.transcripts.is_empty());
    assert_eq!(store.stores.store.last(), 1);
    assert_eq!(store.phase, Phase::Stopped);
    assert!(submit(&mut store, 2, &[]).0.is_empty());
}

#[test]
fn an_oversized_later_turn_refuses_the_whole_batch_and_prior_data_survives_crash() {
    use super::fixtures::disk::Disk;
    let mut disk = Disk::new(&[]);
    let mut store = disk.store();
    let opened = disk.open(&mut store);
    let [Event::Opened { last: 0, .. }] = &*opened else { panic!("empty store opened: {opened:?}") };
    let prior = fixtures::turn(LIVE, 1, 1, b"old");
    let committed = disk.request(
        &mut store,
        fixtures::commit(
            1,
            Box::new([
                fixtures::header(1),
                fixtures::save(fixtures::live_task(LIVE)),
                fixtures::save(Record::Core(CoreRecord::Turn(prior.clone()))),
            ]),
        ),
    );
    let [Event::Committed { number: 1, .. }] = &*committed else { panic!("prior commit durable: {committed:?}") };
    let first = fixtures::turn(LIVE, 1, 2, b"new");
    let oversized = fixtures::turn(23, 1, 1, b"long");
    let limit = u32::try_from(
        crate::transcripts::frame::encode(&first, &store.codec).expect("bounded tiny frame fixture").len(),
    )
    .expect("bounded tiny frame fixture");
    let bytes = u32::try_from(
        crate::transcripts::frame::encode(&oversized, &store.codec).expect("bounded tiny frame fixture").len(),
    )
    .expect("bounded tiny frame fixture");
    assert_eq!(bytes, limit.checked_add(1).expect("bounded tiny frame fixture"));
    store.limits.store.page.bytes = limit;
    let before = disk.requests().len();
    let refused = disk.request(
        &mut store,
        fixtures::commit(
            2,
            Box::new([
                fixtures::header(2),
                fixtures::save(Record::Core(CoreRecord::Turn(first))),
                fixtures::save(Record::Core(CoreRecord::Turn(oversized))),
                crate::Write::Put { key: Box::new([41]), value: Box::new([42]) },
            ]),
        ),
    );
    let [Event::FrameTooLarge { number: 2, task: 23, bytes: found, limit: bound }] = &*refused else {
        panic!("later turn refuses complete batch: {refused:?}");
    };
    assert_eq!((*found, *bound), (bytes, limit));
    assert_eq!(disk.requests().len(), before, "no batch data IO, including its first frame or map rows");
    assert_eq!(store.stores.store.last(), 1);
    disk.crash(17);
    let mut reopened = disk.store();
    let opened = disk.open(&mut reopened);
    let [Event::Opened { last: 1, .. }] = &*opened else { panic!("prior commit recovered: {opened:?}") };
    let read = disk.request(
        &mut reopened,
        crate::Request::Transcript {
            owner: Token::new(801),
            read: crate::TranscriptRead::Page { task: LIVE, backwards: false, after: None },
            rows: 2,
            bytes: 4096,
        },
    );
    let (_, turns, next) = page(&read);
    assert_eq!(turns, &[prior]);
    assert_eq!(next, None);
    let row = disk.request(
        &mut reopened,
        crate::Request::Get { owner: Token::new(802), key: crate::Get::Application(Box::new([41])) },
    );
    let [Event::Got { row: None, .. }] = &*row else { panic!("refused application row absent: {row:?}") };
}

#[test]
fn a_named_frame_refusal_precedes_failures_of_earlier_pending_commits() {
    let (mut store, _) = writing();
    assert!(submit(&mut store, 3, &[]).0.is_empty());
    let turn = fixtures::turn(LIVE, 1, 2, b"long");
    let bytes = u32::try_from(
        crate::transcripts::frame::encode(&turn, &store.codec).expect("bounded tiny frame fixture").len(),
    )
    .expect("bounded tiny frame fixture");
    let limit = bytes.checked_sub(1).expect("bounded tiny frame fixture");
    store.limits.store.page.bytes = limit;
    let (events, below) = submit(&mut store, 4, &[turn]);
    let [
        Event::FrameTooLarge { number: 4, task: LIVE, bytes: found, limit: bound },
        Event::Failed { number: 2 },
        Event::Failed { number: 3 },
    ] = &*events
    else {
        panic!("named terminal then earlier pending failures: {events:?}")
    };
    assert_eq!((*found, *bound), (bytes, limit));
    assert_eq!(store.phase, Phase::Stopped);
    assert!(store.transcripts.current.is_none() && store.transcripts.queue.is_empty());
    assert!(below.store.is_empty() && below.secrets.is_empty() && below.transcripts.is_empty());
}

fn readable(turns: &[TurnRecord]) -> (Store, Box<[u8]>) {
    let (mut store, _) = fixtures::opened_with_transcripts();
    let (_, below) = submit(&mut store, 2, turns);
    let disk = fixtures::written(&mut store, below, turns);
    (store, disk)
}
fn ask(store: &mut Store, owner: u64, read: crate::TranscriptRead, rows: u32, bytes: u64) -> (Box<[Event]>, Below) {
    let mut above = Queue::with_capacity(crate::MAX_OUT_DOWN);
    let mut below = fixtures::below();
    crate::down(
        store,
        &fixtures::env(),
        crate::Request::Transcript { owner: Token::new(owner), read, rows, bytes },
        &mut above,
        &mut below,
    );
    (fixtures::events(&mut above), below)
}
fn read_answer(store: &mut Store, mut below: Below, disk: &[u8], chunk: u32) -> (Box<[Event]>, Below) {
    let request = below.transcripts.pop().expect("transcript io request");
    let event = match request {
        file::Request::OpenRead { owner, .. } => {
            file::Event::Opened { owner, file: Token::new(701), len: u64::try_from(disk.len()).expect("disk length") }
        }
        file::Request::ReadAt { owner, offset, max, .. } => {
            let start = usize::try_from(offset).expect("fixture offset");
            let count = usize::try_from(max.min(chunk)).expect("fixture count");
            let end = start.checked_add(count).expect("fixture read end").min(disk.len());
            let bytes = Box::from(disk.get(start..end).expect("read within fixture disk"));
            file::Event::Read { owner, bytes }
        }
        file::Request::MakeDirectory { .. }
        | file::Request::OpenPrivate { .. }
        | file::Request::Create { .. }
        | file::Request::CreateNoFollow { .. }
        | file::Request::OpenReadNoFollow { .. }
        | file::Request::OpenWrite { .. }
        | file::Request::OpenWriteNoFollow { .. }
        | file::Request::OpenDirectory { .. }
        | file::Request::Load { .. }
        | file::Request::Scan { .. }
        | file::Request::Store { .. }
        | file::Request::Stat { .. }
        | file::Request::WriteAt { .. }
        | file::Request::Cut { .. }
        | file::Request::Sync { .. }
        | file::Request::Close { .. }
        | file::Request::SyncDirectory { .. }
        | file::Request::Rename { .. }
        | file::Request::Remove { .. }
        | file::Request::ListPage { .. }
        | file::Request::Lock { .. }
        | file::Request::List { .. } => panic!("expected transcript read"),
    };
    fixtures::answer(store, Io::Transcripts(Arrived { event, fd: None }))
}
fn answer_reads(store: &mut Store, mut below: Below, disk: &[u8], chunk: u32) -> Box<[Event]> {
    for _ in 0_u32..4096 {
        let (events, next) = read_answer(store, below, disk, chunk);
        if !events.is_empty() {
            assert!(next.transcripts.is_empty());
            return events;
        }
        below = next;
    }
    panic!("read must settle within fixture bound");
}
fn appended(disk: &[u8], pending: &[u8]) -> Box<[u8]> {
    let mut extended = skein_lib::Writer::new(disk.len().checked_add(pending.len()).expect("fixture disk"));
    extended.put(disk).expect("committed frames");
    extended.put(pending).expect("pending frame");
    extended.finish()
}
fn reopen(store: Store) -> Store {
    let mut store = fixtures::over(store.into_stores());
    fixtures::run(
        &mut store,
        crate::Request::Open { owner: Token::new(800), root: fixtures::ROOT, installation: fixtures::INSTALLATION },
    );
    store
}
fn unread(events: &[Event]) {
    let [Event::Unread { owner }] = events else {
        panic!("unread terminal: {events:?}");
    };
    assert_eq!(*owner, Token::new(801));
}
fn page(events: &[Event]) -> (Token, &[TurnRecord], Option<u64>) {
    let [Event::Turns { owner, turns, next }] = events else {
        panic!("turn page: {events:?}");
    };
    (*owner, turns, *next)
}
fn read_turns(
    store: &mut Store,
    disk: &[u8],
    read: crate::TranscriptRead,
    rows: u32,
) -> (Box<[TurnRecord]>, Option<u64>) {
    let (events, below) = ask(store, 801, read, rows, 4096);
    assert!(events.is_empty());
    let events = answer_reads(store, below, disk, 4096);
    let (owner, turns, next) = page(&events);
    assert_eq!(owner, Token::new(801));
    (Box::from(turns), next)
}
fn window(after: Option<u64>) -> crate::TranscriptRead {
    crate::TranscriptRead::Window { task: LIVE, attempt: 2, turn: 1, after }
}
fn forwards(after: Option<u64>) -> crate::TranscriptRead {
    crate::TranscriptRead::Page { task: LIVE, backwards: false, after }
}
fn backwards(after: Option<u64>) -> crate::TranscriptRead {
    crate::TranscriptRead::Page { task: LIVE, backwards: true, after }
}
fn pair() -> [TurnRecord; 2] {
    [fixtures::window_turn(LIVE, 2, 1, b"one"), fixtures::turn(LIVE, 2, 2, b"two")]
}
// Each pair turn has a 55-byte versioned body and 12 bytes of framing.
const PAIR_FRAME: u64 = 67;
// One window-opening turn needs only one page; two attempts expose an
// unchanged empty cursor without relying on a wall-clock deadline.
const WINDOW_REQUESTS: u32 = 2;
#[test]
fn an_undersized_window_read_terminates_without_repeating_its_cursor() {
    let turns = [fixtures::window_turn(LIVE, 2, 1, b"one")];
    let (mut store, disk) = readable(&turns);
    let mut after = None;
    for _ in 0..WINDOW_REQUESTS {
        let (events, below) = ask(&mut store, 801, window(after), 1, PAIR_FRAME.checked_sub(1).expect("one below"));
        let events = if events.is_empty() { answer_reads(&mut store, below, &disk, 4096) } else { events };
        match events.as_ref() {
            [Event::Unread { .. }] => {
                unread(&events);
                assert!(store.transcripts.reads.is_empty());
                assert_eq!(store.transcripts.reading, None);
                return;
            }
            [Event::Turns { turns, next: Some(next), .. }] => {
                assert!(turns.is_empty(), "undersized first turn cannot fit");
                after = Some(*next);
            }
            _ => panic!("unread or continuation: {events:?}"),
        }
    }
    panic!("window did not terminate after {WINDOW_REQUESTS} requests; unchanged cursor {after:?}");
}

#[test]
fn direct_reads_without_row_or_byte_room_are_unread() {
    let (mut store, disk) = readable(&pair());
    let selected = crate::TranscriptRead::Turn { task: LIVE, attempt: 2, turn: 1 };
    for read in [window(None), forwards(None), backwards(None), selected] {
        for (rows, bytes) in [(0, PAIR_FRAME), (1, 0)] {
            let (events, below) = ask(&mut store, 801, read, rows, bytes);
            let events = if events.is_empty() { answer_reads(&mut store, below, &disk, 4096) } else { events };
            unread(&events);
        }
    }
}

#[test]
fn exact_frame_room_keeps_a_prefix_and_advances_its_cursor() {
    let turns = pair();
    let (mut store, disk) = readable(&turns);
    for read in [window(None), forwards(None), backwards(None)] {
        let (_, below) = ask(&mut store, 801, read, 2, PAIR_FRAME);
        let events = answer_reads(&mut store, below, &disk, 4096);
        let (_, found, next) = page(&events);
        let (expected, cursor) = match read {
            crate::TranscriptRead::Page { backwards: true, .. } => (turns.get(1..).expect("tail"), PAIR_FRAME),
            crate::TranscriptRead::Window { .. }
            | crate::TranscriptRead::Turn { .. }
            | crate::TranscriptRead::Page { backwards: false, .. } => (turns.get(..1).expect("prefix"), PAIR_FRAME),
        };
        assert_eq!(found, expected);
        assert_eq!(next, Some(cursor));
    }
    let (_, below) = ask(&mut store, 801, window(Some(PAIR_FRAME)), 2, PAIR_FRAME);
    let events = answer_reads(&mut store, below, &disk, 4096);
    let (_, found, next) = page(&events);
    assert_eq!(found, turns.get(1..).expect("remaining turn"));
    assert_eq!(next, None);
}

#[test]
fn a_nonempty_prefix_continues_before_a_turn_that_refuses() {
    let turns = [fixtures::window_turn(LIVE, 2, 1, b"one"), fixtures::turn(LIVE, 2, 2, b"four")];
    let (mut store, disk) = readable(&turns);
    let (_, below) = ask(&mut store, 801, window(None), 2, PAIR_FRAME);
    let events = answer_reads(&mut store, below, &disk, 4096);
    let (_, found, next) = page(&events);
    assert_eq!(found, turns.get(..1).expect("fitting prefix"));
    assert_eq!(next, Some(PAIR_FRAME));
    // The next turn needs one more payload byte than this request permits.
    let (_, below) = ask(&mut store, 801, window(next), 2, PAIR_FRAME);
    unread(&answer_reads(&mut store, below, &disk, 4096));
}

#[test]
fn exhausted_pages_remain_empty_terminal_answers() {
    let (mut store, disk) = readable(&pair());
    for read in [forwards(Some(PAIR_FRAME.checked_mul(2).expect("two frames"))), backwards(Some(0))] {
        let (events, below) = ask(&mut store, 801, read, 0, 0);
        assert!(below.transcripts.is_empty());
        let (_, found, next) = page(&events);
        assert!(found.is_empty());
        assert_eq!(next, None);
    }
    let (_, below) = ask(&mut store, 801, window(Some(u64::try_from(disk.len()).expect("end"))), 1, 0);
    let events = answer_reads(&mut store, below, &disk, 4096);
    let (_, found, next) = page(&events);
    assert!(found.is_empty());
    assert_eq!(next, None);
}

fn framed_readable(raw: &[u8]) -> Store {
    use jig_test_patches_jig::frames;
    let mut store = fixtures::over(fixtures::stores());
    store.patch_sets(super::frame_patches::SETS);
    fixtures::run(
        &mut store,
        crate::Request::Open { owner: Token::new(800), root: fixtures::ROOT, installation: fixtures::INSTALLATION },
    );
    fixtures::plant(
        &mut store.stores,
        &[
            skein_kv::Op::Put {
                key: crate::keys::encode_own(&crate::keys::Own::Transcript { task: frames::TASK }, fixtures::KEY_LIMIT)
                    .expect("frame places key"),
                value: crate::codec::component::Transcript {
                    length: u64::try_from(raw.len()).expect("frame"),
                    window: 0,
                }
                .encode()
                .expect("frame places"),
            },
            skein_kv::Op::Put {
                key: crate::keys::encode_own(
                    &crate::keys::Own::Attempt { task: frames::TASK, attempt: frames::ATTEMPT },
                    fixtures::KEY_LIMIT,
                )
                .expect("attempt key"),
                value: crate::codec::component::Attempt { first: 0 }.encode().expect("attempt place"),
            },
        ],
    );
    store
}
fn fixture_page(store: &mut Store, raw: &[u8], read: crate::TranscriptRead, bytes: u64) -> Box<[Event]> {
    // The fixture has one written smith site, then its terminal.
    const STOPS: u32 = 2;
    let (mut events, mut below) = ask(store, 801, read, 1, bytes);
    for _ in 0..STOPS {
        if events.is_empty() {
            events = answer_reads(store, below, raw, 4096);
        }
        let mut events_iter = events.into_iter();
        let event = events_iter.next().expect("frame read event");
        assert!(events_iter.next().is_none());
        if let Event::Site { owner, of, key, site, at, mut patcher } = event {
            assert_eq!(of, crate::Owner::Smith);
            let (old, new) = patcher.parts();
            let result = jig_test_patches_smith::patches::written(
                site,
                old,
                new,
                at,
                &mut jig_test_patches_smith::patches::Context { key },
            );
            let mut above = Queue::with_capacity(crate::MAX_OUT_DOWN);
            below = fixtures::below();
            crate::down(
                store,
                &fixtures::env(),
                crate::Request::Site { owner, patcher, result },
                &mut above,
                &mut below,
            );
            events = fixtures::events(&mut above);
        } else {
            return Box::new([event]);
        }
    }
    panic!("fixture frame did not terminate");
}
// Fixture said/replay fields each allow 64 bytes. Its current turn is 74
// bytes; a current frame with read=None has another 56 bytes (body + 12).
const FIXTURE_FRAME: u64 = 130;
const FIXTURE_TEXT: usize = 64;
// 64 replay bytes and the 18-byte frame key passed to the written site.
const FIXTURE_TOKENS: u32 = 82;
#[test]
fn current_and_patched_frames_share_the_exact_direct_read_bound() {
    use jig_test_patches_jig::frames;
    let said = [b's'; FIXTURE_TEXT];
    let replay = [b'r'; FIXTURE_TEXT];
    // Written tokens are replay length plus the 18-byte frame key.
    let expected = jig_store_world::patches::fixture_turn(2, &said, &[], FIXTURE_TOKENS);
    for (body, version, raw_length) in [(5, 1, 189), (6, 1, 194), (6, 2, FIXTURE_FRAME)] {
        let transcript = jig_store_world::patches::fixture_turn(version, &said, &replay, FIXTURE_TOKENS);
        let raw = frames::frame(body, &transcript);
        assert_eq!(u64::try_from(raw.len()).expect("raw frame length"), raw_length);
        let mut store = framed_readable(&raw);
        for read in [
            crate::TranscriptRead::Window {
                task: frames::TASK,
                attempt: frames::ATTEMPT,
                turn: frames::TURN,
                after: None,
            },
            crate::TranscriptRead::Turn { task: frames::TASK, attempt: frames::ATTEMPT, turn: frames::TURN },
            crate::TranscriptRead::Page { task: frames::TASK, backwards: false, after: None },
            crate::TranscriptRead::Page { task: frames::TASK, backwards: true, after: None },
        ] {
            let events = fixture_page(&mut store, &raw, read, FIXTURE_FRAME);
            let (_, turns, next) = page(&events);
            assert_eq!(turns.len(), 1);
            assert_eq!(turns.first().expect("one turn").transcript, expected);
            assert_eq!(next, None);
            unread(&fixture_page(&mut store, &raw, read, FIXTURE_FRAME.checked_sub(1).expect("one below")));
            // The effective bound is min(request, page), including patched framing.
            store.limits.store.page.bytes =
                u32::try_from(FIXTURE_FRAME.checked_sub(1).expect("one below")).expect("page");
            unread(&fixture_page(&mut store, &raw, read, FIXTURE_FRAME));
            store.limits.store.page.bytes = fixtures::store_limits().store.page.bytes;
        }
    }
}

#[test]
fn configured_read_room_and_frame_capacity_are_measured_separately() {
    use jig_protocol_config::{Profile, profile};
    // The frame bound is TurnRecord::worst_case_bytes + 12, including
    // version, optional read, transcript length, checksum and both lengths.
    const MAX_FRAME: u64 = 8_388_672;
    // Standard's map page includes the task/run revision fields' nine bytes;
    // transcript framing and direct read room are unchanged.
    for (kind, request, page) in [(Profile::Small, 65_536, 1_590_110), (Profile::Standard, 262_144, 10_666_845)] {
        let configured = profile(kind);
        let codec = crate::codec::limits(&configured.core).expect("profile codec");
        assert_eq!(u64::from(configured.core.resume_bytes), request);
        assert_eq!(configured.store.store.page.bytes, page);
        assert_eq!(crate::core_v6::TurnRecord::worst_case_bytes(&codec.core).expect("body") + 12, MAX_FRAME);
        assert!(request.min(u64::from(page)) < MAX_FRAME);
    }
    let limits = fixtures::store_limits();
    let sets = super::frame_patches::SETS;
    let capacity = crate::transcripts::patches::capacity(&sets.sets[0], 0, &limits).expect("body capacity");
    assert_eq!(u64::from(capacity.bytes) + 12, MAX_FRAME);
    for (limits, page) in [(jig_store_world::root::limits(), 330_532), (jig_store_world::patches::limits(), 2_048)] {
        assert_eq!(limits.core.resume_bytes, 256);
        assert_eq!(limits.store.page.bytes, page);
        let room = u64::from(limits.core.resume_bytes).min(u64::from(page));
        assert!(room >= FIXTURE_FRAME);
        assert!(room < MAX_FRAME);
    }
    assert_eq!(limits.core.resume_bytes, 256);
    assert_eq!(limits.store.page.bytes, 16_778_240);
    // Supplied well-formed smith fixtures: the current turn's maximum is
    // 74 bytes; the earlier layout's replay grows its raw maximum to 138.
    for (text, expected) in [(sets.sets[1].families[0].texts[0], 138), (sets.sets[1].families[0].texts[1], 74)] {
        let schema = skein_codec::schema::parse(text.as_bytes()).expect("fixture schema");
        let root = schema.find(b"Turn").expect("fixture turn");
        assert_eq!(schema.most_bytes(root), expected);
    }
}
#[test]
fn a_window_read_starts_at_the_windows_first_turn() {
    let turns = [
        fixtures::turn(LIVE, 1, 1, b"old"),
        fixtures::window_turn(LIVE, 2, 1, b"window"),
        fixtures::turn(LIVE, 2, 2, b"next"),
    ];
    let (mut store, disk) = readable(&turns);
    let (found, next) = read_turns(&mut store, &disk, window(None), 2);
    assert_eq!(&*found, turns.get(1..).expect("window turns"));
    assert_eq!(next, None);
}
#[test]
fn a_window_named_at_another_turn_reads_as_unread() {
    let (mut store, disk) = readable(&pair());
    let (events, below) =
        ask(&mut store, 801, crate::TranscriptRead::Window { task: LIVE, attempt: 2, turn: 2, after: None }, 2, 4096);
    assert!(events.is_empty());
    let events = answer_reads(&mut store, below, &disk, 4096);
    unread(&events);
}
#[test]
fn a_turn_is_found_from_its_attempts_first_turn() {
    let turns = [
        fixtures::turn(LIVE, 1, 1, b"old"),
        fixtures::turn(LIVE, 2, 1, b"first"),
        fixtures::turn(LIVE, 2, 2, b"found"),
    ];
    let (mut store, disk) = readable(&turns);
    let (_, below) = ask(&mut store, 801, crate::TranscriptRead::Turn { task: LIVE, attempt: 2, turn: 2 }, 2, 4096);
    let request = below.transcripts.iter().next().expect("first read");
    let file::Request::ReadAt { offset, .. } = request else {
        panic!("read attempt start");
    };
    assert_eq!(*offset, 67);
    let events = answer_reads(&mut store, below, &disk, 4096);
    let (_, found, next) = page(&events);
    assert_eq!(next, None);
    assert_eq!(found, turns.get(2..).expect("named turn"));
}
#[test]
fn a_backwards_page_uses_the_trailing_lengths() {
    let turns = pair();
    let (mut store, disk) = readable(&turns);
    let (found, next) = read_turns(&mut store, &disk, backwards(None), 1);
    assert_eq!(&*found, turns.get(1..).expect("last turn"));
    let (found, next) = read_turns(&mut store, &disk, backwards(next), 1);
    assert_eq!(&*found, turns.get(..1).expect("first turn"));
    assert_eq!(next, None);
}
#[test]
fn a_tail_page_comes_back_oldest_first() {
    let turns = pair();
    let (mut store, disk) = readable(&turns);
    assert_eq!(read_turns(&mut store, &disk, backwards(None), 2), (Box::from(turns), None));
}
#[test]
fn a_read_never_passes_the_committed_length() {
    let turns = [fixtures::window_turn(LIVE, 2, 1, b"one")];
    let (mut store, mut disk) = readable(&turns);
    let (_, below) = submit(&mut store, 3, &[fixtures::turn(LIVE, 2, 2, b"uncommitted")]);
    let (pending, _, _sync) = written_frame(&mut store, below);
    disk = appended(&disk, &pending);
    // Crash before the pending frame's sync and commit, retaining the durable map.
    store = reopen(store);
    let (events, below) = ask(&mut store, 801, window(None), 2, 4096);
    assert!(events.is_empty());
    let events = answer_reads(&mut store, below, &disk, 4096);
    let (_, found, next) = page(&events);
    assert_eq!(next, None);
    assert_eq!(found, &turns);
    assert_eq!(store.stores.store.last(), 2);
}
#[test]
fn a_frame_failing_its_checksum_reads_as_unread() {
    let (mut store, mut disk) = readable(&pair());
    *disk.get_mut(4).expect("first body byte") ^= 1;
    let (_, below) = ask(&mut store, 801, forwards(None), 2, 4096);
    let events = answer_reads(&mut store, below, &disk, 4096);
    unread(&events);
}
#[test]
fn a_window_page_continues_from_its_offset_cursor() {
    let turns = pair();
    let (mut store, disk) = readable(&turns);
    let (first, next) = read_turns(&mut store, &disk, window(None), 1);
    assert_eq!(&*first, turns.get(..1).expect("first turn"));
    assert_eq!(next, Some(67));
    let (second, next) = read_turns(&mut store, &disk, window(next), 1);
    assert_eq!(&*second, turns.get(1..).expect("second turn"));
    assert_eq!(next, None);
}
#[test]
fn a_forward_page_stops_within_its_byte_budget() {
    let turns = pair();
    let (mut store, disk) = readable(&turns);
    let (_, below) = ask(&mut store, 801, forwards(None), 2, 67);
    let events = answer_reads(&mut store, below, &disk, 4096);
    let (_, found, next) = page(&events);
    assert_eq!(found, turns.get(..1).expect("first turn"));
    assert_eq!(next, Some(67));
}
#[test]
fn a_short_read_is_filled_before_the_frame_is_decoded() {
    let turns = pair();
    let (mut store, disk) = readable(&turns);
    let (_, below) = ask(&mut store, 801, window(None), 2, 4096);
    let events = answer_reads(&mut store, below, &disk, 3);
    let (_, found, next) = page(&events);
    assert_eq!(next, None);
    assert_eq!(found, &turns);
}
#[test]
fn a_frame_with_a_length_past_the_committed_end_is_unread() {
    let (mut store, mut disk) = readable(&pair());
    *disk.get_mut(3).expect("leading length low byte") = 200;
    let (_, below) = ask(&mut store, 801, forwards(None), 2, 4096);
    let events = answer_reads(&mut store, below, &disk, 4096);
    unread(&events);
}
#[test]
fn a_transcript_without_a_place_is_unread() {
    let mut store = fixtures::opened();
    let (events, below) = ask(&mut store, 801, forwards(None), 2, 4096);
    unread(&events);
    assert!(below.transcripts.is_empty());
}
#[test]
fn a_missing_transcript_file_is_unread() {
    let (store, _) = readable(&pair());
    let mut store = reopen(store);
    let (_, below) = ask(&mut store, 801, forwards(None), 2, 4096);
    let events = fixtures::script(&mut store, below, &[Answer::Missing]);
    unread(&events);
}
#[test]
fn a_read_queued_behind_another_owner_gets_its_own_page() {
    let turns = pair();
    let (mut store, disk) = readable(&turns);
    let (_, below) = ask(&mut store, 801, forwards(None), 1, 4096);
    let (events, waiting) = ask(&mut store, 802, backwards(None), 1, 4096);
    assert!(events.is_empty());
    assert!(waiting.transcripts.is_empty());
    // First terminal starts the waiting owner's read in the same up step.
    let mut below = below;
    for _ in 0_u32..2 {
        let (events, next) = read_answer(&mut store, below, &disk, 4096);
        if !events.is_empty() {
            assert_eq!(page(&events).0, Token::new(801));
        }
        below = next;
    }
    let events = answer_reads(&mut store, below, &disk, 4096);
    let (owner, found, _) = page(&events);
    assert_eq!(owner, Token::new(802));
    assert_eq!(found, turns.get(1..).expect("tail turn"));
}
#[test]
fn writing_reopens_a_file_that_was_opened_for_reading() {
    let (store, disk) = readable(&pair());
    let mut store = reopen(store);
    read_turns(&mut store, &disk, forwards(None), 2);
    let (_, mut below) = submit(&mut store, 3, &[fixtures::turn(LIVE, 2, 3, b"new")]);
    let Some(file::Request::Close { file, .. }) = below.transcripts.pop() else {
        panic!("close read token");
    };
    assert_eq!(file, Token::new(701));
    below.transcripts.push(file::Request::Close { owner: Token::new(0x5452), file });
    let events = fixtures::script(
        &mut store,
        below,
        &[Answer::Closed, Answer::Opened { len: 134 }, Answer::Written, Answer::Synced],
    );
    let [Event::Committed { number: 3, .. }] = &*events else {
        panic!("write after read token closes");
    };
}
#[test]
fn a_read_script_delivers_one_complete_frame() {
    let turns = pair();
    let (mut store, disk) = readable(&turns);
    let (_, below) = ask(&mut store, 801, forwards(None), 1, 4096);
    let events = fixtures::script(
        &mut store,
        below,
        &[
            Answer::Read { bytes: Box::new([0, 0, 0, 67]) },
            Answer::Read { bytes: Box::from(disk.get(..67).expect("first frame")) },
        ],
    );
    let (_, found, next) = page(&events);
    assert_eq!(next, Some(67));
    assert_eq!(found, turns.get(..1).expect("first turn"));
}
#[test]
#[should_panic(expected = "startup bounds cover transcript reads")]
fn a_read_past_the_bounded_table_is_a_contract_break() {
    let (mut store, _) = readable(&pair());
    for owner in 801..806 {
        ask(&mut store, owner, forwards(None), 1, 4096);
    }
}
#[test]
fn a_window_with_no_row_room_still_checks_its_named_start() {
    let (mut store, disk) = readable(&pair());
    let (_, below) =
        ask(&mut store, 801, crate::TranscriptRead::Window { task: LIVE, attempt: 2, turn: 2, after: None }, 0, 4096);
    let events = answer_reads(&mut store, below, &disk, 4096);
    unread(&events);
}
#[test]
fn a_full_file_set_keeps_the_file_read_more_recently() {
    let mut store = two_open_transcripts();
    let turn = fixtures::turn(LIVE, 1, 1, b"one");
    let disk = crate::transcripts::frame::encode(&turn, &fixtures::codec_limits()).expect("fixture frame");
    read_turns(&mut store, &disk, forwards(None), 1);
    let (_, mut below) = submit(&mut store, 3, &[fixtures::turn(23, 1, 1, b"third")]);
    let Some(file::Request::Close { file, .. }) = below.transcripts.pop() else {
        panic!("bounded file set closes");
    };
    assert_eq!(file, Token::new(1_003));
    assert_eq!(store.transcripts.files.len(), 2);
}
#[test]
fn a_read_waiting_on_a_writer_keeps_its_durable_places() {
    let turns = [fixtures::window_turn(LIVE, 2, 1, b"one")];
    let (mut store, disk) = readable(&turns);
    let (_, below) = submit(&mut store, 3, &[fixtures::turn(LIVE, 2, 2, b"pending")]);
    let (pending, _, mut sync) = written_frame(&mut store, below);
    let disk = appended(&disk, &pending);
    let (events, waiting) = ask(&mut store, 801, window(None), 2, 4096);
    assert!(events.is_empty());
    assert!(waiting.transcripts.is_empty());
    let Some(file::Request::Sync { owner, .. }) = sync.transcripts.pop() else {
        panic!("pending writer sync");
    };
    let (_, below) =
        fixtures::answer(&mut store, Io::Transcripts(Arrived { event: file::Event::Synced { owner }, fd: None }));
    assert_eq!(store.stores.store.last(), 3);
    let events = answer_reads(&mut store, below, &disk, 4096);
    let (_, found, next) = page(&events);
    assert_eq!(found, &turns);
    assert_eq!(next, None);
}
fn cut_open(length: Option<u64>, claimed: bool) -> (Store, Box<[Event]>, Below) {
    let mut store = fixtures::over(fixtures::stores());
    // Three creation rows, the header, a run proof and a transcript place.
    let mut ops = skein_lib::List::with_capacity(6);
    ops.push(fixtures::format_put(fixtures::DEPLOYMENT)).expect("format");
    ops.push(fixtures::layouts_put()).expect("layouts");
    ops.push(fixtures::header_put(fixtures::DEPLOYMENT, 1)).expect("header");
    ops.push(fixtures::installation_put(fixtures::INSTALLATION)).expect("installation");
    if claimed {
        let record = fixtures::run_proof(LIVE);
        ops.push(skein_kv::Op::Put {
            key: crate::keys::encode(&record.key(), fixtures::KEY_LIMIT).expect("proof key"),
            value: crate::codec::encode(&record, &store.codec, &store.names).expect("proof"),
        })
        .expect("proof slot");
    }
    if let Some(length) = length {
        ops.push(skein_kv::Op::Put {
            key: crate::keys::encode_own(&crate::keys::Own::Transcript { task: LIVE }, fixtures::KEY_LIMIT)
                .expect("place key"),
            value: crate::codec::component::Transcript { length, window: 0 }.encode().expect("place"),
        })
        .expect("place slot");
    }
    fixtures::plant(&mut store.stores, ops.as_slice());
    let mut above = Queue::with_capacity(crate::MAX_OUT_DOWN);
    let mut below = fixtures::below();
    crate::down(
        &mut store,
        &fixtures::env(),
        crate::Request::Open { owner: Token::new(801), root: fixtures::ROOT, installation: fixtures::INSTALLATION },
        &mut above,
        &mut below,
    );
    (store, fixtures::events(&mut above), below)
}
fn cut_length(request: &file::Request) -> u64 {
    let file::Request::Cut { length, .. } = request else {
        panic!("cut request");
    };
    *length
}
fn cut_request(store: &mut Store, mut below: Below, len: u64) -> (file::Request, Below) {
    let Some(file::Request::OpenWriteNoFollow { owner, name, .. }) = below.transcripts.pop() else {
        panic!("claimed file open");
    };
    assert_eq!(&*name, b"0000000000000000/0000000000000015");
    let (events, mut below) = fixtures::answer(
        store,
        Io::Transcripts(Arrived { event: file::Event::Opened { owner, file: Token::new(750), len }, fd: None }),
    );
    assert!(events.is_empty());
    let request = below.transcripts.pop().expect("cut claimed file");
    (request, below)
}
fn cut_done(store: &mut Store, request: file::Request, mut below: Below) -> Box<[Event]> {
    below.transcripts.push(request);
    fixtures::script(store, below, &[Answer::Cut, Answer::Synced, Answer::Closed])
}
fn opened(events: &[Event]) {
    let [Event::Opened { owner, last, installation: _ }] = events else {
        panic!("open after cuts: {events:?}");
    };
    assert_eq!(*owner, Token::new(801));
    assert_eq!(*last, 1);
}
#[test]
fn open_cuts_a_claimed_tasks_file_to_its_committed_length() {
    let (mut store, events, below) = cut_open(Some(63), true);
    assert!(events.is_empty());
    let (request, below) = cut_request(&mut store, below, 120);
    assert_eq!(cut_length(&request), 63);
    opened(&cut_done(&mut store, request, below));
}
#[test]
fn open_cuts_a_file_without_places_to_zero() {
    let (mut store, events, below) = cut_open(None, true);
    assert!(events.is_empty());
    let (request, below) = cut_request(&mut store, below, 120);
    assert_eq!(cut_length(&request), 0);
    opened(&cut_done(&mut store, request, below));
}
#[test]
fn open_leaves_a_missing_file_missing() {
    let (mut store, events, below) = cut_open(Some(63), true);
    assert!(events.is_empty());
    opened(&fixtures::script(&mut store, below, &[Answer::Missing]));
}
#[test]
fn open_leaves_an_ended_tasks_file_untouched() {
    let (_, events, below) = cut_open(Some(63), false);
    opened(&events);
    assert!(below.transcripts.is_empty());
}
fn committed(events: &[Event], number: u64) {
    let [Event::Committed { number: found, .. }] = events else {
        panic!("committed terminal: {events:?}");
    };
    assert_eq!(*found, number);
}
fn drop_submit(store: &mut Store, number: u64, task: u64) -> (Box<[Event]>, Below) {
    let mut above = Queue::with_capacity(crate::MAX_OUT_DOWN);
    let mut below = fixtures::below();
    crate::down(
        store,
        &fixtures::env(),
        fixtures::commit(number, Box::new([fixtures::header(number), crate::Write::DropTranscript { task }])),
        &mut above,
        &mut below,
    );
    (fixtures::events(&mut above), below)
}
fn removed(store: &mut Store, below: Below, answer: Answer) -> Box<[Event]> {
    fixtures::script(store, below, &[Answer::Closed, Answer::Opened { len: 0 }, answer, Answer::Synced, Answer::Closed])
}
fn attempt_place(store: &mut Store, attempt: u64) -> Option<Box<[u8]>> {
    let key = crate::keys::encode_own(&crate::keys::Own::Attempt { task: LIVE, attempt }, fixtures::KEY_LIMIT)
        .expect("attempt key");
    fixtures::get_kv(store, &key)
}
fn transcript_place(store: &mut Store) -> Option<Box<[u8]>> {
    let key =
        crate::keys::encode_own(&crate::keys::Own::Transcript { task: LIVE }, fixtures::KEY_LIMIT).expect("place key");
    fixtures::get_kv(store, &key)
}
fn dropped() -> Store {
    let turns = [fixtures::turn(LIVE, 1, 1, b"one"), fixtures::turn(LIVE, 2, 1, b"two")];
    let (mut store, _) = readable(&turns);
    let (events, below) = drop_submit(&mut store, 3, LIVE);
    assert!(events.is_empty());
    removed(&mut store, below, Answer::Removed);
    store
}
#[test]
fn a_dropped_transcript_is_removed_before_its_commit() {
    let (mut store, _) = readable(&pair());
    let (events, below) = drop_submit(&mut store, 3, LIVE);
    assert!(events.is_empty());
    assert_eq!(store.stores.store.last(), 2);
    let events = removed(&mut store, below, Answer::Removed);
    committed(&events, 3);
    assert_eq!(transcript_place(&mut store), None);
}
#[test]
fn dropping_a_missing_file_is_not_a_failure() {
    let (mut store, _) = readable(&pair());
    let (_, below) = drop_submit(&mut store, 3, LIVE);
    let events = removed(&mut store, below, Answer::NotFound);
    committed(&events, 3);
}
#[test]
fn places_without_a_file_read_as_unread() {
    let (mut store, _) = readable(&pair());
    let (_, below) = drop_submit(&mut store, 3, LIVE);
    fixtures::script(&mut store, below, &[Answer::Closed, Answer::Opened { len: 0 }, Answer::Removed]);
    assert!(transcript_place(&mut store).is_some());
    let mut store = reopen(store);
    let (_, below) = ask(&mut store, 801, forwards(None), 2, 4096);
    unread(&fixtures::script(&mut store, below, &[Answer::Missing]));
}
#[test]
fn a_drop_erases_the_transcript_place_and_leaves_its_attempts_as_orphans() {
    let mut store = dropped();
    assert_eq!(transcript_place(&mut store), None);
    assert!(attempt_place(&mut store, 1).is_some());
    assert!(attempt_place(&mut store, 2).is_some());
}
#[test]
fn orphan_attempts_go_a_bounded_number_per_later_commit() {
    let mut store = dropped();
    submit(&mut store, 4, &[]);
    assert_eq!(attempt_place(&mut store, 1), None);
    assert!(attempt_place(&mut store, 2).is_some());
    submit(&mut store, 5, &[]);
    assert_eq!(attempt_place(&mut store, 2), None);
    assert_eq!(store.stores.store.last(), 5);
}
#[test]
fn open_queues_attempts_whose_transcript_place_is_gone() {
    let mut store = reopen(dropped());
    submit(&mut store, 4, &[]);
    assert_eq!(attempt_place(&mut store, 1), None);
    assert!(attempt_place(&mut store, 2).is_some());
}
#[test]
fn orphan_erases_are_included_in_the_commits_operation_bound() {
    let limits = fixtures::store_limits();
    let sizes =
        crate::sizes::check(&limits.core, &fixtures::APPLICATION, &limits.journal, &limits.store, 1).expect("fits");
    assert_eq!(sizes.ops, 12);
}
#[test]
fn dropping_a_transcript_fails_without_submitting_the_commit() {
    let (mut store, _) = fixtures::opened_with_transcripts();
    let (events, below) = drop_submit(&mut store, 2, 11);
    assert!(events.is_empty());
    let events = fixtures::script(
        &mut store,
        below,
        &[Answer::Opened { len: 0 }, Answer::NotFound, Answer::Synced, Answer::Closed],
    );
    committed(&events, 2);
    assert_eq!(store.stores.store.last(), 2);
}
