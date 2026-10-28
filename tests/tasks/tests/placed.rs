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
fn accepting_one_release_keeps_a_siblings_proposal_pending() {
    let mut w = World::new(924, LIMITS);
    goal_tree(&mut w);
    stop(&mut w, 1, 1, 100);
    park(&mut w);
    for proposer in [2, 3] {
        let reply_to = w.to();
        w.send(Event::Propose {
            reply_to,
            proposal: Proposal {
                number: 100 + proposer,
                proposer,
                project: 1,
                action: ProposalAction::ReleasePlaced { task: proposer, hold: 100, revision: 1 },
                reason: Box::new([]),
                as_holder: false,
                state: ProposalState::Pending { holder: ProposalHolder::Person(1), since: w.env.wall },
            },
        });
    }
    let sibling = w.record(3).proposal.clone().expect("sibling proposal pending");
    let sibling_revision = w.record(3).revision;
    let reply_to = w.to();
    w.send(Event::DecideProposal {
        reply_to,
        proposer: 2,
        proposal: 102,
        message: Some(104),
        by: Party::Person(1),
        decision: tasks::ProposalDecision::Accept,
    });
    due(&w, 2);
    let hold = w.placed_hold(100).expect("standing hold");
    assert_eq!(hold.exempt.as_ref(), &[2]);
    assert_eq!(hold.revision, 1);
    assert!(!hold.lifted);
    assert!(w.record(2).proposal.is_none());
    placed(&w, 3, 100);
    assert_eq!(w.record(3).proposal.as_deref(), Some(sibling.as_ref()));
    assert_eq!(w.record(3).revision, sibling_revision);
    w.restart();
    assert_eq!(w.record(3).proposal.as_deref(), Some(sibling.as_ref()));
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
        PlacedHold { scope: Scope::Project, owner: Owner::Task { task: 1 }, ..good.clone() },
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
fn a_compound_cannot_stop_and_release_the_same_subtree_in_one_decision() {
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
    assert!(
        matches!(&w.replies[&token.raw()], Reply::Refused(problem) if problem.why == Refusal::Placed { hold: 100 })
    );
    assert!(w.placed_hold(100).is_none());
    for task in 1..=4 {
        assert_eq!(w.record(task).placed, None);
    }
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

#[test]
fn accepting_a_release_proposal_of_the_root_lifts_the_hold_and_records_it_accepted() {
    let mut w = World::new(925, LIMITS);
    goal_tree(&mut w);
    stop(&mut w, 1, 1, 100);
    park(&mut w);
    for (proposer, target) in [(2, 1), (3, 4)] {
        let reply_to = w.to();
        w.send(Event::Propose {
            reply_to,
            proposal: Proposal {
                number: 100 + proposer,
                proposer,
                project: 1,
                action: ProposalAction::ReleasePlaced { task: target, hold: 100, revision: 1 },
                reason: Box::new([]),
                as_holder: false,
                state: ProposalState::Pending { holder: ProposalHolder::Person(1), since: w.env.wall },
            },
        });
    }
    let reply_to = w.to();
    w.send(Event::DecideProposal {
        reply_to,
        proposer: 2,
        proposal: 102,
        message: Some(104),
        by: Party::Person(1),
        decision: tasks::ProposalDecision::Accept,
    });
    for number in 1..=4 {
        due(&w, number);
    }
    assert!(!w.records.contains_key(&Key::PlacedHold(100)));
    for (proposer, state) in [(2, ProposalState::Accepted { by: Party::Person(1) }), (3, ProposalState::Withdrawn)] {
        assert!(w.record(proposer).proposal.is_none());
        assert!(w.records.values().any(|row| matches!(row, Stored::History(history)
            if history.proposal.as_ref().is_some_and(|p| p.number == 100 + proposer && p.state == state))));
    }
}

// Project stops: the person's conversation stays open; its delegates do not.
fn project_stop(w: &mut World, project: u32, by: u64, hold: u64) -> Reply {
    let reply_to = w.to();
    let call = reply_to.into_token();
    w.send(Event::Place {
        reply_to: skein_lib::ReplyTo::new(call),
        hold: PlacedHold {
            number: hold,
            project,
            owner: Owner::Party { person: by },
            mode: Mode::Stop,
            scope: Scope::Project,
            reason: Box::new([]),
            placed_at: w.env.wall.as_nanos() / 1_000_000,
            revision: 1,
            exempt: Box::new([]),
            lifted: false,
        },
    });
    w.replies[&call.raw()].clone()
}

fn persons_chat(w: &mut World, number: u64) {
    let mut chat = task(number, &[]);
    chat.flags = tasks::Flags { conversation: true, ended_by: tasks::EndedBy::Requester, ..tasks::Flags::PLAIN };
    assert_eq!(w.make(Party::Person(1), vec![chat]), Reply::Made(vec![number]));
}

#[test]
fn a_project_stop_holds_every_task_but_the_persons_chat_and_cancels_their_runs() {
    // Goal 1 -> [2 -> 4, 3]; direct chat 9 -> conversation 10.
    let mut w = World::new(930, LIMITS);
    goal_tree(&mut w);
    w.claim(3, 3);
    persons_chat(&mut w, 9);
    w.claim(9, 9);
    let mut delegate = task(10, &[]);
    delegate.flags.conversation = true;
    assert_eq!(w.make(Party::Task(9), vec![delegate]), Reply::Made(vec![10]));
    w.claim(10, 10);
    assert_eq!(project_stop(&mut w, 1, 1, 100), Reply::Done);
    for number in [1, 2, 3, 4, 10] {
        placed(&w, number, 100);
    }
    assert_eq!(w.stops, [(1, 1), (2, 2), (3, 3), (4, 4), (10, 10)].into_iter().collect());
    assert_eq!(w.record(9).placed, None);
    assert_eq!(w.record(9).phase, Phase::Active(Active::Running { attempt: 9 }));
}

#[test]
fn a_task_made_by_the_person_under_a_project_stop_starts_held() {
    // Empty project stopped first; then person goal 1, chat 9 and deployment task 2.
    let mut w = World::new(931, LIMITS);
    assert_eq!(project_stop(&mut w, 1, 1, 100), Reply::Done);
    assert_eq!(w.make(Party::Person(1), vec![task(1, &[])]), Reply::Made(vec![1]));
    placed(&w, 1, 100);
    assert!(!w.activations.contains(&1));
    persons_chat(&mut w, 9);
    due(&w, 9);
    assert_eq!(w.make(Party::Deployment { project: 1 }, vec![task(2, &[])]), Reply::Made(vec![2]));
    placed(&w, 2, 100);
    assert!(!w.activations.contains(&2));
}

#[test]
fn the_persons_chat_delegates_nothing_under_a_project_stop() {
    // Direct chat 9; its proposed delegate 10 is refused by the lowest standing hold.
    let mut w = World::new(932, LIMITS);
    persons_chat(&mut w, 9);
    w.claim(9, 9);
    project_stop(&mut w, 1, 1, 101);
    project_stop(&mut w, 1, 1, 100);
    let before = w.records.clone();
    assert!(matches!(w.make(Party::Task(9), vec![task(10, &[])]),
        Reply::Refused(problem) if problem.why == Refusal::Placed { hold: 100 }));
    assert_eq!(w.records, before);
    assert_eq!(w.record(9).placed, None);
}

#[test]
fn a_procedures_batch_under_a_project_stop_is_made_held() {
    // Procedure 1 -> new agent delegates [2, 3].
    let mut w = World::new(933, LIMITS);
    let mut procedure = task(1, &[]);
    procedure.executor = tasks::Executor::Procedure { connector: 1, code: 1 };
    assert_eq!(w.make(Party::Person(1), vec![procedure]), Reply::Made(vec![1]));
    project_stop(&mut w, 1, 1, 100);
    assert_eq!(w.make(Party::Task(1), vec![task(2, &[]), task(3, &[])]), Reply::Made(vec![2, 3]));
    for number in [2, 3] {
        placed(&w, number, 100);
        assert!(!w.activations.contains(&number));
    }
}

#[test]
fn one_task_released_under_a_project_stop_runs_and_ends_while_it_stands() {
    // Goal 1 -> [2 -> 4, 3]; release the leaf goal 3 alone.
    let mut w = World::new(934, LIMITS);
    goal_tree(&mut w);
    project_stop(&mut w, 1, 1, 100);
    park(&mut w);
    let revision = w.placed_hold(100).expect("standing project hold").revision;
    assert_eq!(release_from(&mut w, Party::Person(1), 3), Reply::Done);
    due(&w, 3);
    assert_eq!(w.placed_hold(100).expect("standing project hold").exempt.as_ref(), &[3]);
    w.claim(3, 5);
    assert!(matches!(w.make(Party::Task(3), vec![task(5, &[])]),
        Reply::Refused(problem) if problem.why == Refusal::Placed { hold: 100 }));
    w.finish(3);
    w.settle(3);
    assert!(matches!(w.results.get(&3), Some(tasks::Ending::Done(_))));
    let hold = w.placed_hold(100).expect("project hold outlives task");
    assert!(!hold.lifted);
    assert_eq!(hold.revision, revision);
    for number in [1, 2, 4] {
        placed(&w, number, 100);
    }
}

#[test]
fn lifting_a_project_stop_with_its_tasks_releases_them() {
    // Goal 1 -> [2 -> 4, 3]; lift them all, then make goal 5.
    let mut w = World::new(935, LIMITS);
    goal_tree(&mut w);
    project_stop(&mut w, 1, 1, 100);
    park(&mut w);
    assert_eq!(lift(&mut w, 100, 1, true), Reply::Lifted { released: 4 });
    for number in 1..=4 {
        due(&w, number);
    }
    assert!(w.placed_hold(100).is_none());
    assert!(!w.records.contains_key(&Key::PlacedHold(100)));
    assert_eq!(w.make(Party::Person(1), vec![task(5, &[])]), Reply::Made(vec![5]));
    due(&w, 5);
}

#[test]
fn a_project_stop_covers_only_its_project() {
    // Project 1: goal 1 -> [2 -> 4, 3]; project 2: running 9 -> new 10.
    let mut w = World::new(936, LIMITS);
    goal_tree(&mut w);
    w.open_project_period(2, 0, 100_000);
    let mut other = task(9, &[]);
    other.project = 2;
    other.funder = tasks::Funder::Period { project: 2, period: 0 };
    assert_eq!(w.make(Party::Person(1), vec![other]), Reply::Made(vec![9]));
    w.claim(9, 9);
    project_stop(&mut w, 1, 1, 100);
    for number in 1..=4 {
        placed(&w, number, 100);
    }
    assert_eq!(w.record(9).placed, None);
    assert_eq!(w.record(9).phase, Phase::Active(Active::Running { attempt: 9 }));
    assert!(!w.stops.contains(&(9, 9)));
    let mut child = task(10, &[]);
    child.project = 2;
    child.funder = tasks::Funder::Period { project: 2, period: 0 };
    assert_eq!(w.make(Party::Task(9), vec![child]), Reply::Made(vec![10]));
    due(&w, 10);
}

#[test]
fn a_subtree_stop_inside_a_project_stop_hands_its_tasks_on_when_lifted() {
    // Goal 1 -> [2 -> 4, 3]; stop subtree 2 first, then its project.
    let mut w = World::new(937, LIMITS);
    goal_tree(&mut w);
    stop(&mut w, 2, 1, 100);
    project_stop(&mut w, 1, 1, 101);
    for number in [2, 4] {
        placed(&w, number, 100);
    }
    for number in [1, 3] {
        placed(&w, number, 101);
    }
    park(&mut w);
    assert_eq!(release_from(&mut w, Party::Person(1), 2), Reply::Done);
    assert!(w.placed_hold(100).is_none());
    for number in 1..=4 {
        placed(&w, number, 101);
    }
}

#[test]
fn a_task_cannot_place_a_project_hold() {
    // Goal 1 -> [2 -> 4, 3]; malformed project placements leave it alone.
    let mut w = World::new(938, LIMITS);
    goal_tree(&mut w);
    for (owner, mode, project) in
        [(Owner::Task { task: 1 }, Mode::Stop, 1), (Owner::Party { person: 1 }, Mode::Stop, 0)]
    {
        let before = w.records.clone();
        let reply_to = w.to();
        let call = reply_to.into_token();
        w.send(Event::Place {
            reply_to: skein_lib::ReplyTo::new(call),
            hold: PlacedHold {
                number: 100,
                project,
                owner,
                mode,
                scope: Scope::Project,
                reason: Box::new([]),
                placed_at: 0,
                revision: 1,
                exempt: Box::new([]),
                lifted: false,
            },
        });
        assert!(matches!(&w.replies[&call.raw()], Reply::Refused(problem) if problem.why == Refusal::AuthorityShape));
        assert_eq!(w.records, before);
    }
}

#[test]
fn a_project_stop_survives_a_restart() {
    // Goal 1 -> [2 -> 4, 3], exempt 2, direct chat 9; new goal 5 after restart.
    let mut w = World::new(939, LIMITS);
    goal_tree(&mut w);
    persons_chat(&mut w, 9);
    project_stop(&mut w, 1, 1, 100);
    park(&mut w);
    assert_eq!(release_from(&mut w, Party::Person(1), 2), Reply::Done);
    let hold = w.placed_hold(100).expect("standing hold").clone();
    let rows = [1, 2, 3, 4, 9].map(|number| w.record(number).clone());
    w.restart();
    assert_eq!(w.placed_hold(100), Some(&hold));
    for row in rows {
        assert_eq!(w.record(row.number), &row);
    }
    due(&w, 2);
    due(&w, 9);
    for number in [1, 3, 4] {
        placed(&w, number, 100);
    }
    assert_eq!(w.make(Party::Person(1), vec![task(5, &[])]), Reply::Made(vec![5]));
    placed(&w, 5, 100);
    assert!(!w.activations.contains(&5));
}

#[test]
fn restart_refuses_a_project_hold_with_an_unheld_task_in_scope_or_a_task_owner() {
    // Goal 1 -> [2 -> 4, 3]; corrupt task 4 or the project hold's owner.
    let mut w = World::new(940, LIMITS);
    goal_tree(&mut w);
    project_stop(&mut w, 1, 1, 100);
    park(&mut w);
    for task_owner in [false, true] {
        let mut rows = w.records.values().filter(|row| row.key().live()).cloned().collect::<Vec<_>>();
        for row in &mut rows {
            if let Stored::PlacedHold(hold) = row
                && task_owner
            {
                hold.owner = Owner::Task { task: 1 };
            }
            if let Stored::Live(task) = row
                && !task_owner
                && task.number == 4
            {
                task.placed = None;
                task.phase = Phase::Active(Active::Due);
            }
        }
        assert!(check_rows(rows, &w.env).iter().any(|reply| matches!(reply, tasks::Request::RestoreRefused { .. })));
    }
}

fn adopt(w: &mut World, task: u64, by: u64, compound: bool) -> Reply {
    let reply_to = w.to();
    let call = reply_to.into_token();
    let reply_to = skein_lib::ReplyTo::new(call);
    if compound {
        let mut operations = skein_lib::List::with_capacity(1);
        operations.push(tasks::Operation::Move { task, reason: Box::new([]) }).expect("one adoption");
        w.send(Event::Compound {
            reply_to,
            by: Party::Person(by),
            project: 1,
            operations,
            period: 0,
            pool_budget: 1000,
            period_budget: 10_000,
        });
    } else {
        w.send(Event::Move {
            reply_to,
            task,
            person: by,
            period: 0,
            pool_budget: 1000,
            period_budget: 10_000,
            reason: Box::new([]),
        });
    }
    w.replies[&call.raw()].clone()
}

#[test]
fn a_task_a_project_stop_holds_is_moved_by_no_one() {
    // Person 1's task 1 -> conversation 2; both are held by project stop 100.
    for compound in [false, true] {
        let mut w = World::new(941, LIMITS);
        assert_eq!(w.make(Party::Person(1), vec![task(1, &[])]), Reply::Made(vec![1]));
        let mut conversation = task(2, &[]);
        conversation.flags.conversation = true;
        assert_eq!(w.make(Party::Task(1), vec![conversation]), Reply::Made(vec![2]));
        assert_eq!(project_stop(&mut w, 1, 1, 100), Reply::Done);
        for number in [1, 2] {
            placed(&w, number, 100);
        }
        for by in [1, 9] {
            let before = w.records.clone();
            assert!(matches!(adopt(&mut w, 2, by, compound), Reply::Refused(problem)
                if problem.why == Refusal::Placed { hold: 100 }));
            assert_eq!(w.records, before);
        }
        assert_eq!(release_from(&mut w, Party::Person(1), 2), Reply::Done);
        due(&w, 2);
        assert_eq!(adopt(&mut w, 2, 9, compound), Reply::Done);
        assert_eq!(w.record(2).requester, Party::Person(9));
        due(&w, 2);
        placed(&w, 1, 100);
        assert_eq!(w.placed_hold(100).expect("standing project hold").exempt.as_ref(), &[2]);
        w.restart();
        assert_eq!(w.record(2).requester, Party::Person(9));
        due(&w, 2);
    }
}

#[test]
fn a_task_held_by_a_lifted_project_stop_can_be_moved() {
    // Person 1's task 1 stays held after lifting stop 100, then person 9 adopts it.
    let mut w = World::new(942, LIMITS);
    assert_eq!(w.make(Party::Person(1), vec![task(1, &[])]), Reply::Made(vec![1]));
    assert_eq!(project_stop(&mut w, 1, 1, 100), Reply::Done);
    assert_eq!(lift(&mut w, 100, 1, false), Reply::Lifted { released: 0 });
    assert_eq!(adopt(&mut w, 1, 9, false), Reply::Done);
    assert_eq!(w.record(1).requester, Party::Person(9));
    placed(&w, 1, 100);
    w.restart();
    placed(&w, 1, 100);
}

#[test]
fn a_moved_conversation_uses_its_projected_requester_when_released_from_another_hold() {
    // Task 1 -> conversation 2; subtree stop 100 holds 2 before project stop 101.
    let mut w = World::new(943, LIMITS);
    assert_eq!(w.make(Party::Person(1), vec![task(1, &[])]), Reply::Made(vec![1]));
    let mut conversation = task(2, &[]);
    conversation.flags.conversation = true;
    assert_eq!(w.make(Party::Task(1), vec![conversation]), Reply::Made(vec![2]));
    assert_eq!(stop(&mut w, 2, 1, 100), Reply::Done);
    assert_eq!(project_stop(&mut w, 1, 1, 101), Reply::Done);
    let mut operations = skein_lib::List::with_capacity(2);
    operations.push(tasks::Operation::Move { task: 2, reason: Box::new([]) }).expect("adoption");
    operations.push(tasks::Operation::Release { task: 2, again: None, message: None }).expect("release");
    let reply_to = w.to();
    let call = reply_to.into_token();
    w.send(Event::Compound {
        reply_to: skein_lib::ReplyTo::new(call),
        by: Party::Person(1),
        project: 1,
        operations,
        period: 0,
        pool_budget: 1000,
        period_budget: 10_000,
    });
    assert_eq!(w.replies[&call.raw()], Reply::Done);
    assert_eq!(w.record(2).requester, Party::Person(1));
    due(&w, 2);
    placed(&w, 1, 101);
    assert!(w.placed_hold(100).is_none());
    assert!(!w.placed_hold(101).expect("standing project stop").lifted);
    w.restart();
    due(&w, 2);
}

// Drains: live runs park, and procedures say when their existing work is idle.
fn drain(w: &mut World, scope: Scope, by: u64, hold: u64) -> Reply {
    let token = w.to().into_token();
    w.send(Event::Place {
        reply_to: skein_lib::ReplyTo::new(token),
        hold: PlacedHold {
            number: hold,
            project: 1,
            owner: Owner::Party { person: by },
            mode: Mode::Drain,
            scope,
            reason: b"finish existing work".as_slice().into(),
            placed_at: 0,
            revision: 1,
            exempt: Box::new([]),
            lifted: false,
        },
    });
    w.replies[&token.raw()].clone()
}

fn procedure(w: &mut World, number: u64) {
    let mut new = task(number, &[]);
    new.executor = tasks::Executor::Procedure { connector: 1, code: 1 };
    assert_eq!(w.make(Party::Person(1), vec![new]), Reply::Made(vec![number]));
}

fn procedure_step(w: &mut World, number: u64, decision: tasks::ProcedureDecision) -> Reply {
    let token = w.to().into_token();
    let read = w.contexts[&number].inbox.last().map(|word| word.number);
    w.send(Event::Procedure {
        reply_to: skein_lib::ReplyTo::new(token),
        task: number,
        step: w.record(number).attempt + 1,
        read,
        decision,
    });
    w.replies[&token.raw()].clone()
}

#[test]
fn a_drain_parks_each_live_run_and_holds_each_idle_task_at_once() {
    let mut w = World::new(950, LIMITS);
    goal_tree(&mut w);
    let before = [1, 2, 4].map(|task| w.record(task).phase.clone());
    assert_eq!(drain(&mut w, Scope::Subtree { task: 1 }, 1, 100), Reply::Done);
    assert_eq!(w.parks, [(1, 1), (2, 2), (4, 4)].into_iter().collect());
    assert!(w.stops.is_empty());
    for (number, phase) in [1, 2, 4].into_iter().zip(before) {
        assert_eq!(w.record(number).placed, Some(100));
        assert_eq!(w.record(number).phase, phase);
    }
    placed(&w, 3, 100);
}

#[test]
fn a_run_parked_by_a_drain_ends_due_with_no_try_spent_and_is_held() {
    let mut w = World::new(951, LIMITS);
    w.make(Party::Person(1), vec![task(1, &[])]);
    w.claim(1, 1);
    drain(&mut w, Scope::Subtree { task: 1 }, 1, 100);
    w.terminal(1, End::Parked);
    placed(&w, 1, 100);
    assert!(matches!(w.record(1).phase, Phase::Held { was: tasks::Was::Active(Active::Due), .. }));
    assert_eq!(w.record(1).tries, tasks::Tries::NONE);
    assert_eq!(w.record(1).refusals, 0);
    assert_eq!(lift(&mut w, 100, 1, true), Reply::Lifted { released: 1 });
    due(&w, 1);
}

#[test]
fn a_due_task_under_a_drain_gets_no_run() {
    let mut w = World::new(952, LIMITS);
    drain(&mut w, Scope::Project, 1, 100);
    assert_eq!(w.make(Party::Person(1), vec![task(1, &[])]), Reply::Made(vec![1]));
    placed(&w, 1, 100);
    assert!(!w.activations.contains(&1));
    let before = w.record(1).clone();
    let token = w.to().into_token();
    w.send(Event::Prepare { reply_to: skein_lib::ReplyTo::new(token), task: 1 });
    assert!(matches!(w.replies[&token.raw()], Reply::Refused(_)));
    assert_eq!(w.record(1), &before);
    // Even a new procedure is held, rather than asked to drain work it never began.
    procedure(&mut w, 2);
    placed(&w, 2, 100);
    assert!(!w.activations.contains(&2));
}

#[test]
fn a_run_finishing_its_turn_under_a_drain_delegates_nothing() {
    let mut w = World::new(953, LIMITS);
    goal_tree(&mut w);
    drain(&mut w, Scope::Subtree { task: 1 }, 1, 100);
    assert!(matches!(w.make(Party::Task(1), vec![task(5, &[])]),
        Reply::Refused(problem) if problem.why == Refusal::Placed { hold: 100 }));
    assert_eq!(w.record(1).phase, Phase::Active(Active::Running { attempt: 1 }));
}

#[test]
fn a_procedure_under_a_drain_steps_draining_until_its_connector_says_idle() {
    let mut w = World::new(954, LIMITS);
    procedure(&mut w, 1);
    drain(&mut w, Scope::Subtree { task: 1 }, 1, 100);
    // Its current step was offered before placement; the next one hears the drain.
    assert!(!w.contexts[&1].draining);
    assert_eq!(procedure_step(&mut w, 1, tasks::ProcedureDecision::new(tasks::ProcedureEnd::Wait)), Reply::Done);
    assert_eq!(w.record(1).phase, Phase::Active(Active::Idle));
    w.send(Event::WakeProcedure { task: 1 });
    assert!(w.contexts[&1].draining);
    let decision = tasks::ProcedureDecision {
        batch: Box::new([task(2, &[])]),
        ..tasks::ProcedureDecision::new(tasks::ProcedureEnd::Wait)
    };
    assert_eq!(procedure_step(&mut w, 1, decision), Reply::Made(vec![2]));
    placed(&w, 2, 100);
    assert!(!w.activations.contains(&2));
    w.send(Event::WakeProcedure { task: 1 });
    assert!(w.contexts[&1].draining);
    assert_eq!(procedure_step(&mut w, 1, tasks::ProcedureDecision::new(tasks::ProcedureEnd::Idle)), Reply::Done);
    placed(&w, 1, 100);
}

#[test]
fn an_idle_procedure_under_a_drain_is_stepped_to_say_whether_it_is_idle() {
    let mut w = World::new(955, LIMITS);
    procedure(&mut w, 1);
    procedure_step(&mut w, 1, tasks::ProcedureDecision::new(tasks::ProcedureEnd::Idle));
    assert_eq!(w.record(1).phase, Phase::Active(Active::Idle));
    let activations = w.activations(1);
    drain(&mut w, Scope::Subtree { task: 1 }, 1, 100);
    assert_eq!(w.activations(1), activations + 1);
    assert!(w.contexts[&1].draining);
    assert_eq!(w.record(1).phase, Phase::Active(Active::Due));
    procedure_step(&mut w, 1, tasks::ProcedureDecision::new(tasks::ProcedureEnd::Idle));
    placed(&w, 1, 100);
}

#[test]
fn a_person_task_under_a_drain_is_held_at_once() {
    let mut w = World::new(956, LIMITS);
    let mut new = task(1, &[]);
    new.executor = tasks::Executor::Person(tasks::PersonAddress::Person(1));
    w.make(Party::Person(1), vec![new]);
    drain(&mut w, Scope::Subtree { task: 1 }, 1, 100);
    placed(&w, 1, 100);
    assert!(w.parks.is_empty());
}

#[test]
fn a_closing_task_under_a_drain_finishes() {
    let mut w = World::new(957, LIMITS);
    w.make(Party::Person(1), vec![task(1, &[])]);
    w.claim(1, 1);
    w.finish(1);
    let before = w.record(1).phase.clone();
    assert!(matches!(before, Phase::Closing(_)));
    drain(&mut w, Scope::Subtree { task: 1 }, 1, 100);
    assert_eq!(w.record(1).phase, before);
    w.settle(1);
    assert!(matches!(w.results.get(&1), Some(tasks::Ending::Done(_))));
    assert!(w.placed_hold(100).is_none());
}

#[test]
fn a_project_drain_keeps_the_persons_chat_running() {
    let mut w = World::new(958, LIMITS);
    goal_tree(&mut w);
    persons_chat(&mut w, 9);
    w.claim(9, 9);
    drain(&mut w, Scope::Project, 1, 100);
    assert_eq!(w.record(9).placed, None);
    assert_eq!(w.record(9).phase, Phase::Active(Active::Running { attempt: 9 }));
    assert!(!w.parks.contains(&(9, 9)));
    assert_eq!(w.parks.len(), 3);
}

#[test]
fn lifting_a_drain_before_its_runs_park_lets_them_go_on() {
    for with_tasks in [false, true] {
        let mut w = World::new(959, LIMITS);
        goal_tree(&mut w);
        procedure(&mut w, 5);
        drain(&mut w, Scope::Project, 1, 100);
        assert_eq!(lift(&mut w, 100, 1, with_tasks), Reply::Lifted { released: u32::from(with_tasks) });
        for number in [1, 2, 4, 5] {
            assert_eq!(w.record(number).placed, None);
        }
        park(&mut w);
        for number in [1, 2, 4] {
            due(&w, number);
        }
        w.send(Event::WakeProcedure { task: 5 });
        assert!(!w.contexts[&5].draining);
        procedure_step(&mut w, 5, tasks::ProcedureDecision::new(tasks::ProcedureEnd::Idle));
        assert_eq!(w.record(5).phase, Phase::Active(Active::Idle));
        if with_tasks {
            due(&w, 3);
        } else {
            placed(&w, 3, 100);
        }
        w.restart();
    }
}

#[test]
fn a_task_released_from_a_drain_is_exempt_and_runs() {
    for number in [2, 3] {
        let mut w = World::new(960, LIMITS);
        goal_tree(&mut w);
        drain(&mut w, Scope::Project, 1, 100);
        assert_eq!(release_from(&mut w, Party::Person(1), number), Reply::Done);
        if number == 2 {
            assert_eq!(w.record(2).phase, Phase::Active(Active::Running { attempt: 2 }));
            w.terminal(2, End::Parked);
        }
        due(&w, number);
        assert_eq!(w.placed_hold(100).expect("standing drain").exempt.as_ref(), &[number]);
        w.claim(number, 5);
        assert_eq!(w.record(number).phase, Phase::Active(Active::Running { attempt: 5 }));
        w.restart();
        assert!(!w.parks.contains(&(number, 5)));
    }
}

#[test]
fn a_drain_in_progress_goes_on_draining_after_a_restart() {
    let mut w = World::new(961, LIMITS);
    goal_tree(&mut w);
    procedure(&mut w, 5);
    drain(&mut w, Scope::Project, 1, 100);
    w.send(Event::WakeProcedure { task: 5 });
    procedure_step(&mut w, 5, tasks::ProcedureDecision::new(tasks::ProcedureEnd::Wait));
    w.terminal(4, End::Parked);
    w.restart();
    assert_eq!(w.parks, [(1, 1), (2, 2)].into_iter().collect());
    placed(&w, 3, 100);
    placed(&w, 4, 100);
    w.send(Event::WakeProcedure { task: 5 });
    assert!(w.contexts[&5].draining);
    for task in [1, 2] {
        w.terminal(task, End::Parked);
        placed(&w, task, 100);
    }
    procedure_step(&mut w, 5, tasks::ProcedureDecision::new(tasks::ProcedureEnd::Idle));
    placed(&w, 5, 100);
    w.restart();
    assert!(w.parks.is_empty());
    for task in 1..=5 {
        placed(&w, task, 100);
    }
}

#[test]
fn restart_refuses_an_unwrapped_task_under_a_drain_that_is_not_draining() {
    let mut w = World::new(962, LIMITS);
    goal_tree(&mut w);
    drain(&mut w, Scope::Project, 1, 100);
    let mut rows = w.records.values().filter(|row| row.key().live()).cloned().collect::<Vec<_>>();
    for row in &mut rows {
        if let Stored::Live(task) = row
            && task.number == 3
        {
            task.phase = Phase::Active(Active::Due);
        }
    }
    assert!(check_rows(rows, &w.env).iter().any(|reply| matches!(reply, tasks::Request::RestoreRefused { .. })));
}

#[test]
fn a_stop_inside_a_drain_holds_and_cancels_its_live_tasks_at_once() {
    let mut w = World::new(963, LIMITS);
    goal_tree(&mut w);
    drain(&mut w, Scope::Project, 1, 100);
    stop(&mut w, 2, 1, 101);
    for task in [2, 4] {
        placed(&w, task, 100);
    }
    assert_eq!(w.stops, [(2, 2), (4, 4)].into_iter().collect());
    w.restart();
    for task in [2, 4] {
        placed(&w, task, 100);
    }
}

#[test]
fn lifting_a_drain_hands_its_live_tasks_to_a_standing_stop() {
    let mut w = World::new(964, LIMITS);
    goal_tree(&mut w);
    drain(&mut w, Scope::Project, 1, 100);
    stop(&mut w, 2, 1, 101);
    assert_eq!(lift(&mut w, 100, 1, true), Reply::Lifted { released: 1 });
    for task in [2, 4] {
        placed(&w, task, 101);
    }
    assert_eq!(w.record(1).placed, None);
    assert_eq!(w.record(1).phase, Phase::Active(Active::Running { attempt: 1 }));
    w.terminal(1, End::Parked);
    due(&w, 1);
    w.restart();
}

#[test]
fn a_draining_run_that_finishes_normally_can_finish_closing() {
    let mut w = World::new(965, LIMITS);
    w.make(Party::Person(1), vec![task(1, &[])]);
    w.claim(1, 1);
    drain(&mut w, Scope::Project, 1, 100);
    w.finish(1);
    assert!(matches!(w.record(1).phase, Phase::Closing(_)));
    w.settle(1);
    assert!(matches!(w.results.get(&1), Some(tasks::Ending::Done(_))));
}
