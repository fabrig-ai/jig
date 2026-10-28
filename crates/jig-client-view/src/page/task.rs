//! One task page, whose header names the shared object and whose plan keeps only
//! bounded tree context. Executor facts decide which sections have content. Parts
//! are references handed to the root with the page generation, never their values.
//! History and ended subtrees are read on demand; live plans are watched. Agent
//! activity is the conversation owner's state, never copied here. See client/domain/pages.md, 5.
//!
//! | Page | Event | Next | Emits |
//! |---|---|---|---|
//! | loading | task document | header known | section parts and executor activity |
//! | plan | tree delivery | bounded chips and folds | shared object reads |
//! | history | more | pending read | history cursor |
//! | plan | open ended subtree | pending read | ended-delegate cursor |
//! | any | leave | empty | handled by generation routing |
use crate::{
    Request, Ui,
    address::TaskSection,
    objects::{Body, Object, Source},
};
use alloc::boxed::Box;
use jig_client::contract::{
    self, Executor, Limit, ObjectKey, Part, Phase, PhaseCounts, ProposalKey, Read, Task, Tree, TreeNode,
};
use skein_lib::{Env, Id, List, Map, Queue, Token};
/// Which plan rows a person has asked to see.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PlanFilter {
    /// Every phase.
    All,
    /// Rows held, or with held work below them.
    Held,
    /// Rows whose work has not ended.
    Live,
}
/// Offered whose current task document has something to show.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Offered {
    pub offered: List<TaskSection>,
}
/// Bounded plan context, preserving counts of every held task beyond the window.
#[derive(Debug)]
#[expect(clippy::partial_pub_fields, reason = "fold choices change only through admitted page actions")]
pub struct Plan {
    /// Whole supplied owner tree, independent of older node fixtures.
    pub filter: PlanFilter,
    folds: Map<u64, bool>,
}
/// The page's header handle and section-specific read choices.
#[derive(Debug)]
#[expect(clippy::partial_pub_fields, reason = "proposal refresh positions change only through admitted Task facts")]
pub struct TaskPage {
    pub header: Option<Id<Object>>,
    /// Supplied proposal cards, including their departing shared objects.
    pub proposals: List<ProposalKey>,
    proposal_activity: Option<u64>,
    /// Whole owner ended page under the selected original read.
    pub owner_ended: Option<Box<contract::OwnerEndedPage>>,
    pub plan: Plan,
    pub subtree_history: bool,
    /// Loss reported by the current ended-delegate read window.
    pub ended_cut: bool,
    pub ended_after: Option<contract::Cursor>,
    pub ended_before: Option<contract::Cursor>,
    pub ended_task: Option<u64>,
}
impl TaskPage {
    pub(crate) fn new(rows: u32, objects: u32) -> Self {
        Self {
            header: None,
            proposals: List::with_capacity(objects),
            proposal_activity: None,
            owner_ended: None,
            plan: Plan {
                filter: PlanFilter::All,
                folds: Map::with_capacity(rows.checked_add(1).expect("root fold fits")),
            },
            subtree_history: false,
            ended_cut: false,
            ended_after: None,
            ended_before: None,
            ended_task: None,
        }
    }
    pub(crate) fn clear(&mut self) {
        self.header = None;
        self.proposals.clear();
        self.proposal_activity = None;
        self.owner_ended = None;
        self.plan.filter = PlanFilter::All;
        let mut keys = List::with_capacity(self.plan.folds.len());
        for (task, _) in &self.plan.folds {
            keys.push(*task).expect("fold key fits");
        }
        for task in &keys {
            let _open = self.plan.folds.remove(task).expect("collected fold exists");
        }
        self.subtree_history = false;
        self.ended_cut = false;
        self.ended_after = None;
        self.ended_before = None;
        self.ended_task = None;
    }
}
pub(crate) fn counts() -> PhaseCounts {
    PhaseCounts { ended: 0, waiting: 0, running: 0, held: 0, closing: 0, done: 0, failed: 0, cancelled: 0 }
}
/// Whether a section has content for this executor and its current facts.
#[must_use]
pub fn offers(task: &Task) -> Offered {
    let (impact, panels) = match &task.context {
        Some(context) => (
            context.impact.tasks.waiting > 0
                || context.impact.tasks.running > 0
                || context.impact.tasks.held > 0
                || context.impact.tasks.closing > 0
                || context.impact.tasks.done > 0
                || context.impact.tasks.failed > 0
                || context.impact.tasks.cancelled > 0
                || context.impact.tasks.ended > 0,
            !context.parts.is_empty(),
        ),
        None => (false, false),
    };
    let plan = !task.delegates.is_empty() || impact;
    let panels = panels
        || !task.resources.is_empty()
        || match task.chip.executor {
            Executor::Procedure { .. } => true,
            Executor::Agent { .. } | Executor::Party { .. } | Executor::Role { .. } => false,
        };
    let budget = match task.chip.executor {
        Executor::Agent { .. } | Executor::Procedure { .. } => true,
        Executor::Party { .. } | Executor::Role { .. } => {
            task.budget.numbers.budget > 0
                || task.budget.numbers.spent > 0
                || task.budget.numbers.spent_below > 0
                || task.budget.numbers.reserved > 0
        }
    };
    // The mandatory result contract supplies overview content even with an empty spec.
    let overview = true;
    let mut offered = List::with_capacity(6);
    for section in [
        TaskSection::Overview,
        TaskSection::Activity,
        TaskSection::Plan,
        TaskSection::History,
        TaskSection::Budget,
        TaskSection::Panels,
    ] {
        let present = match section {
            TaskSection::Overview => overview,
            TaskSection::Activity | TaskSection::History => true,
            TaskSection::Plan => plan,
            TaskSection::Budget => budget,
            TaskSection::Panels => panels,
        };
        if present {
            offered.push(section).expect("at most six sections");
        }
    }
    Offered { offered }
}
/// Whether a section should be offered, without interpreting acts or authority.
#[must_use]
pub fn offered(sections: &Offered, section: TaskSection) -> bool {
    sections.offered.as_slice().contains(&section)
}
/// Retains a whole bounded tree, folding additional rows by phase and preserving
/// the owner's total held count so a held descendant cannot disappear.
pub(crate) fn tree(domain: &mut Ui, tree: Tree) {
    let plan = &mut domain.task_page.plan;
    let mut removed = List::with_capacity(plan.folds.len());
    for (task, _) in &plan.folds {
        let mut retained = false;
        for node in &tree.nodes {
            if node.chip.task == *task {
                retained = true;
            }
        }
        if !retained {
            removed.push(*task).expect("fold key fits");
        }
    }
    for task in &removed {
        let _removed = plan.folds.remove(task);
    }
    domain.changed();
}
/// Moves the complete owner envelope; local choices do not alter source rows.
pub(crate) fn owner_tree(domain: &mut Ui, tree: &contract::OwnerTree) {
    let plan = &mut domain.task_page.plan;
    let mut removed = List::with_capacity(plan.folds.len());
    for (task, _) in &plan.folds {
        let mut retained = tree.root.task == *task;
        for row in &tree.rows {
            retained |= row.chip.task == *task;
        }
        if !retained {
            removed.push(*task).expect("fold keys fit");
        }
    }
    for task in &removed {
        let _open = plan.folds.remove(task);
    }
    domain.changed();
}
impl Plan {
    /// Existing local fold choice, defaulting to the supplied chip phase.
    #[must_use]
    pub fn owner_expanded(&self, chip: &contract::Chip) -> bool {
        match self.folds.get(&chip.task) {
            Some(open) => *open,
            None => crate::page::conversations::live(chip.phase),
        }
    }
    /// Live subtrees begin open; ended subtrees begin folded to their line.
    #[must_use]
    pub fn expanded(&self, node: &TreeNode) -> bool {
        match self.folds.get(&node.chip.task) {
            Some(open) => *open,
            None => crate::page::conversations::live(node.chip.phase),
        }
    }
    /// Filtering preserves every retained row so changing the filter loses no facts.
    #[must_use]
    pub fn visible(&self, node: &TreeNode) -> bool {
        match self.filter {
            PlanFilter::All => true,
            PlanFilter::Held => node.chip.phase == Phase::Held || node.held_below > 0,
            PlanFilter::Live => crate::page::conversations::live(node.chip.phase),
        }
    }
}
/// Chooses only the part references named by the selected section.
#[must_use]
pub fn parts(task: &Task, section: TaskSection, limit: u32) -> List<Part> {
    let mut parts = List::with_capacity(limit);
    // The shared chip is shown in every task section.
    if let Some(part) = crate::application::owner_hold(&task.chip) {
        push(&mut parts, part);
    }
    match section {
        TaskSection::Overview => {
            for resource in &task.resources {
                if let Some(part) = resource.part() {
                    push(&mut parts, part.clone());
                }
            }
            result(&mut parts, task);
        }
        TaskSection::Activity => match task.chip.executor {
            Executor::Procedure { connector, .. } => push(&mut parts, Part::Panel { connector, task: task.chip.task }),
            Executor::Agent { .. } | Executor::Party { .. } | Executor::Role { .. } => {}
        },
        TaskSection::Budget => {
            for grant in &task.authority.authority.grants {
                push(&mut parts, Part::Kind { connector: grant.connector, kind: grant.kind });
            }
        }
        TaskSection::Panels => {
            if let Some(context) = &task.context {
                if let Some(closing) = &context.closing
                    && let Some(connectors) = &closing.connectors
                {
                    for connector in connectors {
                        push(&mut parts, Part::Panel { connector: *connector, task: task.chip.task });
                    }
                }
                append(&mut parts, &context.parts);
            }
            for resource in &task.resources {
                if let Some(part) = resource.part() {
                    push(&mut parts, part.clone());
                }
            }
            match task.chip.executor {
                Executor::Procedure { connector, .. } => {
                    push(&mut parts, Part::Panel { connector, task: task.chip.task });
                }
                Executor::Agent { .. } | Executor::Party { .. } | Executor::Role { .. } => {}
            }
        }
        TaskSection::Plan | TaskSection::History => {}
    }
    parts
}
fn append(out: &mut List<Part>, values: &List<Part>) {
    for part in values {
        push(out, part.clone());
    }
}
fn push(out: &mut List<Part>, part: Part) {
    for kept in &*out {
        if *kept == part {
            return;
        }
    }
    if out.room() > 0 {
        out.push(part).expect("part fits its bounded list");
    }
}
fn result(out: &mut List<Part>, task: &Task) {
    if let Some(result) = &task.result {
        match result {
            contract::TaskResult::Ending(_) => {}
            contract::TaskResult::Document(result) => match &result.value {
                Some(contract::ResultValue::Connector { part, .. }) => push(out, part.clone()),
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
            },
        }
    }
}
/// A Task watch names its header and the exact proposal references together.
pub(crate) fn delivered(domain: &mut Ui, source: Source, task: Box<Task>) {
    let key = ObjectKey::Task(task.chip.task);
    let keys = task_names(task.chip.task, &task.proposals);
    let missed = domain.objects.replace(source, keys.as_slice());
    crate::page::routing::excess(domain, missed);
    if domain.objects.find(key).is_none() {
        crate::page::routing::refuse(domain, Limit::Objects);
    }
    domain.changed();
}
fn task_names(number: u64, proposals: &List<ProposalKey>) -> List<ObjectKey> {
    let mut keys = List::with_capacity(proposals.len().checked_add(1).expect("bounded header and proposal keys"));
    keys.push(ObjectKey::Task(number)).expect("header fits");
    for proposal in proposals {
        let key = ObjectKey::Proposal(*proposal);
        if !keys.as_slice().contains(&key) {
            keys.push(key).expect("supplied proposal keys fit");
        }
    }
    keys
}
fn proposal_cards(
    domain: &mut Ui,
    env: &Env<crate::Limits>,
    number: u64,
    proposals: List<ProposalKey>,
    activity: u64,
    out: &mut Queue<Request>,
    lent: &jig_client::Client,
) {
    let mut source = None;
    for (id, watch) in &domain.handles.watches {
        if watch.owner == (crate::page::shown::owner(domain))
            && watch.subject == (contract::Subject::Task { task: number })
        {
            source = Some(Source::Watch(*id));
            break;
        }
    }
    let Some(source) = source else {
        return;
    };
    let keys = task_names(number, &proposals);
    let missed = domain.objects.replace(source, keys.as_slice());
    crate::page::routing::excess(domain, missed);
    if missed > 0 {
        crate::page::routing::refuse(domain, Limit::Objects);
    }
    let moved = domain.task_page.proposal_activity != Some(activity);
    domain.task_page.proposal_activity = Some(activity);
    let mut shown = List::with_capacity(env.limits.objects);
    for key in &domain.task_page.proposals {
        if domain.objects.find(ObjectKey::Proposal(*key)).is_some() {
            shown.push(*key).expect("retained proposal names fit the working set");
        }
    }
    for proposal in &proposals {
        let key = ObjectKey::Proposal(*proposal);
        if !domain.objects.named(key, source) {
            continue;
        }
        if !shown.as_slice().contains(proposal) {
            shown.push(*proposal).expect("unique proposal names fit the working set");
        }
        let id = domain.objects.find(key).expect("admitted proposal exists");
        let object = domain.borrow(lent).objects().get(id).expect("named proposal exists");
        if moved || object.body.is_none() {
            let what = Read::Proposal { proposal: *proposal };
            if !domain.handles.pending(&what) && domain.handles.read(Source::Card(id), what, out).is_none() {
                crate::page::routing::refuse(domain, Limit::Reads);
            }
        }
    }
    domain.task_page.proposals = shown;
}
/// After the task document reveals its executor, opens only that activity owner.
pub(crate) fn document(domain: &mut Ui, env: &Env<crate::Limits>, out: &mut Queue<Request>, lent: &jig_client::Client) {
    let Some(number) = domain.shown_task() else {
        return;
    };
    let Some(id) = domain.objects.find(ObjectKey::Task(number)) else {
        return;
    };
    domain.task_page.header = Some(id);
    let projected = domain.borrow(lent).objects().get(id).expect("named task exists");
    let task = match &projected.body {
        Some(Body::Task(task)) => task,
        Some(
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
            | Body::Note { .. },
        )
        | None => return,
    };
    let project = task.project;
    let executor = task.chip.executor;
    let phase = task.chip.phase;
    let proposals = task.proposals.clone();
    let activity = task.activity;
    let mut values = List::with_capacity(env.limits.parts.saturating_add(1));
    for section in [
        TaskSection::Overview,
        TaskSection::Activity,
        TaskSection::Plan,
        TaskSection::History,
        TaskSection::Budget,
        TaskSection::Panels,
    ] {
        if domain.shows(crate::page::Section::Task { task: number, section }, lent) {
            for part in parts(task, section, env.limits.parts.saturating_add(1)).into_boxed() {
                if values.as_slice().contains(&part) {
                    continue;
                }
                if values.push(part).is_err() {
                    break;
                }
            }
        }
    }
    proposal_cards(domain, env, number, proposals, activity, out, lent);
    let mut emitted = 0_u32;
    for part in values.into_boxed() {
        if emitted == env.limits.parts {
            crate::page::routing::refuse(domain, Limit::Objects);
            break;
        }
        emitted = emitted.saturating_add(1);
        let scope = crate::application::owner_scope(&part, contract::PartScope::Project(project));
        out.push(Request::ShowPart { part, scope });
    }
    if domain.shows(crate::page::Section::Task { task: number, section: TaskSection::Plan }, lent) {
        if crate::page::conversations::live(phase) {
            open_once(domain, contract::Subject::Tree { task: number, window: env.limits.rows }, out);
        } else {
            domain.task_page.ended_task = Some(number);
            read(domain, ENDED_READ, Read::Ended { task: number, after: None }, out);
        }
    }
    if !domain.shows(crate::page::Section::Task { task: number, section: TaskSection::Activity }, lent) {
        return;
    }
    match executor {
        Executor::Agent { .. } => {
            let subject = contract::Subject::Run { task: number, turn: None, offset: 0 };
            if open_once(domain, subject, out) {
                read(
                    domain,
                    crate::activity::routing::ACTIVITY_READ,
                    Read::Activity { task: number, before: None },
                    out,
                );
            }
        }
        Executor::Procedure { .. } => {}
        Executor::Party { .. } | Executor::Role { .. } => {
            let key = ObjectKey::PersonTask(number);
            let source = Source::Window { number: PERSON_TASK_READ };
            if domain.objects.name(key, source).is_none() {
                crate::page::routing::refuse(domain, Limit::Objects);
                return;
            }
            let object = domain.objects.find(key).expect("person task named above");
            if domain.borrow(lent).objects().get(object).expect("named person task exists").body.is_none() {
                read(domain, PERSON_TASK_READ, Read::PersonTask { task: number }, out);
            }
        }
    }
}
fn open_once(domain: &mut Ui, subject: contract::Subject, out: &mut Queue<Request>) -> bool {
    super::routing::open_once(domain, subject, crate::page::shown::owner(domain), out)
}

fn read(domain: &mut Ui, number: u16, what: Read, out: &mut Queue<Request>) {
    crate::page::shown::context_read(domain, &what);
    if domain.handles.pending(&what) {
        return;
    }
    if domain.handles.read(Source::Window { number }, what, out).is_none() {
        crate::page::routing::refuse(domain, Limit::Reads);
    }
}
/// Fold presentation belongs to this page and never changes committed work.
pub(crate) fn fold(domain: &mut Ui, object: Token, open: bool, lent: &jig_client::Client) {
    let Some(shown) = domain.shown_task() else {
        return;
    };
    if !domain.shows(crate::page::Section::Task { task: shown, section: TaskSection::Plan }, lent) {
        return;
    }
    let Some(object) = domain.borrow(lent).objects().get(Id::from_token(object)) else {
        return;
    };
    let task = match object.key {
        ObjectKey::Task(task) => task,
        ObjectKey::Proposal(_)
        | ObjectKey::Escalation(_)
        | ObjectKey::Question { .. }
        | ObjectKey::PersonTask(_)
        | ObjectKey::Result(_)
        | ObjectKey::Note { .. } => return,
    };
    let projected = domain.borrow(lent).task_page();
    let plan = &projected.plan;
    if let Some(owner) = &plan.owner {
        let mut retained = owner.root.task == task;
        for row in &owner.rows {
            retained |= row.chip.task == task;
        }
        if retained && domain.task_page.plan.folds.insert(task, open).is_ok() {
            domain.changed();
        }
        return;
    }
    for node in plan.nodes {
        if node.chip.task == task {
            if domain.task_page.plan.folds.insert(task, open).is_ok() {
                domain.changed();
            }
            return;
        }
    }
}
/// Selects a plan filter and reopens its live source without inventing history.
pub(crate) fn filter(domain: &mut Ui, filter: PlanFilter) {
    domain.task_page.plan.filter = filter;
    domain.changed();
}
/// Chooses task-only or subtree history and starts its first page.
pub(crate) fn history(domain: &mut Ui, subtree: bool, out: &mut Queue<Request>) {
    let Some(task) = domain.shown_task() else {
        return;
    };
    domain.task_page.subtree_history = subtree;
    domain.history_rows.rows.clear();
    domain.history_rows.cut = false;
    read(domain, HISTORY_READ, Read::History { task, subtree, after: None }, out);
    domain.changed();
}
/// Opens one ended subtree, whose results name shared cards.
pub(crate) fn open_ended(domain: &mut Ui, task: u64, out: &mut Queue<Request>, lent: &jig_client::Client) {
    let Some(shown) = domain.shown_task() else {
        return;
    };
    if !domain.shows(crate::page::Section::Task { task: shown, section: TaskSection::Plan }, lent) {
        return;
    }
    let mut found = false;
    let projected = domain.borrow(lent).task_page();
    if let Some(owner) = &projected.plan.owner {
        found = owner.root.task == task;
        for row in &owner.rows {
            found |= row.chip.task == task;
        }
    }
    for node in projected.plan.nodes {
        if node.chip.task == task {
            found = true;
        }
    }
    if !found {
        return;
    }
    domain.task_page.owner_ended = None;
    domain.task_page.ended_task = Some(task);
    domain.task_page.ended_cut = false;
    domain.task_page.ended_after = None;
    domain.task_page.ended_before = None;
    read(domain, ENDED_READ, Read::Ended { task, after: None }, out);
}
/// The history window's binding number.
pub const HISTORY_WINDOW: u64 = 14;
/// The history read source.
pub(crate) const HISTORY_READ: u16 = 2;
/// Ended delegates in the opened subtree.
pub const ENDED_WINDOW: u64 = 13;
/// The ended subtree read source.
pub(crate) const ENDED_READ: u16 = 16;
/// The person task read source.
pub(crate) const PERSON_TASK_READ: u16 = 15;
pub(crate) fn more(domain: &mut Ui, window: Token, end: jig_client::thread::window::End, out: &mut Queue<Request>) {
    if window == Token::new(HISTORY_WINDOW) {
        let edge = match end {
            jig_client::thread::window::End::Before => &domain.history_rows.edges.before,
            jig_client::thread::window::End::After => &domain.history_rows.edges.after,
        };
        let cursor = match edge {
            jig_client::thread::window::Edge::More(cursor) => Some(cursor.clone()),
            jig_client::thread::window::Edge::End => None,
        };
        if let Some(cursor) = cursor {
            let Some(task) = domain.shown_task() else {
                return;
            };
            read(
                domain,
                HISTORY_READ,
                Read::History { task, subtree: domain.task_page.subtree_history, after: Some(cursor) },
                out,
            );
        }
    } else if window == Token::new(ENDED_WINDOW) {
        let cursor = match end {
            jig_client::thread::window::End::Before => domain.task_page.ended_before.clone(),
            jig_client::thread::window::End::After => domain.task_page.ended_after.clone(),
        };
        if let Some(task) = domain.task_page.ended_task
            && let Some(cursor) = cursor
        {
            read(domain, ENDED_READ, Read::Ended { task, after: Some(cursor) }, out);
        }
    }
}
/// Whether these limits are representable: every edge bound they size
/// is finite and every capacity they derive fits (programming-model.md, 6.3).
#[must_use]
pub(crate) fn valid(limits: &crate::Limits) -> bool {
    limits.rows.checked_add(1).is_some()
}

pub(crate) fn ended(domain: &mut Ui, before: Option<contract::Cursor>, after: Option<contract::Cursor>) {
    domain.task_page.ended_before = before;
    domain.task_page.ended_after = after;
}

/// Install one admitted owner page without naming synthetic Result objects.
pub(crate) fn owner_ended(domain: &mut Ui, page: Box<contract::OwnerEndedPage>) {
    domain.task_page.ended_cut = page.cut;
    ended(domain, page.before.clone(), page.after.clone());
    domain.task_page.owner_ended = Some(page);
    domain.changed();
}
