//! Committed people request results (client/domain/contract.md, 4).
//! Keeps only the owner's supplied numbers, choices and clocks. Retry,
//! signed-out, refusal and connector answers are separate boundary routes.
use super::{ProposalKey, Revision, SeenThrough};
use alloc::boxed::Box;
use skein_lib::Wall;

/// The retained decision for a proposal, without an inferred explanation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OwnerProposalChoice {
    /// The proposal was accepted.
    Accepted,
    /// The proposal was rejected.
    Rejected,
    /// The proposal was passed to another holder.
    Passed,
    /// The proposal was withdrawn.
    Withdrawn,
    /// The proposal ceased to apply.
    Stale,
}

/// The retained decision for a held task's escalation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OwnerEscalationChoice {
    /// The task's hold was released.
    Released,
    /// The escalation was rejected and the task remains held.
    Rejected,
    /// The escalation was passed to another holder.
    Passed,
}

/// Facts returned by a committed jig request, including a historical winner.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum OwnerOutcome {
    /// The committed placed hold and its independent revision.
    Placed {
        hold: u64,
        revision: Revision,
    },
    Lifted {
        hold: u64,
        released: u32,
    },
    /// The sign-in took the address; its end shows on the accounts watch.
    AccountFinishing {
        source: u32,
    },
    /// Consecutive task numbers and any resolved `AsAllowed` budget.
    Created {
        first: u64,
        last: u64,
        given: Option<super::CreatedGiven>,
    },
    /// Operations committed together, taking consecutive message numbers.
    Compound {
        project: u32,
        operations: u32,
        first_message: u64,
    },
    /// A results or conversation-replies position was marked seen.
    MarkedSeen {
        seen: SeenThrough,
    },
    /// A note correction or deletion was committed.
    NoteEdited {
        name: u64,
    },
    /// A service party and its project role were made.
    ServiceMade {
        person: u64,
    },
    /// The owner committed a project's adoption.
    Adopted {
        project: u32,
    },
    /// A task was stopped for a later release or decision.
    Stopped {
        task: u64,
    },
    /// A task and its descendants were cancelled.
    Cancelled {
        task: u64,
    },
    /// A task's hold was released.
    Released {
        task: u64,
    },
    /// A role-addressed task was taken.
    PersonTaken {
        task: u64,
    },
    /// The claimant handed the task back to its role.
    PersonHandedBack {
        task: u64,
    },
    /// A person's task answer was admitted.
    PersonAnswered {
        task: u64,
    },
    /// A task and its remaining funding moved to the requesting person.
    Moved {
        task: u64,
    },
    /// A proposal's current or historical decision names its maker and winner.
    ProposalDecided {
        proposal: ProposalKey,
        by: u64,
        at: Wall,
        choice: OwnerProposalChoice,
        repeated: bool,
    },
    /// Words were admitted under a message number, with the supplied release fact.
    Said {
        task: u64,
        message: u64,
    },
    /// A named question was answered by a message.
    QuestionAnswered {
        task: u64,
        question: u64,
        message: u64,
    },
    /// A task accepted an amendment, with the supplied release fact.
    Amended {
        task: u64,
    },
    /// A release proposal waits for the placed hold owner.
    ReleaseProposed {
        proposal: u64,
    },
    Proposed {
        proposal: u64,
        to: u32,
    },
    /// A party withdrew its own proposal.
    Withdrawn {
        proposal: u64,
    },
    /// A project's roster and affected waiting recipients changed together.
    RolesSet {
        project: u32,
    },
    /// A project's policy changed for later checks.
    PolicyChanged {
        project: u32,
    },
    /// The project was renamed.
    Renamed {
        project: u32,
    },
    /// A project was made in the deployment.
    ProjectMade {
        project: u32,
    },
    /// A person's project pool was opened or resized.
    PoolSet {
        project: u32,
        person: u64,
    },
    /// A hold revision's current or historical decision names its winner.
    EscalationDecided {
        task: u64,
        revision: Revision,
        by: u64,
        at: Wall,
        choice: OwnerEscalationChoice,
    },
    /// An LLM account sign-in's address to open on the engine's machine.
    AccountKeyKept {
        source: u32,
    },
    AccountProgramDeclared {
        source: u32,
        name: Box<[u8]>,
    },
    AccountChanged,
    /// A moved account was adopted or renewed (domain/accounts.md, 6).
    AccountMovedIn {
        account: u32,
        renewed: bool,
    },
    AccountRemoved {
        confirmed: bool,
        failed_hosts: Box<[AccountHostFailure]>,
    },
    AccountSignInStarted {
        source: u32,
        address: Box<[u8]>,
    },
    /// Whether the issuer confirmed the end of the LLM account sign-in.
    AccountSignedOut {
        confirmed: bool,
        failed_hosts: Box<[AccountHostFailure]>,
    },
}

/// A configured worker that did not confirm the original operation.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct AccountHostFailure {
    pub name: Box<[u8]>,
    pub why: AccountHostWhy,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum AccountHostWhy {
    NotInContact,
    Lost,
    Failed,
}
