//! Durable refusals with facts, never phrases; client/domain/contract.md, 4.
//! These are final answers; busy and signed out are envelopes below this vocabulary.
use super::{AdmissionLimit, Ending, Executor, Grant, HoldReason, Revision};
use alloc::boxed::Box;
use skein_lib::Wall;
/// The authority the engine found lacking.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Lacking {
    /// No policy role may accept this kind of proposal.
    Approver,
    /// The budget needed and left.
    Budget {
        needed: u64,
        left: u64,
    },
    /// The grant missing.
    Grant(Grant),
    /// Only the supplied operation kind is known.
    GrantKind {
        kind: u16,
    },
    /// No valid funding relation supports the request.
    Funding,
    Tool,
    Scope,
    /// A model the request names is beyond authority (domain/authority.md, sections 3 and 5).
    Models,
    Reference,
    ResourceAccess,
    ResourceUnavailable,
    Shape,
    Again {
        required: bool,
    },
    Account,
    Writer,
    Required {
        connector: u16,
        kind: u16,
    },
    Failed {
        connector: u16,
        kind: u16,
    },
    Unguarded {
        connector: u16,
        kind: u16,
    },
    /// The executor kind missing.
    TaskKind {
        executor: Option<Executor>,
    },
    /// Delegation beyond depth.
    Depth {
        needed: Option<u32>,
        allowed: u32,
    },
    /// A deadline beyond authority.
    Deadline {
        asked: Option<Wall>,
        latest: Option<Wall>,
    },
    /// The tasks needed and left.
    Tasks {
        needed: Option<u32>,
        left: u32,
    },
}

/// The admission limit reached.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Limit {
    /// An engine structural admission category, without an invented numerical bound.
    Admission { which: AdmissionLimit },
    /// A project live task limit.
    LiveTasks { project: u32, limit: u32 },
    /// A task inbox limit.
    Inbox { task: u64, limit: u32 },
    /// The words byte limit.
    Words { limit: u32 },
    /// The client field limit.
    Fields,
    /// The client record limit.
    Records,
    /// The client request limit.
    Requests,
    /// The client read limit.
    Reads,
    /// The watch limit.
    Watches,
    /// The client working set limit.
    Objects,
}

/// The current hold facts actually supplied by the owner.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum CurrentHold {
    /// A complete held object's document.
    Document(Box<HoldReason>),
    /// Admission supplies only its exact category and numeric identities.
    Reason(HoldCategory),
}
/// Sparse engine hold facts; absent words, clocks and parts stay absent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HoldCategory {
    /// A configured bound refused the run's model call (domain/hosts.md, 6.3).
    Exceeded(Exceeded),
    Failures {
        class: HoldFailure,
    },
    Stalled,
    Dependency {
        task: u64,
    },
    Stopped,
    Placed {
        hold: u64,
    },
    Drift,
    Effects,
    EffectFailed,
    Uncertain {
        entry: u64,
    },
    Budget,
    Deadline,
    HoldsWaited,
    /// Joining a queue would close a circle of waits (domain/tasks.md, 6.7).
    Circle,
    ResourceUnavailable,
    ResourceAccess,
    PoolLost {
        pool: u64,
    },
    Procedure,
}
/// Exact admission failure categories, independent of detailed attempt failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HoldFailure {
    Transient,
    Permanent,
    Run,
    Agent,
    Lost,
    Invalid,
}

/// Which independently decidable facts changed.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Moved {
    /// The hold or its reason.
    Hold { reason: Option<CurrentHold> },
    /// The task specification.
    Spec,
    /// The person task question or options.
    PersonTask,
    /// The note.
    Note,
    /// The recipient at which the object waits.
    Waiting,
}

/// A durable request refusal and all facts needed to show it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Refusal {
    /// A finish refused while the sign-in may still wait (accounts.md, section 5).
    SignInRedirect {
        why: SignInRedirectWhy,
    },
    Stale(super::RefusalStale),
    Full(super::RefusalFull),
    /// An account move refused (client/domain/contract.md, 4).
    AccountMove {
        why: AccountMoveWhy,
    },
    Circle {
        tasks: skein_lib::List<u64>,
        cut: bool,
    },
    Dependency {
        dependency: u64,
        how: super::OwnerTaskStatus,
    },
    Kept {
        pending: skein_lib::List<u64>,
    },
    /// An explicit account preference cannot fund the prospective task.
    Account {
        account: u32,
        why: super::AccountWhy,
    },
    /// A root task has no escalation holder.
    NoEscalationHolder,
    /// The provider's account use rule refuses this change.
    Provider,
    /// A malformed operation, at its place in the compound.
    Malformed {
        at: u32,
        why: super::Malformed,
    },
    /// The project role does not allow the act.
    Role {
        held: Option<HeldRole>,
    },
    /// Another person holds this role-addressed task.
    Taken {
        by: u64,
    },
    /// The object waits for someone else.
    NotWaiting,
    /// This holder cannot pass further.
    NoFurther,
    /// Authority lacked a named permission or amount; proposal eligibility is supplied when known.
    Authority {
        lacking: Lacking,
        proposable: Option<bool>,
    },
    /// An admission limit was reached.
    Limit {
        which: Limit,
    },
    /// The decided thing changed.
    Moved {
        now: Revision,
        what: Moved,
    },
    /// The object ended.
    Ended {
        how: Ending,
    },
    /// No visible object with that name.
    Unknown,
    /// Another project holds this name.
    NameTaken,
    /// The deployment does not offer this act.
    NotOffered,
    /// The key names another ask.
    KeyConflict,
    /// An LLM choice names a model, by its configured name, that cannot be chosen, and why.
    Llm {
        model: super::Text,
        why: super::LlmWhy,
    },
}

/// The role the engine actually found held, when known.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HeldRole {
    Owner,
    Maintainer,
    Member,
    Observer,
    Policy { role: u32 },
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

/// Why an account move was refused (client/domain/contract.md, 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AccountMoveWhy {
    NotSignedIn,
    NotOurs,
    Held,
    Cancelled,
    Moving,
}

/// Why a finish was refused (client/domain/contract.md, section 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SignInRedirectWhy {
    /// No sign-in of this party's waits for its redirect under that number.
    NotWaiting,
    /// The address does not match the sign-in's redirect or its required fields.
    Address,
    /// The state belongs to another sign-in; this sign-in still waits.
    State,
    /// Too many states from other sign-ins; it still waits for its browser.
    TooMany,
}
