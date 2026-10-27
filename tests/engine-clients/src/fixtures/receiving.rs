//! A receiving core with the small profile's document admission bounds.
use jig_domain_authority as authority;
use jig_domain_core as core;
use jig_domain_people as people;
use skein_lib::{Duration, Env, Map, ReplyTo, Time, Token, Wall};

/// The one configured model, which the charter names.
fn model() -> core::Model {
    core::Model {
        dialect: 1,
        endpoint: 1,
        program: None,
        name: Box::from(b"scripted".as_slice()),
        window: 8192,
        output: 1024,
        input_price: 1,
        cached_price: 1,
        cache_write_price: None,
        output_price: 1,
        price_unit: 1_000_000,
        efforts: Box::default(),
        effort: None,
    }
}

/// The configured models as the client side names them, mirroring the core's.
#[must_use]
pub fn models() -> Box<[jig_protocol_engine_clients::Model]> {
    let model = model();
    Box::new([jig_protocol_engine_clients::Model {
        name: model.name,
        provider: None,
        window: model.window,
        output: model.output,
        efforts: model.efforts,
        prices: jig_protocol_engine_clients::Prices {
            input: model.input_price,
            cached: model.cached_price,
            cache_write: model.cache_write_price,
            output: model.output_price,
            unit: model.price_unit,
        },
        description: None,
    }])
}

fn configuration(limits: &core::Limits) -> core::Config {
    let rules = authority::Rules {
        ceiling: authority::Authority {
            tools: authority::Tools(1023),
            models: authority::Models::ALL,
            grants: Box::new([]),
            delegation: authority::Delegation { kinds: Box::new([]), tasks: 0, depth: 0 },
            budget: authority::Budget { spend: 100, deadline: None },
            notes: authority::Scopes(0),
            note_resources: Box::new([]),
        },
        period_spend: 100,
        minimum_run_spend: 1,
        maximum_run_spend: 100,
        implies: authority::Implies::new(Box::new([]), 0).expect("empty implications"),
        requirements: Box::new([]),
    };
    core::Config {
        worker_names: Box::new([]),
        deployment: [31; 16],
        seed: 31,
        workers_in_process: false,
        hub_turns: 2,
        owners: Box::new([]),
        grants: Box::new([]),
        seed_projects: Box::new([]),
        connectors: Box::new([]),
        settings: core::Settings {
            deployment_provider: 2,
            models: Box::new([model()]),
            resume_bytes: 256,
            tail_bytes: 128,
            tools: core::ToolFamilies::JIG,
            key_bytes: 256,
            redirect_bytes: 4096,
            llm_providers: jig_domain_accounts::Providers::none(),
        },
        defaults: core::settings::Defaults {
            charters: Box::new([core::Charter {
                number: 1,
                hosts: jig_domain_fleet::Kinds::Workers,
                spend: 100,
                policy: core::RunPolicy {
                    tools: jig_domain_authority::Tools(2047),
                    instructions: Box::new([]),
                    waiting: Duration::from_secs(1),
                    resume: true,
                    turns: 8,
                    time: Duration::from_secs(60),
                    reserve_turns: 1,
                    reserve_time: Duration::from_secs(10),
                    model: model(),
                    alternatives: Box::new([]),
                    inspect: true,
                    modify: true,
                    shell: true,
                    agents: true,
                    call_timeout: Duration::from_secs(10),
                },
            }]),
            templates: Box::new([]),
            policies: Map::with_capacity(0),
            rules: (authority::Domain::new(rules, limits.authority).expect("bounded authority")).rules().clone(),
        },
    }
}

#[must_use]
pub fn signed_small_core() -> (core::Core, Env<core::Limits>, u64) {
    let env = Env {
        now: Time::ZERO,
        wall: Wall::EPOCH,
        limits: jig_protocol_config::profile(jig_protocol_config::Profile::Small).core,
    };
    let mut core = core::Core::new(configuration(&env.limits), &env.limits).expect("fixture core configuration");
    drop(core::step(&mut core, &env, core::Event::People(people::Event::Restored)));
    let core::Requests::Out(mut out) = core::step(
        &mut core,
        &env,
        core::Event::SignIn { held: None, reply_to: ReplyTo::new(Token::new(90)), identity: super::first_owner() },
    );
    let mut sign_in = None;
    while let Some(request) = out.pop() {
        if let core::Request::Held(value) = request
            && let core::Held::PeopleReply { reply: people::Reply::SignedIn { sign_in: number, .. }, .. } = *value
        {
            sign_in = Some(number);
        }
    }
    (core, env, sign_in.expect("small core signs in"))
}
