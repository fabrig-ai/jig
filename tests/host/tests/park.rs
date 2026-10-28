use jig_domain_host::{Ending, Failure, Reason, RunFailure};
use jig_host_world::turn_world::World;
use skein_lib::Token;

#[test]
fn a_run_parked_mid_turn_parks_after_its_turn_is_kept() {
    let mut world = World::new();
    world.assign(true);
    world.park();
    assert_eq!(world.stats().stops, 0);
    world.turn_with(1, b"selected turn", Some(Token::new(17)), false);
    assert!(!world.reading());
    assert_eq!(world.stats().agent_acks, 0);
    world.message_from();
    assert!(world.received_message().is_none(), "no reader input past the selected turn");
    assert!(world.answer().is_none());
    assert_eq!(world.stats().saves, 0);
    world.commit_turn(1);
    let answer = world.answer().expect("committed parked answer");
    assert!(matches!(answer.ending, Ending::Parked { .. }));
    assert_eq!((answer.turns, answer.spent, answer.read), (1, 17, Some(Token::new(17))));
    assert_eq!(world.stats().failures, 0);
    assert_eq!((world.stats().saves, world.stats().releases), (1, 1));
    world.acknowledge_answer();
    world.assert_settled();
}

#[test]
fn a_waiting_run_parks_once_its_turns_are_kept() {
    let mut world = World::new();
    world.assign(false);
    world.turn(1, b"one");
    world.turn(2, b"two");
    world.yield_run();
    world.park();
    assert_eq!(world.stats().stops, 1);
    world.commit_turn(2);
    assert!(world.answer().is_none(), "every retained turn must commit");
    world.commit_turn(1);
    let answer = world.answer().expect("all retained turns committed");
    assert!(matches!(answer.ending, Ending::Parked { .. }));
    assert_eq!((answer.turns, answer.spent), (2, 34));
}

#[test]
fn a_run_parked_while_starting_parks_without_a_turn() {
    let mut world = World::new();
    world.starting();
    world.park();
    assert!(world.answer().is_none());
    world.started();
    let answer = world.answer().expect("start settled");
    assert!(matches!(answer.ending, Ending::Parked { .. }));
    assert_eq!(answer.turns, 0);
}

#[test]
fn a_run_parked_while_preparing_parks_without_starting() {
    let mut world = World::new();
    world.preparing();
    world.park();
    assert!(world.started_state().is_none());
    assert!(matches!(world.answer().expect("prepare settled").ending, Ending::Parked { .. }));
}

#[test]
fn a_run_that_finishes_before_its_park_keeps_its_ending() {
    for failure in [None, Some(RunFailure::Cancelled)] {
        let mut world = World::new();
        world.assign(false);
        world.hold_stop();
        world.park();
        world.turn(1, b"last");
        if let Some(failure) = failure {
            world.fail(1, failure);
        } else {
            world.finish(1, false);
        }
        world.gone();
        world.commit_turn(1);
        let ending = &world.answer().expect("run's own ending").ending;
        assert!(matches!((failure, ending), (None, Ending::Ended { .. }) | (Some(_), Ending::Failed { .. })));
    }
}

#[test]
fn a_cancel_after_a_park_cancels_at_once() {
    let mut world = World::new();
    world.assign(false);
    world.park();
    world.cancel();
    assert_eq!(world.stats().stops, 1);
    assert!(matches!(
        world.answer().expect("immediate cancellation").ending,
        Ending::Failed { failure: Failure::Cancelled(Reason::Engine), .. }
    ));
}

#[test]
fn a_cancel_after_the_selected_turn_does_not_wait_for_commit() {
    let mut world = World::new();
    world.assign(false);
    world.park();
    world.turn(1, b"selected");
    world.cancel();
    assert!(matches!(
        world.answer().expect("cancel releases commit wait").ending,
        Ending::Failed { failure: Failure::Cancelled(Reason::Engine), .. }
    ));
}

#[test]
fn a_park_naming_another_attempt_or_repeated_changes_nothing() {
    let mut world = World::new();
    world.assign(false);
    world.park_attempt(Token::new(933));
    world.turn(1, b"unaffected");
    assert_eq!(world.stats().stops, 0);
    world.park();
    world.park();
    world.turn(2, b"selected");
    world.park();
    assert_eq!(world.stats().stops, 1);
    world.commit_turn(1);
    world.commit_turn(2);
    assert!(matches!(world.answer().expect("parked once").ending, Ending::Parked { .. }));
}

#[test]
fn a_park_after_a_cancel_changes_nothing() {
    let mut world = World::new();
    world.assign(false);
    world.hold_stop();
    world.cancel();
    world.park();
    world.gone();
    assert!(matches!(
        world.answer().expect("cancel wins").ending,
        Ending::Failed { failure: Failure::Cancelled(Reason::Engine), .. }
    ));
}

#[test]
fn a_yield_during_a_pending_park_stops_without_another_turn() {
    let mut world = World::new();
    world.assign(false);
    world.turn(1, b"last");
    world.park();
    world.yield_run();
    assert_eq!(world.stats().stops, 1);
    assert!(world.answer().is_none());
    world.commit_turn(1);
    assert!(matches!(world.answer().expect("yielded park").ending, Ending::Parked { .. }));
}

#[test]
fn a_run_that_already_finished_ignores_a_later_park() {
    let mut world = World::new();
    world.assign(false);
    world.hold_stop();
    world.finish(0, false);
    world.park();
    world.gone();
    assert!(matches!(world.answer().expect("run's ending").ending, Ending::Ended { .. }));
}

#[test]
fn a_runs_own_park_and_accounting_win_while_it_stops() {
    let mut world = World::new();
    world.assign(false);
    world.hold_stop();
    world.turn(1, b"one");
    world.park();
    world.turn(2, b"selected");
    world.finish_with_spend(2, true, 40);
    world.gone();
    world.commit_turn(1);
    world.commit_turn(2);
    let answer = world.answer().expect("own park");
    assert!(matches!(answer.ending, Ending::Parked { .. }));
    assert_eq!(answer.spent, 40, "the agent's last report replaces its previous cumulative report of 34");
}

#[test]
fn a_cancel_while_the_park_saves_keeps_the_cancellation_ending() {
    let mut world = World::new();
    world.assign(true);
    world.hold_save();
    world.park();
    world.turn(1, b"selected");
    world.commit_turn(1);
    assert_eq!(world.stats().saves, 1);
    world.cancel();
    world.saved();
    assert!(matches!(
        world.answer().expect("saved cancellation").ending,
        Ending::Failed { failure: Failure::Cancelled(Reason::Engine), .. }
    ));
}
