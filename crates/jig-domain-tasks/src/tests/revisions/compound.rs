use super::*;
use crate::{Active, Operation, Phase, Problem, Refusal};
use skein_lib::List;

const ROOT: u64 = 1;
const FIRST: u64 = 2;
const SECOND: u64 = 3;
const REQUESTER: u64 = 9;
const PROJECT: u32 = 1;
const OTHER_PROJECT: u32 = 2;
const FIRST_MESSAGE: u64 = 1;
const SECOND_MESSAGE: u64 = 2;
const OTHER_PARTY: u64 = 10;
const HOLDS: [Hold; 3] = [Hold::Placed { hold: REQUESTER }, Hold::Placed { hold: OTHER_PARTY }, Hold::Budget];

pub(super) fn compound(test: &mut Harness, operations: Vec<Operation>, pool_budget: u64) -> Vec<Request> {
    let mut bounded = List::with_capacity(u32::try_from(operations.len()).unwrap());
    for operation in operations {
        bounded.push(operation).unwrap();
    }
    test.send(Event::Compound {
        reply_to: to(),
        by: Party::Person(REQUESTER),
        project: PROJECT,
        operations: bounded,
        period: 0,
        pool_budget,
        period_budget: 10_000,
    })
}

fn cancel(task: u64) -> Operation {
    Operation::Cancel { task, reason: b"done".as_slice().into(), grace: None, message: None }
}

fn refused(out: Vec<Request>, at: u32, task: u64, why: Refusal) {
    assert_eq!(out, vec![Request::CompoundRefused { reply_to: to(), at, problem: Problem::new(Some(task), why) }]);
}

#[test]
fn a_refused_operation_leaves_the_operations_before_it_unmade() {
    let mut test = chat::cohort();
    let before = test.stored.clone();
    let out = compound(
        &mut test,
        vec![cancel(FIRST), Operation::Release { task: SECOND, again: None, message: None }],
        1_000,
    );
    refused(out, 1, SECOND, Refusal::State);
    assert_eq!(test.stored, before);
}

#[test]
fn a_compound_makes_its_operations_in_order_in_one_step() {
    let mut test = chat::cohort();
    let first = Amendment {
        spec: Some(Spec {
            asks: false,
            words: b"first".as_slice().into(),
            parameters: Box::new([]),
            inputs: Box::new([]),
        }),
        ..amendment()
    };
    let second = Amendment {
        spec: Some(Spec {
            asks: false,
            words: b"second".as_slice().into(),
            parameters: Box::new([]),
            inputs: Box::new([]),
        }),
        ..amendment()
    };
    compound(
        &mut test,
        vec![
            Operation::Amend { task: FIRST, message: FIRST_MESSAGE, stop_run: false, amendment: Box::new(first) },
            Operation::Amend { task: FIRST, message: SECOND_MESSAGE, stop_run: false, amendment: Box::new(second) },
        ],
        1_000,
    );
    let row = test.domain.task(FIRST).unwrap();
    assert_eq!(row.spec.words.as_ref(), b"second");
    assert_eq!(row.revisions.spec, 2);
    assert_eq!(row.inbox.len(), 1);
    assert_eq!(row.last_message, SECOND_MESSAGE);
}

#[test]
fn an_operation_on_a_task_an_earlier_cancel_closed_is_refused() {
    let mut test = chat::cohort();
    let before = test.stored.clone();
    let out = compound(&mut test, vec![cancel(ROOT), Operation::Stop { task: FIRST, hold: 100 + FIRST }], 1_000);
    refused(out, 1, FIRST, Refusal::State);
    assert_eq!(test.stored, before);
}

#[test]
fn moves_count_one_pool_across_the_request() {
    let mut test = chat::cohort();
    let before = test.stored.clone();
    let out = compound(
        &mut test,
        vec![
            Operation::Move { task: FIRST, reason: Box::new([]) },
            Operation::Move { task: SECOND, reason: Box::new([]) },
        ],
        150,
    );
    refused(out, 1, SECOND, Refusal::Funding);
    assert_eq!(test.stored, before);
}

#[test]
fn an_amendment_then_a_release_of_one_held_task_are_made_together() {
    let mut test = chat::cohort();
    test.send(place(FIRST, REQUESTER, REQUESTER));
    let change = Amendment {
        spec: Some(Spec {
            asks: false,
            words: b"changed".as_slice().into(),
            parameters: Box::new([]),
            inputs: Box::new([]),
        }),
        ..amendment()
    };
    compound(
        &mut test,
        vec![
            Operation::Amend { task: FIRST, message: FIRST_MESSAGE, stop_run: false, amendment: Box::new(change) },
            Operation::Release { task: FIRST, again: None, message: None },
        ],
        1_000,
    );
    let row = test.domain.task(FIRST).unwrap();
    assert_eq!(row.spec.words.as_ref(), b"changed");
    assert_eq!(row.phase, Phase::Active(Active::Due));
    assert_eq!(row.revisions.spec, 1);
}

#[test]
fn words_then_a_stop_release_in_one_request_wake_the_task() {
    let mut test = chat::cohort();
    test.send(place(FIRST, REQUESTER, REQUESTER));
    let word = crate::Word {
        number: FIRST_MESSAGE,
        from: Party::Person(REQUESTER),
        kind: MessageKind::Words,
        words: b"continue".as_slice().into(),
        at: test.env.wall,
        hits: 1,
        eligible: true,
    };
    compound(
        &mut test,
        vec![Operation::Message { task: FIRST, word }, Operation::Release { task: FIRST, again: None, message: None }],
        1_000,
    );
    let row = test.domain.task(FIRST).unwrap();
    assert_eq!(row.phase, Phase::Active(Active::Due));
    assert_eq!(row.inbox[0].words.as_ref(), b"continue");
    assert_eq!(row.last_message, FIRST_MESSAGE);
}

#[test]
fn an_operation_on_another_projects_task_is_refused_at_its_place() {
    let mut test = Harness::new();
    test.make(vec![task(ROOT, &[])]);
    test.send(Event::OpenPeriod { reply_to: to(), project: OTHER_PROJECT, period: 0, budget: 10_000 });
    let mut other = task(SECOND, &[]);
    other.project = OTHER_PROJECT;
    other.funder = Funder::Period { project: OTHER_PROJECT, period: 0 };
    test.make(vec![other]);
    let before = test.stored.clone();
    let out = compound(
        &mut test,
        vec![Operation::Stop { task: ROOT, hold: 100 + ROOT }, Operation::Stop { task: SECOND, hold: 100 + SECOND }],
        1_000,
    );
    refused(out, 1, SECOND, Refusal::Project);
    assert_eq!(test.stored, before);
}

#[test]
fn words_alone_keep_the_hold_and_its_revision() {
    for why in HOLDS {
        let mut test = chat::cohort();
        match why {
            Hold::Placed { hold } => {
                test.send(place(FIRST, hold, hold));
            }
            _ => {
                test.send(Event::Hold { task: FIRST, why });
            }
        }
        let before = test.domain.task(FIRST).unwrap().clone();
        let out = test.send(Event::AuthorizedMessage {
            reply_to: to(),
            project: PROJECT,
            task: FIRST,
            word: crate::Word {
                number: FIRST_MESSAGE,
                from: Party::Person(REQUESTER),
                kind: MessageKind::Words,
                words: b"continue".as_slice().into(),
                at: test.env.wall,
                hits: 1,
                eligible: true,
            },
        });
        let row = test.domain.task(FIRST).unwrap();
        assert_eq!(row.phase, before.phase, "{why:?}");
        assert_eq!(row.revisions.hold, before.revisions.hold, "{why:?}");
        assert_eq!(row.revision, before.revision, "{why:?}");
        assert_eq!(row.inbox[0].words.as_ref(), b"continue", "{why:?}");
        assert!(
            out.iter().any(|request| match request {
                Request::Sent { .. } => true,
                _ => false,
            }),
            "{why:?}"
        );
    }
}

#[test]
fn a_stop_after_a_move_uses_the_moved_tree() {
    let mut test = chat::cohort();
    let out = compound(
        &mut test,
        vec![Operation::Move { task: FIRST, reason: Box::new([]) }, Operation::Stop { task: ROOT, hold: 91 }],
        1_000,
    );
    assert!(out.iter().any(|request| match request {
        Request::Done { .. } => true,
        _ => false,
    }));
    assert_eq!(test.domain.task(FIRST).unwrap().placed, None);
    assert_eq!(test.domain.task(ROOT).unwrap().placed, Some(91));
    assert_eq!(test.domain.task(SECOND).unwrap().placed, Some(91));
}

#[test]
fn moving_a_held_delegate_cannot_escape_a_standing_scope() {
    let mut test = chat::cohort();
    let before = test.stored.clone();
    let out = compound(
        &mut test,
        vec![Operation::Stop { task: ROOT, hold: 91 }, Operation::Move { task: FIRST, reason: Box::new([]) }],
        1_000,
    );
    refused(out, 1, FIRST, Refusal::Placed { hold: 91 });
    assert_eq!(test.stored, before);
}

#[test]
fn moving_a_stopped_scope_root_preserves_its_hold() {
    let mut test = chat::cohort();
    let out = compound(
        &mut test,
        vec![Operation::Stop { task: FIRST, hold: 91 }, Operation::Move { task: FIRST, reason: Box::new([]) }],
        1_000,
    );
    assert!(out.iter().any(|request| match request {
        Request::Done { .. } => true,
        _ => false,
    }));
    assert_eq!(test.domain.task(FIRST).unwrap().placed, Some(91));
    assert_eq!(test.domain.task(FIRST).unwrap().requester, Party::Person(REQUESTER));
}

#[test]
fn a_compound_that_changes_a_tasks_placed_hold_twice_is_refused_naming_the_second() {
    let mut test = chat::cohort();
    let before = test.stored.clone();
    let out = compound(
        &mut test,
        vec![Operation::Stop { task: ROOT, hold: 100 }, Operation::Release { task: ROOT, again: None, message: None }],
        1_000,
    );
    refused(out, 1, ROOT, Refusal::Placed { hold: 100 });
    assert_eq!(test.stored, before);

    test.make(vec![task(7, &[])]);
    test.send(place(7, REQUESTER, 101));
    let out = compound(
        &mut test,
        vec![Operation::Stop { task: ROOT, hold: 100 }, Operation::Release { task: 7, again: None, message: None }],
        1_000,
    );
    assert!(out.iter().any(|request| match request {
        Request::Done { .. } => true,
        _ => false,
    }));
    assert_eq!(test.domain.task(ROOT).unwrap().placed, Some(100));
    assert_eq!(test.domain.task(7).unwrap().placed, None);
}

#[test]
fn repeated_subtree_placement_names_the_earlier_hold_before_capacity() {
    let mut test = chat::cohort();
    test.env.limits.placed_holds = 1;
    let before = test.stored.clone();
    let out = compound(
        &mut test,
        vec![Operation::Stop { task: ROOT, hold: 100 }, Operation::Stop { task: FIRST, hold: 101 }],
        1_000,
    );
    refused(out, 1, FIRST, Refusal::Placed { hold: 100 });
    assert_eq!(test.stored, before);
}
