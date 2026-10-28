//! Token routing between a client root and scripted engine. No root state is read.
#![allow(clippy::wildcard_imports, reason = "world adapters translate the complete owner vocabulary")]
use crate::{harness::*, test::Test};
use jig_client::{Ask, Down, Readable, Watchable};
use jig_client_domain::contract::*;
use jig_client_scripted_engine::{self as engine, Application, ApplicationValue, Decision, Envelope, Truth, Up};
use jig_client_test_domain::{self as domain, Event, Request, family, first};
use sha2::{Digest, Sha256};
use skein_lib::{RequestEnvelope, Time, Token, Wall};
use skein_world::domain::Ledger;
use std::collections::BTreeMap;
/// Scripted family documents with revision-naming decisions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Family {
    pub documents: BTreeMap<family::Subject, family::Document>,
}
impl Application for Family {
    type Ask = family::Ask;
    type Answer = family::Answer;
    type Subject = family::Subject;
    type Document = family::Document;
    fn decide(&mut self, _: &Truth, _: u64, ask: &Self::Ask, _: Wall) -> Decision<Self::Answer> {
        let answer = match ask {
            family::Ask::Touch { project, item, revision, parameters } => {
                let subject =
                    family::Subject { scope: family::Scope::Project(*project), kind: family::Kind::Item(*item) };
                match self.documents.get_mut(&subject) {
                    Some(family::Document::Item(doc)) if doc.revision == *revision => {
                        doc.revision = Revision { value: doc.revision.value + 1 };
                        if let Some(value) = parameters {
                            doc.value = *value;
                        }
                        family::Answer::Touched { item: *item, revision: doc.revision }
                    }
                    Some(doc) => match doc.revision() {
                        Some(revision) => family::Answer::Moved { item: *item, revision },
                        None => family::Answer::Connector(jig_client_test_domain::connector_values::ConnectorAnswer {
                            value: jig_client_test_domain::connector_values::ApplicationOutcome::Unknown,
                        }),
                    },
                    None => family::Answer::Gone,
                }
            }
            family::Ask::ChangeObject { project, object, revision, value } => {
                let subject =
                    family::Subject { scope: family::Scope::Project(*project), kind: family::Kind::Item(*object) };
                let outcome = match self.documents.get_mut(&subject) {
                    Some(family::Document::Connector(doc)) => match doc.value.as_mut() {
                        jig_client_test_domain::connector_values::ClientDocument::Object { object: number, now }
                            if number == object =>
                        {
                            if now.revision != revision.value {
                                jig_client_test_domain::connector_values::ApplicationOutcome::Moved {
                                    object: *object,
                                    now: *now,
                                }
                            } else if let Some(next) = now.revision.checked_add(1) {
                                now.revision = next;
                                now.value = *value;
                                jig_client_test_domain::connector_values::ApplicationOutcome::Changed {
                                    object: *object,
                                    now: *now,
                                }
                            } else {
                                jig_client_test_domain::connector_values::ApplicationOutcome::Exhausted
                            }
                        }
                        jig_client_test_domain::connector_values::ClientDocument::Object { .. }
                        | jig_client_test_domain::connector_values::ClientDocument::Panel { .. }
                        | jig_client_test_domain::connector_values::ClientDocument::Result { .. }
                        | jig_client_test_domain::connector_values::ClientDocument::Effect { .. }
                        | jig_client_test_domain::connector_values::ClientDocument::Outbox { .. }
                        | jig_client_test_domain::connector_values::ClientDocument::Hold { .. }
                        | jig_client_test_domain::connector_values::ClientDocument::Project { .. }
                        | jig_client_test_domain::connector_values::ClientDocument::Deployment { .. } => {
                            jig_client_test_domain::connector_values::ApplicationOutcome::Unknown
                        }
                    },
                    None | Some(family::Document::Item(_) | family::Document::Part(_)) => {
                        jig_client_test_domain::connector_values::ApplicationOutcome::Unknown
                    }
                };
                family::Answer::Connector(jig_client_test_domain::connector_values::ConnectorAnswer { value: outcome })
            }
        };
        Decision { answer, changes: vec![] }
    }
    fn document(&self, truth: &Truth, party: u64, subject: &Self::Subject) -> Option<Self::Document> {
        let allowed = match subject.scope {
            family::Scope::Project(project) => {
                truth.frames.get(&party).is_some_and(|frame| frame.projects.iter().any(|p| p.project == project))
            }
            family::Scope::Deployment => truth.frames.get(&party).is_some_and(|f| f.deployment_role.is_some()),
        };
        if allowed { self.documents.get(subject).cloned() } else { None }
    }
    fn marks(&self, _: &engine::Change) -> Vec<Self::Subject> {
        self.documents.keys().copied().collect()
    }
    fn fits(&self, value: ApplicationValue<'_, Self>, bounds: &engine::Bounds) -> bool {
        match value {
            ApplicationValue::Document(doc) => family_document_fits(doc, bounds),
            ApplicationValue::State(state) => state.documents.values().all(|doc| family_document_fits(doc, bounds)),
            ApplicationValue::Ask(_) | ApplicationValue::Answer(_) | ApplicationValue::Subject(_) => true,
        }
    }
}
fn family_document_fits(document: &family::Document, bounds: &engine::Bounds) -> bool {
    match document {
        family::Document::Item(_) | family::Document::Part(_) => document.fits(bounds.rows),
        family::Document::Connector(value) => {
            let limits = domain::connector_values::ConnectorLimits {
                rows: bounds.rows,
                paths: bounds.rows,
                segments: bounds.rows,
                segment_bytes: bounds.text,
                bytes: bounds.document_bytes,
            };
            value.matches_subject() && domain::connector_bounds::document_fits(value, &limits)
        }
    }
}
#[derive(Clone, Copy)]
enum Route {
    Jig(ClientId, Token),
    First(ClientId, Token),
}
/// The scripted engine with opaque client token routes.
pub struct Scripted {
    /// Whole subjects received at the scripted engine boundary.
    pub opened: Vec<Subject>,
    pub engine: engine::Engine<Family>,
    ledger: Ledger<Token, ()>,
    parties: BTreeMap<ClientId, u64>,
    hellos: std::collections::BTreeSet<ClientId>,
    shell: Vec<(ClientId, Event)>,
    lose_sign_out: bool,
    routes: BTreeMap<Token, Route>,
    watch_routes: std::collections::BTreeSet<Token>,
    serial: u64,
    committed: usize,
    last_attempt: Option<Token>,
    last_watch: Option<Token>,
    last_object: Option<ObjectKey>,
    page_watch: Option<Token>,
    link: Vec<Observed>,
    recovery: Option<Time>,
    pending_sign_ins: Vec<u64>,
    pending_faults: Vec<FrontFault>,
}
impl Scripted {
    /// Lose the next shell sign-out before it reaches the engine.
    pub fn lose_next_sign_out(&mut self) {
        self.lose_sign_out = true;
    }
    fn hello_events(&mut self) -> Vec<(ClientId, Event)> {
        std::mem::take(&mut self.hellos)
            .into_iter()
            .map(|client| {
                (
                    client,
                    crate::test::up(jig_client::Up::Served {
                        served: self.engine.versions().contains(19),
                        versions: None,
                    }),
                )
            })
            .collect()
    }
    /// Installs a checked story and its application documents.
    #[must_use]
    pub fn new(script: engine::Script, seed: u64, family: Family) -> Self {
        let mut initial = Vec::new();
        for source in &script.truth.sources {
            observe_change(&engine::Change::Source(source.clone()), 0, &mut initial);
        }
        Self {
            opened: Vec::new(),
            ledger: Ledger::new("scripted wire operation"),
            engine: engine::Engine::with_application(script, seed, family).expect("script keeps contract"),
            parties: BTreeMap::new(),
            hellos: std::collections::BTreeSet::new(),
            shell: Vec::new(),
            lose_sign_out: false,
            routes: BTreeMap::new(),
            watch_routes: std::collections::BTreeSet::new(),
            serial: 0,
            committed: 0,
            last_attempt: None,
            last_watch: None,
            last_object: None,
            page_watch: None,
            link: initial,
            recovery: None,
            pending_sign_ins: Vec::new(),
            pending_faults: Vec::new(),
        }
    }
    fn transport(&mut self, client: ClientId, local: Token, first: bool, fresh: bool) -> Token {
        if !fresh
            && let Some((token, _)) = self.routes.iter().rev().find(|(token, route)| {
                self.watch_routes.contains(token)
                    && match route {
                        Route::Jig(c, t) => !first && *c == client && *t == local,
                        Route::First(c, t) => first && *c == client && *t == local,
                    }
            })
        {
            return *token;
        }
        self.serial += 1;
        let token = Token::new(self.serial);
        self.ledger.open(token, ());
        self.routes.insert(token, if first { Route::First(client, local) } else { Route::Jig(client, local) });
        token
    }
}
fn envelope(value: Envelope) -> RequestEnvelope {
    match value {
        Envelope::Final => RequestEnvelope::Final,
        Envelope::Again => RequestEnvelope::Again,
        Envelope::Lost => RequestEnvelope::Lost,
        Envelope::SignedOut => RequestEnvelope::SignedOut,
    }
}
fn family_scope(value: jig_client::contract::Scope) -> engine::ApplicationScope {
    match value {
        jig_client::contract::Scope::Project(p) => engine::ApplicationScope::Project(p),
        jig_client::contract::Scope::Deployment => engine::ApplicationScope::Deployment,
        jig_client::contract::Scope::Own => panic!("this scripted family offers project and deployment scopes"),
    }
}
impl Scripted {
    fn recover(&mut self, now: Time, wall: Wall) {
        if self.recovery.is_some_and(|at| at <= now) {
            self.recovery = None;
            self.link.push(Observed::Reachable { live: true });
            for party in std::mem::take(&mut self.pending_sign_ins) {
                self.engine
                    .act(now, wall, engine::Change::SignIn { sign_in: party, party })
                    .expect("recovered sign in fits");
                for (client, owner) in &self.parties {
                    if *owner == party {
                        self.shell.push((*client, crate::test::up(jig_client::Up::SignedIn)));
                    }
                }
            }
            for fault in std::mem::take(&mut self.pending_faults) {
                self.fault(now, fault);
            }
        }
    }
    fn routed(&mut self, client: ClientId, request: Request) -> Request {
        let Request::Client { request: jig_client::Request::Down { down } } = request else { return request };
        let down = match down {
            Down::Send { attempt, key, request, value } => {
                let first = matches!(&request, Ask::Family { .. });
                Down::Send { attempt: self.transport(client, attempt, first, true), key, request, value }
            }
            Down::Read { read, what } => {
                let first = matches!(&what, Readable::Family { .. });
                Down::Read { read: self.transport(client, read, first, true), what }
            }
            Down::Open { watch, subject } => {
                let first = matches!(&subject, Watchable::Family { .. });
                let watch = self.transport(client, watch, first, true);
                self.watch_routes.insert(watch);
                Down::Open { watch, subject }
            }
            Down::Continue { watch, subject } => {
                Down::Continue { watch: self.transport(client, watch, false, false), subject }
            }
            Down::Close { watch } => {
                let first = self
                    .routes
                    .values()
                    .any(|route| matches!(route, Route::First(owner, local) if *owner == client && *local == watch));
                Down::Close { watch: self.transport(client, watch, first, false) }
            }
            Down::Save { .. } | Down::Erase { .. } => unreachable!("store writes belong to the shell"),
        };
        Request::Client { request: jig_client::Request::Down { down } }
    }
}

impl Front<Test> for Scripted {
    fn bind(&mut self, client: ClientId, party: u64) {
        self.parties.insert(client, party);
    }
    fn starting(&mut self, client: ClientId) -> Vec<Event> {
        self.hellos.insert(client);
        vec![crate::test::up(jig_client::Up::SignedIn)]
    }
    fn wire(&mut self, client: ClientId, now: Time, wall: Wall, request: Request) {
        let party = self.parties[&client];
        let request = self.routed(client, request);
        let Request::Client { request: jig_client::Request::Down { down } } = request else {
            unreachable!("shell classified a wire request")
        };
        match down {
            Down::Send { attempt, key, request: ask, value } => {
                assert!(value.is_none(), "the scripted engine carries no value");
                self.last_attempt = Some(attempt);
                match ask {
                    Ask::Jig(ask) => {
                        self.engine.send(now, wall, party, key, attempt, *ask).expect("jig send fits");
                    }
                    Ask::Family { scope, record } => {
                        let ask = crate::test::family_request(&record);
                        let jig_client::contract::Scope::Project(project) = scope else {
                            panic!("scripted family requests name a project")
                        };
                        let digest: [u8; 32] = Sha256::digest(format!("{ask:?}").as_bytes()).into();
                        self.engine
                            .send_application(now, wall, party, key, attempt, 1, project, digest, ask)
                            .expect("family send fits");
                    }
                }
            }
            Down::Read { read, what: Readable::Jig(what) } => {
                self.last_object = match &what {
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
                    | Read::Holds { .. }
                    | Read::Spend { .. }
                    | Read::Project { .. }
                    | Read::Notes { .. }
                    | Read::Settings
                    | Read::Setting(_)
                    | Read::Revision(_)
                    | Read::Revisions(_)
                    | Read::Deployment => self.last_object,
                };
                self.engine.read(now, party, read, what).expect("read fits");
            }
            Down::Open { watch, subject: Watchable::Jig(subject) } => {
                self.opened.push(subject.clone());
                self.last_watch = Some(watch);
                if let Subject::Task { task } = &subject {
                    self.last_object = Some(ObjectKey::Task(*task));
                    self.page_watch = Some(watch);
                }
                self.engine.open(now, party, watch, subject).expect("watch fits");
            }
            Down::Continue { watch, subject } => match subject {
                Subject::Run { turn, offset, .. } => {
                    self.engine.continue_run(watch, turn, offset).expect("run watch exists");
                }
                Subject::Me
                | Subject::Task { .. }
                | Subject::Tree { .. }
                | Subject::Chips { .. }
                | Subject::Inbox { .. }
                | Subject::Tasks { .. }
                | Subject::Settings
                | Subject::Deployment
                | Subject::Accounts => unreachable!("only runs continue"),
            },
            Down::Close { watch } => self.engine.close(now, watch),
            Down::Read { read, what: Readable::Family { scope, record, .. } } => {
                let subject = crate::test::family_subject(&record, true);
                self.engine
                    .read_scoped_application(now, party, read, 1, family_scope(scope), &subject)
                    .expect("family read fits");
            }
            Down::Open { watch, subject: Watchable::Family { scope, record, .. } } => {
                let subject = crate::test::family_subject(&record, false);
                self.last_watch = Some(watch);
                self.engine
                    .open_scoped_application(now, party, watch, 1, family_scope(scope), subject)
                    .expect("family watch fits");
            }
            Down::Save { .. } | Down::Erase { .. } => unreachable!("the shell keeps records"),
        }
    }

    fn poll(&mut self, now: Time, wall: Wall) -> Vec<(ClientId, Event)> {
        self.recover(now, wall);
        let mut events = self.hello_events();
        events.append(&mut self.shell);
        events.extend(self.engine.poll(now).into_iter().filter_map(|up| self.callback(up)).collect::<Vec<_>>());
        events
    }
    fn next_due(&self) -> Option<Time> {
        if self.hellos.is_empty() && self.shell.is_empty() {
            self.recovery.or_else(|| self.engine.next_due())
        } else {
            Some(Time::ZERO)
        }
    }
    fn observed(&mut self) -> Vec<Observed> {
        let mut seen = std::mem::take(&mut self.link);
        for commit in &self.engine.commits()[self.committed..] {
            seen.push(Observed::Durable { commit: commit.number, at: commit.durable.expect("durable ledger") });
            if let (Some(party), Some(key)) = (commit.party, commit.key) {
                seen.push(Observed::Made { party, key, commit: commit.number });
            }
            let mut facts = Vec::new();
            for change in &commit.changes {
                observe_change(change, commit.number, &mut facts);
            }
            // All changes in this durable commit are atomic. Ending discharges retention first.
            facts.sort_by_key(|fact| !matches!(fact, Observed::Ended { .. }));
            seen.extend(facts);
        }
        self.committed = self.engine.commits().len();
        seen
    }
    fn fault(&mut self, now: Time, fault: FrontFault) {
        if matches!(fault, FrontFault::ObjectGone) {
            if self.recovery.is_some_and(|at| now < at) {
                self.pending_faults.push(fault);
                return;
            }
            if let Some(key) = self.last_object {
                let wall = Wall::from_nanos(1_000_000_000_000 + now.as_nanos());
                self.engine.act(now, wall, engine::Change::Forget { key }).expect("forget fits");
                // Its authoritative listing stops naming the vanished card too.
                let sources = self.engine.truth().sources.clone();
                for mut source in sources {
                    if let engine::Rows::Inbox(rows) = &mut source.rows {
                        let before = rows.len();
                        rows.retain(|row| row.object != key);
                        if rows.len() != before {
                            self.engine.act(now, wall, engine::Change::Source(source)).expect("listing fits");
                        }
                    }
                }
            }
            return;
        }
        if let FrontFault::Restart { offline } = &fault {
            self.link.push(Observed::Reachable { live: false });
            self.recovery = Some(now.saturating_add(*offline));
        }
        let fault = match fault {
            FrontFault::Busy => engine::Fault::Busy,
            FrontFault::NotReady => engine::Fault::NotReady,
            FrontFault::Restart { offline } => engine::Fault::Restart { offline },
            FrontFault::Slow { span } => engine::Fault::Slow { span },
            FrontFault::DropWatch => {
                let Some(watch) = self.page_watch.or(self.last_watch) else {
                    return;
                };
                engine::Fault::DropWatch { watch }
            }
            FrontFault::NoAnswer => {
                let Some(attempt) = self.last_attempt else {
                    return;
                };
                engine::Fault::Lose { attempt }
            }
            FrontFault::SignedOut => {
                let sign_in = *self.parties.values().next().expect("client bound");
                for (&client, &party) in &self.parties {
                    if party == sign_in {
                        self.link.push(Observed::SignInEnded { client });
                    }
                }
                engine::Fault::SignOut { sign_in }
            }
            FrontFault::Versions => {
                self.hellos.extend(self.parties.keys().copied());
                engine::Fault::Versions(VersionRange { first: 2, last: 2 })
            }
            FrontFault::ObjectGone => unreachable!("object disappearance handled above"),
        };
        self.engine.fault(now, fault);
    }
    fn reload(&mut self, client: ClientId, now: Time) {
        let tokens: Vec<_> = self
            .routes
            .iter()
            .filter_map(|(t, r)| match r {
                Route::Jig(c, _) | Route::First(c, _) => (*c == client).then_some(*t),
            })
            .collect();
        for token in tokens {
            self.engine.close(now, token);
            self.routes.remove(&token);
            self.watch_routes.remove(&token);
        }
    }
    fn received(&mut self, client: ClientId, event: &Event) {
        let selected = match event {
            Event::Client {
                event: jig_client::Event::Up { up: jig_client::Up::Watched { watch, event: WatchEvent::Delivered(_) } },
            } => Some((*watch, false)),
            Event::First(first::Event::Watch { watch, event: family::WatchEvent::Delivered(_) }) => {
                Some((*watch, true))
            }
            Event::Start { .. }
            | Event::Went { .. }
            | Event::Act { .. }
            | Event::Client { event: jig_client::Event::Up { up: jig_client::Up::Answer { .. } } }
            | Event::Jig(_)
            | Event::First(_)
            | Event::SignedOut
            | Event::Client { .. } => None,
        };
        if let Some((local, first)) = selected {
            let token = self.transport(client, local, first, false);
            self.engine.taken(token);
        }
    }
    fn sign_out(&mut self, client: ClientId, now: Time, wall: Wall) {
        if std::mem::take(&mut self.lose_sign_out) || self.recovery.is_some_and(|at| now < at) {
            self.shell.push((client, Event::Client { event: jig_client::Event::SignOutLost }));
            return;
        }
        let sign_in = self.parties[&client];
        self.engine.act(now, wall, engine::Change::SignedOut { sign_in }).expect("sign out fits");
        for (&client, &party) in &self.parties {
            if party == sign_in {
                self.link.push(Observed::SignInEnded { client });
            }
        }
        self.shell.push((client, Event::SignedOut));
    }
    fn sign_in(&mut self, client: ClientId, now: Time, wall: Wall) {
        let party = self.parties[&client];
        if self.recovery.is_some_and(|at| now < at) {
            if !self.pending_sign_ins.contains(&party) {
                self.pending_sign_ins.push(party);
            }
            return;
        }
        self.engine.act(now, wall, engine::Change::SignIn { sign_in: party, party }).expect("sign in fits");
        self.shell.push((client, crate::test::up(jig_client::Up::SignedIn)));
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "the observation table classifies every scripted change without inspecting client state"
)]
fn observe_change(change: &engine::Change, commit: u64, seen: &mut Vec<Observed>) {
    match change {
        engine::Change::Source(source) => {
            if let (Read::Activity { task, .. }, engine::Rows::Activity(rows)) = (&source.read, &source.rows) {
                for row in rows {
                    if let Some((message, words)) = row.event.word_message() {
                        seen.push(Observed::Kept { task: *task, message, words: words.bytes().to_vec(), commit });
                    }
                }
                seen.push(Observed::Retained {
                    task: *task,
                    messages: rows
                        .iter()
                        .filter_map(|row| row.event.word_message().map(|(message, _)| message))
                        .collect(),
                    commit,
                });
            }
        }
        engine::Change::Forget { key } => seen.push(Observed::Stopped { card: crate::observe::key(*key), commit }),
        engine::Change::Object { key, value, .. } => {
            let terminal = match value {
                ReadValue::Proposal(p) => p.ending.is_some(),
                ReadValue::Escalation(e) => e.ending.is_some(),
                ReadValue::Question(q) => q.ending.is_some(),
                ReadValue::PersonTask(p) => p.ending.is_some(),
                ReadValue::Task(t) => t.result.is_some(),
                ReadValue::Result(_) => true,
                ReadValue::OwnerPersonTask(_)
                | ReadValue::Historical(_)
                | ReadValue::OwnerProposal(_)
                | ReadValue::OwnerEscalation(_)
                | ReadValue::Tasks(_)
                | ReadValue::Me(_)
                | ReadValue::Ended(_)
                | ReadValue::History(_)
                | ReadValue::Activity(_)
                | ReadValue::Turn(_)
                | ReadValue::Inbox(_)
                | ReadValue::OwnerInbox(_)
                | ReadValue::Holds(_)
                | ReadValue::Spend(_)
                | ReadValue::OwnerEnded(_)
                | ReadValue::OwnerSpend(_)
                | ReadValue::OwnerProject(_)
                | ReadValue::Project(_)
                | ReadValue::Notes(_)
                | ReadValue::Settings(_)
                | ReadValue::Setting(_)
                | ReadValue::Revision(_)
                | ReadValue::Revisions(_)
                | ReadValue::Deployment(_)
                | ReadValue::OwnerDeployment(_) => false,
            };
            if terminal {
                seen.push(Observed::Stopped { card: crate::observe::key(*key), commit });
            }
            match value {
                ReadValue::Task(t) if t.result.is_some() => seen.push(Observed::Ended { task: t.chip.task, commit }),
                ReadValue::Result(r) => seen.push(Observed::Ended { task: r.task.task, commit }),
                ReadValue::Task(_)
                | ReadValue::Tasks(_)
                | ReadValue::Me(_)
                | ReadValue::Proposal(_)
                | ReadValue::Historical(_)
                | ReadValue::OwnerProposal(_)
                | ReadValue::Escalation(_)
                | ReadValue::OwnerEscalation(_)
                | ReadValue::Question(_)
                | ReadValue::PersonTask(_)
                | ReadValue::OwnerPersonTask(_)
                | ReadValue::Ended(_)
                | ReadValue::History(_)
                | ReadValue::Activity(_)
                | ReadValue::Turn(_)
                | ReadValue::Inbox(_)
                | ReadValue::OwnerInbox(_)
                | ReadValue::Holds(_)
                | ReadValue::Spend(_)
                | ReadValue::OwnerEnded(_)
                | ReadValue::OwnerSpend(_)
                | ReadValue::OwnerProject(_)
                | ReadValue::Project(_)
                | ReadValue::Notes(_)
                | ReadValue::Settings(_)
                | ReadValue::Setting(_)
                | ReadValue::Revision(_)
                | ReadValue::Revisions(_)
                | ReadValue::Deployment(_)
                | ReadValue::OwnerDeployment(_) => {}
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
        | engine::Change::Deployment(_)
        | engine::Change::OwnerDeployment(_)
        | engine::Change::Tree(_)
        | engine::Change::Seen { .. } => {}
    }
}

impl Scripted {
    fn sign_out_receipt(&mut self, client: ClientId, up: &Up<Family>) {
        if matches!(
            up,
            Up::Answer { envelope: Envelope::SignedOut, .. }
                | Up::ApplicationAnswer { envelope: Envelope::SignedOut, .. }
                | Up::Read { terminal: ReadResult::None(ReadNone::SignedOut), .. }
                | Up::ApplicationRead { terminal: Err(ReadNone::SignedOut), .. }
                | Up::Ended { why: WatchEnd::SignedOut, .. }
        ) {
            self.shell.push((client, crate::test::up(jig_client::Up::SignedOut)));
        }
    }
    fn callback(&mut self, up: Up<Family>) -> Option<(ClientId, Event)> {
        let token = match &up {
            Up::Answer { attempt, .. } | Up::ApplicationAnswer { attempt, .. } => *attempt,
            Up::Read { token, .. }
            | Up::ApplicationRead { token, .. }
            | Up::Watch { token, .. }
            | Up::Ended { token, .. } => *token,
            Up::App { watch, .. } => *watch,
        };
        let terminal = matches!(
            &up,
            Up::Answer { .. }
                | Up::Read { .. }
                | Up::ApplicationAnswer { .. }
                | Up::ApplicationRead { .. }
                | Up::Ended { .. }
        );
        if terminal {
            self.ledger.end(token);
        }
        let route = self.routes.get(&token).copied()?;
        if terminal {
            self.watch_routes.remove(&token);
            self.routes.remove(&token);
        }
        let (client, local) = match route {
            Route::Jig(c, t) | Route::First(c, t) => (c, t),
        };
        self.sign_out_receipt(client, &up);
        let event = match up {
            Up::Answer { attempt: _, envelope: e, answer } => crate::test::up(jig_client::Up::Answer {
                attempt: local,
                envelope: envelope(e),
                answer: answer.map(jig_client::Answered::Jig),
                why: None,
            }),
            Up::Read { token: _, terminal } => {
                crate::test::up(jig_client::Up::ReadDone { read: local, result: jig_client::Readout::Jig(terminal) })
            }
            Up::Watch { token: _, event } => match route {
                Route::Jig(_, _) => crate::test::up(jig_client::Up::Watched { watch: local, event }),
                Route::First(_, _) => Event::First(first::Event::Watch {
                    watch: local,
                    event: match event {
                        WatchEvent::Opened => family::WatchEvent::Opened,
                        WatchEvent::Alive => family::WatchEvent::Alive,
                        WatchEvent::Behind | WatchEvent::Delivered(_) => {
                            panic!("application watches carry application documents")
                        }
                    },
                }),
            },
            Up::ApplicationAnswer { attempt: _, envelope: e, answer } => crate::test::up(jig_client::Up::Answer {
                attempt: local,
                envelope: envelope(e),
                answer: answer
                    .map(|a| crate::test::family_answer(a.expect("test family fixture requires a typed outcome"))),
                why: None,
            }),
            Up::ApplicationRead { token: _, terminal } => Event::First(first::Event::Read {
                read: local,
                result: match terminal {
                    Ok(d) => family::ReadResult::Value(d),
                    Err(ReadNone::Busy | ReadNone::Unreachable) => family::ReadResult::Busy,
                    Err(ReadNone::SignedOut) => family::ReadResult::SignedOut,
                    Err(
                        ReadNone::Unknown
                        | ReadNone::Refused
                        | ReadNone::Gone
                        | ReadNone::TooLarge { .. }
                        | ReadNone::Malformed,
                    ) => family::ReadResult::Unknown,
                },
            }),
            Up::App { watch: _, document } => {
                Event::First(first::Event::Watch { watch: local, event: family::WatchEvent::Delivered(document) })
            }
            Up::Ended { token: _, why } => match route {
                Route::Jig(_, _) => crate::test::up(jig_client::Up::WatchEnded { watch: local, end: why }),
                Route::First(_, _) => Event::First(first::Event::Ended {
                    watch: local,
                    end: match why {
                        WatchEnd::Closed => family::WatchEnd::Closed,
                        WatchEnd::SignedOut => family::WatchEnd::SignedOut,
                        WatchEnd::Dropped => family::WatchEnd::Dropped,
                        WatchEnd::NotServed => family::WatchEnd::Unsupported,
                        WatchEnd::Unknown => family::WatchEnd::Unknown,
                        WatchEnd::NotAllowed => family::WatchEnd::NotAllowed,
                        WatchEnd::Busy => family::WatchEnd::Busy,
                        WatchEnd::SubjectEnded => family::WatchEnd::SubjectEnded,
                        WatchEnd::Stopping => family::WatchEnd::Stopping,
                    },
                }),
            },
        };
        Some((client, event))
    }
}
