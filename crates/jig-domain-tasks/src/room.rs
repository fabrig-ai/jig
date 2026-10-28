//! Procedure batches wait for freeing live counts (domain/connectors.md, 6.2).
//! The live task rows are the bounded waiter set. Their durable order survives
//! arbitrary restore order; each ending offers its one place to the oldest
//! eligible waiter. A wake grants no reservation: admission checks fresh facts.
use crate::domain::{Domain, publish, record, task_mut, wake_procedure};
use crate::{Active, Capacity, CapacityAction, CapacityCause, Executor, Limits, Phase, Problem, Refusal, Request};
use skein_lib::{Env, Queue};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum LiveRoom {
    Deployment,
    Project(u32),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct RoomWait {
    pub room: LiveRoom,
    pub order: u64,
}

impl Problem {
    /// Only owner-proven batch counts which a task ending can free. Shape,
    /// authority, spend and the tree's lifetime allotment remain refusals.
    #[must_use]
    pub fn freeing_count(&self) -> Option<LiveRoom> {
        let Capacity { action, cause, project, .. } = self.capacity?;
        match action {
            CapacityAction::Batch { .. } => {}
            CapacityAction::Proposal { .. }
            | CapacityAction::PersonProposal { .. }
            | CapacityAction::Message { .. }
            | CapacityAction::Source { .. }
            | CapacityAction::Introduce { .. }
            | CapacityAction::Subscription { .. }
            | CapacityAction::Effect { .. }
            | CapacityAction::Claim { .. }
            | CapacityAction::Release
            | CapacityAction::Place { .. }
            | CapacityAction::Move => return None,
        }
        match cause {
            CapacityCause::Tasks { .. } if self.why == Refusal::Live => return Some(LiveRoom::Deployment),
            CapacityCause::ProjectTasks { .. } if self.why == Refusal::Project => {
                if let Some(project) = project {
                    return Some(LiveRoom::Project(project));
                }
            }
            CapacityCause::Tasks { .. }
            | CapacityCause::ProjectTasks { .. }
            | CapacityCause::PlacedHolds { .. }
            | CapacityCause::ProjectPlacedHolds { .. }
            | CapacityCause::PartyPlacedHolds { .. }
            | CapacityCause::Exemptions { .. }
            | CapacityCause::CreationFlights { .. }
            | CapacityCause::ResourceReports { .. }
            | CapacityCause::Tree { .. }
            | CapacityCause::Delegates { .. }
            | CapacityCause::Inbox { .. }
            | CapacityCause::Holdings { .. }
            | CapacityCause::HoldWaiters { .. }
            | CapacityCause::Proposals { .. }
            | CapacityCause::References { .. }
            | CapacityCause::Subscriptions { .. }
            | CapacityCause::Funders { .. }
            | CapacityCause::Writers { .. } => {}
        }
        None
    }
}

pub(crate) fn wait(domain: &mut Domain, env: &Env<Limits>, task: u64, room: LiveRoom, out: &mut Queue<Request>) {
    let mut last = 0;
    for (number, _) in &domain.names {
        if let Some(wait) = record(domain, *number).expect("live waiter set").room_wait {
            last = last.max(wait.order);
        }
    }
    let Some(order) = last.checked_add(1) else { return };
    let row = &mut task_mut(domain, task).expect("refused procedure live").record;
    row.room_wait = Some(RoomWait { room, order });
    row.phase = Phase::Active(Active::Idle);
    publish(domain, env, task, out);
}

pub(crate) fn freed(domain: &mut Domain, env: &Env<Limits>, project: u32, out: &mut Queue<Request>) {
    let mut oldest: Option<(u64, u64)> = None;
    for (number, _) in &domain.names {
        let row = record(domain, *number).expect("live waiter set");
        let Some(wait) = row.room_wait else { continue };
        let eligible = match wait.room {
            LiveRoom::Deployment => true,
            LiveRoom::Project(waiting) => waiting == project,
        };
        if eligible
            && row.phase == Phase::Active(Active::Idle)
            && match oldest {
                Some((order, _)) => wait.order < order,
                None => true,
            }
        {
            oldest = Some((wait.order, *number));
        }
    }
    if let Some((_, number)) = oldest {
        wake_procedure(domain, env, number, out);
    }
}

pub(crate) fn valid(domain: &Domain, task: &crate::TaskRecord) -> bool {
    let Some(wait) = task.room_wait else { return true };
    let procedure = match task.executor {
        Executor::Procedure { .. } => true,
        Executor::Agent { .. } | Executor::Person(_) => false,
    };
    if wait.order == 0 || !procedure {
        return false;
    }
    match wait.room {
        LiveRoom::Project(project) if project != task.project => return false,
        LiveRoom::Deployment | LiveRoom::Project(_) => {}
    }
    for (number, _) in &domain.names {
        if *number != task.number
            && let Some(other) = record(domain, *number).expect("live waiter set").room_wait
            && other.order == wait.order
        {
            return false;
        }
    }
    true
}
