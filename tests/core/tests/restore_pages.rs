//! Public cold-restore page scheduling (`domain/testing.md`, sections 4 and 7).
use jig_core_world::{
    client_peer::ClientPeer,
    effects::World,
    peers::Peers,
    store_peer::{StorePeer, with_start_mark},
};
use jig_domain_core as core;
use jig_domain_people as people;
use jig_test_domain as root;
use skein_lib::{Duration, ReplyTo, Token};

const SEED: u64 = 645;
const KEY: [u8; 16] = [0x91; 16];
const UTF8_TEXTS: [&[u8]; 3] = [b"\0~^\x01\0!", b"~\0\x01", b"\0^!"];

fn identity() -> people::Identity {
    people::Identity {
        key: people::IdentityKey { provider: 0, subject: Box::from([0, 255, 128, 7]) },
        login: Box::from(UTF8_TEXTS[1]),
        name: Box::from(UTF8_TEXTS[2]),
    }
}

fn sign_in(token: u64) -> root::Event {
    root::Event::Core(core::Event::SignIn {
        held: None,
        reply_to: ReplyTo::new(Token::new(token)),
        identity: identity(),
    })
}

fn ask(world: &World, sign_in: u64, token: u64) -> root::Event {
    root::Event::Core(core::Event::People(people::Event::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(Token::new(token)),
        sign_in,
        key: KEY,
        ask: jig_core_world::fixtures::chat(world, 1, UTF8_TEXTS[0].into()),
    }))
}

fn reply(world: &mut World, token: u64) -> (Option<u64>, people::Reply) {
    let peer = world.client_peer.as_mut().expect("external client peer");
    assert_eq!(peer.delivered.len(), 1, "one externally released answer");
    let (applied, delivery) = peer.delivered.pop().expect("actual release");
    assert!(applied <= world.store.applied, "the answer followed an actually applied boundary");
    let root::Delivery::Core(core::Held::PeopleReply { to, reply }) = delivery else {
        panic!("actual people reply");
    };
    assert_eq!(to.into_token(), Token::new(token), "the original caller's one-answer capability");
    (
        match reply {
            people::Reply::SignedIn { sign_in, .. } => Some(sign_in),
            people::Reply::Signer(_)
            | people::Reply::Full { .. }
            | people::Reply::SignedOut
            | people::Reply::Outcome(_)
            | people::Reply::Again(_)
            | people::Reply::Refused(_) => None,
        },
        reply,
    )
}

fn world() -> (World, u64) {
    let (configuration, limits) = jig_core_world::effects::fixture(SEED, false);
    let mut world = World::scripted(configuration, limits, Peers::new(Vec::new(), Vec::new()));
    assert!(!world.hold_restore_pages, "all constructors default to complete restoration");
    world.client_peer = Some(ClientPeer::default());
    world.send(sign_in(7100));
    let (sign_in, answer) = reply(&mut world, 7100);
    let people::Reply::SignedIn { person, .. } = answer else { panic!("real sign-in {answer:?}") };
    world.send(root::Event::Core(core::Event::People(people::Event::Roles {
        project: 1,
        holdings: Box::new([people::Holding { person, role: people::Role::Owner }]),
    })));
    world.store_peer = Some(StorePeer::after(world.store.applied));
    (world, sign_in.expect("root-issued authenticated sign-in"))
}

fn pause(world: &mut World) {
    world.hold_restore_pages = true;
    world.restart_with(jig_core_world::effects::fixture(SEED, false).0);
    assert!(world.store.loading(), "the requested original store page remains outstanding");
    assert_eq!(world.restart_steps, [core::RestartStep::LoadCore]);
}

#[test]
fn a_paused_restore_answers_an_ask_not_ready_without_a_commit_or_retained_answer() {
    let (mut world, sign_in) = world();
    let rows = world.store.rows.clone();
    let applied = world.store.applied;
    let commits = world.commits.len();
    pause(&mut world);
    world.send(ask(&world, sign_in, 7101));
    assert_eq!(reply(&mut world, 7101).1, people::Reply::Again(people::Again::NotReady));
    assert!(world.store.loading());
    assert_eq!(world.store.applied, applied, "NotReady must not commit");
    assert_eq!(world.commits.len(), commits, "NotReady must not submit a commit");
    assert_eq!(world.store.rows, rows, "no business mutation or retained keyed answer");
    assert!(world.store_peer.as_mut().expect("store peer").take().is_empty());
    world.finish_restore_pages();
    assert_eq!(world.store.rows, with_start_mark(&rows), "whole original restored facts and the start's mark");
    world.send(ask(&world, sign_in, 7101));
    assert!(matches!(reply(&mut world, 7101).1, people::Reply::Outcome(people::Outcome::Created { .. })));
}

#[test]
fn a_paused_restore_answers_sign_in_not_ready_without_changing_durable_rows() {
    let (mut world, _) = world();
    let rows = world.store.rows.clone();
    let applied = world.store.applied;
    let commits = world.commits.len();
    pause(&mut world);
    world.send(sign_in(7102));
    assert_eq!(reply(&mut world, 7102).1, people::Reply::Again(people::Again::NotReady));
    assert_eq!(world.store.applied, applied, "NotReady sign-in must not commit");
    assert_eq!(world.commits.len(), commits, "NotReady sign-in must not submit a commit");
    assert_eq!(world.store.rows, rows, "NotReady sign-in leaves the complete durable store unchanged");
}

#[test]
fn paused_polls_preserve_multiple_whole_pages_and_repeated_release_is_safe() {
    let (mut world, _) = world();
    world.enable_tracing();
    let rows = world.store.rows.clone();
    let core_count = rows.keys().filter(|key| matches!(key, root::Key::Core(_))).count();
    assert!(core_count > 2, "multiple original core store pages");
    pause(&mut world);
    let trace_start = world.trace.len();
    for _ in 0..4 {
        world.drain();
        assert!(world.store.loading());
        assert_eq!(world.restart_steps, [core::RestartStep::LoadCore]);
        assert_eq!(world.store.rows, rows);
    }
    assert!(!world.trace[trace_start..].iter().any(|line| line.starts_with("store page")));
    world.finish_restore_pages();
    assert!(!world.store.loading());
    assert_eq!(world.restart_steps.last(), Some(&core::RestartStep::ReadAfresh { connector: 2 }));
    let rows = with_start_mark(&rows);
    assert_eq!(world.store.rows, rows);
    let pages: Vec<_> =
        world.trace[trace_start..].iter().filter(|line| line.starts_with("store page LoadCore ")).cloned().collect();
    let expected: Vec<_> = (0..core_count.div_ceil(2))
        .map(|page| format!("store page LoadCore {} rows", (core_count - 2 * page).min(2)))
        .collect();
    assert_eq!(pages, expected, "every original page boundary in order");
    let applied = world.store.applied;
    world.finish_restore_pages();
    assert_eq!(world.store.rows, rows);
    assert_eq!(world.store.applied, applied);
    pause(&mut world);
    world.finish_restore_pages();
    assert_eq!(world.store.rows, with_start_mark(&rows));
}

#[test]
fn finishing_held_original_pages_restores_real_authentication_and_admission() {
    let (mut world, sign_in) = world();
    let rows = world.store.rows.clone();
    pause(&mut world);
    world.finish_restore_pages();
    assert_eq!(world.store.rows, with_start_mark(&rows));
    world.send(ask(&world, sign_in, 7105));
    assert!(matches!(reply(&mut world, 7105).1, people::Reply::Outcome(people::Outcome::Created { .. })));
    assert!(world.store.rows.values().any(|row| matches!(row,
        root::Record::Core(core::Record::People(people::Stored::Answer { key, ask, .. }))
        if key.key == KEY && **ask == jig_core_world::fixtures::chat(&world, 1, UTF8_TEXTS[0].into())
    )));
}

#[test]
fn default_restore_replays_the_same_durable_whole_request_without_making_it_twice() {
    let (mut world, sign_in) = world();
    world.send(ask(&world, sign_in, 7103));
    let answer = reply(&mut world, 7103).1;
    assert!(matches!(answer, people::Reply::Outcome(people::Outcome::Created { .. })));
    assert!(world.store.rows.values().any(|row| matches!(row,
        root::Record::Core(core::Record::People(people::Stored::Answer { key, ask, .. }))
        if key.key == KEY && **ask == jig_core_world::fixtures::chat(&world, 1, UTF8_TEXTS[0].into())
    )));
    let rows = world.store.rows.clone();
    let applied = world.store.applied;
    let commits = world.commits.len();
    world.restart_with(jig_core_world::effects::fixture(SEED, false).0);
    assert!(!world.store.loading());
    let rows = with_start_mark(&rows);
    assert_eq!(world.store.rows, rows);
    world.send(ask(&world, sign_in, 7103));
    assert_eq!(reply(&mut world, 7103), (None, answer));
    assert_eq!(world.store.rows, rows);
    assert_eq!(world.store.applied, applied + 1);
    assert_eq!(world.commits.len(), commits + 1);
}

#[test]
fn a_held_restore_expires_sign_ins_at_the_current_external_wall_time() {
    let (mut world, sign_in) = world();
    let rows = world.store.rows.clone();
    let applied = world.store.applied;
    pause(&mut world);
    world.correct_wall_backwards(Duration::from_secs(10));
    world.advance(Duration::from_secs(75));
    assert!(world.store.loading());
    assert_eq!(world.store.rows, rows, "time passing during the pause does not apply store writes");
    assert_eq!(world.store.applied, applied);
    assert!(world.store_peer.as_mut().expect("store peer").take().is_empty());
    let release_at = world.wall_time();
    world.finish_restore_pages();
    let mut expected = rows;
    assert!(expected.remove(&root::Key::Core(core::Key::People(people::Key::SignIn(sign_in)))).is_some());
    let header = expected
        .values_mut()
        .find_map(|row| match row {
            root::Record::Core(core::Record::Core(core::CoreRecord::Deployment(header))) => Some(header),
            root::Record::Core(_) | root::Record::Connector { .. } => None,
        })
        .expect("whole original deployment header");
    assert_eq!(header.commits, applied);
    header.commits += 1;
    assert_eq!(world.store.rows, with_start_mark(&expected), "restore erases the expired sign-in, then marks");
    let snapshots = world.store_peer.as_mut().expect("store peer").take();
    assert_eq!(snapshots.len(), 2);
    assert_eq!(snapshots[0].rows, expected);
    assert_eq!(snapshots[0].commit, applied + 1);
    assert_eq!(snapshots[1].rows, world.store.rows);
    assert_eq!(snapshots[1].commit, world.store.applied);
    assert_eq!(snapshots[1].commit, applied + 2);
    for snapshot in snapshots {
        assert!(snapshot.at.as_nanos() > release_at.as_nanos());
        assert!(snapshot.at.as_nanos() <= world.wall_time().as_nanos());
    }
    world.send(ask(&world, sign_in, 7104));
    assert_eq!(reply(&mut world, 7104).1, people::Reply::SignedOut);
}
