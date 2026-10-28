//! One confirmation keeps the revision read, choices made and the prior card state.
//! Words and authority judgments remain the object's document and the fields;
//! no request key or retry is known. See client/domain/view.md, 18.5.
//!
//! | Confirmation | Event | Next | Emits |
//! |---|---|---|---|
//! | closed | offered act | open; card deciding | nothing |
//! | open | another offered act | old closed; new open | nothing |
//! | open | named revision changes | changed | nothing |
//! | changed/incomplete | confirm | refused in place | nothing |
//! | complete | confirm | closed; card pending | one send |
//! | open | dismiss/navigation | closed; prior card | nothing |
pub(crate) mod proposal;
use crate::{
    Request, Ui, acts,
    fields::Purpose,
    intents::Progress,
    objects::{Body, Card, Object},
};
use alloc::boxed::Box;
use jig_client::contract::{
    ActKind, Admission, Amendment, HoldReason, Limit, ObjectKey, PersonContract, Recipient, Refusal, Revision, Text,
};
use skein_lib::{Env, Id, List, Queue};
/// The person choices, bounded independently of the incoming document.
#[derive(Debug)]
pub struct Choices {
    pub option: Option<u32>,
    pub cannot: bool,
    pub again: Option<bool>,
    pub disposals: List<jig_client::shapes::Delegate>,
    pub change: Option<Amendment>,
    pub release: bool,
    pub priority: Option<u32>,
}
/// Why a confirmation cannot yet send its request.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Missing {
    /// Required words have not been written.
    Words { field: Purpose },
    /// A choice has not been made.
    Choice,
    /// An uncertain effect requires the repeat decision.
    Again,
    /// The amendment has not been supplied.
    Change,
    /// Every live delegate needs a disposal.
    Delegates,
    /// A goal priority has not been supplied.
    Priority,
    /// The object's project context has not arrived.
    Context,
    /// The named revision moved while the confirmation was open.
    Changed { now: Option<Revision> },
    /// The independently named hold changed while reviewing an amendment.
    ChangedHold { now: Option<Revision> },
    /// The engine does not admit the act, or local admission is full.
    Refused { why: Refusal },
}
/// Whether a person supplied the reason in the current amendment form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReasonInput {
    /// No person input has arrived in this confirmation.
    Missing,
    /// A person supplied bounded bytes, including an empty value.
    Supplied,
}
/// The form shown for an operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
    Act,
    CloseChat,
}
/// One act and the revision its card showed when the confirmation opened.
#[derive(Debug)]
pub struct Confirmation {
    /// Ui write generation for fields owned by this new confirmation.
    /// Stable through edits and live-document updates; advances on opening another.
    pub writes: u64,
    pub act: ActKind,
    pub form: Form,
    pub object: Id<Object>,
    pub revision: Option<Revision>,
    /// Hold snapshot for an amendment that also releases.
    pub release_revision: Option<Revision>,
    /// A moved hold stays invalid until the person reviews it again.
    pub release_changed: bool,
    /// A person supplied the reason in this confirmation, including an empty value.
    pub reason_input: ReasonInput,
    pub choices: Choices,
    pub missing: Option<Missing>,
    pub changed: bool,
    pub proposed: Option<Recipient>,
    pub dangerous: bool,
    /// The card to restore if the confirmation closes before sending.
    pub previous: Card,
}
fn clear_fields(domain: &mut Ui) {
    for purpose in [
        Purpose::Reason,
        Purpose::Answer,
        Purpose::Amendment,
        Purpose::Instructions,
        Purpose::NoteDescription,
        Purpose::NoteBody,
    ] {
        if domain.fields.get(purpose).is_some() {
            let empty = Text::new(Box::from([]), 0).expect("empty text fits");
            let cleared = domain.fields.write(purpose, empty);
            assert!(cleared.is_ok(), "an existing field can be cleared");
        }
    }
}
/// Closes and clears fields, restoring a card only while no request exists yet.
pub(crate) fn close(domain: &mut Ui) {
    domain.proposing = None;
    let Some(open) = domain.confirming.take() else {
        return;
    };
    if let Some(object) = domain.objects.get_mut(open.object) {
        match object.card {
            Card::Deciding { request: None, .. } => object.card = open.previous,
            Card::Open
            | Card::Deciding { request: Some(_), .. }
            | Card::NotSent { .. }
            | Card::Refused { .. }
            | Card::Unknown { .. }
            | Card::Leaving { .. } => {}
        }
    }
    clear_fields(domain);
    domain.objects.prune();
    domain.changed();
}
struct NoteFields {
    description: Text,
    body: Text,
}
/// Opens only an act present and admitted in the engine's document.
pub(crate) fn open(domain: &mut Ui, env: &Env<crate::Limits>, id: Id<Object>, act: ActKind, lent: &jig_client::Client) {
    domain.project_page.form.confirmation = None;
    close(domain);
    if domain.blocked || domain.confirmed.is_none() {
        return;
    }
    let Some(object) = domain.borrow(lent).objects().get(id) else {
        return;
    };
    match object.card {
        Card::Open | Card::NotSent { .. } | Card::Refused { .. } => {}
        Card::Deciding { .. } | Card::Unknown { .. } | Card::Leaving { .. } => return,
    }
    let proposed = match if act == ActKind::Reorder && domain.shown_project().is_some() {
        crate::page::project::moving::admission(domain)
    } else {
        acts::offered(&object, act)
    } {
        Admission::Allowed | Admission::Bounded { .. } => None,
        Admission::Proposed { to } => Some(to),
        Admission::Refused(refusal) => {
            domain.objects.get_mut(id).expect("named object exists").card = Card::Refused { refusal };
            domain.changed();
            return;
        }
    };
    let note_fields = match &object.body {
        Some(Body::Note { document, .. }) => {
            if document.cut {
                None
            } else {
                match (document.description.whole(), document.body.whole()) {
                    (Some(description), Some(body)) => Some(NoteFields { description, body }),
                    (None | Some(_), None) | (None, Some(_)) => None,
                }
            }
        }
        Some(
            Body::Historical(_)
            | Body::Chip(_)
            | Body::Task(_)
            | Body::Proposal(_)
            | Body::OwnerProposal(_)
            | Body::Escalation(_)
            | Body::OwnerEscalation(_)
            | Body::Question(_)
            | Body::PersonTask(_)
            | Body::OwnerPersonTask(_)
            | Body::Result(_),
        )
        | None => None,
    };
    let revision = acts::revision(&object, act);
    let previous = object.card.clone();
    if let Some(note_fields) = note_fields
        && act == ActKind::EditNote
    {
        let description = domain.fields.write(Purpose::NoteDescription, note_fields.description);
        let body = domain.fields.write(Purpose::NoteBody, note_fields.body);
        if description.is_err() || body.is_err() {
            clear_fields(domain);
            domain.objects.get_mut(id).expect("named note exists").card =
                Card::Refused { refusal: Refusal::Limit { which: Limit::Fields } };
            domain.changed();
            return;
        }
    }
    let dangerous = jig_client::facts::dangerous(act);
    let choices = Choices {
        option: None,
        cannot: false,
        again: None,
        disposals: List::with_capacity(env.limits.delegates),
        change: None,
        release: false,
        priority: None,
    };
    domain.confirming = Some(Confirmation {
        writes: domain.shown,
        act,
        form: Form::Act,
        object: id,
        revision,
        release_revision: if act == ActKind::Message { acts::hold_revision(&object) } else { None },
        release_changed: false,
        reason_input: ReasonInput::Missing,
        choices,
        missing: None,
        changed: false,
        proposed,
        dangerous,
        previous,
    });
    domain.objects.get_mut(id).expect("named object exists").card =
        Card::Deciding { request: None, progress: Progress::Confirming };
    sync(domain, lent);
    domain.changed();
}
/// Receives a successful person edit without mistaking a domain-cleared blank for input.
pub(crate) fn edited(domain: &mut Ui, field: Purpose) {
    if field == Purpose::Reason
        && let Some(open) = domain.confirming.as_mut()
        && open.act == ActKind::Amend
    {
        open.reason_input = ReasonInput::Supplied;
    }
}
/// Updates only the validity of the revision the act names and the engine's admission.
pub(crate) fn sync(domain: &mut Ui, lent: &jig_client::Client) {
    let Some(open) = domain.confirming.as_ref() else {
        return;
    };
    let Some(object) = domain.borrow(lent).objects().get(open.object) else {
        domain.confirming.as_mut().expect("confirmation exists").missing =
            Some(Missing::Refused { why: Refusal::Unknown });
        return;
    };
    let now = acts::revision(&object, open.act);
    let hold_now = if (open.act == ActKind::Amend && open.choices.release) || open.act == ActKind::Message {
        acts::hold_revision(&object)
    } else {
        None
    };
    let release_changed = open.release_changed || hold_now != open.release_revision;
    let changed = open.changed || now != open.revision || release_changed;
    let proposed = match draft_admission(domain, &object, open) {
        Admission::Proposed { to } => Some(to),
        Admission::Allowed | Admission::Bounded { .. } | Admission::Refused(_) => None,
    };
    let missing = if release_changed {
        Some(Missing::ChangedHold { now: hold_now })
    } else if changed {
        Some(Missing::Changed { now })
    } else {
        missing(domain, open, &object)
    };
    let open = domain.confirming.as_mut().expect("confirmation exists");
    if open.changed != changed || open.missing != missing || open.proposed != proposed {
        open.changed = changed;
        open.release_changed = release_changed;
        open.missing = missing;
        open.proposed = proposed;
        domain.changed();
    }
}
fn empty(domain: &Ui, field: Purpose) -> bool {
    match domain.fields.get(field) {
        Some(field) => field.text.bytes().is_empty(),
        None => true,
    }
}
fn answer_field(open: &Confirmation, object: &Object) -> Result<Option<Purpose>, Missing> {
    Ok(if open.choices.cannot {
        Some(Purpose::Reason)
    } else {
        match &object.body {
            Some(Body::Question(_)) => Some(Purpose::Answer),
            Some(Body::OwnerPersonTask(_)) => None,
            Some(Body::PersonTask(document)) => match &document.contract {
                PersonContract::Words => Some(Purpose::Answer),
                PersonContract::Choice { options } => {
                    let Some(option) = open.choices.option else {
                        return Err(Missing::Choice);
                    };
                    let mut offered = false;
                    for line in options {
                        if line.number == option && line.fields.is_empty() {
                            offered = true;
                        }
                    }
                    if !offered {
                        return Err(Missing::Choice);
                    }
                    None
                }
                PersonContract::Approval { .. } => None,
            },
            Some(
                Body::Historical(_)
                | Body::Chip(_)
                | Body::Task(_)
                | Body::Proposal(_)
                | Body::OwnerProposal(_)
                | Body::Escalation(_)
                | Body::OwnerEscalation(_)
                | Body::Result(_)
                | Body::Note { .. },
            )
            | None => return Err(Missing::Refused { why: Refusal::NotOffered }),
        }
    })
}
fn delegates_missing(open: &Confirmation, object: &Object) -> Option<Missing> {
    let Some(body) = object.body.as_ref() else {
        return Some(Missing::Refused { why: Refusal::NotOffered });
    };
    let task = match body {
        Body::Task(task) => task,
        Body::Historical(_)
        | Body::Chip(_)
        | Body::Proposal(_)
        | Body::OwnerProposal(_)
        | Body::Escalation(_)
        | Body::OwnerEscalation(_)
        | Body::Question(_)
        | Body::PersonTask(_)
        | Body::OwnerPersonTask(_)
        | Body::Result(_)
        | Body::Note { .. } => return Some(Missing::Refused { why: Refusal::NotOffered }),
    };
    if task.delegates.len() != open.choices.disposals.len() {
        return Some(Missing::Delegates);
    }
    for delegate in &task.delegates {
        let mut chosen = false;
        for disposal in &open.choices.disposals {
            let task = disposal.task;
            if task == delegate.chip.task {
                chosen = true;
            }
        }
        if !chosen {
            return Some(Missing::Delegates);
        }
    }
    None
}

fn release_missing(open: &Confirmation, object: &Object) -> Option<Missing> {
    let uncertain = match &object.body {
        Some(Body::Task(document)) => {
            let Some(hold) = &document.hold else {
                return Some(Missing::Refused { why: Refusal::NotOffered });
            };
            Some(hold.uncertain())
        }
        Some(Body::OwnerEscalation(document)) => {
            let Some(hold) = &document.task.hold else {
                return Some(Missing::Refused { why: Refusal::NotOffered });
            };
            Some(hold.uncertain())
        }
        Some(Body::Escalation(document)) => Some(match &document.hold.reason {
            HoldReason::EffectUncertain { .. } => true,
            HoldReason::Tries { .. }
            | HoldReason::Stalled { .. }
            | HoldReason::Dependency { .. }
            | HoldReason::Drift { .. }
            | HoldReason::EffectFailed { .. }
            | HoldReason::Budget { .. }
            | HoldReason::Deadline { .. }
            | HoldReason::Stopped { .. }
            | HoldReason::Resource { .. }
            | HoldReason::Pool { .. }
            | HoldReason::Procedure { .. } => false,
        }),
        Some(
            Body::Historical(_)
            | Body::Chip(_)
            | Body::Proposal(_)
            | Body::OwnerProposal(_)
            | Body::Question(_)
            | Body::PersonTask(_)
            | Body::OwnerPersonTask(_)
            | Body::Result(_)
            | Body::Note { .. },
        )
        | None => None,
    };
    let Some(uncertain) = uncertain else {
        return Some(Missing::Refused { why: Refusal::NotOffered });
    };
    if uncertain && open.choices.again.is_none() {
        return Some(Missing::Again);
    }
    None
}

fn draft_admission(domain: &Ui, object: &Object, open: &Confirmation) -> Admission {
    if domain.proposing == Some(open.object) {
        return match proposal::recipient(object) {
            Some(to) => Admission::Proposed { to },
            None => Admission::Refused(Refusal::NotOffered),
        };
    }
    if open.act == ActKind::Amend
        && open.choices.release
        && let Admission::Refused(why) = acts::offered(object, ActKind::Release)
    {
        return Admission::Refused(why);
    }
    let offer = acts::offered(object, open.act);
    let Some(change) = &open.choices.change else {
        return offer;
    };
    match open.act {
        ActKind::Amend => {
            let before = match &object.body {
                Some(Body::Task(document)) => &document.authority,
                Some(Body::OwnerEscalation(document)) => &document.task.authority,
                Some(Body::Escalation(document)) => &document.steering.authority,
                Some(
                    Body::Historical(_)
                    | Body::Chip(_)
                    | Body::Proposal(_)
                    | Body::OwnerProposal(_)
                    | Body::Question(_)
                    | Body::PersonTask(_)
                    | Body::OwnerPersonTask(_)
                    | Body::Result(_)
                    | Body::Note { .. },
                )
                | None => return Admission::Refused(Refusal::NotOffered),
            };
            jig_client::contract::admission::amendment(&offer, change, before)
        }
        ActKind::Withdraw
        | ActKind::Message
        | ActKind::Answer
        | ActKind::Take
        | ActKind::HandBack
        | ActKind::Accept
        | ActKind::Reject
        | ActKind::PassUp
        | ActKind::Release
        | ActKind::LeaveHeld
        | ActKind::Stop
        | ActKind::Cancel
        | ActKind::Move
        | ActKind::Reorder
        | ActKind::Seen
        | ActKind::EditNote
        | ActKind::DeleteNote
        | ActKind::Adopt
        | ActKind::Rename
        | ActKind::ChangePolicy
        | ActKind::SetRoles
        | ActKind::MakeService
        | ActKind::SetPool => offer,
    }
}

fn owner_answer_missing(
    domain: &Ui,
    open: &Confirmation,
    document: &jig_client::contract::OwnerPersonTask,
) -> Option<Missing> {
    if crate::objects::person_task::answer(document, open.choices.option, &domain.fields).is_some() {
        return None;
    }
    Some(match &document.contract {
        jig_client::contract::TaskContractDocument::Verdict { choices, .. } => {
            let mut selectable = false;
            if let Some(option) = open.choices.option {
                for choice in choices {
                    if choice.code == option && choice.fields.is_empty() {
                        selectable = true;
                    }
                }
            }
            if selectable { Missing::Words { field: Purpose::Reason } } else { Missing::Choice }
        }
        jig_client::contract::TaskContractDocument::Report { .. } => Missing::Words { field: Purpose::Answer },
        jig_client::contract::TaskContractDocument::Approval { .. }
        | jig_client::contract::TaskContractDocument::Change { .. } => Missing::Refused { why: Refusal::NotOffered },
    })
}
fn missing(domain: &Ui, open: &Confirmation, object: &Object) -> Option<Missing> {
    if object.project.is_none() {
        return Some(Missing::Context);
    }
    if let Some(document) = crate::objects::person_task::owner_facts(object)
        && document.project().is_none()
    {
        return Some(Missing::Context);
    }
    match if open.act == ActKind::Reorder && domain.shown_project().is_some() {
        crate::page::project::moving::admission(domain)
    } else {
        draft_admission(domain, object, open)
    } {
        Admission::Allowed | Admission::Proposed { .. } | Admission::Bounded { .. } => {}
        Admission::Refused(why) => return Some(Missing::Refused { why }),
    }
    if open.act == ActKind::Answer
        && !open.choices.cannot
        && let Some(document) = crate::objects::person_task::owner_facts(object)
    {
        return owner_answer_missing(domain, open, document);
    }
    if open.form == Form::CloseChat {
        return delegates_missing(open, object);
    }
    let field = match open.act {
        ActKind::Reject | ActKind::LeaveHeld | ActKind::Cancel | ActKind::Move => Some(Purpose::Reason),
        ActKind::Answer => match answer_field(open, object) {
            Ok(field) => field,
            Err(missing) => return Some(missing),
        },
        ActKind::Message => {
            let task = match object.key {
                ObjectKey::Task(task) => task,
                ObjectKey::Proposal(_)
                | ObjectKey::Escalation(_)
                | ObjectKey::Question { .. }
                | ObjectKey::PersonTask(_)
                | ObjectKey::Result(_)
                | ObjectKey::Note { .. } => return Some(Missing::Refused { why: Refusal::NotOffered }),
            };
            Some(Purpose::Composer { task })
        }
        ActKind::Release => {
            if let Some(missing) = release_missing(open, object) {
                return Some(missing);
            }
            None
        }
        ActKind::Amend => return amendment_missing(domain, open),
        ActKind::Reorder => {
            if open.choices.priority.is_none() {
                return Some(Missing::Priority);
            }
            None
        }
        ActKind::Take
        | ActKind::HandBack
        | ActKind::Accept
        | ActKind::PassUp
        | ActKind::Stop
        | ActKind::DeleteNote
        | ActKind::Withdraw => None,
        ActKind::EditNote => {
            if let Some(Body::Note { document, .. }) = &object.body
                && (document.cut || document.description.cut || document.body.cut)
            {
                return Some(Missing::Change);
            }
            if domain.fields.get(Purpose::NoteDescription).is_none() {
                return Some(Missing::Words { field: Purpose::NoteDescription });
            }
            if domain.fields.get(Purpose::NoteBody).is_none() {
                return Some(Missing::Words { field: Purpose::NoteBody });
            }
            None
        }
        ActKind::Seen
        | ActKind::Adopt
        | ActKind::Rename
        | ActKind::ChangePolicy
        | ActKind::SetRoles
        | ActKind::MakeService
        | ActKind::SetPool => return Some(Missing::Refused { why: Refusal::NotOffered }),
    };
    if let Some(field) = field
        && empty(domain, field)
    {
        return Some(Missing::Words { field });
    }
    None
}
/// Changes one choice; the form checks membership again before sending.
pub(crate) fn choose(domain: &mut Ui, option: u32) {
    if let Some(open) = &mut domain.confirming {
        open.choices.option = Some(option);
        open.choices.cannot = false;
        domain.changed();
    }
}
/// Selects the failure answer that every person task contract permits.
pub(crate) fn cannot(domain: &mut Ui) {
    if let Some(open) = &mut domain.confirming {
        open.choices.cannot = true;
        domain.changed();
    }
}
/// Records the person's decision about an unrecoverable uncertain effect.
pub(crate) fn again(domain: &mut Ui, again: bool) {
    if let Some(open) = &mut domain.confirming {
        open.choices.again = Some(again);
        domain.changed();
    }
}
/// Changes a disposal without growing past the configured live-delegate bound.
pub(crate) fn dispose(domain: &mut Ui, task: u64, take_over: bool) {
    let Some(open) = &mut domain.confirming else {
        return;
    };
    let value = jig_client::shapes::Delegate {
        task,
        fate: if take_over { jig_client::shapes::Fate::Move } else { jig_client::shapes::Fate::Cancel },
    };
    for index in 0..open.choices.disposals.len() {
        let existing = open.choices.disposals.get_mut(index).expect("bounded disposal index exists");
        let named = existing.task;
        if named == task {
            *existing = value;
            domain.changed();
            return;
        }
    }
    if open.choices.disposals.push(value).is_err() {
        open.missing = Some(Missing::Delegates);
    }
    domain.changed();
}
/// Keeps the caller's decoded, bounded amendment for the one request.
pub(crate) fn change(domain: &mut Ui, change: Amendment) {
    if let Some(open) = &mut domain.confirming {
        open.choices.change = Some(change);
        domain.changed();
    }
}
/// Records whether the one write also releases the held task.
pub(crate) fn write_release(domain: &mut Ui, release: bool) {
    if let Some(open) = &mut domain.confirming
        && open.act == ActKind::Message
    {
        open.choices.release = release;
        domain.changed();
    }
}
/// Records a goal's new place, leaving admission to the engine.
pub(crate) fn priority(domain: &mut Ui, priority: u32) {
    if let Some(open) = &mut domain.confirming
        && open.act == ActKind::Reorder
    {
        open.choices.priority = Some(priority);
        domain.changed();
    }
}
/// Checks the recalled revision and complete form before asking for one send.
pub(crate) fn send(domain: &mut Ui, out: &mut Queue<Request>, lent: &jig_client::Client) {
    sync(domain, lent);
    let Some(open) = domain.confirming.as_ref() else {
        return;
    };
    if open.missing.is_some() || domain.blocked || domain.confirmed.is_none() {
        return;
    }
    let Some(object) = domain.borrow(lent).objects().get(open.object) else {
        return;
    };
    let ask = if open.act == ActKind::Reorder && domain.shown_project().is_some() {
        crate::page::project::moving::ask(domain, &object, open.choices.priority.expect("complete priority"), lent)
    } else if open.form == Form::CloseChat {
        let Some(me) = domain.frame(lent).document else { return };
        let Some(project) = acts::request_project(&object) else { return };
        let ObjectKey::Task(task) = object.key else { return };
        let reason = match acts::words(&domain.fields, Purpose::Reason) {
            Some(reason) => reason,
            None => Text::new(Box::from([]), domain.text_limit).expect("empty reason fits"),
        };
        match jig_client::shapes::close_chat(me, project, task, open.choices.disposals.as_slice(), reason) {
            Ok(ask) => Some(ask),
            Err(_) => {
                domain.confirming.as_mut().expect("open confirmation").missing =
                    Some(Missing::Refused { why: Refusal::NotOffered });
                domain.changed();
                return;
            }
        }
    } else {
        acts::ask(&object, open, &domain.fields)
    };
    let Some(ask) = ask else { return };
    let key = object.key;
    let act = open.act;
    if crate::intents::send_card(domain, key, act, Box::new(ask), out, lent).is_some() {
        close(domain);
    } else if let Some(open) = &mut domain.confirming {
        open.missing = Some(Missing::Refused { why: Refusal::Limit { which: Limit::Requests } });
    }
}

/// Open the composition of two independently offered operations.
pub(crate) fn amend_release(domain: &mut Ui, env: &Env<crate::Limits>, id: Id<Object>, lent: &jig_client::Client) {
    let hold = {
        let Some(object) = domain.borrow(lent).objects().get(id) else { return };
        if let Admission::Refused(_) = acts::offered(&object, ActKind::Release) {
            return;
        }
        acts::hold_revision(&object)
    };
    let Some(hold) = hold else { return };
    open(domain, env, id, ActKind::Amend, lent);
    if let Some(open) = domain.confirming.as_mut()
        && open.object == id
        && open.act == ActKind::Amend
    {
        open.choices.release = true;
        open.release_revision = Some(hold);
    }
    sync(domain, lent);
}

fn amendment_missing(domain: &Ui, open: &Confirmation) -> Option<Missing> {
    if open.choices.change.is_none() {
        return Some(Missing::Change);
    }
    if open.choices.release && open.release_revision.is_none() {
        return Some(Missing::Refused { why: Refusal::NotOffered });
    }
    if open.reason_input == ReasonInput::Missing || domain.fields.get(Purpose::Reason).is_none() {
        return Some(Missing::Words { field: Purpose::Reason });
    }
    None
}

/// Offer the shape's compound only from the task's cancel offer and requesting party.
pub(crate) fn can_close(client: &crate::Borrowed<'_>, task: &jig_client::contract::Task) -> bool {
    let Some(me) = client.frame().document else { return false };
    if !task.flags.conversation
        || task.requester != jig_client::contract::OwnerRequester::Person(me.party.number)
        || jig_client::shapes::template(me, task.project, jig_client::shapes::CHAT).is_none()
    {
        return false;
    }
    match jig_client::facts::offer(task.acts.as_slice(), ActKind::Cancel) {
        Some(Admission::Allowed | Admission::Bounded { .. } | Admission::Proposed { .. }) => true,
        Some(Admission::Refused(_)) | None => false,
    }
}
pub(crate) fn close_chat(domain: &mut Ui, env: &Env<crate::Limits>, id: Id<Object>, lent: &jig_client::Client) {
    let borrowed = domain.borrow(lent);
    let Some(object) = borrowed.objects().get(id) else { return };
    let Some(Body::Task(task)) = object.body else { return };
    if !can_close(&borrowed, &task) {
        return;
    }
    open(domain, env, id, ActKind::Cancel, lent);
    if let Some(open) = &mut domain.confirming {
        open.form = Form::CloseChat;
    }
    sync(domain, lent);
}
