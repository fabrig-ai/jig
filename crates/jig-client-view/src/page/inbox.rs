//! Inbox context and filters, without a second copy of an object's card.
//! Whole live documents replace their source; historical pages carry opaque
//! cursors. Needs sort oldest first and updates newest first. Reading results
//! asks the keyed table to mark the newest displayed position once per opening.
//! See client/domain/pages.md, 3, and contract.md, 3.2 and 6–7.
use crate::address::{Address, ProjectSection};
use alloc::boxed::Box;
use jig_client::contract::{
    Count, Cursor, InboxDocument, InboxEntry, InboxGroup, InboxPage, InboxReadGroup, InboxRow, Needs, ObjectKey,
    OwnerInbox, OwnerInboxEntry, OwnerInboxPage, OwnerInboxSelection, Read,
};
use skein_lib::List;
/// An inbox filter, shared by live rows and historical reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Filter {
    pub project: Option<u32>,
    pub kind: Option<InboxGroup>,
    pub goal: Option<u64>,
}
/// One read group's bounded window and independent continuation.
#[derive(Clone, Debug)]
pub struct InboxWindow {
    pub cut: bool,
    pub rows: List<InboxRow>,
    pub before: Option<Cursor>,
    pub after: Option<Cursor>,
    pub more: bool,
    pub reading: Option<skein_lib::Token>,
    /// Original accepted query, including its independent opaque continuation.
    pub query: Option<Read>,
    /// Whole original retained context; never replaced with current requester facts.
    pub selection: Option<List<OwnerInboxSelection>>,
}
impl InboxWindow {
    fn new(rows: u32) -> Self {
        Self {
            cut: false,
            rows: List::with_capacity(rows),
            before: None,
            after: None,
            more: false,
            reading: None,
            query: None,
            selection: None,
        }
    }
    // Read rows past the watch remain; duplicate snapshots never resurrect a removed live key.
    fn past(&mut self, live: &List<InboxRow>, goal: Option<u64>, document: Option<&InboxDocument>) {
        if self.rows.is_empty() {
            return;
        }
        let capacity = self.rows.capacity();
        let rows = core::mem::replace(&mut self.rows, List::with_capacity(capacity));
        for row in rows.into_boxed() {
            let mut watched = false;
            for current in live {
                let membership_known = match current {
                    InboxRow::Entry(_) => true,
                    InboxRow::Owner(value) => owner_membership(value, goal, document).is_some(),
                };
                if current.identity() == row.identity() && membership_known {
                    watched = true;
                    break;
                }
            }
            if !watched {
                self.rows.push(row).expect("retained read rows fit their original capacity");
            }
        }
    }
    fn clear(&mut self) {
        self.rows.clear();
        self.cut = false;
        self.before = None;
        self.after = None;
        self.more = false;
        self.reading = None;
        self.query = None;
        self.selection = None;
    }
}
/// Context rows merge the live watch and both read groups by object key.
/// A watched row wins; each source is bounded by one document's row limit.
#[derive(Debug)]
pub struct Inbox {
    pub state: State,
    pub rows: List<InboxRow>,
    /// Whole latest source; exact owner aggregates never become four counters.
    pub document: Option<InboxDocument>,
    /// A selected live goal has no supplied context for at least one row.
    pub goal_context_pending: bool,
    pub needs: Needs,
    pub updates: Count,
    pub live: List<InboxRow>,
    /// The shared live watch has rows beyond its window; read cursors remain independent.
    pub live_more: bool,
}
/// Page-owned read windows and choices. Live source documents stay with the client.
#[derive(Clone, Debug)]
pub struct State {
    pub filter: Filter,
    pub needs_window: InboxWindow,
    pub updates_window: InboxWindow,
    pub refusal: Option<jig_client::contract::Refusal>,
    pub opening_updates: bool,
    pub marked_results: u64,
    /// The committed read position; pending mark-seen requests do not advance it.
    pub results_seen: u64,
}
impl core::ops::Deref for Inbox {
    type Target = State;
    fn deref(&self) -> &State {
        &self.state
    }
}
impl core::ops::DerefMut for Inbox {
    fn deref_mut(&mut self) -> &mut State {
        &mut self.state
    }
}
impl State {
    pub(crate) fn new(rows: u32) -> Self {
        Self {
            filter: Filter { project: None, kind: None, goal: None },
            needs_window: InboxWindow::new(rows),
            updates_window: InboxWindow::new(rows),
            refusal: None,
            opening_updates: false,
            marked_results: 0,
            results_seen: 0,
        }
    }
    pub(crate) fn snapshot(&self) -> Inbox {
        let mut projected = Inbox::new(self.needs_window.rows.capacity());
        projected.state = self.clone();
        projected.merge();
        projected
    }
    pub(crate) fn clear(&mut self) {
        self.needs_window.clear();
        self.updates_window.clear();
    }
    pub(crate) fn filter(&mut self, filter: Filter) {
        self.filter = filter;
        self.clear();
    }
    #[must_use]
    pub const fn window(&self, group: InboxReadGroup) -> &InboxWindow {
        match group {
            InboxReadGroup::Needs => &self.needs_window,
            InboxReadGroup::Updates => &self.updates_window,
        }
    }
    pub(crate) fn window_mut(&mut self, group: InboxReadGroup) -> &mut InboxWindow {
        match group {
            InboxReadGroup::Needs => &mut self.needs_window,
            InboxReadGroup::Updates => &mut self.updates_window,
        }
    }
    pub(crate) fn page(&mut self, group: InboxReadGroup, page: InboxPage) {
        let mut projected = self.snapshot();
        projected.page(group, page);
        *self = projected.state;
    }
    pub(crate) fn owner_page(&mut self, group: InboxReadGroup, query: Read, page: OwnerInboxPage) {
        let mut projected = self.snapshot();
        projected.owner_page(group, query, page);
        *self = projected.state;
    }
}

impl Inbox {
    pub(crate) fn into_state(self) -> State {
        self.state
    }
    /// Allocates the page's bounded context window.
    #[must_use]
    pub fn new(rows: u32) -> Self {
        Self {
            state: State::new(rows),
            rows: List::with_capacity(rows.checked_mul(4).expect("inbox source rows fit")),
            document: None,
            goal_context_pending: false,
            needs: Needs { questions: 0, proposals: 0, escalations: 0, person_tasks: 0 },
            updates: Count { value: 0, limited: false },
            live: List::with_capacity(rows.checked_mul(2).expect("two live groups fit")),
            live_more: false,
        }
    }
    /// Rebuild this entry's projection after changing its local filter.
    pub fn filter(&mut self, filter: Filter) {
        self.state.filter(filter);
        self.merge();
    }
    /// A filtered row belongs here only while its shared object still waits.
    #[must_use]
    pub fn includes(&self, entry: &InboxRow) -> bool {
        let membership = match entry {
            InboxRow::Entry(value) => self.state.filter.goal.is_none() || self.state.filter.goal == value.goal,
            InboxRow::Owner(value) => {
                owner_membership(value, self.state.filter.goal, self.document.as_ref()) == Some(true)
            }
        };
        (self.state.filter.project.is_none() || self.state.filter.project == entry.project())
            && (self.state.filter.kind.is_none() || self.state.filter.kind == Some(entry.group()))
            && membership
    }
    /// Read window for exactly one parent group.
    #[must_use]
    pub const fn window(&self, group: InboxReadGroup) -> &InboxWindow {
        match group {
            InboxReadGroup::Needs => &self.state.needs_window,
            InboxReadGroup::Updates => &self.state.updates_window,
        }
    }
    pub(crate) const fn window_mut(&mut self, group: InboxReadGroup) -> &mut InboxWindow {
        match group {
            InboxReadGroup::Needs => &mut self.state.needs_window,
            InboxReadGroup::Updates => &mut self.state.updates_window,
        }
    }
    /// The whole live watch replaces its own source, preserving read windows.
    pub(crate) fn replace(&mut self, entries: List<InboxEntry>, more: bool) {
        self.live.clear();
        for entry in entries.into_boxed() {
            self.live.push(InboxRow::Entry(Box::new(entry))).expect("whole live source fits");
        }
        self.live_more = more;
        self.state.needs_window.past(&self.live, self.state.filter.goal, self.document.as_ref());
        self.state.updates_window.past(&self.live, self.state.filter.goal, self.document.as_ref());
        self.merge();
    }
    /// A read terminal moves only its group's rows and cursor.
    pub(crate) fn page(&mut self, group: InboxReadGroup, page: InboxPage) {
        let window = self.window_mut(group);
        window.selection = None;
        window.cut = page.cut;
        window.rows.clear();
        for row in page.rows.into_boxed() {
            window.rows.push(InboxRow::Entry(Box::new(row))).expect("whole historical source fits");
        }
        window.before = page.before;
        window.after = page.after;
        window.more = window.after.is_some();
        match group {
            InboxReadGroup::Needs => {
                self.state.needs_window.past(&self.live, self.state.filter.goal, self.document.as_ref());
            }
            InboxReadGroup::Updates => {
                self.state.updates_window.past(&self.live, self.state.filter.goal, self.document.as_ref());
            }
        }
        self.merge();
    }
    /// Whole exact owner delivery; both allocated groups remain in the envelope.
    pub(crate) fn owner(&mut self, value: Box<OwnerInbox>) {
        self.state.results_seen = value.results_seen;
        self.state.marked_results = self.state.marked_results.max(value.results_seen);
        self.live_more = value.next.is_some();
        self.live.clear();
        for entries in [&value.needs, &value.updates] {
            for entry in entries {
                self.live.push(InboxRow::Owner(Box::new(entry.clone()))).expect("both bounded live groups fit");
            }
        }
        self.document = Some(InboxDocument::Owner(value));
        self.state.needs_window.past(&self.live, self.state.filter.goal, self.document.as_ref());
        self.state.updates_window.past(&self.live, self.state.filter.goal, self.document.as_ref());
        self.merge();
    }
    /// The source selected these rows using the original query; no guessed goal
    /// or ancestry is required or used to re-filter a bounded historical page.
    pub(crate) fn owner_page(&mut self, group: InboxReadGroup, query: Read, page: OwnerInboxPage) {
        let window = self.window_mut(group);
        window.selection = page.selection;
        window.cut = page.cut;
        window.rows.clear();
        for row in page.rows.into_boxed() {
            window.rows.push(InboxRow::Owner(Box::new(row))).expect("whole owner page fits");
        }
        window.before = page.before;
        window.after = page.after;
        window.more = window.after.is_some();
        window.query = Some(query);
        match group {
            InboxReadGroup::Needs => {
                self.state.needs_window.past(&self.live, self.state.filter.goal, self.document.as_ref());
            }
            InboxReadGroup::Updates => {
                self.state.updates_window.past(&self.live, self.state.filter.goal, self.document.as_ref());
            }
        }
        self.merge();
    }
    fn merge(&mut self) {
        self.rows.clear();
        self.goal_context_pending = false;
        for entry in &self.live {
            match entry {
                InboxRow::Owner(value) => {
                    self.goal_context_pending |=
                        owner_membership(value, self.state.filter.goal, self.document.as_ref()).is_none();
                }
                InboxRow::Entry(_) => {}
            }
            if self.includes(entry) {
                self.rows.push(entry.clone()).expect("bounded live groups fit");
            }
        }
        for window in [&self.state.needs_window, &self.state.updates_window] {
            for entry in &window.rows {
                let included = match entry {
                    InboxRow::Entry(_) => self.includes(entry),
                    InboxRow::Owner(_) => query_filter(window.query.as_ref()) == Some(self.state.filter),
                };
                if !included {
                    continue;
                }
                let mut known = false;
                for current in &self.rows {
                    if current.identity() == entry.identity() {
                        known = true;
                        break;
                    }
                }
                if !known {
                    self.rows.push(entry.clone()).expect("four bounded inbox sources fit");
                }
            }
        }
        // Adjacent swaps preserve supplied order at unknown-time boundaries.
        for _pass in 0..self.rows.len() {
            for second in 1..self.rows.len() {
                let first = second.checked_sub(1).expect("row has predecessor");
                let a = self.rows.get(first).expect("row exists");
                let b = self.rows.get(second).expect("row exists");
                if earlier(b, a) {
                    let b = b.clone();
                    let a = core::mem::replace(self.rows.get_mut(first).expect("row exists"), b);
                    *self.rows.get_mut(second).expect("row exists") = a;
                }
            }
        }
    }
}
// Match the original cursor, never the card key, Chip, or current requester tree.
// A missing association remains unknown; an associated empty root list is a
// supplied negative membership answer. Historical pages keep their own query.
fn owner_membership(entry: &OwnerInboxEntry, goal: Option<u64>, document: Option<&InboxDocument>) -> Option<bool> {
    let Some(goal) = goal else { return Some(true) };
    let selection = match document {
        Some(InboxDocument::Owner(value)) => value.selection.as_ref()?,
        Some(InboxDocument::Entry(_)) | None => return None,
    };
    for context in selection {
        if context.cursor == entry.cursor {
            for root in &context.goals {
                if *root == goal {
                    return Some(true);
                }
            }
            return Some(false);
        }
    }
    None
}
/// Whether an inbox kind holds work up.
#[must_use]
pub const fn needs(group: InboxGroup) -> bool {
    match group {
        InboxGroup::Questions | InboxGroup::Proposals | InboxGroup::Escalations | InboxGroup::PersonTasks => true,
        InboxGroup::Results | InboxGroup::Replies => false,
    }
}
fn earlier(a: &InboxRow, b: &InboxRow) -> bool {
    let a_needs = needs(a.group());
    let b_needs = needs(b.group());
    if a_needs != b_needs {
        return a_needs;
    }
    match a.at() {
        Some(a_at) => match b.at() {
            Some(b_at) => {
                if a_at != b_at {
                    return if a_needs { a_at < b_at } else { a_at > b_at };
                }
                // Existing supplied rows retain their original tie ordering;
                // exact owner ties retain the owner's source order.
                match a {
                    InboxRow::Entry(a) => match b {
                        InboxRow::Entry(b) => a.object < b.object,
                        InboxRow::Owner(_) => false,
                    },
                    InboxRow::Owner(_) => false,
                }
            }
            None => false,
        },
        None => false,
    }
}
/// Original source filter, independent of owner row fields.
#[must_use]
pub const fn query_filter(query: Option<&Read>) -> Option<Filter> {
    match query {
        Some(Read::Inbox { project, kind, goal, .. }) => Some(Filter { project: *project, kind: *kind, goal: *goal }),
        Some(
            Read::Tasks { .. }
            | Read::Me
            | Read::Task { .. }
            | Read::Proposal { .. }
            | Read::Escalation { .. }
            | Read::Question { .. }
            | Read::PersonTask { .. }
            | Read::Result { .. }
            | Read::Ended { .. }
            | Read::History { .. }
            | Read::Activity { .. }
            | Read::Turn { .. }
            | Read::Holds { .. }
            | Read::Spend { .. }
            | Read::Project { .. }
            | Read::Notes { .. }
            | Read::Settings
            | Read::Setting(_)
            | Read::Revision(_)
            | Read::Revisions(_)
            | Read::Deployment,
        )
        | None => None,
    }
}
/// Whole Inbox storage, projections and merge scratch including independent windows.
#[must_use]
pub fn worst_case(limits: &crate::Limits) -> Option<u64> {
    let rows = limits.rows.checked_mul(4)?;
    List::<InboxRow>::worst_case(rows)?
        .checked_mul(3)?
        .checked_add(u64::from(rows).checked_mul(limits.document_bytes)?.checked_mul(3)?)?
        .checked_add(limits.document_bytes.checked_mul(3)?)
}
/// The known address behind an entry; a party maker supplies no task address.
#[must_use]
pub const fn address(key: ObjectKey) -> Option<Address> {
    let task = match key {
        ObjectKey::Task(task)
        | ObjectKey::Escalation(task)
        | ObjectKey::PersonTask(task)
        | ObjectKey::Result(task)
        | ObjectKey::Question { task, .. } => task,
        ObjectKey::Proposal(proposal) => match proposal.proposer {
            jig_client::contract::Proposer::Task(task) => task,
            jig_client::contract::Proposer::Party(_) => return None,
        },
        ObjectKey::Note { project, .. } => return Some(Address::Project { project, section: ProjectSection::Notes }),
    };
    Some(Address::task(task))
}
