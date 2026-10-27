//! Full chip metadata admission (client/domain/contract.md, 5.3).
//! Counts every reserved list element, optional envelope and nested raw byte.
use crate::{Bounds, payload::Payload};
use core::mem::size_of;
use jig_client_domain::contract::{
    OwnerChipDetails, OwnerChipEnding, OwnerChipHold, OwnerChipObservation, OwnerChipWork, OwnerPersonWait,
    OwnerResourceLine,
};

impl Payload for OwnerChipDetails {
    fn heap(&self, bounds: &Bounds) -> Option<u64> {
        self.executor
            .heap(bounds)?
            .checked_add(self.wake.heap(bounds)?)?
            .checked_add(self.person_wait.heap(bounds)?)?
            .checked_add(self.hold.heap(bounds)?)?
            .checked_add(self.budget.heap(bounds)?)?
            .checked_add(self.dependencies.heap(bounds)?)?
            .checked_add(self.resources.heap(bounds)?)?
            .checked_add(self.procedure.heap(bounds)?)?
            .checked_add(self.closing_connectors.heap(bounds)?)?
            .checked_add(self.ending.heap(bounds)?)?
            .checked_add(self.observation.heap(bounds)?)
    }
}
impl Payload for OwnerResourceLine {
    fn heap(&self, bounds: &Bounds) -> Option<u64> {
        self.part.heap(bounds)
    }
}
impl Payload for OwnerPersonWait {
    fn heap(&self, _bounds: &Bounds) -> Option<u64> {
        Some(0)
    }
}
impl Payload for OwnerChipHold {
    fn heap(&self, bounds: &Bounds) -> Option<u64> {
        use jig_client_domain::contract::OwnerHoldEscalation;
        let escalation = match &self.escalation {
            Some(OwnerHoldEscalation::Rejected { reason, .. }) => reason.heap(bounds)?,
            Some(OwnerHoldEscalation::Routing | OwnerHoldEscalation::Waiting { .. }) | None => 0,
        };
        self.account
            .heap(bounds)?
            .checked_add(self.placed.heap(bounds)?)?
            .checked_add(self.words.heap(bounds)?)?
            .checked_add(self.circle.heap(bounds)?)?
            .checked_add(self.source.heap(bounds)?)?
            .checked_add(self.reason.heap(bounds)?)?
            .checked_add(escalation)
    }
}
impl Payload for OwnerChipEnding {
    fn heap(&self, bounds: &Bounds) -> Option<u64> {
        match self {
            Self::Done(value) => value.heap(bounds),
            Self::Failed(reason) => reason.heap(bounds),
            Self::Cancelled { reason, result, by, .. } => {
                reason.heap(bounds)?.checked_add(result.heap(bounds)?)?.checked_add(by.heap(bounds)?)
            }
        }
    }
}
impl Payload for OwnerChipObservation {
    fn heap(&self, bounds: &Bounds) -> Option<u64> {
        self.work.heap(bounds)
    }
}
impl Payload for OwnerChipWork {
    fn heap(&self, bounds: &Bounds) -> Option<u64> {
        match self {
            Self::Tool { call, tool } => call.heap(bounds)?.checked_add(tool.heap(bounds)?),
            Self::Preparing | Self::Starting | Self::Active | Self::Writing | Self::Waiting | Self::Ending => Some(0),
        }
    }
}
/// Complete inline owner value plus every nested allocation; refuses whole on overflow.
#[must_use]
pub fn bytes(value: &OwnerChipDetails, bounds: &Bounds) -> Option<u64> {
    u64::try_from(size_of::<OwnerChipDetails>()).ok()?.checked_add(value.heap(bounds)?)
}
/// Optional boxed carrier envelopes are separately counted for consumer admission.
#[must_use]
pub fn boxed_bytes(value: &Option<Box<OwnerChipDetails>>, bounds: &Bounds) -> Option<u64> {
    u64::try_from(size_of::<Option<Box<OwnerChipDetails>>>()).ok()?.checked_add(value.heap(bounds)?)
}
/// Admit the exact whole supplied metadata without trimming any nested field.
#[must_use]
pub fn fits(value: &OwnerChipDetails, bounds: &Bounds) -> bool {
    crate::payload::fits(value, bounds)
}
/// Admit an optional fixture envelope, distinguishing absent from mandatory owner metadata.
#[must_use]
pub fn boxed_fits(value: &Option<Box<OwnerChipDetails>>, bounds: &Bounds) -> bool {
    crate::payload::fits(value, bounds)
}

impl Payload for jig_client_domain::contract::Dependency {
    fn heap(&self, _bounds: &Bounds) -> Option<u64> {
        Some(0)
    }
}
impl Payload for jig_client_domain::contract::Circle {
    fn heap(&self, bounds: &Bounds) -> Option<u64> {
        self.tasks.heap(bounds)
    }
}
