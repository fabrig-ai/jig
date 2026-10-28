//! The real ops root with withheld store commit answers (dynamic-memory.md, 4.6).
use jig_domain_accounts as accounts;
use jig_domain_core as core;
use jig_domain_fleet as fleet;
use jig_domain_host as host;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_ops_domain as root;
use jig_ops_domain_infrastructure as infrastructure;
use skein_lib::{Duration, Env, Queue, ReplyTo, Time, Token, Wall};
use std::collections::VecDeque;

// More than the finite cold-start and turn continuation chains need.
const STEPS: usize = 256;
// One reserved output batch and the optional commit it accompanies.
const OUTPUTS: u32 = 256;
// An isolated reliance row, reused by the pressure setup.
const PRESSURE_TASK: u64 = 900;

pub struct Driver {
    pub domain: root::Domain,
    pub env: Env<root::Limits>,
    /// Inputs not yet offered; a handed-back input waits at the head.
    pub events: VecDeque<root::Event>,
    pub returned: u64,
    pub released: Vec<root::Output>,
    pub pending: VecDeque<u64>,
    /// The store's transactions, in submission order, with answers withheld.
    pub transactions: Vec<(u64, Vec<root::Write>)>,
    /// Worker notices at release, before their child continuations are offered.
    pub notices: Vec<host::LinkDown>,
    pub sign_in: u64,
    pub owner: Token,
}

impl Driver {
    #[must_use]
    pub fn started(seed: u64) -> Self {
        let (configuration, limits) = crate::conformance::config::root(seed);
        let rows = jig_core_world::fixtures::account_rows(&configuration.core.settings.llm_providers);
        let seeded = jig_core_world::fixtures::seeded_project_rows(&configuration.core);
        let mut world = Self {
            domain: root::Domain::new(configuration, &limits).expect("fixture core configuration"),
            env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits },
            events: VecDeque::new(),
            returned: 0,
            released: Vec::new(),
            pending: VecDeque::new(),
            transactions: Vec::new(),
            notices: Vec::new(),
            sign_in: 0,
            owner: Token::new(0),
        };
        for row in seeded {
            world.send(root::Event::Store(root::Store::Restore(root::Record::Core(row))));
            world.settle();
        }
        for row in rows {
            let jig_test_domain::Record::Core(core::Record::Accounts(record)) = row else {
                panic!("account fixture row")
            };
            world.send(root::Event::Core(core::Event::Account(accounts::Event::Restore { record })));
        }
        for event in [
            core::Event::People(people::Event::Roles { project: 1, holdings: Box::new([]) }),
            core::Event::People(people::Event::Restored),
            core::Event::Tasks(tasks::Event::Check),
            core::Event::Tasks(tasks::Event::Restored),
            core::Event::Period { project: 1, period: 1, budget: 1000, next: None },
            core::Event::Fleet(fleet::Event::Loaded),
            core::Event::Account(accounts::Event::Want),
            core::Event::Account(accounts::Event::Component(accounts::FromComponent::Granted {
                form: accounts::Form::Credential(accounts::Scheme::Bearer),
                account: 1,
                generation: 1,
                valid: Duration::from_secs(60),
            })),
        ] {
            world.send(root::Event::Core(event));
            world.settle();
        }
        world.send(root::Event::Core(core::Event::SignIn {
            held: None,
            reply_to: ReplyTo::new(Token::new(101)),
            identity: people::Identity {
                key: people::IdentityKey { provider: 0, subject: 7_u64.to_be_bytes().into() },
                login: b"person".as_slice().into(),
                name: b"Person".as_slice().into(),
            },
        }));
        world.settle();
        world.sign_in = world
            .released
            .iter()
            .find_map(|output| {
                if let root::Output::Core(core::Held::PeopleReply {
                    reply: people::Reply::SignedIn { sign_in, .. },
                    ..
                }) = output
                {
                    Some(*sign_in)
                } else {
                    None
                }
            })
            .expect("signed in");
        world.released.clear();
        world.transactions.clear();
        world
    }

    /// Start a chat with the same account and project as the retained-turn world.
    pub fn chat(&mut self) {
        self.send(root::Event::Core(core::Event::People(people::Event::Ask {
            models: Box::new([]),
            reply_to: ReplyTo::new(Token::new(102)),
            sign_in: self.sign_in,
            key: [2; 16],
            ask: crate::conformance::config::chat(b"order story"),
        })));
        self.settle();
        self.owner = self
            .released
            .iter()
            .find_map(|output| {
                if let root::Output::Agent(host::ToAgent::Start { owner, .. }) = output { Some(*owner) } else { None }
            })
            .expect("started chat");
        self.send(root::Event::Agent(host::FromAgent::Started { owner: self.owner, agent: self.owner }));
        self.settle();
        self.released.clear();
        self.transactions.clear();
        self.notices.clear();
    }

    /// Offer once; a refused input is counted and dropped.
    pub fn send(&mut self, event: root::Event) {
        if self.offer(event).is_err() {
            self.returned += 1;
        }
        self.collect();
    }

    /// A refused input waits at the head for the next offer.
    pub fn send_retaining(&mut self, event: root::Event) {
        if let Err(event) = self.offer(event) {
            self.returned += 1;
            self.events.push_front(event);
        }
        self.collect();
    }

    #[expect(clippy::result_large_err, reason = "the service admission boundary returns the original event by value")]
    fn offer(&mut self, event: root::Event) -> Result<(), root::Event> {
        root::try_step(&mut self.domain, &self.env, event)
    }

    /// Release once; store submissions wait, child callbacks join the input tail.
    pub fn collect(&mut self) {
        let mut out = Queue::with_capacity(OUTPUTS);
        root::release(&mut self.domain, &self.env, &mut out);
        while let Some(output) = out.pop() {
            match output {
                root::Output::Commit { number, mut writes } => {
                    let mut transaction = Vec::new();
                    while let Some(write) = writes.pop() {
                        transaction.push(write);
                    }
                    self.transactions.push((number, transaction));
                    self.pending.push_back(number);
                }
                root::Output::ToChild(child) => {
                    match &child {
                        root::Released::Host(host::Event::Down(host::LinkDown::TurnKept { run, attempt, turn })) => {
                            self.notices.push(host::LinkDown::TurnKept { run: *run, attempt: *attempt, turn: *turn });
                        }
                        root::Released::Host(host::Event::Down(host::LinkDown::Acknowledge { run, attempt })) => {
                            self.notices.push(host::LinkDown::Acknowledge { run: *run, attempt: *attempt });
                        }
                        root::Released::Core(_)
                        | root::Released::Fleet(_)
                        | root::Released::Infrastructure(_)
                        | root::Released::Observability(_)
                        | root::Released::Host(_)
                        | root::Released::Procedure { .. }
                        | root::Released::Restart(_)
                        | root::Released::RestartDone(_) => {}
                    }
                    self.events.push_back(root::Event::Released(child));
                }
                root::Output::Core(core::Held::NotesLoad { owner, .. }) => {
                    self.events.push_back(root::Event::Core(core::Event::Notes(jig_domain_notes::Event::Loaded {
                        owner,
                        rows: jig_domain_notes::Rows {
                            records: skein_lib::List::with_capacity(self.env.limits.core.notes.load_rows),
                        },
                        more: false,
                    })));
                }
                other @ (root::Output::ApplicationDocument { .. }
                | root::Output::ApplicationRead { .. }
                | root::Output::IncidentAssociationRefused { .. }
                | root::Output::Application { .. }
                | root::Output::Switch
                | root::Output::Load { .. }
                | root::Output::CoreLoad(_)
                | root::Output::Core(_)
                | root::Output::Now(_)
                | root::Output::Observability(_)
                | root::Output::Infrastructure(_)
                | root::Output::ReadResult { .. }
                | root::Output::Read { .. }
                | root::Output::Agent(_)
                | root::Output::ProjectMade { .. }
                | root::Output::Stop) => self.released.push(other),
            }
        }
        self.domain.reclaim();
    }

    pub fn commit_one(&mut self) {
        let number = self.pending.pop_front().expect("oldest withheld commit");
        self.send(root::Event::Store(root::Store::Committed { number, fill: 0 }));
    }

    pub fn settle(&mut self) {
        for _ in 0..STEPS {
            if let Some(event) = self.events.pop_front() {
                self.send_retaining(event);
            }
            if !self.pending.is_empty() {
                self.commit_one();
            }
            self.collect();
            if self.events.is_empty() && self.pending.is_empty() && self.domain.quiescent() {
                return;
            }
        }
        panic!("direct root did not settle: {:?}", self.domain);
    }

    pub fn fill_to_watermark(&mut self) {
        let returned = self.returned;
        for _ in 0..=self.env.limits.watermark {
            self.send(root::Event::Infrastructure(infrastructure::Event::Names {
                project: 1,
                task: PRESSURE_TASK,
                resources: Box::new([infrastructure::Resource::Service(crate::infra_service())]),
            }));
            if self.returned > returned {
                return;
            }
        }
        panic!("admission binds within watermark + 1 writing probes");
    }
}
