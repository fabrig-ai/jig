//! Whole owner wake facts and separate standing interests.
//! See domain/tasks.md, 8.3–8.4, and client/domain/contract.md, 5.1–5.3.
//! These values carry the owner's choices; they infer no charter defaults,
//! connector names or schedule. The decoding owner bounds list capacities.
use skein_lib::{Duration, List, Wall};

/// How one kind of message contributes to activation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WakeRule {
    Never,
    Immediate,
    Batch { count: u32, age: Duration },
}

/// Which delegate results contribute to activation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ResultsWake {
    Never,
    Each,
    LastOrFailure,
}

/// Exact policy supplied by the owner, independent of subscriptions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OwnerWakePolicy {
    pub markers: u32,
    pub words: WakeRule,
    pub notices: WakeRule,
    pub news: WakeRule,
    pub results: ResultsWake,
    pub questions: bool,
    pub answers: bool,
    pub timers: bool,
}

/// One standing interest; connector topics are numeric owner identities.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SubscriptionKind {
    Task {
        target: u64,
        held: bool,
        result: bool,
    },
    /// `None` is a single firing, distinct from any supplied period.
    Timer {
        at: Wall,
        period: Option<Duration>,
    },
    Topic {
        connector: u16,
        topic: u64,
    },
}

/// The owner's number identifies this subscription, independently of its kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Subscription {
    pub number: u64,
    pub kind: SubscriptionKind,
}

/// A task's bounded whole subscription list, separate from its wake policy.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Subscriptions {
    pub entries: List<Subscription>,
}
