//! Decision objects and retained rows (`client/domain/objects.md`, sections 4–8;
//! `client/domain/pages.md`, 5.2). Store readers supply retained rows. Routing
//! trails and before/after history require those store rows; no trail is cached.
#![expect(clippy::manual_map, reason = "step code uses explicit matches instead of closures")]
use super::{
    AuthorityLine, Chip, ContractLine, Ending, HoldLine, Missing, Parameter, Part, PartKey, PartKind, PriorPhase,
    Proposer, Task, TaskLink, Text,
};
use crate::{Core, Limits};
use alloc::boxed::Box;
use jig_domain_tasks as tasks;
use skein_lib::{List, Wall};
mod party;

/// A specification without application values; loss is explicit on every text and list.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct SpecLine {
    pub words: Text,
    pub parameters: List<Parameter>,
    pub inputs: List<u64>,
    pub cut: bool,
}
/// One future batch member; dependencies retain allocated or local batch numbers.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ProposedTask {
    pub deadline: tasks::Deadline,
    pub number: u64,
    pub executor: tasks::Executor,
    pub spec: SpecLine,
    pub contract: ContractLine,
    pub authority: AuthorityLine,
    pub budget: u64,
    pub funder: tasks::Funder,
    pub dependencies: List<u64>,
    pub resources: List<Part>,
    pub wake: tasks::WakePolicy,
    pub priority: Option<u32>,
    pub cut: bool,
}
/// The relevant task fields before and after one proposed amendment.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct AmendmentState {
    pub addressed: Option<tasks::PersonAddress>,
    pub taken_by: Option<u64>,
    /// Known current instructions in proposals; exact retained override in history.
    pub instructions: Option<Text>,
    pub spec: SpecLine,
    pub authority: AuthorityLine,
    pub wake: tasks::WakePolicy,
    pub dependencies: List<u64>,
    pub cut: bool,
}
/// What accepting the immutable proposal would do.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum ProposalAction {
    Settings {
        against: u32,
        items: List<crate::reads::RevisionItemHead>,
        review: Option<Box<super::SettingsReview>>,
        made: Option<u32>,
    },
    Batch(List<ProposedTask>),
    Effect {
        part: Part,
        authority: AuthorityLine,
        attempt: u64,
        completion: u32,
        position: u32,
        purpose: u64,
    },
    ProcedureEffect {
        part: Part,
        authority: AuthorityLine,
        step: u64,
        purpose: u64,
    },
    Amend {
        task: TaskLink,
        addressed: Option<tasks::AddressChange>,
        before: Box<AmendmentState>,
        after: Box<AmendmentState>,
        reason: Text,
        release: Option<u64>,
    },
    Widen {
        task: TaskLink,
        before: AuthorityLine,
        after: AuthorityLine,
    },
    Release {
        task: TaskLink,
        hold: Option<HoldLine>,
    },
}
/// Current recipient; earlier recipients are retained proposal history rows.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ProposalWaiting {
    pub holder: tasks::ProposalHolder,
    pub since: Wall,
}
/// Funding facts before an acceptance; absent funds have never been allocated.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct FundingOffer {
    pub source: tasks::Funder,
    pub needed: u64,
    pub left: Option<u64>,
    pub after: Option<u64>,
}
/// A pending proposal. Acts and a store-loaded routing trail are supplied later.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Proposal {
    pub acts: List<crate::acts::Offered>,
    pub proposer: Proposer,
    pub number: u64,
    pub project: u32,
    pub reason: Text,
    pub action: ProposalAction,
    pub as_holder: bool,
    pub waiting: ProposalWaiting,
    pub funding: List<FundingOffer>,
}
/// The full task overview supplies the hold's tries, resources, dependencies,
/// budget, previous phase and kept result without duplicating a second snapshot.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Escalation {
    pub acts: List<crate::acts::Offered>,
    pub task: Box<Task>,
}
/// An unanswered question's words are whole; over its byte bound it is refused.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Question {
    pub acts: List<crate::acts::Offered>,
    pub task: Chip,
    pub message: u64,
    pub from: tasks::Party,
    pub at: Wall,
    pub words: Box<[u8]>,
}
/// A shown dependency result; content not in live memory is named for a store read.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ShownResult {
    pub task: TaskLink,
    pub result: Option<ResultDoc>,
}
/// The person task's current spec, address, form and shown results.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct PersonTask {
    pub acts: List<crate::acts::Offered>,
    pub task: Chip,
    pub revision: u64,
    pub requester: tasks::Party,
    pub spec: SpecLine,
    pub address: tasks::PersonAddress,
    pub taken_by: Option<u64>,
    pub since: Option<Wall>,
    pub stall: Option<skein_lib::Duration>,
    pub deadline: Option<Wall>,
    pub contract: ContractLine,
    pub results: List<ShownResult>,
    pub cut: bool,
}
/// One result by its contract; a store-loaded row supplies ended outcomes.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ResultDoc {
    pub acts: List<crate::acts::Offered>,
    pub task: Chip,
    pub contract: ContractLine,
    pub ending: Ending,
    pub at: Option<Wall>,
    pub requester: tasks::Party,
    pub executor: tasks::Executor,
    pub chosen_by: Option<u64>,
    /// Exact result-created tasks, including those already ended or taken away.
    /// The owner always supplies `Some`, including a known empty batch.
    pub followups: Option<List<TaskLink>>,
    pub delegates: List<TaskLink>,
    pub proposal: Option<super::ProposalRef>,
    pub cut: bool,
}
/// Timestamped semantic history with immutable amendment settings.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct HistoryLine {
    pub task: u64,
    pub revision: u64,
    pub by: tasks::Party,
    pub change: tasks::Change,
    pub reason: Text,
    pub proposal: Option<ProposalHistory>,
    pub amendment: Option<(Box<AmendmentState>, Box<AmendmentState>)>,
    pub at: Option<Wall>,
}
/// Proposal facts retained with a history row, including its recipient or decision.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ProposalHistory {
    pub action: RetainedAction,
    pub number: u64,
    pub proposer: u64,
    pub as_holder: bool,
    pub state: ProposalHistoryState,
}
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum ProposalHistoryState {
    Pending(ProposalWaiting),
    Accepted { by: tasks::Party },
    Rejected { by: tasks::Party, reason: Text },
    Withdrawn,
}
/// The immutable action kept by a proposal history row; an amendment names
/// only its requested fields because no before snapshot is retained there.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum RetainedAction {
    Settings { against: u32, items: List<crate::reads::RevisionItemHead>, made: Option<u32> },
    Batch(List<ProposedTask>),
    Effect { part: Part, authority: AuthorityLine, attempt: u64, completion: u32, position: u32, purpose: u64 },
    ProcedureEffect { part: Part, authority: AuthorityLine, step: u64, purpose: u64 },
    Amend { task: u64, change: Box<AmendmentChange> },
    Widen { task: u64, authority: AuthorityLine },
    Release { task: u64 },
}
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct AmendmentChange {
    pub addressed: Option<tasks::AddressChange>,
    pub instructions: Option<Text>,
    pub release: Option<u64>,
    pub spec: Option<SpecLine>,
    pub authority: Option<AuthorityLine>,
    pub wake: Option<tasks::WakePolicy>,
    pub dependencies: Option<List<u64>>,
    pub reason: Text,
    pub cut: bool,
}
/// Timestamped lifecycle row named by a task activity position.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct MilestoneLine {
    pub task: u64,
    pub position: u64,
    pub phase: MilestonePhase,
    pub at: Option<Wall>,
}
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum MilestonePhase {
    Waiting,
    Active(tasks::Active),
    Closing { stage: tasks::Stage, ending: Ending },
    Held { before: PriorPhase, why: tasks::Hold, source: Option<Part>, ending: Option<Ending> },
    Ended(Ending),
}
/// The ended delegate's identity and outcome, made from its retained task row.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct EndedDelegate {
    pub chip: Chip,
    pub ended: Ending,
    pub at: Option<Wall>,
}

fn bound(limits: &Limits) -> usize {
    usize::try_from(limits.documents.lines).expect("line bound fits")
}
fn numbers(limits: &Limits, values: &[u64]) -> List<u64> {
    let mut result = List::with_capacity(limits.documents.lines);
    for value in values.iter().take(bound(limits)) {
        result.push(*value).expect("bounded numbers");
    }
    result
}
pub(super) fn spec(limits: &Limits, value: &tasks::Spec) -> SpecLine {
    let mut parameters = List::with_capacity(limits.documents.lines);
    for value in value.parameters.iter().take(bound(limits)) {
        parameters
            .push(match value {
                tasks::Parameter::Number { name, value } => Parameter::Number { name: *name, value: *value },
                tasks::Parameter::Bytes { name, value } => {
                    Parameter::Bytes { name: *name, value: super::small::text(value, limits.documents.text_bytes) }
                }
                tasks::Parameter::Resource { name, connector, resource } => Parameter::Resource {
                    name: *name,
                    part: Part { connector: *connector, kind: PartKind::Resource, key: PartKey::Number(*resource) },
                },
            })
            .expect("bounded spec parameters");
    }
    SpecLine {
        words: super::small::text(&value.words, limits.documents.text_bytes),
        parameters,
        inputs: numbers(limits, &value.inputs),
        cut: value.parameters.len() > bound(limits) || value.inputs.len() > bound(limits),
    }
}
pub(super) fn proposed(limits: &Limits, value: &tasks::New) -> Result<ProposedTask, Missing> {
    let mut resources = List::with_capacity(limits.documents.lines);
    for value in value.holdings.iter().take(bound(limits)) {
        let (name, kind) = match value {
            tasks::Holding::Write { resource, .. } => (resource, PartKind::Resource),
            tasks::Holding::Slot { pool, .. } => (pool, PartKind::Pool),
        };
        resources
            .push(super::task_tree::part_path(limits, name.connector, kind, &name.path)?)
            .expect("bounded holdings");
    }
    Ok(ProposedTask {
        deadline: value.deadline,
        number: value.number,
        executor: value.executor,
        spec: spec(limits, &value.spec),
        contract: super::task_tree::contract(limits, &value.contract)?,
        authority: super::task_tree::authority(limits, &value.authority)?,
        budget: value.numbers.budget,
        funder: value.funder,
        dependencies: numbers(limits, &value.dependencies),
        resources,
        wake: value.wake,
        priority: value.tracked,
        cut: value.dependencies.len() > bound(limits) || value.holdings.len() > bound(limits),
    })
}
fn amendment(
    core: &Core,
    limits: &Limits,
    row: &tasks::TaskRecord,
    change: Option<&tasks::Amendment>,
) -> Result<AmendmentState, Missing> {
    let (description, authority, wake, dependencies) = match change {
        None => (&row.spec, &row.authority, row.wake, &row.dependencies),
        Some(change) => (
            change.spec.as_ref().unwrap_or(&row.spec),
            change.authority.as_ref().unwrap_or(&row.authority),
            change.wake.unwrap_or(row.wake),
            change.dependencies.as_ref().unwrap_or(&row.dependencies),
        ),
    };
    Ok(AmendmentState {
        addressed: match change {
            Some(change) => match change.addressed {
                Some(change) => Some(change.address),
                None => match row.executor {
                    tasks::Executor::Person(address) => Some(address),
                    tasks::Executor::Agent { .. } | tasks::Executor::Procedure { .. } => None,
                },
            },
            None => match row.executor {
                tasks::Executor::Person(address) => Some(address),
                tasks::Executor::Agent { .. } | tasks::Executor::Procedure { .. } => None,
            },
        },
        taken_by: match change {
            Some(change) if change.addressed.is_some() => None,
            Some(_) | None => row.taken_by,
        },
        instructions: match row.executor {
            tasks::Executor::Agent { charter } => {
                let value = match change {
                    Some(change) => change.instructions.as_ref().or(row.instructions.as_ref()),
                    None => row.instructions.as_ref(),
                };
                let value = match value {
                    Some(value) => Some(value),
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
        spec: spec(limits, description),
        authority: super::task_tree::authority(limits, authority)?,
        wake,
        dependencies: numbers(limits, dependencies),
        cut: dependencies.len() > bound(limits),
    })
}
fn action(
    core: &Core,
    limits: &Limits,
    party: u64,
    number: u64,
    value: &tasks::ProposalAction,
) -> Result<ProposalAction, Missing> {
    Ok(match value {
        tasks::ProposalAction::Batch(batch) => {
            if batch.len() > bound(limits) {
                return Err(Missing::TooLarge);
            }
            let mut result = List::with_capacity(limits.documents.lines);
            for value in batch {
                result.push(proposed(limits, value)?).expect("whole bounded batch");
            }
            ProposalAction::Batch(result)
        }
        tasks::ProposalAction::Effect { connector, authority, attempt, completion, position, purpose } => {
            ProposalAction::Effect {
                part: Part { connector: *connector, kind: PartKind::Effect, key: PartKey::Number(number) },
                authority: super::task_tree::authority(limits, authority)?,
                attempt: *attempt,
                completion: *completion,
                position: *position,
                purpose: *purpose,
            }
        }
        tasks::ProposalAction::ProcedureEffect { connector, authority, step, purpose } => {
            ProposalAction::ProcedureEffect {
                part: Part { connector: *connector, kind: PartKind::Effect, key: PartKey::Number(number) },
                authority: super::task_tree::authority(limits, authority)?,
                step: *step,
                purpose: *purpose,
            }
        }
        tasks::ProposalAction::Amend { task, amendment, .. } => {
            party::amendment(core, limits, party, *task, amendment)?
        }
        tasks::ProposalAction::Widen { task, authority } => ProposalAction::Widen {
            task: super::task_tree::link(core, limits, party, *task)?,
            before: super::task_tree::authority(limits, &super::small::visible(core, party, *task)?.authority)?,
            after: super::task_tree::authority(limits, authority)?,
        },
        tasks::ProposalAction::ReleasePlaced { task, .. } | tasks::ProposalAction::Release { task } => {
            ProposalAction::Release {
                task: super::task_tree::link(core, limits, party, *task)?,
                hold: super::task_tree::hold(limits, super::small::visible(core, party, *task)?)?,
            }
        }
    })
}
pub(super) fn left(value: tasks::Numbers) -> u64 {
    value
        .budget
        .checked_sub(value.spent)
        .expect("spend fits budget")
        .checked_sub(value.spent_below)
        .expect("settled spend fits")
        .checked_sub(value.reserved)
        .expect("reserved fits")
}
fn funding(core: &Core, source: tasks::Funder, needed: u64) -> FundingOffer {
    let balance = match source {
        tasks::Funder::Task(task) => match core.tasks.task(task) {
            Some(row) => Some(row.numbers),
            None => None,
        },
        tasks::Funder::Pool { .. } | tasks::Funder::Period { .. } | tasks::Funder::Recurring { .. } => {
            match core.tasks.funding(source) {
                Some(row) => Some(row.numbers),
                None => None,
            }
        }
    };
    let left = match balance {
        Some(value) => Some(left(value)),
        None => None,
    };
    FundingOffer {
        source,
        needed,
        left,
        after: match left {
            Some(left) => left.checked_sub(needed),
            None => None,
        },
    }
}
fn holder_source(core: &Core, party: u64, project: u32, holder: tasks::ProposalHolder) -> tasks::Funder {
    match holder {
        tasks::ProposalHolder::Task(number) => tasks::Funder::Task(number),
        tasks::ProposalHolder::Person(person) => tasks::Funder::Pool { project, person, period: core.period(project) },
        tasks::ProposalHolder::Policy { .. } => {
            tasks::Funder::Pool { project, person: party, period: core.period(project) }
        }
    }
}
fn reviewed_party_action(
    core: &Core,
    limits: &Limits,
    party: u64,
    value: &tasks::PersonProposal,
) -> Result<(ProposalAction, tasks::ProposalKind), Missing> {
    let (mut action, kind) = party::action(core, limits, party, &value.action)?;
    match &mut action {
        ProposalAction::Settings { review, .. } => {
            *review = super::settings_review::review(core, limits, value.proposer, value.number);
        }
        ProposalAction::Batch(_)
        | ProposalAction::Effect { .. }
        | ProposalAction::ProcedureEffect { .. }
        | ProposalAction::Amend { .. }
        | ProposalAction::Widen { .. }
        | ProposalAction::Release { .. } => {}
    }
    Ok((action, kind))
}
pub(super) fn proposal(
    core: &Core,
    limits: &Limits,
    party: u64,
    proposer: Proposer,
    number: u64,
    decision: Wall,
) -> Result<Proposal, Missing> {
    let (project, reason, mut made, as_holder, waiting) = match proposer {
        Proposer::Task(task) => {
            let row = super::small::visible(core, party, task)?;
            let value = row.proposal.as_ref().ok_or(Missing::Ended)?;
            if value.number != number {
                return Err(Missing::Ended);
            }
            let waiting = match value.state {
                tasks::ProposalState::Pending { holder, since } => ProposalWaiting { holder, since },
                tasks::ProposalState::Accepted { .. }
                | tasks::ProposalState::Rejected { .. }
                | tasks::ProposalState::Withdrawn => return Err(Missing::Ended),
            };
            (
                value.project,
                super::small::text(&value.reason, limits.documents.text_bytes),
                action(core, limits, party, number, &value.action)?,
                value.as_holder,
                waiting,
            )
        }
        Proposer::Party(person) => {
            let value = core.tasks.person_proposal(person, number).ok_or(Missing::Ended)?;
            if party != person && core.people.role(party, value.project).is_none() {
                return Err(Missing::NotAllowed);
            }
            let since = match value.state {
                tasks::PersonProposalState::Pending { since } => since,
                tasks::PersonProposalState::SettingsAccepted { .. }
                | tasks::PersonProposalState::Accepted { .. }
                | tasks::PersonProposalState::Rejected { .. }
                | tasks::PersonProposalState::Withdrawn { .. } => {
                    return Err(Missing::Ended);
                }
            };
            let (action, kind) = reviewed_party_action(core, limits, party, &value)?;
            (
                value.project,
                super::small::text(&value.reason, limits.documents.text_bytes),
                action,
                kind != tasks::ProposalKind::Settings,
                ProposalWaiting { holder: tasks::ProposalHolder::Policy { project: value.project, kind }, since },
            )
        }
    };
    let mut offers = List::with_capacity(limits.documents.lines);
    let source = holder_source(core, party, project, waiting.holder);
    match &mut made {
        ProposalAction::Batch(batch) => {
            let mut needed = 0_u64;
            for index in 0..batch.len() {
                let member = batch.get_mut(index).expect("bounded batch index");
                needed = needed.checked_add(member.budget).ok_or(Missing::TooLarge)?;
                member.funder = source;
            }
            offers.push(funding(core, source, needed)).expect("one acceptance source");
        }
        ProposalAction::Settings { .. }
        | ProposalAction::Effect { .. }
        | ProposalAction::ProcedureEffect { .. }
        | ProposalAction::Release { .. } => {
            // Price belongs to the connector's effect description; authority's
            // budget is a ceiling, never a quoted effect price.
        }
        ProposalAction::Amend { task, .. } | ProposalAction::Widen { task, .. } => {
            let task = match task {
                TaskLink::Live(chip) | TaskLink::Retained { chip, .. } => chip.number,
                TaskLink::Stored { number, .. } => *number,
            };
            let row = core.tasks.task(task).ok_or(Missing::Ended)?;
            let ceiling = match &made {
                ProposalAction::Amend { after, .. } => after.authority.ceiling.spend,
                ProposalAction::Widen { after, .. } => after.ceiling.spend,
                ProposalAction::Settings { .. }
                | ProposalAction::Batch(_)
                | ProposalAction::Effect { .. }
                | ProposalAction::ProcedureEffect { .. }
                | ProposalAction::Release { .. } => {
                    unreachable!("matched authority action")
                }
            };
            let needed = if source == row.funder {
                ceiling.saturating_sub(row.numbers.budget)
            } else {
                ceiling.saturating_sub(row.numbers.spent).saturating_sub(row.numbers.spent_below)
            };
            offers.push(funding(core, source, needed)).expect("one widening source");
        }
    }
    Ok(Proposal {
        acts: super::offers::subject(core, limits, party, &super::Subject::Proposal { proposer, number }, decision)?,
        proposer,
        number,
        project,
        reason,
        action: made,
        as_holder,
        waiting,
        funding: offers,
    })
}
pub(super) fn escalation(
    core: &Core,
    limits: &Limits,
    party: u64,
    number: u64,
    decision: Wall,
) -> Result<Escalation, Missing> {
    let task = super::task_tree::task(core, limits, party, number, decision)?;
    if task.hold.is_none() {
        return Err(Missing::Ended);
    }
    Ok(Escalation {
        acts: super::offers::subject(core, limits, party, &super::Subject::Escalation { task: number }, decision)?,
        task: Box::new(task),
    })
}
pub(super) fn question(
    core: &Core,
    limits: &Limits,
    party: u64,
    number: u64,
    message: u64,
    decision: Wall,
) -> Result<Question, Missing> {
    let row = super::small::visible(core, party, number)?;
    if !crate::inbox::questions_live(&row.phase) {
        return Err(Missing::Ended);
    }
    for line in &row.question_lines {
        if line.message != message || line.answer.is_some() {
            continue;
        }
        let fact = core.question_activity.get(&(number, line.line)).ok_or(Missing::Ended)?;
        let tasks::Line::Words { by, kind: tasks::MessageKind::Question, words, .. } = &fact.line else {
            return Err(Missing::Unknown);
        };
        if words.len() > usize::try_from(limits.documents.text_bytes).expect("text bound fits") {
            return Err(Missing::TooLarge);
        }
        return Ok(Question {
            acts: super::offers::subject(
                core,
                limits,
                party,
                &super::Subject::Question { task: number, message },
                decision,
            )?,
            task: super::small::chip(core, limits, party, number)?,
            message,
            from: *by,
            at: fact.at,
            words: skein_lib::bytes::copy_of(words),
        });
    }
    Err(Missing::Ended)
}

pub(super) fn person(
    core: &Core,
    limits: &Limits,
    party: u64,
    number: u64,
    decision: Wall,
) -> Result<PersonTask, Missing> {
    let row = super::small::visible(core, party, number)?;
    person_from_row(core, limits, party, row, decision)
}

/// Construct a person form from its retained task row.
pub fn person_from_row(
    core: &Core,
    limits: &Limits,
    party: u64,
    row: &tasks::TaskRecord,
    decision: Wall,
) -> Result<PersonTask, Missing> {
    if core.people.role(party, row.project).is_none() {
        return Err(Missing::NotAllowed);
    }
    let address = match row.executor {
        tasks::Executor::Person(address) => address,
        tasks::Executor::Agent { .. } | tasks::Executor::Procedure { .. } => return Err(Missing::Unknown),
    };
    let mut results = List::with_capacity(limits.documents.lines);
    for task in row.dependencies.iter().chain(row.spec.inputs.iter()).take(bound(limits)) {
        results
            .push(ShownResult {
                task: super::task_tree::link(core, limits, party, *task)?,
                result: match core.tasks.task(*task) {
                    Some(row) => match result_from_row(core, limits, party, row, decision) {
                        Ok(result) => Some(result),
                        Err(Missing::Ended | Missing::Unknown) => None,
                        Err(Missing::NotAllowed) => return Err(Missing::NotAllowed),
                        Err(Missing::TooLarge) => return Err(Missing::TooLarge),
                    },
                    None => None,
                },
            })
            .expect("bounded dependency results");
    }
    Ok(PersonTask {
        acts: super::offers::person(core, limits, party, row, decision)?,
        task: super::small::chip_with_core(core, limits, row)?,
        revision: row.revisions.person,
        requester: row.requester,
        spec: spec(limits, &row.spec),
        address,
        taken_by: row.taken_by,
        since: match row.person_wait {
            Some(wait) => Some(wait.since),
            None => None,
        },
        stall: match row.person_wait {
            Some(wait) => Some(wait.stall),
            None => None,
        },
        deadline: match row.person_wait {
            Some(wait) => Some(wait.deadline),
            None => None,
        },
        contract: super::task_tree::contract(limits, &row.contract)?,
        results,
        cut: row.dependencies.len().checked_add(row.spec.inputs.len()).ok_or(Missing::TooLarge)? > bound(limits),
    })
}
/// Make a result from the row loaded by the store reader, or a live closing row.
pub fn result_from_row(
    core: &Core,
    limits: &Limits,
    party: u64,
    row: &tasks::TaskRecord,
    decision: Wall,
) -> Result<ResultDoc, Missing> {
    if core.people.role(party, row.project).is_none() {
        return Err(Missing::NotAllowed);
    }
    let value = match &row.phase {
        tasks::Phase::Ended(ending) => ending,
        tasks::Phase::Closing(closing) | tasks::Phase::Held { was: tasks::Was::Closing(closing), .. } => {
            &closing.ending
        }
        tasks::Phase::Waiting
        | tasks::Phase::Active(_)
        | tasks::Phase::Held { was: tasks::Was::Waiting | tasks::Was::Active(_), .. } => return Err(Missing::Ended),
    };
    let mut delegates = List::with_capacity(limits.documents.lines);
    for task in row.delegates.iter().take(bound(limits)) {
        delegates.push(super::task_tree::link(core, limits, party, *task)?).expect("bounded delegates");
    }
    let mut followups = List::with_capacity(limits.documents.lines);
    for task in row.result_followups.iter().take(bound(limits)) {
        followups.push(super::task_tree::link(core, limits, party, *task)?).expect("bounded followups");
    }
    Ok(ResultDoc {
        acts: super::offers::subject(core, limits, party, &super::Subject::Result { task: row.number }, decision)?,
        task: super::small::chip_with_core(core, limits, row)?,
        contract: super::task_tree::contract(limits, &row.contract)?,
        ending: super::task_tree::ending(limits, value)?,
        at: row.ended_at,
        requester: row.requester,
        executor: row.executor,
        chosen_by: match row.executor {
            tasks::Executor::Person(_) => match row.person_answer {
                Some(answer) => Some(answer.by),
                None => None,
            },
            tasks::Executor::Agent { .. } | tasks::Executor::Procedure { .. } => None,
        },
        delegates,
        followups: Some(followups),
        proposal: match &row.proposal {
            Some(value) => Some(super::ProposalRef { proposer: Proposer::Task(value.proposer), number: value.number }),
            None => None,
        },
        cut: row.delegates.len() > bound(limits) || row.result_followups.len() > bound(limits),
    })
}
pub(super) fn result(
    core: &Core,
    limits: &Limits,
    party: u64,
    number: u64,
    decision: Wall,
) -> Result<ResultDoc, Missing> {
    match core.tasks.task(number) {
        Some(row) => result_from_row(core, limits, party, row, decision),
        None => {
            if core.tasks.stub(number).is_some() {
                Err(Missing::Ended)
            } else {
                Err(Missing::Unknown)
            }
        }
    }
}
/// Copy a retained semantic history row without inventing absent before/after values.
pub fn history_from_row(limits: &Limits, row: &tasks::History) -> Result<HistoryLine, Missing> {
    Ok(HistoryLine {
        task: row.task,
        revision: row.revision,
        by: row.by,
        change: row.change,
        reason: super::small::text(&row.reason, limits.documents.text_bytes),
        at: Some(row.at),
        amendment: match &row.amendment {
            Some(change) => {
                Some((Box::new(settings(limits, &change.before)?), Box::new(settings(limits, &change.after)?)))
            }
            None => None,
        },
        proposal: match &row.proposal {
            Some(value) => Some(ProposalHistory {
                action: retained_action(limits, value.number, &value.action)?,
                number: value.number,
                proposer: value.proposer,
                as_holder: value.as_holder,
                state: match &value.state {
                    tasks::ProposalState::Pending { holder, since } => {
                        ProposalHistoryState::Pending(ProposalWaiting { holder: *holder, since: *since })
                    }
                    tasks::ProposalState::Accepted { by } => ProposalHistoryState::Accepted { by: *by },
                    tasks::ProposalState::Rejected { by, reason } => ProposalHistoryState::Rejected {
                        by: *by,
                        reason: super::small::text(reason, limits.documents.text_bytes),
                    },
                    tasks::ProposalState::Withdrawn => ProposalHistoryState::Withdrawn,
                },
            }),
            None => None,
        },
    })
}
/// Copy an ended delegate after the store reader has checked project visibility.
pub fn ended_delegate_from_row(limits: &Limits, row: &tasks::TaskRecord) -> Result<EndedDelegate, Missing> {
    match &row.phase {
        tasks::Phase::Ended(value) => Ok(EndedDelegate {
            chip: super::small::chip_from_row(limits, row)?,
            ended: super::task_tree::ending(limits, value)?,
            at: row.ended_at,
        }),
        tasks::Phase::Waiting | tasks::Phase::Active(_) | tasks::Phase::Closing(_) | tasks::Phase::Held { .. } => {
            Err(Missing::Unknown)
        }
    }
}

pub(super) fn worst_case(limits: &super::DocumentLimits) -> Option<u64> {
    // A task page prices every nested authority, part, chip, form and result
    // carrier. A proposal batch or person task can hold one per bounded line.
    super::task_tree::overview_worst_case(limits)?
        .checked_mul(u64::from(limits.lines).checked_add(4)?)?
        .checked_add(u64::from(limits.lines).checked_add(2)?.checked_mul(super::offers::heap_worst_case(limits)?)?)?
        .checked_add(u64::try_from(size_of::<Proposal>()).ok()?)?
        .checked_add(u64::try_from(size_of::<PersonTask>()).ok()?)
}

/// Convert an immutable retained action, without borrowing its ended target.
pub fn retained_action(limits: &Limits, number: u64, value: &tasks::ProposalAction) -> Result<RetainedAction, Missing> {
    Ok(match value {
        tasks::ProposalAction::Batch(batch) => {
            if batch.len() > bound(limits) {
                return Err(Missing::TooLarge);
            }
            let mut result = List::with_capacity(limits.documents.lines);
            for value in batch {
                result.push(proposed(limits, value)?).expect("bounded retained batch");
            }
            RetainedAction::Batch(result)
        }
        tasks::ProposalAction::Effect { connector, authority, attempt, completion, position, purpose } => {
            RetainedAction::Effect {
                part: Part { connector: *connector, kind: PartKind::Effect, key: PartKey::Number(number) },
                authority: super::task_tree::authority(limits, authority)?,
                attempt: *attempt,
                completion: *completion,
                position: *position,
                purpose: *purpose,
            }
        }
        tasks::ProposalAction::ProcedureEffect { connector, authority, step, purpose } => {
            RetainedAction::ProcedureEffect {
                part: Part { connector: *connector, kind: PartKind::Effect, key: PartKey::Number(number) },
                authority: super::task_tree::authority(limits, authority)?,
                step: *step,
                purpose: *purpose,
            }
        }
        tasks::ProposalAction::Amend { task, amendment: change, .. } => RetainedAction::Amend {
            task: *task,
            change: Box::new(AmendmentChange {
                addressed: change.addressed,
                instructions: match &change.instructions {
                    Some(value) => Some(super::small::text(value, limits.documents.text_bytes)),
                    None => None,
                },
                release: change.release,
                spec: match &change.spec {
                    Some(value) => Some(spec(limits, value)),
                    None => None,
                },
                authority: match &change.authority {
                    Some(value) => Some(super::task_tree::authority(limits, value)?),
                    None => None,
                },
                wake: change.wake,
                dependencies: match &change.dependencies {
                    Some(value) => Some(numbers(limits, value)),
                    None => None,
                },
                reason: super::small::text(&change.reason, limits.documents.text_bytes),
                cut: match &change.dependencies {
                    Some(value) => value.len() > bound(limits),
                    None => false,
                },
            }),
        },
        tasks::ProposalAction::Widen { task, authority } => {
            RetainedAction::Widen { task: *task, authority: super::task_tree::authority(limits, authority)? }
        }
        tasks::ProposalAction::ReleasePlaced { task, .. } | tasks::ProposalAction::Release { task } => {
            RetainedAction::Release { task: *task }
        }
    })
}
/// Convert a retained lifecycle milestone without inventing missing actor/time.
pub fn milestone_from_row(limits: &Limits, value: &tasks::Milestone) -> Result<MilestoneLine, Missing> {
    Ok(MilestoneLine {
        task: value.task,
        position: value.position,
        at: Some(value.at),
        phase: match &value.phase {
            tasks::Phase::Waiting => MilestonePhase::Waiting,
            tasks::Phase::Active(active) => MilestonePhase::Active(*active),
            tasks::Phase::Closing(closing) => MilestonePhase::Closing {
                stage: closing.stage,
                ending: super::task_tree::ending(limits, &closing.ending)?,
            },
            tasks::Phase::Held { was, why, source } => MilestonePhase::Held {
                why: *why,
                source: super::task_tree::hold_source(limits, source.as_deref())?,
                ending: match was {
                    tasks::Was::Closing(closing) => Some(super::task_tree::ending(limits, &closing.ending)?),
                    tasks::Was::Waiting | tasks::Was::Active(_) => None,
                },
                before: match was {
                    tasks::Was::Waiting => PriorPhase::Waiting,
                    tasks::Was::Active(active) => PriorPhase::Active(*active),
                    tasks::Was::Closing(closing) => PriorPhase::Closing(closing.stage),
                },
            },
            tasks::Phase::Ended(ending) => MilestonePhase::Ended(super::task_tree::ending(limits, ending)?),
        },
    })
}

fn settings(limits: &Limits, row: &tasks::Settings) -> Result<AmendmentState, Missing> {
    Ok(AmendmentState {
        addressed: row.addressed,
        taken_by: row.taken_by,
        instructions: match &row.instructions {
            Some(value) => Some(super::small::text(value, limits.documents.text_bytes)),
            None => None,
        },
        spec: spec(limits, &row.spec),
        authority: super::task_tree::authority(limits, &row.authority)?,
        wake: row.wake,
        dependencies: numbers(limits, &row.dependencies),
        cut: row.dependencies.len() > bound(limits),
    })
}
