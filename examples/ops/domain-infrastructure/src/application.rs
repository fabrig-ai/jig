//! Infrastructure-owned keyed requests (examples.md, section 8;
//! domain/connectors.md, section 5.5).

use crate::{Domain, Environment, EnvironmentFact, Key, Limits, Record, RecordKey, Request, Resource, SystemRequest};
use skein_lib::{Duration, Env, Map, Queue, Token, Wall};

/// Move an environment's end, naming this connector's revision.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ApplicationRequest {
    pub environment: Environment,
    pub revision: u64,
    pub until: u64,
}

/// The connector's current end, in wall-clock seconds.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct EnvironmentEnd {
    pub revision: u64,
    pub until: u64,
}

/// Own outcome retained independently of the people child's key metadata.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum ApplicationOutcome {
    MovedEnd { environment: Environment, now: EnvironmentEnd },
    Moved { environment: Environment, now: EnvironmentEnd },
    Role { held: u32 },
    Unknown,
    Past,
    Exhausted,
}

#[derive(Debug)]
pub(crate) struct State {
    prepared: Map<Token, ApplicationRequest>,
    ends: Map<Environment, (Key, EnvironmentEnd)>,
    answers: Map<Token, (Wall, ApplicationOutcome)>,
}

impl State {
    pub(crate) fn end(&self, environment: &Environment) -> Option<EnvironmentEnd> {
        Some(self.ends.get(environment)?.1)
    }
    /// The request basis is the saved desired end, or the owner's initial
    /// revision and observed lease when no desired record exists (examples.md, 8).
    pub(crate) fn request_end(&self, environment: &Environment, fact: EnvironmentFact) -> (Key, EnvironmentEnd) {
        self.ends
            .get(environment)
            .copied()
            .unwrap_or((fact.created_by, EnvironmentEnd { revision: 0, until: fact.until }))
    }
    pub(crate) fn new(limits: &Limits) -> Self {
        Self {
            prepared: Map::with_capacity(limits.staged),
            ends: Map::with_capacity(limits.environments),
            answers: Map::with_capacity(limits.proposals),
        }
    }
    pub(crate) fn prepare(&mut self, owner: Token, value: ApplicationRequest, limits: &Limits) {
        if !super::domain::resource_valid(&Resource::Environment(value.environment.clone()), limits) {
            return;
        }
        let _prepared = self.prepared.insert(owner, value);
    }
    pub(crate) fn drop(&mut self, owner: Token) {
        self.prepared.remove(&owner);
    }
    pub(crate) fn restore_end(&mut self, environment: Environment, created_by: Key, now: EnvironmentEnd) {
        assert!(self.ends.insert(environment, (created_by, now)).is_ok(), "restored environment end fits");
    }
    pub(crate) fn read_afresh(&self, refresh: &mut Map<Resource, bool>) {
        for (environment, _) in &self.ends {
            refresh.insert(Resource::Environment(environment.clone()), false).expect("saved end resources fit");
        }
    }
    pub(crate) fn reconcile(
        &mut self,
        environment: &Environment,
        fact: Option<EnvironmentFact>,
        out: &mut Queue<Request>,
    ) {
        let Some(stored) = self.ends.get(environment).copied() else { return };
        let Some(fact) = fact else { return };
        let (created_by, now) = stored;
        if created_by != fact.created_by {
            let Some(revision) = now.revision.checked_add(1) else { return };
            let now = EnvironmentEnd { revision, until: fact.until };
            self.ends.insert(environment.clone(), (fact.created_by, now)).expect("existing end revision fits");
            out.push(Request::Save {
                record: Record::EnvironmentEnd { environment: environment.clone(), created_by: fact.created_by, now },
            });
        } else if now.until != fact.until {
            out.push(Request::System(SystemRequest::MoveEnd {
                environment: environment.clone(),
                created_by,
                revision: now.revision,
                until: now.until,
            }));
        }
    }
    pub(crate) fn restore_answer(&mut self, token: Token, until: Wall, outcome: ApplicationOutcome) {
        assert!(
            token.raw() != 0 && self.answers.insert(token, (until, outcome)).is_ok(),
            "restored application outcome fits"
        );
    }
}

impl Domain {
    /// The root assembles this value in its admitted decision and journals the answer.
    #[must_use]
    #[expect(clippy::manual_map, reason = "step code uses explicit matching instead of closures")]
    pub fn application_outcome(&self, token: Token) -> Option<ApplicationOutcome> {
        match self.application.answers.get(&token) {
            Some((_, outcome)) => Some(outcome.clone()),
            None => None,
        }
    }
}

#[expect(clippy::too_many_arguments, reason = "the connector receives its admitted party envelope unchanged")]
pub(crate) fn decide(
    domain: &mut Domain,
    env: &Env<Limits>,
    request: Token,
    token: Token,
    owner: Token,
    _person: u64,
    _project: u32,
    role: u32,
    retention: Duration,
    out: &mut Queue<Request>,
) {
    let Some(value) = domain.application.prepared.remove(&owner) else { return busy(request, out) };
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
    if domain.application.answers.len() >= env.limits.proposals {
        return busy(request, out);
    }
    let outcome = if role > 2 {
        ApplicationOutcome::Role { held: role }
    } else {
        match domain.environments.get(&value.environment) {
            Some(Some(fact)) => {
                let now = domain.application.request_end(&value.environment, *fact);
                let (created_by, now) = now;
                if created_by != fact.created_by {
                    return keep(domain, env, request, token, retention, ApplicationOutcome::Exhausted, out);
                }
                if now.revision != value.revision {
                    ApplicationOutcome::Moved { environment: value.environment, now }
                } else if value.until <= env.wall.as_nanos() / 1_000_000_000 {
                    ApplicationOutcome::Past
                } else if now.revision == u64::MAX {
                    ApplicationOutcome::Exhausted
                } else {
                    let now = EnvironmentEnd {
                        revision: now.revision.checked_add(1).expect("end revision preflight"),
                        until: value.until,
                    };
                    if domain.application.ends.insert(value.environment.clone(), (created_by, now)).is_err() {
                        return busy(request, out);
                    }
                    out.push(Request::Save {
                        record: Record::EnvironmentEnd { environment: value.environment.clone(), created_by, now },
                    });
                    out.push(Request::System(SystemRequest::MoveEnd {
                        environment: value.environment.clone(),
                        created_by,
                        revision: now.revision,
                        until: now.until,
                    }));
                    ApplicationOutcome::MovedEnd { environment: value.environment, now }
                }
            }
            Some(None) | None => ApplicationOutcome::Unknown,
        }
    };
    keep(domain, env, request, token, retention, outcome, out);
}

fn keep(
    domain: &mut Domain,
    env: &Env<Limits>,
    request: Token,
    token: Token,
    retention: Duration,
    outcome: ApplicationOutcome,
    out: &mut Queue<Request>,
) {
    let until = Wall::from_nanos(env.wall.as_nanos().saturating_add(retention.as_nanos()));
    assert!(domain.application.answers.insert(token, (until, outcome.clone())).is_ok(), "reserved outcome capacity");
    out.push(Request::Save { record: Record::ApplicationAnswer { token, until, outcome } });
    out.push(Request::ApplicationDone { request, token });
}

fn busy(request: Token, out: &mut Queue<Request>) {
    out.push(Request::ApplicationBusy { request });
}
