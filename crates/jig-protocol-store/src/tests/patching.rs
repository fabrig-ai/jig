//! Static sets, owner ordering and record stops (store.md 10.3).
use crate::patching::machines::{FamilyIx, Machines, Room, Rooms};
use crate::patching::plan::{Levels, Planned, Unplanned};
use crate::patching::walk::Stop;
use crate::patching::{machines, plan, walk};
use crate::{KeptFamily, KeptMap, Keys, Owner, PatchSet, PatchSets};
use jig_test_patches::{self as fixture, patches as app};
use jig_test_patches_jig::{self as upstream, patches as jig};
use skein_codec::patch::{Halt, Site};
use skein_codec::schema::{self, Schema};
use skein_codec::value::{Node, Value};
use skein_lib::{Writer, bytes};

const LABEL: KeptFamily = KeptFamily {
    flights: &[],
    frames: None,
    name: "fixture-label",
    map: KeptMap::Store,
    texts: app::kept::fixture_label::TEXTS,
    keys: &[Keys { from: fixture::keys::LABELS_FROM, to: fixture::keys::LABELS_TO, root: fixture::keys::LABELS_ROOT }],
};
const SHELF: KeptFamily = KeptFamily {
    flights: &[],
    frames: None,
    name: "fixture-shelf",
    map: KeptMap::Store,
    texts: app::kept::fixture_shelf::TEXTS,
    keys: &[Keys {
        from: fixture::keys::SHELVES_FROM,
        to: fixture::keys::SHELVES_TO,
        root: fixture::keys::SHELVES_ROOT,
    }],
};
const TOKEN: KeptFamily = KeptFamily {
    flights: &[],
    frames: None,
    name: "fixture-token",
    texts: app::kept::fixture_token::TEXTS,
    map: KeptMap::Secrets,
    keys: &[Keys { from: fixture::keys::TOKENS_FROM, to: fixture::keys::TOKENS_TO, root: fixture::keys::TOKENS_ROOT }],
};
const NOTE: KeptFamily = KeptFamily {
    flights: &[],
    frames: None,
    name: "fixture-note",
    map: KeptMap::Store,
    texts: jig::kept::fixture_note::TEXTS,
    keys: &[Keys { from: &[1, b'n'], to: &[1, b'o'], root: upstream::keys::NOTES_ROOT }],
};
const APPLICATION: PatchSet = PatchSet {
    fresh: None,
    owner: Owner::Application,
    name: fixture::NAME,
    families: &[LABEL, SHELF, TOKEN],
    patches: app::PATCHES,
};
const JIG: PatchSet =
    PatchSet { fresh: None, owner: Owner::Jig, name: upstream::NAME, families: &[NOTE], patches: jig::PATCHES };
pub(super) const SETS: PatchSets = PatchSets { sets: &[APPLICATION, JIG] };
pub(super) const BASE: PatchSets = PatchSets {
    sets: &[
        PatchSet {
            families: &[
                KeptFamily { texts: &[app::kept::fixture_label::TEXTS[0]], ..LABEL },
                KeptFamily { texts: &[app::kept::fixture_shelf::TEXTS[0]], ..SHELF },
                KeptFamily { texts: &[app::kept::fixture_token::TEXTS[0]], ..TOKEN },
            ],
            patches: &[],
            ..APPLICATION
        },
        PatchSet {
            families: &[KeptFamily { texts: &[jig::kept::fixture_note::TEXTS[0]], ..NOTE }],
            patches: &[],
            ..JIG
        },
    ],
};
pub(super) const ROOM: Room = Room { bytes: 128, nodes: 16 };
pub(super) const ROOMS: Rooms = Rooms { store: ROOM, secrets: ROOM };
const LABEL_STEP: Planned = Planned { owner: Owner::Application, patch: 1 };
const SHELF_STEP: Planned = Planned { owner: Owner::Application, patch: 2 };
const JIG_STEP: Planned = Planned { owner: Owner::Jig, patch: 1 };
const FRAME_STEP: Planned = Planned { owner: Owner::Jig, patch: 2 };

pub(super) fn levels(application: u32, jig: u32) -> Levels {
    Levels { skein: None, smith: None, jig: Some(jig), application: Some(application) }
}
#[test]
fn the_binary_is_before_baseline_only_without_any_kept_text() {
    for (sets, expected) in [
        (PatchSets::NONE, Levels { skein: None, smith: None, jig: None, application: None }),
        (
            PatchSets { sets: &[PatchSet { families: &[], ..APPLICATION }] },
            Levels { skein: None, smith: None, jig: None, application: None },
        ),
        (
            PatchSets { sets: &[PatchSet { families: &[KeptFamily { texts: &[], ..LABEL }], ..APPLICATION }] },
            Levels { skein: None, smith: None, jig: None, application: None },
        ),
        (BASE, levels(0, 0)),
        (SETS, levels(2, 2)),
    ] {
        assert_eq!(plan::binary(&sets), expected);
    }
}
#[test]
fn patches_wait_for_exact_upstream_levels() {
    for (found, expected) in [
        (levels(0, 0), &[LABEL_STEP, JIG_STEP, SHELF_STEP, FRAME_STEP][..]),
        (levels(1, 0), &[JIG_STEP, SHELF_STEP, FRAME_STEP][..]),
        (levels(2, 0), &[JIG_STEP, FRAME_STEP][..]),
        (levels(2, 1), &[FRAME_STEP][..]),
        (levels(2, 2), &[][..]),
    ] {
        let plan = plan::plan(&SETS, &found, &plan::binary(&SETS)).expect("fixture plans");
        assert_eq!(plan.steps(), expected);
    }
}

const OWNER_OK: &str = "patch 1\nowner fixture 1\n";
const LEVEL_TWO: &str = "patch 1\nowner fixture 2\n";
const UPSTREAM_ONE: &str = "patch 1\nowner fixture 1 after fixture-jig 1\n";
const UPSTREAM_ZERO: &str = "patch 1\nowner fixture 1 after fixture-jig 0\n";
const UPSTREAM_THREE: &str = "patch 1\nowner fixture 1 after fixture-jig 3\n";
const SET_OWNER_OK: PatchSets = PatchSets { sets: &[PatchSet { patches: &[OWNER_OK], ..APPLICATION }, JIG] };
const SET_LEVEL_TWO: PatchSets = PatchSets { sets: &[PatchSet { patches: &[LEVEL_TWO], ..APPLICATION }, JIG] };
const SET_UPSTREAM_ZERO: PatchSets = PatchSets { sets: &[PatchSet { patches: &[UPSTREAM_ZERO], ..APPLICATION }, JIG] };
const SET_UPSTREAM_ONE: PatchSets = PatchSets { sets: &[PatchSet { patches: &[UPSTREAM_ONE], ..APPLICATION }, JIG] };
const SET_UPSTREAM_THREE: PatchSets =
    PatchSets { sets: &[PatchSet { patches: &[UPSTREAM_THREE], ..APPLICATION }, JIG] };
#[test]
fn duplicate_owners_are_refused_beside_a_single_owner() {
    let sets = PatchSets { sets: &[APPLICATION, APPLICATION, JIG] };
    assert_eq!(
        plan::plan(&sets, &levels(0, 0), &levels(2, 1)).expect_err("duplicate owner"),
        Unplanned { owner: Owner::Application, patch: 0 }
    );
    assert_eq!(plan::plan(&SETS, &levels(0, 0), &levels(2, 2)).expect("single owners").steps().len(), 4);
}
#[test]
fn a_header_must_read_and_name_its_owner_and_place() {
    for (sets, refusal) in [
        (PatchSets { sets: &[PatchSet { patches: &["patch ?\nowner fixture 1\n"], ..APPLICATION }, JIG] }, true),
        (PatchSets { sets: &[PatchSet { patches: &["patch 1\nowner other 1\n"], ..APPLICATION }, JIG] }, true),
        (SET_LEVEL_TWO, true),
        (SET_OWNER_OK, false),
    ] {
        let result = plan::plan(&sets, &levels(0, 0), &plan::binary(&sets));
        if refusal {
            assert_eq!(result.expect_err("bad header"), Unplanned { owner: Owner::Application, patch: 1 });
        } else {
            assert_eq!(result.expect("valid header").steps().len(), 3);
        }
    }
}
#[test]
fn after_cannot_pass_the_binary_or_precede_the_store() {
    for (sets, found_jig, expected) in [
        (SET_UPSTREAM_THREE, 0, Err(Unplanned { owner: Owner::Application, patch: 1 })),
        (SET_UPSTREAM_ONE, 0, Ok(())),
        (SET_UPSTREAM_ZERO, 1, Err(Unplanned { owner: Owner::Application, patch: 1 })),
        (SET_UPSTREAM_ONE, 1, Ok(())),
    ] {
        let result = plan::plan(&sets, &levels(0, found_jig), &plan::binary(&sets));
        assert_eq!(
            match result {
                Ok(_) => Ok(()),
                Err(error) => Err(error),
            },
            expected
        );
    }
}

fn made(found: Levels) -> Machines {
    let plan = plan::plan(&SETS, &found, &plan::binary(&SETS)).expect("fixture plan");
    machines::make(&SETS, &plan, &found, ROOMS).expect("fixture machines")
}
fn shelf_ix(machines: &Machines) -> FamilyIx {
    machines.find(0, 1).expect("shelf machine")
}
fn label_ix(machines: &Machines) -> FamilyIx {
    machines.find(0, 0).expect("label machine")
}

#[test]
fn an_unkept_text_is_refused_beside_a_valid_kept_text() {
    for (sets, expected) in [
        (
            PatchSets {
                sets: &[PatchSet { families: &[KeptFamily { texts: &["not a schema"], ..LABEL }], ..APPLICATION }, JIG],
            },
            Err(Unplanned { owner: Owner::Application, patch: 0 }),
        ),
        (SETS, Ok(())),
    ] {
        let found = levels(0, 0);
        let result = match plan::plan(&sets, &found, &plan::binary(&sets)) {
            Ok(plan) => match machines::make(&sets, &plan, &found, ROOMS) {
                Ok(_) => Ok(()),
                Err(error) => Err(error),
            },
            Err(error) => Err(error),
        };
        assert_eq!(result, expected);
    }
}
#[test]
fn a_patch_must_plan_its_family_changes() {
    const GOOD: &str = "patch 1\nowner fixture 1\nfamily fixture-label 1 to 2\nadd Label.color option u8 none\n";
    const BAD: &str = "patch 1\nowner fixture 1\nfamily fixture-label 1 to 2\nadd Label.color option u8 ?\n";
    for (sets, expected) in [
        (
            PatchSets { sets: &[PatchSet { families: &[LABEL], patches: &[BAD], ..APPLICATION }, JIG] },
            Err(Unplanned { owner: Owner::Application, patch: 1 }),
        ),
        (PatchSets { sets: &[PatchSet { families: &[LABEL], patches: &[GOOD], ..APPLICATION }, JIG] }, Ok(())),
    ] {
        let found = levels(0, 0);
        let result = match plan::plan(&sets, &found, &plan::binary(&sets)) {
            Ok(plan) => match machines::make(&sets, &plan, &found, ROOMS) {
                Ok(_) => Ok(()),
                Err(error) => Err(error),
            },
            Err(error) => Err(error),
        };
        assert_eq!(result, expected);
    }
}

pub(super) fn label(version: u16) -> Box<[u8]> {
    let mut out = Writer::new(if version == 1 { 7 } else { 8 });
    out.put(&version.to_be_bytes()).expect("version");
    out.put(&1_u32.to_be_bytes()).expect("caption length");
    out.put(b"a").expect("caption");
    if version == 2 {
        out.put(&[0]).expect("color none");
    }
    out.finish()
}
pub(super) fn token(value: &[u8], issued: u64) -> Box<[u8]> {
    let mut out = Writer::new(14_usize.checked_add(value.len()).expect("token length"));
    out.put(&1_u16.to_be_bytes()).expect("version");
    out.put(&u32::try_from(value.len()).expect("value length").to_be_bytes()).expect("length");
    out.put(value).expect("value");
    out.put(&issued.to_be_bytes()).expect("issued");
    out.finish()
}

#[test]
fn a_secrets_family_patches_in_its_own_maps_room() {
    let found = levels(1, 1);
    let plan = plan::plan(&SETS, &found, &plan::binary(&SETS)).expect("token and shelf plan");
    let input = token(&[7; 64], 91);
    assert_eq!(input.len(), 78);
    for (store_bytes, secrets_bytes, accepted) in [(64, 128, true), (128, 64, false)] {
        // The rooms come from each map's own value limit, as the open picks them.
        let mut limits = super::fixtures::store_limits();
        limits.store.value = store_bytes;
        limits.secrets.value = secrets_bytes;
        let mut machines = machines::make(&SETS, &plan, &found, Rooms::of(&limits)).expect("unequal map rooms");
        let family = machines.find(0, 2).expect("token machine");
        assert_eq!(machines.room(family).bytes, secrets_bytes);
        let result = walk::start(&mut machines, SHELF_STEP, family, "Token", input.clone());
        if accepted {
            let (walk, stop) = result.expect("token fits the secrets room");
            assert_eq!(stop, Stop::Done);
            let bytes = walk::finished(&mut machines, &walk).expect("patched token");
            let (_, value, root) = decoded(app::kept::fixture_token::TEXTS[1], "Token", bytes);
            assert_eq!(
                value
                    .content(value.field(root, app::kept::fixture_token::v2::token::VALUE).expect("value"))
                    .expect("bytes"),
                &[7; 64]
            );
            assert_eq!(
                value
                    .some(value.field(root, app::kept::fixture_token::v2::token::EXPIRES).expect("expires"))
                    .expect("option"),
                None
            );
        } else {
            assert_eq!(result.expect_err("token exceeds the secrets room"), Halt::Room);
        }
    }
}
pub(super) fn shelf(version: u16, books: u32, label: &[u8]) -> Box<[u8]> {
    let size = 15_usize.checked_add(label.len()).expect("fixture size");
    let size = if version == 2 { size.checked_add(4).expect("weight") } else { size };
    let mut out = Writer::new(size);
    out.put(&version.to_be_bytes()).expect("version");
    out.put(&1_u32.to_be_bytes()).expect("name length");
    out.put(b"a").expect("name");
    out.put(&books.to_be_bytes()).expect("books");
    out.put(&u32::try_from(label.len()).expect("held length").to_be_bytes()).expect("label length");
    out.put(label).expect("label");
    if version == 2 {
        out.put(&0_u32.to_be_bytes()).expect("weight");
    }
    out.finish()
}
pub(super) fn decoded(text: &str, root: &str, bytes: &[u8]) -> (Schema, Value, Node) {
    let schema = schema::parse(text.as_bytes()).expect("kept schema");
    let mut value = Value::with_room(ROOM.nodes);
    let root = schema.find(root.as_bytes()).expect("root");
    let node = value.decode(&schema, root, bytes::copy_of(bytes)).expect("bytes decode");
    (schema, value, node)
}
fn answer(machines: &mut Machines, walk: &mut walk::Walk, stop: Stop, key: &[u8]) -> Result<Stop, Halt> {
    let Stop::Site { family, site, at } = stop else {
        unreachable!("written shelf weight");
    };
    assert_eq!(site, Site::new(2, 0));
    let mut patcher = walk::take(machines, family);
    let mut context = app::Context { key: bytes::copy_of(key) };
    let (old, new) = patcher.parts();
    let result = app::written(site, old, new, at, &mut context);
    walk::give(machines, family, patcher);
    walk::resume(machines, walk, result)
}
#[test]
fn a_shelf_stops_at_its_written_weight_and_uses_the_record_key() {
    let mut machines = made(levels(0, 0));
    let family = shelf_ix(&machines);
    let input = shelf(1, 7, &label(1));
    drop(decoded(app::kept::fixture_shelf::TEXTS[0], "Shelf", &input));
    let (mut walk, stop) = walk::start(&mut machines, SHELF_STEP, family, "Shelf", input).expect("written stop");
    assert_eq!(answer(&mut machines, &mut walk, stop, b"s/42"), Ok(Stop::Done));
    let bytes = walk::finished(&mut machines, &walk).expect("whole shelf");
    let (_, value, root) = decoded(app::kept::fixture_shelf::TEXTS[1], "Shelf", bytes);
    assert_eq!(
        value.uint(value.field(root, app::kept::fixture_shelf::v2::shelf::WEIGHT).expect("weight")).expect("u32"),
        74
    );
}
#[test]
fn an_empty_shelf_refuses_with_its_declared_reason() {
    let mut machines = made(levels(0, 0));
    let family = shelf_ix(&machines);
    let (mut walk, stop) =
        walk::start(&mut machines, SHELF_STEP, family, "Shelf", shelf(1, 0, &label(1))).expect("site");
    assert_eq!(answer(&mut machines, &mut walk, stop, b"s"), Err(Halt::Refused(app::refusal::p0002::EMPTY)));
    assert_eq!(walk::finished(&mut machines, &walk), Err(Halt::Order));
}
#[test]
fn a_shelf_at_another_version_halts_before_its_site() {
    let mut machines = made(levels(0, 0));
    let family = shelf_ix(&machines);
    let result = walk::start(&mut machines, SHELF_STEP, family, "Shelf", shelf(2, 1, &label(1)));
    let Halt::Decode(_) = result.expect_err("wrong version") else {
        unreachable!("decoder refuses the version");
    };
}
#[test]
fn a_held_label_catches_up_to_the_reached_version() {
    let mut machines = made(levels(0, 0));
    machines.advance(LABEL_STEP);
    let family = shelf_ix(&machines);
    let (mut walk, stop) =
        walk::start(&mut machines, SHELF_STEP, family, "Shelf", shelf(1, 2, &label(1))).expect("site");
    assert_eq!(answer(&mut machines, &mut walk, stop, b"s"), Ok(Stop::Done));
    let (_, value, root) = decoded(
        app::kept::fixture_shelf::TEXTS[1],
        "Shelf",
        walk::finished(&mut machines, &walk).expect("whole shelf"),
    );
    let content = value
        .content(value.field(root, app::kept::fixture_shelf::v2::shelf::LABEL).expect("held label"))
        .expect("bytes");
    let (_, value, root) = decoded(app::kept::fixture_label::TEXTS[1], "Label", content);
    assert_eq!(
        value.some(value.field(root, app::kept::fixture_label::v2::label::COLOR).expect("color")).expect("option"),
        None
    );
}
#[test]
fn a_held_label_already_reached_is_carried_byte_for_byte() {
    let mut machines = made(levels(1, 0));
    let family = shelf_ix(&machines);
    let original = label(2);
    let (mut walk, stop) =
        walk::start(&mut machines, SHELF_STEP, family, "Shelf", shelf(1, 2, &original)).expect("site");
    assert_eq!(answer(&mut machines, &mut walk, stop, b"s"), Ok(Stop::Done));
    let (_, value, root) = decoded(
        app::kept::fixture_shelf::TEXTS[1],
        "Shelf",
        walk::finished(&mut machines, &walk).expect("whole shelf"),
    );
    assert_eq!(
        value
            .content(value.field(root, app::kept::fixture_shelf::v2::shelf::LABEL).expect("held label"))
            .expect("bytes"),
        original.as_ref()
    );
}
#[test]
fn advance_moves_only_the_step_owners_families_with_blocks() {
    let mut machines = made(levels(0, 0));
    let label = label_ix(&machines);
    let shelf = shelf_ix(&machines);
    machines.advance(JIG_STEP);
    assert_eq!(machines.get(label).reached, 1);
    machines.advance(LABEL_STEP);
    assert_eq!(machines.get(label).reached, 2);
    assert_eq!(machines.get(shelf).reached, 1);
    machines.advance(SHELF_STEP);
    assert_eq!(machines.get(shelf).reached, 2);
}
#[test]
fn the_machine_heap_bound_grows_with_record_room() {
    // This crate has no counting allocator. This checks the declared bound's
    // availability and room sensitivity, not a measurement of allocations.
    let bound = machines::worst_case_heap(&SETS, ROOMS).expect("fixture bound fits");
    let larger = machines::worst_case_heap(&SETS, Rooms { store: Room { bytes: 256, nodes: 32 }, ..ROOMS })
        .expect("larger bound fits");
    assert!(larger > bound, "record room contributes to the bound");
}

#[test]
fn the_plan_honours_dependencies_through_all_four_owners() {
    const SKEIN: PatchSet = PatchSet {
        fresh: None,
        owner: Owner::Skein,
        name: "first",
        families: &[KeptFamily { keys: &[], ..NOTE }],
        patches: &["patch 1\nowner first 1\n", "patch 1\nowner first 2\n"],
    };
    pub(super) const SMITH: PatchSet = PatchSet {
        fresh: None,
        owner: Owner::Smith,
        name: "second",
        families: &[KeptFamily { keys: &[], ..NOTE }],
        patches: &["patch 1\nowner second 1 after first 1\n"],
    };
    const THIRD: PatchSet = PatchSet {
        fresh: None,
        owner: Owner::Jig,
        name: "third",
        families: &[KeptFamily { keys: &[], ..NOTE }],
        patches: &["patch 1\nowner third 1 after second 1\n"],
    };
    const LAST: PatchSet = PatchSet {
        fresh: None,
        owner: Owner::Application,
        name: "last",
        families: &[KeptFamily { keys: &[], ..NOTE }],
        patches: &["patch 1\nowner last 1 after third 1\n"],
    };
    let sets = PatchSets { sets: &[SKEIN, LAST, SMITH, THIRD] };
    let found = Levels { skein: Some(0), smith: Some(0), jig: Some(0), application: Some(0) };
    assert_eq!(
        plan::plan(&sets, &found, &plan::binary(&sets)).expect("four-owner plan").steps(),
        &[
            Planned { owner: Owner::Skein, patch: 1 },
            Planned { owner: Owner::Smith, patch: 1 },
            JIG_STEP,
            LABEL_STEP,
            Planned { owner: Owner::Skein, patch: 2 },
        ]
    );
}
#[test]
fn an_identity_step_reaches_held_fields_without_moving_the_holder() {
    let mut machines = made(levels(0, 0));
    let family = shelf_ix(&machines);
    let (walk, stopped) =
        walk::start(&mut machines, LABEL_STEP, family, "Shelf", shelf(1, 2, &label(1))).expect("identity walk");
    assert_eq!(stopped, Stop::Done);
    let (_, value, root) = decoded(
        app::kept::fixture_shelf::TEXTS[0],
        "Shelf",
        walk::finished(&mut machines, &walk).expect("shelf unchanged"),
    );
    let held =
        value.content(value.field(root, app::kept::fixture_shelf::v1::shelf::LABEL).expect("label")).expect("bytes");
    assert_eq!(held, label(2).as_ref());
}
#[test]
fn a_written_held_site_names_its_own_machine_then_resumes_the_holder() {
    const WRITTEN: &str = "patch 1\nowner fixture 1 after fixture-jig 0\nfamily fixture-label 1 to 2\nadd Label.color option u8 written\nwritten Label.color\n";
    const TEST_SETS: PatchSets =
        PatchSets { sets: &[PatchSet { patches: &[WRITTEN, app::PATCHES[1]], ..APPLICATION }, JIG] };
    let found = levels(0, 0);
    let plan = plan::plan(&TEST_SETS, &found, &plan::binary(&TEST_SETS)).expect("plan");
    let mut machines = machines::make(&TEST_SETS, &plan, &found, ROOMS).expect("machines");
    machines.advance(LABEL_STEP);
    let family = shelf_ix(&machines);
    let (mut walk, stopped) =
        walk::start(&mut machines, SHELF_STEP, family, "Shelf", shelf(1, 2, &label(1))).expect("held site");
    let Stop::Site { family: held, site, .. } = stopped else {
        unreachable!("label site");
    };
    assert_eq!(held, label_ix(&machines));
    assert_eq!(site, Site::new(1, 0));
    let mut patcher = walk::take(&mut machines, held);
    let result = patcher.parts().1.put_none();
    walk::give(&mut machines, held, patcher);
    let stopped = walk::resume(&mut machines, &mut walk, result).expect("holder site");
    let Stop::Site { family: holder, .. } = stopped else {
        unreachable!("shelf site");
    };
    assert_eq!(holder, family);
    assert_eq!(answer(&mut machines, &mut walk, stopped, b"s"), Ok(Stop::Done));
}
#[test]
fn record_room_is_checked_on_both_sides_of_a_shelfs_node_count() {
    let found = levels(0, 0);
    let plan = plan::plan(&SETS, &found, &plan::binary(&SETS)).expect("plan");
    for (nodes, accepted) in [(3, false), (4, false), (5, true)] {
        // The old shelf has four nodes; the new one adds the weight node.
        let mut machines =
            machines::make(&SETS, &plan, &found, Rooms { store: Room { nodes, ..ROOM }, ..ROOMS }).expect("machines");
        let family = shelf_ix(&machines);
        let result = walk::start(&mut machines, SHELF_STEP, family, "Shelf", shelf(1, 2, &label(1)));
        if accepted {
            let (mut walk, stopped) = result.expect("record fits");
            assert_eq!(answer(&mut machines, &mut walk, stopped, b"s"), Ok(Stop::Done));
        } else if let Ok((mut walk, stopped)) = result {
            assert_eq!(answer(&mut machines, &mut walk, stopped, b"s"), Err(Halt::Room));
        } else {
            assert_eq!(result.expect_err("old record exceeds nodes"), Halt::Decode(skein_codec::value::Refusal::Room));
        }
    }
}

#[test]
fn a_held_record_two_versions_behind_runs_each_pass() {
    const LABEL_ZERO: &str = "family fixture-label 0\nversioned record Label { caption: text 32 }\n";
    const ZERO_TO_ONE: &str = "patch 1\nowner fixture 1\nfamily fixture-label 0 to 1\n";
    const ONE_TO_TWO: &str = "patch 1\nowner fixture 2\nfamily fixture-label 1 to 2\nadd Label.color option u8 none\n";
    const SHELF_THREE: &str = "patch 1\nowner fixture 3 after fixture-jig 1\nfamily fixture-shelf 1 to 2\nadd Shelf.weight u32 written\nwritten Shelf.weight\n";
    const TEST_SETS: PatchSets = PatchSets {
        sets: &[
            PatchSet {
                families: &[
                    KeptFamily {
                        texts: &[LABEL_ZERO, app::kept::fixture_label::TEXTS[0], app::kept::fixture_label::TEXTS[1]],
                        ..LABEL
                    },
                    SHELF,
                ],
                patches: &[ZERO_TO_ONE, ONE_TO_TWO, SHELF_THREE],
                ..APPLICATION
            },
            JIG,
        ],
    };
    let found = levels(2, 1);
    let plan = plan::plan(&TEST_SETS, &found, &plan::binary(&TEST_SETS)).expect("plan");
    let mut machines = machines::make(&TEST_SETS, &plan, &found, ROOMS).expect("machines");
    let family = shelf_ix(&machines);
    // Version zero's layout is the fixture's v1 layout, with its own prefix.
    let mut old_label = label(1);
    *old_label.get_mut(1).expect("version low byte") = 0;
    let step = Planned { owner: Owner::Application, patch: 3 };
    let (mut walk, stopped) =
        walk::start(&mut machines, step, family, "Shelf", shelf(1, 2, &old_label)).expect("shelf site");
    let Stop::Site { family: stopped_family, site, .. } = stopped else {
        unreachable!("shelf site after both held passes");
    };
    assert_eq!(stopped_family, family);
    assert_eq!(site, Site::new(3, 0));
    let mut patcher = walk::take(&mut machines, family);
    let result = patcher.parts().1.put_uint(23);
    walk::give(&mut machines, family, patcher);
    assert_eq!(walk::resume(&mut machines, &mut walk, result), Ok(Stop::Done));
    let (_, value, root) = decoded(
        app::kept::fixture_shelf::TEXTS[1],
        "Shelf",
        walk::finished(&mut machines, &walk).expect("whole shelf"),
    );
    let held = value
        .content(value.field(root, app::kept::fixture_shelf::v2::shelf::LABEL).expect("held label"))
        .expect("bytes");
    assert_eq!(held, label(2).as_ref());
}

#[test]
fn an_identity_walk_of_another_owners_holder_is_allowed() {
    const HOLDER: &str = "family holder 1\nversioned record Holder { note: bytes 64 holds fixture-note Note }\n";
    const HOLDER_SET: PatchSet = PatchSet {
        fresh: None,
        owner: Owner::Application,
        name: "holder",
        patches: &[],
        families: &[KeptFamily {
            flights: &[],
            frames: None,
            map: KeptMap::Store,
            name: "holder",
            texts: &[HOLDER],
            keys: &[],
        }],
    };
    const SECRET: &str = "family holder 1\nversioned record Holder { note: secret 64 holds fixture-note Note }\n";
    const SECRET_SET: PatchSet =
        PatchSet { families: &[KeptFamily { texts: &[SECRET], ..HOLDER_SET.families[0] }], ..HOLDER_SET };
    for holder in [PatchSets { sets: &[HOLDER_SET, JIG] }, PatchSets { sets: &[SECRET_SET, JIG] }] {
        let sets = holder;
        let found = Levels { skein: None, smith: None, jig: Some(0), application: Some(0) };
        let plan = plan::plan(&sets, &found, &plan::binary(&sets)).expect("plan");
        let mut machines = machines::make(&sets, &plan, &found, ROOMS).expect("machines");
        let mut input = Writer::new(13);
        input.put(&1_u16.to_be_bytes()).expect("holder version");
        input.put(&7_u32.to_be_bytes()).expect("held bytes");
        input.put(&1_u16.to_be_bytes()).expect("note version");
        input.put(&1_u32.to_be_bytes()).expect("note length");
        input.put(b"a").expect("note text");
        let (walk, stop) =
            walk::start(&mut machines, JIG_STEP, FamilyIx(0), "Holder", input.finish()).expect("identity");
        assert_eq!(stop, Stop::Done);
        assert_eq!(machines.walked(JIG_STEP).get(0), Some(&true));
        assert_eq!(skein_codec::version_of(walk::finished(&mut machines, &walk).expect("holder")), Ok(1));
    }
}
#[test]
fn two_families_holding_each_other_refuse_the_plan() {
    const A: &str = "family cycle-a 1\nversioned record A { b: bytes 64 holds cycle-b B }\n";
    const B1: &str = "family cycle-b 1\nversioned record B { a: bytes 64 holds cycle-a A }\n";
    const B2: &str = "family cycle-b 2\nversioned record B { a: bytes 64 holds cycle-a A }\n";
    const PATCH: &str = "patch 1\nowner cycle 1\nfamily cycle-b 1 to 2\n";
    let sets = PatchSets {
        sets: &[PatchSet {
            fresh: None,
            owner: Owner::Application,
            name: "cycle",
            patches: &[PATCH],
            families: &[
                KeptFamily { flights: &[], frames: None, map: KeptMap::Store, name: "cycle-a", texts: &[A], keys: &[] },
                KeptFamily {
                    flights: &[],
                    frames: None,
                    map: KeptMap::Store,
                    name: "cycle-b",
                    texts: &[B1, B2],
                    keys: &[],
                },
            ],
        }],
    };
    let found = Levels { skein: None, smith: None, jig: None, application: Some(0) };
    assert_eq!(
        plan::plan(&sets, &found, &plan::binary(&sets)).expect_err("hold cycle"),
        Unplanned { owner: Owner::Application, patch: 0 }
    );
}

pub(super) const SMITH: PatchSet = PatchSet {
    owner: Owner::Smith,
    name: jig_test_patches_smith::NAME,
    fresh: Some(jig_test_patches_smith::FRESH),
    patches: jig_test_patches_smith::patches::PATCHES,
    families: &[KeptFamily {
        flights: &[],
        name: "fixture-turn",
        texts: jig_test_patches_smith::patches::kept::fixture_turn::TEXTS,
        map: KeptMap::Store,
        keys: &[],
        frames: Some(jig_test_patches_smith::ROOT),
    }],
};
#[test]
fn a_pending_patch_marked_by_its_sets_word_closes_windows() {
    let sets = PatchSets { sets: &[SMITH] };
    let found = Levels { smith: Some(0), skein: None, jig: None, application: None };
    assert!(plan::plan(&sets, &found, &plan::binary(&sets)).expect("marked plan").close());
}
#[test]
fn absent_or_other_fresh_words_leave_windows_open() {
    const NONE: PatchSets = PatchSets { sets: &[PatchSet { fresh: None, ..SMITH }] };
    const OTHER: PatchSets = PatchSets { sets: &[PatchSet { fresh: Some("another-mark"), ..SMITH }] };
    for sets in [NONE, OTHER] {
        let found = Levels { smith: Some(0), skein: None, jig: None, application: None };
        assert!(!plan::plan(&sets, &found, &plan::binary(&sets)).expect("unmarked plan").close());
    }
}
#[test]
fn an_applied_fresh_mark_does_not_close_for_later_patches() {
    const LATER: PatchSets = PatchSets {
        sets: &[PatchSet {
            patches: &[jig_test_patches_smith::patches::PATCHES[0], "patch 1\nowner fixture-smith 2\n"],
            ..SMITH
        }],
    };
    let found = Levels { smith: Some(1), skein: None, jig: None, application: None };
    let plan = plan::plan(&LATER, &found, &plan::binary(&LATER)).expect("later plan");
    assert_eq!(plan.steps(), &[Planned { owner: Owner::Smith, patch: 2 }]);
    assert!(!plan.close());
}

/// One fault per otherwise valid set, shared by both admission paths.
pub(super) const RANGE_FAULTS: &[(&str, PatchSets, Owner)] = &[
    (
        "empty range",
        PatchSets {
            sets: &[PatchSet {
                families: &[KeptFamily { keys: &[Keys { from: b"l", to: b"l", root: "Label" }], ..LABEL }],
                ..BASE.sets[0]
            }],
        },
        Owner::Application,
    ),
    (
        "inverted range",
        PatchSets {
            sets: &[PatchSet {
                families: &[KeptFamily { keys: &[Keys { from: b"m", to: b"l", root: "Label" }], ..LABEL }],
                ..BASE.sets[0]
            }],
        },
        Owner::Application,
    ),
    (
        "overlapping families",
        PatchSets {
            sets: &[PatchSet {
                families: &[LABEL, KeptFamily { keys: &[Keys { from: b"l/", to: b"m", root: "Shelf" }], ..SHELF }],
                ..BASE.sets[0]
            }],
        },
        Owner::Application,
    ),
    (
        "overlapping ranges",
        PatchSets {
            sets: &[PatchSet {
                families: &[KeptFamily {
                    keys: &[
                        Keys { from: b"l", to: b"m", root: "Label" },
                        Keys { from: b"l/", to: b"m", root: "Label" },
                    ],
                    ..LABEL
                }],
                ..BASE.sets[0]
            }],
        },
        Owner::Application,
    ),
    (
        "component format row",
        PatchSets {
            sets: &[PatchSet {
                families: &[KeptFamily { keys: &[Keys { from: &[0, 0], to: &[0, 1], root: "Note" }], ..NOTE }],
                ..BASE.sets[1]
            }],
        },
        Owner::Jig,
    ),
    (
        "component layouts row",
        PatchSets {
            sets: &[PatchSet {
                families: &[KeptFamily { keys: &[Keys { from: &[0, 3], to: &[0, 4], root: "Note" }], ..NOTE }],
                ..BASE.sets[1]
            }],
        },
        Owner::Jig,
    ),
    (
        "installation identifier",
        PatchSets {
            sets: &[PatchSet {
                families: &[KeptFamily { keys: &[Keys { from: &[1, 4, 2], to: &[1, 4, 4], root: "Note" }], ..NOTE }],
                ..BASE.sets[1]
            }],
        },
        Owner::Jig,
    ),
    (
        "jig outside its tag",
        PatchSets {
            sets: &[PatchSet {
                families: &[KeptFamily { keys: &[Keys { from: &[2, b'n'], to: &[2, b'o'], root: "Note" }], ..NOTE }],
                ..BASE.sets[1]
            }],
        },
        Owner::Jig,
    ),
    (
        "application secrets outside its tag",
        PatchSets {
            sets: &[PatchSet {
                families: &[KeptFamily { keys: &[Keys { from: &[0x80], to: &[0x81], root: "Token" }], ..TOKEN }],
                ..BASE.sets[0]
            }],
        },
        Owner::Application,
    ),
    (
        "root only in an older text",
        PatchSets {
            sets: &[PatchSet {
                families: &[KeptFamily {
                    texts: &[
                        "family fixture-label 1\nversioned record Gone { value: u8 }\n",
                        app::kept::fixture_label::TEXTS[1],
                    ],
                    keys: &[Keys { from: b"l", to: b"m", root: "Gone" }],
                    ..LABEL
                }],
                ..BASE.sets[0]
            }],
        },
        Owner::Application,
    ),
    (
        "missing declaration",
        PatchSets {
            sets: &[PatchSet {
                families: &[KeptFamily { keys: &[Keys { from: b"l", to: b"m", root: "Absent" }], ..LABEL }],
                ..BASE.sets[0]
            }],
        },
        Owner::Application,
    ),
    (
        "missing root before baseline still checks ranges",
        PatchSets {
            sets: &[PatchSet {
                families: &[KeptFamily { texts: &[], keys: &[Keys { from: b"l", to: b"l", root: "Absent" }], ..LABEL }],
                ..BASE.sets[0]
            }],
        },
        Owner::Application,
    ),
];

#[test]
fn invalid_ranges_refuse_the_set_at_patch_zero() {
    for &(label, sets, owner) in RANGE_FAULTS {
        assert_eq!(plan::bounds(&sets), Err(Unplanned { owner, patch: 0 }), "{label}");
    }
}

#[test]
fn disjoint_maps_and_adjacent_ranges_are_admitted() {
    const ADJACENT: PatchSets = PatchSets {
        sets: &[PatchSet {
            families: &[
                KeptFamily {
                    keys: &[
                        Keys { from: b"l", to: b"l/", root: "Label" },
                        Keys { from: b"l/", to: b"m", root: "Label" },
                    ],
                    ..LABEL
                },
                KeptFamily { keys: &[Keys { from: b"l", to: b"m", root: "Token" }], ..TOKEN },
            ],
            ..BASE.sets[0]
        }],
    };
    const BEFORE: PatchSets = PatchSets {
        sets: &[PatchSet {
            families: &[KeptFamily { texts: &[], keys: &[Keys { from: b"l", to: b"m", root: "Absent" }], ..LABEL }],
            ..BASE.sets[0]
        }],
    };
    const SECRETS: PatchSets = PatchSets {
        sets: &[
            super::baseline::SETS.sets[0],
            super::baseline::SETS.sets[1],
            crate::JIG,
            PatchSet { families: &[TOKEN], ..BASE.sets[0] },
        ],
    };
    for sets in [BASE, SETS, ADJACENT, BEFORE, SECRETS] {
        assert_eq!(plan::bounds(&sets), Ok(()));
    }
}
