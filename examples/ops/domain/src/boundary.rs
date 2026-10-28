//! The root wraps its children's vocabulary. Replace the connector arms when
//! copying this application; keep Store and Released (domain/root.md, 2–4).
use crate::{Range, Record, Write};
use alloc::boxed::Box;
use jig_domain_core as core;
use jig_domain_fleet as fleet;
use jig_domain_host as host;
use jig_domain_tasks as tasks;
use jig_ops_domain_infrastructure as infrastructure;
use jig_ops_domain_observability as observability;
use skein_lib::{Queue, ReplyTo, Token, Wall};

/// One store response, belonging to its named owner.
#[derive(Debug)]
#[expect(clippy::large_enum_variant, reason = "store replies carry the core's bounded proposal event whole")]
pub enum Store {
    /// This whole transaction is durable.
    Committed { number: u64, fill: u64 },
    /// This transaction failed and no later held output may leave.
    Failed { number: u64 },
    /// One row in the core's requested restoration order.
    Restore(Record),
    /// The store completed this exact restart range.
    Restored(core::RestartStep),
    /// The generation is active; save the start’s wires.
    Switched,
    /// Ordered opaque transcript rows for a prepared activation.
    Transcript { task: u64, rows: Box<[core::TranscriptRow]>, done: bool },
    /// A child-specific loaded page in the core's vocabulary.
    Loaded(core::Event),
}

/// A released output addressed to a child; iterate feeds it back as one event.
#[derive(Debug)]
#[expect(clippy::large_enum_variant, reason = "a released continuation moves once, whole, through the journal")]
pub enum Released {
    /// A core continuation.
    Core(core::Event),
    /// A fleet continuation named by the core.
    Fleet(fleet::Event),
    /// One infrastructure entry may now be made.
    Infrastructure(infrastructure::Event),
    /// One observability entry may now be made.
    Observability(observability::Event),
    /// The engine's own run hub, over its permanent link.
    Host(host::Event),
    /// A procedure may choose its next step after the preceding decision.
    Procedure { task: u64, step: u64, connector: u16, code: u32 },
    /// One core-selected stage of cold restart.
    Restart(core::RestartStep),
    /// The start’s mark is durable.
    RestartDone(core::RestartStep),
}

/// Inputs to the reference root; protocol adapters supply typed child values.
#[derive(Debug)]
pub enum Event {
    /// Watch a typed connector subject, named within the client's declared scope.
    WatchApplication {
        watcher: Token,
        sign_in: u64,
        scope: jig_domain_people::ApplicationScope,
        subject: ClientSubject,
        window: core::documents::Window,
    },
    /// Read a typed connector document without a key.
    ReadApplication { reply: ReplyTo, sign_in: u64, scope: jig_domain_people::ApplicationScope, subject: ClientSubject },
    /// A decoded infrastructure request with its protocol encoding's digest.
    Application {
        reply_to: ReplyTo,
        sign_in: u64,
        key: [u8; 16],
        project: u32,
        digest: [u8; 32],
        value: infrastructure::ApplicationRequest,
    },
    /// A typed core input, including authenticated parties and accounts.
    Core(core::Event),
    /// Observability's typed protocol input.
    Observability(observability::Event),
    /// Infrastructure's typed protocol input.
    Infrastructure(infrastructure::Event),
    /// The host's agent or engine-side input.
    Host(host::Event),
    /// What the hub's agent capability reports. Ops runs no agent in its
    /// engine; whoever hosts its runs' agents answers the hub's requests.
    Agent(host::FromAgent),
    /// What a run's agent streams while a completion is in flight, for the
    /// run's followers (client/domain/contract.md, 7.3).
    AgentContent { owner: Token, content: AgentContent },
    /// A typed store terminal or page.
    Store(Store),
    /// A child-addressed output released by the journal.
    Released(Released),
    /// Begin the core's cold-start script.
    Restart { pending: bool },
    /// One due core timer.
    Timer(core::Timer),
    /// One observability retry deadline.
    ObservabilityTimer,
    /// One infrastructure retry deadline.
    InfrastructureTimer,
    /// One due backoff for the engine hub's retained turn.
    TurnTimer,
    /// A result read by the protocol layer after the fleet fenced its answer.
    ResultRead { payload: Token, end: tasks::End },
    /// A connector read decoded outside the root, with its authenticated task.
    ConnectorRead { to: ReplyTo, task: u64, name: Box<[u8]>, tool: Box<[u8]>, read: observability::Read },
    /// A rendered no-record answer, for a read or an undecodable call.
    CallAnswered { to: ReplyTo, call: core::SettledCall },
    /// A decoded infrastructure effect with the core's caller envelope.
    Effect {
        to: ReplyTo,
        key: core::CallKey,
        effect: infrastructure::Effect,
        deadline: Wall,
        proposal: Option<Box<[u8]>>,
    },
}

/// A run's live content, named by the hub's owner of its agent.
#[derive(Debug)]
pub enum AgentContent {
    /// Text at `offset` of the completion `turn`'s `retry`; empty as it begins.
    Text { turn: u32, retry: u32, offset: u32, text: Box<[u8]> },
    /// A call the completion makes.
    Call(jig_domain_views::Call),
}

/// A timer retains its owner's identity through the root's admission.
#[derive(Clone, Copy, Debug)]
pub enum Timer {
    /// A core child selected by the timer adapter.
    Core(core::Timer),
    /// Observability's retry deadline.
    Observability,
    /// Infrastructure's retry deadline.
    Infrastructure,
    /// The engine hub's retained turn backoff.
    Turn,
}

/// The journal is the only source of root outputs.
#[derive(Debug)]
#[expect(clippy::large_enum_variant, reason = "the journal owns complete concrete outputs")]
pub enum Output {
    /// A made project, released after its commit.
    ProjectMade { project: u32 },
    /// One whole connector-owned document held behind the journal's barrier.
    ApplicationDocument { watcher: Token, document: ClientDocument },
    /// One connector-owned read terminal after all reflected commits are durable.
    ApplicationRead { reply: ReplyTo, answer: Result<ClientDocument, ClientMissing> },
    /// A connector refused an incident association; the journal preserves the reason.
    IncidentAssociationRefused { task: u64, step: u64, reason: observability::AssociationRefusal },
    /// Infrastructure's own outcome, released only by the commit barrier.
    Application { to: ReplyTo, outcome: infrastructure::ApplicationOutcome },
    /// One atomic transaction, numbered by the journal.
    Commit { number: u64, writes: Queue<Write> },
    /// Make the pending generation active, or answer Switched immediately.
    Switch,
    /// Load this child's rows for the core-selected restart step.
    Load { step: core::RestartStep, range: Range },
    /// A core-owned store read, routed by its variant.
    CoreLoad(core::Now),
    /// Core output for its party, view, note or external protocol owner.
    Core(core::Held),
    /// Core output with no durable dependency, such as a view fact.
    Now(core::Now),
    /// One request to the shared production's observability API.
    Observability(observability::SystemRequest),
    /// One request to the shared production's infrastructure API.
    Infrastructure(infrastructure::SystemRequest),
    /// An ended answer whose result the protocol layer reads after the fence.
    ReadResult { payload: Token, outcome: Box<[u8]> },
    /// A bounded read answer, with no state-changing effect.
    Read { owner: Token, bytes: Box<[u8]> },
    /// A request to the hub's agent capability, after its commit.
    Agent(host::ToAgent),
    /// Feed this released continuation back as an event.
    ToChild(Released),
    /// A failed journal commit or refused restart stopped this engine.
    Stop,
}

/// A turn or terminal the fleet asks the root to carry back by opaque token.
#[derive(Debug)]
pub(crate) enum Payload {
    Turn {
        task: u64,
        attempt: u64,
        turn: u32,
        spent: u64,
        read: Option<u64>,
        body: Box<[u8]>,
        opens_window: bool,
        refused: Box<[(u64, host::MessageRefusal)]>,
    },
    Answer {
        task: u64,
        attempt: u64,
        spent: u64,
        ending: host::Ending,
        read: Option<u64>,
        refused: Box<[(u64, host::MessageRefusal)]>,
    },
    Word(tasks::Word),
    Settled(core::SettledCall),
}

/// Each connector owns its typed client subject; the root reads its variant only.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum ClientSubject {
    Infrastructure(infrastructure::ClientSubject),
    Observability(observability::ClientSubject),
}

/// The journal's owned assembly of either connector's document.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum ClientDocument {
    Infrastructure(Box<infrastructure::ClientDocument>),
    Observability(Box<observability::ClientDocument>),
}

/// A connector-owned read's total refusal translation.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ClientMissing {
    Unknown,
    NotAllowed,
}
