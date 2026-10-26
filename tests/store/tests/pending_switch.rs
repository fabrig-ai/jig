//! Restores use the checked copy; activation is a separate durable request (store.md 10.2).
use jig_protocol_store::{Event, OpenRefusal, Request, Wire, Write, codec, component_v3, keys};
use jig_store_world::{
    World,
    patches::{self, BASE, Records, SETS},
    root,
};
use skein_lib::{Env, Queue, Reader, Time, Token, Wall};

const SEED: u64 = 960;
// Initial commit, four public patch pages and the seal in Records::story.
const PATCHED_LAST: u64 = 6;
const RESTORED_LAST: u64 = PATCHED_LAST + 1;
const RESTORED_KEY: &[u8] = b"restored";
const RESTORED_VALUE: &[u8] = b"copy alone";
const SWITCH: Token = Token::new(961);

fn pending() -> World {
    let mut world = Records::story().committed(SEED, BASE);
    world.patch_sets(SETS);
    world.trace_writes();
    world.open().expect("checked pending copy");
    world
}

fn restore_commit() -> Box<[Write]> {
    Box::new([
        Write::Save(root::header(RESTORED_LAST)),
        Write::Put { key: RESTORED_KEY.into(), value: RESTORED_VALUE.into() },
    ])
}

fn manifest(world: &mut World) -> codec::component::Manifest {
    let contents = world.contents();
    let bytes = contents.get(b"generations/2/manifest".as_slice()).expect("manifest").as_ref().expect("file");
    codec::component::Manifest::decode(bytes).expect("manifest decodes")
}

#[test]
fn a_patched_copy_opens_pending_and_switches_when_asked() {
    let mut world = pending();
    let original = patches::source(&mut world);
    assert!(world.pending);
    assert_eq!(world.active(), b"1\n");
    assert!(!world.paths().iter().any(|path| path == b"generations/2/manifest"));
    assert_eq!(world.commit(restore_commit()), Some(RESTORED_LAST));
    assert_eq!(world.application(false).get(RESTORED_KEY), Some(&Box::from(RESTORED_VALUE)));
    assert_eq!(patches::source(&mut world), original, "the pending commit leaves A's maps alone");
    assert_eq!(world.switch(), Some(()));
    assert_eq!(world.active(), b"2\n");
    assert_eq!(manifest(&mut world).store, RESTORED_LAST);
    assert_eq!(patches::source(&mut world), original);
    world.crash(SEED);
    assert_eq!(world.open(), Some(RESTORED_LAST));
    assert!(!world.pending);
    assert!(world.patched().is_empty());
    assert_eq!(world.application(true).get(RESTORED_KEY), Some(&Box::from(RESTORED_VALUE)));
}

#[test]
fn a_start_that_ends_before_asking_leaves_the_old_generation_active() {
    let mut world = pending();
    let original = patches::source(&mut world);
    world.commit(restore_commit());
    world.lay(&[skein_fake_machine::Item::file(b"generations/2/pending-copy", b"discard me")]);
    world.crash(SEED);
    assert_eq!(world.active(), b"1\n");
    assert_eq!(world.open(), Some(PATCHED_LAST));
    assert!(world.pending);
    assert!(!world.patched().is_empty());
    assert!(!world.application(false).contains_key(RESTORED_KEY));
    assert!(!world.paths().iter().any(|path| path == b"generations/2/pending-copy"));
    patches::kept(&original, &patches::source(&mut world), &"abandoned restore");
}

#[test]
fn a_switch_waits_for_the_commits_asked_before_it() {
    let mut world = pending();
    world.enqueue(Request::Commit { number: RESTORED_LAST, writes: restore_commit() });
    world.enqueue(Request::Switch { owner: SWITCH });
    assert!(!world.paths().iter().any(|path| path == b"generations/2/manifest"));
    assert!(!world.manifest_queued(), "no manifest request before the accepting commit's durability");
    let events = world.settle();
    assert!(matches!(
        events.as_slice(),
        [Event::Committed { number: RESTORED_LAST, .. }, Event::Switched { owner: SWITCH }]
    ));
    assert_eq!(manifest(&mut world).store, RESTORED_LAST);
}

fn transcript_writes(drop: bool) -> Box<[Write]> {
    let write = if drop {
        Write::DropTranscript { task: root::LIVE }
    } else {
        Write::Save(jig_domain_core::Record::Core(jig_domain_core::CoreRecord::Turn(root::turn(
            root::LIVE,
            1,
            1,
            b"turn",
        ))))
    };
    Box::new([Write::Save(root::header(RESTORED_LAST)), write])
}

#[test]
fn a_transcript_write_while_pending_is_refused() {
    for drop in [false, true] {
        let mut world = pending();
        let before = world.contents();
        let events = world.request(Request::Commit { number: RESTORED_LAST, writes: transcript_writes(drop) });
        assert!(matches!(events.as_slice(), [Event::Failed { number: RESTORED_LAST }]));
        assert_eq!(world.contents(), before, "refused before any data IO");
    }
}

#[test]
fn a_transcript_write_after_the_switch_is_accepted() {
    for drop in [false, true] {
        let mut world = pending();
        world.switch();
        let events = world.request(Request::Commit { number: RESTORED_LAST, writes: transcript_writes(drop) });
        assert!(matches!(events.as_slice(), [Event::Committed { number: RESTORED_LAST, .. }]));
    }
}

#[test]
fn a_switch_is_refused_without_a_pending_open() {
    let mut world = World::new(SEED);
    let events = world.request(Request::Switch { owner: SWITCH });
    assert!(matches!(events.as_slice(), [Event::OpenRefused { owner: SWITCH, why: OpenRefusal::Failed }]));
}

#[test]
fn a_second_switch_is_refused() {
    let mut world = pending();
    world.switch();
    let events = world.request(Request::Switch { owner: SWITCH });
    assert!(matches!(events.as_slice(), [Event::OpenRefused { owner: SWITCH, why: OpenRefusal::Failed }]));
}

#[test]
fn each_cut_around_the_switch_leaves_one_generation_whole() {
    let mut completed = pending();
    let first = completed.operation_count();
    completed.switch();
    let cuts: Vec<_> = patches::cuts(&completed).into_iter().filter(|cut| *cut >= first).collect();
    assert!(!cuts.is_empty());
    for cut in cuts {
        let mut world = pending();
        let original = patches::source(&mut world);
        world.cut_at(cut);
        assert_eq!(world.switch(), None, "cut {cut}");
        world.crash(u64::from(cut));
        let old_active = world.active() == b"1\n";
        assert_eq!(world.open(), Some(PATCHED_LAST), "cut {cut}");
        assert_eq!(world.pending, old_active, "only the old generation patches again: cut {cut}");
        if world.pending {
            world.switch();
        }
        patches::whole(&mut world);
        patches::kept(&original, &patches::source(&mut world), &cut);
        Records::story().check(&mut world, 2, &cut);
    }
}

const WIRES: &[Wire] = &[Wire { name: "channel", version: 3 }, Wire { name: "payload", version: 7 }];
// store.md 5.3: 16 wires and 32-byte names, and each first refused size.
const MAX_WIRES: usize = 16;
const MAX_NAME: &str = "01234567890123456789012345678901";
const LONG_NAME: &str = "012345678901234567890123456789012";

fn wires_row(mut world: World) -> Vec<(Vec<u8>, u32)> {
    world.commit(Box::new([Write::Save(root::header(1)), Write::Wires]));
    let mut stores = world.into_stores();
    let mut events = Queue::with_capacity(skein_kv::MAX_OUT.above);
    let mut files = Queue::with_capacity(skein_kv::MAX_OUT.below);
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: root::limits().store };
    skein_kv::down(
        &mut stores.store,
        &env,
        skein_kv::Request::Get {
            owner: SWITCH,
            key: keys::encode_own(&keys::Own::Wires, root::limits().store.key).expect("component key"),
        },
        &mut events,
        &mut files,
    );
    let Some(skein_kv::Event::Got { value: Some(bytes), .. }) = events.pop() else { panic!("wires row") };
    let row = component_v3::Wires::decode(&component_v3::CEILINGS, &mut Reader::new(&bytes)).expect("wires decode");
    row.wires().iter().map(|wire| (wire.name().to_vec(), wire.version())).collect()
}

#[test]
fn the_wires_commit_puts_the_component_row() {
    let mut world = World::new(SEED);
    assert!(world.wires(WIRES));
    assert_eq!(wires_row(world), [(b"channel".to_vec(), 3), (b"payload".to_vec(), 7)]);
}

#[test]
fn no_wires_handed_writes_an_empty_row() {
    assert_eq!(wires_row(World::new(SEED)), []);
}

#[test]
fn wires_accept_the_count_and_name_bounds() {
    let mut world = World::new(SEED);
    assert!(world.wires(&[Wire { name: MAX_NAME, version: 1 }; MAX_WIRES]));
    assert_eq!(wires_row(world), vec![(MAX_NAME.as_bytes().to_vec(), 1); MAX_WIRES]);
}

#[test]
fn wires_refuse_one_past_the_count_or_name_bound() {
    for wires in [&[Wire { name: "wire", version: 1 }; MAX_WIRES + 1][..], &[Wire { name: LONG_NAME, version: 1 }][..]]
    {
        let mut world = World::new(SEED);
        assert!(!world.wires(wires));
        assert_eq!(wires_row(world), [], "refused data was not retained");
    }
}
