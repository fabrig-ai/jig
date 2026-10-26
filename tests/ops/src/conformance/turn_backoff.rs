//! A small ops root world with one fleet admission and two retained hub turns.
use jig_domain_accounts as accounts;
use jig_domain_core as core;
use jig_domain_fleet as fleet;
use jig_domain_host as host;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_ops_domain as root;
use skein_lib::{Duration, Env, Queue, ReplyTo, Time, Token, Wall};

pub struct World {
    pub domain: root::Domain,
    pub env: Env<root::Limits>,
    owner: Token,
    pub saved: Vec<core::TurnRecord>,
    pub kept: Vec<u32>,
}

impl World {
    #[must_use]
    pub fn started() -> Self {
        let (configuration, mut limits) = super::config::root(187);
        limits.core.fleet.turns = 1;
        limits.host.turns = 2;
        let rows = jig_core_world::fixtures::account_rows(&configuration.core.settings.llm_providers);
        let seeded = jig_core_world::fixtures::seeded_project_rows(&configuration.core);
        let mut world = Self {
            domain: root::Domain::new(configuration, &limits).expect("fixture core configuration"),
            env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits },
            owner: Token::new(0),
            saved: Vec::new(),
            kept: Vec::new(),
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
        let sign_in = world
            .settle()
            .into_iter()
            .find_map(|output| {
                if let root::Output::Core(core::Held::PeopleReply {
                    reply: people::Reply::SignedIn { sign_in, .. },
                    ..
                }) = output
                {
                    Some(sign_in)
                } else {
                    None
                }
            })
            .expect("signed in");
        world.send(root::Event::Core(core::Event::People(people::Event::Ask {
            models: Box::new([]),
            reply_to: ReplyTo::new(Token::new(102)),
            sign_in,
            key: [2; 16],
            ask: super::config::chat(b"backoff story"),
        })));
        world.owner = world
            .settle()
            .into_iter()
            .find_map(|output| {
                if let root::Output::Agent(host::ToAgent::Start { owner, .. }) = output { Some(owner) } else { None }
            })
            .expect("started chat");
        world.send(root::Event::Agent(host::FromAgent::Started { owner: world.owner, agent: world.owner }));
        world.settle();
        world
    }

    pub fn send(&mut self, event: root::Event) {
        root::step(&mut self.domain, &self.env, event);
    }

    pub fn turn(&mut self, number: u32) {
        self.send(root::Event::Host(host::Event::Turn {
            owner: self.owner,
            turn: host::Turn {
                turn: number,
                spent: u64::from(number),
                read: None,
                opens_window: number == 1,
                refused: Box::new([]),
                body: Box::new([u8::try_from(number).expect("small world turn")]),
            },
        }));
    }

    pub fn take(&mut self) -> Vec<root::Output> {
        let mut queue = Queue::with_capacity(256);
        root::release(&mut self.domain, &self.env, &mut queue);
        let mut outputs = Vec::new();
        while let Some(output) = queue.pop() {
            outputs.push(output);
        }
        self.domain.reclaim();
        outputs
    }

    pub fn settle(&mut self) -> Vec<root::Output> {
        let mut seen = Vec::new();
        for _ in 0..64 {
            let outputs = self.take();
            if outputs.is_empty() && self.domain.quiescent() {
                return seen;
            }
            for output in outputs {
                match output {
                    root::Output::Commit { number, mut writes } => {
                        while let Some(write) = writes.pop() {
                            if let root::Write::Save(root::Record::Core(core::Record::Core(core::CoreRecord::Turn(
                                row,
                            )))) = write
                            {
                                self.saved.push(row);
                            }
                        }
                        self.send(root::Event::Store(root::Store::Committed { number, fill: 0 }));
                    }
                    root::Output::ToChild(child) => {
                        if let root::Released::Host(host::Event::Down(host::LinkDown::TurnKept { turn, .. })) = &child {
                            self.kept.push(*turn);
                        }
                        self.send(root::Event::Released(child));
                    }
                    root::Output::Core(core::Held::NotesLoad { owner, .. }) => {
                        self.send(root::Event::Core(core::Event::Notes(jig_domain_notes::Event::Loaded {
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
                    | root::Output::Busy
                    | root::Output::Stop) => seen.push(other),
                }
            }
        }
        panic!("root world did not settle");
    }

    pub fn fire_at(&mut self, at: Time) {
        self.env.now = at;
        root::fire(&mut self.domain, &self.env, root::Timer::Turn);
        self.settle();
    }
}
