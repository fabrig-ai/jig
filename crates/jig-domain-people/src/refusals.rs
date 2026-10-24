//! Durable refusal facts and unsaved retry kinds (client/domain/contract.md,
//! section 4). Facts contain only numbers and enums; their owner keeps any words.
use crate::Role;
use alloc::boxed::Box;
use skein_lib::Wall;

/// Maximum named tasks in circle and settings-full refusal records.
pub const NAMED_TASKS: u32 = 32;

/// Who may accept a proposal of a refused request (client/domain/contract.md, 3.5).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Accepter {
    /// No proposal of this request could be accepted.
    Nobody,
    /// The lowest-numbered policy role deciding this kind.
    Role(u32),
}

/// A final reason a keyed request was refused.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Refusal {
    /// A finish refused while the sign-in may still wait (accounts.md, section 5).
    SignInRedirect {
        why: crate::SignInRedirectWhy,
    },
    Stale {
        newest: u32,
    },
    Full {
        revision: u32,
        tasks: Box<[u64]>,
        more: u32,
    },
    /// Another project holds this name under simple case folding.
    NameTaken,
    /// A holder's account move could not proceed (domain/accounts.md, section 6).
    AccountMove {
        why: crate::AccountMoveWhy,
    },
    /// The bounded wait circle in traversal order (tasks.md, 6.7).
    Circle {
        tasks: Box<[u64]>,
        cut: bool,
    },
    /// An added prerequisite already failed or was cancelled (tasks.md, 7).
    Dependency {
        dependency: u64,
        how: DependencyStatus,
    },
    /// An answer is kept by its pending prerequisites (tasks.md, 5.7).
    Kept {
        pending: Box<[u64]>,
    },
    /// An explicit account preference was refused before the task changes.
    Account {
        account: u32,
        why: crate::AccountWhy,
    },
    /// The named batch member is not well formed.
    NotWellFormed {
        at: u32,
        why: Malformed,
    },
    /// The held role does not permit this request here.
    Role {
        held: Option<Role>,
    },
    /// The request lacks the stated authority.
    Authority {
        lacking: Lack,
        /// Who could accept a proposal of this request.
        /// None preserves refusals that make no claim about a proposal.
        proposable: Option<Accepter>,
    },
    /// The stated admission bound was reached.
    Limit {
        which: LimitKind,
    },
    /// The decided subject changed since it was observed.
    Moved {
        now: crate::Moved,
    },
    /// The subject ended with the stated outcome.
    Ended {
        how: Ending,
    },
    /// Another party holds the role-addressed person task.
    Taken {
        by: u64,
    },
    /// The proposal or escalation waits for someone else.
    NotWaiting,
    /// The final holder cannot pass further.
    NoFurther,
    /// No visible subject has that name.
    Unknown,
    /// The deployment does not offer this operation.
    NotOffered,
    /// A plan's uses name another person, directly or through a project
    /// (domain/accounts.md, 4.2; domain/providers.md, 2.6 and 3.4).
    Provider,
    /// This party's key names another ask.
    KeyConflict,
    /// The root's requester has nobody to hold its escalations (domain/people.md, 5.1).
    NoEscalationHolder,
    /// The task's LLM choice names this configured model, refused for `why` (domain/tasks.md, 3).
    Llm {
        model: u8,
        why: crate::LlmWhy,
    },
}

/// Why a batch member is not well formed.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Malformed {
    Settings {
        setting: crate::Setting,
        fault: crate::SettingFault,
    },
    SettingsUnchanged,
    /// Running work is chosen only on accepting a settings proposal.
    Running,
    Name(crate::NameRule),
    /// A proposed batch must name every budget.
    ProposedAsAllowed,
    Cycle,
    AsAllowedTwice,
    Conversation,
    EndedBy,
}

/// Authority missing from a party's request.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Lack {
    /// No policy role may accept this kind of party proposal.
    Approver,
    /// Requested spend exceeds what this source can supply.
    Budget { needs: u64, left: u64 },
    /// No grant covers this operation kind.
    Grant { kind: u16 },
    /// No valid funding relation supports the requested reservation.
    Funding,
    /// The requested executor is not allowed.
    TaskKind { executor: Option<crate::TaskExecutor> },
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
    /// A requested model is absent (domain/authority.md, sections 3 and 5).
    Models,
    /// No verified reference permits this action.
    Reference,
    /// Current resource access forbids this action.
    ResourceAccess,
    /// A required connector resource is unavailable.
    ResourceUnavailable,
    /// The authority value has an invalid shape.
    Shape,
    /// Whether an unrecoverable effect choice was required or unexpected.
    Again { required: bool },
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
    /// The deployment has no room for another pending settings set.
    SettingsProposals,
    /// The deployment has no room for another project.
    Projects,
    /// Empty or too many operations in one compound request.
    Operations,
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
    /// The cancel grace exceeds its span.
    Grace,
    /// The project has no room for another goal.
    Goals,
    /// The policy payload exceeds its bounds.
    Policy,
    /// The resource payload exceeds its bounds.
    Resource,
    /// The service payload exceeds its bounds.
    Service,
    /// The deployment has no room for another LLM account.
    Accounts,
    /// The holder has no room for another LLM account.
    HolderAccounts,
    /// The account, or the sources signing out, have no room for another source.
    Sources,
    /// As many sign-ins are under way as the deployment allows.
    SignIns,
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

/// The terminal phase of an added prerequisite (client/domain/contract.md, 4).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum DependencyStatus {
    Done,
    Failed,
    Cancelled,
}
