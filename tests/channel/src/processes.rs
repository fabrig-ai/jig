//! Host adapters expose only service ports to the scenario (service.md, 4).
use crate::{fixtures, relay::Relay, world::Capture};
use jig_test_service::Service;
use skein_io::kernel;
use skein_lib::{Queue, Time, Wall};
use skein_shell::Host;
use skein_world::{Inherited, PipeHost};
use smith_agent_process_world as agent_fixture;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) enum Proc {
    Auth(Box<dyn Host>),
    Client(Box<jig_client_protocol_world::native::NativeClient>),
    Engine(Box<ServiceHost>),
    Worker(Box<ServiceHost>),
    Agent { host: Box<PipeHost<smith_agent_shell::Agent>>, errors: agent_fixture::AgentErrors },
    Peer(Box<skein_fake_peers::llm::Peer>),
    Probe(Box<crate::relay::Peer>),
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "independent host, shutdown, held-write and transcript-fault controls"
)]
pub(crate) struct ServiceHost {
    pub(crate) service: Service,
    pub(crate) wire: Wire,
    pub(crate) stopping: bool,
    pub(crate) submitted: u32,
    worker: bool,
    pub(crate) files: Capture,
    pub(crate) codec: jig_protocol_store::codec::Limits,
    pub(crate) store_limits: jig_test_service::limits::Limits,
    pub(crate) holding: bool,
    pub(crate) corrupt_transcript: bool,
    pub(crate) restart_root: Option<kernel::Fd>,
    held: Queue<kernel::Submit>,
    pub(crate) relay: Relay,
    pub(crate) revision: (u64, u64),
}
impl ServiceHost {
    fn new(service: Service, worker: bool) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        Self {
            service,
            wire: Wire::default(),
            stopping: false,
            submitted: 0,
            worker,
            files: Capture::default(),
            codec: jig_protocol_store::codec::limits(&fixtures::store_limits().root.core)
                .expect("admitted fixture codec"),
            store_limits: fixtures::store_limits(),
            holding: false,
            corrupt_transcript: false,
            restart_root: None,
            held: Queue::with_capacity(4),
            relay: Relay::new(1024),
            revision: (NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed), 0),
        }
    }
}
impl Host for ServiceHost {
    fn iterate(&mut self, now: Time, wall: Wall) {
        if !self.holding {
            while let Some(submit) = self.held.pop() {
                self.service.submissions().push(submit);
            }
        }
        self.relay.receive(now, self.service.completions());
        for complete in self.service.completions().iter() {
            self.wire.complete(complete, self.worker, now);
            self.files.complete(complete);
        }
        jig_test_service::iterate(&mut self.service, now, wall);
        self.revision.1 += 1;
        for _ in 0..self.service.submissions().len() {
            let mut submit = self.service.submissions().pop().expect("service submissions");
            if self.corrupt_transcript {
                self.files.corrupt_transcript(&mut submit.kind, &self.codec);
            }
            if self.files.pending(&submit.kind) && self.holding {
                self.held.push(submit);
            } else {
                self.service.submissions().push(submit);
            }
        }
        self.submitted += self.service.submissions().len();
        for submit in self.service.submissions().iter() {
            if let kernel::Op::Close { fd } = submit.kind
                && let Some(bound) = self.wire.stop_bounds.get(&fd.raw())
            {
                self.wire.lost.entry(fd.raw()).or_insert(now.saturating_add(*bound));
            }
        }
    }
    fn completions(&mut self) -> &mut Queue<kernel::Complete> {
        self.service.completions()
    }
    fn submissions(&mut self) -> &mut Queue<kernel::Submit> {
        self.service.submissions()
    }
    fn work_pending(&self, now: Time) -> bool {
        self.service.work_pending(now) || self.relay.ready(now) || (!self.holding && !self.held.is_empty())
    }
    fn next_deadline(&self) -> Option<Time> {
        [self.service.next_deadline(), self.relay.until].into_iter().flatten().min()
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        self.service.next_policy_deadline()
    }
    fn is_empty(&self) -> bool {
        self.service.is_empty() && self.restart_root.is_none()
    }
    fn operations(&self) -> u32 {
        self.service.operations()
    }
}
impl Proc {
    fn host(&self) -> &dyn Host {
        match self {
            Self::Engine(host) | Self::Worker(host) => host.as_ref(),
            Self::Agent { host, .. } => host.as_ref(),
            Self::Auth(fake) => fake.as_ref(),
            Self::Peer(peer) => peer.as_ref(),
            Self::Client(client) => client.as_ref(),
            Self::Probe(peer) => peer.as_ref(),
        }
    }
    fn host_mut(&mut self) -> &mut dyn Host {
        match self {
            Self::Engine(host) | Self::Worker(host) => host.as_mut(),
            Self::Agent { host, .. } => host.as_mut(),
            Self::Auth(fake) => fake.as_mut(),
            Self::Peer(peer) => peer.as_mut(),
            Self::Client(client) => client.as_mut(),
            Self::Probe(peer) => peer.as_mut(),
        }
    }
}
impl Host for Proc {
    fn drain(&mut self) {
        self.host_mut().drain();
    }
    fn iterate(&mut self, now: Time, wall: Wall) {
        self.host_mut().iterate(now, wall);
    }
    fn completions(&mut self) -> &mut Queue<kernel::Complete> {
        self.host_mut().completions()
    }
    fn submissions(&mut self) -> &mut Queue<kernel::Submit> {
        self.host_mut().submissions()
    }
    fn work_pending(&self, now: Time) -> bool {
        self.host().work_pending(now)
    }
    fn next_deadline(&self) -> Option<Time> {
        self.host().next_deadline()
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        self.host().next_policy_deadline()
    }
    fn is_empty(&self) -> bool {
        self.host().is_empty()
    }
    fn exit(&self) -> Option<kernel::Exit> {
        self.host().exit()
    }
    fn operations(&self) -> u32 {
        self.host().operations()
    }
}

pub(crate) fn engine(inherited: &Inherited) -> Proc {
    engine_configured(inherited, false)
}
pub(crate) fn engine_providers(inherited: &Inherited) -> Proc {
    engine_document(inherited, &fixtures::providers_document())
}
pub(crate) fn engine_drain(inherited: &Inherited) -> Proc {
    engine_configured(inherited, false)
}
pub(crate) fn engine_calls(inherited: &Inherited) -> Proc {
    engine_configured(inherited, true)
}
pub(crate) fn engine_message(inherited: &Inherited) -> Proc {
    engine_document(inherited, &fixtures::engine_message_document())
}
pub(crate) fn engine_window(inherited: &Inherited) -> Proc {
    engine_document(inherited, &fixtures::engine_window_document())
}
pub(crate) fn engine_application_generation(inherited: &Inherited) -> Proc {
    let document = String::from_utf8(fixtures::engine_document()).expect("fixture UTF-8").replace(
        r#""prefix":["one"],"resources":[]"#,
        r#""prefix":["one"],"resources":[{"path":["one","item"],"state":0}]"#,
    );
    engine_document(inherited, document.as_bytes())
}
fn engine_configured(inherited: &Inherited, calls: bool) -> Proc {
    engine_document(inherited, &if calls { fixtures::engine_call_document() } else { fixtures::engine_document() })
}
pub(crate) fn engine_document(inherited: &Inherited, document: &[u8]) -> Proc {
    let mut service =
        jig_test_service::startup(document, &fixtures::files(false), None, &fixtures::seeds(), None, None)
            .expect("engine configuration");
    service.adopt_signals(inherited.signal).expect("engine signal source");
    let root = inherited.roots.iter().find(|(name, _)| name.as_ref() == b"state").expect("deployment root").1;
    service.adopt_store(root, 1000).expect("store directory");
    let mut host = ServiceHost::new(service, false);
    host.files.root(root);
    host.restart_root = inherited.roots.iter().find(|(name, _)| name.as_ref() == b"restart-state").map(|(_, fd)| *fd);
    Proc::Engine(Box::new(host))
}
pub(crate) fn worker(root: kernel::Fd) -> Proc {
    worker_configured(root, false, false, false)
}
pub(crate) fn worker_bad_charter(root: kernel::Fd) -> Proc {
    worker_configured(root, true, false, false)
}
pub(crate) fn worker_message(root: kernel::Fd) -> Proc {
    worker_configured(root, false, true, false)
}
pub(crate) fn worker_providers(root: kernel::Fd) -> Proc {
    worker_configured(root, false, false, true)
}
fn worker_configured(root: kernel::Fd, bad_charter: bool, message: bool, providers: bool) -> Proc {
    worker_document(root, bad_charter, message, providers, Backend::Direct, fixtures::WORKER_DOCUMENT)
}
#[derive(Clone, Copy)]
enum Backend {
    Direct,
    Calls,
    Program,
}
pub(crate) fn worker_calls(root: kernel::Fd) -> Proc {
    let document = String::from_utf8(fixtures::WORKER_DOCUMENT.to_vec())
        .expect("worker JSON")
        .replace("\"core.call_answer_bytes\":2048", "\"core.call_answer_bytes\":64")
        .replace("\"hub.run_calls\":1", "\"hub.run_calls\":2");
    worker_document(root, false, false, false, Backend::Calls, document.as_bytes())
}
fn worker_document(
    root: kernel::Fd,
    bad_charter: bool,
    message: bool,
    providers: bool,
    backend: Backend,
    document: &[u8],
) -> Proc {
    let mut roots = skein_lib::List::with_capacity(1);
    roots.push(root).expect("one worker root");
    let programs = matches!(backend, Backend::Program);
    let mut agent =
        if programs { fixtures::agent_with_limits(&crate::program_runs::limits()) } else { fixtures::agent() };
    if programs {
        agent.bounds.models[0].window = 272_000;
    }
    if providers {
        let mut models = agent.bounds.models.into_vec();
        models.push(jig_domain_host::ModelBound { name: b"other".as_slice().into(), window: 1_048_576, output: 512 });
        agent.bounds.models = models.into_boxed_slice();
        agent.host_limits.accounts = 3;
    }

    if matches!(backend, Backend::Calls) {
        agent.host_limits.calls = 2;
        agent.limits.calls = 2;
        agent.limits.channel.output_bytes = smith_host_protocol::output_bytes(&agent.limits).expect("host output");
    }
    if bad_charter {
        agent.endpoints = smith_protocol_channel::Endpoints::new(skein_lib::List::with_capacity(0));
    }
    if message {
        agent.host_limits.message_bytes = 256;
        agent.bounds.message_bytes = 512;
    }
    let service = jig_test_service::startup(
        document,
        &fixtures::files(true),
        Some(&agent),
        &fixtures::seeds(),
        None,
        Some(jig_test_service::WorkerRuntime { roots }),
    )
    .expect("worker configuration");
    Proc::Worker(Box::new(ServiceHost::new(service, true)))
}
pub(crate) fn agent(_spawn: &kernel::Spawn, inherited: &Inherited) -> Proc {
    agent_configured(inherited, fixtures::agent_configuration(), false)
}
pub(crate) fn agent_providers(_spawn: &kernel::Spawn, inherited: &Inherited) -> Proc {
    let mut config = fixtures::agent_configuration();
    let mut other = config.domain.models[0].clone();
    other.model = b"other".as_slice().into();
    config.domain.models = Box::new([config.domain.models[0].clone(), other]);
    config.limits.llm.llms = 2;
    config.limits.llm.accounts = 3;
    config.limits.memory = 4 << 30;
    agent_configured(inherited, config, false)
}
pub(crate) fn agent_slow(_spawn: &kernel::Spawn, inherited: &Inherited) -> Proc {
    agent_configured(inherited, fixtures::agent_configuration(), true)
}
/// An agent calling Anthropic's path, where a key goes out as a key.
pub(crate) fn agent_anthropic(_spawn: &kernel::Spawn, inherited: &Inherited) -> Proc {
    agent_configured(inherited, fixtures::anthropic_agent_configuration(), false)
}
fn agent_configured(inherited: &Inherited, mut configuration: smith_agent_service::Config, slow: bool) -> Proc {
    let fd = inherited.pipes.iter().find(|(child, _)| *child == 2).expect("stderr").1;
    let (errors, capture) = agent_fixture::AgentErrors::pipe(fd);
    if slow {
        configuration.domain.models[0].head = skein_lib::Duration::from_secs(30);
    }

    let agent = agent_fixture::configured(configuration, fixtures::SEED, inherited, errors.clone());
    Proc::Agent { host: Box::new(capture.host(agent)), errors }
}

/// Decode exactly the bytes the kernel accepted or received, once per completed operation.
/// A short send or receive extends its descriptor's retained frame prefix.
#[derive(Default)]
pub(crate) struct Wire {
    sources: BTreeMap<i32, Vec<Box<[u8]>>>,
    inventory: BTreeMap<i32, Vec<Box<[u8]>>>,
    completed: BTreeSet<skein_lib::Token>,
    streams: BTreeMap<(i32, bool), Vec<u8>>,
    clients: BTreeSet<i32>,
    pub(crate) acknowledgements: Vec<(u64, u64)>,
    pub(crate) calls_answered: u32,
    pub(crate) frames: Vec<(Time, u16, Box<[u8]>)>,
    pub(crate) grants: Vec<Box<[u8]>>,
    pub(crate) granted: Vec<(i32, jig_protocol_worker_wire::Granted)>,
    pub(crate) channel: Option<kernel::Fd>,
    pub(crate) adopting_hellos: u32,
    pub(crate) start_messages: u32,
    pub(crate) waited: bool,
    pub(crate) closed: bool,
    pub(crate) lost: BTreeMap<i32, Time>,
    stop_bounds: BTreeMap<i32, skein_lib::Duration>,
    pub(crate) placements: BTreeMap<(u64, u64), i32>,
}
/// Every secret a grant carries joins those no record may hold.
fn secrets(grants: &mut Vec<Box<[u8]>>, grant: &jig_protocol_worker_wire::Grant) {
    use jig_protocol_worker_wire::GrantValue;
    match grant.value() {
        GrantValue::Credential(value) => grants.push(value.bytes().expose().into()),
        GrantValue::Application(value) => grants.push(value.secret().expose().into()),
        GrantValue::Program(_) => {}
    }
}

impl Wire {
    #[expect(
        clippy::too_many_lines,
        reason = "one observer records worker frames and checks the independent placement fence"
    )]
    pub(crate) fn complete(&mut self, complete: &kernel::Complete, worker: bool, now: Time) {
        if worker
            && let kernel::Op::Connect { fd, .. } = &complete.kind
            && complete.result.is_ok()
        {
            self.channel = Some(*fd);
        }
        if let kernel::Op::Close { fd } = complete.kind
            && self.streams.keys().any(|(known, _)| *known == fd.raw())
        {
            self.closed = true;
            self.sources.remove(&fd.raw());
            self.inventory.remove(&fd.raw());
        }
        let Ok(kernel::Done::Count(count)) = complete.result else { return };
        let (fd, bytes) = match (&complete.kind, worker) {
            (kernel::Op::Send { fd, bytes, from }, _) => {
                let start = usize::try_from(*from).expect("send offset");
                (*fd, &bytes[start..start + usize::try_from(count).expect("send length")])
            }
            (kernel::Op::Recv { fd, buf }, _) => (*fd, &buf[..usize::try_from(count).expect("receive length")]),
            _ => return,
        };
        if !self.completed.insert(complete.op) {
            return;
        }
        let send = matches!(&complete.kind, kernel::Op::Send { .. });
        if worker && Some(fd) != self.channel {
            return;
        }
        if self.clients.contains(&fd.raw()) {
            return;
        }
        let stream = self.streams.entry((fd.raw(), send)).or_default();
        stream.extend_from_slice(bytes);
        if !worker && (stream.starts_with(b"GET ") || stream.starts_with(b"POST ") || stream.starts_with(b"HTTP/")) {
            self.clients.insert(fd.raw());
            self.streams.retain(|(number, _), _| *number != fd.raw());
            return;
        }
        while stream.len() >= 8 {
            let header = skein_channel::parse_header(&stream[..8]).expect("channel frame header");
            let end = 8 + usize::try_from(header.body_len).expect("bounded frame body");
            if stream.len() < end {
                break;
            }
            self.channel = Some(fd);
            self.frames.push((now, header.kind, stream[8..end].into()));
            if header.kind == 0x0188 {
                let sources = jig_protocol_worker_wire::Sources::decode(
                    &jig_protocol_worker_wire::CEILINGS,
                    &mut skein_lib::Reader::new(&stream[8..end]),
                )
                .expect("whole sources");
                self.sources
                    .insert(fd.raw(), sources.sources().iter().map(|source| Box::from(source.name())).collect());
            }
            if header.kind == 0x0108 {
                let rows = jig_protocol_worker_wire::Logins::decode(
                    &jig_protocol_worker_wire::CEILINGS,
                    &mut skein_lib::Reader::new(&stream[8..end]),
                )
                .expect("whole inventory");
                // Read and write completions can be delivered in either order.
                // Retain the received evidence even if Sources' send completion
                // has not reached the observer yet.
                self.inventory.insert(fd.raw(), rows.logins().iter().map(|row| Box::from(row.name())).collect());
            }
            if header.kind == 0x0180 {
                if !worker {
                    assert!(
                        self.sources.get(&fd.raw()).is_some_and(|names| {
                            self.inventory.get(&fd.raw()).is_some_and(|rows| {
                                names.len() == rows.len() && names.iter().all(|name| rows.contains(name))
                            })
                        }),
                        "assignment follows the matching first inventory, including empty Sources"
                    );
                }
                let assignment = jig_protocol_worker_wire::Assignment::decode(
                    &jig_protocol_worker_wire::CEILINGS,
                    &mut skein_lib::Reader::new(&stream[8..end]),
                )
                .expect("assignment");
                self.placements.insert((assignment.task(), assignment.attempt()), fd.raw());
                self.start_messages += assignment.messages().len();
                for grant in assignment.grants() {
                    secrets(&mut self.grants, grant);
                }
            }
            if header.kind == 0x0187 {
                let granted = jig_protocol_worker_wire::Granted::decode(
                    &jig_protocol_worker_wire::CEILINGS,
                    &mut skein_lib::Reader::new(&stream[8..end]),
                )
                .expect("granted credential");
                secrets(&mut self.grants, granted.grant());
                self.granted.push((fd.raw(), granted));
            }
            if header.kind == 0x0104 {
                let fact = jig_protocol_worker_wire::Fact::decode(
                    &jig_protocol_worker_wire::CEILINGS,
                    &mut skein_lib::Reader::new(&stream[8..end]),
                )
                .expect("activity fact");
                if let jig_protocol_worker_wire::FactValue::Phase(value) = fact.value() {
                    self.waited |= matches!(value.value(), jig_protocol_worker_wire::Phase::Waiting);
                }
            }
            if header.kind == 0x0100 {
                let hello = jig_protocol_worker_wire::Hello::decode(
                    &jig_protocol_worker_wire::CEILINGS,
                    &mut skein_lib::Reader::new(&stream[8..end]),
                )
                .expect("hello");
                self.stop_bounds.insert(fd.raw(), hello.stop_bound());
                if !hello.hosting().is_empty() {
                    self.adopting_hellos += 1;
                }
            }
            if header.kind == 0x0185 {
                let ack = jig_protocol_worker_wire::Acknowledge::decode(
                    &jig_protocol_worker_wire::CEILINGS,
                    &mut skein_lib::Reader::new(&stream[8..end]),
                )
                .expect("channel acknowledgement");
                self.acknowledgements.push((ack.task(), ack.attempt()));
            } else if header.kind == 0x0182 {
                self.calls_answered += 1;
            }
            stream.drain(..end);
        }
    }
}

pub(crate) struct Machine(pub(crate) skein_fake_machine::Machine);
impl skein_world::Machine for Machine {
    fn open_root(&mut self, path: &[u8]) -> Result<skein_sim::Handle, kernel::Error> {
        assert_eq!(path, b"state");
        Ok(skein_sim::Handle::new(self.0.reopen_root(0).raw()))
    }
    fn close_root(&mut self, root: skein_sim::Handle) {
        self.0.close(skein_fake_machine::Opened::new(root.raw()));
    }
    fn cut(&mut self, cut: skein_world::Cut, held: &[skein_sim::Handle], seed: u64) {
        match cut {
            skein_world::Cut::Kill => {
                for handle in held {
                    self.0.close(skein_fake_machine::Opened::new(handle.raw()));
                }
            }
            skein_world::Cut::PowerLoss => self.0.crash(seed),
        }
    }
    fn step(&mut self, call: skein_sim::Call, answers: &mut Queue<skein_sim::Answer>) {
        skein_fake_machine::step(&mut self.0, call, answers);
    }
}

/// A seeded real provider: the second request fails with context-too-long;
/// subsequent requests accept the native compaction and resumed run.
pub(crate) fn window_provider(scripts: Box<[skein_fake_llm_domain::api::Script]>) -> skein_fake_peers::llm::Peer {
    use skein_fake_llm_protocol::{documents, provider};
    let mut config = skein_llm_world::fake::config();
    config.too_long = 200;
    let mut protocol = skein_llm_world::fake::limits(&skein_llm_world::limits());
    // This story checks resumed windows; tiny HTTP pieces only repeat the loop.
    protocol.http.send = 4096;
    protocol.http.read = 4096;
    protocol.sse.chunk = 4096;
    protocol.documents.openai.request = 32_768;
    protocol.documents.openai.strings = 4096;
    protocol.documents.openai.tokens = 1024;
    protocol.documents.openai.output_items = 16;
    protocol.documents.anthropic.output_items = 16;
    skein_fake_peers::llm::Peer::new(
        (std::net::Ipv4Addr::LOCALHOST, 443).into(),
        skein_fake_peers::Transport::Plaintext,
        agent_fixture::fake::limits(),
        provider::Config {
            without_credential: false,
            echo: skein_llm::openai::Echo::NONE,
            usage_fields: documents::UsageFields::ALL,
            provider: documents::Provider::OpenAi,
            path: skein_llm::Endpoint::openai().target,
            headers: Box::default(),
        },
        skein_llm::Credential::bearer(fixtures::TOKEN.into()),
        protocol,
        skein_fake_llm_domain::Domain::try_scripted(&config, 12, scripts).expect("window provider script"),
        config,
    )
    .expect("window provider")
}

pub(crate) fn auth(spawn: &kernel::Spawn, inherited: &Inherited) -> Proc {
    assert!(inherited.pipes.iter().all(|(number, _)| *number != 0));
    assert!(spawn.env.iter().any(|entry| entry.as_ref() == b"CLAUDE_CONFIG_DIR=/private/programs/alice"));
    assert!(
        !spawn
            .env
            .iter()
            .any(|entry| entry.starts_with(b"ANTHROPIC_") || entry.starts_with(b"CLAUDE_CODE_OAUTH_TOKEN="))
    );
    Proc::Auth(Box::new((smith_fake_claude::hosted(b"claude").make)(spawn, inherited)))
}
pub(crate) fn worker_programs(root: kernel::Fd) -> Proc {
    worker_programs_with(root, false)
}
pub(crate) fn worker_program_runs(root: kernel::Fd) -> Proc {
    worker_programs_with(root, true)
}
fn worker_programs_with(root: kernel::Fd, runs: bool) -> Proc {
    let document = String::from_utf8(fixtures::WORKER_DOCUMENT.to_vec()).expect("fixture JSON");
    let document = if runs {
        document.replace("agent-program", "program-agent")
    } else {
        document.replace(r#""hub.slots":1"#, r#""hub.detail_bytes":8,"hub.slots":1"#)
    };
    let mut worker = worker_document(
        root,
        false,
        false,
        false,
        if runs { Backend::Program } else { Backend::Direct },
        document.as_bytes(),
    );
    let Proc::Worker(host) = &mut worker else { unreachable!("worker factory") };
    host.service
        .configure_program_sources(
            0,
            jig_test_service::program_sources::Resources::new(
                smith_protocol_program::Programs {
                    directory: b"/private/programs".as_slice().into(),
                    claude: if runs {
                        b"/private/programs/claude".as_slice().into()
                    } else {
                        b"claude".as_slice().into()
                    },
                },
                Box::new([
                    Box::from(b"HOME=/private/home".as_slice()),
                    Box::from(b"ANTHROPIC_API_KEY=fixture-only".as_slice()),
                    Box::from(b"CLAUDE_CODE_OAUTH_TOKEN=fixture-only".as_slice()),
                ]),
            )
            .expect("private bounded source resources"),
        )
        .expect("before worker loop");
    worker
}
