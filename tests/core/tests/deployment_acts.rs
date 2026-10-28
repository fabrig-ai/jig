use jig_core_world::{
    effects::{Cut, World},
    fixtures::live_row,
    stories::delegates::delegate_pair,
};
use jig_domain_accounts as accounts;
use jig_domain_core as core;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_test_domain as root;
use skein_lib::{Duration, Env, ReplyTo, Time, Token, Wall};

const PROJECT: u32 = 1;
const CONNECTOR: u16 = 1;
const KEY: u8 = 91;
const PROVIDER: u32 = 1;
const GRACE: Duration = Duration::from_secs(5);
const REASON: &[u8] = b"finished";
fn configuration(seed: u64) -> (root::Config, root::Limits) {
    let (mut config, mut limits) = jig_core_world::faults::plan_fixture(seed);
    limits.core.people.operations = 4;
    limits.core.accounts.accounts = 2;
    config.core.grants = Box::new([people::ActGrant {
        connector: CONNECTOR,
        may: Box::new([
            people::ActKind::Close,
            people::ActKind::AddKey,
            people::ActKind::RemoveAccount,
            people::ActKind::Adopt,
        ]),
    }]);
    for provider in &mut config.core.settings.llm_providers.providers {
        provider.offers.key = true;
    }
    jig_core_world::peers::journal(&mut limits);
    (config, limits)
}
fn world(seed: u64) -> World {
    let (config, limits) = configuration(seed);
    World::configured(seed, false, config, limits)
}
fn close(task: u64, project: u32, grace: Option<Duration>) -> people::Act {
    people::Act::Close { project, task, grace, reason: REASON.into() }
}
fn event(key: u8, act: people::Act, carried: Option<core::Carried>) -> root::Event {
    root::Event::Core(core::Event::Act {
        reply_to: ReplyTo::new(Token::new(u64::from(key))),
        connector: CONNECTOR,
        key: [key; 16],
        act: Box::new(act),
        carried,
    })
}
fn act(world: &mut World, key: u8, act: people::Act) {
    world.send(event(key, act, None));
}
fn answered(world: &World) -> &people::Outcome {
    &world.act_answers.last().expect("one act answer").1
}
fn cancelled(world: &World, task: u64) {
    let key = root::Key::Core(core::Key::Tasks(tasks::Key::Ended(task)));
    let row =
        if let Some(root::Record::Core(core::Record::Tasks(tasks::Stored::Ended(row)))) = world.store.rows.get(&key) {
            row
        } else {
            live_row(world, task)
        };
    let ending = match &row.phase {
        tasks::Phase::Closing(closing) => &closing.ending,
        tasks::Phase::Ended(ending) => ending,
        phase @ (tasks::Phase::Waiting | tasks::Phase::Active(_) | tasks::Phase::Held { .. }) => {
            panic!("cancelled: {phase:?}")
        }
    };
    assert!(
        matches!(ending, tasks::Ending::Cancelled { by: tasks::Party::Deployment { project: PROJECT }, reason, .. } if reason.as_ref() == REASON)
    );
}
#[test]
fn a_connector_closes_a_persons_chat_cancelling_every_delegate_then_the_chat_with_its_grace() {
    let mut world = world(1800);
    let chat = world.assignments[0].0;
    let delegates = delegate_pair(&mut world);
    act(&mut world, KEY, close(chat, PROJECT, Some(GRACE)));
    assert!(matches!(answered(&world), people::Outcome::Compound { project: PROJECT, operations: 3, .. }));
    for task in [delegates.0, delegates.1] {
        cancelled(&world, task);
    }
    let finish = live_row(&world, chat).grace.as_ref().expect("conversation finish asked");
    assert_eq!(finish.by, tasks::Party::Deployment { project: PROJECT });
    assert_eq!(finish.reason.as_ref(), REASON);
    assert_eq!(world.inbound.last().expect("finish message").from, tasks::Party::Deployment { project: PROJECT });
    world.advance(GRACE);
    cancelled(&world, chat);
}
#[test]
fn a_close_refuses_what_is_no_persons_live_conversation() {
    let mut world = world(1801);
    let chat = world.assignments[0].0;
    let (delegate, _) = delegate_pair(&mut world);
    let before = live_row(&world, chat).clone();
    for (key, act_value, why) in [
        (KEY, close(delegate, PROJECT, None), people::Refusal::NotOffered),
        (KEY + 1, close(chat, PROJECT + 1, None), people::Refusal::Unknown),
        (KEY + 2, close(u64::MAX, PROJECT, None), people::Refusal::Unknown),
    ] {
        act(&mut world, key, act_value);
        assert_eq!(answered(&world), &people::Outcome::Refused(why));
    }
    let over = close(chat, PROJECT, Some(Duration::from_secs(601)));
    act(&mut world, KEY + 3, over.clone());
    let refused = people::Outcome::CompoundRefused {
        at: 2,
        why: people::Refusal::configured(
            people::LimitKind::Grace,
            600_000,
            people::LimitKey::TasksGrace,
            people::LimitUnit::Milliseconds,
        ),
    };
    assert_eq!(answered(&world), &refused);
    assert_eq!(live_row(&world, chat), &before);
    let commits = world.commits.len();
    act(&mut world, KEY + 3, over);
    assert_eq!(answered(&world), &refused);
    assert_eq!(world.commits.len(), commits);
    act(&mut world, KEY + 4, close(chat, PROJECT, None));
    act(&mut world, KEY + 5, close(chat, PROJECT, None));
    assert!(matches!(answered(&world), people::Outcome::Refused(people::Refusal::Ended { .. })));
}
fn add_key(world: &mut World, key: u8, number: u64) {
    world.send(event(
        key,
        people::Act::AddKey { provider: PROVIDER, join: Some(0), digest: [7; 32] },
        Some(core::Carried { number, digest: [7; 32] }),
    ));
}
#[test]
fn a_connector_adds_the_deployments_key_and_a_replay_leaves_the_value_unused() {
    let mut world = world(1802);
    add_key(&mut world, KEY, 1_u64 << 63);
    let people::Outcome::AccountKeyKept { source } = *answered(&world) else {
        panic!("key kept: {:?}", answered(&world))
    };
    let account = world.domain.accounts().source(source).expect("source").account;
    assert_eq!(
        world.domain.accounts().account(account).expect("deployment account").holder(),
        accounts::Holder::Deployment
    );
    let before = world.people_answers.len() + world.application_answers.len() + world.act_answers.len();
    add_key(&mut world, KEY, (1_u64 << 63) + 1);
    assert_eq!(answered(&world), &people::Outcome::AccountKeyKept { source });
    assert_eq!(world.carried_unused.last(), Some(&(before, (1_u64 << 63) + 1)));
}
#[test]
fn a_connector_removes_a_deployment_account_and_not_a_persons() {
    let mut world = world(1803);
    add_key(&mut world, KEY, 1_u64 << 63);
    let people::Outcome::AccountKeyKept { source } = *answered(&world) else { panic!("key kept") };
    let account = world.domain.accounts().source(source).expect("bounded deployment act fixture").account;
    // A party's key travels through the same carried-value entrance.
    world.send(root::Event::Core(core::Event::Key {
        reply_to: ReplyTo::new(Token::new(1000)),
        sign_in: world.sign_in.expect("bounded deployment act fixture"),
        key: [KEY + 2; 16],
        ask: Box::new(people::Ask::AccountChange {
            holder: people::AccountHolder::Own,
            change: people::AccountChange::AddKey { provider: PROVIDER, join: None, digest: [8; 32] },
        }),
        carried: core::Carried { number: (1_u64 << 63) + 1, digest: [8; 32] },
    }));
    let people::Reply::Outcome(people::Outcome::AccountKeyKept { source: own }) =
        *world.people_answers.last().expect("bounded deployment act fixture")
    else {
        panic!("person key kept")
    };
    let own = world.domain.accounts().source(own).expect("bounded deployment act fixture").account;
    act(&mut world, KEY + 3, people::Act::RemoveAccount { account: own });
    assert!(matches!(answered(&world), people::Outcome::Refused(_)));
    assert!(world.domain.accounts().account(own).is_some());
    act(&mut world, KEY + 4, people::Act::RemoveAccount { account });
    assert!(matches!(answered(&world), people::Outcome::AccountRemoved { .. }));
    assert!(world.domain.accounts().account(account).is_none());
}
#[test]
fn a_connector_adopts_its_own_resource_into_a_project() {
    // This story scripts the root at the core boundary: it answers the routed Ask::Adopt.
    let (mut core, env) = adoption_core(1804);
    let adoption = adoption();
    let core::Requests::Out(mut out) = core::step(
        &mut core,
        &env,
        core::Event::Act {
            reply_to: ReplyTo::new(Token::new(1)),
            connector: CONNECTOR,
            key: [KEY; 16],
            act: Box::new(people::Act::Adopt { project: PROJECT, adoption: adoption.clone() }),
            carried: None,
        },
    );
    let mut routed = None;
    while let Some(next) = out.pop() {
        if let core::Request::Ask {
            connector: CONNECTOR,
            ask: core::Ask::Adopt { request, project: PROJECT, adoption: kept },
        } = next
        {
            assert_eq!(kept, adoption);
            routed = Some(request);
        }
    }
    let core::Requests::Out(mut out) = core::step(
        &mut core,
        &env,
        core::Event::People(people::Event::Decided {
            request: routed.expect("adoption routed"),
            outcome: people::Outcome::Adopted { project: PROJECT },
        }),
    );
    let mut saved = false;
    let mut answered = false;
    while let Some(next) = out.pop() {
        match next {
            core::Request::Write(core::Write::Save(core::Record::People(people::Stored::Acted { .. }))) => saved = true,
            core::Request::Held(held)
                if matches!(
                    *held,
                    core::Held::Acted { outcome: people::Outcome::Adopted { project: PROJECT }, .. }
                ) =>
            {
                assert!(saved);
                answered = true;
            }
            core::Request::Write(_)
            | core::Request::Ask { .. }
            | core::Request::Held(_)
            | core::Request::Now(_)
            | core::Request::ProjectMade { .. }
            | core::Request::Decided => {}
        }
    }
    assert!(answered);
}
fn restart_cut(durable: bool) {
    let mut world = world(1805);
    let chat = world.assignments[0].0;
    let number = world.store.applied + 1;
    let cut = if durable { Cut::Durable(number) } else { Cut::Submitted(number) };
    world.cut = Some(cut);
    act(&mut world, KEY, close(chat, PROJECT, None));
    assert_eq!(world.reached_cut, Some(cut));
    assert!(world.act_answers.is_empty());
    let mut config = configuration(1805).0;
    if durable {
        config.core.grants = Box::new([]);
        config.core.owners = Box::new([]);
    }
    world.restart_with(config);
    let before = world.commits.len();
    act(&mut world, KEY, close(chat, PROJECT, None));
    assert!(matches!(answered(&world), people::Outcome::Compound { operations: 1, .. }));
    if durable {
        assert_eq!(world.commits.len(), before);
    } else {
        assert_eq!(world.commits.len(), before + 1);
    }
    cancelled(&world, chat);
}
#[test]
fn a_restart_between_an_acts_commit_and_its_answer_answers_the_resend_once() {
    restart_cut(true);
}
#[test]
fn an_act_lost_to_a_restart_is_made_when_sent_again() {
    restart_cut(false);
}
#[test]
fn an_act_is_refused_without_its_grant_or_an_owner_at_the_core() {
    for (grant, owner, why) in
        [(false, true, people::Refusal::NotOffered), (true, false, people::Refusal::Role { held: None })]
    {
        let (mut config, limits) = configuration(1806);
        if !grant {
            config.core.grants = Box::new([]);
        }
        if !owner {
            config.core.owners = Box::new([]);
        }
        let mut world = World::configured_unassigned(1806, false, config, limits);
        act(&mut world, KEY, close(1, PROJECT, None));
        assert_eq!(answered(&world), &people::Outcome::Refused(why));
        assert!(!world.store.rows.contains_key(&root::Key::Core(core::Key::People(people::Key::Acted(
            people::ActKey { connector: CONNECTOR, key: [KEY; 16] }
        )))));
    }
}
#[test]
fn acts_refuse_a_missing_extra_or_mismatched_carried_value() {
    let mut world = world(1807);
    let chat = world.assignments[0].0;
    let before = world.people_answers.len() + world.act_answers.len();
    for (key, act_value, carried) in [
        (KEY, people::Act::AddKey { provider: PROVIDER, join: Some(0), digest: [7; 32] }, None),
        (KEY + 1, close(chat, PROJECT, None), Some(core::Carried { number: 99, digest: [7; 32] })),
        (
            KEY + 2,
            people::Act::AddKey { provider: PROVIDER, join: Some(0), digest: [7; 32] },
            Some(core::Carried { number: 100, digest: [8; 32] }),
        ),
    ] {
        world.send(event(key, act_value, carried));
        assert_eq!(answered(&world), &people::Outcome::Refused(people::Refusal::NotOffered));
    }
    assert_eq!(world.carried_unused, [(before + 1, 99), (before + 2, 100)]);
}

#[test]
fn a_close_accepts_the_tasks_grace_at_its_bound() {
    const TASK_GRACE_BOUND: Duration = Duration::from_secs(600);
    let mut world = world(1808);
    let chat = world.assignments[0].0;
    act(&mut world, KEY, close(chat, PROJECT, Some(TASK_GRACE_BOUND)));
    assert!(matches!(answered(&world), people::Outcome::Compound { operations: 1, .. }));
    assert_eq!(live_row(&world, chat).grace.as_ref().expect("finish requested").reason.as_ref(), REASON);
}
#[test]
fn a_failed_key_keep_returns_the_value_before_its_answer() {
    const FIRST_VALUE: u64 = 1_u64 << 63;
    let mut world = world(1809);
    let before = world.people_answers.len() + world.act_answers.len();
    world.fail_llm_key_keep = Some(accounts::KeyFailure::NotStored);
    add_key(&mut world, KEY, FIRST_VALUE);
    assert_eq!(answered(&world), &people::Outcome::Again(people::Again::NotReady));
    assert_eq!(world.carried_unused.last(), Some(&(before, FIRST_VALUE)));
    add_key(&mut world, KEY, FIRST_VALUE + 1);
    assert!(matches!(answered(&world), people::Outcome::AccountKeyKept { .. }));
}

fn adoption_core(seed: u64) -> (core::Core, Env<core::Limits>) {
    let (config, limits) = configuration(seed);
    let seeded = jig_core_world::fixtures::seeded_project_rows(&config.core);
    let mut core = core::Core::new(config.core, &limits.core).expect("bounded deployment act fixture");
    for row in seeded {
        let restored = match row {
            core::Record::Core(row) => core.restore_core(row, &limits.core) != core::Restored::Rejected,
            core::Record::People(people::Stored::Policy { project, name, value }) => {
                core.restore_policy(&limits.core, project, name, value)
            }
            core::Record::Tasks(_) | core::Record::People(_) | core::Record::Notes(_) | core::Record::Accounts(_) => {
                false
            }
        };
        assert!(restored, "a seeded project's row restores");
    }
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.core };
    core::step(&mut core, &env, core::Event::People(people::Event::Restored));
    (core, env)
}
fn adoption() -> people::Adoption {
    people::Adoption {
        resource: people::ResourceName { connector: CONNECTOR, path: Box::new([Box::from(&b"own"[..])]) },
        role: people::ResourceRole::Owned,
        options: Box::new([]),
    }
}
#[test]
fn a_deployment_adoption_of_an_unknown_project_is_refused() {
    let (mut core, env) = adoption_core(1810);
    let core::Requests::Out(mut out) = core::step(
        &mut core,
        &env,
        core::Event::Act {
            reply_to: ReplyTo::new(Token::new(1)),
            connector: CONNECTOR,
            key: [KEY; 16],
            act: Box::new(people::Act::Adopt { project: PROJECT + 1, adoption: adoption() }),
            carried: None,
        },
    );
    let mut answered = false;
    while let Some(next) = out.pop() {
        match next {
            core::Request::Ask { .. } => panic!("an unknown project never reaches the connector"),
            core::Request::Held(held)
                if matches!(
                    *held,
                    core::Held::Acted { outcome: people::Outcome::Refused(people::Refusal::Unknown), .. }
                ) =>
            {
                answered = true;
            }
            core::Request::Write(_)
            | core::Request::Held(_)
            | core::Request::Now(_)
            | core::Request::ProjectMade { .. }
            | core::Request::Decided => {}
        }
    }
    assert!(answered);
}

// The seed and one made project, plus deployment roles, fit both child capacities.
fn project_world(granted: bool) -> World {
    let (mut config, mut limits) = configuration(1850);
    limits.core.authority.projects = 2;
    limits.core.people.projects = 3;
    config.core.defaults.rules =
        (jig_domain_authority::Domain::new(config.core.defaults.rules.clone(), limits.core.authority)
            .expect("room past the seed"))
        .rules()
        .clone();
    if granted {
        config.core.grants =
            Box::new([people::ActGrant { connector: CONNECTOR, may: Box::new([people::ActKind::MakeProject]) }]);
    }
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
    World::first_start(config, limits)
}
fn make_project(maker: u64) -> people::Act {
    people::Act::MakeProject { maker, name: "Tally".into(), policy: 1 }
}
#[test]
fn a_connector_granted_make_project_makes_a_project_for_a_deployment_owner() {
    const OWNER: u64 = 1;
    const MADE: u32 = 2;
    let mut world = project_world(true);
    act(&mut world, KEY, make_project(OWNER));
    assert_eq!(answered(&world), &people::Outcome::ProjectMade { project: MADE });
    assert_eq!(
        world.store.rows[&root::Key::Core(core::Key::People(people::Key::Roles(MADE)))],
        root::Record::Core(core::Record::People(people::Stored::Roles {
            project: MADE,
            holdings: Box::new([people::Holding { person: OWNER, role: people::Role::Owner }]),
        }))
    );
    let commits = world.commits.len();
    act(&mut world, KEY, make_project(OWNER));
    assert_eq!(answered(&world), &people::Outcome::ProjectMade { project: MADE });
    assert_eq!(world.commits.len(), commits);
}
#[test]
fn make_project_is_not_offered_to_a_connector_without_its_grant() {
    let mut world = project_world(false);
    act(&mut world, KEY, make_project(1));
    assert_eq!(answered(&world), &people::Outcome::Refused(people::Refusal::NotOffered));
}
#[test]
fn a_make_project_act_refuses_a_maker_outside_its_subject() {
    let mut world = project_world(true);
    act(&mut world, KEY, make_project(u64::MAX));
    assert_eq!(answered(&world), &people::Outcome::Refused(people::Refusal::Unknown));
}
