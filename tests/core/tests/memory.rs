use jig_core_world::world;
use jig_domain_accounts as accounts;
use jig_domain_core as core;
use jig_domain_fleet as fleet;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_lib::{Duration, Env, Time, Wall};
use skein_world::domain::heap::{self, Meter};

#[global_allocator]
static HEAP: heap::Counting = heap::Counting;
// The core world's existing fixed scenario ceiling.
const CORE_HEAP_CEILING: u64 = 12_000_000;

fn core_children_routes_and_an_in_flight_decision_stay_under_the_scenario_ceiling() {
    let limits = world::limits();
    let bound = CORE_HEAP_CEILING;
    let configuration = world::config(91);
    let mut observer = jig_core_world::observations::Observer::new(&configuration);
    let config = configuration.core;
    let meter = Meter::new();
    meter.start();
    let mut domain = core::Core::new(config, &limits.core).expect("fixture core configuration");
    let measured = meter.end();
    meter.check(measured, bound, &limits.core);

    domain.restart_begin(false);
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.core };
    for event in [
        core::Event::People(people::Event::Restored),
        core::Event::Tasks(tasks::Event::Check),
        core::Event::Tasks(tasks::Event::Restored),
        core::Event::Fleet(fleet::Event::Loaded),
        core::Event::Account(accounts::Event::Component(accounts::FromComponent::Granted {
            form: accounts::Form::Credential(accounts::Scheme::Bearer),
            account: 1,
            generation: 1,
            valid: Duration::from_secs(60),
        })),
    ] {
        meter.start();
        let requests = core::step(&mut domain, &env, event);
        let measured = meter.end();
        drop(requests);
        let held = meter.check(measured, bound, &limits.core);
        observer.observe(0, jig_core_world::referee::Observed::Heap { held, maximum: bound });
    }
    observer.observe(0, jig_core_world::referee::Observed::Heap { held: meter.held(), maximum: bound });
}

fn invalid_limits_are_refused() {
    let limits = world::limits().core;
    assert!(core::valid(&core::Limits { connectors: 0, ..limits }));
    assert!(!core::valid(&core::Limits { resume_bytes: 0, ..limits }));
    assert!(!core::valid(&core::Limits { call_records: 0, ..limits }));
    assert!(!core::valid(&core::Limits { tasks: tasks::Limits { tasks: u32::MAX, ..limits.tasks }, ..limits }));
}

fn both_worlds_check_the_counted_heap_at_every_iteration() {
    let mut walking = world::World::goal(190);
    walking.run_goal();
    drop(walking);
    let mut effects = jig_core_world::effects::World::new(191, false);
    effects.delegate();
    effects.call(2, 9000);
    effects.restart(191, false);
}

// Each named scenario preserves its own fixture and original assertions.
#[test]
fn memory_scenario_family() {
    core_children_routes_and_an_in_flight_decision_stay_under_the_scenario_ceiling();
    invalid_limits_are_refused();
    both_worlds_check_the_counted_heap_at_every_iteration();
}

#[test]
fn settled_finishing_queries_allocate_nothing_and_empty_closure_holds_one_output() {
    let limits = world::limits().core;
    let meter = Meter::new();
    meter.start();
    let mut domain = core::Core::new(world::config(192).core, &limits).expect("fixture core");
    domain.counters = core::Counters::new(domain.counters.deployment());
    let setup = meter.end();
    let bound = CORE_HEAP_CEILING;
    meter.check(setup, bound, &limits);
    let held = meter.held();
    meter.start();
    for _ in 0..100 {
        assert!(!domain.finish_owed());
    }
    let queries = meter.end();
    assert_eq!((queries.held(), queries.peak()), (held, held));
    meter.check(queries, bound, &limits);
    meter.start();
    let core::Requests::Out(mut out) =
        core::finish_decision(&mut domain, &Env { now: Time::ZERO, wall: Wall::EPOCH, limits });
    let closed = meter.end();
    assert!(matches!(out.pop(), Some(core::Request::Decided)));
    assert!(out.is_empty());
    let one_output = skein_lib::Queue::<core::Request>::worst_case(1).expect("one output fits");
    assert!(closed.peak() - held <= one_output, "closure allocates at most one output's room");
    drop(out);
    meter.check(closed, bound, &limits);
    meter.start();
    let mut quiet = skein_lib::Queue::with_capacity(0);
    jig_domain_views::step(
        &mut domain.views,
        &Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.views },
        jig_domain_views::Event::Text {
            run: skein_lib::Token::new(41),
            turn: jig_domain_views::TurnName { attempt: 1, turn: 1, retry: 0 },
            offset: 0,
            text: Box::new([b'x']),
            call: None,
        },
        &mut quiet,
    );
    let retained = meter.end();
    assert!(quiet.is_empty());
    meter.check(retained, bound, &limits);
    let held = meter.held();
    meter.start();
    for _ in 0..100 {
        assert!(domain.finish_owed());
    }
    let queries = meter.end();
    assert_eq!((queries.held(), queries.peak()), (held, held), "retained metadata inspection also allocates nothing");
    meter.check(queries, bound, &limits);
}

#[test]
fn populated_projection_and_call_owner_routes_restore_under_the_counted_heap_bound() {
    use jig_test_connector as connector;
    use jig_test_domain as root;
    let (mut configuration, limits) = jig_core_world::effects::fixture(193, false);
    let mut policy = configuration.core.seed_projects[0].policy.policy.clone();
    policy.projections.clone_from(&policy.ceiling.grants);
    configuration.core.seed_projects[0].policy.policy = policy.clone();
    let mut restart = jig_core_world::effects::fixture(193, false).0;
    restart.core.seed_projects[0].policy.policy = policy;
    let mut world = jig_core_world::effects::World::configured(193, false, configuration, limits);
    world.hold_writes = true;
    world.call(1, 230);
    let request = jig_core_world::fixtures::goal(
        &world,
        1,
        b"routing owners".as_slice().into(),
        people::TaskExecutor::Agent { charter: 1 },
        jig_core_world::fixtures::Goal { budget: 20, priority: 1, ..Default::default() },
    );
    jig_core_world::stories::reads::request(&mut world, 94, request);
    let goal = world
        .people_answers
        .iter()
        .rev()
        .find_map(|reply| match reply {
            people::Reply::Outcome(people::Outcome::Created { first, .. }) => Some(*first),
            people::Reply::Signer(_)
            | people::Reply::Full { .. }
            | people::Reply::SignedIn { .. }
            | people::Reply::SignedOut
            | people::Reply::Outcome(_)
            | people::Reply::Refused(_)
            | people::Reply::Again(_) => None,
        })
        .expect("tracked goal admitted alongside the caller");
    world.hold_projection_settlements = true;
    world.send(root::Event::ProjectionEffect { goal, number: 1, effect: jig_core_world::effects::World::effect() });
    let pending = std::mem::take(&mut world.pending_writes);
    assert_eq!(
        world
            .store
            .rows
            .values()
            .filter(|row| matches!(row, root::Record::Connector { record: connector::Record::Outbox(_), .. }))
            .count(),
        2
    );
    world.restart_with(restart);
    assert_eq!(
        world
            .store
            .rows
            .values()
            .filter(|row| matches!(row, root::Record::Connector { record: connector::Record::Outbox(_), .. }))
            .count(),
        2
    );
    for (number, call) in pending {
        let event = world.systems[usize::from(number - 1)].answer(call, jig_test_system::Fault::None);
        world.send(root::Event::Connector { number, event: connector::Event::System(event) });
    }
    world.retry_due();
    world.release_writes();
    assert!(
        !world
            .store
            .rows
            .values()
            .any(|row| matches!(row, root::Record::Connector { record: connector::Record::Outbox(_), .. }))
    );
}
