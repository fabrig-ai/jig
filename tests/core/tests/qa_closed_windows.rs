//! Independent QA from engine.md 7.2, through real store closing.
#[path = "support/fresh_windows.rs"]
mod windows;

use jig_core_world::{effects, fixtures};
use jig_domain_core as core;
use jig_domain_tasks as tasks;
use skein_lib::Duration;

fn wake(world: &mut effects::World) {
    world.say(87);
}
fn unchanged(world: &effects::World, tries: tasks::Tries, refused: u32) {
    assert_eq!(windows::tries(world), tries, "closing spends no try");
    assert_eq!(fixtures::live_row(world, windows::proof(world).task).refusals, refused);
}
fn closed(world: &effects::World) {
    assert_eq!(windows::proof(world).window, Some(core::CurrentWindow { attempt: 2, turn: 1, closed: true }));
}

#[test]
fn closed_window_starts_fresh_with_only_its_tail() {
    let mut world = windows::parked();
    let tries = windows::tries(&world);
    let task = windows::proof(&world).task;
    let refused = fixtures::live_row(&world, task).refusals;
    windows::restart(&mut world, windows::AFTER);
    closed(&world);
    wake(&mut world);
    windows::fresh(&world);
    assert_eq!(windows::tail(&world), Some(windows::TAIL));
    unchanged(&world, tries, refused);
}

#[test]
fn repeated_restart_retains_closed_window_and_tail() {
    let mut world = windows::parked();
    let tries = windows::tries(&world);
    windows::restart(&mut world, windows::AFTER);
    windows::restart(&mut world, windows::AFTER);
    closed(&world);
    wake(&mut world);
    windows::fresh(&world);
    assert_eq!(windows::tail(&world), Some(windows::TAIL));
    assert_eq!(windows::tries(&world), tries);
}

#[test]
fn unread_closed_tail_starts_without_counting_a_failure() {
    let mut world = windows::parked();
    let task = windows::proof(&world).task;
    let tries = windows::tries(&world);
    let refused = fixtures::live_row(&world, task).refusals;
    windows::restart(&mut world, windows::AFTER);
    world.store.unread_transcript(task);
    world.skip_restart_oracle("this fixture deliberately makes its committed transcript unread");
    let assignments = world.assignments.len();
    wake(&mut world);
    windows::fresh(&world);
    assert_eq!(windows::tail(&world), None);
    assert_eq!(world.assignments.len(), assignments + 1);
    let attempt = world.assigned.as_ref().expect("immediate assignment").attempt;
    world.advance(Duration::from_secs(1));
    assert_eq!(world.assigned.as_ref().expect("same assignment").attempt, attempt);
    assert_eq!(world.assignments.len(), assignments + 1);
    unchanged(&world, tries, refused);
}

#[test]
fn transient_ending_keeps_closed_tail_for_the_retry() {
    let mut world = windows::parked();
    // End the attempt the full upgrade stops as transient instead of parked.
    world.say(88);
    world.end(tasks::End::Failed(tasks::Class::Transient), 0);
    let mut tries = windows::tries(&world);
    tries.transient = 1; // The scripted transient ending, with no additional upgrade charge.
    let task = windows::proof(&world).task;
    let refused = fixtures::live_row(&world, task).refusals;
    windows::restart(&mut world, windows::AFTER);
    world.retry_due();
    windows::fresh(&world);
    assert_eq!(windows::tail(&world), Some(windows::TAIL));
    unchanged(&world, tries, refused);
}

#[test]
fn first_turn_after_closed_window_opens_a_resumable_conversation() {
    let mut world = windows::parked();
    windows::restart(&mut world, windows::AFTER);
    wake(&mut world);
    let assignment = world.assigned.as_ref().expect("fresh assignment");
    let attempt = assignment.attempt;
    let message = assignment.messages.last().expect("wake message").number;
    world.turn(1, 1, Some(message), &windows::turn(b"fresh conversation"));
    assert_eq!(windows::proof(&world).window, Some(core::CurrentWindow { attempt, turn: 1, closed: false }));
    world.end(tasks::End::Parked, 1);
    windows::restart(&mut world, windows::AFTER);
    world.say(89);
    let assignment = world.assigned.as_ref().expect("resumed assignment");
    assert!(assignment.run.policy.resume);
    assert_eq!(assignment.conversation, attempt);
    assert_eq!(assignment.transcript.len(), 1);
    assert_eq!(assignment.transcript[0], windows::turn(b"fresh conversation"));
}

#[test]
fn closed_tail_loads_past_the_resume_limit() {
    let mut world = windows::parked();
    world.store.patch_sets(windows::AFTER);
    let mut config = effects::fixture(windows::SEED, false).0;
    config.core.settings.resume_bytes = 1;
    world.restart_with(config);
    windows::hello(&mut world);
    world.grant_account(1);
    wake(&mut world);
    windows::fresh(&world);
    assert_eq!(windows::tail(&world), Some(windows::TAIL));
}
