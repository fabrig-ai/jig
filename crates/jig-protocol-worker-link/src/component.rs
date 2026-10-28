//! Opening, loss and redial transitions (protocol/channel.md, 4 and 10.1).
use crate::{
    Event, Limits, REFUSED_FRAMING, REFUSED_LIMITS, Request,
    backoff::Backoff,
    busy::Busy,
    connection::{Connection, Phase},
    grace::Grace,
    limits,
    relays::Relays,
    translate,
};
use alloc::boxed::Box;
use core::mem;
use jig_domain_host as host;
use jig_protocol_worker_wire::{self as wire, sizes};
use skein_channel::{self as channel, Closed, Machine, Role};
use skein_lib::{Decimal, List, Queue, Secret, Time, Token, Writer, bytes};

#[derive(Debug)]
pub struct Credential {
    pub name: Box<[u8]>,
    pub secret: Secret,
    pub payload: u32,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Unusable {
    Limits,
    Credential,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Life {
    Active,
    Stopping,
    Aborting,
    Closed,
}
#[derive(Debug)]
pub struct Component {
    limits: Limits,
    credential: wire::Credential,
    phase: Phase,
    backoff: Backoff,
    events: Queue<channel::Event>,
    below: Queue<channel::Lower>,
    next_part: u64,
    dropped: u64,
    relays: Relays,
    busy: Busy,
    grace: Grace,
    life: Life,
    facts: u64,
    pending_logins: Option<channel::Frame>,
}
impl Component {
    pub fn new(limits: &Limits, credential: Credential, seed: u64) -> Result<Self, Unusable> {
        limits::worst_case(limits).ok_or(Unusable::Limits)?;
        let bodies = sizes::limits(&limits.sizes).ok_or(Unusable::Limits)?;
        let credential = wire::Credential::new(
            &bodies,
            wire::CredentialParts { name: credential.name, secret: credential.secret, payload: credential.payload },
        );
        let Ok(credential) = credential else { return Err(Unusable::Credential) };
        let backoff = Backoff::new(limits, seed);
        Ok(Self {
            limits: *limits,
            credential,
            phase: Phase::Idle { at: Time::ZERO },
            backoff,
            events: Queue::with_capacity(channel::MAX_UP.saturating_mul(3)),
            below: Queue::with_capacity(channel::MAX_DOWN.saturating_mul(3)),
            next_part: 1,
            dropped: 0,
            relays: Relays::new(limits),
            busy: Busy::new(limits, seed),
            grace: Grace::new(),
            life: Life::Active,
            facts: 0,
            pending_logins: None,
        })
    }
    pub fn down(&mut self, now: Time, event: Event, out: &mut Queue<Request>) {
        if self.life == Life::Closed {
            return;
        }
        match event {
            Event::SourceFault => self.lose(now, Closed::Owner, out),
            Event::Stop => self.end(now, false, out),
            Event::Abort => self.end(now, true, out),
            Event::CancelRelay { run, attempt, call } => {
                if self.relays.end(run, attempt, &call) {
                    out.push(Request::RelayCancelled { run, attempt, call });
                }
            }
            Event::Dialled { connection } => self.dialled(now, connection, out),
            Event::Unreachable => match self.phase {
                Phase::Connecting => {
                    if self.life != Life::Active {
                        self.completed(out);
                        return;
                    }
                    let span = self.backoff.next();
                    self.phase = Phase::Idle { at: now.saturating_add(span) };
                    out.push(Request::Fact { why: Closed::Stream });
                }
                Phase::Idle { .. } | Phase::Opening { .. } | Phase::Open { .. } | Phase::Closing { .. } => {}
            },
            Event::Lower { connection, event } => {
                if self.phase.token() == Some(connection) {
                    if let Some(link) = self.phase.connection() {
                        link.machine.up(event, &mut self.events, &mut self.below);
                    }
                    self.pump(now, out);
                }
            }
            Event::Gone { connection } => self.gone(now, connection, out),
            Event::Hello { hello } => {
                if self.life != Life::Active {
                    return;
                }
                self.hello(now, hello, out);
                self.pump(now, out);
            }
            Event::Up { up, parts } => {
                if self.life != Life::Active {
                    return;
                }
                self.up(up, &parts, out);
                self.pump(now, out);
            }
        }
    }
    pub fn fire(&mut self, now: Time, out: &mut Queue<Request>) {
        if self.life == Life::Closed {
            return;
        }
        if self.grace.fire(now) {
            out.push(Request::GracePassed);
        }
        if let Some((run, attempt, turn)) = self.busy.expire(now) {
            out.push(Request::Resend { run, attempt, turn });
        }
        match self.phase {
            Phase::Idle { at } => {
                if now >= at {
                    self.phase = Phase::Connecting;
                    out.push(Request::Dial);
                }
            }
            Phase::Opening { until, .. } => {
                if now >= until {
                    self.lose(now, Closed::Owner, out);
                    self.pump(now, out);
                }
            }
            Phase::Closing { until, .. } => {
                if let Some(until) = until
                    && now >= until
                {
                    if let Some(connection) = self.phase.token() {
                        out.push(Request::Abort { connection });
                    }
                    match &mut self.phase {
                        Phase::Closing { until, .. } => *until = None,
                        Phase::Idle { .. } | Phase::Connecting | Phase::Opening { .. } | Phase::Open { .. } => {}
                    }
                }
            }
            Phase::Open { proved, ref mut connection, .. } => {
                if expired(connection.silence, now) || expired(connection.stall, now) {
                    self.lose(now, Closed::Stream, out);
                    self.pump(now, out);
                    return;
                }
                if !connection.queued() && expired(connection.ping, now) {
                    connection.ping = Some(now.saturating_add(self.limits.ping));
                    self.command(channel::Request::Ping);
                }
                if let Some(at) = proved
                    && now >= at
                {
                    self.backoff.proved();
                    match &mut self.phase {
                        Phase::Open { proved, .. } => *proved = None,
                        Phase::Idle { .. } | Phase::Connecting | Phase::Opening { .. } | Phase::Closing { .. } => {}
                    }
                }
            }
            Phase::Connecting => {}
        }
        self.pump(now, out);
    }
    #[must_use]
    pub fn next_deadline(&self) -> Option<Time> {
        if self.life == Life::Closed {
            return None;
        }
        let phase = match self.phase {
            Phase::Idle { at } => Some(at),
            Phase::Opening { until, .. } => Some(until),
            Phase::Closing { until, .. } => until,
            Phase::Open { proved, ref connection, .. } => crate::connection::earlier(proved, connection.next()),
            Phase::Connecting => None,
        };
        crate::connection::earlier(crate::connection::earlier(phase, self.busy.next()), self.grace.next())
    }
    #[must_use]
    pub fn dropped_facts(&self) -> u64 {
        self.facts
    }

    /// Records dropped because their bounds or named parts do not fit.
    #[must_use]
    pub fn dropped_records(&self) -> u64 {
        self.dropped
    }

    fn dialled(&mut self, now: Time, token: Token, out: &mut Queue<Request>) {
        match self.phase {
            Phase::Connecting => {}
            Phase::Idle { .. } | Phase::Opening { .. } | Phase::Open { .. } | Phase::Closing { .. } => {
                out.push(Request::Abort { connection: token });
                return;
            }
        }
        let bodies = sizes::limits(&self.limits.sizes).expect("constructor checked sizes");
        let schema = wire::schema(&bodies).expect("constructor checked schema");
        let channel = limits::channel_limits(&self.limits).expect("constructor checked channel limits");
        let machine = Machine::new(schema, Role::Initiator, channel).expect("constructor checked channel");
        self.phase = Phase::Opening {
            connection: Connection { token, machine, ping: None, silence: None, stall: None },
            until: now.saturating_add(self.limits.handshake),
        };
        if self.life != Life::Active {
            self.close_ending(now);
            self.pump(now, out);
            return;
        }
        let mut writer = Writer::new(usize::try_from(self.credential.measure()).expect("checked credential fits"));
        self.credential.encode(&mut writer).expect("measured credential fits");
        self.command(channel::Request::Open { credential: writer.finish() });
        self.pump(now, out);
    }
    fn gone(&mut self, now: Time, token: Token, out: &mut Queue<Request>) {
        if self.phase.token() != Some(token) {
            return;
        }
        self.pending_logins = None;
        if self.life != Life::Active {
            self.completed(out);
            return;
        }
        let phase = mem::replace(&mut self.phase, Phase::Connecting);
        let at = match phase {
            Phase::Closing { retry, .. } => retry,
            Phase::Open { .. } => {
                self.busy.clear();
                self.grace.lost(now, self.limits.grace);
                out.push(Request::Lost);
                out.push(Request::Fact { why: Closed::Stream });
                let span = self.backoff.next();
                now.saturating_add(span)
            }
            Phase::Opening { .. } => {
                out.push(Request::Fact { why: Closed::Stream });
                let span = self.backoff.next();
                now.saturating_add(span)
            }
            Phase::Idle { at } => at,
            Phase::Connecting => now,
        };
        self.phase = Phase::Idle { at };
    }
    fn hello(&mut self, now: Time, value: host::Hello, out: &mut Queue<Request>) {
        match &mut self.phase {
            Phase::Open { hello, .. } => {
                if *hello {
                    return;
                }
                let limits = sizes::limits(&self.limits.sizes).expect("checked sizes");
                let Some(frame) = translate::hello(value, &limits) else {
                    self.dropped = self.dropped.saturating_add(1);
                    return;
                };
                *hello = true;
                self.command(channel::Request::Send { token: Token::new(0), frame });
                // Queue replay without polling lower demands: one hello step stays bounded.
                for index in 0..self.limits.sizes.slots.saturating_mul(self.limits.sizes.calls) {
                    let Some(frame) = self.relays.replay(index) else { break };
                    match self.phase {
                        Phase::Open { .. } => {}
                        Phase::Idle { .. } | Phase::Connecting | Phase::Opening { .. } | Phase::Closing { .. } => break,
                    }
                    self.command(channel::Request::Send { token: Token::new(1), frame });
                    for _ in 0..channel::MAX_UP {
                        let Some(event) = self.events.pop() else { break };
                        self.received(now, event, out);
                    }
                }
            }
            Phase::Idle { .. } | Phase::Connecting | Phase::Opening { .. } | Phase::Closing { .. } => {}
        }
    }
    fn up(&mut self, up: host::LinkUp, parts: &[wire::Part], out: &mut Queue<Request>) {
        let limits = sizes::limits(&self.limits.sizes).expect("checked sizes");
        let up = match up {
            host::LinkUp::Call { run, attempt, call, tool, writes, input, deadline } => {
                if input.len() > usize::try_from(limits.call_input).expect("call bound fits") {
                    self.refuse_call(run, attempt, call, b"call too large", out);
                    return;
                }
                let name = bytes::copy_of(&call);
                let up = host::LinkUp::Call { run, attempt, call, tool, writes, input, deadline };
                let Some(frame) = translate::up(up, parts, &limits) else {
                    self.dropped = self.dropped.saturating_add(1);
                    self.refuse_call(run, attempt, name, b"call refused", out);
                    return;
                };
                if let Err(call) = self.relays.add(run, attempt, name, &frame) {
                    out.push(Request::Down {
                        down: host::LinkDown::Relayed { run, attempt, call, answer: host::Relayed::Busy },
                        parts: Box::new([]),
                    });
                    return;
                }
                if let Phase::Open { hello: true, .. } = self.phase {
                    self.command(channel::Request::Send { token: Token::new(1), frame });
                }
                return;
            }
            up @ (host::LinkUp::Logins { .. }
            | host::LinkUp::SignedOut { .. }
            | host::LinkUp::Piece { .. }
            | host::LinkUp::Activity { .. }
            | host::LinkUp::Unauthorized { .. }
            | host::LinkUp::Limited { .. }
            | host::LinkUp::AccountRefused { .. }
            | host::LinkUp::Turn { .. }
            | host::LinkUp::Answer { .. }) => up,
        };
        match self.phase {
            Phase::Open { hello: true, .. } => {
                let Some(frame) = translate::up(up, parts, &limits) else {
                    self.dropped = self.dropped.saturating_add(1);
                    return;
                };
                if frame.kind() == wire::Kind::Logins.number() {
                    self.pending_logins = Some(frame);
                    return;
                }
                if frame.kind() == 0x0104 && !self.fact_fits(&frame) {
                    self.facts = self.facts.saturating_add(1);
                    return;
                }
                self.command(channel::Request::Send { token: Token::new(1), frame });
            }
            Phase::Idle { .. }
            | Phase::Connecting
            | Phase::Opening { .. }
            | Phase::Open { hello: false, .. }
            | Phase::Closing { .. } => {}
        }
    }
    fn refuse_call(&self, run: Token, attempt: Token, call: Box<[u8]>, reason: &[u8], out: &mut Queue<Request>) {
        let bound = usize::try_from(self.limits.sizes.answer).expect("answer bound fits");
        let text = bytes::copy_of(reason.get(..reason.len().min(bound)).expect("bounded ASCII reason"));
        out.push(Request::Down {
            down: host::LinkDown::Relayed { run, attempt, call, answer: host::Relayed::Answer { error: true, text } },
            parts: Box::new([]),
        });
    }
    fn command(&mut self, request: channel::Request) {
        if let Some(link) = self.phase.connection() {
            link.machine.down(request, &mut self.events, &mut self.below);
        }
    }
    fn pump(&mut self, now: Time, out: &mut Queue<Request>) {
        self.send_logins();
        if let Some(link) = self.phase.connection() {
            link.machine.poll(&mut self.events, &mut self.below);
        }
        // Each channel operation emits at most MAX_UP, and handlers perform at most two.
        for _ in 0..channel::MAX_UP.saturating_mul(3) {
            self.drain_lower(now, out);
            let Some(event) = self.events.pop() else { break };
            self.received(now, event, out);
            if let Some(link) = self.phase.connection() {
                link.machine.poll(&mut self.events, &mut self.below);
            }
        }
        self.drain_lower(now, out);
        assert!(self.events.is_empty(), "one channel event cascade fits its bound");
        if let Some(connection) = self.phase.connection() {
            if connection.queued() {
                if connection.stall.is_none() {
                    connection.stall = Some(now.saturating_add(self.limits.stall));
                }
            } else {
                connection.stall = None;
            }
        }
        if self.life == Life::Stopping {
            match &mut self.phase {
                Phase::Open { connection, .. } if !connection.queued() => {
                    self.close_ending(now);
                    self.pump(now, out);
                }
                Phase::Idle { .. }
                | Phase::Connecting
                | Phase::Opening { .. }
                | Phase::Open { .. }
                | Phase::Closing { .. } => {}
            }
        }
    }
    fn drain_lower(&mut self, now: Time, out: &mut Queue<Request>) {
        for _ in 0..channel::MAX_DOWN.saturating_mul(3) {
            let Some(lower) = self.below.pop() else { break };
            match &lower {
                channel::Lower::Write(skein_lib::stream::OutputDown::Send { bytes, .. }) => {
                    let header = channel::parse_header(bytes.get(..8).expect("a whole output frame"))
                        .expect("checked frame header");
                    if header.kind == 0x0100 {
                        match &mut self.phase {
                            Phase::Open { proved, .. } => *proved = Some(now.saturating_add(self.limits.proved)),
                            Phase::Idle { .. } | Phase::Connecting | Phase::Opening { .. } | Phase::Closing { .. } => {}
                        }
                    }
                }
                channel::Lower::Read(_)
                | channel::Lower::Write(
                    skein_lib::stream::OutputDown::Room { .. }
                    | skein_lib::stream::OutputDown::Cancel { .. }
                    | skein_lib::stream::OutputDown::Release { .. },
                )
                | channel::Lower::FinishWrite => {}
            }
            if let Some(connection) = self.phase.token() {
                out.push(Request::Lower { connection, lower });
            }
        }
    }
    fn received(&mut self, now: Time, event: channel::Event, out: &mut Queue<Request>) {
        match &event {
            channel::Event::Body { .. } | channel::Event::Ping | channel::Event::Unsupported { .. } => {
                if self.life == Life::Active
                    && let Some(connection) = self.phase.connection()
                {
                    connection.silence = Some(now.saturating_add(self.limits.silence));
                }
            }
            channel::Event::Ready { .. }
            | channel::Event::Refused { .. }
            | channel::Event::Closed { .. }
            | channel::Event::Ended
            | channel::Event::OutputFailed
            | channel::Event::Unsent { .. }
            | channel::Event::Opening { .. }
            | channel::Event::Drained
            | channel::Event::Sent { .. } => {}
        }
        match event {
            channel::Event::Ready { terms, .. } => {
                let bodies = sizes::limits(&self.limits.sizes).expect("checked sizes");
                let schema = wire::schema(&bodies).expect("checked schema");
                if let Some(kind) = wire::terms_gap(&schema, Role::Initiator, &terms) {
                    self.command(channel::Request::Refuse {
                        reason: REFUSED_LIMITS,
                        text: bytes::copy_of(Decimal::of(u64::from(kind)).as_bytes()),
                    });
                    let started = self.begin_loss(now, Closed::RefusedHere(REFUSED_LIMITS), out);
                    assert!(started, "terms are checked during opening");
                } else {
                    let phase = mem::replace(&mut self.phase, Phase::Connecting);
                    self.phase = match phase {
                        Phase::Opening { connection, .. } => Phase::Open { connection, proved: None, hello: false },
                        other @ (Phase::Idle { .. }
                        | Phase::Connecting
                        | Phase::Open { .. }
                        | Phase::Closing { .. }) => other,
                    };
                    self.grace.clear();
                    if let Some(connection) = self.phase.connection() {
                        connection.live(now, &self.limits);
                    }
                    out.push(Request::Ready);
                    self.command(channel::Request::Read);
                }
            }
            channel::Event::Refused { reason, .. } => self.lose(now, Closed::RefusedPeer(reason), out),
            channel::Event::Closed { why } => {
                if self.life != Life::Active {
                    if let Some(connection) = self.phase.token() {
                        if self.life == Life::Aborting {
                            out.push(Request::Abort { connection });
                        } else {
                            out.push(Request::Close { connection });
                        }
                    }
                    return;
                }
                let started = self.begin_loss(now, why, out);
                if started {
                    self.command(channel::Request::Close);
                }
                if let Some(connection) = self.phase.token() {
                    out.push(Request::Close { connection });
                }
            }
            channel::Event::Ended | channel::Event::OutputFailed => self.lose(now, Closed::Stream, out),
            channel::Event::Body { kind, body } => self.body(now, kind, &body, out),
            channel::Event::Ping | channel::Event::Unsupported { .. } => {
                self.command(channel::Request::Read);
            }
            channel::Event::Unsent { .. } => self.lose(now, Closed::Owner, out),
            channel::Event::Opening { .. } => self.lose(now, Closed::Framing, out),
            channel::Event::Drained => {
                self.send_logins();
                if let Some(connection) = self.phase.connection() {
                    if self.life == Life::Active {
                        connection.ping = Some(now.saturating_add(self.limits.ping));
                    }
                    connection.stall = Some(now.saturating_add(self.limits.stall));
                }
            }
            channel::Event::Sent { .. } => {}
        }
    }
    fn body(&mut self, now: Time, kind: u16, body: &[u8], out: &mut Queue<Request>) {
        let limits = sizes::limits(&self.limits.sizes).expect("checked sizes");
        if kind == 0x0184 {
            let record = wire::TurnBusy::decode(&limits, &mut skein_lib::Reader::new(body));
            match record {
                Ok(record)
                    if self.busy.add(now, Token::new(record.task()), Token::new(record.attempt()), record.number()) =>
                {
                    self.command(channel::Request::Read);
                }
                Ok(_) => self.lose(now, Closed::Owner, out),
                Err(_) => {
                    self.command(channel::Request::Refuse { reason: REFUSED_FRAMING, text: Box::new([]) });
                    self.begin_loss(now, Closed::RefusedHere(REFUSED_FRAMING), out);
                }
            }
            return;
        }
        let mut parts = List::with_capacity(1);
        match translate::down(kind, body, &limits, now, &mut parts, &mut self.next_part, out) {
            Some(down) => {
                let forward = match &down {
                    host::LinkDown::Relayed { run, attempt, call, .. } => self.relays.end(*run, *attempt, call),
                    host::LinkDown::TurnKept { run, attempt, turn } => {
                        self.busy.kept(*run, *attempt, *turn);
                        true
                    }
                    host::LinkDown::Acknowledge { run, attempt } => {
                        self.busy.acknowledged(*run, *attempt);
                        true
                    }
                    host::LinkDown::Sources { .. }
                    | host::LinkDown::Check { .. }
                    | host::LinkDown::SignOut { .. }
                    | host::LinkDown::Assign { .. }
                    | host::LinkDown::Message { .. }
                    | host::LinkDown::Cancel { .. }
                    | host::LinkDown::Park { .. }
                    | host::LinkDown::Grant { .. } => true,
                };
                if forward {
                    out.push(Request::Down { down, parts: parts.into_boxed() });
                }
                self.command(channel::Request::Read);
            }
            None => {
                self.command(channel::Request::Refuse { reason: REFUSED_FRAMING, text: Box::new([]) });
                let started = self.begin_loss(now, Closed::RefusedHere(REFUSED_FRAMING), out);
                assert!(started, "a body arrives on an open channel");
            }
        }
    }
    fn send_logins(&mut self) {
        let Some(connection) = self.phase.connection() else { return };
        if connection.queued() {
            return;
        }
        let Some(frame) = self.pending_logins.take() else { return };
        self.command(channel::Request::Send { token: Token::new(1), frame });
    }
    fn fact_fits(&mut self, frame: &channel::Frame) -> bool {
        let cap = sizes::worker_cap(&self.limits.sizes).expect("checked cap");
        let rest = cap.cap.bytes.checked_sub(cap.facts).expect("facts included in cap");
        let frames = cap.cap.frames.checked_sub(cap.facts.div_euclid(25)).expect("facts included in cap");
        let Some(connection) = self.phase.connection() else { return false };
        let room = connection.machine.room();
        room.bytes.saturating_sub(rest) >= frame.wire_len() && room.frames > frames
    }
    fn end(&mut self, now: Time, abort: bool, out: &mut Queue<Request>) {
        self.pending_logins = None;
        if self.life == Life::Closed {
            return;
        }
        if self.life == Life::Aborting || (self.life == Life::Stopping && !abort) {
            return;
        }
        self.life = if abort { Life::Aborting } else { Life::Stopping };
        self.busy.clear();
        self.grace.clear();
        if let Some(connection) = self.phase.connection() {
            connection.quiet();
        }
        match &mut self.phase {
            Phase::Open { proved, .. } => *proved = None,
            Phase::Idle { .. } | Phase::Connecting | Phase::Opening { .. } | Phase::Closing { .. } => {}
        }
        match self.phase {
            Phase::Idle { .. } => self.completed(out),
            Phase::Connecting => {}
            Phase::Open { .. } if !abort => self.command(channel::Request::Finish),
            Phase::Opening { .. } | Phase::Open { .. } | Phase::Closing { .. } => self.close_ending(now),
        }
        self.pump(now, out);
    }
    fn close_ending(&mut self, now: Time) {
        let phase = mem::replace(&mut self.phase, Phase::Connecting);
        self.phase = match phase {
            Phase::Opening { connection, .. } | Phase::Open { connection, .. } | Phase::Closing { connection, .. } => {
                Phase::Closing { connection, retry: Time::ZERO, until: Some(now.saturating_add(self.limits.stall)) }
            }
            other @ (Phase::Idle { .. } | Phase::Connecting) => other,
        };
        self.command(channel::Request::Close);
    }
    fn completed(&mut self, out: &mut Queue<Request>) {
        self.phase = Phase::Idle { at: Time::ZERO };
        self.life = Life::Closed;
        self.grace.clear();
        self.busy.clear();
        out.push(Request::Closed);
    }
    fn lose(&mut self, now: Time, why: Closed, out: &mut Queue<Request>) {
        if self.life != Life::Active {
            self.life = Life::Aborting;
            self.close_ending(now);
            return;
        }
        if self.begin_loss(now, why, out) {
            self.command(channel::Request::Close);
        }
    }
    fn begin_loss(&mut self, now: Time, why: Closed, out: &mut Queue<Request>) -> bool {
        self.pending_logins = None;
        let phase = mem::replace(&mut self.phase, Phase::Connecting);
        let mut started = false;
        self.phase = match phase {
            Phase::Open { mut connection, .. } => {
                self.busy.clear();
                self.grace.lost(now, self.limits.grace);
                connection.quiet();
                connection.stall = None;
                out.push(Request::Lost);
                let span = self.backoff.next();
                out.push(Request::Fact { why });
                started = true;
                Phase::Closing {
                    connection,
                    retry: now.saturating_add(span),
                    until: Some(now.saturating_add(self.limits.handshake)),
                }
            }
            Phase::Opening { connection, .. } => {
                let span = self.backoff.next();
                out.push(Request::Fact { why });
                started = true;
                Phase::Closing {
                    connection,
                    retry: now.saturating_add(span),
                    until: Some(now.saturating_add(self.limits.handshake)),
                }
            }
            other @ (Phase::Idle { .. } | Phase::Connecting | Phase::Closing { .. }) => other,
        };
        started
    }
}

fn expired(at: Option<Time>, now: Time) -> bool {
    match at {
        Some(at) => now >= at,
        None => false,
    }
}
