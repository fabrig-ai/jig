#![expect(dead_code, reason = "each settings binary uses its own subset of the shared story helpers")]
//! Settings travel through the real restart, store and commit barrier (engine.md, 17.7).
use jig_core_world::{client_peer::ClientPeer, effects::World, fixtures, peers, world};
use jig_domain_core::{self as core, documents, reads, settings};
use jig_domain_people as people;
use jig_test_domain as root;
use skein_lib::{Duration, ReplyTo, Token};

pub(super) const SEED: u64 = 37_001;
pub(super) const OWNER: u64 = 1;
pub(super) fn configuration() -> (root::Config, root::Limits) {
    let document = jig_protocol_config::document(
        include_bytes!("engine.json"),
        &jig_protocol_config::DocumentLimits { bytes: 16384, tokens: 4096, depth: 16 },
    )
    .expect("settings story fixture");
    let mut engine = jig_protocol_config::engine(
        &document,
        0,
        &jig_protocol_config::Directories::working(),
        jig_protocol_config::Defaults::Document,
    )
    .expect("settings story fixture");
    engine.charters[0].policy.instructions = Box::from(b"Answer within the granted authority.".as_slice());
    let mut config = world::config(SEED);
    config.core = jig_protocol_config::core(&engine, [31; 16], SEED);
    let mut limits = world::limits();
    limits.core = engine.limits.core;
    peers::journal(&mut limits);
    (config, limits)
}
pub(super) fn start(history: u32) -> World {
    let (config, mut limits) = configuration();
    limits.core.settings.history = history;
    let mut world = World::first_start_with_accounts(config, &limits);
    world.client_peer = Some(ClientPeer::default());
    assert!(world.domain.ready());
    world
}
pub(super) fn sign_in(world: &mut World) -> u64 {
    world.send(root::Event::Core(core::Event::SignIn {
        held: None,
        reply_to: ReplyTo::new(Token::new(SEED)),
        identity: people::Identity {
            key: people::IdentityKey { provider: 1, subject: Box::from(b"owner-a".as_slice()) },
            login: Box::new([]),
            name: Box::new([]),
        },
    }));
    let (
        _,
        root::Delivery::Core(core::Held::PeopleReply {
            reply: people::Reply::SignedIn { person, sign_in, .. }, ..
        }),
    ) = world.client_peer.as_mut().expect("settings story fixture").delivered.pop().expect("settings story fixture")
    else {
        panic!("owner signs in")
    };
    assert_eq!(person, OWNER);
    sign_in
}
pub(super) fn ask(world: &mut World, signed: u64, key: u8, ask: people::Ask) -> people::Reply {
    world.send(root::Event::Core(core::Event::People(people::Event::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(Token::new(SEED)),
        sign_in: signed,
        key: [key; 16],
        ask,
    })));
    let (_, root::Delivery::Core(core::Held::PeopleReply { reply, .. })) =
        world.client_peer.as_mut().expect("settings story fixture").delivered.pop().expect("settings story fixture")
    else {
        panic!("request reply")
    };
    reply
}
pub(super) fn frame(world: &mut World, signed: u64) -> documents::Frame {
    world.send(root::Event::Core(core::Event::Read {
        reply: ReplyTo::new(Token::new(SEED)),
        sign_in: signed,
        read: reads::Read::Frame,
    }));
    let (_, root::Delivery::Core(core::Held::Read { answer: reads::Answer::Document(document), .. })) =
        world.client_peer.as_mut().expect("settings story fixture").delivered.pop().expect("settings story fixture")
    else {
        panic!("me reply")
    };
    let documents::Document::Frame(frame) = *document else { panic!("me document") };
    frame
}
pub(super) fn key(key: settings::Key) -> root::Key {
    root::Key::Core(core::Key::Core(core::CoreKey::Settings(key)))
}
pub(super) fn record(record: settings::Record) -> root::Record {
    root::Record::Core(core::Record::Core(core::CoreRecord::Settings(record)))
}
pub(super) fn revision(world: &World, number: u32, history: bool) -> &settings::Revision {
    let address = if history { settings::Key::History(number) } else { settings::Key::Held(number) };
    let root::Record::Core(core::Record::Core(core::CoreRecord::Settings(settings::Record::Revision {
        revision, ..
    }))) = &world.store.rows[&key(address)]
    else {
        panic!("settings revision")
    };
    revision
}
pub(super) fn restart(world: &mut World, config: root::Config) {
    world.restart_with(config);
    assert!(
        world.domain.ready(),
        "restart trace: {:?}; steps: {:?}; failure: {:?}",
        world.trace,
        world.restart_steps,
        world.domain.restore_failure()
    );
}

pub(super) fn change(
    world: &mut World,
    signed: u64,
    key_number: u8,
    against: u32,
    items: Box<[settings::Item]>,
) -> people::Reply {
    world.send(root::Event::Core(core::Event::ChangeSettings {
        reply_to: ReplyTo::new(Token::new(SEED)),
        sign_in: signed,
        key: [key_number; 16],
        digest: [key_number; 32],
        change: Box::new(settings::Change { against, items, running: settings::Running::Stays }),
    }));
    let (_, root::Delivery::Core(core::Held::PeopleReply { reply, .. })) =
        world.client_peer.as_mut().expect("client").delivered.pop().expect("settings reply")
    else {
        panic!("settings reply")
    };
    reply
}
pub(super) fn template_edit(world: &World, grace: u64) -> settings::Item {
    let mut value = world.domain.settings().template(1).expect("chat template").clone();
    value.grace = Some(Duration::from_secs(grace));
    settings::Item {
        setting: settings::Setting::Template(1),
        edit: settings::Edit::Set(settings::Value::Template(Box::new(value))),
    }
}
pub(super) fn changed(reply: &people::Reply, revision: u32) {
    assert_eq!(reply, &people::Reply::Outcome(people::Outcome::SettingsChanged { revision }));
}

pub(super) fn settings_rows(world: &World) -> Vec<(root::Key, root::Record)> {
    world
        .store
        .rows
        .iter()
        .filter(|(key, _)| matches!(key, root::Key::Core(core::Key::Core(core::CoreKey::Settings(_)))))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

pub(super) fn propose(
    world: &mut World,
    signed: u64,
    request_key: u8,
    against: u32,
    items: Box<[settings::Item]>,
) -> people::Reply {
    world.send(root::Event::Core(core::Event::ProposeSettings {
        reply_to: ReplyTo::new(Token::new(SEED)),
        sign_in: signed,
        key: [request_key; 16],
        digest: [request_key; 32],
        change: Box::new(settings::Proposed {
            against,
            items,
            reason: b"Please review these settings".as_slice().into(),
        }),
    }));
    let (_, root::Delivery::Core(core::Held::PeopleReply { reply, .. })) =
        world.client_peer.as_mut().expect("client").delivered.pop().expect("proposal reply")
    else {
        panic!("proposal reply")
    };
    reply
}
pub(super) fn proposed(reply: &people::Reply) -> u64 {
    let people::Reply::Outcome(people::Outcome::Proposed { proposal, to }) = reply else {
        panic!("proposed: {reply:?}")
    };
    assert_eq!(*to, people::Role::Owner.number());
    *proposal
}
pub(super) fn party(world: &mut World, subject: &[u8], role: Option<(u32, people::Role)>) -> (u64, u64) {
    world.send(root::Event::Core(core::Event::SignIn {
        held: None,
        reply_to: ReplyTo::new(Token::new(SEED)),
        identity: people::Identity {
            key: people::IdentityKey { provider: 1, subject: subject.into() },
            login: subject.into(),
            name: subject.into(),
        },
    }));
    let (
        _,
        root::Delivery::Core(core::Held::PeopleReply {
            reply: people::Reply::SignedIn { person, sign_in, .. }, ..
        }),
    ) = world.client_peer.as_mut().expect("client").delivered.pop().expect("signed in")
    else {
        panic!("signed in")
    };
    if let Some((project, role)) = role {
        let root::Record::Core(core::Record::People(people::Stored::Roles { holdings, .. })) =
            &world.store.rows[&root::Key::Core(core::Key::People(people::Key::Roles(project)))]
        else {
            panic!("seeded roster")
        };
        let mut holdings = holdings.to_vec();
        holdings.push(people::Holding { person, role });
        world.send(root::Event::Core(core::Event::People(people::Event::Roles {
            project,
            holdings: holdings.into_boxed_slice(),
        })));
    }
    (person, sign_in)
}
pub(super) fn decide(
    world: &mut World,
    signed: u64,
    request_key: u8,
    proposer: u64,
    proposal: u64,
    decision: people::ProposalDecision,
    running: Option<people::Running>,
) -> people::Reply {
    ask(
        world,
        signed,
        request_key,
        people::Ask::DecideProposal {
            project: people::DEPLOYMENT,
            proposer: people::Proposer::Party(proposer),
            proposal,
            decision,
            running,
        },
    )
}
pub(super) fn read(world: &mut World, signed: u64, value: reads::Read) -> reads::Answer {
    world.send(root::Event::Core(core::Event::Read {
        reply: ReplyTo::new(Token::new(SEED)),
        sign_in: signed,
        read: value,
    }));
    let (_, root::Delivery::Core(core::Held::Read { answer, .. })) =
        world.client_peer.as_mut().expect("client").delivered.pop().expect("read reply")
    else {
        panic!("read reply")
    };
    answer
}
pub(super) fn proposal_document(world: &mut World, signed: u64, proposer: u64, proposal: u64) -> documents::Proposal {
    let reads::Answer::Document(document) =
        read(world, signed, reads::Read::Proposal { proposer: people::Proposer::Party(proposer), proposal })
    else {
        panic!("pending proposal document")
    };
    let documents::Document::Proposal(value) = *document else { panic!("proposal document") };
    *value
}
pub(super) fn inventory(world: &mut World, signed: u64) -> documents::Settings {
    let reads::Answer::Document(document) = read(world, signed, reads::Read::Settings) else {
        panic!("settings document")
    };
    let documents::Document::Settings(value) = *document else { panic!("settings inventory") };
    *value
}
pub(super) fn proposal_set_key(proposer: u64, proposal: u64) -> root::Key {
    key(settings::Key::Proposal {
        proposer: people::Proposer::Party(proposer),
        proposal: u32::try_from(proposal).expect("proposal fits"),
    })
}
pub(super) fn charter_and_template(world: &World) -> Box<[settings::Item]> {
    let mut charter = world.domain.settings().charter(1).expect("configured charter").clone();
    charter.policy.instructions = b"Changed by review".as_slice().into();
    Box::new([
        settings::Item {
            setting: settings::Setting::Charter(1),
            edit: settings::Edit::Set(settings::Value::Charter(Box::new(charter))),
        },
        settings::Item { setting: settings::Setting::Template(2), edit: settings::Edit::Removed },
    ])
}
pub(super) fn removing_charter(world: &World) -> Box<[settings::Item]> {
    let mut rules = world.domain.settings().rules().clone();
    rules.ceiling.delegation.kinds = Box::new([]);
    Box::new([
        settings::Item { setting: settings::Setting::Charter(1), edit: settings::Edit::Removed },
        settings::Item { setting: settings::Setting::Template(1), edit: settings::Edit::Removed },
        settings::Item { setting: settings::Setting::Policy(1), edit: settings::Edit::Removed },
        settings::Item {
            setting: settings::Setting::Rules,
            edit: settings::Edit::Set(settings::Value::Rules(Box::new(rules))),
        },
    ])
}
pub(super) fn updates(world: &mut World, signed: u64) -> Vec<documents::InboxEntry> {
    let reads::Answer::Page(page) = read(
        world,
        signed,
        reads::Read::Inbox {
            group: reads::Group::Updates,
            filter: reads::Filter { project: None, goal: None },
            from: None,
        },
    ) else {
        panic!("updates page")
    };
    page.rows
        .into_boxed()
        .into_vec()
        .into_iter()
        .map(|row| {
            let reads::PageRow::Inbox(entry) = row else { panic!("inbox entry") };
            entry
        })
        .collect()
}

pub(super) fn run_configuration() -> (root::Config, root::Limits) {
    let (mut config, mut limits) = configuration();
    config.core.defaults.rules.ceiling.tools.0 |= 1024;
    config.core.defaults.rules.maximum_run_spend = 20;
    for seed in &mut config.core.seed_projects {
        seed.policy.policy.ceiling.tools.0 |= 1024;
        for role in &mut seed.policy.policy.roles {
            role.authority.tools.0 |= 1024;
        }
    }
    let policy = config.core.defaults.policies.get_mut(&1).expect("project policy");
    policy.policy.ceiling.tools.0 |= 1024;
    for role in &mut policy.policy.roles {
        role.authority.tools.0 |= 1024;
    }
    limits.core.settings.proposals = 2;
    peers::journal(&mut limits);
    (config, limits)
}
pub(super) fn proposing_run(settings_family: bool) -> (World, u64, u64) {
    settings_run(settings_family, None)
}
pub(super) fn settings_run(settings_family: bool, history: Option<u32>) -> (World, u64, u64) {
    use skein_lib::{Duration, Token};
    let (config, mut limits) = run_configuration();
    if let Some(history) = history {
        limits.core.settings.history = history;
    }
    let mut world = World::first_start_with_accounts(config, &limits);
    world.client_peer = Some(ClientPeer::default());
    let signed = sign_in(&mut world);
    world.send(root::Event::Core(core::Event::Period { project: 1, period: 1, budget: 100, next: None }));
    world.send(root::Event::Core(core::Event::Fleet(jig_domain_fleet::Event::Hello {
        worker: 0,
        channel: Token::new(7),
        hello: jig_domain_fleet::Hello {
            bounds: fixtures::bounds(),
            stop_bound: Duration::from_millis(1),
            slots: 1,
            workstreams: Box::new([]),
            hosting: Box::new([]),
        },
    })));
    let mut goal = fixtures::chat(&world, 1, b"Review the settings".as_slice().into());
    let people::Ask::Create { batch, .. } = &mut goal else { panic!("goal creation") };
    batch[0].flags.conversation = false;
    batch[0].flags.listed = true;
    batch[0].budget = people::Budget::Named;
    batch[0].authority.spend = 40;
    batch[0].authority.tools = if settings_family { 2047 } else { 1023 };
    let people::Reply::Outcome(people::Outcome::Created { first, .. }) = ask(&mut world, signed, 101, goal) else {
        panic!("goal admitted")
    };
    assert_eq!(world.assigned.as_ref().expect("assigned proposing goal").task, first);
    (world, signed, first)
}
pub(super) fn run_charter(world: &World, instructions: &[u8]) -> settings::Item {
    let mut charter = world.domain.settings().charter(1).expect("fixture charter").clone();
    charter.policy.instructions = instructions.into();
    settings::Item {
        setting: settings::Setting::Charter(1),
        edit: settings::Edit::Set(settings::Value::Charter(Box::new(charter))),
    }
}
