use jig_domain_tasks::{
    self as tasks, Active, CapacityCause, Class, Control, End, Event, Hold, Key, Mode, Owner, Party, Phase, PlacedHold,
    Proposal, ProposalAction, ProposalHolder, ProposalState, Refusal, Scope, Stored,
};
use jig_tasks_world::{LIMITS, Reply, World, task};

fn stop(w: &mut World, task: u64, by: u64, hold: u64) -> Reply {
    let reply_to = w.to();
    let call = reply_to.into_token().raw();
    let reply_to = skein_lib::ReplyTo::new(skein_lib::Token::new(call));
    w.send(Event::Place {
        reply_to,
        hold: PlacedHold {
            number: hold,
            project: w.record(task).project,
            owner: Owner::Party { person: by },
            mode: Mode::Stop,
            scope: Scope::Subtree { task },
            reason: Box::new([]),
            placed_at: w.env.wall.as_nanos() / 1_000_000,
            revision: 1,
            exempt: Box::new([]),
            lifted: false,
        },
    });
    w.replies[&call].clone()
}

fn release_from(w: &mut World, by: Party, task: u64) -> Reply {
    let reply_to = w.to();
    let call = reply_to.into_token().raw();
    let reply_to = skein_lib::ReplyTo::new(skein_lib::Token::new(call));
    w.send(Event::Control { reply_to, by, task, control: Control::Release { again: None, message: None } });
    w.replies[&call].clone()
}

fn lift(w: &mut World, hold: u64, revision: u64, with_tasks: bool) -> Reply {
    let reply_to = w.to();
    let call = reply_to.into_token().raw();
    let reply_to = skein_lib::ReplyTo::new(skein_lib::Token::new(call));
    w.send(Event::Lift { reply_to, by: Party::Person(1), hold, revision, with_tasks });
    w.replies[&call].clone()
}

fn goal_tree(w: &mut World) {
    let mut goal = task(1, &[]);
    goal.tracked = Some(1);
    assert_eq!(w.make(Party::Person(1), vec![goal]), Reply::Made(vec![1]));
    w.claim(1, 1);
    assert_eq!(w.make(Party::Task(1), vec![task(2, &[]), task(3, &[])]), Reply::Made(vec![2, 3]));
    w.claim(2, 2);
    assert_eq!(w.make(Party::Task(2), vec![task(4, &[])]), Reply::Made(vec![4]));
    w.claim(4, 4);
}

fn park(w: &mut World) {
    for number in [1, 2, 4] {
        w.terminal(number, End::Parked);
    }
}

fn placed(w: &World, task: u64, hold: u64) {
    assert_eq!(w.record(task).placed, Some(hold));
    assert!(matches!(w.record(task).phase, Phase::Held { why: Hold::Placed { hold: number }, .. } if number == hold));
    assert!(matches!(w.record(task).escalation, tasks::Escalation::Unheld { .. }));
}

fn due(w: &World, task: u64) {
    assert_eq!(w.record(task).placed, None);
    assert_eq!(w.record(task).phase, Phase::Active(Active::Due));
}

#[test]
fn a_stop_holds_the_goal_and_every_delegate_and_cancels_their_runs() {
    let mut w = World::new(901, LIMITS);
    goal_tree(&mut w);
    assert_eq!(stop(&mut w, 1, 1, 100), Reply::Done);
    for number in 1..=4 {
        placed(&w, number, 100);
    }
    assert_eq!(w.stops, [(1, 1), (2, 2), (4, 4)].into_iter().collect());
}

#[test]
fn a_task_held_for_failures_keeps_its_escalation_under_a_stop() {
    let mut w = World::new(902, LIMITS);
    goal_tree(&mut w);
    w.send(Event::Hold { task: 3, why: Hold::Failures(Class::Run) });
    let escalation = w.record(3).escalation.clone();
    stop(&mut w, 1, 1, 100);
    assert_eq!(w.record(3).escalation, escalation);
    assert_eq!(w.record(3).placed, Some(100));
    assert!(matches!(w.record(3).phase, Phase::Held { why: Hold::Failures(Class::Run), .. }));
    let revision = w.record(3).revisions.hold;
    let reply_to = w.to();
    w.send(Event::DecideEscalation {
        reply_to,
        task: 3,
        revision,
        by: 1,
        entry: None,
        decision: tasks::EscalationDecision::Release,
    });
    placed(&w, 3, 100);
    assert_eq!(w.record(3).revisions.hold, revision + 1);
}

#[test]
fn a_task_above_a_partys_stop_is_refused_naming_the_hold() {
    let mut w = World::new(903, LIMITS);
    w.make(Party::Person(1), vec![task(9, &[])]);
    w.make(Party::Task(9), vec![task(1, &[])]);
    stop(&mut w, 1, 1, 100);
    let before = w.record(1).clone();
    assert!(
        matches!(release_from(&mut w, Party::Task(9), 1), Reply::Refused(problem) if problem.why == Refusal::HoldOwner { hold: 100 })
    );
    assert_eq!(w.record(1), &before);
}

#[test]
fn releasing_the_stopped_root_lifts_the_hold_whole() {
    let mut w = World::new(904, LIMITS);
    goal_tree(&mut w);
    stop(&mut w, 1, 1, 100);
    park(&mut w);
    assert_eq!(release_from(&mut w, Party::Person(1), 1), Reply::Done);
    for number in 1..=4 {
        due(&w, number);
    }
    assert!(!w.records.contains_key(&Key::PlacedHold(100)));
}

#[test]
fn releasing_a_delegate_exempts_it_and_its_siblings_stay_held() {
    let mut w = World::new(905, LIMITS);
    goal_tree(&mut w);
    stop(&mut w, 1, 1, 100);
    park(&mut w);
    assert_eq!(release_from(&mut w, Party::Person(1), 2), Reply::Done);
    due(&w, 2);
    for number in [1, 3, 4] {
        placed(&w, number, 100);
    }
    assert_eq!(w.placed_hold(100).expect("placed test setup").exempt.as_ref(), &[2]);
    w.claim(2, 5);
    assert!(
        matches!(w.make(Party::Task(2), vec![task(5, &[])]), Reply::Refused(problem) if problem.why == Refusal::Placed { hold: 100 })
    );
}

#[test]
fn a_lift_without_tasks_keeps_the_record_until_the_last_release() {
    let mut w = World::new(906, LIMITS);
    goal_tree(&mut w);
    stop(&mut w, 1, 1, 100);
    park(&mut w);
    let reply_to = w.to();
    w.send(Event::Propose {
        reply_to,
        proposal: Proposal {
            number: 101,
            proposer: 2,
            project: 1,
            action: ProposalAction::ReleasePlaced { task: 4, hold: 100, revision: 1 },
            reason: Box::new([]),
            as_holder: false,
            state: ProposalState::Pending { holder: ProposalHolder::Person(1), since: w.env.wall },
        },
    });
    assert!(w.record(2).proposal.is_some());
    assert_eq!(lift(&mut w, 100, 1, false), Reply::Lifted { released: 0 });
    assert!(w.placed_hold(100).expect("placed test setup").lifted);
    assert_eq!(w.placed_hold(100).expect("placed test setup").revision, 2);
    assert!(w.record(2).proposal.is_none());
    for number in [4, 3, 2] {
        assert_eq!(release_from(&mut w, Party::Person(1), number), Reply::Done);
        assert!(w.placed_hold(100).is_some());
    }
    assert_eq!(release_from(&mut w, Party::Person(1), 1), Reply::Done);
    assert!(w.placed_hold(100).is_none());
    assert!(!w.records.contains_key(&Key::PlacedHold(100)));
}

#[test]
fn a_lift_with_tasks_releases_those_nothing_else_holds() {
    let mut w = World::new(907, LIMITS);
    goal_tree(&mut w);
    w.send(Event::Hold { task: 3, why: Hold::Failures(Class::Run) });
    let escalation = w.record(3).escalation.clone();
    stop(&mut w, 1, 1, 100);
    park(&mut w);
    assert_eq!(lift(&mut w, 100, 1, true), Reply::Lifted { released: 3 });
    for number in [1, 2, 4] {
        due(&w, number);
    }
    assert_eq!(w.record(3).placed, None);
    assert!(matches!(w.record(3).phase, Phase::Held { why: Hold::Failures(Class::Run), .. }));
    assert_eq!(w.record(3).escalation, escalation);
    assert!(w.placed_hold(100).is_none());
}

#[test]
fn a_stop_inside_a_stop_keeps_each_task_with_the_first() {
    let mut w = World::new(908, LIMITS);
    goal_tree(&mut w);
    stop(&mut w, 2, 1, 100);
    stop(&mut w, 1, 1, 101);
    for number in [2, 4] {
        placed(&w, number, 100);
    }
    for number in [1, 3] {
        placed(&w, number, 101);
    }
    park(&mut w);
    assert_eq!(release_from(&mut w, Party::Person(1), 4), Reply::Done);
    placed(&w, 4, 101);
}

#[test]
fn a_lift_naming_an_old_revision_is_refused_as_moved() {
    let mut w = World::new(909, LIMITS);
    goal_tree(&mut w);
    stop(&mut w, 1, 1, 100);
    assert_eq!(lift(&mut w, 100, 1, false), Reply::Lifted { released: 0 });
    assert!(
        matches!(lift(&mut w, 100, 1, false), Reply::Refused(problem) if problem.why == Refusal::HoldRevision { revision: 2 })
    );
}

#[test]
fn a_run_finishing_its_turn_under_a_stop_delegates_nothing() {
    let mut w = World::new(910, LIMITS);
    goal_tree(&mut w);
    stop(&mut w, 1, 1, 100);
    assert!(
        matches!(w.make(Party::Task(1), vec![task(5, &[])]), Reply::Refused(problem) if problem.why == Refusal::Placed { hold: 100 })
    );
    w.terminal(1, End::Parked);
    placed(&w, 1, 100);
    assert!(matches!(w.record(1).phase, Phase::Held { was: tasks::Was::Active(Active::Idle), .. }));
}

#[test]
fn a_subtree_hold_ends_with_its_root() {
    let mut w = World::new(911, LIMITS);
    goal_tree(&mut w);
    stop(&mut w, 1, 1, 100);
    let reply_to = w.to();
    w.send(Event::Control {
        reply_to,
        by: Party::Person(1),
        task: 1,
        control: Control::Cancel { reason: Box::new([]), grace: None, message: None },
    });
    w.complete_cancel();
    assert_eq!(w.live(), 0);
    assert!(!w.records.contains_key(&Key::PlacedHold(100)));
}

#[test]
fn placed_holds_survive_a_restart() {
    let mut w = World::new(912, LIMITS);
    goal_tree(&mut w);
    stop(&mut w, 1, 1, 100);
    park(&mut w);
    release_from(&mut w, Party::Person(1), 2);
    let revision = w.placed_hold(100).expect("placed test setup").revision;
    lift(&mut w, 100, revision, false);
    let hold = w.placed_hold(100).expect("placed test setup").clone();
    let rows = (1..=4).map(|number| w.record(number).clone()).collect::<Vec<_>>();
    w.restart();
    assert_eq!(w.placed_hold(100), Some(&hold));
    for row in rows {
        assert_eq!(w.record(row.number), &row);
    }
    assert_eq!(release_from(&mut w, Party::Person(1), 4), Reply::Done);
    due(&w, 4);
}

#[test]
fn placed_holds_are_bounded() {
    for (limits, cause) in [
        (tasks::Limits { placed_holds: 2, ..LIMITS }, CapacityCause::PlacedHolds { limit: 2 }),
        (tasks::Limits { project_placed_holds: 2, ..LIMITS }, CapacityCause::ProjectPlacedHolds { limit: 2 }),
        (tasks::Limits { party_placed_holds: 2, ..LIMITS }, CapacityCause::PartyPlacedHolds { limit: 2 }),
    ] {
        let mut w = World::new(913, limits);
        goal_tree(&mut w);
        assert_eq!(stop(&mut w, 1, 1, 100), Reply::Done);
        assert_eq!(stop(&mut w, 1, 1, 101), Reply::Done);
        assert!(
            matches!(stop(&mut w, 1, 1, 102), Reply::Refused(problem) if problem.capacity.expect("placed test setup").cause == cause)
        );
        assert!(!w.records.contains_key(&Key::PlacedHold(102)));
    }
    let mut w = World::new(914, tasks::Limits { exemptions: 1, ..LIMITS });
    goal_tree(&mut w);
    stop(&mut w, 1, 1, 100);
    park(&mut w);
    assert_eq!(release_from(&mut w, Party::Person(1), 2), Reply::Done);
    assert!(
        matches!(release_from(&mut w, Party::Person(1), 3), Reply::Refused(problem) if problem.why == Refusal::Busy)
    );
    assert_eq!(
        w.capacity_refusals().as_slice().last().expect("placed test setup").request.cause,
        CapacityCause::Exemptions { limit: 1 }
    );
    assert!(matches!(w.records[&Key::PlacedHold(100)], Stored::PlacedHold(_)));
    placed(&w, 3, 100);
}

#[test]
fn a_procedures_new_delegates_are_held_in_their_creation_commit() {
    let mut w = World::new(915, LIMITS);
    let mut procedure = task(1, &[]);
    procedure.executor = tasks::Executor::Procedure { connector: 1, code: 1 };
    w.make(Party::Person(1), vec![procedure]);
    stop(&mut w, 1, 1, 100);
    assert_eq!(w.make(Party::Task(1), vec![task(2, &[])]), Reply::Made(vec![2]));
    placed(&w, 2, 100);
    assert!(!w.activations.contains(&2));
    w.restart();
    placed(&w, 2, 100);
}

#[test]
fn releasing_a_stopped_root_keeps_a_delegates_failure_and_escalation() {
    let mut w = World::new(916, LIMITS);
    goal_tree(&mut w);
    w.send(Event::Hold { task: 3, why: Hold::Failures(Class::Run) });
    let escalation = w.record(3).escalation.clone();
    stop(&mut w, 1, 1, 100);
    park(&mut w);
    release_from(&mut w, Party::Person(1), 1);
    for task in [1, 2, 4] {
        due(&w, task);
    }
    assert_eq!(w.record(3).placed, None);
    assert_eq!(w.record(3).escalation, escalation);
    assert!(matches!(w.record(3).phase, Phase::Held { why: Hold::Failures(Class::Run), .. }));
    assert!(w.placed_hold(100).is_none());
}

#[test]
fn task_owned_holds_may_be_lifted_by_the_owner_its_ancestors_and_its_person() {
    let mut w = World::new(917, LIMITS);
    goal_tree(&mut w);
    let reply_to = w.to();
    w.send(Event::Place {
        reply_to,
        hold: PlacedHold {
            number: 100,
            project: 1,
            owner: Owner::Task { task: 2 },
            mode: Mode::Stop,
            scope: Scope::Subtree { task: 4 },
            reason: Box::new([]),
            placed_at: 0,
            revision: 1,
            exempt: Box::new([]),
            lifted: false,
        },
    });
    for by in [Party::Task(2), Party::Task(1), Party::Person(1)] {
        assert!(w.may_lift(100, by, false));
    }
    for by in [Party::Task(3), Party::Task(4), Party::Person(2)] {
        assert!(!w.may_lift(100, by, false));
    }
    assert!(w.may_lift(100, Party::Person(2), true));
    stop(&mut w, 1, 1, 101);
    assert!(!w.may_lift(101, Party::Task(1), true));
    assert!(!w.may_lift(101, Party::Person(2), false));
    assert!(w.may_lift(101, Party::Person(2), true));
    assert_eq!(release_from(&mut w, Party::Task(2), 4), Reply::Done);
    placed(&w, 4, 101);
}

#[test]
fn unoffered_and_malformed_placements_change_nothing() {
    let mut w = World::new(918, LIMITS);
    goal_tree(&mut w);
    let good = PlacedHold {
        number: 100,
        project: 1,
        owner: Owner::Party { person: 1 },
        mode: Mode::Stop,
        scope: Scope::Subtree { task: 1 },
        reason: Box::new([]),
        placed_at: 0,
        revision: 1,
        exempt: Box::new([]),
        lifted: false,
    };
    let variants = [
        PlacedHold { mode: Mode::Drain, ..good.clone() },
        PlacedHold { scope: Scope::Project, ..good.clone() },
        PlacedHold { revision: 2, ..good.clone() },
        PlacedHold { lifted: true, ..good.clone() },
        PlacedHold { exempt: Box::new([2]), ..good.clone() },
        PlacedHold { project: 2, ..good.clone() },
        PlacedHold { scope: Scope::Subtree { task: 99 }, ..good.clone() },
        PlacedHold { reason: vec![1; LIMITS.message_bytes as usize + 1].into_boxed_slice(), ..good },
    ];
    for hold in variants {
        let before = w.records.clone();
        let reply_to = w.to();
        let call = reply_to.into_token();
        w.send(Event::Place { reply_to: skein_lib::ReplyTo::new(call), hold });
        assert!(matches!(w.replies[&call.raw()], Reply::Refused(_)));
        assert_eq!(w.records, before);
    }
    let before = w.record(1).clone();
    w.send(Event::Hold { task: 1, why: Hold::Placed { hold: 100 } });
    assert_eq!(w.record(1), &before);
}

#[test]
fn a_compound_stop_and_root_release_see_each_others_subtree_changes() {
    let mut w = World::new(919, LIMITS);
    goal_tree(&mut w);
    let mut operations = skein_lib::List::with_capacity(3);
    operations.push(tasks::Operation::Stop { task: 1, hold: 100 }).expect("placed test setup");
    operations.push(tasks::Operation::Release { task: 1, again: None, message: None }).expect("placed test setup");
    operations.push(tasks::Operation::Stop { task: 2, hold: 101 }).expect("placed test setup");
    let reply_to = w.to();
    let token = reply_to.into_token();
    w.send(Event::Compound {
        reply_to: skein_lib::ReplyTo::new(token),
        by: Party::Person(1),
        project: 1,
        operations,
        period: 0,
        pool_budget: 10_000,
        period_budget: 100_000,
    });
    assert_eq!(w.replies[&token.raw()], Reply::Done);
    assert!(w.placed_hold(100).is_none());
    for task in [2, 4] {
        placed(&w, task, 101);
    }
    assert_eq!(w.record(1).placed, None);
    assert_eq!(w.record(3).placed, None);
}

fn check_rows(rows: Vec<Stored>, env: &skein_lib::Env<tasks::Limits>) -> Vec<tasks::Request> {
    let mut domain = jig_tasks_world::domain(&env.limits, 920);
    let mut out = skein_lib::Queue::with_capacity(tasks::max_out(&env.limits));
    for record in rows {
        tasks::step(&mut domain, env, Event::Restore { record }, &mut out);
    }
    tasks::step(&mut domain, env, Event::Check, &mut out);
    let mut replies = Vec::new();
    while let Some(reply) = out.pop() {
        replies.push(reply);
    }
    replies
}

#[test]
fn restart_refuses_missing_holds_missing_scope_roots_and_unheld_tasks_in_scope() {
    let mut w = World::new(920, LIMITS);
    goal_tree(&mut w);
    stop(&mut w, 1, 1, 100);
    park(&mut w);
    for corruption in 0..3 {
        let mut rows = w.records.values().filter(|row| row.key().live()).cloned().collect::<Vec<_>>();
        match corruption {
            0 => rows.retain(|row| row.key() != Key::PlacedHold(100)),
            1 => {
                for row in &mut rows {
                    if let Stored::PlacedHold(hold) = row {
                        hold.scope = Scope::Subtree { task: 99 };
                    }
                }
            }
            2 => {
                for row in &mut rows {
                    if let Stored::Live(task) = row
                        && task.number == 4
                    {
                        task.placed = None;
                        task.phase = Phase::Active(Active::Due);
                    }
                }
            }
            _ => unreachable!(),
        }
        assert!(check_rows(rows, &w.env).iter().any(|reply| matches!(reply, tasks::Request::RestoreRefused { .. })));
    }
}

#[test]
fn restart_refuses_each_placed_hold_count_over_its_limit() {
    let mut w = World::new(921, tasks::Limits { exemptions: 2, ..LIMITS });
    goal_tree(&mut w);
    stop(&mut w, 1, 1, 100);
    stop(&mut w, 1, 1, 101);
    park(&mut w);
    release_from(&mut w, Party::Person(1), 2);
    release_from(&mut w, Party::Person(1), 3);
    let rows = w.records.values().filter(|row| row.key().live()).cloned().collect::<Vec<_>>();
    for limits in [
        tasks::Limits { placed_holds: 1, ..w.env.limits },
        tasks::Limits { project_placed_holds: 1, ..w.env.limits },
        tasks::Limits { party_placed_holds: 1, ..w.env.limits },
        tasks::Limits { exemptions: 1, ..w.env.limits },
    ] {
        let env = skein_lib::Env { now: w.env.now, wall: w.env.wall, limits };
        assert!(
            check_rows(rows.clone(), &env).iter().any(|reply| matches!(reply, tasks::Request::RestoreRefused { .. }))
        );
    }
}

#[test]
fn inheriting_a_procedures_resource_under_a_stop_starts_no_activation() {
    let mut w = World::new(922, LIMITS);
    w.configure_holds(
        1,
        vec![tasks::Kind { connector: 1, kind: 1, hold: tasks::HoldKind::Exclusive { taken: tasks::Taken::Waits } }],
    );
    let resource =
        tasks::Holding::Write { resource: tasks::Name { connector: 1, path: Box::new([Box::new([1])]) }, kind: 1 };
    let mut procedure = task(1, &[]);
    procedure.executor = tasks::Executor::Procedure { connector: 1, code: 1 };
    procedure.holdings = Box::new([resource.clone()]);
    assert_eq!(w.make(Party::Person(1), vec![procedure]), Reply::Made(vec![1]));
    stop(&mut w, 1, 1, 100);
    let mut child = task(2, &[]);
    child.holdings = Box::new([resource]);
    assert_eq!(w.make(Party::Task(1), vec![child]), Reply::Made(vec![2]));
    assert!(w.record(2).holds_taken);
    placed(&w, 2, 100);
    assert!(!w.activations.contains(&2));
}
