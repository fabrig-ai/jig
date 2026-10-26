#![expect(clippy::manual_map, reason = "step code matches optional owner facts without closures")]
//! Whole owner facts encoded without clipping (protocol/client.md, 3).
use super::{chips, owner, refusals};
use super::{pieces, problem, w};
use alloc::boxed::Box;
use jig_domain_core::acts;
use jig_domain_core::documents as d;
use jig_domain_tasks as t;
use skein_lib::List;
pub fn link(value: &d::TaskLink, limits: &w::Limits) -> Result<w::OwnerTaskLink, w::Problem> {
    Ok(match value {
        d::TaskLink::Live(value) => w::OwnerTaskLink::Live(w::OwnerTaskLinkLive::new(
            limits,
            w::OwnerTaskLinkLiveParts { value: chips::chip(value, limits)? },
        )?),
        d::TaskLink::Retained { chip, at } => w::OwnerTaskLink::Retained(w::OwnerTaskLinkRetained::new(
            limits,
            w::OwnerTaskLinkRetainedParts {
                chip: chips::chip(chip, limits)?,
                at: match *at {
                    Some(item) => Some(item.as_nanos()),
                    None => None,
                },
            },
        )?),
        d::TaskLink::Stored { number, status } => w::OwnerTaskLink::Stored(w::OwnerTaskLinkStored::new(
            limits,
            w::OwnerTaskLinkStoredParts {
                number: *number,
                status: match status {
                    Some(t::Status::Done) => Some(w::OwnerTaskStatus::Done),
                    Some(t::Status::Failed) => Some(w::OwnerTaskStatus::Failed),
                    Some(t::Status::Cancelled) => Some(w::OwnerTaskStatus::Cancelled),
                    None => None,
                },
            },
        )?),
    })
}
pub fn proposal(value: &d::ProposalRef, limits: &w::Limits) -> Result<w::ProposalKey, w::Problem> {
    w::ProposalKey::new(
        limits,
        w::ProposalKeyParts {
            proposer: match value.proposer {
                jig_domain_people::Proposer::Task(number) => {
                    w::Proposer::Task(w::ProposerTask::new(limits, w::ProposerTaskParts { value: number })?)
                }
                jig_domain_people::Proposer::Party(number) => {
                    w::Proposer::Party(w::ProposerParty::new(limits, w::ProposerPartyParts { value: number })?)
                }
            },
            number: value.number,
        },
    )
}
pub fn act(value: acts::Act, _limits: &w::Limits) -> Result<w::ActKind, w::Problem> {
    Ok(match value {
        acts::Act::Reorder => w::ActKind::Reorder,
        acts::Act::Message => w::ActKind::Message,
        acts::Act::Answer => w::ActKind::Answer,
        acts::Act::Take => w::ActKind::Take,
        acts::Act::HandBack => w::ActKind::HandBack,
        acts::Act::Accept => w::ActKind::Accept,
        acts::Act::Withdraw => w::ActKind::Withdraw,
        acts::Act::Reject => w::ActKind::Reject,
        acts::Act::PassUp => w::ActKind::PassUp,
        acts::Act::Release => w::ActKind::Release,
        acts::Act::LeaveHeld => w::ActKind::LeaveHeld,
        acts::Act::Amend => w::ActKind::Amend,
        acts::Act::Stop => w::ActKind::Stop,
        acts::Act::Cancel => w::ActKind::Cancel,
        acts::Act::Move => w::ActKind::MoveTask,
        acts::Act::Seen => w::ActKind::Seen,
        acts::Act::EditNote => w::ActKind::EditNote,
        acts::Act::DeleteNote => w::ActKind::DeleteNote,
        acts::Act::Adopt => w::ActKind::Adopt,
        acts::Act::Rename => w::ActKind::Rename,
        acts::Act::ChangePolicy => w::ActKind::ChangePolicy,
        acts::Act::SetRoles => w::ActKind::SetRoles,
        acts::Act::MakeService => w::ActKind::MakeService,
        acts::Act::SetPool => w::ActKind::SetPool,
    })
}
pub fn recipient(value: &acts::Recipient, limits: &w::Limits) -> Result<w::Recipient, w::Problem> {
    Ok(match value {
        acts::Recipient::Task(number) => {
            w::Recipient::Task(w::RecipientTask::new(limits, w::RecipientTaskParts { value: *number })?)
        }
        acts::Recipient::Party(number) => {
            w::Recipient::Party(w::RecipientParty::new(limits, w::RecipientPartyParts { value: *number })?)
        }
        acts::Recipient::Role { project, role } => {
            w::Recipient::Role(w::RecipientRole::new(limits, w::RecipientRoleParts { project: *project, role: *role })?)
        }
        acts::Recipient::Roles { project, roles } => w::Recipient::Roles(w::RecipientRoles::new(
            limits,
            w::RecipientRolesParts { project: *project, roles: roles.clone() },
        )?),
    })
}
pub(super) fn bounds(
    models: &[crate::Model],
    value: &acts::Bounds,
    limits: &w::Limits,
) -> Result<w::Bounds, w::Problem> {
    Ok(match value {
        acts::Bounds::Create {
            authority,
            pool_left,
            charters,
            procedures,
            latest,
            budget,
            maximum_budget,
            maximum_deadline,
        } => {
            let mut rows = List::with_capacity(procedures.capacity());
            for item in procedures {
                rows.push(w::CreateProcedure::new(
                    limits,
                    w::CreateProcedureParts { connector: item.connector, code: item.code },
                )?)
                .expect("supplied goal procedures");
            }
            w::Bounds::Create(w::BoundsCreate::new(
                limits,
                w::BoundsCreateParts {
                    authority: owner::authority(models, authority, limits)?,
                    pool_left: *pool_left,
                    charters: charters.clone(),
                    procedures: rows,
                    latest: match latest {
                        Some(item) => Some(item.as_nanos()),
                        None => None,
                    },
                    budget: *budget,
                    maximum_budget: *maximum_budget,
                    maximum_deadline: match maximum_deadline {
                        Some(item) => Some(item.as_nanos()),
                        None => None,
                    },
                },
            )?)
        }
        acts::Bounds::Amend { budget_left, grants, implications, authority, ceiling } => {
            let mut rows = List::with_capacity(grants.capacity());
            for item in grants {
                rows.push(owner::grant(item, limits)?).expect("supplied grants");
            }
            let grants = owner::grants_pieces(
                &rows,
                limits.bounds_amend_grants.min(w::CEILINGS.bounds_amend_grants),
                w::Path::BoundsAmendGrants,
            )?;
            let mut pairs = List::with_capacity(implications.capacity());
            for item in implications {
                pairs
                    .push(w::OwnerKindImplication::new(
                        limits,
                        w::OwnerKindImplicationParts {
                            connector: item.connector,
                            kind: item.kind,
                            implies: item.implies,
                        },
                    )?)
                    .expect("supplied implications");
            }
            w::Bounds::Amend(w::BoundsAmend::new(
                limits,
                w::BoundsAmendParts {
                    budget_left: *budget_left,
                    grants,
                    implications: pairs,
                    authority: owner::authority(models, authority, limits)?,
                    ceiling: owner::authority(models, ceiling, limits)?,
                },
            )?)
        }
    })
}
fn admission(models: &[crate::Model], value: &acts::Admission, limits: &w::Limits) -> Result<w::Admission, w::Problem> {
    Ok(match value {
        acts::Admission::Allowed => w::Admission::Allowed,
        acts::Admission::Proposed { to } => w::Admission::Proposed(w::AdmissionProposed::new(
            limits,
            w::AdmissionProposedParts { to: recipient(to, limits)? },
        )?),
        acts::Admission::Refused(value) => w::Admission::Refused(w::AdmissionRefused::new(
            limits,
            w::AdmissionRefusedParts { value: refusals::refusal(value.clone(), limits)? },
        )?),
        acts::Admission::Bounded { within, beyond } => {
            let beyond = match beyond {
                acts::Beyond::Proposed { to } => w::Beyond::Proposed(w::BeyondProposed::new(
                    limits,
                    w::BeyondProposedParts { to: recipient(to, limits)? },
                )?),
                acts::Beyond::Refused(value) => w::Beyond::Refused(w::BeyondRefused::new(
                    limits,
                    w::BeyondRefusedParts { value: refusals::refusal(value.clone(), limits)? },
                )?),
            };
            w::Admission::Bounded(Box::new(w::AdmissionBounded::new(
                limits,
                w::AdmissionBoundedParts { within: bounds(models, within, limits)?, beyond },
            )?))
        }
    })
}
pub fn offered(
    models: &[crate::Model],
    value: &List<acts::Offered>,
    limits: &w::Limits,
    bound: u32,
    path: w::Path,
) -> Result<Box<[u8]>, w::Problem> {
    if value.capacity() > 25 {
        return Err(problem(skein_codec::Reason::Bound));
    }
    let mut rows = List::with_capacity(value.capacity());
    for item in value {
        rows.push(w::Offered::new(
            limits,
            w::OfferedParts {
                effect: match &item.effect {
                    Some(value) => Some(effect(value, limits)?),
                    None => None,
                },
                act: act(item.act, limits)?,
                admission: admission(models, &item.admission, limits)?,
            },
        )?)
        .expect("supplied offered acts");
    }
    offered_pieces(&rows, bound.min(16_777_216), path)
}

pub fn offered_pieces(rows: &List<w::Offered>, bound: u32, path: w::Path) -> Result<Box<[u8]>, w::Problem> {
    let mut encoded = List::with_capacity(rows.len());
    for row in rows {
        let mut out = skein_lib::Writer::new(usize::try_from(row.measure()).expect("u32 record measure fits usize"));
        match row.encode(&mut out) {
            Ok(()) => (),
            Err(_) => return Err(problem(skein_codec::Reason::Bound)),
        }
        encoded.push(out.finish()).expect("supplied piece count");
    }
    pieces(&encoded, bound, path)
}
pub fn chips_pieces(rows: &List<w::Chip>, bound: u32, path: w::Path) -> Result<Box<[u8]>, w::Problem> {
    let mut encoded = List::with_capacity(rows.len());
    for row in rows {
        let mut out = skein_lib::Writer::new(usize::try_from(row.measure()).expect("u32 record measure fits usize"));
        match row.encode(&mut out) {
            Ok(()) => (),
            Err(_) => return Err(problem(skein_codec::Reason::Bound)),
        }
        encoded.push(out.finish()).expect("supplied piece count");
    }
    pieces(&encoded, bound, path)
}
pub fn links_pieces(rows: &List<w::OwnerTaskLink>, bound: u32, path: w::Path) -> Result<Box<[u8]>, w::Problem> {
    let mut encoded = List::with_capacity(rows.len());
    for row in rows {
        let mut out = skein_lib::Writer::new(usize::try_from(row.measure()).expect("u32 record measure fits usize"));
        match row.encode(&mut out) {
            Ok(()) => (),
            Err(_) => return Err(problem(skein_codec::Reason::Bound)),
        }
        encoded.push(out.finish()).expect("supplied piece count");
    }
    pieces(&encoded, bound, path)
}
pub fn task_resources_pieces(rows: &List<w::TaskResource>, bound: u32, path: w::Path) -> Result<Box<[u8]>, w::Problem> {
    let mut encoded = List::with_capacity(rows.len());
    for row in rows {
        let mut out = skein_lib::Writer::new(usize::try_from(row.measure()).expect("u32 record measure fits usize"));
        match row.encode(&mut out) {
            Ok(()) => (),
            Err(_) => return Err(problem(skein_codec::Reason::Bound)),
        }
        encoded.push(out.finish()).expect("supplied piece count");
    }
    pieces(&encoded, bound, path)
}
pub fn delegates_pieces(rows: &List<w::LiveDelegate>, bound: u32, path: w::Path) -> Result<Box<[u8]>, w::Problem> {
    let mut encoded = List::with_capacity(rows.len());
    for row in rows {
        let mut out = skein_lib::Writer::new(usize::try_from(row.measure()).expect("u32 record measure fits usize"));
        match row.encode(&mut out) {
            Ok(()) => (),
            Err(_) => return Err(problem(skein_codec::Reason::Bound)),
        }
        encoded.push(out.finish()).expect("supplied piece count");
    }
    pieces(&encoded, bound, path)
}
pub fn document(value: &d::Task, models: &[crate::Model], limits: &w::Limits) -> Result<w::Task, w::Problem> {
    // waits_on is carried by the document wire in jig 22.
    if value.project != value.chip.details.project
        || value.executor != value.chip.details.executor
        || value.words != value.spec.words
        || value.parameters != value.spec.parameters
        || value.parameters.capacity() != value.spec.parameters.capacity()
    {
        return Err(problem(skein_codec::Reason::Malformed));
    }
    let mut lineage_rows = List::with_capacity(value.lineage.capacity());
    let mut lineage = List::with_capacity(value.lineage.capacity());
    for item in &value.lineage {
        lineage_rows.push(chips::chip(item, limits)?).expect("supplied lineage chips");
        lineage.push(item.number).expect("supplied lineage");
    }
    let lineage_chips = chips_pieces(
        &lineage_rows,
        limits.task_lineage_chips.min(w::CEILINGS.task_lineage_chips),
        w::Path::TaskLineageChips,
    )?;
    let inputs =
        link_list(&value.inputs, limits, limits.task_inputs.min(w::CEILINGS.task_inputs), w::Path::TaskInputs)?;
    let dependencies = link_list(
        &value.dependencies,
        limits,
        limits.task_dependencies.min(w::CEILINGS.task_dependencies),
        w::Path::TaskDependencies,
    )?;
    let references = link_list(
        &value.references,
        limits,
        limits.task_references.min(w::CEILINGS.task_references),
        w::Path::TaskReferences,
    )?;
    let resources = resources(&value.resources, limits)?;
    let mut proposals = List::with_capacity(value.proposals.capacity());
    for item in &value.proposals {
        proposals.push(proposal(item, limits)?).expect("supplied proposals");
    }
    w::Task::new(
        limits,
        w::TaskParts {
            settings: settings(value.settings, limits)?,
            dependency_facts: chips::dependencies(&value.dependency_facts, limits)?,
            llm: super::llm::choice(value.llm.as_ref(), models, limits)?,
            kept_account: value.kept_account,
            run_wait: super::task_wait::waiting(value.run_wait, limits)?,
            label: value.label,
            flags: super::me::flags(value.flags, limits)?,
            last_activity: value.last_activity.as_nanos(),
            waits_on: super::queries::waits(value.waits_on.clone(), limits)?,
            chip: chips::chip(&value.chip, limits)?,
            goal: value.goal,
            priority: value.priority,
            lineage_chips,
            inputs,
            project: value.project,
            requester: owner::requester(value.requester, limits)?,
            lineage,
            spec: owner::specification(&value.spec, limits)?,
            spec_revision: w::Revision::new(limits, w::RevisionParts { value: value.spec_revision })?,
            person_revision: match value.person_revision {
                Some(item) => Some(w::Revision::new(limits, w::RevisionParts { value: item })?),
                None => None,
            },
            taken_by: value.taken_by,
            instructions: match &value.instructions {
                Some(item) => Some(owner::text(item, limits)?),
                None => None,
            },
            wake: owner::wake(value.wake, limits)?,
            subscriptions: owner::subscriptions(&value.subscriptions, limits)?,
            result_contract: owner::contract(&value.contract, limits)?,
            dependencies,
            references,
            cut: value.cut,
            resources,
            budget: owner::budget(&value.budget, limits)?,
            authority: owner::authority(models, &value.authority, limits)?,
            hold: match &value.hold {
                Some(item) => Some(w::TaskHold::Owner(w::TaskHoldOwner::new(
                    limits,
                    w::TaskHoldOwnerParts { value: chips::held(item, limits)? },
                )?)),
                None => None,
            },
            proposals,
            activity: value.activity,
            changed: value.changed.as_nanos(),
            acts: offered(models, &value.acts, limits, limits.task_acts, w::Path::TaskActs)?,
            result: match &value.result {
                Some(item) => Some(w::TaskResult::Ending(w::TaskResultEnding::new(
                    limits,
                    w::TaskResultEndingParts { value: chips::ending(item, limits)? },
                )?)),
                None => None,
            },
            delegates: delegates(&value.delegates, limits)?,
            context: None,
        },
    )
}

fn link_list(
    value: &List<d::TaskLink>,
    limits: &w::Limits,
    bound: u32,
    path: w::Path,
) -> Result<Box<[u8]>, w::Problem> {
    let mut rows = List::with_capacity(value.capacity());
    for item in value {
        rows.push(link(item, limits)?).expect("supplied task links");
    }
    links_pieces(&rows, bound, path)
}

fn resources(value: &List<d::ResourceLine>, limits: &w::Limits) -> Result<Box<[u8]>, w::Problem> {
    let mut resource_rows = List::with_capacity(value.capacity());
    for item in value {
        resource_rows
            .push(w::TaskResource::Owner(w::TaskResourceOwner::new(
                limits,
                w::TaskResourceOwnerParts { value: chips::resource(item, limits)? },
            )?))
            .expect("supplied resources");
    }
    task_resources_pieces(&resource_rows, limits.task_resources.min(w::CEILINGS.task_resources), w::Path::TaskResources)
}

fn delegates(value: &List<d::LiveDelegate>, limits: &w::Limits) -> Result<Box<[u8]>, w::Problem> {
    let mut delegate_rows = List::with_capacity(value.capacity());
    for item in value {
        delegate_rows
            .push(w::LiveDelegate::new(
                limits,
                w::LiveDelegateParts { chip: chips::chip(&item.chip, limits)?, takeover_budget: item.takeover_budget },
            )?)
            .expect("supplied delegates");
    }
    delegates_pieces(&delegate_rows, limits.task_delegates.min(w::CEILINGS.task_delegates), w::Path::TaskDelegates)
}

pub fn effect(value: &acts::Effect, limits: &w::Limits) -> Result<w::Effect, w::Problem> {
    Ok(match value {
        acts::Effect::Release(value) => w::Effect::Release(w::EffectRelease::new(
            limits,
            w::EffectReleaseParts {
                what: match value {
                    acts::ReleaseEffect::Judged => w::ReleaseEffect::Judged,
                    acts::ReleaseEffect::RunAgain => w::ReleaseEffect::RunAgain,
                    acts::ReleaseEffect::Uncertain => w::ReleaseEffect::Uncertain,
                },
            },
        )?),
        acts::Effect::Move { reservation } => {
            w::Effect::MoveTask(w::EffectMove::new(limits, w::EffectMoveParts { reservation: *reservation })?)
        }
        acts::Effect::Cancel(value) => w::Effect::Cancel(w::Impact::new(
            limits,
            w::ImpactParts {
                tasks: w::PhaseCounts::new(
                    limits,
                    w::PhaseCountsParts {
                        ended: 0,
                        waiting: value.tasks.waiting,
                        running: value
                            .tasks
                            .idle
                            .checked_add(value.tasks.starting)
                            .expect("task bound")
                            .checked_add(value.tasks.running)
                            .expect("task bound")
                            .checked_add(value.tasks.backing_off)
                            .expect("task bound"),
                        closing: value.tasks.closing,
                        held: value.tasks.held,
                        done: value.tasks.done,
                        failed: value.tasks.failed,
                        cancelled: value.tasks.cancelled,
                    },
                )?,
                live_runs: value.live_runs,
                proposals: value.proposals,
            },
        )?),
    })
}

fn settings(value: Option<d::TaskSettings>, limits: &w::Limits) -> Result<Option<w::TaskSettings>, w::Problem> {
    match value {
        Some(value) => Ok(Some(w::TaskSettings::new(
            limits,
            w::TaskSettingsParts { task: value.task, run: value.run, newest: value.newest },
        )?)),
        None => Ok(None),
    }
}
