//! Worker channel ownership and routing (protocol/channel.md, sections 4 and 10.2).
use crate::{
    Event, Limits, Request, Worker,
    connection::{Connection, Ending, Phase},
    opening::Verdict,
};
use alloc::boxed::Box;
use jig_protocol_worker_wire::{self as wire, Reason};
use skein_channel::{Closed, Event as ChannelEvent, Request as ChannelRequest, Role};
use skein_lib::{Decimal, Map, Queue, Reader, Time, Token, bytes};
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Unusable {
    Limits,
    Workers,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Life {
    Running,
    Stopping,
    Closing,
    Aborting,
    Closed,
}
#[derive(Debug)]
pub struct Component {
    pub(crate) limits: Limits,
    pub(crate) workers: Box<[Worker]>,
    pub(crate) payload: u32,
    pub(crate) codec: wire::v9::Limits,
    pub(crate) connections: Map<Token, Connection>,
    schema: skein_channel::Schema,
    next_channel: u64,
    next_part: u64,
    life: Life,
    dropped_down: u64,
}
impl Component {
    pub fn new(limits: &Limits, workers: Box<[Worker]>, payload: u32) -> Result<Self, Unusable> {
        if !crate::valid(limits) {
            return Err(Unusable::Limits);
        }
        let codec = wire::sizes::limits(&limits.sizes).ok_or(Unusable::Limits)?;
        let schema = crate::limits::schema(limits).ok_or(Unusable::Limits)?;
        if workers.len() > usize::try_from(limits.names).expect("u32 fits usize") {
            return Err(Unusable::Workers);
        }
        for (index, worker) in workers.iter().enumerate() {
            if worker.name.len() > usize::try_from(limits.sizes.name).expect("u32 fits usize") {
                return Err(Unusable::Workers);
            }
            for earlier in workers.iter().take(index) {
                if earlier.name == worker.name {
                    return Err(Unusable::Workers);
                }
            }
        }
        let connections = Map::with_capacity(limits.workers.checked_add(limits.opening).expect("checked limits"));
        Ok(Self {
            limits: *limits,
            workers,
            payload,
            codec,
            connections,
            schema,
            next_channel: 1,
            next_part: 1,
            life: Life::Running,
            dropped_down: 0,
        })
    }
    pub fn down(&mut self, now: Time, event: Event, out: &mut Queue<Request>) {
        match event {
            Event::Accepted { connection } => self.accepted(now, connection, out),
            Event::Lower { connection, event } => {
                if let Some(mut state) = self.connections.remove(&connection) {
                    match &event {
                        skein_channel::LowerEvent::Read(skein_lib::stream::Up::Bytes(_))
                            if state.phase == Phase::Open =>
                        {
                            state.silence = Some(now.saturating_add(self.limits.silence));
                        }
                        skein_channel::LowerEvent::Read(_)
                        | skein_channel::LowerEvent::Write(_)
                        | skein_channel::LowerEvent::WriteFailed(_) => {}
                    }
                    state.up(event, connection, out);
                    self.progress(now, connection, &mut state, out);
                    self.keep(connection, state);
                }
            }
            Event::Gone { connection } => {
                if let Some(mut state) = self.connections.remove(&connection) {
                    lost(&mut state, out);
                }
            }
            Event::Stop => {
                if self.life == Life::Running {
                    self.life = Life::Stopping;
                }
            }
            Event::Close => {
                if self.life != Life::Closed && self.life != Life::Aborting {
                    self.life = Life::Closing;
                }
                self.fire(now, out);
            }
            Event::Abort => {
                if self.life != Life::Closed {
                    self.life = Life::Aborting;
                }
                self.fire(now, out);
            }
            Event::Down { channel, down, parts, values } => {
                if let Some(connection) = self.target(channel) {
                    let record = crate::translate::down(down, &parts, values, &self.codec);
                    self.send(now, connection, record, out);
                } else {
                    self.dropped_down = self.dropped_down.saturating_add(1);
                }
            }
            Event::TurnBusy { channel, run, attempt, turn } => {
                if let Some(connection) = self.target(channel) {
                    let record = crate::translate::turn_busy(run, attempt, turn, &self.codec);
                    self.send(now, connection, Some(record), out);
                } else {
                    self.dropped_down = self.dropped_down.saturating_add(1);
                }
            }
            Event::NotAdmitted { channel, text } => {
                if let Some(connection) = self.target(channel) {
                    let mut state = self.connections.remove(&connection).expect("heard channel has its connection");
                    self.refuse(now, connection, &mut state, Reason::NotAdmitted.code(), text, out);
                    self.progress(now, connection, &mut state, out);
                    self.keep(connection, state);
                }
            }
        }
        self.completed(out);
    }
    fn completed(&mut self, out: &mut Queue<Request>) {
        if (self.life == Life::Closing || self.life == Life::Aborting) && self.connections.is_empty() {
            self.life = Life::Closed;
            out.push(Request::Closed);
        }
    }
    fn target(&self, channel: Token) -> Option<Token> {
        for (connection, state) in &self.connections {
            if state.channel == channel && state.heard && state.phase == Phase::Open {
                return Some(*connection);
            }
        }
        None
    }
    fn send(&mut self, now: Time, connection: Token, record: Option<crate::translate::Down>, out: &mut Queue<Request>) {
        let Some(record) = record else {
            self.dropped_down = self.dropped_down.saturating_add(1);
            return;
        };
        let mut state = self.connections.remove(&connection).expect("heard channel exists");
        state.down(ChannelRequest::Send { token: state.channel, frame: record.frame() }, connection, out);
        self.progress(now, connection, &mut state, out);
        self.keep(connection, state);
    }
    fn accepted(&mut self, now: Time, connection: Token, out: &mut Queue<Request>) {
        if self.life == Life::Closed
            || self.connections.len() == self.connections.capacity()
            || self.connections.contains_key(&connection)
        {
            out.push(Request::TurnAway { connection });
            return;
        }
        let mut state = Connection::new(self.schema.clone(), &self.limits, now);
        out.push(Request::Take { connection });
        self.progress(now, connection, &mut state, out);
        self.keep(connection, state);
    }
    fn keep(&mut self, connection: Token, state: Connection) {
        let replaced = self.connections.insert(connection, state).expect("an accepted connection has a slot");
        assert!(replaced.is_none(), "one state per connection");
    }
    fn progress(&mut self, now: Time, connection: Token, state: &mut Connection, out: &mut Queue<Request>) {
        // Up produces one owner event; a decision and poll can produce Ready or Closed.
        for _ in 0_u32..3 {
            for _ in 0..skein_channel::MAX_UP {
                let Some(event) = state.above.pop() else { break };
                self.heard(now, connection, state, event, out);
            }
            state.poll(connection, out);
            if state.above.is_empty() {
                break;
            }
        }
        assert!(state.above.is_empty(), "channel decisions settle within three polls");
        if state.terminal {
            return;
        }
        if state.queued() {
            if state.stall.is_none() {
                state.stall = Some(now.saturating_add(self.limits.stall));
            }
        } else {
            state.stall = None;
            if state.ending == Ending::Graceful {
                state.down(ChannelRequest::Close, connection, out);
                for _ in 0..skein_channel::MAX_UP {
                    let Some(event) = state.above.pop() else { break };
                    self.heard(now, connection, state, event, out);
                }
            }
        }
    }
    fn heard(
        &mut self,
        now: Time,
        connection: Token,
        state: &mut Connection,
        event: ChannelEvent,
        out: &mut Queue<Request>,
    ) {
        match event {
            ChannelEvent::Opening { credential, .. } => {
                let verdict = self.judge(&credential);
                if self.life != Life::Running {
                    self.refuse(now, connection, state, Reason::Stopping.code(), Box::new([]), out);
                    return;
                }
                let worker = match verdict {
                    Verdict::Refuse(reason) => {
                        self.refuse(now, connection, state, reason.code(), Box::new([]), out);
                        return;
                    }
                    Verdict::Accept { worker } => worker,
                    Verdict::Replace { worker, older } => {
                        if let Some(mut old) = self.connections.remove(&older) {
                            self.refuse(now, older, &mut old, Reason::Replaced.code(), Box::new([]), out);
                            old.poll(older, out);
                            for _ in 0..skein_channel::MAX_UP {
                                let Some(event) = old.above.pop() else { break };
                                match event {
                                    ChannelEvent::Closed { why } => closed(older, &mut old, why, out),
                                    ChannelEvent::Opening { .. }
                                    | ChannelEvent::Ready { .. }
                                    | ChannelEvent::Body { .. }
                                    | ChannelEvent::Ping
                                    | ChannelEvent::Unsupported { .. }
                                    | ChannelEvent::Refused { .. }
                                    | ChannelEvent::Ended
                                    | ChannelEvent::Sent { .. }
                                    | ChannelEvent::Unsent { .. }
                                    | ChannelEvent::Drained
                                    | ChannelEvent::OutputFailed => {}
                                }
                            }
                            self.keep(older, old);
                        }
                        worker
                    }
                };
                state.worker = Some(worker);
                state.channel = Token::new(self.next_channel);
                self.next_channel =
                    self.next_channel.checked_add(1).expect("channel tokens never wrap in a process life");
                state.phase = Phase::Terms;
                state.hello_deadline = Some(now.saturating_add(self.limits.hello));
                state.down(ChannelRequest::Accept { version: wire::VERSION }, connection, out);
            }
            ChannelEvent::Ready { terms, .. } => {
                if let Some(kind) = wire::terms_gap(&self.schema, Role::Responder, &terms) {
                    let text = bytes::copy_of(Decimal::of(u64::from(kind)).as_bytes());
                    self.refuse(now, connection, state, 2, text, out);
                } else {
                    state.phase = Phase::Hello;
                    state.deadline = state.hello_deadline;
                    state.down(ChannelRequest::Read, connection, out);
                }
            }
            ChannelEvent::Closed { why } => closed(connection, state, why, out),
            ChannelEvent::Body { kind, body } => self.body(now, connection, state, kind, &body, out),
            ChannelEvent::Ping | ChannelEvent::Unsupported { .. } => {
                if state.phase == Phase::Open {
                    state.silence = Some(now.saturating_add(self.limits.silence));
                    state.down(ChannelRequest::Read, connection, out);
                } else if state.phase == Phase::Hello {
                    state.down(ChannelRequest::Read, connection, out);
                }
            }
            ChannelEvent::Ended
            | ChannelEvent::Refused { .. }
            | ChannelEvent::Unsent { why: skein_channel::Unsent::Full, .. }
            | ChannelEvent::OutputFailed => {
                lose(state, out);
                state.down(ChannelRequest::Close, connection, out);
            }
            ChannelEvent::Drained => {
                if state.phase == Phase::Open {
                    state.ping = Some(now.saturating_add(self.limits.ping));
                }
                state.stall = Some(now.saturating_add(self.limits.stall));
            }
            ChannelEvent::Sent { .. } | ChannelEvent::Unsent { .. } => {}
        }
    }
    fn body(
        &mut self,
        now: Time,
        connection: Token,
        state: &mut Connection,
        kind: u16,
        body: &[u8],
        out: &mut Queue<Request>,
    ) {
        match state.phase {
            Phase::Hello => {
                if kind == 0x0100
                    && let Ok(record) = wire::Hello::decode(&self.codec, &mut Reader::new(body))
                {
                    let Some(worker) = state.worker else {
                        self.refuse(now, connection, state, 3, Box::new([]), out);
                        return;
                    };
                    let Some(hello) = crate::translate::hello(record) else {
                        self.refuse(now, connection, state, 3, Box::new([]), out);
                        return;
                    };
                    state.phase = Phase::Open;
                    state.heard = true;
                    state.deadline = None;
                    state.ping = Some(now.saturating_add(self.limits.ping));
                    state.silence = Some(now.saturating_add(self.limits.silence));
                    out.push(Request::Hello { channel: state.channel, worker, hello });
                    state.down(ChannelRequest::Read, connection, out);
                    return;
                }
                self.refuse(now, connection, state, 3, Box::new([]), out);
            }
            Phase::Open => {
                state.silence = Some(now.saturating_add(self.limits.silence));
                if let Some(record) = crate::translate::decode(kind, body, &self.codec) {
                    let mut parts = crate::translate::Parts::new(self.next_part);
                    let up = crate::translate::up(record, &mut parts);
                    self.next_part = parts.next();
                    out.push(Request::Up { channel: state.channel, up, parts: parts.finish() });
                    state.down(ChannelRequest::Read, connection, out);
                } else {
                    self.refuse(now, connection, state, 3, Box::new([]), out);
                }
            }
            Phase::Closing => {}
            Phase::Opening | Phase::Terms => self.refuse(now, connection, state, 3, Box::new([]), out),
        }
    }
    fn refuse(
        &self,
        now: Time,
        connection: Token,
        state: &mut Connection,
        reason: u16,
        text: Box<[u8]>,
        out: &mut Queue<Request>,
    ) {
        lost(state, out);
        state.worker = None;
        state.phase = Phase::Closing;
        state.ping = None;
        state.silence = None;
        state.deadline = Some(now.saturating_add(self.limits.stall));
        state.down(ChannelRequest::Refuse { reason, text }, connection, out);
    }
    pub fn fire(&mut self, now: Time, out: &mut Queue<Request>) {
        let mut due = None;
        for (token, state) in &self.connections {
            if self.shutdown_pending(state) || expired(state.next_deadline(), now) {
                due = Some(*token);
                break;
            }
        }
        if let Some(connection) = due {
            let mut state = self.connections.remove(&connection).expect("due connection exists");
            if self.shutdown_pending(&state) {
                state.phase = Phase::Closing;
                state.ping = None;
                state.silence = None;
                state.deadline = None;
                state.ending = if self.life == Life::Aborting { Ending::Abort } else { Ending::Graceful };
                if self.life == Life::Aborting {
                    lost(&mut state, out);
                    state.down(ChannelRequest::Close, connection, out);
                } else {
                    state.down(ChannelRequest::Finish, connection, out);
                }
            } else if expired(state.deadline, now) || expired(state.silence, now) || expired(state.stall, now) {
                lost(&mut state, out);
                state.deadline = None;
                state.silence = None;
                state.stall = None;
                state.ping = None;
                state.worker = None;
                state.phase = Phase::Closing;
                state.down(ChannelRequest::Close, connection, out);
            } else {
                state.ping = Some(now.saturating_add(self.limits.ping));
                state.down(ChannelRequest::Ping, connection, out);
            }
            self.progress(now, connection, &mut state, out);
            self.keep(connection, state);
        }
        self.completed(out);
    }
    fn shutdown_pending(&self, state: &Connection) -> bool {
        (self.life == Life::Closing || self.life == Life::Aborting)
            && !state.terminal
            && ((state.ending == Ending::Active && (self.life == Life::Aborting || state.phase != Phase::Closing))
                || (self.life == Life::Aborting && state.ending == Ending::Graceful))
    }
    #[must_use]
    pub fn next_deadline(&self) -> Option<Time> {
        let mut next = None;
        for (_, state) in &self.connections {
            if self.shutdown_pending(state) {
                return Some(Time::ZERO);
            }
            if let Some(deadline) = state.next_deadline() {
                next = Some(match next {
                    Some(before) => deadline.min(before),
                    None => deadline,
                });
            }
        }
        next
    }
    /// Outputs dropped because their channel, parts or values are unavailable, or bounds do not fit.
    #[must_use]
    pub fn dropped_down(&self) -> u64 {
        self.dropped_down
    }
    pub fn reclaim(&mut self) {}
}

fn closed(connection: Token, state: &mut Connection, why: Closed, out: &mut Queue<Request>) {
    if state.terminal {
        return;
    }
    lost(state, out);
    state.terminal = true;
    state.worker = None;
    state.deadline = None;
    state.ping = None;
    state.silence = None;
    state.stall = None;
    state.phase = Phase::Closing;
    match why {
        Closed::Owner if state.ending == Ending::Graceful => out.push(Request::Close { connection }),
        Closed::RefusedHere(_) => out.push(Request::Close { connection }),
        Closed::Owner | Closed::RefusedPeer(_) | Closed::Framing | Closed::Stream | Closed::Truncated => {
            out.push(Request::Abort { connection });
        }
    }
}

fn lost(state: &mut Connection, out: &mut Queue<Request>) {
    if state.heard {
        state.heard = false;
        out.push(Request::Lost { channel: state.channel });
    }
}

fn expired(deadline: Option<Time>, now: Time) -> bool {
    match deadline {
        Some(at) => at <= now,
        None => false,
    }
}

fn lose(state: &mut Connection, out: &mut Queue<Request>) {
    lost(state, out);
    state.phase = Phase::Closing;
    state.ping = None;
    state.silence = None;
    state.stall = None;
    state.deadline = None;
}
