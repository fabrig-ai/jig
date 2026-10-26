//! Durable parent and scripted executors at the retained tasks boundary
//! (domain/tasks.md, 11; domain/engine.md, 7).
mod compound;
use crate::referee::{Seen, Stimulus, Tasks};
use crate::stories::{ChildResult, Restart, RetryEnd, Story, TurnState};
use crate::trace::Trace;
use jig_domain_tasks::{
    self as tasks, Accepted, Authority, Budget, Cause, CharterModels, Contract, Delegation, Domain, End, Ending, Event,
    Executor, Fact, Funder, Key, Limits, MessageKind, Models, New, Numbers, Party, Problem, Request, ResultKind,
    Retries, Retry, RunContext, Scopes, Spec, Stored, TaskResult, Tools, Word,
};
use skein_lib::{Duration, Env, Queue, ReplyTo, Rng, Time, Token, Wall};
use skein_world::domain::Referee;
use std::collections::{BTreeMap, BTreeSet};

pub const RETRY: Retry = Retry { retries: 2, base: Duration::from_millis(10), max: Duration::from_secs(1) };

pub const LIMITS: Limits = Limits {
    tasks: 16,
    funders: 32,
    project_tasks: 16,
    project_goals: 2,
    tree_tasks: 16,
    depth: 4,
    delegates: 8,
    references: 8,
    subscriptions: 8,
    batch: 8,
    dependencies: 8,
    holdings: 4,
    hold_kinds: 4,
    resource_reports: 64,
    pools: 8,
    hold_segments: 4,
    hold_bytes: 64,
    hold_waiters: 8,
    grace: skein_lib::Duration::from_secs(600),
    hold_wait: Duration::from_secs(10),
    circle: 8,
    inputs: 4,
    spec_bytes: 64,
    parameters: 4,
    result_bytes: 32,
    run_inbox: 8,
    inbox_messages: 8,
    inbox_bytes: 128,
    message_bytes: 32,
    proposal_stall: Duration::from_millis(10),
    escalation_stall: Duration::from_secs(3600),
    person_stall: Duration::from_secs(3600),
    saved_resources: 2,
    contract_choices: 4,
    charters: 2,
    versions: 8,
    revisions: 4,
    authority_grants: 4,
    authority_segments: 4,
    authority_bytes: 64,
    executor_kinds: 3,
    retries: Retries { transient: RETRY, permanent: RETRY, run: RETRY, agent: RETRY, lost: RETRY, invalid: RETRY },
    facts: 16,
    room_refusals: 2,
};

#[must_use]
pub fn authority() -> Authority {
    Authority {
        tools: Tools(0),
        models: Models::Named(0),
        grants: Box::new([]),
        delegation: Delegation { kinds: Box::new([]), tasks: 16, depth: 4 },
        budget: Budget { spend: 100, deadline: None },
        notes: Scopes(0),
        note_resources: Box::new([]),
    }
}

/// The configured models' declared efforts, in configuration's order: model 0 declares two,
/// model 1 none, model 2 one and model 3 none (domain/tasks.md, section 3).
pub const EFFORTS: [u8; 4] = [2, 0, 1, 0];

/// The configured charters and their models: charter 1 lists models 0, 1 and 2, in that order,
/// and charter 2 model 3.
#[must_use]
pub fn charters() -> Box<[CharterModels]> {
    Box::new([
        CharterModels { number: 1, revision: 1, models: Some(Box::new([0, 1, 2])) },
        CharterModels { number: 2, revision: 1, models: Some(Box::new([3])) },
    ])
}

/// A tasks domain configured with `charters` and `EFFORTS`.
#[must_use]
pub fn domain(limits: &Limits, seed: u64) -> Domain {
    Domain::new(limits, seed, 1, charters(), Box::new(EFFORTS))
}

#[must_use]
pub fn task(number: u64, dependencies: &[u64]) -> New {
    New {
        deadline: jig_domain_tasks::Deadline::Named,
        label: 0,
        flags: jig_domain_tasks::Flags::PLAIN,
        number,
        project: 1,
        executor: Executor::Agent { charter: 1 },
        llm: None,
        spec: Spec { words: Box::new([1]), parameters: Box::new([]), inputs: Box::new([]) },
        contract: Contract::Report { words: 32 },
        authority: authority(),
        numbers: Numbers { budget: 100, spent: 0, spent_below: 0, reserved: 0 },
        funder: Funder::Period { project: 1, period: 0 },
        dependencies: dependencies.into(),
        holdings: Box::new([]),
        wake: tasks::WakePolicy::DEFAULT,
        recurring: None,
        tracked: None,
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Reply {
    Made(Vec<u64>),
    Done,
    Refused(Problem),
    Acknowledged(Accepted),
    Turn(Accepted),
    WriterWaiting(tasks::Name),
}

/// Complete frozen run evidence, including internal deterministic state and all
/// parent/executor state; facts are diagnostic only (domain/tasks.md, 11).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Frozen {
    /// Durable parent rows (domain/tasks.md, 5).
    pub records: BTreeMap<Key, Stored>,
    /// Committed call replies (domain/tasks.md, 5).
    pub replies: BTreeMap<u64, Reply>,
    /// Pending executor preparations (domain/engine.md, 7).
    pub activations: BTreeSet<u64>,
    activation_counts: BTreeMap<u64, u32>,
    /// Owned activation briefs (domain/engine.md, 9).
    pub contexts: BTreeMap<u64, RunContext>,
    /// Physical executor attempts (domain/engine.md, 7).
    pub runs: BTreeMap<u64, u64>,
    /// Requested stops awaiting terminal work (domain/tasks.md, 5.1).
    pub stops: BTreeSet<(u64, u64)>,
    /// Effects settlement gates (domain/tasks.md, 5.1).
    pub closing: BTreeSet<u64>,
    /// Committed requester outcomes (domain/tasks.md, 5.6).
    pub results: BTreeMap<u64, Ending>,
    ended_topics: Vec<(u64, u64, u16)>,
    deadlines: BTreeMap<u64, (Wall, Time)>,
    kinds: Vec<tasks::Kind>,
    reports: BTreeMap<tasks::Name, tasks::HoldKind>,
    restoring: bool,
    facts: Vec<Fact>,
    consume_facts: bool,
    limits: Limits,
    seed: u64,
    priced: Option<(u64, u64)>,
    internal: std::sync::Arc<Internal>,
    now: Time,
    wall: Wall,
    call: u64,
    message: u64,
    commit: u64,
}

/// Retain complete non-cloneable deterministic state without eager dumps.
#[derive(Debug)]
struct Internal {
    domain: Domain,
    referee: Referee<Tasks>,
    accounting: crate::accounting_referee::Accounting,
    pending: Vec<Request>,
    out: Queue<Request>,
}
impl PartialEq for Internal {
    fn eq(&self, other: &Self) -> bool {
        // Equality explicitly asks for the complete private-state evidence.
        // This matches the former frozen strings, but routine sweeps format nothing.
        format!("{:?}", (&self.domain, &self.referee, &self.accounting, &self.pending, &self.out))
            == format!("{:?}", (&other.domain, &other.referee, &other.accounting, &other.pending, &other.out))
    }
}
impl Eq for Internal {}

#[derive(Debug)]
pub struct World {
    pub env: Env<Limits>,
    domain: Domain,
    out: Queue<Request>,
    pending: Vec<Request>,
    pub records: BTreeMap<Key, Stored>,
    pub replies: BTreeMap<u64, Reply>,
    pub activations: BTreeSet<u64>,
    activation_counts: BTreeMap<u64, u32>,
    /// Owned activation brief supplied by the child (domain/engine.md, 9).
    pub contexts: BTreeMap<u64, RunContext>,
    pub runs: BTreeMap<u64, u64>,
    pub stops: BTreeSet<(u64, u64)>,
    pub closing: BTreeSet<u64>,
    pub results: BTreeMap<u64, Ending>,
    pub ended_topics: Vec<(u64, u64, u16)>,
    pub facts: Vec<Fact>,
    pub accounting_referee: crate::accounting_referee::Accounting,
    pub consume_facts: bool,
    pub referee: Referee<Tasks>,
    pub trace: Trace,
    seed: u64,
    call: u64,
    message: u64,
    commit: u64,
    priced: Option<(u64, u64)>,
    restoring: bool,
    deadlines: BTreeMap<u64, (Wall, Time)>,
    kinds: Vec<tasks::Kind>,
    reports: BTreeMap<tasks::Name, tasks::HoldKind>,
}

impl World {
    #[must_use]
    pub fn new(seed: u64, limits: Limits) -> World {
        Self::start(seed, &limits, Trace::default())
    }

    fn start(seed: u64, limits: &Limits, trace: Trace) -> World {
        let mut world = World {
            env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits: *limits },
            domain: domain(limits, seed),
            out: Queue::with_capacity(tasks::max_out(limits)),
            pending: Vec::new(),
            records: BTreeMap::new(),
            replies: BTreeMap::new(),
            activations: BTreeSet::new(),
            activation_counts: BTreeMap::new(),
            contexts: BTreeMap::new(),
            runs: BTreeMap::new(),
            stops: BTreeSet::new(),
            closing: BTreeSet::new(),
            results: BTreeMap::new(),
            ended_topics: Vec::new(),
            facts: Vec::new(),
            accounting_referee: crate::accounting_referee::Accounting::default(),
            consume_facts: true,
            referee: Referee::new(Tasks::default()),
            trace,
            seed,
            call: 0,
            message: 0,
            commit: 0,
            priced: None,
            restoring: true,
            deadlines: BTreeMap::new(),
            kinds: Vec::new(),
            reports: BTreeMap::new(),
        };
        world.observe(Seen::Newest { revision: 1 });
        world.send(Event::Check);
        world.send(Event::Restored);
        world.open_period(0, 100_000);
        world
    }

    #[must_use]
    /// Capture all retained domain, parent and executor state between steps
    /// (domain/tasks.md, 11).
    pub fn frozen(self) -> Frozen {
        Frozen {
            records: self.records,
            replies: self.replies,
            activations: self.activations,
            activation_counts: self.activation_counts,
            contexts: self.contexts,
            runs: self.runs,
            stops: self.stops,
            closing: self.closing,
            results: self.results,
            ended_topics: self.ended_topics,
            deadlines: self.deadlines,
            kinds: self.kinds,
            reports: self.reports,
            restoring: self.restoring,
            facts: self.facts,
            consume_facts: self.consume_facts,
            limits: self.env.limits,
            seed: self.seed,
            priced: self.priced,
            internal: std::sync::Arc::new(Internal {
                domain: self.domain,
                referee: self.referee,
                accounting: self.accounting_referee,
                pending: self.pending,
                out: self.out,
            }),
            now: self.env.now,
            wall: self.env.wall,
            call: self.call,
            message: self.message,
            commit: self.commit,
        }
    }

    pub fn open_period(&mut self, period: u64, budget: u64) {
        let reply_to = self.to();
        self.send(Event::OpenPeriod { reply_to, project: 1, period, budget });
    }

    /// Install connector hold rules, replayed before live rows on restart.
    pub fn configure_holds(&mut self, connector: u16, kinds: Vec<tasks::Kind>) {
        self.kinds.extend_from_slice(&kinds);
        self.send(Event::Kinds { connector, kinds: kinds.into_boxed_slice() });
    }

    pub fn carve_pool(&mut self, period: u64, budget: u64) {
        let reply_to = self.to();
        self.send(Event::CarvePool { reply_to, project: 1, person: 9, period, budget });
    }

    pub fn to(&mut self) -> ReplyTo {
        self.call += 1;
        ReplyTo::new(Token::new(self.call))
    }

    pub fn observe(&mut self, seen: Seen) {
        self.trace.observed(self.env.now, &seen);
        let mut stimuli = Vec::new();
        self.referee.observe(self.env.now, seen, &mut stimuli);
        self.referee.assert_holding(self.seed);
        for stimulus in stimuli {
            match stimulus {
                Stimulus::Restart => self.restart(),
            }
        }
    }

    fn route_timer(&mut self, task: u64, subscription: u64) {
        self.message = self.message.checked_add(1).expect("message number room");
        tasks::step(
            &mut self.domain,
            &self.env,
            Event::Notice {
                task,
                word: Word {
                    number: self.message,
                    from: Party::Task(task),
                    kind: MessageKind::Timer { subscription },
                    words: Box::new([]),
                    at: self.env.wall,
                    hits: 1,
                    eligible: false,
                },
            },
            &mut self.out,
        );
    }

    fn route_hint(&mut self, request: &Request) {
        match request {
            Request::PersonStallDue { task } => {
                self.message = self.message.checked_add(1).expect("stall message number room");
                tasks::step(
                    &mut self.domain,
                    &self.env,
                    Event::PersonStallNotice { task: *task, message: self.message },
                    &mut self.out,
                );
            }
            Request::MetDue { task, dependency, removed, by } => {
                self.message = self.message.checked_add(1).expect("met message number room");
                tasks::step(
                    &mut self.domain,
                    &self.env,
                    Event::MetNotice {
                        task: *task,
                        dependency: *dependency,
                        removed: *removed,
                        by: *by,
                        message: self.message,
                    },
                    &mut self.out,
                );
            }
            Request::Notify { task, subscription, target, state, words } => {
                self.message = self.message.checked_add(1).expect("message number room");
                tasks::step(
                    &mut self.domain,
                    &self.env,
                    Event::Notice {
                        task: *task,
                        word: Word {
                            number: self.message,
                            from: Party::Task(*target),
                            kind: MessageKind::Notice { subscription: *subscription, target: *target, state: *state },
                            words: words.clone(),
                            at: self.env.wall,
                            hits: 1,
                            eligible: false,
                        },
                    },
                    &mut self.out,
                );
            }
            Request::Timer { task, subscription } => self.route_timer(*task, *subscription),
            Request::Taken { .. }
            | Request::Waiting { .. }
            | Request::WriterWaiting { .. }
            | Request::EndTopic { .. }
            | Request::ResolveUncertain { .. }
            | Request::Sent { .. }
            | Request::RecurringDue { .. }
            | Request::Relay { .. }
            | Request::EscalationsInspected { .. }
            | Request::EscalationsRechecked { .. }
            | Request::EscalationNeeded { .. }
            | Request::EscalationDecided { .. }
            | Request::ProposalDecided { .. }
            | Request::PersonProposed { .. }
            | Request::PersonProposalDecided { .. }
            | Request::ProposalRerouteNeeded { .. }
            | Request::ProposalStalled { .. }
            | Request::EscalationStalled { .. }
            | Request::Made { .. }
            | Request::ProcedureRefused { .. }
            | Request::CompoundRefused { .. }
            | Request::Refused { .. }
            | Request::Done { .. }
            | Request::Acknowledged { .. }
            | Request::TurnAcknowledged { .. }
            | Request::Activate { .. }
            | Request::Stop { .. }
            | Request::Adopt { .. }
            | Request::Close { .. }
            | Request::Release { .. }
            | Request::Ended { .. }
            | Request::Save { .. }
            | Request::Erase { .. }
            | Request::MovedToNewest { .. }
            | Request::Released { .. }
            | Request::RestoreRefused { .. } => {}
        }
    }

    #[expect(clippy::too_many_lines, reason = "the world records every closed task input family")]
    fn remember_input(&mut self, event: &Event) {
        self.priced = match event {
            Event::Amend { task, funder, amendment, .. } => {
                if let Some(Stored::Live(row)) = self.records.get(&Key::Live(*task)) {
                    let destination = funder.unwrap_or(row.funder);
                    let budget = match &amendment.authority {
                        Some(authority) => authority.budget.spend,
                        None => row.numbers.budget,
                    };
                    self.accounting_referee.amending(*task, destination, budget);
                }
                None
            }

            Event::Move { task, person, period, .. } => {
                self.accounting_referee.moving(*task, *person, *period);
                None
            }
            Event::Compound { by, period, operations, .. } => {
                let person = match by {
                    Party::Person(person) => *person,
                    Party::Task(_) | Party::Deployment { .. } => 0,
                };
                self.remember_compound(person, *period, operations);
                None
            }
            Event::Turn { task, cumulative, .. }
            | Event::Activation { task, cause: Cause::Priced { cumulative }, .. } => Some((*task, *cumulative)),
            Event::MoveToNewest { .. }
            | Event::Charters { .. }
            | Event::ResourceAccess { .. }
            | Event::Resource { .. }
            | Event::Kinds { .. }
            | Event::Slots { .. }
            | Event::AllocationGone { .. }
            | Event::ReadAfresh { .. }
            | Event::ChargeEffect { .. }
            | Event::EffectInFlight { .. }
            | Event::EffectSettled { .. }
            | Event::OpenPeriod { .. }
            | Event::PeriodNext { .. }
            | Event::TickRecurring { .. }
            | Event::RenewStanding { .. }
            | Event::RecurringBatch { .. }
            | Event::Procedure { .. }
            | Event::WakeProcedure { .. }
            | Event::TakePerson { .. }
            | Event::HandBackPerson { .. }
            | Event::AnswerPerson { .. }
            | Event::Control { .. }
            | Event::Propose { .. }
            | Event::ProposePerson { .. }
            | Event::WithdrawPersonProposal { .. }
            | Event::DecidePersonProposal { .. }
            | Event::DecideProposal { .. }
            | Event::DecideSettingsProposal { .. }
            | Event::WithdrawProposal { .. }
            | Event::StalledProposal { .. }
            | Event::CarvePool { .. }
            | Event::ResizePool { .. }
            | Event::Make { .. }
            | Event::MakeResultFollowups { .. }
            | Event::Prepare { .. }
            | Event::Claim { .. }
            | Event::Started { .. }
            | Event::Refusals { .. }
            | Event::Activation { cause: Cause::Unpriced, .. }
            | Event::PreparationWaiting { .. }
            | Event::PreparationFailed { .. }
            | Event::PreparationUnread { .. }
            | Event::Hold { .. }
            | Event::HoldFrom { .. }
            | Event::EffectsSettled { .. }
            | Event::Retained { .. }
            | Event::Settled { .. }
            | Event::Restore { .. }
            | Event::Check
            | Event::Restored
            | Event::InspectEscalations { .. }
            | Event::RecheckEscalations { .. }
            | Event::RoutedEscalation { .. }
            | Event::DecideEscalation { .. }
            | Event::Message { .. }
            | Event::AuthorizedMessage { .. }
            | Event::Introduce { .. }
            | Event::DelegateResult { .. }
            | Event::RememberStub { .. }
            | Event::Subscribe { .. }
            | Event::SubscribeTopic { .. }
            | Event::Unsubscribe { .. }
            | Event::MetNotice { .. }
            | Event::PersonStallNotice { .. }
            | Event::Notice { .. } => None,
        };
        match event {
            Event::DecideSettingsProposal { decision, .. } => {
                self.message = self.message.max(decision.message);
            }
            Event::Amend { message, .. } => {
                self.message = self.message.max(*message);
            }
            Event::Message { word, .. }
            | Event::AuthorizedMessage { word, .. }
            | Event::DelegateResult { word, .. }
            | Event::Notice { word, .. } => {
                self.message = self.message.max(word.number);
            }
            Event::Charters { .. }
            | Event::ResourceAccess { .. }
            | Event::Resource { .. }
            | Event::Kinds { .. }
            | Event::Slots { .. }
            | Event::AllocationGone { .. }
            | Event::ReadAfresh { .. }
            | Event::ChargeEffect { .. }
            | Event::EffectInFlight { .. }
            | Event::EffectSettled { .. }
            | Event::OpenPeriod { .. }
            | Event::PeriodNext { .. }
            | Event::TickRecurring { .. }
            | Event::RenewStanding { .. }
            | Event::RecurringBatch { .. }
            | Event::Procedure { .. }
            | Event::WakeProcedure { .. }
            | Event::TakePerson { .. }
            | Event::HandBackPerson { .. }
            | Event::AnswerPerson { .. }
            | Event::Control { .. }
            | Event::MoveToNewest { .. }
            | Event::Move { .. }
            | Event::Compound { .. }
            | Event::Propose { .. }
            | Event::ProposePerson { .. }
            | Event::WithdrawPersonProposal { .. }
            | Event::DecidePersonProposal { .. }
            | Event::DecideProposal { .. }
            | Event::WithdrawProposal { .. }
            | Event::StalledProposal { .. }
            | Event::CarvePool { .. }
            | Event::ResizePool { .. }
            | Event::Make { .. }
            | Event::MakeResultFollowups { .. }
            | Event::Prepare { .. }
            | Event::Claim { .. }
            | Event::Started { .. }
            | Event::Refusals { .. }
            | Event::Activation { .. }
            | Event::PreparationWaiting { .. }
            | Event::PreparationFailed { .. }
            | Event::PreparationUnread { .. }
            | Event::Hold { .. }
            | Event::HoldFrom { .. }
            | Event::EffectsSettled { .. }
            | Event::Retained { .. }
            | Event::Settled { .. }
            | Event::Restore { .. }
            | Event::Check
            | Event::Restored
            | Event::InspectEscalations { .. }
            | Event::RecheckEscalations { .. }
            | Event::RoutedEscalation { .. }
            | Event::DecideEscalation { .. }
            | Event::Introduce { .. }
            | Event::RememberStub { .. }
            | Event::Subscribe { .. }
            | Event::SubscribeTopic { .. }
            | Event::Unsubscribe { .. }
            | Event::MetNotice { .. }
            | Event::PersonStallNotice { .. }
            | Event::Turn { .. } => {}
        }
    }

    pub fn stage(&mut self, event: Event) {
        assert!(self.pending.is_empty(), "one parent decision at a time");
        if matches!(event, Event::Restored) {
            self.restoring = false;
        }
        if let Event::Procedure { task, step, read, .. } = &event {
            self.accounting_referee.procedure(*task, *step, *read);
        }
        let (event, procedure_reply) = if let Event::Procedure { reply_to, task, step, read, decision } = event {
            let token = reply_to.into_token();
            (Event::Procedure { reply_to: ReplyTo::new(token), task, step, read, decision }, Some(token))
        } else {
            (event, None)
        };
        if let Event::Resource { name, hold } = &event {
            self.reports.insert(name.clone(), *hold);
        }
        if let Event::Charters { newest, .. } = &event {
            self.observe(Seen::Newest { revision: *newest });
        }
        if let Event::Prepare { task, .. } = &event {
            self.observe(Seen::PreparationAsked { task: *task });
        }
        self.remember_input(&event);
        self.trace.input(self.env.now, &event);
        tasks::step(&mut self.domain, &self.env, event, &mut self.out);
        while let Some(request) = self.out.pop() {
            let request = if let Request::ProcedureRefused { problem, .. } = request {
                Request::Refused {
                    reply_to: ReplyTo::new(procedure_reply.expect("procedure refusal has its input")),
                    problem,
                }
            } else {
                request
            };
            self.route_hint(&request);
            if let Request::EscalationNeeded { context } = &request {
                tasks::step(
                    &mut self.domain,
                    &self.env,
                    Event::RoutedEscalation {
                        task: context.task,
                        revision: context.escalation.revision(),
                        holder: tasks::EscalationHolder::Person(context.requester),
                        entry: 1,
                    },
                    &mut self.out,
                );
            }
            if let Request::Ended { task, requester: Party::Task(parent), ending } = &request {
                let last = self.pending.iter().rev().find_map(|pending| {
                    if let Request::Save { record: Stored::Live(row) } = pending
                        && row.number == *parent
                    {
                        return Some(row.last_message);
                    }
                    None
                });
                let last = last.unwrap_or_else(|| self.record(*parent).last_message);
                let (kind, words) = result_notice(ending.clone());
                self.message = self.message.max(last).checked_add(1).expect("message number room");
                tasks::step(
                    &mut self.domain,
                    &self.env,
                    Event::DelegateResult {
                        task: *parent,
                        word: Word {
                            number: self.message,
                            from: Party::Task(*task),
                            kind: MessageKind::Result(kind),
                            words,
                            at: self.env.wall,
                            hits: 1,
                            eligible: false,
                        },
                    },
                    &mut self.out,
                );
            }
            self.pending.push(request);
        }
        if self.consume_facts {
            while let Some(fact) = self.domain.pop_fact() {
                self.facts.push(fact);
            }
        }
    }

    pub fn send(&mut self, event: Event) {
        self.stage(event);
        self.durable();
        self.deliver();
    }

    /// Commit state and requester results together before executor delivery
    /// (domain/tasks.md, 5; domain/engine.md, 7).
    pub fn durable(&mut self) {
        self.commit += 1;
        let mut terminals = Vec::new();
        let mut ordered = Vec::new();
        for request in &self.pending {
            match request {
                Request::Save { record } => {
                    self.saved_observations(record, &mut ordered);
                    self.records.insert(record.key(), record.clone());
                }
                Request::Erase { key } => {
                    self.records.remove(key);
                }
                Request::Ended { task, ending, .. } => {
                    assert!(self.results.insert(*task, ending.clone()).is_none(), "requester result committed once");
                    ordered.push(Seen::Ended { task: *task, status: status(ending), after: self.commit });
                }
                Request::Acknowledged { task, attempt, accepted: Accepted::New, .. } => {
                    terminals.push((*task, *attempt));
                }
                Request::Taken { .. }
                | Request::Waiting { .. }
                | Request::Made { .. }
                | Request::RecurringDue { .. }
                | Request::ProcedureRefused { .. }
                | Request::CompoundRefused { .. }
                | Request::Refused { .. }
                | Request::Done { .. }
                | Request::Acknowledged { accepted: Accepted::Already, .. }
                | Request::TurnAcknowledged { .. }
                | Request::Activate { .. }
                | Request::Stop { .. }
                | Request::Adopt { .. }
                | Request::Close { .. }
                | Request::Release { .. }
                | Request::MovedToNewest { .. }
                | Request::Released { .. }
                | Request::RestoreRefused { .. }
                | Request::EscalationNeeded { .. }
                | Request::EscalationsInspected { .. }
                | Request::EscalationsRechecked { .. }
                | Request::EscalationDecided { .. }
                | Request::ProposalDecided { .. }
                | Request::PersonProposed { .. }
                | Request::PersonProposalDecided { .. }
                | Request::ProposalRerouteNeeded { .. }
                | Request::ProposalStalled { .. }
                | Request::EscalationStalled { .. }
                | Request::ResolveUncertain { .. }
                | Request::Sent { .. }
                | Request::Relay { .. }
                | Request::MetDue { .. }
                | Request::PersonStallDue { .. }
                | Request::Notify { .. }
                | Request::Timer { .. }
                | Request::EndTopic { .. }
                | Request::WriterWaiting { .. } => {}
            }
        }
        if self.pending.iter().any(|request| {
            matches!(
                request,
                Request::Acknowledged { accepted: Accepted::New, .. }
                    | Request::TurnAcknowledged { accepted: Accepted::New, .. }
            )
        }) && let Some((task, cumulative)) = self.priced
        {
            self.accounting_referee.charged(task, cumulative);
        }
        self.priced = None;
        self.project_deadlines();
        if !self.restoring {
            self.accounting_referee.committed(&self.records, &self.pending).expect("independent conservation");
        }
        self.observe(Seen::Durable { commit: self.commit });
        self.observe_made();
        for (task, attempt) in terminals {
            self.runs.remove(&task);
            self.observe(Seen::Terminal { task, attempt });
        }
        for seen in ordered {
            self.observe(seen);
        }
    }

    fn saved_observations(&self, record: &Stored, ordered: &mut Vec<Seen>) {
        if let Stored::Live(row) = record
            && let Some(Stored::Live(before)) = self.records.get(&record.key())
            && row.requester != before.requester
        {
            ordered.push(Seen::Moved { task: row.number, parent: row.requester });
        }
        if let Stored::Live(row) = record
            && let Some(Stored::Live(before)) = self.records.get(&record.key())
        {
            if row.settings.task != before.settings.task {
                ordered.push(Seen::MovedToNewest { task: row.number, revision: row.settings.task });
            }
            if row.settings.run != before.settings.run
                && let Some(revision) = row.settings.run
            {
                ordered.push(Seen::Prepared { task: row.number, revision });
            }
        }
        if let Stored::History(history) = record
            && let Some(amendment) = &history.amendment
        {
            ordered.push(Seen::Amended { task: history.task, dependencies: amendment.after.dependencies.to_vec() });
        }
    }

    /// Each live row the referee has not met yet is made, as it was committed.
    fn observe_made(&mut self) {
        let made = self
            .records
            .values()
            .filter_map(live_row)
            .filter(|record| !self.referee.expectations().has_task(record.number))
            .map(|record| Seen::Made {
                task: record.number,
                parent: record.requester,
                dependencies: record.dependencies.to_vec(),
                depth: record.depth,
                settings: record.settings,
                newest: self.domain.newest(),
            })
            .collect::<Vec<_>>();
        for seen in made {
            self.observe(seen);
        }
    }

    #[expect(clippy::too_many_lines, reason = "the world dispatches every child output to its scripted owner")]
    pub fn deliver(&mut self) {
        for request in std::mem::take(&mut self.pending) {
            match request {
                Request::Taken { .. }
                | Request::Waiting { .. }
                | Request::Save { .. }
                | Request::RecurringDue { .. }
                | Request::Erase { .. }
                | Request::Ended { .. }
                | Request::EscalationNeeded { .. }
                | Request::Relay { .. }
                | Request::MetDue { .. }
                | Request::PersonStallDue { .. }
                | Request::Notify { .. }
                | Request::Timer { .. }
                | Request::ProposalRerouteNeeded { .. }
                | Request::ProposalStalled { .. }
                | Request::EscalationStalled { .. }
                | Request::ResolveUncertain { .. }
                | Request::MovedToNewest { .. }
                | Request::Released { .. }
                | Request::Release { .. } => {}
                Request::EndTopic { task, subscription, connector } => {
                    self.ended_topics.push((task, subscription, connector));
                }
                Request::WriterWaiting { reply_to, resource, .. } => {
                    self.reply(reply_to, Reply::WriterWaiting(resource));
                }
                Request::PersonProposalDecided { reply_to, .. } => {
                    if let Some(reply_to) = reply_to {
                        self.reply(reply_to, Reply::Done);
                    }
                }
                Request::PersonProposed { reply_to, .. }
                | Request::Sent { reply_to, .. }
                | Request::Done { reply_to }
                | Request::ProposalDecided { reply_to, .. } => self.reply(reply_to, Reply::Done),
                Request::Made { reply_to, tasks } => {
                    let token = reply_to.into_token();
                    if token.raw() != u64::MAX - 1 {
                        self.reply(ReplyTo::new(token), Reply::Made(tasks.into_vec()));
                    }
                }
                Request::Refused { reply_to, problem } | Request::CompoundRefused { reply_to, problem, .. } => {
                    self.reply(reply_to, Reply::Refused(problem));
                }
                Request::ProcedureRefused { .. } => unreachable!("stage translates procedure terminal"),
                Request::Acknowledged { reply_to, accepted, .. } => self.reply(reply_to, Reply::Acknowledged(accepted)),
                Request::TurnAcknowledged { reply_to, accepted, .. } => self.reply(reply_to, Reply::Turn(accepted)),
                Request::Activate { context } => {
                    self.activate(*context);
                }
                Request::Stop { task, attempt } => {
                    self.stops.insert((task, attempt));
                }
                Request::Adopt { task, attempt, kept } => {
                    assert_eq!(kept, self.record(task).turn, "adoption carries durable turn fence");
                    self.runs.insert(task, attempt);
                    self.observe(Seen::Assigned {
                        task,
                        attempt,
                        after: self.commit,
                        adopted: true,
                        settings: self.record(task).settings.run.expect("adopted run revision"),
                    });
                }
                Request::Close { task, .. } => {
                    self.closing.insert(task);
                    self.observe(Seen::Closing { task });
                }
                Request::RestoreRefused { problem } => panic!("world store corrupted: {problem:?}"),
                Request::EscalationsInspected { .. }
                | Request::EscalationsRechecked { .. }
                | Request::EscalationDecided { .. } => {
                    panic!("this child world sends no escalation decisions")
                }
            }
        }
        self.observe(Seen::Limit { live: self.live(), cap: self.env.limits.tasks });
        let live = self.records.values().filter_map(live_row).cloned().collect();
        self.observe(Seen::Stored { live, limits: Box::new(self.env.limits) });
        let mut live = Vec::new();
        let mut pools = Vec::new();
        let mut writers = Vec::new();
        for record in self.records.values() {
            match record {
                Stored::Live(row) => live.push(*row.clone()),
                Stored::Pool(row) => pools.push(row.clone()),
                Stored::Writer(row) => writers.push(row.clone()),
                Stored::Ended(_)
                | Stored::Stub(_)
                | Stored::Ledger(_)
                | Stored::ClosedLedger(_)
                | Stored::DecidedProposal(_)
                | Stored::Activity(_)
                | Stored::Milestone(_)
                | Stored::History(_)
                | Stored::Result(_)
                | Stored::EndedByParty(_)
                | Stored::EndedByProject(_)
                | Stored::EndedDelegate(_)
                | Stored::Delegate(_)
                | Stored::Update(_)
                | Stored::PersonProposal(_) => {}
            }
        }
        let mut reports = Vec::new();
        for (name, hold) in &self.reports {
            reports.push((name.clone(), *hold));
        }
        self.observe(Seen::Resources { live, pools, writers, kinds: self.kinds.clone(), reports });
        self.domain.reclaim();
    }

    fn reply(&mut self, to: ReplyTo, reply: Reply) {
        let call = to.into_token().raw();
        assert!(self.replies.insert(call, reply).is_none(), "one reply per call");
        self.observe(Seen::Replied { call, after: self.commit });
    }

    #[must_use]
    pub fn live(&self) -> usize {
        self.records.keys().filter(|key| matches!(key, Key::Live(_))).count()
    }

    /// Current holder eligibility as the core reads it (domain/tasks.md, 5.7 and section 9).
    #[must_use]
    pub fn delegation(&self, number: u64) -> tasks::DelegationContext {
        self.domain.delegation(number).expect("live task's delegation context")
    }

    #[must_use]
    pub fn record(&self, number: u64) -> &tasks::TaskRecord {
        match &self.records[&Key::Live(number)] {
            Stored::Live(record) => record,
            Stored::Ended(_)
            | Stored::Writer(_)
            | Stored::Result(_)
            | Stored::EndedByParty(_)
            | Stored::EndedByProject(_)
            | Stored::EndedDelegate(_)
            | Stored::Delegate(_)
            | Stored::Update(_)
            | Stored::Pool(_)
            | Stored::ClosedLedger(_)
            | Stored::DecidedProposal(_)
            | Stored::Stub(_)
            | Stored::Ledger(_)
            | Stored::Activity(_)
            | Stored::Milestone(_)
            | Stored::History(_)
            | Stored::PersonProposal(_) => {
                unreachable!("live key")
            }
        }
    }

    pub fn make(&mut self, creator: Party, batch: Vec<New>) -> Reply {
        let before = self.records.keys().copied().collect::<BTreeSet<_>>();
        let members = batch.iter().map(|task| task.number).collect();
        let reply_to = self.to();
        let call = self.call;
        self.send(Event::Make { reply_to, creator, batch: batch.into_boxed_slice() });
        let reply = self.replies[&call].clone();
        let made = self
            .records
            .keys()
            .filter_map(|key| match key {
                Key::Live(number) if !before.contains(key) => Some(*number),
                Key::Live(_)
                | Key::Writer(_)
                | Key::Pool(_)
                | Key::ClosedLedger(_)
                | Key::DecidedProposal(_)
                | Key::Ended(_)
                | Key::Stub(_)
                | Key::Ledger(_)
                | Key::Milestone { .. }
                | Key::Activity { .. }
                | Key::History { .. }
                | Key::Result { .. }
                | Key::EndedByParty { .. }
                | Key::EndedByProject { .. }
                | Key::EndedDelegate { .. }
                | Key::Delegate { .. }
                | Key::Update { .. }
                | Key::PersonProposal(_) => None,
            })
            .collect();
        self.observe(Seen::Batch { members, accepted: matches!(reply, Reply::Made(_)), made });
        reply
    }

    pub fn claim(&mut self, task: u64, attempt: u64) {
        let numbers = self.record(task).numbers;
        let budget = numbers.budget - numbers.spent - numbers.spent_below - numbers.reserved;
        self.claim_budget(task, attempt, budget);
    }

    pub fn claim_budget(&mut self, task: u64, attempt: u64, budget: u64) {
        assert_eq!(self.claim_budget_writing(task, attempt, budget, Box::new([])), Reply::Done);
    }

    pub fn claim_budget_writing(&mut self, task: u64, attempt: u64, budget: u64, writes: Box<[tasks::Name]>) -> Reply {
        assert!(self.activations.remove(&task), "activated before preparation");
        let context = &self.contexts[&task];
        let record = self.record(task);
        assert_eq!(context.spec, record.spec);
        assert_eq!(context.contract, record.contract);
        assert_eq!(context.authority, record.authority);
        let reply_to = self.to();
        self.send(Event::Prepare { reply_to, task });
        let reply_to = self.to();
        let call = self.call;
        self.send(Event::Claim { account: None, reply_to, task, attempt, resumed: false, budget, writes });
        let reply = self.replies[&call].clone();
        if reply == Reply::Done {
            assert!(self.runs.insert(task, attempt).is_none());
            self.observe(Seen::Assigned {
                task,
                attempt,
                after: self.commit,
                adopted: false,
                settings: self.record(task).settings.run.expect("claimed run revision"),
            });
            self.send(Event::Started { task, attempt });
        }
        reply
    }

    pub fn terminal(&mut self, task: u64, end: End) -> Reply {
        self.terminal_cause(task, end, Cause::Unpriced)
    }

    /// Submit a priced or recovery terminal (domain/tasks.md, 5).
    pub fn terminal_cause(&mut self, task: u64, end: End, cause: Cause) -> Reply {
        let attempt = self.runs[&task];
        let reply_to = self.to();
        let call = self.call;
        self.send(Event::Activation { reply_to, task, attempt, end, saved: None, cause });
        self.replies[&call].clone()
    }

    pub fn claim_fresh(&mut self, task: u64) {
        let attempt = self
            .records
            .values()
            .filter_map(|row| match row {
                Stored::Live(record) | Stored::Ended(record) => Some(record.attempt),
                Stored::Ledger(_)
                | Stored::Writer(_)
                | Stored::Result(_)
                | Stored::EndedByParty(_)
                | Stored::EndedByProject(_)
                | Stored::EndedDelegate(_)
                | Stored::Delegate(_)
                | Stored::Update(_)
                | Stored::Pool(_)
                | Stored::ClosedLedger(_)
                | Stored::DecidedProposal(_)
                | Stored::Stub(_)
                | Stored::Activity(_)
                | Stored::Milestone(_)
                | Stored::History(_)
                | Stored::PersonProposal(_) => None,
            })
            .max()
            .unwrap_or(0)
            + 1;
        self.claim(task, attempt);
    }

    pub fn finish(&mut self, task: u64) {
        self.terminal(
            task,
            End::Finished { result: TaskResult::Report { words: Box::new([1]) }, cancel_delegates: false },
        );
    }

    pub fn settle(&mut self, task: u64) {
        self.settle_effects(task);
        self.release(task);
    }

    pub fn settle_effects(&mut self, task: u64) {
        assert!(self.closing.contains(&task));
        self.send(Event::EffectsSettled { task });
    }

    pub fn release(&mut self, task: u64) {
        assert!(self.closing.remove(&task));
        self.observe(Seen::Settled { task });
        self.send(Event::Settled { task });
    }

    fn activate(&mut self, context: RunContext) {
        *self.activation_counts.entry(context.task).or_default() += 1;
        self.activations.insert(context.task);
        self.contexts.insert(context.task, context);
    }

    /// Count delivered preparations, including deliveries before a restart.
    #[must_use]
    pub fn activations(&self, task: u64) -> u32 {
        self.activation_counts.get(&task).copied().unwrap_or(0)
    }

    pub fn advance(&mut self) {
        let at = self.deadlines.values().map(|(_, at)| *at).min().expect("backoff pending");
        self.elapse(Duration::from_nanos(at.as_nanos().saturating_sub(self.env.now.as_nanos())));
    }

    /// Advance the controlled world clock and run the task timers once.
    pub fn elapse(&mut self, by: Duration) {
        self.env.wall = Wall::from_nanos(self.env.wall.as_nanos().saturating_add(by.as_nanos()));
        self.env.now = self.env.now.saturating_add(by);
        tasks::fire(&mut self.domain, &self.env, &mut self.out);
        while let Some(request) = self.out.pop() {
            self.route_hint(&request);
            self.pending.push(request);
        }
        self.durable();
        self.deliver();
    }

    fn project_deadlines(&mut self) {
        let mut projected = BTreeMap::new();
        for row in self.records.values() {
            if let Stored::Live(task) = row
                && let tasks::Phase::Active(tasks::Active::BackingOff { until }) = task.phase
            {
                let at = match self.deadlines.get(&task.number) {
                    Some((old, at)) if *old == until => *at,
                    Some(_) | None => self.env.now.saturating_add(Duration::from_nanos(
                        until.as_nanos().saturating_sub(self.env.wall.as_nanos()),
                    )),
                };
                projected.insert(task.number, (until, at));
            }
        }
        self.deadlines = projected;
    }

    pub fn restart(&mut self) {
        self.pending.clear();
        self.priced = None;
        self.restoring = true;
        self.deadlines.clear();
        self.accounting_referee.reset(&self.records);
        self.domain = domain(&self.env.limits, self.seed);
        let kinds = self.kinds.clone();
        for configured in kinds {
            self.send(Event::Kinds { connector: configured.connector, kinds: Box::new([configured]) });
        }
        for (name, hold) in self.reports.clone() {
            self.send(Event::Resource { name, hold });
        }
        self.activations.clear();
        self.contexts.clear();
        self.stops.clear();
        self.closing.clear();
        let rows = self.records.values().filter(|row| row.key().live()).cloned().collect::<Vec<_>>();
        for record in rows {
            self.send(Event::Restore { record });
        }
        self.send(Event::Check);
        self.send(Event::Restored);
    }

    /// Complete the stopped descendants after an accepted delegate-cancelling
    /// terminal (domain/tasks.md, 5.6).
    pub fn complete_cancel(&mut self) {
        for (task, _) in self.stops.clone() {
            if self.runs.contains_key(&task) {
                self.terminal(task, End::Parked);
            }
        }
        while let Some(task) = self.closing.iter().next().copied() {
            self.settle(task);
        }
        self.observe(Seen::Finished);
        self.referee.assert_passed(self.seed);
    }

    /// Report and retain the connector's latest named rule (domain/tasks.md, section 6).
    pub fn report_resource(&mut self, name: tasks::Name, hold: tasks::HoldKind) {
        self.send(Event::Resource { name, hold });
    }
}

fn status(ending: &Ending) -> tasks::Status {
    match ending {
        Ending::Done(_) => tasks::Status::Done,
        Ending::Failed { .. } => tasks::Status::Failed,
        Ending::Cancelled { .. } => tasks::Status::Cancelled,
    }
}

fn result_notice(ending: Ending) -> (ResultKind, Box<[u8]>) {
    match ending {
        Ending::Done(result) => match result {
            TaskResult::Approved { state, words } => {
                let words_present = words.is_some();
                let words = match words {
                    Some(words) => words,
                    None => Box::new([]),
                };
                (ResultKind::Approved { state, words_present }, words)
            }
            TaskResult::Report { words } => (ResultKind::Report, words),
            TaskResult::Verdict { code, words, fields } => (ResultKind::Verdict { code, fields }, words),
            TaskResult::Change { connector, kind, resource, words } => {
                (ResultKind::Change { connector, kind, resource }, words)
            }
            TaskResult::Failure { reason } => (ResultKind::Failed, reason),
        },
        Ending::Failed { reason } => (ResultKind::Failed, reason),
        Ending::Cancelled { reason, .. } => (ResultKind::Cancelled, reason),
    }
}

/// Seeded dependency, priced work, retry, crash and delegate-close story
/// (domain/tasks.md, 11). Replay includes complete frozen state.
#[must_use]
pub fn run_story(seed: u64) -> (Trace, Frozen) {
    run_story_facts(seed, true)
}

#[must_use]
pub fn run_story_facts(seed: u64, consume_facts: bool) -> (Trace, Frozen) {
    story(seed, consume_facts, Trace::default(), None)
}

/// Replay one complete story with an explicit diagnostic setting.
#[must_use]
pub fn run_story_traced(seed: u64, consume_facts: bool, enabled: bool) -> (Trace, Frozen) {
    story(seed, consume_facts, Trace::with_enabled(enabled), None)
}

/// Run one enumerated choice combination with drawn arrival/pricing times.
#[must_use]
pub fn run_story_choices(seed: u64, choices: Story) -> (Trace, Frozen) {
    story(seed, true, Trace::default(), Some(choices))
}

fn story(seed: u64, consume_facts: bool, trace: Trace, choices: Option<Story>) -> (Trace, Frozen) {
    let mut w = World::start(seed, &LIMITS, trace);
    w.consume_facts = consume_facts;
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| story_run(seed, &mut w, choices))).unwrap_or_else(
        |cause| {
            let detail = cause
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| cause.downcast_ref::<&str>().copied())
                .unwrap_or("non-string panic");
            panic!("task seed {seed}, choices {choices:?}: {detail}\n{}", w.trace.lines.join("\n"));
        },
    );
    (std::mem::take(&mut w.trace), w.frozen())
}

fn story_run(seed: u64, w: &mut World, choices: Option<Story>) {
    let mut rng = Rng::new(seed);
    let arrival = Duration::from_millis(rng.between(1, 20));
    w.env.now = w.env.now.saturating_add(arrival);
    w.env.wall = Wall::from_nanos(arrival.as_nanos());
    w.make(Party::Person(1), vec![task(1, &[])]);
    w.claim(1, 1);
    w.make(Party::Task(1), vec![task(2, &[]), task(3, &[2])]);
    w.claim_fresh(2);
    let reply_to = w.to();
    w.stage(Event::Turn {
        refused: Box::new([]),
        reply_to,
        task: 1,
        attempt: 1,
        turn: 1,
        read: None,
        offered: None,
        outstanding: Box::new([]),
        cumulative: 5,
    });
    // Defaults remain eager draws at their original positions: an ordinary
    // seeded replay's random stream and every action stay unchanged.
    if choices.map_or(rng.chance(500), |story| story.turn == TurnState::Committed) {
        w.durable();
    }
    w.restart();
    let drawn_retry = match rng.below(3) {
        0 => RetryEnd::Refused,
        1 => RetryEnd::Lost,
        2 => RetryEnd::None,
        _ => unreachable!(),
    };
    match choices.map_or(drawn_retry, |story| story.retry) {
        RetryEnd::Refused => {
            w.terminal(2, End::Refused);
            w.advance();
            w.claim_fresh(2);
        }
        RetryEnd::Lost => {
            w.terminal(2, End::Failed(tasks::Class::Lost));
            w.advance();
            w.claim_fresh(2);
        }
        RetryEnd::None => {}
    }
    if choices.map_or(rng.chance(500), |story| story.restart_after_retry == Restart::Restore) {
        w.restart();
    }
    if choices.map_or(rng.chance(200), |story| story.child == ChildResult::Failed) {
        w.terminal(2, End::Finished { result: TaskResult::Failure { reason: Box::new([1]) }, cancel_delegates: false });
        w.settle(2);
        assert!(matches!(w.record(3).phase, tasks::Phase::Held { .. }));
    } else {
        w.terminal_cause(
            2,
            End::Finished { result: TaskResult::Report { words: Box::new([1]) }, cancel_delegates: false },
            Cause::Priced { cumulative: rng.between(1, 10) },
        );
        w.settle(2);
        w.claim_fresh(3);
        w.finish(3);
        w.settle(3);
    }
    w.make(Party::Task(1), vec![task(4, &[])]);
    w.claim_fresh(4);
    w.make(Party::Task(4), vec![task(5, &[])]);
    w.claim_fresh(5);
    if choices.map_or(rng.chance(500), |story| story.restart_before_close == Restart::Restore) {
        w.restart();
    }
    for task in [3, 4] {
        if w.records.contains_key(&Key::Live(task)) {
            w.observe(Seen::Cancelled { task });
        }
    }
    w.terminal(1, End::Finished { result: TaskResult::Report { words: Box::new([1]) }, cancel_delegates: true });
    w.complete_cancel();
    assert_eq!(w.results.len(), 5);
    assert_eq!(w.live(), 0);
}

fn live_row(record: &Stored) -> Option<&jig_domain_tasks::TaskRecord> {
    match record {
        Stored::Live(record) => Some(record),
        Stored::Ended(_)
        | Stored::Stub(_)
        | Stored::Ledger(_)
        | Stored::Writer(_)
        | Stored::Pool(_)
        | Stored::PersonProposal(_)
        | Stored::History(_)
        | Stored::Milestone(_)
        | Stored::Activity(_)
        | Stored::Result(_)
        | Stored::EndedByParty(_)
        | Stored::EndedByProject(_)
        | Stored::EndedDelegate(_)
        | Stored::Delegate(_)
        | Stored::Update(_)
        | Stored::ClosedLedger(_)
        | Stored::DecidedProposal(_) => None,
    }
}
