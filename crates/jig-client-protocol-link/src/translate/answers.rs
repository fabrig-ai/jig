//! Field-for-field client contract translation (protocol/client.md, 3 and 5).
use crate::Limits;
use crate::translate::text_problem;
use alloc::boxed::Box;
use jig_client_domain::contract as c;
use jig_client_protocol_wire::v19 as w;
use skein_lib::{List, Reader, Wall, bytes};
pub(crate) fn seen(value: &w::SeenThrough, _limits: &Limits) -> c::SeenThrough {
    match value {
        w::SeenThrough::Results(row) => c::SeenThrough::Results { through: row.through() },
        w::SeenThrough::Conversation(row) => c::SeenThrough::Conversation { task: row.task(), through: row.through() },
    }
}

pub(crate) fn proposer(value: &w::Proposer, _limits: &Limits) -> c::Proposer {
    match value {
        w::Proposer::Task(row) => c::Proposer::Task(row.value()),
        w::Proposer::Party(row) => c::Proposer::Party(row.value()),
    }
}

pub(crate) fn proposal_key(value: &w::ProposalKey, limits: &Limits) -> c::ProposalKey {
    c::ProposalKey { proposer: proposer(value.proposer(), limits), number: value.number() }
}

pub(crate) fn owner_proposal_choice(value: &w::OwnerProposalChoice, _limits: &Limits) -> c::OwnerProposalChoice {
    match value {
        w::OwnerProposalChoice::Accepted => c::OwnerProposalChoice::Accepted,
        w::OwnerProposalChoice::Rejected => c::OwnerProposalChoice::Rejected,
        w::OwnerProposalChoice::Passed => c::OwnerProposalChoice::Passed,
        w::OwnerProposalChoice::Withdrawn => c::OwnerProposalChoice::Withdrawn,
        w::OwnerProposalChoice::Stale => c::OwnerProposalChoice::Stale,
    }
}

pub(crate) fn revision(value: &w::Revision, _limits: &Limits) -> c::Revision {
    c::Revision { value: value.value() }
}

pub(crate) fn owner_escalation_choice(value: &w::OwnerEscalationChoice, _limits: &Limits) -> c::OwnerEscalationChoice {
    match value {
        w::OwnerEscalationChoice::Released => c::OwnerEscalationChoice::Released,
        w::OwnerEscalationChoice::Rejected => c::OwnerEscalationChoice::Rejected,
        w::OwnerEscalationChoice::Passed => c::OwnerEscalationChoice::Passed,
    }
}

#[expect(clippy::manual_map, reason = "step code matches optional fields")]
pub(crate) fn owner_outcome(value: &w::OwnerOutcome, limits: &Limits) -> Result<c::OwnerOutcome, w::Problem> {
    Ok(match value {
        w::OwnerOutcome::Created(row) => c::OwnerOutcome::Created {
            first: row.first(),
            last: row.last(),
            given: match row.given() {
                Some(v) => Some(c::CreatedGiven { task: v.task(), amount: v.amount() }),
                None => None,
            },
        },
        w::OwnerOutcome::Compound(row) => c::OwnerOutcome::Compound {
            project: row.project(),
            operations: row.operations(),
            first_message: row.first_message(),
        },
        w::OwnerOutcome::MarkedSeen(row) => c::OwnerOutcome::MarkedSeen { seen: seen(row.seen(), limits) },
        w::OwnerOutcome::NoteEdited(row) => c::OwnerOutcome::NoteEdited { name: row.name() },
        w::OwnerOutcome::ServiceMade(row) => c::OwnerOutcome::ServiceMade { person: row.person() },
        w::OwnerOutcome::Adopted(row) => c::OwnerOutcome::Adopted { project: row.project() },
        w::OwnerOutcome::Stopped(row) => c::OwnerOutcome::Stopped { task: row.task() },
        w::OwnerOutcome::Cancelled(row) => c::OwnerOutcome::Cancelled { task: row.task() },
        w::OwnerOutcome::Placed(row) => {
            c::OwnerOutcome::Placed { hold: row.hold(), revision: revision(row.revision(), limits) }
        }
        w::OwnerOutcome::Lifted(row) => c::OwnerOutcome::Lifted { hold: row.hold(), released: row.released() },
        w::OwnerOutcome::Released(row) => c::OwnerOutcome::Released { task: row.task() },
        w::OwnerOutcome::PersonTaken(row) => c::OwnerOutcome::PersonTaken { task: row.task() },
        w::OwnerOutcome::PersonHandedBack(row) => c::OwnerOutcome::PersonHandedBack { task: row.task() },
        w::OwnerOutcome::PersonAnswered(row) => c::OwnerOutcome::PersonAnswered { task: row.task() },
        w::OwnerOutcome::Moved(row) => c::OwnerOutcome::Moved { task: row.task() },
        w::OwnerOutcome::ProposalDecided(row) => c::OwnerOutcome::ProposalDecided {
            proposal: proposal_key(row.proposal(), limits),
            by: row.by(),
            at: Wall::from_nanos(row.at()),
            choice: owner_proposal_choice(row.choice(), limits),
            repeated: row.repeated(),
        },
        w::OwnerOutcome::Said(row) => c::OwnerOutcome::Said { task: row.task(), message: row.message() },
        w::OwnerOutcome::QuestionAnswered(row) => {
            c::OwnerOutcome::QuestionAnswered { task: row.task(), question: row.question(), message: row.message() }
        }
        w::OwnerOutcome::Amended(row) => c::OwnerOutcome::Amended { task: row.task() },
        w::OwnerOutcome::Proposed(row) => c::OwnerOutcome::Proposed { proposal: row.proposal(), to: row.to() },
        w::OwnerOutcome::Withdrawn(row) => c::OwnerOutcome::Withdrawn { proposal: row.proposal() },
        w::OwnerOutcome::RolesSet(row) => c::OwnerOutcome::RolesSet { project: row.project() },
        w::OwnerOutcome::Renamed(row) => c::OwnerOutcome::Renamed { project: row.project() },
        w::OwnerOutcome::ProjectMade(row) => c::OwnerOutcome::ProjectMade { project: row.project() },
        w::OwnerOutcome::PolicyChanged(row) => c::OwnerOutcome::PolicyChanged { project: row.project() },
        w::OwnerOutcome::AccountSignInStarted(row) => {
            c::OwnerOutcome::AccountSignInStarted { source: row.source(), address: bytes::copy_of(row.address()) }
        }
        w::OwnerOutcome::AccountKeyKept(row) => c::OwnerOutcome::AccountKeyKept { source: row.source() },
        w::OwnerOutcome::AccountProgramDeclared(row) => {
            if !c::valid_program_name(row.name()) {
                return Err(super::piece_problem(skein_codec::Reason::Malformed));
            }
            c::OwnerOutcome::AccountProgramDeclared { source: row.source(), name: bytes::copy_of(row.name()) }
        }
        w::OwnerOutcome::AccountFinishing(row) => c::OwnerOutcome::AccountFinishing { source: row.source() },
        w::OwnerOutcome::AccountMovedIn(row) => {
            c::OwnerOutcome::AccountMovedIn { account: row.account(), renewed: row.renewed() }
        }
        w::OwnerOutcome::AccountChanged => c::OwnerOutcome::AccountChanged,
        w::OwnerOutcome::AccountRemoved(row) => c::OwnerOutcome::AccountRemoved {
            confirmed: row.confirmed(),
            failed_hosts: super::account_hosts::decode(row.failed_hosts(), row.confirmed(), limits)?,
        },
        w::OwnerOutcome::AccountSignedOut(row) => c::OwnerOutcome::AccountSignedOut {
            confirmed: row.confirmed(),
            failed_hosts: super::account_hosts::decode(row.failed_hosts(), row.confirmed(), limits)?,
        },
        w::OwnerOutcome::PoolSet(row) => c::OwnerOutcome::PoolSet { project: row.project(), person: row.person() },
        w::OwnerOutcome::EscalationDecided(row) => c::OwnerOutcome::EscalationDecided {
            task: row.task(),
            revision: revision(row.revision(), limits),
            by: row.by(),
            at: Wall::from_nanos(row.at()),
            choice: owner_escalation_choice(row.choice(), limits),
        },
    })
}

pub(crate) fn owner_pattern_end(value: &w::OwnerPatternEnd, _limits: &Limits) -> c::OwnerPatternEnd {
    match value {
        w::OwnerPatternEnd::Exact(row) => c::OwnerPatternEnd::Exact(bytes::copy_of(row.value())),
        w::OwnerPatternEnd::Open(row) => c::OwnerPatternEnd::Open(bytes::copy_of(row.value())),
    }
}

pub(crate) fn owner_pattern(value: &w::OwnerPattern, limits: &Limits) -> c::OwnerPattern {
    let mut list0 = List::with_capacity(value.segments().len());
    for item in value.segments() {
        list0.push(bytes::copy_of(item)).expect("reserved supplied list");
    }
    c::OwnerPattern { segments: list0, last: owner_pattern_end(value.last(), limits) }
}

pub(crate) fn note_scope(value: &w::NoteScope, limits: &Limits) -> c::NoteScope {
    match value {
        w::NoteScope::Deployment => c::NoteScope::Deployment,
        w::NoteScope::Project(row) => c::NoteScope::Project(row.value()),
        w::NoteScope::Resource(row) => c::NoteScope::Resource {
            project: row.project(),
            connector: row.connector(),
            pattern: owner_pattern(row.pattern(), limits),
        },
        w::NoteScope::Task(row) => c::NoteScope::Task { project: row.project(), task: row.task() },
    }
}

pub(crate) fn made(value: &w::Made, limits: &Limits) -> c::Made {
    match value {
        w::Made::Tasks(row) => {
            let mut list0 = List::with_capacity(row.tasks().len());
            for item in row.tasks() {
                list0.push(*item).expect("reserved supplied list");
            }
            c::Made::Tasks { tasks: list0 }
        }
        w::Made::MovingOut(row) => {
            c::Made::MovingOut { account: row.account(), number: row.number(), record: row.record().clone() }
        }
        w::Made::Service(row) => c::Made::Service { party: row.party() },
        w::Made::PersonResult(row) => c::Made::PersonResult { task: row.task() },
        w::Made::Note(row) => c::Made::Note { note: row.note(), scope: note_scope(row.scope(), limits) },
    }
}

pub(crate) fn actor(value: &w::Actor, _limits: &Limits) -> c::Actor {
    match value {
        w::Actor::Party(row) => c::Actor::Party(row.value()),
        w::Actor::Task(row) => c::Actor::Task(row.value()),
        w::Actor::Deployment => c::Actor::Deployment,
        w::Actor::ProjectDeployment(row) => c::Actor::ProjectDeployment { project: row.project() },
    }
}

#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn decision(value: &w::Decision, limits: &Limits) -> Result<c::Decision, w::Problem> {
    Ok(match value {
        w::Decision::Accepted => c::Decision::Accepted,
        w::Decision::Rejected(row) => c::Decision::Rejected {
            reason: match c::Text::new(bytes::copy_of(row.reason()), limits.client.text) {
                Ok(value) => value,
                Err(problem) => return Err(text_problem(problem)),
            },
        },
        w::Decision::PassedUp => c::Decision::PassedUp,
        w::Decision::Released(row) => {
            let opt0 = match row.again() {
                Some(item) => Some(*item),
                None => None,
            };
            c::Decision::Released { again: opt0 }
        }
        w::Decision::LeftHeld(row) => c::Decision::LeftHeld {
            reason: match c::Text::new(bytes::copy_of(row.reason()), limits.client.text) {
                Ok(value) => value,
                Err(problem) => return Err(text_problem(problem)),
            },
        },
    })
}

pub(crate) fn party_kind(value: &w::PartyKind, _limits: &Limits) -> c::PartyKind {
    match value {
        w::PartyKind::Person => c::PartyKind::Person,
        w::PartyKind::Service => c::PartyKind::Service,
    }
}

pub(crate) fn party(value: &w::Party, limits: &Limits) -> Result<c::Party, w::Problem> {
    Ok(c::Party {
        number: value.number(),
        kind: party_kind(value.kind(), limits),
        name: match c::Text::new(bytes::copy_of(value.name()), limits.client.text) {
            Ok(value) => value,
            Err(problem) => return Err(text_problem(problem)),
        },
    })
}

pub(crate) fn outcome(value: &w::Outcome, limits: &Limits) -> Result<c::Outcome, w::Problem> {
    Ok(match value {
        w::Outcome::Owner(row) => c::Outcome::Owner(owner_outcome(row.value(), limits)?),
        w::Outcome::Made(row) => c::Outcome::Made { what: made(row.what(), limits) },
        w::Outcome::Proposed(row) => c::Outcome::Proposed { proposal: proposal_key(row.proposal(), limits) },
        w::Outcome::Decided(row) => c::Outcome::Decided {
            by: actor(row.by(), limits),
            how: decision(row.how(), limits)?,
            at: Wall::from_nanos(row.at()),
        },
        w::Outcome::DecidedFirst(row) => c::Outcome::DecidedFirst {
            by: party(row.by(), limits)?,
            how: decision(row.how(), limits)?,
            at: Wall::from_nanos(row.at()),
        },
        w::Outcome::MovedToNewest(value) => c::Outcome::MovedToNewest(c::OutcomeMovedToNewest {
            newest: value.newest(),
            moved: value.moved(),
            stayed: value.stayed(),
        }),
        w::Outcome::SettingsChanged(value) => {
            c::Outcome::SettingsChanged(super::settings::restore_outcome_settings_changed(value, limits)?)
        }
        w::Outcome::Changed => c::Outcome::Changed,
    })
}

pub(crate) fn held_role(value: &w::HeldRole, _limits: &Limits) -> c::HeldRole {
    match value {
        w::HeldRole::Owner => c::HeldRole::Owner,
        w::HeldRole::Maintainer => c::HeldRole::Maintainer,
        w::HeldRole::Member => c::HeldRole::Member,
        w::HeldRole::Observer => c::HeldRole::Observer,
        w::HeldRole::Policy(row) => c::HeldRole::Policy { role: row.role() },
    }
}

pub(crate) fn grant(value: &w::Grant, limits: &Limits) -> c::Grant {
    c::Grant { connector: value.connector(), kind: value.kind(), pattern: owner_pattern(value.pattern(), limits) }
}

pub(crate) fn executor(value: &w::Executor, _limits: &Limits) -> c::Executor {
    match value {
        w::Executor::Agent(row) => c::Executor::Agent { charter: row.charter() },
        w::Executor::Procedure(row) => c::Executor::Procedure { connector: row.connector(), kind: row.kind() },
        w::Executor::Party(row) => c::Executor::Party { party: row.party() },
        w::Executor::Role(row) => c::Executor::Role { project: row.project(), role: row.role() },
    }
}

pub(crate) fn hold_owner(value: &w::HoldOwner) -> c::HoldOwner {
    match value {
        w::HoldOwner::Party(row) => c::HoldOwner::Party { person: row.person() },
        w::HoldOwner::Task(row) => c::HoldOwner::Task { task: row.task() },
    }
}
pub(crate) fn placed(value: Option<&w::PlacedLine>, limits: &Limits) -> Result<Option<c::PlacedHold>, w::Problem> {
    let Some(value) = value else { return Ok(None) };
    if value.reason().len() > usize::try_from(limits.client.text).expect("text bound fits") {
        return Err(super::too_large());
    }
    Ok(Some(c::PlacedHold {
        number: value.number(),
        owner: hold_owner(value.owner()),
        mode: match value.mode() {
            w::HoldMode::Stop => c::HoldMode::Stop,
            w::HoldMode::Drain => c::HoldMode::Drain,
        },
        scope: match value.scope() {
            w::HoldScope::Subtree(row) => c::HoldScope::Subtree { task: row.task() },
            w::HoldScope::Project => c::HoldScope::Project,
        },
        reason: value.reason().into(),
        placed_at: Wall::from_nanos(value.placed_at()),
        revision: c::Revision { value: value.revision() },
        lifted: value.lifted(),
        may_lift: value.may_lift(),
    }))
}
#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn lacking(value: &w::Lacking, limits: &Limits) -> c::Lacking {
    match value {
        w::Lacking::Approver => c::Lacking::Approver,
        w::Lacking::Budget(row) => c::Lacking::Budget { needed: row.needed(), left: row.left() },
        w::Lacking::Grant(row) => c::Lacking::Grant(grant(row.value(), limits)),
        w::Lacking::GrantKind(row) => c::Lacking::GrantKind { kind: row.kind() },
        w::Lacking::Funding => c::Lacking::Funding,
        w::Lacking::Tool => c::Lacking::Tool,
        w::Lacking::Scope => c::Lacking::Scope,
        w::Lacking::Models => c::Lacking::Models,
        w::Lacking::HoldOwner(row) => c::Lacking::HoldOwner { hold: row.hold(), owner: hold_owner(row.owner()) },
        w::Lacking::Reference => c::Lacking::Reference,
        w::Lacking::ResourceAccess => c::Lacking::ResourceAccess,
        w::Lacking::ResourceUnavailable => c::Lacking::ResourceUnavailable,
        w::Lacking::Shape => c::Lacking::Shape,
        w::Lacking::Again(row) => c::Lacking::Again { required: row.required() },
        w::Lacking::Account => c::Lacking::Account,
        w::Lacking::Writer => c::Lacking::Writer,
        w::Lacking::Required(row) => c::Lacking::Required { connector: row.connector(), kind: row.kind() },
        w::Lacking::Failed(row) => c::Lacking::Failed { connector: row.connector(), kind: row.kind() },
        w::Lacking::Unguarded(row) => c::Lacking::Unguarded { connector: row.connector(), kind: row.kind() },
        w::Lacking::TaskKind(row) => {
            let opt0 = match &row.executor() {
                Some(item) => Some(executor(item, limits)),
                None => None,
            };
            c::Lacking::TaskKind { executor: opt0 }
        }
        w::Lacking::Depth(row) => {
            let opt0 = match row.needed() {
                Some(item) => Some(*item),
                None => None,
            };
            c::Lacking::Depth { needed: opt0, allowed: row.allowed() }
        }
        w::Lacking::Deadline(row) => {
            let opt0 = match row.asked() {
                Some(item) => Some(Wall::from_nanos(*item)),
                None => None,
            };
            let opt1 = match row.latest() {
                Some(item) => Some(Wall::from_nanos(*item)),
                None => None,
            };
            c::Lacking::Deadline { asked: opt0, latest: opt1 }
        }
        w::Lacking::Tasks(row) => {
            let opt0 = match row.needed() {
                Some(item) => Some(*item),
                None => None,
            };
            c::Lacking::Tasks { needed: opt0, left: row.left() }
        }
    }
}

pub(crate) fn admission_limit(value: &w::AdmissionLimit, _limits: &Limits) -> c::AdmissionLimit {
    match value {
        w::AdmissionLimit::Goals => c::AdmissionLimit::Goals,
        w::AdmissionLimit::Operations => c::AdmissionLimit::Operations,
        w::AdmissionLimit::Grace => c::AdmissionLimit::Grace,
        w::AdmissionLimit::Words => c::AdmissionLimit::Words,
        w::AdmissionLimit::Identity => c::AdmissionLimit::Identity,
        w::AdmissionLimit::Roles => c::AdmissionLimit::Roles,
        w::AdmissionLimit::Arithmetic => c::AdmissionLimit::Arithmetic,
        w::AdmissionLimit::Duplicate => c::AdmissionLimit::Duplicate,
        w::AdmissionLimit::Empty => c::AdmissionLimit::Empty,
        w::AdmissionLimit::Batch => c::AdmissionLimit::Batch,
        w::AdmissionLimit::ProjectLiveTasks => c::AdmissionLimit::ProjectLiveTasks,
        w::AdmissionLimit::LiveTasks => c::AdmissionLimit::LiveTasks,
        w::AdmissionLimit::Tree => c::AdmissionLimit::Tree,
        w::AdmissionLimit::Depth => c::AdmissionLimit::Depth,
        w::AdmissionLimit::Delegates => c::AdmissionLimit::Delegates,
        w::AdmissionLimit::Subscription => c::AdmissionLimit::Subscription,
        w::AdmissionLimit::Dependencies => c::AdmissionLimit::Dependencies,
        w::AdmissionLimit::Cycle => c::AdmissionLimit::Cycle,
        w::AdmissionLimit::Spec => c::AdmissionLimit::Spec,
        w::AdmissionLimit::Contract => c::AdmissionLimit::Contract,
        w::AdmissionLimit::Authority => c::AdmissionLimit::Authority,
        w::AdmissionLimit::Inputs => c::AdmissionLimit::Inputs,
        w::AdmissionLimit::Attempts => c::AdmissionLimit::Attempts,
        w::AdmissionLimit::LiveDelegates => c::AdmissionLimit::LiveDelegates,
        w::AdmissionLimit::Restore => c::AdmissionLimit::Restore,
        w::AdmissionLimit::Read => c::AdmissionLimit::Read,
        w::AdmissionLimit::Turn => c::AdmissionLimit::Turn,
        w::AdmissionLimit::HoldKind => c::AdmissionLimit::HoldKind,
        w::AdmissionLimit::HoldTaken => c::AdmissionLimit::HoldTaken,
        w::AdmissionLimit::Holds => c::AdmissionLimit::Holds,
        w::AdmissionLimit::InboxFull => c::AdmissionLimit::InboxFull,
        w::AdmissionLimit::Notes => c::AdmissionLimit::Notes,
        w::AdmissionLimit::Revision => c::AdmissionLimit::Revision,
        w::AdmissionLimit::Question => c::AdmissionLimit::Question,
        w::AdmissionLimit::Priority => c::AdmissionLimit::Priority,
        w::AdmissionLimit::Policy => c::AdmissionLimit::Policy,
        w::AdmissionLimit::Resource => c::AdmissionLimit::Resource,
        w::AdmissionLimit::Service => c::AdmissionLimit::Service,
        w::AdmissionLimit::PlacedHolds => c::AdmissionLimit::PlacedHolds,
        w::AdmissionLimit::ProjectPlacedHolds => c::AdmissionLimit::ProjectPlacedHolds,
        w::AdmissionLimit::PartyPlacedHolds => c::AdmissionLimit::PartyPlacedHolds,
        w::AdmissionLimit::Exemptions => c::AdmissionLimit::Exemptions,
        w::AdmissionLimit::Accounts => c::AdmissionLimit::Accounts,
        w::AdmissionLimit::HolderAccounts => c::AdmissionLimit::HolderAccounts,
        w::AdmissionLimit::Sources => c::AdmissionLimit::Sources,
        w::AdmissionLimit::SignIns => c::AdmissionLimit::SignIns,
        w::AdmissionLimit::SettingsProposals => c::AdmissionLimit::SettingsProposals,
        w::AdmissionLimit::Projects => c::AdmissionLimit::Projects,
    }
}

pub(crate) fn limit(value: &w::Limit, limits: &Limits) -> c::Limit {
    match value {
        w::Limit::Admission(row) => c::Limit::Admission { which: admission_limit(row.which(), limits) },
        w::Limit::LiveTasks(row) => c::Limit::LiveTasks { project: row.project(), limit: row.limit() },
        w::Limit::Inbox(row) => c::Limit::Inbox { task: row.task(), limit: row.limit() },
        w::Limit::Words(row) => c::Limit::Words { limit: row.limit() },
        w::Limit::Fields => c::Limit::Fields,
        w::Limit::Records => c::Limit::Records,
        w::Limit::Requests => c::Limit::Requests,
        w::Limit::Reads => c::Limit::Reads,
        w::Limit::Watches => c::Limit::Watches,
        w::Limit::Objects => c::Limit::Objects,
    }
}

fn hold_reason(value: &w::HoldReason, limits: &Limits) -> Result<c::HoldReason, w::Problem> {
    super::facts::hold_reason(value, limits)
}

pub(crate) fn hold_failure(value: &w::HoldFailure, _limits: &Limits) -> c::HoldFailure {
    match value {
        w::HoldFailure::Transient => c::HoldFailure::Transient,
        w::HoldFailure::Permanent => c::HoldFailure::Permanent,
        w::HoldFailure::Run => c::HoldFailure::Run,
        w::HoldFailure::Agent => c::HoldFailure::Agent,
        w::HoldFailure::Lost => c::HoldFailure::Lost,
        w::HoldFailure::Invalid => c::HoldFailure::Invalid,
    }
}

pub(crate) fn hold_category(value: &w::HoldCategory, limits: &Limits) -> c::HoldCategory {
    match value {
        w::HoldCategory::Exceeded(value) => c::HoldCategory::Exceeded(c::Exceeded {
            bound: match value.bound() {
                w::Bound::Reasoning => c::Bound::Reasoning,
                w::Bound::Metadata => c::Bound::Metadata,
                w::Bound::Items => c::Bound::Items,
                w::Bound::Answer => c::Bound::Answer,
                w::Bound::Tools => c::Bound::Tools,
                w::Bound::HistoryItems => c::Bound::HistoryItems,
                w::Bound::History => c::Bound::History,
                w::Bound::Window => c::Bound::Window,
                w::Bound::Output => c::Bound::Output,
            },
            value: value.value(),
        }),
        w::HoldCategory::Failures(row) => c::HoldCategory::Failures { class: hold_failure(row.class(), limits) },
        w::HoldCategory::Stalled => c::HoldCategory::Stalled,
        w::HoldCategory::Dependency(row) => c::HoldCategory::Dependency { task: row.task() },
        w::HoldCategory::Stopped => c::HoldCategory::Stopped,
        w::HoldCategory::Placed(row) => c::HoldCategory::Placed { hold: row.hold() },
        w::HoldCategory::Drift => c::HoldCategory::Drift,
        w::HoldCategory::Effects => c::HoldCategory::Effects,
        w::HoldCategory::EffectFailed => c::HoldCategory::EffectFailed,
        w::HoldCategory::Uncertain(row) => c::HoldCategory::Uncertain { entry: row.entry() },
        w::HoldCategory::Budget => c::HoldCategory::Budget,
        w::HoldCategory::Deadline => c::HoldCategory::Deadline,
        w::HoldCategory::HoldsWaited => c::HoldCategory::HoldsWaited,
        w::HoldCategory::ResourceUnavailable => c::HoldCategory::ResourceUnavailable,
        w::HoldCategory::ResourceAccess => c::HoldCategory::ResourceAccess,
        w::HoldCategory::PoolLost(row) => c::HoldCategory::PoolLost { pool: row.pool() },
        w::HoldCategory::Procedure => c::HoldCategory::Procedure,
        w::HoldCategory::Circle => c::HoldCategory::Circle,
    }
}

pub(crate) fn current_hold(value: &w::CurrentHold, limits: &Limits) -> Result<c::CurrentHold, w::Problem> {
    Ok(match value {
        w::CurrentHold::Document(row) => c::CurrentHold::Document(Box::new(hold_reason(row.value(), limits)?)),
        w::CurrentHold::Reason(row) => c::CurrentHold::Reason(hold_category(row.value(), limits)),
    })
}

pub(crate) fn moved(value: &w::Moved, limits: &Limits) -> Result<c::Moved, w::Problem> {
    Ok(match value {
        w::Moved::Hold(row) => {
            let opt0 = match &row.reason() {
                Some(item) => Some(current_hold(item, limits)?),
                None => None,
            };
            c::Moved::Hold { reason: opt0 }
        }
        w::Moved::PlacedHold(row) => c::Moved::PlacedHold { revision: c::Revision { value: row.revision() } },
        w::Moved::Spec => c::Moved::Spec,
        w::Moved::PersonTask => c::Moved::PersonTask,
        w::Moved::Note => c::Moved::Note,
        w::Moved::Waiting => c::Moved::Waiting,
    })
}

fn ending(value: &w::Ending, limits: &Limits) -> Result<c::Ending, w::Problem> {
    super::facts::ending(value, limits)
}

#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn refusal(value: &w::Refusal, limits: &Limits) -> Result<c::Refusal, w::Problem> {
    Ok(match value {
        w::Refusal::Role(row) => {
            let opt0 = match &row.held() {
                Some(item) => Some(held_role(item, limits)),
                None => None,
            };
            c::Refusal::Role { held: opt0 }
        }
        w::Refusal::Taken(row) => c::Refusal::Taken { by: row.by() },
        w::Refusal::Circle(value) => c::Refusal::Circle { tasks: value.tasks().clone(), cut: value.cut() },
        w::Refusal::Kept(value) => c::Refusal::Kept { pending: value.pending().clone() },
        w::Refusal::Dependency(value) => c::Refusal::Dependency {
            dependency: value.dependency(),
            how: match value.how() {
                w::OwnerTaskStatus::Done => c::OwnerTaskStatus::Done,
                w::OwnerTaskStatus::Failed => c::OwnerTaskStatus::Failed,
                w::OwnerTaskStatus::Cancelled => c::OwnerTaskStatus::Cancelled,
            },
        },
        w::Refusal::Stale(value) => c::Refusal::Stale(super::settings::restore_refusal_stale(value, limits)?),
        w::Refusal::Full(value) => c::Refusal::Full(super::settings::restore_refusal_full(value, limits)?),
        w::Refusal::NotWaiting => c::Refusal::NotWaiting,
        w::Refusal::NoFurther => c::Refusal::NoFurther,
        w::Refusal::Authority(row) => {
            let opt0 = match row.proposable() {
                Some(item) => Some(*item),
                None => None,
            };
            c::Refusal::Authority { lacking: lacking(row.lacking(), limits), proposable: opt0 }
        }
        w::Refusal::Limit(row) => c::Refusal::Limit { which: limit(row.which(), limits) },
        w::Refusal::Moved(row) => {
            c::Refusal::Moved { now: revision(row.now(), limits), what: moved(row.what(), limits)? }
        }
        w::Refusal::Ended(row) => c::Refusal::Ended { how: ending(row.how(), limits)? },
        w::Refusal::Malformed(row) => c::Refusal::Malformed {
            at: row.at(),
            why: match row.why() {
                w::Malformed::Settings(value) => {
                    c::Malformed::Settings(super::settings::restore_malformed_settings(value, limits)?)
                }
                w::Malformed::Running => c::Malformed::Running,
                w::Malformed::SettingsUnchanged => c::Malformed::SettingsUnchanged,
                w::Malformed::TwoAsAllowed => c::Malformed::TwoAsAllowed,
                w::Malformed::ConversationNotAgent => c::Malformed::ConversationNotAgent,

                w::Malformed::Name(value) => c::Malformed::Name(match value.rule() {
                    w::NameRule::Empty => c::NameRule::Empty,
                    w::NameRule::Long => c::NameRule::Long,
                    w::NameRule::Control => c::NameRule::Control,
                    w::NameRule::Space => c::NameRule::Space,
                    w::NameRule::Digits => c::NameRule::Digits,
                }),
                w::Malformed::EndedBy => c::Malformed::EndedBy,
                w::Malformed::Cycle => c::Malformed::Cycle,
                w::Malformed::ProposedAsAllowed => c::Malformed::ProposedAsAllowed,
                w::Malformed::ProjectMismatch => c::Malformed::ProjectMismatch,
            },
        },
        w::Refusal::SignInRedirect(row) => c::Refusal::SignInRedirect {
            why: match row.why() {
                w::SignInRedirectWhy::NotWaiting => c::SignInRedirectWhy::NotWaiting,
                w::SignInRedirectWhy::Address => c::SignInRedirectWhy::Address,
                w::SignInRedirectWhy::State => c::SignInRedirectWhy::State,
                w::SignInRedirectWhy::TooMany => c::SignInRedirectWhy::TooMany,
            },
        },
        w::Refusal::AccountMove(row) => c::Refusal::AccountMove {
            why: match row.why() {
                w::AccountMoveWhy::NotSignedIn => c::AccountMoveWhy::NotSignedIn,
                w::AccountMoveWhy::NotOurs => c::AccountMoveWhy::NotOurs,
                w::AccountMoveWhy::Held => c::AccountMoveWhy::Held,
                w::AccountMoveWhy::Cancelled => c::AccountMoveWhy::Cancelled,
                w::AccountMoveWhy::Moving => c::AccountMoveWhy::Moving,
            },
        },
        w::Refusal::Account(row) => c::Refusal::Account {
            account: row.account(),
            why: match row.why() {
                w::AccountWhy::Unknown => c::AccountWhy::Unknown,
                w::AccountWhy::Provider => c::AccountWhy::Provider,
                w::AccountWhy::Uses => c::AccountWhy::Uses,
                w::AccountWhy::Limited => c::AccountWhy::Limited,
            },
        },
        w::Refusal::Provider => c::Refusal::Provider,
        w::Refusal::Unknown => c::Refusal::Unknown,
        w::Refusal::NoEscalationHolder => c::Refusal::NoEscalationHolder,
        w::Refusal::Llm(row) => super::llm::refusal(row, limits)?,
        w::Refusal::NameTaken => c::Refusal::NameTaken,
        w::Refusal::NotOffered => c::Refusal::NotOffered,
        w::Refusal::KeyConflict => c::Refusal::KeyConflict,
    })
}
/// Takes the shell-only value beside a made service's domain answer.
pub(crate) fn credential(record: &w::Owned, limits: &Limits) -> Option<(u64, skein_lib::Secret)> {
    if record.owner() != &w::Owner::Jig || record.version() != jig_client_protocol_wire::VERSION {
        return None;
    }
    let w::Final::Done(value) = w::Final::decode(&limits.wire, &mut Reader::new(record.record())).ok()? else {
        return None;
    };
    let w::Outcome::Made(value) = value.into_parts().outcome else { return None };
    let w::Made::Service(value) = value.into_parts().what else { return None };
    let parts = value.into_parts();
    if parts.credential.expose().is_empty() {
        return None;
    }
    Some((parts.party, parts.credential))
}
/// Decodes one final jig answer in its owner's vocabulary.
pub fn answer(record: w::KeyedAnswer, limits: &Limits) -> Result<c::Answer, super::Undecodable> {
    let value = match record {
        w::KeyedAnswer::FinalAnswer(value) => value,
        w::KeyedAnswer::NotServed(_)
        | w::KeyedAnswer::Busy
        | w::KeyedAnswer::NotReady
        | w::KeyedAnswer::AnswersFull(_)
        | w::KeyedAnswer::SignedOut => {
            return Err(super::Undecodable::Malformed);
        }
    };
    if value.owner() != &w::Owner::Jig || value.version() != jig_client_protocol_wire::VERSION {
        return Err(super::Undecodable::Version);
    }
    if u64::try_from(value.record().len()).expect("usize fits u64") > limits.client.document_bytes {
        return Err(super::Undecodable::TooLarge);
    }
    let value = match w::Final::decode(&limits.wire, &mut Reader::new(value.record())) {
        Ok(value) => value,
        Err(problem) => return Err(classify(problem)),
    };
    match value {
        w::Final::Done(value) => match outcome(value.outcome(), limits) {
            Ok(value) => Ok(c::Answer::Done(value)),
            Err(problem) => Err(classify(problem)),
        },
        w::Final::Refused(value) => match refusal(value.refusal(), limits) {
            Ok(refusal) => Ok(c::Answer::Refused { refusal, at: *value.at() }),
            Err(problem) => Err(classify(problem)),
        },
    }
}

fn classify(problem: w::Problem) -> super::Undecodable {
    match problem.reason {
        skein_codec::Reason::Bound => super::Undecodable::TooLarge,
        skein_codec::Reason::Malformed
        | skein_codec::Reason::Tag
        | skein_codec::Reason::Utf8
        | skein_codec::Reason::Short
        | skein_codec::Reason::Bool
        | skein_codec::Reason::Version
        | skein_codec::Reason::Trailing => super::Undecodable::Malformed,
    }
}

#[cfg(test)]
#[test]
fn original_nonzero_recipient_survives_wire_decoding_and_contract_translation() {
    use skein_lib::{Duration, RequestLimits, Writer};
    let limits = Limits::new(
        jig_client_domain::Limits {
            owners: &[],
            watches: 4,
            handles: 8,
            turn_bytes: 32,
            ask_bytes: 16384,
            requests: RequestLimits {
                requests: 4,
                bytes: 65536,
                out: 16,
                first: Duration::from_nanos(10),
                most: Duration::from_nanos(80),
                margin: Duration::from_secs(1),
            },
            fields: 4,
            reads: 4,
            parts: 8,
            delegates: 2,
            document_bytes: 65536,
            records: 2,
            kept: 4,
            projects: 2,
            text: 32,
            turn_text: 32,
            rows: 2,
            segments: 4,
            backoff: jig_client_domain::Backoff { first: Duration::from_nanos(10), most: Duration::from_nanos(80) },
            authority: 8,
            accounts: jig_client_domain::AccountCounts { accounts: 8, sources: 8, sign_ins: 8 },
            heartbeat: Duration::from_nanos(100),
        },
        4,
    )
    .expect("existing small client fixture bounds");
    for to in [7, 91, u32::MAX] {
        let original = w::OwnerOutcome::Proposed(
            w::OwnerOutcomeProposed::new(&limits.wire, w::OwnerOutcomeProposedParts { proposal: 31, to })
                .expect("recipient record"),
        );
        let mut out = Writer::new(usize::try_from(original.measure()).expect("small result"));
        original.encode(&mut out).expect("measured room");
        let bytes = out.finish();
        let decoded = w::OwnerOutcome::decode(&limits.wire, &mut Reader::new(&bytes)).expect("real record decoder");
        assert_eq!(
            owner_outcome(&decoded, &limits).expect("contract translation"),
            c::OwnerOutcome::Proposed { proposal: 31, to }
        );
    }
}
