use jig_channel_world::worker_orders;
use jig_protocol_worker_wire as wire;

/// The engine's cancel follows the assignment it names, so the run is
/// cancelled rather than starting after an unrecognized cancel.
#[test]
fn link_records_follow_the_assignment_they_name_under_load() {
    let outcome = worker_orders::run(true);
    assert!(outcome.admission_waits > 0, "worker records waited at admission");
    let assignment =
        outcome.records.iter().position(|kind| *kind == wire::Kind::Assignment.number()).expect("assignment");
    let cancel = outcome.records.iter().position(|kind| *kind == wire::Kind::Cancel.number()).expect("cancel");
    assert!(assignment < cancel);
    assert_eq!(outcome.answers.len(), 1);
    assert!(matches!(outcome.answers[0].ending(), wire::Ending::Failed(failure)
        if matches!(failure.failure(), wire::Failure::Cancelled(_))));
    assert_eq!(outcome.acknowledgements, 1);
    assert_eq!(outcome.starts, 1);
    assert_eq!(outcome.starts_after_cancel, 0);
}

/// The final turn and encoded result survive the agent's exit, once each.
#[test]
fn a_run_ends_after_its_last_hand_out_under_load() {
    let outcome = worker_orders::run(false);
    assert!(outcome.admission_waits > 0, "worker records waited at admission");
    let turn = outcome.records.iter().position(|kind| *kind == wire::Kind::Turn.number()).expect("turn");
    let answer = outcome.records.iter().position(|kind| *kind == wire::Kind::Answer.number()).expect("answer");
    assert!(turn < answer);
    assert_eq!(outcome.turns.len(), 1);
    assert_eq!(outcome.turns[0].number(), 1);
    assert_eq!(outcome.answers.len(), 1);
    assert!(matches!(outcome.answers[0].ending(), wire::Ending::Ended(_)));
    assert_eq!(outcome.acknowledgements, 1);
}
