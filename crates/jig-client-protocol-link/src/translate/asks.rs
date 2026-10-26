//! Field-for-field client contract translation (protocol/client.md, 3 and 5).
use crate::translate::too_large;
use alloc::boxed::Box;
use jig_client_domain::contract as c;
use jig_client_protocol_wire::v19 as w;
use skein_lib::{List, Writer, bytes};
pub(crate) fn wire_revision(value: c::Revision, limits: &w::Limits) -> Result<w::Revision, w::Problem> {
    w::Revision::new(limits, w::RevisionParts { value: value.value })
}

#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn wire_person_answer(value: &c::PersonAnswer, limits: &w::Limits) -> Result<w::PersonAnswer, w::Problem> {
    Ok(match value {
        c::PersonAnswer::Choice { option, reason } => w::PersonAnswer::Choice(w::PersonAnswerChoice::new(
            limits,
            w::PersonAnswerChoiceParts { option: *option, reason: bytes::copy_of(reason.bytes()) },
        )?),
        c::PersonAnswer::Approved { state, words } => {
            let opt0 = match words {
                Some(item) => Some(bytes::copy_of(item.bytes())),
                None => None,
            };
            w::PersonAnswer::Approved(w::PersonAnswerApproved::new(
                limits,
                w::PersonAnswerApprovedParts { state: bytes::copy_of(state), words: opt0 },
            )?)
        }
        c::PersonAnswer::Words(value) => w::PersonAnswer::Words(w::PersonAnswerWords::new(
            limits,
            w::PersonAnswerWordsParts { value: bytes::copy_of(value.bytes()) },
        )?),
        c::PersonAnswer::Cannot { reason } => w::PersonAnswer::Cannot(w::PersonAnswerCannot::new(
            limits,
            w::PersonAnswerCannotParts { reason: bytes::copy_of(reason.bytes()) },
        )?),
    })
}

pub(crate) fn wire_proposer(value: &c::Proposer, limits: &w::Limits) -> Result<w::Proposer, w::Problem> {
    Ok(match value {
        c::Proposer::Task(value) => {
            w::Proposer::Task(w::ProposerTask::new(limits, w::ProposerTaskParts { value: *value })?)
        }
        c::Proposer::Party(value) => {
            w::Proposer::Party(w::ProposerParty::new(limits, w::ProposerPartyParts { value: *value })?)
        }
    })
}

pub(crate) fn wire_proposal_key(value: &c::ProposalKey, limits: &w::Limits) -> Result<w::ProposalKey, w::Problem> {
    w::ProposalKey::new(
        limits,
        w::ProposalKeyParts { proposer: wire_proposer(&value.proposer, limits)?, number: value.number },
    )
}

#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn wire_decision(value: &c::Decision, limits: &w::Limits) -> Result<w::Decision, w::Problem> {
    Ok(match value {
        c::Decision::Accepted => w::Decision::Accepted,
        c::Decision::Rejected { reason } => w::Decision::Rejected(w::DecisionRejected::new(
            limits,
            w::DecisionRejectedParts { reason: bytes::copy_of(reason.bytes()) },
        )?),
        c::Decision::PassedUp => w::Decision::PassedUp,
        c::Decision::Released { again } => {
            let opt0 = match again {
                Some(item) => Some(*item),
                None => None,
            };
            w::Decision::Released(w::DecisionReleased::new(limits, w::DecisionReleasedParts { again: opt0 })?)
        }
        c::Decision::LeftHeld { reason } => w::Decision::LeftHeld(w::DecisionLeftHeld::new(
            limits,
            w::DecisionLeftHeldParts { reason: bytes::copy_of(reason.bytes()) },
        )?),
    })
}

pub(crate) fn wire_person_address(
    value: &c::PersonAddress,
    limits: &w::Limits,
) -> Result<w::PersonAddress, w::Problem> {
    Ok(match value {
        c::PersonAddress::Party(value) => {
            w::PersonAddress::Party(w::PersonAddressParty::new(limits, w::PersonAddressPartyParts { value: *value })?)
        }
        c::PersonAddress::Role(value) => {
            w::PersonAddress::Role(w::PersonAddressRole::new(limits, w::PersonAddressRoleParts { value: *value })?)
        }
    })
}

pub(crate) fn wire_address_change(
    value: &c::AddressChange,
    limits: &w::Limits,
) -> Result<w::AddressChange, w::Problem> {
    w::AddressChange::new(
        limits,
        w::AddressChangeParts {
            revision: wire_revision(value.revision, limits)?,
            address: wire_person_address(&value.address, limits)?,
        },
    )
}

pub(crate) fn wire_parameter(value: &c::Parameter, limits: &w::Limits) -> Result<w::Parameter, w::Problem> {
    Ok(match value {
        c::Parameter::Number { name, value } => w::Parameter::Number(w::ParameterNumber::new(
            limits,
            w::ParameterNumberParts { name: *name, value: *value },
        )?),
        c::Parameter::Bytes { name, value } => w::Parameter::Bytes(w::ParameterBytes::new(
            limits,
            w::ParameterBytesParts { name: *name, value: bytes::copy_of(value.bytes()) },
        )?),
        c::Parameter::Resource { name, connector, resource } => w::Parameter::Resource(w::ParameterResource::new(
            limits,
            w::ParameterResourceParts { name: *name, connector: *connector, resource: *resource },
        )?),
    })
}

pub(crate) fn wire_specification(value: &c::Specification, limits: &w::Limits) -> Result<w::Specification, w::Problem> {
    let mut list0 = List::with_capacity(value.parameters.len());
    for item in &value.parameters {
        list0.push(wire_parameter(item, limits)?).expect("reserved supplied list");
    }
    let mut list1 = List::with_capacity(value.inputs.len());
    for item in &value.inputs {
        list1.push(*item).expect("reserved supplied list");
    }
    w::Specification::new(
        limits,
        w::SpecificationParts { words: bytes::copy_of(value.words.bytes()), parameters: list0, inputs: list1 },
    )
}

pub(crate) fn wire_wake_rule(value: &c::WakeRule, limits: &w::Limits) -> Result<w::WakeRule, w::Problem> {
    Ok(match value {
        c::WakeRule::Never => w::WakeRule::Never,
        c::WakeRule::Immediate => w::WakeRule::Immediate,
        c::WakeRule::Batch { count, age } => {
            w::WakeRule::Batch(w::WakeRuleBatch::new(limits, w::WakeRuleBatchParts { count: *count, age: *age })?)
        }
    })
}

pub(crate) fn wire_results_wake(value: c::ResultsWake, _limits: &w::Limits) -> w::ResultsWake {
    match value {
        c::ResultsWake::Never => w::ResultsWake::Never,
        c::ResultsWake::Each => w::ResultsWake::Each,
        c::ResultsWake::LastOrFailure => w::ResultsWake::LastOrFailure,
    }
}

pub(crate) fn wire_owner_wake_policy(
    value: &c::OwnerWakePolicy,
    limits: &w::Limits,
) -> Result<w::OwnerWakePolicy, w::Problem> {
    w::OwnerWakePolicy::new(
        limits,
        w::OwnerWakePolicyParts {
            words: wire_wake_rule(&value.words, limits)?,
            notices: wire_wake_rule(&value.notices, limits)?,
            news: wire_wake_rule(&value.news, limits)?,
            results: wire_results_wake(value.results, limits),
            questions: value.questions,
            answers: value.answers,
            timers: value.timers,
        },
    )
}

pub(crate) fn wire_owner_pattern_end(
    value: &c::OwnerPatternEnd,
    limits: &w::Limits,
) -> Result<w::OwnerPatternEnd, w::Problem> {
    Ok(match value {
        c::OwnerPatternEnd::Exact(value) => w::OwnerPatternEnd::Exact(w::OwnerPatternEndExact::new(
            limits,
            w::OwnerPatternEndExactParts { value: bytes::copy_of(value) },
        )?),
        c::OwnerPatternEnd::Open(value) => w::OwnerPatternEnd::Open(w::OwnerPatternEndOpen::new(
            limits,
            w::OwnerPatternEndOpenParts { value: bytes::copy_of(value) },
        )?),
    })
}

pub(crate) fn wire_owner_pattern(value: &c::OwnerPattern, limits: &w::Limits) -> Result<w::OwnerPattern, w::Problem> {
    let mut list0 = List::with_capacity(value.segments.len());
    for item in &value.segments {
        list0.push(bytes::copy_of(item)).expect("reserved supplied list");
    }
    w::OwnerPattern::new(
        limits,
        w::OwnerPatternParts { segments: list0, last: wire_owner_pattern_end(&value.last, limits)? },
    )
}

pub(crate) fn wire_owner_grant(value: &c::OwnerGrant, limits: &w::Limits) -> Result<w::OwnerGrant, w::Problem> {
    w::OwnerGrant::new(
        limits,
        w::OwnerGrantParts {
            connector: value.connector,
            kind: value.kind,
            pattern: wire_owner_pattern(&value.pattern, limits)?,
        },
    )
}

pub(super) fn wire_owner_grant_piece(rows: &List<w::OwnerGrant>, limits: &w::Limits) -> Result<Box<[u8]>, w::Problem> {
    let mut length = 0_u32;
    for row in rows {
        length = length.checked_add(row.measure()).ok_or(too_large())?;
        length = length.checked_add(4).ok_or(too_large())?;
    }
    if length > limits.owned_record {
        return Err(too_large());
    }
    let mut writer = Writer::new(usize::try_from(length).expect("u32 fits usize"));
    for row in rows {
        let mut item = Writer::new(usize::try_from(row.measure()).expect("u32 fits usize"));
        row.encode(&mut item).expect("measured row");
        let item = item.finish();
        skein_codec::piece::write(&mut writer, &item).expect("measured piece");
    }
    Ok(writer.finish())
}

pub(crate) fn wire_owner_authority_executor(
    value: c::OwnerAuthorityExecutor,
    limits: &w::Limits,
) -> Result<w::OwnerAuthorityExecutor, w::Problem> {
    Ok(match value {
        c::OwnerAuthorityExecutor::Charter(value) => w::OwnerAuthorityExecutor::Charter(
            w::OwnerAuthorityExecutorCharter::new(limits, w::OwnerAuthorityExecutorCharterParts { value })?,
        ),
        c::OwnerAuthorityExecutor::Procedure(value) => w::OwnerAuthorityExecutor::Procedure(
            w::OwnerAuthorityExecutorProcedure::new(limits, w::OwnerAuthorityExecutorProcedureParts { value })?,
        ),
        c::OwnerAuthorityExecutor::Role(value) => w::OwnerAuthorityExecutor::Role(w::OwnerAuthorityExecutorRole::new(
            limits,
            w::OwnerAuthorityExecutorRoleParts { value },
        )?),
    })
}

pub(crate) fn wire_owner_delegation(
    value: &c::OwnerDelegation,
    limits: &w::Limits,
) -> Result<w::OwnerDelegation, w::Problem> {
    let mut list0 = List::with_capacity(value.kinds.len());
    for item in &value.kinds {
        list0.push(wire_owner_authority_executor(*item, limits)?).expect("reserved supplied list");
    }
    w::OwnerDelegation::new(limits, w::OwnerDelegationParts { kinds: list0, tasks: value.tasks, depth: value.depth })
}

pub(crate) fn wire_owner_resource_scope(
    value: &c::OwnerResourceScope,
    limits: &w::Limits,
) -> Result<w::OwnerResourceScope, w::Problem> {
    w::OwnerResourceScope::new(
        limits,
        w::OwnerResourceScopeParts { connector: value.connector, pattern: wire_owner_pattern(&value.pattern, limits)? },
    )
}

fn wire_owner_resource_scope_piece(
    rows: &List<w::OwnerResourceScope>,
    limits: &w::Limits,
) -> Result<Box<[u8]>, w::Problem> {
    let mut length = 0_u32;
    for row in rows {
        length = length.checked_add(row.measure()).ok_or(too_large())?;
        length = length.checked_add(4).ok_or(too_large())?;
    }
    if length > limits.owned_record {
        return Err(too_large());
    }
    let mut writer = Writer::new(usize::try_from(length).expect("u32 fits usize"));
    for row in rows {
        let mut item = Writer::new(usize::try_from(row.measure()).expect("u32 fits usize"));
        row.encode(&mut item).expect("measured row");
        let item = item.finish();
        skein_codec::piece::write(&mut writer, &item).expect("measured piece");
    }
    Ok(writer.finish())
}

#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn wire_owner_authority(
    value: &c::OwnerAuthority,
    limits: &w::Limits,
) -> Result<w::OwnerAuthority, w::Problem> {
    let mut list0 = List::with_capacity(value.grants.len());
    for item in &value.grants {
        list0.push(wire_owner_grant(item, limits)?).expect("reserved supplied list");
    }
    let opt1 = match value.deadline {
        Some(item) => Some(item.as_nanos()),
        None => None,
    };
    let mut list2 = List::with_capacity(value.note_resources.len());
    for item in &value.note_resources {
        list2.push(wire_owner_resource_scope(item, limits)?).expect("reserved supplied list");
    }
    w::OwnerAuthority::new(
        limits,
        w::OwnerAuthorityParts {
            models: super::model_sets::encode(value.models.as_ref(), limits)?,
            tools: value.tools,
            grants: wire_owner_grant_piece(&list0, limits)?,
            delegation: wire_owner_delegation(&value.delegation, limits)?,
            spend: value.spend,
            deadline: opt1,
            notes: value.notes,
            note_resources: wire_owner_resource_scope_piece(&list2, limits)?,
        },
    )
}

#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn wire_amendment(value: &c::Amendment, limits: &w::Limits) -> Result<w::Amendment, w::Problem> {
    let opt0 = match &value.addressed {
        Some(item) => Some(wire_address_change(item, limits)?),
        None => None,
    };
    let opt1 = match &value.spec {
        Some(item) => Some(wire_specification(item, limits)?),
        None => None,
    };
    let opt2 = match &value.wake {
        Some(item) => Some(wire_owner_wake_policy(item, limits)?),
        None => None,
    };
    let opt3 = match &value.dependencies {
        Some(item) => {
            let mut list0 = List::with_capacity((item).len());
            for item in item {
                list0.push(*item).expect("reserved supplied list");
            }
            Some(list0)
        }
        None => None,
    };
    let opt4 = match &value.authority {
        Some(item) => Some(wire_owner_authority(item, limits)?),
        None => None,
    };
    let opt5 = match &value.instructions {
        Some(item) => Some(bytes::copy_of(item.bytes())),
        None => None,
    };
    w::Amendment::new(
        limits,
        w::AmendmentParts {
            addressed: opt0,
            spec: opt1,
            wake: opt2,
            dependencies: opt3,
            authority: opt4,
            instructions: opt5,
            tracked: match value.tracked {
                Some(v) => Some(super::operations::tracking(v, limits)?),
                None => None,
            },
            llm: super::llm::wire_choice(value.llm.as_ref(), limits)?,
        },
    )
}

pub(crate) fn wire_seen(value: &c::SeenThrough, limits: &w::Limits) -> Result<w::SeenThrough, w::Problem> {
    Ok(match value {
        c::SeenThrough::Results { through } => {
            w::SeenThrough::Results(w::SeenResults::new(limits, w::SeenResultsParts { through: *through })?)
        }
        c::SeenThrough::Conversation { task: chat, through } => w::SeenThrough::Conversation(w::SeenConversation::new(
            limits,
            w::SeenConversationParts { task: *chat, through: *through },
        )?),
    })
}

pub(crate) fn wire_note_scope(value: &c::NoteScope, limits: &w::Limits) -> Result<w::NoteScope, w::Problem> {
    Ok(match value {
        c::NoteScope::Deployment => w::NoteScope::Deployment,
        c::NoteScope::Project(value) => {
            w::NoteScope::Project(w::NoteScopeProject::new(limits, w::NoteScopeProjectParts { value: *value })?)
        }
        c::NoteScope::Resource { project, connector, pattern } => w::NoteScope::Resource(w::NoteScopeResource::new(
            limits,
            w::NoteScopeResourceParts {
                project: *project,
                connector: *connector,
                pattern: wire_owner_pattern(pattern, limits)?,
            },
        )?),
        c::NoteScope::Task { project, task: goal } => {
            w::NoteScope::Task(w::NoteScopeTask::new(limits, w::NoteScopeTaskParts { project: *project, task: *goal })?)
        }
    })
}

pub(crate) fn wire_policy_edit_role(
    value: &c::PolicyEditRole,
    limits: &w::Limits,
) -> Result<w::PolicyEditRole, w::Problem> {
    w::PolicyEditRole::new(
        limits,
        w::PolicyEditRoleParts {
            number: value.number,
            authority: wire_owner_authority(&value.authority, limits)?,
            period_spend: value.period_spend,
            requests: value.requests,
            decides: value.decides,
        },
    )
}

pub(crate) fn wire_policy_judge(value: c::PolicyJudge, limits: &w::Limits) -> Result<w::PolicyJudge, w::Problem> {
    w::PolicyJudge::new(
        limits,
        w::PolicyJudgeParts {
            connector: value.connector,
            requirement: value.requirement,
            parameters: value.parameters,
        },
    )
}

pub(crate) fn wire_policy_guard(value: &c::PolicyGuard, limits: &w::Limits) -> Result<w::PolicyGuard, w::Problem> {
    Ok(match value {
        c::PolicyGuard::Guarded => w::PolicyGuard::Guarded,
        c::PolicyGuard::Observed { freshness } => w::PolicyGuard::Observed(w::PolicyGuardObserved::new(
            limits,
            w::PolicyGuardObservedParts { freshness: *freshness },
        )?),
    })
}

pub(crate) fn wire_policy_requirement(
    value: &c::PolicyRequirement,
    limits: &w::Limits,
) -> Result<w::PolicyRequirement, w::Problem> {
    w::PolicyRequirement::new(
        limits,
        w::PolicyRequirementParts {
            connector: value.connector,
            kind: value.kind,
            pattern: wire_owner_pattern(&value.pattern, limits)?,
            judge: wire_policy_judge(value.judge, limits)?,
            guard: wire_policy_guard(&value.guard, limits)?,
            must_be_guarded: value.must_be_guarded,
        },
    )
}

fn wire_policy_requirement_piece(
    rows: &List<w::PolicyRequirement>,
    limits: &w::Limits,
) -> Result<Box<[u8]>, w::Problem> {
    let mut length = 0_u32;
    for row in rows {
        length = length.checked_add(row.measure()).ok_or(too_large())?;
        length = length.checked_add(4).ok_or(too_large())?;
    }
    if length > limits.owned_record {
        return Err(too_large());
    }
    let mut writer = Writer::new(usize::try_from(length).expect("u32 fits usize"));
    for row in rows {
        let mut item = Writer::new(usize::try_from(row.measure()).expect("u32 fits usize"));
        row.encode(&mut item).expect("measured row");
        let item = item.finish();
        skein_codec::piece::write(&mut writer, &item).expect("measured piece");
    }
    Ok(writer.finish())
}

pub(crate) fn wire_policy_permission(
    value: c::PolicyPermission,
    limits: &w::Limits,
) -> Result<w::PolicyPermission, w::Problem> {
    w::PolicyPermission::new(
        limits,
        w::PolicyPermissionParts { connector: value.connector, permission: value.permission, role: value.role },
    )
}

pub(crate) fn wire_policy_edit(value: &c::PolicyEdit, limits: &w::Limits) -> Result<w::PolicyEdit, w::Problem> {
    Ok(match value {
        c::PolicyEdit::Templates { overrides } => super::operations::templates(overrides, limits)?,
        c::PolicyEdit::ProjectSpend { period_spend } => w::PolicyEdit::ProjectSpend(w::PolicyEditProjectSpend::new(
            limits,
            w::PolicyEditProjectSpendParts { period_spend: *period_spend },
        )?),
        c::PolicyEdit::Role(value) => w::PolicyEdit::Role(w::PolicyEditRoleValue::new(
            limits,
            w::PolicyEditRoleValueParts { value: wire_policy_edit_role(value, limits)? },
        )?),
        c::PolicyEdit::Requirements { requirements } => {
            let mut list0 = List::with_capacity((requirements).len());
            for item in requirements {
                list0.push(wire_policy_requirement(item, limits)?).expect("reserved supplied list");
            }
            w::PolicyEdit::Requirements(w::PolicyEditRequirements::new(
                limits,
                w::PolicyEditRequirementsParts { requirements: wire_policy_requirement_piece(&list0, limits)? },
            )?)
        }
        c::PolicyEdit::Projections { grants } => {
            let mut list0 = List::with_capacity((grants).len());
            for item in grants {
                list0.push(wire_owner_grant(item, limits)?).expect("reserved supplied list");
            }
            w::PolicyEdit::Projections(w::PolicyEditProjections::new(
                limits,
                w::PolicyEditProjectionsParts { grants: wire_owner_grant_piece(&list0, limits)? },
            )?)
        }
        c::PolicyEdit::Permissions { mappings } => {
            let mut list0 = List::with_capacity((mappings).len());
            for item in mappings {
                list0.push(wire_policy_permission(*item, limits)?).expect("reserved supplied list");
            }
            w::PolicyEdit::Permissions(w::PolicyEditPermissions::new(
                limits,
                w::PolicyEditPermissionsParts { mappings: list0 },
            )?)
        }
    })
}

pub(crate) fn wire_held_role(value: c::HeldRole, limits: &w::Limits) -> Result<w::HeldRole, w::Problem> {
    Ok(match value {
        c::HeldRole::Owner => w::HeldRole::Owner,
        c::HeldRole::Maintainer => w::HeldRole::Maintainer,
        c::HeldRole::Member => w::HeldRole::Member,
        c::HeldRole::Observer => w::HeldRole::Observer,
        c::HeldRole::Policy { role } => {
            w::HeldRole::Policy(w::HeldRolePolicy::new(limits, w::HeldRolePolicyParts { role })?)
        }
    })
}

pub(crate) fn wire_holding(value: &c::Holding, limits: &w::Limits) -> Result<w::Holding, w::Problem> {
    w::Holding::new(limits, w::HoldingParts { party: value.party, role: wire_held_role(value.role, limits)? })
}

#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
#[expect(clippy::too_many_lines, reason = "one exhaustive field translation arm per client ask")]
pub fn ask(value: &c::Ask, limits: &w::Limits) -> Result<w::Ask, w::Problem> {
    Ok(match value {
        c::Ask::MoveToNewest(value) => {
            w::Ask::MoveToNewest(w::MoveToNewest::new(limits, w::MoveToNewestParts { revision: value.revision })?)
        }
        c::Ask::ChangeSettings(value) => w::Ask::ChangeSettings(super::settings::wire_change_settings(value, limits)?),
        c::Ask::Propose { project, action, reason } => super::operations::propose(*project, action, reason, limits)?,
        c::Ask::Withdraw { project, proposal } => {
            w::Ask::Withdraw(w::Withdraw::new(limits, w::WithdrawParts { project: *project, proposal: *proposal })?)
        }
        c::Ask::Create { project, tasks } => super::operations::create(*project, tasks, limits)?,
        c::Ask::Compound { project, operations } => super::compound::encode(*project, operations, limits)?,
        c::Ask::Message { project, task, words } => w::Ask::Message(w::Message::new(
            limits,
            w::MessageParts { project: *project, task: *task, words: bytes::copy_of(words.bytes()) },
        )?),
        c::Ask::Answer { project, on } => w::Ask::Answer(w::Answer::new(
            limits,
            w::AnswerParts { project: *project, on: super::operations::answer(on, limits)? },
        )?),
        c::Ask::Take { project, task } => {
            w::Ask::Take(w::Take::new(limits, w::TakeParts { project: *project, task: *task })?)
        }
        c::Ask::HandBack { project, task } => {
            w::Ask::HandBack(w::HandBack::new(limits, w::HandBackParts { project: *project, task: *task })?)
        }
        c::Ask::Decide { project, on } => w::Ask::Decide(w::Decide::new(
            limits,
            w::DecideParts { project: *project, on: super::operations::decide(on, limits)? },
        )?),
        c::Ask::Release { project, task, revision, again } => {
            let opt0 = match again {
                Some(item) => Some(*item),
                None => None,
            };
            w::Ask::Release(w::Release::new(
                limits,
                w::ReleaseParts {
                    project: *project,
                    task: *task,
                    revision: wire_revision(*revision, limits)?,
                    again: opt0,
                },
            )?)
        }
        c::Ask::Amend { project, task, revision, change, reason } => w::Ask::Amend(Box::new(w::Amend::new(
            limits,
            w::AmendParts {
                project: *project,
                task: *task,
                revision: match revision {
                    Some(value) => Some(wire_revision(*value, limits)?),
                    None => None,
                },
                change: wire_amendment(change, limits)?,
                reason: bytes::copy_of(reason.bytes()),
            },
        )?)),
        c::Ask::Stop { project, task } => {
            w::Ask::Stop(w::Stop::new(limits, w::StopParts { project: *project, task: *task })?)
        }
        c::Ask::Cancel { project, task, reason, grace } => w::Ask::Cancel(w::Cancel::new(
            limits,
            w::CancelParts { project: *project, task: *task, reason: bytes::copy_of(reason.bytes()), grace: *grace },
        )?),
        c::Ask::Move { project, task, reason } => w::Ask::MoveTask(w::Move::new(
            limits,
            w::MoveParts { project: *project, task: *task, reason: bytes::copy_of(reason.bytes()) },
        )?),
        c::Ask::Seen { through } => {
            w::Ask::Seen(w::Seen::new(limits, w::SeenParts { through: wire_seen(through, limits)? })?)
        }
        c::Ask::EditNote { project, note, scope, revision, description, body, references } => {
            let mut list0 = List::with_capacity((references).len());
            for item in references {
                list0.push(*item).expect("reserved supplied list");
            }
            w::Ask::EditNote(w::EditNote::new(
                limits,
                w::EditNoteParts {
                    project: *project,
                    note: *note,
                    scope: wire_note_scope(scope, limits)?,
                    revision: wire_revision(*revision, limits)?,
                    description: bytes::copy_of(description.bytes()),
                    body: bytes::copy_of(body.bytes()),
                    references: list0,
                },
            )?)
        }
        c::Ask::DeleteNote { project, note, scope, revision } => w::Ask::DeleteNote(w::DeleteNote::new(
            limits,
            w::DeleteNoteParts {
                project: *project,
                note: *note,
                scope: wire_note_scope(scope, limits)?,
                revision: wire_revision(*revision, limits)?,
            },
        )?),
        c::Ask::ChangePolicy { project, edit } => w::Ask::ChangePolicy(w::ChangePolicy::new(
            limits,
            w::ChangePolicyParts { project: *project, edit: wire_policy_edit(edit, limits)? },
        )?),
        c::Ask::SetRoles { project, holdings } => {
            let mut list0 = List::with_capacity((holdings).len());
            for item in holdings {
                list0.push(wire_holding(item, limits)?).expect("reserved supplied list");
            }
            w::Ask::SetRoles(w::SetRoles::new(limits, w::SetRolesParts { project: *project, holdings: list0 })?)
        }
        c::Ask::MakeService { project, name, role } => w::Ask::MakeService(w::MakeService::new(
            limits,
            w::MakeServiceParts {
                project: *project,
                name: bytes::copy_of(name.bytes()),
                role: wire_held_role(*role, limits)?,
            },
        )?),
        c::Ask::AccountChange { holder, change } => w::Ask::AccountChange(w::AskAccountChange::new(
            limits,
            w::AskAccountChangeParts { holder: account_holder(*holder), change: account_change(change, limits)? },
        )?),
        c::Ask::SignIn { holder, provider } => w::Ask::SignIn(w::AskSignIn::new(
            limits,
            w::AskSignInParts { holder: account_holder(*holder), provider: *provider },
        )?),
        c::Ask::SignOut { holder, source } => w::Ask::SignOut(w::AskSignOut::new(
            limits,
            w::AskSignOutParts { holder: account_holder(*holder), source: *source },
        )?),
        c::Ask::Rename { project, name } => w::Ask::Rename(w::Rename::new(
            limits,
            w::RenameParts { project: *project, name: bytes::copy_of(name.bytes()) },
        )?),
        c::Ask::MakeProject { name, policy } => w::Ask::MakeProject(w::MakeProject::new(
            limits,
            w::MakeProjectParts { name: bytes::copy_of(name.bytes()), policy: *policy },
        )?),
        c::Ask::SetPool { project, party, budget } => w::Ask::SetPool(w::SetPool::new(
            limits,
            w::SetPoolParts { project: *project, party: *party, budget: *budget },
        )?),
    })
}

fn account_holder(value: c::AccountHolder) -> w::AccountHolder {
    match value {
        c::AccountHolder::Own => w::AccountHolder::Own,
        c::AccountHolder::Deployment => w::AccountHolder::Deployment,
    }
}

fn account_change(value: &c::AccountChange, limits: &w::Limits) -> Result<w::AccountChange, w::Problem> {
    Ok(match value {
        c::AccountChange::MoveOut { account } => w::AccountChange::MoveOut(w::AccountChangeMoveOut::new(
            limits,
            w::AccountChangeMoveOutParts { account: *account },
        )?),
        c::AccountChange::Written { number } => w::AccountChange::Written(w::AccountChangeWritten::new(
            limits,
            w::AccountChangeWrittenParts { number: *number },
        )?),
        c::AccountChange::FinishSignIn { source, digest } => {
            w::AccountChange::FinishSignIn(w::AccountChangeFinishSignIn::new(
                limits,
                w::AccountChangeFinishSignInParts { source: *source, digest: *digest },
            )?)
        }
        c::AccountChange::MoveIn { digest } => w::AccountChange::MoveIn(w::AccountChangeMoveIn::new(
            limits,
            w::AccountChangeMoveInParts { digest: *digest },
        )?),
        c::AccountChange::DeclareProgram { provider, join, name } => {
            if !c::valid_program_name(name) {
                return Err(super::piece_problem(skein_codec::Reason::Malformed));
            }
            w::AccountChange::DeclareProgram(w::AccountChangeDeclareProgram::new(
                limits,
                w::AccountChangeDeclareProgramParts { provider: *provider, join: *join, name: name.clone() },
            )?)
        }
        c::AccountChange::CheckProgram { source } => w::AccountChange::CheckProgram(w::AccountChangeCheckProgram::new(
            limits,
            w::AccountChangeCheckProgramParts { source: *source },
        )?),
        c::AccountChange::AddKey { provider, join, digest } => w::AccountChange::AddKey(w::AccountChangeAddKey::new(
            limits,
            w::AccountChangeAddKeyParts { provider: *provider, join: *join, digest: *digest },
        )?),
        c::AccountChange::ReplaceKey { source, digest } => {
            w::AccountChange::ReplaceKey(w::AccountChangeReplaceKey::new(
                limits,
                w::AccountChangeReplaceKeyParts { source: *source, digest: *digest },
            )?)
        }
        c::AccountChange::RemoveKey { source } => w::AccountChange::RemoveKey(w::AccountChangeRemoveKey::new(
            limits,
            w::AccountChangeRemoveKeyParts { source: *source },
        )?),
        c::AccountChange::Resume { source } => w::AccountChange::Resume(w::AccountChangeResume::new(
            limits,
            w::AccountChangeResumeParts { source: *source },
        )?),
        c::AccountChange::Remove { account } => w::AccountChange::Remove(w::AccountChangeRemove::new(
            limits,
            w::AccountChangeRemoveParts { account: *account },
        )?),
        c::AccountChange::MoveAccount { account, place } => {
            w::AccountChange::MoveAccount(w::AccountChangeMoveAccount::new(
                limits,
                w::AccountChangeMoveAccountParts { account: *account, place: *place },
            )?)
        }
        c::AccountChange::MoveSource { account, source, place } => {
            w::AccountChange::MoveSource(w::AccountChangeMoveSource::new(
                limits,
                w::AccountChangeMoveSourceParts { account: *account, source: *source, place: *place },
            )?)
        }
        c::AccountChange::SetUses { account, uses } => account_set_uses(*account, uses.as_deref(), limits)?,
    })
}

fn account_set_uses(
    account: u32,
    uses: Option<&[c::OwnerAccountUse]>,
    limits: &w::Limits,
) -> Result<w::AccountChange, w::Problem> {
    Ok(w::AccountChange::SetUses(w::AccountChangeSetUses::new(
        limits,
        w::AccountChangeSetUsesParts {
            account,
            uses: match uses {
                None => None,
                Some(uses) => {
                    if uses.len()
                        > usize::try_from(limits.account_change_set_uses_uses_some.min(c::ACCOUNT_USES))
                            .expect("account uses bound fits")
                    {
                        return Err(w::Problem {
                            path: w::Path::AccountChangeSetUsesUses,
                            reason: skein_codec::Reason::Bound,
                        });
                    }
                    let mut rows =
                        List::with_capacity(u32::try_from(uses.len()).expect("account uses bounded by the wire"));
                    for use_ in uses {
                        rows.push(match use_ {
                            c::OwnerAccountUse::Party(value) => w::OwnerAccountUse::Party(
                                w::OwnerAccountUseParty::new(limits, w::OwnerAccountUsePartyParts { party: *value })?,
                            ),
                            c::OwnerAccountUse::Project(value) => {
                                w::OwnerAccountUse::Project(w::OwnerAccountUseProject::new(
                                    limits,
                                    w::OwnerAccountUseProjectParts { project: *value },
                                )?)
                            }
                        })
                        .expect("reserved uses");
                    }
                    Some(rows)
                }
            },
        },
    )?))
}
