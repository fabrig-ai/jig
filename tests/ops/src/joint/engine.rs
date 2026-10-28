//! Real Ops domain on the conformance loop, retaining scripted non-person peers
//! (client/domain/testing.md, 5; examples.md, 10).
use crate::conformance::{self, Config, Delivery, Ops, Systems};
use jig_client_world::harness::Observed;
use jig_client_world::joint::truth::Truth;
use jig_conformance::{Application, Clock, Input, Output, referee};
use jig_domain_core as core;
use jig_domain_people as people;
use jig_fake_store::Store;
use jig_ops_domain as root;
use skein_lib::Time;

pub type Rows = Store<root::Key, root::Record>;

/// Commit positions read from durable owner rows, for replaying actual cuts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Milestone {
    Acceptance(u64),
    Outbox(u64),
    Made,
    MoveEnd,
}

pub struct Peers {
    pub conformance: conformance::Peers,
    pub packets: Vec<root::Output>,
    pub truth: Truth,
    pub observations: Vec<Observed>,
    pub protocol: jig_client_world::joint::referee::Referee,
    /// Whole authenticated requests observed at the actual root boundary.
    pub keyed_asks: Vec<(u64, [u8; 16], people::Ask)>,
    pub application_asks: Vec<(u64, [u8; 16], jig_ops_domain_infrastructure::ApplicationRequest, [u8; 32])>,
    /// Opaque family request bytes that crossed the native component boundary.
    pub application_bytes: Vec<([u8; 16], Box<[u8]>)>,
    pub milestones: Vec<(Milestone, u64)>,
    /// The outside alert sender retains its whole notifications across a
    /// receiver crash; cold recovery redelivers without inventing new facts.
    pub alert_news: Vec<(bool, jig_ops_domain_observability::Alert)>,
    replay_news: bool,
    applied: u64,
    tree_tasks: u32,
    delayed_infrastructure: Vec<(u64, root::Output)>,
}

pub struct OpsApplication;

/// Project only actual durable core rows into the reusable jig observer/store face.
/// Connector rows remain with Ops's own conformance referee.
#[must_use]
pub fn core_store(store: &Rows) -> super::reads::Store {
    let mut values = super::reads::Store::default();
    for row in store.rows.values() {
        match row {
            root::Record::Core(record) => {
                let row = jig_test_domain::Record::Core(record.clone());
                values.rows.insert(row.key(), row);
            }
            root::Record::Infrastructure(_) | root::Record::Observability(_) => {}
        }
    }
    values.applied = store.applied;
    values
}

fn task_row(store: &super::reads::Store, number: u64) -> Option<&jig_domain_tasks::TaskRecord> {
    store.rows.values().find_map(|record| match record {
        jig_test_domain::Record::Core(core::Record::Tasks(
            jig_domain_tasks::Stored::Live(task) | jig_domain_tasks::Stored::Ended(task),
        )) if task.number == number => Some(task.as_ref()),
        jig_test_domain::Record::Core(_) | jig_test_domain::Record::Connector { .. } => None,
    })
}

/// The independent store supplies real withdrawn proposal history too. The
/// core world's minimal read peer intentionally does not select this family.
fn read_page(store: &Rows, range: &core::reads::Range, tree_tasks: u32) -> core::reads::Loaded {
    let store = core_store(store);
    let core::reads::Scope::Proposal { proposer: core::documents::Proposer::Task(task), proposal } = range.scope else {
        return super::reads::page(&store, range, tree_tasks);
    };
    let mut ordered = std::collections::BTreeMap::new();
    for record in store.rows.values() {
        let jig_test_domain::Record::Core(core::Record::Tasks(jig_domain_tasks::Stored::History(history))) = record
        else {
            continue;
        };
        let Some(kept) = &history.proposal else { continue };
        if kept.proposer != task || kept.number != proposal {
            continue;
        }
        let Some(owner) = task_row(&store, history.task) else { continue };
        let mut ancestors = Vec::new();
        let mut parent = owner.requester;
        while let jig_domain_tasks::Party::Task(number) = parent {
            assert!(!ancestors.contains(&number), "actual task ancestry is acyclic");
            ancestors.push(number);
            let Some(row) = task_row(&store, number) else { break };
            parent = row.requester;
        }
        let row = core::reads::Row {
            value: core::Record::Tasks(jig_domain_tasks::Stored::History(history.clone())),
            owner: Some(Box::new(owner.clone())),
            ancestors: ancestors.into(),
            related: None,
            unjoined: None,
            ended_read: None,
            linked: Box::new([]),
            inbox: None,
        };
        ordered.insert(core::reads::row_key(&row).expect("actual history position"), row);
    }
    let mut candidates: Vec<_> = ordered
        .into_iter()
        .filter(|(position, _)| match &range.after {
            Some(after) => {
                if range.backwards {
                    position < after
                } else {
                    position > after
                }
            }
            None => true,
        })
        .collect();
    if range.backwards {
        candidates.reverse();
    }
    let mut rows = Vec::new();
    let mut bytes = 0;
    let mut last = None;
    let mut more = false;
    for (position, row) in candidates {
        let size = row.owned_bytes().expect("whole actual history row pricing");
        if rows.len() == range.rows as usize || size > range.bytes - bytes {
            if rows.is_empty() {
                return core::reads::Loaded::Busy;
            }
            more = true;
            break;
        }
        bytes += size;
        last = Some(position);
        rows.push(row);
    }
    core::reads::Loaded::Page { rows: rows.into(), next: if more { last } else { None } }
}

impl Application for OpsApplication {
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
        Ops::build(config)
    }
    fn systems(config: &Config, seed: u64) -> Systems {
        Ops::systems(config, seed)
    }
    fn peers(config: &Config, seed: u64) -> Peers {
        let mut conformance = Ops::peers(config, seed);
        // Interactive navigation and the post-action health wait outlive the
        // inline scenario's 2,000 iterations; keep their joint allowance finite.
        conformance.policy.story_steps = 20_000;
        Peers {
            conformance,
            packets: Vec::new(),
            truth: Truth::default(),
            observations: Vec::new(),
            protocol: jig_client_world::joint::referee::Referee::default(),
            keyed_asks: Vec::new(),
            application_asks: Vec::new(),
            application_bytes: Vec::new(),
            milestones: Vec::new(),
            alert_news: Vec::new(),
            replay_news: false,
            applied: 0,
            tree_tasks: config.limits().core.tasks.tree_tasks,
            delayed_infrastructure: Vec::new(),
        }
    }
    fn policy(peers: &Peers) -> &referee::Policy {
        Ops::policy(&peers.conformance)
    }
    fn policy_generation(peers: &Self::Peers) -> u64 {
        Ops::policy_generation(&peers.conformance)
    }

    fn recovery(config: &Config, connector: u16, kind: u16) -> referee::Recovery {
        Ops::recovery(config, connector, kind)
    }
    fn start(config: &Config, store: &mut Rows) -> Vec<root::Event> {
        Ops::start(config, store)
    }
    fn cold(peers: &mut Peers, store: &Rows, clock: Clock) {
        peers.packets.clear();
        peers.protocol.restarted();
        peers.replay_news = true;
        Ops::cold(&mut peers.conformance, store, clock);
    }
    fn timers(config: &Config) -> Vec<root::Event> {
        Ops::timers(config)
    }
    fn step(domain: &mut root::Domain, config: &Config, clock: Clock, input: Input<root::Event, root::Record>) {
        Ops::step(domain, config, clock, input);
    }
    fn release(
        domain: &mut root::Domain,
        config: &Config,
        clock: Clock,
    ) -> Vec<Output<jig_fake_store::Write<root::Key, root::Record>, Delivery>> {
        Ops::release(domain, config, clock)
    }
    fn inbound(peers: &mut Peers, store: &Rows, clock: Clock, event: &mut root::Event) {
        if let root::Event::Core(core::Event::People(people::Event::Ask { sign_in, key, ask, .. })) = event {
            peers.keyed_asks.push((*sign_in, *key, ask.clone()));
        }
        if let root::Event::Application { sign_in, key, value, digest, .. } = event {
            peers.application_asks.push((*sign_in, *key, value.clone(), *digest));
        }
        if let root::Event::Core(core::Event::Delivered { watcher }) = event {
            peers.protocol.acknowledged(*watcher);
        }
        Ops::inbound(&mut peers.conformance, store, clock, event);
    }
    #[expect(clippy::too_many_lines, reason = "held root outputs retain their original exhaustive owner routes")]
    fn deliver(
        peers: &mut Peers,
        systems: &mut Systems,
        store: &mut Rows,
        clock: Clock,
        delivery: Delivery,
    ) -> Vec<Input<root::Event, root::Record>> {
        let delivery = match delivery {
            Delivery::Root(value) => *value,
            other @ Delivery::Results { .. } => {
                return Ops::deliver(&mut peers.conformance, systems, store, clock, other);
            }
        };
        if let root::Output::CoreLoad(core::Now::SeenConversation { request, task }) = delivery {
            let row = store.rows.values().find_map(|record| match record {
                root::Record::Core(core::Record::Tasks(jig_domain_tasks::Stored::Ended(row))) if row.number == task => {
                    Some(row.clone())
                }
                root::Record::Core(_) | root::Record::Infrastructure(_) | root::Record::Observability(_) => None,
            });
            return vec![Input::Event(root::Event::Core(core::Event::SeenConversationLoaded {
                request,
                row,
                busy: false,
            }))];
        }
        if let root::Output::Core(core::Held::ReadLoad { load, range }) = delivery {
            let loaded = read_page(store, &range, peers.tree_tasks);
            return vec![Input::Event(root::Event::Core(core::Event::ReadLoaded { load, result: loaded }))];
        }
        match &delivery {
            root::Output::ApplicationDocument { document, .. }
            | root::Output::ApplicationRead { answer: Ok(document), .. } => {
                conformance::client_document(&mut peers.conformance, store, document);
            }
            root::Output::ApplicationRead { answer: Err(_), .. }
            | root::Output::IncidentAssociationRefused { .. }
            | root::Output::Application { .. }
            | root::Output::Commit { .. }
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
            | root::Output::Stop => {}
        }
        // The genuine external system has a finite response latency. Keep its
        // original Apply and let P see the committed restarting panel meanwhile.
        if let root::Output::Infrastructure(jig_ops_domain_infrastructure::SystemRequest::Apply { .. }) = &delivery {
            peers
                .delayed_infrastructure
                .push((clock.now.checked_add(1_000_000_000).expect("finite system response"), delivery));
            return Vec::new();
        }
        let client_owned = match &delivery {
            root::Output::Core(
                core::Held::PeopleReply { .. }
                | core::Held::Read { .. }
                | core::Held::Delivery { .. }
                | core::Held::WatchEnded { .. }
                | core::Held::ApplicationWatchEnded { .. }
                | core::Held::CarriedUnused { .. },
            )
            | root::Output::ApplicationDocument { .. }
            | root::Output::ApplicationRead { .. }
            | root::Output::Application { .. }
            | root::Output::Now(
                core::Now::RunText { .. } | core::Now::WatchRefused { .. } | core::Now::SignInRefused { .. },
            ) => true,
            root::Output::Core(
                core::Held::ApplicationDelivery { .. }
                | core::Held::ReadLoad { .. }
                | core::Held::Applied { .. }
                | core::Held::MakeEffect { .. }
                | core::Held::SettledCall { .. }
                | core::Held::Assign { .. }
                | core::Held::Grant { .. }
                | core::Held::Relayed { .. }
                | core::Held::Inbound { .. }
                | core::Held::Relay { .. }
                | core::Held::CallAnswer { .. }
                | core::Held::Result { .. }
                | core::Held::Acknowledge { .. }
                | core::Held::TaskTerminalAcknowledged { .. }
                | core::Held::TaskTurnKept { .. }
                | core::Held::AcknowledgeTurn { .. }
                | core::Held::Drained
                | core::Held::Cancel { .. }
                | core::Held::Sources { .. }
                | core::Held::CheckSource { .. }
                | core::Held::SignOutSource { .. }
                | core::Held::SourceSignedOut { .. }
                | core::Held::Refuse { .. }
                | core::Held::TurnBusy { .. }
                | core::Held::StopRun { .. }
                | core::Held::Load(..)
                | core::Held::NotesLoad { .. }
                | core::Held::NotesWritten { .. }
                | core::Held::Acted { .. }
                | core::Held::NotesDeleted { .. },
            )
            | root::Output::Now(
                core::Now::Relayed { .. }
                | core::Now::EffectAnswer { .. }
                | core::Now::SettingsAnswer { .. }
                | core::Now::SettledCallRefused { .. }
                | core::Now::Call { .. }
                | core::Now::RelayBusy { .. }
                | core::Now::CallBusy { .. }
                | core::Now::DropCall { .. }
                | core::Now::Undelivered { .. }
                | core::Now::Account(_)
                | core::Now::NotesIndexed { .. }
                | core::Now::NotesRecalled { .. }
                | core::Now::NotesRefused { .. }
                | core::Now::NoteBusy { .. }
                | core::Now::DropPayload { .. }
                | core::Now::DropAssignment { .. }
                | core::Now::TurnPayload { .. }
                | core::Now::AnswerPayload { .. }
                | core::Now::AcceptedTurn { .. }
                | core::Now::RefusedPayload { .. }
                | core::Now::Activate { .. }
                | core::Now::PrepareAgent { .. }
                | core::Now::StartPreparation { .. }
                | core::Now::SeenConversation { .. }
                | core::Now::LoadEndedRead { .. }
                | core::Now::HistoricalProposal { .. }
                | core::Now::DecidedProposal { .. }
                | core::Now::HistoricalPersonTask { .. }
                | core::Now::EndedTask { .. }
                | core::Now::PartyInputs { .. }
                | core::Now::CompleteBrief { .. }
                | core::Now::BriefCorePlanned { .. }
                | core::Now::WorkspaceRequest { .. }
                | core::Now::RunPreparationFailed { .. }
                | core::Now::RunPrepared { .. }
                | core::Now::HistoricalEscalation { .. }
                | core::Now::RestoreRefused,
            )
            | root::Output::IncidentAssociationRefused { .. }
            | root::Output::Commit { .. }
            | root::Output::Switch
            | root::Output::Load { .. }
            | root::Output::ReadResult { .. }
            | root::Output::CoreLoad(_)
            | root::Output::Observability(_)
            | root::Output::Infrastructure(_)
            | root::Output::Read { .. }
            | root::Output::Agent(_)
            | root::Output::ToChild(_)
            | root::Output::ProjectMade { .. }
            | root::Output::Stop => false,
        };
        if client_owned {
            peers.packets.push(delivery);
            Vec::new()
        } else {
            Ops::deliver(&mut peers.conformance, systems, store, clock, Delivery::Root(Box::new(delivery)))
        }
    }
    fn poll(
        peers: &mut Peers,
        systems: &mut Systems,
        store: &mut Rows,
        clock: Clock,
        ready: bool,
    ) -> Vec<Input<root::Event, root::Record>> {
        let mut events = Ops::poll(&mut peers.conformance, systems, store, clock, ready);
        if ready && peers.replay_news {
            peers.replay_news = false;
            for (own, alert) in &peers.alert_news {
                events.push(Input::Event(root::Event::Observability(jig_ops_domain_observability::Event::System(
                    jig_ops_domain_observability::SystemEvent::Alert { own: *own, alert: alert.clone() },
                ))));
            }
        }
        let delayed = std::mem::take(&mut peers.delayed_infrastructure);
        for (due, delivery) in delayed {
            if clock.now >= due {
                events.extend(Ops::deliver(
                    &mut peers.conformance,
                    systems,
                    store,
                    clock,
                    Delivery::Root(Box::new(delivery)),
                ));
            } else {
                peers.delayed_infrastructure.push((due, delivery));
            }
        }
        events
    }
    fn submitted(peers: &mut Peers, store: &Rows, clock: Clock) {
        Ops::submitted(&mut peers.conformance, store, clock);
    }
    fn observed(peers: &mut Peers, systems: &Systems, store: &Rows, clock: Clock) -> Vec<referee::Observed> {
        if peers.applied != store.applied {
            for row in store.rows.values() {
                let milestone = match row {
                    root::Record::Core(core::Record::Core(core::CoreRecord::ProposalDecision(decision)))
                        if decision.choice == people::ProposalChoice::Accepted =>
                    {
                        match decision.proposer {
                            people::Proposer::Task(task) => Some(Milestone::Acceptance(task)),
                            people::Proposer::Party(_) => None,
                        }
                    }
                    root::Record::Infrastructure(jig_ops_domain_infrastructure::Record::Outbox(entry)) => {
                        Some(Milestone::Outbox(entry.key.task))
                    }
                    root::Record::Infrastructure(jig_ops_domain_infrastructure::Record::Made { .. }) => {
                        Some(Milestone::Made)
                    }
                    root::Record::Infrastructure(jig_ops_domain_infrastructure::Record::ApplicationAnswer {
                        outcome: jig_ops_domain_infrastructure::ApplicationOutcome::MovedEnd { .. },
                        ..
                    }) => Some(Milestone::MoveEnd),
                    root::Record::Core(_) | root::Record::Infrastructure(_) | root::Record::Observability(_) => None,
                };
                if let Some(milestone) = milestone
                    && !peers.milestones.iter().any(|(found, _)| *found == milestone)
                {
                    peers.milestones.push((milestone, store.applied));
                }
            }
            peers.observations.extend(peers.truth.applied(jig_core_world::store_peer::DurableSnapshot {
                commit: store.applied,
                at: Time::from_nanos(clock.now),
                rows: core_store(store).rows,
            }));
            peers.applied = store.applied;
        }
        Ops::observed(&mut peers.conformance, systems, store, clock)
    }
    fn ready(domain: &root::Domain) -> bool {
        Ops::ready(domain)
    }
    fn quiescent(domain: &root::Domain) -> bool {
        Ops::quiescent(domain)
    }
    fn reclaim(domain: &mut root::Domain) {
        Ops::reclaim(domain);
    }
    fn maximum(config: &Config) -> u64 {
        Ops::maximum(config)
    }
    fn configuration_heap(config: &Config) -> u64 {
        Ops::configuration_heap(config)
    }
}
