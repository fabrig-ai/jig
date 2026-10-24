//! Immutable semantic changes and lifecycle milestones (objects.md, 11).
//! Source positions, revisions, stored overrides and optional clocks are copied
//! independently; no present-day state supplies historical missing facts.
use super::{chips, owner, results};
use jig_client::DomainLimits as Limits;
use jig_client_domain::contract as client;
use jig_domain_core::documents;
use jig_domain_tasks as tasks;

/// # Errors
/// Refuses any complete nested text/list allocation above receiving bounds.
pub fn proposed(
    value: &documents::ProposedTask,
    limits: &Limits,
    models: &[jig_domain_core::Model],
) -> Result<client::ProposedTask, owner::Invalid> {
    Ok(client::ProposedTask {
        deadline: match value.deadline {
            tasks::Deadline::Named => client::NewDeadline::Named,
            tasks::Deadline::Span(span) => client::NewDeadline::Span(span),
        },
        number: value.number,
        spec: owner::specification(&value.spec, limits)?,
        executor: owner::executor(value.executor),
        contract: owner::contract(&value.contract, limits)?,
        authority: owner::authority(&value.authority, limits, models)?,
        budget: value.budget,
        funder: owner::funder(value.funder),
        dependencies: owner::list(&value.dependencies, limits, |value| Ok(*value))?,
        resources: owner::list(&value.resources, limits, |value| owner::part(value, limits))?,
        wake: owner::wake(value.wake),
        priority: value.priority,
        cut: value.cut,
    })
}

fn amendment(
    value: &documents::AmendmentChange,
    limits: &Limits,
    models: &[jig_domain_core::Model],
) -> Result<client::OwnerRetainedAmendment, owner::Invalid> {
    Ok(client::OwnerRetainedAmendment {
        addressed: value.addressed.map(|value| client::AddressChange {
            revision: client::Revision { value: value.revision },
            address: chips::address(value.address),
        }),
        instructions: value.instructions.as_ref().map(|value| owner::text(value, limits)).transpose()?,
        release: value.release,
        spec: value.spec.as_ref().map(|value| owner::specification(value, limits)).transpose()?,
        authority: value.authority.as_ref().map(|value| owner::authority(value, limits, models)).transpose()?,
        wake: value.wake.map(owner::wake),
        dependencies: value
            .dependencies
            .as_ref()
            .map(|value| owner::list(value, limits, |value| Ok(*value)))
            .transpose()?,
        reason: owner::text(&value.reason, limits)?,
        cut: value.cut,
    })
}

/// # Errors
/// Refuses any complete retained action above receiving bounds; raw parts keep
/// their supplied kind and key without an inferred application scope.
pub fn action(
    value: &documents::RetainedAction,
    limits: &Limits,
    models: &[jig_domain_core::Model],
) -> Result<client::OwnerRetainedAction, owner::Invalid> {
    Ok(match value {
        documents::RetainedAction::Settings { .. } => return Err(owner::Invalid::Settings),
        documents::RetainedAction::Batch(value) => {
            client::OwnerRetainedAction::Batch(owner::list(value, limits, |value| proposed(value, limits, models))?)
        }
        documents::RetainedAction::Effect { part, authority, attempt, completion, position, purpose } => {
            client::OwnerRetainedAction::Effect {
                part: owner::part(part, limits)?,
                authority: owner::authority(authority, limits, models)?,
                attempt: *attempt,
                completion: *completion,
                position: *position,
                purpose: *purpose,
            }
        }
        documents::RetainedAction::ProcedureEffect { part, authority, step, purpose } => {
            client::OwnerRetainedAction::ProcedureEffect {
                part: owner::part(part, limits)?,
                authority: owner::authority(authority, limits, models)?,
                step: *step,
                purpose: *purpose,
            }
        }
        documents::RetainedAction::Amend { task, change } => {
            client::OwnerRetainedAction::Amend { task: *task, change: Box::new(amendment(change, limits, models)?) }
        }
        documents::RetainedAction::Widen { task, authority } => {
            client::OwnerRetainedAction::Widen { task: *task, authority: owner::authority(authority, limits, models)? }
        }
        documents::RetainedAction::Release { task } => client::OwnerRetainedAction::Release { task: *task },
    })
}

fn kind(value: tasks::ProposalKind) -> client::OwnerMessageProposalKind {
    match value {
        tasks::ProposalKind::Batch => client::OwnerMessageProposalKind::Batch,
        tasks::ProposalKind::Effect => client::OwnerMessageProposalKind::Effect,
        tasks::ProposalKind::Amend => client::OwnerMessageProposalKind::Amend,
        tasks::ProposalKind::Widen => client::OwnerMessageProposalKind::Widen,
        tasks::ProposalKind::Release => client::OwnerMessageProposalKind::Release,
        tasks::ProposalKind::Settings => client::OwnerMessageProposalKind::Settings,
    }
}

pub(super) fn holder(value: tasks::ProposalHolder) -> client::OwnerHistoryHolder {
    match value {
        tasks::ProposalHolder::Task(value) => client::OwnerHistoryHolder::Task(value),
        tasks::ProposalHolder::Person(value) => client::OwnerHistoryHolder::Person(value),
        tasks::ProposalHolder::Policy { project, kind: value } => {
            client::OwnerHistoryHolder::Policy { project, kind: kind(value) }
        }
    }
}

pub(super) fn proposal(
    value: &documents::ProposalHistory,
    limits: &Limits,
    models: &[jig_domain_core::Model],
) -> Result<client::OwnerProposalHistory, owner::Invalid> {
    Ok(client::OwnerProposalHistory {
        action: action(&value.action, limits, models)?,
        number: value.number,
        proposer: value.proposer,
        as_holder: value.as_holder,
        state: match &value.state {
            documents::ProposalHistoryState::Pending(value) => {
                client::OwnerProposalHistoryState::Pending { holder: holder(value.holder), since: value.since }
            }
            documents::ProposalHistoryState::Accepted { by } => {
                client::OwnerProposalHistoryState::Accepted { by: results::requester(*by) }
            }
            documents::ProposalHistoryState::Rejected { by, reason } => client::OwnerProposalHistoryState::Rejected {
                by: results::requester(*by),
                reason: owner::text(reason, limits)?,
            },
            documents::ProposalHistoryState::Withdrawn => client::OwnerProposalHistoryState::Withdrawn,
        },
    })
}

/// # Errors
/// Refuses any complete history snapshot/action above receiving bounds.
pub fn line(
    value: &documents::HistoryLine,
    limits: &Limits,
    models: &[jig_domain_core::Model],
) -> Result<client::OwnerHistoryLine, owner::Invalid> {
    Ok(client::OwnerHistoryLine {
        task: value.task,
        revision: value.revision,
        by: results::requester(value.by),
        change: match value.change {
            tasks::Change::Cancelled => client::OwnerHistoryChange::Cancelled,
            tasks::Change::Released => client::OwnerHistoryChange::Released,
            tasks::Change::Amended => client::OwnerHistoryChange::Amended,
            tasks::Change::Moved => client::OwnerHistoryChange::Moved,
            tasks::Change::Proposed => client::OwnerHistoryChange::Proposed,
            tasks::Change::ProposalPassed => client::OwnerHistoryChange::ProposalPassed,
            tasks::Change::ProposalAccepted => client::OwnerHistoryChange::ProposalAccepted,
            tasks::Change::ProposalRejected => client::OwnerHistoryChange::ProposalRejected,
            tasks::Change::ProposalWithdrawn => client::OwnerHistoryChange::ProposalWithdrawn,
        },
        reason: owner::text(&value.reason, limits)?,
        proposal: value.proposal.as_ref().map(|value| proposal(value, limits, models)).transpose()?,
        amendment: value
            .amendment
            .as_ref()
            .map(|(before, after)| {
                Ok((
                    Box::new(owner::amendment(before, limits, models)?),
                    Box::new(owner::amendment(after, limits, models)?),
                ))
            })
            .transpose()?,
        at: value.at,
    })
}

/// # Errors
/// Refuses any complete milestone ending or raw part above receiving bounds.
pub fn milestone(
    value: &documents::MilestoneLine,
    limits: &Limits,
) -> Result<client::OwnerMilestoneLine, owner::Invalid> {
    Ok(client::OwnerMilestoneLine {
        task: value.task,
        position: value.position,
        at: value.at,
        phase: match &value.phase {
            documents::MilestonePhase::Waiting => client::OwnerMilestonePhase::Waiting,
            documents::MilestonePhase::Active(value) => client::OwnerMilestonePhase::Active(chips::active(*value)),
            documents::MilestonePhase::Closing { stage, ending } => client::OwnerMilestonePhase::Closing {
                stage: chips::stage(*stage),
                ending: results::ending(ending, limits)?,
            },
            documents::MilestonePhase::Held { before, why, source, ending } => client::OwnerMilestonePhase::Held {
                before: match before {
                    documents::PriorPhase::Waiting => client::OwnerPriorPhase::Waiting,
                    documents::PriorPhase::Active(value) => client::OwnerPriorPhase::Active(chips::active(*value)),
                    documents::PriorPhase::Closing(value) => client::OwnerPriorPhase::Closing(chips::stage(*value)),
                },
                why: chips::hold(*why),
                source: source.as_ref().map(|value| owner::part(value, limits)).transpose()?,
                ending: ending.as_ref().map(|value| results::ending(value, limits)).transpose()?,
            },
            documents::MilestonePhase::Ended(value) => {
                client::OwnerMilestonePhase::Ended(results::ending(value, limits)?)
            }
        },
    })
}

#[cfg(test)]
pub(super) mod tests {
    const UTF8_TEXTS: [&[u8]; 1] = [b"~\0"];

    use super::*;
    use skein_lib::{List, Wall};
    pub(crate) fn limits() -> Limits {
        Limits { rows: 8, text: 200, turn_text: 200, document_bytes: 100_000, ..crate::test::limits().client.domain }
    }
    pub(crate) fn source() -> documents::HistoryLine {
        let before = owner::tests::amendment_source();
        let mut after = before.clone();
        after.instructions = Some(documents::Text { bytes: Box::new([]), cut: false });
        after.cut = true;
        after.spec.words.cut = true;
        documents::HistoryLine {
            task: u64::MAX,
            revision: 0,
            by: tasks::Party::Deployment { project: u32::MAX },
            change: tasks::Change::Amended,
            reason: documents::Text { bytes: Box::from(UTF8_TEXTS[0]), cut: true },
            proposal: Some(documents::ProposalHistory {
                action: documents::RetainedAction::Release { task: u64::MAX },
                number: u64::MAX,
                proposer: 0,
                as_holder: true,
                state: documents::ProposalHistoryState::Pending(documents::ProposalWaiting {
                    holder: tasks::ProposalHolder::Policy { project: u32::MAX, kind: tasks::ProposalKind::Effect },
                    since: Wall::EPOCH,
                }),
            }),
            amendment: Some((Box::new(before), Box::new(after))),
            at: None,
        }
    }
    #[test]
    fn immutable_history_keeps_raw_proposer_stored_override_absence_and_independent_revisions_clocks_and_losses() {
        let mut source = source();
        let copied = line(&source, &limits(), &[jig_core_world::world::scripted_model()]).expect("whole history");
        assert_eq!((copied.task, copied.revision, copied.at), (u64::MAX, 0, None));
        assert_eq!(copied.by, client::OwnerRequester::Deployment { project: u32::MAX });
        assert_eq!(copied.reason.bytes.bytes(), UTF8_TEXTS[0]);
        assert!(copied.reason.cut);
        assert_eq!(copied.change, client::OwnerHistoryChange::Amended);
        let proposal = copied.proposal.expect("retained proposal");
        assert_eq!((proposal.number, proposal.proposer, proposal.as_holder), (u64::MAX, 0, true));
        assert_eq!(
            proposal.state,
            client::OwnerProposalHistoryState::Pending {
                holder: client::OwnerHistoryHolder::Policy {
                    project: u32::MAX,
                    kind: client::OwnerMessageProposalKind::Effect
                },
                since: Wall::EPOCH,
            }
        );
        let (before, after) = copied.amendment.expect("stored snapshots");
        assert_eq!(before.instructions, None);
        assert_eq!(after.instructions.as_ref().expect("explicit stored clear").bytes.bytes(), &[]);
        assert!(!after.instructions.as_ref().expect("empty source").cut);
        assert!(after.cut && after.spec.specification.words.cut);
        assert!(!before.cut);
        assert_eq!(before.addressed, Some(client::PersonAddress::Role(u32::MAX)));
        assert_eq!((before.dependencies.capacity(), before.dependencies.len()), (4, 2));
        source.at = Some(Wall::EPOCH);
        source.amendment = None;
        source.proposal.as_mut().expect("proposal").state = documents::ProposalHistoryState::Rejected {
            by: tasks::Party::Person(u64::MAX),
            reason: documents::Text { bytes: Box::new([]), cut: true },
        };
        let copied =
            line(&source, &limits(), &[jig_core_world::world::scripted_model()]).expect("independent optional facts");
        assert_eq!(copied.at, Some(Wall::EPOCH));
        assert_eq!(copied.amendment, None);
        assert!(
            matches!(copied.proposal.expect("proposal").state, client::OwnerProposalHistoryState::Rejected { by: client::OwnerRequester::Person(u64::MAX), reason } if reason.cut && reason.bytes.bytes().is_empty())
        );
        assert_eq!(
            line(&source, &Limits { text: 1, turn_text: 1, ..limits() }, &[jig_core_world::world::scripted_model()]),
            Err(owner::Invalid::TooLarge)
        );
    }
    #[test]
    fn retained_batches_keep_allocated_nested_capacity_exact_executor_funding_and_independent_loss() {
        let state = owner::tests::amendment_source();
        let proposed = documents::ProposedTask {
            deadline: jig_domain_tasks::Deadline::Named,
            number: u64::MAX,
            executor: tasks::Executor::Person(tasks::PersonAddress::Role(u32::MAX)),
            spec: state.spec.clone(),
            contract: documents::ContractLine::Report { words: u32::MAX },
            authority: state.authority.clone(),
            budget: u64::MAX,
            funder: tasks::Funder::Recurring { project: u32::MAX, task: u64::MAX, period: 0 },
            dependencies: state.dependencies.clone(),
            resources: List::with_capacity(7),
            wake: state.wake,
            priority: Some(0),
            cut: true,
        };
        let mut batch = List::with_capacity(6);
        batch.push(proposed).expect("one source task");
        let client::OwnerRetainedAction::Batch(copied) = action(
            &documents::RetainedAction::Batch(batch.clone()),
            &limits(),
            &[jig_core_world::world::scripted_model()],
        )
        .expect("whole batch") else {
            panic!("batch")
        };
        assert_eq!((copied.capacity(), copied.len()), (6, 1));
        let task = copied.get(0).expect("task");
        assert_eq!((task.number, task.budget, task.priority, task.cut), (u64::MAX, u64::MAX, Some(0), true));
        assert_eq!(task.resources.capacity(), 7);
        assert_eq!(task.executor, client::OwnerExecutor::Person(client::PersonAddress::Role(u32::MAX)));
        assert_eq!(task.dependencies.capacity(), 4);
        assert_eq!(
            action(
                &documents::RetainedAction::Batch(batch),
                &Limits { rows: 5, turn_text: limits().turn_text, ..limits() },
                &[jig_core_world::world::scripted_model()]
            ),
            Err(owner::Invalid::TooLarge)
        );
    }
    #[test]
    fn retained_effects_keep_raw_parts_and_all_source_scalars_without_an_application_scope() {
        let state = owner::tests::amendment_source();
        let raw =
            tasks::Part { connector: u16::MAX, kind: tasks::PartKind::Outbox, key: tasks::PartKey::Number(u64::MAX) };
        let effect = documents::RetainedAction::Effect {
            part: raw,
            authority: state.authority.clone(),
            attempt: u64::MAX,
            completion: u32::MAX,
            position: 0,
            purpose: u64::MAX,
        };
        assert!(
            matches!(action(&effect, &limits(), &[jig_core_world::world::scripted_model()]).expect("whole raw effect"), client::OwnerRetainedAction::Effect { part, attempt: u64::MAX, completion: u32::MAX, position: 0, purpose: u64::MAX, .. } if part.connector == u16::MAX && part.kind == client::OwnerPartKind::Outbox && part.key == client::OwnerPartKey::Number(u64::MAX))
        );
    }
    #[test]
    fn retained_amendments_keep_selected_empty_replacements_independent_revisions_and_release_zero() {
        let state = owner::tests::amendment_source();
        let change = documents::AmendmentChange {
            addressed: Some(tasks::AddressChange { revision: 12, address: tasks::PersonAddress::Role(u32::MAX) }),
            instructions: Some(documents::Text { bytes: Box::new([]), cut: true }),
            release: Some(0),
            spec: Some(state.spec),
            authority: Some(state.authority),
            wake: Some(state.wake),
            dependencies: Some(List::with_capacity(7)),
            reason: documents::Text { bytes: Box::from([0]), cut: false },
            cut: true,
        };
        let client::OwnerRetainedAction::Amend { task, change } = action(
            &documents::RetainedAction::Amend { task: u64::MAX, change: Box::new(change) },
            &limits(),
            &[jig_core_world::world::scripted_model()],
        )
        .expect("whole selected delta") else {
            panic!("amend")
        };
        assert_eq!(task, u64::MAX);
        assert_eq!(change.addressed.expect("address").revision.value, 12);
        assert_eq!(change.release, Some(0));
        assert!(change.instructions.expect("stored selected empty").cut);
        assert_eq!(change.dependencies.expect("supplied empty dependencies").capacity(), 7);
        assert!(change.cut);
        assert!(matches!(
            action(
                &documents::RetainedAction::Widen { task: 0, authority: owner::tests::amendment_source().authority },
                &limits(),
                &[jig_core_world::world::scripted_model()]
            )
            .expect("widen"),
            client::OwnerRetainedAction::Widen { task: 0, .. }
        ));
        assert_eq!(
            action(
                &documents::RetainedAction::Release { task: 0 },
                &limits(),
                &[jig_core_world::world::scripted_model()]
            )
            .expect("release"),
            client::OwnerRetainedAction::Release { task: 0 }
        );
    }
}
