//! The start's switch and mark follow durable restores (engine.md, 6).
use jig_core_world::{effects::World, world};
use jig_domain_core as core;
use jig_protocol_store::{Wire, codec, component_v3};
use jig_test_domain as root;
use jig_test_store_files::Fault;
use skein_lib::{Duration, Reader};

#[expect(dead_code, reason = "this story shares the patch sets, not the transcript scenarios")]
#[path = "support/fresh_windows.rs"]
mod fresh_windows;
use fresh_windows as f;

const WIRES: &[Wire] = &[Wire { name: "channel", version: 3 }, Wire { name: "fixture", version: 7 }];
const RESTORES: &[core::RestartStep] = &[
    core::RestartStep::LoadCore,
    core::RestartStep::LoadInbox,
    core::RestartStep::RestoreConnector { connector: 1 },
    core::RestartStep::RestoreConnector { connector: 2 },
    core::RestartStep::Switch,
];

fn before() -> World {
    let mut world = World::with_patch_sets(f::SEED, false, f::BEFORE);
    // Establish the recovered source bytes before comparing generations: a
    // normal open may trim a log's unused tail during recovery (store.md, 10.1).
    world.store.crash(f::SEED);
    world
}

fn at_switch(sets: jig_protocol_store::PatchSets) -> World {
    let mut world = before();
    world.store.wires(WIRES);
    world.hold_switch = true;
    world.restart_with_patch_sets(f::SEED, false, sets);
    world
}

fn mark(world: &World, since: usize) -> &[root::Write] {
    let marks: Vec<_> = world.commits[since..].iter().filter(|writes| writes.contains(&root::Write::Wires)).collect();
    assert_eq!(marks.len(), 1, "one start, one mark");
    marks[0]
}

fn durable_mark(world: World, since: usize, last: u64) {
    assert_eq!(world.store.applied, last + 1, "the mark is the next numbered commit");
    assert_eq!(mark(&world, since).len(), 2, "wires and deployment header alone");
    assert_eq!(world.domain.restart_step(), Some(core::RestartStep::Switch), "no adoption before mark acknowledgement");
    let mut world = world;
    world.release_commits();
    assert!(world.domain.ready());
    let bytes = world.store.into_wires();
    let row = component_v3::Wires::decode(&component_v3::CEILINGS, &mut Reader::new(&bytes)).expect("component wires");
    let wires: Vec<_> = row.wires().iter().map(|wire| (wire.name().to_vec(), wire.version())).collect();
    assert_eq!(wires, [(b"channel".to_vec(), 3), (b"fixture".to_vec(), 7)]);
}

#[test]
fn a_patched_copy_is_restored_then_switched() {
    let mut world = before();
    let source = world.store.generation_contents(b"generations/1");
    world.store.wires(WIRES);
    world.store.patch_sets(f::AFTER);
    world.hold_switch = true;
    let mut config = jig_core_world::effects::fixture(f::SEED, false).0;
    config.core.defaults.templates[0].grace = Some(Duration::from_secs(6));
    world.restart_with(config);
    assert_eq!(world.restart_steps, RESTORES);
    assert_eq!(world.store.file_bytes(b"active"), Some(b"1\n".to_vec()));
    assert_eq!(world.store.generation_contents(b"generations/1"), source);
    assert_eq!(world.store.file_bytes(b"generations/2/manifest"), None);
    assert_eq!(world.domain.settings().newest(), 2, "release revision committed into the copy");
    assert!(world.store.rows.values().any(|row| matches!(row,
        root::Record::Core(core::Record::Core(core::CoreRecord::Settings(core::settings::Record::Revision { history: false, revision })))
        if revision.head.number == 2 && revision.head.by == core::settings::Decider::Release
    )));
    let last = world.store.applied;
    let since = world.commits.len();
    world.store.fault(Fault::Hold { commits: 1 });
    world.finish_switch();
    assert_eq!(world.store.switch_requests, 1);
    assert_eq!(world.store.file_bytes(b"active"), Some(b"2\n".to_vec()));
    let bytes = world.store.file_bytes(b"generations/2/manifest").expect("copy manifest");
    assert_eq!(codec::component::Manifest::decode(&bytes).expect("manifest").store, last);
    assert_eq!(world.store.generation_contents(b"generations/1"), source);
    durable_mark(world, since, last);
}

#[test]
fn a_start_with_nothing_pending_takes_the_switch_step_and_commits_its_wires() {
    let mut world = at_switch(f::BEFORE);
    assert_eq!(world.restart_steps, RESTORES);
    assert!(!world.store.generation_pending);
    let last = world.store.applied;
    let since = world.commits.len();
    world.store.fault(Fault::Hold { commits: 1 });
    world.finish_switch();
    assert_eq!(world.store.switch_requests, 0);
    durable_mark(world, since, last);
}

#[test]
fn a_first_start_takes_the_switch_step() {
    let world = World::first_start(world::config(f::SEED), world::limits());
    let restores = &world.restart_steps[..RESTORES.len()];
    assert_eq!(restores, RESTORES);
    assert_eq!(world.store.switch_requests, 0);
    assert_eq!(mark(&world, 0).len(), 2);
    assert!(world.domain.ready());
    let bytes = world.store.into_wires();
    let row = component_v3::Wires::decode(&component_v3::CEILINGS, &mut Reader::new(&bytes)).expect("empty wires row");
    assert!(row.wires().is_empty(), "an empty shell wire list is still committed");
}

#[test]
fn a_restart_refused_before_the_switch_leaves_the_old_generation_active() {
    let mut world = before();
    let source = world.store.generation_contents(b"generations/1");
    let rows = world.store.rows.clone();
    let last = world.store.applied;
    let mut limits = world::limits();
    limits.core.policy_bytes = 1; // The saved project policy exceeds one byte.
    world.store.patch_sets(f::AFTER);
    let mut config = jig_core_world::effects::fixture(f::SEED, false).0;
    // Projects already exist in the store; this build seeds no new policies.
    config.core.seed_projects = Box::new([]);
    world.restart_with_limits(config, limits);
    assert!(world.stopped);
    assert_eq!(world.domain.restore_failure(), Some(core::RestartRequest::ProjectPolicy { project: 1 }));
    assert!(!world.restart_steps.contains(&core::RestartStep::Switch));
    assert_eq!(world.store.switch_requests, 0);
    assert_eq!(world.store.file_bytes(b"active"), Some(b"1\n".to_vec()));
    assert_eq!(world.store.generation_contents(b"generations/1"), source);
    world.store.patch_sets(f::BEFORE);
    world.store.crash(f::SEED);
    assert_eq!(world.store.applied, last);
    assert_eq!(world.store.rows, rows);
    assert!(!world.store.generation_pending);
}

#[test]
fn a_refused_store_switch_stops_the_start() {
    let mut world = at_switch(f::BEFORE);
    let since = world.commits.len();
    // A stale shell pending flag asks a real nonpending store to switch;
    // its OpenRefused terminal must stop the root through the normal route.
    world.store.generation_pending = true;
    world.finish_switch();
    assert!(world.stopped);
    assert!(!world.domain.ready());
    assert!(world.commits[since..].is_empty(), "no mark after refusal");
}

#[test]
fn a_failed_mark_never_adopts_the_runs() {
    let mut world = at_switch(f::BEFORE);
    let next = world.store.applied + 1;
    world.store.fault(Fault::Fail { commit: next });
    world.finish_switch();
    assert!(world.stopped);
    assert_eq!(world.domain.restart_step(), Some(core::RestartStep::Switch));
    assert_eq!(world.restart_steps, RESTORES);
}
