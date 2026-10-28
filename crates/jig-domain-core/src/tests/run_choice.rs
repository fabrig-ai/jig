//! The run's data check is pure; the core world only tests routing.
#![allow(
    clippy::disallowed_types,
    clippy::disallowed_macros,
    clippy::disallowed_methods,
    reason = "small input tables are ordinary Rust tests"
)]
use super::*;
const TASK: u64 = 1;
const FIRST: Token = Token::new(7);
const SECOND: Token = Token::new(11);

fn account(core: &mut Core, limits: &Limits, event: accounts::Event) {
    let mut out = Queue::with_capacity(accounts::max_out(&limits.accounts));
    accounts::step(
        &mut core.accounts,
        &Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.accounts },
        event,
        &mut out,
    );
}
fn granted(core: &mut Core, limits: &Limits, source: u32) {
    account(
        core,
        limits,
        accounts::Event::Component(accounts::FromComponent::Granted {
            form: accounts::Form::Credential(accounts::Scheme::Bearer),
            account: source,
            generation: 1,
            valid: Duration::from_secs(60),
        }),
    );
}
pub(super) fn host(core: &mut Core, limits: &Limits, channel: Token, names: &[&[u8]]) {
    host_with_logins(core, limits, channel, names, Box::new([]));
}
fn host_with_logins(core: &mut Core, limits: &Limits, channel: Token, names: &[&[u8]], logins: Box<[fleet::Login]>) {
    host_with_endpoints(core, limits, channel, names, logins, Box::new([]));
}
fn host_with_endpoints(
    core: &mut Core,
    limits: &Limits,
    channel: Token,
    names: &[&[u8]],
    logins: Box<[fleet::Login]>,
    endpoints: Box<[u32]>,
) {
    let mut bounds = fixtures::bounds();
    bounds.program_endpoints = endpoints;
    bounds.models =
        names.iter().map(|name| fleet::ModelBound { name: (*name).into(), window: 8192, output: 1024 }).collect();
    let mut out = Queue::with_capacity(fleet::max_out(&limits.fleet));
    fleet::step(
        &mut core.fleet,
        &Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.fleet },
        fleet::Event::Hello {
            worker: u32::from(channel == SECOND),
            channel,
            hello: fleet::Hello {
                bounds,
                stop_bound: Duration::ZERO,
                slots: 1,
                workstreams: Box::new([]),
                hosting: Box::new([]),
            },
        },
        &mut out,
    );
    fleet::step(
        &mut core.fleet,
        &Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.fleet },
        fleet::Event::Logins { channel, logins },
        &mut out,
    );
}
fn ready(none: bool) -> (Core, Limits) {
    let (base, mut limits) = core();
    limits.accounts.accounts = 4;
    limits.accounts.holder_accounts = 4;
    limits.accounts.credentials = 3;
    limits.fleet.workers = 2;
    limits.run_bytes = 4096;
    let mut config = config(41);
    let mut same = scripted_model();
    same.name = b"same".as_slice().into();
    let mut other = scripted_model();
    other.name = b"other".as_slice().into();
    config.settings.models = Box::new([scripted_model(), same.clone(), other.clone()]);
    config.defaults.charters[0].policy.alternatives = Box::new([same, other]);
    config.settings.llm_providers.providers[0].models =
        Box::new([b"scripted".as_slice().into(), b"same".as_slice().into()]);
    let provider = accounts::Provider {
        name: b"Fake".as_slice().into(),
        signed_in_plan: !none,
        manage_usage: Some(b"https://usage.example/".as_slice().into()),
        number: 2,
        models: Box::new([b"other".as_slice().into()]),
        offers: accounts::Offers { sign_in: !none, key: false, programs: Box::new([]) },
    };
    config.settings.llm_providers.providers = Box::new([config.settings.llm_providers.providers[0].clone(), provider]);
    let mut core = seeded_core(config, &limits).expect("fixture core configuration");
    core.people = base.people;
    restore_key_account(&mut core, &limits, 0, 1, 1);
    if !none {
        restore_key_account(&mut core, &limits, 3, 2, 2);
    }
    make_task(&mut core, &limits, TASK);
    account(&mut core, &limits, accounts::Event::Want);
    granted(&mut core, &limits, 1);
    host(&mut core, &limits, FIRST, &[b"scripted", b"same"]);
    host(&mut core, &limits, SECOND, &[b"scripted", b"same", b"other"]);
    (core, limits)
}

#[test]
fn a_lower_numbered_goals_waiting_run_starts_first() {
    // One occupied host slot is freed for the two waiting runs.
    const SLOTS: u32 = 1;
    const OLDER: u64 = 2;
    const NEWER: u64 = 3;
    const BUSY_ATTEMPT: Token = Token::new(9);
    const EARLIER: Wall = Wall::from_nanos(1);
    const LATER: Wall = Wall::from_nanos(2);
    for (older_priority, newer_priority, newer_since, queued, expected) in [
        (Some(9), Some(1), LATER, [OLDER, NEWER], NEWER),
        (None, Some(0), LATER, [OLDER, NEWER], NEWER),
        (Some(0), None, LATER, [NEWER, OLDER], OLDER),
        (Some(1), Some(1), LATER, [NEWER, OLDER], OLDER),
        (Some(1), Some(1), EARLIER, [NEWER, OLDER], OLDER),
    ] {
        let (mut core, limits) = tree_core();
        for (task, priority, since) in [(OLDER, older_priority, EARLIER), (NEWER, newer_priority, newer_since)] {
            let row = new_row(&core, task, tasks::Funder::Period { project: 1, period: 1 }, priority);
            fixtures::make_batch_at(&mut core, &limits, tasks::Party::Person(1), Box::new([row]), since);
        }
        account(&mut core, &limits, accounts::Event::Want);
        granted(&mut core, &limits, 1);
        let environment = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.fleet };
        let mut out = Queue::with_capacity(fleet::max_out(&limits.fleet));
        fleet::step(
            &mut core.fleet,
            &environment,
            fleet::Event::Hello {
                worker: 0,
                channel: FIRST,
                hello: fleet::Hello {
                    bounds: fixtures::bounds(),
                    stop_bound: Duration::ZERO,
                    slots: SLOTS,
                    workstreams: Box::new([]),
                    hosting: Box::new([fleet::Hosted {
                        run: Token::new(TASK),
                        attempt: BUSY_ATTEMPT,
                        phase: fleet::Phase::Active,
                    }]),
                },
            },
            &mut out,
        );
        while out.pop().is_some() {}
        fleet::step(
            &mut core.fleet,
            &environment,
            fleet::Event::Logins { channel: FIRST, logins: Box::new([]) },
            &mut out,
        );
        while out.pop().is_some() {}
        fleet::step(
            &mut core.fleet,
            &environment,
            fleet::Event::Adopt {
                reply_to: ReplyTo::new(Token::new(1)),
                run: Token::new(TASK),
                attempt: BUSY_ATTEMPT,
                kept: 0,
                kind: fleet::HostKind::Worker,
                worked: false,
                holds: Box::new([]),
                grants: Box::new([]),
            },
            &mut out,
        );
        while out.pop().is_some() {}
        // Queue in reverse task order too, so equal waits exercise the task
        // number tie-break rather than succeeding merely because of FIFO.
        for task in queued {
            core.due.push(core.tasks.run_context(task).expect("waiting run"));
        }
        let blocked = core.take_due();
        fleet::step(
            &mut core.fleet,
            &environment,
            fleet::Event::Answer {
                channel: FIRST,
                run: Token::new(TASK),
                attempt: BUSY_ATTEMPT,
                answer: fleet::Answer::Ended,
                payload: Token::new(1),
            },
            &mut out,
        );
        while out.pop().is_some() {}
        fleet::step(
            &mut core.fleet,
            &environment,
            fleet::Event::Acknowledge { run: Token::new(TASK), attempt: BUSY_ATTEMPT },
            &mut out,
        );
        let started = core.take_due().expect("the freed slot starts a waiting run");
        assert!(blocked.is_none(), "both runs wait while the one slot is occupied");
        assert_eq!(started.task, expected, "priorities {older_priority:?}, {newer_priority:?}");
    }
}

#[test]
fn optional_unavailable_provider_is_omitted_and_same_provider_shares_one_source() {
    let (core, _) = ready(false);
    let choice = core.run_choice(TASK).unwrap();
    assert_eq!(choice.placement.channel, FIRST);
    assert_eq!(choice.accounts.as_ref(), [authority::AccountBinding { provider: 1, account: 0, source: 1 }]);
    assert_eq!(choice.policy.alternatives.iter().map(|model| model.name.as_ref()).collect::<Vec<_>>(), [b"same"]);
}

#[test]
fn usable_retained_provider_constrains_the_concrete_host() {
    let (mut core, limits) = ready(false);
    granted(&mut core, &limits, 2);
    let mut stale = core.tasks.run_context(TASK).unwrap();
    stale.payer = tasks::Payer::Party(999);
    stale.numbers.budget = 1;
    let RunAdmission::Allow(allowed) = core.run_admission(&stale, Wall::EPOCH, Box::new([])) else {
        panic!("current data allows")
    };
    assert_eq!(allowed.spend, 100);
    assert_eq!(allowed.choice.placement.channel, SECOND);
    assert_eq!(
        allowed.choice.accounts.as_ref(),
        [
            authority::AccountBinding { provider: 1, account: 0, source: 1 },
            authority::AccountBinding { provider: 2, account: 3, source: 2 },
        ]
    );
}

#[test]
fn a_none_provider_is_retained_without_a_credential_grant() {
    let (core, _) = ready(true);
    let choice = core.run_choice(TASK).unwrap();
    assert_eq!(choice.placement.channel, SECOND);
    assert_eq!(
        choice.accounts.as_ref(),
        [
            authority::AccountBinding { provider: 1, account: 0, source: 1 },
            authority::AccountBinding { provider: 2, account: 2, source: 3 },
        ]
    );
    assert_eq!(core.accounts.source_grant(3, Time::ZERO), None);
}

#[test]
fn an_unknown_reset_wait_has_no_readiness_spin_after_a_long_advance() {
    let (mut core, limits) = ready(false);
    account(&mut core, &limits, accounts::Event::Limited { account: 1, reset: None });
    core.run_now = Time::ZERO.saturating_add(Duration::from_secs(1_000_000));
    core.due.push(core.tasks.run_context(TASK).unwrap());
    assert!(!core.due_ready());
    assert_eq!(
        core.run_choice(TASK),
        Err(RunWait::Account {
            account: Some(0),
            source: Some(1),
            why: accounts::WaitWhy::Source(accounts::State::Limited { until: None })
        })
    );
}

#[test]
fn loaded_bindings_keep_the_committed_associations() {
    let (mut core, limits) = ready(false);
    let mut cases = Vec::new();
    let binding = authority::AccountBinding { provider: 1, account: 0, source: 1 };
    cases.push((vec![binding], true));
    cases.push((vec![binding, binding], true));
    cases.push((vec![authority::AccountBinding { source: 2, ..binding }], true));
    cases.push((vec![authority::AccountBinding { provider: 2, ..binding }], true));
    cases.push((vec![authority::AccountBinding { source: 99, ..binding }], true));
    cases.push((vec![], true));
    for (accounts, accepted) in cases {
        let mut proof = RunProof::claimed(TASK, 7, 7, None, fleet::HostKind::Worker);
        proof.placement = Some(FIRST);
        proof.accounts = accounts.into();
        core.restoring_proofs
            .insert(TASK, RestoringProof { attempt: 7, turn: 0, run_spent: 0, last_answer: None })
            .unwrap();
        assert_eq!(core.restore_proof(proof.clone(), limits.tasks.result_bytes), accepted);
        assert_eq!(core.proofs.get(&TASK), Some(&proof));
        core.proofs.remove(&TASK);
    }
}

#[test]
fn adoption_uses_recorded_sources_through_refresh_and_removal() {
    let (mut core, limits) = ready(false);
    granted(&mut core, &limits, 2);
    let mut proof = RunProof::claimed(TASK, 7, 7, None, fleet::HostKind::Worker);
    proof.placement = Some(FIRST);
    proof.accounts = Box::new([authority::AccountBinding { provider: 2, account: 3, source: 2 }]);
    core.proofs.insert(TASK, proof).unwrap();
    for usable in [true, false] {
        if !usable {
            account(&mut core, &limits, accounts::Event::Limited { account: 2, reset: None });
        }
        core.adopt(&Env { now: Time::ZERO, wall: Wall::EPOCH, limits }, TASK, 7, 0);
        let fleet::Event::Adopt { holds, grants, .. } = core.adopted.pop().unwrap() else {
            panic!("recorded adoption")
        };
        assert_eq!(holds.as_ref(), [fleet::GrantName { table: fleet::GrantTable::Accounts, account: 2 }]);
        assert_eq!(grants.len(), usize::from(usable));
        if let Some(grant) = grants.first() {
            assert_eq!(grant.account, 2);
        }
        core.unreported_restored.remove(&TASK);
    }
    core.proofs.get_mut(&TASK).unwrap().accounts[0] =
        authority::AccountBinding { provider: 1, account: 99, source: 99 };
    core.adopt(&Env { now: Time::ZERO, wall: Wall::EPOCH, limits }, TASK, 7, 0);
    let fleet::Event::Adopt { grants, .. } = core.adopted.pop().unwrap() else { panic!("removed source history") };
    assert!(grants.is_empty(), "the usable source 1 cannot replace removed source 99");
}

#[test]
fn a_default_chooses_a_usable_provider_once_with_its_first_effort() {
    let (mut core, limits) = ready(false);
    granted(&mut core, &limits, 2);
    account(&mut core, &limits, accounts::Event::Limited { account: 1, reset: None });
    let mut batch = [new_row(&core, 2, tasks::Funder::Period { project: 1, period: 1 }, None)];
    core.resolve_llm(&mut batch).unwrap();
    assert_eq!(batch[0].llm.as_ref().unwrap().main, tasks::ModelEffort { model: 2, effort: Some(0) });
    fixtures::make_batch(&mut core, &limits, tasks::Party::Person(1), Box::new(batch));
    account(&mut core, &limits, accounts::Event::Limited { account: 2, reset: None });
    assert_eq!(core.chosen_policy(2).model.name.as_ref(), b"other");
}

fn terminal_after_zero_reset(account_terminal: bool) -> tasks::Tries {
    let (mut core, limits) = ready(false);
    account(&mut core, &limits, accounts::Event::Limited { account: 1, reset: Some(Time::ZERO) });
    send_task(&mut core, &limits, tasks::Event::Prepare { reply_to: ReplyTo::new(FIRST), task: TASK });
    send_task(
        &mut core,
        &limits,
        tasks::Event::Claim {
            reply_to: ReplyTo::new(FIRST),
            task: TASK,
            attempt: 7,
            resumed: false,
            budget: 10,
            account: Some(0),
            writes: Box::new([]),
        },
    );
    let mut proof = RunProof::claimed(TASK, 7, 7, None, fleet::HostKind::Worker);
    proof.placement = Some(FIRST);
    proof.accounts = Box::new([authority::AccountBinding { provider: 1, account: 0, source: 1 }]);
    proof.account_failure = Some(1);
    proof.account_terminal = account_terminal;
    core.proofs.insert(TASK, proof).unwrap();
    let _out = step(
        &mut core,
        &Env { now: Time::ZERO, wall: Wall::EPOCH, limits },
        Event::AnswerPayload {
            run: Token::new(TASK),
            attempt: Token::new(7),
            payload: Token::new(99),
            cumulative: 1,
            end: tasks::End::Failed(tasks::Class::Run),
            saved: None,
            invalid_saved: false,
            read: None,
            refused: Box::new([]),
        },
    );
    core.tasks.task(TASK).unwrap().tries
}

#[test]
fn a_terminal_limit_with_a_zero_reset_spares_tries() {
    assert_eq!(terminal_after_zero_reset(true), tasks::Tries::NONE);
}

#[test]
fn a_recovered_notice_preserves_an_ordinary_failure() {
    assert_eq!(terminal_after_zero_reset(false).run, 1);
}

#[test]
fn a_retained_due_context_is_refreshed_before_preparation() {
    let (mut core, _) = ready(false);
    let mut stale = core.tasks.run_context(TASK).unwrap();
    stale.payer = tasks::Payer::Party(999);
    stale.numbers.budget = 1;
    core.due.push(stale);
    let current = core.take_due().unwrap();
    assert_eq!(current.payer, tasks::Payer::Project(1));
    assert_eq!(current.numbers.budget, 100);
}

#[test]
fn a_held_tasks_due_context_leaves_the_queue() {
    let (mut core, limits) = ready(false);
    core.due.push(core.tasks.run_context(TASK).unwrap());
    send_task(&mut core, &limits, tasks::Event::Hold { task: TASK, why: tasks::Hold::Uncertain { entry: 1 } });
    assert!(core.take_due().is_none());
    assert!(core.due.is_empty());
}

#[test]
fn a_task_document_preserves_an_unknown_account_reset() {
    let (mut core, limits) = ready(false);
    account(&mut core, &limits, accounts::Event::Limited { account: 1, reset: None });
    let row = core.tasks.task(TASK).unwrap();
    let document = task_from_row(&core, &limits, 1, row, Wall::EPOCH).unwrap();
    assert_eq!(
        document.run_wait,
        Some(TaskRunWait::Account {
            account: Some(0),
            source: Some(1),
            why: TaskWaitWhy::Source(TaskSourceState::Limited { until: None }),
        })
    );
}

#[test]
fn a_task_document_distinguishes_an_application_credential_wait() {
    let (mut core, limits) = ready(false);
    core.workspace_credentials.insert(TASK, Box::new([7])).unwrap();
    let document = task_from_row(&core, &limits, 1, core.tasks.task(TASK).unwrap(), Wall::EPOCH).unwrap();
    assert_eq!(document.run_wait, Some(TaskRunWait::Credential));
}

#[test]
fn a_progress_count_leaves_due_task_watches_unmarked() {
    let (mut core, mut limits) = ready(false);
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
    fleet::resume(&mut core.fleet, &env, &mut out);
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
    let subject =
        core.view_names.insert(WatchedSubject { subject: Subject::Task { task: TASK }, watchers: 1 }).unwrap();
    core.view_subjects
        .insert(Token::new(99), WatchBinding { party: 1, sign_in: 10, subject, window: window() })
        .unwrap();
    step(
        &mut core,
        &Env { now: Time::ZERO, wall: Wall::from_nanos(1), limits },
        Event::Fleet(fleet::Event::Observe {
            channel: Token::new(0),
            run: Token::new(2),
            attempt: Token::new(7),
            report: fleet::Report::Count { kind: fleet::CountKind::TextArrived, elapsed: Duration::ZERO, count: 1 },
        }),
    );
    assert!(core.changed_views.is_empty());
}

const OWNER: u64 = 1;
const OTHER_PERSON: u64 = 2;
const PERSONAL_SOURCE: u32 = 3;
fn login(name: &[u8], state: fleet::LoginState) -> fleet::Login {
    fleet::Login { name: fleet::SourceName::new(name).unwrap(), state }
}
fn inventory(core: &mut Core, limits: &Limits, channel: Token, logins: Box<[fleet::Login]>) {
    let mut out = Queue::with_capacity(fleet::max_out(&limits.fleet));
    fleet::step(
        &mut core.fleet,
        &Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.fleet },
        fleet::Event::Logins { channel, logins },
        &mut out,
    );
}
pub(super) fn placement_fleet(core: &Core, limits: &Limits) -> fleet::Domain {
    let mut config = config(41);
    config.defaults.charters[0].hosts = core.charter(1).unwrap().hosts;
    config.defaults.charters[0].policy = core.charter(1).unwrap().policy.clone();
    config.defaults.charters[0].policy.alternatives = core.settings.models[1..].into();
    let mut fleet = fleet::Domain::new(&limits.fleet, fleet_needs::configured(&config, limits), Box::new([]));
    fleet.configure_workers(
        fleet::Membership::new(config.worker_names, limits.fleet.configured_workers, limits.fleet.worker_name_bytes)
            .unwrap(),
    );
    fleet
}
pub(super) fn placement_fixture(
    main_holder: accounts::Holder,
    login_holder: accounts::Holder,
    personal_first: bool,
) -> (Core, Limits) {
    placement_with_accounts(main_holder, login_holder, personal_first, Vec::new())
}
fn placement_with_accounts(
    main_holder: accounts::Holder,
    login_holder: accounts::Holder,
    personal_first: bool,
    extra: Vec<accounts::Stored>,
) -> (Core, Limits) {
    let (mut core, limits) = ready(false);
    core.current.test_charter_mut(1).policy.alternatives = Box::new([]);
    if personal_first {
        core.settings.models[0].program = Some(b"claude_code".as_slice().into());
        core.current.test_charter_mut(1).policy.model.program = Some(b"claude_code".as_slice().into());
    }
    core.accounts = accounts::Domain::new(&limits.accounts, core.settings.llm_providers.clone());
    let program_account = if main_holder == login_holder { 0 } else { 2 };
    let mut records = vec![
        accounts::Stored::Account {
            number: 0,
            provider: 1,
            holder: main_holder,
            place: 0,
            uses: Some(Box::new([Use::Project(1)])),
            subject: Box::new([]),
            display: Box::new([]),
        },
        accounts::Stored::Source {
            account: 0,
            number: 1,
            place: u32::from(personal_first),
            kind: accounts::Kind::Key,
            name: Box::new([]),
            held: None,
        },
        accounts::Stored::Source {
            account: program_account,
            number: PERSONAL_SOURCE,
            place: u32::from(!personal_first),
            kind: accounts::Kind::Program,
            name: b"personal".as_slice().into(),
            held: None,
        },
        accounts::Stored::Issued {
            number: PERSONAL_SOURCE,
            holder: login_holder,
            name: b"personal".as_slice().into(),
            signed_out: false,
        },
    ];
    if program_account != 0 {
        records.insert(
            1,
            accounts::Stored::Account {
                number: program_account,
                provider: 2,
                holder: login_holder,
                place: 0,
                uses: None,
                subject: Box::new([]),
                display: Box::new([]),
            },
        );
    }
    records.extend(extra);
    for record in records {
        account(&mut core, &limits, accounts::Event::Restore { record });
    }
    account(&mut core, &limits, accounts::Event::Want);
    granted(&mut core, &limits, 1);
    core.people = people::Domain::new(&limits.people, Box::new([]), u16::MAX, Box::new([]));
    let mut out = Queue::with_capacity(people::max_out(&limits.people));
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.people };
    for person in [OWNER, OTHER_PERSON] {
        people::step(
            &mut core.people,
            &env,
            people::Event::Restore {
                record: people::Stored::Person {
                    number: person,
                    identity: people::Identity {
                        key: people::IdentityKey { provider: 0, subject: person.to_be_bytes().into() },
                        login: b"person".as_slice().into(),
                        name: b"Person".as_slice().into(),
                    },
                },
            },
            &mut out,
        );
    }
    people::step(
        &mut core.people,
        &env,
        people::Event::Restore {
            record: people::Stored::Roles {
                project: 1,
                holdings: Box::new([people::Holding { person: OWNER, role: people::Role::Owner }]),
            },
        },
        &mut out,
    );
    people::step(&mut core.people, &env, people::Event::Restored, &mut out);
    core.fleet = placement_fleet(&core, &limits);
    let mut out = Queue::with_capacity(fleet::max_out(&limits.fleet));
    fleet::step(
        &mut core.fleet,
        &Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.fleet },
        fleet::Event::Sources { sources: Box::new([fleet::SourceName::new(b"personal").unwrap()]) },
        &mut out,
    );
    host_with_logins(
        &mut core,
        &limits,
        FIRST,
        &[b"scripted", b"other"],
        Box::new([login(b"personal", fleet::LoginState::Usable)]),
    );
    host_with_logins(
        &mut core,
        &limits,
        SECOND,
        &[b"scripted"],
        Box::new([login(b"personal", fleet::LoginState::NotSignedIn)]),
    );
    (core, limits)
}
#[test]
fn another_person_s_kept_login_keeps_a_run_off_its_host() {
    let (mut core, limits) =
        placement_fixture(accounts::Holder::Deployment, accounts::Holder::Party(OTHER_PERSON), false);
    for state in [
        fleet::LoginState::Usable,
        fleet::LoginState::SignedInOtherwise(fleet::LoginMethod::new(b"api-key").unwrap()),
        fleet::LoginState::Failed { reason: fleet::SourceFailure::Exited, detail: Box::new([]) },
        fleet::LoginState::Failed { reason: fleet::SourceFailure::Deadline, detail: Box::new([]) },
    ] {
        inventory(&mut core, &limits, FIRST, Box::new([login(b"personal", state)]));
        assert_eq!(core.run_choice(TASK).unwrap().placement.channel, SECOND);
    }
}
#[test]
fn a_person_s_own_run_takes_the_host_keeping_their_logins() {
    let (core, _) = placement_fixture(accounts::Holder::Party(OWNER), accounts::Holder::Party(OWNER), false);
    assert_eq!(core.run_choice(TASK).unwrap().placement.channel, FIRST);
}
#[test]
fn a_program_source_is_chosen_only_where_its_login_is_usable() {
    let (core, _) = placement_fixture(accounts::Holder::Party(OWNER), accounts::Holder::Party(OWNER), true);
    let choice = core.run_choice(TASK).unwrap();
    assert_eq!((choice.placement.channel, choice.accounts[0].source), (FIRST, PERSONAL_SOURCE));
}
#[test]
fn an_unusable_program_login_does_not_fall_back_to_a_key() {
    let (mut core, limits) = placement_fixture(accounts::Holder::Party(OWNER), accounts::Holder::Party(OWNER), true);
    inventory(&mut core, &limits, FIRST, Box::new([login(b"personal", fleet::LoginState::NotSignedIn)]));
    assert_eq!(core.run_choice(TASK), Err(RunWait::Host));
}
#[test]
fn a_deployment_held_login_restricts_no_run() {
    let (core, _) = placement_fixture(accounts::Holder::Party(OWNER), accounts::Holder::Deployment, false);
    assert_eq!(core.run_choice(TASK).unwrap().placement.channel, FIRST);
}
#[test]
fn a_direct_run_takes_a_worker_without_kept_logins() {
    let (mut core, limits) =
        placement_fixture(accounts::Holder::Deployment, accounts::Holder::Party(OTHER_PERSON), false);
    inventory(&mut core, &limits, FIRST, Box::new([login(b"personal", fleet::LoginState::NotSignedIn)]));
    assert_eq!(core.run_choice(TASK).unwrap().placement.channel, FIRST);
}
#[test]
fn an_unnamed_kept_login_closes_a_host() {
    let (mut core, limits) = placement_fixture(accounts::Holder::Deployment, accounts::Holder::Party(OWNER), false);
    core.accounts = accounts::Domain::new(&limits.accounts, core.settings.llm_providers.clone());
    restore_key_account(&mut core, &limits, 0, 1, 1);
    account(&mut core, &limits, accounts::Event::Want);
    granted(&mut core, &limits, 1);
    assert_eq!(core.run_choice(TASK).unwrap().placement.channel, SECOND);
}
#[test]
fn one_person_s_deployment_places_every_run_on_their_host() {
    let (mut core, _) = placement_fixture(accounts::Holder::Deployment, accounts::Holder::Party(OTHER_PERSON), false);
    core.people = super::core().0.people;
    assert_eq!(core.run_choice(TASK).unwrap().placement.channel, FIRST);
}
#[test]
fn two_people_s_kept_logins_close_a_host() {
    let (mut core, limits) = placement_with_accounts(
        accounts::Holder::Party(OWNER),
        accounts::Holder::Party(OWNER),
        false,
        vec![accounts::Stored::Issued {
            number: 4,
            holder: accounts::Holder::Party(OTHER_PERSON),
            name: b"other".as_slice().into(),
            signed_out: false,
        }],
    );
    let mut out = Queue::with_capacity(fleet::max_out(&limits.fleet));
    fleet::step(
        &mut core.fleet,
        &Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.fleet },
        fleet::Event::Sources {
            sources: Box::new([
                fleet::SourceName::new(b"personal").unwrap(),
                fleet::SourceName::new(b"other").unwrap(),
            ]),
        },
        &mut out,
    );
    inventory(
        &mut core,
        &limits,
        FIRST,
        Box::new([login(b"personal", fleet::LoginState::Usable), login(b"other", fleet::LoginState::Usable)]),
    );
    inventory(
        &mut core,
        &limits,
        SECOND,
        Box::new([login(b"personal", fleet::LoginState::NotSignedIn), login(b"other", fleet::LoginState::NotSignedIn)]),
    );
    assert_eq!(core.run_choice(TASK).unwrap().placement.channel, SECOND);
}
#[test]
fn the_engine_never_takes_a_program_source() {
    let (mut core, limits) = placement_fixture(accounts::Holder::Party(OWNER), accounts::Holder::Party(OWNER), true);
    core.current.test_charter_mut(1).hosts = fleet::Kinds::Engine;
    let mut limits = limits;
    limits.fleet.engine_slots = 1;
    core.fleet = placement_fleet(&core, &limits);
    host(&mut core, &limits, Token::new(0), &[b"scripted"]);
    assert_eq!(core.run_choice(TASK), Err(RunWait::Host));
}

#[test]
fn a_retained_provider_binds_an_account_the_rule_allows() {
    let mut extra = Vec::new();
    for (number, source, holder) in [(2, 7, accounts::Holder::Deployment), (5, 6, accounts::Holder::Party(OWNER))] {
        extra.push(accounts::Stored::Account {
            number,
            provider: 2,
            holder,
            place: number,
            uses: Some(Box::new([Use::Project(1)])),
            subject: Box::new([]),
            display: Box::new([]),
        });
        extra.push(accounts::Stored::Source {
            account: number,
            number: source,
            place: 0,
            kind: accounts::Kind::Key,
            name: Box::new([]),
            held: None,
        });
    }
    let (mut core, limits) =
        placement_with_accounts(accounts::Holder::Party(OWNER), accounts::Holder::Party(OWNER), false, extra);
    core.current.test_charter_mut(1).policy.alternatives = Box::new([core.settings.models[2].clone()]);
    granted(&mut core, &limits, 7);
    granted(&mut core, &limits, 6);
    let choice = core.run_choice(TASK).unwrap();
    assert_eq!(
        choice.accounts.as_ref(),
        [
            authority::AccountBinding { provider: 1, account: 0, source: 1 },
            authority::AccountBinding { provider: 2, account: 5, source: 6 }
        ]
    );
}

#[test]
fn a_login_kept_after_the_check_fails_the_assignment_recheck() {
    let (mut core, limits) =
        placement_fixture(accounts::Holder::Deployment, accounts::Holder::Party(OTHER_PERSON), false);
    inventory(&mut core, &limits, FIRST, Box::new([login(b"personal", fleet::LoginState::NotSignedIn)]));
    let choice = core.run_choice(TASK).unwrap();
    let mut proof = RunProof::claimed(TASK, 7, 7, None, choice.placement.kind);
    proof.placement = Some(choice.placement.channel);
    proof.accounts = choice.accounts;
    core.proofs.insert(TASK, proof).unwrap();
    inventory(&mut core, &limits, FIRST, Box::new([login(b"personal", fleet::LoginState::Usable)]));
    assert!(!core.claim_bindings_usable(TASK, 7));
}

#[test]
fn a_declared_model_s_account_takes_no_program_source() {
    let (mut core, limits) = placement_with_accounts(
        accounts::Holder::Party(OWNER),
        accounts::Holder::Party(OWNER),
        false,
        vec![
            accounts::Stored::Account {
                number: 5,
                provider: 2,
                holder: accounts::Holder::Party(OWNER),
                place: 1,
                uses: None,
                subject: Box::new([]),
                display: Box::new([]),
            },
            accounts::Stored::Source {
                account: 5,
                number: 6,
                place: 0,
                kind: accounts::Kind::Program,
                name: b"other-program".as_slice().into(),
                held: None,
            },
            accounts::Stored::Issued {
                number: 6,
                holder: accounts::Holder::Party(OWNER),
                name: b"other-program".as_slice().into(),
                signed_out: false,
            },
        ],
    );
    core.settings.models[2].program = Some(b"claude_code".as_slice().into());
    core.current.test_charter_mut(1).policy.alternatives = Box::new([core.settings.models[2].clone()]);
    let mut out = Queue::with_capacity(fleet::max_out(&limits.fleet));
    fleet::step(
        &mut core.fleet,
        &Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.fleet },
        fleet::Event::Sources {
            sources: Box::new([
                fleet::SourceName::new(b"personal").unwrap(),
                fleet::SourceName::new(b"other-program").unwrap(),
            ]),
        },
        &mut out,
    );
    inventory(
        &mut core,
        &limits,
        FIRST,
        Box::new([login(b"personal", fleet::LoginState::Usable), login(b"other-program", fleet::LoginState::Usable)]),
    );
    let choice = core.run_choice(TASK).expect("credential main runs");
    assert_eq!(choice.accounts.as_ref(), &[authority::AccountBinding { provider: 1, account: 0, source: 1 }]);
    assert!(choice.policy.alternatives.is_empty());
}

#[test]
fn a_declared_model_with_mains_program_shares_mains_account() {
    let (mut core, limits) = placement_fixture(accounts::Holder::Party(OWNER), accounts::Holder::Party(OWNER), true);
    core.settings.models[1].program = core.settings.models[0].program.clone();
    core.current.test_charter_mut(1).policy.alternatives = Box::new([core.settings.models[1].clone()]);
    core.fleet = placement_fleet(&core, &limits);
    let mut out = Queue::with_capacity(fleet::max_out(&limits.fleet));
    fleet::step(
        &mut core.fleet,
        &Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.fleet },
        fleet::Event::Sources { sources: Box::new([fleet::SourceName::new(b"personal").unwrap()]) },
        &mut out,
    );
    host_with_logins(
        &mut core,
        &limits,
        FIRST,
        &[b"scripted", b"same"],
        Box::new([login(b"personal", fleet::LoginState::Usable)]),
    );
    let choice = core.run_choice(TASK).expect("program models run together");
    assert_eq!(choice.placement.channel, FIRST);
    assert_eq!(
        choice.accounts.as_ref(),
        &[authority::AccountBinding { provider: 1, account: 0, source: PERSONAL_SOURCE }]
    );
    assert_eq!(choice.policy.alternatives.as_ref(), core::slice::from_ref(&core.settings.models[1]));
}

fn fleet_event(core: &mut Core, limits: &Limits, wall: Wall, event: fleet::Event) {
    let mut out = Queue::with_capacity(fleet::max_out(&limits.fleet));
    fleet::step(&mut core.fleet, &Env { now: Time::ZERO, wall, limits: limits.fleet }, event, &mut out);
}
fn document_wait(core: &Core, limits: &Limits) -> Option<TaskRunWait> {
    task_from_row(core, limits, 1, core.tasks.task(TASK).unwrap(), Wall::EPOCH).unwrap().run_wait
}

#[test]
fn due_and_preparing_documents_refine_only_the_host_wait_from_an_actual_refused_hello() {
    for kind in [fleet::HostKind::Engine, fleet::HostKind::Worker] {
        let (mut core, mut limits) = ready(false);
        limits.fleet.history = 0;
        limits.fleet.engine_slots = 1;
        core.current.test_charter_mut(1).hosts = fleet::Kinds::Both;
        let mut configuration = config(41);
        configuration.defaults.charters[0].hosts = fleet::Kinds::Both;
        let needs = fleet_needs::configured(&configuration, &limits);
        core.fleet = fleet::Domain::new(&limits.fleet, needs.clone(), Box::new([]));
        assert_eq!(document_wait(&core, &limits), Some(TaskRunWait::Host));
        let channel = if kind == fleet::HostKind::Engine { Token::new(0) } else { FIRST };
        let at = Wall::from_nanos(1234);
        let mut bounds = fixtures::bounds();
        bounds.turns = 0;
        fleet_event(
            &mut core,
            &limits,
            at,
            fleet::Event::Hello {
                channel,
                worker: 0,
                hello: fleet::Hello {
                    bounds,
                    stop_bound: Duration::ZERO,
                    slots: 1,
                    workstreams: Box::new([]),
                    hosting: Box::new([]),
                },
            },
        );
        let expected = Some(TaskRunWait::Refused(TaskWaitRefused::Hello {
            kind,
            at,
            why: fleet::HelloRefusal::Agent { why: fleet::AgentShortfall::Turns { charter: 1 } },
        }));
        assert_eq!(document_wait(&core, &limits), expected);
        assert!(core.fleet.hello_refusals().is_empty());
        core.current.test_charter_mut(1).hosts = match kind {
            fleet::HostKind::Engine => fleet::Kinds::Workers,
            fleet::HostKind::Worker => fleet::Kinds::Engine,
        };
        assert_eq!(document_wait(&core, &limits), Some(TaskRunWait::Host), "another kind cannot explain this task");
        core.current.test_charter_mut(1).hosts = fleet::Kinds::Both;
        send_task(&mut core, &limits, tasks::Event::Prepare { reply_to: ReplyTo::new(FIRST), task: TASK });
        assert_eq!(core.tasks.task(TASK).unwrap().phase, tasks::Phase::Active(tasks::Active::Preparing));
        assert_eq!(document_wait(&core, &limits), expected);
        send_task(&mut core, &limits, tasks::Event::Hold { task: TASK, why: tasks::Hold::Uncertain { entry: 1 } });
        assert_eq!(document_wait(&core, &limits), None);
        // A new fleet has no durable refusal; observing it again reconstructs the explanation.
        core.fleet = fleet::Domain::new(&limits.fleet, needs, Box::new([]));
        let mut row = core.tasks.task(TASK).unwrap().clone();
        row.phase = tasks::Phase::Active(tasks::Active::Due);
        assert_eq!(task_from_row(&core, &limits, 1, &row, Wall::EPOCH).unwrap().run_wait, Some(TaskRunWait::Host));
        fleet_event(
            &mut core,
            &limits,
            at,
            fleet::Event::Hello {
                channel,
                worker: 0,
                hello: fleet::Hello {
                    bounds: fleet::Bounds { turns: 0, ..fixtures::bounds() },
                    stop_bound: Duration::ZERO,
                    slots: 1,
                    workstreams: Box::new([]),
                    hosting: Box::new([]),
                },
            },
        );
        assert_eq!(task_from_row(&core, &limits, 1, &row, Wall::EPOCH).unwrap().run_wait, expected);
    }
}

#[test]
fn a_standing_refusal_does_not_replace_account_or_credential_waits_or_placed_work() {
    let (mut core, limits) = ready(false);
    let at = Wall::from_nanos(1234);
    fleet_event(
        &mut core,
        &limits,
        at,
        fleet::Event::Hello {
            channel: Token::new(99),
            worker: 0,
            hello: fleet::Hello {
                bounds: fleet::Bounds { turns: 0, ..fixtures::bounds() },
                stop_bound: Duration::ZERO,
                slots: 1,
                workstreams: Box::new([]),
                hosting: Box::new([]),
            },
        },
    );
    assert_eq!(document_wait(&core, &limits), None, "an admitted compatible path remains usable");
    core.workspace_credentials.insert(TASK, Box::new([7])).unwrap();
    assert_eq!(document_wait(&core, &limits), Some(TaskRunWait::Credential));
    account(&mut core, &limits, accounts::Event::Limited { account: 1, reset: None });
    assert_eq!(
        document_wait(&core, &limits),
        Some(TaskRunWait::Account {
            account: Some(0),
            source: Some(1),
            why: TaskWaitWhy::Source(TaskSourceState::Limited { until: None }),
        })
    );
    let mut row = core.tasks.task(TASK).unwrap().clone();
    for phase in [tasks::Active::Claimed { attempt: 1 }, tasks::Active::Running { attempt: 1 }, tasks::Active::Idle] {
        row.phase = tasks::Phase::Active(phase);
        assert_eq!(task_from_row(&core, &limits, 1, &row, Wall::EPOCH).unwrap().run_wait, None);
    }
    row.phase = tasks::Phase::Active(tasks::Active::Due);
    // A due person task always carries its wait; its document reads the row itself.
    row.person_wait = Some(tasks::PersonWait {
        since: Wall::EPOCH,
        stall: Duration::from_secs(1),
        deadline: Wall::from_nanos(1_000_000_000),
        notice: None,
    });
    for executor in
        [tasks::Executor::Procedure { connector: 1, code: 1 }, tasks::Executor::Person(tasks::PersonAddress::Person(1))]
    {
        row.executor = executor;
        assert_eq!(task_from_row(&core, &limits, 1, &row, Wall::EPOCH).unwrap().run_wait, None);
    }
}

const OPEN_TASK: u64 = 2;
const OPEN_ATTEMPT: u64 = 7;
fn open(core: &mut Core, limits: &Limits) {
    make_task_ended_by(core, limits, OPEN_TASK, tasks::EndedBy::Requester);
}

fn claim_open(core: &mut Core, limits: &Limits, attempt: u64) {
    open(core, limits);
    claim_existing_open(core, limits, attempt);
}

fn claim_existing_open(core: &mut Core, limits: &Limits, attempt: u64) {
    send_task(core, limits, tasks::Event::Prepare { reply_to: ReplyTo::new(FIRST), task: OPEN_TASK });
    send_task(
        core,
        limits,
        tasks::Event::Claim {
            reply_to: ReplyTo::new(FIRST),
            task: OPEN_TASK,
            attempt,
            resumed: false,
            budget: 10,
            account: Some(0),
            writes: Box::new([]),
        },
    );
    let mut proof = RunProof::claimed(OPEN_TASK, attempt, attempt, None, fleet::HostKind::Worker);
    proof.placement = Some(FIRST);
    proof.accounts = Box::new([authority::AccountBinding { provider: 1, account: 0, source: PERSONAL_SOURCE }]);
    core.proofs.insert(OPEN_TASK, proof).unwrap();
}

fn crossed_claim() -> (Core, Env<Limits>) {
    let (mut core, limits) = ready(false);
    claim_open(&mut core, &limits, OPEN_ATTEMPT);
    core.proofs.get_mut(&OPEN_TASK).unwrap().crossed = true;
    (core, Env { now: Time::ZERO, wall: Wall::EPOCH, limits })
}

fn crossed_callback(worked: bool) -> (u32, u32, tasks::End) {
    let (mut core, mut env) = crossed_claim();
    core.adopt(&env, OPEN_TASK, OPEN_ATTEMPT, 0);
    let fleet::Event::Cross { reply_to, run, attempt, .. } = core.adopted.pop().unwrap() else {
        panic!("crossed claim")
    };
    // Exercise the parent's handling of either fleet terminal while its
    // restored proof is still unworked and no worker has reported it.
    drop(step(&mut core, &env, Event::Fleet(fleet::Event::Cross { reply_to, run, attempt, worked })));
    env.now = env.now.saturating_add(env.limits.fleet.grace);
    env.wall = Wall::from_nanos(env.limits.fleet.grace.as_nanos());
    drop(fire(&mut core, &env, Timer::Fleet));
    let task = core.tasks.task(OPEN_TASK).unwrap();
    let proof = core.proofs.get(&OPEN_TASK).unwrap();
    (task.tries.lost, task.refusals, proof.terminal.as_ref().unwrap().end.clone())
}

#[test]
fn a_crossed_claims_lost_callback_is_lost_even_unworked_and_unreported() {
    assert_eq!(crossed_callback(true), (1, 0, tasks::End::Failed(tasks::Class::Lost)));
}

#[test]
fn a_crossed_claims_not_started_callback_is_refused() {
    assert_eq!(crossed_callback(false), (0, 1, tasks::End::Refused));
}

#[test]
fn a_crossed_claim_passes_its_committed_turns_or_calls_to_the_fleet() {
    let key = CallKey { task: OPEN_TASK, attempt: OPEN_ATTEMPT, completion: 1, position: 0 };
    let earlier = CallKey { attempt: OPEN_ATTEMPT - 1, ..key };
    for (kept, call, expected) in [(0, None, false), (1, None, true), (0, Some(key), true), (0, Some(earlier), false)] {
        let (mut core, env) = crossed_claim();
        if let Some(call) = call {
            core.call_parts.insert(call, CallPart::Controlled).unwrap();
        }
        core.adopt(&env, OPEN_TASK, OPEN_ATTEMPT, kept);
        let fleet::Event::Cross { worked, .. } = core.adopted.pop().unwrap() else { panic!("crossed claim") };
        assert_eq!(worked, expected);
    }
}

#[test]
fn crossed_worker_claims_still_count_toward_restart_admission() {
    for (room, admitted) in [(0, false), (1, true)] {
        let (mut core, env) = crossed_claim();
        core.adopt(&env, OPEN_TASK, OPEN_ATTEMPT, 0);
        core.restart = restart::Restart::Waiting(RestartStep::AdoptRuns);
        // One restored worker claim needs one recovery slot, crossed or not.
        core.restart_attempts = room;
        assert_eq!(core.restart_admit_claims(), admitted);
    }
}

fn invalid_start_answer(attempt: u64, refused: &[u64]) -> Event {
    Event::AnswerPayload {
        run: Token::new(OPEN_TASK),
        attempt: Token::new(attempt),
        payload: Token::new(99),
        cumulative: 0,
        end: tasks::End::Failed(tasks::Class::Invalid),
        saved: None,
        invalid_saved: false,
        read: None,
        refused: refused.into(),
    }
}

#[test]
fn a_stale_grants_refusal_drops_its_payload_without_rechecking() {
    let (mut core, limits) = placement_fixture(accounts::Holder::Party(OWNER), accounts::Holder::Party(OWNER), true);
    let current = OPEN_ATTEMPT + 1;
    claim_open(&mut core, &limits, current);
    let before = core.tasks.task(OPEN_TASK).unwrap().clone();
    let out =
        step(&mut core, &Env { now: Time::ZERO, wall: Wall::EPOCH, limits }, invalid_start_answer(OPEN_ATTEMPT, &[]));
    assert_eq!(core.tasks.task(OPEN_TASK).unwrap(), &before);
    assert_eq!(core.tasks.task(OPEN_TASK).unwrap().refusals, before.refusals);
    assert_eq!(core.tasks.task(OPEN_TASK).unwrap().tries, tasks::Tries::NONE);
    assert_eq!(core.proofs.get(&OPEN_TASK).unwrap().attempt, current);
    assert_eq!(core.proofs.get(&OPEN_TASK).unwrap().terminal, None);
    let Requests::Out(mut out) = out;
    assert_eq!(out.len(), 2);
    let Some(Request::Now(now)) = out.pop() else { panic!("drop the stale payload") };
    let Now::DropPayload { payload } = *now else { panic!("drop the stale payload") };
    assert_eq!(payload, Token::new(99));
    let Some(Request::Decided) = out.pop() else { panic!("one terminal decision") };
    assert!(out.is_empty());
}

#[test]
fn an_alternatives_program_source_does_not_spare_a_try() {
    let (mut core, limits) = placement_fixture(accounts::Holder::Deployment, accounts::Holder::Party(OWNER), false);
    let mut alternative = core.settings.models[2].clone();
    alternative.program = Some(b"claude_code".as_slice().into());
    core.current.test_charter_mut(1).policy.alternatives = Box::new([alternative]);
    claim_open(&mut core, &limits, OPEN_ATTEMPT);
    let bindings = Box::new([
        authority::AccountBinding { provider: 1, account: 0, source: 1 },
        authority::AccountBinding { provider: 2, account: 2, source: PERSONAL_SOURCE },
    ]);
    core.proofs.get_mut(&OPEN_TASK).unwrap().accounts = bindings.clone();
    let refusals = core.tasks.task(OPEN_TASK).unwrap().refusals;
    let _out =
        step(&mut core, &Env { now: Time::ZERO, wall: Wall::EPOCH, limits }, invalid_start_answer(OPEN_ATTEMPT, &[]));
    let proof = core.proofs.get(&OPEN_TASK).unwrap();
    assert_eq!(proof.terminal.as_ref().unwrap().end, tasks::End::Failed(tasks::Class::Invalid));
    assert_eq!(proof.accounts.as_ref(), bindings.as_ref());
    let task = core.tasks.task(OPEN_TASK).unwrap();
    let tasks::Phase::Active(tasks::Active::BackingOff { .. }) = task.phase else {
        panic!("an invalid start backs off before its next try");
    };
    assert_eq!(task.tries, tasks::Tries { invalid: 1, ..tasks::Tries::NONE });
    assert_eq!(task.refusals, refusals);
}

#[test]
fn an_invalid_start_spends_a_try_despite_wrong_provider() {
    let (mut core, limits) = placement_fixture(accounts::Holder::Party(OWNER), accounts::Holder::Party(OWNER), true);
    claim_open(&mut core, &limits, OPEN_ATTEMPT);
    core.proofs.get_mut(&OPEN_TASK).unwrap().accounts[0].provider = 2;
    let refusals = core.tasks.task(OPEN_TASK).unwrap().refusals;
    let _out =
        step(&mut core, &Env { now: Time::ZERO, wall: Wall::EPOCH, limits }, invalid_start_answer(OPEN_ATTEMPT, &[]));
    assert_eq!(
        core.proofs.get(&OPEN_TASK).unwrap().terminal.as_ref().unwrap().end,
        tasks::End::Failed(tasks::Class::Invalid)
    );
    let task = core.tasks.task(OPEN_TASK).unwrap();
    assert_eq!(task.refusals, refusals);
    assert_eq!(task.tries, tasks::Tries { invalid: 1, ..tasks::Tries::NONE });
    assert_eq!(task.attempt, OPEN_ATTEMPT);
}

#[test]
fn a_grants_refusal_above_the_offered_message_is_invalid() {
    const OFFERED: u64 = 1;
    let (mut core, limits) = placement_fixture(accounts::Holder::Party(OWNER), accounts::Holder::Party(OWNER), true);
    claim_open(&mut core, &limits, OPEN_ATTEMPT);
    core.proofs.get_mut(&OPEN_TASK).unwrap().offered = Some(OFFERED);
    let refusals = core.tasks.task(OPEN_TASK).unwrap().refusals;
    let _out = step(
        &mut core,
        &Env { now: Time::ZERO, wall: Wall::EPOCH, limits },
        invalid_start_answer(OPEN_ATTEMPT, &[OFFERED + 1]),
    );
    assert_eq!(
        core.proofs.get(&OPEN_TASK).unwrap().terminal.as_ref().unwrap().end,
        tasks::End::Failed(tasks::Class::Invalid)
    );
    let task = core.tasks.task(OPEN_TASK).unwrap();
    assert_eq!(task.refusals, refusals);
    assert_eq!(task.tries, tasks::Tries { invalid: 1, ..tasks::Tries::NONE });
}

#[test]
fn claim_rechecks_an_alternatives_changed_binding_without_rewriting_the_choice() {
    let (mut core, limits) = ready(false);
    open(&mut core, &limits);
    granted(&mut core, &limits, 2);
    let choice = core.run_choice(OPEN_TASK).unwrap();
    let bindings = choice.accounts.clone();
    assert_eq!(bindings.len(), 2);
    assert_eq!(bindings[1], authority::AccountBinding { provider: 2, account: 3, source: 2 });
    let mut proof = RunProof::claimed(OPEN_TASK, OPEN_ATTEMPT, OPEN_ATTEMPT, None, choice.placement.kind);
    proof.placement = Some(choice.placement.channel);
    proof.accounts = choice.accounts;
    core.proofs.insert(OPEN_TASK, proof).unwrap();
    assert!(core.claim_bindings_usable(OPEN_TASK, OPEN_ATTEMPT));
    let task = core.tasks.task(OPEN_TASK).unwrap().clone();
    account(&mut core, &limits, accounts::Event::Limited { account: 2, reset: None });
    assert!(!core.claim_bindings_usable(OPEN_TASK, OPEN_ATTEMPT));
    assert_eq!(core.proofs.get(&OPEN_TASK).unwrap().accounts, bindings);
    assert_eq!(core.tasks.task(OPEN_TASK).unwrap(), &task);
    assert_eq!(core.tasks.task(OPEN_TASK).unwrap().tries, tasks::Tries::NONE);
}

#[test]
fn a_repaired_source_makes_the_waiting_open_task_due() {
    let (mut core, limits) = ready(false);
    open(&mut core, &limits);
    account(&mut core, &limits, accounts::Event::Limited { account: 1, reset: None });
    core.due.push(core.tasks.run_context(OPEN_TASK).unwrap());
    assert!(!core.due_ready());
    assert!(core.take_due().is_none());
    assert_eq!(
        core.run_choice(OPEN_TASK),
        Err(RunWait::Account {
            account: Some(0),
            source: Some(1),
            why: accounts::WaitWhy::Source(accounts::State::Limited { until: None }),
        })
    );
    let before = core.tasks.task(OPEN_TASK).unwrap().clone();
    account(
        &mut core,
        &limits,
        accounts::Event::Change {
            request: Token::new(100),
            holder: accounts::Holder::Deployment,
            change: accounts::Change::Resume { source: 1 },
        },
    );
    assert!(core.due_ready());
    assert_eq!(core.take_due().unwrap().task, OPEN_TASK);
    assert!(core.take_due().is_none());
    assert_eq!(
        core.run_choice(OPEN_TASK).unwrap().accounts.as_ref(),
        &[authority::AccountBinding { provider: 1, account: 0, source: 1 }]
    );
    assert_eq!(core.tasks.task(OPEN_TASK).unwrap(), &before);
    assert_eq!(core.tasks.task(OPEN_TASK).unwrap().phase, tasks::Phase::Active(tasks::Active::Due));
    assert_eq!(core.tasks.task(OPEN_TASK).unwrap().attempt, 0);
}

#[test]
fn a_repeated_grants_refusal_spends_its_tries() {
    // The fixture allows one Invalid retry: the second failure holds the task.
    const LAST_INVALID_TRY: u32 = 2;
    let (mut core, limits) = placement_fixture(accounts::Holder::Party(OWNER), accounts::Holder::Party(OWNER), true);
    open(&mut core, &limits);
    let mut env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits };
    for (attempt, invalid) in [(OPEN_ATTEMPT, 1), (OPEN_ATTEMPT + 1, LAST_INVALID_TRY)] {
        claim_existing_open(&mut core, &limits, attempt);
        let _out = step(&mut core, &env, invalid_start_answer(attempt, &[]));
        assert_eq!(
            core.proofs.get(&OPEN_TASK).unwrap().terminal.as_ref().unwrap().end,
            tasks::End::Failed(tasks::Class::Invalid)
        );
        let task = core.tasks.task(OPEN_TASK).unwrap();
        assert_eq!(task.tries, tasks::Tries { invalid, ..tasks::Tries::NONE });
        assert_eq!(task.refusals, 0);
        assert_eq!(task.attempt, attempt);
        if let tasks::Phase::Active(tasks::Active::BackingOff { until }) = task.phase {
            env.now = core.tasks.next_deadline().unwrap();
            env.wall = until;
            drop(fire(&mut core, &env, Timer::Tasks));
        }
    }
    assert_eq!(
        core.tasks.task(OPEN_TASK).unwrap().phase,
        tasks::Phase::Held {
            was: tasks::Was::Active(tasks::Active::Due),
            why: tasks::Hold::Failures(tasks::Class::Invalid),
            source: None,
        }
    );
    core.due.push(core.tasks.run_context(OPEN_TASK).unwrap());
    assert!(!core.due_ready());
    assert!(core.take_due().is_none());
    assert_eq!(core.proofs.len(), 1);
}

#[test]
fn an_open_task_chooses_its_program_source() {
    let (mut core, limits) = placement_fixture(accounts::Holder::Party(OWNER), accounts::Holder::Party(OWNER), true);
    open(&mut core, &limits);
    let choice = core.run_choice(OPEN_TASK).unwrap();
    assert_eq!(
        choice.accounts.as_ref(),
        &[authority::AccountBinding { provider: 1, account: 0, source: PERSONAL_SOURCE }]
    );
    assert_eq!(choice, core.run_choice(TASK).unwrap());
    core.due.push(core.tasks.run_context(OPEN_TASK).unwrap());
    assert!(core.due_ready());
    assert_eq!(core.take_due().unwrap().task, OPEN_TASK);
    assert!(core.take_due().is_none());
    assert_eq!(core.tasks.task(OPEN_TASK).unwrap().attempt, 0);
    assert_eq!(core.tasks.task(OPEN_TASK).unwrap().tries, tasks::Tries::NONE);
}

#[test]
fn an_open_program_endpoint_still_checks_its_account() {
    let (mut core, limits) = ready(false);
    open(&mut core, &limits);
    let mut fleet = placement_fleet(&core, &limits);
    core::mem::swap(&mut core.fleet, &mut fleet);
    let main_endpoint = core.chosen_main(OPEN_TASK).endpoint;
    host_with_endpoints(&mut core, &limits, FIRST, &[b"scripted", b"same"], Box::new([]), Box::new([main_endpoint]));
    assert_eq!(core.run_choice(OPEN_TASK).unwrap(), core.run_choice(TASK).unwrap());
    account(&mut core, &limits, accounts::Event::Limited { account: 1, reset: None });
    assert_eq!(
        core.run_choice(OPEN_TASK),
        Err(RunWait::Account {
            account: Some(0),
            source: Some(1),
            why: accounts::WaitWhy::Source(accounts::State::Limited { until: None })
        })
    );
}

#[test]
fn claim_rechecks_the_same_open_model_and_grant_view() {
    for task in [TASK, OPEN_TASK] {
        let (mut core, limits) = ready(false);
        open(&mut core, &limits);
        let choice = core.run_choice(task).unwrap();
        let bindings = choice.accounts.clone();
        let mut proof = RunProof::claimed(task, OPEN_ATTEMPT, OPEN_ATTEMPT, None, choice.placement.kind);
        proof.placement = Some(choice.placement.channel);
        proof.accounts = choice.accounts;
        core.proofs.insert(task, proof).unwrap();
        assert!(core.claim_bindings_usable(task, OPEN_ATTEMPT));
        account(&mut core, &limits, accounts::Event::Limited { account: 1, reset: None });
        assert!(!core.claim_bindings_usable(task, OPEN_ATTEMPT));
        assert_eq!(core.proofs.get(&task).unwrap().accounts, bindings);
    }
}

#[test]
fn an_invalid_start_spends_a_try_across_fences_and_endings() {
    for ending in [tasks::EndedBy::Result, tasks::EndedBy::Requester] {
        for program in [true, false] {
            for (invalid_saved, cumulative, read, refused, accepted_turn) in [
                (false, 0, None, &[][..], false),
                (true, 0, None, &[][..], false),
                (false, 1, None, &[][..], false),
                (false, 0, Some(1), &[][..], false),
                (false, 0, None, &[0][..], false),
                (false, 0, None, &[][..], true),
            ] {
                let (mut core, limits) =
                    placement_fixture(accounts::Holder::Party(OWNER), accounts::Holder::Party(OWNER), true);
                make_task_ended_by(&mut core, &limits, OPEN_TASK, ending);
                claim_existing_open(&mut core, &limits, OPEN_ATTEMPT);
                core.proofs.get_mut(&OPEN_TASK).unwrap().accounts[0].source = if program { PERSONAL_SOURCE } else { 1 };
                if accepted_turn {
                    core.proofs.get_mut(&OPEN_TASK).unwrap().turn =
                        Some(TurnProof { turn: 1, cumulative: 0, read: None });
                }
                let _out = step(
                    &mut core,
                    &Env { now: Time::ZERO, wall: Wall::EPOCH, limits },
                    Event::AnswerPayload {
                        run: Token::new(OPEN_TASK),
                        attempt: Token::new(OPEN_ATTEMPT),
                        payload: Token::new(99),
                        cumulative,
                        end: tasks::End::Failed(tasks::Class::Invalid),
                        saved: None,
                        invalid_saved,
                        read,
                        refused: Box::from(refused),
                    },
                );
                assert_eq!(
                    core.tasks.task(OPEN_TASK).unwrap().tries,
                    tasks::Tries { invalid: 1, ..tasks::Tries::NONE }
                );
                let terminal = core.proofs.get(&OPEN_TASK).unwrap().terminal.clone();
                assert_eq!(terminal.as_ref().unwrap().end, tasks::End::Failed(tasks::Class::Invalid));
                let before = core.tasks.task(OPEN_TASK).unwrap().clone();
                let _replay = step(
                    &mut core,
                    &Env { now: Time::ZERO, wall: Wall::EPOCH, limits },
                    invalid_start_answer(OPEN_ATTEMPT, &[]),
                );
                assert_eq!(core.tasks.task(OPEN_TASK).unwrap(), &before);
                assert_eq!(core.proofs.get(&OPEN_TASK).unwrap().terminal, terminal);
            }
        }
    }
}

#[test]
fn an_open_task_retains_its_program_endpoint_alternative() {
    const PROGRAM_ENDPOINT: u32 = 17;
    let (mut core, limits) = ready(false);
    open(&mut core, &limits);
    core.current.test_charter_mut(1).policy.alternatives[0].endpoint = PROGRAM_ENDPOINT;
    let mut old = placement_fleet(&core, &limits);
    core::mem::swap(&mut core.fleet, &mut old);
    host_with_endpoints(&mut core, &limits, FIRST, &[b"scripted", b"same"], Box::new([]), Box::new([PROGRAM_ENDPOINT]));
    let choice = core.run_choice(OPEN_TASK).unwrap();
    assert_eq!(
        choice.policy.alternatives.as_ref(),
        core::slice::from_ref(&core.current.test_charter_mut(1).policy.alternatives[0])
    );
    assert_eq!(choice, core.run_choice(TASK).unwrap());
    let mut proof = RunProof::claimed(OPEN_TASK, 7, 7, None, choice.placement.kind);
    proof.placement = Some(choice.placement.channel);
    proof.accounts = choice.accounts;
    core.proofs.insert(OPEN_TASK, proof).unwrap();
    assert!(core.claim_bindings_usable(OPEN_TASK, 7));
}

#[test]
fn a_program_host_and_its_replacement_make_open_due() {
    let (mut core, limits) = ready(false);
    open(&mut core, &limits);
    let endpoint = core.chosen_main(OPEN_TASK).endpoint;
    let mut old = placement_fleet(&core, &limits);
    core::mem::swap(&mut core.fleet, &mut old);
    host_with_endpoints(&mut core, &limits, FIRST, &[b"scripted", b"same"], Box::new([]), Box::new([endpoint]));
    core.due.push(core.tasks.run_context(OPEN_TASK).unwrap());
    assert!(core.due_ready());
    assert_eq!(core.take_due().unwrap().task, OPEN_TASK);
    assert!(core.take_due().is_none());
    let document = task_from_row(&core, &limits, 1, core.tasks.task(OPEN_TASK).unwrap(), Wall::EPOCH).unwrap();
    assert_eq!(document.run_wait, None);
    let mut out = Queue::with_capacity(fleet::max_out(&limits.fleet));
    fleet::step(
        &mut core.fleet,
        &Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.fleet },
        fleet::Event::Lost { channel: FIRST },
        &mut out,
    );
    core.fleet.reclaim();
    core.due.push(core.tasks.run_context(OPEN_TASK).unwrap());
    assert_eq!(core.run_choice(OPEN_TASK), Err(RunWait::Host));
    assert!(!core.due_ready());
    host(&mut core, &limits, Token::new(9), &[b"scripted", b"same"]);
    assert_eq!(core.take_due().unwrap().task, OPEN_TASK);
    assert!(core.take_due().is_none());
    assert_eq!(core.tasks.task(OPEN_TASK).unwrap().attempt, 0);
}

#[test]
fn program_metadata_on_an_ineligible_probe_does_not_refuse_a_native_choice() {
    let (mut core, limits) = ready(false);
    open(&mut core, &limits);
    granted(&mut core, &limits, 2);
    let endpoint = core.chosen_main(OPEN_TASK).endpoint;
    let mut old = placement_fleet(&core, &limits);
    core::mem::swap(&mut core.fleet, &mut old);
    host_with_endpoints(&mut core, &limits, FIRST, &[b"scripted", b"same"], Box::new([]), Box::new([endpoint]));
    host(&mut core, &limits, SECOND, &[b"scripted", b"same", b"other"]);
    assert_eq!(core.run_choice(OPEN_TASK).unwrap().placement.channel, SECOND);
}

#[test]
fn an_open_start_takes_the_first_eligible_hosts_answer_in_either_order() {
    // Equal free slots probe in channel order; exchange which channel is program-marked.
    for (program, native) in [(FIRST, SECOND), (SECOND, FIRST)] {
        let (mut core, limits) = ready(false);
        open(&mut core, &limits);
        let endpoint = core.chosen_main(OPEN_TASK).endpoint;
        let mut old = placement_fleet(&core, &limits);
        core::mem::swap(&mut core.fleet, &mut old);
        host_with_endpoints(&mut core, &limits, program, &[b"scripted", b"same"], Box::new([]), Box::new([endpoint]));
        host(&mut core, &limits, native, &[b"scripted", b"same"]);
        assert_eq!(core.run_choice(OPEN_TASK).unwrap().placement.channel, FIRST);
        core.due.push(core.tasks.run_context(OPEN_TASK).unwrap());
        assert!(core.due_ready());
        assert_eq!(core.take_due().unwrap().task, OPEN_TASK);
        assert!(core.take_due().is_none());
        assert!(core.proofs.get(&OPEN_TASK).is_none());
        assert_eq!(core.tasks.task(OPEN_TASK).unwrap().attempt, 0);
    }
}

#[test]
fn a_bound_failure_is_not_rechecked_while_its_account_waits() {
    let (mut core, limits) = placement_fixture(accounts::Holder::Party(OWNER), accounts::Holder::Party(OWNER), true);
    claim_open(&mut core, &limits, OPEN_ATTEMPT);
    core.proofs.get_mut(&OPEN_TASK).unwrap().account_terminal = true;
    let exceeded = tasks::Exceeded { bound: tasks::Bound::Reasoning, value: 2048 };
    let mut answer = invalid_start_answer(OPEN_ATTEMPT, &[]);
    let Event::AnswerPayload { end, .. } = &mut answer else { panic!("answer fixture") };
    *end = tasks::End::Exceeded(exceeded);
    let _out = step(&mut core, &Env { now: Time::ZERO, wall: Wall::EPOCH, limits }, answer);
    assert_eq!(core.proofs.get(&OPEN_TASK).unwrap().terminal.as_ref().unwrap().end, tasks::End::Exceeded(exceeded));
    let task = core.tasks.task(OPEN_TASK).unwrap();
    assert_eq!(
        task.phase,
        tasks::Phase::Held {
            was: tasks::Was::Active(tasks::Active::Due),
            why: tasks::Hold::Exceeded(exceeded),
            source: None,
        }
    );
    assert_eq!(task.tries, tasks::Tries::NONE);
}
