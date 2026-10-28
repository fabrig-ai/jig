//! The application-held client against the real engine over native HTTP bytes.
mod accounts;
mod facts;
mod operations;
mod shapes;
mod thread;
use crate::{engine::Engine, fixtures, net::Net};
use jig_client::{self as client, Ask, Event, Handle, Key, Latest, Link, Outcome, Readable, Told, Watchable};
use jig_client_protocol_wire::v19 as w;
use jig_client_shell_native::{Conn, Connections};
use jig_client_world::{ClientId, harness::Observed};
use skein_lib::Reader;
use skein_lib::{Duration, List, Queue, Time, stream::Down};
use std::collections::BTreeMap;
use std::ops::{Deref, DerefMut};
const DEPLOYMENT_READY: Duration = Duration::from_secs(1);
const DEPLOYMENT: u32 = 0;
const SECOND_CLIENT: ClientId = ClientId(1);
const CLIENT: ClientId = ClientId(0);
/// Checks revisions from the documents the application was told changed.
#[derive(Default)]
pub struct Referee {
    revisions: Vec<(Watchable, u64)>,
    stored: BTreeMap<[u8; 16], u64>,
    /// Each value the application carried: how often it was given, and how
    /// often an exchange sent it (protocol/client.md, 5).
    carried: BTreeMap<Box<[u8]>, (u64, u64)>,
}
impl Referee {
    /// # Errors
    /// A subject was told with a revision older than one already delivered.
    pub fn document(&mut self, subject: Watchable, revision: u64) -> Result<(), &'static str> {
        if let Some((_, previous)) = self.revisions.iter_mut().find(|(held, _)| *held == subject) {
            if revision < *previous {
                return Err("document revision moved backwards");
            }
            *previous = revision;
        } else {
            self.revisions.push((subject, revision));
        }
        Ok(())
    }
    fn stored(&mut self, record: &client::Record) {
        let kept = w::Kept::decode(&w::CEILINGS, &mut Reader::new(&record.bytes)).expect("client's kept record");
        let entry = w::TableEntry::decode(&w::CEILINGS, &mut Reader::new(kept.record())).expect("table record");
        self.stored.insert(*entry.key(), entry.scope());
    }
    /// # Errors
    /// Refuses an exchange whose record is not stored or whose party is unconfirmed.
    pub fn keyed(&self, key: [u8; 16], party: Option<u64>) -> Result<(), &'static str> {
        let scope = self.stored.get(&key).ok_or("keyed exchange left before its record was stored")?;
        if Some(*scope) != party {
            return Err("keyed exchange left for an unconfirmed party");
        }
        Ok(())
    }
    /// The application handed the client a request carrying `value`.
    pub fn carry(&mut self, value: &[u8]) {
        let (given, _) = self.carried.entry(Box::from(value)).or_default();
        *given += 1;
    }
    /// # Errors
    /// Refuses an exchange carrying a value whose request was stored, whose
    /// party is unconfirmed, or that sends the value more often than the
    /// application gave it: a carried value is sent once, never resent.
    pub fn carrying(&mut self, key: [u8; 16], value: &[u8], party: Option<u64>) -> Result<(), &'static str> {
        if self.stored.contains_key(&key) {
            return Err("carried request kept in the small store");
        }
        if party.is_none() {
            return Err("carried exchange left for an unconfirmed party");
        }
        match self.carried.get_mut(value) {
            Some((given, sent)) if *sent < *given => {
                *sent += 1;
                Ok(())
            }
            Some(_) | None => Err("carried value sent more often than it was given"),
        }
    }
    /// # Errors
    /// Refuses a record for the small store that holds a carried value.
    pub fn saved(&self, bytes: &[u8]) -> Result<(), &'static str> {
        for value in self.carried.keys() {
            if !value.is_empty() && bytes.windows(value.len()).any(|held| held == value.as_ref()) {
                return Err("carried value reached the small store");
            }
        }
        Ok(())
    }
}
/// One application's client and host, connected to the story's shared engine.
pub struct Front {
    pub client: client::Client,
    connections: Connections,
    out: Queue<client::Request>,
    up: Queue<client::Event>,
    below: Queue<(Conn, Down)>,
    told: Vec<Told>,
    told_at: BTreeMap<Key, Time>,
    store: BTreeMap<Box<[u8]>, Box<[u8]>>,
    subjects: BTreeMap<Handle, Watchable>,
    referee: Referee,
    refuse_save: bool,
    seed: u64,
    exchanges: u64,
}
impl Front {
    fn new(seed: u64) -> Self {
        Self::with_limits(seed, fixtures::domain_limits())
    }
    fn with_limits(seed: u64, domain: jig_client_domain::Limits) -> Self {
        let limits = client::Limits { domain, exchanges: 8 };
        Self {
            client: client::Client::new(&limits, seed).expect("bounded client"),
            connections: Connections::new(&fixtures::connections_limits(), fixtures::HOST),
            out: Queue::uncapped(),
            up: Queue::uncapped(),
            below: Queue::uncapped(),
            told: Vec::new(),
            told_at: BTreeMap::new(),
            store: BTreeMap::new(),
            subjects: BTreeMap::new(),
            referee: Referee::default(),
            refuse_save: false,
            seed,
            exchanges: 0,
        }
    }
    fn event(&mut self, now: Time, event: Event) {
        if let Event::Carry { value: Some(value), .. } = &event {
            self.referee.carry(value.expose());
        }
        self.client.step(now, fixtures::wall(now), event, &mut self.out);
    }
    fn advance(&mut self, now: Time, id: ClientId, net: &mut Net, engine: &mut Engine) -> bool {
        let active = !self.up.is_empty() || !self.out.is_empty() || !self.below.is_empty();
        while let Some(event) = self.up.pop() {
            self.event(now, event);
        }
        while let Some(request) = self.out.pop() {
            self.request(now, request);
        }
        self.connections.take(now, &mut self.up, &mut self.below);
        while let Some((conn, down)) = self.below.pop() {
            if let Some(event) = net.down(id, conn, down) {
                engine.io_event(event);
            }
        }
        self.client.reclaim();
        active
    }
    fn request(&mut self, now: Time, request: client::Request) {
        match request {
            client::Request::Authorized(_) | client::Request::ServiceCredential { .. } => {}
            client::Request::Told { told } => {
                if let Told::Changed { handle } = &told
                    && let Some(subject) = self.subjects.get(handle)
                    && let Some(document) = self.client.document(*handle)
                    && let Some(Latest::Jig(client::contract::Delivery::Task(task))) = document.latest
                {
                    self.referee.document(subject.clone(), task.spec_revision.value).expect("ordered document");
                }
                if let Told::Answered { key, .. } = &told {
                    self.told_at.insert(*key, now);
                }
                self.told.push(told);
            }
            client::Request::Save { records } => {
                if core::mem::take(&mut self.refuse_save) {
                    self.event(now, Event::NotStored);
                } else {
                    for record in records.into_boxed() {
                        self.referee.saved(&record.bytes).expect("no carried value in the small store");
                        if record.key.as_ref() != client::STREAM_KEY {
                            self.referee.stored(&record);
                        }
                        self.store.insert(record.key, record.bytes);
                    }
                    self.event(now, Event::Stored);
                }
            }
            client::Request::Erase { records } => {
                for key in records.into_boxed() {
                    self.store.remove(&key);
                }
                self.event(now, Event::Stored);
            }
            client::Request::Exchange { exchange, credential, message } => {
                self.exchanges += 1;
                if let client::Message::Request(bytes) = &message {
                    let keyed =
                        w::Keyed::decode(&w::CEILINGS, &mut Reader::new(bytes)).expect("client's keyed request");
                    let party = self.client.link().party;
                    if let Some(value) = keyed.value() {
                        self.referee.carrying(*keyed.key(), value.expose(), party).expect("a carried value sent once");
                    } else {
                        self.referee.keyed(*keyed.key(), party).expect("kept and confirmed exchange");
                    }
                }
                self.connections.down(
                    now,
                    client::Request::Exchange { exchange, credential, message },
                    &mut self.below,
                );
            }
            client::Request::OpenStream { credential } => {
                self.connections.down(now, client::Request::OpenStream { credential }, &mut self.below);
            }
            client::Request::Down { .. } | client::Request::SignOut => panic!("linked clients emit wire work"),
            client::Request::CloseStream => {
                self.connections.down(now, client::Request::CloseStream, &mut self.below);
            }
        }
        let limits = client::Limits { domain: fixtures::domain_limits(), exchanges: 8 };
        assert!(
            self.store.keys().filter(|key| key.len() == 16).count()
                <= usize::try_from(limits.domain.requests.requests).expect("table bound fits"),
            "bounded small store"
        );
    }
    #[must_use]
    pub fn answered(&self, key: Key) -> Vec<Outcome> {
        self.told
            .iter()
            .filter_map(|t| {
                if let Told::Answered { key: held, outcome } = t {
                    (*held == key).then(|| outcome.clone())
                } else {
                    None
                }
            })
            .collect()
    }
    #[must_use]
    pub fn kept(&self, key: Key) -> bool {
        self.told.iter().any(|t| matches!(t, Told::Kept { key: held, .. } if *held == key))
    }
    #[must_use]
    pub fn told_links(&self) -> Vec<Link> {
        self.told.iter().filter_map(|t| if let Told::Link { link } = t { Some(*link) } else { None }).collect()
    }
    #[must_use]
    pub fn pages(&self, handle: Handle) -> Vec<client::Readout> {
        self.told
            .iter()
            .filter_map(|t| {
                if let Told::Page { handle: held, page } = t { (*held == handle).then(|| page.clone()) } else { None }
            })
            .collect()
    }
    #[must_use]
    pub fn told_at(&self, key: Key) -> Time {
        self.told_at[&key]
    }
    #[must_use]
    pub fn exchanges(&self) -> u64 {
        self.exchanges
    }
}
/// Scripted applications against one durable real engine over HTTP bytes.
pub struct Story {
    front: Front,
    second: Option<Front>,
    engine: Engine,
    net: Net,
    now: Time,
    seen: Vec<Observed>,
}
impl Deref for Story {
    type Target = Front;
    fn deref(&self) -> &Front {
        &self.front
    }
}
impl DerefMut for Story {
    fn deref_mut(&mut self) -> &mut Front {
        &mut self.front
    }
}
impl Story {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        let mut limits = fixtures::owner_limits();
        limits.core.people.sign_ins = 3;
        Self::bounded(seed, &limits)
    }
    fn bounded(seed: u64, limits: &jig_test_domain::Limits) -> Self {
        Self::configured(seed, limits, jig_core_world::world::config(seed))
    }
    fn configured(seed: u64, limits: &jig_test_domain::Limits, config: jig_test_domain::Config) -> Self {
        Self::configured_peers(seed, limits, config, fixtures::peers())
    }
    fn configured_peers(
        seed: u64,
        limits: &jig_test_domain::Limits,
        config: jig_test_domain::Config,
        peers: jig_core_world::peers::Peers,
    ) -> Self {
        let mut story = Self {
            front: Front::new(seed),
            second: None,
            engine: Engine::with_peers(seed, limits, config, peers),
            net: Net::new(seed),
            now: Time::ZERO,
            seen: Vec::new(),
        };
        story.event(Event::Start { kept: List::with_capacity(0) });
        story.advance();
        story
    }
    #[must_use]
    pub fn live(seed: u64) -> Self {
        let mut story = Self::new(seed);
        story.sign_in();
        assert!(story.run_until(|s| s.client.link().state == Link::Live, Duration::from_secs(1)));
        story
    }
    pub fn sign_in(&mut self) {
        let printed = self.engine.link.clone();
        let secret = self.front.connections.printed(&printed).expect("engine's printed link");
        self.event(Event::SignIn { with: client::With::Link { secret, held: None } });
        self.advance();
        if self.client.link().state == Link::SignedOut && self.engine.link != printed {
            let secret = self.front.connections.printed(&self.engine.link).expect("fresh printed link");
            self.event(Event::SignIn { with: client::With::Link { secret, held: None } });
        }
    }
    pub fn watch(&mut self, handle: Handle, subject: Watchable) {
        self.front.subjects.insert(handle, subject.clone());
        self.event(Event::Watch { handle, subject });
    }
    pub fn read(&mut self, handle: Handle, read: Readable) {
        self.event(Event::Read { handle, read });
    }
    pub fn send(&mut self, key: Key, request: Ask) {
        self.event(Event::Send { name: None, key, request });
    }
    /// Sends `request` under the application's `key`, named `name`.
    pub fn send_named(&mut self, key: Key, name: skein_lib::RequestKey, request: Ask) {
        self.event(Event::Send { key, name: Some(name), request });
    }
    /// Sends a request carrying `value` under `name` (protocol/client.md, 5).
    pub fn carry(&mut self, key: Key, name: skein_lib::RequestKey, request: Ask, value: &[u8]) {
        self.event(Event::Carry { key, name, request, value: Some(skein_lib::Secret::new(Box::from(value))) });
    }
    /// The numbers of the carried values the engine's secrets hold.
    #[must_use]
    pub fn carried(&self) -> Vec<u64> {
        self.engine.carried.keys().copied().collect()
    }
    pub fn event(&mut self, event: Event) {
        self.front.event(self.now, event);
    }
    pub fn cut_answer(&mut self) {
        self.net.cut_answer();
    }
    pub fn cut_stream(&mut self) {
        self.net.cut_stream(CLIENT);
    }
    pub fn refuse_next_save(&mut self) {
        self.front.refuse_save = true;
    }
    pub fn reload(&mut self) {
        let credential = self.client.link().credential.cloned().expect("signed in before reload");
        self.net.cut_client(CLIENT);
        self.advance();
        self.net.retire(CLIENT);
        let mut front = Front::new(self.front.seed);
        front.store = core::mem::take(&mut self.front.store);
        let mut kept = List::with_capacity(u32::try_from(front.store.len()).expect("bounded store"));
        for (key, bytes) in &front.store {
            let record = client::Record { key: key.clone(), bytes: bytes.clone() };
            if record.key.as_ref() != client::STREAM_KEY {
                front.referee.stored(&record);
            }
            kept.push(record).expect("bounded store");
        }
        front.event(self.now, Event::Start { kept });
        front.event(self.now, Event::SignIn { with: client::With::Held(credential) });
        self.front = front;
    }
    pub fn restart_engine(&mut self, offline: Duration) {
        self.net.cut_all();
        self.advance();
        self.engine.driver.restart(jig_core_world::world::config(self.engine.seed), self.now, fixtures::wall(self.now));
        self.net.offline(self.now.saturating_add(offline));
        while self.engine.out.pop().is_some() {}
        self.engine.component = jig_protocol_engine_clients::Component::new(
            fixtures::settings(self.engine.limits.core.documents.rows),
            &self.engine.env,
            &mut self.engine.out,
        )
        .expect("bounded replacement");
    }
    #[must_use]
    pub fn live_deployment(seed: u64) -> Self {
        let mut limits = fixtures::owner_limits();
        limits.core.people.projects = 2;
        let mut story = Self::bounded(seed, &limits);
        story.sign_in();
        assert!(story.run_until(|s| s.client.link().state == Link::Live, DEPLOYMENT_READY));
        story.grant_deployment_role();
        story
    }
    fn grant_deployment_role(&mut self) {
        self.engine.driver.world.send(jig_test_domain::Event::Core(jig_domain_core::Event::People(
            jig_domain_people::Event::Roles {
                project: DEPLOYMENT,
                holdings: Box::new([jig_domain_people::Holding {
                    person: fixtures::FIRST_OWNER,
                    role: jig_domain_people::Role::Owner,
                }]),
            },
        )));
    }
    pub fn restart_serving(&mut self, served: List<jig_protocol_engine_clients::Served>) {
        self.restart_engine(Duration::ZERO);
        let mut settings = fixtures::settings(self.engine.limits.core.documents.rows);
        settings.served = served;
        while self.engine.out.pop().is_some() {}
        self.engine.component =
            jig_protocol_engine_clients::Component::new(settings, &self.engine.env, &mut self.engine.out)
                .expect("bounded served replacement");
    }
    #[must_use]
    pub fn application_asks(&self) -> Vec<jig_domain_people::Ask> {
        self.engine
            .driver
            .world
            .store
            .rows
            .values()
            .filter_map(|row| match row {
                jig_test_domain::Record::Core(jig_domain_core::Record::People(jig_domain_people::Stored::Answer {
                    ask,
                    ..
                })) if matches!(ask.as_ref(), jig_domain_people::Ask::Application { .. }) => Some(ask.as_ref().clone()),
                jig_test_domain::Record::Core(_) | jig_test_domain::Record::Connector { .. } => None,
            })
            .collect()
    }
    pub fn serve_jig(&mut self, version: u32) {
        self.net.cut_all();
        self.advance();
        let mut settings = fixtures::settings(self.engine.limits.core.documents.rows);
        settings.served = jig_client_world::test::list([
            jig_protocol_engine_clients::Served { family: None, first: version, last: version },
            jig_protocol_engine_clients::Served { family: Some(1), first: 2, last: 2 },
        ]);
        while self.engine.out.pop().is_some() {}
        self.engine.component =
            jig_protocol_engine_clients::Component::new(settings, &self.engine.env, &mut self.engine.out)
                .expect("bounded version replacement");
    }
    pub fn add_second_client(&mut self) {
        let mut second = Front::new(self.front.seed.checked_add(1).expect("second seed"));
        let printed = self.engine.link.clone();
        let secret = second.connections.printed(&printed).expect("engine's printed link");
        second.event(self.now, Event::Start { kept: List::with_capacity(0) });
        second.event(self.now, Event::SignIn { with: client::With::Link { secret, held: None } });
        self.second = Some(second);
        self.advance();
        // A consumed printed link is refused and the engine prints its replacement.
        if self.second().client.link().state == Link::SignedOut && self.engine.link != printed {
            let second = self.second.as_mut().expect("second client attached");
            let secret = second.connections.printed(&self.engine.link).expect("fresh printed link");
            second.event(self.now, Event::SignIn { with: client::With::Link { secret, held: None } });
        }
        assert!(self.run_until(|s| s.second().client.link().state == Link::Live, Duration::from_secs(1)));
    }
    #[must_use]
    pub fn second(&self) -> &Front {
        self.second.as_ref().expect("second client attached")
    }
    pub fn send_second(&mut self, key: Key, request: Ask) {
        self.second.as_mut().expect("second client attached").event(self.now, Event::Send { name: None, key, request });
    }
    pub fn start_task(&mut self) -> u64 {
        self.engine.driver.scripted_party(fixtures::starter(), self.now, fixtures::wall(self.now));
        self.engine.driver.world.send(jig_test_domain::Event::Timer(jig_domain_core::Timer::People));
        self.advance();
        self.engine
            .driver
            .world
            .store
            .rows
            .values()
            .find_map(|row| match row {
                jig_test_domain::Record::Core(jig_domain_core::Record::People(jig_domain_people::Stored::Answer {
                    outcome: jig_domain_people::Outcome::Created { first: task, .. },
                    ..
                })) => Some(*task),
                jig_test_domain::Record::Core(_) | jig_test_domain::Record::Connector { .. } => None,
            })
            .expect("scripted party started a task")
    }
    /// How many durable answers the engine made under the engine key `key`.
    #[must_use]
    pub fn made_under(&self, key: skein_lib::RequestKey) -> usize {
        self.seen.iter().filter(|o| matches!(o, Observed::Made { key: held, .. } if *held == key)).count()
    }
    #[must_use]
    pub fn made(&self, party: u64) -> usize {
        self.seen.iter().filter(|o| matches!(o, Observed::Made { party: held, .. } if *held == party)).count()
    }
    #[must_use]
    pub fn durable_at(&self, party: u64) -> Time {
        let commit = self
            .seen
            .iter()
            .find_map(|o| match o {
                Observed::Made { party: held, commit, .. } if *held == party => Some(*commit),
                Observed::Durable { .. }
                | Observed::Made { .. }
                | Observed::Kept { .. }
                | Observed::Retained { .. }
                | Observed::Stopped { .. }
                | Observed::Ended { .. }
                | Observed::Reachable { .. }
                | Observed::SignInEnded { .. } => None,
            })
            .expect("party's made commit");
        self.seen
            .iter()
            .find_map(|o| match o {
                Observed::Durable { commit: held, at } if *held == commit => Some(*at),
                Observed::Durable { .. }
                | Observed::Made { .. }
                | Observed::Kept { .. }
                | Observed::Retained { .. }
                | Observed::Stopped { .. }
                | Observed::Ended { .. }
                | Observed::Reachable { .. }
                | Observed::SignInEnded { .. } => None,
            })
            .expect("made commit is durable")
    }
    #[must_use]
    pub fn deliveries_taken(&self) -> bool {
        !self.engine.offered.is_empty() && self.engine.offered == self.engine.taken
    }
    pub fn run_until(&mut self, done: impl Fn(&Self) -> bool, within: Duration) -> bool {
        let end = self.now.saturating_add(within);
        for _ in 0..100_000 {
            self.advance();
            if done(self) {
                return true;
            }
            if self.now >= end {
                return false;
            }
            let second = self
                .second
                .iter()
                .flat_map(|f| f.client.next_deadline().into_iter().chain(f.connections.next_deadline()));
            let next = self
                .client
                .next_deadline()
                .into_iter()
                .chain(self.front.connections.next_deadline())
                .chain(second)
                .chain(self.engine.component.next_deadline())
                .chain(self.engine.server.next_deadline())
                .chain(self.engine.driver.world.next_due())
                .chain(self.net.next_due())
                .filter(|t| *t > self.now)
                .min()
                .unwrap_or(end);
            self.now = next.min(end);
            self.front.client.fire(self.now, fixtures::wall(self.now), &mut self.front.out);
            if let Some(second) = &mut self.second {
                second.client.fire(self.now, fixtures::wall(self.now), &mut second.out);
            }
        }
        panic!("story did not settle")
    }
    fn advance(&mut self) {
        self.engine.clock(self.now, fixtures::wall(self.now));
        self.net.clock(self.now);
        let mut quiet = 0;
        for _ in 0..100_000 {
            let mut active = self.engine.poll();
            while let Some(request) = self.engine.out.pop() {
                active = true;
                if let Some(request) = self.engine.request(request)
                    && let Some(event) = self.net.io(request)
                {
                    self.engine.io_event(event);
                }
            }
            while let Some(request) = self.engine.io() {
                active = true;
                if let Some(event) = self.net.io(request) {
                    self.engine.io_event(event);
                }
            }
            active |= self.front.advance(self.now, CLIENT, &mut self.net, &mut self.engine);
            if let Some(second) = &mut self.second {
                active |= second.advance(self.now, SECOND_CLIENT, &mut self.net, &mut self.engine);
            }
            let (events, clients, changed) = self.net.advance(&mut self.engine.rules);
            active |= changed;
            for event in events {
                self.engine.io_event(event);
            }
            for (id, conn, up) in clients {
                let front = if id == CLIENT {
                    &mut self.front
                } else {
                    assert_eq!(id, SECOND_CLIENT);
                    self.second.as_mut().expect("second client attached")
                };
                front.connections.up(self.now, conn, up, &mut front.up, &mut front.below);
            }
            self.seen.extend(self.engine.observed());
            quiet = if active { 0 } else { quiet + 1 };
            if quiet == 32 || (!active && self.engine.out.is_empty() && !self.net.pending()) {
                return;
            }
        }
        panic!("client HTTP bytes did not quiesce")
    }
}

pub fn transient_request(seed: u64, fault: &jig_client_world::FrontFault) {
    let mut limits = fixtures::owner_limits();
    limits.core.people.requests = 1;
    limits.core.people.request_retention = Duration::from_millis(100);
    let mut story = Story::bounded(seed, &limits);
    let mut receiving = fixtures::domain_limits();
    receiving.requests.margin = Duration::from_nanos(1);
    story.front.client = client::Client::new(&client::Limits { domain: receiving, exchanges: 8 }, seed)
        .expect("short owner retention fixture");
    story.sign_in();
    assert!(story.run_until(|s| s.client.link().state == Link::Live, Duration::from_secs(1)));
    let not_ready = match fault {
        jig_client_world::FrontFault::Busy => {
            story.engine.driver.scripted_party(crate::stories::pressure(), story.now, fixtures::wall(story.now));
            false
        }
        jig_client_world::FrontFault::NotReady => {
            story.engine.driver.pause_restore(
                jig_core_world::world::config(seed),
                story.now,
                fixtures::wall(story.now),
            );
            true
        }
        jig_client_world::FrontFault::Restart { .. }
        | jig_client_world::FrontFault::Slow { .. }
        | jig_client_world::FrontFault::DropWatch
        | jig_client_world::FrontFault::NoAnswer
        | jig_client_world::FrontFault::SignedOut
        | jig_client_world::FrontFault::Versions
        | jig_client_world::FrontFault::ObjectGone => unreachable!("this story checks transient request refusals"),
    };
    let released = story.now.saturating_add(Duration::from_nanos(100_000_001));
    story.now = story.now.saturating_add(Duration::from_millis(50));
    story.advance();
    let key = Key { asker: client::Asker::JIG, number: 62 };
    story.send(key, Ask::Jig(Box::new(client::contract::Ask::Stop { project: 1, task: u64::MAX })));
    assert!(
        story.run_until(
            |s| s.client.pending(key).is_some_and(|p| matches!(p.progress, client::Progress::Retrying { .. })),
            Duration::from_millis(50)
        ),
        "actual busy"
    );
    let saved = *story.front.referee.stored.keys().next().expect("the client's retained original key");
    story.now = released;
    if not_ready {
        story.engine.driver.finish_restore(story.now, fixtures::wall(story.now));
    }
    assert!(
        story.run_until(|s| !s.answered(key).is_empty(), Duration::from_secs(1)),
        "busy retry is a real final owner answer"
    );
    assert_eq!(
        story.seen.iter().filter(|value| matches!(value, Observed::Made { key, .. } if *key == saved)).count(),
        1,
        "busy key is made once"
    );
}
