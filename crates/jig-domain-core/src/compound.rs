//! One party key, with policy and task admission interleaved in operation order.
use crate::{Core, Counters, Event, Family, Limits, PersonTaskRoute, fresh};
use alloc::boxed::Box;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_lib::{Env, List, Queue, ReplyTo, Token};

pub(crate) struct PartyTask {
    pub person: u64,
    pub project: u32,
    pub role: Option<people::Role>,
    pub task: u64,
}
pub(crate) struct Route {
    pub request: Token,
    pub person: u64,
    pub project: u32,
    pub role: Option<people::Role>,
}

impl Route {
    fn task(&self, task: u64) -> PartyTask {
        PartyTask { person: self.person, project: self.project, role: self.role, task }
    }
}

pub(crate) fn route(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    route: Route,
    inputs: List<people::Operation>,
) {
    let context = tasks::CompoundContext {
        by: tasks::Party::Person(route.person),
        project: route.project,
        period: core.period(route.project),
        pool_budget: core.pool_budget(route.project, route.person),
        period_budget: core.period_budget(route.project),
    };
    let mut admission = tasks::CompoundAdmission::new(&core.tasks, env.limits.tasks.tasks);
    let mut operations = List::with_capacity(inputs.len());
    let mut first_message = 0;
    let child_env = Env { now: env.now, wall: env.wall, limits: env.limits.tasks };
    for (at, input) in inputs.into_boxed().into_iter().enumerate() {
        let at = u32::try_from(at).expect("bounded operation count");
        let mut operation = match check(core, env, &route, &admission, input) {
            Ok(operation) => operation,
            Err(why) => return refused(work, route.request, at, people::Outcome::Refused(why)),
        };
        if !projection_room(core, &operations, &operation, env.limits.tasks.tasks) {
            return refused(
                work,
                route.request,
                at,
                people::Outcome::Refused(core.limit_refusal(people::LimitKind::Goals)),
            );
        }
        match issue(&mut core.counters, &mut operation) {
            Ok(message) if first_message == 0 => first_message = message,
            Ok(_) => {}
            Err(why) => return refused(work, route.request, at, people::Outcome::Refused(why)),
        }
        if let Err(problem) = admission.check(&child_env, context, operation.clone()) {
            if let Some(capacity) = problem.capacity {
                core.tasks.remember_capacity(env.wall, capacity);
            }
            let outcome = crate::routing::refusals::person_problem_refusal(core, *problem);
            return refused(work, route.request, at, outcome);
        }
        operations.push(operation).expect("same operation count");
    }
    assert!(
        core.person_tasks.insert(
            route.request,
            PersonTaskRoute::Compound { project: route.project, operations: operations.len(), first_message }
        ) == Ok(None),
        "one compound continuation"
    );
    work.push(Event::Tasks(tasks::Event::Compound {
        reply_to: ReplyTo::new(route.request),
        by: context.by,
        project: context.project,
        operations,
        period: context.period,
        pool_budget: context.pool_budget,
        period_budget: context.period_budget,
    }));
}

pub(crate) fn refused(work: &mut Queue<Event>, request: Token, at: u32, outcome: people::Outcome) {
    let outcome = if let people::Outcome::Refused(why) = outcome {
        people::Outcome::CompoundRefused { at, why }
    } else {
        outcome
    };
    work.push(Event::People(people::Event::Decided { request, outcome }));
}

#[expect(clippy::too_many_lines, reason = "the six operations share their standalone policy and revision judges")]
fn check(
    core: &Core,
    env: &Env<Limits>,
    route: &Route,
    admission: &tasks::CompoundAdmission<'_>,
    operation: people::Operation,
) -> Result<tasks::Operation, people::Refusal> {
    let (task, act) = match &operation {
        people::Operation::Cancel { task, .. } => (*task, crate::acts::Act::Cancel),
        people::Operation::Stop { task, .. } => (*task, crate::acts::Act::Stop),
        people::Operation::Release { task, .. } => (*task, crate::acts::Act::Release),
        people::Operation::Move { task, .. } => (*task, crate::acts::Act::Move),
        people::Operation::Amend { task, .. } => (*task, crate::acts::Act::Amend),
        people::Operation::Message { task, .. } => (*task, crate::acts::Act::Message),
    };
    let row = admission.task(task).ok_or(people::Refusal::Unknown)?;
    if row.project != route.project {
        return Err(people::Refusal::Unknown);
    }
    crate::acts::role(core, route.project, route.role, act)?;
    match operation {
        people::Operation::Cancel { reason, grace, .. } => {
            core.control_admit_in(
                &people::Ask::Cancel { project: route.project, task, reason: Box::new([]), grace },
                route.person,
                route.project,
                route.role,
                &env.limits.tasks,
                admission,
            )?;
            if reason.len() > usize::try_from(env.limits.tasks.result_bytes).expect("u32 fits usize") {
                return Err(people::Refusal::configured(
                    people::LimitKind::Contract,
                    u64::from(env.limits.tasks.result_bytes),
                    people::LimitKey::TasksResultBytes,
                    people::LimitUnit::Bytes,
                ));
            }
            if let tasks::Phase::Ended(ending) = &row.phase {
                return Err(crate::routing::refusals::ended(ending));
            }
            let message = if grace.is_some() { Some(0) } else { None };
            Ok(tasks::Operation::Cancel { task, reason, grace, message })
        }
        people::Operation::Stop { .. } => {
            core.control_admit_in(
                &people::Ask::Stop { project: route.project, task },
                route.person,
                route.project,
                route.role,
                &env.limits.tasks,
                admission,
            )?;
            crate::acts::mutable_phase(&row.phase)?;
            Ok(tasks::Operation::Stop { task, hold: 0 })
        }
        people::Operation::Release { revision, again, .. } => {
            core.control_admit_in(
                &people::Ask::Release { project: route.project, task, revision, again },
                route.person,
                route.project,
                route.role,
                &env.limits.tasks,
                admission,
            )?;
            if let tasks::Phase::Closing(closing) = &row.phase {
                return Err(crate::routing::refusals::ended(&closing.ending));
            }
            let tasks::Phase::Held { .. } = row.phase else { return Err(people::Refusal::NotWaiting) };
            Core::hold_revision_in(task, revision, admission)?;
            let required = uncertain(&row.phase);
            if required != again.is_some() {
                return Err(people::Refusal::Authority { proposable: None, lacking: people::Lack::Again { required } });
            }
            let message = if again == Some(false) { Some(0) } else { None };
            Ok(tasks::Operation::Release { task, again, message })
        }
        people::Operation::Move { reason, .. } => {
            core.move_admit_in(route.task(task), env.limits.tasks.tree_tasks, env.limits.tasks.depth, admission)?;
            Ok(tasks::Operation::Move { task, reason })
        }
        people::Operation::Amend { revision, amendment, release, .. } => {
            crate::acts::bounds::amend_preflight_in(core, &env.limits, route.person, task, route.role, admission)?;
            crate::acts::mutable_phase(&row.phase)?;
            let mut amendment = crate::amendments::translate(*amendment, row.authority.models, &env.limits.tasks)?;
            let stop_run = crate::acts::bounds::amendment_admit_in(
                core,
                route.task(task),
                &amendment,
                env.limits.tasks.depth,
                &env.limits.tasks,
                admission,
            )?;
            crate::acts::mutable_phase(&row.phase)?;
            Core::party_amend_revision_in(task, revision, &amendment, admission)?;
            if let Some(revision) = release {
                Core::hold_revision_in(task, revision, admission)?;
                if uncertain(&row.phase) {
                    return Err(people::Refusal::Authority {
                        proposable: None,
                        lacking: people::Lack::Again { required: true },
                    });
                }
            }
            if release.is_some()
                && let Some(hold) = core.tasks.placed_on(task)
            {
                core.party_hold_admit(route.person, hold)?;
            }
            amendment.release = release;
            Ok(tasks::Operation::Amend { task, message: 0, stop_run, amendment: Box::new(amendment) })
        }
        people::Operation::Message { words, .. } => {
            crate::acts::write_in(core, &env.limits, route.person, task, admission)?;
            Ok(tasks::Operation::Message {
                task,
                word: tasks::Word {
                    number: 0,
                    from: tasks::Party::Person(route.person),
                    kind: tasks::MessageKind::Words,
                    words,
                    at: env.wall,
                    hits: 1,
                    eligible: false,
                },
            })
        }
    }
}

fn issue(counters: &mut Counters, operation: &mut tasks::Operation) -> Result<u64, people::Refusal> {
    let number = fresh(counters, Family::Message)
        .ok_or(people::Refusal::Limit { which: people::LimitKind::Arithmetic, bound: None })?;
    match operation {
        tasks::Operation::Cancel { message: Some(message), .. }
        | tasks::Operation::Release { message: Some(message), .. }
        | tasks::Operation::Amend { message, .. } => *message = number,
        tasks::Operation::Message { word, .. } => word.number = number,
        tasks::Operation::Stop { hold, .. } => *hold = number,
        tasks::Operation::Cancel { message: None, .. }
        | tasks::Operation::Release { message: None, .. }
        | tasks::Operation::Move { .. } => {}
    }
    Ok(number)
}

fn projection_room(core: &Core, prior: &List<tasks::Operation>, operation: &tasks::Operation, bound: u32) -> bool {
    let tasks::Operation::Amend { task, amendment, .. } = operation else { return true };
    if !tracks(amendment) || core.projections.contains_key(task) {
        return true;
    }
    let mut count = core.projections.len();
    for (at, earlier) in prior.iter().enumerate() {
        if let tasks::Operation::Amend { task: number, amendment, .. } = earlier
            && tracks(amendment)
            && !core.projections.contains_key(number)
        {
            if number == task {
                return true;
            }
            let mut seen = false;
            for previous in prior.iter().take(at) {
                if let tasks::Operation::Amend { task: other, amendment, .. } = previous
                    && other == number
                    && tracks(amendment)
                {
                    seen = true;
                }
            }
            if !seen {
                count = count.checked_add(1).expect("bounded operations");
            }
        }
    }
    count < bound
}

fn tracks(amendment: &tasks::Amendment) -> bool {
    match amendment.tracked {
        Some(tasks::Tracking::Tracked { .. }) => true,
        Some(tasks::Tracking::Untracked) | None => false,
    }
}
fn uncertain(phase: &tasks::Phase) -> bool {
    match phase {
        tasks::Phase::Held { why: tasks::Hold::Uncertain { .. }, .. } => true,
        tasks::Phase::Held { .. }
        | tasks::Phase::Waiting
        | tasks::Phase::Active(_)
        | tasks::Phase::Closing(_)
        | tasks::Phase::Ended(_) => false,
    }
}
