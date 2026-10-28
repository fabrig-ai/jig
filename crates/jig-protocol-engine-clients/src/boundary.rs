//! The protocol routing's vocabulary (protocol/client.md, sections 2 and 8).
use alloc::boxed::Box;
use jig_domain_core as core;
use jig_domain_people as people;
use jig_domain_views as views;
use skein_lib::{Duration, List, ReplyTo, Secret, Token, Wall};
#[derive(Debug)]
pub enum Event {
    Transport(FromTransport),
    Reply {
        to: ReplyTo,
        reply: people::Reply,
    },
    SignInRefused {
        to: ReplyTo,
    },
    SignInEnded {
        sign_in: u64,
    },
    Read {
        reply: ReplyTo,
        answer: core::reads::Answer,
    },
    Admitted {
        watcher: Token,
    },
    WatchRefused {
        watcher: Token,
        why: core::WatchRefusal,
    },
    Delivery {
        watcher: Token,
        document: Box<core::documents::Document>,
    },
    WatchEnded {
        watcher: Token,
        why: views::End,
    },
    RunText {
        watcher: Token,
        turn: views::TurnName,
        from: u32,
        to: u32,
        text: Box<[u8]>,
        cut: bool,
        call: Option<views::Call>,
        writing: Option<views::WritingCall>,
    },
    Application(FromApplication),
    Secrets(SecretsAnswer),
    Mint {
        exchange: Token,
        identity: people::Identity,
    },
    Redeem {
        exchange: Token,
        authorization: Option<Secret>,
        content_type: Option<Box<[u8]>>,
        body: Secret,
    },
    Claims {
        exchange: Token,
        authorization: Option<Secret>,
    },
    MovedTaken {
        exchange: Token,
        record: Option<Secret>,
    },
    Stop,
    Close,
    Abort,
}
#[derive(Debug)]
pub enum Request {
    Transport(ToTransport),
    Core(Box<core::Event>),
    Application(ToApplication),
    Secrets(SecretsRequest),
    Minted { exchange: Token, credential: Secret, expires: Wall },
    MintRefused { exchange: Token, why: MintRefusal },
    Link { secret: Secret },
    TakeMoved { exchange: Token, source: u32 },
    Redeemed { exchange: Token, answer: ProviderAnswer },
    Claimed { exchange: Token, answer: ProviderAnswer },
    Closed,
}
#[derive(Debug)]
pub enum ToApplication {
    Request {
        reply_to: ReplyTo,
        sign_in: u64,
        key: [u8; 16],
        family: u16,
        version: u32,
        scope: people::ApplicationScope,
        record: Box<[u8]>,
        digest: [u8; 32],
        /// The value the request carried, by its number and digest: kept in
        /// the secrets' store before this request (protocol/client.md, 5).
        carried: Option<core::Carried>,
    },
    Read {
        reply: ReplyTo,
        sign_in: u64,
        family: u16,
        version: u32,
        scope: people::ApplicationScope,
        window: jig_domain_core::documents::Window,
        record: Box<[u8]>,
    },
    Watch {
        watcher: Token,
        sign_in: u64,
        family: u16,
        version: u32,
        scope: people::ApplicationScope,
        window: jig_domain_core::documents::Window,
        record: Box<[u8]>,
    },
}
#[derive(Debug)]
pub enum FromApplication {
    Answered { to: ReplyTo, family: u16, record: Box<[u8]> },
    Read { reply: ReplyTo, family: u16, record: Option<Box<[u8]>> },
    Delivery { watcher: Token, family: u16, record: Box<[u8]> },
    Undecodable { to: ReplyTo },
    // The core left a carried value unused (`core::Held::CarriedUnused`):
    // erased when this component's own decode kept it (store.md 9).
    Unused { number: u64 },
}
#[derive(Debug)]
pub enum SecretsRequest {
    Keep { owner: SecretOwner, value: Kept },
    Erase { owner: SecretOwner },
    Get { owner: SecretOwner },
    // Keep the value `exchange`'s request carried, under a number the store
    // draws itself (store.md 9): answered `Carried`.
    Carry { exchange: Token, value: Secret, kind: CarriedKind },
    // Hold `exchange`'s value in memory, never written (store.md 9):
    // answered `Carried` as a carry is.
    Hold { exchange: Token, value: Secret },
}
/// The kind of a persistent request value (store.md 9).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CarriedKind {
    Key,
    Value,
    Moved,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SecretOwner {
    SignIn(u64),
    Service(u64),
    /// A value a request carried, by the number the store drew; only erased.
    Carried(u64),
}
#[derive(Debug)]
pub enum SecretsAnswer {
    Kept { owner: SecretOwner },
    Erased { owner: SecretOwner },
    Got { owner: SecretOwner, value: Option<Kept> },
    // `exchange`'s value is kept or held under `number`; none when the
    // store accepted nothing.
    Carried { exchange: Token, number: Option<u64> },
    Failed,
}
#[derive(Clone, Debug)]
pub struct Kept {
    pub digest: Secret,
    pub expires: Wall,
}
/// Why an application's mint made no credential (protocol/client.md, 8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MintRefusal {
    NotApplication,
    Bound,
    NotReady,
    Busy,
}
#[derive(Debug)]
pub struct Settings {
    pub provider: Option<skein_oauth_issuer::Issuer>,
    pub addresses: Box<[u16]>,
    pub link: Option<people::Identity>,
    pub deployment_provider: u16,
    pub heartbeat: Duration,
    /// Default watch window from the receiving core's document limits.
    pub rows: u32,
    pub served: List<Served>,
    /// Providers of kind application, whose verified identities Mint accepts.
    pub providers: List<u16>,
    pub seed: Secret,
    pub stream: u64,
    /// The configured models in configuration's order: the client names a model and its effort
    /// by their names, which the component maps to the core's indices (contract.md, 3.2 and 5.1).
    pub models: Box<[Model]>,
    /// The key bound advertised by me, independent of the moved file bound.
    pub key_bytes: u32,
    /// The driver's redirect bound: the most bytes a pasted sign-in address may have.
    pub redirect_bytes: u32,
    /// The moved file form's bound under the configured accounts limits.
    pub moved_bytes: u64,
}
/// A configured model as the client protocol names and shows it (configuration.md, 3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Model {
    pub name: Box<[u8]>,
    /// The number of the LLM provider serving it, when configuration names one.
    pub provider: Option<u32>,
    /// Its context window in tokens.
    pub window: u32,
    /// Its longest completion in tokens.
    pub output: u32,
    /// The efforts it declares, by name in its order.
    pub efforts: Box<[Box<[u8]>]>,
    pub prices: Prices,
    pub description: Option<Box<[u8]>>,
}
/// Configured amounts per `unit` tokens in the deployment's spend unit; absent cache writes
/// are priced at input (configuration.md, 3; domain/authority.md, 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Prices {
    pub input: u64,
    pub cached: u64,
    pub cache_write: Option<u64>,
    pub output: u64,
    pub unit: u32,
}
#[derive(Clone, Copy, Debug)]
pub struct Served {
    pub family: Option<u16>,
    pub first: u32,
    pub last: u32,
}

/// Messages from an opaque transport (protocol/client.md, 4.2).
#[derive(Debug, PartialEq)]
pub enum FromTransport {
    Exchange { exchange: Token, credential: Option<Secret>, message: Message },
    StreamOpen { stream: Token, credential: Option<Secret> },
    StreamTaken { stream: Token },
    StreamGone { stream: Token },
}
#[derive(Debug, PartialEq)]
pub enum Message {
    Authorize(Box<[u8]>),
    Request(Box<[u8]>),
    Read(Box<[u8]>),
    Watch(Box<[u8]>),
    Unwatch(Box<[u8]>),
    Gone(Box<[u8]>),
    SignInLink(Box<[u8]>),
    SignInService(Box<[u8]>),
    SignOut(Box<[u8]>),
}
#[derive(Debug)]
pub enum ToTransport {
    Reply { exchange: Token, reply: Reply },
    Streamed { stream: Token, message: Streamed },
    StreamEnded { stream: Token, why: Ended },
}
#[derive(Debug)]
pub enum Reply {
    Record(Box<[u8]>),
    NotARecord,
    Conflict,
    StreamFull(jig_client_protocol_wire::v19::StreamFull),
}
#[derive(Debug)]
pub enum Streamed {
    Record(Box<[u8]>),
    Heartbeat,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ended {
    SignedOut,
    Stopping,
    Busy,
}

/// A provider endpoint answer (client.md, 18); a body may contain bearer tokens.
#[derive(Debug)]
pub struct ProviderAnswer {
    pub status: u16,
    pub challenge: Option<Box<[u8]>>,
    pub body: Secret,
}
