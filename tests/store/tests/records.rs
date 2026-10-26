//! A source's record in the secrets' store across crashes (store.md 9: a
//! record kept before lent, stating the digest of the one it replaces).
use jig_protocol_store::{Event, SecretOwner, SecretValue, Secrets};
use jig_store_world::{World, root};
use skein_lib::{Secret, sha256};

const SOURCE: u64 = 4;

fn record(byte: u8) -> Secret {
    Secret::new(vec![byte; 300].into_boxed_slice())
}

fn replace(value: u8, replaces: Option<u8>) -> Secrets {
    Secrets::Replace {
        owner: SecretOwner::Source,
        number: SOURCE,
        value: jig_protocol_store::SecretValue::Record(record(value)),
        replaces: replaces.map(|byte| sha256::digest(record(byte).expose())),
    }
}

/// A store holding the source's first record, durable.
fn held(seed: u64) -> World {
    let mut world = World::new(seed);
    assert_eq!(world.commit(root::first()), Some(1));
    let events = world.secret(replace(1, None));
    assert!(matches!(events.as_slice(), [Event::Kept { .. }]), "first record: {events:?}");
    world
}

fn found(world: &mut World) -> Option<SecretValue> {
    let events = world.secret(Secrets::Get { owner: SecretOwner::Source, number: SOURCE });
    let [Event::Secret { value, .. }] = events.as_slice() else { panic!("secret terminal: {events:?}") };
    value.clone()
}

#[test]
fn a_kept_record_survives_a_crash_at_each_step() {
    let count = {
        let mut world = held(18);
        world.secret(replace(2, Some(1)));
        world.operation_count()
    };
    assert!(count > 0, "a replace writes");
    for cut in 0..=count {
        let mut world = held(u64::from(cut));
        world.cut_at(cut);
        let kept = matches!(world.secret(replace(2, Some(1))).as_slice(), [Event::Kept { .. }]);
        world.crash(u64::from(cut));
        world.reopen();
        let found = found(&mut world);
        if kept {
            assert_eq!(found, Some(SecretValue::Record(record(2))), "kept only once durable, cut {cut}");
        } else {
            assert!(
                found == Some(SecretValue::Record(record(1))) || found == Some(SecretValue::Record(record(2))),
                "a whole record, cut {cut}"
            );
        }
        assert!(
            cut > 0 || (!kept && found == Some(SecretValue::Record(record(1)))),
            "nothing written before the first operation"
        );
        assert!(cut < count || kept, "kept once every operation is done");
        let held = if found == Some(SecretValue::Record(record(2))) { 2 } else { 1 };
        let events = world.secret(replace(3, Some(held)));
        assert!(matches!(events.as_slice(), [Event::Kept { .. }]), "the held record replaced, cut {cut}");
    }
}

#[test]
fn a_replace_with_a_stale_digest_is_refused_and_never_written() {
    let mut world = held(18);
    for stated in [Some(2), None] {
        let events = world.secret(replace(3, stated));
        assert!(matches!(events.as_slice(), [Event::Stale { .. }]), "stale {stated:?}: {events:?}");
        assert_eq!(world.operation_count(), 0, "nothing written, {stated:?}");
    }
    world.crash(18);
    world.reopen();
    assert_eq!(found(&mut world), Some(SecretValue::Record(record(1))));
}

#[test]
fn a_replace_states_the_record_on_its_way_not_the_durable_one() {
    let mut world = held(18);
    let events = world.secrets(vec![replace(2, Some(1)), replace(3, Some(1)), replace(4, Some(2))]);
    assert!(
        matches!(events.as_slice(), [Event::Stale { .. }, Event::Kept { .. }, Event::Kept { .. }]),
        "the second states a record no longer held: {events:?}"
    );
    world.crash(18);
    world.reopen();
    assert_eq!(found(&mut world), Some(SecretValue::Record(record(4))));
}
