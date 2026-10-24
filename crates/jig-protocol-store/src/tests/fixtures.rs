//! Shared keys favor number boundaries and common prefixes (protocol/store.md, 4.1).
use alloc::boxed::Box;
use jig_domain_core as core;
use jig_domain_people as people;

pub const KEY_LIMIT: u32 = 1024;
mod keys;
pub use keys::{key, own, pattern, pattern_key, tasks_key, within};

/// Small notes bounds shared by codec round trips.
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
pub fn note_entry() -> jig_domain_notes::Entry {
    let mut references = skein_lib::List::with_capacity(NOTES_LIMITS.references);
    references.push(21).expect("two references fit");
    references.push(34).expect("two references fit");
    jig_domain_notes::Entry {
        name: 17,
        scope: jig_domain_notes::Scope::Goal { project: 3, goal: 21 },
        description: Box::from(b"an entry".as_slice()),
        body: Box::from(b"its body".as_slice()),
        references,
        author: jig_domain_notes::Author::Task { task: 21, attempt: 2 },
        revision: 4,
        changed: skein_lib::Wall::from_nanos(u64::MAX),
    }
}
pub fn note_line() -> jig_domain_notes::Line {
    use jig_domain_notes::{Last, Pattern, Scope};
    jig_domain_notes::Line {
        scope: Scope::Resources {
            project: 3,
            connector: 2,
            pattern: Pattern {
                segments: Box::new([Box::from(b"a".as_slice()), Box::from(b"\0b".as_slice())]),
                last: Last::Open(Box::from(b"c".as_slice())),
            },
        },
        name: 17,
        description: Box::from(b"a resource".as_slice()),
        revision: 4,
    }
}

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
pub fn task_funding() -> jig_domain_tasks::FundingRecord {
    use jig_domain_tasks::{Funder, FundingRecord, Numbers, PeriodTimes};
    FundingRecord {
        period: Some(PeriodTimes {
            began: skein_lib::Wall::from_nanos(11),
            next: Some(skein_lib::Wall::from_nanos(29)),
        }),
        funder: Funder::Period { project: 3, period: 5 },
        parent: None,
        numbers: Numbers { budget: 100, spent: 7, spent_below: 13, reserved: 20 },
        made: 6,
        closed: false,
    }
}
pub fn task_name() -> jig_domain_tasks::Name {
    jig_domain_tasks::Name { connector: 2, path: Box::new([Box::from(b"a\0".as_slice()), Box::from(b"b".as_slice())]) }
}
pub fn task_writer() -> jig_domain_tasks::WriterSlot {
    jig_domain_tasks::WriterSlot {
        number: 9,
        resource: task_name(),
        writer: jig_domain_tasks::Writer::Run { task: 17, attempt: 3 },
        lost: true,
    }
}
pub fn task_authority() -> jig_domain_tasks::Authority {
    use jig_domain_tasks::{
        Authority, AuthorityExecutor, Budget, Delegation, Grant, Last, Models, Pattern, Scopes, Tools,
    };
    Authority {
        tools: Tools(u64::MAX),
        models: Models::ALL,
        grants: Box::new([
            Grant {
                connector: 2,
                kind: 3,
                pattern: Pattern {
                    segments: Box::new([Box::from(b"a".as_slice())]),
                    last: Last::Exact(Box::from(b"b".as_slice())),
                },
            },
            Grant {
                connector: 4,
                kind: 5,
                pattern: Pattern { segments: Box::new([]), last: Last::Open(Box::from(b"c".as_slice())) },
            },
        ]),
        delegation: Delegation {
            kinds: Box::new([
                AuthorityExecutor::Charter(11),
                AuthorityExecutor::Procedure(13),
                AuthorityExecutor::Role(17),
            ]),
            tasks: 19,
            depth: 23,
        },
        budget: Budget { spend: 29, deadline: Some(skein_lib::Wall::from_nanos(u64::MAX)) },
        notes: Scopes(7),
        note_resources: Box::new([]),
    }
}
pub fn task_spec() -> jig_domain_tasks::Spec {
    use jig_domain_tasks::{Parameter, Spec};
    Spec {
        words: Box::from(b"spec".as_slice()),
        parameters: Box::new([
            Parameter::Number { name: 1, value: u64::MAX },
            Parameter::Bytes { name: 2, value: Box::from(b"\0value".as_slice()) },
            Parameter::Resource { name: 3, connector: u16::MAX, resource: 5 },
        ]),
        inputs: Box::new([0, u64::MAX]),
    }
}
pub fn task_contract() -> jig_domain_tasks::Contract {
    use jig_domain_tasks::{Contract, FieldRule, Verdict};
    Contract::Verdict {
        choices: Box::new([Verdict {
            code: 7,
            words: 16,
            followups: 3,
            fields: Box::new([FieldRule { name: Box::from(b"field".as_slice()), max: 11 }]),
        }]),
    }
}

pub fn creation_spec() -> jig_domain_tasks::Spec {
    let mut spec = task_spec();
    spec.inputs = Box::new([]);
    spec
}
/// A full LLM choice: a main model with an effort, and sub-agents with and without one.
pub fn task_llm() -> jig_domain_tasks::LlmChoice {
    use jig_domain_tasks::ModelEffort;
    jig_domain_tasks::LlmChoice {
        account: Some(u32::MAX),
        main: ModelEffort { model: 63, effort: Some(u8::MAX) },
        subagents: Box::new([ModelEffort { model: 0, effort: None }, ModelEffort { model: 7, effort: Some(1) }]),
    }
}
pub fn task_creation(number: u64) -> jig_domain_tasks::New {
    use jig_domain_tasks::{Executor, Funder, Holding, New, Numbers};
    New {
        deadline: jig_domain_tasks::Deadline::Named,
        label: 7,
        flags: jig_domain_tasks::Flags::PLAIN,
        number,
        project: 3,
        executor: Executor::Agent { charter: 11 },
        llm: Some(task_llm()),
        spec: creation_spec(),
        contract: jig_domain_tasks::Contract::Report { words: TASKS_LIMITS.result_bytes },
        authority: task_authority(),
        numbers: Numbers { budget: 29, spent: 0, spent_below: 0, reserved: 0 },
        funder: Funder::Period { project: 3, period: 5 },
        dependencies: Box::new([17]),
        holdings: Box::new([
            Holding::Write { resource: task_name(), kind: 2 },
            Holding::Slot { pool: task_name(), kind: 3 },
        ]),
        wake: task_wake(),
        recurring: None,
        tracked: Some(1),
    }
}
pub fn task_template() -> jig_domain_tasks::RecurringTemplate {
    let mut first = task_creation(1);
    first.dependencies = Box::new([]);
    jig_domain_tasks::RecurringTemplate {
        key: 41,
        batch: Box::new([first]),
        overlap: jig_domain_tasks::RecurringOverlap::Wait,
    }
}
pub fn task_phase() -> jig_domain_tasks::Phase {
    use jig_domain_tasks::{Active, Hold, HoldSource, Phase, Was};
    Phase::Held {
        was: Was::Active(Active::Running { attempt: 3 }),
        why: Hold::Uncertain { entry: 11 },
        source: Some(Box::new(HoldSource::Resource(task_name()))),
    }
}
pub fn task_settings() -> jig_domain_tasks::Settings {
    jig_domain_tasks::Settings {
        tracked: Some(7),
        addressed: Some(jig_domain_tasks::PersonAddress::Role(19)),
        taken_by: Some(31),
        instructions: Some(Box::from(b"override".as_slice())),
        spec: creation_spec(),
        wake: task_wake(),
        dependencies: Box::new([17]),
        authority: task_authority(),
        llm: Some(task_llm()),
    }
}
pub fn task_amendment() -> jig_domain_tasks::Amendment {
    jig_domain_tasks::Amendment {
        tracked: Some(jig_domain_tasks::Tracking::Tracked { priority: 7 }),
        addressed: Some(jig_domain_tasks::AddressChange {
            revision: 7,
            address: jig_domain_tasks::PersonAddress::Person(31),
        }),
        instructions: Some(Box::from(b"new override".as_slice())),
        release: Some(11),
        spec: Some(creation_spec()),
        wake: Some(task_wake()),
        dependencies: Some(Box::new([17])),
        authority: Some(task_authority()),
        llm: Some(task_llm()),
        reason: Box::from(b"amend".as_slice()),
    }
}
pub fn task_proposal() -> jig_domain_tasks::Proposal {
    use jig_domain_tasks::{Proposal, ProposalAction, ProposalHolder, ProposalKind, ProposalState};
    Proposal {
        number: 43,
        proposer: 17,
        project: 3,
        action: ProposalAction::Release { task: 17 },
        reason: Box::from(b"proposal".as_slice()),
        as_holder: true,
        state: ProposalState::Pending {
            holder: ProposalHolder::Policy { project: 3, kind: ProposalKind::Batch },
            since: skein_lib::Wall::from_nanos(47),
        },
    }
}
pub fn task_history() -> jig_domain_tasks::History {
    use jig_domain_tasks::{AmendmentHistory, Change, History, Party};
    let before = task_settings();
    let mut after = task_settings();
    after.instructions = Some(Box::from(b"new override".as_slice()));
    after.taken_by = None;
    after.addressed = Some(jig_domain_tasks::PersonAddress::Person(31));
    History {
        task: 17,
        revision: 3,
        by: Party::Deployment { project: 3 },
        change: Change::Amended,
        reason: Box::from(b"amendment".as_slice()),
        proposal: Some(Box::new(task_proposal())),
        at: skein_lib::Wall::from_nanos(53),
        amendment: Some(Box::new(AmendmentHistory { before, after })),
    }
}
pub fn task_person_proposal() -> jig_domain_tasks::PersonProposal {
    let mut goal = task_creation(23);
    goal.funder = jig_domain_tasks::Funder::Pool { project: 3, person: 31, period: 5 };
    goal.dependencies = Box::new([]);
    jig_domain_tasks::PersonProposal {
        number: 43,
        proposer: 31,
        project: 3,
        action: jig_domain_tasks::PersonAction::Batch(Box::new([goal])),
        reason: Box::from(b"proposed batch".as_slice()),
        state: jig_domain_tasks::PersonProposalState::Pending { since: skein_lib::Wall::from_nanos(47) },
    }
}

pub fn task_wake() -> jig_domain_tasks::WakePolicy {
    use jig_domain_tasks::{ResultsWake, WakePolicy, WakeRule};
    WakePolicy {
        words: WakeRule::Batch { count: 3, age: skein_lib::Duration::from_nanos(7) },
        notices: WakeRule::Immediate,
        news: WakeRule::Never,
        results: ResultsWake::Each,
        questions: true,
        answers: false,
        timers: true,
    }
}

pub const PEOPLE_LIMITS: people::Limits = people::Limits {
    operations: 2,
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
    amendment_bytes: 512,
    sign_in_lifetime: skein_lib::Duration::from_nanos(1_000),
    request_retention: skein_lib::Duration::from_nanos(1_000),
    facts: 2,
};

pub fn identity() -> people::Identity {
    people::Identity {
        key: people::IdentityKey { provider: 3, subject: Box::from(b"subject".as_slice()) },
        login: Box::from(b"login".as_slice()),
        name: Box::from(b"name".as_slice()),
    }
}

pub fn people_pattern() -> people::Pattern {
    people::Pattern { segments: Box::new([Box::from(b"a".as_slice())]), last: people::Last::Open(Box::new([0, 255])) }
}

pub fn people_authority() -> people::Authority {
    people::Authority {
        tools: 9,
        models: None,
        grants: Box::new([people::Grant { connector: 1, kind: 2, pattern: people_pattern() }]),
        delegation: people::Delegation {
            kinds: Box::new([people::Executor::Charter(3), people::Executor::Procedure(4), people::Executor::Role(5)]),
            tasks: 2,
            depth: 3,
        },
        spend: 17,
        deadline: Some(skein_lib::Wall::from_nanos(u64::MAX)),
        notes: 3,
        note_resources: Box::new([people::ResourceScope { connector: 1, pattern: people_pattern() }]),
    }
}

/// Configured models' names, in the configuration's order.
pub fn model_names(names: &[&[u8]]) -> Box<[Box<[u8]>]> {
    let mut result = skein_lib::List::with_capacity(u64::BITS);
    for name in names {
        result.push(Box::from(*name)).expect("at most one name per model bit");
    }
    result.into_boxed()
}

pub fn people_policy() -> people::PolicyValue {
    people::PolicyValue {
        ceiling: people_authority(),
        escalation_role: Some(0),
        templates: Box::new([
            people::TemplateOverride {
                template: 1,
                executor: Some(people::TemplateExecutor::Charter(1)),
                budget: Some(people::TemplateBudget::Amount(21)),
                deadline: Some(people::TemplateDeadline::None),
                priority: Some(5),
            },
            people::TemplateOverride {
                template: 2,
                executor: Some(people::TemplateExecutor::Chosen),
                budget: Some(people::TemplateBudget::AsAllowed { cap: Some(34) }),
                deadline: Some(people::TemplateDeadline::Span(skein_lib::Duration::from_secs(55))),
                priority: None,
            },
            people::TemplateOverride {
                template: 3,
                executor: None,
                budget: Some(people::TemplateBudget::Given),
                deadline: Some(people::TemplateDeadline::Given),
                priority: None,
            },
        ]),
        period_spend: 99,
        projections: Box::new([]),
        roles: Box::new([people::PolicyRole {
            number: 4,
            authority: people_authority(),
            period_spend: 33,
            requests: 2,
            decides: 3,
        }]),
        requirements: Box::new([people::Requirement {
            connector: 1,
            kind: 2,
            pattern: people_pattern(),
            judge: people::Judge { connector: 2, requirement: 3, parameters: 4 },
            guard: people::Guard::Observed { freshness: skein_lib::Duration::from_nanos(91) },
            must_be_guarded: true,
        }]),
        permissions: Box::new([people::PermissionRole { connector: 1, permission: 2, role: 4 }]),
    }
}

pub fn people_amendment() -> people::Amendment {
    people::Amendment {
        tracked: Some(people::Tracking::Tracked { priority: 7 }),
        addressed: Some(people::AddressChange { revision: 7, address: people::PersonAddress::Role(4) }),
        instructions: Some(Box::new([0, 255])),
        spec: Some(people::Spec {
            words: Box::new([1]),
            parameters: Box::new([
                people::Parameter::Number { name: 3, value: 8 },
                people::Parameter::Bytes { name: 4, value: Box::new([0]) },
                people::Parameter::Resource { name: 5, connector: 1, resource: 6 },
            ]),
            inputs: Box::new([8]),
        }),
        wake: Some(people::WakePolicy {
            words: people::WakeRule::Never,
            notices: people::WakeRule::Immediate,
            news: people::WakeRule::Batch { count: 2, age: skein_lib::Duration::from_nanos(99) },
            results: people::ResultsWake::LastOrFailure,
            questions: true,
            answers: false,
            timers: true,
        }),
        dependencies: Some(Box::new([9])),
        authority: Some(people_authority()),
        ..person_unchanged(&[2])
    }
}

pub fn people_member() -> people::Member {
    people::Member {
        llm: None,
        deadline: jig_domain_people::Deadline::Named,
        executor: people::TaskExecutor::Agent { charter: 3 },
        words: b"a task".as_slice().into(),
        parameters: Box::new([]),
        inputs: Box::new([]),
        budget: people::Budget::Named,
        named: Box::new([]),
        contract: people::Contract::Report { words: 17 },
        authority: people_authority(),
        wake: people::WakePolicy {
            words: people::WakeRule::Never,
            notices: people::WakeRule::Never,
            news: people::WakeRule::Never,
            results: people::ResultsWake::Never,
            questions: false,
            answers: false,
            timers: false,
        },
        dependencies: Box::new([]),
        tracked: Some(3),
        label: 41,
        flags: people::Flags::PLAIN,
    }
}
fn people_propose() -> people::Ask {
    people::Ask::Propose {
        project: 1,
        action: people::Action::Batch(Box::new([people_member()])),
        reason: Box::from(b"please fund it".as_slice()),
    }
}

fn people_amend_proposal() -> people::Ask {
    people::Ask::Propose {
        project: 1,
        action: people::Action::Amend { task: 2, revision: 3, amendment: Box::new(people_amendment()) },
        reason: b"please amend".as_slice().into(),
    }
}

pub fn people_asks() -> Box<[people::Ask]> {
    use people::{Ask, Proposer};
    let [cancel_move, amend_release, message_stop] = people_compounds();
    let mut procedure = people_member();
    procedure.executor = people::TaskExecutor::Procedure { connector: 1, code: 1 };
    procedure.contract = people::Contract::Change { connector: 1, kind: 5, words: 17 };
    procedure.inputs = Box::new([2]);
    procedure.budget = people::Budget::AsAllowed { cap: Some(30) };
    procedure.dependencies = Box::new([people::Dependency::Batch(0), people::Dependency::Existing(2)]);
    procedure.named = procedure.authority.grants.clone();
    let mut person = people_member();
    person.executor = people::TaskExecutor::Person(people::PersonAddress::Role(2));
    Box::new([
        Ask::Create { project: 1, batch: Box::new([people_member(), procedure]) },
        Ask::Create { project: 1, batch: Box::new([person]) },
        cancel_move,
        amend_release,
        message_stop,
        people_propose(),
        people_amend_proposal(),
        Ask::Withdraw { project: 1, proposal: 43 },
        Ask::MarkSeen { seen: people::Seen::Conversation { task: 2, through: 3 } },
        Ask::Application { scope: jig_domain_people::ApplicationScope::Project(1), connector: 2, digest: [3; 32] },
        Ask::EditNote {
            project: 1,
            name: 2,
            scope: Box::new(people::NoteScope::Resources { connector: 1, pattern: people_pattern() }),
            change: Box::new(people::NoteChange::Correct {
                description: Box::new([2]),
                body: Box::new([0, 255]),
                references: Box::new([3]),
                recalled: 4,
            }),
        },
        Ask::MakeService { project: 1, name: Box::new([2]), role: people::Role::Maintainer },
        Ask::AccountSignIn { holder: people::AccountHolder::Own, provider: 2 },
        Ask::AccountSignOut { holder: people::AccountHolder::Deployment, source: 3 },
        Ask::Adopt {
            project: 1,
            adoption: people::Adoption {
                resource: people::ResourceName { connector: 1, path: Box::new([Box::new([2])]) },
                role: people::ResourceRole::Owned,
                options: Box::new([Box::new([3])]),
            },
        },
        Ask::Stop { project: 1, task: 2 },
        Ask::Cancel { project: 1, task: 2, reason: Box::new([3]), grace: Some(skein_lib::Duration::from_secs(5)) },
        Ask::Release { project: 1, task: 2, revision: 3, again: Some(true) },
        Ask::TakePerson { project: 1, task: 2 },
        Ask::HandBackPerson { project: 1, task: 2 },
        Ask::AnswerPerson {
            project: 1,
            task: 2,
            revision: 3,
            result: people::PersonResult::Approved { state: Box::new([2]), words: Some(Box::new([3])) },
        },
        Ask::Move { project: 1, task: 2, reason: Box::new([3]) },
        Ask::DecideProposal {
            project: 1,
            proposer: Proposer::Party(2),
            proposal: 3,
            decision: people::ProposalDecision::Reject { reason: Box::new([4]) },
            running: None,
        },
        Ask::Say { project: 1, task: 2, words: Box::new([3]) },
        Ask::AnswerQuestion { project: 1, task: 2, question: 3, words: Box::new([4]) },
        Ask::Amend { project: 1, task: 2, revision: 3, amendment: people_amendment(), release: Some(4) },
        Ask::SetRoles {
            project: 1,
            holdings: Box::new([people::Holding { person: 2, role: people::Role::Policy { role: 4 } }]),
        },
        Ask::ChangePolicy {
            project: 1,
            change: people::PolicyChange::Role(people::PolicyRole {
                number: 4,
                authority: people_authority(),
                period_spend: 5,
                requests: 2,
                decides: 3,
            }),
        },
        Ask::SetPool { project: 1, person: 2, budget: 3 },
        Ask::DecideEscalation {
            project: 1,
            task: 2,
            revision: 3,
            decision: people::EscalationDecision::Reject { reason: Box::new([4]) },
        },
    ])
}

pub const CORE_LIMITS: core::Limits = core::Limits {
    settings: core::settings::Limits { history: 64, proposals: 4, alternatives: 16, permissions: 32 },
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
    policy_bytes: 8192,
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

mod record;
pub use record::{task_record, task_word};
pub mod calls;

pub mod core_rows;

pub mod people_outcomes;

pub const APPLICATION: crate::sizes::Application = crate::sizes::Application { version: 1, key: 64, value: 128 };
pub const DEPLOYMENT: [u8; 16] = [7; 16];
/// The installation's identifier the fixtures' opens draw.
pub const INSTALLATION: [u8; 16] = [5; 16];

pub mod disk;
mod reads;
mod store;
pub use reads::{
    CHILD, GOAL, GRANDCHILD, activity_line, answer_line, delegate, ended_delegate, ended_of, escalation_decision,
    history_with_proposal, hold_milestone, ledger_period, notes_entry_in, notified_line, person_proposal,
    proposal_decision, question_line, read, terminal, update, with_hold, with_links, with_proposal_rows,
    with_question_lines,
};
pub use store::{
    ALICE, ENDED, LIVE, OTHER_DEPLOYMENT, PROJECT, ROOT, answer, below, call, codec_limits, commit, committed,
    directory, ended_task, env, format_put, get_kv, header, header_put, header_record, history, installation_put,
    layouts_put, live_task, notes_entry, notes_line, opened, opened_with_budget, over, person, plant, result_index,
    run, run_proof, save, sign_in_secret, stores, turn, window_turn,
};

pub use store::limits as store_limits;
pub use store::{Answer, events, opened_with_transcripts, script, written};

/// An amendment retaining every field, with the supplied explanation.
#[must_use]
pub fn person_unchanged(reason: &[u8]) -> jig_domain_people::Amendment {
    jig_domain_people::Amendment {
        llm: None,
        tracked: None,
        addressed: None,
        instructions: None,
        spec: None,
        wake: None,
        dependencies: None,
        authority: None,
        reason: reason.into(),
    }
}

fn people_compound(values: [people::Operation; 2]) -> people::Ask {
    let mut operations = skein_lib::List::with_capacity(2);
    for operation in values {
        operations.push(operation).expect("two operation fixtures");
    }
    people::Ask::Compound { project: 1, operations }
}

fn people_compounds() -> [people::Ask; 3] {
    [
        people_compound([
            people::Operation::Cancel {
                task: 2,
                reason: Box::new([3]),
                grace: Some(skein_lib::Duration::from_secs(5)),
            },
            people::Operation::Move { task: 2, reason: Box::new([3]) },
        ]),
        people_compound([
            people::Operation::Amend {
                task: 2,
                revision: 3,
                amendment: Box::new(people_amendment()),
                release: Some(4),
            },
            people::Operation::Release { task: 2, revision: 3, again: Some(false) },
        ]),
        people_compound([
            people::Operation::Message { task: 2, words: Box::new([3]) },
            people::Operation::Stop { task: 2 },
        ]),
    ]
}

// A finite wall distinct from the row timestamps.
pub const DEADLINE: skein_lib::Wall = skein_lib::Wall::from_nanos(101);

/// The fixture sets' families, in their supplied order, on a first start.
pub const FIRST_MANIFEST_COUNTS: &[(&str, u64, u64)] =
    &[("fixture-label", 0, 0), ("fixture-shelf", 0, 0), ("fixture-token", 0, 0), ("fixture-note", 0, 0)];

/// Manifest count rows through the codec's checked constructor.
pub fn manifest_counts(rows: &[(&str, u64, u64)]) -> skein_lib::List<crate::codec::component::Count> {
    let mut counts = skein_lib::List::with_capacity(u32::try_from(rows.len()).expect("fixture count"));
    for (family, before, after) in rows {
        let count = crate::codec::component::Count::new(
            &crate::component_v3::CEILINGS,
            crate::codec::component::CountParts {
                family: Box::from(family.as_bytes()),
                before: *before,
                after: *after,
            },
        )
        .expect("fixture family");
        counts.push(count).expect("fixture count");
    }
    counts
}
