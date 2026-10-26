//! Typed live documents, made on demand from the core's committed vocabulary
//! (`client/domain/contract.md`, sections 5 and 7; `domain/engine.md`, 11).
//! No document is retained here, and this module neither delivers nor decides.
//! `make` authenticates visibility from a supplied party number and copies only
//! bounded text and lists. Its caller holds the answer until durable.
//!
//! Frame and inbox counts include every live need and bounded unread updates.
//! Read positions move only on a keyed mark. Objects carry actor-specific offers
//! from the shared admission judges. Run writing status,
//! provisional text and calls belong to the watch stream; committed turns never
//! enter its text. Application subjects belong to their connectors.
//!
//! Store-only rows use public bounded row constructors without restoring them.
//! Chips retain owner phase clocks, readiness, retries, resource references and
//! endings. Core-aware constructors also join fenced best-effort observations;
//! their engine receipt clocks are distinct from placement/adoption clocks.
//! Retained failure and dependency explanations use exact task/activity reads.
//! Closing chips name the connectors they wait on (QUESTIONS 29). Effect
//! descriptions, prices and counts remain in each connector's panel. Recurring wall
//! schedules still require their schedule owner. `make_with_clock` projects
//! host grace from monotonic deadlines; account deadlines carry their source wall.

use jig_domain_people::ApplicationScope;

mod chip_phase;
mod endings;
pub use chip_phase::{ChipDetails, ChipObservation, ChipWork};
mod inbox_deployment;
pub use inbox_deployment::{Inbox, InboxEntry, InboxObject};
mod catalogue;
pub use catalogue::{PayerReadiness, ProviderOffer, Readiness};
mod accounts;
pub use accounts::{AccountLine, Accounts, LastEnd, SignInLine, SourceLine};
pub use jig_domain_accounts::Use;
mod deployment;
mod settings;
mod settings_review;
pub use settings::{Follows, HeldRevision, SettingLine, Settings, SettingsProposal};
pub use settings_review::{ReviewItem, SettingsReview, settings_heads};
mod programs;
pub use deployment::{
    Account, AccountState, Capacity, Deployment, DocumentClock, HelloRefusalLine, Host, HostRun, ProjectCapacity,
    RunLocation, StoreFill, make_with_clock,
};
pub use programs::{ProgramHost, ProgramLogin};
mod inbox_selection;
pub(crate) use inbox_selection::retained as inbox_selection_from_ancestry;
pub use inbox_selection::{InboxSelection, inbox_selection_worst_case};
mod objects;
mod offers;
mod project_note;
mod task_lines;
pub use task_lines::{ConversationRead, TaskLine, task_line};
pub mod task_query;
pub use task_query::{Lines, PhaseClass, QueryOrder, TaskQuery, lines};
mod small;
pub use objects::{
    AmendmentChange, AmendmentState, EndedDelegate, Escalation, FundingOffer, HistoryLine, MilestoneLine,
    MilestonePhase, PersonTask, Proposal, ProposalAction, ProposalHistory, ProposalHistoryState, ProposalWaiting,
    ProposedTask, Question, ResultDoc, RetainedAction, ShownResult, SpecLine, ended_delegate_from_row,
    history_from_row, milestone_from_row, person_from_row, result_from_row, retained_action,
};
pub use project_note::{
    Ledger, Member, Note, NoteLocation, Policy, PolicyRole, Project, ProjectResource, Recurring, Requirement,
    ResourceRole, SpendLine, note_from_entry,
};
mod live_delegates;
pub use live_delegates::LiveDelegate;
pub(crate) mod task_tree;
mod task_wait;
pub use task_tree::{
    AuthorityLine, BudgetLine, ContractLine, Dependency, Ending, EscalationState, FoldedCount, GrantLine, HoldLine,
    NoteScope, Parameter, Part, PartKey, PartKind, PatternEnd, PhaseCounts, PriorPhase, ProposalRef, ResourceLine,
    ResultValue, Task, TaskLink, TaskSettings, Tree, TreeRow, task_from_row,
};
pub use task_wait::{OpenProgram, TaskRunWait, TaskSourceState, TaskWaitRefused, TaskWaitWhy};

pub use crate::waits::WaitsOn;
use crate::{Core, Limits};
use alloc::boxed::Box;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_lib::{Duration, List, Wall};

/// One bounded UTF-8 text value, with loss stated separately.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Text {
    pub bytes: Box<[u8]>,
    pub cut: bool,
}

/// Versions of jig's contract served by this configured deployment.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Versions {
    pub minimum: u32,
    pub maximum: u32,
}

/// Bounds of one constructed document and its configured served versions.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct DocumentLimits {
    pub roles: u32,
    pub segments: u32,
    pub grants: u32,
    pub executors: u32,
    pub implications: u32,
    pub rows: u32,
    pub text_bytes: u32,
    pub chips: u32,
    pub lines: u32,
    pub versions: Versions,
}

/// Continuation position in a subject's stable numbered list.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Cursor {
    /// A task or goal number in the subject's ordered list.
    Number { number: u64 },
    /// Exact identity of one inbox row; number families may overlap.
    Inbox { task: u64, object: InboxObject, at: Option<Wall> },
}

/// The bounded slice requested by a watch or read.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Window {
    pub rows: u32,
    pub after: Option<Cursor>,
}

/// Identity of the immutable proposal's maker.
pub use jig_domain_people::Proposer;

/// One subject whose current document a party may read or watch.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Subject {
    /// A connector-owned subject within the client's stated role scope.
    Application {
        scope: ApplicationScope,
        connector: u16,
        subject: skein_lib::Token,
    },
    /// The authenticated party's frame.
    Frame {
        party: u64,
    },
    /// A task's overview and controls.
    Task {
        task: u64,
    },
    /// A task's live requester subtree.
    Tree {
        task: u64,
    },
    /// Each named task's compact live status.
    Chips {
        tasks: List<u64>,
    },
    /// The party's waiting objects and unread updates.
    Inbox {
        party: u64,
    },
    /// The party's accounts.
    Accounts {
        party: u64,
    },
    /// Filtered live tasks in one project, newest change first.
    /// A fixed task query, independent of any application shape.
    Query {
        query: TaskQuery,
    },
    /// A task's current attempt.
    Run {
        task: u64,
    },
    /// Hosts, accounts and capacity.
    Deployment,
    Settings,
    /// An immutable pending proposal.
    Proposal {
        proposer: Proposer,
        number: u64,
    },
    /// A held task's escalation.
    Escalation {
        task: u64,
    },
    /// An unanswered question.
    Question {
        task: u64,
        message: u64,
    },
    /// A task addressed to a person or role.
    PersonTask {
        task: u64,
    },
    /// A result still named by the live engine.
    Result {
        task: u64,
    },
    /// A project's resources, policy and funding.
    Project {
        project: u32,
    },
    /// A note retained by its owning child.
    Note {
        note: u64,
    },
}

/// The subjects a watch may name (`protocol/client.md`, section 7).
/// Own subjects take their party from the authenticated sign-in in the core.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Watched {
    Application { scope: ApplicationScope, connector: u16, subject: skein_lib::Token },
    Frame,
    Task { task: u64 },
    Tree { task: u64 },
    Chips { tasks: List<u64> },
    Inbox,
    Accounts,
    Query { query: TaskQuery },
    Run { task: u64 },
    Deployment,
    Settings,
}

/// Why no live document is available.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Missing {
    /// No visible subject is known by this name.
    Unknown,
    /// The supplied party may not see this subject.
    NotAllowed,
    /// The subject belongs to stored history.
    Ended,
    /// The requested whole value exceeds construction bounds.
    TooLarge,
}

/// A bounded count, with saturation stated.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Count {
    /// Every currently known entry was counted.
    Known(u32),
    /// The bounded cache may omit older entries.
    AtLeast(u32),
}

/// A task's executor kind; its detail lives on the task document.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ExecutorKind {
    /// An agent with a configured charter.
    Agent,
    /// A connector-owned procedure.
    Procedure,
    /// A party or project role.
    Person,
}

/// A compact typed phase and its immediate waiting facts.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum PhaseLine {
    /// Waiting for live dependencies or resource holdings.
    Waiting { dependencies: List<u64>, more: bool, holds: bool },
    /// Current activation state of an agent or procedure.
    Active(tasks::Active),
    /// Waiting for the addressed party or role.
    Person { address: tasks::PersonAddress, taken_by: Option<u64>, since: Option<Wall> },
    /// Closing through its ordered obligations.
    Closing(tasks::Stage),
    /// Held for this reason at this independent revision.
    Held { reason: tasks::Hold, revision: u64 },
    /// Ended outside the live arena.
    Ended,
}

/// A task named in passing, with its bounded first-line title.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Chip {
    pub number: u64,
    pub title: Text,
    pub executor: ExecutorKind,
    pub phase: PhaseLine,
    /// Nonrecursive owner facts and optional fenced live observations.
    pub details: Box<ChipDetails>,
}

/// A held project role and the actual current funding facts available to the party.
/// Each project has its kept name (core.md, 6.1); unopened pools have no ledger.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct FrameProjectRole {
    pub project: u32,
    pub role: people::Role,
    pub name: Text,
    pub pool: Option<Ledger>,
    /// Direct creating bounds for this party and project (contract.md, 5.3).
    pub creating: Option<crate::acts::Creating>,
    /// The configured models the party's tasks here may use (domain/authority.md, section 3).
    pub models: jig_domain_authority::Models,
    /// Configured templates with this project's policy overrides applied.
    pub templates: List<crate::Template>,
}

/// Party identity, current roles, bounded counts and contract configuration.
/// Deployment membership is supplied independently of the bounded project rows
/// (`client/domain/contract.md`, sections 2, 5 and 7.1).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Frame {
    pub party: u64,
    pub kind: people::Kind,
    pub login: Text,
    pub name: Text,
    pub deployment_role: Option<people::Role>,
    pub projects: List<FrameProjectRole>,
    pub needs: u32,
    /// Exact kinds, including needs beyond the recent inbox prefix.
    pub need_counts: people::Needs,
    pub updates: Count,
    pub results_seen: u64,
    pub retention: Duration,
    pub cut: bool,
    /// Public configured provider offers within the document line bound.
    pub providers: List<ProviderOffer>,
    /// Whether additional provider offers were omitted.
    pub providers_cut: bool,
    /// Actual startup key ceiling, including the client carried-value bound.
    pub key_bytes: u32,
    /// The driver's redirect bound: the most bytes a pasted sign-in address may have.
    pub redirect_bytes: u32,
}

/// A live attempt's committed progress; provisional writing is supplied by watches.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Run {
    pub task: u64,
    pub attempt: Option<u64>,
    pub committed_turn: u32,
    pub writing: Option<jig_domain_views::Writing>,
    /// Current content call, independently of committed or absent main writing.
    pub call: Option<jig_domain_views::Call>,
}

/// One whole typed document, within its configured limits.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Document {
    /// The party's frame.
    Frame(Frame),
    /// Compact status of each named task.
    Chips(List<Chip>),
    /// Current attempt and committed turn.
    Run(Run),
    /// One task page's current overview.
    Task(Box<Task>),
    /// A bounded live subtree with folds.
    Tree(Tree),
    /// A filtered live project task list.
    /// A query window over live tasks.
    Lines(Lines),
    /// A pending immutable proposal.
    Proposal(Box<Proposal>),
    /// A held task and its preserved story.
    Escalation(Escalation),
    /// Whole unanswered question text.
    Question(Question),
    /// A current person task and its form.
    PersonTask(Box<PersonTask>),
    /// An outcome retained in a live closing row.
    Result(Box<ResultDoc>),
    /// Project resources, roles, policy and ledgers.
    Project(Box<Project>),
    /// A note loaded by its owning child.
    Note(Note),
    /// Party-only bounded waiting and unread groups.
    Inbox(Inbox),
    /// The party's own accounts and sign-ins.
    Accounts(Box<Accounts>),
    /// Fleet placement, accounts and finite capacities.
    Deployment(Box<Deployment>),
    Settings(Box<Settings>),
}

/// Make the current visible document without changing state or emitting effects.
/// Clamp its window to the engine's rows (`client/domain/contract.md`, 7.2).
pub fn make(
    core: &Core,
    limits: &Limits,
    party: u64,
    subject: &Subject,
    window: &Window,
    decision: Wall,
) -> Result<Document, Missing> {
    if document_worst_case(&limits.documents).is_none() {
        return Err(Missing::TooLarge);
    }
    let window = &Window { rows: window.rows.min(limits.documents.rows), ..*window };
    core.people.identity(party).ok_or(Missing::Unknown)?;
    match subject {
        Subject::Application { .. } => Err(Missing::Unknown),
        Subject::Frame { party: owner } => {
            if *owner != party {
                return Err(Missing::NotAllowed);
            }
            let frame = small::frame(core, limits, party)?;
            Ok(Document::Frame(frame))
        }
        Subject::Chips { tasks: named } => {
            if named.len() > limits.documents.chips {
                return Err(Missing::TooLarge);
            }
            let mut chips = List::with_capacity(named.len());
            for number in named {
                chips.push(small::chip(core, limits, party, *number)?).expect("named chips fit");
            }
            Ok(Document::Chips(chips))
        }
        Subject::Run { task } => {
            let run = small::run(core, limits, party, *task)?;
            Ok(Document::Run(run))
        }
        Subject::Task { task } => {
            let task = task_tree::task(core, limits, party, *task, decision)?;
            Ok(Document::Task(Box::new(task)))
        }
        Subject::Tree { task } => {
            let tree = task_tree::tree(core, limits, party, *task, window)?;
            Ok(Document::Tree(tree))
        }
        Subject::Query { query } => Ok(Document::Lines(lines(core, limits, party, query, window)?)),
        Subject::Inbox { party: owner } => {
            if *owner != party {
                return Err(Missing::NotAllowed);
            }
            Ok(Document::Inbox(inbox_deployment::inbox(core, limits, party, window)?))
        }
        Subject::Accounts { party: owner } => {
            Ok(Document::Accounts(Box::new(accounts::make(core, limits, party, *owner, window)?)))
        }
        Subject::Settings => Ok(Document::Settings(Box::new(settings::make(core, party)?))),
        Subject::Deployment => Ok(Document::Deployment(Box::new(deployment::deployment(core, limits, party, None)?))),
        Subject::Proposal { proposer, number } => {
            Ok(Document::Proposal(Box::new(objects::proposal(core, limits, party, *proposer, *number, decision)?)))
        }
        Subject::Escalation { task } => {
            Ok(Document::Escalation(objects::escalation(core, limits, party, *task, decision)?))
        }
        Subject::Question { task, message } => {
            Ok(Document::Question(objects::question(core, limits, party, *task, *message, decision)?))
        }
        Subject::PersonTask { task } => {
            Ok(Document::PersonTask(Box::new(objects::person(core, limits, party, *task, decision)?)))
        }
        Subject::Result { task } => {
            Ok(Document::Result(Box::new(objects::result(core, limits, party, *task, decision)?)))
        }
        Subject::Project { project } => {
            Ok(Document::Project(Box::new(project_note::project(core, limits, party, *project, decision)?)))
        }
        Subject::Note { .. } => Err(Missing::Ended),
    }
}

/// Checked allocation bound for one document; parents price each retained delivery separately.
#[must_use]
/// The inbox selection roots additionally require `document_with_context_worst_case`,
/// because their configured requester depth belongs to the task limits.
pub fn document_worst_case(limits: &DocumentLimits) -> Option<u64> {
    if limits.rows == 0
        || limits.text_bytes == 0
        || limits.chips == 0
        || limits.lines == 0
        || limits.versions.minimum == 0
        || limits.versions.minimum > limits.versions.maximum
    {
        return None;
    }
    let chip = chip_phase::heap_worst_case(limits)?;
    let chips = List::<Chip>::worst_case(limits.chips)?.checked_add(u64::from(limits.chips).checked_mul(chip)?)?;
    let charter = List::<u32>::worst_case(limits.lines)?;
    let creating = charter
        .checked_add(List::<crate::acts::CreateProcedure>::worst_case(limits.lines)?)?
        .checked_add(List::<u32>::worst_case(limits.roles)?)?
        .checked_add(u64::try_from(size_of::<AuthorityLine>()).ok()?)?
        .checked_add(task_tree::authority_worst_case(limits)?)?;
    let frame = List::<FrameProjectRole>::worst_case(limits.lines)?
        .checked_add(u64::from(limits.lines).checked_mul(creating)?)?
        .checked_add(u64::from(limits.lines).checked_mul(u64::from(limits.text_bytes))?)?
        .checked_add(u64::from(limits.text_bytes).checked_mul(2)?)?
        .checked_add(catalogue::offers_worst_case(limits)?)?;
    u64::try_from(size_of::<Document>()).ok()?.checked_add(
        chips
            .max(frame)
            .max(u64::from(limits.text_bytes).checked_mul(3)?)
            .max(task_tree::worst_case(limits)?)
            .max(task_query::worst_case(limits)?)
            .max(objects::worst_case(limits)?)
            .max(project_note::worst_case(limits)?)
            .max(deployment::worst_case(limits)?)
            .max(accounts::worst_case(limits)?),
    )
}

/// Bound of a document including the inbox's requester selection context.
#[must_use]
pub fn document_with_context_worst_case(limits: &Limits) -> Option<u64> {
    document_worst_case(&limits.documents)?
        .checked_add(settings::worst_case(limits)?)?
        .checked_add(settings_review::worst_case(limits)?)?
        .checked_add(inbox_selection_worst_case(limits.documents.rows, limits.tasks.depth)?)?
        .checked_add(u64::from(limits.documents.lines).checked_mul(crate::templates::list_worst_case(limits)?)?)
}

/// Peak scratch allocated while constructing party/deployment pages. Child
/// accessors copy bounded shallow facts only; documents themselves are separate.
pub(crate) fn document_scratch_worst_case(limits: &Limits) -> Option<u64> {
    let waits = List::<u64>::worst_case(limits.tasks.tree_tasks)?
        .checked_add(List::<people::Entry>::worst_case(crate::inbox::waiting_capacity(limits)?)?)?;
    Some(
        deployment::scratch_worst_case(limits)?
            .max(inbox_deployment::scratch_worst_case(limits)?)
            .max(settings::scratch_worst_case(limits)?)
            .max(catalogue::scratch_worst_case(limits)?)
            .max(List::<jig_domain_accounts::Pending>::worst_case(limits.accounts.sign_ins)?)
            .max(List::<crate::Template>::worst_case(limits.templates)?)
            .max(task_query::scratch_worst_case(limits)?.checked_add(waits)?),
    )
}

/// Join volatile live observations to a loaded owner row. The reader checks
/// project visibility first; this does not restore or change the row.
pub fn chip_from_row_with_core(core: &Core, limits: &Limits, row: &tasks::TaskRecord) -> Result<Chip, Missing> {
    small::chip_with_core(core, limits, row)
}

pub(crate) fn chip_heap_worst_case(limits: &DocumentLimits) -> Option<u64> {
    chip_phase::heap_worst_case(limits)
}

/// Construct a bounded chip from a loaded owner row, without live observations
/// or restoring it. Whole opaque resource keys must fit the receiving limits.
pub fn chip_from_row(limits: &Limits, row: &tasks::TaskRecord) -> Result<Chip, Missing> {
    small::chip_from_row(limits, row)
}

/// Funding numbers copied from their retained owner row.
#[must_use]
pub fn budget_from_row(row: &tasks::TaskRecord) -> BudgetLine {
    task_tree::budget(row)
}

/// Complete bounded authority facts used when judging draft controls.
pub(crate) fn act_authority(limits: &Limits, value: &tasks::Authority) -> Result<AuthorityLine, Missing> {
    task_tree::authority(limits, value)
}
