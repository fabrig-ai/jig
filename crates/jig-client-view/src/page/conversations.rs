//! The party's chat list and first-words form, with page-owned keyed intents.
//! List rows retain committed runtime, spend and unread facts; card objects are
//! shared by key. No charter or budget is chosen here: the frame offers both.
//! A pending start survives page navigation and opens its task only after a durable
//! answer. Historical reads are bounded and use opaque cursors. See pages.md, 4.
//!
//! | Form | Event | Next | Emits |
//! |---|---|---|---|
//! | ready/refused | submit | pending | one start-chat intent |
//! | pending | submit | pending | nothing |
//! | pending | refused | refused | nothing |
//! | pending | made chat | ready | navigate to chat activity |
use crate::{
    Request, Ui,
    address::{Address, Place, TaskSection},
    fields::Purpose,
    intents::Origin,
    objects::{Body, Source},
    page::{IntentPurpose, inbox::Filter},
};
use alloc::boxed::Box;
use jig_client::contract::{
    self, Answer, Ask, InboxGroup, InboxReadGroup, Limit, ObjectKey, Read, Refusal, SeenThrough, Text,
};
use jig_client::thread::window::{Edge, Edges, End};
use skein_lib::{Env, List, Queue, Token};
/// First-words submission and its refusal, held apart from the list window.
#[derive(Debug)]
pub struct State {
    pub starting: Option<Token>,
    pub refusal: Option<Refusal>,
    pub project: Option<u32>,
    pub scope: Option<u64>,
    pub words: Option<Text>,
    pub more: bool,
    pub show_live: bool,
}
impl State {
    pub(crate) const fn new() -> Self {
        Self { starting: None, refusal: None, project: None, scope: None, words: None, more: false, show_live: false }
    }
}
/// Starts a chat with only a project and the first words; a second press joins.
pub fn start(
    domain: &mut Ui,
    _env: &Env<crate::Limits>,
    project: u32,
    out: &mut Queue<Request>,
    lent: &jig_client::Client,
) {
    if domain.blocked || domain.chats.starting.is_some() {
        return;
    }
    let Some(scope) = domain.confirmed else {
        return;
    };
    let Some(field) = domain.fields.get(Purpose::NewChat) else {
        return;
    };
    if field.text.bytes().is_empty() {
        return;
    }
    let words = field.text.clone();
    let Some(me) = domain.frame(lent).document else { return };
    let ask = match jig_client::shapes::chat(me, project, words.clone()) {
        Ok(ask) => Box::new(ask),
        Err(_) => {
            domain.chats.refusal = Some(Refusal::NotOffered);
            domain.changed();
            return;
        }
    };
    match domain.intents.ask(
        scope,
        Origin::Page { handle: domain.page_handle(), purpose: IntentPurpose::BeginConversation },
        ask,
        out,
    ) {
        Some(id) => {
            domain.chats.starting = Some(id.token());
            domain.chats.refusal = None;
            domain.chats.project = Some(project);
            domain.chats.scope = Some(scope);
            domain.chats.words = Some(words);
        }
        None => domain.chats.refusal = Some(Refusal::Limit { which: Limit::Requests }),
    }
    domain.changed();
}
/// Starts from a card, naming the thing by its committed title in the first words.
pub fn about(
    domain: &mut Ui,
    env: &Env<crate::Limits>,
    key: ObjectKey,
    out: &mut Queue<Request>,
    lent: &jig_client::Client,
) {
    let Some(id) = domain.objects.find(key) else {
        return;
    };
    let Some(object) = domain.borrow(lent).objects().get(id) else {
        return;
    };
    let (project, title) = match &object.body {
        Some(Body::Task(task)) => (Some(task.project), Some(task.chip.title.bytes.clone())),
        Some(Body::OwnerEscalation(value)) => (Some(value.task.project), Some(value.task.chip.title.bytes.clone())),
        Some(Body::Chip(chip)) => (None, Some(chip.title.bytes.clone())),
        Some(Body::Escalation(value)) => (None, Some(value.task.title.bytes.clone())),
        Some(Body::Question(value)) => (None, Some(value.task.title.bytes.clone())),
        Some(Body::PersonTask(value)) => (None, Some(value.task.title.bytes.clone())),
        Some(Body::OwnerPersonTask(value)) => (value.project(), Some(value.task.title.bytes.clone())),
        Some(Body::Result(value)) => (None, Some(value.task.title.bytes.clone())),
        Some(Body::Proposal(value)) => (Some(value.project), Some(value.why.clone())),
        Some(Body::OwnerProposal(value)) => (Some(value.project), Some(value.reason.bytes.clone())),
        Some(Body::Note { project, document }) => (Some(*project), Some(document.description.bytes.clone())),
        Some(Body::Historical(_)) | None => (None, None),
    };
    let mut project = project.or(object.project);
    if project.is_none() {
        for entry in &domain.borrow(lent).inbox().rows {
            if entry.card_key() == key {
                project = entry.project();
                break;
            }
        }
    }
    let Some(project) = project else {
        return;
    };
    let Some(title) = title else {
        return;
    };
    match domain.write_field(Purpose::NewChat, title) {
        Ok(()) => start(domain, env, project, out, lent),
        Err(which) => crate::page::routing::refuse(domain, which),
    }
}
pub(crate) fn answered(
    domain: &mut Ui,
    env: &Env<crate::Limits>,
    request_and_handle: (Token, Option<jig_client::Handle>),
    purpose: IntentPurpose,
    answer: Answer,
    out: &mut Queue<Request>,
    lent: &jig_client::Client,
) {
    let (request, handle) = request_and_handle;
    match purpose {
        IntentPurpose::ProjectSettings => crate::page::project::owner_form::answered(domain, request, answer),
        IntentPurpose::CreateGoal => {
            crate::page::project::goals::answered(domain, env, request, handle, answer, out, lent);
        }
        IntentPurpose::Seen(_) => {
            match answer {
                Answer::Done(_) => {}
                Answer::Refused { refusal, .. } => {
                    crate::notices::kept::show(
                        domain,
                        crate::notices::kept::Kind::PageAnswer { answer: Answer::Refused { refusal, at: None } },
                    );
                }
            }
            domain.changed();
        }
        IntentPurpose::BeginConversation => {
            if domain.chats.starting != Some(request) {
                return;
            }
            domain.chats.starting = None;
            match answer {
                Answer::Refused { refusal, .. } => domain.chats.refusal = Some(refusal),
                Answer::Done(outcome) => match crate::outcome::route(&outcome) {
                    crate::outcome::Route::Created { task } => {
                        let same = match &domain.chats.words {
                            Some(words) => match domain.fields.get(Purpose::NewChat) {
                                Some(field) => field.text == *words,
                                None => false,
                            },
                            None => false,
                        };
                        if same && domain.chats.scope == domain.confirmed {
                            let empty = Text::new(Box::from([]), domain.text_limit).expect("empty words fit");
                            match domain.write_field(Purpose::NewChat, empty) {
                                Ok(()) => {}
                                Err(which) => crate::page::routing::refuse(domain, which),
                            }
                        }
                        let address = Address::Task { task, section: TaskSection::Activity };
                        if handle == domain.page_handle() && domain.chats.scope == domain.confirmed {
                            crate::page::routing::navigate(domain, env, Place::Jig(address), false, out, lent);
                        } else {
                            crate::notices::kept::show(
                                domain,
                                crate::notices::kept::Kind::Answer {
                                    object: ObjectKey::Task(task),
                                    answer: Answer::Done(outcome),
                                },
                            );
                        }
                    }
                    crate::outcome::Route::Words { .. }
                    | crate::outcome::Route::Proposed
                    | crate::outcome::Route::Other => {
                        crate::notices::kept::show(
                            domain,
                            crate::notices::kept::Kind::PageAnswer { answer: Answer::Done(outcome) },
                        );
                    }
                },
            }
            domain.chats.words = None;
            domain.changed();
        }
    }
}
fn seen(domain: &mut Ui, value: SeenThrough, out: &mut Queue<Request>) -> bool {
    let Some(scope) = domain.confirmed else {
        return false;
    };
    domain
        .intents
        .ask(
            scope,
            Origin::Page { handle: domain.page_handle(), purpose: IntentPurpose::Seen(value) },
            Box::new(Ask::Seen { through: value }),
            out,
        )
        .is_some()
}
pub(crate) fn open_updates(domain: &mut Ui, out: &mut Queue<Request>, lent: &jig_client::Client) {
    let Some(project) = domain.shown_inbox() else {
        return;
    };
    if !domain.shows(crate::page::Section::Inbox { group: InboxReadGroup::Updates, project }, lent) {
        return;
    }
    domain.inbox.opening_updates = true;
    mark_results(domain, out, lent);
}
pub(crate) fn mark_results(domain: &mut Ui, out: &mut Queue<Request>, lent: &jig_client::Client) {
    if !domain.inbox.opening_updates {
        return;
    }
    let mut through = domain.inbox.marked_results;
    for entry in &domain.borrow(lent).inbox().rows {
        if let Some(position) = entry.result_position() {
            through = through.max(position);
        }
    }
    if through > domain.inbox.marked_results && seen(domain, SeenThrough::Results { through }, out) {
        domain.inbox.marked_results = through;
        domain.inbox.opening_updates = false;
    }
}
pub(crate) fn open_chat(
    domain: &mut Ui,
    env: &Env<crate::Limits>,
    task: u64,
    out: &mut Queue<Request>,
    lent: &jig_client::Client,
) {
    let mut through: Option<u64> = None;
    for entry in &domain.borrow(lent).inbox().rows {
        if entry.group() == InboxGroup::Replies
            && entry.card_key() == ObjectKey::Task(task)
            && let Some(line) = entry.reply()
            && line.chip.task == task
            && Some(line.project) == entry.project()
            && let Some(position) = entry.turn_position()
            && let Some(conversation) = line.conversation
            && position > conversation.seen
        {
            through = Some(match through {
                Some(previous) => previous.max(position),
                None => position,
            });
        }
    }
    if let Some(through) = through {
        let sent = seen(domain, SeenThrough::Conversation { task, through }, out);
        if !sent {
            crate::page::routing::refuse(domain, Limit::Requests);
        }
    }
    crate::page::routing::navigate(
        domain,
        env,
        Place::Jig(Address::Task { task, section: TaskSection::Activity }),
        false,
        out,
        lent,
    );
}
pub(crate) fn filter_inbox(domain: &mut Ui, filter: Filter, out: &mut Queue<Request>) {
    if domain.shown_inbox().is_none() {
        return;
    }
    if domain.inbox.filter == filter
        && (domain.inbox.needs_window.reading.is_some() || domain.inbox.updates_window.reading.is_some())
    {
        return;
    }
    domain.inbox.filter(filter);
    start_inbox(domain, out);
    domain.changed();
}
pub(crate) fn start_inbox(domain: &mut Ui, out: &mut Queue<Request>) {
    for group in [InboxReadGroup::Needs, InboxReadGroup::Updates] {
        start_group(domain, group, out);
    }
}
pub(crate) fn start_group(domain: &mut Ui, group: InboxReadGroup, out: &mut Queue<Request>) {
    let filter = domain.inbox.filter;
    if let Some(kind) = filter.kind
        && InboxReadGroup::of(kind) != group
    {
        return;
    }
    ask_read(
        domain,
        inbox_number(group),
        Read::Inbox { group, project: filter.project, kind: filter.kind, goal: filter.goal, after: None },
        out,
    );
}
const fn inbox_number(group: InboxReadGroup) -> u16 {
    match group {
        InboxReadGroup::Needs => INBOX_NEEDS_READ,
        InboxReadGroup::Updates => INBOX_UPDATES_READ,
    }
}
pub(crate) const fn inbox_group(number: u16) -> Option<InboxReadGroup> {
    match number {
        INBOX_NEEDS_READ => Some(InboxReadGroup::Needs),
        INBOX_UPDATES_READ => Some(InboxReadGroup::Updates),
        _ => None,
    }
}
fn ask_read(domain: &mut Ui, number: u16, what: Read, out: &mut Queue<Request>) {
    for (id, reading) in &domain.handles.reads {
        if reading.what == what {
            if let Some(group) = inbox_group(number) {
                domain.inbox.window_mut(group).reading = Some(Token::new(id.number));
            }
            return;
        }
    }
    match domain.handles.read(Source::Window { number }, what, out) {
        Some(id) => {
            if let Some(group) = inbox_group(number) {
                domain.inbox.window_mut(group).reading = Some(Token::new(id.number));
            }
        }
        None => crate::page::routing::refuse(domain, Limit::Reads),
    }
}
/// The Needs window's binding number.
pub const INBOX_WINDOW: u64 = 10;
/// The needs read source.
pub(crate) const INBOX_NEEDS_READ: u16 = 10;
/// The Updates window's independent binding number.
pub const INBOX_UPDATES_WINDOW: u64 = 12;
/// The updates read source.
pub(crate) const INBOX_UPDATES_READ: u16 = 12;
/// The conversations window's binding number.
pub const CONVERSATIONS_WINDOW: u64 = 11;
/// The conversations read source.
pub(crate) const CONVERSATIONS_READ: u16 = 11;
pub(crate) fn more(domain: &mut Ui, window: Token, end: End, out: &mut Queue<Request>, lent: &jig_client::Client) {
    if window == Token::new(INBOX_WINDOW) || window == Token::new(INBOX_UPDATES_WINDOW) {
        let group = if window == Token::new(INBOX_WINDOW) { InboxReadGroup::Needs } else { InboxReadGroup::Updates };
        let filter = domain.inbox.filter;
        if let Some(kind) = filter.kind
            && InboxReadGroup::of(kind) != group
        {
            return;
        }
        let selected = domain.inbox.window(group);
        let cursor = match end {
            End::Before => selected.before.clone(),
            End::After => selected.after.clone(),
        };
        if cursor.is_some() || selected.more {
            ask_read(
                domain,
                inbox_number(group),
                Read::Inbox { group, project: filter.project, kind: filter.kind, goal: filter.goal, after: cursor },
                out,
            );
        }
    } else if window == Token::new(CONVERSATIONS_WINDOW) {
        let displayed = domain.borrow(lent).conversation_rows();
        let edge = match end {
            End::Before => &displayed.edges.before,
            End::After => &displayed.edges.after,
        };
        let earlier = match edge {
            Edge::More(_) => true,
            Edge::End => domain.chats.more,
        };
        if earlier {
            let Some(project) = domain.shown_conversations() else { return };
            let Some(me) = domain.frame(lent).document else { return };
            let Some(mut query) = crate::queries::chats(me, project) else { return };
            query.phase = Some(contract::PhaseClass::Ended);
            query.order = contract::QueryOrder::Ended;
            let ended = match edge {
                Edge::More(cursor) => Some(cursor.clone()),
                Edge::End => None,
            };
            ask_read(domain, CONVERSATIONS_READ, Read::Tasks { query, live: None, ended }, out);
        }
    }
}
/// Whether the chat has a live phase, independent of the runtime's words.
#[must_use]
pub const fn live(phase: contract::Phase) -> bool {
    match phase {
        contract::Phase::Ended | contract::Phase::Done | contract::Phase::Failed | contract::Phase::Cancelled => false,
        contract::Phase::Waiting
        | contract::Phase::Idle
        | contract::Phase::Due
        | contract::Phase::Preparing
        | contract::Phase::Claimed
        | contract::Phase::Running
        | contract::Phase::BackingOff
        | contract::Phase::Active
        | contract::Phase::Closing
        | contract::Phase::Held => true,
    }
}
/// Whether these limits are representable: every edge bound they size
/// is finite and every capacity they derive fits (programming-model.md, 6.3).
#[must_use]
pub(crate) fn valid(limits: &crate::Limits) -> bool {
    limits.rows.checked_mul(3).is_some()
}

/// Receives the live snapshot at its reached end, preserving retained ended rows.
pub(crate) fn delivered(
    domain: &mut Ui,
    env: &Env<crate::Limits>,
    source: Source,
    conversations: contract::Tasks,
    out: &mut Queue<Request>,
    lent: &jig_client::Client,
) {
    domain.chats.show_live = domain.conversation_rows.edges.reaches(End::Before);
    let mut projected = domain.conversation_rows.clone();
    if project_live(&mut projected, conversations) > 0 {
        super::routing::refuse(domain, Limit::Objects);
    }
    domain.conversation_rows.edges = projected.edges;
    crate::page::replies::conversations(domain, env, source, out, lent);
    domain.changed();
}
pub(crate) fn project_live(window: &mut crate::windows::ConversationWindow, conversations: contract::Tasks) -> u32 {
    if window.edges.reaches(End::Before) {
        // A current live snapshot replaces live rows, while retained past rows
        // still belong to the read window. An overlapping watch row wins whole.
        let capacity = window.rows.capacity();
        let old = core::mem::replace(&mut window.rows, List::with_capacity(capacity));
        for row in old.into_boxed() {
            let mut named = false;
            for current in &conversations.rows {
                if current.chip.task == row.chip.task {
                    named = true;
                    break;
                }
            }
            if !live(row.chip.phase) && !named {
                window.rows.push(row).expect("retained rows fit their window");
            }
        }
        let after = window.edges.after.clone();
        let rows = conversations.rows;

        let edges = Edges { before: Edge::End, after: Edge::End };
        let dropped = window.slide(rows, edges, End::Before);
        if dropped == 0 {
            window.edges.after = after;
        }
        if dropped > 0 {
            return dropped;
        }
    }
    0
}

/// Merge the visible layout, retaining only facts supplied by reads in the page.
pub(crate) fn read_page(domain: &mut Ui, _source: Source, page: contract::TaskPage, lent: &jig_client::Client) {
    let contract::EndedGroup::Rows { rows, after, cut } = page.ended else { return };
    domain.chats.more = false;
    let mut displayed = domain.borrow(lent).conversation_rows();
    displayed.cut |= cut;
    displayed.slide(rows, super::routing::page_edges(None, after), End::After);
    domain.conversation_rows = displayed;
}
