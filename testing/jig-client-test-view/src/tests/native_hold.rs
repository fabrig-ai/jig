use super::*;
use jig_client_test_domain::connector_values as owner;

pub(super) fn limits() -> jig_client_test_domain::Limits {
    let mut limits = super::limits();
    limits.jig.document_bytes = 65536;
    limits.first.connector.rows = 1;
    limits.first.connector.paths = 1;
    limits.first.connector.segments = 2;
    limits.first.connector.segment_bytes = 16;
    limits
}
pub(super) fn drive(root: &mut Domain, event: Event) -> Vec<Request> {
    transport::drive(root, event, &limits())
}
pub(super) fn ready() -> Domain {
    let mut root = jig_client_test_domain::unlinked(&limits(), 43);
    let mut rows = drive(&mut root, Event::Client { event: jig_client::Event::Up { up: jig_client::Up::SignedIn } });
    rows.extend(drive(
        &mut root,
        Event::Start {
            address: Address::Jig(jig_client_view::address::Address::Inbox { project: None }),
            kept: List::with_capacity(0),
            offset: jig_client_view::boundary::Offset { minutes: 0 },
        },
    ));
    let watch = rows
        .iter()
        .find_map(|row| match row {
            Request::Client {
                request:
                    jig_client::Request::Down {
                        down: jig_client::Down::Open { watch, subject: jig_client::Watchable::Jig(Subject::Me) },
                    },
            } => Some(*watch),
            _ => None,
        })
        .unwrap();
    drive(
        &mut root,
        Event::Client { event: jig_client::Event::Up { up: jig_client::Up::Served { served: true, versions: None } } },
    );
    drive(
        &mut root,
        Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::FamilyServed { family: 1, versions: Some(VersionRange { first: 2, last: 2 }) },
            },
        },
    );
    drive(
        &mut root,
        Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Watched {
                    watch,
                    event: WatchEvent::Delivered(Delivery::Me(Box::new(Me {
                        providers: List::with_capacity(0),
                        providers_cut: false,
                        key_bytes: 256,
                        redirect_bytes: 4096,
                        words_bytes: 65_536,
                        amendment_bytes: 65_536,
                        party: MeParty {
                            number: 7,
                            kind: PartyKind::Person,
                            login: DocumentText::complete(text(b"login")),
                            name: DocumentText::complete(text(b"Pat")),
                        },
                        projects: List::with_capacity(0),
                        needs: Needs { questions: 0, proposals: 0, escalations: 0, person_tasks: 0 },
                        needs_total: 0,
                        results_seen: 0,
                        cut: false,
                        updates: Count { value: 0, limited: false },
                        retention: Duration::from_secs(1000),
                        deployment_role: None,
                    }))),
                },
            },
        },
    );
    root
}
fn origin() -> OwnerPart {
    OwnerPart {
        connector: jig_client_test_domain::FIRST,
        kind: OwnerPartKind::Resource,
        key: OwnerPartKey::Path(list([Box::<[u8]>::from(*b"origin/only"), Box::<[u8]>::from([255, 0])])),
    }
}
fn task(account: Option<OwnerPart>) -> Task {
    let mut task = super::task();
    task.context = None;
    task.chip.executor = Executor::Agent { charter: 1 };
    task.chip.phase = Phase::Held;
    task.chip.details = Box::new(ChipPhase::Owner {
        phase: OwnerChipPhase::Held { reason: HoldCategory::Drift, revision: 3 },
        details: Box::new(OwnerChipDetails {
            dependencies: List::with_capacity(0),
            project: task.project,
            executor: OwnerExecutor::Agent { charter: 1 },
            phase_since: Wall::from_nanos(10),
            run_since: None,
            attempt: None,
            retry: None,
            wake: task.wake,
            person_wait: None,
            hold: Some(OwnerChipHold {
                placed: None,
                circle: None,
                account,
                source: Some(origin()),
                reason: HoldCategory::Drift,
                revision: 3,
                before: OwnerPriorPhase::Waiting,
                escalation: None,
                tries: OwnerFailureTries { transient: 0, permanent: 0, run: 0, agent: 0, lost: 0, invalid: 0 },
                hold_wait_since: None,
            }),
            budget: task.budget.clone(),
            activity: 0,
            resources: List::with_capacity(0),
            resources_cut: false,
            resources_observed: false,
            procedure: None,
            live_delegates: 0,
            closing_connectors: None,
            ending: None,
            ended_at: None,
            observation: None,
        }),
    });
    task
}
fn load(root: &mut Domain, task: Task) -> Vec<Request> {
    let opened = drive(
        root,
        Event::Went {
            place: Place::Address(Address::Jig(jig_client_view::address::Address::Task {
                task: task.chip.task,
                section: jig_client_view::address::TaskSection::Overview,
            })),
        },
    );
    let watch = opened
        .iter()
        .find_map(|row| match row {
            Request::Client {
                request:
                    jig_client::Request::Down {
                        down:
                            jig_client::Down::Open { watch, subject: jig_client::Watchable::Jig(Subject::Task { .. }) },
                    },
            } => Some(*watch),
            _ => None,
        })
        .unwrap();
    drive(
        root,
        Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Watched { watch, event: WatchEvent::Delivered(Delivery::Task(Box::new(task))) },
            },
        },
    )
}
pub(super) fn view() -> View<'static> {
    let jig = words();
    let first = family_words();
    let limits = limits();
    let bounds = Limits::for_world(&limits, jig, first, 1024, 256).unwrap();
    View::new(&bounds, &limits, jig, first, first::Address { project: 3, item: 5 })
}

#[test]
fn a_supplied_native_hold_slot_reads_and_renders_its_actual_scoped_account() {
    let mut root = ready();
    let account =
        OwnerPart { connector: jig_client_test_domain::FIRST, kind: OwnerPartKind::Hold, key: OwnerPartKey::Number(9) };
    let original = task(Some(account.clone()));
    let rows = load(&mut root, original.clone());
    let subject = family::Subject { scope: family::Scope::Project(3), kind: family::Kind::Hold(9) };
    let reads: Vec<_> = rows
        .iter()
        .filter_map(|row| match row {
            Request::First(first::Out::Read { read, subject }) => Some((*read, *subject)),
            _ => None,
        })
        .collect();
    assert_eq!(reads.len(), 1);
    assert_eq!(reads[0].1, subject);
    let mut view = view();
    patches(&mut view, &root);
    assert!(
        view.tree().nodes().iter().any(
            |node| node.role == Role::Status && node.name.as_deref() == Some(b"Loading family document".as_slice())
        )
    );
    let path =
        owner::Path::new(list([Box::<[u8]>::from(*b"account/resource"), Box::<[u8]>::from([255, 0])]), 16).unwrap();
    drive(
        &mut root,
        Event::First(first::Event::Read {
            read: reads[0].0,
            result: first::ReadResult::Value(family::Document::Connector(owner::ConnectorDocument {
                subject,
                value: Box::new(owner::ClientDocument::Hold {
                    task: 9,
                    resources: list([owner::ClientResource {
                        path,
                        role: owner::ResourceRole::Owned,
                        hold: owner::Hold::None,
                        fact: Some(owner::Fact { state: Some(11), observed: Wall::from_nanos(12), pending: false }),
                    }]),
                    effects: List::with_capacity(0),
                }),
            })),
        }),
    );
    let watch = rows
        .iter()
        .find_map(|row| match row {
            Request::First(first::Out::Open { watch, subject: actual }) if *actual == subject => Some(*watch),
            _ => None,
        })
        .unwrap();
    patches(&mut view, &root);
    for text in [
        b"Hold account".as_slice(),
        b"Observed resource state: 11",
        b"Owned resource",
        b"account/resource",
        b"origin/only",
        b"\\xFF\\x00",
    ] {
        assert!(contains(&view, text), "missing {}", String::from_utf8_lossy(text));
    }
    assert!(view.tree().nodes().iter().any(|node| node.key == Some(crate::family_key(&subject))));
    assert!(patches(&mut view, &root).is_empty());
    let retained =
        root.jig().borrow(root.client()).objects().get(root.jig().objects().find(ObjectKey::Task(9)).unwrap()).unwrap();
    let jig_client_view::objects::Body::Task(retained) = retained.body.as_ref().unwrap() else { panic!("whole task") };
    assert_eq!(retained.as_ref(), &original);
    open_families(&mut root, &rows);
    let left = drive(
        &mut root,
        Event::Went { place: Place::Address(Address::Jig(jig_client_view::address::Address::Inbox { project: None })) },
    );
    assert!(left.iter().any(|row| match row {
        Request::First(first::Out::Close { watch: actual }) => *actual == watch,
        _ => false,
    }));
    patches(&mut view, &root);
    assert!(!contains(&view, b"Hold account"));
}

#[test]
fn an_absent_native_hold_account_keeps_its_origin_without_opening_a_family_read() {
    let mut root = ready();
    let rows = load(&mut root, task(None));
    assert!(
        rows.iter()
            .find_map(|row| match row {
                Request::First(first::Out::Read { read, .. }) => Some(*read),
                _ => None,
            })
            .is_none()
    );
    let mut view = view();
    patches(&mut view, &root);
    assert!(contains(&view, b"origin/only"));
    assert!(!contains(&view, b"Hold account"));
    assert!(view.tree().nodes().iter().any(|node| node.name.as_deref() == Some(b"Cannot show this part".as_slice())));
}
