//! Whole policy edit admission (client/domain/contract.md, 5.3).
//! Reuses exact authority pricing and retains every allocated list capacity.
use crate::{Bounds, payload::Payload};
use core::mem::size_of;
use jig_client_domain::contract::{PolicyEdit, PolicyEditRole, PolicyPermission, PolicyRequirement};

impl Payload for PolicyEditRole {
    fn heap(&self, bounds: &Bounds) -> Option<u64> {
        self.authority.heap(bounds)
    }
}
impl Payload for PolicyRequirement {
    fn heap(&self, bounds: &Bounds) -> Option<u64> {
        self.pattern.heap(bounds)
    }
}
impl Payload for PolicyPermission {
    fn heap(&self, _bounds: &Bounds) -> Option<u64> {
        Some(0)
    }
}
impl Payload for PolicyEdit {
    fn heap(&self, bounds: &Bounds) -> Option<u64> {
        match self {
            Self::Templates { overrides } => overrides.heap(bounds),
            Self::ProjectSpend { .. } | Self::RoleSpend { .. } => Some(0),
            Self::Ceiling { ceiling } => ceiling.heap(bounds),
            Self::Role(value) => value.heap(bounds),
            Self::Requirements { requirements } => requirements.heap(bounds),
            Self::Projections { grants } => crate::payload::authority(grants, bounds),
            Self::Permissions { mappings } => mappings.heap(bounds),
        }
    }
}
/// Whole payload size, or an explicit bound or checked arithmetic failure.
#[must_use]
pub fn bytes(value: &PolicyEdit, bounds: &Bounds) -> Option<u64> {
    value.heap(bounds)?.checked_add(u64::try_from(size_of::<PolicyEdit>()).ok()?)
}
/// Rejects the whole edit when any nested value exceeds its bound.
#[must_use]
pub fn fits(value: &PolicyEdit, bounds: &Bounds) -> bool {
    crate::payload::fits(value, bounds)
}

#[cfg(test)]
mod tests;
