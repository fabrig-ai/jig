//! Party acts judged from current engine facts (`client/domain/contract.md`,
//! sections 3.2 and 5.3; `domain/people.md`, section 5.1).
//! No state, drafts, keys or document cache is retained. A request and a
//! document use the same judgments. Draft-dependent acts carry numerical and
//! listed bounds; the client compares its form with these engine facts.

pub(crate) mod bounds;
mod effects;
pub use effects::{Effect, Impact, ReleaseEffect};
mod objects;
mod requests;
mod standing;
use crate::{Core, Limits, documents};
use alloc::boxed::Box;
use jig_domain_people as people;
pub(crate) use objects::mutable_phase;
pub(crate) use objects::{escalation, person, proposal, question, write, write_in};
pub(crate) use requests::request;
use skein_lib::{List, Wall};
pub(crate) use standing::role;

/// An act in the client's closed order, translated between vocabularies by the wire.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Act {
    /// Write to an agent.
    Message,
    /// Answer a question or person task.
    Answer,
    /// Take a role task.
    Take,
    /// Hand a task back.
    HandBack,
    /// Accept a proposal.
    Accept,
    /// Reject a proposal.
    Reject,
    /// Pass a decision up.
    PassUp,
    /// Release a held task.
    Release,
    /// Leave a task held.
    LeaveHeld,
    /// Amend a specification.
    Amend,
    /// Stop a live run.
    Stop,
    /// Cancel a subtree.
    Cancel,
    /// Become a requester.
    Move,
    /// Reorder tracked work in its project.
    Reorder,
    /// Mark updates read.
    Seen,
    /// Edit a note.
    EditNote,
    /// Delete a note.
    DeleteNote,
    /// Adopt a resource.
    Adopt,
    /// Change policy.
    ChangePolicy,
    /// Rename the project.
    Rename,
    /// Change roles.
    SetRoles,
    /// Make a service.
    MakeService,
    /// Set a pool.
    SetPool,
    /// Withdraw the party's own pending proposal.
    Withdraw,
}

/// What a current engine judgment says about an act.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Admission {
    /// The act is made directly.
    Allowed,
    /// The act becomes a proposal waiting at this holder.
    Proposed { to: Recipient },
    /// The act is refused with the same facts as its request.
    Refused(people::Refusal),
    /// The draft is compared with supplied engine facts before confirmation.
    Bounded { within: Bounds, beyond: Beyond },
}

/// Configured connector and permitted procedure code for the existing goal form.
/// This is an authority fact, not a connector's procedure schema or display name.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct CreateProcedure {
    pub connector: u16,
    pub code: u32,
}

/// Creating bounds and the judgement beyond the first (contract.md, 5.3).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Creating {
    pub within: Bounds,
    pub beyond: Beyond,
}

/// A draft's numerical and listed bounds, supplied by the engine.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Bounds {
    /// A goal's direct funding, executor and deadline choices.
    Create {
        authority: Box<documents::AuthorityLine>,
        pool_left: u64,
        charters: List<u32>,
        procedures: List<CreateProcedure>,
        latest: Option<Wall>,
        budget: u64,
        maximum_budget: u64,
        maximum_deadline: Option<Wall>,
    },
    /// Authority the party may add to the task's current authority.
    Amend {
        budget_left: u64,
        grants: List<documents::GrantLine>,
        /// Complete configured connector-kind relation; equality is implicit.
        implications: List<jig_domain_authority::Implication>,
        authority: Box<documents::AuthorityLine>,
        ceiling: Box<documents::AuthorityLine>,
    },
}

/// The result when a supported draft exceeds the direct bounds.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Beyond {
    /// The draft becomes a proposal for this holder.
    Proposed { to: Recipient },
    /// The draft is refused with these engine facts.
    Refused(people::Refusal),
}

/// A holder named by current routing policy.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Recipient {
    /// A task above.
    Task(u64),
    /// A named party.
    Party(u64),
    /// A role in a project.
    Role { project: u32, role: u32 },
    /// Every policy role eligible for this collective decision.
    Roles { project: u32, roles: List<u32> },
}

/// One act and its current engine judgment.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Offered {
    pub effect: Option<Effect>,
    pub act: Act,
    pub admission: Admission,
}

/// Judge an act using the same current facts as its authenticated request.
#[must_use]
pub fn admit(
    core: &Core,
    limits: &Limits,
    party: u64,
    act: Act,
    object: &documents::Subject,
    decision: Wall,
) -> Admission {
    objects::admit(core, limits, party, act, object, decision)
}

fn allowed(value: Result<(), people::Refusal>) -> Admission {
    match value {
        Ok(()) => Admission::Allowed,
        Err(why) => Admission::Refused(why),
    }
}

/// Judge a note edit with the scope supplied by its owner-loaded entry.
pub(crate) fn note(
    core: &Core,
    party: u64,
    project: u32,
    scope: &people::NoteScope,
    act: Act,
) -> Result<(), people::Refusal> {
    let held = core.people.role(party, project);
    role(core, project, held, act)?;
    if core.note_authorized(project, held, scope) {
        Ok(())
    } else {
        Err(people::Refusal::Authority { proposable: None, lacking: people::Lack::Shape })
    }
}
