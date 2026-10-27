//! One person: two workers, two slots each, four LLM accounts (configuration.md, 6).
//! Byte limits are bytes; durations below state their units. Counts are items.
//! **tasks**: `tasks=8, funders=8, project_tasks=8, project_goals=8, tree_tasks=32, depth=8, delegates=4, references=8, subscriptions=4, batch=2, circle=8, dependencies=8, holdings=4, hold_kinds=8, resource_reports=128, pools=8, hold_segments=4, hold_bytes=1024, hold_waiters=8, hold_wait=Duration::from_secs(120), grace=Duration::from_secs(600), inputs=8, spec_bytes=4096, parameters=4, result_bytes=512, run_inbox=8, inbox_messages=8, inbox_bytes=4096, message_bytes=4096, person_stall=Duration::from_secs(3600), proposal_stall=Duration::from_secs(3600), escalation_stall=Duration::from_secs(3600), saved_resources=4, contract_choices=1, charters=4, charter_documents=2, authority_grants=4, authority_segments=4, authority_bytes=2048, executor_kinds=4, retries=RETRIES, facts=32, room_refusals=8`.
//! **people**: `operations=8, people=4, inbox_entries=8, inbox_tasks=16, inbox_recipients=32, sign_ins=8, projects=3, holdings=8, goals=16, initial_owners=2, requests=32, room_refusals=8, pending=8, waiters=4, identity_bytes=1024, words=2048, amendment_bytes=8192, sign_in_lifetime=Duration::from_secs(86400), request_retention=Duration::from_secs(86400), facts=32`.
//! **fleet**: `needs_bytes=65536, grants=8, workers=2, engine_slots=0, slots=2, workstreams=8, attempts=8, calls=32, call_name_bytes=64, progress_bytes=1024, turns=32, grace=Duration::from_secs(120), facts=32, history=8`.
//! **authority**: `projects=2, roles=4, requirements=4, facts=16, grants=4, executors=4, segments=4, segment_bytes=256, implications=8, batch=2, accounts=4, writes=4`.
//! **brief**: `briefs=4, sections=8, read_bytes=8192, brief_bytes=16384`.
//! **accounts**: `credentials=3, accounts=4, holder_accounts=4, sources=3, sign_ins=2, backoff_base=Duration::from_secs(2), backoff_max=Duration::from_secs(60), retry=Duration::from_secs(30), move_deadline=Duration::from_secs(60), spent_attention=Duration::from_secs(300), facts=32`.
//! **views**: `runs=4, watchers=4, watchers_per_party=4, subjects_per_change=32, text_bytes=65536, facts=32`.
//! **notes**: `scopes=8, entries_per_scope=16, pattern_bytes=256, description_bytes=256, body_bytes=4096, references=8, load_rows=4, lines=4, recalled=4`.
//! **documents**: `segments=4, rows=4, text_bytes=4096, chips=4, lines=4, versions=core::documents::Versions { minimum: 1, maximum: 1 }`.
//! **budgets**: `tail=2048, task=4096, dependencies=2048, attempts=1024, plan=2048, notes=2048`.
//! **core**: `templates=8, fill_mark=268435456, connectors=2, resume_bytes=65536, run_bytes=16384, policy_bytes=16384, escalation_reason_bytes=512, load_slots=4, load_bytes=65536, call_records=32, call_answer_bytes=32768, tasks=TASKS, authority=AUTHORITY, people=PEOPLE, fleet=FLEET, brief=BRIEF, brief_parts=8, brief_core_budgets=BUDGETS, accounts=ACCOUNTS, notes=NOTES, documents=DOCUMENTS, views=VIEWS`.
//! **hub**: `slots=2, accounts=8, charter_bytes=65536, transcript_bytes=262144, delivery_evidence_bytes=4096, turn_bytes=65536, outcome_bytes=4096, detail_bytes=4096, held=8, inbox=8, event_bytes=65536, run_calls=8, facts=64, told=64, turns=8, turn_queue_bytes=524288`.
//! **link**: `chunk=16384, name=32, secret=256, names=8, opening=4, handshake=Duration::from_secs(10), hello=Duration::from_secs(10), ping=Duration::from_secs(5), silence=Duration::from_secs(15), stall=Duration::from_secs(30), backoff=Duration::from_millis(250), ceiling=Duration::from_secs(30), proved=Duration::from_secs(60), busy=Duration::from_secs(2), facts=64`.
//! **driver**: `listeners=2 × ACCOUNTS.sign_ins` (each sign-in and one settling before it), the sign-in deadline `client.sign_in_time=Duration::from_secs(300)`.
//! **clients**: `provider_codes=16, provider_tokens=64`.
use super::{Limits, Link};
use jig_domain_accounts as accounts;
use jig_domain_authority as authority;
use jig_domain_brief as brief;
use jig_domain_core as core;
use jig_domain_fleet as fleet;
use jig_domain_host as host;
use jig_domain_notes as notes;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_domain_views as views;
use skein_lib::Duration;
const RETRY: tasks::Retry = tasks::Retry { retries: 3, base: Duration::from_secs(1), max: Duration::from_secs(30) };
const RETRIES: tasks::Retries =
    tasks::Retries { transient: RETRY, permanent: RETRY, run: RETRY, agent: RETRY, lost: RETRY, invalid: RETRY };
pub(super) const TASKS: tasks::Limits = tasks::Limits {
    tasks: 8,
    funders: 8,
    project_tasks: 8,
    project_goals: 8,
    tree_tasks: 32,
    depth: 8,
    delegates: 4,
    references: 8,
    subscriptions: 4,
    batch: 2,
    circle: 8,
    dependencies: 8,
    holdings: 4,
    hold_kinds: 8,
    resource_reports: 128,
    pools: 8,
    hold_segments: 4,
    hold_bytes: 1_024,
    hold_waiters: 8,
    grace: Duration::from_secs(600),
    hold_wait: Duration::from_secs(120),
    inputs: 8,
    spec_bytes: 4_096,
    parameters: 4,
    result_bytes: 512,
    run_inbox: 8,
    inbox_messages: 8,
    inbox_bytes: 4_096,
    message_bytes: 4_096,
    person_stall: Duration::from_secs(3600),
    proposal_stall: Duration::from_secs(3600),
    escalation_stall: Duration::from_secs(3600),
    saved_resources: 4,
    contract_choices: 1,
    charters: 4,
    charter_documents: 2,
    revisions: 4,
    versions: 8,
    authority_grants: 4,
    authority_segments: 4,
    authority_bytes: 2_048,
    executor_kinds: 4,
    retries: RETRIES,
    facts: 32,
    room_refusals: 8,
};
pub(super) const PEOPLE: people::Limits = people::Limits {
    operations: 8,
    batch: 2,
    people: 4,
    inbox_entries: 8,
    inbox_tasks: 16,
    inbox_recipients: 32,
    sign_ins: 8,
    projects: 3,
    holdings: 8,
    initial_owners: 2,
    requests: 32,
    room_refusals: 8,
    pending: 8,
    waiters: 4,
    program_hosts: 9,
    program_host_name_bytes: 32,
    act_grants: 0,
    identity_bytes: 1_024,
    words: 2_048,
    amendment_bytes: 8_192,
    sign_in_lifetime: Duration::from_secs(86400),
    request_retention: Duration::from_secs(86400),
    facts: 32,
};
pub(super) const FLEET: fleet::Limits = fleet::Limits {
    configured_workers: 8,
    worker_name_bytes: 32,
    workers: 2,
    program_sources: 12,
    source_detail_bytes: 4096,
    needs_bytes: 65_536,
    versions: 8,
    grants: 8,
    engine_slots: 0,
    slots: 2,
    workstreams: 8,
    attempts: 8,
    calls: 32,
    call_name_bytes: 64,
    progress_bytes: 1_024,
    turns: 32,
    grace: Duration::from_secs(120),
    facts: 32,
    history: 8,
};
pub(super) const AUTHORITY: authority::Limits = authority::Limits {
    projects: 2,
    roles: 4,
    requirements: 4,
    facts: 16,
    grants: 4,
    executors: 4,
    segments: 4,
    segment_bytes: 256,
    implications: 8,
    batch: 2,
    accounts: 4,
    writes: 4,
};
pub(super) const BRIEF: brief::Limits =
    brief::Limits { briefs: 4, sections: 8, read_bytes: 8_192, brief_bytes: 16_384 };
pub(super) const ACCOUNTS: accounts::Limits = accounts::Limits {
    accounts: 4,
    holder_accounts: 4,
    program_names: 12,
    program_hosts: 9,
    sources: 3,
    sign_ins: 2,
    credentials: 3,
    backoff_base: Duration::from_secs(2),
    backoff_max: Duration::from_secs(60),
    retry: Duration::from_secs(30),
    move_deadline: Duration::from_secs(60),
    spent_attention: Duration::from_secs(300),
    facts: 32,
};
pub(super) const VIEWS: views::Limits = views::Limits {
    runs: 4,
    watchers: 4,
    watchers_per_party: 4,
    subjects_per_change: 32,
    text_bytes: 65_536,
    facts: 32,
};
pub(super) const NOTES: notes::Limits = notes::Limits {
    scopes: 8,
    entries_per_scope: 16,
    pattern_bytes: 256,
    description_bytes: 256,
    body_bytes: 4_096,
    references: 8,
    load_rows: 4,
    lines: 4,
    recalled: 4,
};
pub(super) const DOCUMENTS: core::documents::DocumentLimits = core::documents::DocumentLimits {
    roles: AUTHORITY.roles,
    grants: super::authority_grants(&TASKS, &AUTHORITY),
    executors: super::authority_executors(&TASKS, &AUTHORITY),
    implications: AUTHORITY.implications,
    segments: super::name_segments(&TASKS, &AUTHORITY),
    rows: 4,
    text_bytes: 4_096,
    chips: 4,
    lines: 4,
    versions: core::documents::Versions { minimum: 1, maximum: 1 },
};
pub(super) const BUDGETS: core::CoreBriefBudgets = core::CoreBriefBudgets {
    tail: 2_048,
    task: 4_096,
    dependencies: 2_048,
    attempts: 1_024,
    plan: 2_048,
    notes: 2_048,
};
pub(super) const CORE: core::Limits = core::Limits {
    settings: core::settings::Limits {
        revisions: 4,
        versions: 8,
        history: 64,
        proposals: 4,
        alternatives: 16,
        permissions: 32,
    },
    templates: 8,
    fill_mark: 268_435_456,
    connectors: 2,
    resume_bytes: 65_536,
    run_bytes: 16_384,
    policy_bytes: 16_384,
    escalation_reason_bytes: 512,
    load_slots: 4,
    load_bytes: 65_536,
    call_records: 32,
    call_answer_bytes: 32_768,
    tasks: TASKS,
    authority: AUTHORITY,
    people: PEOPLE,
    fleet: FLEET,
    brief: BRIEF,
    brief_parts: 8,
    brief_core_budgets: BUDGETS,
    accounts: ACCOUNTS,
    notes: NOTES,
    documents: DOCUMENTS,
    views: VIEWS,
};
pub(super) const HUB: host::Limits = host::Limits {
    slots: 2,
    accounts: 8,
    program_sources: 12,
    source_interval: Duration::from_secs(60),
    source_timeout: Duration::from_secs(10),
    charter_bytes: 65_536,
    transcript_bytes: 262_144,
    delivery_evidence_bytes: 4_096,
    turn_bytes: 65_536,
    outcome_bytes: 4_096,
    detail_bytes: 4_096,
    held: 8,
    inbox: 8,
    event_bytes: 65_536,
    run_calls: 8,
    facts: 64,
    told: 64,
    turns: 8,
    turn_queue_bytes: 524_288,
    piece_bytes: 4096,
};
pub(super) const LINK: Link = Link {
    chunk: 16_384,
    name: 32,
    secret: 256,
    names: 8,
    opening: 4,
    handshake: Duration::from_secs(10),
    hello: Duration::from_secs(10),
    ping: Duration::from_secs(5),
    silence: Duration::from_secs(15),
    stall: Duration::from_secs(30),
    backoff: Duration::from_millis(250),
    ceiling: Duration::from_secs(30),
    proved: Duration::from_secs(60),
    busy: Duration::from_secs(2),
    facts: 4470,
};
pub(super) const IO: skein_io::Limits = skein_io::Limits {
    sockets: 16,
    names: 8,
    refusals: 8,
    intake: 1_048_576,
    receive: 16_384,
    output: 1_048_576,
    sends: 8,
    accepts: 4,
    backlog: 16,
    close_timeout: Duration::from_secs(30),
    retry: Duration::from_millis(250),
};
// The root adds 18 positions to core's 18,572. Joined direct/holder terminals
// raise fleet max_out 21 -> 25 and core room by 8 * 4 = 32 positions.
// Close cancels each of Small's two pending sign-ins (protocol/llm.md, 5.2).
// Account moves add 18 core route positions (accounts::max_out: 58 -> 76).
// Retained-revision release adds 2 * 4 heads + 8 versions + one extent = 17.
// Held room covers three decisions: 3 * 18,625 = 55,875.
pub(super) const JOURNAL: skein_lib::JournalLimits =
    skein_lib::JournalLimits { commits: 3, writes: 18_625, held: 55_875, now: 256, release: 256 };
pub(super) const LIMITS: Limits = Limits {
    core: CORE,
    store: STORE,
    hub: HUB,
    link: LINK,
    // Smith keeps the two-byte id length and up to 256 id bytes in its 8192-byte value.
    token: 7934,
    driver: DRIVER,
    clients: CLIENTS,
    clients_http: jig_protocol_engine_clients_http::Limits {
        calls: 4,
        streams: 2,
        body: CLIENTS.message_bytes,
        read: 16384,
        send: 16384,
        event: 87416,
    },
    io: IO,
    journal: JOURNAL,
    routes: 512,
};

/// Native client HTTP: 4 sockets, 8 exchanges, 5 minute link, 30 second stall.
pub(super) const CLIENTS: jig_protocol_engine_clients::Limits = jig_protocol_engine_clients::Limits {
    streams: 2,
    watches_per_stream: 4,
    queued_bytes: 65536,
    identity_bytes: 1024,
    families: 32,
    providers: 32,
    mints: 4,
    provider_codes: 16,
    provider_tokens: 64,
    exchanges: 8,
    exchanges_per_client: 2,
    // One full moved file and its keyed account-change envelope.
    message_bytes: 131_328,
    wire: jig_client_protocol_wire::v19::CEILINGS,
    link_span: Duration::from_secs(300),
};

/// Accounts use bounded OAuth exchanges and the shared socket limits.
pub(super) const DRIVER: skein_oauth_accounts::Limits = skein_oauth_accounts::Limits {
    accounts: super::driver_places(&CORE.accounts).expect("all sources and pending account sign-ins"),
    file_stall: Duration::from_secs(30),
    lock_wait: Duration::from_secs(1),
    exchanges: 4,
    listeners: CORE.accounts.sign_ins.checked_mul(2).expect("a listener and a settling one per sign-in"),
    server: skein_http::server::Limits { head: 4096, headers: 32, body: 1024, read: 1024, response: 4096, send: 1024 },
    refresh_lead: Duration::from_secs(60),
    client: skein_oauth::ClientLimits {
        document: skein_oauth::Limits {
            document_bytes: 65536,
            string_bytes: 16384,
            token_bytes: 7934,
            client_bytes: 1024,
            detail_bytes: 1024,
            record_bytes: 65536,
            // Account sign-in: up to one KiB of issuer-issued client id (skein oauth.md 5).
            issued_client_bytes: 1024,
            granted_bytes: 4096,
            // Account sign-in: one KiB of OpenID subject (skein oauth.md 5).
            subject_bytes: 1024,
            id_token_bytes: 7934,
            depth: 16,
            tokens: 4096,
        },
        uri_bytes: 4096,
        scope_bytes: 4096,
        state_bytes: 64,
        code_bytes: 4096,
        url_bytes: jig_domain_accounts::ADDRESS,
        request_bytes: 65536,
        // Account sign-in terms (skein oauth.md 5, 6.7): host id 17 + 45,
        // agent_name_hint 15 + up to 256 bytes.
        parameter_bytes: 62 + 15 + 256,
        // One URI's 4096 bytes for the account sign-in resource.
        resource_bytes: 4096,
        // 32 random bytes, above the OAuth client's 16-byte minimum.
        nonce_bytes: 32,
        // Account sign-in discovery and key-set documents: 64 KiB each.
        metadata_bytes: 65_536,
        key_set_bytes: 65_536,
        // Four rotating issuer keys (skein oauth.md 5).
        keys: 4,
        modulus_bits: 2048,
        skew: Duration::ZERO,
        sign_in_time: Duration::from_secs(300),
        request_time: Duration::from_secs(30),
        backoff_base: Duration::from_secs(1),
        backoff_ceiling: Duration::from_secs(30),
        max_attempts: 5,
    },
    http: skein_http::client::Limits { request: 65536, head: 16384, headers: 32, read: 16384, send: 16384 },
    tls: skein_tls::client::Limits { read: 16384, send: 65536, records: 131_072 },
    io: IO,
};

// 19.s3: application key2078/value4304, one orphan erase, 256 MiB map.
// Nine choices plus the kept pin add ten optional u32s (50 bytes) to the largest tasks row.
// The largest tasks row can reference all 64 names; sparse and ALL rows keep only used names.
// Added provenance costs 25 bytes per dependency plus its four-byte list count;
// the optional wait-bound wall adds nine bytes, so the largest row grows by 213 bytes.
// Ending authority adds (batch + 1)^2 = (2 + 1)^2 = 9 bytes: the row,
// its recurring members and its proposed creations with their recurring members.
// Task/run settings revisions add nine bytes: u32 plus an optional u32.
pub(super) const STORE_KV: skein_kv::Limits = skein_kv::Limits {
    key: 2079,
    value: 1_567_838,
    // Two operations per journal write, three creation rows and an orphan erase: 2 * 18,625 + 4.
    ops: 37_254,
    // 256 MiB values + 37,254 operations * (2,079 key + 9 framing) + 26 frame bytes.
    commit: 346_221_834,
    queued: 3,
    // Three queued commits; segment holds two, snapshot follows one.
    queued_bytes: 1_038_665_502,
    budget: 268_435_456,
    segment: 692_443_668,
    snapshot_after: 346_221_834,
    chunk: 1_569_937,
    // Accounting ceiling for applications' derived pages: the shipped largest
    // admitted ordinary turn (1,590,046) plus current TurnRecord and frame (64).
    // No row, chunk, decoder, queue or read-slot capacity changes with it.
    page: skein_kv::Page { rows: 2, bytes: 1_590_110 },
    deadline: Duration::from_secs(30),
};
pub(super) const STORE: jig_protocol_store::Limits = jig_protocol_store::Limits {
    core: CORE,
    store: STORE_KV,
    secrets: STORE_KV,
    journal: JOURNAL,
    joins: 4,
    queue: 3,
    files: 8,
    reads: 12,
    orphan_erases: 1,
    held: CLIENTS.exchanges.checked_mul(2).expect("clients carried capacity"),
};
