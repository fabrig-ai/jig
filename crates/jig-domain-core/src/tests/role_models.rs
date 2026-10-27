//! Omitted role edits keep the old selector; explicit edits replace it under the ceiling
//! (domain/authority.md, section 6).
use super::*;

const ROLE: u32 = 4;
const CEILING: authority::Models = authority::Models::Named(0b111);
const KEPT: authority::Models = authority::Models::Named(0b010);

fn project_policy() -> authority::Policy {
    let mut ceiling = authority_value(1000, Box::new([]));
    ceiling.models = CEILING;
    let mut given = authority_value(500, Box::new([]));
    given.models = KEPT;
    authority::Policy {
        projections: Box::new([]),
        escalation_role: None,
        ceiling,
        period_spend: 1000,
        roles: Box::new([authority::Role {
            number: ROLE,
            authority: given,
            period_spend: 500,
            requests: authority::Requests::ALL,
            decides: authority::Proposals::ALL,
        }]),
        requirements: Box::new([]),
    }
}

fn edited(number: u32) -> authority::Policy {
    let mut value = authority_to_people(&authority_value(400, Box::new([]))).unwrap();
    value.models = None;
    let change = people::PolicyChange::Role(people::PolicyRole {
        number,
        authority: value,
        period_spend: 400,
        requests: 0,
        decides: 0,
    });
    let mut policy = project_policy();
    let mut permissions: Box<[people::PermissionRole]> = Box::new([]);
    let mut templates: Box<[people::TemplateOverride]> = Box::new([]);
    policy::apply(&mut policy, &mut permissions, &mut templates, change, 1, 2).unwrap();
    policy
}

#[test]
fn an_edited_role_keeps_its_models() {
    let policy = edited(ROLE);
    assert_eq!(policy.roles[0].authority.models, KEPT);
    assert_eq!(policy.roles[0].authority.budget.spend, 400, "the rest of the edit applies");
}

#[test]
fn a_new_role_takes_every_model_the_ceiling_names() {
    let policy = edited(ROLE + 1);
    assert_eq!(policy.roles[1].number, ROLE + 1);
    assert_eq!(policy.roles[1].authority.models, CEILING);
}

#[test]
fn explicit_role_selectors_narrow_empty_and_widen_under_the_ceiling() {
    for (models, expected) in [
        (people::Models::NONE, authority::Models::NONE),
        (people::Models::Named(1), authority::Models::Named(1)),
        (people::Models::Named(7), CEILING),
    ] {
        let mut policy = project_policy();
        let mut value = authority_to_people(&policy.roles[0].authority).unwrap();
        value.models = Some(models);
        let change = people::PolicyChange::Role(people::PolicyRole {
            number: ROLE,
            authority: value,
            period_spend: 400,
            requests: 0,
            decides: 0,
        });
        policy::apply(
            &mut policy,
            &mut Box::<[people::PermissionRole]>::default(),
            &mut Box::<[people::TemplateOverride]>::default(),
            change,
            1,
            2,
        )
        .unwrap();
        assert_eq!(policy.roles[0].authority.models, expected);
    }
}
#[test]
fn an_explicit_role_past_the_ceiling_refuses_the_entire_edit() {
    for models in [people::Models::Named(8), people::Models::ALL] {
        let (mut core, _) = make_tree();
        let mut policy = core.authority.policy(1).unwrap().clone();
        policy.ceiling.models = CEILING;
        let mut out = Queue::with_capacity(4);
        for role in &mut policy.roles {
            role.authority.models = role.authority.models.and(CEILING);
        }
        authority::step(&mut core.authority, authority::Event::Policy { project: 1, policy: policy.clone() }, &mut out);
        assert_eq!(core.authority.policy(1), Some(&policy));
        let mut value = authority_to_people(&policy.ceiling).unwrap();
        value.models = Some(models);
        let change = people::PolicyChange::Role(people::PolicyRole {
            number: ROLE,
            authority: value,
            period_spend: 1,
            requests: 0,
            decides: 0,
        });
        assert_eq!(
            core.change_policy(&limits(), 1, 1, change),
            Err(people::Refusal::Authority { proposable: None, lacking: people::Lack::Models })
        );
        assert_eq!(core.authority.policy(1), Some(&policy));
    }
}

#[test]
fn role_spend_changes_only_the_named_roles_period_spend() {
    for period_spend in [0, 700] {
        let (mut core, limits) = make_tree();
        let mut expected = core.authority.policy(1).unwrap().clone();
        let role = expected.roles[0].number;
        expected.roles[0].period_spend = period_spend;
        let change = people::PolicyChange::RoleSpend { role, period_spend };
        assert_eq!(people::policy_change_bytes(&change), Some(0));
        let answer = core.change_policy(&limits, 1, 1, change).unwrap();
        assert_eq!(core.authority.policy(1), Some(&expected));
        assert_eq!(
            answer,
            policy::snapshot(
                &expected,
                core.permission_roles.get(&1).unwrap(),
                core.template_overrides.get(&1).unwrap(),
            )
            .unwrap()
        );
    }
}

#[test]
fn role_spend_for_an_absent_role_refuses_unknown_and_keeps_the_policy() {
    // Neither a missing reserved role nor a missing application role is created.
    for role in [1, ROLE] {
        let (mut core, limits) = make_tree();
        let before = core.authority.policy(1).unwrap().clone();
        for value in &before.roles {
            assert_ne!(value.number, role);
        }
        assert_eq!(
            core.change_policy(&limits, 1, 1, people::PolicyChange::RoleSpend { role, period_spend: 700 }),
            Err(people::Refusal::Unknown)
        );
        assert_eq!(core.authority.policy(1), Some(&before));
    }
}
