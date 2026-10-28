//! Finite story configuration, reconstructed unchanged on every cold start.
use jig_domain_host as host;
use jig_ops_domain::{Config, Limits};
use jig_ops_domain_infrastructure as infrastructure;
use skein_lib::{Duration, JournalLimits};

#[must_use]
#[expect(clippy::too_many_lines, reason = "the fixture lists every child bound")]
pub fn root_for(seed: u64, environment_chat: bool) -> (Config, Limits) {
    let mut core_limits = jig_core_world::world::limits().core;
    core_limits.fleet.engine_slots = 1;
    core_limits.tasks.tasks = 8;
    core_limits.tasks.hold_kinds = 3;
    core_limits.tasks.project_tasks = 8;
    core_limits.tasks.tree_tasks = 8;
    core_limits.tasks.depth = 4;
    core_limits.tasks.delegates = 3;
    core_limits.tasks.batch = 2;
    core_limits.tasks.parameters = 4;
    core_limits.tasks.spec_bytes = 512;
    core_limits.tasks.authority_grants = 8;
    core_limits.documents.grants = 8;
    core_limits.documents.executors = 8;
    core_limits.tasks.authority_segments = 3;
    core_limits.tasks.authority_bytes = if environment_chat { 128 } else { 64 };
    core_limits.tasks.executor_kinds = 3;
    if environment_chat {
        core_limits.tasks.parameters = 5;
        core_limits.tasks.executor_kinds = 5;
    }
    core_limits.tasks.funders = 16;
    core_limits.tasks.inbox_messages = 16;
    core_limits.tasks.inbox_bytes = 2048;
    core_limits.tasks.result_bytes = 512;
    core_limits.tasks.contract_choices = 2;
    core_limits.tasks.message_bytes = 512;
    core_limits.tasks.proposal_stall = Duration::from_secs(60);
    core_limits.people.words = 512;
    core_limits.people.amendment_bytes = 2048;
    core_limits.people.requests = 16;
    // A close cancels the chat and up to its three live delegates.
    core_limits.people.operations = 4;
    core_limits.people.inbox_entries = 16;
    core_limits.people.sign_in_lifetime = Duration::from_secs(7200);
    core_limits.authority.grants = 8;
    core_limits.notes.scopes = 11;
    core_limits.authority.segments = 4;
    core_limits.authority.executors = if environment_chat { 5 } else { 3 };
    core_limits.authority.batch = 2;
    core_limits.authority.requirements = 2;
    core_limits.fleet.attempts = 8;
    core_limits.fleet.turns = 32;
    core_limits.fleet.calls = 8;
    core_limits.call_records = 16;
    core_limits.resume_bytes = 32768;
    core_limits.brief.sections = 11;
    core_limits.brief.brief_bytes = 8192;
    core_limits.brief.read_bytes = 8192;
    core_limits.call_answer_bytes = 1024;
    let journal = JournalLimits { commits: 4, writes: 20000, held: 20000, now: 128, release: 128 };
    let mut limits = Limits {
        core: core_limits,
        infrastructure: crate::infrastructure_limits(),
        observability: crate::limits(),
        host: host::Limits {
            slots: 1,
            accounts: 1,
            program_sources: 12,
            source_interval: skein_lib::Duration::from_secs(60),
            source_timeout: skein_lib::Duration::from_secs(10),
            charter_bytes: 65536,
            transcript_bytes: 32768,
            delivery_evidence_bytes: 0,
            turn_bytes: 8192,
            outcome_bytes: 512,
            detail_bytes: 64,
            held: 2,
            inbox: 4,
            event_bytes: 8192,
            run_calls: 4,
            facts: 2,
            told: 2,
            turns: 32,
            turn_queue_bytes: 32768,
            piece_bytes: 4096,
        },
        charter: smith_charter::v4::CEILINGS,
        endpoints: 1,
        journal,
        watermark: jig_ops_domain::default_watermark(&journal),
        routes: 256,
    };
    let mut config = Config {
        agent_bounds: super::agent_bounds(),
        tools: crate::protocol::tools(Duration::from_secs(30)),
        core: jig_core_world::world::config(seed).core,
        triage_templates: Box::new([jig_ops_domain_observability::TriageTemplate {
            number: 1,
            delegate: super::scripts::triage(),
        }]),
        lead_templates: Box::new([jig_ops_domain_observability::LeadTemplate {
            number: 1,
            delegate: super::scripts::lead(),
        }]),
        backend: infrastructure::Backend::OperationIds,
        charter_endpoints: Box::new([jig_protocol_charter::EndpointName {
            number: 1,
            dialect: 1,
            name: Box::from(&b"fake"[..]),
        }]),
    };
    limits.observability.answer_bytes = 2048;
    limits.infrastructure.tasks = 8;
    limits.infrastructure.procedures = 8;
    config.core.grants =
        Box::new([jig_domain_people::ActGrant { connector: 2, may: Box::new([jig_domain_people::ActKind::Close]) }]);
    let (domain, policy) = policy_domain(&limits.core.authority, environment_chat);
    config.core.defaults.rules = (domain).rules().clone();
    config.core.seed_projects[0].policy.policy = policy;
    if environment_chat {
        config.core.owners = Box::new([jig_domain_people::InitialOwner {
            project: 1,
            identity: jig_domain_people::IdentityKey { provider: 0, subject: 8_u64.to_be_bytes().into() },
        }]);
    }
    if environment_chat {
        for template in &mut config.core.defaults.templates {
            if template.flags.conversation {
                template.contract = jig_domain_tasks::Contract::Report { words: limits.core.tasks.result_bytes };
                template.grace = Some(Duration::from_secs(1));
            }
        }
    }
    config.core.defaults.charters[0].hosts = jig_domain_fleet::Kinds::Engine;
    config.core.defaults.charters[0].policy.resume = true;
    config.core.settings.resume_bytes = 32768;
    config.core.defaults.charters[0].policy.waiting = Duration::ZERO;
    config.core.defaults.charters[0].policy.turns = 32;
    config.core.defaults.charters[0].policy.time = Duration::from_secs(7200);
    (config, limits)
}

pub fn root(seed: u64) -> (Config, Limits) {
    root_for(seed, false)
}

use jig_domain_authority as authority;
use jig_domain_tasks as tasks;
use skein_lib::Queue;

fn pattern(environment: Option<&[u8]>) -> authority::Pattern {
    let mut segments = vec![b"env".as_slice().into()];
    if let Some(environment) = environment {
        segments.push(environment.into());
        segments.push(b"service".as_slice().into());
    }
    authority::Pattern { segments: segments.into_boxed_slice(), last: authority::Last::Open(Box::new([])) }
}

/// Staging grants are the agent policy; the person additionally covers production.
#[must_use]
pub fn authority(spend: u64, person: bool) -> authority::Authority {
    let mut grants = Vec::new();
    for kind in [1, 2] {
        grants.push(authority::Grant {
            connector: jig_ops_domain_connectors::number(jig_ops_domain_connectors::Connector::Infrastructure),
            kind,
            pattern: pattern(if person { None } else { Some(b"staging") }),
        });
    }
    for kind in [2, 3, 4] {
        grants.push(authority::Grant {
            connector: jig_ops_domain_connectors::number(jig_ops_domain_connectors::Connector::Observability),
            kind,
            pattern: authority::Pattern { segments: Box::new([]), last: authority::Last::Open(Box::new([])) },
        });
    }
    authority::Authority {
        tools: authority::Tools(1023),
        models: authority::Models::ALL,
        grants: grants.into_boxed_slice(),
        delegation: authority::Delegation {
            kinds: Box::new([
                authority::Executor::Charter(1),
                authority::Executor::Procedure(1),
                authority::Executor::Procedure(2),
            ]),
            tasks: if person { 8 } else { 6 },
            depth: if person { 5 } else { 4 },
        },
        budget: authority::Budget { spend, deadline: None },
        notes: authority::Scopes(0),
        note_resources: Box::new([]),
    }
}

pub fn environment_authority(spend: u64, person: bool) -> authority::Authority {
    let mut value = authority(spend, person);
    let mut grants = value.grants.into_vec();
    grants.push(authority::Grant {
        connector: 2,
        kind: 4,
        pattern: authority::Pattern {
            segments: Box::new([b"pool".as_slice().into()]),
            last: authority::Last::Exact(b"staging".as_slice().into()),
        },
    });
    for kind in [4, 5] {
        grants.push(authority::Grant {
            connector: 2,
            kind,
            pattern: authority::Pattern {
                segments: Box::new([
                    b"pool".as_slice().into(),
                    b"staging".as_slice().into(),
                    b"environment".as_slice().into(),
                ]),
                last: authority::Last::Open(Box::new([])),
            },
        });
    }
    value.grants = grants.into();
    let mut kinds = value.delegation.kinds.into_vec();
    kinds.push(authority::Executor::Procedure(infrastructure::PROVISION_CODE));
    kinds.push(authority::Executor::Procedure(infrastructure::TEAR_DOWN_CODE));
    value.delegation.kinds = kinds.into();
    value
}

/// The rules' domain, empty of projects, and project 1's policy, which the first start seeds.
fn policy_domain(limits: &authority::Limits, environment_chat: bool) -> (authority::Domain, authority::Policy) {
    let requirements: Box<[_]> = [
        authority::Requirement {
            connector: jig_ops_domain_connectors::number(jig_ops_domain_connectors::Connector::Infrastructure),
            kind: 1,
            pattern: pattern(None),
            judge: authority::Judge {
                connector: jig_ops_domain_connectors::number(jig_ops_domain_connectors::Connector::Observability),
                requirement: 1,
                parameters: 0,
            },
            guard: authority::Guard::Observed { freshness: Duration::from_secs(30) },
            must_be_guarded: false,
        },
        authority::Requirement {
            connector: jig_ops_domain_connectors::number(jig_ops_domain_connectors::Connector::Infrastructure),
            kind: 2,
            pattern: pattern(None),
            judge: authority::Judge {
                connector: jig_ops_domain_connectors::number(jig_ops_domain_connectors::Connector::Observability),
                requirement: 2,
                parameters: 0,
            },
            guard: authority::Guard::Observed { freshness: Duration::from_secs(30) },
            must_be_guarded: false,
        },
    ]
    .into();
    let ceiling = if environment_chat { environment_authority(1000, true) } else { authority(1000, true) };
    let domain = authority::Domain::new(
        authority::Rules {
            ceiling: ceiling.clone(),
            period_spend: 1000,
            minimum_run_spend: 1,
            maximum_run_spend: 19,
            implies: authority::Implies::new(Box::new([]), 0).expect("empty implications"),
            requirements,
        },
        *limits,
    )
    .expect("bounded scenario policy");
    let mut scratch = authority::Domain::new(domain.rules().clone(), *limits).expect("bounded scenario policy");
    let policy = authority::Policy {
        projections: Box::new([]),
        escalation_role: Some(0),
        ceiling,
        period_spend: 1000,
        roles: Box::new([authority::Role {
            number: 0,
            authority: if environment_chat {
                let mut value = environment_authority(20, false);
                value.delegation.tasks += 1;
                value.delegation.depth += 1;
                value
            } else {
                let mut value = authority(100, false);
                value.delegation.tasks += 1;
                value.delegation.depth += 1;
                value
            },
            period_spend: 500,
            requests: authority::Requests::ALL,
            decides: authority::Proposals::ALL,
        }]),
        requirements: Box::new([]),
    };
    let mut out = Queue::with_capacity(authority::POLICY_MAX_OUT);
    authority::step(&mut scratch, authority::Event::Policy { project: 1, policy: policy.clone() }, &mut out);
    assert_eq!(out.pop(), Some(authority::PolicyFact::Added { project: 1 }), "a checked scenario policy");
    (domain, policy)
}

#[must_use]
pub fn task_authority(spend: u64) -> tasks::Authority {
    let value = authority(spend, false);
    tasks::Authority {
        tools: tasks::Tools(value.tools.0),
        models: tasks::Models::from_parts(value.models.bits(), value.models.is_all()),
        grants: value
            .grants
            .into_iter()
            .map(|grant| tasks::Grant {
                connector: grant.connector,
                kind: grant.kind,
                pattern: tasks::Pattern {
                    segments: grant.pattern.segments,
                    last: match grant.pattern.last {
                        authority::Last::Open(x) => tasks::Last::Open(x),
                        authority::Last::Exact(x) => tasks::Last::Exact(x),
                    },
                },
            })
            .collect(),
        delegation: tasks::Delegation {
            kinds: Box::new([
                tasks::AuthorityExecutor::Charter(1),
                tasks::AuthorityExecutor::Procedure(1),
                tasks::AuthorityExecutor::Procedure(2),
            ]),
            tasks: 6,
            depth: 4,
        },
        budget: tasks::Budget { spend, deadline: None },
        notes: tasks::Scopes(0),
        note_resources: Box::new([]),
    }
}

#[must_use]
pub fn chat(words: &[u8]) -> jig_domain_people::Ask {
    const CONFIG_SEED: u64 = 0;
    let (config, limits) = root(CONFIG_SEED);
    let members = jig_core_world::fixtures::Creating::new(&config.core, limits.core.tasks.tree_tasks);
    jig_core_world::fixtures::create_chat(&members, 1, 0, words.into())
}
