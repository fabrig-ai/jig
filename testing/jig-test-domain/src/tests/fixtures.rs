use crate::{Assignment, Config, Domain, Limits};
use jig_domain_authority as authority;
use jig_domain_core as core;
use jig_domain_fleet as fleet;
use jig_domain_host as host;
use jig_domain_tasks as tasks;
use skein_lib::{Duration, Env, JournalLimits, List, Map, Queue, Time, Token, Wall};

pub const CHANNEL: Token = Token::new(17);
pub const RUN: Token = Token::new(2);
pub const ATTEMPT: Token = Token::new(3);

pub fn hello() -> host::Hello {
    host::Hello {
        slots: 2,
        stop_bound: Duration::from_secs(10),
        workstreams: Box::new([4, 5]),
        hosting: Box::new([host::Hosting { run: RUN, attempt: ATTEMPT, phase: host::Phase::Waiting }]),
        bounds: host::Bounds {
            program_endpoints: Box::new([]),
            models: Box::new([host::ModelBound { name: b"small".as_slice().into(), window: 1024, output: 128 }]),
            turns: 8,
            spend: 100,
            time: Duration::from_secs(60),
            waiting: Duration::from_secs(30),
            checks: Duration::from_secs(1),
            message_bytes: 128,
            inbox: 4,
            unacknowledged: 2,
        },
    }
}
pub fn turn() -> host::LinkUp {
    host::LinkUp::Turn {
        run: RUN,
        attempt: ATTEMPT,
        turn: host::Turn {
            opens_window: true,
            turn: 1,
            spent: 9,
            read: Some(Token::new(7)),
            refused: Box::new([host::Refused { name: Token::new(6), reason: host::MessageRefusal::Full }]),
            body: b"opaque turn".as_slice().into(),
        },
    }
}
pub fn answer() -> host::LinkUp {
    host::LinkUp::Answer {
        run: RUN,
        attempt: ATTEMPT,
        answer: host::Answer {
            turns: 1,
            spent: 12,
            read: Some(Token::new(7)),
            refused: Box::new([host::Refused { name: Token::new(6), reason: host::MessageRefusal::Ending }]),
            ending: host::Ending::Ended {
                outcome: b"opaque result".as_slice().into(),
                work: host::Work { left: None, saved: None },
            },
        },
    }
}
pub fn authority() -> authority::Authority {
    authority::Authority {
        tools: authority::Tools(1023),
        models: authority::Models::ALL,
        grants: Box::new([]),
        delegation: authority::Delegation { kinds: Box::new([]), tasks: 0, depth: 0 },
        budget: authority::Budget { spend: 100, deadline: None },
        notes: authority::Scopes(0),
        note_resources: Box::new([]),
    }
}
pub fn env() -> Env<Limits> {
    let core = jig_protocol_config::profile::profile(jig_protocol_config::profile::Profile::Small).core;
    let room = core::room_max(&core).expect("small profile has room");
    let connectors = jig_test_connector::MAX_OUT.checked_mul(2).expect("two connector outputs");
    let writes = room.writes.checked_add(connectors).expect("write room");
    let held = room.held.checked_add(connectors).expect("delivery room");
    Env {
        now: Time::from_nanos(1),
        wall: Wall::from_nanos(1),
        limits: Limits {
            core,
            connector: jig_test_connector_world::LIMITS,
            routes: 512,
            journal: JournalLimits { commits: 2, writes, held, now: held, release: held },
        },
    }
}
pub fn domain() -> Domain {
    let limits = env().limits;
    let authority = authority::Domain::new(
        authority::Rules {
            ceiling: authority(),
            period_spend: 100,
            minimum_run_spend: 1,
            maximum_run_spend: 100,
            implies: authority::Implies::new(Box::new([]), 0).expect("no implications"),
            requirements: Box::new([]),
        },
        limits.core.authority,
    )
    .expect("bounded authority");
    Domain::new(
        Config {
            first: jig_test_connector_world::config(1),
            second: jig_test_connector_world::config(2),
            core: core::Config {
                worker_names: Box::new([b"alpha".as_slice().into(), b"beta".as_slice().into()]),
                deployment: [1; 16],
                seed: 1,
                workers_in_process: false,
                hub_turns: 2,
                grants: Box::new([]),
                owners: Box::new([]),
                seed_projects: Box::new([]),
                connectors: Box::new([1, 2]),
                settings: core::Settings {
                    deployment_provider: 3,
                    models: Box::new([run().policy.model]),
                    resume_bytes: 256,
                    tail_bytes: 128,
                    tools: core::ToolFamilies::JIG,
                    key_bytes: 256,
                    redirect_bytes: 4096,
                    llm_providers: jig_domain_accounts::Providers {
                        providers: Box::new([jig_domain_accounts::Provider {
                            name: b"Fake".as_slice().into(),
                            signed_in_plan: true,
                            manage_usage: Some(b"https://usage.example/".as_slice().into()),
                            number: 1,
                            models: Box::new([b"small".as_slice().into()]),
                            offers: jig_domain_accounts::Offers { sign_in: true, key: false, programs: Box::new([]) },
                        }]),
                    },
                },
                defaults: core::settings::Defaults {
                    charters: Box::new([core::Charter {
                        number: 1,
                        hosts: fleet::Kinds::Workers,
                        spend: 37,
                        policy: run().policy,
                    }]),
                    templates: Box::new([]),
                    policies: Map::with_capacity(0),
                    rules: (authority).rules().clone(),
                },
            },
        },
        &limits,
    )
}
pub fn run() -> Box<core::RunCharter> {
    Box::new(core::RunCharter {
        revision: 1,
        ended_by: jig_domain_tasks::EndedBy::Result,
        sources: Box::new([Some(1)]),
        accounts: Box::new([]),
        placement: jig_domain_fleet::PlacementConstraint {
            channel: Token::new(1),
            kind: jig_domain_fleet::HostKind::Worker,
        },
        budget: 37,
        contract: tasks::Contract::Report { words: 128 },
        authority: tasks::Authority {
            tools: tasks::Tools(1023),
            models: tasks::Models::Named(1),
            grants: Box::new([]),
            delegation: tasks::Delegation { kinds: Box::new([]), tasks: 0, depth: 0 },
            budget: tasks::Budget { spend: 100, deadline: None },
            notes: tasks::Scopes(0),
            note_resources: Box::new([]),
        },
        policy: core::RunPolicy {
            instructions: b"Answer the person".as_slice().into(),
            waiting: Duration::from_secs(30),
            resume: true,
            turns: 8,
            time: Duration::from_secs(60),
            reserve_turns: 1,
            reserve_time: Duration::from_secs(10),
            model: core::Model {
                dialect: 1,
                endpoint: 1,
                program: None,
                name: b"small".as_slice().into(),
                window: 1024,
                output: 128,
                input_price: 1,
                cached_price: 1,
                cache_write_price: None,
                output_price: 1,
                price_unit: 1_000_000,
                efforts: Box::new([]),
                effort: None,
            },
            alternatives: Box::new([]),
            inspect: true,
            modify: false,
            shell: false,
            agents: false,
            call_timeout: Duration::from_secs(10),
        },
    })
}
pub fn assignment() -> Assignment {
    Assignment {
        live: core::LiveAsk::NONE,
        brief: Box::new([]),
        conversation: 1,
        task: RUN.raw(),
        attempt: ATTEMPT.raw(),
        charter: 1,
        run: run(),
        messages: Box::new([]),
        transcript: Box::new([b"prior turn".as_slice().into()]),
        answered: Box::new([]),
        grants: Box::new([jig_domain_accounts::Grant {
            form: jig_domain_accounts::Form::Credential(jig_domain_accounts::Scheme::Bearer),
            account: 1,
            generation: 2,
            valid: Duration::from_secs(30),
        }]),
    }
}
pub fn work() -> Queue<crate::Work> {
    Queue::with_capacity(32)
}

pub fn channel_records() -> Box<[(crate::Delivery, host::LinkDown)]> {
    let mut records = List::with_capacity(7);
    for (value, down) in held_records() {
        records.push((crate::Delivery::Core(value), down)).expect("seven complete records");
    }
    records
        .push((
            crate::Delivery::CallBusy { channel: CHANNEL, task: 2, attempt: 3, name: b"opaque call".as_slice().into() },
            host::LinkDown::Relayed {
                run: RUN,
                attempt: ATTEMPT,
                call: b"opaque call".as_slice().into(),
                answer: host::Relayed::Busy,
            },
        ))
        .expect("seven complete records");
    records
        .push((
            crate::Delivery::CallAnswer {
                channel: CHANNEL,
                task: 2,
                attempt: 3,
                name: b"opaque call".as_slice().into(),
                writes: true,
                call: core::SettledCall {
                    serial: 1,
                    tool: b"tool".as_slice().into(),
                    answer: core::SettledAnswer::Host { error: true, body: b"refused".as_slice().into() },
                },
            },
            host::LinkDown::Relayed {
                run: RUN,
                attempt: ATTEMPT,
                call: b"opaque call".as_slice().into(),
                answer: host::Relayed::Answer { error: true, text: b"refused".as_slice().into() },
            },
        ))
        .expect("seven complete records");
    records
        .push((
            crate::Delivery::Message {
                channel: CHANNEL,
                task: 2,
                attempt: 3,
                word: tasks::Word {
                    number: 7,
                    from: tasks::Party::Person(8),
                    kind: tasks::MessageKind::ProposalDecision { proposal: 9, accepted: true },
                    words: b"Hello".as_slice().into(),
                    at: Wall::from_nanos(1),
                    hits: 1,
                    eligible: true,
                },
            },
            host::LinkDown::Message {
                run: RUN,
                attempt: ATTEMPT,
                name: Token::new(7),
                sender: b"8".as_slice().into(),
                words: b"Proposal accepted 9:\nHello".as_slice().into(),
                finish_requested: false,
            },
        ))
        .expect("seven complete records");
    records.into_boxed()
}

pub fn start_refusal() -> host::LinkUp {
    let host::LinkUp::Answer { run, attempt, mut answer } = answer() else {
        panic!("answer fixture");
    };
    answer.ending = host::Ending::Refused(host::Refusal::Invalid(host::Invalid::Start {
        cause: jig_domain_host::StartCause::Other,
        why: b"the model is absent".as_slice().into(),
    }));
    host::LinkUp::Answer { run, attempt, answer }
}

fn held_records() -> [(core::Held, host::LinkDown); 4] {
    [
        (
            core::Held::Acknowledge { channel: CHANNEL, run: RUN, attempt: ATTEMPT },
            host::LinkDown::Acknowledge { run: RUN, attempt: ATTEMPT },
        ),
        (
            core::Held::AcknowledgeTurn { channel: CHANNEL, run: RUN, attempt: ATTEMPT, turn: 1 },
            host::LinkDown::TurnKept { run: RUN, attempt: ATTEMPT, turn: 1 },
        ),
        (
            core::Held::Cancel { channel: CHANNEL, run: RUN, attempt: ATTEMPT },
            host::LinkDown::Cancel { run: RUN, attempt: ATTEMPT },
        ),
        (
            core::Held::Grant {
                channel: CHANNEL,
                run: RUN,
                attempt: ATTEMPT,
                grant: fleet::Grant {
                    form: fleet::Form::Application,
                    account: 7,
                    generation: 2,
                    valid: Duration::from_secs(30),
                },
            },
            host::LinkDown::Grant {
                run: RUN,
                attempt: ATTEMPT,
                grant: host::Grant {
                    form: host::Form::Application,
                    account: 7,
                    generation: 2,
                    valid: Duration::from_secs(30),
                },
            },
        ),
    ]
}

pub fn fill_journal(domain: &mut Domain, env: &Env<Limits>) {
    for _ in 0..env.limits.journal.commits {
        let mut decision = domain.journal.decision(&skein_lib::JournalRoom { writes: 1, held: 0 }).expect("one write");
        decision
            .write(crate::Write::Erase(crate::Key::Core(core::Key::Tasks(tasks::Key::Ended(1)))))
            .expect("reserved write");
        domain.journal.accept(decision);
        let _commit = domain.journal.commit().expect("commit in flight");
    }
}
