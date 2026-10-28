use jig_core_world::Fault;
use jig_core_world::{
    Store,
    client_peer::ClientPeer,
    effects::{Cut, World},
    store_peer::StorePeer,
};
use jig_domain_core as core;
use jig_domain_people as people;
use jig_test_domain as root;
use jig_test_domain::Write;
use skein_lib::{Duration, ReplyTo, Time, Token, Wall};

fn sign_in(number: u8) -> root::Event {
    root::Event::Core(core::Event::SignIn {
        held: None,
        reply_to: ReplyTo::new(Token::new(7000 + u64::from(number))),
        identity: people::Identity {
            key: people::IdentityKey { provider: 0, subject: Box::from([number, 255]) },
            login: Box::from([number]),
            name: Box::from([number, 0, 255]),
        },
    })
}

fn observed_world(seed: u64) -> World {
    let mut world = World::new(seed, false);
    world.skip_restart_oracle("these store-capture fixtures deliberately use non-UTF-8 identities and opaque turns");
    assert!(world.store_peer.is_none(), "default bootstrap has no capturing peer");
    let mut peer = StorePeer::after(world.store.applied);
    peer.capture(Time::ZERO, &world.store);
    assert!(peer.take().is_empty(), "installing after bootstrap does not reclassify it");
    world.store_peer = Some(peer);
    world.client_peer = Some(ClientPeer::default());
    world
}

fn opaque_turn(bytes: &[u8]) -> root::Record {
    root::Record::Core(core::Record::Core(core::CoreRecord::Turn(core::TurnRecord {
        opens_window: false,
        task: u64::MAX - 1,
        attempt: u64::MAX - 2,
        turn: u32::MAX,
        spent: u64::MAX - 3,
        read: Some(u64::MAX - 4),
        at: Wall::from_nanos(999_999),
        transcript: bytes.into(),
    })))
}

#[test]
fn an_applied_world_transaction_is_captured_before_its_held_client_answer() {
    let mut world = observed_world(641);
    let before = world.store.applied;
    world.cut = Some(Cut::Durable(before + 1));
    world.store.fault(Fault::Hold { commits: 1 });
    world.send(sign_in(11));
    assert_eq!(world.store.applied, before + 1);
    assert!(world.client_peer.as_ref().expect("client").delivered.is_empty());
    assert_eq!(world.store.held.front(), Some(&world.store.applied));
    let snapshots = world.store_peer.as_mut().expect("store peer").take();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].commit, world.store.applied, "the actual numbered durable transaction");
    assert_eq!(snapshots[0].rows, world.store.rows, "the complete externally applied store");
    assert!(snapshots[0].at > Time::ZERO);
    assert!(snapshots[0].at.as_nanos() <= world.wall_time().as_nanos());
    assert_eq!(world.reached_cut, Some(Cut::Durable(world.store.applied)));
    assert_eq!(snapshots[0].at, Time::from_nanos(world.wall_time().as_nanos()), "the exact public applied tick");
}

#[test]
fn repeated_drains_and_released_acknowledgements_do_not_duplicate_a_snapshot() {
    let mut world = observed_world(642);
    world.store.fault(Fault::Hold { commits: 1 });
    world.send(sign_in(12));
    let applied = world.store.applied;
    assert_eq!(world.store_peer.as_mut().expect("store peer").take().len(), 1);
    for _ in 0..3 {
        world.drain();
    }
    assert_eq!(world.store.applied, applied);
    assert!(world.store_peer.as_mut().expect("store peer").take().is_empty());
    world.release_commits();
    assert_eq!(world.store.applied, applied);
    assert_eq!(world.client_peer.as_ref().expect("client").delivered.len(), 1);
    assert!(world.store_peer.as_mut().expect("store peer").take().is_empty());
    let mut empty = StorePeer::new();
    empty.capture(Time::from_nanos(1), &Store::new(&jig_core_world::world::limits(), &[]));
    assert!(empty.take().is_empty(), "no invented Durable0");
}

#[test]
fn a_single_drain_preserves_every_intermediate_applied_number_and_whole_snapshot() {
    let mut world = observed_world(643);
    let before = world.store.applied;
    world.store.fault(Fault::Hold { commits: 3 });
    world.send_all(vec![sign_in(13), sign_in(14), sign_in(15)]);
    let snapshots = world.store_peer.as_mut().expect("store peer").take();
    assert_eq!(snapshots.len(), 3, "three real sign-in transactions in one drain");
    let actual: Vec<_> = world.store.held.iter().copied().collect();
    assert_eq!(snapshots.iter().map(|row| row.commit).collect::<Vec<_>>(), actual);
    assert_eq!(actual, vec![before + 1, before + 2, before + 3]);
    assert!(snapshots.windows(2).all(|pair| pair[0].at < pair[1].at));
    assert!(snapshots.windows(2).all(|pair| pair[0].rows != pair[1].rows));
    assert_eq!(snapshots.last().expect("last").rows, world.store.rows);
    assert!(world.client_peer.as_ref().expect("client").delivered.is_empty());
}

#[test]
fn whole_opaque_rows_remain_immutable_after_later_save_erase_and_store_mutation() {
    let mut store = Store::new(&jig_core_world::world::limits(), &[]);
    let mut peer = StorePeer::new();
    let original = opaque_turn(&[0, 255, 128, 1, 0, 254]);
    let key = original.key();
    store.submit(1, writes(1, Write::Save(original.clone())), 0);
    assert_eq!(store.tick(false), Ok(Some(1)));
    peer.capture(Time::from_nanos(17), &store);
    let first = peer.take().pop().expect("first durable snapshot");
    assert_eq!(first.rows.get(&key), Some(&original));
    let replacement = opaque_turn(b"replacement allocation");
    store.submit(2, writes(2, Write::Save(replacement.clone())), 0);
    assert_eq!(store.tick(false), Ok(Some(2)));
    peer.capture(Time::from_nanos(18), &store);
    store.submit(3, writes(3, Write::Erase(key.clone())), 0);
    assert_eq!(store.tick(false), Ok(Some(3)));
    peer.capture(Time::from_nanos(19), &store);
    let later = peer.take();
    assert_eq!(later[0].rows.get(&key), Some(&replacement));
    assert_eq!(later[1].rows, header_rows(3));
    store.submit(4, writes(4, Write::Save(opaque_turn(b"external mutation"))), 0);
    assert_eq!(store.tick(false), Ok(Some(4)));
    assert_eq!(first.rows.get(&key), Some(&original), "all allocated bytes and scalar facts survived");
    assert_eq!(later[0].rows.get(&key), Some(&replacement));
    assert_eq!(later[1].rows, header_rows(3));
}

#[test]
fn pending_failed_and_abandoned_transactions_emit_nothing_and_crash_preserves_captured_rows() {
    let mut store = Store::new(&jig_core_world::world::limits(), &[]);
    let mut peer = StorePeer::new();
    let durable = opaque_turn(b"actually applied");
    let key = durable.key();
    store.submit(1, writes(1, Write::Save(durable.clone())), 0);
    assert_eq!(store.tick(true), Ok(None));
    peer.capture(Time::from_nanos(20), &store);
    store.submit(2, writes(2, Write::Erase(key.clone())), 2);
    assert_eq!(store.tick(false), Ok(None));
    peer.capture(Time::from_nanos(21), &store);
    store.crash(18);
    peer.capture(Time::from_nanos(22), &store);
    let captured = peer.take();
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].rows.get(&key), Some(&durable));
    store.fault(Fault::Fail { commit: 2 });
    store.submit(2, writes(2, Write::Erase(key.clone())), 0);
    store.submit(3, writes(3, Write::Save(opaque_turn(b"abandoned after failure"))), 0);
    assert_eq!(store.tick(false), Err(2));
    peer.capture(Time::from_nanos(23), &store);
    assert!(peer.take().is_empty());
    assert_eq!(store.applied, 1);
    assert_eq!(store.rows.get(&key), Some(&durable));
    assert!(store.pending.is_empty());
    store.crash(18);
    peer.capture(Time::from_nanos(24), &store);
    assert!(peer.take().is_empty());
    assert_eq!(captured[0].rows.get(&key), Some(&durable));
}

#[test]
fn supplied_monotonic_time_is_exact_and_independent_of_wall_corrections() {
    let mut store = Store::new(&jig_core_world::world::limits(), &[]);
    let mut peer = StorePeer::new();
    let row = opaque_turn(b"independent wall timestamp");
    store.submit(1, writes(1, Write::Save(row)), 0);
    assert_eq!(store.tick(false), Ok(Some(1)));
    peer.capture(Time::from_nanos(7), &store);
    assert_eq!(peer.take()[0].at, Time::from_nanos(7), "no conversion from the row's unrelated Wall");
    let mut ordinary = observed_world(644);
    let mut corrected = observed_world(644);
    corrected.correct_wall_backwards(Duration::from_secs(60));
    ordinary.send(sign_in(16));
    corrected.send(sign_in(16));
    let first = ordinary.store_peer.as_mut().expect("store peer").take();
    let second = corrected.store_peer.as_mut().expect("store peer").take();
    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 1);
    assert_eq!(first[0].at, second[0].at, "Wall corrections never alter the supplied monotonic tick");
    assert_eq!(first[0].commit, second[0].commit);
}

const HEADER: core::Deployment = core::Deployment {
    id: [18; 16],
    tasks: 0,
    people: 0,
    sign_ins: 0,
    messages: 0,
    runs: 0,
    calls: 0,
    connector_rows: 0,
    projects: 0,
    applications: 0,
    commits: 0,
};
fn header(commits: u64) -> root::Record {
    let mut header = HEADER;
    header.commits = commits;
    root::Record::Core(core::Record::Core(core::CoreRecord::Deployment(header)))
}
fn header_rows(commits: u64) -> std::collections::BTreeMap<root::Key, root::Record> {
    let header = header(commits);
    std::collections::BTreeMap::from([(root::Key::Core(core::Key::Core(core::CoreKey::Deployment)), header)])
}
fn writes(number: u64, write: Write) -> Box<[Write]> {
    Box::new([Write::Save(header(number)), write])
}
