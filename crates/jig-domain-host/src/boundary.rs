//! The host's boundary (domain/hosts.md, sections 4 to 7). Its root
//! translates core messages and the optional workspace and agent capabilities. A
//! request's `owner` names the hosted run or its call; a terminal echoes it.
//! The root owns application workspace items and interprets opaque tokens.

use alloc::boxed::Box;

use skein_lib::{Duration, Token};

/// parent -> host
#[derive(PartialEq, Eq, Debug)]
pub enum Event {
    SourceAnswered {
        owner: Token,
        answer: crate::sources::Answer,
    },
    SourceLost,
    /// One completed conversation turn from the agent.
    Turn {
        owner: Token,
        turn: Turn,
    },
    /// A host or delivery call named in the run's own bytes.
    Called {
        owner: Token,
        call: Box<[u8]>,
        ask: Ask,
    },
    /// The agent withdrew its call after its deadline.
    Withdrawn {
        owner: Token,
        call: Box<[u8]>,
    },
    /// A piece of live content (hosts.md, 6.5).
    Piece {
        owner: Token,
        piece: crate::Piece,
    },
    /// Typed agent activity, kept best effort for the engine.
    Activity {
        owner: Token,
        activity: crate::Activity,
    },
    /// The provider rejected the account's credential of `generation`.
    Unauthorized {
        owner: Token,
        account: u32,
        generation: u64,
    },
    /// The account reached its provider limit; `reset`, when the provider
    /// gave one, is how long until it lifts.
    Limited {
        owner: Token,
        account: u32,
        reset: Option<Duration>,
    },
    /// The provider refused the account, with its code and reason.
    AccountRefused {
        owner: Token,
        account: u32,
        code: Box<[u8]>,
        reason: Box<[u8]>,
    },
    /// Last word, with cumulative accounting.
    Finished {
        owner: Token,
        turns: u32,
        spent: u64,
        read: Option<Token>,
        finish: Finish,
    },
    /// Terminal for a cancelled relay: its local delivery and wait have ended.
    RelayCancelled {
        run: Token,
        attempt: Token,
        call: Box<[u8]>,
    },
    /// From the top level: cancel every run hosted now, for `reason` (lost
    /// contact with the engine past its grace, or shutdown). The runs are
    /// cancelled one at a time, from the ready list. A root shutting down
    /// admits no more runs: assignments after it are refused as busy.
    CancelAll {
        reason: Reason,
    },
    /// The link opened; the root supplies the hello's configuration.
    Opened {
        stop_bound: Duration,
        workstreams: Box<[u64]>,
        bounds: crate::Bounds,
    },
    Down(crate::LinkDown),
    /// Terminal for `Prepare`: the workspace is ready, and `workspace` names it
    /// from now on.
    Prepared {
        owner: Token,
        workspace: Token,
    },
    /// Terminal for `Prepare`: the workspace could not be prepared, and
    /// nothing of it is held.
    Unprepared {
        owner: Token,
        failure: Preparation,
        detail: Box<[u8]>,
    },
    /// The agent started, and `agent` names it from now on.
    Started {
        owner: Token,
        agent: Token,
    },
    /// Terminal for a start the agent refused; its bounded words name why.
    StartRefused {
        cause: StartCause,
        owner: Token,
        why: Box<[u8]>,
    },
    /// The agent could not take an inbound event for the run, for `reason`.
    Refused {
        owner: Token,
        name: Token,
        reason: MessageRefusal,
    },
    /// The run yielded: it waits for its next inbound event.
    Yielded {
        owner: Token,
    },
    /// The agent child domain is stopping the agent for `fault`.
    Faulted {
        owner: Token,
        fault: AgentFailure,
    },
    /// Terminal for `Start`: the agent and everything it started are gone.
    /// `detail` is for operators, such as the tail of its error output.
    Gone {
        owner: Token,
        detail: Box<[u8]>,
    },
    /// Terminal for a delivery, including the root's opaque left-work token.
    Delivered {
        owner: Token,
        delivery: Delivery,
    },
    /// Terminal for a save, including the root's opaque saved-work token.
    Saved {
        owner: Token,
        at: Option<Token>,
    },
}

/// host -> parent
#[derive(PartialEq, Eq, Debug)]
pub enum Request {
    Source {
        owner: Token,
        name: crate::SourceName,
        operation: crate::sources::Operation,
        deadline: skein_lib::Time,
    },
    SourceFault,
    Up(crate::LinkUp),
    Hello {
        hello: crate::Hello,
    },
    /// Pass a named message to the agent.
    Message {
        agent: Token,
        name: Token,
        sender: Box<[u8]>,
        words: Box<[u8]>,
        finish_requested: bool,
    },
    /// Answer a call under the agent's opaque name.
    Reply {
        agent: Token,
        call: Box<[u8]>,
        reply: Reply,
    },
    /// Start an agent with its activation and committed conversation state.
    Start {
        owner: Token,
        start: Box<Start>,
    },

    /// The hub has taken this turn and now owns its retention.
    AcknowledgeAgentTurn {
        agent: Token,
        turn: u32,
    },
    Deliver {
        owner: Token,
        workspace: Token,
        title: Box<[u8]>,
        body: Box<[u8]>,
    },
    /// Cancel the local delivery and wait of a relay. Exactly one terminal
    /// follows: `RelayCancelled`, or `Relayed` if its answer won. Remote
    /// effects are not rolled back.
    CancelRelay {
        run: Token,
        attempt: Token,
        call: Box<[u8]>,
    },
    /// To the engine: an inbound event for the run `run`'s attempt `attempt`
    /// was not passed on, for `reason`.
    /// Prepare `workspace` for the hosted run `owner`.
    Prepare {
        owner: Token,
        workspace: Workspace,
    },
    /// Abandon the prepare in flight for `owner`, whose run is cancelled. Its
    /// terminal still comes: `Unprepared`, or `Prepared` if the prepare won
    /// the race, and then the workspace is released.
    Abort {
        owner: Token,
    },
    /// Stop the agent `agent`: cancel its run, then end what is left of it
    /// past the grace. Its start's `Gone` comes once it has all gone. Sent
    /// whenever its run leaves live, also once the run has said how it
    /// finishes or the agent was faulted, when it changes nothing: the agent
    /// child domain winds the agent down then anyway.
    Stop {
        agent: Token,
    },
    Grant {
        agent: Token,
        grant: Grant,
    },
    /// Ask the workspace to save unfinished work.
    Save {
        owner: Token,
        workspace: Token,
    },
    /// The run is done with `workspace`: it goes back to the cache.
    Release {
        workspace: Token,
    },
}

/// The host's agent capability, translated by its root (domain/hosts.md,
/// section 6). These requests carry no process or channel vocabulary.
#[derive(PartialEq, Eq, Debug)]
pub enum ToAgent {
    /// Pass a named message with its sender label and words.
    Message {
        agent: Token,
        name: Token,
        sender: Box<[u8]>,
        words: Box<[u8]>,
        finish_requested: bool,
    },
    /// Answer a call under the agent's opaque name.
    Answer {
        agent: Token,
        call: Box<[u8]>,
        reply: Reply,
    },
    /// Start an agent with its activation and committed conversation state.
    Start {
        owner: Token,
        start: Box<Start>,
    },
    /// The hub holds this turn independently of the engine link.
    Acknowledge {
        agent: Token,
        turn: u32,
    },
    Grant {
        agent: Token,
        grant: Grant,
    },
    Cancel {
        agent: Token,
    },
}

/// What the agent capability reports back to the host. Its root translates
/// the application agent's own vocabulary into these events.
#[derive(PartialEq, Eq, Debug)]
pub enum FromAgent {
    /// A named host or delivery call with arguments.
    Called {
        owner: Token,
        call: Box<[u8]>,
        ask: Ask,
    },
    /// The agent withdrew its named call.
    Withdrawn {
        owner: Token,
        call: Box<[u8]>,
    },
    Turn {
        owner: Token,
        turn: Turn,
    },
    /// A piece of live content (hosts.md, 6.5).
    Piece {
        owner: Token,
        piece: crate::Piece,
    },
    /// Typed agent activity, kept best effort for the engine.
    Activity {
        owner: Token,
        activity: crate::Activity,
    },
    /// The provider rejected the account's credential of `generation`.
    Unauthorized {
        owner: Token,
        account: u32,
        generation: u64,
    },
    /// The account reached its provider limit; `reset`, when the provider
    /// gave one, is how long until it lifts.
    Limited {
        owner: Token,
        account: u32,
        reset: Option<Duration>,
    },
    /// The provider refused the account, with its code and reason.
    AccountRefused {
        owner: Token,
        account: u32,
        code: Box<[u8]>,
        reason: Box<[u8]>,
    },
    Finished {
        owner: Token,
        turns: u32,
        spent: u64,
        read: Option<Token>,
        finish: Finish,
    },
    Started {
        owner: Token,
        agent: Token,
    },
    /// The agent refused its start, before any run began.
    StartRefused {
        cause: StartCause,
        owner: Token,
        why: Box<[u8]>,
    },
    Refused {
        owner: Token,
        name: Token,
        reason: MessageRefusal,
    },
    Yielded {
        owner: Token,
    },
    Faulted {
        owner: Token,
        fault: AgentFailure,
    },
    Gone {
        owner: Token,
        detail: Box<[u8]>,
    },
}

impl Event {
    /// Bring an agent capability's report into the host's event stream.
    #[must_use]
    pub fn from_agent(event: FromAgent) -> Event {
        match event {
            FromAgent::Called { owner, call, ask } => Event::Called { owner, call, ask },
            FromAgent::Withdrawn { owner, call } => Event::Withdrawn { owner, call },
            FromAgent::Turn { owner, turn } => Event::Turn { owner, turn },
            FromAgent::Piece { owner, piece } => Event::Piece { owner, piece },
            FromAgent::Activity { owner, activity } => Event::Activity { owner, activity },
            FromAgent::Unauthorized { owner, account, generation } => {
                Event::Unauthorized { owner, account, generation }
            }
            FromAgent::Limited { owner, account, reset } => Event::Limited { owner, account, reset },
            FromAgent::AccountRefused { owner, account, code, reason } => {
                Event::AccountRefused { owner, account, code, reason }
            }
            FromAgent::Finished { owner, turns, spent, read, finish } => {
                Event::Finished { owner, turns, spent, read, finish }
            }
            FromAgent::Started { owner, agent } => Event::Started { owner, agent },
            FromAgent::StartRefused { cause, owner, why } => Event::StartRefused { cause, owner, why },
            FromAgent::Refused { owner, name, reason } => Event::Refused { owner, name, reason },
            FromAgent::Yielded { owner } => Event::Yielded { owner },
            FromAgent::Faulted { owner, fault } => Event::Faulted { owner, fault },
            FromAgent::Gone { owner, detail } => Event::Gone { owner, detail },
        }
    }
}

impl Request {
    /// Take an agent request for translation by the root, returning
    /// any request addressed to another capability unchanged.
    #[expect(clippy::result_large_err, reason = "the root receives an owned link record with its refusal metadata")]
    pub fn to_agent(self) -> Result<ToAgent, Request> {
        match self {
            Request::Message { agent, name, sender, words, finish_requested } => {
                Ok(ToAgent::Message { agent, name, sender, words, finish_requested })
            }
            Request::Reply { agent, call, reply } => Ok(ToAgent::Answer { agent, call, reply }),
            Request::Start { owner, start } => Ok(ToAgent::Start { owner, start }),

            Request::Grant { agent, grant } => Ok(ToAgent::Grant { agent, grant }),
            Request::AcknowledgeAgentTurn { agent, turn } => Ok(ToAgent::Acknowledge { agent, turn }),
            Request::Stop { agent } => Ok(ToAgent::Cancel { agent }),
            other @ (Request::Source { .. }
            | Request::SourceFault
            | Request::Up(..)
            | Request::Deliver { .. }
            | Request::CancelRelay { .. }
            | Request::Hello { .. }
            | Request::Prepare { .. }
            | Request::Abort { .. }
            | Request::Save { .. }
            | Request::Release { .. }) => Err(other),
        }
    }
}

/// The agent's complete start (hosts.md, sections 2 and 9).
#[derive(PartialEq, Eq, Debug)]
pub struct Start {
    pub live: crate::LiveAsk,
    pub workspace: Option<Token>,
    pub charter: Box<[u8]>,
    pub activation: u64,
    pub identity: [u8; 16],
    pub turns: Box<[Box<[u8]>]>,
    pub answered: Box<[AnsweredCall]>,
    pub messages: Box<[StartMessage]>,
    pub grants: Box<[Grant]>,
}

/// A start's inbox message, oldest first (hosts.md, section 9).
#[derive(PartialEq, Eq, Hash, Debug)]
pub struct StartMessage {
    pub name: Token,
    pub sender: Box<[u8]>,
    pub words: Box<[u8]>,
    pub finish_requested: bool,
}

/// The form in which a grant lends its value (protocol/channel.md, section 8).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Form {
    /// A credential with the scheme its provider takes.
    Credential(Scheme),
    /// An application's credential, held in its own table.
    Application,
    /// A program source lent by its configuration-directory name.
    Program(crate::SourceName),
}

/// A credential's scheme (protocol/channel.md, section 8).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Scheme {
    /// A bearer credential.
    Bearer,
    /// A key credential.
    Key,
}

impl Form {
    /// The protocol table that holds this form's value.
    #[must_use]
    pub const fn table(self) -> GrantTable {
        match self {
            Self::Credential(_) => GrantTable::Accounts,
            Self::Application => GrantTable::Application,
            Self::Program(_) => GrantTable::Name,
        }
    }
}

/// The protocol layer's table that holds a credential (channel.md, section 8).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum GrantTable {
    Accounts,
    Application,
    /// The value is the form's name; no table entry is held.
    Name,
}

/// What the core gives the host for one run (domain/hosts.md, section 2).
#[derive(PartialEq, Eq, Hash, Debug)]
pub struct RunAssignment {
    pub identity: [u8; 16],
    /// The engine's names for the run and for this attempt at it.
    pub run: Token,
    pub attempt: Token,
    /// Nothing when the run has no workspace items to prepare.
    pub workspace: Option<Workspace>,
    /// Whether the workspace must save unfinished work before release.
    pub save: bool,
    pub live: crate::LiveAsk,
    /// What the agent's run is given, passed through.
    pub charter: Box<[u8]>,
    pub grants: Box<[Grant]>,
}

/// An assignment with the conversation state the agent consumes (hosts.md, 2).
#[derive(PartialEq, Eq, Hash, Debug)]
pub struct Assignment {
    pub messages: Box<[StartMessage]>,
    pub assignment: RunAssignment,
    /// Ordered committed turn bodies. An empty list starts a fresh conversation.
    pub turns: Box<[Box<[u8]>]>,
    /// Calls settled after the last committed turn, in commit order.
    pub answered: Box<[AnsweredCall]>,
}

/// A call committed after the last turn, to tell the next activation.
#[derive(PartialEq, Eq, Hash, Debug)]
pub struct AnsweredCall {
    /// The run's opaque call name.
    pub name: Box<[u8]>,
    /// The host tool's name as declared by the charter.
    pub tool: Box<[u8]>,
    pub answer: SettledAnswer,
}

/// The committed answer to a host call or workspace delivery.
#[derive(PartialEq, Eq, Hash, Debug)]
pub enum SettledAnswer {
    /// A host tool's answer, including whether it is an error.
    Host { error: bool, body: Box<[u8]> },
    /// A workspace delivery's outcome and opaque, versioned evidence for replay.
    Delivery { outcome: DeliveryOutcome, evidence: Box<[u8]> },
}

/// A turn's cumulative spend and last read message name are opaque accounting.
#[derive(PartialEq, Eq, Hash, Debug)]
pub struct Turn {
    pub opens_window: bool,
    pub refused: Box<[Refused]>,
    pub turn: u32,
    pub spent: u64,
    pub read: Option<Token>,
    pub body: Box<[u8]>,
}

#[derive(PartialEq, Eq, Hash, Debug)]
pub enum Finish {
    Ended { outcome: Box<[u8]> },
    Parked,
    Failed { failure: RunFailure },
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Answer {
    pub read: Option<Token>,
    pub refused: Box<[Refused]>,
    pub turns: u32,
    pub spent: u64,
    pub ending: Ending,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Ending {
    Refused(Refusal),
    Ended { outcome: Box<[u8]>, work: Work },
    Parked { work: Work },
    Failed { failure: Failure, detail: Box<[u8]>, work: Work },
}

/// The application's workspace items, held by its root.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Workspace {
    /// The task whose runs may reuse the workspace.
    pub workstream: u64,
    /// The root's token for the application's bounded workspace items.
    pub items: Token,
}

/// A host call of a run.
#[derive(PartialEq, Eq, Hash, Debug)]
pub enum Ask {
    /// A host tool call, carrying its charter name, write flag, input and time left.
    Relay {
        tool: Box<[u8]>,
        writes: bool,
        input: Box<[u8]>,
        deadline: Duration,
    },
    Deliver {
        title: Box<[u8]>,
        body: Box<[u8]>,
    },
}

/// The answer to a host call.
#[derive(PartialEq, Eq, Hash, Debug)]
pub enum Reply {
    /// The engine's answer to a relayed call, as it is.
    Relayed { error: bool, text: Box<[u8]> },
    /// How the delivery went.
    Delivered(Delivery),
    /// The run is cancelled or ending: nothing was done. A delivery in flight as
    /// the run leaves live is waited for, and answered with how it went.
    Unavailable,
    /// The run withdrew the relayed call: nothing more is done for it, and
    /// the engine's answer, if one comes, is dropped.
    Withdrawn,
    /// The run has as many calls in flight as it may, or a delivery in flight
    /// already: nothing was done. Calls answered within the loop's current
    /// iteration keep their slots until its reclaim point, so a busy call may
    /// find room in the next.
    Busy,
}

/// A workspace's delivery outcome and the root's token for what the run left.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Delivery {
    pub outcome: DeliveryOutcome,
    pub left: Token,
    /// Whether the workspace changed even if the delivery reported a failure.
    pub changed: bool,
}

/// The result of making a delivery through an application's workspace.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum DeliveryOutcome {
    Delivered,
    Nothing,
    Stale,
    Refused,
    Failed,
}

/// Why an inbound event was not passed on.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum MessageRefusal {
    /// It holds more bytes than the limits allow.
    TooLarge,
    /// The run is not live yet, and holds as many events as it may.
    Full,
    NameInUse,
    /// The run is ending.
    Ending,
}

/// A hosted run, for the reconnect report.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Hosting {
    pub run: Token,
    pub attempt: Token,
    pub phase: Phase,
}

/// Where a hosted run is in its lifecycle (hosts.md, section 6).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Phase {
    Answered,
    Preparing,
    Starting,
    /// Its agent is at work.
    Active,
    /// It yielded, and waits for its next inbound event.
    Waiting,
    /// How it ends is decided: it is stopping, saving, or waiting for what is
    /// in flight to settle, and answers next.
    Ending,
}

/// The root's tokens for what a run delivered and saved on its workspace.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Work {
    pub left: Option<Token>,
    pub saved: Option<Token>,
}

/// Why an assignment was refused.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Refusal {
    /// Every slot is taken (by a run hosted, or one whose answer the engine
    /// has yet to acknowledge), the run is hosted already under another
    /// attempt that has not answered yet, or the host is shutting down. A
    /// later retry, on this host or another, may find room.
    Busy,
    /// The assignment does not fit the limits.
    Invalid(Invalid),
}

/// What about an assignment does not fit the limits.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Invalid {
    /// The agent refused the start; its words are for operators.
    Start {
        cause: StartCause,
        why: Box<[u8]>,
    },
    /// The charter holds more bytes than a run may.
    Charter,
    Transcript,
    Grants,
    Messages,
}

/// Why a hosted run failed (hosts.md, section 6): what the engine acts on.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Failure {
    /// Its workspace could not be prepared.
    Unprepared(Preparation),
    /// The run failed, as it reports it.
    Run(RunFailure),
    /// Its agent failed.
    Agent(AgentFailure),
    /// It was cancelled.
    Cancelled(Reason),
}

/// Why a workspace could not be prepared.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Preparation {
    /// A later attempt may work.
    Transient,
    /// A resource needs a change before another attempt can work.
    Permanent { resource: Option<Token> },
}

/// Why a run failed, as it reports it (hosts.md, section 6).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum RunFailure {
    /// A configured bound refused the run's model call (domain/hosts.md, 6.3).
    Exceeded(Exceeded),
    /// The LLM could not do the work.
    Model,
    /// The run's budget ran out.
    Budget,
    /// The LLM did not keep to the run's rules.
    Policy,
    /// The run was cancelled.
    Cancelled,
    /// The run's target changed since it started.
    Stale,
    /// The run's account reached its provider limit; `reset`, when the
    /// provider gave one, is how long until it lifts.
    Limited { account: u32, reset: Option<Duration> },
    /// The provider refused the run's account.
    Refused { account: u32 },
}

/// How an agent failed.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum AgentFailure {
    /// It could not be started.
    Unstarted,
    /// It exited without saying how its run finishes.
    Exited,
    /// It broke the channel's rules, or said more than the limits allow.
    Rules,
    /// The watchdog stopped it: no progress.
    NoProgress,
    /// The watchdog stopped it: past its wall time.
    WallTime,
}

/// Why a run was cancelled.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Reason {
    /// The engine cancelled it.
    Engine,
    /// A worker root lost contact with the core for longer than the grace.
    Contact,
    /// The root is shutting down.
    Shutdown,
}

/// Names of credential grants; their values belong to the protocol layer.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Grant {
    pub form: Form,
    pub account: u32,
    pub generation: u64,
    pub valid: Duration,
}

/// A message refusal carried by its next turn or answer (hosts.md, 6.4).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Refused {
    pub name: Token,
    pub reason: MessageRefusal,
}

#[cfg(test)]
mod tests {
    use super::{Form, GrantTable, Scheme};

    #[test]
    fn a_form_names_its_table() {
        for (form, table) in [
            (Form::Credential(Scheme::Bearer), GrantTable::Accounts),
            (Form::Credential(Scheme::Key), GrantTable::Accounts),
            (Form::Application, GrantTable::Application),
            (Form::Program(crate::SourceName::new(b"claude").expect("source name")), GrantTable::Name),
        ] {
            assert_eq!(form.table(), table);
        }
    }

    #[test]
    fn a_form_takes_34_bytes() {
        // tag 1 + length 1 + 32 name bytes.
        assert_eq!(size_of::<Form>(), 34);
    }
}

/// Why the agent refused a start, independent of its diagnostic bytes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum StartCause {
    /// Rendered host charter and tools exceeded the receiving host's byte bound.
    HostPart {
        bound: u64,
    },
    Other,
    Grants,
}

/// A configured bound a run's model call met, and that bound's configured
/// value in its own unit: never a measured size.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Exceeded {
    pub bound: Bound,
    pub value: u64,
}

/// A configured bound on a model call.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Bound {
    /// The model's reasoning, per item or in all: bytes.
    Reasoning,
    /// The model's other replay metadata: bytes.
    Metadata,
    /// The items of the model's answer: a count.
    Items,
    /// The model's answer: bytes.
    Answer,
    /// The tools offered to the model: a count.
    Tools,
    /// The messages and blocks one request carries: a count of both.
    HistoryItems,
    /// The run's retained history, which compaction could not bring under
    /// one of its configured ceilings: on messages, on blocks or on bytes.
    /// The value is that ceiling, and the bound doesn't name its unit.
    History,
    /// The model's window: tokens.
    Window,
    /// The model's declared output ceiling: tokens.
    Output,
    /// Rendered host charter and tool schemas: bytes.
    HostPart,
}
