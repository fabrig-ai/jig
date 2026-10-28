//! Configuration's first-start project seed and the deployment's roles
//! (protocol/configuration.md, 3 and 8; domain/people.md, 3).

use alloc::boxed::Box;
use jig_domain_authority as authority;
use jig_domain_people as people;
use skein_lib::{Env, List, Map, Queue};

use crate::{Core, Event, Family, Limits, Record, Request, Requests, Write, fresh};

pub(super) fn seed(core: &mut Core, env: &Env<Limits>, work: &mut Queue<Event>) -> Requests {
    seed_projects(core);
    let owners = core::mem::replace(&mut core.seed_owners, Box::new([]));
    let mut identities = Map::with_capacity(env.limits.people.initial_owners);
    let mut out = Queue::uncapped();
    if !crate::settings::restart::first_start(core, env, &mut out) {
        return Requests::Out(out);
    }
    let mut holdings = List::with_capacity(env.limits.people.holdings);
    for owner in &owners {
        if identities.get(&owner.identity).is_some() {
            continue;
        }
        let person = fresh(&mut core.counters, Family::Person).expect("first-start person space");
        let record = people::Stored::Person {
            number: person,
            identity: people::Identity { key: owner.identity.clone(), login: Box::new([]), name: Box::new([]) },
        };
        let mut child = Queue::uncapped();
        people::step(
            &mut core.people,
            &Env { now: env.now, wall: env.wall, limits: env.limits.people },
            people::Event::Restore { record: record.clone() },
            &mut child,
        );
        assert!(child.is_empty(), "configured owner restores within people limits");
        identities.insert(owner.identity.clone(), person).expect("bounded configured identities");
        out.push(Request::Write(Write::Save(Record::People(record))));
        holdings.push(people::Holding { person, role: people::Role::Owner }).expect("configured owner roster");
    }
    let mut child = Queue::uncapped();
    people::step(
        &mut core.people,
        &Env { now: env.now, wall: env.wall, limits: env.limits.people },
        people::Event::Roles { project: people::DEPLOYMENT, holdings: holdings.into_boxed() },
        &mut child,
    );
    for _ in 0..child.len() {
        let people::Request::Save { record } = child.pop().expect("seeded deployment role row") else {
            unreachable!("configured deployment roster fits people limits");
        };
        out.push(Request::Write(Write::Save(Record::People(record))));
    }
    for &project in &core.projects {
        core.counters.deployment.projects = core.counters.deployment.projects.max(u64::from(project));
        let policy = core.authority.policy(project).expect("configured project policy");
        let permissions = match core.permission_roles.get(&project) {
            Some(roles) => roles.as_ref(),
            None => &[],
        };
        let templates = match core.template_overrides.get(&project) {
            Some(entries) => entries.as_ref(),
            None => &[],
        };
        let name = core.names.get(&project).expect("project policy has a name").clone();
        let value = crate::policy::snapshot(policy, permissions, templates).expect("configured policy snapshot");
        out.push(Request::Write(Write::Save(Record::People(people::Stored::Policy { project, name, value }))));
        let mut holdings = List::with_capacity(env.limits.people.holdings);
        for owner in &owners {
            if owner.project != project {
                continue;
            }
            let person = *identities.get(&owner.identity).expect("configured owner person seeded");
            holdings.push(people::Holding { person, role: people::Role::Owner }).expect("configured owner roster");
        }
        let mut child = Queue::uncapped();
        people::step(
            &mut core.people,
            &Env { now: env.now, wall: env.wall, limits: env.limits.people },
            people::Event::Roles { project, holdings: holdings.into_boxed() },
            &mut child,
        );
        for _ in 0..child.len() {
            let people::Request::Save { record } = child.pop().expect("seeded role row") else {
                unreachable!("configured roster fits people limits");
            };
            out.push(Request::Write(Write::Save(Record::People(record))));
        }
    }
    let mut child = Queue::uncapped();
    people::step(
        &mut core.people,
        &Env { now: env.now, wall: env.wall, limits: env.limits.people },
        people::Event::Restored,
        &mut child,
    );
    let Requests::Out(mut restored) = super::people_routes::tag_people(core, env, child, work);
    for _ in 0..restored.len() {
        out.push(restored.pop().expect("people restored requests"));
    }
    Requests::Out(out)
}

pub(crate) fn seed_projects(core: &mut Core) {
    let projects = core::mem::replace(&mut core.seed_projects, Box::new([]));
    for project in projects {
        let mut facts = Queue::uncapped();
        authority::step(
            &mut core.authority,
            authority::Event::Policy { project: project.number, policy: project.policy.policy },
            &mut facts,
        );
        assert!(
            facts.pop() == Some(authority::PolicyFact::Added { project: project.number }),
            "checked seed policy admission"
        );
        core.projects.push(project.number).expect("bounded seed projects");
        core.names.insert(project.number, project.name).expect("bounded seed names");
        core.permission_roles.insert(project.number, project.policy.permissions).expect("bounded seed permissions");
        core.template_overrides.insert(project.number, project.policy.templates).expect("bounded seed templates");
    }
}
