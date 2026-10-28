//! Project decisions at the engine's lowest complete boundary (people.md, 5.1–5.3).
use jig_core_world::{
    client_peer::ClientPeer,
    effects::{Cut, World},
    world,
};
use jig_domain_authority as authority;
use jig_domain_core::{self as core, documents, reads};
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_test_domain as root;
use skein_lib::{Map, ReplyTo, Token};

const SEED: u32 = 1;
const MADE: u32 = 2;
const NEXT: u32 = 3;
const POLICY: u32 = 1;
const OWNER: u64 = 1;
const OWNER_SUBJECT: u64 = 7; // world::config's first owner.
const OTHER_SUBJECT: u64 = 8;
const PROJECT_ROOM: u32 = 3; // The seed and two made projects.
const ROSTER_ROOM: u32 = PROJECT_ROOM + 1; // Includes deployment roles.
const KEY: u8 = 91;
const REPLY: Token = Token::new(12_001);

fn configuration() -> (root::Config, root::Limits) {
    let mut config = world::config(12_001);
    let mut limits = world::limits();
    limits.core.authority.projects = PROJECT_ROOM;
    limits.core.people.projects = ROSTER_ROOM;
    config.core.defaults.rules = (authority::Domain::new(config.core.defaults.rules.clone(), limits.core.authority)
        .expect("project room"))
    .rules()
    .clone();
    let seed = &config.core.seed_projects[0].policy;
    let mut policy = seed.policy.clone();
    policy.period_spend = 700;
    config.core.defaults.policies = Map::with_capacity(1);
    config
        .core
        .defaults
        .policies
        .insert(POLICY, core::ConfiguredPolicy { policy, permissions: Box::new([]), templates: seed.templates.clone() })
        .expect("one configured policy");
    jig_core_world::peers::journal(&mut limits);
    (config, limits)
}
fn start(mut config: root::Config, mut limits: root::Limits) -> (World, u64) {
    config.core.defaults.rules = (authority::Domain::new(config.core.defaults.rules.clone(), limits.core.authority)
        .expect("configured authority room"))
    .rules()
    .clone();
    jig_core_world::peers::journal(&mut limits);
    let mut world = World::first_start(config, limits);
    world.client_peer = Some(ClientPeer::default());
    let (_, signed) = sign_in(&mut world, OWNER_SUBJECT);
    (world, signed)
}
fn setup() -> (World, u64) {
    let (config, limits) = configuration();
    start(config, limits)
}
fn sign_in(world: &mut World, subject: u64) -> (u64, u64) {
    world.send(root::Event::Core(core::Event::SignIn {
        held: None,
        reply_to: ReplyTo::new(REPLY),
        identity: people::Identity {
            key: people::IdentityKey { provider: 0, subject: subject.to_be_bytes().into() },
            login: b"owner".as_slice().into(),
            name: b"Owner".as_slice().into(),
        },
    }));
    let people::Reply::SignedIn { person, sign_in, .. } = take_reply(world) else { panic!("signed in") };
    (person, sign_in)
}
fn take_reply(world: &mut World) -> people::Reply {
    let (_, root::Delivery::Core(core::Held::PeopleReply { reply, .. })) =
        world.client_peer.as_mut().expect("client peer").delivered.pop().expect("party reply")
    else {
        panic!("party reply")
    };
    reply
}
fn event(signed: u64, key: u8, ask: people::Ask) -> root::Event {
    root::Event::Core(core::Event::People(people::Event::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(REPLY),
        sign_in: signed,
        key: [key; 16],
        ask,
    }))
}
fn request(world: &mut World, signed: u64, key: u8, ask: people::Ask) -> people::Reply {
    world.send(event(signed, key, ask));
    take_reply(world)
}
fn make(name: &str, policy: u32) -> people::Ask {
    people::Ask::MakeProject { name: name.into(), policy }
}
fn rename(project: u32, name: &str) -> people::Ask {
    people::Ask::Rename { project, name: name.into() }
}
fn made(world: &mut World, signed: u64) {
    let _reply = request(world, signed, KEY, make("Tally", POLICY));
}
fn refused(why: people::Refusal) -> people::Reply {
    people::Reply::Outcome(people::Outcome::Refused(why))
}
fn read(world: &mut World, signed: u64, what: reads::Read) -> documents::Document {
    world.send(root::Event::Core(core::Event::Read { reply: ReplyTo::new(REPLY), sign_in: signed, read: what }));
    let (_, root::Delivery::Core(core::Held::Read { answer: reads::Answer::Document(document), .. })) =
        world.client_peer.as_mut().expect("client peer").delivered.pop().expect("read answer")
    else {
        panic!("read document")
    };
    *document
}
fn frame(world: &mut World, signed: u64) -> documents::Frame {
    let documents::Document::Frame(frame) = read(world, signed, reads::Read::Frame) else { panic!("frame") };
    frame
}
fn project(world: &mut World, signed: u64) -> documents::Project {
    let documents::Document::Project(project) = read(world, signed, reads::Read::Project { project: MADE }) else {
        panic!("project")
    };
    *project
}

#[test]
fn a_deployment_owner_makes_a_project_and_finds_it_in_me() {
    let (config, mut limits) = configuration();
    limits.core.tasks.funders = 1; // Exactly the made project's first period.
    let (mut world, signed) = start(config, limits);
    assert_eq!(
        request(&mut world, signed, KEY, make("Tally", POLICY)),
        people::Reply::Outcome(people::Outcome::ProjectMade { project: MADE })
    );
    let me = frame(&mut world, signed);
    let names: Vec<_> = me.projects.iter().map(|row| (row.project, row.role, row.name.bytes.as_ref())).collect();
    assert_eq!(
        names,
        [(SEED, people::Role::Owner, b"Project 1".as_slice()), (MADE, people::Role::Owner, b"Tally".as_slice())]
    );
    let project = project(&mut world, signed);
    assert_eq!(project.policy.period_spend, 700);
    assert_eq!(project.members.len(), 1);
    assert_eq!((project.members.as_slice()[0].party, project.members.as_slice()[0].role), (OWNER, people::Role::Owner));
}

#[test]
fn a_project_owner_without_deployment_ownership_is_refused_first() {
    let (mut world, _) = setup();
    let (person, signed) = sign_in(&mut world, OTHER_SUBJECT);
    world.send(root::Event::Core(core::Event::People(people::Event::Roles {
        project: SEED,
        holdings: Box::new([people::Holding { person, role: people::Role::Owner }]),
    })));
    assert_eq!(request(&mut world, signed, KEY, make("12", 9)), refused(people::Refusal::Role { held: None }));
}
#[test]
fn a_make_checks_the_name_before_the_configured_policy() {
    let (mut world, signed) = setup();
    assert_eq!(
        request(&mut world, signed, KEY, make("12", 9)),
        refused(people::Refusal::NotWellFormed { at: 0, why: people::Malformed::Name(people::NameRule::Digits) })
    );
}
#[test]
fn a_make_checks_the_configured_policy_before_room() {
    let (config, mut limits) = configuration();
    limits.core.authority.projects = 1; // Exactly the seed.
    let (mut world, signed) = start(config, limits);
    assert_eq!(request(&mut world, signed, KEY, make("Project 1", 9)), refused(people::Refusal::Unknown));
}
#[test]
fn a_full_deployment_checks_room_before_name_taken() {
    let (config, mut limits) = configuration();
    limits.core.authority.projects = 1; // Exactly the seed.
    let (mut world, signed) = start(config, limits);
    assert_eq!(
        request(&mut world, signed, KEY, make("Project 1", POLICY)),
        refused(people::Refusal::configured(
            people::LimitKind::Projects,
            1,
            people::LimitKey::AuthorityProjects,
            people::LimitUnit::Items
        ))
    );
}
#[test]
fn a_full_roster_refuses_a_make_even_with_authority_room() {
    let (config, mut limits) = configuration();
    limits.core.people.projects = 2; // Seed and deployment roles.
    let (mut world, signed) = start(config, limits);
    assert_eq!(
        request(&mut world, signed, KEY, make("Tally", POLICY)),
        refused(people::Refusal::configured(
            people::LimitKind::Projects,
            2,
            people::LimitKey::PeopleProjects,
            people::LimitUnit::Items
        ))
    );
}
#[test]
fn a_make_waits_for_first_period_room_without_spending_a_number() {
    let (config, mut limits) = configuration();
    limits.core.tasks.funders = 0;
    let (mut world, signed) = start(config, limits);
    assert_eq!(request(&mut world, signed, KEY, make("Tally", POLICY)), people::Reply::Again(people::Again::Busy));
    let key = root::Key::Core(core::Key::Core(core::CoreKey::Deployment));
    let root::Record::Core(core::Record::Core(core::CoreRecord::Deployment(header))) = &world.store.rows[&key] else {
        panic!("durable deployment header")
    };
    assert_eq!(header.projects, u64::from(SEED));
    assert_eq!(frame(&mut world, signed).projects.len(), 1);
    assert!(world.project_made.is_empty());
}
#[test]
fn a_refused_make_spends_no_project_number() {
    let (mut world, signed) = setup();
    made(&mut world, signed);
    assert_eq!(request(&mut world, signed, KEY + 1, make("tally", POLICY)), refused(people::Refusal::NameTaken));
    assert_eq!(
        request(&mut world, signed, KEY + 2, make("Next", POLICY)),
        people::Reply::Outcome(people::Outcome::ProjectMade { project: NEXT })
    );
}
#[test]
fn a_made_projects_first_period_funds_the_makers_chat() {
    let (mut world, signed) = setup();
    made(&mut world, signed);
    let people::Ask::Create { batch, .. } = jig_core_world::fixtures::chat(&world, SEED, b"funded".as_slice().into())
    else {
        panic!("chat fixture")
    };
    let ask = people::Ask::Create { project: MADE, batch };
    let _reply = request(&mut world, signed, KEY + 1, ask);
    let row = jig_core_world::fixtures::live_row(&world, 1);
    assert_eq!(row.project, MADE);
    assert_eq!(row.numbers.budget, 100);
    assert_eq!(row.funder, tasks::Funder::Pool { project: MADE, person: OWNER, period: 1 });
}
#[test]
fn a_restart_restores_a_made_project_whose_policy_configuration_no_longer_names() {
    let (mut world, signed) = setup();
    made(&mut world, signed);
    let (mut config, _) = configuration();
    config.core.defaults.policies = Map::with_capacity(0);
    world.restart_with(config);
    let project = project(&mut world, signed);
    assert_eq!(project.policy.ceiling.ceiling.spend, 1000);
    assert_eq!(project.policy.escalation_role, Some(0));
    assert_eq!(project.policy.period_spend, 700);
    let (config, _) = configuration();
    world.restart_with(config);
    assert_eq!(
        request(&mut world, signed, KEY + 1, make("Next", POLICY)),
        people::Reply::Outcome(people::Outcome::ProjectMade { project: NEXT })
    );
}
#[test]
fn a_make_retried_under_its_key_is_made_once() {
    let (mut world, signed) = setup();
    made(&mut world, signed);
    let commits = world.commits.len();
    assert_eq!(
        request(&mut world, signed, KEY, make("Tally", POLICY)),
        people::Reply::Outcome(people::Outcome::ProjectMade { project: MADE })
    );
    assert_eq!(world.commits.len(), commits);
    let (config, _) = configuration();
    world.restart_with(config);
    assert_eq!(
        request(&mut world, signed, KEY, make("Tally", POLICY)),
        people::Reply::Outcome(people::Outcome::ProjectMade { project: MADE })
    );
}
#[test]
fn the_root_hears_a_project_made_after_its_commit() {
    let (mut world, signed) = setup();
    let commit = world.store.applied + 1;
    world.cut = Some(Cut::Durable(commit));
    world.send(event(signed, KEY, make("Tally", POLICY)));
    assert_eq!(world.reached_cut, Some(Cut::Durable(commit)));
    assert!(world.project_made.is_empty());
    world.cut = None;
    world.drain();
    assert_eq!(world.project_made, [(commit, MADE)]);
}
#[test]
fn an_owner_renames_a_project_and_every_partys_me_says_it() {
    let (mut world, signed) = setup();
    let (other, other_signed) = sign_in(&mut world, OTHER_SUBJECT);
    world.send(root::Event::Core(core::Event::People(people::Event::Roles {
        project: SEED,
        holdings: Box::new([
            people::Holding { person: OWNER, role: people::Role::Owner },
            people::Holding { person: other, role: people::Role::Observer },
        ]),
    })));
    assert_eq!(
        request(&mut world, signed, KEY, rename(SEED, "Tally")),
        people::Reply::Outcome(people::Outcome::Renamed { project: SEED })
    );
    for signed in [signed, other_signed] {
        assert_eq!(frame(&mut world, signed).projects.as_slice()[0].name.bytes.as_ref(), b"Tally");
    }
}
#[test]
fn a_maintainer_cannot_rename_even_with_a_malformed_name() {
    let (mut world, signed) = setup();
    world.send(root::Event::Core(core::Event::People(people::Event::Roles {
        project: SEED,
        holdings: Box::new([people::Holding { person: OWNER, role: people::Role::Maintainer }]),
    })));
    assert_eq!(
        request(&mut world, signed, KEY, rename(SEED, "12")),
        refused(people::Refusal::Role { held: Some(people::Role::Maintainer) })
    );
}
#[test]
fn a_rename_refuses_an_out_of_shape_name() {
    let (mut world, signed) = setup();
    assert_eq!(
        request(&mut world, signed, KEY, rename(SEED, "12")),
        refused(people::Refusal::NotWellFormed { at: 0, why: people::Malformed::Name(people::NameRule::Digits) })
    );
}
#[test]
fn a_rename_refuses_another_projects_name_in_another_case() {
    let (mut world, signed) = setup();
    made(&mut world, signed);
    assert_eq!(request(&mut world, signed, KEY + 1, rename(SEED, "tally")), refused(people::Refusal::NameTaken));
}
#[test]
fn a_project_may_change_its_own_names_case() {
    let (mut world, signed) = setup();
    assert_eq!(
        request(&mut world, signed, KEY, rename(SEED, "PROJECT 1")),
        people::Reply::Outcome(people::Outcome::Renamed { project: SEED })
    );
}
#[test]
fn a_restart_keeps_a_rename() {
    let (mut world, signed) = setup();
    let _reply = request(&mut world, signed, KEY, rename(SEED, "Tally"));
    let (config, _) = configuration();
    world.restart_with(config);
    assert_eq!(frame(&mut world, signed).projects.as_slice()[0].name.bytes.as_ref(), b"Tally");
}
