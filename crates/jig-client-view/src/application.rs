//! Jig objects shown by an application's page remain jig's working-set objects.
//! One window names them; their chips and deciding documents remain live through
//! jig's watches and reads. No application value is held here (root.md, section 7).
use crate::{
    Request, Ui,
    handles::Owner,
    objects::{Body, Source},
};
use jig_client::contract::{self, Delivery, ObjectKey, Part, Read, ReadResult, ReadValue, Subject};
use skein_lib::{Env, List, Queue};
pub(crate) const WINDOW: u16 = u16::MAX;
fn source(_domain: &Ui) -> Source {
    Source::Window { number: WINDOW }
}
pub(crate) fn show(domain: &mut Ui, env: &Env<crate::Limits>, objects: List<ObjectKey>, out: &mut Queue<Request>) {
    if domain.confirmed.is_none() || domain.link != jig_client::Link::Live {
        return;
    }
    let source = source(domain);
    let mut unchanged = true;
    let mut count = 0_u32;
    for (key, _) in domain.objects.ids() {
        if domain.objects.named(*key, source) {
            count = count.checked_add(1).expect("named objects fit their capacity");
            if !objects.as_slice().contains(key) {
                unchanged = false;
            }
        }
    }
    if unchanged && count == objects.len() {
        return;
    }
    close(domain, out);
    let missed = domain.objects.replace(source, objects.as_slice());
    if missed > 0 {
        crate::page::routing::refuse(domain, contract::Limit::Objects);
    }
    let mut tasks = List::with_capacity(env.limits.rows);
    let mut deciding = false;
    for key in &objects {
        if !domain.objects.named(*key, source) {
            continue;
        }
        read(domain, *key, out);
        match key {
            ObjectKey::Task(task) => {
                if tasks.push(*task).is_err() {
                    crate::page::routing::refuse(domain, contract::Limit::Objects);
                }
            }
            ObjectKey::Proposal(_)
            | ObjectKey::Escalation(_)
            | ObjectKey::Question { .. }
            | ObjectKey::PersonTask(_)
            | ObjectKey::Result(_)
            | ObjectKey::Note { .. } => deciding = true,
        }
    }
    if !tasks.is_empty() {
        open(domain, Subject::Chips { tasks }, out);
    }
    if deciding {
        open(domain, Subject::Inbox { window: env.limits.rows }, out);
    }
    domain.changed();
}
fn open(domain: &mut Ui, subject: Subject, out: &mut Queue<Request>) {
    if domain.hold_watch(subject, Owner::Application, out).is_none() {
        crate::page::routing::refuse(domain, contract::Limit::Watches);
    }
}
fn close(domain: &mut Ui, out: &mut Queue<Request>) {
    for _ in 0..domain.handles.watches.len() {
        let mut found = None;
        for (handle, held) in &domain.handles.watches {
            if held.owner == Owner::Application {
                found = Some(*handle);
                break;
            }
        }
        let Some(handle) = found else {
            break;
        };
        domain.release(handle, out);
    }
}
fn read(domain: &mut Ui, key: ObjectKey, out: &mut Queue<Request>) {
    let what = crate::page::routing::document_read(key);
    if !domain.handles.pending(&what) && domain.handles.read(source(domain), what, out).is_none() {
        crate::page::routing::refuse(domain, contract::Limit::Reads);
    }
}
pub(crate) fn delivery(
    domain: &mut Ui,
    env: &Env<crate::Limits>,
    delivery: Delivery,
    out: &mut Queue<Request>,
    lent: &jig_client::Client,
) {
    match delivery {
        Delivery::OwnerProposal(proposal) => {
            let key = ObjectKey::Proposal(proposal.key);
            if domain.objects.named(key, source(domain))
                && put(domain, env, key, Body::OwnerProposal(proposal), out, lent, false)
            {
                domain.changed();
            }
        }
        Delivery::Chips(chips) => {
            for chip in chips.into_boxed() {
                let key = ObjectKey::Task(chip.task);
                if domain.objects.named(key, source(domain))
                    && put(domain, env, key, Body::Chip(chip), out, lent, false)
                {
                    domain.changed();
                }
            }
        }
        Delivery::Inbox(_) | Delivery::OwnerInbox(_) => {
            let mut keys = List::with_capacity(env.limits.objects);
            for (key, _) in domain.objects.ids() {
                assert!(keys.push(*key).is_ok(), "objects fit their registry");
            }
            for key in &keys {
                if domain.objects.named(*key, source(domain)) {
                    read(domain, *key, out);
                }
            }
        }
        Delivery::Me(_)
        | Delivery::OwnerProject(_)
        | Delivery::Task(_)
        | Delivery::Tree(_)
        | Delivery::OwnerTree(_)
        | Delivery::Tasks(_)
        | Delivery::Run(_)
        | Delivery::RunState(_)
        | Delivery::Settings(_)
        | Delivery::Deployment(_)
        | Delivery::OwnerDeployment(_)
        | Delivery::OwnerAccounts(_) => {}
    }
}
pub(crate) fn read_result(
    domain: &mut Ui,
    env: &Env<crate::Limits>,
    what: Read,
    result: ReadResult,
    out: &mut Queue<Request>,
    lent: &jig_client::Client,
) {
    let Some(key) = key(&what) else {
        return;
    };
    if !domain.objects.named(key, source(domain)) {
        return;
    }
    match result {
        ReadResult::None(why) => match why {
            contract::ReadNone::SignedOut => crate::ui::signed_out(domain, out, lent),
            contract::ReadNone::TooLinked { joins } => {
                crate::notices::kept::show(domain, crate::notices::kept::Kind::TooLinked { joins });
            }
            contract::ReadNone::TooLarge { .. }
            | contract::ReadNone::Malformed
            | contract::ReadNone::Unknown
            | contract::ReadNone::Gone
            | contract::ReadNone::Refused
            | contract::ReadNone::Busy
            | contract::ReadNone::Unreachable => {
                crate::page::shown::status(domain, None, None, true);
                crate::page::routing::refuse(domain, contract::Limit::Reads);
            }
        },
        ReadResult::Value(value) => {
            let stored = match value {
                ReadValue::Task(document) => put(domain, env, key, Body::Task(document), out, lent, true),
                ReadValue::Proposal(document) => put(domain, env, key, Body::Proposal(document), out, lent, true),
                ReadValue::Historical(document) => put(domain, env, key, Body::Historical(document), out, lent, true),
                ReadValue::OwnerProposal(document) => {
                    put(domain, env, key, Body::OwnerProposal(document), out, lent, true)
                }
                ReadValue::Escalation(document) => put(domain, env, key, Body::Escalation(document), out, lent, true),
                ReadValue::OwnerEscalation(document) => {
                    put(domain, env, key, Body::OwnerEscalation(document), out, lent, true)
                }
                ReadValue::Question(document) => put(domain, env, key, Body::Question(document), out, lent, true),
                ReadValue::PersonTask(document) => put(domain, env, key, Body::PersonTask(document), out, lent, true),
                ReadValue::OwnerPersonTask(document) => {
                    put(domain, env, key, Body::OwnerPersonTask(document), out, lent, true)
                }
                ReadValue::Result(document) => put(domain, env, key, Body::Result(document), out, lent, true),
                ReadValue::Tasks(_)
                | ReadValue::Me(_)
                | ReadValue::Ended(_)
                | ReadValue::History(_)
                | ReadValue::Activity(_)
                | ReadValue::Turn(_)
                | ReadValue::Inbox(_)
                | ReadValue::OwnerInbox(_)
                | ReadValue::Spend(_)
                | ReadValue::OwnerEnded(_)
                | ReadValue::OwnerSpend(_)
                | ReadValue::OwnerProject(_)
                | ReadValue::Project(_)
                | ReadValue::Notes(_)
                | ReadValue::Settings(_)
                | ReadValue::Setting(_)
                | ReadValue::Revision(_)
                | ReadValue::Revisions(_)
                | ReadValue::Deployment(_)
                | ReadValue::OwnerDeployment(_) => false,
            };
            if stored {
                domain.changed();
            }
        }
    }
}
fn key(what: &Read) -> Option<ObjectKey> {
    match what {
        Read::Task { task } => Some(ObjectKey::Task(*task)),
        Read::Proposal { proposal } => Some(ObjectKey::Proposal(*proposal)),
        Read::Escalation { task } => Some(ObjectKey::Escalation(*task)),
        Read::Question { task, message } => Some(ObjectKey::Question { task: *task, message: *message }),
        Read::PersonTask { task } => Some(ObjectKey::PersonTask(*task)),
        Read::Result { task } => Some(ObjectKey::Result(*task)),
        Read::Tasks { .. }
        | Read::Me
        | Read::Ended { .. }
        | Read::History { .. }
        | Read::Activity { .. }
        | Read::Turn { .. }
        | Read::Inbox { .. }
        | Read::Spend { .. }
        | Read::Project { .. }
        | Read::Notes { .. }
        | Read::Settings
        | Read::Setting(_)
        | Read::Revision(_)
        | Read::Revisions(_)
        | Read::Deployment => None,
    }
}
/// Stores the whole source and registers its bounded supplied parts only if accepted.
/// The caller marks its accepted body change; a refused body leaves prior facts intact.
#[must_use]
pub(crate) fn put(
    domain: &mut Ui,
    env: &Env<crate::Limits>,
    key: ObjectKey,
    body: Body,
    out: &mut Queue<Request>,
    lent: &jig_client::Client,
    retain: bool,
) -> bool {
    let parts = parts(&body, env.limits.parts);
    if parts.full {
        crate::page::routing::refuse(domain, contract::Limit::Objects);
    }
    let project = match &body {
        Body::Task(task) => Some(task.project),
        Body::OwnerEscalation(value) => Some(value.task.project),
        Body::Proposal(proposal) => Some(proposal.project),
        Body::OwnerProposal(proposal) => Some(proposal.project),
        Body::OwnerPersonTask(person) => person.project(),
        Body::Note { project, .. } => Some(*project),
        Body::Chip(chip) => match chip.details.as_ref() {
            contract::ChipPhase::Owner { details, .. } => Some(details.project),
            contract::ChipPhase::Waiting { .. }
            | contract::ChipPhase::Procedure { .. }
            | contract::ChipPhase::Held { .. }
            | contract::ChipPhase::Closing(_)
            | contract::ChipPhase::Active(_)
            | contract::ChipPhase::Idle { .. }
            | contract::ChipPhase::Starting
            | contract::ChipPhase::Running { .. }
            | contract::ChipPhase::WaitingForWords { .. }
            | contract::ChipPhase::BackingOff { .. }
            | contract::ChipPhase::Person { .. }
            | contract::ChipPhase::Ended { .. } => object_project(domain, key, lent),
        },
        Body::Historical(_) | Body::Escalation(_) | Body::Question(_) | Body::PersonTask(_) | Body::Result(_) => {
            object_project(domain, key, lent)
        }
    };
    if (!retain && domain.objects.find(key).is_some()) || (retain && domain.objects.put(key, body)) {
        for part in parts.values.into_boxed() {
            if let Some(project) = project {
                let scope = owner_scope(&part, contract::PartScope::Project(project));
                out.push(Request::ShowPart { part, scope });
            } else if let Some(scope) = scope(domain, &part, lent) {
                out.push(Request::ShowPart { part, scope });
            }
        }

        true
    } else {
        false
    }
}
struct Parts {
    values: List<Part>,
    full: bool,
}
fn parts(body: &Body, limit: u32) -> Parts {
    let mut parts = Parts { values: List::with_capacity(limit), full: false };
    match body {
        Body::Chip(chip) => chip_parts(&mut parts, chip),
        Body::Task(task) => {
            chip_parts(&mut parts, &task.chip);
            for resource in &task.resources {
                if let Some(part) = resource.part() {
                    push(&mut parts, part.clone());
                }
            }
            if let Some(context) = &task.context {
                append(&mut parts, context.parts.as_slice());
            }
        }
        Body::OwnerEscalation(value) => {
            // Native accounts already belong to the shared task parts list.
            if owner_hold(&value.task.chip).is_none() {
                chip_parts(&mut parts, &value.task.chip);
            }
            append(
                &mut parts,
                crate::page::task::parts(&value.task, crate::address::TaskSection::Overview, limit.saturating_add(1))
                    .as_slice(),
            );
        }
        Body::Proposal(proposal) => match &proposal.action {
            contract::ProposalAction::Effect { part } => push(&mut parts, part.clone()),
            contract::ProposalAction::Batch { .. }
            | contract::ProposalAction::Widen { .. }
            | contract::ProposalAction::Amend { .. } => {}
        },
        Body::OwnerProposal(proposal) => match &proposal.action {
            contract::OwnerProposalAction::Effect { part, .. }
            | contract::OwnerProposalAction::ProcedureEffect { part, .. } => {
                if let Some(part) = owner_effect(part) {
                    push(&mut parts, part);
                }
            }
            contract::OwnerProposalAction::Settings(_)
            | contract::OwnerProposalAction::Batch(_)
            | contract::OwnerProposalAction::Amend { .. }
            | contract::OwnerProposalAction::Widen { .. }
            | contract::OwnerProposalAction::Release { .. } => {}
        },
        Body::Escalation(escalation) => {
            chip_parts(&mut parts, &escalation.task);
            hold_parts(&mut parts, &escalation.hold.reason);
        }
        Body::Question(question) => chip_parts(&mut parts, &question.task),
        Body::PersonTask(person) => chip_parts(&mut parts, &person.task),
        Body::OwnerPersonTask(person) => chip_parts(&mut parts, &person.task),
        Body::Result(result) => {
            chip_parts(&mut parts, &result.task);
            match &result.value {
                Some(contract::ResultValue::Connector { part, .. }) => push(&mut parts, part.clone()),
                Some(
                    contract::ResultValue::Report(_)
                    | contract::ResultValue::Verdict { .. }
                    | contract::ResultValue::OwnerVerdict { .. }
                    | contract::ResultValue::Change { .. }
                    | contract::ResultValue::Choice { .. }
                    | contract::ResultValue::Approved { .. }
                    | contract::ResultValue::Failure(_)
                    | contract::ResultValue::Words(_),
                )
                | None => {}
            }
        }
        Body::Historical(_) | Body::Note { .. } => {}
    }
    parts
}
fn chip_parts(parts: &mut Parts, chip: &contract::Chip) {
    match chip.details.as_ref() {
        contract::ChipPhase::Waiting { hold, .. } => {
            if let Some(part) = hold {
                push(parts, part.clone());
            }
        }
        contract::ChipPhase::Procedure { panel } => push(parts, panel.clone()),
        contract::ChipPhase::Held { reason, was, .. } => {
            hold_parts(parts, reason);
            if let Some(contract::HeldRuntime::Closing(closing)) = was {
                closing_parts(parts, closing);
            }
        }
        contract::ChipPhase::Closing(closing) => closing_parts(parts, closing),
        contract::ChipPhase::Owner { .. } => {
            if let Some(part) = owner_hold(chip) {
                push(parts, part);
            }
        }
        contract::ChipPhase::Active(_)
        | contract::ChipPhase::Idle { .. }
        | contract::ChipPhase::Starting
        | contract::ChipPhase::Running { .. }
        | contract::ChipPhase::WaitingForWords { .. }
        | contract::ChipPhase::BackingOff { .. }
        | contract::ChipPhase::Person { .. }
        | contract::ChipPhase::Ended { .. } => {}
    }
}
// The account is an explicit owner reference, independent of the hold's origin
// (client/domain/contract.md, 5.1; objects.md, 3).
pub(crate) fn owner_hold(chip: &contract::Chip) -> Option<Part> {
    let details = match chip.details.as_ref() {
        contract::ChipPhase::Owner { details, .. } => details,
        contract::ChipPhase::Waiting { .. }
        | contract::ChipPhase::Procedure { .. }
        | contract::ChipPhase::Held { .. }
        | contract::ChipPhase::Closing(_)
        | contract::ChipPhase::Active(_)
        | contract::ChipPhase::Idle { .. }
        | contract::ChipPhase::Starting
        | contract::ChipPhase::Running { .. }
        | contract::ChipPhase::WaitingForWords { .. }
        | contract::ChipPhase::BackingOff { .. }
        | contract::ChipPhase::Person { .. }
        | contract::ChipPhase::Ended { .. } => return None,
    };
    let account = details.hold.as_ref()?.account.as_ref()?;
    match account.kind {
        contract::OwnerPartKind::Hold => match &account.key {
            contract::OwnerPartKey::Number(task) => Some(Part::Hold { connector: account.connector, task: *task }),
            contract::OwnerPartKey::Path(_) => None,
        },
        contract::OwnerPartKind::Resource
        | contract::OwnerPartKind::Pool
        | contract::OwnerPartKind::Result
        | contract::OwnerPartKind::Procedure
        | contract::OwnerPartKind::Effect
        | contract::OwnerPartKind::Outbox => None,
    }
}
// The connector's effect number is independent of the jig proposal maker
// (client/domain/root.md, 6; objects.md, 4).
/// Preserve the raw effect subject namespace for a supplied owner part.
/// Maker-keyed effects remain distinct; other owner part shapes do not
/// acquire a guessed subject.
#[must_use]
pub fn owner_effect(part: &contract::OwnerPart) -> Option<Part> {
    match part.kind {
        contract::OwnerPartKind::Effect => match &part.key {
            contract::OwnerPartKey::Number(proposal) => {
                Some(Part::OwnerEffect { connector: part.connector, proposal: *proposal })
            }
            contract::OwnerPartKey::Path(_) => None,
        },
        contract::OwnerPartKind::Resource
        | contract::OwnerPartKind::Pool
        | contract::OwnerPartKind::Result
        | contract::OwnerPartKind::Procedure
        | contract::OwnerPartKind::Outbox
        | contract::OwnerPartKind::Hold => None,
    }
}
fn closing_parts(parts: &mut Parts, closing: &contract::Closing) {
    if let Some(connectors) = &closing.connectors {
        for connector in connectors {
            push(parts, Part::Connector { connector: *connector });
        }
    }
}
fn hold_parts(parts: &mut Parts, reason: &contract::HoldReason) {
    match reason {
        contract::HoldReason::Drift { part }
        | contract::HoldReason::EffectFailed { part }
        | contract::HoldReason::EffectUncertain { part }
        | contract::HoldReason::Pool { part }
        | contract::HoldReason::Procedure { part } => push(parts, part.clone()),
        contract::HoldReason::Resource { resource, .. } => push(parts, resource.clone()),
        contract::HoldReason::Tries { .. }
        | contract::HoldReason::Stalled { .. }
        | contract::HoldReason::Dependency { .. }
        | contract::HoldReason::Budget { .. }
        | contract::HoldReason::Deadline { .. }
        | contract::HoldReason::Stopped { .. } => {}
    }
}
fn push(parts: &mut Parts, part: Part) {
    if parts.values.push(part).is_err() {
        parts.full = true;
    }
}
fn append(parts: &mut Parts, values: &[Part]) {
    for part in values {
        push(parts, part.clone());
    }
}

fn object_project(domain: &Ui, key: ObjectKey, lent: &jig_client::Client) -> Option<u32> {
    domain.borrow(lent).objects().get(domain.objects.find(key)?)?.project
}
/// An outbox reference names its owner; other parts retain their enclosing document scope.
pub(crate) fn owner_scope(part: &Part, enclosing: contract::PartScope) -> contract::PartScope {
    match part {
        Part::Outbox { scope, .. } => *scope,
        Part::Connector { .. }
        | Part::Resource { .. }
        | Part::Kind { .. }
        | Part::Panel { .. }
        | Part::Result { .. }
        | Part::Effect { .. }
        | Part::OwnerEffect { .. }
        | Part::Hold { .. }
        | Part::Project { .. }
        | Part::Deployment { .. } => enclosing,
    }
}
#[expect(clippy::manual_map, reason = "step code matches optional values explicitly")]
pub(crate) fn scope(domain: &Ui, part: &Part, lent: &jig_client::Client) -> Option<contract::PartScope> {
    match part {
        Part::Outbox { scope, .. } => Some(*scope),
        Part::Project { project, .. } => Some(contract::PartScope::Project(*project)),
        Part::Deployment { .. } | Part::Connector { .. } | Part::Resource { .. } | Part::Kind { .. } => {
            Some(contract::PartScope::Deployment)
        }
        Part::Panel { task, .. } | Part::Result { task, .. } | Part::Hold { task, .. } => {
            let project = match object_project(domain, ObjectKey::Task(*task), lent) {
                Some(project) => Some(project),
                None => current_project(domain, lent),
            };
            match project {
                Some(project) => Some(contract::PartScope::Project(project)),
                None => None,
            }
        }
        Part::Effect { proposal, .. } => {
            let project = match object_project(domain, ObjectKey::Proposal(*proposal), lent) {
                Some(project) => Some(project),
                None => current_project(domain, lent),
            };
            match project {
                Some(project) => Some(contract::PartScope::Project(project)),
                None => None,
            }
        }
        Part::OwnerEffect { .. } => match current_project(domain, lent) {
            Some(project) => Some(contract::PartScope::Project(project)),
            None => None,
        },
    }
}
fn current_project(domain: &Ui, lent: &jig_client::Client) -> Option<u32> {
    if let Some(project) = domain.shown_project() {
        return Some(project);
    }
    if let Some(task) = domain.shown_task() {
        return object_project(domain, ObjectKey::Task(task), lent);
    }
    domain.shown_inbox().or(domain.shown_conversations()).flatten()
}
