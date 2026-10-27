//! Opaque shell token routes and the finite Ops protocol adapter, beside the shared world.
#![allow(clippy::wildcard_imports, reason = "adapter exhaustively translates owner boundary vocabularies")]
use super::{Document, Ops, OpsScript, Subject};
use jig_client_domain::contract::*;
use jig_client_scripted_engine::{self as engine, ApplicationScope, Envelope, Up};
use jig_client_world::{harness::*, observe};
use jig_ops_client_domain::{self as domain, Event, Request, infrastructure as infra, observability as obs};
use jig_ops_domain_connectors::{Connector, number};
use sha2::{Digest, Sha256};
use skein_lib::{RequestEnvelope, Time, Token, Wall};
use skein_world::domain::Ledger;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Channel {
    Jig,
    Observability,
    Infrastructure,
}
#[derive(Clone, Copy)]
struct Route {
    client: ClientId,
    local: Token,
    channel: Channel,
}
/// Scripted jig truth and Ops documents with one shell route per operation lifetime.
pub struct Scripted {
    pub engine: engine::Engine<OpsScript>,
    routes: BTreeMap<Token, Route>,
    parties: BTreeMap<ClientId, u64>,
    hellos: BTreeSet<ClientId>,
    shell: Vec<(ClientId, Event)>,
    ledger: Ledger<Token, ()>,
    serial: u64,
    committed: usize,
    observed: Vec<Observed>,
    last_attempt: Option<Token>,
    last_watch: Option<Token>,
    last_object: Option<ObjectKey>,
    recovery: Option<Time>,
    sign_ins: Vec<u64>,
    pending_forget: Vec<ObjectKey>,
}
impl Scripted {
    /// Installs owner supplied truth and bounded application documents.
    #[must_use]
    pub fn new(script: engine::Script, seed: u64, ops: OpsScript) -> Self {
        let mut observed = vec![];
        for source in &script.truth.sources {
            source_facts(source, 0, &mut observed);
        }
        Self {
            engine: engine::Engine::with_application(script, seed, ops).expect("Ops fixture admitted"),
            routes: BTreeMap::new(),
            parties: BTreeMap::new(),
            hellos: BTreeSet::new(),
            shell: Vec::new(),
            ledger: Ledger::new("Ops shell operation"),
            serial: 0,
            committed: 0,
            observed,
            last_attempt: None,
            last_watch: None,
            last_object: None,
            recovery: None,
            sign_ins: vec![],
            pending_forget: vec![],
        }
    }
    fn forget(&mut self, now: Time, wall: Wall, key: ObjectKey) {
        self.engine.act(now, wall, engine::Change::Forget { key }).expect("owner object disappearance");
        for mut source in self.engine.truth().sources.clone() {
            if let engine::Rows::Inbox(rows) = &mut source.rows {
                let before = rows.len();
                rows.retain(|row| row.object != key);
                if rows.len() != before {
                    self.engine.act(now, wall, engine::Change::Source(source)).expect("owner listing removal");
                }
            }
        }
    }
    fn open(&mut self, client: ClientId, local: Token, channel: Channel) -> Token {
        self.serial = self.serial.checked_add(1).expect("finite shell route identities");
        let token = Token::new(self.serial);
        self.ledger.open(token, ());
        assert!(self.routes.insert(token, Route { client, local, channel }).is_none());
        token
    }
    fn token(&self, client: ClientId, local: Token, channel: Channel) -> Option<Token> {
        self.routes.iter().find_map(|(token, route)| {
            (route.client == client && route.local == local && route.channel == channel).then_some(*token)
        })
    }
    fn recover(&mut self, now: Time, wall: Wall) {
        if self.recovery.is_some_and(|at| now >= at) {
            self.recovery = None;
            self.observed.push(Observed::Reachable { live: true });
            for key in std::mem::take(&mut self.pending_forget) {
                self.forget(now, wall, key);
            }
            for party in std::mem::take(&mut self.sign_ins) {
                self.engine.act(now, wall, engine::Change::SignIn { sign_in: party, party }).expect("queued sign in");
            }
        }
    }
}
fn envelope(envelope: Envelope) -> RequestEnvelope {
    match envelope {
        Envelope::Final => RequestEnvelope::Final,
        Envelope::Again => RequestEnvelope::Again,
        Envelope::Lost => RequestEnvelope::Lost,
        Envelope::SignedOut => RequestEnvelope::SignedOut,
    }
}
fn subject_scope(subject: &Subject) -> ApplicationScope {
    match subject {
        Subject::Observability(obs::Subject { scope: obs::Scope::Project(project), .. })
        | Subject::Infrastructure(infra::Subject { scope: infra::Scope::Project(project), .. }) => {
            ApplicationScope::Project(*project)
        }
        Subject::Observability(obs::Subject { scope: obs::Scope::Deployment, .. })
        | Subject::Infrastructure(infra::Subject { scope: infra::Scope::Deployment, .. }) => {
            ApplicationScope::Deployment
        }
    }
}
fn family_subject(record: &jig_client::Family, scope: jig_client::contract::Scope) -> (Channel, Subject) {
    match record.family {
        1 => (
            Channel::Observability,
            Subject::Observability(
                domain::wire::observability::subject_value(&record.bytes, scope).expect("whole incident subject"),
            ),
        ),
        2 => (
            Channel::Infrastructure,
            Subject::Infrastructure(
                domain::wire::infrastructure::subject_value(&record.bytes, scope).expect("whole environment subject"),
            ),
        ),
        _ => panic!("declared family"),
    }
}
fn watch(channel: Channel, local: Token, event: WatchEvent) -> Event {
    match channel {
        Channel::Jig => {
            Event::Client { event: jig_client::Event::Up { up: jig_client::Up::Watched { watch: local, event } } }
        }
        Channel::Observability => Event::Observability(obs::Event::Watch {
            watch: local,
            event: match event {
                WatchEvent::Opened => obs::WatchEvent::Opened,
                WatchEvent::Alive => obs::WatchEvent::Alive,
                WatchEvent::Behind | WatchEvent::Delivered(_) => unreachable!("Ops docs use Up::App"),
            },
        }),
        Channel::Infrastructure => Event::Infrastructure(infra::Event::Watch {
            watch: local,
            event: match event {
                WatchEvent::Opened => infra::WatchEvent::Opened,
                WatchEvent::Alive => infra::WatchEvent::Alive,
                WatchEvent::Behind | WatchEvent::Delivered(_) => unreachable!("Ops docs use Up::App"),
            },
        }),
    }
}
fn end(channel: Channel, local: Token, why: WatchEnd) -> Event {
    match channel {
        Channel::Jig => {
            Event::Client { event: jig_client::Event::Up { up: jig_client::Up::WatchEnded { watch: local, end: why } } }
        }
        Channel::Observability => Event::Observability(obs::Event::Ended {
            watch: local,
            end: match why {
                WatchEnd::Closed => obs::WatchEnd::Closed,
                WatchEnd::SignedOut => obs::WatchEnd::SignedOut,
                WatchEnd::NotServed
                | WatchEnd::Dropped
                | WatchEnd::Unknown
                | WatchEnd::NotAllowed
                | WatchEnd::Busy
                | WatchEnd::SubjectEnded
                | WatchEnd::Stopping => obs::WatchEnd::Dropped,
            },
        }),
        Channel::Infrastructure => Event::Infrastructure(infra::Event::Ended {
            watch: local,
            end: match why {
                WatchEnd::Closed => infra::WatchEnd::Closed,
                WatchEnd::SignedOut => infra::WatchEnd::SignedOut,
                WatchEnd::NotServed
                | WatchEnd::Dropped
                | WatchEnd::Unknown
                | WatchEnd::NotAllowed
                | WatchEnd::Busy
                | WatchEnd::SubjectEnded
                | WatchEnd::Stopping => infra::WatchEnd::Dropped,
            },
        }),
    }
}
impl Front<Ops> for Scripted {
    fn bind(&mut self, client: ClientId, party: u64) {
        self.parties.insert(client, party);
        self.hellos.insert(client);
        self.shell.push((client, Event::Client { event: jig_client::Event::Up { up: jig_client::Up::SignedIn } }));
    }
    fn wire(&mut self, client: ClientId, now: Time, wall: Wall, request: Request) {
        let Request::Client { request: jig_client::Request::Down { down } } = request else {
            panic!("unlinked client transport")
        };
        let party = self.parties[&client];
        match down {
            jig_client::Down::Send { attempt, key, request, value } => {
                assert!(value.is_none(), "the ops engine carries no value");
                let channel = match &request {
                    jig_client::Ask::Jig(_) => Channel::Jig,
                    jig_client::Ask::Family { .. } => Channel::Infrastructure,
                };
                let attempt = self.open(client, attempt, channel);
                self.last_attempt = Some(attempt);
                match request {
                    jig_client::Ask::Jig(ask) => {
                        self.engine.send(now, wall, party, key, attempt, *ask).expect("jig send admitted");
                    }
                    jig_client::Ask::Family { record, .. } => {
                        let ask = domain::wire::ask_value(&record.bytes).expect("whole infrastructure request");
                        let infra::Ask::MoveEnd { project, environment, revision, until } = &ask else {
                            panic!("native environments use joint front")
                        };
                        let mut canonical = Sha256::new();
                        canonical.update(b"Ops.MoveEnd.v1");
                        canonical.update(project.to_be_bytes());
                        canonical.update(environment.to_be_bytes());
                        canonical.update(revision.value.to_be_bytes());
                        canonical.update(until.as_nanos().to_be_bytes());
                        self.engine
                            .send_application(
                                now,
                                wall,
                                party,
                                key,
                                attempt,
                                number(Connector::Infrastructure),
                                *project,
                                canonical.finalize().into(),
                                ask,
                            )
                            .expect("environment send admitted");
                    }
                }
            }
            jig_client::Down::Read { read, what: jig_client::Readable::Jig(what) } => {
                self.last_object = self.read_object(&what);
                let token = self.open(client, read, Channel::Jig);
                self.engine.read(now, party, token, what).expect("jig read admitted");
            }
            jig_client::Down::Open { watch, subject: jig_client::Watchable::Jig(subject) } => {
                let token = self.open(client, watch, Channel::Jig);
                self.last_watch = Some(token);
                self.engine.open(now, party, token, subject).expect("jig watch admitted");
            }
            jig_client::Down::Continue { watch, subject } => {
                let token = self.token(client, watch, Channel::Jig).expect("live run route");
                let SubjectRun { turn, offset } = run_position(&subject);
                self.engine.continue_run(token, turn, offset).expect("continue live run");
            }
            jig_client::Down::Close { watch } => {
                if let Some(token) = self
                    .routes
                    .iter()
                    .find_map(|(token, route)| (route.client == client && route.local == watch).then_some(*token))
                {
                    self.engine.close(now, token);
                }
            }
            jig_client::Down::Read { read, what: jig_client::Readable::Family { scope, record, .. } } => {
                let (channel, subject) = family_subject(&record, scope);
                let token = self.open(client, read, channel);
                self.engine
                    .read_scoped_application(now, party, token, record.family, subject_scope(&subject), &subject)
                    .expect("family read admitted");
            }
            jig_client::Down::Open { watch, subject: jig_client::Watchable::Family { scope, record, .. } } => {
                let (channel, subject) = family_subject(&record, scope);
                let token = self.open(client, watch, channel);
                self.last_watch = Some(token);
                self.engine
                    .open_scoped_application(now, party, token, record.family, subject_scope(&subject), subject)
                    .expect("family watch admitted");
            }
            jig_client::Down::Save { .. } | jig_client::Down::Erase { .. } => panic!("stores remain shell duties"),
        }
    }
    fn poll(&mut self, now: Time, wall: Wall) -> Vec<(ClientId, Event)> {
        self.recover(now, wall);
        let mut delivered: Vec<_> = std::mem::take(&mut self.hellos)
            .into_iter()
            .map(|client| {
                (
                    client,
                    Event::Client {
                        event: jig_client::Event::Up {
                            up: jig_client::Up::Served { served: self.engine.versions().contains(19), versions: None },
                        },
                    },
                )
            })
            .collect();
        delivered.append(&mut self.shell);
        for up in self.engine.poll(now) {
            let token = match &up {
                Up::Answer { attempt, .. } | Up::ApplicationAnswer { attempt, .. } => *attempt,
                Up::Read { token, .. }
                | Up::Watch { token, .. }
                | Up::ApplicationRead { token, .. }
                | Up::Ended { token, .. } => *token,
                Up::App { watch, .. } => *watch,
            };
            let terminal = matches!(
                &up,
                Up::Answer { .. }
                    | Up::ApplicationAnswer { .. }
                    | Up::Read { .. }
                    | Up::ApplicationRead { .. }
                    | Up::Ended { .. }
            );
            if terminal {
                self.ledger.end(token);
            }
            let Some(route) = self.routes.get(&token).copied() else { continue };
            if terminal {
                self.routes.remove(&token);
            }
            let local = route.local;
            let event = match up {
                Up::Answer { envelope: env, answer, .. } => {
                    super::callbacks::answer(local, envelope(env), answer.map(domain::Answered::Jig))
                }
                Up::ApplicationAnswer { envelope: env, answer, .. } => super::callbacks::answer(
                    local,
                    envelope(env),
                    answer.map(|result| {
                        domain::Answered::Infrastructure(match result {
                            Ok(answer) => answer,
                            Err(refusal) => infra::Answer::EntranceRefused(
                                jig_client_domain::contract::EntranceRefusal::new(refusal)
                                    .expect("bounded entrance facts"),
                            ),
                        })
                    }),
                ),
                Up::Read { terminal, .. } => Event::Client {
                    event: jig_client::Event::Up {
                        up: jig_client::Up::ReadDone { read: local, result: jig_client::Readout::Jig(terminal) },
                    },
                },
                Up::Watch { event, .. } => watch(route.channel, local, event),
                Up::App { document, .. } => match document {
                    Document::Observability(doc) => Event::Observability(obs::Event::Watch {
                        watch: local,
                        event: obs::WatchEvent::Delivered(Box::new(doc)),
                    }),
                    Document::Infrastructure(doc) => Event::Infrastructure(infra::Event::Watch {
                        watch: local,
                        event: infra::WatchEvent::Delivered(Box::new(doc)),
                    }),
                },
                Up::ApplicationRead { terminal, .. } => application_read(route.channel, local, terminal),
                Up::Ended { why, .. } => end(route.channel, local, why),
            };
            delivered.push((route.client, event));
        }
        super::callbacks::events(delivered)
    }
    fn next_due(&self) -> Option<Time> {
        if !self.hellos.is_empty() || !self.shell.is_empty() {
            return Some(Time::ZERO);
        }
        match (self.recovery, self.engine.next_due()) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, None) | (None, a) => a,
        }
    }
    fn observed(&mut self) -> Vec<Observed> {
        let mut out = std::mem::take(&mut self.observed);
        for commit in &self.engine.commits()[self.committed..] {
            out.push(Observed::Durable { commit: commit.number, at: commit.durable.expect("durable commit ledger") });
            if let (Some(party), Some(key)) = (commit.party, commit.key) {
                out.push(Observed::Made { party, key, commit: commit.number });
            }
            let mut facts = vec![];
            for change in &commit.changes {
                change_facts(change, commit.number, &mut facts);
            }
            facts.sort_by_key(|fact| !matches!(fact, Observed::Ended { .. }));
            out.extend(facts);
        }
        self.committed = self.engine.commits().len();
        out
    }
    fn fault(&mut self, now: Time, fault: FrontFault) {
        let fault = match fault {
            FrontFault::Busy => engine::Fault::Busy,
            FrontFault::NotReady => engine::Fault::NotReady,
            FrontFault::Slow { span } => engine::Fault::Slow { span },
            FrontFault::Restart { offline } => {
                self.observed.push(Observed::Reachable { live: false });
                self.recovery = Some(now.saturating_add(offline));
                engine::Fault::Restart { offline }
            }
            FrontFault::DropWatch => {
                let Some(watch) = self.last_watch else { return };
                engine::Fault::DropWatch { watch }
            }
            FrontFault::NoAnswer => {
                let Some(attempt) = self.last_attempt else { return };
                engine::Fault::Lose { attempt }
            }
            FrontFault::SignedOut => {
                let sign_in = *self.parties.values().next().expect("bound client");
                for (&client, &party) in &self.parties {
                    if party == sign_in {
                        self.observed.push(Observed::SignInEnded { client });
                    }
                }
                engine::Fault::SignOut { sign_in }
            }
            FrontFault::Versions => {
                self.hellos.extend(self.parties.keys().copied());
                engine::Fault::Versions(VersionRange { first: 2, last: 2 })
            }
            FrontFault::ObjectGone => {
                if let Some(key) = self.last_object {
                    if self.recovery.is_some_and(|at| now < at) {
                        self.pending_forget.push(key);
                    } else {
                        self.forget(now, Wall::from_nanos(now.as_nanos()), key);
                    }
                }
                return;
            }
        };
        self.engine.fault(now, fault);
    }
    fn reload(&mut self, client: ClientId, now: Time) {
        self.hellos.insert(client);
        self.shell.push((client, Event::Client { event: jig_client::Event::Up { up: jig_client::Up::SignedIn } }));
        let abandoned: Vec<_> =
            self.routes.iter().filter_map(|(token, route)| (route.client == client).then_some(*token)).collect();
        for token in abandoned {
            self.engine.close(now, token);
            self.routes.remove(&token);
        }
    }
    fn received(&mut self, client: ClientId, event: &Event) {
        let (local, channel) = match event {
            Event::Client {
                event: jig_client::Event::Up { up: jig_client::Up::Watched { watch, event: WatchEvent::Delivered(_) } },
            } => (*watch, Channel::Jig),
            Event::Client {
                event: jig_client::Event::Up { up: jig_client::Up::FamilyDelivered { watch, record } },
            } => (*watch, if record.family == 1 { Channel::Observability } else { Channel::Infrastructure }),
            Event::Start { .. }
            | Event::Went { .. }
            | Event::Act { .. }
            | Event::Client { .. }
            | Event::Jig(_)
            | Event::Infrastructure(_)
            | Event::Observability(_)
            | Event::SignedOut => return,
        };
        if let Some(token) = self.token(client, local, channel) {
            self.engine.taken(token);
        }
    }
    fn sign_out(&mut self, client: ClientId, now: Time, wall: Wall) {
        if self.recovery.is_some_and(|at| now < at) {
            self.shell.push((client, Event::Client { event: jig_client::Event::SignOutLost }));
            return;
        }
        let sign_in = self.parties[&client];
        for (&client, &party) in &self.parties {
            if party == sign_in {
                self.observed.push(Observed::SignInEnded { client });
            }
        }
        self.engine.act(now, wall, engine::Change::SignedOut { sign_in }).expect("sign out fits");
        self.shell.push((client, Event::SignedOut));
    }
    fn sign_in(&mut self, client: ClientId, now: Time, wall: Wall) {
        let party = self.parties[&client];
        if self.recovery.is_some_and(|at| now < at) {
            if !self.sign_ins.contains(&party) {
                self.sign_ins.push(party);
            }
        } else {
            self.engine.act(now, wall, engine::Change::SignIn { sign_in: party, party }).expect("sign in admitted");
        }
    }
}
struct SubjectRun {
    turn: Option<jig_client_domain::contract::ProvisionalTurnName>,
    offset: u32,
}
fn run_position(subject: &jig_client_domain::contract::Subject) -> SubjectRun {
    match subject {
        jig_client_domain::contract::Subject::Run { turn, offset, .. } => SubjectRun { turn: *turn, offset: *offset },
        jig_client_domain::contract::Subject::Me
        | jig_client_domain::contract::Subject::Task { .. }
        | jig_client_domain::contract::Subject::Tree { .. }
        | jig_client_domain::contract::Subject::Chips { .. }
        | jig_client_domain::contract::Subject::Inbox { .. }
        | jig_client_domain::contract::Subject::Tasks { .. }
        | jig_client_domain::contract::Subject::Settings
        | jig_client_domain::contract::Subject::Deployment
        | jig_client_domain::contract::Subject::Accounts => unreachable!("only run watches continue"),
    }
}
fn source_facts(source: &engine::Source, commit: u64, out: &mut Vec<Observed>) {
    if let (Read::Activity { task, .. }, engine::Rows::Activity(rows)) = (&source.read, &source.rows) {
        let mut messages = vec![];
        for row in rows {
            if let Activity::Words { message, words, .. } = &row.event {
                messages.push(*message);
                out.push(Observed::Kept { task: *task, message: *message, words: words.bytes().to_vec(), commit });
            }
        }
        out.push(Observed::Retained { task: *task, messages, commit });
    }
}
fn change_facts(change: &engine::Change, commit: u64, out: &mut Vec<Observed>) {
    match change {
        engine::Change::Source(source) => source_facts(source, commit, out),
        engine::Change::Forget { key } => out.push(Observed::Stopped { card: observe::key(*key), commit }),
        engine::Change::Object { key, value, .. } => {
            let terminal = match value {
                ReadValue::Task(task) => task.result.is_some(),
                ReadValue::Result(_) => true,
                ReadValue::Proposal(row) => row.ending.is_some(),
                ReadValue::Escalation(row) => row.ending.is_some(),
                ReadValue::Question(row) => row.ending.is_some(),
                ReadValue::PersonTask(row) => row.ending.is_some(),
                ReadValue::Tasks(_)
                | ReadValue::Me(_)
                | ReadValue::Ended(_)
                | ReadValue::OwnerEnded(_)
                | ReadValue::History(_)
                | ReadValue::Historical(_)
                | ReadValue::Activity(_)
                | ReadValue::Turn(_)
                | ReadValue::Inbox(_)
                | ReadValue::OwnerInbox(_)
                | ReadValue::OwnerProposal(_)
                | ReadValue::OwnerPersonTask(_)
                | ReadValue::OwnerEscalation(_)
                | ReadValue::Spend(_)
                | ReadValue::OwnerSpend(_)
                | ReadValue::Project(_)
                | ReadValue::OwnerProject(_)
                | ReadValue::Notes(_)
                | ReadValue::OwnerDeployment(_)
                | ReadValue::Settings(_)
                | ReadValue::Setting(_)
                | ReadValue::Revision(_)
                | ReadValue::Revisions(_)
                | ReadValue::Deployment(_) => false,
            };
            if terminal {
                out.push(Observed::Stopped { card: observe::key(*key), commit });
            }
            match value {
                ReadValue::Task(task) if task.result.is_some() => {
                    out.push(Observed::Ended { task: task.chip.task, commit });
                }
                ReadValue::Result(result) => out.push(Observed::Ended { task: result.task.task, commit }),
                ReadValue::Task(_)
                | ReadValue::Tasks(_)
                | ReadValue::Me(_)
                | ReadValue::Proposal(_)
                | ReadValue::Escalation(_)
                | ReadValue::Question(_)
                | ReadValue::PersonTask(_)
                | ReadValue::Ended(_)
                | ReadValue::OwnerEnded(_)
                | ReadValue::History(_)
                | ReadValue::Historical(_)
                | ReadValue::Activity(_)
                | ReadValue::Turn(_)
                | ReadValue::Inbox(_)
                | ReadValue::OwnerInbox(_)
                | ReadValue::OwnerProposal(_)
                | ReadValue::OwnerPersonTask(_)
                | ReadValue::OwnerEscalation(_)
                | ReadValue::Spend(_)
                | ReadValue::OwnerSpend(_)
                | ReadValue::Project(_)
                | ReadValue::OwnerProject(_)
                | ReadValue::Notes(_)
                | ReadValue::OwnerDeployment(_)
                | ReadValue::Settings(_)
                | ReadValue::Setting(_)
                | ReadValue::Revision(_)
                | ReadValue::Revisions(_)
                | ReadValue::Deployment(_) => {}
            }
        }
        engine::Change::Settings(_)
        | engine::Change::Setting(_)
        | engine::Change::Revision { .. }
        | engine::Change::Revisions { .. }
        | engine::Change::Request(_)
        | engine::Change::Me(_)
        | engine::Change::SignIn { .. }
        | engine::Change::SignedOut { .. }
        | engine::Change::OwnerProject(_)
        | engine::Change::Project(_)
        | engine::Change::Note(_)
        | engine::Change::DeleteNote { .. }
        | engine::Change::OwnerDeployment(_)
        | engine::Change::Deployment(_)
        | engine::Change::Tree(_)
        | engine::Change::Seen { .. } => {}
    }
}

impl Scripted {
    fn read_object(&self, what: &Read) -> Option<ObjectKey> {
        match what {
            Read::Task { task } => Some(ObjectKey::Task(*task)),
            Read::Proposal { proposal } => Some(ObjectKey::Proposal(*proposal)),
            Read::Escalation { task } => Some(ObjectKey::Escalation(*task)),
            Read::Question { task, message } => Some(ObjectKey::Question { task: *task, message: *message }),
            Read::PersonTask { task } => Some(ObjectKey::PersonTask(*task)),
            Read::Result { task } => Some(ObjectKey::Result(*task)),
            Read::Tasks { .. }
            | Read::Me
            | Read::Ended { .. }
            | Read::History { .. }
            | Read::Activity { .. }
            | Read::Turn { .. }
            | Read::Inbox { .. }
            | Read::Spend { .. }
            | Read::Project { .. }
            | Read::Notes { .. }
            | Read::Settings
            | Read::Setting(_)
            | Read::Revision(_)
            | Read::Revisions(_)
            | Read::Deployment => self.last_object,
        }
    }
}

fn application_read(channel: Channel, local: Token, terminal: std::result::Result<Document, ReadNone>) -> Event {
    match (channel, terminal) {
        (Channel::Observability, Ok(Document::Observability(doc))) => {
            Event::Observability(obs::Event::Read { read: local, result: obs::ReadResult::Value(Box::new(doc)) })
        }
        (Channel::Infrastructure, Ok(Document::Infrastructure(doc))) => {
            Event::Infrastructure(infra::Event::Read { read: local, result: infra::ReadResult::Value(Box::new(doc)) })
        }
        (Channel::Observability, Err(reason)) => Event::Observability(obs::Event::Read {
            read: local,
            result: match reason {
                ReadNone::SignedOut => obs::ReadResult::SignedOut,
                ReadNone::Busy | ReadNone::Unreachable => obs::ReadResult::Busy,
                ReadNone::Unknown
                | ReadNone::Refused
                | ReadNone::Gone
                | ReadNone::TooLinked { .. }
                | ReadNone::TooLarge { .. }
                | ReadNone::Malformed => obs::ReadResult::Unknown,
            },
        }),
        (Channel::Infrastructure, Err(reason)) => Event::Infrastructure(infra::Event::Read {
            read: local,
            result: match reason {
                ReadNone::SignedOut => infra::ReadResult::SignedOut,
                ReadNone::Busy | ReadNone::Unreachable => infra::ReadResult::Busy,
                ReadNone::Unknown
                | ReadNone::Refused
                | ReadNone::Gone
                | ReadNone::TooLinked { .. }
                | ReadNone::TooLarge { .. }
                | ReadNone::Malformed => infra::ReadResult::Unknown,
            },
        }),
        (Channel::Jig, _)
        | (Channel::Observability, Ok(Document::Infrastructure(_)))
        | (Channel::Infrastructure, Ok(Document::Observability(_))) => {
            panic!("application payload owner mismatch")
        }
    }
}
