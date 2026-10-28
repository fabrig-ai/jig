//! Period inputs and their durable output records, through the child entry points.
use super::revisions::{Harness, LIMITS, to};
use crate::{Domain, Event, Funder, PeriodTimes, Request, Stored, max_out, step};
use skein_lib::{Queue, Wall};

#[test]
fn supplied_period_boundaries_preserve_the_opening_wall_across_duplicates_and_restart() {
    let mut test = Harness::new();
    test.env.wall = Wall::from_nanos(1200);
    test.send(Event::OpenPeriod { reply_to: to(), project: 1, period: 1, budget: 100 });
    let source = Funder::Period { project: 1, period: 1 };
    let first = PeriodTimes { began: test.env.wall, next: None };
    assert_eq!(test.domain.funding(source).unwrap().period, Some(first));
    let next = Some(Wall::from_nanos(1100));
    test.env.wall = Wall::from_nanos(900);
    let out = test.send(Event::PeriodNext { project: 1, period: 1, next });
    let mut saved = false;
    for request in out {
        if let Request::Save { record: Stored::Ledger(row) } = request {
            assert_eq!(row.funder, source);
            assert_eq!(row.period, Some(PeriodTimes { began: first.began, next }));
            saved = true;
        }
    }
    assert!(saved);
    assert!(test.send(Event::PeriodNext { project: 1, period: 1, next }).is_empty());
    test.restart();
    assert_eq!(test.domain.funding(source).unwrap().period, Some(PeriodTimes { began: first.began, next }));
    test.send(Event::PeriodNext { project: 1, period: 1, next: None });
    assert_eq!(test.domain.funding(source).unwrap().period, Some(first));
    test.env.wall = Wall::from_nanos(800);
    test.send(Event::OpenPeriod { reply_to: to(), project: 1, period: 2, budget: 100 });
    assert!(test.send(Event::PeriodNext { project: 1, period: 1, next }).is_empty());
    assert!(test.send(Event::PeriodNext { project: 9, period: 2, next }).is_empty());
    test.restart();
    assert_eq!(
        test.domain.funding(Funder::Period { project: 1, period: 2 }).unwrap().period,
        Some(PeriodTimes { began: Wall::from_nanos(800), next: None })
    );
}

#[test]
fn loaded_ledgers_keep_the_original_opening_facts() {
    let test = Harness::new();
    let mut row = *test.domain.funding(Funder::Period { project: 1, period: 0 }).unwrap();
    let original = row;
    for invalid in [false, true] {
        row = original;
        if invalid {
            row.funder = Funder::Pool { project: 1, period: 0, person: 9 };
            row.parent = Some(original.funder);
        } else {
            row.period = None;
        }
        let mut domain = Domain::new(&LIMITS, 91, 1, crate::tests::charters(), Box::new([0]));
        let mut out = Queue::with_capacity(max_out(&LIMITS));
        step(&mut domain, &test.env, Event::Restore { record: Stored::Ledger(row) }, &mut out);
        assert!(out.is_empty());
        assert_eq!(domain.funding(row.funder), Some(&row));
    }
}

#[test]
fn a_closed_period_leaves_memory_and_saves_under_closed_ledger() {
    let mut test = Harness::new();
    let old = Funder::Period { project: 1, period: 0 };
    let out = test.send(Event::OpenPeriod { reply_to: to(), project: 1, period: 1, budget: 100 });
    assert!(test.domain.funding(old).is_none());
    assert!(!test.stored.contains_key(&crate::Key::Ledger(old)));
    let mut erased = false;
    for request in out {
        if let Request::Erase { key: crate::Key::Ledger(source) } = request {
            erased |= source == old;
        }
    }
    assert!(erased);
    let Some(Stored::ClosedLedger(row)) = test.stored.get(&crate::Key::ClosedLedger(old)) else {
        panic!("the closed period is history")
    };
    assert!(row.closed);
    assert_eq!(row.funder, old);
}

#[test]
fn a_restored_closed_ledger_refuses_the_start() {
    let test = Harness::new();
    let mut row = *test.domain.funding(Funder::Period { project: 1, period: 0 }).unwrap();
    row.closed = true;
    let mut domain = Domain::new(&LIMITS, 91, 1, crate::tests::charters(), Box::new([0]));
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    step(&mut domain, &test.env, Event::Restore { record: Stored::ClosedLedger(row) }, &mut out);
    let mut refused = false;
    while let Some(request) = out.pop() {
        if let Request::RestoreRefused { .. } = request {
            refused = true;
        }
    }
    assert!(refused);
    assert!(!domain.ready());
}

#[test]
fn the_newest_period_stays_live_after_its_pools_close() {
    let mut test = Harness::new();
    let pool = Funder::Pool { project: 1, person: 9, period: 0 };
    test.send(Event::CarvePool { reply_to: to(), project: 1, person: 9, period: 0, budget: 40 });
    test.send(Event::OpenPeriod { reply_to: to(), project: 1, period: 1, budget: 100 });
    assert!(test.domain.funding(pool).is_none());
    assert!(test.stored.contains_key(&crate::Key::ClosedLedger(pool)));
    let newest = test.domain.funding(Funder::Period { project: 1, period: 1 }).unwrap();
    assert!(!newest.closed);
    assert_eq!(newest.numbers.reserved, 0);
}

#[test]
fn a_period_raise_keeps_its_spending_reservations_times_and_made() {
    let mut test = Harness::new();
    let source = Funder::Period { project: 1, period: 0 };
    let row = test.domain.funding.get_mut(&source).unwrap();
    row.numbers = crate::Numbers { budget: 100, spent: 10, spent_below: 20, reserved: 30 };
    row.made = 4;
    row.period = Some(PeriodTimes { began: Wall::from_nanos(100), next: Some(Wall::from_nanos(200)) });
    let mut expected = *row;
    expected.numbers.budget = 200;
    let out = test.send(Event::ResizePeriod { reply_to: to(), project: 1, period: 0, budget: 200 });
    assert_eq!(test.domain.funding(source), Some(&expected));
    assert_eq!(
        out.as_slice(),
        &[Request::Save { record: Stored::Ledger(expected) }, Request::Done { reply_to: to() },]
    );
}

#[test]
fn a_period_resize_admits_exactly_what_is_used() {
    let mut test = Harness::new();
    let source = Funder::Period { project: 1, period: 0 };
    test.domain.funding.get_mut(&source).unwrap().numbers =
        crate::Numbers { budget: 100, spent: 10, spent_below: 20, reserved: 30 };
    let out = test.send(Event::ResizePeriod { reply_to: to(), project: 1, period: 0, budget: 60 });
    assert_eq!(test.domain.funding(source).unwrap().numbers.budget, 60);
    assert_eq!(
        out.as_slice(),
        &[
            Request::Save { record: Stored::Ledger(*test.domain.funding(source).unwrap()) },
            Request::Done { reply_to: to() },
        ]
    );
    let before = *test.domain.funding(source).unwrap();
    let out = test.send(Event::ResizePeriod { reply_to: to(), project: 1, period: 0, budget: 59 });
    assert_eq!(
        out.as_slice(),
        &[Request::Refused { reply_to: to(), problem: crate::Problem::new(None, crate::Refusal::Funding) }]
    );
    assert_eq!(test.domain.funding(source), Some(&before));
}

#[test]
fn an_older_period_cannot_be_resized() {
    let mut test = Harness::new();
    test.make([super::revisions::task(1, &[])].into());
    test.send(Event::OpenPeriod { reply_to: to(), project: 1, period: 1, budget: 100 });
    let source = Funder::Period { project: 1, period: 0 };
    let before = *test.domain.funding(source).unwrap();
    let out = test.send(Event::ResizePeriod { reply_to: to(), project: 1, period: 0, budget: 20_000 });
    assert_eq!(
        out.as_slice(),
        &[Request::Refused { reply_to: to(), problem: crate::Problem::new(None, crate::Refusal::Funding) }]
    );
    assert_eq!(test.domain.funding(source), Some(&before));
}

#[test]
fn an_unopened_period_cannot_be_resized() {
    let mut test = Harness::new();
    let before = *test.domain.funding(Funder::Period { project: 1, period: 0 }).unwrap();
    let out = test.send(Event::ResizePeriod { reply_to: to(), project: 1, period: 9, budget: 100 });
    assert_eq!(
        out.as_slice(),
        &[Request::Refused { reply_to: to(), problem: crate::Problem::new(None, crate::Refusal::Unknown) }]
    );
    assert!(test.domain.funding(Funder::Period { project: 1, period: 9 }).is_none());
    assert_eq!(test.domain.funding(before.funder), Some(&before));
}

#[test]
fn a_period_resize_before_ready_is_refused() {
    let mut test = Harness::new();
    test.domain = Domain::new(&LIMITS, 91, 1, crate::tests::charters(), Box::new([0]));
    let out = test.send(Event::ResizePeriod { reply_to: to(), project: 1, period: 0, budget: 100 });
    assert_eq!(
        out.as_slice(),
        &[Request::Refused { reply_to: to(), problem: crate::Problem::new(None, crate::Refusal::NotReady) }]
    );
    assert!(test.domain.funding(Funder::Period { project: 1, period: 0 }).is_none());
}
