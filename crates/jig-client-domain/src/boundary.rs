//! Client handles and exchanges (client/domain/client.md, 3 and 8).
use crate::contract;
use alloc::boxed::Box;
use skein_lib::{List, RequestEnvelope, RequestKey, RequestRecord, Secret, Token};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spoken {
    pub family: u16,
    pub version: u32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Family {
    pub family: u16,
    pub version: u32,
    pub bytes: Box<[u8]>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ask {
    Jig(Box<contract::Ask>),
    Family { scope: contract::Scope, record: Family },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Readable {
    Jig(contract::Read),
    Family { scope: contract::Scope, window: contract::Window, record: Family },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Watchable {
    Jig(contract::Subject),
    Family { scope: contract::Scope, window: contract::Window, record: Family },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Readout {
    /// Refused by the client's bound on reads in flight.
    Limit,
    NotServed {
        family: Option<u16>,
        served: Option<contract::VersionRange>,
    },
    Jig(contract::ReadResult),
    Family {
        family: u16,
        record: Option<Box<[u8]>>,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answered {
    Jig(contract::Answer),
    Family(Family),
}
/// The application child that owns a handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Asker(pub u32);
impl Asker {
    pub const JIG: Self = Self(65536);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Handle {
    pub asker: Asker,
    pub number: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Opening,
    Live,
    Behind,
    Ended,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Latest {
    Jig(contract::Delivery),
    Family(Family),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Document<'a> {
    pub state: State,
    pub latest: Option<&'a Latest>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    NotServed,
    Unknown,
    NotAllowed,
    SubjectEnded,
    SignedOut,
    Limit,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Start { kept: List<RequestRecord<Held>> },
    Send { key: Key, name: Option<RequestKey>, request: Ask },
    // A value carried either way: sent once under its name, never kept (client.md, 6).
    Carry { key: Key, name: RequestKey, request: Ask, value: Option<Secret> },
    Watch { handle: Handle, subject: Watchable },
    Unwatch { handle: Handle },
    Read { handle: Handle, read: Readable },
    Up { up: Up },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Up {
    SignedIn,
    SignedOut,
    NotKept { key: RequestKey },
    Served { served: bool, versions: Option<contract::VersionRange> },
    FamilyServed { family: u16, versions: Option<contract::VersionRange> },
    Answer { attempt: Token, envelope: RequestEnvelope, answer: Option<Answered>, why: Option<Retry> },
    Watched { watch: Token, event: contract::WatchEvent },
    FamilyDelivered { watch: Token, record: Family },
    WatchEnded { watch: Token, end: contract::WatchEnd },
    ReadDone { read: Token, result: Readout },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    Down { down: Down },
    Told { told: Told },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Down {
    Save { record: RequestRecord<Held> },
    Erase { key: RequestKey },
    // `value` only for a request carrying one, sent once (client.md, 6).
    Send { attempt: Token, key: RequestKey, request: Ask, value: Option<Secret> },
    Open { watch: Token, subject: Watchable },
    Continue { watch: Token, subject: contract::Subject },
    Close { watch: Token },
    Read { read: Token, what: Readable },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Told {
    Link { link: Link },
    Pending { key: Key },
    Answered { key: Key, outcome: Outcome },
    Kept { key: Key, request: Ask },
    Dropped { count: u32 },
    Changed { handle: Handle },
    Text { handle: Handle },
    Ended { handle: Handle, why: Why },
    Page { handle: Handle, page: Readout },
}

/// The application's stable request name, kept beside its wire key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Key {
    pub asker: Asker,
    pub number: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Held {
    pub key: Key,
    pub request: Ask,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    NotKept,
    Answered(Answered),
    Unknown,
    Limit,
    Unconfirmed,
    /// Its name is held by another request in the table; nothing of it kept or sent.
    Held,
    /// A request carrying a value the engine did not make; its caller asks again.
    Unsent,
    /// The engine refused admission because its retained-answer table is full.
    Full {
        held: u32,
        room_at: skein_lib::Wall,
    },
    /// A request carrying a value whose exchange ended with no whole answer:
    /// the engine may have made it; its caller asks again under its name.
    Lost,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Progress {
    NotServed,
    InFlight,
    Retrying { why: Retry },
    Parked,
    Unknown,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Retry {
    Busy,
    Full { held: u32, room_at: skein_lib::Wall },
    NotReady,
    Lost,
    Undecodable,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pending {
    pub progress: Progress,
    pub party: u64,
    pub owner: Option<u16>,
}

/// The stream and the client-owned watch of me (client/domain/client.md, 8.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Link {
    SignedOut,
    Starting,
    Live,
    Offline,
    OutOfDate,
}

/// The engine-wide bound which refused a client connection (protocol/client.md, 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamBound {
    Component,
    Transport,
    Exchanges,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamFull {
    pub bound: StreamBound,
    pub size: u32,
}
