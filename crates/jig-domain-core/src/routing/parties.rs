//! Party pools, task creation, adoption, service, policy and role requests
//! routed through authority, people, tasks and connectors
//! (`domain/people.md`, section 5.1). Controls and decisions are separate
//! families so each route file stays under 1,500 lines.

use super::refusals::pool_refused;
use super::{Creation, CreationKind};
use crate::{Ask, Core, Event, Limits, PersonTaskRoute, Record, Request, Requests, Write};
use alloc::boxed::Box;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_lib::{Env, List, Queue, ReplyTo, Token};

pub(super) fn route_holdings(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    request: Token,
    connector: u16,
    holdings: Option<Box<[tasks::Holding]>>,
) -> Requests {
    let mut out = Queue::with_capacity(env.limits.connectors.checked_add(1).expect("connector output bound"));
    created_holdings(core, env, &mut out, work, request, connector, holdings);
    out.push(Request::Decided);
    Requests::Out(out)
}
#[expect(clippy::too_many_arguments, reason = "a keyed pool edit carries its owner, beneficiary and budget")]
pub(super) fn pool_route(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    request: Token,
    owner: u64,
    project: u32,
    person: u64,
    budget: u64,
) {
    if let Err(why) = core.role_allowed(owner, project, env.limits.tasks.tree_tasks) {
        pool_refused(work, request, why);
        return;
    }
    if core.person_tasks.len() >= env.limits.tasks.tasks || work.room() < 2 {
        super::refusals::pool_again(work, request, people::Again::Busy);
        return;
    }
    let Some(role) = core.people.role(person, project) else {
        pool_refused(work, request, people::Refusal::Unknown);
        return;
    };
    let Some(allowed) = core.authority.role(project, role.number()) else {
        pool_refused(work, request, people::Refusal::Unknown);
        return;
    };
    if budget > allowed.period_spend {
        pool_refused(
            work,
            request,
            people::Refusal::Authority {
                proposable: None,
                lacking: people::Lack::Budget { needs: budget, left: allowed.period_spend },
            },
        );
        return;
    }
    let Some(policy) = core.authority.policy(project) else {
        pool_refused(work, request, people::Refusal::Unknown);
        return;
    };
    if core.period_budget(project) > policy.period_spend {
        pool_refused(
            work,
            request,
            people::Refusal::Authority {
                proposable: None,
                lacking: people::Lack::Budget { needs: core.period_budget(project), left: policy.period_spend },
            },
        );
        return;
    }
    let period = tasks::Funder::Period { project, period: core.period(project) };
    let funder = tasks::Funder::Pool { project, person, period: core.period(project) };
    let needed =
        u32::from(core.tasks.funding(period).is_none()).saturating_add(u32::from(core.tasks.funding(funder).is_none()));
    if core.tasks.funding_room() < needed {
        super::refusals::pool_again(work, request, people::Again::Busy);
        return;
    }
    if core.tasks.funding(period).is_none() && budget > core.period_budget(project) {
        pool_refused(
            work,
            request,
            people::Refusal::Authority {
                proposable: None,
                lacking: people::Lack::Budget { needs: budget, left: core.period_budget(project) },
            },
        );
        return;
    }
    if core.tasks.funding(period).is_none() {
        work.push(Event::Tasks(tasks::Event::OpenPeriod {
            reply_to: ReplyTo::new(Token::new(0)),
            project,
            period: core.period(project),
            budget: core.period_budget(project),
        }));
    }
    assert!(
        core.person_tasks.insert(request, PersonTaskRoute::PoolSet { project, person }) == Ok(None),
        "one admitted pool route"
    );
    if core.tasks.funding(funder).is_some() {
        work.push(Event::Tasks(tasks::Event::ResizePool {
            reply_to: ReplyTo::new(request),
            project,
            person,
            period: core.period(project),
            budget,
        }));
    } else {
        work.push(Event::Tasks(tasks::Event::CarvePool {
            reply_to: ReplyTo::new(request),
            project,
            person,
            period: core.period(project),
            budget,
        }));
    }
}

pub(super) fn begin_creation(
    core: &mut Core,
    env: &Env<Limits>,
    out: &mut Queue<Request>,
    work: &mut Queue<Event>,
    request: Token,
    kind: CreationKind,
    members: Box<[tasks::New]>,
) {
    if core.creating.len() == core.creating.capacity() {
        super::refusals::pool_again(work, request, people::Again::Busy);
        return;
    }
    let remaining = u32::try_from(core.connectors.len()).expect("configured connector count fits u32");
    assert!(remaining > 0, "task creation has a numbered connector");
    creation_holdings(core, out, request, members.first().expect("nonempty creation"));
    let inserted = core.creating.insert(
        request,
        Creation {
            kind,
            members,
            at: 0,
            remaining,
            holdings: List::with_capacity(env.limits.tasks.holdings),
            answered: List::with_capacity(remaining),
            failed: false,
            failed_bound: None,
        },
    );
    match inserted {
        Ok(None) => {}
        Ok(Some(_)) | Err(_) => unreachable!("one keyed task creation"),
    }
}

pub(super) fn created_holdings(
    core: &mut Core,
    env: &Env<Limits>,
    out: &mut Queue<Request>,
    work: &mut Queue<Event>,
    request: Token,
    connector: u16,
    holdings: Option<Box<[tasks::Holding]>>,
) {
    let Some(creating) = core.creating.get_mut(&request) else { return };
    assert!(creating.remaining > 0, "one holdings answer per connector");
    assert!(
        core.connectors.contains(&connector) && !creating.answered.as_slice().contains(&connector),
        "one answer from each numbered connector"
    );
    creating.answered.push(connector).expect("one answer per configured connector");
    creating.remaining = creating.remaining.checked_sub(1).expect("one holdings answer per connector");
    match holdings {
        Some(holdings) => {
            for holding in holdings {
                if creating.holdings.push(holding).is_err() {
                    creating.failed = true;
                    creating.failed_bound = Some(people::LimitBound::configured(
                        u64::from(env.limits.tasks.holdings),
                        people::LimitKey::TasksHoldings,
                        people::LimitUnit::Items,
                    ));
                }
            }
        }
        None => creating.failed = true,
    }
    if creating.remaining != 0 {
        return;
    }
    let member = creating.members.get_mut(creating.at).expect("creation member");
    member.holdings =
        core::mem::replace(&mut creating.holdings, List::with_capacity(env.limits.tasks.holdings)).into_boxed();
    creating.at = creating.at.checked_add(1).expect("bounded batch");
    if let Some(member) = creating.members.get(creating.at) {
        creating.remaining = u32::try_from(core.connectors.len()).expect("connector count");
        creating.answered.clear();
        let member = member.clone();
        creation_holdings(core, out, request, &member);
        return;
    }
    let creating = core.creating.remove(&request).expect("completed creation flight");
    if creating.failed {
        pool_refused(
            work,
            request,
            people::Refusal::Limit { which: people::LimitKind::Holds, bound: creating.failed_bound },
        );
        return;
    }
    finish_creation(core, env, work, request, creating);
}

fn finish_creation(core: &mut Core, env: &Env<Limits>, work: &mut Queue<Event>, request: Token, creating: Creation) {
    match creating.kind {
        CreationKind::Propose { person, reason, to, stubs } => {
            super::propose::keep(
                core,
                env,
                work,
                super::propose::Proposal { request, person, reason, to, batch: creating.members, stubs },
            );
            return;
        }
        CreationKind::Batch { .. } => {}
    }
    let CreationKind::Batch { person, .. } = creating.kind else { unreachable!("proposal handled above") };
    {
        let first = creating.members.first().expect("nonempty batch").number;
        let (period, pool) = core.open_person_pool(creating.members.first().expect("nonempty batch").project, person);
        if let Some(event) = period {
            work.push(Event::Tasks(event));
        }
        if let Some(event) = pool {
            work.push(Event::Tasks(event));
        }
        let CreationKind::Batch { given, stubs, .. } = creating.kind else { unreachable!("batch creation") };
        assert!(core.made.insert(request, super::Made { first, given, stubs }).is_ok(), "one batch make flight");
        work.push(Event::Tasks(tasks::Event::Make {
            reply_to: ReplyTo::new(request),
            creator: tasks::Party::Person(person),
            batch: creating.members,
        }));
    }
}

#[expect(clippy::too_many_arguments, reason = "one keyed role edit carries the candidate roster and its owner")]
pub(super) fn roles_route(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    out: &mut Queue<Request>,
    request: Token,
    person: u64,
    project: u32,
    holdings: Box<[people::Holding]>,
) {
    if let Err(why) = core.role_allowed(person, project, env.limits.tasks.tree_tasks) {
        pool_refused(work, request, why);
        return;
    }
    for holding in &holdings {
        match holding.role {
            people::Role::Policy { role } => {
                if core.authority.role(project, role).is_none() {
                    pool_refused(work, request, people::Refusal::Unknown);
                    return;
                }
            }
            people::Role::Owner | people::Role::Maintainer | people::Role::Member | people::Role::Observer => {}
        }
    }
    let contexts = match core.tasks.project_escalations(&env.limits.tasks, project) {
        Ok(contexts) => contexts,
        Err(refusal) => {
            assert!(refusal == tasks::Refusal::NotReady, "project inspection refusal is readiness");
            super::refusals::pool_again(work, request, people::Again::NotReady);
            return;
        }
    };
    let changed = match core.roles_preflight(&env.limits, &contexts, &holdings) {
        Ok(changed) => changed,
        Err(why) => {
            pool_refused(work, request, why);
            return;
        }
    };
    let handoffs = u32::try_from(contexts.len()).expect("bounded Waiting count").checked_add(1).expect("recheck room");
    drop(contexts);
    drop(holdings);
    if work.room() < handoffs.checked_add(env.limits.tasks.tasks).expect("validated recheck bound")
        || out.room() < changed.checked_add(2).expect("validated role cohort bound")
    {
        super::refusals::pool_again(work, request, people::Again::Busy);
        return;
    }
    if let Err(why) = core.role_allowed(person, project, env.limits.tasks.tree_tasks) {
        pool_refused(work, request, why);
        return;
    }
    core.touch_accounts(None);
    core.touch_deployment();
    core.touch_project_parties(project);
    let mut applied = Queue::with_capacity(people::max_out(&env.limits.people));
    people::step(
        &mut core.people,
        &Env { now: env.now, wall: env.wall, limits: env.limits.people },
        people::Event::ApplyRoles { reply_to: ReplyTo::new(request), request },
        &mut applied,
    );
    let mut terminal = None;
    for _ in 0..applied.len() {
        match applied.pop().expect("role application output count") {
            people::Request::Save { record } => {
                super::view_routes::touch_people_row(core, &record);
                out.push(Request::Write(Write::Save(Record::People(record))));
            }
            people::Request::RolesApplied { reply_to, request: actual, result } => {
                assert!(
                    reply_to.into_token() == request && actual == request && terminal.is_none(),
                    "one role terminal"
                );
                terminal = Some(result);
            }
            people::Request::Erase { .. }
            | people::Request::Act { .. }
            | people::Request::Acted { .. }
            | people::Request::Route { .. }
            | people::Request::Reply { .. }
            | people::Request::RolesRefused { .. }
            | people::Request::ServiceMade { .. }
            | people::Request::RestoreRefused { .. } => unreachable!("role application has one save and terminal"),
        }
    }
    if let Err(why) = terminal.expect("role application terminal") {
        work.push(Event::People(people::Event::Decided { request, outcome: why }));
        return;
    }
    work.push(Event::Tasks(tasks::Event::RecheckEscalations { reply_to: ReplyTo::new(request), project }));
    let mut reroutes = core.proposal_recheck_project(&env.limits, project, env.wall);
    for _ in 0..reroutes.len() {
        work.push(Event::Tasks(reroutes.pop().expect("bounded proposal recheck")));
    }
    work.push(Event::People(people::Event::Decided { request, outcome: people::Outcome::RolesSet { project } }));
}

pub(super) fn make_service_request(
    core: &mut Core,
    env: &Env<Limits>,
    request: Token,
    person: u64,
    project: u32,
    pending: &mut Queue<people::Request>,
    service_role: people::Role,
) {
    let outcome = match core.role_allowed(person, project, env.limits.tasks.tree_tasks) {
        Ok(()) if core.authority.role(project, service_role.number()).is_some() => {
            match crate::fresh(&mut core.counters, crate::Family::Person) {
                Some(candidate) => {
                    let mut follow = Queue::with_capacity(people::max_out(&env.limits.people));
                    people::step(
                        &mut core.people,
                        &Env { now: env.now, wall: env.wall, limits: env.limits.people },
                        people::Event::MakeService { request, person: candidate },
                        &mut follow,
                    );
                    for _ in 0..follow.len() {
                        pending.push(follow.pop().expect("service output count"));
                    }
                    None
                }
                None => Some(people::Outcome::Refused(core.limit_refusal(people::LimitKind::Arithmetic))),
            }
        }
        Ok(()) => Some(people::Outcome::Refused(people::Refusal::Unknown)),
        Err(refusal) => Some(people::Outcome::Refused(refusal)),
    };
    if let Some(outcome) = outcome {
        let mut follow = Queue::with_capacity(people::max_out(&env.limits.people));
        people::step(
            &mut core.people,
            &Env { now: env.now, wall: env.wall, limits: env.limits.people },
            people::Event::Decided { request, outcome },
            &mut follow,
        );
        for _ in 0..follow.len() {
            pending.push(follow.pop().expect("service decision count"));
        }
    }
}

#[expect(clippy::too_many_arguments, reason = "the route retains the admitted party and request fields")]
pub(super) fn adopt_request(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    request: Token,
    person: u64,
    project: u32,
    out: &mut Queue<Request>,
    adoption: people::Adoption,
) {
    match core.role_allowed(person, project, env.limits.tasks.tree_tasks) {
        Ok(()) => {
            out.push(Request::Ask {
                connector: adoption.resource.connector,
                ask: Ask::Adopt { request, project, adoption },
            });
        }
        Err(why) => {
            work.push(Event::People(people::Event::Decided { request, outcome: people::Outcome::Refused(why) }));
        }
    }
}

#[expect(clippy::too_many_arguments, reason = "the route retains the admitted party and request fields")]
pub(super) fn change_policy_request(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    request: Token,
    person: u64,
    project: u32,
    out: &mut Queue<Request>,
    change: people::PolicyChange,
) {
    if work.room()
        < env
            .limits
            .tasks
            .tasks
            .checked_add(env.limits.people.holdings)
            .expect("task recheck and pool resize bound")
            .checked_add(2)
            .expect("period resize and decision bound")
        || out.room() < 2
    {
        super::refusals::pool_again(work, request, people::Again::Busy);
        return;
    }
    let period = core.period(project);
    match core.change_policy(&env.limits, person, project, change) {
        Ok((snapshot, resize)) => {
            core.touch_project_parties(project);
            let name = core.names.get(&project).expect("project policy has a name").clone();
            out.push(Request::Write(Write::Save(Record::People(people::Stored::Policy {
                project,
                name,
                value: snapshot,
            }))));
            if let Some(budget) = resize.period {
                work.push(Event::Tasks(tasks::Event::ResizePeriod {
                    reply_to: ReplyTo::new(Token::new(u64::MAX - 2)),
                    project,
                    period,
                    budget,
                }));
            }
            for cuts in [true, false] {
                for pool in &resize.pools {
                    if (pool.to < pool.from) == cuts {
                        work.push(Event::Tasks(tasks::Event::ResizePool {
                            reply_to: ReplyTo::new(Token::new(u64::MAX - 2)),
                            project,
                            person: pool.person,
                            period,
                            budget: pool.to,
                        }));
                    }
                }
            }
            let mut reroutes = core.proposal_recheck_project(&env.limits, project, env.wall);
            for _ in 0..reroutes.len() {
                work.push(Event::Tasks(reroutes.pop().expect("policy holder recheck")));
            }
            work.push(Event::People(people::Event::Decided {
                request,
                outcome: people::Outcome::PolicyChanged { project },
            }));
        }
        Err(why) => pool_refused(work, request, why),
    }
}

fn creation_holdings(core: &Core, out: &mut Queue<Request>, request: Token, new: &tasks::New) {
    for &connector in core.connectors.as_ref() {
        out.push(Request::Ask {
            connector,
            ask: Ask::TaskHoldings {
                request,
                project: new.project,
                root: new.number,
                number: new.number,
                executor: new.executor,
                spec: new.spec.clone(),
            },
        });
    }
}
