//! Whole committed outcome admission (client/domain/contract.md, 4).
//! Scalar facts occupy their complete inline representation. The typed
//! proposal key is recursively priced through its owner's Payload impl.
use super::{Payload, sum};
use crate::{Bounds, Invalid};
use jig_client_domain::contract::{OwnerEscalationChoice, OwnerOutcome, OwnerProposalChoice};
use std::mem::size_of;

impl Payload for OwnerProposalChoice {
    fn heap(&self, _bounds: &Bounds) -> Option<u64> {
        match self {
            Self::Accepted | Self::Rejected | Self::Passed | Self::Withdrawn | Self::Stale => Some(0),
        }
    }
}
impl Payload for OwnerEscalationChoice {
    fn heap(&self, _bounds: &Bounds) -> Option<u64> {
        match self {
            Self::Released | Self::Rejected | Self::Passed => Some(0),
        }
    }
}
impl Payload for OwnerOutcome {
    fn heap(&self, bounds: &Bounds) -> Option<u64> {
        match self {
            Self::Created { .. }
            | Self::Compound { .. }
            | Self::AccountKeyKept { .. }
            | Self::AccountFinishing { .. }
            | Self::AccountMovedIn { .. }
            | Self::Placed { .. }
            | Self::Lifted { .. }
            | Self::AccountChanged => Some(0),
            Self::AccountProgramDeclared { name, .. } => {
                if !jig_client_domain::contract::valid_program_name(name) {
                    return None;
                }
                u64::try_from(name.len()).ok()
            }
            Self::AccountRemoved { confirmed, failed_hosts } | Self::AccountSignedOut { confirmed, failed_hosts } => {
                if failed_hosts.len() > 128 || *confirmed && !failed_hosts.is_empty() {
                    return None;
                }
                let mut bytes = u64::try_from(
                    failed_hosts.len().checked_mul(size_of::<jig_client_domain::contract::AccountHostFailure>())?,
                )
                .ok()?;
                for host in &**failed_hosts {
                    if host.name.len() > 4096 {
                        return None;
                    }
                    bytes = bytes.checked_add(u64::try_from(host.name.len()).ok()?)?;
                }
                Some(bytes)
            }
            Self::AccountSignInStarted { address, .. } => {
                if address.len() > 2048 {
                    return None;
                }
                u64::try_from(address.len()).ok()
            }
            Self::MarkedSeen { seen } => seen.heap(bounds),
            Self::NoteEdited { name } => name.heap(bounds),
            Self::ServiceMade { person } => person.heap(bounds),
            Self::Renamed { project }
            | Self::ProjectMade { project }
            | Self::Adopted { project }
            | Self::RolesSet { project }
            | Self::PolicyChanged { project } => project.heap(bounds),
            Self::Stopped { task }
            | Self::Cancelled { task }
            | Self::Released { task }
            | Self::PersonTaken { task }
            | Self::PersonHandedBack { task }
            | Self::PersonAnswered { task }
            | Self::Moved { task } => task.heap(bounds),
            Self::Proposed { proposal, to } => sum(&[proposal.heap(bounds), to.heap(bounds)]),
            Self::Withdrawn { proposal } => proposal.heap(bounds),
            Self::ProposalDecided { proposal, by, at, choice, repeated: _ } => {
                sum(&[proposal.heap(bounds), by.heap(bounds), at.heap(bounds), choice.heap(bounds)])
            }
            Self::Said { task, message } => sum(&[task.heap(bounds), message.heap(bounds).heap(bounds)]),
            Self::QuestionAnswered { task, question, message } => {
                sum(&[task.heap(bounds), question.heap(bounds), message.heap(bounds)])
            }
            Self::Amended { task } => sum(&[task.heap(bounds).heap(bounds)]),
            Self::PoolSet { project, person } => sum(&[project.heap(bounds), person.heap(bounds)]),
            Self::EscalationDecided { task, revision, by, at, choice } => {
                sum(&[task.heap(bounds), revision.heap(bounds), by.heap(bounds), at.heap(bounds), choice.heap(bounds)])
            }
        }
    }
}

/// Complete inline and recursively owned heap bytes of one committed outcome.
#[must_use]
pub fn owner_outcome_bytes(value: &OwnerOutcome, bounds: &Bounds) -> Option<u64> {
    u64::try_from(size_of::<OwnerOutcome>()).ok()?.checked_add(value.heap(bounds)?)
}

/// Admit the complete value against the supplied document-byte bound.
pub fn admit_owner_outcome(value: &OwnerOutcome, bounds: &Bounds) -> Result<(), Invalid> {
    match owner_outcome_bytes(value, bounds) {
        Some(bytes) if bytes <= bounds.document_bytes => Ok(()),
        Some(_) | None => Err(Invalid::Payload),
    }
}

#[cfg(test)]
mod tests;
