//! A team: eight workers, four slots each, sixteen LLM accounts (configuration.md, 6).
//! Every field omitted below has the small profile's documented value. Token: 16384 bytes.
//! Byte limits are bytes; durations below state their units. Counts are items.
//! **tasks**: `placed_holds=16, project_placed_holds=8, party_placed_holds=4, exemptions=32, tasks=32, funders=32, project_tasks=32, project_goals=16, tree_tasks=128, delegates=8, resource_reports=256, pools=32, hold_waiters=32, charters=8, facts=128, room_refusals=32, batch=4, circle=32, dependencies=16, inputs=16, references=16, holdings=8, parameters=8, saved_resources=8, authority_grants=8, authority_bytes=4096, executor_kinds=8`.
//! **people**: `operations=16, people=16, inbox_tasks=64, sign_ins=32, projects=9, holdings=32, goals=64, initial_owners=4, requests=128, room_refusals=32, pending=32, facts=128`.
//! **fleet**: `needs_bytes=262144, grants=32, workers=8, slots=4, workstreams=64, attempts=64, calls=256, turns=128, facts=256, history=64`.
//! **authority**: `projects=8, accounts=16, roles=8, requirements=8, grants=8, executors=8, batch=4, writes=8`.
//! **brief**: `briefs=32`.
//! **accounts**: `credentials=15, accounts=16, sign_ins=4, facts=128`.
//! **views**: `runs=32, watchers=16, facts=256`.
//! **notes**: `scopes=16, entries_per_scope=64, body_bytes=8192, lines=8, recalled=8`.
//! **documents**: `rows=8, text_bytes=8192, chips=8, lines=8`.
//! **core**: `templates=16, fill_mark=1073741824, connectors=4, load_slots=16, call_records=128, tasks=TASKS, authority=AUTHORITY, people=PEOPLE, fleet=FLEET, brief=BRIEF, accounts=ACCOUNTS, notes=NOTES, views=VIEWS, resume_bytes=262144, run_bytes=65536, documents=DOCUMENTS`.
//! **hub**: `slots=4, accounts=32, facts=256, told=256`.
//! **link**: `opening=16, facts=256`.
//! **driver**: `exchanges=8, listeners=2 × ACCOUNTS.sign_ins`.
//! **clients**: `provider_codes=256, provider_tokens=1024`.
use super::small;
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
pub(super) const TASKS: tasks::Limits = tasks::Limits {
    placed_holds: 16,
    project_placed_holds: 8,
    party_placed_holds: 4,
    exemptions: 32,

    tasks: 32,
    funders: 32,
    project_tasks: 32,
    project_goals: 16,
    tree_tasks: 128,
    delegates: 8,
    resource_reports: 256,
    pools: 32,
    hold_waiters: 32,
    charters: 8,
    revisions: 8,
    versions: 16,
    facts: 128,
    room_refusals: 32,
    batch: 4,
    circle: 32,
    dependencies: 16,
    inputs: 16,
    references: 16,
    holdings: 8,
    parameters: 8,
    saved_resources: 8,
    authority_grants: 8,
    authority_bytes: 4_096,
    executor_kinds: 8,
    ..small::TASKS
};
pub(super) const PEOPLE: people::Limits = people::Limits {
    program_hosts: 65,
    operations: 16,
    batch: 4,
    people: 16,
    inbox_tasks: 64,
    sign_ins: 32,
    projects: 9,
    holdings: 32,
    initial_owners: 4,
    requests: 128,
    room_refusals: 32,
    pending: 32,
    facts: 128,
    ..small::PEOPLE
};
pub(super) const FLEET: fleet::Limits = fleet::Limits {
    configured_workers: 64,
    workers: 8,
    program_sources: 12,
    source_detail_bytes: 4096,
    needs_bytes: 262_144,
    versions: 16,
    grants: 32,
    slots: 4,
    workstreams: 64,
    attempts: 64,
    calls: 256,
    turns: 128,
    facts: 256,
    history: 64,
    ..small::FLEET
};
pub(super) const AUTHORITY: authority::Limits = authority::Limits {
    projects: 8,
    accounts: 16,
    roles: 8,
    requirements: 8,
    grants: 8,
    executors: 8,
    batch: 4,
    writes: 8,
    ..small::AUTHORITY
};
pub(super) const BRIEF: brief::Limits = brief::Limits { briefs: 32, ..small::BRIEF };
pub(super) const ACCOUNTS: accounts::Limits =
    accounts::Limits { program_hosts: 65, accounts: 16, sign_ins: 4, credentials: 15, facts: 128, ..small::ACCOUNTS };
pub(super) const VIEWS: views::Limits = views::Limits { runs: 32, watchers: 16, facts: 256, ..small::VIEWS };
pub(super) const NOTES: notes::Limits =
    notes::Limits { scopes: 16, entries_per_scope: 64, body_bytes: 8_192, lines: 8, recalled: 8, ..small::NOTES };
pub(super) const DOCUMENTS: core::documents::DocumentLimits = core::documents::DocumentLimits {
    roles: AUTHORITY.roles,
    grants: super::authority_grants(&TASKS, &AUTHORITY),
    executors: super::authority_executors(&TASKS, &AUTHORITY),
    implications: AUTHORITY.implications,
    rows: 8,
    text_bytes: 8_192,
    chips: 8,
    lines: 8,
    ..small::DOCUMENTS
};
pub(super) const CORE: core::Limits = core::Limits {
    settings: core::settings::Limits {
        revisions: 8,
        versions: 16,
        history: 64,
        proposals: 8,
        alternatives: 16,
        permissions: 32,
    },
    templates: 16,
    fill_mark: 1_073_741_824,
    connectors: 4,
    load_slots: 16,
    call_records: 128,
    tasks: TASKS,
    authority: AUTHORITY,
    people: PEOPLE,
    fleet: FLEET,
    brief: BRIEF,
    accounts: ACCOUNTS,
    notes: NOTES,
    views: VIEWS,
    resume_bytes: 262_144,
    run_bytes: 65_536,
    documents: DOCUMENTS,
    ..small::CORE
};
pub(super) const HUB: host::Limits = host::Limits { slots: 4, accounts: 32, facts: 256, told: 256, ..small::HUB };
pub(super) const LINK: Link = Link { names: 64, opening: 16, facts: 4470, ..small::LINK };
pub(super) const LIMITS: Limits = Limits {
    core: CORE,
    store: STORE,
    hub: HUB,
    link: LINK,
    // Smith keeps the two-byte id length and up to 256 id bytes in its 8192-byte value.
    token: 7934,
    driver: skein_oauth_accounts::Limits {
        accounts: super::driver_places(&CORE.accounts).expect("all sources and pending account sign-ins"),
        exchanges: 8,
        listeners: CORE.accounts.sign_ins.checked_mul(2).expect("a listener and a settling one per sign-in"),
        io: skein_io::Limits { sockets: 64, names: 64, ..small::IO },
        ..small::DRIVER
    },
    clients: CLIENTS,
    clients_http: jig_protocol_engine_clients_http::Limits {
        calls: 16,
        streams: 8,
        body: CLIENTS.message_bytes,
        read: 16384,
        send: 16384,
        event: 87416,
    },
    io: skein_io::Limits { sockets: 64, names: 64, ..small::IO },
    journal: JOURNAL,
    routes: 2048,
};

pub(super) const CLIENTS: jig_protocol_engine_clients::Limits = jig_protocol_engine_clients::Limits {
    streams: 8,
    exchanges: 32,
    mints: 16,
    provider_codes: 256,
    provider_tokens: 1024,
    ..small::CLIENTS
};

// One extra Sources output at hello adds eight core route positions.
// Close cancellation adds one output per pending sign-in: four in Standard (protocol/llm.md, 5.2).
// Account moves add 56 core route positions (accounts::max_out: 206 -> 262).
// Retained-revision release adds 2 * 8 heads + 16 versions + one extent = 33.
// Placed holds: tightened core room plus 18 connector slots needs 151,673 writes.
const JOURNAL: skein_lib::JournalLimits =
    skein_lib::JournalLimits { commits: 3, writes: 151_673, held: 455_019, now: 1024, release: 1024 };
// Same 19.s3 composition as small; the standard domain supplies the larger value and operation bounds.
// Its 25 choices plus the kept pin add 26 optional u32s (130 bytes) to the largest tasks row.
// The largest tasks row can reference all 64 names; sparse and ALL rows keep only used names.
// Added provenance costs 25 bytes per dependency plus its four-byte list count;
// the optional wait-bound wall adds nine bytes, so the largest row grows by 413 bytes.
// Ending authority adds (batch + 1)^2 = (4 + 1)^2 = 25 bytes: the row,
// its recurring members and its proposed creations with their recurring members.
// Task/run settings revisions add nine bytes: u32 plus an optional u32.
// A refused procedure room wait adds at most fourteen bytes: option, count scope, project and order.
const STORE_KV: skein_kv::Limits = skein_kv::Limits {
    value: 10_664_789,
    ops: 303_350,
    commit: 901_830_282,
    queued_bytes: 2_705_490_846,
    segment: 1_803_660_564,
    snapshot_after: 901_830_282,
    chunk: 10_666_888,
    page: skein_kv::Page { rows: 2, bytes: 10_666_868 },
    ..small::STORE_KV
};
const STORE: jig_protocol_store::Limits = jig_protocol_store::Limits {
    core: CORE,
    store: STORE_KV,
    secrets: STORE_KV,
    journal: JOURNAL,
    held: CLIENTS.exchanges.checked_mul(2).expect("clients carried capacity"),
    joins: 16,
    reads: 48,
    ..small::STORE
};
