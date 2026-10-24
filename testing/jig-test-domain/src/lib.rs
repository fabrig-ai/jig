//! The testing application's root. It owns two numbered test connectors and
//! one commit door; the core owns every cross-child decision. The root only
//! translates connector values and wraps records (`domain/root.md`, 3–4).
//!
//! | State | Input | Next state | Output |
//! | --- | --- | --- | --- |
//! | open | admitted core or connector event | decision held | at most one commit |
//! | decision held | committed number | durable | held outputs may leave |
//! | open | failed number | stopped | stop, no later delivery |

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

use alloc::boxed::Box;
use jig_domain_core as core;
use jig_domain_fleet as fleet;
use jig_domain_tasks as tasks;
use jig_test_connector as connector;
use skein_lib::{
    Decision, Env, Journal, JournalLimits, JournalRoom, List, Map, Queue, ReplyTo, Set, Time, Token, Wall,
};

/// A store address owned by the core or one numbered connector.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Key {
    /// A core or child address.
    Core(core::Key),
    /// A connector-owned address.
    Connector { number: u16, key: connector::RecordKey },
}

/// A durable row owned by the core or one numbered connector.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Record {
    /// A core or child row.
    Core(core::Record),
    /// A connector-owned row.
    Connector { number: u16, record: connector::Record },
}

impl Record {
    /// The row's stable store address.
    #[must_use]
    pub fn key(&self) -> Key {
        match self {
            Self::Core(row) => Key::Core(row.key()),
            Self::Connector { number, record } => Key::Connector { number: *number, key: record.key() },
        }
    }
}

/// One mutation in an atomic decision.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Write {
    /// Copy the old settings revision into history atomically.
    CopySettingsRevision(u32),
    /// Replace the row under its key.
    Save(Record),
    /// Remove the row under its key.
    Erase(Key),
}

/// What the journal releases after its commit, or through its read door.
#[derive(Debug)]
pub enum Delivery {
    /// The root heard this project made after its commit.
    ProjectMade { project: u32 },
    /// A typed hub record for a worker channel.
    Link { channel: Token, down: jig_domain_host::LinkDown },
    /// The host retries this call after the engine drains.
    CallBusy { channel: Token, task: u64, attempt: u64, name: Box<[u8]> },
    /// A whole connector-owned document released after every reflected commit.
    ApplicationDocument { watcher: Token, number: u16, document: Box<connector::ClientDocument> },
    /// One connector-owned read terminal, held until durable.
    ApplicationRead {
        reply: ReplyTo,
        number: u16,
        answer: Result<connector::ClientDocument, connector::DocumentMissing>,
    },
    /// The connector's own terminal, released by the journal after durability.
    Application { to: ReplyTo, number: u16, outcome: connector::ApplicationOutcome },
    /// One committed whole-tree projection handoff.
    Projection { connector: u16, feed: Box<core::ProjectionFeed> },
    /// One committed inbox word for its current host.
    Message { channel: Token, task: u64, attempt: u64, word: tasks::Word },
    /// Perform this step of the core-owned restart script.
    Restart(core::RestartStep),
    /// Core-held party, host, task or view output.
    Core(core::Held),
    /// An answer with its original opaque call name and durability class.
    CallAnswer {
        channel: Token,
        task: u64,
        attempt: u64,
        name: Box<[u8]>,
        call: core::SettledCall,
        /// Writes wait for commitment; reads use the journal's door.
        writes: bool,
    },
    /// A prepared worker assignment, after the claim is durable.
    Assigned { channel: Token, assignment: Assignment },
    /// A post-commit continuation inside the core's fleet.
    Fleet(fleet::Event),
    /// Connector output to its fake system.
    System { connector: u16, call: connector::SystemRequest },
    /// A fresh read owed after this run's loss committed.
    LostRead { connector: u16, task: u64, attempt: u64, resource: tasks::Name, call: connector::SystemRequest },
    /// A procedure is due at its owning connector.
    Procedure { task: u64, step: u64, connector: u16, code: u16, context: Box<tasks::RunContext> },
}

/// The root's assembled, worker assignment.
#[derive(Debug)]
pub struct Assignment {
    pub live: core::LiveAsk,
    /// Ordered sections selected by the brief child, typed until protocol routing.
    pub brief: Box<[jig_domain_brief::GatherPlaced]>,
    /// The conversation's first attempt, shared by its resumes.
    pub conversation: u64,
    /// Claimed task and activation fence.
    pub task: u64,
    pub attempt: u64,
    /// Charter and task-specific policy and contract.
    pub charter: u32,
    pub run: Box<core::RunCharter>,
    /// Whole words and prior committed conversation offered to the worker.
    pub messages: Box<[tasks::Word]>,
    pub transcript: Box<[Box<[u8]>]>,
    pub answered: Box<[core::SettledCall]>,
    /// The selected account and application credential grants.
    pub grants: Box<[jig_domain_accounts::Grant]>,
}

/// What the root asks its surrounding world to do.
#[derive(Debug)]
pub enum Request {
    /// Read a fenced opaque terminal in the protocol routing.
    ReadResult { payload: Token, outcome: Box<[u8]> },
    /// Return a connector input to its owner during the drain.
    Busy { event: Box<Event> },
    /// First cold load, before the deployment counter is known.
    Restart(core::RestartStep),
    /// Apply one numbered transaction as a whole.
    Commit { number: u64, writes: Queue<Write> },
    /// Release one output only after its prerequisite commit.
    Deliver(Delivery),
    /// An immediate observation with no durability dependency.
    Now(core::Now),
    /// A failed store commit stopped this root.
    Stop,
}

/// A test input at the root boundary.
#[derive(Debug)]
#[expect(clippy::large_enum_variant, reason = "the testing root moves admitted core events by value")]
pub enum Event {
    /// A channel opening, record or loss in the hub vocabulary.
    Link {
        channel: Token,
        link: link::Event,
    },
    /// The protocol routing read a fenced result.
    ResultRead {
        payload: Token,
        end: tasks::End,
    },
    /// The service begins its drain.
    Stop,
    /// Prepare and watch the application's typed subject in the client's declared scope.
    WatchApplication {
        watcher: Token,
        sign_in: u64,
        scope: jig_domain_people::ApplicationScope,
        number: u16,
        subject: connector::ClientSubject,
        window: core::documents::Window,
    },
    /// Prepare and read one typed connector document without a key.
    ReadApplication {
        reply: ReplyTo,
        sign_in: u64,
        scope: jig_domain_people::ApplicationScope,
        number: u16,
        subject: connector::ClientSubject,
        window: core::documents::Window,
    },
    /// A protocol-decoded application value with its independently computed
    /// digest, and the number and digest of a value it carried, already kept
    /// in the secrets' store (`protocol/client.md`, section 5).
    Application {
        reply_to: ReplyTo,
        sign_in: u64,
        key: [u8; 16],
        scope: jig_domain_people::ApplicationScope,
        number: u16,
        digest: [u8; 32],
        value: connector::ApplicationRequest,
        carried: Option<core::Carried>,
    },
    /// A correlated reply to the connector's fresh read after a lost writer.
    WriterRead {
        connector: u16,
        resource: tasks::Name,
        event: connector::SystemEvent,
    },
    /// A numbered connector's independent retry deadline elapsed.
    ConnectorTimer {
        number: u16,
    },
    RestartBegin,
    RestartDone(core::RestartStep),
    /// Drive one due child timer through the same decision barrier.
    Timer(core::Timer),
    /// A party, host, account or other event in jig's vocabulary.
    Core(core::Event),
    /// A generation named by the application's credential source.
    Credential {
        number: u32,
        generation: u64,
        valid: skein_lib::Duration,
    },
    /// One numbered connector's input from its system or a scripted peer.
    Connector {
        number: u16,
        event: connector::Event,
    },
    /// A scripted connector's projection write for a tracked goal.
    ProjectionEffect {
        goal: u64,
        number: u16,
        effect: connector::Effect,
    },
    /// A decoded call for an offered topic.
    TopicCall {
        to: ReplyTo,
        key: core::CallKey,
        number: u16,
        topic: u16,
    },
    /// A decoded named effect call, whose payload stays with its connector.
    EffectCall {
        to: ReplyTo,
        key: core::CallKey,
        number: u16,
        effect: connector::Effect,
        deadline: Wall,
        proposal: Option<Box<[u8]>>,
    },
    /// The rendered answer to a read or an undecodable call, with no durable call record.
    CallAnswered {
        to: ReplyTo,
        call: core::SettledCall,
    },
    /// A scripted worker's provisional writing, named by its host.
    Text {
        task: u64,
        attempt: u64,
        turn: u32,
        offset: u32,
        text: Box<[u8]>,
    },
    /// A scripted worker's retained turn, including its opaque body.
    Turn {
        channel: Token,
        task: u64,
        attempt: u64,
        turn: u32,
        cumulative: u64,
        read: Option<u64>,
        opens_window: bool,
        refused: Box<[u64]>,
        transcript: Box<[u8]>,
    },
    /// A scripted worker's retained terminal answer.
    Answer {
        channel: Token,
        task: u64,
        attempt: u64,
        cumulative: u64,
        end: tasks::End,
        read: Option<u64>,
        refused: Box<[u64]>,
    },
    /// A bounded store page of prior opaque turns; pages arrive oldest first.
    TranscriptLoaded {
        task: u64,
        rows: Box<[core::TranscriptRow]>,
        done: bool,
    },
    /// Store acknowledgement of this exact commit number.
    Committed {
        number: u64,
        fill: u64,
    },
    /// A store failure of this exact commit number.
    Failed {
        number: u64,
    },
}

/// Finite room reserved before either child changes state.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// The core's bounded state and route buffers.
    pub core: core::Limits,
    /// Each test connector's bounded state and output.
    pub connector: connector::Limits,
    /// The application's journal capacities.
    pub journal: JournalLimits,
    /// At most this many synchronous child continuations per decision.
    pub routes: u32,
}

/// The testing application's two numbered connectors.
#[derive(Debug)]
pub struct Config {
    /// The core's configuration.
    pub core: core::Config,
    /// Connector one.
    pub first: connector::Config,
    /// Connector two.
    pub second: connector::Config,
}

/// The root's live components and one commit barrier.
#[derive(Debug)]
pub struct Domain {
    pieces_dropped: u64,
    core: core::Core,
    first: connector::Domain,
    second: connector::Domain,
    journal: Journal<Write, Delivery>,
    now: Queue<Request>,
    store_answers: Queue<Box<Event>>,
    links: Set<Token>,
    busy: Queue<Box<Event>>,
    assignments: Map<u64, Assignment>,
    briefs: Map<u64, Box<[jig_domain_brief::GatherPlaced]>>,
    workspace_writes: Map<u64, Box<[tasks::Name]>>,
    procedure_steps: Map<u64, (u64, u16)>,
    topic_calls: Map<Token, (ReplyTo, core::CallKey, u16)>,
    effects: Map<Token, connector::Effect>,
    effect_procedures: Map<Token, u64>,
    outbox_procedures: Map<u64, u16>,
    judges: Map<Token, (Token, jig_domain_authority::Judge, [u8; 32])>,
    outbox_tasks: Map<u64, u64>,
    payloads: Map<Token, Payload>,
    next_payload: u64,
    decision_wrote: bool,
    stopped_reported: bool,
    restart_active: bool,
    restart_load: Option<core::RestartStep>,
}

impl Domain {
    /// Borrow the core's settings; the testing root only translates this lookup.
    #[must_use]
    pub fn settings(&self) -> &core::settings::Current {
        &self.core.current
    }
    #[must_use]
    pub fn restore_failure(&self) -> Option<core::RestartRequest> {
        self.core.restore_failure()
    }
    /// Project each child's original elapsed deadline and the saved people
    /// wall expiry at the supplied clocks. The loop sends the owner's Timer.
    #[must_use]
    pub fn core_deadlines(&self, now: Time, wall: Wall) -> [(core::Timer, Option<Time>); 6] {
        let elapsed = self.core.people.next_deadline();
        let saved = match self.core.people.next_wall_expiry() {
            Some(expiry) if self.core.people.ready() => Some(
                now.saturating_add(skein_lib::Duration::from_nanos(expiry.as_nanos().saturating_sub(wall.as_nanos()))),
            ),
            Some(_) | None => None,
        };
        let people = match (elapsed, saved) {
            (Some(left), Some(right)) => Some(left.min(right)),
            (Some(at), None) | (None, Some(at)) => Some(at),
            (None, None) => None,
        };
        [
            (core::Timer::People, people),
            (core::Timer::Tasks, self.core.tasks.next_deadline()),
            (core::Timer::Fleet, self.core.fleet.next_deadline()),
            (core::Timer::Brief, self.core.brief.next_deadline()),
            (core::Timer::Account, self.core.accounts.next_deadline()),
            (core::Timer::View, self.core.views.next_deadline()),
        ]
    }

    /// Project the two connector owners' existing absolute deadlines unchanged.
    #[must_use]
    pub fn connector_deadlines(&self) -> [(u16, Option<Wall>); 2] {
        [(1, connector::next_deadline(&self.first)), (2, connector::next_deadline(&self.second))]
    }

    /// The core's retained effect reply deadline, including stale rights.
    #[must_use]
    pub fn effect_deadline(&self) -> Option<Wall> {
        self.core.effect_deadline()
    }

    /// The accounts child, for a world that reads what a restart restored.
    #[must_use]
    pub const fn accounts(&self) -> &jig_domain_accounts::Domain {
        &self.core.accounts
    }

    /// Install the application's fixed hold vocabulary before its first input.
    pub fn configure_holds(&mut self, env: &Env<Limits>, connector: u16, kinds: Box<[tasks::Kind]>) {
        let mut out = Queue::with_capacity(tasks::max_out(&env.limits.core.tasks));
        tasks::step(
            &mut self.core.tasks,
            &Env { now: env.now, wall: env.wall, limits: env.limits.core.tasks },
            tasks::Event::Kinds { connector, kinds },
            &mut out,
        );
        assert!(out.is_empty(), "fixed hold vocabulary has no effects");
    }
    /// Pieces whose kind or offset the core cannot keep (engine.md, 11).
    #[must_use]
    pub fn pieces_dropped(&self) -> u64 {
        self.pieces_dropped
    }

    /// Build the testing application without issuing a store request.
    #[must_use]
    pub fn new(config: Config, limits: &Limits) -> Domain {
        assert!(limits.routes > 0 && limits.journal.writes > 0 && limits.journal.held > 0, "finite route room");
        let core_room = core::room_max(&limits.core).expect("valid core room");
        let connector_room = connector::MAX_OUT.checked_mul(2).expect("two connector outputs");
        let writes = core_room.writes.checked_add(connector_room).expect("combined route writes fit");
        let held = core_room.held.checked_add(connector_room).expect("combined route deliveries fit");
        assert!(
            writes <= limits.journal.writes && held <= limits.journal.held,
            "journal admits the whole core and connector route before stepping"
        );
        Domain {
            pieces_dropped: 0,
            core: core::Core::new(config.core, &limits.core).expect("fixture core configuration"),
            first: connector::Domain::new(config.first, &limits.connector),
            second: connector::Domain::new(config.second, &limits.connector),
            journal: Journal::new(&limits.journal),
            now: Queue::with_capacity(limits.journal.now),
            store_answers: Queue::with_capacity(limits.routes),
            links: Set::with_capacity(limits.core.fleet.workers),
            busy: Queue::with_capacity(limits.routes),
            assignments: Map::with_capacity(limits.core.tasks.tasks),
            briefs: Map::with_capacity(limits.core.tasks.tasks),
            workspace_writes: Map::with_capacity(limits.core.tasks.tasks),
            procedure_steps: Map::with_capacity(limits.core.tasks.tasks),
            topic_calls: Map::with_capacity(limits.core.call_records),
            effects: Map::with_capacity(
                limits.core.call_records.checked_add(limits.core.tasks.tasks).expect("effect room"),
            ),
            effect_procedures: Map::with_capacity(limits.core.tasks.tasks),
            outbox_procedures: Map::with_capacity(limits.connector.entries.checked_mul(2).expect("outbox procedures")),
            judges: Map::with_capacity(limits.core.authority.facts),
            outbox_tasks: Map::with_capacity(limits.connector.entries.checked_mul(2).expect("outboxes")),
            payloads: Map::with_capacity(
                limits.core.fleet.turns.checked_add(limits.core.fleet.attempts).expect("payload room"),
            ),
            next_payload: 1,
            decision_wrote: false,
            stopped_reported: false,
            restart_active: false,
            restart_load: None,
        }
    }

    #[must_use]
    pub fn ready(&self) -> bool {
        !self.restart_active || self.core.restart_ready()
    }

    /// The client's current held-task context, admitted by the core's visibility rule.
    #[must_use]
    pub fn escalation(&self, person: u64, task: u64) -> Option<Box<tasks::EscalationContext>> {
        let context = self.core.tasks.escalation(task)?;
        let role = self.core.people.role(person, context.project);
        if self.core.escalation_visible(&context, person, role) { Some(context) } else { None }
    }

    /// Release retired child slots after every complete application iteration.
    pub fn reclaim(&mut self) {
        self.core.reclaim();
    }

    /// Whether no work can advance without another input. An assignment may
    /// still wait for a host whose next hello supplies its capacity.
    #[must_use]
    pub fn quiescent(&self) -> bool {
        self.journal.idle()
            && self.now.is_empty()
            && (!self.ready() || (!self.core.due_ready() && !self.core.finish_owed()))
    }
}

use link::Work;

/// Admit one input and route all synchronous continuations inside one decision.
pub fn step(domain: &mut Domain, env: &Env<Limits>, event: Event) {
    let Ok(event) = stopping::defer(domain, event) else { return };
    match event {
        Event::Link { channel, link } => decide(domain, env, Work::Link { channel, link }),
        Event::ResultRead { payload, end } => link::read(domain, env, payload, end),
        Event::Stop => decide(domain, env, Work::Core(core::Event::Stopping)),
        Event::Application { reply_to, sign_in, key, scope, number, digest, value, carried } => {
            decide(domain, env, Work::Application { reply_to, sign_in, key, scope, number, digest, value, carried });
        }
        Event::ConnectorTimer { number } => decide(domain, env, Work::ConnectorFire { number }),
        Event::WriterRead { connector, resource, event } => {
            decide(domain, env, Work::WriterRead { connector, resource, event });
        }
        Event::RestartBegin => {
            domain.restart_active = true;
            match domain.core.restart_begin() {
                core::RestartRequest::Step(step) => domain.restart_load = Some(step),
                core::RestartRequest::Idle
                | core::RestartRequest::Settings { .. }
                | core::RestartRequest::ProjectPolicy { .. }
                | core::RestartRequest::Refused { .. } => {}
            }
        }
        Event::RestartDone(step) => {
            if step == core::RestartStep::LoadCore {
                let restored_env = Env { now: env.now, wall: env.wall, limits: env.limits.core };
                if domain.core.settings_restored(&restored_env) {
                    let room = core::room_max(&env.limits.core).expect("validated core room").writes;
                    decide(domain, env, Work::Core(core::Event::SettingsCommit { room }));
                }
            }
            let request = domain.core.restart_done(step);
            decide(domain, env, Work::Restart(request));
        }
        Event::TranscriptLoaded { task, rows, done } => transcript_loaded(domain, env, task, rows, done),
        Event::Committed { number, fill } => {
            domain.journal.committed(number);
            let _filled = core::step(
                &mut domain.core,
                &Env { now: env.now, wall: env.wall, limits: env.limits.core },
                core::Event::Fill { bytes: fill },
            );
        }
        Event::Failed { number } => domain.journal.failed(number),
        Event::ProjectionEffect { goal, number, effect } => {
            decide(domain, env, Work::ProjectionEffect { goal, number, effect });
        }
        Event::TopicCall { to, key, number, topic } => decide(domain, env, Work::TopicCall { to, key, number, topic }),
        Event::EffectCall { to, key, number, effect, deadline, proposal } => {
            decide(domain, env, Work::EffectCall { to, key, number, effect, deadline, proposal });
        }
        Event::Timer(timer) => decide(domain, env, Work::Timer(timer)),
        Event::Core(event) => decide(domain, env, Work::Core(event)),
        Event::Credential { number, generation, valid } => decide(
            domain,
            env,
            Work::Core(core::Event::Account(jig_domain_accounts::Event::Credential { number, generation, valid })),
        ),
        Event::WatchApplication { watcher, sign_in, scope, number, subject, window } => {
            decide(domain, env, Work::WatchApplication { watcher, sign_in, scope, number, subject, window });
        }
        Event::ReadApplication { reply, sign_in, scope, number, subject, window } => {
            decide(domain, env, Work::ReadApplication { reply, sign_in, scope, number, subject, window });
        }
        Event::CallAnswered { to, call } => decide(domain, env, Work::CallAnswered { to, call }),
        Event::Connector { number, event } => decide(domain, env, Work::Connector { number, event }),
        Event::Text { task, attempt, turn, offset, text } => decide(
            domain,
            env,
            Work::Core(core::Event::RunText {
                run: Token::new(task),
                attempt: Token::new(attempt),
                turn,
                // This finite scripted source supplies one initial completion.
                retry: 0,
                offset,
                text,
                call: None,
            }),
        ),
        Event::Turn { channel, task, attempt, turn, cumulative, read, transcript, opens_window, refused } => {
            decide(
                domain,
                env,
                Work::Turn { channel, task, attempt, turn, cumulative, read, transcript, opens_window, refused },
            );
        }
        Event::Answer { channel, task, attempt, cumulative, end, read, refused } => {
            decide(domain, env, Work::Answer { channel, task, attempt, cumulative, end, read, refused });
        }
    }
}

fn transcript_loaded(domain: &mut Domain, env: &Env<Limits>, task: u64, rows: Box<[core::TranscriptRow]>, done: bool) {
    if !domain.core.transcripts.contains_key(&task) {
        return;
    }
    if rows.len() > usize::try_from(env.limits.core.resume_bytes).expect("u32 fits usize") {
        return;
    }
    for (row, text) in rows {
        if !domain.core.append_transcript(task, row, text) {
            decide(domain, env, Work::Core(core::Event::PreparationFailed { task }));
            return;
        }
    }
    if done {
        let event = if domain.core.transcript_readable(task) {
            core::Event::StartBrief { task }
        } else {
            core::Event::TranscriptUnread { task }
        };
        decide(domain, env, Work::Core(event));
    }
}

#[expect(clippy::too_many_lines, reason = "one bounded decision routes the complete root work vocabulary")]
fn decide(domain: &mut Domain, env: &Env<Limits>, first: Work) {
    let core_room = match &first {
        Work::Core(event) => core::room(&env.limits.core, event),
        Work::WatchApplication { .. }
        | Work::Link { .. }
        | Work::ReadApplication { .. }
        | Work::Application { .. }
        | Work::Restart(_)
        | Work::AdoptRestored
        | Work::AdoptDone
        | Work::Timer(_)
        | Work::WriterRead { .. }
        | Work::Finish
        | Work::ResumeFleet
        | Work::Connector { .. }
        | Work::ConnectorResume { .. }
        | Work::ConnectorFire { .. }
        | Work::ProjectionEffect { .. }
        | Work::TopicCall { .. }
        | Work::EffectCall { .. }
        | Work::Turn { .. }
        | Work::Answer { .. }
        | Work::CallAnswered { .. } => core::room_max(&env.limits.core),
    }
    .expect("validated core room");
    let connector_room = connector::MAX_OUT.checked_mul(2).expect("two connector outputs");
    let room = JournalRoom {
        writes: core_room.writes.checked_add(connector_room).expect("combined route writes"),
        held: core_room.held.checked_add(connector_room).expect("combined route deliveries"),
    };
    let Some(mut decision) = domain.journal.decision(&room) else { return };
    domain.decision_wrote = false;
    let mut finish_only = false;
    let mut work = Queue::with_capacity(env.limits.routes);
    work.push(first);
    for _ in 0..env.limits.routes {
        let Some(next) = work.pop() else {
            let requests =
                core::finish_decision(&mut domain.core, &Env { now: env.now, wall: env.wall, limits: env.limits.core });
            let core::Requests::Out(ref out) = requests;
            let done = out.len() == 1;
            route_core(domain, env, &mut decision, requests, &mut work);
            if done && work.is_empty() {
                break;
            }
            continue;
        };
        match next {
            Work::Link { channel, link } => link::receive(domain, channel, link, &mut work),
            Work::WatchApplication { watcher, sign_in, scope, number, subject, window } => {
                let token = Token::new(domain.next_payload);
                domain.next_payload = domain.next_payload.checked_add(1).expect("root subject token room");
                if number == 1 || number == 2 {
                    work.push(Work::Connector { number, event: connector::Event::DocumentPrepare { token, subject } });
                }
                work.push(Work::Core(core::Event::Watch {
                    watcher,
                    sign_in,
                    subject: core::documents::Watched::Application { scope, connector: number, subject: token },
                    window,
                }));
            }
            Work::ReadApplication { reply, sign_in, scope, number, subject, window } => {
                let token = Token::new(domain.next_payload);
                domain.next_payload = domain.next_payload.checked_add(1).expect("root read token room");
                if number == 1 || number == 2 {
                    work.push(Work::Connector { number, event: connector::Event::DocumentPrepare { token, subject } });
                }
                work.push(Work::Core(core::Event::Read {
                    reply,
                    sign_in,
                    read: core::reads::Read::Application { scope, connector: number, read: token, window },
                }));
            }
            Work::Application { reply_to, sign_in, key, scope, number, digest, value, carried } => {
                let owner = Token::new(domain.next_payload);
                domain.next_payload = domain.next_payload.checked_add(1).expect("root token room");
                if number == 1 || number == 2 {
                    work.push(Work::Connector { number, event: connector::Event::ApplicationPrepare { owner, value } });
                }
                work.push(Work::Core(core::Event::Application {
                    reply_to,
                    sign_in,
                    key,
                    scope,
                    connector: number,
                    digest,
                    owner,
                    carried,
                }));
            }
            Work::ConnectorFire { number } => {
                let mut out = Queue::with_capacity(connector::MAX_OUT);
                connector::fire(
                    numbered(domain, number),
                    &Env { now: env.now, wall: env.wall, limits: env.limits.connector },
                    &mut out,
                );
                route_connector(domain, env, &mut decision, number, &mut out, &mut work);
            }
            Work::Restart(request) => match request {
                core::RestartRequest::Idle => {}
                core::RestartRequest::Settings { .. }
                | core::RestartRequest::ProjectPolicy { .. }
                | core::RestartRequest::Refused { .. } => {
                    domain.stopped_reported = false;
                }
                core::RestartRequest::Step(step) => match step {
                    core::RestartStep::AdoptRuns => {
                        work.push(Work::Core(core::Event::Tasks(tasks::Event::Restored)));
                        work.push(Work::AdoptRestored);
                    }
                    core::RestartStep::Open => {
                        let _request = domain.core.restart_done(step);
                    }
                    core::RestartStep::SettleOutbox { connector: number } => {
                        work.push(Work::Connector {
                            number,
                            event: connector::Event::Restart(connector::RestartStep::SettleOutbox),
                        });
                    }
                    core::RestartStep::LoadInbox => {
                        // The accounts' records came with the core's ranges:
                        // want each real source once its account rows are restored.
                        work.push(Work::Core(core::Event::Account(jig_domain_accounts::Event::Want)));
                        hold(&mut decision, Delivery::Restart(step));
                    }
                    core::RestartStep::LoadCore
                    | core::RestartStep::RestoreConnector { .. }
                    | core::RestartStep::ReadAfresh { .. } => hold(&mut decision, Delivery::Restart(step)),
                },
            },
            Work::AdoptRestored => {
                if domain.core.restart_admit_claims() {
                    for _ in 0..domain.core.adopted.len() {
                        work.push(Work::Core(core::Event::Fleet(
                            domain.core.adopted.pop().expect("restored claim count"),
                        )));
                    }
                    work.push(Work::Core(core::Event::Fleet(fleet::Event::Loaded)));
                    work.push(Work::AdoptDone);
                }
            }
            Work::AdoptDone => {
                let request = domain.core.restart_done(core::RestartStep::AdoptRuns);
                work.push(Work::Restart(request));
            }

            Work::ProjectionEffect { goal, number, effect } => {
                let owner = effect_payload(domain, effect);
                work.push(Work::Core(core::Event::EffectStart {
                    owner,
                    connector: number,
                    origin: core::EffectOrigin::Projection { goal, entry: None },
                }));
            }
            Work::TopicCall { to, key, number, topic } => topics::start(domain, to, key, number, topic, &mut work),
            Work::EffectCall { to, key, number, effect, deadline, proposal } => {
                let owner = effect_payload(domain, effect);
                let origin = match proposal {
                    Some(reason) => core::EffectOrigin::Propose { to, key, reason, as_holder: false },
                    None => core::EffectOrigin::Call { to, key, deadline },
                };
                work.push(Work::Core(core::Event::EffectStart { owner, connector: number, origin }));
            }
            Work::Timer(timer) => {
                let requests =
                    core::fire(&mut domain.core, &Env { now: env.now, wall: env.wall, limits: env.limits.core }, timer);
                route_core(domain, env, &mut decision, requests, &mut work);
            }
            Work::CallAnswered { to, call } => {
                let answer = payload(domain, Payload::SettledCall(call));
                work.push(Work::Core(core::Event::Fleet(fleet::Event::Relayed { to, answer })));
            }
            Work::Core(event) => {
                let requests =
                    core::step(&mut domain.core, &Env { now: env.now, wall: env.wall, limits: env.limits.core }, event);
                route_core(domain, env, &mut decision, requests, &mut work);
            }
            Work::Finish => finish_only = true,
            Work::ResumeFleet => {
                let requests = core::resume_fleet(
                    &mut domain.core,
                    &Env { now: env.now, wall: env.wall, limits: env.limits.core },
                );
                route_core(domain, env, &mut decision, requests, &mut work);
            }
            Work::WriterRead { connector, resource, event } => {
                if let connector::SystemEvent::Fact { resource: observed, fact, .. } = &event
                    && !fact.pending
                    && observed.segments() == resource.path.as_ref()
                {
                    work.push(Work::Connector { number: connector, event: connector::Event::System(event) });
                    work.push(Work::Core(core::Event::Tasks(tasks::Event::ReadAfresh { resource })));
                }
            }
            Work::Connector { number, event } => {
                let mut requests = Queue::with_capacity(connector::MAX_OUT);
                let connector = numbered(domain, number);
                connector::step(
                    connector,
                    &Env { now: env.now, wall: env.wall, limits: env.limits.connector },
                    event,
                    &mut requests,
                );
                route_connector(domain, env, &mut decision, number, &mut requests, &mut work);
            }
            Work::ConnectorResume { number } => {
                let mut requests = Queue::with_capacity(connector::MAX_OUT);
                connector::resume(numbered(domain, number), &mut requests);
                route_connector(domain, env, &mut decision, number, &mut requests, &mut work);
            }
            Work::Turn { channel, task, attempt, turn, cumulative, read, transcript, opens_window, refused } => {
                let body = Payload::Turn { task, attempt, turn, cumulative, read, transcript, opens_window, refused };
                messages::receive(domain, &mut work, channel, body);
            }
            Work::Answer { channel, task, attempt, cumulative, end, read, refused } => {
                let body =
                    Payload::Answer { task, attempt, cumulative, end: link::Ending::Scripted(end), read, refused };
                messages::receive(domain, &mut work, channel, body);
            }
        }
    }
    assert!(work.is_empty(), "synchronous routes fit configured room");
    if finish_only {
        assert!(!domain.decision_wrote && !domain.core.counters.dirty(), "stopping continuation makes no commit");
    }
    if domain.decision_wrote || domain.core.counters.dirty() {
        let header = domain.core.counters.next_commit();
        write(
            domain,
            &mut decision,
            Write::Save(Record::Core(core::Record::Core(core::CoreRecord::Deployment(header)))),
        );
    }
    if domain.core.restart_failure().is_none() {
        domain.journal.accept(decision);
    }
}

fn effect_payload(domain: &mut Domain, effect: connector::Effect) -> Token {
    let token = Token::new(domain.next_payload);
    domain.next_payload = domain.next_payload.checked_add(1).expect("bounded owner numbers");
    assert!(domain.effects.insert(token, effect).is_ok(), "admitted effect room");
    token
}

fn payload(domain: &mut Domain, value: Payload) -> Token {
    let token = Token::new(domain.next_payload);
    domain.next_payload = domain.next_payload.checked_add(1).expect("bounded test flight numbering");
    assert!(domain.payloads.insert(token, value).is_ok(), "payload room reserved");
    token
}

fn numbered(domain: &mut Domain, number: u16) -> &mut connector::Domain {
    match number {
        1 => &mut domain.first,
        2 => &mut domain.second,
        _ => unreachable!("testing application has exactly two connectors"),
    }
}

/// Drain at most one commit and all currently durable outputs.
pub fn release(domain: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    admission::resume(domain, env);
    stopping::release(domain, out);
    if let Some(step) = domain.restart_load.take() {
        out.push(Request::Restart(step));
        return;
    }
    if domain.journal.stopped() || domain.core.restart_failure().is_some() {
        if !domain.stopped_reported {
            domain.stopped_reported = true;
            out.push(Request::Stop);
        }
        return;
    }
    if let Some(commit) = domain.journal.commit() {
        out.push(Request::Commit { number: commit.number, writes: commit.writes });
    }
    let mut ready = Queue::with_capacity(env.limits.journal.release);
    let _released = domain.journal.release(&mut ready);
    for _ in 0..ready.len() {
        let delivery = ready.pop().expect("release count");
        match delivery {
            Delivery::Core(core::Held::MakeEffect { connector, entry }) => {
                step(domain, env, Event::Connector { number: connector, event: connector::Event::Make { entry } });
            }
            Delivery::Core(core::Held::TaskTurnKept { task, attempt, turn }) => {
                step(
                    domain,
                    env,
                    Event::Core(core::Event::Fleet(jig_domain_fleet::Event::TurnKept {
                        run: Token::new(task),
                        attempt: Token::new(attempt),
                        turn,
                    })),
                );
            }
            Delivery::Core(core::Held::TaskTerminalAcknowledged { task, attempt }) => {
                step(
                    domain,
                    env,
                    Event::Core(core::Event::Fleet(jig_domain_fleet::Event::Acknowledge {
                        run: Token::new(task),
                        attempt: Token::new(attempt),
                    })),
                );
            }
            Delivery::Fleet(event) => step(domain, env, Event::Core(core::Event::Fleet(event))),
            Delivery::Core(core::Held::StopRun { task, attempt }) => {
                step(
                    domain,
                    env,
                    Event::Core(core::Event::Fleet(fleet::Event::Cancel {
                        run: Token::new(task),
                        attempt: Token::new(attempt),
                    })),
                );
            }
            Delivery::Core(core::Held::Relay { task, attempt, word, .. }) => {
                let event = relay_message(domain, task, attempt, word);
                step(domain, env, event);
            }
            other @ (Delivery::ApplicationDocument { .. }
            | Delivery::Link { .. }
            | Delivery::ApplicationRead { .. }
            | Delivery::Application { .. }
            | Delivery::ProjectMade { .. }
            | Delivery::Projection { .. }
            | Delivery::Message { .. }
            | Delivery::Restart(_)
            | Delivery::CallBusy { .. }
            | Delivery::CallAnswer { .. }
            | Delivery::Core(_)
            | Delivery::Assigned { .. }
            | Delivery::System { .. }
            | Delivery::LostRead { .. }
            | Delivery::Procedure { .. }) => out.push(Request::Deliver(other)),
        }
    }
    for _ in 0..domain.now.len() {
        let now = domain.now.pop().expect("bounded now count");
        out.push(now);
    }
    continue_ready(domain, env);
}

fn continue_ready(domain: &mut Domain, env: &Env<Limits>) {
    if domain.stopping() {
        if domain.core.finish_owed() {
            decide(domain, env, Work::Finish);
        }
        return;
    }
    if resume_restart_outbox(domain, env) {
        return;
    }
    if domain.journal.idle() && (!domain.restart_active || domain.core.restart_ready()) {
        if let Some(number) = closing_connector(domain) {
            decide(domain, env, Work::ConnectorResume { number });
            return;
        }
        if let Some(context) = domain.core.take_due() {
            decide(domain, env, Work::Core(core::Event::Activate { context, ready: true }));
            return;
        }
        if domain.core.fleet.is_ready() || domain.core.finish_owed() || timers_due(domain, env) {
            decide(domain, env, Work::ResumeFleet);
        }
    }
}

fn wall_due(env: &Env<Limits>, expiry: Wall) -> bool {
    let remaining = expiry.as_nanos().saturating_sub(env.wall.as_nanos());
    env.now.saturating_add(skein_lib::Duration::from_nanos(remaining)) <= env.now
}

fn timers_due(domain: &Domain, env: &Env<Limits>) -> bool {
    for (_, due) in domain.core_deadlines(env.now, env.wall) {
        if let Some(due) = due
            && due <= env.now
        {
            return true;
        }
    }
    for (_, due) in domain.connector_deadlines() {
        if let Some(due) = due
            && wall_due(env, due)
        {
            return true;
        }
    }
    match domain.effect_deadline() {
        Some(due) => wall_due(env, due),
        None => false,
    }
}

fn closing_connector(domain: &Domain) -> Option<u16> {
    if connector::closing_ready(&domain.first) {
        Some(1)
    } else if connector::closing_ready(&domain.second) {
        Some(2)
    } else {
        None
    }
}

fn resume_restart_outbox(domain: &mut Domain, env: &Env<Limits>) -> bool {
    if !domain.journal.idle() {
        return false;
    }
    let number = match domain.core.restart_step() {
        Some(core::RestartStep::SettleOutbox { connector }) => connector,
        Some(
            core::RestartStep::LoadCore
            | core::RestartStep::LoadInbox
            | core::RestartStep::RestoreConnector { .. }
            | core::RestartStep::AdoptRuns
            | core::RestartStep::ReadAfresh { .. }
            | core::RestartStep::Open,
        )
        | None => return false,
    };
    decide(
        domain,
        env,
        Work::Connector { number, event: connector::Event::Restart(connector::RestartStep::SettleOutbox) },
    );
    true
}

fn write(domain: &mut Domain, decision: &mut Decision<Write, Delivery>, value: Write) {
    domain.decision_wrote = true;
    assert!(decision.write(value).is_ok(), "reserved write room");
}

fn hold(decision: &mut Decision<Write, Delivery>, value: Delivery) {
    assert!(decision.hold(value).is_ok(), "reserved delivery room");
}

// The translations below are deliberately exhaustive: a new boundary variant
// must make the testing application say how it crosses its root.
mod application_facts;
mod client_documents;
pub mod link;
mod messages;
mod routes;
mod steps;
pub use steps::procedure_input;
#[cfg(test)]
mod tests;
mod topics;
use messages::Payload;
mod stopping;
use routes::{route_connector, route_core};

mod limits;
pub use limits::{max_out, worst_case};
mod admission;
pub use admission::try_step;

mod census;
pub use census::inbox_census;

/// Translate one durable row during the requested restart load. The root
/// supplies header, children, proofs, then calls; the core validates their links.
#[must_use]
pub fn restore_record(domain: &mut Domain, env: &Env<Limits>, record: Record) -> bool {
    if domain.core.restart_step() == Some(core::RestartStep::LoadCore) {
        let after_settings = match &record {
            Record::Core(core::Record::Core(core::CoreRecord::Deployment(_) | core::CoreRecord::Settings(_))) => false,
            Record::Core(_) | Record::Connector { .. } => true,
        };
        if after_settings
            && !domain.core.settings_restored(&Env { now: env.now, wall: env.wall, limits: env.limits.core })
        {
            return false;
        }
    }
    if domain.core.restart_step() == Some(core::RestartStep::LoadInbox) {
        return if let Record::Core(row) = record {
            domain.core.restore_inbox_row(&env.limits.core, &row)
        } else {
            false
        };
    }
    match record {
        Record::Core(core::Record::Core(row)) => match domain.core.restore_core(row, &env.limits.core) {
            core::Restored::Deployment { commits } => {
                domain.journal = Journal::from_durable(&env.limits.journal, commits);
                true
            }
            core::Restored::Live | core::Restored::Archive => true,
            core::Restored::Rejected => false,
        },
        Record::Core(core::Record::Tasks(row)) => {
            if !domain.core.restore_task_row(&row) {
                return false;
            }
            let mut out = Queue::with_capacity(tasks::max_out(&env.limits.core.tasks));
            tasks::step(
                &mut domain.core.tasks,
                &Env { now: env.now, wall: env.wall, limits: env.limits.core.tasks },
                tasks::Event::Restore { record: row },
                &mut out,
            );
            out.is_empty()
        }
        Record::Core(core::Record::People(jig_domain_people::Stored::Policy { project, name, value })) => {
            domain.core.restore_policy(&env.limits.core, project, name, value)
        }
        Record::Core(core::Record::People(row)) => {
            if !domain.core.restore_people_row(&row) {
                return false;
            }
            let mut out = Queue::with_capacity(jig_domain_people::max_out(&env.limits.core.people));
            jig_domain_people::step(
                &mut domain.core.people,
                &Env { now: env.now, wall: env.wall, limits: env.limits.core.people },
                jig_domain_people::Event::Restore { record: row },
                &mut out,
            );
            out.is_empty()
        }
        Record::Core(core::Record::Notes(_)) => true,
        Record::Core(core::Record::Accounts(record)) => {
            // A record breaking the child's rules is counted there and
            // dropped; it never refuses the start (store.md 4.4 item 9).
            let mut out = Queue::with_capacity(jig_domain_accounts::max_out(&env.limits.core.accounts));
            jig_domain_accounts::step(
                &mut domain.core.accounts,
                &Env { now: env.now, wall: env.wall, limits: env.limits.core.accounts },
                jig_domain_accounts::Event::Restore { record },
                &mut out,
            );
            out.is_empty()
        }
        Record::Connector { number, record } => restore_connector(domain, env, number, record),
    }
}

fn relay_message(domain: &mut Domain, task: u64, attempt: u64, word: tasks::Word) -> Event {
    let name = Token::new(word.number);
    let body = payload(domain, Payload::InboxWord(word));
    Event::Core(core::Event::Fleet(fleet::Event::Inbound {
        run: Token::new(task),
        attempt: Token::new(attempt),
        message: fleet::Message { name, sender: body, words: body },
    }))
}

/// Restore connector-owned rows and replay their current adoption reports.
fn restore_connector(domain: &mut Domain, env: &Env<Limits>, number: u16, record: connector::Record) -> bool {
    match &record {
        connector::Record::Outbox(row) => {
            if domain.outbox_tasks.insert(row.number, row.task).is_err() {
                return false;
            }
            // The application encodes procedure purposes with attempt zero.
            if row.key.attempt == 0 && domain.outbox_procedures.insert(row.number, number).is_err() {
                return false;
            }
        }
        connector::Record::ApplicationObject { .. }
        | connector::Record::ApplicationAnswer { .. }
        | connector::Record::Proposal { .. }
        | connector::Record::Procedure(_)
        | connector::Record::ProcedureDelegateOutcome { .. }
        | connector::Record::Result { .. }
        | connector::Record::Task { .. }
        | connector::Record::Adoption { .. }
        | connector::Record::Subscription { .. }
        | connector::Record::Pool { .. }
        | connector::Record::Made { .. } => {}
    }
    let mut out = Queue::with_capacity(connector::MAX_OUT);
    connector::step(
        numbered(domain, number),
        &Env { now: env.now, wall: env.wall, limits: env.limits.connector },
        connector::Event::Restore { record },
        &mut out,
    );
    for _ in 0..out.len() {
        match out.pop().expect("restored connector output count") {
            connector::Request::Resource { project, resource } => {
                let role = match resource.role {
                    connector::ResourceRole::Owned => core::connector::ResourceRole::Owned,
                    connector::ResourceRole::Participating => core::connector::ResourceRole::Participant,
                    connector::ResourceRole::Context => core::connector::ResourceRole::Context,
                    connector::ResourceRole::Unavailable => core::connector::ResourceRole::Unavailable,
                };
                if !domain.core.restore_resource_role(
                    &Env { now: env.now, wall: env.wall, limits: env.limits.core },
                    project,
                    tasks::Name { connector: number, path: resource.path.segments().into() },
                    role,
                ) {
                    return false;
                }
            }
            connector::Request::Topic { .. }
            | connector::Request::TopicRefused { .. }
            | connector::Request::Document { .. }
            | connector::Request::ReadAnswered { .. }
            | connector::Request::Marked { .. }
            | connector::Request::Closed { .. }
            | connector::Request::Verdict { .. }
            | connector::Request::Step { .. }
            | connector::Request::Answer { .. }
            | connector::Request::Ready { .. }
            | connector::Request::Section { .. }
            | connector::Request::Workspace { .. }
            | connector::Request::DriftResource { .. }
            | connector::Request::Changed { .. }
            | connector::Request::Described { .. }
            | connector::Request::EffectRefused { .. }
            | connector::Request::EffectBusy { .. }
            | connector::Request::Make { .. }
            | connector::Request::Outcome { .. }
            | connector::Request::Named { .. }
            | connector::Request::Unknown { .. }
            | connector::Request::Refused { .. }
            | connector::Request::Adopted { .. }
            | connector::Request::Slots { .. }
            | connector::Request::Drift { .. }
            | connector::Request::News { .. }
            | connector::Request::Save { .. }
            | connector::Request::Erase { .. }
            | connector::Request::RestartDone
            | connector::Request::ApplicationDone { .. }
            | connector::Request::ApplicationAgain { .. }
            | connector::Request::ApplicationRelease { .. }
            | connector::Request::System(_) => return false,
        }
    }
    true
}

/// Settings rows accompanying a deployment header in an explicitly seeded test store.
#[must_use]
pub fn initial_settings(defaults: &core::settings::Defaults) -> Box<[core::Record]> {
    use core::settings as s;
    let count = u32::try_from(defaults.charters.len())
        .expect("fixture charters")
        .checked_add(u32::try_from(defaults.templates.len()).expect("fixture templates"))
        .expect("fixture settings count")
        .checked_add(defaults.policies.len())
        .expect("fixture settings count")
        .checked_add(3)
        .expect("fixture settings count");
    let mut rows = List::with_capacity(count);
    for charter in &defaults.charters {
        rows.push(core::Record::Core(core::CoreRecord::Settings(s::Record::Default {
            setting: s::Setting::Charter(charter.number),
            default: s::Default { value: s::Value::Charter(Box::new(charter.clone())), revision: 1 },
        })))
        .expect("fixture settings count");
    }
    for template in &defaults.templates {
        rows.push(core::Record::Core(core::CoreRecord::Settings(s::Record::Default {
            setting: s::Setting::Template(template.number),
            default: s::Default { value: s::Value::Template(Box::new(template.clone())), revision: 1 },
        })))
        .expect("fixture settings count");
    }
    for (&number, policy) in &defaults.policies {
        rows.push(core::Record::Core(core::CoreRecord::Settings(s::Record::Default {
            setting: s::Setting::Policy(number),
            default: s::Default { value: s::Value::Policy(Box::new(policy.clone())), revision: 1 },
        })))
        .expect("fixture settings count");
    }
    rows.push(core::Record::Core(core::CoreRecord::Settings(s::Record::Default {
        setting: s::Setting::Rules,
        default: s::Default { value: s::Value::Rules(Box::new(defaults.rules.clone())), revision: 1 },
    })))
    .expect("fixture settings count");
    rows.push(core::Record::Core(core::CoreRecord::Settings(s::Record::Revision {
        history: false,
        revision: s::Revision {
            head: s::Head {
                number: 1,
                against: 0,
                at: Wall::EPOCH,
                by: s::Decider::FirstStart,
                proposal: None,
                moved: false,
            },
            items: Box::new([]),
        },
    })))
    .expect("fixture settings count");
    rows.push(core::Record::Core(core::CoreRecord::Settings(s::Record::Extent { oldest: 0 })))
        .expect("fixture settings count");
    rows.into_boxed()
}

/// The same fixture rows from an unmodified core's already loaded binary defaults.
#[must_use]
pub fn initial_settings_current(current: &core::settings::Current) -> Box<[core::Record]> {
    use core::settings as s;
    let names = current.offered();
    let mut rows = List::with_capacity(names.len().checked_add(2).expect("fixture settings count"));
    for &setting in &names {
        rows.push(core::Record::Core(core::CoreRecord::Settings(s::Record::Default {
            setting,
            default: s::Default {
                value: current.value(setting).expect("offered fixture default").clone(),
                revision: 1,
            },
        })))
        .expect("fixture settings count");
    }
    rows.push(core::Record::Core(core::CoreRecord::Settings(s::Record::Revision {
        history: false,
        revision: s::Revision {
            head: s::Head {
                number: 1,
                against: 0,
                at: Wall::EPOCH,
                by: s::Decider::FirstStart,
                proposal: None,
                moved: false,
            },
            items: Box::new([]),
        },
    })))
    .expect("fixture settings count");
    rows.push(core::Record::Core(core::CoreRecord::Settings(s::Record::Extent { oldest: 0 })))
        .expect("fixture settings count");
    rows.into_boxed()
}
