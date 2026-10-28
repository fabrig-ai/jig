//! Deployment acts retain the same child decisions as party requests (people.md, 5.3).
use crate::{Ask, Carried, Core, Event, Held, Limits, PersonTaskRoute, Request, Requests};
use alloc::boxed::Box;
use jig_domain_accounts as accounts;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_lib::{Env, List, Queue, ReplyTo, Token};

#[expect(clippy::too_many_arguments, reason = "the connector's keyed envelope crosses admission unchanged")]
pub(super) fn start(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    reply_to: ReplyTo,
    connector: u16,
    key: [u8; 16],
    act: Box<people::Act>,
    carried: Option<Carried>,
) -> Requests {
    let digest = match &*act {
        people::Act::AddKey { digest, .. } => Some(*digest),
        people::Act::MakeProject { .. }
        | people::Act::Close { .. }
        | people::Act::RemoveAccount { .. }
        | people::Act::Adopt { .. } => None,
    };
    let valid = match carried {
        Some(carried) => digest == Some(carried.digest),
        None => digest.is_none(),
    };
    if !valid {
        let mut out = Queue::with_capacity(2);
        out.push(Request::Held(Box::new(Held::Acted {
            to: reply_to,
            outcome: people::Outcome::Refused(people::Refusal::NotOffered),
        })));
        out.push(Request::Decided);
        return super::application_routes::first(super::application_routes::number(carried), Requests::Out(out));
    }
    assert!(core.key_input.is_none(), "one key admission at a time");
    core.key_input = carried;
    let routed =
        super::people_routes::route_people(core, env, work, people::Event::Act { reply_to, connector, key, act });
    let unused = super::application_routes::number(core.key_input.take());
    super::application_routes::first(unused, routed)
}

pub(super) fn route(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    out: &mut Queue<Request>,
    request: Token,
    act: people::Act,
) {
    match act {
        people::Act::MakeProject { maker, name, policy } => {
            super::projects::make_project_act(core, env, work, out, request, maker, name, policy);
        }
        people::Act::Close { project, task, grace, reason } => {
            if core.tasks.task(task).is_none() {
                return super::result_reads::closed_subject(core, work, out, request, project, task);
            }
            if let Err(why) = close(core, work, request, project, task, grace, reason) {
                refused(work, request, why);
            }
        }
        people::Act::AddKey { provider, join, digest } => {
            account(core, work, request, people::AccountChange::AddKey { provider, join, digest });
        }
        people::Act::RemoveAccount { account: number } => {
            account(core, work, request, people::AccountChange::Remove { account: number });
        }
        people::Act::Adopt { project, adoption } => {
            if !core.projects.as_slice().contains(&project) {
                return refused(work, request, people::Refusal::Unknown);
            }
            out.push(Request::Ask {
                connector: adoption.resource.connector,
                ask: Ask::Adopt { request, project, adoption },
            });
        }
    }
}
fn account(core: &mut Core, work: &mut Queue<Event>, request: Token, change: people::AccountChange) {
    let holder = accounts::Holder::Deployment;
    match super::key_routes::admitted(core, request, holder, None, change) {
        Ok(change) => work.push(Event::Account(accounts::Event::Change { request, holder, change })),
        Err(why) => refused(work, request, why),
    }
}
fn refused(work: &mut Queue<Event>, request: Token, why: people::Refusal) {
    work.push(Event::People(people::Event::Decided { request, outcome: people::Outcome::Refused(why) }));
}
fn close(
    core: &mut Core,
    work: &mut Queue<Event>,
    request: Token,
    project: u32,
    task: u64,
    grace: Option<skein_lib::Duration>,
    reason: Box<[u8]>,
) -> Result<(), people::Refusal> {
    let row = core.tasks.task(task).ok_or(people::Refusal::Unknown)?;
    if row.project != project {
        return Err(people::Refusal::Unknown);
    }
    match &row.phase {
        tasks::Phase::Ended(ending) => return Err(super::refusals::ended(ending)),
        tasks::Phase::Closing(closing) => return Err(super::refusals::ended(&closing.ending)),
        tasks::Phase::Waiting | tasks::Phase::Active(_) | tasks::Phase::Held { .. } => {}
    }
    if !row.flags.conversation {
        return Err(people::Refusal::NotOffered);
    }
    match row.requester {
        tasks::Party::Person(_) => {}
        tasks::Party::Task(_) | tasks::Party::Deployment { .. } => return Err(people::Refusal::NotOffered),
    }
    if core.person_tasks.len() == core.person_tasks.capacity() {
        return Err(people::Refusal::configured(
            people::LimitKind::LiveTasks,
            u64::from(core.person_tasks.capacity()),
            people::LimitKey::PeoplePending,
            people::LimitUnit::Items,
        ));
    }
    let mut operations = List::with_capacity(
        u32::try_from(row.delegates.len()).expect("bounded delegates").checked_add(1).expect("bounded delegates"),
    );
    for delegate in &row.delegates {
        operations
            .push(tasks::Operation::Cancel { task: *delegate, grace: None, message: None, reason: reason.clone() })
            .expect("one per delegate");
    }
    operations
        .push(tasks::Operation::Cancel { task, grace, message: None, reason })
        .expect("conversation after delegates");
    let mut first_message = 0;
    for at in 0..operations.len() {
        let operation = operations.get_mut(at).expect("bounded operation position");
        let number = crate::fresh(&mut core.counters, crate::Family::Message)
            .ok_or(core.limit_refusal(people::LimitKind::Arithmetic))?;
        if first_message == 0 {
            first_message = number;
        }
        if let tasks::Operation::Cancel { grace: Some(_), message, .. } = operation {
            *message = Some(number);
        }
    }
    assert!(
        core.person_tasks
            .insert(request, PersonTaskRoute::Compound { project, operations: operations.len(), first_message })
            == Ok(None),
        "one compound continuation"
    );
    work.push(Event::Tasks(tasks::Event::Compound {
        reply_to: ReplyTo::new(request),
        by: tasks::Party::Deployment { project },
        project,
        operations,
        period: core.period(project),
        pool_budget: 0,
        period_budget: core.period_budget(project),
    }));
    Ok(())
}
