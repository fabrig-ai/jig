//! Task pages, live trees and goal lines (`client/domain/pages.md`, 5 and 6.2).
//! Copies bounded rows from their owners. Trees fold every unshown live row into
//! phase counts; held counts cover the whole subtree. Goal progress uses retained
//! goal projections, including ended rows. Nothing here mutates a task or policy.

#![expect(clippy::manual_map, reason = "step code uses explicit matches instead of closures")]

use super::{Chip, Cursor, Missing, Proposer, Text, Window};
use crate::{Core, Limits};
use alloc::boxed::Box;
use jig_domain_tasks as tasks;
use skein_lib::{List, Wall};

pub use tasks::{Part, PartKey, PartKind};

/// Terminal coverage rule of a granted resource pattern.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum PatternEnd {
    /// One literal terminal segment.
    Exact(Box<[u8]>),
    /// A prefix and its descendants.
    Open(Box<[u8]>),
}

/// One connector permission on an unchanged resource pattern.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct GrantLine {
    pub connector: u16,
    pub kind: u16,
    pub segments: List<Box<[u8]>>,
    pub last: PatternEnd,
}

/// A connector resource scope where notes may be written.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct NoteScope {
    pub connector: u16,
    pub segments: List<Box<[u8]>>,
    pub last: PatternEnd,
}

/// An authority carrier in bounded document form.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct AuthorityLine {
    pub models: tasks::Models,
    pub tools: tasks::Tools,
    pub grants: List<GrantLine>,
    pub executors: List<tasks::AuthorityExecutor>,
    pub tasks: u32,
    pub depth: u32,
    pub ceiling: tasks::Budget,
    pub notes: tasks::Scopes,
    pub note_resources: List<NoteScope>,
}

/// Result form and its admitted bounds, usable as a person task's form.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum ContractLine {
    /// Exact connector state required for approval; it is never shortened.
    Approval { state: Box<[u8]>, part: Part },
    /// A report in words.
    Report { words: u32 },
    /// A configured choice with an explanation and bounded follow-ups.
    Verdict { choices: List<tasks::Verdict>, cut: bool },
    /// A connector-owned change and its summary.
    Change { connector: u16, kind: u16, words: u32 },
}

/// Contract-shaped result words and an optional connector-owned value.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum ResultValue {
    /// Whole approved state and optional bounded explanation.
    Approved { state: Box<[u8]>, words: Option<Text> },
    /// Successful report.
    Report(Text),
    /// A selected verdict code with its explanation.
    Verdict { code: u32, fields: Box<[tasks::ResultField]>, words: Text },
    /// A connector value with summary words.
    Change { kind: u16, part: Part, words: Text },
    /// A task that could not complete its contract.
    Failure(Text),
}

/// Ending carried while a live task closes, or in an ended result document.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Ending {
    /// Completed its contract.
    Done(ResultValue),
    /// Failed with these words.
    Failed(Text),
    /// Cancelled, preserving any previously completed result.
    Cancelled { reason: Text, result: Option<ResultValue>, by: tasks::Party, at: Wall },
}

/// One task reference, including a historical result still named by live work.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum TaskLink {
    /// An ended task whose stored owner row supplied its identity and wall time.
    Retained { chip: Chip, at: Option<Wall> },
    /// The referenced task is live and visible.
    Live(Chip),
    /// Its title and complete result belong to a stored task row.
    Stored { number: u64, status: Option<tasks::Status> },
}

/// Dependency facts from its owning task and the prerequisite's live row or stub
/// (client/domain/contract.md, 5.1; domain/tasks.md, 5.7 and 7).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Dependency {
    pub number: u64,
    pub pending: bool,
    pub added: Option<tasks::Added>,
    pub keeps: bool,
    pub status: Option<tasks::Status>,
}

/// A pending proposal's immutable identity.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ProposalRef {
    pub proposer: Proposer,
    pub number: u64,
}

/// Current held-decision recipient and any durable rejection words.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum EscalationState {
    /// The hold is being routed to its eligible holder in this decision.
    Routing,
    /// An identified holder may decide it.
    Waiting { holder: tasks::EscalationHolder, entry: u64, since: Wall },
    /// Decided to leave it held.
    Rejected { by: u64, reason: Text },
}

/// A hold's reason, revision and prior lifecycle, with its escalation.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct HoldLine {
    pub circle: Option<tasks::Circle>,
    /// The connector's account, independently of the resource or outbox origin.
    pub account: Option<Part>,
    pub source: Option<Part>,
    pub reason: tasks::Hold,
    pub revision: u64,
    pub before: PriorPhase,
    pub escalation: Option<EscalationState>,
    pub tries: tasks::Tries,
    pub hold_wait_since: Option<Wall>,
}

/// The lifecycle preserved by a hold.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PriorPhase {
    /// Waiting for dependencies or resource holdings.
    Waiting,
    /// An activation's phase.
    Active(tasks::Active),
    /// Closing through this obligation.
    Closing(tasks::Stage),
}

/// A task's concrete allotment and its financial source.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct BudgetLine {
    pub numbers: tasks::Numbers,
    pub left: u64,
    pub funder: tasks::Funder,
    pub deadline: Option<Wall>,
}

/// A resource the task holds or awaits, and its authentic writer if present.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ResourceLine {
    pub part: Part,
    pub kind: u16,
    pub taken: bool,
    pub holder: Option<u64>,
    pub writer: Option<tasks::Writer>,
    pub writer_lost: bool,
}

/// A bounded specification parameter; connector resources remain references.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Parameter {
    /// An opaque named numeric value.
    Number { name: u32, value: u64 },
    /// A named UTF-8 text value.
    Bytes { name: u32, value: Text },
    /// A connector-owned numbered resource.
    Resource { name: u32, part: Part },
}

/// The settings revisions an agent task's runs read their charter from
/// (client/domain/contract.md, 5.1; domain/engine.md, 17.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TaskSettings {
    /// The revision the task records.
    pub task: u32,
    /// Its live run's, only while that run was prepared under another.
    pub run: Option<u32>,
    /// The newest revision's number.
    pub newest: u32,
}

/// A task page's header, overview, funding, authority and current held decision.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Task {
    /// None for a procedure's or a person's task.
    pub settings: Option<TaskSettings>,
    pub waits_on: super::WaitsOn,
    pub label: u32,
    pub flags: tasks::Flags,
    pub last_activity: Wall,
    pub chip: Chip,
    pub acts: List<crate::acts::Offered>,
    pub project: u32,
    pub goal: Option<u64>,
    pub priority: Option<u32>,
    pub executor: tasks::Executor,
    pub requester: tasks::Party,
    pub lineage: List<Chip>,
    /// Every live direct delegate; identities are never omitted from a close decision.
    pub delegates: List<super::LiveDelegate>,
    pub changed: Wall,
    pub spec_revision: u64,
    /// Present only for a person executor; independent of spec and hold.
    pub person_revision: Option<u64>,
    /// The party holding a role-addressed task, if any.
    pub taken_by: Option<u64>,
    /// Last committed immutable activity position.
    pub activity: u64,
    /// Current specification source facts; list loss is independent of the page's cut.
    pub spec: super::SpecLine,
    pub words: Text,
    pub parameters: List<Parameter>,
    pub instructions: Option<Text>,
    /// An agent task's LLM choice as kept, by the core's indices; `None` takes its charter's
    /// default (domain/tasks.md, 3).
    pub llm: Option<tasks::LlmChoice>,
    /// The last successfully claimed main account.
    pub kept_account: Option<u32>,
    pub run_wait: Option<super::TaskRunWait>,
    pub resources: List<ResourceLine>,
    pub inputs: List<TaskLink>,
    pub dependencies: List<TaskLink>,
    pub dependency_facts: List<Dependency>,
    pub references: List<TaskLink>,
    pub subscriptions: List<tasks::Subscription>,
    pub wake: tasks::WakePolicy,
    pub contract: ContractLine,
    pub result: Option<Ending>,
    pub budget: BudgetLine,
    pub authority: AuthorityLine,
    pub hold: Option<HoldLine>,
    pub proposals: List<ProposalRef>,
    /// At least one list omitted rows; individual text values state their cuts.
    pub cut: bool,
}

/// Phase totals of every task counted, independent of visible rows.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PhaseCounts {
    pub waiting: u32,
    pub idle: u32,
    pub starting: u32,
    pub running: u32,
    pub backing_off: u32,
    pub closing: u32,
    pub held: u32,
    pub done: u32,
    pub failed: u32,
    pub cancelled: u32,
}

impl PhaseCounts {
    /// Begin a bounded phase census.
    pub const ZERO: Self = Self {
        waiting: 0,
        idle: 0,
        starting: 0,
        running: 0,
        backing_off: 0,
        closing: 0,
        held: 0,
        done: 0,
        failed: 0,
        cancelled: 0,
    };

    pub(super) fn add(&mut self, phase: &tasks::Phase) {
        let count = match phase {
            tasks::Phase::Waiting => &mut self.waiting,
            tasks::Phase::Active(active) => match active {
                tasks::Active::Idle => &mut self.idle,
                tasks::Active::Due | tasks::Active::Preparing | tasks::Active::Claimed { .. } => &mut self.starting,
                tasks::Active::Running { .. } => &mut self.running,
                tasks::Active::BackingOff { .. } => &mut self.backing_off,
            },
            tasks::Phase::Closing(_) => &mut self.closing,
            tasks::Phase::Held { .. } => &mut self.held,
            tasks::Phase::Ended(ending) => match ending {
                tasks::Ending::Done(_) => &mut self.done,
                tasks::Ending::Failed { .. } => &mut self.failed,
                tasks::Ending::Cancelled { .. } => &mut self.cancelled,
            },
        };
        *count = count.checked_add(1).expect("census bounded by task limits");
    }
}

/// A visible live descendant and its immediate requester relationship.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct TreeRow {
    pub chip: Chip,
    pub parent: u64,
    pub depth: u32,
    pub proposals: List<ProposalRef>,
    pub spent: u64,
    pub dependencies: List<TaskLink>,
    pub cut: bool,
}

/// Unshown descendants folded under their nearest shown common root.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct FoldedCount {
    pub parent: u64,
    pub phases: PhaseCounts,
    pub held_below: u32,
}

/// A window on a task's live subtree; counts include rows outside the window.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Tree {
    pub root: Chip,
    pub rows: List<TreeRow>,
    pub folded: List<FoldedCount>,
    pub held_below: u32,
    pub next: Option<Cursor>,
}

pub(super) fn part_path(limits: &Limits, connector: u16, kind: PartKind, bytes: &[Box<[u8]>]) -> Result<Part, Missing> {
    Ok(Part { connector, kind, key: PartKey::Path(path(limits, bytes)?) })
}

fn path(limits: &Limits, bytes: &[Box<[u8]>]) -> Result<List<Box<[u8]>>, Missing> {
    if bytes.len() > usize::try_from(limits.documents.segments).expect("segment count fits") {
        return Err(Missing::TooLarge);
    }
    let mut result = List::with_capacity(limits.documents.segments);
    for segment in bytes {
        if segment.len() > usize::try_from(limits.documents.text_bytes).expect("text bound fits") {
            return Err(Missing::TooLarge);
        }
        result.push(skein_lib::bytes::copy_of(segment)).expect("validated path length");
    }
    Ok(result)
}

fn pattern(limits: &Limits, value: &tasks::Pattern) -> Result<PatternEnd, Missing> {
    let bytes = match &value.last {
        tasks::Last::Exact(bytes) | tasks::Last::Open(bytes) => bytes,
    };
    if bytes.len() > usize::try_from(limits.documents.text_bytes).expect("text bound fits") {
        return Err(Missing::TooLarge);
    }
    Ok(match &value.last {
        tasks::Last::Exact(bytes) => PatternEnd::Exact(skein_lib::bytes::copy_of(bytes)),
        tasks::Last::Open(bytes) => PatternEnd::Open(skein_lib::bytes::copy_of(bytes)),
    })
}

pub(super) fn authority(limits: &Limits, value: &tasks::Authority) -> Result<AuthorityLine, Missing> {
    let mut grants = List::with_capacity(limits.documents.grants);
    for grant in &value.grants {
        grants
            .push(GrantLine {
                connector: grant.connector,
                kind: grant.kind,
                segments: path(limits, &grant.pattern.segments)?,
                last: pattern(limits, &grant.pattern)?,
            })
            .expect("bounded grants");
    }
    let mut executors = List::with_capacity(limits.documents.executors);
    for kind in &value.delegation.kinds {
        executors.push(*kind).expect("bounded executor list");
    }
    let mut note_resources = List::with_capacity(limits.documents.grants);
    for scope in &value.note_resources {
        note_resources
            .push(NoteScope {
                connector: scope.connector,
                segments: path(limits, &scope.pattern.segments)?,
                last: pattern(limits, &scope.pattern)?,
            })
            .expect("bounded note scopes");
    }
    Ok(AuthorityLine {
        models: value.models,
        tools: value.tools,
        grants,
        executors,
        tasks: value.delegation.tasks,
        depth: value.delegation.depth,
        ceiling: value.budget,
        notes: value.notes,
        note_resources,
    })
}

pub(super) fn contract(limits: &Limits, value: &tasks::Contract) -> Result<ContractLine, Missing> {
    Ok(match value {
        tasks::Contract::Approval { state, part } => {
            if state.len() > usize::try_from(limits.documents.text_bytes).expect("u32 fits usize") {
                return Err(Missing::TooLarge);
            }
            match &part.key {
                PartKey::Number(_) => {}
                PartKey::Path(segments) => {
                    if segments.capacity() > limits.documents.segments {
                        return Err(Missing::TooLarge);
                    }
                    for segment in segments {
                        if segment.len() > usize::try_from(limits.documents.text_bytes).expect("u32 fits usize") {
                            return Err(Missing::TooLarge);
                        }
                    }
                }
            }
            ContractLine::Approval { state: state.clone(), part: part.clone() }
        }
        tasks::Contract::Report { words } => ContractLine::Report { words: *words },
        tasks::Contract::Change { connector, kind, words } => {
            ContractLine::Change { connector: *connector, kind: *kind, words: *words }
        }
        tasks::Contract::Verdict { choices } => {
            let mut result = List::with_capacity(limits.documents.lines);
            for choice in choices.iter().take(bound(limits)) {
                if choice.fields.len() > bound(limits) {
                    return Err(Missing::TooLarge);
                }
                let mut names = 0_usize;
                for field in &choice.fields {
                    names = names.checked_add(field.name.len()).ok_or(Missing::TooLarge)?;
                }
                if names > usize::try_from(limits.documents.text_bytes).expect("u32 fits usize") {
                    return Err(Missing::TooLarge);
                }
                result.push(choice.clone()).expect("bounded form choices");
            }
            ContractLine::Verdict { cut: size(result.len()) < choices.len(), choices: result }
        }
    })
}

pub(super) fn result_value(limits: &Limits, value: &tasks::TaskResult) -> Result<ResultValue, Missing> {
    Ok(match value {
        tasks::TaskResult::Approved { state, words } => {
            if state.len() > usize::try_from(limits.documents.text_bytes).expect("u32 fits usize") {
                return Err(Missing::TooLarge);
            }
            ResultValue::Approved {
                state: state.clone(),
                words: match words {
                    Some(words) => Some(super::small::text(words, limits.documents.text_bytes)),
                    None => None,
                },
            }
        }
        tasks::TaskResult::Report { words } => {
            ResultValue::Report(super::small::text(words, limits.documents.text_bytes))
        }
        tasks::TaskResult::Verdict { code, words, fields } => {
            let mut total = 0_usize;
            if fields.len() > bound(limits) {
                return Err(Missing::TooLarge);
            }
            for field in fields {
                total = total
                    .checked_add(field.name.len())
                    .ok_or(Missing::TooLarge)?
                    .checked_add(field.value.len())
                    .ok_or(Missing::TooLarge)?;
            }
            if total > usize::try_from(limits.documents.text_bytes).expect("u32 fits usize") {
                return Err(Missing::TooLarge);
            }
            ResultValue::Verdict {
                code: *code,
                fields: fields.clone(),
                words: super::small::text(words, limits.documents.text_bytes),
            }
        }
        tasks::TaskResult::Change { connector, kind, resource, words } => ResultValue::Change {
            kind: *kind,
            part: Part { connector: *connector, kind: PartKind::Result, key: PartKey::Number(*resource) },
            words: super::small::text(words, limits.documents.text_bytes),
        },
        tasks::TaskResult::Failure { reason } => {
            ResultValue::Failure(super::small::text(reason, limits.documents.text_bytes))
        }
    })
}

pub(super) fn ending(limits: &Limits, value: &tasks::Ending) -> Result<Ending, Missing> {
    Ok(match value {
        tasks::Ending::Done(result) => Ending::Done(result_value(limits, result)?),
        tasks::Ending::Failed { reason } => Ending::Failed(super::small::text(reason, limits.documents.text_bytes)),
        tasks::Ending::Cancelled { reason, result, by, at } => Ending::Cancelled {
            by: *by,
            at: *at,
            reason: super::small::text(reason, limits.documents.text_bytes),
            result: match result {
                Some(result) => Some(result_value(limits, result)?),
                None => None,
            },
        },
    })
}

pub(super) fn hold_source(limits: &Limits, source: Option<&tasks::HoldSource>) -> Result<Option<Part>, Missing> {
    match source {
        None | Some(tasks::HoldSource::Circle(_)) => Ok(None),
        Some(tasks::HoldSource::Resource(name)) => {
            Ok(Some(part_path(limits, name.connector, PartKind::Resource, &name.path)?))
        }
        Some(tasks::HoldSource::Effect { connector, entry }) => {
            Ok(Some(Part { connector: *connector, kind: PartKind::Outbox, key: PartKey::Number(*entry) }))
        }
    }
}

// client/domain/root.md, section 6: the core names the hold account from retained origin facts.
fn hold_account(task: u64, reason: tasks::Hold, source: Option<&tasks::HoldSource>) -> Option<Part> {
    match reason {
        tasks::Hold::Drift | tasks::Hold::Effects | tasks::Hold::EffectFailed => {
            let connector = match source {
                Some(tasks::HoldSource::Resource(name)) => name.connector,
                Some(tasks::HoldSource::Effect { connector, .. }) => *connector,
                None | Some(tasks::HoldSource::Circle(_)) => return None,
            };
            Some(Part { connector, kind: PartKind::Hold, key: PartKey::Number(task) })
        }
        tasks::Hold::Exceeded(_)
        | tasks::Hold::Failures(_)
        | tasks::Hold::Stalled
        | tasks::Hold::Dependency(_)
        | tasks::Hold::Stopped
        | tasks::Hold::Placed { .. }
        | tasks::Hold::Uncertain { .. }
        | tasks::Hold::Budget
        | tasks::Hold::Deadline
        | tasks::Hold::HoldsWaited
        | tasks::Hold::ResourceUnavailable
        | tasks::Hold::ResourceAccess
        | tasks::Hold::PoolLost { .. }
        | tasks::Hold::Circle
        | tasks::Hold::Procedure => None,
    }
}
pub(super) fn hold(limits: &Limits, row: &tasks::TaskRecord) -> Result<Option<HoldLine>, Missing> {
    match &row.phase {
        tasks::Phase::Held { was, why, source } => Ok(Some(HoldLine {
            circle: match source.as_deref() {
                Some(tasks::HoldSource::Circle(circle)) => {
                    let mut tasks = List::with_capacity(limits.documents.lines);
                    for number in circle.tasks.iter().take(bound(limits)) {
                        tasks.push(*number).expect("bounded circle chips");
                    }
                    Some(tasks::Circle {
                        cut: circle.cut || circle.tasks.len() > bound(limits),
                        tasks: tasks.into_boxed(),
                    })
                }
                None | Some(tasks::HoldSource::Resource(_) | tasks::HoldSource::Effect { .. }) => None,
            },
            account: hold_account(row.number, *why, source.as_deref()),
            source: hold_source(limits, source.as_deref())?,
            reason: *why,
            revision: row.revisions.hold,
            before: match was {
                tasks::Was::Waiting => PriorPhase::Waiting,
                tasks::Was::Active(active) => PriorPhase::Active(*active),
                tasks::Was::Closing(closing) => PriorPhase::Closing(closing.stage),
            },
            escalation: match &row.escalation {
                tasks::Escalation::Unheld { .. } => None,
                tasks::Escalation::Routing { .. } => Some(EscalationState::Routing),
                tasks::Escalation::Waiting { holder, entry, since, .. } => {
                    Some(EscalationState::Waiting { holder: *holder, entry: *entry, since: *since })
                }
                tasks::Escalation::Rejected { by, reason, .. } => Some(EscalationState::Rejected {
                    by: *by,
                    reason: super::small::text(reason, limits.documents.text_bytes),
                }),
            },
            tries: row.tries,
            hold_wait_since: row.hold_wait_since,
        })),
        tasks::Phase::Waiting | tasks::Phase::Active(_) | tasks::Phase::Closing(_) | tasks::Phase::Ended(_) => Ok(None),
    }
}

pub(super) fn budget(row: &tasks::TaskRecord) -> BudgetLine {
    let left = row
        .numbers
        .budget
        .checked_sub(row.numbers.spent)
        .expect("spend within allotment")
        .checked_sub(row.numbers.spent_below)
        .expect("settled spend within allotment")
        .checked_sub(row.numbers.reserved)
        .expect("reservations within remaining allotment");
    BudgetLine { numbers: row.numbers, left, funder: row.funder, deadline: row.authority.budget.deadline }
}

fn bound(limits: &Limits) -> usize {
    size(limits.documents.lines)
}
fn size(value: u32) -> usize {
    usize::try_from(value).expect("bounded count fits usize")
}

pub(super) fn link(core: &Core, limits: &Limits, party: u64, number: u64) -> Result<TaskLink, Missing> {
    match core.tasks.task(number) {
        Some(_) => Ok(TaskLink::Live(super::small::chip(core, limits, party, number)?)),
        None => Ok(TaskLink::Stored {
            number,
            status: match core.tasks.stub(number) {
                Some(stub) => Some(stub.phase),
                None => None,
            },
        }),
    }
}

fn links(core: &Core, limits: &Limits, party: u64, numbers: &[u64]) -> Result<List<TaskLink>, Missing> {
    let mut result = List::with_capacity(limits.documents.lines);
    for number in numbers.iter().take(bound(limits)) {
        result.push(link(core, limits, party, *number)?).expect("bounded task links");
    }
    Ok(result)
}

pub(super) fn descendant(core: &Core, number: u64, root: u64, depth: u32) -> bool {
    let mut current = number;
    for _ in 0..=depth {
        if current == root {
            return true;
        }
        let Some(row) = core.tasks.task(current) else { return false };
        match row.requester {
            tasks::Party::Task(parent) => current = parent,
            tasks::Party::Person(_) | tasks::Party::Deployment { .. } => return false,
        }
    }
    false
}

fn proposal(row: &tasks::TaskRecord) -> Option<ProposalRef> {
    let value = row.proposal.as_ref()?;
    match value.state {
        tasks::ProposalState::Pending { .. } => {
            Some(ProposalRef { proposer: Proposer::Task(value.proposer), number: value.number })
        }
        tasks::ProposalState::Accepted { .. }
        | tasks::ProposalState::Rejected { .. }
        | tasks::ProposalState::Withdrawn => None,
    }
}

pub(super) fn task(core: &Core, limits: &Limits, party: u64, number: u64, decision: Wall) -> Result<Task, Missing> {
    let row = super::small::visible(core, party, number)?;
    task_from_row(core, limits, party, row, decision)
}

/// Construct a visible task from its retained owner row, without restoring it.
#[expect(clippy::too_many_lines, reason = "the task page gathers one bounded overview without changing child state")]
pub fn task_from_row(
    core: &Core,
    limits: &Limits,
    party: u64,
    row: &tasks::TaskRecord,
    decision: Wall,
) -> Result<Task, Missing> {
    if core.people.role(party, row.project).is_none() {
        return Err(Missing::NotAllowed);
    }
    let number = row.number;
    let mut lineage = List::with_capacity(limits.documents.lines);
    let mut ancestor = row.requester;
    let mut cut = false;
    let goal = core.goal_of(row);
    for _ in 0..limits.tasks.depth {
        match ancestor {
            tasks::Party::Task(parent) => {
                let parent_row = match super::small::visible(core, party, parent) {
                    Ok(row) => row,
                    Err(Missing::Unknown | Missing::Ended) => {
                        cut = true;
                        break;
                    }
                    Err(why @ (Missing::NotAllowed | Missing::TooLarge)) => return Err(why),
                };
                if lineage.room() > 0 {
                    lineage.push(super::small::chip(core, limits, party, parent)?).expect("lineage room checked");
                } else {
                    cut = true;
                }
                ancestor = parent_row.requester;
            }
            tasks::Party::Person(_) | tasks::Party::Deployment { .. } => break,
        }
    }
    let mut ordered = List::with_capacity(limits.documents.lines);
    for index in (0..lineage.len()).rev() {
        ordered.push(lineage.get(index).expect("lineage index").clone()).expect("same bounded lineage");
    }
    let mut parameters = List::with_capacity(limits.documents.lines);
    for value in row.spec.parameters.iter().take(bound(limits)) {
        let parameter = match value {
            tasks::Parameter::Number { name, value } => Parameter::Number { name: *name, value: *value },
            tasks::Parameter::Bytes { name, value } => {
                Parameter::Bytes { name: *name, value: super::small::text(value, limits.documents.text_bytes) }
            }
            tasks::Parameter::Resource { name, connector, resource } => Parameter::Resource {
                name: *name,
                part: Part { connector: *connector, kind: PartKind::Resource, key: PartKey::Number(*resource) },
            },
        };
        parameters.push(parameter).expect("bounded parameters");
    }
    let mut resources = List::with_capacity(limits.documents.lines);
    for value in row.holdings.iter().take(bound(limits)) {
        let (name, kind, part_kind) = match value {
            tasks::Holding::Write { resource, kind } => (resource, *kind, PartKind::Resource),
            tasks::Holding::Slot { pool, kind } => (pool, *kind, PartKind::Pool),
        };
        let slot = core.tasks.writer(name);
        resources
            .push(ResourceLine {
                part: part_path(limits, name.connector, part_kind, &name.path)?,
                kind,
                taken: row.holds_taken,
                holder: core.tasks.holder(name),
                writer: match slot {
                    Some(slot) => Some(slot.writer),
                    None => None,
                },
                writer_lost: match slot {
                    Some(slot) => slot.lost,
                    None => false,
                },
            })
            .expect("bounded resource lines");
    }
    let mut proposals = List::with_capacity(limits.documents.lines);
    for candidate in core.tasks.view_tasks().as_ref() {
        if descendant(core, candidate.number, number, limits.tasks.depth)
            && let Some(reference) = proposal(core.tasks.task(candidate.number).expect("live indexed task"))
        {
            if proposals.room() == 0 {
                cut = true;
            } else {
                proposals.push(reference).expect("proposal room checked");
            }
        }
    }
    let mut subscriptions = List::with_capacity(limits.documents.lines);
    for value in row.subscriptions.iter().take(bound(limits)) {
        subscriptions.push(*value).expect("bounded subscriptions");
    }
    cut |= size(parameters.len()) < row.spec.parameters.len()
        || size(resources.len()) < row.holdings.len()
        || size(subscriptions.len()) < row.subscriptions.len()
        || row.spec.inputs.len() > bound(limits)
        || row.dependencies.len() > bound(limits)
        || row.references.len() > bound(limits);
    let result = match &row.phase {
        tasks::Phase::Closing(closing) => Some(ending(limits, &closing.ending)?),
        tasks::Phase::Held { was, .. } => match was {
            tasks::Was::Closing(closing) => Some(ending(limits, &closing.ending)?),
            tasks::Was::Waiting | tasks::Was::Active(_) => None,
        },
        tasks::Phase::Ended(value) => Some(ending(limits, value)?),
        tasks::Phase::Waiting | tasks::Phase::Active(_) => None,
    };
    let acts = super::offers::task(core, limits, party, row, decision)?;
    let authority = authority(limits, &row.authority)?;
    Ok(Task {
        settings: match row.executor {
            tasks::Executor::Agent { .. } => Some(TaskSettings {
                task: row.settings.task,
                run: match row.settings.run {
                    Some(run) if run != row.settings.task => Some(run),
                    Some(_) | None => None,
                },
                newest: core.current.newest(),
            }),
            tasks::Executor::Procedure { .. } | tasks::Executor::Person(_) => None,
        },
        waits_on: crate::waits::from_row(core, limits, row),
        label: row.label,
        flags: row.flags,
        last_activity: row.last_activity,
        acts,
        chip: super::small::chip_with_core(core, limits, row)?,
        project: row.project,
        goal,
        priority: row.tracked,
        executor: row.executor,
        requester: row.requester,
        lineage: ordered,
        delegates: super::live_delegates::from_row(core, limits, party, row)?,
        changed: row.changed,
        spec_revision: row.revisions.spec,
        person_revision: match row.executor {
            tasks::Executor::Person(_) => Some(row.revisions.person),
            tasks::Executor::Agent { .. } | tasks::Executor::Procedure { .. } => None,
        },
        taken_by: row.taken_by,
        activity: row.activity,
        spec: super::objects::spec(limits, &row.spec),
        words: super::small::text(&row.spec.words, limits.documents.text_bytes),
        parameters,
        instructions: match row.executor {
            tasks::Executor::Agent { charter } => {
                let value = match &row.instructions {
                    Some(instructions) => Some(instructions),
                    None => match core.charter_at(charter, row.settings.task) {
                        Some(charter) => Some(&charter.policy.instructions),
                        None => None,
                    },
                };
                match value {
                    Some(value) => Some(super::small::text(value, limits.documents.text_bytes)),
                    None => None,
                }
            }
            tasks::Executor::Procedure { .. } | tasks::Executor::Person(_) => None,
        },
        llm: row.llm.clone(),
        kept_account: row.kept_account,
        run_wait: super::task_wait::waiting(core, row),
        resources,
        inputs: links(core, limits, party, &row.spec.inputs)?,
        dependencies: links(core, limits, party, &row.dependencies)?,
        dependency_facts: dependency_facts(core, limits, row),
        references: links(core, limits, party, &row.references)?,
        subscriptions,
        wake: row.wake,
        contract: contract(limits, &row.contract)?,
        result,
        budget: budget(row),
        authority,
        hold: hold(limits, row)?,
        proposals,
        cut,
    })
}

pub(super) fn tree(core: &Core, limits: &Limits, party: u64, number: u64, window: &Window) -> Result<Tree, Missing> {
    let root = super::small::visible(core, party, number)?;
    let mut rows = List::with_capacity(window.rows);
    let mut phases = PhaseCounts::ZERO;
    let mut held_below = 0_u32;
    let mut folded_total = 0_u32;
    let mut last = None;
    let after = match window.after {
        Some(Cursor::Number { number }) => number,
        Some(Cursor::Inbox { .. }) => return Err(Missing::Unknown),
        None => 0,
    };
    let mut more = false;
    for candidate in core.tasks.view_tasks().as_ref() {
        if candidate.number == number || !descendant(core, candidate.number, number, limits.tasks.depth) {
            continue;
        }
        let row = super::small::visible(core, party, candidate.number)?;
        let held = match row.phase {
            tasks::Phase::Held { .. } => true,
            tasks::Phase::Waiting | tasks::Phase::Active(_) | tasks::Phase::Closing(_) | tasks::Phase::Ended(_) => {
                false
            }
        };
        if held {
            held_below = held_below.checked_add(1).expect("bounded held count");
        }
        if rows.room() == 0 || candidate.number <= after {
            phases.add(&row.phase);
            folded_total = folded_total.checked_add(1).expect("bounded folded count");
            more |= candidate.number > after;
            continue;
        }
        let parent = match row.requester {
            tasks::Party::Task(parent) => parent,
            tasks::Party::Person(_) | tasks::Party::Deployment { .. } => {
                unreachable!("descendant has a requester task")
            }
        };
        let mut proposals = List::with_capacity(1);
        if let Some(value) = proposal(row) {
            proposals.push(value).expect("one proposal per task");
        }
        rows.push(TreeRow {
            chip: super::small::chip(core, limits, party, row.number)?,
            parent,
            depth: row.depth.checked_sub(root.depth).expect("subtree depth"),
            proposals,
            spent: row.numbers.spent.checked_add(row.numbers.spent_below).expect("bounded spent"),
            dependencies: links(core, limits, party, &row.dependencies)?,
            cut: row.dependencies.len() > bound(limits),
        })
        .expect("tree window room checked");
        last = Some(Cursor::Number { number: row.number });
    }
    let mut folded = List::with_capacity(1);
    if folded_total > 0 {
        folded.push(FoldedCount { parent: number, held_below: phases.held, phases }).expect("one fold at window root");
    }
    Ok(Tree {
        root: super::small::chip(core, limits, party, number)?,
        rows,
        folded,
        held_below,
        next: if more { last } else { None },
    })
}

pub(super) fn overview_worst_case(limits: &super::DocumentLimits) -> Option<u64> {
    let text = u64::from(limits.text_bytes);
    let lines = u64::from(limits.lines);
    let rows = u64::from(limits.rows);
    let chip = super::chip_phase::heap_worst_case(limits)?;
    let path =
        List::<Box<[u8]>>::worst_case(limits.segments)?.checked_add(u64::from(limits.segments).checked_mul(text)?)?;
    let authority = authority_worst_case(limits)?;
    let references = List::<TaskLink>::worst_case(limits.lines)?.checked_add(lines.checked_mul(chip)?)?;
    let specification = text
        .checked_add(List::<Parameter>::worst_case(limits.lines)?)?
        .checked_add(lines.checked_mul(text)?)?
        .checked_add(List::<u64>::worst_case(limits.lines)?)?;
    let task = u64::try_from(size_of::<Task>())
        .ok()?
        .checked_add(specification)?
        .checked_add(text.checked_mul(7)?)?
        .checked_add(path.checked_mul(2)?)?
        .checked_add(chip)?
        .checked_add(List::<super::LiveDelegate>::worst_case(limits.lines)?)?
        .checked_add(lines.checked_mul(chip)?)?
        .checked_add(List::<Chip>::worst_case(limits.lines)?.checked_add(lines.checked_mul(chip)?)?.checked_mul(2)?)?
        .checked_add(List::<Parameter>::worst_case(limits.lines)?.checked_add(lines.checked_mul(text)?)?)?
        .checked_add(List::<ResourceLine>::worst_case(limits.lines)?.checked_add(lines.checked_mul(path)?)?)?
        .checked_add(references.checked_mul(3)?)?
        .checked_add(List::<tasks::Subscription>::worst_case(limits.lines)?)?
        .checked_add(List::<tasks::Verdict>::worst_case(limits.lines)?)?
        .checked_add(
            lines.checked_mul(
                lines.checked_mul(u64::try_from(size_of::<tasks::FieldRule>()).ok()?)?.checked_add(text)?,
            )?,
        )?
        .checked_add(lines.checked_mul(u64::try_from(size_of::<tasks::ResultField>()).ok()?)?.checked_add(text)?)?
        .checked_add(List::<Dependency>::worst_case(limits.lines)?)?
        .checked_add(List::<u64>::worst_case(limits.lines)?)?
        .checked_add(32 * 8)?
        .checked_add(List::<ProposalRef>::worst_case(limits.lines)?)?
        .checked_add(u64::from(tasks::MODELS).checked_mul(u64::try_from(size_of::<tasks::ModelEffort>()).ok()?)?)?
        .checked_add(authority)?;
    let tree = chip
        .checked_add(List::<TreeRow>::worst_case(limits.rows)?)?
        .checked_add(
            rows.checked_mul(chip.checked_add(references)?.checked_add(List::<ProposalRef>::worst_case(1)?)?)?,
        )?
        .checked_add(List::<FoldedCount>::worst_case(1)?)?;
    Some(task.max(tree))
}

pub(super) fn grant_worst_case(limits: &super::DocumentLimits) -> Option<u64> {
    List::<Box<[u8]>>::worst_case(limits.segments)?
        .checked_add(u64::from(limits.segments).checked_add(1)?.checked_mul(u64::from(limits.text_bytes))?)
}
pub(super) fn authority_worst_case(limits: &super::DocumentLimits) -> Option<u64> {
    List::<GrantLine>::worst_case(limits.grants)?
        .checked_add(List::<NoteScope>::worst_case(limits.grants)?)?
        .checked_add(u64::from(limits.grants).checked_mul(grant_worst_case(limits)?)?.checked_mul(2)?)?
        .checked_add(List::<tasks::AuthorityExecutor>::worst_case(limits.executors)?)
}

pub(super) fn worst_case(limits: &super::DocumentLimits) -> Option<u64> {
    overview_worst_case(limits)?.checked_add(super::offers::heap_worst_case(limits)?)
}

pub(super) fn dependency_facts(core: &Core, limits: &Limits, row: &tasks::TaskRecord) -> List<Dependency> {
    let mut facts = dependency_rows(limits, row);
    for index in 0..facts.len() {
        let fact = facts.get_mut(index).expect("bounded dependency index");
        fact.status = match core.tasks.task(fact.number) {
            Some(task) => match &task.phase {
                tasks::Phase::Ended(ending) => Some(match ending {
                    tasks::Ending::Done(_) => tasks::Status::Done,
                    tasks::Ending::Failed { .. } => tasks::Status::Failed,
                    tasks::Ending::Cancelled { .. } => tasks::Status::Cancelled,
                }),
                tasks::Phase::Waiting
                | tasks::Phase::Active(_)
                | tasks::Phase::Held { .. }
                | tasks::Phase::Closing(_) => None,
            },
            None => match core.tasks.stub(fact.number) {
                Some(stub) => Some(stub.phase),
                None => None,
            },
        };
    }
    facts
}
pub(super) fn dependency_rows(limits: &Limits, row: &tasks::TaskRecord) -> List<Dependency> {
    let mut facts = List::with_capacity(limits.documents.lines);
    let keeps = match row.phase {
        tasks::Phase::Active(_) => true,
        tasks::Phase::Waiting | tasks::Phase::Held { .. } | tasks::Phase::Closing(_) | tasks::Phase::Ended(_) => false,
    } && !row.waiting_on.is_empty();
    for number in row.dependencies.iter().take(bound(limits)) {
        let mut added = None;
        for value in &row.added {
            if value.task == *number {
                added = Some(value.clone());
                break;
            }
        }
        facts
            .push(Dependency {
                number: *number,
                pending: row.waiting_on.contains(number),
                added,
                keeps: keeps && row.waiting_on.contains(number),
                status: None,
            })
            .expect("bounded dependency facts");
    }
    facts
}

/// A retained ended root has no live subtree (client/domain/contract.md, 7.2).
pub(crate) fn ended_tree(core: &Core, limits: &Limits, party: u64, row: &tasks::TaskRecord) -> Result<Tree, Missing> {
    if core.people.role(party, row.project).is_none() {
        return Err(Missing::NotAllowed);
    }
    match row.phase {
        tasks::Phase::Ended(_) => {}
        tasks::Phase::Waiting | tasks::Phase::Active(_) | tasks::Phase::Closing(_) | tasks::Phase::Held { .. } => {
            return Err(Missing::Unknown);
        }
    }
    Ok(Tree {
        root: super::small::chip_with_core(core, limits, row)?,
        rows: List::with_capacity(0),
        folded: List::with_capacity(0),
        held_below: 0,
        next: None,
    })
}
