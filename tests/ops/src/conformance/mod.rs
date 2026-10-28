//! Ops's two proof-of-concept stories on jig's independent conformance loop.
mod application;
mod config;
pub use config::chat;
#[cfg(test)]
mod creating;
mod documents;
mod observations;
mod protocol;
mod scripts;
pub use scripts::agent_bounds;
mod systems;

use jig_conformance::referee as r;
use jig_conformance::{Application, Clock, Input, Output};
use jig_domain_accounts as accounts;
use jig_domain_core as core;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_ops_domain as root;
use jig_ops_domain_infrastructure as infra;
use jig_ops_domain_observability as obs;
use jig_ops_fake_production as production;
use observations::Store;
use skein_lib::{Duration, Env, List, Queue, ReplyTo, Time, Token, Wall};
pub use systems::Systems;

/// Only these two stories belong to this proof of concept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Story {
    Alert,
    Night,
    Environment,
}

/// The same seed and story reconstruct every cold domain.
#[derive(Clone, Copy, Debug)]
pub struct Config {
    pub seed: u64,
    pub story: Story,
    limits: root::Limits,
    documents: bool,
    client_backed: bool,
    close_grant: bool,
}

impl Config {
    /// Fix the finite limits once; cold starts still rebuild the same root.
    #[must_use]
    pub fn new(seed: u64, story: Story) -> Self {
        Self {
            seed,
            story,
            limits: config::root_for(seed, story == Story::Environment).1,
            documents: false,
            client_backed: false,
            close_grant: true,
        }
    }

    /// Omit the deployment close grant while preserving the same story.
    #[must_use]
    pub fn without_close_grant(mut self) -> Self {
        self.close_grant = false;
        self
    }

    /// Replace only the on-call person with clients; retain scripted hosts and systems.
    #[must_use]
    pub fn client_backed(mut self) -> Self {
        self.client_backed = true;
        // A public task page watches its frame, task, plan, chips and live turn,
        // with application parts beside them. The inline person watched none.
        self.limits.core.views.watchers = 32;
        self.limits.core.views.watchers_per_party = 32;
        self.limits.core.views.runs = 8;
        // A closing plan can require final reads for its shown cards and header
        // together. Match this client's declared simultaneous read capacity.
        self.limits.core.load_slots = 32;
        self
    }

    /// The joint client story's finite recovery profile. Its actual owner
    /// deadline remains committed before every attempt (connectors.md, 4.3).
    #[must_use]
    pub fn client_recovery(mut self) -> Self {
        self.limits.infrastructure.retry_after_seconds = 1;
        self.limits.core.fleet.grace = Duration::from_secs(1);
        self
    }

    /// The exact finite engine bounds used by this fixture.
    #[must_use]
    pub fn limits(&self) -> &root::Limits {
        &self.limits
    }

    /// Bound the retained connector answers for an application-request story.
    #[must_use]
    pub fn application_answers(mut self, count: u32) -> Self {
        self.limits.infrastructure.proposals = count;
        self
    }
}

/// Outside party state and evidence survive domain crashes.
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent account, sign-in, application-request and document peers progress separately"
)]
pub struct Peers {
    pub(crate) policy: r::Policy,
    applied: u64,
    observed: Vec<r::Observed>,
    application_observed: Vec<r::Observed>,
    intents: jig_core_world::observations::intents::Intents,
    application_requests: bool,
    pub sign_in: Option<u64>,
    account_ready: bool,
    asked_sign_in: bool,
    role_widened: bool,
    pub results: Vec<u64>,
    pub read_results: Vec<Token>,
    delivered_results: std::collections::BTreeSet<u64>,
    pending_results: Vec<(u64, u64, u64)>,
    pub assignments: Vec<(u64, u64)>,
    pub entries: Vec<(u64, u16)>,
    pub accepted: Vec<u64>,
    pub replies: u64,
    pub application_answers: Vec<infra::ApplicationOutcome>,
    pub close_answers: Vec<people::Outcome>,
    pub documents: Vec<(u64, root::ClientDocument)>,
    pub ended_documents: Vec<u64>,
    pub lead_commits: Vec<(u64, obs::Incident, obs::Delegation)>,
    document_story: bool,
    client_documents: bool,
    watched_documents: std::collections::BTreeSet<u64>,
    document_rows: Vec<(u64, root::Record)>,
    pending_document_effects: Vec<infra::SystemRequest>,
    agents: scripts::Agents,
    protocol: crate::protocol::Stand,
    pub call_answers: Vec<(bool, Box<[u8]>)>,
    asked_accept: std::collections::BTreeSet<(u64, u64)>,
}

/// Released root outputs and receipts observed in a whole committed transaction.
#[derive(Debug)]
pub enum Delivery {
    Root(Box<root::Output>),
    Results { commit: u64, rows: Vec<(u64, u64)> },
}

// The most held of this world's stories whose binary counts the heap,
// the ordinary ops adapter, focused and fuzzy suites: 14,778,622 bytes at jig c5bd3bba7,
// reached by application_documents::a_remediations_panel_says_restarting_then_waiting_for_healthy
// and application_documents::an_incidents_subject_follows_an_alert_to_its_resolution.
// A ceiling, a quarter above, rounded up to two significant digits
// (skein's dynamic-memory draft, 7.1); it moves by hand only in the commit
// of a real change that passes it, never on a type's size.
const OPS_HEAP_CEILING: u64 = 19_000_000;

/// The real ops root and its independent production; no surrogate engine.
pub struct Ops;

fn env(config: &Config, clock: Clock) -> Env<root::Limits> {
    Env { now: Time::from_nanos(clock.now), wall: Wall::from_nanos(clock.now), limits: config.limits }
}

impl Application for Ops {
    type Domain = root::Domain;
    type Store = jig_fake_store::Store<root::Key, root::Record>;
    type Write = jig_fake_store::Write<root::Key, root::Record>;
    type Config = Config;
    type Systems = Systems;
    type Peers = Peers;
    type Key = root::Key;
    type Record = root::Record;
    type Event = root::Event;
    type Delivery = Delivery;

    fn store(_: &Config) -> Self::Store {
        jig_fake_store::Store::new()
    }
    fn build(config: &Config) -> root::Domain {
        let mut configuration = config::root_for(config.seed, config.story == Story::Environment).0;
        if !config.close_grant {
            configuration.core.grants = Box::new([]);
        }
        root::Domain::new(configuration, &config.limits).expect("fixture core configuration")
    }
    fn systems(_: &Config, _: u64) -> Systems {
        let mut production = production::Production::new(0, production::Backend::Fake);
        production.add_service(production::ServiceName::new("production", "checkout"), "v2", 3);
        production.add_service(production::ServiceName::new("staging", "checkout"), "v2", 3);
        production.set_pool_quota("staging", 5);
        Systems { production, now: 0 }
    }
    fn peers(config: &Config, _: u64) -> Peers {
        Peers {
            policy: observations::policy(config.seed, config.story == Story::Environment),
            applied: 0,
            observed: Vec::new(),
            application_observed: Vec::new(),
            intents: jig_core_world::observations::intents::Intents::default(),
            application_requests: false,
            sign_in: None,
            account_ready: false,
            asked_sign_in: config.client_backed,
            role_widened: false,
            results: Vec::new(),
            read_results: Vec::new(),
            delivered_results: std::collections::BTreeSet::new(),
            pending_results: Vec::new(),
            assignments: Vec::new(),
            entries: Vec::new(),
            accepted: Vec::new(),
            replies: 0,
            application_answers: Vec::new(),
            close_answers: Vec::new(),
            documents: Vec::new(),
            ended_documents: Vec::new(),
            document_story: config.documents,
            client_documents: config.client_backed,
            watched_documents: std::collections::BTreeSet::new(),
            document_rows: Vec::new(),
            lead_commits: Vec::new(),
            pending_document_effects: Vec::new(),
            agents: {
                let run = config::root_for(config.seed, config.story == Story::Environment).0.core.defaults.charters[0]
                    .policy
                    .clone();
                scripts::Agents::new(
                    config.client_backed,
                    config.story == Story::Environment,
                    run.waiting,
                    run.call_timeout,
                )
            },
            protocol: crate::protocol::Stand::new(config.limits.core),
            call_answers: Vec::new(),
            asked_accept: std::collections::BTreeSet::new(),
        }
    }
    fn policy(peers: &Peers) -> &r::Policy {
        &peers.policy
    }
    fn policy_generation(_: &Peers) -> u64 {
        0
    }
    fn recovery(_: &Config, connector: u16, kind: u16) -> r::Recovery {
        match (connector, kind) {
            (2, 1 | 4) => r::Recovery::Keyed,
            (2, 2 | 5) => r::Recovery::Conditional,
            _ => panic!("declared story kinds"),
        }
    }
    fn start(config: &Config, store: &mut Store) -> Vec<root::Event> {
        if store.rows.is_empty() {
            let record =
                root::Record::Core(core::Record::People(people::Stored::Roles { project: 1, holdings: Box::new([]) }));
            store.rows.insert(record.key(), record);
            let configuration = config::root_for(config.seed, config.story == Story::Environment).0;
            for row in jig_core_world::fixtures::account_rows(&configuration.core.settings.llm_providers) {
                let jig_test_domain::Record::Core(row) = row else { panic!("account fixture row") };
                let row = root::Record::Core(row);
                store.rows.insert(row.key(), row);
            }
        }
        vec![root::Event::Restart { pending: false }]
    }
    fn cold(peers: &mut Peers, store: &Store, _: Clock) {
        peers.observed.push(r::Observed::ColdStore { receipts: observations::snapshot(store).receipts });
        peers.account_ready = false;
        peers.asked_accept.clear();
        peers.role_widened = false;
        peers.agents.clear();
        peers.protocol.clear();
    }
    fn timers(_: &Config) -> Vec<root::Event> {
        vec![
            root::Event::Timer(core::Timer::Tasks),
            root::Event::Timer(core::Timer::People),
            root::Event::Timer(core::Timer::Fleet),
            root::Event::Timer(core::Timer::Account),
            root::Event::Timer(core::Timer::Brief),
            root::Event::ObservabilityTimer,
            root::Event::InfrastructureTimer,
            root::Event::TurnTimer,
        ]
    }
    fn step(domain: &mut root::Domain, config: &Config, clock: Clock, input: Input<root::Event, root::Record>) {
        let event = match input {
            Input::Event(event) => event,
            Input::Committed(number) => root::Event::Store(root::Store::Committed { number, fill: 0 }),
            Input::Failed(number) => root::Event::Store(root::Store::Failed { number }),
            Input::Restore(row) => root::Event::Store(root::Store::Restore(row)),
        };
        root::step(domain, &env(config, clock), event);
    }
    fn release(
        domain: &mut root::Domain,
        config: &Config,
        clock: Clock,
    ) -> Vec<Output<jig_fake_store::Write<root::Key, root::Record>, Delivery>> {
        let mut queue = Queue::with_capacity(256);
        root::release(domain, &env(config, clock), &mut queue);
        let mut outputs = Vec::new();
        while let Some(output) = queue.pop() {
            match output {
                root::Output::Commit { number, mut writes } => {
                    let mut rows = Vec::new();
                    let mut results = Vec::new();
                    while let Some(write) = writes.pop() {
                        rows.push(match write {
                            root::Write::Wires => continue,
                            root::Write::Save(row) => {
                                if let root::Record::Core(core::Record::Tasks(tasks::Stored::Live(parent))) = &row {
                                    for word in &parent.inbox {
                                        if let (tasks::MessageKind::Result(_), tasks::Party::Task(child)) =
                                            (&word.kind, word.from)
                                        {
                                            results.push((child, parent.number));
                                        }
                                    }
                                }
                                jig_fake_store::Write::Save { key: row.key(), row }
                            }
                            root::Write::CopySettingsRevision(_) => {
                                unreachable!("this scripted ops world restarts with identical defaults")
                            }
                            root::Write::Erase(key) => jig_fake_store::Write::Erase(key),
                        });
                    }
                    outputs.push(Output::Commit { number, writes: rows.into_boxed_slice() });
                    if !results.is_empty() {
                        outputs.push(Output::Deliver(Delivery::Results { commit: number, rows: results }));
                    }
                }
                root::Output::Stop => outputs.push(Output::Stop),
                other @ (root::Output::ApplicationDocument { .. }
                | root::Output::ApplicationRead { .. }
                | root::Output::IncidentAssociationRefused { .. }
                | root::Output::Application { .. }
                | root::Output::Switch
                | root::Output::Load { .. }
                | root::Output::ReadResult { .. }
                | root::Output::CoreLoad(_)
                | root::Output::Core(_)
                | root::Output::Now(_)
                | root::Output::Observability(_)
                | root::Output::Infrastructure(_)
                | root::Output::Read { .. }
                | root::Output::Agent(_)
                | root::Output::ToChild(_)
                | root::Output::ProjectMade { .. }
                | root::Output::Busy) => outputs.push(Output::Deliver(Delivery::Root(Box::new(other)))),
            }
        }
        outputs
    }
    fn inbound(peers: &mut Peers, store: &Store, _: Clock, event: &mut root::Event) {
        peers.application_observed.extend(observations::inbound(&mut peers.intents, store, event));
        peers.agents.stepped(event);
        if let Some(observed) = application::requested(store, event) {
            peers.application_requests = true;
            peers.application_observed.push(observed);
        }
    }

    #[expect(clippy::too_many_lines, reason = "each outside output has an exhaustive adapter")]
    fn deliver(
        peers: &mut Peers,
        systems: &mut Systems,
        store: &mut Store,
        clock: Clock,
        delivery: Delivery,
    ) -> Vec<Input<root::Event, root::Record>> {
        let delivery = match delivery {
            Delivery::Root(root) => *root,
            Delivery::Results { commit, rows } => {
                for (task, parent) in rows {
                    peers.pending_results.push((commit, task, parent));
                }
                return Vec::new();
            }
        };
        match delivery {
            root::Output::Application { to, outcome } => {
                peers.application_observed.push(r::Observed::ApplicationAnswered {
                    reply: to.into_token().raw(),
                    connector: 2,
                    outcome: application::encoded(&outcome),
                });
                peers.application_answers.push(outcome);
                Vec::new()
            }
            root::Output::ToChild(child) => {
                if let root::Released::Host(jig_domain_host::Event::Down(jig_domain_host::LinkDown::Assign {
                    assignment,
                    ..
                })) = &child
                {
                    let task = assignment.assignment.run.raw();
                    let attempt = assignment.assignment.attempt.raw();
                    let budget = observations::snapshot(store).tasks[&task].run_reserved;
                    peers.assignments.push((task, attempt));
                    let proof = store
                        .rows
                        .values()
                        .find_map(|record| match record {
                            root::Record::Core(core::Record::Core(core::CoreRecord::RunProof(proof)))
                                if proof.task == task =>
                            {
                                Some(proof)
                            }
                            root::Record::Core(_)
                            | root::Record::Observability(_)
                            | root::Record::Infrastructure(_) => None,
                        })
                        .expect("durable assignment proof");
                    peers.observed.push(r::Observed::Assigned {
                        task,
                        attempt,
                        budget,
                        conversation: proof.conversation,
                        resumed: !assignment.turns.is_empty(),
                        // These scenarios use the first-start settings and draw no changes.
                        settings: 1,
                    });
                }
                vec![Input::Event(root::Event::Released(child))]
            }
            root::Output::Switch => vec![Input::Event(root::Event::Store(root::Store::Switched))],
            root::Output::Load { step, range } => {
                let position = match step {
                    core::RestartStep::LoadCore | core::RestartStep::LoadInbox => 0,
                    core::RestartStep::RestoreConnector { connector } => u32::from(connector),
                    core::RestartStep::Switch
                    | core::RestartStep::AdoptRuns
                    | core::RestartStep::ReadAfresh { .. }
                    | core::RestartStep::SettleOutbox { .. }
                    | core::RestartStep::Open => panic!("only row loads go to the store"),
                };
                if step != core::RestartStep::LoadInbox {
                    peers.observed.push(r::Observed::RestartStage { position });
                }
                let rows: Vec<_> = if range == root::Range::Core {
                    core::RestoreRange::LIVE
                        .iter()
                        .flat_map(|range| {
                            store.rows.iter().filter_map(move |(key, row)| match key {
                                root::Key::Core(key) if range.admits(key) => Some(row.clone()),
                                root::Key::Core(_) | root::Key::Observability(_) | root::Key::Infrastructure(_) => None,
                            })
                        })
                        .collect()
                } else {
                    store
                        .rows
                        .iter()
                        .filter_map(|(key, row)| {
                            let admitted = match (&range, key) {
                                (root::Range::Census(census), root::Key::Core(key)) => census.admits(key),
                                (root::Range::Observability, root::Key::Observability(_))
                                | (root::Range::Infrastructure, root::Key::Infrastructure(_)) => true,
                                _ => false,
                            };
                            admitted.then(|| row.clone())
                        })
                        .collect()
                };
                let rows = if matches!(range, root::Range::Census(_)) {
                    let mut rows = rows;
                    if let root::Range::Census(census) = &range {
                        rows.sort_by_key(|row| match row.key() {
                            root::Key::Core(key) => census.interval(&key),
                            root::Key::Observability(_) | root::Key::Infrastructure(_) => None,
                        });
                    }
                    rows.into_iter().flat_map(|row| census_owner(store, &row)).collect()
                } else {
                    rows
                };
                let mut events: Vec<_> = rows.into_iter().map(Input::Restore).collect();
                events.push(Input::Event(root::Event::Store(root::Store::Restored(step))));
                events
            }
            root::Output::Infrastructure(call) => {
                if peers.document_story
                    && let infra::SystemRequest::Apply { key, .. } = &call
                    && !documents::restarting(peers, key.task)
                {
                    // The external system may answer slowly. Let the client
                    // acknowledge the committed restarting panel first.
                    peers.pending_document_effects.push(call);
                    return Vec::new();
                }
                if let infra::SystemRequest::Apply { key, attempt, effect } = &call {
                    let attempt = *attempt;
                    let stable = observations::key(*key);
                    let (kind, resources, condition, target) = observations::parts(effect);
                    peers
                        .observed
                        .push(r::Observed::Released { requires: vec![r::Receipt::Outbox(2, stable, attempt)] });
                    let before = match effect {
                        infra::Effect::Restart { operation, .. } => {
                            vec![jig_test_system::ValueObserved {
                                state: systems.production.find_restart(*operation).map(|_| *operation),
                                owner: None,
                            }]
                        }
                        infra::Effect::Scale { service, .. } => {
                            let name = production::ServiceName::new(
                                std::str::from_utf8(&service.environment).expect("fixture bytes"),
                                std::str::from_utf8(&service.name).expect("fixture bytes"),
                            );
                            vec![jig_test_system::ValueObserved {
                                state: Some(u64::from(
                                    systems.production.read_service(&name).expect("fixture service").replicas,
                                )),
                                owner: None,
                            }]
                        }
                        infra::Effect::CreateEnvironment { environment, .. } => {
                            let pool = std::str::from_utf8(&environment.pool).expect("fixture pool");
                            let name = production::EnvironmentName::new(
                                pool,
                                std::str::from_utf8(&environment.name).expect("fixture environment"),
                            );
                            let before = systems.production.read_environment(&name);
                            peers.observed.push(r::Observed::Read {
                                resource: r::Name {
                                    connector: 2,
                                    path: infra::Resource::Pool(infra::Pool(environment.pool.clone()))
                                        .segments()
                                        .iter()
                                        .map(|v| v.to_vec())
                                        .collect(),
                                },
                                state: Some(u64::from(systems.production.used_slots(pool))),
                                at: clock.now,
                            });
                            vec![jig_test_system::ValueObserved {
                                state: before.as_ref().map(|row| row.until),
                                owner: before.map(|row| {
                                    let (attempt, completion, position) = match row.key.origin {
                                        production::Purpose::Call { attempt, completion, position } => {
                                            (attempt, completion, position)
                                        }
                                        production::Purpose::Procedure { .. } => (0, 0, 0),
                                        production::Purpose::Projection { .. } => (0, 0, 1),
                                    };
                                    jig_test_system::SystemKey {
                                        deployment: row.key.deployment,
                                        task: row.key.task,
                                        purpose: row.key.purpose,
                                        attempt,
                                        completion,
                                        position,
                                    }
                                }),
                            }]
                        }
                        infra::Effect::TearDown { environment, .. } => {
                            let name = production::EnvironmentName::new(
                                std::str::from_utf8(&environment.pool).expect("fixture pool"),
                                std::str::from_utf8(&environment.name).expect("fixture environment"),
                            );
                            vec![jig_test_system::ValueObserved {
                                state: systems.production.read_environment(&name).map(|_| 1),
                                owner: None,
                            }]
                        }
                        infra::Effect::Rollback { .. } => panic!("unused story effect"),
                    };
                    let previous = systems.production.observed().len();
                    let event = systems.infrastructure(call);
                    let applied = match &systems.production.observed()[previous] {
                        production::ObservedEffect::Restart { applied, .. }
                        | production::ObservedEffect::Scale { applied, .. }
                        | production::ObservedEffect::Create { applied, .. }
                        | production::ObservedEffect::TearDown { applied, .. } => *applied,
                        production::ObservedEffect::Silence { .. } | production::ObservedEffect::Rollback { .. } => {
                            panic!("unused story effect")
                        }
                    };
                    peers.observed.push(r::Observed::Effect {
                        connector: 2,
                        effect: jig_test_system::Observed {
                            kind,
                            key: stable,
                            resources: resources
                                .into_iter()
                                .map(|resource| jig_test_system::Name(resource.path))
                                .collect(),
                            condition,
                            target,
                            before,
                            applied,
                            copy: if attempt == 1 {
                                jig_test_system::Copy::First
                            } else {
                                jig_test_system::Copy::Retried
                            },
                        },
                    });
                    return vec![Input::Event(root::Event::Infrastructure(infra::Event::System(event)))];
                }
                vec![Input::Event(root::Event::Infrastructure(infra::Event::System(systems.infrastructure(call))))]
            }
            root::Output::Observability(call) => {
                let event = systems.observability(call);
                observations::fact(&mut peers.observed, &event);
                vec![Input::Event(root::Event::Observability(obs::Event::System(event)))]
            }
            root::Output::Now(core::Now::Call { to, run, attempt, call }) => {
                vec![Input::Event(peers.protocol.decode(to, run, attempt, &call, Wall::from_nanos(clock.now)))]
            }
            root::Output::Core(core::Held::CallAnswer { to, key, part }) => {
                vec![Input::Event(peers.protocol.answer(to, key, &part, true))]
            }
            root::Output::Now(core::Now::SettingsAnswer { to, key, answer }) => {
                peers.protocol.settings_answer(to, key, &answer).into_iter().map(Input::Event).collect()
            }
            root::Output::Now(core::Now::EffectAnswer { to, key, part }) => {
                vec![Input::Event(peers.protocol.answer(to, key, &part, false))]
            }
            root::Output::Now(core::Now::NoteBusy { to, key }) => vec![Input::Event(peers.protocol.busy(to, key))],
            root::Output::Now(core::Now::SettledCallRefused { to, key, name: _, tool }) => {
                vec![Input::Event(peers.protocol.refused(to, key, tool))]
            }
            root::Output::ReadResult { payload, outcome } => {
                peers.read_results.push(payload);
                vec![Input::Event(root::Event::ResultRead { payload, end: crate::protocol::result(&outcome) })]
            }
            root::Output::Now(core::Now::Account(accounts::Request::Component(accounts::ToComponent::Want {
                account,
            }))) => {
                vec![Input::Event(root::Event::Core(core::Event::Account(accounts::Event::Component(
                    accounts::FromComponent::Granted {
                        form: accounts::Form::Credential(accounts::Scheme::Bearer),
                        account,
                        generation: 1,
                        valid: Duration::from_secs(7200),
                    },
                ))))]
            }
            root::Output::Now(core::Now::Account(accounts::Request::Component(accounts::ToComponent::Close))) => {
                vec![Input::Event(root::Event::Core(core::Event::Account(accounts::Event::Component(
                    accounts::FromComponent::Closed,
                ))))]
            }
            root::Output::Core(core::Held::PeopleReply { mut to, reply }) => {
                let token = std::mem::replace(&mut to, ReplyTo::new(Token::new(0))).into_token();
                peers.application_observed.extend(peers.intents.reply(token, &reply));
                peers.replies += 1;
                match reply {
                    people::Reply::SignedIn { sign_in: number, .. } => {
                        peers.sign_in = Some(number);
                        peers.observed.push(r::Observed::Released { requires: vec![r::Receipt::SignIn(number)] });
                    }
                    people::Reply::Outcome(_) => {}
                    other @ (people::Reply::Signer(_)
                    | people::Reply::Full { .. }
                    | people::Reply::SignedOut
                    | people::Reply::Refused(_)
                    | people::Reply::Again(_)) => {
                        panic!("unexpected party reply: {other:?}")
                    }
                }
                Vec::new()
            }
            root::Output::Core(core::Held::Result { task, person, .. }) => {
                peers.results.push(task);
                peers.observed.push(r::Observed::Result { task, to: r::Source::Person(person) });
                peers.observed.push(r::Observed::Released { requires: vec![r::Receipt::Ended(task)] });
                Vec::new()
            }
            root::Output::Core(core::Held::NotesLoad { owner, .. }) => {
                vec![Input::Event(root::Event::Core(core::Event::Notes(jig_domain_notes::Event::Loaded {
                    owner,
                    rows: jig_domain_notes::Rows { records: List::with_capacity(0) },
                    more: false,
                })))]
            }
            root::Output::CoreLoad(core::Now::StartPreparation { task, from, .. }) => {
                let mut rows: Vec<_> = store
                    .rows
                    .values()
                    .filter_map(|record| match record {
                        root::Record::Core(core::Record::Core(core::CoreRecord::Turn(row)))
                            if row.task == task && from.is_some_and(|start| (row.attempt, row.turn) >= start) =>
                        {
                            Some(row.clone())
                        }
                        root::Record::Core(_) | root::Record::Infrastructure(_) | root::Record::Observability(_) => {
                            None
                        }
                    })
                    .collect();
                rows.sort_by_key(|row| (row.attempt, row.turn));
                if let Some(start) = from
                    && !rows.first().is_some_and(|row| (row.attempt, row.turn) == start && row.opens_window)
                {
                    return vec![Input::Event(root::Event::Core(core::Event::TranscriptUnread { task }))];
                }
                vec![Input::Event(root::Event::Store(root::Store::Transcript {
                    task,
                    rows: rows
                        .into_iter()
                        .map(|row| {
                            let limits = jig_protocol_transcript::Limits::new(
                                u32::try_from(row.transcript.len()).expect("stored turn bound"),
                                jig_client_domain::contract::TurnLimits {
                                    messages: u32::MAX,
                                    blocks: u32::MAX,
                                    text: u32::try_from(row.transcript.len()).expect("stored turn bound"),
                                },
                            );
                            let text = jig_protocol_transcript::brief_text(&limits, &row.transcript).ok();
                            (row, text)
                        })
                        .collect(),
                    done: true,
                }))]
            }
            root::Output::ApplicationDocument { watcher, document } => {
                documents::delivered(peers, store, document);
                vec![Input::Event(root::Event::Core(core::Event::Delivered { watcher }))]
            }
            root::Output::Core(
                core::Held::ApplicationWatchEnded { watcher, .. } | core::Held::WatchEnded { watcher, .. },
            ) => {
                peers.ended_documents.push(watcher.raw());
                Vec::new()
            }
            root::Output::Core(core::Held::Acted { outcome, .. }) => {
                peers.close_answers.push(outcome);
                Vec::new()
            }
            root::Output::IncidentAssociationRefused { .. } => panic!("owner association refused in conformance story"),
            root::Output::ApplicationRead { .. }
            | root::Output::Read { .. }
            | root::Output::Core(_)
            | root::Output::Now(_)
            | root::Output::CoreLoad(_)
            | root::Output::ProjectMade { .. }
            | root::Output::Busy => Vec::new(),
            root::Output::Agent(request) => protocol::agent(peers, request),
            root::Output::Commit { .. } | root::Output::Stop => unreachable!("harness consumed commit/stop"),
        }
    }
    fn poll(
        peers: &mut Peers,
        systems: &mut Systems,
        store: &mut Store,
        clock: Clock,
        ready: bool,
    ) -> Vec<Input<root::Event, root::Record>> {
        let now = clock.now / 1_000_000_000;
        if now > systems.now {
            systems.production.advance(now - systems.now);
            systems.now = now;
        }
        let mut events = Vec::new();
        if ready && !peers.account_ready {
            peers.account_ready = true;
            if !store.rows.contains_key(&root::Key::Core(core::Key::Tasks(tasks::Key::Ledger(tasks::Funder::Period {
                project: 1,
                period: 1,
            })))) {
                events.push(Input::Event(root::Event::Core(core::Event::Period {
                    project: 1,
                    period: 1,
                    budget: 1000,
                    next: None,
                })));
            }
            events.push(Input::Event(root::Event::Core(core::Event::Account(accounts::Event::Component(
                accounts::FromComponent::Granted {
                    form: accounts::Form::Credential(accounts::Scheme::Bearer),
                    account: 1,
                    generation: 1,
                    valid: Duration::from_secs(7200),
                },
            )))));
        }
        if ready && !peers.asked_sign_in {
            peers.asked_sign_in = true;
            events.push(Input::Event(root::Event::Core(core::Event::SignIn {
                held: None,
                reply_to: ReplyTo::new(Token::new(100)),
                identity: people::Identity {
                    key: people::IdentityKey { provider: 0, subject: 7_u64.to_be_bytes().into() },
                    login: b"oncall".as_slice().into(),
                    name: Box::new([]),
                },
            })));
        }
        if ready {
            events.extend(funding::poll(peers, store));
        }
        if ready && let Some(sign_in) = peers.sign_in {
            for record in store.rows.values() {
                if let root::Record::Core(core::Record::Tasks(tasks::Stored::Live(row))) = record
                    && let Some(proposal) = &row.proposal
                    && matches!(
                        proposal.state,
                        tasks::ProposalState::Pending { holder: tasks::ProposalHolder::Person(1), .. }
                    )
                    && peers.asked_accept.insert((row.number, proposal.number))
                {
                    events.push(Input::Event(root::Event::Core(core::Event::People(people::Event::Ask {
                        models: Box::new([]),
                        reply_to: ReplyTo::new(Token::new(200)),
                        sign_in,
                        key: {
                            let mut key = [0; 16];
                            key[..8].copy_from_slice(&row.number.to_be_bytes());
                            key[8..].copy_from_slice(&proposal.number.to_be_bytes());
                            key
                        },
                        ask: people::Ask::DecideProposal {
                            project: 1,
                            proposer: people::Proposer::Task(row.number),
                            proposal: proposal.number,
                            decision: people::ProposalDecision::Accept,
                            running: None,
                        },
                    }))));
                }
            }
        }
        if ready && peers.document_story {
            events.extend(documents::poll(peers, store));
            let pending = std::mem::take(&mut peers.pending_document_effects);
            for call in pending {
                events.extend(Self::deliver(
                    peers,
                    systems,
                    store,
                    clock,
                    Delivery::Root(Box::new(root::Output::Infrastructure(call))),
                ));
            }
        }
        events.extend(peers.agents.poll(clock));
        events
    }
    fn submitted(peers: &mut Peers, store: &Store, _: Clock) {
        documents::leads_submitted(peers, store);
        if peers.document_story || peers.client_documents {
            documents::submitted(peers, store);
        }
        if peers.application_requests {
            peers.application_observed.extend(application::submitted(store));
        }
    }
    fn observed(peers: &mut Peers, _: &Systems, store: &Store, _: Clock) -> Vec<r::Observed> {
        let mut observed = std::mem::take(&mut peers.application_observed);
        if peers.applied != store.applied {
            observed.extend(observations::intents(&mut peers.intents, store));
            let snapshot = observations::snapshot(store);
            for row in store.rows.values() {
                if let root::Record::Core(core::Record::Core(core::CoreRecord::ProposalDecision(row))) = row
                    && row.choice == people::ProposalChoice::Accepted
                    && !peers.accepted.contains(&store.applied)
                    && peers.accepted.is_empty()
                {
                    peers.accepted.push(store.applied);
                }
            }
            for decision in &snapshot.decisions {
                if !peers.entries.iter().any(|(commit, kind)| *commit == store.applied && *kind == decision.kind) {
                    peers.entries.push((store.applied, decision.kind));
                }
            }
            observed.push(r::Observed::Durable { number: store.applied, snapshot });
            for row in store.rows.values() {
                if let root::Record::Core(core::Record::Tasks(tasks::Stored::Live(row))) = row {
                    for word in &row.inbox {
                        if let (tasks::MessageKind::Result(_), tasks::Party::Task(child)) = (&word.kind, word.from)
                            && peers.delivered_results.insert(child)
                        {
                            observed.push(r::Observed::Result { task: child, to: r::Source::Task(row.number) });
                        }
                    }
                }
            }
            peers.applied = store.applied;
        }
        peers.pending_results.retain(|&(commit, task, parent)| {
            if commit > store.applied {
                return true;
            }
            if peers.delivered_results.insert(task) {
                observed.push(r::Observed::Result { task, to: r::Source::Task(parent) });
            }
            false
        });
        observed.append(&mut peers.observed);
        observed
    }
    fn ready(domain: &root::Domain) -> bool {
        domain.ready()
    }
    fn quiescent(domain: &root::Domain) -> bool {
        domain.quiescent()
    }
    fn reclaim(domain: &mut root::Domain) {
        domain.reclaim();
    }
    fn maximum(_: &Config) -> u64 {
        OPS_HEAP_CEILING
    }
    fn configuration_heap(_: &Config) -> u64 {
        0
    }
}

/// Native client facts share the independent submitted-row durability check.
pub(crate) fn client_document(peers: &mut Peers, store: &Store, document: &root::ClientDocument) {
    let retained = match document {
        root::ClientDocument::Observability(value) => matches!(value.as_ref(), obs::ClientDocument::Incident { .. }),
        root::ClientDocument::Infrastructure(value) => matches!(value.as_ref(), infra::ClientDocument::Panel { .. }),
    };
    if retained {
        documents::delivered(peers, store, document.clone());
    }
}

/// Play the night's recurring template without any agent assignment.
/// Returns the actual harness so the test can inspect outside evidence.
#[must_use]
pub fn night(seed: u64, cut: jig_conformance::Cut) -> jig_conformance::Harness<Ops> {
    let mut world = jig_conformance::Harness::<Ops>::new(Config::new(seed, Story::Night), seed);
    world.cut(cut);
    world.drain().expect("cold ops root");
    world.send(root::Event::Infrastructure(infra::Event::Names {
        project: 1,
        task: 1,
        resources: Box::new([infra::Resource::Service(infra::Service::new(
            b"staging".as_slice().into(),
            b"checkout".as_slice().into(),
        ))]),
    }));
    world.send(root::Event::Core(core::Event::Tasks(tasks::Event::Kinds {
        connector: 2,
        kinds: Box::new([tasks::Kind {
            connector: 2,
            kind: 1,
            hold: tasks::HoldKind::Exclusive { taken: tasks::Taken::Waits },
        }]),
    })));
    world.drain().expect("infrastructure write hold configuration");
    world.advance(1800 * 1_000_000_000);
    world.drain().expect("thirty minutes of staging observations");
    let mut child_authority = config::task_authority(10);
    child_authority.delegation = tasks::Delegation { kinds: Box::new([]), tasks: 0, depth: 0 };
    let mut standing = config::task_authority(30);
    standing.delegation =
        tasks::Delegation { kinds: Box::new([tasks::AuthorityExecutor::Procedure(1)]), tasks: 1, depth: 1 };
    world.send(root::Event::Core(core::Event::StartRecurring {
        project: 1,
        authority: standing,
        template: tasks::RecurringTemplate {
            key: 1,
            overlap: tasks::RecurringOverlap::Skip,
            batch: Box::new([tasks::New {
                deadline: jig_domain_tasks::Deadline::Named,
                label: 0,
                flags: jig_domain_tasks::Flags::PLAIN,
                number: 1,
                project: 1,
                executor: tasks::Executor::Procedure { connector: 2, code: 1 },
                llm: None,
                spec: tasks::Spec {
                    words: b"night scale staging".as_slice().into(),
                    parameters: Box::new([
                        tasks::Parameter::Bytes { name: 1, value: b"staging".as_slice().into() },
                        tasks::Parameter::Bytes { name: 2, value: b"checkout".as_slice().into() },
                        tasks::Parameter::Number { name: 3, value: 1 },
                        tasks::Parameter::Number { name: 4, value: 7200 },
                    ]),
                    inputs: Box::new([]),
                },
                contract: tasks::Contract::Report { words: 128 },
                authority: child_authority,
                numbers: tasks::Numbers { budget: 10, spent: 0, spent_below: 0, reserved: 0 },
                funder: tasks::Funder::Period { project: 1, period: 0 },
                dependencies: Box::new([]),
                holdings: Box::new([tasks::Holding::Write {
                    resource: tasks::Name {
                        connector: 2,
                        path: infra::Resource::Service(infra::Service::new(
                            b"staging".as_slice().into(),
                            b"checkout".as_slice().into(),
                        ))
                        .segments(),
                    },
                    kind: 1,
                }]),
                wake: tasks::WakePolicy::DEFAULT,
                recurring: None,
                tracked: None,
            }]),
        },
    }));
    world.drain().unwrap_or_else(|error| panic!("night: {error:?}\n{}", world.trace.join("\n")));
    // A crash after the sent entry may leave a conditional write uncertain.
    // Advance its configured retry bound so lookup and settlement can finish.
    world.advance(config::root(seed).1.infrastructure.retry_after_seconds * 1_000_000_000);
    world.drain().unwrap_or_else(|error| panic!("night retry: {error:?}\n{}", world.trace.join("\n")));
    world
}

/// The alert storm and incident, including the person's authenticated decision.
#[must_use]
pub fn alert(seed: u64, cut: jig_conformance::Cut) -> jig_conformance::Harness<Ops> {
    alert_documents(seed, cut, false)
}

/// Observe paced whole application documents during the real alert story.
#[must_use]
pub fn documents(seed: u64) -> jig_conformance::Harness<Ops> {
    alert_documents(seed, jig_conformance::Cut::None, true)
}

fn alert_documents(seed: u64, cut: jig_conformance::Cut, documents: bool) -> jig_conformance::Harness<Ops> {
    let mut configuration = Config::new(seed, Story::Alert);
    configuration.documents = documents;
    let mut world = jig_conformance::Harness::<Ops>::new(configuration, seed);
    world.cut(cut);
    world.drain().expect("cold ops root");
    world.send(root::Event::Infrastructure(infra::Event::Names {
        project: 1,
        task: 1,
        resources: Box::new([infra::Resource::Service(infra::Service::new(
            b"production".as_slice().into(),
            b"checkout".as_slice().into(),
        ))]),
    }));
    world.send(root::Event::Core(core::Event::Tasks(tasks::Event::Kinds {
        connector: 2,
        kinds: Box::new([tasks::Kind {
            connector: 2,
            kind: 1,
            hold: tasks::HoldKind::Exclusive { taken: tasks::Taken::Waits },
        }]),
    })));
    world.send(root::Event::Core(core::Event::People(people::Event::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(Token::new(101)),
        sign_in: world.peers.sign_in.expect("on-call session"),
        key: [2; 16],
        ask: config::chat(b"@watch_setup"),
    })));
    world.drain().unwrap_or_else(|error| panic!("watch: {error:?}\n{}", world.trace.join("\n")));
    for number in [7, 8, 9] {
        world.send(root::Event::Observability(obs::Event::System(obs::SystemEvent::Alert {
            own: false,
            alert: obs::Alert {
                number,
                service: obs::Service::new(b"production".as_slice().into(), b"checkout".as_slice().into()),
                severity: 1,
            },
        })));
    }
    world.drain().unwrap_or_else(|error| panic!("storm: {error:?}\n{}", world.trace.join("\n")));
    world.systems.production.inject_incident(&production::ServiceName::new("production", "checkout"));
    world.send(root::Event::Observability(obs::Event::System(obs::SystemEvent::Alert {
        own: false,
        alert: obs::Alert {
            number: 12,
            service: obs::Service::new(b"production".as_slice().into(), b"checkout".as_slice().into()),
            severity: 3,
        },
    })));
    world.advance(1_000_000_000);
    world.drain().unwrap_or_else(|error| panic!("incident: {error:?}\n{}", world.trace.join("\n")));
    world.advance(config::root(seed).1.infrastructure.retry_after_seconds * 1_000_000_000);
    world.drain().unwrap_or_else(|error| panic!("recovery: {error:?}\n{}", world.trace.join("\n")));
    world.advance(config::root(seed).1.infrastructure.retry_after_seconds * 1_000_000_000);
    world.drain().expect("settle an uncertain restart after its retry deadline");
    // A cut around lead creation can defer I or T2 to the next retry clock.
    // Two bounded fact/recovery windows cover that resumed run and its effect.
    for _ in 0..2 {
        refresh_alert_facts(&mut world);
        world.drain().expect("restart after recovered triage or lead");
        world.advance(10_000_000_000);
        world.drain().expect("production recovery clock");
    }
    let refreshed = world.systems.infrastructure(infra::SystemRequest::Service {
        service: infra::Service::new(b"production".as_slice().into(), b"checkout".as_slice().into()),
    });
    world.send(root::Event::Infrastructure(infra::Event::System(refreshed)));
    world.drain().unwrap_or_else(|error| panic!("result: {error:?}\n{}", world.trace.join("\n")));
    world.advance(1_000_000_000);
    world.drain().expect("read the result");
    if documents {
        let refreshed = world.systems.observability(obs::SystemRequest::Facts {
            service: obs::Service::new(b"production".as_slice().into(), b"checkout".as_slice().into()),
        });
        world.send(root::Event::Observability(obs::Event::System(refreshed)));
        world.drain().expect("incident health after remediation");
    }
    read_report(&mut world, cut);
    world
}

fn refresh_alert_facts(world: &mut jig_conformance::Harness<Ops>) {
    let refreshed = world.systems.infrastructure(infra::SystemRequest::Service {
        service: infra::Service::new(b"production".as_slice().into(), b"checkout".as_slice().into()),
    });
    world.send(root::Event::Infrastructure(infra::Event::System(refreshed)));
    let refreshed = world.systems.observability(obs::SystemRequest::Facts {
        service: obs::Service::new(b"production".as_slice().into(), b"checkout".as_slice().into()),
    });
    observations::fact(&mut world.peers.observed, &refreshed);
    world.send(root::Event::Observability(obs::Event::System(refreshed)));
}

fn read_report(world: &mut jig_conformance::Harness<Ops>, cut: jig_conformance::Cut) {
    // The scripted client reads the committed archive under this fixture's
    // authenticated project-owner permission, without copying a live result.
    let sign_in = world.peers.sign_in.expect("on-call session");
    let row = world
        .store
        .rows
        .get(&root::Key::Core(core::Key::People(people::Key::SignIn(sign_in))))
        .expect("durable sign-in");
    let root::Record::Core(core::Record::People(people::Stored::SignIn { person, expires, .. })) = row else {
        panic!("sign-in row");
    };
    assert_eq!(*person, 1);
    assert!(expires.as_nanos() > world.clock().now);
    let role = world.store.rows.get(&root::Key::Core(core::Key::People(people::Key::Roles(1)))).expect("project roles");
    let root::Record::Core(core::Record::People(people::Stored::Roles { holdings, .. })) = role else {
        panic!("roles row");
    };
    assert!(holdings.iter().any(|holding| holding.person == *person && holding.role == people::Role::Owner));
    let result = world.store.rows.get(&root::Key::Core(core::Key::Tasks(tasks::Key::Ended(5)))).unwrap_or_else(|| {
        panic!(
            "incident report missing {cut:?}: {:?}\n{}",
            world
                .store
                .rows
                .values()
                .filter_map(|row| match row {
                    root::Record::Core(core::Record::Tasks(tasks::Stored::Live(task))) if task.number == 5 =>
                        Some((task.phase.clone(), task.inbox.clone(), task.numbers)),
                    root::Record::Core(_) | root::Record::Observability(_) | root::Record::Infrastructure(_) => None,
                })
                .collect::<Vec<_>>(),
            world.trace.join("\n")
        )
    });
    let root::Record::Core(core::Record::Tasks(tasks::Stored::Ended(task))) = result else {
        panic!("ended task row");
    };
    assert_eq!(task.project, 1);
    assert!(
        matches!(&task.phase, tasks::Phase::Ended(tasks::Ending::Done(tasks::TaskResult::Report { words })) if words.as_ref() == b"checkout recovered after the accepted restart")
    );
    world.peers.results.push(task.number);
}

impl Peers {
    #[must_use]
    pub fn has_active_agent(&self) -> bool {
        self.agents.active()
    }
    /// A scripted active agent finishes with exactly the supplied result bytes.
    pub fn finish_result(&mut self, outcome: &[u8]) -> root::Event {
        self.agents.finish_result(outcome)
    }
}

fn census_owner(store: &Store, row: &root::Record) -> Vec<root::Record> {
    let task = match row {
        root::Record::Core(core::Record::Tasks(tasks::Stored::Result(row))) => row.task,
        root::Record::Core(core::Record::People(jig_domain_people::Stored::EndedRead { task, .. })) => *task,
        root::Record::Core(_) | root::Record::Observability(_) | root::Record::Infrastructure(_) => return Vec::new(),
    };
    let mut rows = Vec::new();
    if let root::Record::Core(core::Record::People(jig_domain_people::Stored::EndedRead { .. })) = row {
        rows.push(row.clone());
    }
    if let Some(owner) = store.rows.get(&root::Key::Core(core::Key::Tasks(tasks::Key::Ended(task)))) {
        rows.push(owner.clone());
    }
    rows
}

mod funding;
pub mod turn_backoff;
