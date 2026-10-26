//! Conformance over decoded client/agent bytes and independently read durable rows.
pub(super) mod agent;
use crate::world::Proc;
use jig_channel_world::world::store::Store;
use jig_client_domain::contract as client;
use jig_client_protocol_world::native::Received;
use jig_conformance::referee as rules;
use jig_domain_core as core;
use jig_test_domain as root;
use skein_lib::Time;
use skein_shell::Host;
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Scenario {
    Answer,
    MidRun,
    Restart,
    Fault { cut: skein_world::Cut, before: u32 },
}
/// Read-only evidence captured before shutdown changes the task.
#[derive(Clone, Debug)]
pub struct ChatOutcome {
    pub calls: usize,
    pub claims: u64,
    pub tries: jig_domain_tasks::Tries,
    pub phase: jig_domain_tasks::Phase,
    pub last_reply: u64,
    pub reply: Vec<u8>,
    pub mode: Option<smith_charter::Mode>,
    pub idle_elapsed: bool,
}
/// The world's external observer survives every service lifetime.
pub struct Referee {
    chat: crate::fixtures::Chat,
    chat_outcome: Rc<RefCell<Option<ChatOutcome>>>,
    idle_until: Option<Time>,
    handed: bool,
    done: bool,
    scenario: Scenario,
    controls: skein_world::Controls,
    store: Rc<RefCell<Store>>,
    rules: Rc<RefCell<rules::Referee>>,
    applied: u64,
    clients: BTreeMap<usize, usize>,
    keys: BTreeSet<usize>,
    ready: BTreeSet<usize>,
    agent_seen: usize,
    commits_seen: usize,
    rows: BTreeMap<root::Key, root::Record>,
    requests: BTreeMap<(usize, skein_lib::Token), [u8; 16]>,
    parties: BTreeMap<usize, u64>,
    status: Vec<(u32, u32, bool)>,
    attached: bool,
    cold: Option<()>,
    drained: Option<()>,
    pub(super) restarts: Rc<RefCell<u32>>,
}
impl Referee {
    pub(super) fn new(
        scenario: Scenario,
        controls: skein_world::Controls,
        mut store: Store,
        rules: Rc<RefCell<rules::Referee>>,
        restarts: Rc<RefCell<u32>>,
        chat: crate::fixtures::Chat,
        chat_outcome: Rc<RefCell<Option<ChatOutcome>>>,
    ) -> Self {
        let applied = store.applied;
        let rows = store.rows.clone();
        if applied > 0 {
            rules.borrow_mut().observe(0, rules::Observed::Restart).expect("new service lifetime");
            store.recovering();
        }
        Self {
            chat,
            chat_outcome,
            idle_until: None,
            handed: false,
            done: false,
            scenario,
            controls,
            store: Rc::new(RefCell::new(store)),
            rules,
            applied,
            clients: BTreeMap::new(),
            keys: BTreeSet::new(),
            ready: BTreeSet::new(),
            agent_seen: 0,
            commits_seen: 0,
            rows,
            requests: BTreeMap::new(),
            parties: BTreeMap::new(),
            status: Vec::new(),
            attached: false,
            cold: (applied > 0).then_some(()),
            drained: None,
            restarts,
        }
    }
    fn expectation_deadline(&self) -> Time {
        Time::from_nanos(if self.chat == crate::fixtures::Chat::Finite { 10_000_000_000 } else { 60_000_000_000 })
    }
    fn accounts(&mut self, procs: &mut [Proc]) {
        for (index, proc) in procs.iter_mut().enumerate() {
            if let Proc::Client(client) = proc {
                if client.account_sign_in_ready() && self.keys.insert(index) {
                    client.account_request(jig_client::accounts::add_key(
                        jig_client::Key { asker: jig_client::Asker(77), number: 1 },
                        [61; 16],
                        jig_client::contract::AccountHolder::Deployment,
                        1,
                        None,
                        skein_lib::Secret::new(jig_channel_world::fixtures::TOKEN.into()),
                    ));
                }
                if !self.ready.contains(&index)
                    && client.received.iter().any(|r| {
                        matches!(
                            r,
                            Received::Answer {
                                answer: jig_client::contract::Answer::Done(jig_client::contract::Outcome::Owner(
                                    jig_client::contract::OwnerOutcome::AccountKeyKept { .. }
                                )),
                                ..
                            }
                        )
                    })
                {
                    self.ready.insert(index);
                    client.resume_after_account_setup();
                }
            }
        }
    }
    fn text_reached(&mut self, now: Time, service: &crate::world::ServiceHost, other: &[Proc]) -> bool {
        use crate::fixtures::Chat;
        use jig_domain_tasks::{Active, Phase, Stored};
        let calls = other
            .iter()
            .filter_map(|proc| {
                if let Proc::Peer(peer) = proc {
                    Some(
                        peer.observations()
                            .iter()
                            .filter(|seen| matches!(seen, skein_fake_peers::llm::Observation::Query { .. }))
                            .count(),
                    )
                } else {
                    None
                }
            })
            .sum();
        let store = self.store.borrow();
        let task = store.rows.values().find_map(|row| {
            if let root::Record::Core(core::Record::Tasks(Stored::Live(task))) = row { Some(task) } else { None }
        });
        let Some(task) = task else { return false };
        let parked = task.phase == Phase::Active(Active::Idle);
        let backing_off = matches!(task.phase, Phase::Active(Active::BackingOff { .. }));
        if (self.chat == Chat::OpenText && parked || self.chat == Chat::FiniteText && backing_off)
            && self.chat_outcome.borrow().is_none()
        {
            let mut reply = Vec::new();
            for row in store.rows.values() {
                if let root::Record::Core(core::Record::Core(core::CoreRecord::Turn(turn))) = row {
                    let config = jig_channel_world::fixtures::agent_configuration();
                    let decoded = smith_protocol_channel::decode_turn(
                        &turn.transcript,
                        turn.turn,
                        &config.limits.channel.transcript,
                        &config.channel_endpoints,
                    )
                    .expect("committed turn bytes");
                    for message in &decoded.messages {
                        if message.role == smith_domain::llm::Role::Assistant {
                            for block in &message.content {
                                if let smith_domain::session::llm::Block::Text { text, .. } = block {
                                    reply.extend_from_slice(text);
                                }
                            }
                        }
                    }
                }
            }
            *self.chat_outcome.borrow_mut() = Some(ChatOutcome {
                calls,
                claims: task.attempt,
                tries: task.tries,
                phase: task.phase.clone(),
                last_reply: task.last_reply,
                reply,
                mode: service.wire.mode.clone(),
                idle_elapsed: false,
            });
            self.idle_until = Some(now.saturating_add(crate::fixtures::idle_interval()));
        }
        let mut outcome = self.chat_outcome.borrow_mut();
        let Some(outcome) = outcome.as_mut() else { return false };
        outcome.claims = task.attempt;
        outcome.calls = calls;
        if self.chat == Chat::OpenText {
            assert_eq!(calls, 1, "an idle Open chat makes no further LLM call");
            assert_eq!(task.attempt, 1, "an idle Open chat takes no further claim");
            outcome.idle_elapsed = self.idle_until.is_some_and(|until| now >= until);
            outcome.idle_elapsed && parked
        } else {
            task.attempt == 2
        }
    }
    fn say(&mut self, now: Time, event: rules::Observed) {
        self.rules
            .borrow_mut()
            .observe(now.as_nanos(), event)
            .expect("one-process conformance over bytes and durable rows");
    }
}
pub(super) fn conformance() -> Rc<RefCell<rules::Referee>> {
    Rc::new(RefCell::new(rules::Referee::new(policy())))
}
fn policy() -> rules::Policy {
    let scope = rules::Scope {
        budget: 100,
        executors: BTreeSet::from([(0, 1)]),
        descendants: 8,
        depth: 8,
        ..rules::Scope::default()
    };
    rules::Policy {
        deployment: scope.clone(),
        projects: BTreeMap::from([(1, scope.clone())]),
        people: BTreeMap::from([((1, 1), scope)]),
        projections: BTreeMap::new(),
        effect_accepters: BTreeSet::new(),
        requirements: vec![],
        implications: BTreeSet::new(),
        kinds: BTreeMap::new(),
        owned: vec![],
        participating: BTreeSet::new(),
        mechanics: BTreeSet::new(),
        delivery_bound: 10_000_000_000,
        uncertainty_bound: 10_000_000_000,
        story_steps: 100_000,
    }
}
pub(super) fn terminals(store: &Store) -> u32 {
    store
        .rows
        .values()
        .filter(|row| {
            matches!(row,
                root::Record::Core(core::Record::Core(core::CoreRecord::Terminal(terminal)))
                if matches!(terminal.end, jig_domain_tasks::End::Finished { .. })
            )
        })
        .count()
        .try_into()
        .expect("fixture terminals")
}
pub(super) fn settled(store: &Store) -> u32 {
    store
        .rows
        .values()
        .filter(|row| {
            matches!(row,
                root::Record::Core(core::Record::Core(core::CoreRecord::Terminal(terminal)))
                if matches!(terminal.end, jig_domain_tasks::End::Failed(jig_domain_tasks::Class::Lost))
            )
        })
        .count()
        .try_into()
        .expect("settled worker claims")
}
fn snapshot(rows: &BTreeMap<root::Key, root::Record>) -> rules::Snapshot {
    let mut snapshot = rules::Snapshot::default();
    for row in rows.values() {
        if let root::Record::Core(core::Record::Tasks(
            jig_domain_tasks::Stored::Live(task) | jig_domain_tasks::Stored::Ended(task),
        )) = row
        {
            snapshot.tasks.insert(task.number, jig_core_world::observations::task(task));
            snapshot.receipts.insert(rules::Receipt::Task(task.number));
            for word in &task.inbox {
                snapshot.receipts.insert(rules::Receipt::Word(task.number, word.number));
            }
        }
        if let root::Record::Core(core::Record::Core(core::CoreRecord::RunProof(proof))) = row {
            snapshot.receipts.insert(rules::Receipt::Claim(proof.task, proof.attempt));
        }
        if let root::Record::Core(core::Record::Core(core::CoreRecord::Turn(turn))) = row {
            snapshot.receipts.insert(rules::Receipt::Turn(turn.task, turn.attempt, turn.turn));
        }
        if let root::Record::Core(core::Record::Core(core::CoreRecord::Terminal(end))) = row {
            snapshot.receipts.insert(rules::Receipt::Terminal(end.task, end.attempt));
        }
        if let root::Record::Core(core::Record::People(
            jig_domain_people::Stored::ReadConversation { party, task, through }
            | jig_domain_people::Stored::EndedRead { party, task, through },
        )) = row
        {
            snapshot.read_positions.insert((*party, *task), *through);
        }
        if let root::Record::Core(core::Record::People(jig_domain_people::Stored::SignIn { number, .. })) = row {
            snapshot.receipts.insert(rules::Receipt::SignIn(*number));
        }
        if let root::Record::Core(core::Record::People(jig_domain_people::Stored::Answer { key, .. })) = row {
            snapshot.receipts.insert(rules::Receipt::Party(key.person, key.key));
        }
    }
    snapshot
}
fn received(seen: &Received, store: &Store) -> Vec<rules::Receipt> {
    if let Received::Answer {
        answer: client::Answer::Done(client::Outcome::Owner(client::OwnerOutcome::Created { first, last, .. })),
        ..
    } = seen
    {
        return (*first..=*last).map(rules::Receipt::Task).collect();
    }
    if let Received::SignedIn { person } = seen {
        let receipts: Vec<_> = store
            .rows
            .values()
            .filter_map(|row| match row {
                root::Record::Core(core::Record::People(jig_domain_people::Stored::SignIn {
                    number,
                    person: owner,
                    ..
                })) if owner == person => Some(rules::Receipt::SignIn(*number)),
                root::Record::Core(_) | root::Record::Connector { .. } => None,
            })
            .collect();
        assert!(!receipts.is_empty(), "signed-in response preceded a durable sign-in");
        return receipts;
    }
    if let Received::Delivery(client::Delivery::RunState(run)) = seen
        && run.committed_turn > 0
    {
        return vec![rules::Receipt::Turn(run.task, run.attempt.expect("committed run"), run.committed_turn)];
    }
    if let Received::Read(client::ReadResult::Value(client::ReadValue::Activity(page))) = seen {
        return page
            .rows
            .iter()
            .filter_map(|row| {
                if let client::Activity::Owner { source, .. } = &row.event {
                    row.event.committed_turn().map(|turn| rules::Receipt::Turn(source.task, turn.attempt, turn.number))
                } else {
                    None
                }
            })
            .collect();
    }
    Vec::new()
}
impl skein_world::Referee<Proc> for Referee {
    fn act(&mut self, now: Time, procs: &mut [Proc]) {
        self.accounts(procs);
        let clients_done = procs
            .iter()
            .filter_map(|proc| match proc {
                Proc::Client(client) => Some(client.is_empty()),
                Proc::Service(_) | Proc::Agent { .. } | Proc::Peer(_) => None,
            })
            .all(|done| done);
        self.status = procs
            .iter()
            .filter_map(|proc| match proc {
                Proc::Client(client) => Some((client.signed_in, client.chats_made, client.is_empty())),
                Proc::Service(_) | Proc::Agent { .. } | Proc::Peer(_) => None,
            })
            .collect();
        let agent_started = procs.iter().any(|proc| matches!(proc, Proc::Agent { .. }));
        let (services, other) = procs.split_at_mut(1);
        let Proc::Service(service) = &mut services[0] else { panic!("one service process") };
        if !service.attached {
            service.store = self.store.clone();
            service.attached = true;
            if self.attached {
                self.handed = false;
                self.agent_seen = 0;
                self.commits_seen = 0;
                self.say(now, rules::Observed::Restart);
                self.store.borrow_mut().recovering();
                self.cold = Some(());
                *self.restarts.borrow_mut() += 1;
                if matches!(self.scenario, Scenario::Fault { cut: skein_world::Cut::PowerLoss, .. }) {
                    self.keys.clear();
                    self.ready.clear();
                    self.clients.clear();
                    self.requests.clear();
                    self.parties.clear();
                }
            }
            self.attached = true;
        }
        if !self.handed
            && (self.cold.is_none() || !self.attached)
            && let Some(link) = service.service.links().pop()
        {
            for proc in other.iter_mut() {
                if let Proc::Client(client) = proc
                    && client.signed_in == 0
                {
                    client.sign_in(&link).expect("printed link");
                }
            }
            self.handed = true;
        }
        let reached = match self.scenario {
            Scenario::Answer => {
                clients_done
                    && if self.chat == crate::fixtures::Chat::Finite {
                        terminals(&self.store.borrow()) > 0
                    } else {
                        self.text_reached(now, service, other)
                    }
            }
            Scenario::MidRun => {
                clients_done
                    && agent_started
                    && self
                        .store
                        .borrow()
                        .rows
                        .values()
                        .any(|row| matches!(row, root::Record::Core(core::Record::Core(core::CoreRecord::Turn(_)))))
            }
            Scenario::Restart => settled(&self.store.borrow()) > 0,
            Scenario::Fault { .. } => *self.restarts.borrow() > 0 && clients_done && self.cold.is_none(),
        };
        if !self.done && reached {
            self.controls.signal(0, skein_io::kernel::ServiceSignal::Terminate);
            self.done = true;
        }
        if self.done {
            for proc in other {
                if let Proc::Peer(peer) = proc {
                    peer.shutdown();
                }
            }
        }
    }
    fn observe(&mut self, now: Time, procs: &[Proc]) {
        let Proc::Service(service) = &procs[0] else { panic!("one service process") };
        if !service.attached {
            return;
        }
        let durable = service.store.borrow();
        for commit in &service.commits[self.commits_seen..] {
            if commit.number <= self.applied {
                continue;
            }
            for write in &commit.writes {
                match write {
                    root::Write::Wires => {}
                    root::Write::CopySettingsRevision(number) => {
                        use jig_domain_core as core;
                        let key = root::Key::Core(core::Key::Core(core::CoreKey::Settings(core::settings::Key::Held(
                            *number,
                        ))));
                        let root::Record::Core(core::Record::Core(core::CoreRecord::Settings(
                            core::settings::Record::Revision { revision, .. },
                        ))) = self.rows.get(&key).expect("held settings revision").clone()
                        else {
                            panic!("revision row")
                        };
                        let row = root::Record::Core(core::Record::Core(core::CoreRecord::Settings(
                            core::settings::Record::Revision { history: true, revision },
                        )));
                        self.rows.insert(row.key(), row);
                    }
                    root::Write::Save(record) => {
                        self.rows.insert(record.key(), record.clone());
                    }
                    root::Write::Erase(key) => {
                        self.rows.remove(key);
                    }
                }
            }
            self.applied = commit.number;
            self.say(now, rules::Observed::Durable { number: commit.number, snapshot: snapshot(&self.rows) });
        }
        self.commits_seen = service.commits.len();
        if self.cold.is_some() && durable.applied >= self.applied && durable.applied > 0 {
            self.say(now, rules::Observed::ColdStore { receipts: snapshot(&durable.rows).receipts });
            self.cold = None;
        }
        for event in &service.wire.events[self.agent_seen..] {
            self.say(now, event.clone());
        }
        self.agent_seen = service.wire.events.len();
        for (index, proc) in procs.iter().enumerate() {
            if let Proc::Client(client) = proc {
                let offset = self.clients.get(&index).copied().unwrap_or(0);
                for event in &client.received[offset..] {
                    if let Received::Sent { attempt, key } = event {
                        self.requests.insert((index, *attempt), *key);
                    }
                    if let Received::SignedIn { person } = event {
                        self.parties.insert(index, *person);
                    }
                    let mut requires = received(event, &durable);
                    if let Received::Answer { attempt, .. } = event {
                        requires.push(rules::Receipt::Party(
                            *self.parties.get(&index).expect("signed-in native client"),
                            *self.requests.get(&(index, *attempt)).expect("the answered request's actual key"),
                        ));
                    }
                    if !requires.is_empty() {
                        self.say(now, rules::Observed::Released { requires });
                    }
                }
                self.clients.insert(index, client.received.len());
            }
        }
        if self.done && self.drained.is_none() && service.is_empty() {
            assert!(durable.pending.is_empty(), "a drained shutdown leaves no commit waiting");
            self.say(now, rules::Observed::Drained { at: self.applied });
            self.drained = Some(());
        }
        self.say(now, rules::Observed::Tick);
    }
    fn next_deadline(&self) -> Option<Time> {
        if self.done {
            None
        } else {
            Some(
                self.idle_until
                    .filter(|_| self.chat == crate::fixtures::Chat::OpenText)
                    .unwrap_or(self.expectation_deadline()),
            )
        }
    }
    fn overdue(&self, now: Time) -> Option<String> {
        if !self.done && now >= self.expectation_deadline() {
            Some(format!(
                "one-process expectation not met: restarts{} link{}, finished{} lost{}, durable{}, clients{:?}",
                *self.restarts.borrow(),
                self.handed,
                terminals(&self.store.borrow()),
                settled(&self.store.borrow()),
                self.store.borrow().applied,
                self.status
            ))
        } else {
            None
        }
    }
    fn passed(&self) -> bool {
        self.done
    }
}
