//! Durable card answers and local retention expiry, kept as distinct facts.
//! Refusals preserve the engine's judgment; a moved object is read afresh,
//! while an unknown request is never sent automatically again. See client.md,
//! 6, objects.md, 9, and contract.md, 4.
//!
//! | Card event | State | Effect |
//! |---|---|---|
//! | done, still waiting | open with answer | none |
//! | take or hand back, shown | open with answer | fresh whole object read |
//! | done, stopped waiting | leaving with outcome | linger |
//! | refused moved | refused with facts | fresh read |
//! | refused otherwise | refused with facts | none |
//! | retention expired | unknown, inspect address | notice if hidden |
use crate::{
    Request, Ui,
    objects::{Card, Object, Source},
};
use jig_client::contract::{ActKind, Answer, Limit, ObjectKey, Refusal};
use skein_lib::{Id, Queue};
/// The useful next actions from a factual refusal, never an authority judgment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Recovery {
    pub look_again: bool,
    pub change_ask: bool,
}
/// Whether the party should inspect fresh facts or change the request they made.
#[must_use]
pub fn recovery(object: &Object) -> Recovery {
    let mut recovery = Recovery { look_again: false, change_ask: false };
    match &object.card {
        Card::Unknown { .. } => recovery.look_again = true,
        Card::Refused { refusal } => match refusal {
            Refusal::Stale(_) | Refusal::Moved { .. } => recovery.look_again = true,
            Refusal::Forgotten(_)
            | Refusal::Full(_)
            | Refusal::Circle { .. }
            | Refusal::Dependency { .. }
            | Refusal::Kept { .. }
            | Refusal::Authority { .. }
            | Refusal::Limit { .. }
            | Refusal::Llm { .. }
            | Refusal::Account { .. } => {
                recovery.change_ask = true;
            }
            Refusal::Malformed { .. }
            | Refusal::Taken { .. }
            | Refusal::NotWaiting
            | Refusal::NoFurther
            | Refusal::Role { .. }
            | Refusal::Ended { .. }
            | Refusal::Unknown
            | Refusal::NameTaken
            | Refusal::NotOffered
            | Refusal::NoEscalationHolder
            | Refusal::SignInRedirect { .. }
            | Refusal::AccountMove { .. }
            | Refusal::Provider
            | Refusal::KeyConflict => {}
        },
        Card::Open | Card::NotSent { .. } | Card::Deciding { .. } | Card::Leaving { .. } => {}
    }
    recovery
}
/// Reads the current object, joining a read already in flight and preserving its refusal.
pub fn look_again(domain: &mut Ui, id: Id<Object>, out: &mut Queue<Request>, lent: &jig_client::Client) {
    let Some(object) = domain.borrow(lent).objects().get(id) else {
        return;
    };
    if !object.shown || !recovery(&object).look_again {
        return;
    }
    let what = super::document_read(object.key);
    if domain.handles.pending(&what) {
        return;
    }
    if domain.handles.read(Source::Card(id), what, out).is_none() {
        crate::notices::kept::show(domain, crate::notices::kept::Kind::Refused(Limit::Reads));
    }
    domain.changed();
}
pub(crate) fn answered(
    domain: &mut Ui,
    key: ObjectKey,
    act: ActKind,
    answer: Answer,
    out: &mut Queue<Request>,
    lent: &jig_client::Client,
) {
    let Some(id) = domain.objects.find(key) else {
        crate::notices::kept::show(domain, crate::notices::kept::Kind::Answer { object: key, answer });
        return;
    };
    if !domain.borrow(lent).objects().get(id).expect("named card exists").shown {
        crate::notices::kept::show(domain, crate::notices::kept::Kind::Answer { object: key, answer: answer.clone() });
    }
    domain.objects.get_mut(id).expect("named card exists").answer = Some(answer.clone());
    match answer {
        Answer::Refused { refusal, .. } => {
            domain.objects.get_mut(id).expect("named card exists").card = Card::Refused { refusal };
            if recovery(&domain.borrow(lent).objects().get(id).expect("named card exists")).look_again {
                look_again(domain, id, out, lent);
            }
        }
        Answer::Done(outcome) => {
            if stops_waiting(key, act, &outcome) {
                let removed = domain.leave_answer(id, outcome);
                assert!(removed, "answered card exists");
            } else {
                domain.objects.get_mut(id).expect("named card exists").card = Card::Open;
                if act == ActKind::Take || act == ActKind::HandBack {
                    current_facts(domain, id, out, lent);
                }
            }
        }
    }
    domain.objects.prune();
    domain.changed();
}
fn current_facts(domain: &mut Ui, id: Id<Object>, out: &mut Queue<Request>, lent: &jig_client::Client) {
    let object = domain.borrow(lent).objects().get(id).expect("answered card exists");
    if !object.shown {
        return;
    }
    let what = super::document_read(object.key);
    // A read started before the durable answer may still describe the old offers.
    domain.handles.forget_read(&what, out);
    if domain.handles.read(Source::Card(id), what, out).is_none() {
        crate::notices::kept::show(domain, crate::notices::kept::Kind::Refused(Limit::Reads));
    }
}
fn stops_waiting(key: ObjectKey, act: ActKind, outcome: &jig_client::contract::Outcome) -> bool {
    if act == ActKind::Take || act == ActKind::HandBack {
        return false;
    }
    if act == ActKind::DeleteNote {
        return true;
    }
    super::stops_waiting(key, outcome)
}
pub(crate) fn unknown(domain: &mut Ui, key: ObjectKey, act: ActKind, lent: &jig_client::Client) {
    let Some(id) = domain.objects.find(key) else {
        crate::notices::kept::show(domain, crate::notices::kept::Kind::Unknown { object: key, act });
        return;
    };
    if !domain.borrow(lent).objects().get(id).expect("named card exists").shown {
        crate::notices::kept::show(domain, crate::notices::kept::Kind::Unknown { object: key, act });
    }
    let object = domain.objects.get_mut(id).expect("named card exists");
    object.card = Card::Unknown { act, look: super::document_read(key) };
    object.answer = None;
    domain.objects.prune();
    domain.changed();
}
