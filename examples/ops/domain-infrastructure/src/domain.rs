use alloc::boxed::Box;
use skein_lib::{Env, List, Map, Queue, Time, Token};

use crate::{
    ApplyResult, Backend, Description, Effect, Entry, Environment, EnvironmentFact, Event, Form, Hold, Key, Limits,
    Looked, Named, Outcome, Phase, Pool, Procedure, ProcedurePhase, ProcedureSignal, ProcedureState, Record, RecordKey,
    Recovery, Request, Resource, Service, ServiceFact, StepDecision, SystemEvent, SystemRequest,
};

/// One fact may wake sixteen scale readers and report drift to sixteen tasks.
pub const MAX_OUT: u32 = 64;

/// Infrastructure's bounded working set and durable records.
#[derive(Debug)]
pub struct Domain {
    pub(crate) client_subjects: Map<Token, crate::ClientSubject>,
    pub(crate) application: crate::application::State,
    restart: Option<crate::RestartStage>,
    /// Restores are passive until the scripted read-afresh step (engine.md, 6).
    restoring: bool,
    closing: Map<u64, bool>,
    closes: Map<[u8; 16], (u32, u64)>,
    refresh: Map<Resource, bool>,
    pub(crate) backend: Backend,
    pub(crate) services: Map<Service, ServiceFact>,
    pub(crate) environments: Map<Environment, Option<EnvironmentFact>>,
    pub(crate) pools: Map<Pool, (u32, u32)>,
    pub(crate) reliance: Map<u64, (u32, Box<[Named]>)>,
    pub(crate) procedures: Map<u64, ProcedureState>,
    staged: Map<Token, Effect>,
    pub(crate) proposals: Map<u64, (u64, Effect)>,
    pub(crate) effects: Map<Key, Entry>,
    pub(crate) made: Map<Environment, (Key, Option<u32>)>,
    pub(crate) drifts: Map<Resource, crate::DriftAccount>,
}

impl Domain {
    /// Allocates the connector's tables and chooses the backend's recovery class.
    #[must_use]
    pub fn new(backend: Backend, limits: &Limits) -> Self {
        assert!(crate::worst_case(limits).is_some(), "connector limits fit");
        assert!(
            limits.tasks <= 16
                && limits.effects <= 16
                && limits.procedures <= 16
                && limits.made <= 16
                && limits.resources_per_task <= 16,
            "MAX_OUT covers one step"
        );
        Self {
            client_subjects: Map::with_capacity(limits.proposals),
            application: crate::application::State::new(limits),
            restart: None,
            restoring: false,
            closes: Map::with_capacity(limits.made),
            closing: Map::with_capacity(
                limits
                    .tasks
                    .checked_add(limits.procedures)
                    .expect("owners")
                    .checked_add(limits.effects)
                    .expect("outbox owners"),
            ),
            refresh: Map::with_capacity(
                limits
                    .tasks
                    .checked_mul(limits.resources_per_task)
                    .expect("live resources")
                    .checked_add(limits.procedures)
                    .expect("procedure resources")
                    .checked_add(limits.environments)
                    .expect("saved environment ends")
                    .checked_add(limits.made)
                    .expect("retained creation resources"),
            ),
            backend,
            services: Map::with_capacity(limits.services),
            environments: Map::with_capacity(limits.environments),
            pools: Map::with_capacity(limits.pools),
            reliance: Map::with_capacity(limits.tasks),
            procedures: Map::with_capacity(limits.procedures),
            staged: Map::with_capacity(limits.staged),
            proposals: Map::with_capacity(limits.proposals),
            effects: Map::with_capacity(limits.effects),
            made: Map::with_capacity(limits.made),
            drifts: Map::with_capacity(crate::drift_accounts::capacity(limits).expect("drift bound")),
        }
    }
}

fn wall_seconds(env: &Env<Limits>) -> u64 {
    env.wall.as_nanos() / 1_000_000_000
}

fn bytes_valid(bytes: &[u8], limits: &Limits) -> bool {
    !bytes.is_empty() && u32::try_from(bytes.len()).unwrap_or(u32::MAX) <= limits.name_bytes
}

pub(crate) fn resource_valid(resource: &Resource, limits: &Limits) -> bool {
    match resource {
        Resource::Service(service) => bytes_valid(&service.environment, limits) && bytes_valid(&service.name, limits),
        Resource::Environment(environment) => {
            bytes_valid(&environment.pool, limits) && bytes_valid(&environment.name, limits)
        }
        Resource::Pool(pool) | Resource::Quota(pool) => bytes_valid(&pool.0, limits),
    }
}

fn hold(resource: &Resource) -> Hold {
    match resource {
        Resource::Service(..) | Resource::Environment(..) => Hold::ExclusiveWait,
        Resource::Pool(..) => Hold::PooledWait,
        Resource::Quota(..) => Hold::None,
    }
}

fn effect_valid(effect: &Effect, limits: &Limits) -> bool {
    match effect {
        Effect::Restart { service, .. } | Effect::Scale { service, .. } => {
            resource_valid(&Resource::Service(service.clone()), limits)
        }
        Effect::Rollback { service, from, to } => {
            resource_valid(&Resource::Service(service.clone()), limits)
                && bytes_valid(from, limits)
                && bytes_valid(to, limits)
        }
        Effect::CreateEnvironment { environment, .. } | Effect::TearDown { environment, .. } => {
            resource_valid(&Resource::Environment(environment.clone()), limits)
        }
    }
}

pub(crate) fn describe(effect: &Effect, backend: Backend) -> Description {
    match effect {
        Effect::Restart { service, operation } => Description {
            kind: 1,
            resources: Box::from([Resource::Service(service.clone())]),
            condition: false,
            form: Form::Transition,
            recovery: if backend == Backend::OperationIds { Recovery::Keyed } else { Recovery::Unrecoverable },
            price: None,
            state: *operation,
        },
        Effect::Scale { service, from: _, to } => Description {
            kind: 2,
            resources: Box::from([Resource::Service(service.clone())]),
            condition: true,
            form: Form::Set,
            recovery: Recovery::Conditional,
            price: None,
            state: u64::from(*to),
        },
        Effect::Rollback { service, from: _, to: _ } => Description {
            kind: 3,
            resources: Box::from([Resource::Service(service.clone())]),
            condition: true,
            form: Form::Transition,
            recovery: Recovery::Conditional,
            price: None,
            state: 0,
        },
        Effect::CreateEnvironment { environment, until, price } => Description {
            kind: 4,
            resources: Box::from([
                Resource::Environment(environment.clone()),
                Resource::Pool(Pool(environment.pool.clone())),
            ]),
            condition: false,
            form: Form::Creation,
            recovery: Recovery::Keyed,
            price: Some(*price),
            state: *until,
        },
        Effect::TearDown { environment, created_by: _ } => Description {
            kind: 5,
            resources: Box::from([Resource::Environment(environment.clone())]),
            condition: true,
            form: Form::Transition,
            recovery: Recovery::Conditional,
            price: None,
            state: 0,
        },
    }
}

fn names(
    domain: &mut Domain,
    env: &Env<Limits>,
    task: u64,
    project: u32,
    resources: Box<[Resource]>,
    out: &mut Queue<Request>,
) {
    if u32::try_from(resources.len()).unwrap_or(u32::MAX) > env.limits.resources_per_task
        || domain.reliance.len() >= env.limits.tasks && !domain.reliance.contains_key(&task)
    {
        return;
    }
    for resource in &resources {
        if !resource_valid(resource, &env.limits) {
            return;
        }
    }
    let mut named = List::with_capacity(env.limits.resources_per_task);
    for resource in &resources {
        named
            .push(Named { resource: resource.clone(), role: crate::ResourceRole::Owned, hold: hold(resource) })
            .expect("resource count checked");
    }
    let resources = named.into_boxed();
    domain.reliance.insert(task, (project, resources.clone())).expect("task capacity checked");
    out.push(Request::Save { record: Record::Rely { task, project, resources: resources.clone() } });
    out.push(Request::Named { task, project, resources });
}

// examples.md 7.3 and connectors.md 11: a pending restart has not written;
// its healthy condition is read again rather than held as write drift.
fn pending_restart_satisfied(domain: &Domain, task: u64, resource: &Resource) -> bool {
    let Some(state) = domain.procedures.get(&task) else { return false };
    if state.phase != ProcedurePhase::EffectOutstanding {
        return false;
    }
    let effect = match &state.procedure {
        Procedure::Remediate { service, operation, .. } => {
            if resource != &Resource::Service(service.clone()) {
                return false;
            }
            let Some(fact) = domain.services.get(service) else { return false };
            if !fact.healthy {
                return false;
            }
            Effect::Restart { service: service.clone(), operation: *operation }
        }
        Procedure::Scale { .. } | Procedure::Provision { .. } | Procedure::TearDown { .. } => return false,
    };
    for (key, entry) in &domain.effects {
        if key.task == task && entry.effect == effect {
            return false;
        }
    }
    for (_, proposal) in &domain.proposals {
        if proposal.0 == task && proposal.1 == effect {
            return true;
        }
    }
    false
}

fn drift(domain: &Domain, resource: Resource, out: &mut Queue<Request>) {
    for (task, (_, resources)) in &domain.reliance {
        let mut found = false;
        for named in resources {
            found |= named.resource == resource;
        }
        if found && !pending_restart_satisfied(domain, *task, &resource) {
            out.push(Request::Drift { task: *task, resource: resource.clone() });
        }
    }
}

fn fresh_service(
    domain: &mut Domain,
    env: &Env<Limits>,
    service: Service,
    fact: ServiceFact,
    other_hand: bool,
    out: &mut Queue<Request>,
) {
    if !resource_valid(&Resource::Service(service.clone()), &env.limits)
        || domain.services.len() >= env.limits.services && !domain.services.contains_key(&service)
    {
        return;
    }
    domain.refresh.remove(&Resource::Service(service.clone()));
    let old = domain.services.insert(service.clone(), fact.clone()).expect("service capacity checked");
    let mut ready = List::with_capacity(env.limits.procedures);
    for (task, state) in &domain.procedures {
        match &state.procedure {
            Procedure::Scale { service: target, .. } => {
                if target == &service && state.phase == ProcedurePhase::ReadingFacts {
                    ready.push(*task).expect("procedure count fits");
                }
            }
            Procedure::Remediate { .. } | Procedure::Provision { .. } | Procedure::TearDown { .. } => {}
        }
    }
    for task in &ready {
        let state = domain.procedures.get_mut(task).expect("waiting procedure exists");
        state.phase = ProcedurePhase::FactsReady;
        out.push(Request::Save { record: Record::Procedure(state.clone()) });
    }
    if old != Some(fact.clone()) || !ready.is_empty() {
        out.push(Request::Changed { resource: Resource::Service(service.clone()) });
    }
    let revised = match old {
        Some(before) => before.revision != fact.revision,
        None => false,
    };
    if other_hand && revised {
        let resource = Resource::Service(service);
        crate::drift_accounts::observed(
            domain,
            env,
            crate::DriftAccount { resource: resource.clone(), kind: crate::DriftKind::Changed, at: env.wall, by: None },
            out,
        );
        drift(domain, resource, out);
    }
}

fn fresh_environment(
    domain: &mut Domain,
    env: &Env<Limits>,
    environment: Environment,
    fact: Option<EnvironmentFact>,
    other_hand: bool,
    out: &mut Queue<Request>,
) {
    if !resource_valid(&Resource::Environment(environment.clone()), &env.limits)
        || domain.environments.len() >= env.limits.environments && !domain.environments.contains_key(&environment)
    {
        return;
    }
    domain.application.reconcile(&environment, fact, out);
    domain.refresh.remove(&Resource::Environment(environment.clone()));
    let old = domain.environments.insert(environment.clone(), fact).expect("environment capacity checked");
    if old != Some(fact) {
        out.push(Request::Changed { resource: Resource::Environment(environment.clone()) });
    }
    if other_hand && fact.is_none() && domain.made.contains_key(&environment) {
        let resource = Resource::Environment(environment);
        if old != Some(fact) {
            crate::drift_accounts::observed(
                domain,
                env,
                crate::DriftAccount {
                    resource: resource.clone(),
                    kind: crate::DriftKind::Deleted,
                    at: env.wall,
                    by: None,
                },
                out,
            );
        }
        drift(domain, resource, out);
    }
}

fn fresh_pool(domain: &mut Domain, env: &Env<Limits>, pool: Pool, quota: u32, used: u32, out: &mut Queue<Request>) {
    if !resource_valid(&Resource::Pool(pool.clone()), &env.limits)
        || domain.pools.len() >= env.limits.pools && !domain.pools.contains_key(&pool)
    {
        return;
    }
    domain.refresh.remove(&Resource::Pool(pool.clone()));
    domain.refresh.remove(&Resource::Quota(pool.clone()));
    let old = domain.pools.insert(pool.clone(), (quota, used)).expect("pool capacity checked");
    if old != Some((quota, used)) {
        out.push(Request::Slots { pool: pool.clone(), quota, used });
        out.push(Request::Changed { resource: Resource::Quota(pool) });
    }
}

fn restore(domain: &mut Domain, record: Record) {
    domain.restoring = true;
    match record {
        Record::Closing { key, project, chat } => {
            domain.closes.insert(key, (project, chat)).expect("retained close fits creation bound");
        }
        Record::EnvironmentEnd { environment, created_by, now } => {
            domain.application.restore_end(environment, created_by, now);
        }
        Record::ApplicationAnswer { token, until, outcome } => domain.application.restore_answer(token, until, outcome),
        Record::Rely { task, project, resources } => {
            domain.reliance.insert(task, (project, resources)).expect("restored task fits");
        }
        Record::Procedure(state) => {
            domain.procedures.insert(state.task, state).expect("restored procedure fits");
        }
        Record::Proposal { number, task, effect } => {
            domain.proposals.insert(number, (task, effect)).expect("restored proposal fits");
        }
        Record::Outbox(entry) => {
            domain.effects.insert(entry.key, entry).expect("restored effect fits");
        }
        Record::Drift(account) => {
            domain.drifts.insert(account.resource.clone(), account).expect("restored drift fits");
        }
        Record::Made { environment, key, project } => {
            domain.made.insert(environment, (key, project)).expect("restored owner fits");
        }
    }
}

fn make(domain: &mut Domain, env: &Env<Limits>, key: Key, out: &mut Queue<Request>) {
    if let Some(entry) = domain.effects.get_mut(&key)
        && entry.phase == Phase::Kept
        && (entry.attempt == 0 || wall_seconds(env) >= entry.deadline)
    {
        entry.phase = Phase::Sent;
        entry.attempt = entry.attempt.checked_add(1).expect("attempt fits");
        entry.deadline = wall_seconds(env).checked_add(env.limits.retry_after_seconds).expect("deadline fits");
        out.push(Request::Save { record: Record::Outbox(entry.clone()) });
        out.push(Request::System(SystemRequest::Apply { key, attempt: entry.attempt, effect: entry.effect.clone() }));
    }
}

#[expect(clippy::manual_map, reason = "step code uses no closure-taking methods")]
fn settled(domain: &mut Domain, key: Key, outcome: Outcome, out: &mut Queue<Request>) {
    let Some(entry) = domain.effects.remove(&key) else {
        return;
    };
    out.push(Request::Erase { key: RecordKey::Outbox(key) });
    let mut close = None;
    if outcome == Outcome::Made {
        match entry.effect {
            Effect::CreateEnvironment { environment, .. } => {
                let project = match entry.project {
                    Some(project) => Some(project),
                    None => match domain.reliance.get(&key.task) {
                        Some((project, _)) => Some(*project),
                        None => None,
                    },
                };
                domain.made.insert(environment.clone(), (key, project)).expect("made capacity reserved");
                out.push(Request::Save { record: Record::Made { environment, key, project } });
            }
            Effect::TearDown { environment, created_by } => {
                if let Some(state) = domain.procedures.get(&key.task)
                    && let Some(chat) = state.chat
                    && let Some(project) = entry.project
                {
                    // A provision has one stable creation purpose per task in
                    // this deployment. Acts have their own connector key space.
                    let mut key = [0; 16];
                    let bytes = created_by.task.to_be_bytes().into_iter().chain(created_by.purpose.to_be_bytes());
                    for (slot, byte) in key.iter_mut().zip(bytes) {
                        *slot = byte;
                    }
                    domain.closes.insert(key, (project, chat)).expect("creation reserved close room");
                    out.push(Request::Save { record: Record::Closing { key, project, chat } });
                    close = Some(Request::Close { key, project, chat });
                }
                domain.made.remove(&environment);
                out.push(Request::Erase { key: RecordKey::Made(environment) });
            }
            Effect::Restart { .. } | Effect::Scale { .. } | Effect::Rollback { .. } => {}
        }
    }
    out.push(Request::Outcome { entry: entry.number, key, outcome });
    if let Some(close) = close {
        out.push(close);
    }
}

fn applied(domain: &mut Domain, key: Key, attempt: u32, result: ApplyResult, out: &mut Queue<Request>) {
    let Some(entry) = domain.effects.get_mut(&key) else {
        return;
    };
    if attempt > entry.attempt || (entry.phase == Phase::Kept && entry.attempt == 0) || entry.phase == Phase::Held {
        return;
    }
    match result {
        ApplyResult::Made => settled(domain, key, Outcome::Made, out),
        ApplyResult::Conflict | ApplyResult::Missing | ApplyResult::Full | ApplyResult::Error => {
            if attempt == entry.attempt && entry.phase == Phase::Sent {
                settled(domain, key, Outcome::Failed, out);
            }
        }
        ApplyResult::Uncertain => {
            if attempt == entry.attempt && entry.phase == Phase::Sent {
                let recovery = describe(&entry.effect, domain.backend).recovery;
                if recovery == Recovery::Unrecoverable {
                    entry.phase = Phase::Held;
                    out.push(Request::Save { record: Record::Outbox(entry.clone()) });
                    out.push(Request::Outcome { entry: entry.number, key, outcome: Outcome::Uncertain });
                } else {
                    entry.phase = Phase::Uncertain;
                    out.push(Request::Save { record: Record::Outbox(entry.clone()) });
                    out.push(Request::System(SystemRequest::Look { key, effect: entry.effect.clone() }));
                }
            }
        }
    }
}

fn looked(domain: &mut Domain, env: &Env<Limits>, key: Key, result: Looked, out: &mut Queue<Request>) {
    let Some(entry) = domain.effects.get_mut(&key) else {
        return;
    };
    if entry.phase != Phase::Sent && entry.phase != Phase::Uncertain {
        return;
    }
    match result {
        Looked::Made => settled(domain, key, Outcome::Made, out),
        Looked::Ambiguous => {
            entry.phase = Phase::Held;
            out.push(Request::Save { record: Record::Outbox(entry.clone()) });
            out.push(Request::Outcome { entry: entry.number, key, outcome: Outcome::Uncertain });
        }
        Looked::CanRetry => {
            if wall_seconds(env) >= entry.deadline {
                if entry.attempt >= env.limits.max_attempts {
                    entry.phase = Phase::Held;
                    out.push(Request::Save { record: Record::Outbox(entry.clone()) });
                    out.push(Request::Outcome { entry: entry.number, key, outcome: Outcome::Uncertain });
                } else {
                    entry.phase = Phase::Kept;
                    out.push(Request::Save { record: Record::Outbox(entry.clone()) });
                    out.push(Request::Make { key });
                }
            } else {
                entry.phase = Phase::Uncertain;
                out.push(Request::Save { record: Record::Outbox(entry.clone()) });
            }
        }
    }
}

fn start_procedure(
    domain: &mut Domain,
    env: &Env<Limits>,
    task: u64,
    project: u32,
    procedure: Procedure,
    chat: Option<u64>,
    out: &mut Queue<Request>,
) {
    if domain.procedures.contains_key(&task) || domain.procedures.len() >= env.limits.procedures {
        return;
    }
    // Retain the admitted scope and this owner's procedure resource before its
    // panel or outbox can be constructed. Existing names remain authoritative.
    if !domain.reliance.contains_key(&task) {
        let resource = match &procedure {
            Procedure::Remediate { service, .. } | Procedure::Scale { service, .. } => {
                Resource::Service(service.clone())
            }
            Procedure::Provision { environment, .. } | Procedure::TearDown { environment, .. } => {
                Resource::Environment(environment.clone())
            }
        };
        names(domain, env, task, project, Box::from([resource]), out);
    }
    match domain.reliance.get(&task) {
        Some((owner, _)) if *owner == project => {}
        Some(_) | None => return,
    }
    let state = ProcedureState { task, chat, procedure, phase: ProcedurePhase::Active };
    domain.procedures.insert(task, state.clone()).expect("procedure capacity checked");
    out.push(Request::Save { record: Record::Procedure(state) });
}

fn procedure(domain: &mut Domain, env: &Env<Limits>, task: u64, signal: ProcedureSignal, out: &mut Queue<Request>) {
    let Some(mut state) = domain.procedures.get(&task).cloned() else {
        return;
    };
    let before = state.clone();
    let now = wall_seconds(env);
    match signal {
        ProcedureSignal::Step => {}
        ProcedureSignal::EffectMade => {
            if state.phase == ProcedurePhase::EffectOutstanding {
                state.phase = ProcedurePhase::WaitingCondition;
            }
        }
        ProcedureSignal::EffectWaiting => {
            if state.phase == ProcedurePhase::EffectOutstanding {
                state.phase = ProcedurePhase::Active;
            }
        }
        ProcedureSignal::EffectFailed => {
            state.phase = ProcedurePhase::Held;
        }
        ProcedureSignal::Release => match &state.procedure {
            Procedure::Provision { environment, .. } => {
                let Some(key) = domain.made.get(environment) else {
                    return;
                };
                state.procedure = Procedure::TearDown {
                    environment: environment.clone(),
                    created_by: key.0,
                    deadline: now.saturating_add(30),
                };
                state.phase = ProcedurePhase::Active;
            }
            Procedure::Remediate { .. } | Procedure::Scale { .. } | Procedure::TearDown { .. } => {
                state.phase = ProcedurePhase::Done;
            }
        },
    }
    match signal {
        ProcedureSignal::EffectMade | ProcedureSignal::EffectWaiting => {
            // An outcome updates this owner's state and refreshes facts. A
            // decision belongs to the next step offered by the task hub.
            let read = match &state.procedure {
                Procedure::Scale { service, .. } => {
                    state.phase = ProcedurePhase::ReadingFacts;
                    SystemRequest::Service { service: service.clone() }
                }
                Procedure::Remediate { service, .. } => SystemRequest::Service { service: service.clone() },
                Procedure::Provision { environment, .. } | Procedure::TearDown { environment, .. } => {
                    SystemRequest::Environment { environment: environment.clone() }
                }
            };
            if state != before {
                domain.procedures.insert(task, state.clone()).expect("live procedure");
                out.push(Request::Save { record: Record::Procedure(state) });
            }
            out.push(Request::System(read));
            return;
        }
        ProcedureSignal::EffectFailed => {
            domain.procedures.insert(task, state.clone()).expect("live procedure");
            out.push(Request::Save { record: Record::Procedure(state) });
            return;
        }
        ProcedureSignal::Step | ProcedureSignal::Release => {}
    }
    let (phase, decision, read) = decide(domain, now, &state);
    state.phase = phase;
    if state.phase == ProcedurePhase::Done {
        domain.procedures.remove(&task);
        out.push(Request::Erase { key: RecordKey::Procedure(task) });
    } else {
        domain.procedures.insert(task, state.clone()).expect("procedure remains within capacity");
        if state != before {
            out.push(Request::Save { record: Record::Procedure(state) });
        }
    }
    if let Some(read) = read {
        out.push(Request::System(read));
    }
    if let Some(decision) = decision {
        out.push(Request::Step { task, decision });
    }
}

#[expect(clippy::too_many_lines, reason = "procedures share one level-triggered decision point")]
fn decide(
    domain: &Domain,
    now: u64,
    state: &ProcedureState,
) -> (ProcedurePhase, Option<StepDecision>, Option<SystemRequest>) {
    match &state.procedure {
        Procedure::Remediate { service, operation, deadline } => {
            if state.phase == ProcedurePhase::Held || state.phase == ProcedurePhase::Done {
                return (state.phase, None, None);
            }
            if now > *deadline {
                return (ProcedurePhase::Held, Some(StepDecision::Hold { reason: 1 }), None);
            }
            let Some(fact) = domain.services.get(service) else {
                return (
                    state.phase,
                    Some(StepDecision::Wait { until: *deadline }),
                    Some(SystemRequest::Service { service: service.clone() }),
                );
            };
            if fact.healthy {
                if pending_restart_satisfied(domain, state.task, &Resource::Service(service.clone())) {
                    return (
                        ProcedurePhase::Done,
                        Some(StepDecision::FinishReport {
                            words: Box::from(*b"Service is already healthy; finished without restarting it."),
                        }),
                        None,
                    );
                }
                return (ProcedurePhase::Done, Some(StepDecision::Finish), None);
            }
            match state.phase {
                ProcedurePhase::Active => (
                    ProcedurePhase::EffectOutstanding,
                    Some(StepDecision::Effect(Effect::Restart { service: service.clone(), operation: *operation })),
                    None,
                ),
                ProcedurePhase::EffectOutstanding => (state.phase, Some(StepDecision::Wait { until: *deadline }), None),
                ProcedurePhase::WaitingCondition => (
                    state.phase,
                    Some(StepDecision::Wait { until: *deadline }),
                    Some(SystemRequest::Service { service: service.clone() }),
                ),
                ProcedurePhase::ReadingFacts
                | ProcedurePhase::FactsReady
                | ProcedurePhase::Holding
                | ProcedurePhase::Done
                | ProcedurePhase::Held => (state.phase, None, None),
            }
        }
        Procedure::Scale { service, replicas, deadline } => {
            if now > *deadline {
                return (ProcedurePhase::Held, Some(StepDecision::Hold { reason: 4 }), None);
            }
            match state.phase {
                ProcedurePhase::Active | ProcedurePhase::WaitingCondition => (
                    ProcedurePhase::ReadingFacts,
                    Some(StepDecision::Wait { until: *deadline }),
                    Some(SystemRequest::Service { service: service.clone() }),
                ),
                ProcedurePhase::ReadingFacts | ProcedurePhase::EffectOutstanding => {
                    (state.phase, Some(StepDecision::Wait { until: *deadline }), None)
                }
                ProcedurePhase::FactsReady => match domain.services.get(service) {
                    Some(fact) if fact.replicas == *replicas => {
                        (ProcedurePhase::Done, Some(StepDecision::Finish), None)
                    }
                    Some(fact) => (
                        ProcedurePhase::EffectOutstanding,
                        Some(StepDecision::Effect(Effect::Scale {
                            service: service.clone(),
                            from: fact.replicas,
                            to: *replicas,
                        })),
                        None,
                    ),
                    None => (ProcedurePhase::Active, Some(StepDecision::Wait { until: *deadline }), None),
                },
                ProcedurePhase::Holding | ProcedurePhase::Done | ProcedurePhase::Held => (state.phase, None, None),
            }
        }
        Procedure::Provision { environment, until, price, deadline } => {
            if state.phase == ProcedurePhase::Holding || state.phase == ProcedurePhase::Held {
                return (state.phase, None, None);
            }
            if now > *deadline {
                return (ProcedurePhase::Held, Some(StepDecision::Hold { reason: 2 }), None);
            }
            let fact = domain.environments.get(environment);
            match fact {
                Some(Some(found)) if found.ready_at <= now => {
                    (ProcedurePhase::Holding, Some(StepDecision::Finish), None)
                }
                Some(Some(found)) => (
                    ProcedurePhase::WaitingCondition,
                    Some(StepDecision::Wait { until: found.ready_at.min(*deadline) }),
                    Some(SystemRequest::Environment { environment: environment.clone() }),
                ),
                Some(None) => match state.phase {
                    ProcedurePhase::Active => (
                        ProcedurePhase::EffectOutstanding,
                        Some(StepDecision::Effect(Effect::CreateEnvironment {
                            environment: environment.clone(),
                            until: *until,
                            price: *price,
                        })),
                        None,
                    ),
                    ProcedurePhase::EffectOutstanding => {
                        (state.phase, Some(StepDecision::Wait { until: *deadline }), None)
                    }
                    ProcedurePhase::WaitingCondition => (
                        state.phase,
                        Some(StepDecision::Wait { until: *deadline }),
                        Some(SystemRequest::Environment { environment: environment.clone() }),
                    ),
                    ProcedurePhase::ReadingFacts
                    | ProcedurePhase::FactsReady
                    | ProcedurePhase::Holding
                    | ProcedurePhase::Done
                    | ProcedurePhase::Held => (state.phase, None, None),
                },
                None => (
                    state.phase,
                    Some(StepDecision::Wait { until: *deadline }),
                    Some(SystemRequest::Environment { environment: environment.clone() }),
                ),
            }
        }
        Procedure::TearDown { environment, created_by, deadline } => {
            if state.phase == ProcedurePhase::Held || state.phase == ProcedurePhase::Done {
                return (state.phase, None, None);
            }
            if now > *deadline {
                return (ProcedurePhase::Held, Some(StepDecision::Hold { reason: 3 }), None);
            }
            match domain.environments.get(environment) {
                Some(None) => (ProcedurePhase::Done, Some(StepDecision::Finish), None),
                Some(Some(_)) => match state.phase {
                    ProcedurePhase::Active => (
                        ProcedurePhase::EffectOutstanding,
                        Some(StepDecision::Effect(Effect::TearDown {
                            environment: environment.clone(),
                            created_by: *created_by,
                        })),
                        None,
                    ),
                    ProcedurePhase::EffectOutstanding => {
                        (state.phase, Some(StepDecision::Wait { until: *deadline }), None)
                    }
                    ProcedurePhase::WaitingCondition => (
                        state.phase,
                        Some(StepDecision::Wait { until: *deadline }),
                        Some(SystemRequest::Environment { environment: environment.clone() }),
                    ),
                    ProcedurePhase::ReadingFacts
                    | ProcedurePhase::FactsReady
                    | ProcedurePhase::Holding
                    | ProcedurePhase::Done
                    | ProcedurePhase::Held => (state.phase, None, None),
                },
                None => (
                    state.phase,
                    Some(StepDecision::Wait { until: *deadline }),
                    Some(SystemRequest::Environment { environment: environment.clone() }),
                ),
            }
        }
    }
}

fn stage(domain: &mut Domain, env: &Env<Limits>, token: Token, effect: Effect, out: &mut Queue<Request>) {
    if effect_valid(&effect, &env.limits) && domain.staged.len() < env.limits.staged {
        let description = describe(&effect, domain.backend);
        let waits_for_dependencies = match &effect {
            Effect::Restart { .. }
            | Effect::Scale { .. }
            | Effect::Rollback { .. }
            | Effect::CreateEnvironment { .. }
            | Effect::TearDown { .. } => true,
        };
        domain.staged.insert(token, effect).expect("stage capacity checked");
        out.push(Request::Described { token, description, waits_for_dependencies });
    } else {
        out.push(Request::Refused { token });
    }
}

/// Routes one event with room reserved for its worst case.
#[expect(clippy::too_many_lines, reason = "one exhaustive dispatcher routes the complete connector vocabulary")]
pub fn step(domain: &mut Domain, env: &Env<Limits>, event: Event, out: &mut Queue<Request>) {
    assert!(out.room() >= MAX_OUT, "the root reserves MAX_OUT slots");
    match event {
        Event::Closed { key, outcome } => match outcome {
            crate::CloseOutcome::NotOffered | crate::CloseOutcome::NoOwner | crate::CloseOutcome::Again => {}
            crate::CloseOutcome::Closed | crate::CloseOutcome::Ended | crate::CloseOutcome::Refused => {
                if domain.closes.remove(&key).is_some() {
                    out.push(Request::Erase { key: RecordKey::Closing(key) });
                }
            }
        },
        Event::SendCloses => {
            for (key, (project, chat)) in &domain.closes {
                out.push(Request::Close { key: *key, project: *project, chat: *chat });
            }
        }
        Event::DocumentPrepare { token, subject } => {
            crate::client_documents::prepare(domain, &env.limits, token, subject);
        }
        Event::DocumentForget { subject } => {
            domain.client_subjects.remove(&subject);
        }
        Event::Document { watcher, scope, subject, party } => {
            crate::client_documents::document(domain, &env.limits, watcher, scope, subject, party, out);
        }
        Event::ReadDocument { scope, read, party } => {
            crate::client_documents::read(domain, &env.limits, scope, read, party, out);
        }
        Event::ApplicationPrepare { owner, value } => domain.application.prepare(owner, value, &env.limits),
        Event::ApplicationDrop { owner } => domain.application.drop(owner),
        Event::Application { request, token, owner, person, project, role, retention } => {
            crate::application::decide(domain, env, request, token, owner, person, project, role, retention, out);
        }
        Event::Close { task } => {
            domain.closing.insert(task, true).expect("live owner capacity");
        }
        Event::Release { task } => {
            if domain.reliance.remove(&task).is_some() {
                out.push(Request::Erase { key: RecordKey::Rely(task) });
            }
            if domain.procedures.remove(&task).is_some() {
                out.push(Request::Erase { key: RecordKey::Procedure(task) });
            }

            out.push(Request::Released { task });
        }

        Event::Names { task, project, resources } => names(domain, env, task, project, resources, out),
        Event::Unnamed { task } => {
            if domain.reliance.remove(&task).is_some() {
                out.push(Request::Erase { key: RecordKey::Rely(task) });
            }
        }
        Event::StartProcedure { task, project, chat, procedure, pending: _, added: _ } => {
            start_procedure(domain, env, task, project, procedure, chat, out);
        }
        Event::ProcedureDelegateOutcome { .. } => {}
        Event::Procedure { task, signal } => procedure(domain, env, task, signal, out),
        Event::Describe { token, effect } => stage(domain, env, token, effect, out),
        Event::DescribeProposal { token, number } => match domain.proposals.get(&number) {
            Some((_, effect)) => {
                let effect = effect.clone();
                stage(domain, env, token, effect, out);
            }
            None => out.push(Request::Refused { token }),
        },
        Event::KeepProposal { token, number, task } => {
            let effect = domain.staged.remove(&token).expect("described proposal payload");
            domain.proposals.insert(number, (task, effect.clone())).expect("proposal capacity reserved");
            out.push(Request::Save { record: Record::Proposal { number, task, effect } });
        }
        Event::DropProposal { number } => {
            if domain.proposals.remove(&number).is_some() {
                out.push(Request::Erase { key: RecordKey::Proposal(number) });
            }
        }
        Event::Keep { token, entry: number, key, project } => {
            if let Some(effect) = domain.staged.remove(&token) {
                let owner_room = match &effect {
                    Effect::CreateEnvironment { environment, .. } => creation_room(domain, &env.limits, environment),
                    Effect::Restart { .. }
                    | Effect::Scale { .. }
                    | Effect::Rollback { .. }
                    | Effect::TearDown { .. } => true,
                };
                if domain.effects.len() < env.limits.effects && !domain.effects.contains_key(&key) && owner_room {
                    let entry = Entry { number, key, effect, project, attempt: 0, deadline: 0, phase: Phase::Kept };
                    domain.effects.insert(key, entry.clone()).expect("outbox capacity checked");
                    out.push(Request::Save { record: Record::Outbox(entry) });
                    out.push(Request::Make { key });
                } else {
                    out.push(Request::Refused { token });
                }
            }
        }
        Event::Drop { token } => {
            domain.staged.remove(&token);
        }
        Event::ResolveEntry { entry: number, repeat } => {
            let mut found = None;
            for (key, entry) in &domain.effects {
                if entry.number == number && entry.phase == Phase::Held {
                    found = Some(*key);
                    break;
                }
            }
            if let Some(key) = found {
                if repeat {
                    let entry = domain.effects.get_mut(&key).expect("held entry found");
                    entry.phase = Phase::Kept;
                    out.push(Request::Save { record: Record::Outbox(entry.clone()) });
                    make(domain, env, key, out);
                } else {
                    settled(domain, key, Outcome::Skipped, out);
                }
            }
        }
        Event::Make { key } => make(domain, env, key, out),
        Event::MakeEntry { entry: number } => {
            let mut key = None;
            for (_, entry) in &domain.effects {
                if entry.number == number {
                    key = Some(entry.key);
                }
            }
            if let Some(key) = key {
                make(domain, env, key, out);
            }
        }
        Event::Restore { record } => restore(domain, record),
        Event::ReadAfresh => read_afresh(domain, out),
        Event::Restart => {
            domain.restoring = false;
            domain.restart = Some(crate::RestartStage::Outbox);
            for (key, entry) in &domain.effects {
                match entry.phase {
                    Phase::Kept => out.push(Request::Make { key: *key }),
                    Phase::Sent | Phase::Uncertain => {
                        out.push(Request::System(SystemRequest::Look { key: *key, effect: entry.effect.clone() }));
                    }
                    Phase::Held => {}
                }
            }
        }
        Event::System(system) => match system {
            SystemEvent::Drift { account } => crate::drift_accounts::observed(domain, env, account, out),
            SystemEvent::Service { service, fact, other_hand } => {
                fresh_service(domain, env, service, fact, other_hand, out);
            }
            SystemEvent::Environment { environment, fact, other_hand } => {
                fresh_environment(domain, env, environment, fact, other_hand, out);
            }
            SystemEvent::Pool { pool, quota, used } => fresh_pool(domain, env, pool, quota, used, out),
            SystemEvent::Applied { key, attempt, result } => applied(domain, key, attempt, result, out),
            SystemEvent::Looked { key, result } => looked(domain, env, key, result, out),
        },
    }
    closing_complete(domain, out);
    restart_complete(domain, out);
    crate::client_marks::collect(domain, env, out);
}

// connectors.md 6.2: the connector top arms the procedure's genuine time
// condition. Readiness comes from the supplied fact; deadlines are inclusive
// in decide, so a missing/late fact stalls in the following whole second.
fn provision_deadline(domain: &Domain, state: &ProcedureState) -> Option<u64> {
    if state.phase != ProcedurePhase::WaitingCondition {
        return None;
    }
    match &state.procedure {
        Procedure::Provision { environment, deadline, .. } => match domain.environments.get(environment) {
            Some(Some(fact)) if fact.ready_at <= *deadline => Some(fact.ready_at),
            Some(Some(_) | None) | None => deadline.checked_add(1),
        },
        Procedure::Scale { .. } | Procedure::Remediate { .. } | Procedure::TearDown { .. } => None,
    }
}

/// Rechecks uncertain entries and wakes due procedures at their absolute deadlines.
pub fn fire(domain: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    if domain.restoring {
        return;
    }
    assert!(out.room() >= MAX_OUT, "the root reserves MAX_OUT slots");
    for (key, entry) in &domain.effects {
        if wall_seconds(env) >= entry.deadline && entry.phase == Phase::Uncertain {
            out.push(Request::System(SystemRequest::Look { key: *key, effect: entry.effect.clone() }));
        }
    }
    for _ in 0..env.limits.effects {
        let mut retry = None;
        for (key, entry) in &domain.effects {
            if entry.phase == Phase::Kept && entry.attempt > 0 && wall_seconds(env) >= entry.deadline {
                retry = Some(*key);
                break;
            }
        }
        match retry {
            Some(key) => make(domain, env, key, out),
            None => break,
        }
    }
    for (task, state) in &domain.procedures {
        match provision_deadline(domain, state) {
            Some(at) if wall_seconds(env) >= at => out.push(Request::Wake { task: *task }),
            Some(_) | None => {}
        }
    }
    crate::client_marks::collect(domain, env, out);
}

/// Earliest absolute retry or procedure deadline in the world's clock domain.
#[must_use]
#[expect(clippy::manual_map, reason = "step code uses no closure-taking methods")]
pub fn next_deadline(domain: &Domain) -> Option<Time> {
    if domain.restoring {
        return None;
    }
    let mut soonest = None;
    for (_, entry) in &domain.effects {
        if entry.phase == Phase::Uncertain || (entry.phase == Phase::Kept && entry.attempt > 0) {
            let at = entry.deadline.checked_mul(1_000_000_000)?;
            soonest = Some(match soonest {
                Some(earlier) if earlier < at => earlier,
                Some(_) | None => at,
            });
        }
    }
    for (_, state) in &domain.procedures {
        if let Some(at) = provision_deadline(domain, state) {
            let at = at.checked_mul(1_000_000_000)?;
            soonest = Some(match soonest {
                Some(earlier) if earlier < at => earlier,
                Some(_) | None => at,
            });
        }
    }
    match soonest {
        Some(at) => Some(Time::from_nanos(at)),
        None => None,
    }
}

fn read_afresh(domain: &mut Domain, out: &mut Queue<Request>) {
    domain.restoring = false;
    domain.restart = Some(crate::RestartStage::ReadAfresh);
    domain.application.read_afresh(&mut domain.refresh);
    for (environment, _) in &domain.made {
        domain
            .refresh
            .insert(Resource::Environment(environment.clone()), false)
            .expect("retained creation resources fit");
    }
    // connectors.md, 14: restored owner facts also rebuild the core's
    // volatile resource reports; the root carries this existing Named fact.
    for (task, (project, resources)) in &domain.reliance {
        out.push(Request::Named { task: *task, project: *project, resources: resources.clone() });
        for named in resources {
            domain.refresh.insert(named.resource.clone(), false).expect("restored live resources fit");
        }
    }
    for (_, state) in &domain.procedures {
        let resource = match &state.procedure {
            Procedure::Scale { service, .. } | Procedure::Remediate { service, .. } => {
                Resource::Service(service.clone())
            }
            Procedure::Provision { environment, .. } | Procedure::TearDown { environment, .. } => {
                Resource::Environment(environment.clone())
            }
        };
        domain.refresh.insert(resource, false).expect("restored procedure resources fit");
    }
}

fn restart_complete(domain: &mut Domain, out: &mut Queue<Request>) {
    let mut sent = List::with_capacity(MAX_OUT);
    for (resource, requested) in &domain.refresh {
        if !*requested && out.room() > 1 {
            let request = match resource {
                Resource::Service(service) => SystemRequest::Service { service: service.clone() },
                Resource::Environment(environment) => SystemRequest::Environment { environment: environment.clone() },
                Resource::Pool(pool) | Resource::Quota(pool) => SystemRequest::Pool { pool: pool.clone() },
            };
            out.push(Request::System(request));
            sent.push(resource.clone()).expect("output bound");
        }
    }
    for resource in &sent {
        *domain.refresh.get_mut(resource).expect("live refresh") = true;
    }
    let complete = match domain.restart {
        None => false,
        Some(crate::RestartStage::ReadAfresh) => domain.refresh.is_empty(),
        Some(crate::RestartStage::Outbox) => {
            let mut settled = true;
            for (_, entry) in &domain.effects {
                match entry.phase {
                    Phase::Held => {}
                    Phase::Kept => {
                        if entry.attempt == 0 {
                            settled = false;
                        }
                    }
                    Phase::Sent | Phase::Uncertain => settled = false,
                }
            }
            settled
        }
    };
    if complete {
        let stage = domain.restart.take().expect("completed restart stage");
        out.push(Request::Restarted { stage });
    }
}

impl Domain {
    /// Procedures whose own facts depend on this resource.
    #[must_use]
    pub fn readers(&self, resource: &Resource) -> Box<[u64]> {
        let mut tasks = List::with_capacity(self.procedures.len());
        for (task, state) in &self.procedures {
            let named = match &state.procedure {
                Procedure::Scale { service, .. } | Procedure::Remediate { service, .. } => {
                    Resource::Service(service.clone())
                }
                Procedure::Provision { environment, .. } | Procedure::TearDown { environment, .. } => {
                    Resource::Environment(environment.clone())
                }
            };
            if &named == resource {
                tasks.push(*task).expect("procedure count");
            }
        }
        tasks.into_boxed()
    }
}

fn closing_complete(domain: &mut Domain, out: &mut Queue<Request>) {
    let mut ready = List::with_capacity(MAX_OUT);
    for (task, _) in &domain.closing {
        let mut pending = false;
        for (key, _) in &domain.effects {
            pending |= key.task == *task;
        }
        if !pending && out.room() > 1 {
            ready.push(*task).expect("output bound");
            out.push(Request::Closed { task: *task });
        }
    }
    for task in &ready {
        domain.closing.remove(task);
    }
}

// A creation reserves the place its unanswered close may occupy after teardown.
fn creation_room(domain: &Domain, limits: &Limits, environment: &Environment) -> bool {
    if domain.made.contains_key(environment) {
        return true;
    }
    let mut reserved = domain.made.len().saturating_add(domain.closes.len());
    for (_, entry) in &domain.effects {
        match &entry.effect {
            Effect::CreateEnvironment { .. } => reserved = reserved.saturating_add(1),
            Effect::Restart { .. } | Effect::Scale { .. } | Effect::Rollback { .. } | Effect::TearDown { .. } => {}
        }
    }
    reserved < limits.made
}
