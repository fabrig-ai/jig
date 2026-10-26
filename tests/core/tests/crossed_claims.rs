//! engine.md, 15: a fresh mark stops a live resumed claim before retrying.
#[path = "support/fresh_windows.rs"]
mod fresh_windows;

use fresh_windows::{AFTER, BEFORE, SEED, TAIL};
use jig_core_world::{effects::World, fixtures};
use jig_domain_core as core;
use jig_domain_fleet as fleet;
use jig_domain_tasks as tasks;
use jig_test_domain as root;
use skein_lib::{Duration, Token};

// The world's fleet grace (world::limits); check its two sides one nanosecond apart.
const GRACE: Duration = Duration::from_secs(5);
const TICK: Duration = Duration::from_nanos(1);
const BEFORE_GRACE: Duration = Duration::from_nanos(4_999_999_999);
const WAKE: u8 = 87;
const AFTER_SECOND: jig_protocol_store::PatchSets = jig_protocol_store::PatchSets {
    sets: &[jig_protocol_store::PatchSet { patches: fresh_windows::MARKS, ..AFTER.sets[0] }],
};

fn live_resumed() -> (World, u64, u64) {
    let mut world = fresh_windows::parked();
    let read = world.say(WAKE);
    // A real committed turn in the resumed attempt, with no additional said
    // text: the closed window's rendered tail stays the named TAIL fixture.
    world.turn(1, 1, Some(read), &fresh_windows::turn(b""));
    // Hold time through restart and peer traffic so the grace's two sides
    // are exact; the ordinary scripted drain advances a millisecond per step.
    let (now, wall) = world.clocks();
    world.set_clock(now, wall);
    let assignment = world.assigned.as_ref().expect("live resumed assignment");
    let (task, attempt) = (assignment.task, assignment.attempt);
    (world, task, attempt)
}

fn hosting(world: &World) -> fleet::Hosted {
    let assignment = world.assigned.as_ref().expect("live resumed assignment");
    fleet::Hosted {
        run: Token::new(assignment.task),
        attempt: Token::new(assignment.attempt),
        phase: fleet::Phase::Active,
    }
}

fn lost_once_then_fresh(world: &mut World, task: u64, attempt: u64) {
    assert_eq!(
        fresh_windows::tries(world),
        tasks::Tries { transient: 0, permanent: 0, run: 0, agent: 0, lost: 1, invalid: 0 }
    );
    assert_eq!(fixtures::live_row(world, task).refusals, 0);
    world.retry_due();
    fresh_windows::fresh(world);
    assert_eq!(fresh_windows::tail(world), Some(TAIL));
    assert!(fresh_windows::proof(world).attempt > attempt);
}

fn next_fresh_claim() -> (World, u64, u64) {
    let (mut world, task, _) = live_resumed();
    fresh_windows::restart(&mut world, AFTER);
    world.advance(GRACE);
    world.retry_due();
    let next = fresh_windows::proof(&world).attempt;
    (world, task, next)
}

#[test]
fn a_crossed_claim_listed_at_hello_is_stopped_and_lost() {
    let (mut world, task, attempt) = live_resumed();
    world.restart_with_patch_sets(SEED, false, AFTER);
    let proof = fresh_windows::proof(&world);
    assert!(proof.crossed);
    assert_eq!(proof.window, Some(core::CurrentWindow { attempt: 2, turn: 1, closed: true }));
    let hosted = hosting(&world);
    fresh_windows::hello_hosting(&mut world, Box::new([hosted]));
    world.grant_account(1);
    assert_eq!(world.cancellations, [(task, attempt)]);
    assert_eq!(fresh_windows::tries(&world).lost, 0, "cancelling waits for the worker's answer");
    assert_eq!(world.assignments.last(), Some(&(task, attempt)), "the next run still waits");

    let last_turn = fixtures::live_row(&world, task).turn;
    world.turn(2, 2, None, &fresh_windows::turn(b"late crossed turn"));
    let late = root::Key::Core(core::Key::Core(core::CoreKey::Turn { task, attempt, turn: 2 }));
    assert!(!world.store.rows.contains_key(&late));
    assert_eq!(fixtures::live_row(&world, task).turn, last_turn);
    // The chat fixture takes no resources, so there is no writer slot to
    // exercise; keep that explicit instead of claiming writer coverage.
    assert!(fixtures::live_row(&world, task).holdings.is_empty());
    world.end(tasks::End::Failed(tasks::Class::Agent), 1);
    lost_once_then_fresh(&mut world, task, attempt);
}

#[test]
fn a_crossed_claim_no_worker_lists_is_lost_once_the_grace_passes() {
    let (mut world, task, attempt) = live_resumed();
    fresh_windows::restart(&mut world, AFTER);
    assert!(fresh_windows::proof(&world).crossed);
    assert_eq!(jig_core_world::world::limits().core.fleet.grace, GRACE);
    world.advance(BEFORE_GRACE);
    assert_eq!(fresh_windows::tries(&world).lost, 0);
    assert_eq!(world.assignments.last(), Some(&(task, attempt)));
    world.advance(TICK);
    lost_once_then_fresh(&mut world, task, attempt);
}

#[test]
fn an_in_process_crossed_claim_yet_to_commit_a_turn_is_not_started() {
    let mut world = World::with_patch_sets(SEED, false, BEFORE);
    let (task, attempt) = (world.task(), world.attempt());
    let (now, wall) = world.clocks();
    world.set_clock(now, wall);
    world.store.patch_sets(AFTER);
    let mut config = jig_core_world::effects::fixture(SEED, false).0;
    config.core.workers_in_process = true;
    world.restart_with(config);
    assert!(world.domain.ready());
    assert_eq!(fixtures::live_row(&world, task).refusals, 1);
    assert_eq!(fixtures::live_row(&world, task).tries.lost, 0);
    assert_eq!(world.clocks(), (now, wall), "settlement waits for no grace");
    fresh_windows::hello(&mut world);
    world.grant_account(1);
    world.advance(Duration::from_millis(100));
    assert!(fresh_windows::proof(&world).attempt > attempt, "retry must place a new claim");
    fresh_windows::fresh(&world);
    assert_eq!(fresh_windows::tail(&world), None);
}

#[test]
fn a_crash_after_the_switch_ends_the_crossed_claim_at_the_next_restart() {
    let (mut world, task, attempt) = live_resumed();
    world.switch_patch_sets(SEED, AFTER);
    assert!(fresh_windows::proof(&world).crossed, "crossing survives before adoption");
    // Crash the new root before the durable mark is acknowledged.
    // No worker has listed this attempt, and no grace has elapsed.
    fresh_windows::restart(&mut world, AFTER);
    assert!(fresh_windows::proof(&world).crossed);
    assert_eq!(fresh_windows::tries(&world).lost, 0);
    world.advance(BEFORE_GRACE);
    assert_eq!(fresh_windows::tries(&world).lost, 0);
    world.advance(TICK);
    lost_once_then_fresh(&mut world, task, attempt);
    world.advance(GRACE);
    assert_eq!(fresh_windows::tries(&world).lost, 1, "the old claim is counted once");
}

#[test]
fn the_next_attempt_restarted_before_its_first_turn_is_adopted() {
    let (mut world, task, attempt) = next_fresh_claim();
    let hosted = hosting(&world);
    world.restart_with_patch_sets(SEED, false, AFTER);
    fresh_windows::hello_hosting(&mut world, Box::new([hosted]));
    world.grant_account(1);
    assert_eq!(fixtures::live_row(&world, task).phase, tasks::Phase::Active(tasks::Active::Running { attempt }));
    assert!(!fresh_windows::proof(&world).crossed);
    assert_eq!(fresh_windows::tries(&world).lost, 1);
    assert!(world.cancellations.is_empty());
    world.advance(GRACE);
    assert_eq!(fresh_windows::tries(&world).lost, 1);
    assert_eq!(world.assignments.last(), Some(&(task, attempt)));
}

#[test]
fn a_second_mark_before_the_next_attempts_first_turn_crosses_it_again() {
    let (mut world, task, attempt) = next_fresh_claim();
    fresh_windows::restart(&mut world, AFTER_SECOND);
    assert!(fresh_windows::proof(&world).crossed);
    assert_eq!(fresh_windows::proof(&world).window, None);
    let refusals = fixtures::live_row(&world, task).refusals;
    world.advance(GRACE);
    assert_eq!(fixtures::live_row(&world, task).tries.lost, 1);
    assert_eq!(fixtures::live_row(&world, task).refusals, refusals + 1);
    world.advance(Duration::from_millis(100));
    fresh_windows::fresh(&world);
    assert!(fresh_windows::proof(&world).attempt > attempt);
    // engine.md, 16's default: no current window was committed by the
    // intervening fresh attempt, so its successor loses the old tail.
    assert_eq!(fresh_windows::tail(&world), None);
}

fn writer_slot(world: &World, task: u64, attempt: u64) -> Option<&tasks::WriterSlot> {
    world.store.rows.values().find_map(|row| {
        if let root::Record::Core(core::Record::Tasks(tasks::Stored::Writer(slot))) = row
            && slot.writer == (tasks::Writer::Run { task, attempt })
        {
            Some(slot)
        } else {
            None
        }
    })
}

#[test]
fn a_crossed_claims_writer_slot_waits_for_the_read_afresh() {
    let mut world = World::with_patch_sets(SEED, false, BEFORE);
    fixtures::configure_resources(&mut world);
    let parent = (world.task(), world.attempt());
    let spec = fixtures::resource_member(&world, 1, b"writer across a fresh mark");
    let task = fixtures::delegate(&mut world, parent, 1, spec);
    // Leave only the writer running. Its caller waits for the delegate.
    world.send(root::Event::Answer {
        read: None,
        refused: Box::new([]),
        channel: Token::new(7),
        task: parent.0,
        attempt: parent.1,
        cumulative: 0,
        end: tasks::End::Parked,
    });
    let attempt = world.attempt();
    let (now, wall) = world.clocks();
    world.set_clock(now, wall);
    world.hold_lost_reads = true;
    let hosted = hosting(&world);
    world.restart_with_patch_sets(SEED, false, AFTER);
    fresh_windows::hello_hosting(&mut world, Box::new([hosted]));
    world.grant_account(1);
    let reads = world.systems[0].reads().len();
    let assignments = world.assignments.len();
    assert!(fresh_windows::proof(&world).crossed);
    assert_eq!(world.cancellations, [(task, attempt)]);
    world.end(tasks::End::Failed(tasks::Class::Agent), 0);
    assert_eq!(fixtures::live_row(&world, task).tries.lost, 1);
    assert!(writer_slot(&world, task, attempt).expect("lost run retains its slot").lost);
    assert_eq!(world.pending_lost_reads.len(), 1, "one resource needs a fresh read");
    // Let the retry become due while the connector's answer is held.
    world.advance(Duration::from_secs(1));
    assert_eq!(world.assignments.len(), assignments, "no replacement can write before the fresh read");
    assert_eq!(world.systems[0].reads().len(), reads);
    assert!(writer_slot(&world, task, attempt).is_some());
    world.finish_lost_reads();
    world.retry_due();
    assert_eq!(world.systems[0].reads().len(), reads + 1);
    assert!(writer_slot(&world, task, attempt).is_none());
    assert_eq!(world.task(), task);
    assert!(world.attempt() > attempt, "the replacement is placed after the fresh read");
    assert!(writer_slot(&world, task, world.attempt()).is_some());
    fresh_windows::fresh(&world);
}
