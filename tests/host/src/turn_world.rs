//! A peer world for the worker host (domain/hosts.md, sections
//! 6.4-6.7 and 11). The parent plays a workspace and agent; the engine's
//! fake store commits turns before acknowledging them. The link may lose
//! contact and replays the host's retained turns before its held answer.

use jig_domain_host::{LinkDown, LinkUp};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use jig_domain_host::{
    self as host, Answer, AnsweredCall, Ask, Assignment, Delivery, DeliveryOutcome, Ending, Event, Finish, Limits,
    Reason, Request, RunAssignment, Turn, Workspace,
};
use skein_lib::{Duration, Env, Queue, Time, Token, Wall};

const RUN: Token = Token::new(31);
const ATTEMPT: Token = Token::new(932);
const SPACE: Token = Token::new(400);
const AGENT: Token = Token::new(500);

/// What the peers saw, counted independently of the host's private state.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Stats {
    pub transmissions: u32,
    pub commits: u32,
    pub turn_acks: u32,
    pub answers: u32,
    pub answer_acks: u32,
    pub deliveries: u32,
    pub relays: u32,
    pub replies: u32,
    pub saves: u32,
    pub releases: u32,
    pub facts: u32,
    pub stops: u32,
    pub agent_acks: u32,
    pub failures: u32,
}

/// The committed conversation state received by the scripted agent.
pub struct StartedState {
    pub live: host::LiveAsk,
    pub identity: [u8; 16],
    pub messages: Box<[host::StartMessage]>,
    pub activation: u64,
    pub turns: Box<[Box<[u8]>]>,
    pub answered: Box<[AnsweredCall]>,
}

/// The call the scripted engine received from the host.
pub struct RelayedCall {
    pub name: Box<[u8]>,
    pub tool: Box<[u8]>,
    pub writes: bool,
    pub input: Box<[u8]>,
    pub deadline: Duration,
}

/// The message the scripted agent received from the host.
pub struct ReceivedMessage {
    pub finish_requested: bool,
    pub name: Token,
    pub sender: Box<[u8]>,
    pub words: Box<[u8]>,
}

/// One host and its scripted capabilities and committed engine observations.
#[expect(clippy::struct_excessive_bools, reason = "independent link, agent, workspace and acknowledgement states")]
pub struct World {
    limits: Limits,
    env: Env<Limits>,
    host: host::Domain,
    out: Queue<Request>,
    events: VecDeque<Event>,
    connected: bool,
    owner: Option<Token>,
    reading: bool,
    agent_live: bool,
    stop_waits: bool,
    stop_pending: bool,
    has_workspace: bool,
    space_live: bool,
    answer: Option<Answer>,
    answer_seen: Option<String>,
    answer_acked: bool,
    seen: BTreeMap<u32, Turn>,
    committed: BTreeSet<u32>,
    acked: BTreeSet<u32>,
    stats: Stats,
    started_state: Option<StartedState>,
    relayed_call: Option<RelayedCall>,
    answered_call: Option<Box<[u8]>>,
    received_message: Option<ReceivedMessage>,
    hello: Option<host::Hello>,
    reply: Option<host::Reply>,
    next_relay: Option<host::Relayed>,
    received_grant: Option<host::Grant>,
    prepare_waits: bool,
    start_waits: bool,
    save_waits: bool,
    save_pending: bool,
    last_read: Option<Token>,
    refusals: crate::refusals::Referee,
    pub reports: Vec<host::LinkUp>,
    pub link_words: Vec<&'static str>,
}

impl World {
    /// A small window that can hold two maximum bodies.
    #[must_use]
    pub fn new() -> Self {
        Self::with_piece_limits(2, 4096)
    }

    /// The same peers with explicit live-piece and shared activity bounds.
    #[must_use]
    pub fn with_piece_limits(told: u32, piece_bytes: u32) -> Self {
        let mut limits = crate::Settings::calm(1).host;
        limits.slots = 1;
        limits.transcript_bytes = 512;
        limits.delivery_evidence_bytes = 128;
        limits.turn_bytes = 32;
        limits.turns = 2;
        limits.turn_queue_bytes = 64;
        limits.told = told;
        limits.piece_bytes = piece_bytes;
        assert!(host::worst_case(&limits).is_some());
        Self {
            limits,
            env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits },
            host: host::Domain::new(&limits),
            out: Queue::with_capacity(host::max_out(&limits)),
            events: VecDeque::new(),
            connected: true,
            owner: None,
            reading: false,
            agent_live: false,
            stop_waits: false,
            stop_pending: false,
            has_workspace: false,
            space_live: false,
            answer: None,
            answer_seen: None,
            answer_acked: false,
            seen: BTreeMap::new(),
            committed: BTreeSet::new(),
            acked: BTreeSet::new(),
            stats: Stats::default(),
            started_state: None,
            relayed_call: None,
            answered_call: None,
            received_message: None,
            hello: None,
            reply: None,
            next_relay: None,
            received_grant: None,
            link_words: Vec::new(),
            reports: Vec::new(),
            prepare_waits: false,
            start_waits: false,
            save_waits: false,
            save_pending: false,
            last_read: None,
            refusals: crate::refusals::Referee::default(),
        }
    }

    #[must_use]
    pub const fn stats(&self) -> Stats {
        self.stats
    }

    #[must_use]
    pub fn reading(&self) -> bool {
        self.reading
    }

    #[must_use]
    pub fn answer(&self) -> Option<&Answer> {
        self.answer.as_ref()
    }

    #[must_use]
    pub fn lost_facts(&self) -> u64 {
        self.host.activity_lost()
    }

    #[must_use]
    pub fn piece_counts(&self) -> (u64, u64) {
        (self.host.pieces_fenced(), self.host.pieces_lost())
    }

    #[must_use]
    pub fn hello(&self) -> Option<&host::Hello> {
        self.hello.as_ref()
    }

    #[must_use]
    pub fn reply(&self) -> Option<&host::Reply> {
        self.reply.as_ref()
    }

    pub fn relay_answer(&mut self, answer: host::Relayed) {
        self.next_relay = Some(answer);
        self.call(false);
    }

    #[must_use]
    pub fn slots_taken(&self) -> u32 {
        self.host.hosted()
    }

    pub fn another_assignment(&mut self) -> host::Refusal {
        let assignment = RunAssignment {
            identity: [0; 16],
            run: Token::new(32),
            attempt: Token::new(933),
            workspace: None,
            save: false,
            live: jig_domain_host::LiveAsk::NONE,
            charter: Box::from(&b"charter"[..]),
            grants: Box::new([]),
        };
        host::step(&mut self.host, &self.env, crate::fixtures::assign(assignment), &mut self.out);
        let Some(Request::Up(LinkUp::Answer { answer: Answer { ending: Ending::Refused(refusal), .. }, .. })) =
            self.out.pop()
        else {
            panic!("assignment should be refused")
        };
        refusal
    }

    /// Assigns one attempt. Its workspace prepares and its agent starts
    /// through the parent's immediate scripted terminals.
    pub fn assign(&mut self, workspace: bool) {
        self.assign_with_grants(workspace, Box::new([]));
    }

    /// Assign an itemless run holding the engine's bounded grants.
    pub fn assign_granted(&mut self, grants: Box<[host::Grant]>) {
        self.assign_with_grants(false, grants);
    }

    /// A renewed name received from the engine's grant table.
    pub fn grant(&mut self, grant: host::Grant) {
        self.send(Event::Down(LinkDown::Grant { run: RUN, attempt: ATTEMPT, grant }));
    }

    /// The last grant handed to the scripted agent.
    #[must_use]
    pub fn received_grant(&self) -> Option<host::Grant> {
        self.received_grant
    }

    fn assign_with_grants(&mut self, workspace: bool, grants: Box<[host::Grant]>) {
        self.has_workspace = workspace;

        let workspace = if workspace { Some(Workspace { workstream: 3, items: Token::new(4) }) } else { None };
        let assignment = RunAssignment {
            identity: [0; 16],
            run: RUN,
            attempt: ATTEMPT,
            workspace,
            save: true,
            live: jig_domain_host::LiveAsk::NONE,
            charter: Box::from(&b"charter"[..]),
            grants,
        };
        self.send(Event::Down(LinkDown::Assign {
            assignment: Assignment {
                messages: Box::new([]),
                assignment,
                turns: vec![Box::from(&b"prior turn"[..])].into_boxed_slice(),
                answered: Box::new([]),
            },
        }));
        if !self.prepare_waits {
            assert!(self.owner.is_some() && self.agent_live && self.reading);
        }
    }

    /// Assign a resumed run with each turn and its settled call tail intact.
    pub fn assign_conversation(&mut self, turns: Box<[Box<[u8]>]>, answered: Box<[AnsweredCall]>) {
        self.assign_live_conversation(turns, answered, host::LiveAsk::NONE);
    }

    /// Assign a conversation with its ask passed through to the scripted agent.
    pub fn assign_live_conversation(
        &mut self,
        turns: Box<[Box<[u8]>]>,
        answered: Box<[AnsweredCall]>,
        live: host::LiveAsk,
    ) {
        let assignment = RunAssignment {
            identity: [0; 16],
            run: RUN,
            attempt: ATTEMPT,
            workspace: None,
            save: false,
            live,
            charter: Box::from(&b"charter"[..]),
            grants: Box::new([]),
        };
        self.send(Event::Down(LinkDown::Assign {
            assignment: Assignment { messages: Box::new([]), assignment, turns, answered },
        }));
        assert!(self.owner.is_some() && self.agent_live && self.reading);
    }

    /// Assign a fresh run with its identity and oldest inbox messages.
    pub fn assign_start(&mut self, identity: [u8; 16], messages: Box<[host::StartMessage]>) {
        let assignment = RunAssignment {
            identity,
            run: RUN,
            attempt: ATTEMPT,
            workspace: None,
            save: false,
            live: jig_domain_host::LiveAsk::NONE,
            charter: Box::from(&b"charter"[..]),
            grants: Box::new([]),
        };
        self.send(crate::fixtures::assign_with(assignment, messages));
    }

    /// The activation and committed state the scripted agent received.
    #[must_use]
    pub fn started_state(&self) -> Option<&StartedState> {
        self.started_state.as_ref()
    }

    /// The call the engine saw.
    #[must_use]
    pub fn relayed_call(&self) -> Option<&RelayedCall> {
        self.relayed_call.as_ref()
    }

    /// The name under which the agent received the call's answer.
    #[must_use]
    pub fn answered_call(&self) -> Option<&[u8]> {
        self.answered_call.as_deref()
    }

    /// The message the agent saw.
    #[must_use]
    pub fn received_message(&self) -> Option<&ReceivedMessage> {
        self.received_message.as_ref()
    }

    pub fn turn(&mut self, number: u32, body: &[u8]) {
        self.turn_with(number, body, None, false);
    }

    pub fn turn_with(&mut self, number: u32, body: &[u8], read: Option<Token>, opens_window: bool) {
        self.last_read = read;
        assert!(self.reading, "the agent waits for host credit");
        self.reading = false;
        let turn = Turn {
            opens_window,
            refused: Box::new([]),
            turn: number,
            spent: u64::from(number) * 17,
            read,
            body: Box::from(body),
        };
        self.send(Event::from_agent(host::FromAgent::Turn { owner: self.owner.expect("assigned"), turn }));
    }

    pub fn refuse(&mut self, name: Token, reason: host::MessageRefusal) {
        self.send(Event::from_agent(host::FromAgent::Refused { owner: self.owner.expect("assigned"), name, reason }));
    }
    pub fn relay_message(&mut self, name: Token) {
        self.send(Event::Down(LinkDown::Message {
            run: RUN,
            attempt: ATTEMPT,
            name,
            sender: Box::new([]),
            words: Box::from(&b"message"[..]),
            finish_requested: false,
        }));
    }
    pub fn park(&mut self) { self.park_attempt(ATTEMPT); }
    pub fn park_attempt(&mut self, attempt: Token) {
        self.send(Event::Down(LinkDown::Park { run: RUN, attempt }));
    }
    pub fn cancel(&mut self) {
        self.send(Event::Down(LinkDown::Cancel { run: RUN, attempt: ATTEMPT }));
    }
    pub fn yield_run(&mut self) { self.send(Event::Yielded { owner: self.owner() }); }
    pub fn starting(&mut self) { self.start_waits = true; self.assign(false); }
    pub fn started(&mut self) {
        self.start_waits = false;
        self.send(Event::Started { owner: self.owner(), agent: AGENT });
    }
    pub fn preparing(&mut self) {
        self.prepare_waits = true;
        self.assign(true);
    }
    pub fn prepared(&mut self) {
        self.prepare_waits = false;
        self.send(Event::Prepared { owner: self.owner.expect("preparing"), workspace: SPACE });
    }
    #[must_use]
    pub fn received_turn(&self, number: u32) -> Option<&Turn> {
        self.seen.get(&number)
    }
    #[must_use]
    pub fn lost_refusals(&self) -> u64 {
        self.host.refusals_lost()
    }
    pub fn activity(&mut self, activity: host::Activity) {
        self.agent_report(host::FromAgent::Activity { owner: self.owner.expect("assigned"), activity });
    }
    pub fn piece(&mut self, piece: host::Piece) {
        self.agent_report(host::FromAgent::Piece { owner: self.owner(), piece });
    }
    pub fn agent_report(&mut self, report: host::FromAgent) {
        self.send(Event::from_agent(report));
    }
    #[must_use]
    pub fn owner(&self) -> Token {
        self.owner.expect("assigned")
    }
    pub fn unauthorized(&mut self) {
        self.agent_report(host::FromAgent::Unauthorized { owner: self.owner(), account: 7, generation: 4 });
    }
    pub fn limited(&mut self, reset: Option<Duration>) {
        self.agent_report(host::FromAgent::Limited { owner: self.owner(), account: 7, reset });
    }
    pub fn account_refused(&mut self) {
        self.agent_report(host::FromAgent::AccountRefused {
            owner: self.owner(),
            account: 7,
            code: Box::from(&b"forbidden"[..]),
            reason: Box::from(&b"account suspended"[..]),
        });
    }

    pub fn deliver(&mut self) {
        self.send(Event::Called {
            owner: self.owner.expect("assigned"),
            call: Box::from(Token::new(71).raw().to_be_bytes()),
            ask: Ask::Deliver { title: Box::from(&b"title"[..]), body: Box::from(&b"body"[..]) },
        });
    }

    pub fn relay(&mut self) {
        self.send(Event::Called {
            owner: self.owner.expect("assigned"),
            call: Box::from(Token::new(72).raw().to_be_bytes()),
            ask: crate::fixtures::relay(Box::from(&b"read"[..])),
        });
    }

    /// The agent calls an engine host tool under its opaque name.
    pub fn call(&mut self, writes: bool) {
        self.send(Event::Called {
            owner: self.owner.expect("assigned"),
            call: Box::from(&b"call-one"[..]),
            ask: Ask::Relay {
                tool: Box::from(&b"inspect"[..]),
                writes,
                input: Box::from(&b"input words"[..]),
                deadline: Duration::from_nanos(37),
            },
        });
    }

    /// The engine sends a named message with its label and words.
    pub fn message_from(&mut self) {
        self.message_with_finish(false);
    }

    pub fn message_with_finish(&mut self, finish_requested: bool) {
        self.send(Event::Down(LinkDown::Message {
            run: RUN,
            attempt: ATTEMPT,
            name: Token::new(17),
            sender: Box::from(&b"requester"[..]),
            words: Box::from(&b"please check"[..]),
            finish_requested,
        }));
    }

    pub fn finish(&mut self, turns: u32, parked: bool) {
        let finish = if parked { Finish::Parked } else { Finish::Ended { outcome: Box::from(&b"done"[..]) } };
        self.send(Event::Finished {
            read: self.last_read,
            owner: self.owner.expect("assigned"),
            turns,
            spent: u64::from(turns) * 17,
            finish,
        });
    }
    pub fn fail(&mut self, turns: u32, failure: host::RunFailure) {
        self.send(Event::Finished {
            read: self.last_read,
            owner: self.owner.expect("assigned"),
            turns,
            spent: u64::from(turns) * 17,
            finish: Finish::Failed { failure },
        });
    }

    /// A stop can be held to check that saving waits for the agent's exit.
    pub fn hold_stop(&mut self) {
        self.stop_waits = true;
    }
    pub fn hold_save(&mut self) {
        self.save_waits = true;
    }

    pub fn saved(&mut self) {
        assert!(self.save_pending);
        self.save_pending = false;
        self.send(Event::Saved { owner: self.owner(), at: Some(Token::new(8)) });
    }


    pub fn gone(&mut self) {
        assert!(self.stop_pending && self.agent_live);
        self.stop_pending = false;
        self.agent_live = false;
        self.send(Event::Gone { owner: self.owner.expect("assigned"), detail: Box::new([]) });
    }

    pub fn cancel_contact(&mut self) {
        self.send(Event::CancelAll { reason: Reason::Contact });
        self.resume();
    }

    pub fn shutdown(&mut self) {
        self.send(Event::CancelAll { reason: Reason::Shutdown });
        self.resume();
    }

    /// A channel loss does not drop host-owned turn bodies or the link's answer.
    pub fn lose_contact(&mut self) {
        self.connected = false;
    }

    /// Hello reports the run; the link then sends turns before the answer.
    pub fn reconnect(&mut self) {
        self.connected = true;
        self.send(crate::fixtures::opened());
        for index in 0..self.host.retained_turns() {
            let LinkUp::Turn { run, attempt, turn } = self.host.turn_at(index).expect("retained position") else {
                panic!("turn")
            };
            assert_eq!((run, attempt), (RUN, ATTEMPT));
            self.receive_turn(turn);
        }
        for index in 0..self.host.kept_answers() {
            let answer = self.host.answer_at(index).expect("kept answer");
            if let LinkUp::Answer { answer, .. } = answer {
                self.receive_answer(&answer);
            }
        }
        self.drain_facts();
    }

    /// The engine says busy; the link resends the same retained body after
    /// its own backoff, which is below this timer-free host.
    pub fn retry_busy(&mut self, number: u32) {
        let turn = self.host.turn(RUN, ATTEMPT, number).expect("busy turn retained");
        if self.connected {
            self.receive_turn(turn);
        }
    }

    /// Fake store commit, then its one acknowledgement.
    pub fn commit_turn(&mut self, number: u32) {
        assert!(self.seen.contains_key(&number), "only a received turn can commit");
        assert!(self.committed.insert(number), "one commit per turn");
        self.stats.commits += 1;
        assert!(self.acked.insert(number), "one ACK per committed turn");
        self.stats.turn_acks += 1;
        self.send(Event::Down(LinkDown::TurnKept { run: RUN, attempt: ATTEMPT, turn: number }));
    }

    pub fn acknowledge_answer(&mut self) {
        assert!(self.answer_seen.is_some(), "the engine received the answer");
        assert_eq!(self.committed.len(), self.seen.len(), "all turns commit before the answer");
        assert!(!self.answer_acked, "one answer ACK");
        self.answer_acked = true;
        self.stats.answer_acks += 1;
        self.send(Event::Down(LinkDown::Acknowledge { run: RUN, attempt: ATTEMPT }));
    }

    /// The external referee: one commit and ACK per observed turn, one
    /// answer and ACK, and no live parent capability after settlement.
    pub fn assert_settled(&self) {
        assert!(!self.agent_live && !self.space_live && !self.stop_pending);
        assert_eq!(self.seen.len(), self.committed.len());
        assert_eq!(self.committed, self.acked);
        assert!(self.answer_seen.is_some() && self.answer_acked);
        assert_eq!((self.stats.answers, self.stats.answer_acks), (1, 1));
        assert_eq!(self.stats.releases, u32::from(self.has_workspace));
    }

    fn send(&mut self, event: Event) {
        self.events.push_back(event);
        self.settle();
    }

    fn settle(&mut self) {
        for _ in 0..128 {
            let Some(event) = self.events.pop_front() else {
                self.host.reclaim();
                self.drain_facts();
                return;
            };
            self.refusals.event(&event);
            host::step(&mut self.host, &self.env, event, &mut self.out);
            while let Some(request) = self.out.pop() {
                self.route(request);
            }
            self.drain_facts();
        }
        panic!("the peer hand-offs did not settle");
    }

    fn resume(&mut self) {
        for _ in 0..self.limits.slots {
            if !self.host.is_ready() {
                return;
            }
            host::resume(&mut self.host, &self.env, &mut self.out);
            while let Some(request) = self.out.pop() {
                self.route(request);
            }
            self.settle();
        }
        assert!(!self.host.is_ready());
    }

    fn route(&mut self, request: Request) {
        self.refusals.request(&request);
        match request {
            Request::Source { .. } | Request::SourceFault => unreachable!("source scheduling has its own world"),
            Request::Up(
                record @ (LinkUp::Unauthorized { .. }
                | LinkUp::Limited { .. }
                | LinkUp::AccountRefused { .. }
                | LinkUp::Logins { .. }
                | LinkUp::SignedOut { .. }
                | LinkUp::Piece { .. }
                | LinkUp::Activity { .. }),
            ) => {
                self.receive_report(record);
            }
            Request::Up(LinkUp::Call { run, attempt, call, tool, writes, input, deadline }) => {
                assert_eq!((run, attempt), (RUN, ATTEMPT));
                self.answer_relay(RelayedCall { name: call, tool, writes, input, deadline });
            }
            Request::Reply { agent, call, reply } => {
                assert_eq!(agent, AGENT);

                self.reply = Some(reply);
                self.answered_call = Some(call);
                self.stats.replies += 1;
            }
            Request::Message { agent, name, sender, words, finish_requested } => {
                assert_eq!(agent, AGENT);
                self.received_message = Some(ReceivedMessage { finish_requested, name, sender, words });
            }
            Request::Start { owner, start } => {
                self.refusals.admitted(owner, RUN, ATTEMPT);
                self.start(owner, *start);
            }
            Request::Prepare { owner, .. } => {
                self.owner = Some(owner);
                self.space_live = true;
                self.refusals.admitted(owner, RUN, ATTEMPT);
                if !self.prepare_waits {
                    self.events.push_back(Event::Prepared { owner, workspace: SPACE });
                }
            }

            Request::Up(LinkUp::Turn { run, attempt, turn, .. }) => {
                assert_eq!((run, attempt), (RUN, ATTEMPT));
                if self.connected {
                    self.receive_turn(turn);
                }
            }
            Request::AcknowledgeAgentTurn { agent, .. } => {
                assert_eq!(agent, AGENT);
                self.stats.agent_acks += 1;
                self.reading = true;
            }

            Request::Deliver { owner, workspace, .. } => {
                assert!(self.agent_live && self.space_live && workspace == SPACE);
                self.stats.deliveries += 1;
                self.events.push_back(Event::Delivered {
                    owner,
                    delivery: Delivery { outcome: DeliveryOutcome::Delivered, left: Token::new(7), changed: true },
                });
            }

            Request::Stop { agent } => {
                assert_eq!(agent, AGENT);
                self.stats.stops += 1;
                self.reading = false;
                self.stop_pending = true;
                if !self.stop_waits {
                    self.stop_pending = false;
                    self.agent_live = false;
                    self.events.push_back(Event::Gone { owner: self.owner.expect("assigned"), detail: Box::new([]) });
                }
            }
            Request::Save { owner, workspace } => {
                assert!(!self.agent_live && self.space_live && workspace == SPACE);
                self.stats.saves += 1;
                if self.save_waits {
                    self.save_pending = true;
                } else {
                    self.events.push_back(Event::Saved { owner, at: Some(Token::new(8)) });
                }
            }
            Request::Release { workspace } => {
                assert!(!self.agent_live && workspace == SPACE && self.space_live);
                self.space_live = false;
                self.stats.releases += 1;
            }
            Request::Up(LinkUp::Answer { run, attempt, answer }) => {
                assert_eq!((run, attempt), (RUN, ATTEMPT));
                assert!(!self.agent_live && !self.space_live);
                assert!(self.answer.is_none(), "one answer");
                if self.connected {
                    self.receive_answer(&answer);
                }
                self.answer = Some(answer);
            }
            Request::Hello { hello } => {
                self.link_words.push("hello");
                assert_eq!(
                    hello.hosting.len(),
                    usize::from(self.agent_live || (self.answer.is_some() && !self.answer_acked))
                );
                self.hello = Some(hello);
            }
            Request::Grant { agent, grant } => self.receive_grant(agent, grant),
            Request::Abort { owner } => {
                self.space_live = false;
                self.events.push_back(Event::Unprepared {
                    owner, failure: host::Preparation::Transient, detail: Box::new([]),
                });
            }
            Request::CancelRelay { .. } => {
                panic!("unscripted request: {request:?}")
            }
        }
    }

    /// Answers a relayed call with the scripted answer, or a plain reply.
    fn answer_relay(&mut self, relayed: RelayedCall) {
        let call = relayed.name.clone();
        self.relayed_call = Some(relayed);
        self.stats.relays += 1;
        let answer =
            self.next_relay.take().unwrap_or(host::Relayed::Answer { error: false, text: Box::from(&b"reply"[..]) });
        self.events.push_back(Event::Down(LinkDown::Relayed { run: RUN, attempt: ATTEMPT, call, answer }));
    }

    fn receive_grant(&mut self, agent: Token, grant: host::Grant) {
        assert_eq!(agent, AGENT);
        assert!(self.agent_live);
        assert_eq!(grant.form, host::Form::Credential(host::Scheme::Key));
        self.received_grant = Some(grant);
    }

    fn start(&mut self, owner: Token, start: host::Start) {
        let host::Start { workspace, charter, activation, identity, turns, answered, messages, grants, live } = start;
        assert_eq!(workspace, if self.space_live { Some(SPACE) } else { None });
        assert_eq!(&*charter, b"charter");
        for grant in &grants {
            assert_eq!(grant.form, host::Form::Credential(host::Scheme::Key));
        }
        self.owner = Some(owner);
        self.agent_live = true;
        self.reading = true;
        self.started_state = Some(StartedState { live, identity, messages, activation, turns, answered });
        if !self.start_waits { self.events.push_back(Event::Started { owner, agent: AGENT }); }
    }

    fn receive_turn(&mut self, turn: Turn) {
        self.link_words.push("turn");
        if let Some(previous) = self.seen.get(&turn.turn) {
            assert_eq!(previous, &turn, "replay preserves the whole turn");
        } else {
            assert_eq!(turn.turn, u32::try_from(self.seen.len()).expect("bounded") + 1, "no turn gap");
            self.seen.insert(turn.turn, turn);
        }
        self.stats.transmissions += 1;
    }

    fn receive_report(&mut self, record: host::LinkUp) {
        if self.connected {
            if matches!(record, LinkUp::Piece { .. }) {
                self.link_words.push("piece");
            }
            self.reports.push(record);
        }
    }

    fn receive_answer(&mut self, answer: &Answer) {
        self.link_words.push("answer");
        assert_eq!(usize::try_from(answer.turns).expect("bounded"), self.seen.len(), "turns precede their answer");
        match &answer.ending {
            Ending::Ended { .. } | Ending::Parked { .. } | Ending::Failed { .. } | Ending::Refused(_) => {}
        }
        let copy = format!("{answer:?}");
        if let Some(previous) = &self.answer_seen {
            assert_eq!(previous, &copy, "answer replay is identical");
        } else {
            self.answer_seen = Some(copy);
            self.stats.answers += 1;
        }
    }

    fn drain_facts(&mut self) {
        if self.connected {
            while let Some(record) = self.host.pop_activity() {
                if !matches!(record, LinkUp::Activity { activity: host::Activity::Phase(_), .. }) {
                    self.stats.facts += 1;
                }
                self.receive_report(record);
            }
        }
        while let Some(fact) = self.host.pop_fact() {
            if matches!(fact, host::Fact::Failed { .. }) { self.stats.failures += 1; }
        }
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod referee_tests {
    use super::{Turn, World};

    fn turn(number: u32, body: &[u8]) -> Turn {
        Turn { opens_window: false, refused: Box::new([]), turn: number, spent: 17, read: None, body: Box::from(body) }
    }

    #[test]
    #[should_panic(expected = "replay preserves the whole turn")]
    fn changed_replay_is_rejected() {
        let mut world = World::new();
        world.receive_turn(turn(1, b"original"));
        world.receive_turn(turn(1, b"changed"));
    }

    #[test]
    #[should_panic(expected = "no turn gap")]
    fn unseen_prefix_is_rejected() {
        World::new().receive_turn(turn(2, b"gap"));
    }
}
