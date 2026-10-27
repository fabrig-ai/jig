//! Jig's closed words and typed holes (client/domain/view.md, section 7).
use crate::Owner;
use alloc::boxed::Box;
use skein_lib::List;
use skein_ui::phrases::{HoleKind, HoleShape, PhraseId, PhraseShape};
/// Each sentence jig says, independent of language word order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JigPhrase {
    SettingsChanged,
    MovedToNewest,
    SettingsRestored,
    SettingsStale,
    SettingsFull,
    SettingsFullTask,
    SettingsFullMore,
    SettingsUnchanged,
    MalformedRunning,
    SettingCharter,
    SettingTemplate,
    SettingPolicy,
    SettingRules,
    SettingFaultShape,
    SettingFaultCount,
    SettingFaultReference,
    SettingFaultRules,
    SettingFaultBudget,
    SettingFaultGrace,
    SettingFaultOwner,

    ActRename,
    NameTaken,
    NameEmpty,
    NameLong,
    NameControl,
    NameSpace,
    NameDigits,
    EngineLimitProjects,

    OpenProgramModel,
    OpenProgramUnavailable,
    OpenProgramChooseModel,
    OpenProgramSource,
    OpenProgramNoSwitch,
    RefusedWorkerAt,
    RefusedEngineAt,
    MalformedEndedBy,

    DependencyParked,
    KeptUntil,
    DependencyRemoved,
    DependencyMet,
    DependencyFailed,
    DependencyCancelled,
    DependencyPending,
    DependencyDone,
    CircleWaitsFor,
    CircleHold,
    CircleLifts,
    CircleRefused,
    KeptAnswer,
    DependencyByTask,
    DependencyByParty,
    DependencyByDeployment,

    Word(jig_client::words::Word),
    RequestNotKept,
    AnswersFull,
    RequestUnconfirmed,
    RequestsInFlight,
    RequestsDropped,
    NoticesPart,
    NothingKnown,
    LinkPart,
    NeedsPart,
    PartyPart,
    ProjectsPart,
    GoToTaskPart,

    OwnerHoldRelease,
    QuestionFromTask,
    QuestionFromPerson,
    QuestionFromDeployment,
    CachedReplies,
    TasksIdle,
    TasksStarting,
    TasksBackingOff,

    /// A source page is incomplete; no particular row or continuation is inferred.
    PageCut,
    ArchiveCumulativeSpend,
    ArchiveAcceptedAt,
    ArchiveReadThrough,
    ArchiveReadAbsent,
    /// The stored party writer of a note.
    NoteWriterParty,
    /// The stored task and attempt writer of a note.
    NoteWriterTask,
    /// No changed time was supplied for this note.
    NoteChangedNotSupplied,
    /// Some owner task lists were omitted.
    TaskListsCut,
    /// The actual other goal this task serves: (0).
    ServedGoal,
    ResourceSegments,
    LiteralGrant,
    LiteralGrantMissing,
    ScopedResourceNotes,
    ChipTitleCut,
    ChipDependenciesCut,
    ChipWaitingHolds,
    FrameLogin,
    FramePerson,
    FrameService,
    FrameResultsSeen,
    FrameNeedsKinds,
    FrameProjectsCut,
    FramePoolUnopened,
    PolicyPeriodSpend,
    PolicyEditRole,
    PolicyRequests,
    PolicyDecides,
    PolicyRequirements,
    PolicyProjections,
    PolicyPermissions,
    PolicyEffect,
    PolicyJudge,
    PolicyGuarded,
    PolicyObserved,
    PolicyMustGuard,
    PolicyGuardNotRequired,
    PolicyPermission,

    /// A proposal made by an actual party.
    ProposedByParty,
    /// A task maker's proposal part.
    PartProposalTask,
    /// A party maker's proposal part.
    PartProposalParty,
    /// A proposal number owned by the part's connector.
    PartOwnerProposal,
    /// Executor reported failure.
    FailureRun,
    /// Agent failed.
    FailureAgent,
    /// Result broke its contract.
    FailureInvalid,

    /// Loading navigation.
    LoadingNavigation,
    /// By (0).
    ByNamed,
    /// Refused.
    Refused,
    /// Leaving.
    Leaving,
    /// Live.
    Live,
    /// Offline.
    Offline,
    /// Connecting.
    Starting,
    /// Behind: waiting for current facts.
    Behind,
    /// (0) need you.
    Needs,
    /// More than (0) need you.
    MoreNeeds,
    /// (0) updates not read.
    Updates,
    /// More than (0) updates not read.
    MoreUpdates,
    /// Signed in as (0).
    SignedIn,
    /// Sign out.
    SignOut,
    /// Not signed out: the engine was not reached.
    SignOutLost,
    /// Signed out.
    SignedOut,
    /// Work.
    Me,
    /// Navigation.
    Navigation,
    /// Project: (0).
    Project,
    /// All projects.
    AllProjects,
    /// Go to task number.
    TaskNumber,
    /// Inbox.
    Inbox,
    /// Conversations.
    Conversations,
    /// Project.
    ProjectPage,
    /// Deployment.
    Deployment,
    /// Task (0).
    TaskPage,
    /// Open.
    Open,
    /// Being decided: review the confirmation.
    Confirming,
    /// Being decided: waiting for the engine.
    InFlight,

    /// Being decided: waiting for the engine connection or your sign-in.
    Parked,
    /// Outcome unknown: inspect the current object before acting again.
    Unknown,
    /// Inspect current object.
    LookAgain,
    NoGoalTemplate,
    ProposeInstead,
    MoveGoalDown,
    MoveGoalUp,
    /// Finding how this ended.
    Looking,
    /// Ended; its final facts are no longer available.
    GoneUnknown,
    /// Changed.
    Changed,
    /// Proposed by task (0), proposal (1).
    Proposed,

    /// Failed: (0).
    Failed,
    /// Cancelled: (0).
    Cancelled,
    /// Withdrawn: (0).
    Withdrawn,
    /// By party (0).
    ByParty,
    /// By task (0).
    ByTask,
    /// By the deployment.
    ByDeployment,
    /// At (0).
    At,
    /// Accepted.
    Accepted,
    /// Rejected: (0).
    Rejected,
    /// Passed up.
    PassedUp,
    /// Released.
    Released,
    /// Released: make the effect again.
    ReleasedAgain,
    /// Released: do not make the effect again.
    ReleasedWithout,
    /// Left held: (0).
    LeftHeld,
    /// Decided first by (0).
    DecidedFirst,
    /// Made task (0).
    MadeTask,
    /// Made service party (0).
    MadeService,
    /// Answered person task (0).
    MadeResult,
    /// Made note (0).
    MadeNote,
    MovingOut,
    /// Project (0): requires role (1).
    RoleMissing,
    /// Your pool has (0) left; this needs (1).
    BudgetMissing,
    /// Requires effect kind (0) on connector (1), resource (2).
    GrantMissing,
    /// Delegation needs depth (0); allowed (1).
    DepthMissing,
    /// Deadline (0) exceeds latest allowed (1).
    DeadlineMissing,
    /// Needs (0) tasks; (1) left.
    TasksMissing,
    /// Agent.
    Agent,
    /// Procedure.
    Procedure,
    /// Person.
    Person,
    /// Person holding a role.
    RoleExecutor,
    /// Executor kind not allowed.
    KindMissing,
    /// Project (0): live task limit (1).
    LimitLive,
    /// Task (0): inbox limit (1).
    LimitInbox,
    /// Words exceed (0) bytes.
    LimitWords,
    /// Conversation word snapshot limit (0).
    LimitConversation,
    /// Field limit reached.
    LimitFields,
    /// Kept record limit reached.
    LimitRecords,
    /// Request limit reached.
    LimitRequests,
    /// Read limit reached.
    LimitReads,
    /// Watch limit reached.
    LimitWatches,
    /// Working set limit reached.
    LimitObjects,
    /// The hold or its reason changed.
    MovedHold,
    /// The specification changed.
    MovedSpec,
    /// The question or its options changed.
    MovedPerson,
    /// The note changed.
    MovedNote,
    /// The recipient changed.
    MovedWaiting,
    /// Current revision (0).
    Revision,
    /// (0).
    Amount,
    /// Waiting (0) min.
    WaitingSpan,
    /// (0) min.
    Span,
    /// Object.
    Object,
    /// Proposal.
    Proposal,
    /// Escalation.
    Escalation,
    /// Question.
    Question,
    /// Person task.
    PersonTask,
    /// Result.
    Result,
    /// Note.
    Note,
    /// In project (0).
    ContextProject,
    /// Task (0).
    ContextTask,
    /// Task (0): (1).
    ChipNumber,
    /// Wakes on any news.
    WakeAny,
    /// Wakes at the next scheduled time.
    WakeAt,
    /// Wakes on a delegate result.
    WakeResult,
    /// Wakes on subscribed news.
    WakeTopic,
    /// Wakes on inbox messages.
    WakeInbox,
    /// Attempt (0).
    Attempt,
    /// Retrying at (0): try (1) of (2).
    RetryAt,
    /// Host lost.
    HostLost,
    /// Temporary failure.
    Temporary,
    /// Permanent failure.
    Permanent,
    /// Stalled.
    Stall,
    /// Reason: (0).
    Reason,
    /// Closing: (0) delegates.
    Closing,
    /// Conjunction before a connector name in a closing phase.
    ClosingConnector,
    /// The named connector owns the effects being settled.
    ClosingEffects,
    /// No policy recipient may accept this proposal.
    NobodyMayAccept,
    /// Missing proposal approval authority.
    ApprovalAuthorityMissing,
    /// Waiting for party (0).
    WaitingParty,
    /// Waiting for task (0).
    WaitingTask,
    /// Waiting for project (0), role (1).
    WaitingRole,
    /// Since (0).
    Since,
    /// From this pool.
    Pool,
    /// Why this is proposed.
    ProposalWhy,
    /// The accepted tasks will be yours.
    OwnTasks,
    /// Tasks acceptance would make.
    Batch,
    /// Proposed task (0).
    BatchTask,
    /// After proposed task (0).
    AfterTask,
    /// Budget (0).
    Budget,
    /// Before.
    Before,
    /// After.
    After,
    /// Delegation depth (0).
    Depth,
    /// Tasks allowed (0).
    TasksAllowed,
    /// Effect kind (0) on connector (1), resource (2).
    Grant,
    /// Tool (0) allowed.
    ToolAllowed,
    /// Passed by this holder.
    PassedBy,
    /// Widen authority.
    Widen,
    /// Amend task (0).
    AmendTask,
    /// Effect.
    Effect,
    /// Your answer.
    AnswerField,
    /// State to approve: (0).
    ApprovalState,
    /// Choose an option.
    Options,
    /// Cannot do it.
    CannotDo,
    /// Taken by (0).
    TakenBy,
    /// Becomes news for its requester after (0) min.
    StallSpan,
    /// Verdict (0).
    Verdict,
    /// Choice (0), by (1).
    ChoiceResult,
    /// Approved (0), by (1).
    ApprovalYes,
    /// Did not approve (0), by (1).
    ApprovalNo,
    /// Open at first reply not read: (0).
    FirstUnread,
    /// Reply.
    Reply,
    /// Loading connector content.
    LoadingPart,
    /// Loading object.
    LoadingObject,
    /// Write.
    ActWrite,
    /// Answer.
    ActAnswer,
    /// Take it.
    ActTake,
    /// Hand it back.
    ActHandBack,
    /// Accept.
    ActAccept,
    /// Reject.
    ActReject,
    /// Pass up.
    ActPassUp,
    /// Release.
    ActRelease,
    /// Leave it held.
    ActLeaveHeld,
    /// Amend.
    ActAmend,
    /// Amend and release.
    ActAmendRelease,
    /// Withdraw the party's pending proposal.
    ActWithdraw,
    /// Stop.
    ActStop,
    /// Cancel.
    ActCancel,
    /// Close.
    ActClose,
    /// Take over.
    ActTakeOver,
    /// Set goal.
    ActSetGoal,
    /// Reorder.
    ActReorder,
    /// Mark read.
    ActMarkSeen,
    /// Edit note.
    ActEditNote,
    /// Delete note.
    ActDeleteNote,
    /// Adopt resource.
    ActAdopt,
    /// Change policy.
    ActChangePolicy,
    /// Set roles.
    ActSetRoles,
    /// Make service.
    ActMakeService,
    /// Set pool.
    ActSetPool,
    /// Chat about it.
    ChatAbout,
    /// Subscribed topic (0).
    Topic,
    /// For (0) min.
    ElapsedSpan,
    /// Your reason.
    ReasonField,
    /// Loading phase details.
    LoadingPhase,
    /// Deadline (0).
    Deadline,
    /// Held: stalled on (0).
    StalledOn,
    /// Held: dependency (0) ended unsuccessfully.
    DependencyEnded,
    /// Held: the system changed.
    DriftHold,
    /// Held: the effect failed for good.
    EffectFailedHold,
    /// Held: the effect may or may not have been made; inspect the system before deciding.
    EffectUncertainHold,
    /// Held: spent (0), budget (1).
    BudgetHold,
    /// Held: past deadline (0).
    DeadlineHold,
    /// Held: stopped by (0) at (1).
    StoppedHold,
    /// Held: waiting for a resource past its bound.
    ResourceHold,
    /// Held: the pool lost its allocation.
    PoolHold,
    /// Held: the procedure needs a decision.
    ProcedureHold,
    /// Loading task (0) phase facts.
    LoadingDependency,
    /// Before the hold.
    BeforeHold,
    /// Task (0), attempt (1): spent (2).
    SpentRun,
    /// Task (0): spent (1).
    SpentDelegate,
    /// Review before confirming.
    Confirmation,
    /// Confirm.
    Confirm,
    /// Dismiss.
    Dismiss,
    /// This will be proposed to its holder.
    WillPropose,
    /// The revision this action names changed; inspect the current facts and open it again.
    ChangedConfirmation,
    /// Required words are missing.
    MissingWords,
    /// Choose an option.
    MissingChoice,
    /// Choose whether to make the effect again.
    MissingAgain,
    /// Supply the amendment.
    MissingChange,
    /// Choose what becomes of every live delegate.
    MissingDelegates,
    /// Supply a priority.
    MissingPriority,
    /// Waiting for this object’s project context.
    MissingContext,
    /// Acceptance makes or changes the following, funded as shown.
    WillAccept,
    /// Reject this proposal and send its proposer your reason.
    WillReject,
    /// Pass this decision to the holder above.
    WillPassUp,
    /// Remove this escalation from your inbox; the task stays held with your reason.
    WillLeaveHeld,
    /// Stop the live run and hold its task.
    WillStop,
    /// Cancel this task and everything below it; already made effects remain.
    WillCancel,
    /// Close this chat after disposing of every live delegate; pending effects settle.
    WillClose,
    /// Become this task’s requester and move its funding.
    WillTakeOver,
    /// Take this role task so others cannot answer it at once.
    WillTake,
    /// Hand this task back to its role.
    WillHandBack,
    /// Send these words to the task.
    WillWrite,
    /// Send the answer matching the task’s contract.
    WillAnswer,
    /// Replace the named specification with these changes.
    WillAmend,
    /// Amend and release in one decision.
    WillAmendRelease,
    /// Change this goal’s priority.
    WillPrioritise,
    /// Replace the named note revision.
    WillEditNote,
    /// Delete this note revision.
    WillDeleteNote,
    /// Make a goal from these words and options.
    WillGoal,
    /// Releasing carries on closing with the kept result.
    ContinueClosing,
    /// Release to wait again.
    ReleaseWait,
    /// Release with procedure counts reset.
    ReleaseProcedure,
    /// Make the effect again.
    RepeatEffect,
    /// Release without making the effect again; tell its executor so.
    SkipEffect,
    /// Cancel delegate (0) and its subtree.
    DelegateCancel,
    /// Take over delegate (0), reserving (1).
    DelegateTake,
    /// Choose each delegate’s outcome.
    DelegateChoices,
    /// Write and release together.
    WriteAndRelease,
    /// Write without releasing.
    WriteOnly,
    /// New specification.
    NewSpec,
    /// New instructions.
    NewInstructions,
    /// No instruction text was supplied by the document owner.
    InstructionNotSupplied,
    /// Supplied complete instructions contain no text.
    InstructionEmpty,
    /// New budget.
    NewBudget,
    /// New deadline (UTC, YYYY-MM-DD HH:MM).
    NewDeadline,
    /// Drop dependency (0).
    DropDependency,
    /// New goal priority.
    PriorityField,
    /// Note description.
    NoteDescription,
    /// Note body.
    NoteBody,
    /// This text value is incomplete.
    DocumentTextCut,
    /// The named independent person-task revision (0).
    PersonAddressRevision,
    /// No completion wall was supplied by the owner.
    ResultTimeUnknown,
    /// Exact owner change result kind (0).
    ChangeResultKind,
    /// This note reference list is incomplete.
    NoteReferencesCut,
    /// Charter (0).
    Charter,
    /// Connector (0), procedure code (1), supplied by the goal bounds.
    CreateProcedure,
    /// Priority (0).
    Priority,
    /// Notice.
    Notice,
    /// Outcome unknown for this action; inspect its current facts.
    UnknownAction,
    /// Results through (0).
    ResultsThrough,
    /// Chat (0), replies through (1).
    RepliesThrough,

    /// Address to party number.
    AddressParty,
    /// Address to task number.
    AddressTask,
    /// Address to role number in this project.
    AddressRole,
    /// Needs you.
    NeedsYou,
    /// Updates.
    InboxUpdates,
    /// All kinds.
    AllKinds,
    /// All goals.
    AllGoals,
    /// Goal (0).
    OnlyGoal,
    /// New.
    NewUpdate,
    /// For the person on call.
    OnCall,
    /// Passes up at (0).
    PassesUp,
    /// Read earlier.
    ReadEarlier,
    /// Read later.
    ReadLater,
    /// Start of the list.
    WindowStart,
    /// End of the list.
    WindowEnd,
    /// More entries are available.
    MoreRows,
    /// First words.
    NewChat,
    /// Start a conversation.
    BeginConversation,
    /// Live delegates: (0).
    Delegates,
    /// Sign in.
    SignInPage,
    /// Sign in with (0).
    SignInWith,
    /// Page missing.
    MissingPage,
    /// Client out of date: update or reload this client to continue; pending requests are kept.
    UpdateClient,
    /// Someone else's work: sign in as its owner to continue.
    SomeoneElse,
    /// Agent spend: (0).
    OwnSpend,
    /// Delegate spend: (0).
    DelegateSpend,
    /// Overview.
    Overview,
    /// Activity.
    Activity,
    /// Plan.
    Plan,
    /// History.
    History,
    /// Budget and authority.
    BudgetAuthority,
    /// Connectors.
    Panels,
    /// Goal.
    Goal,
    /// Resources.
    Resources,
    /// Policy.
    Policy,
    /// Roles and services.
    Roles,
    /// Spend.
    Spend,
    /// Recurring tasks.
    Recurring,
    RecurringKey,
    RecurringSkip,
    RecurringWait,
    RecurringLastPeriod,
    RecurringPendingPeriod,
    RecurringCut,
    /// A supplied next run wall time.
    NextRun,
    /// The source has not supplied the next period wall time.
    NextPeriod,
    /// A collective policy holder in project (0), followed by every supplied role.
    WaitingRoles,
    /// The independently named hold revision (0).
    HoldRevision,
    /// The engine's current period number.
    CurrentPeriod,
    /// When its retained period ledger opened.
    PeriodBegan,
    /// Settings.
    Settings,
    /// Goals.
    Goals,
    /// Tasks.
    Tasks,
    /// Notes.
    Notes,
    /// Reserved: (0).
    Reserved,
    /// Left: (0).
    Left,
    /// Total budget: (0).
    TotalBudget,
    /// Own spend: (0).
    OwnBudget,
    /// Ended delegate spend: (0).
    EndedBudget,
    /// Held below: (0).
    HeldBelow,
    /// Fold subtree.
    FoldSubtree,
    /// Open subtree.
    OpenSubtree,
    /// All plan tasks.
    AllPlan,
    /// Held only.
    HeldPlan,
    /// Live only.
    LivePlan,
    /// This task only.
    OwnHistory,
    /// This task and its subtree.
    SubtreeHistory,
    /// Made in batch (0).
    BatchMade,
    /// Requester changed.
    MovedWork,
    /// Goal specification.
    GoalWords,
    /// Goal budget.
    GoalBudget,
    /// Goal deadline (UTC, YYYY-MM-DD HH:MM).
    GoalDeadline,
    /// Goal priority.
    GoalPriority,
    /// Priority orders work competing for one resource.
    PriorityMeaning,
    /// Your pool has left: (0).
    PoolLeft,
    /// Beyond your pool, this will be proposed.
    GoalPropose,
    /// Search notes by description.
    NoteSearch,
    /// Tasks I asked for.
    AskedByMe,
    /// All tasks.
    AllTasks,
    /// Period (0).
    Period,
    /// Role (0).
    RoleNumber,
    /// Party (0).
    PartyNumber,
    /// Allotment: (0).
    Allotment,
    /// Decides kind (0).
    DecidesKind,
    /// Spend per period: (0).
    PolicySpend,
    /// Host (0).
    HostNumber,
    /// Connected.
    HostConnected,
    /// Lost.
    HostDisconnected,
    /// Slots used: (0).
    SlotsUsed,
    /// Slots free: (0).
    SlotsFree,
    /// LLM account (0).
    AccountNumber,
    /// Usable.
    AccountUsable,
    /// Unavailable.
    AccountUnavailable,
    /// Capacity.
    Capacity,
    /// Live tasks: (0).
    TasksLive,
    /// Task limit: (0).
    CapacityLimit,
    /// Grace holds until (0).
    Grace,
    /// Tasks waiting for this account.
    WaitingAccount,

    /// Resource scope: (0).
    ResourceScope,
    /// Result expected.
    ResultExpected,
    /// A report.
    ReportExpected,
    /// Words.
    WordsExpected,
    /// Approval of the named state.
    ApprovalExpected,
    /// Pending proposals: (0).
    ProposalsWaiting,
    /// Conversation.
    ConversationLog,
    /// Being sent.
    WordsBeingSent,
    /// Sent.
    WordsSent,
    /// Read.
    WordsRead,
    /// Resumed its conversation.
    RunResumed,
    /// Started fresh; may have lost earlier detail. Tail events: (0).
    RunFreshTail,
    /// Run finished.
    RunFinished,
    /// Run cancelled.
    RunCancelled,
    /// Input tokens: (0).
    TurnInput,
    /// Output tokens: (0).
    TurnOutput,
    /// Cache tokens read: (0).
    CacheRead,
    /// Cache tokens written: (0).
    CacheWritten,
    /// Turn spent: (0).
    TurnSpent,
    /// Provisional.
    Provisional,
    /// Some turn content did not fit.
    TurnUnreadVersion,
    TurnUnreadMalformed,
    TurnUnreadTooLarge,
    TurnCut,
    /// Uninterpreted block: (0).
    OpaqueDialect,
    /// Opaque bytes: (0).
    OpaqueBytes,
    /// Call (0).
    CallNumber,
    /// Open input and result for (0).
    OpenCall,
    /// Fold input and result for (0).
    FoldCall,
    /// Call failed.
    CallFailed,
    /// Call completed.
    CallCompleted,
    /// Call in progress.
    CallPending,
    /// News topic (0).
    NewsTopic,
    /// Turn (0).
    TurnNumber,
    /// Instructions.
    TurnInstructions,
    PanelPart,
    ResultPart,
    HoldPart,
    OutboxEntry,
    TakenByNumber,
    RunIdle,
    RunDue,
    RunPreparing,
    RunClaimed,
    RunOwnerActive,
    RunBackingOff,
    RunPhaseSince,
    RunPlacement,
    RunUnobserved,
    RunObservedSince,
    RunReportedAt,
    RunActualStart,
    RunStartUnknown,
    WorkPreparing,
    WorkStarting,
    WorkActive,
    WorkWriting,
    WorkTool,
    WorkWaiting,
    WorkEnding,
    RunRetryFailed,
    RunRetryRefused,
    ClosingRun,
    ClosingDelegates,
    ClosingStageEffects,
    ClosingReleases,
    ClosingSettled,
    ClosingStageUnknown,
    ClosingConnectorsUnknown,
    ClosingConnectorsEmpty,
    ClosingPendingEnding,
    HeldPriorLifecycle,
    HeldPriorWaiting,

    EngineLimitGoals,
    MalformedTwoAsAllowed,
    MalformedConversation,
    MalformedCycle,
    MalformedProposedAsAllowed,
    EngineLimitOperations,
    MalformedProject,
    EngineLimitGrace,
    EngineLimitWords,
    EngineLimitIdentity,
    EngineLimitRoles,
    EngineLimitArithmetic,
    EngineLimitDuplicate,
    EngineLimitEmpty,
    EngineLimitBatch,
    EngineLimitProjectLiveTasks,
    EngineLimitLiveTasks,
    EngineLimitTree,
    EngineLimitDepth,
    EngineLimitDelegates,
    EngineLimitSubscription,
    EngineLimitDependencies,
    EngineLimitCycle,
    EngineLimitSpec,
    EngineLimitContract,
    EngineLimitAuthority,
    EngineLimitInputs,
    EngineLimitAttempts,
    EngineLimitLiveDelegates,
    EngineLimitRestore,
    EngineLimitRead,
    EngineLimitTurn,
    EngineLimitHoldKind,
    EngineLimitHoldTaken,
    EngineLimitHolds,
    EngineLimitInboxFull,
    EngineLimitNotes,
    EngineLimitRevision,
    EngineLimitQuestion,
    EngineLimitPriority,
    EngineLimitPolicy,
    EngineLimitResource,
    EngineLimitService,
    EngineLimitAccounts,
    EngineLimitHolderAccounts,
    EngineLimitSources,
    EngineLimitSignIns,
    EngineLimitPlacedHolds,
    EngineLimitProjectPlacedHolds,
    EngineLimitPartyPlacedHolds,
    EngineLimitExemptions,
    EngineLimitSettingsProposals,
    FailedBare,
    CancelledBare,
    DecidedBare,
    DeletedEnding,
    ByProjectDeployment,
    NoHeldRole,
    HeldOwner,
    HeldMaintainer,
    HeldMember,
    HeldObserver,
    HeldPolicy,
    DisposalMissing,
    DisposalUnknown,
    DisposalRepeated,
    GrantKindMissing,
    FundingMissing,
    DepthMaximum,
    DeadlineAuthorityMissing,
    TasksLeft,
    ToolMissing,
    ScopeMissing,
    ModelMissing,
    ReferenceMissing,
    ResourceAccessMissing,
    ResourceUnavailable,
    AuthorityShapeMissing,
    AgainRequired,
    AgainUnexpected,
    AccountMissing,
    WriterMissing,
    RequiredJudgeMissing,
    FailedJudge,
    UnguardedJudge,
    PartConnector,
    PartResource,
    PartKind,
    PartTask,
    PartProject,
    PartProposal,
    /// Opaque numeric parameter name and value.
    SpecificationNumber,
    /// Opaque bytes parameter name.
    SpecificationBytes,
    /// Parameter name, connector and numbered resource.
    SpecificationResource,
    /// An input task identity.
    SpecificationInput,
    /// The owner supplied an incomplete specification.
    SpecificationCut,
}
impl JigPhrase {
    /// Stable phrase identity within jig's owner.
    #[must_use]
    #[expect(clippy::too_many_lines, reason = "closed phrase-number data is kept in one exhaustive table")]
    pub const fn id(self) -> PhraseId {
        let number = match self {
            Self::SettingsChanged => 7900,
            Self::MovedToNewest => 7919,
            Self::SettingsRestored => 7924,
            Self::SettingsStale => 7901,
            Self::SettingsFull => 7902,
            Self::SettingsFullTask => 7903,
            Self::SettingsFullMore => 7904,
            Self::SettingsUnchanged => 7905,
            Self::MalformedRunning => 7917,
            Self::SettingCharter => 7906,
            Self::SettingTemplate => 7907,
            Self::SettingPolicy => 7908,
            Self::SettingRules => 7909,
            Self::SettingFaultShape => 7910,
            Self::SettingFaultCount => 7911,
            Self::SettingFaultReference => 7912,
            Self::SettingFaultRules => 7913,
            Self::SettingFaultBudget => 7914,
            Self::SettingFaultGrace => 7915,
            Self::SettingFaultOwner => 7916,

            Self::ActRename => 7837,
            Self::NameTaken => 7830,
            Self::NameEmpty => 7831,
            Self::NameLong => 7832,
            Self::NameControl => 7833,
            Self::NameSpace => 7834,
            Self::NameDigits => 7835,
            Self::EngineLimitProjects => 7836,

            Self::OpenProgramModel => 7816,
            Self::OpenProgramUnavailable => 7817,
            Self::OpenProgramChooseModel => 7818,
            Self::OpenProgramSource => 7819,
            Self::OpenProgramNoSwitch => 7820,
            Self::RefusedWorkerAt => 7821,
            Self::RefusedEngineAt => 7822,
            Self::MalformedEndedBy => 7823,

            Self::Word(word) => return word.id(),
            Self::DependencyParked => 7815,
            Self::KeptUntil => 7800,
            Self::DependencyRemoved => 7801,
            Self::DependencyMet => 7802,
            Self::DependencyFailed => 7803,
            Self::DependencyCancelled => 7804,
            Self::DependencyPending => 7805,
            Self::DependencyDone => 7806,
            Self::CircleWaitsFor => 7807,
            Self::CircleHold => 7808,
            Self::CircleLifts => 7809,
            Self::CircleRefused => 7810,
            Self::KeptAnswer => 7811,
            Self::DependencyByTask => 7812,
            Self::DependencyByParty => 7813,
            Self::DependencyByDeployment => 7814,

            Self::OwnerHoldRelease => 673,
            Self::NoticesPart => 680,
            Self::NothingKnown => 674,
            Self::LinkPart => 675,
            Self::NeedsPart => 676,
            Self::PartyPart => 677,
            Self::ProjectsPart => 678,
            Self::GoToTaskPart => 679,

            Self::QuestionFromTask => 670,
            Self::QuestionFromPerson => 671,
            Self::QuestionFromDeployment => 672,
            Self::CachedReplies => 640,
            Self::TasksIdle => 641,
            Self::TasksStarting => 642,
            Self::TasksBackingOff => 643,

            Self::NoteWriterParty => 634,
            Self::NoteWriterTask => 635,
            Self::NoteChangedNotSupplied => 636,
            Self::TaskListsCut => 637,
            Self::ServedGoal => 638,
            Self::PageCut => 639,
            Self::ResourceSegments => 630,
            Self::LiteralGrant => 631,
            Self::LiteralGrantMissing => 632,
            Self::ScopedResourceNotes => 633,
            Self::ChipTitleCut => 620,
            Self::ChipDependenciesCut => 621,
            Self::ChipWaitingHolds => 622,
            Self::FrameLogin => 611,
            Self::FramePerson => 612,
            Self::FrameService => 613,
            Self::FrameResultsSeen => 614,
            Self::FrameNeedsKinds => 615,
            Self::FrameProjectsCut => 616,
            Self::FramePoolUnopened => 618,
            Self::PolicyPeriodSpend => 590,
            Self::PolicyEditRole => 591,
            Self::RequestNotKept => 7404,
            Self::AnswersFull => 7499,
            Self::RequestUnconfirmed => 7405,
            Self::RequestsInFlight => 7406,
            Self::RequestsDropped => 7407,
            Self::PolicyRequests => 592,
            Self::PolicyDecides => 593,
            Self::PolicyRequirements => 594,
            Self::PolicyProjections => 595,
            Self::PolicyPermissions => 596,
            Self::PolicyEffect => 597,
            Self::PolicyJudge => 598,
            Self::PolicyGuarded => 599,
            Self::PolicyObserved => 600,
            Self::PolicyMustGuard => 601,
            Self::PolicyGuardNotRequired => 602,
            Self::PolicyPermission => 603,

            Self::ProposedByParty => 570,
            Self::PartProposalTask => 571,
            Self::PartProposalParty => 572,
            Self::PartOwnerProposal => 573,
            Self::RunIdle => 530,
            Self::RunDue => 531,
            Self::RunPreparing => 532,
            Self::RunClaimed => 533,
            Self::RunOwnerActive => 534,
            Self::RunBackingOff => 535,
            Self::RunPhaseSince => 536,
            Self::RunPlacement => 537,
            Self::RunUnobserved => 538,
            Self::RunObservedSince => 539,
            Self::RunReportedAt => 540,
            Self::RunActualStart => 541,
            Self::RunStartUnknown => 542,
            Self::WorkPreparing => 543,
            Self::WorkStarting => 544,
            Self::WorkActive => 545,
            Self::WorkWriting => 546,
            Self::WorkTool => 547,
            Self::WorkWaiting => 548,
            Self::WorkEnding => 549,
            Self::RunRetryFailed => 550,
            Self::RunRetryRefused => 551,
            Self::ClosingRun => 552,
            Self::ClosingDelegates => 553,
            Self::ClosingStageEffects => 554,
            Self::ClosingReleases => 555,
            Self::ClosingSettled => 556,
            Self::ClosingStageUnknown => 557,
            Self::ClosingConnectorsUnknown => 558,
            Self::ClosingConnectorsEmpty => 559,
            Self::ClosingPendingEnding => 560,
            Self::HeldPriorLifecycle => 561,
            Self::HeldPriorWaiting => 562,

            Self::SpecificationNumber => 3200,
            Self::SpecificationBytes => 3201,
            Self::SpecificationResource => 3202,
            Self::SpecificationInput => 3203,
            Self::SpecificationCut => 3204,
            Self::LoadingNavigation => 0,
            Self::ByNamed => 1,
            Self::Refused => 2,
            Self::Leaving => 3,
            Self::Live => 4,
            Self::Offline => 5,
            Self::Starting => 6,
            Self::Behind => 7,
            Self::Needs => 8,
            Self::MoreNeeds => 9,
            Self::Updates => 10,
            Self::MoreUpdates => 11,
            Self::SignedIn => 12,
            Self::SignOut => 563,
            Self::SignOutLost => 564,
            Self::SignedOut => 13,
            Self::Me => 14,
            Self::Navigation => 15,
            Self::Project => 16,
            Self::AllProjects => 17,
            Self::TaskNumber => 18,
            Self::Inbox => 19,
            Self::Conversations => 20,
            Self::ProjectPage => 21,
            Self::Deployment => 22,
            Self::TaskPage => 23,
            Self::Open => 24,
            Self::Confirming => 25,
            Self::InFlight => 26,
            Self::Parked => 28,
            Self::Unknown => 29,
            Self::LookAgain => 30,
            Self::NoGoalTemplate => 1003,
            Self::ProposeInstead => 1002,
            Self::MoveGoalDown => 1001,
            Self::MoveGoalUp => 1000,
            Self::Looking => 31,
            Self::GoneUnknown => 32,
            Self::Changed => 33,
            Self::Proposed => 34,
            Self::Failed => 36,
            Self::Cancelled => 37,
            Self::Withdrawn => 38,
            Self::ByParty => 39,
            Self::ByTask => 40,
            Self::ByDeployment => 41,
            Self::At => 42,
            Self::Accepted => 43,
            Self::Rejected => 44,
            Self::PassedUp => 45,
            Self::Released => 46,
            Self::ReleasedAgain => 47,
            Self::ReleasedWithout => 48,
            Self::LeftHeld => 49,
            Self::DecidedFirst => 50,
            Self::MadeTask => 54,
            Self::MadeService => 55,
            Self::MadeResult => 56,
            Self::MadeNote => 57,
            Self::MovingOut => 7838,
            Self::RoleMissing => 58,
            Self::BudgetMissing => 59,
            Self::GrantMissing => 60,
            Self::DepthMissing => 61,
            Self::DeadlineMissing => 62,
            Self::TasksMissing => 63,
            Self::Agent => 64,
            Self::Procedure => 65,
            Self::Person => 66,
            Self::RoleExecutor => 67,
            Self::KindMissing => 68,
            Self::LimitLive => 69,
            Self::LimitInbox => 70,
            Self::LimitWords => 71,
            Self::LimitConversation => 72,
            Self::LimitFields => 73,
            Self::LimitRecords => 74,
            Self::LimitRequests => 75,
            Self::LimitReads => 76,
            Self::LimitWatches => 77,
            Self::LimitObjects => 78,
            Self::MovedHold => 79,
            Self::MovedSpec => 80,
            Self::MovedPerson => 81,
            Self::MovedNote => 82,
            Self::MovedWaiting => 83,
            Self::Revision => 84,
            Self::Amount => 88,
            Self::WaitingSpan => 89,
            Self::Span => 90,
            Self::Object => 91,
            Self::Proposal => 92,
            Self::Escalation => 93,
            Self::Question => 94,
            Self::PersonTask => 95,
            Self::Result => 96,
            Self::Note => 97,
            Self::ContextProject => 98,
            Self::ContextTask => 99,
            Self::ChipNumber => 100,
            Self::WakeAny => 103,
            Self::WakeAt => 104,
            Self::WakeResult => 105,
            Self::WakeTopic => 106,
            Self::WakeInbox => 107,
            Self::Attempt => 111,
            Self::RetryAt => 112,
            Self::HostLost => 114,
            Self::Temporary => 115,
            Self::Permanent => 116,
            Self::Stall => 117,
            Self::Reason => 118,
            Self::Closing => 119,
            Self::ClosingConnector => 403,
            Self::ClosingEffects => 404,
            Self::NobodyMayAccept => 405,
            Self::ApprovalAuthorityMissing => 406,
            Self::WaitingParty => 122,
            Self::WaitingTask => 123,
            Self::WaitingRole => 124,
            Self::Since => 125,
            Self::Pool => 127,
            Self::ProposalWhy => 128,
            Self::OwnTasks => 129,
            Self::Batch => 130,
            Self::BatchTask => 131,
            Self::AfterTask => 132,
            Self::Budget => 133,
            Self::Before => 134,
            Self::After => 135,
            Self::Depth => 136,
            Self::TasksAllowed => 137,
            Self::Grant => 138,
            Self::ToolAllowed => 139,
            Self::PassedBy => 140,
            Self::Widen => 141,
            Self::AmendTask => 142,
            Self::Effect => 143,
            Self::AnswerField => 144,
            Self::ApprovalState => 145,
            Self::Options => 146,
            Self::CannotDo => 147,
            Self::TakenBy => 148,
            Self::StallSpan => 149,
            Self::Verdict => 150,
            Self::ChoiceResult => 151,
            Self::ApprovalYes => 152,
            Self::ApprovalNo => 153,
            Self::FirstUnread => 154,
            Self::Reply => 155,
            Self::LoadingPart => 156,
            Self::LoadingObject => 157,
            Self::ActWrite => 158,
            Self::ActAnswer => 159,
            Self::ActTake => 160,
            Self::ActHandBack => 161,
            Self::ActAccept => 162,
            Self::ActReject => 163,
            Self::ActPassUp => 164,
            Self::ActRelease => 165,
            Self::ActLeaveHeld => 166,
            Self::ActAmend => 167,
            Self::ActAmendRelease => 168,
            Self::ActWithdraw => 809,
            Self::ActStop => 169,
            Self::ActCancel => 170,
            Self::ActClose => 171,
            Self::ActTakeOver => 172,
            Self::ActSetGoal => 173,
            Self::ActReorder => 174,
            Self::ActMarkSeen => 175,
            Self::ActEditNote => 176,
            Self::ActDeleteNote => 177,
            Self::ActAdopt => 178,
            Self::ActChangePolicy => 179,
            Self::ActSetRoles => 180,
            Self::ActMakeService => 181,
            Self::ActSetPool => 182,
            Self::ChatAbout => 183,
            Self::Topic => 185,
            Self::ElapsedSpan => 186,
            Self::ReasonField => 187,
            Self::LoadingPhase => 188,
            Self::Deadline => 189,
            Self::StalledOn => 191,
            Self::DependencyEnded => 192,
            Self::DriftHold => 193,
            Self::EffectFailedHold => 194,
            Self::EffectUncertainHold => 195,
            Self::BudgetHold => 196,
            Self::DeadlineHold => 197,
            Self::StoppedHold => 198,
            Self::ResourceHold => 199,
            Self::PoolHold => 200,
            Self::ProcedureHold => 201,
            Self::LoadingDependency => 202,
            Self::BeforeHold => 203,
            Self::SpentRun => 204,
            Self::SpentDelegate => 205,
            Self::Confirmation => 206,
            Self::Confirm => 207,
            Self::Dismiss => 208,
            Self::WillPropose => 210,
            Self::ChangedConfirmation => 211,
            Self::MissingWords => 212,
            Self::MissingChoice => 213,
            Self::MissingAgain => 214,
            Self::MissingChange => 215,
            Self::MissingDelegates => 216,
            Self::MissingPriority => 217,
            Self::MissingContext => 218,
            Self::WillAccept => 219,
            Self::WillReject => 220,
            Self::WillPassUp => 221,
            Self::WillLeaveHeld => 222,
            Self::WillStop => 223,
            Self::WillCancel => 224,
            Self::WillClose => 225,
            Self::WillTakeOver => 226,
            Self::WillTake => 227,
            Self::WillHandBack => 228,
            Self::WillWrite => 229,
            Self::WillAnswer => 230,
            Self::WillAmend => 231,
            Self::WillAmendRelease => 232,
            Self::WillPrioritise => 233,
            Self::WillEditNote => 234,
            Self::WillDeleteNote => 235,
            Self::WillGoal => 236,
            Self::ContinueClosing => 238,
            Self::ReleaseWait => 239,
            Self::ReleaseProcedure => 241,
            Self::RepeatEffect => 242,
            Self::SkipEffect => 243,
            Self::DelegateCancel => 257,
            Self::DelegateTake => 258,
            Self::DelegateChoices => 259,
            Self::WriteAndRelease => 260,
            Self::WriteOnly => 261,
            Self::NewSpec => 262,
            Self::NewInstructions => 263,
            Self::InstructionNotSupplied => 580,
            Self::InstructionEmpty => 581,
            Self::NewBudget => 264,
            Self::NewDeadline => 265,
            Self::DropDependency => 266,
            Self::PriorityField => 267,
            Self::NoteDescription => 268,
            Self::NoteBody => 269,
            Self::DocumentTextCut => 3500,
            Self::PersonAddressRevision => 3600,
            Self::ArchiveCumulativeSpend => 7400,
            Self::ArchiveAcceptedAt => 7401,
            Self::ArchiveReadThrough => 7402,
            Self::ArchiveReadAbsent => 7403,
            Self::ResultTimeUnknown => 3700,
            Self::ChangeResultKind => 3701,
            Self::NoteReferencesCut => 3300,
            Self::Charter => 270,
            Self::CreateProcedure => 510,
            Self::Priority => 271,
            Self::Notice => 272,
            Self::UnknownAction => 273,
            Self::ResultsThrough => 274,
            Self::RepliesThrough => 275,
            Self::AddressParty => 278,
            Self::AddressTask => 279,
            Self::AddressRole => 280,
            Self::NeedsYou => 286,
            Self::InboxUpdates => 287,
            Self::AllKinds => 288,
            Self::AllGoals => 289,
            Self::OnlyGoal => 290,
            Self::NewUpdate => 291,
            Self::OnCall => 292,
            Self::PassesUp => 293,
            Self::ReadEarlier => 294,
            Self::ReadLater => 295,
            Self::WindowStart => 296,
            Self::WindowEnd => 297,
            Self::MoreRows => 298,
            Self::NewChat => 299,
            Self::BeginConversation => 300,
            Self::Delegates => 301,
            Self::SignInPage => 302,
            Self::SignInWith => 303,
            Self::MissingPage => 304,
            Self::UpdateClient => 305,
            Self::SomeoneElse => 306,
            Self::OwnSpend => 307,
            Self::DelegateSpend => 308,
            Self::Overview => 309,
            Self::Activity => 310,
            Self::Plan => 311,
            Self::History => 312,
            Self::BudgetAuthority => 313,
            Self::Panels => 314,
            Self::Goal => 315,
            Self::Resources => 316,
            Self::Policy => 317,
            Self::Roles => 318,
            Self::Spend => 319,
            Self::Recurring => 320,
            Self::NextRun => 407,
            Self::NextPeriod => 408,
            Self::WaitingRoles => 411,
            Self::HoldRevision => 412,
            Self::CurrentPeriod => 409,
            Self::PeriodBegan => 410,
            Self::Settings => 321,
            Self::Goals => 322,
            Self::Tasks => 323,
            Self::Notes => 324,
            Self::Reserved => 325,
            Self::Left => 326,
            Self::TotalBudget => 327,
            Self::OwnBudget => 328,
            Self::EndedBudget => 329,
            Self::HeldBelow => 330,
            Self::FoldSubtree => 331,
            Self::OpenSubtree => 332,
            Self::AllPlan => 333,
            Self::HeldPlan => 334,
            Self::LivePlan => 335,
            Self::OwnHistory => 336,
            Self::SubtreeHistory => 337,
            Self::BatchMade => 338,
            Self::MovedWork => 339,
            Self::GoalWords => 340,
            Self::GoalBudget => 341,
            Self::GoalDeadline => 342,
            Self::GoalPriority => 343,
            Self::PriorityMeaning => 344,
            Self::PoolLeft => 345,
            Self::GoalPropose => 346,
            Self::NoteSearch => 347,
            Self::AskedByMe => 348,
            Self::AllTasks => 349,
            Self::Period => 350,
            Self::RoleNumber => 351,
            Self::PartyNumber => 352,
            Self::Allotment => 353,
            Self::DecidesKind => 354,
            Self::PolicySpend => 355,
            Self::HostNumber => 356,
            Self::HostConnected => 357,
            Self::HostDisconnected => 358,
            Self::SlotsUsed => 359,
            Self::SlotsFree => 360,
            Self::AccountNumber => 361,
            Self::AccountUsable => 362,
            Self::AccountUnavailable => 363,
            Self::Capacity => 364,
            Self::TasksLive => 365,
            Self::CapacityLimit => 366,
            Self::Grace => 367,
            Self::WaitingAccount => 368,
            Self::ResourceScope => 371,
            Self::ResultExpected => 372,
            Self::ReportExpected => 373,
            Self::WordsExpected => 374,
            Self::ApprovalExpected => 375,
            Self::ProposalsWaiting => 376,
            Self::ConversationLog => 377,
            Self::WordsBeingSent => 378,
            Self::WordsSent => 379,
            Self::WordsRead => 380,
            Self::RunResumed => 381,
            Self::RunFreshTail => 382,
            Self::RunFinished => 383,
            Self::RunCancelled => 384,
            Self::TurnInput => 385,
            Self::TurnOutput => 386,
            Self::CacheRead => 387,
            Self::CacheWritten => 388,
            Self::TurnSpent => 389,
            Self::Provisional => 390,
            Self::TurnUnreadVersion => 806,
            Self::TurnUnreadMalformed => 807,
            Self::TurnUnreadTooLarge => 808,
            Self::TurnCut => 391,
            Self::OpaqueDialect => 392,
            Self::OpaqueBytes => 393,
            Self::CallNumber => 394,
            Self::OpenCall => 395,
            Self::FoldCall => 396,
            Self::CallFailed => 397,
            Self::CallCompleted => 398,
            Self::CallPending => 399,
            Self::NewsTopic => 400,
            Self::TurnNumber => 401,
            Self::TurnInstructions => 402,
            Self::PanelPart => 490,
            Self::ResultPart => 491,
            Self::HoldPart => 492,
            Self::OutboxEntry => 508,
            Self::TakenByNumber => 509,

            Self::EngineLimitGoals => 7413,
            Self::MalformedTwoAsAllowed => 7415,
            Self::MalformedConversation => 7416,
            Self::MalformedCycle => 7417,
            Self::MalformedProposedAsAllowed => 7418,
            Self::EngineLimitOperations => 7411,
            Self::MalformedProject => 7412,
            Self::EngineLimitGrace => 7410,
            Self::EngineLimitWords => 413,
            Self::EngineLimitIdentity => 414,
            Self::EngineLimitRoles => 415,
            Self::EngineLimitArithmetic => 416,
            Self::EngineLimitDuplicate => 417,
            Self::EngineLimitEmpty => 418,
            Self::EngineLimitBatch => 419,
            Self::EngineLimitProjectLiveTasks => 420,
            Self::EngineLimitLiveTasks => 421,
            Self::EngineLimitTree => 422,
            Self::EngineLimitDepth => 423,
            Self::EngineLimitDelegates => 424,
            Self::EngineLimitSubscription => 425,
            Self::EngineLimitDependencies => 426,
            Self::EngineLimitCycle => 427,
            Self::EngineLimitSpec => 428,
            Self::EngineLimitContract => 429,
            Self::EngineLimitAuthority => 430,
            Self::EngineLimitInputs => 431,
            Self::EngineLimitAttempts => 432,
            Self::EngineLimitLiveDelegates => 433,
            Self::EngineLimitRestore => 434,
            Self::EngineLimitRead => 435,
            Self::EngineLimitTurn => 436,
            Self::EngineLimitHoldKind => 437,
            Self::EngineLimitHoldTaken => 438,
            Self::EngineLimitHolds => 439,
            Self::EngineLimitInboxFull => 440,
            Self::EngineLimitNotes => 441,
            Self::EngineLimitRevision => 442,
            Self::EngineLimitQuestion => 443,
            Self::EngineLimitPriority => 444,
            Self::EngineLimitPolicy => 445,
            Self::EngineLimitResource => 446,
            Self::EngineLimitService => 447,
            Self::EngineLimitAccounts => 565,
            Self::EngineLimitHolderAccounts => 566,
            Self::EngineLimitSources => 567,
            Self::EngineLimitSignIns => 568,
            Self::EngineLimitPlacedHolds => 7920,
            Self::EngineLimitProjectPlacedHolds => 7921,
            Self::EngineLimitPartyPlacedHolds => 7922,
            Self::EngineLimitExemptions => 7923,
            Self::EngineLimitSettingsProposals => 7918,
            Self::FailedBare => 448,
            Self::CancelledBare => 449,
            Self::DecidedBare => 450,
            Self::DeletedEnding => 451,
            Self::ByProjectDeployment => 452,
            Self::NoHeldRole => 454,
            Self::HeldOwner => 455,
            Self::HeldMaintainer => 456,
            Self::HeldMember => 457,
            Self::HeldObserver => 458,
            Self::HeldPolicy => 459,
            Self::DisposalMissing => 460,
            Self::DisposalUnknown => 461,
            Self::DisposalRepeated => 462,
            Self::GrantKindMissing => 466,
            Self::FundingMissing => 467,
            Self::DepthMaximum => 468,
            Self::DeadlineAuthorityMissing => 469,
            Self::TasksLeft => 470,
            Self::ToolMissing => 471,
            Self::ScopeMissing => 472,
            Self::ModelMissing => 810,
            Self::ReferenceMissing => 473,
            Self::ResourceAccessMissing => 474,
            Self::ResourceUnavailable => 475,
            Self::AuthorityShapeMissing => 476,
            Self::AgainRequired => 477,
            Self::AgainUnexpected => 478,
            Self::AccountMissing => 479,
            Self::WriterMissing => 480,
            Self::RequiredJudgeMissing => 481,
            Self::FailedJudge => 482,
            Self::UnguardedJudge => 483,
            Self::PartConnector => 484,
            Self::PartResource => 485,
            Self::PartKind => 486,
            Self::PartTask => 487,
            Self::PartProject => 488,
            Self::PartProposal => 489,
            Self::FailureRun => 505,
            Self::FailureAgent => 506,
            Self::FailureInvalid => 507,
            Self::RecurringKey => 800,
            Self::RecurringSkip => 801,
            Self::RecurringWait => 802,
            Self::RecurringLastPeriod => 803,
            Self::RecurringPendingPeriod => 804,
            Self::RecurringCut => 805,
        };
        PhraseId { owner: Owner::Jig.opaque(), number }
    }
    /// Hole contract checked for every supplied language.
    #[must_use]
    #[expect(clippy::too_many_lines, reason = "every phrase declares its holes in one exhaustive table")]
    pub fn shape(self, text: u32) -> PhraseShape {
        let holes: Box<[HoleShape]> = match self {
            Self::Word(word) => return word.shape(text),

            Self::DependencyByTask | Self::DependencyByParty | Self::DependencyByDeployment | Self::AnswersFull => {
                Box::new([HoleShape { kind: HoleKind::Number, bytes: 0 }, HoleShape { kind: HoleKind::Wall, bytes: 0 }])
            }

            Self::FrameNeedsKinds => Box::new([
                HoleShape { kind: HoleKind::Number, bytes: 0 },
                HoleShape { kind: HoleKind::Number, bytes: 0 },
                HoleShape { kind: HoleKind::Number, bytes: 0 },
                HoleShape { kind: HoleKind::Number, bytes: 0 },
            ]),
            Self::PolicyJudge | Self::PolicyPermission | Self::SpecificationResource | Self::MovedToNewest => {
                Box::new([
                    HoleShape { kind: HoleKind::Number, bytes: 0 },
                    HoleShape { kind: HoleKind::Number, bytes: 0 },
                    HoleShape { kind: HoleKind::Number, bytes: 0 },
                ])
            }
            Self::MalformedRunning
            | Self::SettingsUnchanged
            | Self::SettingRules
            | Self::SettingFaultShape
            | Self::SettingFaultCount
            | Self::SettingFaultReference
            | Self::SettingFaultRules
            | Self::SettingFaultBudget
            | Self::SettingFaultGrace
            | Self::SettingFaultOwner
            | Self::OpenProgramModel
            | Self::OpenProgramUnavailable
            | Self::OpenProgramChooseModel
            | Self::OpenProgramNoSwitch
            | Self::DependencyParked
            | Self::CircleHold
            | Self::CircleLifts
            | Self::CircleRefused
            | Self::KeptAnswer
            | Self::ActWithdraw
            | Self::OwnerHoldRelease
            | Self::PageCut
            | Self::NoteChangedNotSupplied
            | Self::TaskListsCut
            | Self::FramePerson
            | Self::FrameService
            | Self::FrameProjectsCut
            | Self::FramePoolUnopened
            | Self::PolicyRequirements
            | Self::PolicyProjections
            | Self::PolicyPermissions
            | Self::PolicyGuarded
            | Self::PolicyMustGuard
            | Self::PolicyGuardNotRequired
            | Self::RunIdle
            | Self::RunDue
            | Self::RunPreparing
            | Self::RunUnobserved
            | Self::RunStartUnknown
            | Self::WorkPreparing
            | Self::WorkStarting
            | Self::WorkActive
            | Self::WorkWriting
            | Self::WorkWaiting
            | Self::WorkEnding
            | Self::ClosingDelegates
            | Self::ClosingStageEffects
            | Self::ClosingReleases
            | Self::ClosingSettled
            | Self::ClosingStageUnknown
            | Self::ClosingConnectorsUnknown
            | Self::ClosingConnectorsEmpty
            | Self::ClosingPendingEnding
            | Self::HeldPriorLifecycle
            | Self::HeldPriorWaiting
            | Self::SpecificationCut
            | Self::FailureRun
            | Self::FailureAgent
            | Self::FailureInvalid
            | Self::PanelPart
            | Self::ResultPart
            | Self::HoldPart
            | Self::EngineLimitGoals
            | Self::EngineLimitOperations
            | Self::EngineLimitGrace
            | Self::EngineLimitWords
            | Self::EngineLimitIdentity
            | Self::EngineLimitRoles
            | Self::EngineLimitArithmetic
            | Self::EngineLimitDuplicate
            | Self::EngineLimitEmpty
            | Self::EngineLimitBatch
            | Self::EngineLimitProjectLiveTasks
            | Self::EngineLimitLiveTasks
            | Self::EngineLimitTree
            | Self::EngineLimitDepth
            | Self::EngineLimitDelegates
            | Self::EngineLimitSubscription
            | Self::EngineLimitDependencies
            | Self::EngineLimitCycle
            | Self::EngineLimitSpec
            | Self::EngineLimitContract
            | Self::EngineLimitAuthority
            | Self::EngineLimitInputs
            | Self::EngineLimitAttempts
            | Self::EngineLimitLiveDelegates
            | Self::EngineLimitRestore
            | Self::EngineLimitRead
            | Self::EngineLimitTurn
            | Self::EngineLimitHoldKind
            | Self::EngineLimitHoldTaken
            | Self::EngineLimitHolds
            | Self::EngineLimitInboxFull
            | Self::EngineLimitNotes
            | Self::EngineLimitRevision
            | Self::EngineLimitQuestion
            | Self::EngineLimitPriority
            | Self::EngineLimitPolicy
            | Self::EngineLimitResource
            | Self::EngineLimitAccounts
            | Self::EngineLimitHolderAccounts
            | Self::EngineLimitSources
            | Self::ActRename
            | Self::NameTaken
            | Self::NameEmpty
            | Self::NameLong
            | Self::NameControl
            | Self::NameSpace
            | Self::NameDigits
            | Self::EngineLimitProjects
            | Self::EngineLimitPlacedHolds
            | Self::EngineLimitProjectPlacedHolds
            | Self::EngineLimitPartyPlacedHolds
            | Self::EngineLimitExemptions
            | Self::EngineLimitSettingsProposals
            | Self::EngineLimitSignIns
            | Self::EngineLimitService
            | Self::FailedBare
            | Self::CancelledBare
            | Self::DecidedBare
            | Self::DeletedEnding
            | Self::NoHeldRole
            | Self::HeldOwner
            | Self::HeldMaintainer
            | Self::HeldMember
            | Self::HeldObserver
            | Self::FundingMissing
            | Self::DeadlineAuthorityMissing
            | Self::ToolMissing
            | Self::ScopeMissing
            | Self::ModelMissing
            | Self::ReferenceMissing
            | Self::ResourceAccessMissing
            | Self::ResourceUnavailable
            | Self::AuthorityShapeMissing
            | Self::AgainRequired
            | Self::AgainUnexpected
            | Self::AccountMissing
            | Self::WriterMissing
            | Self::Overview
            | Self::Activity
            | Self::Plan
            | Self::History
            | Self::BudgetAuthority
            | Self::Panels
            | Self::Goal
            | Self::Resources
            | Self::Policy
            | Self::Roles
            | Self::Spend
            | Self::Recurring
            | Self::NextPeriod
            | Self::RecurringSkip
            | Self::RecurringWait
            | Self::RecurringCut
            | Self::Settings
            | Self::Goals
            | Self::Tasks
            | Self::Notes
            | Self::FoldSubtree
            | Self::OpenSubtree
            | Self::AllPlan
            | Self::HeldPlan
            | Self::LivePlan
            | Self::OwnHistory
            | Self::SubtreeHistory
            | Self::MovedWork
            | Self::GoalWords
            | Self::GoalBudget
            | Self::GoalDeadline
            | Self::GoalPriority
            | Self::PriorityMeaning
            | Self::GoalPropose
            | Self::NoteSearch
            | Self::AskedByMe
            | Self::AllTasks
            | Self::HostConnected
            | Self::HostDisconnected
            | Self::AccountUsable
            | Self::AccountUnavailable
            | Self::Capacity
            | Self::WaitingAccount
            | Self::ResultExpected
            | Self::ReportExpected
            | Self::WordsExpected
            | Self::ApprovalExpected
            | Self::ConversationLog
            | Self::WordsBeingSent
            | Self::WordsSent
            | Self::WordsRead
            | Self::RunResumed
            | Self::RunFinished
            | Self::RunCancelled
            | Self::Provisional
            | Self::TurnUnreadMalformed
            | Self::TurnUnreadTooLarge
            | Self::TurnCut
            | Self::CallFailed
            | Self::CallCompleted
            | Self::CallPending
            | Self::TurnInstructions
            | Self::NeedsYou
            | Self::InboxUpdates
            | Self::AllKinds
            | Self::AllGoals
            | Self::NewUpdate
            | Self::OnCall
            | Self::ReadEarlier
            | Self::ReadLater
            | Self::WindowStart
            | Self::WindowEnd
            | Self::MoreRows
            | Self::NewChat
            | Self::BeginConversation
            | Self::SignInPage
            | Self::MissingPage
            | Self::UpdateClient
            | Self::SomeoneElse
            | Self::LoadingNavigation
            | Self::RequestNotKept
            | Self::RequestUnconfirmed
            | Self::RequestsInFlight
            | Self::Refused
            | Self::Leaving
            | Self::Live
            | Self::Offline
            | Self::Starting
            | Self::Behind
            | Self::SignOut
            | Self::SignOutLost
            | Self::NothingKnown
            | Self::LinkPart
            | Self::NeedsPart
            | Self::PartyPart
            | Self::ProjectsPart
            | Self::GoToTaskPart
            | Self::NoticesPart
            | Self::SignedOut
            | Self::Me
            | Self::Navigation
            | Self::AllProjects
            | Self::TaskNumber
            | Self::Inbox
            | Self::Conversations
            | Self::ProjectPage
            | Self::Deployment
            | Self::Open
            | Self::Confirming
            | Self::InFlight
            | Self::Parked
            | Self::Unknown
            | Self::LookAgain
            | Self::NoGoalTemplate
            | Self::ProposeInstead
            | Self::MoveGoalDown
            | Self::MoveGoalUp
            | Self::Looking
            | Self::GoneUnknown
            | Self::Changed
            | Self::ByDeployment
            | Self::Accepted
            | Self::PassedUp
            | Self::Released
            | Self::ReleasedAgain
            | Self::ReleasedWithout
            | Self::Agent
            | Self::Procedure
            | Self::Person
            | Self::RoleExecutor
            | Self::KindMissing
            | Self::LimitFields
            | Self::LimitRecords
            | Self::LimitRequests
            | Self::LimitReads
            | Self::LimitWatches
            | Self::LimitObjects
            | Self::MovedHold
            | Self::MovedSpec
            | Self::MovedPerson
            | Self::MovedNote
            | Self::MovedWaiting
            | Self::Object
            | Self::Proposal
            | Self::Escalation
            | Self::Question
            | Self::PersonTask
            | Self::Result
            | Self::Note
            | Self::WakeAny
            | Self::WakeAt
            | Self::WakeResult
            | Self::WakeTopic
            | Self::WakeInbox
            | Self::HostLost
            | Self::Temporary
            | Self::Permanent
            | Self::Stall
            | Self::ResourceSegments
            | Self::ChipTitleCut
            | Self::ChipDependenciesCut
            | Self::ChipWaitingHolds
            | Self::Pool
            | Self::ProposalWhy
            | Self::OwnTasks
            | Self::Batch
            | Self::Before
            | Self::After
            | Self::PassedBy
            | Self::Widen
            | Self::Effect
            | Self::AnswerField
            | Self::Options
            | Self::CannotDo
            | Self::Reply
            | Self::ClosingConnector
            | Self::ClosingEffects
            | Self::NobodyMayAccept
            | Self::ApprovalAuthorityMissing
            | Self::LoadingPart
            | Self::LoadingObject
            | Self::ActWrite
            | Self::ActAnswer
            | Self::ActTake
            | Self::ActHandBack
            | Self::ActAccept
            | Self::ActReject
            | Self::ActPassUp
            | Self::ActRelease
            | Self::ActLeaveHeld
            | Self::ActAmend
            | Self::ActAmendRelease
            | Self::ActStop
            | Self::ActCancel
            | Self::ActClose
            | Self::ActTakeOver
            | Self::ActSetGoal
            | Self::ActReorder
            | Self::ActMarkSeen
            | Self::ActEditNote
            | Self::ActDeleteNote
            | Self::ActAdopt
            | Self::ActChangePolicy
            | Self::ActSetRoles
            | Self::ActMakeService
            | Self::ActSetPool
            | Self::ChatAbout
            | Self::ReasonField
            | Self::LoadingPhase
            | Self::DriftHold
            | Self::EffectFailedHold
            | Self::EffectUncertainHold
            | Self::ResourceHold
            | Self::PoolHold
            | Self::ProcedureHold
            | Self::BeforeHold
            | Self::Confirmation
            | Self::Confirm
            | Self::Dismiss
            | Self::WillPropose
            | Self::ChangedConfirmation
            | Self::MissingWords
            | Self::MissingChoice
            | Self::MissingAgain
            | Self::MissingChange
            | Self::MissingDelegates
            | Self::MissingPriority
            | Self::MissingContext
            | Self::WillAccept
            | Self::WillReject
            | Self::WillPassUp
            | Self::WillLeaveHeld
            | Self::WillStop
            | Self::WillCancel
            | Self::WillClose
            | Self::WillTakeOver
            | Self::WillTake
            | Self::WillHandBack
            | Self::WillWrite
            | Self::WillAnswer
            | Self::WillAmend
            | Self::WillAmendRelease
            | Self::WillPrioritise
            | Self::WillEditNote
            | Self::WillDeleteNote
            | Self::WillGoal
            | Self::ContinueClosing
            | Self::ReleaseWait
            | Self::ReleaseProcedure
            | Self::RepeatEffect
            | Self::SkipEffect
            | Self::DelegateChoices
            | Self::WriteAndRelease
            | Self::WriteOnly
            | Self::NewSpec
            | Self::NewInstructions
            | Self::InstructionNotSupplied
            | Self::InstructionEmpty
            | Self::NewBudget
            | Self::NewDeadline
            | Self::PriorityField
            | Self::NoteDescription
            | Self::NoteBody
            | Self::DocumentTextCut
            | Self::ArchiveReadAbsent
            | Self::ResultTimeUnknown
            | Self::NoteReferencesCut
            | Self::Notice
            | Self::UnknownAction
            | Self::AddressParty
            | Self::AddressTask
            | Self::AddressRole => Box::new([]),
            Self::FrameLogin
            | Self::PartResource
            | Self::ResourceScope
            | Self::OpaqueDialect
            | Self::OpenCall
            | Self::FoldCall
            | Self::SignInWith
            | Self::ByNamed
            | Self::SignedIn
            | Self::Project
            | Self::Failed
            | Self::Cancelled
            | Self::Withdrawn
            | Self::Rejected
            | Self::LeftHeld
            | Self::DecidedFirst
            | Self::Reason
            | Self::ApprovalState
            | Self::TakenBy
            | Self::StalledOn => Box::new([HoleShape { kind: HoleKind::Name, bytes: text }]),
            Self::SettingsChanged
            | Self::SettingsStale
            | Self::SettingsFull
            | Self::SettingsFullTask
            | Self::SettingsFullMore
            | Self::SettingCharter
            | Self::SettingTemplate
            | Self::SettingPolicy
            | Self::MalformedEndedBy
            | Self::TurnUnreadVersion
            | Self::FrameResultsSeen
            | Self::PolicyEditRole
            | Self::PolicyRequests
            | Self::PolicyDecides
            | Self::PolicyObserved
            | Self::RunClaimed
            | Self::RunOwnerActive
            | Self::RunRetryRefused
            | Self::ClosingRun
            | Self::SpecificationBytes
            | Self::SpecificationInput
            | Self::ByProjectDeployment
            | Self::HeldPolicy
            | Self::DisposalMissing
            | Self::DisposalUnknown
            | Self::DisposalRepeated
            | Self::GrantKindMissing
            | Self::DepthMaximum
            | Self::TasksLeft
            | Self::OutboxEntry
            | Self::TakenByNumber
            | Self::PartConnector
            | Self::PartKind
            | Self::PartTask
            | Self::PartProject
            | Self::HeldBelow
            | Self::BatchMade
            | Self::Period
            | Self::CurrentPeriod
            | Self::RecurringKey
            | Self::RecurringLastPeriod
            | Self::RecurringPendingPeriod
            | Self::WaitingRoles
            | Self::HoldRevision
            | Self::RoleNumber
            | Self::PartyNumber
            | Self::DecidesKind
            | Self::HostNumber
            | Self::SlotsUsed
            | Self::SlotsFree
            | Self::AccountNumber
            | Self::TasksLive
            | Self::CapacityLimit
            | Self::ProposalsWaiting
            | Self::RunFreshTail
            | Self::TurnInput
            | Self::TurnOutput
            | Self::CacheRead
            | Self::CacheWritten
            | Self::OpaqueBytes
            | Self::CallNumber
            | Self::NewsTopic
            | Self::TurnNumber
            | Self::OnlyGoal
            | Self::Delegates
            | Self::Needs
            | Self::MoreNeeds
            | Self::Updates
            | Self::MoreUpdates
            | Self::TaskPage
            | Self::NoteWriterParty
            | Self::ServedGoal
            | Self::QuestionFromTask
            | Self::QuestionFromPerson
            | Self::QuestionFromDeployment
            | Self::CachedReplies
            | Self::TasksIdle
            | Self::TasksStarting
            | Self::TasksBackingOff
            | Self::ByParty
            | Self::ByTask
            | Self::MalformedTwoAsAllowed
            | Self::MalformedConversation
            | Self::MalformedProposedAsAllowed
            | Self::MalformedCycle
            | Self::MalformedProject
            | Self::MadeTask
            | Self::MadeService
            | Self::MadeResult
            | Self::MadeNote
            | Self::LimitWords
            | Self::LimitConversation
            | Self::Revision
            | Self::ContextProject
            | Self::ContextTask
            | Self::Attempt
            | Self::WaitingParty
            | Self::WaitingTask
            | Self::BatchTask
            | Self::AfterTask
            | Self::Depth
            | Self::TasksAllowed
            | Self::ToolAllowed
            | Self::AmendTask
            | Self::Verdict
            | Self::PersonAddressRevision
            | Self::ArchiveReadThrough
            | Self::ChangeResultKind
            | Self::FirstUnread
            | Self::Topic
            | Self::DependencyEnded
            | Self::LoadingDependency
            | Self::DelegateCancel
            | Self::DropDependency
            | Self::Charter
            | Self::Priority
            | Self::Closing
            | Self::PartOwnerProposal
            | Self::RequestsDropped
            | Self::KeptUntil
            | Self::DependencyRemoved
            | Self::DependencyMet
            | Self::DependencyFailed
            | Self::DependencyCancelled
            | Self::DependencyPending
            | Self::DependencyDone
            | Self::CircleWaitsFor
            | Self::ResultsThrough => Box::new([HoleShape { kind: HoleKind::Number, bytes: 0 }]),
            Self::SettingsRestored
            | Self::OpenProgramSource
            | Self::PolicyEffect
            | Self::RunRetryFailed
            | Self::SpecificationNumber
            | Self::CreateProcedure
            | Self::RequiredJudgeMissing
            | Self::FailedJudge
            | Self::UnguardedJudge
            | Self::PartProposal
            | Self::PartProposalTask
            | Self::PartProposalParty
            | Self::ProposedByParty
            | Self::Proposed
            | Self::RoleMissing
            | Self::DepthMissing
            | Self::TasksMissing
            | Self::LimitLive
            | Self::LimitInbox
            | Self::WaitingRole
            | Self::RepliesThrough
            | Self::LiteralGrant
            | Self::LiteralGrantMissing
            | Self::NoteWriterTask
            | Self::ScopedResourceNotes
            | Self::MovingOut => Box::new([
                HoleShape { kind: HoleKind::Number, bytes: 0 },
                HoleShape { kind: HoleKind::Number, bytes: 0 },
            ]),
            Self::RefusedWorkerAt
            | Self::RefusedEngineAt
            | Self::RunBackingOff
            | Self::RunPhaseSince
            | Self::RunPlacement
            | Self::RunObservedSince
            | Self::RunReportedAt
            | Self::RunActualStart
            | Self::NextRun
            | Self::PeriodBegan
            | Self::Grace
            | Self::PassesUp
            | Self::ArchiveAcceptedAt
            | Self::At
            | Self::Since
            | Self::Deadline
            | Self::DeadlineHold => Box::new([HoleShape { kind: HoleKind::Wall, bytes: 0 }]),
            Self::BudgetMissing | Self::BudgetHold => Box::new([
                HoleShape { kind: HoleKind::Amount, bytes: 0 },
                HoleShape { kind: HoleKind::Amount, bytes: 0 },
            ]),
            Self::GrantMissing | Self::Grant => Box::new([
                HoleShape { kind: HoleKind::Number, bytes: 0 },
                HoleShape { kind: HoleKind::Number, bytes: 0 },
                HoleShape { kind: HoleKind::Name, bytes: text },
            ]),
            Self::DeadlineMissing => {
                Box::new([HoleShape { kind: HoleKind::Wall, bytes: 0 }, HoleShape { kind: HoleKind::Wall, bytes: 0 }])
            }
            Self::PolicyPeriodSpend
            | Self::Reserved
            | Self::Left
            | Self::TotalBudget
            | Self::OwnBudget
            | Self::EndedBudget
            | Self::PoolLeft
            | Self::Allotment
            | Self::PolicySpend
            | Self::ArchiveCumulativeSpend
            | Self::TurnSpent
            | Self::OwnSpend
            | Self::DelegateSpend
            | Self::Amount
            | Self::Budget => Box::new([HoleShape { kind: HoleKind::Amount, bytes: 0 }]),
            Self::WaitingSpan | Self::Span | Self::StallSpan | Self::ElapsedSpan => {
                Box::new([HoleShape { kind: HoleKind::Span, bytes: 0 }])
            }
            Self::ChipNumber | Self::ChoiceResult => Box::new([
                HoleShape { kind: HoleKind::Number, bytes: 0 },
                HoleShape { kind: HoleKind::Name, bytes: text },
            ]),
            Self::RetryAt => Box::new([
                HoleShape { kind: HoleKind::Wall, bytes: 0 },
                HoleShape { kind: HoleKind::Number, bytes: 0 },
                HoleShape { kind: HoleKind::Number, bytes: 0 },
            ]),
            Self::WorkTool | Self::ApprovalYes | Self::ApprovalNo => Box::new([
                HoleShape { kind: HoleKind::Name, bytes: text },
                HoleShape { kind: HoleKind::Name, bytes: text },
            ]),
            Self::StoppedHold => Box::new([
                HoleShape { kind: HoleKind::Name, bytes: text },
                HoleShape { kind: HoleKind::Wall, bytes: 0 },
            ]),
            Self::SpentRun => Box::new([
                HoleShape { kind: HoleKind::Number, bytes: 0 },
                HoleShape { kind: HoleKind::Number, bytes: 0 },
                HoleShape { kind: HoleKind::Amount, bytes: 0 },
            ]),
            Self::SpentDelegate | Self::DelegateTake => Box::new([
                HoleShape { kind: HoleKind::Number, bytes: 0 },
                HoleShape { kind: HoleKind::Amount, bytes: 0 },
            ]),
        };
        PhraseShape { id: self.id(), holes, plural: None }
    }
}
/// Every jig phrase; loading checks this whole contract.
pub const ALL: [JigPhrase; 645] = [
    JigPhrase::SettingsChanged,
    JigPhrase::MovedToNewest,
    JigPhrase::SettingsRestored,
    JigPhrase::SettingsStale,
    JigPhrase::SettingsFull,
    JigPhrase::SettingsFullTask,
    JigPhrase::SettingsFullMore,
    JigPhrase::SettingsUnchanged,
    JigPhrase::MalformedRunning,
    JigPhrase::SettingCharter,
    JigPhrase::SettingTemplate,
    JigPhrase::SettingPolicy,
    JigPhrase::SettingRules,
    JigPhrase::SettingFaultShape,
    JigPhrase::SettingFaultCount,
    JigPhrase::SettingFaultReference,
    JigPhrase::SettingFaultRules,
    JigPhrase::SettingFaultBudget,
    JigPhrase::SettingFaultGrace,
    JigPhrase::SettingFaultOwner,
    JigPhrase::ActRename,
    JigPhrase::NameTaken,
    JigPhrase::NameEmpty,
    JigPhrase::NameLong,
    JigPhrase::NameControl,
    JigPhrase::NameSpace,
    JigPhrase::NameDigits,
    JigPhrase::EngineLimitProjects,
    JigPhrase::OpenProgramModel,
    JigPhrase::OpenProgramUnavailable,
    JigPhrase::OpenProgramChooseModel,
    JigPhrase::OpenProgramSource,
    JigPhrase::OpenProgramNoSwitch,
    JigPhrase::RefusedWorkerAt,
    JigPhrase::RefusedEngineAt,
    JigPhrase::MalformedEndedBy,
    JigPhrase::DependencyParked,
    JigPhrase::KeptUntil,
    JigPhrase::DependencyRemoved,
    JigPhrase::DependencyMet,
    JigPhrase::DependencyFailed,
    JigPhrase::DependencyCancelled,
    JigPhrase::DependencyPending,
    JigPhrase::DependencyDone,
    JigPhrase::CircleWaitsFor,
    JigPhrase::CircleHold,
    JigPhrase::CircleLifts,
    JigPhrase::CircleRefused,
    JigPhrase::KeptAnswer,
    JigPhrase::DependencyByTask,
    JigPhrase::DependencyByParty,
    JigPhrase::DependencyByDeployment,
    JigPhrase::NoticesPart,
    JigPhrase::NothingKnown,
    JigPhrase::LinkPart,
    JigPhrase::NeedsPart,
    JigPhrase::PartyPart,
    JigPhrase::ProjectsPart,
    JigPhrase::GoToTaskPart,
    JigPhrase::OwnerHoldRelease,
    JigPhrase::QuestionFromTask,
    JigPhrase::QuestionFromPerson,
    JigPhrase::QuestionFromDeployment,
    JigPhrase::CachedReplies,
    JigPhrase::TasksIdle,
    JigPhrase::TasksStarting,
    JigPhrase::TasksBackingOff,
    JigPhrase::PageCut,
    JigPhrase::NoteWriterParty,
    JigPhrase::NoteWriterTask,
    JigPhrase::NoteChangedNotSupplied,
    JigPhrase::TaskListsCut,
    JigPhrase::ServedGoal,
    JigPhrase::ResourceSegments,
    JigPhrase::LiteralGrant,
    JigPhrase::LiteralGrantMissing,
    JigPhrase::ScopedResourceNotes,
    JigPhrase::ChipTitleCut,
    JigPhrase::ChipDependenciesCut,
    JigPhrase::ChipWaitingHolds,
    JigPhrase::FrameLogin,
    JigPhrase::FramePerson,
    JigPhrase::FrameService,
    JigPhrase::FrameResultsSeen,
    JigPhrase::FrameNeedsKinds,
    JigPhrase::FrameProjectsCut,
    JigPhrase::FramePoolUnopened,
    JigPhrase::PolicyPeriodSpend,
    JigPhrase::PolicyEditRole,
    JigPhrase::RequestNotKept,
    JigPhrase::AnswersFull,
    JigPhrase::RequestUnconfirmed,
    JigPhrase::RequestsInFlight,
    JigPhrase::RequestsDropped,
    JigPhrase::PolicyRequests,
    JigPhrase::PolicyDecides,
    JigPhrase::PolicyRequirements,
    JigPhrase::PolicyProjections,
    JigPhrase::PolicyPermissions,
    JigPhrase::PolicyEffect,
    JigPhrase::PolicyJudge,
    JigPhrase::PolicyGuarded,
    JigPhrase::PolicyObserved,
    JigPhrase::PolicyMustGuard,
    JigPhrase::PolicyGuardNotRequired,
    JigPhrase::PolicyPermission,
    JigPhrase::ProposedByParty,
    JigPhrase::PartProposalTask,
    JigPhrase::PartProposalParty,
    JigPhrase::PartOwnerProposal,
    JigPhrase::RunIdle,
    JigPhrase::RunDue,
    JigPhrase::RunPreparing,
    JigPhrase::RunClaimed,
    JigPhrase::RunOwnerActive,
    JigPhrase::RunBackingOff,
    JigPhrase::RunPhaseSince,
    JigPhrase::RunPlacement,
    JigPhrase::RunUnobserved,
    JigPhrase::RunObservedSince,
    JigPhrase::RunReportedAt,
    JigPhrase::RunActualStart,
    JigPhrase::RunStartUnknown,
    JigPhrase::WorkPreparing,
    JigPhrase::WorkStarting,
    JigPhrase::WorkActive,
    JigPhrase::WorkWriting,
    JigPhrase::WorkTool,
    JigPhrase::WorkWaiting,
    JigPhrase::WorkEnding,
    JigPhrase::RunRetryFailed,
    JigPhrase::RunRetryRefused,
    JigPhrase::ClosingRun,
    JigPhrase::ClosingDelegates,
    JigPhrase::ClosingStageEffects,
    JigPhrase::ClosingReleases,
    JigPhrase::ClosingSettled,
    JigPhrase::ClosingStageUnknown,
    JigPhrase::ClosingConnectorsUnknown,
    JigPhrase::ClosingConnectorsEmpty,
    JigPhrase::ClosingPendingEnding,
    JigPhrase::HeldPriorLifecycle,
    JigPhrase::HeldPriorWaiting,
    JigPhrase::SpecificationNumber,
    JigPhrase::SpecificationBytes,
    JigPhrase::SpecificationResource,
    JigPhrase::SpecificationInput,
    JigPhrase::SpecificationCut,
    JigPhrase::LoadingNavigation,
    JigPhrase::ByNamed,
    JigPhrase::Refused,
    JigPhrase::Leaving,
    JigPhrase::Live,
    JigPhrase::Offline,
    JigPhrase::Starting,
    JigPhrase::Behind,
    JigPhrase::Needs,
    JigPhrase::MoreNeeds,
    JigPhrase::Updates,
    JigPhrase::MoreUpdates,
    JigPhrase::SignedIn,
    JigPhrase::SignOut,
    JigPhrase::SignOutLost,
    JigPhrase::SignedOut,
    JigPhrase::Me,
    JigPhrase::Navigation,
    JigPhrase::Project,
    JigPhrase::AllProjects,
    JigPhrase::TaskNumber,
    JigPhrase::Inbox,
    JigPhrase::Conversations,
    JigPhrase::ProjectPage,
    JigPhrase::Deployment,
    JigPhrase::TaskPage,
    JigPhrase::Open,
    JigPhrase::Confirming,
    JigPhrase::InFlight,
    JigPhrase::Parked,
    JigPhrase::Unknown,
    JigPhrase::LookAgain,
    JigPhrase::NoGoalTemplate,
    JigPhrase::ProposeInstead,
    JigPhrase::MoveGoalDown,
    JigPhrase::MoveGoalUp,
    JigPhrase::Looking,
    JigPhrase::GoneUnknown,
    JigPhrase::Changed,
    JigPhrase::Proposed,
    JigPhrase::Failed,
    JigPhrase::Cancelled,
    JigPhrase::Withdrawn,
    JigPhrase::ByParty,
    JigPhrase::ByTask,
    JigPhrase::ByDeployment,
    JigPhrase::At,
    JigPhrase::Accepted,
    JigPhrase::Rejected,
    JigPhrase::PassedUp,
    JigPhrase::Released,
    JigPhrase::ReleasedAgain,
    JigPhrase::ReleasedWithout,
    JigPhrase::LeftHeld,
    JigPhrase::DecidedFirst,
    JigPhrase::MadeTask,
    JigPhrase::MadeService,
    JigPhrase::MadeResult,
    JigPhrase::MadeNote,
    JigPhrase::MovingOut,
    JigPhrase::RoleMissing,
    JigPhrase::BudgetMissing,
    JigPhrase::GrantMissing,
    JigPhrase::DepthMissing,
    JigPhrase::DeadlineMissing,
    JigPhrase::TasksMissing,
    JigPhrase::Agent,
    JigPhrase::Procedure,
    JigPhrase::Person,
    JigPhrase::RoleExecutor,
    JigPhrase::KindMissing,
    JigPhrase::LimitLive,
    JigPhrase::LimitInbox,
    JigPhrase::LimitWords,
    JigPhrase::LimitConversation,
    JigPhrase::LimitFields,
    JigPhrase::LimitRecords,
    JigPhrase::LimitRequests,
    JigPhrase::LimitReads,
    JigPhrase::LimitWatches,
    JigPhrase::LimitObjects,
    JigPhrase::MovedHold,
    JigPhrase::MovedSpec,
    JigPhrase::MovedPerson,
    JigPhrase::MovedNote,
    JigPhrase::MovedWaiting,
    JigPhrase::Revision,
    JigPhrase::Amount,
    JigPhrase::WaitingSpan,
    JigPhrase::Span,
    JigPhrase::Object,
    JigPhrase::Proposal,
    JigPhrase::Escalation,
    JigPhrase::Question,
    JigPhrase::PersonTask,
    JigPhrase::Result,
    JigPhrase::Note,
    JigPhrase::ContextProject,
    JigPhrase::ContextTask,
    JigPhrase::ChipNumber,
    JigPhrase::WakeAny,
    JigPhrase::WakeAt,
    JigPhrase::WakeResult,
    JigPhrase::WakeTopic,
    JigPhrase::WakeInbox,
    JigPhrase::Attempt,
    JigPhrase::RetryAt,
    JigPhrase::HostLost,
    JigPhrase::Temporary,
    JigPhrase::Permanent,
    JigPhrase::Stall,
    JigPhrase::Reason,
    JigPhrase::Closing,
    JigPhrase::ClosingConnector,
    JigPhrase::ClosingEffects,
    JigPhrase::NobodyMayAccept,
    JigPhrase::ApprovalAuthorityMissing,
    JigPhrase::WaitingParty,
    JigPhrase::WaitingTask,
    JigPhrase::WaitingRole,
    JigPhrase::Since,
    JigPhrase::Pool,
    JigPhrase::ProposalWhy,
    JigPhrase::OwnTasks,
    JigPhrase::Batch,
    JigPhrase::BatchTask,
    JigPhrase::AfterTask,
    JigPhrase::Budget,
    JigPhrase::Before,
    JigPhrase::After,
    JigPhrase::Depth,
    JigPhrase::TasksAllowed,
    JigPhrase::Grant,
    JigPhrase::ToolAllowed,
    JigPhrase::PassedBy,
    JigPhrase::Widen,
    JigPhrase::AmendTask,
    JigPhrase::Effect,
    JigPhrase::AnswerField,
    JigPhrase::ApprovalState,
    JigPhrase::Options,
    JigPhrase::CannotDo,
    JigPhrase::TakenBy,
    JigPhrase::StallSpan,
    JigPhrase::Verdict,
    JigPhrase::ChoiceResult,
    JigPhrase::ApprovalYes,
    JigPhrase::ApprovalNo,
    JigPhrase::FirstUnread,
    JigPhrase::Reply,
    JigPhrase::LoadingPart,
    JigPhrase::LoadingObject,
    JigPhrase::ActWrite,
    JigPhrase::ActAnswer,
    JigPhrase::ActTake,
    JigPhrase::ActHandBack,
    JigPhrase::ActAccept,
    JigPhrase::ActReject,
    JigPhrase::ActPassUp,
    JigPhrase::ActRelease,
    JigPhrase::ActLeaveHeld,
    JigPhrase::ActAmend,
    JigPhrase::ActAmendRelease,
    JigPhrase::ActWithdraw,
    JigPhrase::ActStop,
    JigPhrase::ActCancel,
    JigPhrase::ActClose,
    JigPhrase::ActTakeOver,
    JigPhrase::ActSetGoal,
    JigPhrase::ActReorder,
    JigPhrase::ActMarkSeen,
    JigPhrase::ActEditNote,
    JigPhrase::ActDeleteNote,
    JigPhrase::ActAdopt,
    JigPhrase::ActChangePolicy,
    JigPhrase::ActSetRoles,
    JigPhrase::ActMakeService,
    JigPhrase::ActSetPool,
    JigPhrase::ChatAbout,
    JigPhrase::Topic,
    JigPhrase::ElapsedSpan,
    JigPhrase::ReasonField,
    JigPhrase::LoadingPhase,
    JigPhrase::Deadline,
    JigPhrase::StalledOn,
    JigPhrase::DependencyEnded,
    JigPhrase::DriftHold,
    JigPhrase::EffectFailedHold,
    JigPhrase::EffectUncertainHold,
    JigPhrase::BudgetHold,
    JigPhrase::DeadlineHold,
    JigPhrase::StoppedHold,
    JigPhrase::ResourceHold,
    JigPhrase::PoolHold,
    JigPhrase::ProcedureHold,
    JigPhrase::LoadingDependency,
    JigPhrase::BeforeHold,
    JigPhrase::SpentRun,
    JigPhrase::SpentDelegate,
    JigPhrase::Confirmation,
    JigPhrase::Confirm,
    JigPhrase::Dismiss,
    JigPhrase::WillPropose,
    JigPhrase::ChangedConfirmation,
    JigPhrase::MissingWords,
    JigPhrase::MissingChoice,
    JigPhrase::MissingAgain,
    JigPhrase::MissingChange,
    JigPhrase::MissingDelegates,
    JigPhrase::MissingPriority,
    JigPhrase::MissingContext,
    JigPhrase::WillAccept,
    JigPhrase::WillReject,
    JigPhrase::WillPassUp,
    JigPhrase::WillLeaveHeld,
    JigPhrase::WillStop,
    JigPhrase::WillCancel,
    JigPhrase::WillClose,
    JigPhrase::WillTakeOver,
    JigPhrase::WillTake,
    JigPhrase::WillHandBack,
    JigPhrase::WillWrite,
    JigPhrase::WillAnswer,
    JigPhrase::WillAmend,
    JigPhrase::WillAmendRelease,
    JigPhrase::WillPrioritise,
    JigPhrase::WillEditNote,
    JigPhrase::WillDeleteNote,
    JigPhrase::WillGoal,
    JigPhrase::ContinueClosing,
    JigPhrase::ReleaseWait,
    JigPhrase::ReleaseProcedure,
    JigPhrase::RepeatEffect,
    JigPhrase::SkipEffect,
    JigPhrase::DelegateCancel,
    JigPhrase::DelegateTake,
    JigPhrase::DelegateChoices,
    JigPhrase::WriteAndRelease,
    JigPhrase::WriteOnly,
    JigPhrase::NewSpec,
    JigPhrase::NewInstructions,
    JigPhrase::InstructionNotSupplied,
    JigPhrase::InstructionEmpty,
    JigPhrase::NewBudget,
    JigPhrase::NewDeadline,
    JigPhrase::DropDependency,
    JigPhrase::PriorityField,
    JigPhrase::NoteDescription,
    JigPhrase::NoteBody,
    JigPhrase::DocumentTextCut,
    JigPhrase::PersonAddressRevision,
    JigPhrase::ArchiveCumulativeSpend,
    JigPhrase::ArchiveAcceptedAt,
    JigPhrase::ArchiveReadThrough,
    JigPhrase::ArchiveReadAbsent,
    JigPhrase::ResultTimeUnknown,
    JigPhrase::ChangeResultKind,
    JigPhrase::NoteReferencesCut,
    JigPhrase::Charter,
    JigPhrase::CreateProcedure,
    JigPhrase::Priority,
    JigPhrase::Notice,
    JigPhrase::UnknownAction,
    JigPhrase::ResultsThrough,
    JigPhrase::RepliesThrough,
    JigPhrase::AddressParty,
    JigPhrase::AddressTask,
    JigPhrase::AddressRole,
    JigPhrase::NeedsYou,
    JigPhrase::InboxUpdates,
    JigPhrase::AllKinds,
    JigPhrase::AllGoals,
    JigPhrase::OnlyGoal,
    JigPhrase::NewUpdate,
    JigPhrase::OnCall,
    JigPhrase::PassesUp,
    JigPhrase::ReadEarlier,
    JigPhrase::ReadLater,
    JigPhrase::WindowStart,
    JigPhrase::WindowEnd,
    JigPhrase::MoreRows,
    JigPhrase::NewChat,
    JigPhrase::BeginConversation,
    JigPhrase::Delegates,
    JigPhrase::SignInPage,
    JigPhrase::SignInWith,
    JigPhrase::MissingPage,
    JigPhrase::UpdateClient,
    JigPhrase::SomeoneElse,
    JigPhrase::OwnSpend,
    JigPhrase::DelegateSpend,
    JigPhrase::Overview,
    JigPhrase::Activity,
    JigPhrase::Plan,
    JigPhrase::History,
    JigPhrase::BudgetAuthority,
    JigPhrase::Panels,
    JigPhrase::Goal,
    JigPhrase::Resources,
    JigPhrase::Policy,
    JigPhrase::Roles,
    JigPhrase::Spend,
    JigPhrase::Recurring,
    JigPhrase::NextRun,
    JigPhrase::NextPeriod,
    JigPhrase::WaitingRoles,
    JigPhrase::HoldRevision,
    JigPhrase::CurrentPeriod,
    JigPhrase::PeriodBegan,
    JigPhrase::Settings,
    JigPhrase::Goals,
    JigPhrase::Tasks,
    JigPhrase::Notes,
    JigPhrase::Reserved,
    JigPhrase::Left,
    JigPhrase::TotalBudget,
    JigPhrase::OwnBudget,
    JigPhrase::EndedBudget,
    JigPhrase::HeldBelow,
    JigPhrase::FoldSubtree,
    JigPhrase::OpenSubtree,
    JigPhrase::AllPlan,
    JigPhrase::HeldPlan,
    JigPhrase::LivePlan,
    JigPhrase::OwnHistory,
    JigPhrase::SubtreeHistory,
    JigPhrase::BatchMade,
    JigPhrase::MovedWork,
    JigPhrase::GoalWords,
    JigPhrase::GoalBudget,
    JigPhrase::GoalDeadline,
    JigPhrase::GoalPriority,
    JigPhrase::PriorityMeaning,
    JigPhrase::PoolLeft,
    JigPhrase::GoalPropose,
    JigPhrase::NoteSearch,
    JigPhrase::AskedByMe,
    JigPhrase::AllTasks,
    JigPhrase::Period,
    JigPhrase::RoleNumber,
    JigPhrase::PartyNumber,
    JigPhrase::Allotment,
    JigPhrase::DecidesKind,
    JigPhrase::PolicySpend,
    JigPhrase::HostNumber,
    JigPhrase::HostConnected,
    JigPhrase::HostDisconnected,
    JigPhrase::SlotsUsed,
    JigPhrase::SlotsFree,
    JigPhrase::AccountNumber,
    JigPhrase::AccountUsable,
    JigPhrase::AccountUnavailable,
    JigPhrase::Capacity,
    JigPhrase::TasksLive,
    JigPhrase::CapacityLimit,
    JigPhrase::Grace,
    JigPhrase::WaitingAccount,
    JigPhrase::ResourceScope,
    JigPhrase::ResultExpected,
    JigPhrase::ReportExpected,
    JigPhrase::WordsExpected,
    JigPhrase::ApprovalExpected,
    JigPhrase::ProposalsWaiting,
    JigPhrase::ConversationLog,
    JigPhrase::WordsBeingSent,
    JigPhrase::WordsSent,
    JigPhrase::WordsRead,
    JigPhrase::RunResumed,
    JigPhrase::RunFreshTail,
    JigPhrase::RunFinished,
    JigPhrase::RunCancelled,
    JigPhrase::TurnInput,
    JigPhrase::TurnOutput,
    JigPhrase::CacheRead,
    JigPhrase::CacheWritten,
    JigPhrase::TurnSpent,
    JigPhrase::Provisional,
    JigPhrase::TurnUnreadVersion,
    JigPhrase::TurnUnreadMalformed,
    JigPhrase::TurnUnreadTooLarge,
    JigPhrase::TurnCut,
    JigPhrase::OpaqueDialect,
    JigPhrase::OpaqueBytes,
    JigPhrase::CallNumber,
    JigPhrase::OpenCall,
    JigPhrase::FoldCall,
    JigPhrase::CallFailed,
    JigPhrase::CallCompleted,
    JigPhrase::CallPending,
    JigPhrase::NewsTopic,
    JigPhrase::TurnNumber,
    JigPhrase::TurnInstructions,
    JigPhrase::PanelPart,
    JigPhrase::ResultPart,
    JigPhrase::HoldPart,
    JigPhrase::OutboxEntry,
    JigPhrase::TakenByNumber,
    JigPhrase::EngineLimitGoals,
    JigPhrase::MalformedTwoAsAllowed,
    JigPhrase::MalformedConversation,
    JigPhrase::MalformedCycle,
    JigPhrase::MalformedProposedAsAllowed,
    JigPhrase::EngineLimitOperations,
    JigPhrase::MalformedProject,
    JigPhrase::EngineLimitGrace,
    JigPhrase::EngineLimitWords,
    JigPhrase::EngineLimitIdentity,
    JigPhrase::EngineLimitRoles,
    JigPhrase::EngineLimitArithmetic,
    JigPhrase::EngineLimitDuplicate,
    JigPhrase::EngineLimitEmpty,
    JigPhrase::EngineLimitBatch,
    JigPhrase::EngineLimitProjectLiveTasks,
    JigPhrase::EngineLimitLiveTasks,
    JigPhrase::EngineLimitTree,
    JigPhrase::EngineLimitDepth,
    JigPhrase::EngineLimitDelegates,
    JigPhrase::EngineLimitSubscription,
    JigPhrase::EngineLimitDependencies,
    JigPhrase::EngineLimitCycle,
    JigPhrase::EngineLimitSpec,
    JigPhrase::EngineLimitContract,
    JigPhrase::EngineLimitAuthority,
    JigPhrase::EngineLimitInputs,
    JigPhrase::EngineLimitAttempts,
    JigPhrase::EngineLimitLiveDelegates,
    JigPhrase::EngineLimitRestore,
    JigPhrase::EngineLimitRead,
    JigPhrase::EngineLimitTurn,
    JigPhrase::EngineLimitHoldKind,
    JigPhrase::EngineLimitHoldTaken,
    JigPhrase::EngineLimitHolds,
    JigPhrase::EngineLimitInboxFull,
    JigPhrase::EngineLimitNotes,
    JigPhrase::EngineLimitRevision,
    JigPhrase::EngineLimitQuestion,
    JigPhrase::EngineLimitPriority,
    JigPhrase::EngineLimitPolicy,
    JigPhrase::EngineLimitResource,
    JigPhrase::EngineLimitService,
    JigPhrase::EngineLimitAccounts,
    JigPhrase::EngineLimitHolderAccounts,
    JigPhrase::EngineLimitSources,
    JigPhrase::EngineLimitSignIns,
    JigPhrase::EngineLimitPlacedHolds,
    JigPhrase::EngineLimitProjectPlacedHolds,
    JigPhrase::EngineLimitPartyPlacedHolds,
    JigPhrase::EngineLimitExemptions,
    JigPhrase::EngineLimitSettingsProposals,
    JigPhrase::FailedBare,
    JigPhrase::CancelledBare,
    JigPhrase::DecidedBare,
    JigPhrase::DeletedEnding,
    JigPhrase::ByProjectDeployment,
    JigPhrase::NoHeldRole,
    JigPhrase::HeldOwner,
    JigPhrase::HeldMaintainer,
    JigPhrase::HeldMember,
    JigPhrase::HeldObserver,
    JigPhrase::HeldPolicy,
    JigPhrase::DisposalMissing,
    JigPhrase::DisposalUnknown,
    JigPhrase::DisposalRepeated,
    JigPhrase::GrantKindMissing,
    JigPhrase::FundingMissing,
    JigPhrase::DepthMaximum,
    JigPhrase::DeadlineAuthorityMissing,
    JigPhrase::TasksLeft,
    JigPhrase::ToolMissing,
    JigPhrase::ScopeMissing,
    JigPhrase::ModelMissing,
    JigPhrase::ReferenceMissing,
    JigPhrase::ResourceAccessMissing,
    JigPhrase::ResourceUnavailable,
    JigPhrase::AuthorityShapeMissing,
    JigPhrase::AgainRequired,
    JigPhrase::AgainUnexpected,
    JigPhrase::AccountMissing,
    JigPhrase::WriterMissing,
    JigPhrase::RequiredJudgeMissing,
    JigPhrase::FailedJudge,
    JigPhrase::UnguardedJudge,
    JigPhrase::PartConnector,
    JigPhrase::PartResource,
    JigPhrase::PartKind,
    JigPhrase::PartTask,
    JigPhrase::PartProject,
    JigPhrase::PartProposal,
    JigPhrase::FailureRun,
    JigPhrase::FailureAgent,
    JigPhrase::FailureInvalid,
    JigPhrase::RecurringKey,
    JigPhrase::RecurringSkip,
    JigPhrase::RecurringWait,
    JigPhrase::RecurringLastPeriod,
    JigPhrase::RecurringPendingPeriod,
    JigPhrase::RecurringCut,
];
/// Shapes with names bounded by the client's text capacity.
#[must_use]
pub fn shapes(text: u32) -> Box<[PhraseShape]> {
    let mut rows = List::with_capacity(u32::try_from(ALL.len()).expect("phrase count fits"));
    for phrase in ALL {
        rows.push(phrase.shape(text)).expect("all jig shapes fit");
    }
    crate::owner_words::shapes(rows.into_boxed(), text)
}
