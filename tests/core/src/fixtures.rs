//! Shared host bounds for the domain world.
use jig_domain_authority as authority;
use jig_domain_core as core;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_test_domain as root;
use skein_lib::{ReplyTo, Token};
pub const WORKSPACE_CREDENTIAL: u32 = 7;
pub const CREDENTIAL_RESOURCE: u64 = 1;
#[must_use]
pub fn bounds() -> jig_domain_fleet::Bounds {
    use skein_lib::Duration;
    jig_domain_fleet::Bounds {
        program_endpoints: Box::new([]),
        models: Box::new([
            jig_domain_fleet::ModelBound { name: b"scripted".as_slice().into(), window: 8192, output: 1024 },
            jig_domain_fleet::ModelBound { name: b"research".as_slice().into(), window: 8192, output: 1024 },
        ]),
        turns: u32::MAX,
        spend: u64::MAX,
        time: Duration::from_secs(3600),
        waiting: Duration::from_secs(3600),
        checks: Duration::from_nanos(1),
        message_bytes: u32::MAX,
        inbox: u32::MAX,
        unacknowledged: u32::MAX,
    }
}

#[must_use]
pub fn live_row(world: &crate::effects::World, task: u64) -> &jig_domain_tasks::TaskRecord {
    match world
        .store
        .rows
        .get(&jig_test_domain::Key::Core(jig_domain_core::Key::Tasks(jig_domain_tasks::Key::Live(task))))
    {
        Some(jig_test_domain::Record::Core(jig_domain_core::Record::Tasks(jig_domain_tasks::Stored::Live(row)))) => row,
        row => panic!("live task row missing: {row:?}"),
    }
}

pub const QUERY_REQUESTER: u64 = 1;

/// A requesting party's task query, shared by the independent watch stories.
pub const OWN_TASKS: jig_domain_core::documents::TaskQuery = jig_domain_core::documents::TaskQuery {
    project: None,
    requester: Some(jig_domain_tasks::Party::Person(QUERY_REQUESTER)),
    root: None,
    parent: None,
    label: None,
    phase: None,
    executor: None,
    tracked: None,
    order: jig_domain_core::documents::QueryOrder::Number,
};

/// A delegate result and one standing state subscription each reserve a result payload.
#[must_use]
pub fn notices_world(seed: u64) -> crate::effects::World {
    let (config, mut limits) = crate::effects::fixture(seed, false);
    limits.core.tasks.inbox_bytes = 256;
    crate::effects::World::configured(seed, false, config, limits)
}

/// Task numbers in a live query page, retaining its requested order.
#[must_use]
pub fn query_numbers(world: &crate::effects::World) -> Vec<u64> {
    let jig_domain_core::reads::Answer::Tasks(page) = crate::reads::latest(world) else { panic!("query page") };
    page.live.iter().map(|line| line.chip.number).collect()
}

#[must_use]
pub fn templates() -> Box<[jig_domain_core::Template]> {
    use jig_domain_people::{TemplateBudget, TemplateDeadline, TemplateExecutor};
    use jig_domain_tasks::{Contract, Flags, Retention};
    Box::new([
        jig_domain_core::Template {
            number: 1,
            name: Box::from(&b"chat"[..]),
            executor: TemplateExecutor::Charter(1),
            contract: Contract::Report { words: 32 },
            shallower: 1,
            budget: TemplateBudget::AsAllowed { cap: None },
            deadline: TemplateDeadline::None,
            tracked: None,
            flags: Flags {
                ended_by: jig_domain_tasks::EndedBy::Result,
                conversation: true,
                listed: false,
                retention: Retention::Kept,
            },
            label: jig_client_test_records::CHAT,
            grace: Some(skein_lib::Duration::from_millis(5000)),
            llm: None,
        },
        jig_domain_core::Template {
            number: 2,
            name: Box::from(&b"goal"[..]),
            executor: TemplateExecutor::Chosen,
            contract: Contract::Report { words: 32 },
            shallower: 1,
            budget: TemplateBudget::Given,
            deadline: TemplateDeadline::Given,
            tracked: Some(1),
            flags: Flags {
                ended_by: jig_domain_tasks::EndedBy::Result,
                conversation: false,
                listed: true,
                retention: Retention::Kept,
            },
            label: jig_client_test_records::GOAL,
            grace: None,
            llm: None,
        },
    ])
}

const HOLDER_REQUEST_BASE: u64 = 8200;
/// Submit an owner or member request with the fixture's request-token range.
pub fn ask_as(world: &mut crate::effects::World, sign_in: u64, key: u8, ask: jig_domain_people::Ask) {
    world.send(jig_test_domain::Event::Core(jig_domain_core::Event::People(jig_domain_people::Event::Ask {
        models: Box::new([]),
        reply_to: skein_lib::ReplyTo::new(skein_lib::Token::new(HOLDER_REQUEST_BASE + u64::from(key))),
        sign_in,
        key: [key; 16],
        ask,
    })));
}

#[must_use]
pub fn member(world: &crate::effects::World, executor: tasks::Executor, words: &[u8]) -> core::Delegate {
    let mut authority = world.assigned.as_ref().expect("task assigned").run.authority.clone();
    authority.budget.spend = 20;
    authority.delegation =
        tasks::Delegation { kinds: Box::new([tasks::AuthorityExecutor::Charter(1)]), tasks: 1, depth: 1 };
    core::Delegate {
        deadline: jig_domain_tasks::Deadline::Named,
        label: 0,
        flags: jig_domain_tasks::Flags::PLAIN,
        tracked: None,
        budget: jig_domain_people::Budget::Named,
        executor,
        spec: tasks::Spec { asks: false, words: words.into(), parameters: Box::new([]), inputs: Box::new([]) },
        contract: tasks::Contract::Report { words: 128 },
        authority,
        llm: None,
        symbolic_grants: Box::new([]),
        dependencies: Box::new([]),
        wake: tasks::WakePolicy::DEFAULT,
    }
}

/// A running caller with the supplied authority wall, for delegation deadline stories.
#[must_use]
pub fn run_with_deadline(seed: u64, deadline: Option<skein_lib::Wall>) -> crate::effects::World {
    let (config, limits) = crate::faults::plan_fixture(seed);
    let people::Ask::Create { project, mut batch } =
        create_chat(&Creating::new(&config.core, limits.core.tasks.tree_tasks), 1, 0, b"caller".as_slice().into())
    else {
        panic!("caller creation")
    };
    batch[0].authority.deadline = deadline;
    crate::effects::World::configured_with(seed, false, config, limits, people::Ask::Create { project, batch })
}

/// Assert the last party answer carries this refusal.
pub fn refusal(world: &crate::effects::World, why: jig_domain_people::Refusal) {
    use jig_domain_people as people;
    assert_eq!(world.people_answers.last(), Some(&people::Reply::Outcome(people::Outcome::Refused(why))));
}

/// A person's amendment retaining each field.
#[must_use]
pub fn person_unchanged(reason: &[u8]) -> jig_domain_people::Amendment {
    jig_domain_people::Amendment {
        tracked: None,
        addressed: None,
        instructions: None,
        spec: None,
        wake: None,
        dependencies: None,
        authority: None,
        llm: None,
        reason: reason.into(),
    }
}

/// Finish the assigned chat with a report.
pub fn finish_chat(world: &mut crate::effects::World, cumulative: u64, words: &[u8]) {
    world.end(
        tasks::End::Finished { result: tasks::TaskResult::Report { words: words.into() }, cancel_delegates: false },
        cumulative,
    );
}

/// A task amendment retaining every setting.
#[must_use]
pub fn unchanged(reason: &[u8]) -> tasks::Amendment {
    tasks::Amendment {
        tracked: None,
        addressed: None,
        instructions: None,
        release: None,
        spec: None,
        wake: None,
        dependencies: None,
        authority: None,
        llm: None,
        reason: reason.into(),
    }
}

pub fn delegate(world: &mut crate::effects::World, run: (u64, u64), completion: u32, member: core::Delegate) -> u64 {
    world.send(root::Event::Core(core::Event::DelegateValidated {
        to: ReplyTo::new(Token::new(9000 + u64::from(completion))),
        key: core::CallKey { task: run.0, attempt: run.1, completion, position: 0 },
        batch: Box::new([member]),
        stubs: Box::new([]),
    }));
    let core::CallPart::Delegated(numbers) = &world.answers.last().expect("delegate answer").2 else {
        panic!("delegation was not admitted: {:?}", world.answers.last());
    };
    numbers[0].task
}

#[must_use]
pub fn two_projects(seed: u64) -> (root::Config, root::Limits) {
    let mut config = crate::world::config(seed);
    let mut limits = crate::world::limits();
    limits.core.authority.projects = 2;
    limits.core.tasks.funders = 8;
    // Both configured projects and the deployment's roles.
    limits.core.people.projects = 3;
    limits.core.people.initial_owners = 2;
    crate::peers::journal(&mut limits);
    let policy = config.core.seed_projects[0].policy.policy.clone();
    config.core.defaults.rules = (authority::Domain::new(config.core.defaults.rules.clone(), limits.core.authority)
        .expect("two projects"))
    .rules()
    .clone();
    config.core.seed_projects = [1, 2]
        .into_iter()
        .map(|number| {
            let mut policy = policy.clone();
            policy.period_spend = if number == 1 { 1000 } else { 700 };
            core::SeedProject {
                number,
                name: format!("Project {number}").into_boxed_str(),
                policy: core::ConfiguredPolicy {
                    policy,
                    permissions: if number == 1 {
                        config.core.seed_projects[0].policy.permissions.clone()
                    } else {
                        Box::new([])
                    },
                    templates: Box::new([]),
                },
            }
        })
        .collect();
    config.core.owners = Box::new([
        config.core.owners[0].clone(),
        people::InitialOwner { project: 2, identity: config.core.owners[0].identity.clone() },
    ]);
    (config, limits)
}

/// Move the former implicit goal-proposal stories onto the explicit batch request.
#[must_use]
pub fn propose_goal(ask: jig_domain_people::Ask) -> jig_domain_people::Ask {
    use jig_domain_people as people;
    let people::Ask::Create { project, batch } = ask else { panic!("creation fixture") };
    people::Ask::Propose {
        project,
        reason: Box::from(b"please fund this goal".as_slice()),
        action: people::Action::Batch(batch),
    }
}

/// One requested root, shared by direct creation and explicit proposal stories.
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

const FIXTURE_INBOX_REQUEST: u64 = 8301;
const FIXTURE_SIGN_IN_REQUEST: u64 = 8300;
const FIXTURE_PROJECT: u32 = 1;

/// Read every page of one party inbox group.
pub fn inbox_objects(
    world: &mut crate::effects::World,
    sign_in: u64,
    group: jig_domain_core::reads::Group,
) -> Vec<jig_domain_core::documents::InboxObject> {
    use jig_domain_core::reads;
    world.sign_in = Some(sign_in);
    let mut from = None;
    let mut objects = Vec::new();
    for page_number in 0..32 {
        crate::reads::ask(
            world,
            FIXTURE_INBOX_REQUEST + page_number,
            reads::Read::Inbox {
                group,
                filter: reads::Filter { project: Some(FIXTURE_PROJECT), goal: None },
                from: from.take(),
            },
        );
        let reads::Answer::Page(page) = crate::reads::latest(world) else { panic!("inbox page") };
        for row in &page.rows {
            let reads::PageRow::Inbox(entry) = row else { panic!("inbox entry") };
            objects.push(entry.object);
        }
        from.clone_from(&page.next);
        if from.is_none() {
            return objects;
        }
    }
    panic!("bounded inbox pages")
}
/// The shared requesting party's exact proposal inbox identity.
#[must_use]
pub fn proposal_object(proposal: u64) -> jig_domain_core::documents::InboxObject {
    jig_domain_core::documents::InboxObject::Proposal {
        proposer: jig_domain_people::Proposer::Party(QUERY_REQUESTER),
        number: proposal,
    }
}

/// Sign in a second identity and give it a role, preserving the other holdings.
pub fn second_party(world: &mut crate::effects::World, subject: &[u8], role: jig_domain_people::Role) -> u64 {
    use jig_domain_people as people;
    let mut holdings = match world
        .store
        .rows
        .get(&jig_test_domain::Key::Core(jig_domain_core::Key::People(people::Key::Roles(FIXTURE_PROJECT))))
    {
        Some(jig_test_domain::Record::Core(jig_domain_core::Record::People(people::Stored::Roles {
            holdings,
            ..
        }))) => holdings.to_vec(),
        value => panic!("project roles: {value:?}"),
    };
    world.send(jig_test_domain::Event::Core(jig_domain_core::Event::SignIn {
        held: None,
        reply_to: skein_lib::ReplyTo::new(skein_lib::Token::new(FIXTURE_SIGN_IN_REQUEST)),
        identity: people::Identity {
            key: people::IdentityKey { provider: 0, subject: subject.into() },
            login: b"second".as_slice().into(),
            name: b"Second".as_slice().into(),
        },
    }));
    let Some(people::Reply::SignedIn { sign_in, person, .. }) = world.people_answers.last().cloned() else {
        panic!("second party signed in")
    };
    holdings.retain(|holding| holding.person != person);
    holdings.push(people::Holding { person, role });
    world.send(jig_test_domain::Event::Core(jig_domain_core::Event::People(people::Event::Roles {
        project: FIXTURE_PROJECT,
        holdings: holdings.into_boxed_slice(),
    })));
    sign_in
}

/// A real admitted hold spans four segments while short document lists hold two.
#[must_use]
pub fn long_resource_world(seed: u64) -> (crate::effects::World, u64, jig_test_connector::Path) {
    let (mut configuration, mut limits) = crate::effects::fixture(seed, false);
    limits.core.documents.lines = 2;
    limits.core.fleet.slots = 2;
    limits.connector.path_segments = limits.core.tasks.hold_segments;
    let mut path = skein_lib::List::with_capacity(limits.core.tasks.hold_segments);
    for segment in jig_test_connector_world::path(1, 1).segments() {
        path.push(segment.clone()).expect("prefix fits");
    }
    for segment in [b"third".as_slice(), b"last".as_slice()] {
        path.push(segment.into()).expect("four admitted segments");
    }
    let path = jig_test_connector::Path::new(path, 128).expect("bounded resource");
    configuration.first.resources[0].path = path.clone();
    crate::peers::journal(&mut limits);
    let mut world = crate::effects::World::configured(seed, false, configuration, limits);
    world.delegate_on_path(CREDENTIAL_RESOURCE, path.clone());
    let core::CallPart::Delegated(numbers) = &world.answers.last().expect("delegation answer").2 else {
        panic!("resource delegate admitted: {:?}", world.answers.last())
    };
    let child = numbers[0].task;
    world.reads = Some(crate::reads::Parties::new(world.store.applied));
    (world, child, path)
}

/// The configured authority and task bound used by a creating party.
#[derive(Clone, Debug)]
pub struct Creating {
    roles: std::collections::BTreeMap<(u32, u32), authority::Authority>,
    tasks: u32,
}
impl Creating {
    #[must_use]
    pub fn new(config: &core::Config, tasks: u32) -> Self {
        let roles = config
            .seed_projects
            .iter()
            .flat_map(|project| {
                project
                    .policy
                    .policy
                    .roles
                    .iter()
                    .map(move |role| ((project.number, role.number), role.authority.clone()))
            })
            .collect();
        Self { roles, tasks }
    }
}
/// Only a goal's supplied terms differ from ordinary creation.
#[derive(Clone, Copy, Debug, Default)]
pub struct Goal {
    pub budget: u64,
    pub priority: u32,
    pub deadline: Option<skein_lib::Wall>,
}
#[must_use]
pub fn chat(world: &crate::effects::World, project: u32, words: Box<[u8]>) -> people::Ask {
    chat_for(world, project, 0, words)
}
#[must_use]
pub fn create_chat(creating: &Creating, project: u32, role: u32, words: Box<[u8]>) -> people::Ask {
    let role =
        creating.roles.get(&(project, role)).or_else(|| creating.roles.get(&(project, 0))).expect("fixture role");
    let mut member = people_member(&[], 0);
    member.words = words;
    member.authority = core::authority_to_people(role).expect("fixture authority");
    member.authority.spend = 0;
    member.authority.delegation.tasks = member.authority.delegation.tasks.min(creating.tasks).saturating_sub(1);
    member.authority.delegation.depth = member.authority.delegation.depth.saturating_sub(1);
    member.budget = people::Budget::AsAllowed { cap: None };
    member.flags = people::Flags {
        ended_by: people::EndedBy::Result,
        conversation: true,
        listed: false,
        retention: people::Retention::Kept,
    };
    member.label = jig_client_test_records::CHAT;
    member.contract = people::Contract::Report { words: 128 };
    member.wake = people::WakePolicy {
        words: people::WakeRule::Immediate,
        notices: people::WakeRule::Immediate,
        news: people::WakeRule::Immediate,
        results: people::ResultsWake::LastOrFailure,
        questions: true,
        answers: true,
        timers: true,
    };
    people::Ask::Create { project, batch: Box::new([member]) }
}

#[must_use]
pub fn goal(
    world: &crate::effects::World,
    project: u32,
    words: Box<[u8]>,
    executor: people::TaskExecutor,
    terms: Goal,
) -> people::Ask {
    let people::Ask::Create { mut batch, .. } = chat(world, project, words) else { unreachable!() };
    let member = &mut batch[0];
    member.executor = executor;
    member.budget = people::Budget::Named;
    member.authority.spend = terms.budget;
    member.authority.deadline = terms.deadline;
    member.tracked = Some(terms.priority);
    member.flags.conversation = false;
    member.flags.listed = true;
    member.label = jig_client_test_records::GOAL;
    if matches!(executor, people::TaskExecutor::Procedure { .. }) {
        member.authority.tools = 0;
    }
    people::Ask::Create { project, batch }
}

#[must_use]
pub fn chat_for(world: &crate::effects::World, project: u32, role: u32, words: Box<[u8]>) -> people::Ask {
    let people::Ask::Create { mut batch, .. } = create_chat(&world.creating, project, role, words) else {
        unreachable!()
    };
    if let Some(root::Record::Core(core::Record::People(people::Stored::Policy { value, .. }))) =
        world.store.rows.get(&root::Key::Core(core::Key::People(people::Key::Policy(project))))
        && let Some(current) = value.roles.iter().find(|value| value.number == role)
    {
        let task_cap = world.creating.tasks;
        batch[0].authority = current.authority.clone();
        batch[0].authority.spend = 0;
        batch[0].authority.delegation.tasks = current.authority.delegation.tasks.min(task_cap).saturating_sub(1);
        batch[0].authority.delegation.depth = current.authority.delegation.depth.saturating_sub(1);
    }
    people::Ask::Create { project, batch }
}

pub mod deadlines;

/// Explicit durable key accounts used by the lower domain world; zero is a real account row.
#[must_use]
pub fn account_rows(providers: &jig_domain_accounts::Providers) -> Box<[root::Record]> {
    use jig_domain_accounts::{Holder, Kind, Stored};
    let mut rows = Vec::new();
    for (index, entry) in providers.providers.iter().filter(|p| !p.offers.none()).enumerate() {
        let index = u32::try_from(index).expect("bounded fixture value");
        let (account, source) = if index == 0 { (0, 1) } else { (2 * index + 1, 2 * index) };
        let provider = entry.number;
        {
            for row in [
                Stored::Account {
                    number: account,
                    provider,
                    holder: Holder::Deployment,
                    place: index,
                    uses: None,
                    subject: Box::new([]),
                    display: Box::new([]),
                },
                Stored::Source { account, number: source, place: 0, kind: Kind::Key, name: Box::new([]), held: None },
            ] {
                rows.push(root::Record::Core(core::Record::Accounts(row)));
            }
        }
    }
    if !rows.is_empty() {
        rows.push(root::Record::Core(core::Record::Accounts(Stored::Next {
            number: u32::try_from(rows.len()).expect("bounded fixture value"),
        })));
    }
    rows.into_boxed_slice()
}

/// A lower-world story supplies the exact durable source it will grant later.
#[must_use]
pub fn waiting_on_source(seed: u64, source: u32) -> crate::effects::World {
    let (config, limits) = crate::effects::fixture(seed, false);
    let mut rows = account_rows(&config.core.settings.llm_providers).into_vec();
    for row in &mut rows {
        match row {
            root::Record::Core(core::Record::Accounts(jig_domain_accounts::Stored::Source { number, .. })) => {
                *number = source;
            }
            root::Record::Core(core::Record::Accounts(jig_domain_accounts::Stored::Next { number })) => {
                *number = source + 1;
            }
            root::Record::Core(_) | root::Record::Connector { .. } => {}
        }
    }
    configured_sources(seed, config, limits, rows.into_boxed_slice())
}
#[must_use]
pub fn configured_sources(
    seed: u64,
    config: root::Config,
    limits: root::Limits,
    rows: Box<[root::Record]>,
) -> crate::effects::World {
    let initial = create_chat(
        &Creating::new(&config.core, limits.core.tasks.tree_tasks),
        1,
        0,
        b"make an effect".as_slice().into(),
    );
    crate::effects::World::configured_with_rows(seed, false, config, limits, initial, rows)
}

/// What a first start leaves for its seed projects, for worlds that hand-bootstrap rosters without
/// one: the deployment header marking every seeded number, then each project's policy row.
#[must_use]
pub fn seeded_project_rows(config: &core::Config) -> Vec<core::Record> {
    let mut deployment = core::Counters::bootstrap(config.deployment).deployment();
    for project in &config.seed_projects {
        deployment.projects = deployment.projects.max(u64::from(project.number));
    }
    let mut rows = vec![core::Record::Core(core::CoreRecord::Deployment(deployment))];
    for row in project_policy_rows(config) {
        let root::Record::Core(row) = row else { panic!("a policy row is the core's") };
        rows.push(row);
    }
    rows
}

/// Complete policy rows for worlds that hand-bootstrap rosters before cold restore.
pub(crate) fn project_policy_rows(config: &core::Config) -> Vec<root::Record> {
    config
        .seed_projects
        .iter()
        .map(|project| {
            let policy = &project.policy.policy;
            let grant = |value: &authority::Grant| people::Grant {
                connector: value.connector,
                kind: value.kind,
                pattern: core::pattern_to_people(&value.pattern),
            };
            let value = people::PolicyValue {
                ceiling: core::authority_to_people(&policy.ceiling).expect("bounded ceiling"),
                escalation_role: policy.escalation_role,
                period_spend: policy.period_spend,
                projections: policy.projections.iter().map(grant).collect(),
                roles: policy
                    .roles
                    .iter()
                    .map(|role| people::PolicyRole {
                        number: role.number,
                        authority: core::authority_to_people(&role.authority).expect("bounded role"),
                        period_spend: role.period_spend,
                        requests: role.requests.0,
                        decides: role.decides.0,
                    })
                    .collect(),
                requirements: policy
                    .requirements
                    .iter()
                    .map(|requirement| people::Requirement {
                        connector: requirement.connector,
                        kind: requirement.kind,
                        pattern: core::pattern_to_people(&requirement.pattern),
                        judge: people::Judge {
                            connector: requirement.judge.connector,
                            requirement: requirement.judge.requirement,
                            parameters: requirement.judge.parameters,
                        },
                        guard: match requirement.guard {
                            authority::Guard::Guarded => people::Guard::Guarded,
                            authority::Guard::Observed { freshness } => people::Guard::Observed { freshness },
                        },
                        must_be_guarded: requirement.must_be_guarded,
                    })
                    .collect(),
                permissions: project.policy.permissions.clone(),
                templates: project.policy.templates.clone(),
            };
            root::Record::Core(core::Record::People(people::Stored::Policy {
                project: project.number,
                name: project.name.clone(),
                value,
            }))
        })
        .collect()
}

/// Names for the standard fixture's two projects.
#[must_use]
pub fn project_names() -> skein_lib::Map<u32, Box<str>> {
    let mut names = skein_lib::Map::with_capacity(2);
    names.insert(1, Box::from("Project 1")).expect("first project name");
    names.insert(2, Box::from("Project 2")).expect("second project name");
    names
}

/// Restore project policies from a world's durable rows before its tasks.
pub fn restore_project_policies(core: &mut core::Core, limits: &core::Limits, store: &crate::Store) {
    for row in store.rows.values() {
        if let root::Record::Core(core::Record::People(people::Stored::Policy { project, name, value })) = row {
            assert!(core.restore_policy(limits, *project, name.clone(), value.clone()), "durable policy restores");
        }
    }
}

pub use jig_test_domain::initial_settings_current;

/// A delegate that writes one of the resource fixture's holds.
#[must_use]
pub fn resource_member(world: &crate::effects::World, id: u64, words: &[u8]) -> core::Delegate {
    let mut child = member(world, tasks::Executor::Agent { charter: 1 }, words);
    child.spec.parameters = Box::new([tasks::Parameter::Resource { name: 1, connector: 1, resource: id }]);
    child.authority.budget.spend = 10;
    child.authority.delegation = tasks::Delegation { kinds: Box::new([]), tasks: 0, depth: 0 };
    child
}

/// Adopt the exclusive and pooled resources used by writer and hold stories.
pub fn configure_resources(world: &mut crate::effects::World) {
    for id in [1, 2] {
        world.send(root::Event::Connector {
            number: 1,
            event: jig_test_connector::Event::Adopt {
                project: 1,
                resource: jig_test_connector_world::path(1, id),
                role: jig_test_connector::ResourceRole::Owned,
            },
        });
    }
    world.configure_holds(
        1,
        Box::new([
            tasks::Kind { connector: 1, kind: 1, hold: tasks::HoldKind::Exclusive { taken: tasks::Taken::Waits } },
            tasks::Kind { connector: 1, kind: 2, hold: tasks::HoldKind::Pooled { taken: tasks::Taken::Waits } },
        ]),
    );
}
