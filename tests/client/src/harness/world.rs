//! Tabs retain records and history; epochs discard old deliveries. Each callback
//! writes its store, dispatches wire duties, then applies and checks patches
//! (client/domain/testing.md, 4; client/domain/view.md, 11).
mod decisions;
#[cfg(test)]
mod words_tests;
use super::{App, CardKey, Duty, Fault, Front, FrontFault, Observation, StoreWrite};
use crate::model::Model;
use skein_fake_person::{Doing, Next, Person, RandomDoing, RandomPerson, Step, TreeFace};
use skein_lib::{Duration, Env, Queue, Rng, Time, Wall};
use skein_ui::Act;
use skein_ui::{NodeId, Tree};
use skein_world::domain::{Referee, Schedule, Span, Trace, Verdict};

/// One tab or device in this world.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ClientId(pub usize);
/// Constructs one application's root and view from the world's next seed.
pub type ClientConstructor<A> = fn(u64) -> (<A as App>::Root, <A as App>::View);
/// One scripted or random person.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PersonId(pub usize);
/// Deterministic latency, storage and iteration bounds.
#[derive(Clone, Debug)]
pub struct Settings {
    pub seed: u64,
    /// Complete public-word language, including application overrides.
    pub words: skein_ui::phrases::Language,
    pub latency: Span,
    pub store_records: usize,
    pub iterations: u32,
    pub memory: bool,
    /// Practical owned-heap guard per client, in addition to the composed envelope.
    pub heap_bytes_per_client: u64,
    pub random_steps: u32,
    pub word_bytes: u32,
    pub faults_per_mille: u32,
    pub fault_limit: u32,
    pub heartbeat: Duration,
    pub request_within: Duration,
    pub link_within: Duration,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            seed: 1,
            words: jig_client::words::english(),
            latency: Span::millis(1, 3),
            store_records: 32,
            iterations: 4096,
            memory: false,
            heap_bytes_per_client: 16 * 1024 * 1024,
            random_steps: 32,
            word_bytes: 32,
            faults_per_mille: 0,
            fault_limit: 32,
            heartbeat: Duration::from_millis(100),
            request_within: Duration::from_secs(2),
            link_within: Duration::from_millis(500),
        }
    }
}
/// The boundary at which a run yields its evidence.
#[derive(Clone, Copy, Debug)]
pub enum Until {
    Done,
    Time(Time),
    Steps(u32),
}
/// Clock, callback and heap evidence from a completed run.
#[derive(Debug)]
pub struct Outcome {
    /// Observation-based referee result at this run boundary.
    pub verdict: Verdict,
    /// Safety checks and liveness obligations met.
    pub judged: (u64, u64),
    pub now: Time,
    pub iterations: u32,
    pub callbacks: u64,
    pub faults: u64,
    pub dropped: u64,
    pub trace: Vec<String>,
    pub heap_peak: u64,
    pub heap_bound: u64,
}
struct TextShell {
    print: skein_ui_text::Print,
    held: Option<(NodeId, Box<[u8]>)>,
}
struct Client<A: App> {
    constructor: ClientConstructor<A>,
    limits: A::Limits,
    heap_bound: u64,
    root: A::Root,
    view: A::View,
    model: Model,
    out: Queue<A::Request>,
    patches: Queue<skein_ui::Patch>,
    kept: Vec<A::Record>,
    deferred: Vec<StoreWrite<A::Record, A::Key>>,
    history: Vec<A::Address>,
    at: usize,
    party: u64,
    epoch: u64,
    navigation: u64,
    delivery: Time,
    acted_card: Option<CardKey>,
    acted_scene: Option<jig_client_test_kit::check::Scene>,
    decisions: std::collections::BTreeSet<[u8; 16]>,
    text_shell: Option<TextShell>,
}
enum PersonKind<A: App> {
    Script(Person<A::Address>),
    Random { person: RandomPerson, left: u32, except: Option<&'static [u8]> },
}
struct Walker<A: App> {
    client: ClientId,
    person: PersonKind<A>,
    done: bool,
    wake: Option<Time>,
}
struct Due<A: App> {
    client: ClientId,
    epoch: u64,
    event: A::Event,
    navigation: Option<u64>,
}
/// One deterministic loop shared by client, joint and application worlds.
pub struct World<A: App, F: Front<A>> {
    settings: Settings,
    front: F,
    clients: Vec<Client<A>>,
    people: Vec<Walker<A>>,
    schedule: Schedule<Due<A>>,
    faults: Schedule<Fault>,
    faults_drawn: u64,
    last_removed: Option<(ClientId, NodeId)>,
    rng: Rng,
    trace: Trace,
    referee: Referee<super::referee::Rules>,
    now: Time,
    callbacks: u64,
    dropped: u64,
    quota_failure: bool,
    meter: Option<skein_world::domain::heap::Meter>,
    heap_peak: u64,
    default_heap_bound: Option<u64>,
}
impl<A: App, F: Front<A>> World<A, F> {
    /// Creates one seeded loop in front of the supplied engine boundary.
    #[must_use]
    pub fn new(settings: Settings, front: F) -> Self {
        let meter = if settings.memory { Some(skein_world::domain::heap::Meter::new()) } else { None };
        Self {
            meter,
            default_heap_bound: None,
            heap_peak: 0,
            rng: Rng::new(settings.seed),
            referee: Referee::new(super::referee::Rules::with_words(
                settings.heartbeat,
                settings.request_within,
                settings.link_within,
                &settings.words,
            )),
            settings,
            front,
            clients: Vec::new(),
            people: Vec::new(),
            schedule: Schedule::new(),
            faults: Schedule::new(),
            faults_drawn: 0,
            last_removed: None,
            trace: Trace::default(),
            now: Time::ZERO,
            callbacks: 0,
            dropped: 0,
            quota_failure: false,
        }
    }
    /// Configure bounded random exploration before admitting any clients.
    pub fn randomness(&mut self, steps: u32, faults_per_mille: u32, fault_limit: u32) {
        assert!(self.clients.is_empty(), "configure randomness before admitting clients");
        assert!(faults_per_mille <= 1000, "per-mille probability");
        self.settings.random_steps = steps;
        self.settings.faults_per_mille = faults_per_mille;
        self.settings.fault_limit = fault_limit;
    }
    /// Count every client/view/model heap iteration from this fixture boundary.
    pub fn measure_heap(&mut self) {
        assert!(self.clients.is_empty(), "start the meter before admitting clients");
        self.meter = Some(skein_world::domain::heap::Meter::new());
    }
    /// Starts a device with an empty small store.
    pub fn client(&mut self, party: u64, address: A::Address) -> ClientId {
        let bound = *self
            .default_heap_bound
            .get_or_insert_with(|| A::worst_case(&A::limits()).min(self.settings.heap_bytes_per_client));
        self.make_client(party, address, Vec::new(), A::limits(), A::new, bound)
    }
    /// Starts a device with supplied receiving bounds and a root/view fixture.
    /// The constructor must use the same bounds; tabs and reloads retain both.
    pub fn client_with(
        &mut self,
        party: u64,
        address: A::Address,
        limits: A::Limits,
        constructor: ClientConstructor<A>,
    ) -> ClientId {
        let heap_bound = A::worst_case(&limits).min(self.settings.heap_bytes_per_client);
        self.make_client(party, address, Vec::new(), limits, constructor, heap_bound)
    }
    /// Play this client's subsequent person acts through the printed numbers.
    pub fn at_text_shell(&mut self, client: ClientId) {
        self.clients[client.0].text_shell = Some(TextShell {
            print: skein_ui_text::Print::new(&A::view_limits(&self.clients[client.0].view)).expect("print bounds fit"),
            held: None,
        });
    }
    fn make_client(
        &mut self,
        party: u64,
        address: A::Address,
        kept: Vec<A::Record>,
        limits: A::Limits,
        constructor: ClientConstructor<A>,
        heap_bound: u64,
    ) -> ClientId {
        if let Some(meter) = &self.meter {
            meter.start();
        }
        let constructor = self.front.constructor(constructor);
        let (root, view) = constructor(self.rng.next_u64());
        let id = ClientId(self.clients.len());
        let patches = Queue::with_capacity(A::patch_limit(&view));
        let out = Queue::uncapped();
        self.clients.push(Client {
            constructor,
            limits,
            heap_bound,
            patches,
            out,
            root,
            view,
            model: Model::default(),
            kept: kept.clone(),
            deferred: Vec::new(),
            history: vec![address.clone()],
            at: 0,
            party,
            epoch: 0,
            navigation: 0,
            delivery: self.now,
            acted_card: None,
            acted_scene: None,
            decisions: std::collections::BTreeSet::new(),
            text_shell: None,
        });
        self.observe(Observation::Client { client: id });
        self.front.bind(id, party);
        for event in self.front.starting(id) {
            self.callback(id, Some(event));
        }
        self.callback(id, Some(A::start(address, kept, 0)));
        self.check_heap();
        id
    }
    /// Starts another tab with this tab’s address and kept records.
    pub fn tab(&mut self, of: ClientId) -> ClientId {
        let c = &self.clients[of.0];
        self.make_client(
            c.party,
            c.history[c.at].clone(),
            c.kept.clone(),
            c.limits.clone(),
            c.constructor,
            c.heap_bound,
        )
    }
    /// Adds a person whose script acts only through the visible tree.
    pub fn person(&mut self, client: ClientId, steps: Vec<Step<A::Address>>) -> PersonId {
        let id = PersonId(self.people.len());
        self.people.push(Walker {
            client,
            person: PersonKind::Script(Person::new(steps.into_boxed_slice())),
            done: false,
            wake: None,
        });
        id
    }
    /// Adds a seeded person with a bounded number of actions.
    pub fn random_person(&mut self, client: ClientId) -> PersonId {
        let id = PersonId(self.people.len());
        self.people.push(Walker {
            client,
            person: PersonKind::Random {
                person: RandomPerson::new(self.rng.next_u64(), self.settings.word_bytes),
                left: self.settings.random_steps,
                except: None,
            },
            done: false,
            wake: None,
        });
        id
    }
    /// Add random exploration that skips pressing one named button in this story.
    pub fn random_person_without_button(&mut self, client: ClientId, name: &'static [u8]) -> PersonId {
        let id = self.random_person(client);
        if let PersonKind::Random { except, .. } = &mut self.people[id.0].person {
            *except = Some(name);
        }
        id
    }
    /// Restarts this tab from its kept records and advances its delivery epoch.
    pub fn reload(&mut self, id: ClientId) {
        self.observe(Observation::Reloaded { client: id });
        self.clients[id.0].acted_card = None;
        self.front.reload(id, self.now);
        let c = &mut self.clients[id.0];
        c.epoch += 1;
        c.acted_card = None;
        let (root, view) = (c.constructor)(self.rng.next_u64());
        c.root = root;
        c.view = view;
        c.model = Model::default();
        c.deferred.clear();
        c.kept = self.front.restored(id, c.kept.clone());
        let event = A::start(c.history[c.at].clone(), c.kept.clone(), 0);
        for initial in self.front.starting(id) {
            self.callback(id, Some(initial));
        }
        self.callback(id, Some(event));
    }
    /// Navigates and records an address in the shell history.
    pub fn go(&mut self, id: ClientId, address: A::Address) {
        self.observe(Observation::LeftPage { client: id });
        self.clients[id.0].acted_card = None;
        self.advance_navigation(id);
        self.history(id, address.clone(), false);
        self.callback(id, Some(A::went(address)));
    }
    /// Moves to the previous history entry when present.
    pub fn back(&mut self, id: ClientId) {
        if self.clients[id.0].at > 0 {
            self.observe(Observation::LeftPage { client: id });
            self.clients[id.0].acted_card = None;
            self.advance_navigation(id);
            self.clients[id.0].at -= 1;
            let address = self.clients[id.0].history[self.clients[id.0].at].clone();
            self.callback(id, Some(A::went(address)));
        }
    }
    /// Moves to the next history entry when present.
    pub fn forward(&mut self, id: ClientId) {
        if self.clients[id.0].at + 1 < self.clients[id.0].history.len() {
            self.observe(Observation::LeftPage { client: id });
            self.clients[id.0].acted_card = None;
            self.advance_navigation(id);
            self.clients[id.0].at += 1;
            let address = self.clients[id.0].history[self.clients[id.0].at].clone();
            self.callback(id, Some(A::went(address)));
        }
    }
    /// Reports a platform act and mirrors a local field edit before diffing.
    pub fn act(&mut self, id: ClientId, node: NodeId, act: Act) {
        self.clients[id.0].acted_scene =
            Some(jig_client_test_kit::check::scene(A::tree(&self.clients[id.0].view), node));
        if let Some(card) = A::card(A::tree(&self.clients[id.0].view), node) {
            self.clients[id.0].acted_card = Some(card);
        }
        if let Act::Input { value } = &act {
            self.clients[id.0].model.input(node, value);
        }
        if let Some(event) = A::act(&mut self.clients[id.0].view, node, act) {
            self.callback(id, Some(event));
        }
    }
    /// Delivers a shell event through the same callback path.
    pub fn event(&mut self, id: ClientId, event: A::Event) {
        self.callback(id, Some(event));
    }
    /// Makes the next callback’s store writes fail before dispatch.
    pub fn fail_store(&mut self) {
        self.quota_failure = true;
    }
    /// Injects a transport fault into the front.
    pub fn fault(&mut self, fault: impl Into<Fault>) {
        let fault = fault.into();
        self.faults_drawn += 1;
        self.trace.log(self.now, format!("fault {fault:?}"));
        match fault {
            Fault::Front(fault) => self.front.fault(self.now, fault),
            Fault::DoublePress { client, node } => {
                self.act(client, node, Act::Press);
                self.act(client, node, Act::Press);
            }
            Fault::Reload { client } => self.reload(client),
            Fault::StalePress { client, node } => self.act(client, node, Act::Press),
            Fault::OtherParty { client, party } => self.sign_as(client, party),
            Fault::TwoTabs { first, first_node, second, second_node } => {
                self.act(first, first_node, Act::Press);
                self.act(second, second_node, Act::Press);
            }
        }
    }
    /// Schedules a fault alongside deliveries and the owners' deadlines.
    pub fn fault_at(&mut self, at: Time, fault: impl Into<Fault>) {
        self.faults.send(at, fault.into());
    }
    /// A different shell sign-in restarts the client with its existing records.
    pub fn sign_as(&mut self, client: ClientId, party: u64) {
        self.clients[client.0].party = party;
        self.front.bind(client, party);
        self.front.sign_in(client, self.now, Wall::from_nanos(1_000_000_000_000 + self.now.as_nanos()));
        self.reload(client);
    }
    fn draw_fault(&mut self) {
        if self.clients.is_empty()
            || self.faults_drawn >= u64::from(self.settings.fault_limit)
            || !self.people.iter().any(|p| !p.done)
            || !self.rng.chance(self.settings.faults_per_mille)
        {
            return;
        }
        let client = ClientId(
            usize::try_from(self.rng.below(u64::try_from(self.clients.len()).expect("clients"))).expect("client index"),
        );
        let fault = match self.rng.below(14) {
            0 => Fault::Front(FrontFault::Busy),
            1 => Fault::Front(FrontFault::NotReady),
            2 => Fault::Front(FrontFault::Restart { offline: Duration::from_millis(40) }),
            3 => Fault::Front(FrontFault::Slow { span: Duration::from_millis(20) }),
            4 => Fault::Front(FrontFault::DropWatch),
            5 => Fault::Front(FrontFault::NoAnswer),
            6 => Fault::Reload { client },
            7 => {
                let Some(node) = A::tree(&self.clients[client.0].view).nodes().iter().find(|n| {
                    n.role == skein_ui::Role::Button
                        && n.binding.is_some()
                        && !n.states.iter().any(|s| *s == skein_ui::State::Disabled)
                }) else {
                    return;
                };
                Fault::DoublePress { client, node: node.id }
            }
            8 => Fault::Front(FrontFault::ObjectGone),
            9 => Fault::Front(FrontFault::SignedOut),
            10 => Fault::Front(FrontFault::Versions),
            11 => {
                let Some(other) = self.clients.iter().find(|other| other.party != self.clients[client.0].party) else {
                    return;
                };
                Fault::OtherParty { client, party: other.party }
            }
            12 => {
                let Some((client, node)) = self.last_removed else {
                    return;
                };
                Fault::StalePress { client, node }
            }
            13 => {
                let c = &self.clients[client.0];
                let Some((other, second_node, first_node)) = self
                    .clients
                    .iter()
                    .enumerate()
                    .filter(|(i, o)| *i != client.0 && o.party == c.party && o.history[o.at] == c.history[c.at])
                    .find_map(|(i, o)| {
                        A::tree(&c.view)
                            .nodes()
                            .iter()
                            .filter(|n| n.role == skein_ui::Role::Button && n.binding.is_some())
                            .find_map(|n| {
                                A::tree(&o.view)
                                    .nodes()
                                    .iter()
                                    .find(|m| m.role == n.role && m.name == n.name && m.binding.is_some())
                                    .map(|m| (ClientId(i), m.id, n.id))
                            })
                    })
                else {
                    return;
                };
                Fault::TwoTabs { first: client, first_node, second: other, second_node }
            }
            _ => unreachable!("fourteen choices"),
        };
        self.fault(fault);
    }
    /// Exposes the truth boundary for fixture changes.
    pub fn front(&mut self) -> &mut F {
        &mut self.front
    }
    /// The application's root, for fixture setup through its public client.
    #[must_use]
    pub fn root(&self, id: ClientId) -> &A::Root {
        &self.clients[id.0].root
    }
    /// Returns the visible accessibility tree.
    #[must_use]
    pub fn tree(&self, id: ClientId) -> &Tree {
        A::tree(&self.clients[id.0].view)
    }
    /// Returns the records that survived successful store writes.
    #[must_use]
    pub fn kept(&self, id: ClientId) -> &[A::Record] {
        &self.clients[id.0].kept
    }
    /// Returns the loop’s monotonic time.
    #[must_use]
    pub fn now(&self) -> Time {
        self.now
    }
    fn history(&mut self, id: ClientId, address: A::Address, replace: bool) {
        let c = &mut self.clients[id.0];
        if replace {
            c.history[c.at] = address;
        } else if c.history[c.at] != address {
            c.history.truncate(c.at + 1);
            c.history.push(address);
            c.at += 1;
        }
    }
    fn advance_navigation(&mut self, id: ClientId) -> u64 {
        let c = &mut self.clients[id.0];
        c.navigation = c.navigation.checked_add(1).expect("world navigation generation fits");
        c.navigation
    }
    fn platform_show(&mut self, id: ClientId, address: A::Address, replace: bool) {
        self.observe(Observation::LeftPage { client: id });
        self.clients[id.0].acted_card = None;
        let changed = self.clients[id.0].history[self.clients[id.0].at] != address;
        self.history(id, address.clone(), replace);
        // A Went may ask to replace the current history entry; that acknowledgement
        // does not change the platform address and must not trigger another Went.
        if changed {
            // A later platform move supersedes this acknowledgement even within
            // one reload epoch, including leaving and returning to this address.
            let navigation = self.advance_navigation(id);
            self.schedule.send(
                self.now.saturating_add(self.settings.latency.draw(&mut self.rng)),
                Due {
                    client: id,
                    epoch: self.clients[id.0].epoch,
                    event: A::went(address),
                    navigation: Some(navigation),
                },
            );
        }
    }
    fn observe(&mut self, observation: Observation) {
        self.referee.observe(self.now, observation, &mut Vec::new());
        self.referee.assert_holding(self.settings.seed);
    }
    fn callback(&mut self, id: ClientId, event: Option<A::Event>) {
        if let Some(event) = &event {
            if A::window_changed(event) {
                self.observe(Observation::WindowChanged { client: id });
            }
            for receipt in A::received(event).into_iter().chain(self.front.receipts(id, event)) {
                self.observe(Observation::Delivered { client: id, receipt });
            }
        }
        let c = &mut self.clients[id.0];
        let env = Env {
            now: self.now,
            wall: Wall::from_nanos(1_000_000_000_000 + self.now.as_nanos()),
            limits: c.limits.clone(),
        };
        match event {
            Some(event) => A::step(&mut c.root, &env, event, &mut c.out),
            None => A::fire(&mut c.root, &env, &mut c.out),
        }
        A::take(&mut c.root, &env, &mut c.out);
        let mut wires = Vec::new();
        let mut shows = Vec::new();
        let mut signs = 0;
        let mut sign_outs = 0;
        let mut failed = self.quota_failure;
        self.quota_failure = false;
        while let Some(request) = c.out.pop() {
            match A::duty(request) {
                Duty::Store(write) => c.deferred.push(write),
                Duty::Wire(request) => wires.push(request),
                Duty::Show { address, replace } => shows.push((address, replace)),
                Duty::SignIn { provider: _ } => signs += 1,
                Duty::SignOut => sign_outs += 1,
                Duty::Fact => {}
            }
        }
        let writes = std::mem::take(&mut c.deferred);
        for write in writes {
            if failed {
                c.deferred.push(write);
                continue;
            }
            match write {
                StoreWrite::Save(record) => {
                    let key = A::record_key(&record);
                    if let Some(old) = c.kept.iter_mut().find(|old| A::record_key(old) == key) {
                        *old = record;
                    } else if c.kept.len() < self.settings.store_records {
                        c.kept.push(record);
                    } else {
                        failed = true;
                        c.deferred.push(StoreWrite::Save(record));
                    }
                }
                StoreWrite::Erase(key) => c.kept.retain(|record| A::record_key(record) != key),
            }
        }
        self.front.stored(id, &c.kept);
        for request in wires {
            if failed {
                if let Some(event) = A::unsent(request) {
                    self.schedule.send(
                        self.now.saturating_add(Duration::from_nanos(1)),
                        Due { client: id, epoch: self.clients[id.0].epoch, event, navigation: None },
                    );
                }
            } else {
                self.decision(id, &request, true);
                if let Some(mut sent) = A::sent(&request).or_else(|| self.front.sent(&request)) {
                    if self.clients[id.0].acted_card.is_some() {
                        sent.card.clone_from(&self.clients[id.0].acted_card);
                    }
                    self.observe(Observation::Request { client: id, party: self.clients[id.0].party, sent });
                }
                self.front.wire(id, self.now, env.wall, request);
            }
        }
        for (address, replace) in shows {
            self.platform_show(id, address, replace);
        }
        for _ in 0..sign_outs {
            self.observe(Observation::SignOutAsked { client: id });
            self.front.sign_out(id, self.now, env.wall);
        }
        for _ in 0..signs {
            self.front.sign_in(id, self.now, env.wall);
        }
        self.build(id);
    }
    fn build(&mut self, id: ClientId) {
        let c = &mut self.clients[id.0];
        A::build(&mut c.view, &c.root, &mut c.patches);
        let changed = !c.patches.is_empty();
        let seen = if changed {
            // The model is still the old platform tree after the view has built.
            let old_nodes: Vec<_> = c.model.ids().collect();
            while let Some(patch) = c.patches.pop() {
                c.model.apply(patch);
            }
            c.model.check(A::tree(&c.view));
            let mut current: Vec<_> = A::tree(&c.view).nodes().iter().map(|node| node.id).collect();
            current.sort_unstable();
            if let Some(node) = old_nodes.into_iter().find(|node| current.binary_search(node).is_err()) {
                self.last_removed = Some((id, node));
            }
            let mut seen = A::seen(A::tree(&c.view));
            if let Some(fault) = skein_ui::outline::check(A::tree(&c.view)) {
                seen.push(super::Seen::Outline { fault });
            }
            Some(seen)
        } else {
            None
        };
        self.callbacks += 1;
        self.trace.log(self.now, format!("client {} epoch {} tree {}", id.0, c.epoch, A::tree(&c.view).len()));
        if let Some(seen) = seen {
            self.observe(Observation::Shown { client: id, seen });
        }
    }

    fn check_heap(&mut self) {
        if let Some(meter) = &self.meter {
            let measured = meter.end();
            let bound = self.heap_bound();
            // Measure owned bytes here; the observation-based referee owns the
            // bounded rule and its failure, including its negative proofs.
            self.heap_peak = self.heap_peak.max(meter.check(measured, u64::MAX, &"client world iteration"));
            self.observe(Observation::Bounded { heap: self.heap_peak, bound });
        }
    }
    fn heap_bound(&self) -> u64 {
        self.clients
            .iter()
            .try_fold(0_u64, |sum, client| sum.checked_add(client.heap_bound))
            .expect("world heap bound fits")
    }
    fn text_act(&mut self, id: ClientId, node: NodeId, act: Act, step: u32) {
        let client = &mut self.clients[id.0];
        let Some(TextShell { print, held }) = &mut client.text_shell else {
            self.act(id, node, act);
            return;
        };
        let tree = A::tree(&client.view);
        let pending = held.take();
        let paired = pending.is_some();
        let first = if let Some((field, words)) = pending {
            assert_eq!(
                (&act, node),
                (&Act::Submit, field),
                "step {step}: typed without sending: not played at the text shell"
            );
            Act::Input { value: words }
        } else {
            skein_ui_text::render(tree, b"jig", Some(jig_client_view::regions::JigRegion::Link.region()), print);
            if tree.find(node).is_some_and(|n| n.states.as_slice().contains(&skein_ui::State::SubmitsOnEnter))
                && let Act::Input { value } = act
            {
                *held = Some((node, value));
                return;
            }
            act
        };
        let number = print
            .numbers
            .as_slice()
            .iter()
            .position(|id| *id == node)
            .unwrap_or_else(|| panic!("step {step}: acted node {node:?} has no printed number"))
            + 1;
        let mut line = number.to_string().into_bytes();
        if let Act::Input { value } = &first {
            line.push(b' ');
            line.extend_from_slice(value);
        }
        let read = skein_ui_text::command(&line, Some(print.numbers.as_slice()), tree, u32::MAX);
        assert_eq!(&read, &skein_ui_text::Command::Act { node, first, submit: paired }, "step {step}: text shell act");
        match read {
            skein_ui_text::Command::Act { node, first, submit } => {
                self.act(id, node, first);
                if submit {
                    self.act(id, node, Act::Submit);
                }
            }
            skein_ui_text::Command::Print
            | skein_ui_text::Command::Close
            | skein_ui_text::Command::Nothing
            | skein_ui_text::Command::Refused { .. } => unreachable!("the reader gave the asserted act"),
        }
    }
    fn walk_people(&mut self) -> bool {
        let mut progress = false;
        for index in 0..self.people.len() {
            if self.people[index].done
                || (matches!(self.people[index].person, PersonKind::Random { .. })
                    && self.people[index].wake.is_some_and(|at| at > self.now))
            {
                continue;
            }
            let id = self.people[index].client;
            let next = match &mut self.people[index].person {
                PersonKind::Script(person) => {
                    let before = person.at();
                    let next = person.next(&mut TreeFace::new(A::tree(&self.clients[id.0].view)), self.now);
                    progress |= before != person.at();
                    next
                }
                PersonKind::Random { person, left, except } => {
                    if *left == 0 {
                        Next::Done
                    } else {
                        *left -= 1;
                        progress = true;
                        match person.next(A::tree(&self.clients[id.0].view)) {
                            Some(RandomDoing::Act { node, act: Act::Press })
                                if except.is_some()
                                    && A::tree(&self.clients[id.0].view).find(node).is_some_and(|node| {
                                        node.role == skein_ui::Role::Button && node.name.as_deref() == *except
                                    }) =>
                            {
                                Next::Wait { until: None }
                            }
                            Some(RandomDoing::Act { node, act }) => Next::Do(Doing::Act { node, act }),
                            Some(RandomDoing::Reload) => Next::Do(Doing::Reload),
                            Some(RandomDoing::Back) => {
                                self.back(id);
                                Next::Wait { until: None }
                            }
                            None => Next::Wait { until: None },
                        }
                    }
                }
            };
            match next {
                Next::Do(Doing::Act { node, act }) => {
                    let step = match &self.people[index].person {
                        PersonKind::Script(p) => p.at(),
                        PersonKind::Random { .. } => 0,
                    };
                    self.text_act(id, node, act, step);
                }
                Next::Do(Doing::Go { address }) => self.go(id, address),
                Next::Do(Doing::Reload) => self.reload(id),
                Next::Wait { until } => self.people[index].wake = until,
                Next::Done => {
                    assert!(
                        self.clients[id.0].text_shell.as_ref().is_none_or(|text| text.held.is_none()),
                        "typed without sending: not played at the text shell"
                    );
                    self.people[index].done = true;
                }
                Next::Failed { step, why } => panic!(
                    "seed {} client {} step {step}: {why}; tree {:?}; all trees {:?}",
                    self.settings.seed,
                    id.0,
                    A::tree(&self.clients[id.0].view)
                        .nodes()
                        .iter()
                        .filter_map(|n| n.name.as_ref().map(|s| String::from_utf8_lossy(s)))
                        .collect::<Vec<_>>(),
                    self.clients
                        .iter()
                        .map(|client| A::tree(&client.view)
                            .nodes()
                            .iter()
                            .filter_map(|n| n.name.as_ref().map(|s| String::from_utf8_lossy(s)))
                            .collect::<Vec<_>>())
                        .collect::<Vec<_>>(),
                ),
            }
            if matches!(self.people[index].person, PersonKind::Random { .. }) && !self.people[index].done {
                self.people[index].wake = Some(self.now.saturating_add(self.settings.latency.draw(&mut self.rng)));
            }
        }
        progress
    }
    /// Runs callbacks, people and deadlines until the requested boundary.
    fn next_time(&self) -> Option<Time> {
        [
            self.schedule.next_time(),
            self.faults.next_time(),
            self.front.next_due(),
            self.referee.next_deadline(),
            self.clients.iter().filter_map(|c| A::next_deadline(&c.root)).min(),
            self.people.iter().filter(|p| !p.done).filter_map(|p| p.wake).min(),
        ]
        .into_iter()
        .flatten()
        .min()
    }
    fn deliver(&mut self, due: Due<A>) {
        let c = &self.clients[due.client.0];
        if due.epoch == c.epoch
            && due.navigation.is_none_or(|generation| generation == c.navigation)
            && self.front.delivers(due.client, &due.event)
        {
            if due.navigation.is_some() {
                self.observe(Observation::LeftPage { client: due.client });
                self.clients[due.client.0].acted_card = None;
            }
            self.front.received(due.client, &due.event);
            self.callback(due.client, Some(due.event));
        } else {
            self.dropped += 1;
        }
    }
    pub fn run(&mut self, until: Until) -> Outcome {
        let mut iterations = 0;
        for _ in 0..self.settings.iterations {
            if let Some(meter) = &self.meter {
                meter.start();
            }
            iterations += 1;
            while let Some(fault) = self.faults.next(self.now) {
                self.fault(fault);
            }
            self.draw_fault();
            let wall = Wall::from_nanos(1_000_000_000_000 + self.now.as_nanos());
            for (client, event) in self.front.poll(self.now, wall) {
                let at = self
                    .now
                    .saturating_add(self.settings.latency.draw(&mut self.rng))
                    .max(self.clients[client.0].delivery.saturating_add(Duration::from_nanos(1)));
                self.clients[client.0].delivery = at;
                self.schedule.send(at, Due { client, epoch: self.clients[client.0].epoch, event, navigation: None });
            }
            for observed in self.front.observed() {
                self.observe(Observation::Truth(observed));
            }
            self.referee.fire(self.now, &mut Vec::new());
            self.referee.assert_holding(self.settings.seed);
            while let Some(due) = self.schedule.next(self.now) {
                self.deliver(due);
            }
            for index in 0..self.clients.len() {
                if A::next_deadline(&self.clients[index].root).is_some_and(|at| at <= self.now) {
                    self.callback(ClientId(index), None);
                }
            }
            let progress = self.walk_people();
            let done = self.people.iter().all(|person| person.done);
            let stop = match until {
                Until::Done => {
                    done && self.schedule.is_empty()
                        && self.front.next_due().is_none_or(|at| at > self.now)
                        && self.faults.is_empty()
                        && self.referee.verdict() == Verdict::Passed
                }
                Until::Time(at) => self.now >= at,
                Until::Steps(count) => iterations >= count,
            };
            self.check_heap();
            if stop {
                break;
            }
            if progress {
                self.now = self.now.saturating_add(Duration::from_nanos(1));
                continue;
            }
            let next = self.next_time();
            self.now = next.map_or(self.now.saturating_add(Duration::from_millis(1)), |at| {
                at.max(self.now.saturating_add(Duration::from_nanos(1)))
            });
        }
        assert!(
            iterations < self.settings.iterations,
            "seed {} exceeded iteration bound at {:?}, walkers {:?}, tree {:?}",
            self.settings.seed,
            self.now,
            self.people
                .iter()
                .map(|p| match &p.person {
                    PersonKind::Script(p) => p.at(),
                    PersonKind::Random { left, .. } => *left,
                })
                .collect::<Vec<_>>(),
            self.clients
                .iter()
                .map(|c| A::tree(&c.view)
                    .nodes()
                    .iter()
                    .filter_map(|n| n.name.as_ref().map(|s| String::from_utf8_lossy(s)))
                    .collect::<Vec<_>>())
                .collect::<Vec<_>>()
        );
        for client in &self.clients {
            client.model.check(A::tree(&client.view));
        }
        Outcome {
            verdict: self.referee.verdict(),
            judged: self.referee.judged(),
            now: self.now,
            iterations,
            callbacks: self.callbacks,
            faults: self.faults_drawn,
            dropped: self.dropped,
            trace: self.trace.lines().to_vec(),
            heap_peak: self.heap_peak,
            heap_bound: self.heap_bound(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Until;
    use skein_fake_person::{Find, Step};
    use skein_lib::Duration;
    use skein_ui::Role;
    use skein_world::domain::Verdict;

    #[test]
    fn a_public_family_link_delivers_the_platform_went_callback() {
        let mut world = crate::stories::incident_world(5090);
        let client = world.client(7, crate::stories::inbox());
        let within = Duration::from_secs(1);
        world.person(
            client,
            vec![
                Step::See { find: Find::named(Role::Link, b"Family"), within },
                Step::Press { find: Find::named(Role::Link, b"Family") },
                Step::See { find: Find::text(b"Item 1"), within },
                Step::See { find: Find::text(b"Chat nine"), within },
            ],
        );
        let outcome = world.run(Until::Done);
        assert_eq!(outcome.verdict, Verdict::Passed);
        assert!(outcome.judged.0 > 0);
    }
}

#[cfg(test)]
#[path = "navigation_tests.rs"]
mod navigation_tests;
