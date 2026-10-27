//! Ordered admission and whole application of a party's task operations (people.md, 5.1).
mod staged;
use crate::{Amendment, Domain, Funder, Limits, Numbers, Party, Problem, Refusal, Request, Word};
use alloc::boxed::Box;
use skein_lib::{Duration, Env, List, Queue, ReplyTo};
pub use staged::CompoundTask;

/// One authenticated party operation in a single project's whole request.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Operation {
    Cancel { task: u64, reason: Box<[u8]>, grace: Option<Duration>, message: Option<u64> },
    Move { task: u64, reason: Box<[u8]> },
    Amend { task: u64, message: u64, stop_run: bool, amendment: Box<Amendment> },
    Release { task: u64, again: Option<bool>, message: Option<u64> },
    Message { task: u64, word: Word },
    Stop { task: u64, hold: u64 },
}
impl Operation {
    fn task(&self) -> u64 {
        match self {
            Self::Cancel { task, .. }
            | Self::Move { task, .. }
            | Self::Amend { task, .. }
            | Self::Release { task, .. }
            | Self::Message { task, .. }
            | Self::Stop { task, .. } => *task,
        }
    }
}

pub(crate) enum Plan {
    Cancel(Box<crate::control::CancelPlan>),
    Move { plan: Box<crate::moving::Plan>, reason: Box<[u8]> },
    Amend(Box<crate::control::AmendPlan>),
    Release(Box<crate::control::ReleasePlan>),
    Message(Box<crate::inbox::MessagePlan>),
    Stop(Box<crate::placed::Placement>),
}

impl Plan {
    pub(crate) fn placement(&self) -> Option<&crate::placed::Placement> {
        match self {
            Self::Stop(plan) => Some(plan),
            Self::Cancel(_) | Self::Move { .. } | Self::Amend(_) | Self::Release(_) | Self::Message(_) => None,
        }
    }
    pub(crate) fn placed_change(&self) -> Option<&crate::placed::ReleaseChanges> {
        match self {
            Self::Release(plan) => plan.placed.as_deref(),
            Self::Amend(plan) => plan.release.as_ref()?.placed.as_deref(),
            Self::Cancel(_) | Self::Move { .. } | Self::Message(_) | Self::Stop(_) => None,
        }
    }
}

pub(crate) struct Staged {
    pub(crate) closing: List<u64>,
    pub(crate) moved: List<u64>,
    pub(crate) reserved: u64,
    pub(crate) plans: List<Plan>,
    pub(crate) circles: List<Option<(u64, crate::Circle)>>,
    pub(crate) funding: List<(Funder, Numbers)>,
}
impl Staged {
    pub(crate) fn new(bound: u32) -> Self {
        Self {
            closing: List::with_capacity(bound),
            moved: List::with_capacity(bound),
            reserved: 0,
            plans: List::with_capacity(bound),
            circles: List::with_capacity(bound),
            funding: List::with_capacity(bound.saturating_mul(4)),
        }
    }
    pub(crate) fn after(&mut self, domain: &Domain, limits: &Limits, plan: &Plan) {
        let circle = self.returned_circle(domain, limits, plan);
        self.circles.push(circle).expect("one return judgement per operation");
        match plan {
            Plan::Cancel(plan) => {
                let closing = plan.closing();
                if let Some(closing) = closing {
                    for task in closing {
                        if !has(&self.closing, *task) {
                            self.closing.push(*task).expect("live tasks bound");
                        }
                    }
                }
            }
            Plan::Move { plan, .. } => {
                if !has(&self.moved, plan.number) {
                    self.moved.push(plan.number).expect("operation bound");
                }
                if self.numbers(domain, plan.destination).is_none() {
                    let mut period = match self.numbers(domain, plan.period_source) {
                        Some(numbers) => numbers,
                        None => Numbers { budget: plan.period_budget, spent: 0, spent_below: 0, reserved: 0 },
                    };
                    period.reserved = period.reserved.checked_add(plan.pool_budget).expect("admission checked");
                    self.funding.push((plan.period_source, period)).expect("period creation per move");
                }
                if plan.source != plan.destination {
                    let mut source = self.numbers(domain, plan.source).expect("checked source");
                    source.reserved = source.reserved.checked_sub(plan.budget).expect("source checked");
                    source.spent_below = source.spent_below.checked_add(plan.spent).expect("admission checked");
                    self.funding.push((plan.source, source)).expect("three ledgers per move");
                    let mut destination = match self.numbers(domain, plan.destination) {
                        Some(numbers) => numbers,
                        None => Numbers { budget: plan.pool_budget, spent: 0, spent_below: 0, reserved: 0 },
                    };
                    destination.reserved = destination.reserved.checked_add(plan.left).expect("admission checked");
                    self.funding.push((plan.destination, destination)).expect("three ledgers per move");
                    self.reserved = self.reserved.checked_add(plan.left).expect("admission checked");
                    let mut numbers = self.record(domain, plan.number).expect("moved task").numbers;
                    numbers.budget = plan.left;
                    numbers.spent = 0;
                    numbers.spent_below = 0;
                    self.funding.push((Funder::Task(plan.number), numbers)).expect("task normalization per move");
                }
            }
            Plan::Amend(plan) => {
                if let Some(authority) = &plan.amendment.authority {
                    let row = self.record(domain, plan.number).expect("checked task");
                    let funder = row.funder;
                    let budget = row.numbers.budget;
                    let row_numbers = row.numbers;
                    let mut numbers = self.numbers(domain, funder).expect("checked funder");
                    numbers.reserved = numbers
                        .reserved
                        .checked_sub(budget)
                        .expect("reservation checked")
                        .checked_add(authority.budget.spend)
                        .expect("resize checked");
                    self.funding.push((funder, numbers)).expect("one ledger per amendment");
                    let mut numbers = row_numbers;
                    numbers.budget = authority.budget.spend;
                    self.funding.push((Funder::Task(plan.number), numbers)).expect("task budget per amendment");
                }
            }
            Plan::Release(_) | Plan::Message(_) | Plan::Stop(_) => {}
        }
    }
}

pub(crate) struct Compound {
    pub reply_to: ReplyTo,
    pub by: Party,
    pub project: u32,
    pub operations: List<Operation>,
    pub period: u64,
    pub pool_budget: u64,
    pub period_budget: u64,
}

pub(crate) fn compound(domain: &mut Domain, env: &Env<Limits>, request: Compound, out: &mut Queue<Request>) {
    if request.operations.len() > env.limits.tasks {
        return out.push(Request::CompoundRefused {
            reply_to: request.reply_to,
            at: 0,
            problem: Problem::new(None, Refusal::Batch),
        });
    }
    let context = CompoundContext {
        by: request.by,
        project: request.project,
        period: request.period,
        pool_budget: request.pool_budget,
        period_budget: request.period_budget,
    };
    let mut admission = CompoundAdmission::new(domain, env.limits.tasks);
    for (at, operation) in request.operations.into_boxed().into_iter().enumerate() {
        if let Err(problem) = admission.check(env, context, operation) {
            return out.push(Request::CompoundRefused {
                reply_to: request.reply_to,
                at: u32::try_from(at).expect("bounded operations"),
                problem: *problem,
            });
        }
    }
    let staged = admission.staged;
    let reroute = !staged.moved.is_empty();
    for plan in staged.plans.into_boxed() {
        apply(domain, env, request.by, plan, out);
    }
    if reroute {
        crate::moving::reroute(domain, env, out);
    }
    out.push(Request::Done { reply_to: request.reply_to });
}

/// Authenticated project and funding context shared by all compound operations.
#[derive(Clone, Copy, Debug)]
pub struct CompoundContext {
    pub by: Party,
    pub project: u32,
    pub period: u64,
    pub pool_budget: u64,
    pub period_budget: u64,
}

/// A borrowed task-domain projection, changed only by admitted operation plans.
pub struct CompoundAdmission<'a> {
    domain: &'a Domain,
    staged: Staged,
    bound: u32,
}
impl core::fmt::Debug for CompoundAdmission<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("CompoundAdmission")
            .field("operations", &self.staged.plans.len())
            .finish_non_exhaustive()
    }
}
impl<'a> CompoundAdmission<'a> {
    /// Reserve bounded scratch, without changing or copying the domain.
    #[must_use]
    pub fn new(domain: &'a Domain, bound: u32) -> Self {
        Self { domain, staged: Staged::new(bound), bound }
    }
    /// Read a task as the preceding admitted operations leave it.
    #[must_use]
    pub fn task(&self, number: u64) -> Option<CompoundTask<'_>> {
        if has(&self.staged.closing, number) {
            return None;
        }
        self.staged.record(self.domain, number)
    }
    /// Financial payer after the preceding compound operations, including moves and refunds.
    #[must_use]
    pub fn payer(&self, number: u64) -> Option<crate::Payer> {
        let mut funder = Funder::Task(number);
        for _ in 0..=self.domain.names.capacity() {
            funder = match funder {
                Funder::Pool { person, .. } => return Some(crate::Payer::Party(person)),
                Funder::Period { project, .. } | Funder::Recurring { project, .. } => {
                    return Some(crate::Payer::Project(project));
                }
                Funder::Task(task) => self.task(task)?.funder,
            };
        }
        None
    }
    /// Read a funder's cumulative balances.
    #[must_use]
    pub fn funding(&self, funder: Funder) -> Option<Numbers> {
        self.staged.numbers(self.domain, funder)
    }
    /// Check a person-address amendment against the projected executor and revision.
    pub fn address_admit(&self, number: u64, change: &Option<crate::AddressChange>) -> Result<(), Refusal> {
        let row = self.task(number).ok_or(Refusal::Unknown)?;
        Staged::address(&row, change.as_ref())
    }
    /// Whether an earlier move has detached this task from its old tree.
    #[must_use]
    pub fn moved(&self, number: u64) -> bool {
        self.staged.detached(number)
    }
    /// Check and retain one plan against the preceding plans; never apply it.
    pub fn check(
        &mut self,
        env: &Env<Limits>,
        context: CompoundContext,
        operation: Operation,
    ) -> Result<(), Box<Problem>> {
        let number = operation.task();
        if self.staged.plans.len() >= self.bound {
            return Err(Box::new(Problem::new(None, Refusal::Batch)));
        }
        let problem = if !maker_allowed(context.by, context.project, &operation, self.domain) {
            Some(Refusal::Reference)
        } else if !self.domain.ready() {
            Some(Refusal::NotReady)
        } else if has(&self.staged.closing, number) {
            Some(Refusal::State)
        } else {
            match self.staged.record(self.domain, number) {
                None => Some(Refusal::Unknown),
                Some(row) if row.project != context.project => Some(Refusal::Project),
                Some(_) => None,
            }
        };
        if let Some(why) = problem {
            return Err(Box::new(Problem::new(Some(number), why)));
        }
        let plan = check(self.domain, &self.staged, env, context, operation)?;
        self.staged.after(self.domain, &env.limits, &plan);
        assert!(self.staged.plans.push(plan).is_ok(), "operation bound");
        Ok(())
    }
}

fn maker_allowed(by: Party, project: u32, operation: &Operation, domain: &Domain) -> bool {
    match by {
        Party::Person(person) => person != 0,
        Party::Task(_) => false,
        Party::Deployment { project: held } => {
            if held != project {
                return false;
            }
            match operation {
                Operation::Cancel { task, .. } => match domain.task(*task) {
                    Some(row) => row.project == project,
                    None => false,
                },
                Operation::Move { .. }
                | Operation::Amend { .. }
                | Operation::Release { .. }
                | Operation::Message { .. }
                | Operation::Stop { .. } => false,
            }
        }
    }
}
fn person(by: Party) -> Result<u64, Box<Problem>> {
    match by {
        Party::Person(person) => Ok(person),
        Party::Task(_) | Party::Deployment { .. } => Err(Box::new(Problem::new(None, Refusal::Reference))),
    }
}

fn check(
    domain: &Domain,
    staged: &Staged,
    env: &Env<Limits>,
    request: CompoundContext,
    operation: Operation,
) -> Result<Plan, Box<Problem>> {
    let by = request.by;
    match operation {
        Operation::Cancel { task, reason, grace, message } => Ok(Plan::Cancel(Box::new(crate::control::check_cancel(
            domain,
            staged,
            env,
            by,
            crate::control::Cancel { number: task, reason, grace, message },
        )?))),
        Operation::Amend { task, message, stop_run, amendment } => match crate::control::check_amend(
            domain,
            staged,
            env,
            by,
            crate::control::Amend {
                number: task,
                message,
                stop_run,
                funder: None,
                amendment: *amendment,
                places: Box::new([]),
            },
        ) {
            Ok(plan) => Ok(Plan::Amend(Box::new(plan))),
            Err(problem) => Err(problem),
        },
        Operation::Release { task, again, message } => {
            match crate::control::check_release(domain, staged, env, task, by, again, message) {
                Ok(plan) => Ok(Plan::Release(Box::new(plan))),
                Err(why) => Err(Box::new(crate::control::release_problem(domain, staged, &env.limits, task, by, why))),
            }
        }
        Operation::Message { task, word } => {
            if word.from != by {
                return Err(Box::new(Problem::new(Some(task), Refusal::Reference)));
            }
            Ok(Plan::Message(Box::new(crate::inbox::check_message(
                domain,
                staged,
                env,
                crate::inbox::Message { project: request.project, number: task, word, authorized: true },
            )?)))
        }
        Operation::Stop { task, hold } => {
            let placed = crate::PlacedHold {
                number: hold,
                project: request.project,
                owner: crate::Owner::Party { person: person(request.by)? },
                mode: crate::Mode::Stop,
                scope: crate::Scope::Subtree { task },
                reason: Box::new([]),
                placed_at: env.wall.as_nanos().checked_div(1_000_000).expect("milliseconds divisor"),
                revision: 1,
                exempt: Box::new([]),
                lifted: false,
            };
            Ok(Plan::Stop(Box::new(crate::placed::check_place(domain, staged, &env.limits, placed)?)))
        }
        Operation::Move { task, reason } => match crate::moving::check(
            domain,
            staged,
            env,
            crate::moving::Move {
                number: task,
                person: person(request.by)?,
                period: request.period,
                pool_budget: request.pool_budget,
                period_budget: request.period_budget,
                reason: &reason,
            },
        ) {
            Ok(plan) => Ok(Plan::Move { plan: Box::new(plan), reason }),
            Err(problem) => Err(Box::new(problem)),
        },
    }
}

fn apply(domain: &mut Domain, env: &Env<Limits>, by: Party, plan: Plan, out: &mut Queue<Request>) {
    match plan {
        Plan::Cancel(plan) => crate::control::apply_cancel(domain, env, by, *plan, out),
        Plan::Move { plan, reason } => crate::moving::make(domain, env, *plan, &reason, out),
        Plan::Amend(plan) => crate::control::apply_amend(domain, env, by, *plan, out),
        Plan::Release(plan) => crate::control::apply_release(domain, env, by, *plan, out),
        Plan::Message(plan) => {
            crate::inbox::apply_message(domain, env, *plan, out);
        }
        Plan::Stop(plan) => crate::placed::apply_place(domain, env, *plan, out),
    }
}

pub(crate) fn has(list: &List<u64>, number: u64) -> bool {
    for entry in list {
        if *entry == number {
            return true;
        }
    }
    false
}

/// Input operations, owned plans and their bounded field projections.
pub(crate) fn worst_case(limits: &Limits, payload: u64) -> Option<u64> {
    let tasks = limits.tasks;
    List::<Plan>::worst_case(tasks)?
        .checked_add(List::<Option<(u64, crate::Circle)>>::worst_case(tasks)?)?
        .checked_add(List::<Operation>::worst_case(tasks)?)?
        .checked_add(List::<u64>::worst_case(tasks)?.checked_mul(2)?)?
        .checked_add(List::<(Funder, Numbers)>::worst_case(tasks.checked_mul(4)?)?)?
        .checked_add(
            u64::from(tasks).checked_mul(
                u64::try_from(size_of::<crate::control::AmendPlan>())
                    .ok()?
                    .checked_add(payload.checked_mul(4)?)?
                    .checked_add(List::<u64>::worst_case(tasks)?.checked_mul(2)?)?,
            )?,
        )?
        // Each projected return can carry a fresh bounded circle, beside the input hold.
        .checked_add(
            u64::from(tasks).checked_mul(
                u64::try_from(size_of::<crate::HoldSource>())
                    .ok()?
                    .checked_add(u64::from(limits.circle).checked_mul(8)?)?,
            )?,
        )?
        .checked_add(crate::placed::compound_bytes(limits)?)?
        .checked_add(payload)
}
