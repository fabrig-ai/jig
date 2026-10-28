//! Activity pages join their opaque turns in one bounded transcript walk.
use super::*;

const WORDS: [&[u8]; 3] = [b"one", b"two", b"tri"];
fn turn_line(position: u64, turn: u32) -> core::Record {
    core::Record::Tasks(tasks::Stored::Activity(tasks::ActivityLine {
        task: CHILD,
        position,
        at: skein_lib::Wall::from_nanos(19),
        line: tasks::Line::Turn { reply: u64::from(turn), attempt: 1, turn, read: None },
    }))
}
fn transcript_page_with_last(last: &[u8]) -> (Store, Box<[u8]>) {
    let mut store = fixtures::opened();
    let mut owner = task_owner();
    owner.activity = 5;
    fixtures::committed(
        &mut store,
        Box::new([
            fixtures::header(1),
            fixtures::save(core::Record::Tasks(tasks::Stored::Ended(Box::new(owner)))),
            fixtures::save(turn_line(1, 1)),
            fixtures::save(fixtures::activity_line(CHILD, 2)),
        ]),
    );
    fixtures::committed(
        &mut store,
        Box::new([
            fixtures::header(2),
            fixtures::save(turn_line(3, 2)),
            fixtures::save(fixtures::activity_line(CHILD, 4)),
            fixtures::save(turn_line(5, 3)),
        ]),
    );
    let turns = [
        fixtures::turn(CHILD, 1, 1, WORDS[0]),
        fixtures::turn(CHILD, 1, 2, WORDS[1]),
        fixtures::turn(CHILD, 1, 3, last),
    ];
    let mut above = Queue::uncapped();
    let mut below = fixtures::below();
    crate::down(
        &mut store,
        &fixtures::env(),
        fixtures::commit(
            3,
            Box::new([
                fixtures::header(3),
                fixtures::save(core::Record::Core(core::CoreRecord::Turn(turns[0].clone()))),
                fixtures::save(core::Record::Core(core::CoreRecord::Turn(turns[1].clone()))),
                fixtures::save(core::Record::Core(core::CoreRecord::Turn(turns[2].clone()))),
            ]),
        ),
        &mut above,
        &mut below,
    );
    assert!(above.is_empty());
    let disk = fixtures::written(&mut store, below, &turns);
    (store, disk)
}
fn full_range(after: Option<reads::Position>) -> reads::Range {
    let mut range = activity_range(after);
    range.rows = 5;
    range
}
fn joined_page(
    store: &mut Store,
    range: reads::Range,
    disk: &[u8],
) -> (Box<[reads::Row]>, Option<reads::Position>, Box<[u64]>) {
    joined_page_with_fault(store, range, disk, false)
}
fn joined_page_with_fault(
    store: &mut Store,
    range: reads::Range,
    disk: &[u8],
    missing: bool,
) -> (Box<[reads::Row]>, Option<reads::Position>, Box<[u64]>) {
    let mut below = waiting_party(store, range);
    let mut frames = skein_lib::List::with_capacity(6);
    for _ in 0_u32..32 {
        let Some(file::Request::ReadAt { owner, offset, max, .. }) = below.transcripts.pop() else {
            panic!("one bounded transcript request");
        };
        assert!(below.transcripts.is_empty());
        if max >= 64 {
            frames.push(offset).expect("at most five frame reads");
        }
        let first = usize::try_from(offset).expect("fixture offset");
        let end = first.checked_add(usize::try_from(max).expect("fixture read")).expect("bounded disk");
        let (events, next) = fixtures::answer(
            store,
            Io::Transcripts(Arrived {
                event: if missing {
                    file::Event::Failed {
                        owner,
                        error: skein_io::kernel::Error::NotFound,
                        committed: false,
                        residue: None,
                    }
                } else {
                    file::Event::Read { owner, bytes: Box::from(&disk[first..end]) }
                },
                fd: None,
            }),
        );
        below = next;
        if !events.is_empty() {
            assert!(below.transcripts.is_empty());
            let (rows, next) = page(read_result(events));
            return (rows, next, frames.into_boxed());
        }
    }
    panic!("three turns finish within the bounded walk");
}
#[test]
fn an_activity_page_joins_each_turn_in_one_walk() {
    let (mut store, disk) = transcript_page_with_last(WORDS[2]);
    store.limits.joins = 5; // Two owner lookups, then one join per turn.
    let (rows, next, frames) = joined_page(&mut store, full_range(None), &disk);
    assert_eq!(frames.as_ref(), [134, 67, 0]);
    assert_eq!(next, None);
    assert_eq!(
        values(&rows).as_ref(),
        [
            turn_line(5, 3),
            fixtures::activity_line(CHILD, 4),
            turn_line(3, 2),
            fixtures::activity_line(CHILD, 2),
            turn_line(1, 1)
        ]
    );
    for (index, turn, bytes) in [(0, 3, b"tri"), (2, 2, b"two"), (4, 1, b"one")] {
        assert_eq!(
            rows[index].related,
            Some(core::Record::Core(core::CoreRecord::Turn(fixtures::turn(CHILD, 1, turn, bytes))))
        );
    }
}
#[test]
fn a_later_activity_page_finds_its_newest_turn_from_its_attempt() {
    let (mut store, disk) = transcript_page_with_last(WORDS[2]);
    let (rows, next, frames) = joined_page(&mut store, full_range(Some(activity_position(4))), &disk);
    assert_eq!(frames.as_ref(), [0, 67, 0]);
    assert_eq!(next, None);
    assert_eq!(values(&rows).as_ref(), [turn_line(3, 2), fixtures::activity_line(CHILD, 2), turn_line(1, 1)]);
}

fn line_cost(value: core::Record) -> u64 {
    reads::Row {
        value,
        owner: Some(Box::new(task_owner())),
        ancestors: Box::new([]),
        related: None,
        unjoined: None,
        ended_read: None,
        linked: Box::new([]),
        inbox: None,
    }
    .owned_bytes()
    .expect("fixture row size")
}
#[test]
fn a_turn_alone_past_the_page_comes_as_its_size() {
    let large = [42; 2048];
    let (mut store, disk) = transcript_page_with_last(&large);
    for (label, after, expected_frames) in
        [("tail", None, &[][..]), ("later", Some(activity_position(6)), &[0_u64, 67][..])]
    {
        let mut range = full_range(after);
        range.rows = 1;
        range.bytes = line_cost(turn_line(5, 3)) + 512;
        let (rows, next, frames) = joined_page(&mut store, range, &disk);
        assert_eq!(frames.as_ref(), expected_frames, "{label}: the large turn's bytes are not read");
        assert_eq!(next, Some(activity_position(5)), "{label}");
        assert_eq!(rows.len(), 1, "{label}");
        assert_eq!(rows[0].related, None, "{label}");
        assert_eq!(
            rows[0].unjoined,
            Some(reads::UnjoinedTurn::TooLarge {
                task: CHILD,
                attempt: 1,
                turn: 3,
                spent: 17,
                read: Some(12),
                at: skein_lib::Wall::from_nanos(19),
                bytes: 2048,
            }),
            "{label}"
        );
    }
}
#[test]
fn a_turn_past_what_is_left_ends_the_page_before_it() {
    let large = [42; 1024];
    let (mut store, disk) = transcript_page_with_last(&large);
    let mut range = full_range(None);
    range.rows = 3;
    range.bytes = line_cost(turn_line(5, 3))
        + 1024
        + u64::try_from(size_of::<core::Record>()).expect("record size")
        + line_cost(fixtures::activity_line(CHILD, 4))
        + line_cost(turn_line(3, 2))
        - 1;
    let (rows, next, frames) = joined_page(&mut store, range, &disk);
    assert_eq!(frames.as_ref(), [134]);
    assert_eq!(next, Some(activity_position(4)));
    assert_eq!(values(&rows).as_ref(), [turn_line(5, 3), fixtures::activity_line(CHILD, 4)]);
}
#[test]
fn an_unread_frame_ends_the_page_after_its_line() {
    let (mut store, disk) = transcript_page_with_last(WORDS[2]);
    let mut checksum = disk.clone();
    checksum[190] ^= 1;
    let mut lengths = disk.clone();
    *lengths.last_mut().expect("trailing length") ^= 1;
    let mut identity = disk.clone();
    identity[160] ^= 1;
    for (label, bytes, missing) in [
        ("checksum", checksum, false),
        ("lengths", lengths, false),
        ("different turn", identity, false),
        ("missing file", disk, true),
    ] {
        let (rows, next, _) = joined_page_with_fault(&mut store, full_range(None), &bytes, missing);
        assert_eq!(values(&rows).as_ref(), [turn_line(5, 3)], "{label}");
        assert_eq!(next, Some(activity_position(5)), "{label}");
        assert_eq!(rows[0].related, None, "{label}");
        assert_eq!(rows[0].unjoined, Some(reads::UnjoinedTurn::Unread { task: CHILD, attempt: 1, turn: 3 }), "{label}");
    }
}

#[test]
fn an_activity_first_turn_past_its_joins_names_the_bound() {
    const OWNER_JOINS: u32 = 1; // The owner fits, but the newest turn cannot join.
    let (mut store, _) = transcript_page_with_last(WORDS[2]);
    store.limits.joins = OWNER_JOINS;
    for after in [None, Some(activity_position(4))] {
        let result = load(&mut store, full_range(after));
        assert_too_linked(&result, OWNER_JOINS);
    }
}

#[test]
fn an_activity_join_cut_after_words_keeps_its_retry_position() {
    const OWNER_JOINS: u32 = 1;
    let (mut store, _) = transcript_page_with_last(WORDS[2]);
    store.limits.joins = OWNER_JOINS;
    let mut range = full_range(Some(activity_position(5)));
    let (rows, next) = page(load(&mut store, range.clone()));
    assert_eq!(values(&rows).as_ref(), [fixtures::activity_line(CHILD, 4)]);
    assert_eq!(next, Some(activity_position(4)));
    range.after = next;
    let result = load(&mut store, range);
    assert_too_linked(&result, OWNER_JOINS);
}
