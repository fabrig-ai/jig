//! Proposals and immutable task history (protocol/store.md, 5.1, None).
use super::{phase, value};
use crate::tasks_v9 as wire;
use alloc::boxed::Box;
use jig_domain_tasks as domain;
use skein_lib::{List, Wall};

/// Translate `ProposalKind` to its store value.
pub fn kind(source: &domain::ProposalKind, _limits: &wire::Limits) -> Result<wire::ProposalKind, wire::Problem> {
    Ok(match source {
        domain::ProposalKind::Settings => wire::ProposalKind::Settings,
        domain::ProposalKind::Batch => wire::ProposalKind::Batch,
        domain::ProposalKind::Effect => wire::ProposalKind::Effect,
        domain::ProposalKind::Amend => wire::ProposalKind::Amend,
        domain::ProposalKind::Widen => wire::ProposalKind::Widen,
        domain::ProposalKind::Release => wire::ProposalKind::Release,
    })
}
/// Read `ProposalKind` from its store value.
pub fn read_kind(source: wire::ProposalKind, _limits: &wire::Limits) -> Result<domain::ProposalKind, wire::Problem> {
    Ok(match source {
        wire::ProposalKind::Settings => domain::ProposalKind::Settings,
        wire::ProposalKind::Batch => domain::ProposalKind::Batch,
        wire::ProposalKind::Effect => domain::ProposalKind::Effect,
        wire::ProposalKind::Amend => domain::ProposalKind::Amend,
        wire::ProposalKind::Widen => domain::ProposalKind::Widen,
        wire::ProposalKind::Release => domain::ProposalKind::Release,
    })
}

/// Translate `ProposalHolder` to its store value.
pub fn holder(source: &domain::ProposalHolder, limits: &wire::Limits) -> Result<wire::ProposalHolder, wire::Problem> {
    Ok(match source {
        domain::ProposalHolder::Task(value) => {
            wire::ProposalHolder::Task(wire::Task::new(limits, wire::TaskParts { task: *value })?)
        }
        domain::ProposalHolder::Person(value) => {
            wire::ProposalHolder::Person(wire::Person::new(limits, wire::PersonParts { person: *value })?)
        }
        domain::ProposalHolder::Policy { project, kind } => wire::ProposalHolder::Policy(wire::PolicyHolder::new(
            limits,
            wire::PolicyHolderParts { project: *project, kind: self::kind(kind, limits)? },
        )?),
    })
}
/// Read `ProposalHolder` from its store value.
pub fn read_holder(
    source: wire::ProposalHolder,
    limits: &wire::Limits,
) -> Result<domain::ProposalHolder, wire::Problem> {
    Ok(match source {
        wire::ProposalHolder::Task(value) => {
            let wire::TaskParts { task } = value.into_parts();
            domain::ProposalHolder::Task(task)
        }
        wire::ProposalHolder::Person(value) => {
            let wire::PersonParts { person } = value.into_parts();
            domain::ProposalHolder::Person(person)
        }
        wire::ProposalHolder::Policy(value) => {
            let wire::PolicyHolderParts { project, kind } = value.into_parts();
            domain::ProposalHolder::Policy { project, kind: read_kind(kind, limits)? }
        }
    })
}

/// Translate `ProposalState` to its store value.
pub fn state(source: &domain::ProposalState, limits: &wire::Limits) -> Result<wire::ProposalState, wire::Problem> {
    Ok(match source {
        domain::ProposalState::Pending { holder, since } => wire::ProposalState::Pending(wire::PendingProposal::new(
            limits,
            wire::PendingProposalParts { holder: self::holder(holder, limits)?, since: since.as_nanos() },
        )?),
        domain::ProposalState::Accepted { by } => wire::ProposalState::Accepted(wire::AcceptedProposal::new(
            limits,
            wire::AcceptedProposalParts { by: value::party(by, limits)? },
        )?),
        domain::ProposalState::Rejected { by, reason } => wire::ProposalState::Rejected(wire::RejectedProposal::new(
            limits,
            wire::RejectedProposalParts { by: value::party(by, limits)?, reason: reason.clone() },
        )?),
        domain::ProposalState::Withdrawn => wire::ProposalState::Withdrawn,
    })
}
/// Read `ProposalState` from its store value.
pub fn read_state(source: wire::ProposalState, limits: &wire::Limits) -> Result<domain::ProposalState, wire::Problem> {
    Ok(match source {
        wire::ProposalState::Pending(value) => {
            let wire::PendingProposalParts { holder, since } = value.into_parts();
            domain::ProposalState::Pending { holder: read_holder(holder, limits)?, since: Wall::from_nanos(since) }
        }
        wire::ProposalState::Accepted(value) => {
            let wire::AcceptedProposalParts { by } = value.into_parts();
            domain::ProposalState::Accepted { by: value::read_party(by) }
        }
        wire::ProposalState::Rejected(value) => {
            let wire::RejectedProposalParts { by, reason } = value.into_parts();
            domain::ProposalState::Rejected { by: value::read_party(by), reason }
        }
        wire::ProposalState::Withdrawn => domain::ProposalState::Withdrawn,
    })
}

/// Translate `AddressChange` to its store record.
pub fn address_change(
    source: &domain::AddressChange,
    limits: &wire::Limits,
) -> Result<wire::AddressChange, wire::Problem> {
    let domain::AddressChange { revision, address } = source;
    wire::AddressChange::new(
        limits,
        wire::AddressChangeParts { revision: *revision, address: value::person_address(address, limits)? },
    )
}
/// Read `AddressChange` from its store record.
pub fn read_address_change(
    source: wire::AddressChange,
    _limits: &wire::Limits,
) -> Result<domain::AddressChange, wire::Problem> {
    let wire::AddressChangeParts { revision, address } = source.into_parts();
    Ok(domain::AddressChange { revision, address: value::read_person_address(address) })
}

/// Translate `Amendment` to its store record.
pub fn amendment(
    source: &domain::Amendment,
    limits: &wire::Limits,
    moved: Option<&[u64; 64]>,
) -> Result<wire::Amendment, wire::Problem> {
    let domain::Amendment {
        tracked,
        addressed,
        instructions,
        release,
        spec,
        wake,
        dependencies,
        authority,
        llm,
        reason,
    } = source;
    wire::Amendment::new(
        limits,
        wire::AmendmentParts {
            tracked: optional_tracking(*tracked, limits)?,
            addressed: optional_address_change(addressed.as_ref(), limits)?,
            instructions: instructions.clone(),
            release: *release,
            spec: optional_spec(spec.as_ref(), limits)?,
            wake: optional_wake(wake.as_ref(), limits)?,
            dependencies: optional_dependencies(dependencies.as_deref(), limits)?,
            authority: optional_authority(authority.as_ref(), limits, moved)?,
            llm: value::llm(llm.as_ref(), limits)?,
            reason: reason.clone(),
        },
    )
}
/// Read `Amendment` from its store record.
pub fn read_amendment(source: wire::Amendment, limits: &wire::Limits) -> Result<domain::Amendment, wire::Problem> {
    let wire::AmendmentParts {
        tracked,
        addressed,
        instructions,
        release,
        spec,
        wake,
        dependencies,
        authority,
        llm,
        reason,
    } = source.into_parts();
    Ok(domain::Amendment {
        tracked: read_tracking(tracked),
        addressed: read_optional_address_change(addressed, limits)?,
        instructions,
        release,
        spec: read_optional_spec(spec, limits)?,
        wake: read_optional_wake(wake, limits)?,
        dependencies: read_optional_dependencies(dependencies),
        authority: read_optional_authority(authority, limits)?,
        llm: value::read_llm(llm),
        reason,
    })
}

/// Translate `ProposalAction` to its store value.
pub fn action(
    source: &domain::ProposalAction,
    limits: &wire::Limits,
    moved: Option<&[u64; 64]>,
) -> Result<wire::ProposalAction, wire::Problem> {
    Ok(match source {
        domain::ProposalAction::Batch(value) => wire::ProposalAction::Batch(wire::ProposedBatch::new(
            limits,
            wire::ProposedBatchParts { members: batch(value, limits, moved)? },
        )?),
        domain::ProposalAction::Effect { connector, authority, attempt, completion, position, purpose } => {
            wire::ProposalAction::Effect(wire::ProposedEffect::new(
                limits,
                wire::ProposedEffectParts {
                    connector: *connector,
                    authority: value::authority(authority, limits, moved)?,
                    attempt: *attempt,
                    completion: *completion,
                    position: *position,
                    purpose: *purpose,
                },
            )?)
        }
        domain::ProposalAction::ProcedureEffect { connector, authority, step, purpose } => {
            wire::ProposalAction::ProcedureEffect(wire::ProposedProcedureEffect::new(
                limits,
                wire::ProposedProcedureEffectParts {
                    connector: *connector,
                    authority: value::authority(authority, limits, moved)?,
                    step: *step,
                    purpose: *purpose,
                },
            )?)
        }
        domain::ProposalAction::Amend { task, revision, amendment } => {
            wire::ProposalAction::Amend(Box::new(wire::ProposedAmendment::new(
                limits,
                wire::ProposedAmendmentParts {
                    task: *task,
                    revision: *revision,
                    amendment: self::amendment(amendment, limits, moved)?,
                },
            )?))
        }
        domain::ProposalAction::Widen { task, authority } => wire::ProposalAction::Widen(wire::ProposedWidening::new(
            limits,
            wire::ProposedWideningParts { task: *task, authority: value::authority(authority, limits, moved)? },
        )?),
        domain::ProposalAction::Release { task } => {
            wire::ProposalAction::Release(wire::Task::new(limits, wire::TaskParts { task: *task })?)
        }
    })
}
/// Read `ProposalAction` from its store value.
pub fn read_action(
    source: wire::ProposalAction,
    limits: &wire::Limits,
) -> Result<domain::ProposalAction, wire::Problem> {
    Ok(match source {
        wire::ProposalAction::Batch(value) => {
            let wire::ProposedBatchParts { members } = value.into_parts();
            domain::ProposalAction::Batch(read_batch(members, limits)?)
        }
        wire::ProposalAction::Effect(value) => {
            let wire::ProposedEffectParts { connector, authority, attempt, completion, position, purpose } =
                value.into_parts();
            domain::ProposalAction::Effect {
                connector,
                authority: value::read_authority(authority, limits)?,
                attempt,
                completion,
                position,
                purpose,
            }
        }
        wire::ProposalAction::ProcedureEffect(value) => {
            let wire::ProposedProcedureEffectParts { connector, authority, step, purpose } = value.into_parts();
            domain::ProposalAction::ProcedureEffect {
                connector,
                authority: value::read_authority(authority, limits)?,
                step,
                purpose,
            }
        }
        wire::ProposalAction::Amend(value) => {
            let wire::ProposedAmendmentParts { task, revision, amendment } = (*value).into_parts();
            domain::ProposalAction::Amend { task, revision, amendment: read_amendment(amendment, limits)? }
        }
        wire::ProposalAction::Widen(value) => {
            let wire::ProposedWideningParts { task, authority } = value.into_parts();
            domain::ProposalAction::Widen { task, authority: value::read_authority(authority, limits)? }
        }
        wire::ProposalAction::Release(value) => {
            let wire::TaskParts { task } = value.into_parts();
            domain::ProposalAction::Release { task }
        }
    })
}

/// Translate `Proposal` to its store record.
pub fn proposal(
    source: &domain::Proposal,
    limits: &wire::Limits,
    moved: Option<&[u64; 64]>,
) -> Result<wire::Proposal, wire::Problem> {
    let domain::Proposal { number, proposer, project, action, reason, as_holder, state } = source;
    wire::Proposal::new(
        limits,
        wire::ProposalParts {
            number: *number,
            proposer: *proposer,
            project: *project,
            action: self::action(action, limits, moved)?,
            reason: reason.clone(),
            as_holder: *as_holder,
            state: self::state(state, limits)?,
        },
    )
}
/// Read `Proposal` from its store record.
pub fn read_proposal(source: wire::Proposal, limits: &wire::Limits) -> Result<domain::Proposal, wire::Problem> {
    let wire::ProposalParts { number, proposer, project, action, reason, as_holder, state } = source.into_parts();
    Ok(domain::Proposal {
        number,
        proposer,
        project,
        action: read_action(action, limits)?,
        reason,
        as_holder,
        state: read_state(state, limits)?,
    })
}

/// Translate `Change` to its store value.
pub fn change(source: &domain::Change, _limits: &wire::Limits) -> Result<wire::Change, wire::Problem> {
    Ok(match source {
        domain::Change::Cancelled => wire::Change::Cancelled,
        domain::Change::Released => wire::Change::Released,
        domain::Change::Amended => wire::Change::Amended,
        domain::Change::Moved => wire::Change::Moved,
        domain::Change::Proposed => wire::Change::Proposed,
        domain::Change::ProposalPassed => wire::Change::ProposalPassed,
        domain::Change::ProposalAccepted => wire::Change::ProposalAccepted,
        domain::Change::ProposalRejected => wire::Change::ProposalRejected,
        domain::Change::ProposalWithdrawn => wire::Change::ProposalWithdrawn,
    })
}
/// Read `Change` from its store value.
pub fn read_change(source: wire::Change, _limits: &wire::Limits) -> Result<domain::Change, wire::Problem> {
    Ok(match source {
        wire::Change::Cancelled => domain::Change::Cancelled,
        wire::Change::Released => domain::Change::Released,
        wire::Change::Amended => domain::Change::Amended,
        wire::Change::Moved => domain::Change::Moved,
        wire::Change::Proposed => domain::Change::Proposed,
        wire::Change::ProposalPassed => domain::Change::ProposalPassed,
        wire::Change::ProposalAccepted => domain::Change::ProposalAccepted,
        wire::Change::ProposalRejected => domain::Change::ProposalRejected,
        wire::Change::ProposalWithdrawn => domain::Change::ProposalWithdrawn,
    })
}

/// Translate `Settings` to its store record.
pub fn settings(
    source: &domain::Settings,
    limits: &wire::Limits,
    moved: Option<&[u64; 64]>,
) -> Result<wire::Settings, wire::Problem> {
    let domain::Settings { tracked, addressed, taken_by, instructions, spec, wake, dependencies, authority, llm } =
        source;
    wire::Settings::new(
        limits,
        wire::SettingsParts {
            tracked: *tracked,
            addressed: optional_address(addressed.as_ref(), limits)?,
            taken_by: *taken_by,
            instructions: instructions.clone(),
            spec: value::spec(spec, limits)?,
            wake: phase::wake(wake, limits)?,
            dependencies: self::dependencies(dependencies, limits)?,
            authority: value::authority(authority, limits, moved)?,
            llm: value::llm(llm.as_ref(), limits)?,
        },
    )
}
/// Read `Settings` from its store record.
pub fn read_settings(source: wire::Settings, limits: &wire::Limits) -> Result<domain::Settings, wire::Problem> {
    let wire::SettingsParts { tracked, addressed, taken_by, instructions, spec, wake, dependencies, authority, llm } =
        source.into_parts();
    Ok(domain::Settings {
        tracked,
        addressed: read_optional_address(addressed),
        taken_by,
        instructions,
        spec: value::read_spec(spec, limits)?,
        wake: phase::read_wake(wake, limits)?,
        dependencies: phase::read_references(dependencies),
        authority: value::read_authority(authority, limits)?,
        llm: value::read_llm(llm),
    })
}

/// Translate `AmendmentHistory` to its store record.
pub fn amendment_history(
    source: &domain::AmendmentHistory,
    limits: &wire::Limits,
    moved: Option<&[u64; 64]>,
) -> Result<wire::AmendmentHistory, wire::Problem> {
    let domain::AmendmentHistory { before, after } = source;
    wire::AmendmentHistory::new(
        limits,
        wire::AmendmentHistoryParts {
            before: settings(before, limits, moved)?,
            after: settings(after, limits, moved)?,
        },
    )
}
/// Read `AmendmentHistory` from its store record.
pub fn read_amendment_history(
    source: wire::AmendmentHistory,
    limits: &wire::Limits,
) -> Result<domain::AmendmentHistory, wire::Problem> {
    let wire::AmendmentHistoryParts { before, after } = source.into_parts();
    Ok(domain::AmendmentHistory { before: read_settings(before, limits)?, after: read_settings(after, limits)? })
}

/// Translate `History` to its store record.
pub fn history(
    source: &domain::History,
    limits: &wire::Limits,
    moved: Option<&[u64; 64]>,
) -> Result<wire::History, wire::Problem> {
    let domain::History { task, revision, by, change, reason, proposal, at, amendment } = source;
    wire::History::new(
        limits,
        wire::HistoryParts {
            task: *task,
            revision: *revision,
            by: value::party(by, limits)?,
            change: self::change(change, limits)?,
            reason: reason.clone(),
            proposal: optional_proposal(proposal.as_deref(), limits, moved)?,
            at: at.as_nanos(),
            amendment: optional_amendment_history(amendment.as_deref(), limits, moved)?,
        },
    )
}
/// Read `History` from its store record.
pub fn read_history(source: wire::History, limits: &wire::Limits) -> Result<domain::History, wire::Problem> {
    let wire::HistoryParts { task, revision, by, change, reason, proposal, at, amendment } = source.into_parts();
    Ok(domain::History {
        task,
        revision,
        by: value::read_party(by),
        change: read_change(change, limits)?,
        reason,
        proposal: read_optional_proposal(proposal, limits)?,
        at: Wall::from_nanos(at),
        amendment: read_optional_amendment_history(amendment, limits)?,
    })
}

/// Translate `PersonProposalState` to its store value.
pub fn person_state(
    source: &domain::PersonProposalState,
    limits: &wire::Limits,
) -> Result<wire::PersonProposalState, wire::Problem> {
    Ok(match source {
        domain::PersonProposalState::SettingsAccepted { by, revision } => {
            wire::PersonProposalState::SettingsAccepted(wire::SettingsAccepted::new(
                limits,
                wire::SettingsAcceptedParts { by: value::party(by, limits)?, revision: *revision },
            )?)
        }
        domain::PersonProposalState::Withdrawn { at } => wire::PersonProposalState::Withdrawn(
            wire::WithdrawnPersonProposal::new(limits, wire::WithdrawnPersonProposalParts { at: at.as_nanos() })?,
        ),
        domain::PersonProposalState::Pending { since } => wire::PersonProposalState::Pending(
            wire::PendingPersonProposal::new(limits, wire::PendingPersonProposalParts { since: since.as_nanos() })?,
        ),
        domain::PersonProposalState::Accepted { by } => wire::PersonProposalState::Accepted(
            wire::AcceptedProposal::new(limits, wire::AcceptedProposalParts { by: value::party(by, limits)? })?,
        ),
        domain::PersonProposalState::Rejected { by, reason, message, at } => {
            wire::PersonProposalState::Rejected(wire::RejectedPersonProposal::new(
                limits,
                wire::RejectedPersonProposalParts {
                    by: value::party(by, limits)?,
                    reason: reason.clone(),
                    message: *message,
                    at: at.as_nanos(),
                },
            )?)
        }
    })
}
/// Read `PersonProposalState` from its store value.
pub fn read_person_state(
    source: wire::PersonProposalState,
    _limits: &wire::Limits,
) -> Result<domain::PersonProposalState, wire::Problem> {
    Ok(match source {
        wire::PersonProposalState::SettingsAccepted(row) => {
            let row = row.into_parts();
            domain::PersonProposalState::SettingsAccepted { by: value::read_party(row.by), revision: row.revision }
        }
        wire::PersonProposalState::Withdrawn(value) => {
            domain::PersonProposalState::Withdrawn { at: Wall::from_nanos(value.into_parts().at) }
        }
        wire::PersonProposalState::Pending(value) => {
            let wire::PendingPersonProposalParts { since } = value.into_parts();
            domain::PersonProposalState::Pending { since: Wall::from_nanos(since) }
        }
        wire::PersonProposalState::Accepted(value) => {
            let wire::AcceptedProposalParts { by } = value.into_parts();
            domain::PersonProposalState::Accepted { by: value::read_party(by) }
        }
        wire::PersonProposalState::Rejected(value) => {
            let wire::RejectedPersonProposalParts { by, reason, message, at } = value.into_parts();
            domain::PersonProposalState::Rejected {
                by: value::read_party(by),
                reason,
                message,
                at: Wall::from_nanos(at),
            }
        }
    })
}

/// Translate `PersonProposal` to its store record.
pub fn person_proposal(
    source: &domain::PersonProposal,
    limits: &wire::Limits,
    moved: Option<&[u64; 64]>,
) -> Result<wire::PersonProposal, wire::Problem> {
    let domain::PersonProposal { number, proposer, project, action, reason, state } = source;
    let action = match action {
        domain::PersonAction::Settings { against, items } => {
            if items.len() > usize::try_from(limits.proposed_settings_items).expect("item bound fits") {
                return Err(wire::Problem {
                    path: wire::Path::ProposedSettingsItems,
                    reason: skein_codec::Reason::Bound,
                });
            }
            let mut heads = List::with_capacity(domain::SETTINGS_ITEMS);
            for item in items {
                heads.push(setting_head(item, limits)?).expect("bounded heads");
            }
            wire::PersonAction::Settings(wire::ProposedSettings::new(
                limits,
                wire::ProposedSettingsParts { against: *against, items: heads },
            )?)
        }
        domain::PersonAction::Batch(members) => wire::PersonAction::Batch(wire::ProposedBatch::new(
            limits,
            wire::ProposedBatchParts { members: batch(members, limits, moved)? },
        )?),
        domain::PersonAction::Amend { task, revision, amendment } => {
            wire::PersonAction::Amend(Box::new(wire::ProposedAmendment::new(
                limits,
                wire::ProposedAmendmentParts {
                    task: *task,
                    revision: *revision,
                    amendment: self::amendment(amendment, limits, moved)?,
                },
            )?))
        }
    };
    wire::PersonProposal::new(
        limits,
        wire::PersonProposalParts {
            number: *number,
            proposer: *proposer,
            project: *project,
            action,
            reason: reason.clone(),
            state: person_state(state, limits)?,
        },
    )
}
/// Read `PersonProposal` from its store record.
pub fn read_person_proposal(
    source: wire::PersonProposal,
    limits: &wire::Limits,
) -> Result<domain::PersonProposal, wire::Problem> {
    let wire::PersonProposalParts { number, proposer, project, action, reason, state } = source.into_parts();
    let action = match action {
        wire::PersonAction::Settings(row) => {
            let row = row.into_parts();
            let mut items = List::with_capacity(domain::SETTINGS_ITEMS);
            for item in row.items.into_boxed() {
                items.push(read_setting_head(item)).expect("decoded heads");
            }
            domain::PersonAction::Settings { against: row.against, items: items.into_boxed() }
        }
        wire::PersonAction::Batch(batch) => {
            domain::PersonAction::Batch(read_batch(batch.into_parts().members, limits)?)
        }
        wire::PersonAction::Amend(value) => {
            let wire::ProposedAmendmentParts { task, revision, amendment } = (*value).into_parts();
            domain::PersonAction::Amend { task, revision, amendment: Box::new(read_amendment(amendment, limits)?) }
        }
    };
    Ok(domain::PersonProposal { number, proposer, project, action, reason, state: read_person_state(state, limits)? })
}

/// Translate `Milestone` to its store record.
pub fn milestone(source: &domain::Milestone, limits: &wire::Limits) -> Result<wire::Milestone, wire::Problem> {
    let domain::Milestone { task, position, phase, at } = source;
    wire::Milestone::new(
        limits,
        wire::MilestoneParts {
            task: *task,
            position: *position,
            phase: phase::phase(phase, limits)?,
            at: at.as_nanos(),
        },
    )
}
/// Read `Milestone` from its store record.
pub fn read_milestone(source: wire::Milestone, limits: &wire::Limits) -> Result<domain::Milestone, wire::Problem> {
    let wire::MilestoneParts { task, position, phase, at } = source.into_parts();
    Ok(domain::Milestone { task, position, phase: phase::read_phase(phase, limits)?, at: Wall::from_nanos(at) })
}

fn batch(
    source: &[domain::New],
    limits: &wire::Limits,
    moved: Option<&[u64; 64]>,
) -> Result<List<wire::Creation>, wire::Problem> {
    let mut members = List::with_capacity(limits.proposed_batch_members);
    for item in source {
        let item = phase::new(item, limits, moved)?;
        if members.push(item).is_err() {
            return Err(wire::Problem { path: wire::Path::ProposedBatchMembers, reason: skein_codec::Reason::Bound });
        }
    }
    Ok(members)
}
fn read_batch(source: List<wire::Creation>, limits: &wire::Limits) -> Result<Box<[domain::New]>, wire::Problem> {
    let mut members = List::with_capacity(source.len());
    for item in source.into_boxed() {
        members.push(phase::read_new(item, limits)?).expect("same number of members");
    }
    Ok(members.into_boxed())
}
fn dependencies(source: &[u64], limits: &wire::Limits) -> Result<List<wire::Number>, wire::Problem> {
    phase::references(source, limits.settings_dependencies, wire::Path::SettingsDependencies, limits)
}
fn optional_dependencies(
    source: Option<&[u64]>,
    limits: &wire::Limits,
) -> Result<Option<List<wire::Number>>, wire::Problem> {
    match source {
        Some(value) => Ok(Some(phase::references(
            value,
            limits.amendment_dependencies_some,
            wire::Path::AmendmentDependencies,
            limits,
        )?)),
        None => Ok(None),
    }
}
#[expect(clippy::manual_map, reason = "step code uses exhaustive matches instead of closures or function pointers")]
fn read_optional_dependencies(source: Option<List<wire::Number>>) -> Option<Box<[u64]>> {
    match source {
        Some(value) => Some(phase::read_references(value)),
        None => None,
    }
}
fn optional_address(
    source: Option<&domain::PersonAddress>,
    limits: &wire::Limits,
) -> Result<Option<wire::PersonAddress>, wire::Problem> {
    match source {
        Some(value) => Ok(Some(value::person_address(value, limits)?)),
        None => Ok(None),
    }
}
#[expect(clippy::manual_map, reason = "step code uses exhaustive matches instead of closures or function pointers")]
fn read_optional_address(source: Option<wire::PersonAddress>) -> Option<domain::PersonAddress> {
    match source {
        Some(value) => Some(value::read_person_address(value)),
        None => None,
    }
}
fn optional_address_change(
    source: Option<&domain::AddressChange>,
    limits: &wire::Limits,
) -> Result<Option<wire::AddressChange>, wire::Problem> {
    match source {
        Some(value) => Ok(Some(address_change(value, limits)?)),
        None => Ok(None),
    }
}
fn read_optional_address_change(
    source: Option<wire::AddressChange>,
    limits: &wire::Limits,
) -> Result<Option<domain::AddressChange>, wire::Problem> {
    match source {
        Some(value) => Ok(Some(read_address_change(value, limits)?)),
        None => Ok(None),
    }
}

fn optional_spec(source: Option<&domain::Spec>, limits: &wire::Limits) -> Result<Option<wire::Spec>, wire::Problem> {
    match source {
        Some(value) => Ok(Some(value::spec(value, limits)?)),
        None => Ok(None),
    }
}
fn read_optional_spec(
    source: Option<wire::Spec>,
    limits: &wire::Limits,
) -> Result<Option<domain::Spec>, wire::Problem> {
    match source {
        Some(value) => Ok(Some(value::read_spec(value, limits)?)),
        None => Ok(None),
    }
}

fn optional_wake(
    source: Option<&domain::WakePolicy>,
    limits: &wire::Limits,
) -> Result<Option<wire::WakePolicy>, wire::Problem> {
    match source {
        Some(value) => Ok(Some(phase::wake(value, limits)?)),
        None => Ok(None),
    }
}
fn read_optional_wake(
    source: Option<wire::WakePolicy>,
    limits: &wire::Limits,
) -> Result<Option<domain::WakePolicy>, wire::Problem> {
    match source {
        Some(value) => Ok(Some(phase::read_wake(value, limits)?)),
        None => Ok(None),
    }
}

fn optional_authority(
    source: Option<&domain::Authority>,
    limits: &wire::Limits,
    moved: Option<&[u64; 64]>,
) -> Result<Option<wire::Authority>, wire::Problem> {
    match source {
        Some(value) => Ok(Some(value::authority(value, limits, moved)?)),
        None => Ok(None),
    }
}
fn read_optional_authority(
    source: Option<wire::Authority>,
    limits: &wire::Limits,
) -> Result<Option<domain::Authority>, wire::Problem> {
    match source {
        Some(value) => Ok(Some(value::read_authority(value, limits)?)),
        None => Ok(None),
    }
}

fn optional_proposal(
    source: Option<&domain::Proposal>,
    limits: &wire::Limits,
    moved: Option<&[u64; 64]>,
) -> Result<Option<wire::Proposal>, wire::Problem> {
    match source {
        Some(value) => Ok(Some(proposal(value, limits, moved)?)),
        None => Ok(None),
    }
}
fn read_optional_proposal(
    source: Option<wire::Proposal>,
    limits: &wire::Limits,
) -> Result<Option<Box<domain::Proposal>>, wire::Problem> {
    match source {
        Some(value) => Ok(Some(Box::new(read_proposal(value, limits)?))),
        None => Ok(None),
    }
}

fn optional_amendment_history(
    source: Option<&domain::AmendmentHistory>,
    limits: &wire::Limits,
    moved: Option<&[u64; 64]>,
) -> Result<Option<wire::AmendmentHistory>, wire::Problem> {
    match source {
        Some(value) => Ok(Some(amendment_history(value, limits, moved)?)),
        None => Ok(None),
    }
}
fn read_optional_amendment_history(
    source: Option<wire::AmendmentHistory>,
    limits: &wire::Limits,
) -> Result<Option<Box<domain::AmendmentHistory>>, wire::Problem> {
    match source {
        Some(value) => Ok(Some(Box::new(read_amendment_history(value, limits)?))),
        None => Ok(None),
    }
}

fn optional_tracking(
    source: Option<domain::Tracking>,
    limits: &wire::Limits,
) -> Result<Option<wire::Tracking>, wire::Problem> {
    Ok(match source {
        Some(domain::Tracking::Tracked { priority }) => {
            Some(wire::Tracking::Tracked(wire::Tracked::new(limits, wire::TrackedParts { priority })?))
        }
        Some(domain::Tracking::Untracked) => Some(wire::Tracking::Untracked),
        None => None,
    })
}
fn read_tracking(source: Option<wire::Tracking>) -> Option<domain::Tracking> {
    match source {
        Some(wire::Tracking::Tracked(value)) => {
            Some(domain::Tracking::Tracked { priority: value.into_parts().priority })
        }
        Some(wire::Tracking::Untracked) => Some(domain::Tracking::Untracked),
        None => None,
    }
}

fn setting_head(item: &domain::SettingHead, limits: &wire::Limits) -> Result<wire::SettingHead, wire::Problem> {
    let setting = match item.setting {
        domain::Setting::Charter(number) => {
            wire::Setting::Charter(wire::SettingNumber::new(limits, wire::SettingNumberParts { number })?)
        }
        domain::Setting::Template(number) => {
            wire::Setting::Template(wire::SettingNumber::new(limits, wire::SettingNumberParts { number })?)
        }
        domain::Setting::Policy(number) => {
            wire::Setting::Policy(wire::SettingNumber::new(limits, wire::SettingNumberParts { number })?)
        }
        domain::Setting::Rules => wire::Setting::Rules,
    };
    wire::SettingHead::new(
        limits,
        wire::SettingHeadParts {
            setting,
            edit: match item.edit {
                domain::SettingEdit::Set => wire::SettingEdit::Set,
                domain::SettingEdit::Removed => wire::SettingEdit::Removed,
                domain::SettingEdit::Reset => wire::SettingEdit::Reset,
            },
        },
    )
}
fn read_setting_head(item: wire::SettingHead) -> domain::SettingHead {
    let item = item.into_parts();
    domain::SettingHead {
        setting: match item.setting {
            wire::Setting::Charter(n) => domain::Setting::Charter(n.number()),
            wire::Setting::Template(n) => domain::Setting::Template(n.number()),
            wire::Setting::Policy(n) => domain::Setting::Policy(n.number()),
            wire::Setting::Rules => domain::Setting::Rules,
        },
        edit: match item.edit {
            wire::SettingEdit::Set => domain::SettingEdit::Set,
            wire::SettingEdit::Removed => domain::SettingEdit::Removed,
            wire::SettingEdit::Reset => domain::SettingEdit::Reset,
        },
    }
}
