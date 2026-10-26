use jig_protocol_store::{OpenRefusal, Owner};
use jig_store_world::patches::{
    self,
    flights::{BEFORE, CHANGES_FLIGHT, CHANGES_OTHER, LABEL_NAME, NAMED, NO_RANGES, ROW, SEED, committed, key},
};
use skein_fake_machine::Item;

// The single sent label used by the refusal and unaffected-family stories.
const ONE_FLIGHT: u32 = 1;

fn refused(count: u32) -> (jig_store_world::World, OpenRefusal) {
    let mut world = committed(count);
    world.patch_sets(CHANGES_FLIGHT);
    assert_eq!(world.open(), None);
    let why = world.refusal().expect("typed open refusal").clone();
    (world, why)
}

#[test]
fn a_flight_under_a_pending_patch_of_its_family_refuses_before_any_write() {
    let mut world = committed(ONE_FLIGHT);
    // A refusal must precede cleanup as well as copying and dropping.
    world.lay(&[
        Item::directory(b"generations/0"),
        Item::file(b"generations/0/previous", b"previous generation"),
        Item::directory(b"generations/9"),
        Item::file(b"generations/9/stray", b"cleanup has not run"),
    ]);
    let before = world.contents();
    world.patch_sets(CHANGES_FLIGHT);
    assert_eq!(world.open(), None);
    assert_eq!(
        world.refusal(),
        Some(&OpenRefusal::InFlight {
            owner: Owner::Application,
            family: LABEL_NAME,
            count: ONE_FLIGHT,
            keys: Box::new([key(0)]),
        })
    );
    assert_eq!(world.active(), b"1\n");
    patches::kept(&before, &world.contents(), &"flight refusal");
    assert!(!world.paths().iter().any(|path| path.starts_with(b"generations/2")));
    world.patch_sets(BEFORE);
    world.crash(SEED);
    assert_eq!(world.open(), Some(1));
    assert_eq!(world.application(true).get(&key(0)).map(AsRef::as_ref), Some(ROW));
}

#[test]
fn a_flight_in_a_family_no_pending_patch_changes_is_patched_past() {
    let mut world = committed(ONE_FLIGHT);
    world.patch_sets(CHANGES_OTHER);
    assert!(world.open_switched().is_some(), "opens: {:?}", world.refusal());
    assert_eq!(world.active(), b"2\n");
    assert_eq!(world.application(true).get(&key(0)).map(AsRef::as_ref), Some(ROW));
}

#[test]
fn in_flight_counts_every_record_and_names_at_most_64() {
    for count in [NAMED, NAMED + 1] {
        let (_, why) = refused(count);
        let keys = (0..NAMED).map(key).collect::<Vec<_>>().into_boxed_slice();
        assert_eq!(why, OpenRefusal::InFlight { owner: Owner::Application, family: LABEL_NAME, count, keys });
    }
}

#[test]
fn a_set_naming_no_flight_range_never_refuses() {
    let mut world = committed(ONE_FLIGHT);
    world.patch_sets(NO_RANGES);
    assert!(world.open_switched().is_some(), "opens: {:?}", world.refusal());
    assert_eq!(world.active(), b"2\n");
    assert_eq!(
        world.application(true).get(&key(0)).map(AsRef::as_ref),
        Some(b"\x00\x02\x00\x00\x00\x04sent\x00".as_slice())
    );
}

#[test]
fn in_flight_words_are_the_designs() {
    let why = OpenRefusal::InFlight { owner: Owner::Application, family: LABEL_NAME, count: 0, keys: Box::new([]) };
    assert_eq!(
        why.words().join("<deployment>"),
        "<deployment> holds effects its last run sent and did not settle, in records this build's patches change, and this start has changed nothing in it. Start the build before it again, let it settle them, and stop it: a stop settles what it sent first. Then start this build."
    );
}

const SENT: u64 = 7;
const SECRET_FLIGHT: jig_protocol_store::PatchSets = jig_protocol_store::PatchSets {
    sets: &[jig_protocol_store::PatchSet {
        families: &[
            CHANGES_OTHER.sets[0].families[0],
            jig_protocol_store::KeptFamily {
                flights: &[jig_protocol_store::Keys {
                    from: &SENT.to_be_bytes(),
                    to: &(SENT + 1).to_be_bytes(),
                    root: "Token",
                }],
                ..CHANGES_OTHER.sets[0].families[1]
            },
        ],
        ..CHANGES_OTHER.sets[0]
    }],
};

fn secret_committed() -> jig_store_world::World {
    use jig_protocol_store::{SecretOwner, Secrets};
    use skein_lib::Secret;
    let mut world = committed(ONE_FLIGHT);
    assert_eq!(world.open(), Some(1));
    world.mark().expect("start mark before keeping the sent effect");
    let mut value = b"\x00\x01\x00\x00\x00\x03key".to_vec();
    value.extend(SENT.to_be_bytes());
    world.secret(Secrets::Keep {
        owner: SecretOwner::Application,
        number: SENT,
        value: Secret::new(value.into_boxed_slice()),
    });
    world.crash(SEED);
    world
}

fn secret_refusal() -> OpenRefusal {
    OpenRefusal::InFlight {
        owner: Owner::Application,
        family: "fixture-token",
        count: ONE_FLIGHT,
        keys: Box::new([Box::new(SENT.to_be_bytes())]),
    }
}

#[test]
fn a_secret_flight_is_named_without_its_application_prefix() {
    let mut world = secret_committed();
    world.patch_sets(SECRET_FLIGHT);
    assert_eq!(world.open(), None);
    assert_eq!(world.refusal(), Some(&secret_refusal()));
}

#[test]
fn the_first_family_follows_patch_order_before_family_order() {
    use jig_protocol_store::{PatchSet, PatchSets};
    const LABEL_SECOND: &str =
        "patch 1\nowner fixture 2\n\nfamily fixture-label 1 to 2\nadd Label.color option u8 none\n";
    const BOTH: PatchSets = PatchSets {
        sets: &[PatchSet {
            families: &[CHANGES_FLIGHT.sets[0].families[0], SECRET_FLIGHT.sets[0].families[1]],
            patches: &[CHANGES_OTHER.sets[0].patches[0], LABEL_SECOND],
            ..CHANGES_FLIGHT.sets[0]
        }],
    };
    let mut world = secret_committed();
    world.patch_sets(BOTH);
    assert_eq!(world.open(), None);
    assert_eq!(world.refusal(), Some(&secret_refusal()));
}

#[test]
fn every_declared_flight_range_contributes_to_the_refusal() {
    use jig_protocol_store::{KeptFamily, Keys, PatchSet, PatchSets};
    const SPLIT: PatchSets = PatchSets {
        sets: &[PatchSet {
            families: &[
                KeptFamily {
                    flights: &[
                        Keys { from: b"l/f/000", to: b"l/f/001", root: "Label" },
                        Keys { from: b"l/f/002", to: b"l/f/003", root: "Label" },
                    ],
                    ..CHANGES_FLIGHT.sets[0].families[0]
                },
                CHANGES_FLIGHT.sets[0].families[1],
            ],
            ..CHANGES_FLIGHT.sets[0]
        }],
    };
    // Two flight ranges, with one ordinary label between them.
    const LABELS: u32 = 3;
    const FLIGHT_RANGES: u32 = 2;
    let mut world = committed(LABELS);
    world.patch_sets(SPLIT);
    assert_eq!(world.open(), None);
    assert_eq!(
        world.refusal(),
        Some(&OpenRefusal::InFlight {
            owner: Owner::Application,
            family: LABEL_NAME,
            count: FLIGHT_RANGES,
            keys: Box::new([key(0), key(2)]),
        })
    );
}
