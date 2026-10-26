//! The application supplies a period boundary; joined project documents retain it.
use super::*;

#[test]
fn recurring_lines_join_the_supplied_period_and_its_durable_opening_fact() {
    let (mut core, mut limits) = tree_core();
    let (authenticated, auth_limits) = watch_core();
    core.people = authenticated.people;
    limits.people = auth_limits.people;
    let mut recurring = new_row(&core, 10, tasks::Funder::Period { project: 1, period: 1 }, None);
    recurring.executor = tasks::Executor::Procedure { connector: RECURRING, code: 1 };
    recurring.numbers = tasks::Numbers { budget: 0, spent: 0, spent_below: 0, reserved: 0 };
    recurring.recurring = Some(Box::new(tasks::RecurringTemplate {
        key: 1,
        batch: Box::new([new_row(&core, 1, tasks::Funder::Task(10), None)]),
        overlap: tasks::RecurringOverlap::Skip,
    }));
    send_task(
        &mut core,
        &limits,
        tasks::Event::Make {
            reply_to: ReplyTo::new(Token::new(1)),
            creator: tasks::Party::Deployment { project: 1 },
            batch: Box::new([recurring]),
        },
    );
    let Document::Project(project) =
        make(&core, &limits, 1, &Subject::Project { project: 1 }, &window(), Wall::EPOCH).unwrap()
    else {
        panic!("project")
    };
    assert_eq!(project.recurring.len(), 1);
    let line = project.recurring.get(0).unwrap();
    assert_eq!(line.current_period, 1);
    assert_eq!(line.began, Some(Wall::EPOCH));
    assert_eq!(line.next, None);
    let mut deployment = core.counters.deployment();
    deployment.tasks = 10;
    core.counters = Counters::new(deployment);
    let next = Some(Wall::from_nanos(6000));
    let out = watch_outputs(&mut core, &limits, Event::Period { project: 1, period: 1, budget: 1000, next });
    let retained = *core.tasks.funding(tasks::Funder::Period { project: 1, period: 1 }).unwrap();
    assert_eq!(retained.period.unwrap().next, next);
    let mut saved = false;
    for request in &out {
        if let Request::Write(Write::Save(Record::Tasks(tasks::Stored::Ledger(row)))) = request
            && row.funder == (tasks::Funder::Period { project: 1, period: 1 })
        {
            assert_eq!(row.period.unwrap().next, next);
            saved = true;
        }
    }
    assert!(saved);
    let Document::Project(project) =
        make(&core, &limits, 1, &Subject::Project { project: 1 }, &window(), Wall::EPOCH).unwrap()
    else {
        panic!("project")
    };
    let line = project.recurring.get(0).unwrap();
    assert_eq!(line.current_period, 1);
    assert_eq!(line.began, Some(Wall::EPOCH));
    assert_eq!(line.next, next);
    // Restore only the owner's durable live rows and ledgers at a different wall.
    let mut restored = fixtures::fresh_tasks(&core, &limits);
    let env = Env { now: Time::ZERO, wall: Wall::from_nanos(9000), limits: limits.tasks };
    let mut requests = Queue::with_capacity(tasks::max_out(&limits.tasks));
    for (_, ledger) in core.tasks.funding_records() {
        tasks::step(
            &mut restored,
            &env,
            tasks::Event::Restore { record: tasks::Stored::Ledger(*ledger) },
            &mut requests,
        );
    }
    for row in core.tasks.view_tasks() {
        let record = Box::new(core.tasks.task(row.number).unwrap().clone());
        tasks::step(&mut restored, &env, tasks::Event::Restore { record: tasks::Stored::Live(record) }, &mut requests);
    }
    tasks::step(&mut restored, &env, tasks::Event::Check, &mut requests);
    tasks::step(&mut restored, &env, tasks::Event::Restored, &mut requests);
    while let Some(request) = requests.pop() {
        if let tasks::Request::RestoreRefused { problem } = request {
            panic!("restore {problem:?}");
        }
    }
    core.tasks = restored;
    let Document::Project(project) =
        make(&core, &limits, 1, &Subject::Project { project: 1 }, &window(), Wall::EPOCH).unwrap()
    else {
        panic!("project")
    };
    let line = project.recurring.get(0).unwrap();
    assert_eq!(line.current_period, 1);
    assert_eq!(line.began, Some(Wall::EPOCH));
    assert_eq!(line.next, next);
}
