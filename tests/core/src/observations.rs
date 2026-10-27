//! Translations at the testing root's outside boundary. Only configuration,
//! durable store rows and peer traffic enter here; no domain state is read.
//! The conformance world can replace these translations and keep the referee
//! (`domain/testing.md`, 4, 6).
pub mod acceptance;
pub mod intents;
mod projects;

use std::collections::{BTreeMap, BTreeSet};

use crate::{Store, referee as r};
use jig_domain_authority as authority;
use jig_domain_core as core;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_test_connector as connector;
use jig_test_domain as root;

fn pattern(base: &[Box<[u8]>], terminal: &[u8], open: bool) -> r::Pattern {
    r::Pattern { base: base.iter().map(|part| part.to_vec()).collect(), terminal: terminal.to_vec(), open }
}

fn authority_pattern(value: &authority::Pattern) -> r::Pattern {
    match &value.last {
        authority::Last::Exact(last) => pattern(&value.segments, last, false),
        authority::Last::Open(last) => pattern(&value.segments, last, true),
    }
}

fn task_pattern(value: &tasks::Pattern) -> r::Pattern {
    match &value.last {
        tasks::Last::Exact(last) => pattern(&value.segments, last, false),
        tasks::Last::Open(last) => pattern(&value.segments, last, true),
    }
}

/// Translate configured authority for an application's independent referee.
#[must_use]
pub fn scope(value: &authority::Authority) -> r::Scope {
    r::Scope {
        budget: value.budget.spend,
        deadline: value.budget.deadline.map(skein_lib::Wall::as_nanos),
        tools: value.tools.0,
        notes: value.notes.0,
        grants: authority_grants(&value.grants),
        executors: value
            .delegation
            .kinds
            .iter()
            .map(|kind| match kind {
                authority::Executor::Charter(number) => (0, *number),
                authority::Executor::Procedure(number) => (1, *number),
                authority::Executor::Role(number) => (2, *number),
            })
            .collect(),
        descendants: value.delegation.tasks,
        depth: value.delegation.depth,
        note_resources: value
            .note_resources
            .iter()
            .map(|resource| (resource.connector, authority_pattern(&resource.pattern)))
            .collect(),
    }
}

fn authority_grants(grants: &[authority::Grant]) -> Vec<r::Grant> {
    grants
        .iter()
        .map(|grant| r::Grant {
            connector: grant.connector,
            kind: grant.kind,
            pattern: authority_pattern(&grant.pattern),
        })
        .collect()
}

fn task_scope(value: &tasks::Authority) -> r::Scope {
    r::Scope {
        budget: value.budget.spend,
        deadline: value.budget.deadline.map(skein_lib::Wall::as_nanos),
        tools: value.tools.0,
        notes: value.notes.0,
        grants: value
            .grants
            .iter()
            .map(|grant| r::Grant {
                connector: grant.connector,
                kind: grant.kind,
                pattern: task_pattern(&grant.pattern),
            })
            .collect(),
        executors: value
            .delegation
            .kinds
            .iter()
            .map(|kind| match kind {
                tasks::AuthorityExecutor::Charter(number) => (0, *number),
                tasks::AuthorityExecutor::Procedure(number) => (1, *number),
                tasks::AuthorityExecutor::Role(number) => (2, *number),
            })
            .collect(),
        descendants: value.delegation.tasks,
        depth: value.delegation.depth,
        note_resources: value
            .note_resources
            .iter()
            .map(|resource| (resource.connector, task_pattern(&resource.pattern)))
            .collect(),
    }
}

fn name(value: &tasks::Name) -> r::Name {
    r::Name { connector: value.connector, path: value.path.iter().map(|part| part.to_vec()).collect() }
}

fn path(connector: u16, value: &connector::Path) -> r::Name {
    r::Name { connector, path: value.segments().iter().map(|part| part.to_vec()).collect() }
}

fn source(value: tasks::Party) -> r::Source {
    match value {
        tasks::Party::Task(task) => r::Source::Task(task),
        tasks::Party::Person(person) => r::Source::Person(person),
        tasks::Party::Deployment { .. } => r::Source::Deployment,
    }
}

fn phase(value: &tasks::Phase) -> r::Phase {
    match value {
        tasks::Phase::Waiting => r::Phase::Waiting,
        tasks::Phase::Active(_) => r::Phase::Active,
        tasks::Phase::Closing(_) => r::Phase::Closing,
        tasks::Phase::Held { .. } => r::Phase::Held,
        tasks::Phase::Ended(tasks::Ending::Done(_)) => r::Phase::Done,
        tasks::Phase::Ended(tasks::Ending::Failed { .. } | tasks::Ending::Cancelled { .. }) => r::Phase::Ended,
    }
}

/// Translate a durable task row without consulting live engine state.
#[must_use]
pub fn task(value: &tasks::TaskRecord) -> r::Task {
    r::Task {
        settings: r::SettingsRevision { task: value.settings.task, run: value.settings.run },
        tracked: value.tracked.is_some(),
        number: value.number,
        project: value.project,
        source: source(value.requester),
        scope: task_scope(&value.authority),
        executor: match value.executor {
            tasks::Executor::Agent { charter } => (0, charter),
            tasks::Executor::Procedure { code, .. } => (1, code),
            tasks::Executor::Person(tasks::PersonAddress::Role(role)) => (2, role),
            tasks::Executor::Person(tasks::PersonAddress::Person(_)) => (2, 0),
        },
        budget: value.numbers.budget,
        spent: value.numbers.spent,
        spent_below: value.numbers.spent_below,
        reserved: value.numbers.reserved,
        run_reserved: value.run_reserved,
        run_spent: value.run_spent,
        attempt: value.attempt,
        period: value.recurring.as_ref().map(|recurring| recurring.last_period),
        phase: phase(&value.phase),
        spec_revision: value.revisions.spec,
        hold_revision: value.revisions.hold,
        dependencies: value.dependencies.to_vec(),
        delegates: value.delegates.to_vec(),
        pools: if value.holds_taken && !matches!(value.phase, tasks::Phase::Ended(_)) {
            value
                .holdings
                .iter()
                .filter_map(|holding| match holding {
                    tasks::Holding::Slot { pool, .. } => Some(name(pool)),
                    tasks::Holding::Write { .. } => None,
                })
                .collect()
        } else {
            Vec::new()
        },
        inbox: value.inbox.iter().map(|word| word.number).collect(),
        proposal: value.proposal.as_ref().map(|proposal| {
            let tasks::ProposalState::Pending { holder, .. } = proposal.state else {
                panic!("live proposal is pending")
            };
            (
                proposal.number,
                match holder {
                    tasks::ProposalHolder::Task(task) => r::Source::Task(task),
                    tasks::ProposalHolder::Person(person) => r::Source::Person(person),
                    tasks::ProposalHolder::Policy { .. } => r::Source::Deployment,
                },
            )
        }),
    }
}

fn call(key: core::CallKey) -> r::Receipt {
    r::Receipt::Call(key.task, key.attempt, key.completion, key.position)
}

#[derive(Clone, Debug)]
struct SeenFence {
    after: u64,
    position: Option<people::Seen>,
}

impl SeenFence {
    fn new(store: &Store, person: u64, seen: people::Seen) -> Self {
        let mut current = 0;
        let mut ended = false;
        let records = store.rows.values().chain(store.pending.iter().flat_map(|pending| {
            pending.writes.iter().filter_map(|write| match write {
                root::Write::Save(record) => Some(record),
                root::Write::Erase(_) | root::Write::CopySettingsRevision(_) => None,
            })
        }));
        for record in records {
            match seen {
                people::Seen::Results { .. } => {
                    if let root::Record::Core(core::Record::People(people::Stored::ReadResults { party, through })) =
                        record
                        && *party == person
                    {
                        current = current.max(*through);
                    }
                }
                people::Seen::Conversation { task, .. } => {
                    if let root::Record::Core(core::Record::Tasks(tasks::Stored::Ended(row))) = record {
                        ended |= row.number == task;
                    }
                    if let root::Record::Core(core::Record::People(
                        people::Stored::ReadConversation { party, task: chat, through }
                        | people::Stored::EndedRead { party, task: chat, through },
                    )) = record
                        && *party == person
                        && *chat == task
                    {
                        current = current.max(*through);
                    }
                }
            }
        }
        // Results and live conversation marks at or below the current position
        // write nothing. Ended marks always save max(old, wanted), even if
        // forgetting erases that position in the same commit.
        let position = match seen {
            people::Seen::Results { through } => (through > current).then_some(seen),
            people::Seen::Conversation { task, through } => (ended || through > current)
                .then_some(people::Seen::Conversation { task, through: current.max(through) }),
        };
        Self { after: store.pending.back().map_or(store.applied, |pending| pending.number), position }
    }
}

/// The testing root adapter, reusable with the ordinary referee by other worlds.
#[derive(Clone, Debug)]
pub struct Observer {
    pub referee: r::Referee,
    intents: intents::Intents,
    project_makes: projects::Makes,
    policy_generation: u64,
    roles: BTreeMap<(u32, u32), r::Scope>,
    decisions_by_role: BTreeMap<(u32, u32), u8>,
    applied: u64,
    application_requests: bool,
    activity: BTreeMap<(u64, u64), tasks::ActivityLine>,
    effects: [usize; 2],
    reads: [usize; 2],
    // Seen uses a commit fence instead of a new Answer receipt.
    requests: BTreeMap<u64, (u64, [u8; 16], Option<SeenFence>)>,
    proposals: BTreeMap<u64, (u16, connector::Effect)>,
    accepted: BTreeMap<(u16, connector::Key), r::Source>,
    settled_calls: BTreeSet<(u64, u64, u64)>,
    // Durable fresh marks fence these attempts: their traffic is acknowledged
    // and discarded without a turn or terminal row (engine.md, 7.2 and 8).
    crossed: BTreeSet<(u64, u64)>,
    words: BTreeSet<(u64, u64)>,
    policy_changes: BTreeMap<(u64, [u8; 16]), (u32, people::PolicyChange)>,
    applied_changes: BTreeSet<(u64, [u8; 16])>,
    forgetting: BTreeMap<u64, Vec<u64>>,
    recorded: Option<Vec<r::Observed>>,
}

impl Observer {
    /// Capture policy from scenario configuration before constructing the root.
    #[must_use]
    pub fn new(configuration: &root::Config) -> Self {
        let deployment = &configuration.core.defaults.rules;
        let mut policy = r::Policy {
            deployment: scope(&deployment.ceiling),
            projects: BTreeMap::new(),
            projections: BTreeMap::new(),
            people: BTreeMap::new(),
            effect_accepters: BTreeSet::new(),
            requirements: Vec::new(),
            implications: BTreeSet::new(),
            kinds: BTreeMap::new(),
            owned: Vec::new(),
            participating: BTreeSet::new(),
            mechanics: BTreeSet::new(),
            delivery_bound: 60_000_000_000,
            uncertainty_bound: 60_000_000_000,
            story_steps: 30_000,
        };
        let mut roles = BTreeMap::new();
        let mut decisions_by_role = BTreeMap::new();
        // The testing application's configured projects are contiguous and small.
        for seed in &configuration.core.seed_projects {
            let project = seed.number;
            {
                let value = &seed.policy.policy;
                policy.projects.insert(project, scope(&value.ceiling));
                policy.projections.insert(project, authority_grants(&value.projections));
                for role in &value.roles {
                    roles.insert((project, role.number), scope(&role.authority));
                    decisions_by_role.insert((project, role.number), role.decides.0);
                }
                for (requirement, scope) in deployment
                    .requirements
                    .iter()
                    .map(|r| (r, None))
                    .chain(value.requirements.iter().map(|r| (r, Some(project))))
                {
                    policy.requirements.push(r::Requirement {
                        project: scope,
                        connector: requirement.connector,
                        kind: requirement.kind,
                        pattern: authority_pattern(&requirement.pattern),
                        judge: requirement.judge.connector,
                        freshness: match requirement.guard {
                            authority::Guard::Guarded => None,
                            authority::Guard::Observed { freshness } => Some(freshness.as_nanos()),
                        },
                    });
                }
            }
        }
        for (number, connector) in [(1, &configuration.first), (2, &configuration.second)] {
            let parts = connector.prefix.segments();
            // The deployment prefix owns its complete subtree, but not the prefix object itself.
            policy.owned.push((
                number,
                r::Pattern { base: parts.iter().map(|part| part.to_vec()).collect(), terminal: Vec::new(), open: true },
            ));
            for kind in &connector.kinds {
                policy.kinds.insert(
                    (number, kind.kind),
                    r::Kind {
                        price: kind.price.unwrap_or(0),
                        recovery: match kind.recovery {
                            connector::Recovery::Keyed => r::Recovery::Keyed,
                            connector::Recovery::Conditional => r::Recovery::Conditional,
                            connector::Recovery::Idempotent => r::Recovery::Idempotent,
                            connector::Recovery::Unrecoverable => r::Recovery::Unrecoverable,
                        },
                    },
                );
            }
        }
        // Enumerate the finite configured kind vocabulary rather than reuse an
        // authority decision function when checking a later observed effect.
        for &(connector, given) in policy.kinds.keys() {
            for &(other, needed) in policy.kinds.keys() {
                if connector == other && given != needed && deployment.implies.allows(connector, needed, given) {
                    policy.implications.insert((connector, given, needed));
                }
            }
        }
        Self {
            referee: r::Referee::new(policy),
            policy_generation: 0,
            roles,
            decisions_by_role,
            applied: 0,
            application_requests: false,
            activity: BTreeMap::new(),
            effects: [0; 2],
            reads: [0; 2],
            requests: BTreeMap::new(),
            proposals: BTreeMap::new(),
            accepted: BTreeMap::new(),
            settled_calls: BTreeSet::new(),
            crossed: BTreeSet::new(),
            words: BTreeSet::new(),
            policy_changes: BTreeMap::new(),
            applied_changes: BTreeSet::new(),
            forgetting: BTreeMap::new(),
            recorded: None,
            intents: intents::Intents::configured(configuration),
            project_makes: projects::Makes::new(configuration),
        }
    }

    /// Submit one translated observation, identifying the first broken promise.
    pub fn observe(&mut self, now: u64, observed: r::Observed) {
        if let Some(recorded) = &mut self.recorded {
            recorded.push(observed);
        } else {
            self.referee.observe(now, observed).unwrap_or_else(|error| panic!("referee {error:?} at {now}"));
        }
    }

    /// Translate boundary evidence for a referee owned by an enclosing harness.
    #[must_use]
    pub fn recording(configuration: &root::Config) -> Self {
        let mut observer = Self::new(configuration);
        observer.recorded = Some(Vec::new());
        observer
    }

    /// Take observations in the order the neighbours and store produced them.
    #[must_use]
    pub fn take(&mut self) -> Vec<r::Observed> {
        std::mem::take(self.recorded.as_mut().expect("recording observer"))
    }

    /// Observe inbound peer traffic, before the root may decide it.
    pub fn inbound(&mut self, store: &Store, now: u64, event: &mut root::Event) {
        self.project_makes.inbound(store, event);
        for observed in self.intents.inbound_root(store, event) {
            self.observe(now, observed);
        }
        match event {
            root::Event::Core(core::Event::People(people::Event::Ask { models: _, reply_to, sign_in, key, ask })) => {
                let person = store.rows.values().find_map(|row| match row {
                    root::Record::Core(core::Record::People(people::Stored::SignIn { number, person, .. }))
                        if number == sign_in =>
                    {
                        Some(*person)
                    }
                    root::Record::Core(_) | root::Record::Connector { .. } => None,
                });
                if let Some(person) = person {
                    let to = destination(reply_to);
                    let seen = if let people::Ask::MarkSeen { seen } = ask {
                        Some(SeenFence::new(store, person, *seen))
                    } else {
                        None
                    };
                    self.requests.insert(to, (person, *key, seen));
                    if let people::Ask::ChangePolicy { project, change } = ask {
                        self.policy_changes.insert((person, *key), (*project, change.clone()));
                    }
                }
            }
            root::Event::Turn { task, attempt, cumulative, .. }
            | root::Event::Answer { task, attempt, cumulative, .. } => {
                self.observe(now, r::Observed::Completion { task: *task, attempt: *attempt, cumulative: *cumulative });
            }
            root::Event::Connector {
                number,
                event:
                    connector::Event::System(connector::SystemEvent::Applied {
                        entry,
                        result: connector::ApplyResult::Uncertain,
                        ..
                    }),
            } => {
                if let Some(key) = store.rows.values().find_map(|row| match row {
                    root::Record::Connector { number: owner, record: connector::Record::Outbox(outbox) }
                        if owner == number && outbox.number == *entry =>
                    {
                        Some(outbox.key)
                    }
                    root::Record::Core(_) | root::Record::Connector { .. } => None,
                }) {
                    self.observe(now, r::Observed::Uncertain { connector: *number, key: key.into() });
                }
            }
            root::Event::Application { .. } => {
                if let Some(observed) = crate::application_observations::requested(store, event) {
                    self.application_requests = true;
                    self.observe(now, observed);
                }
            }
            root::Event::Link { .. }
            | root::Event::ResultRead { .. }
            | root::Event::WatchApplication { .. }
            | root::Event::ReadApplication { .. }
            | root::Event::Stop
            | root::Event::RestartBegin
            | root::Event::RestartDone(_)
            | root::Event::Timer(_)
            | root::Event::Core(_)
            | root::Event::Credential { .. }
            | root::Event::Connector { .. }
            | root::Event::ProjectionEffect { .. }
            | root::Event::CallAnswered { .. }
            | root::Event::TopicCall { .. }
            | root::Event::EffectCall { .. }
            | root::Event::TranscriptLoaded { .. }
            | root::Event::Committed { .. }
            | root::Event::WriterRead { .. }
            | root::Event::ConnectorTimer { .. }
            | root::Event::Text { .. }
            | root::Event::Failed { .. } => {}
        }
    }

    /// Capture reads before decisions and arrivals before their results reach the root.
    pub fn systems(&mut self, now: u64, systems: &[jig_test_system::System; 2]) {
        for (index, system) in systems.iter().enumerate() {
            let connector = u16::try_from(index + 1).expect("two systems");
            for read in &system.reads()[self.reads[index]..] {
                self.observe(
                    now,
                    r::Observed::Read {
                        resource: r::Name { connector, path: read.resource.0.clone() },
                        state: read.state,
                        at: read.at,
                    },
                );
            }
            self.reads[index] = system.reads().len();
            for effect in &system.observed()[self.effects[index]..] {
                self.observe(now, r::Observed::Effect { connector, effect: effect.clone() });
            }
            self.effects[index] = system.observed().len();
        }
    }

    /// Revision of the independently observed policy used by the enclosing referee.
    #[must_use]
    pub const fn policy_generation(&self) -> u64 {
        self.policy_generation
    }

    /// A restart changes future makes' configured policies, never an existing project's.
    pub fn configure_projects(&mut self, config: &root::Config) {
        self.project_makes.configure(config);
    }

    fn made_project(&mut self, project: u32, policy: &authority::Policy) {
        self.policy_generation = self.policy_generation.checked_add(1).expect("policy generation fits");
        self.referee.policy.projects.insert(project, scope(&policy.ceiling));
        self.referee.policy.projections.insert(project, authority_grants(&policy.projections));
        for role in &policy.roles {
            self.roles.insert((project, role.number), scope(&role.authority));
            self.decisions_by_role.insert((project, role.number), role.decides.0);
        }
        for requirement in &policy.requirements {
            self.referee.policy.requirements.push(r::Requirement {
                project: Some(project),
                connector: requirement.connector,
                kind: requirement.kind,
                pattern: authority_pattern(&requirement.pattern),
                judge: requirement.judge.connector,
                freshness: match requirement.guard {
                    authority::Guard::Guarded => None,
                    authority::Guard::Observed { freshness } => Some(freshness.as_nanos()),
                },
            });
        }
    }

    fn policy_change(&mut self, project: u32, change: &people::PolicyChange) {
        self.policy_generation = self.policy_generation.checked_add(1).expect("policy generation fits");
        match change {
            people::PolicyChange::Projections { grants } => {
                self.referee.policy.projections.insert(
                    project,
                    grants
                        .iter()
                        .map(|grant| r::Grant {
                            connector: grant.connector,
                            kind: grant.kind,
                            pattern: person_pattern(&grant.pattern),
                        })
                        .collect(),
                );
            }
            people::PolicyChange::Role(role) => {
                self.roles.insert((project, role.number), person_scope(&role.authority));
                self.decisions_by_role.insert((project, role.number), role.decides);
            }
            people::PolicyChange::Requirements { requirements } => {
                self.referee.policy.requirements.retain(|r| r.project != Some(project));
                for requirement in requirements {
                    self.referee.policy.requirements.push(r::Requirement {
                        project: Some(project),
                        connector: requirement.connector,
                        kind: requirement.kind,
                        pattern: person_pattern(&requirement.pattern),
                        judge: requirement.judge.connector,
                        freshness: match requirement.guard {
                            people::Guard::Guarded => None,
                            people::Guard::Observed { freshness } => Some(freshness.as_nanos()),
                        },
                    });
                }
            }
            people::PolicyChange::ProjectSpend { .. }
            | people::PolicyChange::Permissions { .. }
            | people::PolicyChange::Templates { .. } => {}
        }
    }

    /// Compare the cold store with the receipts that outside observers saw.
    pub fn cold(&mut self, now: u64, store: &Store) {
        self.requests.retain(|_, (_, _, seen)| seen.is_none());
        let receipts = store
            .rows
            .values()
            .filter_map(|row| match row {
                root::Record::Core(core::Record::Tasks(tasks::Stored::Live(value) | tasks::Stored::Ended(value))) => {
                    Some(r::Receipt::Task(value.number))
                }
                root::Record::Core(core::Record::Core(core::CoreRecord::RunProof(value))) => {
                    if value.crossed {
                        self.crossed.insert((value.task, value.attempt));
                    }
                    Some(r::Receipt::Claim(value.task, value.attempt))
                }
                root::Record::Core(core::Record::Core(core::CoreRecord::Turn(value))) => {
                    Some(r::Receipt::Turn(value.task, value.attempt, value.turn))
                }
                root::Record::Core(core::Record::Core(core::CoreRecord::Terminal(value))) => {
                    Some(r::Receipt::Terminal(value.task, value.attempt))
                }
                root::Record::Core(core::Record::Core(core::CoreRecord::Call(value))) => Some(call(value.key)),
                root::Record::Core(_) | root::Record::Connector { .. } => None,
            })
            .chain(store.rows.values().filter_map(|row| match row {
                root::Record::Core(core::Record::Tasks(tasks::Stored::Ended(value))) => {
                    Some(r::Receipt::Ended(value.number))
                }
                root::Record::Core(_) | root::Record::Connector { .. } => None,
            }))
            .collect();
        self.observe(now, r::Observed::ColdStore { receipts });
    }

    /// Expenses and lost turns retained independently by the scripted hosts.
    pub fn workers(&mut self, now: u64, workers: &[jig_fake_workers::Worker]) {
        let mut totals: BTreeMap<u64, (u64, u64)> = BTreeMap::new();
        for worker in workers {
            for ((task, attempt), expense) in &worker.expenses {
                if expense.lost_turns > 0 {
                    self.observe(
                        now,
                        r::Observed::LostTurns {
                            task: *task,
                            attempt: *attempt,
                            turns: expense.lost_turns,
                            spent: expense.lost,
                        },
                    );
                }
                let total = totals.entry(*task).or_default();
                total.0 += expense.spent;
                total.1 += expense.lost;
            }
        }
        for (task, (spent, lost_bound)) in totals {
            self.observe(now, r::Observed::Spend { task, spent, lost_bound });
        }
    }

    /// Observe the application records the store has just received.
    pub fn submitted(&mut self, now: u64, store: &Store) {
        self.intents.submitted(store);
        if let Some(pending) = store.pending.back() {
            for (person, _, seen) in self.requests.values_mut() {
                let Some(fence) = seen else { continue };
                let Some(position) = fence.position else { continue };
                for write in &pending.writes {
                    let moves_position = match position {
                        people::Seen::Results { through } => matches!(write,
                            root::Write::Save(root::Record::Core(core::Record::People(people::Stored::ReadResults { party, through: written })))
                                if party == person && *written == through
                        ),
                        people::Seen::Conversation { task, through } => matches!(write,
                            root::Write::Save(root::Record::Core(core::Record::People(
                                people::Stored::ReadConversation { party, task: chat, through: written }
                                | people::Stored::EndedRead { party, task: chat, through: written }
                            ))) if party == person && *chat == task && *written == through
                        ),
                    };
                    if moves_position {
                        fence.after = fence.after.max(pending.number);
                        // A later mark or live-to-ended copy cannot retag this reply.
                        fence.position = None;
                        break;
                    }
                }
            }
            let mut tasks = Vec::new();
            for write in &pending.writes {
                if let root::Write::Erase(root::Key::Core(core::Key::Tasks(tasks::Key::Ended(task)))) = write {
                    for key in store.rows.keys() {
                        if matches!(key, root::Key::Core(core::Key::People(people::Key::ReadConversation { task: chat, .. })) if chat == task)
                        {
                            assert!(
                                pending.writes.contains(&root::Write::Erase(key.clone())),
                                "read marker erases with its owner"
                            );
                        }
                    }
                    tasks.push(*task);
                }
            }
            if !tasks.is_empty() {
                self.forgetting.insert(pending.number, tasks);
            }
        }
        if !self.application_requests {
            return;
        }
        for observed in crate::application_observations::submitted(store) {
            self.observe(now, observed);
        }
    }

    /// Snapshot only the store's durable rows, including held acknowledgements.
    pub fn durable(&mut self, now: u64, store: &Store) {
        self.durable_at(now, store, store.applied);
    }

    /// A mark-only upgrade writes windows and the seal's header. Neither is
    /// part of the referee's snapshot: each patch commit has the same view.
    pub fn fresh_mark(&mut self, now: u64, store: &Store) {
        for number in self.applied + 1..=store.applied {
            self.durable_at(now, store, number);
        }
    }

    #[expect(clippy::too_many_lines, reason = "one boundary translation projects every durable family")]
    fn durable_at(&mut self, now: u64, store: &Store, number: u64) {
        if number == self.applied {
            return;
        }
        for observed in self.intents.durable(store) {
            self.observe(now, observed);
        }
        for (project, policy) in self.project_makes.durable(store) {
            if !self.referee.policy.projects.contains_key(&project) {
                self.made_project(project, &policy);
            }
        }
        // Apply only an externally requested policy edit whose durable answer
        // says it succeeded. Saved policy carriers are never an authority oracle.
        for row in store.rows.values() {
            if let root::Record::Core(core::Record::People(people::Stored::Answer {
                key,
                outcome: people::Outcome::PolicyChanged { .. },
                ..
            })) = row
                && self.applied_changes.insert((key.person, key.key))
                && let Some((project, change)) = self.policy_changes.get(&(key.person, key.key)).cloned()
            {
                self.policy_change(project, &change);
            }
        }
        let mut snapshot = r::Snapshot::default();
        let mut decisions = Vec::new();
        let mut task_acceptances = Vec::new();
        let mut authority_acceptances = Vec::new();
        let mut words = Vec::new();
        let mut reached = Vec::new();
        for row in store.rows.values() {
            match row {
                root::Record::Core(core::Record::Core(core::CoreRecord::Settings(
                    core::settings::Record::Revision { revision, history: false },
                ))) => {
                    snapshot.newest = snapshot.newest.max(revision.head.number);
                }
                root::Record::Core(core::Record::Tasks(tasks::Stored::PersonProposal(value)))
                    if matches!(value.state, tasks::PersonProposalState::Pending { .. }) =>
                {
                    snapshot.party_proposals.insert(value.number, value.proposer);
                }
                root::Record::Core(core::Record::Tasks(tasks::Stored::Live(value) | tasks::Stored::Ended(value))) => {
                    let mut snapshot_task = task(value);
                    snapshot_task.tracked |= store.rows.contains_key(&root::Key::Core(core::Key::Core(
                        core::CoreKey::Projection(core::ProjectionKey::Goal(value.number)),
                    )));
                    snapshot.tasks.insert(value.number, snapshot_task);
                    snapshot.receipts.insert(r::Receipt::Task(value.number));
                    if let tasks::Escalation::Waiting { holder: tasks::EscalationHolder::Task(holder), entry, .. } =
                        value.escalation
                    {
                        snapshot.receipts.insert(r::Receipt::Word(holder, entry));
                    }
                    if let Some(proposal) = &value.proposal
                        && let tasks::ProposalState::Pending { holder: tasks::ProposalHolder::Task(holder), .. } =
                            proposal.state
                    {
                        snapshot.receipts.insert(r::Receipt::Word(holder, proposal.number));
                    }
                    if matches!(value.phase, tasks::Phase::Ended(_)) {
                        snapshot.receipts.insert(r::Receipt::Ended(value.number));
                    }
                    for word in &value.inbox {
                        snapshot.receipts.insert(r::Receipt::Word(value.number, word.number));
                        if matches!(word.from, tasks::Party::Person(_))
                            && matches!(word.kind, tasks::MessageKind::Words | tasks::MessageKind::Answer { .. })
                            && self.words.insert((value.number, word.number))
                        {
                            words.push((value.number, word.number));
                        }
                        if matches!(word.kind, tasks::MessageKind::Result(_))
                            && let tasks::Party::Task(child) = word.from
                        {
                            reached.push(r::Receipt::Ended(child));
                        }
                    }
                }
                root::Record::Core(core::Record::Tasks(tasks::Stored::Activity(value))) => {
                    let kept = self.activity.entry((value.task, value.position)).or_insert_with(|| value.clone());
                    assert_eq!(kept, value, "immutable activity row changed");
                }
                root::Record::Core(core::Record::Tasks(tasks::Stored::History(value))) => {
                    if let Some(accepted) = acceptance::task(value) {
                        authority_acceptances.push(accepted);
                    }
                    if value.change == tasks::Change::ProposalAccepted
                        && let Some(proposal) = &value.proposal
                        && let tasks::ProposalAction::Effect { connector, attempt, completion, position, .. } =
                            proposal.action
                    {
                        task_acceptances.push((
                            connector,
                            value.task,
                            attempt,
                            completion,
                            position,
                            source(value.by),
                            proposal.number,
                        ));
                    }
                }
                root::Record::Core(core::Record::Tasks(tasks::Stored::Writer(value))) => {
                    let (task, attempt) = match value.writer {
                        tasks::Writer::Run { task, attempt } => (task, attempt),
                        tasks::Writer::Effect { entry } => (0, entry),
                    };
                    snapshot.writers.push((name(&value.resource), task, attempt));
                }
                root::Record::Core(core::Record::Tasks(tasks::Stored::Pool(value))) => {
                    snapshot.pools.insert(name(&value.pool), value.slots);
                }
                root::Record::Core(core::Record::Tasks(
                    tasks::Stored::Ledger(value) | tasks::Stored::ClosedLedger(value),
                )) => {
                    snapshot.balances.push((
                        value.numbers.budget,
                        value.numbers.spent,
                        value.numbers.spent_below,
                        value.numbers.reserved,
                    ));
                    if let tasks::Funder::Pool { person, .. } = value.funder {
                        *snapshot.person_spend.entry(person).or_default() += value.numbers.spent;
                    }
                }
                root::Record::Core(core::Record::People(people::Stored::Roles { project, holdings })) => {
                    self.policy_generation = self.policy_generation.checked_add(1).expect("policy generation fits");
                    self.referee.policy.people.retain(|(number, _), _| number != project);
                    self.referee.policy.effect_accepters.retain(|(number, _)| number != project);
                    for holding in holdings {
                        if let Some(scope) = self.roles.get(&(*project, holding.role.number())) {
                            self.referee.policy.people.insert((*project, holding.person), scope.clone());
                            if self
                                .decisions_by_role
                                .get(&(*project, holding.role.number()))
                                .is_some_and(|bits| bits & 2 != 0)
                            {
                                self.referee.policy.effect_accepters.insert((*project, holding.person));
                            }
                        }
                    }
                }
                root::Record::Core(core::Record::People(
                    people::Stored::ReadConversation { party, task, through }
                    | people::Stored::EndedRead { party, task, through },
                )) => {
                    snapshot.read_positions.insert((*party, *task), *through);
                }
                root::Record::Core(core::Record::People(people::Stored::SignIn { number, .. })) => {
                    snapshot.receipts.insert(r::Receipt::SignIn(*number));
                }
                root::Record::Core(core::Record::People(people::Stored::Answer { key, ask, outcome, .. })) => {
                    snapshot.receipts.insert(r::Receipt::Party(key.person, key.key));
                    if let people::Ask::Say { task, .. } = ask.as_ref()
                        && let people::Outcome::Said { task: answered, message } = outcome
                    {
                        assert_eq!(task, answered, "party's words answered for another task");
                        if self.words.insert((*task, *message)) {
                            words.push((*task, *message));
                        }
                    }
                }
                root::Record::Core(core::Record::Core(core::CoreRecord::RunProof(value))) => {
                    snapshot.receipts.insert(r::Receipt::Claim(value.task, value.attempt));
                    if value.crossed {
                        self.crossed.insert((value.task, value.attempt));
                    }
                }
                root::Record::Core(core::Record::Core(core::CoreRecord::Turn(value))) => {
                    snapshot.receipts.insert(r::Receipt::Turn(value.task, value.attempt, value.turn));
                }
                root::Record::Core(core::Record::Core(core::CoreRecord::Terminal(value))) => {
                    snapshot.receipts.insert(r::Receipt::Terminal(value.task, value.attempt));
                }
                root::Record::Core(core::Record::Core(core::CoreRecord::Call(value))) => {
                    snapshot.receipts.insert(call(value.key));
                    if let Some(settled) = &value.settled {
                        self.settled_calls.insert((value.key.task, value.key.attempt, settled.serial));
                    }
                    if let core::CallPart::Delegated(members) = &value.part {
                        snapshot.batches.push(members.iter().map(|member| member.task).collect());
                    }
                }
                root::Record::Core(core::Record::Core(core::CoreRecord::ProposalDecision(value)))
                    if value.choice == people::ProposalChoice::Accepted =>
                {
                    decisions.push(*value);
                }
                root::Record::Connector {
                    number,
                    record: connector::Record::Proposal { number: proposal, effect, .. },
                } => {
                    self.proposals.insert(*proposal, (*number, effect.clone()));
                }
                root::Record::Connector { number, record: connector::Record::Outbox(value) } => {
                    if let Some(attempt) = value.attempt {
                        snapshot.receipts.insert(r::Receipt::Outbox(*number, value.key.into(), attempt.number));
                        snapshot
                            .attempts
                            .insert((*number, value.key.into()), (attempt.number, attempt.deadline.as_nanos()));
                    }
                    if value.phase == connector::EffectPhase::Held {
                        snapshot.held_effects.insert((*number, value.key.into()));
                    }
                    snapshot.decisions.push(r::Decision {
                        projection: value.key.attempt == 0 && value.key.completion == 0 && value.key.position == 1,
                        connector: *number,
                        key: value.key.into(),
                        kind: value.effect.kind,
                        resources: value.effect.resources.iter().map(|resource| path(*number, resource)).collect(),
                        condition: value.effect.condition,
                        target: value.effect.target,
                        judged_state: value.effect.state,
                        accepted_by: self.accepted.get(&(*number, value.key)).copied(),
                    });
                }
                root::Record::Connector { number, record: connector::Record::Made { key, .. } } => {
                    snapshot.settled_effects.insert((*number, (*key).into()));
                }
                root::Record::Core(_) | root::Record::Connector { .. } => {}
            }
        }
        for accepted in decisions {
            let key = root::Key::Core(core::Key::Tasks(tasks::Key::DecidedProposal(accepted.proposal)));
            if let Some(root::Record::Core(core::Record::Tasks(tasks::Stored::DecidedProposal(proposal)))) =
                store.rows.get(&key)
                && let tasks::PersonAction::Amend { task, .. } = proposal.action
            {
                authority_acceptances.push((
                    accepted.at,
                    task,
                    r::Acceptance { proposal: accepted.proposal, by: r::Source::Person(accepted.by) },
                ));
            }
            if let Some((first, last)) = accepted.created {
                for task in first..=last {
                    if snapshot.tasks.get(&task).is_some_and(|row| {
                        row.source
                            == match accepted.proposer {
                                people::Proposer::Task(task) => r::Source::Task(task),
                                people::Proposer::Party(person) => r::Source::Person(person),
                            }
                    }) {
                        snapshot.accepted_tasks.insert(
                            task,
                            r::Acceptance { proposal: accepted.proposal, by: r::Source::Person(accepted.by) },
                        );
                    }
                }
            }

            if let Some((connector, effect)) = self.proposals.get(&accepted.proposal) {
                for decision in &mut snapshot.decisions {
                    if decision.connector == *connector
                        && decision.kind == effect.kind
                        && decision.target == effect.target
                        && decision.key.task
                            == match accepted.proposer {
                                people::Proposer::Task(task) => task,
                                people::Proposer::Party(_) => 0,
                            }
                        && decision.key.purpose == effect.purpose
                    {
                        decision.accepted_by = Some(r::Source::Person(accepted.by));
                        snapshot.acceptances.insert(
                            (decision.connector, decision.key),
                            r::Acceptance { proposal: accepted.proposal, by: r::Source::Person(accepted.by) },
                        );
                        self.accepted.insert(
                            (
                                *connector,
                                connector::Key {
                                    deployment: decision.key.deployment,
                                    task: decision.key.task,
                                    attempt: decision.key.attempt,
                                    completion: decision.key.completion,
                                    position: decision.key.position,
                                    purpose: decision.key.purpose,
                                },
                            ),
                            r::Source::Person(accepted.by),
                        );
                    }
                }
            }
        }
        authority_acceptances.sort_by_key(|(at, ..)| *at);
        for (_, task, accepted) in authority_acceptances {
            snapshot.accepted_tasks.insert(task, accepted);
        }
        for (connector, task, attempt, completion, position, holder, proposal) in task_acceptances {
            for decision in &mut snapshot.decisions {
                if decision.connector == connector
                    && decision.key.task == task
                    && decision.key.attempt == attempt
                    && decision.key.completion == completion
                    && decision.key.position == position
                {
                    decision.accepted_by = Some(holder);
                    snapshot.acceptances.insert((connector, decision.key), r::Acceptance { proposal, by: holder });
                }
            }
        }
        for ((task, position), kept) in &self.activity {
            let key = root::Key::Core(core::Key::Tasks(tasks::Key::Activity { task: *task, position: *position }));
            assert!(
                matches!(store.rows.get(&key), Some(root::Record::Core(core::Record::Tasks(tasks::Stored::Activity(row)))) if row == kept),
                "immutable activity row disappeared"
            );
        }
        self.observe(now, r::Observed::Durable { number, snapshot });
        self.applied = number;
        let applied: Vec<_> = self.forgetting.keys().copied().filter(|commit| *commit <= number).collect();
        for number in applied {
            for task in self.forgetting.remove(&number).expect("submitted retention") {
                self.observe(now, r::Observed::Forgotten { task });
            }
        }
        for (task, message) in words {
            self.observe(now, r::Observed::Words { task, message });
        }
        for receipt in reached {
            self.observe(now, r::Observed::Reached(receipt));
        }
    }

    /// A released output and the neighbours' receipt of its words or result.
    pub fn delivered(&mut self, now: u64, delivery: &mut root::Delivery) {
        if let root::Delivery::Core(core::Held::PeopleReply { to, reply }) = &mut *delivery {
            for observed in self.intents.reply(skein_lib::Token::new(destination(to)), reply) {
                self.observe(now, observed);
            }
        }
        let mut requires = Vec::new();
        match delivery {
            root::Delivery::Assigned { assignment, .. } => {
                self.observe(
                    now,
                    r::Observed::Assigned {
                        task: assignment.task,
                        attempt: assignment.attempt,
                        conversation: assignment.conversation,
                        resumed: !assignment.transcript.is_empty(),
                        budget: assignment.run.budget,
                        settings: assignment.run.revision,
                    },
                );
                for word in &assignment.messages {
                    self.observe(now, r::Observed::Reached(r::Receipt::Word(assignment.task, word.number)));
                }
            }
            root::Delivery::Core(core::Held::AcknowledgeTurn { run, attempt, turn, .. }) => {
                let receipt = if self.crossed.contains(&(run.raw(), attempt.raw())) {
                    r::Receipt::Claim(run.raw(), attempt.raw())
                } else {
                    r::Receipt::Turn(run.raw(), attempt.raw(), *turn)
                };
                requires.push(receipt);
            }
            root::Delivery::Core(core::Held::Acknowledge { run, attempt, .. }) => {
                let receipt = if self.crossed.contains(&(run.raw(), attempt.raw())) {
                    r::Receipt::Claim(run.raw(), attempt.raw())
                } else {
                    r::Receipt::Terminal(run.raw(), attempt.raw())
                };
                requires.push(receipt);
            }
            root::Delivery::Core(core::Held::CallAnswer { key, .. }) => {
                requires.push(call(*key));
            }
            root::Delivery::CallAnswer { task, attempt, call: settled, writes, .. } => {
                // Serial zero is a no-record answer; a settled body must be durable.
                assert!(
                    !*writes || settled.serial == 0 || self.settled_calls.contains(&(*task, *attempt, settled.serial)),
                    "answer preceded its durable named-call record"
                );
            }
            root::Delivery::Core(core::Held::PeopleReply { to, reply }) => {
                self.reply_receipts(to, reply, &mut requires);
            }
            root::Delivery::Message { task: run, word, .. } => {
                requires.push(r::Receipt::Word(*run, word.number));
                self.observe(now, r::Observed::Reached(r::Receipt::Word(*run, word.number)));
            }
            root::Delivery::Core(core::Held::Result { person, task, .. }) => {
                requires.push(r::Receipt::Ended(*task));
                self.observe(now, r::Observed::Result { task: *task, to: r::Source::Person(*person) });
            }
            root::Delivery::System { connector, call: connector::SystemRequest::Apply { key, attempt, .. } } => {
                requires.push(r::Receipt::Outbox(*connector, (*key).into(), *attempt));
            }
            root::Delivery::Procedure { task, .. } => {
                requires.push(r::Receipt::Task(*task));
            }
            root::Delivery::LostRead { task, attempt, .. } => {
                requires.push(r::Receipt::Terminal(*task, *attempt));
            }
            root::Delivery::Application { to, number, outcome } => {
                self.observe(
                    now,
                    r::Observed::ApplicationAnswered {
                        reply: destination(to),
                        connector: *number,
                        outcome: crate::application_observations::encoded(outcome),
                    },
                );
            }
            root::Delivery::Restart(_)
            | root::Delivery::Core(_)
            | root::Delivery::Fleet(_)
            | root::Delivery::ApplicationDocument { .. }
            | root::Delivery::ApplicationRead { .. }
            | root::Delivery::Projection { .. }
            | root::Delivery::Link { .. }
            | root::Delivery::ProjectMade { .. }
            | root::Delivery::CallBusy { .. }
            | root::Delivery::System { .. } => {}
        }
        self.observe(now, r::Observed::Released { requires });
    }

    fn reply_receipts(&mut self, to: &mut skein_lib::ReplyTo, reply: &people::Reply, requires: &mut Vec<r::Receipt>) {
        match reply {
            people::Reply::SignedIn { sign_in, .. } => requires.push(r::Receipt::SignIn(*sign_in)),
            people::Reply::Signer(_)
            | people::Reply::Full { .. }
            | people::Reply::Again(_)
            | people::Reply::SignedOut
            | people::Reply::Refused(_) => {
                let to = destination(to);
                self.requests.remove(&to);
            }
            people::Reply::Outcome(_) => {
                let to = destination(to);
                if let Some((person, key, seen)) = self.requests.remove(&to) {
                    if let Some(fence) = seen {
                        assert!(self.applied >= fence.after, "Seen reply preceded its durable position/forget commit");
                    } else {
                        requires.push(r::Receipt::Party(person, key));
                    }
                }
            }
        }
    }
}

pub(crate) fn destination(to: &mut skein_lib::ReplyTo) -> u64 {
    let token = std::mem::replace(to, skein_lib::ReplyTo::new(skein_lib::Token::new(0))).into_token();
    *to = skein_lib::ReplyTo::new(token);
    token.raw()
}

fn person_pattern(value: &people::Pattern) -> r::Pattern {
    match &value.last {
        people::Last::Exact(last) => pattern(&value.segments, last, false),
        people::Last::Open(last) => pattern(&value.segments, last, true),
    }
}

fn person_scope(value: &people::Authority) -> r::Scope {
    r::Scope {
        budget: value.spend,
        deadline: value.deadline.map(skein_lib::Wall::as_nanos),
        tools: value.tools,
        notes: value.notes,
        grants: value
            .grants
            .iter()
            .map(|grant| r::Grant {
                connector: grant.connector,
                kind: grant.kind,
                pattern: person_pattern(&grant.pattern),
            })
            .collect(),
        executors: value
            .delegation
            .kinds
            .iter()
            .map(|kind| match kind {
                people::Executor::Charter(n) => (0, *n),
                people::Executor::Procedure(n) => (1, *n),
                people::Executor::Role(n) => (2, *n),
            })
            .collect(),
        descendants: value.delegation.tasks,
        depth: value.delegation.depth,
        note_resources: value
            .note_resources
            .iter()
            .map(|resource| (resource.connector, person_pattern(&resource.pattern)))
            .collect(),
    }
}

/// Conservative heap price for this testing root's bounded journal and routes.
/// A handed row is priced as a complete child bound, so nested owned payloads
/// fit without teaching the generic referee the root's record vocabulary.
#[must_use]
pub fn root_bound(limits: &root::Limits) -> Option<u64> {
    root::worst_case(limits)
}

/// A conservative heap observation at each testing-root iteration. Test peers'
/// allocations after construction are counted too; passing this check therefore
/// bounds the engine even while those peers retain their independent ledgers.
/// The engine's own tighter step peaks are measured separately in memory tests.
pub struct Heap {
    meter: skein_world::domain::heap::Meter,
    maximum: u64,
    configuration: u64,
}

impl std::fmt::Debug for Heap {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Heap").field("maximum", &self.maximum).finish_non_exhaustive()
    }
}

impl Heap {
    /// Start before root construction, after the scenario built its configuration.
    #[must_use]
    pub fn new(limits: &root::Limits) -> Self {
        Self {
            meter: skein_world::domain::heap::Meter::new(),
            maximum: root_bound(limits).expect("testing root bound fits"),
            // Pre-existing configuration allocations transferred into the root
            // are covered by a complete bound for both of their owning children.
            configuration: core::worst_case(&limits.core).expect("core bound fits")
                + connector::worst_case(&limits.connector).expect("connector bound fits") * 2,
        }
    }

    /// Heap observation, including pre-existing configuration ownership.
    #[must_use]
    pub fn observed(&self) -> r::Observed {
        r::Observed::Heap {
            held: self.meter.held().checked_add(self.configuration).expect("world heap fits"),
            maximum: self.maximum,
        }
    }
}

#[cfg(test)]
mod activity_tests {
    use super::*;

    #[test]
    fn activity_referee_rejects_changed_or_erased_lines() {
        for erased in [false, true] {
            let configuration = crate::effects::fixture(507, false).0;
            let mut observer = Observer::new(&configuration);
            let mut store = Store::new(&crate::world::limits(), &configuration.core.settings.models);
            let row = root::Record::Core(core::Record::Tasks(tasks::Stored::Activity(tasks::ActivityLine {
                task: 1,
                position: 1,
                at: skein_lib::Wall::EPOCH,
                line: tasks::Line::Milestone { position: 1 },
            })));
            let key = row.key();
            store.rows.insert(key.clone(), row);
            store.applied = 1;
            observer.durable(0, &store);
            if erased {
                store.rows.remove(&key);
            } else {
                store.rows.insert(
                    key,
                    root::Record::Core(core::Record::Tasks(tasks::Stored::Activity(tasks::ActivityLine {
                        task: 1,
                        position: 1,
                        at: skein_lib::Wall::EPOCH,
                        line: tasks::Line::Milestone { position: 2 },
                    }))),
                );
            }
            store.applied = 2;
            let rejected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| observer.durable(1, &store)));
            assert!(rejected.is_err(), "referee must reject an immutable row changing or disappearing");
        }
    }
}
