//! Root-to-tasks semantic inputs and tasks-to-root persistence/lifecycle outputs
//! (domain/tasks.md, sections 2–5).
//! Inputs have no protocol bytes or connector events. Root authorizes them,
//! owns exact transport replay and commits related rows/effects atomically.
use crate::{Authority, Class, Funder, Numbers, Tries};
use alloc::boxed::Box;
use skein_lib::{Duration, List, ReplyTo, Wall};

/// A closed wake rule for task words and subscription hints (domain/tasks.md, section 8.3).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum WakeRule {
    Never,
    Immediate,
    Batch { count: u32, age: Duration },
}

/// How delegate result messages wake their requester.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ResultsWake {
    Never,
    Each,
    LastOrFailure,
}

/// Inert creator-selected wake policy, bounded at task admission.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct WakePolicy {
    pub words: WakeRule,
    pub notices: WakeRule,
    pub news: WakeRule,
    pub results: ResultsWake,
    pub questions: bool,
    pub answers: bool,
    pub timers: bool,
}

impl WakePolicy {
    pub const DEFAULT: Self = Self {
        words: WakeRule::Immediate,
        notices: WakeRule::Immediate,
        news: WakeRule::Immediate,
        results: ResultsWake::LastOrFailure,
        questions: true,
        answers: true,
        timers: true,
    };
}

/// Connector-classified news can be lowered by a task policy.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum NewsClass {
    Wakes,
    Kept,
    Dropped,
}

/// State of a watched task, distinct from a terminal result.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum NoticeState {
    Held { revision: u64 },
    Done,
    Failed,
    Cancelled,
}

/// One bounded standing task or timer interest; topics are joined in session 07.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SubscriptionKind {
    Task { target: u64, held: bool, result: bool },
    Timer { at: Wall, period: Option<Duration> },
    Topic { connector: u16, topic: u64 },
}

/// Root-issued subscription kept with its owner task.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Subscription {
    pub number: u64,
    pub kind: SubscriptionKind,
}

/// Kind of a durable task-inbox message (domain/tasks.md, sections 5.6 and 8).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum MessageKind {
    /// Virtual, durable held decision entry projected from the held task.
    Escalation { task: u64, revision: u64 },
    /// Virtual inbox entry for a pending proposal; its reason is in `Word::words`.
    /// The proposal owns its durable state and takes no inbox room.
    Proposal { proposer: u64, proposal: u64, kind: crate::ProposalKind },
    /// A person delegate passed its waiting deadline, delivered once to its requester.
    PersonStalled { task: u64 },
    /// Final accepted or rejected proposal, delivered to its proposer.
    ProposalDecision { proposal: u64, accepted: bool },
    /// A settings proposal outcome, delivered using its reserved inbox room.
    SettingsDecision { proposal: u64, outcome: crate::SettingsOutcome },
    /// Whole words from a person or referenced task.
    Words,
    /// A person settled the numbered uncertain effect as not made.
    EffectNotMade { entry: u64 },
    /// Latest committed amendment, always waking and merging by task.
    Amendment { revision: u64 },
    /// A cancel grace asks its executor to finish, in its own reserved slot.
    FinishRequested { revision: u64 },
    /// A bounded question for which the sender keeps answer room.
    Question,
    /// Answer to a named question previously asked of this sender.
    Answer { question: u64 },
    /// Merged task state hint for one standing interest.
    Notice { subscription: u64, target: u64, state: NoticeState },
    /// Merged timer fire for one standing interest.
    Timer { subscription: u64 },
    /// Merged connector hint; the connector route arrives in session 07.
    News { subscription: u64, class: NewsClass },
    /// A delegate's terminal result, sent with its end.
    Result(ResultKind),
    /// The core's notice that a kept task's last pending dependency is done,
    /// or was removed by an amendment: always waking, one per task, merged
    /// (domain/tasks.md, 5.7 and 8.1). `dependency` is the one whose end or
    /// removal emptied the pending set; of several removed at once, the
    /// first in the old list.
    Met { dependency: u64, removed: bool },
    /// Its person answered this task's received question in their inbox.
    QuestionAnswered { question: u64 },
    /// The question ended without an answer; its asker may go on.
    QuestionEnded { question: u64 },
}

/// Terminal shape carried with a delegate's bounded message words.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum ResultKind {
    /// Exact approved state; presence distinguishes absent from empty explanation words.
    Approved {
        state: Box<[u8]>,
        words_present: bool,
    },
    Report,
    Verdict {
        code: u32,
        fields: Box<[ResultField]>,
    },
    Change {
        connector: u16,
        kind: u16,
        resource: u64,
    },
    Failed,
    Cancelled,
}

/// One durable whole message in a task's bounded inbox
/// (domain/tasks.md, section 8.2).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Word {
    /// Root-issued commit-order message identity.
    pub number: u64,
    /// Root-verified sender, checked against requester or live delegate.
    pub from: Party,
    /// Reason this message entered the inbox.
    pub kind: MessageKind,
    /// Whole bounded words, never cut while in the inbox.
    pub words: Box<[u8]>,
    /// Injected time of admission, for oldest-first reads.
    pub at: Wall,
    /// Number of hints merged into this whole inbox entry.
    pub hits: u32,
    /// A batching threshold already reached, retained across restart.
    pub eligible: bool,
}

impl Word {
    /// Whether this message asks an agent to finish (domain/hosts.md, section 9).
    #[must_use]
    pub fn finish_requested(&self) -> bool {
        match self.kind {
            MessageKind::FinishRequested { .. } => true,
            MessageKind::Escalation { .. }
            | MessageKind::Proposal { .. }
            | MessageKind::PersonStalled { .. }
            | MessageKind::SettingsDecision { .. }
            | MessageKind::ProposalDecision { .. }
            | MessageKind::Words
            | MessageKind::EffectNotMade { .. }
            | MessageKind::Amendment { .. }
            | MessageKind::Question
            | MessageKind::Answer { .. }
            | MessageKind::Notice { .. }
            | MessageKind::Timer { .. }
            | MessageKind::News { .. }
            | MessageKind::Result(_)
            | MessageKind::Met { .. }
            | MessageKind::QuestionAnswered { .. }
            | MessageKind::QuestionEnded { .. } => false,
        }
    }

    /// Whole admitted payload, including the opaque state of an approval result.
    #[must_use]
    pub fn payload_bytes(&self) -> Option<usize> {
        match &self.kind {
            MessageKind::Result(ResultKind::Approved { state, words_present }) => {
                if !words_present && !self.words.is_empty() {
                    return None;
                }
                state.len().checked_add(self.words.len())
            }
            MessageKind::Result(ResultKind::Verdict { fields, .. }) => {
                crate::verdict::fields_bytes(fields)?.checked_add(self.words.len())
            }
            MessageKind::Result(
                ResultKind::Report | ResultKind::Change { .. } | ResultKind::Failed | ResultKind::Cancelled,
            )
            | MessageKind::Escalation { .. }
            | MessageKind::Proposal { .. }
            | MessageKind::PersonStalled { .. }
            | MessageKind::SettingsDecision { .. }
            | MessageKind::ProposalDecision { .. }
            | MessageKind::Words
            | MessageKind::EffectNotMade { .. }
            | MessageKind::FinishRequested { .. }
            | MessageKind::Amendment { .. }
            | MessageKind::Met { .. }
            | MessageKind::QuestionAnswered { .. }
            | MessageKind::QuestionEnded { .. }
            | MessageKind::Question
            | MessageKind::Answer { .. }
            | MessageKind::Notice { .. }
            | MessageKind::Timer { .. }
            | MessageKind::News { .. } => Some(self.words.len()),
        }
    }
}

/// A connector resource on which a run left state for its next activation.
/// The path is opaque to tasks and ordered by literal segment bytes.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct SavedResource {
    pub connector: u16,
    pub path: Box<[Box<[u8]>]>,
}

/// An unanswered question and the task allowed to use its reserved answer room.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct QuestionCredit {
    pub number: u64,
    pub answerer: Party,
    /// Logical recipient, distinct from the person who answers for that recipient.
    pub asked: Party,
    /// Original question activity in this credit owner's record (protocol/store.md, 15.7).
    pub line: u64,
}

/// Root-numbered terminal notices for one asker's retired question credits.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct QuestionNotice {
    pub question: u64,
    pub message: u64,
}

/// Root-verified requester/creator identity; requester topology and financial source are
/// separate values. (domain/tasks.md, sections 2–3).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Party {
    /// Task requester/creator whose delegation topology this child owns.
    Task(
        /** Deployment task number of the creator/requester; task requesters must remain live until their delegates end. */
         u64,
    ),
    /// Person requester/creator authenticated by the root.
    Person(/** Authenticated deployment person number supplied by the root. */ u64),
    /// Deployment-owned work for one project, authorized by the root.
    Deployment {
        /** Project on whose behalf deployment configuration starts the task. */
        project: u32,
    },
}

/// Task executor selected by the root. The procedure's owner interprets its opaque code.
/// (domain/tasks.md, section 2).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Executor {
    /// Configured agent charter.
    Agent {
        /** Configured agent charter number, checked against `Domain`'s admitted charter table. */
        charter: u32,
    },
    /// Connector or core procedure stepped by its owner through the root.
    Procedure { connector: u16, code: u32 },
    /// A person or any current holder of a project role answers in the people inbox.
    Person(PersonAddress),
}

/// Opaque person-task destination; root authenticates the identity or role membership.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PersonAddress {
    /// One named person may answer without a role claim.
    Person(u64),
    /// Any current holder of a project role may take and answer.
    Role(u32),
}

/// What to do when a new period arrives before the preceding batch has ended.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum RecurringOverlap {
    /// Let the due period pass without another batch.
    Skip,
    /// Remember the latest due period and make its batch after the old one ends.
    Wait,
}

/// A core procedure's durable batch template, with local member numbers starting at one.
/// The live procedure's authority is its per-period budget ceiling.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct RecurringTemplate {
    /// Stable deployment configuration identity within a project, used to fence restarts.
    pub key: u32,
    pub batch: Box<[New]>,
    pub overlap: RecurringOverlap,
}

/// The template and period cursor retained with one live recurring task.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct RecurringState {
    pub template: RecurringTemplate,
    /// Exact IDs of the last accepted template batch, retained after its members end.
    pub last_batch: Box<[u64]>,
    pub last_period: u64,
    pub pending_period: Option<u64>,
}

/// One procedure decision. Each step commits its task changes with the owner's state.
/// (domain/tasks.md, section 5.3).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ProcedureDecision {
    pub batch: Box<[New]>,
    pub amendments: Box<[ProcedureAmendment]>,
    pub cancels: Box<[ProcedureCancel]>,
    pub messages: Box<[ProcedureMessage]>,
    pub proposals: Box<[crate::Proposal]>,
    pub end: ProcedureEnd,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ProcedureAmendment {
    pub task: u64,
    pub message: u64,
    pub stop_run: bool,
    pub amendment: crate::Amendment,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ProcedureCancel {
    pub task: u64,
    pub reason: Box<[u8]>,
    pub grace: Option<Duration>,
    pub message: Option<u64>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ProcedureMessage {
    pub task: u64,
    pub word: Word,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ProcedureEnd {
    Result(TaskResult),
    Hold(Hold),
    Wait,
}

impl ProcedureDecision {
    #[must_use]
    pub fn new(end: ProcedureEnd) -> Self {
        Self {
            batch: Box::new([]),
            amendments: Box::new([]),
            cancels: Box::new([]),
            messages: Box::new([]),
            proposals: Box::new([]),
            end,
        }
    }
}

/// Bounded semantic parameter carrier; tasks does not interpret names or resources, and the
/// current root chat constructor supplies no parameters. (domain/tasks.md, section 3).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Parameter {
    /// Numeric specification carrier; current chat construction supplies no parameters.
    Number {
        /** Opaque semantic parameter name, carried without interpretation by tasks. */
        name: u32,
        /** Numeric parameter value, carried without interpretation here. */
        value: u64,
    },
    /// Bounded UTF-8 text specification carrier; current chat construction supplies no parameters.
    Bytes {
        /** Opaque semantic parameter name, carried without interpretation by tasks. */
        name: u32,
        /** Owned UTF-8 text bytes; these and `Spec::words` share `Limits::spec_bytes`. */
        value: Box<[u8]>,
    },
    /// Connector/resource carrier; no connector lookup or effect is performed by tasks.
    Resource {
        /** Opaque semantic parameter name, carried without interpretation by tasks. */
        name: u32,
        /** Connector identity carried without lookup or interpretation here. */
        connector: u16,
        /** Opaque semantic resource identifier; tasks does not load connector data. */
        resource: u64,
    },
}

/// Owned bounded task description supplied by root; current admission requires empty historical
/// inputs. (domain/tasks.md, section 3).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Spec {
    /// Owned UTF-8 specification words; combined with text-valued parameters, bounded by
    /// `Limits::spec_bytes`.
    pub words: Box<[u8]>,
    /// Parameters bounded by `Limits::parameters`; tasks does not interpret their names, and
    /// current root chat construction supplies an empty slice.
    pub parameters: Box<[Parameter]>,
    /// Historical input identities, bounded by `Limits::inputs` for shape measurement;
    /// current `Make` and live restore require this slice empty.
    pub inputs: Box<[u64]>,
}

/// One required named machine field and its value byte cap.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct FieldRule {
    pub name: Box<[u8]>,
    pub max: u32,
}

/// A named opaque machine value retained with a verdict.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ResultField {
    pub name: Box<[u8]>,
    pub value: Box<[u8]>,
}

/// One admitted result choice with a unique code and a per-choice word-byte bound.
/// (domain/tasks.md, sections 3 and 5.6).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Verdict {
    /// Distinct permitted choice code within its contract.
    pub code: u32,
    /// Maximum word bytes for this choice, no greater than `Limits::result_bytes`.
    pub words: u32,
    /// Maximum whole-batch follow-up delegates this choice permits.
    pub followups: u32,
    /// Required closed-list machine fields, whose values remain opaque.
    pub fields: Box<[FieldRule]>,
}

/// Root-supplied bounded result shape; tasks validates returned results against it before terminal
/// admission. (domain/tasks.md, sections 3 and 5.6).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Contract {
    /// Approval of exact opaque connector state (domain/people.md, section 8).
    Approval { state: Box<[u8]>, part: crate::Part },
    /// Bounded report result.
    Report {
        /** Maximum report bytes, no greater than `Limits::result_bytes`. */
        words: u32,
    },
    /// One of a nonempty admitted list of distinct verdict codes and byte caps.
    Verdict {
        /** Nonempty distinct-code choices, bounded by `Limits::contract_choices`, each with a result-byte bound. */
        choices: Box<[Verdict]>,
    },
    /// Connector/kind-matching result with an opaque resource and bounded report.
    Change {
        /** Required connector identity in a successful change result. */
        connector: u16,

        kind: u16,
        /** Maximum change-report bytes, no greater than `Limits::result_bytes`. */
        words: u32,
    },
}

/// Owned executor outcome checked against the admitted task contract before lifecycle admission;
/// connector interpretation and outward delivery remain with root. (domain/tasks.md, section 5.6).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum TaskResult {
    /// Exact state approved by the addressed person, with optional explanation.
    Approved { state: Box<[u8]>, words: Option<Box<[u8]>> },
    /// Successful bounded report matching a report contract.
    Report {
        /** Report bytes, bounded by both the admitted contract and `Limits::result_bytes`. */
        words: Box<[u8]>,
    },
    /// Successful admitted verdict code with bounded explanation.
    Verdict {
        code: u32,
        /// Each required field exactly once; values are never read by tasks.
        fields: Box<[ResultField]>,
        /** Explanation bytes bounded by that choice's word cap and `Limits::result_bytes`. */
        words: Box<[u8]>,
    },
    /// Successful matching connector/kind result; tasks performs no connector effect.
    Change {
        /** Connector identity that must equal the admitted change contract. */
        connector: u16,
        /** Connector-defined kind that must equal the admitted change contract. */
        kind: u16,
        /** Opaque resulting resource identifier; root/connector interpretation remains outside tasks. */
        resource: u64,
        /** Change-report bytes bounded by the admitted contract and `Limits::result_bytes`. */
        words: Box<[u8]>,
    },
    /// Bounded failure result allowed by every contract; produces an `Ending::Failed`.
    Failure {
        /** Failure-result reason, allowed by every contract and bounded by `Limits::result_bytes`. */
        reason: Box<[u8]>,
    },
}

/// Why an agent's proposed result failed its current admitted contract.
/// The next run receives this durable reason in its attempt summary.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum InvalidResult {
    /// The successful result used a different contract form.
    Form,
    /// A verdict named a code absent from the current closed choices.
    Verdict,
    /// The result's words exceeded their admitted bound.
    Words,
    /// A change named a different connector or result kind.
    Change,
    /// The verdict proposed more follow-up tasks than its contract permits.
    Followups,
}

/// Durable final outcome, or a pending closing outcome; requester notification follows
/// settlement. (domain/tasks.md, sections 5.1 and 5.6).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Ending {
    /// Checked successful result proposed for final ending; retained while closing and made
    /// historical only after settlement.
    Done(/** Successful checked result retained in pending closing or in the historical ended record. */ TaskResult),
    /// Failure ending with its bounded reason, pending while closing or final in the historical
    /// record.
    Failed {
        /** Owned failure reason bounded by `Limits::result_bytes`. */
        reason: Box<[u8]>,
    },
    /// Cancellation of descendant work, optionally preserving an already-completed result; no
    /// arbitrary public cancel route is exposed.
    Cancelled {
        /// Party whose decision cancelled this work.
        by: Party,
        /// Wall time when cancellation was decided.
        at: Wall,
        /** Owned cancellation reason bounded by `Limits::result_bytes`. */
        reason: Box<[u8]>,
        /** Optional already-completed result retained with cancellation, separately bounded and checked against the contract. */
        result: Option<TaskResult>,
    },
}

/// Content-free terminal classification used in observations and dependency readiness.
/// (domain/tasks.md, section 5.1).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Status {
    /// Task completed successfully; dependents can become due after settlement.
    Done,
    /// Task failed; waiting dependents are held.
    Failed,
    /// Task was cancelled; waiting dependents are held.
    Cancelled,
}

/// Bounded working-set pointer to an ended result still named by live work.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Stub {
    pub task: u64,
    pub phase: Status,
    /// `Key::Ended(result.raw())` is the historical result row.
    pub result: skein_lib::Token,
}

/// Root-to-tasks activation terminal, fenced by task/attempt; a terminal may park or retry without
/// ending the task. (domain/tasks.md, sections 5.2, 5.5 and 5.6).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum End {
    /// A configured bound refused the run's model call (domain/hosts.md, 6.3).
    Exceeded(Exceeded),
    /// Proposed final result; valid finishing with live delegates requires cancelling them or is
    /// refused without admission.
    Finished {
        /** Owned proposed result; invalid bounded shapes consume an `Invalid` failure, while oversized priced terminals refuse before charging. */
        result: TaskResult,
        /** Whether to cancel live descendant work; otherwise a valid finish with live delegates is refused and its attempt remains live. */
        cancel_delegates: bool,
    },
    /// A checked verdict whose follow-up batch is part of the result decision.
    FinishedWithFollowups {
        /// Verdict checked against the task's current result contract.
        result: TaskResult,
        /// Either immediately admitted delegates or one proposal for the whole batch.
        followups: ResultFollowups,
    },
    /// Successful activation park; clears retry/refusal counters and leaves the task idle until words wake it.
    Parked,
    /// Count one classified failure, then back off or hold beyond the configured retry allowance.
    Failed(/** Reported failure category whose counter/backoff policy applies. */ Class),
    /// Pre-execution refusal pause using the transient delay policy, without spending a failure
    /// try.
    Refused,
    /// Its source or checked placement requires a fresh check; release the run without a try or timer.
    Recheck,
}

/// Root-authorized follow-ups from one verdict, admitted with its result.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum ResultFollowups {
    /// A whole batch within the finishing task's authority and allotment.
    Delegates(Box<[New]>),
    /// One pending proposal for a whole batch beyond that authority.
    Proposal(Box<crate::Proposal>),
}

/// Root-reported or task-derived reason to preserve a task's prior lifecycle while stopping its
/// live activation. (domain/tasks.md, section 5.5).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Hold {
    /// A configured bound refused the run's model call (domain/hosts.md, 6.3).
    Exceeded(Exceeded),
    /// Configured retry allowance was exhausted.
    Failures(/** Failure class whose retry allowance was exhausted. */ Class),
    /// Executor made no progress within its configured stall bound.
    Stalled,
    /// A required dependency failed or was cancelled.
    Dependency(/** `Dependency` that ended failed or cancelled, causing this dependent to be held. */ u64),
    /// Root reports a person's stop or another authorized stop decision.
    Stopped,
    /// The placed hold that holds this task, when no cause of 5.5 does.
    Placed { hold: u64 },
    /// Root reports resource drift requiring a decision; tasks owns no connector drift detection.
    Drift,
    /// Root reports a permanently failed effect requiring a decision.
    Effects,
    /// A connector effect failed for good.
    EffectFailed,
    /// An unrecoverable effect may or may not have reached its system.
    Uncertain { entry: u64 },
    /// Available funding is too small for another run or effect.
    Budget,
    /// Root reports that the permitted deadline prevents another activation.
    Deadline,
    /// A task waited beyond the bound for its required resource holds.
    HoldsWaited,
    /// A connector reports that a named resource is no longer available.
    ResourceUnavailable,
    /// Current adoption forbids a running workspace write.
    ResourceAccess,
    /// The numbered pool lost its allocation while this task held or awaited it.
    PoolLost { pool: u64 },
    /// A procedure asked its requester for a decision.
    Procedure,
    /// Joining a queue, or the waits a release gives back, would close a circle of waits; the
    /// hold's source names the circle, and the hold lifts once it breaks (domain/tasks.md, 6.7).
    Circle,
}

impl Hold {
    pub(crate) const fn valid(self) -> bool {
        match self {
            Hold::Dependency(task) => task != 0,
            Hold::Placed { hold } => hold != 0,
            Hold::Uncertain { entry } => entry != 0,
            Hold::PoolLost { pool } => pool != 0,
            Hold::Exceeded(_)
            | Hold::Failures(_)
            | Hold::Stalled
            | Hold::Stopped
            | Hold::Drift
            | Hold::ResourceUnavailable
            | Hold::ResourceAccess
            | Hold::Effects
            | Hold::EffectFailed
            | Hold::Budget
            | Hold::Deadline
            | Hold::HoldsWaited
            | Hold::Procedure
            | Hold::Circle => true,
        }
    }
}

/// Current agent-task activation phase; task and attempt identity fence every external transition.
/// (domain/tasks.md, section 5.2).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Active {
    /// `Activation` parked; no current wake input is exposed by this contracted child.
    Idle,
    /// An activation is requested and root owns preparation.
    Due,
    /// Root is gathering the brief/resources; a claim or preparation failure follows.
    Preparing,
    /// Root committed the fenced attempt before assignment/start.
    Claimed {
        /** Committed root-issued strictly increasing nonzero attempt identity. */
        attempt: u64,
    },
    /// Root reported the exact claimed attempt started.
    Running {
        /** Current claimed attempt reported as started. */
        attempt: u64,
    },
    /// `Retry`/refusal pause with one projected deadline.
    BackingOff {
        /** Saved wall-time retry threshold, projected once to a monotonic deadline and reprojected on restore. */
        until: Wall,
    },
}

/// Ordered closing obligation; the root confirms effect settlement before resource release.
/// (domain/tasks.md, section 5.1).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Stage {
    /// Wait for the exact live attempt's terminal.
    Run {
        /** `Live` attempt whose terminal must be heard before closing advances. */
        attempt: u64,
    },
    /// Wait for all live delegates and incoming financial allocations to settle.
    Delegates,
    /// `Close` output is owed settlement of all prior effects from root.
    Effects,
    /// `Release` output is owed settlement of resource cleanup from root.
    Releases,
    /// Root closing obligations settled; final financial posting and historical end can commit.
    /// Unheld `Settled` is transient and refused on live restore; held prior closing may retain it.
    Settled,
}

/// Owned pending ending and closing stage retained until every obligation and financial allocation
/// settles. (domain/tasks.md, section 5.1).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Closing {
    /// Next unsettled closing obligation.
    pub stage: Stage,
    /// Bounded pending final outcome, preserved through holds and cancellation.
    pub ending: Ending,
}

/// Lifecycle preserved while held; terminal/settlement notifications can update it without lifting
/// the hold. (domain/tasks.md, section 5.1).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Was {
    /// Prior dependency-waiting phase.
    Waiting,
    /// Prior agent activation phase.
    Active(/** Prior agent activation phase, including its attempt/backoff fence. */ Active),
    /// Prior pending closing state.
    Closing(/** Prior closing stage and owned pending ending. */ Closing),
}

/// Task's complete lifecycle value; ended values occur in historical durable rows rather than the
/// live arena. (domain/tasks.md, section 5.1).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Phase {
    /// `Some` live dependency remains; readiness becomes `Due` when `waiting_on` empties.
    Waiting,
    /// `Agent` activation is idle, due, preparing, claimed, running or backing off.
    Active(Active),
    /// Pending ending waits for ordered activation/delegate/root settlement.
    Closing(/** Owned pending ending and next closing obligation. */ Closing),
    /// Prior lifecycle preserved for a decision; current contracted API exposes no general
    /// release/amend route.
    Held {
        /** Prior phase retained and updated while held; the hold is not lifted by a terminal. */
        was: Was,

        why: Hold,
        /// Exact connector-owned cause, retained in lifecycle milestones.
        source: Option<Box<HoldSource>>,
    },
    /// Historical outcome outside the live arena; restoring it as live is refused.
    Ended(/** Historical final ending; this phase is excluded from the live restore arena. */ Ending),
}

/// What explains a hold beyond its reason: a connector's resource or effect, or the circle of
/// waits a `Hold::Circle` names (client/domain/objects.md, 5; domain/tasks.md, 6.7).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum HoldSource {
    Resource(Name),
    Effect { connector: u16, entry: u64 },
    Circle(Circle),
}

/// A circle of waits: tasks each waiting for the next, the last for the first, so that none of
/// them could end (domain/tasks.md, 6.7).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Circle {
    /// The circle's tasks in order, from the one whose new wait would close it, at most
    /// `Limits::circle`.
    pub tasks: Box<[u64]>,
    /// The circle has more tasks than these.
    pub cut: bool,
}

/// A connector resource name opaque to the tasks hub (domain/tasks.md, 6.1).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Name {
    /// Connector number assigned by the application's root.
    pub connector: u16,
    /// Literal path segments with no connector-specific interpretation here.
    pub path: Box<[Box<[u8]>]>,
}

/// Whether a busy resource refuses admission or queues its next holder.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Taken {
    /// Refuse a batch needing this resource and name its holder.
    Refuses,
    /// Admit the task with no holds until the complete set is free.
    Waits,
}

/// Connector-configured admission rule for one resource kind.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum HoldKind {
    /// The connector orders writes itself; no task hold is taken.
    Shared,
    /// One task holds the resource.
    Exclusive { taken: Taken },
    /// Up to the connector's current number of slots hold the pool.
    Pooled { taken: Taken },
}

/// One resource a task needs before it may become active.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Holding {
    /// Exclusive write resource and its connector-defined kind.
    Write { resource: Name, kind: u16 },
    /// Counted pool resource and its connector-defined kind.
    Slot { pool: Name, kind: u16 },
}

/// The one writer of an exclusively held resource (domain/tasks.md, 6.3).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Writer {
    /// A claimed agent run, fenced by its attempt.
    Run { task: u64, attempt: u64 },
    /// A connector-owned outbox effect.
    Effect { entry: u64 },
}

/// Durable writer slot. A lost run retains the slot until its connector reads afresh.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct WriterSlot {
    pub number: u64,
    pub resource: Name,
    pub writer: Writer,
    pub lost: bool,
}

/// Last durable connector report of one pool's current admission capacity.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct PoolSlots {
    pub number: u64,
    pub pool: Name,
    pub slots: u32,
}

/// One connector's configured resource kind.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Kind {
    pub connector: u16,
    pub kind: u16,
    pub hold: HoldKind,
}

/// The proposal deadline retained until the task-making decision.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Deadline {
    Named,
    Span(Duration),
}

/// Root-authorized member of an atomic creation batch; tasks preflights the whole graph, payloads,
/// capacity and finite reservations before mutation. (domain/tasks.md, sections 3–4).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct New {
    /// Opaque application label, immutable after creation.
    pub label: u32,
    /// Immutable engine flags.
    pub flags: crate::Flags,
    /// Fresh never-reused task number supplied by the root; live or within-batch duplicates refuse.
    pub number: u64,
    /// Policy project authenticated by root and checked against creator/project structural limits.
    pub project: u32,
    /// `Agent` charter executor, checked against admitted configuration.
    pub executor: Executor,
    /// An agent task's LLM choice, checked against the configured models, its charter and its
    /// authority; absent for a procedure's or a person's task, and where the root resolves the
    /// default (domain/tasks.md, sections 3 and 4).
    pub llm: Option<crate::LlmChoice>,
    /// Owned specification within configured counts/bytes, with empty historical inputs.
    pub spec: Spec,
    /// Admitted bounded result contract.
    pub contract: Contract,
    /// Complete root-approved authority carrier; tasks checks bounded shape, not policy inclusion.
    pub authority: Authority,
    /// Deadline selection retained in pending proposals.
    pub deadline: Deadline,
    /// Fresh allotment: budget equals authority spend, with all spend/reservation counters zero.
    pub numbers: Numbers,
    /// Finite source; task sources must be live ancestors of the task creator and external
    /// sources must belong to this project.
    pub funder: Funder,
    /// Distinct immutable dependencies, at most `Limits::dependencies`: members of this batch or
    /// the creator's current live delegates.
    pub dependencies: Box<[u64]>,
    /// Connector resources taken together when the task first becomes active.
    pub holdings: Box<[Holding]>,
    /// Creator-selected policy for wakes and batching.
    pub wake: WakePolicy,
    /// Present only for the core recurring procedure; template members cannot recur.
    pub recurring: Option<Box<RecurringTemplate>>,
    /// Goal priority when people track this task; absent for ordinary work.
    pub tracked: Option<u32>,
}

/// Revisions of the individual things a party decides, independent of history,
/// spend and executor progress (client/domain/contract.md, section 5.2).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Revisions {
    /// Moves when a hold begins or its reason changes, never when it passes up.
    pub hold: u64,
    /// Moves when an amendment changes the task's specification or policy.
    pub spec: u64,
    /// Moves when a person task's question or displayed results change.
    pub person: u64,
}

/// The first accepted person answer, independent of the temporary role claim.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PersonAnswer {
    pub by: u64,
    pub at: Wall,
}

/// One waiting interval of a person task, retained through claims and restart.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PersonWait {
    pub since: Wall,
    pub stall: Duration,
    pub deadline: Wall,
    pub notice: Option<PersonStallNotice>,
}

/// One numbered news item issued when a person task passed its wait.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PersonStallNotice {
    pub message: u64,
    pub at: Wall,
    /// Exact immutable outgoing activity in this task's ordered tail.
    pub activity: u64,
}

/// One committed turn's reply reference (domain/people.md, section 6).
/// Retained hints form the newest (wall time, reply ordinal) prefix, independently of `last_reply`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ReplyHint {
    pub number: u64,
    pub activity: u64,
    pub attempt: u64,
    pub turn: u32,
    pub at: Wall,
}

/// Settings revisions retained live and ended (domain/engine.md, 17.3).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SettingsRevision {
    /// The newest revision when the task was made, whatever its parent's.
    pub task: u32,
    /// The revision its agent run was prepared under; none between attempts.
    pub run: Option<u32>,
}

/// Owned durable task state emitted to root storage; root uses `RunContext` for preparation and
/// does not maintain a mutable copy of this ledger. (domain/tasks.md, sections 3 and 5).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[expect(clippy::struct_excessive_bools, reason = "independent durable lifecycle facts are explicit")]
pub struct TaskRecord {
    /// The placed hold holding this task, whatever its phase says (domain/tasks.md, 5.9).
    pub placed: Option<u64>,
    /// Settings read by this task and its run (domain/engine.md, 17.3).
    pub settings: SettingsRevision,
    /// Opaque application label, immutable after creation.
    pub label: u32,
    /// Immutable engine flags.
    pub flags: crate::Flags,
    /// Last words heard, turn committed or delegate ended; creation until then.
    pub last_activity: Wall,
    /// Lifetime ended descendants, summed as each delegate ends; counts saturate.
    pub ended_below: crate::Endings,
    /// Newest question and answer activity positions, at most `inbox_messages`.
    pub question_lines: Box<[crate::QuestionLine]>,
    /// Latest hold milestone; zero before the first hold.
    pub hold: u64,
    /// Newest decided proposal history positions, at most `inbox_messages`.
    pub proposal_rows: Box<[crate::ProposalRow]>,
    /// Agent instruction replacement; absent uses the configured charter default.
    pub instructions: Option<Box<[u8]>>,
    /// An agent task's LLM choice, kept as given and changed only by an amendment
    /// (domain/tasks.md, sections 3 and 7).
    pub llm: Option<crate::LlmChoice>,
    /// Main account from the last successful checked claim, independent of its preference.
    pub kept_account: Option<u32>,
    /// Messages the current attempt's run refused; untaken by later fences of that attempt.
    pub refused: Box<[u64]>,
    /// A cancel asked this task to finish by a durable deadline.
    pub grace: Option<crate::grace::Grace>,
    /// Independent revisions retained in live and ended records.
    pub revisions: Revisions,
    /// Injected creation time used to order active person tasks in derived inboxes.
    pub created_at: Wall,
    /// Wall time of the last decision that changed this retained row.
    pub changed: Wall,
    /// Injected wall time at which the current lifecycle phase began.
    pub phase_since: Wall,
    /// First matching task start report: root reports placement or adoption, not agent or tool start.
    /// Retained through holds and the terminal; a new claim clears it.
    pub run_since: Option<Wall>,
    /// The classified failure or pre-execution refusal that last paused this activation.
    pub retry_notice: Option<crate::RetryNotice>,
    /// Injected time of the durable ending, used for newest-first result pages.
    pub ended_at: Option<Wall>,
    /// Last committed semantic change to this task; history rows are keyed by this sequence.
    pub revision: u64,
    /// Last lifecycle milestone, independent of semantic amendments and proposals.
    pub milestone: u64,
    /// Last immutable activity position; restored with live state, never reused.
    pub activity: u64,
    /// Actual activation cause, retained through preparation and restart.
    pub due_reason: crate::Why,
    /// An admitted authority narrowing stopped the current run; its terminal starts the task anew.
    pub narrowing: bool,
    /// Root-issued commit order for an ended result; zero while live and until the root saves an ending.
    pub result_position: u64,
    /// Consecutive committed reply ordinal across attempts.
    pub last_reply: u64,
    /// Newest opaque reply hints, bounded by `inbox_messages`; older lines are store-loaded.
    pub replies: Box<[ReplyHint]>,
    /// One bounded semantic held-chat decision and checked revision. Root owns
    /// authentication, routing and historical receipts.
    pub escalation: crate::Escalation,
    /// At most one pending action awaiting a holder; its terminal leaves live memory as history.
    pub proposal: Option<Box<crate::Proposal>>,
    /// A result-created proposal keeps closing at its delegate gate until decided.
    pub result_proposal: bool,
    /// Stable never-reused deployment task identity.
    pub number: u64,
    pub project: u32,
    /// Original current requester topology, distinct from the financial funder.
    pub requester: Party,
    /// Committed cumulative expense of the current attempt; a `new` `Claim` resets only this
    /// attempt baseline, not total direct spend.
    pub run_spent: u64,
    /// Unspent part of this attempt's reserved run allowance, included in `numbers.reserved`.
    pub run_reserved: u64,
    /// Requester-tree root identity, equal to number for a top-level task.
    pub root: u64,
    /// Structural depth below that root, bounded by `Limits::depth`.
    pub depth: u32,
    pub executor: Executor,
    /// Current role-task holder; direct-person tasks never need a claim.
    pub taken_by: Option<u64>,
    /// The first accepted answer, retained after the role claim is cleared.
    pub person_answer: Option<PersonAnswer>,
    /// The interval starts when dependency and resource waits clear.
    pub person_wait: Option<PersonWait>,
    /// Durable core recurring template and last considered period.
    pub recurring: Option<Box<RecurringState>>,
    /// Goal priority retained with the task for project ordering.
    pub tracked: Option<u32>,
    /// Owned bounded specification; live state has empty historical inputs.
    pub spec: Spec,
    pub contract: Contract,
    /// Owned bounded permission carrier checked by root policy, not by tasks.
    pub authority: Authority,
    /// Authentic current-allotment accounting, updated atomically with lifecycle admission.
    pub numbers: Numbers,
    /// Source of the current allotment, kept separate from requester ancestry.
    pub funder: Funder,
    /// Admitted dependency identities, bounded by `Limits::dependencies`: those named at creation,
    /// then those amendments added, the oldest first (domain/tasks.md, section 7).
    pub dependencies: Box<[u64]>,
    /// The dependencies an amendment added, each with when and by whom, in their order among
    /// `dependencies`.
    pub added: Box<[Added]>,
    /// `Exact` still-live subset of dependencies, bounded by `Limits::dependencies`; removed once
    /// per dependency end in the same decision. A waiting task waits for them; an active one is
    /// kept by them (domain/tasks.md, 5.7). `Restore` checks count/uniqueness/subset before
    /// retention and complete live coverage after all pages.
    pub waiting_on: Box<[u64]>,
    /// Resources currently held or awaited; a connector may pass a failed delegate's hold to its tree root during release.
    pub holdings: Box<[Holding]>,
    /// True only after the whole requested set has been taken.
    pub holds_taken: bool,
    /// First wall time at which dependencies cleared but a hold was unavailable: the task's
    /// place in its queues.
    pub hold_wait_since: Option<Wall>,
    /// When the wait bound counts from, if not from `hold_wait_since`: a task that left its
    /// queues for a dependency added or a circle comes back at its place, its bound counted
    /// afresh (domain/tasks.md, 6.2 and 6.7).
    pub hold_bound_since: Option<Wall>,
    /// Current live direct delegates, bounded by `Limits::delegates`; requester ending waits until
    /// they are gone.
    pub delegates: Box<[u64]>,
    /// Exact result-created task identities, retained after delegates end or are taken.
    /// A result admits one batch, directly or through its accepted proposal.
    pub result_followups: Box<[u64]>,
    /// Introduced peer tasks this task may message or name as a live dependency.
    pub references: Box<[u64]>,
    /// Open questions with one reserved answer slot apiece.
    pub questions: Box<[QuestionCredit]>,
    /// Standing interests that end with the task.
    pub subscriptions: Box<[Subscription]>,
    /// Current wake and batching policy.
    pub wake: WakePolicy,
    /// Latest newly admitted contiguous turn in the current attempt; `new` `Claim` starts at zero.
    pub turn: u32,
    /// Newest message ever admitted, including those taken by committed turns.
    pub last_message: u64,
    /// Whole unread words in increasing root message order.
    pub inbox: Box<[Word]>,
    /// Resource names with saved state, ordered and unique.
    pub saved: Box<[SavedResource]>,
    /// Some attempt of this task committed a turn, so its next preparation must read its transcript.
    pub ever_turned: bool,
    /// Lifetime tasks made in this subtree, including itself, bounded by `Limits::tree_tasks`;
    /// ending delegates does not return capacity.
    pub made: u32,
    /// Latest root-issued attempt identity; `new` claims must strictly increase it.
    pub attempt: u64,
    /// Most recently accepted activation-terminal attempt while the task remains live; not an exact
    /// transport replay receipt.
    pub last_answer: Option<u64>,
    /// Per-class failure history used with the configured retry policy.
    pub tries: Tries,
    /// Last invalid result's contract reason, kept through the next preparation.
    pub invalid_result: Option<InvalidResult>,
    /// Saturating pre-execution refusal counter used for pauses rather than failure tries.
    pub refusals: u32,
    /// Current lifecycle or the ended historical outcome.
    pub phase: Phase,
}

pub use crate::index::Key;

/// Tasks-to-root persistence row or root-to-tasks startup restore input; live tasks,
/// funding, writer slots and pool counts are restored before admission.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Stored {
    /// A standing hold, or a lifted hold still named by live tasks.
    PlacedHold(Box<crate::PlacedHold>),
    /// One party's durable update index.
    Update(crate::Update),
    Result(crate::ResultRow),
    EndedByParty(crate::EndedByParty),
    EndedByProject(crate::EndedByProject),
    EndedDelegate(crate::Edge),
    Delegate(crate::Edge),
    /// Historical activity loaded on demand, never restored as live state.
    Activity(crate::ActivityLine),
    /// One immutable lifecycle milestone, loaded on demand.
    Milestone(crate::Milestone),
    /// Pending person-origin goal proposal.
    PersonProposal(Box<crate::PersonProposal>),
    /// A decided person-origin goal proposal, loaded on demand.
    DecidedProposal(Box<crate::PersonProposal>),
    /// A closed funding source, loaded on demand.
    ClosedLedger(crate::FundingRecord),
    /// Committed change read on demand, never restored as live state.
    History(crate::History),
    /// Current bounded live task state, emitted on mutation and admitted once at startup.
    Live(
        /** Owned boxed bounded live task row; startup validates its shape and later validates graph/financial links. */
         Box<TaskRecord>,
    ),
    /// Historical final task state; root loads it for authenticated result reads, never as
    /// live startup state.
    Ended(
        /** Owned boxed historical ending retained in root storage; never a live restore input or a replayed result delivery. */
         Box<TaskRecord>,
    ),
    /// Compact durable pointer to a historical result named by live work.
    Stub(Stub),
    /// Authentic finite external source accounting; restored links are checked with live
    /// reservations.
    Ledger(
        /** Finite period/pool accounting owned by tasks and admitted at startup under `Limits::funders`. */
        crate::FundingRecord,
    ),
    /// Occupied writer slot, restored before claims are adopted.
    Writer(WriterSlot),
    /// Last reported pool capacity, restored before hold readiness runs.
    Pool(PoolSlots),
}

impl Stored {
    /// Pure fixed-size projection of this row's logical `key`; no allocation, mutation, persistence
    /// or lifecycle output.
    #[must_use]
    pub const fn key(&self) -> Key {
        match self {
            Stored::PlacedHold(row) => Key::PlacedHold(row.number),
            Stored::Update(row) => row.key(),
            Stored::EndedByParty(row) => row.key(),
            Stored::EndedByProject(row) => row.key(),
            Stored::EndedDelegate(row) => Key::EndedDelegate { requester: row.requester, task: row.task },
            Stored::Delegate(row) => Key::Delegate { requester: row.requester, task: row.task },
            Stored::Result(row) => Key::Result { party: row.party, position: row.position },
            Stored::PersonProposal(row) => Key::PersonProposal(row.number),
            Stored::DecidedProposal(row) => Key::DecidedProposal(row.number),
            Stored::ClosedLedger(row) => Key::ClosedLedger(row.funder),
            Stored::History(row) => Key::History { task: row.task, revision: row.revision },
            Stored::Activity(row) => Key::Activity { task: row.task, position: row.position },
            Stored::Milestone(row) => Key::Milestone { task: row.task, position: row.position },
            Stored::Live(record) => Key::Live(record.number),
            Stored::Ended(record) => Key::Ended(record.number),
            Stored::Stub(stub) => Key::Stub(stub.task),
            Stored::Ledger(record) => Key::Ledger(record.funder),
            Stored::Writer(slot) => Key::Writer(slot.number),
            Stored::Pool(row) => Key::Pool(row.number),
        }
    }
}

/// Terminal bounded admission reason; refusal makes no requested lifecycle or financial mutation.
/// (domain/tasks.md, sections 4–5).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Refusal {
    /// A standing placed hold forbids delegation into its scope or detachment from it.
    Placed { hold: u64 },
    /// The task caller may not lift this placed hold.
    HoldOwner { hold: u64 },
    /// The named placed hold revision moved.
    HoldRevision { revision: u64 },
    /// An explicit account preference was refused before the task changes.
    Account { account: u32, why: crate::AccountWhy },
    /// A cancel gave a grace longer than the configured span.
    Grace,
    /// The project has no room for more live tracked tasks.
    Goals,
    /// No policy role may accept this proposal kind; nothing of it is committed.
    Unproposable,
    /// The address edit names an older person task revision.
    PersonRevision { revision: u64 },
    /// A required resource is unavailable in this project.
    ResourceUnavailable,
    /// Startup is restoring, checked or failed; no mutation is admitted.
    NotReady,
    /// Named creator/task is not in the live arena.
    Unknown,
    /// Finite source table has no room before source creation/carving.
    Busy,
    /// Task/source identity is already retained or a task number repeats within the batch.
    Duplicate,
    /// `Make` batch is empty.
    Empty,
    /// `Batch` exceeds the configured direct-child count.
    Batch,
    /// `Batch` cannot fit the live task slab/name set, including slots not yet reclaimed.
    Live,
    /// Creator/project identity or per-project live task count is invalid.
    Project,
    /// Lifetime requester-tree task count would exceed its configured bound.
    Tree,
    /// A `new` child's structural depth would exceed the configured bound.
    Depth,
    /// Creator's live delegate list cannot fit the batch.
    Delegates,
    /// A sender lacks a live delegation or introduced reference to the target.
    Reference,
    /// Subscription count, identity, target or timer shape was refused.
    Subscription,
    /// `Dependency` count, uniqueness or current-live/same-batch identity is invalid.
    Dependencies,
    /// The new waits would close a circle of waits; `blocked_by` names its tasks in order, each
    /// waiting for the next, at most `Limits::circle` (domain/tasks.md, 6.7).
    Circle { cut: bool },
    /// A dependency an amendment adds has ended failed or cancelled; `blocked_by` names it
    /// (domain/tasks.md, section 7).
    DependencyEnded { how: Status },
    /// The task is kept by pending dependencies, which `blocked_by` names, the oldest first: a
    /// person's answer, a procedure's step ending it done, or an effect that waits for them
    /// (domain/tasks.md, 5.7).
    Kept,
    /// Its own question keeps it until its requester answers.
    KeptQuestion { question: u64 },
    /// The asked task cannot answer a question waiting for its person.
    PersonAnswers { question: u64 },
    /// `Agent` charter is absent from the admitted configured table.
    Executor,
    /// Specification count or aggregate owned bytes are invalid.
    Spec,
    /// `TaskResult`-contract shape or a priced terminal's result-byte admission is invalid.
    Contract,
    /// Carried authority collections or aggregate bytes exceed the configured shape bounds.
    AuthorityShape,
    /// Historical inputs are nonempty; no current root historical-input creation route is
    /// implemented.
    Inputs,
    /// Current task phase does not permit the requested admission.
    State,
    /// `Attempt` is stale, zero or incompatible with the current phase.
    Attempt,
    /// Valid finishing declines to cancel still-live delegates; activation and charge remain
    /// unadmitted.
    LiveDelegates,
    /// `Live` startup row shape, graph or financial links are invalid; historical rows
    /// cannot be restored live.
    Restore,
    /// A turn's read fence was not an offered unread message.
    Read,
    /// `Turn` is not the next contiguous current-attempt turn, or cumulative expense decreases.
    Turn,
    /// Authentic finite source/reservation or eventual funding-chain arithmetic cannot admit the
    /// whole decision.
    Funding,
    /// A task names an unknown, malformed, or mismatched resource kind.
    HoldKind,
    /// A required exclusive resource is busy and its kind refuses waiting.
    HoldTaken,
    /// A task's hold list or a resource's waiting queue exceeds its bound.
    Holds,
    /// An LLM choice names `model`, which it may not, and why (domain/tasks.md, sections 3 and 4).
    Llm { model: u8, why: crate::LlmFault },
}

/// `Refusal` location and reason returned to the root; batch-wide failures may have no single task
/// identity. (domain/tasks.md, sections 4–5).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Problem {
    /// Offending task identity when one exists; `None` denotes a batch/source/global failure.
    pub task: Option<u64>,
    /// Bounded structural, lifecycle, financial or restore admission reason.
    pub why: Refusal,
    /// The tasks the refusal names: the live delegates that prevented a finish, in the requester's
    /// current order; a circle's tasks; an ended dependency; or the pending dependencies keeping
    /// the task. Absent for every other refusal.
    pub blocked_by: Option<Box<[u64]>>,
    /// Exact room check metadata; absent for shape, state and other refusals.
    pub capacity: Option<crate::Capacity>,
}

impl Problem {
    /// A refusal with no live-delegate blockers.
    #[must_use]
    pub const fn new(task: Option<u64>, why: Refusal) -> Self {
        Self { task, why, blocked_by: None, capacity: None }
    }

    /// An unchanged refusal carrying owner-proven capacity metadata.
    #[must_use]
    pub const fn capacity(task: Option<u64>, why: Refusal, capacity: crate::Capacity) -> Self {
        Self { task, why, blocked_by: None, capacity: Some(capacity) }
    }

    /// A finish refused until these named delegates end or are cancelled.
    #[must_use]
    pub fn live_delegates(task: u64, delegates: &[u64]) -> Self {
        Self::naming(Some(task), Refusal::LiveDelegates, delegates)
    }

    /// A refusal naming these tasks.
    #[must_use]
    pub fn naming(task: Option<u64>, why: Refusal, tasks: &[u64]) -> Self {
        Self { task, why, blocked_by: Some(Box::from(tasks)), capacity: None }
    }
}

/// Lifecycle acknowledgement classification, not an exact payload replay proof; exact transport
/// replay is the root's responsibility. (domain/tasks.md, section 5.6).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Accepted {
    /// This event newly admitted the task transition or turn.
    New,
    /// `Unpriced` terminal attempt equals the live task's recorded `last_answer`; root remains
    /// responsible for exact payload proof.
    Already,
}

/// Root-issued semantic task/finance inputs after authorization and transport fencing;
/// notifications have no reply destination and may be ignored if stale. (domain/tasks.md, sections 4–5).
#[derive(PartialEq, Eq, Debug)]
pub enum Event {
    /// Root-checked placement; the root numbers it and judges standing, the child judges room.
    Place { reply_to: ReplyTo, hold: crate::PlacedHold },
    /// Root-checked lift naming the placed hold revision (domain/tasks.md, 5.9).
    Lift { reply_to: ReplyTo, by: Party, hold: u64, revision: u64, with_tasks: bool },
    /// Move eligible live agent tasks on `from`, or every older revision, to the newest (engine.md, 17.4).
    MoveToNewest { from: Option<u32> },
    /// Replace held charter versions and name the newest settings after a commit or start
    /// (engine.md, 17.2 and 17.3).
    Charters { newest: u32, charters: Box<[crate::CharterModels]>, efforts: Box<[u8]> },
    /// One project's party operations, admitted in order and made whole.
    Compound {
        reply_to: ReplyTo,
        by: Party,
        project: u32,
        operations: List<crate::Operation>,
        period: u64,
        pool_budget: u64,
        period_budget: u64,
    },
    /// Latest connector admission rule for this individual resource.
    Resource { name: Name, hold: HoldKind },
    /// Current adoption access for a named resource in one project.
    ResourceAccess { project: u32, name: Name, access: ResourceAccess },
    /// Root installs one connector's bounded resource-kind configuration.
    Kinds { connector: u16, kinds: Box<[Kind]> },
    /// Connector reports its current slot count for one opaque pool.
    Slots { pool: Name, slots: u32 },
    /// Connector reports that a holder's allocation in this pool vanished.
    AllocationGone { pool: Name, task: u64 },
    /// Connector facts changed for one idle procedure; a due step is offered once.
    WakeProcedure { task: u64 },
    /// Root admits a person's goal proposal after validating policy and shape.
    ProposePerson { reply_to: ReplyTo, proposal: crate::PersonProposal },
    /// Withdraw the proposing party's pending proposal.
    WithdrawPersonProposal { reply_to: Option<ReplyTo>, proposer: u64, proposal: u64 },
    /// Root commits a policy holder's decision on a person-origin goal proposal.
    DecidePersonProposal {
        reply_to: ReplyTo,
        proposer: u64,
        proposal: u64,
        by: Party,
        message: Option<u64>,
        decision: crate::ProposalDecision,
        revision: Option<u32>,
    },
    /// A newly opened project period makes the core recurring procedure due.
    TickRecurring { task: u64, period: u64 },
    /// A subscribed top-level procedure receives its next period's task and spend allotment.
    RenewStanding { task: u64, period: u64 },
    /// Root supplies fresh identities for the template batch requested by the core procedure.
    RecurringBatch { task: u64, period: u64, numbers: Box<[u64]> },
    /// Fenced decision from the owner of one due procedure task.
    Procedure {
        reply_to: ReplyTo,
        task: u64,
        step: u64,
        /// Last message in the prefix offered to this step, or none for an empty inbox.
        read: Option<u64>,
        decision: ProcedureDecision,
    },
    /// Root-authenticated role holder takes an active person task from every role inbox.
    TakePerson { reply_to: ReplyTo, task: u64, person: u64 },
    /// The current holder returns a role task to the role inbox.
    HandBackPerson { reply_to: ReplyTo, task: u64, person: u64 },
    /// Root-authenticated addressee answers a live person task under its result contract.
    AnswerPerson { reply_to: ReplyTo, task: u64, person: u64, result: TaskResult },
    /// Root-authorized bounded proposal and resolved first holder.
    Propose { reply_to: ReplyTo, proposal: crate::Proposal },
    /// Root-checked current holder resolves a pending proposal after executing an acceptance.
    DecideProposal {
        reply_to: ReplyTo,
        proposer: u64,
        proposal: u64,
        /// Fresh root-issued message for acceptance/rejection, absent for pass.
        message: Option<u64>,
        by: Party,
        decision: crate::ProposalDecision,
    },
    /// Root completes a settings proposal atomically with its revision or stale withdrawal.
    DecideSettingsProposal { reply_to: Option<ReplyTo>, decision: crate::SettingsDecision },
    /// Proposer withdraws its own pending action before it closes.
    WithdrawProposal { reply_to: ReplyTo, proposer: u64, proposal: u64 },
    /// Root routes one pending proposal only from the holder and revision it checked.
    StalledProposal {
        proposer: u64,
        proposal: u64,
        from: crate::ProposalHolder,
        revision: u64,
        holder: crate::ProposalHolder,
    },
    /// Root-checked live ancestor controls a delegate and its subtree.
    Control { reply_to: ReplyTo, by: Party, task: u64, control: crate::Control },
    /// Root-checked amendment of a live delegate, with a fresh commit-order message number.
    Amend {
        /// Ended dependencies and the ended requesters used to judge their reach.
        places: Box<[crate::DependencyPlace]>,
        reply_to: ReplyTo,
        by: Party,
        task: u64,
        message: u64,
        stop_run: bool,
        amendment: crate::Amendment,
        /// The accepter's source; a direct amendment keeps its present funding.
        funder: Option<Funder>,
    },
    /// Root-authorized person adoption of one live task, with its reservation moved to that person's pool.
    Move {
        reply_to: ReplyTo,
        task: u64,
        person: u64,
        period: u64,
        pool_budget: u64,
        period_budget: u64,
        reason: Box<[u8]>,
    },
    /// Install one root-numbered standing interest for a current task.
    Subscribe { reply_to: ReplyTo, task: u64, subscription: Subscription },
    /// Root-authorized connector topic interest with the same durable task subscription.
    SubscribeTopic { reply_to: ReplyTo, task: u64, subscription: Subscription },
    /// Remove one interest owned by a current task.
    Unsubscribe { reply_to: ReplyTo, task: u64, subscription: u64 },
    /// Root places a merged state/timer/connector hint in a subscribed inbox.
    Notice { task: u64, word: Word },
    /// Give two live tasks referenced by the introducer reciprocal references.
    Introduce { reply_to: ReplyTo, by: u64, left: u64, right: u64 },
    /// Admit authenticated person words to a live chat and wake or relay after their commit.
    Message { reply_to: ReplyTo, project: u32, task: u64, word: Word },
    /// A person's words whose project standing was admitted by the core; retains that actor.
    AuthorizedMessage { reply_to: ReplyTo, project: u32, task: u64, word: Word },
    /// Root delivers a just-ended delegate's result to its task requester in
    /// the same decision that archived the delegate.
    DelegateResult { task: u64, word: Word },
    /// Root verified a newly named historical input against its ended store row.
    RememberStub { stub: Stub },
    /// Root `SetRoles` preflight: inspect only current person-requested `Waiting`
    /// contexts without mutation; one terminal even before readiness.
    InspectEscalations {
        /// Stage-local root right consumed once by `EscalationsInspected`
        reply_to: ReplyTo,
        /// Root-validated project whose current `Waiting` tasks are inspected.
        project: u32,
    },
    /// Root after successful `ApplyRoles` in the same synchronous decision; emit
    /// existing `EscalationNeeded` for `Waiting` only and one completion.
    RecheckEscalations {
        /// Stage-local root right consumed by `EscalationsRechecked`
        reply_to: ReplyTo,
        /// Same preflight project; rejected/currently unheld tasks stay inert
        project: u32,
    },
    /// Root resolves the just-created routing obligation before commitment;
    /// stale notification changes nothing.
    RoutedEscalation {
        /// Held person chat.
        task: u64,
        /// Positive semantic revision.
        revision: u64,
        /// Root-verified eligible requester or final policy role.
        holder: crate::EscalationHolder,
        entry: u64,
    },
    /// Root has authenticated the current recipient and checked authority;
    /// one `EscalationDecided` terminal follows.
    DecideEscalation {
        /// Root-owned correlation owed one semantic terminal.
        reply_to: ReplyTo,
        task: u64,
        /// Waiting revision, never a root transport receipt.
        revision: u64,
        /// Positive authenticated deciding person.
        by: u64,
        /// Root-numbered next holder entry on pass only.
        entry: Option<u64>,
        /// Bounded authorized release/reject/pass choice.
        decision: crate::EscalationDecision,
    },
    /// The period source supplied a next boundary; no cadence is inferred.
    /// Update its current ledger, preserve the original opening wall, and ignore stale notices.
    PeriodNext { project: u32, period: u64, next: Option<Wall> },
    /// `Open` one finite monotonic project period after root authorization; emits ledger `Save`
    /// plus `Done`, or one `Refused` with no mutation.
    OpenPeriod {
        /// Root-issued destination owed one `Done` or `Refused` terminal.
        reply_to: ReplyTo,
        project: u32,
        /// Fresh period identity, strictly greater than retained periods for this project.
        period: u64,
        /// Finite root-authorized deployment-unit budget; source capacity is checked before
        /// mutation.
        budget: u64,
    },
    /// Reserve one finite person's pool from its original period after root authorization; emits
    /// both ledger Saves plus `Done`, or `Refused` atomically.
    CarvePool {
        /// Root-issued destination owed one `Done` or `Refused` terminal.
        reply_to: ReplyTo,
        project: u32,
        /// Person whose current role and allotment root authenticated and checked.
        person: u64,
        /// Existing original project period whose available budget is reserved for this pool.
        period: u64,
        /// Complete finite pool budget, atomically reserved from the original period.
        budget: u64,
    },
    /// Post the connector's maximum price in the effect's deciding commit.
    /// Root already checked authority and the current funding balance.
    ChargeEffect { funder: Funder, maximum: u64 },
    /// Change an existing current-period person's pool while preserving reservations and spending.
    ResizePool { reply_to: ReplyTo, project: u32, person: u64, period: u64, budget: u64 },
    /// Change the open period's budget after root authorization, its reservations and
    /// spending kept (domain/authority.md, 7); emits the ledger's `Save` plus `Done`, or one
    /// `Refused` with no mutation.
    ResizePeriod { reply_to: ReplyTo, project: u32, period: u64, budget: u64 },
    /// Admit the next contiguous turn, offered inbox read and checked expense delta atomically;
    /// stale turns/attempts and arithmetic failures refuse without debit.
    Turn {
        /// Root-issued destination owed one `TurnAcknowledged` or `Refused` terminal.
        reply_to: ReplyTo,
        /// `Live` task whose claimed attempt and next turn are being admitted.
        task: u64,
        /// Nonzero current attempt fence; stale attempts cannot charge or advance turns.
        attempt: u64,
        /// Exactly the next contiguous nonzero turn in this attempt; root handles exact replay
        /// before this child input.
        turn: u32,
        /// Highest committed inbox message read by this turn; taking is atomic with the charge.
        read: Option<u64>,
        /// Offered messages still in the inbox that this turn refuses.
        refused: Box<[u64]>,
        /// Highest message this exact run was offered by its assignment or committed relay.
        offered: Option<u64>,
        /// Exact unconsumed offers authenticated by root for this attempt, at most `run_inbox`.
        outstanding: Box<[u64]>,
        /// Whole priced attempt expense; tasks posts only the checked delta above `run_spent`
        /// from this attempt's reserved run allowance.
        cumulative: u64,
    },
    /// Root-validated answer refusals, kept before its activation in the same decision.
    /// Stale attempts or names outside the offered inbox are ignored.
    Refusals { task: u64, attempt: u64, refused: Box<[u64]>, offered: Option<u64> },
    /// Create an authorized whole batch and reservations atomically, producing `Made` or
    /// `Refused` plus bounded persistence/lifecycle outputs.
    Make {
        /// Root-issued destination owed one `Made` or `Refused` terminal for the whole batch.
        reply_to: ReplyTo,
        /// Root-verified creator/requester of every `new` task; task creator and `funding`
        /// ancestry are checked separately.
        creator: Party,
        /// Owned nonempty batch bounded by `Limits::batch`; all graph, payload and reservation
        /// checks precede mutation.
        batch: Box<[New]>,
    },
    /// Admit the accepted batch of a result-created proposal while its proposer closes.
    MakeResultFollowups {
        /// Destination owed one `Made` or `Refused` terminal for the whole batch.
        reply_to: ReplyTo,
        /// The closing task whose result made this proposal.
        proposer: u64,
        /// Exact still-pending result proposal being accepted in this decision.
        proposal: u64,
        /// Whole authorized batch with its final funding source.
        batch: Box<[New]>,
    },
    /// `Due`-to-`Preparing` admission, producing `Done` or `Refused`.
    Prepare {
        /// Destination owed one `Done` or `Refused` terminal.
        reply_to: ReplyTo,
        /// `Live` `Due` task to enter `Preparing`; other phases refuse without mutation.
        task: u64,
    },
    /// `Preparing`-to-`Claimed` admission with a strictly increasing root attempt, producing `Done`
    /// or `Refused`.
    Claim {
        /// Destination owed one `Done` or `Refused` terminal.
        reply_to: ReplyTo,
        /// `Live` `Preparing` task whose attempt is being claimed.
        task: u64,
        /// Root-issued nonzero identity strictly greater than the latest claim; success resets
        /// attempt turn/expense baselines.
        attempt: u64,
        /// Whether the prepared run includes an earlier committed transcript.
        resumed: bool,
        /// Root-checked allowance reserved from the task at this claim.
        budget: u64,
        /// Checked main account; success keeps it in this same claim decision.
        account: Option<u32>,
        /// Complete set of held resources this run will write, in one atomic claim.
        writes: Box<[Name]>,
    },
    /// A connector has read a lost run's resource afresh; its slot may now be freed.
    ReadAfresh { resource: Name },
    /// Claim the writer slot for a connector-owned effect until it settles.
    EffectInFlight { reply_to: ReplyTo, task: u64, resource: Name, entry: u64 },
    /// A connector-owned effect's terminal frees its matching writer slot.
    EffectSettled { resource: Name, entry: u64 },
    /// Notification changing only the matching `Claimed` attempt to `Running`; absent/stale inputs
    /// are ignored and no reply is owed.
    Started {
        /// `Live` claimed task the root reports as started.
        task: u64,
        /// Matching claimed attempt; absent tasks and stale phases/attempts are ignored.
        attempt: u64,
    },
    /// Admit a root-fenced activation terminal with explicit expense provenance; emits one
    /// `Acknowledged` or `Refused` and atomic resulting saves.
    Activation {
        /// Destination owed one `Acknowledged` or `Refused` terminal; root delays exposure until
        /// the decision is durable.
        reply_to: ReplyTo,
        /// `Live` task whose activation terminal is admitted.
        task: u64,
        /// Root-fenced current attempt, including a stopped run preserved while held or closing.
        attempt: u64,
        /// Bounded root-supplied activation outcome; tasks normalizes invalid outcomes. A
        /// park/retry need not end the task.
        end: End,
        /// Full set of resource names with saved state; absent preserves the previous set.
        saved: Option<Box<[SavedResource]>>,
        /// `Priced` worker cumulative expense or unpriced topology/readiness cause; root owns exact
        /// replay fencing.
        cause: Cause,
    },
    /// Notification pausing only a `Preparing` task; does not consume a failure try and has no
    /// reply.
    /// Preparation became stale or waits for an account/host; return to due without a retry timer.
    PreparationWaiting { task: u64 },
    PreparationFailed {
        /// `Preparing` task to pause without consuming an activation failure try; absent or other-
        /// phase tasks are ignored.
        task: u64,
    },
    /// An unread transcript fails only a preparing task as transient, under its retry policy.
    PreparationUnread { task: u64 },
    /// Authorized notification preserving prior state and issuing `Stop` for any live run;
    /// absent tasks and unchanged reasons have no output.
    Hold {
        /// `Live` task to hold while retaining its prior phase; a changed reason
        /// replaces the current hold, and advances its revision.
        task: u64,
        /// Authorized or root-classified hold reason; tasks judges no policy here.
        why: Hold,
    },
    /// Connector-classified hold with its exact opaque resource or effect identity.
    HoldFrom { task: u64, why: Hold, source: Box<HoldSource> },
    /// Root notification that prior effects settled; matching `Effects` stages advance.
    EffectsSettled {
        /// Closing task whose prior effects have settled.
        task: u64,
    },
    /// A connector kept one failed task's resource for its tree root until that root closes.
    Retained { task: u64, root: u64, holding: Holding },
    /// Root notification after cleanup finishes; matching `Releases` stages advance,
    /// including held prior closing, without lifting a hold.
    Settled {
        /// Task whose root-owned cleanup obligations settled; only `Releases` stages advance,
        /// including preserved closing while held.
        task: u64,
    },
    /// Root numbers the news requested by the due person-task timer.
    PersonStallNotice { task: u64, message: u64 },
    /// Root numbers the notice that a kept task goes on (domain/tasks.md, 5.7 and 8.1).
    MetNotice { task: u64, dependency: u64, removed: bool, by: Party, message: u64 },
    /// A root-authenticated inbox answer reaches the credit owner in this decision.
    PersonQuestionAnswer {
        reply_to: ReplyTo,
        project: u32,
        asked: u64,
        person: u64,
        question: u64,
        word: Word,
        notice: u64,
    },
    /// Root numbers the notice consuming an ended question's reserved answer room.
    QuestionEndedNotice { task: u64, notices: Box<[QuestionNotice]> },
    /// A caller cancels its own unanswered question.
    CancelQuestion { reply_to: ReplyTo, task: u64, question: u64 },
    /// Admit one `Live`/`Ledger` row during restoring; invalid input permanently fails restoration
    /// with `RestoreRefused`.
    Restore {
        /// One bounded `Live` or `Ledger` row while restoring; historical `Ended` and
        /// invalid/duplicate rows fail restoration.
        record: Stored,
    },
    /// From the parent, once every record its checks read is restored,
    /// the connectors' among them, and before the store switches
    /// (domain/engine.md, 6): the whole set's checks, its links, stubs,
    /// writers, waits and funders. A broken set fails the start; a
    /// waiter left on a circle of waits is held, its writes landing in
    /// the copy (tasks.md, 6.7).
    Check,
    /// Replay bounded activations/adoptions/closing outputs after `Check` passed.
    /// Before `Check`, refuses as out of order. Repetition after ready/failed is ignored.
    Restored,
}

/// Tasks-to-root persistence, lifecycle and terminal reply outputs; root groups resulting
/// saves/erases with effects and delays outward replies until durability. (domain/tasks.md, section 5).
#[derive(PartialEq, Eq, Debug)]
pub enum Request {
    /// Placed hold lifted; count of tasks released from their held phase.
    Lifted { reply_to: ReplyTo, released: u32 },
    /// The selected live agent tasks moved to the newest, and those that could not move.
    MovedToNewest { moved: u32, stayed: u32 },
    /// The last live holder of this settings revision ended in this step.
    Released { revision: u32 },
    /// Nothing was made: the first failed operation and its owner diagnostic.
    CompoundRefused { reply_to: ReplyTo, at: u32, problem: Problem },
    /// No part of this procedure step changed task state.
    ProcedureRefused { task: u64, step: u64, problem: Problem },
    /// The complete requested set became held in this commit.
    Taken { task: u64, holdings: Box<[Holding]> },
    /// One task entered or changed position in a resource's waiting queue.
    Waiting { task: u64, resource: Name, place: u32 },
    /// A writer slot was busy; the caller may pause and retry without a failed attempt.
    WriterWaiting { reply_to: ReplyTo, task: Option<u64>, resource: Name },
    /// Terminal for one admitted person-origin goal proposal.
    PersonProposed { reply_to: ReplyTo, proposal: u64 },
    /// Terminal for one person-origin goal decision.
    PersonProposalDecided { reply_to: Option<ReplyTo>, proposer: u64, number: u64, outcome: crate::ProposalOutcome },
    /// The core procedure needs one whole root-numbered template batch in this period.
    RecurringDue { task: u64, period: u64, members: u32 },
    /// A changed requester tree asks root to recheck the current proposal recipient from the nearest holder.
    ProposalRerouteNeeded { proposer: u64, proposal: u64 },
    /// Root supplies one message number for the elapsed person-task wait.
    PersonStallDue { task: u64 },
    /// Root supplies one message number for the notice that a kept task goes on (domain/tasks.md, 5.7 and 8.1).
    MetDue { task: u64, dependency: u64, removed: bool, by: Party },
    /// A question ends unanswered; its reserved answer slot becomes its notice.
    QuestionEndedDue { task: u64, questions: Box<[u64]> },
    /// One nonfinal held decision passed its configured wait.
    EscalationStalled { task: u64, revision: u64, holder: crate::EscalationHolder },
    /// One nonfinal holder's wait bound elapsed; root chooses the next holder.
    ProposalStalled { proposer: u64, proposal: u64, holder: crate::ProposalHolder },
    /// Exact semantic proposal decision terminal for a named call or keyed person request.
    ProposalDecided { reply_to: ReplyTo, proposer: u64, number: u64, outcome: crate::ProposalOutcome },
    /// Root allocates a commit-order message number for this subscribed state change.
    Notify { task: u64, subscription: u64, target: u64, state: NoticeState, words: Box<[u8]> },
    /// Root allocates a commit-order message number for this due timer.
    Timer { task: u64, subscription: u64 },
    /// An ending task releases a connector topic interest in the same decision.
    EndTopic { task: u64, subscription: u64, connector: u16 },
    /// Accepted person words; root answers the keyed request after the inbox write commits.
    Sent { reply_to: ReplyTo, task: u64, word: Word },
    /// Resolve an unrecoverable outbox entry with the release in one decision.
    ResolveUncertain { task: u64, entry: u64, again: bool },
    /// Root relays this whole committed message through the fleet to the current run.
    Relay { task: u64, attempt: u64, previous: Option<u64>, word: Word },
    /// Terminal read-only `SetRoles` preflight; snapshot is owned transient root
    /// context, not retained state.
    EscalationsInspected {
        /// Echoed stage-local right.
        reply_to: ReplyTo,
        /// At most `Limits::tasks` `Waiting` contexts without owned rejection
        /// reasons; only `NotReady` can refuse.
        result: Result<Box<[crate::EscalationContext]>, Refusal>,
    },
    /// Terminal for `RecheckEscalations` after its existing `EscalationNeeded` outputs; root
    /// completes the keyed role flight after routing them.
    EscalationsRechecked {
        /// Echoed stage-local right, consumed once.
        reply_to: ReplyTo,
        /// Success or `NotReady` before any recheck output
        result: Result<(), Refusal>,
    },
    /// Tasks asks root to resolve its new held person chat in this atomic
    /// decision; no outward notice precedes commitment.
    EscalationNeeded {
        /// Bounded held context; root drops it after routing
        context: Box<crate::EscalationContext>,
    },
    /// One semantic decision terminal; root commits accepted child writes,
    /// its history and people's keyed answer together.
    EscalationDecided {
        /// Echoed root correlation, consumed once.
        reply_to: ReplyTo,
        task: u64,
        revision: u64,
        /// Accepted semantic transition or unchanged refusal.
        outcome: crate::EscalationOutcome,
    },
    /// One successful `Make` terminal reporting the complete batch.
    Made {
        /** Original `Make` destination for its one successful terminal reply. */
        reply_to: ReplyTo,
        /** All `new` task numbers in input order, bounded by `Limits::batch`; no partial creation is reported. */
        tasks: Box<[u64]>,
    },
    /// One reply-bearing entrance/lifecycle/financial refusal; no requested mutation was admitted.
    Refused {
        /** Original reply-bearing event destination, consumed once on refusal. */
        reply_to: ReplyTo,
        /** Admission location/reason; requested mutation was not made. */
        problem: Problem,
    },
    /// One successful `funding`, preparation or claim terminal.
    Done {
        /** Original successful `funding`/preparation/claim destination, consumed once. */
        reply_to: ReplyTo,
    },
    /// One activation-terminal reply; durability and exact transport replay proof belong to root.
    Acknowledged {
        /** Original activation-terminal destination; root durability barrier precedes outward acknowledgement. */
        reply_to: ReplyTo,
        /** Task whose lifecycle terminal was admitted or already recorded. */
        task: u64,
        /** Fenced activation attempt. */
        attempt: u64,
        /** New lifecycle admission or existing last-answer identity; exact payload replay is separately verified by root. */
        accepted: Accepted,
    },
    /// One newly admitted turn reply; root saves its transcript with the charged task state before
    /// outward acknowledgement.
    TurnAcknowledged {
        /** Original `new`-turn destination; root commits transcript and charged task row together before acknowledging. */
        reply_to: ReplyTo,
        /** Task whose turn and expense delta were admitted. */
        task: u64,
        /** Current claimed attempt identity. */
        attempt: u64,
        /** New contiguous turn committed in task state. */
        turn: u32,
        /** Current charged-turn route emits `Accepted::New`; root owns replay acknowledgement without re-entering this route. */
        accepted: Accepted,
    },
    /// Bounded semantic preparation request to root, not a rendered brief or a raw mutable
    /// task peek.
    Activate {
        /** Owned bounded semantic preparation snapshot; root selects the route and drops the snapshot on claim/failure. */
        context: Box<RunContext>,
    },
    /// Ask root/fleet to stop one exact live attempt; its lifecycle terminal is still owed.
    Stop {
        /** Task whose live activation must be stopped by root/fleet. */
        task: u64,
        /** `Exact` live attempt to stop; its terminal remains owed before closing can advance. */
        attempt: u64,
    },
    /// Ask root/fleet to reconcile a restored claim and highest committed turn.
    Adopt {
        /** `Restored` live task whose claim the root must reconcile with fleet. */
        task: u64,
        /** `Restored` committed attempt identity to adopt; no `new` attempt is minted here. */
        attempt: u64,
        /** Highest committed turn in the restored attempt, supplied for fleet reconciliation. */
        kept: u32,
    },
    /// Ask root to settle prior effects, then return `EffectsSettled`; emitted only
    /// after delegates and funded allocations settle.
    Close {
        /** Task whose delegates/financial allocations have settled and root closing obligations must finish. */
        task: u64,
        /** Bounded pending final ending; root uses it to plan cleanup after effect settlement. */
        ending: Ending,
    },
    /// Ask root to release this task's resources after prior effects settle.
    Release { task: u64, ending: Ending },
    /// One newly ended requester-identified notification emitted with the ended record and exact
    /// financial posting; current root exposes person result notices, with no task-requester inbox
    /// route or child delivery credit.
    Ended {
        /** Task removed from the live arena and saved as a historical ended row. */
        task: u64,
        /** Requester identifying this notification; current root consumes person result notices, and tasks retains no delivery credit or inbox. */
        requester: Party,
        /** Bounded final result/reason; emitted with ended/funder writes in one root decision. */
        ending: Ending,
    },
    /// Owned persistence output for the parent's current atomic decision.
    Save {
        /** Owned row joining the root's atomic decision; not an `IO` submission or a durable acknowledgement by itself. */
        record: Stored,
    },
    /// Logical-row removal for the parent's current atomic decision.
    Erase {
        /** Row removal joining the same atomic decision as related saves and outputs. */
        key: Key,
    },
    /// Terminal startup failure; this instance cannot become ready through more
    /// `Restore`/`Restored` inputs.
    RestoreRefused {
        /** Terminal startup validation failure; this domain instance remains unready. */
        problem: Problem,
    },
}

/// Root's expense provenance for a lifecycle terminal; tasks owns authentic delta/accounting
/// admission while root owns exact transport replay proofs.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Cause {
    /// Worker terminal with authentic cumulative expense: preflight lifecycle and eventual
    /// financial arithmetic before admitting the `new` delta together.
    Priced {
        /// Authentic whole attempt expense, monotonic relative to `run_spent` and preflighted
        /// against eventual funding-chain representability.
        cumulative: u64,
    },
    /// Loss, fleet refusal or invalid-answer normalization supplied by root; performs lifecycle
    /// admission without charging and still owes one terminal reply.
    Unpriced,
}

/// A dependency an amendment added to a live task (domain/tasks.md, section 7).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Added {
    pub task: u64,
    /// When the amendment was committed.
    pub at: Wall,
    /// The amendment's maker: a party, or a task amending one below it.
    pub by: Party,
}

/// Owned semantic preparation snapshot emitted to root, bounded by task limits; not a
/// second mutable `TaskRecord` or `funding` ledger. (domain/tasks.md, section 3).
/// (domain/engine.md, sections 7.1 and 9).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RunContext {
    pub ended_by: crate::EndedBy,
    pub llm: Option<crate::LlmChoice>,
    /// Current financial payer; preparations must recheck it before admission.
    pub payer: crate::Payer,
    pub task: u64,
    /// Newest admitted inbox identity, including words already taken.
    pub last_message: u64,
    /// Whole unread words carried into a newly prepared run brief.
    pub inbox: Box<[Word]>,
    /// Current direct children and their lifecycle at preparation time.
    pub delegates: Box<[DelegateState]>,
    /// Dependency identities whose ended results the root loads; those still pending are in
    /// `pending`.
    pub dependencies: Box<[u64]>,
    /// The dependencies an amendment added, with when and by whom, for the brief's account of
    /// what the task waited for (domain/engine.md, section 9).
    pub added: Box<[Added]>,
    /// Pending dependencies keeping the task, the oldest first; a procedure steps knowing them
    /// (domain/tasks.md, 5.7; domain/connectors.md, 6.1).
    pub pending: Box<[u64]>,
    /// Resources whose next activation resumes saved state.
    pub saved: Box<[SavedResource]>,
    /// Last claimed attempt; a task-wide transcript load must not use turns from a later claim.
    pub previous_attempt: u64,
    /// Whether a committed conversation exists across this task's attempts.
    pub ever_turned: bool,
    /// Failure classes seen before this preparation, for the brief's attempt summary.
    pub tries: Tries,
    /// Contract reason given to this next run after an invalid terminal.
    pub invalid_result: Option<InvalidResult>,
    pub project: u32,
    /// Task executor selected by the root.
    pub executor: Executor,
    /// Bounded owned semantic description; not rendered brief bytes.
    pub spec: Spec,
    /// Bounded result contract root includes in the brief.
    pub contract: Contract,
    /// Requester included in preparation semantics.
    pub requester: Party,
    /// `Exact` carried permission value root translates to authority's independent vocabulary.
    pub authority: Authority,
    /// Borrowed-then-copied authentic financial snapshot for this preparation; root does not mutate
    /// or persist it as a shadow ledger.
    pub numbers: Numbers,
}

/// One direct child's current lifecycle in its requester's next brief.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DelegateState {
    pub task: u64,
    pub phase: Phase,
    pub revisions: Revisions,
}

/// Authority and accounting view for a run's delegation call.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DelegationContext {
    pub project: u32,
    pub requester: Party,
    /// A held or closing ancestor cannot take a proposal decision.
    pub deciding: bool,
    pub authority: Authority,
    pub numbers: Numbers,
    pub tasks_left: u32,
}

/// Current connector access, independent of the resource's hold kind.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ResourceAccess {
    /// Owned or participating writes can be checked against grants.
    Writable,
    /// Reads remain available, but writes are forbidden.
    Context,
    /// Neither effects nor holds are available.
    Unavailable,
}

/// A configured bound a run's model call met, and that bound's configured
/// value in its own unit: never a measured size.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Exceeded {
    pub bound: Bound,
    pub value: u64,
}

/// A configured bound on a model call.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Bound {
    /// The model's reasoning, per item or in all: bytes.
    Reasoning,
    /// The model's other replay metadata: bytes.
    Metadata,
    /// The items of the model's answer: a count.
    Items,
    /// The model's answer: bytes.
    Answer,
    /// The tools offered to the model: a count.
    Tools,
    /// The messages and blocks one request carries: a count of both.
    HistoryItems,
    /// The run's retained history, which compaction could not bring under
    /// one of its configured ceilings: on messages, on blocks or on bytes.
    /// The value is that ceiling, and the bound doesn't name its unit.
    History,
    /// The model's window: tokens.
    Window,
    /// The model's declared output ceiling: tokens.
    Output,
}
