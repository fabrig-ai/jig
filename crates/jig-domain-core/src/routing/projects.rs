//! One project decision includes its roster, policy, first period and answer
//! (people.md, 5.1–5.3; core.md, 6.1; contract.md, 3.2).
use crate::{Core, Event, Limits, Record, Request, Write};
use alloc::boxed::Box;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_lib::{Env, Queue, ReplyTo, Token};

#[expect(clippy::too_many_arguments, reason = "the keyed party request retains its routing envelope")]
pub(super) fn rename_request(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    out: &mut Queue<Request>,
    request: Token,
    person: u64,
    project: u32,
    name: Box<str>,
) {
    match core.rename(&env.limits, person, project, name) {
        Ok(()) => {
            let policy = core.authority.policy(project).expect("renamed project policy");
            let permissions = core.permission_roles.get(&project).expect("project permission mappings");
            let templates = core.template_overrides.get(&project).expect("project template overrides");
            let value = crate::policy::snapshot(policy, permissions, templates).expect("checked policy snapshot");
            let name = core.names.get(&project).expect("renamed project name").clone();
            out.push(Request::Write(Write::Save(Record::People(people::Stored::Policy { project, name, value }))));
            core.touch_project_parties(project);
            decided(work, request, people::Outcome::Renamed { project });
        }
        Err(why) => decided(work, request, people::Outcome::Refused(why)),
    }
}

#[expect(clippy::too_many_arguments, reason = "the keyed party request retains its routing envelope")]
pub(super) fn make_project_request(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    out: &mut Queue<Request>,
    request: Token,
    maker: u64,
    name: Box<str>,
    policy: u32,
) {
    if core.tasks.funding_room() < 1 {
        return super::refusals::pool_again(work, request, people::Again::Busy);
    }
    match core.make_project(&env.limits, maker, name, policy) {
        Ok(made) => {
            let project = made.project;
            let budget = made.policy.period_spend;
            out.push(Request::Write(Write::Save(Record::People(people::Stored::Policy {
                project,
                name: made.name,
                value: made.policy,
            }))));
            work.push(Event::People(people::Event::Roles {
                project,
                holdings: Box::new([people::Holding { person: maker, role: people::Role::Owner }]),
            }));
            work.push(Event::Tasks(tasks::Event::OpenPeriod {
                reply_to: ReplyTo::new(Token::new(u64::MAX - 2)),
                project,
                period: 1,
                budget,
            }));
            work.push(Event::Tasks(tasks::Event::PeriodNext { project, period: 1, next: None }));
            core.touch_party(maker);
            out.push(Request::ProjectMade { project });
            decided(work, request, people::Outcome::ProjectMade { project });
        }
        Err(why) => decided(work, request, people::Outcome::Refused(why)),
    }
}

#[expect(clippy::too_many_arguments, reason = "the deployment act names the maker without a sign-in")]
pub(super) fn make_project_act(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    out: &mut Queue<Request>,
    request: Token,
    maker: u64,
    name: Box<str>,
    policy: u32,
) {
    if core.people.role(maker, people::DEPLOYMENT) != Some(people::Role::Owner) {
        return decided(work, request, people::Outcome::Refused(people::Refusal::Unknown));
    }
    make_project_request(core, env, work, out, request, maker, name, policy);
}

fn decided(work: &mut Queue<Event>, request: Token, outcome: people::Outcome) {
    work.push(Event::People(people::Event::Decided { request, outcome }));
}
