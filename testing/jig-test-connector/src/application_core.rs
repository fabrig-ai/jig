//! Facts returned for a connector's party part (domain/connectors.md, 5.5).
use skein_lib::Wall;

/// A final reason a keyed request was refused.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Refusal {
    /// The held role does not permit this request here.
    Role { held: Option<u32> },
    /// The request lacks the stated authority.
    Authority { lacking: Lack, proposable: Option<bool> },
    /// The stated admission bound was reached.
    Limit { which: LimitKind },
    /// The decided subject changed since it was observed.
    Moved { now: Moved },
    /// The subject ended with the stated outcome.
    Ended { how: Ending },
    /// Another party holds the role-addressed person task.
    Taken { by: u64 },
    /// The proposal or escalation waits for someone else.
    NotWaiting,
    /// The final holder cannot pass further.
    NoFurther,
    /// No visible subject has that name.
    Unknown,
    /// The deployment does not offer this operation.
    NotOffered,
    /// This party's key names another ask.
    KeyConflict,
}

/// Authority missing from a party's request.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Lack {
    /// No policy role may accept this kind of party proposal.
    Approver,
    /// Uncertain effect recovery needs an explicit repeat/skip choice.
    Again { required: bool },
    /// Requested spend exceeds what this source can supply.
    Budget { needs: u64, left: u64 },
    /// No grant covers this operation kind.
    Grant { kind: u16 },
    /// No valid funding relation supports the requested reservation.
    Funding,
    /// The requested executor is not allowed.
    TaskKind,
    /// Descendant depth exceeds this maximum.
    Depth { most: u32 },
    /// The deadline is past or later than permitted.
    Deadline,
    /// Lifetime task capacity cannot supply the request.
    Tasks { left: u32 },
    /// The tool family is absent.
    Tool,
    /// The note scope is absent.
    Scope,
    /// A requested model is absent.
    Models,
    /// No verified reference permits this action.
    Reference,
    /// Current resource access forbids this action.
    ResourceAccess,
    /// A required connector resource is unavailable.
    ResourceUnavailable,
    /// The authority value has an invalid shape.
    Shape,
    /// A required model account is unusable.
    Account,
    /// A written resource is held elsewhere.
    Writer,
    /// A required judge has no usable fact.
    Required { connector: u16, kind: u16 },
    /// A required judge refused the effect.
    Failed { connector: u16, kind: u16 },
    /// The effect cannot guard a required fact.
    Unguarded { connector: u16, kind: u16 },
}

/// The bound a final refusal identifies.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum LimitKind {
    /// The words or other byte payload exceed their bound.
    Words,
    /// The identity payload exceeds its bound.
    Identity,
    /// The roster exceeds its bound or repeats a party.
    Roles,
    /// A checked identity or accounting counter is exhausted.
    Arithmetic,
    /// An identity is repeated in the admitted input.
    Duplicate,
    /// The required input is empty.
    Empty,
    /// The batch exceeds its bound.
    Batch,
    /// The project has no live task capacity.
    ProjectLiveTasks,
    /// The live task set is full.
    LiveTasks,
    /// The task tree exceeds its bound.
    Tree,
    /// The task tree exceeds its depth bound.
    Depth,
    /// The delegate list exceeds its bound.
    Delegates,
    /// The subscription list exceeds its bound.
    Subscription,
    /// The dependency list exceeds its bound.
    Dependencies,
    /// The dependency tree would contain a cycle.
    Cycle,
    /// The task spec exceeds its bounds.
    Spec,
    /// The result contract exceeds its bounds.
    Contract,
    /// The authority value exceeds its structural bounds.
    Authority,
    /// The shown input list exceeds its bound.
    Inputs,
    /// The attempt counter is exhausted.
    Attempts,
    /// The live delegate allowance is exhausted.
    LiveDelegates,
    /// The stored record is invalid or restoration is finished.
    Restore,
    /// The read payload exceeds its bounds.
    Read,
    /// The turn count exceeds its bound.
    Turn,
    /// The resource hold kind is invalid.
    HoldKind,
    /// The resource hold is already taken.
    HoldTaken,
    /// The hold list exceeds its bound.
    Holds,
    /// The inbox has no admission room.
    InboxFull,
    /// The notes set is full.
    Notes,
    /// The revision counter is exhausted.
    Revision,
    /// The question payload exceeds its bounds.
    Question,
    /// The priority batch exceeds its bounds.
    Priority,
    /// The policy payload exceeds its bounds.
    Policy,
    /// The resource payload exceeds its bounds.
    Resource,
    /// The service payload exceeds its bounds.
    Service,
    PlacedHolds,
    ProjectPlacedHolds,
    PartyPlacedHolds,
    Exemptions,
}

/// A party in an ending fact, independent of the tasks child's vocabulary.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Party {
    /// An authenticated person or service.
    Person(u64),
    /// A task cancelled its descendant work.
    Task(u64),
    /// A deployment rule made the decision.
    Deployment { project: u32 },
}

/// The final outcome of a subject that cannot be decided again.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Ending {
    /// The task completed successfully.
    Done,
    /// The task failed.
    Failed,
    /// The named party cancelled it at this wall time.
    Cancelled { by: Party, at: Wall },
    /// The named party decided the immutable subject at this wall time.
    Decided { by: u64, at: Wall },
    /// The corrected or deleted note is no longer present.
    Deleted,
}

/// An unsaved answer: nothing was made and the key may be retried.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Again {
    /// Temporary admission pressure.
    Busy,
    /// Restoration or a required neighbour is not ready.
    NotReady,
}

/// The current revision of a changed decision subject.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Moved {
    /// A hold began anew or its reason changed.
    Hold { revision: u64, reason: HoldReason },
    /// A task's words, parameters, dependencies or policy changed.
    Spec { revision: u64 },
    /// A person task's question, options or shown results changed.
    Person { revision: u64 },
    /// A note changed after it was recalled.
    Note { revision: u32 },
}

/// The reason a task is held, in the party's vocabulary.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum HoldReason {
    /// A configured bound refused the run's model call (domain/hosts.md, 6.3).
    Exceeded(Exceeded),
    /// A queue join would close a circle of waits.
    Circle,
    /// Its retry allowance was exhausted.
    Failures { class: FailureClass },
    /// Its executor stopped progressing.
    Stalled,
    /// An input failed or was cancelled.
    Dependency { task: u64 },
    /// An authorized stop has no named party.
    Stopped,
    /// A separately numbered placed hold keeps the task held.
    Placed { hold: u64 },
    /// A resource drifted away from what the task needs.
    Drift,
    /// Its effects need a decision.
    Effects,
    /// An effect failed permanently.
    EffectFailed,
    /// An effect's result is uncertain.
    Uncertain { entry: u64 },
    /// It lacks budget for its next activation.
    Budget,
    /// Its deadline has passed.
    Deadline,
    /// It waited too long for its resource holds.
    HoldsWaited,
    /// A required resource is unavailable.
    ResourceUnavailable,
    /// Adoption no longer permits its workspace writes.
    ResourceAccess,
    /// Its allocation in a pool disappeared.
    PoolLost { pool: u64 },
    /// Its procedure asked for a decision.
    Procedure,
}

/// The class of failures that exhausted a task's retry allowance.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum FailureClass {
    /// A temporary neighbour or read failure.
    Transient,
    /// A permanent failure awaiting a change outside the engine.
    Permanent,
    /// The executor reported failure.
    Run,
    /// The agent crashed, stalled or broke its channel.
    Agent,
    /// The host disappeared.
    Lost,
    /// The result broke its contract.
    Invalid,
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
    /// Rendered host charter and tool schemas: bytes.
    HostPart,
}
