//! Connector-scoped deployment flights and retained outcomes (people.md, 5.3).
use super::{Domain, Pending, answer_expiry, answer_reservations, deadline};
use crate::{Act, ActKey, ActKind, Again, Key, LimitKind, Limits, Outcome, Refusal, Request, Stored};
use alloc::boxed::Box;
use skein_lib::{Env, Id, List, Queue, ReplyTo, Time, Wall};

#[derive(Debug)]
pub(crate) struct PendingAct {
    key: ActKey,
    act: Act,
    replies: List<ReplyTo>,
}
#[derive(Debug)]
pub(crate) struct Acted {
    act: Act,
    outcome: Outcome,
    at: Wall,
    due: Time,
}

impl Acted {
    pub(super) const fn at(&self) -> Wall {
        self.at
    }
}

pub(super) fn valid(limits: &Limits, act: &Act) -> Result<(), Refusal> {
    match act {
        Act::Close { task, reason, .. } => {
            if *task == 0 {
                return Err(Refusal::Unknown);
            }
            if reason.len() > usize::try_from(limits.words).expect("u32 fits usize") {
                return Err(Refusal::Limit { which: LimitKind::Words });
            }
        }
        Act::Adopt { adoption, .. } => {
            if !super::valid_adoption(limits, adoption) {
                return Err(Refusal::Limit { which: LimitKind::Resource });
            }
        }
        Act::MakeProject { name, .. } => {
            if name.len() > crate::NAME_BYTES {
                return Err(Refusal::Limit { which: LimitKind::Words });
            }
        }
        Act::AddKey { .. } | Act::RemoveAccount { .. } => {}
    }
    Ok(())
}
fn kind(act: &Act) -> ActKind {
    match act {
        Act::MakeProject { .. } => ActKind::MakeProject,
        Act::Close { .. } => ActKind::Close,
        Act::AddKey { .. } => ActKind::AddKey,
        Act::RemoveAccount { .. } => ActKind::RemoveAccount,
        Act::Adopt { .. } => ActKind::Adopt,
    }
}
fn answer(to: ReplyTo, outcome: Outcome, out: &mut Queue<Request>) {
    out.push(Request::Acted { to, outcome });
}

pub(super) fn admit(
    domain: &mut Domain,
    env: &Env<Limits>,
    to: ReplyTo,
    key: ActKey,
    act: Act,
    out: &mut Queue<Request>,
) {
    if !domain.ready() {
        return answer(to, Outcome::Again(Again::NotReady), out);
    }
    if let Err(why) = valid(&env.limits, &act) {
        return answer(to, Outcome::Refused(why), out);
    }
    if let Some(saved) = domain.acted.get(&key)
        && (answer_expiry(saved.at, &env.limits) <= env.wall || saved.due <= env.now)
    {
        drop(domain.acted.remove(&key));
        domain.act_alarms.cancel(key);
        out.push(Request::Erase { key: Key::Acted(key) });
    }
    if let Some(saved) = domain.acted.get(&key) {
        let outcome = if saved.act == act { saved.outcome.clone() } else { Outcome::Refused(Refusal::KeyConflict) };
        return answer(to, outcome, out);
    }
    if let Some(id) = domain.act_flights.get(&key).copied() {
        let Pending::Act(flight) = domain.pending.get_mut(id).expect("indexed act is live") else {
            unreachable!("act index")
        };
        if flight.act != act {
            return answer(to, Outcome::Refused(Refusal::KeyConflict), out);
        }
        if flight.replies.len() >= env.limits.waiters {
            return busy(domain, to, out);
        }
        flight.replies.push(to).expect("waiter room checked");
        return;
    }
    let mut granted = false;
    for grant in &domain.grants {
        if grant.connector == key.connector && grant.may.contains(&kind(&act)) {
            granted = true;
        }
    }
    if !granted {
        return answer(to, Outcome::Refused(Refusal::NotOffered), out);
    }
    if domain.owners.is_empty() {
        return answer(to, Outcome::Refused(Refusal::Role { held: None }), out);
    }
    // Permanent subject refusals also need a reserved answer place.
    if domain.answers.len().saturating_add(domain.acted.len()).saturating_add(answer_reservations(domain))
        >= env.limits.requests
    {
        return busy(domain, to, out);
    }
    if let Act::Adopt { adoption, .. } = &act
        && adoption.resource.connector != key.connector
    {
        let outcome = Outcome::Refused(Refusal::NotOffered);
        save(domain, env, key, act, outcome.clone(), env.wall, out);
        return answer(to, outcome, out);
    }
    if domain.pending.is_full() {
        return busy(domain, to, out);
    }
    let mut replies = List::with_capacity(env.limits.waiters);
    replies.push(to).expect("positive waiter capacity");
    let id = domain
        .pending
        .insert(Pending::Act(PendingAct { key, act: act.clone(), replies }))
        .expect("pending room checked");
    assert!(domain.act_flights.insert(key, id) == Ok(None), "one connector key per flight");
    out.push(Request::Act { request: id.token(), connector: key.connector, act: Box::new(act) });
}

fn busy(domain: &mut Domain, to: ReplyTo, out: &mut Queue<Request>) {
    // The existing capacity feed names party keys; count unsampled deployment pressure
    // without inventing a person or changing that client document.
    domain.room_refusals_lost = domain.room_refusals_lost.saturating_add(1);
    answer(to, Outcome::Again(Again::Busy), out);
}

pub(super) fn decided(
    domain: &mut Domain,
    env: &Env<Limits>,
    id: Id<Pending>,
    outcome: Outcome,
    out: &mut Queue<Request>,
) {
    let Pending::Act(flight) = domain.pending.get_mut(id).expect("one decision per act flight") else {
        unreachable!("act decision")
    };
    let key = flight.key;
    let act = flight.act.clone();
    let replies = core::mem::replace(&mut flight.replies, List::with_capacity(0));
    domain.pending.retire(id);
    assert!(domain.act_flights.remove(&key) == Some(id), "act has one index");
    if permanent(&outcome) {
        save(domain, env, key, act, outcome.clone(), env.wall, out);
    }
    for to in replies.into_boxed() {
        answer(to, outcome.clone(), out);
    }
}

fn save(
    domain: &mut Domain,
    env: &Env<Limits>,
    key: ActKey,
    act: Act,
    outcome: Outcome,
    at: Wall,
    out: &mut Queue<Request>,
) {
    out.push(Request::Save { record: Stored::Acted { key, act: act.clone(), outcome: outcome.clone(), at } });
    retain(domain, env, key, act, outcome, at);
}
fn retain(domain: &mut Domain, env: &Env<Limits>, key: ActKey, act: Act, outcome: Outcome, at: Wall) {
    let due = deadline(env, answer_expiry(at, &env.limits));
    assert!(domain.acted.insert(key, Acted { act, outcome, at, due }).is_ok(), "answer room reserved");
    assert!(domain.act_alarms.arm(key, due).is_ok(), "one alarm per act");
}
pub(super) fn restore(
    domain: &mut Domain,
    env: &Env<Limits>,
    key: ActKey,
    act: Act,
    outcome: Outcome,
    at: Wall,
    out: &mut Queue<Request>,
) {
    if answer_expiry(at, &env.limits) <= env.wall {
        out.push(Request::Erase { key: Key::Acted(key) });
    } else {
        retain(domain, env, key, act, outcome, at);
    }
}

fn permanent(outcome: &Outcome) -> bool {
    match outcome {
        Outcome::Again(_) | Outcome::SignedOut => false,
        Outcome::Compound { .. }
        | Outcome::CompoundRefused { .. }
        | Outcome::MarkedSeen { .. }
        | Outcome::Applied { .. }
        | Outcome::NoteEdited { .. }
        | Outcome::ServiceMade { .. }
        | Outcome::Adopted { .. }
        | Outcome::RolesSet { .. }
        | Outcome::PolicyChanged { .. }
        | Outcome::Renamed { .. }
        | Outcome::ProjectMade { .. }
        | Outcome::SettingsChanged { .. }
        | Outcome::MovedToNewest { .. }
        | Outcome::PoolSet { .. }
        | Outcome::Said { .. }
        | Outcome::QuestionAnswered { .. }
        | Outcome::Amended { .. }
        | Outcome::Moved { .. }
        | Outcome::Stopped { .. }
        | Outcome::Cancelled { .. }
        | Outcome::Placed { .. }
        | Outcome::Lifted { .. }
        | Outcome::Released { .. }
        | Outcome::Created { .. }
        | Outcome::Proposed { .. }
        | Outcome::Withdrawn { .. }
        | Outcome::PersonTaken { .. }
        | Outcome::PersonHandedBack { .. }
        | Outcome::PersonAnswered { .. }
        | Outcome::EscalationDecided { .. }
        | Outcome::ProposalDecided { .. }
        | Outcome::AccountSignInStarted { .. }
        | Outcome::AccountProgramDeclared { .. }
        | Outcome::AccountKeyKept { .. }
        | Outcome::AccountMovingOut { .. }
        | Outcome::AccountFinishing { .. }
        | Outcome::AccountMovedIn { .. }
        | Outcome::AccountChanged
        | Outcome::AccountRemoved { .. }
        | Outcome::AccountSignedOut { .. }
        | Outcome::Refused(_) => true,
    }
}

pub(super) fn valid_outcome(limits: &Limits, act: &Act, outcome: &Outcome) -> bool {
    if let Outcome::Refused(_) = outcome {
        return true;
    }
    match act {
        Act::Close { project, .. } => match outcome {
            Outcome::Compound { project: answered, operations, first_message } => {
                answered == project && *operations != 0 && *operations <= limits.operations && *first_message != 0
            }
            Outcome::CompoundRefused { at, .. } => *at < limits.operations,
            Outcome::Again(_)
            | Outcome::SignedOut
            | Outcome::MarkedSeen { .. }
            | Outcome::Applied { .. }
            | Outcome::NoteEdited { .. }
            | Outcome::ServiceMade { .. }
            | Outcome::Adopted { .. }
            | Outcome::RolesSet { .. }
            | Outcome::PolicyChanged { .. }
            | Outcome::Renamed { .. }
            | Outcome::ProjectMade { .. }
            | Outcome::SettingsChanged { .. }
            | Outcome::MovedToNewest { .. }
            | Outcome::PoolSet { .. }
            | Outcome::Said { .. }
            | Outcome::QuestionAnswered { .. }
            | Outcome::Amended { .. }
            | Outcome::Moved { .. }
            | Outcome::Stopped { .. }
            | Outcome::Cancelled { .. }
            | Outcome::Placed { .. }
            | Outcome::Lifted { .. }
            | Outcome::Released { .. }
            | Outcome::Created { .. }
            | Outcome::Proposed { .. }
            | Outcome::Withdrawn { .. }
            | Outcome::PersonTaken { .. }
            | Outcome::PersonHandedBack { .. }
            | Outcome::PersonAnswered { .. }
            | Outcome::EscalationDecided { .. }
            | Outcome::ProposalDecided { .. }
            | Outcome::AccountSignInStarted { .. }
            | Outcome::AccountProgramDeclared { .. }
            | Outcome::AccountKeyKept { .. }
            | Outcome::AccountMovingOut { .. }
            | Outcome::AccountFinishing { .. }
            | Outcome::AccountMovedIn { .. }
            | Outcome::AccountChanged
            | Outcome::AccountRemoved { .. }
            | Outcome::AccountSignedOut { .. }
            | Outcome::Refused(_) => false,
        },
        Act::AddKey { provider, join, digest } => super::valid_answer_shape(
            &crate::Ask::AccountChange {
                holder: crate::AccountHolder::Deployment,
                change: crate::AccountChange::AddKey { provider: *provider, join: *join, digest: *digest },
            },
            outcome.clone(),
        ),
        Act::RemoveAccount { account } => super::valid_answer_shape(
            &crate::Ask::AccountChange {
                holder: crate::AccountHolder::Deployment,
                change: crate::AccountChange::Remove { account: *account },
            },
            outcome.clone(),
        ),
        Act::MakeProject { name, policy, .. } => {
            super::valid_answer_shape(&crate::Ask::MakeProject { name: name.clone(), policy: *policy }, outcome.clone())
        }
        Act::Adopt { project, adoption } => super::valid_answer_shape(
            &crate::Ask::Adopt { project: *project, adoption: adoption.clone() },
            outcome.clone(),
        ),
    }
}

#[cfg(test)]
#[path = "../tests/act_charges.rs"]
mod tests;
