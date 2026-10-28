mod client_store;
use super::*;
use jig_client_protocol_http::Unlinked;
const EXCHANGE: u64 = 42;
use jig_client as client;
use skein_lib::{
    Queue, Secret, Time, Token, Writer,
    base64::{self, Alphabet},
    stream::Down,
};
use skein_world::stream::Wire;
const TOKEN: [u8; 40] = [7; 40];
const MIXED_BATCHES: u32 = 2; // One Save and one Erase.
const TOTAL_BATCHES: u32 = 3; // One active Save, then the two mixed batches.
pub(crate) fn limits() -> Limits {
    Limits {
        exchanges: 2,
        body_bytes: 128,
        deadline: skein_lib::Duration::from_secs(1),
        http: skein_http::client::Limits { head: 1024, headers: 8, request: 1024, read: 64, send: 64 },
    }
}
struct Peer {
    shell: Connections,
    wire: Wire,
    up: Queue<client::Event>,
    below: Queue<(Conn, Down)>,
    conn: Option<Conn>,
}
impl Peer {
    fn new() -> Self {
        Self {
            shell: Connections::new(&limits(), "127.0.0.1:8080"),
            wire: Wire::new(1),
            up: Queue::with_capacity(256),
            below: Queue::with_capacity(256),
            conn: None,
        }
    }
    fn exchange(&mut self) {
        self.shell.down(
            Time::ZERO,
            client::Request::Exchange {
                exchange: Token::new(EXCHANGE),
                credential: Some(Secret::new(Box::new(TOKEN))),
                message: client::Message::Request(Box::from(b"ask".as_slice())),
            },
            &mut self.below,
        );
    }
    fn drive(&mut self) {
        self.shell.take(Time::ZERO, &mut self.up, &mut self.below);
        for _ in 0..4000 {
            while let Some((conn, down)) = self.below.pop() {
                self.conn = Some(conn);
                self.wire.take(down);
            }
            if let Some(up) = self.wire.answer() {
                self.shell.up(Time::ZERO, self.conn.expect("one test connection"), up, &mut self.up, &mut self.below);
            }
        }
    }
    fn respond(&mut self, response: &[u8]) {
        self.wire.write(response);
        self.drive();
    }
}
#[test]
fn an_exchange_carries_the_bearer_token() {
    let mut peer = Peer::new();
    peer.exchange();
    peer.respond(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok");
    let text = std::str::from_utf8(&peer.wire.received).expect("HTTP request");
    let mut writer = Writer::new(54);
    base64::encode(Alphabet::Url, &TOKEN, &mut writer).expect("bearer");
    let expected = format!("authorization: Bearer {}", std::str::from_utf8(&writer.finish()).expect("base64"));
    assert!(text.to_ascii_lowercase().contains(&expected.to_ascii_lowercase()), "{text}");
    assert!(text.to_ascii_lowercase().contains("content-type: application/octet-stream\r\n"));
    assert!(text.to_ascii_lowercase().contains("content-length: 3\r\n"));
    assert!(text.to_ascii_lowercase().contains("host: 127.0.0.1:8080\r\n"));
    assert!(
        matches!(peer.up.pop(), Some(client::Event::Reply { exchange, message: body }) if exchange == Token::new(EXCHANGE) && body.as_ref() == b"ok")
    );
}
#[test]
fn a_body_of_unknown_length_read_as_it_arrives_is_the_same_whatever_its_cuts() {
    let response = b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\nwhole body";
    for size in [response.len(), 1] {
        let mut peer = Peer::new();
        peer.exchange();
        for piece in response.chunks(size) {
            peer.respond(piece);
        }
        peer.wire.eof = true;
        peer.drive();
        assert!(matches!(peer.up.pop(), Some(client::Event::Reply { exchange, message: body })
            if exchange == Token::new(EXCHANGE) && body.as_ref() == b"whole body"));
        assert!(peer.up.is_empty());
    }
}
#[test]
fn a_connection_cut_mid_body_fails_the_exchange() {
    let mut peer = Peer::new();
    peer.exchange();
    peer.respond(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nab");
    peer.wire.eof = true;
    peer.drive();
    assert!(matches!(peer.up.pop(), Some(client::Event::Lost { exchange }) if exchange == Token::new(EXCHANGE)));
    assert!(peer.up.is_empty());
}
#[test]
fn the_stream_s_bytes_reach_the_link_in_order() {
    let mut peer = Peer::new();
    peer.shell.down(Time::ZERO, client::Request::OpenStream { credential: None }, &mut peer.below);
    peer.respond(b"HTTP/1.1 200 OK\r\n\r\ndata: Zmlyc3Q=\n\ndata: c2Vjb25k\n\n");
    let mut bytes = Vec::new();
    while let Some(event) = peer.up.pop() {
        if let client::Event::Streamed { message: client::Streamed::Record(part) } = event {
            bytes.extend_from_slice(&part);
        }
    }
    assert_eq!(bytes, b"firstsecond");
    peer.shell.down(Time::ZERO, client::Request::CloseStream, &mut peer.below);
    peer.drive();
    assert!(peer.wire.finished);
}
#[test]
fn a_link_of_another_form_is_refused() {
    let mut shell = Connections::new(&limits(), "127.0.0.1:8080");
    for (label, text, expected) in [
        ("scheme", b"https://127.0.0.1:8080/".as_slice(), Unlinked::NotHttp),
        ("secret", b"http://127.0.0.1:8080/jig/v1/sign-in/link".as_slice(), Unlinked::NoSecret),
        ("address", b"http://example/".as_slice(), Unlinked::Address),
        ("short secret", b"http://127.0.0.1:8080/jig/v1/sign-in/link?secret=AA".as_slice(), Unlinked::NoSecret),
    ] {
        assert_eq!(shell.printed(text), Err(expected), "{label}");
    }
}
#[derive(Clone, Copy)]
enum FileCase {
    KeepToken,
    ReloadToken,
    Store,
    SaveAndErase,
    DuringWrite,
    RefusedDuringWrite,
    ClientTurn,
}
struct Disk {
    shell: Shell,
    case: FileCase,
    started: bool,
    closing: bool,
    writes: u32,
    failures: u32,
    sockets: u32,
    token: Option<Vec<u8>>,
    write_acks: Vec<u32>,
    answers: Vec<bool>,
    client: Option<client::Client>,
    keyed: u32,
}
impl Disk {
    fn new(root: skein_io::kernel::Fd, case: FileCase) -> Self {
        let mut bounds = limits();
        if matches!(case, FileCase::ClientTurn) {
            bounds.body_bytes = client_store::STORE_BYTES;
        }
        let mut shell = Shell::new(Settings {
            engine: skein_io::Addr::Ip((std::net::Ipv4Addr::LOCALHOST, 8080).into()),
            directory: b"private".as_slice().into(),
            limits: bounds,
            io: file_io_limits(),
        });
        shell.adopt(root, 1000, 7);
        Self {
            shell,
            case,
            started: false,
            closing: false,
            writes: 0,
            failures: 0,
            sockets: 0,
            token: None,
            write_acks: Vec::new(),
            answers: Vec::new(),
            client: None,
            keyed: 0,
        }
    }
    fn start(&mut self) {
        self.started = true;
        match self.case {
            FileCase::KeepToken => {
                self.shell.keep(Some(Secret::new(Box::new(TOKEN))));
            }
            FileCase::ClientTurn => {
                let (client, mut out) = client_store::mixed_turn();
                self.client = Some(client);
                while let Some(request) = out.pop() {
                    self.shell.down(request);
                }
            }
            FileCase::ReloadToken => {
                self.closing = true;
            }
            FileCase::Store | FileCase::SaveAndErase | FileCase::DuringWrite | FileCase::RefusedDuringWrite => {
                self.shell.down(save(b"key", b"pending"));
                if matches!(self.case, FileCase::SaveAndErase) {
                    self.shell.down(erase(b"absent"));
                }
                self.shell.down(client::Request::Exchange {
                    exchange: Token::new(EXCHANGE),
                    credential: Some(Secret::new(Box::new(TOKEN))),
                    message: client::Message::Request(b"ask".as_slice().into()),
                });
            }
        }
    }
}
impl skein_shell::Host for Disk {
    fn iterate(&mut self, now: Time, wall: skein_lib::Wall) {
        self.shell.iterate(now, wall);
        if !self.started
            && let Some((credential, _)) = self.shell.loaded()
        {
            self.token = credential.map(|token| token.expose().to_vec());
            self.start();
        }
        if self.shell.kept().is_some() {
            self.closing = true;
        }
        while let Some(event) = self.shell.events().pop() {
            match event {
                client::Event::Stored => {
                    self.writes += 1;
                    self.answers.push(true);
                    if let Some(client) = &mut self.client {
                        // The fixture uses the same output bound as the client.
                        let mut out = Queue::with_capacity(client::max_out(&client_store::bounds()));
                        client.step(now, wall, client::Event::Stored, &mut out);
                        while let Some(request) = out.pop() {
                            if matches!(request, client::Request::Exchange { message: client::Message::Request(_), .. })
                            {
                                self.keyed += 1;
                            }
                            self.shell.down(request);
                        }
                    }
                }
                client::Event::NotStored => {
                    self.failures += 1;
                    self.answers.push(false);
                    self.closing = true;
                }
                client::Event::Lost { .. } => self.closing = true,
                other @ (client::Event::SignIn { .. }
                | client::Event::Authorize { .. }
                | client::Event::SignOut
                | client::Event::SignOutLost
                | client::Event::Start { .. }
                | client::Event::StartUnlinked { .. }
                | client::Event::Send { .. }
                | client::Event::Carry { .. }
                | client::Event::Full { .. }
                | client::Event::Read { .. }
                | client::Event::Reply { .. }
                | client::Event::Watch { .. }
                | client::Event::Unwatch { .. }
                | client::Event::Up { .. }
                | client::Event::Streamed { .. }
                | client::Event::StreamGone) => panic!("file fixture event {other:?}"),
            }
        }
        let writing =
            self.shell.submissions().iter().any(|submit| matches!(submit.kind, skein_io::kernel::Op::Write { .. }));
        if writing {
            self.write_acks.push(self.writes);
            if matches!(self.case, FileCase::DuringWrite | FileCase::RefusedDuringWrite) && self.write_acks.len() == 1 {
                // A record as large as the whole store cannot fit its framing.
                let bytes = if matches!(self.case, FileCase::RefusedDuringWrite) {
                    vec![0; usize::try_from(limits().body_bytes).expect("store bound")]
                } else {
                    b"later".to_vec()
                };
                self.shell.down(save(b"next", &bytes));
                self.shell.down(erase(b"key"));
            }
        }
        for submit in self.shell.submissions().iter() {
            if matches!(submit.kind, skein_io::kernel::Op::Socket { .. }) {
                assert!(self.writes > 0, "store durable before connection");
                self.sockets += 1;
            }
        }
        if self.closing {
            self.shell.close();
        }
    }
    fn completions(&mut self) -> &mut Queue<skein_io::kernel::Complete> {
        self.shell.completions()
    }
    fn submissions(&mut self) -> &mut Queue<skein_io::kernel::Submit> {
        self.shell.submissions()
    }
    fn work_pending(&self, now: Time) -> bool {
        self.shell.work_pending(now)
    }
    fn next_deadline(&self) -> Option<Time> {
        self.shell.next_deadline()
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        None
    }
    fn is_empty(&self) -> bool {
        self.shell.is_empty()
    }
    fn worst_case(&self) -> u64 {
        self.shell.worst_case()
    }
    fn operations(&self) -> u32 {
        self.shell.operations()
    }
}
struct DiskMachine(skein_fake_machine::Machine);
impl skein_world::Machine for DiskMachine {
    fn step(&mut self, call: skein_sim::Call, answers: &mut Queue<skein_sim::Answer>) {
        skein_fake_machine::step(&mut self.0, call, answers);
    }
}
struct DiskJudge;
impl skein_world::Referee<Disk> for DiskJudge {
    fn act(&mut self, _now: Time, _procs: &mut [Disk]) {}
    fn observe(&mut self, _now: Time, _procs: &[Disk]) {}
    fn next_deadline(&self) -> Option<Time> {
        Some(Time::ZERO.saturating_add(skein_lib::Duration::from_secs(1)))
    }
    fn overdue(&self, now: Time) -> Option<String> {
        (now >= Time::ZERO.saturating_add(skein_lib::Duration::from_secs(1)))
            .then(|| "file fixture did not drain".to_owned())
    }
    fn passed(&self) -> bool {
        true
    }
}
pub(crate) fn file_io_limits() -> skein_io::Limits {
    skein_io::Limits {
        sockets: 4,
        names: 1,
        refusals: 1,
        intake: 1024,
        receive: 64,
        output: 1024,
        sends: 2,
        accepts: 1,
        backlog: 2,
        close_timeout: skein_lib::Duration::from_secs(5),
        retry: skein_lib::Duration::from_millis(1),
    }
}
fn disk_world(
    machine: skein_fake_machine::Machine,
    root: skein_fake_machine::Opened,
    case: FileCase,
    fail: bool,
) -> skein_world::Outcome<Disk, DiskMachine> {
    let mut world = skein_world::World::new(7, skein_sim::Config::calm(), DiskJudge, skein_world::Memory::Unchecked)
        .with_machine(DiskMachine(machine));
    world.spawn_root(skein_sim::Handle::new(root.raw()), |root| Disk::new(root, case));
    world.run_with_faults(|_, _, submissions| {
        if fail && submissions.iter().any(|submit| matches!(submit.kind, skein_io::kernel::Op::Write { .. })) {
            Some(skein_sim::Faults { no_space: 1000, ..skein_sim::Faults::NONE })
        } else {
            None
        }
    })
}
fn fresh_disk(case: FileCase, fail: bool) -> skein_world::Outcome<Disk, DiskMachine> {
    let mut machine = skein_fake_machine::Machine::for_user(1000);
    let root = machine.lay(&[]);
    disk_world(machine, root, case, fail)
}
fn save(key: &[u8], bytes: &[u8]) -> client::Request {
    let mut records = skein_lib::List::with_capacity(1);
    records.push(client::Record { key: key.into(), bytes: bytes.into() }).expect("one record fits");
    client::Request::Save { records }
}
fn erase(key: &[u8]) -> client::Request {
    let mut records = skein_lib::List::with_capacity(1);
    records.push(key.into()).expect("one key fits");
    client::Request::Erase { records }
}
#[test]
fn a_mixed_turn_releases_the_clients_next_request() {
    let outcome = fresh_disk(FileCase::ClientTurn, false);
    let disk = &outcome.procs[0];
    assert_eq!(disk.keyed, 1, "the client's storage barrier releases its next request");
    assert_eq!(disk.writes, MIXED_BATCHES, "the terminal erase and next save each receive Stored");
    assert_eq!(disk.write_acks, [0], "both batches share one write");
    assert_eq!(disk.sockets, 1);
}
#[test]
fn save_and_erase_each_receive_stored_before_the_exchange() {
    let outcome = fresh_disk(FileCase::SaveAndErase, false);
    let disk = &outcome.procs[0];
    assert_eq!(disk.writes, MIXED_BATCHES, "one acknowledgement per batch");
    assert_eq!(disk.write_acks, [0], "both batches share one write");
    assert_eq!(disk.sockets, 1, "the exchange is no longer held");
    assert_eq!(disk.failures, 0);
}
#[test]
fn batches_during_a_write_are_answered_by_the_next_write() {
    let outcome = fresh_disk(FileCase::DuringWrite, false);
    let disk = &outcome.procs[0];
    assert_eq!(disk.write_acks, [0, 1], "later batches are not acknowledged by the first write");
    assert_eq!(disk.writes, TOTAL_BATCHES, "one first batch and two later batches");
    assert_eq!(disk.sockets, 1);
    assert_eq!(disk.failures, 0);
}
#[test]
fn a_queued_refusal_follows_the_active_writes_acknowledgement() {
    let outcome = fresh_disk(FileCase::RefusedDuringWrite, false);
    let disk = &outcome.procs[0];
    assert_eq!(disk.answers, [true, false, false], "the first batch is stored before the two refused batches");
    assert_eq!(disk.write_acks, [0], "refused batches never start another write");
    assert_eq!(disk.sockets, 0, "the refused batches withhold the exchange");
}
#[test]
fn save_and_erase_each_receive_not_stored_on_failure() {
    let outcome = fresh_disk(FileCase::SaveAndErase, true);
    let disk = &outcome.procs[0];
    assert_eq!(disk.failures, MIXED_BATCHES, "one failure per batch");
    assert_eq!(disk.writes, 0);
    assert_eq!(disk.sockets, 0, "failed storage withholds the exchange");
}
#[test]
fn the_token_is_kept_and_read_back_at_start() {
    let mut saved = fresh_disk(FileCase::KeepToken, false);
    let mut machine = std::mem::replace(&mut saved.machine.0, skein_fake_machine::Machine::new());
    let root = machine.reopen_root(0);
    let loaded = disk_world(machine, root, FileCase::ReloadToken, false);
    assert_eq!(loaded.procs[0].token.as_deref(), Some(TOKEN.as_slice()));
}
#[test]
fn the_small_store_is_written_before_the_step_s_exchanges() {
    let outcome = fresh_disk(FileCase::Store, false);
    assert_eq!(outcome.procs[0].writes, 1);
    assert_eq!(outcome.procs[0].sockets, 1);
    assert_eq!(outcome.procs[0].failures, 0);
}
#[test]
fn a_failed_store_write_withholds_the_exchanges() {
    let outcome = fresh_disk(FileCase::Store, true);
    assert_eq!(outcome.procs[0].failures, 1);
    assert_eq!(outcome.procs[0].writes, 0);
    assert_eq!(outcome.procs[0].sockets, 0);
}

#[test]
fn an_exchange_past_its_deadline_is_answered_lost() {
    let mut peer = Peer::new();
    peer.exchange();
    peer.drive();
    let until = Time::ZERO.saturating_add(limits().deadline);
    assert_eq!(peer.shell.next_deadline(), Some(until));
    peer.shell.take(until, &mut peer.up, &mut peer.below);
    assert!(matches!(peer.up.pop(), Some(client::Event::Lost { exchange }) if exchange == Token::new(EXCHANGE)));
    assert_eq!(peer.shell.next_deadline(), None);
    peer.respond(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok");
    assert!(peer.up.is_empty());
}

#[test]
fn the_native_shell_sends_its_engine_s_address_as_host() {
    let shell = Shell::new(Settings {
        engine: skein_io::Addr::Ip("[::1]:7180".parse().expect("engine address")),
        directory: Box::from(b"private".as_slice()),
        limits: limits(),
        io: file_io_limits(),
    });
    let mut peer = Peer::new();
    peer.shell = shell.transport.connections;
    peer.exchange();
    peer.respond(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok");
    let text = std::str::from_utf8(&peer.wire.received).expect("HTTP request");
    assert!(text.to_ascii_lowercase().contains("host: [::1]:7180\r\n"), "{text}");
}

#[test]
fn transport_tags_socket_events_and_waits_for_connection() {
    const OWNER: u64 = 1 << 60;
    let engine = skein_io::Addr::Ip("127.0.0.1:8080".parse().expect("engine"));
    for (tag, expected) in [(0, Token::new(1)), (OWNER, Token::new(OWNER | 1))] {
        let mut transport = Transport::new(engine, &limits(), file_io_limits().sockets, tag);
        let mut events = Queue::with_capacity(1);
        let mut requests = Queue::with_capacity(1); // Exercise io output backpressure.
        transport.down(Time::ZERO, client::Request::OpenStream { credential: None });
        transport.iterate(Time::ZERO, &mut events, &mut requests);
        let Some(skein_io::Request::Connect { owner, .. }) = requests.pop() else { panic!("connect first") };
        assert_eq!(owner, expected);
        transport.iterate(Time::ZERO, &mut events, &mut requests);
        assert!(requests.is_empty());
        assert!(transport.owns(owner));
        let socket = Token::new(7);
        transport.event(Time::ZERO, skein_io::Event::Connecting { owner, socket });
        transport.event(Time::ZERO, skein_io::Event::Connected { owner });
        transport.iterate(Time::ZERO, &mut events, &mut requests);
        assert!(matches!(requests.pop(), Some(skein_io::Request::Stream { stream, .. }) if stream == socket));
    }
}

#[test]
fn a_binary_409_stream_refusal_reaches_the_client_without_a_line_ending() {
    let mut peer = Peer::new();
    peer.shell.down(Time::ZERO, client::Request::OpenStream { credential: None }, &mut peer.below);
    peer.respond(b"HTTP/1.1 409 Conflict\r\nContent-Length: 5\r\n\r\n\x01\0\0\0\x02");
    assert_eq!(
        peer.up.pop(),
        Some(client::Event::Streamed {
            message: client::Streamed::Full(client::StreamFull { bound: client::StreamBound::Transport, size: 2 })
        })
    );
    assert!(peer.up.is_empty());
}
