//! Live task and finite-source ownership, step dispatch and retry scheduling
//! (domain/tasks.md, sections 2, 5 and 10). Root supplies authorized events
//! and iteration time, routes outputs and owns durability/transport proofs.
//! A procedure keeps the transient inbox prefix offered to its current step;
//! restoring reoffers its durable unread inbox. Tasks never performs IO or keeps
//! full historical results or root shadows.
use crate::{Active, Event, Fact, Limits, New, Party, Phase, Problem, Refusal, Request, Stored, TaskRecord, Tries};
use alloc::boxed::Box;
use skein_lib::{Deadlines, Env, Id, List, Map, Queue, ReplyTo, Rng, Slab, Time, Wall};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Alarm {
    pub until: Wall,
    pub due: Time,
}

#[derive(Debug)]
pub(crate) struct Task {
    pub published: Option<Box<TaskRecord>>,
    pub record: TaskRecord,
    pub alarm: Option<Alarm>,
    pub grace_alarm: Option<Alarm>,
    pub procedure_offered: Option<u64>,
    pub observed_hold: Option<crate::Hold>,
    pub observed_phase: Option<Phase>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Startup {
    Restoring,
    Ready,
    Failed,
}

/// Bounded live task arena, indexed waits, retry timers, finite period/pool ledgers,
/// configured charters and deterministic retry randomness. Keeps bounded historical stubs,
/// transport receipt, connector state or mutable root shadow ledger. (domain/tasks.md, sections 2, 4–5 and 10).
#[derive(Debug)]
pub struct Domain {
    pub(crate) startup: Startup,
    pub(crate) waits: crate::waits::Index,
    pub(crate) circle_changes: crate::waits::CircleChanges,
    pub(crate) tasks: Slab<Task>,
    pub(crate) names: Map<u64, Id<Task>>,
    pub(crate) live_agents: u32,
    pub(crate) newest: u32,
    pub(crate) holders: Map<u32, u32>,
    pub(crate) stubs: Map<u64, crate::Stub>,
    pub(crate) alarms: Deadlines<u64>,
    pub(crate) grace_alarms: Deadlines<u64>,
    pub(crate) timers: Deadlines<u64>,
    pub(crate) wakes: Deadlines<u64>,
    pub(crate) proposal_alarms: Deadlines<u64>,
    pub(crate) escalation_alarms: Deadlines<u64>,
    pub(crate) hold_alarms: Deadlines<u64>,
    pub(crate) hold_kinds: Map<crate::holds::KindKey, crate::HoldKind>,
    pub(crate) resource_access: Map<crate::holds::ResourceKey, crate::ResourceAccess>,
    pub(crate) resource_access_full: bool,
    pub(crate) resources: Map<crate::Name, crate::HoldKind>,
    pub(crate) pools: Map<crate::Name, crate::PoolSlots>,
    pub(crate) next_pool: u64,
    pub(crate) writers: Map<crate::Name, crate::WriterSlot>,
    pub(crate) next_writer: u64,
    pub(crate) funding: Map<crate::Funder, crate::FundingRecord>,
    pub(crate) person_proposals: Map<u64, crate::PersonProposal>,
    pub(crate) charters: Box<[crate::CharterModels]>,
    /// Each configured model's count of declared efforts, in configuration's order.
    pub(crate) efforts: Box<[u8]>,
    pub(crate) rng: Rng,
    facts: Queue<Fact>,
    lost: u64,
    pub(crate) room_refusals: Queue<crate::CapacityRefusal>,
    pub(crate) room_refusals_lost: u64,
    pub(crate) room_refusals_mark: u64,
}

/// One bounded live row projected for an authenticated tree or goal watch.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ViewTask {
    pub number: u64,
    pub project: u32,
    pub requester: Party,
    pub phase: u32,
    pub tracked: Option<u32>,
}

/// Stable codes carried by the views child; details stay in task history.
#[must_use]
pub fn view_phase(phase: &Phase) -> u32 {
    match phase {
        Phase::Waiting => 0,
        Phase::Active(_) => 1,
        Phase::Closing(_) => 2,
        Phase::Held { .. } => 3,
        Phase::Ended(_) => 4,
    }
}

impl Domain {
    /// Total live agent tasks, across settings revisions (engine.md, 17.3).
    #[must_use]
    pub const fn live_agent_tasks(&self) -> u32 {
        self.live_agents
    }

    /// Earliest armed monotonic task deadline; observing it expires nothing.
    #[must_use]
    pub fn next_deadline(&self) -> Option<Time> {
        let mut next: Option<Time> = None;
        for at in [
            self.alarms.next(),
            self.grace_alarms.next(),
            self.timers.next(),
            self.wakes.next(),
            self.proposal_alarms.next(),
            self.escalation_alarms.next(),
            self.hold_alarms.next(),
        ]
        .into_iter()
        .flatten()
        {
            next = Some(match next {
                Some(previous) => previous.min(at),
                None => at,
            });
        }
        next
    }

    /// Inspect the bounded waiting escalation cohort before an atomic role edit.
    pub fn project_escalations(
        &self,
        limits: &Limits,
        project: u32,
    ) -> Result<Box<[crate::EscalationContext]>, Refusal> {
        crate::escalation::project_contexts(self, limits, project)
    }

    /// One bounded walk returning the first tasks incompatible with prospective charters,
    /// plus the exact count omitted (engine.md, 17.2; M3's newest-only rule).
    #[must_use]
    pub fn incompatible_charters(&self, charters: &[crate::CharterModels], limit: u32) -> (Box<[u64]>, u32) {
        let mut tasks = List::with_capacity(limit);
        let mut more = 0_u32;
        for (number, _) in &self.names {
            let row = record(self, *number).expect("indexed live task");
            match row.phase {
                Phase::Ended(_) => continue,
                Phase::Waiting | Phase::Active(_) | Phase::Closing(_) | Phase::Held { .. } => {}
            }
            let charter = match row.executor {
                crate::Executor::Agent { charter } => charter,
                crate::Executor::Procedure { .. } | crate::Executor::Person(_) => continue,
            };
            let mut models = None;
            let mut revision = 0;
            for offered in charters {
                if offered.number == charter && offered.revision >= revision {
                    revision = offered.revision;
                    models = offered.models.as_deref();
                }
            }
            let incompatible = match models {
                None => true,
                Some(models) => match &row.llm {
                    None => false,
                    Some(choice) => {
                        let mut fits = models.contains(&choice.main.model);
                        for model in &choice.subagents {
                            fits &= models.contains(&model.model);
                        }
                        !fits
                    }
                },
            };
            if incompatible {
                if tasks.room() != 0 {
                    tasks.push(*number).expect("bounded named tasks");
                } else {
                    more = more.checked_add(1).expect("bounded live tasks");
                }
            }
        }
        (tasks.into_boxed(), more)
    }

    /// Borrowed live projection copied into a bounded result for snapshot construction.
    #[must_use]
    pub fn view_tasks(&self) -> Box<[ViewTask]> {
        let mut rows = List::with_capacity(self.names.capacity());
        for (number, _) in &self.names {
            let row = record(self, *number).expect("indexed live row");
            rows.push(ViewTask {
                number: *number,
                project: row.project,
                requester: row.requester,
                phase: view_phase(&row.phase),
                tracked: row.tracked,
            })
            .expect("one row per live name");
        }
        rows.into_boxed()
    }
    /// Create restoring state from validated `limits`, deterministic `seed`, unique configured
    /// charter versions bounded by `limits.charters + limits.versions`, and `efforts`, each
    /// configured model's count of declared efforts in configuration's order, at most
    /// `MODELS` (domain/tasks.md, section 3). Panics on invalid/unrepresentable limits or
    /// malformed charter configuration; no task becomes active until `Restore`/`Restored`
    /// completes.
    #[must_use]
    pub fn new(
        limits: &Limits,
        seed: u64,
        newest: u32,
        charters: Box<[crate::CharterModels]>,
        efforts: Box<[u8]>,
    ) -> Domain {
        assert!(crate::worst_case(limits).is_some(), "task limits are valid");
        validate_charters(limits, &charters, &efforts);
        Domain {
            startup: Startup::Restoring,
            waits: crate::waits::Index::new(limits),
            circle_changes: crate::waits::CircleChanges::new(limits),
            tasks: Slab::with_capacity(limits.tasks),
            names: Map::with_capacity(limits.tasks),
            live_agents: 0,
            newest,
            holders: Map::with_capacity(limits.revisions.checked_add(1).expect("revision bound")),
            stubs: Map::with_capacity(crate::limits::stub_capacity(limits).expect("valid stub bound")),
            alarms: Deadlines::with_capacity(limits.tasks),
            grace_alarms: Deadlines::with_capacity(limits.tasks),
            timers: Deadlines::with_capacity(limits.tasks.checked_mul(limits.subscriptions).expect("timer room")),
            wakes: Deadlines::with_capacity(limits.tasks),
            proposal_alarms: Deadlines::with_capacity(limits.tasks),
            escalation_alarms: Deadlines::with_capacity(limits.tasks),
            hold_alarms: Deadlines::with_capacity(limits.tasks),
            hold_kinds: Map::with_capacity(limits.hold_kinds),
            resource_access: Map::with_capacity(limits.resource_reports),
            resource_access_full: false,
            resources: Map::with_capacity(limits.resource_reports),
            pools: Map::with_capacity(limits.pools),
            next_pool: 0,
            writers: Map::with_capacity(limits.tasks.checked_mul(limits.holdings).expect("writer room")),
            next_writer: 0,
            funding: Map::with_capacity(limits.funders),
            person_proposals: Map::with_capacity(limits.tasks),
            charters,
            efforts,
            rng: Rng::new(seed),
            facts: Queue::with_capacity(limits.facts),
            lost: 0,
            room_refusals: Queue::with_capacity(limits.room_refusals),
            room_refusals_lost: 0,
            room_refusals_mark: 0,
        }
    }

    /// Borrow the configured charters and their models admitted by this task hub.
    #[must_use]
    pub fn charters(&self) -> &[crate::CharterModels] {
        &self.charters
    }

    /// Pure borrowed lookup of authentic period/pool accounting for root policy checks; returns
    /// `None` when absent. The table is bounded by `Limits::funders`; no readiness transition,
    /// allocation or output occurs and the root must not persist a second mutable ledger.
    #[must_use]
    pub fn funding(&self, funder: crate::Funder) -> Option<&crate::FundingRecord> {
        self.funding.get(&funder)
    }

    /// Borrow all authentic external ledgers; callers filter their project and period.
    #[must_use]
    pub const fn funding_records(&self) -> &Map<crate::Funder, crate::FundingRecord> {
        &self.funding
    }

    /// Borrow the retained pending person-origin proposals for a project census.
    #[must_use]
    pub const fn person_proposals(&self) -> &Map<u64, crate::PersonProposal> {
        &self.person_proposals
    }

    /// Pure remaining ledger slots for a root-administered period/pool pair.
    #[must_use]
    pub fn funding_room(&self) -> u32 {
        self.funding.capacity().saturating_sub(self.funding.len())
    }

    /// Current opaque executor identity for root procedure routing.
    #[must_use]
    pub fn executor(&self, task: u64) -> Option<crate::Executor> {
        Some(record(self, task)?.executor)
    }

    /// Stable tree root for a connector's task-scoped names.
    #[must_use]
    pub fn root(&self, task: u64) -> Option<u64> {
        Some(record(self, task)?.root)
    }

    /// Borrowed current task facts for a connector projection in the same root decision.
    #[must_use]
    pub fn task(&self, number: u64) -> Option<&TaskRecord> {
        record(self, number)
    }

    /// Reservation a new requester must fund, including this task's live reservations
    /// but excluding spend already charged (domain/authority.md, 7).
    #[must_use]
    pub fn takeover_budget(&self, number: u64) -> Option<u64> {
        crate::funders::remaining(record(self, number)?.numbers)
    }

    /// Borrow an ended result reference still named by a live task; content is stored history.
    #[must_use]
    pub fn stub(&self, number: u64) -> Option<&crate::Stub> {
        self.stubs.get(&number)
    }

    /// The latest connector report for admission of this individual resource.
    #[must_use]
    pub fn resource_hold(&self, name: &crate::Name) -> Option<crate::HoldKind> {
        self.resources.get(name).copied()
    }

    /// Current authentic holder of an opaque exclusive resource.
    #[must_use]
    pub fn holder(&self, resource: &crate::Name) -> Option<u64> {
        crate::holds::writer_holder(self, resource)
    }

    /// Current authentic writer slot, including a lost attempt awaiting a fresh read.
    #[must_use]
    pub fn writer(&self, resource: &crate::Name) -> Option<&crate::WriterSlot> {
        self.writers.get(resource)
    }

    /// Whether one held resource may admit this effect, without taking its slot.
    /// The core checks the complete set before asking for any slot.
    #[must_use]
    pub fn effect_writer_ready(&self, limits: &Limits, task: u64, resource: &crate::Name, entry: Option<u64>) -> bool {
        crate::writers::effect_ready(self, limits, task, resource, entry)
    }

    /// Lost resources of this fenced run, whose owning connector owes fresh reads.
    #[must_use]
    pub fn lost_writes(&self, task: u64, attempt: u64) -> Box<[crate::Name]> {
        let mut names = List::with_capacity(self.writers.capacity());
        for (name, slot) in &self.writers {
            if slot.lost && slot.writer == (crate::Writer::Run { task, attempt }) {
                names.push(name.clone()).expect("bounded lost writer names");
            }
        }
        names.into_boxed()
    }

    /// Current resource slots for a connector entry, including restored flights.
    #[must_use]
    pub fn effect_writers(&self, entry: u64) -> Box<[crate::Name]> {
        let mut names = List::with_capacity(self.writers.capacity());
        for (name, slot) in &self.writers {
            if slot.writer == (crate::Writer::Effect { entry }) {
                names.push(name.clone()).expect("bounded writer names");
            }
        }
        names.into_boxed()
    }

    /// Current deployment-owned core recurring task identities for one project.
    #[must_use]
    pub fn recurring_tasks(&self, project: u32) -> Box<[u64]> {
        let mut numbers = List::with_capacity(self.names.capacity());
        for (number, _) in &self.names {
            let row = record(self, *number).expect("live name");
            if row.project == project && row.recurring.is_some() {
                numbers.push(*number).expect("one identity per task");
            }
        }
        numbers.into_boxed()
    }

    /// Current subscribed top-level connector procedures eligible for period renewal.
    #[must_use]
    pub fn standing_tasks(&self, project: u32) -> Box<[u64]> {
        let mut numbers = List::with_capacity(self.names.capacity());
        for (number, _) in &self.names {
            let row = record(self, *number).expect("live name");
            let procedure = match row.executor {
                crate::Executor::Procedure { .. } => true,
                crate::Executor::Agent { .. } | crate::Executor::Person(_) => false,
            };
            if row.project == project
                && row.root == *number
                && procedure
                && row.recurring.is_none()
                && !row.subscriptions.is_empty()
            {
                numbers.push(*number).expect("one identity per task");
            }
        }
        numbers.into_boxed()
    }

    /// Borrow a recurring task's durable template for root authority checks.
    #[must_use]
    pub fn recurring_template(&self, task: u64) -> Option<&crate::RecurringTemplate> {
        Some(&record(self, task)?.recurring.as_ref()?.template)
    }

    /// Next fenced step for a due procedure, or none when it is not ready to step.
    #[must_use]
    pub fn procedure_due(&self, task: u64) -> Option<(u16, u32, u64)> {
        let record = record(self, task)?;
        if record.phase != Phase::Active(Active::Due) {
            return None;
        }
        if record.recurring.is_some() {
            return None;
        }
        match record.executor {
            crate::Executor::Agent { .. } | crate::Executor::Person(_) => None,
            crate::Executor::Procedure { connector, code } => Some((connector, code, record.attempt.checked_add(1)?)),
        }
    }

    /// Inbox prefix offered to the current procedure step; rebuilt from unread rows on restart.
    #[must_use]
    pub fn procedure_offered(&self, task: u64) -> Option<u64> {
        self.tasks.get(*self.names.get(&task)?).expect("indexed task").procedure_offered
    }

    /// Borrowed current creation ceiling; no mutable task ledger is copied
    /// into the root or retained after this call's decision.
    #[must_use]
    pub fn delegation(&self, task: u64) -> Option<crate::DelegationContext> {
        let record = record(self, task)?;
        let made_below = record.made.checked_sub(1)?;
        let tasks_left = record.authority.delegation.tasks.checked_sub(made_below)?;
        Some(crate::DelegationContext {
            project: record.project,
            requester: record.requester,
            deciding: !crate::run::ends::kept(self, task).blocks_claims()
                && match record.phase {
                    Phase::Waiting | Phase::Active(_) => match record.executor {
                        crate::Executor::Agent { .. } => true,
                        crate::Executor::Procedure { .. } | crate::Executor::Person(_) => false,
                    },
                    Phase::Closing(_) | Phase::Held { .. } | Phase::Ended(_) => false,
                },
            authority: record.authority.clone(),
            numbers: record.numbers,
            tasks_left,
        })
    }

    /// Clone one bounded current proposal for a root authority and holder decision.
    #[must_use]
    pub fn proposal(&self, proposer: u64, number: u64) -> Option<crate::Proposal> {
        crate::proposals::context(self, proposer, number)
    }

    /// Whether this exact pending proposal was made by the proposer's result.
    #[must_use]
    pub fn result_proposal(&self, proposer: u64, number: u64) -> bool {
        match record(self, proposer) {
            Some(row) => match &row.proposal {
                Some(proposal) => row.result_proposal && proposal.number == number,
                None => false,
            },
            None => false,
        }
    }

    /// Clone one pending person-origin goal proposal for the root's authority check.
    #[must_use]
    pub fn person_proposal(&self, proposer: u64, number: u64) -> Option<crate::PersonProposal> {
        let proposal = self.person_proposals.get(&number)?;
        if proposal.proposer == proposer { Some(proposal.clone()) } else { None }
    }

    /// Clone one bounded held decision context for root's current route.
    #[must_use]
    pub fn escalation(&self, task: u64) -> Option<Box<crate::EscalationContext>> {
        crate::escalation::context(self, task)
    }

    /// The oldest inbox word or pending decision numbered past `after` (tasks.md, 8.2).
    #[must_use]
    pub fn next_waiting(&self, task: u64, after: Option<u64>) -> Option<crate::Word> {
        crate::waiting::next(self, task, after)
    }

    #[must_use]
    pub(crate) fn ready(&self) -> bool {
        self.startup == Startup::Ready
    }

    /// Whether an existing task has a different requester and a mutable phase;
    /// authority and funding remain the parent's and move preflight's checks.
    #[must_use]
    pub fn takeover_state(&self, number: u64, person: u64) -> bool {
        crate::moving::state(self, number, person)
    }

    /// Reclaim retired live task slots at the parent's iteration reclaim point after outputs have
    /// been routed; emits no persistence or lifecycle output.
    pub fn reclaim(&mut self) {
        self.tasks.reclaim();
    }

    /// Remove one optional content-free observation from the bounded diagnostic queue; keeping or
    /// dropping facts changes no decision, durability barrier or reply.
    pub fn pop_fact(&mut self) -> Option<Fact> {
        self.facts.pop()
    }
}

pub(crate) fn output_bound(limits: &Limits) -> Option<u32> {
    limits
        .tasks
        .checked_mul(28)?
        .checked_add(limits.batch.checked_mul(30)?)?
        .checked_add(limits.funders.checked_mul(3)?)?
        .checked_add(limits.tasks.checked_mul(limits.subscriptions)?.checked_mul(2)?)?
        .checked_add(crate::limits::stub_capacity(limits)?)?
        .checked_add(limits.tasks.checked_mul(limits.holdings)?.checked_mul(3)?)?
        .checked_add(crate::activity::lines_per_decision(limits)?)?
        // Each party proposal terminal uses four outputs: pending erase, update
        // index, terminal row and notification, within this eight-per-row room.
        .checked_add(limits.tasks.checked_mul(8)?)?
        .checked_add(8)
}

/// Required free `Request` slots for one `step` or `fire` under validated `limits`, including
/// a possible stale-stub erase for every bounded live name. Cascades are bounded by the live
/// set; panics if bound arithmetic is invalid. Caller counts output payload copies separately.
#[must_use]
pub fn max_out(limits: &Limits) -> u32 {
    output_bound(limits).expect("task limits admit output bound")
}

/// Apply one root-issued event to `domain` using the iteration clocks and immutable limits in
/// `env`, then advance bounded dependency/closing cascades when ready. Caller reserves `max_out`
/// free slots. Reply-bearing inputs produce one terminal reply; notifications may emit no output.
/// Root checks authority, fences exact transport replay and commits saves/erases with resulting
/// effects before external replies.
#[expect(clippy::too_many_lines, reason = "the closed task event vocabulary dispatches to focused handlers")]
pub fn step(domain: &mut Domain, env: &Env<Limits>, event: Event, out: &mut Queue<Request>) {
    match event {
        Event::Charters { newest, charters, efforts } => {
            validate_charters(&env.limits, &charters, &efforts);
            domain.newest = newest;
            domain.charters = charters;
            domain.efforts = efforts;
        }
        Event::Compound { reply_to, by, project, operations, period, pool_budget, period_budget } => {
            crate::compound::compound(
                domain,
                env,
                crate::compound::Compound { reply_to, by, project, operations, period, pool_budget, period_budget },
                out,
            );
        }
        Event::ResourceAccess { project, name, access } => {
            crate::holds::access(domain, env, project, name, access, out);
        }
        Event::Resource { name, hold } => crate::holds::resource(domain, &env.limits, name, hold),
        Event::Kinds { connector, kinds } => crate::holds::kinds(domain, &env.limits, connector, &kinds),
        Event::Slots { pool, slots } => crate::holds::slots(domain, env, pool, slots, out),
        Event::AllocationGone { pool, task } => crate::holds::allocation_gone(domain, env, &pool, task, out),
        Event::WakeProcedure { task } => wake_procedure(domain, env, task, out),
        Event::ProposePerson { reply_to, proposal } => {
            crate::proposals::propose_person(domain, env, reply_to, proposal, out);
        }
        Event::WithdrawPersonProposal { reply_to, proposer, proposal } => {
            crate::person_proposals::withdraw(domain, env, reply_to, proposer, proposal, out);
        }
        Event::DecidePersonProposal { reply_to, proposer, proposal, by, message, decision, revision } => {
            crate::proposals::decide_person(
                domain, env, reply_to, proposer, proposal, by, message, decision, revision, out,
            );
        }
        Event::TickRecurring { task, period } => crate::recurring::tick(domain, env, task, period, out),
        Event::RenewStanding { task, period } => crate::standing::renew(domain, env, task, period, out),
        Event::RecurringBatch { task, period, numbers } => {
            crate::recurring::make_batch(domain, env, task, period, &numbers, out);
        }
        Event::Procedure { reply_to, task, step, read, decision } => {
            crate::procedure::stepped(domain, env, reply_to, task, step, read, decision, out);
        }
        Event::TakePerson { reply_to, task, person } => {
            crate::person::take(domain, env, reply_to, task, person, out);
        }
        Event::HandBackPerson { reply_to, task, person } => {
            crate::person::hand_back(domain, env, reply_to, task, person, out);
        }
        Event::AnswerPerson { reply_to, task, person, result } => {
            crate::person::answer(domain, env, reply_to, task, person, result, out);
        }
        Event::Propose { reply_to, proposal } => {
            crate::proposals::propose(domain, env, reply_to, proposal, out);
        }
        Event::DecideProposal { reply_to, proposer, proposal, message, by, decision } => {
            crate::proposals::decide(domain, env, reply_to, proposer, proposal, message, by, decision, out);
        }
        Event::StalledProposal { proposer, proposal, from, revision, holder } => {
            crate::proposals::stalled(domain, env, proposer, proposal, from, revision, holder, out);
        }
        Event::WithdrawProposal { reply_to, proposer, proposal } => {
            crate::proposals::withdraw(domain, env, reply_to, proposer, proposal, out);
        }
        Event::Control { reply_to, by, task, control } => {
            crate::control::apply(domain, env, reply_to, by, task, control, out);
        }
        Event::Amend { reply_to, by, task, message, stop_run, amendment, funder, places } => {
            crate::control::amend(
                domain,
                env,
                reply_to,
                by,
                crate::control::Amend { number: task, message, stop_run, funder, amendment, places },
                out,
            );
        }
        Event::Move { reply_to, task, person, period, pool_budget, period_budget, reason } => {
            crate::moving::apply(domain, env, reply_to, task, person, period, pool_budget, period_budget, &reason, out);
        }
        Event::Subscribe { reply_to, task, subscription } => {
            crate::subscriptions::subscribe(domain, env, reply_to, task, subscription, false, out);
        }
        Event::SubscribeTopic { reply_to, task, subscription } => {
            crate::subscriptions::subscribe(domain, env, reply_to, task, subscription, true, out);
        }
        Event::Unsubscribe { reply_to, task, subscription } => {
            crate::subscriptions::unsubscribe(domain, env, reply_to, task, subscription, out);
        }
        Event::Notice { task, word } => crate::subscriptions::notice(domain, env, task, word, out),
        Event::Introduce { reply_to, by, left, right } => {
            crate::refs::introduce(domain, env, reply_to, by, left, right, out);
        }
        Event::RememberStub { stub } => crate::closing::remember_stub(domain, stub, out),
        Event::InspectEscalations { reply_to, project } => {
            let result = crate::escalation::project_contexts(domain, &env.limits, project);
            out.push(Request::EscalationsInspected { reply_to, result });
            return;
        }
        Event::RecheckEscalations { reply_to, project } => {
            let result = match crate::escalation::project_contexts(domain, &env.limits, project) {
                Ok(contexts) => {
                    for context in contexts {
                        out.push(Request::EscalationNeeded { context: Box::new(context) });
                    }
                    Ok(())
                }
                Err(refusal) => Err(refusal),
            };
            out.push(Request::EscalationsRechecked { reply_to, result });
            return;
        }
        Event::RoutedEscalation { task, revision, holder, entry } => {
            crate::escalation::routed(domain, env, task, revision, holder, entry, out);
        }
        Event::DecideEscalation { reply_to, task, revision, by, entry, decision } => {
            crate::escalation::decide(domain, env, reply_to, task, revision, by, entry, decision, out);
        }
        Event::PeriodNext { project, period, next } => crate::periods::observe(domain, project, period, next, out),
        Event::OpenPeriod { reply_to, project, period, budget } => {
            crate::funders::open(domain, env, reply_to, project, period, budget, out);
        }
        Event::CarvePool { reply_to, project, person, period, budget } => {
            crate::funders::carve(domain, env, reply_to, project, person, period, budget, out);
        }
        Event::ResizePool { reply_to, project, person, period, budget } => {
            crate::funders::resize_pool(domain, reply_to, project, person, period, budget, out);
        }
        Event::Make { reply_to, creator, batch } => make(domain, env, reply_to, creator, batch, out),
        Event::MakeResultFollowups { reply_to, proposer, proposal, batch } => {
            make_result_followups(domain, env, reply_to, proposer, proposal, batch, out);
        }
        Event::Message { reply_to, project, task, word, .. } => {
            crate::inbox::message(domain, env, reply_to, project, task, word, false, out);
        }
        Event::AuthorizedMessage { reply_to, project, task, word } => {
            crate::inbox::message(domain, env, reply_to, project, task, word, true, out);
        }
        Event::DelegateResult { task, word } => {
            crate::inbox::delegate_result(domain, env, task, word, out);
            crate::recurring::after_delegate(domain, env, task, out);
        }
        Event::Prepare { reply_to, task } => crate::run::prepare(domain, env, reply_to, task, out),
        Event::Claim { reply_to, task, attempt, resumed, budget, account, writes } => {
            crate::run::claim(domain, env, reply_to, task, attempt, resumed, budget, account, &writes, out);
        }
        Event::ReadAfresh { resource } => crate::writers::read_afresh(domain, env, &resource, out),
        Event::ChargeEffect { funder, maximum } => crate::funders::charge_effect(domain, env, funder, maximum, out),
        Event::EffectInFlight { reply_to, task, resource, entry } => {
            crate::writers::effect_in_flight(domain, env, reply_to, task, resource, entry, out);
        }
        Event::EffectSettled { resource, entry } => crate::writers::effect_settled(domain, env, &resource, entry, out),
        Event::Turn { reply_to, task, attempt, turn, read, refused, offered, outstanding, cumulative } => {
            crate::admission::turn(
                domain,
                env,
                reply_to,
                task,
                attempt,
                turn,
                read,
                &refused,
                offered,
                &outstanding,
                cumulative,
                out,
            );
        }
        Event::Started { task, attempt } => crate::run::started(domain, env, task, attempt, out),
        Event::Refusals { task, attempt, refused, offered } => {
            crate::admission::answer_refusals(domain, env, task, attempt, &refused, offered, out);
        }
        Event::Activation { reply_to, task, attempt, end, saved, cause } => match cause {
            crate::Cause::Priced { cumulative } => {
                crate::admission::activation(domain, env, reply_to, task, attempt, end, saved, cumulative, out);
            }
            crate::Cause::Unpriced => match end {
                crate::End::FinishedWithFollowups { .. } => {
                    refused(reply_to, Some(task), Refusal::Contract, out);
                }
                crate::End::Finished { .. }
                | crate::End::Exceeded(_)
                | crate::End::Parked
                | crate::End::Failed(_)
                | crate::End::Refused
                | crate::End::Recheck => {
                    crate::run::activation(domain, env, reply_to, task, attempt, end, saved, out);
                }
            },
        },
        Event::PreparationWaiting { task } => crate::run::preparation_waiting(domain, env, task, out),
        Event::PreparationFailed { task } => crate::run::preparation_failed(domain, env, task, out),
        Event::PreparationUnread { task } => crate::run::preparation_unread(domain, env, task, out),
        Event::Hold { task, why } => crate::run::hold(domain, env, task, why, out),
        Event::HoldFrom { task, why, source } => crate::run::hold_from(domain, env, task, why, Some(source), out),
        Event::EffectsSettled { task } => crate::closing::effects_settled(domain, env, task, out),
        Event::Retained { task, root, holding } => crate::holds::retained(domain, env, task, root, &holding, out),
        Event::Settled { task } => crate::closing::settled(domain, env, task, out),
        Event::PersonStallNotice { task, message } => crate::person::stalled(domain, env, task, message, out),
        Event::MetNotice { task, dependency, removed, by, message } => {
            crate::wake::met(domain, env, task, dependency, removed, by, message, out);
        }
        Event::Restore { record } => crate::stored::restore(domain, env, record, out),
        Event::Restored => {
            crate::stored::restored(domain, env, out);
            if domain.ready() {
                crate::proposals::rearm_all(domain, env);
                crate::proposals::wake_restored(domain, env, out);
                crate::escalation::rearm_all(domain, env);
                crate::escalation::wake_restored(domain, env, out);
            }
        }
    }
    if domain.ready() {
        crate::holds::expire(domain, env, out);
        crate::closing::progress(domain, env, out);
    }
}

/// Expire at most one due retry deadline using `env.now` after successful restoration, then advance
/// bounded closing/readiness cascades. Caller reserves `max_out` free `Request` slots and drives
/// later iterations while due; wall correction does not reproject an already armed deadline.
pub fn fire(domain: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    if !domain.ready() {
        return;
    }
    crate::grace::fire(domain, env, out);
    crate::subscriptions::timer_due(domain, env, out);
    crate::wake::fire(domain, env, out);
    crate::proposals::fire(domain, env, out);
    crate::escalation::fire(domain, env, out);
    crate::holds::expire(domain, env, out);
    if let Some(number) = domain.alarms.expire(env.now)
        && let Some(task) = task_mut(domain, number)
    {
        match task.record.phase {
            Phase::Active(Active::Due | Active::Idle) if crate::person::pending(&task.record) => {
                out.push(Request::PersonStallDue { task: number });
            }
            Phase::Active(Active::Idle)
                if match task.record.authority.budget.deadline {
                    Some(wall) => env.wall > wall,
                    None => false,
                } =>
            {
                task.record.phase = Phase::Active(Active::Due);
                publish(domain, env, number, out);
                activate(domain, number, out);
            }
            Phase::Active(Active::BackingOff { .. }) => {
                task.record.phase = crate::run::ends::for_record(&task.record)
                    .amended(task.record.executor, Phase::Active(Active::Due));
                let due = task.record.phase == Phase::Active(Active::Due);
                publish(domain, env, number, out);
                if due {
                    activate(domain, number, out);
                }
            }
            Phase::Waiting
            | Phase::Active(
                Active::Idle | Active::Due | Active::Preparing | Active::Claimed { .. } | Active::Running { .. },
            )
            | Phase::Closing(_)
            | Phase::Held { .. }
            | Phase::Ended(_) => {}
        }
    }
    crate::closing::progress(domain, env, out);
}

/// A changed writer or connector fact makes an idle procedure due.
pub(crate) fn wake_procedure(domain: &mut Domain, env: &Env<Limits>, task: u64, out: &mut Queue<Request>) {
    let wake = match record(domain, task) {
        Some(row) => match row.executor {
            crate::Executor::Procedure { .. } => row.phase == Phase::Active(Active::Idle),
            crate::Executor::Agent { .. } | crate::Executor::Person(_) => false,
        },
        None => false,
    };
    if wake {
        let row = &mut task_mut(domain, task).expect("live procedure").record;
        row.due_reason = crate::Why::Changed;
        row.phase = Phase::Active(Active::Due);
        publish(domain, env, task, out);
        activate(domain, task, out);
    }
}

pub(crate) fn record(domain: &Domain, number: u64) -> Option<&TaskRecord> {
    Some(&domain.tasks.get(*domain.names.get(&number)?).expect("name indexes live task").record)
}

pub(crate) fn task_mut(domain: &mut Domain, number: u64) -> Option<&mut Task> {
    let id = *domain.names.get(&number)?;
    Some(domain.tasks.get_mut(id).expect("name indexes live task"))
}

pub(crate) fn snapshot(domain: &Domain, capacity: u32) -> List<u64> {
    let mut numbers = List::with_capacity(capacity);
    for (number, _) in &domain.names {
        numbers.push(*number).expect("one snapshot entry per live name");
    }
    numbers
}

pub(crate) fn fact(domain: &mut Domain, observation: Fact) {
    if domain.facts.try_push(observation).is_err() {
        domain.lost = domain.lost.saturating_add(1);
    }
}

pub(crate) fn refused(to: ReplyTo, task: Option<u64>, why: Refusal, out: &mut Queue<Request>) {
    out.push(Request::Refused { reply_to: to, problem: Problem { task, why, blocked_by: None, capacity: None } });
}

pub(crate) fn entrance(domain: &Domain, to: ReplyTo, number: u64) -> Result<ReplyTo, (ReplyTo, Refusal)> {
    if !domain.ready() {
        return Err((to, Refusal::NotReady));
    }
    if !domain.names.contains_key(&number) {
        return Err((to, Refusal::Unknown));
    }
    Ok(to)
}

#[expect(clippy::manual_map, reason = "step code does not use closures")]
pub(crate) fn activate(domain: &mut Domain, number: u64, out: &mut Queue<Request>) {
    let offered = match record(domain, number).expect("activation names live task").inbox.last() {
        Some(word) => Some(word.number),
        None => None,
    };
    match record(domain, number).expect("activation names live task").executor {
        crate::Executor::Procedure { .. } => {
            task_mut(domain, number).expect("live procedure").procedure_offered = offered;
        }
        crate::Executor::Agent { .. } | crate::Executor::Person(_) => {}
    }
    if let Some(context) = domain.run_context(number) {
        out.push(Request::Activate { context });
    }
}

impl Domain {
    /// The newest word a run of this task reads: its own inbox's and the proposals and
    /// escalations waiting on it, which never enter that inbox. A preparation's context
    /// carries it as `last_message`, and stays current while it is unchanged.
    #[must_use]
    pub fn run_message(&self, number: u64) -> Option<u64> {
        let task = record(self, number)?;
        let mut last = task.last_message;
        for word in &task.inbox {
            last = last.max(word.number);
        }
        for word in crate::proposals::waiting_for(self, number) {
            last = last.max(word.number);
        }
        for word in crate::escalation::waiting_for(self, number) {
            last = last.max(word.number);
        }
        Some(last)
    }

    /// Fresh read-only preparation facts; admission must not trust an older async snapshot.
    #[must_use]
    pub fn run_context(&self, number: u64) -> Option<Box<crate::RunContext>> {
        let domain = self;
        record(domain, number)?;
        let task = record(domain, number).expect("activation names live task");
        match task.executor {
            crate::Executor::Person(_) => return None,
            crate::Executor::Agent { .. } | crate::Executor::Procedure { .. } => {}
        }
        let waiting = crate::proposals::waiting_for(domain, number);
        let escalations = crate::escalation::waiting_for(domain, number);
        let capacity = task.inbox.len().checked_add(waiting.len()).expect("bounded inbox and proposals");
        let capacity = capacity.checked_add(escalations.len()).expect("bounded inbox and decisions");
        let mut unordered = List::with_capacity(u32::try_from(capacity).expect("bounded inbox and proposals"));
        for word in &task.inbox {
            unordered.push(word.clone()).expect("inbox counted");
        }
        for word in waiting {
            unordered.push(word).expect("virtual proposal counted");
        }
        for word in escalations {
            unordered.push(word).expect("virtual escalation counted");
        }
        let mut inbox = List::with_capacity(unordered.len());
        let mut previous = 0_u64;
        for _ in 0..unordered.len() {
            let mut next: Option<&crate::Word> = None;
            for candidate in &unordered {
                if candidate.number > previous {
                    next = match next {
                        Some(current) if current.number < candidate.number => Some(current),
                        Some(_) | None => Some(candidate),
                    };
                }
            }
            let next = next.expect("distinct globally numbered inbox entries");
            previous = next.number;
            inbox.push(next.clone()).expect("ordered inbox room");
        }
        let mut delegates = List::with_capacity(u32::try_from(task.delegates.len()).expect("bounded delegates"));
        for child in &task.delegates {
            let child_record = record(domain, *child).expect("live delegate named by requester");
            delegates
                .push(crate::DelegateState {
                    task: *child,
                    phase: child_record.phase.clone(),
                    revisions: child_record.revisions,
                })
                .expect("bounded delegate snapshot");
        }
        Some(Box::new(crate::RunContext {
            ended_by: task.flags.ended_by,
            llm: task.llm.clone(),
            payer: domain.payer(number).expect("admitted financial chain has a payer"),
            task: number,
            last_message: domain.run_message(number).expect("activation names live task"),
            inbox: inbox.into_boxed(),
            delegates: delegates.into_boxed(),
            dependencies: task.dependencies.clone(),
            added: task.added.clone(),
            pending: task.waiting_on.clone(),
            saved: task.saved.clone(),
            previous_attempt: task.attempt,
            ever_turned: task.ever_turned,
            tries: task.tries,
            invalid_result: task.invalid_result,
            project: task.project,
            executor: task.executor,
            spec: task.spec.clone(),
            contract: task.contract.clone(),
            requester: task.requester,
            authority: task.authority.clone(),
            numbers: task.numbers,
        }))
    }
}

fn milestone(domain: &mut Domain, env: &Env<Limits>, number: u64, out: &mut Queue<Request>) {
    let task = task_mut(domain, number).expect("milestone task is live");
    if task.observed_phase.as_ref() == Some(&task.record.phase) {
        return;
    }
    let initial = task.observed_phase.is_none();
    task.record.phase_since = env.wall;
    let previous_held = match &task.observed_phase {
        Some(Phase::Held { .. }) => true,
        Some(Phase::Waiting | Phase::Active(_) | Phase::Closing(_) | Phase::Ended(_)) | None => false,
    };
    if !initial && task.record.phase == Phase::Waiting && !previous_held {
        task.record.due_reason = crate::Why::Ready;
    }
    if task.record.phase == Phase::Active(Active::Due) {
        task.record.due_reason = match task.observed_phase.as_ref() {
            Some(Phase::Waiting)
                if task.record.due_reason == crate::Why::Made && task.record.dependencies.is_empty() =>
            {
                crate::Why::Made
            }
            Some(Phase::Waiting) if task.record.due_reason != crate::Why::Released => crate::Why::Ready,
            Some(Phase::Held { .. }) => crate::Why::Released,
            Some(Phase::Active(Active::BackingOff { .. })) => crate::Why::Retry { tries: task.record.tries },
            Some(
                Phase::Waiting
                | Phase::Closing(_)
                | Phase::Ended(_)
                | Phase::Active(
                    Active::Idle | Active::Due | Active::Preparing | Active::Claimed { .. } | Active::Running { .. },
                ),
            )
            | None => task.record.due_reason,
        };
    }
    task.record.milestone = task.record.milestone.checked_add(1).expect("task history position fits");
    task.observed_phase = Some(task.record.phase.clone());
    let position = task.record.milestone;
    let held = match task.record.phase {
        Phase::Held { .. } => true,
        Phase::Waiting | Phase::Active(_) | Phase::Closing(_) | Phase::Ended(_) => false,
    };
    if held {
        task.record.hold = position;
    }
    if initial || held {
        crate::activity::append(&mut task.record, env.wall, crate::Line::Milestone { position }, out);
    }
    out.push(Request::Save {
        record: Stored::Milestone(crate::Milestone {
            task: number,
            position: task.record.milestone,
            phase: task.record.phase.clone(),
            at: env.wall,
        }),
    });
}

pub(crate) fn publish(domain: &mut Domain, env: &Env<Limits>, number: u64, out: &mut Queue<Request>) {
    crate::settings::publish(domain, number);
    crate::waits::refresh(domain, number);
    crate::proposals::withdraw_on_close(domain, env, number, out);
    crate::inbox::keep_refusals(domain, number, &[]);
    let task = task_mut(domain, number).expect("published task is live");
    let source_changed = match &task.record.phase {
        Phase::Held { source, .. } => match &task.observed_phase {
            Some(Phase::Held { source: previous, .. }) => source != previous,
            Some(Phase::Waiting | Phase::Active(_) | Phase::Closing(_) | Phase::Ended(_)) | None => false,
        },
        Phase::Waiting | Phase::Active(_) | Phase::Closing(_) | Phase::Ended(_) => false,
    };
    milestone(domain, env, number, out);
    let task = task_mut(domain, number).expect("published task is live");
    crate::person::waiting(&mut task.record, env);
    let held = match task.record.phase {
        Phase::Held { why, .. } => Some(why),
        Phase::Waiting | Phase::Active(_) | Phase::Closing(_) | Phase::Ended(_) => None,
    };
    let became_held = held.is_some() && (task.observed_hold != held || source_changed);
    let unavailable = crate::run::ends::for_record(&task.record).blocks_claims()
        || match task.record.phase {
            Phase::Held { .. } | Phase::Closing(_) | Phase::Ended(_) => true,
            Phase::Waiting | Phase::Active(_) => false,
        };
    if became_held {
        task.record.revisions.hold = task.record.revisions.hold.checked_add(1).expect("hold revision room");
        task.record.escalation = crate::Escalation::Unheld { revision: task.record.revisions.hold };
    }
    task.observed_hold = held;
    let escalation = crate::escalation::begin(&mut task.record);
    // Admission permits equality with the wall. Wake just after it, without
    // another alarm or a durable deadline carrier (domain/authority.md, 7).
    #[expect(clippy::manual_map, reason = "step code has no closures (programming-model.md, section 10)")]
    let wall = match task.record.authority.budget.deadline {
        Some(wall) => match wall.as_nanos().checked_add(1) {
            Some(nanos) => Some(Wall::from_nanos(nanos)),
            None => None,
        },
        None => None,
    };
    let until = match task.record.phase {
        Phase::Active(Active::Due | Active::Idle) if crate::person::pending(&task.record) => {
            Some(task.record.person_wait.expect("active person wait").deadline)
        }
        Phase::Active(Active::Idle) => wall,
        Phase::Active(Active::BackingOff { until }) => Some(match wall {
            Some(wall) => until.min(wall),
            None => until,
        }),
        Phase::Waiting
        | Phase::Active(Active::Due | Active::Preparing | Active::Claimed { .. } | Active::Running { .. })
        | Phase::Closing(_)
        | Phase::Held { .. }
        | Phase::Ended(_) => None,
    };
    let alarm = match until {
        Some(until) => match task.alarm {
            Some(old) if old.until == until => Some(old),
            Some(_) | None => Some(Alarm {
                until,
                due: env.now.saturating_add(skein_lib::Duration::from_nanos(
                    until.as_nanos().saturating_sub(env.wall.as_nanos()),
                )),
            }),
        },
        None => None,
    };
    task.alarm = alarm;
    crate::changed::observe(task, env.wall);
    out.push(Request::Save { record: Stored::Live(Box::new(task.record.clone())) });
    crate::grace::schedule(domain, env, number);
    crate::escalation::schedule(domain, env, number);
    if escalation {
        out.push(Request::EscalationNeeded {
            context: crate::escalation::context(domain, number).expect("new held person context"),
        });
    }
    match alarm {
        Some(alarm) => {
            let armed = domain.alarms.arm(number, alarm.due);
            assert!(armed.is_ok(), "one alarm per live task");
        }
        None => {
            domain.alarms.cancel(number);
        }
    }
    if became_held {
        let revision = record(domain, number).expect("held task").revisions.hold;
        crate::subscriptions::notify_state(domain, number, crate::NoticeState::Held { revision }, &[], out);
    }
    if unavailable {
        crate::proposals::holder_unavailable(domain, number, out);
        crate::escalation::holder_unavailable(domain, number, out);
    }
}

pub(crate) fn make(
    domain: &mut Domain,
    env: &Env<Limits>,
    to: ReplyTo,
    creator: Party,
    batch: Box<[New]>,
    out: &mut Queue<Request>,
) {
    if !domain.ready() {
        return refused(to, None, Refusal::NotReady, out);
    }
    if let Err(problem) = crate::batch::check(domain, &env.limits, creator, &batch) {
        return crate::capacity::refused(domain, env, to, problem, out);
    }
    let numbers = make_admitted(domain, env, creator, batch, out);
    out.push(Request::Made { reply_to: to, tasks: numbers });
}

fn make_result_followups(
    domain: &mut Domain,
    env: &Env<Limits>,
    to: ReplyTo,
    proposer: u64,
    proposal: u64,
    batch: Box<[New]>,
    out: &mut Queue<Request>,
) {
    if !domain.ready() {
        return refused(to, Some(proposer), Refusal::NotReady, out);
    }
    if let Err(problem) =
        crate::batch::check_result_followups(domain, &env.limits, Party::Task(proposer), proposal, &batch)
    {
        return crate::capacity::refused(domain, env, to, problem, out);
    }
    remember_result_followups(domain, &env.limits, proposer, &batch);
    let numbers = make_admitted(domain, env, Party::Task(proposer), batch, out);
    out.push(Request::Made { reply_to: to, tasks: numbers });
}

/// Retain the exact preflighted result batch before construction publishes its creator.
pub(crate) fn remember_result_followups(domain: &mut Domain, limits: &Limits, number: u64, batch: &[New]) {
    let mut identities = List::with_capacity(limits.batch);
    for member in batch {
        identities.push(member.number).expect("admitted result batch");
    }
    let row = &mut task_mut(domain, number).expect("admitted result creator").record;
    assert!(row.result_followups.is_empty(), "one admitted result batch");
    row.result_followups = identities.into_boxed();
}

/// Construct a whole batch after its entrance and funding were preflighted in this decision.
#[expect(clippy::manual_map, reason = "the subset uses a closed match instead of a closure")]
#[expect(clippy::too_many_lines, reason = "one whole admitted batch constructs every durable member")]
pub(crate) fn make_admitted(
    domain: &mut Domain,
    env: &Env<Limits>,
    creator: Party,
    batch: Box<[New]>,
    out: &mut Queue<Request>,
) -> Box<[u64]> {
    crate::funders::reserve(domain, env, &batch, out);
    let parent = match creator {
        Party::Task(number) => Some(number),
        Party::Person(_) | Party::Deployment { .. } => None,
    };
    let (root, depth) = match parent {
        Some(number) => {
            let parent = record(domain, number).expect("creator admitted");
            (Some(parent.root), parent.depth.checked_add(1).expect("depth admitted"))
        }
        None => (None, 0),
    };
    let mut numbers = List::with_capacity(env.limits.batch);
    for new in &batch {
        numbers.push(new.number).expect("batch admitted");
    }
    let count = numbers.len();
    let mut handed = List::with_capacity(env.limits.batch);
    if let Some(number) = parent {
        let creator = record(domain, number).expect("creator admitted");
        let creator_holds = creator.holds_taken;
        let held = creator.holdings.clone();
        let mut remaining =
            List::with_capacity(env.limits.tasks.checked_mul(env.limits.holdings).expect("bounded live holds"));
        for holding in &held {
            let mut to = None;
            for new in &batch {
                if !creator_holds {
                    break;
                }
                for wanted in &new.holdings {
                    if crate::holds::same(wanted, holding) {
                        to = Some(new.number);
                        break;
                    }
                }
                if to.is_some() {
                    break;
                }
            }
            match to {
                Some(child) => {
                    let mut known = false;
                    for existing in &handed {
                        if *existing == child {
                            known = true;
                            break;
                        }
                    }
                    if !known {
                        handed.push(child).expect("one handed recipient per batch member");
                    }
                }
                None => remaining.push(holding.clone()).expect("retained held set fits"),
            }
        }
        task_mut(domain, number).expect("creator admitted").record.holdings = remaining.into_boxed();
        let old = task_mut(domain, number).expect("creator admitted");
        old.record.delegates = append(env.limits.delegates, &old.record.delegates, &batch);
        let root_number = root.expect("task creator has a tree root");
        let source_period = crate::funders::original_period(domain, number).expect("admitted creator has a period");
        let current_period = crate::funders::original_period(domain, root_number).expect("live root has a period");
        let archived = if source_period == current_period {
            None
        } else {
            match source_period {
                crate::Funder::Period { project, period } => {
                    Some(crate::Funder::Recurring { project, task: root_number, period })
                }
                crate::Funder::Task(_) | crate::Funder::Pool { .. } | crate::Funder::Recurring { .. } => {
                    unreachable!("resolved original period")
                }
            }
        };
        // Count each made task in its own period, so a standing root's old
        // delegates retain their old allotment after that root renews.
        let mut ancestor = Some(number);
        for _ in 0..env.limits.tasks {
            let Some(number) = ancestor else {
                break;
            };
            let task = task_mut(domain, number).expect("all ancestors of live task are live");
            if number != root_number || archived.is_none() {
                task.record.made = task.record.made.checked_add(count).expect("tree count admitted");
            }
            ancestor = match task.record.requester {
                Party::Task(parent) => Some(parent),
                Party::Person(_) | Party::Deployment { .. } => None,
            };
            publish(domain, env, number, out);
        }
        let ledger = match archived {
            Some(funder) => Some(funder),
            None => {
                if record(domain, root_number).expect("live root").recurring.is_some() {
                    match current_period {
                        crate::Funder::Period { project, period } => {
                            Some(crate::Funder::Recurring { project, task: root_number, period })
                        }
                        crate::Funder::Task(_) | crate::Funder::Pool { .. } | crate::Funder::Recurring { .. } => {
                            unreachable!("resolved original period")
                        }
                    }
                } else {
                    None
                }
            }
        };
        if let Some(funder) = ledger {
            let ledger = domain.funding.get_mut(&funder).expect("period tree allotment admitted");
            ledger.made = ledger.made.checked_add(count).expect("period tree count admitted");
            crate::funders::save_funding(domain, funder, out);
        }
    }
    for new in batch {
        let number = new.number;
        let task = Task {
            published: None,
            procedure_offered: None,
            record: TaskRecord {
                settings: crate::SettingsRevision { task: domain.newest, run: None },
                label: new.label,
                flags: new.flags,
                last_activity: env.wall,
                ended_below: crate::Endings::ZERO,
                question_lines: Box::new([]),
                hold: 0,
                proposal_rows: Box::new([]),
                instructions: None,
                llm: new.llm,
                kept_account: None,
                refused: Box::new([]),
                grace: None,
                created_at: env.wall,
                changed: env.wall,
                phase_since: env.wall,
                run_since: None,
                retry_notice: None,
                ended_at: None,
                revision: 0,
                revisions: crate::Revisions { hold: 0, spec: 0, person: 0 },
                milestone: 0,
                activity: 0,
                due_reason: crate::Why::Made,
                narrowing: false,
                result_position: 0,
                last_reply: 0,
                replies: Box::new([]),
                escalation: crate::Escalation::Unheld { revision: 0 },
                proposal: None,
                result_proposal: false,
                number,
                project: new.project,
                requester: creator,
                run_spent: 0,
                run_reserved: 0,
                root: root.unwrap_or(number),
                depth,
                executor: new.executor,
                taken_by: None,
                person_answer: None,
                person_wait: None,
                result_followups: Box::new([]),
                tracked: new.tracked,
                recurring: match new.recurring {
                    Some(template) => Some(Box::new(crate::RecurringState {
                        template: *template,
                        last_batch: Box::new([]),
                        last_period: 0,
                        pending_period: None,
                    })),
                    None => None,
                },
                spec: new.spec,
                contract: new.contract,
                authority: new.authority,
                numbers: new.numbers,
                funder: new.funder,
                waiting_on: new.dependencies.clone(),
                holdings: new.holdings,
                holds_taken: false,
                hold_wait_since: None,
                hold_bound_since: None,
                dependencies: new.dependencies,
                added: Box::new([]),
                delegates: Box::new([]),
                references: Box::new([]),
                questions: Box::new([]),
                subscriptions: Box::new([]),
                wake: new.wake,
                turn: 0,
                last_message: 0,
                inbox: Box::new([]),
                saved: Box::new([]),
                ever_turned: false,
                made: 1,
                attempt: 0,
                last_answer: None,
                tries: Tries::NONE,
                invalid_result: None,
                refusals: 0,
                phase: Phase::Waiting,
            },
            alarm: None,
            grace_alarm: None,
            observed_hold: None,
            observed_phase: None,
        };
        match task.record.executor {
            crate::Executor::Agent { .. } => {
                domain.live_agents = domain.live_agents.checked_add(1).expect("bounded live agents");
            }
            crate::Executor::Person(_) | crate::Executor::Procedure { .. } => {}
        }
        let id = domain.tasks.insert(task).expect("batch slab room admitted");
        let indexed = domain.names.insert(number, id);
        assert!(indexed == Ok(None), "batch names admitted");
        let mut inherited = false;
        for child in &handed {
            if *child == number {
                inherited = true;
                break;
            }
        }
        if inherited {
            crate::holds::take(domain, env, number, out);
            task_mut(domain, number).expect("handed child live").record.phase = Phase::Active(Active::Due);
        }
        publish(domain, env, number, out);
        if let Some(requester) = parent {
            out.push(Request::Save { record: Stored::Delegate(crate::Edge { requester, task: number }) });
        }
        fact(domain, Fact::Made { task: number, requester: creator });
        if inherited {
            activate(domain, number, out);
        }
    }
    numbers.into_boxed()
}

fn append(capacity: u32, old: &[u64], batch: &[New]) -> Box<[u64]> {
    let mut numbers = List::with_capacity(capacity);
    for number in old {
        numbers.push(*number).expect("existing numbers admitted");
    }
    for new in batch {
        numbers.push(new.number).expect("new numbers admitted");
    }
    numbers.into_boxed()
}

fn validate_charters(limits: &Limits, charters: &[crate::CharterModels], efforts: &[u8]) {
    assert!(
        charters.len()
            <= usize::try_from(limits.charters.checked_add(limits.versions).expect("version bound"))
                .expect("u32 fits usize"),
        "charter versions fit"
    );
    let models = usize::try_from(crate::MODELS).expect("u32 fits usize");
    assert!(efforts.len() <= models, "configured models fit");
    for (at, charter) in charters.iter().enumerate() {
        for earlier in charters.iter().take(at) {
            assert!(
                earlier.number != charter.number || earlier.revision != charter.revision,
                "charter versions are unique"
            );
        }
        let Some(configured) = &charter.models else { continue };
        assert!(configured.len() <= models, "a charter's models fit");
        for (index, model) in configured.iter().enumerate() {
            assert!(usize::from(*model) < efforts.len(), "a charter's models are configured");
            for earlier in configured.iter().take(index) {
                assert!(earlier != model, "a charter lists each model once");
            }
        }
    }
}
