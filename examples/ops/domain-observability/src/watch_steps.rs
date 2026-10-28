//! A watch owns its triage and lead associations (`examples.md`, 5 and 7.1).
//! A triage returns closed facts; no result explanation is read. Every selected
//! step is saved before its delegate outcome, and lead associations outlive W.
//!
//! | State | Input | Next state | Emits |
//! | --- | --- | --- | --- |
//! | no triage | alert wake | triage selected | save, delegate |
//! | triage selected | delegate outcome | triage bound | save |
//! | triage bound | its noise verdict | no triage | erase, wait |
//! | triage bound | its incident verdict | lead selected | incident, save, delegate |
//! | lead selected | its delegate outcome | lead bound | incident, save |
//! | lead bound | conflicting outcome | unchanged | refusal |
use crate::{Domain, Limits, Record, RecordKey, Request, Service};
use alloc::boxed::Box;
use skein_lib::{Env, Queue};

/// The triage's closed result, with no follow-up delegation.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum TriageVerdict {
    /// The triage found no incident.
    Noise,
    /// These alerts concern this service at this severity.
    Incident { alerts: Box<[u64]>, service: Service, severity: u8 },
}

/// Offered procedure news, with result explanations excluded at translation.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum WatchMessage {
    /// The standing watch has a pending alert batch.
    News,
    /// A delegate returned a closed triage result.
    Triage { child: u64, verdict: TriageVerdict },
    /// A delegate supplied malformed closed triage fields.
    InvalidTriage { child: u64 },
    /// A delegate ended without a triage verdict.
    Ended { child: u64 },
}

/// The one application-owned delegate chosen by a procedure step.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum DelegationKind {
    /// Only this retained delegate may decide this explicit alert batch.
    Triage { alerts: Box<[u64]> },
    /// Only this step's outcome may set the named incident's lead.
    Lead { incident: u64 },
}

/// Durable procedure-step association, separate from a watch's live interests.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Delegation {
    /// Owning W procedure.
    pub task: u64,
    /// The core-issued step that requested one delegate.
    pub step: u64,
    /// Scope retained even after W ends.
    pub project: u32,
    /// The owner-selected purpose of this step.
    pub kind: DelegationKind,
    /// Absent until the same-decision core outcome.
    pub child: Option<u64>,
    /// The exact core outcome was recorded, including a refused creation.
    pub settled: bool,
}

/// A closed reason the owner refused an association.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum AssociationRefusal {
    /// The supplied outcome names no retained owner step.
    UnknownStep,
    /// The step or incident already names another association.
    Conflict,
    /// A verdict exceeds its retained batch or supplied service facts.
    Verdict,
    /// A bounded owner table has no room.
    Full,
    /// The configured delegate envelope is absent.
    Template,
}

/// The owner chooses a single delegate, waits, or holds on a refused input.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum WatchDecision {
    /// Append the explicit batch to the configured triage's words.
    Triage { delegate: Box<[u8]>, alerts: Box<[u64]> },
    /// The lead uses its configured charter, budget and spec unchanged.
    Lead { delegate: Box<[u8]> },
    /// The owner has no action due.
    Wait,
    /// A malformed or conflicting association requires attention.
    Hold,
}

pub(crate) fn capacity(limits: &Limits) -> Option<u32> {
    crate::incidents::capacity(limits)?.checked_add(limits.watches)
}

/// Offered news includes each bounded owner association and each live topic interest.
#[must_use]
pub fn message_capacity(limits: &Limits) -> Option<u32> {
    capacity(limits)?.checked_add(limits.subscriptions)?.checked_add(1)
}

fn refused(task: u64, step: u64, reason: AssociationRefusal, out: &mut Queue<Request>) {
    out.push(Request::AssociationRefused { task, step, reason });
    out.push(Request::WatchDecision { task, step, decision: WatchDecision::Hold });
}

fn triage(domain: &Domain, task: u64) -> Option<Delegation> {
    for (_, value) in &domain.delegations {
        if value.task == task {
            match &value.kind {
                DelegationKind::Triage { .. } => return Some(value.clone()),
                DelegationKind::Lead { .. } => {}
            }
        }
    }
    None
}

fn save(domain: &mut Domain, value: Delegation, out: &mut Queue<Request>) {
    domain.delegations.insert((value.task, value.step), value.clone()).expect("association room checked");
    out.push(Request::Save { record: Record::Delegation(value) });
}

fn valid_verdict(domain: &Domain, limits: &Limits, association: &Delegation, verdict: &TriageVerdict) -> bool {
    match verdict {
        TriageVerdict::Noise => true,
        TriageVerdict::Incident { alerts, service, .. } => {
            let retained = match &association.kind {
                DelegationKind::Triage { alerts } => alerts,
                DelegationKind::Lead { .. } => return false,
            };
            if alerts.is_empty()
                || u32::try_from(alerts.len()).unwrap_or(u32::MAX) > limits.alerts_per_batch
                || !crate::domain::service_valid(service, limits)
            {
                return false;
            }
            for (index, alert) in alerts.iter().enumerate() {
                if !retained.contains(alert) {
                    return false;
                }
                for earlier in alerts.iter().take(index) {
                    if earlier == alert {
                        return false;
                    }
                }
                match domain.alerts.get(&(association.project, *alert)) {
                    Some(account) if account.alert.service == *service => {}
                    Some(_) | None => return false,
                }
            }
            true
        }
    }
}

/// A watch consumes only the result of its retained triage child.
pub(crate) fn step(
    domain: &mut Domain,
    env: &Env<Limits>,
    task: u64,
    step: u64,
    batch: u64,
    messages: Box<[WatchMessage]>,
    out: &mut Queue<Request>,
) {
    if u32::try_from(messages.len()).unwrap_or(u32::MAX) > message_capacity(&env.limits).expect("watch news bound") {
        return refused(task, step, AssociationRefusal::Full, out);
    }
    let Some(watch) = domain.watches.get(&task).cloned() else { return };
    if domain.delegations.contains_key(&(task, step)) {
        return refused(task, step, AssociationRefusal::Conflict, out);
    }
    let mut result = None;
    let mut news = false;
    let association = triage(domain, task);
    for message in messages {
        match message {
            WatchMessage::News => news = true,
            WatchMessage::Triage { child, verdict } => match &association {
                Some(value) if value.child == Some(child) => {
                    if result.is_some() || !valid_verdict(domain, &env.limits, value, &verdict) {
                        return refused(task, step, AssociationRefusal::Verdict, out);
                    }
                    result = Some(verdict);
                }
                Some(_) | None => {}
            },
            WatchMessage::InvalidTriage { child } => match &association {
                Some(value) if value.child == Some(child) => {
                    return refused(task, step, AssociationRefusal::Verdict, out);
                }
                Some(_) | None => {}
            },
            WatchMessage::Ended { child } => match &association {
                Some(value) if value.child == Some(child) => {
                    domain.delegations.remove(&(task, value.step));
                    out.push(Request::Erase { key: RecordKey::Delegation { task, step: value.step } });
                }
                Some(_) | None => {}
            },
        }
    }
    match result {
        Some(verdict) => {
            let association = association.expect("only the retained triage produced a result");
            domain.delegations.remove(&(task, association.step));
            out.push(Request::Erase { key: RecordKey::Delegation { task, step: association.step } });
            match verdict {
                TriageVerdict::Noise => {
                    out.push(Request::WatchDecision { task, step, decision: WatchDecision::Wait });
                    if !watch.pending.is_empty() {
                        out.push(Request::WakeProcedure { task });
                    }
                }
                TriageVerdict::Incident { alerts, service, severity } => {
                    let delegate = match domain.lead_templates.get(&watch.template) {
                        Some(value) if !value.is_empty() => value.clone(),
                        Some(_) | None => return refused(task, step, AssociationRefusal::Template, out),
                    };
                    let number = match crate::incidents::open(domain, env, &watch, alerts, service, severity, out) {
                        Ok(number) => number,
                        Err(reason) => return refused(task, step, reason, out),
                    };
                    save(
                        domain,
                        Delegation {
                            task,
                            step,
                            project: watch.project,
                            kind: DelegationKind::Lead { incident: number },
                            child: None,
                            settled: false,
                        },
                        out,
                    );
                    out.push(Request::WatchDecision { task, step, decision: WatchDecision::Lead { delegate } });
                    if !watch.pending.is_empty() {
                        out.push(Request::WakeProcedure { task });
                    }
                }
            }
        }
        None => make_triage(domain, env, watch, step, batch, news, out),
    }
}

fn make_triage(
    domain: &mut Domain,
    env: &Env<Limits>,
    watch: crate::Watch,
    step: u64,
    batch: u64,
    news: bool,
    out: &mut Queue<Request>,
) {
    let task = watch.task;
    if triage(domain, task).is_some() || (!news && watch.pending.is_empty()) || watch.last_batch == Some(batch) {
        out.push(Request::WatchDecision { task, step, decision: WatchDecision::Wait });
        return;
    }
    if domain.delegations.len() >= capacity(&env.limits).expect("association bound") {
        return refused(task, step, AssociationRefusal::Full, out);
    }
    let delegate = match domain.templates.get(&watch.template) {
        Some(value) if !value.is_empty() => value.clone(),
        Some(_) | None => return refused(task, step, AssociationRefusal::Template, out),
    };
    let alerts = watch.pending;
    if alerts.is_empty() {
        out.push(Request::WatchDecision { task, step, decision: WatchDecision::Wait });
        return;
    }
    let watch = domain.watches.get_mut(&task).expect("live watch");
    watch.pending = Box::new([]);
    watch.last_batch = Some(batch);
    out.push(Request::Save { record: Record::Watch(watch.clone()) });
    let project = watch.project;
    save(
        domain,
        Delegation {
            task,
            step,
            project,
            kind: DelegationKind::Triage { alerts: alerts.clone() },
            child: None,
            settled: false,
        },
        out,
    );
    out.push(Request::WatchDecision { task, step, decision: WatchDecision::Triage { delegate, alerts } });
}

/// Only the exact retained step's core-issued outcome may name a lead.
pub(crate) fn outcome(domain: &mut Domain, task: u64, step: u64, child: Option<u64>, out: &mut Queue<Request>) {
    let Some(value) = domain.delegations.get(&(task, step)).cloned() else {
        out.push(Request::AssociationRefused { task, step, reason: AssociationRefusal::UnknownStep });
        return;
    };
    if value.settled {
        if value.child != child {
            out.push(Request::AssociationRefused { task, step, reason: AssociationRefusal::Conflict });
        }
        return;
    }
    if child == Some(0) {
        out.push(Request::AssociationRefused { task, step, reason: AssociationRefusal::Conflict });
        return;
    }
    match &value.kind {
        DelegationKind::Triage { .. } => {
            if child.is_none() {
                domain.delegations.remove(&(task, step));
                out.push(Request::Erase { key: RecordKey::Delegation { task, step } });
                return;
            }
        }
        DelegationKind::Lead { incident } => {
            let Some(incident) = domain.incidents.get_mut(&(value.project, *incident)) else {
                out.push(Request::AssociationRefused { task, step, reason: AssociationRefusal::UnknownStep });
                return;
            };
            if incident.lead.is_some() {
                out.push(Request::AssociationRefused { task, step, reason: AssociationRefusal::Conflict });
                return;
            }
            incident.lead = child;
            out.push(Request::Save { record: Record::Incident(incident.clone()) });
        }
    }
    let mut value = value;
    value.child = child;
    value.settled = true;
    save(domain, value, out);
}
