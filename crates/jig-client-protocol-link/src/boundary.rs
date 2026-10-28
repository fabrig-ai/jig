//! Owner routing and shell exchanges (protocol/client.md, 2 and 5).
use alloc::boxed::Box;
use jig_client_domain::{Answered, Ask, Family, Readable, Readout, Retry, Watchable, contract};
use skein_lib::{List, RequestEnvelope, RequestKey, RequestRecord, Secret, Token};
/// A record the shell read with its 16-byte request key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub key: Box<[u8]>,
    pub bytes: Box<[u8]>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved {
    pub version: u16,
    pub record: RequestRecord<jig_client_domain::Held>,
}
#[derive(Debug)]
pub enum Event {
    SignIn {
        with: With,
    },
    SignOut,
    Authorize {
        query: Box<[u8]>,
    },
    Start {
        kept: List<Record>,
    },
    Save {
        record: Saved,
    },
    Erase {
        key: RequestKey,
    },
    /// Every write and removal of this step is durable.
    Stored,
    /// This step's writes failed; withhold and fail its held exchanges.
    NotStored,
    Send {
        attempt: Token,
        key: RequestKey,
        ask: Ask,
        /// A value the request carries once, put into `Keyed` and never kept (protocol/client.md, 5).
        value: Option<Secret>,
    },
    Read {
        read: Token,
        what: Readable,
    },
    Reply {
        exchange: Token,
        message: Box<[u8]>,
    },
    Lost {
        exchange: Token,
    },
    Full {
        exchange: Token,
        full: jig_client_domain::StreamFull,
    },
    Open {
        watch: Token,
        subject: Watchable,
    },
    Continue {
        watch: Token,
        subject: contract::Subject,
    },
    Close {
        watch: Token,
    },
    Streamed {
        message: Streamed,
    },
    StreamGone,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Request {
    Authorized(Authorized),
    Save {
        record: Record,
    },
    Erase {
        key: Box<[u8]>,
    },
    Kept {
        record: Saved,
    },
    Dropped,
    /// One message; its transport owns the mapping and deadline.
    Exchange {
        exchange: Token,
        credential: Option<Secret>,
        message: Message,
    },
    NotKept {
        attempt: Token,
    },
    Answer {
        attempt: Token,
        envelope: RequestEnvelope,
        answer: Option<Answered>,
        why: Option<Retry>,
    },
    /// The shell shows this once; it never enters the client's domain.
    ServiceCredential {
        party: u64,
        credential: Secret,
    },
    OpenStream {
        credential: Option<Secret>,
    },
    Signing {
        signing: Signing,
    },
    CloseStream,
    Watch {
        watch: Token,
        event: contract::WatchEvent,
    },
    FamilyDelivery {
        watch: Token,
        record: Family,
    },
    Ended {
        watch: Token,
        end: contract::WatchEnd,
    },
    Hello {
        family: Option<u16>,
        versions: Option<contract::VersionRange>,
    },
    /// One terminal per read.
    Read {
        read: Token,
        result: Readout,
    },
    Fact {
        fact: Fact,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fact {
    UndecodableAnswer,
    UndecodableDocument,
    UndecodableEvent,
    StreamDropped,
    UndecodableStore,
    StreamFull { full: jig_client_domain::StreamFull },
}

/// Authentication held by the link (client/domain/client.md, 12).
#[derive(Debug, PartialEq, Eq)]
pub enum With {
    Service(Secret),
    Transport,
    Link { secret: Secret, held: Option<Secret> },
    Held(Secret),
}
#[derive(Debug, PartialEq, Eq)]
pub enum Signing {
    SignedIn { party: u64 },
    Again,
    Busy,
    Full { sign_ins: u32, earliest: skein_lib::Wall, link_until: skein_lib::Wall },
    Refused,
    Lost,
    SignedOut,
}

/// A protocol record, independent of its transport's route.
#[derive(Debug, PartialEq, Eq)]
pub enum Message {
    Authorize(Box<[u8]>),
    Request(Box<[u8]>),
    Read(Box<[u8]>),
    Watch(Box<[u8]>),
    Unwatch(Box<[u8]>),
    StreamGone(Box<[u8]>),
    SignInLink(Box<[u8]>),
    SignInService,
    SignOut,
}
/// A transport exchange's terminal outcome.
#[derive(Debug, PartialEq, Eq)]
pub enum Heard {
    Reply { exchange: Token, message: Box<[u8]> },
    Lost { exchange: Token },
    Full { exchange: Token, full: jig_client_domain::StreamFull },
    Streamed { message: Streamed },
    StreamGone,
}

/// One whole message cut by the transport from its stream.
#[derive(Debug, PartialEq, Eq)]
pub enum Streamed {
    Record(Box<[u8]>),
    Heartbeat,
    /// The stream refused, its opening answered signed out.
    SignedOut,
    Full(jig_client_domain::StreamFull),
}

/// Authorization's terminal answer (protocol/client.md, 18).
#[derive(Debug, PartialEq, Eq)]
pub enum Authorized {
    Go { location: Box<[u8]> },
    Refused(AuthorizeWhy),
    SignedOut,
    NotReady,
    Busy,
    Lost,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthorizeWhy {
    Malformed,
    UnknownClient,
    UnknownRedirect,
    NoProvider,
    NotAPerson,
}
