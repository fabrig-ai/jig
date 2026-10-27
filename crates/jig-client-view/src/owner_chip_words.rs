//! Checked chip metadata sayings (client/domain/view.md, 7).
//! IDs 7000..7098 are coordinated append-only owner sayings.
use crate::Owner;
use crate::words::Words;
use alloc::boxed::Box;
use skein_lib::{List, bytes::copy_of};
use skein_ui::phrases::{
    Form, HoleKind, HoleShape, Invalid, Language, Phrase, PhraseId, PhraseLimits, PhraseShape, Phrasebook, PluralClass,
    PluralRule, Segment, SpanUnit, WallFormat,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChipPhrase {
    Project,
    PhaseSince,
    Placement,
    Attempt,
    Activity,
    LiveDelegates,
    ConnectorsUnknown,
    ConnectorsEmpty,
    Connector,
    EndedAt,
    ResourcesObserved,
    ResourcesUnobserved,
    ResourcesCut,
    Procedure,
    Kind,
    Taken,
    NotTaken,
    Holder,
    WriterTask,
    WriterAttempt,
    WriterEffect,
    WriterLost,
    WriterKept,
    WaitSince,
    Stall,
    Deadline,
    NoticeMessage,
    NoticeAt,
    NoticeActivity,
    Revision,
    HoldWaitSince,
    PriorWaiting,
    PriorIdle,
    PriorDue,
    PriorPreparing,
    PriorClaimed,
    PriorRunning,
    PriorBackingOff,
    PriorClosingRun,
    PriorDelegates,
    PriorEffects,
    PriorReleases,
    PriorSettled,
    Routing,
    EscalationTask,
    EscalationPerson,
    EscalationProject,
    EscalationRole,
    Entry,
    EscalationSince,
    RejectedBy,
    TransientTries,
    PermanentTries,
    RunTries,
    AgentTries,
    LostTries,
    InvalidTries,
    Done,
    Failed,
    Cancelled,
    RequesterTask,
    RequesterPerson,
    RequesterDeployment,
    CancelledAt,
    Unobserved,
    Preparing,
    Starting,
    Active,
    Writing,
    Tool,
    Waiting,
    Ending,
    ObservedSince,
    ReportedAt,
    Started,
    StartUnknown,
    Cut,
    Failures,
    Transient,
    Permanent,
    RunFailure,
    AgentFailure,
    Lost,
    InvalidFailure,
    Stalled,
    Dependency,
    Stopped,
    Placed,
    Drift,
    Effects,
    EffectFailed,
    Uncertain,
    Budget,
    HoldDeadline,
    HoldsWaited,
    ResourceUnavailable,
    ResourceAccess,
    PoolLost,
    HoldProcedure,
}
pub const ALL: [ChipPhrase; 99] = [
    ChipPhrase::Project,
    ChipPhrase::PhaseSince,
    ChipPhrase::Placement,
    ChipPhrase::Attempt,
    ChipPhrase::Activity,
    ChipPhrase::LiveDelegates,
    ChipPhrase::ConnectorsUnknown,
    ChipPhrase::ConnectorsEmpty,
    ChipPhrase::Connector,
    ChipPhrase::EndedAt,
    ChipPhrase::ResourcesObserved,
    ChipPhrase::ResourcesUnobserved,
    ChipPhrase::ResourcesCut,
    ChipPhrase::Procedure,
    ChipPhrase::Kind,
    ChipPhrase::Taken,
    ChipPhrase::NotTaken,
    ChipPhrase::Holder,
    ChipPhrase::WriterTask,
    ChipPhrase::WriterAttempt,
    ChipPhrase::WriterEffect,
    ChipPhrase::WriterLost,
    ChipPhrase::WriterKept,
    ChipPhrase::WaitSince,
    ChipPhrase::Stall,
    ChipPhrase::Deadline,
    ChipPhrase::NoticeMessage,
    ChipPhrase::NoticeAt,
    ChipPhrase::NoticeActivity,
    ChipPhrase::Revision,
    ChipPhrase::HoldWaitSince,
    ChipPhrase::PriorWaiting,
    ChipPhrase::PriorIdle,
    ChipPhrase::PriorDue,
    ChipPhrase::PriorPreparing,
    ChipPhrase::PriorClaimed,
    ChipPhrase::PriorRunning,
    ChipPhrase::PriorBackingOff,
    ChipPhrase::PriorClosingRun,
    ChipPhrase::PriorDelegates,
    ChipPhrase::PriorEffects,
    ChipPhrase::PriorReleases,
    ChipPhrase::PriorSettled,
    ChipPhrase::Routing,
    ChipPhrase::EscalationTask,
    ChipPhrase::EscalationPerson,
    ChipPhrase::EscalationProject,
    ChipPhrase::EscalationRole,
    ChipPhrase::Entry,
    ChipPhrase::EscalationSince,
    ChipPhrase::RejectedBy,
    ChipPhrase::TransientTries,
    ChipPhrase::PermanentTries,
    ChipPhrase::RunTries,
    ChipPhrase::AgentTries,
    ChipPhrase::LostTries,
    ChipPhrase::InvalidTries,
    ChipPhrase::Done,
    ChipPhrase::Failed,
    ChipPhrase::Cancelled,
    ChipPhrase::RequesterTask,
    ChipPhrase::RequesterPerson,
    ChipPhrase::RequesterDeployment,
    ChipPhrase::CancelledAt,
    ChipPhrase::Unobserved,
    ChipPhrase::Preparing,
    ChipPhrase::Starting,
    ChipPhrase::Active,
    ChipPhrase::Writing,
    ChipPhrase::Tool,
    ChipPhrase::Waiting,
    ChipPhrase::Ending,
    ChipPhrase::ObservedSince,
    ChipPhrase::ReportedAt,
    ChipPhrase::Started,
    ChipPhrase::StartUnknown,
    ChipPhrase::Cut,
    ChipPhrase::Failures,
    ChipPhrase::Transient,
    ChipPhrase::Permanent,
    ChipPhrase::RunFailure,
    ChipPhrase::AgentFailure,
    ChipPhrase::Lost,
    ChipPhrase::InvalidFailure,
    ChipPhrase::Stalled,
    ChipPhrase::Dependency,
    ChipPhrase::Stopped,
    ChipPhrase::Placed,
    ChipPhrase::Drift,
    ChipPhrase::Effects,
    ChipPhrase::EffectFailed,
    ChipPhrase::Uncertain,
    ChipPhrase::Budget,
    ChipPhrase::HoldDeadline,
    ChipPhrase::HoldsWaited,
    ChipPhrase::ResourceUnavailable,
    ChipPhrase::ResourceAccess,
    ChipPhrase::PoolLost,
    ChipPhrase::HoldProcedure,
];
impl ChipPhrase {
    #[must_use]
    #[expect(clippy::too_many_lines, reason = "one total table keeps the closed phrase identities together")]
    pub fn id(self) -> PhraseId {
        let number = match self {
            Self::Project => 7000,
            Self::PhaseSince => 7001,
            Self::Placement => 7002,
            Self::Attempt => 7003,
            Self::Activity => 7004,
            Self::LiveDelegates => 7005,
            Self::ConnectorsUnknown => 7006,
            Self::ConnectorsEmpty => 7007,
            Self::Connector => 7008,
            Self::EndedAt => 7009,
            Self::ResourcesObserved => 7010,
            Self::ResourcesUnobserved => 7011,
            Self::ResourcesCut => 7012,
            Self::Procedure => 7013,
            Self::Kind => 7014,
            Self::Taken => 7015,
            Self::NotTaken => 7016,
            Self::Holder => 7017,
            Self::WriterTask => 7018,
            Self::WriterAttempt => 7019,
            Self::WriterEffect => 7020,
            Self::WriterLost => 7021,
            Self::WriterKept => 7022,
            Self::WaitSince => 7023,
            Self::Stall => 7024,
            Self::Deadline => 7025,
            Self::NoticeMessage => 7026,
            Self::NoticeAt => 7027,
            Self::NoticeActivity => 7028,
            Self::Revision => 7029,
            Self::HoldWaitSince => 7030,
            Self::PriorWaiting => 7031,
            Self::PriorIdle => 7032,
            Self::PriorDue => 7033,
            Self::PriorPreparing => 7034,
            Self::PriorClaimed => 7035,
            Self::PriorRunning => 7036,
            Self::PriorBackingOff => 7037,
            Self::PriorClosingRun => 7038,
            Self::PriorDelegates => 7039,
            Self::PriorEffects => 7040,
            Self::PriorReleases => 7041,
            Self::PriorSettled => 7042,
            Self::Routing => 7043,
            Self::EscalationTask => 7044,
            Self::EscalationPerson => 7045,
            Self::EscalationProject => 7046,
            Self::EscalationRole => 7047,
            Self::Entry => 7048,
            Self::EscalationSince => 7049,
            Self::RejectedBy => 7050,
            Self::TransientTries => 7051,
            Self::PermanentTries => 7052,
            Self::RunTries => 7053,
            Self::AgentTries => 7054,
            Self::LostTries => 7055,
            Self::InvalidTries => 7056,
            Self::Done => 7057,
            Self::Failed => 7058,
            Self::Cancelled => 7059,
            Self::RequesterTask => 7060,
            Self::RequesterPerson => 7061,
            Self::RequesterDeployment => 7062,
            Self::CancelledAt => 7063,
            Self::Unobserved => 7064,
            Self::Preparing => 7065,
            Self::Starting => 7066,
            Self::Active => 7067,
            Self::Writing => 7068,
            Self::Tool => 7069,
            Self::Waiting => 7070,
            Self::Ending => 7071,
            Self::ObservedSince => 7072,
            Self::ReportedAt => 7073,
            Self::Started => 7074,
            Self::StartUnknown => 7075,
            Self::Cut => 7076,
            Self::Failures => 7077,
            Self::Transient => 7078,
            Self::Permanent => 7079,
            Self::RunFailure => 7080,
            Self::AgentFailure => 7081,
            Self::Lost => 7082,
            Self::InvalidFailure => 7083,
            Self::Stalled => 7084,
            Self::Dependency => 7085,
            Self::Stopped => 7086,
            Self::Placed => 7087,
            Self::Drift => 7088,
            Self::Effects => 7089,
            Self::EffectFailed => 7090,
            Self::Uncertain => 7091,
            Self::Budget => 7092,
            Self::HoldDeadline => 7093,
            Self::HoldsWaited => 7094,
            Self::ResourceUnavailable => 7095,
            Self::ResourceAccess => 7096,
            Self::PoolLost => 7097,
            Self::HoldProcedure => 7098,
        };
        PhraseId { owner: Owner::Jig.opaque(), number }
    }
    #[expect(clippy::too_many_lines, reason = "one total table keeps the closed phrase shapes together")]
    fn hole(self) -> Option<HoleKind> {
        match self {
            Self::Project
            | Self::Attempt
            | Self::Activity
            | Self::LiveDelegates
            | Self::Connector
            | Self::Kind
            | Self::Holder
            | Self::WriterTask
            | Self::WriterAttempt
            | Self::WriterEffect
            | Self::NoticeMessage
            | Self::NoticeActivity
            | Self::Revision
            | Self::PriorClaimed
            | Self::PriorRunning
            | Self::PriorClosingRun
            | Self::EscalationTask
            | Self::EscalationPerson
            | Self::EscalationProject
            | Self::EscalationRole
            | Self::Entry
            | Self::RejectedBy
            | Self::TransientTries
            | Self::PermanentTries
            | Self::RunTries
            | Self::AgentTries
            | Self::LostTries
            | Self::InvalidTries
            | Self::RequesterTask
            | Self::RequesterPerson
            | Self::RequesterDeployment
            | Self::Dependency
            | Self::Uncertain
            | Self::PoolLost => Some(HoleKind::Number),
            Self::PhaseSince
            | Self::Placement
            | Self::EndedAt
            | Self::WaitSince
            | Self::Deadline
            | Self::NoticeAt
            | Self::HoldWaitSince
            | Self::PriorBackingOff
            | Self::EscalationSince
            | Self::CancelledAt
            | Self::ObservedSince
            | Self::ReportedAt
            | Self::Started => Some(HoleKind::Wall),
            Self::ConnectorsUnknown
            | Self::ConnectorsEmpty
            | Self::ResourcesObserved
            | Self::ResourcesUnobserved
            | Self::ResourcesCut
            | Self::Procedure
            | Self::Taken
            | Self::NotTaken
            | Self::WriterLost
            | Self::WriterKept
            | Self::PriorWaiting
            | Self::PriorIdle
            | Self::PriorDue
            | Self::PriorPreparing
            | Self::PriorDelegates
            | Self::PriorEffects
            | Self::PriorReleases
            | Self::PriorSettled
            | Self::Routing
            | Self::Done
            | Self::Failed
            | Self::Cancelled
            | Self::Unobserved
            | Self::Preparing
            | Self::Starting
            | Self::Active
            | Self::Writing
            | Self::Tool
            | Self::Waiting
            | Self::Ending
            | Self::StartUnknown
            | Self::Cut
            | Self::Failures
            | Self::Transient
            | Self::Permanent
            | Self::RunFailure
            | Self::AgentFailure
            | Self::Lost
            | Self::InvalidFailure
            | Self::Stalled
            | Self::Stopped
            | Self::Placed
            | Self::Drift
            | Self::Effects
            | Self::EffectFailed
            | Self::Budget
            | Self::HoldDeadline
            | Self::HoldsWaited
            | Self::ResourceUnavailable
            | Self::ResourceAccess
            | Self::HoldProcedure => None,
            Self::Stall => Some(HoleKind::Span),
        }
    }
    #[expect(clippy::too_many_lines, reason = "one total table keeps the closed owner labels together")]
    fn label(self) -> &'static [u8] {
        match self {
            Self::Project => b"Project: ",
            Self::PhaseSince => b"Phase since: ",
            Self::Placement => b"Placed or adopted: ",
            Self::Attempt => b"Attempt: ",
            Self::Activity => b"Activity through: ",
            Self::LiveDelegates => b"Live delegates: ",
            Self::ConnectorsUnknown => b"Closing connectors unavailable",
            Self::ConnectorsEmpty => b"No closing connectors",
            Self::Connector => b"Closing connector: ",
            Self::EndedAt => b"Ended at: ",
            Self::ResourcesObserved => b"Resource holder and writer facts observed",
            Self::ResourcesUnobserved => b"Resource holder and writer facts unavailable",
            Self::ResourcesCut => b"Resource list incomplete",
            Self::Procedure => b"Procedure reference",
            Self::Kind => b"Resource kind: ",
            Self::Taken => b"Resource taken",
            Self::NotTaken => b"Resource not taken",
            Self::Holder => b"Holder task: ",
            Self::WriterTask => b"Writer task: ",
            Self::WriterAttempt => b"Writer attempt: ",
            Self::WriterEffect => b"Writer effect: ",
            Self::WriterLost => b"Writer lost",
            Self::WriterKept => b"Writer not lost",
            Self::WaitSince => b"Person wait began: ",
            Self::Stall => b"Person stall span: ",
            Self::Deadline => b"Person stall deadline: ",
            Self::NoticeMessage => b"Stall notice message: ",
            Self::NoticeAt => b"Stall notice at: ",
            Self::NoticeActivity => b"Stall notice activity: ",
            Self::Revision => b"Hold revision: ",
            Self::HoldWaitSince => b"Hold wait began: ",
            Self::PriorWaiting => b"Prior stage waiting",
            Self::PriorIdle => b"Prior stage idle",
            Self::PriorDue => b"Prior stage due",
            Self::PriorPreparing => b"Prior stage preparing",
            Self::PriorClaimed => b"Prior stage claimed attempt: ",
            Self::PriorRunning => b"Prior stage running attempt: ",
            Self::PriorBackingOff => b"Prior retry at: ",
            Self::PriorClosingRun => b"Prior closing run attempt: ",
            Self::PriorDelegates => b"Prior closing delegates",
            Self::PriorEffects => b"Prior closing effects",
            Self::PriorReleases => b"Prior closing releases",
            Self::PriorSettled => b"Prior closing settled",
            Self::Routing => b"Escalation routing",
            Self::EscalationTask => b"Escalation holder task: ",
            Self::EscalationPerson => b"Escalation holder person: ",
            Self::EscalationProject => b"Escalation holder project: ",
            Self::EscalationRole => b"Escalation holder role: ",
            Self::Entry => b"Escalation entry: ",
            Self::EscalationSince => b"Escalation waiting since: ",
            Self::RejectedBy => b"Left held by: ",
            Self::TransientTries => b"Transient tries: ",
            Self::PermanentTries => b"Permanent tries: ",
            Self::RunTries => b"Run tries: ",
            Self::AgentTries => b"Agent tries: ",
            Self::LostTries => b"Lost tries: ",
            Self::InvalidTries => b"Invalid tries: ",
            Self::Done => b"Completed with result",
            Self::Failed => b"Failed with reason",
            Self::Cancelled => b"Cancelled with reason",
            Self::RequesterTask => b"Cancellation requester task: ",
            Self::RequesterPerson => b"Cancellation requester person: ",
            Self::RequesterDeployment => b"Cancellation requester deployment project: ",
            Self::CancelledAt => b"Cancelled at: ",
            Self::Unobserved => b"Run work unobserved",
            Self::Preparing => b"Reported work preparing",
            Self::Starting => b"Reported work starting",
            Self::Active => b"Reported work active",
            Self::Writing => b"Reported work writing",
            Self::Tool => b"Reported tool call",
            Self::Waiting => b"Reported work waiting",
            Self::Ending => b"Reported work ending",
            Self::ObservedSince => b"Current work since: ",
            Self::ReportedAt => b"Work reported at: ",
            Self::Started => b"Actual run start: ",
            Self::StartUnknown => b"Actual run start unavailable",
            Self::Cut => b"Supplied words incomplete",
            Self::Failures => b"Failure tries exhausted",
            Self::Transient => b"Transient failure",
            Self::Permanent => b"Permanent failure",
            Self::RunFailure => b"Run failure",
            Self::AgentFailure => b"Agent failure",
            Self::Lost => b"Lost failure",
            Self::InvalidFailure => b"Invalid failure",
            Self::Stalled => b"Held stalled",
            Self::Dependency => b"Held dependency task: ",
            Self::Stopped => b"Held stopped",
            Self::Placed => b"Held by a placed hold",
            Self::Drift => b"Held drift",
            Self::Effects => b"Held effects",
            Self::EffectFailed => b"Held effect failed",
            Self::Uncertain => b"Held uncertain entry: ",
            Self::Budget => b"Held budget",
            Self::HoldDeadline => b"Held deadline",
            Self::HoldsWaited => b"Held waited holds",
            Self::ResourceUnavailable => b"Held resource unavailable",
            Self::ResourceAccess => b"Held resource access",
            Self::PoolLost => b"Held lost pool: ",
            Self::HoldProcedure => b"Held procedure",
        }
    }
}
#[must_use]
pub fn shapes() -> Box<[PhraseShape]> {
    let mut values = List::with_capacity(99);
    for phrase in ALL {
        let holes: Box<[HoleShape]> = match phrase.hole() {
            Some(kind) => Box::new([HoleShape { kind, bytes: 0 }]),
            None => Box::new([]),
        };
        values.push(PhraseShape { id: phrase.id(), holes, plural: None }).expect("closed chip shapes fit");
    }
    values.into_boxed()
}
#[must_use]
pub fn language() -> Language {
    let mut values = List::with_capacity(99);
    for phrase in ALL {
        let label = phrase.label();
        let segments: Box<[Segment]> = match phrase.hole() {
            Some(kind) => Box::new([Segment::Text(copy_of(label)), Segment::Hole { index: 0, kind }]),
            None => Box::new([Segment::Text(copy_of(label))]),
        };
        values
            .push(Phrase { id: phrase.id(), forms: Box::new([Form { class: PluralClass::Other, segments }]) })
            .expect("closed chip sayings fit");
    }
    Language {
        plural: PluralRule::English,
        span_unit: SpanUnit::Nanos,
        wall: WallFormat::Nanos,
        phrases: values.into_boxed(),
    }
}
/// A standalone fragment shares the normal complete checked `Words` vocabulary.
#[derive(Debug)]
pub struct OwnerChipWords {
    words: Words,
}
impl OwnerChipWords {
    pub fn load(
        languages: Box<[Language]>,
        limits: &PhraseLimits,
        language: u16,
        offset_minutes: i32,
    ) -> Result<Self, Invalid> {
        if usize::from(language) >= languages.len() {
            return Err(Invalid::Language);
        }
        if u64::from(offset_minutes.unsigned_abs()).checked_mul(60_000_000_000).is_none() {
            return Err(Invalid::Overflow);
        }
        let book = Phrasebook::load(crate::phrases::jig::shapes(limits.hole_bytes), languages, limits)?;
        Ok(Self { words: Words::new(book, language, offset_minutes) })
    }
    #[must_use]
    pub fn words(&self) -> &Words {
        &self.words
    }
}
