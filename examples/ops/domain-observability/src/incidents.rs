//! Incident history belongs to observability (`examples.md`, sections 3 and 8).
use crate::{Alert, Domain, Fact, Limits, Record, Request, Service};
use alloc::boxed::Box;
use skein_lib::{Env, List, Queue, Wall};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum IncidentState {
    Alerting,
    Resolved,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct IncidentHealth {
    pub observed: u64,
    pub healthy_replicas: u32,
    pub errors: u32,
    pub error_rate_percent: u8,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct IncidentPoint {
    /// When this owner observed the change, rather than a claimed system actor's time.
    pub at: Wall,
    /// None is the alert's arrival; Some is a later service observation.
    pub health: Option<IncidentHealth>,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Incident {
    pub project: u32,
    pub number: u64,
    pub watch: u64,
    /// The task created by the retained lead step, absent until its outcome.
    pub lead: Option<u64>,
    /// The explicit alert set covered by the triage verdict.
    pub alerts: Box<[u64]>,
    pub raised: Wall,
    pub service: Service,
    pub severity: u8,
    pub state: IncidentState,
    pub timeline: List<IncidentPoint>,
    pub cut: bool,
}

pub(crate) fn capacity(limits: &Limits) -> Option<u32> {
    limits.watches.checked_mul(limits.alerts_per_batch)
}

fn append(incident: &mut Incident, point: IncidentPoint) {
    if incident.timeline.room() == 0 {
        // Keep the newest bounded suffix and state explicitly that its prefix was cut.
        for index in 1..incident.timeline.len() {
            let next = *incident.timeline.get(index).expect("bounded timeline index");
            *incident
                .timeline
                .get_mut(index.checked_sub(1).expect("index starts at one"))
                .expect("preceding timeline index") = next;
        }
        let last = incident.timeline.len().checked_sub(1).expect("nonzero timeline capacity");
        *incident.timeline.get_mut(last).expect("last timeline entry") = point;
        incident.cut = true;
    } else {
        incident.timeline.push(point).expect("timeline room checked");
    }
}

/// An alert has an account before a triage names an incident.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct AlertAccount {
    pub project: u32,
    pub watch: u64,
    pub raised: Wall,
    pub alert: Alert,
    pub state: IncidentState,
}

pub(crate) fn raised(domain: &mut Domain, env: &Env<Limits>, task: u64, alert: &Alert, out: &mut Queue<Request>) {
    let Some(watch) = domain.watches.get(&task) else { return };
    let key = (watch.project, alert.number);
    if domain.alerts.contains_key(&key) || domain.alerts.len() >= capacity(&env.limits).expect("alert bound") {
        return;
    }
    let account = AlertAccount {
        project: watch.project,
        watch: task,
        raised: env.wall,
        alert: alert.clone(),
        state: IncidentState::Alerting,
    };
    domain.alerts.insert(key, account.clone()).expect("alert capacity checked");
    out.push(Request::Save { record: Record::Alert(account) });
}

pub(crate) fn open(
    domain: &mut Domain,
    env: &Env<Limits>,
    watch: &crate::Watch,
    alerts: Box<[u64]>,
    service: Service,
    severity: u8,
    out: &mut Queue<Request>,
) -> Result<u64, crate::AssociationRefusal> {
    // The primary covered alert names this incident account; it never names a lead task.
    let number = *alerts.first().ok_or(crate::AssociationRefusal::Verdict)?;
    let project = watch.project;
    let task = watch.task;
    let key = (project, number);
    let raised = domain.alerts.get(&key).ok_or(crate::AssociationRefusal::Verdict)?.raised;
    if domain.incidents.contains_key(&key) {
        return Err(crate::AssociationRefusal::Conflict);
    }
    if domain.incidents.len() >= capacity(&env.limits).ok_or(crate::AssociationRefusal::Full)? {
        return Err(crate::AssociationRefusal::Full);
    }
    let mut incident = Incident {
        project,
        number,
        watch: task,
        lead: None,
        alerts,
        raised,
        service,
        severity,
        state: IncidentState::Alerting,
        timeline: List::with_capacity(env.limits.samples_per_service.max(1)),
        cut: false,
    };
    append(&mut incident, IncidentPoint { at: raised, health: None });
    domain.incidents.insert(key, incident.clone()).expect("incident capacity checked");
    out.push(Request::Save { record: Record::Incident(incident) });
    Ok(number)
}

pub(crate) fn observed(
    domain: &mut Domain,
    env: &Env<Limits>,
    service: &Service,
    fact: &Fact,
    out: &mut Queue<Request>,
) {
    let health = IncidentHealth {
        observed: fact.observed,
        healthy_replicas: fact.healthy_replicas,
        errors: fact.errors,
        error_rate_percent: fact.error_rate_percent,
    };
    if health.healthy_replicas > 0 && health.errors == 0 && health.error_rate_percent == 0 {
        resolve_alerts(domain, env, service, out);
    }

    let mut keys = List::with_capacity(capacity(&env.limits).expect("incident bound"));
    for (key, incident) in &domain.incidents {
        if incident.service == *service && incident.state == IncidentState::Alerting {
            keys.push(*key).expect("bounded incidents");
        }
    }
    for key in &keys {
        let incident = domain.incidents.get_mut(key).expect("selected incident");
        let same = match incident.timeline.last() {
            Some(point) => point.health == Some(health),
            None => false,
        };
        if !same {
            append(incident, IncidentPoint { at: env.wall, health: Some(health) });
            if health.healthy_replicas > 0 && health.errors == 0 && health.error_rate_percent == 0 {
                incident.state = IncidentState::Resolved;
            }
            out.push(Request::Save { record: Record::Incident(incident.clone()) });
        }
    }
}

fn resolve_alerts(domain: &mut Domain, env: &Env<Limits>, service: &Service, out: &mut Queue<Request>) {
    let mut keys = List::with_capacity(capacity(&env.limits).expect("alert bound"));
    for (key, account) in &domain.alerts {
        if account.alert.service == *service && account.state == IncidentState::Alerting {
            keys.push(*key).expect("bounded alert accounts");
        }
    }
    for key in &keys {
        let account = domain.alerts.get_mut(key).expect("selected account");
        account.state = IncidentState::Resolved;
        out.push(Request::Save { record: Record::Alert(account.clone()) });
    }
}
