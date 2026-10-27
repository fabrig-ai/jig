use jig_domain_tasks::{Event, Operation, Party};
use jig_tasks_world::{LIMITS, Reply, World, task};
use skein_lib::{Duration, List};
const ROOT: u64 = 1;
const CHILD: u64 = 2;
const REQUESTER: u64 = 9;
const PROJECT: u32 = 1;
const FINISH: u64 = 1;

#[test]
fn a_compound_closing_a_task_and_moving_its_delegate_survives_a_restart() {
    let mut world = World::new(204, LIMITS);
    world.make(Party::Person(REQUESTER), vec![task(ROOT, &[])]);
    let mut child = task(CHILD, &[]);
    child.funder = jig_domain_tasks::Funder::Task(ROOT);
    world.make(Party::Task(ROOT), vec![child]);
    let mut operations = List::with_capacity(2);
    operations
        .push(Operation::Cancel {
            task: ROOT,
            reason: b"finish".as_slice().into(),
            grace: Some(Duration::from_secs(5)),
            message: Some(FINISH),
        })
        .expect("compound fixture");
    operations.push(Operation::Move { task: CHILD, reason: b"keep".as_slice().into() }).expect("compound fixture");
    let reply_to = world.to();
    world.send(Event::Compound {
        reply_to,
        by: jig_domain_tasks::Party::Person(REQUESTER),
        project: PROJECT,
        operations,
        period: 0,
        pool_budget: 1_000,
        period_budget: 10_000,
    });
    assert_eq!(world.replies.last_key_value().expect("compound fixture").1, &Reply::Done);
    world.restart();
    assert_eq!(world.record(ROOT).grace.as_ref().expect("compound fixture").message, FINISH);
    assert_eq!(world.record(CHILD).requester, Party::Person(REQUESTER));
    assert_eq!(world.record(CHILD).root, CHILD);
}

fn deployment(world: &mut World, by: Party, operations: Vec<Operation>) -> Reply {
    let mut bounded = List::with_capacity(u32::try_from(operations.len()).expect("bounded compound fixture"));
    for operation in operations {
        bounded.push(operation).expect("bounded compound fixture");
    }
    let to = world.to().into_token();
    world.send(Event::Compound {
        reply_to: skein_lib::ReplyTo::new(to),
        by,
        project: PROJECT,
        operations: bounded,
        period: 0,
        pool_budget: 0,
        period_budget: 0,
    });
    world.replies[&to.raw()].clone()
}
fn cancel(task: u64) -> Operation {
    Operation::Cancel { task, reason: b"closed".as_slice().into(), grace: None, message: None }
}
#[test]
fn the_deployments_compound_cancels_its_projects_tasks() {
    let mut world = World::new(205, LIMITS);
    world.make(Party::Person(REQUESTER), vec![task(ROOT, &[]), task(CHILD, &[])]);
    assert_eq!(
        deployment(&mut world, Party::Deployment { project: PROJECT }, vec![cancel(CHILD), cancel(ROOT)]),
        Reply::Done
    );
    for task in [ROOT, CHILD] {
        let jig_domain_tasks::Phase::Closing(closing) = &world.record(task).phase else { panic!("cancelled task") };
        assert!(matches!(
            closing.ending,
            jig_domain_tasks::Ending::Cancelled { by: Party::Deployment { project: PROJECT }, .. }
        ));
    }
}
#[test]
fn the_deployments_grace_and_its_ask_to_finish_survive_a_restart() {
    let mut world = World::new(206, LIMITS);
    world.make(Party::Person(REQUESTER), vec![task(ROOT, &[])]);
    let close = Operation::Cancel {
        task: ROOT,
        reason: b"closed".as_slice().into(),
        grace: Some(Duration::from_secs(5)),
        message: Some(FINISH),
    };
    assert_eq!(deployment(&mut world, Party::Deployment { project: PROJECT }, vec![close]), Reply::Done);
    world.restart();
    let grace = world.record(ROOT).grace.as_ref().expect("the deployment's grace is kept");
    assert_eq!((grace.by, grace.message), (Party::Deployment { project: PROJECT }, FINISH));
    assert!(world.record(ROOT).inbox.iter().any(|word| word.number == FINISH
        && word.from == Party::Deployment { project: PROJECT }
        && matches!(word.kind, jig_domain_tasks::MessageKind::FinishRequested { .. })));
}
#[test]
fn the_deployments_compound_refuses_any_other_operation_or_project_whole() {
    use jig_domain_tasks::{Amendment, MessageKind, Refusal, Word};
    let amendment = Amendment {
        tracked: None,
        addressed: None,
        instructions: None,
        release: None,
        spec: None,
        wake: None,
        dependencies: None,
        authority: None,
        llm: None,
        reason: Box::new([]),
    };
    let message = Word {
        number: 1,
        from: Party::Deployment { project: PROJECT },
        kind: MessageKind::Words,
        words: b"no".as_slice().into(),
        at: skein_lib::Wall::EPOCH,
        hits: 1,
        eligible: false,
    };
    let rows = [
        (
            "amend",
            Party::Deployment { project: PROJECT },
            Operation::Amend { task: CHILD, message: 1, stop_run: false, amendment: Box::new(amendment) },
            PROJECT,
        ),
        ("stop", Party::Deployment { project: PROJECT }, Operation::Stop { task: CHILD, hold: 100 + CHILD }, PROJECT),
        (
            "move",
            Party::Deployment { project: PROJECT },
            Operation::Move { task: CHILD, reason: Box::new([]) },
            PROJECT,
        ),
        (
            "release",
            Party::Deployment { project: PROJECT },
            Operation::Release { task: CHILD, again: None, message: None },
            PROJECT,
        ),
        ("message", Party::Deployment { project: PROJECT }, Operation::Message { task: CHILD, word: message }, PROJECT),
        ("another project", Party::Deployment { project: PROJECT }, cancel(CHILD), PROJECT + 1),
        ("task maker", Party::Task(ROOT), cancel(CHILD), PROJECT),
        ("another deployment project", Party::Deployment { project: PROJECT + 1 }, cancel(CHILD), PROJECT),
    ];
    for (name, by, operation, project) in rows {
        let mut world = World::new(206, LIMITS);
        let mut child = task(CHILD, &[]);
        child.project = project;
        world.make(Party::Person(REQUESTER), vec![task(ROOT, &[]), child]);
        let before = world.records.clone();
        let Reply::Refused(problem) = deployment(&mut world, by, vec![cancel(ROOT), operation]) else {
            panic!("{name} refused")
        };
        assert_eq!(problem.why, Refusal::Reference, "{name}");
        assert_eq!(world.records, before, "{name} is refused whole");
    }
}
