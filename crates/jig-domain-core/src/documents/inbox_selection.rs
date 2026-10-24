//! Goal-filter membership carried with the inbox's original row cursor
//! (`client/domain/pages.md`, section 3). Live membership follows the bounded
//! requester walk; retained membership uses the supplied original ancestry.

use super::{Cursor, Missing};
use crate::{Core, Limits};
use jig_domain_tasks as tasks;
use skein_lib::List;

/// Complete qualifying goal roots for one inbox row, including its task itself.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct InboxSelection {
    pub cursor: Cursor,
    pub goals: List<u64>,
}

impl InboxSelection {
    /// Heap held by the roots, including reserved unused capacity.
    #[must_use]
    pub fn owned_bytes(&self) -> Option<u64> {
        List::<u64>::worst_case(self.goals.capacity())
    }
}

/// Heap bound of a whole parallel context list and all its root allocations.
#[must_use]
pub fn inbox_selection_worst_case(rows: u32, depth: u32) -> Option<u64> {
    let roots = List::<u64>::worst_case(depth.checked_add(1)?)?;
    List::<InboxSelection>::worst_case(rows)?.checked_add(u64::from(rows).checked_mul(roots)?)
}

pub(super) fn live(core: &Core, limits: &Limits, task: u64, cursor: Cursor) -> Result<InboxSelection, Missing> {
    let capacity = limits.tasks.depth.checked_add(1).ok_or(Missing::TooLarge)?;
    let mut goals = List::with_capacity(capacity);
    if task == tasks::PROPOSAL_WITHOUT_TASK {
        return Ok(InboxSelection { cursor, goals });
    }
    let mut number = task;
    for _ in 0..=limits.tasks.depth {
        goals.push(number).expect("one root for each bounded requester step");
        let Some(row) = core.tasks.task(number) else {
            break;
        };
        match row.requester {
            tasks::Party::Task(parent) => number = parent,
            tasks::Party::Person(_) | tasks::Party::Deployment { .. } => break,
        }
    }
    Ok(InboxSelection { cursor, goals })
}

pub(crate) fn retained(
    limits: &Limits,
    task: u64,
    cursor: Cursor,
    ancestors: &[u64],
) -> Result<InboxSelection, Missing> {
    let capacity = limits.tasks.depth.checked_add(1).ok_or(Missing::TooLarge)?;
    if ancestors.len() > usize::try_from(limits.tasks.depth).expect("configured depth fits") {
        return Err(Missing::TooLarge);
    }
    let mut goals = List::with_capacity(capacity);
    if task != tasks::PROPOSAL_WITHOUT_TASK {
        goals.push(task).expect("self always has a place");
    }
    for root in ancestors {
        goals.push(*root).expect("whole admitted original ancestry fits");
    }
    Ok(InboxSelection { cursor, goals })
}
