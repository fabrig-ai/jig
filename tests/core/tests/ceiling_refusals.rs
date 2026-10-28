//! Policy ceilings precede period room and survive the keyed decision (authority.md, 6).
use jig_core_world::{effects::World, world};
use jig_domain_core::{self as core, settings};
use jig_domain_people as people;
use jig_test_domain as root;
use skein_lib::{ReplyTo, Token};
#[path = "settings/support.rs"]
mod support;

fn start() -> World {
    let (_, limits) = jig_core_world::effects::fixture(630, false);
    World::configured(630, false, world::config(630), limits)
}
fn ask(world: &mut World, key: u8, change: people::PolicyChange) {
    world.send(root::Event::Core(core::Event::People(people::Event::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(Token::new(9000 + u64::from(key))),
        sign_in: world.sign_in.expect("signed-in owner"),
        key: [key; 16],
        ask: people::Ask::ChangePolicy { project: 1, change },
    })));
}
fn refused(world: &World, expected: people::Ceiling) {
    assert_eq!(
        world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::Refused(people::Refusal::Authority {
            lacking: people::Lack::Ceiling(expected),
            proposable: None
        },)))
    );
}
fn policy(world: &World) -> root::Record {
    world.store.rows[&root::Key::Core(core::Key::People(people::Key::Policy(1)))].clone()
}
#[test]
fn a_project_raise_past_the_rules_names_the_limit_and_replays_it() {
    let mut world = start();
    let before = policy(&world);
    for _ in 0..2 {
        ask(&mut world, 61, people::PolicyChange::ProjectSpend { period_spend: 1001 });
        refused(
            &world,
            people::Ceiling {
                holder: people::CeilingHolder::Rules,
                role: None,
                dimension: people::CeilingDimension::PeriodSpend,
                value: Some(1000),
                edit: Some(1001),
            },
        );
        assert_eq!(policy(&world), before, "a refusal cannot change policy");
    }
}
#[test]
fn a_role_raise_past_both_the_project_ceiling_and_period_room_names_the_ceiling() {
    let mut world = start();
    let before = policy(&world);
    ask(&mut world, 62, people::PolicyChange::RoleSpend { role: 0, period_spend: 1001 });
    refused(
        &world,
        people::Ceiling {
            holder: people::CeilingHolder::Project,
            role: Some(0),
            dimension: people::CeilingDimension::PeriodSpend,
            value: Some(1000),
            edit: Some(1001),
        },
    );
    assert_eq!(policy(&world), before);
}
#[test]
fn a_project_cut_below_a_role_names_the_role_to_lower_first() {
    let mut world = start();
    let before = policy(&world);
    ask(&mut world, 63, people::PolicyChange::ProjectSpend { period_spend: 499 });
    refused(
        &world,
        people::Ceiling {
            holder: people::CeilingHolder::Role { role: 0 },
            role: None,
            dimension: people::CeilingDimension::PeriodSpend,
            value: Some(500),
            edit: Some(499),
        },
    );
    assert_eq!(policy(&world), before);
}
#[test]
fn a_rechecked_project_authority_names_the_narrowed_rules_dimension() {
    let mut world = support::start(64);
    let signed = support::sign_in(&mut world);
    let mut rules = world.domain.settings().rules().clone();
    rules.ceiling.tools = jig_domain_authority::Tools(0);
    let answer = support::change(
        &mut world,
        signed,
        64,
        1,
        Box::new([
            settings::Item { setting: settings::Setting::Policy(1), edit: settings::Edit::Removed },
            settings::Item {
                setting: settings::Setting::Rules,
                edit: settings::Edit::Set(settings::Value::Rules(Box::new(rules))),
            },
        ]),
    );
    assert_eq!(world.domain.settings().newest(), 2, "the narrowing applies: {answer:?}");
    world.client_peer = None;
    world.sign_in = Some(signed);
    let before = policy(&world);
    ask(&mut world, 65, people::PolicyChange::ProjectSpend { period_spend: 100 });
    refused(
        &world,
        people::Ceiling {
            holder: people::CeilingHolder::Rules,
            role: None,
            dimension: people::CeilingDimension::Tools,
            value: None,
            edit: None,
        },
    );
    assert_eq!(policy(&world), before);
}
