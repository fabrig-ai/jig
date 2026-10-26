//! Secrets and people's orphan cleanup wait for this start's durable mark (store.md 5.3).
use jig_domain_core as core;
use jig_protocol_store::{Event, Request, Restore, SecretOwner, Secrets, Write};
use jig_store_world::{
    World,
    patches::{BASE, SETS},
    root,
};
use skein_lib::{Secret, Token};

const SEED: u64 = 963;
const OWNER: Token = Token::new(964);
const VALUE: &[u8] = b"private fixture";
const SECRET: u64 = 7;
// One terminal page of the fixture's empty People range.
const PAGE_ROWS: u32 = 2;
const PAGE_BYTES: u64 = 2048;

fn read(kind: SecretOwner) -> Request {
    Request::Secrets { owner: OWNER, request: Secrets::Get { owner: kind, number: SECRET } }
}

fn restored_people(world: &mut World) {
    world.request(Request::Restore {
        owner: OWNER,
        range: Restore::Jig(Box::new([core::RestoreRange::People])),
        after: None,
        rows: PAGE_ROWS,
        bytes: PAGE_BYTES,
    });
}

fn seeded() -> World {
    let mut world = World::unopened(SEED);
    world.patch_sets(BASE);
    world.open().expect("initial open");
    world.mark().expect("previous start mark");
    for owner in [SecretOwner::Source, SecretOwner::SignIn] {
        world.secret(Secrets::Keep { owner, number: SECRET, value: Secret::new(VALUE.into()) });
    }
    world.crash(SEED);
    world
}

#[test]
fn secrets_wait_for_the_mark() {
    for sets in [BASE, SETS] {
        let mut world = seeded();
        world.patch_sets(sets);
        let last = world.open().expect("ordinary or pending open");
        let before = world.contents();
        restored_people(&mut world);
        assert!(matches!(
            world.request(read(SecretOwner::Source)).as_slice(),
            [Event::SecretsNotReady { owner: OWNER }]
        ));
        assert_eq!(world.contents(), before, "no sweep or secret mutation before the mark");
        if world.pending {
            world.switch().expect("activate checked copy");
        }
        // Switching alone cannot release a secret or the orphan sweep.
        assert!(matches!(
            world.request(read(SecretOwner::Source)).as_slice(),
            [Event::SecretsNotReady { owner: OWNER }]
        ));
        world.enqueue(Request::Commit {
            number: last + 1,
            writes: Box::new([Write::Save(root::header(last + 1)), Write::Wires]),
        });
        assert!(
            matches!(
                world.request(read(SecretOwner::Source)).as_slice(),
                [Event::SecretsNotReady { owner: OWNER }, Event::Committed { .. }]
            ),
            "an accepted mark is still gated until Committed"
        );
        let events = world.request(read(SecretOwner::Source));
        assert!(matches!(events.as_slice(), [Event::Secret { value: Some(value), .. }] if value.expose() == VALUE));
        assert!(
            matches!(world.request(read(SecretOwner::SignIn)).as_slice(), [Event::Secret { value: None, .. }]),
            "the deferred sweep removed the orphan"
        );
        let second = last + 2;
        world.enqueue(Request::Commit {
            number: second,
            writes: Box::new([Write::Save(root::header(second)), Write::Wires]),
        });
        assert!(
            matches!(world.request(read(SecretOwner::Source)).first(), Some(Event::Secret { value: Some(_), .. })),
            "a second Wires cannot close the gate"
        );
    }
}

#[test]
fn the_first_start_waits_for_its_mark_too() {
    let mut world = World::new(SEED);
    assert!(matches!(world.request(read(SecretOwner::Source)).as_slice(), [Event::SecretsNotReady { .. }]));
    world.mark().expect("first start mark");
    assert!(matches!(world.request(read(SecretOwner::Source)).as_slice(), [Event::Secret { value: None, .. }]));
}
