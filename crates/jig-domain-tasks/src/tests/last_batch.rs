//! The retained batch is the accepted template batch, independently of live membership.
use super::revisions::{Harness, LIMITS, task, to};
use crate::{
    Contract, Control, Domain, Event, Executor, Funder, New, Numbers, Party, RecurringOverlap, RecurringTemplate,
    Request, Stored, max_out, step, task_bytes,
};
use skein_lib::Queue;

fn master() -> New {
    let mut first = task(1, &[]);
    first.numbers.budget = 20;
    first.authority.budget.spend = 20;
    let mut second = first.clone();
    second.number = 2;
    let mut parent = task(100, &[]);
    parent.executor = Executor::Procedure { connector: 1, code: 1 };
    parent.contract = Contract::Report { words: 0 };
    parent.numbers = Numbers { budget: 0, spent: 0, spent_below: 0, reserved: 0 };
    parent.recurring =
        Some(Box::new(RecurringTemplate { key: 1, batch: Box::new([first, second]), overlap: RecurringOverlap::Skip }));
    parent
}

fn accepted() -> Harness {
    let mut test = Harness::new();
    test.send(Event::Make { reply_to: to(), creator: Party::Deployment { project: 1 }, batch: Box::new([master()]) });
    assert!(test.domain.task(100).unwrap().recurring.as_ref().unwrap().last_batch.is_empty());
    test.send(Event::OpenPeriod { reply_to: to(), project: 1, period: 1, budget: 1000 });
    test.send(Event::TickRecurring { task: 100, period: 1 });
    for numbers in [Box::new([0, 11]), Box::new([10, 10]), Box::new([100, 11])] {
        test.send(Event::RecurringBatch { task: 100, period: 1, numbers });
        assert!(test.domain.task(100).unwrap().recurring.as_ref().unwrap().last_batch.is_empty());
        assert!(test.domain.funding(Funder::Recurring { project: 1, task: 100, period: 1 }).is_none());
    }
    test.send(Event::RecurringBatch { task: 100, period: 1, numbers: Box::new([10, 11]) });
    assert!(test.domain.task(10).is_some());
    assert!(test.domain.task(11).is_some());
    assert_eq!(test.domain.task(100).unwrap().recurring.as_ref().unwrap().last_batch.as_ref(), [10, 11]);
    test
}

#[test]
fn the_complete_last_batch_survives_skips_endings_restart_and_rejected_replacement() {
    let mut test = accepted();
    let row = test.domain.task(100).unwrap();
    let mut without = row.clone();
    without.recurring.as_mut().unwrap().last_batch = Box::new([]);
    assert_eq!(task_bytes(row), task_bytes(&without).unwrap().checked_add(16));
    test.send(Event::OpenPeriod { reply_to: to(), project: 1, period: 2, budget: 1000 });
    test.send(Event::TickRecurring { task: 100, period: 2 });
    assert_eq!(test.domain.task(100).unwrap().recurring.as_ref().unwrap().last_period, 2);
    test.send(Event::RecurringBatch { task: 100, period: 2, numbers: Box::new([12, 13]) });
    assert!(test.domain.task(12).is_none());
    test.restart();
    for child in [10, 11] {
        test.send(Event::Control {
            reply_to: to(),
            by: Party::Task(100),
            task: child,
            control: Control::Cancel { reason: Box::new([]), grace: None, message: None },
        });
        test.send(Event::EffectsSettled { task: child });
        test.send(Event::Settled { task: child });
        assert!(test.domain.task(child).is_none());
    }
    assert!(test.domain.task(100).unwrap().delegates.is_empty());
    test.restart();
    assert_eq!(test.domain.task(100).unwrap().recurring.as_ref().unwrap().last_batch.as_ref(), [10, 11]);
    test.send(Event::OpenPeriod { reply_to: to(), project: 1, period: 3, budget: 1000 });
    test.send(Event::TickRecurring { task: 100, period: 3 });
    test.send(Event::RecurringBatch { task: 100, period: 3, numbers: Box::new([12, 12]) });
    assert_eq!(test.domain.task(100).unwrap().recurring.as_ref().unwrap().last_batch.as_ref(), [10, 11]);
    test.send(Event::RecurringBatch { task: 100, period: 3, numbers: Box::new([12, 13]) });
    assert_eq!(test.domain.task(100).unwrap().recurring.as_ref().unwrap().last_batch.as_ref(), [12, 13]);
    test.restart();
    assert_eq!(test.domain.task(100).unwrap().recurring.as_ref().unwrap().last_batch.as_ref(), [12, 13]);
}

#[test]
fn cold_restore_rejects_partial_duplicate_zero_self_and_oversized_last_batches() {
    let test = accepted();
    let original = test.domain.task(100).unwrap();
    let invalid: [Box<[u64]>; 5] = [
        Box::new([10]),
        Box::new([10, 10]),
        Box::new([0, 11]),
        Box::new([100, 11]),
        Box::new([1, 2, 3, 4, 5, 6, 7, 8, 9]),
    ];
    for numbers in invalid {
        let mut row = original.clone();
        row.recurring.as_mut().unwrap().last_batch = numbers;
        let mut domain = Domain::new(&LIMITS, 91, 1, crate::tests::charters(), Box::new([0]));
        let mut out = Queue::with_capacity(max_out(&LIMITS));
        step(&mut domain, &test.env, Event::Restore { record: Stored::Live(Box::new(row)) }, &mut out);
        let mut refused = false;
        while let Some(request) = out.pop() {
            if let Request::RestoreRefused { .. } = request {
                refused = true;
            }
        }
        assert!(refused);
    }
}

#[test]
fn taking_over_a_batch_member_preserves_its_original_batch_identity_after_restart() {
    let mut test = accepted();
    let out = test.send(Event::Move {
        reply_to: to(),
        task: 11,
        person: 9,
        period: 1,
        pool_budget: 100,
        period_budget: 1000,
        reason: Box::new([]),
    });
    let mut moved = false;
    for request in out {
        if let Request::Done { .. } = request {
            moved = true;
        }
    }
    assert!(moved);
    assert_eq!(test.domain.task(11).unwrap().requester, Party::Person(9));
    assert_eq!(test.domain.task(100).unwrap().delegates.as_ref(), [10]);
    assert_eq!(test.domain.task(100).unwrap().recurring.as_ref().unwrap().last_batch.as_ref(), [10, 11]);
    test.restart();
    assert_eq!(test.domain.task(100).unwrap().recurring.as_ref().unwrap().last_batch.as_ref(), [10, 11]);
}
