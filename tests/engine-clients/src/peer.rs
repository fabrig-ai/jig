//! io's stream records joined to an actual HTTP client, with scripted durability.
use jig_protocol_engine_clients_http as mapping;
mod closing;
use crate::fixtures;
use jig_client_protocol_wire::v19 as w;
use jig_domain_core as core;
use jig_domain_people as people;
use jig_protocol_engine_clients::{
    Component, Event, Kept, Limits, Request, SecretOwner, SecretsAnswer, SecretsRequest, Settings,
};
use skein_http::client as http;
use skein_lib::{
    Env, Queue, ReplyTo, Time, Token, Wall, Writer,
    base64::{self, Alphabet},
    stream::{Down, Read, Up},
};
use skein_world::stream::Wire;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
#[derive(Debug, PartialEq, Eq)]
enum ExchangeState {
    Active,
    Drained,
    Cut,
}
#[derive(Debug)]
struct Exchange {
    owner: Option<Token>,
    client: http::Client,
    events: Queue<http::Event>,
    below: Queue<Down>,
    to_engine: Wire,
    to_client: Wire,
    sent_engine: usize,
    sent_client: usize,
    upload: Option<Box<[u8]>>,
    body: Vec<u8>,
    status: Option<u16>,
    done: bool,
    streaming: bool,
    paused: bool,
    pause_after: Option<usize>,
    state: ExchangeState,
}
impl Exchange {
    fn new(method: skein_http::Method, target: &[u8], body: Box<[u8]>, bearer: Option<&[u8]>) -> Self {
        let limits = fixtures::client_limits();
        let mut headers = vec![skein_http::Header {
            name: Box::from(b"host".as_slice()),
            value: Box::from(fixtures::HOST.as_bytes()),
        }];
        if method == skein_http::Method::Post {
            headers.push(skein_http::Header {
                name: Box::from(b"content-type".as_slice()),
                value: Box::from(b"application/octet-stream".as_slice()),
            });
        }
        if let Some(bearer) = bearer {
            headers.push(skein_http::Header { name: Box::from(b"authorization".as_slice()), value: Box::from(bearer) });
        }
        let mut result = Self {
            owner: None,
            client: http::Client::new(&limits),
            events: Queue::with_capacity(8),
            below: Queue::with_capacity(8),
            to_engine: Wire::new(1),
            to_client: Wire::new(2),
            sent_engine: 0,
            sent_client: 0,
            upload: Some(body),
            body: Vec::new(),
            status: None,
            done: false,
            streaming: method == skein_http::Method::Get,
            paused: false,
            pause_after: None,
            state: ExchangeState::Active,
        };
        let length = result.upload.as_ref().expect("upload").len();
        result.down(http::Request::Call(http::Call {
            method,
            target: Box::from(target),
            headers: headers.into_boxed_slice(),
            body: if result.streaming || length == 0 {
                http::Body::None
            } else {
                http::Body::Length(u64::try_from(length).expect("short body"))
            },
            close: false,
        }));
        if !result.streaming && length != 0 {
            result.down(http::Request::Upload(Down::Demand {
                read: Read::Nothing,
                room: u32::try_from(length).expect("short body"),
            }));
        }
        result
    }
    fn env() -> Env<http::Limits> {
        Env { now: Time::ZERO, wall: Wall::EPOCH, limits: fixtures::client_limits() }
    }
    fn down(&mut self, request: http::Request) {
        http::down(&mut self.client, &Self::env(), request, &mut self.events, &mut self.below);
    }
    fn progress(&self) -> (u32, u32, usize, usize, usize) {
        (self.events.len(), self.below.len(), self.to_client.trace.len(), self.to_engine.trace.len(), self.body.len())
    }
    fn advance(&mut self) -> (Option<Up>, bool) {
        if self.state == ExchangeState::Cut {
            return (None, false);
        }
        let before = self.progress();
        if let Some(event) = self.events.pop() {
            match event {
                http::Event::Upload(Up::Room) => {
                    let upload = self.upload.take().expect("upload once");
                    self.down(http::Request::Upload(Down::Send(upload)));
                    self.down(http::Request::Upload(Down::Finish));
                }
                http::Event::Response(response) => {
                    self.status = Some(response.status);
                    self.read();
                }
                http::Event::Body(Up::Bytes(bytes)) => {
                    self.body.extend_from_slice(&bytes);
                    self.read();
                }
                http::Event::Body(Up::End) | http::Event::Closed => {}
                http::Event::Done(_) => self.done = true,
                http::Event::Failed(error) => panic!("client failed: {error:?}"),
                http::Event::Upload(Up::Failed(_) | Up::End | Up::Bytes(_))
                | http::Event::Body(Up::Failed(_) | Up::Room) => panic!("unexpected body event"),
            }
        }
        if let Some(down) = self.below.pop() {
            self.to_client.take(down);
        }
        let bytes = &self.to_client.received[self.sent_client..];
        self.to_engine.write(bytes);
        self.sent_client = self.to_client.received.len();
        let bytes = &self.to_engine.received[self.sent_engine..];
        self.to_client.write(bytes);
        self.sent_engine = self.to_engine.received.len();
        if let Some(up) = self.to_client.answer() {
            http::up(&mut self.client, &Self::env(), up, &mut self.events, &mut self.below);
        }
        if self.pause_after.is_some_and(|limit| self.body.len() >= limit) {
            self.paused = true;
        }
        let up = if self.paused || self.state != ExchangeState::Active { None } else { self.to_engine.answer() };
        let changed = before != self.progress();
        (up, changed)
    }
    fn read(&mut self) {
        self.down(http::Request::Body(Down::Demand {
            read: Read::Arrived { max: fixtures::client_limits().read },
            room: 0,
        }));
    }
}
#[derive(Debug)]
pub struct Peer {
    server: jig_test_service::server::Server,
    server_out: Queue<jig_test_service::server::Routed>,
    mount: mapping::Mount,
    refused: skein_lib::Map<Token, ()>,
    mount_out: Queue<mapping::Request>,
    component: Component,
    env: Env<Limits>,
    out: Queue<Request>,
    sockets: BTreeMap<Token, Exchange>,
    next_socket: u64,
    links: Vec<Box<[u8]>>,
    core: VecDeque<core::Event>,
    application: VecDeque<jig_protocol_engine_clients::ToApplication>,
    keep: Option<(SecretOwner, Kept)>,
    digests: BTreeMap<SecretOwner, Kept>,
    hold_gets: bool,
    gets: VecDeque<(SecretOwner, Option<Kept>)>,
    sign_ins: BTreeMap<u64, u64>,
    watch_sign_ins: BTreeMap<Token, u64>,

    last_core: Option<String>,
    last_watch: Option<core::documents::Watched>,
    bearer: Option<Box<[u8]>>,
    streams: BTreeMap<[u8; 16], Token>,
    next_watch: u64,
    in_flight: BTreeSet<Token>,
    changed: BTreeMap<Token, core::documents::Document>,
    unwatching: BTreeSet<Token>,
    mints: BTreeMap<Token, Result<(skein_lib::Secret, Wall), jig_protocol_engine_clients::MintRefusal>>,
    mint_identity: Option<people::Identity>,
    next_mint: u64,
    closed: usize,
    close_counts: BTreeMap<Token, usize>,
    hold_erases: bool,
    hold_closes: bool,
    closing_sockets: BTreeSet<Token>,
    erases: VecDeque<SecretOwner>,
    carries: VecDeque<(Token, skein_lib::Secret)>,
    carried: BTreeMap<u64, skein_lib::Secret>,
    next_carried: u64,
}
impl Peer {
    #[must_use]
    pub fn started(settings: &Settings) -> Self {
        Self::with_limits(settings, fixtures::limits())
    }
    #[must_use]
    pub fn with_limits(settings: &Settings, limits: Limits) -> Self {
        let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits };
        let mut out = Queue::with_capacity(64);
        let component = Component::new(fixtures::copy_settings(settings), &env, &mut out).expect("component starts");
        let mut peer = Self {
            component,
            server: fixtures::server(),
            server_out: Queue::with_capacity(64),
            mount: fixtures::mount(),
            refused: skein_lib::Map::with_capacity(fixtures::server_limits().connections),
            mount_out: Queue::with_capacity(64),
            env,
            out,
            sockets: BTreeMap::new(),
            next_socket: 100,
            links: Vec::new(),
            core: VecDeque::new(),
            application: VecDeque::new(),
            keep: None,
            digests: BTreeMap::new(),
            hold_gets: false,
            gets: VecDeque::new(),
            sign_ins: BTreeMap::new(),
            watch_sign_ins: BTreeMap::new(),

            last_core: None,
            last_watch: None,
            bearer: None,
            streams: BTreeMap::new(),
            next_watch: 1,
            in_flight: BTreeSet::new(),
            changed: BTreeMap::new(),
            unwatching: BTreeSet::new(),
            mints: BTreeMap::new(),
            mint_identity: None,
            next_mint: 1,
            closed: 0,
            close_counts: BTreeMap::new(),
            hold_erases: false,
            hold_closes: false,
            closing_sockets: BTreeSet::new(),
            erases: VecDeque::new(),
            carries: VecDeque::new(),
            carried: BTreeMap::new(),
            next_carried: 1 << 63,
        };
        peer.pump();
        peer
    }
    pub fn event(&mut self, event: Event) {
        if let Event::Stop = &event {
            self.server.stop(&mut self.server_out);
        }
        if let Event::Close = &event {
            self.server.close(&mut self.server_out);
        }
        if let Event::Abort = &event {
            self.server.abort(&mut self.server_out);
        }
        self.component.down(&self.env, event, &mut self.out);
        self.pump();
    }
    fn mount_env(&self) -> Env<mapping::Limits> {
        Env { now: self.env.now, wall: self.env.wall, limits: fixtures::http_limits() }
    }
    fn mount_event(&mut self, event: mapping::Event) {
        self.mount.down(&self.mount_env(), event, &mut self.mount_out);
    }
    fn mount_step(&mut self) -> bool {
        let mut active = self.mount.up(&self.mount_env(), &mut self.mount_out);
        while let Some(request) = self.mount_out.pop() {
            active = true;
            match request {
                mapping::Request::Http { call, request } => self.server.answer(call, request, &mut self.server_out),
                mapping::Request::Component(message) => {
                    if self.closed == 0 {
                        self.component.down(&self.env, Event::Transport(message), &mut self.out);
                    }
                }
            }
        }
        active
    }
    fn server_step(&mut self) -> bool {
        let mut active = false;
        self.server.clock(self.env.now, self.env.wall);
        self.server.fire(&mut self.server_out);
        active |= self.mount_step();
        active |= self.server.advance(&mut self.server_out);
        if let Some(routed) = self.server_out.pop() {
            active = true;
            match routed {
                jig_test_service::server::Routed::Io(request) => self.io(request),
                jig_test_service::server::Routed::Call { call, head } => {
                    if self.mount.room() > 0 {
                        self.mount_event(mapping::Event::Call { call, head });
                    } else {
                        self.refused.insert(call, ()).expect("bounded refused server calls");
                        self.server.answer(call, skein_http::server::Request::Discard, &mut self.server_out);
                        self.server.answer(
                            call,
                            skein_http::server::Request::Respond(skein_http::server::Response {
                                status: 503,
                                headers: Box::new([]),
                                body: skein_http::server::Body::None,
                                close: true,
                            }),
                            &mut self.server_out,
                        );
                    }
                }
                jig_test_service::server::Routed::Http { call, event } => {
                    if self.refused.get(&call).is_some() {
                        match event {
                            skein_http::server::Event::Done(_)
                            | skein_http::server::Event::Failed(_)
                            | skein_http::server::Event::Closed => {
                                self.refused.remove(&call);
                                self.server.done(call, &mut self.server_out);
                            }
                            skein_http::server::Event::Call(_)
                            | skein_http::server::Event::Ended
                            | skein_http::server::Event::Body(_)
                            | skein_http::server::Event::Reply(_)
                            | skein_http::server::Event::Refused(_) => {}
                        }
                    } else {
                        self.mount_event(mapping::Event::Http { call, event });
                    }
                }
            }
        }
        active
    }
    #[expect(
        clippy::too_many_lines,
        reason = "one exhaustive pump of the component, transport, store and scripted core"
    )]
    fn pump(&mut self) {
        // Every packet is tiny; this bound includes the seeded wire's harmless delays.
        let mut quiet = 0;
        for _ in 0..512 {
            let mut active = false;
            active |= self.server_step();
            if self.closed == 0 {
                active |= self.component.up(&self.env, &mut self.out);
            }
            if let Some(request) = self.out.pop() {
                active = true;
                match request {
                    Request::TakeMoved { exchange, .. } => {
                        self.component.down(&self.env, Event::MovedTaken { exchange, record: None }, &mut self.out);
                    }
                    Request::Transport(message) => self.mount_event(mapping::Event::Component(message)),
                    Request::Core(event) => {
                        if let core::Event::Delivered { watcher } = event.as_ref() {
                            self.in_flight.remove(watcher);
                            if let Some(document) = self.changed.remove(watcher) {
                                self.in_flight.insert(*watcher);
                                self.component.down(
                                    &self.env,
                                    Event::Delivery { watcher: *watcher, document: Box::new(document) },
                                    &mut self.out,
                                );
                            } else if self.unwatching.remove(watcher) {
                                self.component.down(
                                    &self.env,
                                    Event::WatchEnded { watcher: *watcher, why: jig_domain_views::End::Unwatched },
                                    &mut self.out,
                                );
                            }
                        } else if let core::Event::Unwatch { watcher } = event.as_ref() {
                            self.changed.remove(watcher);
                            if self.in_flight.contains(watcher) {
                                self.unwatching.insert(*watcher);
                            } else {
                                self.component.down(
                                    &self.env,
                                    Event::WatchEnded { watcher: *watcher, why: jig_domain_views::End::Unwatched },
                                    &mut self.out,
                                );
                            }
                        }
                        self.last_core = Some(format!("{event:?}"));
                        self.core.push_back(*event);
                    }
                    Request::Secrets(SecretsRequest::Keep { owner, value }) => self.keep = Some((owner, value)),
                    Request::Secrets(SecretsRequest::Get { owner }) => {
                        let value = self.digests.get(&owner).cloned();
                        if self.hold_gets {
                            self.gets.push_back((owner, value));
                        } else {
                            self.component.down(
                                &self.env,
                                Event::Secrets(SecretsAnswer::Got { owner, value }),
                                &mut self.out,
                            );
                        }
                    }
                    Request::Secrets(
                        SecretsRequest::Hold { exchange, value } | SecretsRequest::Carry { exchange, value, .. },
                    ) => {
                        self.carries.push_back((exchange, value));
                    }
                    Request::Secrets(SecretsRequest::Erase { owner }) => {
                        self.erases.push_back(owner);
                        if !self.hold_erases {
                            self.erase_one();
                        }
                    }
                    Request::Link { secret } => self.links.push(fixtures::printed(&secret)),
                    Request::Minted { exchange, credential, expires } => {
                        self.mints.insert(exchange, Ok((credential, expires)));
                    }
                    Request::MintRefused { exchange, why } => {
                        self.mints.insert(exchange, Err(why));
                    }
                    Request::Redeemed { .. } | Request::Claimed { .. } => {
                        panic!("provider endpoints use their own story")
                    }
                    Request::Closed => self.closed += 1,
                    Request::Application(request) => self.application.push_back(request),
                }
            }
            let mut ups = Vec::new();
            for exchange in self.sockets.values_mut() {
                if let Some(owner) = exchange.owner {
                    let (up, changed) = exchange.advance();
                    active |= changed;
                    if let Some(up) = up {
                        active = true;
                        ups.push((owner, up));
                    }
                }
            }
            for (owner, up) in ups {
                self.server.up(skein_io::Event::Stream { owner, up }, &mut self.server_out);
            }
            quiet = if active { 0 } else { quiet + 1 };
            if quiet == 16 {
                break;
            }
        }
    }
    fn io(&mut self, request: skein_io::Request) {
        match request {
            skein_io::Request::Listen { owner, addr } => self
                .server
                .up(skein_io::Event::Listening { owner, listener: Token::new(99), addr }, &mut self.server_out),
            skein_io::Request::Bind { socket, owner } => {
                self.sockets.get_mut(&socket).expect("accepted socket").owner = Some(owner);
            }
            skein_io::Request::Stream { stream, down } => {
                let exchange = self.sockets.get_mut(&stream).expect("socket stream");
                if exchange.state == ExchangeState::Active {
                    exchange.to_engine.take(down);
                }
            }
            skein_io::Request::Close { entity } => self.close_socket(entity, ExchangeState::Drained),
            skein_io::Request::Abort { entity } => self.close_socket(entity, ExchangeState::Cut),
            skein_io::Request::Reject { .. } => panic!("unexpected rejected socket"),
            skein_io::Request::Connect { .. }
            | skein_io::Request::Spawn { .. }
            | skein_io::Request::Signal { .. }
            | skein_io::Request::Usage { .. }
            | skein_io::Request::Window { .. }
            | skein_io::Request::Output { .. } => panic!("unexpected io request"),
        }
    }
    #[must_use]
    pub fn signed_in(settings: &Settings, sign_in: u64) -> Self {
        let mut peer = Self::started(settings);
        let exchange = peer.post_link(&peer.printed_link());
        peer.answer_signed_in(sign_in, fixtures::FIRST_PERSON);
        peer.answer_kept(sign_in);
        let answer = peer.sign_in_answer(exchange);
        let mut bearer = Writer::new(61);
        bearer.put(b"Bearer ").expect("prefix");
        base64::encode(Alphabet::Url, answer.token().as_ref().expect("native token").expose(), &mut bearer)
            .expect("bearer");
        peer.bearer = Some(bearer.finish());
        peer
    }
    pub fn open_stream(&mut self) -> [u8; 16] {
        let socket = Token::new(self.next_socket);
        self.next_socket += 1;
        self.sockets.insert(
            socket,
            Exchange::new(skein_http::Method::Get, b"/jig/v1/stream", Box::new([]), self.bearer.as_deref()),
        );
        self.server.up(
            skein_io::Event::Accepted { owner: Token::new(0), socket, peer: fixtures::address() },
            &mut self.server_out,
        );
        self.pump();
        let events = self.stream_records(socket);
        let Some(w::StreamEvent::Hello(hello)) = events.first() else {
            panic!("hello first: {:?}", self.sockets.get(&socket).expect("stream").body);
        };
        let name = *hello.stream();
        self.streams.insert(name, socket);
        name
    }
    fn stream_records(&self, socket: Token) -> Vec<w::StreamEvent> {
        let body = &self.sockets.get(&socket).expect("stream").body;
        let mut events = Vec::new();
        let mut rest = body.as_slice();
        while let Some(end) = skein_lib::bytes::find(rest, b"\n\n") {
            let block = &rest[..end];
            rest = &rest[end + 2..];
            if let Some(value) = block.split(|byte| *byte == b'\n').find_map(|line| line.strip_prefix(b"data: ")) {
                let bytes = base64::decode(Alphabet::Standard, value, 65536).expect("base64 record");
                events.push(
                    w::StreamEvent::decode(&self.env.limits.wire, &mut skein_lib::Reader::new(&bytes))
                        .expect("whole wire event"),
                );
            }
        }
        events
    }
    #[must_use]
    pub fn events(&self, stream: [u8; 16]) -> Vec<w::StreamEvent> {
        self.stream_records(self.streams[&stream])
    }
    #[must_use]
    pub fn stream_bytes(&self, stream: [u8; 16]) -> &[u8] {
        &self.sockets[&self.streams[&stream]].body
    }
    pub fn request_watch_event(&mut self, stream: [u8; 16], subject: &w::Subject) -> (Token, core::Event) {
        let mut record = Writer::new(usize::try_from(subject.measure()).expect("subject size"));
        subject.encode(&mut record).expect("subject");
        let subject = w::Owned::new(
            &self.env.limits.wire,
            w::OwnedParts { owner: w::Owner::Jig, version: 19, record: record.finish() },
        )
        .expect("subject owner");
        let open = w::Open::new(&self.env.limits.wire, w::OpenParts { stream, watch: self.next_watch, subject })
            .expect("open");
        self.next_watch += 1;
        let mut body = Writer::new(usize::try_from(open.measure()).expect("open size"));
        open.encode(&mut body).expect("open record");
        let bearer = self.bearer.clone().expect("signed in");
        let exchange = self.post(b"/jig/v1/watch", body.finish(), Some(&bearer));
        let event = self.take_core().expect("watch request");
        (exchange, event)
    }
    pub fn request_watch(&mut self, stream: [u8; 16], subject: &w::Subject) -> (Token, Token) {
        let (exchange, event) = self.request_watch_event(stream, subject);
        self.last_watch = if let core::Event::Watch { subject, .. } = &event { Some(subject.clone()) } else { None };
        let (core::Event::Watch { watcher, sign_in, .. } | core::Event::WatchRun { watcher, sign_in, .. }) = event
        else {
            panic!("watch request: {event:?}");
        };
        assert_eq!(sign_in, fixtures::FIRST_SIGN_IN);
        self.watch_sign_ins.insert(watcher, sign_in);
        (exchange, watcher)
    }
    pub fn open_watch(&mut self, stream: [u8; 16], subject: &w::Subject) -> Token {
        let (exchange, watcher) = self.request_watch(stream, subject);
        assert!(self.answered(exchange).is_none());
        self.event(Event::Admitted { watcher });
        assert_eq!(
            w::OpenAnswer::decode(
                &self.env.limits.wire,
                &mut skein_lib::Reader::new(self.answered(exchange).expect("open answer"))
            ),
            Ok(w::OpenAnswer::Admitted)
        );
        watcher
    }
    pub fn refuse_watch(&mut self, watcher: Token, why: core::WatchRefusal) {
        self.event(Event::WatchRefused { watcher, why });
    }
    pub fn release_delivery(&mut self, watcher: Token, document: core::documents::Document) {
        if self.in_flight.insert(watcher) {
            self.event(Event::Delivery { watcher, document: Box::new(document) });
        } else {
            self.changed.insert(watcher, document);
        }
    }
    #[must_use]
    pub fn requested_subject(&self) -> Option<&core::documents::Watched> {
        self.last_watch.as_ref()
    }
    pub fn release_current_frame(&mut self, watcher: Token) {
        let party = self.sign_ins[&self.watch_sign_ins[&watcher]];
        let core::documents::Document::Frame(mut frame) = fixtures::me_document(7) else {
            panic!("frame fixture");
        };
        frame.party = party;
        self.release_delivery(watcher, core::documents::Document::Frame(frame));
    }
    pub fn restart(&mut self, settings: &Settings) {
        self.component.down(&self.env, Event::Abort, &mut self.out);
        self.pump();
        self.sockets.clear();
        self.streams.clear();
        self.core.clear();
        self.watch_sign_ins.clear();
        self.last_watch = None;
        self.component = Component::new(fixtures::copy_settings(settings), &self.env, &mut self.out).expect("restart");
        self.server = fixtures::server();
        self.server_out = Queue::with_capacity(64);
        self.mount = fixtures::mount();
        self.mount_out = Queue::with_capacity(64);
        self.closed = 0;
        self.pump();
    }
    pub fn release_text(&mut self, watcher: Token) {
        self.event(Event::RunText {
            writing: None,
            watcher,
            turn: jig_domain_views::TurnName { attempt: 7, turn: 7, retry: 7 },
            from: 7,
            to: 7,
            text: Box::new([]),
            cut: false,
            call: None,
        });
    }
    #[must_use]
    pub fn taken(&self, watcher: Token) -> usize {
        self.core
            .iter()
            .filter(|event| matches!(event, core::Event::Delivered {watcher: value} if *value == watcher))
            .count()
    }
    #[must_use]
    pub fn text_taken(&self, watcher: Token) -> usize {
        self.core
            .iter()
            .filter(|event| matches!(event, core::Event::StreamDelivered {watcher: value} if *value == watcher))
            .count()
    }
    #[must_use]
    pub fn unwatched(&self, watcher: Token) -> bool {
        self.core.iter().any(|event| matches!(event, core::Event::Unwatch {watcher: value} if *value == watcher))
    }
    pub fn stop_reading(&mut self, stream: [u8; 16]) {
        self.sockets.get_mut(&self.streams[&stream]).expect("stream").paused = true;
    }
    pub fn read_all(&mut self, stream: [u8; 16]) {
        let exchange = self.sockets.get_mut(&self.streams[&stream]).expect("stream");
        exchange.paused = false;
        exchange.pause_after = None;
        self.pump();
    }
    pub fn read_one_piece(&mut self, stream: [u8; 16]) {
        let exchange = self.sockets.get_mut(&self.streams[&stream]).expect("stream");
        exchange.pause_after = Some(exchange.body.len() + 1);
    }
    pub fn cut_stream(&mut self, stream: [u8; 16]) {
        let exchange = self.sockets.get_mut(&self.streams[&stream]).expect("stream");
        exchange.state = ExchangeState::Cut;
        let owner = exchange.owner.expect("bound stream");
        self.server.up(
            skein_io::Event::Stream { owner, up: Up::Failed(skein_lib::stream::Fault::Other) },
            &mut self.server_out,
        );
        self.pump();
    }
    pub fn advance_time(&mut self, span: skein_lib::Duration) {
        self.env.now = self.env.now.saturating_add(span);
        self.component.fire(&self.env, &mut self.out);
        self.pump();
    }
    #[must_use]
    pub fn printed_link(&self) -> Box<[u8]> {
        self.links.last().expect("printed link").clone()
    }
    pub fn post_link(&mut self, link: &[u8]) -> Token {
        self.post(b"/jig/v1/sign-in/link", self.link_body(link), None)
    }
    #[must_use]
    pub fn link_body(&self, link: &[u8]) -> Box<[u8]> {
        let secret = link.split(|byte| *byte == b'=').next_back().expect("link query");
        let secret = base64::decode(Alphabet::Url, secret, 32).expect("link secret");
        let record = w::LinkSignIn::new(&self.env.limits.wire, w::LinkSignInParts { secret }).expect("bounded secret");
        let mut out = Writer::new(usize::try_from(record.measure()).expect("record size"));
        record.encode(&mut out).expect("encoded link");
        out.finish()
    }
    pub fn post(&mut self, target: &[u8], body: Box<[u8]>, bearer: Option<&[u8]>) -> Token {
        let socket = Token::new(self.next_socket);
        self.next_socket += 1;
        self.sockets.insert(socket, Exchange::new(skein_http::Method::Post, target, body, bearer));
        self.server.up(
            skein_io::Event::Accepted { owner: Token::new(0), socket, peer: fixtures::address() },
            &mut self.server_out,
        );
        self.pump();
        socket
    }
    pub fn cut_exchange(&mut self, socket: Token) {
        let exchange = self.sockets.get_mut(&socket).expect("exchange");
        exchange.state = ExchangeState::Cut;
        let owner = exchange.owner.expect("bound exchange");
        self.server.up(
            skein_io::Event::Stream { owner, up: Up::Failed(skein_lib::stream::Fault::Other) },
            &mut self.server_out,
        );
        self.pump();
    }
    pub fn hold_gets(&mut self) {
        self.hold_gets = true;
    }
    pub fn answer_get(&mut self) {
        let (owner, value) = self.gets.pop_front().expect("pending Get");
        self.event(Event::Secrets(SecretsAnswer::Got { owner, value }));
    }
    pub fn keep_digest(&mut self, owner: SecretOwner, token: &[u8], expires: Wall) {
        self.digests.insert(
            owner,
            Kept { digest: skein_lib::Secret::new(Box::new(skein_lib::sha256::digest(token))), expires },
        );
    }
    fn take_sign_in(&mut self) -> ReplyTo {
        let event = self.core.pop_front().expect("core sign-in");
        // The assertion first checks the variant; destructuring reads its public vocabulary.
        assert!(matches!(event, core::Event::SignIn { .. }));
        if let core::Event::SignIn { held: None, reply_to, identity } = event {
            assert_eq!(identity, self.mint_identity.take().unwrap_or_else(fixtures::first_owner));
            reply_to
        } else {
            panic!("expected sign-in")
        }
    }
    pub fn answer_signed_in(&mut self, sign_in: u64, person: u64) {
        let to = self.take_sign_in();
        self.sign_ins.insert(sign_in, person);

        self.event(Event::Reply {
            to,
            reply: people::Reply::SignedIn { sign_in, person, expires: fixtures::expires() },
        });
    }
    pub fn refuse_sign_in(&mut self) {
        let to = self.take_sign_in();
        self.event(Event::SignInRefused { to });
    }
    pub fn answer_kept(&mut self, sign_in: u64) {
        let (number, value) = self.keep.take().expect("digest keep requested");
        assert_eq!(number, SecretOwner::SignIn(sign_in));
        self.digests.insert(number, value);
        self.event(Event::Secrets(SecretsAnswer::Kept { owner: SecretOwner::SignIn(sign_in) }));
    }
    pub fn fail_keep(&mut self) {
        self.keep = None;
        self.event(Event::Secrets(SecretsAnswer::Failed));
    }
    #[must_use]
    pub fn digest_kept(&self) -> Option<u64> {
        self.keep.as_ref().map(|(owner, _)| match owner {
            SecretOwner::SignIn(number) | SecretOwner::Service(number) => *number,
            SecretOwner::Carried(_) => panic!("a carried value is kept by its own request"),
        })
    }
    #[must_use]
    pub fn answered(&self, exchange: Token) -> Option<&[u8]> {
        let exchange = self.sockets.get(&exchange).expect("exchange socket");
        exchange.done.then_some(&exchange.body)
    }
    #[must_use]
    pub fn sign_in_answer(&self, exchange: Token) -> w::SignedIn {
        match w::SignInAnswer::decode(
            &self.env.limits.wire,
            &mut skein_lib::Reader::new(self.answered(exchange).expect("answered")),
        )
        .expect("wire answer")
        {
            w::SignInAnswer::SignedIn(answer) => answer,
            w::SignInAnswer::Refused => panic!("sign-in refused"),
            w::SignInAnswer::NotReady => panic!("sign-in not ready"),
            w::SignInAnswer::Busy => panic!("sign-in busy"),
            w::SignInAnswer::Full(_) => panic!("sign-ins full"),
        }
    }
    pub fn answer_request(&mut self, to: ReplyTo, reply: people::Reply) {
        self.event(Event::Reply { to, reply });
    }
    pub fn answer_read(&mut self, reply: ReplyTo, answer: core::reads::Answer) {
        self.event(Event::Read { reply, answer });
    }
    pub fn take_core(&mut self) -> Option<core::Event> {
        self.core.pop_front()
    }
    #[must_use]
    pub fn core_record(&self) -> &str {
        self.last_core.as_deref().expect("core sign-in")
    }
}

impl Peer {
    pub fn take_application(&mut self) -> Option<jig_protocol_engine_clients::ToApplication> {
        self.application.pop_front()
    }
}
