#[path = "ended.rs"]
mod ended;
use crate::referee::{People, RouteKind, Seen, Stimulus};
use jig_domain_people::{
    self as people, Again, Ask, Domain, Entry, Event, Holding, Identity, IdentityKey, InitialOwner, Key, Lack, Limits,
    Outcome, ProposalChoice, ProposalDecision, Refusal, Reply, Request, RequestKey, Role, Seed, Stored,
};
use skein_lib::{Duration, Env, Queue, ReplyTo, Rng, Time, Token, Wall};
use skein_world::domain::{Referee, Trace};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Clone, Copy, Debug)]
struct Winner {
    by: u64,
    choice: ProposalChoice,
    at: Wall,
}

pub const LIMITS: Limits = Limits {
    operations: 2,
    batch: 2,
    people: 8,
    inbox_entries: 4,
    inbox_tasks: 16,
    inbox_recipients: 64,
    sign_ins: 8,
    projects: 3,
    holdings: 8,
    initial_owners: 3,
    requests: 32,
    pending: 8,
    waiters: 3,
    program_hosts: 65,
    program_host_name_bytes: 32,
    act_grants: 2,
    identity_bytes: 32,
    words: 64,
    amendment_bytes: 512,
    sign_in_lifetime: Duration::from_secs(60),
    request_retention: Duration::from_secs(120),
    room_refusals: 2,
    facts: 16,
};

pub const ENDINGS: &[&str] = &[
    "started",
    "authority",
    "observer",
    "duplicate",
    "conflict",
    "expired",
    "signed out",
    "restart before durable",
    "restart after durable",
    "answers full",
    "waiter busy",
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Settings {
    pub seed: u64,
    pub limits: Limits,
    pub authority_refuses: bool,
    pub facts: bool,
    pub account_limit: Option<people::LimitKind>,
}

impl Settings {
    #[must_use]
    pub const fn calm(seed: u64) -> Settings {
        Settings { seed, limits: LIMITS, authority_refuses: false, facts: true, account_limit: None }
    }

    #[must_use]
    pub fn random(seed: u64) -> Settings {
        let mut rng = Rng::new(seed);
        let authority_refuses = rng.below(2) == 0;
        let limits = Limits {
            requests: u32::try_from(rng.between(4, 8)).expect("tiny capacity fits"),
            pending: u32::try_from(rng.between(1, 3)).expect("tiny capacity fits"),
            waiters: u32::try_from(rng.between(1, 3)).expect("tiny capacity fits"),
            people: 2,
            sign_ins: 2,
            projects: 1,
            holdings: 2,
            ..LIMITS
        };
        Settings { limits, authority_refuses, ..Settings::calm(seed) }
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Stats {
    pub routes: u32,
    pub tasks: u32,
    pub replies: u32,
    pub restarts: u32,
    pub endings: BTreeSet<&'static str>,
}

#[derive(Debug)]
struct Commit {
    number: u64,
    changes: Vec<Change>,
    tasks: Vec<(RequestKey, u64)>,
    proposals: Vec<((people::Proposer, u64), Winner)>,
    person_claims: Vec<(u64, Option<u64>)>,
    person_answers: Vec<(u64, u64, Wall)>,
    sayings: Vec<(u64, u64, Box<[u8]>)>,
    resolved: Vec<(u64, bool)>,
    amendments: Vec<(u64, u64, bool)>,
    pending_amendments: Vec<((u64, u64), u64)>,
}

#[derive(Debug)]
enum Change {
    Save(Stored),
    Erase(Key),
}

#[derive(Debug)]
struct PersonClaim {
    holder: Option<u64>,
    answer: Option<(u64, Wall)>,
}

#[derive(Debug)]
struct PersonTask {
    entries: Box<[Entry]>,
    holder: Option<u64>,
    answer: Option<(u64, Wall)>,
}

#[derive(Debug)]
struct Held {
    commit: u64,
    call: u64,
    reply: Reply,
}

#[derive(Debug)]
pub struct World {
    settings: Settings,
    domain: Domain,
    env: Env<Limits>,
    out: Queue<Request>,
    owners: Box<[InitialOwner]>,
    grants: Box<[people::ActGrant]>,
    pub acts: Vec<(Token, u16, people::Act)>,
    records: BTreeMap<Key, Stored>,
    durable_tasks: BTreeMap<RequestKey, u64>,
    ended_chats: BTreeMap<u64, (u64, u64)>,
    proposals: BTreeMap<(people::Proposer, u64), Winner>,
    holds: BTreeMap<u64, (u64, people::HoldReason)>,
    pub sayings: BTreeMap<u64, (u64, Box<[u8]>)>,
    pub executor_outcomes: Vec<(u64, bool)>,
    specs: BTreeMap<u64, u64>,
    pending_amendments: BTreeMap<(u64, u64), u64>,
    person_tasks: BTreeMap<u64, PersonTask>,
    commits: VecDeque<Commit>,
    held: VecDeque<Held>,
    calls: BTreeSet<u64>,
    replies: BTreeMap<u64, Reply>,
    deferred: Vec<(Token, RequestKey)>,
    next: u64,
    issued: u64,
    durable: u64,
    stats: Stats,
    trace: Trace,
    referee: Referee<People>,
    defer_route: bool,
    context: Option<RequestKey>,
    hold_replies: bool,
}

#[must_use]
pub fn identity(user: u64) -> Identity {
    Identity {
        key: IdentityKey { provider: 0, subject: (user).to_be_bytes().into() },
        login: b"login".to_vec().into_boxed_slice(),
        name: b"name".to_vec().into_boxed_slice(),
    }
}

impl World {
    #[must_use]
    pub fn new(settings: Settings) -> World {
        Self::with_owners(settings, Box::new([]))
    }

    #[must_use]
    pub fn with_owners(settings: Settings, owners: Box<[InitialOwner]>) -> World {
        Self::configured(settings, owners, Box::new([]))
    }

    #[must_use]
    pub fn configured(settings: Settings, owners: Box<[InitialOwner]>, grants: Box<[people::ActGrant]>) -> World {
        let domain = Domain::new(&settings.limits, owners.clone(), 1, grants.clone());
        let mut world = World {
            settings,
            domain,
            env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits: settings.limits },
            out: Queue::with_capacity(people::max_out(&settings.limits)),
            owners,
            grants,
            acts: Vec::new(),
            records: BTreeMap::new(),
            durable_tasks: BTreeMap::new(),
            ended_chats: BTreeMap::new(),
            proposals: BTreeMap::new(),
            holds: BTreeMap::new(),
            sayings: BTreeMap::new(),
            executor_outcomes: Vec::new(),
            specs: BTreeMap::new(),
            pending_amendments: BTreeMap::new(),
            person_tasks: BTreeMap::new(),
            commits: VecDeque::new(),
            held: VecDeque::new(),
            calls: BTreeSet::new(),
            replies: BTreeMap::new(),
            deferred: Vec::new(),
            next: settings.seed.saturating_mul(1000),
            issued: 0,
            durable: 0,
            stats: Stats::default(),
            trace: Trace::default(),
            referee: Referee::new(People::default()),
            defer_route: false,
            context: None,
            hold_replies: false,
        };
        world.send(Event::Restored);
        world
    }

    #[must_use]
    pub fn stats(&self) -> &Stats {
        &self.stats
    }

    #[must_use]
    pub fn trace(&self) -> &[String] {
        self.trace.lines()
    }

    #[must_use]
    pub fn reply(&self, call: u64) -> Option<Reply> {
        self.replies.get(&call).cloned()
    }

    /// Present the scripted owner's current hold snapshot.
    pub fn held_task(&mut self, task: u64, revision: u64, reason: people::HoldReason) {
        self.holds.insert(task, (revision, reason));
    }

    /// Present the scripted owner's current spec snapshot.
    pub fn amended_task(&mut self, task: u64, revision: u64) {
        self.specs.insert(task, revision);
    }

    #[must_use]
    pub fn spec_revision(&self, task: u64) -> Option<u64> {
        self.specs.get(&task).copied()
    }

    #[must_use]
    pub fn held(&self, task: u64) -> bool {
        self.holds.contains_key(&task)
    }

    #[must_use]
    pub fn tasks(&self) -> usize {
        self.durable_tasks.len()
    }

    #[must_use]
    pub fn records(&self) -> &BTreeMap<Key, Stored> {
        &self.records
    }

    pub fn ending(&mut self, name: &'static str) {
        self.stats.endings.insert(name);
    }

    fn name(&mut self) -> u64 {
        self.next += 1;
        self.next
    }

    fn call(&mut self) -> (u64, ReplyTo) {
        let number = self.name();
        assert!(self.calls.insert(number));
        self.observe(Seen::Called { call: number });
        (number, ReplyTo::new(Token::new(number)))
    }

    pub fn signin(&mut self, sign_in: u64, user: u64) -> u64 {
        self.signin_held(sign_in, user, None)
    }

    pub fn signin_held(&mut self, sign_in: u64, user: u64, held: Option<u64>) -> u64 {
        let (call, reply_to) = self.call();
        let person = self.name();
        self.send(Event::SignedIn {
            reply_to,
            person,
            sign_in,
            identity: identity(user),
            kind: people::Kind::Person,
            held,
        });
        call
    }

    pub fn signin_service(&mut self, sign_in: u64, person: u64) -> u64 {
        let (call, reply_to) = self.call();
        let candidate = self.name();
        self.send(Event::SignedIn {
            held: None,
            reply_to,
            person: candidate,
            sign_in,
            identity: Identity {
                key: IdentityKey { provider: 1, subject: person.to_be_bytes().into() },
                login: Box::new([]),
                name: Box::new([]),
            },
            kind: people::Kind::Service,
        });
        call
    }

    pub fn signout(&mut self, sign_in: u64) -> u64 {
        let (call, reply_to) = self.call();
        self.send(Event::SignOut { reply_to, sign_in });
        call
    }

    pub fn roles(&mut self, project: u32, holdings: Box<[Holding]>) {
        self.send(Event::Roles { project, holdings });
    }

    pub fn seed(&mut self, project: u32, collaborators: Box<[Seed]>) {
        assert!(self.domain.can_seed(&self.env.limits, project, &collaborators));
        self.send(Event::Seed { project, collaborators });
    }

    #[must_use]
    pub fn role_for_identity(&self, identity: IdentityKey, project: u32) -> Option<(u64, Role)> {
        self.domain.role_for_identity(identity, project)
    }

    pub fn remember_result(&mut self, person: u64, task: u64, position: u64) {
        self.domain.remember_result(
            &self.env.limits,
            person,
            people::ResultRef { task, project: 1, at: self.env.wall, position, forgettable: None },
        );
    }

    #[must_use]
    pub fn counts(&self, person: u64) -> people::Counts {
        self.domain.counts(&self.env.limits, person).expect("known party counts")
    }

    #[must_use]
    pub fn results_seen(&self, person: u64) -> u64 {
        self.domain.results_seen(person).expect("known party results position")
    }

    #[must_use]
    pub fn conversation_seen(&self, person: u64, task: u64) -> u64 {
        self.domain.conversation_seen(person, task).expect("known party conversation position")
    }

    pub fn forget_chat(&mut self, task: u64) {
        self.send(Event::Forget { task, project: 1, party: None, last_reply: 0 });
    }

    pub fn waiting(&mut self, task: u64, entries: Box<[Entry]>) {
        if let Some(entry) = entries.iter().find(|entry| matches!(entry.kind, people::EntryKind::ChatReply { .. }))
            && let (people::Whom::Person(party), people::EntryKind::ChatReply { message }) = (entry.whom, entry.kind)
        {
            self.send(Event::ChatState {
                party,
                project: entry.project,
                task,
                last_reply: message,
                live: true,
                seen: None,
                replies: Box::new([]),
            });
        }

        if entries.iter().any(|entry| entry.kind == people::EntryKind::PersonTask) {
            self.person_tasks.entry(task).or_insert_with(|| PersonTask {
                entries: entries.clone(),
                holder: None,
                answer: None,
            });
        }
        self.send(Event::Waiting { task, entries });
    }

    fn person_state(&self, task: u64) -> Option<PersonClaim> {
        let saved = self.person_tasks.get(&task)?;
        let mut holder = saved.holder;
        let mut answer = saved.answer;
        for commit in &self.commits {
            for &(number, next) in &commit.person_claims {
                if number == task {
                    holder = next;
                }
            }
            for &(number, by, at) in &commit.person_answers {
                if number == task {
                    holder = None;
                    answer = Some((by, at));
                }
            }
        }
        Some(PersonClaim { holder, answer })
    }

    #[must_use]
    pub fn inbox(&self, person: u64) -> Box<[Entry]> {
        self.domain.cached_waiting(&self.env.limits, person).expect("known person")
    }

    pub fn check_role_waiting(&mut self, task: u64, holders: [u64; 2], open: bool) {
        let present = holders.map(|person| self.inbox(person).iter().any(|entry| entry.task == task));
        if open {
            self.observe(Seen::RoleWaitingOpened { task, both_present: present == [true, true] });
        } else {
            self.observe(Seen::RoleWaitingResolved { task, both_absent: present == [false, false] });
        }
    }

    pub fn ask(&mut self, sign_in: u64, key: [u8; 16], words: &[u8]) -> u64 {
        self.request(sign_in, key, crate::fixtures::create(words, None))
    }

    pub fn act(&mut self, connector: u16, key: [u8; 16], act: people::Act) -> u64 {
        let (call, reply_to) = self.call();
        self.send(Event::Act { reply_to, connector, key, act: Box::new(act) });
        call
    }
    pub fn decide_acts(&mut self, outcome: &Outcome) {
        for (request, _, _) in std::mem::take(&mut self.acts) {
            self.send(Event::Decided { request, outcome: outcome.clone() });
        }
    }
    pub fn reconfigure(&mut self, owners: Box<[InitialOwner]>, grants: Box<[people::ActGrant]>) {
        self.owners = owners;
        self.grants = grants;
        self.restart();
    }
    pub fn request(&mut self, sign_in: u64, key: [u8; 16], ask: Ask) -> u64 {
        let (call, reply_to) = self.call();
        let person = self
            .records
            .values()
            .chain(self.commits.iter().flat_map(|c| {
                c.changes.iter().filter_map(|change| match change {
                    Change::Save(row) => Some(row),
                    Change::Erase(_) => None,
                })
            }))
            .find_map(|row| match row {
                Stored::SignIn { number, person, .. } if *number == sign_in => Some(*person),
                Stored::SignIn { .. }
                | Stored::Person { .. }
                | Stored::ReadResults { .. }
                | Stored::EndedRead { .. }
                | Stored::ReadConversation { .. }
                | Stored::Roles { .. }
                | Stored::Acted { .. }
                | Stored::Answer { .. }
                | Stored::Policy { .. } => None,
            })
            .unwrap_or(0);
        self.context = Some(RequestKey { person, key });
        self.send(Event::Ask { models: Box::new([]), reply_to, sign_in, key, ask });
        self.context = None;
        call
    }

    #[must_use]
    pub fn capacity_refusals(&self) -> skein_lib::List<people::CapacityRefusal> {
        self.domain.capacity_refusals()
    }
    #[must_use]
    pub fn capacity_forgotten(&self) -> u64 {
        self.domain.capacity_refusals_lost()
    }
    pub fn finish_account_deferred(&mut self) {
        for (request, _) in std::mem::take(&mut self.deferred) {
            self.send(Event::Decided { request, outcome: Outcome::AccountChanged });
        }
    }

    pub fn defer_routes(&mut self, defer: bool) {
        self.defer_route = defer;
    }

    pub fn decide_deferred(&mut self) {
        for (request, key) in std::mem::take(&mut self.deferred) {
            self.context = Some(key);
            let task = self.name();
            self.send_decision(request, key, task);
            self.context = None;
        }
    }

    fn send_decision(&mut self, request: Token, key: RequestKey, task: u64) {
        let outcome = if self.settings.authority_refuses {
            Outcome::Refused(Refusal::Authority { proposable: None, lacking: Lack::Shape })
        } else {
            Outcome::Created { first: task, last: task, given: None }
        };
        self.send_inner(Event::Decided { request, outcome }, Some((key, task)));
    }

    fn send(&mut self, event: Event) {
        self.send_inner(event, None);
    }

    #[expect(clippy::too_many_lines, reason = "the scripted parent drains one complete people step and its follow-up")]
    fn send_inner(&mut self, event: Event, task: Option<(RequestKey, u64)>) {
        let mut changes = Vec::new();
        let mut tasks = Vec::new();
        let mut proposals = Vec::new();
        let mut person_claims = Vec::new();
        let mut person_answers = Vec::new();
        let mut sayings = Vec::new();
        let mut resolved = Vec::new();
        let mut amendments = Vec::new();
        let mut pending_amendments = Vec::new();
        if !self.settings.authority_refuses {
            tasks.extend(task);
        }
        let mut replies = Vec::new();
        people::step(&mut self.domain, &self.env, event, &mut self.out);
        // Same-step root routing: Route and Decided share the decision's commit.
        while let Some(request) = self.out.pop() {
            self.trace.log(self.env.now, format_args!("{request:?}"));
            match request {
                Request::Act { request, connector, act } => {
                    self.stats.routes += 1;
                    self.acts.push((request, connector, *act));
                }
                Request::Acted { to, outcome } => replies.push((to.into_token().raw(), Reply::Outcome(outcome))),
                Request::Save { record } => changes.push(Change::Save(record)),
                Request::Erase { key } => changes.push(Change::Erase(key)),
                Request::Reply { to, reply } => {
                    if let Reply::Full { earliest, .. } = &reply {
                        assert!(*earliest > self.env.wall, "Full names a future expiry at its answer's clocks");
                    }
                    replies.push((to.into_token().raw(), reply));
                }
                Request::Route { request, person, project, role, ask, models: _ } => {
                    assert!(
                        project == 1
                            || matches!(
                                ask.as_ref(),
                                Ask::MarkSeen { .. }
                                    | Ask::AccountSignIn { .. }
                                    | Ask::AccountSignOut { .. }
                                    | Ask::AccountChange { .. }
                            )
                    );
                    let key = self.context.expect("a routed call carries the client's key");
                    assert_eq!(person, key.person);
                    let kind = match ask.as_ref() {
                        Ask::Compound { .. } => RouteKind::Control,
                        Ask::Application { .. } => RouteKind::Application,
                        Ask::Create { .. } => RouteKind::Goal,
                        Ask::Propose { action: people::Action::Batch(_), .. } | Ask::Withdraw { .. } => {
                            unreachable!("party batches are exercised by the core world")
                        }
                        Ask::MakeService { .. } => RouteKind::Service,
                        Ask::DecideProposal { .. }
                        | Ask::Propose {
                            action: people::Action::Amend { .. } | people::Action::Settings { .. }, ..
                        } => RouteKind::Proposal,
                        Ask::TakePerson { .. } | Ask::HandBackPerson { .. } | Ask::AnswerPerson { .. } => {
                            RouteKind::TakePerson
                        }
                        Ask::MarkSeen { .. } | Ask::Release { .. } | Ask::Amend { .. } | Ask::Say { .. } => {
                            RouteKind::Control
                        }
                        Ask::AccountSignIn { .. } | Ask::AccountSignOut { .. } | Ask::AccountChange { .. } => {
                            RouteKind::Account
                        }
                        Ask::EditNote { .. }
                        | Ask::Adopt { .. }
                        | Ask::Stop { .. }
                        | Ask::Cancel { .. }
                        | Ask::Move { .. }
                        | Ask::AnswerQuestion { .. }
                        | Ask::SetRoles { .. }
                        | Ask::Rename { .. }
                        | Ask::MakeProject { .. }
                        | Ask::ChangeSettings { .. }
                        | Ask::MoveToNewest { .. }
                        | Ask::Restore { .. }
                        | Ask::ChangePolicy { .. }
                        | Ask::SetPool { .. }
                        | Ask::DecideEscalation { .. } => {
                            panic!("the scripted parent handles only the named people stories")
                        }
                    };
                    let service = match self.records.get(&Key::Person(person)) {
                        Some(Stored::Person { identity, .. }) => identity.key.provider == 1,
                        Some(_) | None => false,
                    };
                    // The parent now owns role admission; the people child
                    // routes authenticated requests with their current role.
                    let allowed = match kind {
                        RouteKind::Service => role == Some(Role::Owner),
                        RouteKind::Chat | RouteKind::Goal | RouteKind::Proposal | RouteKind::TakePerson => {
                            role.is_some() && role != Some(Role::Observer)
                        }
                        // People checked an account request's holder; it carries no role.
                        RouteKind::Control | RouteKind::Application | RouteKind::Account => true,
                    };
                    if !allowed {
                        people::step(
                            &mut self.domain,
                            &self.env,
                            Event::Decided { request, outcome: Outcome::Refused(Refusal::Role { held: role }) },
                            &mut self.out,
                        );
                        continue;
                    }
                    self.stats.routes += 1;
                    self.observe(Seen::Routed { role: role.unwrap_or(Role::Observer), kind, service });
                    if kind == RouteKind::Account
                        && let Some(which) = self.settings.account_limit
                    {
                        people::step(
                            &mut self.domain,
                            &self.env,
                            Event::Decided { request, outcome: Outcome::Refused(Refusal::Limit { which }) },
                            &mut self.out,
                        );
                        continue;
                    }
                    if self.defer_route {
                        assert!(
                            kind == RouteKind::Goal || kind == RouteKind::Account,
                            "creation or account deferred script"
                        );
                        self.deferred.push((request, key));
                    } else {
                        match *ask {
                            Ask::Compound { project, operations } => {
                                let outcome = if self.settings.authority_refuses {
                                    Outcome::CompoundRefused { at: 1, why: Refusal::Unknown }
                                } else {
                                    Outcome::Compound { project, operations: operations.len(), first_message: 1 }
                                };
                                people::step(
                                    &mut self.domain,
                                    &self.env,
                                    Event::Decided { request, outcome },
                                    &mut self.out,
                                );
                            }
                            Ask::MarkSeen { seen } => {
                                let allowed = match seen {
                                    people::Seen::Results { .. } => true,
                                    people::Seen::Conversation { task, .. } => self
                                        .durable_tasks
                                        .iter()
                                        .any(|(key, number)| key.person == person && *number == task),
                                };
                                let outcome = if allowed {
                                    Outcome::MarkedSeen { seen }
                                } else {
                                    Outcome::Refused(Refusal::Role { held: self.domain.role(person, 1) })
                                };
                                if allowed
                                    && let people::Seen::Conversation { task, .. } = seen
                                    && self.mark_ended(request, person, task)
                                {
                                    continue;
                                }
                                people::step(
                                    &mut self.domain,
                                    &self.env,
                                    match (allowed, seen) {
                                        (true, people::Seen::Conversation { task: _, through }) => {
                                            Event::ConversationSeen { request, project: 1, last_reply: through }
                                        }
                                        _ => Event::Decided { request, outcome },
                                    },
                                    &mut self.out,
                                );
                            }
                            Ask::Application { connector, .. } => {
                                let token = Token::new(self.name());
                                people::step(
                                    &mut self.domain,
                                    &self.env,
                                    Event::Decided { request, outcome: Outcome::Applied { connector, token } },
                                    &mut self.out,
                                );
                            }
                            // The accounts child's decision is the accounts world's; this
                            // parent answers as it would once the sign-in or sign-out is under way.
                            Ask::AccountSignIn { .. } => {
                                people::step(
                                    &mut self.domain,
                                    &self.env,
                                    Event::Decided {
                                        request,
                                        outcome: Outcome::AccountSignInStarted {
                                            source: 1,
                                            address: b"https://issuer.test/visit".as_slice().into(),
                                        },
                                    },
                                    &mut self.out,
                                );
                            }
                            Ask::AccountChange { change, .. } => {
                                assert_eq!(role, None, "account admission needs no role");
                                let outcome = match change {
                                    people::AccountChange::AddKey { .. } | people::AccountChange::ReplaceKey { .. } => {
                                        Outcome::AccountKeyKept { source: 1 }
                                    }
                                    people::AccountChange::DeclareProgram { name, .. } => {
                                        Outcome::AccountProgramDeclared { source: 1, name: name.clone() }
                                    }

                                    people::AccountChange::Remove { .. } => {
                                        Outcome::AccountRemoved { confirmed: true, failed_hosts: Box::new([]) }
                                    }
                                    people::AccountChange::MoveOut { account } => {
                                        Outcome::AccountMovingOut { account, number: 2, source: 1 }
                                    }
                                    people::AccountChange::FinishSignIn { source, .. } => {
                                        Outcome::AccountFinishing { source }
                                    }
                                    people::AccountChange::MoveIn { .. } => {
                                        Outcome::AccountMovedIn { account: 1, renewed: false }
                                    }
                                    people::AccountChange::Written { .. }
                                    | people::AccountChange::CheckProgram { .. }
                                    | people::AccountChange::RemoveKey { .. }
                                    | people::AccountChange::Resume { .. }
                                    | people::AccountChange::MoveAccount { .. }
                                    | people::AccountChange::MoveSource { .. }
                                    | people::AccountChange::SetUses { .. } => Outcome::AccountChanged,
                                };
                                people::step(
                                    &mut self.domain,
                                    &self.env,
                                    Event::Decided { request, outcome },
                                    &mut self.out,
                                );
                            }
                            Ask::AccountSignOut { .. } => {
                                people::step(
                                    &mut self.domain,
                                    &self.env,
                                    Event::Decided {
                                        request,
                                        outcome: Outcome::AccountSignedOut {
                                            confirmed: true,
                                            failed_hosts: Box::new([]),
                                        },
                                    },
                                    &mut self.out,
                                );
                            }
                            Ask::MakeService { .. } => {
                                let candidate = self.name();
                                people::step(
                                    &mut self.domain,
                                    &self.env,
                                    Event::MakeService { request, person: candidate },
                                    &mut self.out,
                                );
                            }
                            Ask::Propose { action: people::Action::Settings { .. }, .. } => {
                                let proposal = self.name();
                                people::step(
                                    &mut self.domain,
                                    &self.env,
                                    Event::Decided {
                                        request,
                                        outcome: Outcome::Proposed { proposal, to: people::Role::Owner.number() },
                                    },
                                    &mut self.out,
                                );
                            }
                            Ask::Propose { action: people::Action::Amend { task, .. }, .. } => {
                                let proposal = self.name();
                                pending_amendments.push(((person, proposal), task));
                                people::step(
                                    &mut self.domain,
                                    &self.env,
                                    Event::Decided { request, outcome: Outcome::Proposed { proposal, to: 0 } },
                                    &mut self.out,
                                );
                            }
                            Ask::Propose { action: people::Action::Batch(_), .. } | Ask::Withdraw { .. } => {
                                unreachable!("party batches are exercised by the core world")
                            }
                            Ask::Create { .. } => {
                                let outcome = if self.settings.authority_refuses {
                                    Outcome::Refused(Refusal::Authority { proposable: None, lacking: Lack::Shape })
                                } else {
                                    let number = self.name();
                                    tasks.push((key, number));
                                    Outcome::Created { first: number, last: number, given: None }
                                };
                                people::step(
                                    &mut self.domain,
                                    &self.env,
                                    Event::Decided { request, outcome },
                                    &mut self.out,
                                );
                            }
                            Ask::DecideProposal { proposer, proposal, decision, .. } => {
                                let name = (proposer, proposal);
                                let Winner { by, choice, at } = if let Some(&winner) = self.proposals.get(&name) {
                                    winner
                                } else {
                                    let choice = match decision {
                                        ProposalDecision::Accept => ProposalChoice::Accepted,
                                        ProposalDecision::Reject { .. } => ProposalChoice::Rejected,
                                        ProposalDecision::Pass => ProposalChoice::Passed,
                                    };
                                    let winner = Winner { by: person, choice, at: self.env.wall };
                                    proposals.push((name, winner));
                                    if choice == ProposalChoice::Accepted
                                        && let people::Proposer::Party(party) = proposer
                                        && let Some(task) = self.pending_amendments.get(&(party, proposal))
                                    {
                                        amendments.push((*task, self.specs[task] + 1, false));
                                    }
                                    winner
                                };
                                let outcome =
                                    Outcome::ProposalDecided { proposer, proposal, by, at, choice, repeated: false };
                                people::step(
                                    &mut self.domain,
                                    &self.env,
                                    Event::Decided { request, outcome },
                                    &mut self.out,
                                );
                            }
                            Ask::Say { task, words, .. } => {
                                let message = self.name();
                                sayings.push((task, message, words));
                                people::step(
                                    &mut self.domain,
                                    &self.env,
                                    Event::Decided { request, outcome: Outcome::Said { task, message } },
                                    &mut self.out,
                                );
                            }
                            Ask::Release { task, revision, again, .. } => {
                                let outcome = match self.holds.get(&task) {
                                    Some(&(now, reason)) if revision != now => Outcome::Refused(Refusal::Moved {
                                        now: people::Moved::Hold { revision: now, reason },
                                    }),
                                    Some(&(_, reason)) => {
                                        let required = matches!(reason, people::HoldReason::Uncertain { .. });
                                        if required == again.is_some() {
                                            if let people::HoldReason::Uncertain { entry } = reason {
                                                resolved.push((entry, again.expect("required choice")));
                                            }
                                            Outcome::Released { task }
                                        } else {
                                            Outcome::Refused(Refusal::Authority {
                                                proposable: None,
                                                lacking: Lack::Again { required },
                                            })
                                        }
                                    }
                                    None => Outcome::Refused(Refusal::Unknown),
                                };
                                if let Outcome::Released { task } = outcome
                                    && let Some(revision) = self.specs.get(&task)
                                {
                                    amendments.push((task, *revision, true));
                                }
                                people::step(
                                    &mut self.domain,
                                    &self.env,
                                    Event::Decided { request, outcome },
                                    &mut self.out,
                                );
                            }
                            Ask::Amend { task, revision, amendment, release, .. } => {
                                let outcome = match self.specs.get(&task).copied() {
                                    Some(now) if revision != now => {
                                        Outcome::Refused(Refusal::Moved { now: people::Moved::Spec { revision: now } })
                                    }
                                    Some(now) => {
                                        if let Some((held_revision, reason)) = self.holds.get(&task).copied()
                                            && release.is_some()
                                            && release != Some(held_revision)
                                        {
                                            Outcome::Refused(Refusal::Moved {
                                                now: people::Moved::Hold { revision: held_revision, reason },
                                            })
                                        } else if release.is_some()
                                            && self.holds.get(&task).is_some_and(|(_, reason)| {
                                                matches!(reason, people::HoldReason::Uncertain { .. })
                                            })
                                        {
                                            Outcome::Refused(Refusal::Authority {
                                                proposable: None,
                                                lacking: Lack::Again { required: true },
                                            })
                                        } else if amendment.authority.is_some() && self.settings.authority_refuses {
                                            Outcome::Refused(Refusal::Authority {
                                                proposable: Some(people::Accepter::Role(0)),
                                                lacking: Lack::Shape,
                                            })
                                        } else {
                                            let released = release.is_some() && self.holds.contains_key(&task);
                                            amendments.push((task, now + 1, released));
                                            Outcome::Amended { task, released }
                                        }
                                    }
                                    None => Outcome::Refused(Refusal::Unknown),
                                };
                                people::step(
                                    &mut self.domain,
                                    &self.env,
                                    Event::Decided { request, outcome },
                                    &mut self.out,
                                );
                            }
                            act @ (Ask::TakePerson { task, .. }
                            | Ask::HandBackPerson { task, .. }
                            | Ask::AnswerPerson { task, .. }) => {
                                let outcome = match self.person_state(task) {
                                    Some(PersonClaim { answer: Some((by, at)), .. }) => {
                                        Outcome::Refused(Refusal::Ended { how: people::Ending::Decided { by, at } })
                                    }
                                    Some(PersonClaim { holder: Some(by), answer: None }) if by != person => {
                                        Outcome::Refused(Refusal::Taken { by })
                                    }
                                    Some(PersonClaim { holder, answer: None }) => match act {
                                        Ask::TakePerson { .. } if holder.is_none() => {
                                            person_claims.push((task, Some(person)));
                                            Outcome::PersonTaken { task }
                                        }
                                        Ask::HandBackPerson { .. } if holder == Some(person) => {
                                            person_claims.push((task, None));
                                            Outcome::PersonHandedBack { task }
                                        }
                                        Ask::AnswerPerson { .. } if holder == Some(person) => {
                                            person_answers.push((task, person, self.env.wall));
                                            Outcome::PersonAnswered { task }
                                        }
                                        Ask::TakePerson { .. }
                                        | Ask::HandBackPerson { .. }
                                        | Ask::AnswerPerson { .. } => Outcome::Refused(Refusal::NotWaiting),
                                        Ask::Compound { .. }
                                        | Ask::MarkSeen { .. }
                                        | Ask::Application { .. }
                                        | Ask::EditNote { .. }
                                        | Ask::MakeService { .. }
                                        | Ask::Adopt { .. }
                                        | Ask::Create { .. }
                                        | Ask::Propose { .. }
                                        | Ask::Withdraw { .. }
                                        | Ask::Stop { .. }
                                        | Ask::Cancel { .. }
                                        | Ask::Release { .. }
                                        | Ask::Move { .. }
                                        | Ask::DecideProposal { .. }
                                        | Ask::Say { .. }
                                        | Ask::AnswerQuestion { .. }
                                        | Ask::Amend { .. }
                                        | Ask::SetRoles { .. }
                                        | Ask::Rename { .. }
                                        | Ask::MakeProject { .. }
                                        | Ask::ChangeSettings { .. }
                                        | Ask::MoveToNewest { .. }
                                        | Ask::Restore { .. }
                                        | Ask::ChangePolicy { .. }
                                        | Ask::SetPool { .. }
                                        | Ask::DecideEscalation { .. }
                                        | Ask::AccountSignIn { .. }
                                        | Ask::AccountSignOut { .. }
                                        | Ask::AccountChange { .. } => unreachable!("person task act"),
                                    },
                                    None => Outcome::Refused(Refusal::Unknown),
                                };
                                people::step(
                                    &mut self.domain,
                                    &self.env,
                                    Event::Decided { request, outcome },
                                    &mut self.out,
                                );
                            }
                            Ask::EditNote { .. }
                            | Ask::Adopt { .. }
                            | Ask::Stop { .. }
                            | Ask::Cancel { .. }
                            | Ask::Move { .. }
                            | Ask::AnswerQuestion { .. }
                            | Ask::SetRoles { .. }
                            | Ask::Rename { .. }
                            | Ask::MakeProject { .. }
                            | Ask::ChangeSettings { .. }
                            | Ask::MoveToNewest { .. }
                            | Ask::Restore { .. }
                            | Ask::ChangePolicy { .. }
                            | Ask::SetPool { .. }
                            | Ask::DecideEscalation { .. } => unreachable!("scripted route kind was checked"),
                        }
                    }
                }
                Request::RolesApplied { .. } | Request::RolesRefused { .. } => {
                    self.ending("busy");
                }
                Request::ServiceMade { request, outcome } => {
                    people::step(&mut self.domain, &self.env, Event::Decided { request, outcome }, &mut self.out);
                }
                Request::RestoreRefused { .. } => panic!("valid world records must restore"),
            }
        }
        if !changes.is_empty()
            || !tasks.is_empty()
            || !proposals.is_empty()
            || !person_claims.is_empty()
            || !person_answers.is_empty()
        {
            self.issued += 1;
            self.commits.push_back(Commit {
                number: self.issued,
                changes,
                tasks,
                proposals,
                person_claims,
                person_answers,
                sayings,
                resolved,
                amendments,
                pending_amendments,
            });
        }
        for (call, reply) in replies {
            self.held.push_back(Held { commit: self.issued, call, reply });
        }
        self.release();
        self.domain.reclaim();
        if self.settings.facts {
            while self.domain.pop_fact().is_some() {}
        }
    }

    pub fn commit_all(&mut self) {
        while let Some(commit) = self.commits.pop_front() {
            for change in commit.changes {
                match change {
                    Change::Save(row) => {
                        self.records.insert(row.key(), row);
                    }
                    Change::Erase(key) => {
                        self.records.remove(&key);
                    }
                }
            }
            self.durable = commit.number;
            self.observe(Seen::Durable { commit: self.durable });
            for (key, task) in commit.tasks {
                assert!(self.durable_tasks.insert(key, task).is_none(), "same key cannot make another task");
                self.stats.tasks += 1;
                self.observe(Seen::Created { key });
            }
            for (name, winner) in commit.proposals {
                assert!(self.proposals.insert(name, winner).is_none(), "first proposal decision wins");
            }
            self.executor_outcomes.extend(commit.resolved);
            self.pending_amendments.extend(commit.pending_amendments);
            for (task, revision, released) in commit.amendments {
                self.specs.insert(task, revision);
                if released {
                    self.holds.remove(&task);
                }
            }
            for (task, message, words) in commit.sayings {
                assert!(self.sayings.insert(message, (task, words)).is_none());
            }
            for (task, holder) in commit.person_claims {
                let saved = self.person_tasks.get_mut(&task).expect("known person task");
                saved.holder = holder;
                let entries = if holder.is_none() { saved.entries.clone() } else { Box::new([]) };
                self.waiting(task, entries);
            }
            for (task, by, at) in commit.person_answers {
                let saved = self.person_tasks.get_mut(&task).expect("known person task");
                saved.holder = None;
                saved.answer = Some((by, at));
                self.waiting(task, Box::new([]));
            }
            self.release();
        }
    }

    pub fn hold_replies(&mut self, hold: bool) {
        self.hold_replies = hold;
    }

    fn release(&mut self) {
        if self.hold_replies {
            return;
        }
        while self.held.front().is_some_and(|reply| reply.commit <= self.durable) {
            let held = self.held.pop_front().expect("front present");
            assert!(self.calls.remove(&held.call), "one reply per live call");
            match &held.reply {
                Reply::Refused(why) | Reply::Outcome(Outcome::Refused(why)) => {
                    self.observe(Seen::Refused { why: why.clone() });
                }
                Reply::Signer(_)
                | Reply::Full { .. }
                | Reply::SignedIn { .. }
                | Reply::SignedOut
                | Reply::Outcome(_)
                | Reply::Again(_) => {}
            }
            assert!(self.replies.insert(held.call, held.reply).is_none());
            self.stats.replies += 1;
            self.observe(Seen::Replied { call: held.call, after: held.commit });
        }
    }

    /// Cold restart drops volatile outputs and flights, retaining only durable
    /// people records and scripted tasks. Client calls lost with the process
    /// are abandoned, then retried with a fresh `ReplyTo` and the same key.
    pub fn restart(&mut self) {
        self.referee.inject(self.env.now, Stimulus::Restart);
        self.fire_referee();
    }

    fn restart_cold(&mut self) {
        for call in std::mem::take(&mut self.calls) {
            self.observe(Seen::Abandoned { call });
        }
        self.commits.clear();
        self.held.clear();
        self.deferred.clear();
        self.acts.clear();
        self.issued = self.durable;
        self.domain = Domain::new(&self.env.limits, self.owners.clone(), 1, self.grants.clone());
        for record in self.records.values().filter(|record| record.key().live()).cloned().collect::<Vec<_>>() {
            self.send(Event::Restore { record });
        }
        self.send(Event::Restored);
        self.restore_ended();
        self.stats.restarts += 1;
    }

    /// Correct the wall clock before the next input, without firing its timer first.
    pub fn set_wall(&mut self, wall: Wall) {
        self.env.wall = wall;
    }

    pub fn advance(&mut self, duration: Duration) {
        self.env.now = self.env.now.saturating_add(duration);
        self.env.wall = Wall::from_nanos(self.env.wall.as_nanos().saturating_add(duration.as_nanos()));
        while self.domain.is_due(self.env.now) {
            people::fire(&mut self.domain, &self.env, &mut self.out);
            // Fire's erases form their own decision, handled by a harmless
            // Restored event after its already queued outputs.
            self.send(Event::Restored);
        }
        self.commit_all();
        self.fire_referee();
    }

    fn observe(&mut self, seen: Seen) {
        let mut stimuli = Vec::new();
        self.referee.observe(self.env.now, seen, &mut stimuli);
        for stimulus in stimuli {
            match stimulus {
                Stimulus::Restart => self.restart_cold(),
            }
        }
        self.referee.assert_holding(self.settings.seed);
    }

    fn fire_referee(&mut self) {
        let mut stimuli = Vec::new();
        self.referee.fire(self.env.now, &mut stimuli);
        for stimulus in stimuli {
            match stimulus {
                Stimulus::Restart => self.restart_cold(),
            }
        }
        self.referee.assert_holding(self.settings.seed);
    }

    pub fn assert_settled(&self) {
        assert!(self.calls.is_empty() && self.commits.is_empty() && self.held.is_empty() && self.deferred.is_empty());
        self.referee.assert_passed(self.settings.seed);
    }

    /// Compact scenario matrix: each seed draws authority and crash cuts while
    /// every configured outcome is named and checked across the sweep.
    pub fn run(&mut self) {
        let signin = self.signin(10, 1);
        self.commit_all();
        let Reply::SignedIn { person, .. } = self.reply(signin).expect("sign-in replied") else { panic!("signed in") };
        self.roles(1, Box::new([Holding { person, role: Role::Member }]));
        self.commit_all();
        let mut rng = Rng::new(self.settings.seed);
        let first = self.ask(10, [1; 16], b"hello");
        assert!(self.reply(first).is_none(), "answer waits for durability");
        let before = rng.below(2) == 0;
        if before {
            self.restart();
            self.ending("restart before durable");
        } else {
            self.hold_replies(true);
            self.commit_all();
            assert!(self.reply(first).is_none());
            self.restart();
            self.hold_replies(false);
            self.ending("restart after durable");
        }
        let replay = self.ask(10, [1; 16], b"hello");
        self.commit_all();
        assert!(matches!(self.reply(replay), Some(Reply::Outcome(_))));
        self.ending(if self.settings.authority_refuses { "authority" } else { "started" });
        let routes = self.stats.routes;
        let duplicate = self.ask(10, [1; 16], b"hello");
        self.commit_all();
        assert_eq!(self.reply(duplicate), self.reply(replay));
        assert_eq!(self.stats.routes, routes);
        self.ending("duplicate");
        let conflict = self.ask(10, [1; 16], b"different");
        self.commit_all();
        assert_eq!(self.reply(conflict), Some(Reply::Refused(Refusal::KeyConflict)));
        self.ending("conflict");
        self.defer_routes(true);
        let mut waiting = Vec::new();
        for _ in 0..self.settings.limits.waiters {
            waiting.push(self.ask(10, [7; 16], b"pending"));
        }
        let overflow = self.ask(10, [7; 16], b"pending");
        self.commit_all();
        assert_eq!(self.reply(overflow), Some(Reply::Again(Again::Busy)));
        self.ending("waiter busy");
        self.decide_deferred();
        self.commit_all();
        self.defer_routes(false);
        for call in waiting {
            assert!(matches!(self.reply(call), Some(Reply::Outcome(_))));
        }
        self.roles(1, Box::new([Holding { person, role: Role::Observer }]));
        self.commit_all();
        let denied = self.ask(10, [2; 16], b"hello");
        self.commit_all();
        assert_eq!(
            self.reply(denied),
            Some(Reply::Outcome(Outcome::Refused(Refusal::Role { held: Some(Role::Observer) })))
        );
        self.ending("observer");
        for byte in
            20..u8::try_from(self.settings.limits.requests).expect("scenario request limit fits u8").saturating_add(20)
        {
            let call = self.ask(10, [byte; 16], b"fill");
            self.commit_all();
            if let Some(Reply::Again(Again::Full { held, .. })) = self.reply(call) {
                assert_eq!(held, self.settings.limits.requests);
                self.ending("answers full");
            }
        }
        assert!(self.stats.endings.contains("answers full"));
        self.advance(Duration::from_secs(61));
        let expired = self.ask(10, [3; 16], b"hello");
        self.commit_all();
        assert_eq!(self.reply(expired), Some(Reply::SignedOut));
        self.ending("expired");
        let signin = self.signin(11, 1);
        self.commit_all();
        assert!(matches!(self.reply(signin), Some(Reply::SignedIn { .. })));
        let signout = self.signout(11);
        self.commit_all();
        assert_eq!(self.reply(signout), Some(Reply::SignedOut));
        self.ending("signed out");
        self.assert_settled();
    }
}
