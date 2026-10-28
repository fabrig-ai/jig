use super::*;

#[test]
fn a_charter_whose_default_spend_misses_its_floor_is_refused() {
    let mut config = config(1);
    config.defaults.charters[0].spend = 1;
    let current = settings::Current::new(config.defaults, &limits());
    assert_eq!(
        settings::check(&current, &config.settings, &config.connectors, &limits()),
        Err(settings::Failure { setting: settings::Setting::Charter(1), fault: settings::Fault::Budget })
    );
}

pub(super) fn context(task: u64, charter: u32) -> Box<tasks::RunContext> {
    Box::new(tasks::RunContext {
        ended_by: tasks::EndedBy::Result,
        added: Box::new([]),
        pending: Box::new([]),
        llm: None,
        payer: tasks::Payer::Party(1),
        task,
        last_message: 0,
        inbox: Box::new([]),
        delegates: Box::new([]),
        dependencies: Box::new([]),
        saved: Box::new([]),
        previous_attempt: 0,
        ever_turned: false,
        tries: tasks::Tries::NONE,
        invalid_result: None,
        project: 1,
        executor: tasks::Executor::Agent { charter },
        spec: tasks::Spec {
            asks: false,
            words: b"task".as_slice().into(),
            parameters: Box::new([]),
            inputs: Box::new([]),
        },
        contract: tasks::Contract::Report { words: 64 },
        requester: tasks::Party::Person(1),
        authority: task_authority(&config(1).seed_projects[0].policy.policy.roles[0].authority),
        numbers: tasks::Numbers { budget: 20, spent: 0, spent_below: 0, reserved: 0 },
    })
}

#[test]
fn take_due_skips_an_unusable_account_and_starts_a_procedure() {
    let (mut core, limits) = core();
    make_task(&mut core, &limits, 1);
    let mut procedure = new_row(&core, 2, tasks::Funder::Period { project: 1, period: 1 }, None);
    procedure.executor = tasks::Executor::Procedure { connector: 1, code: 1 };
    procedure.llm = None;
    fixtures::make_batch(&mut core, &limits, tasks::Party::Person(1), Box::new([procedure]));
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.accounts };
    let mut out = Queue::with_capacity(accounts::max_out(&limits.accounts));
    accounts::step(&mut core.accounts, &env, accounts::Event::Want, &mut out);
    while out.pop().is_some() {}
    accounts::step(&mut core.accounts, &env, accounts::Event::Limited { account: 1, reset: None }, &mut out);
    core.due.push(core.tasks.run_context(1).unwrap());
    core.due.push(core.tasks.run_context(2).unwrap());
    assert!(core.due_ready());
    assert_eq!(core.take_due().expect("procedure ready").task, 2);
    assert!(!core.due_ready());
    assert_eq!(core.due.pop().expect("account wait retains order").task, 1);
}

#[test]
fn startup_needs_own_models_and_preserve_charter_requirements() {
    let mut config = config(1);
    let mut limits = limits();
    limits.tasks.charters = 2;
    limits.run_bytes = 1024;
    let mut alternative = scripted_model();
    alternative.name = b"alternative".as_slice().into();
    alternative.window = 4096;
    alternative.output = 512;
    config.settings.models = Box::new([scripted_model(), alternative.clone()]);
    config.defaults.charters[0].policy.alternatives = Box::new([alternative]);
    let mut engine = config.defaults.charters[0].clone();
    engine.number = 73;
    engine.hosts = fleet::Kinds::Engine;
    engine.policy.modify = false;
    config.defaults.charters = Box::new([config.defaults.charters[0].clone(), engine]);
    crate::charters::assert_catalogue(&config.settings);

    let needs = fleet_needs::configured(&config, &limits);
    assert_eq!(needs.spend, config.defaults.rules.maximum_run_spend);
    assert_eq!(needs.message_bytes, limits.tasks.message_bytes);
    assert_eq!(needs.start_messages, limits.tasks.run_inbox);
    assert_eq!(needs.unacknowledged, config.hub_turns);
    drop(config);

    assert_eq!(needs.charters.len(), 2);
    let worker = &needs.charters[0];
    assert_eq!(worker.version, fleet::Version { charter: 1, revision: 1 });
    assert_eq!(worker.kinds, fleet::Kinds::Workers);
    assert_eq!(worker.turns, 8);
    assert_eq!(worker.time, Duration::from_secs(60));
    assert_eq!(worker.waiting, Duration::from_secs(1));
    assert_eq!(worker.change_reserve, Some(Duration::from_secs(10)));
    assert_eq!(
        worker.models.as_ref(),
        &[
            fleet::ModelNeed { name: b"scripted".as_slice().into(), window: 8192, output: 1024 },
            fleet::ModelNeed { name: b"alternative".as_slice().into(), window: 4096, output: 512 },
        ]
    );
    let engine = &needs.charters[1];
    assert_eq!(engine.version, fleet::Version { charter: 73, revision: 1 });
    assert_eq!(engine.kinds, fleet::Kinds::Engine);
    assert_eq!(engine.change_reserve, None);
}
