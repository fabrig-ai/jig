//! Unit fixtures use the core world's small configuration and child entry points.
#![expect(clippy::too_many_lines, reason = "tests state complete bounded fixtures")]
use crate::Settings;
use crate::documents::*;
use crate::*;
use skein_lib::{Duration, Env, ReplyTo, Time, Token, Wall};

#[test]
fn restored_application_tokens_cannot_exceed_the_deployments_allocations() {
    let (mut core, _) = make_tree();
    let connector = core.connectors[0];
    let token = fresh(&mut core.counters, Family::Application).expect("application token");
    let mut record = people::Stored::Answer {
        models: Box::new([]),
        key: people::RequestKey { person: 1, key: [1; 16] },
        ask: Box::new(people::Ask::Application {
            scope: jig_domain_people::ApplicationScope::Project(1),
            connector,
            digest: [1; 32],
        }),
        outcome: people::Outcome::Applied { connector, token: Token::new(token.checked_add(1).expect("next token")) },
        at: Wall::EPOCH,
    };
    assert!(!core.restore_people_row(&record));
    if let people::Stored::Answer { outcome, .. } = &mut record {
        *outcome = people::Outcome::Applied { connector, token: Token::new(token) };
    }
    assert!(core.restore_people_row(&record));
    if let people::Stored::Answer { outcome, .. } = &mut record {
        *outcome = people::Outcome::Applied { connector: u16::MAX, token: Token::new(token) };
    }
    assert!(!core.restore_people_row(&record));
}

pub(crate) fn limits() -> Limits {
    let retry = tasks::Retry { retries: 1, base: Duration::from_millis(10), max: Duration::from_secs(1) };
    let tasks = tasks::Limits {
        placed_holds: 1,
        project_placed_holds: 1,
        party_placed_holds: 1,
        exemptions: 1,
        circle: 8,
        tasks: 2,
        funders: 4,
        project_tasks: 2,
        project_goals: 8,
        tree_tasks: 2,
        depth: 1,
        delegates: 1,
        references: 2,
        subscriptions: 2,
        batch: 1,
        dependencies: 1,
        holdings: 2,
        hold_kinds: 2,
        resource_reports: 64,
        pools: 2,
        hold_segments: 4,
        hold_bytes: 128,
        hold_waiters: 2,
        grace: Duration::from_secs(600),
        hold_wait: Duration::from_secs(10),
        inputs: 1,
        spec_bytes: 64,
        parameters: 1,
        result_bytes: 128,
        run_inbox: 4,
        inbox_messages: 4,
        inbox_bytes: 192,
        message_bytes: 64,
        proposal_stall: Duration::from_millis(10),
        escalation_stall: Duration::from_secs(3600),
        person_stall: Duration::from_secs(3600),
        saved_resources: 2,
        contract_choices: 1,
        charters: 1,
        revisions: 4,
        versions: 8,
        authority_grants: 1,
        authority_segments: 1,
        authority_bytes: 32,
        executor_kinds: 2,
        retries: tasks::Retries {
            transient: retry,
            permanent: retry,
            run: retry,
            agent: retry,
            lost: retry,
            invalid: retry,
        },
        facts: 2,
        room_refusals: 2,
    };
    let people = people::Limits {
        operations: 8,
        batch: 2,
        people: 2,
        inbox_entries: 2,
        inbox_tasks: 16,
        inbox_recipients: 64,
        sign_ins: 2,
        // The configured project and the deployment's roles.
        projects: 2,
        holdings: 2,
        initial_owners: 1,
        requests: 4,
        pending: 2,
        waiters: 2,
        program_hosts: 65,
        program_host_name_bytes: 32,
        act_grants: 2,
        identity_bytes: 32,
        words: 64,
        amendment_bytes: 512,
        sign_in_lifetime: Duration::from_secs(60),
        request_retention: Duration::from_secs(120),
        room_refusals: 2,
        facts: 2,
    };
    let fleet = fleet::Limits {
        configured_workers: 64,
        worker_name_bytes: 32,
        program_sources: 12,
        source_detail_bytes: 4096,
        needs_bytes: 4096,
        versions: 0,
        workers: 1,
        engine_slots: 0,
        slots: 1,
        workstreams: 1,
        attempts: 2,
        grants: 8,
        calls: 1,
        call_name_bytes: 64,
        progress_bytes: 64,
        turns: 2,
        grace: Duration::from_secs(5),
        facts: 2,
        history: 2,
    };
    Limits {
        settings: settings::Limits {
            revisions: 4,
            versions: 8,
            history: 64,
            proposals: 4,
            alternatives: 16,
            permissions: 32,
        },
        templates: 8,
        fill_mark: 1000,
        documents: DocumentLimits {
            grants: 1,
            executors: 2,
            implications: 0,
            roles: authority_limits().roles,
            segments: 4,
            rows: 32,
            text_bytes: 256,
            chips: 32,
            lines: 32,
            versions: Versions { minimum: 1, maximum: 1 },
        },
        connectors: 2,
        resume_bytes: 256,
        run_bytes: 512,
        policy_bytes: 8192,
        escalation_reason_bytes: 128,
        load_slots: 2,
        load_bytes: 65_536,
        call_records: 2,
        call_answer_bytes: 256,
        tasks,
        authority: authority_limits(),
        people,
        fleet,
        brief: jig_domain_brief::Limits { briefs: 2, sections: 3, read_bytes: 256, brief_bytes: 384 },
        brief_parts: 2,
        brief_core_budgets: CoreBriefBudgets {
            tail: 128,
            task: 128,
            dependencies: 128,
            attempts: 128,
            plan: 128,
            notes: 64,
        },
        accounts: accounts::Limits {
            accounts: 1,
            holder_accounts: 1,
            program_names: 12,
            program_hosts: 65,
            sources: 3,
            sign_ins: 1,
            credentials: 2,
            backoff_base: Duration::from_secs(2),
            backoff_max: Duration::from_secs(8),
            retry: Duration::from_secs(30),
            move_deadline: Duration::from_secs(60),
            spent_attention: Duration::from_secs(1),
            facts: 2,
        },
        views: jig_domain_views::Limits {
            runs: 2,
            watchers: 4,
            watchers_per_party: 8,
            subjects_per_change: 64,
            text_bytes: 64,
            facts: 8,
        },
        notes: jig_domain_notes::Limits {
            scopes: 4,
            entries_per_scope: 1,
            pattern_bytes: 64,
            description_bytes: 64,
            body_bytes: 256,
            references: 1,
            load_rows: 1,
            lines: 1,
            recalled: 1,
        },
    }
}

// Existing Small-profile bounds (jig-protocol-config/src/profile/small.rs). The
// tiny unit limits cannot express a 16 KiB policy while keeping its shape valid.
pub(crate) const RESTORE_POLICY_BYTES: u64 = 16_384;
const RESTORE_GRANTS: u32 = 4;
const RESTORE_REQUIREMENTS: u32 = 4;
const RESTORE_SEGMENTS: u32 = 4;
const RESTORE_SEGMENT_BYTES: u32 = 256;
const RESTORE_NOTE_SCOPES: u32 = 8;
pub(crate) const RESTORE_PROJECT: u32 = 1;
const RESTORE_CONNECTOR: u16 = 1;

/// One policy of the requested owned size, under independent Small-profile
/// shape bounds. Full note scopes account for most bytes; requirement words
/// fill the rest without changing counts, references, authority or funding.
pub(crate) fn restoring_sized_policy(bytes: u64) -> (Core, Limits, ConfiguredPolicy) {
    let mut limits = limits();
    limits.policy_bytes = RESTORE_POLICY_BYTES;
    limits.authority.grants = RESTORE_GRANTS;
    limits.authority.requirements = RESTORE_REQUIREMENTS;
    limits.authority.segments = RESTORE_SEGMENTS;
    limits.authority.segment_bytes = RESTORE_SEGMENT_BYTES;
    limits.notes.scopes = RESTORE_NOTE_SCOPES;
    let mut config = config(41);
    let mut configured = config.seed_projects[0].policy.clone();
    configured.permissions = Box::new([]);
    let word = restore_policy_word(u64::from(RESTORE_SEGMENT_BYTES));
    let mut segments = List::with_capacity(RESTORE_SEGMENTS);
    for _ in 0..RESTORE_SEGMENTS {
        segments.push(word.clone()).expect("scope segment count");
    }
    let scope = authority::ResourceScope {
        connector: RESTORE_CONNECTOR,
        pattern: authority::Pattern { segments: segments.into_boxed(), last: authority::Last::Exact(word) },
    };
    let mut scopes = List::with_capacity(RESTORE_GRANTS);
    for _ in 0..RESTORE_GRANTS {
        scopes.push(scope.clone()).expect("scope count");
    }
    let scopes = scopes.into_boxed();
    config.defaults.rules.ceiling.note_resources = scopes.clone();
    configured.policy.ceiling.note_resources = scopes.clone();
    configured.policy.roles[0].authority.note_resources = scopes;
    let mut segments = List::with_capacity(RESTORE_SEGMENTS);
    for _ in 0..RESTORE_SEGMENTS {
        segments.push(Box::<[u8]>::from([])).expect("requirement segment count");
    }
    let requirement = authority::Requirement {
        connector: RESTORE_CONNECTOR,
        kind: 1,
        pattern: authority::Pattern { segments: segments.into_boxed(), last: authority::Last::Exact(Box::new([])) },
        judge: authority::Judge { connector: RESTORE_CONNECTOR, requirement: 1, parameters: 0 },
        guard: authority::Guard::Guarded,
        must_be_guarded: false,
    };
    let mut requirements = List::with_capacity(RESTORE_REQUIREMENTS);
    for _ in 0..RESTORE_REQUIREMENTS {
        requirements.push(requirement.clone()).expect("requirement count");
    }
    configured.policy.requirements = requirements.into_boxed();
    let snapshot = policy::snapshot(&configured.policy, &configured.permissions, &configured.templates)
        .expect("bounded policy snapshot");
    let base = people::policy_bytes(&snapshot).expect("bounded policy bytes");
    let mut remaining = bytes.checked_sub(base).expect("requested size holds the policy carriers and scopes");
    for requirement in &mut configured.policy.requirements {
        for segment in &mut requirement.pattern.segments {
            let count = remaining.min(u64::from(RESTORE_SEGMENT_BYTES));
            *segment = restore_policy_word(count);
            remaining = remaining.checked_sub(count).expect("remaining requirement bytes");
        }
        let count = remaining.min(u64::from(RESTORE_SEGMENT_BYTES));
        requirement.pattern.last = authority::Last::Exact(restore_policy_word(count));
        remaining = remaining.checked_sub(count).expect("remaining terminal bytes");
    }
    config.seed_projects = Box::new([]);
    let mut core = Core::new(config, &limits).expect("restoring core");
    core.restart_begin(false);
    let mut deployment = core.counters.deployment();
    deployment.projects = u64::from(RESTORE_PROJECT);
    deployment.commits = 1;
    core.restore_core(CoreRecord::Deployment(deployment), &limits);
    (core, limits, configured)
}

fn restore_policy_word(count: u64) -> Box<[u8]> {
    let mut word = List::with_capacity(u32::try_from(count).expect("bounded word size"));
    for _ in 0..count {
        word.push(b'x').expect("word byte count");
    }
    word.into_boxed()
}

fn authority_limits() -> authority::Limits {
    authority::Limits {
        projects: 1,
        roles: 1,
        requirements: 1,
        facts: 2,
        grants: 1,
        executors: 2,
        segments: 1,
        segment_bytes: 32,
        implications: 0,
        batch: 1,
        accounts: 3,
        writes: 1,
    }
}
fn authority_value(spend: u64, kinds: Box<[authority::Executor]>) -> authority::Authority {
    authority::Authority {
        tools: authority::Tools(1023),
        models: authority::Models::ALL,
        grants: Box::new([]),
        delegation: authority::Delegation { kinds, tasks: 3, depth: 2 },
        budget: authority::Budget { spend, deadline: None },
        notes: authority::Scopes(0),
        note_resources: Box::new([]),
    }
}
/// The one configured model, its charter's, which declares two efforts (tasks.md, 3).
fn scripted_model() -> Model {
    Model {
        dialect: 1,
        endpoint: 1,
        program: None,
        name: b"scripted".as_slice().into(),
        window: 8192,
        output: 1024,
        input_price: 1,
        cached_price: 1,
        cache_write_price: None,
        output_price: 1,
        price_unit: 1_000_000,
        efforts: Box::new([b"low".as_slice().into(), b"high".as_slice().into()]),
        effort: None,
    }
}

pub(crate) fn config(seed: u64) -> Config {
    let ceiling = authority_value(1000, Box::new([authority::Executor::Charter(1), authority::Executor::Procedure(1)]));
    let rules = authority::Rules {
        ceiling: ceiling.clone(),
        period_spend: 1000,
        minimum_run_spend: 1,
        maximum_run_spend: 100,
        implies: authority::Implies::new(Box::new([]), 0).expect("empty implication configuration"),
        requirements: Box::new([]),
    };
    let domain = authority::Domain::new(rules, authority_limits()).expect("valid authority configuration");
    let policy = authority::Policy {
        projections: Box::new([]),
        escalation_role: Some(0),
        ceiling,
        period_spend: 1000,
        roles: Box::new([authority::Role {
            number: 0,
            authority: {
                let mut given = authority_value(
                    500,
                    Box::new([authority::Executor::Charter(1), authority::Executor::Procedure(1)]),
                );
                given.delegation.tasks = 2;
                given.delegation.depth = 2;
                given
            },
            period_spend: 500,
            requests: authority::Requests::ALL,
            decides: authority::Proposals::ALL,
        }]),
        requirements: Box::new([]),
    };
    Config {
        worker_names: Box::new([b"alpha".as_slice().into(), b"beta".as_slice().into()]),
        deployment: [31; 16],
        seed,
        workers_in_process: false,
        hub_turns: 2,
        grants: Box::new([]),
        owners: Box::new([people::InitialOwner {
            project: 1,
            identity: people::IdentityKey { provider: 0, subject: 7_u64.to_be_bytes().into() },
        }]),
        seed_projects: Box::new([SeedProject {
            number: 1,
            name: Box::from("Project 1"),
            policy: ConfiguredPolicy {
                policy,
                permissions: Box::new([
                    people::PermissionRole { connector: 1, permission: 3, role: 1 },
                    people::PermissionRole { connector: 1, permission: 2, role: 2 },
                    people::PermissionRole { connector: 1, permission: 1, role: 3 },
                ]),
                templates: Box::new([]),
            },
        }]),
        connectors: Box::new([1, 2]),
        settings: Settings {
            deployment_provider: 3,
            models: Box::new([scripted_model()]),
            resume_bytes: 256,
            tail_bytes: 128,
            tools: ToolFamilies::JIG,
            key_bytes: 256,
            redirect_bytes: 4096,
            llm_providers: accounts::Providers {
                providers: Box::new([accounts::Provider {
                    name: b"Fake".as_slice().into(),
                    signed_in_plan: true,
                    manage_usage: Some(b"https://usage.example/".as_slice().into()),
                    number: 1,
                    models: Box::new([b"scripted".as_slice().into()]),
                    offers: accounts::Offers { sign_in: true, key: false, programs: Box::new([]) },
                }]),
            },
        },
        defaults: settings::Defaults {
            charters: Box::new([Charter {
                number: 1,
                hosts: fleet::Kinds::Workers,
                spend: 100,
                policy: RunPolicy {
                    instructions: b"Complete the task within its authority.".as_slice().into(),
                    waiting: Duration::from_secs(1),
                    resume: true,
                    turns: 8,
                    time: Duration::from_secs(60),
                    reserve_turns: 1,
                    reserve_time: Duration::from_secs(10),
                    model: scripted_model(),
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
            rules: (domain).rules().clone(),
        },
    }
}

fn restore_key_account(core: &mut Core, limits: &Limits, number: u32, source: u32, provider: u32) {
    let mut out = Queue::with_capacity(accounts::max_out(&limits.accounts));
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.accounts };
    for record in [
        accounts::Stored::Account {
            number,
            provider,
            holder: accounts::Holder::Deployment,
            place: 0,
            uses: None,
            subject: Box::new([]),
            display: Box::new([]),
        },
        accounts::Stored::Source {
            account: number,
            number: source,
            place: 0,
            kind: accounts::Kind::Key,
            name: Box::new([]),
            held: None,
        },
    ] {
        accounts::step(&mut core.accounts, &env, accounts::Event::Restore { record }, &mut out);
    }
}

fn clear_projects(core: &mut Core, limits: &Limits) {
    core.authority = authority::Domain::new(core.authority.rules().clone(), limits.authority).expect("restore rules");
    core.projects.clear();
    core.names = Map::with_capacity(limits.authority.projects);
    core.permission_roles = Map::with_capacity(limits.authority.projects);
    core.template_overrides = Map::with_capacity(limits.authority.projects);
}

fn seeded_core(config: Config, limits: &Limits) -> Result<Core, Problem> {
    let mut core = Core::new(config, limits)?;
    routing::first_start::seed_projects(&mut core);
    Ok(core)
}

pub(crate) fn core() -> (Core, Limits) {
    let limits = limits();
    let mut core = seeded_core(config(41), &limits).expect("fixture core configuration");
    core.counters.deployment.projects = 1;
    restore_key_account(&mut core, &limits, 0, 1, 1);
    let mut out = Queue::with_capacity(people::max_out(&limits.people));
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.people };
    people::step(
        &mut core.people,
        &env,
        people::Event::Restore {
            record: people::Stored::Person {
                number: 1,
                identity: people::Identity {
                    key: people::IdentityKey { provider: 0, subject: Box::new([1]) },
                    login: b"owner".as_slice().into(),
                    name: b"A Person".as_slice().into(),
                },
            },
        },
        &mut out,
    );
    people::step(
        &mut core.people,
        &env,
        people::Event::Restore {
            record: people::Stored::Roles {
                project: 1,
                holdings: Box::new([people::Holding { person: 1, role: people::Role::Owner }]),
            },
        },
        &mut out,
    );
    people::step(&mut core.people, &env, people::Event::Restored, &mut out);
    (core, limits)
}

pub(crate) fn make_task(core: &mut Core, limits: &Limits, number: u64) {
    make_task_ended_by(core, limits, number, tasks::EndedBy::Result);
}
fn make_task_ended_by(core: &mut Core, limits: &Limits, number: u64, ended_by: tasks::EndedBy) {
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.tasks };
    let mut out = Queue::with_capacity(settings::task_room(limits));
    tasks::step(&mut core.tasks, &env, tasks::Event::Check, &mut out);
    tasks::step(&mut core.tasks, &env, tasks::Event::Restored, &mut out);
    while out.pop().is_some() {}
    tasks::step(
        &mut core.tasks,
        &env,
        tasks::Event::OpenPeriod { reply_to: ReplyTo::new(Token::new(1)), project: 1, period: 1, budget: 1000 },
        &mut out,
    );
    while out.pop().is_some() {}
    let mut given = core.creating_authority(1, 1, people::Role::Owner, u32::MAX);
    given.budget.spend = 100;
    let row = tasks::New {
        deadline: jig_domain_tasks::Deadline::Named,
        label: 0,
        flags: tasks::Flags { ended_by, conversation: true, ..tasks::Flags::PLAIN },
        number,
        project: 1,
        executor: tasks::Executor::Agent { charter: 1 },
        llm: None,
        spec: tasks::Spec {
            words: b"A task title\nThe specification".as_slice().into(),
            parameters: Box::new([]),
            inputs: Box::new([]),
        },
        contract: tasks::Contract::Report { words: 64 },
        authority: task_authority(&given),
        numbers: tasks::Numbers { budget: 100, spent: 0, spent_below: 0, reserved: 0 },
        funder: tasks::Funder::Period { project: 1, period: 1 },
        dependencies: Box::new([]),
        holdings: Box::new([]),
        wake: tasks::WakePolicy::DEFAULT,
        recurring: None,
        tracked: None,
    };
    tasks::step(
        &mut core.tasks,
        &env,
        tasks::Event::Make {
            reply_to: ReplyTo::new(Token::new(1)),
            creator: tasks::Party::Person(1),
            batch: Box::new([row]),
        },
        &mut out,
    );
    assert!(core.tasks.task(number).is_some());
}

fn window() -> Window {
    Window { rows: 1, after: None }
}

#[test]
fn documents_frame_carries_roles_retention_and_configured_charters() {
    let (core, limits) = core();
    let Document::Frame(frame) = make(&core, &limits, 1, &Subject::Frame { party: 1 }, &window(), Wall::EPOCH).unwrap()
    else {
        panic!()
    };
    assert_eq!(frame.party, 1);
    assert_eq!(frame.kind, people::Kind::Person);
    assert_eq!(
        frame.projects.as_slice(),
        &[FrameProjectRole {
            project: 1,
            role: people::Role::Owner,
            name: Text { bytes: Box::from(&b"Project 1"[..]), cut: false },
            pool: None,
            creating: Some(crate::acts::Creating {
                within: frame_facts::owner_creating(limits.documents.lines, 1, 1),
                beyond: crate::acts::Beyond::Proposed {
                    to: crate::acts::Recipient::Roles {
                        project: 1,
                        roles: {
                            let mut roles = List::with_capacity(limits.authority.roles);
                            roles.push(0).unwrap();
                            roles
                        }
                    }
                },
            }),
            models: authority::Models::ALL,
            templates: List::with_capacity(0),
        }]
    );
    assert_eq!(frame.retention, limits.people.request_retention);
    assert_eq!(frame.needs, 0);
    assert_eq!(frame.updates, Count::Known(0));
    let mut upgraded = limits;
    upgraded.documents.versions = Versions { minimum: u32::MAX, maximum: u32::MAX };
    assert_eq!(
        make(&core, &upgraded, 1, &Subject::Frame { party: 1 }, &window(), Wall::EPOCH).unwrap(),
        Document::Frame(frame),
        "protocol served versions never change the business Frame"
    );
}

#[test]
fn documents_chip_cuts_the_title_and_keeps_its_phase_facts() {
    let (mut core, mut limits) = core();
    make_task(&mut core, &limits, 1);
    limits.documents.text_bytes = 6;
    let mut tasks = List::with_capacity(1);
    tasks.push(1).unwrap();
    let Document::Chips(chips) = make(&core, &limits, 1, &Subject::Chips { tasks }, &window(), Wall::EPOCH).unwrap()
    else {
        panic!()
    };
    let chip = &chips.as_slice()[0];
    assert_eq!(chip.title, Text { bytes: b"A task".as_slice().into(), cut: true });
    assert_eq!(chip.executor, ExecutorKind::Agent);
    assert_eq!(chip.phase, PhaseLine::Active(tasks::Active::Due));
}

#[test]
fn documents_refuse_a_party_without_a_project_role() {
    let (mut core, limits) = core();
    make_task(&mut core, &limits, 1);
    let mut out = Queue::with_capacity(people::max_out(&limits.people));
    people::step(
        &mut core.people,
        &Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.people },
        people::Event::Roles { project: 1, holdings: Box::new([]) },
        &mut out,
    );
    let mut tasks = List::with_capacity(1);
    tasks.push(1).unwrap();
    assert_eq!(make(&core, &limits, 1, &Subject::Chips { tasks }, &window(), Wall::EPOCH), Err(Missing::NotAllowed));
    assert_eq!(make(&core, &limits, 1, &Subject::Frame { party: 2 }, &window(), Wall::EPOCH), Err(Missing::NotAllowed));
}

#[test]
fn documents_run_has_only_the_live_proofs_committed_progress() {
    let (mut core, limits) = core();
    make_task(&mut core, &limits, 1);
    core.proofs
        .insert(
            1,
            RunProof {
                accounts: Box::new([]),
                placement: None,
                account_failure: None,
                account_terminal: false,
                task: 1,
                attempt: 7,
                conversation: 1,
                credentials: Box::new([]),
                window: None,
                crossed: false,
                host: fleet::HostKind::Engine,
                offered: None,
                outstanding: Box::new([]),
                turn: Some(TurnProof { turn: 3, cumulative: 0, read: None }),
                terminal: None,
            },
        )
        .unwrap();
    assert_eq!(
        make(&core, &limits, 1, &Subject::Run { task: 1 }, &window(), Wall::EPOCH),
        Ok(Document::Run(Run { task: 1, attempt: Some(7), committed_turn: 3, writing: None, call: None }))
    );
    core.proofs.remove(&1);
    assert_eq!(
        make(&core, &limits, 1, &Subject::Run { task: 1 }, &window(), Wall::EPOCH),
        Ok(Document::Run(Run { task: 1, attempt: None, committed_turn: 0, writing: None, call: None }))
    );
}

#[test]
fn documents_clamp_oversized_windows_and_reject_invalid_bounds() {
    let (core, mut limits) = make_tree();
    let subject = Subject::Tree { task: 1 };
    assert_eq!(limits.documents.rows, 32);
    let Document::Tree(tree) =
        make(&core, &limits, 1, &subject, &Window { rows: 33, after: None }, Wall::EPOCH).unwrap()
    else {
        panic!("tree document")
    };
    assert_eq!(tree.root.number, 1);
    assert_eq!(tree.rows.len(), 3);
    assert_eq!(tree.rows.get(2).unwrap().chip.number, 4);
    assert_eq!(tree.next, None);
    limits.documents.rows = 2;
    let Document::Tree(tree) =
        make(&core, &limits, 1, &subject, &Window { rows: 33, after: None }, Wall::EPOCH).unwrap()
    else {
        panic!("clamped tree document")
    };
    assert_eq!(tree.root.number, 1);
    assert_eq!(tree.rows.len(), 2);
    assert_eq!(tree.rows.get(1).unwrap().chip.number, 3);
    assert_eq!(tree.next, Some(Cursor::Number { number: 3 }));
    let Document::Tree(tree) =
        make(&core, &limits, 1, &subject, &Window { rows: 33, after: Some(Cursor::Number { number: 2 }) }, Wall::EPOCH)
            .unwrap()
    else {
        panic!("clamped continuation")
    };
    assert_eq!(tree.rows.len(), 2);
    assert_eq!(tree.rows.get(0).unwrap().chip.number, 3);
    assert_eq!(tree.rows.get(1).unwrap().chip.number, 4);
    assert_eq!(tree.next, None);
    assert!(document_worst_case(&limits.documents).is_some());
    limits.documents.lines = 0;
    assert_eq!(document_worst_case(&limits.documents), None);
    assert_eq!(make(&core, &limits, 1, &subject, &window(), Wall::EPOCH), Err(Missing::TooLarge));
    limits.documents.lines = u32::MAX;
    limits.documents.chips = u32::MAX;
    assert_eq!(document_worst_case(&limits.documents), None);
}

fn tree_core() -> (Core, Limits) {
    let (old, mut limits) = core();
    limits.views.watchers = 8;
    limits.views.watchers_per_party = 8;
    limits.tasks.tasks = 16;
    limits.people.inbox_tasks = 32;
    limits.tasks.project_tasks = 16;
    limits.tasks.tree_tasks = 16;
    limits.tasks.depth = 4;
    limits.tasks.delegates = 8;
    limits.tasks.batch = 8;
    limits.tasks.inbox_messages = 16;
    limits.tasks.inbox_bytes = 2048;
    let mut config = config(17);
    let mut given = config.seed_projects[0].policy.policy.roles[0].authority.clone();
    given.delegation.tasks = 17;
    given.delegation.depth = 5;
    set_seed_role_authority(&mut config, given);
    let mut core = seeded_core(config, &limits).expect("fixture core configuration");
    core.counters.deployment.projects = 1;
    core.people = old.people;
    core.accounts = old.accounts;
    send_task(&mut core, &limits, tasks::Event::Check);
    send_task(&mut core, &limits, tasks::Event::Restored);
    send_task(
        &mut core,
        &limits,
        tasks::Event::OpenPeriod { reply_to: ReplyTo::new(Token::new(1)), project: 1, period: 1, budget: 1000 },
    );
    (core, limits)
}

fn send_task(core: &mut Core, limits: &Limits, event: tasks::Event) {
    let mut out = Queue::with_capacity(settings::task_room(limits));
    tasks::step(&mut core.tasks, &Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.tasks }, event, &mut out);
    while let Some(request) = out.pop() {
        if let tasks::Request::Save { record: tasks::Stored::Activity(line) } = &request {
            let _retained = core.remember_question(line);
        }
        if let tasks::Request::Save { record: tasks::Stored::Live(row) } = request {
            core.prune_questions(row.number);
            let mut projections = Queue::with_capacity(room_max(limits).unwrap().writes);
            projections::save(core, limits, &row, &mut projections);
        }
    }
}

fn new_row(core: &Core, number: u64, funder: tasks::Funder, tracked: Option<u32>) -> tasks::New {
    let mut authority = task_authority(&core.creating_authority(1, 1, people::Role::Owner, u32::MAX));
    let budget = match funder {
        tasks::Funder::Task(_) => 10,
        tasks::Funder::Pool { .. } | tasks::Funder::Recurring { .. } | tasks::Funder::Period { .. } => 100,
    };
    authority.budget.spend = budget;
    tasks::New {
        deadline: jig_domain_tasks::Deadline::Named,
        label: 0,
        flags: jig_domain_tasks::Flags::PLAIN,
        number,
        project: 1,
        executor: tasks::Executor::Agent { charter: 1 },
        llm: None,
        spec: tasks::Spec {
            words: b"Work\nDetailed instructions".as_slice().into(),
            parameters: Box::new([]),
            inputs: Box::new([]),
        },
        contract: tasks::Contract::Report { words: 64 },
        authority,
        numbers: tasks::Numbers { budget, spent: 0, spent_below: 0, reserved: 0 },
        funder,
        dependencies: Box::new([]),
        holdings: Box::new([]),
        wake: tasks::WakePolicy::DEFAULT,
        recurring: None,
        tracked,
    }
}

fn make_tree() -> (Core, Limits) {
    let (mut core, limits) = tree_core();
    let root = new_row(&core, 1, tasks::Funder::Period { project: 1, period: 1 }, Some(4));
    send_task(
        &mut core,
        &limits,
        tasks::Event::Make {
            reply_to: ReplyTo::new(Token::new(1)),
            creator: tasks::Party::Person(1),
            batch: Box::new([root]),
        },
    );
    let children = [
        new_row(&core, 2, tasks::Funder::Task(1), None),
        new_row(&core, 3, tasks::Funder::Task(1), None),
        new_row(&core, 4, tasks::Funder::Task(1), None),
    ];
    send_task(
        &mut core,
        &limits,
        tasks::Event::Make {
            reply_to: ReplyTo::new(Token::new(1)),
            creator: tasks::Party::Task(1),
            batch: Box::new(children),
        },
    );
    assert!(core.tasks.task(4).is_some());
    send_task(&mut core, &limits, tasks::Event::Hold { task: 3, why: tasks::Hold::Budget });
    (core, limits)
}

#[test]
fn amendment_acceptance_reserves_its_terminal_proposal_revision() {
    let (mut core, limits) = make_tree();
    let original = core.tasks.task(3).unwrap().clone();
    let mut source = core.tasks.task(1).unwrap().clone();
    let mut target = original.clone();
    target.revision = u64::MAX - 2;
    target.escalation = tasks::Escalation::Waiting {
        revision: target.revisions.hold,
        holder: tasks::EscalationHolder::Task(1),
        entry: 1,
        since: Wall::EPOCH,
    };
    source.revision = u64::MAX;
    core.tasks = fixtures::fresh_tasks(&core, &limits);
    send_task(&mut core, &limits, tasks::Event::Restore { record: tasks::Stored::Live(Box::new(source)) });
    send_task(&mut core, &limits, tasks::Event::Restore { record: tasks::Stored::Live(Box::new(target.clone())) });
    let arithmetic = Err(people::Refusal::Limit { which: people::LimitKind::Arithmetic });
    assert_eq!(core.amendment_accept_revision(3, 3, Some(target.revisions.hold)), arithmetic);
    assert_eq!(core.amendment_accept_revision(3, 3, None), Ok(()));
    assert_eq!(core.amendment_accept_revision(1, 3, Some(target.revisions.hold)), arithmetic);
    assert_eq!(core.tasks.task(3), Some(&target));
    assert_eq!(core.tasks.task(3).unwrap().spec, original.spec);
    target.revision = u64::MAX - 3;
    core.tasks = fixtures::fresh_tasks(&core, &limits);
    send_task(&mut core, &limits, tasks::Event::Restore { record: tasks::Stored::Live(Box::new(target.clone())) });
    assert_eq!(core.amendment_accept_revision(3, 3, Some(target.revisions.hold)), Ok(()));
    assert_eq!(core.tasks.task(3), Some(&target));
}

#[test]
fn documents_tree_folds_every_unshown_phase_and_keeps_held_counts() {
    let (core, limits) = make_tree();
    let Document::Tree(tree) = make(&core, &limits, 1, &Subject::Tree { task: 1 }, &window(), Wall::EPOCH).unwrap()
    else {
        panic!()
    };
    assert_eq!(tree.rows.len(), 1);
    assert_eq!(tree.rows.get(0).unwrap().chip.number, 2);
    assert_eq!(tree.held_below, 1);
    assert_eq!(tree.folded.get(0).unwrap().held_below, 1);
    assert_eq!(tree.folded.get(0).unwrap().phases.held, 1);
    assert_eq!(tree.folded.get(0).unwrap().phases.starting, 1);
    assert_eq!(tree.next, Some(Cursor::Number { number: 2 }));
    let Document::Tree(next) =
        make(&core, &limits, 1, &Subject::Tree { task: 1 }, &Window { rows: 2, after: tree.next }, Wall::EPOCH)
            .unwrap()
    else {
        panic!()
    };
    assert_eq!(next.rows.get(0).unwrap().chip.number, 3);
    assert_eq!(next.rows.get(1).unwrap().chip.number, 4);
    assert_eq!(next.next, None);
    assert_eq!(next.held_below, 1);
}

#[test]
fn documents_task_carries_overview_lineage_and_the_hold_reason() {
    let (core, limits) = make_tree();
    let Document::Task(task) = make(&core, &limits, 1, &Subject::Task { task: 3 }, &window(), Wall::EPOCH).unwrap()
    else {
        panic!()
    };
    assert_eq!(task.words.bytes.as_ref(), b"Work\nDetailed instructions");
    assert_eq!(task.goal, Some(1));
    assert_eq!(task.lineage.get(0).unwrap().number, 1);
    assert_eq!(task.budget.left, 10);
    assert_eq!(task.budget.funder, tasks::Funder::Task(1));
    let hold = task.hold.unwrap();
    assert_eq!(hold.reason, tasks::Hold::Budget);
    assert_eq!(hold.revision, core.tasks.task(3).unwrap().revisions.hold);
    assert_eq!(hold.before, PriorPhase::Active(tasks::Active::Due));
}

#[test]
fn documents_part_keys_are_preserved_or_refused_whole() {
    let (mut core, mut limits) = tree_core();
    let mut row = new_row(&core, 1, tasks::Funder::Period { project: 1, period: 1 }, None);
    row.authority.grants = Box::new([tasks::Grant {
        connector: 1,
        kind: 1,
        pattern: tasks::Pattern {
            segments: Box::new([b"whole".as_slice().into()]),
            last: tasks::Last::Exact(b"end".as_slice().into()),
        },
    }]);
    send_task(
        &mut core,
        &limits,
        tasks::Event::Make {
            reply_to: ReplyTo::new(Token::new(1)),
            creator: tasks::Party::Person(1),
            batch: Box::new([row]),
        },
    );
    limits.documents.text_bytes = 3;
    assert_eq!(make(&core, &limits, 1, &Subject::Task { task: 1 }, &window(), Wall::EPOCH), Err(Missing::TooLarge));
    limits.documents.text_bytes = 5;
    let Document::Task(task) = make(&core, &limits, 1, &Subject::Task { task: 1 }, &window(), Wall::EPOCH).unwrap()
    else {
        panic!()
    };
    assert_eq!(task.authority.grants.get(0).unwrap().segments.get(0).unwrap().as_ref(), b"whole");
    assert_eq!(task.authority.grants.get(0).unwrap().last, PatternEnd::Exact(b"end".as_slice().into()));
    assert!(document_worst_case(&limits.documents).is_some());
    assert!(worst_case(&limits).is_some());
}

#[test]
fn documents_proposal_tracks_the_current_holder_and_funding_after_a_pass() {
    let (mut core, limits) = make_tree();
    let next = new_row(&core, 5, tasks::Funder::Task(1), None);
    send_task(
        &mut core,
        &limits,
        tasks::Event::Propose {
            reply_to: ReplyTo::new(Token::new(50)),
            proposal: tasks::Proposal {
                number: 10,
                proposer: 2,
                project: 1,
                action: tasks::ProposalAction::Batch(Box::new([next])),
                reason: Box::from(&b"More room"[..]),
                as_holder: true,
                state: tasks::ProposalState::Pending { holder: tasks::ProposalHolder::Task(1), since: Wall::EPOCH },
            },
        },
    );
    let target = Subject::Proposal { proposer: Proposer::Task(2), number: 10 };
    let Document::Proposal(proposal) = make(&core, &limits, 1, &target, &window(), Wall::EPOCH).unwrap() else {
        panic!()
    };
    assert_eq!(proposal.reason.bytes.as_ref(), b"More room");
    assert_eq!(proposal.waiting.holder, tasks::ProposalHolder::Task(1));
    assert_eq!(proposal.funding.get(0).unwrap().source, tasks::Funder::Task(1));
    assert_eq!(proposal.funding.get(0).unwrap().needed, 10);
    assert_eq!(proposal.funding.get(0).unwrap().left, Some(70));
    assert_eq!(proposal.funding.get(0).unwrap().after, Some(60));
    let ProposalAction::Batch(batch) = proposal.action else { panic!() };
    assert_eq!(batch.get(0).unwrap().number, 5);
    assert_eq!(batch.get(0).unwrap().authority.ceiling.spend, 10);
    send_task(
        &mut core,
        &limits,
        tasks::Event::DecideProposal {
            reply_to: ReplyTo::new(Token::new(51)),
            proposer: 2,
            proposal: 10,
            message: None,
            by: tasks::Party::Task(1),
            decision: tasks::ProposalDecision::Pass { holder: tasks::ProposalHolder::Person(1) },
        },
    );
    let Document::Proposal(proposal) = make(&core, &limits, 1, &target, &window(), Wall::EPOCH).unwrap() else {
        panic!()
    };
    assert_eq!(proposal.waiting.holder, tasks::ProposalHolder::Person(1));
    send_task(
        &mut core,
        &limits,
        tasks::Event::WithdrawProposal { reply_to: ReplyTo::new(Token::new(52)), proposer: 2, proposal: 10 },
    );
    assert_eq!(make(&core, &limits, 1, &target, &window(), Wall::EPOCH), Err(Missing::Ended));
    assert!(core.waiting_entries(&limits, core.tasks.task(1).unwrap()).is_empty());
}

#[test]
fn person_address_proposals_keep_the_named_revision_and_actual_beforefacts() {
    let (mut core, limits) = tree_core();
    let root = new_row(&core, 1, tasks::Funder::Period { project: 1, period: 1 }, None);
    send_task(
        &mut core,
        &limits,
        tasks::Event::Make {
            reply_to: ReplyTo::new(Token::new(1)),
            creator: tasks::Party::Person(1),
            batch: Box::new([root]),
        },
    );
    let mut child = new_row(&core, 2, tasks::Funder::Task(1), None);
    child.executor = tasks::Executor::Person(tasks::PersonAddress::Role(u32::MAX));
    send_task(
        &mut core,
        &limits,
        tasks::Event::Make {
            reply_to: ReplyTo::new(Token::new(2)),
            creator: tasks::Party::Task(1),
            batch: Box::new([child]),
        },
    );
    send_task(
        &mut core,
        &limits,
        tasks::Event::TakePerson { reply_to: ReplyTo::new(Token::new(3)), task: 2, person: 1 },
    );
    let named = tasks::AddressChange { revision: 0, address: tasks::PersonAddress::Person(2) };
    let mut wider = core.tasks.task(2).unwrap().authority.clone();
    wider.budget.spend = 15;
    let edit = tasks::Amendment {
        tracked: None,
        addressed: Some(named),
        instructions: None,
        release: None,
        spec: None,
        wake: None,
        dependencies: None,
        authority: Some(wider),
        llm: None,
        reason: b"new address".as_slice().into(),
    };
    let action = tasks::ProposalAction::Amend { task: 2, revision: 0, amendment: edit.clone() };
    send_task(
        &mut core,
        &limits,
        tasks::Event::Propose {
            reply_to: ReplyTo::new(Token::new(4)),
            proposal: tasks::Proposal {
                number: 20,
                proposer: 1,
                project: 1,
                action: action.clone(),
                reason: b"Reason".as_slice().into(),
                as_holder: false,
                state: tasks::ProposalState::Pending { holder: tasks::ProposalHolder::Person(1), since: Wall::EPOCH },
            },
        },
    );
    let Document::Proposal(doc) =
        make(&core, &limits, 1, &Subject::Proposal { proposer: Proposer::Task(1), number: 20 }, &window(), Wall::EPOCH)
            .unwrap()
    else {
        panic!("proposal")
    };
    let ProposalAction::Amend { addressed, before, after, .. } = doc.action else { panic!("amendment") };
    assert_eq!(addressed, Some(named));
    assert_eq!(before.addressed, Some(tasks::PersonAddress::Role(u32::MAX)));
    assert_eq!(before.taken_by, Some(1));
    assert_eq!(after.addressed, Some(tasks::PersonAddress::Person(2)));
    assert_eq!(after.taken_by, None);
    let RetainedAction::Amend { change, .. } = retained_action(&limits, 20, &action).unwrap() else {
        panic!("retained amendment")
    };
    assert_eq!(change.addressed, Some(named));
    let mut moved = edit.clone();
    moved.authority = None;
    moved.addressed = Some(tasks::AddressChange { revision: 0, address: tasks::PersonAddress::Role(5) });
    send_task(
        &mut core,
        &limits,
        tasks::Event::Amend {
            places: Box::new([]),
            funder: None,
            reply_to: ReplyTo::new(Token::new(5)),
            by: tasks::Party::Person(1),
            task: 2,
            message: 21,
            stop_run: false,
            amendment: moved,
        },
    );
    let current = core.tasks.task(2).unwrap();
    assert_eq!(current.revisions.person, 1);
    assert_eq!(tasks::address_admit(current, &edit.addressed), Err(tasks::Refusal::PersonRevision { revision: 1 }));
    let Document::Proposal(doc) =
        make(&core, &limits, 1, &Subject::Proposal { proposer: Proposer::Task(1), number: 20 }, &window(), Wall::EPOCH)
            .unwrap()
    else {
        panic!("pending proposal")
    };
    let ProposalAction::Amend { addressed, before, after, .. } = doc.action else { panic!("amendment") };
    assert_eq!(addressed, Some(named), "the immutable named revision is not refreshed");
    assert_eq!(before.addressed, Some(tasks::PersonAddress::Role(5)), "beforefacts are actual current owner facts");
    assert_eq!(before.taken_by, None);
    assert_eq!(after.addressed, Some(tasks::PersonAddress::Person(2)));
}

#[test]
fn documents_proposal_actions_keep_effect_parts_and_amendment_before_and_after() {
    let (mut core, limits) = make_tree();
    let mut changed = core.tasks.task(2).unwrap().spec.clone();
    changed.words = Box::from(&b"New question"[..]);
    let mut wider = core.tasks.task(2).unwrap().authority.clone();
    wider.budget.spend = 15;
    let actions = [
        tasks::ProposalAction::Effect {
            connector: 2,
            authority: wider.clone(),
            attempt: 7,
            completion: 8,
            position: 9,
            purpose: 11,
        },
        tasks::ProposalAction::ProcedureEffect { connector: 2, authority: wider.clone(), step: 13, purpose: 17 },
        tasks::ProposalAction::Amend {
            task: 2,
            revision: 0,
            amendment: tasks::Amendment {
                tracked: None,
                addressed: None,
                release: Some(1),
                instructions: Some(b"Use the new evidence".as_slice().into()),
                spec: Some(changed),
                wake: None,
                dependencies: None,
                authority: Some(wider.clone()),
                llm: None,
                reason: Box::from(&b"Reconsider"[..]),
            },
        },
        tasks::ProposalAction::Widen { task: 2, authority: wider },
        tasks::ProposalAction::Release { task: 3 },
    ];
    for (offset, action) in actions.into_iter().enumerate() {
        let number = u64::try_from(offset).unwrap() + 20;
        send_task(
            &mut core,
            &limits,
            tasks::Event::Propose {
                reply_to: ReplyTo::new(Token::new(number)),
                proposal: tasks::Proposal {
                    number,
                    proposer: 2,
                    project: 1,
                    action,
                    reason: Box::from(&b"Reason"[..]),
                    as_holder: false,
                    state: tasks::ProposalState::Pending { holder: tasks::ProposalHolder::Task(1), since: Wall::EPOCH },
                },
            },
        );
        let Document::Proposal(proposal) =
            make(&core, &limits, 1, &Subject::Proposal { proposer: Proposer::Task(2), number }, &window(), Wall::EPOCH)
                .unwrap()
        else {
            panic!()
        };
        match proposal.action {
            ProposalAction::Effect { part, purpose, .. } => {
                assert_eq!(part.connector, 2);
                assert_eq!(part.key, PartKey::Number(number));
                assert_eq!(purpose, 11);
                assert!(proposal.funding.is_empty());
            }
            ProposalAction::ProcedureEffect { part, authority, step, purpose } => {
                assert_eq!(part.connector, 2);
                assert_eq!(part.kind, PartKind::Effect);
                assert_eq!(part.key, PartKey::Number(number));
                assert_eq!(step, 13);
                assert_eq!(purpose, 17);
                assert_eq!(authority.ceiling.spend, 15);
                assert!(proposal.funding.is_empty());
            }
            ProposalAction::Amend { before, after, release, .. } => {
                assert_eq!(release, Some(1));
                assert_eq!(
                    before.instructions.as_ref().unwrap().bytes,
                    core.charter(1).expect("chat charter").policy.instructions
                );
                assert_eq!(after.instructions.as_ref().unwrap().bytes.as_ref(), b"Use the new evidence");
                assert_eq!(before.spec.words.bytes.as_ref(), b"Work\nDetailed instructions");
                assert_eq!(after.spec.words.bytes.as_ref(), b"New question");
                assert_eq!(after.authority.ceiling.spend, 15);
                assert_eq!(proposal.funding.get(0).unwrap().needed, 5);
            }
            ProposalAction::Widen { before, after, .. } => {
                assert_eq!(before.ceiling.spend, 10);
                assert_eq!(after.ceiling.spend, 15);
            }
            ProposalAction::Release { hold, .. } => assert_eq!(hold.unwrap().reason, tasks::Hold::Budget),
            ProposalAction::Settings { .. } | ProposalAction::Batch(_) => panic!(),
        }
        send_task(
            &mut core,
            &limits,
            tasks::Event::WithdrawProposal { reply_to: ReplyTo::new(Token::new(90)), proposer: 2, proposal: number },
        );
    }
}

#[test]
fn documents_every_escalation_reason_keeps_the_task_story_and_independent_revision() {
    let reasons = [
        tasks::Hold::Failures(tasks::Class::Lost),
        tasks::Hold::Stalled,
        tasks::Hold::Dependency(2),
        tasks::Hold::Stopped,
        tasks::Hold::Placed { hold: 1 },
        tasks::Hold::Drift,
        tasks::Hold::Effects,
        tasks::Hold::EffectFailed,
        tasks::Hold::Uncertain { entry: 4 },
        tasks::Hold::Budget,
        tasks::Hold::Deadline,
        tasks::Hold::HoldsWaited,
        tasks::Hold::ResourceUnavailable,
        tasks::Hold::ResourceAccess,
        tasks::Hold::PoolLost { pool: 6 },
        tasks::Hold::Procedure,
    ];
    for reason in reasons {
        let (mut core, limits) = make_tree();
        let event = if let tasks::Hold::Placed { hold } = reason {
            placed(2, 1, hold)
        } else {
            tasks::Event::Hold { task: 2, why: reason }
        };
        send_task(&mut core, &limits, event);
        let Document::Escalation(value) =
            make(&core, &limits, 1, &Subject::Escalation { task: 2 }, &window(), Wall::EPOCH).unwrap()
        else {
            panic!()
        };
        let hold = value.task.hold.unwrap();
        assert_eq!(hold.reason, reason);
        assert_eq!(hold.revision, core.tasks.task(2).unwrap().revisions.hold);
        assert_eq!(hold.before, PriorPhase::Active(tasks::Active::Due));
        assert_eq!(value.task.budget.numbers.budget, 10);
        assert_eq!(value.task.lineage.get(0).unwrap().number, 1);
    }
}

#[test]
fn documents_question_words_are_whole_or_refused_and_answered_questions_end() {
    let (mut core, mut limits) = make_tree();
    send_task(
        &mut core,
        &limits,
        tasks::Event::Message {
            reply_to: ReplyTo::new(Token::new(60)),
            project: 1,
            task: 1,
            word: tasks::Word {
                number: 60,
                from: tasks::Party::Task(2),
                kind: tasks::MessageKind::Question,
                words: Box::from(&b"Whole question?"[..]),
                at: Wall::from_nanos(9),
                hits: 1,
                eligible: true,
            },
        },
    );
    send_task(&mut core, &limits, tasks::Event::Prepare { reply_to: ReplyTo::new(Token::new(62)), task: 1 });
    send_task(
        &mut core,
        &limits,
        tasks::Event::Claim {
            account: None,
            reply_to: ReplyTo::new(Token::new(63)),
            task: 1,
            attempt: 1,
            resumed: false,
            budget: 10,
            writes: Box::new([]),
        },
    );
    send_task(&mut core, &limits, tasks::Event::Started { task: 1, attempt: 1 });
    send_task(
        &mut core,
        &limits,
        tasks::Event::Turn {
            refused: Box::new([]),
            reply_to: ReplyTo::new(Token::new(64)),
            task: 1,
            attempt: 1,
            turn: 1,
            read: Some(60),
            offered: Some(60),
            outstanding: Box::new([]),
            cumulative: 0,
        },
    );
    assert!(core.tasks.task(1).unwrap().inbox.is_empty());
    let entries = core.waiting_entries(&limits, core.tasks.task(1).unwrap());
    let mut listed = false;
    for entry in &entries {
        listed |= entry.kind == people::EntryKind::Question { message: 60 } && entry.at == Wall::from_nanos(9);
    }
    assert!(listed);
    let target = Subject::Question { task: 1, message: 60 };
    let Document::Question(question) = make(&core, &limits, 1, &target, &window(), Wall::EPOCH).unwrap() else {
        panic!()
    };
    let mut answerable = false;
    for offer in &question.acts {
        answerable |= offer.act == crate::acts::Act::Answer && offer.admission == crate::acts::Admission::Allowed;
    }
    assert!(answerable);
    assert_eq!(question.words.as_ref(), b"Whole question?");
    assert_eq!(question.at, Wall::from_nanos(9));
    limits.documents.text_bytes = 5;
    assert_eq!(make(&core, &limits, 1, &target, &window(), Wall::EPOCH), Err(Missing::TooLarge));
    limits.documents.text_bytes = 256;
    send_task(
        &mut core,
        &limits,
        tasks::Event::Message {
            reply_to: ReplyTo::new(Token::new(61)),
            project: 1,
            task: 1,
            word: tasks::Word {
                number: 61,
                from: tasks::Party::Person(1),
                kind: tasks::MessageKind::Answer { question: 60 },
                words: Box::from(&b"Yes"[..]),
                at: Wall::from_nanos(10),
                hits: 1,
                eligible: true,
            },
        },
    );
    assert_eq!(make(&core, &limits, 1, &target, &window(), Wall::EPOCH), Err(Missing::Ended));
}

#[test]
fn documents_person_task_has_the_current_revision_holder_and_answer_form() {
    let (mut core, limits) = make_tree();
    let mut row = new_row(&core, 5, tasks::Funder::Period { project: 1, period: 1 }, None);
    row.executor = tasks::Executor::Person(tasks::PersonAddress::Role(0));
    row.contract = tasks::Contract::Verdict {
        choices: Box::new([tasks::Verdict { fields: Box::new([]), code: 3, words: 32, followups: 0 }]),
    };
    send_task(
        &mut core,
        &limits,
        tasks::Event::Make {
            reply_to: ReplyTo::new(Token::new(70)),
            creator: tasks::Party::Person(1),
            batch: Box::new([row]),
        },
    );
    send_task(
        &mut core,
        &limits,
        tasks::Event::TakePerson { reply_to: ReplyTo::new(Token::new(71)), task: 5, person: 1 },
    );
    let Document::PersonTask(task) =
        make(&core, &limits, 1, &Subject::PersonTask { task: 5 }, &window(), Wall::EPOCH).unwrap()
    else {
        panic!()
    };
    assert_eq!(task.address, tasks::PersonAddress::Role(0));
    assert_eq!(task.taken_by, Some(1));
    let wait = core.tasks.task(5).unwrap().person_wait.unwrap();
    assert_eq!(task.since, Some(wait.since));
    assert_eq!(task.stall, Some(limits.tasks.person_stall));
    assert_eq!(task.deadline, Some(wait.deadline));
    assert_eq!(task.revision, core.tasks.task(5).unwrap().revisions.person);
    let ContractLine::Verdict { choices, cut } = task.contract else { panic!() };
    assert!(!cut);
    assert_eq!(choices.get(0).unwrap().code, 3);
}

#[test]
fn documents_retained_results_and_ended_delegates_preserve_cancellation_facts() {
    let (core, limits) = make_tree();
    let mut row = core.tasks.task(2).unwrap().clone();
    row.phase = tasks::Phase::Ended(tasks::Ending::Cancelled {
        reason: Box::from(&b"Done enough"[..]),
        result: Some(tasks::TaskResult::Report { words: Box::from(&b"Partial work"[..]) }),
        by: tasks::Party::Person(1),
        at: Wall::from_nanos(19),
    });
    row.ended_at = Some(Wall::from_nanos(19));
    let result = result_from_row(&core, &limits, 1, &row, Wall::EPOCH).unwrap();
    assert!(result.followups.as_ref().unwrap().is_empty());
    assert_eq!(result.at, Some(Wall::from_nanos(19)));
    assert_eq!(result.task.phase, PhaseLine::Ended);
    let Ending::Cancelled { by, at, result, .. } = result.ending else { panic!() };
    assert_eq!(by, tasks::Party::Person(1));
    assert_eq!(at, Wall::from_nanos(19));
    let ResultValue::Report(words) = result.unwrap() else { panic!() };
    assert_eq!(words.bytes.as_ref(), b"Partial work");
    let delegate = ended_delegate_from_row(&limits, &row).unwrap();
    assert_eq!(delegate.chip.number, 2);
    assert_eq!(delegate.at, row.ended_at);
    for ending in [
        tasks::Ending::Done(tasks::TaskResult::Verdict {
            fields: Box::new([]),
            code: 3,
            words: Box::from(&b"Choice"[..]),
        }),
        tasks::Ending::Failed { reason: Box::from(&b"Failure"[..]) },
        tasks::Ending::Done(tasks::TaskResult::Change {
            connector: 2,
            kind: 3,
            resource: 4,
            words: Box::from(&b"Changed"[..]),
        }),
    ] {
        row.phase = tasks::Phase::Ended(ending);
        let result = result_from_row(&core, &limits, 1, &row, Wall::EPOCH).unwrap();
        match result.ending {
            Ending::Done(ResultValue::Verdict { code, .. }) => assert_eq!(code, 3),
            Ending::Done(ResultValue::Change { part, .. }) => assert_eq!(part.key, PartKey::Number(4)),
            Ending::Failed(reason) => assert_eq!(reason.bytes.as_ref(), b"Failure"),
            Ending::Done(ResultValue::Approved { .. } | ResultValue::Report(_) | ResultValue::Failure(_))
            | Ending::Cancelled { .. } => panic!(),
        }
    }
    row.phase = tasks::Phase::Ended(tasks::Ending::Done(tasks::TaskResult::Verdict {
        fields: Box::new([]),
        code: 3,
        words: Box::new([]),
    }));
    row.delegates = Box::new([1]);
    row.result_followups = Box::new([3, 99]);
    let result = result_from_row(&core, &limits, 1, &row, Wall::EPOCH).unwrap();
    let TaskLink::Live(delegate) = result.delegates.get(0).unwrap() else { panic!() };
    assert_eq!(delegate.number, 1);
    let followups = result.followups.unwrap();
    let TaskLink::Live(child) = followups.get(0).unwrap() else { panic!() };
    assert_eq!(child.number, 3);
    match followups.get(1).unwrap() {
        TaskLink::Stored { number, status } => {
            assert_eq!(*number, 99);
            assert_eq!(*status, None);
        }
        TaskLink::Live(_) | TaskLink::Retained { .. } => panic!(),
    }
    let mut limited = limits;
    limited.documents.lines = 1;
    let result = result_from_row(&core, &limited, 1, &row, Wall::EPOCH).unwrap();
    assert!(result.cut);
    assert_eq!(result.followups.unwrap().len(), 1);
}

#[test]
fn documents_notes_carry_the_child_revision_and_cut_display_text_only() {
    let (core, mut limits) = make_tree();
    limits.documents.text_bytes = 5;
    let mut references = List::with_capacity(1);
    references.push(2).unwrap();
    let entry = notes::Entry {
        name: 8,
        scope: notes::Scope::Project { project: 1 },
        description: Box::from(&b"Description"[..]),
        body: Box::from(&b"Full body"[..]),
        references,
        author: notes::Author::Task { task: 2, attempt: 1 },
        revision: 7,
        changed: Wall::from_nanos(100),
    };
    let note = note_from_entry(&core, &limits, 1, &entry).unwrap();
    assert_eq!(note.revision, 7);
    assert_eq!(note.writer, entry.author);
    assert_eq!(note.tasks.get(0), Some(&2));
    assert!(note.description.cut);
    assert!(note.body.cut);
    assert_eq!(note.changed, Some(entry.changed));
    assert_eq!(make(&core, &limits, 1, &Subject::Note { note: 8 }, &window(), Wall::EPOCH), Err(Missing::Ended));
}

#[test]
fn documents_history_retains_every_change_kind_and_a_passed_proposal_state() {
    let (_, limits) = make_tree();
    let changes = [
        tasks::Change::Cancelled,
        tasks::Change::Released,
        tasks::Change::Amended,
        jig_domain_tasks::Change::Amended,
        tasks::Change::Moved,
        tasks::Change::Proposed,
        tasks::Change::ProposalPassed,
        tasks::Change::ProposalAccepted,
        tasks::Change::ProposalRejected,
        tasks::Change::ProposalWithdrawn,
    ];
    for change in changes {
        let row = tasks::History {
            amendment: None,
            at: Wall::EPOCH,
            task: 2,
            revision: 5,
            by: tasks::Party::Person(1),
            change,
            reason: Box::from(&b"Why"[..]),
            proposal: Some(Box::new(tasks::Proposal {
                number: 6,
                proposer: 2,
                project: 1,
                action: tasks::ProposalAction::Release { task: 2 },
                reason: Box::from(&b"Why"[..]),
                as_holder: false,
                state: tasks::ProposalState::Pending {
                    holder: tasks::ProposalHolder::Person(1),
                    since: Wall::from_nanos(8),
                },
            })),
        };
        let history = history_from_row(&limits, &row).unwrap();
        assert_eq!(history.change, change);
        assert_eq!(history.by, row.by);
        assert_eq!(history.reason.bytes.as_ref(), b"Why");
        assert_eq!(
            history.proposal.unwrap().state,
            ProposalHistoryState::Pending(ProposalWaiting {
                holder: tasks::ProposalHolder::Person(1),
                since: Wall::from_nanos(8)
            })
        );
    }
}

#[test]
fn documents_project_exposes_two_roles_members_pool_and_resource_parts() {
    let (mut core, limits) = make_tree();
    let mut policy = core.authority.policy(1).unwrap().clone();
    let mut second = policy.roles[0].clone();
    second.number = 1;
    policy.roles = Box::new([policy.roles[0].clone(), second]);
    let mut admitted = authority_limits();
    admitted.roles = 2;
    core.authority = authority::Domain::new(core.authority.rules().clone(), admitted).unwrap();
    let mut out = Queue::with_capacity(authority::POLICY_MAX_OUT);
    authority::step(&mut core.authority, authority::Event::Policy { project: 1, policy }, &mut out);
    assert_eq!(out.pop(), Some(authority::PolicyFact::Added { project: 1 }));
    let first_identity = core.people.identity(1).unwrap().clone();
    core.people = people::Domain::new(&limits.people, Box::new([]), core.settings.deployment_provider, Box::new([]));
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.people };
    let mut restored = Queue::with_capacity(people::max_out(&limits.people));
    for record in [
        people::Stored::Person { number: 1, identity: first_identity },
        people::Stored::Person {
            number: 2,
            identity: people::Identity {
                key: people::IdentityKey {
                    provider: core.settings.deployment_provider,
                    subject: Box::from(&b"service"[..]),
                },
                login: Box::from(&b"service"[..]),
                name: Box::from(&b"Service"[..]),
            },
        },
        people::Stored::Roles {
            project: 1,
            holdings: Box::new([
                people::Holding { person: 1, role: people::Role::Owner },
                people::Holding { person: 2, role: people::Role::Maintainer },
            ]),
        },
    ] {
        people::step(&mut core.people, &env, people::Event::Restore { record }, &mut restored);
        while restored.pop().is_some() {}
    }
    people::step(&mut core.people, &env, people::Event::Restored, &mut restored);
    send_task(
        &mut core,
        &limits,
        tasks::Event::CarvePool {
            reply_to: ReplyTo::new(Token::new(80)),
            project: 1,
            person: 1,
            period: 1,
            budget: 200,
        },
    );
    core.resource_roles
        .insert(
            resources::Key {
                project: 1,
                name: tasks::Name { connector: 2, path: Box::new([Box::from(&b"resource"[..])]) },
            },
            connector::ResourceRole::Owned,
        )
        .unwrap();
    let Document::Project(project) =
        make(&core, &limits, 1, &Subject::Project { project: 1 }, &window(), Wall::EPOCH).unwrap()
    else {
        panic!()
    };
    assert_eq!(project.policy.roles.len(), 2);
    assert_eq!(project.members.get(0).unwrap().role, people::Role::Owner);
    assert_eq!(project.members.get(1).unwrap().kind, people::Kind::Service);
    assert_eq!(project.members.get(1).unwrap().role, people::Role::Maintainer);
    assert_eq!(project.current_period.unwrap().numbers.reserved, 300);
    assert_eq!(project.pools.get(0).unwrap().left, 200);
    assert_eq!(project.resources.get(0).unwrap().part.connector, 2);
    assert_eq!(project.resources.get(0).unwrap().role, ResourceRole::Owned);
    assert_eq!(project.spend.get(0).unwrap().chip.number, 1);
    assert_eq!(project.spend.get(0).unwrap().numbers.reserved, 30);
}

#[test]
fn documents_lifecycle_history_keeps_holds_and_endings() {
    let (_, limits) = make_tree();
    let row = tasks::Milestone {
        at: Wall::EPOCH,
        task: 2,
        position: 3,
        phase: tasks::Phase::Held {
            was: tasks::Was::Active(tasks::Active::Due),
            why: tasks::Hold::Deadline,
            source: None,
        },
    };
    let line = milestone_from_row(&limits, &row).unwrap();
    assert_eq!(line.position, 3);
    assert_eq!(
        line.phase,
        MilestonePhase::Held {
            before: PriorPhase::Active(tasks::Active::Due),
            why: tasks::Hold::Deadline,
            ending: None,
            source: None
        }
    );
    assert_eq!(line.at, Some(Wall::EPOCH));
    let line = milestone_from_row(
        &limits,
        &tasks::Milestone {
            at: Wall::EPOCH,
            task: 2,
            position: 4,
            phase: tasks::Phase::Ended(tasks::Ending::Failed { reason: Box::from(&b"Failure"[..]) }),
        },
    );
    let line = line.unwrap();
    let MilestonePhase::Ended(Ending::Failed(words)) = line.phase else { panic!() };
    assert_eq!(words.bytes.as_ref(), b"Failure");
}

fn restore_document_party(core: &mut Core, limits: &Limits, projects: &[u32]) {
    let identity = core.people.identity(1).unwrap().clone();
    core.people = people::Domain::new(&limits.people, Box::new([]), core.settings.deployment_provider, Box::new([]));
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.people };
    let mut out = Queue::with_capacity(people::max_out(&limits.people));
    people::step(
        &mut core.people,
        &env,
        people::Event::Restore { record: people::Stored::Person { number: 1, identity } },
        &mut out,
    );
    while out.pop().is_some() {}
    for project in projects {
        people::step(
            &mut core.people,
            &env,
            people::Event::Restore {
                record: people::Stored::Roles {
                    project: *project,
                    holdings: Box::new([people::Holding { person: 1, role: people::Role::Owner }]),
                },
            },
            &mut out,
        );
        while out.pop().is_some() {}
    }
    people::step(&mut core.people, &env, people::Event::Restored, &mut out);
    assert!(core.people.identity(1).is_some());
}
#[test]
fn documents_inbox_keeps_two_projects_groups_order_and_exact_cursor() {
    let (mut core, mut limits) = make_tree();
    limits.people.projects = 2;
    limits.people.inbox_entries = 8;
    restore_document_party(&mut core, &limits, &[1, 2]);
    send_task(
        &mut core,
        &limits,
        tasks::Event::OpenPeriod { reply_to: ReplyTo::new(Token::new(101)), project: 2, period: 1, budget: 1000 },
    );
    let mut task = new_row(&core, 5, tasks::Funder::Period { project: 2, period: 1 }, None);
    task.project = 2;
    send_task(
        &mut core,
        &limits,
        tasks::Event::Make {
            reply_to: ReplyTo::new(Token::new(102)),
            creator: tasks::Party::Person(1),
            batch: Box::new([task]),
        },
    );
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.people };
    let mut out = Queue::with_capacity(people::max_out(&limits.people));
    let needs = [
        people::Entry {
            task: 1,
            project: 1,
            whom: people::Whom::Person(1),
            kind: people::EntryKind::Question { message: 9 },
            at: Wall::from_nanos(30),
        },
        people::Entry {
            task: 1,
            project: 1,
            whom: people::Whom::Person(1),
            kind: people::EntryKind::Question { message: 8 },
            at: Wall::from_nanos(20),
        },
        people::Entry {
            task: 5,
            project: 2,
            whom: people::Whom::Role { project: 2, role: 0 },
            kind: people::EntryKind::PersonTask,
            at: Wall::from_nanos(10),
        },
    ];
    people::step(
        &mut core.people,
        &env,
        people::Event::Waiting { task: 1, entries: Box::new([needs[0], needs[1]]) },
        &mut out,
    );
    people::step(
        &mut core.people,
        &env,
        people::Event::Waiting {
            task: 5,
            entries: Box::new([
                needs[2],
                people::Entry {
                    task: 5,
                    project: 2,
                    whom: people::Whom::Person(1),
                    kind: people::EntryKind::Reply { message: 7 },
                    at: Wall::from_nanos(40),
                },
            ]),
        },
        &mut out,
    );
    let Document::Inbox(first) =
        make(&core, &limits, 1, &Subject::Inbox { party: 1 }, &Window { rows: 2, after: None }, Wall::EPOCH).unwrap()
    else {
        panic!()
    };
    assert_eq!(first.needs.len(), 2);
    assert_eq!(first.cached_needs, 3);
    assert!(first.updates.is_empty());
    assert_eq!(first.needs.get(0).unwrap().project, Some(2));
    assert_eq!(first.needs.get(1).unwrap().object, InboxObject::Question { message: 8 });
    let Document::Inbox(next) =
        make(&core, &limits, 1, &Subject::Inbox { party: 1 }, &Window { rows: 2, after: first.next }, Wall::EPOCH)
            .unwrap()
    else {
        panic!()
    };
    assert_eq!(next.needs.get(0).unwrap().object, InboxObject::Question { message: 9 });
    assert_eq!(next.updates.get(0).unwrap().object, InboxObject::Reply { message: 7 });
    assert_eq!(next.next, None);
    assert_eq!(make(&core, &limits, 1, &Subject::Inbox { party: 2 }, &window(), Wall::EPOCH), Err(Missing::NotAllowed));
}
#[test]
fn documents_cached_inbox_results_preserve_positions_project_and_time() {
    let (mut core, limits) = make_tree();
    core.people.remember_result(
        &limits.people,
        1,
        people::ResultRef { task: 100, project: 1, at: Wall::from_nanos(30), position: 30, forgettable: None },
    );
    core.people.remember_result(
        &limits.people,
        1,
        people::ResultRef { task: 99, project: 1, at: Wall::from_nanos(31), position: 31, forgettable: None },
    );
    let Document::Inbox(inbox) =
        make(&core, &limits, 1, &Subject::Inbox { party: 1 }, &Window { rows: 2, after: None }, Wall::EPOCH).unwrap()
    else {
        panic!()
    };
    assert_eq!(inbox.updates.get(0).unwrap().object, InboxObject::Result { position: 31 });
    assert_eq!(inbox.updates.get(0).unwrap().project, Some(1));
    assert_eq!(inbox.updates.get(0).unwrap().at, Some(Wall::from_nanos(31)));
    assert_eq!(inbox.updates.get(0).unwrap().unread, Some(true));
    assert!(inbox.updates_time_complete);
    assert_eq!(inbox.cached_updates, Count::Known(2));
}
#[test]
fn documents_deployment_keeps_lost_worker_attempts_and_translates_the_grace_clock() {
    let (mut core, mut limits) = make_tree();
    limits.people.projects = 2;
    assert_eq!(make(&core, &limits, 1, &Subject::Deployment, &window(), Wall::EPOCH), Err(Missing::NotAllowed));
    restore_document_party(&mut core, &limits, &[0]);
    let env = Env { now: Time::from_nanos(100), wall: Wall::from_nanos(1000), limits: limits.fleet };
    let mut out = Queue::with_capacity(fleet::max_out(&limits.fleet));
    fleet::step(
        &mut core.fleet,
        &env,
        fleet::Event::Hello {
            worker: 0,
            channel: Token::new(10),
            hello: fleet::Hello {
                bounds: fixtures::bounds(),
                stop_bound: Duration::from_secs(1),
                slots: 1,
                workstreams: Box::new([]),
                hosting: Box::new([fleet::Hosted {
                    run: Token::new(2),
                    attempt: Token::new(9),
                    phase: fleet::Phase::Active,
                }]),
            },
        },
        &mut out,
    );
    while out.pop().is_some() {}
    let clock = DocumentClock { now: env.now, wall: env.wall };
    let Document::Deployment(deployment) =
        make_with_clock(&core, &limits, 1, &Subject::Deployment, &window(), clock).unwrap()
    else {
        panic!()
    };
    assert_eq!(deployment.hosts.len(), 1);
    assert_eq!(deployment.hosts.get(0).unwrap().used, 1);
    assert_eq!(deployment.hosts.get(0).unwrap().free, 0);
    assert_eq!(deployment.attempts.get(0).unwrap().attempt, 9);
    fleet::step(
        &mut core.fleet,
        &env,
        fleet::Event::Hello {
            worker: 1,
            channel: Token::new(11),
            hello: fleet::Hello {
                bounds: fixtures::bounds(),
                stop_bound: Duration::ZERO,
                slots: 2,
                workstreams: Box::new([]),
                hosting: Box::new([]),
            },
        },
        &mut out,
    );
    fleet::step(&mut core.fleet, &env, fleet::Event::Lost { channel: Token::new(10) }, &mut out);
    let Document::Deployment(deployment) =
        make_with_clock(&core, &limits, 1, &Subject::Deployment, &window(), clock).unwrap()
    else {
        panic!()
    };
    let host = deployment.hosts.get(0).unwrap();
    assert_eq!(host.channel, 10);
    assert!(!host.connected);
    assert_eq!(host.slots, 1);
    assert_eq!(host.used, 1);
    assert_eq!(host.runs.get(0).unwrap().attempt, 9);
    assert_eq!(host.grace_until, Some(Wall::from_nanos(1000 + limits.fleet.grace.as_nanos())));
    assert_eq!(host.grace_remaining, Some(limits.fleet.grace));
    assert_eq!(deployment.workers.used, 0);
    assert_eq!(deployment.attempts.len(), 1);
    let refused = deployment.hello_refusals.get(0).unwrap();
    assert_eq!(refused.channel, 11);
    assert_eq!(refused.at, env.wall);
    assert_eq!(refused.why, fleet::HelloRefusal::Workers { limit: 1 });
    let RunLocation::Adrift { channel, until, remaining } = deployment.attempts.get(0).unwrap().location else {
        panic!()
    };
    assert_eq!(channel, Some(10));
    assert_eq!(until, Some(Wall::from_nanos(1000 + limits.fleet.grace.as_nanos())));
    assert_eq!(remaining, Some(limits.fleet.grace));
    let Document::Deployment(without_clock) =
        make(&core, &limits, 1, &Subject::Deployment, &window(), Wall::EPOCH).unwrap()
    else {
        panic!()
    };
    assert_eq!(
        without_clock.attempts.get(0).unwrap().location,
        RunLocation::Adrift { channel: Some(10), until: None, remaining: None }
    );
}
#[test]
fn documents_deployment_accounts_are_secret_free_and_show_unusable_state() {
    let (mut core, mut limits) = make_tree();
    run_choice::host(&mut core, &limits, Token::new(10), &[b"scripted"]);
    limits.people.projects = 2;
    restore_document_party(&mut core, &limits, &[0, 1]);
    let env = Env { now: Time::from_nanos(100), wall: Wall::from_nanos(1000), limits: limits.accounts };
    let mut out = Queue::with_capacity(accounts::max_out(&limits.accounts));
    accounts::step(&mut core.accounts, &env, accounts::Event::Want, &mut out);
    while out.pop().is_some() {}
    accounts::step(
        &mut core.accounts,
        &env,
        accounts::Event::Component(accounts::FromComponent::Granted {
            form: accounts::Form::Credential(accounts::Scheme::Bearer),
            account: 1,
            generation: 1,
            valid: Duration::from_secs(60),
        }),
        &mut out,
    );
    while out.pop().is_some() {}
    accounts::step(
        &mut core.accounts,
        &env,
        accounts::Event::Limited { account: 1, reset: Some(env.now.saturating_add(Duration::from_secs(5))) },
        &mut out,
    );
    let Document::Deployment(deployment) = make_with_clock(
        &core,
        &limits,
        1,
        &Subject::Deployment,
        &window(),
        DocumentClock { now: env.now, wall: env.wall },
    )
    .unwrap() else {
        panic!()
    };
    let account = deployment.accounts.get(0).unwrap();
    assert!(!account.usable);
    assert_eq!(account.number, 0);
    let reset = Some(Wall::from_nanos(1000 + Duration::from_secs(5).as_nanos()));
    assert_eq!(account.state, AccountState::Later { until: reset });
    assert_eq!(account.spent_until, reset);
    assert!(!account.waiting.is_empty());
}

fn watch_core() -> (Core, Limits) {
    let (mut core, mut limits) = core();
    limits.people.projects = 2;
    limits.people.requests = 0;
    make_task(&mut core, &limits, 1);
    core.proofs
        .insert(
            1,
            RunProof {
                accounts: Box::new([]),
                placement: None,
                account_failure: None,
                account_terminal: false,
                task: 1,
                attempt: 7,
                conversation: 1,
                credentials: Box::new([]),
                window: None,
                crossed: false,
                host: fleet::HostKind::Engine,
                offered: None,
                outstanding: Box::new([]),
                turn: None,
                terminal: None,
            },
        )
        .unwrap();
    let identity = core.people.identity(1).unwrap().clone();
    core.people = people::Domain::new(&limits.people, Box::new([]), core.settings.deployment_provider, Box::new([]));
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.people };
    let mut out = Queue::with_capacity(people::max_out(&limits.people));
    let records = [
        people::Stored::Person { number: 1, identity },
        people::Stored::SignIn { number: 10, person: 1, expires: Wall::from_nanos(Duration::from_secs(60).as_nanos()) },
        people::Stored::SignIn {
            number: 11,
            person: 1,
            expires: Wall::from_nanos(Duration::from_secs(120).as_nanos()),
        },
        people::Stored::Roles {
            project: 1,
            holdings: Box::new([people::Holding { person: 1, role: people::Role::Owner }]),
        },
        people::Stored::Roles {
            project: people::DEPLOYMENT,
            holdings: Box::new([people::Holding { person: 1, role: people::Role::Observer }]),
        },
    ];
    for record in records {
        people::step(&mut core.people, &env, people::Event::Restore { record }, &mut out);
        while out.pop().is_some() {}
    }
    people::step(&mut core.people, &env, people::Event::Restored, &mut out);
    (core, limits)
}

fn watch_outputs(core: &mut Core, limits: &Limits, event: Event) -> Box<[Request]> {
    match step(core, &Env { now: Time::ZERO, wall: Wall::EPOCH, limits: *limits }, event) {
        Requests::Out(mut out) => {
            let mut requests = List::with_capacity(out.len());
            while let Some(request) = out.pop() {
                requests.push(request).unwrap();
            }
            requests.into_boxed()
        }
    }
}

fn watch_subjects() -> [(&'static str, Watched, Subject); 9] {
    let mut chips = List::with_capacity(1);
    chips.push(1).unwrap();
    let query = TaskQuery {
        project: Some(1),
        requester: None,
        root: None,
        parent: None,
        label: None,
        phase: None,
        executor: None,
        tracked: None,
        order: QueryOrder::Changed,
    };
    [
        ("Frame", Watched::Frame, Subject::Frame { party: 1 }),
        ("Task", Watched::Task { task: 1 }, Subject::Task { task: 1 }),
        ("Tree", Watched::Tree { task: 1 }, Subject::Tree { task: 1 }),
        ("Chips", Watched::Chips { tasks: chips.clone() }, Subject::Chips { tasks: chips }),
        ("Inbox", Watched::Inbox, Subject::Inbox { party: 1 }),
        ("Accounts", Watched::Accounts, Subject::Accounts { party: 1 }),
        ("Tasks", Watched::Query { query }, Subject::Query { query }),
        ("Run", Watched::Run { task: 1 }, Subject::Run { task: 1 }),
        ("Deployment", Watched::Deployment, Subject::Deployment),
    ]
}

#[test]
fn an_unkeyed_watch_on_each_subject_makes_its_first_document_as_a_held_output() {
    for (label, subject, named) in watch_subjects() {
        let (mut core, limits) = watch_core();
        let expected = make(&core, &limits, 1, &named, &window(), Wall::EPOCH).unwrap();
        let out = watch_outputs(
            &mut core,
            &limits,
            Event::Watch { watcher: Token::new(30), sign_in: 10, subject, window: window() },
        );
        assert_eq!(out.len(), 2);
        let Request::Held(held) = &out[0] else { panic!("document is held") };
        let Held::Delivery { watcher, document } = held.as_ref() else { panic!("typed delivery") };
        assert_eq!(*watcher, Token::new(30));
        assert_eq!(document.as_ref(), &expected, "{label}");
        let Request::Decided = &out[1] else { panic!("one decision") };
    }
}

#[test]
fn watch_admission_checks_each_subjects_project_or_owner() {
    for (label, subject, _) in watch_subjects() {
        match subject {
            Watched::Frame | Watched::Inbox | Watched::Accounts => continue,
            Watched::Task { .. }
            | Watched::Tree { .. }
            | Watched::Chips { .. }
            | Watched::Query { .. }
            | Watched::Run { .. }
            | Watched::Settings
            | Watched::Deployment
            | Watched::Application { .. } => {}
        }
        let (mut core, limits) = watch_core();
        let mut out = Queue::with_capacity(people::max_out(&limits.people));
        for project in [0, 1] {
            people::step(
                &mut core.people,
                &Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.people },
                people::Event::Roles { project, holdings: Box::new([]) },
                &mut out,
            );
        }
        let out = watch_outputs(
            &mut core,
            &limits,
            Event::Watch { watcher: Token::new(30), sign_in: 10, subject, window: window() },
        );
        assert_watch_refusal(&out[0], WatchRefusal::NotAllowed);
        assert!(core.view_subjects.is_empty(), "{label}");
    }
}

#[test]
fn signed_out_unknown_and_full_watches_are_refused_without_keyed_work() {
    let (mut core, limits) = watch_core();
    for (label, sign_in, subject, why) in [
        ("SignedOut", 99, Watched::Frame, WatchRefusal::SignedOut),
        ("Unknown", 10, Watched::Run { task: 99 }, WatchRefusal::Unknown),
    ] {
        let out = watch_outputs(
            &mut core,
            &limits,
            Event::Watch { watcher: Token::new(30), sign_in, subject, window: window() },
        );
        assert_watch_refusal(&out[0], why);
        assert!(core.view_subjects.is_empty(), "{label}");
    }
    for watcher in 1..=limits.views.watchers {
        watch_outputs(
            &mut core,
            &limits,
            Event::Watch {
                watcher: Token::new(u64::from(watcher)),
                sign_in: 10,
                subject: Watched::Frame,
                window: window(),
            },
        );
    }
    let out = watch_outputs(
        &mut core,
        &limits,
        Event::Watch { watcher: Token::new(30), sign_in: 10, subject: Watched::Frame, window: window() },
    );
    assert_watch_refusal(&out[0], WatchRefusal::Busy);
}

#[test]
fn a_tree_watch_past_the_rows_bound_keeps_its_clamped_window() {
    let (mut core, mut limits) = make_tree();
    let (authenticated, auth_limits) = watch_core();
    core.people = authenticated.people;
    limits.people = auth_limits.people;
    limits.documents.rows = 2;
    let watcher = Token::new(30);
    let after = Some(Cursor::Number { number: 2 });
    let out = watch_outputs(
        &mut core,
        &limits,
        Event::Watch { watcher, sign_in: 10, subject: Watched::Tree { task: 1 }, window: Window { rows: 32, after } },
    );
    assert_eq!(out.len(), 2);
    let Request::Held(held) = &out[0] else { panic!("admitted delivery") };
    let Held::Delivery { document, .. } = held.as_ref() else { panic!("document delivery") };
    let Document::Tree(tree) = document.as_ref() else { panic!("tree document") };
    assert_eq!(tree.rows.len(), 2);
    assert_eq!(tree.rows.get(0).unwrap().chip.number, 3);
    assert_eq!(tree.rows.get(1).unwrap().chip.number, 4);
    assert_eq!(core.view_subjects.get(&watcher).unwrap().window, Window { rows: 2, after });
    assert_eq!(core.views.watchers(), 1);
}

#[test]
fn an_application_watch_asks_its_connector_with_the_clamped_window() {
    let (mut core, mut limits) = watch_core();
    limits.documents.rows = 2;
    let watcher = Token::new(30);
    let connector = core.connectors[0];
    let subject = Token::new(90);
    let scope = people::ApplicationScope::Project(1);
    let after = Some(Cursor::Number { number: 7 });
    let out = watch_outputs(
        &mut core,
        &limits,
        Event::Watch {
            watcher,
            sign_in: 10,
            subject: Watched::Application { scope, connector, subject },
            window: Window { rows: 32, after },
        },
    );
    assert_eq!(out.len(), 2);
    let Request::Ask {
        connector: owner,
        ask: Ask::Document { watcher: got, scope: got_scope, subject: got_subject, party, window },
    } = &out[0]
    else {
        panic!("connector construction")
    };
    assert_eq!((*owner, *got, *got_scope, *got_subject, *party), (connector, watcher, scope, subject, 1));
    assert_eq!(*window, Window { rows: 2, after });
    assert_eq!(core.view_subjects.get(&watcher).unwrap().window, *window);
}

#[test]
fn a_watch_whose_task_has_an_oversized_whole_value_is_refused_unknown() {
    let (mut core, mut limits) = tree_core();
    let (authenticated, auth_limits) = watch_core();
    core.people = authenticated.people;
    limits.people = auth_limits.people;
    let mut row = new_row(&core, 1, tasks::Funder::Period { project: 1, period: 1 }, None);
    row.authority.grants = Box::new([tasks::Grant {
        connector: 1,
        kind: 1,
        pattern: tasks::Pattern {
            segments: Box::new([b"whole".as_slice().into()]),
            last: tasks::Last::Exact(b"end".as_slice().into()),
        },
    }]);
    send_task(
        &mut core,
        &limits,
        tasks::Event::Make {
            reply_to: ReplyTo::new(Token::new(1)),
            creator: tasks::Party::Person(1),
            batch: Box::new([row]),
        },
    );
    limits.documents.text_bytes = 4;
    assert_eq!(make(&core, &limits, 1, &Subject::Task { task: 1 }, &window(), Wall::EPOCH), Err(Missing::TooLarge));
    let out = watch_outputs(
        &mut core,
        &limits,
        Event::Watch { watcher: Token::new(30), sign_in: 10, subject: Watched::Task { task: 1 }, window: window() },
    );
    assert_eq!(out.len(), 2);
    assert_watch_refusal(&out[0], WatchRefusal::Unknown);
    assert!(core.view_subjects.is_empty());
    assert!(core.view_names.is_empty());
    assert_eq!(core.views.watchers(), 0);
}

#[test]
fn an_unwatch_waits_for_its_delivery_then_ends_as_a_held_output() {
    let (mut core, limits) = watch_core();
    let watcher = Token::new(30);
    watch_outputs(&mut core, &limits, Event::Watch { watcher, sign_in: 10, subject: Watched::Frame, window: window() });
    let out = watch_outputs(&mut core, &limits, Event::Unwatch { watcher });
    assert_eq!(out.len(), 1);
    let out = watch_outputs(&mut core, &limits, Event::Delivered { watcher });
    let Request::Held(held) = &out[0] else { panic!("ending is held") };
    let Held::WatchEnded { watcher: got, why } = held.as_ref() else { panic!("watch ending") };
    assert_eq!(*got, watcher);
    assert_eq!(*why, views::End::Unwatched);
    assert!(core.view_subjects.is_empty());
}

fn assert_watch_refusal(request: &Request, why: WatchRefusal) {
    let Request::Now(now) = request else { panic!("refusal writes nothing") };
    let Now::WatchRefused { why: got, .. } = now.as_ref() else { panic!("watch refusal") };
    assert_eq!(*got, why);
}

fn finish_watch_outputs(core: &mut Core, limits: &Limits) -> Box<[Request]> {
    for _ in 0_u32..8 {
        let Requests::Out(mut out) =
            finish_decision(core, &Env { now: Time::ZERO, wall: Wall::EPOCH, limits: *limits });
        let mut requests = List::with_capacity(out.len());
        let mut handoff = false;
        while let Some(request) = out.pop() {
            match request {
                Request::ProjectMade { .. } | Request::Write(_) | Request::Ask { .. } => handoff = true,
                request @ (Request::Held(_) | Request::Now(_) | Request::Decided) => requests.push(request).unwrap(),
            }
        }
        if !handoff {
            return requests.into_boxed();
        }
    }
    panic!("bounded projection completion");
}

#[test]
fn one_task_change_marks_trees_queries_and_named_chips_once() {
    let (mut core, mut limits) = make_tree();
    let (authenticated, auth_limits) = watch_core();
    core.people = authenticated.people;
    limits.people = auth_limits.people;
    finish_watch_outputs(&mut core, &limits);
    let mut chips = List::with_capacity(1);
    chips.push(2).unwrap();
    let subjects = [
        Watched::Task { task: 2 },
        Watched::Tree { task: 2 },
        Watched::Tree { task: 1 },
        Watched::Query {
            query: TaskQuery {
                project: Some(1),
                requester: None,
                root: None,
                parent: None,
                label: None,
                phase: None,
                executor: None,
                tracked: Some(true),
                order: QueryOrder::Priority,
            },
        },
        Watched::Chips { tasks: chips },
    ];
    for (index, subject) in subjects.into_iter().enumerate() {
        let watcher = Token::new(u64::try_from(index).unwrap() + 30);
        watch_outputs(&mut core, &limits, Event::Watch { watcher, sign_in: 10, subject, window: window() });
        watch_outputs(&mut core, &limits, Event::Delivered { watcher });
    }
    let changed =
        watch_outputs(&mut core, &limits, Event::Tasks(tasks::Event::Hold { task: 2, why: tasks::Hold::Budget }));
    assert!(!changed.is_empty());
    let out = finish_watch_outputs(&mut core, &limits);
    assert_eq!(out.len(), 6);
    let mut watchers = skein_lib::Set::with_capacity(5);
    for request in out.iter().take(5) {
        let Request::Held(held) = request else { panic!("held change") };
        let Held::Delivery { watcher, document } = held.as_ref() else { panic!("document change") };
        assert!(watchers.insert(*watcher).unwrap());
        let binding = core.view_subjects.get(watcher).unwrap();
        let subject = &core.view_names.get(binding.subject).unwrap().subject;
        assert_eq!(document.as_ref(), &make(&core, &limits, 1, subject, &window(), Wall::EPOCH).unwrap());
    }
    assert_eq!(finish_watch_outputs(&mut core, &limits).len(), 1);
}

#[test]
fn ten_changed_decisions_coalesce_behind_one_inflight_delivery() {
    let (mut core, limits) = watch_core();
    let watcher = Token::new(30);
    watch_outputs(&mut core, &limits, Event::Watch { watcher, sign_in: 10, subject: Watched::Inbox, window: window() });
    for message in 1..=10 {
        watch_outputs(
            &mut core,
            &limits,
            Event::People(people::Event::Waiting {
                task: 1,
                entries: Box::new([people::Entry {
                    task: 1,
                    project: 1,
                    whom: people::Whom::Person(1),
                    kind: people::EntryKind::Reply { message },
                    at: Wall::from_nanos(message),
                }]),
            }),
        );
        assert_eq!(finish_watch_outputs(&mut core, &limits).len(), 1);
    }
    let out = watch_outputs(&mut core, &limits, Event::Delivered { watcher });
    assert_eq!(out.len(), 2);
    let Request::Held(held) = &out[0] else { panic!("coalesced held delivery") };
    let Held::Delivery { document, .. } = held.as_ref() else { panic!("typed current document") };
    assert_eq!(
        document.as_ref(),
        &make(&core, &limits, 1, &Subject::Inbox { party: 1 }, &window(), Wall::EPOCH).unwrap()
    );
    assert_eq!(watch_outputs(&mut core, &limits, Event::Delivered { watcher }).len(), 1);
}

#[test]
fn a_no_write_watch_delivery_waits_for_the_last_commit_already_made() {
    let (mut core, limits) = watch_core();
    let mut journal = skein_lib::Journal::<u8, Box<Held>>::new(&skein_lib::JournalLimits {
        commits: 2,
        writes: 1,
        held: 4,
        now: 1,
        release: 4,
    });
    let mut earlier = journal.decision(&skein_lib::JournalRoom { writes: 1, held: 0 }).unwrap();
    earlier.write(7).unwrap();
    journal.accept(earlier);
    let commit = journal.commit().unwrap();
    let out = watch_outputs(
        &mut core,
        &limits,
        Event::Watch { watcher: Token::new(30), sign_in: 10, subject: Watched::Task { task: 1 }, window: window() },
    );
    let mut decision = journal.decision(&skein_lib::JournalRoom { writes: 0, held: 1 }).unwrap();
    for request in out {
        if let Request::Held(held) = request {
            decision.hold(held).unwrap();
        }
    }
    journal.accept(decision);
    assert!(journal.commit().is_none());
    let mut released = Queue::with_capacity(4);
    assert_eq!(journal.release(&mut released), skein_lib::Released::None);
    journal.committed(commit.number);
    assert_eq!(journal.release(&mut released), skein_lib::Released::Some);
    assert_eq!(released.len(), 1);
    let delivery = released.pop().unwrap();
    let Held::Delivery { watcher, .. } = delivery.as_ref() else { panic!("held first document") };
    assert_eq!(*watcher, Token::new(30));
}

#[test]
fn watches_share_subject_tokens_and_expired_signins_end_only_their_own_watch() {
    let (mut core, limits) = watch_core();
    for (number, sign_in) in [(30, 10), (31, 11)] {
        let watcher = Token::new(number);
        watch_outputs(
            &mut core,
            &limits,
            Event::Watch { watcher, sign_in, subject: Watched::Task { task: 1 }, window: window() },
        );
        watch_outputs(&mut core, &limits, Event::Delivered { watcher });
    }
    assert_eq!(
        core.view_subjects.get(&Token::new(30)).unwrap().subject,
        core.view_subjects.get(&Token::new(31)).unwrap().subject
    );
    let Requests::Out(mut out) = routing::finish_watches(
        &mut core,
        &Env {
            now: Time::from_nanos(Duration::from_secs(61).as_nanos()),
            wall: Wall::from_nanos(Duration::from_secs(61).as_nanos()),
            limits,
        },
    );
    let Request::Held(held) = out.pop().unwrap() else { panic!("held watch ending") };
    let Held::WatchEnded { watcher, why } = held.as_ref() else { panic!("admission ending") };
    assert_eq!((*watcher, *why), (Token::new(30), views::End::SignedOut));
    assert!(core.view_subjects.contains_key(&Token::new(31)));
    assert_eq!(core.view_names.len(), 1);
}

#[test]
fn taking_a_changed_delivery_after_signin_expiry_ends_instead_of_making_a_document() {
    let (mut core, limits) = watch_core();
    let watcher = Token::new(30);
    watch_outputs(&mut core, &limits, Event::Watch { watcher, sign_in: 10, subject: Watched::Frame, window: window() });
    core.touch_party(1);
    finish_watch_outputs(&mut core, &limits);
    let at = Duration::from_secs(61).as_nanos();
    let Requests::Out(mut out) = step(
        &mut core,
        &Env { now: Time::from_nanos(at), wall: Wall::from_nanos(at), limits },
        Event::Delivered { watcher },
    );
    let Request::Held(held) = out.pop().unwrap() else { panic!("held signed-out ending") };
    let Held::WatchEnded { why, .. } = held.as_ref() else { panic!("no document after expiry") };
    assert_eq!(*why, views::End::SignedOut);
    assert!(core.view_subjects.is_empty());
}

mod activity;
mod completion_retry;
mod conversation;
mod read_filters;
mod run_positions;
fn open_run_watch(core: &mut Core, limits: &Limits, number: u64) {
    let watcher = Token::new(number);
    watch_outputs(
        core,
        limits,
        Event::Watch { watcher, sign_in: 10, subject: Watched::Run { task: 1 }, window: window() },
    );
    watch_outputs(core, limits, Event::Delivered { watcher });
}
fn run_text_outputs(
    core: &mut Core,
    limits: &Limits,
    attempt: u64,
    turn: u32,
    offset: u32,
    text: &[u8],
) -> Box<[Request]> {
    watch_outputs(
        core,
        limits,
        Event::RunText {
            retry: 0,
            run: Token::new(1),
            attempt: Token::new(attempt),
            turn,
            offset,
            text: skein_lib::bytes::copy_of(text),
            call: None,
        },
    )
}
fn assert_stream(request: &Request, watcher: u64, turn: views::TurnName, from: u32, to: u32, text: &[u8], cut: bool) {
    let Request::Now(now) = request else { panic!("provisional stream uses the door") };
    let Now::RunText { watcher: got, turn: named, from: start, to: end, text: bytes, cut: trimmed, .. } = now.as_ref()
    else {
        panic!("typed text stream")
    };
    assert_eq!(
        (*got, *named, *start, *end, bytes.as_ref(), *trimmed),
        (Token::new(watcher), turn, from, to, text, cut)
    );
}

#[test]
fn two_watchers_of_one_run_advance_independent_turn_offsets() {
    let (mut core, limits) = watch_core();
    for watcher in [30, 31] {
        open_run_watch(&mut core, &limits, watcher);
    }
    let turn = views::TurnName { attempt: 7, turn: 1, retry: 0 };
    let first = run_text_outputs(&mut core, &limits, 7, 1, 0, b"abc");
    assert_stream(&first[0], 30, turn, 0, 3, b"abc", false);
    assert_stream(&first[1], 31, turn, 0, 3, b"abc", false);
    watch_outputs(&mut core, &limits, Event::StreamDelivered { watcher: Token::new(30) });
    let second = run_text_outputs(&mut core, &limits, 7, 1, 3, b"def");
    assert_stream(&second[0], 30, turn, 3, 6, b"def", false);
    watch_outputs(&mut core, &limits, Event::StreamDelivered { watcher: Token::new(30) });
    let third = run_text_outputs(&mut core, &limits, 7, 1, 6, b"ghi");
    assert_stream(&third[0], 30, turn, 6, 9, b"ghi", false);
    let slow = watch_outputs(&mut core, &limits, Event::StreamDelivered { watcher: Token::new(31) });
    assert_stream(&slow[0], 31, turn, 3, 9, b"defghi", false);
    let Document::Run(run) = make(&core, &limits, 1, &Subject::Run { task: 1 }, &window(), Wall::EPOCH).unwrap() else {
        panic!("run document")
    };
    let writing = run.writing.unwrap();
    assert_eq!((writing.turn, writing.from, writing.to, writing.text.as_ref()), (turn, 0, 9, b"abcdefghi".as_slice()));
}

#[test]
fn turn_three_writing_follows_committed_turn_two_and_fences_old_attempts() {
    let (mut core, limits) = watch_core();
    core.proofs.get_mut(&1).unwrap().turn = Some(TurnProof { turn: 2, cumulative: 0, read: None });
    open_run_watch(&mut core, &limits, 30);
    assert_eq!(run_text_outputs(&mut core, &limits, 6, 3, 0, b"stale").len(), 1);
    assert_eq!(run_text_outputs(&mut core, &limits, 7, 2, 0, b"committed").len(), 1);
    let out = run_text_outputs(&mut core, &limits, 7, 3, 0, b"third");
    assert_stream(&out[0], 30, views::TurnName { attempt: 7, turn: 3, retry: 0 }, 0, 5, b"third", false);
    let Document::Run(run) = make(&core, &limits, 1, &Subject::Run { task: 1 }, &window(), Wall::EPOCH).unwrap() else {
        panic!("current supplied run document")
    };
    let writing = run.writing.unwrap();
    assert_eq!(
        (writing.turn, writing.from, writing.to, writing.text.as_ref()),
        (views::TurnName { attempt: 7, turn: 3, retry: 0 }, 0, 5, &b"third"[..])
    );
    core.proofs.get_mut(&1).unwrap().turn = Some(TurnProof { turn: 3, cumulative: 0, read: None });
    finish_watch_outputs(&mut core, &limits);
    let Document::Run(run) = make(&core, &limits, 1, &Subject::Run { task: 1 }, &window(), Wall::EPOCH).unwrap() else {
        panic!("run document")
    };
    assert!(run.writing.is_none());
    assert_eq!(watch_outputs(&mut core, &limits, Event::StreamDelivered { watcher: Token::new(30) }).len(), 1);
}

#[test]
fn a_slow_run_stream_reports_the_bounded_suffix_and_cut_offset() {
    let (mut core, limits) = watch_core();
    open_run_watch(&mut core, &limits, 30);
    let width = usize::try_from(limits.views.text_bytes).unwrap();
    let mut a = List::with_capacity(limits.views.text_bytes);
    let mut b = List::with_capacity(limits.views.text_bytes);
    for _ in 0..width {
        a.push(b'a').unwrap();
        b.push(b'b').unwrap();
    }
    let a = a.into_boxed();
    let b = b.into_boxed();
    run_text_outputs(&mut core, &limits, 7, 1, 0, &a);
    run_text_outputs(&mut core, &limits, 7, 1, limits.views.text_bytes, &b);
    run_text_outputs(&mut core, &limits, 7, 1, limits.views.text_bytes * 2, &a);
    let out = watch_outputs(&mut core, &limits, Event::StreamDelivered { watcher: Token::new(30) });
    assert_stream(
        &out[0],
        30,
        views::TurnName { attempt: 7, turn: 1, retry: 0 },
        limits.views.text_bytes * 2,
        limits.views.text_bytes * 3,
        &a,
        true,
    );
}

#[test]
fn a_run_watch_stays_open_between_attempts_and_the_new_attempt_starts_at_zero() {
    let (mut core, limits) = watch_core();
    open_run_watch(&mut core, &limits, 30);
    run_text_outputs(&mut core, &limits, 7, 1, 0, b"old");
    core.proofs.get_mut(&1).unwrap().terminal =
        Some(TerminalRecord { task: 1, attempt: 7, cumulative: 0, end: tasks::End::Parked });
    watch_outputs(&mut core, &limits, Event::Tasks(tasks::Event::Hold { task: 1, why: tasks::Hold::Budget }));
    let ended = finish_watch_outputs(&mut core, &limits);
    let Request::Held(held) = &ended[0] else { panic!("idle run document") };
    let Held::Delivery { document, .. } = held.as_ref() else { panic!("attempt ending leaves the watch open") };
    let Document::Run(run) = document.as_ref() else { panic!("idle run") };
    assert_eq!(run.attempt, None);
    assert!(run.writing.is_none());
    assert!(core.view_subjects.contains_key(&Token::new(30)));
    watch_outputs(&mut core, &limits, Event::StreamDelivered { watcher: Token::new(30) });
    let proof = core.proofs.get_mut(&1).unwrap();
    proof.attempt = 8;
    proof.turn = None;
    proof.terminal = None;
    let next = run_text_outputs(&mut core, &limits, 8, 1, 0, b"new");
    assert_stream(&next[0], 30, views::TurnName { attempt: 8, turn: 1, retry: 0 }, 0, 3, b"new", false);
}

#[test]
fn an_expired_run_signin_receives_no_next_provisional_suffix() {
    let (mut core, limits) = watch_core();
    open_run_watch(&mut core, &limits, 30);
    run_text_outputs(&mut core, &limits, 7, 1, 0, b"first");
    run_text_outputs(&mut core, &limits, 7, 1, 5, b"secret");
    let at = Duration::from_secs(61).as_nanos();
    let Requests::Out(mut out) = step(
        &mut core,
        &Env { now: Time::from_nanos(at), wall: Wall::from_nanos(at), limits },
        Event::StreamDelivered { watcher: Token::new(30) },
    );
    let Request::Held(held) = out.pop().unwrap() else { panic!("held expiry ending") };
    let Held::WatchEnded { why, .. } = held.as_ref() else { panic!("no provisional suffix after expiry") };
    assert_eq!(*why, views::End::SignedOut);
    assert_eq!(out.len(), 1);
}

fn read_start(core: &mut Core, limits: &Limits, sign_in: u64, read: reads::Read) -> Held {
    let outputs = watch_outputs(core, limits, Event::Read { reply: ReplyTo::new(Token::new(90)), sign_in, read });
    let mut held = None;
    for output in outputs {
        match output {
            Request::Held(answer) => {
                assert!(held.is_none(), "one answer per read");
                held = Some(*answer);
            }
            Request::Decided => {}
            Request::ProjectMade { .. } | Request::Write(_) | Request::Ask { .. } | Request::Now(_) => {
                panic!("read changes nothing")
            }
        }
    }
    held.expect("one terminal answer")
}

fn loaded_answer(core: &mut Core, limits: &Limits, load: Token, result: reads::Loaded) -> Option<reads::Answer> {
    let mut answer = None;
    for output in watch_outputs(core, limits, Event::ReadLoaded { load, result }) {
        match output {
            Request::Held(held) => {
                let Held::Read { answer: value, .. } = *held else { panic!("one held terminal") };
                assert!(answer.is_none());
                answer = Some(value);
            }
            Request::Decided => {}
            Request::ProjectMade { .. } | Request::Write(_) | Request::Ask { .. } | Request::Now(_) => {
                panic!("read decides nothing")
            }
        }
    }
    answer
}

fn read_answer(core: &mut Core, limits: &Limits, sign_in: u64, read: reads::Read) -> Held {
    let held = read_start(core, limits, sign_in, read);
    if let Held::ReadLoad { load, .. } = held {
        return Held::Read {
            reply: ReplyTo::new(Token::new(90)),
            answer: loaded_answer(core, limits, load, reads::Loaded::Page { rows: Box::new([]), next: None }).unwrap(),
        };
    }
    held
}

#[expect(clippy::manual_map, reason = "the test helper follows the exhaustive option style")]
pub(crate) fn stored_row(value: Record, owner: Option<tasks::TaskRecord>) -> reads::Row {
    reads::Row {
        value,
        owner: match owner {
            Some(value) => Some(Box::new(value)),
            None => None,
        },
        ancestors: Box::new([]),
        related: None,
        unjoined: None,
        ended_read: None,
        linked: Box::new([]),
        inbox: None,
    }
}

#[test]
fn stored_activity_pages_follow_committed_positions_with_exact_continuations() {
    let (mut core, mut limits) = watch_core();
    limits.documents.rows = 2;
    let owner = core.tasks.task(1).unwrap().clone();
    let line = |position, at| tasks::ActivityLine {
        task: 1,
        position,
        at: Wall::from_nanos(at),
        line: tasks::Line::Words {
            message: position,
            by: tasks::Party::Person(1),
            kind: tasks::MessageKind::Words,
            words: Box::new(*b"whole original words"),
        },
    };
    let Held::ReadLoad { load, range } =
        read_start(&mut core, &limits, 10, reads::Read::Activity { task: 1, before: None })
    else {
        panic!()
    };
    assert!(range.backwards);
    assert_eq!(range.rows, 2);
    let rows = Box::new([
        stored_row(Record::Tasks(tasks::Stored::Activity(line(3, 10))), Some(owner.clone())),
        stored_row(Record::Tasks(tasks::Stored::Activity(line(2, 30))), Some(owner.clone())),
    ]);
    let next = reads::row_key(&rows[1]).unwrap();
    let reads::Answer::Page(page) =
        loaded_answer(&mut core, &limits, load, reads::Loaded::Page { rows, next: Some(next.clone()) }).unwrap()
    else {
        panic!()
    };
    assert_eq!(page.rows.len(), 2);
    let reads::PageRow::Activity { line: first, .. } = page.rows.get(0).unwrap() else { panic!() };
    assert_eq!(first.position, 3);
    core.reclaim();
    let Held::ReadLoad { load, range } =
        read_start(&mut core, &limits, 10, reads::Read::Activity { task: 1, before: page.next })
    else {
        panic!()
    };
    assert_eq!(range.after, Some(next));
    let reads::Answer::Page(last) = loaded_answer(
        &mut core,
        &limits,
        load,
        reads::Loaded::Page {
            rows: Box::new([stored_row(Record::Tasks(tasks::Stored::Activity(line(1, 20))), Some(owner))]),
            next: None,
        },
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(last.rows.len(), 1);
    assert_eq!(last.next, None);
    assert!(loaded_answer(&mut core, &limits, load, reads::Loaded::Gone).is_none());
}

#[test]
fn store_load_admission_is_bounded_and_each_terminal_consumes_its_slot() {
    let (mut core, mut limits) = watch_core();
    limits.load_slots = 1;
    // Reconstruct with the chosen capacity; authentication is restored below.
    core.reads = skein_lib::Slab::with_capacity(1);
    let Held::ReadLoad { load, .. } = read_start(&mut core, &limits, 10, reads::Read::Task { task: 100 }) else {
        panic!()
    };
    let Held::Read { answer, .. } = read_start(&mut core, &limits, 10, reads::Read::Task { task: 101 }) else {
        panic!()
    };
    assert_eq!(answer, reads::Answer::None(reads::Why::Busy));
    assert_eq!(core.reads.len(), 1);
    assert_eq!(
        loaded_answer(&mut core, &limits, load, reads::Loaded::Busy),
        Some(reads::Answer::None(reads::Why::Busy))
    );
    core.reclaim();
    let Held::ReadLoad { load: fresh, .. } = read_start(&mut core, &limits, 10, reads::Read::Task { task: 101 }) else {
        panic!()
    };
    assert_ne!(fresh, load);
    assert!(loaded_answer(&mut core, &limits, load, reads::Loaded::Gone).is_none());
    assert_eq!(
        loaded_answer(&mut core, &limits, fresh, reads::Loaded::Gone),
        Some(reads::Answer::None(reads::Why::Gone))
    );
}

#[test]
fn stored_read_refuses_rows_over_either_bound_and_cursors_from_another_query() {
    let (mut core, mut limits) = watch_core();
    limits.documents.rows = 1;
    limits.load_bytes = 1;
    let owner = core.tasks.task(1).unwrap().clone();
    let Held::ReadLoad { load, .. } =
        read_start(&mut core, &limits, 10, reads::Read::Activity { task: 1, before: None })
    else {
        panic!()
    };
    assert_eq!(
        loaded_answer(
            &mut core,
            &limits,
            load,
            reads::Loaded::Page {
                rows: Box::new([stored_row(Record::Tasks(tasks::Stored::Ended(Box::new(owner))), None)]),
                next: None
            }
        ),
        Some(reads::Answer::None(reads::Why::Busy))
    );
    let Held::Read { answer, .. } = read_start(
        &mut core,
        &limits,
        10,
        reads::Read::Activity {
            task: 1,
            before: Some(reads::Cursor::Store { scope: reads::Scope::Activity { task: 2 }, after: None }),
        },
    ) else {
        panic!()
    };
    assert_eq!(answer, reads::Answer::None(reads::Why::Unknown));
}

#[test]
fn retained_task_read_keeps_cancellation_actor_time_and_full_spec() {
    let (mut core, limits) = watch_core();
    let mut owner = core.tasks.task(1).unwrap().clone();
    owner.number = 99;
    owner.phase = tasks::Phase::Ended(tasks::Ending::Cancelled {
        by: tasks::Party::Person(1),
        at: Wall::from_nanos(12),
        reason: Box::new(*b"released"),
        result: None,
    });
    owner.ended_at = Some(Wall::from_nanos(13));
    let expected = owner.spec.clone();
    let Held::ReadLoad { load, .. } = read_start(&mut core, &limits, 10, reads::Read::Task { task: 99 }) else {
        panic!()
    };
    let reads::Answer::Historical(value) = loaded_answer(
        &mut core,
        &limits,
        load,
        reads::Loaded::Page {
            rows: Box::new([stored_row(Record::Tasks(tasks::Stored::Ended(Box::new(owner))), None)]),
            next: None,
        },
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(value.by, Some(tasks::Party::Person(1)));
    assert_eq!(value.at, Some(Wall::from_nanos(13)));
    let reads::Past::Task { document, result } = value.value else { panic!() };
    assert_eq!(document.chip.number, 99);
    assert_eq!(document.words.bytes, expected.words);
    let Ending::Cancelled { by, at, .. } = result.ending else { panic!() };
    assert_eq!(by, tasks::Party::Person(1));
    assert_eq!(at, Wall::from_nanos(12));
}

#[test]
fn stored_reply_reads_exact_child_activity_and_preserves_original_word_facts() {
    let (mut core, limits) = watch_core();
    let owner = core.tasks.task(1).unwrap().clone();
    let word = tasks::Word {
        number: 801,
        from: tasks::Party::Task(1),
        kind: tasks::MessageKind::Words,
        words: Box::new(*b"exact original reply"),
        at: Wall::from_nanos(17),
        hits: 2,
        eligible: true,
    };
    let line = tasks::ActivityLine {
        task: 1,
        position: 8,
        at: word.at,
        line: tasks::Line::Notified { recipient: tasks::Party::Person(1), word: word.clone() },
    };
    let Held::ReadLoad { load, .. } = read_start(&mut core, &limits, 10, reads::Read::Reply { task: 1, activity: 8 })
    else {
        panic!()
    };
    let reads::Answer::Historical(value) = loaded_answer(
        &mut core,
        &limits,
        load,
        reads::Loaded::Page {
            rows: Box::new([stored_row(Record::Tasks(tasks::Stored::Activity(line)), Some(owner))]),
            next: None,
        },
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(value.value, reads::Past::Reply(word));
}

#[test]
fn stored_person_forms_load_dependency_results_and_retained_links() {
    let (mut core, limits) = watch_core();
    let mut owner = core.tasks.task(1).unwrap().clone();
    owner.number = 100;
    owner.executor = tasks::Executor::Person(tasks::PersonAddress::Person(1));
    owner.spec.inputs = Box::new([99]);
    owner.phase = tasks::Phase::Ended(tasks::Ending::Cancelled {
        by: tasks::Party::Person(1),
        at: Wall::from_nanos(8),
        reason: Box::new([]),
        result: None,
    });
    let mut dependency = owner.clone();
    dependency.number = 99;
    dependency.spec.inputs = Box::new([]);
    dependency.ended_at = Some(Wall::from_nanos(8));
    let mut row = stored_row(Record::Tasks(tasks::Stored::Ended(Box::new(owner))), None);
    row.linked = Box::new([Box::new(dependency)]);
    let Held::ReadLoad { load, .. } = read_start(&mut core, &limits, 10, reads::Read::PersonTask { task: 100 }) else {
        panic!()
    };
    let reads::Answer::Historical(value) =
        loaded_answer(&mut core, &limits, load, reads::Loaded::Page { rows: Box::new([row]), next: None }).unwrap()
    else {
        panic!()
    };
    let reads::Past::PersonTask { document, .. } = value.value else { panic!() };
    let shown = document.results.get(0).unwrap();
    assert!(shown.result.is_some());
    let TaskLink::Retained { chip, at } = &shown.task else { panic!() };
    assert_eq!(chip.number, 99);
    assert_eq!(*at, Some(Wall::from_nanos(8)));
}

#[test]
fn a_load_completion_rechecks_current_project_visibility() {
    let (mut core, limits) = watch_core();
    let owner = core.tasks.task(1).unwrap().clone();
    let Held::ReadLoad { load, .. } =
        read_start(&mut core, &limits, 10, reads::Read::Activity { task: 1, before: None })
    else {
        panic!()
    };
    let mut out = Queue::with_capacity(people::max_out(&limits.people));
    people::step(
        &mut core.people,
        &Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.people },
        people::Event::Roles { project: 1, holdings: Box::new([]) },
        &mut out,
    );
    assert_eq!(
        loaded_answer(
            &mut core,
            &limits,
            load,
            reads::Loaded::Page {
                rows: Box::new([stored_row(Record::Tasks(tasks::Stored::Live(Box::new(owner))), None)]),
                next: None
            }
        ),
        Some(reads::Answer::None(reads::Why::Unknown))
    );
}

#[test]
fn live_reads_use_owner_documents_without_a_key_or_state_change() {
    let (mut core, limits) = watch_core();
    for (read, subject) in [
        (reads::Read::Frame, Subject::Frame { party: 1 }),
        (reads::Read::Task { task: 1 }, Subject::Task { task: 1 }),
        (reads::Read::Project { project: 1 }, Subject::Project { project: 1 }),
        (reads::Read::Deployment, Subject::Deployment),
    ] {
        let expected = make_with_clock(
            &core,
            &limits,
            1,
            &subject,
            &Window { rows: limits.documents.rows, after: None },
            DocumentClock { now: Time::ZERO, wall: Wall::EPOCH },
        )
        .unwrap();
        let Held::Read { reply, answer: reads::Answer::Document(document) } = read_answer(&mut core, &limits, 10, read)
        else {
            panic!("live document expected")
        };
        assert_eq!(reply.into_token(), Token::new(90));
        assert_eq!(*document, expected);
    }
}

#[test]
fn invisible_and_absent_reads_have_the_same_unknown_answer() {
    let (mut core, limits) = watch_core();
    for (sign_in, read, why) in [
        (99, reads::Read::Frame, reads::Why::SignedOut),
        (10, reads::Read::Project { project: 9 }, reads::Why::Unknown),
        (10, reads::Read::Task { task: 99 }, reads::Why::Unknown),
    ] {
        let Held::Read { answer, .. } = read_answer(&mut core, &limits, sign_in, read) else {
            panic!("read terminal expected")
        };
        assert_eq!(answer, reads::Answer::None(why));
    }
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.people };
    let mut out = Queue::with_capacity(people::max_out(&limits.people));
    people::step(
        &mut core.people,
        &env,
        people::Event::Restore { record: people::Stored::Roles { project: 1, holdings: Box::new([]) } },
        &mut out,
    );
    let Held::Read { answer, .. } = read_answer(&mut core, &limits, 10, reads::Read::Task { task: 1 }) else {
        panic!("read terminal expected")
    };
    assert_eq!(answer, reads::Answer::None(reads::Why::Unknown));
}

#[test]
fn a_core_read_answer_waits_for_the_last_commit_already_made() {
    use skein_lib::{Journal, JournalLimits, JournalRoom, Released};
    let (mut core, limits) = watch_core();
    let journal_limits = JournalLimits { commits: 2, writes: 1, held: 4, now: 1, release: 4 };
    let mut journal = Journal::<u64, Held>::new(&journal_limits);
    let room = JournalRoom { writes: 1, held: 1 };
    let mut prior = journal.decision(&room).unwrap();
    prior.write(123).unwrap();
    journal.accept(prior);
    let commit = journal.commit().unwrap().number;
    let mut read = journal.decision(&room).unwrap();
    read.hold(read_answer(&mut core, &limits, 10, reads::Read::Task { task: 1 })).unwrap();
    journal.accept(read);
    assert!(journal.commit().is_none(), "read makes no commit");
    let mut released = Queue::with_capacity(4);
    assert_eq!(journal.release(&mut released), Released::None);
    assert!(released.is_empty());
    journal.committed(commit);
    assert_eq!(journal.release(&mut released), Released::Some);
    let Some(Held::Read { answer: reads::Answer::Document(_), .. }) = released.pop() else {
        panic!("held document released after durability")
    };
    assert!(released.is_empty());
}

#[test]
fn deployment_capacity_carries_keyed_refusal_identity_time_and_bounded_omissions() {
    let (mut core, mut limits) = make_tree();
    limits.people.projects = 2;
    limits.documents.lines = 1;
    restore_document_party(&mut core, &limits, &[0, 1]);
    let identity = core.people.identity(1).unwrap().clone();
    let mut env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.people };
    let mut out = Queue::with_capacity(people::max_out(&limits.people));
    people::step(
        &mut core.people,
        &env,
        people::Event::SignedIn {
            held: None,
            reply_to: ReplyTo::new(Token::new(500)),
            person: 10,
            sign_in: 20,
            identity,
            kind: people::Kind::Person,
        },
        &mut out,
    );
    while out.pop().is_some() {}
    for key in 1..=3_u8 {
        people::step(
            &mut core.people,
            &env,
            people::Event::Ask {
                models: Box::new([]),
                reply_to: ReplyTo::new(Token::new(501)),
                sign_in: 20,
                key: [key; 16],
                ask: people::Ask::Create { project: 1, batch: Box::new([fixtures::member(b"chat", 1)]) },
            },
            &mut out,
        );
        let mut routed = None;
        while let Some(request) = out.pop() {
            match request {
                people::Request::Route { request, .. } => routed = Some(request),
                people::Request::Act { .. }
                | people::Request::Acted { .. }
                | people::Request::Save { .. }
                | people::Request::Erase { .. }
                | people::Request::Reply { .. }
                | people::Request::ServiceMade { .. }
                | people::Request::RolesApplied { .. }
                | people::Request::RolesRefused { .. }
                | people::Request::RestoreRefused { .. } => {}
            }
        }
        env.wall = Wall::from_nanos(u64::from(key));
        people::step(
            &mut core.people,
            &env,
            people::Event::Decided {
                request: routed.expect("one keyed route"),
                outcome: people::Outcome::Refused(people::Refusal::Limit { which: people::LimitKind::LiveTasks }),
            },
            &mut out,
        );
        while out.pop().is_some() {}
        core.people.reclaim();
    }
    let Document::Deployment(document) = make(&core, &limits, 1, &Subject::Deployment, &window(), Wall::EPOCH).unwrap()
    else {
        panic!()
    };
    assert!(document.cut);
    assert_eq!(document.person_room_refusals.len(), 1);
    let refusal = document.person_room_refusals.get(0).unwrap();
    assert_eq!(refusal.key, people::RequestKey { person: 1, key: [3; 16] });
    assert_eq!(refusal.project, Some(1));
    assert_eq!(refusal.action, people::CapacityAction::Create);
    assert_eq!(refusal.at, Wall::from_nanos(3));
    assert_eq!(refusal.cause, people::CapacityCause::Limit { which: people::LimitKind::LiveTasks });
    assert_eq!(document.person_room_refusals_forgotten, 0);
    assert_eq!(document.person_answer_retention, limits.people.request_retention);
}

#[test]
fn goal_deadlines_are_checked_against_the_role_and_policy() {
    let (mut core, _) = core();
    let deadline = Wall::from_nanos(2_000_000_000);
    let earlier = Wall::from_nanos(1_000_000_000);
    let later = Wall::from_nanos(3_000_000_000);
    let mut policy = core.authority.policy(1).expect("configured project").clone();
    policy.roles[0].authority.budget.deadline = Some(deadline);
    let mut out = Queue::with_capacity(authority::POLICY_MAX_OUT);
    authority::step(&mut core.authority, authority::Event::Policy { project: 1, policy: policy.clone() }, &mut out);
    let started = crate::acts::bounds::create_start(
        &core,
        1,
        1,
        people::Role::Owner,
        tasks::Executor::Agent { charter: 1 },
        10,
        Some(earlier),
        2,
    )
    .expect("within role");
    assert_eq!(started.authority.budget.spend, 10);
    assert_eq!(started.authority.budget.deadline, Some(earlier));
    assert_eq!(
        crate::acts::bounds::create_start(
            &core,
            1,
            1,
            people::Role::Owner,
            tasks::Executor::Agent { charter: 1 },
            10,
            Some(later),
            2
        )
        .unwrap_err(),
        people::Refusal::Authority { lacking: people::Lack::Deadline, proposable: Some(people::Accepter::Role(0)) }
    );
    assert_eq!(
        crate::acts::bounds::create_start(
            &core,
            1,
            1,
            people::Role::Owner,
            tasks::Executor::Agent { charter: 1 },
            10,
            None,
            2
        )
        .unwrap_err(),
        people::Refusal::Authority { lacking: people::Lack::Deadline, proposable: Some(people::Accepter::Role(0)) }
    );
    policy.ceiling.budget.deadline = Some(deadline);
    while out.pop().is_some() {}
    authority::step(&mut core.authority, authority::Event::Policy { project: 1, policy }, &mut out);
    assert_eq!(
        crate::acts::bounds::create_start(
            &core,
            1,
            1,
            people::Role::Owner,
            tasks::Executor::Agent { charter: 1 },
            10,
            Some(later),
            2
        )
        .expect_err("beyond policy"),
        people::Refusal::Authority { proposable: Some(people::Accepter::Nobody), lacking: people::Lack::Deadline }
    );
}

#[test]
fn a_person_result_document_uses_the_durable_answer_instead_of_the_cleared_claim() {
    let (core, limits) = make_tree();
    let mut row = core.tasks.task(3).expect("task").clone();
    row.executor = tasks::Executor::Person(tasks::PersonAddress::Role(0));
    row.phase =
        tasks::Phase::Ended(tasks::Ending::Done(tasks::TaskResult::Report { words: b"chosen".as_slice().into() }));
    row.taken_by = None;
    row.person_answer = Some(tasks::PersonAnswer { by: 1, at: Wall::from_nanos(9) });
    assert_eq!(result_from_row(&core, &limits, 1, &row, Wall::EPOCH).expect("role answer").chosen_by, Some(1));
    row.executor = tasks::Executor::Person(tasks::PersonAddress::Person(1));
    assert_eq!(result_from_row(&core, &limits, 1, &row, Wall::EPOCH).expect("direct answer").chosen_by, Some(1));
    row.person_answer = None;
    row.phase = tasks::Phase::Ended(tasks::Ending::Cancelled {
        by: tasks::Party::Person(1),
        at: Wall::from_nanos(10),
        reason: Box::new([]),
        result: None,
    });
    assert_eq!(result_from_row(&core, &limits, 1, &row, Wall::EPOCH).expect("cancelled").chosen_by, None);
}

#[test]
fn deployment_capacity_carries_direct_batch_refusals_and_task_history_cut() {
    let (mut core, mut limits) = make_tree();
    limits.people.projects = 2;
    limits.documents.lines = 1;
    limits.tasks.tasks = 4;
    restore_document_party(&mut core, &limits, &[0, 1]);
    for number in 5..=7 {
        let new = new_row(&core, number, tasks::Funder::Task(1), Some(4));
        step(
            &mut core,
            &Env { now: Time::ZERO, wall: Wall::from_nanos(number), limits },
            Event::Tasks(tasks::Event::Make {
                reply_to: ReplyTo::new(Token::new(number)),
                creator: tasks::Party::Task(1),
                batch: Box::new([new]),
            }),
        );
    }
    let Document::Deployment(document) = make(&core, &limits, 1, &Subject::Deployment, &window(), Wall::EPOCH).unwrap()
    else {
        panic!("deployment")
    };
    assert!(document.cut);
    assert_eq!(document.task_room_refusals.len(), 1);
    assert_eq!(document.task_room_refusals_forgotten, 1);
    let fact = document.task_room_refusals.get(0).unwrap();
    assert_eq!(fact.at, Wall::from_nanos(7));
    assert_eq!(fact.request.project, Some(1));
    assert_eq!(fact.request.by, tasks::Party::Task(1));
    assert_eq!(fact.request.action, tasks::CapacityAction::Batch { first: Some(7), members: 1 });
    assert_eq!(fact.request.cause, tasks::CapacityCause::Tasks { limit: 4 });
    assert!(core.tasks.task(7).is_none());
}
mod application_documents;

mod acts;
mod chip_phase;
mod hold_origin;
mod live_delegates;

mod document_offers;
mod last_batch;
mod periods;

mod frame_deployment_role;
mod frame_facts;
mod project_baselines;

mod deployment_connectors;

#[test]
fn approval_documents_keep_opaque_state_whole_and_refuse_a_smaller_bound() {
    let (mut core, limits) = make_tree();
    let mut row = new_row(&core, 5, tasks::Funder::Period { project: 1, period: 1 }, None);
    row.executor = tasks::Executor::Person(tasks::PersonAddress::Person(1));
    let part = Part { connector: 1, kind: PartKind::Resource, key: PartKey::Number(5) };
    row.contract = tasks::Contract::Approval { state: Box::from(&b"opaque\x00\xff"[..]), part: part.clone() };
    send_task(
        &mut core,
        &limits,
        tasks::Event::Make {
            reply_to: ReplyTo::new(Token::new(450)),
            creator: tasks::Party::Person(1),
            batch: Box::new([row]),
        },
    );
    let subject = Subject::PersonTask { task: 5 };
    let Document::PersonTask(task) = make(&core, &limits, 1, &subject, &window(), Wall::EPOCH).unwrap() else {
        panic!("person approval");
    };
    assert_eq!(task.contract, ContractLine::Approval { state: Box::from(&b"opaque\x00\xff"[..]), part });
    let mut smaller = limits;
    smaller.documents.text_bytes = 3;
    assert_eq!(make(&core, &smaller, 1, &subject, &window(), Wall::EPOCH), Err(Missing::TooLarge));
    let mut row = core.tasks.task(5).unwrap().clone();
    row.phase = tasks::Phase::Ended(tasks::Ending::Done(tasks::TaskResult::Approved {
        state: Box::from(&b"opaque\x00\xff"[..]),
        words: None,
    }));
    let result = result_from_row(&core, &limits, 1, &row, Wall::EPOCH).unwrap();
    assert_eq!(
        result.ending,
        Ending::Done(ResultValue::Approved { state: Box::from(&b"opaque\x00\xff"[..]), words: None })
    );
    assert_eq!(result_from_row(&core, &smaller, 1, &row, Wall::EPOCH), Err(Missing::TooLarge));
}

mod chip_executor;
mod task_settings;
mod task_specification;

#[test]
fn verdict_documents_preserve_machine_field_rules_and_values_whole() {
    let (mut core, limits) = make_tree();
    let mut row = new_row(&core, 5, tasks::Funder::Period { project: 1, period: 1 }, None);
    let rule = tasks::FieldRule { name: b"state".as_slice().into(), max: 8 };
    row.contract = tasks::Contract::Verdict {
        choices: Box::new([tasks::Verdict { code: 2, words: 4, followups: 0, fields: Box::new([rule.clone()]) }]),
    };
    send_task(
        &mut core,
        &limits,
        tasks::Event::Make {
            reply_to: ReplyTo::new(Token::new(461)),
            creator: tasks::Party::Person(1),
            batch: Box::new([row]),
        },
    );
    let Document::Task(task) = make(&core, &limits, 1, &Subject::Task { task: 5 }, &window(), Wall::EPOCH).unwrap()
    else {
        panic!("task document");
    };
    let ContractLine::Verdict { choices, cut } = task.contract else {
        panic!("verdict contract");
    };
    assert!(!cut);
    assert_eq!(choices.get(0).expect("one choice").fields.as_ref(), &[rule]);
    let fields: Box<[tasks::ResultField]> =
        Box::new([tasks::ResultField { name: b"state".as_slice().into(), value: b"\x00\xff".as_slice().into() }]);
    let mut row = core.tasks.task(5).unwrap().clone();
    row.phase = tasks::Phase::Ended(tasks::Ending::Done(tasks::TaskResult::Verdict {
        code: 2,
        words: Box::new([]),
        fields: fields.clone(),
    }));
    assert_eq!(
        result_from_row(&core, &limits, 1, &row, Wall::EPOCH).unwrap().ending,
        Ending::Done(ResultValue::Verdict { code: 2, fields, words: Text { bytes: Box::new([]), cut: false } })
    );
    let mut smaller = limits;
    smaller.documents.text_bytes = 3;
    assert_eq!(make(&core, &smaller, 1, &Subject::Task { task: 5 }, &window(), Wall::EPOCH), Err(Missing::TooLarge));
    assert_eq!(result_from_row(&core, &smaller, 1, &row, Wall::EPOCH), Err(Missing::TooLarge));
}

mod engine_timers;
mod sign_ins;

mod fill;
mod inbox_selection;
mod restore_admission;

mod project_period_watches;

mod fixtures;
mod reserve;
mod waits;

mod charters;

#[test]
fn an_answer_payload_for_a_replaced_attempt_is_dropped() {
    let (mut core, limits) = watch_core();
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits };
    let event = Event::AnswerPayload {
        grants_refused: false,
        read: None,
        refused: Box::new([]),
        run: Token::new(1),
        attempt: Token::new(6),
        payload: Token::new(91),
        cumulative: 0,
        end: tasks::End::Parked,
        saved: None,
        invalid_saved: false,
    };
    let Requests::Out(mut out) = step(&mut core, &env, event);
    let Request::Now(answer) = out.pop().unwrap() else { panic!("payload drop") };
    let Now::DropPayload { payload } = *answer else { panic!("drop stale answer") };
    assert_eq!(payload, Token::new(91));
    assert!(core.current_proof(1, 7), "the replacement proof remains current");
}

mod account_signing;
mod accounts_document;
mod credentials;

mod funding_fixtures;
mod windows;
use funding_fixtures::{set_role_authority, set_seed_role_authority};

mod read_activity;

mod own_watches;

mod default_llm;
mod goal_of;
mod kept_models;
mod role_models;
mod task_query;

mod brief_revisions;

mod escalation_holder;
mod virtual_words;

mod templates;

mod forget;

mod coverage;

mod open_shape;
mod run_choice;

mod account_admission;

#[test]
fn outcomes_reserve_one_more_than_configured_workers() {
    let mut limits = limits();
    limits.people.program_hosts = limits.fleet.configured_workers;
    assert_eq!(worst_case(&limits), None);
}
#[test]
fn outcome_names_cover_the_configured_name_bound() {
    let mut limits = limits();
    limits.people.program_host_name_bytes = limits.fleet.worker_name_bytes - 1;
    assert_eq!(worst_case(&limits), None);
}

#[test]
fn startup_refuses_too_many_configured_workers() {
    let mut limits = limits();
    let config = config(41);
    limits.fleet.configured_workers = 1;
    assert_eq!(Core::new(config, &limits).err(), Some(Problem::Workers { limit: 1 }));
}
#[test]
fn startup_refuses_an_oversized_worker_name() {
    let mut config = config(41);
    let limits = limits();
    config.worker_names = Box::new([configured_worker_name(limits.fleet.worker_name_bytes + 1)]);
    assert_eq!(
        Core::new(config, &limits).err(),
        Some(Problem::WorkerName { worker: 0, limit: limits.fleet.worker_name_bytes })
    );
}

#[test]
fn startup_accepts_the_configured_worker_count_bound() {
    let config = config(41);
    let mut limits = limits();
    limits.fleet.configured_workers = u32::try_from(config.worker_names.len()).expect("fixture worker count");
    let _core = Core::new(config, &limits).expect("configured membership at its bound");
}
#[test]
fn startup_accepts_the_worker_name_byte_bound() {
    let mut config = config(41);
    let limits = limits();
    config.worker_names = Box::new([configured_worker_name(limits.fleet.worker_name_bytes)]);
    let _core = Core::new(config, &limits).expect("configured membership at its bound");
}

fn configured_worker_name(length: u32) -> Box<[u8]> {
    let mut name = skein_lib::bytes::zeroed(usize::try_from(length).expect("fixture name bound"));
    name.fill(b'a');
    name
}

mod result_reads;

mod amendment_reads;

#[test]
fn finishing_owes_header_changed_marks_and_first_drain_but_not_future_alarms() {
    let (mut core, limits) = watch_core();
    assert!(core.finish_owed(), "bootstrap header is owed");
    core.counters.next_commit();
    assert!(!core.finish_owed(), "future sign-in alarms are independent of finishing");
    assert!(core.people.next_deadline().unwrap() > Time::ZERO);
    assert_decided_only(finish_watch_outputs(&mut core, &limits).as_ref());
    let watcher = Token::new(30);
    watch_outputs(&mut core, &limits, Event::Watch { watcher, sign_in: 10, subject: Watched::Frame, window: window() });
    watch_outputs(&mut core, &limits, Event::Delivered { watcher });
    assert!(!core.finish_owed());
    core.touch_party(1);
    assert!(core.finish_owed());
    assert!(!core.quiescent(true), "broader idle includes the same finishing derivation");
    let out = finish_watch_outputs(&mut core, &limits);
    let [Request::Held(_), Request::Decided] = out.as_ref() else { panic!("one changed delivery") };
    assert!(!core.finish_owed(), "an inflight delivery is a child-held wait");
    watch_outputs(&mut core, &limits, Event::Stopping);
    assert!(core.finish_owed());
    assert_eq!(finish_watch_outputs(&mut core, &limits).len(), 1, "ending awaits the delivery terminal");
    assert!(core.views.is_closing(watcher));
    assert!(!core.finish_owed(), "closing must not repeatedly owe an ending");
    let ended = watch_outputs(&mut core, &limits, Event::Delivered { watcher });
    assert_one_watch_ending(&ended, views::End::Stopping);
    assert!(core.finish_owed(), "empty bindings still owe the first drain");
    let drained = finish_watch_outputs(&mut core, &limits);
    let [Request::Held(held), Request::Decided] = drained.as_ref() else { panic!("one held drain") };
    let Held::Drained = held.as_ref() else { panic!("drained terminal") };
    assert!(!core.finish_owed());
    assert_decided_only(finish_watch_outputs(&mut core, &limits).as_ref());
}

#[test]
fn retained_writing_reconciles_public_proof_changes_even_without_watchers() {
    for change in 0_u8..4 {
        let (mut core, limits) = watch_core();
        core.counters.next_commit();
        run_text_outputs(&mut core, &limits, 7, 1, 0, b"unwatched");
        assert_eq!(core.views.runs(), 1);
        assert!(!core.finish_owed(), "current provisional writing waits on its host");
        match change {
            0 => core.proofs.get_mut(&1).unwrap().turn = Some(TurnProof { turn: 1, cumulative: 0, read: None }),
            1 => core.proofs.get_mut(&1).unwrap().attempt = 8,
            2 => {
                core.proofs.remove(&1);
            }
            3 => {
                core.proofs.get_mut(&1).unwrap().terminal =
                    Some(TerminalRecord { task: 1, attempt: 7, cumulative: 0, end: tasks::End::Parked });
            }
            _ => unreachable!(),
        }
        assert!(core.finish_owed());
        assert_decided_only(finish_watch_outputs(&mut core, &limits).as_ref());
        assert!(!core.finish_owed());
        assert_eq!(core.views.runs(), u32::from(change == 0));
        if change == 0 {
            assert!(core.views.retained_run_states().next().unwrap().1.metadata().2);
            assert_decided_only(finish_watch_outputs(&mut core, &limits).as_ref());
        }
    }
}

#[test]
fn finishing_inspects_membership_role_and_query_visibility_without_clocks() {
    for change in 0_u8..5 {
        let (mut core, limits) = watch_core();
        core.counters.next_commit();
        let watcher = Token::new(30);
        let subject = if change == 3 {
            Watched::Query {
                query: TaskQuery {
                    project: Some(1),
                    requester: None,
                    root: None,
                    parent: None,
                    label: None,
                    phase: None,
                    executor: None,
                    tracked: None,
                    order: QueryOrder::Priority,
                },
            }
        } else {
            Watched::Task { task: 1 }
        };
        watch_outputs(&mut core, &limits, Event::Watch { watcher, sign_in: 10, subject, window: window() });
        watch_outputs(&mut core, &limits, Event::Delivered { watcher });
        assert!(!core.finish_owed());
        match change {
            0 => {
                core.view_subjects.get_mut(&watcher).unwrap().sign_in = 999;
            }
            1 => {
                core.view_subjects.get_mut(&watcher).unwrap().party = 2;
            }
            2 | 3 => {
                let mut ignored = Queue::with_capacity(people::max_out(&limits.people));
                people::step(
                    &mut core.people,
                    &Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.people },
                    people::Event::Roles { project: 1, holdings: Box::new([]) },
                    &mut ignored,
                );
            }
            4 => core.tasks = fixtures::fresh_tasks(&core, &limits),
            _ => unreachable!(),
        }
        assert!(core.finish_owed());
        let ended = finish_watch_outputs(&mut core, &limits);
        let expected = match change {
            0 | 1 => views::End::SignedOut,
            2 | 3 => views::End::NotAllowed,
            4 => views::End::Gone,
            _ => unreachable!(),
        };
        assert_one_watch_ending(&ended, expected);
        assert!(!core.finish_owed());
    }
}

#[test]
fn projection_readiness_shares_generation_restore_and_pending_connector_rules() {
    let (mut core, limits) = make_tree();
    core.counters.next_commit();
    assert!(core.finish_owed());
    finish_watch_outputs(&mut core, &limits);
    assert!(!core.finish_owed());
    let commit = core.counters.deployment().commits;
    let state = core.projections.get_mut(&1).unwrap();
    state.milestones = Box::new([ProjectionMilestone {
        identity: MilestoneId::Lifecycle { task: 1, position: 1 },
        phase: Some(tasks::Phase::Waiting),
        change: None,
        words: Box::new([]),
    }]);
    state.fed_at = Some(commit);
    assert!(!core.finish_owed(), "same-generation unread milestones wait");
    assert_decided_only(finish_watch_outputs(&mut core, &limits).as_ref());
    core.counters.next_commit();
    assert!(core.finish_owed(), "public commit generation makes the milestone eligible");
    finish_watch_outputs(&mut core, &limits);
    assert!(!core.finish_owed());
    core.counters.next_commit(); // accept the preceding projection decision before a new input
    let mut amendment = fixtures::unchanged(b"untrack");
    amendment.tracked = Some(tasks::Tracking::Untracked);
    send_task(
        &mut core,
        &limits,
        tasks::Event::Amend {
            places: Box::new([]),
            funder: None,
            reply_to: ReplyTo::new(Token::new(5)),
            by: tasks::Party::Person(1),
            task: 1,
            message: 21,
            stop_run: false,
            amendment,
        },
    );
    assert!(core.tasks.task(1).unwrap().tracked.is_none());
    assert!(core.finish_owed());
    finish_watch_outputs(&mut core, &limits);
    assert!(!core.projections.get(&1).unwrap().pending.is_empty());
    assert!(!core.finish_owed(), "an untracked goal waits on its closing connectors first");
    core.projections.get_mut(&1).unwrap().changed = true;
    assert!(!core.finish_owed(), "pending connectors take precedence over changed");

    // The durable projection header is independently restored and still owes
    // its changed pass; no private dirty flag can stand in for this state.
    let (mut restored, restored_limits) = self::core();
    fresh(&mut restored.counters, Family::Task).unwrap();
    restored.counters.next_commit();
    let row = ProjectionRecord::Goal {
        goal: ProjectionGoal {
            number: 1,
            project: 1,
            priority: 0,
            words: b"restored".as_slice().into(),
            phase: tasks::Phase::Waiting,
        },
        pending: Box::new([]),
        changed: true,
        fed_at: None,
    };
    assert_eq!(restored.restore_core(CoreRecord::Projection(row), &restored_limits), Restored::Live);
    assert!(restored.finish_owed());
    finish_watch_outputs(&mut restored, &restored_limits);
    assert!(!restored.finish_owed());
}

fn assert_decided_only(out: &[Request]) {
    let [Request::Decided] = out else { panic!("only Decided") };
}

fn assert_one_watch_ending(out: &[Request], expected: views::End) {
    let mut endings = 0_u32;
    for request in out {
        if let Request::Held(held) = request
            && let Held::WatchEnded { why, .. } = held.as_ref()
        {
            assert_eq!(*why, expected);
            endings = endings.checked_add(1).expect("bounded watch endings");
        }
    }
    assert_eq!(endings, 1);
}

mod project_names;
mod projects;

#[test]
fn configured_policies_are_retained_by_the_core() {
    let limits = limits();
    let mut config = config(41);
    let policy = config.seed_projects[0].policy.policy.clone();
    config.defaults.policies = Map::with_capacity(limits.authority.projects);
    config
        .defaults
        .policies
        .insert(
            1,
            ConfiguredPolicy {
                policy,
                permissions: Box::new([people::PermissionRole { connector: 1, permission: 1, role: 0 }]),
                templates: Box::new([]),
            },
        )
        .expect("one configured policy");
    let core = Core::new(config, &limits).expect("fixture configuration");
    let policy = core.current.policy(1).expect("retained configured policy");
    assert_eq!(policy.policy.period_spend, 1000);
    assert_eq!(policy.permissions.as_ref(), &[people::PermissionRole { connector: 1, permission: 1, role: 0 }]);
    assert!(policy.templates.is_empty());
}

#[test]
fn a_restored_policy_cannot_pass_the_deployments_projects_mark() {
    let limits = limits();
    let config = config(41);
    let value = policy::snapshot(&config.seed_projects[0].policy.policy, &[], &[]).expect("bounded policy");
    let mut core = Core::new(config, &limits).expect("restoring core");
    core.counters.deployment.projects = 0;
    assert!(!core.restore_policy(&limits, 1, Box::from("Project 1"), value.clone()));
    core.counters.deployment.projects = 1;
    assert!(core.restore_policy(&limits, 1, Box::from("Project 1"), value));
}

mod ended_watches;

mod restart_checks;

fn placed(task: u64, person: u64, number: u64) -> tasks::Event {
    tasks::Event::Place {
        reply_to: ReplyTo::new(Token::new(number)),
        hold: tasks::PlacedHold {
            number,
            project: 1,
            owner: tasks::Owner::Party { person },
            mode: tasks::Mode::Stop,
            scope: tasks::Scope::Subtree { task },
            reason: Box::new([]),
            placed_at: 0,
            revision: 1,
            exempt: Box::new([]),
            lifted: false,
        },
    }
}
