//! Content-free room-refusal facts for deployment capacity
//! (client/domain/pages.md, section 7; domain/people.md, section 5.1).
//! Permanent facts come from retained keyed answers; entrance pressure is
//! bounded live history, independent of diagnostics and replay decisions.

use crate::{Ask, LimitKind, Outcome, Refusal, RequestKey};
use skein_lib::Wall;

/// The refused operation's kind and existing object identity, without its payload.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum CapacityAction {
    Compound,
    Create,
    Application { connector: u16 },
    Note { name: u64 },
    Service,
    Adoption,
    Roles,
    Policy,
    Pool { person: u64 },
    Escalation { task: u64 },
    Proposal { proposer: crate::Proposer, number: u64 },
    Words { task: u64 },
    Question { task: u64, question: u64 },
    Amend { task: u64 },
    Move { task: u64 },
    TakePerson { task: u64 },
    HandBackPerson { task: u64 },
    AnswerPerson { task: u64 },
    Stop { task: u64 },
    Cancel { task: u64 },
    Release { task: u64 },
    MarkSeen { seen: crate::Seen },
    AccountSignIn,
    AccountSignOut,
    AccountChange,
}

/// Exact entrance pressure, or a retained permanent room-limit answer.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum CapacityCause {
    Requests { limit: u32 },
    Pending { limit: u32 },
    Waiters { limit: u32 },
    Limit { which: LimitKind },
}

/// One refusal at its actual decision wall time. The opaque request key identifies
/// the request; no words, provider identity, credential or reply right are copied.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct CapacityRefusal {
    pub key: RequestKey,
    pub project: Option<u32>,
    pub action: CapacityAction,
    pub at: Wall,
    pub cause: CapacityCause,
}

/// The refused operation's content-free action.
pub(crate) fn action(ask: &Ask, person: u64) -> CapacityAction {
    match ask {
        Ask::Compound { .. } => CapacityAction::Compound,
        Ask::Application { connector, .. } => CapacityAction::Application { connector: *connector },
        Ask::EditNote { name, .. } => CapacityAction::Note { name: *name },
        Ask::MakeService { .. } => CapacityAction::Service,
        Ask::Adopt { .. } => CapacityAction::Adoption,
        Ask::SetRoles { .. } => CapacityAction::Roles,
        Ask::ChangeSettings { .. }
        | Ask::MoveToNewest { .. }
        | Ask::ChangePolicy { .. }
        | Ask::Rename { .. }
        | Ask::MakeProject { .. } => CapacityAction::Policy,
        Ask::SetPool { person, .. } => CapacityAction::Pool { person: *person },
        Ask::Create { .. } | Ask::Propose { .. } => CapacityAction::Create,
        Ask::Withdraw { proposal, .. } => {
            CapacityAction::Proposal { proposer: crate::Proposer::Party(person), number: *proposal }
        }
        Ask::DecideEscalation { task, .. } => CapacityAction::Escalation { task: *task },
        Ask::DecideProposal { proposer, proposal, .. } => {
            CapacityAction::Proposal { proposer: *proposer, number: *proposal }
        }
        Ask::Say { task, .. } => CapacityAction::Words { task: *task },
        Ask::AnswerQuestion { task, question, .. } => CapacityAction::Question { task: *task, question: *question },
        Ask::Amend { task, .. } => CapacityAction::Amend { task: *task },
        Ask::Move { task, .. } => CapacityAction::Move { task: *task },
        Ask::TakePerson { task, .. } => CapacityAction::TakePerson { task: *task },
        Ask::HandBackPerson { task, .. } => CapacityAction::HandBackPerson { task: *task },
        Ask::AnswerPerson { task, .. } => CapacityAction::AnswerPerson { task: *task },
        Ask::Stop { task, .. } => CapacityAction::Stop { task: *task },
        Ask::Cancel { task, .. } => CapacityAction::Cancel { task: *task },
        Ask::Release { task, .. } => CapacityAction::Release { task: *task },
        Ask::MarkSeen { seen } => CapacityAction::MarkSeen { seen: *seen },
        Ask::AccountSignIn { .. } => CapacityAction::AccountSignIn,
        Ask::AccountSignOut { .. } => CapacityAction::AccountSignOut,
        Ask::AccountChange { .. } => CapacityAction::AccountChange,
    }
}

pub(crate) fn project(ask: &Ask) -> Option<u32> {
    match ask {
        Ask::Compound { project, .. }
        | Ask::EditNote { project, .. }
        | Ask::MakeService { project, .. }
        | Ask::Adopt { project, .. }
        | Ask::SetRoles { project, .. }
        | Ask::Rename { project, .. }
        | Ask::ChangePolicy { project, .. }
        | Ask::SetPool { project, .. }
        | Ask::DecideEscalation { project, .. }
        | Ask::DecideProposal { project, .. }
        | Ask::Say { project, .. }
        | Ask::AnswerQuestion { project, .. }
        | Ask::Amend { project, .. }
        | Ask::Move { project, .. }
        | Ask::TakePerson { project, .. }
        | Ask::HandBackPerson { project, .. }
        | Ask::AnswerPerson { project, .. }
        | Ask::Stop { project, .. }
        | Ask::Cancel { project, .. }
        | Ask::Release { project, .. }
        | Ask::Create { project, .. }
        | Ask::Propose { project, .. }
        | Ask::Withdraw { project, .. } => Some(*project),
        Ask::MakeProject { .. } | Ask::ChangeSettings { .. } | Ask::MoveToNewest { .. } => Some(crate::DEPLOYMENT),
        Ask::Application { scope, .. } => match scope {
            crate::ApplicationScope::Own => None,
            crate::ApplicationScope::Project(project) => Some(*project),
            crate::ApplicationScope::Deployment => Some(0),
        },
        Ask::MarkSeen { .. } | Ask::AccountSignIn { .. } | Ask::AccountSignOut { .. } | Ask::AccountChange { .. } => {
            None
        }
    }
}

pub(crate) fn permanent(outcome: &Outcome) -> Option<CapacityCause> {
    match outcome {
        Outcome::Refused(refusal) | Outcome::CompoundRefused { why: refusal, .. } => room(refusal),
        Outcome::Compound { .. }
        | Outcome::Applied { .. }
        | Outcome::Again(_)
        | Outcome::SignedOut
        | Outcome::NoteEdited { .. }
        | Outcome::ServiceMade { .. }
        | Outcome::Adopted { .. }
        | Outcome::Created { .. }
        | Outcome::Proposed { .. }
        | Outcome::Withdrawn { .. }
        | Outcome::Stopped { .. }
        | Outcome::Cancelled { .. }
        | Outcome::Released { .. }
        | Outcome::PersonTaken { .. }
        | Outcome::PersonHandedBack { .. }
        | Outcome::PersonAnswered { .. }
        | Outcome::Moved { .. }
        | Outcome::ProposalDecided { .. }
        | Outcome::Said { .. }
        | Outcome::QuestionAnswered { .. }
        | Outcome::Amended { .. }
        | Outcome::RolesSet { .. }
        | Outcome::PolicyChanged { .. }
        | Outcome::Renamed { .. }
        | Outcome::ProjectMade { .. }
        | Outcome::SettingsChanged { .. }
        | Outcome::MovedToNewest { .. }
        | Outcome::PoolSet { .. }
        | Outcome::EscalationDecided { .. }
        | Outcome::AccountSignInStarted { .. }
        | Outcome::AccountProgramDeclared { .. }
        | Outcome::AccountKeyKept { .. }
        | Outcome::AccountMovingOut { .. }
        | Outcome::AccountFinishing { .. }
        | Outcome::AccountMovedIn { .. }
        | Outcome::AccountChanged
        | Outcome::AccountRemoved { .. }
        | Outcome::AccountSignedOut { .. }
        | Outcome::MarkedSeen { .. } => None,
    }
}

fn room(refusal: &Refusal) -> Option<CapacityCause> {
    match refusal {
        Refusal::Limit { which } => match which {
            LimitKind::SettingsProposals
            | LimitKind::Projects
            | LimitKind::Goals
            | LimitKind::ProjectLiveTasks
            | LimitKind::LiveTasks
            | LimitKind::Tree
            | LimitKind::Delegates
            | LimitKind::LiveDelegates
            | LimitKind::Holds
            | LimitKind::InboxFull
            | LimitKind::Accounts
            | LimitKind::HolderAccounts
            | LimitKind::Sources
            | LimitKind::SignIns
            | LimitKind::Notes => Some(CapacityCause::Limit { which: *which }),
            LimitKind::Operations
            | LimitKind::Words
            | LimitKind::Identity
            | LimitKind::Roles
            | LimitKind::Arithmetic
            | LimitKind::Duplicate
            | LimitKind::Empty
            | LimitKind::Batch
            | LimitKind::Depth
            | LimitKind::Subscription
            | LimitKind::Dependencies
            | LimitKind::Cycle
            | LimitKind::Spec
            | LimitKind::Contract
            | LimitKind::Authority
            | LimitKind::Inputs
            | LimitKind::Attempts
            | LimitKind::Restore
            | LimitKind::Read
            | LimitKind::Turn
            | LimitKind::HoldKind
            | LimitKind::HoldTaken
            | LimitKind::Revision
            | LimitKind::Question
            | LimitKind::Grace
            | LimitKind::Priority
            | LimitKind::Policy
            | LimitKind::Resource
            | LimitKind::Service => None,
        },
        Refusal::Stale { .. }
        | Refusal::Full { .. }
        | Refusal::Circle { .. }
        | Refusal::Dependency { .. }
        | Refusal::Kept { .. }
        | Refusal::NotWellFormed { .. }
        | Refusal::Role { .. }
        | Refusal::Authority { .. }
        | Refusal::Moved { .. }
        | Refusal::Taken { .. }
        | Refusal::Ended { .. }
        | Refusal::NotWaiting
        | Refusal::NoFurther
        | Refusal::NameTaken
        | Refusal::Unknown
        | Refusal::Provider
        | Refusal::NotOffered
        | Refusal::NoEscalationHolder
        | Refusal::Llm { .. }
        | Refusal::SignInRedirect { .. }
        | Refusal::AccountMove { .. }
        | Refusal::Account { .. }
        | Refusal::KeyConflict => None,
    }
}
