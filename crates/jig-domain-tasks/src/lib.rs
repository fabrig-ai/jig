//! The tasks child: bounded live lifecycle, immutable dependencies and authentic
//! finite funding (programming-model.md, section 4.5; domain/tasks.md, sections 2–5 and 10). It keeps live task rows and name indices, retry timers,
//! configured agent charters, period/pool ledgers and optional diagnostic facts.
//! It keeps bounded pointers to ended results named by live work. It never knows protocol bytes, worker channels, connector internals, people
//! authentication or the authority child's policy tables. Root supplies those
//! decisions through independent semantic types.
//!
//! `Domain::new` starts restoring. `Stored::Live`, `Stored::Ledger`, `Stored::Stub`,
//! `Stored::Writer` and `Stored::Pool`
//! enter `Restore`; `Check` validates the complete live graph and authentic
//! financial reservations before the store switches. `Restored` replays bounded
//! activation/adoption outputs after the start's mark is durable. Historical
//! ended rows remain in root storage and cannot re-enter live state

//!
//! Current per-project resource access is a transient connector report cache.
//! Unavailability refuses holds and holds live tasks; context narrowing stops
//! a claimed run writer. Committed effect writers settle normally.
//! `step` receives root-authorized batches, finite-source inputs and
//! lifecycle notifications. Reply-bearing inputs have one terminal reply;
//! notifications can be ignored when stale. Priced turns/terminals preflight
//! the whole lifecycle/financial admission before posting the new cumulative
//! expense delta. Root owns exact transport replay proof and transcript rows;
//! tasks keeps no replay receipt or result-delivery credit

//!
//! `Request::Activate` carries a bounded `RunContext`, not rendered
//! bytes or a second mutable task ledger. Root gathers the brief and
//! claims the task. `Request::Close` awaits root's closing obligations;
//! `Ended`, the historical task row, and original-source
//! posting share one atomic decision. The current root exposes person result notices and
//! delays outward replies/effects until durability; historical result reads
//! remain the root's route (domain/tasks.md, section 5.6; domain/engine.md, sections 5.6, 7.1 and 9).
//!
//! `fire` expires at most one projected retry timer per call and advances bounded
//! cascades. Caller reserves free `Request` room from `max_out`; `worst_case`
//! counts retained containers/payloads and bounded scratch, while callers count
//! queues and owned output copies. `stored_bytes` measures existing row heap
//! without cloning; `Domain::funding` borrows authentic finite accounting without
//! creating a mutable root shadow (domain/tasks.md, section 10).
//! Current chat construction supplies a report contract and empty parameters.
//! A held person chat owns one bounded semantic escalation revision/recipient
//! or rejected reason. Root authenticates its read/decision route, resolves
//! eligibility and owns immutable transport history; this child accepts/rejects/
//! passes only the authorized hold revision. A hold revision changes when the
//! task is held anew or its reason changes; passing changes only its recipient.
//! Task spec and person revisions are kept in live and ended rows separately
//! from history (client/domain/contract.md, section 5.2).
//!
//! | Kept state | Event | Next state | Emits |
//! |---|---|---|---|
//! | unheld | hold begins | hold revision + 1, routing | save, escalation needed |
//! | held | reason changes | hold revision + 1, routing | save, escalation needed |
//! | waiting escalation | pass or reroute | same revision, new recipient | save, wake |
//! | current spec | changed amendment | spec revision + 1 | history, save, message |
//! | person question | changed spec/dependencies/address or holder released | person revision + 1 | save, message |
//! | person inputs | dependency result arrives | person revision + 1 | save |
//! | any revisions | spend, turn, priority, delegate phase | same revisions | ordinary outputs |
//! Agent and procedure executors are routed. The root checks historical inputs
//! before delegation.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
extern crate alloc;
mod activity;
mod admission;
mod batch;
mod boundary;
mod capacity;
mod changed;

mod closing;
mod compound;
mod control;
pub use compound::{CompoundAdmission, CompoundContext, CompoundTask, Operation};
mod domain;
mod payer;
pub use payer::{Payer, PayerLink};
mod escalation;
mod facts;
mod failures;
mod flags;
mod goals;
mod grace;
#[doc(hidden)]
pub use flags::shaped;
pub use flags::{EndedBy, Flags, Retention};
mod funders;
mod hold_origin;
mod holds;
mod inbox;
mod index;
pub use index::{Edge, EndedByParty, EndedByProject, Endings, Newest, ResultRow, Update, UpdateObject};
mod limits;
mod llm;
mod moving;
mod owned;
mod parts;
mod periods;
mod positions;
pub use positions::{AnswerLine, ProposalRow, QuestionLine};
mod person;
mod person_proposals;
mod procedure;
mod proposals;
mod recurring;
mod refs;
mod retire;
mod run;
mod runtime;
mod settings;
mod settings_moves;
mod standing;
mod stored;
mod subscriptions;
#[cfg(test)]
mod tests;
mod value;
mod verdict;
mod waiting;
mod waits;
mod wake;
mod writers;
pub use activity::{ActivityLine, EndKind, Line, Why, line_bytes_limit, lines_per_decision};
pub use batch::{bounded_batch, valid_authority, valid_contract, valid_spec};
pub use boundary::{
    Accepted, Active, Added, Bound, Cause, Circle, Closing, Contract, Deadline, DelegateState, DelegationContext, End,
    Ending, Event, Exceeded, Executor, FieldRule, Hold, HoldKind, HoldSource, Holding, InvalidResult, Key, Kind,
    MessageKind, Name, New, NewsClass, NoticeState, Parameter, Party, PersonAddress, PersonAnswer, PersonStallNotice,
    PersonWait, Phase, PoolSlots, Problem, ProcedureAmendment, ProcedureCancel, ProcedureDecision, ProcedureEnd,
    ProcedureMessage, QuestionCredit, RecurringOverlap, RecurringState, RecurringTemplate, Refusal, ReplyHint, Request,
    ResourceAccess, ResultField, ResultFollowups, ResultKind, ResultsWake, Revisions, RunContext, SavedResource,
    SettingsRevision, Spec, Stage, Status, Stored, Stub, Subscription, SubscriptionKind, Taken, TaskRecord, TaskResult,
    Verdict, WakePolicy, WakeRule, Was, Word, Writer, WriterSlot,
};
pub use capacity::{Capacity, CapacityAction, CapacityCause, CapacityRefusal};

pub use control::{
    AddressChange, Amendment, AmendmentHistory, Change, Control, DependencyPlace, History, Milestone, Settings,
    Tracking, address_admit, instruction_admit,
};
pub use domain::{Domain, ViewTask, fire, max_out, step, view_phase};
pub use escalation::{Escalation, EscalationContext, EscalationDecision, EscalationHolder, EscalationOutcome};
pub use facts::Fact;
pub use failures::{Class, Retries, Retry, Tries};
pub use funders::FundingRecord;
pub use grace::Grace;
pub use holds::valid_name;
pub use limits::{Limits, worst_case};
pub use llm::{AccountWhy, CharterModels, LlmChoice, LlmFault, MODELS, ModelEffort};
pub use owned::{phase_bytes, stored_bytes, task_bytes, terminal_bytes};
pub use parts::{Part, PartKey, PartKind};
pub use periods::PeriodTimes;
pub use proposals::{
    PROPOSAL_WITHOUT_TASK, PersonAction, PersonProposal, PersonProposalState, Proposal, ProposalAction,
    ProposalDecision, ProposalHolder, ProposalKind, ProposalOutcome, ProposalState, SETTINGS_ITEMS, Setting,
    SettingEdit, SettingHead, SettingsDecision, SettingsOutcome,
};
pub use runtime::RetryNotice;
pub use value::{
    Authority, AuthorityExecutor, Budget, Delegation, Funder, Grant, Last, Models, Numbers, Pattern, ResourceScope,
    Scopes, Tools,
};

pub use run::run_attempt;
