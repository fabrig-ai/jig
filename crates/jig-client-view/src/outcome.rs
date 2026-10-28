//! Routing facts from retained outcomes (client/domain/contract.md, 4).
//! Classification never rewrites the whole durable answer or joins missing facts.
use jig_client::contract::{Made, Outcome, OwnerOutcome};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Route {
    Created { task: u64 },
    Words { task: u64, message: u64 },
    Proposed,
    Other,
}
pub(crate) fn route(outcome: &Outcome) -> Route {
    match outcome {
        Outcome::Owner(value) => match value {
            OwnerOutcome::Created { first, last, .. } => {
                if first == last {
                    Route::Created { task: *first }
                } else {
                    Route::Other
                }
            }
            OwnerOutcome::Said { task, message, .. } => Route::Words { task: *task, message: *message },
            OwnerOutcome::Proposed { .. } | OwnerOutcome::ReleaseProposed { .. } => Route::Proposed,
            OwnerOutcome::Withdrawn { .. }
            | OwnerOutcome::Compound { .. }
            | OwnerOutcome::MarkedSeen { .. }
            | OwnerOutcome::NoteEdited { .. }
            | OwnerOutcome::ServiceMade { .. }
            | OwnerOutcome::Adopted { .. }
            | OwnerOutcome::Stopped { .. }
            | OwnerOutcome::Cancelled { .. }
            | OwnerOutcome::Placed { .. }
            | OwnerOutcome::Lifted { .. }
            | OwnerOutcome::Released { .. }
            | OwnerOutcome::PersonTaken { .. }
            | OwnerOutcome::PersonHandedBack { .. }
            | OwnerOutcome::PersonAnswered { .. }
            | OwnerOutcome::Moved { .. }
            | OwnerOutcome::ProposalDecided { .. }
            | OwnerOutcome::QuestionAnswered { .. }
            | OwnerOutcome::Amended { .. }
            | OwnerOutcome::RolesSet { .. }
            | OwnerOutcome::PolicyChanged { .. }
            | OwnerOutcome::AccountProgramDeclared { .. }
            | OwnerOutcome::AccountKeyKept { .. }
            | OwnerOutcome::AccountFinishing { .. }
            | OwnerOutcome::AccountMovedIn { .. }
            | OwnerOutcome::AccountChanged
            | OwnerOutcome::AccountRemoved { .. }
            | OwnerOutcome::AccountSignInStarted { .. }
            | OwnerOutcome::AccountSignedOut { .. }
            | OwnerOutcome::Renamed { .. }
            | OwnerOutcome::ProjectMade { .. }
            | OwnerOutcome::PoolSet { .. }
            | OwnerOutcome::EscalationDecided { .. } => Route::Other,
        },
        Outcome::Made { what } => match what {
            Made::Tasks { tasks } => {
                if tasks.len() == 1 {
                    Route::Created { task: *tasks.get(0).expect("one created task") }
                } else {
                    Route::Other
                }
            }
            Made::MovingOut { .. } | Made::Service { .. } | Made::PersonResult { .. } | Made::Note { .. } => {
                Route::Other
            }
        },
        Outcome::Proposed { .. } => Route::Proposed,
        Outcome::SettingsChanged(_)
        | Outcome::MovedToNewest(_)
        | Outcome::Decided { .. }
        | Outcome::DecidedFirst { .. }
        | Outcome::Changed => Route::Other,
    }
}
