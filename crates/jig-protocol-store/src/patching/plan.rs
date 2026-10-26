//! Order across owners, using four explicit stack frames (protocol/store.md 10.3).
use super::sets::{Owner, PatchSet, PatchSets};
use alloc::boxed::Box;
use skein_codec::patch::{Header, header};
use skein_lib::List;

pub(crate) const DOWNSTREAM: [Owner; 4] = [Owner::Application, Owner::Jig, Owner::Smith, Owner::Skein];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Levels {
    pub skein: Option<u32>,
    pub smith: Option<u32>,
    pub jig: Option<u32>,
    pub application: Option<u32>,
}
impl Levels {
    pub(crate) fn of(&self, owner: Owner) -> Option<u32> {
        match owner {
            Owner::Skein => self.skein,
            Owner::Smith => self.smith,
            Owner::Jig => self.jig,
            Owner::Application => self.application,
        }
    }
    pub(crate) fn set(&mut self, owner: Owner, level: Option<u32>) {
        match owner {
            Owner::Skein => self.skein = level,
            Owner::Smith => self.smith = level,
            Owner::Jig => self.jig = level,
            Owner::Application => self.application = level,
        }
    }
}
pub(crate) fn binary(sets: &PatchSets) -> Levels {
    let mut levels = Levels { skein: None, smith: None, jig: None, application: None };
    for set in sets.sets {
        let mut kept = false;
        for family in set.families {
            if !family.texts.is_empty() {
                kept = true;
            }
        }
        if kept {
            levels.set(set.owner, Some(u32::try_from(set.patches.len()).expect("static patch count fits u32")));
        }
    }
    levels
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Planned {
    pub owner: Owner,
    pub patch: u32,
}
#[derive(Debug)]
pub(crate) struct Plan {
    steps: List<Planned>,
    close: bool,
}
impl Plan {
    pub(crate) fn close(&self) -> bool {
        self.close
    }
    pub(crate) fn steps(&self) -> &[Planned] {
        self.steps.as_slice()
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Unplanned {
    pub owner: Owner,
    pub patch: u32,
}
#[derive(Clone, Copy, Debug)]
struct Frame {
    owner: Owner,
    target: u32,
}

#[expect(clippy::manual_find, reason = "step code uses no closures (programming-model.md 10.3)")]
pub(crate) fn set(sets: &PatchSets, owner: Owner) -> Option<&PatchSet> {
    for set in sets.sets {
        if set.owner == owner {
            return Some(set);
        }
    }
    None
}
pub(crate) fn text(set: &PatchSet, patch: u32) -> Result<&'static str, Unplanned> {
    let ix = patch.checked_sub(1).ok_or(Unplanned { owner: set.owner, patch })?;
    match set.patches.get(usize::try_from(ix).expect("u32 fits usize")) {
        Some(text) => Ok(text),
        None => Err(Unplanned { owner: set.owner, patch }),
    }
}
fn read(set: &PatchSet, patch: u32) -> Result<Header, Unplanned> {
    let text = text(set, patch)?.as_bytes();
    let Ok(head) = header(text) else {
        return Err(Unplanned { owner: set.owner, patch });
    };
    let span = head.owner();
    let start = usize::try_from(span.start).expect("text span fits usize");
    let end = usize::try_from(span.end).expect("text span fits usize");
    if text.get(start..end) != Some(set.name.as_bytes()) || head.level() != patch {
        return Err(Unplanned { owner: set.owner, patch });
    }
    Ok(head)
}
/// The manifest counts each kept family by its name (store.md 2, 10.3): the
/// set where the families pass its bounds, in number or a name's bytes, is
/// refused as patch 0. Judged before the plan and before a first start writes.
pub(crate) fn bounds(sets: &PatchSets) -> Result<(), Unplanned> {
    let ceilings = &crate::component_v3::CEILINGS;
    let name = usize::try_from(ceilings.count_family).expect("family name ceiling fits usize");
    let mut families = 0_u32;
    for (index, set) in sets.sets.iter().enumerate() {
        for previous in sets.sets.get(..index).expect("earlier sets") {
            if previous.owner == set.owner {
                return Err(Unplanned { owner: set.owner, patch: 0 });
            }
        }
        for family in set.families {
            families = families.checked_add(1).expect("the ceiling stops the count first");
            if families > ceilings.manifest_counts || family.name.len() > name {
                return Err(Unplanned { owner: set.owner, patch: 0 });
            }
        }
    }
    frames(sets)?;
    holds(sets)
}
fn validate(sets: &PatchSets, found: &Levels, wanted: &Levels) -> Result<u32, Unplanned> {
    bounds(sets)?;
    let mut pending = 0_u32;
    for owner in DOWNSTREAM {
        let mut seen = false;
        for set in sets.sets {
            if set.owner != owner {
                continue;
            }
            if seen {
                return Err(Unplanned { owner, patch: 0 });
            }
            seen = true;
            for (index, _) in set.patches.iter().enumerate() {
                let patch = u32::try_from(index)
                    .expect("static patch index fits u32")
                    .checked_add(1)
                    .expect("static patch count fits u32");
                let head = read(set, patch)?;
                // Applied patches no longer constrain the store's upstream levels.
                if patch <= found.of(owner).unwrap_or(0) {
                    continue;
                }
                let text = text(set, patch)?.as_bytes();
                for upstream in sets.sets {
                    if upstream.owner >= owner {
                        continue;
                    }
                    if let Some(level) = head.after(text, upstream.name.as_bytes())
                        && (Some(level) < found.of(upstream.owner) || Some(level) > wanted.of(upstream.owner))
                    {
                        return Err(Unplanned { owner, patch });
                    }
                }
            }
        }
        let count = wanted
            .of(owner)
            .unwrap_or(0)
            .checked_sub(found.of(owner).unwrap_or(0))
            .expect("caller refused Ahead and Unleveled");
        pending = pending.checked_add(count).expect("static pending patch count fits u32");
    }
    Ok(pending)
}

pub(crate) fn plan(sets: &PatchSets, found: &Levels, wanted: &Levels) -> Result<Plan, Unplanned> {
    let pending = validate(sets, found, wanted)?;
    let mut steps = List::with_capacity(pending);
    let mut close = false;
    let mut stack = List::<Frame>::with_capacity(4);
    let mut reached = *found;
    // An iteration emits one patch, pushes a frame, or pops one. Every pushed
    // dependency emits at least one of the pending patches. Four root frames
    // add eight pushes/pops: at most 3 * pending + 8 iterations in all.
    let turns =
        u64::from(pending).checked_mul(3).expect("u32 times three fits u64").checked_add(8).expect("bounded turns");
    for _ in 0..turns {
        if stack.is_empty() {
            for owner in DOWNSTREAM {
                if reached.of(owner) < wanted.of(owner) {
                    stack
                        .push(Frame { owner, target: wanted.of(owner).expect("pending owner has a level") })
                        .expect("one root frame");
                    break;
                }
            }
            if stack.is_empty() {
                return Ok(Plan { steps, close });
            }
        }
        let top = stack.len().checked_sub(1).expect("nonempty stack");
        let frame = *stack.get(top).expect("top frame");
        let current = reached.of(frame.owner).expect("pending owner is leveled");
        if current == frame.target {
            stack.truncate(top);
            continue;
        }
        let patch = current.checked_add(1).expect("pending level fits u32");
        let own = set(sets, frame.owner).ok_or(Unplanned { owner: frame.owner, patch: 0 })?;
        let head = read(own, patch)?;
        let text = text(own, patch)?.as_bytes();
        let mut dependency = None;
        for owner in DOWNSTREAM {
            if owner >= frame.owner {
                continue;
            }
            if let Some(upstream) = set(sets, owner)
                && let Some(target) = head.after(text, upstream.name.as_bytes())
            {
                let current = reached.of(owner).unwrap_or(0);
                if current > target {
                    return Err(Unplanned { owner: frame.owner, patch });
                }
                if current < target {
                    dependency = Some(Frame { owner, target });
                    break;
                }
            }
        }
        if let Some(dependency) = dependency {
            stack.push(dependency).expect("dependencies go strictly upstream: at most four frames");
        } else {
            if let Some(word) = own.fresh
                && head.marked(text, word.as_bytes())
            {
                close = true;
            }
            steps.push(Planned { owner: frame.owner, patch }).expect("one entry per pending patch");
            reached.set(frame.owner, Some(patch));
        }
    }
    unreachable!("the pending patches and four owners bound the plan stack's work")
}

/// The two transcript roots are owners' data, checked even at equal levels.
pub(crate) fn frames(sets: &PatchSets) -> Result<(), Unplanned> {
    let mut smith = None;
    for set in sets.sets {
        let refusal = Unplanned { owner: set.owner, patch: 0 };
        let mut seen = false;
        for family in set.families {
            if family.frames.is_none() {
                continue;
            }
            if seen || (set.owner != Owner::Jig && set.owner != Owner::Smith) {
                return Err(refusal);
            }
            seen = true;
            if set.owner == Owner::Smith {
                smith = Some(family.name.as_bytes());
            }
        }
    }
    for set in sets.sets {
        for (ix, family) in set.families.iter().enumerate() {
            let Some(name) = family.frames else { continue };
            if family.texts.is_empty() {
                continue;
            }
            let refusal = Unplanned { owner: set.owner, patch: 0 };
            // Metadata alone resolves a root's spelling back over renames.
            let empty = super::machines::Room { bytes: 0, nodes: 0 };
            let capacity = super::machines::capacity(set, ix, empty).ok_or(refusal)?;
            let machine = super::machines::kept_patcher(set, ix, capacity)?;
            let latest = family.texts.last().expect("nonempty kept texts");
            let Ok(latest) = skein_codec::schema::parse(latest.as_bytes()) else { return Err(refusal) };
            let last = latest.version();
            for text in family.texts {
                let Ok(schema) = skein_codec::schema::parse(text.as_bytes()) else { return Err(refusal) };
                let Ok(root) = super::walk::held_root(&machine, schema.version(), last, name.as_bytes()) else {
                    return Err(refusal);
                };
                let (first, count) = match schema.declaration(root).shape {
                    skein_codec::schema::Shape::Record { versioned: true, first, count } => (first, count),
                    skein_codec::schema::Shape::Record { versioned: false, .. }
                    | skein_codec::schema::Shape::Enum { .. } => return Err(refusal),
                };
                if set.owner != Owner::Jig {
                    continue;
                }
                let mut held = false;
                for offset in 0..count {
                    let field = skein_codec::schema::FieldIx::new(first.index().checked_add(offset).expect("field"));
                    if let Some(other) = held_family(&schema, schema.field(field).ty) {
                        if smith != Some(other) || held {
                            return Err(refusal);
                        }
                        held = true;
                    }
                }
                if !held {
                    return Err(refusal);
                }
            }
        }
    }
    Ok(())
}

fn held_family(schema: &skein_codec::schema::Schema, mut ty: skein_codec::schema::TypeIx) -> Option<&[u8]> {
    use skein_codec::schema::{Held, Type};
    for _ in 0..schema.text().len() {
        match schema.ty(ty) {
            Type::Option(item) | Type::List { item, .. } => ty = item,
            Type::Bytes { held: Held::Record { family, .. }, .. } => return Some(schema.spelled(family)),
            Type::U8
            | Type::U16
            | Type::U32
            | Type::U64
            | Type::Bool
            | Type::Duration
            | Type::Fixed(_)
            | Type::Secret(_)
            | Type::Text(_)
            | Type::Named(_)
            | Type::Bytes { held: Held::Opaque | Held::Pieces(_), .. } => return None,
        }
    }
    None
}

/// Incremental transitive closure. Adding i -> j closes a cycle exactly when
/// j already reaches i; each row is a manifest-bounded, 64-family bit set.
fn holds(sets: &PatchSets) -> Result<(), Unplanned> {
    use skein_codec::schema::{self, DeclIx, FieldIx, Shape};
    let mut families = List::with_capacity(crate::component_v3::CEILINGS.manifest_counts);
    for set in sets.sets {
        for family in set.families {
            families.push(family).expect("bounds checked the manifest count");
        }
    }
    let mut reach = List::with_capacity(families.len());
    for _ in &families {
        reach.push(0_u64).expect("one row per family");
    }
    let mut index = 0_u32;
    for set in sets.sets {
        let refusal = Unplanned { owner: set.owner, patch: 0 };
        for family in set.families {
            let own = 1_u64.checked_shl(index).expect("at most 64 families");
            for text in family.texts {
                let Ok(schema) = schema::parse(text.as_bytes()) else { return Err(refusal) };
                for decl in 0..schema.declarations() {
                    match schema.declaration(DeclIx::new(decl)).shape {
                        Shape::Enum { .. } => {}
                        Shape::Record { first, count, .. } => {
                            for offset in 0..count {
                                let field = FieldIx::new(first.index().checked_add(offset).expect("schema field"));
                                let Some(name) = held_family(&schema, schema.field(field).ty) else { continue };
                                for target in 0..families.len() {
                                    if families.get(target).expect("family").name.as_bytes() != name {
                                        continue;
                                    }
                                    let to = 1_u64.checked_shl(target).expect("at most 64 families");
                                    let reached = *reach.get(target).expect("target row");
                                    if index == target || reached & own != 0 {
                                        return Err(refusal);
                                    }
                                    let added = to | reached;
                                    for source in 0..families.len() {
                                        let row = reach.get_mut(source).expect("source row");
                                        if source == index || *row & own != 0 {
                                            *row |= added;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            index = index.checked_add(1).expect("manifest-bound families");
        }
    }
    Ok(())
}

/// The read-only part of planning, before cleanup or closing A (store.md
/// 10.3). Only a patch's own blocks select families; held-value walks do not.
pub(crate) fn flights(
    store: &mut crate::Store,
    plan: &Plan,
    machines: &super::machines::Machines,
) -> Result<(), crate::OpenRefusal> {
    let sets = store.patch_sets;
    for &step in plan.steps() {
        for ix in 0..machines.len() {
            let machine = machines.get(super::machines::FamilyIx(ix));
            if machine.block(step).is_none() {
                continue;
            }
            let family = sets
                .sets
                .get(machine.set)
                .expect("machine's set")
                .families
                .get(machine.family)
                .expect("machine's family");
            if family.flights.is_empty() {
                continue;
            }
            let FlightKeys { count, keys } =
                flight_keys(store, step.owner, family).ok_or(crate::OpenRefusal::Failed)?;
            if count != 0 {
                return Err(crate::OpenRefusal::InFlight { owner: step.owner, family: family.name, count, keys });
            }
        }
    }
    Ok(())
}

/// store.md 10.3 bounds the keys named in a refusal independently of the count.
const FLIGHT_KEYS: u32 = 64;
struct FlightKeys {
    count: u32,
    keys: Box<[Box<[u8]>]>,
}
fn flight_keys(store: &mut crate::Store, owner: Owner, family: &super::sets::KeptFamily) -> Option<FlightKeys> {
    use crate::{KeptMap, pages};
    let rows = match family.map {
        KeptMap::Store => store.stores.store.len(),
        KeptMap::Secrets => store.stores.secrets.len(),
    };
    let mut count = 0_u32;
    let mut keys = List::<Box<[u8]>>::with_capacity(FLIGHT_KEYS);
    for keys_range in family.flights {
        let range = pages::kept_interval(store, owner, family.map, keys_range)?;
        let mut cursor = None;
        // One row per page keeps the read's transient room independent of
        // the number named. Each nonterminal page must consume a record.
        for _ in 0..=rows {
            let page = pages::walk_map(store, family.map, core::slice::from_ref(&range), cursor.as_ref(), 1)?;
            if page.rows.is_empty() && page.next.is_some() {
                return None;
            }
            for row in page.rows {
                count = count.saturating_add(1);
                if keys.len() < FLIGHT_KEYS {
                    let key = if owner == Owner::Application {
                        match family.map {
                            KeptMap::Store => crate::keys::of_application(&row.row.key)?,
                            KeptMap::Secrets => crate::secrets::of_application(&row.row.key)?,
                        }
                    } else {
                        row.row.key.as_ref()
                    };
                    keys.push(skein_lib::bytes::copy_of(key)).expect("at most the first 64 keys");
                }
            }
            cursor = page.next;
            if cursor.is_none() {
                break;
            }
        }
        if cursor.is_some() {
            return None;
        }
    }
    Some(FlightKeys { count, keys: keys.into_boxed() })
}

/// Named keys coexist with a one-row memory page and its exclusive cursors.
/// This is an alternative to copying, walking and checking, not resident state.
pub(crate) fn flights_worst_case(limits: &crate::Limits, sets: &PatchSets) -> Option<u64> {
    let mut peak = 0_u64;
    for set in sets.sets {
        for family in set.families {
            if family.flights.is_empty() {
                continue;
            }
            let map = match family.map {
                super::sets::KeptMap::Store => &limits.store,
                super::sets::KeptMap::Secrets => &limits.secrets,
            };
            let key = u64::from(map.key).checked_add(1)?;
            let named = List::<Box<[u8]>>::worst_case(FLIGHT_KEYS)?
                .checked_add(u64::from(FLIGHT_KEYS).checked_mul(u64::from(map.key))?)?;
            // The caller's range and cursor (3 keys); walk_map's range,
            // last and row cursor (4); load/past and the lookahead row (5).
            // Exclusive cursors can be one byte longer than stored keys.
            let page = List::<crate::pages::WalkRow>::worst_case(1)?
                .checked_add(List::<skein_kv::Row>::worst_case(1)?)?
                .checked_add(u64::from(map.page.bytes).checked_mul(2)?)?
                .checked_add(key.checked_mul(12)?)?;
            peak = peak.max(named.checked_add(page)?);
        }
    }
    Some(peak)
}
