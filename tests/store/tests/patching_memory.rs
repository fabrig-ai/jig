//! Open and refusal paths retain their phase observations without heap totals.
use jig_store_world::{
    World,
    patches::{BASE, Records, SETS, limits},
    world::Step,
};
use std::cell::RefCell;

struct Observing {
    steps: [u64; 2],
    patching: bool,
}
thread_local! {
    static OBSERVING: RefCell<Option<Observing>> = const { RefCell::new(None) };
}
fn observe(step: Step) {
    OBSERVING.with_borrow_mut(|slot| match step {
        Step::Construct => *slot = Some(Observing { steps: [0; 2], patching: false }),
        Step::Begin { patching, .. } => slot.as_mut().expect("constructed").patching = patching,
        Step::End { patching } => {
            let observing = slot.as_mut().expect("constructed");
            let phase = usize::from(observing.patching || patching);
            observing.steps[phase] += 1;
        }
    });
}
fn observed(
    items: &[skein_fake_machine::Item],
    limits: &jig_protocol_store::Limits,
    sets: jig_protocol_store::PatchSets,
    opens: bool,
) -> [u64; 2] {
    let mut world = World::unopened_observed(920, *limits, Some(observe));
    world.lay(items);
    world.patch_sets(sets);
    assert_eq!(world.open_switched().is_some(), opens, "observed open: {:?}", world.refusal());
    OBSERVING.with_borrow_mut(|slot| slot.take().expect("observed").steps)
}
#[test]
fn a_whole_patching_open_observes_both_phases() {
    let records = Records::story();
    let mut source = records.committed(920, BASE);
    let items = image(&mut source);
    let mut steps = [0; 2];
    for sets in [BASE, SETS] {
        for (index, count) in observed(&items, &limits(), sets, true).into_iter().enumerate() {
            steps[index] += count;
        }
    }
    assert!(steps.into_iter().all(|count| count > 0), "both phases ran: {steps:?}");
}
#[test]
fn the_fresh_closing_walk_observes_both_phases() {
    use jig_store_world::patches::{FRESH_AFTER, FRESH_WINDOWS, fresh_committed, fresh_limits};
    let mut source = fresh_committed(920, FRESH_WINDOWS, false);
    let steps = observed(&image(&mut source), &fresh_limits(), FRESH_AFTER, true);
    assert!(steps.into_iter().all(|count| count > 0), "both phases ran: {steps:?}");
}
fn image(world: &mut World) -> Vec<skein_fake_machine::Item> {
    world
        .contents()
        .into_iter()
        .map(|(path, bytes)| match bytes {
            Some(bytes) => skein_fake_machine::Item::file(&path, &bytes).mode(0o600),
            None => skein_fake_machine::Item::directory(&path).mode(0o700),
        })
        .collect()
}

#[test]
fn a_flight_refusal_is_observed_in_patch_planning() {
    use jig_store_world::patches::flights::{self, CHANGES_FLIGHT, NAMED};
    let mut source = flights::committed(NAMED + 1);
    let steps = observed(&image(&mut source), &limits(), CHANGES_FLIGHT, false);
    assert!(steps[1] > 0, "the refusal reached patch planning");
}
