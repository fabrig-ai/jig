//! A value a client's request carried, across crashes (store.md 9: kept
//! durably before the core sees the request, numbered by jig and never
//! reused, erased when its request goes unused).
use jig_protocol_store::{CARRIED, CarriedKind, Event, SecretOwner, SecretValue, Secrets};
use jig_store_world::{World, root};
use skein_lib::Secret;

fn value(byte: u8) -> Secret {
    Secret::new(vec![byte; 300].into_boxed_slice())
}

fn opened(seed: u64) -> World {
    let mut world = World::new(seed);
    assert_eq!(world.commit(root::first()), Some(1));
    world
}

/// The number a carry was kept under, none when it was not answered.
fn carry(world: &mut World, byte: u8, kind: CarriedKind) -> Option<u64> {
    match world.secret(Secrets::Carry { kind, value: value(byte) }).as_slice() {
        [Event::Carried { number, .. }] => Some(*number),
        [] => None,
        events => panic!("a carry's terminal: {events:?}"),
    }
}

fn found(world: &mut World, number: u64) -> Option<SecretValue> {
    let events = world.secret(Secrets::Get { owner: SecretOwner::Application, number });
    let [Event::Secret { value, .. }] = events.as_slice() else { panic!("secret terminal: {events:?}") };
    value.clone()
}

#[test]
fn a_carried_value_is_durable_once_answered_and_its_number_moves_with_it() {
    let count = {
        let mut world = opened(18);
        carry(&mut world, 1, CarriedKind::Value);
        world.operation_count()
    };
    assert!(count > 0, "a carry writes");
    for cut in 0..=count {
        let mut world = opened(u64::from(cut));
        world.cut_at(cut);
        let carried = carry(&mut world, 1, CarriedKind::Value);
        world.crash(u64::from(cut));
        world.reopen();
        let kept = found(&mut world, CARRIED);
        if let Some(number) = carried {
            assert_eq!(number, CARRIED);
            assert_eq!(
                kept,
                Some(SecretValue::Carried { kind: CarriedKind::Value, value: value(1) }),
                "kept only once durable, cut {cut}"
            );
        } else {
            assert!(
                kept.is_none() || kept == Some(SecretValue::Carried { kind: CarriedKind::Value, value: value(1) }),
                "a whole value, cut {cut}"
            );
        }
        assert!(cut < count || carried.is_some(), "carried once every operation is done");
        let next = if kept.is_some() { CARRIED + 1 } else { CARRIED };
        assert_eq!(carry(&mut world, 2, CarriedKind::Value), Some(next), "the counter moved with the value, cut {cut}");
        assert_eq!(found(&mut world, next), Some(SecretValue::Carried { kind: CarriedKind::Value, value: value(2) }));
        if kept.is_some() {
            assert_eq!(
                found(&mut world, CARRIED),
                Some(SecretValue::Carried { kind: CarriedKind::Value, value: value(1) }),
                "never overwritten, cut {cut}"
            );
        }
    }
}

#[test]
fn an_erased_carried_value_stays_erased_and_its_number_is_never_reused() {
    let mut world = opened(18);
    assert_eq!(carry(&mut world, 1, CarriedKind::Value), Some(CARRIED));
    assert_eq!(carry(&mut world, 2, CarriedKind::Value), Some(CARRIED + 1));
    let events = world.secret(Secrets::Erase { owner: SecretOwner::Application, number: CARRIED + 1 });
    assert!(matches!(events.as_slice(), [Event::Erased { .. }]), "erased: {events:?}");
    world.crash(18);
    world.reopen();
    assert_eq!(found(&mut world, CARRIED), Some(SecretValue::Carried { kind: CarriedKind::Value, value: value(1) }));
    assert_eq!(found(&mut world, CARRIED + 1), None, "erased for good");
    assert_eq!(
        carry(&mut world, 3, CarriedKind::Value),
        Some(CARRIED + 2),
        "the highest number erased is not drawn again"
    );
}

#[test]
fn adopt_moves_a_carried_value_in_one_commit() {
    fn adopt(world: &mut World, carried: u64) -> bool {
        matches!(
            world.secret(Secrets::Adopt { carried, owner: SecretOwner::Source, number: 7, replaces: None }).as_slice(),
            [Event::Kept { .. }]
        )
    }
    let count = {
        let mut world = opened(18);
        let carried = carry(&mut world, 7, CarriedKind::Key).expect("carried LLM key");
        assert!(adopt(&mut world, carried));
        world.operation_count()
    };
    for cut in 0..=count {
        let mut world = opened(u64::from(cut));
        let carried = carry(&mut world, 7, CarriedKind::Key).expect("carried LLM key");
        world.cut_at(cut);
        let kept = adopt(&mut world, carried);
        world.crash(u64::from(cut));
        world.reopen();
        let original = found(&mut world, carried);
        let target = world.secret(Secrets::Get { owner: SecretOwner::Source, number: 7 });
        let [Event::Secret { value: target, .. }] = target.as_slice() else { panic!("source LLM key") };
        assert!(
            (original == Some(SecretValue::Carried { kind: CarriedKind::Key, value: value(7) }) && target.is_none())
                || (original.is_none() && *target == Some(SecretValue::Key(value(7)))),
            "adoption changes both rows together, cut {cut}"
        );
        if kept {
            assert_eq!(*target, Some(SecretValue::Key(value(7))), "answered only once durable");
        }
    }
}

fn adopt_value(world: &mut World, carried: u64, replaces: Option<[u8; 32]>) -> Vec<Event> {
    world.secret(Secrets::AdoptValue {
        carried,
        holds: skein_lib::sha256::digest(value(7).expose()),
        owner: SecretOwner::Source,
        number: 7,
        value: jig_protocol_store::SecretValue::Record(value(8)),
        replaces,
    })
}

#[test]
fn adopt_value_changes_both_rows_in_one_commit() {
    let count = {
        let mut world = opened(18);
        let carried = carry(&mut world, 7, CarriedKind::Moved).expect("carried file");
        assert!(matches!(adopt_value(&mut world, carried, None).as_slice(), [Event::Kept { .. }]));
        world.operation_count()
    };
    for cut in 0..=count {
        let mut world = opened(u64::from(cut));
        let carried = carry(&mut world, 7, CarriedKind::Moved).expect("carried file");
        world.cut_at(cut);
        let kept = matches!(adopt_value(&mut world, carried, None).as_slice(), [Event::Kept { .. }]);
        world.crash(u64::from(cut));
        world.reopen();
        let original = found(&mut world, carried);
        let target = world.secret(Secrets::Get { owner: SecretOwner::Source, number: 7 });
        let [Event::Secret { value: target, .. }] = target.as_slice() else { panic!("source record") };
        assert!(
            (original == Some(SecretValue::Carried { kind: CarriedKind::Moved, value: value(7) }) && target.is_none())
                || (original.is_none() && *target == Some(SecretValue::Record(value(8)))),
            "one commit, cut {cut}"
        );
        if kept {
            assert_eq!(*target, Some(SecretValue::Record(value(8))), "answered once durable");
        }
    }
}

#[test]
fn adopt_value_is_stale_when_either_digest_changed() {
    let held_target = Secrets::Keep { owner: SecretOwner::Source, number: 7, value: SecretValue::Record(value(9)) };
    for (byte, writes, expected_target) in
        [(9, vec![], None), (7, vec![held_target], Some(SecretValue::Record(value(9))))]
    {
        let mut world = opened(19);
        let carried = carry(&mut world, byte, CarriedKind::Moved).expect("carried file");
        for write in writes {
            assert!(matches!(world.secret(write).as_slice(), [Event::Kept { .. }]));
        }
        assert!(matches!(adopt_value(&mut world, carried, None).as_slice(), [Event::Stale { .. }]));
        assert_eq!(
            found(&mut world, carried),
            Some(SecretValue::Carried { kind: CarriedKind::Moved, value: value(byte) })
        );
        let target = world.secret(Secrets::Get { owner: SecretOwner::Source, number: 7 });
        let [Event::Secret { value: target, .. }] = target.as_slice() else { panic!("source record") };
        assert_eq!(*target, expected_target);
    }
}
