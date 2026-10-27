//! Jig's enum phrases, overridable one by one (client/domain/client.md, 19).
mod decision;
mod facts;
pub use facts::decision;
mod english;
use crate::{
    contract::{Bound, HoldCategory, Phase, Refusal, WaitsOn},
    facts::ChatStatus,
};
use alloc::boxed::Box;
pub use decision::DecisionWord;
use skein_lib::List;
use skein_ui::phrases::{HoleKind, HoleShape, Language, PhraseId, PhraseShape};
/// Phrase count, for a caller composing a bounded phrasebook.
pub const PHRASES: u32 = 120;
/// Owner of jig's public enum vocabulary.
pub const OWNER: skein_ui::Owner = skein_ui::Owner { number: 65_537 };
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Word {
    Lifted,
    Phase(Phase),
    Hold(HoldWord),
    Refusal(RefusalWord),
    Waiting(WaitingWord),
    Status(StatusWord),
    Decision(DecisionWord),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoldWord {
    Exceeded(Bound),
    Circle,
    Failures,
    Stalled,
    Dependency,
    Stopped,
    Placed,
    Drift,
    Effects,
    EffectFailed,
    Uncertain,
    Budget,
    Deadline,
    HoldsWaited,
    ResourceUnavailable,
    ResourceAccess,
    PoolLost,
    Procedure,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefusalWord {
    HoldOwnerParty,
    HoldOwnerTask,
    SignInRedirectNotWaiting,
    SignInRedirectAddress,
    SignInRedirectState,
    SignInRedirectTooMany,
    Stale,
    Full,
    Circle,
    Dependency,
    Kept,
    NoEscalationHolder,
    Malformed,
    Role,
    Taken,
    NotWaiting,
    NoFurther,
    Authority,
    Limit,
    Moved,
    Ended,
    Unknown,
    Provider,
    NotOffered,
    NameTaken,
    KeyConflict,
    Llm,
    AccountUnknown,
    AccountProvider,
    AccountUses,
    AccountLimited,
    AccountMoveNotSignedIn,
    AccountMoveNotOurs,
    AccountMoveHeld,
    AccountMoveCancelled,
    AccountMoveMoving,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaitingWord {
    Dependencies,
    Party,
    Delegates,
    News,
    Nothing,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusWord {
    HostRefused,
    Starting,
    Writing,
    Tool,
    Working,
    WaitingForYou,
    WaitingFor,
    Parked,
    Retrying,
    Held,
    Closing,
    Ended,
}
impl Word {
    #[must_use]
    pub const fn id(self) -> PhraseId {
        PhraseId { owner: OWNER, number: self.data().0 }
    }
    #[must_use]
    pub fn hold(value: &HoldCategory) -> Self {
        Self::Hold(match value {
            HoldCategory::Exceeded(value) => HoldWord::Exceeded(value.bound),
            HoldCategory::Failures { .. } => HoldWord::Failures,
            HoldCategory::Stalled => HoldWord::Stalled,
            HoldCategory::Dependency { .. } => HoldWord::Dependency,
            HoldCategory::Stopped => HoldWord::Stopped,
            HoldCategory::Placed { .. } => HoldWord::Placed,
            HoldCategory::Drift => HoldWord::Drift,
            HoldCategory::Effects => HoldWord::Effects,
            HoldCategory::EffectFailed => HoldWord::EffectFailed,
            HoldCategory::Uncertain { .. } => HoldWord::Uncertain,
            HoldCategory::Budget => HoldWord::Budget,
            HoldCategory::Deadline => HoldWord::Deadline,
            HoldCategory::HoldsWaited => HoldWord::HoldsWaited,
            HoldCategory::ResourceUnavailable => HoldWord::ResourceUnavailable,
            HoldCategory::ResourceAccess => HoldWord::ResourceAccess,
            HoldCategory::PoolLost { .. } => HoldWord::PoolLost,
            HoldCategory::Procedure => HoldWord::Procedure,
            HoldCategory::Circle => HoldWord::Circle,
        })
    }
    #[must_use]
    pub fn refusal(value: &Refusal) -> Self {
        Self::Refusal(match value {
            Refusal::Stale(_) => RefusalWord::Stale,
            Refusal::Full(_) => RefusalWord::Full,
            Refusal::Circle { .. } => RefusalWord::Circle,
            Refusal::Dependency { .. } => RefusalWord::Dependency,
            Refusal::Kept { .. } => RefusalWord::Kept,
            Refusal::NoEscalationHolder => RefusalWord::NoEscalationHolder,
            Refusal::Malformed { .. } => RefusalWord::Malformed,
            Refusal::Role { .. } => RefusalWord::Role,
            Refusal::Taken { .. } => RefusalWord::Taken,
            Refusal::NotWaiting => RefusalWord::NotWaiting,
            Refusal::NoFurther => RefusalWord::NoFurther,
            Refusal::Authority { lacking: crate::contract::Lacking::HoldOwner { owner, .. }, .. } => match owner {
                crate::contract::HoldOwner::Party { .. } => RefusalWord::HoldOwnerParty,
                crate::contract::HoldOwner::Task { .. } => RefusalWord::HoldOwnerTask,
            },
            Refusal::Authority { .. } => RefusalWord::Authority,
            Refusal::Limit { .. } => RefusalWord::Limit,
            Refusal::Moved { .. } => RefusalWord::Moved,
            Refusal::Ended { .. } => RefusalWord::Ended,
            Refusal::Provider => RefusalWord::Provider,
            Refusal::Unknown => RefusalWord::Unknown,
            Refusal::NameTaken => RefusalWord::NameTaken,
            Refusal::NotOffered => RefusalWord::NotOffered,
            Refusal::KeyConflict => RefusalWord::KeyConflict,
            Refusal::Llm { .. } => RefusalWord::Llm,
            Refusal::SignInRedirect { why } => match why {
                crate::contract::SignInRedirectWhy::NotWaiting => RefusalWord::SignInRedirectNotWaiting,
                crate::contract::SignInRedirectWhy::Address => RefusalWord::SignInRedirectAddress,
                crate::contract::SignInRedirectWhy::State => RefusalWord::SignInRedirectState,
                crate::contract::SignInRedirectWhy::TooMany => RefusalWord::SignInRedirectTooMany,
            },
            Refusal::AccountMove { why } => match why {
                crate::contract::AccountMoveWhy::NotSignedIn => RefusalWord::AccountMoveNotSignedIn,
                crate::contract::AccountMoveWhy::NotOurs => RefusalWord::AccountMoveNotOurs,
                crate::contract::AccountMoveWhy::Held => RefusalWord::AccountMoveHeld,
                crate::contract::AccountMoveWhy::Cancelled => RefusalWord::AccountMoveCancelled,
                crate::contract::AccountMoveWhy::Moving => RefusalWord::AccountMoveMoving,
            },
            Refusal::Account { why, .. } => match why {
                crate::contract::AccountWhy::Unknown => RefusalWord::AccountUnknown,
                crate::contract::AccountWhy::Provider => RefusalWord::AccountProvider,
                crate::contract::AccountWhy::Uses => RefusalWord::AccountUses,
                crate::contract::AccountWhy::Limited => RefusalWord::AccountLimited,
            },
        })
    }
    #[must_use]
    pub fn waiting(value: &WaitsOn) -> Self {
        Self::Waiting(match value {
            WaitsOn::Dependencies { .. } => WaitingWord::Dependencies,
            WaitsOn::Party { .. } => WaitingWord::Party,
            WaitsOn::Delegates => WaitingWord::Delegates,
            WaitsOn::News => WaitingWord::News,
            WaitsOn::Nothing => WaitingWord::Nothing,
        })
    }
    #[must_use]
    pub fn status(value: &ChatStatus) -> Self {
        Self::Status(match value {
            ChatStatus::HostRefused(_) => StatusWord::HostRefused,
            ChatStatus::Starting => StatusWord::Starting,
            ChatStatus::Writing => StatusWord::Writing,
            ChatStatus::Tool { .. } => StatusWord::Tool,
            ChatStatus::Working { .. } => StatusWord::Working,
            ChatStatus::WaitingForYou { .. } => StatusWord::WaitingForYou,
            ChatStatus::WaitingFor(_) => StatusWord::WaitingFor,
            ChatStatus::Parked => StatusWord::Parked,
            ChatStatus::Retrying { .. } => StatusWord::Retrying,
            ChatStatus::Held(_) => StatusWord::Held,
            ChatStatus::Closing => StatusWord::Closing,
            ChatStatus::Ended { .. } => StatusWord::Ended,
        })
    }
    #[must_use]
    pub fn shape(self, text: u32) -> PhraseShape {
        let mut holes = List::with_capacity(3);
        for kind in self.data().2 {
            holes
                .push(HoleShape { kind: *kind, bytes: if *kind == HoleKind::Name { text } else { 0 } })
                .expect("one bounded hole");
        }
        PhraseShape { id: self.id(), holes: holes.into_boxed(), plural: None }
    }
}
/// All public words, for composing phrasebook capacities without allocation.
pub const ALL: [Word; 120] = [
    Word::Refusal(RefusalWord::Stale),
    Word::Refusal(RefusalWord::Full),
    Word::Hold(HoldWord::Exceeded(Bound::Reasoning)),
    Word::Hold(HoldWord::Exceeded(Bound::Metadata)),
    Word::Hold(HoldWord::Exceeded(Bound::Items)),
    Word::Hold(HoldWord::Exceeded(Bound::Answer)),
    Word::Hold(HoldWord::Exceeded(Bound::Tools)),
    Word::Hold(HoldWord::Exceeded(Bound::HistoryItems)),
    Word::Hold(HoldWord::Exceeded(Bound::History)),
    Word::Hold(HoldWord::Exceeded(Bound::Window)),
    Word::Hold(HoldWord::Exceeded(Bound::Output)),
    Word::Status(StatusWord::HostRefused),
    Word::Hold(HoldWord::Circle),
    Word::Refusal(RefusalWord::Circle),
    Word::Refusal(RefusalWord::Dependency),
    Word::Refusal(RefusalWord::Kept),
    Word::Waiting(WaitingWord::Dependencies),
    Word::Phase(Phase::Waiting),
    Word::Phase(Phase::Idle),
    Word::Phase(Phase::Due),
    Word::Phase(Phase::Preparing),
    Word::Phase(Phase::Claimed),
    Word::Phase(Phase::Running),
    Word::Phase(Phase::BackingOff),
    Word::Phase(Phase::Active),
    Word::Phase(Phase::Closing),
    Word::Phase(Phase::Held),
    Word::Phase(Phase::Ended),
    Word::Phase(Phase::Done),
    Word::Phase(Phase::Failed),
    Word::Phase(Phase::Cancelled),
    Word::Hold(HoldWord::Failures),
    Word::Hold(HoldWord::Stalled),
    Word::Hold(HoldWord::Dependency),
    Word::Hold(HoldWord::Stopped),
    Word::Hold(HoldWord::Placed),
    Word::Hold(HoldWord::Drift),
    Word::Hold(HoldWord::Effects),
    Word::Hold(HoldWord::EffectFailed),
    Word::Hold(HoldWord::Uncertain),
    Word::Hold(HoldWord::Budget),
    Word::Hold(HoldWord::Deadline),
    Word::Hold(HoldWord::HoldsWaited),
    Word::Hold(HoldWord::ResourceUnavailable),
    Word::Hold(HoldWord::ResourceAccess),
    Word::Hold(HoldWord::PoolLost),
    Word::Hold(HoldWord::Procedure),
    Word::Refusal(RefusalWord::NoEscalationHolder),
    Word::Refusal(RefusalWord::Malformed),
    Word::Refusal(RefusalWord::Role),
    Word::Refusal(RefusalWord::Taken),
    Word::Refusal(RefusalWord::NotWaiting),
    Word::Refusal(RefusalWord::NoFurther),
    Word::Refusal(RefusalWord::Authority),
    Word::Refusal(RefusalWord::Limit),
    Word::Refusal(RefusalWord::Moved),
    Word::Refusal(RefusalWord::Ended),
    Word::Refusal(RefusalWord::Provider),
    Word::Refusal(RefusalWord::Unknown),
    Word::Refusal(RefusalWord::NotOffered),
    Word::Refusal(RefusalWord::NameTaken),
    Word::Refusal(RefusalWord::KeyConflict),
    Word::Refusal(RefusalWord::Llm),
    Word::Refusal(RefusalWord::AccountUnknown),
    Word::Refusal(RefusalWord::AccountProvider),
    Word::Refusal(RefusalWord::AccountUses),
    Word::Refusal(RefusalWord::AccountLimited),
    Word::Refusal(RefusalWord::AccountMoveNotSignedIn),
    Word::Refusal(RefusalWord::AccountMoveNotOurs),
    Word::Refusal(RefusalWord::AccountMoveHeld),
    Word::Refusal(RefusalWord::AccountMoveCancelled),
    Word::Refusal(RefusalWord::AccountMoveMoving),
    Word::Refusal(RefusalWord::SignInRedirectNotWaiting),
    Word::Refusal(RefusalWord::SignInRedirectAddress),
    Word::Refusal(RefusalWord::SignInRedirectState),
    Word::Refusal(RefusalWord::SignInRedirectTooMany),
    Word::Waiting(WaitingWord::Party),
    Word::Waiting(WaitingWord::Delegates),
    Word::Waiting(WaitingWord::News),
    Word::Waiting(WaitingWord::Nothing),
    Word::Status(StatusWord::Starting),
    Word::Status(StatusWord::Writing),
    Word::Status(StatusWord::Tool),
    Word::Status(StatusWord::Working),
    Word::Status(StatusWord::WaitingForYou),
    Word::Status(StatusWord::WaitingFor),
    Word::Status(StatusWord::Parked),
    Word::Status(StatusWord::Retrying),
    Word::Status(StatusWord::Held),
    Word::Status(StatusWord::Closing),
    Word::Status(StatusWord::Ended),
    Word::Decision(DecisionWord::Dangerous),
    Word::Decision(DecisionWord::ReleaseFresh),
    Word::Decision(DecisionWord::ResumeRelease),
    Word::Decision(DecisionWord::EngineFollowsChoice),
    Word::Decision(DecisionWord::TakeoverFunds),
    Word::Decision(DecisionWord::TasksWaiting),
    Word::Decision(DecisionWord::TasksRunning),
    Word::Decision(DecisionWord::TasksHeld),
    Word::Decision(DecisionWord::TasksClosing),
    Word::Decision(DecisionWord::TasksDone),
    Word::Decision(DecisionWord::TasksFailed),
    Word::Decision(DecisionWord::TasksCancelled),
    Word::Decision(DecisionWord::LiveRuns),
    Word::Decision(DecisionWord::PendingProposals),
    Word::Decision(DecisionWord::TasksEnded),
    Word::Decision(DecisionWord::EffectsSettle),
    Word::Decision(DecisionWord::Funds),
    Word::Decision(DecisionWord::Needed),
    Word::Decision(DecisionWord::Left),
    Word::Decision(DecisionWord::LeftAbsent),
    Word::Decision(DecisionWord::AfterFunding),
    Word::Decision(DecisionWord::AfterAbsent),
    Word::Decision(DecisionWord::FunderTask),
    Word::Decision(DecisionWord::FunderPool),
    Word::Decision(DecisionWord::FunderRecurring),
    Word::Decision(DecisionWord::FunderPeriod),
    Word::Lifted,
    Word::Refusal(RefusalWord::HoldOwnerParty),
    Word::Refusal(RefusalWord::HoldOwnerTask),
];
#[must_use]
pub fn phrase_shapes(text: u32) -> Box<[PhraseShape]> {
    let mut shapes = List::with_capacity(u32::try_from(ALL.len()).expect("closed word count"));
    for word in ALL {
        shapes.push(word.shape(text)).expect("all word shapes fit");
    }
    shapes.into_boxed()
}
#[must_use]
pub fn english() -> Language {
    english::language()
}
