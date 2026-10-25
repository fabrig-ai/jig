//! The records that cross the boundary with the fleet's parent, the engine's
//! root domain (programming-model.md, 4.5). The fleet defines them; its
//! parent depends on it.
//!
//! The fleet has two faces, both through its parent:
//!
//! - The workers', which the parent routes to and from their channels
//!   (domain/hosts.md, sections 2 and 4). A worker's channel is named by the
//!   protocol's token for it, `channel`, and is in contact from its
//!   [`Event::Hello`], the first thing said on it, until its
//!   [`Event::Lost`]. [`Request::Assign`] is a call to the worker, answered
//!   by one [`Event::Answer`] under the run's and the attempt's names. The
//!   worker keeps the answer, and the slot it takes, until the fleet sends
//!   [`Request::Acknowledge`], which it does once the parent has made the
//!   answer durable, or once the answer is for an attempt fenced off; it
//!   sends it again after every hello until then. A refusal goes once and
//!   keeps nothing. [`Request::Inbound`], [`Request::Cancel`] and
//!   [`Request::Relayed`] are notices to the worker hosting an attempt;
//!   [`Event::Relay`] is a call of the run's, which the fleet passes up and
//!   whose answer it passes down, at most once. [`Request::Refuse`] turns a worker away
//!   at its hello, when the fleet has no room for another: its channel is
//!   to be closed. An answer from a channel not in contact is dropped, and
//!   not acknowledged. Everything a worker sends names a run and an attempt,
//!   and is dropped unless that attempt is the parent's live claim (attempts
//!   are fenced): only its answer is still taken once it is cancelled.
//! - The parent's own. [`Event::Start`] and [`Event::Adopt`] are calls,
//!   each ended by exactly one of [`Request::Answered`], [`Request::Lost`],
//!   [`Request::Withdrawn`] or [`Request::Refused`]; a start is told
//!   [`Request::Placed`] before, each time it is assigned, and an adoption
//!   once a worker is found to host it. An attempt a worker refuses as busy
//!   is placed again, not ended. An answer handed to the parent is answered
//!   by exactly one [`Event::Acknowledge`], once the parent has made it
//!   durable or no longer wants it: until then its worker keeps it. After a
//!   restart, the parent adopts the claims its records hold and then says
//!   [`Event::Loaded`]; an attempt a worker lists that no claim adopts is
//!   told as [`Request::Listed`], and waits to be adopted for the grace from
//!   then. [`Event::Cancel`] and [`Event::Inbound`] name an attempt, and an
//!   inbound event that does not reach a worker comes back as
//!   [`Request::Undelivered`]. A [`Request::Relay`] is a call the parent
//!   answers with exactly one [`Event::Relayed`].
//!
//! What the fleet passes through and never reads is the parent's. An
//! assignment (its charter, workspace and transcript) is named by its run and
//! attempt, and the parent attaches it to the [`Request::Assign`] that
//! names them. An inbound event, a relayed call and its answer, a run's
//! answer, turns and facts are each named by a token the parent issues, and
//! echoed exactly once: in the request that passes it on, or in a
//! [`Request::Drop`] that tells the parent to forget it. Tokens of different
//! families may be equal.

use alloc::boxed::Box;

use skein_lib::{Duration, ReplyTo, Token};

/// A kind of host that can run an attempt.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum HostKind {
    /// A worker connected on a channel.
    Worker,
    /// The engine's own host.
    Engine,
}

/// The kinds a run's charter permits.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Kinds {
    /// Only workers may host the run.
    Workers,
    /// Only the engine may host the run.
    Engine,
    /// Either kind may host the run.
    Both,
}

impl Kinds {
    /// Whether a host kind is allowed by the charter.
    #[must_use]
    pub const fn allows(self, kind: HostKind) -> bool {
        match self {
            Kinds::Workers => match kind {
                HostKind::Worker => true,
                HostKind::Engine => false,
            },
            Kinds::Engine => match kind {
                HostKind::Worker => false,
                HostKind::Engine => true,
            },
            Kinds::Both => true,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Grant {
    pub form: Form,
    pub account: u32,
    pub generation: u64,
    pub valid: Duration,
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

/// A credential's name, independent of its current generation.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct GrantName {
    pub table: GrantTable,
    pub account: u32,
}

/// References to the parent's ordered turn bodies and settled call tail.
/// The fleet retains these tokens while placement waits and reads neither value.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Assignment {
    /// The parent's ordered turn-body collection.
    pub turns: Token,
    /// The parent's calls settled since its last turn, including opaque delivery evidence.
    pub answered: Token,
}

/// A message's name and the parent's label and words, passed through unchanged.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Message {
    pub name: Token,
    pub sender: Token,
    pub words: Token,
}

/// A named host call (hosts.md, section 2). Its name and write flag are kept while
/// the parent decides it; its other fields are moved into the parent's request.
#[derive(PartialEq, Eq, Hash, Debug)]
pub struct Call {
    /// The run's opaque call name, bounded by the fleet's call-name limit.
    pub name: Box<[u8]>,
    /// The host tool's name as declared in the charter.
    pub tool: Box<[u8]>,
    pub writes: bool,
    pub input: Box<[u8]>,
    /// Time remaining at relay, independent of either host's clock.
    pub deadline: Duration,
}

/// parent -> fleet
#[derive(PartialEq, Eq, Debug)]
pub enum Event {
    /// Replace needs, retaining channels and their running attempts (engine.md, 17.2).
    Needs(crate::Needs),
    /// Desired absence for issued names; names remain in Sources for reconnect checks.
    CleanupPrograms {
        names: Box<[crate::SourceName]>,
    },
    CheckProgram {
        name: crate::SourceName,
    },
    SignOutProgram {
        source: u32,
        name: crate::SourceName,
    },
    Sources {
        sources: Box<[crate::SourceName]>,
    },
    Check {
        channel: Token,
        name: crate::SourceName,
    },
    SignOut {
        to: ReplyTo,
        channel: Token,
        name: crate::SourceName,
    },
    Logins {
        channel: Token,
        logins: Box<[crate::Login]>,
    },
    SignedOut {
        channel: Token,
        name: crate::SourceName,
        result: crate::SignOutResult,
    },
    /// Place a run on a host declaring every model of its configured charter,
    /// with references to its ordered committed conversation state.
    Start {
        reply_to: ReplyTo,
        run: Token,
        attempt: Token,
        workstream: u64,
        kinds: Kinds,
        charter: u32,
        placement: Option<PlacementConstraint>,
        assignment: Assignment,
        grants: Box<[Grant]>,
    },
    /// Pass a named message and its label and words to the live host.
    Inbound {
        run: Token,
        attempt: Token,
        message: Message,
    },
    /// Pass a named host call with its declared tool and input to the parent.
    Relay {
        channel: Token,
        run: Token,
        attempt: Token,
        call: Call,
    },
    /// From the parent, a claim restored after restart. A worker may report
    /// it within the grace; an engine-hosted claim is settled at once.
    Adopt {
        reply_to: ReplyTo,
        run: Token,
        attempt: Token,
        /// The committed prefix restored with the claim; zero before its first turn.
        kept: u32,
        /// Kind of host that held the claim before restart.
        kind: HostKind,
        /// Whether a turn or call was committed for this attempt.
        worked: bool,
        holds: Box<[GrantName]>,
        grants: Box<[Grant]>,
    },
    /// From the parent: cancel the attempt `attempt` of the run `run`. Its
    /// call is still ended by its answer, unless it was never placed.
    Cancel {
        run: Token,
        attempt: Token,
    },
    /// From the parent, the one answer to a `Relay`.
    Relayed {
        to: ReplyTo,
        answer: Token,
    },
    /// From the parent, the one answer to an `Answered`: the answer of the
    /// attempt `attempt` of the run `run` is durable, or not wanted, and its
    /// worker may forget it.
    Acknowledge {
        run: Token,
        attempt: Token,
    },
    /// From the parent: its cold read after a restart is done, every claim
    /// its records hold adopted. Attempts workers list that no claim adopts
    /// wait to be adopted for the grace from now on, and those listed later
    /// for the grace from their listing.
    Loaded,
    Grant {
        grant: Grant,
    },
    /// From a worker: the provider rejected the account's credential of
    /// `generation`.
    Unauthorized {
        channel: Token,
        run: Token,
        attempt: Token,
        account: u32,
        generation: u64,
    },
    /// From a worker: the account reached its provider limit; `reset`, when
    /// the provider gave one, is how long until it lifts.
    Limited {
        channel: Token,
        run: Token,
        attempt: Token,
        account: u32,
        reset: Option<Duration>,
    },
    /// From a worker: the provider refused the account, with its code and
    /// reason.
    AccountRefused {
        channel: Token,
        run: Token,
        attempt: Token,
        account: u32,
        code: Box<[u8]>,
        reason: Box<[u8]>,
    },
    /// From a worker, first on its channel: its slots, the workstreams it
    /// holds workspaces for, and the runs it hosts or holds answers of.
    Hello {
        channel: Token,
        worker: u32,
        hello: Hello,
    },
    /// From the protocol: the channel of a worker closed.
    Lost {
        channel: Token,
    },
    /// From a worker, the answer to an `Assign`: sent at once, and again
    /// after every hello until it is acknowledged. Taken only from a channel
    /// in contact.
    Answer {
        channel: Token,
        run: Token,
        attempt: Token,
        answer: Answer,
        payload: Token,
    },
    /// From the current worker: an opaque turn numbered from one. Its body
    /// is echoed exactly once by `Turned` or `Drop`; a duplicate before
    /// commitment is dropped unacknowledged. Strays' turns wait for adoption.
    Turn {
        channel: Token,
        run: Token,
        attempt: Token,
        turn: u32,
        body: Token,
    },
    /// The parent committed the prefix through `turn`. Commitments arrive in
    /// order; the worker may forget this turn. Adoption restores the prefix.
    TurnKept {
        run: Token,
        attempt: Token,
        turn: u32,
    },
    /// The parent could not commit an admitted turn. Release its admission
    /// and tell its current worker to retry it after a backoff.
    TurnBusy {
        run: Token,
        attempt: Token,
        turn: u32,
    },
    /// From a worker: an inbound event for the attempt `attempt` of the run
    /// `run` was not passed on, for `bounce`.
    /// Best-effort typed activity from the current hosting channel.
    Observe {
        channel: Token,
        run: Token,
        attempt: Token,
        report: crate::Report,
    },
}

/// fleet -> parent
#[derive(PartialEq, Eq, Debug)]
pub enum Request {
    ProgramTargets {
        source: u32,
        workers: Box<[crate::Worker]>,
    },
    ProgramSignedOut {
        source: u32,
        worker: crate::Worker,
        result: crate::ProgramResult,
    },
    Sources {
        channel: Token,
        sources: Box<[crate::SourceName]>,
    },
    Check {
        channel: Token,
        name: crate::SourceName,
    },
    SignOut {
        channel: Token,
        name: crate::SourceName,
    },
    /// None means the host did not confirm sign-out, including channel loss.
    SignedOut {
        to: ReplyTo,
        channel: Token,
        name: crate::SourceName,
        result: Option<crate::SignOutResult>,
    },
    /// An owned live observation changed; this grants no reply right.
    Observed {
        run: Token,
        attempt: Token,
    },
    /// Place the attempt with its conversation state.
    Assign {
        channel: Token,
        kind: HostKind,
        run: Token,
        attempt: Token,
        /// The attempt number used as the agent's activation.
        activation: u64,
        assignment: Assignment,
    },
    /// Pass a named message with its label and words to the current host.
    Inbound {
        channel: Token,
        run: Token,
        attempt: Token,
        message: Message,
    },
    /// Pass a host call to the parent for an answer.
    Relay {
        reply_to: ReplyTo,
        run: Token,
        attempt: Token,
        call: Call,
    },
    /// Answer a call under the name its agent gave it.
    Relayed {
        channel: Token,
        run: Token,
        attempt: Token,
        call: Box<[u8]>,
        answer: Token,
        /// Whether the original call changes state and its answer must wait for commitment.
        writes: bool,
    },
    /// The current host's bounded call found no live call capacity; retry at its original address.
    RelayBusy {
        channel: Token,
        run: Token,
        attempt: Token,
        call: Call,
    },
    /// Return a fenced call or one whose name exceeds the bound; the parent owns its bytes.
    DropCall {
        call: Call,
    },
    Grant {
        channel: Token,
        run: Token,
        attempt: Token,
        grant: Grant,
    },
    /// To the parent: a worker's notice that the provider rejected the
    /// account's credential of `generation`.
    Unauthorized {
        run: Token,
        attempt: Token,
        account: u32,
        generation: u64,
    },
    /// To the parent: a worker's notice that the account reached its
    /// provider limit, `reset` from now when the provider gave one.
    Limited {
        run: Token,
        attempt: Token,
        account: u32,
        reset: Option<Duration>,
    },
    /// To the parent: a worker's notice that the provider refused the
    /// account.
    AccountRefused {
        run: Token,
        attempt: Token,
        account: u32,
        code: Box<[u8]>,
        reason: Box<[u8]>,
    },
    /// To a worker: cancel the attempt `attempt` of the run `run`. Sent
    /// again on every channel whose hello lists it unanswered, the first
    /// having maybe been lost.
    Cancel {
        channel: Token,
        run: Token,
        attempt: Token,
    },
    /// To a worker: the engine has the answer of the attempt `attempt` of the
    /// run `run` durably, or does not want it, and the worker forgets it.
    Acknowledge {
        channel: Token,
        run: Token,
        attempt: Token,
    },
    /// To the parent: this turn's body, once per admission, for commitment.
    /// The parent owns the body after this request and answers with `TurnKept`
    /// or `TurnBusy`. An engine restart may hand an uncommitted turn on again.
    Turned {
        run: Token,
        attempt: Token,
        turn: u32,
        body: Token,
    },
    /// To the current worker: this turn is committed or fenced off.
    AcknowledgeTurn {
        channel: Token,
        run: Token,
        attempt: Token,
        turn: u32,
    },
    /// To the current worker: no admission or commitment room; keep this turn
    /// and retry after a backoff. A received body also gets `Drop`.
    TurnBusy {
        channel: Token,
        run: Token,
        attempt: Token,
        turn: u32,
    },
    /// About a worker: the fleet has no room for it, or its hello is beyond
    /// the limits. Close its channel; it dials again later.
    Refuse {
        channel: Token,
    },
    /// To the parent: the attempt is on a worker, assigned or found there.
    Placed {
        run: Token,
        attempt: Token,
    },
    /// To the parent: a worker lists the attempt `attempt` of the run `run`,
    /// which no claim adopts. It waits to be adopted for the grace, from the
    /// parent's `Loaded` at the earliest, and is then cancelled.
    Listed {
        run: Token,
        attempt: Token,
    },
    /// To the parent, terminal for `Start` and `Adopt`: the worker's answer,
    /// never `Busy`. Each gets the parent's `Acknowledge`, until which its
    /// worker keeps it (a refusal keeps nothing, and the fleet forgets it at
    /// once).
    Answered {
        to: ReplyTo,
        run: Token,
        attempt: Token,
        answer: Answer,
        payload: Token,
    },
    /// To the parent, terminal for an engine-hosted adoption with no committed work.
    /// It consumes no try of the task.
    NotStarted {
        to: ReplyTo,
        run: Token,
        attempt: Token,
    },
    /// To the parent, terminal for `Start` and `Adopt`: the attempt is
    /// presumed lost, its worker out of contact past the grace or none
    /// having said it hosts it. Whatever still comes for it is dropped.
    Lost {
        to: ReplyTo,
        run: Token,
        attempt: Token,
    },
    /// To the parent, terminal for `Start` and `Adopt`: the attempt was
    /// withdrawn, for `withdrawal`.
    Withdrawn {
        to: ReplyTo,
        run: Token,
        attempt: Token,
        withdrawal: Withdrawal,
    },
    /// To the parent, terminal for `Start` and `Adopt`: refused at the
    /// entrance, nothing done.
    Refused {
        to: ReplyTo,
        run: Token,
        attempt: Token,
        refusal: Refusal,
    },
    /// To the parent: an inbound event its worker did not pass on, for
    /// `bounce`.
    /// A message reached no live host; all its values remain the parent's.
    Undelivered {
        run: Token,
        attempt: Token,
        message: Message,
        undelivered: Undelivered,
    },
    /// To the parent: forget `payload`, which goes no further: it was for an
    /// attempt fenced off or gone, a duplicate, or a call the fleet had no
    /// room for.
    Drop {
        payload: Token,
    },
}

/// What a worker says first on every channel (domain/engine.md, section 8).
#[derive(PartialEq, Eq, Hash, Debug)]
pub struct Hello {
    pub bounds: crate::Bounds,
    /// The worker's longest stop time after losing contact. It must be
    /// strictly below the engine's grace.
    pub stop_bound: Duration,
    /// How many runs it hosts at once: none once it is shutting down.
    pub slots: u32,
    pub workstreams: Box<[u64]>,
    /// The runs it hosts, and those whose answers it holds.
    pub hosting: Box<[Hosted]>,
}

/// A run a worker hosts, or whose answer it holds.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Hosted {
    pub run: Token,
    pub attempt: Token,
    pub phase: Phase,
}

/// Where a hosted run is in its lifecycle (domain/hosts.md, 4.2), as a hello
/// says.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Phase {
    Preparing,
    Starting,
    /// Its agent is at work.
    Active,
    /// It yielded, and waits for its next inbound event.
    Waiting,
    /// How it ends is decided, and it answers next.
    Ending,
    /// It answered while there was no channel: the answer follows the hello.
    Answered,
}

/// How a run's attempt was answered (domain/hosts.md, 4.3): what the fleet
/// acts on. The rest is the payload's.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Answer {
    /// It ended with its outcome.
    Ended,
    /// It parked.
    Parked,
    /// It failed, `turned` naming the account the run failed on, when the
    /// provider limited or refused it.
    Failed { turned: Option<Turned> },
    /// Refused at the entrance, every slot taken or the worker shutting
    /// down: the fleet places the attempt again, and nothing more on that
    /// worker until it frees a slot. Never handed to the parent.
    Busy,
    /// Refused at the entrance: the assignment does not fit the worker's
    /// limits.
    Invalid,
}

/// The account a run failed on, when the provider limited or refused it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Turned {
    /// The account reached its provider limit; `reset`, when the provider
    /// gave one, is how long until it lifts.
    Limited { account: u32, reset: Option<Duration> },
    /// The provider refused the account.
    Refused { account: u32 },
}

/// Why an inbound event reached no worker.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Undelivered {
    /// The attempt waits to be placed.
    Unplaced,
    /// Its worker is out of contact, or not found yet.
    Adrift,
    /// The attempt is not the parent's live claim: cancelled, withdrawn,
    /// lost, answered, or never made.
    Gone,
}

/// Why an attempt was withdrawn.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Withdrawal {
    /// The parent cancelled it before it was placed.
    Cancelled,
    /// The parent started or adopted a newer attempt of its run. Placed, it
    /// is cancelled, and fenced.
    Replaced,
}

/// Why a start or an adoption was refused at the entrance.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Refusal {
    /// The checked host lost room; the parent must obtain a fresh run allowance.
    Recheck,
    /// The fleet tracks as many of the parent's attempts as it may.
    Busy,
    /// The workstream task number is zero.
    Workstream,
    /// The fleet knows the attempt already.
    Duplicate,
}

/// Read-only connected and retained lost host facts; no reply right is retained.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct HostView {
    pub worker: Option<u32>,
    pub channel: Token,
    pub kind: HostKind,
    pub slots: u32,
    pub used: u32,
    pub draining: bool,
    pub connected: bool,
    pub grace_until: Option<skein_lib::Time>,
}
/// The semantic attempt state, without its outstanding reply right.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum AttemptState {
    Waiting,
    Adopted,
    Claimed,
    Cancelled,
    Handed,
    Acknowledged,
    Stray,
    Kept,
    Fenced,
    Closed,
}
/// Current placement; monotonic timer facts are translated by the parent with
/// its injected clock when constructing a wall-time document.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Placement {
    None,
    Connected { channel: Token, kind: HostKind },
    Adrift { channel: Option<Token>, until: skein_lib::Time },
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct AttemptView {
    pub run: Token,
    pub attempt: Token,
    pub kept: u32,
    pub listed: bool,
    pub state: AttemptState,
    pub placement: Placement,
    pub until: Option<skein_lib::Time>,
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

/// A checked channel identity. A queued or busy attempt cannot move to another host.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PlacementConstraint {
    pub channel: Token,
    pub kind: HostKind,
}
