//! The real deployment's project names reach the public application-held client.
use super::*;
use jig_client::{Asker, Client, Handle, Readable, Readout, Told};
use skein_lib::{List, Queue};

const PROJECT: u32 = 2;
const ME: Handle = Handle { asker: Asker::JIG, number: 1 };
const PAGE: Handle = Handle { asker: Asker::JIG, number: 2 };
// Each public request and its reply settles in fewer than sixteen synchronous passes.
const PASSES: u32 = 16;

fn exchange(session: &mut Session, client: &mut Client, now: Time, wall: Wall, event: jig_client::Event) -> Vec<Told> {
    let mut out = Queue::uncapped();
    client.step(now, wall, event, &mut out);
    let mut told = Vec::new();
    let mut kept = std::collections::BTreeMap::new();
    for _ in 0..PASSES {
        while let Some(request) = out.pop() {
            if let jig_client::Request::Told { told: value } = request {
                told.push(value);
            } else if let jig_client::Request::Down { down: jig_client::Down::Save { record } } = request {
                let _previous = kept.insert(record.key, record);
            } else if let jig_client::Request::Down { down: jig_client::Down::Erase { key } } = request {
                let _previous = kept.remove(&key);
            } else {
                session.send(ClientId(0), now, wall, Request::Client { request }).expect("public request");
            }
        }
        let callbacks = session.poll(now, wall).expect("real engine replies");
        if callbacks.is_empty() {
            return told;
        }
        for (owner, callback) in callbacks {
            session.received(owner, now, wall, &callback);
            let Event::Client { event } = callback else { panic!("client callback") };
            client.step(now, wall, event, &mut out);
        }
    }
    panic!("public exchange did not settle");
}

#[test]
fn the_frame_names_each_project() {
    let (mut configuration, limits) = jig_core_world::fixtures::two_projects(8455);
    configuration.core.seed_projects[0].name = Box::from("Tally");
    // The shared deployment fixture leaves project 2 unnamed: its seed is Project 2.
    let (mut session, mut client, now, wall) = public(configuration, &limits);
    let client::ReadValue::Me(me) = read(&mut session, &mut client, now, wall, ME, client::Read::Me) else {
        panic!("public me document")
    };
    let names: Vec<_> = me.projects.iter().map(|row| (row.project, row.name.bytes())).collect();
    assert_eq!(names, [(1, b"Tally".as_slice()), (PROJECT, b"Project 2".as_slice())]);
    let client::ReadValue::OwnerProject(project) =
        read(&mut session, &mut client, now, wall, PAGE, client::Read::Project { project: PROJECT })
    else {
        panic!("public project document")
    };
    assert_eq!((project.number, project.name.bytes()), (PROJECT, b"Project 2".as_slice()));
}

fn read(
    session: &mut Session,
    client: &mut Client,
    now: Time,
    wall: Wall,
    handle: Handle,
    what: client::Read,
) -> client::ReadValue {
    let output = exchange(session, client, now, wall, jig_client::Event::Read { handle, read: Readable::Jig(what) });
    output
        .into_iter()
        .find_map(|value| {
            let Told::Page { handle: delivered, page: Readout::Jig(client::ReadResult::Value(value)) } = value else {
                return None;
            };
            (delivered == handle).then_some(value)
        })
        .expect("one public document")
}

fn public(configuration: root::Config, limits: &root::Limits) -> (Session, Client, Time, Wall) {
    let (mut session, now, wall) = session_with_limits(configuration, *limits, Peers::new(Vec::new(), Vec::new()));
    session.bind(ClientId(0), 1, identity(), now, wall);
    session.poll(now, wall).expect("signed-in owner");
    let mut client = Client::unlinked(&session.limits.client, 8455).expect("public client");
    exchange(&mut session, &mut client, now, wall, jig_client::Event::StartUnlinked { kept: List::with_capacity(0) });
    exchange(&mut session, &mut client, now, wall, jig_client::Event::Up { up: jig_client::Up::SignedIn });
    (session, client, now, wall)
}

fn projects() -> (Session, Client, Time, Wall) {
    let (mut config, mut limits) = jig_core_world::fixtures::two_projects(8456);
    // Two seed projects and one made project, plus deployment roles.
    limits.core.authority.projects = 3;
    limits.core.people.projects = 4;
    config.core.defaults.rules =
        (jig_domain_authority::Domain::new(config.core.defaults.rules.clone(), limits.core.authority)
            .expect("project room"))
        .rules()
        .clone();
    let seed = &config.core.seed_projects[0].policy;
    config.core.defaults.policies = skein_lib::Map::with_capacity(1);
    config
        .core
        .defaults
        .policies
        .insert(
            1,
            core::ConfiguredPolicy {
                policy: seed.policy.clone(),
                permissions: Box::new([]),
                templates: seed.templates.clone(),
            },
        )
        .expect("one configured policy");
    jig_core_world::peers::journal(&mut limits);
    public(config, &limits)
}
fn ask(
    session: &mut Session,
    client: &mut Client,
    now: Time,
    wall: Wall,
    number: u64,
    ask: client::Ask,
) -> client::Answer {
    let key = jig_client::Key { asker: Asker::JIG, number };
    let output = exchange(
        session,
        client,
        now,
        wall,
        jig_client::Event::Send { key, name: None, request: jig_client::Ask::Jig(Box::new(ask)) },
    );
    output
        .into_iter()
        .find_map(|told| {
            let Told::Answered {
                key: answered,
                outcome: jig_client::Outcome::Answered(jig_client::Answered::Jig(answer)),
            } = told
            else {
                return None;
            };
            (answered == key).then_some(answer)
        })
        .expect("public keyed answer")
}
fn name(value: &[u8]) -> client::Text {
    client::Text::new(value.into(), 64).expect("project name")
}
#[test]
fn a_make_through_the_public_api_is_answered_and_in_me() {
    const MADE: u32 = 3; // Past both seed projects.
    let (mut session, mut client, now, wall) = projects();
    assert_eq!(
        ask(&mut session, &mut client, now, wall, 1, client::Ask::MakeProject { name: name(b"Tally"), policy: 1 }),
        client::Answer::Done(client::Outcome::Owner(client::OwnerOutcome::ProjectMade { project: MADE }))
    );
    let client::ReadValue::Me(me) = read(&mut session, &mut client, now, wall, ME, client::Read::Me) else {
        panic!("me")
    };
    assert!(me.projects.iter().any(|row| row.project == MADE && row.name.bytes() == b"Tally"));
}
#[test]
fn a_rename_through_the_public_api_reaches_me() {
    let (mut session, mut client, now, wall) = projects();
    assert_eq!(
        ask(&mut session, &mut client, now, wall, 1, client::Ask::Rename { project: PROJECT, name: name(b"Tally") }),
        client::Answer::Done(client::Outcome::Owner(client::OwnerOutcome::Renamed { project: PROJECT }))
    );
    let client::ReadValue::Me(me) = read(&mut session, &mut client, now, wall, ME, client::Read::Me) else {
        panic!("me")
    };
    assert_eq!(me.projects.iter().find(|row| row.project == PROJECT).expect("renamed project").name.bytes(), b"Tally");
}
#[test]
fn name_taken_reaches_the_client_with_its_own_refusal() {
    let (mut session, mut client, now, wall) = projects();
    let _answer =
        ask(&mut session, &mut client, now, wall, 1, client::Ask::Rename { project: PROJECT, name: name(b"Tally") });
    assert_eq!(
        ask(&mut session, &mut client, now, wall, 2, client::Ask::MakeProject { name: name(b"tally"), policy: 1 }),
        client::Answer::Refused { refusal: client::Refusal::NameTaken, at: None }
    );
}
