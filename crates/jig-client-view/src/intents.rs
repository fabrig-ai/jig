//! Intents remain until the root table gives their one durable answer.
//! Joining, progress and the terminal belong here; object policy is jig 21.
//! See client/domain/client.md, 6, and client/domain/root.md, 11.
use alloc::boxed::Box;
use jig_client::contract::Ask;
use skein_lib::Token;
/// What the root table says about an intent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Progress {
    /// The person is reviewing the one confirmation; no request exists yet.
    Confirming,
    /// One attempt is waiting for its answer.
    InFlight,
    /// Waiting to send again.
    Retrying,
    Full {
        held: u32,
        room_at: skein_lib::Wall,
    },
    /// Waiting for the link or its party.
    Parked,
    /// Retention ended without a known outcome.
    Unknown,
}

use crate::boundary::Request;
use jig_client::contract::{ActKind, Answer, ObjectKey};
use skein_lib::{Id, Map, Queue, Slab};
/// A local refusal, before the engine could judge the request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NotSent {
    NotKept,
    Unconfirmed,
    Limit,
}
impl NotSent {
    #[must_use]
    pub const fn phrase(self) -> crate::phrases::jig::JigPhrase {
        use crate::phrases::jig::JigPhrase;
        match self {
            Self::NotKept => JigPhrase::RequestNotKept,
            Self::Unconfirmed => JigPhrase::RequestUnconfirmed,
            Self::Limit => JigPhrase::RequestsInFlight,
        }
    }
}
/// The owner to which one terminal is applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// An object's act; a second press joins by object and act.
    Card { object: ObjectKey, act: ActKind },
    /// A page's request, with the generation that originated it.
    Page { handle: Option<jig_client::Handle>, purpose: crate::page::IntentPurpose },
}
/// One intent, never the root table's key, saved record or retry state.
#[derive(Debug)]
pub struct Intent {
    pub scope: u64,
    pub origin: Origin,
    pub ask: Box<Ask>,
    pub progress: Progress,
    pub answer: Option<Answer>,
    /// The composer edit included by this write, so later typing survives its answer.
    pub draft_touch: Option<u64>,
}
/// The owner and answer consumed at the durable terminal.
#[derive(Debug)]
pub struct Receipt {
    pub origin: Origin,
    pub answer: Answer,
    pub scope: u64,
    pub draft_touch: Option<u64>,
    pub ask: Box<Ask>,
}
/// Pending intents, with retired facts available until reclaim.
#[derive(Debug)]
pub struct Intents {
    pub(crate) seed: u64,
    slab: Slab<Intent>,
    ids: Map<Id<Intent>, ()>,
}
impl Intents {
    /// Allocates the configured pending obligations.
    #[must_use]
    pub fn new(capacity: u32) -> Self {
        Self { seed: 0, slab: Slab::with_capacity(capacity), ids: Map::with_capacity(capacity) }
    }
    /// Reads an intent, including a terminal awaiting reclaim.
    #[must_use]
    pub fn get(&self, id: Id<Intent>) -> Option<&Intent> {
        self.slab.get(id)
    }
    /// Pending registrations, excluding retired terminals.
    #[must_use]
    pub const fn ids(&self) -> &Map<Id<Intent>, ()> {
        &self.ids
    }
    /// The original request joined by another press under the same owner.
    #[must_use]
    pub fn joined(&self, scope: u64, origin: Origin) -> Option<Id<Intent>> {
        for (id, ()) in &self.ids {
            let intent = self.slab.get(*id).expect("registered intent exists");
            if intent.scope == scope && intent.origin == origin {
                return Some(*id);
            }
        }
        None
    }
    /// Joins a pending act or emits one new send under its intent token.
    pub fn ask(&mut self, scope: u64, origin: Origin, ask: Box<Ask>, out: &mut Queue<Request>) -> Option<Id<Intent>> {
        for (id, ()) in &self.ids {
            let intent = self.slab.get(*id).expect("registered intent exists");
            if intent.scope == scope && intent.origin == origin {
                return Some(*id);
            }
        }
        let intent = Intent { scope, origin, ask, progress: Progress::InFlight, answer: None, draft_touch: None };
        match self.slab.insert(intent) {
            Ok(id) => {
                let old = self.ids.insert(id, ()).expect("registry fits its paired slab");
                assert!(old.is_none(), "new intent is not registered");
                let ask = self.slab.get(id).expect("inserted intent exists").ask.clone();
                out.push(Request::Client {
                    asked: jig_client::Event::Send {
                        name: None,
                        key: jig_client::Key { asker: jig_client::Asker::JIG, number: id.token().raw() ^ self.seed },
                        request: jig_client::Ask::Jig(ask),
                    },
                });
                Some(id)
            }
            Err(_) => None,
        }
    }
    /// Records only the root table's report of a pending request.
    pub fn progress(&mut self, token: Token, progress: Progress) -> Option<Origin> {
        let id = Id::from_token(token);
        self.ids.get(&id)?;
        let intent = self.slab.get_mut(id).expect("registered intent exists");
        intent.progress = progress;
        Some(intent.origin)
    }
    /// Applies a terminal once and retires the pending obligation.
    pub fn terminal(&mut self, token: Token, answer: Answer) -> Option<Receipt> {
        let id = Id::from_token(token);
        self.ids.remove(&id)?;
        let intent = self.slab.get_mut(id).expect("registered intent exists");
        intent.answer = Some(answer.clone());
        let receipt = Receipt {
            origin: intent.origin,
            scope: intent.scope,
            draft_touch: intent.draft_touch,
            ask: intent.ask.clone(),
            answer,
        };
        self.slab.retire(id);
        Some(receipt)
    }
    /// Retires a request once the table's retention expires, without inventing an answer.
    pub fn unknown(&mut self, token: Token) -> Option<Origin> {
        let id = Id::from_token(token);
        self.ids.remove(&id)?;
        let intent = self.slab.get_mut(id).expect("registered intent exists");
        intent.progress = Progress::Unknown;
        let origin = intent.origin;
        self.slab.retire(id);
        Some(origin)
    }
    /// Frees slots only after their durable terminal was delivered.
    pub fn reclaim(&mut self) {
        self.slab.reclaim();
    }
}

/// A card policy's one send, joined by object and act. Jig 21 calls this after
/// validating its confirmation and completes the durable card transition.
pub fn send_card(
    domain: &mut crate::Ui,
    object: ObjectKey,
    act: ActKind,
    ask: Box<Ask>,
    out: &mut Queue<Request>,
    lent: &jig_client::Client,
) -> Option<Token> {
    let scope = domain.confirmed?;
    let object_id = domain.objects.find(object)?;
    let origin = Origin::Card { object, act };
    let joined = domain.intents.joined(scope, origin).is_some();
    if !joined && let Some(which) = crate::activity::sending::admission(domain, scope, &ask) {
        domain.objects.get_mut(object_id).expect("named card exists").card =
            crate::objects::Card::Refused { refusal: jig_client::contract::Refusal::Limit { which } };

        domain.changed();
        return None;
    }
    match domain.intents.ask(scope, origin, ask, out) {
        Some(id) => {
            crate::activity::sending::pending(domain, id, lent);
            if act == ActKind::Message && !joined {
                let task = match object {
                    ObjectKey::Task(task) => Some(task),
                    ObjectKey::Proposal(_)
                    | ObjectKey::Escalation(_)
                    | ObjectKey::Question { .. }
                    | ObjectKey::PersonTask(_)
                    | ObjectKey::Result(_)
                    | ObjectKey::Note { .. } => None,
                };
                if let Some(task) = task {
                    let touch = domain.drafts.touch(task);
                    let intent = domain.intents.slab.get_mut(id).expect("joined intent exists");
                    if intent.draft_touch.is_none() {
                        intent.draft_touch = touch;
                    }
                }
            }
            let progress = domain.intents.get(id).expect("joined intent exists").progress;
            let object = domain.objects.get_mut(object_id).expect("named card exists");
            object.card = crate::objects::Card::Deciding { request: Some(id.token()), progress };
            object.answer = None;
            domain.changed();
            Some(id.token())
        }
        None => {
            domain.objects.get_mut(object_id).expect("named card exists").card =
                crate::objects::Card::NotSent { why: NotSent::Limit };
            domain.changed();
            None
        }
    }
}
pub(crate) fn progress(
    domain: &mut crate::Ui,
    request: Token,
    progress: Progress,
    _out: &mut Queue<Request>,
    lent: &jig_client::Client,
) {
    if progress == Progress::Unknown {
        unknown(domain, request, lent);
        return;
    }
    let Some(origin) = domain.intents.progress(request, progress) else {
        return;
    };
    match origin {
        Origin::Card { object, .. } => {
            if let Some(id) = domain.objects.find(object) {
                domain.objects.get_mut(id).expect("named card exists").card =
                    crate::objects::Card::Deciding { request: Some(request), progress };
                domain.changed();
            }
        }
        Origin::Page { .. } => domain.changed(),
    }
}
pub(crate) fn answered(
    domain: &mut crate::Ui,
    env: &skein_lib::Env<crate::Limits>,
    request: Token,
    mut answer: Answer,
    out: &mut Queue<Request>,
    lent: &jig_client::Client,
) {
    // A moved record belongs to the caller's shell, never retained UI state (client.md, 6).
    if let Answer::Done(jig_client::contract::Outcome::Made {
        what: jig_client::contract::Made::MovingOut { record, .. },
    }) = &mut answer
    {
        *record = skein_lib::Secret::new(Box::new([]));
    }
    let Some(receipt) = domain.intents.terminal(request, answer) else {
        return;
    };
    crate::activity::sending::answered(domain, request, &receipt.answer);
    match receipt.origin {
        Origin::Page { handle, purpose } => {
            crate::page::conversations::answered(domain, env, (request, handle), purpose, receipt.answer, out, lent);
        }
        Origin::Card { object, act } => {
            if act == ActKind::Message {
                match object {
                    ObjectKey::Task(task) => crate::drafts::sent(
                        domain,
                        receipt.scope,
                        task,
                        receipt.draft_touch,
                        &receipt.ask,
                        &receipt.answer,
                        out,
                    ),
                    ObjectKey::Proposal(_)
                    | ObjectKey::Escalation(_)
                    | ObjectKey::Question { .. }
                    | ObjectKey::PersonTask(_)
                    | ObjectKey::Result(_)
                    | ObjectKey::Note { .. } => {}
                }
            }
            crate::confirm::proposal::remember(domain, object, receipt.ask, &receipt.answer);
            crate::objects::answer::answered(domain, object, act, receipt.answer, out, lent);
        }
    }
}
fn unknown(domain: &mut crate::Ui, request: Token, lent: &jig_client::Client) {
    let Some(origin) = domain.intents.unknown(request) else {
        return;
    };
    domain
        .conversation
        .words
        .refused(jig_client::Key { asker: jig_client::Asker::JIG, number: request.raw() ^ domain.seed });
    match origin {
        Origin::Card { object, act } => crate::objects::answer::unknown(domain, object, act, lent),
        Origin::Page { purpose, .. } => {
            if domain.chats.starting == Some(request) {
                domain.chats.starting = None;
                domain.chats.words = None;
            }
            crate::page::project::goals::unknown(domain, request);
            crate::page::project::owner_form::unknown(domain, request);
            crate::notices::kept::show(domain, crate::notices::kept::Kind::PageUnknown { purpose });
        }
    }
    domain.changed();
}
