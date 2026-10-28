//! Each kept frame family opens and decodes independently of the maps.
#[path = "support/frame_reads.rs"]
mod frame_reads;
use frame_reads::{SETS, closed, decode, open, turn};
use jig_protocol_store::{KeptFamily, PatchSet, PatchSets};
use jig_test_patches_jig::frames;
// A body needs the held family's name, even when that family keeps no text.
const BODY_ONLY: PatchSets = PatchSets {
    sets: &[
        SETS.sets[0],
        PatchSet { families: &[KeptFamily { texts: &[], ..SETS.sets[1].families[0] }], patches: &[], ..SETS.sets[1] },
    ],
};
const TURN_ONLY: PatchSets = PatchSets { sets: &[SETS.sets[1]] };

#[test]
fn each_serving_machine_decodes_its_kept_frames() {
    let limits = jig_store_world::patches::limits();
    for (label, sets, body) in [("body and turn", SETS, 5), ("body", BODY_ONLY, 5), ("turn", TURN_ONLY, 6)] {
        let raw = frames::frame(body, &turn(1, b"hello", b"r", 0));
        let (mut component, env) = closed(sets, &limits);
        open(&mut component, &env);
        let result = decode(&mut component, &env, &raw, false);
        assert!(result.is_some(), "{label}");
    }
}
