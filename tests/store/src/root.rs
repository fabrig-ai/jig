//! Scripted records, without an engine domain (protocol/store.md, 14).
use jig_domain_core as core;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_protocol_store::{Limits, Write, sizes};
use skein_lib::{Duration, JournalLimits, Wall};
pub const LIVE: u64 = 21;
pub const DEPLOYMENT: [u8; 16] = [18; 16];
pub const APPLICATION: sizes::Application = sizes::Application { version: 1, key: 16, value: 64 };

pub const NOTES_LIMITS: jig_domain_notes::Limits = jig_domain_notes::Limits {
    scopes: 2,
    entries_per_scope: 2,
    pattern_bytes: 16,
    description_bytes: 32,
    body_bytes: 64,
    references: 2,
    load_rows: 2,
    lines: 2,
    recalled: 2,
};

pub const TASKS_LIMITS: jig_domain_tasks::Limits = jig_domain_tasks::Limits {
    circle: 8,
    tasks: 2,
    funders: 2,
    project_tasks: 2,
    project_goals: 8,
    tree_tasks: 2,
    depth: 2,
    delegates: 2,
    references: 2,
    subscriptions: 2,
    batch: 2,
    dependencies: 2,
    holdings: 2,
    hold_kinds: 2,
    resource_reports: 2,
    pools: 2,
    hold_segments: 2,
    hold_bytes: 16,
    grace: skein_lib::Duration::from_secs(600),
    hold_wait: skein_lib::Duration::from_nanos(1),
    hold_waiters: 2,
    inputs: 2,
    spec_bytes: 32,
    parameters: 3,
    result_bytes: 32,
    run_inbox: 2,
    inbox_messages: 2,
    inbox_bytes: 32,
    message_bytes: 16,
    person_stall: skein_lib::Duration::from_nanos(1),
    proposal_stall: skein_lib::Duration::from_nanos(1),
    escalation_stall: skein_lib::Duration::from_nanos(1),
    saved_resources: 2,
    contract_choices: 2,
    charters: 2,
    charter_documents: 2,
    revisions: 4,
    versions: 8,
    authority_grants: 2,
    authority_segments: 2,
    authority_bytes: 32,
    executor_kinds: 3,
    retries: jig_domain_tasks::Retries {
        transient: TASK_RETRY,
        permanent: TASK_RETRY,
        run: TASK_RETRY,
        agent: TASK_RETRY,
        lost: TASK_RETRY,
        invalid: TASK_RETRY,
    },
    facts: 2,
    room_refusals: 2,
};

const TASK_RETRY: jig_domain_tasks::Retry = jig_domain_tasks::Retry {
    retries: 1,
    base: skein_lib::Duration::from_nanos(1),
    max: skein_lib::Duration::from_nanos(2),
};

pub const PEOPLE_LIMITS: people::Limits = people::Limits {
    operations: 8,
    batch: 2,
    people: 2,
    inbox_entries: 2,
    inbox_tasks: 2,
    inbox_recipients: 2,
    sign_ins: 2,
    projects: 2,
    holdings: 2,
    initial_owners: 2,
    requests: 2,
    room_refusals: 2,
    pending: 2,
    waiters: 2,
    program_hosts: 65,
    program_host_name_bytes: 32,
    act_grants: 2,
    identity_bytes: 32,
    words: 32,
    amendment_bytes: 32,
    sign_in_lifetime: skein_lib::Duration::from_nanos(1_000),
    request_retention: skein_lib::Duration::from_nanos(1_000),
    facts: 2,
};

pub const CORE_LIMITS: core::Limits = core::Limits {
    settings: core::settings::Limits {
        revisions: 4,
        versions: 8,
        history: 64,
        proposals: 4,
        alternatives: 16,
        permissions: 32,
    },
    templates: 8,
    fill_mark: 1000,
    documents: core::documents::DocumentLimits {
        grants: 2,
        executors: 3,
        implications: 0,
        roles: 2,
        segments: 2,
        rows: 32,
        text_bytes: 256,
        chips: 32,
        lines: 32,
        versions: core::documents::Versions { minimum: 1, maximum: 1 },
    },
    connectors: 2,
    resume_bytes: 256,
    run_bytes: 256,
    policy_bytes: 256,
    escalation_reason_bytes: 128,
    load_slots: 2,
    load_bytes: 65_536,
    call_records: 2,
    call_answer_bytes: 256,
    tasks: TASKS_LIMITS,
    authority: core_authority_limits(),
    people: PEOPLE_LIMITS,
    fleet: core_fleet_limits(),
    brief: jig_domain_brief::Limits { briefs: 2, sections: 3, read_bytes: 256, brief_bytes: 384 },
    brief_parts: 2,
    brief_core_budgets: core::CoreBriefBudgets {
        tail: 128,
        task: 128,
        dependencies: 128,
        attempts: 128,
        plan: 128,
        notes: 64,
    },
    accounts: jig_domain_accounts::Limits {
        accounts: 1,
        holder_accounts: 1,
        program_names: 12,
        program_hosts: 65,
        sources: 3,
        sign_ins: 1,
        credentials: 2,
        backoff_base: skein_lib::Duration::from_nanos(2_000_000_000),
        backoff_max: skein_lib::Duration::from_nanos(8_000_000_000),
        retry: skein_lib::Duration::from_nanos(30_000_000_000),
        move_deadline: skein_lib::Duration::from_secs(60),
        spent_attention: skein_lib::Duration::from_nanos(1_000_000_000),
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
    notes: NOTES_LIMITS,
};

const fn core_authority_limits() -> jig_domain_authority::Limits {
    jig_domain_authority::Limits {
        projects: 1,
        roles: 2,
        requirements: 1,
        facts: 2,
        grants: 1,
        executors: 2,
        segments: 1,
        segment_bytes: 32,
        implications: 0,
        batch: 1,
        accounts: 1,
        writes: 1,
    }
}

const fn core_fleet_limits() -> jig_domain_fleet::Limits {
    jig_domain_fleet::Limits {
        configured_workers: 64,
        worker_name_bytes: 32,
        workers: 1,
        program_sources: 12,
        source_detail_bytes: 4096,
        needs_bytes: 4096,
        versions: 0,
        engine_slots: 0,
        slots: 1,
        workstreams: 1,
        attempts: 2,
        grants: 8,
        calls: 1,
        call_name_bytes: 64,
        progress_bytes: 64,
        turns: 2,
        grace: skein_lib::Duration::from_nanos(5_000_000_000),
        facts: 2,
        history: 2,
    }
}

#[must_use]
pub fn limits() -> Limits {
    let journal = JournalLimits { commits: 2, writes: 2000, held: 2000, now: 4, release: 4 };
    let mut map = skein_kv::Limits {
        key: 1024,
        // The people family's policy row keeps up to 64 model names (protocol/store.md, 5.1).
        value: 524_288,
        // Two places per journal write, three creation rows and one orphan erase.
        ops: 4004,
        commit: 1_073_741_824,
        queued: 2,
        queued_bytes: 2_147_483_648,
        budget: 65_536,
        segment: 1_073_741_824,
        snapshot_after: 1_073_741_824,
        chunk: 525_332,
        page: skein_kv::Page { rows: 2, bytes: 525_312 },
        deadline: Duration::from_secs(1),
    };
    let sizes = sizes::check(&CORE_LIMITS, &APPLICATION, &journal, &map, 1).expect("scripted record bounds fit");
    map.key = sizes.key;
    map.value = sizes.value;
    map.ops = sizes.ops;
    map.commit = sizes.commit;
    map.queued_bytes = u64::from(sizes.commit) * 2;
    map.chunk = sizes.key + sizes.value + 20;
    map.page.bytes = sizes.key + sizes.value;
    Limits {
        core: CORE_LIMITS,
        store: map,
        secrets: map,
        journal,
        joins: 8,
        queue: 2,
        files: 2,
        reads: 4,
        orphan_erases: 1,
        held: 2,
    }
}

#[must_use]
pub fn header(commits: u64) -> core::Record {
    core::Record::Core(core::CoreRecord::Deployment(core::Deployment {
        id: DEPLOYMENT,
        tasks: LIVE,
        people: 0,
        sign_ins: 0,
        messages: 0,
        runs: 1,
        calls: 0,
        connector_rows: 0,
        projects: 0,
        applications: 0,
        commits,
    }))
}

#[must_use]
pub fn live_task(number: u64) -> core::Record {
    core::Record::Tasks(tasks::Stored::Live(Box::new(tasks::TaskRecord {
        settings: tasks::SettingsRevision { task: 1, run: None },
        added: Box::new([]),
        hold_bound_since: None,
        kept_account: None,
        label: 0,
        flags: jig_domain_tasks::Flags::PLAIN,
        last_activity: skein_lib::Wall::EPOCH,
        ended_below: tasks::Endings { done: 0, failed: 0, cancelled: 0 },
        question_lines: Box::new([]),
        hold: 0,
        proposal_rows: Box::new([]),
        instructions: None,
        llm: None,
        refused: Box::new([]),
        grace: None,
        revisions: tasks::Revisions { hold: 0, spec: 1, person: 0 },
        created_at: Wall::EPOCH,
        changed: Wall::EPOCH,
        phase_since: Wall::EPOCH,
        run_since: Some(Wall::EPOCH),
        retry_notice: None,
        ended_at: None,
        revision: 1,
        milestone: 1,
        activity: 0,
        due_reason: tasks::Why::Made,
        narrowing: false,
        result_position: 0,
        last_reply: 0,
        replies: Box::new([]),
        escalation: tasks::Escalation::Unheld { revision: 0 },
        proposal: None,
        result_proposal: false,
        number,
        project: 1,
        requester: tasks::Party::Person(1),
        run_spent: 0,
        run_reserved: 0,
        root: number,
        depth: 0,
        executor: tasks::Executor::Agent { charter: 1 },
        taken_by: None,
        person_answer: None,
        person_wait: None,
        recurring: None,
        tracked: None,
        spec: tasks::Spec {
            words: Box::from(b"scripted task".as_slice()),
            parameters: Box::new([]),
            inputs: Box::new([]),
        },
        contract: tasks::Contract::Report { words: 32 },
        authority: tasks::Authority {
            tools: tasks::Tools(0),
            // This store world has no configured models.
            models: tasks::Models::NONE,
            grants: Box::new([]),
            delegation: tasks::Delegation { kinds: Box::new([]), tasks: 2, depth: 2 },
            budget: tasks::Budget { spend: 1, deadline: None },
            notes: tasks::Scopes(0),
            note_resources: Box::new([]),
        },
        numbers: tasks::Numbers { budget: 1, spent: 0, spent_below: 0, reserved: 0 },
        funder: tasks::Funder::Period { project: 1, period: 1 },
        dependencies: Box::new([]),
        waiting_on: Box::new([]),
        holdings: Box::new([]),
        holds_taken: false,
        hold_wait_since: None,
        delegates: Box::new([]),
        result_followups: Box::new([]),
        references: Box::new([]),
        questions: Box::new([]),
        subscriptions: Box::new([]),
        wake: tasks::WakePolicy::DEFAULT,
        turn: 0,
        last_message: 0,
        inbox: Box::new([]),
        saved: Box::new([]),
        ever_turned: false,
        made: 1,
        attempt: 1,
        last_answer: None,
        tries: tasks::Tries { transient: 0, permanent: 0, run: 0, agent: 0, lost: 0, invalid: 0 },
        invalid_result: None,
        refusals: 0,
        phase: tasks::Phase::Active(tasks::Active::Running { attempt: 1 }),
    })))
}

#[must_use]
pub fn run_proof(task: u64) -> core::Record {
    core::Record::Core(core::CoreRecord::RunProof(core::RunProof {
        accounts: Box::new([]),
        placement: None,
        account_failure: None,
        account_terminal: false,
        task,
        attempt: 1,
        conversation: 1,
        credentials: Box::new([]),
        window: Some(core::CurrentWindow { attempt: 1, turn: 1, closed: false }),
        crossed: false,
        host: jig_domain_fleet::HostKind::Worker,
        offered: None,
        outstanding: Box::new([]),
        turn: None,
        terminal: None,
    }))
}

#[must_use]
pub fn turn(task: u64, attempt: u64, number: u32, bytes: &[u8]) -> core::TurnRecord {
    core::TurnRecord {
        task,
        attempt,
        turn: number,
        opens_window: number == 1,
        spent: 0,
        read: None,
        at: Wall::EPOCH,
        transcript: Box::from(bytes),
    }
}

#[must_use]
pub fn first() -> Box<[Write]> {
    Box::new([Write::Save(header(1)), Write::Wires, Write::Save(live_task(LIVE)), Write::Save(run_proof(LIVE))])
}

#[must_use]
pub fn turn_commit(number: u64, task: u64, turn_number: u32, bytes: &[u8]) -> Box<[Write]> {
    record_turn_commit(number, turn(task, 1, turn_number, bytes))
}

#[must_use]
pub fn record_turn_commit(number: u64, turn: core::TurnRecord) -> Box<[Write]> {
    let core::Record::Core(core::CoreRecord::RunProof(mut proof)) = run_proof(turn.task) else {
        unreachable!("run fixture")
    };
    proof.attempt = turn.attempt;
    if let Some(window) = &mut proof.window {
        window.attempt = turn.attempt;
        if turn.opens_window {
            window.turn = turn.turn;
        }
    }
    proof.turn = Some(core::TurnProof { turn: turn.turn, cumulative: turn.spent, read: turn.read });
    Box::new([
        Write::Save(header(number)),
        Write::Save(core::Record::Core(core::CoreRecord::RunProof(proof))),
        Write::Save(core::Record::Core(core::CoreRecord::Turn(turn))),
    ])
}

#[must_use]
pub fn plain_commit(number: u64) -> Box<[Write]> {
    Box::new([Write::Save(header(number)), Write::Save(live_task(LIVE + 1))])
}
