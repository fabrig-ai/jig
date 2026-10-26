//! Accepted streams, the workers' component and its typed root handoff (channel.md, 10.2).
use crate::{Files, tools, values::Values};
use alloc::boxed::Box;
use jig_domain_host as host;
use jig_protocol_config::{self as config, Path, Problems, What};
use jig_protocol_engine_workers as kit;
use jig_test_domain as root;
use skein_channel::{Lower, LowerEvent};
use skein_io::{self as io};
use skein_lib::{List, Map, Queue, Time, Token, stream};
const LISTENERS: u64 = 1_u64 << 60;
const CONNECTIONS: u64 = 2_u64 << 60;
#[derive(Debug)]
pub(super) struct Workers {
    pub(super) component: kit::Component,
    out: Queue<kit::Request>,
    pub(super) io: Queue<io::Request>,
    pub(super) listeners: List<(Token, Option<Token>)>,
    pub(super) sockets: Map<Token, Token>,
    next: u64,
    pub(super) opened: bool,
    pub(super) ending: Option<crate::stop::Closing>,
    pub(super) component_closed: bool,
    deployment: [u8; 16],
    endpoints: List<jig_protocol_charter::EndpointName>,
}
fn invalid(what: What) -> Problems {
    let mut found = Problems::empty();
    found.push(Path::root().member(b"engine").member(b"hosts"), what);
    found
}
impl Workers {
    pub fn new(
        config: &config::Engine,
        files: &Files,
        deployment: [u8; 16],
        limits: &kit::Limits,
        room: u32,
    ) -> Result<Self, Problems> {
        let mut workers = List::with_capacity(limits.names);
        for worker in &config.hosts.workers {
            let digest = match config::digest(files.contents(&worker.digest).ok_or(invalid(What::Unreadable))?) {
                Ok(digest) => digest,
                Err(what) => return Err(invalid(what)),
            };
            workers.push(kit::Worker { name: worker.name.clone(), digest }).or(Err(invalid(What::Bound)))?;
        }
        let component = kit::Component::new(limits, workers.into_boxed(), 1).or(Err(invalid(What::Bound)))?;
        let count = u32::try_from(config.hosts.listener.len()).or(Err(invalid(What::Bound)))?;
        if count == 0 || count > config.limits.io.sockets {
            return Err(invalid(What::Bound));
        }
        let mut listeners = List::with_capacity(count);
        let mut io = Queue::with_capacity(room);
        for (index, address) in config.hosts.listener.iter().enumerate() {
            let config::Address::Addr(address) = address else {
                return Err(invalid(What::Form));
            };
            let owner = Token::new(
                LISTENERS.checked_add(u64::try_from(index).expect("listener count")).expect("listener owner"),
            );
            io.push(io::Request::Listen { owner, addr: *address });
            listeners.push((owner, None)).expect("listener count");
        }
        let mut endpoints = List::with_capacity(u32::try_from(config.llm.endpoints.len()).expect("endpoint count"));
        for endpoint in &config.llm.endpoints {
            endpoints
                .push(jig_protocol_charter::EndpointName {
                    number: endpoint.number,
                    name: endpoint.name.clone(),
                    dialect: endpoint.dialect,
                })
                .expect("endpoint count");
        }
        Ok(Self {
            component,
            out: Queue::with_capacity(kit::MAX_OUT_DOWN),
            io,
            listeners,
            sockets: Map::with_capacity(limits.workers.checked_add(limits.opening).expect("connections")),
            next: 1,
            opened: false,
            ending: None,
            component_closed: false,
            deployment,
            endpoints,
        })
    }
    pub fn next_deadline(&self) -> Option<Time> {
        self.component.next_deadline()
    }
    pub fn opened(&self) -> bool {
        self.opened
    }
    pub fn batch_room(&self, batch: u32) -> bool {
        self.io.room() >= batch.checked_mul(kit::MAX_OUT_DOWN).expect("worker route room")
    }
    pub fn room(&self) -> bool {
        self.io.room() >= kit::MAX_OUT_DOWN
    }
    pub fn work_pending(&self, now: Time) -> bool {
        !self.io.is_empty()
            || !self.out.is_empty()
            || match self.next_deadline() {
                Some(deadline) => deadline <= now,
                None => false,
            }
    }
    pub fn down(&mut self, now: Time, event: kit::Event, events: &mut Queue<root::Event>) {
        self.component.down(now, event, &mut self.out);
        self.drain(events);
    }
    pub fn assignment(
        &mut self,
        now: Time,
        channel: Token,
        value: root::Assignment,
        values: &Values,
        events: &mut Queue<root::Event>,
    ) {
        let (task, attempt) = (value.task, value.attempt);
        match self.encoded_assignment(now, channel, value, values) {
            Some(event) => self.down(now, event, events),
            None => events.push(root::Event::Answer {
                channel,
                task,
                attempt,
                cumulative: 0,
                read: None,
                refused: Box::default(),
                end: jig_domain_tasks::End::Failed(jig_domain_tasks::Class::Invalid),
            }),
        }
    }
    pub fn encoded_assignment(
        &self,
        now: Time,
        channel: Token,
        value: root::Assignment,
        values: &Values,
    ) -> Option<kit::Event> {
        Some(delivery(
            now,
            channel,
            host::LinkDown::Assign {
                // This application's charter exposes no delivery tool, so it keeps no delivery evidence.
                assignment: tools::charter(value, self.endpoints.as_slice(), &self.deployment, &[])?,
            },
            values,
        ))
    }
    pub fn send(
        &mut self,
        now: Time,
        channel: Token,
        down: host::LinkDown,
        values: &Values,
        events: &mut Queue<root::Event>,
    ) {
        self.down(now, delivery(now, channel, down, values), events);
    }
    pub fn up(&mut self, now: Time, event: io::Event, events: &mut Queue<root::Event>) -> bool {
        let Some(owner) = owner(&event) else {
            return false;
        };
        match event {
            io::Event::Listening { listener, .. } => {
                for index in 0..self.listeners.len() {
                    let (name, slot) = self.listeners.get_mut(index).expect("listener index");
                    if *name == owner {
                        *slot = Some(listener);
                    }
                }
                self.opened = listeners_open(&self.listeners);
                self.late_listener(listener);
            }
            io::Event::Accepted { socket, .. } => {
                let connection = Token::new(CONNECTIONS.checked_add(self.next).expect("connection owner"));
                self.next = self.next.checked_add(1).expect("connection owner sequence");
                if self.sockets.insert(connection, socket).is_err() {
                    self.io.push(io::Request::Reject { socket });
                } else {
                    self.down(now, kit::Event::Accepted { connection }, events);
                }
            }
            io::Event::Stream { up, .. } => {
                self.down(now, kit::Event::Lower { connection: owner, event: LowerEvent::Read(up) }, events);
            }
            io::Event::Output { up, .. } => {
                self.down(now, kit::Event::Lower { connection: owner, event: LowerEvent::Write(up) }, events);
            }
            io::Event::Closed { .. } => {
                if self.listener_closed(owner) {
                    return true;
                }
                let _socket = self.sockets.remove(&owner);
                self.down(now, kit::Event::Gone { connection: owner }, events);
            }
            io::Event::Failed { .. }
            | io::Event::Connecting { .. }
            | io::Event::Connected { .. }
            | io::Event::Spawned { .. }
            | io::Event::Exited { .. }
            | io::Event::Usage { .. }
            | io::Event::Resized
            | io::Event::Window { .. }
            | io::Event::Shutdown { .. } => return false,
        }
        true
    }
    fn drain(&mut self, events: &mut Queue<root::Event>) {
        for _ in 0..self.out.len() {
            match self.out.pop().expect("worker requests") {
                kit::Request::Take { connection } => self.io.push(io::Request::Bind {
                    socket: *self.sockets.get(&connection).expect("accepted socket"),
                    owner: connection,
                }),
                kit::Request::TurnAway { connection } => {
                    if let Some(socket) = self.sockets.remove(&connection) {
                        self.io.push(io::Request::Reject { socket });
                    }
                }
                kit::Request::Close { connection } => {
                    if let Some(socket) = self.sockets.get(&connection) {
                        self.io.push(io::Request::Close { entity: *socket });
                    }
                }
                kit::Request::Abort { connection } => {
                    if let Some(socket) = self.sockets.get(&connection) {
                        self.io.push(io::Request::Abort { entity: *socket });
                    }
                }
                kit::Request::Lower { connection, lower } => {
                    if let Some(socket) = self.sockets.get(&connection) {
                        self.io.push(match lower {
                            Lower::Read(down) => io::Request::Stream { stream: *socket, down },
                            Lower::Write(down) => io::Request::Output { stream: *socket, down },
                            Lower::FinishWrite => io::Request::Stream { stream: *socket, down: stream::Down::Finish },
                        });
                    }
                }
                kit::Request::Hello { channel, worker, hello } => {
                    events.push(root::Event::Link { channel, link: root::link::Event::Hello { worker, hello } });
                }
                kit::Request::Up { channel, up, parts } => {
                    drop(parts);
                    events.push(root::Event::Link { channel, link: root::link::Event::Up(up) });
                }
                kit::Request::Lost { channel } => {
                    events.push(root::Event::Link { channel, link: root::link::Event::Lost });
                }
                kit::Request::Closed => self.component_closed = true,
            }
        }
    }
    pub fn iterate(&mut self, now: Time, events: &mut Queue<root::Event>) {
        if self.room() && events.room() >= kit::MAX_OUT_FIRE {
            self.component.fire(now, &mut self.out);
            self.drain(events);
        }
    }
    pub fn abort(&mut self, now: Time, events: &mut Queue<root::Event>) {
        if self.ending != Some(crate::stop::Closing::Abort) {
            self.ending = Some(crate::stop::Closing::Abort);
            for (_, listener) in &self.listeners {
                if let Some(listener) = listener {
                    self.io.push(io::Request::Abort { entity: *listener });
                }
            }
            self.down(now, kit::Event::Abort, events);
        }
    }
}
fn listeners_open(listeners: &List<(Token, Option<Token>)>) -> bool {
    for (_, entity) in listeners {
        if entity.is_none() {
            return false;
        }
    }
    true
}
pub(super) fn owner(event: &io::Event) -> Option<Token> {
    match event {
        io::Event::Listening { owner, .. }
        | io::Event::Accepted { owner, .. }
        | io::Event::Connecting { owner, .. }
        | io::Event::Connected { owner }
        | io::Event::Stream { owner, .. }
        | io::Event::Output { owner, .. }
        | io::Event::Spawned { owner, .. }
        | io::Event::Exited { owner, .. }
        | io::Event::Usage { owner, .. }
        | io::Event::Window { owner, .. }
        | io::Event::Failed { owner, .. }
        | io::Event::Closed { owner } => Some(*owner),
        io::Event::Resized | io::Event::Shutdown { .. } => None,
    }
}

fn delivery(now: Time, channel: Token, down: host::LinkDown, values: &Values) -> kit::Event {
    let grants = match &down {
        host::LinkDown::Assign { assignment } => values.grants(&assignment.assignment.grants, now),
        host::LinkDown::Grant { grant, .. } => values.grants(::core::slice::from_ref(grant), now),
        host::LinkDown::Sources { .. }
        | host::LinkDown::Check { .. }
        | host::LinkDown::SignOut { .. }
        | host::LinkDown::Message { .. }
        | host::LinkDown::Relayed { .. }
        | host::LinkDown::TurnKept { .. }
        | host::LinkDown::Acknowledge { .. }
        | host::LinkDown::Cancel { .. } => Box::default(),
    };
    kit::Event::Down { channel, down, parts: Box::default(), values: grants }
}

// The endpoint names come from the whole checked document, rather than a profile count.
pub(super) fn configured_bytes(config: &config::Engine) -> Option<u64> {
    let count = u32::try_from(config.llm.endpoints.len()).ok()?;
    let mut bytes = List::<jig_protocol_charter::EndpointName>::worst_case(count)?;
    for endpoint in &config.llm.endpoints {
        bytes = bytes.checked_add(u64::try_from(endpoint.name.len()).ok()?)?;
    }
    Some(bytes)
}
