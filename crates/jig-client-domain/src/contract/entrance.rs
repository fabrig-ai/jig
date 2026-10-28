//! Checked application admission facts; client/domain/root.md, sections 4 and 6.
//! An application final answer can keep these facts without variable allocations.
use super::{CurrentHold, Decision, Ending, Lacking, Moved, Refusal};

/// A fixed-size refusal from the common entrance of an application request.
/// Rich object documents belong in jig's answers, whose owner prices their payload.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EntranceRefusal {
    refusal: Refusal,
}
impl EntranceRefusal {
    /// Admit only facts with no owned text, list or boxed document allocation.
    pub fn new(refusal: Refusal) -> Result<Self, Refusal> {
        if fixed(&refusal) { Ok(Self { refusal }) } else { Err(refusal) }
    }
    /// The exact facts to render on the application's request card.
    #[must_use]
    pub const fn refusal(&self) -> &Refusal {
        &self.refusal
    }
    /// Move the supplied facts without altering them.
    #[must_use]
    pub fn into_refusal(self) -> Refusal {
        self.refusal
    }
}
fn fixed(refusal: &Refusal) -> bool {
    match refusal {
        Refusal::Authority { lacking, .. } => match lacking {
            Lacking::Grant(_) => false,
            Lacking::Ceiling(_)
            | Lacking::Approver
            | Lacking::Budget { .. }
            | Lacking::GrantKind { .. }
            | Lacking::Funding
            | Lacking::Tool
            | Lacking::Scope
            | Lacking::HoldOwner { .. }
            | Lacking::Models
            | Lacking::Reference
            | Lacking::ResourceAccess
            | Lacking::ResourceUnavailable
            | Lacking::Shape
            | Lacking::Again { .. }
            | Lacking::Account
            | Lacking::Writer
            | Lacking::Required { .. }
            | Lacking::Failed { .. }
            | Lacking::Unguarded { .. }
            | Lacking::TaskKind { .. }
            | Lacking::Depth { .. }
            | Lacking::Deadline { .. }
            | Lacking::Tasks { .. } => true,
        },
        Refusal::Moved { what, .. } => match what {
            Moved::Hold { reason } => match reason {
                Some(CurrentHold::Document(_)) => false,
                None | Some(CurrentHold::Reason(_)) => true,
            },
            Moved::PlacedHold { .. } | Moved::Spec | Moved::PersonTask | Moved::Note | Moved::Waiting => true,
        },
        Refusal::Ended { how } => match how {
            Ending::Done | Ending::Deleted => true,
            Ending::Failed { reason } | Ending::Cancelled { reason, .. } => reason.is_none(),
            Ending::Withdrawn { .. } | Ending::Task(_) => false,
            Ending::Decided { party, how, .. } => party.is_none() && fixed_decision(how.as_ref()),
        },
        Refusal::Stale(_)
        | Refusal::Malformed { .. }
        | Refusal::Role { .. }
        | Refusal::Taken { .. }
        | Refusal::NotWaiting
        | Refusal::NoFurther
        | Refusal::Limit { .. }
        | Refusal::SignInRedirect { .. }
        | Refusal::AccountMove { .. }
        | Refusal::Provider
        | Refusal::Unknown
        | Refusal::NameTaken
        | Refusal::NotOffered
        | Refusal::KeyConflict
        | Refusal::NoEscalationHolder => true,
        Refusal::Full(_)
        | Refusal::Llm { .. }
        | Refusal::Account { .. }
        | Refusal::Circle { .. }
        | Refusal::Dependency { .. }
        | Refusal::Kept { .. } => false,
    }
}
fn fixed_decision(decision: Option<&Decision>) -> bool {
    match decision {
        None | Some(Decision::Accepted | Decision::PassedUp | Decision::Released { .. }) => true,
        Some(Decision::Rejected { .. } | Decision::LeftHeld { .. }) => false,
    }
}
