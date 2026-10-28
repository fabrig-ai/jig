//! Closed source activity/message sayings (client/domain/view.md, 7).
//! Allocated phrase numbers 7200 onward are independent of conversation rendering.
use crate::Owner;
use alloc::boxed::Box;
use skein_lib::{List, bytes::copy_of};
use skein_ui::phrases::{
    Form, HoleKind, HoleShape, Language, Phrase, PhraseId, PhraseShape, PluralClass, PluralRule, Segment, SpanUnit,
    WallFormat,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivityPhrase {
    Activity,
    Task,
    Position,
    At,
    Word,
    Message,
    From,
    Recipient,
    TaskIdentity,
    PersonIdentity,
    DeploymentIdentity,
    Hits,
    Eligible,
    Ineligible,
    WholeWords,
    Words,
    Heard,
    Notified,
    RunBegan,
    Attempt,
    Resumed,
    NotResumed,
    Made,
    Ready,
    MessageCause,
    Released,
    Retry,
    Narrowed,
    Changed,
    TransientTries,
    PermanentTries,
    RunTries,
    AgentTries,
    LostTries,
    InvalidTries,
    RunEnded,
    Spent,
    Parked,
    Finished,
    CancelledEnd,
    FailedEnd,
    Refused,
    Transient,
    Permanent,
    RunFailure,
    AgentFailure,
    Lost,
    Invalid,
    Turn,
    Reply,
    TurnNumber,
    Read,
    ReadAbsent,
    Decided,
    Revision,
    Milestone,
    Escalation,
    Proposal,
    Proposer,
    ProposalNumber,
    Batch,
    Effect,
    Amend,
    Widen,
    Release,
    PersonStalled,
    ProposalDecision,
    SettingsDecision,
    SettingsStale,
    Newest,
    Accepted,
    Rejected,
    MessageWords,
    EffectNotMade,
    Entry,
    Amendment,
    Question,
    Answer,
    QuestionNumber,
    Notice,
    Subscription,
    Target,
    Held,
    Done,
    NoticeFailed,
    NoticeCancelled,
    Timer,
    News,
    Wakes,
    Kept,
    Dropped,
    Result,
    Approved,
    State,
    WordsPresent,
    WordsAbsent,
    Report,
    Verdict,
    Code,
    FieldName,
    FieldValue,
    Change,
    Connector,
    ChangeKind,
    Resource,
    ResultFailed,
    ResultCancelled,
    QuestionAnswered,
    QuestionEnded,
    KeptQuestion,
}
// The original 107 activity phrases plus three question-rail captions.
pub const ALL: [ActivityPhrase; 110] = [
    ActivityPhrase::Activity,
    ActivityPhrase::Task,
    ActivityPhrase::Position,
    ActivityPhrase::At,
    ActivityPhrase::Word,
    ActivityPhrase::Message,
    ActivityPhrase::From,
    ActivityPhrase::Recipient,
    ActivityPhrase::TaskIdentity,
    ActivityPhrase::PersonIdentity,
    ActivityPhrase::DeploymentIdentity,
    ActivityPhrase::Hits,
    ActivityPhrase::Eligible,
    ActivityPhrase::Ineligible,
    ActivityPhrase::WholeWords,
    ActivityPhrase::Words,
    ActivityPhrase::Heard,
    ActivityPhrase::Notified,
    ActivityPhrase::RunBegan,
    ActivityPhrase::Attempt,
    ActivityPhrase::Resumed,
    ActivityPhrase::NotResumed,
    ActivityPhrase::Made,
    ActivityPhrase::Ready,
    ActivityPhrase::MessageCause,
    ActivityPhrase::Released,
    ActivityPhrase::Retry,
    ActivityPhrase::Narrowed,
    ActivityPhrase::Changed,
    ActivityPhrase::TransientTries,
    ActivityPhrase::PermanentTries,
    ActivityPhrase::RunTries,
    ActivityPhrase::AgentTries,
    ActivityPhrase::LostTries,
    ActivityPhrase::InvalidTries,
    ActivityPhrase::RunEnded,
    ActivityPhrase::Spent,
    ActivityPhrase::Parked,
    ActivityPhrase::Finished,
    ActivityPhrase::CancelledEnd,
    ActivityPhrase::FailedEnd,
    ActivityPhrase::Refused,
    ActivityPhrase::Transient,
    ActivityPhrase::Permanent,
    ActivityPhrase::RunFailure,
    ActivityPhrase::AgentFailure,
    ActivityPhrase::Lost,
    ActivityPhrase::Invalid,
    ActivityPhrase::Turn,
    ActivityPhrase::Reply,
    ActivityPhrase::TurnNumber,
    ActivityPhrase::Read,
    ActivityPhrase::ReadAbsent,
    ActivityPhrase::Decided,
    ActivityPhrase::Revision,
    ActivityPhrase::Milestone,
    ActivityPhrase::Escalation,
    ActivityPhrase::Proposal,
    ActivityPhrase::Proposer,
    ActivityPhrase::ProposalNumber,
    ActivityPhrase::Batch,
    ActivityPhrase::Effect,
    ActivityPhrase::Amend,
    ActivityPhrase::Widen,
    ActivityPhrase::Release,
    ActivityPhrase::PersonStalled,
    ActivityPhrase::ProposalDecision,
    ActivityPhrase::SettingsDecision,
    ActivityPhrase::SettingsStale,
    ActivityPhrase::Newest,
    ActivityPhrase::Accepted,
    ActivityPhrase::Rejected,
    ActivityPhrase::MessageWords,
    ActivityPhrase::EffectNotMade,
    ActivityPhrase::Entry,
    ActivityPhrase::Amendment,
    ActivityPhrase::Question,
    ActivityPhrase::Answer,
    ActivityPhrase::QuestionNumber,
    ActivityPhrase::Notice,
    ActivityPhrase::Subscription,
    ActivityPhrase::Target,
    ActivityPhrase::Held,
    ActivityPhrase::Done,
    ActivityPhrase::NoticeFailed,
    ActivityPhrase::NoticeCancelled,
    ActivityPhrase::Timer,
    ActivityPhrase::News,
    ActivityPhrase::Wakes,
    ActivityPhrase::Kept,
    ActivityPhrase::Dropped,
    ActivityPhrase::Result,
    ActivityPhrase::Approved,
    ActivityPhrase::State,
    ActivityPhrase::WordsPresent,
    ActivityPhrase::WordsAbsent,
    ActivityPhrase::Report,
    ActivityPhrase::Verdict,
    ActivityPhrase::Code,
    ActivityPhrase::FieldName,
    ActivityPhrase::FieldValue,
    ActivityPhrase::Change,
    ActivityPhrase::Connector,
    ActivityPhrase::ChangeKind,
    ActivityPhrase::Resource,
    ActivityPhrase::ResultFailed,
    ActivityPhrase::ResultCancelled,
    ActivityPhrase::QuestionAnswered,
    ActivityPhrase::QuestionEnded,
    ActivityPhrase::KeptQuestion,
];
impl ActivityPhrase {
    #[must_use]
    #[expect(
        clippy::too_many_lines,
        reason = "one total data table keeps all closed activity phrase identities together"
    )]
    pub fn id(self) -> PhraseId {
        let number = match self {
            Self::Activity => 7200,
            Self::Task => 7201,
            Self::Position => 7202,
            Self::At => 7203,
            Self::Word => 7204,
            Self::Message => 7205,
            Self::From => 7206,
            Self::Recipient => 7207,
            Self::TaskIdentity => 7208,
            Self::PersonIdentity => 7209,
            Self::DeploymentIdentity => 7210,
            Self::Hits => 7211,
            Self::Eligible => 7212,
            Self::Ineligible => 7213,
            Self::WholeWords => 7214,
            Self::Words => 7215,
            Self::Heard => 7216,
            Self::Notified => 7217,
            Self::RunBegan => 7218,
            Self::Attempt => 7219,
            Self::Resumed => 7220,
            Self::NotResumed => 7221,
            Self::Made => 7222,
            Self::Ready => 7223,
            Self::MessageCause => 7224,
            Self::Released => 7225,
            Self::Retry => 7226,
            Self::Narrowed => 7227,
            Self::Changed => 7228,
            Self::TransientTries => 7229,
            Self::PermanentTries => 7230,
            Self::RunTries => 7231,
            Self::AgentTries => 7232,
            Self::LostTries => 7233,
            Self::InvalidTries => 7234,
            Self::RunEnded => 7235,
            Self::Spent => 7236,
            Self::Parked => 7237,
            Self::Finished => 7238,
            Self::CancelledEnd => 7239,
            Self::FailedEnd => 7240,
            Self::Refused => 7241,
            Self::Transient => 7242,
            Self::Permanent => 7243,
            Self::RunFailure => 7244,
            Self::AgentFailure => 7245,
            Self::Lost => 7246,
            Self::Invalid => 7247,
            Self::Turn => 7248,
            Self::Reply => 7249,
            Self::TurnNumber => 7250,
            Self::Read => 7251,
            Self::ReadAbsent => 7252,
            Self::Decided => 7253,
            Self::Revision => 7254,
            Self::Milestone => 7255,
            Self::Escalation => 7256,
            Self::Proposal => 7257,
            Self::Proposer => 7258,
            Self::ProposalNumber => 7259,
            Self::Batch => 7260,
            Self::Effect => 7261,
            Self::Amend => 7262,
            Self::Widen => 7263,
            Self::Release => 7264,
            Self::PersonStalled => 7265,
            Self::ProposalDecision => 7266,
            Self::SettingsDecision => 7304,
            Self::SettingsStale => 7305,
            Self::Newest => 7306,
            Self::Accepted => 7267,
            Self::Rejected => 7268,
            Self::MessageWords => 7269,
            Self::EffectNotMade => 7270,
            Self::Entry => 7271,
            Self::Amendment => 7272,
            Self::Question => 7273,
            Self::Answer => 7274,
            Self::QuestionNumber => 7275,
            Self::Notice => 7276,
            Self::Subscription => 7277,
            Self::Target => 7278,
            Self::Held => 7279,
            Self::Done => 7280,
            Self::NoticeFailed => 7281,
            Self::NoticeCancelled => 7282,
            Self::Timer => 7283,
            Self::News => 7284,
            Self::Wakes => 7285,
            Self::Kept => 7286,
            Self::Dropped => 7287,
            Self::Result => 7288,
            Self::Approved => 7289,
            Self::State => 7290,
            Self::WordsPresent => 7291,
            Self::WordsAbsent => 7292,
            Self::Report => 7293,
            Self::Verdict => 7294,
            Self::Code => 7295,
            Self::FieldName => 7296,
            Self::FieldValue => 7297,
            Self::Change => 7298,
            Self::Connector => 7299,
            Self::ChangeKind => 7300,
            Self::Resource => 7301,
            Self::ResultFailed => 7302,
            Self::ResultCancelled => 7303,
            Self::QuestionAnswered => 7307,
            Self::QuestionEnded => 7308,
            Self::KeptQuestion => 7309,
        };
        PhraseId { owner: Owner::Jig.opaque(), number }
    }
    #[expect(clippy::too_many_lines, reason = "one total data table keeps all closed activity phrase shapes together")]
    fn hole(self) -> Option<HoleKind> {
        match self {
            Self::Activity
            | Self::Word
            | Self::From
            | Self::Recipient
            | Self::Eligible
            | Self::Ineligible
            | Self::WholeWords
            | Self::Words
            | Self::Heard
            | Self::Notified
            | Self::RunBegan
            | Self::Resumed
            | Self::NotResumed
            | Self::Made
            | Self::Ready
            | Self::MessageCause
            | Self::Released
            | Self::Retry
            | Self::Narrowed
            | Self::Changed
            | Self::RunEnded
            | Self::Parked
            | Self::Finished
            | Self::CancelledEnd
            | Self::FailedEnd
            | Self::Refused
            | Self::Transient
            | Self::Permanent
            | Self::RunFailure
            | Self::AgentFailure
            | Self::Lost
            | Self::Invalid
            | Self::Turn
            | Self::ReadAbsent
            | Self::Decided
            | Self::Milestone
            | Self::Escalation
            | Self::Proposal
            | Self::Batch
            | Self::Effect
            | Self::Amend
            | Self::Widen
            | Self::Release
            | Self::PersonStalled
            | Self::SettingsDecision
            | Self::SettingsStale
            | Self::ProposalDecision
            | Self::Accepted
            | Self::Rejected
            | Self::MessageWords
            | Self::EffectNotMade
            | Self::Amendment
            | Self::Question
            | Self::Answer
            | Self::Notice
            | Self::Held
            | Self::Done
            | Self::NoticeFailed
            | Self::NoticeCancelled
            | Self::Timer
            | Self::News
            | Self::Wakes
            | Self::Kept
            | Self::Dropped
            | Self::Result
            | Self::Approved
            | Self::State
            | Self::WordsPresent
            | Self::WordsAbsent
            | Self::Report
            | Self::Verdict
            | Self::FieldName
            | Self::FieldValue
            | Self::Change
            | Self::ResultFailed
            | Self::ResultCancelled => None,
            Self::Task
            | Self::Position
            | Self::Message
            | Self::TaskIdentity
            | Self::PersonIdentity
            | Self::DeploymentIdentity
            | Self::Hits
            | Self::Attempt
            | Self::TransientTries
            | Self::PermanentTries
            | Self::RunTries
            | Self::AgentTries
            | Self::LostTries
            | Self::InvalidTries
            | Self::Spent
            | Self::Reply
            | Self::TurnNumber
            | Self::Read
            | Self::Newest
            | Self::Revision
            | Self::Proposer
            | Self::ProposalNumber
            | Self::Entry
            | Self::QuestionNumber
            | Self::QuestionAnswered
            | Self::QuestionEnded
            | Self::KeptQuestion
            | Self::Subscription
            | Self::Target
            | Self::Code
            | Self::Connector
            | Self::ChangeKind
            | Self::Resource => Some(HoleKind::Number),
            Self::At => Some(HoleKind::Wall),
        }
    }
    #[expect(
        clippy::too_many_lines,
        reason = "one total data table keeps all closed activity phrase captions together"
    )]
    fn label(self) -> &'static [u8] {
        match self {
            Self::Activity => b"Task activity",
            Self::Task => b"Activity task: ",
            Self::Position => b"Commit position: ",
            Self::At => b"Recorded at: ",
            Self::Word => b"Whole message",
            Self::Message => b"Message: ",
            Self::From => b"From",
            Self::Recipient => b"Recipient",
            Self::TaskIdentity => b"Task: ",
            Self::PersonIdentity => b"Person: ",
            Self::DeploymentIdentity => b"Deployment project: ",
            Self::Hits => b"Merged hints: ",
            Self::Eligible => b"Eligible",
            Self::Ineligible => b"Not eligible",
            Self::WholeWords => b"Whole words",
            Self::Words => b"Words recorded",
            Self::Heard => b"Message heard",
            Self::Notified => b"Message notified",
            Self::RunBegan => b"Run began",
            Self::Attempt => b"Attempt: ",
            Self::Resumed => b"Resumed",
            Self::NotResumed => b"Not resumed",
            Self::Made => b"Cause: made",
            Self::Ready => b"Cause: ready",
            Self::MessageCause => b"Cause: message",
            Self::Released => b"Cause: released",
            Self::Retry => b"Cause: retry",
            Self::Narrowed => b"Cause: narrowed",
            Self::Changed => b"Cause: changed",
            Self::TransientTries => b"Transient failures: ",
            Self::PermanentTries => b"Permanent failures: ",
            Self::RunTries => b"Run failures: ",
            Self::AgentTries => b"Agent failures: ",
            Self::LostTries => b"Lost failures: ",
            Self::InvalidTries => b"Invalid failures: ",
            Self::RunEnded => b"Run ended",
            Self::Spent => b"Spent: ",
            Self::Parked => b"End: parked",
            Self::Finished => b"End: finished",
            Self::CancelledEnd => b"End: cancelled",
            Self::FailedEnd => b"End: failed",
            Self::Refused => b"End: refused",
            Self::Transient => b"Class: transient",
            Self::Permanent => b"Class: permanent",
            Self::RunFailure => b"Class: run",
            Self::AgentFailure => b"Class: agent",
            Self::Lost => b"Class: lost",
            Self::Invalid => b"Class: invalid",
            Self::Turn => b"Committed turn reference",
            Self::Reply => b"Archive reply: ",
            Self::TurnNumber => b"Turn: ",
            Self::Read => b"Last read message: ",
            Self::ReadAbsent => b"Last read message not supplied",
            Self::Decided => b"Decision recorded",
            Self::Revision => b"Revision: ",
            Self::Milestone => b"Milestone",
            Self::Escalation => b"Kind: escalation",
            Self::Proposal => b"Kind: proposal",
            Self::Proposer => b"Proposer number: ",
            Self::ProposalNumber => b"Proposal: ",
            Self::Batch => b"Proposal kind: batch",
            Self::Effect => b"Proposal kind: effect",
            Self::Amend => b"Proposal kind: amend",
            Self::Widen => b"Proposal kind: widen",
            Self::Release => b"Proposal kind: release",
            Self::PersonStalled => b"Kind: person stalled",
            Self::ProposalDecision => b"Kind: proposal decision",
            Self::SettingsDecision => b"Kind: settings decision",
            Self::SettingsStale => b"Withdrawn as stale",
            Self::Newest => b"Newest revision: ",
            Self::Accepted => b"Accepted",
            Self::Rejected => b"Rejected",
            Self::MessageWords => b"Kind: words",
            Self::EffectNotMade => b"Kind: effect not made",
            Self::Entry => b"Entry: ",
            Self::Amendment => b"Kind: amendment",
            Self::Question => b"Kind: question",
            Self::Answer => b"Kind: answer",
            Self::QuestionNumber => b"Question: ",
            Self::Notice => b"Kind: notice",
            Self::Subscription => b"Subscription: ",
            Self::Target => b"Target task: ",
            Self::Held => b"Notice: held",
            Self::Done => b"Notice: done",
            Self::NoticeFailed => b"Notice: failed",
            Self::NoticeCancelled => b"Notice: cancelled",
            Self::Timer => b"Kind: timer",
            Self::News => b"Kind: news",
            Self::Wakes => b"News: wakes",
            Self::Kept => b"News: kept",
            Self::Dropped => b"News: dropped",
            Self::Result => b"Kind: result",
            Self::Approved => b"Result: approved",
            Self::State => b"Approved state",
            Self::WordsPresent => b"Explanation words present",
            Self::WordsAbsent => b"Explanation words absent",
            Self::Report => b"Result: report",
            Self::Verdict => b"Result: verdict",
            Self::Code => b"Verdict code: ",
            Self::FieldName => b"Field name",
            Self::FieldValue => b"Field value",
            Self::Change => b"Result: change",
            Self::Connector => b"Connector: ",
            Self::ChangeKind => b"Change kind: ",
            Self::Resource => b"Resource: ",
            Self::ResultFailed => b"Result: failed",
            Self::ResultCancelled => b"Result: cancelled",
            Self::QuestionAnswered => b"Person answered question in inbox: ",
            Self::QuestionEnded => b"Question ended unanswered: ",
            Self::KeptQuestion => b"Kept by question: ",
        }
    }
}
#[must_use]
pub fn shapes() -> Box<[PhraseShape]> {
    let mut result = List::with_capacity(u32::try_from(ALL.len()).expect("activity phrase count fits"));
    for phrase in ALL {
        let holes: Box<[HoleShape]> = match phrase.hole() {
            Some(kind) => Box::new([HoleShape { kind, bytes: 0 }]),
            None => Box::new([]),
        };
        result.push(PhraseShape { id: phrase.id(), holes, plural: None }).expect("closed activity shapes fit");
    }
    result.into_boxed()
}
#[must_use]
pub fn language() -> Language {
    let mut result = List::with_capacity(u32::try_from(ALL.len()).expect("activity phrase count fits"));
    for phrase in ALL {
        let segments: Box<[Segment]> = match phrase.hole() {
            Some(kind) => Box::new([Segment::Text(copy_of(phrase.label())), Segment::Hole { index: 0, kind }]),
            None => Box::new([Segment::Text(copy_of(phrase.label()))]),
        };
        result
            .push(Phrase { id: phrase.id(), forms: Box::new([Form { class: PluralClass::Other, segments }]) })
            .expect("closed activity sayings fit");
    }
    Language {
        plural: PluralRule::English,
        span_unit: SpanUnit::Nanos,
        wall: WallFormat::Nanos,
        phrases: result.into_boxed(),
    }
}
