//! Entry-point checks for the reference journal and routes.
mod host_tools;
use crate::{Config, Domain, Event, Limits, Output, Released, Store, release, step, try_step};
use alloc::boxed::Box;
use jig_domain_accounts as accounts;
use jig_domain_core as core;
use jig_domain_host as host;
use jig_ops_domain_infrastructure as infrastructure;
use jig_ops_domain_observability as observability;
use skein_lib::{Duration, Env, JournalLimits, List, Queue, Time, Token, Wall};

fn fixture() -> (Domain, Env<Limits>) {
    fixture_with_commits(1)
}

fn fixture_with_commits(commits: u32) -> (Domain, Env<Limits>) {
    let mut core_limits = jig_core_world::world::limits().core;
    core_limits.fleet.engine_slots = 1;
    core_limits.tasks.hold_kinds = 3;
    let journal = JournalLimits { commits, writes: 20000, held: 20000, now: 128, release: 128 };
    let limits = Limits {
        core: core_limits,
        infrastructure: jig_ops_world::infrastructure_limits(),
        observability: jig_ops_world::limits(),
        host: host::Limits {
            slots: 1,
            accounts: 1,
            program_sources: 12,
            source_interval: Duration::from_secs(60),
            source_timeout: Duration::from_secs(10),
            charter_bytes: 65536,
            transcript_bytes: 1024,
            delivery_evidence_bytes: 0,
            turn_bytes: 1024,
            outcome_bytes: 512,
            detail_bytes: 64,
            held: 2,
            inbox: 4,
            event_bytes: 512,
            run_calls: 2,
            facts: 2,
            told: 2,
            turns: 2,
            turn_queue_bytes: 2048,
            piece_bytes: 4096,
        },
        charter: smith_charter::v4::CEILINGS,
        endpoints: 1,
        journal,
        watermark: crate::default_watermark(&journal),
        routes: 256,
    };
    let mut config = Config {
        agent_bounds: jig_ops_world::agent_bounds(),
        tools: jig_ops_world::protocol::tools(Duration::from_secs(30)),
        core: jig_core_world::world::config(17).core,
        triage_templates: Box::new([]),
        lead_templates: Box::new([]),
        backend: infrastructure::Backend::OperationIds,
        charter_endpoints: Box::new([jig_protocol_charter::EndpointName {
            number: 1,
            dialect: 1,
            name: Box::from(&b"fake"[..]),
        }]),
    };
    config.core.defaults.charters[0].hosts = jig_domain_fleet::Kinds::Engine;
    (
        Domain::new(config, &limits).expect("fixture core configuration"),
        Env { now: Time::ZERO, wall: Wall::EPOCH, limits },
    )
}

fn take(domain: &mut Domain, env: &Env<Limits>) -> Box<[Output]> {
    let mut out = Queue::with_capacity(256);
    release(domain, env, &mut out);
    let mut values = List::with_capacity(256);
    while let Some(value) = out.pop() {
        values.push(value).unwrap();
    }
    domain.reclaim();
    values.into_boxed()
}

fn name(task: u64) -> Event {
    Event::Infrastructure(infrastructure::Event::Names {
        project: 1,
        task,
        resources: Box::new([infrastructure::Resource::Service(jig_ops_world::infra_service())]),
    })
}

// Rewriting the same reliance row adds one commit without growing the
// connector's working set. Drain submissions, withholding every store answer.
fn fill_to_watermark(domain: &mut Domain, env: &Env<Limits>) {
    for _ in 0..=env.limits.watermark {
        if try_step(domain, env, name(1)).is_err() {
            return;
        }
        take(domain, env);
    }
    panic!("admission returns a pressure probe within watermark + 1 writing probes");
}

#[test]
fn admission_precedes_a_connectors_change_and_its_outputs_wait_for_the_commit() {
    const FIRST_COMMIT: u64 = 1;
    // The fixture watermark is 1 commit + 128 door outputs; retry adds one commit.
    const RETRY_COMMIT: u64 = 130;
    const SUBSCRIBER: u64 = 2;
    const ONCE: usize = 1;
    let (mut domain, env) = fixture();
    try_step(&mut domain, &env, name(1)).expect("first decision enters");
    let first = take(&mut domain, &env);
    match first.as_ref() {
        [Output::Commit { number: FIRST_COMMIT, .. }] => {}
        other => panic!("one commit: {other:?}"),
    }
    fill_to_watermark(&mut domain, &env);
    // An independent subscription starts new work, rather than injecting a
    // connector's Names continuation as the rejected probe.
    let event = Event::Observability(observability::Event::Subscribe {
        task: SUBSCRIBER,
        topic: observability::Topic::Alerts(jig_ops_world::service()),
        wake_at: 5,
        keep_at: 1,
    });
    let event = try_step(&mut domain, &env, event).expect_err("new work waits at the watermark");
    let Event::Observability(observability::Event::Subscribe { task, topic, .. }) = &event else {
        panic!("the caller retains its original subscription")
    };
    assert_eq!(*task, SUBSCRIBER);
    assert_eq!(*topic, observability::Topic::Alerts(jig_ops_world::service()));
    assert!(take(&mut domain, &env).is_empty(), "refusal changes no child and emits no output");
    try_step(&mut domain, &env, Event::Store(Store::Committed { number: FIRST_COMMIT, fill: 0 }))
        .expect("commit completion enters at the watermark");
    take(&mut domain, &env);
    try_step(&mut domain, &env, event).expect("the same owned input enters after drain");
    let retry = take(&mut domain, &env);
    let [Output::Commit { number, writes }] = retry.as_ref() else { panic!("retry commits before any system output") };
    assert_eq!(*number, RETRY_COMMIT);
    let mut subscriptions = 0_usize;
    for write in writes {
        if let crate::Write::Save(crate::Record::Observability(observability::Record::Subscription { task, .. })) =
            write
            && *task == SUBSCRIBER
        {
            subscriptions = subscriptions.checked_add(1).expect("one subscription record");
        }
    }
    assert_eq!(subscriptions, ONCE, "the refused event is admitted exactly once when retried");
    assert!(!writes.is_empty(), "the retried decision retains its writes");
}

fn connector_read(token: Token) -> Event {
    const READ_BYTES: u32 = 128;
    Event::Observability(observability::Event::Read {
        token,
        read: observability::Read::Logs {
            service: jig_ops_world::service(),
            window: observability::Window { from: 0, through: 1 },
            filter: Box::new([]),
            max_bytes: READ_BYTES,
        },
    })
}

#[test]
fn a_failed_commit_stops_the_reference_root_before_held_system_requests_leave() {
    let (mut domain, env) = fixture();
    step(
        &mut domain,
        &env,
        Event::Observability(observability::Event::Subscribe {
            task: 1,
            topic: observability::Topic::Alerts(jig_ops_world::service()),
            wake_at: 5,
            keep_at: 1,
        }),
    );
    let values = take(&mut domain, &env);
    let Output::Commit { number, .. } = &values[0] else { panic!("subscription committed") };
    let number = *number;
    assert_eq!(values.len(), 1, "the fact read follows the subscription commit");
    step(&mut domain, &env, Event::Store(Store::Failed { number }));
    match take(&mut domain, &env).as_ref() {
        [Output::Stop] => {}
        other => panic!("stop: {other:?}"),
    }
    assert!(take(&mut domain, &env).is_empty());
    // Even an always-taken read must stop before touching a child; routing it
    // would attempt to use the stopped journal's door.
    try_step(&mut domain, &env, connector_read(Token::new(92))).expect("completion class still enters");
    assert!(try_step(&mut domain, &env, name(2)).is_err(), "new work is handed back after stop");
    assert!(take(&mut domain, &env).is_empty(), "a stopped journal moves no child");
}

#[test]
fn a_connector_read_answer_uses_the_door_and_makes_no_commit() {
    const READ: Token = Token::new(91);
    const INITIAL_COMMIT: u64 = 1;
    const COMMIT: u64 = 2;
    let (mut domain, env) = fixture();
    // Persist the fixture's initially dirty deployment counters first.
    try_step(&mut domain, &env, name(1)).expect("initial deployment write");
    take(&mut domain, &env);
    try_step(&mut domain, &env, Event::Store(Store::Committed { number: INITIAL_COMMIT, fill: 0 }))
        .expect("initial deployment durability");
    take(&mut domain, &env);
    // Admit the request before the unrelated write fills the backlog.
    try_step(&mut domain, &env, connector_read(READ)).expect("admit read request");
    match take(&mut domain, &env).as_ref() {
        [Output::Observability(observability::SystemRequest::Read { .. })] => {}
        other => panic!("read request uses the door: {other:?}"),
    }
    try_step(&mut domain, &env, name(2)).expect("unrelated write enters");
    match take(&mut domain, &env).as_ref() {
        [Output::Commit { number: COMMIT, .. }] => {}
        other => panic!("unrelated pending commit: {other:?}"),
    }
    fill_to_watermark(&mut domain, &env);
    try_step(
        &mut domain,
        &env,
        Event::Observability(observability::Event::System(observability::SystemEvent::ReadDone {
            token: READ,
            bytes: Box::from(&b"log bytes"[..]),
        })),
    )
    .expect("the admitted read's completion enters under pressure");
    match take(&mut domain, &env).as_ref() {
        [Output::Read { owner, bytes }] => {
            assert_eq!(*owner, READ);
            assert_eq!(bytes.as_ref(), b"log bytes");
        }
        other => panic!("read answer uses the door without another commit: {other:?}"),
    }
}

#[test]
fn cold_restart_loads_every_child_before_opening_decisions() {
    let (mut original, env) = fixture();
    step(&mut original, &env, name(1));
    let mut header = None;
    for output in take(&mut original, &env) {
        match output {
            Output::Commit { mut writes, .. } => {
                for _ in 0..writes.len() {
                    match writes.pop().unwrap() {
                        crate::Write::Save(
                            record @ crate::Record::Core(core::Record::Core(core::CoreRecord::Deployment(_))),
                        ) => header = Some(record),
                        crate::Write::Wires
                        | crate::Write::Save(_)
                        | crate::Write::Erase(_)
                        | crate::Write::CopySettingsRevision(_) => {}
                    }
                }
            }
            Output::Switch
            | Output::Load { .. }
            | Output::ReadResult { .. }
            | Output::CoreLoad(_)
            | Output::Core(_)
            | Output::Now(_)
            | Output::Observability(_)
            | Output::Infrastructure(_)
            | Output::ApplicationDocument { .. }
            | Output::ApplicationRead { .. }
            | Output::Application { .. }
            | Output::IncidentAssociationRefused { .. }
            | Output::Read { .. }
            | Output::Agent(_)
            | Output::ToChild(_)
            | Output::ProjectMade { .. }
            | Output::Stop => panic!("one initial commit"),
        }
    }
    assert!(header.is_some());
    let (mut domain, _) = fixture();
    step(&mut domain, &env, Event::Restart { pending: false });
    let mut loads = List::with_capacity(16);
    for _ in 0..16_u32 {
        for output in take(&mut domain, &env) {
            match output {
                Output::Now(core::Now::Account(accounts::Request::Signing(accounts::Signing::Restored))) => {}
                Output::Now(core::Now::Account(accounts::Request::Component(accounts::ToComponent::Want {
                    account,
                }))) => {
                    assert_eq!(account, 1);
                }
                Output::Switch => step(&mut domain, &env, Event::Store(Store::Switched)),
                Output::Load { step: stage, .. } => {
                    loads.push(stage).unwrap();
                    if let Some(record) = header.take() {
                        step(&mut domain, &env, Event::Store(Store::Restore(record)));
                        assert!(take(&mut domain, &env).is_empty(), "header restore is an empty admitted decision");
                        for row in jig_core_world::fixtures::initial_settings_current(&domain.core.current) {
                            step(&mut domain, &env, Event::Store(Store::Restore(crate::Record::Core(row))));
                        }
                    }
                    step(&mut domain, &env, Event::Store(Store::Restored(stage)));
                }
                Output::ToChild(Released::Restart(stage)) => {
                    step(&mut domain, &env, Event::Store(Store::Restored(stage)));
                }
                Output::ToChild(value) => step(&mut domain, &env, Event::Released(value)),
                Output::Commit { number, .. } => {
                    step(&mut domain, &env, Event::Store(Store::Committed { number, fill: 0 }));
                }
                other @ (Output::ReadResult { .. }
                | Output::CoreLoad(_)
                | Output::Core(_)
                | Output::Now(_)
                | Output::Observability(_)
                | Output::Infrastructure(_)
                | Output::ApplicationDocument { .. }
                | Output::ApplicationRead { .. }
                | Output::Application { .. }
                | Output::IncidentAssociationRefused { .. }
                | Output::Read { .. }
                | Output::Agent(_)
                | Output::ProjectMade { .. }
                | Output::Stop) => panic!("empty restart has no other work: {other:?}"),
            }
        }
        if domain.ready() {
            break;
        }
    }
    assert_eq!(
        loads.as_slice(),
        [
            core::RestartStep::LoadCore,
            core::RestartStep::LoadInbox,
            core::RestartStep::RestoreConnector { connector: 1 },
            core::RestartStep::RestoreConnector { connector: 2 }
        ]
    );
    assert!(domain.ready());
}

#[test]
fn the_reference_root_reports_a_finite_sum_and_refuses_overflow() {
    let (_, env) = fixture();
    assert!(crate::worst_case(&env.limits).is_some());
    let mut limits = env.limits;
    limits.core.fleet.attempts = u32::MAX;
    assert!(crate::worst_case(&limits).is_none());
}

fn settle(domain: &mut Domain, env: &Env<Limits>) -> Box<[Output]> {
    let mut seen = List::with_capacity(4096);
    for _ in 0..64_u32 {
        let values = take(domain, env);
        if values.is_empty() && domain.quiescent() {
            break;
        }
        for value in values {
            match value {
                Output::Commit { number, .. } => step(domain, env, Event::Store(Store::Committed { number, fill: 0 })),
                Output::ToChild(value) => step(domain, env, Event::Released(value)),
                Output::Core(core::Held::NotesLoad { owner, .. }) => step(
                    domain,
                    env,
                    Event::Core(core::Event::Notes(jig_domain_notes::Event::Loaded {
                        owner,
                        rows: jig_domain_notes::Rows { records: List::with_capacity(env.limits.core.notes.load_rows) },
                        more: false,
                    })),
                ),
                other @ (Output::Switch
                | Output::Load { .. }
                | Output::ReadResult { .. }
                | Output::CoreLoad(_)
                | Output::Core(_)
                | Output::Now(_)
                | Output::Observability(_)
                | Output::Infrastructure(_)
                | Output::ApplicationDocument { .. }
                | Output::ApplicationRead { .. }
                | Output::Application { .. }
                | Output::IncidentAssociationRefused { .. }
                | Output::Read { .. }
                | Output::Agent(_)
                | Output::ProjectMade { .. }
                | Output::Stop) => seen.push(other).unwrap(),
            }
        }
    }
    seen.into_boxed()
}

fn restore_chat_account(domain: &mut Domain, env: &Env<Limits>) {
    let configuration = jig_core_world::world::config(17);
    for row in jig_core_world::fixtures::account_rows(&configuration.core.settings.llm_providers) {
        let jig_test_domain::Record::Core(core::Record::Accounts(record)) = row else { panic!("account fixture row") };
        step(domain, env, Event::Core(core::Event::Account(accounts::Event::Restore { record })));
        settle(domain, env);
    }
}

#[expect(clippy::too_many_lines, reason = "the held sign-in field extends the existing complete chat fixture")]
fn started_chat() -> (Domain, Env<Limits>, Token) {
    use jig_domain_fleet as fleet;
    use jig_domain_people as people;
    use jig_domain_tasks as tasks;
    let (mut domain, env) = fixture();
    restore_chat_account(&mut domain, &env);
    for row in jig_core_world::fixtures::seeded_project_rows(&jig_core_world::world::config(17).core) {
        step(&mut domain, &env, Event::Store(Store::Restore(crate::Record::Core(row))));
        settle(&mut domain, &env);
    }
    for event in [
        core::Event::People(people::Event::Roles { project: 1, holdings: Box::new([]) }),
        core::Event::People(people::Event::Restored),
        core::Event::Tasks(tasks::Event::Check),
        core::Event::Tasks(tasks::Event::Restored),
        core::Event::Period { project: 1, period: 1, budget: 1000, next: None },
        core::Event::Fleet(fleet::Event::Loaded),
        core::Event::Account(accounts::Event::Want),
        core::Event::Account(accounts::Event::Component(accounts::FromComponent::Granted {
            form: accounts::Form::Credential(accounts::Scheme::Bearer),
            account: 1,
            generation: 1,
            valid: Duration::from_secs(60),
        })),
    ] {
        step(&mut domain, &env, Event::Core(event));
        settle(&mut domain, &env);
    }
    step(
        &mut domain,
        &env,
        Event::Core(core::Event::SignIn {
            held: None,
            reply_to: skein_lib::ReplyTo::new(Token::new(101)),
            identity: people::Identity {
                key: people::IdentityKey { provider: 0, subject: 7_u64.to_be_bytes().into() },
                login: b"person".as_ref().into(),
                name: b"Person".as_ref().into(),
            },
        }),
    );
    let signed = settle(&mut domain, &env);
    let mut sign_in = None;
    for output in signed {
        match output {
            Output::Core(core::Held::PeopleReply {
                reply: people::Reply::SignedIn { sign_in: session, .. }, ..
            }) => sign_in = Some(session),
            Output::Commit { .. }
            | Output::Switch
            | Output::Load { .. }
            | Output::ReadResult { .. }
            | Output::CoreLoad(_)
            | Output::Core(_)
            | Output::Now(_)
            | Output::Observability(_)
            | Output::Infrastructure(_)
            | Output::ApplicationDocument { .. }
            | Output::ApplicationRead { .. }
            | Output::Application { .. }
            | Output::IncidentAssociationRefused { .. }
            | Output::Read { .. }
            | Output::Agent(_)
            | Output::ToChild(_)
            | Output::ProjectMade { .. }
            | Output::Stop => {}
        }
    }
    let sign_in = sign_in.expect("signed in");
    step(
        &mut domain,
        &env,
        Event::Core(core::Event::People(people::Event::Ask {
            models: Box::new([]),
            reply_to: skein_lib::ReplyTo::new(Token::new(102)),
            sign_in,
            key: [2; 16],
            ask: chat_request(&env.limits.core, b"find the incident"),
        })),
    );
    let outputs = settle(&mut domain, &env);
    let mut started = None;
    for output in &outputs {
        match output {
            Output::Agent(host::ToAgent::Start { owner, .. }) => started = Some(*owner),
            Output::Agent(_)
            | Output::Commit { .. }
            | Output::Switch
            | Output::Load { .. }
            | Output::ReadResult { .. }
            | Output::CoreLoad(_)
            | Output::Core(_)
            | Output::Now(_)
            | Output::Observability(_)
            | Output::Infrastructure(_)
            | Output::ApplicationDocument { .. }
            | Output::ApplicationRead { .. }
            | Output::Application { .. }
            | Output::IncidentAssociationRefused { .. }
            | Output::Read { .. }
            | Output::ToChild(_)
            | Output::ProjectMade { .. }
            | Output::Stop => {}
        }
    }
    let owner = started.expect("the assigned hub run asked its agent to start");
    step(&mut domain, &env, Event::Agent(host::FromAgent::Started { owner, agent: owner }));
    settle(&mut domain, &env);
    (domain, env, owner)
}

/// A relayed call name in smith's big-endian channel record.
fn call_name(activation: u64, completion: u32, position: u32) -> Box<[u8]> {
    let mut bytes = List::with_capacity(16);
    for byte in activation.to_be_bytes().into_iter().chain(completion.to_be_bytes()).chain(position.to_be_bytes()) {
        bytes.push(byte).unwrap();
    }
    bytes.into_boxed()
}

#[test]
fn a_committed_chat_assignment_starts_its_agent_through_the_engine_hub() {
    started_chat();
}

#[test]
#[expect(clippy::too_many_lines, reason = "a complete journal scenario also checks admitted wrong-scope refusal")]
fn connector_documents_and_reads_share_the_last_connector_commit_barrier() {
    use jig_domain_people as people;
    use skein_lib::ReplyTo;
    let (mut domain, env) = fixture_with_commits(2);
    let identity = people::Identity {
        key: people::IdentityKey { provider: 0, subject: Box::from(*b"party") },
        login: Box::from(*b"party"),
        name: Box::from(*b"Party"),
    };
    for record in [
        people::Stored::Person { number: 1, identity },
        people::Stored::SignIn { number: 10, person: 1, expires: Wall::from_nanos(Duration::from_secs(60).as_nanos()) },
        people::Stored::Roles {
            project: 1,
            holdings: Box::new([people::Holding { person: 1, role: people::Role::Observer }]),
        },
    ] {
        step(&mut domain, &env, Event::Core(core::Event::People(people::Event::Restore { record })));
        let initial = settle(&mut domain, &env);
        assert!(initial.is_empty(), "restore outputs: {initial:?}");
    }
    step(&mut domain, &env, Event::Core(core::Event::People(people::Event::Restored)));
    assert!(take(&mut domain, &env).is_empty());
    let service = jig_ops_world::service();
    step(
        &mut domain,
        &env,
        Event::Observability(observability::Event::System(observability::SystemEvent::Fact {
            service: service.clone(),
            fact: observability::Fact {
                observed: 0,
                healthy_replicas: 2,
                errors: 0,
                error_rate_percent: 0,
                load_percent: 10,
                load: Box::new([]),
            },
        })),
    );
    assert!(take(&mut domain, &env).is_empty());
    step(
        &mut domain,
        &env,
        Event::Observability(observability::Event::StartWatch {
            task: 7,
            project: 1,
            services: Box::new([service.clone()]),
            template: 1,
        }),
    );
    let pending = take(&mut domain, &env);
    let [Output::Commit { number, .. }] = pending.as_ref() else {
        panic!("watch ownership is committed first: {pending:?}")
    };
    let number = *number;
    let subject = crate::ClientSubject::Observability(observability::ClientSubject::Service {
        project: 1,
        service: service.clone(),
    });
    step(
        &mut domain,
        &env,
        Event::WatchApplication {
            watcher: Token::new(501),
            sign_in: 10,
            scope: jig_domain_people::ApplicationScope::Project(1),
            subject: subject.clone(),
            window: core::documents::Window { rows: 1, after: None },
        },
    );
    assert!(take(&mut domain, &env).is_empty(), "assembled document stays behind the connector commit");
    step(
        &mut domain,
        &env,
        Event::ReadApplication {
            reply: ReplyTo::new(Token::new(502)),
            sign_in: 10,
            scope: jig_domain_people::ApplicationScope::Project(1),
            subject,
        },
    );
    assert!(take(&mut domain, &env).is_empty(), "assembled read stays behind the same commit");
    step(&mut domain, &env, Event::Store(Store::Committed { number, fill: 0 }));
    let released = take(&mut domain, &env);
    let mut documents = 0_u32;
    let mut answers = 0_u32;
    for output in released {
        match output {
            Output::ApplicationDocument { watcher, document } => {
                assert_eq!(watcher, Token::new(501));
                assert_service_document(document);
                documents = documents.checked_add(1).unwrap();
            }
            Output::ApplicationRead { reply, answer } => {
                assert_eq!(reply.into_token(), Token::new(502));
                assert_service_document(answer.unwrap());
                answers = answers.checked_add(1).unwrap();
            }
            Output::Commit { .. }
            | Output::Switch
            | Output::Load { .. }
            | Output::ReadResult { .. }
            | Output::CoreLoad(_)
            | Output::Core(_)
            | Output::Now(_)
            | Output::Observability(_)
            | Output::Infrastructure(_)
            | Output::Application { .. }
            | Output::IncidentAssociationRefused { .. }
            | Output::Read { .. }
            | Output::Agent(_)
            | Output::ToChild(_)
            | Output::ProjectMade { .. }
            | Output::Stop => {}
        }
    }
    assert_eq!((documents, answers), (1, 1));
    step(
        &mut domain,
        &env,
        Event::WatchApplication {
            watcher: Token::new(503),
            sign_in: 10,
            scope: jig_domain_people::ApplicationScope::Project(1),
            subject: crate::ClientSubject::Observability(observability::ClientSubject::Service { project: 2, service }),
            window: core::documents::Window { rows: 1, after: None },
        },
    );
    let refused = take(&mut domain, &env);
    let mut ended = false;
    for output in refused {
        if let Output::Core(core::Held::WatchEnded { watcher, why }) = output {
            assert_eq!(watcher, Token::new(503));
            assert_eq!(why, jig_domain_views::End::Gone);
            ended = true;
        } else if let Output::ApplicationDocument { .. } = output {
            panic!("the wrong subject scope produces no document");
        }
    }
    assert!(ended, "role admission does not grant the wrong subject scope");
}

fn assert_service_document(document: crate::ClientDocument) {
    let crate::ClientDocument::Observability(value) = document else { panic!("observability owns the document") };
    let observability::ClientDocument::Service { fact, .. } = *value else { panic!("service document") };
    assert_eq!(fact.healthy_replicas, 2);
}

#[test]
fn an_unknown_connector_refuses_the_restart_without_loading_a_child() {
    for connector in [0, 3, u16::MAX] {
        let (mut domain, env) = fixture();
        step(&mut domain, &env, Event::Released(Released::Restart(core::RestartStep::RestoreConnector { connector })));
        match take(&mut domain, &env).as_ref() {
            [Output::Stop] => {}
            other => panic!("unknown owner stops without a child load: {other:?}"),
        }
        assert_eq!(domain.core.restart_failure(), Some(core::RestartStep::LoadCore));
        assert!(!domain.core.restart_ready(), "unknown owner cannot open decisions");
    }
}

fn triage_fields(alerts: &[u8]) -> Box<[jig_domain_tasks::ResultField]> {
    Box::new([
        jig_domain_tasks::ResultField { name: Box::from(&b"alerts"[..]), value: Box::from(alerts) },
        jig_domain_tasks::ResultField { name: Box::from(&b"environment"[..]), value: Box::from(&b"production"[..]) },
        jig_domain_tasks::ResultField { name: Box::from(&b"service"[..]), value: Box::from(&b"checkout"[..]) },
        jig_domain_tasks::ResultField { name: Box::from(&b"severity"[..]), value: Box::from(&b"3"[..]) },
    ])
}

#[test]
fn a_closed_verdict_translates_only_explicit_machine_fields_to_the_watch() {
    let (_, env) = fixture();
    let limits = &env.limits.observability;
    assert_eq!(
        crate::translate::watch_result(
            100,
            &jig_domain_tasks::ResultKind::Verdict { code: 1, fields: Box::new([]) },
            limits
        ),
        observability::WatchMessage::Triage { child: 100, verdict: observability::TriageVerdict::Noise }
    );
    assert_eq!(
        crate::translate::watch_result(
            100,
            &jig_domain_tasks::ResultKind::Verdict { code: 2, fields: triage_fields(b"12 13") },
            limits
        ),
        observability::WatchMessage::Triage {
            child: 100,
            verdict: observability::TriageVerdict::Incident {
                alerts: Box::new([12, 13]),
                service: observability::Service::new(Box::from(&b"production"[..]), Box::from(&b"checkout"[..])),
                severity: 3,
            }
        }
    );
}

#[test]
fn malformed_machine_fields_are_refused_without_reading_an_explanation() {
    let (_, env) = fixture();
    let limits = &env.limits.observability;
    for alerts in [b"".as_slice(), b"12 ", b"12 report 13", b"18446744073709551616"] {
        assert_eq!(
            crate::translate::watch_result(
                100,
                &jig_domain_tasks::ResultKind::Verdict { code: 2, fields: triage_fields(alerts) },
                limits
            ),
            observability::WatchMessage::InvalidTriage { child: 100 }
        );
    }
    let mut fields = triage_fields(b"12");
    fields[1].name = Box::from(&b"alerts"[..]);
    assert_eq!(
        crate::translate::watch_result(100, &jig_domain_tasks::ResultKind::Verdict { code: 2, fields }, limits),
        observability::WatchMessage::InvalidTriage { child: 100 }
    );
    assert_eq!(
        crate::translate::watch_result(
            100,
            &jig_domain_tasks::ResultKind::Verdict { code: 2, fields: Box::new([]) },
            limits
        ),
        observability::WatchMessage::InvalidTriage { child: 100 }
    );
}

fn environment_spec() -> jig_domain_tasks::Spec {
    use jig_domain_tasks::Parameter;
    jig_domain_tasks::Spec {
        words: Box::from(b"staging for payments, a week".as_slice()),
        parameters: Box::new([
            Parameter::Bytes { name: 1, value: Box::from(b"staging".as_slice()) },
            Parameter::Bytes { name: 2, value: Box::from(b"payments".as_slice()) },
            Parameter::Number { name: 3, value: 604_800 },
            Parameter::Number { name: 4, value: 40 },
            Parameter::Number { name: 5, value: 7200 },
        ]),
        inputs: Box::new([]),
    }
}

#[test]
fn provision_preserves_opaque_environment_parameters_and_distinct_pool_hold() {
    use jig_domain_tasks::{Executor, HoldKind, Holding, Name, Taken};
    let mut spec = environment_spec();
    let environment =
        infrastructure::Environment::new(Box::from(b"staging".as_slice()), Box::from(b"payments".as_slice()));
    assert_eq!(
        crate::translate::procedure(infrastructure::PROVISION_CODE, &spec, [1; 16]),
        Some(infrastructure::Procedure::Provision {
            environment: environment.clone(),
            until: 604_800,
            price: 40,
            deadline: 7200,
        })
    );
    assert_eq!(
        crate::translate::holdings(
            2,
            Executor::Procedure { connector: 2, code: infrastructure::PROVISION_CODE },
            &spec
        ),
        Some(Box::from([
            Holding::Write {
                resource: Name { connector: 2, path: infrastructure::Resource::Environment(environment).segments() },
                kind: 2
            },
            Holding::Slot {
                pool: Name {
                    connector: 2,
                    path: infrastructure::Resource::Pool(infrastructure::Pool(Box::from(b"staging".as_slice())))
                        .segments()
                },
                kind: 3
            },
        ]))
    );
    let kinds = crate::translate::hold_kinds(2);
    assert_eq!(
        kinds.as_ref(),
        &[
            jig_domain_tasks::Kind { connector: 2, kind: 1, hold: HoldKind::Exclusive { taken: Taken::Waits } },
            jig_domain_tasks::Kind { connector: 2, kind: 2, hold: HoldKind::Exclusive { taken: Taken::Waits } },
            jig_domain_tasks::Kind { connector: 2, kind: 3, hold: HoldKind::Pooled { taken: Taken::Waits } },
        ]
    );
    spec.parameters = spec.parameters[..4].into();
    assert_eq!(
        crate::translate::procedure(infrastructure::PROVISION_CODE, &spec, [1; 16]),
        None,
        "a missing deadline is not manufactured"
    );
    spec.parameters[1] = jig_domain_tasks::Parameter::Number { name: 2, value: 99 };
    assert_eq!(
        crate::translate::holdings(
            2,
            Executor::Procedure { connector: 2, code: infrastructure::PROVISION_CODE },
            &spec
        ),
        None,
        "numeric service identities are not substituted for an environment name"
    );
}

#[test]
fn adding_provision_preserves_scale_and_remediate_codes_and_service_hold() {
    use jig_domain_tasks::{Executor, Holding, Name};
    let mut spec = environment_spec();
    spec.parameters = spec.parameters[..4].into();
    let service = infrastructure::Service::new(Box::from(b"staging".as_slice()), Box::from(b"payments".as_slice()));
    assert_eq!(
        crate::translate::procedure(1, &spec, [1; 16]),
        Some(infrastructure::Procedure::Scale { service: service.clone(), replicas: 604_800, deadline: 40 })
    );
    assert_eq!(
        crate::translate::procedure(2, &spec, [1; 16]),
        Some(infrastructure::Procedure::Remediate { service: service.clone(), operation: 604_800, deadline: 40 })
    );
    for code in [1, 2] {
        assert_eq!(
            crate::translate::holdings(2, Executor::Procedure { connector: 2, code }, &spec),
            Some(Box::from([Holding::Write {
                resource: Name { connector: 2, path: infrastructure::Resource::Service(service.clone()).segments() },
                kind: 1
            },]))
        );
    }
    assert_eq!(crate::translate::procedure(99, &spec, [1; 16]), None);
}

fn chat_request(limits: &core::Limits, words: &[u8]) -> jig_domain_people::Ask {
    let config = jig_core_world::world::config(17).core;
    let members = jig_core_world::fixtures::Creating::new(&config, limits.tasks.tree_tasks);
    jig_core_world::fixtures::create_chat(&members, 1, 0, words.into())
}

#[test]
fn either_start_refusal_cause_assembles_an_invalid_end() {
    for cause in [host::StartCause::Grants, host::StartCause::Other] {
        let (mut domain, _env) = fixture();
        let run = Token::new(1);
        let attempt = Token::new(7);
        let payload = Token::new(99);
        domain
            .payloads
            .insert(
                payload,
                crate::boundary::Payload::Answer {
                    task: run.raw(),
                    attempt: attempt.raw(),
                    spent: 0,
                    ending: host::Ending::Refused(host::Refusal::Invalid(host::Invalid::Start {
                        cause,
                        why: Box::from(&b"start refused"[..]),
                    })),
                    read: None,
                    refused: Box::new([]),
                },
            )
            .unwrap();
        let mut work = Queue::with_capacity(1);
        crate::assemble::answer_payload(&mut domain, run, attempt, payload, &mut work);
        let Some(crate::domain::Work::Core(core::Event::AnswerPayload {
            run: answered_run,
            attempt: answered_attempt,
            payload: answered_payload,
            end,
            ..
        })) = work.pop()
        else {
            panic!("one assembled answer")
        };
        assert_eq!((answered_run, answered_attempt, answered_payload), (run, attempt, payload));
        assert_eq!(end, jig_domain_tasks::End::Failed(jig_domain_tasks::Class::Invalid));
        assert!(work.is_empty());
    }
}

#[test]
fn a_loaded_transcript_passes_its_protocol_rendered_text_to_the_brief() {
    let (mut domain, env) = fixture();
    domain.core.begin_transcript(1, 1);
    domain.core.transcripts.get_mut(&1).unwrap().window = Some((1, 1));
    step(
        &mut domain,
        &env,
        Event::Store(Store::Transcript {
            task: 1,
            rows: Box::new([(
                core::TurnRecord {
                    task: 1,
                    attempt: 1,
                    turn: 1,
                    spent: 0,
                    read: None,
                    opens_window: true,
                    at: Wall::EPOCH,
                    transcript: Box::from(&[0xff, 0x80][..]),
                },
                Some(Box::from(&b"assistant: conversation before restart\n"[..])),
            )]),
            done: false,
        }),
    );
    assert_eq!(
        core::read_brief_part(
            &domain.core,
            core::BriefTextLimits { parts: 1, read_bytes: 128, brief_bytes: 128 },
            1,
            core::BriefPart::TranscriptTail
        )
        .expect("ops preparation tail")
        .as_ref(),
        b"assistant: conversation before restart\n"
    );
}

#[test]
fn stopping_ignores_the_starts_mark_continuations() {
    let (mut domain, env) = fixture();
    step(&mut domain, &env, name(1));
    settle(&mut domain, &env);
    step(&mut domain, &env, Event::Core(core::Event::Stopping));
    settle(&mut domain, &env);
    for event in [Event::Store(Store::Switched), Event::Released(Released::RestartDone(core::RestartStep::Switch))] {
        step(&mut domain, &env, event);
        assert!(take(&mut domain, &env).is_empty());
        assert_eq!(domain.core.restart_failure(), None);
    }
}
