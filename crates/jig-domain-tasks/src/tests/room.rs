//! Freeing counts wait; lifetime counts still refuse (domain/connectors.md, 6.2).
#![allow(
    clippy::disallowed_macros,
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::wildcard_enum_match_arm,
    clippy::match_like_matches_macro,
    reason = "ordinary test parent scripts and inspects bounded step outputs (programming-model.md, 10.2)"
)]
use super::revisions::{Harness, task, to};
use crate::{
    Active, Control, Event, Executor, LiveRoom, Party, Phase, ProcedureDecision, ProcedureEnd, Refusal, Request,
};
use alloc::boxed::Box;

fn world() -> Harness {
    let mut h = Harness::new();
    let mut first = task(1, &[]);
    first.executor = Executor::Procedure { connector: 2, code: 1 };
    let mut second = task(2, &[]);
    second.executor = first.executor;
    h.make(vec![first, second, task(3, &[]), task(4, &[])]);
    h
}

fn refuse(h: &mut Harness, owner: u64, expected: Refusal) {
    let mut child = task(10, &[]);
    child.funder = crate::Funder::Task(owner);
    child.numbers.budget = 1;
    child.authority.budget.spend = 1;
    let decision = ProcedureDecision { batch: Box::new([child]), ..ProcedureDecision::new(ProcedureEnd::Wait) };
    let out = h.send(Event::Procedure { reply_to: to(), task: owner, step: 1, read: None, decision });
    assert!(
        out.iter().any(|request| match request {
            Request::ProcedureRefused { task, problem, .. } => *task == owner && problem.why == expected,
            _ => false,
        }),
        "{out:?}"
    );
    assert!(h.domain.task(owner).unwrap().delegates.is_empty());
    assert_eq!(h.domain.task(owner).unwrap().numbers.reserved, 0);
}

fn end(h: &mut Harness, task: u64) -> Vec<Request> {
    h.send(Event::Control {
        reply_to: to(),
        by: Party::Person(9),
        task,
        control: Control::Cancel { reason: Box::new([]), grace: None, message: None },
    });
    h.send(Event::EffectsSettled { task });
    let out = h.send(Event::Settled { task });
    assert!(h.domain.task(task).is_none());
    out
}

#[test]
fn deployment_room_waits_and_each_ending_wakes_one_in_refusal_order() {
    let mut h = world();
    h.env.limits.tasks = 4;
    refuse(&mut h, 2, Refusal::Live);
    refuse(&mut h, 1, Refusal::Live);
    assert_eq!(h.domain.task(2).unwrap().room_wait.unwrap().room, LiveRoom::Deployment);
    assert_eq!(h.domain.task(1).unwrap().phase, Phase::Active(Active::Idle));
    assert!(h.domain.procedure_due(1).is_none());
    let out = end(&mut h, 3);
    let activated: Vec<_> = out
        .iter()
        .filter_map(|request| match request {
            Request::Activate { context } => Some(context.task),
            _ => None,
        })
        .collect();
    assert_eq!(activated, vec![2]);
    assert!(h.domain.task(2).unwrap().room_wait.is_none());
    assert!(h.domain.procedure_due(2).is_some());
    assert!(h.domain.procedure_due(1).is_none());
    let out = end(&mut h, 4);
    assert!(out.iter().any(|request| match request {
        Request::Activate { context } => context.task == 1,
        _ => false,
    }));
    assert!(h.domain.procedure_due(1).is_some());
}

#[test]
fn project_room_ignores_other_projects_and_rebuilds_fifo_after_restart() {
    let mut h = world();
    h.env.limits.project_tasks = 4;
    h.send(Event::OpenPeriod { reply_to: to(), project: 2, period: 0, budget: 1000 });
    let mut unrelated = task(5, &[]);
    unrelated.project = 2;
    unrelated.funder = crate::Funder::Period { project: 2, period: 0 };
    h.make(vec![unrelated]);
    refuse(&mut h, 2, Refusal::Project);
    refuse(&mut h, 1, Refusal::Project);
    assert_eq!(h.domain.task(2).unwrap().room_wait.unwrap().room, LiveRoom::Project(1));
    h.restart();
    assert!(h.domain.procedure_due(1).is_none());
    assert!(h.domain.procedure_due(2).is_none());
    let out = end(&mut h, 5);
    assert!(!out.iter().any(|request| match request {
        Request::Activate { context } => [1, 2].contains(&context.task),
        _ => false,
    }));
    end(&mut h, 3);
    assert!(h.domain.procedure_due(2).is_some());
    assert!(h.domain.procedure_due(1).is_none());
    end(&mut h, 4);
    assert!(h.domain.procedure_due(1).is_some());
}

#[test]
fn the_tree_lifetime_allotment_and_spend_still_refuse_without_a_room_wait() {
    let mut h = world();
    h.env.limits.tree_tasks = 1;
    refuse(&mut h, 1, Refusal::Tree);
    assert!(h.domain.procedure_due(1).is_some());
    assert!(h.domain.task(1).unwrap().room_wait.is_none());
    h.env.limits.tree_tasks = 16;
    crate::domain::task_mut(&mut h.domain, 1).unwrap().record.numbers.spent = 100;
    refuse(&mut h, 1, Refusal::Funding);
    assert!(h.domain.task(1).unwrap().room_wait.is_none());
    assert!(h.domain.procedure_due(1).is_some());
}

#[test]
fn a_woken_step_rechecks_current_room_and_rejoins_after_older_waiters() {
    let mut h = world();
    h.env.limits.tasks = 4;
    refuse(&mut h, 2, Refusal::Live);
    refuse(&mut h, 1, Refusal::Live);
    end(&mut h, 3);
    h.make(vec![task(5, &[])]);
    refuse(&mut h, 2, Refusal::Live);
    assert!(h.domain.task(2).unwrap().room_wait.unwrap().order > h.domain.task(1).unwrap().room_wait.unwrap().order);
    end(&mut h, 4);
    assert!(h.domain.procedure_due(1).is_some());
    assert!(h.domain.procedure_due(2).is_none());
    end(&mut h, 5);
    assert!(h.domain.procedure_due(2).is_some());
    let mut child = task(10, &[]);
    child.funder = crate::Funder::Task(2);
    child.numbers.budget = 1;
    child.authority.budget.spend = 1;
    let out = h.send(Event::Procedure {
        reply_to: to(),
        task: 2,
        step: 1,
        read: None,
        decision: ProcedureDecision { batch: Box::new([child]), ..ProcedureDecision::new(ProcedureEnd::Wait) },
    });
    assert!(
        out.iter().any(|request| match request {
            Request::Made { .. } => true,
            _ => false,
        }),
        "{out:?}"
    );
    assert!(h.domain.task(2).unwrap().room_wait.is_none());
    assert_eq!(h.domain.task(2).unwrap().delegates.as_ref(), &[10]);
}

#[test]
fn deployment_and_project_waits_share_the_one_place_freed() {
    let mut h = world();
    h.env.limits.tasks = 4;
    refuse(&mut h, 2, Refusal::Live);
    h.env.limits.tasks = 5;
    h.env.limits.project_tasks = 4;
    refuse(&mut h, 1, Refusal::Project);
    end(&mut h, 3);
    assert!(h.domain.procedure_due(2).is_some());
    assert!(h.domain.procedure_due(1).is_none());
    end(&mut h, 4);
    assert!(h.domain.procedure_due(1).is_some());
}
