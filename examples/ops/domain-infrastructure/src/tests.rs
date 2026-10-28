use alloc::boxed::Box;
use skein_lib::{Env, Queue, Time, Token, Wall};

use super::*;

fn limits() -> Limits {
    Limits {
        services: 4,
        environments: 4,
        pools: 2,
        tasks: 4,
        procedures: 4,
        staged: 4,
        proposals: 4,
        effects: 4,
        made: 4,
        resources_per_task: 4,
        name_bytes: 64,
        max_attempts: 3,
        retry_after_seconds: 30,
    }
}

fn service() -> Service {
    Service::new(Box::from(*b"production"), Box::from(*b"checkout"))
}
fn environment() -> Environment {
    Environment::new(Box::from(*b"staging"), Box::from(*b"payments"))
}
fn key() -> Key {
    Key::procedure([1; 16], 7, 3)
}

fn event(domain: &mut Domain, seconds: u64, input: Event) -> Queue<Request> {
    let wall = seconds.checked_mul(1_000_000_000).expect("world time fits");
    let env = Env { now: Time::from_nanos(seconds), wall: Wall::from_nanos(wall), limits: limits() };
    let mut out = Queue::with_capacity(MAX_OUT);
    step(domain, &env, input, &mut out);
    out
}

#[test]
fn effect_descriptions_name_their_recovery_and_price() {
    let mut domain = Domain::new(Backend::OperationIds, &limits());
    let effect = Effect::CreateEnvironment { environment: environment(), until: 100, price: 50 };
    let mut out = event(&mut domain, 1, Event::Describe { token: Token::new(1), effect });
    assert_eq!(
        out.pop(),
        Some(Request::Described {
            waits_for_dependencies: true,
            token: Token::new(1),
            description: Description {
                kind: 4,
                resources: Box::from([
                    Resource::Environment(environment()),
                    Resource::Pool(Pool(Box::from(*b"staging")))
                ]),
                condition: false,
                form: Form::Creation,
                recovery: Recovery::Keyed,
                price: Some(50),
                state: 100,
            }
        })
    );
    let mut no_ids = Domain::new(Backend::NoOperationIds, &limits());
    let mut restart = event(
        &mut no_ids,
        1,
        Event::Describe { token: Token::new(2), effect: Effect::Restart { service: service(), operation: 9 } },
    );
    assert_eq!(
        restart.pop(),
        Some(Request::Described {
            waits_for_dependencies: true,
            token: Token::new(2),
            description: Description {
                kind: 1,
                resources: Box::from([Resource::Service(service())]),
                condition: false,
                form: Form::Transition,
                recovery: Recovery::Unrecoverable,
                price: None,
                state: 9,
            }
        })
    );
}

#[test]
fn a_service_is_exclusive_and_a_pool_slot_waits() {
    let mut domain = Domain::new(Backend::OperationIds, &limits());
    let mut named = event(
        &mut domain,
        1,
        Event::Names {
            project: 1,
            task: 7,
            resources: Box::from([Resource::Service(service()), Resource::Pool(Pool(Box::from(*b"staging")))]),
        },
    );
    assert_eq!(
        named.pop(),
        Some(Request::Save {
            record: Record::Rely {
                project: 1,
                task: 7,
                resources: Box::from([
                    Named {
                        role: ResourceRole::Owned,
                        resource: Resource::Service(service()),
                        hold: Hold::ExclusiveWait
                    },
                    Named {
                        role: ResourceRole::Owned,
                        resource: Resource::Pool(Pool(Box::from(*b"staging"))),
                        hold: Hold::PooledWait
                    },
                ])
            }
        })
    );
    assert_eq!(
        named.pop(),
        Some(Request::Named {
            project: 1,
            task: 7,
            resources: Box::from([
                Named { role: ResourceRole::Owned, resource: Resource::Service(service()), hold: Hold::ExclusiveWait },
                Named {
                    role: ResourceRole::Owned,
                    resource: Resource::Pool(Pool(Box::from(*b"staging"))),
                    hold: Hold::PooledWait
                },
            ])
        })
    );
}

#[test]
fn an_uncertain_restart_without_ids_is_held_and_not_resent() {
    let mut domain = Domain::new(Backend::NoOperationIds, &limits());
    let token = Token::new(1);
    event(&mut domain, 1, Event::Describe { token, effect: Effect::Restart { service: service(), operation: 9 } });
    event(&mut domain, 1, Event::Keep { project: Some(1), entry: 3, token, key: key() });
    event(&mut domain, 1, Event::Make { key: key() });
    let mut uncertain = event(
        &mut domain,
        1,
        Event::System(SystemEvent::Applied { key: key(), attempt: 1, result: ApplyResult::Uncertain }),
    );
    assert_eq!(
        uncertain.pop(),
        Some(Request::Save {
            record: Record::Outbox(Entry {
                project: Some(1),
                number: 3,
                key: key(),
                effect: Effect::Restart { service: service(), operation: 9 },
                attempt: 1,
                deadline: 31,
                phase: Phase::Held,
            })
        })
    );
    assert_eq!(uncertain.pop(), Some(Request::Outcome { entry: 3, key: key(), outcome: Outcome::Uncertain }));
    let mut restarted = event(&mut domain, 31, Event::Restart);
    assert_eq!(restarted.pop(), Some(Request::Restarted { stage: RestartStage::Outbox }));
    assert!(restarted.is_empty(), "held restart is not retried");
}

#[test]
fn a_procedure_waits_for_current_facts_and_never_asks_for_two_effects() {
    let mut domain = Domain::new(Backend::OperationIds, &limits());
    let procedure = Procedure::Remediate { service: service(), operation: 9, deadline: 100 };
    event(
        &mut domain,
        1,
        Event::StartProcedure {
            chat: None,
            pending: Box::new([]),
            added: Box::new([]),
            task: 7,
            project: 1,
            procedure,
        },
    );
    let mut first = event(&mut domain, 1, Event::Procedure { task: 7, signal: ProcedureSignal::Step });
    assert_eq!(first.pop(), Some(Request::System(SystemRequest::Service { service: service() })));
    let fact = ServiceFact { version: Box::from(*b"v1"), replicas: 2, healthy: false, revision: 1, observed: 1 };
    event(&mut domain, 1, Event::System(SystemEvent::Service { service: service(), fact, other_hand: false }));
    let effect = event(&mut domain, 1, Event::Procedure { task: 7, signal: ProcedureSignal::Step });
    assert_eq!(effect.len(), 2);
    let repeated = event(&mut domain, 1, Event::Procedure { task: 7, signal: ProcedureSignal::Step });
    assert_eq!(repeated.len(), 1);
    let mut second = repeated;
    assert_eq!(second.pop(), Some(Request::Step { task: 7, decision: StepDecision::Wait { until: 100 } }));
}

fn pending_restart() -> Domain {
    let mut domain = Domain::new(Backend::OperationIds, &limits());
    event(
        &mut domain,
        1,
        Event::StartProcedure {
            chat: None,
            pending: Box::new([]),
            added: Box::new([]),
            task: 7,
            project: 1,
            procedure: Procedure::Remediate { service: service(), operation: 9, deadline: 100 },
        },
    );
    event(
        &mut domain,
        1,
        Event::System(SystemEvent::Service {
            service: service(),
            fact: ServiceFact { version: Box::from(*b"v1"), replicas: 2, healthy: false, revision: 1, observed: 1 },
            other_hand: false,
        }),
    );
    event(&mut domain, 1, Event::Procedure { task: 7, signal: ProcedureSignal::Step });
    event(
        &mut domain,
        1,
        Event::Describe { token: Token::new(1), effect: Effect::Restart { service: service(), operation: 9 } },
    );
    event(&mut domain, 1, Event::KeepProposal { token: Token::new(1), number: 3, task: 7 });
    domain
}

fn healthy_by_hand(domain: &mut Domain) -> Queue<Request> {
    event(
        domain,
        2,
        Event::System(SystemEvent::Service {
            service: service(),
            fact: ServiceFact { version: Box::from(*b"v1"), replicas: 2, healthy: true, revision: 2, observed: 2 },
            other_hand: true,
        }),
    )
}

#[test]
fn an_unmade_pending_restart_reads_healthy_without_suppressing_other_drift() {
    let mut domain = pending_restart();
    event(&mut domain, 1, Event::Names { task: 8, project: 1, resources: Box::from([Resource::Service(service())]) });
    let mut refreshed = healthy_by_hand(&mut domain);
    let mut pending_drift = false;
    let mut other_drift = false;
    let mut observed = false;
    let mut changed = false;
    while let Some(request) = refreshed.pop() {
        match request {
            Request::Drift { task, resource } => {
                assert_eq!(resource, Resource::Service(service()));
                pending_drift |= task == 7;
                other_drift |= task == 8;
            }
            Request::Save { record: Record::Drift(account) } => {
                assert_eq!(account.resource, Resource::Service(service()));
                assert_eq!(account.kind, DriftKind::Changed);
                observed = true;
            }
            Request::Changed { resource } => {
                assert_eq!(resource, Resource::Service(service()));
                changed = true;
            }
            Request::Document { .. }
            | Request::ReadAnswered { .. }
            | Request::Marked { .. }
            | Request::ApplicationDone { .. }
            | Request::ApplicationBusy { .. }
            | Request::Closed { .. }
            | Request::Released { .. }
            | Request::Restarted { .. }
            | Request::Named { .. }
            | Request::Slots { .. }
            | Request::Wake { .. }
            | Request::Described { .. }
            | Request::Close { .. }
            | Request::Refused { .. }
            | Request::Make { .. }
            | Request::Outcome { .. }
            | Request::Step { .. }
            | Request::Save { .. }
            | Request::Erase { .. }
            | Request::System(_) => {}
        }
    }
    assert!(!pending_drift && other_drift && observed && changed);
    assert!(domain.effects.is_empty(), "a kept proposal has no permission or outbox");
    let mut finished = event(&mut domain, 2, Event::Procedure { task: 7, signal: ProcedureSignal::Step });
    assert_eq!(finished.pop(), Some(Request::Erase { key: RecordKey::Procedure(7) }));
    assert_eq!(
        finished.pop(),
        Some(Request::Step {
            task: 7,
            decision: StepDecision::FinishReport {
                words: Box::from(*b"Service is already healthy; finished without restarting it."),
            },
        })
    );
    assert!(finished.is_empty());

    let mut written = pending_restart();
    event(
        &mut written,
        1,
        Event::Restore {
            record: Record::Outbox(Entry {
                project: Some(1),
                number: 3,
                key: key(),
                effect: Effect::Restart { service: service(), operation: 9 },
                attempt: 0,
                deadline: 1,
                phase: Phase::Kept,
            }),
        },
    );
    let mut requests = healthy_by_hand(&mut written);
    let mut held = false;
    while let Some(request) = requests.pop() {
        held |= request == Request::Drift { task: 7, resource: Resource::Service(service()) };
    }
    assert!(held, "an admitted write retains ordinary drift behavior");

    let mut no_proposal = pending_restart();
    event(&mut no_proposal, 1, Event::DropProposal { number: 3 });
    let mut requests = healthy_by_hand(&mut no_proposal);
    let mut held = false;
    while let Some(request) = requests.pop() {
        held |= request == Request::Drift { task: 7, resource: Resource::Service(service()) };
    }
    assert!(held, "the exception requires the genuine pending proposal");
    let mut finished = event(&mut no_proposal, 2, Event::Procedure { task: 7, signal: ProcedureSignal::Step });
    assert_eq!(finished.pop(), Some(Request::Erase { key: RecordKey::Procedure(7) }));
    assert_eq!(finished.pop(), Some(Request::Step { task: 7, decision: StepDecision::Finish }));
}

#[test]
fn a_committed_person_retry_opens_restart_but_waits_for_the_old_deadline() {
    let mut domain = Domain::new(Backend::NoOperationIds, &limits());
    let row = Entry {
        project: Some(1),
        number: 1,
        key: key(),
        effect: Effect::Restart { service: service(), operation: 9 },
        attempt: 1,
        deadline: 30,
        phase: Phase::Held,
    };
    assert!(event(&mut domain, 2, Event::Restore { record: Record::Outbox(row) }).is_empty());
    let mut choice = event(&mut domain, 2, Event::ResolveEntry { entry: 1, repeat: true });
    let Some(Request::Save { record: Record::Outbox(row) }) = choice.pop() else {
        panic!("retry choice saved");
    };
    assert_eq!(row.deadline, 30);
    assert_eq!(row.attempt, 1);
    assert_eq!(row.phase, Phase::Kept);
    assert!(choice.is_empty());
    let mut late = event(
        &mut domain,
        3,
        Event::System(SystemEvent::Applied { key: key(), attempt: 1, result: ApplyResult::Made }),
    );
    assert_eq!(late.pop(), Some(Request::Erase { key: RecordKey::Outbox(key()) }));
    assert_eq!(late.pop(), Some(Request::Outcome { entry: 1, key: key(), outcome: Outcome::Made }));
    let mut restored = Domain::new(Backend::NoOperationIds, &limits());
    event(&mut restored, 3, Event::Restore { record: Record::Outbox(row) });
    let mut restart = event(&mut restored, 3, Event::Restart);
    assert_eq!(restart.pop(), Some(Request::Make { key: key() }));
    assert_eq!(restart.pop(), Some(Request::Restarted { stage: RestartStage::Outbox }));
    assert!(event(&mut restored, 3, Event::Make { key: key() }).is_empty());
    let mut out = Queue::with_capacity(MAX_OUT);
    fire(&mut restored, &Env { now: Time::ZERO, wall: Wall::from_nanos(30_000_000_000), limits: limits() }, &mut out);
    let Some(Request::Save { record: Record::Outbox(row) }) = out.pop() else {
        panic!("new attempt saved before write");
    };
    assert_eq!(row.attempt, 2);
    let Some(Request::System(SystemRequest::Apply { attempt, .. })) = out.pop() else {
        panic!("new attempt released");
    };
    assert_eq!(attempt, 2);
}

#[test]
fn environment_document_supplies_the_request_revision_without_creating_an_end_or_rewriting_the_lease() {
    let mut domain = Domain::new(Backend::OperationIds, &limits());
    let environment = environment();
    let fact = EnvironmentFact { created_by: key(), ready_at: 2, until: 1000, price: 50 };
    let _out = event(
        &mut domain,
        1,
        Event::Restore { record: Record::Made { environment: environment.clone(), key: key(), project: Some(3) } },
    );
    domain.environments.insert(environment.clone(), Some(fact)).expect("bounded observed environment");
    let token = Token::new(21);
    let _out = event(
        &mut domain,
        1,
        Event::DocumentPrepare {
            token,
            subject: ClientSubject::Environment { project: 3, environment: environment.clone() },
        },
    );
    let mut out = event(
        &mut domain,
        1,
        Event::Document { watcher: Token::new(22), scope: ClientScope::Project(3), subject: token, party: 7 },
    );
    let Some(Request::Document {
        document: Ok(ClientDocument::Environment { environment: supplied, fact: supplied_fact, end_revision, end, .. }),
        ..
    }) = out.pop()
    else {
        panic!("actual environment document")
    };
    assert_eq!(supplied, environment);
    assert_eq!(supplied_fact, fact);
    assert_eq!(end_revision, 0);
    assert_eq!(end, None);
    let owner = Token::new(23);
    let _out = event(
        &mut domain,
        1,
        Event::ApplicationPrepare {
            owner,
            value: ApplicationRequest { environment: environment.clone(), revision: end_revision, until: 2000 },
        },
    );
    let _out = event(
        &mut domain,
        1,
        Event::Application {
            request: Token::new(24),
            token: Token::new(25),
            owner,
            person: 7,
            project: 3,
            role: 1,
            retention: skein_lib::Duration::from_secs(60),
        },
    );
    assert_eq!(
        domain.application_outcome(Token::new(25)),
        Some(ApplicationOutcome::MovedEnd {
            environment: environment.clone(),
            now: EnvironmentEnd { revision: 1, until: 2000 }
        })
    );
    let mut out = event(
        &mut domain,
        1,
        Event::Document { watcher: Token::new(22), scope: ClientScope::Project(3), subject: token, party: 7 },
    );
    let Some(Request::Document {
        document: Ok(ClientDocument::Environment { fact: supplied_fact, end_revision, end, .. }),
        ..
    }) = out.pop()
    else {
        panic!("current actual environment document")
    };
    assert_eq!(supplied_fact, fact, "desired end does not rewrite the observed backend lease");
    assert_eq!(end_revision, 1);
    assert_eq!(end, Some(EnvironmentEnd { revision: 1, until: 2000 }));
}

#[test]
fn provision_wakes_at_supplied_readiness_and_keeps_deadline_and_holding_behavior() {
    for deadline in [4, 10] {
        let mut domain = Domain::new(Backend::OperationIds, &limits());
        event(
            &mut domain,
            0,
            Event::StartProcedure {
                chat: None,
                pending: Box::new([]),
                added: Box::new([]),
                task: 7,
                project: 1,
                procedure: Procedure::Provision { environment: environment(), until: 100, price: 50, deadline },
            },
        );
        let fact = Some(EnvironmentFact { created_by: key(), ready_at: 5, until: 100, price: 50 });
        event(
            &mut domain,
            0,
            Event::System(SystemEvent::Environment { environment: environment(), fact, other_hand: false }),
        );
        let mut out = event(&mut domain, 0, Event::Procedure { task: 7, signal: ProcedureSignal::Step });
        let mut decisions = skein_lib::List::with_capacity(MAX_OUT);
        while let Some(request) = out.pop() {
            if let Request::Step { task: 7, decision } = request {
                decisions.push(decision).expect("finite decisions fit the output bound");
            }
        }
        assert_eq!(decisions.as_slice(), &[StepDecision::Wait { until: deadline.min(5) }]);
        assert_eq!(next_deadline(&domain), Some(Time::from_nanos(5_000_000_000)));
        assert_restored_provision_timer(&domain, fact);
        for seconds in [4, 5] {
            let env = Env {
                now: Time::from_nanos(seconds * 1_000_000_000),
                wall: Wall::from_nanos(seconds * 1_000_000_000),
                limits: limits(),
            };
            let mut timer = Queue::with_capacity(MAX_OUT);
            fire(&mut domain, &env, &mut timer);
            if seconds == 4 {
                assert!(timer.is_empty());
            } else {
                assert_eq!(timer.pop(), Some(Request::Wake { task: 7 }));
                assert!(timer.is_empty(), "a timed wake is neither a write nor a fact change");
            }
        }
        let mut unchanged = event(
            &mut domain,
            3,
            Event::System(SystemEvent::Environment { environment: environment(), fact, other_hand: false }),
        );
        while let Some(request) = unchanged.pop() {
            if let Request::Changed { .. } = request {
                panic!("readiness is a time condition, not a manufactured fact change");
            }
        }
        let mut out = event(&mut domain, 5, Event::Procedure { task: 7, signal: ProcedureSignal::Step });
        let mut decisions = skein_lib::List::with_capacity(MAX_OUT);
        while let Some(request) = out.pop() {
            if let Request::Step { task: 7, decision } = request {
                decisions.push(decision).expect("finite decisions fit the output bound");
            }
        }
        assert_eq!(
            decisions.as_slice(),
            &[if deadline == 4 { StepDecision::Hold { reason: 2 } } else { StepDecision::Finish }]
        );
        assert_eq!(next_deadline(&domain), None);
        let mut repeated = event(&mut domain, 6, Event::Procedure { task: 7, signal: ProcedureSignal::Step });
        while let Some(request) = repeated.pop() {
            if let Request::Step { .. } = request {
                panic!("Holding or Held remains settled");
            }
        }
    }
}

fn assert_restored_provision_timer(domain: &Domain, fact: Option<EnvironmentFact>) {
    let mut restored = Domain::new(Backend::OperationIds, &limits());
    let state = domain.procedures.get(&7).expect("the waiting state was supplied for saving").clone();
    event(&mut restored, 3, Event::Restore { record: Record::Procedure(state) });
    event(&mut restored, 3, Event::ReadAfresh);
    event(
        &mut restored,
        3,
        Event::System(SystemEvent::Environment { environment: environment(), fact, other_hand: false }),
    );
    assert_eq!(next_deadline(&restored), Some(Time::from_nanos(5_000_000_000)));
    let env = Env { now: Time::from_nanos(5_000_000_000), wall: Wall::from_nanos(5_000_000_000), limits: limits() };
    let mut out = Queue::with_capacity(MAX_OUT);
    fire(&mut restored, &env, &mut out);
    assert_eq!(out.pop(), Some(Request::Wake { task: 7 }));
    assert!(out.is_empty(), "restoring a time condition makes no effect and invents no fact change");
}

#[test]
fn restored_resource_reports_keep_owner_roles_holds_and_task_scopes_during_read_afresh() {
    let first: Box<[Named]> = Box::from([
        Named { resource: Resource::Service(service()), role: ResourceRole::Owned, hold: Hold::ExclusiveWait },
        Named {
            resource: Resource::Pool(Pool(Box::from(*b"staging"))),
            role: ResourceRole::Participant,
            hold: Hold::PooledWait,
        },
    ]);
    let second: Box<[Named]> =
        Box::from([Named { resource: Resource::Service(service()), role: ResourceRole::Context, hold: Hold::None }]);
    let mut domain = Domain::new(Backend::OperationIds, &limits());
    assert!(
        event(
            &mut domain,
            1,
            Event::Restore { record: Record::Rely { task: 7, project: 1, resources: first.clone() } }
        )
        .is_empty()
    );
    assert!(
        event(
            &mut domain,
            1,
            Event::Restore { record: Record::Rely { task: 8, project: 2, resources: second.clone() } }
        )
        .is_empty()
    );
    let mut out = event(&mut domain, 2, Event::ReadAfresh);
    assert_eq!(out.pop(), Some(Request::Named { task: 7, project: 1, resources: first }));
    assert_eq!(out.pop(), Some(Request::Named { task: 8, project: 2, resources: second }));
    let mut reads = 0_u32;
    while let Some(request) = out.pop() {
        match request {
            Request::System(SystemRequest::Service { service: got }) => {
                assert_eq!(got, service());
                reads += 1;
            }
            Request::System(SystemRequest::Pool { pool }) => {
                assert_eq!(pool, Pool(Box::from(*b"staging")));
                reads += 1;
            }
            other @ (Request::Document { .. }
            | Request::ReadAnswered { .. }
            | Request::Marked { .. }
            | Request::ApplicationDone { .. }
            | Request::ApplicationBusy { .. }
            | Request::Closed { .. }
            | Request::Released { .. }
            | Request::Restarted { .. }
            | Request::Named { .. }
            | Request::Slots { .. }
            | Request::Drift { .. }
            | Request::Wake { .. }
            | Request::Changed { .. }
            | Request::Described { .. }
            | Request::Close { .. }
            | Request::Refused { .. }
            | Request::Make { .. }
            | Request::Outcome { .. }
            | Request::Step { .. }
            | Request::Save { .. }
            | Request::Erase { .. }
            | Request::System(_)) => {
                panic!("restoration supplies facts and fresh reads, not new writes or effects: {other:?}");
            }
        }
    }
    assert_eq!(reads, 2, "the shared service has one actual fresh read");
}

#[test]
fn restored_effect_timers_wait_for_read_afresh() {
    let mut domain = Domain::new(Backend::OperationIds, &limits());
    event(
        &mut domain,
        2,
        Event::Restore {
            record: Record::Outbox(Entry {
                project: Some(1),
                number: 3,
                key: key(),
                effect: Effect::Restart { service: service(), operation: 9 },
                attempt: 1,
                deadline: 1,
                phase: Phase::Uncertain,
            }),
        },
    );
    let env = Env { now: Time::from_nanos(2_000_000_000), wall: Wall::from_nanos(2_000_000_000), limits: limits() };
    let mut out = Queue::with_capacity(MAX_OUT);
    fire(&mut domain, &env, &mut out);
    assert!(out.is_empty(), "no system lookup before the scripted post-mark step");
    assert_eq!(next_deadline(&domain), None);
    event(&mut domain, 2, Event::ReadAfresh);
    fire(&mut domain, &env, &mut out);
    let Some(Request::System(SystemRequest::Look { .. })) = out.pop() else {
        panic!("system lookup resumes after read afresh");
    };
}

#[test]
fn valid_refuses_each_unrepresentable_limit() {
    let base = limits();
    assert!(valid(&base));
    for (condition, invalid) in [
        ("positive name bytes", Limits { name_bytes: 0, ..base }),
        ("positive attempts", Limits { max_attempts: 0, ..base }),
        ("positive retry", Limits { retry_after_seconds: 0, ..base }),
        ("closing task and procedure count", Limits { tasks: u32::MAX, resources_per_task: 0, ..base }),
        ("closing effect count", Limits { effects: u32::MAX, ..base }),
        ("resource product", Limits { resources_per_task: u32::MAX, ..base }),
        ("procedure refresh count", Limits { tasks: 1, resources_per_task: u32::MAX, ..base }),
        ("environment refresh count", Limits { environments: u32::MAX, ..base }),
        ("creation refresh and drift count", Limits { made: u32::MAX, ..base }),
    ] {
        assert!(!valid(&invalid), "{condition}");
    }
}

#[test]
fn validity_does_not_price_retained_heap() {
    let limits = Limits { services: u32::MAX, name_bytes: u32::MAX, ..limits() };
    assert!(valid(&limits));
}
