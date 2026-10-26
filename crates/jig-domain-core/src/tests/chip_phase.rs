//! Chip facts through public row constructors and the fleet owner entry point.
use super::*;

#[test]
fn chip_owner_facts_keep_readiness_retry_hold_and_ending_clocks_distinct() {
    let (core, limits) = make_tree();
    let mut row = core.tasks.task(2).unwrap().clone();
    row.created_at = Wall::from_nanos(1);
    row.phase_since = Wall::from_nanos(2);
    row.executor = tasks::Executor::Person(tasks::PersonAddress::Role(0));
    row.person_wait = Some(tasks::PersonWait {
        since: Wall::from_nanos(3),
        stall: Duration::from_secs(1),
        deadline: Wall::from_nanos(1_000_000_003),
        notice: None,
    });
    row.phase = tasks::Phase::Active(tasks::Active::Due);
    let chip = chip_from_row(&limits, &row).unwrap();
    assert_eq!(
        chip.phase,
        PhaseLine::Person { address: tasks::PersonAddress::Role(0), taken_by: None, since: Some(Wall::from_nanos(3)) }
    );
    assert_eq!(chip.details.phase_since, Wall::from_nanos(2));
    row.person_wait = None;
    let chip = chip_from_row(&limits, &row).unwrap();
    assert_eq!(chip.phase, PhaseLine::Person { address: tasks::PersonAddress::Role(0), taken_by: None, since: None });
    row.phase = tasks::Phase::Held {
        why: tasks::Hold::Failures(tasks::Class::Lost),
        was: tasks::Was::Active(tasks::Active::BackingOff { until: Wall::from_nanos(50) }),
        source: None,
    };
    row.tries.lost = 2;
    row.retry_notice = Some(tasks::RetryNotice::Failed { class: tasks::Class::Lost, count: 2, allowance: 3 });
    let chip = chip_from_row(&limits, &row).unwrap();
    assert_eq!(chip.details.hold.unwrap().tries.lost, 2);
    assert_eq!(chip.details.retry, row.retry_notice);
    row.phase = tasks::Phase::Ended(tasks::Ending::Cancelled {
        by: tasks::Party::Person(1),
        at: Wall::from_nanos(70),
        reason: Box::from(&b"stopped"[..]),
        result: Some(tasks::TaskResult::Report { words: Box::from(&b"partial"[..]) }),
    });
    row.ended_at = Some(Wall::from_nanos(71));
    let chip = chip_from_row(&limits, &row).unwrap();
    assert_eq!(chip.details.ended_at, row.ended_at);
    let Ending::Cancelled { by, at, result, .. } = chip.details.ending.unwrap() else { panic!("full ending") };
    assert_eq!((by, at), (tasks::Party::Person(1), Wall::from_nanos(70)));
    assert!(result.is_some());
}

#[test]
fn chip_resource_keys_are_whole_and_oversized_keys_refuse_the_document() {
    let (core, mut limits) = make_tree();
    let mut row = core.tasks.task(2).unwrap().clone();
    row.phase = tasks::Phase::Waiting;
    row.holdings = Box::new([tasks::Holding::Write {
        resource: tasks::Name { connector: 1, path: Box::new([Box::from(&b"whole"[..])]) },
        kind: 2,
    }]);
    let chip = chip_from_row(&limits, &row).unwrap();
    let PartKey::Path(path) = &chip.details.resources.get(0).unwrap().part.key else { panic!("literal key") };
    assert_eq!(path.get(0).unwrap().as_ref(), b"whole");
    limits.documents.text_bytes = 4;
    assert_eq!(chip_from_row(&limits, &row), Err(Missing::TooLarge));
    row.phase = tasks::Phase::Ended(tasks::Ending::Done(tasks::TaskResult::Report { words: Box::new([]) }));
    assert!(
        chip_from_row(&limits, &row).unwrap().details.resources.is_empty(),
        "ended chip keeps its ending rather than querying old holdings"
    );
}

#[test]
fn chip_live_observation_uses_receipt_time_and_drops_after_loss() {
    let (mut core, mut limits) = make_tree();
    limits.fleet.engine_slots = 1;
    core.fleet = fleet::Domain::new(&limits.fleet, fixtures::needs(), Box::new([]));
    let mut env = Env { now: Time::ZERO, wall: Wall::from_nanos(20), limits: limits.fleet };
    let mut out = Queue::with_capacity(fleet::max_out(&limits.fleet));
    fleet::step(
        &mut core.fleet,
        &env,
        fleet::Event::Start {
            placement: None,
            version: jig_domain_fleet::Version { charter: 1, revision: 1 },
            grants: Box::new([]),
            reply_to: ReplyTo::new(Token::new(1)),
            run: Token::new(2),
            attempt: Token::new(7),
            workstream: 1,
            kinds: fleet::Kinds::Engine,
            assignment: fleet::Assignment { turns: Token::new(0), answered: Token::new(0) },
        },
        &mut out,
    );
    while out.pop().is_some() {}
    fleet::resume(&mut core.fleet, &env, &mut out);
    while out.pop().is_some() {}
    let mut row = core.tasks.task(2).unwrap().clone();
    row.phase = tasks::Phase::Active(tasks::Active::Running { attempt: 7 });
    row.attempt = 7;
    row.run_since = Some(Wall::from_nanos(10));
    assert!(chip_from_row_with_core(&core, &limits, &row).unwrap().details.observation.is_none());
    fleet::step(
        &mut core.fleet,
        &env,
        fleet::Event::Observe {
            channel: Token::new(0),
            run: Token::new(2),
            attempt: Token::new(7),
            report: fleet::Report::Started,
        },
        &mut out,
    );
    while out.pop().is_some() {}
    env.wall = Wall::from_nanos(30);
    fleet::step(
        &mut core.fleet,
        &env,
        fleet::Event::Observe {
            channel: Token::new(0),
            run: Token::new(2),
            attempt: Token::new(7),
            report: fleet::Report::Activity(fleet::Activity::Waiting),
        },
        &mut out,
    );
    let chip = chip_from_row_with_core(&core, &limits, &row).unwrap();
    assert_eq!(chip.details.run_since, Some(Wall::from_nanos(10)));
    let observation = chip.details.observation.unwrap();
    assert_eq!(observation.work, ChipWork::Waiting);
    assert_eq!(observation.started, Some(Wall::from_nanos(20)));
    assert_eq!(observation.since, Wall::from_nanos(30));
    while out.pop().is_some() {}
    fleet::step(
        &mut core.fleet,
        &env,
        fleet::Event::Observe {
            channel: Token::new(0),
            run: Token::new(2),
            attempt: Token::new(7),
            report: fleet::Report::Activity(fleet::Activity::Tool {
                call: Box::from(&b"call"[..]),
                tool: Box::from(&b"read"[..]),
            }),
        },
        &mut out,
    );
    let mut short = limits;
    short.documents.text_bytes = 3;
    assert_eq!(
        chip_from_row_with_core(&core, &short, &row),
        Err(Missing::TooLarge),
        "opaque live names are never shortened"
    );
    while out.pop().is_some() {}
    fleet::step(&mut core.fleet, &env, fleet::Event::Cancel { run: Token::new(2), attempt: Token::new(7) }, &mut out);
    assert!(chip_from_row_with_core(&core, &limits, &row).unwrap().details.observation.is_none());
    assert!(
        chip_from_row(&limits, &row).unwrap().details.observation.is_none(),
        "stored row never invents volatile observations"
    );
}

#[test]
fn chip_changes_refresh_inline_inbox_and_named_documents() {
    let (mut core, mut limits) = make_tree();
    let (authenticated, auth_limits) = watch_core();
    core.people = authenticated.people;
    limits.people = auth_limits.people;
    finish_watch_outputs(&mut core, &limits);
    let mut numbers = List::with_capacity(1);
    numbers.push(2).unwrap();
    for (watcher, subject) in [(30, Watched::Inbox), (31, Watched::Chips { tasks: numbers })] {
        watch_outputs(
            &mut core,
            &limits,
            Event::Watch { watcher: Token::new(watcher), sign_in: 10, subject, window: window() },
        );
        watch_outputs(&mut core, &limits, Event::Delivered { watcher: Token::new(watcher) });
    }
    watch_outputs(&mut core, &limits, Event::Tasks(tasks::Event::Hold { task: 2, why: tasks::Hold::Budget }));
    let out = finish_watch_outputs(&mut core, &limits);
    let mut watchers = skein_lib::Set::with_capacity(2);
    for request in &out {
        if let Request::Held(held) = request
            && let Held::Delivery { watcher, .. } = held.as_ref()
        {
            assert!(watchers.insert(*watcher).unwrap());
        }
    }
    assert!(watchers.contains(&Token::new(30)), "inbox changed");
    assert!(watchers.contains(&Token::new(31)), "named chip changed");
    assert_eq!(watchers.len(), 2);
}

#[test]
fn chip_closing_names_the_connectors_waited_on_without_counting_their_effects() {
    let (mut core, limits) = make_tree();
    watch_outputs(
        &mut core,
        &limits,
        Event::Tasks(tasks::Event::Control {
            reply_to: ReplyTo::new(Token::new(200)),
            by: tasks::Party::Person(1),
            task: 4,
            control: tasks::Control::Cancel { reason: Box::from(&b"done"[..]), grace: None, message: None },
        }),
    );
    finish_watch_outputs(&mut core, &limits);
    let row = core.tasks.task(4).expect("closing task waits for connectors");
    let chip = chip_from_row_with_core(&core, &limits, row).unwrap();
    let waiting = chip.details.closing_connectors.unwrap();
    assert_eq!(waiting.as_slice(), core.connectors.as_ref());
    assert_eq!(chip.details.live_delegates, 0);
    let first = core.connectors[0];
    watch_outputs(&mut core, &limits, Event::EffectConnector(connector::Event::Closed { task: 4, connector: first }));
    let row = core.tasks.task(4).unwrap();
    let waiting = chip_from_row_with_core(&core, &limits, row).unwrap().details.closing_connectors.unwrap();
    assert_eq!(waiting.len(), 1);
    assert!(!waiting.as_slice().contains(&first));
    let mut short = limits;
    short.documents.lines = 0;
    assert_eq!(chip_from_row_with_core(&core, &short, row), Err(Missing::TooLarge));
}

fn long_operation_work(operation: fleet::Operation) -> ChipWork {
    let (mut core, mut limits) = make_tree();
    limits.fleet.engine_slots = 1;
    core.fleet = fleet::Domain::new(&limits.fleet, fixtures::needs(), Box::new([]));
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.fleet };
    let mut out = Queue::with_capacity(fleet::max_out(&limits.fleet));
    fleet::step(
        &mut core.fleet,
        &env,
        fleet::Event::Start {
            placement: None,
            version: jig_domain_fleet::Version { charter: 1, revision: 1 },
            grants: Box::new([]),
            reply_to: ReplyTo::new(Token::new(1)),
            run: Token::new(2),
            attempt: Token::new(7),
            workstream: 1,
            kinds: fleet::Kinds::Engine,
            assignment: fleet::Assignment { turns: Token::new(0), answered: Token::new(0) },
        },
        &mut out,
    );
    while out.pop().is_some() {}
    fleet::resume(&mut core.fleet, &env, &mut out);
    while out.pop().is_some() {}
    fleet::step(
        &mut core.fleet,
        &env,
        fleet::Event::Observe {
            channel: Token::new(0),
            run: Token::new(2),
            attempt: Token::new(7),
            report: fleet::Report::Activity(fleet::Activity::Long { operation, span: Duration::from_secs(37) }),
        },
        &mut out,
    );
    let mut row = core.tasks.task(2).unwrap().clone();
    row.phase = tasks::Phase::Active(tasks::Active::Running { attempt: 7 });
    row.attempt = 7;
    chip_from_row_with_core(&core, &limits, &row).unwrap().details.observation.unwrap().work
}

fn a_long_completion_shows_the_run_as_writing() {
    assert_eq!(long_operation_work(fleet::Operation::Completion), ChipWork::Writing);
}

fn long_checks_shows_the_run_as_active() {
    assert_eq!(long_operation_work(fleet::Operation::Checks), ChipWork::Active);
}

fn a_long_command_shows_the_run_as_active() {
    assert_eq!(long_operation_work(fleet::Operation::Command), ChipWork::Active);
}

#[test]
fn long_operations_show_the_runs_work() {
    table([
        ("a_long_completion_shows_the_run_as_writing", a_long_completion_shows_the_run_as_writing),
        ("long_checks_shows_the_run_as_active", long_checks_shows_the_run_as_active),
        ("a_long_command_shows_the_run_as_active", a_long_command_shows_the_run_as_active),
    ]);
}

type Case = (&'static str, fn());

fn table<const N: usize>(cases: [Case; N]) {
    let mut failures = [None; N];
    let mut passed = true;
    for ((name, case), failure) in cases.into_iter().zip(&mut failures) {
        if std::panic::catch_unwind(case).is_err() {
            *failure = Some(name);
            passed = false;
        }
    }
    assert!(passed, "{failures:?}");
}
