//! Fenced procedure decisions at the tasks hub (domain/tasks.md, sections 5.3, 5.7 and 8.2).
//! The owner keeps procedure state and chooses a step; tasks keeps only the
//! executor identity, live topology, result contract and the step's offered inbox fence. It never
//! knows connector facts or effects. A due step may make one whole batch,
//! finish after its delegates, hold, or wait for changed facts.
#![expect(clippy::result_large_err, reason = "bounded owner refusal carries exact capacity diagnostics inline")]
use crate::domain::{Domain, make, publish, record, refused, task_mut};
use crate::{
    Active, Closing, Ending, Executor, Limits, Party, Phase, ProcedureDecision, ProcedureEnd, Refusal, Request, Stage,
    TaskResult,
};
use alloc::boxed::Box;
use skein_lib::{Env, Queue, ReplyTo};

#[expect(clippy::too_many_arguments, reason = "one fenced procedure decision")]
pub(crate) fn stepped(
    domain: &mut Domain,
    env: &Env<Limits>,
    to: ReplyTo,
    number: u64,
    step: u64,
    read: Option<u64>,
    decision: ProcedureDecision,
    out: &mut Queue<Request>,
) {
    let token = to.into_token();
    let mut requests = Queue::with_capacity(crate::max_out(&env.limits));
    apply(domain, env, Fence { to: ReplyTo::new(token), number, step, read }, decision, &mut requests);
    let mut terminal = false;
    for _ in 0..requests.len() {
        let request = requests.pop().expect("step output count");
        if let Request::Done { .. } | Request::Sent { .. } = &request {
            continue;
        }
        if let Request::Refused { problem, .. } = request {
            terminal = true;
            out.push(Request::ProcedureRefused { task: number, step, problem });
        } else {
            if let Request::Made { .. } = &request {
                terminal = true;
            }
            out.push(request);
        }
    }
    if !terminal {
        out.push(Request::Done { reply_to: ReplyTo::new(token) });
    }
}

struct Fence {
    to: ReplyTo,
    number: u64,
    step: u64,
    read: Option<u64>,
}

fn apply(domain: &mut Domain, env: &Env<Limits>, fence: Fence, decision: ProcedureDecision, out: &mut Queue<Request>) {
    let Fence { to, number, step, read } = fence;
    if !domain.ready() {
        return refused(to, Some(number), Refusal::NotReady, out);
    }
    let Some(task) = record(domain, number) else {
        return refused(to, Some(number), Refusal::Unknown, out);
    };
    match task.executor {
        Executor::Procedure { .. } => {}
        Executor::Agent { .. } | Executor::Person(_) => return refused(to, Some(number), Refusal::Executor, out),
    }
    if task.phase != Phase::Active(Active::Due) || task.attempt.checked_add(1) != Some(step) {
        return refused(to, Some(number), Refusal::Attempt, out);
    }
    if read != domain.procedure_offered(number) {
        return refused(to, Some(number), Refusal::Read, out);
    }
    if let Err(problem) = check(domain, env, number, &decision) {
        return crate::capacity::refused(domain, env, to, problem, out);
    }
    crate::inbox::keep_refusals(domain, number, &[]);
    let live = task_mut(domain, number).expect("procedure step preflighted");
    live.record.attempt = step;
    crate::inbox::take(&mut live.record, read);
    live.procedure_offered = None;
    task_mut(domain, number).expect("procedure task live").record.phase = Phase::Active(Active::Idle);
    let made = !decision.batch.is_empty();
    if made {
        make(domain, env, to, Party::Task(number), decision.batch, out);
    }
    for part in decision.amendments {
        crate::control::amend(
            domain,
            env,
            ReplyTo::new(skein_lib::Token::new(u64::MAX - 1)),
            Party::Task(number),
            crate::control::Amend {
                places: Box::new([]),
                number: part.task,
                message: part.message,
                stop_run: part.stop_run,
                funder: None,
                amendment: part.amendment,
            },
            out,
        );
    }
    let project = record(domain, number).expect("procedure task live").project;
    for part in decision.messages {
        crate::inbox::message(
            domain,
            env,
            ReplyTo::new(skein_lib::Token::new(u64::MAX - 1)),
            project,
            part.task,
            part.word,
            false,
            out,
        );
    }
    for proposal in decision.proposals {
        crate::proposals::propose(domain, env, ReplyTo::new(skein_lib::Token::new(u64::MAX - 1)), proposal, out);
    }
    for part in decision.cancels {
        crate::control::apply(
            domain,
            env,
            ReplyTo::new(skein_lib::Token::new(u64::MAX - 1)),
            Party::Task(number),
            part.task,
            crate::Control::Cancel { reason: part.reason, grace: part.grace, message: part.message },
            out,
        );
    }
    match decision.end {
        ProcedureEnd::Result(result) => {
            let ending = match result {
                TaskResult::Failure { reason } => Ending::Failed { reason },
                TaskResult::Approved { .. }
                | TaskResult::Report { .. }
                | TaskResult::Verdict { .. }
                | TaskResult::Change { .. } => Ending::Done(result),
            };
            task_mut(domain, number).expect("procedure task live").record.phase =
                Phase::Closing(Closing { stage: Stage::Delegates, ending });
            publish(domain, env, number, out);
        }
        ProcedureEnd::Hold(why) => crate::run::hold(domain, env, number, why, out),
        ProcedureEnd::Idle => idle(domain, env, number, out),
        ProcedureEnd::Wait => publish(domain, env, number, out),
    }
    crate::wake::after_step(domain, env, number, out);
}

/// Only the connector can say a procedure has finished draining its work.
fn idle(domain: &mut Domain, env: &Env<Limits>, number: u64, out: &mut Queue<Request>) {
    let row = record(domain, number).expect("procedure task live");
    if crate::placed::draining(domain, row) {
        let hold = row.placed.expect("draining procedure has a hold");
        task_mut(domain, number).expect("procedure task live").record.phase =
            Phase::Held { was: crate::Was::Active(Active::Due), why: crate::Hold::Placed { hold }, source: None };
    }
    publish(domain, env, number, out);
}

fn problem(task: u64, why: Refusal) -> crate::Problem {
    crate::Problem { task: Some(task), why, blocked_by: None, capacity: None }
}

fn check(
    domain: &mut Domain,
    env: &Env<Limits>,
    number: u64,
    decision: &ProcedureDecision,
) -> Result<(), crate::Problem> {
    let task = record(domain, number).expect("step task checked");
    if decision.cancels.len() > usize::try_from(env.limits.batch).expect("batch fits")
        || decision.amendments.len() > usize::try_from(env.limits.batch).expect("batch fits")
        || decision.messages.len() > usize::try_from(env.limits.batch).expect("batch fits")
        || decision.proposals.len() > 1
    {
        return Err(problem(number, Refusal::Batch));
    }
    if let ProcedureEnd::Result(result) = &decision.end {
        let end = crate::End::Finished { result: result.clone(), cancel_delegates: false };
        if crate::run::ends::kept(domain, number).parks(&end) {
            return Err(crate::Problem::naming(Some(number), Refusal::Kept, &task.waiting_on));
        }
        if !task.delegates.is_empty() || !decision.batch.is_empty() {
            return Err(crate::Problem::live_delegates(number, &task.delegates));
        }
        if !crate::run::valid_result(&task.contract, result, &env.limits) {
            return Err(problem(number, Refusal::Contract));
        }
    }
    if !decision.batch.is_empty() {
        crate::batch::check(domain, &env.limits, Party::Task(number), &decision.batch)?;
    }
    // Stage only the admission fields; no lifecycle, timer, history or output changes escape preflight.
    let mut rows = skein_lib::List::with_capacity(env.limits.tasks);
    for (number, _) in &domain.names {
        rows.push((*number, record(domain, *number).expect("live task").clone())).expect("bounded preview");
    }
    let mut funding = skein_lib::List::with_capacity(env.limits.funders);
    for (funder, row) in &domain.funding {
        funding.push((*funder, row.numbers)).expect("bounded funding preview");
    }
    let result = check_parts(domain, env, number, decision);
    for (number, row) in rows.into_boxed() {
        task_mut(domain, number).expect("preview keeps live tasks").record = row;
    }
    for (funder, numbers) in funding.into_boxed() {
        domain.funding.get_mut(&funder).expect("preview keeps funders").numbers = numbers;
    }
    result
}

fn check_parts(
    domain: &mut Domain,
    env: &Env<Limits>,
    number: u64,
    decision: &ProcedureDecision,
) -> Result<(), crate::Problem> {
    let project = record(domain, number).expect("step task checked").project;
    let parent = &mut task_mut(domain, number).expect("step task checked").record;
    let mut delegates = skein_lib::List::with_capacity(env.limits.delegates);
    let mut goals = 0_u32;
    for task in &parent.delegates {
        delegates.push(*task).expect("bounded delegates");
    }
    for task in &decision.batch {
        if task.tracked.is_some() {
            goals = goals.checked_add(1).expect("batch fits u32");
        }
        delegates.push(task.number).expect("batch preflighted");
        parent.numbers.reserved = parent.numbers.reserved.checked_add(task.numbers.budget).expect("batch preflighted");
    }
    parent.delegates = delegates.into_boxed();
    for part in &decision.amendments {
        if let Err(problem) = crate::control::preview_amend(domain, env, Party::Task(number), part) {
            return Err(*problem);
        }
        if let Err(why) = crate::goals::room(domain, project, goals, &env.limits) {
            return Err(problem(part.task, why));
        }
    }
    for part in &decision.messages {
        if part.word.from != Party::Task(number) {
            return Err(problem(part.task, Refusal::Reference));
        }
        crate::inbox::preview_message(domain, env, project, part.task, part.word.clone())?;
    }
    for proposal in &decision.proposals {
        if proposal.proposer != number {
            return Err(problem(number, Refusal::Reference));
        }
        if let Some(problem) = crate::proposals::admission_problem(domain, &env.limits, proposal) {
            return Err(problem);
        }
    }
    check_cancels(domain, env, number, &decision.cancels)
}

fn check_cancels(
    domain: &Domain,
    env: &Env<Limits>,
    number: u64,
    cancels: &[crate::ProcedureCancel],
) -> Result<(), crate::Problem> {
    let mut staged = crate::compound::Staged::new(env.limits.tasks);
    for part in cancels {
        if record(domain, part.task).is_none() {
            return Err(problem(part.task, Refusal::Unknown));
        }
        if !crate::control::allowed(domain, part.task, Party::Task(number), env.limits.tasks) {
            return Err(problem(part.task, Refusal::Reference));
        }
        if staged.closing.as_slice().contains(&part.task) {
            return Err(problem(part.task, Refusal::State));
        }
        let plan = match crate::control::check_cancel(
            domain,
            &staged,
            env,
            Party::Task(number),
            crate::control::Cancel {
                number: part.task,
                reason: part.reason.clone(),
                grace: part.grace,
                message: part.message,
            },
        ) {
            Ok(plan) => plan,
            Err(problem) => return Err(*problem),
        };
        let plan = crate::compound::Plan::Cancel(Box::new(plan));
        staged.after(domain, &env.limits, &plan);
        assert!(staged.plans.push(plan).is_ok(), "cancel batch bounded");
    }
    Ok(())
}
