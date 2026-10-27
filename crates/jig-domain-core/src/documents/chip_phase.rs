//! Nonrecursive chip facts (`client/domain/objects.md`, 3 and 5). Durable
//! lifecycle clocks remain distinct from best-effort engine receipt clocks.
use super::{BudgetLine, HoldLine, Missing, Part, PartKey, PartKind, ResourceLine};
use crate::{Core, Limits};
use alloc::boxed::Box;
use jig_domain_fleet as fleet;
use jig_domain_tasks as tasks;
use skein_lib::{List, Token, Wall};

/// Latest whole activity name, without transport or provisional content.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum ChipWork {
    Preparing,
    Starting,
    Active,
    Writing,
    Tool { call: Box<[u8]>, tool: Box<[u8]> },
    Waiting,
    Ending,
}

/// Fenced host observation; all walls are engine receipt times.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ChipObservation {
    pub channel: Token,
    pub work: ChipWork,
    pub since: Wall,
    pub reported_at: Wall,
    pub started: Option<Wall>,
}

/// Facts needed beside every chip, without nesting other chips or histories.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ChipDetails {
    pub dependencies: List<super::task_tree::Dependency>,
    /// Owner context for a role address, including in a standalone retained chip.
    pub project: u32,
    /// Concrete owner identity; the chip's coarse executor kind remains separate.
    pub executor: tasks::Executor,
    pub phase_since: Wall,
    /// Placement/adoption, never inferred to be actual agent start.
    pub run_since: Option<Wall>,
    pub attempt: Option<u64>,
    pub retry: Option<tasks::RetryNotice>,
    pub wake: tasks::WakePolicy,
    pub person_wait: Option<tasks::PersonWait>,
    pub hold: Option<HoldLine>,
    pub budget: BudgetLine,
    /// Exact immutable activity tail for retained failure/result explanations.
    pub activity: u64,
    pub resources: List<ResourceLine>,
    pub resources_cut: bool,
    /// False for a row-only read: missing holder/writer is then unknown.
    pub resources_observed: bool,
    pub procedure: Option<Part>,
    pub live_delegates: u32,
    /// Exact connectors waited on (QUESTIONS 29), unknown in an unjoined stored closing row.
    pub closing_connectors: Option<List<u16>>,
    pub ending: Option<super::Ending>,
    pub ended_at: Option<Wall>,
    pub observation: Option<ChipObservation>,
}

pub(super) fn from_row(limits: &Limits, row: &tasks::TaskRecord) -> Result<ChipDetails, Missing> {
    let needs_resources = match row.phase {
        tasks::Phase::Waiting | tasks::Phase::Held { .. } => true,
        tasks::Phase::Active(_) | tasks::Phase::Closing(_) | tasks::Phase::Ended(_) => false,
    };
    let mut resources = List::with_capacity(if needs_resources { limits.documents.lines } else { 0 });
    if needs_resources {
        for holding in row.holdings.iter().take(usize::try_from(limits.documents.lines).expect("line bound fits")) {
            let (name, kind, part_kind) = match holding {
                tasks::Holding::Write { resource, kind } => (resource, *kind, PartKind::Resource),
                tasks::Holding::Slot { pool, kind } => (pool, *kind, PartKind::Pool),
            };
            resources
                .push(ResourceLine {
                    part: super::task_tree::part_path(limits, name.connector, part_kind, &name.path)?,
                    kind,
                    taken: row.holds_taken,
                    holder: None,
                    writer: None,
                    writer_lost: false,
                })
                .expect("bounded resource facts");
        }
    }
    let ending = match &row.phase {
        tasks::Phase::Closing(closing) => Some(super::task_tree::ending(limits, &closing.ending)?),
        tasks::Phase::Ended(ending) => Some(super::task_tree::ending(limits, ending)?),
        tasks::Phase::Held { was, .. } => match was {
            tasks::Was::Closing(closing) => Some(super::task_tree::ending(limits, &closing.ending)?),
            tasks::Was::Waiting | tasks::Was::Active(_) => None,
        },
        tasks::Phase::Waiting | tasks::Phase::Active(_) => None,
    };
    Ok(ChipDetails {
        dependencies: super::task_tree::dependency_rows(limits, row),
        project: row.project,
        executor: row.executor,
        phase_since: row.phase_since,
        run_since: row.run_since,
        attempt: if row.attempt == 0 { None } else { Some(row.attempt) },
        retry: row.retry_notice,
        wake: row.wake,
        person_wait: row.person_wait,
        hold: super::task_tree::hold(limits, row)?,
        budget: super::task_tree::budget(row),
        activity: row.activity,
        resources_cut: needs_resources
            && usize::try_from(resources.len()).expect("line bound fits") < row.holdings.len(),
        resources,
        resources_observed: false,
        procedure: match row.executor {
            tasks::Executor::Procedure { connector, code } => {
                Some(Part { connector, kind: PartKind::Procedure, key: PartKey::Number(u64::from(code)) })
            }
            tasks::Executor::Agent { .. } | tasks::Executor::Person(_) => None,
        },
        live_delegates: u32::try_from(row.delegates.len()).expect("bounded direct delegates"),
        closing_connectors: None,
        ending,
        ended_at: row.ended_at,
        observation: None,
    })
}

pub(super) fn observe(
    core: &Core,
    limits: &Limits,
    row: &tasks::TaskRecord,
    details: &mut ChipDetails,
) -> Result<(), Missing> {
    details.dependencies = super::task_tree::dependency_facts(core, limits, row);
    for index in 0..details.resources.len() {
        let holding =
            row.holdings.get(usize::try_from(index).expect("bounded holding index")).expect("one resource per holding");
        let name = match holding {
            tasks::Holding::Write { resource, .. } => resource,
            tasks::Holding::Slot { pool, .. } => pool,
        };
        let resource = details.resources.get_mut(index).expect("bounded resource index");
        resource.holder = core.tasks.holder(name);
        if let Some(slot) = core.tasks.writer(name) {
            resource.writer = Some(slot.writer);
            resource.writer_lost = slot.lost;
        }
    }
    details.resources_observed = true;
    let closing = match &row.phase {
        tasks::Phase::Closing(_) => true,
        tasks::Phase::Held { was, .. } => match was {
            tasks::Was::Closing(_) => true,
            tasks::Was::Waiting | tasks::Was::Active(_) => false,
        },
        tasks::Phase::Waiting | tasks::Phase::Active(_) | tasks::Phase::Ended(_) => false,
    };
    if closing {
        let mut connectors = List::with_capacity(limits.documents.lines);
        for ((task, connector), ()) in &core.closing_connectors {
            if *task == row.number {
                if connectors.room() == 0 {
                    return Err(Missing::TooLarge);
                }
                connectors.push(*connector).expect("whole connector list room checked");
            }
        }
        details.closing_connectors = Some(connectors);
    }
    let current = match row.phase {
        tasks::Phase::Active(tasks::Active::Claimed { attempt } | tasks::Active::Running { attempt }) => Some(attempt),
        tasks::Phase::Waiting
        | tasks::Phase::Active(
            tasks::Active::Idle | tasks::Active::Due | tasks::Active::Preparing | tasks::Active::BackingOff { .. },
        )
        | tasks::Phase::Closing(_)
        | tasks::Phase::Held { .. }
        | tasks::Phase::Ended(_) => None,
    };
    if let Some(attempt) = current
        && let Some(observation) = core.fleet.observation(Token::new(row.number), Token::new(attempt))
    {
        let work = match &observation.activity {
            fleet::Activity::Preparing => ChipWork::Preparing,
            fleet::Activity::Starting => ChipWork::Starting,
            fleet::Activity::Active => ChipWork::Active,
            fleet::Activity::Writing | fleet::Activity::Long { operation: fleet::Operation::Completion, .. } => {
                ChipWork::Writing
            }
            fleet::Activity::Long { operation: fleet::Operation::Checks | fleet::Operation::Command, .. } => {
                ChipWork::Active
            }
            fleet::Activity::Waiting => ChipWork::Waiting,
            fleet::Activity::Ending => ChipWork::Ending,
            fleet::Activity::Tool { call, tool } => {
                let bound = usize::try_from(limits.documents.text_bytes).expect("text bound fits");
                if call.len() > bound || tool.len() > bound {
                    return Err(Missing::TooLarge);
                }
                ChipWork::Tool { call: skein_lib::bytes::copy_of(call), tool: skein_lib::bytes::copy_of(tool) }
            }
        };
        details.observation = Some(ChipObservation {
            channel: observation.channel,
            work,
            since: observation.since,
            reported_at: observation.reported_at,
            started: observation.started,
        });
    }
    Ok(())
}

pub(super) fn heap_worst_case(limits: &super::DocumentLimits) -> Option<u64> {
    let text = u64::from(limits.text_bytes);
    let lines = u64::from(limits.lines);
    let path = List::<Box<[u8]>>::worst_case(limits.lines)?.checked_add(lines.checked_mul(text)?)?;
    u64::try_from(size_of::<ChipDetails>())
        .ok()?
        .checked_add(text.checked_mul(8)?)?
        .checked_add(List::<u64>::worst_case(limits.lines)?)?
        .checked_add(List::<u16>::worst_case(limits.lines)?)?
        .checked_add(List::<super::task_tree::Dependency>::worst_case(limits.lines)?)?
        .checked_add(32 * 8)?
        .checked_add(List::<ResourceLine>::worst_case(limits.lines)?)?
        .checked_add(lines.checked_mul(path)?)?
        .checked_add(path)
}
