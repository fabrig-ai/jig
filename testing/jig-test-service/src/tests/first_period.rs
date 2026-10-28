use super::fixtures as f;
use crate::{PartyOutput, Service};
use jig_domain_core as core;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_protocol_store as store;
use skein_lib::{ReplyTo, Token};

const CONFIGURED_PROJECTS: u32 = 2; // The fixture seeds projects 1 and 2.
const DEPLOYMENT_ROSTER: u32 = 1;

fn started() -> Service {
    let document = f::engine_document();
    let mut parsed = crate::startup::parse(&document).expect("fixture configuration");
    let first = parsed.engine.projects.into_iter().next().expect("configured project");
    let mut policy = first.policy.clone();
    policy.period_spend = 73;
    policy.roles[0].period_spend = 73;
    let roles = first.roles.clone();
    parsed.engine.projects = Box::new([
        first,
        jig_protocol_config::Project {
            name: Box::from("Project 2"),
            number: 2,
            roles,
            policy,
            templates: Box::new([]),
        },
    ]);
    let owner = parsed.engine.owners[0].clone();
    parsed.engine.owners = Box::new([owner.clone(), people::InitialOwner { project: 2, ..owner }]);
    let core = &mut parsed.engine.limits.core;
    core.authority.projects = 2;
    core.people.projects = CONFIGURED_PROJECTS + DEPLOYMENT_ROSTER;
    core.people.holdings = 2;
    core.people.initial_owners = 2;
    core.people.requests = 4;
    core.tasks.tasks = 2;
    core.tasks.exemptions = 2;
    core.people.operations = 2;
    core.tasks.project_tasks = 2;
    core.tasks.tree_tasks = 2;
    core.tasks.batch = 1;
    core.tasks.holdings = 2;
    core.tasks.run_inbox = 2;
    core.people.people = 2;
    core.people.sign_ins = 2;
    core.people.pending = 2;
    core.people.waiters = 2;
    core.people.inbox_tasks = 4;
    core.fleet.workers = 2;
    core.fleet.attempts = 2;
    core.fleet.calls = 2;
    core.views.watchers = 4;
    core.views.subjects_per_change = 4;
    core.documents.rows = 2;
    parsed.limits = crate::limits::for_profile(jig_protocol_config::Profile::Small, &parsed.engine.limits);
    parsed.limits.root.journal = f::journal(&parsed.limits.root);
    let (mut engine, io, bytes) =
        crate::startup::engine_startup(parsed, &f::files(&document), None, &f::seeds(), None, false)
            .expect("fixture startup");
    f::memory_store(&mut engine);
    Service::new(Some(engine), skein_lib::List::with_capacity(0), io, bytes, 512)
}
fn settle(service: &mut Service) {
    for _ in 0_u32..64 {
        f::turn(service);
        if !service.work_pending(skein_lib::Time::from_nanos(1)) {
            break;
        }
    }
    assert!(f::engine(service).domain.ready());
    assert_eq!(service.end(), None);
}
fn reply(service: &mut Service) -> people::Reply {
    settle(service);
    let Some(PartyOutput::Held(jig_test_domain::Delivery::Core(core::Held::PeopleReply { reply, .. }))) =
        f::engine(service).parties_out().pop()
    else {
        panic!("durable party reply");
    };
    reply
}
fn get(service: &mut Service, key: core::Key) -> Option<core::Record> {
    let engine = f::engine(service);
    engine.store.ask(store::Request::Get { owner: Token::new(991), key: store::Get::Jig(key) });
    let Some(store::Event::Got { row, .. }) = engine.store.above.pop() else { panic!("store get") };
    match row {
        Some(store::Row::Jig(record)) => Some(record),
        Some(store::Row::Application { .. }) => panic!("jig row"),
        None => None,
    }
}
fn ledger(service: &mut Service, project: u32, period: u64) -> tasks::FundingRecord {
    let key = core::Key::Tasks(tasks::Key::Ledger(tasks::Funder::Period { project, period }));
    let Some(core::Record::Tasks(tasks::Stored::Ledger(row))) = get(service, key) else { panic!("period ledger") };
    row
}
fn commits(service: &mut Service) -> u64 {
    let Some(core::Record::Core(core::CoreRecord::Deployment(row))) =
        get(service, core::Key::Core(core::CoreKey::Deployment))
    else {
        panic!("deployment header")
    };
    row.commits
}

#[test]
fn a_first_start_opens_each_seeded_projects_period() {
    let mut service = started();
    settle(&mut service);
    f::engine(&mut service).parties_in().push(core::Event::SignIn {
        held: None,
        reply_to: ReplyTo::new(Token::new(10)),
        identity: people::Identity {
            key: people::IdentityKey { provider: 1, subject: b"owner-a".as_slice().into() },
            login: b"owner".as_slice().into(),
            name: b"Owner".as_slice().into(),
        },
    });
    let people::Reply::SignedIn { sign_in, .. } = reply(&mut service) else { panic!("signed in") };
    for (label, project, budget, task) in [("first", 1, 100, 1), ("second", 2, 73, 2)] {
        let row = ledger(&mut service, project, 1);
        assert_eq!(row.numbers.budget, budget, "{label}");
        assert_eq!(
            row.period,
            Some(tasks::PeriodTimes { began: skein_lib::Wall::from_nanos(1), next: None }),
            "{label}"
        );
        f::engine(&mut service).parties_in().push(core::Event::People(people::Event::Ask {
            models: Box::new([]),
            reply_to: ReplyTo::new(Token::new(u64::from(project) + 20)),
            sign_in,
            key: u128::from(project).to_be_bytes(),
            ask: f::chat(project, b"first chat"),
        }));
        assert_eq!(
            reply(&mut service),
            people::Reply::Outcome(people::Outcome::Created {
                first: task,
                last: task,
                given: Some(people::Given { task, amount: budget })
            }),
            "{label}"
        );
    }
}

fn stored_period() -> tasks::FundingRecord {
    tasks::FundingRecord {
        period: Some(tasks::PeriodTimes { began: skein_lib::Wall::EPOCH, next: None }),
        funder: tasks::Funder::Period { project: 1, period: 9 },
        parent: None,
        numbers: tasks::Numbers { budget: 41, spent: 0, spent_below: 0, reserved: 0 },
        made: 0,
        closed: false,
    }
}
fn later_start() -> Service {
    let mut service = started();
    let engine = f::engine(&mut service);
    let opened = engine.store.above.pop().expect("store opened before its fixture commit");
    let header = core::Deployment {
        id: [7; 16],
        tasks: 0,
        people: 0,
        sign_ins: 0,
        messages: 0,
        runs: 0,
        calls: 0,
        connector_rows: 0,
        projects: 0,
        applications: 0,
        commits: 1,
    };
    let settings = jig_test_domain::initial_settings_current(engine.domain.settings());
    let count = u32::try_from(settings.len())
        .expect("fixture settings count")
        .checked_add(3)
        .expect("fixture first commit count");
    let mut writes = skein_lib::List::with_capacity(count);
    for row in settings {
        writes.push(store::Write::Save(row)).expect("fixture first commit");
    }
    for write in [
        store::Write::Save(core::Record::Core(core::CoreRecord::Deployment(header))),
        store::Write::Save(core::Record::Tasks(tasks::Stored::Ledger(stored_period()))),
        // The accounts' counter a first start keeps after making any none sources.
        store::Write::Save(core::Record::Accounts(jig_domain_accounts::Stored::Next { number: 2 })),
    ] {
        writes.push(write).expect("fixture first commit");
    }
    engine.store.ask(store::Request::Commit { number: 1, writes: writes.into_boxed() });
    engine.store.iterate();
    let Some(store::Event::Committed { number, .. }) = engine.store.above.pop() else { panic!("fixture commit") };
    assert_eq!(number, 1);
    engine.store.above.push(opened);
    service
}

#[test]
fn a_later_start_says_no_period() {
    let mut service = later_start();
    settle(&mut service);
    assert!(f::engine(&mut service).first_periods.is_empty(), "no Period remains to be said on a later start");
    assert_eq!(ledger(&mut service, 1, 9), stored_period());
    assert_eq!(
        get(&mut service, core::Key::Tasks(tasks::Key::Ledger(tasks::Funder::Period { project: 2, period: 1 }))),
        None
    );
    assert_eq!(commits(&mut service), 2, "restart said no Period and committed only its start mark");
}
