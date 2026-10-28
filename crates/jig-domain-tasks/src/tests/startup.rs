//! Whole-set checks precede replay (domain/engine.md, 6; tasks.md, 6.7).
#![allow(
    clippy::disallowed_macros,
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::wildcard_enum_match_arm,
    clippy::match_like_matches_macro,
    reason = "entry-point fixtures are ordinary Rust (programming-model.md, 10.2)"
)]
use super::revisions::{Harness, LIMITS, task};
use crate::{
    Active, Closing, Domain, Ending, Event, Key, Name, Phase, Refusal, Request, Stage, Status, Stored, Stub, Writer,
    WriterSlot,
};
use skein_lib::Token;

const DUE: u64 = 1;
const RUNNING: u64 = 2;
const STOPPING: u64 = 3;
const EFFECTS: u64 = 4;
const RELEASING: u64 = 5;
const UNNAMED: u64 = 99;
const ATTEMPT: u64 = 7;

fn cold(source: &Harness) -> Harness {
    let mut cold = Harness::new();
    cold.domain = Domain::new(&LIMITS, 91, source.domain.newest(), super::charters(), Box::new([0]));
    cold.stored.clear();
    for (key, hold) in &source.domain.hold_kinds {
        cold.send(Event::Kinds {
            connector: key.connector,
            kinds: Box::new([crate::Kind { connector: key.connector, kind: key.kind, hold: *hold }]),
        });
    }
    for row in source.stored.values().filter(|row| row.key().live()) {
        cold.send(Event::Restore { record: row.clone() });
    }
    cold
}

fn one_task() -> Harness {
    let mut source = Harness::new();
    source.make(vec![task(DUE, &[])]);
    source
}

#[test]
fn restored_escalation_keeps_its_original_stall_deadline() {
    let mut source = one_task();
    source.send(Event::Hold { task: DUE, why: crate::Hold::Stopped });
    source.route(DUE, crate::EscalationHolder::Person(9), 1);
    let before = source.domain.task(DUE).expect("held task").escalation.clone();
    let deadline = source.domain.escalation_alarms.next().expect("original stall timer");
    let mut restored = cold(&source);
    // One minute has passed since the stored decision; restart must not give
    // its holder a new hour to answer (LIMITS.escalation_stall).
    restored.env.now = skein_lib::Time::from_nanos(60_000_000_000);
    restored.env.wall = skein_lib::Wall::from_nanos(60_000_000_000);
    restored.send(Event::Check);
    let out = restored.send(Event::Restored);
    assert_eq!(restored.domain.escalation_alarms.next(), Some(deadline));
    assert_eq!(restored.domain.task(DUE).expect("restored task").escalation, before);
    assert!(!out.iter().any(|request| match request {
        Request::Save { .. } => true,
        _ => false,
    }));
}

fn refusal(out: &[Request], why: Refusal) {
    let [Request::RestoreRefused { problem }] = out else { panic!("one restore refusal: {out:?}") };
    assert_eq!(problem.why, why);
}

fn lifecycle(request: &Request) -> bool {
    match request {
        Request::Activate { .. }
        | Request::Adopt { .. }
        | Request::Stop { .. }
        | Request::Close { .. }
        | Request::Release { .. } => true,
        _ => false,
    }
}

fn refused_at_check(mut cold: Harness) {
    refusal(&cold.send(Event::Check), Refusal::Restore);
    assert!(!cold.domain.checked());
    assert!(cold.send(Event::Restored).is_empty(), "failed checks never replay");
}

#[test]
fn a_broken_task_link_refuses_at_check() {
    let mut source = one_task();
    let Some(Stored::Live(row)) = source.stored.get_mut(&Key::Live(DUE)) else { panic!("live fixture") };
    row.root = UNNAMED;
    refused_at_check(cold(&source));
}

#[test]
fn a_stub_named_by_no_live_task_refuses_at_check() {
    let mut cold = cold(&one_task());
    cold.send(Event::Restore {
        record: Stored::Stub(Stub { task: UNNAMED, phase: Status::Done, result: Token::new(UNNAMED) }),
    });
    refused_at_check(cold);
}

#[test]
fn a_writer_without_a_holder_refuses_at_check() {
    let mut cold = cold(&one_task());
    cold.send(Event::Restore {
        record: Stored::Writer(WriterSlot {
            number: 1,
            resource: Name { connector: 1, path: Box::new([b"writer".as_slice().into()]) },
            writer: Writer::Effect { entry: 1 },
            lost: false,
        }),
    });
    refused_at_check(cold);
}

#[test]
fn unrepresentable_eventual_funding_refuses_at_check() {
    let mut source = one_task();
    let Some(Stored::Live(row)) = source.stored.get_mut(&Key::Live(DUE)) else { panic!("live fixture") };
    row.numbers.spent = 1;
    let funder = row.funder;
    let Some(Stored::Ledger(row)) = source.stored.get_mut(&Key::Ledger(funder)) else { panic!("ledger fixture") };
    row.numbers.budget = u64::MAX;
    row.numbers.reserved = 0;
    row.numbers.spent_below = u64::MAX;
    refused_at_check(cold(&source));
}

#[test]
fn restored_before_check_refuses_and_never_replays() {
    let mut cold = cold(&one_task());
    refusal(&cold.send(Event::Restored), Refusal::NotReady);
    assert!(cold.send(Event::Check).is_empty());
    assert!(cold.send(Event::Restored).is_empty());
    assert!(!cold.domain.checked());
}

#[test]
fn restoring_another_row_after_check_refuses_the_start() {
    let source = one_task();
    let mut cold = cold(&source);
    cold.send(Event::Check);
    let out = cold.send(Event::Restore { record: source.stored[&Key::Live(DUE)].clone() });
    refusal(&out, Refusal::NotReady);
    assert!(cold.send(Event::Restored).is_empty());
}

fn replay_fixture() -> Harness {
    let mut source = Harness::new();
    source.make(vec![
        task(DUE, &[]),
        task(RUNNING, &[]),
        task(STOPPING, &[]),
        task(EFFECTS, &[]),
        task(RELEASING, &[]),
    ]);
    for (number, phase) in [
        (RUNNING, Phase::Active(Active::Running { attempt: ATTEMPT })),
        (
            STOPPING,
            Phase::Closing(Closing {
                stage: Stage::Run { attempt: ATTEMPT },
                ending: Ending::Failed { reason: Box::new([]) },
            }),
        ),
        (EFFECTS, Phase::Closing(Closing { stage: Stage::Effects, ending: Ending::Failed { reason: Box::new([]) } })),
        (
            RELEASING,
            Phase::Closing(Closing { stage: Stage::Releases, ending: Ending::Failed { reason: Box::new([]) } }),
        ),
    ] {
        let Some(Stored::Live(row)) = source.stored.get_mut(&Key::Live(number)) else { panic!("live fixture") };
        row.phase = phase;
        row.attempt = ATTEMPT;
        if number == RUNNING {
            row.run_since = Some(source.env.wall);
        }
    }
    cold(&source)
}

#[test]
fn checked_tasks_replay_each_lifecycle_request_exactly_once() {
    let mut cold = replay_fixture();
    assert!(cold.send(Event::Check).is_empty());
    assert!(cold.domain.checked());
    assert!(!cold.domain.ready());
    assert!(cold.send(Event::Check).is_empty(), "a duplicate check cannot replay work");
    let lifecycle: Vec<_> = cold
        .send(Event::Restored)
        .into_iter()
        .filter(lifecycle)
        .map(|request| match request {
            Request::Activate { context } => ("activate", context.task),
            Request::Adopt { task, attempt: ATTEMPT, kept: 0 } => ("adopt", task),
            Request::Stop { task, attempt: ATTEMPT } => ("stop", task),
            Request::Close { task, ending: Ending::Failed { .. } } => ("close", task),
            Request::Release { task, ending: Ending::Failed { .. } } => ("release", task),
            other => panic!("unexpected replay {other:?}"),
        })
        .collect();
    assert_eq!(
        lifecycle,
        [
            ("activate", DUE),
            ("adopt", RUNNING),
            ("adopt", STOPPING),
            ("stop", STOPPING),
            ("close", EFFECTS),
            ("release", RELEASING)
        ]
    );
    assert!(cold.domain.ready());
    assert!(cold.send(Event::Restored).is_empty(), "duplicate completion cannot replay work");
}

#[test]
fn a_circle_waiter_is_held_and_written_at_check() {
    use crate::{Hold, HoldKind, Holding, Kind, Party, Taken};
    const HOLDER: u64 = 1;
    const DELEGATE: u64 = 2;
    const WAITER: u64 = 3;
    let name = Name { connector: 1, path: Box::new([b"resource".as_slice().into()]) };
    let kinds = Box::new([Kind { connector: 1, kind: 1, hold: HoldKind::Exclusive { taken: Taken::Waits } }]);
    let mut source = Harness::new();
    source.send(Event::Kinds { connector: 1, kinds: kinds.clone() });
    let mut holder = task(HOLDER, &[]);
    holder.holdings = Box::new([Holding::Write { resource: name.clone(), kind: 1 }]);
    let mut waiter = task(WAITER, &[]);
    waiter.holdings = Box::new([Holding::Write { resource: name, kind: 1 }]);
    source.make(vec![holder, waiter]);
    source.send(Event::Make {
        reply_to: super::revisions::to(),
        creator: Party::Task(HOLDER),
        batch: Box::new([task(DELEGATE, &[])]),
    });
    let Some(Stored::Live(row)) = source.stored.get_mut(&Key::Live(DELEGATE)) else { panic!("delegate fixture") };
    row.dependencies = Box::new([WAITER]);
    row.waiting_on = Box::new([WAITER]);
    row.phase = Phase::Waiting;
    row.holds_taken = false;
    let mut cold = cold(&source);
    let out = cold.send(Event::Check);
    assert!(cold.domain.checked(), "{out:?}");
    assert!(
        out.iter().any(|request| match request {
            Request::Save { record: Stored::Live(row) } if row.number == WAITER => match row.phase {
                Phase::Held { why: Hold::Circle, .. } => true,
                _ => false,
            },
            _ => false,
        }),
        "the repaired hold is durable before switching: {out:?}"
    );
    assert!(!out.iter().any(lifecycle), "no replay at Check");
}
