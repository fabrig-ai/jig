//! Whole procedure-step admission and the connector's same-decision outcome.
use crate::{Ask, Core, Limits, ProcedureStep, Request, Requests, StepEnd, StepRefusal};
use alloc::boxed::Box;
use jig_domain_tasks as tasks;
use skein_lib::{Env, List, Queue};

pub(crate) fn problem(task: u64, why: tasks::Refusal) -> StepRefusal {
    StepRefusal::Refused(tasks::Problem { task: Some(task), why, blocked_by: None, capacity: None })
}

pub(super) fn refused(core: &Core, task: u64, step: u64, why: StepRefusal) -> Requests {
    let mut out = Queue::with_capacity(2);
    if let Some(row) = core.tasks.task(task)
        && let tasks::Executor::Procedure { connector, .. } = row.executor
    {
        out.push(Request::Ask { connector, ask: Ask::StepRefused { task, step, why } });
    }
    out.push(Request::Decided);
    Requests::Out(out)
}

#[expect(clippy::result_large_err, reason = "step refusals retain exact owner diagnostics")]
pub(super) fn parts(
    core: &mut Core,
    env: &Env<Limits>,
    task: u64,
    step: &ProcedureStep,
) -> Result<tasks::ProcedureDecision, StepRefusal> {
    let mut amendments = List::with_capacity(env.limits.tasks.batch);
    let mut messages = List::with_capacity(env.limits.tasks.batch);
    for part in &step.amendments {
        let mut amendment = part.amendment.clone();
        core.keep_models(part.task, &mut amendment);
        let stop_run =
            match core.task_amend_admit(task, part.task, &amendment, env.limits.tasks.depth, &env.limits.tasks) {
                Ok(stop_run) => stop_run,
                Err(crate::TaskAmendDenied::Refused { task, why }) => return Err(problem(task, why)),
                Err(crate::TaskAmendDenied::Denied(answer)) => {
                    return Err(StepRefusal::Denied { answer, findings: Box::new([]) });
                }
            };
        if let Err(jig_domain_people::Refusal::Moved { now }) =
            core.party_amend_revision(part.task, part.revision, &amendment)
        {
            return Err(StepRefusal::Moved { task: part.task, now });
        }
        let Some(message) = crate::fresh(&mut core.counters, crate::Family::Message) else {
            return Err(problem(task, tasks::Refusal::Busy));
        };
        if amendments.push(tasks::ProcedureAmendment { task: part.task, message, stop_run, amendment }).is_err() {
            return Err(problem(task, tasks::Refusal::Batch));
        }
    }
    let Some(row) = core.tasks.task(task) else { return Err(problem(task, tasks::Refusal::Unknown)) };
    for part in &step.messages {
        if row.requester != tasks::Party::Task(part.target)
            && !row.delegates.contains(&part.target)
            && !row.references.contains(&part.target)
        {
            return Err(problem(part.target, tasks::Refusal::Reference));
        }
        let Some(number) = crate::fresh(&mut core.counters, crate::Family::Message) else {
            return Err(problem(task, tasks::Refusal::Busy));
        };
        if messages
            .push(tasks::ProcedureMessage {
                task: part.target,
                word: tasks::Word {
                    number,
                    from: tasks::Party::Task(task),
                    kind: part.kind.clone(),
                    words: part.words.clone(),
                    at: env.wall,
                    hits: 1,
                    eligible: true,
                },
            })
            .is_err()
        {
            return Err(problem(task, tasks::Refusal::Batch));
        }
    }
    let end = match &step.end {
        StepEnd::Wait => tasks::ProcedureEnd::Wait,
        StepEnd::Hold(why) => tasks::ProcedureEnd::Hold(*why),
        StepEnd::HoldFrom { why, source } => tasks::ProcedureEnd::HoldFrom { why: *why, source: source.clone() },
        StepEnd::Result(result) => tasks::ProcedureEnd::Result(result.clone()),
    };
    Ok(tasks::ProcedureDecision {
        batch: Box::new([]),
        amendments: amendments.into_boxed(),
        cancels: cancels(core, env, &step.cancels)?,
        messages: messages.into_boxed(),
        proposals: proposals(core, env, task, &step.proposals)?,
        end,
    })
}

pub(super) fn apply(core: &mut Core, env: &Env<Limits>, task: u64, step: u64, event: tasks::Event) -> Requests {
    let tasks::Event::Procedure { decision, .. } = &event else { unreachable!("procedure event") };
    let mut revised = List::with_capacity(env.limits.tasks.batch.checked_mul(2).expect("step revision room"));
    let mut first = None;
    for member in &decision.batch {
        if first.is_none() {
            first = Some(member.number);
        }
        revised.push(member.number).expect("batch bounded");
    }
    for part in &decision.amendments {
        if !revised.as_slice().contains(&part.task) {
            revised.push(part.task).expect("amendments bounded");
        }
    }
    let mut child = Queue::uncapped();
    tasks::step(&mut core.tasks, &Env { now: env.now, wall: env.wall, limits: env.limits.tasks }, event, &mut child);
    let mut kept = Queue::uncapped();
    let mut refusal = None;
    for _ in 0..child.len() {
        let request = child.pop().expect("task output count");
        if let tasks::Request::ProcedureRefused { problem, .. } = request {
            refusal = Some(StepRefusal::Refused(problem));
        } else {
            kept.push(request);
        }
    }
    if let Some(why) = refusal {
        return refused(core, task, step, why);
    }
    let Requests::Out(mut routed) = super::task_routes::tag_tasks(core, env, kept, false, false, false);
    let mut out = Queue::uncapped();
    for _ in 0..routed.len() {
        let request = routed.pop().expect("routed step count");
        if let Request::Decided = &request {
            continue;
        }
        out.push(request);
    }
    let mut revisions = List::with_capacity(revised.capacity());
    for number in revised.into_boxed() {
        revisions
            .push((number, core.tasks.task(number).expect("made or amended task").revisions.spec))
            .expect("revision room");
    }
    if let Some(row) = core.tasks.task(task)
        && let tasks::Executor::Procedure { connector, .. } = row.executor
    {
        out.push(Request::Ask {
            connector,
            ask: Ask::StepMade { task, step, first, revisions: revisions.into_boxed() },
        });
    }
    out.push(Request::Decided);
    Requests::Out(out)
}

pub(crate) fn append_problem(core: &Core, task: u64, step: u64, problem: tasks::Problem, out: &mut Queue<Request>) {
    let Requests::Out(mut requests) = refused(core, task, step, StepRefusal::Refused(problem));
    for _ in 0..requests.len() {
        if let Some(Request::Ask { connector, ask }) = requests.pop() {
            out.push(Request::Ask { connector, ask });
        }
    }
}

#[expect(clippy::result_large_err, reason = "step refusals retain exact owner diagnostics")]
fn proposals(
    core: &mut Core,
    env: &Env<Limits>,
    task: u64,
    actions: &[tasks::ProposalAction],
) -> Result<Box<[tasks::Proposal]>, StepRefusal> {
    let Some(context) = core.tasks.delegation(task) else { return Err(problem(task, tasks::Refusal::Unknown)) };
    if actions.len() > 1 {
        return Err(problem(task, tasks::Refusal::Busy));
    }
    let mut proposals = List::with_capacity(1);
    for action in actions {
        let mut action = action.clone();
        core.keep_proposal_models(task, &mut action);
        match &action {
            tasks::ProposalAction::Batch(_)
            | tasks::ProposalAction::Amend { .. }
            | tasks::ProposalAction::Widen { .. } => {}
            tasks::ProposalAction::Settings { .. }
            | tasks::ProposalAction::Effect { .. }
            | tasks::ProposalAction::ProcedureEffect { .. }
            | tasks::ProposalAction::ReleasePlaced { .. }
            | tasks::ProposalAction::Release { .. } => {
                return Err(problem(task, tasks::Refusal::Unproposable));
            }
        }
        if let tasks::ProposalAction::Amend { amendment, .. } = &action
            && (amendment.authority.is_none() || amendment.release.is_some())
        {
            return Err(problem(task, tasks::Refusal::AuthorityShape));
        }
        if let Err(why) = core.proposal_admit_action(context.project, &action, env.wall) {
            return Err(problem(task, why));
        }
        let Some(holder) = core.proposal_holder(&env.limits, task, &action, None, env.wall) else {
            return Err(problem(task, tasks::Refusal::Reference));
        };
        if !crate::acts::bounds::proposal_has_recipient(core, holder) {
            return Err(problem(task, tasks::Refusal::Unproposable));
        }
        let Some(number) = crate::fresh(&mut core.counters, crate::Family::Message) else {
            return Err(problem(task, tasks::Refusal::Busy));
        };
        proposals
            .push(tasks::Proposal {
                number,
                proposer: task,
                project: context.project,
                action,
                reason: Box::new([]),
                as_holder: false,
                state: tasks::ProposalState::Pending { holder, since: env.wall },
            })
            .expect("one proposal per step");
    }
    Ok(proposals.into_boxed())
}

#[expect(clippy::result_large_err, reason = "step refusals retain exact owner diagnostics")]
fn cancels(
    core: &mut Core,
    env: &Env<Limits>,
    parts: &[crate::StepCancel],
) -> Result<Box<[tasks::ProcedureCancel]>, StepRefusal> {
    let mut cancels = List::with_capacity(env.limits.tasks.batch);
    for part in parts {
        let message = if part.grace.is_some() {
            let Some(message) = crate::fresh(&mut core.counters, crate::Family::Message) else {
                return Err(problem(part.task, tasks::Refusal::Busy));
            };
            Some(message)
        } else {
            None
        };
        if cancels
            .push(tasks::ProcedureCancel { task: part.task, reason: part.reason.clone(), grace: part.grace, message })
            .is_err()
        {
            return Err(problem(part.task, tasks::Refusal::Batch));
        }
    }
    Ok(cancels.into_boxed())
}
