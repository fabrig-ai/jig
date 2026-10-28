use alloc::boxed::Box;
use skein_lib::{Env, Queue, Time, Token, Wall};

use super::*;

fn limits() -> Limits {
    Limits {
        services: 4,
        subscriptions: 8,
        watches: 2,
        staged: 2,
        effects: 2,
        judges: 4,
        reads: 2,
        subscribers_per_alert: 8,
        samples_per_service: 8,
        services_per_watch: 2,
        alerts_per_batch: 4,
        name_bytes: 32,
        answer_bytes: 64,
        max_attempts: 3,
        retry_after_seconds: 30,
    }
}

fn service() -> Service {
    Service::new(Box::from(*b"prod"), Box::from(*b"checkout"))
}

fn event(domain: &mut Domain, seconds: u64, input: Event) -> Queue<Request> {
    let wall = seconds.checked_mul(1_000_000_000).expect("world clock fits");
    let env = Env { now: Time::from_nanos(seconds), wall: Wall::from_nanos(wall), limits: limits() };
    let mut out = Queue::with_capacity(MAX_OUT);
    step(domain, &env, input, &mut out);
    out
}

fn fact(at: u64, load: u8) -> Fact {
    Fact {
        observed: at,
        healthy_replicas: 2,
        errors: 0,
        error_rate_percent: 0,
        load_percent: load,
        load: Box::from([LoadPoint { at: 0, percent: 10 }, LoadPoint { at, percent: load }]),
    }
}

#[test]
fn a_fresh_observed_verdict_uses_this_connectors_facts() {
    let mut domain = Domain::new(&limits());
    let token = Token::new(1);
    let mut judged = event(
        &mut domain,
        30,
        Event::Judge { token, requirement: Requirement::HealthyReplicaElsewhere, service: service(), freshness: 60 },
    );
    assert_eq!(judged.pop(), Some(Request::Verdict { token, verdict: Verdict::Wait }));
    assert_eq!(judged.pop(), Some(Request::System(SystemRequest::Facts { service: service() })));
    let mut fresh = event(&mut domain, 30, Event::System(SystemEvent::Fact { service: service(), fact: fact(30, 20) }));
    assert_eq!(fresh.pop(), Some(Request::Changed { service: service() }));
    assert_eq!(fresh.pop(), Some(Request::Verdict { token, verdict: Verdict::Met { observed: 30 } }));
    let mut load = event(
        &mut domain,
        30,
        Event::Judge {
            token: Token::new(2),
            requirement: Requirement::LoadBelow { percent: 40, for_seconds: 30 },
            service: service(),
            freshness: 60,
        },
    );
    assert_eq!(load.pop(), Some(Request::Verdict { token: Token::new(2), verdict: Verdict::Met { observed: 30 } }));
}

#[test]
fn a_watch_emits_one_triage_per_committed_batch() {
    let mut domain = Domain::new(&limits());
    let mut started =
        event(&mut domain, 1, Event::StartWatch { project: 1, task: 7, services: Box::from([service()]), template: 3 });
    assert_eq!(
        started.pop(),
        Some(Request::Save {
            record: Record::Subscription {
                topic: Topic::Alerts(service()),
                task: 7,
                subscription: 7,
                wake_at: 1,
                keep_at: 0
            }
        })
    );
    assert_eq!(
        started.pop(),
        Some(Request::Save {
            record: Record::Watch(Watch {
                project: 1,
                task: 7,
                services: Box::from([service()]),
                template: 3,
                last_batch: None,
                pending: Box::new([])
            })
        })
    );
    assert_eq!(started.pop(), Some(Request::WatchInterest { task: 7, subscription: 7, topic: 7 }));
    assert!(started.is_empty());
    assert!(
        event(&mut domain, 2, Event::StartWatch { project: 1, task: 7, services: Box::from([service()]), template: 3 })
            .is_empty()
    );
    let wake = Event::WakeWatch { task: 7, batch: 99, alerts: Box::from([1, 2]) };
    let mut first = event(&mut domain, 2, wake.clone());
    assert_eq!(
        first.pop(),
        Some(Request::Save {
            record: Record::Watch(Watch {
                project: 1,
                task: 7,
                services: Box::from([service()]),
                template: 3,
                last_batch: Some(99),
                pending: Box::new([]),
            })
        })
    );
    assert_eq!(
        first.pop(),
        Some(Request::Triage { watch: 7, template: 3, batch: 99, alerts: Box::from([1, 2]), delegate: Box::new([]) })
    );
    assert!(event(&mut domain, 2, wake).is_empty(), "same batch makes no second task");
}

#[test]
fn a_silence_is_saved_before_its_system_call_and_found_after_uncertainty() {
    let mut domain = Domain::new(&limits());
    let token = Token::new(5);
    let key = Key::procedure([1; 16], 7, 3);
    let effect = Effect { rule: Box::from(*b"checkout-errors"), until: 100 };
    let mut described = event(&mut domain, 1, Event::Describe { token, effect: effect.clone() });
    assert_eq!(
        described.pop(),
        Some(Request::Described { token, effect: effect.clone(), waits_for_dependencies: false })
    );
    let mut kept = event(&mut domain, 1, Event::Keep { project: Some(1), entry: 3, token, key });
    assert_eq!(
        kept.pop(),
        Some(Request::Save {
            record: Record::Outbox(Entry {
                project: Some(1),
                number: 3,
                key,
                effect: effect.clone(),
                attempt: 0,
                deadline: 0,
                phase: Phase::Kept
            })
        })
    );
    assert_eq!(kept.pop(), Some(Request::Make { key }));
    let mut made = event(&mut domain, 1, Event::Make { key });
    assert_eq!(
        made.pop(),
        Some(Request::Save {
            record: Record::Outbox(Entry {
                project: Some(1),
                number: 3,
                key,
                effect: effect.clone(),
                attempt: 1,
                deadline: 31,
                phase: Phase::Sent
            })
        })
    );
    assert_eq!(made.pop(), Some(Request::System(SystemRequest::Silence { key, attempt: 1, effect: effect.clone() })));
    let mut uncertain =
        event(&mut domain, 1, Event::System(SystemEvent::Applied { key, attempt: 1, outcome: Outcome::Uncertain }));
    assert_eq!(
        uncertain.pop(),
        Some(Request::Save {
            record: Record::Outbox(Entry {
                project: Some(1),
                number: 3,
                key,
                effect: effect.clone(),
                attempt: 1,
                deadline: 31,
                phase: Phase::Uncertain
            })
        })
    );
    assert_eq!(uncertain.pop(), Some(Request::System(SystemRequest::FindSilence { key, rule: effect.rule })));
    let mut found = event(&mut domain, 1, Event::System(SystemEvent::Found { key, found: true }));
    assert_eq!(found.pop(), Some(Request::Erase { key: RecordKey::Outbox(key) }));
    assert_eq!(found.pop(), Some(Request::Outcome { entry: 3, key, outcome: Outcome::Made }));
}

#[test]
fn an_uncertain_silence_waits_for_its_durable_deadline_before_retry() {
    let mut domain = Domain::new(&limits());
    let token = Token::new(5);
    let key = Key::procedure([1; 16], 7, 3);
    let effect = Effect { rule: Box::from(*b"checkout-errors"), until: 100 };
    event(&mut domain, 1, Event::Describe { token, effect: effect.clone() });
    event(&mut domain, 1, Event::Keep { project: Some(1), entry: 3, token, key });
    event(&mut domain, 1, Event::Make { key });
    event(&mut domain, 1, Event::System(SystemEvent::Applied { key, attempt: 1, outcome: Outcome::Uncertain }));
    assert_eq!(next_deadline(&domain), Some(Time::from_nanos(31_000_000_000)));
    let mut early = event(&mut domain, 2, Event::System(SystemEvent::Found { key, found: false }));
    assert_eq!(
        early.pop(),
        Some(Request::Save {
            record: Record::Outbox(Entry {
                project: Some(1),
                number: 3,
                key,
                effect: effect.clone(),
                attempt: 1,
                deadline: 31,
                phase: Phase::Uncertain,
            })
        })
    );
    assert!(early.is_empty(), "no retry before the deadline");
    let env = Env { now: Time::from_nanos(31), wall: Wall::from_nanos(31_000_000_000), limits: limits() };
    let mut due = Queue::with_capacity(MAX_OUT);
    fire(&mut domain, &env, &mut due);
    assert_eq!(due.pop(), Some(Request::System(SystemRequest::FindSilence { key, rule: effect.rule.clone() })));
    let mut after = event(&mut domain, 31, Event::System(SystemEvent::Found { key, found: false }));
    assert_eq!(
        after.pop(),
        Some(Request::Save {
            record: Record::Outbox(Entry {
                project: Some(1),
                number: 3,
                key,
                effect: effect.clone(),
                attempt: 1,
                deadline: 31,
                phase: Phase::Kept
            })
        })
    );
    assert_eq!(after.pop(), Some(Request::Make { key }));
    event(&mut domain, 31, Event::Make { key });
    let mut late =
        event(&mut domain, 31, Event::System(SystemEvent::Applied { key, attempt: 1, outcome: Outcome::Made }));
    assert_eq!(late.pop(), Some(Request::Erase { key: RecordKey::Outbox(key) }));
    assert_eq!(late.pop(), Some(Request::Outcome { entry: 3, key, outcome: Outcome::Made }));
}

#[test]
fn sustained_load_needs_a_whole_window_and_a_low_starting_sample() {
    let mut domain = Domain::new(&limits());
    event(&mut domain, 10, Event::System(SystemEvent::Fact { service: service(), fact: fact(10, 10) }));
    let judge = Event::Judge {
        token: Token::new(1),
        requirement: Requirement::LoadBelow { percent: 40, for_seconds: 30 },
        service: service(),
        freshness: 60,
    };
    assert_eq!(
        event(&mut domain, 10, judge.clone()).pop(),
        Some(Request::Verdict { token: Token::new(1), verdict: Verdict::Wait })
    );
    let facts = Fact {
        load: Box::from([
            LoadPoint { at: 0, percent: 95 },
            LoadPoint { at: 20, percent: 10 },
            LoadPoint { at: 40, percent: 10 },
        ]),
        ..fact(40, 10)
    };
    event(&mut domain, 40, Event::System(SystemEvent::Fact { service: service(), fact: facts }));
    assert_eq!(
        event(&mut domain, 40, judge).pop(),
        Some(Request::Verdict { token: Token::new(1), verdict: Verdict::Wait })
    );
}

#[test]
fn a_service_document_uses_the_projects_owned_watch_not_the_clients_claim() {
    let mut domain = Domain::new(&limits());
    event(&mut domain, 1, Event::StartWatch { project: 1, task: 7, services: Box::from([service()]), template: 3 });
    event(&mut domain, 1, Event::System(SystemEvent::Fact { service: service(), fact: fact(1, 20) }));
    let read = Token::new(100);
    event(
        &mut domain,
        1,
        Event::DocumentPrepare { token: read, subject: ClientSubject::Service { project: 2, service: service() } },
    );
    let mut unknown = event(&mut domain, 1, Event::ReadDocument { scope: ClientScope::Project(2), read, party: 7 });
    assert_eq!(unknown.pop(), Some(Request::ReadAnswered { read, answer: Err(DocumentMissing::Unknown) }));
    assert!(unknown.is_empty());
    event(
        &mut domain,
        1,
        Event::DocumentPrepare { token: read, subject: ClientSubject::Service { project: 1, service: service() } },
    );
    let mut known = event(&mut domain, 1, Event::ReadDocument { scope: ClientScope::Project(1), read, party: 7 });
    assert_eq!(
        known.pop(),
        Some(Request::ReadAnswered {
            read,
            answer: Ok(ClientDocument::Service { service: service(), fact: fact(1, 20) })
        })
    );
    assert!(known.is_empty());
}

fn incident_owner() -> Domain {
    let mut domain = Domain::with_templates(
        &limits(),
        Box::new([TriageTemplate { number: 3, delegate: Box::from(&b"triage"[..]) }]),
    )
    .with_leads(&limits(), Box::new([LeadTemplate { number: 3, delegate: Box::from(&b"lead"[..]) }]));
    event(&mut domain, 1, Event::StartWatch { task: 7, project: 1, services: Box::new([service()]), template: 3 });
    event(
        &mut domain,
        1,
        Event::System(SystemEvent::Alert { own: false, alert: Alert { number: 12, service: service(), severity: 3 } }),
    );
    let requested = event(
        &mut domain,
        1,
        Event::WatchStep { task: 7, step: 1, batch: 10, messages: Box::new([WatchMessage::News]) },
    );
    assert!(has_request(&requested, ExpectedRequest::Triage));
    event(&mut domain, 1, Event::ProcedureDelegateOutcome { task: 7, step: 1, child: Some(100) });
    domain
}

fn read_incident(domain: &mut Domain) -> Result<ClientDocument, DocumentMissing> {
    let read = Token::new(501);
    event(
        domain,
        2,
        Event::DocumentPrepare { token: read, subject: ClientSubject::Incident { project: 1, incident: 12 } },
    );
    let mut result = event(domain, 2, Event::ReadDocument { scope: ClientScope::Project(1), read, party: 1 });
    let Request::ReadAnswered { answer, .. } = result.pop().expect("one owner answer") else {
        panic!("expected read answer")
    };
    answer
}

fn incident_verdict(child: u64) -> WatchMessage {
    WatchMessage::Triage {
        child,
        verdict: TriageVerdict::Incident { alerts: Box::new([12]), service: service(), severity: 3 },
    }
}

#[test]
fn only_the_retained_triage_child_can_open_an_incident_and_noise_opens_none() {
    let mut domain = incident_owner();
    let unrelated = event(
        &mut domain,
        2,
        Event::WatchStep { task: 7, step: 2, batch: 11, messages: Box::new([incident_verdict(999)]) },
    );
    assert!(has_request(&unrelated, ExpectedRequest::Wait));
    assert_eq!(read_incident(&mut domain), Err(DocumentMissing::Unknown));
    let malformed = event(
        &mut domain,
        2,
        Event::WatchStep {
            task: 7,
            step: 4,
            batch: 13,
            messages: Box::new([WatchMessage::InvalidTriage { child: 100 }]),
        },
    );
    assert!(has_request(&malformed, ExpectedRequest::Verdict));
    let noise = event(
        &mut domain,
        2,
        Event::WatchStep {
            task: 7,
            step: 3,
            batch: 12,
            messages: Box::new([WatchMessage::Triage { child: 100, verdict: TriageVerdict::Noise }]),
        },
    );
    assert!(has_request(&noise, ExpectedRequest::Wait));
    assert_eq!(read_incident(&mut domain), Err(DocumentMissing::Unknown));
    event(&mut domain, 3, Event::System(SystemEvent::Fact { service: service(), fact: fact(3, 20) }));
    let read = Token::new(502);
    event(
        &mut domain,
        3,
        Event::DocumentPrepare { token: read, subject: ClientSubject::Alert { project: 1, alert: 12 } },
    );
    let mut answer = event(&mut domain, 3, Event::ReadDocument { scope: ClientScope::Project(1), read, party: 1 });
    let Some(Request::ReadAnswered {
        answer: Ok(ClientDocument::Alert { alert, raised, state, fact: observed }), ..
    }) = answer.pop()
    else {
        panic!("noise still has an alert account")
    };
    assert_eq!(alert.number, 12);
    assert_eq!(raised, Wall::from_nanos(1_000_000_000));
    assert_eq!(state, IncidentState::Resolved);
    assert_eq!(observed, Some(fact(3, 20)));
    assert_eq!(read_incident(&mut domain), Err(DocumentMissing::Unknown));
}

#[test]
fn the_lead_is_unknown_until_the_same_step_outcome_and_a_conflict_never_replaces_it() {
    let mut domain = incident_owner();
    let selected = event(
        &mut domain,
        2,
        Event::WatchStep { task: 7, step: 2, batch: 11, messages: Box::new([incident_verdict(100)]) },
    );
    assert!(has_request(&selected, ExpectedRequest::Lead));
    let ClientDocument::Incident { incident, .. } = read_incident(&mut domain).expect("owner opened incident") else {
        panic!()
    };
    assert_eq!(incident.lead, None);
    let wrong = event(&mut domain, 2, Event::ProcedureDelegateOutcome { task: 7, step: 99, child: Some(999) });
    assert!(has_request(&wrong, ExpectedRequest::UnknownStep));
    event(&mut domain, 2, Event::ProcedureDelegateOutcome { task: 7, step: 2, child: Some(200) });
    let conflict = event(&mut domain, 2, Event::ProcedureDelegateOutcome { task: 7, step: 2, child: Some(999) });
    assert!(has_request(&conflict, ExpectedRequest::Conflict));
    let ClientDocument::Incident { incident, .. } = read_incident(&mut domain).expect("owner retained incident") else {
        panic!()
    };
    assert_eq!(incident.lead, Some(200));
    assert_eq!(incident.alerts.as_ref(), &[12]);
    assert_eq!(incident.watch, 7);
    // A distinct retained step naming an already-owned incident cannot replace its lead.
    event(
        &mut domain,
        2,
        Event::Restore {
            record: Record::Delegation(Delegation {
                task: 7,
                step: 3,
                project: 1,
                kind: DelegationKind::Lead { incident: 12 },
                child: None,
                settled: false,
            }),
        },
    );
    let other_step = event(&mut domain, 2, Event::ProcedureDelegateOutcome { task: 7, step: 3, child: Some(999) });
    assert!(has_request(&other_step, ExpectedRequest::Conflict));
    let ClientDocument::Incident { incident, .. } = read_incident(&mut domain).unwrap() else { panic!() };
    assert_eq!(incident.lead, Some(200));
}

#[test]
fn the_lead_step_and_incident_survive_a_cold_start_and_the_watch_ending() {
    use alloc::collections::BTreeMap;
    let mut domain = incident_owner();
    let mut rows = BTreeMap::new();
    let watch = Record::Watch(domain.watches.get(&7).expect("live W").clone());
    rows.insert(watch.key(), watch);
    // Independent committed receipts reconstruct the owner before and after the outcome.
    for request in &event(
        &mut domain,
        2,
        Event::WatchStep { task: 7, step: 2, batch: 11, messages: Box::new([incident_verdict(100)]) },
    ) {
        if let Request::Save { record } = request {
            rows.insert(record.key(), record.clone());
        }
    }
    let mut restarted = Domain::new(&limits());
    for record in rows.values() {
        event(&mut restarted, 2, Event::Restore { record: record.clone() });
    }
    let ClientDocument::Incident { incident, .. } = read_incident(&mut restarted).unwrap() else { panic!() };
    assert_eq!(incident.lead, None, "no lead is invented at a journal cut before the outcome");
    for request in &event(&mut restarted, 2, Event::ProcedureDelegateOutcome { task: 7, step: 2, child: Some(200) }) {
        if let Request::Save { record } = request {
            rows.insert(record.key(), record.clone());
        }
    }
    for request in &event(&mut restarted, 2, Event::StopWatch { task: 7 }) {
        if let Request::Erase { key } = request {
            rows.remove(key);
        }
    }
    assert!(!rows.contains_key(&RecordKey::Watch(7)), "watch ending is durable");
    let mut again = Domain::new(&limits());
    for record in rows.values() {
        event(&mut again, 2, Event::Restore { record: record.clone() });
    }
    let ClientDocument::Incident { incident, .. } = read_incident(&mut again).unwrap() else { panic!() };
    assert_eq!(incident.lead, Some(200));
    let conflict = event(&mut again, 2, Event::ProcedureDelegateOutcome { task: 7, step: 2, child: Some(300) });
    assert!(has_request(&conflict, ExpectedRequest::Conflict));
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ExpectedRequest {
    Triage,
    Lead,
    Wait,
    UnknownStep,
    Conflict,
    Verdict,
}

fn has_request(requests: &Queue<Request>, expected: ExpectedRequest) -> bool {
    for request in requests {
        let found = match request {
            Request::WatchDecision { decision, .. } => match decision {
                WatchDecision::Triage { .. } => Some(ExpectedRequest::Triage),
                WatchDecision::Lead { .. } => Some(ExpectedRequest::Lead),
                WatchDecision::Wait => Some(ExpectedRequest::Wait),
                WatchDecision::Hold => None,
            },
            Request::AssociationRefused { reason, .. } => match reason {
                AssociationRefusal::UnknownStep => Some(ExpectedRequest::UnknownStep),
                AssociationRefusal::Conflict => Some(ExpectedRequest::Conflict),
                AssociationRefusal::Verdict => Some(ExpectedRequest::Verdict),
                AssociationRefusal::Full | AssociationRefusal::Template => None,
            },
            Request::Document { .. }
            | Request::ReadAnswered { .. }
            | Request::Marked { .. }
            | Request::WatchInterest { .. }
            | Request::Closed { .. }
            | Request::Released { .. }
            | Request::Restarted { .. }
            | Request::Verdict { .. }
            | Request::Answer { .. }
            | Request::News { .. }
            | Request::Health { .. }
            | Request::Triage { .. }
            | Request::WakeProcedure { .. }
            | Request::Described { .. }
            | Request::Refused { .. }
            | Request::Make { .. }
            | Request::Outcome { .. }
            | Request::Save { .. }
            | Request::Erase { .. }
            | Request::System(..)
            | Request::Changed { .. } => None,
        };
        if found == Some(expected) {
            return true;
        }
    }
    false
}

#[test]
fn a_refused_lead_creation_keeps_its_step_and_never_accepts_a_later_child() {
    let mut domain = incident_owner();
    event(
        &mut domain,
        2,
        Event::WatchStep { task: 7, step: 2, batch: 11, messages: Box::new([incident_verdict(100)]) },
    );
    let receipt = event(&mut domain, 2, Event::ProcedureDelegateOutcome { task: 7, step: 2, child: None });
    let mut restarted = Domain::new(&limits());
    for request in &receipt {
        if let Request::Save { record } = request {
            event(&mut restarted, 2, Event::Restore { record: record.clone() });
        }
    }
    let retained = restarted.delegations.get(&(7, 2)).expect("refused creation retains its exact lead step");
    assert!(retained.settled);
    assert_eq!(retained.child, None);
    let later = event(&mut restarted, 2, Event::ProcedureDelegateOutcome { task: 7, step: 2, child: Some(200) });
    assert!(has_request(&later, ExpectedRequest::Conflict));
    let ClientDocument::Incident { incident, .. } = read_incident(&mut restarted).expect("retained incident") else {
        panic!()
    };
    assert_eq!(incident.lead, None);
}

#[test]
fn restored_effect_timers_wait_for_read_afresh() {
    let mut domain = Domain::new(&limits());
    let key = Key::procedure([1; 16], 7, 3);
    event(
        &mut domain,
        2,
        Event::Restore {
            record: Record::Outbox(Entry {
                project: Some(1),
                number: 3,
                key,
                effect: Effect { rule: Box::from(*b"checkout-errors"), until: 100 },
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
    let Some(Request::System(SystemRequest::FindSilence { .. })) = out.pop() else {
        panic!("system lookup resumes after read afresh");
    };
}

#[test]
fn valid_refuses_each_unrepresentable_limit() {
    let base = limits();
    assert!(valid(&base));
    for (condition, invalid) in [
        ("positive name bytes", Limits { name_bytes: 0, ..base }),
        ("positive answer bytes", Limits { answer_bytes: 0, ..base }),
        ("positive attempts", Limits { max_attempts: 0, ..base }),
        ("positive retry", Limits { retry_after_seconds: 0, ..base }),
        ("subscriber count", Limits { subscribers_per_alert: 9, ..base }),
        ("closing subscriptions and watches", Limits { subscriptions: u32::MAX, ..base }),
        ("closing effect count", Limits { effects: u32::MAX, ..base }),
        ("watched service product", Limits { services_per_watch: u32::MAX, ..base }),
        ("subscribed and watched service count", Limits { subscriptions: u32::MAX - 1, ..base }),
        ("incident product", Limits { alerts_per_batch: u32::MAX, ..base }),
        ("incident refresh count", Limits { alerts_per_batch: u32::MAX / 2 - 2, ..base }),
        (
            "step association count",
            Limits {
                watches: 2,
                alerts_per_batch: u32::MAX / 2,
                services_per_watch: 0,
                subscriptions: 0,
                subscribers_per_alert: 0,
                effects: 0,
                ..base
            },
        ),
        (
            "message subscription count",
            Limits {
                watches: 1,
                alerts_per_batch: u32::MAX - 2,
                services_per_watch: 0,
                subscriptions: 2,
                subscribers_per_alert: 0,
                effects: 0,
                ..base
            },
        ),
        (
            "message final slot",
            Limits {
                watches: 1,
                alerts_per_batch: u32::MAX - 1,
                services_per_watch: 0,
                subscriptions: 0,
                subscribers_per_alert: 0,
                effects: 0,
                ..base
            },
        ),
    ] {
        assert!(!valid(&invalid), "{condition}");
    }
}

#[test]
fn validity_does_not_price_retained_heap() {
    let limits = Limits { services: u32::MAX, name_bytes: u32::MAX, samples_per_service: u32::MAX, ..limits() };
    assert!(valid(&limits));
}
