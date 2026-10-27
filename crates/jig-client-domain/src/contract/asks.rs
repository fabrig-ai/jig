//! Each visible act is one ask; client/domain/contract.md, 3.2 and 4.
//! Requests carry jig facts alone and their outcomes are durable.
//! Adoption uses the connector's application request (client/domain/pages.md, 6.5).
use super::{Actor, Authority, Decision, HeldRole, NoteScope, Part, Party, ProposalKey, Refusal, Revision, Text};
use alloc::boxed::Box;
use skein_lib::{Duration, List, Wall};

/// A person destination change names the independent person-task revision (domain/tasks.md, 7).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AddressChange {
    pub revision: Revision,
    pub address: super::PersonAddress,
}

/// Whether an amendment tracks its task, and its priority if so.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tracking {
    Tracked { priority: u32 },
    Untracked,
}

/// A task change whose spec revision the request names.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Amendment {
    /// Retarget a person task, with admission decided by its engine document.
    pub addressed: Option<AddressChange>,
    pub spec: Option<super::Specification>,
    pub wake: Option<super::OwnerWakePolicy>,
    /// Replaces the whole list: new names add dependencies, omitted names remove them (tasks.md, 7).
    pub dependencies: Option<List<u64>>,
    pub authority: Option<super::OwnerAuthority>,
    pub instructions: Option<Text>,
    pub tracked: Option<Tracking>,
    /// A new LLM choice for the task's next runs.
    pub llm: Option<super::LlmChoice>,
}

/// The kinds of news that wake an agent.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct WakePolicy {
    pub words: bool,
    pub results: bool,
    pub topics: List<u16>,
    pub subscriptions: List<Part>,
}

/// A person task answer matching its contract.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PersonAnswer {
    /// An option and reason.
    Choice { option: u32, reason: Text },
    /// An approval naming the state seen.
    Approved { state: Box<[u8]>, words: Option<Text> },
    /// An answer in words.
    Words(Text),
    /// A failure with a reason.
    Cannot { reason: Text },
}

/// The read position moved by an explicit act.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SeenThrough {
    /// Results seen through this position.
    Results { through: u64 },
    /// One conversation seen through this position.
    Conversation { task: u64, through: u64 },
}

/// A party and its project role.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Holding {
    pub party: u64,
    pub role: HeldRole,
}

/// The project policy as facts.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Policy {
    pub ceiling: Authority,
    pub period: u64,
    pub spend: u64,
    pub roles: List<RolePolicy>,
    pub requirements: List<Part>,
    pub deciders: List<Decider>,
}

/// What a project role may give, allot and decide.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RolePolicy {
    pub number: u16,
    pub authority: Authority,
    pub allotment: u64,
    pub decisions: List<u16>,
}

/// The policy role for a kind of proposal past a tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Decider {
    pub kind: u16,
    pub role: u16,
}

/// The target and decision of a `Decide`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum DecideOn {
    Proposal { proposal: ProposalKey, decision: Decision, running: Option<super::Running> },
    Escalation { task: u64, revision: Revision, decision: Decision },
}
/// The target and result of an `Answer`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum AnswerOn {
    Question { task: u64, message: u64, words: Text },
    Person { task: u64, revision: Revision, answer: PersonAnswer },
}
/// Every jig request, without keys owned by the root table.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Ask {
    ChangeSettings(super::ChangeSettings),
    MoveToNewest(super::MoveToNewest),
    /// Admit and create a batch together.
    Create {
        project: u32,
        tasks: List<super::NewTask>,
    },
    /// Submit a proposal in the party's own name.
    Propose {
        project: u32,
        action: super::ProposeAction,
        reason: Text,
    },
    /// Withdraw the party's pending proposal.
    Withdraw {
        project: u32,
        proposal: u64,
    },
    /// Add words to a task inbox.
    Message {
        project: u32,
        task: u64,
        words: Text,
    },
    /// Commit the operations together, or refuse at the first failure.
    Compound {
        project: u32,
        operations: List<super::Operation>,
    },
    /// Answer a named question or a person task revision.
    Answer {
        project: u32,
        on: AnswerOn,
    },
    /// Take a role-addressed person task.
    Take {
        project: u32,
        task: u64,
    },
    /// Return a person task to its role.
    HandBack {
        project: u32,
        task: u64,
    },
    /// Decide an immutable proposal or the hold revision seen.
    Decide {
        project: u32,
        on: DecideOn,
    },
    /// Release a hold, choosing whether to repeat an uncertain effect.
    Release {
        project: u32,
        task: u64,
        revision: Revision,
        again: Option<bool>,
    },
    /// Lift a placed hold, optionally releasing its tasks.
    Lift {
        hold: u64,
        revision: Revision,
        with_tasks: bool,
    },
    /// Amend a task.
    Amend {
        project: u32,
        task: u64,
        revision: Option<Revision>,
        change: Box<Amendment>,
        reason: Text,
    },
    /// Stop a run and hold its task.
    Stop {
        project: u32,
        task: u64,
    },
    /// Cancel a task and its subtree.
    Cancel {
        project: u32,
        task: u64,
        reason: Text,
        grace: Option<Duration>,
    },
    /// Become a task requester and fund it.
    Move {
        project: u32,
        task: u64,
        reason: Text,
    },
    /// Move a read position explicitly.
    Seen {
        through: SeenThrough,
    },
    /// Correct the note revision seen.
    EditNote {
        project: u32,
        note: u64,
        scope: NoteScope,
        revision: Revision,
        description: Text,
        body: Text,
        references: List<u64>,
    },
    /// Delete the note revision seen.
    DeleteNote {
        project: u32,
        note: u64,
        scope: NoteScope,
        revision: Revision,
    },
    /// Change a project policy.
    ChangePolicy {
        project: u32,
        edit: super::PolicyEdit,
    },
    /// Replace project roles.
    SetRoles {
        project: u32,
        holdings: List<Holding>,
    },
    /// Make a service with a project role.
    MakeService {
        project: u32,
        name: Text,
        role: HeldRole,
    },
    /// Rename a project as its owner (contract.md, 3.2).
    Rename {
        project: u32,
        name: Text,
    },
    /// Make a project in the deployment scope (contract.md, 7.5).
    MakeProject {
        name: Text,
        policy: u32,
    },
    /// Set a party pool preserving spend and reservations.
    SetPool {
        project: u32,
        party: u64,
        budget: u64,
    },
    /// Start an LLM account sign-in.
    SignIn {
        holder: AccountHolder,
        provider: u32,
    },
    /// Change the holder's LLM account. Key values travel in the envelope only.
    AccountChange {
        holder: AccountHolder,
        change: AccountChange,
    },
    /// End an LLM account sign-in for a source.
    SignOut {
        holder: AccountHolder,
        source: u32,
    },
}

/// The holder of an LLM account sign-in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AccountHolder {
    Own,
    Deployment,
}

/// The durable result of one ask.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Outcome {
    SettingsChanged(super::OutcomeSettingsChanged),
    MovedToNewest(super::OutcomeMovedToNewest),
    /// Whole committed owner facts, without inferred names or explanations.
    Owner(super::OwnerOutcome),
    /// A named thing was made.
    Made {
        what: Made,
    },
    /// The act became a proposal.
    Proposed {
        proposal: ProposalKey,
    },
    /// A decision was made.
    Decided {
        by: Actor,
        how: Decision,
        at: Wall,
    },
    /// Another party decided first.
    DecidedFirst {
        by: Party,
        how: Decision,
        at: Wall,
    },
    /// The requested change was made.
    Changed,
}

/// What an ask made, named for its client.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Made {
    /// Tasks accepted from a batch.
    Tasks { tasks: List<u64> },
    /// A service party.
    Service { party: u64 },
    /// A person task result.
    PersonResult { task: u64 },
    /// A note in its scope.
    Note { note: u64, scope: NoteScope },
    /// An account record delivered once (client/domain/client.md, 6).
    MovingOut { account: u32, number: u32, record: skein_lib::Secret },
}

/// The one terminal the request table hands to this domain.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Answer {
    /// A durable outcome.
    Done(Outcome),
    /// A durable refusal.
    Refused { refusal: Refusal, at: Option<u32> },
}

/// Holder changes to LLM accounts (domain/accounts.md, section 5).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum AccountChange {
    DeclareProgram {
        provider: u32,
        join: Option<u32>,
        name: Box<[u8]>,
    },
    CheckProgram {
        source: u32,
    },
    AddKey {
        provider: u32,
        join: Option<u32>,
        digest: [u8; 32],
    },
    ReplaceKey {
        source: u32,
        digest: [u8; 32],
    },
    RemoveKey {
        source: u32,
    },
    Resume {
        source: u32,
    },
    Remove {
        account: u32,
    },
    MoveAccount {
        account: u32,
        place: u32,
    },
    MoveSource {
        account: u32,
        source: u32,
        place: u32,
    },
    SetUses {
        account: u32,
        uses: Option<Box<[super::OwnerAccountUse]>>,
    },
    MoveOut {
        account: u32,
    },
    Written {
        number: u32,
    },
    MoveIn {
        digest: [u8; 32],
    },
    /// Finish the party's sign-in under `source`, carrying its browser's address as a key is (domain/accounts.md, section 5).
    FinishSignIn {
        source: u32,
        digest: [u8; 32],
    },
}
