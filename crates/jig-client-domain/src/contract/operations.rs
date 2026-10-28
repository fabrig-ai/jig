//! Operations admitted in order and committed together; contract.md, 3.4.
use super::{Amendment, Revision, Text};
use alloc::boxed::Box;
use skein_lib::Duration;
/// Each operation names its compound's project.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Operation {
    Message { project: u32, task: u64, words: Text },
    Amend { project: u32, task: u64, revision: Option<Revision>, change: Box<Amendment>, reason: Text },
    Cancel { project: u32, task: u64, reason: Text, grace: Option<Duration> },
    Release { project: u32, task: u64, revision: Revision, again: Option<bool> },
    Stop { project: u32, task: u64 },
    Move { project: u32, task: u64, reason: Text },
}
/// Why the request is malformed; its refusal names the place.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Malformed {
    Running,
    Settings(super::MalformedSettings),
    SettingsUnchanged,
    ProjectMismatch,
    TwoAsAllowed,
    ConversationNotAgent,
    Cycle,
    ProposedAsAllowed,
    EndedBy,
    Name(NameRule),
}

/// The project name shape rule that failed (domain/core.md, 6.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NameRule {
    Empty,
    Long,
    Control,
    Space,
    Digits,
}

/// A dependency names a place in this batch or an existing task.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TaskRef {
    Member(u32),
    Task(u64),
}
/// The result a new task owes.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum NewContract {
    Report { words: u32 },
    Verdict { choices: skein_lib::List<super::VerdictChoice> },
    Change { connector: u16, kind: u16, words: u32 },
}
/// A named authority budget, or an amount resolved by the engine.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum NewBudget {
    Named,
    AsAllowed { cap: Option<u64> },
}
/// The request deadline retained until admission or acceptance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NewDeadline {
    Named,
    Span(Duration),
}
/// A task's retention after it ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Retention {
    Kept,
    ForgottenOnceRead,
}
/// Independent flags carried to task admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TaskFlags {
    pub conversation: bool,
    pub listed: bool,
    pub retention: Retention,
    pub ended_by: EndedBy,
}
/// A batch member, including grants named after it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct NewTask {
    pub executor: super::OwnerExecutor,
    pub spec: super::Specification,
    pub contract: NewContract,
    pub authority: super::OwnerAuthority,
    pub budget: NewBudget,
    pub deadline: NewDeadline,
    pub wake: Option<super::OwnerWakePolicy>,
    pub dependencies: skein_lib::List<TaskRef>,
    pub named: skein_lib::List<super::OwnerGrant>,
    pub tracked: Option<u32>,
    pub label: u32,
    pub flags: TaskFlags,
    /// The task's LLM choice; `None` is its charter's default.
    pub llm: Option<super::LlmChoice>,
}
/// The engine's resolved budget for one task of a created batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CreatedGiven {
    pub task: u64,
    pub amount: u64,
}

/// A party's explicitly proposed batch or amendment.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ProposeAction {
    ReleasePlaced { task: u64, hold: u64, revision: u64 },
    Settings(super::ProposeSettings),
    Batch { tasks: skein_lib::List<NewTask> },
    Amendment { task: u64, revision: Revision, change: Box<Amendment> },
}

/// The immutable authority that may end a task (domain/tasks.md, 5.8).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum EndedBy {
    /// Its run's judged result ends it.
    Result,
    /// Only its requester ends it; a run's raw finish parks it (5.8).
    Requester,
}
