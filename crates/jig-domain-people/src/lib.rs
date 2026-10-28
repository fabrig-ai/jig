//! People as parties (programming-model.md, 4.5; domain/people.md).
//! Keeps identities, secret-free sign-ins, project roles, keyed answers,
//! task-derived bounded inbox references and each person's result and reply read position.
//! Knows tasks only by number; the parent checks authority and makes tasks.
//! `Save`/`Erase` join the parent's decision; the parent holds replies until
//! durable (domain/engine.md, 5.6).
//!
//! The boundary admits adoption and other owner requests by key, checks their
//! shape, routes its current project role to the parent, and retains committed
//! answers for replay after a restart
//! (domain/people.md, section 5.1). This child never sees provider
//! credentials, protocol bytes, task internals or authority policy state.
//! The root supplies authenticated identities and authoritative role changes.
//! It mints fresh deployment person candidates and sign-in numbers; the protocol
//! keeps cookie secrets or their digests separately, named by the root-issued
//! sign-in number (domain/people.md, section 3; domain/engine.md, sections 4 and 5.4).
//!
//! `Domain::new` starts restoring. `step` admits rows, then `Restored`
//! validates references and arms deadlines; failed restoration stays unready.
//! Ready requests share person-scoped keys across sign-ins: identical pending
//! copies join bounded waiters, saved copies replay the outcome, and conflicting
//! payloads refuse (domain/people.md, sections 3 and 5.1).
//! Application requests carry only scope, connector and the supplied 32-byte
//! digest. Their saved `Applied` result names the connector's outcome by token;
//! the connector owns its value and revision checks (client/domain/contract.md,
//! section 3.3). They share every key, waiter, pending and retention bound with
//! jig's requests. A role in their scope admits them, or the sign-in alone for
//! the party's own; connector policy decides
//! whether that role may make the request.
//!
//! | Key state | Event | Next state | Emits |
//! | --- | --- | --- | --- |
//! | Absent | Authenticated application ask admitted by its scope | Flight | Route |
//! | Flight | Same metadata and digest | Flight | Joins bounded waiter |
//! | Flight | Another ask or digest | Flight | Key conflict |
//! | Flight | Applied token | Kept | Save and each waiter's reply marker |
//! | Kept | Same metadata and digest | Kept | Saved reply marker |
//! | Kept | Another ask or digest | Kept | Key conflict |
//! | Kept | Retention expires | Absent | Erase |
//!
//! Each `Route` requires exactly one matching `Decided`; permanent outcomes
//! save one keyed answer, while `Busy`/`NotReady` closes the flight without saving
//! so its key can retry . `Save` and `Erase` are
//! root decision outputs, not IO operations; task changes and keyed answers
//! share one atomic commit (domain/people.md, section 5.1).
//! `fire` expires at most one due sign-in per call. The caller reserves free
//! request room from `max_out`, accounts output bytes separately, and reclaims
//! retired flights at the iteration boundary. `worst_case` includes retained
//! state and bounded scratch. `Domain::role` supplies current membership for root
//! escalation coverage/standing without authentication or copying. `Domain::has_project` is a pure bounded startup
//! lookup; `Domain::person` is a pure sign-in lookup after restoration, checking
//! both its projected monotonic deadline and saved wall expiry for root result
//! routing, even before timer firing.
//! Optional diagnostic facts do not affect behavior (domain/people.md, section 3).
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
extern crate alloc;
mod settings;
pub use settings::{Running, Setting, SettingFault};
mod scope;
pub use scope::ApplicationScope;
mod templates;
pub use templates::{TemplateBudget, TemplateDeadline, TemplateExecutor, TemplateOverride};
mod amendment;
mod names;
pub use names::{NAME_BYTES, NameRule, name_rule, same_name};
mod models;
pub use models::{
    MODEL_DESCRIPTION_BYTES, MODEL_NAME_BYTES, Models, model_names_worst_case, remap_models, valid_model_context,
    valid_model_names,
};
mod batch;
pub use batch::{
    Action, Budget, Contract, Deadline, Dependency, EndedBy, FieldRule, Flags, Given, Member, Retention, TaskExecutor,
    Verdict, member_bytes, shaped,
};
mod boundary;
mod results;
pub use results::{Forgettable, ReplyRef, ResultRef, Seen, Update};
mod capacity;
mod compound;
pub use compound::Operation;
mod decisions;
mod domain;
pub use domain::EndedMark;
mod facts;
mod limits;
mod llm;
pub use llm::{AccountMoveWhy, AccountWhy, LlmChoice, LlmWhy, MODELS, ModelEffort, SignInRedirectWhy, llm_bytes};
mod policy_value;
mod refusals;
#[cfg(test)]
mod tests;
mod words;
pub use amendment::{
    AddressChange, Amendment, Authority, Delegation, Executor, Grant, Last, Parameter, Pattern, PersonAddress,
    ResourceScope, ResultsWake, Spec, Tracking, WakePolicy, WakeRule, amendment_bytes, authority_bytes,
};
pub use boundary::{
    ADDRESS, AccountChange, AccountHolder, AccountHostFailure, AccountHostWhy, Act, ActGrant, ActKey, ActKind,
    Adoption, Ask, Counts, Entry, EntryKind, EscalationChoice, EscalationDecision, Event, Holding, Identity,
    IdentityKey, InitialOwner, Key, Kind, Needs, NoteChange, NoteScope, Outcome, PersonResult, ProjectRole,
    ProposalChoice, ProposalDecision, Proposer, Reply, Request, RequestKey, ResourceName, ResourceRole, Role, Seed,
    Signer, Stored, USES, Use, Whom,
};
pub use capacity::{CapacityAction, CapacityCause, CapacityRefusal};
pub use decisions::{Bound, Exceeded, FailureClass, HoldReason, Moved};
pub use domain::{Domain, application_part, application_part_done, fire, max_out, step};
pub use facts::Fact;
pub use limits::{Limits, outcome_bytes, valid, worst_case};
pub use policy_value::{
    Guard, Judge, PermissionRole, PolicyChange, PolicyRole, PolicyValue, Requirement, policy_bytes,
    policy_change_bytes, requirements_bytes,
};
pub use refusals::{
    Accepter, Again, DependencyStatus, Ending, HoldOwner, Lack, LimitKind, Malformed, NAMED_TASKS, Party, Refusal,
};

/// The source directory's portable public grammar (accounts.md, 3.4).
#[must_use]
pub fn program_name(name: &[u8]) -> bool {
    if name.is_empty() || name.len() > 32 {
        return false;
    }
    for byte in name {
        if !byte.is_ascii_lowercase() && !byte.is_ascii_digit() && *byte != b'-' {
            return false;
        }
    }
    true
}

/// The deployment’s roster, not a project (people.md, 3: the deployment’s roles).
pub const DEPLOYMENT: u32 = 0;
