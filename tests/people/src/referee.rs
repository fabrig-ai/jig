//! Expectations over client replies and durable task creations, never state.
use jig_domain_people::{Ending, HoldReason, Lack, Moved, Party, Refusal, RequestKey, Role};
use skein_lib::Duration;
use skein_world::domain::{Expectations, Judge};
use std::collections::BTreeSet;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RouteKind {
    Application,
    Chat,
    Goal,
    Service,
    Proposal,
    TakePerson,
    Control,
    Account,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Seen {
    Called { call: u64 },
    Abandoned { call: u64 },
    Routed { role: Role, kind: RouteKind, service: bool },
    RoleWaitingOpened { task: u64, both_present: bool },
    RoleWaitingResolved { task: u64, both_absent: bool },
    Durable { commit: u64 },
    Created { key: RequestKey },
    Replied { call: u64, after: u64 },
    Refused { why: Refusal },
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Name {
    Reply(u64),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stimulus {
    Restart,
}

#[derive(Default, Debug)]
pub struct People {
    durable: u64,
    replied: BTreeSet<u64>,
    created: BTreeSet<RequestKey>,
    role_waiting: BTreeSet<u64>,
}

impl Expectations for People {
    type Seen = Seen;

    type Name = Name;

    type Stimulus = Stimulus;

    fn observe(&mut self, seen: Seen, judge: &mut Judge<Name, Stimulus>) {
        match seen {
            Seen::Called { call } => judge.expect(Name::Reply(call), Duration::from_secs(10)),
            Seen::Abandoned { call } => {
                judge.meet(&Name::Reply(call));
            }
            Seen::Routed { role, kind, service } => match kind {
                RouteKind::Chat if service || role == Role::Observer => judge.fail("chat beyond party role or kind"),
                RouteKind::Goal | RouteKind::Proposal | RouteKind::TakePerson if role == Role::Observer => {
                    judge.fail("observer request routed");
                }
                RouteKind::Service if role != Role::Owner => judge.fail("service creation beyond owner role"),
                RouteKind::Account if service => judge.fail("a service's account request routed"),
                RouteKind::Chat
                | RouteKind::Goal
                | RouteKind::Service
                | RouteKind::Proposal
                | RouteKind::TakePerson
                | RouteKind::Control
                | RouteKind::Application
                | RouteKind::Account => {}
            },
            Seen::RoleWaitingOpened { task, both_present } => {
                if !both_present || !self.role_waiting.insert(task) {
                    judge.fail("role request was not in every holder inbox");
                }
            }
            Seen::RoleWaitingResolved { task, both_absent } => {
                if !both_absent || !self.role_waiting.remove(&task) {
                    judge.fail("acted role request remained in a holder inbox");
                }
            }
            Seen::Refused { why } => {
                if !filled(why) {
                    judge.fail("refusal facts are inconsistent or unnamed");
                }
            }
            Seen::Durable { commit } => self.durable = commit,
            Seen::Created { key } => {
                if !self.created.insert(key) {
                    judge.fail("key made a task twice");
                }
            }
            Seen::Replied { call, after } => {
                if after > self.durable {
                    judge.fail("reply before its commit is durable");
                }
                if !self.replied.insert(call) {
                    judge.fail("call replied twice");
                }
                judge.meet(&Name::Reply(call));
            }
        }
    }
}

fn filled(why: Refusal) -> bool {
    match why {
        Refusal::Stale { newest } => newest != 0,
        Refusal::Full { revision, tasks, more } => revision != 0 && (!tasks.is_empty() || more != 0),
        Refusal::Role { held } => match held {
            Some(Role::Policy { role }) => role > 3,
            Some(Role::Owner | Role::Maintainer | Role::Member | Role::Observer) | None => true,
        },
        Refusal::Authority { lacking, .. } => match lacking {
            Lack::Budget { needs, left } => needs > left,
            Lack::Approver
            | Lack::Grant { .. }
            | Lack::Funding
            | Lack::TaskKind { .. }
            | Lack::Depth { .. }
            | Lack::Deadline
            | Lack::Tasks { .. }
            | Lack::Tool
            | Lack::Scope
            | Lack::Models
            | Lack::Reference
            | Lack::ResourceAccess
            | Lack::ResourceUnavailable
            | Lack::Again { .. }
            | Lack::Shape
            | Lack::Account
            | Lack::Writer
            | Lack::Required { .. }
            | Lack::Failed { .. }
            | Lack::Unguarded { .. } => true,
        },
        Refusal::Moved { now } => match now {
            Moved::Hold { reason, .. } => match reason {
                HoldReason::Dependency { task } => task != 0,
                HoldReason::Placed { hold: party } => party != 0,
                HoldReason::Uncertain { entry } => entry != 0,
                HoldReason::PoolLost { pool } => pool != 0,
                HoldReason::Exceeded(_)
                | HoldReason::Failures { .. }
                | HoldReason::Stalled
                | HoldReason::Stopped
                | HoldReason::Drift
                | HoldReason::Effects
                | HoldReason::EffectFailed
                | HoldReason::Budget
                | HoldReason::Deadline
                | HoldReason::HoldsWaited
                | HoldReason::ResourceUnavailable
                | HoldReason::ResourceAccess
                | HoldReason::Procedure
                | HoldReason::Circle => true,
            },
            Moved::Spec { .. } | Moved::Person { .. } => true,
            Moved::Note { revision } => revision != 0,
        },
        Refusal::Circle { tasks, cut } => {
            (cut || !tasks.is_empty()) && tasks.len() <= 32 && tasks.iter().all(|task| *task != 0)
        }
        Refusal::Dependency { dependency, .. } => dependency != 0,
        Refusal::Kept { pending } => {
            !pending.is_empty() && pending.len() <= 64 && pending.iter().all(|task| *task != 0)
        }
        Refusal::Taken { by } => by != 0,
        Refusal::Ended { how } => match how {
            Ending::Cancelled { by, .. } => match by {
                Party::Person(number) | Party::Task(number) => number != 0,
                Party::Deployment { project } => project != 0,
            },
            Ending::Decided { by, .. } => by != 0,
            Ending::Done | Ending::Failed | Ending::Deleted => true,
        },
        Refusal::Limit { .. }
        | Refusal::NotWaiting
        | Refusal::NoFurther
        | Refusal::NameTaken
        | Refusal::Unknown
        | Refusal::Provider
        | Refusal::NotOffered
        | Refusal::NotWellFormed { .. }
        | Refusal::NoEscalationHolder
        | Refusal::Llm { .. }
        | Refusal::SignInRedirect { .. }
        | Refusal::AccountMove { .. }
        | Refusal::Account { .. }
        | Refusal::KeyConflict => true,
    }
}
