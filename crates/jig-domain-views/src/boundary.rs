//! Opaque subjects and pacing requests (`domain/engine.md`, 11).
//! The parent owns documents, authorization and each subject's meaning.

use alloc::boxed::Box;
use skein_lib::{List, Token};

/// A bounded slice of a subject's document, interpreted only by its owner.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Window {
    pub rows: u32,
    pub after: Option<u64>,
}

/// Why a watch admission was refused.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Refusal {
    /// A configured capacity is exhausted.
    Busy,
}

/// Why a watch ended.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum End {
    /// Its watcher left.
    Unwatched,
    /// Its subject left the engine.
    Gone,
    /// Its retained facts exceed the joins allowed in one read.
    TooLinked { joins: u32 },
    /// Its party may no longer see the subject.
    NotAllowed,
    /// Its party signed out.
    SignedOut,
    /// Its parent is stopping.
    Stopping,
}

/// The parent's result of making a document; its content stays with the parent.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Made {
    /// A whole bounded document is ready.
    Ready,
    /// No visible document remains.
    Gone { why: End },
}

/// A provisional retry, fenced by its attempt and activation turn.
/// `client/domain/contract.md`, section 7.3.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct TurnName {
    pub attempt: u64,
    pub turn: u32,
    pub retry: u32,
}

/// A provider call's opaque identity (`QUESTIONS.md`, item 42).
/// Compare only for equality. The same provider id in another issued completion
/// or owner names a different call; none of these values is a routing number.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct CallIdentity {
    pub owner: Token,
    pub sequence: u32,
    pub retry: u32,
    pub id: Box<[u8]>,
}

/// A tool call being made, priced with its whole id within the byte bound.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Call {
    pub identity: CallIdentity,
    pub tool: Box<[u8]>,
    pub input: Box<[u8]>,
    pub cut: bool,
    /// Set by views when this call becomes current; consecutive repeats retain it.
    /// A different current identity restarts the clock, including A -> B -> A.
    /// An ingress supplies None. No provider or adapter invents this clock.
    pub began: Option<skein_lib::Wall>,
}

/// The provider call whose bounded input suffix is being written.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct WritingCall {
    pub id: Box<[u8]>,
    pub tool: Box<[u8]>,
    pub from: u32,
    pub through: u32,
    pub input: Box<[u8]>,
    pub cut: bool,
}

/// An opaque followed run and the client's acknowledged provisional position.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Follow {
    pub run: Token,
    pub turn: Option<TurnName>,
    pub offset: u32,
}

/// Current volatile writing, copied only when a document is constructed.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Writing {
    pub turn: TurnName,
    pub from: u32,
    pub to: u32,
    pub text: Box<[u8]>,
    pub cut: bool,
    pub call: Option<Call>,
    pub writing: Option<WritingCall>,
}

/// Events from the parent.
#[derive(PartialEq, Eq, Debug)]
pub enum Event {
    /// The parent ends this watch's admission, independently of other parties.
    EndWatch { watcher: Token, end: End },
    /// Admit an opaque subject and party under a unique watcher token.
    Watch { watcher: Token, party: Token, subject: Token, window: Window, follow: Option<Follow> },
    /// The parent finished changing these subjects.
    Changed { subjects: List<Token> },
    /// The parent made the requested document.
    Made { watcher: Token, outcome: Made },
    /// A delivery was consumed or dropped.
    Delivered { watcher: Token },
    /// A provisional stream delivery was consumed or dropped.
    StreamDelivered { watcher: Token },
    /// A live host wrote text starting at this turn's absolute offset.
    Text { run: Token, turn: TurnName, offset: u32, text: Box<[u8]>, call: Option<Call> },
    /// A main completion appends to the call it is currently writing.
    Input { run: Token, turn: TurnName, offset: u32, id: Box<[u8]>, tool: Box<[u8]>, input: Box<[u8]> },
    /// Any content call, independently of the main completion's text identity.
    Called { run: Token, attempt: u64, call: Call },
    /// A committed turn stops being provisional without ending its live run.
    TurnCommitted { run: Token, attempt: u64, turn: u32 },
    /// The run ended; discard its provisional writing, leaving its watches open.
    RunEnded { run: Token },
    /// The watcher left.
    Unwatch { watcher: Token },
    /// A subject ended or became invisible.
    Gone { subject: Token, why: End },
}

/// Requests to the parent.
#[derive(PartialEq, Eq, Debug)]
pub enum Request {
    /// The watch was admitted.
    Watching { watcher: Token },
    /// Make the current document within this watch's window.
    Make { watcher: Token, subject: Token, window: Window },
    /// Deliver the document the parent holds until durable.
    Deliver { watcher: Token },
    /// Deliver a provisional suffix; its turn and absolute positions never mix.
    Stream {
        watcher: Token,
        turn: TurnName,
        from: u32,
        to: u32,
        text: Box<[u8]>,
        cut: bool,
        call: Option<Call>,
        writing: Option<WritingCall>,
    },
    /// Admission found no capacity.
    Refused { watcher: Token, refusal: Refusal },
    /// The watch has released its last outstanding request.
    Ended { watcher: Token, end: End },
}
