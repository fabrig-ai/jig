//! The clients child and secret-free root handoffs (client.md, 2 and 8; service.md, 3).
use crate::{Seeds, family};
use alloc::boxed::Box;
use jig_domain_core as core;
use jig_domain_people as people;
use jig_protocol_config::{self as config, Path, Problems, What};
use jig_protocol_engine_clients as kit;
use jig_protocol_engine_clients_http as http;
use jig_test_domain as root;
use skein_io as io;
use skein_lib::{Env, List, Map, Queue, Time, Token, Wall};
mod translation;
pub(super) use translation::{application, immediate, to_component};
const PREFIX: u64 = 4_u64 << 60;
const HOST_BYTES: u32 = 64;

#[derive(Debug)]
pub(super) struct Clients {
    component: Option<kit::Component>,
    settings: Option<kit::Settings>,
    pub(super) server: crate::server::Server,
    pub(super) mount: http::Mount,
    pub(super) refused: Map<Token, ()>,
    mount_env: Env<http::Limits>,
    mount_out: Queue<http::Request>,
    pub(super) server_out: Queue<crate::server::Routed>,
    component_closed: bool,
    secret: Option<skein_lib::Secret>,
    pub(super) env: Env<kit::Limits>,
    out: Queue<kit::Request>,
    pub(super) io: Queue<io::Request>,
    pub(super) links: Queue<Box<[u8]>>,
    pub(super) secrets: Queue<kit::SecretsRequest>,
    pub(super) closed: bool,
    pending: bool,
    pub(super) starting: Option<()>,
    pub(super) early: Queue<root::Event>,
    endings: Map<u64, Endings>,
    durable: u64,
    writes: u32,
    secret_owners: Map<Token, Asked>,
    next_secret: u64,
}
/// What a secrets request in flight was for: a kept value's owner, or the
/// exchange whose carried value the store numbers itself.
#[derive(Clone, Copy, Debug)]
enum Asked {
    Owner(kit::SecretOwner),
    Carry(Token),
}
#[derive(Debug)]
struct Endings {
    numbers: List<u64>,
    next: u32,
}
#[derive(Debug)]
pub(super) enum Routed {
    Root(Box<root::Event>),
    Application(kit::ToApplication),
    Secrets(kit::SecretsRequest),
    Transport(kit::ToTransport),
    Link(skein_lib::Secret),
    TakeMoved { exchange: Token, source: u32 },
    Closed,
}
pub(super) fn to_root(request: kit::Request) -> Routed {
    match request {
        kit::Request::Core(event) => Routed::Root(Box::new(root::Event::Core(*event))),
        kit::Request::Application(value) => Routed::Application(value),
        kit::Request::Secrets(value) => Routed::Secrets(value),
        kit::Request::Transport(message) => Routed::Transport(message),
        kit::Request::Link { secret } => Routed::Link(secret),
        kit::Request::Minted { .. }
        | kit::Request::MintRefused { .. }
        | kit::Request::Redeemed { .. }
        | kit::Request::Claimed { .. } => {
            unreachable!("the test service mints no application sign-ins")
        }
        kit::Request::TakeMoved { exchange, source } => Routed::TakeMoved { exchange, source },
        kit::Request::Closed => Routed::Closed,
    }
}
fn invalid(what: What) -> Problems {
    let mut problems = Problems::empty();
    problems.push(Path::root().member(b"engine").member(b"clients"), what);
    problems
}
impl Clients {
    pub fn new(
        config: &config::Engine,
        listener: io::Addr,
        seeds: &Seeds,
        room: u32,
        transport: &crate::server::Limits,
    ) -> Result<Self, Problems> {
        if jig_protocol_accounts::moved::bound(&config.limits.driver.client.document)
            > u64::from(config.limits.clients.wire.keyed_value_some)
        {
            return Err(invalid(What::Bound));
        }
        let mut providers = List::with_capacity(u32::try_from(config.providers.len()).expect("bounded providers"));
        let mut link_provider = None;
        let mut deployment_provider = 0;
        for provider in &config.providers {
            match provider.kind {
                config::ProviderKind::Link => link_provider = Some(provider.number),
                config::ProviderKind::Deployment => deployment_provider = provider.number,
                config::ProviderKind::Application => {
                    providers.push(provider.number).expect("provider count");
                }
            }
        }
        let link = if config.clients.link {
            let provider = link_provider.ok_or(invalid(What::Refers))?;
            let mut identity = None;
            for owner in &config.owners {
                if owner.identity.provider == provider {
                    identity = Some(people::Identity {
                        key: owner.identity.clone(),
                        login: Box::from(b"owner".as_slice()),
                        name: Box::from(b"Owner".as_slice()),
                    });
                    break;
                }
            }
            Some(identity.ok_or(invalid(What::Refers))?)
        } else {
            None
        };
        let mut served = List::with_capacity(2);
        served
            .push(kit::Served {
                family: None,
                first: jig_client_protocol_wire::VERSION,
                last: jig_client_protocol_wire::VERSION,
            })
            .or(Err(invalid(What::Bound)))?;
        served
            .push(kit::Served {
                family: Some(1),
                first: jig_client_test_protocol_wire::VERSION,
                last: jig_client_test_protocol_wire::VERSION,
            })
            .or(Err(invalid(What::Bound)))?;
        let settings = kit::Settings {
            provider: None,
            addresses: Box::new([]),
            link,
            deployment_provider,
            heartbeat: config.clients.heartbeat,
            rows: config.limits.core.documents.rows,
            served,
            providers,
            seed: seeds.crypto.clone(),
            stream: seeds.seed,
            models: config::clients::models(&config.llm),
            redirect_bytes: config.limits.driver.client.uri_bytes,
            key_bytes: config
                .limits
                .driver
                .client
                .document
                .token_bytes
                .min(config.limits.clients.wire.keyed_value_some),
            moved_bytes: jig_protocol_accounts::moved::bound(&config.limits.driver.client.document),
        };
        let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: config.limits.clients };
        let out = Queue::with_capacity(room);
        Ok(Self {
            refused: Map::with_capacity(transport.connections),
            component: None,
            settings: Some(settings),
            server: crate::server::Server::new(listener, transport),
            mount: http::Mount::new(http_settings(), &config.limits.clients_http),
            mount_env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits: config.limits.clients_http },
            mount_out: Queue::with_capacity(room),
            server_out: Queue::with_capacity(room),
            component_closed: false,
            secret: None,
            env,
            out,
            io: Queue::with_capacity(room),
            links: Queue::with_capacity(room),
            secrets: Queue::with_capacity(room),
            closed: false,
            pending: true,
            starting: Some(()),
            early: Queue::with_capacity(room),
            endings: Map::with_capacity(config.limits.journal.commits),
            secret_owners: Map::with_capacity(room),
            next_secret: 1,
            durable: 0,
            writes: config.limits.journal.writes,
        })
    }
    pub fn start(&mut self, now: Time, wall: Wall) -> Result<(), Problems> {
        self.env.now = now;
        self.env.wall = wall;
        if let Some(settings) = self.settings.take() {
            self.component =
                Some(kit::Component::new(settings, &self.env, &mut self.out).or(Err(invalid(What::Bound)))?);
        }
        Ok(())
    }
    fn print_link(&mut self) {
        if let Some(listener) = self.server.listening()
            && let Some(secret) = self.secret.take()
        {
            self.links.push(printed(listener, &secret));
        }
    }
    pub fn room(&self, batch: u32) -> bool {
        let count = batch.checked_mul(kit::MAX_OUT_DOWN).expect("client routing room");
        self.out.room() >= count
            && self.io.room() >= count
            && self.links.room() >= count
            && self.secrets.room() >= count
    }
    pub fn opened(&self) -> bool {
        self.server.opened()
    }
    pub fn next_deadline(&self) -> Option<Time> {
        let component = match &self.component {
            Some(component) => component.next_deadline(),
            None => None,
        };
        crate::accounts::earliest(component, self.server.next_deadline())
    }
    pub fn work_pending(&self, now: Time) -> bool {
        !self.closed
            && (self.pending
                || !self.secrets.is_empty()
                || self.endings_ready()
                || !self.out.is_empty()
                || !self.early.is_empty()
                || !self.io.is_empty()
                || !self.links.is_empty()
                || match self.next_deadline() {
                    Some(at) => at <= now,
                    None => false,
                })
    }
    pub fn down(&mut self, event: kit::Event) {
        if let kit::Event::Stop = &event {
            self.server.stop(&mut self.server_out);
        }
        if let kit::Event::Close = &event {
            self.server.close(&mut self.server_out);
        }
        if let kit::Event::Abort = &event {
            self.server.abort(&mut self.server_out);
        }
        if !self.component_closed
            && let Some(component) = &mut self.component
        {
            component.down(&self.env, event, &mut self.out);
            self.pending = true;
        }
    }
    pub fn up(&mut self, event: io::Event) -> bool {
        let event = untag(event);
        if let io::Event::Failed { owner, .. } = &event
            && *owner == Token::new(0)
        {
            return false;
        }
        if let io::Event::Listening { addr, .. } = &event
            && let Some(address) = addr.socket()
        {
            let mut hosts = List::with_capacity(1);
            hosts.push(address_text(address).expect("bound listener address")).expect("one listener host");
            self.mount.down(&self.mount_env, http::Event::Listening { hosts }, &mut self.mount_out);
        }
        self.server.up(event, &mut self.server_out);
        self.print_link();
        self.pending = true;
        true
    }
    pub(super) fn server_requests(&mut self) {
        for _ in 0..self.server_out.len() {
            if self.io.room() == 0 || self.out.room() < kit::MAX_OUT_DOWN {
                break;
            }
            match self.server_out.pop().expect("server requests") {
                crate::server::Routed::Io(request) => self.io.push(tag(request)),
                crate::server::Routed::Call { call, head } => {
                    if self.mount.room() > 0 {
                        self.mount.down(&self.mount_env, http::Event::Call { call, head }, &mut self.mount_out);
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
                crate::server::Routed::Http { call, event } => {
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
                        self.mount.down(&self.mount_env, http::Event::Http { call, event }, &mut self.mount_out);
                    }
                }
            }
        }
        self.closed = self.component_closed && self.server.closed();
    }
    fn root(&mut self, value: root::Event, events: &mut Queue<root::Event>) {
        if self.starting.is_some() {
            self.early.push(value);
        } else {
            events.push(value);
        }
    }
    pub fn advance(
        &mut self,
        events: &mut Queue<root::Event>,
        bound: &jig_test_connector::Limits,
        rows: u32,
        accounts: &mut Option<crate::accounts::Accounts>,
    ) {
        for _ in 0..self.out.len() {
            if self.io.room() == 0
                || self.links.room() == 0
                || self.secrets.room() == 0
                || (self.starting.is_some() && self.early.room() == 0)
                || (self.starting.is_none() && events.room() == 0)
            {
                break;
            }
            match to_root(self.out.pop().expect("client requests")) {
                Routed::Transport(message) => {
                    self.mount.down(&self.mount_env, http::Event::Component(message), &mut self.mount_out);
                }
                Routed::Link(value) => {
                    self.secret = Some(value);
                    self.print_link();
                }
                Routed::TakeMoved { exchange, source } => {
                    let record = match accounts {
                        Some(accounts) => accounts.take_moved(source),
                        None => None,
                    };
                    self.down(kit::Event::MovedTaken { exchange, record });
                }
                Routed::Secrets(value) => self.secrets.push(value),
                Routed::Root(value) => self.root(*value, events),
                Routed::Application(value) => match family::to_root(value, bound, rows) {
                    Ok(value) => self.root(value, events),
                    Err(value) => self.down(*value),
                },
                Routed::Closed => self.component_closed = true,
            }
        }
    }
    pub fn iterate(
        &mut self,
        now: Time,
        wall: Wall,
        events: &mut Queue<root::Event>,
        bound: &jig_test_connector::Limits,
        rows: u32,
        accounts: &mut Option<crate::accounts::Accounts>,
    ) {
        self.env.now = now;
        self.env.wall = wall;
        self.mount_env.now = now;
        self.mount_env.wall = wall;
        self.server.clock(now, wall);
        if self.closed || !self.room(1) || self.early.room() < kit::MAX_OUT_DOWN || events.room() < kit::MAX_OUT_DOWN {
            return;
        }
        self.server.fire(&mut self.server_out);
        let server_pending = self.server.advance(&mut self.server_out);
        self.server_requests();
        self.endings_step(events, bound, rows, accounts);
        self.advance(events, bound, rows, accounts);
        if self.closed {
            return;
        }
        if !self.component_closed {
            self.component.as_mut().expect("clients started at first service clock").fire(&self.env, &mut self.out);
        }
        self.advance(events, bound, rows, accounts);
        self.pending = server_pending || !self.server_out.is_empty();
        self.pending |= self.mount.up(&self.mount_env, &mut self.mount_out);
        if let Some(request) = self.mount_out.pop() {
            self.pending = true;
            match request {
                http::Request::Component(message) => self.down(kit::Event::Transport(message)),
                http::Request::Http { call, request } => self.server.answer(call, request, &mut self.server_out),
            }
        }
        if !self.component_closed {
            self.pending |=
                self.component.as_mut().expect("clients started at first service clock").up(&self.env, &mut self.out);
        }
        self.advance(events, bound, rows, accounts);
    }
}
pub(super) fn owns(token: Token) -> bool {
    token.raw() & (15_u64 << 60) == PREFIX
}
pub(super) fn tag(mut request: io::Request) -> io::Request {
    match &mut request {
        io::Request::Listen { owner, .. }
        | io::Request::Connect { owner, .. }
        | io::Request::Bind { owner, .. }
        | io::Request::Spawn { owner, .. }
        | io::Request::Usage { owner }
        | io::Request::Window { owner, .. } => {
            assert!(owner.raw() < (1_u64 << 60_u32), "bounded client exchange names");
            *owner = Token::new(owner.raw() | PREFIX);
        }
        io::Request::Reject { .. }
        | io::Request::Stream { .. }
        | io::Request::Output { .. }
        | io::Request::Signal { .. }
        | io::Request::Close { .. }
        | io::Request::Abort { .. } => {}
    }
    request
}
pub(super) fn untag(mut event: io::Event) -> io::Event {
    match &mut event {
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
        | io::Event::Closed { owner } => {
            *owner = Token::new(owner.raw() & !(15_u64 << 60));
        }
        io::Event::Resized | io::Event::Shutdown { .. } => {}
    }
    event
}

pub(super) fn worst_case(
    limits: &kit::Limits,
    transport: &crate::server::Limits,
    mapping: &http::Limits,
    room: u32,
    journal: &skein_lib::JournalLimits,
) -> Option<u64> {
    kit::worst_case(limits)?
        .checked_add(http::worst_case(mapping)?)?
        .checked_add(Map::<Token, ()>::worst_case(transport.connections)?)?
        .checked_add(u64::try_from(size_of::<Map<Token, ()>>()).ok()?)?
        // This mount owns one bound listener's textual address.
        .checked_add(List::<Box<[u8]>>::worst_case(1)?.checked_add(u64::from(HOST_BYTES))?)?
        .checked_add(Queue::<http::Request>::worst_case(room)?)?
        .checked_add(Queue::<kit::Request>::worst_case(room)?)?
        .checked_add(Queue::<io::Request>::worst_case(room)?)?
        .checked_add(Queue::<root::Event>::worst_case(room)?)?
        .checked_add(u64::from(limits.message_bytes).checked_mul(64)?.checked_mul(u64::from(room).checked_mul(2)?)?)?
        .checked_add(u64::from(transport.http.send).checked_mul(u64::from(room))?)?
        .checked_add(Queue::<Box<[u8]>>::worst_case(room)?)?
        .checked_add(u64::from(room).checked_mul(512)?)?
        .checked_add(Queue::<kit::SecretsRequest>::worst_case(room)?)?
        .checked_add(Map::<Token, Asked>::worst_case(room)?)?
        // Each queued Keep owns its 32-byte credential digest; row encoding is the store's.
        .checked_add(u64::from(room).checked_mul(32)?)?
        // A carried value waits here on its way to the store.
        .checked_add(u64::from(room).checked_mul(u64::from(limits.wire.keyed_value_some))?)?
        .checked_add(Map::<u64, Endings>::worst_case(journal.commits)?)?
        .checked_add(List::<u64>::worst_case(journal.writes)?.checked_mul(u64::from(journal.commits))?)
}

impl Clients {
    pub fn root_event(&mut self, value: &root::Event) -> Option<Token> {
        match value {
            root::Event::Core(value) => translation::watcher(value),
            root::Event::WatchApplication { watcher, .. } => Some(*watcher),
            root::Event::Committed { number, .. } => {
                self.committed(*number);
                None
            }
            root::Event::Link { .. }
            | root::Event::ResultRead { .. }
            | root::Event::Stop
            | root::Event::ReadApplication { .. }
            | root::Event::Application { .. }
            | root::Event::WriterRead { .. }
            | root::Event::ConnectorTimer { .. }
            | root::Event::Switched
            | root::Event::RestartBegin { .. }
            | root::Event::RestartDone(..)
            | root::Event::Timer(..)
            | root::Event::Credential { .. }
            | root::Event::Connector { .. }
            | root::Event::ProjectionEffect { .. }
            | root::Event::TopicCall { .. }
            | root::Event::EffectCall { .. }
            | root::Event::CallAnswered { .. }
            | root::Event::Text { .. }
            | root::Event::Turn { .. }
            | root::Event::Answer { .. }
            | root::Event::TranscriptLoaded { .. }
            | root::Event::Failed { .. } => None,
        }
    }
}

impl Clients {
    pub fn commit(&mut self, number: u64, writes: &Queue<root::Write>) {
        let mut endings = List::with_capacity(self.writes);
        for write in writes {
            match write {
                root::Write::Erase(key) => {
                    if let Some(number) = ended(key)
                        && !endings.as_slice().contains(&number)
                    {
                        endings.push(number).expect("bounded commit writes");
                    }
                }
                root::Write::Wires | root::Write::Save(_) | root::Write::CopySettingsRevision(_) => {}
            }
        }
        if !endings.is_empty() {
            self.endings.insert(number, Endings { numbers: endings, next: 0 }).expect("bounded commits in flight");
        }
    }
    pub fn committed(&mut self, number: u64) {
        self.durable = self.durable.max(number);
    }
    pub fn commit_room(&self) -> bool {
        self.endings.len() < self.endings.capacity()
    }
    fn endings_ready(&self) -> bool {
        for (number, _) in &self.endings {
            if *number <= self.durable {
                return true;
            }
        }
        false
    }
    fn endings_step(
        &mut self,
        events: &mut Queue<root::Event>,
        bound: &jig_test_connector::Limits,
        rows: u32,
        accounts: &mut Option<crate::accounts::Accounts>,
    ) {
        while self.room(1) && events.room() >= kit::MAX_OUT_DOWN {
            let mut ready = None;
            for (number, _) in &self.endings {
                if *number <= self.durable {
                    ready = Some(*number);
                    break;
                }
            }
            let Some(number) = ready else { return };
            let endings = self.endings.get_mut(&number).expect("durable endings");
            let sign_in = *endings.numbers.get(endings.next).expect("one sign-in per erased key");
            endings.next = endings.next.checked_add(1).expect("bounded erase index");
            if endings.next == endings.numbers.len() {
                self.endings.remove(&number);
            }
            self.down(kit::Event::SignInEnded { sign_in });
            self.advance(events, bound, rows, accounts);
        }
    }
}

fn ended(key: &root::Key) -> Option<u64> {
    match key {
        root::Key::Core(core::Key::People(people::Key::SignIn(number))) => Some(*number),
        root::Key::Core(
            core::Key::People(
                people::Key::Person(..)
                | people::Key::ReadResults { .. }
                | people::Key::EndedRead { .. }
                | people::Key::ReadConversation { .. }
                | people::Key::Roles(..)
                | people::Key::Policy(..)
                | people::Key::Acted(..)
                | people::Key::Answer(..),
            )
            | core::Key::Core(..)
            | core::Key::Tasks(..)
            | core::Key::Notes(..)
            | core::Key::Accounts(..),
        )
        | root::Key::Connector { .. } => None,
    }
}

impl Clients {
    pub fn not_ready(&mut self, value: root::Event) {
        let event = match value {
            root::Event::Core(value) => translation::starting(value),
            root::Event::Application { reply_to, .. } => {
                Some(kit::Event::Reply { to: reply_to, reply: people::Reply::Again(people::Again::NotReady) })
            }
            root::Event::ReadApplication { reply, .. } => {
                Some(kit::Event::Read { reply, answer: core::reads::Answer::None(core::reads::Why::Busy) })
            }
            root::Event::WatchApplication { watcher, .. } => {
                Some(kit::Event::WatchRefused { watcher, why: core::WatchRefusal::Busy })
            }
            root::Event::Link { .. }
            | root::Event::ResultRead { .. }
            | root::Event::Stop
            | root::Event::WriterRead { .. }
            | root::Event::ConnectorTimer { .. }
            | root::Event::Switched
            | root::Event::RestartBegin { .. }
            | root::Event::RestartDone(..)
            | root::Event::Timer(..)
            | root::Event::Credential { .. }
            | root::Event::Connector { .. }
            | root::Event::ProjectionEffect { .. }
            | root::Event::TopicCall { .. }
            | root::Event::EffectCall { .. }
            | root::Event::CallAnswered { .. }
            | root::Event::Text { .. }
            | root::Event::Turn { .. }
            | root::Event::Answer { .. }
            | root::Event::TranscriptLoaded { .. }
            | root::Event::Committed { .. }
            | root::Event::Failed { .. } => None,
        };
        if let Some(event) = event {
            self.down(event);
        }
    }
}

impl Clients {
    pub(super) fn secrets_step(&mut self, store: &mut crate::store::Store) {
        for _ in 0..store.secrets.len() {
            if self.out.room() < kit::MAX_OUT_DOWN {
                break;
            }
            let answer = store.secrets.pop().expect("secrets terminals");
            self.secret_answer(answer);
        }
        self.advance_secrets(store);
    }
    fn advance_secrets(&mut self, store: &mut crate::store::Store) {
        for _ in 0..self.secrets.len() {
            if self.out.room() < kit::MAX_OUT_DOWN || self.secret_owners.len() == self.secret_owners.capacity() {
                break;
            }
            let request = self.secrets.pop().expect("client secrets requests");
            let (owner, request) = secret_request(request);
            let token = Token::new(PREFIX.checked_add(self.next_secret).expect("secret routing name"));
            self.next_secret = self.next_secret.checked_add(1).expect("secret routing sequence");
            self.secret_owners.insert(token, owner).expect("bounded secrets in flight");
            if let Some(answer) = store.secrets(token, request) {
                self.secret_answer(answer);
            }
        }
    }
    fn secret_answer(&mut self, answer: jig_protocol_store::Event) {
        use jig_protocol_store::Event;
        let event = match answer {
            Event::Kept { owner } => kit::SecretsAnswer::Kept { owner: self.owner(owner) },
            Event::Erased { owner } => kit::SecretsAnswer::Erased { owner: self.owner(owner) },
            Event::Carried { owner, number } => {
                kit::SecretsAnswer::Carried { exchange: self.carry(owner), number: Some(number) }
            }
            Event::Stale { owner } => match self.secret_owners.remove(&owner).expect("secret stale owner") {
                Asked::Carry(exchange) => kit::SecretsAnswer::Carried { exchange, number: None },
                Asked::Owner(_) => kit::SecretsAnswer::Failed,
            },
            Event::Secret { owner, value } => {
                let owner = self.owner(owner);
                let value = match value {
                    Some(value) => match kept(value) {
                        Some(value) => Some(value),
                        None => {
                            self.down(kit::Event::Secrets(kit::SecretsAnswer::Failed));
                            return;
                        }
                    },
                    None => None,
                };
                kit::SecretsAnswer::Got { owner, value }
            }
            Event::SecretsLoaded { .. }
            | Event::Switched { .. }
            | Event::Opened { .. }
            | Event::OpenRefused { .. }
            | Event::Site { .. }
            | Event::Patched { .. }
            | Event::Committed { .. }
            | Event::Failed { .. }
            | Event::FrameTooLarge { .. }
            | Event::Restored { .. }
            | Event::Loaded { .. }
            | Event::Read { .. }
            | Event::Got { .. }
            | Event::Turns { .. }
            | Event::Unread { .. }
            | Event::SecretsNotReady { .. }
            | Event::Corrupt { .. } => kit::SecretsAnswer::Failed,
        };
        self.down(kit::Event::Secrets(event));
    }
    fn owner(&mut self, token: Token) -> kit::SecretOwner {
        match self.secret_owners.remove(&token).expect("secret request owner") {
            Asked::Owner(owner) => owner,
            Asked::Carry(_) => unreachable!("the store answers a carry Carried or Stale"),
        }
    }
    fn carry(&mut self, token: Token) -> Token {
        match self.secret_owners.remove(&token).expect("secret carry owner") {
            Asked::Carry(exchange) => exchange,
            Asked::Owner(_) => unreachable!("the store answers Carried only to a carry"),
        }
    }
}
fn secret_request(request: kit::SecretsRequest) -> (Asked, jig_protocol_store::Secrets) {
    use jig_protocol_store::Secrets;
    match request {
        kit::SecretsRequest::Keep { owner, value } => {
            let (kind, number) = place(owner);
            let value = jig_protocol_store::SecretValue::Digest {
                expiry: value.expires.as_nanos(),
                digest: value.digest.expose().try_into().expect("credential digest is 32 bytes"),
            };
            (Asked::Owner(owner), Secrets::Keep { owner: kind, number, value })
        }
        kit::SecretsRequest::Erase { owner } => {
            let (kind, number) = place(owner);
            (Asked::Owner(owner), Secrets::Erase { owner: kind, number })
        }
        kit::SecretsRequest::Get { owner } => {
            let (kind, number) = place(owner);
            (Asked::Owner(owner), Secrets::Get { owner: kind, number })
        }
        kit::SecretsRequest::Hold { exchange, value } => (Asked::Carry(exchange), Secrets::Hold { value }),
        kit::SecretsRequest::Carry { exchange, value, kind } => {
            let kind = match kind {
                kit::CarriedKind::Key => jig_protocol_store::CarriedKind::Key,
                kit::CarriedKind::Value => jig_protocol_store::CarriedKind::Value,
                kit::CarriedKind::Moved => jig_protocol_store::CarriedKind::Moved,
            };
            (Asked::Carry(exchange), Secrets::Carry { value, kind })
        }
    }
}
/// Where the store keeps an owner's value; a carried value is the
/// application's, under the number the store drew (store.md 9).
fn place(owner: kit::SecretOwner) -> (jig_protocol_store::SecretOwner, u64) {
    use jig_protocol_store::SecretOwner;
    match owner {
        kit::SecretOwner::SignIn(number) => (SecretOwner::SignIn, number),
        kit::SecretOwner::Service(number) => (SecretOwner::Service, number),
        kit::SecretOwner::Carried(number) => (SecretOwner::Application, number),
    }
}
fn kept(value: jig_protocol_store::SecretValue) -> Option<kit::Kept> {
    match value {
        jig_protocol_store::SecretValue::Digest { expiry, digest } => {
            Some(kit::Kept { digest: skein_lib::Secret::new(Box::new(digest)), expires: Wall::from_nanos(expiry) })
        }
        jig_protocol_store::SecretValue::Record(_)
        | jig_protocol_store::SecretValue::Key(_)
        | jig_protocol_store::SecretValue::Carried { .. }
        | jig_protocol_store::SecretValue::Application(_)
        | jig_protocol_store::SecretValue::Held(_) => None,
    }
}
pub(super) fn printed(address: ::core::net::SocketAddr, secret: &skein_lib::Secret) -> Box<[u8]> {
    let host = address_text(address).expect("bound listener address");
    let path = http::link_path(&http_settings(), secret);
    let parts: [&[u8]; 3] = [b"http://", &host, &path];
    let mut length = 0_usize;
    for part in parts {
        length = length.checked_add(part.len()).expect("short printed URL");
    }
    let mut out = skein_lib::Writer::new(length);
    for part in parts {
        out.put(part).expect("measured URL");
    }
    out.finish()
}

fn address_text(address: ::core::net::SocketAddr) -> Option<Box<[u8]>> {
    use ::core;
    let mut text = skein_lib::Intake::with_capacity(HOST_BYTES);
    match address.ip() {
        core::net::IpAddr::V4(ip) => {
            for (at, octet) in ip.octets().iter().enumerate() {
                if at != 0 {
                    text.append(b".").ok()?;
                }
                text.append(skein_lib::Decimal::of(u64::from(*octet)).as_bytes()).ok()?;
            }
        }
        core::net::IpAddr::V6(ip) => {
            if !ip.is_loopback() {
                return None;
            }
            text.append(b"[::1]").ok()?;
        }
    }
    text.append(b":").ok()?;
    text.append(skein_lib::Decimal::of(u64::from(address.port())).as_bytes()).ok()?;
    text.meet(skein_lib::stream::Read::Fill(text.len()))
}

pub(super) fn http_settings() -> http::Settings {
    http::Settings { prefix: Box::from(b"/jig".as_slice()), public: None }
}
