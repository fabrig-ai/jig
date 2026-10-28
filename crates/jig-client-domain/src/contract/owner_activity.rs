//! Whole immutable activity and admitted words (domain/engine.md, 5.4 and 7.2;
//! client/domain/contract.md, 5 and 7). Commit position orders lines; wall time
//! is supplied metadata. Turns here are archive references, never decoded content.
use super::{HoldFailure, OwnerFailureTries, OwnerRequester, ResultField, Text};
use skein_lib::{List, Wall};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OwnerActivityLine {
    pub task: u64,
    pub position: u64,
    pub at: Wall,
    pub line: OwnerActivity,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum OwnerActivity {
    Words { message: u64, by: OwnerRequester, kind: OwnerMessageKind, words: Text },
    Heard(OwnerWord),
    Notified { recipient: OwnerRequester, word: OwnerWord },
    RunBegan { attempt: u64, why: OwnerRunWhy, resumed: bool },
    RunEnded { attempt: u64, end: OwnerRunEnd, spent: u64 },
    Turn { reply: u64, attempt: u64, turn: u32, read: Option<u64> },
    Decided { revision: u64 },
    Milestone { position: u64 },
    Kept { attempt: u64, pending: List<u64>, question: Option<u64> },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OwnerRunWhy {
    Made,
    Ready,
    Message { message: u64 },
    Released,
    Retry { tries: OwnerFailureTries },
    Narrowed,
    Changed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OwnerRunEnd {
    Parked,
    Finished,
    Cancelled,
    Failed { class: HoldFailure },
    Refused,
}
/// All bytes are whole admitted words. No text-loss marker exists in this source.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OwnerWord {
    pub number: u64,
    pub from: OwnerRequester,
    pub kind: OwnerMessageKind,
    pub words: Text,
    pub at: Wall,
    pub hits: u32,
    pub eligible: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum OwnerMessageKind {
    SettingsDecision {
        proposal: u64,
        outcome: SettingsOutcome,
    },
    Escalation {
        task: u64,
        revision: u64,
    },
    /// The supplied proposer number has no Task/Person category in this source.
    Proposal {
        proposer: u64,
        proposal: u64,
        kind: OwnerMessageProposalKind,
    },
    PersonStalled {
        task: u64,
    },
    ProposalDecision {
        proposal: u64,
        accepted: bool,
    },
    QuestionAnswered {
        question: u64,
    },
    QuestionEnded {
        question: u64,
    },
    Words,
    EffectNotMade {
        entry: u64,
    },
    Amendment {
        revision: u64,
    },
    Question,
    Answer {
        question: u64,
    },
    Notice {
        subscription: u64,
        target: u64,
        state: OwnerNoticeState,
    },
    Timer {
        subscription: u64,
    },
    News {
        subscription: u64,
        class: OwnerNewsClass,
    },
    Result(OwnerWordResult),
    Met {
        dependency: u64,
        removed: bool,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OwnerMessageProposalKind {
    Settings,
    Batch,
    Effect,
    Amend,
    Widen,
    Release,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OwnerNoticeState {
    Held,
    Done,
    Failed,
    Cancelled,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OwnerNewsClass {
    Wakes,
    Kept,
    Dropped,
}
/// Message metadata, separate from a result document or its connector's part.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum OwnerWordResult {
    /// Presence is independent of the associated whole words, including empty words.
    Approved {
        state: Text,
        words_present: bool,
    },
    Report,
    Verdict {
        code: u32,
        fields: List<ResultField>,
    },
    Change {
        connector: u16,
        kind: u16,
        resource: u64,
    },
    Failed,
    Cancelled,
}

/// The facts supplied by a settings proposal's durable inbox outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SettingsOutcome {
    Accepted { revision: u32 },
    Rejected,
    Stale { newest: u32 },
}
