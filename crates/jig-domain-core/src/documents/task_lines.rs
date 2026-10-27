//! The shared task line, constructed from a live or loaded ended owner row.
use super::{BudgetLine, Chip, Count, Missing, PhaseCounts, WaitsOn};
use crate::{Core, Limits};
use jig_domain_tasks as tasks;
use skein_lib::{List, Wall};

/// The requesting party's position over a conversation's committed turns.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ConversationRead {
    pub seen: u64,
    pub unread: Count,
}

/// One record for every task list (client/domain/contract.md, 5.1).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct TaskLine {
    pub chip: Chip,
    pub project: u32,
    pub requester: tasks::Party,
    pub tracked: Option<u32>,
    pub label: u32,
    pub flags: tasks::Flags,
    pub changed: Wall,
    pub last_activity: Wall,
    pub ended_at: Option<Wall>,
    pub budget: BudgetLine,
    pub waits_on: WaitsOn,
    pub phases: PhaseCounts,
    pub held_below: u32,
    pub conversation: Option<ConversationRead>,
}

/// Construct a visible task's line without retaining derived facts.
pub fn task_line(
    core: &Core,
    limits: &Limits,
    party: u64,
    row: &tasks::TaskRecord,
    ended_read: Option<u64>,
) -> Result<TaskLine, Missing> {
    if core.people.role(party, row.project).is_none() {
        return Err(Missing::NotAllowed);
    }
    let mut phases = PhaseCounts::ZERO;
    let mut pending = List::with_capacity(limits.tasks.tree_tasks);
    let ended = match row.phase {
        tasks::Phase::Ended(_) => true,
        tasks::Phase::Waiting | tasks::Phase::Active(_) | tasks::Phase::Held { .. } | tasks::Phase::Closing(_) => false,
    };
    if !ended {
        pending.push(row.number).expect("validated tree bound");
    }
    for _ in 0..limits.tasks.tree_tasks {
        let Some(index) = pending.len().checked_sub(1) else { break };
        let number = *pending.get(index).expect("pending live task");
        pending.truncate(index);
        let Some(candidate) = core.tasks.task(number) else { continue };
        phases.add(&candidate.phase);
        for delegate in candidate.delegates.iter().rev() {
            pending.push(*delegate).expect("live subtree within tree_tasks");
        }
    }
    drop(pending);
    let held_below = phases.held.saturating_sub(match row.phase {
        tasks::Phase::Held { .. } => 1,
        tasks::Phase::Waiting | tasks::Phase::Active(_) | tasks::Phase::Closing(_) | tasks::Phase::Ended(_) => 0,
    });
    let conversation = match row.requester {
        tasks::Party::Person(requester) if row.flags.conversation => {
            let seen = if ended { ended_read } else { core.people.conversation_seen(requester, row.number) }
                .ok_or(Missing::Unknown)?;
            let unread = row.last_reply.saturating_sub(seen);
            let count = u32::try_from(unread.min(u64::from(limits.people.inbox_entries))).expect("bounded unread");
            Some(ConversationRead {
                seen,
                unread: if unread > u64::from(limits.people.inbox_entries) {
                    Count::AtLeast(count)
                } else {
                    Count::Known(count)
                },
            })
        }
        tasks::Party::Person(_) | tasks::Party::Task(_) | tasks::Party::Deployment { .. } => None,
    };
    Ok(TaskLine {
        chip: super::small::chip_with_core(core, limits, party, row)?,
        project: row.project,
        requester: row.requester,
        tracked: row.tracked,
        label: row.label,
        flags: row.flags,
        changed: row.changed,
        last_activity: row.last_activity,
        ended_at: row.ended_at,
        budget: super::task_tree::budget(row),
        waits_on: if ended { WaitsOn::Nothing } else { core.waits_on(limits, row.number) },
        phases,
        held_below,
        conversation,
    })
}
