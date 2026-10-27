//! A task's supplied hold facts remain distinct from an independently supplied
//! Hold document (client/domain/objects.md, 5 and 11).
use super::{Hold, HoldCategory, HoldReason, OwnerChipHold, Revision};
use alloc::boxed::Box;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TaskHold {
    Document(Box<Hold>),
    Owner(Box<OwnerChipHold>),
}
impl TaskHold {
    /// The independently named hold revision, never the task spec's revision.
    #[must_use]
    pub fn revision(&self) -> Revision {
        match self {
            Self::Document(value) => value.revision,
            Self::Owner(value) => Revision { value: value.revision },
        }
    }
    /// Only the supplied uncertain-effect hold requires a repeat decision.
    #[must_use]
    pub fn uncertain(&self) -> bool {
        match self {
            Self::Document(value) => match &value.reason {
                HoldReason::EffectUncertain { .. } => true,
                HoldReason::Tries { .. }
                | HoldReason::Stalled { .. }
                | HoldReason::Dependency { .. }
                | HoldReason::Drift { .. }
                | HoldReason::EffectFailed { .. }
                | HoldReason::Budget { .. }
                | HoldReason::Deadline { .. }
                | HoldReason::Stopped { .. }
                | HoldReason::Resource { .. }
                | HoldReason::Pool { .. }
                | HoldReason::Procedure { .. } => false,
            },
            Self::Owner(value) => match value.reason {
                HoldCategory::Uncertain { .. } => true,
                HoldCategory::Exceeded(_)
                | HoldCategory::Failures { .. }
                | HoldCategory::Stalled
                | HoldCategory::Dependency { .. }
                | HoldCategory::Stopped
                | HoldCategory::Placed { .. }
                | HoldCategory::Drift
                | HoldCategory::Effects
                | HoldCategory::EffectFailed
                | HoldCategory::Budget
                | HoldCategory::Deadline
                | HoldCategory::HoldsWaited
                | HoldCategory::ResourceUnavailable
                | HoldCategory::ResourceAccess
                | HoldCategory::PoolLost { .. }
                | HoldCategory::Circle
                | HoldCategory::Procedure => false,
            },
        }
    }
}
