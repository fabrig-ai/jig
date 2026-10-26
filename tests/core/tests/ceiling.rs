//! Owners replace a copied project ceiling within the current deployment rules (authority.md, 6).
use jig_core_world::effects::World;
use jig_domain_authority as authority;
use jig_domain_core::{self as core, settings};
use jig_domain_people as people;
use jig_test_domain as root;
use skein_lib::{ReplyTo, Token};
#[path = "settings/support.rs"]
mod support;
use support::*;

fn policy(world: &World) -> &people::PolicyValue {
    let root::Record::Core(core::Record::People(people::Stored::Policy { value, .. })) =
        &world.store.rows[&root::Key::Core(core::Key::People(people::Key::Policy(1)))]
    else {
        panic!("committed project policy")
    };
    value
}
fn replace_rules(world: &mut World, signed: u64, rules: authority::Rules) {
    changed(
        &change(
            world,
            signed,
            90,
            1,
            Box::new([settings::Item {
                setting: settings::Setting::Rules,
                edit: settings::Edit::Set(settings::Value::Rules(Box::new(rules))),
            }]),
        ),
        2,
    );
}
fn replace_ceiling(world: &mut World, signed: u64, key: u8, ceiling: people::Authority) -> people::Reply {
    let models: Box<[Box<[u8]>]> = if ceiling.models == Some(people::Models::Named(1)) {
        Box::new([Box::from(b"scripted".as_slice())])
    } else {
        Box::new([])
    };
    world.send(root::Event::Core(core::Event::People(people::Event::Ask {
        models,
        reply_to: ReplyTo::new(Token::new(SEED)),
        sign_in: signed,
        key: [key; 16],
        ask: people::Ask::ChangePolicy { project: 1, change: people::PolicyChange::Ceiling { ceiling } },
    })));
    let (_, root::Delivery::Core(core::Held::PeopleReply { reply, .. })) =
        world.client_peer.as_mut().expect("client").delivered.pop().expect("ceiling reply")
    else {
        panic!("ceiling reply")
    };
    reply
}
fn shape_refusal() -> people::Reply {
    people::Reply::Outcome(people::Outcome::Refused(people::Refusal::Authority {
        proposable: None,
        lacking: people::Lack::Shape,
    }))
}
#[test]
fn an_owner_widens_a_project_ceiling_within_the_rules() {
    let mut world = start(8);
    let signed = sign_in(&mut world);
    let before = policy(&world).clone();
    let widened = before.ceiling.delegation.depth + 1;
    let mut rules = world.domain.settings().rules().clone();
    rules.ceiling.delegation.depth = widened;
    replace_rules(&mut world, signed, rules);
    assert_eq!(policy(&world), &before, "a settings change leaves the project's copy alone");
    let mut ceiling = before.ceiling.clone();
    ceiling.delegation.depth = widened;
    assert_eq!(
        replace_ceiling(&mut world, signed, 1, ceiling.clone()),
        people::Reply::Outcome(people::Outcome::PolicyChanged { project: 1 })
    );
    assert_eq!(policy(&world).ceiling, ceiling);
    assert_eq!(policy(&world).roles, before.roles, "only the ceiling was edited");
    let mut role = before.roles[0].clone();
    role.authority.delegation.depth = widened;
    assert_eq!(
        ask(
            &mut world,
            signed,
            2,
            people::Ask::ChangePolicy { project: 1, change: people::PolicyChange::Role(role.clone()) }
        ),
        people::Reply::Outcome(people::Outcome::PolicyChanged { project: 1 })
    );
    assert_eq!(policy(&world).roles[0], role);
    restart(&mut world, configuration().0);
    assert_eq!(policy(&world).ceiling, ceiling, "the widened copy survives cold restoration");
    assert_eq!(policy(&world).roles[0], role);
}
#[test]
fn a_ceiling_past_the_rules_is_refused() {
    let mut world = start(8);
    let signed = sign_in(&mut world);
    let before = policy(&world).clone();
    let mut ceiling = before.ceiling.clone();
    ceiling.delegation.depth += 1;
    let (config, limits) = configuration();
    let mut candidate = config.core.seed_projects[0].policy.policy.clone();
    candidate.ceiling.delegation.depth = ceiling.delegation.depth;
    assert_eq!(
        authority::policy_refusal(world.domain.settings().rules(), &limits.core.authority, &candidate),
        Some(authority::PolicyRefusal::AboveRules)
    );
    assert_eq!(replace_ceiling(&mut world, signed, 1, ceiling), shape_refusal());
    assert_eq!(policy(&world), &before);
}
#[test]
fn a_ceiling_below_an_existing_role_refuses_the_whole_edit() {
    let mut world = start(8);
    let signed = sign_in(&mut world);
    let before = policy(&world).clone();
    let mut ceiling = before.ceiling.clone();
    ceiling.delegation.depth = before.roles[0].authority.delegation.depth - 1;
    let (config, limits) = configuration();
    let mut candidate = config.core.seed_projects[0].policy.policy.clone();
    candidate.ceiling.delegation.depth = ceiling.delegation.depth;
    assert_eq!(
        authority::policy_refusal(world.domain.settings().rules(), &limits.core.authority, &candidate),
        Some(authority::PolicyRefusal::InvalidRole)
    );
    assert_eq!(replace_ceiling(&mut world, signed, 1, ceiling.clone()), shape_refusal());
    assert_eq!(policy(&world), &before);
    let mut role = before.roles[0].clone();
    role.authority.delegation.depth = ceiling.delegation.depth;
    assert_eq!(
        ask(&mut world, signed, 2, people::Ask::ChangePolicy { project: 1, change: people::PolicyChange::Role(role) }),
        people::Reply::Outcome(people::Outcome::PolicyChanged { project: 1 })
    );
    assert_eq!(
        replace_ceiling(&mut world, signed, 3, ceiling.clone()),
        people::Reply::Outcome(people::Outcome::PolicyChanged { project: 1 })
    );
    assert_eq!(policy(&world).ceiling, ceiling);
}
#[test]
fn a_ceiling_models_past_the_rules_are_refused() {
    let mut world = start(8);
    let signed = sign_in(&mut world);
    let before = policy(&world).clone();
    let mut rules = world.domain.settings().rules().clone();
    rules.ceiling.models = authority::Models::NONE;
    replace_rules(&mut world, signed, rules);
    for (key, models) in [(1, people::Models::ALL), (2, people::Models::Named(1))] {
        let mut ceiling = before.ceiling.clone();
        ceiling.models = Some(models);
        assert_eq!(
            replace_ceiling(&mut world, signed, key, ceiling),
            people::Reply::Outcome(people::Outcome::Refused(people::Refusal::Authority {
                proposable: None,
                lacking: people::Lack::Models
            }))
        );
        assert_eq!(policy(&world), &before);
    }
}
#[test]
fn a_member_cannot_change_the_ceiling() {
    let mut world = start(8);
    let _owner = sign_in(&mut world);
    let before = policy(&world).clone();
    world.send(root::Event::Core(core::Event::SignIn {
        held: None,
        reply_to: ReplyTo::new(Token::new(91)),
        identity: people::Identity {
            key: people::IdentityKey { provider: 1, subject: Box::from(b"member".as_slice()) },
            login: Box::new([]),
            name: Box::new([]),
        },
    }));
    let (
        _,
        root::Delivery::Core(core::Held::PeopleReply {
            reply: people::Reply::SignedIn { person, sign_in, .. }, ..
        }),
    ) = world.client_peer.as_mut().expect("client").delivered.pop().expect("signed in")
    else {
        panic!("member sign-in")
    };
    world.send(root::Event::Core(core::Event::People(people::Event::Roles {
        project: 1,
        holdings: Box::new([
            people::Holding { person: OWNER, role: people::Role::Owner },
            people::Holding { person, role: people::Role::Member },
        ]),
    })));
    assert!(matches!(
        replace_ceiling(&mut world, sign_in, 1, before.ceiling.clone()),
        people::Reply::Outcome(people::Outcome::Refused(people::Refusal::Role { .. }))
    ));
    assert_eq!(policy(&world), &before);
}
