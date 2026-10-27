//! Worker channel records, without domain or payload policy.
//! Contracts: protocol/channel.md, sections 3, 4.1 and 6.2.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
extern crate alloc;
use alloc::boxed::Box;
use skein_lib::Token;

#[rustfmt::skip]
#[path = "generated/v9.rs"]
pub mod v9;
mod kinds;
pub mod sizes;
#[cfg(test)]
mod tests;

pub use kinds::{KINDS, Kind, KindRule, Overflow, schema, terms_gap};
pub use v9::{
    AccountRefused, AccountRefusedParts, AgentFailure, AgentFailureValue, AgentFailureValueParts, Answer, AnswerParts,
    Bound, Call, CallParts, CancelReason, CancelReasonValue, CancelReasonValueParts, Count, CountKind, CountParts,
    Ended, EndedParts, Ending, Exceeded, ExceededParts, Fact, FactParts, FactValue, Failed, FailedParts, Failure,
    HostPart, HostPartParts, Invalid, InvalidStart, InvalidStartParts, InvalidValue, InvalidValueParts, Limited,
    LimitedAccount, LimitedAccountParts, LimitedParts, Long, LongParts, MessageRefusal, Operation, Parked, ParkedParts,
    PhaseValue, PhaseValueParts, Preparation, PreparationValue, PreparationValueParts, Refusal, RefusalValue,
    RefusalValueParts, Refused, RefusedAccount, RefusedAccountParts, RefusedParts, Resource, ResourceParts, RunFailure,
    RunFailureValue, RunFailureValueParts, StartCause, Turn, TurnParts, Unauthorized, UnauthorizedParts, Work,
    WorkParts,
};
pub use v9::{
    Acknowledge, AcknowledgeParts, AnsweredCall, AnsweredCallParts, ApplicationValue, ApplicationValueParts,
    Assignment, AssignmentParts, Cancel, CancelParts, CredentialValue, CredentialValueParts, DeliveryAnswer,
    DeliveryAnswerParts, DeliveryOutcome, Grant, GrantParts, GrantValue, Granted, GrantedParts, HostAnswer,
    HostAnswerParts, Inbound, InboundParts, Message, MessageParts, ProgramValue, ProgramValueParts, RelayAnswer,
    RelayText, RelayTextParts, Relayed, RelayedParts, Scheme, SettledAnswer, TurnBusy, TurnBusyParts, TurnKept,
    TurnKeptParts, Workspace, WorkspaceParts,
};
pub use v9::{
    Bounds, BoundsParts, CEILINGS, Credential, CredentialParts, Hello, HelloParts, Hosting, HostingParts, ModelBound,
    ModelBoundParts, Phase,
};

/// An application's opaque bytes under the token its vocabulary names.
#[derive(PartialEq, Eq, Debug)]
pub struct Part {
    pub token: Token,
    pub bytes: Box<[u8]>,
}

/// The magic and one version this release speaks (protocol/channel.md, 4.1).
pub const MAGIC: [u8; 4] = *b"jigw";
pub const VERSION: u16 = 9;

/// Jig's refusal reasons, above skein's reserved range (protocol/channel.md, 4.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Reason {
    Unauthorized,
    Application,
    Busy,
    Replaced,
    NotAdmitted,
    Stopping,
}

impl Reason {
    #[must_use]
    pub const fn code(self) -> u16 {
        match self {
            Self::Unauthorized => 256,
            Self::Application => 257,
            Self::Busy => 258,
            Self::Replaced => 259,
            Self::NotAdmitted => 260,
            Self::Stopping => 261,
        }
    }

    #[must_use]
    pub const fn from_code(code: u16) -> Option<Self> {
        match code {
            256 => Some(Self::Unauthorized),
            257 => Some(Self::Application),
            258 => Some(Self::Busy),
            259 => Some(Self::Replaced),
            260 => Some(Self::NotAdmitted),
            261 => Some(Self::Stopping),
            _ => None,
        }
    }
}

pub use v9::{
    Check, CheckParts, Login, LoginOtherwise, LoginOtherwiseParts, LoginParts, LoginState, Logins, LoginsParts,
    ProgramSource, ProgramSourceParts, SignOut, SignOutParts, SignOutResult, SignedOut, SignedOutParts, SourceFailed,
    SourceFailedParts, SourceFailure, Sources, SourcesParts,
};

pub use v9::{InputCall, InputCallParts, LiveAsk, LiveAskParts, LiveKind, Piece, PieceParts};
