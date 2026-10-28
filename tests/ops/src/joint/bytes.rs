//! The real clients component and native HTTP machines around the Ops engine.
use super::front::{ApplicationDocument, Joint, Subject, application_document, subject_route};
use crate::conformance::Story;
use jig_client::{Event, Message, Request, Streamed};
use jig_client_protocol_world::{fixtures, native::wire::Wire, net::Net};
use jig_client_shell_native::{Conn, Connections};
use jig_client_world::harness::{ClientId, Receipt};
use jig_domain_core as core;
use jig_ops_client_domain as d;
use jig_ops_domain as root;
use jig_protocol_engine_clients::{self as clients, FromApplication, SecretsAnswer, SecretsRequest};
use jig_protocol_engine_clients_http as http;
use skein_lib::{Env, Queue, Time, Token, Wall};
use std::collections::BTreeMap;
#[derive(PartialEq, Eq)]
enum Callback {
    Reply(Token),
    Record(Box<[u8]>),
    Gone,
    Heartbeat,
}
struct Side {
    connections: Connections,
    up: Queue<Event>,
    down: Queue<(Conn, skein_lib::stream::Down)>,
    wire: Wire,
    stored: Vec<d::store::Record>,
    stream_epoch: u64,
    family_reads: BTreeMap<Token, u16>,
    family_requests: BTreeMap<Token, u16>,
}
fn connections_limits() -> jig_client_shell_native::Limits {
    let mut limits = fixtures::connections_limits();
    limits.exchanges = super::fixture::limits().client.exchanges;
    limits
}
impl Side {
    fn new() -> Self {
        Self {
            connections: Connections::new(&connections_limits(), fixtures::HOST),
            up: Queue::with_capacity(256),
            down: Queue::with_capacity(256),
            wire: Wire::default(),
            stored: Vec::new(),
            stream_epoch: 0,
            family_reads: BTreeMap::new(),
            family_requests: BTreeMap::new(),
        }
    }
    fn send(&mut self, now: Time, request: Request) {
        use jig_client_protocol_wire::v19 as w;
        use skein_lib::Reader;
        if matches!(&request, Request::CloseStream) {
            self.stream_epoch += 1;
        }
        if let Request::Exchange { exchange, message, .. } = &request {
            match message {
                Message::Read(bytes) => {
                    let value = w::Owned::decode(&w::CEILINGS, &mut Reader::new(bytes)).expect("whole read bytes");
                    if let w::Owner::Family(family) = value.owner() {
                        self.family_reads.insert(*exchange, family.number());
                    }
                }
                Message::Request(bytes) => {
                    let value = w::Keyed::decode(&w::CEILINGS, &mut Reader::new(bytes)).expect("whole keyed bytes");
                    if let w::Owner::Family(family) = value.owner() {
                        self.family_requests.insert(*exchange, family.number());
                    }
                }
                Message::SignInLink(_)
                | Message::SignInService
                | Message::Authorize(_)
                | Message::SignOut
                | Message::Watch(_)
                | Message::Unwatch(_)
                | Message::StreamGone(_) => {}
            }
        }
        self.wire.sent_without_family(&request);
        self.connections.down(now, request, &mut self.down);
    }
    fn receipts(&mut self, event: &Event) -> Vec<Receipt> {
        use jig_client_protocol_wire::v19 as w;
        use skein_lib::Reader;
        let mut receipts: Vec<_> = self
            .wire
            .received_with(event, {
                let limits = super::fixture::limits().client;
                jig_client_protocol_link::Limits::new(limits.domain, limits.exchanges).expect("client receiving bounds")
            })
            .iter()
            .flat_map(jig_client_protocol_world::observe::receipts)
            .collect();
        match event {
            Event::Reply { exchange, message } => {
                if self.family_requests.remove(exchange).is_some() {
                    let value =
                        w::KeyedAnswer::decode(&w::CEILINGS, &mut Reader::new(message)).expect("whole family answer");
                    receipts.push(Receipt::Answer {
                        attempt: *exchange,
                        final_answer: matches!(value, w::KeyedAnswer::FinalAnswer(_)),
                        words: None,
                    });
                }
                if self.family_reads.remove(exchange) == Some(2)
                    && let Ok(w::ReadAnswer::Value(value)) =
                        w::ReadAnswer::decode(&w::CEILINGS, &mut Reader::new(message))
                {
                    receipts.extend(crate::client::app::infrastructure_read_received(value.record()));
                }
            }
            Event::Streamed { message: Streamed::Record(bytes) } => {
                if let Ok(w::StreamEvent::Delivery(value)) =
                    w::StreamEvent::decode(&w::CEILINGS, &mut Reader::new(bytes))
                    && matches!(value.document().owner(), w::Owner::Family(family) if family.number() == 2)
                {
                    receipts.extend(crate::client::app::infrastructure_received(value.document().record()));
                }
            }
            Event::Start { .. }
            | Event::StartUnlinked { .. }
            | Event::SignIn { .. }
            | Event::Authorize { .. }
            | Event::SignOut
            | Event::SignOutLost
            | Event::Watch { .. }
            | Event::Unwatch { .. }
            | Event::Read { .. }
            | Event::Send { .. }
            | Event::Carry { .. }
            | Event::Lost { .. }
            | Event::Full { .. }
            | Event::Streamed { .. }
            | Event::StreamGone
            | Event::Stored
            | Event::NotStored
            | Event::Up { .. } => {}
        }
        receipts
    }
}
pub(super) struct Bytes {
    component: clients::Component,
    server: jig_test_service::server::Server,
    server_out: Queue<jig_test_service::server::Routed>,
    mount: http::Mount,
    refused: skein_lib::Map<Token, ()>,
    mount_out: Queue<http::Request>,
    env: Env<clients::Limits>,
    out: Queue<clients::Request>,
    net: Net,
    sides: BTreeMap<ClientId, Side>,
    secrets: BTreeMap<clients::SecretOwner, clients::Kept>,
    link: Box<[u8]>,
    rules: jig_client_protocol_world::referee::Rules,
    subjects: BTreeMap<Token, Subject>,
    reads: BTreeMap<Token, Subject>,
    receipts: Vec<(ClientId, Callback, u64, Vec<Receipt>)>,
    pending_admissions: Vec<Token>,
    ready_admissions: Vec<Token>,
}
impl Bytes {
    pub fn new(seed: u64, story: Story, rows: u32) -> Self {
        let mut settings = fixtures::settings(rows);
        let (subject, login): (u64, &[u8]) = match story {
            Story::Environment => (8, b"developer"),
            Story::Alert | Story::Night => (7, b"oncall"),
        };
        settings.link = Some(jig_domain_people::Identity {
            key: jig_domain_people::IdentityKey { provider: 0, subject: subject.to_be_bytes().into() },
            login: login.into(),
            name: Box::new([]),
        });
        settings.served = jig_client_world::test::list([
            clients::Served { family: None, first: 19, last: 19 },
            clients::Served { family: Some(1), first: 1, last: 1 },
            clients::Served { family: Some(2), first: 1, last: 1 },
        ]);
        let mut limits = fixtures::engine_limits();
        limits.families = 3;
        limits.watches_per_stream = 64;
        // A page change overlaps its old closes with new watches and reads.
        limits.exchanges = connections_limits().exchanges;
        limits.exchanges_per_client = connections_limits().exchanges;
        let env = Env { now: Time::ZERO, wall: fixtures::wall(Time::ZERO), limits };
        let mut out = Queue::with_capacity(512);
        let component = clients::Component::new(settings, &env, &mut out).expect("Ops clients component");
        Self {
            component,
            server: fixtures::server(),
            server_out: Queue::with_capacity(512),
            mount: fixtures::mount(),
            refused: skein_lib::Map::with_capacity(fixtures::server_limits().connections),
            mount_out: Queue::with_capacity(512),
            env,
            out,
            net: Net::new(seed),
            sides: BTreeMap::new(),
            secrets: BTreeMap::new(),
            link: Box::new([]),
            rules: jig_client_protocol_world::referee::Rules::default(),
            subjects: BTreeMap::new(),
            reads: BTreeMap::new(),
            receipts: Vec::new(),
            pending_admissions: Vec::new(),
            ready_admissions: Vec::new(),
        }
    }
    fn mount_env(&self) -> Env<http::Limits> {
        Env { now: self.env.now, wall: self.env.wall, limits: fixtures::http_limits() }
    }
    fn mount_event(&mut self, event: http::Event) {
        self.mount.down(&self.mount_env(), event, &mut self.mount_out);
    }
    fn mount_step(&mut self) -> bool {
        let mut active = self.mount.up(&self.mount_env(), &mut self.mount_out);
        while let Some(request) = self.mount_out.pop() {
            active = true;
            match request {
                http::Request::Http { call, request } => self.server.answer(call, request, &mut self.server_out),
                http::Request::Component(message) => self.event(clients::Event::Transport(message)),
            }
        }
        active
    }
    fn event(&mut self, event: clients::Event) {
        self.component.down(&self.env, event, &mut self.out);
    }
    pub fn bind(&mut self, client: ClientId) {
        self.sides.insert(client, Side::new());
    }
    pub fn starting(&mut self, client: ClientId) -> Vec<d::Event> {
        let side = self.sides.get_mut(&client).expect("bound byte client");
        let with = match side.wire.credential() {
            Some(credential) => jig_client::With::Held(credential),
            None => jig_client::With::Link {
                secret: side.connections.printed(&self.link).expect("printed Ops link"),
                held: None,
            },
        };
        vec![d::Event::Client { event: Event::SignIn { with } }]
    }
    pub fn send(&mut self, client: ClientId, now: Time, request: Request) {
        self.sides.get_mut(&client).expect("bound byte client").send(now, request);
    }
    pub fn stored(&mut self, client: ClientId, records: &[d::store::Record]) -> bool {
        let side = self.sides.get_mut(&client).expect("bound byte client");
        if side.stored == records {
            return false;
        }
        side.stored = records.to_vec();
        true
    }
    pub fn receipts(&mut self, client: ClientId, event: &d::Event) -> Vec<Receipt> {
        let d::Event::Client { event } = event else { return Vec::new() };
        let Some(key) = callback(event) else { return Vec::new() };
        self.receipts
            .iter()
            .position(|(id, held, _, _)| *id == client && *held == key)
            .map_or_else(Vec::new, |at| self.receipts.remove(at).3)
    }
    pub fn delivers(&mut self, client: ClientId, event: &d::Event) -> bool {
        let d::Event::Client { event: event @ (Event::Streamed { .. } | Event::StreamGone) } = event else {
            return true;
        };
        let key = callback(event).expect("stream callback key");
        let Some(at) = self.receipts.iter().position(|(id, held, _, _)| *id == client && *held == key) else {
            return false;
        };
        if self.receipts[at].2 == self.sides[&client].stream_epoch {
            true
        } else {
            self.receipts.remove(at);
            false
        }
    }
    pub fn next_due(&self) -> Option<Time> {
        self.sides
            .values()
            .filter_map(|side| side.connections.next_deadline())
            .chain(self.net.next_due())
            .chain(self.component.next_deadline())
            .min()
    }
    pub fn reload(&mut self, client: ClientId) {
        self.net.cut_client(client);
        self.net.retire(client);
        let credential = self.sides[&client].wire.credential();
        let mut side = Side::new();
        side.wire.restore(credential);
        self.sides.insert(client, side);
        self.receipts.retain(|(id, _, _, _)| *id != client);
    }
    pub fn cut(&mut self, until: Time) {
        self.net.cut_all();
        self.net.offline(until);
    }
    pub fn drop_stream(&mut self) {
        for client in self.sides.keys().copied() {
            self.net.cut_stream(client);
        }
    }
    pub fn slow(&mut self, now: Time, span: skein_lib::Duration) {
        self.net.clock(now);
        for client in self.sides.keys().copied() {
            self.net.slow(client, span);
        }
    }
}
fn callback(event: &Event) -> Option<Callback> {
    match event {
        Event::Reply { exchange, .. } => Some(Callback::Reply(*exchange)),
        Event::Streamed { message: Streamed::Record(bytes) } => Some(Callback::Record(bytes.clone())),
        Event::Streamed { message: Streamed::Heartbeat } => Some(Callback::Heartbeat),
        Event::Streamed { message: Streamed::SignedOut | jig_client::Streamed::Full(_) } | Event::StreamGone => {
            Some(Callback::Gone)
        }
        Event::Start { .. }
        | Event::StartUnlinked { .. }
        | Event::SignIn { .. }
        | Event::Authorize { .. }
        | Event::SignOut
        | Event::SignOutLost
        | Event::Watch { .. }
        | Event::Unwatch { .. }
        | Event::Read { .. }
        | Event::Send { .. }
        | Event::Carry { .. }
        | Event::Lost { .. }
        | Event::Full { .. }
        | Event::Stored
        | Event::NotStored
        | Event::Up { .. } => None,
    }
}
impl Bytes {
    pub fn pump(&mut self, joint: &mut Joint, now: Time, wall: Wall) -> Vec<(ClientId, d::Event)> {
        self.server.clock(now, wall);
        self.env.now = now;
        self.env.wall = wall;
        self.net.clock(now);
        // An external source fixture can drain and crash the engine too.
        // Replace the component before accepting any work from its old face.
        if self.crashed(joint) {
            return Vec::new();
        }
        let mut callbacks = std::mem::take(&mut joint.callbacks);
        let offline = joint.offline_until.is_some_and(|until| now < until);
        if !offline && joint.offline_until.take().is_some() {
            joint.reconnects += 1;
            joint.observed.append(&mut joint.pending_observed);
            joint.observed.push(jig_client_world::harness::Observed::Reachable { live: true });
        }
        let mut quiet = 0;
        for _ in 0..100_000 {
            let mut active = false;
            self.server.fire(&mut self.server_out);
            active |= self.mount_step();
            active |= self.server.advance(&mut self.server_out);
            while let Some(routed) = self.server_out.pop() {
                active = true;
                self.routed(routed);
            }
            self.component.fire(&self.env, &mut self.out);
            while self.component.up(&self.env, &mut self.out) {
                active = true;
            }
            while let Some(request) = self.out.pop() {
                active = true;
                self.request(joint, request);
            }
            if !offline && joint.work && joint.due.is_none() {
                self.ready_admissions.append(&mut self.pending_admissions);
                joint.engine.drain().expect("Ops byte decisions");
                joint.work = false;
                if self.crashed(joint) {
                    return Vec::new();
                }
                joint.due = Some(Time::from_nanos(joint.engine.clock().now));
            }
            if !offline && joint.due.is_some_and(|due| due <= now) {
                joint.due = None;
                let packets = std::mem::take(&mut joint.engine.peers.packets);
                active |= !packets.is_empty();
                for packet in packets {
                    self.packet(joint, packet);
                }
                // Refusals reach the component before successful admissions.
                // The engine can refuse an opening (for example an ended task).
                for watcher in std::mem::take(&mut self.ready_admissions) {
                    self.event(clients::Event::Admitted { watcher });
                }
                joint.observed.append(&mut joint.engine.peers.observations);
            }
            for (client, side) in &mut self.sides {
                side.connections.take(now, &mut side.up, &mut side.down);
                while let Some(event) = side.up.pop() {
                    active = true;
                    let receipts = side.receipts(&event);
                    if let Some(key) = callback(&event) {
                        self.receipts.push((*client, key, side.stream_epoch, receipts));
                    }
                    callbacks.push((*client, d::Event::Client { event }));
                }
                while let Some((conn, down)) = side.down.pop() {
                    active = true;
                    if let Some(event) = self.net.down(*client, conn, down) {
                        self.server.up(event, &mut self.server_out);
                    }
                }
            }
            let (events, clients, changed) = self.net.advance(&mut self.rules);
            active |= changed;
            for event in events {
                self.server.up(event, &mut self.server_out);
            }
            for (client, conn, up) in clients {
                let side = self.sides.get_mut(&client).expect("original byte route");
                side.connections.up(now, conn, up, &mut side.up, &mut side.down);
            }
            quiet = if active { 0 } else { quiet + 1 };
            if quiet == 32 || (!active && self.out.is_empty() && !joint.work && !self.net.pending()) {
                return callbacks;
            }
        }
        panic!("Ops byte transport did not quiesce");
    }
    fn routed(&mut self, routed: jig_test_service::server::Routed) {
        match routed {
            jig_test_service::server::Routed::Io(request) => {
                if let Some(event) = self.net.io(request) {
                    self.server.up(event, &mut self.server_out);
                }
            }
            jig_test_service::server::Routed::Call { call, head } => {
                if self.mount.room() > 0 {
                    self.mount_event(http::Event::Call { call, head });
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
                    self.mount_event(http::Event::Http { call, event });
                }
            }
        }
    }
    fn crashed(&mut self, joint: &mut Joint) -> bool {
        let crashes = joint.engine.outcome().crashes.len();
        if crashes == joint.crashes_seen {
            return false;
        }
        joint.crashes_seen = crashes;
        let at = Time::from_nanos(joint.engine.clock().now);
        let offline = skein_lib::Duration::from_millis(150);
        joint.disconnected(at, offline);
        self.restart(joint, at.saturating_add(offline));
        true
    }
    fn request(&mut self, joint: &mut Joint, request: clients::Request) {
        match request {
            clients::Request::TakeMoved { exchange, .. } => {
                self.component.down(&self.env, clients::Event::MovedTaken { exchange, record: None }, &mut self.out);
            }
            clients::Request::Transport(message) => self.mount_event(http::Event::Component(message)),
            clients::Request::Link { secret } => self.link = jig_client_protocol_world::fixtures::printed(&secret),
            clients::Request::Secrets(request) => {
                let answer = match request {
                    SecretsRequest::Keep { owner, value } => {
                        self.secrets.insert(owner, value);
                        SecretsAnswer::Kept { owner }
                    }
                    SecretsRequest::Erase { owner } => {
                        self.secrets.remove(&owner);
                        SecretsAnswer::Erased { owner }
                    }
                    SecretsRequest::Get { owner } => {
                        SecretsAnswer::Got { owner, value: self.secrets.get(&owner).cloned() }
                    }
                    SecretsRequest::Hold { .. } | SecretsRequest::Carry { .. } => {
                        panic!("the ops world carries no values")
                    }
                };
                self.event(clients::Event::Secrets(answer));
            }
            clients::Request::Core(event) => {
                let watcher = jig_client_protocol_world::watcher(&event);
                joint.send_event(root::Event::Core(*event));
                if let Some(watcher) = watcher {
                    self.pending_admissions.push(watcher);
                }
            }
            clients::Request::Application(request) => self.application(joint, request),
            clients::Request::Minted { .. }
            | clients::Request::MintRefused { .. }
            | clients::Request::Redeemed { .. }
            | clients::Request::Claimed { .. } => {
                panic!("Ops fixture mints no sign-ins")
            }
            clients::Request::Closed => {}
        }
    }
    fn application(&mut self, joint: &mut Joint, request: clients::ToApplication) {
        let event = match request {
            clients::ToApplication::Request { reply_to, sign_in, key, family, scope, record, digest, .. } => {
                assert_eq!(family, 2);
                joint.engine.peers.application_bytes.push((key, record.clone()));
                let ask = d::wire::ask_value(&record).expect("whole infrastructure request bytes");
                let jig_domain_people::ApplicationScope::Project(project) = scope else {
                    self.event(clients::Event::Application(FromApplication::Undecodable { to: reply_to }));
                    return;
                };
                let (_, value, _) = super::infrastructure::ask(ask).expect("native infrastructure request");
                root::Event::Application { reply_to, sign_in, key, project, value, digest }
            }
            clients::ToApplication::Read { reply, sign_in, family, record, scope, .. } => {
                let subject = subject(family, &record, scope);
                let (scope, owner) = subject_route(subject.clone()).expect("whole owner read");
                let token = reply.into_token();
                self.reads.insert(token, subject);
                let reply = skein_lib::ReplyTo::new(token);
                root::Event::ReadApplication { reply, sign_in, scope, subject: owner }
            }
            clients::ToApplication::Watch { watcher, sign_in, family, record, scope, window, .. } => {
                let subject = subject(family, &record, scope);
                let (scope, owner) = subject_route(subject.clone()).expect("whole owner watch");
                self.subjects.insert(watcher, subject);
                self.pending_admissions.push(watcher);
                root::Event::WatchApplication { watcher, sign_in, scope, subject: owner, window }
            }
        };
        joint.send_event(event);
    }
    fn packet(&mut self, joint: &mut Joint, packet: root::Output) {
        match packet {
            root::Output::Core(held) => {
                if let core::Held::ApplicationWatchEnded { watcher, why, .. } = held {
                    self.subjects.remove(&watcher);
                    self.event(clients::Event::WatchEnded { watcher, why });
                } else if let Some(event) = jig_client_protocol_world::held_event(held) {
                    if matches!(&event, clients::Event::Reply { reply: jig_domain_people::Reply::Outcome(_), .. })
                        && joint.lose_answers > 0
                    {
                        joint.lose_answers -= 1;
                        joint.lost_final_answers += 1;
                        self.net.cut_answer();
                    }
                    self.event(event);
                }
            }
            root::Output::Now(now) => {
                if let Some(event) = jig_client_protocol_world::now_event(now) {
                    self.event(event);
                }
            }
            root::Output::Application { to, outcome } => {
                if joint.lose_answers > 0 {
                    joint.lose_answers -= 1;
                    joint.lost_final_answers += 1;
                    self.net.cut_answer();
                }
                let answer = super::infrastructure::answer(outcome);
                self.event(clients::Event::Application(FromApplication::Answered {
                    to,
                    family: 2,
                    record: d::wire::answer(&answer).expect("whole infrastructure answer"),
                }));
            }
            root::Output::ApplicationDocument { watcher, document } => {
                joint.observe_environment(&document);
                let Some(subject) = self.subjects.get(&watcher) else { return };
                let (family, record) = document_bytes(subject.clone(), document, &joint.limits);
                self.event(clients::Event::Application(FromApplication::Delivery { watcher, family, record }));
            }
            root::Output::ApplicationRead { reply, answer } => {
                let token = reply.into_token();
                let Some(subject) = self.reads.remove(&token) else { return };
                let reply = skein_lib::ReplyTo::new(token);
                let (family, record) = match subject {
                    Subject::Infrastructure(subject) => {
                        let value = match answer {
                            Ok(document) => {
                                joint.observe_environment(&document);
                                let ApplicationDocument::Infrastructure(value) =
                                    application_document(Subject::Infrastructure(subject), document, &joint.limits)
                                else {
                                    panic!("infrastructure read")
                                };
                                d::infrastructure::ReadResult::Value(value)
                            }
                            Err(root::ClientMissing::Unknown) => d::infrastructure::ReadResult::Unknown,
                            Err(root::ClientMissing::NotAllowed) => d::infrastructure::ReadResult::NotAllowed,
                        };
                        (2, d::wire::infrastructure::read_answer(&value).expect("whole infrastructure read"))
                    }
                    Subject::Observability(subject) => {
                        let value = match answer {
                            Ok(document) => {
                                joint.observe_environment(&document);
                                let ApplicationDocument::Observability(value) =
                                    application_document(Subject::Observability(subject), document, &joint.limits)
                                else {
                                    panic!("observability read")
                                };
                                d::observability::ReadResult::Value(value)
                            }
                            Err(root::ClientMissing::Unknown) => d::observability::ReadResult::Unknown,
                            Err(root::ClientMissing::NotAllowed) => d::observability::ReadResult::NotAllowed,
                        };
                        (1, d::wire::observability::read_answer(&value).expect("whole observability read"))
                    }
                };
                self.event(clients::Event::Application(FromApplication::Read { reply, family, record: Some(record) }));
            }
            root::Output::IncidentAssociationRefused { .. }
            | root::Output::Commit { .. }
            | root::Output::Switch
            | root::Output::Load { .. }
            | root::Output::CoreLoad(_)
            | root::Output::Observability(_)
            | root::Output::Infrastructure(_)
            | root::Output::ReadResult { .. }
            | root::Output::Read { .. }
            | root::Output::Agent(_)
            | root::Output::ToChild(_)
            | root::Output::ProjectMade { .. }
            | root::Output::Busy
            | root::Output::Stop => panic!("only public client packets enter the byte face"),
        }
    }
    pub(super) fn restart(&mut self, joint: &Joint, until: Time) {
        let fresh = Self::new(joint.seed, joint.story, joint.limits.jig.rows);
        self.component = fresh.component;
        self.server = fresh.server;
        self.server_out = fresh.server_out;
        self.mount = fresh.mount;
        self.mount_out = fresh.mount_out;
        self.out = fresh.out;
        self.subjects.clear();
        self.reads.clear();
        self.receipts.clear();
        self.pending_admissions.clear();
        self.ready_admissions.clear();
        self.cut(until);
    }
}
fn subject(family: u16, bytes: &[u8], scope: jig_domain_people::ApplicationScope) -> Subject {
    let scope = match scope {
        jig_domain_people::ApplicationScope::Project(project) => jig_client::contract::Scope::Project(project),
        jig_domain_people::ApplicationScope::Deployment => jig_client::contract::Scope::Deployment,
        jig_domain_people::ApplicationScope::Own => jig_client::contract::Scope::Own,
    };
    match family {
        1 => Subject::Observability(
            d::wire::observability::subject_value(bytes, scope).expect("whole observability subject"),
        ),
        2 => Subject::Infrastructure(
            d::wire::infrastructure::subject_value(bytes, scope).expect("whole infrastructure subject"),
        ),
        _ => panic!("declared family"),
    }
}
fn document_bytes(subject: Subject, document: root::ClientDocument, limits: &d::Limits) -> (u16, Box<[u8]>) {
    match application_document(subject, document, limits) {
        ApplicationDocument::Infrastructure(value) => {
            (2, d::wire::infrastructure::document(&value).expect("whole infrastructure document"))
        }
        ApplicationDocument::Observability(value) => {
            (1, d::wire::observability::document(&value).expect("whole observability document"))
        }
    }
}

#[cfg(test)]
mod tests;
