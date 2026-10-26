//! Resident frame machines, bounded independently of map records (store.md 8.6, 13).
use crate::patching::{
    machines::{self, Room},
    plan::Unplanned,
    walk::held_root,
};
use crate::{Limits, Owner, PatchSet, PatchSets, codec};
use alloc::boxed::Box;
use skein_codec::{
    patch::{Capacity, Patcher},
    schema::{self, DeclIx, FieldIx, Schema, Shape, Type, TypeIx, VariantIx},
};
use skein_lib::List;

#[derive(Debug)]
pub(crate) struct Machine {
    pub patcher: Option<Box<Patcher>>,
    pub root: &'static str,
    pub last: u16,
    pub capacity: Capacity,
}
#[derive(Debug)]
pub(crate) struct Machines {
    pub body: Option<Machine>,
    pub turn: Option<Machine>,
}
impl Machines {
    pub(crate) fn new() -> Self {
        Self { body: None, turn: None }
    }
}

pub(crate) fn make(sets: &PatchSets, limits: &Limits) -> Result<Machines, Unplanned> {
    let mut frames = Machines::new();
    for set in sets.sets {
        for (ix, family) in set.families.iter().enumerate() {
            let Some(root) = family.frames else { continue };
            if family.texts.is_empty() {
                continue;
            }
            let refusal = Unplanned { owner: set.owner, patch: 0 };
            let capacity = capacity(set, ix, limits).ok_or(refusal)?;
            let patcher = machines::kept_patcher(set, ix, capacity)?;
            let last = last(family.texts).ok_or(refusal)?;
            let machine = Machine { patcher: Some(patcher), root, last, capacity };
            match set.owner {
                Owner::Jig => frames.body = Some(machine),
                Owner::Smith => frames.turn = Some(machine),
                Owner::Skein | Owner::Application => return Err(refusal),
            }
        }
    }
    Ok(frames)
}
fn last(texts: &[&str]) -> Option<u16> {
    Some(schema::parse(texts.last()?.as_bytes()).ok()?.version())
}

pub(crate) fn capacity(set: &PatchSet, ix: usize, limits: &Limits) -> Option<Capacity> {
    let family = set.families.get(ix)?;
    let root = family.frames?;
    let codec = codec::limits(&limits.core)?;
    let bytes = match set.owner {
        Owner::Jig => u32::try_from(crate::core_v6::TurnRecord::worst_case_bytes(&codec.core)?).ok()?,
        Owner::Smith => codec.core.turn_record_transcript,
        Owner::Skein | Owner::Application => return None,
    };
    let mut capacity = machines::capacity(set, ix, Room { bytes, nodes: 0 })?;
    capacity.patches = u32::try_from(set.patches.len()).ok()?;
    // Either owner's nodes are the most its kept texts allow from the
    // frames' root, so every frame those texts allow is read (store.md 8.6).
    let meta = machines::kept_patcher(set, ix, Capacity { bytes: 0, nodes: 0, ..capacity }).ok()?;
    let last = last(family.texts)?;
    for text in family.texts {
        let schema = schema::parse(text.as_bytes()).ok()?;
        let root = held_root(&meta, schema.version(), last, root.as_bytes()).ok()?;
        capacity.nodes = capacity.nodes.max(node_bound(&schema, root)?);
    }
    Some(capacity)
}

pub(crate) fn worst_case(limits: &Limits, sets: &PatchSets) -> Option<u64> {
    let mut bytes = 0_u64;
    for set in sets.sets {
        for (ix, family) in set.families.iter().enumerate() {
            if family.frames.is_none() || family.texts.is_empty() {
                continue;
            }
            let capacity = capacity(set, ix, limits)?;
            bytes = bytes
                .checked_add(Patcher::worst_case_heap(capacity)?)?
                .checked_add(u64::try_from(size_of::<Patcher>()).ok()?)?;
        }
    }
    Some(bytes)
}

/// Schemas name earlier declarations. Dynamic programming counts each named
/// subtree once; a held field is one leaf, with no traversal into its bytes.
pub(crate) fn node_bound(schema: &Schema, root: DeclIx) -> Option<u32> {
    let mut nodes = List::with_capacity(schema.declarations());
    for index in 0..schema.declarations() {
        let size = match schema.declaration(DeclIx::new(index)).shape {
            Shape::Record { first, count, .. } => {
                let mut size = 1_u32;
                for offset in 0..count {
                    let field = schema.field(FieldIx::new(first.index().checked_add(offset)?));
                    size = size.checked_add(type_nodes(schema, field.ty, &nodes)?)?;
                }
                size
            }
            Shape::Enum { first, count } => {
                let mut payload = 0;
                for offset in 0..count {
                    if let Some(decl) = schema.variant(VariantIx::new(first.index().checked_add(offset)?)).record {
                        payload = payload.max(*nodes.get(decl.index())?);
                    }
                }
                payload.checked_add(1)?
            }
        };
        nodes.push(size).ok()?;
    }
    nodes.get(root.index()).copied()
}
fn type_nodes(schema: &Schema, first: TypeIx, nodes: &List<u32>) -> Option<u32> {
    let mut end = first.index();
    for _ in 0..schema.text().len() {
        match schema.ty(TypeIx::new(end)) {
            Type::List { item, .. } | Type::Option(item) => end = item.index(),
            Type::U8
            | Type::U16
            | Type::U32
            | Type::U64
            | Type::Bool
            | Type::Duration
            | Type::Fixed(_)
            | Type::Secret { .. }
            | Type::Text(_)
            | Type::Named(_)
            | Type::Bytes { .. } => break,
        }
    }
    let mut count = 0_u32;
    for index in (first.index()..=end).rev() {
        count = match schema.ty(TypeIx::new(index)) {
            Type::Option(_) => count.checked_add(1)?,
            Type::List { bound, .. } => count.checked_mul(bound)?.checked_add(1)?,
            Type::Named(decl) => *nodes.get(decl.index())?,
            Type::U8
            | Type::U16
            | Type::U32
            | Type::U64
            | Type::Bool
            | Type::Duration
            | Type::Fixed(_)
            | Type::Secret { .. }
            | Type::Text(_)
            | Type::Bytes { .. } => 1,
        };
    }
    Some(count)
}
