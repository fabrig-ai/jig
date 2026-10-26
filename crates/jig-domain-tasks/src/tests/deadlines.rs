//! Scheduler observations are pure over timers restored through public entrances.
#![allow(
    clippy::disallowed_macros,
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::wildcard_enum_match_arm,
    clippy::match_like_matches_macro,
    reason = "entry-point fixtures are ordinary Rust under programming-model.md section 10.2"
)]
use super::revisions::{Harness, LIMITS, task};
use crate::{
    Active, Domain, Escalation, EscalationHolder, Event, Grace, Hold, HoldKind, Holding, Key, Kind, MessageKind, Name,
    Party, Phase, Proposal, ProposalAction, ProposalHolder, ProposalState, Request, RetryNotice, Stored, Subscription,
    SubscriptionKind, Taken, WakeRule, Was, Word,
};
use skein_lib::{Duration, Time, Wall};

fn deadlines(first: usize) -> Harness {
    let mut h = Harness::new();
    h.make((1..=7).map(|n| task(n, &[])).collect());
    h.env.now = Time::from_nanos(100);
    h.env.wall = Wall::from_nanos(1000);
    let spans: Vec<_> = (0..7).map(|i| Duration::from_nanos(if i == first { 10 } else { 50 })).collect();
    h.env.limits.proposal_stall = spans[4];
    h.env.limits.escalation_stall = spans[5];
    h.env.limits.hold_wait = spans[6];
    let name = Name { connector: 7, path: Box::new([Box::from(&b"wait"[..])]) };
    for (i, span) in spans.into_iter().enumerate() {
        let number = u64::try_from(i).unwrap().checked_add(1).unwrap();
        let mut row = h.domain.task(number).unwrap().clone();
        let at = Wall::from_nanos(h.env.wall.as_nanos().checked_add(span.as_nanos()).unwrap());
        match i {
            0 => {
                row.phase = Phase::Active(Active::BackingOff { until: at });
                row.holdings = Box::new([Holding::Write { resource: name.clone(), kind: 1 }]);
                row.refusals = 1;
                row.retry_notice = Some(RetryNotice::Refused { count: 1 });
            }
            1 => {
                row.last_message = 1;
                row.grace = Some(Grace {
                    by: Party::Person(9),
                    until: at,
                    reason: b"Chat finish grace expired.".as_slice().into(),
                    message: 1,
                });
            }
            2 => {
                row.subscriptions =
                    Box::new([Subscription { number: 1, kind: SubscriptionKind::Timer { at, period: None } }]);
            }
            3 => {
                row.phase = Phase::Active(Active::Idle);
                row.wake.words = WakeRule::Batch { count: 2, age: span };
                row.last_message = 1;
                row.inbox = Box::new([Word {
                    number: 1,
                    from: Party::Task(1),
                    kind: MessageKind::Words,
                    words: Box::new([1]),
                    at: h.env.wall,
                    hits: 1,
                    eligible: false,
                }]);
            }
            4 => {
                row.revision = 1;
                row.proposal = Some(Box::new(Proposal {
                    number: 1,
                    proposer: number,
                    project: 1,
                    action: ProposalAction::Release { task: 1 },
                    reason: Box::new([]),
                    as_holder: false,
                    state: ProposalState::Pending { holder: ProposalHolder::Person(9), since: h.env.wall },
                }));
            }
            5 => {
                row.phase = Phase::Held { was: Was::Active(Active::Due), why: Hold::Stopped, source: None };
                row.revisions.hold = 1;
                row.escalation = Escalation::Waiting {
                    revision: 1,
                    holder: EscalationHolder::Person(9),
                    entry: 1,
                    since: h.env.wall,
                };
            }
            6 => {
                row.phase = Phase::Waiting;
                row.holds_taken = false;
                row.hold_wait_since = Some(h.env.wall);
                row.holdings = Box::new([Holding::Write { resource: name.clone(), kind: 1 }]);
            }
            _ => unreachable!(),
        }
        h.stored.insert(Key::Live(number), Stored::Live(Box::new(row)));
    }
    restore(h)
}

fn restore(mut h: Harness) -> Harness {
    let rows: Vec<_> = h
        .stored
        .values()
        .filter(|r| match r {
            Stored::Live(_) | Stored::Ledger(_) => true,
            _ => false,
        })
        .cloned()
        .collect();
    h.domain = Domain::new(&LIMITS, 91, 1, crate::tests::charters(), Box::new([0]));
    h.send(Event::Kinds {
        connector: 7,
        kinds: Box::new([Kind { connector: 7, kind: 1, hold: HoldKind::Exclusive { taken: Taken::Waits } }]),
    });
    for row in rows {
        assert!(!h.send(Event::Restore { record: row }).iter().any(|r| match r {
            Request::RestoreRefused { .. } => true,
            _ => false,
        }));
    }
    let out = h.send(Event::Restored);
    assert!(
        !out.iter().any(|r| match r {
            Request::RestoreRefused { .. } => true,
            _ => false,
        }),
        "{out:?}"
    );
    assert!(h.domain.ready());
    h
}

#[test]
fn an_empty_task_scheduler_has_no_deadline_before_or_after_restore() {
    let h = Harness::new();
    assert_eq!(Domain::new(&LIMITS, 91, 1, crate::tests::charters(), Box::new([0])).next_deadline(), None);
    assert_eq!(h.domain.next_deadline(), None);
}

#[test]
fn each_of_seven_task_timer_owners_can_supply_the_pure_earliest_deadline() {
    for first in 0..7 {
        let mut h = deadlines(first);
        let before: Vec<_> = (1..=7).map(|n| h.domain.task(n).unwrap().clone()).collect();
        assert_eq!(h.domain.next_deadline(), Some(Time::from_nanos(110)), "timer set {first}");
        h.env.now = Time::from_nanos(110);
        for _ in 0_u32..3 {
            assert_eq!(h.domain.next_deadline(), Some(h.env.now), "observing a due timer does not expire it");
        }
        assert_eq!((1..=7).map(|n| h.domain.task(n).unwrap().clone()).collect::<Vec<_>>(), before);
        assert!(h.domain.next_deadline().unwrap() < Time::from_nanos(150));
    }
}

const TASK_WALL: Wall = Wall::from_nanos(1100);

fn running_at_a_wall() -> Harness {
    use super::revisions::to;
    let mut h = Harness::new();
    h.env.now = Time::from_nanos(100);
    h.env.wall = Wall::from_nanos(1000);
    let mut new = task(1, &[]);
    new.authority.budget.deadline = Some(TASK_WALL);
    h.make(vec![new]);
    h.send(Event::Prepare { reply_to: to(), task: 1 });
    h.send(Event::Claim {
        reply_to: to(),
        account: None,
        task: 1,
        attempt: 1,
        resumed: false,
        budget: 10,
        writes: Box::new([]),
    });
    h.send(Event::Started { task: 1, attempt: 1 });
    assert_eq!(h.domain.task(1).unwrap().phase, Phase::Active(Active::Running { attempt: 1 }));
    h
}

fn parked_at_a_wall() -> Harness {
    let mut h = running_at_a_wall();
    h.send(Event::Activation {
        reply_to: super::revisions::to(),
        task: 1,
        attempt: 1,
        end: crate::End::Parked,
        cause: crate::Cause::Unpriced,
        saved: None,
    });
    assert_eq!(h.domain.task(1).unwrap().phase, Phase::Active(Active::Idle));
    h
}

fn fire(h: &mut Harness, nanos: u64) -> Vec<Request> {
    h.env.now = Time::from_nanos(nanos.checked_sub(900).unwrap());
    h.env.wall = Wall::from_nanos(nanos);
    let mut out = skein_lib::Queue::with_capacity(crate::max_out(&h.env.limits));
    crate::fire(&mut h.domain, &h.env, &mut out);
    let mut requests = Vec::new();
    while let Some(request) = out.pop() {
        requests.push(request);
    }
    requests
}

#[test]
fn a_parked_task_is_made_due_just_past_its_wall() {
    let mut h = parked_at_a_wall();
    assert_eq!(h.domain.next_deadline(), Some(Time::from_nanos(201)));
    assert!(fire(&mut h, 1100).is_empty());
    assert_eq!(h.domain.task(1).unwrap().phase, Phase::Active(Active::Idle));
    let out = fire(&mut h, 1101);
    assert_eq!(h.domain.task(1).unwrap().phase, Phase::Active(Active::Due));
    assert_eq!(
        out.iter()
            .filter(|r| match r {
                Request::Activate { .. } => true,
                _ => false,
            })
            .count(),
        1
    );
    assert_eq!(h.domain.next_deadline(), None);
    assert!(fire(&mut h, 1102).is_empty());
}

#[test]
fn a_running_task_is_not_woken_at_its_wall() {
    let mut h = running_at_a_wall();
    assert_eq!(h.domain.next_deadline(), None);
    assert!(fire(&mut h, 1101).is_empty());
    assert_eq!(h.domain.task(1).unwrap().phase, Phase::Active(Active::Running { attempt: 1 }));
}

#[test]
fn restoring_a_parked_task_rearms_its_wall() {
    let mut h = parked_at_a_wall();
    h.env.now = Time::from_nanos(500);
    h.env.wall = Wall::from_nanos(1050);
    let mut h = restore(h);
    assert_eq!(h.domain.next_deadline(), Some(Time::from_nanos(551)));
    h.env.now = Time::from_nanos(550);
    h.env.wall = TASK_WALL;
    let mut out = skein_lib::Queue::with_capacity(crate::max_out(&h.env.limits));
    crate::fire(&mut h.domain, &h.env, &mut out);
    assert_eq!(h.domain.task(1).unwrap().phase, Phase::Active(Active::Idle));
    h.env.now = Time::from_nanos(551);
    h.env.wall = Wall::from_nanos(1101);
    crate::fire(&mut h.domain, &h.env, &mut out);
    assert_eq!(h.domain.task(1).unwrap().phase, Phase::Active(Active::Due));
    assert!(core::iter::from_fn(|| out.pop()).any(|r| match r {
        Request::Activate { .. } => true,
        _ => false,
    }));
}

#[test]
fn backoff_arms_the_earlier_of_retry_and_wall() {
    for until in [1050, 1150] {
        let mut h = parked_at_a_wall();
        let mut row = h.domain.task(1).unwrap().clone();
        row.phase = Phase::Active(Active::BackingOff { until: Wall::from_nanos(until) });
        row.refusals = 1;
        row.retry_notice = Some(RetryNotice::Refused { count: 1 });
        h.stored.insert(Key::Live(1), Stored::Live(Box::new(row)));
        let mut h = restore(h);
        let wake = until.min(1101);
        assert_eq!(h.domain.next_deadline(), Some(Time::from_nanos(wake.checked_sub(900).unwrap())));
        fire(&mut h, wake);
        assert_eq!(h.domain.task(1).unwrap().phase, Phase::Active(Active::Due));
    }
}
