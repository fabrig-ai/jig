//! io-free HTTP machines, with transport-owned exchange deadlines (protocol/client.md, 4).
use crate::diagnostics::Kind;
use jig_client as client;
use jig_client_protocol_http::{self as mapping, Call, Mapping, Unlinked};
use skein_http::{Header, client as http};
use skein_lib::{
    Duration, Env, Queue, Secret, Time, Token, Wall,
    stream::{self, Down, Read, Up},
};
use std::collections::{BTreeMap, VecDeque};
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Conn(pub u32);
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub exchanges: u32,
    pub body_bytes: u32,
    pub deadline: Duration,
    pub http: http::Limits,
}
/// Whether these limits are representable: every edge bound they size
/// is finite and every capacity they derive fits (programming-model.md, 6.3).
#[must_use]
pub fn valid(limits: &Limits) -> bool {
    let Some(record) = limits.body_bytes.checked_add(512) else {
        return false;
    };
    limits.exchanges > 0
        && limits.body_bytes > 0
        && limits.deadline != Duration::ZERO
        // skein-http's client sum has these conditions; its remaining terms are heap prices.
        && limits.http.head >= 2
        && limits.http.read != 0
        && limits.http.send != 0
        && limits.http.read.checked_mul(3).and_then(|v| v.checked_add(8)).is_some()
        && mapping::valid(&mapping::Limits { record, chunk: limits.http.read })
}

#[derive(Clone, Copy, Debug)]
enum Owner {
    Link(Token),
    Stream,
}
type Pending = (Owner, Call, Time, Kind);
struct Exchange {
    owner: Owner,
    kind: Kind,
    until: Time,
    stream: Option<mapping::Stream>,
    machine: http::Client,
    events: Queue<http::Event>,
    below: Queue<Down>,
    upload: Box<[u8]>,
    sent: usize,
    body: Vec<u8>,
    status: u16,
    remaining: Option<u64>,
    terminal: bool,
}
pub struct Connections {
    limits: Limits,
    mapping: Mapping,
    heard: Queue<mapping::Heard>,
    host: String,
    next: u32,
    exchanges: BTreeMap<Conn, Exchange>,
    pending: VecDeque<Pending>,
    events: VecDeque<client::Event>,
}
impl Connections {
    #[must_use]
    pub fn new(limits: &Limits, host: &str) -> Self {
        assert!(valid(limits), "usable transport limits");
        Self {
            limits: *limits,
            mapping: Mapping::new(b"/jig").expect("default mapping"),
            heard: Queue::uncapped(),
            host: host.to_owned(),
            next: 1,
            exchanges: BTreeMap::new(),
            pending: VecDeque::new(),
            events: VecDeque::new(),
        }
    }
    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }
    /// # Errors
    /// Refuses any link outside the printed HTTP link's form.
    pub fn printed(&mut self, link: &[u8]) -> Result<Secret, Unlinked> {
        let printed = mapping::printed(&self.mapping, link)?;
        let host = String::from_utf8(printed.host.into_vec()).map_err(|_| Unlinked::Address)?;
        if self.host != host {
            return Err(Unlinked::Address);
        }
        self.host = host;
        Ok(printed.secret)
    }
    fn name(&mut self) -> Conn {
        let conn = Conn(self.next);
        self.next = self.next.checked_add(1).expect("finite connection namespace");
        conn
    }
    pub(crate) fn has_exchange_room(&self) -> bool {
        self.pending.iter().filter(|(owner, _, _, _)| matches!(owner, Owner::Link(_))).count()
            + self.exchanges.values().filter(|value| !matches!(value.owner, Owner::Stream)).count()
            < usize::try_from(self.limits.exchanges).expect("u32 fits")
    }
    pub(crate) fn kinds(&self) -> (skein_lib::List<Kind>, skein_lib::List<Kind>) {
        let mut active =
            skein_lib::List::with_capacity(u32::try_from(self.exchanges.len()).expect("bounded HTTP calls"));
        for value in self.exchanges.values() {
            active.push(value.kind).expect("one kind per active HTTP call");
        }
        let mut queued = skein_lib::List::with_capacity(u32::try_from(self.pending.len()).expect("bounded HTTP queue"));
        for (_, _, _, kind) in &self.pending {
            queued.push(*kind).expect("one kind per queued HTTP call");
        }
        (active, queued)
    }
    fn enqueue(&mut self, now: Time, owner: Owner, call: Call, kind: Kind) {
        if self.has_exchange_room() {
            self.pending.push_back((owner, call, now.saturating_add(self.limits.deadline), kind));
        } else {
            self.failed(owner);
        }
    }
    pub fn down(&mut self, now: Time, request: client::Request, below: &mut Queue<(Conn, stream::Down)>) {
        self.queue(now, request, below);
        self.flush(now, u32::MAX, below);
    }
    /// Queues a request without starting its exchange; `take_with_room` starts it.
    pub(crate) fn queue(&mut self, now: Time, request: client::Request, below: &mut Queue<(Conn, Down)>) {
        match request {
            client::Request::Exchange { exchange, message, credential } => {
                let kind = Kind::message(&message);
                self.enqueue(
                    now,
                    Owner::Link(exchange),
                    mapping::exchange(&self.mapping, credential.as_ref(), message),
                    kind,
                );
            }
            client::Request::OpenStream { credential } => {
                if !self.exchanges.values().any(|value| matches!(value.owner, Owner::Stream))
                    && !self.pending.iter().any(|(owner, _, _, _)| matches!(owner, Owner::Stream))
                {
                    self.pending.push_back((
                        Owner::Stream,
                        mapping::stream(&self.mapping, credential.as_ref()),
                        now,
                        Kind::Stream,
                    ));
                }
            }
            client::Request::CloseStream => self.close(now, None, below),
            client::Request::Save { .. }
            | client::Request::Erase { .. }
            | client::Request::Told { .. }
            | client::Request::Authorized(_)
            | client::Request::ServiceCredential { .. } => {}
            client::Request::Down { .. } | client::Request::SignOut => panic!("linked hosts emit wire work"),
        }
    }
    pub fn take(&mut self, now: Time, to_link: &mut Queue<client::Event>, below: &mut Queue<(Conn, Down)>) {
        self.take_with_room(now, u32::MAX, to_link, below);
    }
    /// Whether a call waits for a free socket.
    #[must_use]
    pub(crate) fn waiting(&self) -> bool {
        !self.pending.is_empty()
    }
    /// Expires, then starts at most `free` waiting calls, then hands the link its events.
    pub(crate) fn take_with_room(
        &mut self,
        now: Time,
        free: u32,
        to_link: &mut Queue<client::Event>,
        below: &mut Queue<(Conn, Down)>,
    ) {
        self.expire(now, below);
        self.flush(now, free, below);
        while let Some(event) = self.events.pop_front() {
            to_link.push(event);
        }
    }
    fn flush(&mut self, now: Time, free: u32, below: &mut Queue<(Conn, Down)>) {
        for _ in 0..free {
            let Some((owner, call, until, kind)) = self.pending.pop_front() else { break };
            self.start(now, owner, call, until, kind, below);
        }
    }
    fn stream_limits(&self) -> mapping::Limits {
        mapping::Limits {
            record: self.limits.body_bytes.checked_add(512).expect("bounded stream record"),
            chunk: self.limits.http.read,
        }
    }
    fn env(&self, now: Time) -> Env<http::Limits> {
        Env { now, wall: Wall::EPOCH, limits: self.limits.http }
    }
    fn start(&mut self, now: Time, owner: Owner, call: Call, until: Time, kind: Kind, below: &mut Queue<(Conn, Down)>) {
        if call.body.len() > usize::try_from(self.limits.body_bytes).expect("u32 fits") {
            self.failed(owner);
            return;
        }
        let conn = self.name();
        let length = call.body.len();
        let mut headers = vec![Header { name: b"Host".as_slice().into(), value: self.host.as_bytes().into() }];
        headers.extend(call.headers.into_boxed());
        let mut value = Exchange {
            owner,
            kind,
            until,
            stream: if matches!(owner, Owner::Stream) {
                Some(mapping::Stream::new(&self.stream_limits()))
            } else {
                None
            },
            machine: http::Client::new(&self.limits.http),
            events: Queue::uncapped(),
            below: Queue::uncapped(),
            upload: call.body,
            sent: 0,
            body: Vec::new(),
            status: 0,
            remaining: None,
            terminal: false,
        };
        let env = self.env(now);
        http::down(
            &mut value.machine,
            &env,
            http::Request::Call(http::Call {
                method: call.method,
                target: call.path,
                headers: headers.into_boxed_slice(),
                body: if matches!(owner, Owner::Stream) {
                    http::Body::None
                } else {
                    http::Body::Length(u64::try_from(length).expect("body fits"))
                },
                close: false,
            }),
            &mut value.events,
            &mut value.below,
        );
        if length > 0 {
            Self::upload(&env, &mut value);
        } else if !matches!(owner, Owner::Stream) {
            http::down(
                &mut value.machine,
                &env,
                http::Request::Upload(Down::Finish),
                &mut value.events,
                &mut value.below,
            );
        }
        self.exchanges.insert(conn, value);
        self.pump(now, conn, below);
    }
    fn upload(env: &Env<http::Limits>, value: &mut Exchange) {
        let room = u32::try_from(value.upload.len() - value.sent).expect("bounded upload").min(env.limits.send);
        http::down(
            &mut value.machine,
            env,
            http::Request::Upload(Down::Demand { read: Read::Nothing, room }),
            &mut value.events,
            &mut value.below,
        );
    }
    pub fn up(
        &mut self,
        now: Time,
        conn: Conn,
        up: Up,
        to_link: &mut Queue<client::Event>,
        below: &mut Queue<(Conn, Down)>,
    ) {
        self.receive(now, conn, up, below);
        self.take(now, to_link, below);
    }
    /// Feeds one socket's answer without starting waiting calls; `take_with_room` starts them.
    pub(crate) fn receive(&mut self, now: Time, conn: Conn, up: Up, below: &mut Queue<(Conn, Down)>) {
        self.expire(now, below);
        let env = self.env(now);
        if let Some(value) = self.exchanges.get_mut(&conn) {
            http::up(&mut value.machine, &env, up, &mut value.events, &mut value.below);
        }
        self.pump(now, conn, below);
    }
    fn pump(&mut self, now: Time, conn: Conn, below: &mut Queue<(Conn, Down)>) {
        let env = self.env(now);
        let Some(mut value) = self.exchanges.remove(&conn) else {
            return;
        };
        while let Some(event) = value.events.pop() {
            match event {
                http::Event::Upload(Up::Room) => Self::send_upload(&env, &mut value),
                http::Event::Response(response) => {
                    value.status = response.status;
                    value.remaining =
                        if let http::Framing::Length(length) = response.framing { Some(length) } else { None };
                    if let Some(stream) = &mut value.stream {
                        stream.opened(value.status, &mut self.heard);
                        value.terminal = self.hear();
                    }
                    if !value.terminal {
                        Self::read(&env, &mut value);
                    }
                }
                http::Event::Body(Up::Bytes(bytes)) => {
                    if let Some(remaining) = &mut value.remaining {
                        *remaining -= u64::try_from(bytes.len()).expect("bounded read");
                    }
                    if let Some(stream) = &mut value.stream {
                        stream.body(&bytes, &mut self.heard);
                        value.terminal = self.hear();
                    } else if value.body.len() + bytes.len()
                        <= usize::try_from(self.limits.body_bytes).expect("u32 fits")
                    {
                        value.body.extend_from_slice(&bytes);
                    } else {
                        self.failed(value.owner);
                        value.terminal = true;
                    }
                    if !value.terminal {
                        Self::read(&env, &mut value);
                    }
                }
                http::Event::Done(_) => {
                    match value.owner {
                        Owner::Link(exchange) => {
                            if !value.terminal {
                                let heard = mapping::answered(
                                    exchange,
                                    value.status,
                                    std::mem::take(&mut value.body).into_boxed_slice(),
                                );
                                self.heard.push(heard);
                                self.hear();
                            }
                        }
                        Owner::Stream => {
                            value.stream.as_mut().expect("stream owner").ended(&mut self.heard);
                            self.hear();
                        }
                    }
                    value.terminal = true;
                }
                http::Event::Failed(_) => {
                    if !value.terminal {
                        self.failed(value.owner);
                    }
                    value.terminal = true;
                }
                http::Event::Body(Up::End | Up::Failed(_))
                | http::Event::Upload(Up::Failed(_))
                | http::Event::Closed => {}
                http::Event::Body(Up::Room) | http::Event::Upload(Up::End | Up::Bytes(_)) => {
                    unreachable!("HTTP body faces follow their direction")
                }
            }
        }
        while let Some(down) = value.below.pop() {
            below.push((conn, down));
        }
        if value.terminal {
            http::down(&mut value.machine, &env, http::Request::Close, &mut value.events, &mut value.below);
            while let Some(down) = value.below.pop() {
                below.push((conn, down));
            }
            below.push((conn, Down::Finish));
        } else {
            self.exchanges.insert(conn, value);
        }
    }
    fn send_upload(env: &Env<http::Limits>, value: &mut Exchange) {
        let end = (value.sent + usize::try_from(env.limits.send).expect("u32 fits")).min(value.upload.len());
        let bytes = value.upload[value.sent..end].into();
        value.sent = end;
        http::down(
            &mut value.machine,
            env,
            http::Request::Upload(Down::Send(bytes)),
            &mut value.events,
            &mut value.below,
        );
        if end == value.upload.len() {
            http::down(
                &mut value.machine,
                env,
                http::Request::Upload(Down::Finish),
                &mut value.events,
                &mut value.below,
            );
            value.upload = Box::new([]);
        } else {
            Self::upload(env, value);
        }
    }
    fn read(env: &Env<http::Limits>, value: &mut Exchange) {
        let read = if matches!(value.owner, Owner::Stream) {
            Read::Line { max: env.limits.read.min(1024) }
        } else {
            match value.remaining {
                Some(n) => Read::Fill(u32::try_from(n.max(1).min(u64::from(env.limits.read))).expect("bounded read")),
                None => Read::Arrived { max: env.limits.read },
            }
        };
        http::down(
            &mut value.machine,
            env,
            http::Request::Body(Down::Demand { read, room: 0 }),
            &mut value.events,
            &mut value.below,
        );
    }
    fn hear(&mut self) -> bool {
        let mut gone = false;
        while let Some(heard) = self.heard.pop() {
            self.events.push_back(match heard {
                mapping::Heard::Reply { exchange, message } => client::Event::Reply { exchange, message },
                mapping::Heard::Lost { exchange } => client::Event::Lost { exchange },
                mapping::Heard::Streamed { message } => client::Event::Streamed { message },
                mapping::Heard::StreamGone => {
                    gone = true;
                    client::Event::StreamGone
                }
            });
        }
        gone
    }
    fn failed(&mut self, owner: Owner) {
        match owner {
            Owner::Link(exchange) => self.events.push_back(client::Event::Lost { exchange }),
            Owner::Stream => self.events.push_back(client::Event::StreamGone),
        }
    }
    #[must_use]
    pub fn next_deadline(&self) -> Option<Time> {
        self.exchanges
            .values()
            .filter(|value| matches!(value.owner, Owner::Link(_)))
            .map(|value| value.until)
            .chain(self.pending.iter().filter_map(
                |(owner, _, until, _)| {
                    if matches!(owner, Owner::Link(_)) { Some(*until) } else { None }
                },
            ))
            .min()
    }
    fn expire(&mut self, now: Time, below: &mut Queue<(Conn, Down)>) {
        for _ in 0..self.pending.len() {
            let (owner, call, until, kind) = self.pending.pop_front().expect("pending call");
            if matches!(owner, Owner::Link(_)) && now >= until {
                self.failed(owner);
            } else {
                self.pending.push_back((owner, call, until, kind));
            }
        }
        let expired: Vec<_> = self
            .exchanges
            .values()
            .filter_map(|value| match value.owner {
                Owner::Link(exchange) if now >= value.until => Some(exchange),
                Owner::Link(_) | Owner::Stream => None,
            })
            .collect();
        for exchange in expired {
            self.close(now, Some(exchange), below);
            self.failed(Owner::Link(exchange));
        }
    }
    pub(crate) fn close_all(&mut self, now: Time, below: &mut Queue<(Conn, Down)>) {
        let env = self.env(now);
        for (conn, mut value) in std::mem::take(&mut self.exchanges) {
            http::down(&mut value.machine, &env, http::Request::Close, &mut value.events, &mut value.below);
            while let Some(down) = value.below.pop() {
                below.push((conn, down));
            }
            below.push((conn, Down::Finish));
        }
        self.pending.clear();
        self.events.clear();
    }
    fn close(&mut self, now: Time, exchange: Option<Token>, below: &mut Queue<(Conn, Down)>) {
        self.pending.retain(|(owner, _, _, _)| match owner {
            Owner::Link(token) => Some(*token) != exchange,
            Owner::Stream => exchange.is_some(),
        });
        let conn = self.exchanges.iter().find_map(|(conn, value)| match value.owner {
            Owner::Link(token) if Some(token) == exchange => Some(*conn),
            Owner::Stream if exchange.is_none() => Some(*conn),
            Owner::Link(_) | Owner::Stream => None,
        });
        if let Some(conn) = conn {
            let mut value = self.exchanges.remove(&conn).expect("live connection");
            http::down(&mut value.machine, &self.env(now), http::Request::Close, &mut value.events, &mut value.below);
            while let Some(down) = value.below.pop() {
                below.push((conn, down));
            }
            below.push((conn, Down::Finish));
        }
    }
}
