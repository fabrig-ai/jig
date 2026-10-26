//! The switch gate reads the child's completed check (domain/engine.md, 6).
use super::*;

fn at_switch() -> (Core, Limits) {
    let (mut core, limits) = core();
    core.restart = restart::Restart::Waiting(RestartStep::Switch);
    (core, limits)
}

#[test]
fn a_switch_without_the_childs_check_refuses_the_start() {
    let (mut core, _) = at_switch();
    assert!(!core.restart_checked());
    assert_eq!(core.restart_failure(), Some(RestartStep::Switch));
}

#[test]
fn a_switch_after_the_childs_check_admits_the_start() {
    let (mut core, limits) = at_switch();
    let _requests =
        step(&mut core, &Env { now: Time::ZERO, wall: Wall::EPOCH, limits }, Event::Tasks(tasks::Event::Check));
    assert!(core.restart_checked());
    assert_eq!(core.restart_step(), Some(RestartStep::Switch));
    assert_eq!(core.restore_failure(), None);
}

#[test]
fn checking_at_another_restart_step_refuses_that_step() {
    let (mut core, limits) = at_switch();
    let _requests =
        step(&mut core, &Env { now: Time::ZERO, wall: Wall::EPOCH, limits }, Event::Tasks(tasks::Event::Check));
    core.restart = restart::Restart::Waiting(RestartStep::AdoptRuns);
    assert!(!core.restart_checked());
    assert_eq!(core.restart_failure(), Some(RestartStep::AdoptRuns));
}
