#![expect(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::wildcard_enum_match_arm,
    clippy::match_like_matches_macro,
    reason = "test fixtures collect observed root outputs and locate their variants"
)]
#[path = "tests_client.rs"]
mod client_surface;
use crate as support_domain;
#[path = "tests_support.rs"]
mod support;
use crate::{Address, Answered, Ask, Event, Limits, infrastructure};
use jig_client::contract::{self, Delivery, DocumentText, HeldRole, Me, MeParty, Needs, PartyKind, WatchEvent};
use jig_client_view::boundary::Offset;
use skein_lib::{Duration, Env, List, Queue, RequestEnvelope, Time, Token, Wall};
use support::{Domain, JigOut, Request, new, step, take};
const JIG: jig_client_view::Limits = jig_client_view::Limits {
    watches: 4,
    notices: 2,
    notice_life: Duration::from_secs(5),
    linger: Duration::from_secs(5),
    save_delay: Duration::from_secs(1),
    intents: 2,
    fields: 4,

    objects: 2,
    sources: 8,
    reads: 4,
    parts: 10,
    delegates: 2,
    document_bytes: 65536,
    records: 2,
    kept: 4,
    drafts: 1,
    projects: 2,
    roles: 2,
    text: 32,
    rows: 2,
    segments: 4,
    accounts: jig_client::AccountCounts { accounts: 8, sources: 8, sign_ins: 8 },
    conversation_words: 2,
    navigation_entries: 8,
    updates_count: 99,
};
fn limits() -> Limits {
    Limits {
        jig: JIG,
        infrastructure: infrastructure::Limits {
            parts: 10,
            text: 32,
            objects: 12,
            watches: 24,
            reads: 24,
            requests: 4,
            rows: 2,
            segments: 4,
            retry: Duration::from_nanos(10),
            document_bytes: 65536,
        },
        observability: crate::observability::Limits {
            parts: 10,
            objects: 12,
            reads: 24,
            watches: 24,
            rows: 2,
            segments: 4,
            text: 32,
            retry: Duration::from_nanos(10),
            document_bytes: 65536,
        },
        client: client_limits(),
    }
}
fn env() -> Env<Limits> {
    Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits() }
}
fn drive(root: &mut Domain, event: Event) -> Vec<Request> {
    let mut out = Queue::uncapped();
    step(root, &env(), event, &mut out);
    out_iter(out)
}
fn out_iter(mut out: Queue<Request>) -> Vec<Request> {
    let mut rows = Vec::new();
    while let Some(row) = out.pop() {
        rows.push(row);
    }
    rows
}
fn taken(root: &mut Domain) -> Vec<Request> {
    let mut out = Queue::uncapped();
    take(root, &env(), &mut out);
    out_iter(out)
}
fn start(root: &mut Domain, kept: List<crate::store::Record>) -> Token {
    let mut rows = drive(root, Event::Client { event: jig_client::Event::Up { up: jig_client::Up::SignedIn } });
    rows.extend(drive(
        root,
        Event::Start {
            address: Address::Jig(jig_client_view::address::Address::Inbox { project: None }),
            kept,
            offset: Offset { minutes: 0 },
        },
    ));
    rows.iter()
        .find_map(|row| match row {
            Request::Jig(JigOut::Open { watch, subject: contract::Subject::Me }) => Some(*watch),
            _ => None,
        })
        .unwrap()
}
fn confirm(root: &mut Domain, watch: Token, party: u64) {
    drive(
        root,
        Event::Client { event: jig_client::Event::Up { up: jig_client::Up::Served { served: true, versions: None } } },
    );
    // Explicit synthetic identity and empty offers, supplied independently of the party's display name.
    let frame = Me {
        providers: List::with_capacity(0),
        providers_cut: false,
        key_bytes: 256,
        redirect_bytes: 4096,
        party: MeParty {
            number: party,
            kind: PartyKind::Person,
            login: DocumentText::complete(contract::Text::new(Box::from(*b"fixture-login"), 32).unwrap()),
            name: DocumentText::complete(contract::Text::new(Box::from(*b"Pat"), 32).unwrap()),
        },
        projects: List::with_capacity(0),
        needs: Needs { questions: 0, proposals: 0, escalations: 0, person_tasks: 0 },
        needs_total: 0,
        updates: contract::Count { value: 0, limited: false },
        results_seen: 0,
        cut: false,
        retention: Duration::from_secs(1000),
        deployment_role: Some(HeldRole::Policy { role: 1 }),
    };
    drive(
        root,
        Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Watched { watch, event: WatchEvent::Delivered(Delivery::Me(Box::new(frame))) },
            },
        },
    );
}
fn ready(seed: u64) -> Domain {
    let mut root = new(&limits(), seed);
    let watch = start(&mut root, List::with_capacity(0));
    confirm(&mut root, watch, 7);
    root
}
fn environment(revision: u64) -> infrastructure::Document {
    use infrastructure::documents::{Creation, Environment, Purpose};
    infrastructure::Document::Environment(Environment {
        project: 3,
        number: 5,
        pool: 1,
        name: Box::from(*b"preview"),
        revision: contract::Revision { value: revision },
        created_by: Creation { deployment: [7; 16], task: 9, origin: Purpose::Procedure { purpose: 2 }, purpose: 4 },
        ready_at: None,
        ready: None,
        cost: None,
        observed_until: None,
        end: Some(infrastructure::documents::End {
            revision: contract::Revision { value: revision },
            until: Wall::from_nanos(50_000_000_000),
        }),
    })
}
fn open_families(root: &mut Domain, rows: &[Request]) {
    for row in rows {
        let watch = match row {
            Request::Infrastructure(infrastructure::Out::Open { watch, .. })
            | Request::Observability(crate::observability::Out::Open { watch, .. }) => *watch,
            _ => continue,
        };
        drive(
            root,
            Event::Client {
                event: jig_client::Event::Up { up: jig_client::Up::Watched { watch, event: WatchEvent::Opened } },
            },
        );
    }
}
fn family_page(root: &mut Domain) -> Vec<Request> {
    drive(
        root,
        Event::Went {
            place: crate::Place::Address(Address::Infrastructure(infrastructure::Address::Environment {
                project: 3,
                environment: 5,
            })),
        },
    )
}
fn family_read(rows: &[Request]) -> Token {
    rows.iter()
        .find_map(|row| match row {
            Request::Infrastructure(infrastructure::Out::Read { read, .. }) => Some(*read),
            _ => None,
        })
        .unwrap()
}
fn family_watch(rows: &[Request]) -> Token {
    rows.iter()
        .find_map(|row| match row {
            Request::Infrastructure(infrastructure::Out::Open { watch, .. }) => Some(*watch),
            _ => None,
        })
        .unwrap()
}
fn load_environment(root: &mut Domain) -> skein_lib::Id<infrastructure::Object> {
    let rows = family_page(root);
    drive(
        root,
        Event::Infrastructure(infrastructure::Event::Read {
            read: family_read(&rows),
            result: infrastructure::ReadResult::Value(Box::new(environment(2))),
        }),
    );
    root.infrastructure().page().unwrap()
}
fn until(root: &mut Domain, object: Token) {
    drive(
        root,
        Event::Act {
            action: crate::Action::Infrastructure(infrastructure::Act::Until {
                object,
                revision: contract::Revision { value: 2 },
                value: Wall::from_nanos(100_000_000_000),
            }),
        },
    );
}
#[test]
fn an_end_move_names_current_revision_and_only_durable_answer_finishes_the_card() {
    let mut root = ready(41);
    let object = load_environment(&mut root);
    until(&mut root, object.token());
    drive(
        &mut root,
        Event::Act {
            action: crate::Action::Infrastructure(infrastructure::Act::MoveEnd {
                object: object.token(),
                revision: contract::Revision { value: 1 },
            }),
        },
    );
    assert_eq!(
        root.infrastructure().object(object).unwrap().card,
        infrastructure::Card::Refused {
            answer: infrastructure::Answer::Moved {
                environment: 5,
                revision: contract::Revision { value: 2 },
                until: Some(Wall::from_nanos(50_000_000_000))
            }
        }
    );
    assert!(!taken(&mut root).iter().any(|row| match row {
        Request::Send { .. } => true,
        _ => false,
    }));
    drive(
        &mut root,
        Event::Act {
            action: crate::Action::Infrastructure(infrastructure::Act::MoveEnd {
                object: object.token(),
                revision: contract::Revision { value: 2 },
            }),
        },
    );
    let rows = taken(&mut root);
    let (attempt, ask) = rows
        .iter()
        .find_map(|row| match row {
            Request::Send { attempt, ask: Ask::Infrastructure(ask), .. } => Some((*attempt, ask)),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        ask,
        &infrastructure::Ask::MoveEnd {
            project: 3,
            environment: 5,
            revision: contract::Revision { value: 2 },
            until: Wall::from_nanos(100_000_000_000)
        }
    );
    let answer = infrastructure::Answer::MovedEnd {
        environment: 5,
        revision: contract::Revision { value: 3 },
        until: Wall::from_nanos(100_000_000_000),
    };
    drive(&mut root, support::answer(attempt, RequestEnvelope::Again, Some(Answered::Infrastructure(answer.clone()))));
    assert!(match root.infrastructure().object(object).unwrap().card {
        infrastructure::Card::Deciding { .. } => true,
        _ => false,
    });
    // A retry envelope did not deliver this body to the child.
    let mut future = env();
    future.now = Time::from_nanos(80);
    let mut out = Queue::uncapped();
    take(&mut root, &future, &mut out);
    let attempt = out_iter(out)
        .iter()
        .find_map(|row| match row {
            Request::Send { attempt, .. } => Some(*attempt),
            _ => None,
        })
        .unwrap();
    drive(&mut root, support::answer(attempt, RequestEnvelope::Final, Some(Answered::Infrastructure(answer.clone()))));
    assert_eq!(root.infrastructure().object(object).unwrap().card, infrastructure::Card::Done { answer });
}
#[test]
fn revision_changes_clear_drafts_and_old_watch_documents_cannot_reverse_facts() {
    let mut root = ready(42);
    let rows = family_page(&mut root);
    let watch = family_watch(&rows);
    drive(
        &mut root,
        Event::Infrastructure(infrastructure::Event::Read {
            read: family_read(&rows),
            result: infrastructure::ReadResult::Value(Box::new(environment(2))),
        }),
    );
    let object = root.infrastructure().page().unwrap();
    until(&mut root, object.token());
    drive(
        &mut root,
        Event::Infrastructure(infrastructure::Event::Watch {
            watch,
            event: infrastructure::WatchEvent::Delivered(Box::new(environment(3))),
        }),
    );
    assert_eq!(root.infrastructure().object(object).unwrap().draft, None);
    drive(
        &mut root,
        Event::Infrastructure(infrastructure::Event::Watch {
            watch,
            event: infrastructure::WatchEvent::Delivered(Box::new(environment(1))),
        }),
    );
    assert_eq!(
        root.infrastructure().object(object).unwrap().value,
        infrastructure::Value::Known(Box::new(environment(3)))
    );
}
#[test]
fn abandoned_read_and_closing_watch_tokens_cannot_recreate_a_page() {
    let mut root = ready(43);
    let rows = family_page(&mut root);
    let watch = family_watch(&rows);
    let read = family_read(&rows);
    open_families(&mut root, &rows);
    let rows = drive(
        &mut root,
        Event::Went {
            place: crate::Place::Address(Address::Jig(jig_client_view::address::Address::Inbox { project: None })),
        },
    );
    assert!(rows.contains(&Request::Infrastructure(infrastructure::Out::Close { watch })));
    drive(
        &mut root,
        Event::Infrastructure(infrastructure::Event::Watch {
            watch,
            event: infrastructure::WatchEvent::Delivered(Box::new(environment(2))),
        }),
    );
    drive(
        &mut root,
        Event::Infrastructure(infrastructure::Event::Ended { watch, end: infrastructure::WatchEnd::Closed }),
    );
    drive(
        &mut root,
        Event::Infrastructure(infrastructure::Event::Read {
            read,
            result: infrastructure::ReadResult::Value(Box::new(environment(2))),
        }),
    );
    assert_eq!(root.infrastructure().page(), None);
    assert!(
        drive(
            &mut root,
            Event::Infrastructure(infrastructure::Event::Ended { watch, end: infrastructure::WatchEnd::Closed })
        )
        .is_empty()
    );
}
#[test]
fn whole_document_limits_keep_unknown_owner_facts_unknown() {
    let document = environment(2);
    assert!(document.fits(&limits().infrastructure));
    let mut bound = limits().infrastructure;
    bound.text = 2;
    assert!(!document.fits(&bound));
    bound.text = 32;
    bound.document_bytes = 1;
    assert!(!document.fits(&bound));
    let mut root = ready(44);
    let object = load_environment(&mut root);
    assert_eq!(root.infrastructure().object(object).unwrap().value, infrastructure::Value::Known(Box::new(document)));
    let mut bound = limits();
    bound.jig.kept = bound.client.domain.requests.requests - 1;
    assert_eq!(crate::worst_case(&bound), None);
}
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the same exhaustive scoped lifecycle is checked independently for both connector families"
)]
fn both_connectors_route_all_parts_in_the_supplied_scope_and_leave_resources_in_words() {
    for connector in
        [jig_ops_domain_connectors::Connector::Infrastructure, jig_ops_domain_connectors::Connector::Observability]
    {
        let mut root = ready(45);
        let rows = drive(
            &mut root,
            Event::Went { place: crate::Place::Address(Address::Jig(jig_client_view::address::Address::Deployment)) },
        );
        let watch = rows
            .iter()
            .find_map(|row| match row {
                Request::Jig(JigOut::Open { watch, subject: contract::Subject::Deployment }) => Some(*watch),
                _ => None,
            })
            .unwrap();
        let number = jig_ops_domain_connectors::number(connector);
        let parts = connector_parts(number);
        let expected = parts.clone();
        let rows = drive(
            &mut root,
            Event::Client {
                event: jig_client::Event::Up {
                    up: jig_client::Up::Watched {
                        watch,
                        event: WatchEvent::Delivered(Delivery::Deployment(Box::new(contract::Deployment {
                            hosts: List::with_capacity(0),
                            accounts: List::with_capacity(0),
                            capacity: List::with_capacity(0),
                            proposals: 0,
                            refusals: List::with_capacity(0),
                            parts,
                        }))),
                    },
                },
            },
        );
        assert_eq!(
            rows.iter()
                .filter(|row| match row {
                    Request::Infrastructure(infrastructure::Out::Read { .. })
                    | Request::Observability(crate::observability::Out::Read { .. }) => true,
                    _ => false,
                })
                .count(),
            6
        );

        for part in &expected {
            match connector {
                jig_ops_domain_connectors::Connector::Infrastructure => {
                    assert!(root.infrastructure().contains(part));
                }
                jig_ops_domain_connectors::Connector::Observability => {
                    assert!(root.observability().contains(part));
                }
            }
        }
        for row in &rows {
            match row {
                Request::Infrastructure(infrastructure::Out::Read { subject, .. }) => {
                    assert_eq!(
                        subject.scope,
                        match subject.kind {
                            infrastructure::Kind::Project(project) => infrastructure::Scope::Project(project),
                            _ => infrastructure::Scope::Deployment,
                        }
                    );
                }
                Request::Observability(crate::observability::Out::Read { subject, .. }) => {
                    assert_eq!(
                        subject.scope,
                        match subject.kind {
                            crate::observability::Kind::Project(project) =>
                                crate::observability::Scope::Project(project),
                            _ => crate::observability::Scope::Deployment,
                        }
                    );
                }
                _ => {}
            }
        }
        open_families(&mut root, &rows);
        let rows = drive(
            &mut root,
            Event::Went {
                place: crate::Place::Address(Address::Jig(jig_client_view::address::Address::Inbox { project: None })),
            },
        );
        assert_eq!(
            rows.iter()
                .filter(|row| match row {
                    Request::Infrastructure(infrastructure::Out::Close { .. })
                    | Request::Observability(crate::observability::Out::Close { .. }) => true,
                    _ => false,
                })
                .count(),
            6
        );
        for part in &expected {
            match connector {
                jig_ops_domain_connectors::Connector::Infrastructure => {
                    assert!(!root.infrastructure().contains(part));
                }
                jig_ops_domain_connectors::Connector::Observability => {
                    assert!(!root.observability().contains(part));
                }
            }
        }
    }
}
#[test]
fn incident_facts_keep_cuts_and_unknown_health_and_never_turn_a_watch_into_a_lead() {
    use crate::observability::{
        self,
        documents::{Incident, Service, State},
    };
    let mut root = ready(46);
    let rows = drive(
        &mut root,
        Event::Went {
            place: crate::Place::Address(Address::Observability(observability::Address::Incident {
                project: 3,
                incident: 8,
            })),
        },
    );
    let read = rows
        .iter()
        .find_map(|row| match row {
            Request::Observability(observability::Out::Read { read, .. }) => Some(*read),
            _ => None,
        })
        .unwrap();
    let watch = rows
        .iter()
        .find_map(|row| match row {
            Request::Observability(observability::Out::Open { watch, .. }) => Some(*watch),
            _ => None,
        })
        .unwrap();
    let mut incident = Incident {
        project: 3,
        number: 8,
        revision: contract::Revision { value: 2 },
        watch: 99,
        raised: Wall::EPOCH,
        service: Service { environment: Box::from(*b"prod"), name: Box::from(*b"api") },
        severity: 1,
        state: State::Alerting,
        timeline: List::with_capacity(2),
        cut: true,
        health: None,
        lead: None,
        proposals: List::with_capacity(2),
    };
    let document = observability::Document::Incident(incident.clone());
    assert!(document.objects(2).is_empty());
    drive(
        &mut root,
        Event::Observability(observability::Event::Read {
            read,
            result: observability::ReadResult::Value(Box::new(document)),
        }),
    );
    let id = root.observability().page().unwrap();
    incident.revision = contract::Revision { value: 3 };
    incident.state = State::Resolved;
    let resolved = observability::Document::Incident(incident.clone());
    drive(
        &mut root,
        Event::Observability(observability::Event::Watch {
            watch,
            event: observability::WatchEvent::Delivered(Box::new(resolved.clone())),
        }),
    );
    incident.revision = contract::Revision { value: 1 };
    incident.state = State::Alerting;
    drive(
        &mut root,
        Event::Observability(observability::Event::Watch {
            watch,
            event: observability::WatchEvent::Delivered(Box::new(observability::Document::Incident(incident))),
        }),
    );
    assert_eq!(root.observability().object(id).unwrap().value, observability::Value::Known(Box::new(resolved)));
}

fn incidents_naming_sections(lead: Option<u64>) -> [crate::observability::Document; 2] {
    use crate::observability::{documents::*, native};
    let incident = Incident {
        project: 3,
        number: 8,
        revision: contract::Revision { value: 2 },
        watch: 99,
        raised: Wall::EPOCH,
        service: Service { environment: Box::from(*b"prod"), name: Box::from(*b"api") },
        severity: 1,
        state: State::Alerting,
        timeline: List::with_capacity(0),
        cut: false,
        health: None,
        lead,
        proposals: List::with_capacity(0),
    };
    let native = native::Document {
        subject: Subject { scope: Scope::Project(3), kind: Kind::Incident(8) },
        value: native::Value::Incident {
            incident: native::Incident {
                project: 3,
                number: 8,
                watch: 99,
                lead,
                alerts: Box::from([]),
                raised: Wall::EPOCH,
                service: incident.service.clone(),
                severity: 1,
                state: State::Alerting,
                timeline: List::with_capacity(0),
                cut: false,
            },
            fact: None,
        },
    };
    [Document::Incident(incident), Document::Native(Box::new(native))]
}

#[test]
fn an_incident_with_a_lead_names_its_plan_section() {
    let expected =
        [jig_client_view::page::Section::Task { task: 9, section: jig_client_view::address::TaskSection::Plan }];
    for (name, document) in ["incident", "native"].into_iter().zip(incidents_naming_sections(Some(9))) {
        assert_eq!(document.sections().as_slice(), expected, "{name}");
    }
}

#[test]
fn an_incident_without_a_lead_names_no_sections() {
    for (name, document) in ["incident", "native"].into_iter().zip(incidents_naming_sections(None)) {
        assert_eq!(document.sections().as_slice(), [], "{name}");
    }
}
#[test]
fn application_navigation_is_forwarded_before_start_and_retains_order() {
    use jig_client_view::page::navigation::{Destination, Navigation};
    let mut root = new(&limits(), 47);
    let mut navigation = Navigation::new(2);
    navigation.entries.push(Destination::Jig(jig_client_view::address::Address::Inbox { project: None })).unwrap();
    navigation
        .entries
        .push(Destination::Application {
            owner: jig_ops_domain_connectors::number(jig_ops_domain_connectors::Connector::Observability),
            number: 1,
        })
        .unwrap();
    crate::configure_navigation(&mut root, navigation).unwrap();
    assert_eq!(
        root.jig().navigation().entries.as_slice()[1],
        Destination::Application {
            owner: jig_ops_domain_connectors::number(jig_ops_domain_connectors::Connector::Observability),
            number: 1
        }
    );
    start(&mut root, List::with_capacity(0));
    assert!(crate::configure_navigation(&mut root, Navigation::new(0)).is_err());
}

fn connector_parts(number: u16) -> List<contract::Part> {
    let mut parts = List::with_capacity(9);
    for part in [
        contract::Part::Connector { connector: number },
        contract::Part::Resource {
            connector: number,
            path: {
                let mut path = List::with_capacity(1);
                path.push(Box::<[u8]>::from(*b"service")).unwrap();
                path
            },
        },
        contract::Part::Kind { connector: number, kind: 1 },
        contract::Part::Panel { connector: number, task: 9 },
        contract::Part::Result { connector: number, task: 9 },
        contract::Part::Effect {
            connector: number,
            proposal: contract::ProposalKey { proposer: contract::Proposer::Task(9), number: 2 },
        },
        contract::Part::Hold { connector: number, task: 9 },
        contract::Part::Project { connector: number, project: 3 },
        contract::Part::Deployment { connector: number },
    ] {
        parts.push(part).unwrap();
    }
    parts
}
#[test]
fn an_absent_owner_end_cannot_become_a_revision_naming_request() {
    let mut root = ready(48);
    let rows = family_page(&mut root);
    let mut document = environment(2);
    let infrastructure::Document::Environment(environment) = &mut document else { unreachable!() };
    environment.end = None;
    drive(
        &mut root,
        Event::Infrastructure(infrastructure::Event::Read {
            read: family_read(&rows),
            result: infrastructure::ReadResult::Value(Box::new(document)),
        }),
    );
    let object = root.infrastructure().page().unwrap();
    until(&mut root, object.token());
    assert_eq!(root.infrastructure().object(object).unwrap().draft, None);
    drive(
        &mut root,
        Event::Act {
            action: crate::Action::Infrastructure(infrastructure::Act::MoveEnd {
                object: object.token(),
                revision: contract::Revision { value: 2 },
            }),
        },
    );
    assert!(!taken(&mut root).iter().any(|row| match row {
        Request::Send { .. } => true,
        _ => false,
    }));
}

#[test]
fn observability_project_lists_are_admitted_and_priced_whole() {
    use crate::observability::documents::{Document, Kind, PartDocument, Scope, Service, ServiceState, Subject};
    let mut services = List::with_capacity(2);
    services
        .push(ServiceState {
            service: Service { environment: Box::from(*b"production"), name: Box::from(*b"checkout") },
            health: None,
        })
        .unwrap();
    let mut part = PartDocument {
        subject: Subject { scope: Scope::Project(1), kind: Kind::Project(1) },
        revision: contract::Revision { value: 1 },
        service: None,
        health: None,
        incident: None,
        incidents: List::with_capacity(2),
        services,
        effects: List::with_capacity(2),
        watches: None,
        read_limit: None,
        incident_limit: None,
    };
    assert!(Document::Part(Box::new(part.clone())).fits(&limits().observability));
    part.services = List::with_capacity(3);
    assert!(!Document::Part(Box::new(part)).fits(&limits().observability));
}
#[test]
fn an_outbox_description_can_be_absent_without_becoming_a_proposal() {
    use crate::infrastructure::documents::{
        Creation, Document, Effect, EffectAction, EffectState, Kind, PartDocument, Purpose, Scope, Subject,
    };
    let mut effects = List::with_capacity(2);
    effects
        .push(Effect {
            entry: 900,
            task: 17,
            project: Some(1),
            origin: Creation { deployment: [1; 16], task: 17, origin: Purpose::Procedure { purpose: 3 }, purpose: 3 },
            description: None,
            state: EffectState::Kept,
            attempt: 0,
            deadline: Wall::EPOCH,
            action: EffectAction::CreateEnvironment { environment: 7, until: Wall::EPOCH, price: 40 },
        })
        .unwrap();
    let part = PartDocument {
        subject: Subject { scope: Scope::Project(1), kind: Kind::Outbox(900) },
        revision: contract::Revision { value: 1 },
        panel: None,
        description: None,
        effect: None,
        resources: List::with_capacity(2),
        effects,
        drift: List::with_capacity(2),
        environments: List::with_capacity(2),
        pools: List::with_capacity(2),
        backend: None,
        made: None,
        proposal_limit: None,
        staged_limit: None,
    };
    assert!(Document::Part(Box::new(part)).fits(&limits().infrastructure));
}

#[test]
fn an_entrance_refusal_finishes_only_its_original_child_and_survives_a_duplicate_final() {
    let mut root = ready(5091);
    let object = load_environment(&mut root);
    until(&mut root, object.token());
    drive(
        &mut root,
        Event::Act {
            action: crate::Action::Infrastructure(infrastructure::Act::MoveEnd {
                object: object.token(),
                revision: contract::Revision { value: 2 },
            }),
        },
    );
    let attempt = taken(&mut root)
        .iter()
        .find_map(|row| match row {
            Request::Send { attempt, .. } => Some(*attempt),
            _ => None,
        })
        .unwrap();
    let refusal = contract::EntranceRefusal::new(contract::Refusal::Authority {
        lacking: contract::Lacking::Budget { needed: 10, left: 3 },
        proposable: None,
    })
    .unwrap();
    let answer = infrastructure::Answer::EntranceRefused(refusal);
    drive(&mut root, support::answer(attempt, RequestEnvelope::Again, Some(Answered::Infrastructure(answer.clone()))));
    assert!(match root.infrastructure().object(object).unwrap().card {
        infrastructure::Card::Deciding { .. } => true,
        _ => false,
    });
    let mut future = env();
    future.now = Time::from_nanos(80);
    let mut out = Queue::uncapped();
    take(&mut root, &future, &mut out);
    let attempt = out_iter(out)
        .iter()
        .find_map(|row| match row {
            Request::Send { attempt, .. } => Some(*attempt),
            _ => None,
        })
        .unwrap();
    drive(&mut root, support::answer(attempt, RequestEnvelope::Final, Some(Answered::Infrastructure(answer.clone()))));
    let card = infrastructure::Card::Refused { answer };
    assert_eq!(root.infrastructure().object(object).unwrap().card, card);
    drive(
        &mut root,
        support::answer(attempt, RequestEnvelope::Final, Some(Answered::Infrastructure(infrastructure::Answer::Gone))),
    );
    assert_eq!(root.infrastructure().object(object).unwrap().card, card);
}

#[test]
fn outbox_subjects_copy_the_supplied_scope_and_numeric_entry_independently_of_effects() {
    use crate::{infrastructure as infra, observability as obs};
    let proposal = contract::ProposalKey { proposer: contract::Proposer::Task(17), number: 17 };
    let effect = contract::Part::Effect { connector: 2, proposal };
    for scope in [contract::PartScope::Project(91), contract::PartScope::Deployment] {
        let part = contract::Part::Outbox { connector: 2, scope, entry: 17 };
        let infrastructure = infra::documents::subject(&part, infra::Scope::Project(3)).unwrap();
        let observability = obs::documents::subject(&part, obs::Scope::Project(3)).unwrap();
        assert_eq!(infrastructure.kind, infra::Kind::Outbox(17));
        assert_eq!(observability.kind, obs::Kind::Outbox(17));
        match scope {
            contract::PartScope::Project(project) => {
                assert_eq!(infrastructure.scope, infra::Scope::Project(project));
                assert_eq!(observability.scope, obs::Scope::Project(project));
            }
            contract::PartScope::Deployment => {
                assert_eq!(infrastructure.scope, infra::Scope::Deployment);
                assert_eq!(observability.scope, obs::Scope::Deployment);
            }
        }
        assert_ne!(infra::documents::subject(&effect, infrastructure.scope), Some(infrastructure));
        assert_ne!(obs::documents::subject(&effect, observability.scope), Some(observability));
    }
}

#[test]
fn resource_parts_bound_allocated_segments_and_whole_segment_bytes_in_both_connectors() {
    let limits = limits();
    for connector in [1, 2] {
        let mut valid = List::with_capacity(2);
        valid.push(Box::<[u8]>::from([0xFF; 32])).unwrap();
        valid.push(Box::<[u8]>::from(*b"/\0")).unwrap();
        let empty_overallocated = List::with_capacity(5);
        let mut too_long = List::with_capacity(1);
        too_long.push(Box::<[u8]>::from([b'x'; 33])).unwrap();
        for (path, accepted) in [(valid, true), (empty_overallocated, false), (too_long, false)] {
            let part = contract::Part::Resource { connector, path };
            if connector == 1 {
                let mut side = crate::observability::Side::new(&limits.observability, 61);
                let mut out = Queue::uncapped();
                crate::observability::step(
                    &mut side,
                    &Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.observability },
                    crate::observability::Event::Show {
                        part: part.clone(),
                        scope: crate::observability::Scope::Project(1),
                    },
                    &mut out,
                );
                assert_eq!(side.contains(&part), accepted);
                if accepted {
                    assert!(out.is_empty());
                } else {
                    let Some(crate::observability::Request::Out(crate::observability::Out::Fact { fact })) = out.pop()
                    else {
                        panic!("rejected resource emits its Full fact");
                    };
                    assert_eq!(fact, crate::observability::Fact::Full);
                    assert!(out.is_empty());
                }
            } else {
                let mut side = infrastructure::Side::new(&limits.infrastructure, 62);
                let mut out = Queue::uncapped();
                infrastructure::step(
                    &mut side,
                    &Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.infrastructure },
                    infrastructure::Event::Show { part: part.clone(), scope: infrastructure::Scope::Project(1) },
                    &mut out,
                );
                assert_eq!(side.contains(&part), accepted);
                if accepted {
                    assert!(out.is_empty());
                } else {
                    let Some(infrastructure::Request::Out(infrastructure::Out::Fact { fact })) = out.pop() else {
                        panic!("rejected resource emits its Full fact");
                    };
                    assert_eq!(fact, infrastructure::Fact::Full);
                    assert!(out.is_empty());
                }
            }
        }
    }
}

#[test]
fn native_effect_parts_keep_connector_numbers_separate_from_maker_subjects() {
    use crate::observability as obs;
    let limits = limits();
    for connector in [1, 2, u16::MAX] {
        for number in [0, u64::MAX] {
            let native = contract::Part::OwnerEffect { connector, proposal: number };
            assert_eq!(crate::parts::connector(&native), connector);
            for proposer in [contract::Proposer::Task(number), contract::Proposer::Party(number)] {
                let key = contract::ProposalKey { proposer, number };
                let maker_effect = contract::Part::Effect { connector, proposal: key };
                assert_ne!(native, maker_effect);
                for project in [0, u32::MAX] {
                    assert_eq!(
                        infrastructure::documents::subject(&native, infrastructure::Scope::Project(project)),
                        Some(infrastructure::Subject {
                            scope: infrastructure::Scope::Project(project),
                            kind: infrastructure::Kind::OwnerEffect(number),
                        })
                    );
                    assert_eq!(obs::documents::subject(&native, obs::Scope::Project(project)), None);
                    assert_eq!(
                        infrastructure::documents::subject(&maker_effect, infrastructure::Scope::Project(project)),
                        Some(infrastructure::Subject {
                            scope: infrastructure::Scope::Project(project),
                            kind: infrastructure::Kind::Effect(key)
                        })
                    );
                    assert_eq!(
                        obs::documents::subject(&maker_effect, obs::Scope::Project(project)),
                        Some(obs::Subject { scope: obs::Scope::Project(project), kind: obs::Kind::Effect(key) })
                    );
                }
            }
            assert_eq!(infrastructure::documents::subject(&native, infrastructure::Scope::Deployment), None);
            assert_eq!(obs::documents::subject(&native, obs::Scope::Deployment), None);
            let mut infra = infrastructure::Side::new(&limits.infrastructure, 82);
            let mut infra_out = Queue::uncapped();
            let infra_env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.infrastructure };
            infrastructure::step(
                &mut infra,
                &infra_env,
                infrastructure::Event::Show { part: native.clone(), scope: infrastructure::Scope::Deployment },
                &mut infra_out,
            );
            assert!(infra.contains(&native));
            assert!(infra.part(&native).is_none());
            assert!(infra_out.is_empty());
            infrastructure::step(&mut infra, &infra_env, infrastructure::Event::PartsLeft, &mut infra_out);
            assert!(!infra.contains(&native));
            assert!(infra_out.is_empty());
            let mut side = obs::Side::new(&limits.observability, 83);
            let mut out = Queue::uncapped();
            let side_env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.observability };
            obs::step(
                &mut side,
                &side_env,
                obs::Event::Show { part: native.clone(), scope: obs::Scope::Deployment },
                &mut out,
            );
            assert!(side.contains(&native));
            assert!(side.part(&native).is_none());
            assert!(out.is_empty());
            obs::step(&mut side, &side_env, obs::Event::PartsLeft, &mut out);
            assert!(!side.contains(&native));
            assert!(out.is_empty());
        }
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one whole native environment request exercises document, draft, journal and opaque identity"
)]
fn native_environment_names_request_revision_and_whole_width_end_survive_the_client_request() {
    use infrastructure::native as n;
    let environment = n::Environment { pool: Box::from(*b"Q.pool"), name: Box::from(*b"preview") };
    let subject = infrastructure::Subject {
        scope: infrastructure::Scope::Project(3),
        kind: infrastructure::Kind::NativeEnvironment(environment.clone()),
    };
    let document = infrastructure::Document::Native(Box::new(n::Document {
        subject: subject.clone(),
        value: n::Value::Environment {
            environment: environment.clone(),
            end_revision: 0,
            fact: n::EnvironmentFact {
                created_by: n::Key {
                    deployment: [7; 16],
                    task: 9,
                    origin: n::Purpose::Procedure { purpose: 2 },
                    purpose: 4,
                },
                ready_at: 5,
                until: 50,
                price: 12,
            },
            end: None,
            pool: None,
        },
    }));
    assert!(document.fits(&limits().infrastructure));
    let mut root = ready(512);
    let requests = drive(
        &mut root,
        Event::Went {
            place: crate::Place::Address(Address::Infrastructure(infrastructure::Address::NativeEnvironment {
                project: 3,
                environment: environment.clone(),
            })),
        },
    );
    let read = requests
        .iter()
        .find_map(|request| match request {
            Request::Infrastructure(infrastructure::Out::Read { read, .. }) => Some(*read),
            _ => None,
        })
        .expect("native read");
    drive(
        &mut root,
        Event::Infrastructure(infrastructure::Event::Read {
            read,
            result: infrastructure::ReadResult::Value(Box::new(document.clone())),
        }),
    );
    let id = root.infrastructure().find(subject.clone()).expect("opaque subject retained");
    assert_eq!(
        root.infrastructure().object(id).expect("named object").value,
        infrastructure::Value::Known(Box::new(document))
    );
    drive(
        &mut root,
        Event::Act {
            action: crate::Action::Infrastructure(infrastructure::Act::NativeUntil {
                object: id.token(),
                revision: contract::Revision { value: 0 },
                value: u64::MAX,
            }),
        },
    );
    let requests = drive(
        &mut root,
        Event::Act {
            action: crate::Action::Infrastructure(infrastructure::Act::NativeMoveEnd {
                object: id.token(),
                revision: contract::Revision { value: 0 },
            }),
        },
    );
    assert!(requests.is_empty(), "the table journals before sending");
    let requests = taken(&mut root);
    assert!(requests.iter().any(|request| {
        match request {
            Request::Save { record: crate::store::Record::Unlinked(record) } => {
                crate::wire::ask_value(match &record.request.request {
                    jig_client::Ask::Family { record: value, .. } => &value.bytes,
                    jig_client::Ask::Jig(_) => panic!("family ask"),
                })
                .unwrap()
                    == (infrastructure::Ask::NativeMoveEnd {
                        object: id.token(),
                        project: 3,
                        environment: environment.clone(),
                        revision: contract::Revision { value: 0 },
                        until: u64::MAX,
                    })
            }
            _ => false,
        }
    }));
    assert_ne!(
        subject,
        infrastructure::Subject {
            scope: infrastructure::Scope::Project(3),
            kind: infrastructure::Kind::Environment(0)
        }
    );
    let mut different = environment.clone();
    different.pool = Box::from(*b"P.pool");
    assert!(
        root.infrastructure()
            .find(infrastructure::Subject {
                scope: infrastructure::Scope::Project(3),
                kind: infrastructure::Kind::NativeEnvironment(different)
            })
            .is_none()
    );
}

#[test]
fn infrastructure_reopens_a_dropped_watch_after_the_link_goes_offline_and_live() {
    let mut root = ready(61);
    let initial = family_page(&mut root);
    let old = family_watch(&initial);
    drive(
        &mut root,
        Event::Infrastructure(infrastructure::Event::Read {
            read: family_read(&initial),
            result: infrastructure::ReadResult::Value(Box::new(environment(2))),
        }),
    );
    drive(
        &mut root,
        Event::Infrastructure(infrastructure::Event::Ended { watch: old, end: infrastructure::WatchEnd::Dropped }),
    );
    let offline = support::offline(&root);
    drive(&mut root, offline);
    let id = root.infrastructure().page().unwrap();
    let mut out = Queue::uncapped();
    support::reconnect(&mut root, &env(), &mut out);
    let rows = out_iter(out);
    let watch = family_watch(&rows);
    assert_ne!(old, watch, "the retired watch epoch is not reused");
    let subject = root.infrastructure().object(id).unwrap().subject.clone();
    assert!(rows.iter().any(|row| match row {
        Request::Infrastructure(infrastructure::Out::Open { subject: got, .. }) => got == &subject,
        _ => false,
    }));
    drive(
        &mut root,
        Event::Infrastructure(infrastructure::Event::Ended { watch: old, end: infrastructure::WatchEnd::NotAllowed }),
    );
    let document = environment(3);
    drive(
        &mut root,
        Event::Infrastructure(infrastructure::Event::Watch {
            watch,
            event: infrastructure::WatchEvent::Delivered(Box::new(document.clone())),
        }),
    );
    assert_eq!(root.infrastructure().object(id).unwrap().value, infrastructure::Value::Known(Box::new(document)));
}

#[test]
fn observability_reopens_a_dropped_watch_after_the_link_goes_offline_and_live() {
    use crate::observability;
    let mut root = ready(62);
    let initial = drive(
        &mut root,
        Event::Went {
            place: crate::Place::Address(Address::Observability(observability::Address::Incidents {
                project: 3,
                open: true,
            })),
        },
    );
    let old = initial
        .iter()
        .find_map(|row| match row {
            Request::Observability(observability::Out::Open { watch, .. }) => Some(*watch),
            _ => None,
        })
        .unwrap();
    drive(
        &mut root,
        Event::Observability(observability::Event::Ended { watch: old, end: observability::WatchEnd::Dropped }),
    );
    let offline = support::offline(&root);
    drive(&mut root, offline);
    let id = root.observability().page().unwrap();
    let mut out = Queue::uncapped();
    support::reconnect(&mut root, &env(), &mut out);
    let rows = out_iter(out);
    let watch = rows
        .iter()
        .find_map(|row| match row {
            Request::Observability(observability::Out::Open { watch, .. }) => Some(*watch),
            _ => None,
        })
        .unwrap();
    assert_ne!(old, watch, "the retired watch epoch is not reused");
    let subject = root.observability().object(id).unwrap().subject;
    assert!(rows.iter().any(|row| match row {
        Request::Observability(observability::Out::Open { subject: got, .. }) => got == &subject,
        _ => false,
    }));
    drive(
        &mut root,
        Event::Observability(observability::Event::Ended { watch: old, end: observability::WatchEnd::NotAllowed }),
    );
    let document = observability::Document::Incidents {
        subject,
        revision: contract::Revision { value: 3 },
        incidents: List::with_capacity(2),
    };
    drive(
        &mut root,
        Event::Observability(observability::Event::Watch {
            watch,
            event: observability::WatchEvent::Delivered(Box::new(document.clone())),
        }),
    );
    assert_eq!(root.observability().object(id).unwrap().value, observability::Value::Known(Box::new(document)));
}

fn client_limits() -> jig_client::Limits {
    support::client_limits()
}
