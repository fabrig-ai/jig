//! Admitted records and split facts at the watermark (programming-model.md, 7.1–7.2).
use skein_world::domain::heap;

#[global_allocator]
static HEAP: heap::Counting = heap::Counting;

use jig_core_world::direct::Driver;
use jig_domain_core as core;
use jig_domain_host as host;
use jig_domain_people::ApplicationScope;
use jig_domain_tasks as tasks;
use jig_test_connector as connector;
use jig_test_domain as root;
use skein_lib::{Duration, Token};

const WORKER: Token = Token::new(7);
const WATCHER: Token = Token::new(980);
const TURN: u32 = 1;
const SPENT: u64 = 3;
const BODY: &[u8] = b"kept turn";
const RESULT: &[u8] = b"finished work";
const OLD_STATE: u64 = 10;
const CHANGED_STATE: u64 = 11;
// One turn, one terminal, one party result, and one lost writer per story.
const ONCE: usize = 1;

#[test]
fn admitted_work_is_taken_at_the_watermark() {
    let mut driver = Driver::chat(9401);
    let (task, attempt) = run(&driver);
    driver.fill_to_watermark();
    let returned = driver.returned;
    for event in [
        root::Event::Text { task, attempt, turn: TURN, offset: 0, text: BODY.into() },
        turn(task, attempt),
        answer(task, attempt),
        root::Event::Link {
            channel: WORKER,
            link: root::link::Event::Up(host::LinkUp::Logins { logins: Box::new([]) }),
        },
    ] {
        driver.send(event);
    }
    assert_eq!(driver.returned, returned, "admitted worker records enter at the watermark");
    driver.settle();
    let writes: Vec<_> = driver.transactions.iter().flatten().collect();
    assert_eq!(
        writes
            .iter()
            .filter(|write| matches!(write,
        root::Write::Save(root::Record::Core(core::Record::Core(core::CoreRecord::Turn(row))))
        if row.task == task && row.attempt == attempt && row.turn == TURN))
            .count(),
        ONCE
    );
    assert_eq!(
        writes
            .iter()
            .filter(|write| matches!(write,
        root::Write::Save(root::Record::Core(core::Record::Core(core::CoreRecord::Terminal(row))))
        if row.task == task && row.attempt == attempt))
            .count(),
        ONCE
    );
    assert_eq!(
        driver
            .released
            .iter()
            .filter(|request| matches!(request,
        root::Request::Deliver(root::Delivery::Core(core::Held::Result { task: named, words, .. }))
        if *named == task && words.as_ref() == RESULT))
            .count(),
        ONCE
    );
    assert_eq!(worker_notices(&driver), [Some(TURN), None], "turn kept precedes terminal acknowledgement");
}

#[test]
fn a_workers_call_waits_at_the_watermark() {
    let mut driver = Driver::chat(9402);
    let (task, attempt) = run(&driver);
    driver.fill_to_watermark();
    let event = root::try_step(&mut driver.domain, &driver.env, call(task, attempt)).expect_err("a call is new work");
    let root::Event::Link {
        channel,
        link: root::link::Event::Up(host::LinkUp::Call { run, attempt: retained, call, tool, writes, input, deadline }),
    } = &event
    else {
        panic!("the worker's call returns unchanged")
    };
    assert_eq!((*channel, run.raw(), retained.raw()), (WORKER, task, attempt));
    assert_eq!(
        (call.as_ref(), tool.as_ref(), *writes, input.as_ref(), *deadline),
        (b"held call".as_slice(), b"read".as_slice(), false, b"{}".as_slice(), Duration::from_secs(1))
    );
    driver.settle();
    root::try_step(&mut driver.domain, &driver.env, event).expect("the same call enters after settlement");
}

#[test]
fn a_fact_settles_under_load_and_its_drift_waits() {
    let mut driver = watched_owner(9403);
    driver.fill_to_watermark();
    let event = root::Event::Connector { number: 1, event: connector::Event::System(changed_fact(&driver)) };
    let rest = root::try_step(&mut driver.domain, &driver.env, event).expect_err("drift waits");
    assert!(
        !matches!(
            &rest,
            root::Event::Connector { event: connector::Event::System(connector::SystemEvent::Fact { .. }), .. }
        ),
        "only the new drift returns; the fact has been settled"
    );
    check_settlement_and_drift(&mut driver, rest);
}

#[test]
fn a_lost_writers_read_completes_under_load_and_its_drift_waits() {
    let (mut driver, writer) = Driver::lost_resource_writer(9404);
    let (task, _) = run(&driver);
    let resource = resource_name();
    driver.fill_to_watermark();
    let event = root::Event::WriterRead { connector: 1, resource, event: changed_fact(&driver) };
    let rest = root::try_step(&mut driver.domain, &driver.env, event).expect_err("drift waits");
    driver.collect();
    assert_eq!(
        driver
            .transactions
            .iter()
            .flatten()
            .filter(|write| matches!(write,
        root::Write::Erase(root::Key::Core(core::Key::Tasks(tasks::Key::Writer(number)))) if *number == writer))
            .count(),
        ONCE,
        "the fresh read frees the lost writer under load"
    );
    assert!(matches!(&rest, root::Event::Connector { number: 1, .. }), "only drift remains, not a writer read");
    driver.settle();
    assert_eq!(drift_writes(&driver, task), 0, "the read settles before its new drift");
    driver.send(rest);
    driver.settle();
    assert_eq!(drift_writes(&driver, task), ONCE, "the resource owner drifts once after admission");
}

fn drift_writes(driver: &Driver, task: u64) -> usize {
    driver
        .transactions
        .iter()
        .filter(|transaction| {
            transaction.iter().any(|write| {
                matches!(write,
        root::Write::Save(root::Record::Core(core::Record::Tasks(tasks::Stored::Live(row))))
        if row.number == task && matches!(row.phase, tasks::Phase::Held { why: tasks::Hold::Drift, .. }))
            })
        })
        .count()
}

fn watched_owner(seed: u64) -> Driver {
    let mut driver = Driver::resource_owner(seed);
    let task = driver.assignment().task;
    driver.send(root::Event::WatchApplication {
        watcher: WATCHER,
        sign_in: driver.session(),
        scope: ApplicationScope::Project(1),
        number: 1,
        subject: connector::ClientSubject::Hold { project: 1, task },
        window: core::documents::Window { rows: driver.env.limits.core.documents.rows, after: None },
    });
    driver.settle();
    driver.send(root::Event::Core(core::Event::Delivered { watcher: WATCHER }));
    driver.settle();
    driver
}

fn check_settlement_and_drift(driver: &mut Driver, rest: root::Event) {
    let (task, _) = run(driver);
    driver.collect();
    assert_eq!(facts(driver), [OLD_STATE], "refreshed document waits for preceding durability");
    driver.settle();
    assert_eq!(facts(driver), [OLD_STATE, CHANGED_STATE], "the admitted fact refreshes its watched subject");
    assert!(driver.domain.escalation(1, task).is_none(), "drift waits with the new work");
    driver.send(rest);
    driver.settle();
    assert_eq!(
        driver.domain.escalation(1, task).expect("the changed resource holds its owner").why,
        tasks::Hold::Drift
    );
    assert_eq!(
        driver
            .released
            .iter()
            .filter(|request| matches!(request,
        root::Request::Deliver(root::Delivery::Core(core::Held::Cancel { channel: WORKER, run, .. }))
        if run.raw() == task))
            .count(),
        ONCE,
        "drift fences its live owner once"
    );
}

fn facts(driver: &Driver) -> Vec<u64> {
    driver
        .released
        .iter()
        .filter_map(|request| {
            if let root::Request::Deliver(root::Delivery::ApplicationDocument { watcher: WATCHER, document, .. }) =
                request
                && let connector::ClientDocument::Hold { resources, .. } = document.as_ref()
            {
                return resources.iter().find_map(|resource| resource.fact.and_then(|fact| fact.state));
            }
            None
        })
        .collect()
}

fn resource_name() -> tasks::Name {
    tasks::Name { connector: 1, path: jig_test_connector_world::path(1, 1).segments().into() }
}

fn changed_fact(driver: &Driver) -> connector::SystemEvent {
    connector::SystemEvent::Fact {
        resource: jig_test_connector_world::path(1, 1),
        fact: connector::Fact { state: Some(CHANGED_STATE), observed: driver.env.wall, pending: false },
        origin: connector::Origin::Other,
    }
}

fn run(driver: &Driver) -> (u64, u64) {
    let assignment = driver.assignment();
    (assignment.task, assignment.attempt)
}

fn turn(task: u64, attempt: u64) -> root::Event {
    root::Event::Turn {
        channel: WORKER,
        task,
        attempt,
        turn: TURN,
        cumulative: SPENT,
        read: None,
        opens_window: true,
        refused: Box::new([]),
        transcript: BODY.into(),
    }
}

fn answer(task: u64, attempt: u64) -> root::Event {
    root::Event::Answer {
        channel: WORKER,
        task,
        attempt,
        cumulative: SPENT,
        read: None,
        refused: Box::new([]),
        end: tasks::End::Finished {
            result: tasks::TaskResult::Report { words: RESULT.into() },
            cancel_delegates: false,
        },
    }
}

fn call(task: u64, attempt: u64) -> root::Event {
    root::Event::Link {
        channel: WORKER,
        link: root::link::Event::Up(host::LinkUp::Call {
            run: Token::new(task),
            attempt: Token::new(attempt),
            call: b"held call".as_slice().into(),
            tool: b"read".as_slice().into(),
            writes: false,
            input: b"{}".as_slice().into(),
            deadline: Duration::from_secs(1),
        }),
    }
}

fn worker_notices(driver: &Driver) -> Vec<Option<u32>> {
    driver
        .released
        .iter()
        .filter_map(|request| match request {
            root::Request::Deliver(root::Delivery::Core(core::Held::AcknowledgeTurn {
                channel: WORKER, turn, ..
            })) => Some(Some(*turn)),
            root::Request::Deliver(root::Delivery::Core(core::Held::Acknowledge { channel: WORKER, .. })) => Some(None),
            _ => None,
        })
        .collect()
}
