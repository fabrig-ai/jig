//! The native shell over the existing simulated-world harness, against an
//! engine that answers HTTP calls but never answers a client's FIN.
use jig_client as client;
use jig_client_shell_native::{Limits, Settings, Shell};
use skein_io::{
    self as io,
    kernel::{Complete, Fd, Submit},
};
use skein_lib::{
    Duration, Env, Queue, Time, Token, Wall,
    stream::{Down, Read, Up},
};
use skein_shell::Host;
use std::collections::BTreeMap;

const SEED: u64 = 7;
const EXCHANGES: u32 = 1;
// One stream, one exchange, and one closing socket; five calls need two waits.
const SOCKETS: u32 = 3;
const CALLS: u64 = 5;
// The fake retains every call, the stream and its listener until teardown.
const ENGINE_SOCKETS: u32 = 7;
const CLOSE_TIMEOUT: Duration = Duration::from_secs(5);
const DEADLINE: Duration = Duration::from_secs(20);
const QUEUED_DEADLINE: Duration = Duration::from_secs(1);
const ROOM: u32 = 1024;

fn address() -> io::Addr {
    io::Addr::Ip((std::net::Ipv4Addr::LOCALHOST, 8080).into())
}
fn io_limits() -> io::Limits {
    io::Limits {
        sockets: SOCKETS,
        names: 1,
        refusals: 1,
        intake: ROOM,
        receive: 64,
        output: ROOM,
        sends: 2,
        accepts: 1,
        backlog: 2,
        close_timeout: CLOSE_TIMEOUT,
        retry: Duration::from_millis(1),
    }
}
struct EngineSocket {
    socket: Token,
    request: Vec<u8>,
    response: Option<Box<[u8]>>,
}
struct Process {
    shell: Option<Shell>,
    io: io::Io,
    env: Env<io::Limits>,
    complete: Queue<Complete>,
    submit: Queue<Submit>,
    events: Queue<io::Event>,
    sockets: BTreeMap<Token, EngineSocket>,
    listener: Option<Token>,
    started: bool,
    closing: bool,
    replies: Vec<(u64, Time)>,
    lost: Vec<(u64, Time)>,
}
impl Process {
    fn new(root: Option<Fd>, deadline: Duration) -> Self {
        let shell = root.map(|root| {
            let mut shell = Shell::new(Settings {
                engine: address(),
                directory: b"private".as_slice().into(),
                limits: Limits {
                    exchanges: EXCHANGES,
                    body_bytes: 128,
                    deadline,
                    http: skein_http::client::Limits { head: ROOM, headers: 8, request: ROOM, read: 64, send: 64 },
                },
                io: io_limits(),
            });
            shell.adopt(root, 1000, SEED);
            shell
        });
        let engine_limits = io::Limits { sockets: ENGINE_SOCKETS, ..io_limits() };
        Self {
            shell,
            io: io::Io::new(&engine_limits),
            env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits: engine_limits },
            complete: Queue::with_capacity(256),
            submit: Queue::with_capacity(256),
            events: Queue::with_capacity(256),
            sockets: BTreeMap::new(),
            listener: None,
            started: false,
            closing: false,
            replies: Vec::new(),
            lost: Vec::new(),
        }
    }
    fn exchange(shell: &mut Shell, exchange: u64) {
        shell.down(client::Request::Exchange {
            exchange: Token::new(exchange),
            message: client::Message::Request(b"ask".as_slice().into()),
            credential: None,
        });
    }
    fn request(&mut self, request: io::Request) {
        io::down(&mut self.io, &self.env, request, &mut self.submit);
    }
    fn engine_event(&mut self, event: io::Event) {
        match event {
            io::Event::Listening { listener, .. } => self.listener = Some(listener),
            io::Event::Accepted { socket, .. } => {
                self.sockets.insert(socket, EngineSocket { socket, request: Vec::new(), response: None });
                self.request(io::Request::Bind { socket, owner: socket });
                self.request(io::Request::Stream {
                    stream: socket,
                    down: Down::Demand { read: Read::Arrived { max: ROOM }, room: 0 },
                });
            }
            io::Event::Stream { owner, up: Up::Bytes(bytes) } => {
                let peer = self.sockets.get_mut(&owner).expect("accepted socket");
                peer.request.extend_from_slice(&bytes);
                let stream = peer.request.starts_with(b"GET ");
                let complete =
                    if stream { peer.request.ends_with(b"\r\n\r\n") } else { peer.request.ends_with(b"\r\n\r\nask") };
                if complete {
                    peer.response = Some(if stream {
                        b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\r\n".as_slice().into()
                    } else {
                        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok".as_slice().into()
                    });
                }
                self.request(io::Request::Stream {
                    stream: owner,
                    down: Down::Demand {
                        read: if complete { Read::Nothing } else { Read::Arrived { max: ROOM } },
                        room: if complete { ROOM } else { 0 },
                    },
                });
            }
            io::Event::Stream { owner, up: Up::Room } => {
                let response = self.sockets.get_mut(&owner).expect("accepted socket").response.take().expect("answer");
                self.request(io::Request::Stream { stream: owner, down: Down::Send(response) });
                // Deliberately keep this end open even after the client's FIN.
            }
            io::Event::Closed { owner } => {
                self.sockets.remove(&owner);
                if self.listener == Some(owner) {
                    self.listener = None;
                }
            }
            io::Event::Stream { up: Up::End | Up::Failed(_), .. } => {}
            other @ (io::Event::Connecting { .. }
            | io::Event::Connected { .. }
            | io::Event::Output { .. }
            | io::Event::Spawned { .. }
            | io::Event::Exited { .. }
            | io::Event::Usage { .. }
            | io::Event::Resized
            | io::Event::Window { .. }
            | io::Event::Shutdown { .. }
            | io::Event::Failed { .. }) => panic!("unexpected engine event: {other:?}"),
        }
    }
}
impl Host for Process {
    fn iterate(&mut self, now: Time, wall: Wall) {
        if let Some(shell) = &mut self.shell {
            shell.iterate(now, wall);
            if !self.started && shell.loaded().is_some() {
                self.started = true;
                shell.down(client::Request::OpenStream { credential: None });
                Self::exchange(shell, 1);
            }
            while let Some(event) = shell.events().pop() {
                match event {
                    client::Event::Reply { exchange, message } => {
                        assert_eq!(message.as_ref(), b"ok");
                        self.replies.push((exchange.raw(), now));
                        if exchange.raw() < CALLS {
                            Self::exchange(shell, exchange.raw() + 1);
                        } else {
                            self.closing = true;
                            shell.close();
                        }
                    }
                    client::Event::Lost { exchange } => {
                        self.lost.push((exchange.raw(), now));
                        self.closing = true;
                        shell.close();
                    }
                    other @ (client::Event::Start { .. }
                    | client::Event::StartUnlinked { .. }
                    | client::Event::SignIn { .. }
                    | client::Event::Authorize { .. }
                    | client::Event::SignOut
                    | client::Event::SignOutLost
                    | client::Event::Watch { .. }
                    | client::Event::Unwatch { .. }
                    | client::Event::Read { .. }
                    | client::Event::Send { .. }
                    | client::Event::Carry { .. }
                    | client::Event::Full { .. }
                    | client::Event::Streamed { .. }
                    | client::Event::StreamGone
                    | client::Event::Stored
                    | client::Event::NotStored
                    | client::Event::Up { .. }) => panic!("unexpected client event: {other:?}"),
                }
            }
            return;
        }
        self.env.now = now;
        self.env.wall = wall;
        if !self.started {
            self.started = true;
            self.request(io::Request::Listen { owner: Token::new(0), addr: address() });
        }
        while self.io.is_ready() {
            io::resume(&mut self.io, &self.env, &mut self.events, &mut self.submit);
        }
        while let Some(complete) = self.complete.pop() {
            io::up(&mut self.io, &self.env, complete, &mut self.events, &mut self.submit);
        }
        while let Some(event) = self.events.pop() {
            self.engine_event(event);
        }
        if self.closing {
            let entities: Vec<_> = self.sockets.values().map(|s| s.socket).chain(self.listener).collect();
            for entity in entities {
                self.request(io::Request::Abort { entity });
            }
        }
        self.io.reclaim();
    }
    fn completions(&mut self) -> &mut Queue<Complete> {
        self.shell.as_mut().map_or(&mut self.complete, Shell::completions)
    }
    fn submissions(&mut self) -> &mut Queue<Submit> {
        self.shell.as_mut().map_or(&mut self.submit, Shell::submissions)
    }
    fn work_pending(&self, now: Time) -> bool {
        self.shell.as_ref().map_or_else(
            || self.io.is_ready() || !self.complete.is_empty() || !self.submit.is_empty(),
            |shell| shell.work_pending(now),
        )
    }
    fn next_deadline(&self) -> Option<Time> {
        self.shell.as_ref().map_or_else(|| self.io.next_deadline(), Shell::next_deadline)
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        None
    }
    fn is_empty(&self) -> bool {
        self.shell.as_ref().map_or_else(|| self.closing && self.io.is_empty(), Shell::is_empty)
    }
    fn worst_case(&self) -> u64 {
        self.shell.as_ref().map_or(0, Shell::worst_case)
    }
    fn operations(&self) -> u32 {
        self.shell.as_ref().map_or_else(|| io::operations(&self.env.limits).expect("io operations"), Shell::operations)
    }
}
struct Machine(skein_fake_machine::Machine);
impl skein_world::Machine for Machine {
    fn step(&mut self, call: skein_sim::Call, answers: &mut Queue<skein_sim::Answer>) {
        skein_fake_machine::step(&mut self.0, call, answers);
    }
}
struct Judge {
    done: bool,
}
impl skein_world::Referee<Process> for Judge {
    fn act(&mut self, _now: Time, procs: &mut [Process]) {
        if procs[1].is_empty() {
            procs[0].closing = true;
            self.done = true;
        }
    }
    fn observe(&mut self, _now: Time, _procs: &[Process]) {}
    fn next_deadline(&self) -> Option<Time> {
        if self.done { None } else { Some(Time::ZERO.saturating_add(DEADLINE)) }
    }
    fn overdue(&self, now: Time) -> Option<String> {
        (!self.done && now >= Time::ZERO.saturating_add(DEADLINE)).then(|| "socket waits did not finish".to_owned())
    }
    fn passed(&self) -> bool {
        self.done
    }
}
fn run(deadline: Duration) -> skein_world::Outcome<Process, Machine> {
    let mut machine = skein_fake_machine::Machine::for_user(1000);
    let root = machine.lay(&[]);
    let mut world =
        skein_world::World::new(SEED, skein_sim::Config::calm(), Judge { done: false }, skein_world::Memory::Unchecked)
            .with_machine(Machine(machine));
    world.spawn(|| Process::new(None, deadline));
    world.spawn_root(skein_sim::Handle::new(root.raw()), |root| Process::new(Some(root), deadline));
    world.run()
}

#[test]
fn exchanges_wait_for_closing_sockets_without_losing_their_answers() {
    let outcome = run(DEADLINE);
    let replies = &outcome.procs[1].replies;
    assert_eq!(replies.iter().map(|(exchange, _)| *exchange).collect::<Vec<_>>(), [1, 2, 3, 4, 5]);
    let first_close = Time::ZERO.saturating_add(CLOSE_TIMEOUT);
    assert!(replies[1].1 < first_close);
    assert!(replies[2].1 >= first_close, "the third exchange waits for a socket's close timeout");
    assert!(outcome.procs.iter().all(Host::is_empty));
}

#[test]
fn an_exchange_waiting_for_a_socket_keeps_its_deadline() {
    let outcome = run(QUEUED_DEADLINE);
    let client = &outcome.procs[1];
    assert_eq!(client.replies.iter().map(|(exchange, _)| *exchange).collect::<Vec<_>>(), [1, 2]);
    assert_eq!(client.lost.len(), 1);
    let (exchange, expired) = client.lost[0];
    assert_eq!(exchange, 3);
    assert!(expired >= Time::ZERO.saturating_add(QUEUED_DEADLINE));
    assert!(expired < Time::ZERO.saturating_add(CLOSE_TIMEOUT));
    assert!(outcome.procs.iter().all(Host::is_empty));
}
