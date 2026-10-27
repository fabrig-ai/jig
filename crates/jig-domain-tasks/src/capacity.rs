//! Exact, content-free room refusals (client/domain/pages.md, section 7;
//! domain/tasks.md, section 11). Only the owner check which found no room
//! attaches a cause. The bounded live history changes neither admission nor
//! diagnostic draining, and retains no payload or reply right.
use crate::{Domain, Funder, Limits, New, Party, Problem, Refusal, Request};
use skein_lib::{Env, List, Queue, ReplyTo, Wall};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum CapacityAction {
    Batch { first: Option<u64>, members: u32 },
    Proposal { number: u64 },
    PersonProposal { number: u64 },
    Message { task: u64, number: u64 },
    Source { funder: Funder },
    Introduce { left: u64, right: u64 },
    Subscription { number: u64 },
    Effect { entry: u64 },
    Claim { attempt: u64 },
    Release,
    Place { hold: u64 },
    Move,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum CapacityCause {
    PlacedHolds { limit: u32 },
    ProjectPlacedHolds { limit: u32 },
    PartyPlacedHolds { limit: u32 },
    Exemptions { limit: u32 },
    Tasks { limit: u32 },
    CreationFlights { limit: u32 },
    ResourceReports { limit: u32 },
    ProjectTasks { limit: u32 },
    Tree { limit: u32 },
    Delegates { limit: u32 },
    Inbox { task: u64, messages: u32, bytes: u32 },
    Holdings { limit: u32 },
    HoldWaiters { limit: u32 },
    Proposals { limit: u32 },
    References { limit: u32 },
    Subscriptions { limit: u32 },
    Funders { limit: u32 },
    Writers { limit: u32 },
}

/// Metadata attached only by the exact failed room check, not inferred from Busy.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Capacity {
    pub project: Option<u32>,
    pub by: Party,
    pub action: CapacityAction,
    pub cause: CapacityCause,
}

impl Capacity {
    /// Describe a whole refused atomic batch without retaining its specifications.
    #[must_use]
    #[expect(clippy::manual_map, reason = "step code has no closures (programming-model.md, section 10)")]
    pub fn batch(by: Party, batch: &[New], task: Option<u64>, cause: CapacityCause) -> Self {
        let mut project = None;
        if let Some(first) = batch.first() {
            project = Some(first.project);
            for member in batch {
                if member.project != first.project {
                    project = None;
                }
            }
        }
        if let Some(task) = task {
            for member in batch {
                if member.number == task {
                    project = Some(member.project);
                }
            }
        }
        Self {
            project,
            by,
            action: CapacityAction::Batch {
                first: match batch.first() {
                    Some(value) => Some(value.number),
                    None => None,
                },
                members: u32::try_from(batch.len()).unwrap_or(u32::MAX),
            },
            cause,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct CapacityRefusal {
    pub request: Capacity,
    pub at: Wall,
}

impl Domain {
    /// Oldest to newest admitted facts; no diagnostic or admission state is drained.
    #[must_use]
    pub fn capacity_refusals(&self) -> List<CapacityRefusal> {
        let mut rows = List::with_capacity(self.room_refusals.capacity());
        for row in &self.room_refusals {
            rows.push(*row).expect("bounded capacity history snapshot");
        }
        rows
    }
    #[must_use]
    pub const fn capacity_refusals_lost(&self) -> u64 {
        self.room_refusals_lost
    }
    /// Content revision for the core's level-triggered deployment watch.
    #[must_use]
    pub const fn capacity_mark(&self) -> u64 {
        self.room_refusals_mark
    }
    /// Record an owner-proven refusal, also used by the core's projection-room check.
    pub fn remember_capacity(&mut self, at: Wall, request: Capacity) {
        self.room_refusals_mark = self.room_refusals_mark.saturating_add(1);
        if self.room_refusals.capacity() == 0 {
            self.room_refusals_lost = self.room_refusals_lost.saturating_add(1);
            return;
        }
        if self.room_refusals.room() == 0 {
            self.room_refusals.pop();
            self.room_refusals_lost = self.room_refusals_lost.saturating_add(1);
        }
        self.room_refusals.push(CapacityRefusal { request, at });
    }
}

pub(crate) fn refused(domain: &mut Domain, env: &Env<Limits>, to: ReplyTo, problem: Problem, out: &mut Queue<Request>) {
    if let Some(request) = problem.capacity {
        domain.remember_capacity(env.wall, request);
    }
    out.push(Request::Refused { reply_to: to, problem });
}

pub(crate) fn refuse(
    domain: &mut Domain,
    env: &Env<Limits>,
    to: ReplyTo,
    task: Option<u64>,
    why: Refusal,
    request: Capacity,
    out: &mut Queue<Request>,
) {
    refused(domain, env, to, Problem::capacity(task, why, request), out);
}

pub(crate) fn inbox(task: u64, limits: &Limits) -> CapacityCause {
    CapacityCause::Inbox { task, messages: limits.inbox_messages, bytes: limits.inbox_bytes }
}

pub(crate) fn batch_problem(
    task: Option<u64>,
    why: Refusal,
    by: Party,
    batch: &[New],
    cause: CapacityCause,
) -> Problem {
    Problem::capacity(task, why, Capacity::batch(by, batch, task, cause))
}
