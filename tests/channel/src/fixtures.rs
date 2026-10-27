//! One bounded chat deployment and independent provider script (channel.md, 12).
use crate::world::{Script, Settings};
use jig_test_service::{Files, Seeds};
use skein_lib::{Duration, List, Secret};

pub const SEED: u64 = 7;
pub const TOKEN: &[u8] = b"test-token";
/// A key the story keeps through a real request before the attempt starts.
pub const KEY: &[u8] = b"test-key";
/// The key's next generation, put while the attempt runs.
pub const NEXT_KEY: &[u8] = b"test-key-2";
const DIGEST: &[u8] = b"6fb46f7a92742970166379ed5195e79c4493a7cc5664280c039cfd4095ba5faf";
const REPORT: &[u8] = br#"{"form":"report","text":"Hello","fields":{}}"#;

#[must_use]
pub fn calm(seed: u64) -> Settings {
    Settings { seed, config: skein_sim::Config::calm() }
}
#[must_use]
pub fn one_chat(words: &[u8]) -> Script {
    Script {
        words: words.into(),
        result: REPORT.into(),
        leading_turns: 0,
        latency: Duration::ZERO,
        story: crate::stories::Story::Chat,
        recorded: None,
    }
}

// People's projects includes the configured project and the deployment's roles.
const ENGINE: &[u8] = br#"{
"limits": {"profile": "small", "overrides": {
"tasks.result_bytes":32,"people.amendment_bytes":64,"core.policy_bytes":512,"tasks.spec_bytes":128,"tasks.message_bytes":512,"tasks.hold_bytes":64,"tasks.authority_bytes":64,"people.words":512,"core.call_answer_bytes":2048,"notes.recalled":1,
"people.operations":2,"tasks.tasks":2,"tasks.exemptions":2,"tasks.funders":2,"tasks.project_tasks":1,"tasks.tree_tasks":1,"tasks.project_goals":1,
"tasks.batch":1,"tasks.dependencies":1,"tasks.delegates":1,"tasks.charters":1,
"people.people":1,"people.projects":2,"people.sign_ins":2,"people.holdings":1,
"people.inbox_tasks":4,"people.pending":1,"people.waiters":1,
"fleet.attempts":1,"fleet.calls":1,"fleet.turns":8,
"brief.briefs":1,"brief.sections":1,
"notes.entries_per_scope":1,"views.runs":1,"views.watchers":4,"views.watchers_per_party":4,
"core.load_slots":1,"core.load_bytes":8192,"core.resume_bytes":4096,"core.run_bytes":4096,"views.text_bytes":1024
,"core.templates":2,"documents.rows":1,"documents.lines":1,"documents.chips":1,"documents.text_bytes":128
}},
"llm": {
"providers":[{"name":"fake-provider","number":1,"models":["fake"],"sources":{"keys":true}}],
"endpoints": [{
"name": "fake",
"number": 1,
"dialect": 1,
"provider": "fake-provider",
"address": {"socket": "127.0.0.1:7101"},
"transport": "plaintext",
"headers": [],
"connect_ms": 10000,
"handshake_ms": 10000
}],
"models": [{
"name": "fake",
"endpoint": "fake",
"window": 1048576,
"output": 512,
"prices": {"input": 1, "cached": 1, "output": 1, "unit": 1000000}
}]
},
"hosts": {
"workers": [
{"name": "worker-a", "digest": "secrets/worker-a.digest"}
],
"listener": [{"socket": "127.0.0.1:7200"}]
},
"charters": [{
"number": 1, "instructions": "charters/chat.txt", "model": "fake", "alternatives": [],
"hosts": "workers", "waiting_ms": 10000, "resume": true,
"turns": 4, "time_ms": 30000, "reserve_turns": 1, "reserve_time_ms": 5000, "spend": 20,
"inspect": false, "modify": false, "shell": false, "agents": false, "call_timeout_ms": 10000
}],
"templates": [
{"number": 1, "name": "chat", "executor": {"charter": 1}, "contract": {"report": {"words": 32}}, "shallower": 1, "budget": {"as_allowed": null}, "deadline": "none", "tracked": null, "conversation": true, "listed": false, "retention": "kept", "label": 1, "grace_ms": 5000, "llm": null},
{"number": 2, "name": "goal", "executor": "chosen", "contract": {"report": {"words": 32}}, "shallower": 1, "budget": "given", "deadline": "given", "tracked": {"priority": 1}, "conversation": false, "listed": true, "retention": "kept", "label": 2, "grace_ms": null, "llm": null}
],
"clients": {
"heartbeat_ms": 10000,
"link": true
},
"deployment": {"directory": "state", "seed": 7},
"owners": [{"provider": 1, "subject": "owner-a"}],
"providers": [{"number": 1, "kind": "link"}, {"number": 2, "kind": "deployment"}],
"connectors": [1,2],
"memory_bytes": 1152921504606846976,
"rules": {
"ceiling": {"tools": 0, "grants": [],
"delegation": {"kinds": [{"kind": "charter", "number": 1}], "tasks": 8, "depth": 8},
"budget": {"spend": 100, "deadline_unix_seconds": null}, "notes": 0, "note_resources": []},
"period_spend": 100, "minimum_run_spend": 1, "maximum_run_spend": 100,
"implies": [], "requirements": []
},
"projects": [{"number": 1, "roles": [{"connector": 1, "permission": 1, "role": 0}],
"policy": {"templates": [], "escalation_role": 0, "period_spend": 100, "projections": [], "requirements": [],
"ceiling": {"tools": 0, "grants": [],
"delegation": {"kinds": [{"kind": "charter", "number": 1}], "tasks": 8, "depth": 8},
"budget": {"spend": 100, "deadline_unix_seconds": null}, "notes": 0, "note_resources": []},
"roles": [{"number": 0, "period_spend": 100, "requests": 511, "decides": 31,
"authority": {"tools": 0, "grants": [],
"delegation": {"kinds": [{"kind": "charter", "number": 1}], "tasks": 8, "depth": 8},
"budget": {"spend": 100, "deadline_unix_seconds": null}, "notes": 0, "note_resources": []}}]
}}]
}"#;

pub const WORKER_DOCUMENT: &[u8] = br#"{
"workers":[{
"engine":{"socket":"127.0.0.1:7200"},"name":"worker-a","secret":"worker.secret","slots":1,
"programs":{"check_interval_ms":60000},
"graces":{"grace_ms":10000,"agent_stop_ms":5000,"delivery_ms":1000,"save_ms":1000},
"agent":{"program":"agent-program","configuration":"agent.json","arguments":["agent"]},
"limits":{"profile":"small","overrides":{"hub.slots":1,"hub.run_calls":1,"tasks.message_bytes":512,"people.words":512,"core.call_answer_bytes":2048}},
"memory_bytes":1152921504606846976
}],"application":{"workers":[{"root":"agent-root"}]}}
"#;

#[must_use]
pub fn engine_document() -> Vec<u8> {
    document(ENGINE)
}

fn document(engine: &[u8]) -> Vec<u8> {
    let mut document = b"{\"engine\":".to_vec();
    document.extend_from_slice(engine);
    document.extend_from_slice(
        br#", "application":{"server":{"listener":{"socket":"127.0.0.1:7100"}},"credential":"app.secret","connectors":[
        {"prefix":["one"],"resources":[]},{"prefix":["two"],"resources":[]}]},
        "service":{"components":["accounts","workers","clients"]}}"#,
    );
    document
}
#[must_use]
pub fn files(worker: bool) -> Files {
    let inputs: &[(&[u8], &[u8])] = if worker {
        &[(b"worker.secret", b"worker-secret"), (b"agent.json", b"{}")]
    } else {
        &[
            (b"charters/chat.txt", b"Reply to the person."),
            (b"secrets/worker-a.digest", DIGEST),
            (b"app.secret", b"application-token"),
        ]
    };
    let mut entries = List::with_capacity(u32::try_from(inputs.len()).expect("fixture files"));
    for (name, bytes) in inputs {
        entries.push(((*name).into(), Secret::new((*bytes).into()))).expect("fixture files");
    }
    Files { entries }
}
#[must_use]
pub fn seeds() -> Seeds {
    Seeds { deployment: [7; 16], installation: [9; 16], seed: SEED, crypto: Secret::new(Box::new([7; 32])) }
}
pub(crate) fn endpoints() -> smith_protocol_channel::Endpoints {
    let mut entries = List::with_capacity(1);
    entries
        .push(smith_protocol_channel::Endpoint { name: b"fake".as_slice().into(), number: 0, dialect: 1 })
        .expect("one endpoint");
    smith_protocol_channel::Endpoints::new(entries)
}
#[must_use]
pub fn agent() -> jig_test_service::Agent {
    agent_with_limits(&smith_agent_process_world::limits())
}
pub(crate) fn agent_with_limits(agent_limits: &smith_agent_service::Limits) -> jig_test_service::Agent {
    use skein_lib::Duration;
    let host_limits = smith_host_domain::Limits {
        agents: 1,
        directories: 1,
        conflicts: 0,
        path_bytes: 64,
        name_bytes: 64,
        accounts: 2,
        charter_bytes: 4096,
        transcript_bytes: 16_384,
        answered_bytes: 65_536,
        message_bytes: 65_536,
        messages: 8,
        calls: 1,
        call_bytes: 65_536,
        answer_bytes: smith_host_domain::Delivered::worst_case(),
        turns: 8,
        turn_bytes: 8192,
        unacknowledged_bytes: smith_domain::max_turn_bytes(&agent_limits.domain).expect("one concrete agent turn"),
        fact_bytes: 65_536,
        outcome_bytes: 4096,
        detail_bytes: 64,
        spawn_timeout: Duration::from_secs(1),
        no_progress: Duration::from_secs(120),
        silence: Duration::from_secs(60),
        long_span: Duration::from_secs(60),
        long_open: 1,
        wall_time: Duration::from_secs(300),
        cancel_grace: Duration::from_secs(5),
        exit_grace: Duration::from_secs(5),
        teardown: Duration::from_secs(1),
        kill_after: Duration::from_secs(2),
        facts: 1,
    };
    let channel = agent_limits.channel;
    let mut limits = smith_host_protocol::Limits {
        bodies: channel.bodies,
        charter: channel.charter,
        transcript: channel.transcript,
        endpoints: channel.endpoints,
        channel: channel.channel,
        start_bytes: channel.start_bytes,
        calls: host_limits.calls,
    };
    limits.channel.output_bytes = smith_host_protocol::output_bytes(&limits).expect("host frames");
    jig_test_service::Agent {
        endpoints: endpoints(),
        limits,
        host_limits,
        bounds: jig_domain_host::Bounds {
            program_endpoints: Box::new([]),
            models: Box::new([jig_domain_host::ModelBound {
                name: b"fake".as_slice().into(),
                window: 1_048_576,
                output: 512,
            }]),
            turns: 8,
            spend: 100,
            time: Duration::from_secs(300),
            waiting: Duration::from_secs(60),
            checks: Duration::from_secs(60),
            message_bytes: 65_536,
            inbox: 8,
            unacknowledged: 8,
        },
    }
}

/// The agent calls `OpenAI`'s Responses path, which takes a bearer or a key alike.
#[must_use]
pub fn agent_configuration() -> smith_agent_service::Config {
    agent_configuration_for(skein_llm::Endpoint::openai())
}
/// The agent calls Anthropic's Messages path, where a key and a bearer differ.
#[must_use]
pub fn anthropic_agent_configuration() -> smith_agent_service::Config {
    agent_configuration_for(skein_llm::Endpoint::anthropic())
}
fn agent_configuration_for(llm: skein_llm::Endpoint) -> smith_agent_service::Config {
    let mut config = smith_agent_process_world::configuration();
    config.limits.llm.accounts = 2;
    config.channel_endpoints = endpoints();
    let (destinations, _) = config.llm_endpoints.into_parts();
    let mut destination = destinations.into_boxed().into_vec().pop().expect("one provider destination");
    destination.llm = llm;
    config.llm_endpoints = smith_protocol_llm::Endpoints::new(
        Box::new([smith_protocol_llm::ConfiguredEndpoint {
            name: smith_domain::llm::Endpoint(0),
            destination,
            connect: Duration::from_secs(1),
            handshake: Duration::from_secs(1),
            identity: smith_protocol_llm::IdentityProfile::Plain,
            models: Box::default(),
        }]),
        1,
    )
    .expect("provider account");
    config
}

/// The provider at the dialect's path, admitting the handed-in credential:
/// a grant's token, and in Anthropic's dialect its scheme, reach it as given.
/// Bounds as Smith's process world gives its configured peer.
#[must_use]
pub fn provider(
    scripts: Box<[skein_fake_llm_domain::api::Script]>,
    credential: skein_llm::Credential,
    latency: Duration,
    transport: skein_fake_peers::Transport,
    dialect: skein_fake_llm_protocol::documents::Provider,
) -> skein_fake_peers::llm::Peer {
    use skein_fake_llm_protocol::{documents, provider};
    let mut config = skein_llm_world::fake::config();
    config.latency_min = latency;
    config.latency_max = latency;
    config.script_bytes = config.script_bytes.max(16_384);
    config.answer_bytes = config.answer_bytes.max(16_384);
    config.query_bytes = config.query_bytes.max(16_384);
    let mut protocol = skein_llm_world::fake::limits(&skein_llm_world::limits());
    protocol.documents.openai.request = 32_768;
    protocol.documents.openai.strings = 16_384;
    protocol.documents.openai.answer = protocol.documents.openai.answer.max(16_384);
    protocol.http.body = protocol.http.body.max(32_768);
    protocol.sse.event = protocol.sse.event.max(16_384);
    protocol.documents.openai.tokens = 4096;
    protocol.documents.openai.output_items = 256;
    protocol.documents.anthropic.output_items = 256;
    skein_fake_peers::llm::Peer::new(
        (
            smith_agent_process_world::fake::loopback(),
            if transport == skein_fake_peers::Transport::Tls { 34_443 } else { 443 },
        )
            .into(),
        transport,
        smith_agent_process_world::fake::limits(),
        provider::Config {
            echo: skein_llm::openai::Echo::NONE,
            usage_fields: documents::UsageFields::ALL,
            provider: dialect,
            path: match dialect {
                documents::Provider::OpenAi => skein_llm::Endpoint::openai().target,
                documents::Provider::Anthropic => skein_llm::Endpoint::anthropic().target,
            },
            headers: Box::default(),
            without_credential: false,
        },
        credential,
        protocol,
        skein_fake_llm_domain::Domain::try_scripted(&config, 17, scripts).expect("bounded provider scripts"),
        config,
    )
    .expect("configured provider")
}

pub(crate) fn peer_bounds() -> (skein_channel::Schema, skein_channel::Limits) {
    let profile = store_limits();
    let sizes = profile.workers.sizes;
    let codec = jig_protocol_worker_wire::sizes::limits(&sizes).expect("small wire");
    let cap = jig_protocol_worker_wire::sizes::worker_cap(&sizes).expect("peer output").cap;
    let limits = skein_channel::Limits {
        chunk: 16_384,
        credential: 4096,
        skip: 16_384,
        output_bytes: cap.bytes,
        output_frames: cap.frames,
        kinds: u32::try_from(jig_protocol_worker_wire::KINDS.len()).expect("kinds fit"),
    };
    (jig_protocol_worker_wire::schema(&codec).expect("small schema"), limits)
}

pub(crate) fn engine_call_document() -> Vec<u8> {
    String::from_utf8(engine_document())
        .expect("fixture UTF-8")
        .replace("\"tools\": 0", "\"tools\": 256")
        .replace("\"notes\": 0", "\"notes\": 1")
        .replace("\"core.call_answer_bytes\":2048", "\"core.call_answer_bytes\":64")
        .into_bytes()
}

pub const TASK: u64 = 1;
pub const ATTEMPT: u64 = 1;
#[must_use]
pub fn referee() -> crate::referee::Rules {
    crate::referee::Rules::default()
}

pub(crate) fn engine_window_document() -> Vec<u8> {
    String::from_utf8(engine_document()).expect("fixture UTF-8").replace("\"turns\": 4", "\"turns\": 8").into_bytes()
}

pub(crate) fn peer_hello() -> skein_channel::Frame {
    use jig_protocol_worker_wire as wire;
    let codec = &wire::CEILINGS;
    let mut models = List::with_capacity(1);
    models
        .push(
            wire::ModelBound::new(
                codec,
                wire::ModelBoundParts { name: b"fake".as_slice().into(), window: 1_048_576, output: 512 },
            )
            .expect("peer model"),
        )
        .expect("one model");
    let bounds = wire::Bounds::new(
        codec,
        wire::BoundsParts {
            program_endpoints: List::with_capacity(0),
            models,
            turns: 8,
            spend: 100,
            time: Duration::from_secs(300),
            waiting: Duration::from_secs(60),
            checks: Duration::from_secs(60),
            message_bytes: 65_536,
            inbox: 8,
            unacknowledged: 8,
        },
    )
    .expect("peer bounds");
    let hello = wire::Hello::new(
        codec,
        wire::HelloParts {
            slots: 1,
            stop_bound: Duration::from_secs(15),
            workstreams: List::with_capacity(0),
            hosting: List::with_capacity(0),
            bounds,
        },
    )
    .expect("peer hello");
    let mut body = skein_lib::Writer::new(hello.measure() as usize);
    hello.encode(&mut body).expect("peer hello");
    let mut frame = skein_channel::frame_writer(0x0100, hello.measure()).expect("hello frame");
    frame.put(&body.finish()).expect("hello body");
    frame.finish().expect("hello frame")
}

#[must_use]
pub fn provider_turns(script: &Script) -> Box<[skein_fake_llm_domain::api::Turn]> {
    use skein_fake_llm_domain::api::{Finish, Line, Turn};
    if matches!(script.story, crate::stories::Story::Window) {
        return vec![
            Turn {
                lines: Box::new([Line::Text { text: b"Progress".as_slice().into() }]),
                finish: Finish::Stop,
                tokens: 1,
            },
            Turn {
                lines: Box::new([Line::Call { name: b"wait".as_slice().into(), arguments: b"{}".as_slice().into() }]),
                finish: Finish::ToolCalls,
                tokens: 1,
            },
            Turn {
                lines: Box::new([Line::Text { text: b"Current summary".as_slice().into() }]),
                finish: Finish::Stop,
                tokens: 1,
            },
            Turn {
                lines: Box::new([Line::Call { name: b"finish".as_slice().into(), arguments: script.result.clone() }]),
                finish: Finish::ToolCalls,
                tokens: 1,
            },
        ]
        .into_boxed_slice();
    }
    if matches!(script.story, crate::stories::Story::Input) {
        return Box::new([crate::live_input::turn()]);
    }
    let mut turns = Vec::new();
    if matches!(script.story, crate::stories::Story::Facts) {
        turns.push(Turn { lines: Box::new([crate::live_text::line()]), finish: Finish::Stop, tokens: 1 });
    }
    for number in 0..script.leading_turns {
        turns.push(Turn {
            lines: Box::new([if matches!(script.story, crate::stories::Story::Call) {
                Line::Call {
                    name: b"recall".as_slice().into(),
                    arguments: if number == 0 {
                        let mut input = br#"{"kind":"name","name":1}"#.to_vec();
                        input.resize(65, b' ');
                        input.into_boxed_slice()
                    } else {
                        br#"{"kind":"name","name":1}"#.as_slice().into()
                    },
                }
            } else if matches!(script.story, crate::stories::Story::Wait | crate::stories::Story::InvalidTranscript) {
                Line::Call { name: b"wait".as_slice().into(), arguments: b"{}".as_slice().into() }
            } else {
                Line::Text { text: format!("Progress {number}").into_bytes().into() }
            }]),
            finish: if matches!(
                script.story,
                crate::stories::Story::Call | crate::stories::Story::Wait | crate::stories::Story::InvalidTranscript
            ) {
                Finish::ToolCalls
            } else {
                Finish::Stop
            },
            tokens: 1,
        });
    }
    if matches!(script.story, crate::stories::Story::Call) {
        turns[1].lines = Box::new([
            Line::Call {
                name: b"recall".as_slice().into(),
                arguments: br#"{"kind":"name","name":1}"#.as_slice().into(),
            },
            Line::Call {
                name: b"recall".as_slice().into(),
                arguments: br#"{"kind":"name","name":2}"#.as_slice().into(),
            },
        ]);
    }
    turns.push(Turn {
        lines: Box::new([Line::Call { name: b"finish".as_slice().into(), arguments: script.result.clone() }]),
        finish: Finish::ToolCalls,
        tokens: 1,
    });
    turns.into_boxed_slice()
}

pub(crate) fn party_script(script: &Script) -> Box<[jig_fake_parties::Act]> {
    if matches!(
        script.story,
        crate::stories::Story::Stalled | crate::stories::Story::Quiet | crate::stories::Story::Replaced
    ) {
        return Box::default();
    }
    let mut acts = vec![
        jig_fake_parties::Act::SignIn { provider: 1, subject: b"owner-a".as_slice().into() },
        jig_fake_parties::Act::Request {
            key: [1; 16],
            ask: jig_fake_parties::Ask::Chat { words: script.words.clone() },
        },
    ];
    if matches!(script.story, crate::stories::Story::Inbox) {
        acts.push(jig_fake_parties::Act::Request {
            key: [2; 16],
            ask: jig_fake_parties::Ask::Say {
                task: jig_fake_parties::Task::Last,
                words: b"From the person".as_slice().into(),
                release: None,
            },
        });
    }
    acts.into_boxed_slice()
}

pub(crate) fn engine_message_document() -> Vec<u8> {
    engine_document()
}

fn engine_configuration() -> jig_protocol_config::Engine {
    configuration(ENGINE)
}

fn configuration(text: &[u8]) -> jig_protocol_config::Engine {
    let bounds = jig_protocol_config::DocumentLimits {
        bytes: u32::try_from(text.len()).expect("fixture document size"),
        tokens: u32::try_from(text.len()).expect("fixture document size"),
        depth: 32,
    };
    let document = jig_protocol_config::document(text, &bounds).expect("fixture document");
    jig_protocol_config::engine(
        &document,
        0,
        &jig_protocol_config::Directories::working(),
        jig_protocol_config::Defaults::Document,
    )
    .expect("fixture engine")
}

/// Match the fixture's final admitted core bounds, parsed once per test process.
#[must_use]
pub fn store_limits() -> jig_test_service::limits::Limits {
    static LIMITS: std::sync::OnceLock<jig_test_service::limits::Limits> = std::sync::OnceLock::new();
    *LIMITS.get_or_init(|| {
        jig_test_service::limits::for_profile(jig_protocol_config::Profile::Small, &engine_configuration().limits)
    })
}

/// The fixture's configured model names, by which the store keeps a role's models.
#[must_use]
pub fn store_names() -> &'static [jig_domain_core::settings::ModelName] {
    static NAMES: std::sync::OnceLock<Box<[jig_domain_core::settings::ModelName]>> = std::sync::OnceLock::new();
    NAMES.get_or_init(|| jig_protocol_store::codec::names(&engine_configuration().llm.models))
}

#[must_use]
pub fn people_member(words: &[u8], spend: u64) -> jig_domain_people::Member {
    use jig_domain_people as people;
    people::Member {
        deadline: jig_domain_people::Deadline::Named,
        executor: people::TaskExecutor::Agent { charter: 1 },
        words: words.into(),
        parameters: Box::new([]),
        inputs: Box::new([]),
        budget: people::Budget::Named,
        named: Box::new([]),
        contract: people::Contract::Report { words: 16 },
        authority: people::Authority {
            tools: 0,
            models: None,
            grants: Box::new([]),
            delegation: people::Delegation { kinds: Box::new([]), tasks: 0, depth: 0 },
            spend,
            deadline: None,
            notes: 0,
            note_resources: Box::new([]),
        },
        wake: people::WakePolicy {
            words: people::WakeRule::Immediate,
            notices: people::WakeRule::Never,
            news: people::WakeRule::Never,
            results: people::ResultsWake::Never,
            questions: false,
            answers: false,
            timers: false,
        },
        dependencies: Box::new([]),
        tracked: None,
        label: 0,
        flags: people::Flags::PLAIN,
        llm: None,
    }
}

#[must_use]
pub fn chat(store: &crate::world::Store, project: u32, words: &[u8]) -> jig_domain_people::Ask {
    use jig_domain_people as people;
    let mut member = people_member(words, 0);
    let Some(jig_test_domain::Record::Core(jig_domain_core::Record::People(people::Stored::Policy { value, .. }))) =
        store.rows.get(&jig_test_domain::Key::Core(jig_domain_core::Key::People(people::Key::Policy(project))))
    else {
        panic!("durable configured policy")
    };
    member.authority = value.roles.iter().find(|role| role.number == 0).expect("fixture role").authority.clone();
    member.authority.spend = 0;
    member.authority.delegation.tasks =
        member.authority.delegation.tasks.min(store_limits().root.core.tasks.tree_tasks).saturating_sub(1);
    member.authority.delegation.depth = member.authority.delegation.depth.saturating_sub(1);
    member.budget = people::Budget::AsAllowed { cap: None };
    member.contract = people::Contract::Report { words: 32 };
    member.flags = people::Flags {
        ended_by: people::EndedBy::Result,
        conversation: true,
        listed: false,
        retention: people::Retention::Kept,
    };
    member.label = jig_client_test_records::CHAT;
    people::Ask::Create { project, batch: Box::new([member]) }
}

/// Two numbered providers share an endpoint but require distinct real source grants.
pub(crate) fn providers_document() -> Vec<u8> {
    document(&providers_engine())
}

fn providers_engine() -> Vec<u8> {
    String::from_utf8(ENGINE.to_vec()).expect("bounded fixture value")
        .replace(r#""providers":[{"name":"fake-provider","number":1,"models":["fake"],"sources":{"keys":true}}]"#,
                 r#""providers":[{"name":"fake-provider","number":1,"models":["fake"],"sources":{"keys":true}},{"name":"other","number":2,"models":["other"],"sources":{"keys":true}}]"#)
        .replace("\"unit\": 1000000}\n}]", "\"unit\": 1000000}\n},{\"name\":\"other\",\"endpoint\":\"fake\",\"window\":1048576,\"output\":512,\"prices\":{\"input\":1,\"cached\":1,\"output\":1,\"unit\":1000000}}]")
        .replace(r#""alternatives": []"#, r#""alternatives": ["other"]"#).into_bytes()
}

pub(crate) fn providers_store_names() -> &'static [jig_domain_core::settings::ModelName] {
    static NAMES: std::sync::OnceLock<Box<[jig_domain_core::settings::ModelName]>> = std::sync::OnceLock::new();
    NAMES.get_or_init(|| jig_protocol_store::codec::names(&configuration(&providers_engine()).llm.models))
}
