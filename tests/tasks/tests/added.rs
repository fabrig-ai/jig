use jig_domain_tasks::{
    self as tasks, Active, Amendment, Cause, Control, End, Ending, Event, Executor, Funder, Hold, HoldKind, Holding,
    Key, Kind, Line, Name, Party, PersonAddress, Phase, ProcedureDecision, ProcedureEnd, Refusal, SavedResource,
    Status, Stored, Taken, TaskResult,
};
use jig_tasks_world::{LIMITS, Reply, World, task};
use skein_lib::{Duration, ReplyTo};

fn response(world: &mut World, event: impl FnOnce(ReplyTo) -> Event) -> Reply {
    let to = world.to();
    let call = to.into_token();
    world.send(event(ReplyTo::new(call)));
    world.replies[&call.raw()].clone()
}

fn amendment(dependencies: &[u64]) -> Amendment {
    Amendment {
        tracked: None,
        addressed: None,
        instructions: None,
        release: None,
        spec: None,
        wake: None,
        dependencies: Some(dependencies.into()),
        authority: None,
        llm: None,
        reason: Box::new([1]),
    }
}

fn amend(world: &mut World, by: Party, task: u64, dependencies: &[u64]) -> Reply {
    amend_with_places(world, by, task, dependencies, Box::new([]))
}

fn amend_ended(world: &mut World, by: Party, task: u64, dependency: u64) -> Reply {
    let Some(Stored::Ended(row)) = world.records.get(&Key::Ended(dependency)) else { panic!("stored ended row") };
    let status = match row.phase {
        Phase::Ended(Ending::Done(_)) => Status::Done,
        Phase::Ended(Ending::Failed { .. }) => Status::Failed,
        Phase::Ended(Ending::Cancelled { .. }) => Status::Cancelled,
        Phase::Waiting | Phase::Active(_) | Phase::Closing(_) | Phase::Held { .. } => panic!("ended row"),
    };
    let places = Box::new([tasks::DependencyPlace {
        number: row.number,
        project: row.project,
        requester: row.requester,
        root: row.root,
        depth: row.depth,
        status,
    }]);
    amend_with_places(world, by, task, &[dependency], places)
}

fn amend_with_places(
    world: &mut World,
    by: Party,
    task: u64,
    dependencies: &[u64],
    places: Box<[tasks::DependencyPlace]>,
) -> Reply {
    let message = world.record(task).last_message + 1;
    response(world, |reply_to| Event::Amend {
        places,
        reply_to,
        by,
        task,
        message,
        stop_run: false,
        funder: None,
        amendment: amendment(dependencies),
    })
}

fn refused(reply: Reply, task: u64, why: Refusal, blocked: &[u64]) {
    let Reply::Refused(problem) = reply else { panic!("expected refusal, got {reply:?}") };
    assert_eq!(problem.task, Some(task));
    assert_eq!(problem.why, why);
    assert_eq!(problem.blocked_by.as_deref(), Some(blocked));
}

fn pair() -> World {
    let mut world = World::new(850, LIMITS);
    assert_eq!(world.make(Party::Person(9), vec![task(1, &[]), task(2, &[])]), Reply::Made(vec![1, 2]));
    world
}

fn keep(world: &mut World) {
    assert_eq!(amend(world, Party::Person(9), 1, &[2]), Reply::Done);
    assert_eq!(world.record(1).waiting_on.as_ref(), [2]);
}

fn procedure(world: &mut World, number: u64, end: ProcedureEnd) -> Reply {
    let context = &world.contexts[&number];
    let read = context.inbox.last().map(|word| word.number);
    let step = world.record(number).attempt + 1;
    response(world, |reply_to| Event::Procedure {
        reply_to,
        task: number,
        step,
        read,
        decision: ProcedureDecision::new(end),
    })
}

fn report() -> TaskResult {
    TaskResult::Report { words: Box::new([7]) }
}

fn answer(world: &mut World, number: u64, result: TaskResult) -> Reply {
    response(world, |reply_to| Event::AnswerPerson { reply_to, task: number, person: 10, result })
}

fn cancel(world: &mut World, number: u64) {
    let message = world.record(number).last_message + 1;
    assert_eq!(
        response(world, |reply_to| Event::Control {
            reply_to,
            by: Party::Person(9),
            task: number,
            control: Control::Cancel {
                reason: Box::new([8]),
                grace: Some(Duration::from_secs(5)),
                message: Some(message)
            },
        }),
        Reply::Done
    );
}

fn done(world: &mut World, number: u64) {
    world.claim_fresh(number);
    world.finish(number);
    world.settle(number);
}

#[test]
fn an_amendment_keeps_a_due_agent_and_rejects_stale_preparation_and_claims() {
    let mut world = pair();
    keep(&mut world);
    assert_eq!(world.record(1).phase, Phase::Active(Active::Idle));
    assert!(
        matches!(response(&mut world, |reply_to| Event::Prepare { reply_to, task: 1 }), Reply::Refused(problem) if problem.why == Refusal::State)
    );
    assert!(matches!(response(&mut world, |reply_to| Event::Claim {
        reply_to, task: 1, attempt: 1, resumed: false, budget: 1, account: None, writes: Box::new([]),
    }), Reply::Refused(problem) if problem.why == Refusal::State));
}

#[test]
fn a_preparing_agent_is_kept_and_its_stale_preparation_cannot_make_it_due() {
    let mut world = pair();
    assert_eq!(response(&mut world, |reply_to| Event::Prepare { reply_to, task: 1 }), Reply::Done);
    keep(&mut world);
    world.send(Event::PreparationWaiting { task: 1 });
    world.send(Event::PreparationFailed { task: 1 });
    assert_eq!(world.record(1).phase, Phase::Active(Active::Idle));
}

#[test]
fn a_live_run_finishes_done_as_parked_keeps_saved_state_and_names_its_dependencies() {
    let mut world = pair();
    world.claim(1, 1);
    keep(&mut world);
    world.restart();
    assert_eq!(world.runs.get(&1), Some(&1));
    assert_eq!(world.record(1).waiting_on.as_ref(), [2]);
    let saved: Box<[SavedResource]> = Box::new([SavedResource { connector: 1, path: Box::new([Box::new([3])]) }]);
    let expected = saved.clone();
    assert_eq!(
        response(&mut world, |reply_to| Event::Activation {
            reply_to,
            task: 1,
            attempt: 1,
            end: End::Finished { result: report(), cancel_delegates: false },
            saved: Some(saved),
            cause: Cause::Priced { cumulative: 0 },
        }),
        Reply::Acknowledged(tasks::Accepted::New)
    );
    assert_eq!(world.record(1).saved, expected);
    assert_eq!(world.record(1).tries, tasks::Tries::NONE);
    assert_eq!(world.record(1).phase, Phase::Active(Active::Idle));
    let lines: Vec<_> = world
        .records
        .values()
        .filter_map(|row| {
            if let Stored::Activity(line) = row
                && line.task == 1
            {
                return Some(&line.line);
            }
            None
        })
        .collect();
    let end = lines
        .iter()
        .position(|line| matches!(line, Line::RunEnded { end: tasks::EndKind::Parked, .. }))
        .expect("fixture evidence");
    assert!(matches!(lines.get(end + 1), Some(Line::Kept { attempt: 1, pending }) if pending.as_ref() == [2]));
    done(&mut world, 2);
    assert_eq!(world.record(1).phase, Phase::Active(Active::Due));
    assert!(world.contexts[&1].pending.is_empty());
    assert_eq!(world.contexts[&1].added.as_ref(), [tasks::Added { task: 2, at: world.env.wall, by: Party::Person(9) }]);
    done(&mut world, 1);
    assert!(matches!(world.results[&1], Ending::Done(_)));
}

#[test]
fn a_kept_run_may_still_end_failed() {
    let mut world = pair();
    world.claim(1, 1);
    keep(&mut world);
    assert_eq!(
        world.terminal(
            1,
            End::Finished { result: TaskResult::Failure { reason: Box::new([5]) }, cancel_delegates: false }
        ),
        Reply::Acknowledged(tasks::Accepted::New)
    );
    world.settle(1);
    assert!(matches!(world.results[&1], Ending::Failed { .. }));
}

#[test]
fn every_person_answer_is_refused_while_kept_and_its_take_stands() {
    let mut world = World::new(851, LIMITS);
    let mut person = task(1, &[]);
    person.executor = Executor::Person(PersonAddress::Role(7));
    world.make(Party::Person(9), vec![person, task(2, &[])]);
    assert_eq!(response(&mut world, |reply_to| Event::TakePerson { reply_to, task: 1, person: 10 }), Reply::Done);
    keep(&mut world);
    for result in [report(), TaskResult::Failure { reason: Box::new([1]) }] {
        refused(answer(&mut world, 1, result), 1, Refusal::Kept, &[2]);
        assert_eq!(world.record(1).taken_by, Some(10));
    }
    done(&mut world, 2);
    assert_eq!(answer(&mut world, 1, report()), Reply::Done);
    world.settle(1);
    assert!(matches!(world.results[&1], Ending::Done(_)));
}

#[test]
fn a_procedures_done_step_is_refused_whole_while_kept_but_wait_and_failure_still_step() {
    let mut world = World::new(852, LIMITS);
    let mut new = task(1, &[]);
    new.executor = Executor::Procedure { connector: 1, code: 1 };
    world.make(Party::Person(9), vec![new, task(2, &[])]);
    assert_eq!(procedure(&mut world, 1, ProcedureEnd::Wait), Reply::Done);
    keep(&mut world);
    assert_eq!(world.contexts[&1].pending.as_ref(), [2]);
    let before = world.records.clone();
    refused(procedure(&mut world, 1, ProcedureEnd::Result(report())), 1, Refusal::Kept, &[2]);
    assert_eq!(world.records, before);
    assert_eq!(procedure(&mut world, 1, ProcedureEnd::Wait), Reply::Done);
    done(&mut world, 2);
    assert_eq!(world.record(1).phase, Phase::Active(Active::Due));
    assert!(world.contexts[&1].pending.is_empty());
    assert_eq!(procedure(&mut world, 1, ProcedureEnd::Result(report())), Reply::Done);
    world.settle(1);
    assert!(matches!(world.results[&1], Ending::Done(_)));

    // A separate kept procedure may still fail without its dependency ending.
    let mut world = World::new(852, LIMITS);
    let mut new = task(1, &[]);
    new.executor = Executor::Procedure { connector: 1, code: 1 };
    world.make(Party::Person(9), vec![new, task(2, &[])]);
    keep(&mut world);
    assert_eq!(
        procedure(&mut world, 1, ProcedureEnd::Result(TaskResult::Failure { reason: Box::new([1]) })),
        Reply::Done
    );
    world.settle(1);
    assert!(matches!(world.results[&1], Ending::Failed { .. }));
}

#[test]
fn a_procedures_invalid_done_step_is_refused_as_kept() {
    let mut world = World::new(852, LIMITS);
    let mut new = task(1, &[]);
    new.executor = Executor::Procedure { connector: 1, code: 1 };
    new.contract = tasks::Contract::Report { words: 1 };
    world.make(Party::Person(9), vec![new, task(2, &[])]);
    keep(&mut world);
    let before = world.records.clone();
    let invalid = TaskResult::Report { words: Box::new([7, 8]) };
    refused(procedure(&mut world, 1, ProcedureEnd::Result(invalid)), 1, Refusal::Kept, &[2]);
    assert_eq!(world.records, before);
}

#[test]
fn a_dependency_failing_later_holds_its_kept_dependent_and_stops_a_live_run() {
    let mut world = pair();
    world.claim(1, 1);
    keep(&mut world);
    world.claim(2, 1);
    world.terminal(2, End::Finished { result: TaskResult::Failure { reason: Box::new([1]) }, cancel_delegates: false });
    world.settle(2);
    assert!(matches!(world.record(1).phase, Phase::Held { why: Hold::Dependency(2), .. }));
    assert!(world.stops.contains(&(1, 1)));
    assert!(world.record(1).holds_taken);
}

#[test]
fn a_grace_cancels_a_kept_agent_without_a_live_run_at_once() {
    let mut world = pair();
    keep(&mut world);
    cancel(&mut world, 1);
    assert!(world.record(1).grace.is_none());
    world.settle(1);
    assert!(matches!(world.results[&1], Ending::Cancelled { .. }));
}

#[test]
fn a_grace_cancels_a_kept_person_or_procedure_at_once() {
    for executor in [Executor::Person(PersonAddress::Person(10)), Executor::Procedure { connector: 1, code: 1 }] {
        let mut world = World::new(853, LIMITS);
        let mut new = task(1, &[]);
        new.executor = executor;
        world.make(Party::Person(9), vec![new, task(2, &[])]);
        keep(&mut world);
        cancel(&mut world, 1);
        world.settle(1);
        assert!(matches!(world.results[&1], Ending::Cancelled { .. }));
    }
}

#[test]
fn a_run_parked_by_dependencies_during_a_grace_cancels_at_once() {
    let mut world = pair();
    world.claim(1, 1);
    keep(&mut world);
    cancel(&mut world, 1);
    assert!(world.record(1).grace.is_some());
    world.finish(1);
    world.settle(1);
    assert!(matches!(world.results[&1], Ending::Cancelled { .. }));
}

#[test]
fn additions_record_their_wall_and_maker_in_dependency_order_and_readdition_is_fresh() {
    let mut world = pair();
    world.make(Party::Person(9), vec![task(3, &[])]);
    let first = world.env.wall;
    assert_eq!(amend(&mut world, Party::Person(9), 1, &[3, 2]), Reply::Done);
    assert_eq!(
        world.record(1).added.as_ref(),
        [
            tasks::Added { task: 3, at: first, by: Party::Person(9) },
            tasks::Added { task: 2, at: first, by: Party::Person(9) }
        ]
    );
    assert_eq!(amend(&mut world, Party::Person(9), 1, &[2]), Reply::Done);
    world.elapse(Duration::from_millis(1));
    assert_eq!(amend(&mut world, Party::Person(9), 1, &[2, 3]), Reply::Done);
    assert_eq!(world.record(1).added.first().expect("fixture evidence").at, first);
    assert_eq!(world.record(1).added.last().expect("fixture evidence").at, world.env.wall);
    let history = world
        .records
        .values()
        .filter_map(|row| {
            if let Stored::History(history) = row {
                return history.amendment.as_ref();
            }
            None
        })
        .next_back()
        .expect("fixture evidence");
    assert_eq!(history.before.dependencies.as_ref(), [2]);
    assert_eq!(history.after.dependencies.as_ref(), [2, 3]);
}

#[test]
fn a_task_amender_may_name_any_live_task_below_it_but_not_an_outside_task() {
    let mut world = World::new(854, LIMITS);
    world.make(Party::Person(9), vec![task(1, &[]), task(5, &[])]);
    let mut child = task(2, &[]);
    child.funder = Funder::Task(1);
    child.numbers.budget = 30;
    child.authority.budget.spend = 30;
    world.make(Party::Task(1), vec![child]);
    let mut grandchild = task(3, &[]);
    grandchild.funder = Funder::Task(2);
    grandchild.numbers.budget = 10;
    grandchild.authority.budget.spend = 10;
    world.make(Party::Task(2), vec![grandchild]);
    let mut sibling = task(4, &[]);
    sibling.funder = Funder::Task(1);
    sibling.numbers.budget = 20;
    sibling.authority.budget.spend = 20;
    world.make(Party::Task(1), vec![sibling]);
    assert_eq!(amend(&mut world, Party::Task(1), 4, &[3]), Reply::Done);
    refused(amend(&mut world, Party::Task(1), 4, &[5]), 4, Refusal::Dependencies, &[5]);
}

#[test]
fn a_party_cannot_add_a_dependency_in_another_project() {
    let mut world = pair();
    let mut outside = task(3, &[]);
    outside.project = 2;
    outside.funder = Funder::Period { project: 2, period: 0 };
    response(&mut world, |reply_to| Event::OpenPeriod { reply_to, project: 2, period: 0, budget: 1000 });
    world.make(Party::Person(9), vec![outside]);
    refused(amend(&mut world, Party::Person(9), 1, &[3]), 1, Refusal::Dependencies, &[3]);
}

#[test]
fn amendment_circles_name_the_new_waiter_then_its_dependencies_in_order() {
    let mut world = pair();
    keep(&mut world);
    refused(amend(&mut world, Party::Person(9), 2, &[1]), 2, Refusal::Circle { cut: false }, &[2, 1]);
    refused(amend(&mut world, Party::Person(9), 2, &[2]), 2, Refusal::Circle { cut: false }, &[2]);
}

#[test]
fn a_dependency_on_an_ancestor_names_the_circle_in_order() {
    let mut world = World::new(855, LIMITS);
    world.make(Party::Person(9), vec![task(1, &[])]);
    let mut child = task(2, &[]);
    child.funder = Funder::Task(1);
    world.make(Party::Task(1), vec![child]);
    refused(amend(&mut world, Party::Person(9), 2, &[1]), 2, Refusal::Circle { cut: false }, &[2, 1]);
}

fn resource() -> Holding {
    Holding::Write { resource: Name { connector: 1, path: Box::new([Box::new([1])]) }, kind: 1 }
}

#[test]
fn a_waiter_given_a_dependency_leaves_the_queue_and_returns_ahead_of_a_later_waiter() {
    let mut world = World::new(856, LIMITS);
    world.configure_holds(1, vec![Kind { connector: 1, kind: 1, hold: HoldKind::Exclusive { taken: Taken::Waits } }]);
    for number in [1, 2, 3] {
        let mut new = task(number, &[]);
        new.holdings = Box::new([resource()]);
        world.make(Party::Person(9), vec![new]);
        world.elapse(Duration::from_millis(1));
    }
    let place = world.record(2).hold_wait_since;
    assert_eq!(amend(&mut world, Party::Person(9), 2, &[3]), Reply::Done);
    let mut third = task(4, &[]);
    third.holdings = Box::new([resource()]);
    world.make(Party::Person(9), vec![third]);
    done(&mut world, 1);
    assert!(world.record(3).holds_taken);
    assert!(!world.record(2).holds_taken && !world.record(4).holds_taken);
    done(&mut world, 3);
    assert!(world.record(2).holds_taken);
    assert!(!world.record(4).holds_taken);
    assert_eq!(world.record(2).hold_wait_since, None);
    assert!(place.is_some());
}

#[test]
fn a_holder_after_a_task_whose_delegate_queues_behind_it_is_a_circle() {
    let mut world = World::new(860, LIMITS);
    world.configure_holds(1, vec![Kind { connector: 1, kind: 1, hold: HoldKind::Exclusive { taken: Taken::Waits } }]);
    let mut holder = task(1, &[]);
    holder.holdings = Box::new([resource()]);
    world.make(Party::Person(9), vec![holder, task(2, &[])]);
    let mut child = task(3, &[]);
    child.funder = Funder::Task(2);
    child.holdings = Box::new([resource()]);
    assert_eq!(world.make(Party::Task(2), vec![child]), Reply::Made(vec![3]));
    assert_eq!(world.record(3).phase, Phase::Waiting);
    refused(amend(&mut world, Party::Person(9), 1, &[2]), 1, Refusal::Circle { cut: false }, &[1, 2, 3]);
}

#[test]
fn an_already_done_dependency_is_recorded_as_met_without_keeping_the_task() {
    let mut world = pair();
    world.make(Party::Person(9), vec![task(3, &[2])]);
    done(&mut world, 2);
    assert!(world.records.contains_key(&Key::Stub(2)));
    assert_eq!(amend_ended(&mut world, Party::Person(9), 1, 2), Reply::Done);
    assert!(world.record(1).waiting_on.is_empty());
    assert_eq!(world.record(1).dependencies.as_ref(), [2]);
    assert_eq!(world.record(1).added.as_ref(), [tasks::Added { task: 2, at: world.env.wall, by: Party::Person(9) }]);
    done(&mut world, 1);
    assert!(matches!(world.results[&1], Ending::Done(_)));
}

#[test]
fn an_already_failed_or_cancelled_dependency_is_refused_naming_it() {
    for how in [Status::Failed, Status::Cancelled] {
        let mut world = pair();
        world.make(Party::Person(9), vec![task(3, &[2])]);
        if how == Status::Failed {
            world.claim(2, 1);
            world.terminal(
                2,
                End::Finished { result: TaskResult::Failure { reason: Box::new([1]) }, cancel_delegates: false },
            );
        } else {
            let reply_to = world.to();
            world.send(Event::Control {
                reply_to,
                by: Party::Person(9),
                task: 2,
                control: Control::Cancel { reason: Box::new([]), grace: None, message: None },
            });
        }
        world.settle(2);
        refused(amend_ended(&mut world, Party::Person(9), 1, 2), 1, Refusal::DependencyEnded { how }, &[2]);
    }
}

#[test]
fn an_ended_dependency_without_a_loaded_place_is_unknown_to_the_child() {
    let mut world = pair();
    done(&mut world, 2);
    assert!(!world.records.contains_key(&Key::Stub(2)));
    assert_eq!(
        amend(&mut world, Party::Person(9), 1, &[2]),
        Reply::Refused(tasks::Problem::new(Some(2), Refusal::Unknown))
    );
}

#[test]
fn a_dependency_on_a_person_goes_on_after_the_answer() {
    let mut world = World::new(857, LIMITS);
    let mut person = task(2, &[]);
    person.executor = Executor::Person(PersonAddress::Person(10));
    world.make(Party::Person(9), vec![task(1, &[2]), person]);
    assert_eq!(world.record(1).phase, Phase::Waiting);
    assert_eq!(answer(&mut world, 2, report()), Reply::Done);
    world.settle(2);
    assert_eq!(world.record(1).phase, Phase::Active(Active::Due));
    done(&mut world, 1);
}

#[test]
fn the_referee_rejects_fresh_runs_and_done_ends_after_a_committed_dependency_amendment() {
    use jig_tasks_world::referee::{Seen, Tasks};
    use skein_world::domain::{Referee, Verdict};
    for violation in [
        Seen::Assigned { task: 1, attempt: 1, after: 0, adopted: false, settings: 1 },
        Seen::Ended { task: 1, status: Status::Done, after: 0 },
    ] {
        let mut referee = Referee::new(Tasks::default());
        for seen in [
            Seen::Made {
                task: 1,
                parent: Party::Person(9),
                dependencies: vec![],
                depth: 0,
                settings: tasks::SettingsRevision { task: 1, run: Some(1) },
                newest: 1,
            },
            Seen::Made {
                task: 2,
                parent: Party::Person(9),
                dependencies: vec![],
                depth: 0,
                settings: tasks::SettingsRevision { task: 1, run: Some(1) },
                newest: 1,
            },
            Seen::Amended { task: 1, dependencies: vec![2] },
            Seen::Closing { task: 1 },
            Seen::Settled { task: 1 },
            violation,
        ] {
            referee.observe(skein_lib::Time::ZERO, seen, &mut Vec::new());
        }
        assert!(matches!(referee.verdict(), Verdict::Failed(_)));
    }
}

#[test]
fn the_referee_allows_adoption_of_a_run_live_before_its_task_was_kept() {
    use jig_tasks_world::referee::{Seen, Tasks};
    use skein_world::domain::{Referee, Verdict};
    let mut referee = Referee::new(Tasks::default());
    for seen in [
        Seen::Made {
            task: 1,
            parent: Party::Person(9),
            dependencies: vec![],
            depth: 0,
            settings: tasks::SettingsRevision { task: 1, run: Some(1) },
            newest: 1,
        },
        Seen::Made {
            task: 2,
            parent: Party::Person(9),
            dependencies: vec![],
            depth: 0,
            settings: tasks::SettingsRevision { task: 1, run: Some(1) },
            newest: 1,
        },
        Seen::Assigned { task: 1, attempt: 1, after: 0, adopted: false, settings: 1 },
        Seen::Amended { task: 1, dependencies: vec![2] },
        Seen::Assigned { task: 1, attempt: 1, after: 0, adopted: true, settings: 1 },
    ] {
        referee.observe(skein_lib::Time::ZERO, seen, &mut Vec::new());
    }
    assert!(!matches!(referee.verdict(), Verdict::Failed(_)));
}

#[test]
fn a_proposal_below_a_kept_task_waits_at_the_holder_above() {
    let mut world = World::new(858, LIMITS);
    world.make(Party::Person(9), vec![task(1, &[]), task(4, &[])]);
    for (number, parent, budget) in [(2, 1, 60), (3, 2, 20)] {
        let mut child = task(number, &[]);
        child.funder = Funder::Task(parent);
        child.numbers.budget = budget;
        child.authority.budget.spend = budget;
        assert_eq!(world.make(Party::Task(parent), vec![child]), Reply::Made(vec![number]));
    }
    assert!(world.delegation(2).deciding);
    assert_eq!(amend(&mut world, Party::Person(9), 2, &[4]), Reply::Done);
    assert!(!world.delegation(2).deciding);
    assert!(world.delegation(1).deciding);
    let mut proposed = task(5, &[]);
    proposed.funder = Funder::Task(3);
    let proposal = tasks::Proposal {
        number: 100,
        proposer: 3,
        project: 1,
        action: tasks::ProposalAction::Batch(Box::new([proposed])),
        reason: Box::new([1]),
        as_holder: false,
        state: tasks::ProposalState::Pending { holder: tasks::ProposalHolder::Task(1), since: world.env.wall },
    };
    assert_eq!(response(&mut world, |reply_to| Event::Propose { reply_to, proposal }), Reply::Done);
    assert!(matches!(
        world.record(3).proposal.as_ref().expect("proposal").state,
        tasks::ProposalState::Pending { holder: tasks::ProposalHolder::Task(1), .. }
    ));
    done(&mut world, 4);
    assert!(world.delegation(2).deciding);
}

#[test]
fn restore_refuses_added_entries_outside_or_out_of_order_in_the_dependency_list() {
    let mut world = pair();
    world.make(Party::Person(9), vec![task(3, &[])]);
    assert_eq!(amend(&mut world, Party::Person(9), 1, &[2, 3]), Reply::Done);
    let entry = |task| tasks::Added { task, at: world.env.wall, by: Party::Person(9) };
    for added in [vec![entry(3), entry(2)], vec![entry(2), entry(2)], vec![entry(4)]] {
        let mut row = world.record(1).clone();
        row.added = added.into_boxed_slice();
        let mut domain = jig_tasks_world::domain(&LIMITS, 859);
        let mut out = skein_lib::Queue::with_capacity(tasks::max_out(&LIMITS));
        tasks::step(&mut domain, &world.env, Event::Restore { record: Stored::Live(Box::new(row)) }, &mut out);
        assert!(
            matches!(out.pop(), Some(tasks::Request::RestoreRefused { problem }) if problem.why == Refusal::Restore)
        );
    }
}

#[test]
fn a_claim_committed_before_the_amendment_stays_live_and_its_done_finish_parks() {
    let mut world = pair();
    assert_eq!(response(&mut world, |reply_to| Event::Prepare { reply_to, task: 1 }), Reply::Done);
    assert_eq!(
        response(&mut world, |reply_to| Event::Claim {
            reply_to,
            task: 1,
            attempt: 1,
            resumed: false,
            budget: 1,
            account: None,
            writes: Box::new([]),
        }),
        Reply::Done
    );
    // The world records assignment independently from the task's Claimed phase.
    world.runs.insert(1, 1);
    world.observe(jig_tasks_world::referee::Seen::Assigned {
        task: 1,
        attempt: 1,
        after: 0,
        adopted: false,
        settings: 1,
    });
    keep(&mut world);
    assert_eq!(world.record(1).phase, Phase::Active(Active::Claimed { attempt: 1 }));
    world.send(Event::Started { task: 1, attempt: 1 });
    world.finish(1);
    assert_eq!(world.record(1).phase, Phase::Active(Active::Idle));
    assert_eq!(world.record(1).tries, tasks::Tries::NONE);
}

#[test]
fn a_kept_runs_invalid_result_still_spends_an_invalid_try() {
    let mut world = pair();
    world.claim(1, 1);
    keep(&mut world);
    world.terminal(
        1,
        End::Finished { result: TaskResult::Approved { state: Box::new([]), words: None }, cancel_delegates: false },
    );
    assert_eq!(world.record(1).tries.invalid, 1);
    assert_eq!(world.record(1).invalid_result, Some(tasks::InvalidResult::Form));
    assert!(matches!(world.record(1).phase, Phase::Active(Active::BackingOff { .. })));
}

#[test]
fn a_restored_met_and_amendment_leave_all_ordinary_sender_room_available() {
    let mut world = pair();
    assert_eq!(amend(&mut world, Party::Person(9), 1, &[]), Reply::Done);
    // Restore accounting complements the live Met story in met.rs.
    let Stored::Live(row) = world.records.get_mut(&Key::Live(1)).expect("live row") else { panic!("live") };
    row.last_message += 1;
    let mut inbox = row.inbox.to_vec();
    inbox.push(tasks::Word {
        number: row.last_message,
        from: Party::Person(9),
        kind: tasks::MessageKind::Met { dependency: 2, removed: true },
        words: Box::new([]),
        at: world.env.wall,
        hits: 1,
        eligible: true,
    });
    row.inbox = inbox.into_boxed_slice();
    world.restart();
    for _ in 0..LIMITS.inbox_messages {
        let number = world.record(1).last_message + 1;
        let message = tasks::Word {
            number,
            from: Party::Person(9),
            kind: tasks::MessageKind::Words,
            words: Box::new([1]),
            at: world.env.wall,
            hits: 1,
            eligible: true,
        };
        assert_eq!(
            response(&mut world, |reply_to| Event::Message { reply_to, project: 1, task: 1, word: message }),
            Reply::Done
        );
    }
    assert_eq!(world.record(1).inbox.len(), usize::try_from(LIMITS.inbox_messages).expect("limit fits") + 2);
}

#[test]
fn removing_the_last_live_name_of_a_met_dependency_prunes_its_stub_before_restart() {
    let mut world = pair();
    world.make(Party::Person(9), vec![task(3, &[2])]);
    done(&mut world, 2);
    assert_eq!(amend_ended(&mut world, Party::Person(9), 1, 2), Reply::Done);
    assert_eq!(amend(&mut world, Party::Person(9), 3, &[]), Reply::Done);
    assert!(world.records.contains_key(&Key::Stub(2)));
    assert_eq!(amend(&mut world, Party::Person(9), 1, &[]), Reply::Done);
    assert!(!world.records.contains_key(&Key::Stub(2)));
    world.restart();
    assert!(world.record(1).added.is_empty());
}

#[test]
fn cancelling_a_kept_task_still_accepts_its_live_runs_partial_result() {
    let mut world = pair();
    world.claim(1, 1);
    keep(&mut world);
    assert_eq!(
        response(&mut world, |reply_to| Event::Control {
            reply_to,
            by: Party::Person(9),
            task: 1,
            control: Control::Cancel { reason: Box::new([8]), grace: None, message: None },
        }),
        Reply::Done
    );
    world.finish(1);
    world.settle(1);
    assert!(matches!(world.results[&1], Ending::Cancelled { result: Some(_), .. }));
}

fn end_dependency(world: &mut World, number: u64, how: Status) {
    match how {
        Status::Failed => {
            world.claim_fresh(number);
            assert_eq!(
                world.terminal(
                    number,
                    End::Finished { result: TaskResult::Failure { reason: Box::new([1]) }, cancel_delegates: false }
                ),
                Reply::Acknowledged(tasks::Accepted::New)
            );
        }
        Status::Cancelled => {
            assert_eq!(
                response(world, |reply_to| Event::Control {
                    reply_to,
                    by: Party::Person(9),
                    task: number,
                    control: Control::Cancel { reason: Box::new([1]), grace: None, message: None },
                }),
                Reply::Done
            );
        }
        Status::Done => panic!("fixture ends unsuccessfully"),
    }
    world.settle(number);
}

fn release(world: &mut World, number: u64) -> Reply {
    response(world, |reply_to| Event::Control {
        reply_to,
        by: Party::Person(9),
        task: number,
        control: Control::Release { again: None, message: None },
    })
}

fn dependency_hold(world: &World, number: u64, was: tasks::Was, dependency: u64, revision: u64) {
    let row = world.record(number);
    assert_eq!(row.phase, Phase::Held { was, why: Hold::Dependency(dependency), source: None });
    assert_eq!(row.revisions.hold, revision);
    assert!(
        matches!(row.escalation, tasks::Escalation::Waiting {
        revision: held, holder: tasks::EscalationHolder::Person(9), ..
    } if held == revision),
        "fresh routed escalation: {:?}",
        row.escalation
    );
}

fn remove_and_release(world: &mut World, number: u64) -> Reply {
    let mut amendment = amendment(&[]);
    amendment.release = Some(world.record(number).revisions.hold);
    let message = world.record(number).last_message + 1;
    response(world, |reply_to| Event::Amend {
        places: Box::new([]),
        reply_to,
        by: Party::Person(9),
        task: number,
        message,
        stop_run: false,
        funder: None,
        amendment,
    })
}

fn no_met(world: &World, number: u64) {
    for word in &world.record(number).inbox {
        assert!(!matches!(word.kind, tasks::MessageKind::Met { .. }), "failed dependency was no longer pending");
    }
}

#[test]
fn releasing_a_waiter_reholds_on_a_failed_or_cancelled_dependency_with_a_fresh_escalation() {
    for how in [Status::Failed, Status::Cancelled] {
        let mut world = World::new(881, LIMITS);
        world.make(Party::Person(9), vec![task(1, &[2]), task(2, &[])]);
        end_dependency(&mut world, 2, how);
        let revision = world.record(1).revisions.hold;
        dependency_hold(&world, 1, tasks::Was::Waiting, 2, revision);
        world.restart();
        assert_eq!(release(&mut world, 1), Reply::Done);
        dependency_hold(&world, 1, tasks::Was::Waiting, 2, revision + 1);
        assert!(!world.activations.contains(&1));
        assert_eq!(
            response(&mut world, |reply_to| Event::Claim {
                reply_to,
                task: 1,
                attempt: 2,
                resumed: false,
                budget: 1,
                account: None,
                writes: Box::new([]),
            }),
            Reply::Refused(tasks::Problem::new(Some(1), Refusal::State))
        );
        assert_eq!(remove_and_release(&mut world, 1), Reply::Done);
        assert_eq!(world.record(1).phase, Phase::Active(Active::Due));
        no_met(&world, 1);
        done(&mut world, 1);
    }
}

#[test]
fn a_kept_live_agents_release_reholds_and_removing_its_failed_dependency_resumes_it() {
    let mut world = pair();
    world.claim_fresh(1);
    keep(&mut world);
    end_dependency(&mut world, 2, Status::Failed);
    let was = tasks::Was::Active(Active::Running { attempt: world.record(1).attempt });
    let revision = world.record(1).revisions.hold;
    dependency_hold(&world, 1, was.clone(), 2, revision);
    assert_eq!(release(&mut world, 1), Reply::Done);
    dependency_hold(&world, 1, was, 2, revision + 1);
    assert_eq!(world.terminal(1, End::Parked), Reply::Acknowledged(tasks::Accepted::New));
    assert_eq!(remove_and_release(&mut world, 1), Reply::Done);
    assert_eq!(world.record(1).phase, Phase::Active(Active::Due));
    no_met(&world, 1);
    done(&mut world, 1);
}

#[test]
fn a_release_scans_all_dependencies_in_list_order_even_after_the_hold_reason_changed() {
    let mut world = World::new(882, LIMITS);
    world.make(Party::Person(9), vec![task(1, &[2, 3]), task(2, &[]), task(3, &[])]);
    end_dependency(&mut world, 3, Status::Failed);
    let revision = world.record(1).revisions.hold;
    assert_eq!(release(&mut world, 1), Reply::Done);
    dependency_hold(&world, 1, tasks::Was::Waiting, 3, revision + 1);
    end_dependency(&mut world, 2, Status::Failed);
    world.send(Event::Hold { task: 1, why: Hold::Budget });
    let revision = world.record(1).revisions.hold;
    assert_eq!(release(&mut world, 1), Reply::Done);
    dependency_hold(&world, 1, tasks::Was::Waiting, 2, revision + 1);
}

fn dependency_compound(world: &mut World, operations: Vec<tasks::Operation>) -> Reply {
    let mut staged = skein_lib::List::with_capacity(u32::try_from(operations.len()).expect("small fixture"));
    for operation in operations {
        staged.push(operation).expect("fixture operations bound");
    }
    response(world, |reply_to| Event::Compound {
        reply_to,
        by: jig_domain_tasks::Party::Person(9),
        project: 1,
        operations: staged,
        period: 0,
        pool_budget: 1_000,
        period_budget: 10_000,
    })
}

fn release_operation() -> tasks::Operation {
    tasks::Operation::Release { task: 1, again: None, message: None }
}

fn removing_operation(world: &World, release: Option<u64>) -> tasks::Operation {
    let mut amendment = amendment(&[]);
    amendment.release = release;
    tasks::Operation::Amend {
        task: 1,
        message: world.record(1).last_message + 1,
        stop_run: false,
        amendment: Box::new(amendment),
    }
}

fn failed_waiter() -> World {
    let mut world = World::new(883, LIMITS);
    world.make(Party::Person(9), vec![task(1, &[2]), task(2, &[])]);
    end_dependency(&mut world, 2, Status::Failed);
    world
}

#[test]
fn a_compounds_later_release_sees_the_dependency_hold_projected_by_its_first_release() {
    let mut world = failed_waiter();
    let revision = world.record(1).revisions.hold;
    assert_eq!(dependency_compound(&mut world, vec![release_operation(), release_operation()]), Reply::Done);
    dependency_hold(&world, 1, tasks::Was::Waiting, 2, revision + 2);
    let remove = removing_operation(&world, None);
    assert_eq!(dependency_compound(&mut world, vec![remove, release_operation()]), Reply::Done);
    assert_eq!(world.record(1).phase, Phase::Active(Active::Due));
}

#[test]
fn a_compound_can_remove_the_failed_dependency_and_release_in_its_amendment() {
    let mut world = failed_waiter();
    let revision = world.record(1).revisions.hold;
    let remove = removing_operation(&world, Some(revision));
    assert_eq!(
        dependency_compound(&mut world, vec![remove.clone(), release_operation()]),
        Reply::Refused(tasks::Problem::new(Some(1), Refusal::State))
    );
    dependency_hold(&world, 1, tasks::Was::Waiting, 2, revision);
    assert_eq!(dependency_compound(&mut world, vec![remove]), Reply::Done);
    assert_eq!(world.record(1).phase, Phase::Active(Active::Due));
}

#[test]
fn a_compounds_amendment_release_projects_the_new_hold_revision_for_its_next_amendment() {
    let mut world = failed_waiter();
    let revision = world.record(1).revisions.hold;
    let mut retained = amendment(&[2]);
    retained.release = Some(revision);
    let message = world.record(1).last_message + 1;
    let mut removed = amendment(&[]);
    removed.release = Some(revision + 1);
    assert_eq!(
        dependency_compound(
            &mut world,
            vec![
                tasks::Operation::Amend { task: 1, message, stop_run: false, amendment: Box::new(retained) },
                tasks::Operation::Amend {
                    task: 1,
                    message: message + 1,
                    stop_run: false,
                    amendment: Box::new(removed)
                },
            ]
        ),
        Reply::Done
    );
    assert_eq!(world.record(1).phase, Phase::Active(Active::Due));
    assert_eq!(world.record(1).revisions.hold, revision + 1);
}

#[test]
fn an_escalation_resolution_reholds_on_the_failed_dependency_with_a_fresh_escalation() {
    let mut world = failed_waiter();
    let revision = world.record(1).revisions.hold;
    let reply_to = world.to();
    world.stage(Event::DecideEscalation {
        reply_to,
        task: 1,
        revision,
        by: 9,
        entry: None,
        decision: tasks::EscalationDecision::Release,
    });
    world.durable();
    dependency_hold(&world, 1, tasks::Was::Waiting, 2, revision + 1);
    // As in kept.rs, commit the decision then crash before its terminal is delivered.
    world.restart();
    dependency_hold(&world, 1, tasks::Was::Waiting, 2, revision + 1);
}

#[test]
fn a_person_and_a_procedure_rehold_on_the_failed_dependency_as_an_agent_does() {
    for executor in [Executor::Person(PersonAddress::Person(10)), Executor::Procedure { connector: 1, code: 1 }] {
        let mut world = World::new(884, LIMITS);
        let mut waiter = task(1, &[2]);
        waiter.executor = executor;
        world.make(Party::Person(9), vec![waiter, task(2, &[])]);
        end_dependency(&mut world, 2, Status::Failed);
        let revision = world.record(1).revisions.hold;
        dependency_hold(&world, 1, tasks::Was::Waiting, 2, revision);
        assert_eq!(release(&mut world, 1), Reply::Done);
        dependency_hold(&world, 1, tasks::Was::Waiting, 2, revision + 1);
    }
}

#[test]
fn a_closing_task_releases_to_settlement_even_with_a_failed_dependency() {
    let mut world = pair();
    world.claim_fresh(1);
    keep(&mut world);
    assert_eq!(
        world.terminal(
            1,
            End::Finished { result: TaskResult::Failure { reason: Box::new([1]) }, cancel_delegates: false }
        ),
        Reply::Acknowledged(tasks::Accepted::New)
    );
    world.send(Event::Hold { task: 1, why: Hold::Budget });
    assert!(matches!(world.record(1).phase, Phase::Held { was: tasks::Was::Closing(_), .. }));
    end_dependency(&mut world, 2, Status::Failed);
    assert_eq!(release(&mut world, 1), Reply::Done);
    assert!(matches!(world.record(1).phase, Phase::Closing(_)));
    world.settle(1);
    assert!(matches!(world.results.get(&1), Some(Ending::Failed { .. })));
}
