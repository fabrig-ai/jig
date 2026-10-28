//! Kept schemas, plans and reached versions (protocol/store.md 10.3).
use super::plan::{Levels, Plan, Planned, Unplanned};
use super::sets::{KeptMap, Keys, Owner, PatchSet, PatchSets};
use alloc::boxed::Box;
use skein_codec::patch::{Capacity, Patcher, PlanIx};
use skein_lib::List;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct FamilyIx(pub(crate) u32);
#[derive(Clone, Copy, Debug)]
pub(crate) struct Room {
    pub bytes: u32,
    pub nodes: u32,
}
/// Each map's record room, from that map's own limits: a family's
/// records are patched within the room of the map they live in (10.3).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Rooms {
    pub store: Room,
    pub secrets: Room,
}
impl Rooms {
    pub(crate) fn of(limits: &crate::Limits) -> Self {
        Self {
            store: Room { bytes: limits.store.value, nodes: limits.store.value },
            secrets: Room { bytes: limits.secrets.value, nodes: limits.secrets.value },
        }
    }
    pub(crate) fn of_map(self, map: KeptMap) -> Room {
        match map {
            KeptMap::Store => self.store,
            KeptMap::Secrets => self.secrets,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct Identity {
    pub version: u16,
    pub plan: PlanIx,
}
#[derive(Debug)]
pub(crate) struct Machine {
    pub owner: Owner,
    pub set: usize,
    pub family: usize,
    pub patcher: Option<Box<Patcher>>,
    pub reached: u16,
    pub plans: List<Option<PlanIx>>,
    pub identities: List<Identity>,
}
#[derive(Debug)]
pub(crate) struct Machines {
    families: List<Machine>,
    sets: PatchSets,
    rooms: Rooms,
}
impl Machines {
    pub(crate) fn keys(&self, family: FamilyIx) -> &'static [Keys] {
        let machine = self.get(family);
        self.sets
            .sets
            .get(machine.set)
            .expect("machine's set")
            .families
            .get(machine.family)
            .expect("machine's family")
            .keys
    }
    /// Reverse reachability of blocks through held fields at reached versions.
    pub(crate) fn walked(&self, step: Planned) -> List<bool> {
        let mut selected = List::with_capacity(self.len());
        for ix in 0..self.len() {
            selected.push(self.get(FamilyIx(ix)).block(step).is_some()).expect("one bit per family");
        }
        for _ in 0..self.len() {
            for ix in 0..self.len() {
                let machine = self.get(FamilyIx(ix));
                let schema = machine
                    .patcher
                    .as_ref()
                    .expect("between records")
                    .schema(machine.reached)
                    .expect("reached version kept");
                for decl in 0..schema.declarations() {
                    match schema.declaration(skein_codec::schema::DeclIx::new(decl)).shape {
                        skein_codec::schema::Shape::Record { first, count, .. } => {
                            for offset in 0..count {
                                let field = skein_codec::schema::FieldIx::new(
                                    first.index().checked_add(offset).expect("field index"),
                                );
                                let mut ty = schema.field(field).ty;
                                for _ in 0..schema.text().len() {
                                    use skein_codec::schema::{Held, Type};
                                    match schema.ty(ty) {
                                        Type::List { item, .. } | Type::Option(item) => ty = item,
                                        Type::Bytes { held: Held::Record { family, .. }, .. }
                                        | Type::Secret { held: Held::Record { family, .. }, .. } => {
                                            if let Some(held) = self.named(schema.spelled(family))
                                                && *selected.get(held.0).expect("held family's bit")
                                            {
                                                *selected.get_mut(ix).expect("family's bit") = true;
                                            }
                                            break;
                                        }
                                        Type::U8
                                        | Type::U16
                                        | Type::U32
                                        | Type::U64
                                        | Type::Bool
                                        | Type::Duration
                                        | Type::Fixed(_)
                                        | Type::Secret { held: Held::Opaque | Held::Pieces(_), .. }
                                        | Type::Text(_)
                                        | Type::Named(_)
                                        | Type::Bytes { held: Held::Opaque | Held::Pieces(_), .. } => break,
                                    }
                                }
                            }
                        }
                        skein_codec::schema::Shape::Enum { .. } => {}
                    }
                }
            }
        }
        selected
    }

    pub(crate) fn map(&self, family: FamilyIx) -> KeptMap {
        let machine = self.get(family);
        self.sets
            .sets
            .get(machine.set)
            .expect("machine's set")
            .families
            .get(machine.family)
            .expect("machine's family")
            .map
    }
    pub(crate) fn room(&self, family: FamilyIx) -> Room {
        self.rooms.of_map(self.map(family))
    }
    pub(crate) fn len(&self) -> u32 {
        self.families.len()
    }
    pub(crate) fn get(&self, family: FamilyIx) -> &Machine {
        self.families.get(family.0).expect("family belongs to these machines")
    }
    pub(crate) fn get_mut(&mut self, family: FamilyIx) -> &mut Machine {
        self.families.get_mut(family.0).expect("family belongs to these machines")
    }
    pub(crate) fn patcher(&mut self, family: FamilyIx) -> &mut Patcher {
        self.get_mut(family).patcher.as_mut().expect("patcher is in the component between sites")
    }
    #[cfg(test)]
    pub(crate) fn find(&self, set: usize, family: usize) -> Option<FamilyIx> {
        for (ix, machine) in self.families.iter().enumerate() {
            if machine.set == set && machine.family == family {
                return Some(FamilyIx(u32::try_from(ix).expect("bounded families")));
            }
        }
        None
    }
    pub(crate) fn named(&self, name: &[u8]) -> Option<FamilyIx> {
        for (ix, machine) in self.families.iter().enumerate() {
            let set = self.sets.sets.get(machine.set).expect("machine's set exists");
            let family = set.families.get(machine.family).expect("machine's family exists");
            if family.name.as_bytes() == name {
                return Some(FamilyIx(u32::try_from(ix).expect("bounded families")));
            }
        }
        None
    }
    pub(crate) fn advance(&mut self, step: Planned) {
        for ix in 0..self.families.len() {
            let machine = self.families.get_mut(ix).expect("bounded machine index");
            if machine.owner == step.owner
                && let Some(plan) = machine.at(step.patch)
            {
                machine.reached = machine.patcher.as_ref().expect("walk ended before advance").versions(plan).to;
            }
        }
    }
}
impl Machine {
    pub(crate) fn block(&self, step: Planned) -> Option<PlanIx> {
        if self.owner == step.owner { self.at(step.patch) } else { None }
    }
    pub(crate) fn after(&self, step: Planned) -> u16 {
        match self.block(step) {
            Some(plan) => self.patcher.as_ref().expect("held machine in component").versions(plan).to,
            None => self.reached,
        }
    }

    pub(crate) fn at(&self, patch: u32) -> Option<PlanIx> {
        *self.plans.get(patch.checked_sub(1)?)?
    }
    pub(crate) fn identity(&self, version: u16) -> Option<PlanIx> {
        for identity in &self.identities {
            if identity.version == version {
                return Some(identity.plan);
            }
        }
        None
    }
}

fn count(sets: &PatchSets) -> Option<u32> {
    let mut count = 0_u32;
    for set in sets.sets {
        for family in set.families {
            if !family.texts.is_empty() {
                count = count.checked_add(1)?;
            }
        }
    }
    Some(count)
}
pub(crate) fn capacity(set: &PatchSet, family: usize, room: Room) -> Option<Capacity> {
    let kept = set.families.get(family)?;
    let versions = u32::try_from(kept.texts.len()).ok()?;
    let patches = u32::try_from(set.patches.len()).ok()?.checked_add(versions)?;
    let mut text = 0_usize;
    for kept in kept.texts {
        text = text.max(kept.len());
    }
    for patch in set.patches {
        text = text.max(patch.len());
    }
    // Patcher::new requires the language's ceiling; keep/plan name the
    // offending kept text or patch if the actual text exceeds it.
    let text = u32::try_from(text).ok()?.min(skein_codec::schema::MAX_TEXT);
    Some(Capacity { versions, patches, text, bytes: room.bytes, nodes: room.nodes })
}

pub(crate) fn make(sets: &PatchSets, plan: &Plan, found: &Levels, rooms: Rooms) -> Result<Machines, Unplanned> {
    if plan.steps().is_empty() {
        return Ok(Machines { families: List::with_capacity(0), sets: *sets, rooms });
    }
    let count = count(sets).expect("static family count fits u32");
    let mut families = List::with_capacity(count);
    // Keep upstream machines too, even when their owner has no pending step:
    // an older held record may still need their already-applied patches.
    for (set_ix, set) in sets.sets.iter().enumerate() {
        for (family_ix, family) in set.families.iter().enumerate() {
            if family.texts.is_empty() {
                continue;
            }
            let refusal = Unplanned { owner: set.owner, patch: 0 };
            let capacity = capacity(set, family_ix, rooms.of_map(family.map)).ok_or(refusal)?;
            let mut patcher = Box::new(Patcher::new(capacity));
            let mut identities = List::with_capacity(capacity.versions);
            let mut reached = None;
            for text in family.texts {
                let Ok(version) = patcher.keep(text.as_bytes()) else {
                    return Err(refusal);
                };
                let schema = patcher.schema(version).expect("just kept");
                if schema.spelled(schema.family()) != family.name.as_bytes() {
                    return Err(refusal);
                }
                if reached.is_none() {
                    reached = Some(version);
                }
                let Ok(identity) = patcher.identity(version) else {
                    return Err(refusal);
                };
                identities.push(Identity { version, plan: identity }).expect("one identity per kept version");
            }
            let mut reached = reached.expect("nonempty kept texts");
            let patch_count = u32::try_from(set.patches.len()).expect("static patch count fits u32");
            let mut plans = List::with_capacity(patch_count);
            for (ix, text) in set.patches.iter().enumerate() {
                let patch = u32::try_from(ix)
                    .expect("bounded patch index")
                    .checked_add(1)
                    .expect("static patch count fits u32");
                let refusal = Unplanned { owner: set.owner, patch };
                let Ok(planned) = patcher.plan(text.as_bytes()) else {
                    return Err(refusal);
                };
                if let Some(plan) = planned {
                    let versions = patcher.versions(plan);
                    // Every block moves the family's records from its previous
                    // block's result, even when other families changed between.
                    let previous = if let Some(previous) = last_plan(&plans) {
                        patcher.versions(previous).to
                    } else {
                        identities.get(0).expect("first kept version").version
                    };
                    if versions.from != previous {
                        return Err(refusal);
                    }
                    if patch <= found.of(set.owner).unwrap_or(0) {
                        reached = versions.to;
                    }
                }
                plans.push(planned).expect("one entry per owner patch");
            }
            families
                .push(Machine {
                    owner: set.owner,
                    set: set_ix,
                    family: family_ix,
                    patcher: Some(patcher),
                    reached,
                    plans,
                    identities,
                })
                .expect("one machine per nonempty kept family");
        }
    }
    Ok(Machines { families, sets: *sets, rooms })
}
fn last_plan(plans: &List<Option<PlanIx>>) -> Option<PlanIx> {
    let mut last = None;
    for plan in plans {
        if plan.is_some() {
            last = *plan;
        }
    }
    last
}

pub(crate) fn kept_patcher(set: &PatchSet, ix: usize, capacity: Capacity) -> Result<Box<Patcher>, Unplanned> {
    let refusal = Unplanned { owner: set.owner, patch: 0 };
    let family = set.families.get(ix).ok_or(refusal)?;
    let mut machine = Box::new(Patcher::new(capacity));
    let mut previous = None;
    for text in family.texts {
        let Ok(version) = machine.keep(text.as_bytes()) else { return Err(refusal) };
        if let Some(previous) = previous
            && Some(version) != u16::checked_add(previous, 1)
        {
            return Err(refusal);
        }
        let schema = machine.schema(version).expect("kept version");
        if schema.spelled(schema.family()) != family.name.as_bytes() {
            return Err(refusal);
        }
        previous = Some(version);
    }
    for text in set.patches {
        if machine.plan(text.as_bytes()).is_err() {
            return Err(refusal);
        }
    }
    for text in family.texts {
        let schema = skein_codec::schema::parse(text.as_bytes()).ok().ok_or(refusal)?;
        let version = schema.version();
        if Some(version) != previous && machine.plan_from(version).is_none() {
            return Err(refusal);
        }
    }
    Ok(machine)
}
