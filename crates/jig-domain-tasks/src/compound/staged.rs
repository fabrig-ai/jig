//! Borrow unchanged fields and project only what admitted operations change.
use super::{Plan, Staged};
use crate::{Active, Executor, Funder, Grace, Numbers, Party, Phase, Revisions, Settings, TaskRecord, Tracking};
use alloc::boxed::Box;
use core::ops::Deref;

#[derive(Debug)]
pub struct CompoundTask<'a> {
    /// Original record for fields unchanged by the admitted operations.
    pub base: &'a TaskRecord,
    pub placed: Option<u64>,
    pub phase: Phase,
    pub revision: u64,
    pub revisions: Revisions,
    pub tracked: Option<u32>,
    pub executor: Executor,
    pub taken_by: Option<u64>,
    pub requester: Party,
    pub funder: Funder,
    pub numbers: Numbers,
    pub root: u64,
    pub depth: u32,
    pub last_message: u64,
    pub inbox: &'a [crate::Word],
    pub instructions: &'a Option<Box<[u8]>>,
    pub spec: &'a crate::Spec,
    pub wake: crate::WakePolicy,
    pub dependencies: &'a [u64],
    pub waiting_on: &'a [u64],
    pub added: &'a [crate::Added],
    pub authority: &'a crate::Authority,
    pub llm: &'a Option<crate::LlmChoice>,
    pub kept_account: Option<u32>,
    pub grace: Option<Grace>,
}

impl Deref for CompoundTask<'_> {
    type Target = TaskRecord;
    fn deref(&self) -> &TaskRecord {
        self.base
    }
}

impl CompoundTask<'_> {
    /// Remaining lifetime delegation allowance under the projected authority.
    #[must_use]
    pub fn tasks_left(&self) -> Option<u32> {
        self.authority.delegation.tasks.checked_sub(self.made.checked_sub(1)?)
    }
    /// Reservation a new requester must fund, excluding spend already charged.
    #[must_use]
    pub fn takeover_budget(&self) -> Option<u64> {
        crate::funders::remaining(self.numbers)
    }
    /// Whether a different requester may take over the projected mutable task.
    #[must_use]
    pub fn takeover_state(&self, person: u64) -> bool {
        person != 0 && self.requester != Party::Person(person) && crate::moving::mutable(&self.phase)
    }

    pub(crate) fn settings(&self) -> Settings {
        Settings {
            tracked: self.tracked,
            addressed: match self.executor {
                Executor::Person(address) => Some(address),
                Executor::Agent { .. } | Executor::Procedure { .. } => None,
            },
            taken_by: self.taken_by,
            instructions: self.instructions.clone(),
            spec: self.spec.clone(),
            wake: self.wake,
            dependencies: self.dependencies.into(),
            authority: self.authority.clone(),
            llm: self.llm.clone(),
        }
    }
    fn wake(&mut self, word: &crate::Word) {
        let wakes = match self.executor {
            Executor::Person(_) => false,
            Executor::Procedure { .. } | Executor::Agent { .. } => match word.kind {
                crate::MessageKind::Answer { .. } => self.wake.answers,
                crate::MessageKind::Question => self.wake.questions,
                crate::MessageKind::EffectNotMade { .. }
                | crate::MessageKind::Words
                | crate::MessageKind::Proposal { .. }
                | crate::MessageKind::Escalation { .. }
                | crate::MessageKind::FinishRequested { .. }
                | crate::MessageKind::Amendment { .. }
                | crate::MessageKind::Met { .. }
                | crate::MessageKind::ProposalDecision { .. }
                | crate::MessageKind::PersonStalled { .. }
                | crate::MessageKind::Result(_)
                | crate::MessageKind::Notice { .. }
                | crate::MessageKind::Timer { .. }
                | crate::MessageKind::News { .. } => true,
            },
        };
        let blocked = !crate::run::ends::pending(self.waiting_on, self.flags.ended_by).may_be_due(self.executor);
        if wakes && !blocked && self.phase == Phase::Active(Active::Idle) {
            self.phase = Phase::Active(Active::Due);
        }
    }
}

impl Staged {
    /// A request changes each task's placed hold at most once. Scan retained
    /// plans rather than allocating another live-task table.
    pub(crate) fn check_placed_change(&self, plan: &Plan) -> Result<(), Box<crate::Problem>> {
        if let Some(placement) = plan.placement() {
            for number in &placement.tasks {
                self.placed_change_once(*number)?;
            }
        }
        if let Some(change) = plan.placed_change() {
            for (number, _) in &change.tasks {
                self.placed_change_once(*number)?;
            }
        }
        Ok(())
    }

    pub(crate) fn placed_change_once(&self, number: u64) -> Result<(), Box<crate::Problem>> {
        for earlier in &self.plans {
            let hold = if let Some(placement) = earlier.placement()
                && super::has(&placement.tasks, number)
            {
                Some(placement.hold.number)
            } else if let Some(change) = earlier.placed_change() {
                let mut changed = false;
                for (task, _) in &change.tasks {
                    changed |= *task == number;
                }
                if changed { Some(change.hold.number) } else { None }
            } else {
                None
            };
            if let Some(hold) = hold {
                return Err(Box::new(crate::Problem::new(Some(number), crate::Refusal::Placed { hold })));
            }
        }
        Ok(())
    }

    /// Answers admitted earlier in this compound close their question positions.
    pub(crate) fn answered_question(&self, task: u64, question: u64) -> bool {
        for plan in &self.plans {
            if let Plan::Message(plan) = plan
                && plan.number == task
                && plan.prepared.word.kind == (crate::MessageKind::Answer { question })
            {
                return true;
            }
        }
        false
    }

    pub(crate) fn record<'a>(&'a self, domain: &'a crate::Domain, number: u64) -> Option<CompoundTask<'a>> {
        let base = crate::domain::record(domain, number)?;
        let mut row = borrowed(base);
        for (at, plan) in self.plans.as_slice().iter().enumerate() {
            if let Plan::Cancel(cancel) = plan
                && let Some(selected) = cancel.closing()
                && crate::batch::contains(selected, number)
            {
                row.phase = crate::closing::cancelled(&row.phase, cancel.by, cancel.at, &cancel.reason);
                row.revision = row.revision.checked_add(1).expect("admission checked");
            }
            project_placed(&mut row, plan);
            match plan {
                Plan::Move { plan, .. } => {
                    if super::has(&plan.subtree, number) {
                        row.root = plan.number;
                        row.depth = row.depth.checked_sub(plan.previous_depth).expect("subtree depth checked");
                        row.revision = row.revision.checked_add(1).expect("admission checked");
                    }
                    if plan.number == number {
                        row.requester = Party::Person(plan.person);
                        if plan.source != plan.destination {
                            row.funder = plan.destination;
                            row.numbers =
                                Numbers { budget: plan.left, spent: 0, spent_below: 0, reserved: row.numbers.reserved };
                        }
                    }
                }
                Plan::Amend(plan) if plan.number == number => {
                    project_amend(&mut row, plan);
                    if plan.release.is_some() {
                        project_dependency_hold(domain, &mut row);
                    }
                }
                Plan::Release(plan) if plan.number == number => {
                    row.phase = crate::run::ends::pending(row.waiting_on, row.flags.ended_by)
                        .amended(row.executor, plan.next.clone());
                    if let Some(hold) = row.placed {
                        row.phase = Phase::Held {
                            was: crate::run::was(row.phase.clone()),
                            why: crate::Hold::Placed { hold },
                            source: None,
                        };
                    }
                    row.revision = row.revision.checked_add(1).expect("admission checked");
                    project_dependency_hold(domain, &mut row);
                    if let Some(notice) = &plan.notice {
                        row.last_message = notice.word.number;
                        row.inbox = &notice.inbox;
                    }
                }
                Plan::Message(plan) if plan.number == number => {
                    row.last_message = plan.prepared.word.number;
                    row.inbox = &plan.prepared.inbox;
                    row.wake(&plan.prepared.word);
                }
                Plan::Cancel(plan) if plan.number == number => match &plan.finish {
                    Some(crate::grace::FinishPlan::Word { finish, reason, prepared }) => {
                        row.grace = Some(Grace {
                            by: finish.by,
                            until: finish.until,
                            reason: reason.clone(),
                            message: finish.message,
                        });
                        row.last_message = finish.message;
                        row.inbox = &prepared.inbox;
                        row.revision = row.revision.checked_add(1).expect("admission checked");
                        row.wake(&prepared.word);
                    }
                    Some(crate::grace::FinishPlan::Sooner { finish, reason }) => {
                        let grace = row.grace.as_mut().expect("earlier grace");
                        grace.by = finish.by;
                        grace.until = finish.until;
                        grace.reason.clone_from(reason);
                    }
                    Some(crate::grace::FinishPlan::Cancel { .. } | crate::grace::FinishPlan::Unchanged) | None => {}
                },
                Plan::Amend(_) | Plan::Release(_) | Plan::Message(_) | Plan::Stop(_) | Plan::Cancel(_) => {}
            }
            if let Some(Some((changed, circle))) = self.circles.get(u32::try_from(at).expect("bounded operations"))
                && *changed == number
            {
                row.phase = Phase::Held {
                    was: crate::run::was(row.phase.clone()),
                    why: crate::Hold::Circle,
                    source: Some(Box::new(crate::HoldSource::Circle(circle.clone()))),
                };
            }
        }
        if let Some(numbers) = self.funding_numbers(Funder::Task(number)) {
            row.numbers = numbers;
        }
        Some(row)
    }

    /// An amendment can return a waiter to its queue as well as release a hold (6.2 and 6.7).
    pub(crate) fn amended_phase(
        &self,
        domain: &crate::Domain,
        limits: &crate::Limits,
        plan: &crate::control::AmendPlan,
    ) -> Phase {
        let mut row = self.record(domain, plan.number).expect("admitted amendment");
        project_amend(&mut row, plan);
        if plan.release.is_some() || (row.phase == Phase::Waiting && row.waiting_on.is_empty() && !row.holds_taken) {
            crate::control::returned_phase(
                domain,
                self,
                limits,
                plan.number,
                row.phase,
                row.dependencies,
                row.waiting_on,
            )
        } else {
            row.phase
        }
    }

    pub(crate) fn returned_circle(
        &self,
        domain: &crate::Domain,
        limits: &crate::Limits,
        plan: &Plan,
    ) -> Option<(u64, crate::Circle)> {
        match plan {
            Plan::Amend(plan) => returned_circle(plan.number, self.amended_phase(domain, limits, plan)),
            Plan::Release(plan) => {
                let row = self.record(domain, plan.number).expect("admitted release");
                let phase = crate::run::ends::pending(row.waiting_on, row.flags.ended_by)
                    .amended(row.executor, plan.next.clone());
                returned_circle(
                    plan.number,
                    crate::control::returned_phase(
                        domain,
                        self,
                        limits,
                        plan.number,
                        phase,
                        row.dependencies,
                        row.waiting_on,
                    ),
                )
            }
            Plan::Cancel(_) | Plan::Move { .. } | Plan::Message(_) | Plan::Stop(_) => None,
        }
    }

    pub(crate) fn funding_numbers(&self, funder: Funder) -> Option<Numbers> {
        for (key, numbers) in self.funding.iter().rev() {
            if *key == funder {
                return Some(*numbers);
            }
        }
        None
    }
    pub(crate) fn numbers(&self, domain: &crate::Domain, funder: Funder) -> Option<Numbers> {
        if let Some(numbers) = self.funding_numbers(funder) {
            return Some(numbers);
        }
        match funder {
            Funder::Task(task) => Some(self.record(domain, task)?.numbers),
            Funder::Pool { .. } | Funder::Period { .. } | Funder::Recurring { .. } => {
                Some(domain.funding.get(&funder)?.numbers)
            }
        }
    }
    pub(crate) fn descendant(&self, domain: &crate::Domain, number: u64, ancestor: u64, bound: u32) -> bool {
        let mut at = Some(number);
        for _ in 0..bound {
            let Some(number) = at else { return false };
            if number == ancestor {
                return true;
            }
            at = match self.record(domain, number) {
                Some(row) => match row.requester {
                    Party::Task(parent) => Some(parent),
                    Party::Person(_) | Party::Deployment { .. } => None,
                },
                None => None,
            };
        }
        false
    }
    pub(crate) fn detached(&self, number: u64) -> bool {
        super::has(&self.moved, number)
    }
    pub(crate) fn reference(&self, domain: &crate::Domain, number: u64, target: u64) -> bool {
        if let Some(row) = crate::domain::record(domain, number)
            && row.references.contains(&target)
        {
            return true;
        }
        for plan in &self.plans {
            if let Plan::Move { plan, .. } = plan
                && let Some(parent) = plan.old_parent
                && ((number == parent && target == plan.number) || (number == plan.number && target == parent))
            {
                return true;
            }
        }
        false
    }
    pub(crate) fn references(&self, domain: &crate::Domain, number: u64) -> usize {
        let mut count = crate::domain::record(domain, number).expect("live reference owner").references.len();
        for plan in &self.plans {
            if let Plan::Move { plan, .. } = plan
                && let Some(parent) = plan.old_parent
            {
                let target = if number == parent {
                    Some(plan.number)
                } else if number == plan.number {
                    Some(parent)
                } else {
                    None
                };
                if let Some(target) = target
                    && !crate::domain::record(domain, number).expect("live owner").references.contains(&target)
                {
                    count = count.checked_add(1).expect("admission checked");
                }
            }
        }
        count
    }
}

impl Staged {
    pub(crate) fn goals(
        &self,
        domain: &crate::Domain,
        project: u32,
        limits: &crate::Limits,
    ) -> Result<(), crate::Refusal> {
        let mut goals = 0_u32;
        for (number, _) in &domain.names {
            let row = self.record(domain, *number).expect("live task");
            if row.project == project && row.tracked.is_some() {
                goals = goals.checked_add(1).expect("admission checked");
            }
        }
        if goals < limits.project_goals { Ok(()) } else { Err(crate::Refusal::Goals) }
    }
    pub(crate) fn address(
        row: &CompoundTask<'_>,
        addressed: Option<&crate::AddressChange>,
    ) -> Result<(), crate::Refusal> {
        if let Some(change) = addressed {
            if !match row.executor {
                Executor::Person(_) => true,
                Executor::Agent { .. } | Executor::Procedure { .. } => false,
            } || !crate::control::valid_address(change.address)
            {
                return Err(crate::Refusal::Executor);
            }
            if change.revision != row.revisions.person {
                return Err(crate::Refusal::PersonRevision { revision: row.revisions.person });
            }
        }
        Ok(())
    }
    pub(crate) fn can_resize(&self, domain: &crate::Domain, number: u64, budget: u64) -> bool {
        let Some(row) = self.record(domain, number) else { return false };
        let mut numbers = row.numbers;
        numbers.budget = budget;
        if crate::funders::available(numbers).is_none() {
            return false;
        }
        let Some(mut source) = self.numbers(domain, row.funder) else { return false };
        let Some(reserved) = source.reserved.checked_sub(row.numbers.budget) else { return false };
        let Some(reserved) = reserved.checked_add(budget) else { return false };
        source.reserved = reserved;
        crate::funders::available(source).is_some()
    }
}

fn project_amend<'a>(row: &mut CompoundTask<'a>, plan: &'a crate::control::AmendPlan) {
    let amendment = &plan.amendment;
    if let Some(tracked) = amendment.tracked {
        row.tracked = match tracked {
            Tracking::Tracked { priority } => Some(priority),
            Tracking::Untracked => None,
        };
    }
    if let Some(address) = amendment.addressed {
        row.executor = Executor::Person(address.address);
        row.taken_by = None;
    }
    if amendment.instructions.is_some() {
        row.instructions = &amendment.instructions;
    }
    if let Some(choice) = &amendment.llm {
        if crate::llm::replaces_account(row.kept_account, choice) {
            row.kept_account = None;
        }
        row.llm = &amendment.llm;
    }
    if let Some(value) = &amendment.spec {
        row.spec = value;
    }
    if let Some(value) = amendment.wake {
        row.wake = value;
    }
    if let Some(value) = &amendment.dependencies {
        row.dependencies = value;
        row.waiting_on = &plan.waiting_on;
        row.added = &plan.added;
    }
    if let Some(value) = &amendment.authority {
        row.authority = value;
    }
    row.revision = row.revision.checked_add(1).expect("admission checked");
    row.revisions.spec = row.revisions.spec.checked_add(u64::from(plan.changed)).expect("admission checked");
    row.revisions.person = row.revisions.person.checked_add(u64::from(plan.person_changed)).expect("admission checked");
    row.phase = crate::run::ends::pending(row.waiting_on, row.flags.ended_by).amended(row.executor, row.phase.clone());
    row.last_message = plan.message;
    row.inbox = &plan.prepared.inbox;
    if plan.attempt.is_none() {
        row.wake(&plan.prepared.word);
    }
    if let Some(release) = &plan.release {
        row.phase =
            crate::run::ends::pending(row.waiting_on, row.flags.ended_by).amended(row.executor, release.next.clone());
        row.revision = row.revision.checked_add(1).expect("admission checked");
    }
}

// Stubs do not change during admission; project the hold and its fresh revision
// without caching another collection alongside the circle judgements.
fn project_dependency_hold(domain: &crate::Domain, row: &mut CompoundTask<'_>) {
    if let Some(task) = crate::control::failed_dependency(domain, &row.phase, row.dependencies) {
        row.phase =
            Phase::Held { was: crate::run::was(row.phase.clone()), why: crate::Hold::Dependency(task), source: None };
        row.revisions.hold = row.revisions.hold.checked_add(1).expect("admission checked");
    }
}

fn returned_circle(number: u64, phase: Phase) -> Option<(u64, crate::Circle)> {
    match phase {
        Phase::Held { why: crate::Hold::Circle, source: Some(source), .. } => match *source {
            crate::HoldSource::Circle(circle) => Some((number, circle)),
            crate::HoldSource::Resource(_) | crate::HoldSource::Effect { .. } => unreachable!("circle source"),
        },
        Phase::Waiting | Phase::Active(_) | Phase::Closing(_) | Phase::Held { .. } | Phase::Ended(_) => None,
    }
}

fn borrowed(base: &TaskRecord) -> CompoundTask<'_> {
    CompoundTask {
        base,
        phase: base.phase.clone(),
        placed: base.placed,
        revision: base.revision,
        revisions: base.revisions,
        tracked: base.tracked,
        executor: base.executor,
        taken_by: base.taken_by,
        requester: base.requester,
        funder: base.funder,
        numbers: base.numbers,
        root: base.root,
        depth: base.depth,
        last_message: base.last_message,
        inbox: &base.inbox,
        instructions: &base.instructions,
        spec: &base.spec,
        wake: base.wake,
        dependencies: &base.dependencies,
        waiting_on: &base.waiting_on,
        added: &base.added,
        authority: &base.authority,
        llm: &base.llm,
        kept_account: base.kept_account,
        grace: base.grace.clone(),
    }
}

fn project_placed(row: &mut CompoundTask<'_>, plan: &Plan) {
    if let Some(changes) = plan.placed_change() {
        for (target, placed) in &changes.tasks {
            if *target == row.number {
                row.placed = *placed;
                if crate::placed::placed_reason(&row.phase).is_some() {
                    row.phase = match &row.phase {
                        Phase::Held { was, .. } => crate::placed::resumed(was),
                        Phase::Waiting | Phase::Active(_) | Phase::Closing(_) | Phase::Ended(_) => {
                            unreachable!("placed phase")
                        }
                    };
                    if let Some(hold) = placed {
                        row.phase = Phase::Held {
                            was: crate::run::was(row.phase.clone()),
                            why: crate::Hold::Placed { hold: *hold },
                            source: None,
                        };
                    }
                }
            }
        }
    }
    if let Some(plan) = plan.placement()
        && super::has(&plan.tasks, row.number)
        && row.placed.is_none()
    {
        row.placed = Some(plan.hold.number);
        if crate::placed::reason(&row.phase).is_none() {
            row.phase = Phase::Held {
                was: crate::run::was(row.phase.clone()),
                why: crate::Hold::Placed { hold: plan.hold.number },
                source: None,
            };
            row.revisions.hold = row.revisions.hold.checked_add(1).expect("admission checked");
        }
    }
}
