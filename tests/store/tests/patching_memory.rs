use jig_protocol_store::sizes;
use jig_store_world::{
    World,
    patches::{BASE, Records, SETS, limits},
    world::Step,
};
use skein_heap::{Counting, Meter, Span};
use std::cell::RefCell;

#[global_allocator]
static HEAP: Counting = Counting;

// Span removes allocations made by the simulator, disk and shell between
// calls; incoming file buffers transfer back into the store's account.
// Outgoing allocations stay charged, conservatively, even after the driver
// frees them: this check cannot underestimate the store's retained heap.
struct Measuring {
    meter: Meter,
    outside: Option<Span>,
    net: i64,
    phases: [Phase; 2],
    patching: bool,
}
#[derive(Clone, Copy)]
struct Phase {
    peak: u64,
    bound: u64,
    steps: u64,
}

thread_local! {
    static PRICING: RefCell<(jig_protocol_store::Limits, jig_protocol_store::PatchSets)> = RefCell::new((limits(), SETS));
    static MEASURING: RefCell<Option<Measuring>> = const { RefCell::new(None) };
}

fn observe(step: Step) {
    MEASURING.with_borrow_mut(|slot| match step {
        Step::Construct => {
            let (limits, sets) = PRICING.with_borrow(|pricing| *pricing);
            let phases = [
                Phase { peak: 0, bound: sizes::serving_peak(&limits, &sets).expect("serving bound"), steps: 0 },
                Phase { peak: 0, bound: sizes::patching_peak(&limits, &sets).expect("patching bound"), steps: 0 },
            ];
            let measuring = Measuring { meter: Meter::new(), outside: None, net: 0, phases, patching: false };
            measuring.meter.start();
            *slot = Some(measuring);
        }
        Step::Begin { input, patching } => {
            let measuring = slot.as_mut().expect("constructed");
            measuring.patching = patching;
            measuring.net += measuring.outside.take().expect("between steps").end().net;
            measuring.net -= i64::try_from(input).expect("input heap fits");
            measuring.meter.start();
        }
        Step::End { patching } => {
            let measuring = slot.as_mut().expect("constructed");
            let measured = measuring.meter.end();
            // Planning starts when the store retains Pending; the directory's
            // Patching state lasts through the check. Check transition calls
            // under the patch term if either side owns that state. Construction,
            // the ordinary open before planning and the pending serving steps
            // use serving's term, including during the SETS open and switch.
            let patching = measuring.patching || patching;
            let phase = &mut measuring.phases[usize::from(patching)];
            let bound = u64::try_from(i64::try_from(phase.bound).expect("finite bound") + measuring.net)
                .expect("positive adjusted bound");
            let own = measuring.meter.check(
                measured,
                bound,
                &(if patching { "patching" } else { "serving" }, phase.steps, phase.bound),
            );
            let own = u64::try_from(i64::try_from(own).expect("finite heap") - measuring.net).expect("store held");
            phase.peak = phase.peak.max(own);
            phase.steps += 1;
            measuring.outside = Some(Span::start());
        }
    });
}

#[test]
fn a_whole_patching_open_stays_within_its_priced_peak() {
    let records = Records::story();
    let mut source = records.committed(920, BASE);
    let items = image(&mut source);
    let mut peaks = [0; 2];
    for sets in [BASE, SETS] {
        let phases = measured(&items, &limits(), sets, true);
        for (index, phase) in phases.into_iter().enumerate() {
            let name = if index == 0 { "serving" } else { "patching" };
            eprintln!("{name}: measured {} / priced {} bytes over {} steps", phase.peak, phase.bound, phase.steps);
            peaks[index] = peaks[index].max(phase.peak);
        }
    }
    assert!(peaks[1] > peaks[0], "patching peak must exceed serving: {peaks:?}");
}

fn measured(
    items: &[skein_fake_machine::Item],
    limits: &jig_protocol_store::Limits,
    sets: jig_protocol_store::PatchSets,
    opens: bool,
) -> [Phase; 2] {
    PRICING.with_borrow_mut(|pricing| *pricing = (*limits, sets));
    let mut world = World::unopened_observed(920, *limits, Some(observe));
    world.lay(items);
    world.patch_sets(sets);
    assert_eq!(world.open_switched().is_some(), opens, "measured open: {:?}", world.refusal());
    let phases = MEASURING.with_borrow(|slot| slot.as_ref().expect("measured").phases);
    MEASURING.with_borrow_mut(|slot| *slot = None);
    phases
}

#[test]
fn the_fresh_closing_walk_stays_within_its_priced_peak() {
    use jig_store_world::patches::{FRESH_AFTER, FRESH_WINDOWS, fresh_committed, fresh_limits};
    let mut source = fresh_committed(920, FRESH_WINDOWS, false);
    let items = image(&mut source);
    let phases = measured(&items, &fresh_limits(), FRESH_AFTER, true);
    for (name, phase) in ["fresh serving", "fresh patching"].into_iter().zip(phases) {
        assert!(phase.steps > 0);
        eprintln!("{name}: measured {} / priced {} bytes over {} steps", phase.peak, phase.bound, phase.steps);
    }
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
fn a_flight_refusal_stays_within_the_planning_peak() {
    use jig_store_world::patches::flights::{self, CHANGES_FLIGHT, NAMED};
    let mut source = flights::committed(NAMED + 1);
    let phases = measured(&image(&mut source), &limits(), CHANGES_FLIGHT, false);
    let phase = phases[1];
    assert!(phase.steps > 0, "the refusal is charged to patch planning");
    eprintln!("flight planning: measured {} / priced {} bytes over {} steps", phase.peak, phase.bound, phase.steps);
}
