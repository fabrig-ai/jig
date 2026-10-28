use alloc::boxed::Box;
use skein_lib::{Env, List, Queue, Time, Wall};

use crate::{
    Adoption, Class, Classed, Config, Domain, Event, Hold, HoldMode, Limits, MAX_OUT, Origin, Path, PoolSpec, Record,
    Request, ResourceRole, ResourceSpec, SystemEvent, TopicSpec, step,
};

const PROJECT_DOCUMENT: u64 = 800;
const DEPLOYMENT_DOCUMENT: u64 = 801;
const WINDOW_ROWS: [u32; 3] = [0, 1, 2];

const LIMITS: Limits = Limits {
    tasks: 2,
    adoptions: 3,
    subscriptions: 3,
    pools: 1,
    resources: 2,
    topics: 1,
    kinds: 0,
    requirements: 1,
    procedures: 1,
    actions_per_procedure: 5,
    facts: 2,
    judges: 2,
    values: 2,
    value_bytes: 18,
    staged: 0,
    entries: 0,
    made: 0,
    resources_per_effect: 0,
    max_attempts: 1,
    write_lifetime: skein_lib::Duration::from_secs(1),
    clock_margin: skein_lib::Duration::from_secs(1),
    resources_per_task: 2,
    subscribers_per_topic: 2,
    lost_per_pool: 2,
    path_segments: 3,
    segment_bytes: 16,
};

fn path(parts: &[&[u8]]) -> Path {
    let mut segments = List::with_capacity(3);
    for part in parts {
        segments.push(Box::from(*part)).expect("fixture path fits");
    }
    Path::new(segments, LIMITS.segment_bytes).expect("fixture path has segments")
}

fn prefix() -> Path {
    path(&[b"unit"])
}

fn service() -> Path {
    path(&[b"unit", b"service"])
}

fn pool() -> Path {
    path(&[b"unit", b"pool"])
}

fn domain() -> Domain {
    domain_with_limits(&LIMITS)
}

fn domain_with_limits(limits: &Limits) -> Domain {
    Domain::new(
        Config {
            prefix: prefix(),
            resources: Box::from([
                ResourceSpec {
                    credential: None,
                    path: service(),
                    hold: Hold::Exclusive { mode: HoldMode::Wait },
                    writable: true,
                },
                ResourceSpec {
                    credential: None,
                    path: pool(),
                    hold: Hold::Pooled { mode: HoldMode::Refuse },
                    writable: true,
                },
            ]),
            pools: Box::from([PoolSpec { path: pool(), slots: 2 }]),
            topics: Box::from([TopicSpec { topic: 7, offered: true, resources: Box::new([service()]) }]),
            deployment: [1; 16],
            kinds: Box::from([]),
            requirements: Box::from([crate::RequirementSpec {
                number: 4,
                guarded: true,
                freshness: skein_lib::Duration::from_nanos(5),
            }]),
            procedures: Box::from([crate::ProcedureSpec {
                number: 3,
                actions: Box::from([
                    crate::ProcedureAction::Effect { kind: 1, purpose: 9, target: 3 },
                    crate::ProcedureAction::Delegate { kinds: Box::from([2]) },
                    crate::ProcedureAction::Propose { spend: 7 },
                    crate::ProcedureAction::Wait { state: 3 },
                    crate::ProcedureAction::Finish { result: crate::ProcedureResult::Local { code: 8 } },
                ]),
                max_steps: 5,
                stall: skein_lib::Duration::from_nanos(10),
            }]),
        },
        limits,
    )
}

fn run(domain: &mut Domain, event: Event) -> Box<[Request]> {
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: LIMITS };
    let mut out = Queue::with_capacity(MAX_OUT);
    step(domain, &env, event, &mut out);
    let mut requests = List::with_capacity(MAX_OUT);
    while let Some(request) = out.pop() {
        requests.push(request).expect("the request came from this queue");
    }
    requests.into_boxed()
}

fn run_at(domain: &mut Domain, wall: u64, event: Event) -> Box<[Request]> {
    let env = Env { now: Time::ZERO, wall: Wall::from_nanos(wall), limits: LIMITS };
    let mut out = Queue::with_capacity(MAX_OUT);
    step(domain, &env, event, &mut out);
    let mut requests = List::with_capacity(MAX_OUT);
    while let Some(request) = out.pop() {
        requests.push(request).expect("the request came from this queue");
    }
    requests.into_boxed()
}

fn procedure_event(signal: crate::ProcedureSignal) -> Event {
    Event::Procedure {
        pending: Box::new([]),
        added: Box::new([]),
        project: 1,
        task: 9,
        number: 3,
        resource: service(),
        signal,
    }
}

#[test]
fn verdict_waits_for_a_fresh_fact_and_reanswers_for_the_exact_state() {
    let mut connector = domain();
    let token = skein_lib::Token::new(5);
    let old = crate::Fact { state: Some(3), observed: Wall::from_nanos(10), pending: false };
    drop(run_at(
        &mut connector,
        10,
        Event::System(SystemEvent::Fact { resource: service(), fact: old, origin: Origin::Own }),
    ));
    assert_eq!(
        run_at(&mut connector, 20, Event::Judge { token, requirement: 4, resources: Box::from([service()]), state: 3 }),
        Box::from([
            Request::Verdict { token, verdict: crate::Verdict::Wait },
            Request::System(crate::SystemRequest::ReadFact { resource: service(), observed: Wall::from_nanos(20) }),
        ])
    );
    let fresh = crate::Fact { state: Some(3), observed: Wall::from_nanos(20), pending: false };
    assert_eq!(
        run_at(
            &mut connector,
            20,
            Event::System(SystemEvent::Fact { resource: service(), fact: fresh, origin: Origin::Own })
        ),
        Box::from([
            Request::Changed { resource: service() },
            Request::Verdict {
                token,
                verdict: crate::Verdict::Met { state: 3, observed: Wall::from_nanos(20), guarded: true },
            },
        ])
    );
    assert_eq!(
        run_at(&mut connector, 20, Event::Judge { token, requirement: 4, resources: Box::from([service()]), state: 4 }),
        Box::from([Request::Verdict { token, verdict: crate::Verdict::Refuse { actual: Some(3) } }])
    );
}

#[test]
fn procedure_commits_each_decision_once_and_waits_on_a_fact_not_a_wake() {
    let mut connector = domain();
    let first = run_at(&mut connector, 10, procedure_event(crate::ProcedureSignal::Activate));
    assert_eq!(first.len(), 2);
    assert_eq!(
        first.get(1),
        Some(&Request::Step {
            task: 9,
            decision: crate::StepDecision::Effect(crate::Effect {
                kind: 1,
                resources: Box::from([service()]),
                purpose: 9,
                condition: None,
                target: 3,
                state: 3,
            }),
        })
    );
    assert!(run_at(&mut connector, 10, procedure_event(crate::ProcedureSignal::Message)).is_empty());
    let delegated = run_at(&mut connector, 11, procedure_event(crate::ProcedureSignal::Settled));
    assert_eq!(
        delegated.get(1),
        Some(&Request::Step { task: 9, decision: crate::StepDecision::Delegate { kinds: Box::from([2]) } })
    );
    let proposed = run_at(&mut connector, 12, procedure_event(crate::ProcedureSignal::DelegateDone));
    assert_eq!(proposed.get(1), Some(&Request::Step { task: 9, decision: crate::StepDecision::Propose { spend: 7 } }));
    let waiting = run_at(&mut connector, 13, procedure_event(crate::ProcedureSignal::ProposalDone));
    assert_eq!(
        waiting.get(1),
        Some(&Request::Step { task: 9, decision: crate::StepDecision::Wait { deadline: Wall::from_nanos(23) } })
    );
    assert_eq!(
        run_at(&mut connector, 14, procedure_event(crate::ProcedureSignal::Message)),
        Box::from([Request::Step { task: 9, decision: crate::StepDecision::Wait { deadline: Wall::from_nanos(23) } }])
    );
    let fact = crate::Fact { state: Some(3), observed: Wall::from_nanos(15), pending: false };
    drop(run_at(
        &mut connector,
        15,
        Event::System(SystemEvent::Fact { resource: service(), fact, origin: Origin::Own }),
    ));
    let finished = run_at(&mut connector, 15, procedure_event(crate::ProcedureSignal::Message));
    assert_eq!(finished.len(), 3);
    assert_eq!(
        finished.get(2),
        Some(&Request::Step {
            task: 9,
            decision: crate::StepDecision::Finish { result: crate::ProcedureResult::Local { code: 8 } }
        })
    );
    assert!(run_at(&mut connector, 16, procedure_event(crate::ProcedureSignal::Message)).is_empty());
}

#[test]
fn sections_and_workspace_items_have_one_handover_and_drop_with_their_token() {
    let mut connector = domain();
    drop(run(&mut connector, Event::Adopt { project: 1, resource: service(), role: ResourceRole::Owned }));
    drop(run(&mut connector, Event::Names { task: 4, project: 1, resources: Box::from([service()]) }));
    let token = skein_lib::Token::new(1);
    assert_eq!(
        run(&mut connector, Event::Read { token, read: crate::Read { resource: service(), size: 8 } }),
        Box::from([Request::Answer { token, bytes: Box::from(0_u64.to_be_bytes()) }])
    );
    assert_eq!(
        run(&mut connector, Event::Gather { token, task: 4, budget: 18 }),
        Box::from([Request::Ready { token, size: 18 }])
    );
    assert_eq!(run(&mut connector, Event::Cut { token, size: 10 }), Box::from([Request::Ready { token, size: 10 }]));
    assert_eq!(run(&mut connector, Event::HandOver { token }).len(), 1);
    assert!(run(&mut connector, Event::HandOver { token }).is_empty());
    assert_eq!(run(&mut connector, Event::Items { token, task: 4 }), Box::from([Request::Ready { token, size: 1 }]));
    assert_eq!(
        run(&mut connector, Event::HandOver { token }),
        Box::from([Request::Workspace {
            token,
            items: Box::from([crate::Item { credential: None, resource: service(), writable: true, state: None }]),
        }])
    );
    drop(run(&mut connector, Event::Gather { token, task: 4, budget: 8 }));
    assert!(run(&mut connector, Event::Drop { token }).is_empty());
    assert!(run(&mut connector, Event::HandOver { token }).is_empty());
}

#[test]
fn owned_drift_is_reported_and_restart_refreshes_before_procedures_resume() {
    let mut connector = domain();
    drop(run(&mut connector, Event::Adopt { project: 1, resource: service(), role: ResourceRole::Owned }));
    drop(run(&mut connector, Event::Names { task: 4, project: 1, resources: Box::from([service()]) }));
    let old = crate::Fact { state: Some(1), observed: Wall::from_nanos(3), pending: false };
    drop(run_at(
        &mut connector,
        3,
        Event::System(SystemEvent::Fact { resource: service(), fact: old, origin: Origin::Own }),
    ));
    let changed = crate::Fact { state: Some(2), observed: Wall::from_nanos(4), pending: false };
    assert_eq!(
        run_at(
            &mut connector,
            4,
            Event::System(SystemEvent::Fact { resource: service(), fact: changed, origin: Origin::Other })
        ),
        Box::from([
            Request::Changed { resource: service() },
            Request::DriftResource { tasks: Box::from([4]), resource: service() },
        ])
    );
    assert!(run_at(&mut connector, 5, Event::Restart(crate::RestartStep::Records)).is_empty());
    assert_eq!(
        run_at(&mut connector, 5, Event::Restart(crate::RestartStep::FreshRead { resource: service() })),
        Box::from([Request::System(crate::SystemRequest::ReadFact {
            resource: service(),
            observed: Wall::from_nanos(5)
        })])
    );
    assert_eq!(
        run_at(&mut connector, 5, Event::Restart(crate::RestartStep::SettleOutbox)),
        Box::from([Request::RestartDone])
    );
}

#[test]
fn restored_procedure_waits_for_its_outstanding_effect_answer() {
    let mut original = domain();
    let first = run_at(&mut original, 10, procedure_event(crate::ProcedureSignal::Activate));
    let Some(Request::Save { record }) = first.first() else {
        panic!("procedure state was saved before its effect");
    };
    let mut cold = domain();
    assert!(run_at(&mut cold, 11, Event::Restore { record: record.clone() }).is_empty());
    assert!(run_at(&mut cold, 11, procedure_event(crate::ProcedureSignal::Message)).is_empty());
    let resumed = run_at(&mut cold, 12, procedure_event(crate::ProcedureSignal::Settled));
    assert_eq!(
        resumed.get(1),
        Some(&Request::Step { task: 9, decision: crate::StepDecision::Delegate { kinds: Box::from([2]) } })
    );
}

#[test]
fn adoption_and_names_report_roles_and_holds_from_the_connector() {
    let mut connector = domain();
    assert_eq!(
        run(&mut connector, Event::Names { task: 4, project: 1, resources: Box::from([service()]) }),
        Box::from([Request::Unknown { task: 4, resource: service() }])
    );
    let adopted = run(&mut connector, Event::Adopt { project: 1, resource: service(), role: ResourceRole::Owned });
    assert_eq!(adopted.len(), 3);
    assert_eq!(adopted[2], Request::Adopted { project: 1, resource: service(), result: Adoption::Added });
    let named = run(&mut connector, Event::Names { task: 4, project: 1, resources: Box::from([service()]) });
    assert_eq!(named.len(), 2);
    assert_eq!(
        named[0],
        Request::Save { record: Record::Task { task: 4, project: 1, resources: Box::from([service()]) } }
    );
    assert_eq!(
        named[1],
        Request::Named {
            project: 1,
            task: 4,
            resources: Box::from([crate::Named {
                path: service(),
                role: ResourceRole::Owned,
                hold: Hold::Exclusive { mode: HoldMode::Wait },
            }]),
        }
    );
    assert_eq!(connector.live_tasks(), 1);
    assert_eq!(
        run(&mut connector, Event::Unnamed { task: 4 }),
        Box::from([Request::Erase { key: crate::RecordKey::Task(4) }])
    );
    assert_eq!(connector.live_tasks(), 0);
}

#[test]
fn context_resources_cannot_be_upgraded_to_a_write_role() {
    let mut config = Config {
        prefix: prefix(),
        resources: Box::from([ResourceSpec { credential: None, path: service(), hold: Hold::None, writable: false }]),
        pools: Box::from([]),
        topics: Box::from([]),
        deployment: [1; 16],
        kinds: Box::from([]),
        requirements: Box::from([]),
        procedures: Box::from([]),
    };
    let mut connector = Domain::new(config.clone(), &LIMITS);
    assert_eq!(
        run(&mut connector, Event::Adopt { project: 1, resource: service(), role: ResourceRole::Owned }),
        Box::from([Request::Adopted { project: 1, resource: service(), result: Adoption::Refused }])
    );
    assert_eq!(
        run(&mut connector, Event::Adopt { project: 1, resource: service(), role: ResourceRole::Context }).len(),
        3
    );
    config.resources = Box::from([]);
    let mut other = Domain::new(config, &LIMITS);
    assert_eq!(
        run(&mut other, Event::Adopt { project: 1, resource: service(), role: ResourceRole::Context }),
        Box::from([Request::Adopted { project: 1, resource: service(), result: Adoption::Unknown }])
    );
}

#[test]
fn shrinking_a_pool_reports_slots_and_lost_allocations_without_retracting_holds() {
    let mut connector = domain();
    assert_eq!(connector.slots(&pool()), Some(2));
    let output =
        run(&mut connector, Event::System(SystemEvent::Pool { path: pool(), slots: 1, lost: Box::from([9, 10]) }));
    assert_eq!(output.len(), 3);
    assert_eq!(output[0], Request::Save { record: Record::Pool { path: pool(), slots: 1 } });
    assert_eq!(output[1], Request::Slots { pool: pool(), slots: 1 });
    assert_eq!(output[2], Request::Drift { pool: pool(), tasks: Box::from([9, 10]) });
    assert_eq!(connector.slots(&pool()), Some(1));
}

#[test]
fn topic_news_is_classified_per_subscriber_and_own_echoes_are_dropped() {
    let mut connector = domain();
    run(&mut connector, Event::Subscribe { task: 1, topic: 7, wake_at: 5, keep_at: 2 });
    run(&mut connector, Event::Subscribe { task: 2, topic: 7, wake_at: 8, keep_at: 4 });
    assert!(
        run(&mut connector, Event::System(SystemEvent::News { topic: 7, importance: 9, origin: Origin::Own }))
            .is_empty()
    );
    assert_eq!(
        run(&mut connector, Event::System(SystemEvent::News { topic: 7, importance: 6, origin: Origin::Other })),
        Box::from([Request::News {
            topic: 7,
            subscribers: Box::from([Classed { task: 1, class: Class::Wake }, Classed { task: 2, class: Class::Keep }]),
        }])
    );
    assert!(
        run(&mut connector, Event::System(SystemEvent::News { topic: 7, importance: 1, origin: Origin::Other }))
            .is_empty()
    );
    assert_eq!(
        run(&mut connector, Event::Unsubscribe { task: 1, topic: 7 }),
        Box::from([Request::Erase { key: crate::RecordKey::Subscription { topic: 7, task: 1 } }])
    );
}

#[test]
fn restored_records_rebuild_adoptions_names_topics_and_pool_slots() {
    let mut original = domain();
    let mut records = List::with_capacity(4);
    for event in [
        Event::Adopt { project: 1, resource: service(), role: ResourceRole::Participating },
        Event::Names { task: 1, project: 1, resources: Box::from([service()]) },
        Event::Subscribe { task: 1, topic: 7, wake_at: 4, keep_at: 2 },
        Event::System(SystemEvent::Pool { path: pool(), slots: 1, lost: Box::from([]) }),
    ] {
        for request in run(&mut original, event) {
            match request {
                Request::Save { record } => records.push(record).expect("four changes made four records"),
                Request::Resource { .. }
                | Request::Named { .. }
                | Request::Described { .. }
                | Request::EffectBusy { .. }
                | Request::EffectRefused { .. }
                | Request::Make { .. }
                | Request::Outcome { .. }
                | Request::System(..)
                | Request::Unknown { .. }
                | Request::Refused { .. }
                | Request::Adopted { .. }
                | Request::Slots { .. }
                | Request::Drift { .. }
                | Request::Topic { .. }
                | Request::TopicRefused { .. }
                | Request::News { .. }
                | Request::Erase { .. }
                | Request::Verdict { .. }
                | Request::Step { .. }
                | Request::Answer { .. }
                | Request::Ready { .. }
                | Request::Section { .. }
                | Request::Workspace { .. }
                | Request::DriftResource { .. }
                | Request::Document { .. }
                | Request::ReadAnswered { .. }
                | Request::Marked { .. }
                | Request::Changed { .. }
                | Request::RestartDone
                | Request::ApplicationDone { .. }
                | Request::ApplicationAgain { .. }
                | Request::ApplicationRelease { .. }
                | Request::Closed { .. } => {}
            }
        }
    }
    let mut cold = domain();
    for record in &records {
        let restored = run(&mut cold, Event::Restore { record: record.clone() });
        if let Record::Adoption { project, path, role } = record {
            match restored.as_ref() {
                [Request::Resource { project: reported_project, resource }] => {
                    assert_eq!(reported_project, project);
                    assert_eq!(&resource.path, path);
                    assert_eq!(&resource.role, role);
                }
                other => panic!("restored adoption report: {other:?}"),
            }
        } else {
            assert!(restored.is_empty());
        }
    }
    assert_eq!(cold.slots(&pool()), Some(1));
    assert_eq!(cold.live_tasks(), 1);
    assert_eq!(
        run(&mut cold, Event::System(SystemEvent::News { topic: 7, importance: 3, origin: Origin::Other })),
        Box::from([Request::News { topic: 7, subscribers: Box::from([Classed { task: 1, class: Class::Keep }]) }])
    );
}

#[test]
fn a_prepared_client_subject_checks_its_actual_scope_and_echoes_only_its_owner_name() {
    use crate::{ClientDocument, ClientScope, ClientSubject, DocumentMissing};
    use skein_lib::Token;
    let mut domain = domain();
    run(&mut domain, Event::Adopt { project: 1, resource: service(), role: ResourceRole::Owned });
    let subject = Token::new(100);
    run(
        &mut domain,
        Event::DocumentPrepare { token: subject, subject: ClientSubject::Object { project: 1, object: 1 } },
    );
    let out = run(
        &mut domain,
        Event::Document {
            watcher: Token::new(101),
            scope: ClientScope::Project(2),
            subject,
            party: 7,
            window: crate::ClientWindow { rows: 16, after: None },
        },
    );
    let [Request::Document { subject: named, document: Err(DocumentMissing::Unknown), .. }] = out.as_ref() else {
        panic!("wrong scope is unknown")
    };
    assert_eq!(*named, subject);
    let out = run(
        &mut domain,
        Event::Document {
            watcher: Token::new(101),
            scope: ClientScope::Project(1),
            subject,
            party: 7,
            window: crate::ClientWindow { rows: 16, after: None },
        },
    );
    let [Request::Document { document: Ok(ClientDocument::Object { object: 1, now }), .. }] = out.as_ref() else {
        panic!("the prepared owner constructs its object")
    };
    assert_eq!((now.revision, now.value), (0, 0));
    run(&mut domain, Event::DocumentForget { subject });
    let out = run(
        &mut domain,
        Event::ReadDocument {
            scope: ClientScope::Project(1),
            read: subject,
            party: 7,
            window: crate::ClientWindow { rows: 16, after: None },
        },
    );
    let [Request::ReadAnswered { answer: Err(DocumentMissing::Unknown), .. }] = out.as_ref() else {
        panic!("forgotten name is unknown")
    };
}

#[test]
fn an_offered_topic_names_the_resources_it_concerns() {
    let mut domain = domain();
    let env = Env { now: Time::ZERO, wall: Wall::from_nanos(1), limits: LIMITS };
    let mut out = Queue::with_capacity(MAX_OUT);
    step(&mut domain, &env, Event::DescribeTopic { token: skein_lib::Token::new(81), topic: 7 }, &mut out);
    assert_eq!(out.pop(), Some(Request::Topic { token: skein_lib::Token::new(81), resources: Box::new([service()]) }));
}

#[test]
fn a_topic_not_offered_to_agents_is_refused() {
    let mut domain = domain();
    domain.config.topics[0].offered = false;
    let env = Env { now: Time::ZERO, wall: Wall::from_nanos(1), limits: LIMITS };
    let mut out = Queue::with_capacity(MAX_OUT);
    step(&mut domain, &env, Event::DescribeTopic { token: skein_lib::Token::new(82), topic: 7 }, &mut out);
    assert_eq!(out.pop(), Some(Request::TopicRefused { token: skein_lib::Token::new(82) }));
}

fn project_document(window: crate::ClientWindow) -> crate::ClientDocument {
    use crate::{ClientScope, ClientSubject};
    let mut domain = domain();
    for resource in [service(), pool()] {
        run(&mut domain, Event::Adopt { project: 1, resource, role: ResourceRole::Owned });
    }
    let subject = skein_lib::Token::new(PROJECT_DOCUMENT);
    run(&mut domain, Event::DocumentPrepare { token: subject, subject: ClientSubject::Project { project: 1 } });
    let out = run(&mut domain, Event::ReadDocument { scope: ClientScope::Project(1), read: subject, party: 7, window });
    let [Request::ReadAnswered { answer: Ok(document), .. }] = out.as_ref() else { panic!("project rows") };
    document.clone()
}

#[test]
fn a_document_holds_at_most_its_windows_rows() {
    for rows in WINDOW_ROWS {
        let crate::ClientDocument::Project { resources, .. } =
            project_document(crate::ClientWindow { rows, after: None })
        else {
            panic!("project")
        };
        assert_eq!(resources.len(), rows, "window {rows}");
    }
}

#[test]
fn a_document_starts_after_its_windows_cursor() {
    let crate::ClientDocument::Project { resources, .. } =
        project_document(crate::ClientWindow { rows: 2, after: Some(crate::ClientCursor::Number(1)) })
    else {
        panic!("project")
    };
    assert_eq!(resources.len(), 1);
    assert_eq!(resources.get(0).expect("second row").path, pool());
}

#[test]
fn an_own_scope_shows_what_the_deployment_scope_shows() {
    let mut domain = domain();
    let subject = skein_lib::Token::new(DEPLOYMENT_DOCUMENT);
    run(&mut domain, Event::DocumentPrepare { token: subject, subject: crate::ClientSubject::Deployment });
    let expected = crate::ClientDocument::Deployment {
        outbox: List::with_capacity(LIMITS.entries),
        subject_limit: LIMITS.values,
        staged_limit: LIMITS.staged,
        request_budget: crate::DiagnosticAvailability::NotKept,
        system_last_response: crate::DiagnosticAvailability::NotKept,
    };
    for scope in [crate::ClientScope::Deployment, crate::ClientScope::Own] {
        let out = run(
            &mut domain,
            Event::Document {
                watcher: subject,
                scope,
                subject,
                party: 7,
                window: crate::ClientWindow { rows: 2, after: None },
            },
        );
        let [Request::Document { document, .. }] = out.as_ref() else { panic!("deployment") };
        assert_eq!(document, &Ok(expected.clone()), "scope {scope:?}");
    }
}

const PROCEDURE_TASK: u64 = 9;
const DELEGATE_TASK: u64 = 42;
const FIRST_STEP: u64 = 1;
const AMEND_STEP: u64 = 2;

fn amending_procedure() -> Domain {
    let mut connector = domain();
    connector.config.procedures[0].actions = Box::new([
        crate::ProcedureAction::Delegate { kinds: Box::new([2]) },
        crate::ProcedureAction::Amend { words: 12, priority: None },
    ]);
    drop(run(&mut connector, procedure_event(crate::ProcedureSignal::Activate)));
    drop(run(
        &mut connector,
        Event::StepMade {
            task: PROCEDURE_TASK,
            step: FIRST_STEP,
            first: Some(DELEGATE_TASK),
            revisions: Box::new([(DELEGATE_TASK, 3)]),
        },
    ));
    connector
}

#[test]
fn an_amend_action_names_the_revision_it_was_told() {
    let mut connector = amending_procedure();
    let requests = run(&mut connector, procedure_event(crate::ProcedureSignal::Activate));
    assert_eq!(
        requests.last(),
        Some(&Request::Step {
            task: PROCEDURE_TASK,
            decision: crate::StepDecision::Amend { task: DELEGATE_TASK, revision: 3, words: 12, priority: None },
        })
    );
}

#[test]
fn a_moved_refusal_makes_the_next_step_name_the_new_revision() {
    let mut connector = amending_procedure();
    drop(run(&mut connector, procedure_event(crate::ProcedureSignal::Activate)));
    let requests = run(
        &mut connector,
        Event::StepRefused { task: PROCEDURE_TASK, step: AMEND_STEP, why: crate::StepRefusal::Moved { revision: 4 } },
    );
    assert_eq!(
        requests.last(),
        Some(&Request::Step {
            task: PROCEDURE_TASK,
            decision: crate::StepDecision::Amend { task: DELEGATE_TASK, revision: 4, words: 12, priority: None },
        })
    );
}

#[test]
fn a_kept_step_refusal_clears_the_awaited_action_without_advancing_it() {
    let mut connector = amending_procedure();
    drop(run(&mut connector, procedure_event(crate::ProcedureSignal::Activate)));
    drop(run(
        &mut connector,
        Event::StepRefused {
            task: PROCEDURE_TASK,
            step: AMEND_STEP,
            why: crate::StepRefusal::Kept { pending: Box::new([DELEGATE_TASK]) },
        },
    ));
    assert!(!connector.procedures.get(&PROCEDURE_TASK).expect("live procedure").awaiting);
    let requests = run(&mut connector, procedure_event(crate::ProcedureSignal::Activate));
    assert_eq!(
        requests.last(),
        Some(&Request::Step {
            task: PROCEDURE_TASK,
            decision: crate::StepDecision::Amend { task: DELEGATE_TASK, revision: 3, words: 12, priority: None },
        })
    );
}

#[test]
fn restored_outboxes_wait_for_the_scripted_settlement() {
    // One live outbox, no new decisions or system kind lookup needed to show its timer gate.
    let limits = Limits { entries: 1, ..LIMITS };
    let mut cold = domain_with_limits(&limits);
    let env = Env { now: Time::ZERO, wall: Wall::from_nanos(9), limits };
    let mut out = Queue::with_capacity(MAX_OUT);
    step(
        &mut cold,
        &env,
        Event::Restore {
            record: Record::Outbox(crate::OutboxEntry {
                number: 1,
                task: 4,
                key: crate::Key { attempt: 0, completion: 0, position: 0, deployment: [1; 16], task: 4, purpose: 1 },
                effect: crate::Effect {
                    kind: 1,
                    resources: Box::new([]),
                    purpose: 1,
                    condition: None,
                    target: 2,
                    state: 1,
                },
                attempt: None,
                phase: crate::EffectPhase::Uncertain,
            }),
        },
        &mut out,
    );
    crate::fire(&mut cold, &env, &mut out);
    assert!(out.is_empty(), "no restored system action before the mark");
    assert_eq!(crate::next_deadline(&cold), None);
    // Activation is the existing scripted stage, after adoption and the mark.
    crate::outbox::activate(&mut cold);
    assert_eq!(crate::next_deadline(&cold), Some(Wall::EPOCH));
}

#[test]
fn valid_refuses_each_unrepresentable_limit() {
    assert!(crate::valid(&LIMITS));
    for (condition, invalid) in [
        ("positive path segments", Limits { path_segments: 0, ..LIMITS }),
        ("positive segment bytes", Limits { segment_bytes: 0, ..LIMITS }),
        ("positive attempt count", Limits { max_attempts: 0, ..LIMITS }),
        ("judge count", Limits { judges: 5, ..LIMITS }),
    ] {
        assert!(!crate::valid(&invalid), "{condition}");
    }
    assert!(crate::valid(&Limits { judges: 4, ..LIMITS }));
}

#[test]
fn validity_does_not_price_retained_heap() {
    let limits = Limits { tasks: u32::MAX, resources_per_task: u32::MAX, ..LIMITS };
    assert!(crate::valid(&limits));
}

const DRIFT_TASK: u64 = 9;
const DRIFT_JUDGE: skein_lib::Token = skein_lib::Token::new(5);
const BEFORE_DRIFT: u64 = 2;
const AFTER_DRIFT: u64 = 3;
const OLD_OBSERVATION: u64 = 10;
const FRESH_OBSERVATION: u64 = 20; // Past the fixture requirement's five-nanosecond freshness.

fn waiting_drift() -> Domain {
    let mut connector = domain();
    for event in [
        Event::Adopt { project: 1, resource: service(), role: ResourceRole::Owned },
        Event::Names { project: 1, task: DRIFT_TASK, resources: Box::new([service()]) },
        Event::System(SystemEvent::Fact {
            resource: service(),
            fact: crate::Fact {
                state: Some(BEFORE_DRIFT),
                observed: Wall::from_nanos(OLD_OBSERVATION),
                pending: false,
            },
            origin: Origin::Own,
        }),
    ] {
        drop(run_at(&mut connector, OLD_OBSERVATION, event));
    }
    drop(run_at(
        &mut connector,
        FRESH_OBSERVATION,
        Event::Judge { token: DRIFT_JUDGE, requirement: 4, resources: Box::new([service()]), state: AFTER_DRIFT },
    ));
    connector
}

fn drift_fact() -> crate::Fact {
    crate::Fact { state: Some(AFTER_DRIFT), observed: Wall::from_nanos(FRESH_OBSERVATION), pending: false }
}

#[test]
fn a_settle_answers_its_waiting_judge_and_a_drift_reports_the_change() {
    let mut connector = waiting_drift();
    let (settle, drift) = crate::split(&connector, service(), drift_fact(), Origin::Other);
    let settled = run_at(&mut connector, FRESH_OBSERVATION, Event::System(settle));
    assert_eq!(
        settled.as_ref(),
        [
            Request::Changed { resource: service() },
            Request::Verdict {
                token: DRIFT_JUDGE,
                verdict: crate::Verdict::Met {
                    state: AFTER_DRIFT,
                    observed: Wall::from_nanos(FRESH_OBSERVATION),
                    guarded: true
                }
            },
        ],
        "settlement answers the retained judge without drift"
    );
    let reported = run_at(&mut connector, FRESH_OBSERVATION, Event::System(drift.expect("the kept state changed")));
    assert_eq!(
        reported.as_ref(),
        [Request::DriftResource { tasks: Box::new([DRIFT_TASK]), resource: service() }],
        "the new part reports drift once"
    );
    let mut separate = List::with_capacity(MAX_OUT);
    for request in settled {
        separate.push(request).expect("the split's bounded settlement");
    }
    for request in reported {
        separate.push(request).expect("the split's bounded drift");
    }
    let mut whole = waiting_drift();
    assert_eq!(
        run_at(
            &mut whole,
            FRESH_OBSERVATION,
            Event::System(SystemEvent::Fact { resource: service(), fact: drift_fact(), origin: Origin::Other })
        )
        .as_ref(),
        separate.as_slice(),
        "whole and split facts keep settlement before drift"
    );
}

#[test]
fn a_drift_waiting_through_a_return_to_its_old_state_is_obsolete() {
    let mut connector = waiting_drift();
    let (settle, drift) = crate::split(&connector, service(), drift_fact(), Origin::Other);
    drop(run_at(&mut connector, FRESH_OBSERVATION, Event::System(settle)));
    drop(run_at(
        &mut connector,
        FRESH_OBSERVATION,
        Event::System(SystemEvent::Fact {
            resource: service(),
            origin: Origin::Own,
            fact: crate::Fact {
                state: Some(BEFORE_DRIFT),
                observed: Wall::from_nanos(FRESH_OBSERVATION),
                pending: false,
            },
        }),
    ));
    assert!(
        run_at(&mut connector, FRESH_OBSERVATION, Event::System(drift.expect("the original change had drift")))
            .is_empty(),
        "a delayed drift checks the fact the connector keeps now"
    );
}
