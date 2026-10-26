//! An amendment waits without deciding, then repeats admission on current state
//! (`domain/engine.md`, 5.3; `domain/tasks.md`, section 7).
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_lib::{Env, List, Queue, ReplyTo, Token};

use crate::{CallKey, CallPart, Core, Event, Limits, Request, Requests};

#[derive(Debug)]
pub(crate) enum Replay {
    Agent {
        token: Token,
        key: CallKey,
        target: u64,
        revision: u64,
        amendment: tasks::Amendment,
    },
    Person {
        request: Token,
        person: u64,
        project: u32,
        task: u64,
        revision: u64,
        amendment: people::Amendment,
        release: Option<u64>,
    },
}

#[derive(Debug)]
pub(crate) struct Evidence {
    pub loaded: List<u64>,
    pub places: List<tasks::DependencyPlace>,
}
impl Evidence {
    pub fn new(limits: &tasks::Limits) -> Self {
        let capacity = limits
            .dependencies
            .checked_mul(limits.depth.checked_add(1).expect("bounded depth"))
            .expect("bounded amendment places");
        Self { loaded: List::with_capacity(capacity), places: List::with_capacity(capacity) }
    }
    #[expect(clippy::manual_find, reason = "step code uses explicit bounded scans without closures")]
    fn place(&self, number: u64) -> Option<&tasks::DependencyPlace> {
        for row in &self.places {
            if row.number == number {
                return Some(row);
            }
        }
        None
    }
}
#[derive(Debug)]
pub(crate) struct AmendmentRead {
    pub replay: Replay,
    pub evidence: Evidence,
}

/// The next missing place, recomputed on every replay. Missing store rows count
/// as loaded, so tasks can refuse them by the dependency's number.
pub(crate) fn missing(
    core: &Core,
    limits: &tasks::Limits,
    by: tasks::Party,
    target: u64,
    dependencies: Option<&[u64]>,
    evidence: &Evidence,
) -> Option<u64> {
    let old = core.tasks.task(target)?;
    let dependencies = dependencies?;
    if dependencies.len() > usize::try_from(limits.dependencies).expect("u32 fits usize") {
        return None;
    }
    for &dependency in dependencies {
        if old.dependencies.contains(&dependency) || core.tasks.task(dependency).is_some() {
            continue;
        }
        let mut next = dependency;
        for _ in 0..=limits.depth {
            if !evidence.loaded.as_slice().contains(&next) {
                return Some(next);
            }
            let Some(row) = evidence.place(next) else { break };
            match by {
                tasks::Party::Task(amender) => {
                    let Some(holder) = core.tasks.task(amender) else { break };
                    if row.root != holder.root || row.depth <= holder.depth {
                        break;
                    }
                    match row.requester {
                        tasks::Party::Task(parent) => {
                            if core.tasks.task(parent).is_some()
                                || parent == amender
                                || row.depth.saturating_sub(1) <= holder.depth
                            {
                                break;
                            }
                            next = parent;
                        }
                        tasks::Party::Person(_) | tasks::Party::Deployment { .. } => break,
                    }
                }
                tasks::Party::Person(person) => {
                    let project_reach = match core.people.role(person, old.project) {
                        Some(role) => core.reaches_project(role, old.project),
                        None => false,
                    };
                    if project_reach
                        || row.requester == by
                        || row.root == row.number
                        || core.tasks.task(row.root).is_some()
                    {
                        break;
                    }
                    next = row.root;
                }
                tasks::Party::Deployment { .. } => break,
            }
        }
    }
    None
}

pub(super) fn waiting(core: &Core, key: CallKey) -> bool {
    for (_, read) in &core.result_reads {
        match &read.why {
            super::result_reads::ReadFor::ProjectionNews { .. }
            | super::result_reads::ReadFor::DeploymentClose { .. }
            | super::result_reads::ReadFor::Preparation { .. } => {}
            super::result_reads::ReadFor::Amendment(read) => match &read.replay {
                Replay::Agent { key: pending, .. } => {
                    if *pending == key {
                        return true;
                    }
                }
                Replay::Person { .. } => {}
            },
        }
    }
    false
}

pub(super) fn current(core: &Core, replay: &Replay) -> bool {
    match replay {
        Replay::Agent { key, .. } => {
            core.current_proof(key.task, key.attempt)
                && core.pending_calls.contains_key(key)
                && !core.call_parts.contains_key(key)
        }
        // Owner departure clears the read without deciding the keyed request.
        Replay::Person { person, .. } => core.people.identity(*person).is_some(),
    }
}

pub(super) fn busy(core: &mut Core, work: &mut Queue<Event>, out: &mut Queue<Request>, replay: Replay) {
    match replay {
        Replay::Agent { token, key, target, .. } => super::runs::call_decision(
            core,
            out,
            ReplyTo::new(token),
            key,
            CallPart::ControlRefused(tasks::Problem::new(Some(target), tasks::Refusal::Busy)),
        ),
        Replay::Person { request, .. } => super::refusals::pool_again(work, request, people::Again::Busy),
    }
}

pub(super) fn resume(core: &mut Core, env: &Env<Limits>, work: &mut Queue<Event>, read: AmendmentRead) -> Requests {
    match read.replay {
        Replay::Agent { token, key, target, revision, amendment } => {
            super::runs::named_amend(core, env, work, token, key, target, revision, amendment, read.evidence)
        }
        Replay::Person { request, person, project, task, revision, amendment, release } => {
            let mut out = Queue::with_capacity(3);
            let role = core.people.role(person, project);
            super::party_controls::amend_route(
                core,
                env,
                work,
                &mut out,
                request,
                person,
                role,
                project,
                task,
                revision,
                amendment,
                release,
                read.evidence,
            );
            out.push(Request::Decided);
            Requests::Out(out)
        }
    }
}
