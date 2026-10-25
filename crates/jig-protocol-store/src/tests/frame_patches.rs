//! Patched transcript reads over real file drivers and a fake disk (store.md 8.6).
#![expect(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    reason = "test cases use ordinary Rust collections (programming-model.md 10.2)"
)]
use super::fixtures::{self, ALICE, disk::Disk};
use crate::{Event, KeptFamily, KeptMap, Owner, PatchSet, PatchSets, Request, Store, TranscriptRead, codec, keys};
use jig_domain_core::{self as domain, reads};
use jig_domain_tasks as tasks;
use jig_test_patches_jig::{frames, patches as jig};
use jig_test_patches_smith::patches as smith;
use skein_fake_machine::Item;
use skein_lib::{Token, Wall};

pub(super) const SETS: PatchSets = PatchSets {
    sets: &[
        PatchSet {
            owner: Owner::Jig,
            name: jig_test_patches_jig::NAME,
            patches: jig::PATCHES,
            fresh: None,
            families: &[KeptFamily {
                name: "fixture-frame",
                texts: jig::kept::fixture_frame::TEXTS,
                map: KeptMap::Store,
                keys: &[],
                frames: Some(jig_test_patches_jig::FRAME_ROOT),
            }],
        },
        PatchSet {
            owner: Owner::Smith,
            name: jig_test_patches_smith::NAME,
            patches: smith::PATCHES,
            fresh: Some(jig_test_patches_smith::FRESH),
            families: &[KeptFamily {
                name: "fixture-turn",
                texts: smith::kept::fixture_turn::TEXTS,
                map: KeptMap::Store,
                keys: &[],
                frames: Some(jig_test_patches_smith::ROOT),
            }],
        },
    ],
};
const OWNER: Token = Token::new(71);
const PATH: &[u8] = b"0000000000000000/0000000000000015";
const SAID: &[u8] = b"hello";
const REPLAY: &[u8] = b"replay";
const POSITION: u64 = 1;
const ROWS: u32 = 2;
const BUDGET: u64 = 65_536;
// Transcript places key (two one-byte tags and u64 task) followed by a u64 offset.
const FRAME_KEY_BYTES: u32 = 18;

pub(super) fn turn(version: u16, replay: &[u8]) -> Box<[u8]> {
    let mut out = version.to_be_bytes().to_vec();
    out.extend(u32::try_from(SAID.len()).expect("said").to_be_bytes());
    out.extend(SAID);
    if version == 1 {
        out.extend(u32::try_from(replay.len()).expect("replay").to_be_bytes());
        out.extend(replay);
    } else {
        out.extend(9_u32.to_be_bytes());
    }
    out.into_boxed_slice()
}
fn expected(version: u16) -> Box<[u8]> {
    let mut out = 2_u16.to_be_bytes().to_vec();
    out.extend(u32::try_from(SAID.len()).expect("said").to_be_bytes());
    out.extend(SAID);
    let tokens = if version == 1 {
        u32::try_from(REPLAY.len()).expect("replay").checked_add(FRAME_KEY_BYTES).expect("tokens")
    } else {
        9
    };
    out.extend(tokens.to_be_bytes());
    out.into_boxed_slice()
}
fn activity() -> domain::Record {
    domain::Record::Tasks(tasks::Stored::Activity(tasks::ActivityLine {
        task: frames::TASK,
        position: POSITION,
        at: Wall::EPOCH,
        line: tasks::Line::Turn { reply: POSITION, attempt: frames::ATTEMPT, turn: frames::TURN, read: None },
    }))
}
fn owner_record() -> domain::Record {
    let domain::Record::Tasks(tasks::Stored::Ended(mut owner)) =
        fixtures::ended_of(frames::TASK, tasks::Party::Person(ALICE))
    else {
        panic!("ended fixture")
    };
    owner.activity = POSITION;
    domain::Record::Tasks(tasks::Stored::Ended(owner))
}
fn put(record: &domain::Record) -> skein_kv::Op {
    skein_kv::Op::Put {
        key: keys::encode(&record.key(), fixtures::KEY_LIMIT).expect("key"),
        value: codec::encode(record, &fixtures::codec_limits(), &[]).expect("record"),
    }
}
fn scene(bytes: &[u8], sets: PatchSets) -> (Disk, Store) {
    let disk = Disk::new(&[Item::directory(b"0000000000000000"), Item::file(PATH, bytes)]);
    let mut store = fixtures::over(fixtures::stores());
    store.patch_sets(sets);
    let opened = fixtures::run(
        &mut store,
        Request::Open { owner: Token::new(1), root: disk.root(), installation: fixtures::INSTALLATION },
    );
    let [Event::Opened { .. }] = &*opened else { panic!("opened: {opened:?}") };
    fixtures::plant(
        &mut store.stores,
        &[
            skein_kv::Op::Put {
                key: keys::encode_own(&keys::Own::Transcript { task: frames::TASK }, fixtures::KEY_LIMIT)
                    .expect("places"),
                value: codec::component::Transcript { length: u64::try_from(bytes.len()).expect("frame"), window: 0 }
                    .encode()
                    .expect("places"),
            },
            skein_kv::Op::Put {
                key: keys::encode_own(
                    &keys::Own::Attempt { task: frames::TASK, attempt: frames::ATTEMPT },
                    fixtures::KEY_LIMIT,
                )
                .expect("attempt"),
                value: codec::component::Attempt { first: 0 }.encode().expect("attempt"),
            },
            put(&owner_record()),
            put(&activity()),
        ],
    );
    (disk, store)
}
fn request(kind: u8, budget: u64) -> Request {
    let read = match kind {
        0 | 4 => {
            TranscriptRead::Window { task: frames::TASK, attempt: frames::ATTEMPT, turn: frames::TURN, after: None }
        }
        1 => TranscriptRead::Turn { task: frames::TASK, attempt: frames::ATTEMPT, turn: frames::TURN },
        2 => {
            let mut range = fixtures::read(ALICE, reads::Scope::Activity { task: frames::TASK });
            range.backwards = true;
            range.bytes = budget;
            return Request::Read { owner: OWNER, range };
        }
        3 => TranscriptRead::Page { task: frames::TASK, backwards: false, after: None },
        5 => {
            let mut range = fixtures::read(
                ALICE,
                reads::Scope::Turn { task: frames::TASK, attempt: frames::ATTEMPT, turn: frames::TURN },
            );
            range.bytes = budget;
            return Request::Read { owner: OWNER, range };
        }
        _ => unreachable!("named read kind"),
    };
    Request::Transcript { owner: OWNER, read, rows: ROWS, bytes: budget }
}
fn answer(disk: &mut Disk, store: &mut Store, events: Box<[Event]>, refuse: bool) -> Box<[Event]> {
    answer_frame(disk, store, events, refuse, 0)
}
fn answer_frame(disk: &mut Disk, store: &mut Store, events: Box<[Event]>, refuse: bool, offset: u64) -> Box<[Event]> {
    let mut result = Vec::new();
    for event in events {
        match event {
            Event::Site { owner, of, key, site, at, mut patcher } => {
                assert_eq!(owner, OWNER);
                assert_eq!(of, Owner::Smith);
                let mut name = keys::encode_own(&keys::Own::Transcript { task: frames::TASK }, fixtures::KEY_LIMIT)
                    .expect("name")
                    .into_vec();
                name.extend(offset.to_be_bytes());
                assert_eq!(&*key, name);
                let status = if refuse {
                    Err(skein_codec::patch::Halt::NoSite(site))
                } else {
                    let (old, new) = patcher.parts();
                    smith::written(site, old, new, at, &mut smith::Context { key })
                };
                let events = disk.request(store, Request::Site { owner, patcher, result: status });
                result.extend(answer_frame(disk, store, events, refuse, offset));
            }
            other @ (Event::Opened { .. }
            | Event::OpenRefused { .. }
            | Event::Committed { .. }
            | Event::Failed { .. }
            | Event::FrameTooLarge { .. }
            | Event::Restored { .. }
            | Event::Loaded { .. }
            | Event::Read { .. }
            | Event::Got { .. }
            | Event::Turns { .. }
            | Event::Unread { .. }
            | Event::Kept { .. }
            | Event::Carried { .. }
            | Event::Stale { .. }
            | Event::Erased { .. }
            | Event::Secret { .. }
            | Event::SecretsLoaded { .. }
            | Event::Corrupt { .. }
            | Event::Patched { .. }) => result.push(other),
        }
    }
    result.into_boxed_slice()
}
fn payload(events: &[Event]) -> &[u8] {
    match events {
        [Event::Turns { turns, .. }] => &turns.first().expect("one turn").transcript,
        [Event::Read { result: reads::Loaded::Page { rows, .. }, .. }] => {
            let row = rows.first().expect("one row");
            let record = row.related.as_ref().unwrap_or(&row.value);
            let domain::Record::Core(domain::CoreRecord::Turn(turn)) = record else { panic!("joined turn {record:?}") };
            &turn.transcript
        }
        _ => panic!("read result {events:?}"),
    }
}
#[test]
fn every_read_kind_patches_each_kept_frame_without_writing() {
    const KINDS: u8 = 6; // resume, point, activity, page, tail's window path, party point
    for body in [5, 6] {
        for version in [1, 2] {
            let bytes = frames::frame(body, &turn(version, REPLAY));
            let (mut disk, mut store) = scene(&bytes, SETS);
            for kind in 0..KINDS {
                let events = disk.request(&mut store, request(kind, BUDGET));
                let events = answer(&mut disk, &mut store, events, false);
                assert_eq!(payload(&events), &*expected(version), "body {body}, turn {version}, kind {kind}");
                assert_eq!(&*disk.bytes(PATH), &*bytes);
            }
        }
    }
}
fn unread(events: &[Event]) -> bool {
    match events {
        [
            Event::Unread { .. }
            | Event::Read { result: reads::Loaded::TurnProblem { why: reads::TurnFailure::Malformed, .. }, .. },
        ] => true,
        [Event::Read { result: reads::Loaded::Page { rows, .. }, .. }] => rows.first().is_some_and(|row| {
            row.unjoined
                == Some(reads::UnjoinedTurn::Unread {
                    task: frames::TASK,
                    attempt: frames::ATTEMPT,
                    turn: frames::TURN,
                })
        }),
        _ => false,
    }
}
#[test]
fn every_machine_halt_is_an_unread_frame() {
    const SHORT: &[u8] = &[0];
    for transcript in [turn(0, REPLAY), turn(3, REPLAY), turn(1, b""), Box::from(SHORT)] {
        for body in [5, 6] {
            let (mut disk, mut store) = scene(&frames::frame(body, &transcript), SETS);
            for kind in 0..6 {
                let events = disk.request(&mut store, request(kind, BUDGET));
                let events = answer(&mut disk, &mut store, events, false);
                assert!(unread(&events), "body {body}, kind {kind}: {events:?}");
            }
        }
    }
    for body in [4, 7] {
        let (mut disk, mut store) = scene(&frames::frame(body, &turn(2, REPLAY)), SETS);
        for kind in 0..6 {
            let events = disk.request(&mut store, request(kind, BUDGET));
            assert!(unread(&events), "body {body}, kind {kind}: {events:?}");
        }
    }
    let (mut disk, mut store) = scene(&frames::frame(5, &turn(1, REPLAY)), SETS);
    for kind in 0..6 {
        let events = disk.request(&mut store, request(kind, BUDGET));
        let events = answer(&mut disk, &mut store, events, true);
        assert!(unread(&events), "NoSite, kind {kind}: {events:?}");
    }
}
#[test]
fn each_cut_of_a_behind_read_preserves_the_file() {
    let bytes = frames::frame(5, &turn(1, REPLAY));
    let (mut disk, mut store) = scene(&bytes, SETS);
    let events = disk.request(&mut store, request(0, BUDGET));
    drop(answer(&mut disk, &mut store, events, false));
    let points = disk.operations();
    for point in 0..=points {
        let (mut disk, mut store) = scene(&bytes, SETS);
        disk.cut_at(point);
        let events = disk.request(&mut store, request(0, BUDGET));
        drop(answer(&mut disk, &mut store, events, false));
        assert_eq!(&*disk.bytes(PATH), &*bytes, "cut {point}");
    }
}

#[test]
fn patched_frame_sizes_bound_resumes_turns_pages_and_tails() {
    // v6 body 44, v2 turn (2 + 4 + 5 + 4), framing 12.
    const PATCHED_FRAME: u64 = 71;
    for (replay, budget, accepted) in
        [(b"r".as_slice(), 70, false), (b"r".as_slice(), PATCHED_FRAME, true), (&[b'r'; 64], PATCHED_FRAME, true)]
    {
        let raw = frames::frame(5, &turn(1, replay));
        for kind in [0, 1, 3, 4] {
            let (mut disk, mut store) = scene(&raw, SETS);
            let events = disk.request(&mut store, request(kind, budget));
            let events = answer(&mut disk, &mut store, events, false);
            if accepted {
                let [Event::Turns { turns, .. }] = &*events else { panic!("turns: {events:?}") };
                assert!(!turns.is_empty(), "replay {}, kind {kind}, bound {budget}", replay.len());
            } else {
                assert!(unread(&events), "replay {}, kind {kind}, bound {budget}: {events:?}", replay.len());
            }
        }
    }
}

#[test]
fn serving_opens_refuse_bad_frames_sets_without_io() {
    const WRONG: PatchSets =
        PatchSets { sets: &[PatchSet { owner: Owner::Application, ..SETS.sets[0] }, SETS.sets[1]] };
    const TWO: PatchSets = PatchSets {
        sets: &[
            PatchSet { families: &[SETS.sets[0].families[0], SETS.sets[0].families[0]], ..SETS.sets[0] },
            SETS.sets[1],
        ],
    };
    const OTHER: &str = "family other-frame 6\nversioned record TurnRecord { transcript: bytes 64 holds other Turn }\n";
    const MISMATCH: PatchSets = PatchSets {
        sets: &[
            PatchSet {
                families: &[KeptFamily { name: "other-frame", texts: &[OTHER], ..SETS.sets[0].families[0] }],
                patches: &[],
                ..SETS.sets[0]
            },
            SETS.sets[1],
        ],
    };
    const MISSING: PatchSets = PatchSets { sets: &[SETS.sets[0]] };
    const BAD_ROOT: PatchSets = PatchSets {
        sets: &[
            SETS.sets[0],
            PatchSet {
                families: &[KeptFamily { frames: Some("Missing"), ..SETS.sets[1].families[0] }],
                ..SETS.sets[1]
            },
        ],
    };
    for (sets, owner) in [
        (WRONG, Owner::Application),
        (TWO, Owner::Jig),
        (MISMATCH, Owner::Jig),
        (MISSING, Owner::Jig),
        (BAD_ROOT, Owner::Smith),
    ] {
        let mut store = fixtures::over(fixtures::stores());
        store.patch_sets(sets);
        let events = fixtures::run(
            &mut store,
            Request::Open { owner: Token::new(1), root: fixtures::ROOT, installation: fixtures::INSTALLATION },
        );
        let [Event::OpenRefused { why: crate::OpenRefusal::Set { owner: found, patch: 0 }, .. }] = &*events else {
            panic!("set refusal {events:?}")
        };
        assert_eq!(*found, owner);
        assert_eq!(store.stores.store.last(), 0);
    }
}

#[test]
fn empty_kept_sets_make_no_machine_and_the_same_requests() {
    const EMPTY: PatchSets = PatchSets {
        sets: &[
            PatchSet {
                families: &[KeptFamily { texts: &[], ..SETS.sets[0].families[0] }],
                patches: &[],
                ..SETS.sets[0]
            },
            PatchSet {
                families: &[KeptFamily { texts: &[], ..SETS.sets[1].families[0] }],
                patches: &[],
                ..SETS.sets[1]
            },
        ],
    };
    let bytes = frames::frame(6, &turn(2, REPLAY));
    for kind in 0..6 {
        let (mut disk, mut store) = scene(&bytes, PatchSets::NONE);
        let events = disk.request(&mut store, request(kind, BUDGET));
        let requests = disk.requests();
        let (mut empty_disk, mut empty) = scene(&bytes, EMPTY);
        let empty_events = empty_disk.request(&mut empty, request(kind, BUDGET));
        assert!(empty.transcripts.machines.body.is_none() && empty.transcripts.machines.turn.is_none());
        assert_eq!(empty_disk.requests(), requests);
        assert_eq!(payload(&empty_events), payload(&events));
    }
}

#[test]
fn a_written_read_holds_commits_until_its_answer() {
    let bytes = frames::frame(5, &turn(1, REPLAY));
    let (mut disk, mut store) = scene(&bytes, SETS);
    // The planted map is its first commit; align admission with that map.
    store.phase = crate::Phase::Ready { next: 2, format_due: false };
    let events = disk.request(&mut store, request(0, BUDGET));
    let [Event::Site { .. }] = &*events else { panic!("read site {events:?}") };
    let commit = disk.request(&mut store, fixtures::commit(2, Box::new([fixtures::header(2)])));
    assert!(commit.is_empty());
    assert_eq!(store.stores.store.last(), 1);
    let returned = answer(&mut disk, &mut store, events, false);
    let [Event::Turns { .. }, Event::Committed { number: 2, .. }] = &*returned else {
        panic!("read before commit: {returned:?}")
    };
    assert_eq!(&*disk.bytes(PATH), &*bytes);
}

#[test]
fn stray_serving_sites_follow_the_existing_fail_stop_refusal() {
    let mut store = fixtures::opened();
    let patcher = Box::new(skein_codec::patch::Patcher::new(skein_codec::patch::Capacity {
        versions: 0,
        patches: 0,
        text: 0,
        bytes: 0,
        nodes: 0,
    }));
    let events = fixtures::run(&mut store, Request::Site { owner: OWNER, patcher, result: Ok(()) });
    failed_open(&events, Token::new(1));
    assert_eq!(store.phase, crate::Phase::Stopped);
}

#[test]
fn machine_byte_and_node_room_halt_as_unread() {
    for of in [Owner::Jig, Owner::Smith] {
        let bytes = frames::frame(5, &turn(1, REPLAY));
        let (mut disk, mut store) = scene(&bytes, SETS);
        let (machine, set) = match of {
            Owner::Jig => (store.transcripts.machines.body.as_mut().expect("body"), SETS.sets[0]),
            Owner::Smith => (store.transcripts.machines.turn.as_mut().expect("turn"), SETS.sets[1]),
            Owner::Skein | Owner::Application => unreachable!("frames owners"),
        };
        let capacity = if of == Owner::Jig {
            skein_codec::patch::Capacity { bytes: 1, ..machine.capacity }
        } else {
            skein_codec::patch::Capacity { nodes: 2, ..machine.capacity } // the fixture needs three nodes
        };
        let mut patcher = Box::new(skein_codec::patch::Patcher::new(capacity));
        for text in set.families[0].texts {
            patcher.keep(text.as_bytes()).expect("kept");
        }
        for text in set.patches {
            patcher.plan(text.as_bytes()).expect("plan");
        }
        machine.patcher = Some(patcher);
        for kind in 0..6 {
            let events = disk.request(&mut store, request(kind, BUDGET));
            let events = answer(&mut disk, &mut store, events, false);
            assert!(unread(&events), "{of:?}, kind {kind}: {events:?}");
        }
    }
}

#[test]
fn party_and_activity_bounds_charge_the_patched_frame() {
    const PATCHED_FRAME: u64 = 71; // v6 body 44 + v2 turn 15 + framing 12.
    const PAYLOAD: u64 = 15;
    for kind in [2, 5] {
        let raw = frames::frame(5, &turn(1, REPLAY));
        let (mut disk, mut store) = scene(&raw, SETS);
        let events = disk.request(&mut store, request(kind, BUDGET));
        let events = answer(&mut disk, &mut store, events, false);
        let [Event::Read { result: reads::Loaded::Page { rows, .. }, .. }] = &*events else { panic!("row {events:?}") };
        let base = rows[0].owned_bytes().expect("row").checked_sub(PAYLOAD).expect("metadata");
        for (body, version) in [(5, 1), (6, 2)] {
            for replay in [b"r".as_slice(), &[b'r'; 64]] {
                let raw = frames::frame(body, &turn(version, replay));
                for (frame, accepted) in [(70, false), (PATCHED_FRAME, true)] {
                    let (mut disk, mut store) = scene(&raw, SETS);
                    let events = disk.request(&mut store, request(kind, base.checked_add(frame).expect("bound")));
                    let events = answer(&mut disk, &mut store, events, false);
                    let carried = match &*events {
                        [Event::Read { result: reads::Loaded::Page { rows, .. }, .. }] => carries_turn(&rows[0]),
                        _ => false,
                    };
                    assert_eq!(carried, accepted, "kind {kind}, replay {}, frame {frame}: {events:?}", replay.len());
                    if !accepted {
                        let charged = match &*events {
                            [
                                Event::Read {
                                    result:
                                        reads::Loaded::TurnProblem { why: reads::TurnFailure::TooLarge { bytes }, .. },
                                    ..
                                },
                            ] => *bytes,
                            [Event::Read { result: reads::Loaded::Page { rows, .. }, .. }] => match rows[0].unjoined {
                                Some(reads::UnjoinedTurn::TooLarge { bytes, .. }) => bytes,
                                _ => panic!("large row"),
                            },
                            _ => panic!("large frame {events:?}"),
                        };
                        assert_eq!(charged, PATCHED_FRAME);
                    }
                }
            }
        }
    }
}

#[test]
fn a_site_with_another_reads_token_refuses_the_serving_open() {
    let bytes = frames::frame(5, &turn(1, REPLAY));
    let (mut disk, mut store) = scene(&bytes, SETS);
    let events = disk.request(&mut store, request(0, BUDGET));
    let mut events = events.into_iter();
    let Some(Event::Site { patcher, .. }) = events.next() else { panic!("site") };
    let events = disk.request(&mut store, Request::Site { owner: Token::new(72), patcher, result: Ok(()) });
    failed_open(&events, Token::new(1));
    assert_eq!(store.phase, crate::Phase::Stopped);
}

#[test]
fn a_site_before_any_open_follows_the_existing_refusal() {
    let mut store = fixtures::over(fixtures::stores());
    let capacity = skein_codec::patch::Capacity { versions: 0, patches: 0, text: 0, bytes: 0, nodes: 0 };
    let patcher = Box::new(skein_codec::patch::Patcher::new(capacity));
    let events = fixtures::run(&mut store, Request::Site { owner: OWNER, patcher, result: Ok(()) });
    failed_open(&events, crate::directory::OWNER);
    assert_eq!(store.phase, crate::Phase::Stopped);
}

#[test]
fn the_turn_machine_reads_its_kept_texts_most_nodes_and_one_past_is_unread() {
    // Empty records separate node admission from byte admission: one root,
    // one list and one node per item, in just six transcript bytes. The
    // machine's nodes are the kept texts' most, so the largest turn they
    // allow is read at exactly its machine's room; frame_memory derives and
    // prices smith's real schema the same way.
    const ITEMS: u32 = 64;
    const OVERHEAD: u32 = 2;
    const PATCH: &str = "patch 1\nowner fixture-smith 1\nfamily fixture-turn 1 to 2\n";
    const FIRST: &str = "family fixture-turn 1\nrecord Empty {}\nversioned record Turn { nodes: list 64 Empty }\n";
    const LAST: &str = "family fixture-turn 2\nrecord Empty {}\nversioned record Turn { nodes: list 64 Empty }\n";
    const SMITH: PatchSet = PatchSet {
        families: &[KeptFamily { texts: &[FIRST, LAST], ..SETS.sets[1].families[0] }],
        patches: &[PATCH],
        ..SETS.sets[1]
    };
    const HEADER_BYTES: u32 = 6; // u16 version and u32 list count.
    let schema = skein_codec::schema::parse(FIRST.as_bytes()).expect("node fixture");
    let root = schema.find(b"Turn").expect("Turn");
    assert_eq!(schema.most_bytes(root), HEADER_BYTES);
    let skein_codec::schema::Shape::Record { first, .. } = schema.declaration(root).shape else {
        panic!("record root")
    };
    let skein_codec::schema::Type::List { bound, .. } = schema.ty(schema.field(first).ty) else { panic!("node list") };
    assert_eq!(bound, ITEMS);
    let sets = PatchSets { sets: &[SMITH] };
    for (count, accepted) in [(ITEMS, true), (ITEMS.checked_add(1).expect("one past"), false)] {
        let transcript = node_turn(1, count);
        let bytes = frames::frame(6, &transcript);
        let (mut disk, mut store) = scene(&bytes, sets);
        let machine = store.transcripts.machines.turn.as_ref().expect("smith's machine");
        assert_eq!(machine.capacity.nodes, ITEMS.checked_add(OVERHEAD).expect("nodes"), "the kept texts' most");
        let events = disk.request(&mut store, request(1, u64::from(crate::core_v6::CEILINGS.turn_record_transcript)));
        if accepted {
            assert_eq!(payload(&events), &*node_turn(2, count));
        } else {
            assert!(unread(&events), "past the kept texts: {events:?}");
        }
        assert_eq!(&*disk.bytes(PATH), &*bytes);
    }
}

fn node_turn(version: u16, count: u32) -> Box<[u8]> {
    const HEADER_BYTES: usize = 6; // u16 version and u32 list count.
    let mut transcript = version.to_be_bytes().to_vec();
    transcript.extend(count.to_be_bytes());
    assert_eq!(transcript.len(), HEADER_BYTES);
    transcript.into_boxed_slice()
}

#[test]
fn patched_pages_keep_the_files_offsets_and_site_names() {
    const FIRST_REPLAY: &[u8] = &[b'r'; 64];
    const NEXT_TURN: u32 = 2;
    const FIRST_TOKENS: u32 = 82; // 64 replay bytes + 18 key bytes.
    const SECOND_TOKENS: u32 = 19; // one replay byte + 18 key bytes.
    let first = frames::frame(5, &turn(1, FIRST_REPLAY));
    let offset = u64::try_from(first.len()).expect("first frame");
    let mut second = fixtures::turn(frames::TASK, frames::ATTEMPT, NEXT_TURN, &turn(1, b"r"));
    second.spent = frames::SPENT;
    second.read = None;
    second.at = Wall::EPOCH;
    let second = crate::transcripts::frame::encode(&second, &fixtures::codec_limits()).expect("second frame");
    let bytes = [first.as_ref(), second.as_ref()].concat();
    let (mut disk, mut store) = scene(&bytes, SETS);
    for (after, frame_offset, tokens, next) in
        [(None, 0, FIRST_TOKENS, Some(offset)), (Some(offset), offset, SECOND_TOKENS, None)]
    {
        let read = TranscriptRead::Page { task: frames::TASK, backwards: false, after };
        let events = disk.request(&mut store, Request::Transcript { owner: OWNER, read, rows: 1, bytes: BUDGET });
        let events = answer_frame(&mut disk, &mut store, events, false, frame_offset);
        let [Event::Turns { turns, next: found, .. }] = &*events else { panic!("page {events:?}") };
        assert_eq!(*found, next);
        let mut expected = 2_u16.to_be_bytes().to_vec();
        expected.extend(u32::try_from(SAID.len()).expect("said").to_be_bytes());
        expected.extend(SAID);
        expected.extend(tokens.to_be_bytes()); // replay length (64 or 1) + eighteen key bytes
        assert_eq!(turns[0].transcript, expected.into_boxed_slice());
    }
    assert_eq!(&*disk.bytes(PATH), bytes);
}

#[test]
fn serving_machines_follow_the_switch_without_overlapping_the_copy() {
    // Disk::request's finite settling bound, with an observer at every I/O.
    const SETTLE: u32 = 10_000;
    const BASE: PatchSets = PatchSets {
        sets: &[
            PatchSet {
                patches: &[],
                families: &[KeptFamily { texts: &[jig::kept::fixture_frame::TEXTS[0]], ..SETS.sets[0].families[0] }],
                ..SETS.sets[0]
            },
            PatchSet {
                patches: &[],
                families: &[KeptFamily { texts: &[smith::kept::fixture_turn::TEXTS[0]], ..SETS.sets[1].families[0] }],
                ..SETS.sets[1]
            },
        ],
    };
    let mut disk = Disk::new(&[]);
    let mut store = disk.store();
    store.patch_sets(BASE);
    drop(disk.open(&mut store));
    drop(disk.request(&mut store, fixtures::commit(1, Box::new([fixtures::header(1)]))));
    let items = disk.items();
    drop(store);
    drop(disk);
    let mut disk = Disk::new(&items);
    let mut store = disk.store();
    store.patch_sets(SETS);
    let mut above = skein_lib::Queue::with_capacity(crate::MAX_OUT_DOWN);
    let mut below = fixtures::below();
    let env = fixtures::env();
    crate::down(
        &mut store,
        &env,
        Request::Open { owner: Token::new(1), root: disk.root(), installation: fixtures::INSTALLATION },
        &mut above,
        &mut below,
    );
    let mut patch_steps = 0_u32;
    let mut events = Vec::new();
    for _ in 0..SETTLE {
        crate::fire(&mut store, &env, &mut above, &mut below);
        disk.drive(&mut below, |io, below| {
            if crate::directory::patching(&store).is_some() {
                assert!(
                    store.transcripts.machines.body.is_none() && store.transcripts.machines.turn.is_none(),
                    "the copy holds only its own machines"
                );
                patch_steps = patch_steps.checked_add(1).expect("finite open");
            }
            crate::up(&mut store, &env, io, &mut above, below);
        });
        events.extend(fixtures::events(&mut above));
        if let Some(Event::Opened { .. }) = events.last() {
            break;
        }
    }
    assert!(patch_steps > 0, "the open patched a copy");
    let Some(Event::Opened { last: 2, .. }) = events.last() else { panic!("switched open: {events:?}") };
    assert_eq!(store.transcripts.machines.body.as_ref().expect("serving body").last, 6);
    assert_eq!(store.transcripts.machines.turn.as_ref().expect("serving turn").last, 2);
}

fn failed_open(events: &[Event], owner: Token) {
    let [Event::OpenRefused { owner: found, why: crate::OpenRefusal::Failed }] = events else {
        panic!("failed open {events:?}")
    };
    assert_eq!(*found, owner);
}
fn carries_turn(row: &reads::Row) -> bool {
    if let Some(domain::Record::Core(domain::CoreRecord::Turn(turn))) = &row.related {
        return !turn.transcript.is_empty();
    }
    if let domain::Record::Core(domain::CoreRecord::Turn(turn)) = &row.value {
        return !turn.transcript.is_empty();
    }
    false
}
