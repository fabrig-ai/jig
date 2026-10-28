//! A scripted caller and two independent systems exercise the root boundary.
mod account_preparation;
mod credentials;
mod grants;
use crate::Store;
use jig_domain_accounts as accounts;
use jig_domain_authority as authority;
use jig_domain_core as core;
use jig_domain_fleet as fleet;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_test_connector as connector;
use jig_test_domain as root;
use jig_test_store::{StoreOut, Up};
use jig_test_system::{Fault, System};
use skein_lib::{Duration, Env, Queue, ReplyTo, Time, Token, Wall};
use std::collections::VecDeque;
mod bootstrap;
mod constructors;
mod deliveries;
mod oracle;
mod store;
pub use constructors::fixture;

/// A scenario's cut around one store transaction, before application or reply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cut {
    /// Submission has left the root but is not durable.
    Submitted(u64),
    /// The rows are durable but the root has not received the completion.
    Durable(u64),
}

/// Committed turns supplied to one assignment, captured at the worker boundary.
#[derive(Debug)]
pub struct AssignedTranscript {
    pub task: u64,
    pub turns: Box<[Box<[u8]>]>,
}

/// Root, store and separate systems, with an answer channel that may disappear.
#[derive(Debug)]
#[expect(clippy::struct_excessive_bools, reason = "independently held peer traffic and crash cuts")]
pub struct World {
    bootstrap_owners: Vec<people::InitialOwner>,
    oracle_hellos: Vec<(u32, Token, fleet::Hello)>,
    oracle_configuration: Option<root::Config>,
    oracle_skip: Option<&'static str>,
    fixture_accounts: Box<[root::Record]>,
    pub domain: root::Domain,
    pub store: Store,
    pub systems: [System; 2],
    protocol: crate::protocol::Stand,
    pub host_calls: Vec<(Token, u64, u64, fleet::Call)>,
    pub host_answers: Vec<(Box<[u8]>, core::SettledCall)>,
    pub inbound: Vec<tasks::Word>,
    pub settings_answers: Vec<(Token, core::CallKey, core::SettingsAnswer)>,
    pub answers: Vec<(Token, core::CallKey, core::CallPart)>,
    pub assigned: Option<root::Assignment>,
    last_read: Option<u64>,
    received_grant: Option<(Token, Token, Token, fleet::Grant)>,
    grant_follows_assignment: bool,
    renew_on_running: Option<accounts::FromComponent>,
    pub fail_account_before_workspace: bool,
    pub hold_accounts: bool,
    /// Visit a sign-in, then wait for a scripted pasted address.
    pub wait_for_finish: bool,
    pub finish_script: VecDeque<Option<accounts::SignInRedirectWhy>>,
    pub finishes: Vec<(u32, u64)>,
    /// Fail the next LLM key keep at the scripted accounts component.
    pub fail_llm_key_keep: Option<accounts::KeyFailure>,
    /// Refuse the next moved record as belonging to another application.
    pub fail_move_open: bool,
    moved: std::collections::BTreeMap<u32, skein_lib::Secret>,
    account_generations: std::collections::BTreeMap<u32, u64>,
    move_provider: Option<u32>,
    pub assignments: Vec<(u64, u64)>,
    pub assignment_transcripts: Vec<AssignedTranscript>,
    pub cancellations: Vec<(u64, u64)>,
    pub busy_turns: Vec<(u64, u64, u32)>,
    pub busy_calls: Vec<Box<[u8]>>,
    pub busy_events: Vec<root::Event>,
    pub stop_deliveries: Vec<core::Held>,
    pub commits: Vec<Vec<root::Write>>,
    pub projections: Vec<(usize, u16, core::ProjectionFeed)>,
    pub hold_projection_settlements: bool,
    pub sign_in: Option<u64>,
    pub initial_ask: people::Ask,
    pub(crate) creating: crate::fixtures::Creating,
    pub people_answers: Vec<people::Reply>,
    /// Project notification and the durable store mark at its release.
    pub project_made: Vec<(u64, u32)>,
    pub act_answers: Vec<(Token, people::Outcome)>,
    pub application_answers: Vec<(Token, u16, connector::ApplicationOutcome)>,
    /// Each carried value the core left unused, with how many answers
    /// (people's and applications') were delivered before it.
    pub carried_unused: Vec<(usize, u64)>,
    pub procedure: Option<u64>,
    /// Whole held protocol inputs received by the independent procedure connector.
    pub procedure_inputs: Vec<(u64, u16, u16, Box<tasks::RunContext>)>,
    /// A procedure connector can withhold its reply while a story drives its step.
    pub hold_procedures: bool,
    pub pending_reads: Vec<(u16, connector::SystemRequest)>,
    /// Hold a lost writer's fresh read until the scenario releases it.
    pub hold_lost_reads: bool,
    pub pending_lost_reads: Vec<(u16, tasks::Name, connector::SystemRequest)>,
    pub lost_answers: u32,
    pub hold_writes: bool,
    pub recovery: connector::Recovery,
    pub hold_lookups: bool,
    pub pending_lookups: Vec<(u16, connector::SystemRequest)>,
    pub pending_writes: Vec<(u16, connector::SystemRequest)>,
    pub lose_answer: bool,
    pub trace: Vec<String>,
    tracing: bool,
    pub restart_steps: Vec<core::RestartStep>,
    pub restore_ranges: Vec<core::RestoreRange>,
    pub census_ended: u32,
    pub hold_restart_reads: bool,
    /// Withhold requested whole store pages during a real cold restore.
    pub hold_restore_pages: bool,
    pub hold_switch: bool,
    /// Delay before a submitted store transaction becomes durable.
    pub store_delay: u32,
    /// Store map bytes supplied with subsequent commit answers.
    pub fill: u64,
    /// Whether the root reported a failed commit and stopped.
    pub stopped: bool,
    period_started: bool,
    /// Independent scripted hosts and clients, when this scenario uses them.
    pub peers: Option<crate::peers::Peers>,
    /// Client-facing outputs; their protocol adapter runs in the joint world.
    pub client_peer: Option<crate::client_peer::ClientPeer>,
    /// Whole applied store snapshots, enabled only for external durable observers.
    pub store_peer: Option<crate::store_peer::StorePeer>,
    /// Independent document consumers, enabled only in watch stories.
    pub watches: Option<crate::peers::WatchParties>,
    /// Independent offer observer, enabled for immediately acting scripted parties.
    pub offers: Option<crate::referee::offers::Referee>,
    /// Independent read parties and owner-range store transport, enabled in read stories.
    pub reads: Option<crate::reads::Parties>,
    /// Referee observations, independent of the root state.
    pub observer: crate::observations::Observer,
    heap: crate::observations::Heap,
    restoring: Option<(core::RestartStep, usize)>,
    restart_reads: std::collections::BTreeMap<u16, core::RestartStep>,
    events: VecDeque<root::Event>,
    limits: root::Limits,
    /// Pause at a selected transaction boundary, with its traffic retained.
    pub cut: Option<Cut>,
    /// The boundary at which the last drain paused.
    pub reached_cut: Option<Cut>,
    hold_kinds: Vec<(u16, Box<[tasks::Kind]>)>,
    wall: u64,
    wall_lag: u64,
    supplied_wall: Option<Wall>,
}

impl World {
    /// Supply monotonic time and wall time independently, including wall corrections.
    pub fn set_clock(&mut self, now: Time, wall: Wall) {
        if self.supplied_wall.is_some() {
            assert!(now.as_nanos() >= self.wall, "elapsed time is monotonic");
        }
        if self.env().wall != wall
            && let Some(offers) = &mut self.offers
        {
            offers.clock_changed();
        }
        self.wall = now.as_nanos();
        self.supplied_wall = Some(wall);
    }

    /// The pair actually supplied to the root and its children.
    #[must_use]
    pub fn clocks(&self) -> (Time, Wall) {
        let env = self.env();
        (env.now, env.wall)
    }

    /// Earliest armed owner deadline at the current supplied clock pair.
    /// Absolute Wall values are sampled again after a correction or delivery.
    #[must_use]
    pub fn next_due(&self) -> Option<Time> {
        let (now, wall) = self.clocks();
        let mut next = None;
        for (_, deadline) in self.domain.core_deadlines(now, wall) {
            if let Some(deadline) = deadline {
                next = Some(next.map_or(deadline, |prior: Time| prior.min(deadline)));
            }
        }
        for deadline in self
            .domain
            .connector_deadlines()
            .into_iter()
            .map(|(_, deadline)| deadline)
            .chain([self.domain.effect_deadline()])
            .flatten()
        {
            let remaining = deadline.as_nanos().saturating_sub(wall.as_nanos());
            let due = Time::from_nanos(now.as_nanos().checked_add(remaining).expect("world deadline fits"));
            next = Some(next.map_or(due, |prior| prior.min(due)));
        }
        next
    }

    /// Fire only currently due owners through their ordinary root inputs.
    /// The caller supplies clocks; neither draining nor timer delivery advances them.
    pub fn fire_due(&mut self, now: Time, wall: Wall) {
        self.set_clock(now, wall);
        let mut events = Vec::new();
        for (timer, deadline) in self.domain.core_deadlines(now, wall) {
            if deadline.is_some_and(|deadline| deadline <= now) {
                events.push(root::Event::Timer(timer));
            }
        }
        for (number, deadline) in self.domain.connector_deadlines() {
            if deadline.is_some_and(|deadline| deadline <= wall) {
                events.push(root::Event::ConnectorTimer { number });
            }
        }
        if self.domain.effect_deadline().is_some_and(|deadline| deadline <= wall) {
            events.push(root::Event::Core(core::Event::EffectDeadline));
        }
        self.send_all(events);
    }

    /// Advance simulated time, firing fleet and task deadlines through the root.
    pub fn advance(&mut self, by: Duration) {
        self.wall = self.wall.checked_add(by.as_nanos()).expect("world time fits");
        self.supplied_wall = self
            .supplied_wall
            .map(|wall| Wall::from_nanos(wall.as_nanos().checked_add(by.as_nanos()).expect("supplied wall fits")));
        self.events.push_back(root::Event::Timer(core::Timer::Fleet));
        self.events.push_back(root::Event::Timer(core::Timer::Tasks));
        self.events.push_back(root::Event::Timer(core::Timer::People));
        self.events.push_back(root::Event::ConnectorTimer { number: 1 });
        self.events.push_back(root::Event::ConnectorTimer { number: 2 });
        self.drain();
    }

    /// Correct wall time backwards while elapsed time continues monotonically.
    pub fn correct_wall_backwards(&mut self, by: Duration) {
        self.wall_lag = self.wall_lag.checked_add(by.as_nanos()).expect("wall correction fits");
    }

    fn env(&self) -> Env<root::Limits> {
        Env {
            now: Time::from_nanos(self.wall),
            wall: self.supplied_wall.unwrap_or_else(|| Wall::from_nanos(self.wall.saturating_sub(self.wall_lag))),
            limits: self.limits,
        }
    }

    /// Queue a burst so independent parties draw their pace between its inputs.
    pub fn send_all(&mut self, events: Vec<root::Event>) {
        self.events.extend(events);
        self.drain();
    }

    pub fn send(&mut self, event: root::Event) {
        self.events.push_back(event);
        self.drain();
    }

    /// Retain diagnostics for a focused story that asserts on trace text.
    pub fn enable_tracing(&mut self) {
        self.tracing = true;
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one iteration keeps commit application, peer observations and root outputs in order"
    )]
    pub fn drain(&mut self) {
        for _ in 0..300 {
            if self.stopped {
                return;
            }
            if self.supplied_wall.is_none() {
                self.wall += 1_000_000;
            }
            self.observer.observe(self.wall, crate::referee::Observed::Tick);
            let env = self.env();
            self.observer.systems(self.wall, &self.systems);
            if let Some(mut event) = self.events.pop_front() {
                self.remember_oracle_hello(&event);
                self.observer.inbound(&self.store, self.wall, &mut event);
                if let Some(offers) = &mut self.offers {
                    offers.inbound(&mut event);
                }
                if let Some(watches) = &mut self.watches {
                    watches.inbound(&event);
                }
                if let Some(reads) = &mut self.reads {
                    reads.inbound(&event);
                }
                if self.tracing {
                    self.trace.push(format!("input {event:?}"));
                }
                root::step(&mut self.domain, &env, event);
            }
            let mut out = Queue::with_capacity(128);
            root::release(&mut self.domain, &env, &mut out);
            let empty = out.is_empty();
            for _ in 0..out.len() {
                let request = out.pop().expect("root output count");
                if self.tracing {
                    self.trace.push(format!("output {request:?}"));
                }
                match request {
                    root::Request::Restart(step) => self.restart_step(step),
                    root::Request::Commit { number, mut writes } => {
                        let mut rows = Vec::new();
                        while let Some(write) = writes.pop() {
                            rows.push(write);
                        }
                        rows.extend(
                            std::mem::take(&mut self.fixture_accounts).into_vec().into_iter().map(root::Write::Save),
                        );
                        if let Some(watches) = &mut self.watches {
                            watches.commit(number, &rows);
                        }
                        if let Some(offers) = &mut self.offers {
                            offers.committed(number);
                        }
                        if let Some(reads) = &mut self.reads {
                            reads.commit(number);
                        }
                        self.inject_assignment_renewal(&rows);
                        self.commits.push(rows.clone());
                        self.store.submit(number, rows.into_boxed_slice(), self.store_delay);
                        self.observer.submitted(self.wall, &self.store);
                        if self.cut == Some(Cut::Submitted(number)) {
                            self.reached_cut = self.cut.take();
                            return;
                        }
                    }
                    root::Request::Deliver(delivery) => self.delivered(delivery),
                    root::Request::ReadResult { .. } => panic!("scripted results are already typed"),
                    root::Request::Now(now @ core::Now::StartPreparation { .. }) => self.transcript_loaded(now),
                    root::Request::Now(core::Now::Call { to, run, attempt, call }) => {
                        let token = to.into_token();
                        if self.peers.is_none()
                            && let Some(event) = self.protocol.decode(token, run, attempt, &call)
                        {
                            self.events.push_back(event);
                        } else if let Some(peers) = &mut self.peers {
                            self.events.extend(peers.opaque_answer(ReplyTo::new(token), run, attempt, &call));
                        } else {
                            self.host_calls.push((token, run.raw(), attempt.raw(), call));
                        }
                    }
                    root::Request::Now(core::Now::SettingsAnswer { to, key, answer }) => {
                        let token = to.into_token();
                        if let Some(event) = self
                            .peers
                            .as_mut()
                            .map_or(&mut self.protocol, |peers| &mut peers.protocol)
                            .settings_answer(token, key, &answer)
                        {
                            self.events.push_back(event);
                        }
                        self.settings_answers.push((token, key, *answer));
                    }
                    root::Request::Now(core::Now::EffectAnswer { to, key, part }) => {
                        let token = to.into_token();
                        if let Some(event) = self
                            .peers
                            .as_mut()
                            .map_or(&mut self.protocol, |peers| &mut peers.protocol)
                            .answer(token, key, &part, false)
                        {
                            self.events.push_back(event);
                        }
                        self.answers.push((token, key, part));
                    }
                    root::Request::Now(core::Now::Account(accounts::Request::Component(request))) => {
                        self.account_request(request);
                    }
                    root::Request::Now(core::Now::Account(accounts::Request::Signing(signing))) => {
                        self.account_signing(signing);
                    }
                    root::Request::Now(
                        now @ (core::Now::SeenConversation { .. }
                        | core::Now::LoadEndedRead { .. }
                        | core::Now::HistoricalPersonTask { .. }
                        | core::Now::EndedTask { .. }
                        | core::Now::PartyInputs { .. }
                        | core::Now::HistoricalProposal { .. }
                        | core::Now::DecidedProposal { .. }
                        | core::Now::HistoricalEscalation { .. }),
                    ) => self.store.ask(StoreOut::Now(now)),
                    root::Request::Now(now) => {
                        if let Some(peer) = &mut self.client_peer
                            && crate::client_peer::ClientPeer::immediate(&now)
                        {
                            peer.now.push(now);
                            continue;
                        }
                        if let Some(watches) = &mut self.watches {
                            watches.now(&now);
                        }
                    }
                    root::Request::Busy { event } => self.busy_events.push(*event),
                    root::Request::Stop => self.stopped = true,
                }
            }
            if !self.stopped {
                match self.store.tick(false) {
                    Ok(Some(number)) => self.events.push_front(root::Event::Committed { number, fill: self.fill }),
                    Ok(None) => {}
                    Err(number) => self.events.push_front(root::Event::Failed { number }),
                }
                self.observer.durable(self.wall, &self.store);
                if let Some(peer) = &mut self.store_peer {
                    peer.capture(Time::from_nanos(self.wall), &self.store);
                }
                if self.cut == Some(Cut::Durable(self.store.applied)) {
                    self.reached_cut = self.cut.take();
                    return;
                }
                self.observer.systems(self.wall, &self.systems);
                self.restore_pages();
                if let Some(watches) = &mut self.watches {
                    self.events.extend(watches.tick());
                }
                if self.domain.ready()
                    && let Some(peers) = &mut self.peers
                {
                    if self.period_started {
                        self.events.extend(peers.tick(&self.store, env.now));
                    } else {
                        self.period_started = true;
                        self.events.push_front(root::Event::Core(core::Event::Period {
                            project: 1,
                            period: 1,
                            budget: 1000,
                            next: None,
                        }));
                    }
                }
            }
            if let Some(peers) = &self.peers {
                self.observer.workers(self.wall, &peers.workers);
            }
            self.domain.reclaim();
            self.observer.observe(self.wall, self.heap.observed());
            let held_restore = self.hold_restore_pages && self.restoring.is_some() && self.store.loading();
            if empty
                && self.events.is_empty()
                && self.store.pending.is_empty()
                && (!self.store.loading() || held_restore)
                && (self.domain.quiescent()
                    || held_restore
                    || self.hold_switch
                    || !self.store.held.is_empty()
                    || self.reads.as_ref().is_some_and(crate::reads::Parties::paused))
                && self.watches.as_ref().is_none_or(crate::peers::WatchParties::quiescent)
            {
                return;
            }
        }
        panic!("effect world did not settle: {:?}", self.trace);
    }

    #[must_use]
    pub fn call_key(&self, completion: u32) -> core::CallKey {
        let assignment = self.assigned.as_ref().expect("assigned caller");
        core::CallKey { task: assignment.task, attempt: assignment.attempt, completion, position: 0 }
    }

    #[must_use]
    pub fn effect() -> connector::Effect {
        connector::Effect {
            kind: 5,
            resources: Box::new([jig_test_connector_world::path(1, 1)]),
            purpose: 19,
            condition: None,
            target: 13,
            state: 13,
        }
    }

    pub fn call(&mut self, completion: u32, right: u64) {
        self.send(root::Event::EffectCall {
            to: ReplyTo::new(Token::new(right)),
            key: self.call_key(completion),
            number: 1,
            effect: Self::effect(),
            deadline: Wall::from_nanos(self.wall + 1_000_000_000),
            proposal: None,
        });
    }

    /// The caller's independently stored expense, including made effects.
    #[must_use]
    pub fn spent(&self) -> u64 {
        let task = self.call_key(1).task;
        match self.store.rows.get(&root::Key::Core(core::Key::Tasks(tasks::Key::Live(task)))) {
            Some(root::Record::Core(core::Record::Tasks(tasks::Stored::Live(row)))) => row.numbers.spent,
            row => panic!("live caller's stored balance: {row:?}"),
        }
    }

    /// Expense debited to the accepting person's finite pool.
    #[must_use]
    pub fn holder_spent(&self) -> u64 {
        let funder = tasks::Funder::Pool { project: 1, person: 1, period: 1 };
        let live = root::Key::Core(core::Key::Tasks(tasks::Key::Ledger(funder)));
        let past = root::Key::Core(core::Key::Tasks(tasks::Key::ClosedLedger(funder)));
        match self.store.rows.get(&live).or_else(|| self.store.rows.get(&past)) {
            Some(root::Record::Core(core::Record::Tasks(
                tasks::Stored::Ledger(row) | tasks::Stored::ClosedLedger(row),
            ))) => row.numbers.spent,
            row => panic!("holder's stored balance: {row:?}"),
        }
    }

    /// Explicitly propose the connector payload instead of making it.
    pub fn propose(&mut self, completion: u32) -> u64 {
        self.send(root::Event::EffectCall {
            to: ReplyTo::new(Token::new(220)),
            key: self.call_key(completion),
            number: 1,
            effect: Self::effect(),
            deadline: Wall::from_nanos(self.wall + 1_000_000_000),
            proposal: Some(b"ask the holder".as_slice().into()),
        });
        match self.answers.last() {
            Some((_, _, core::CallPart::Proposed { proposal, .. })) => *proposal,
            answer => panic!("effect proposed: {answer:?}; {:?}", self.trace),
        }
    }

    /// The signed-in person decides the caller's proposal through people.
    pub fn decide_proposal(&mut self, proposal: u64, accept: bool, request: u8) {
        let decision = if accept {
            people::ProposalDecision::Accept
        } else {
            people::ProposalDecision::Reject { reason: b"not now".as_slice().into() }
        };
        self.send(root::Event::Core(core::Event::People(people::Event::Ask {
            models: Box::new([]),
            reply_to: ReplyTo::new(Token::new(u64::from(request) + 230)),
            sign_in: self.sign_in.expect("signed-in person"),
            key: [request; 16],
            ask: people::Ask::DecideProposal {
                project: 1,
                proposer: people::Proposer::Task(self.call_key(1).task),
                proposal,
                decision,
                running: None,
            },
        })));
    }

    /// Cross the call's absolute deadline while its committed write is pending.
    pub fn deadline(&mut self) {
        self.wall += 1_000_000_000;
        self.send(root::Event::Core(core::Event::EffectDeadline));
    }

    /// Deliver the system answers held past the caller's deadline.
    pub fn release_writes(&mut self) {
        self.hold_writes = false;
        for (number, call) in std::mem::take(&mut self.pending_writes) {
            let event = self.systems[usize::from(number - 1)].answer(call, Fault::None);
            self.events.push_back(root::Event::Connector { number, event: connector::Event::System(event) });
        }
        self.drain();
    }

    pub fn delegate(&mut self) {
        let assignment = self.assigned.as_ref().expect("assigned caller");
        let mut authority = assignment.run.authority.clone();
        authority.delegation = tasks::Delegation { kinds: Box::new([]), tasks: 0, depth: 0 };
        authority.budget.spend = 20;
        authority.tools = tasks::Tools(0);
        self.send(root::Event::Core(core::Event::DelegateValidated {
            to: ReplyTo::new(Token::new(201)),
            key: self.call_key(1),
            batch: Box::new([core::Delegate {
                deadline: jig_domain_tasks::Deadline::Named,
                label: 0,
                flags: jig_domain_tasks::Flags::PLAIN,
                tracked: None,
                budget: jig_domain_people::Budget::Named,
                executor: tasks::Executor::Procedure { connector: 1, code: 1 },
                spec: tasks::Spec {
                    words: b"make a checked effect".as_slice().into(),
                    parameters: Box::new([]),
                    inputs: Box::new([]),
                },
                contract: tasks::Contract::Report { words: 128 },
                authority,
                llm: None,
                symbolic_grants: Box::new([]),
                dependencies: Box::new([]),
                wake: tasks::WakePolicy::DEFAULT,
            }]),
            stubs: Box::new([]),
        }));
        assert!(self.procedure.is_some(), "delegated procedure admitted: {:?}", self.trace);
    }

    /// Read the observer's system, then give the idle procedure another step.
    pub fn observed(&mut self, state: u64) {
        self.systems[1].other_hand(&jig_test_connector_world::path(1, 1), state);
        let reads = std::mem::take(&mut self.pending_reads);
        for (number, call) in reads {
            let event = self.systems[usize::from(number - 1)].answer(call, Fault::None);
            self.events.push_back(root::Event::Connector { number, event: connector::Event::System(event) });
        }
        self.drain();
        if let Some(task) = self.procedure {
            self.send(root::Event::Core(core::Event::Tasks(tasks::Event::WakeProcedure { task })));
        }
    }
}

impl World {
    pub fn say(&mut self, key: u8) -> u64 {
        let mut text = [0; 4];
        let words = char::from(key).encode_utf8(&mut text);
        let task = self.assigned.as_ref().expect("assigned chat").task;
        self.send(root::Event::Core(core::Event::People(people::Event::Ask {
            models: Box::new([]),
            reply_to: ReplyTo::new(Token::new(3000 + u64::from(key))),
            sign_in: self.sign_in.expect("signed in"),
            key: [key; 16],
            ask: people::Ask::Say { project: 1, task, words: Box::from(words.as_bytes()) },
        })));
        self.people_answers
            .iter()
            .rev()
            .find_map(|reply| match reply {
                people::Reply::Outcome(people::Outcome::Said { message, .. }) => Some(*message),
                people::Reply::Signer(_)
                | people::Reply::Full { .. }
                | people::Reply::SignedIn { .. }
                | people::Reply::SignedOut
                | people::Reply::Outcome(_)
                | people::Reply::Refused(_)
                | people::Reply::Again(_) => None,
            })
            .expect("durable words")
    }

    pub fn turn(&mut self, number: u32, spent: u64, read: Option<u64>, body: &[u8]) {
        self.send_turn(number, spent, read, body, None, &[]);
    }

    pub fn window_turn(&mut self, number: u32, spent: u64, read: Option<u64>, body: &[u8]) {
        self.send_turn(number, spent, read, body, Some(true), &[]);
    }

    pub fn turn_refusing(&mut self, number: u32, spent: u64, read: Option<u64>, refused: &[u64], body: &[u8]) {
        self.send_turn(number, spent, read, body, None, refused);
    }

    fn send_turn(
        &mut self,
        number: u32,
        spent: u64,
        read: Option<u64>,
        body: &[u8],
        opens: Option<bool>,
        refused: &[u64],
    ) {
        self.last_read = read;
        let assignment = self.assigned.as_ref().expect("assigned chat");
        self.send(root::Event::Turn {
            opens_window: opens.unwrap_or(number == 1 && assignment.transcript.is_empty()),
            refused: refused.into(),
            channel: Token::new(7),
            task: assignment.task,
            attempt: assignment.attempt,
            turn: number,
            cumulative: spent,
            read,
            transcript: body.into(),
        });
    }

    pub fn end(&mut self, end: tasks::End, cumulative: u64) {
        let event = self.end_event(end, cumulative);
        self.send(event);
    }

    #[must_use]
    pub fn end_event(&self, end: tasks::End, cumulative: u64) -> root::Event {
        let assignment = self.assigned.as_ref().expect("assigned chat");
        root::Event::Answer {
            read: self.last_read,
            refused: Box::new([]),
            channel: Token::new(7),
            task: assignment.task,
            attempt: assignment.attempt,
            cumulative,
            end,
        }
    }

    pub fn host_call(&mut self, name: &[u8], tool: &[u8], input: &[u8], writes: bool) -> Token {
        self.relay_call(name, tool, input, writes);
        self.host_calls.last().expect("fleet authenticated call").0
    }
}

impl World {
    pub fn retry_due(&mut self) {
        self.wall += 1_000_000_000;
        self.send(root::Event::Timer(core::Timer::Tasks));
    }
}

impl World {
    #[must_use]
    pub fn wall_time(&self) -> Wall {
        Wall::from_nanos(self.wall)
    }

    pub fn finish_lookups(&mut self) {
        self.hold_lookups = false;
        for (number, call) in std::mem::take(&mut self.pending_lookups) {
            let event = self.systems[usize::from(number - 1)].answer(call, Fault::None);
            self.events.push_back(root::Event::Connector { number, event: connector::Event::System(event) });
        }
        self.drain();
    }

    pub fn waiting_judge(&mut self) {
        self.send(root::Event::Connector {
            number: 2,
            event: connector::Event::System(connector::SystemEvent::Fact {
                resource: jig_test_connector_world::path(1, 1),
                fact: connector::Fact { state: None, observed: Wall::from_nanos(self.wall), pending: true },
                origin: connector::Origin::Other,
            }),
        });
    }
}

mod stopping;
