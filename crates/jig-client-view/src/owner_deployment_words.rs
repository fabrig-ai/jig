//! Deployment diagnostics sayings (client/domain/view.md, 7).
mod accounts;
mod agents;
mod programs;
use crate::Owner;
pub use accounts::AccountPhrase;
pub use agents::AgentPhrase;
use alloc::boxed::Box;
pub use programs::ProgramPhrase;
use skein_lib::{List, bytes::copy_of};
use skein_ui::phrases::{
    Form, HoleKind, HoleShape, Language, Phrase, PhraseId, PhraseShape, PluralClass, PluralRule, Segment, SpanUnit,
    WallFormat,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeploymentPhrase {
    ProgramDetail(ProgramPhrase),
    Agent(AgentPhrase),
    Deployment,
    Hosts,
    Attempts,
    Accounts,
    Capacity,
    Projects,
    HelloRefusals,
    PersonRefusals,
    TaskRefusals,
    Runs,
    WaitingRuns,
    PersonRefusal,
    TaskRefusal,
    UnknownClock,
    UnknownDuration,
    UnknownChannel,
    UnknownProject,
    UnknownFirst,
    RequestKey,
    Cut,
    Listed,
    NotListed,
    Connected,
    Disconnected,
    Draining,
    NotDraining,
    Usable,
    Unusable,
    Expiring,
    NotExpiring,
    Attention,
    NoAttention,
    TaskProposer,
    PartyProposer,
    Host,
    Account,
    Attempt,
    Task,
    Channel,
    Slots,
    Used,
    Free,
    Limit,
    Kept,
    Generation,
    WaitingAttempts,
    Proposals,
    Project,
    HostsForgotten,
    HelloForgotten,
    PersonForgotten,
    TaskForgotten,
    Person,
    Connector,
    Name,
    Number,
    Question,
    Members,
    First,
    Message,
    Left,
    Right,
    Entry,
    Period,
    Through,
    Chat,
    Messages,
    Bytes,
    At,
    GraceUntil,
    SpentUntil,
    GraceRemaining,
    GraceBound,
    StopBound,
    AllowedGrace,
    Retention,
    TasksCapacity,
    WorkersCapacity,
    AttemptsCapacity,
    ProposalsCapacity,
    ProjectTasksCapacity,
    HostKindWorker,
    HostKindEngine,
    AttemptStateWaiting,
    AttemptStateAdopted,
    AttemptStateClaimed,
    AttemptStateCancelled,
    AttemptStateHanded,
    AttemptStateAcknowledged,
    AttemptStateStray,
    AttemptStateKept,
    AttemptStateFenced,
    AttemptStateClosed,
    RunLocationNone,
    RunLocationConnected,
    RunLocationAdrift,
    AccountStateGranted,
    AccountStateAsking,
    AccountStateSignInAgain,
    AccountStateMisconfigured,
    AccountDetail(AccountPhrase),
    AccountStateClosing,
    AccountStateMoving,
    HelloRefusalReservedChannel,
    HelloRefusalWorkers,
    HelloRefusalPosition,
    HelloRefusalStopBound,
    PersonCapacityActionCreate,
    PersonCapacityActionCompound,
    PersonCapacityActionApplication,
    PersonCapacityActionNote,
    PersonCapacityActionService,
    PersonCapacityActionAdoption,
    PersonCapacityActionRoles,
    PersonCapacityActionPolicy,
    PersonCapacityActionPool,
    PersonCapacityActionEscalation,
    PersonCapacityActionProposal,
    PersonCapacityActionWords,
    PersonCapacityActionQuestion,
    PersonCapacityActionAmend,
    PersonCapacityActionMove,
    PersonCapacityActionTakePerson,
    PersonCapacityActionHandBackPerson,
    PersonCapacityActionAnswerPerson,
    PersonCapacityActionStop,
    PersonCapacityActionCancel,
    PersonCapacityActionRelease,
    PersonCapacityActionMarkSeen,
    PersonCapacityActionAccountSignIn,
    PersonCapacityActionAccountSignOut,
    PersonCapacityActionAccountChange,
    PersonCapacityCauseRequests,
    PersonCapacityCausePending,
    PersonCapacityCauseWaiters,
    PersonCapacityCauseLimit,
    TaskCapacityActionBatch,
    TaskCapacityActionProposal,
    TaskCapacityActionPersonProposal,
    TaskCapacityActionMessage,
    TaskCapacityActionSource,
    TaskCapacityActionIntroduce,
    TaskCapacityActionSubscription,
    TaskCapacityActionEffect,
    TaskCapacityActionClaim,
    TaskCapacityActionRelease,
    TaskCapacityActionMove,
    TaskCapacityCauseTasks,
    TaskCapacityCauseCreationFlights,
    TaskCapacityCauseResourceReports,
    TaskCapacityCauseProjectTasks,
    TaskCapacityCauseTree,
    TaskCapacityCauseDelegates,
    TaskCapacityCauseInbox,
    TaskCapacityCauseHoldings,
    TaskCapacityCauseHoldWaiters,
    TaskCapacityCauseProposals,
    TaskCapacityCauseReferences,
    TaskCapacityCauseSubscriptions,
    TaskCapacityCauseFunders,
    TaskCapacityCauseWriters,
    FunderPool,
    FunderRecurring,
    FunderPeriod,
    FunderTask,
    AdmissionGoals,
    AdmissionOperations,
    AdmissionGrace,
    AdmissionWords,
    AdmissionIdentity,
    AdmissionRoles,
    AdmissionArithmetic,
    AdmissionDuplicate,
    AdmissionEmpty,
    AdmissionBatch,
    AdmissionProjectLiveTasks,
    AdmissionLiveTasks,
    AdmissionTree,
    AdmissionDepth,
    AdmissionDelegates,
    AdmissionSubscription,
    AdmissionDependencies,
    AdmissionCycle,
    AdmissionSpec,
    AdmissionContract,
    AdmissionAuthority,
    AdmissionInputs,
    AdmissionAttempts,
    AdmissionLiveDelegates,
    AdmissionRestore,
    AdmissionRead,
    AdmissionTurn,
    AdmissionHoldKind,
    AdmissionHoldTaken,
    AdmissionHolds,
    AdmissionInboxFull,
    AdmissionNotes,
    AdmissionRevision,
    AdmissionQuestion,
    AdmissionPriority,
    AdmissionPolicy,
    AdmissionResource,
    AdmissionService,
    AdmissionAccounts,
    AdmissionHolderAccounts,
    AdmissionSources,
    AdmissionSignIns,
    AdmissionSettingsProposals,
    AdmissionProjects,

    Connectors,
    ConnectorsCut,
    ConnectorsEmpty,
    StoreBytes,
    StoreMark,
}
pub const ALL: [DeploymentPhrase; 286] = [
    DeploymentPhrase::Agent(AgentPhrase::Model),
    DeploymentPhrase::Agent(AgentPhrase::Window),
    DeploymentPhrase::Agent(AgentPhrase::Output),
    DeploymentPhrase::Agent(AgentPhrase::Turns),
    DeploymentPhrase::Agent(AgentPhrase::Spend),
    DeploymentPhrase::Agent(AgentPhrase::Time),
    DeploymentPhrase::Agent(AgentPhrase::Waiting),
    DeploymentPhrase::Agent(AgentPhrase::Checks),
    DeploymentPhrase::Agent(AgentPhrase::Message),
    DeploymentPhrase::Agent(AgentPhrase::Inbox),
    DeploymentPhrase::Agent(AgentPhrase::Unacknowledged),
    DeploymentPhrase::Agent(AgentPhrase::ModelPosition),
    DeploymentPhrase::Deployment,
    DeploymentPhrase::Hosts,
    DeploymentPhrase::Attempts,
    DeploymentPhrase::Accounts,
    DeploymentPhrase::Capacity,
    DeploymentPhrase::Projects,
    DeploymentPhrase::HelloRefusals,
    DeploymentPhrase::PersonRefusals,
    DeploymentPhrase::TaskRefusals,
    DeploymentPhrase::Runs,
    DeploymentPhrase::WaitingRuns,
    DeploymentPhrase::PersonRefusal,
    DeploymentPhrase::TaskRefusal,
    DeploymentPhrase::UnknownClock,
    DeploymentPhrase::UnknownDuration,
    DeploymentPhrase::UnknownChannel,
    DeploymentPhrase::UnknownProject,
    DeploymentPhrase::UnknownFirst,
    DeploymentPhrase::RequestKey,
    DeploymentPhrase::Cut,
    DeploymentPhrase::Listed,
    DeploymentPhrase::NotListed,
    DeploymentPhrase::Connected,
    DeploymentPhrase::Disconnected,
    DeploymentPhrase::Draining,
    DeploymentPhrase::NotDraining,
    DeploymentPhrase::Usable,
    DeploymentPhrase::Unusable,
    DeploymentPhrase::Expiring,
    DeploymentPhrase::NotExpiring,
    DeploymentPhrase::Attention,
    DeploymentPhrase::NoAttention,
    DeploymentPhrase::TaskProposer,
    DeploymentPhrase::PartyProposer,
    DeploymentPhrase::Host,
    DeploymentPhrase::Account,
    DeploymentPhrase::Attempt,
    DeploymentPhrase::Task,
    DeploymentPhrase::Channel,
    DeploymentPhrase::Slots,
    DeploymentPhrase::Used,
    DeploymentPhrase::Free,
    DeploymentPhrase::Limit,
    DeploymentPhrase::Kept,
    DeploymentPhrase::Generation,
    DeploymentPhrase::WaitingAttempts,
    DeploymentPhrase::Proposals,
    DeploymentPhrase::Project,
    DeploymentPhrase::HostsForgotten,
    DeploymentPhrase::HelloForgotten,
    DeploymentPhrase::PersonForgotten,
    DeploymentPhrase::TaskForgotten,
    DeploymentPhrase::Person,
    DeploymentPhrase::Connector,
    DeploymentPhrase::Name,
    DeploymentPhrase::Number,
    DeploymentPhrase::Question,
    DeploymentPhrase::Members,
    DeploymentPhrase::First,
    DeploymentPhrase::Message,
    DeploymentPhrase::Left,
    DeploymentPhrase::Right,
    DeploymentPhrase::Entry,
    DeploymentPhrase::Period,
    DeploymentPhrase::Through,
    DeploymentPhrase::Chat,
    DeploymentPhrase::Messages,
    DeploymentPhrase::Bytes,
    DeploymentPhrase::At,
    DeploymentPhrase::GraceUntil,
    DeploymentPhrase::SpentUntil,
    DeploymentPhrase::GraceRemaining,
    DeploymentPhrase::GraceBound,
    DeploymentPhrase::StopBound,
    DeploymentPhrase::AllowedGrace,
    DeploymentPhrase::Retention,
    DeploymentPhrase::TasksCapacity,
    DeploymentPhrase::WorkersCapacity,
    DeploymentPhrase::AttemptsCapacity,
    DeploymentPhrase::ProposalsCapacity,
    DeploymentPhrase::ProjectTasksCapacity,
    DeploymentPhrase::HostKindWorker,
    DeploymentPhrase::HostKindEngine,
    DeploymentPhrase::AttemptStateWaiting,
    DeploymentPhrase::AttemptStateAdopted,
    DeploymentPhrase::AttemptStateClaimed,
    DeploymentPhrase::AttemptStateCancelled,
    DeploymentPhrase::AttemptStateHanded,
    DeploymentPhrase::AttemptStateAcknowledged,
    DeploymentPhrase::AttemptStateStray,
    DeploymentPhrase::AttemptStateKept,
    DeploymentPhrase::AttemptStateFenced,
    DeploymentPhrase::AttemptStateClosed,
    DeploymentPhrase::RunLocationNone,
    DeploymentPhrase::RunLocationConnected,
    DeploymentPhrase::RunLocationAdrift,
    DeploymentPhrase::AccountStateGranted,
    DeploymentPhrase::AccountStateAsking,
    DeploymentPhrase::AccountStateSignInAgain,
    DeploymentPhrase::AccountStateMisconfigured,
    DeploymentPhrase::AccountDetail(AccountPhrase::Later),
    DeploymentPhrase::AccountDetail(AccountPhrase::LaterUnknown),
    DeploymentPhrase::AccountDetail(AccountPhrase::Unloaded),
    DeploymentPhrase::AccountDetail(AccountPhrase::Cancelled),
    DeploymentPhrase::AccountDetail(AccountPhrase::Directory),
    DeploymentPhrase::AccountDetail(AccountPhrase::File),
    DeploymentPhrase::AccountDetail(AccountPhrase::Unreadable),
    DeploymentPhrase::AccountDetail(AccountPhrase::Refused),
    DeploymentPhrase::AccountDetail(AccountPhrase::Failed),
    DeploymentPhrase::AccountDetail(AccountPhrase::Stalled),
    DeploymentPhrase::AccountDetail(AccountPhrase::TooLarge),
    DeploymentPhrase::AccountDetail(AccountPhrase::Provider),
    DeploymentPhrase::AccountDetail(AccountPhrase::HeldByDeployment),
    DeploymentPhrase::AccountDetail(AccountPhrase::HeldByParty),
    DeploymentPhrase::AccountDetail(AccountPhrase::Plan),
    DeploymentPhrase::AccountDetail(AccountPhrase::Sources),
    DeploymentPhrase::AccountDetail(AccountPhrase::Source),
    DeploymentPhrase::AccountDetail(AccountPhrase::SignIn),
    DeploymentPhrase::AccountDetail(AccountPhrase::Key),
    DeploymentPhrase::AccountDetail(AccountPhrase::Program),
    DeploymentPhrase::AccountDetail(AccountPhrase::None),
    DeploymentPhrase::AccountDetail(AccountPhrase::DefaultUses),
    DeploymentPhrase::AccountDetail(AccountPhrase::PaysParty),
    DeploymentPhrase::AccountDetail(AccountPhrase::PaysProject),
    DeploymentPhrase::AccountDetail(AccountPhrase::ManageUsage),
    DeploymentPhrase::AccountDetail(AccountPhrase::Catalogue),
    DeploymentPhrase::AccountDetail(AccountPhrase::KeyBytes),
    DeploymentPhrase::AccountDetail(AccountPhrase::Ready),
    DeploymentPhrase::AccountDetail(AccountPhrase::Unavailable),
    DeploymentPhrase::AccountDetail(AccountPhrase::UnknownReadiness),
    DeploymentPhrase::AccountDetail(AccountPhrase::Settled),
    DeploymentPhrase::AccountDetail(AccountPhrase::Pending),
    DeploymentPhrase::AccountDetail(AccountPhrase::UnknownSettlement),
    DeploymentPhrase::AccountDetail(AccountPhrase::OffersCut),
    DeploymentPhrase::AccountDetail(AccountPhrase::ReadinessCut),
    DeploymentPhrase::AccountDetail(AccountPhrase::SignInPlan),
    DeploymentPhrase::AccountDetail(AccountPhrase::RefusalNotSignedIn),
    DeploymentPhrase::AccountDetail(AccountPhrase::RefusalNotOurs),
    DeploymentPhrase::AccountDetail(AccountPhrase::RefusalCancelled),
    DeploymentPhrase::AccountDetail(AccountPhrase::RefusalMoving),
    DeploymentPhrase::AccountStateClosing,
    DeploymentPhrase::AccountStateMoving,
    DeploymentPhrase::HelloRefusalReservedChannel,
    DeploymentPhrase::HelloRefusalWorkers,
    DeploymentPhrase::HelloRefusalPosition,
    DeploymentPhrase::HelloRefusalStopBound,
    DeploymentPhrase::PersonCapacityActionCreate,
    DeploymentPhrase::PersonCapacityActionCompound,
    DeploymentPhrase::PersonCapacityActionApplication,
    DeploymentPhrase::PersonCapacityActionNote,
    DeploymentPhrase::PersonCapacityActionService,
    DeploymentPhrase::PersonCapacityActionAdoption,
    DeploymentPhrase::PersonCapacityActionRoles,
    DeploymentPhrase::PersonCapacityActionPolicy,
    DeploymentPhrase::PersonCapacityActionPool,
    DeploymentPhrase::PersonCapacityActionEscalation,
    DeploymentPhrase::PersonCapacityActionProposal,
    DeploymentPhrase::PersonCapacityActionWords,
    DeploymentPhrase::PersonCapacityActionQuestion,
    DeploymentPhrase::PersonCapacityActionAmend,
    DeploymentPhrase::PersonCapacityActionMove,
    DeploymentPhrase::PersonCapacityActionTakePerson,
    DeploymentPhrase::PersonCapacityActionHandBackPerson,
    DeploymentPhrase::PersonCapacityActionAnswerPerson,
    DeploymentPhrase::PersonCapacityActionStop,
    DeploymentPhrase::PersonCapacityActionCancel,
    DeploymentPhrase::PersonCapacityActionRelease,
    DeploymentPhrase::PersonCapacityActionMarkSeen,
    DeploymentPhrase::PersonCapacityActionAccountSignIn,
    DeploymentPhrase::PersonCapacityActionAccountSignOut,
    DeploymentPhrase::PersonCapacityActionAccountChange,
    DeploymentPhrase::PersonCapacityCauseRequests,
    DeploymentPhrase::PersonCapacityCausePending,
    DeploymentPhrase::PersonCapacityCauseWaiters,
    DeploymentPhrase::PersonCapacityCauseLimit,
    DeploymentPhrase::TaskCapacityActionBatch,
    DeploymentPhrase::TaskCapacityActionProposal,
    DeploymentPhrase::TaskCapacityActionPersonProposal,
    DeploymentPhrase::TaskCapacityActionMessage,
    DeploymentPhrase::TaskCapacityActionSource,
    DeploymentPhrase::TaskCapacityActionIntroduce,
    DeploymentPhrase::TaskCapacityActionSubscription,
    DeploymentPhrase::TaskCapacityActionEffect,
    DeploymentPhrase::TaskCapacityActionClaim,
    DeploymentPhrase::TaskCapacityActionRelease,
    DeploymentPhrase::TaskCapacityActionMove,
    DeploymentPhrase::TaskCapacityCauseTasks,
    DeploymentPhrase::TaskCapacityCauseCreationFlights,
    DeploymentPhrase::TaskCapacityCauseResourceReports,
    DeploymentPhrase::TaskCapacityCauseProjectTasks,
    DeploymentPhrase::TaskCapacityCauseTree,
    DeploymentPhrase::TaskCapacityCauseDelegates,
    DeploymentPhrase::TaskCapacityCauseInbox,
    DeploymentPhrase::TaskCapacityCauseHoldings,
    DeploymentPhrase::TaskCapacityCauseHoldWaiters,
    DeploymentPhrase::TaskCapacityCauseProposals,
    DeploymentPhrase::TaskCapacityCauseReferences,
    DeploymentPhrase::TaskCapacityCauseSubscriptions,
    DeploymentPhrase::TaskCapacityCauseFunders,
    DeploymentPhrase::TaskCapacityCauseWriters,
    DeploymentPhrase::FunderPool,
    DeploymentPhrase::FunderRecurring,
    DeploymentPhrase::FunderPeriod,
    DeploymentPhrase::FunderTask,
    DeploymentPhrase::AdmissionGoals,
    DeploymentPhrase::AdmissionOperations,
    DeploymentPhrase::AdmissionGrace,
    DeploymentPhrase::AdmissionWords,
    DeploymentPhrase::AdmissionIdentity,
    DeploymentPhrase::AdmissionRoles,
    DeploymentPhrase::AdmissionArithmetic,
    DeploymentPhrase::AdmissionDuplicate,
    DeploymentPhrase::AdmissionEmpty,
    DeploymentPhrase::AdmissionBatch,
    DeploymentPhrase::AdmissionProjectLiveTasks,
    DeploymentPhrase::AdmissionLiveTasks,
    DeploymentPhrase::AdmissionTree,
    DeploymentPhrase::AdmissionDepth,
    DeploymentPhrase::AdmissionDelegates,
    DeploymentPhrase::AdmissionSubscription,
    DeploymentPhrase::AdmissionDependencies,
    DeploymentPhrase::AdmissionCycle,
    DeploymentPhrase::AdmissionSpec,
    DeploymentPhrase::AdmissionContract,
    DeploymentPhrase::AdmissionAuthority,
    DeploymentPhrase::AdmissionInputs,
    DeploymentPhrase::AdmissionAttempts,
    DeploymentPhrase::AdmissionLiveDelegates,
    DeploymentPhrase::AdmissionRestore,
    DeploymentPhrase::AdmissionRead,
    DeploymentPhrase::AdmissionTurn,
    DeploymentPhrase::AdmissionHoldKind,
    DeploymentPhrase::AdmissionHoldTaken,
    DeploymentPhrase::AdmissionHolds,
    DeploymentPhrase::AdmissionInboxFull,
    DeploymentPhrase::AdmissionNotes,
    DeploymentPhrase::AdmissionRevision,
    DeploymentPhrase::AdmissionQuestion,
    DeploymentPhrase::AdmissionPriority,
    DeploymentPhrase::AdmissionPolicy,
    DeploymentPhrase::AdmissionResource,
    DeploymentPhrase::AdmissionService,
    DeploymentPhrase::AdmissionAccounts,
    DeploymentPhrase::AdmissionHolderAccounts,
    DeploymentPhrase::AdmissionSources,
    DeploymentPhrase::AdmissionSignIns,
    DeploymentPhrase::AdmissionSettingsProposals,
    DeploymentPhrase::AdmissionProjects,
    DeploymentPhrase::Connectors,
    DeploymentPhrase::ConnectorsCut,
    DeploymentPhrase::ConnectorsEmpty,
    DeploymentPhrase::StoreBytes,
    DeploymentPhrase::StoreMark,
    DeploymentPhrase::ProgramDetail(ProgramPhrase::Programs),
    DeploymentPhrase::ProgramDetail(ProgramPhrase::Awaiting),
    DeploymentPhrase::ProgramDetail(ProgramPhrase::Received),
    DeploymentPhrase::ProgramDetail(ProgramPhrase::Usable),
    DeploymentPhrase::ProgramDetail(ProgramPhrase::NotSignedIn),
    DeploymentPhrase::ProgramDetail(ProgramPhrase::Otherwise),
    DeploymentPhrase::ProgramDetail(ProgramPhrase::Failed),
    DeploymentPhrase::ProgramDetail(ProgramPhrase::SignedOut),
    DeploymentPhrase::ProgramDetail(ProgramPhrase::SignOutFailed),
    DeploymentPhrase::ProgramDetail(ProgramPhrase::Cleanup),
    DeploymentPhrase::ProgramDetail(ProgramPhrase::Missing),
    DeploymentPhrase::ProgramDetail(ProgramPhrase::Deadline),
    DeploymentPhrase::ProgramDetail(ProgramPhrase::Unreadable),
    DeploymentPhrase::ProgramDetail(ProgramPhrase::Exited),
    DeploymentPhrase::ProgramDetail(ProgramPhrase::Unconfigured),
    DeploymentPhrase::ProgramDetail(ProgramPhrase::Unknown),
    DeploymentPhrase::ProgramDetail(ProgramPhrase::Closed),
    DeploymentPhrase::ProgramDetail(ProgramPhrase::StatusUnknown),
    DeploymentPhrase::ProgramDetail(ProgramPhrase::OnlyPerson),
    DeploymentPhrase::ProgramDetail(ProgramPhrase::SeveralPeople),
    DeploymentPhrase::ProgramDetail(ProgramPhrase::UnnamedLogin),
];
impl DeploymentPhrase {
    #[must_use]
    #[expect(clippy::too_many_lines, reason = "the closed deployment vocabulary has one exhaustive mapping table")]
    pub const fn id(self) -> PhraseId {
        PhraseId {
            owner: Owner::Jig.opaque(),
            number: match self {
                Self::Deployment => 9200,
                Self::Hosts => 9201,
                Self::Attempts => 9202,
                Self::Accounts => 9203,
                Self::Capacity => 9204,
                Self::Projects => 9205,
                Self::HelloRefusals => 9206,
                Self::PersonRefusals => 9207,
                Self::TaskRefusals => 9208,
                Self::Runs => 9209,
                Self::WaitingRuns => 9210,
                Self::PersonRefusal => 9211,
                Self::TaskRefusal => 9212,
                Self::UnknownClock => 9213,
                Self::UnknownDuration => 9214,
                Self::UnknownChannel => 9215,
                Self::UnknownProject => 9216,
                Self::UnknownFirst => 9217,
                Self::RequestKey => 9218,
                Self::Cut => 9219,
                Self::Listed => 9220,
                Self::NotListed => 9221,
                Self::Connected => 9222,
                Self::Disconnected => 9223,
                Self::Draining => 9224,
                Self::NotDraining => 9225,
                Self::Usable => 9226,
                Self::Unusable => 9227,
                Self::Expiring => 9228,
                Self::NotExpiring => 9229,
                Self::Attention => 9230,
                Self::NoAttention => 9231,
                Self::TaskProposer => 9232,
                Self::PartyProposer => 9233,
                Self::Host => 9234,
                Self::Account => 9235,
                Self::Attempt => 9236,
                Self::Task => 9237,
                Self::Channel => 9238,
                Self::Slots => 9239,
                Self::Used => 9240,
                Self::Free => 9241,
                Self::Limit => 9242,
                Self::Kept => 9243,
                Self::Generation => 9244,
                Self::WaitingAttempts => 9245,
                Self::Proposals => 9246,
                Self::Project => 9247,
                Self::HostsForgotten => 9248,
                Self::HelloForgotten => 9249,
                Self::PersonForgotten => 9250,
                Self::TaskForgotten => 9251,
                Self::Person => 9252,
                Self::Connector => 9253,
                Self::Name => 9254,
                Self::Number => 9255,
                Self::Question => 9256,
                Self::Members => 9257,
                Self::First => 9258,
                Self::Message => 9259,
                Self::Left => 9260,
                Self::Right => 9261,
                Self::Entry => 9262,
                Self::Period => 9263,
                Self::Through => 9264,
                Self::Chat => 9265,
                Self::Messages => 9266,
                Self::Bytes => 9267,
                Self::At => 9268,
                Self::GraceUntil => 9269,
                Self::SpentUntil => 9270,
                Self::GraceRemaining => 9271,
                Self::GraceBound => 9272,
                Self::StopBound => 9273,
                Self::AllowedGrace => 9274,
                Self::Retention => 9275,
                Self::TasksCapacity => 9276,
                Self::WorkersCapacity => 9277,
                Self::AttemptsCapacity => 9278,
                Self::ProposalsCapacity => 9279,
                Self::ProjectTasksCapacity => 9280,
                Self::HostKindWorker => 9281,
                Self::HostKindEngine => 9282,
                Self::AttemptStateWaiting => 9283,
                Self::AttemptStateAdopted => 9284,
                Self::AttemptStateClaimed => 9285,
                Self::AttemptStateCancelled => 9286,
                Self::AttemptStateHanded => 9287,
                Self::AttemptStateAcknowledged => 9288,
                Self::AttemptStateStray => 9289,
                Self::AttemptStateKept => 9290,
                Self::AttemptStateFenced => 9291,
                Self::AttemptStateClosed => 9292,
                Self::RunLocationNone => 9293,
                Self::RunLocationConnected => 9294,
                Self::RunLocationAdrift => 9295,
                Self::AccountStateGranted => 9296,
                Self::AccountStateAsking => 9297,
                Self::AccountStateSignInAgain => 9298,
                Self::AccountStateMisconfigured => 9299,
                Self::AccountDetail(phrase) => phrase.number(),
                Self::Agent(phrase) => phrase.number(),
                Self::ProgramDetail(phrase) => phrase.number(),
                Self::AccountStateClosing => 9300,
                Self::AccountStateMoving => 9485,
                Self::HelloRefusalReservedChannel => 9301,
                Self::HelloRefusalWorkers => 9302,
                Self::HelloRefusalPosition => 9718,
                Self::HelloRefusalStopBound => 9303,
                Self::PersonCapacityActionCreate => 9422,
                Self::PersonCapacityActionCompound => 9423,
                Self::PersonCapacityActionApplication => 9305,
                Self::PersonCapacityActionNote => 9306,
                Self::PersonCapacityActionService => 9307,
                Self::PersonCapacityActionAdoption => 9308,
                Self::PersonCapacityActionRoles => 9309,
                Self::PersonCapacityActionPolicy => 9310,
                Self::PersonCapacityActionPool => 9311,
                Self::PersonCapacityActionEscalation => 9314,
                Self::PersonCapacityActionProposal => 9315,
                Self::PersonCapacityActionWords => 9316,
                Self::PersonCapacityActionQuestion => 9317,
                Self::PersonCapacityActionAmend => 9319,
                Self::PersonCapacityActionMove => 9320,
                Self::PersonCapacityActionTakePerson => 9321,
                Self::PersonCapacityActionHandBackPerson => 9322,
                Self::PersonCapacityActionAnswerPerson => 9323,
                Self::PersonCapacityActionStop => 9324,
                Self::PersonCapacityActionCancel => 9325,
                Self::PersonCapacityActionRelease => 9326,
                Self::PersonCapacityActionMarkSeen => 9327,
                Self::PersonCapacityActionAccountSignIn => 9470,
                Self::PersonCapacityActionAccountSignOut => 9471,
                Self::PersonCapacityActionAccountChange => 9472,
                Self::PersonCapacityCauseRequests => 9328,
                Self::PersonCapacityCausePending => 9329,
                Self::PersonCapacityCauseWaiters => 9330,
                Self::PersonCapacityCauseLimit => 9331,
                Self::TaskCapacityActionBatch => 9332,
                Self::TaskCapacityActionProposal => 9333,
                Self::TaskCapacityActionPersonProposal => 9334,
                Self::TaskCapacityActionMessage => 9335,
                Self::TaskCapacityActionSource => 9336,
                Self::TaskCapacityActionIntroduce => 9337,
                Self::TaskCapacityActionSubscription => 9338,
                Self::TaskCapacityActionEffect => 9339,
                Self::TaskCapacityActionClaim => 9340,
                Self::TaskCapacityActionRelease => 9341,
                Self::TaskCapacityActionMove => 9342,
                Self::TaskCapacityCauseTasks => 9343,
                Self::TaskCapacityCauseCreationFlights => 9344,
                Self::TaskCapacityCauseResourceReports => 9345,
                Self::TaskCapacityCauseProjectTasks => 9346,
                Self::TaskCapacityCauseTree => 9347,
                Self::TaskCapacityCauseDelegates => 9348,
                Self::TaskCapacityCauseInbox => 9349,
                Self::TaskCapacityCauseHoldings => 9350,
                Self::TaskCapacityCauseHoldWaiters => 9351,
                Self::TaskCapacityCauseProposals => 9352,
                Self::TaskCapacityCauseReferences => 9353,
                Self::TaskCapacityCauseSubscriptions => 9354,
                Self::TaskCapacityCauseFunders => 9355,
                Self::TaskCapacityCauseWriters => 9356,
                Self::FunderPool => 9357,
                Self::FunderRecurring => 9358,
                Self::FunderPeriod => 9359,
                Self::FunderTask => 9360,
                Self::AdmissionGoals => 12003,
                Self::AdmissionOperations => 12002,
                Self::AdmissionGrace => 12001,
                Self::AdmissionWords => 9361,
                Self::AdmissionIdentity => 9362,
                Self::AdmissionRoles => 9363,
                Self::AdmissionArithmetic => 9364,
                Self::AdmissionDuplicate => 9365,
                Self::AdmissionEmpty => 9366,
                Self::AdmissionBatch => 9367,
                Self::AdmissionProjectLiveTasks => 9368,
                Self::AdmissionLiveTasks => 9369,
                Self::AdmissionTree => 9370,
                Self::AdmissionDepth => 9371,
                Self::AdmissionDelegates => 9372,
                Self::AdmissionSubscription => 9373,
                Self::AdmissionDependencies => 9374,
                Self::AdmissionCycle => 9375,
                Self::AdmissionSpec => 9376,
                Self::AdmissionContract => 9377,
                Self::AdmissionAuthority => 9378,
                Self::AdmissionInputs => 9379,
                Self::AdmissionAttempts => 9380,
                Self::AdmissionLiveDelegates => 9381,
                Self::AdmissionRestore => 9382,
                Self::AdmissionRead => 9383,
                Self::AdmissionTurn => 9384,
                Self::AdmissionHoldKind => 9385,
                Self::AdmissionHoldTaken => 9386,
                Self::AdmissionHolds => 9387,
                Self::AdmissionInboxFull => 9388,
                Self::AdmissionNotes => 9389,
                Self::AdmissionRevision => 9390,
                Self::AdmissionQuestion => 9391,
                Self::AdmissionPriority => 9392,
                Self::AdmissionPolicy => 9393,
                Self::AdmissionResource => 9394,
                Self::AdmissionService => 9395,
                Self::AdmissionAccounts => 9425,
                Self::AdmissionHolderAccounts => 9426,
                Self::AdmissionSources => 9427,
                Self::AdmissionSignIns => 9428,
                Self::AdmissionSettingsProposals => 9431,
                Self::AdmissionProjects => 9430,

                Self::Connectors => 9396,
                Self::ConnectorsCut => 9397,
                Self::ConnectorsEmpty => 9398,
                Self::StoreBytes => 9399,
                Self::StoreMark => 9420,
            },
        }
    }
    #[expect(clippy::too_many_lines, reason = "the closed deployment vocabulary has one exhaustive mapping table")]
    fn hole(self) -> Option<HoleKind> {
        match self {
            Self::Agent(phrase) => phrase.hole(),
            Self::ProgramDetail(phrase) => phrase.hole(),
            Self::AccountDetail(phrase) => phrase.hole(),
            Self::At | Self::GraceUntil | Self::SpentUntil => Some(HoleKind::Wall),
            Self::Deployment
            | Self::Hosts
            | Self::Attempts
            | Self::Accounts
            | Self::Capacity
            | Self::Projects
            | Self::HelloRefusals
            | Self::PersonRefusals
            | Self::TaskRefusals
            | Self::Runs
            | Self::WaitingRuns
            | Self::PersonRefusal
            | Self::TaskRefusal
            | Self::UnknownClock
            | Self::UnknownDuration
            | Self::UnknownChannel
            | Self::UnknownProject
            | Self::UnknownFirst
            | Self::RequestKey
            | Self::Cut
            | Self::Listed
            | Self::NotListed
            | Self::Connected
            | Self::Disconnected
            | Self::Draining
            | Self::NotDraining
            | Self::Usable
            | Self::Unusable
            | Self::Expiring
            | Self::NotExpiring
            | Self::Attention
            | Self::NoAttention
            | Self::TasksCapacity
            | Self::WorkersCapacity
            | Self::AttemptsCapacity
            | Self::ProposalsCapacity
            | Self::ProjectTasksCapacity
            | Self::HostKindWorker
            | Self::HostKindEngine
            | Self::AttemptStateWaiting
            | Self::AttemptStateAdopted
            | Self::AttemptStateClaimed
            | Self::AttemptStateCancelled
            | Self::AttemptStateHanded
            | Self::AttemptStateAcknowledged
            | Self::AttemptStateStray
            | Self::AttemptStateKept
            | Self::AttemptStateFenced
            | Self::AttemptStateClosed
            | Self::RunLocationNone
            | Self::RunLocationConnected
            | Self::RunLocationAdrift
            | Self::AccountStateGranted
            | Self::AccountStateAsking
            | Self::AccountStateSignInAgain
            | Self::AccountStateMisconfigured
            | Self::AccountStateClosing
            | Self::AccountStateMoving
            | Self::HelloRefusalReservedChannel
            | Self::HelloRefusalWorkers
            | Self::HelloRefusalStopBound
            | Self::PersonCapacityActionCreate
            | Self::PersonCapacityActionCompound
            | Self::PersonCapacityActionApplication
            | Self::PersonCapacityActionNote
            | Self::PersonCapacityActionService
            | Self::PersonCapacityActionAdoption
            | Self::PersonCapacityActionRoles
            | Self::PersonCapacityActionPolicy
            | Self::PersonCapacityActionPool
            | Self::PersonCapacityActionEscalation
            | Self::PersonCapacityActionProposal
            | Self::PersonCapacityActionWords
            | Self::PersonCapacityActionQuestion
            | Self::PersonCapacityActionAmend
            | Self::PersonCapacityActionMove
            | Self::PersonCapacityActionTakePerson
            | Self::PersonCapacityActionHandBackPerson
            | Self::PersonCapacityActionAnswerPerson
            | Self::PersonCapacityActionStop
            | Self::PersonCapacityActionCancel
            | Self::PersonCapacityActionRelease
            | Self::PersonCapacityActionMarkSeen
            | Self::PersonCapacityActionAccountSignIn
            | Self::PersonCapacityActionAccountSignOut
            | Self::PersonCapacityActionAccountChange
            | Self::PersonCapacityCauseRequests
            | Self::PersonCapacityCausePending
            | Self::PersonCapacityCauseWaiters
            | Self::PersonCapacityCauseLimit
            | Self::TaskCapacityActionBatch
            | Self::TaskCapacityActionProposal
            | Self::TaskCapacityActionPersonProposal
            | Self::TaskCapacityActionMessage
            | Self::TaskCapacityActionSource
            | Self::TaskCapacityActionIntroduce
            | Self::TaskCapacityActionSubscription
            | Self::TaskCapacityActionEffect
            | Self::TaskCapacityActionClaim
            | Self::TaskCapacityActionRelease
            | Self::TaskCapacityActionMove
            | Self::TaskCapacityCauseTasks
            | Self::TaskCapacityCauseCreationFlights
            | Self::TaskCapacityCauseResourceReports
            | Self::TaskCapacityCauseProjectTasks
            | Self::TaskCapacityCauseTree
            | Self::TaskCapacityCauseDelegates
            | Self::TaskCapacityCauseInbox
            | Self::TaskCapacityCauseHoldings
            | Self::TaskCapacityCauseHoldWaiters
            | Self::TaskCapacityCauseProposals
            | Self::TaskCapacityCauseReferences
            | Self::TaskCapacityCauseSubscriptions
            | Self::TaskCapacityCauseFunders
            | Self::TaskCapacityCauseWriters
            | Self::FunderPool
            | Self::FunderRecurring
            | Self::FunderPeriod
            | Self::FunderTask
            | Self::AdmissionGoals
            | Self::AdmissionOperations
            | Self::AdmissionGrace
            | Self::AdmissionWords
            | Self::AdmissionIdentity
            | Self::AdmissionRoles
            | Self::AdmissionArithmetic
            | Self::AdmissionDuplicate
            | Self::AdmissionEmpty
            | Self::AdmissionBatch
            | Self::AdmissionProjectLiveTasks
            | Self::AdmissionLiveTasks
            | Self::AdmissionTree
            | Self::AdmissionDepth
            | Self::AdmissionDelegates
            | Self::AdmissionSubscription
            | Self::AdmissionDependencies
            | Self::AdmissionCycle
            | Self::AdmissionSpec
            | Self::AdmissionContract
            | Self::AdmissionAuthority
            | Self::AdmissionInputs
            | Self::AdmissionAttempts
            | Self::AdmissionLiveDelegates
            | Self::AdmissionRestore
            | Self::AdmissionRead
            | Self::AdmissionTurn
            | Self::AdmissionHoldKind
            | Self::AdmissionHoldTaken
            | Self::AdmissionHolds
            | Self::AdmissionInboxFull
            | Self::AdmissionNotes
            | Self::AdmissionRevision
            | Self::AdmissionQuestion
            | Self::AdmissionPriority
            | Self::AdmissionPolicy
            | Self::AdmissionResource
            | Self::AdmissionAccounts
            | Self::AdmissionHolderAccounts
            | Self::AdmissionSources
            | Self::AdmissionProjects
            | Self::AdmissionSettingsProposals
            | Self::AdmissionSignIns
            | Self::AdmissionService
            | Self::Connectors
            | Self::ConnectorsCut
            | Self::ConnectorsEmpty => None,
            Self::TaskProposer
            | Self::PartyProposer
            | Self::Host
            | Self::Account
            | Self::Attempt
            | Self::Task
            | Self::Channel
            | Self::Slots
            | Self::Used
            | Self::Free
            | Self::Limit
            | Self::Kept
            | Self::Generation
            | Self::WaitingAttempts
            | Self::Proposals
            | Self::Project
            | Self::HostsForgotten
            | Self::HelloForgotten
            | Self::PersonForgotten
            | Self::TaskForgotten
            | Self::Person
            | Self::Connector
            | Self::Name
            | Self::Number
            | Self::HelloRefusalPosition
            | Self::Question
            | Self::Members
            | Self::First
            | Self::Message
            | Self::Left
            | Self::Right
            | Self::Entry
            | Self::Period
            | Self::Through
            | Self::Chat
            | Self::Messages
            | Self::Bytes
            | Self::StoreBytes
            | Self::StoreMark => Some(HoleKind::Number),
            Self::GraceRemaining | Self::GraceBound | Self::StopBound | Self::AllowedGrace | Self::Retention => {
                Some(HoleKind::Span)
            }
        }
    }
    #[expect(clippy::too_many_lines, reason = "the closed deployment vocabulary has one exhaustive mapping table")]
    fn label(self) -> &'static [u8] {
        match self {
            Self::Deployment => b"Deployment owner facts",
            Self::Hosts => b"Hosts",
            Self::Attempts => b"Tracked attempts",
            Self::Accounts => b"LLM accounts",
            Self::Capacity => b"Resource capacities",
            Self::Projects => b"Project capacities",
            Self::HelloRefusals => b"Refused hellos",
            Self::PersonRefusals => b"Person room refusals",
            Self::TaskRefusals => b"Task room refusals",
            Self::Runs => b"Hosted runs",
            Self::WaitingRuns => b"Tasks waiting for account",
            Self::PersonRefusal => b"Person request refusal",
            Self::TaskRefusal => b"Task request refusal",
            Self::UnknownClock => b"Clock not supplied",
            Self::UnknownDuration => b"Duration not supplied",
            Self::UnknownChannel => b"Lost host identity not supplied",
            Self::UnknownProject => b"Project not supplied",
            Self::UnknownFirst => b"First task not supplied",
            Self::RequestKey => b"Opaque request key",
            Self::Cut => b"Some rows were omitted",
            Self::Listed => b"Listed by host",
            Self::NotListed => b"Not listed by host",
            Self::Connected => b"Connected",
            Self::Disconnected => b"Disconnected",
            Self::Draining => b"Draining",
            Self::NotDraining => b"Not draining",
            Self::Usable => b"Usable",
            Self::Unusable => b"Unusable",
            Self::Expiring => b"Expiring",
            Self::NotExpiring => b"Not expiring",
            Self::Attention => b"Needs attention",
            Self::NoAttention => b"No attention flag",
            Self::TaskProposer => b"Task maker: ",
            Self::PartyProposer => b"Party maker: ",
            Self::Host => b"Host channel: ",
            Self::Account => b"Account: ",
            Self::Attempt => b"Attempt: ",
            Self::Task => b"Task: ",
            Self::Channel => b"Channel: ",
            Self::Slots => b"Slots: ",
            Self::Used => b"Used: ",
            Self::Free => b"Free: ",
            Self::Limit => b"Limit: ",
            Self::Kept => b"Kept turns: ",
            Self::Generation => b"Account generation: ",
            Self::WaitingAttempts => b"Waiting attempts: ",
            Self::Proposals => b"Pending proposals: ",
            Self::Project => b"Project: ",
            Self::HostsForgotten => b"Hosts forgotten: ",
            Self::HelloForgotten => b"Hellos forgotten: ",
            Self::PersonForgotten => b"Person refusals forgotten: ",
            Self::TaskForgotten => b"Task refusals forgotten: ",
            Self::Person => b"Person: ",
            Self::Connector => b"Connector: ",
            Self::Name => b"Note: ",
            Self::Number => b"Number: ",
            Self::Question => b"Question: ",
            Self::Members => b"Batch members: ",
            Self::First => b"First task: ",
            Self::Message => b"Message: ",
            Self::Left => b"Left task: ",
            Self::Right => b"Right task: ",
            Self::Entry => b"Effect entry: ",
            Self::Period => b"Period: ",
            Self::Through => b"Read through: ",
            Self::Chat => b"Chat: ",
            Self::Messages => b"Inbox messages: ",
            Self::Bytes => b"Inbox bytes: ",
            Self::At => b"At: ",
            Self::GraceUntil => b"Grace until: ",
            Self::SpentUntil => b"Spent until: ",
            Self::GraceRemaining => b"Grace remaining: ",
            Self::GraceBound => b"Deployment grace bound: ",
            Self::StopBound => b"Hello stop bound: ",
            Self::AllowedGrace => b"Allowed grace: ",
            Self::Retention => b"Person answer retention: ",
            Self::TasksCapacity => b"Tasks Capacity",
            Self::WorkersCapacity => b"Workers Capacity",
            Self::AttemptsCapacity => b"Attempts Capacity",
            Self::ProposalsCapacity => b"Proposals Capacity",
            Self::ProjectTasksCapacity => b"Project Tasks Capacity",
            Self::HostKindWorker => b"Host Kind Worker",
            Self::HostKindEngine => b"Host Kind Engine",
            Self::AttemptStateWaiting => b"Attempt State Waiting",
            Self::AttemptStateAdopted => b"Attempt State Adopted",
            Self::AttemptStateClaimed => b"Attempt State Claimed",
            Self::AttemptStateCancelled => b"Attempt State Cancelled",
            Self::AttemptStateHanded => b"Attempt State Handed",
            Self::AttemptStateAcknowledged => b"Attempt State Acknowledged",
            Self::AttemptStateStray => b"Attempt State Stray",
            Self::AttemptStateKept => b"Attempt State Kept",
            Self::AttemptStateFenced => b"Attempt State Fenced",
            Self::AttemptStateClosed => b"Attempt State Closed",
            Self::RunLocationNone => b"Run Location None",
            Self::RunLocationConnected => b"Run Location Connected",
            Self::RunLocationAdrift => b"Run Location Adrift",
            Self::AccountStateGranted => b"Account State Granted",
            Self::AccountStateAsking => b"Account State Asking",
            Self::AccountStateSignInAgain => b"Account State Sign in again",
            Self::AccountStateMisconfigured => b"Account State Misconfigured",
            Self::AccountDetail(phrase) => phrase.label(),
            Self::Agent(phrase) => phrase.label(),
            Self::ProgramDetail(phrase) => phrase.label(),
            Self::AccountStateClosing => b"Account State Closing",
            Self::AccountStateMoving => b"moving",
            Self::HelloRefusalReservedChannel => b"Channel zero belongs to the engine's own host",
            Self::HelloRefusalWorkers => b"No room for another worker",
            Self::HelloRefusalPosition => b"Unconfigured or occupied worker position ",
            Self::HelloRefusalStopBound => b"It would not stop before the engine's grace ends",
            Self::PersonCapacityActionCreate => b"Person Capacity Action Create",
            Self::PersonCapacityActionCompound => b"Person Capacity Action Compound",
            Self::PersonCapacityActionApplication => b"Person Capacity Action Application",
            Self::PersonCapacityActionNote => b"Person Capacity Action Note",
            Self::PersonCapacityActionService => b"Person Capacity Action Service",
            Self::PersonCapacityActionAdoption => b"Person Capacity Action Adoption",
            Self::PersonCapacityActionRoles => b"Person Capacity Action Roles",
            Self::PersonCapacityActionPolicy => b"Person Capacity Action Policy",
            Self::PersonCapacityActionPool => b"Person Capacity Action Pool",
            Self::PersonCapacityActionEscalation => b"Person Capacity Action Escalation",
            Self::PersonCapacityActionProposal => b"Person Capacity Action Proposal",
            Self::PersonCapacityActionWords => b"Person Capacity Action Words",
            Self::PersonCapacityActionQuestion => b"Person Capacity Action Question",
            Self::PersonCapacityActionAmend => b"Person Capacity Action Amend",
            Self::PersonCapacityActionMove => b"Person Capacity Action Move",
            Self::PersonCapacityActionTakePerson => b"Person Capacity Action Take Person",
            Self::PersonCapacityActionHandBackPerson => b"Person Capacity Action Hand Back Person",
            Self::PersonCapacityActionAnswerPerson => b"Person Capacity Action Answer Person",
            Self::PersonCapacityActionStop => b"Person Capacity Action Stop",
            Self::PersonCapacityActionCancel => b"Person Capacity Action Cancel",
            Self::PersonCapacityActionRelease => b"Person Capacity Action Release",
            Self::PersonCapacityActionMarkSeen => b"Person Capacity Action Mark SeenThrough",
            Self::PersonCapacityActionAccountSignIn => b"Sign in to an account",
            Self::PersonCapacityActionAccountSignOut => b"Sign out of an account",
            Self::PersonCapacityActionAccountChange => b"Change an account",
            Self::PersonCapacityCauseRequests => b"Person Capacity Cause Requests",
            Self::PersonCapacityCausePending => b"Person Capacity Cause Pending",
            Self::PersonCapacityCauseWaiters => b"Person Capacity Cause Waiters",
            Self::PersonCapacityCauseLimit => b"Person Capacity Cause Limit",
            Self::TaskCapacityActionBatch => b"Task Capacity Action Batch",
            Self::TaskCapacityActionProposal => b"Task Capacity Action Proposal",
            Self::TaskCapacityActionPersonProposal => b"Task Capacity Action Person Proposal",
            Self::TaskCapacityActionMessage => b"Task Capacity Action Message",
            Self::TaskCapacityActionSource => b"Task Capacity Action Source",
            Self::TaskCapacityActionIntroduce => b"Task Capacity Action Introduce",
            Self::TaskCapacityActionSubscription => b"Task Capacity Action Subscription",
            Self::TaskCapacityActionEffect => b"Task Capacity Action Effect",
            Self::TaskCapacityActionClaim => b"Task Capacity Action Claim",
            Self::TaskCapacityActionRelease => b"Task Capacity Action Release",
            Self::TaskCapacityActionMove => b"Task Capacity Action Move",
            Self::TaskCapacityCauseTasks => b"Task Capacity Cause Tasks",
            Self::TaskCapacityCauseCreationFlights => b"Task Capacity Cause Creation Flights",
            Self::TaskCapacityCauseResourceReports => b"Task Capacity Cause Resource Reports",
            Self::TaskCapacityCauseProjectTasks => b"Task Capacity Cause Project Tasks",
            Self::TaskCapacityCauseTree => b"Task Capacity Cause Tree",
            Self::TaskCapacityCauseDelegates => b"Task Capacity Cause Delegates",
            Self::TaskCapacityCauseInbox => b"Task Capacity Cause Inbox",
            Self::TaskCapacityCauseHoldings => b"Task Capacity Cause Holdings",
            Self::TaskCapacityCauseHoldWaiters => b"Task Capacity Cause Hold Waiters",
            Self::TaskCapacityCauseProposals => b"Task Capacity Cause Proposals",
            Self::TaskCapacityCauseReferences => b"Task Capacity Cause References",
            Self::TaskCapacityCauseSubscriptions => b"Task Capacity Cause Subscriptions",
            Self::TaskCapacityCauseFunders => b"Task Capacity Cause Funders",
            Self::TaskCapacityCauseWriters => b"Task Capacity Cause Writers",
            Self::FunderPool => b"Funding from pool",
            Self::FunderRecurring => b"Funding from recurring",
            Self::FunderPeriod => b"Funding from period",
            Self::FunderTask => b"Funding from task",
            Self::AdmissionGoals => b"Structural limit: goals",
            Self::AdmissionOperations => b"Structural limit: operations",
            Self::AdmissionGrace => b"Structural limit: grace",
            Self::AdmissionWords => b"Structural limit: words",
            Self::AdmissionIdentity => b"Structural limit: identity",
            Self::AdmissionRoles => b"Structural limit: roles",
            Self::AdmissionArithmetic => b"Structural limit: arithmetic",
            Self::AdmissionDuplicate => b"Structural limit: duplicate",
            Self::AdmissionEmpty => b"Structural limit: empty",
            Self::AdmissionBatch => b"Structural limit: batch",
            Self::AdmissionProjectLiveTasks => b"Structural limit: project live tasks",
            Self::AdmissionLiveTasks => b"Structural limit: live tasks",
            Self::AdmissionTree => b"Structural limit: tree",
            Self::AdmissionDepth => b"Structural limit: depth",
            Self::AdmissionDelegates => b"Structural limit: delegates",
            Self::AdmissionSubscription => b"Structural limit: subscription",
            Self::AdmissionDependencies => b"Structural limit: dependencies",
            Self::AdmissionCycle => b"Structural limit: cycle",
            Self::AdmissionSpec => b"Structural limit: spec",
            Self::AdmissionContract => b"Structural limit: contract",
            Self::AdmissionAuthority => b"Structural limit: authority",
            Self::AdmissionInputs => b"Structural limit: inputs",
            Self::AdmissionAttempts => b"Structural limit: attempts",
            Self::AdmissionLiveDelegates => b"Structural limit: live delegates",
            Self::AdmissionRestore => b"Structural limit: restore",
            Self::AdmissionRead => b"Structural limit: read",
            Self::AdmissionTurn => b"Structural limit: turn",
            Self::AdmissionHoldKind => b"Structural limit: hold kind",
            Self::AdmissionHoldTaken => b"Structural limit: hold taken",
            Self::AdmissionHolds => b"Structural limit: holds",
            Self::AdmissionInboxFull => b"Structural limit: inbox full",
            Self::AdmissionNotes => b"Structural limit: notes",
            Self::AdmissionRevision => b"Structural limit: revision",
            Self::AdmissionQuestion => b"Structural limit: question",
            Self::AdmissionPriority => b"Structural limit: priority",
            Self::AdmissionPolicy => b"Structural limit: policy",
            Self::AdmissionResource => b"Structural limit: resource",
            Self::AdmissionService => b"Structural limit: service",
            Self::AdmissionAccounts => b"Structural limit: accounts",
            Self::AdmissionHolderAccounts => b"Structural limit: holder accounts",
            Self::AdmissionSources => b"Structural limit: sources",
            Self::AdmissionSignIns => b"Structural limit: account sign-ins",
            Self::AdmissionSettingsProposals => b"Structural limit: pending settings proposals",
            Self::AdmissionProjects => b"The deployment has no room for another project",

            Self::Connectors => b"Configured connectors",
            Self::ConnectorsCut => b"Some configured connectors omitted",
            Self::ConnectorsEmpty => b"No configured connectors",
            Self::StoreBytes => b"Store map bytes: ",
            Self::StoreMark => b"Store high-water mark in bytes: ",
        }
    }
}
#[must_use]
pub fn shapes() -> Box<[PhraseShape]> {
    let mut result = List::with_capacity(u32::try_from(ALL.len()).expect("deployment phrases fit"));
    for phrase in ALL {
        let holes = match phrase.hole() {
            Some(kind) => Box::from([HoleShape { kind, bytes: 0 }]),
            None => Box::from([]),
        };
        result.push(PhraseShape { id: phrase.id(), holes, plural: None }).expect("deployment shapes fit");
    }
    result.into_boxed()
}
#[must_use]
pub fn language() -> Language {
    let mut result = List::with_capacity(u32::try_from(ALL.len()).expect("deployment phrases fit"));
    for phrase in ALL {
        let mut segments = List::with_capacity(2);
        segments.push(Segment::Text(copy_of(phrase.label()))).expect("deployment label fits");
        if let Some(kind) = phrase.hole() {
            segments.push(Segment::Hole { index: 0, kind }).expect("deployment hole fits");
        }
        result
            .push(Phrase {
                id: phrase.id(),
                forms: Box::new([Form { class: PluralClass::Other, segments: segments.into_boxed() }]),
            })
            .expect("deployment phrases fit");
    }
    Language {
        plural: PluralRule::English,
        span_unit: SpanUnit::Nanos,
        wall: WallFormat::Nanos,
        phrases: result.into_boxed(),
    }
}
