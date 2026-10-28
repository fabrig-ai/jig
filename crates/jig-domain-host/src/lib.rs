//! Jig's host (domain/hosts.md, sections 4 to 6) keeps the lifecycle of runs
//! on the slots of a worker or an engine. It admits assignments, asks an
//! optional application workspace to prepare and serve deliveries, supervises
//! an agent capability, and answers after both have settled. It keeps turns
//! and answers until the core acknowledges them. The root translates the link
//! to the core, workspace and agent capabilities; an engine root gives it a
//! direct link that is never lost and no workspace. The agent may be a process
//! or an inline agent, and the host does not distinguish them.
//! The run lifecycle arms no timers; its callers bound each wait. The source
//! machine owns periodic checks, driven by its parent through `next_deadline`
//! and `resume`. Facts are best effort and never govern a decision.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

mod activity;
mod answers;
mod assignment;
mod boundary;
mod call;
mod domain;
mod facts;
mod hosted;
mod limits;
mod link;
mod live;
pub use live::{LiveAsk, LiveKind, Piece};
mod refusals;
pub mod sources;
#[cfg(test)]
mod tests;
mod turns;

pub use activity::{Activity, CountKind, Operation};
pub use boundary::{
    AgentFailure, Answer, AnsweredCall, Ask, Assignment, Bound, Delivery, DeliveryOutcome, Ending, Event, Exceeded,
    Failure, Finish, Form, FromAgent, Grant, GrantTable, Hosting, Invalid, MessageRefusal, Phase, Preparation, Reason,
    Refusal, Refused, Reply, Request, RunAssignment, RunFailure, Scheme, SettledAnswer, Start, StartCause,
    StartMessage, ToAgent, Turn, Work, Workspace,
};
pub use domain::{Domain, max_out, resume, step};
pub use facts::Fact;
pub use limits::{Limits, valid};

pub use link::{Bounds, Hello, LinkDown, LinkUp, ModelBound, PROGRAM_ENDPOINTS, Relayed, valid_program_endpoints};

pub use link::{Login, LoginMethod, LoginState, SignOutResult, SourceFailure, SourceName};
