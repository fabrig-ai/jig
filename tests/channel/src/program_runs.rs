//! Program runs cross the real worker channel, with a hosted Claude and permission relay.
use crate::{
    account_setup, fixtures,
    processes::{self, Proc},
    program_sources::Files,
    referee::Rules,
    stories::Story,
    world::{self, Store, Views},
};
use jig_domain_core as core;
use jig_protocol_worker_wire as wire;
use skein_fake_machine::{How, Item};
use skein_io::kernel;
use skein_lib::{Duration, Reader, Secret, Time};
use skein_world::{HostedProgram, Inherited, StartupRoot};
use std::{cell::RefCell, rc::Rc};

const SETTLE: Duration = Duration::from_secs(45);

fn worker_open(root: kernel::Fd) -> Proc {
    let mut worker = processes::worker_program_runs(root);
    let Proc::Worker(host) = &mut worker else { unreachable!("worker factory") };
    // Release this channel when the first inbox message is durable, before any placement.
    host.relay.until = Some(Time::ZERO.saturating_add(SETTLE));
    worker
}

fn say(key: [u8; 16], words: &[u8]) -> jig_fake_parties::Act {
    jig_fake_parties::Act::Request {
        key,
        ask: jig_fake_parties::Ask::Say { task: jig_fake_parties::Task::Last, words: words.into(), release: None },
    }
}

fn engine(inherited: &Inherited) -> Proc {
    engine_configured(inherited, false)
}
fn engine_open(inherited: &Inherited) -> Proc {
    engine_configured(inherited, true)
}
fn engine_configured(inherited: &Inherited, open: bool) -> Proc {
    let document = String::from_utf8(fixtures::engine_document())
        .expect("engine JSON")
        .replace(
            r#""sources":{"keys":true}"#,
            r#""manage_usage":"https://usage.example/","sources":{"programs":["claude_code"]}"#,
        )
        .replace(r#""dialect": 1,"#, r#""dialect": 1, "program": "claude_code","#)
        .replace(r#""window": 1048576"#, r#""window": 272000"#)
        .replace(r#""inspect": false"#, r#""inspect": true"#);
    if open {
        let mut document: serde_json::Value = serde_json::from_str(&document).expect("engine JSON");
        document["engine"]["templates"][0]["ended_by"] = "requester".into();
        // Only the listening interval changes; the run and reserve deadlines stay fixed.
        document["engine"]["charters"][0]["waiting_ms"] = 10.into();
        processes::engine_document(inherited, &serde_json::to_vec(&document).expect("Open engine JSON"))
    } else {
        processes::engine_document(inherited, document.as_bytes())
    }
}
fn declarations() -> smith_agent_service::profile::Configuration {
    let mut declarations = smith_agent_process_world::declarations(4096);
    declarations.endpoints[0].models[0].window = 272_000;
    declarations
}
pub(crate) fn limits() -> smith_agent_service::Limits {
    static LIMITS: std::sync::OnceLock<smith_agent_service::Limits> = std::sync::OnceLock::new();
    *LIMITS.get_or_init(|| {
        let mut limits = smith_agent_service::profile::derive(&smith_agent_process_world::profile(), &declarations())
            .expect("272k profile");
        limits.llm.accounts = 2;
        limits.domain.configured_model_bytes = 4096;
        // As smith's program process fixture does, bound the report-only run's receiving room.
        limits.domain.run.run_bytes = limits.domain.run.run_bytes.min(4096);
        limits.domain.run.outcome_bytes = limits.domain.run.outcome_bytes.min(4096);
        limits.domain.run.answer_bytes = limits.domain.run.answer_bytes.min(4096);
        limits.domain.run.check_tail = limits.domain.run.check_tail.min(256);
        limits.domain.run.host_reply_bytes = limits.domain.run.host_reply_bytes.min(4096);
        limits.domain.run.message_bytes = limits.domain.run.message_bytes.min(4096);
        limits.domain.session.delegated_result_bytes =
            smith_domain::feedback_worst_case(&limits.domain.run).expect("bounded feedback");
        limits
    })
}
pub(crate) fn configuration() -> smith_agent_service::Config {
    let mut service = fixtures::agent_configuration();
    service.limits = limits();
    let (destinations, _) = service.llm_endpoints.into_parts();
    let mut destination = destinations.into_boxed().into_vec().pop().expect("one destination");
    destination.limits = service.limits.llm.adapter.client;
    destination.credential.value = service.limits.llm.grant_value_bytes;
    destination.credential.account_id = service.limits.llm.grant_value_bytes;
    service.llm_endpoints = smith_protocol_llm::Endpoints::new(
        Box::new([smith_protocol_llm::ConfiguredEndpoint {
            name: smith_domain::llm::Endpoint(0),
            destination,
            connect: Duration::from_secs(1),
            handshake: Duration::from_secs(1),
            identity: smith_protocol_llm::IdentityProfile::Plain,
            models: Box::new([]),
        }]),
        1,
    )
    .expect("272k endpoint");
    service.domain.models[0].window = 272_000;
    service.domain.programs = Box::new([smith_domain::Program {
        endpoint: smith_domain::run::charter::Endpoint(0),
        name: b"claude_code".as_slice().into(),
    }]);
    service.program = Some(smith_agent_service::ProgramBackend {
        program: smith_protocol_program::Programs {
            claude: b"/private/programs/claude".as_slice().into(),
            directory: b"/private/programs".as_slice().into(),
        },
        endpoints: Box::new([0]),
        relay: b"/private/programs/relay".as_slice().into(),
        entropy: Secret::new(Box::new([7; 32])),
        effective_user: 1000,
    });
    service
}
#[expect(clippy::zero_sized_map_values, reason = "the public shell configuration has a unit selector map")]
fn agent(_spawn: &kernel::Spawn, inherited: &Inherited) -> Proc {
    let service = configuration();
    let pipe = |number| inherited.pipes.iter().find(|(child, _)| *child == number).expect("agent pipe").1;
    let (errors, capture) = smith_agent_process_world::AgentErrors::pipe(pipe(2));
    let agent = smith_agent_shell::Agent::new(
        smith_agent_shell::config::Configuration {
            memory: service.limits.memory,
            service,
            trace: None,
            profile: smith_agent_process_world::profile(),
            declarations: declarations(),
            programs: std::collections::BTreeMap::new(),
            endpoint_programs: std::collections::BTreeMap::new(),
        },
        smith_agent_shell::Resources {
            input: pipe(0),
            output: pipe(1),
            error: Some(pipe(2)),
            signals: inherited.signal,
            seed: fixtures::SEED,
            pid: 101,
            trace: None,
            roots: Some(
                inherited.roots.iter().filter(|(name, _)| name.as_ref() != b"programs").map(|(_, fd)| *fd).collect(),
            ),
            programs: Some(smith_agent_shell::ProgramResources {
                relay: "/private/programs/relay".into(),
                entropy: Secret::new(Box::new([7; 32])),
                directory: Some(
                    inherited.roots.iter().find(|(name, _)| name.as_ref() == b"programs").expect("program directory").1,
                ),
            }),
        },
        Box::new(errors.clone()),
    )
    .expect("program-capable agent");
    Proc::Agent { host: Box::new(capture.host(agent)), errors }
}
fn relay(spawn: &kernel::Spawn, inherited: &Inherited) -> Proc {
    let pipe = |number| inherited.pipes.iter().find(|(child, _)| *child == number).map(|(_, fd)| *fd);
    Proc::Auth(Box::new(smith_agent_shell::relay::Process::new(
        &spawn.args,
        &spawn.env,
        smith_agent_shell::relay::Faces {
            stdin: pipe(0).expect("relay input"),
            stdout: pipe(1).expect("relay output"),
            stderr: pipe(2),
            signal: inherited.signal,
        },
    )))
}
fn claude(spawn: &kernel::Spawn, inherited: &Inherited) -> Proc {
    assert!(spawn.env.iter().any(|entry| entry.as_ref() == b"CLAUDE_CONFIG_DIR=/private/programs/alice"));
    assert!(
        !spawn
            .env
            .iter()
            .any(|entry| entry.starts_with(b"ANTHROPIC_") || entry.starts_with(b"CLAUDE_CODE_OAUTH_TOKEN="))
    );
    Proc::Auth(Box::new((smith_fake_claude::hosted(b"/private/programs/claude").make)(spawn, inherited)))
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Case {
    Answer,
    Limited,
    Unauthorized,
    SignOut,
    Waiting,
    Open,
    Builtin,
}
#[derive(Default, Debug)]
pub struct Run {
    pub observations: Vec<smith_fake_claude::Observation>,
    pub seen: Vec<smith_fake_claude::Seen>,
    pub turns: Vec<core::TurnRecord>,
    pub terminals: Vec<core::TerminalRecord>,
    pub limited: Vec<wire::Limited>,
    pub unauthorized: Vec<wire::Unauthorized>,
    pub starts: u32,
    pub diagnostics: String,
    pub answers: Vec<wire::Answer>,
    pub grants: Vec<(u64, u32, u64, Vec<u8>)>,
    pub held: Vec<(u32, Option<jig_domain_accounts::Held>)>,
    pub authenticated_failures: Vec<u32>,
    pub signed_out: bool,
    pub received: Vec<serde_json::Value>,
    pub messages: Vec<(u64, u64, Vec<u8>)>,
    pub sent_after_park: Option<u64>,
    pub run_waits: Vec<Option<jig_client::contract::TaskRunWait>>,
    pub task: Option<Box<jig_domain_tasks::TaskRecord>>,
}
impl Run {
    fn worker(&mut self, wire: &processes::Wire) {
        self.starts = u32::try_from(wire.placements.len()).expect("attempts");
        self.grants.clear();
        self.messages.clear();
        for (_, kind, body) in &wire.frames {
            if *kind == wire::Kind::Assignment.number() {
                let assignment = wire::Assignment::decode(&wire::CEILINGS, &mut Reader::new(body)).expect("assignment");
                for message in assignment.messages() {
                    self.messages.push((assignment.attempt(), message.name(), message.words().to_vec()));
                }
                for grant in assignment.grants() {
                    if let wire::GrantValue::Program(value) = grant.value() {
                        self.grants.push((
                            assignment.attempt(),
                            grant.number(),
                            grant.generation(),
                            value.name().to_vec(),
                        ));
                    }
                }
            }
        }
        self.answers = wire
            .frames
            .iter()
            .filter(|(_, kind, _)| *kind == wire::Kind::Answer.number())
            .map(|(_, _, body)| wire::Answer::decode(&wire::CEILINGS, &mut Reader::new(body)).expect("answer"))
            .collect();
        self.limited = wire
            .frames
            .iter()
            .filter(|(_, kind, _)| *kind == wire::Kind::Limited.number())
            .map(|(_, _, body)| wire::Limited::decode(&wire::CEILINGS, &mut Reader::new(body)).expect("limited notice"))
            .collect();
        self.unauthorized = wire
            .frames
            .iter()
            .filter(|(_, kind, _)| *kind == wire::Kind::Unauthorized.number())
            .map(|(_, _, body)| {
                wire::Unauthorized::decode(&wire::CEILINGS, &mut Reader::new(body)).expect("unauthorized notice")
            })
            .collect();
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Action {
    Ready,
    Sent,
    Resumed,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Opening {
    Creating,
    Watching,
    Ready,
}
struct Judge {
    shared: Rc<RefCell<Run>>,
    store: Store,
    rules: Rules,
    views: Views,
    accounts: account_setup::Setup,
    party: jig_fake_parties::Party,
    period: bool,
    done: bool,
    case: Case,
    action: Action,
    opening: Opening,
}
impl skein_world::Referee<Proc> for Judge {
    fn act(&mut self, now: Time, procs: &mut [Proc]) {
        self.accounts.act(procs, Story::Chat, self.done);
        if !self.done {
            self.accounts.party(now, procs, &mut self.party, &mut self.views, Story::Chat);
        }
        for proc in procs.iter_mut() {
            if let Proc::Engine(host) = proc {
                world::answer_store(host, &mut self.store, &mut self.rules, false, Story::Chat, now);
                let engine = host.service.engine().expect("engine");
                world::answer_parties(engine, &mut self.party, &mut self.views);
                if !self.period && self.party.sign_in.is_some() {
                    engine.parties_in().push(core::Event::Period { project: 1, period: 1, budget: 100, next: None });
                    self.period = true;
                }
            }
        }
        let turns = world::turns(&self.store).into_iter().cloned().collect();
        let terminals: Vec<_> = self
            .store
            .rows
            .values()
            .filter_map(|row| {
                if let jig_test_domain::Record::Core(core::Record::Core(core::CoreRecord::Terminal(answer))) = row {
                    Some(answer.clone())
                } else {
                    None
                }
            })
            .collect();
        let parked = terminals.iter().any(|answer| answer.end == jig_domain_tasks::End::Parked);
        if self.case == Case::Open {
            self.prepare_open(procs);
        }
        self.steer(procs, parked);
        let signed_out = procs.iter().any(|process| {
            let Proc::Client(client) = process else { return false };
            client.received.iter().any(|received| {
                use jig_client::contract::{Answer, Outcome, OwnerOutcome};
                matches!(
                    received,
                    jig_client_protocol_world::native::Received::Answer {
                        answer: Answer::Done(Outcome::Owner(OwnerOutcome::AccountSignedOut {
                            confirmed: true, failed_hosts,
                        })), ..
                    } if failed_hosts.is_empty()
                )
            })
        });
        if self.case == Case::SignOut && signed_out && self.action == Action::Sent {
            self.party.extend(Box::new([say([4; 16], b"Continue after signing out")]));
            self.action = Action::Resumed;
        }
        self.done = match self.case {
            Case::Waiting => {
                terminals.iter().any(|answer| matches!(answer.end, jig_domain_tasks::End::Finished { .. }))
            }
            Case::SignOut => signed_out && !terminals.is_empty(),
            Case::Open => terminals.iter().filter(|answer| answer.end == jig_domain_tasks::End::Parked).count() == 2,
            Case::Answer | Case::Limited | Case::Unauthorized | Case::Builtin => !terminals.is_empty(),
        };
        self.record(procs, turns, terminals, signed_out);
    }
    fn observe(&mut self, _now: Time, _procs: &[Proc]) {}
    fn next_deadline(&self) -> Option<Time> {
        None
    }
    fn overdue(&self, now: Time) -> Option<String> {
        (now > Time::ZERO.saturating_add(SETTLE)).then(|| {
            format!(
                "program run settles: terminals {:?}; starts {}; diagnostics {}",
                self.shared.borrow().terminals,
                self.shared.borrow().starts,
                self.shared.borrow().diagnostics
            )
        })
    }
    fn passed(&self) -> bool {
        self.done
    }
    fn last_word(&self) -> skein_world::LastWord {
        skein_world::LastWord::LastExpectation
    }
}

impl Judge {
    fn prepare_open(&mut self, procs: &mut [Proc]) {
        if self.opening == Opening::Creating
            && let Some(task) = self.party.last_task
        {
            let client = procs
                .iter_mut()
                .find_map(|p| if let Proc::Client(client) = p { Some(client) } else { None })
                .expect("native watcher");
            client.account_request(jig_client::Event::Watch {
                handle: jig_client::Handle { asker: jig_client::Asker(78), number: 70 },
                subject: jig_client::Watchable::Jig(jig_client::contract::Subject::Task { task }),
            });
            self.opening = Opening::Watching;
        }
        if self.opening == Opening::Watching
            && self.store.rows.values().any(|row| {
                matches!(row, jig_test_domain::Record::Core(core::Record::Tasks(jig_domain_tasks::Stored::Live(task)))
                if task.inbox.iter().any(|message| message.words.as_ref() == b"Hello"))
            })
        {
            for proc in procs.iter_mut() {
                if let Proc::Worker(host) = proc {
                    host.relay.until = None;
                }
            }
            self.opening = Opening::Ready;
        }
    }
    fn steer(&mut self, procs: &mut [Proc], parked: bool) {
        if self.action == Action::Ready && matches!(self.case, Case::Waiting | Case::Open) && parked {
            self.shared.borrow_mut().sent_after_park = self.store.rows.values().find_map(|row| {
                if let jig_test_domain::Record::Core(core::Record::Core(core::CoreRecord::Terminal(answer))) = row
                    && answer.end == jig_domain_tasks::End::Parked
                {
                    Some(answer.attempt)
                } else {
                    None
                }
            });
            self.party.extend(Box::new([say([2; 16], b"After waiting")]));
            self.action = Action::Sent;
        }
        if self.action == Action::Ready && self.case == Case::SignOut && waiting_on(procs, 1) {
            let client = procs
                .iter_mut()
                .find_map(|p| if let Proc::Client(client) = p { Some(client) } else { None })
                .expect("client");
            let ask = jig_client::contract::Ask::SignOut {
                holder: jig_client::contract::AccountHolder::Own,
                source: self.accounts.source().expect("declared source"),
            };
            client.account_request(jig_client::Event::Send {
                key: jig_client::Key { asker: jig_client::Asker(79), number: 1 },
                name: Some([80; 16]),
                request: jig_client::Ask::Jig(Box::new(ask)),
            });
            self.action = Action::Sent;
        }
        // Each new fake invocation begins its turn script again. A fresh word answers
        // the resumed invocation's own wait, after its initial resumption message.
        if self.case == Case::Waiting && self.action == Action::Sent && waiting_on(procs, 2) {
            self.party.extend(Box::new([say([3; 16], b"Continue after waiting")]));
            self.action = Action::Resumed;
        }
    }
    fn record(
        &self,
        procs: &mut [Proc],
        turns: Vec<core::TurnRecord>,
        terminals: Vec<core::TerminalRecord>,
        signed_out: bool,
    ) {
        let diagnostics = world::diagnostics(procs);
        let mut run = self.shared.borrow_mut();
        run.diagnostics = diagnostics;
        run.turns = turns;
        run.terminals = terminals;
        run.signed_out = signed_out;
        run.task = self.store.rows.values().find_map(|row| {
            if let jig_test_domain::Record::Core(core::Record::Tasks(jig_domain_tasks::Stored::Live(task))) = row {
                Some(task.clone())
            } else {
                None
            }
        });
        run.held =
            self.store
                .rows
                .values()
                .filter_map(|row| {
                    if let jig_test_domain::Record::Core(core::Record::Accounts(
                        jig_domain_accounts::Stored::Source { number, held, .. },
                    )) = row
                    {
                        Some((*number, *held))
                    } else {
                        None
                    }
                })
                .collect();
        for row in self.store.rows.values() {
            if let jig_test_domain::Record::Core(core::Record::Core(core::CoreRecord::RunProof(proof))) = row
                && let Some(source) = proof.account_failure
                && !run.authenticated_failures.contains(&source)
            {
                run.authenticated_failures.push(source);
            }
        }
        for proc in procs {
            match proc {
                Proc::Worker(host) => {
                    run.worker(&host.wire);
                    if self.done && !host.stopping {
                        host.stopping = true;
                        host.service.shutdown();
                    }
                }
                Proc::Engine(host) if self.done && !host.stopping => {
                    host.stopping = true;
                    host.service.shutdown();
                }
                Proc::Client(client) => {
                    run.run_waits = client
                        .received
                        .iter()
                        .filter_map(|received| {
                            if let jig_client_protocol_world::native::Received::Delivery(
                                jig_client::contract::Delivery::Task(task),
                            ) = received
                            {
                                Some(task.run_wait)
                            } else {
                                None
                            }
                        })
                        .collect();
                }
                Proc::Engine(_) | Proc::Agent { .. } | Proc::Auth(_) | Proc::Peer(_) | Proc::Probe(_) => {}
            }
        }
    }
}

fn waiting_on(procs: &[Proc], attempt: u64) -> bool {
    procs.iter().any(|p| {
        if let Proc::Worker(worker) = p {
            worker.wire.frames.iter().any(|(_, kind, body)| {
                if *kind != wire::Kind::Fact.number() {
                    return false;
                }
                let fact = wire::Fact::decode(&wire::CEILINGS, &mut Reader::new(body)).expect("fact");
                fact.attempt() == attempt
                    && matches!(fact.value(),wire::FactValue::Phase(phase) if *phase.value()==wire::Phase::Waiting)
            })
        } else {
            false
        }
    })
}

fn party(case: Case) -> jig_fake_parties::Party {
    let mut script = fixtures::party_script(&fixtures::one_chat(b"Hello")).into_vec();
    if case == Case::Open {
        script.push(say([5; 16], b"Hello"));
    }
    jig_fake_parties::Party::new(1, jig_fake_parties::Role::Owner, 100, script.into_boxed_slice())
}

#[must_use]
pub fn run(case: Case, story: &smith_fake_claude::Story) -> Run {
    let mut files = skein_fake_machine::Machine::new();
    let mut items = vec![
        Item::directory(b"state"),
        Item::directory(b"private"),
        Item::directory(b"private/home"),
        Item::directory(b"private/programs"),
        Item::directory(b"private/programs/alice"),
    ];
    for item in &mut items {
        item.mode = 0o700;
    }
    items.push(Item::file(b"private/programs/alice/fake-claude.story", &story.encode()).mode(0o600));
    items.push(Item::file(b"private/programs/alice/fake-claude.login", b"claude.ai").mode(0o600));
    let base = files.lay(&items);
    let worker = files.open(base, b".", How::Directory).expect("worker root");
    let client = files.open(base, b".", How::Directory).expect("client root");
    let shared = Rc::new(RefCell::new(Run::default()));
    let judge = Judge {
        shared: shared.clone(),
        store: Store::new(),
        rules: Rules::default(),
        views: Views::default(),
        accounts: account_setup::Setup::program(),
        party: party(case),
        period: false,
        done: false,
        case,
        action: Action::Ready,
        opening: Opening::Creating,
    };
    let mut world = skein_world::World::new(912, skein_sim::Config::calm(), judge, skein_world::Memory::Unchecked)
        .with_machine(Files { files, base });
    world.host_startup(
        HostedProgram {
            program: b"program-agent".as_slice().into(),
            make: agent,
            instances: 2,
            operations: smith_agent_service::program_operations(&configuration())
                .expect("program routes")
                .checked_add(8)
                .expect("shell operations"),
        },
        |_| vec![StartupRoot { name: b"programs".as_slice().into(), path: b"/private/programs".as_slice().into() }],
        |_| vec![],
    );
    let fake = smith_fake_claude::hosted(b"/private/programs/claude");
    world.host_startup(
        HostedProgram { program: fake.program, make: claude, instances: 2, operations: fake.operations },
        |spawn| smith_fake_claude::config_root(spawn).into_iter().chain(smith_fake_claude::mcp_root(spawn)).collect(),
        |spawn| smith_fake_claude::observation_append(spawn).into_iter().collect(),
    );
    world.host(HostedProgram {
        program: b"/private/programs/relay".as_slice().into(),
        make: relay,
        instances: 1,
        operations: smith_agent_shell::relay::operations(),
    });
    world.spawn_restartable(
        vec![StartupRoot { name: b"state".as_slice().into(), path: b"state".as_slice().into() }],
        if case == Case::Open { engine_open } else { engine },
    );
    world.spawn_root(
        skein_sim::Handle::new(worker.raw()),
        if case == Case::Open { worker_open } else { processes::worker_program_runs },
    );
    world.spawn_root(skein_sim::Handle::new(client.raw()), |root| {
        let mut client = jig_client_protocol_world::native::NativeClient::new(
            jig_client_protocol_world::native::settings(),
            root,
            1000,
            fixtures::SEED,
            jig_client_protocol_world::native::Script::account_sign_in(),
        );
        client.hold_for_account_sign_in();
        Proc::Client(Box::new(client))
    });
    let mut outcome = world.run();
    assert!(
        !shared.borrow().terminals.is_empty(),
        "program ran: {:?}; {}",
        outcome.trace,
        world::diagnostics(&outcome.procs)
    );
    let fd = outcome
        .machine
        .files
        .open(base, b"private/programs/alice/fake-claude.observations", How::Read)
        .expect("fake invoked");
    let bytes = outcome.machine.files.read(fd, 0, 65_536).expect("observations");
    assert!(bytes.len() < 65_536, "fake observations must not be truncated");
    outcome.machine.files.close(fd);
    outcome.machine.files.close(base);
    let mut run = std::mem::take(&mut *shared.borrow_mut());
    run.diagnostics = world::diagnostics(&outcome.procs);

    run.observations = smith_fake_claude::observations(&bytes);
    run.seen = smith_fake_claude::seen(&bytes);
    run.received = smith_fake_claude::received(&bytes);
    run
}
