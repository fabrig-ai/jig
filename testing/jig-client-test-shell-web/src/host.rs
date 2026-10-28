//! One client host shared by the browser's platform parts.
use jig_client_shell_web::adapter::{Asked, Heard};
use jig_client_test_domain::{self as root, Address, Event, Request};
use jig_client_test_view as view;
use jig_client_view::address::{self, Place};
use skein_lib::{Env, List, Queue, Secret, Time, Token, Wall};
use skein_ui::{Act, NodeId, Patch};
use std::collections::BTreeMap;

pub struct Host {
    pub root: root::Domain,
    pub view: view::View<'static>,
    pub bounds: view::Limits,
    pub codec: address::Codec<'static>,
    env: Env<root::Limits>,
    out: Queue<Request>,
    mapping: jig_client_protocol_http::Mapping,
    stream: jig_client_protocol_http::Stream,
    tickets: BTreeMap<u32, Token>,
    next: u32,
    pub show: Option<(Box<[u8]>, bool)>,
}
impl Host {
    pub fn new(seed: u64, now: Time, wall: Wall) -> Self {
        let limits = super::limits::limits();
        let words = Box::leak(Box::new(view::connector_values_words::english(limits.jig.text).expect("English")));
        let family = Box::leak(Box::new(view::words::Words::english(limits.first.text).expect("family English")));
        let bounds = view::Limits::for_world(&limits, words, family, 512, 256).expect("small view bounds");
        Self {
            root: root::new(&limits, seed),
            view: view::View::new(&bounds, &limits, words, family, root::first::Address { project: 1, item: 1 }),
            bounds,
            codec: address::REFERENCE,
            env: Env { now, wall, limits },
            out: Queue::uncapped(),
            mapping: super::mapping::mapping(),
            stream: jig_client_protocol_http::Stream::new(&jig_client_protocol_http::Limits {
                record: 65536,
                chunk: 4096,
            }),
            tickets: BTreeMap::new(),
            next: 0,
            show: None,
        }
    }
    pub fn start(
        &mut self,
        path: &[u8],
        records: Vec<skein_ui_web::store::Record>,
        offset: i32,
        secret: Option<Box<[u8]>>,
    ) {
        let mut kept =
            List::with_capacity(self.env.limits.jig.records + self.env.limits.client.domain.requests.requests + 1);
        for record in records {
            if let Some(record) = super::records::decode(record) {
                kept.push(record).expect("bounded records");
            }
        }
        let address = match address::parse(&self.codec, path) {
            Place::Jig(address) => address,
            Place::Elsewhere | Place::Missing => address::Address::Inbox { project: None },
        };
        self.event(Event::Start {
            address: Address::Jig(address),
            kept,
            offset: jig_client_view::boundary::Offset { minutes: offset },
        });
        let with = secret.map_or(jig_client::With::Transport, |bytes| jig_client::With::Link {
            secret: Secret::new(bytes),
            held: None,
        });
        self.client(jig_client::Event::SignIn { with });
    }
    pub fn clocks(&mut self, now: Time, wall: Wall) {
        self.env.now = now;
        self.env.wall = wall;
    }
    pub fn event(&mut self, event: Event) {
        root::step(&mut self.root, &self.env, event, &mut self.out);
    }
    fn client(&mut self, event: jig_client::Event) {
        self.event(Event::Client { event });
    }
    pub fn due(&mut self) {
        root::fire(&mut self.root, &self.env, &mut self.out);
    }
    pub fn act(&mut self, node: NodeId, act: Act) {
        if let Some(action) = view::decode(&mut self.view, node, act) {
            self.event(Event::Act { action });
        }
    }
    pub fn went(&mut self, path: &[u8]) {
        let place = match address::parse(&self.codec, path) {
            Place::Jig(address) => root::Place::Address(Address::Jig(address)),
            Place::Missing | Place::Elsewhere => root::Place::Missing,
        };
        self.event(Event::Went { place });
    }
    pub fn heard(&mut self, heard: Heard) {
        use jig_client_protocol_http::Heard as H;
        let mut above = Queue::uncapped();
        match heard {
            Heard::Answered { exchange, status, body } => {
                if let Some(token) = self.tickets.remove(&exchange) {
                    above.push(jig_client_protocol_http::answered(token, status, body));
                }
            }
            Heard::Failed { exchange } => {
                if let Some(exchange) = self.tickets.remove(&exchange) {
                    above.push(H::Lost { exchange });
                }
            }
            Heard::StreamOpened => self.stream.opened(200, &mut above),
            Heard::StreamRefused { status } => self.stream.opened(status, &mut above),
            Heard::Piece { bytes } => self.stream.body(&bytes, &mut above),
            Heard::StreamEnded => self.stream.ended(&mut above),
            Heard::Stored { .. } => self.client(jig_client::Event::Stored),
            Heard::NotStored { .. } => self.client(jig_client::Event::NotStored),
        }
        while let Some(heard) = above.pop() {
            self.client(match heard {
                H::Reply { exchange, message } => jig_client::Event::Reply { exchange, message },
                H::Lost { exchange } => jig_client::Event::Lost { exchange },
                H::Streamed { message } => jig_client::Event::Streamed { message },
                H::StreamGone => jig_client::Event::StreamGone,
            });
        }
    }
    // A step's writes are one durability answer: a first successful write
    // cannot release held requests before a later refused write is heard.
    pub fn carried(&mut self, reports: Vec<skein_ui_web::transport::Carried>, stored: bool) {
        let mut refused = false;
        for report in reports {
            match jig_client_shell_web::adapter::carried(report) {
                Heard::NotStored { .. } => refused = true,
                Heard::Stored { .. } => {}
                report => self.heard(report),
            }
        }
        if stored {
            self.client(if refused { jig_client::Event::NotStored } else { jig_client::Event::Stored });
        }
    }
    pub fn requests(&mut self) -> (Vec<Asked>, Vec<Secret>) {
        let mut asked = Vec::new();
        let mut credentials = Vec::new();
        while let Some(request) = self.out.pop() {
            match request {
                Request::Save { record } => {
                    let (key, value) = super::records::encode(record);
                    asked.push(Asked::Save { key, value });
                }
                Request::Erase { key } => asked.push(Asked::Erase { key: super::records::key(key) }),
                Request::Show { address: Address::Jig(place), replace } => {
                    self.show = Some((address::print(&self.codec, place), replace));
                }
                Request::Client { request: jig_client::Request::Exchange { exchange, credential, message } } => {
                    self.next = self.next.checked_add(1).expect("fetch tickets fit");
                    self.tickets.insert(self.next, exchange);
                    asked.push(Asked::Exchange {
                        id: self.next,
                        call: jig_client_protocol_http::exchange(&self.mapping, credential.as_ref(), message),
                        bound: 65536,
                    });
                }
                Request::Client { request: jig_client::Request::OpenStream { credential } } => {
                    self.stream = jig_client_protocol_http::Stream::new(&jig_client_protocol_http::Limits {
                        record: 65536,
                        chunk: 4096,
                    });
                    asked.push(Asked::OpenStream {
                        call: jig_client_protocol_http::stream(&self.mapping, credential.as_ref()),
                        piece: 4096,
                    });
                }
                Request::Client { request: jig_client::Request::CloseStream } => asked.push(Asked::CloseStream),
                Request::Client { request: jig_client::Request::ServiceCredential { credential, .. } } => {
                    credentials.push(credential);
                }
                Request::SignOut => self.client(jig_client::Event::SignOut),
                Request::SignIn { .. }
                | Request::First(_)
                | Request::Fact { .. }
                | Request::Show { .. }
                | Request::Client { .. } => {}
            }
        }
        root::take(&mut self.root, &self.env, &mut self.out);
        (asked, credentials)
    }
    pub fn build(&mut self, patches: &mut Queue<Patch>) {
        view::build(&mut self.view, &self.root, patches);
    }
    pub fn href(&mut self, node: NodeId) -> Option<Box<[u8]>> {
        match view::decode(&mut self.view, node, Act::Press)? {
            root::Action::Jig(jig_client_view::action::Act::Go { address }) => {
                Some(address::print(&self.codec, address))
            }
            root::Action::SignIn | root::Action::First(_) | root::Action::Jig(_) => None,
        }
    }
    pub fn deadline(&self) -> Option<Time> {
        root::next_deadline(&self.root)
    }
}
