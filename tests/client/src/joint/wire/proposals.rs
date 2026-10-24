//! Whole pending proposal facts (client/domain/contract.md, 3.2 and 5).
//! Source identity, current holder and optional funding observations remain
//! independent of immutable history and application part scope.
use super::{chips, history, offers, owner, results};
use jig_client::DomainLimits as Limits;
use jig_client_domain::contract as client;
use jig_domain_core::documents;

#[must_use]
pub fn key(proposer: documents::Proposer, number: u64) -> client::ProposalKey {
    client::ProposalKey {
        proposer: match proposer {
            documents::Proposer::Task(value) => client::Proposer::Task(value),
            documents::Proposer::Party(value) => client::Proposer::Party(value),
        },
        number,
    }
}

/// # Errors
/// Refuses the whole nested allocation above receiving bounds without inferring
/// missing hold, funding, result, actor or scoped application identities.
pub fn action(
    value: &documents::ProposalAction,
    limits: &Limits,
    models: &[jig_domain_core::Model],
) -> Result<client::OwnerProposalAction, owner::Invalid> {
    Ok(match value {
        documents::ProposalAction::Settings { .. } => return Err(owner::Invalid::Settings),
        documents::ProposalAction::Batch(values) => {
            client::OwnerProposalAction::Batch(owner::list(values, limits, |value| {
                history::proposed(value, limits, models)
            })?)
        }
        documents::ProposalAction::Effect { part, authority, attempt, completion, position, purpose } => {
            client::OwnerProposalAction::Effect {
                part: owner::part(part, limits)?,
                authority: owner::authority(authority, limits, models)?,
                attempt: *attempt,
                completion: *completion,
                position: *position,
                purpose: *purpose,
            }
        }
        documents::ProposalAction::ProcedureEffect { part, authority, step, purpose } => {
            client::OwnerProposalAction::ProcedureEffect {
                part: owner::part(part, limits)?,
                authority: owner::authority(authority, limits, models)?,
                step: *step,
                purpose: *purpose,
            }
        }
        documents::ProposalAction::Amend { task, addressed, before, after, reason, release } => {
            client::OwnerProposalAction::Amend {
                task: results::link(task, limits)?,
                addressed: addressed.map(|value| client::AddressChange {
                    revision: client::Revision { value: value.revision },
                    address: chips::address(value.address),
                }),
                before: Box::new(owner::amendment(before, limits, models)?),
                after: Box::new(owner::amendment(after, limits, models)?),
                reason: owner::text(reason, limits)?,
                release: *release,
            }
        }
        documents::ProposalAction::Widen { task, before, after } => client::OwnerProposalAction::Widen {
            task: results::link(task, limits)?,
            before: owner::authority(before, limits, models)?,
            after: owner::authority(after, limits, models)?,
        },
        documents::ProposalAction::Release { task, hold } => client::OwnerProposalAction::Release {
            task: results::link(task, limits)?,
            hold: hold.as_ref().map(|value| chips::held(value, limits).map(Box::new)).transpose()?,
        },
    })
}

/// # Errors
/// Rejects any complete source allocation above receiving bounds. The pending
/// envelope supplies no terminal state, earlier holder trail or display price.
pub fn document(
    value: &documents::Proposal,
    limits: &Limits,
    models: &[jig_domain_core::Model],
) -> Result<client::OwnerProposal, owner::Invalid> {
    Ok(client::OwnerProposal {
        key: key(value.proposer, value.number),
        project: value.project,
        reason: owner::text(&value.reason, limits)?,
        action: action(&value.action, limits, models)?,
        as_holder: value.as_holder,
        waiting: client::OwnerProposalWaiting {
            holder: history::holder(value.waiting.holder),
            since: value.waiting.since,
        },
        funding: owner::list(&value.funding, limits, |value| {
            Ok(client::OwnerFundingOffer {
                source: owner::funder(value.source),
                needed: value.needed,
                left: value.left,
                after: value.after,
            })
        })?,
        acts: offers::offered(&value.acts, limits, models)?,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    const UTF8_TEXTS: [&[u8]; 1] = [b"~\0^"];

    use super::*;
    use jig_domain_tasks as tasks;
    use skein_lib::{List, Wall};

    pub(crate) fn source() -> documents::Proposal {
        let mut funding = List::with_capacity(4);
        for (source, left, after) in [
            (tasks::Funder::Task(u64::MAX), None, None),
            (tasks::Funder::Pool { project: 0, person: u64::MAX, period: 0 }, Some(0), None),
            (tasks::Funder::Period { project: u32::MAX, period: u64::MAX }, None, Some(0)),
            (tasks::Funder::Recurring { project: u32::MAX, task: u64::MAX, period: 0 }, Some(u64::MAX), Some(u64::MAX)),
        ] {
            funding.push(documents::FundingOffer { source, needed: u64::MAX, left, after }).expect("supplied funding");
        }
        documents::Proposal {
            acts: List::with_capacity(2),
            proposer: documents::Proposer::Task(u64::MAX),
            number: u64::MAX,
            project: u32::MAX,
            reason: documents::Text { bytes: Box::from(UTF8_TEXTS[0]), cut: true },
            action: documents::ProposalAction::Release {
                task: documents::TaskLink::Stored { number: 0, status: None },
                hold: None,
            },
            as_holder: true,
            waiting: documents::ProposalWaiting {
                holder: tasks::ProposalHolder::Policy { project: 0, kind: tasks::ProposalKind::Amend },
                since: Wall::EPOCH,
            },
            funding,
        }
    }

    #[test]
    fn pending_proposal_keeps_maker_kind_exact_holder_and_independent_optional_funding_facts() {
        let mut source = source();
        let limits = super::super::rows::tests::limits();
        for (maker, expected) in [
            (documents::Proposer::Task(u64::MAX), client::Proposer::Task(u64::MAX)),
            (documents::Proposer::Party(u64::MAX), client::Proposer::Party(u64::MAX)),
        ] {
            source.proposer = maker;
            let before = source.clone();
            let actual =
                document(&source, &limits, &[jig_core_world::world::scripted_model()]).expect("whole pending source");
            assert_eq!(actual.key, client::ProposalKey { proposer: expected, number: u64::MAX });
            assert_eq!(actual.project, u32::MAX);
            assert_eq!(actual.reason.bytes.bytes(), UTF8_TEXTS[0]);
            assert!(actual.reason.cut);
            assert!(actual.as_holder);
            assert_eq!(actual.waiting.since, Wall::EPOCH);
            assert_eq!(
                actual.waiting.holder,
                client::OwnerHistoryHolder::Policy { project: 0, kind: client::OwnerMessageProposalKind::Amend }
            );
            assert_eq!(actual.funding.capacity(), 4);
            assert_eq!(
                actual.funding.as_slice(),
                &[
                    client::OwnerFundingOffer {
                        source: client::Funder::Task(u64::MAX),
                        needed: u64::MAX,
                        left: None,
                        after: None
                    },
                    client::OwnerFundingOffer {
                        source: client::Funder::Pool { project: 0, person: u64::MAX, period: 0 },
                        needed: u64::MAX,
                        left: Some(0),
                        after: None
                    },
                    client::OwnerFundingOffer {
                        source: client::Funder::Period { project: u32::MAX, period: u64::MAX },
                        needed: u64::MAX,
                        left: None,
                        after: Some(0)
                    },
                    client::OwnerFundingOffer {
                        source: client::Funder::Recurring { project: u32::MAX, task: u64::MAX, period: 0 },
                        needed: u64::MAX,
                        left: Some(u64::MAX),
                        after: Some(u64::MAX)
                    },
                ]
            );
            assert_eq!(source, before);
        }
    }

    #[test]
    fn all_five_pending_actions_preserve_whole_source_without_scope_or_missing_hold_inference() {
        let limits = super::super::rows::tests::limits();
        let state = owner::tests::amendment_source();
        let task = documents::TaskLink::Stored { number: u64::MAX, status: None };
        let mut actions = vec![
            documents::ProposalAction::Batch(List::with_capacity(4)),
            documents::ProposalAction::Effect {
                part: tasks::Part {
                    connector: u16::MAX,
                    kind: tasks::PartKind::Effect,
                    key: tasks::PartKey::Number(u64::MAX),
                },
                authority: state.authority.clone(),
                attempt: u64::MAX,
                completion: u32::MAX,
                position: 0,
                purpose: u64::MAX,
            },
            documents::ProposalAction::Amend {
                task: task.clone(),
                addressed: Some(tasks::AddressChange { revision: 0, address: tasks::PersonAddress::Role(u32::MAX) }),
                before: Box::new(state.clone()),
                after: Box::new(state.clone()),
                reason: documents::Text { bytes: Box::new([]), cut: false },
                release: Some(0),
            },
            documents::ProposalAction::Widen {
                task: task.clone(),
                before: state.authority.clone(),
                after: state.authority.clone(),
            },
            documents::ProposalAction::Release { task, hold: None },
        ];
        for (index, source) in actions.drain(..).enumerate() {
            let before = source.clone();
            let actual =
                action(&source, &limits, &[jig_core_world::world::scripted_model()]).expect("whole pending action");
            match (index, actual) {
                (0, client::OwnerProposalAction::Batch(values)) => {
                    assert_eq!(values.capacity(), 4);
                    assert!(values.is_empty());
                    assert_eq!(
                        action(
                            &source,
                            &Limits { rows: 3, turn_text: limits.turn_text, ..limits },
                            &[jig_core_world::world::scripted_model()]
                        )
                        .err(),
                        Some(owner::Invalid::TooLarge)
                    );
                }
                (
                    1,
                    client::OwnerProposalAction::Effect { part, authority, attempt, completion, position, purpose },
                ) => {
                    assert_eq!(
                        part,
                        client::OwnerPart {
                            connector: u16::MAX,
                            kind: client::OwnerPartKind::Effect,
                            key: client::OwnerPartKey::Number(u64::MAX)
                        }
                    );
                    assert_eq!((attempt, completion, position, purpose), (u64::MAX, u32::MAX, 0, u64::MAX));
                    assert_eq!(authority.authority.tools, u64::MAX);
                }
                (2, client::OwnerProposalAction::Amend { task, addressed, before, after, reason, release }) => {
                    assert_eq!(task, client::OwnerTaskLink::Stored { number: u64::MAX, status: None });
                    assert_eq!(
                        addressed,
                        Some(client::AddressChange {
                            revision: client::Revision { value: 0 },
                            address: client::PersonAddress::Role(u32::MAX)
                        })
                    );
                    assert_eq!(before, after);
                    assert_eq!(before.dependencies.capacity(), 4);
                    assert!(reason.bytes.bytes().is_empty());
                    assert!(!reason.cut);
                    assert_eq!(release, Some(0));
                }
                (3, client::OwnerProposalAction::Widen { task, before, after }) => {
                    assert_eq!(task, client::OwnerTaskLink::Stored { number: u64::MAX, status: None });
                    assert_eq!(before, after);
                }
                (4, client::OwnerProposalAction::Release { task, hold }) => {
                    assert_eq!(task, client::OwnerTaskLink::Stored { number: u64::MAX, status: None });
                    assert!(hold.is_none());
                }
                _ => panic!("all five exact source action variants"),
            }
            assert_eq!(source, before);
        }
    }

    #[test]
    fn whole_pending_proposal_read_prices_reserved_capacity_and_preserves_optional_present_hold() {
        let mut source = source();
        source.action = documents::ProposalAction::Release {
            task: documents::TaskLink::Stored { number: 0, status: None },
            hold: super::super::task::tests::source().hold,
        };
        let before = source.clone();
        let limits = super::super::rows::tests::limits();
        let native =
            document(&source, &limits, &[jig_core_world::world::scripted_model()]).expect("whole pending release");
        let client::OwnerProposalAction::Release { hold: Some(hold), .. } = &native.action else {
            panic!("supplied whole hold")
        };
        assert_eq!(hold.revision, 17);
        let bounds = jig_client_scripted_engine::Bounds {
            rows: limits.rows,
            authority: crate::test::AUTHORITY_ENTRIES,
            segments: limits.segments,
            text: limits.text,
            document_bytes: limits.document_bytes,
            ..jig_client_scripted_engine::Bounds::default()
        };
        let bytes = jig_client_scripted_engine::owner_proposal::bytes(&native, &bounds)
            .expect("independent whole payload pricing")
            .checked_add(u64::try_from(size_of::<client::ReadValue>()).expect("read envelope"))
            .expect("whole read bytes");
        let exact = Limits { document_bytes: bytes, turn_text: limits.turn_text, ..limits };
        assert!(
            super::super::documents::proposal_read(&source, &exact, &[jig_core_world::world::scripted_model()]).is_ok()
        );
        assert_eq!(
            super::super::documents::proposal_read(
                &source,
                &Limits { document_bytes: bytes - 1, turn_text: exact.turn_text, ..exact },
                &[jig_core_world::world::scripted_model()]
            )
            .err(),
            Some(super::super::documents::Invalid::Payload)
        );
        assert_eq!(
            document(
                &source,
                &Limits { rows: 3, turn_text: limits.turn_text, ..limits },
                &[jig_core_world::world::scripted_model()]
            )
            .err(),
            Some(owner::Invalid::TooLarge)
        );
        assert_eq!(source, before);
    }
}
