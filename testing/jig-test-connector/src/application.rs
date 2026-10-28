//! Requests about this connector's own objects (domain/connectors.md, 5.5).

use crate::{Domain, Limits, Record, RecordKey, Request, application_core};
use skein_lib::{Duration, Env, Map, Queue, Token, Wall};

/// An optional jig release made on the admitted party's behalf.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ApplicationRelease {
    pub task: u64,
    pub revision: u64,
    /// Explicit repeat/skip choice for an unrecoverable uncertain effect.
    pub again: Option<bool>,
}

/// A numbered configured object, with the connector's own observed revision.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ApplicationRequest {
    pub object: u64,
    pub revision: u64,
    pub value: u64,
    pub release: Option<ApplicationRelease>,
}

/// The connector's current object; initially revision zero and value zero.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ApplicationObject {
    pub revision: u64,
    pub value: u64,
}

/// The application's own terminal, never read by the core.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ApplicationOutcome {
    Changed { object: u64, now: ApplicationObject },
    Moved { object: u64, now: ApplicationObject },
    Role { held: u32 },
    Unknown,
    Exhausted,
    CoreRefused { why: application_core::Refusal },
}

/// Facts translated by the root from the party's jig release.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ApplicationCoreAnswer {
    Released,
    Refused(application_core::Refusal),
    Again(application_core::Again),
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Pending {
    request: ApplicationRequest,
    token: Token,
    retention: Duration,
}

/// Staged values, current objects and durable outcomes have separate bounded owners.
#[derive(Debug)]
pub(crate) struct State {
    prepared: Map<Token, ApplicationRequest>,
    pending: Map<Token, Pending>,
    pub(crate) objects: Map<u64, ApplicationObject>,
    pub(crate) answers: Map<Token, (Wall, ApplicationOutcome)>,
}

impl State {
    pub(crate) fn object(&self, number: u64) -> ApplicationObject {
        self.objects.get(&number).copied().unwrap_or(ApplicationObject { revision: 0, value: 0 })
    }
    pub(crate) fn new(limits: &Limits) -> Self {
        Self {
            prepared: Map::with_capacity(limits.staged),
            pending: Map::with_capacity(limits.staged),
            objects: Map::with_capacity(limits.resources),
            answers: Map::with_capacity(limits.values),
        }
    }
    pub(crate) fn prepare(&mut self, owner: Token, value: ApplicationRequest) {
        let _prepared = self.prepared.insert(owner, value);
    }
    pub(crate) fn drop(&mut self, owner: Token) {
        self.prepared.remove(&owner);
    }
    pub(crate) fn restore_object(&mut self, object: u64, now: ApplicationObject) {
        assert!(self.objects.insert(object, now).is_ok(), "restored application object fits");
    }
    pub(crate) fn restore_answer(&mut self, token: Token, until: Wall, outcome: ApplicationOutcome) {
        assert!(
            token.raw() != 0 && self.answers.insert(token, (until, outcome)).is_ok(),
            "restored application outcome fits"
        );
    }
}

impl Domain {
    /// Assemble the kept value during the root's admitted decision, before its held reply.
    #[must_use]
    #[expect(clippy::manual_map, reason = "step code uses explicit matching instead of closures")]
    pub fn application_outcome(&self, token: Token) -> Option<ApplicationOutcome> {
        match self.application.answers.get(&token) {
            Some((_, outcome)) => Some(*outcome),
            None => None,
        }
    }
}

#[expect(clippy::too_many_arguments, reason = "the root translates the admission envelope unchanged")]
pub(crate) fn decide(
    domain: &mut Domain,
    env: &Env<Limits>,
    request: Token,
    token: Token,
    owner: Token,
    _person: u64,
    scope: crate::ClientScope,
    role: Option<u32>,
    retention: Duration,
    out: &mut Queue<Request>,
) {
    let Some(value) = domain.application.prepared.remove(&owner) else { return busy(request, out) };
    // Expire at most one old answer: this step's output remains bounded independently of retention.
    let mut expired = None;
    for (key, (until, _)) in &domain.application.answers {
        if *until <= env.wall {
            expired = Some(*key);
            break;
        }
    }
    if let Some(key) = expired {
        domain.application.answers.remove(&key);
        out.push(Request::Erase { key: RecordKey::ApplicationAnswer(key) });
    }
    if domain.application.answers.len().saturating_add(domain.application.pending.len()) >= env.limits.values
        || domain.application.pending.len() >= env.limits.staged
    {
        return busy(request, out);
    }
    let pending = Pending { request: value, token, retention };
    if let Some(role) = role
        && role > 2
    {
        return finish(domain, env, request, pending, ApplicationOutcome::Role { held: role }, out);
    }
    if domain.specification_resource(value.object).is_none() {
        return finish(domain, env, request, pending, ApplicationOutcome::Unknown, out);
    }
    let now =
        domain.application.objects.get(&value.object).copied().unwrap_or(ApplicationObject { revision: 0, value: 0 });
    if value.revision != now.revision {
        return finish(domain, env, request, pending, ApplicationOutcome::Moved { object: value.object, now }, out);
    }
    if now.revision == u64::MAX {
        return finish(domain, env, request, pending, ApplicationOutcome::Exhausted, out);
    }
    match value.release {
        Some(release) => {
            assert!(domain.application.pending.insert(request, pending).is_ok(), "reserved application part");
            let project = match scope {
                crate::ClientScope::Project(project) => project,
                crate::ClientScope::Deployment | crate::ClientScope::Own => 0,
            };
            out.push(Request::ApplicationRelease { request, project, release });
        }
        None => changed(domain, env, request, pending, out),
    }
}

pub(crate) fn core_answer(
    domain: &mut Domain,
    env: &Env<Limits>,
    request: Token,
    answer: ApplicationCoreAnswer,
    out: &mut Queue<Request>,
) {
    let Some(pending) = domain.application.pending.remove(&request) else { return };
    match answer {
        ApplicationCoreAnswer::Released => changed(domain, env, request, pending, out),
        ApplicationCoreAnswer::Refused(why) => {
            finish(domain, env, request, pending, ApplicationOutcome::CoreRefused { why }, out);
        }
        ApplicationCoreAnswer::Again(why) => out.push(Request::ApplicationAgain { request, why }),
    }
}

fn changed(domain: &mut Domain, env: &Env<Limits>, request: Token, pending: Pending, out: &mut Queue<Request>) {
    let value = pending.request;
    let now =
        ApplicationObject { revision: value.revision.checked_add(1).expect("revision preflight"), value: value.value };
    assert!(domain.application.objects.insert(value.object, now).is_ok(), "configured object capacity");
    out.push(Request::Save { record: Record::ApplicationObject { object: value.object, now } });
    finish(domain, env, request, pending, ApplicationOutcome::Changed { object: value.object, now }, out);
}

fn finish(
    domain: &mut Domain,
    env: &Env<Limits>,
    request: Token,
    pending: Pending,
    outcome: ApplicationOutcome,
    out: &mut Queue<Request>,
) {
    let until = Wall::from_nanos(env.wall.as_nanos().saturating_add(pending.retention.as_nanos()));
    assert!(domain.application.answers.insert(pending.token, (until, outcome)).is_ok(), "reserved retained outcome");
    out.push(Request::Save { record: Record::ApplicationAnswer { token: pending.token, until, outcome } });
    out.push(Request::ApplicationDone { request, token: pending.token });
}

fn busy(request: Token, out: &mut Queue<Request>) {
    out.push(Request::ApplicationAgain { request, why: application_core::Again::Busy });
}

pub(crate) fn worst_case(limits: &Limits) -> Option<u64> {
    Map::<Token, ApplicationRequest>::worst_case(limits.staged)?
        .checked_add(Map::<Token, Pending>::worst_case(limits.staged)?)?
        .checked_add(Map::<u64, ApplicationObject>::worst_case(limits.resources)?)?
        .checked_add(Map::<Token, (Wall, ApplicationOutcome)>::worst_case(limits.values)?)
}
