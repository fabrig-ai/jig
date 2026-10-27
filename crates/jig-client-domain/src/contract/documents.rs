//! Whole committed documents, made by the engine and owned by this client.
//! See client/domain/contract.md, 5, and client/domain/objects.md, 3–8.
//! List capacities and Text byte bounds are validated by the protocol before entry;
//! the working set keeps only what its own configured limits admit.
use super::{
    Actor, Amendment, Authority, Count, Decision, DocumentText, Ending, Executor, Holding, Lacking, NoteScope,
    ObjectKey, Part, Party, Phase, Policy, ProposalKey, Proposer, Refusal, Revision, TaskLine, Text, TurnName,
    TurnRead, WakePolicy,
};
use alloc::boxed::Box;
use skein_lib::{Duration, List, Wall};
/// Exact counts of work needing this party.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Needs {
    pub questions: u32,
    pub proposals: u32,
    pub escalations: u32,
    pub person_tasks: u32,
}

/// Funding and spend, in the engine amounts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Budget {
    pub total: u64,
    pub own: u64,
    pub ended: u64,
    pub reserved: u64,
    pub left: u64,
    pub funder: Actor,
}

/// A named task wherever a page mentions it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Chip {
    pub task: u64,
    pub title: DocumentText,
    pub executor: Executor,
    pub phase: Phase,
    /// Whole live phase facts; compact `phase` remains the document's filter value.
    pub details: Box<ChipPhase>,
}

/// Phase facts at every chip, without nesting chips or task documents.
/// Dependency numbers name other live chips in the bounded working set.
/// See client/domain/objects.md, 3, and contract.md, 5.1.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ChipPhase {
    /// Exact producer phase with its mandatory whole owner metadata.
    Owner {
        phase: super::OwnerChipPhase,
        details: Box<super::OwnerChipDetails>,
    },
    Active(super::ActiveRun),
    Waiting {
        dependencies: List<u64>,
        hold: Option<Part>,
    },
    Idle {
        wake: WakePolicy,
    },
    Starting,
    Running {
        attempt: u64,
        since: Wall,
        tool: Option<Text>,
    },
    WaitingForWords {
        attempt: u64,
        since: Wall,
    },
    BackingOff {
        at: Wall,
        attempt: u64,
        tries: u32,
        class: FailureClass,
        reason: Text,
    },
    Procedure {
        panel: Part,
    },
    Person {
        addressed: Recipient,
        taken: Option<TakenBy>,
        since: Option<Wall>,
    },
    Closing(Closing),
    Held {
        reason: HoldReason,
        since: Wall,
        revision: Revision,
        was: Option<super::HeldRuntime>,
    },
    Ended {
        how: Ending,
        at: Wall,
    },
}

/// The settings revisions an agent task's runs read their charter from
/// (client/domain/contract.md, 5.1; domain/engine.md, 17.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TaskSettings {
    /// The revision the task records.
    pub task: u32,
    /// Its live run's, only while that run was prepared under another.
    pub run: Option<u32>,
    /// The newest revision's number.
    pub newest: u32,
}

/// A complete task document with independently moving revisions.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Task {
    /// None for a procedure's or a person's task.
    pub settings: Option<TaskSettings>,
    pub label: u32,
    pub flags: super::TaskFlags,
    pub last_activity: Wall,
    pub waits_on: super::WaitsOn,
    pub chip: Chip,
    /// The actual goal this task serves, including itself when it is a goal.
    pub goal: Option<u64>,
    pub priority: Option<u32>,
    pub lineage_chips: List<Chip>,
    pub inputs: List<super::OwnerTaskLink>,
    pub project: u32,
    pub requester: super::OwnerRequester,
    pub lineage: List<u64>,
    pub spec: super::SpecificationDocument,
    pub spec_revision: Revision,
    /// The independent person-task revision, when supplied by its owner.
    pub person_revision: Option<Revision>,
    /// Exact current taker identity; a display name is not required.
    pub taken_by: Option<u64>,
    /// Supplied instruction text; absence supplies no effective text.
    pub instructions: Option<DocumentText>,
    pub wake: super::OwnerWakePolicy,
    /// Standing interests supplied separately from activation policy.
    pub subscriptions: super::Subscriptions,
    pub result_contract: super::TaskContractDocument,
    pub dependencies: List<super::OwnerTaskLink>,
    pub dependency_facts: List<super::Dependency>,
    pub references: List<super::OwnerTaskLink>,
    /// At least one task list was omitted; this does not identify which list.
    pub cut: bool,
    pub resources: List<super::TaskResource>,
    pub budget: super::OwnerBudget,
    pub authority: super::OwnerAuthorityDocument,
    pub hold: Option<super::TaskHold>,
    pub proposals: List<ProposalKey>,
    pub activity: u64,
    pub changed: Wall,
    pub acts: List<Offered>,
    pub result: Option<super::TaskResult>,
    pub delegates: List<LiveDelegate>,
    /// Separately supplied context; the task owner document supplies none.
    pub context: Option<Box<TaskContext>>,
    /// The task's LLM choice, when its executor runs a model.
    pub llm: Option<super::LlmChoice>,
    pub kept_account: Option<u32>,
    pub run_wait: Option<super::TaskRunWait>,
}

/// Whole separately supplied task context (client/domain/contract.md, 5.1).
/// Absence supplies no runtime, impact, spend, panel, slot or escalation facts.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TaskContext {
    pub escalation: Option<Escalation>,
    pub parts: List<Part>,
    pub closing: Option<Closing>,
    pub spend: List<SpendLine>,
    pub runtime: Runtime,
    pub impact: Impact,
    pub writer_slots: List<WriterSlot>,
}

/// What a task still waits for while closing.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Closing {
    pub delegates: u32,
    /// None for an unjoined owner row; Some(empty) means known no connector waits.
    pub connectors: Option<List<u16>>,
    pub stage: Option<super::ClosingStage>,
    /// Actual pending outcome, not an ending reconstructed from settlement facts.
    pub ending: Option<Ending>,
}

/// Spend attributed to a run or delegate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SpendLine {
    pub task: u64,
    pub attempt: Option<u64>,
    pub spent: u64,
}

/// Allocated offer slots for the closed act vocabulary (contract.md, 5.3).
/// Offers are independent of paged rows; their nested values keep their own bounds.
pub const MAX_OFFERED_ACTS: u32 = 25;

/// An engine offered act kind, without view words.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ActKind {
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
    /// Rename a project.
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

/// What the engine says would happen on an act.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Admission {
    /// It would be made.
    Allowed,
    /// It would be proposed to a holder.
    Proposed { to: Recipient },
    /// Concrete drafts compare with engine-supplied facts before sending.
    Bounded { within: Bounds, beyond: Beyond },
    /// It is not admitted and says why.
    Refused(Refusal),
}

/// Numbers and lists supplied by the engine, never policy inferred by the client.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Bounds {
    /// Creating choices within the party's offered authority.
    Create(super::CreateBounds),
    /// Additional budget and complete held grants for an amendment.
    Amend {
        budget_left: u64,
        grants: List<super::OwnerGrant>,
        /// Complete configured connector kind order; equality is implicit.
        implications: List<super::OwnerKindImplication>,
        /// Complete numbers and lists for directly made widening.
        authority: Box<super::OwnerAuthorityDocument>,
        /// Hard envelope of supported form choices.
        ceiling: Box<super::OwnerAuthorityDocument>,
    },
}

/// The engine's outcome when a concrete draft exceeds its supplied facts.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Beyond {
    /// It would be proposed to this holder.
    Proposed { to: Recipient },
    /// It would be refused with these facts.
    Refused(Refusal),
}

/// An act and its admission judged by the engine.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Offered {
    pub effect: Option<super::Effect>,
    pub act: ActKind,
    pub admission: Admission,
}

/// Where an object waits.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Recipient {
    /// At a task above.
    Task(u64),
    /// At a named party.
    Party(u64),
    /// At a policy role.
    Role { project: u32, role: u32 },
    /// Every policy role that may accept this kind of proposal, in supplied order.
    Roles { project: u32, roles: List<u32> },
}

/// A hold revision and the facts it decides.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Hold {
    pub placed: Option<super::PlacedHold>,
    pub revision: Revision,
    pub reason: HoldReason,
    pub before: Phase,
    pub since: Wall,
    pub result: Option<ResultValue>,
    /// Spend snapshot attributed to each run and delegate when held.
    pub spend: List<SpendLine>,
}

/// Why a task is held, with the facts its card needs.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum HoldReason {
    /// Out of tries, with failures.
    Tries { failures: List<AttemptFailure> },
    /// Stalled on named work.
    Stalled { since: Wall, waiting: Text },
    /// A dependency ended unsuccessfully.
    Dependency { task: u64, how: Ending },
    /// A connector reported drift.
    Drift { part: Part },
    /// A connector effect failed for good.
    EffectFailed { part: Part },
    /// An unrecoverable effect may have happened.
    EffectUncertain { part: Part },
    /// Out of budget.
    Budget { spent: u64, limit: u64 },
    /// Past a deadline.
    Deadline { at: Wall },
    /// Stopped by a party.
    Stopped { by: Party, at: Wall },
    /// Waited too long for a hold.
    Resource { resource: Part, holder: Actor },
    /// A pool lost allocation.
    Pool { part: Part },
    /// A procedure needs a decision.
    Procedure { part: Part },
}

/// One failed attempt and its failure class.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AttemptFailure {
    pub attempt: u64,
    pub class: FailureClass,
    pub reason: Text,
    pub at: Wall,
}

/// The failure class, as a fact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FailureClass {
    /// A host was lost.
    HostLost,
    /// A transient failure.
    Temporary,
    /// A permanent failure.
    Permanent,
    /// The run stalled.
    Stall,
}

/// A held task escalation and the hold it names.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Escalation {
    /// Its committed terminal decision when read after leaving the inbox.
    pub ending: Option<Ending>,
    pub task: Chip,
    pub hold: Hold,
    /// Bounded specification facts for amend-and-release from this card.
    pub steering: Box<Steering>,
    /// Whole cancellation impact, refreshed without changing the hold revision.
    pub impact: Impact,
    pub waiting: Recipient,
    pub trail: List<Passed>,
    pub acts: List<Offered>,
}

/// Bounded before facts for a task amendment, without nesting a task document.
/// The escalation and its held task name the same independent specification revision.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Steering {
    pub addressed: Option<Recipient>,
    /// Absent when retained owner facts do not supply the independent revision.
    pub person_revision: Option<Revision>,
    pub taken_by: Option<u64>,
    pub revision: Revision,
    pub spec: super::SpecificationDocument,
    /// Supplied context only; absent retained text is not a current default.
    pub instructions: Option<DocumentText>,
    pub wake: super::OwnerWakePolicy,
    pub dependencies: List<u64>,
    pub authority: super::OwnerAuthorityDocument,
    pub budget: Budget,
}

/// A holder an object passed and why.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Passed {
    pub by: Actor,
    pub at: Wall,
    pub reason: Text,
    pub lacking: Option<Lacking>,
}

/// An immutable proposal and its current waiting recipient.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Proposal {
    pub key: ProposalKey,
    pub project: u32,
    pub proposer: Proposer,
    pub why: Text,
    pub action: ProposalAction,
    pub needs: Authority,
    pub funding: Budget,
    pub waiting: Recipient,
    pub trail: List<Passed>,
    pub since: Wall,
    pub acts: List<Offered>,
    pub ending: Option<Ending>,
}

/// What acceptance would make or change.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ProposalAction {
    /// A bounded batch, optionally made the accepter's.
    Batch { tasks: List<ProposedTask>, own: bool },
    /// A connector effect.
    Effect { part: Part },
    /// Wider authority.
    Widen { before: Authority, after: Authority },
    /// An amendment beyond the amender authority.
    Amend { task: u64, before: Text, change: Amendment },
}

/// One task in a proposal batch.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ProposedTask {
    pub number: u64,
    pub spec: super::SpecificationDocument,
    pub executor: super::OwnerExecutor,
    pub contract: super::TaskContractDocument,
    pub authority: super::OwnerAuthorityDocument,
    pub deadline: super::NewDeadline,
    pub budget: u64,
    pub funder: super::Funder,
    pub dependencies: List<u64>,
    pub resources: List<super::OwnerPart>,
    pub wake: super::OwnerWakePolicy,
    pub priority: Option<u32>,
    pub cut: bool,
}

/// Words addressed by one task message.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Question {
    pub task: Chip,
    pub from: Option<super::OwnerRequester>,
    pub message: u64,
    pub words: Text,
    pub since: Wall,
    pub acts: List<Offered>,
    pub ending: Option<Ending>,
}

/// A numeric person identity with an optional supplied display name.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TakenBy {
    pub number: u64,
    pub name: Option<Text>,
}

/// A person-task document with its own revision and exact taker identity.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PersonTask {
    pub task: Chip,
    pub revision: Revision,
    pub requester: Actor,
    pub spec: super::SpecificationDocument,
    pub contract: PersonContract,
    pub inputs: List<Result>,
    pub addressed: Recipient,
    pub taken: Option<TakenBy>,
    pub since: Option<Wall>,
    pub stall: Option<Duration>,
    pub deadline: Option<Wall>,
    pub acts: List<Offered>,
    pub ending: Option<Ending>,
}

/// What a person task answer must say.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PersonContract {
    /// Choose from the named options.
    Choice { options: List<OptionLine> },
    /// Approve the named state.
    Approval { state: Text, part: Part },
    /// Answer in words.
    Words,
}

/// One option in a person task choice.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OptionLine {
    pub number: u32,
    pub words: Text,
    pub words_max: u32,
    pub followups: u32,
    pub fields: List<FieldRule>,
}

/// An opaque named field admitted by a verdict choice.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FieldRule {
    pub name: Box<[u8]>,
    pub max: u32,
}

/// An opaque named value supplied with a verdict.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ResultField {
    pub name: Box<[u8]>,
    pub value: Box<[u8]>,
}

/// The kind of result a task owes.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ResultContract {
    /// A report in words.
    Report,
    /// A verdict and its fields.
    Verdict { choices: List<OptionLine> },
    /// A connector result.
    Connector { connector: u16, kind: u16 },
    /// One choice.
    Choice { options: List<OptionLine> },
    /// A named state approval.
    Approval,
    /// Words as a result.
    Words,
}

/// The result payload, with connector values as parts.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ResultValue {
    /// A report.
    Report(DocumentText),
    /// Exact owner verdict payload. `ResultDetails` separately supplies complete
    /// followup/proposal references, including stored tasks without a chip.
    OwnerVerdict { verdict: u32, words: DocumentText, fields: List<ResultField> },
    /// A verdict and follow ups.
    Verdict {
        verdict: u32,
        words: DocumentText,
        fields: List<ResultField>,
        tasks: List<Chip>,
        proposal: Option<ProposalKey>,
    },
    /// A summary and connector result.
    Connector { summary: DocumentText, part: Part },
    /// Exact connector result kind and raw owner part, before any scoped application routing.
    Change { kind: u16, part: super::OwnerPart, words: DocumentText },
    /// A party choice.
    Choice { option: u32, by: Party, reason: DocumentText },
    /// Whole approved state and its optional explanation.
    Approved { state: Box<[u8]>, words: Option<DocumentText> },
    /// A task that could not complete its contract.
    Failure(DocumentText),
    /// An answer in words.
    Words(DocumentText),
}

/// A task ending and its result, as a card.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Result {
    pub task: Chip,
    pub ending: Ending,
    pub value: Option<ResultValue>,
    /// Completion wall when supplied; closing results may have no completion wall.
    pub at: Option<Wall>,
    /// Complete owner metadata when supplied; partial scripted values retain explicit absence.
    pub details: Option<Box<super::ResultDetails>>,
    pub acts: List<Offered>,
}

/// Counts of tasks folded beyond a tree window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PhaseCounts {
    /// Ended chips whose source does not supply a specific outcome.
    pub ended: u32,
    pub waiting: u32,
    pub running: u32,
    pub held: u32,
    pub closing: u32,
    pub done: u32,
    pub failed: u32,
    pub cancelled: u32,
}

/// One visible task and what is folded beneath it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TreeNode {
    pub chip: Chip,
    pub parent: Option<u64>,
    pub dependencies: List<u64>,
    pub proposals: List<ProposalKey>,
    pub spend: u64,
    pub folded: PhaseCounts,
    pub held_below: u32,
}

/// A live subtree within its watch window.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Tree {
    pub task: u64,
    pub nodes: List<TreeNode>,
    pub folded: PhaseCounts,
    pub held_below: u32,
}

/// A group of inbox entries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InboxGroup {
    /// Questions.
    Questions,
    /// Proposals.
    Proposals,
    /// Held tasks.
    Escalations,
    /// Tasks for parties.
    PersonTasks,
    /// Unread results.
    Results,
    /// Unread chat replies.
    Replies,
}

/// An entry names an object, project and read position.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct InboxEntry {
    /// Whole committed chat facts for a reply entry; absent for other groups.
    pub reply: Option<Box<TaskLine>>,
    pub lineage: List<Chip>,
    pub goal: Option<u64>,
    pub recipient: Option<Recipient>,
    pub stall: Option<Wall>,
    pub object: ObjectKey,
    pub project: u32,
    pub group: InboxGroup,
    pub position: u64,
    pub at: Wall,
}

/// The bounded inbox across projects and its counts.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Inbox {
    pub entries: List<InboxEntry>,
    pub needs: Needs,
    pub updates: Count,
    pub results_seen: u64,
    pub replies_seen: List<ReadPosition>,
    pub more: bool,
}

/// The read position for one conversation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ReadPosition {
    pub task: u64,
    pub through: u64,
}

/// A project, its policy, roles, spend and parts.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Project {
    pub period: u64,
    pub spend_lines: List<TaskSpend>,
    pub number: u32,
    pub name: Text,
    pub policy: Policy,
    pub roles: List<Holding>,
    pub spend: Budget,
    pub pools: List<Pool>,
    pub resources: List<Part>,
    pub services: List<Party>,
    pub recurring: List<Recurring>,
    pub parts: List<Part>,
    pub acts: List<Offered>,
}

/// A party pool in the current period.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Pool {
    pub party: u64,
    pub period: u64,
    pub budget: Budget,
}

/// Whole recurring template and retained period facts (pages.md, section 6).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Recurring {
    pub chip: Chip,
    pub key: u32,
    pub batch: List<ProposedTask>,
    pub budget: u64,
    pub overlap: super::RecurringOverlap,
    /// The current period supplied by the engine's funding owner.
    pub current_period: u64,
    /// When its retained ledger opened, if that ledger exists.
    pub began: Option<Wall>,
    /// The period source's next boundary, when supplied.
    pub next: Option<Wall>,
    pub last_period: u64,
    pub pending_period: Option<u64>,
    pub last_batch: List<u64>,
    pub cut: bool,
}

/// The stored writer of a note, without inferred identity or run facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NoteAuthor {
    /// A party made or corrected the note.
    Party { party: u64 },
    /// An identified task attempt wrote the note.
    Task { task: u64, attempt: u64 },
}

/// A note with its independently moving revision.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Note {
    pub number: u64,
    pub revision: Revision,
    pub scope: NoteScope,
    pub description: DocumentText,
    pub body: DocumentText,
    pub writer: NoteAuthor,
    pub tasks: List<u64>,
    pub changed: Option<Wall>,
    pub acts: List<Offered>,
    /// The supplied reference list is incomplete.
    pub cut: bool,
}

/// Hosts, accounts and deployment capacity.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Deployment {
    pub hosts: List<Host>,
    pub accounts: List<Account>,
    pub capacity: List<Capacity>,
    pub proposals: u32,
    pub refusals: List<Refusal>,
    pub parts: List<Part>,
}

/// Host connectivity, grace and run slots.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Host {
    pub number: u64,
    pub connected: bool,
    pub grace: Option<Wall>,
    pub used: u32,
    pub free: u32,
    pub runs: List<Chip>,
    pub refused: Option<Text>,
}

/// An LLM account availability and the work using it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Account {
    pub number: u32,
    pub usable: bool,
    pub reason: Option<Text>,
    pub runs: List<Chip>,
    pub waiting: List<Chip>,
}

/// Live task admission by project or deployment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Capacity {
    pub project: Option<u32>,
    pub live: u32,
    pub limit: u32,
}

/// One durable history change in commit order.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct HistoryLine {
    pub position: u64,
    pub task: u64,
    pub by: Actor,
    pub at: Wall,
    pub change: HistoryChange,
}

/// The change one history line describes.
#[cfg_attr(
    target_pointer_width = "64",
    expect(
        clippy::large_enum_variant,
        reason = "a whole inline hold includes its retained result and is priced recursively"
    )
)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum HistoryChange {
    /// Made in a batch.
    Made { batch: u64 },
    /// Specification changed.
    Amended { before: Text, after: Text },
    /// Held with its cause.
    Held(Hold),
    /// A hold lifted.
    Released,
    /// Cancelled with a reason.
    Cancelled { reason: Text },
    /// Requester changed.
    Moved { from: Actor, to: Actor, reason: Text },
}

/// One activity item in the order the engine committed it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ActivityLine {
    pub position: u64,
    pub at: Wall,
    pub event: Activity,
}

/// A task activity event, turns decoded below this domain.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Activity {
    /// Whole owner activity; its page carries the turn without a separate fetch.
    Owner { source: Box<super::OwnerActivityLine>, decoded: Option<ArchivedTurn> },
    /// Words as the party wrote them.
    Words { by: Party, message: u64, words: Text },
    /// A run began.
    RunBegan { attempt: u64, how: RunBeginning, why: Text },
    /// A run ended.
    RunEnded { attempt: u64, how: RunEnding, spent: u64 },
    /// A committed turn, read or undecodable.
    Turn { name: TurnName, turn: TurnRead },
    /// A result heard.
    Result { result: Result },
    /// News heard.
    News { topic: u16, summary: Text, part: Option<Part> },
    /// A recorded decision.
    Decision { object: ObjectKey, by: Actor, how: Decision },
}
/// Archived metadata and turn, carried by activity or fetched as one complete read.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ArchivedTurn {
    pub task: u64,
    pub name: TurnName,
    pub cumulative: u64,
    pub read: Option<u64>,
    pub at: Wall,
    pub turn: TurnRead,
}
impl Activity {
    /// A committed archive identity, independent of whether its bytes are available.
    #[must_use]
    pub fn committed_turn(&self) -> Option<TurnName> {
        match self {
            Self::Turn { name, .. } => Some(*name),
            Self::Owner { source, .. } => match source.line {
                super::OwnerActivity::Turn { attempt, turn, .. } => Some(TurnName { attempt, number: turn }),
                super::OwnerActivity::Words { .. }
                | super::OwnerActivity::Heard(_)
                | super::OwnerActivity::Notified { .. }
                | super::OwnerActivity::RunBegan { .. }
                | super::OwnerActivity::RunEnded { .. }
                | super::OwnerActivity::Decided { .. }
                | super::OwnerActivity::Kept { .. }
                | super::OwnerActivity::Milestone { .. } => None,
            },
            Self::Words { .. }
            | Self::RunBegan { .. }
            | Self::RunEnded { .. }
            | Self::Result { .. }
            | Self::News { .. }
            | Self::Decision { .. } => None,
        }
    }
    /// Actual decoded content, paired with its supplied committed identity.
    #[must_use]
    pub fn decoded_turn(&self) -> Option<(TurnName, &super::Turn)> {
        match self {
            Self::Turn { name, turn } => Some((*name, turn.decoded()?)),
            Self::Owner { decoded: Some(turn), .. } => Some((self.committed_turn()?, turn.turn.decoded()?)),
            Self::Owner { decoded: None, .. }
            | Self::Words { .. }
            | Self::RunBegan { .. }
            | Self::RunEnded { .. }
            | Self::Result { .. }
            | Self::News { .. }
            | Self::Decision { .. } => None,
        }
    }
    /// The immutable inbox message named by a words activity line.
    #[must_use]
    pub fn word_message(&self) -> Option<(u64, &Text)> {
        match self {
            Self::Words { message, words, .. } => Some((*message, words)),
            Self::Owner { source, .. } => match &source.line {
                super::OwnerActivity::Words { message, words, .. } => Some((*message, words)),
                super::OwnerActivity::Heard(_)
                | super::OwnerActivity::Notified { .. }
                | super::OwnerActivity::RunBegan { .. }
                | super::OwnerActivity::RunEnded { .. }
                | super::OwnerActivity::Turn { .. }
                | super::OwnerActivity::Decided { .. }
                | super::OwnerActivity::Kept { .. }
                | super::OwnerActivity::Milestone { .. } => None,
            },
            Self::Turn { .. }
            | Self::RunBegan { .. }
            | Self::RunEnded { .. }
            | Self::Result { .. }
            | Self::News { .. }
            | Self::Decision { .. } => None,
        }
    }
    /// Supplied committed read-through; missing and zero remain distinct.
    #[must_use]
    pub fn read_position(&self) -> Option<u64> {
        match self {
            Self::Turn { turn, .. } => turn.decoded()?.read_through,
            Self::Owner { source, .. } => match source.line {
                super::OwnerActivity::Turn { read, .. } => read,
                super::OwnerActivity::Words { .. }
                | super::OwnerActivity::Heard(_)
                | super::OwnerActivity::Notified { .. }
                | super::OwnerActivity::RunBegan { .. }
                | super::OwnerActivity::RunEnded { .. }
                | super::OwnerActivity::Decided { .. }
                | super::OwnerActivity::Kept { .. }
                | super::OwnerActivity::Milestone { .. } => None,
            },
            Self::Words { .. }
            | Self::RunBegan { .. }
            | Self::RunEnded { .. }
            | Self::Result { .. }
            | Self::News { .. }
            | Self::Decision { .. } => None,
        }
    }
}

/// How an agent run begins.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RunBeginning {
    /// Resumed its transcript.
    Resumed,
    /// Started from a brief with a conversation tail.
    Fresh { tail: u32 },
}

/// How a run ended.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RunEnding {
    /// Parked for later.
    Parked,
    /// Finished its task.
    Finished,
    /// Cancelled.
    Cancelled,
    /// Failed with a class and reason.
    Failed { class: FailureClass, reason: Text },
}

/// Facts about a task phase that a chip alone does not carry.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Runtime {
    /// Exact owner activation, including absent observation or actual start.
    Active(super::ActiveRun),
    /// Waiting on dependencies or a named resource hold.
    Waiting { dependencies: List<u64>, hold: Option<Part> },
    /// Idle and waiting on its wake policy.
    Idle,
    /// A run is due, preparing or claimed.
    Starting,
    /// A run is writing or calling a tool.
    Running { attempt: u64, since: Wall, tool: Option<Text> },
    /// A live run keeps its slot while waiting for the person.
    WaitingForWords { attempt: u64, since: Wall },
    /// A failed attempt waits until its retry time.
    BackingOff { at: Wall, attempt: u64, tries: u32, class: FailureClass, reason: Text },
    /// A connector procedure is active, shown in its panel.
    Procedure { panel: Part },
    /// A person task is waiting for its named recipient.
    Person { addressed: Recipient, since: Option<Wall> },
    /// Closing while its children and effects settle.
    Closing(Closing),
    /// Held, with separately retained prior lifecycle when supplied.
    Held { was: Option<super::HeldRuntime> },
    /// Ended, with its outcome in the task result.
    Ended,
}

/// What a steering confirmation says a cancel or close would affect.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Impact {
    pub tasks: PhaseCounts,
    pub live_runs: u32,
    pub proposals: u32,
}

/// A resource held for writing and the run holding its writer slot.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct WriterSlot {
    pub resource: Part,
    pub task: u64,
    pub attempt: Option<u64>,
}

/// A live chat delegate and the funding a takeover would reserve.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LiveDelegate {
    pub chip: Chip,
    pub takeover_budget: u64,
}

/// Bounded live project tasks in last-change order.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Tasks {
    pub rows: List<TaskLine>,
    pub more: bool,
}
/// Spend attributed to one goal or chat, largest first.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TaskSpend {
    pub chip: Chip,
    pub spent: u64,
}
/// Whole current or historical project-period spend facts.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SpendPeriod {
    pub period: u64,
    pub budget: Budget,
    pub pools: List<Pool>,
    pub tasks: List<TaskSpend>,
}
