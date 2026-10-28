#![expect(clippy::manual_map, reason = "step code matches optional owner facts without closures")]
//! Whole single-read owner documents (protocol/client.md, 3 and 6).
use super::super::{owner, w};
use alloc::boxed::Box;
use jig_domain_tasks as t;
use skein_lib::{List, Writer, bytes};
pub fn pattern(value: &t::Pattern, limits: &w::Limits) -> Result<w::OwnerPattern, w::Problem> {
    let mut segments = List::with_capacity(super::super::count(value.segments.len())?);
    for row in &value.segments {
        segments.push(bytes::copy_of(row)).expect("supplied rows fit");
    }

    w::OwnerPattern::new(
        limits,
        w::OwnerPatternParts {
            segments,
            last: match &value.last {
                t::Last::Exact(value) => w::OwnerPatternEnd::Exact(w::OwnerPatternEndExact::new(
                    limits,
                    w::OwnerPatternEndExactParts { value: bytes::copy_of(value) },
                )?),
                t::Last::Open(value) => w::OwnerPatternEnd::Open(w::OwnerPatternEndOpen::new(
                    limits,
                    w::OwnerPatternEndOpenParts { value: bytes::copy_of(value) },
                )?),
            },
        },
    )
}
pub fn authority(
    models: &[crate::Model],
    value: &t::Authority,
    limits: &w::Limits,
) -> Result<w::OwnerAuthority, w::Problem> {
    let mut grants = List::with_capacity(super::super::count(value.grants.len())?);
    for row in &value.grants {
        grants
            .push(w::OwnerGrant::new(
                limits,
                w::OwnerGrantParts {
                    connector: row.connector,
                    kind: row.kind,
                    pattern: pattern(&row.pattern, limits)?,
                },
            )?)
            .expect("supplied rows fit");
    }
    let grants = owner::grants_pieces(&grants, limits.owner_authority_grants, w::Path::OwnerAuthorityGrants)?;
    let mut kinds = List::with_capacity(super::super::count(value.delegation.kinds.len())?);
    for row in &value.delegation.kinds {
        kinds.push(owner::authority_executor(*row, limits)?).expect("supplied rows fit");
    }
    let mut scopes = List::with_capacity(super::super::count(value.note_resources.len())?);
    for row in &value.note_resources {
        scopes
            .push(w::OwnerResourceScope::new(
                limits,
                w::OwnerResourceScopeParts { connector: row.connector, pattern: pattern(&row.pattern, limits)? },
            )?)
            .expect("supplied rows fit");
    }
    let scopes =
        owner::scopes_pieces(&scopes, limits.owner_authority_note_resources, w::Path::OwnerAuthorityNoteResources)?;

    w::OwnerAuthority::new(
        limits,
        w::OwnerAuthorityParts {
            models: super::super::llm::authority(
                jig_domain_authority::Models::from_parts(value.models.bits(), value.models.is_all()),
                models,
                limits,
            )?,
            tools: value.tools.0,
            grants,
            delegation: w::OwnerDelegation::new(
                limits,
                w::OwnerDelegationParts { kinds, tasks: value.delegation.tasks, depth: value.delegation.depth },
            )?,
            spend: value.budget.spend,
            deadline: match value.budget.deadline {
                Some(item) => Some(item.as_nanos()),
                None => None,
            },
            notes: value.notes.0,
            note_resources: scopes,
        },
    )
}
pub fn spec(value: &t::Spec, limits: &w::Limits) -> Result<w::Specification, w::Problem> {
    let mut parameters = List::with_capacity(super::super::count(value.parameters.len())?);
    for row in &value.parameters {
        parameters
            .push(match row {
                t::Parameter::Number { name, value } => w::Parameter::Number(w::ParameterNumber::new(
                    limits,
                    w::ParameterNumberParts { name: *name, value: *value },
                )?),
                t::Parameter::Bytes { name, value } => w::Parameter::Bytes(w::ParameterBytes::new(
                    limits,
                    w::ParameterBytesParts { name: *name, value: bytes::copy_of(value) },
                )?),
                t::Parameter::Resource { name, connector, resource } => {
                    w::Parameter::Resource(w::ParameterResource::new(
                        limits,
                        w::ParameterResourceParts { name: *name, connector: *connector, resource: *resource },
                    )?)
                }
            })
            .expect("supplied parameters fit");
    }
    let mut inputs = List::with_capacity(super::super::count(value.inputs.len())?);
    for row in &value.inputs {
        inputs.push(*row).expect("supplied rows fit");
    }

    w::Specification::new(
        limits,
        w::SpecificationParts { asks: value.asks, words: bytes::copy_of(&value.words), parameters, inputs },
    )
}
pub fn contract(value: &t::Contract, limits: &w::Limits) -> Result<w::TaskContractDocument, w::Problem> {
    Ok(match value {
        t::Contract::Approval { state, part } => {
            w::TaskContractDocument::Approval(w::TaskContractDocumentApproval::new(
                limits,
                w::TaskContractDocumentApprovalParts { state: bytes::copy_of(state), part: owner::part(part, limits)? },
            )?)
        }
        t::Contract::Report { words } => w::TaskContractDocument::Report(w::TaskContractDocumentReport::new(
            limits,
            w::TaskContractDocumentReportParts { words: *words },
        )?),
        t::Contract::Change { connector, kind, words } => {
            w::TaskContractDocument::Change(w::TaskContractDocumentChange::new(
                limits,
                w::TaskContractDocumentChangeParts { connector: *connector, kind: *kind, words: *words },
            )?)
        }
        t::Contract::Verdict { choices } => {
            let mut copied = List::with_capacity(super::super::count(choices.len())?);
            for row in choices {
                let mut fields = List::with_capacity(super::super::count(row.fields.len())?);
                for field in &row.fields {
                    fields
                        .push(w::FieldRule::new(
                            limits,
                            w::FieldRuleParts { name: bytes::copy_of(&field.name), max: field.max },
                        )?)
                        .expect("supplied fields fit");
                }
                copied
                    .push(w::VerdictChoice::new(
                        limits,
                        w::VerdictChoiceParts { code: row.code, words: row.words, followups: row.followups, fields },
                    )?)
                    .expect("supplied choices fit");
            }
            w::TaskContractDocument::Verdict(w::TaskContractDocumentVerdict::new(
                limits,
                w::TaskContractDocumentVerdictParts {
                    choices: owner::choices_pieces(
                        &copied,
                        limits.task_contract_document_verdict_choices,
                        w::Path::TaskContractDocumentVerdictChoices,
                    )?,
                    cut: false,
                },
            )?)
        }
    })
}
pub fn goal(
    models: &[crate::Model],
    value: &t::New,
    limits: &w::Limits,
) -> Result<w::OwnerPersonProposalGoal, w::Problem> {
    if value.recurring.is_some() {
        return Err(w::Problem { path: w::Path::OwnerPersonProposalGoal, reason: skein_codec::Reason::Tag });
    }
    let mut dependencies = List::with_capacity(super::super::count(value.dependencies.len())?);
    for row in &value.dependencies {
        dependencies.push(*row).expect("supplied rows fit");
    }
    let mut holdings = List::with_capacity(super::super::count(value.holdings.len())?);
    for row in &value.holdings {
        let (name, kind) = match row {
            t::Holding::Write { resource, kind } => (resource, *kind),
            t::Holding::Slot { pool, kind } => (pool, *kind),
        };
        let mut path = List::with_capacity(super::super::count(name.path.len())?);
        for segment in &name.path {
            path.push(bytes::copy_of(segment)).expect("supplied path fits");
        }
        holdings
            .push(match row {
                t::Holding::Write { .. } => w::OwnerGoalHolding::Write(w::OwnerGoalHoldingWrite::new(
                    limits,
                    w::OwnerGoalHoldingWriteParts { connector: name.connector, path, kind },
                )?),
                t::Holding::Slot { .. } => w::OwnerGoalHolding::Slot(w::OwnerGoalHoldingSlot::new(
                    limits,
                    w::OwnerGoalHoldingSlotParts { connector: name.connector, path, kind },
                )?),
            })
            .expect("supplied holdings fit");
    }
    let holdings = owner_goal_holding_pieces(
        &holdings,
        limits.owner_person_proposal_goal_holdings,
        w::Path::OwnerPersonProposalGoalHoldings,
    )?;

    w::OwnerPersonProposalGoal::new(
        limits,
        w::OwnerPersonProposalGoalParts {
            number: value.number,
            project: value.project,
            executor: owner::executor(value.executor, limits)?,
            spec: spec(&value.spec, limits)?,
            contract: contract(&value.contract, limits)?,
            authority: authority(models, &value.authority, limits)?,
            numbers: owner::numbers(value.numbers, limits)?,
            funder: owner::funder(value.funder, limits)?,
            dependencies,
            holdings,
            wake: owner::wake(value.wake, limits)?,
            tracked: value.tracked,
        },
    )
}
pub(super) fn owner_goal_holding_pieces(
    rows: &List<w::OwnerGoalHolding>,
    bound: u32,
    path: w::Path,
) -> Result<Box<[u8]>, w::Problem> {
    let mut encoded = List::with_capacity(rows.len());
    for row in rows {
        let mut out = Writer::new(usize::try_from(row.measure()).expect("u32 measure fits usize"));
        match row.encode(&mut out) {
            Ok(()) => (),
            Err(_) => return Err(w::Problem { path, reason: skein_codec::Reason::Bound }),
        }
        encoded.push(out.finish()).expect("supplied rows fit");
    }
    super::super::pieces(&encoded, bound, path)
}
