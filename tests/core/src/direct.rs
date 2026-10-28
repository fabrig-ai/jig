//! The reference root's real admission entrance with controlled store answers.

use crate::{Store, effects, fixtures, world};
use jig_domain_accounts as accounts;
use jig_domain_authority as authority;
use jig_domain_core as core;
use jig_domain_fleet as fleet;
use jig_domain_people as people;
use jig_test_domain as root;
use jig_test_store::{StoreOut, Up};
use skein_lib::{Duration, Env, Map, Queue, ReplyTo, Token};
use std::collections::VecDeque;

const WORKER: Token = Token::new(7);
const SIGN_IN: Token = Token::new(101);
const CHAT: Token = Token::new(102);
// The walking world's settlement budget, independent of journal capacity.
const STEPS: usize = 300;

/// The reference root driven through `try_step`, with commit answers withheld.
pub struct Driver {
    pub domain: root::Domain,
    pub env: Env<root::Limits>,
    /// Inputs not yet offered; a handed-back input waits at the head.
    pub events: VecDeque<root::Event>,
    pub returned: u64,
    /// Every request the root released, in order, except store requests served here.
    pub released: Vec<root::Request>,
    /// Commits sent by the root and not yet answered, oldest first.
    pub pending: VecDeque<u64>,
    pub transactions: Vec<Vec<root::Write>>,
    pub stopped: bool,
    store: Store,
    serial: u64,
}

impl Driver {
    /// A signed-in party's chat, assigned to the walking world's worker.
    #[must_use]
    pub fn chat(seed: u64) -> Self {
        Self::configured_chat(world::config(seed), &world::limits())
    }

    /// A chat fixture with room and a configured policy for two more projects.
    #[must_use]
    pub fn project_maker(seed: u64) -> Self {
        // The seed plus two makes; the people's rosters also include deployment roles.
        const PROJECTS: u32 = 3;
        let mut limits = world::limits();
        limits.core.authority.projects = PROJECTS;
        limits.core.people.projects = PROJECTS + 1;
        let mut configuration = world::config(seed);
        configuration.core.defaults.rules =
            authority::Domain::new(configuration.core.defaults.rules.clone(), limits.core.authority)
                .expect("project room")
                .rules()
                .clone();
        let seed = &configuration.core.seed_projects[0].policy;
        configuration.core.defaults.policies = Map::with_capacity(1);
        configuration
            .core
            .defaults
            .policies
            .insert(
                1,
                core::ConfiguredPolicy {
                    policy: seed.policy.clone(),
                    permissions: Box::new([]),
                    templates: seed.templates.clone(),
                },
            )
            .expect("one configured policy");
        Self::configured_chat(configuration, &limits)
    }

    fn configured_chat(configuration: root::Config, limits: &root::Limits) -> Self {
        let creating = fixtures::Creating::new(&configuration.core, limits.core.tasks.tree_tasks);
        let bootstrap = effects::World::first_start_with_accounts(configuration, limits);
        let (now, wall) = bootstrap.clocks();
        let mut driver = Self {
            domain: bootstrap.domain,
            env: Env { now, wall, limits: *limits },
            events: VecDeque::new(),
            returned: 0,
            released: Vec::new(),
            pending: VecDeque::new(),
            transactions: Vec::new(),
            stopped: false,
            store: bootstrap.store,
            serial: 1000,
        };
        driver.send(root::Event::Core(core::Event::Account(accounts::Event::Want)));
        driver.settle();
        driver.send(root::Event::Core(core::Event::Account(accounts::Event::Component(
            accounts::FromComponent::Granted {
                form: accounts::Form::Credential(accounts::Scheme::Bearer),
                account: 1,
                generation: 1,
                valid: Duration::from_secs(60),
            },
        ))));
        driver.settle();
        driver.send(root::Event::Core(core::Event::Fleet(fleet::Event::Hello {
            worker: 0,
            channel: WORKER,
            hello: fleet::Hello {
                bounds: fixtures::bounds(),
                stop_bound: Duration::from_millis(100),
                slots: 1,
                workstreams: Box::new([]),
                hosting: Box::new([]),
            },
        })));
        driver.settle();
        driver.send(root::Event::Core(core::Event::Period { project: 1, period: 1, budget: 1000, next: None }));
        driver.settle();
        driver.send(sign_in(SIGN_IN));
        driver.settle();
        let session = driver.session();
        driver.send(root::Event::Core(core::Event::People(people::Event::Ask {
            models: Box::new([]),
            reply_to: ReplyTo::new(CHAT),
            sign_in: session,
            key: [5; 16],
            ask: fixtures::create_chat(&creating, 1, 0, b"hello".as_slice().into()),
        })));
        driver.settle();
        driver
    }

    /// Offer one input; count and drop any unadmitted input. Release is explicit.
    pub fn send(&mut self, event: root::Event) {
        if root::try_step(&mut self.domain, &self.env, event).is_err() {
            self.returned += 1;
        }
    }

    /// Offer one input, retaining a refusal ahead of later queued inputs.
    pub fn send_retaining(&mut self, event: root::Event) {
        if let Err(event) = root::try_step(&mut self.domain, &self.env, event) {
            self.returned += 1;
            self.events.push_front(event);
        }
    }

    /// Run one release batch. The root itself routes its durable callbacks.
    pub fn collect(&mut self) {
        let mut out = Queue::uncapped();
        root::release(&mut self.domain, &self.env, &mut out);
        while let Some(request) = out.pop() {
            match request {
                root::Request::Commit { number, mut writes } => {
                    let mut rows = Vec::new();
                    while let Some(write) = writes.pop() {
                        rows.push(write);
                    }
                    self.transactions.push(rows.clone());
                    self.store.submit(number, rows.into_boxed_slice(), 0);
                    self.pending.push_back(number);
                }
                root::Request::Deliver(root::Delivery::Core(core::Held::ReadLoad { load, range })) => {
                    self.store.ask(StoreOut::ReadLoad { load, range });
                }
                root::Request::Deliver(root::Delivery::Core(core::Held::Load(load))) => {
                    self.store.ask(StoreOut::Now(*load));
                }
                root::Request::Deliver(root::Delivery::Core(core::Held::NotesLoad { owner, range })) => {
                    self.store.ask(StoreOut::NotesLoad { owner, range });
                }
                root::Request::Deliver(root::Delivery::Core(core::Held::Sources { channel, sources })) => {
                    self.events.push_back(root::Event::Core(core::Event::Fleet(fleet::Event::Logins {
                        channel,
                        logins: sources
                            .iter()
                            .map(|source| fleet::Login { name: *source, state: fleet::LoginState::Usable })
                            .collect(),
                    })));
                }
                root::Request::Now(core::Now::StartPreparation { task, from: None, .. }) => {
                    self.events.push_back(root::Event::TranscriptLoaded { task, rows: Box::new([]), done: true });
                }
                root::Request::Now(now @ core::Now::StartPreparation { from: Some(_), .. }) => {
                    self.store.ask(StoreOut::Now(now));
                }
                root::Request::Stop => {
                    self.stopped = true;
                    self.released.push(root::Request::Stop);
                }
                other @ (root::Request::ReadResult { .. }
                | root::Request::Busy { .. }
                | root::Request::Restart(_)
                | root::Request::Deliver(_)
                | root::Request::Now(_)) => self.released.push(other),
            }
        }
        for answer in self.store.answers() {
            match answer {
                Up::Root(event) => self.events.push_back(event),
                Up::Restore(_) | Up::Ask(_) => panic!("settled chat does not restart"),
            }
        }
        self.domain.reclaim();
    }

    /// Apply and answer the oldest commit, leaving its release to the caller.
    pub fn commit_one(&mut self) {
        let expected = self.pending.pop_front().expect("pending commit");
        let number = self.store.tick(false).expect("store accepts commit").expect("ready commit");
        assert_eq!(number, expected, "the store applies commits oldest first");
        self.send_retaining(root::Event::Committed { number, fill: 0 });
    }

    /// Offer queued inputs, answer commits and release until idle, within a budget.
    pub fn settle(&mut self) {
        for _ in 0..STEPS {
            if let Some(event) = self.events.pop_front() {
                self.send_retaining(event);
            }
            if !self.pending.is_empty() {
                self.commit_one();
            }
            self.collect();
            assert!(!self.stopped, "root stopped while settling");
            if self.domain.quiescent() && self.events.is_empty() && self.pending.is_empty() {
                return;
            }
        }
        panic!("direct root did not settle: {:?}", self.domain);
    }

    /// Write sign-ins without answering commits until admission returns a probe.
    /// The refused setup probe is dropped and does not join the story's inputs.
    pub fn fill_to_watermark(&mut self) {
        let returned = self.returned;
        for _ in 0..=self.env.limits.journal.commits {
            self.probe();
            if self.returned > returned {
                return;
            }
            self.collect();
        }
        panic!("admission returns a pressure probe within the commit allowance");
    }

    pub fn probe(&mut self) {
        self.serial += 1;
        self.send(sign_in(Token::new(self.serial)));
    }

    #[must_use]
    pub fn session(&self) -> u64 {
        self.released
            .iter()
            .find_map(|request| {
                if let root::Request::Deliver(root::Delivery::Core(core::Held::PeopleReply {
                    to,
                    reply: people::Reply::SignedIn { sign_in, .. },
                })) = request
                    && *to == ReplyTo::new(SIGN_IN)
                {
                    Some(*sign_in)
                } else {
                    None
                }
            })
            .expect("durable sign-in")
    }

    #[must_use]
    pub fn assignment(&self) -> &root::Assignment {
        self.released
            .iter()
            .find_map(|request| {
                if let root::Request::Deliver(root::Delivery::Assigned { assignment, .. }) = request {
                    Some(assignment)
                } else {
                    None
                }
            })
            .unwrap_or_else(|| panic!("durable worker assignment: {:?}; {:?}", self.released, self.domain))
    }
}

fn sign_in(reply: Token) -> root::Event {
    root::Event::Core(core::Event::SignIn {
        held: None,
        reply_to: ReplyTo::new(reply),
        identity: people::Identity {
            key: people::IdentityKey { provider: 0, subject: 7_u64.to_be_bytes().into() },
            login: b"person".as_slice().into(),
            name: b"Person".as_slice().into(),
        },
    })
}
