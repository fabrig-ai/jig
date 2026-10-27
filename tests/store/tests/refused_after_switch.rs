//! A child refuses its pending restore before activation (store.md 10.2, step 7).
//! The store checks encodings and counts; the modelled child's stricter rule
//! runs over the copy. Ending that start leaves the old generation active.
use jig_store_world::patches::{self, BASE, Records, SETS};
use jig_test_patches::patches::kept::fixture_shelf;

/// A referee rule standing in for a child's restore rule (engine.md 6);
/// the store world has no children, and jig's store has no such rule.
/// This child accepts shelves weighing at most 30.
fn a_child_refuses(key: &[u8], bytes: &[u8]) -> bool {
    if !key.starts_with(b"s/") {
        return false;
    }
    let schema = skein_codec::schema::parse(fixture_shelf::TEXTS[1].as_bytes()).expect("kept shelf schema");
    let root = schema.find(b"Shelf").expect("shelf root");
    let mut value = skein_codec::value::Value::with_room(16);
    let shelf = value.decode(&schema, root, bytes.into()).expect("patched shelf decodes");
    let weight = value.field(shelf, fixture_shelf::v2::shelf::WEIGHT).expect("shelf weight field");
    value.uint(weight).expect("weight is a u32") > 30
}

#[test]
fn a_record_a_child_refuses_before_switch_keeps_the_old_generation_active() {
    const SEED: u64 = 911;
    let records = Records::story();
    let mut world = records.committed(SEED, BASE);
    let original = patches::source(&mut world);
    world.patch_sets(SETS);
    let last = world.open().expect("pending patched open");
    assert!(world.pending);
    assert_eq!(world.active(), b"1\n");
    records.check_public(&mut world, 2, &"pending restore");

    let key = b"s/2".as_slice();
    let expected = records.rows(2).get(key).expect("story's heaviest shelf").clone();
    // The modelled child ends this start before asking for Switch.
    let refused: Vec<_> =
        world.application(false).into_iter().filter(|(key, bytes)| a_child_refuses(key, bytes)).collect();
    assert_eq!(refused, [(key.into(), expected.clone())]);
    world.crash(SEED);

    for start in ["first further start", "second further start"] {
        assert_eq!(world.open(), Some(last), "{start}");
        assert!(!world.patched().is_empty(), "{start} patches a fresh copy");
        assert!(world.pending);
        assert_eq!(world.active(), b"1\n", "{start}");
        let refused: Vec<_> =
            world.application(false).into_iter().filter(|(key, bytes)| a_child_refuses(key, bytes)).collect();
        assert_eq!(refused, [(key.into(), expected.clone())], "{start}");
        patches::kept(&original, &patches::source(&mut world), &start);
        world.crash(SEED);
    }

    // The old binary starts against A without any operator rewriting active.
    world.patch_sets(BASE);
    assert_eq!(world.open_switched(), Some(2));
    world.accept_recovery(2);
    assert!(world.patched().is_empty());
    assert_eq!(world.active(), b"1\n");
    patches::kept(&original, &patches::source(&mut world), &"operator's way back");
    records.check(&mut world, 1, &"operator's way back");
    assert!(!world.paths().iter().any(|path| path == b"generations/2" || path.starts_with(b"generations/2/")));
}
