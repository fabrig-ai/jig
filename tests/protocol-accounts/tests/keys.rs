use jig_domain_accounts::{Form, FromComponent, KeyFailure, Scheme};
use jig_protocol_accounts_world::key_world::World;
use jig_protocol_store::SecretOwner;
use skein_lib::{Secret, Time};

#[test]
fn a_key_is_kept_and_granted_with_the_key_scheme() {
    let mut world = World::new();
    let carried = world.carry(&[7; 32]);
    world.keep(7, carried, false);
    assert!(matches!(
        world.heard.as_slice(),
        [FromComponent::Granted { account: 7, generation: 1, form: Form::Credential(Scheme::Key), .. }]
    ));
    assert_eq!(world.get(SecretOwner::Source, 7), Some(Secret::new(Box::new([7; 32]))));
    assert_eq!(world.get(SecretOwner::Application, carried), None);
    assert_eq!(world.table.value(7, 1, Time::ZERO).expect("valid protocol world fixture").token.expose(), &[7; 32]);
}
#[test]
fn a_key_longer_than_the_token_bound_fails_to_keep() {
    let mut world = World::new();
    let carried = world.carry(&[7; 33]);
    world.keep(7, carried, false);
    assert_eq!(world.heard, [FromComponent::KeepFailed { account: 7, why: KeyFailure::TooLong }]);
    assert_eq!(world.get(SecretOwner::Source, 7), None);
    assert_eq!(world.get(SecretOwner::Application, carried), Some(Secret::new(Box::new([7; 33]))));
    assert!(world.table.value(7, 1, Time::ZERO).is_none());
}
#[test]
fn a_replaced_key_is_the_next_generation() {
    let mut world = World::new();
    let first = world.carry(&[7; 32]);
    world.keep(7, first, false);
    let next = world.carry(&[9; 32]);
    world.keep(7, next, true);
    assert!(matches!(world.heard.last(), Some(FromComponent::Granted { account: 7, generation: 2, .. })));
    assert_eq!(world.table.value(7, 2, Time::ZERO).expect("valid protocol world fixture").token.expose(), &[9; 32]);
    assert_eq!(world.table.value(7, 1, Time::ZERO).expect("valid protocol world fixture").token.expose(), &[7; 32]);
    assert_eq!(world.get(SecretOwner::Source, 7), Some(Secret::new(Box::new([9; 32]))));
}
#[test]
fn a_removed_key_is_withdrawn_and_erased() {
    let mut world = World::new();
    let carried = world.carry(&[7; 32]);
    world.keep(7, carried, false);
    world.forget(7);
    assert!(world.table.value(7, 1, Time::ZERO).is_none());
    assert!(!world.keys.contains(7));
    assert_eq!(world.get(SecretOwner::Source, 7), None);
}
#[test]
fn a_stale_adopt_keeps_the_old_llm_key_and_returns_the_carried_value() {
    let mut world = World::new();
    let first = world.carry(&[7; 32]);
    world.keep(7, first, false);
    world.secret(jig_protocol_store::Secrets::Keep {
        owner: SecretOwner::Source,
        number: 7,
        value: jig_protocol_store::SecretValue::Key(Secret::new(Box::new([8; 32]))),
    });
    let next = world.carry(&[9; 32]);
    world.keep(7, next, true);
    assert_eq!(world.heard.last(), Some(&FromComponent::KeepFailed { account: 7, why: KeyFailure::NotStored }));
    assert_eq!(world.table.value(7, 1, Time::ZERO).expect("valid protocol world fixture").token.expose(), &[7; 32]);
    assert_eq!(world.get(SecretOwner::Application, next), Some(Secret::new(Box::new([9; 32]))));
}
