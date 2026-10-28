//! Person adoption of a live task, with one reservation transfer and requester-tree rewrite
//! (domain/tasks.md, section 7; domain/authority.md, section 7). Root authenticates the
//! adopter and policy rights; this child checks every local row before changing any of them.
#![expect(
    clippy::result_large_err,
    reason = "bounded owner refusal keeps exact revision and capacity facts inline instead of allocating on refusal"
)]
use crate::domain::{Domain, entrance, publish, record, refused, task_mut};
use crate::{Change, Funder, Limits, Numbers, Party, Phase, Refusal, Request, Was};
use skein_lib::{Env, List, Queue, ReplyTo};

pub(crate) fn mutable(phase: &Phase) -> bool {
    match phase {
        Phase::Waiting | Phase::Active(_) | Phase::Held { was: Was::Waiting | Was::Active(_), .. } => true,
        Phase::Closing(_) | Phase::Held { was: Was::Closing(_), .. } | Phase::Ended(_) => false,
    }
}

/// Current task ownership and phase, shared with the core's offer judgment.
pub(crate) fn state(domain: &Domain, number: u64, person: u64) -> bool {
    let Some(row) = record(domain, number) else { return false };
    person != 0 && row.requester != Party::Person(person) && mutable(&row.phase)
}

#[derive(Debug)]
pub(crate) struct Plan {
    pub(crate) number: u64,
    pub(crate) person: u64,
    pub(crate) pool_budget: u64,
    pub(crate) period_budget: u64,
    pub(crate) previous_root: u64,
    pub(crate) previous_depth: u32,
    pub(crate) source: Funder,
    pub(crate) spent: u64,
    pub(crate) left: u64,
    pub(crate) budget: u64,
    pub(crate) destination: Funder,
    pub(crate) period_source: Funder,
    pub(crate) subtree: List<u64>,
    pub(crate) old_parent: Option<u64>,
}

#[expect(clippy::too_many_arguments, reason = "the existing move carries its person funding and reason")]
pub(crate) fn apply(
    domain: &mut Domain,
    env: &Env<Limits>,
    to: ReplyTo,
    number: u64,
    person: u64,
    period: u64,
    pool_budget: u64,
    period_budget: u64,
    reason: &[u8],
    out: &mut Queue<Request>,
) {
    let plan = match check(
        domain,
        &crate::compound::Staged::new(0),
        env,
        Move { number, person, period, pool_budget, period_budget, reason },
    ) {
        Ok(plan) => plan,
        Err(problem) => return refuse(domain, env, to, problem, out),
    };
    make(domain, env, plan, reason, out);
    reroute(domain, env, out);
    out.push(Request::Done { reply_to: to });
}

pub(crate) fn refuse(
    domain: &mut Domain,
    env: &Env<Limits>,
    to: ReplyTo,
    problem: crate::Problem,
    out: &mut Queue<Request>,
) {
    match problem.capacity {
        Some(capacity) => crate::capacity::refuse(domain, env, to, problem.task, problem.why, capacity, out),
        None => refused(to, problem.task, problem.why, out),
    }
}

pub(crate) struct Move<'a> {
    pub number: u64,
    pub person: u64,
    pub period: u64,
    pub pool_budget: u64,
    pub period_budget: u64,
    pub reason: &'a [u8],
}

#[expect(clippy::too_many_lines, reason = "the full existing move preflight remains together")]
pub(crate) fn check(
    domain: &Domain,
    staged: &crate::compound::Staged,
    env: &Env<Limits>,
    moving: Move<'_>,
) -> Result<Plan, crate::Problem> {
    let Move { number, person, period, pool_budget, period_budget, reason } = moving;
    if let Err((_, why)) = entrance(domain, ReplyTo::new(skein_lib::Token::new(0)), number) {
        return Err(crate::Problem::new(Some(number), why));
    }
    let old = staged.record(domain, number).expect("entrance named live task");
    if person == 0 || old.requester == Party::Person(person) || !mutable(&old.phase) {
        return Err(crate::Problem::new(Some(number), Refusal::State));
    }
    if reason.len() > usize::try_from(env.limits.message_bytes).expect("bounded reason size") {
        return Err(crate::Problem::new(Some(number), Refusal::Read));
    }
    let mut unanswered = 0_usize;
    for line in &old.question_lines {
        if line.answer.is_none() {
            unanswered = unanswered.checked_add(1).expect("bounded question positions");
        }
    }
    let mut carried = 0_usize;
    for credit in &old.questions {
        match credit.asked {
            Party::Task(_) => carried = carried.checked_add(1).expect("bounded outgoing questions"),
            Party::Person(_) | Party::Deployment { .. } => {}
        }
    }
    if unanswered.saturating_add(carried) > usize::try_from(env.limits.inbox_messages).expect("question bound") {
        return Err(crate::Problem::new(Some(number), Refusal::Busy));
    }
    let received_before = crate::inbox::received_credits(old.requester, &old.question_lines, &old.questions, old.inbox);
    let received_after =
        crate::inbox::received_credits(Party::Person(person), &old.question_lines, &old.questions, old.inbox);
    let added = u32::try_from(received_after.checked_sub(received_before).expect("move adds person notice credits"))
        .expect("bounded question notices");
    if added > 0 && !crate::inbox::room_staged(domain, staged, &env.limits, number, added, 0) {
        return Err(crate::Problem::capacity(
            Some(number),
            Refusal::Busy,
            crate::Capacity {
                project: Some(old.project),
                by: Party::Person(person),
                action: crate::CapacityAction::Move,
                cause: crate::capacity::inbox(number, &env.limits),
            },
        ));
    }
    let project = old.project;
    let from = old.requester;
    let previous_root = old.root;
    let previous_depth = old.depth;
    let source = old.funder;
    let Some(spent) = crate::funders::total(old.numbers) else {
        return Err(crate::Problem::new(Some(number), Refusal::Funding));
    };
    let Some(left) = crate::funders::remaining(old.numbers) else {
        return Err(crate::Problem::new(Some(number), Refusal::Funding));
    };
    let destination = Funder::Pool { project, person, period };
    let period_source = Funder::Period { project, period };
    for (task, _) in &domain.names {
        let row = staged.record(domain, *task).expect("live name");
        let exhausted_escalation = match row.escalation {
            crate::Escalation::Waiting { revision: u64::MAX, .. } => true,
            crate::Escalation::Waiting { .. }
            | crate::Escalation::Unheld { .. }
            | crate::Escalation::Routing { .. }
            | crate::Escalation::Rejected { .. } => false,
        };
        let exhausted_proposal = match &row.proposal {
            Some(proposal) => match proposal.state {
                crate::ProposalState::Pending { .. } => row.revision == u64::MAX,
                crate::ProposalState::Accepted { .. }
                | crate::ProposalState::Rejected { .. }
                | crate::ProposalState::Withdrawn => false,
            },
            None => false,
        };
        if exhausted_escalation || exhausted_proposal {
            return Err(crate::Problem::new(Some(*task), Refusal::State));
        }
    }
    let mut subtree = List::with_capacity(env.limits.tasks);
    for (child, _) in &domain.names {
        if staged.descendant(domain, *child, number, env.limits.tasks) {
            let row = staged.record(domain, *child).expect("subtree name live");
            if crate::compound::has(&staged.closing, *child)
                || !mutable(&row.phase)
                || row.revision == u64::MAX
                || row.root != previous_root
                || row.depth < previous_depth
            {
                return Err(crate::Problem::new(Some(*child), Refusal::State));
            }
            if let Err(why) = crate::placed::check_detach(domain, staged, number, *child) {
                return Err(crate::Problem::new(Some(*child), why));
            }
            subtree.push(*child).expect("live task bound");
        }
    }
    let old_parent = match from {
        Party::Task(parent) => {
            let Some(_parent_row) = staged.record(domain, parent) else {
                return Err(crate::Problem::new(Some(number), Refusal::Reference));
            };
            if !staged.reference(domain, parent, number)
                && staged.references(domain, parent) >= usize::try_from(env.limits.references).expect("reference bound")
            {
                return Err(crate::Problem::new(Some(number), Refusal::Reference));
            }
            if !staged.reference(domain, number, parent)
                && staged.references(domain, number) >= usize::try_from(env.limits.references).expect("reference bound")
            {
                return Err(crate::Problem::new(Some(number), Refusal::Reference));
            }
            Some(parent)
        }
        Party::Person(_) | Party::Deployment { .. } => None,
    };
    if source != destination
        && let Err(why) = crate::funders::refunding::check(
            domain,
            staged,
            number,
            destination,
            old.numbers.budget,
            Some(Numbers { budget: pool_budget, spent: 0, spent_below: 0, reserved: 0 }),
        )
    {
        return Err(crate::Problem::new(Some(number), why));
    }
    let create_pool = staged.numbers(domain, destination).is_none();
    let create_period = staged.numbers(domain, period_source).is_none();
    if create_pool && crate::funders::newer_period(domain, project, period) {
        return Err(crate::Problem::new(Some(number), Refusal::Funding));
    }
    if create_period && !create_pool {
        return Err(crate::Problem::new(Some(number), Refusal::Funding));
    }
    if create_pool {
        let period_numbers = match staged.numbers(domain, period_source) {
            Some(numbers) => numbers,
            None => {
                for (funder, _) in &domain.funding {
                    match *funder {
                        Funder::Period { project: old_project, period: old_period }
                            if old_project == project && old_period >= period =>
                        {
                            return Err(crate::Problem::new(Some(number), Refusal::Funding));
                        }
                        Funder::Task(_) | Funder::Pool { .. } | Funder::Recurring { .. } | Funder::Period { .. } => {}
                    }
                }
                Numbers { budget: period_budget, spent: 0, spent_below: 0, reserved: 0 }
            }
        };
        let mut after = period_numbers;
        after.reserved = match after.reserved.checked_add(pool_budget) {
            Some(reserved) => reserved,
            None => return Err(crate::Problem::new(Some(number), Refusal::Funding)),
        };
        if crate::funders::available(after).is_none() {
            return Err(crate::Problem::new(Some(number), Refusal::Funding));
        }
        let needed = if create_period { 2 } else { 1 };
        if match domain.funding.len().checked_add(needed) {
            Some(count) => count > domain.funding.capacity(),
            None => true,
        } {
            return Err(crate::Problem::capacity(
                Some(number),
                Refusal::Busy,
                crate::Capacity {
                    project: Some(project),
                    by: Party::Person(person),
                    action: crate::CapacityAction::Move,
                    cause: crate::CapacityCause::Funders { limit: env.limits.funders },
                },
            ));
        }
    }
    Ok(Plan {
        number,
        person,
        pool_budget,
        period_budget,
        previous_root,
        previous_depth,
        source,
        spent,
        left,
        budget: old.numbers.budget,
        destination,
        period_source,
        subtree,
        old_parent,
    })
}

pub(crate) fn make(domain: &mut Domain, env: &Env<Limits>, plan: Plan, reason: &[u8], out: &mut Queue<Request>) {
    let Plan {
        number,
        person,
        pool_budget,
        period_budget,
        previous_root,
        previous_depth,
        source,
        spent: _,
        left: _,
        budget: _,
        destination,
        period_source,
        subtree,
        old_parent,
    } = plan;
    let create_period = domain.funding.get(&period_source).is_none();
    let create_pool = domain.funding.get(&destination).is_none();
    if create_period {
        let record = crate::FundingRecord {
            funder: period_source,
            parent: None,
            period: Some(crate::PeriodTimes { began: env.wall, next: None }),
            numbers: Numbers { budget: period_budget, spent: 0, spent_below: 0, reserved: 0 },
            made: 0,
            closed: false,
        };
        assert!(domain.funding.insert(period_source, record) == Ok(None), "period room preflighted");
        crate::funders::save_funding(domain, period_source, out);
    }
    if create_pool {
        let period_row = domain.funding.get_mut(&period_source).expect("period preflighted");
        period_row.numbers.reserved =
            period_row.numbers.reserved.checked_add(pool_budget).expect("pool carve preflighted");
        let record = crate::FundingRecord {
            funder: destination,
            parent: Some(period_source),
            period: None,
            numbers: Numbers { budget: pool_budget, spent: 0, spent_below: 0, reserved: 0 },
            made: 0,
            closed: false,
        };
        assert!(domain.funding.insert(destination, record) == Ok(None), "pool room preflighted");
        crate::funders::save_funding(domain, period_source, out);
        crate::funders::save_funding(domain, destination, out);
    }
    if source != destination {
        let budget = record(domain, number).expect("target live").numbers.budget;
        crate::funders::refunding::apply(domain, env, number, destination, budget, out);
    }
    if let Some(parent) = old_parent {
        out.push(Request::Erase { key: crate::Key::Delegate { requester: parent, task: number } });
        let parent_row = task_mut(domain, parent).expect("old requester live");
        let mut delegates = List::with_capacity(env.limits.delegates);
        for delegate in &parent_row.record.delegates {
            if *delegate != number {
                delegates.push(*delegate).expect("delegate subset");
            }
        }
        parent_row.record.delegates = delegates.into_boxed();
        if !parent_row.record.references.contains(&number) {
            let mut references = List::with_capacity(env.limits.references);
            for reference in &parent_row.record.references {
                references.push(*reference).expect("old references bounded");
            }
            references.push(number).expect("reference room preflighted");
            parent_row.record.references = references.into_boxed();
        }
        publish(domain, env, parent, out);
    }
    {
        let task = task_mut(domain, number).expect("moving target live");
        task.record.requester = Party::Person(person);
        if let Some(parent) = old_parent
            && !task.record.references.contains(&parent)
        {
            let mut references = List::with_capacity(env.limits.references);
            for reference in &task.record.references {
                references.push(*reference).expect("old references bounded");
            }
            references.push(parent).expect("bilateral reference room preflighted");
            task.record.references = references.into_boxed();
        }
    }
    crate::questions::moved(domain, env, number, person, out);
    for child in subtree.into_boxed() {
        let row = task_mut(domain, child).expect("subtree live");
        assert!(row.record.root == previous_root, "subtree has one root");
        row.record.root = number;
        row.record.depth = row.record.depth.checked_sub(previous_depth).expect("subtree depth preflight");
        crate::control::history(domain, child, env.wall, Party::Person(person), Change::Moved, reason, out);
        publish(domain, env, child, out);
    }
}

pub(crate) fn reroute(domain: &Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    let mut waiting = List::with_capacity(env.limits.tasks);
    for (task, _) in &domain.names {
        waiting.push(*task).expect("live task bound");
    }
    for task in waiting.into_boxed() {
        let row = record(domain, task).expect("live name");
        match row.escalation {
            crate::Escalation::Waiting { .. } => out.push(Request::EscalationNeeded {
                context: crate::escalation::context(domain, task).expect("waiting task context"),
            }),
            crate::Escalation::Unheld { .. }
            | crate::Escalation::Routing { .. }
            | crate::Escalation::Rejected { .. } => {}
        }
        if let Some(proposal) = &row.proposal {
            match proposal.state {
                crate::ProposalState::Pending { .. } => {
                    out.push(Request::ProposalRerouteNeeded { proposer: task, proposal: proposal.number });
                }
                crate::ProposalState::Accepted { .. }
                | crate::ProposalState::Rejected { .. }
                | crate::ProposalState::Withdrawn => {}
            }
        }
    }
}
