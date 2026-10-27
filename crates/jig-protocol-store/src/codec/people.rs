//! People rows translated exhaustively (protocol/store.md, 5.1).
use crate::people_v13 as wire;
use alloc::boxed::Box;
use jig_domain_core as core;
use jig_domain_people as domain;
use skein_lib::{Duration, List, Reader, Wall, Writer};
mod acts;
mod asks;
mod batch;
mod compound;
mod llm;
mod outcomes;
mod pieces;
pub(crate) mod policy;
pub use asks::{decode_ask, encode_ask};
use pieces::{
    EMPTY, decode_authority_note_resources, decode_delegation_kinds, decode_grant_piece, decode_numbers,
    decode_permissions, decode_requirement_piece, decode_segments, decode_spec_parameters,
    encode_authority_note_resources, encode_delegation_kinds, encode_grant_piece, encode_numbers, encode_permissions,
    encode_requirement_piece, encode_segments, encode_spec_parameters, permission_sum, piece_bound, shared_sum,
};
use policy::{
    decode_amendment, decode_executor, decode_grant, decode_parameter, decode_pattern, decode_permission_role,
    decode_policy_change, decode_policy_role, decode_policy_value, decode_requirement, decode_resource_scope,
    encode_amendment, encode_executor, encode_grant, encode_parameter, encode_pattern, encode_permission_role,
    encode_policy_change, encode_policy_role, encode_policy_value, encode_requirement, encode_resource_scope,
};
/// Derive and admit the people family's bounds against its schema ceilings.
#[must_use]
pub fn limits(domain: &core::Limits) -> Option<wire::Limits> {
    let limits = derived_limits(domain)?;
    wire::check_limits(&limits).ok()?;
    Some(limits)
}

/// Derive the people-v13 bounds before checking its schema ceilings.
///
/// `None` means the derivation's checked arithmetic cannot represent a bound.
/// A returned value may exceed `people_v13::CEILINGS`; callers must use
/// `people_v13::check_limits` before using it to encode or decode records.
/// Configuration uses this diagnostic seam to explain an envelope refusal
/// (protocol/configuration.md, section 6).
#[must_use]
pub fn derived_limits(domain: &core::Limits) -> Option<wire::Limits> {
    let people = &domain.people;
    let authority = &domain.authority;
    let mut limits = wire::Limits {
        act_close_reason: people.words,
        refusal_circle_tasks: domain.tasks.circle,
        refusal_full_tasks: domain.tasks.circle,
        refusal_kept_pending: domain.tasks.dependencies,
        account_change_declare_program_name: 32,
        account_host_failure_name: people.program_host_name_bytes,
        outcome_account_program_declared_name: 32,
        outcome_account_signed_out_failed_hosts: people.program_hosts,
        outcome_account_removed_failed_hosts: people.program_hosts,
        account_change_move_in_digest: 32,
        account_change_finish_sign_in_digest: 32,
        account_change_add_key_digest: 32,
        account_change_replace_key_digest: 32,
        account_change_set_uses_uses_some: domain::USES
            .min(people.words.checked_div(u32::try_from(size_of::<domain::Use>()).ok()?)?),
        outcome_account_sign_in_started_address: domain::ADDRESS,
        ask_compound_operations: people.operations,
        operation_cancel_reason: people.words,
        operation_move_reason: people.words,
        operation_message_words: people.words,
        ask_create_batch: people.batch,
        action_batch_members: people.batch,
        ask_propose_reason: people.words,
        ask_restore_reason: people.words,
        outcome_restored_left_out: 0,
        member_words: people.words,
        member_parameters: 0,
        member_dependencies: 0,
        member_inputs: 0,
        member_named: 0,
        field_rule_name: people.amendment_bytes,
        verdict_fields: 0,
        contract_verdict_choices: 0,
        identity_key_subject: people.identity_bytes,
        identity_login: people.identity_bytes,
        identity_name: people.identity_bytes,
        last_exact_value: people.amendment_bytes.max(authority.segment_bytes),
        last_open_value: people.amendment_bytes.max(authority.segment_bytes),
        pattern_segments: 0,
        delegation_kinds: 0,
        authority_grants: 0,
        authority_note_resources: 0,
        policy_value_projections: authority.grants,
        policy_value_roles: authority.roles,
        policy_value_requirements: authority.requirements,
        policy_value_permissions: 0,
        policy_value_templates: domain.templates,
        policy_change_templates_overrides: domain.templates,
        ask_rename_name: 64,
        ask_make_project_name: 64,
        act_make_project_name: 64,
        policy_change_requirements_requirements: 0,
        policy_change_projections_grants: 0,
        policy_change_permissions_mappings: 0,
        note_change_correct_description: people.amendment_bytes,
        note_change_correct_body: people.amendment_bytes,
        note_change_correct_references: 0,
        resource_name_path: 0,
        adoption_options: 0,
        person_result_approved_state: people.words,
        person_result_approved_words_some: people.words,
        person_result_report_words: people.words,
        person_result_verdict_words: people.words,
        person_result_failure_reason: people.words,
        proposal_decision_reject_reason: people.words,
        escalation_decision_reject_reason: people.words,
        parameter_bytes_value: people.amendment_bytes,
        spec_words: people.amendment_bytes,
        spec_parameters: 0,
        spec_inputs: 0,
        amendment_instructions_some: people.amendment_bytes,
        amendment_dependencies_some: 0,
        amendment_reason: people.words,
        ask_make_service_name: people.words,
        ask_cancel_reason: people.words,
        ask_move_reason: people.words,
        ask_say_words: people.words,
        ask_answer_question_words: people.words,
        ask_set_roles_holdings: people.holdings,
        stored_roles_holdings: people.holdings,
        stored_policy_models: u64::BITS,
        stored_policy_name: u32::try_from(domain::NAME_BYTES).ok()?,
        stored_answer_models: u64::BITS,
        model_name_name: core::MODEL_NAME_BYTES,
        llm_choice_subagents: domain::MODELS,
        segment_bytes: people.amendment_bytes.max(authority.segment_bytes),
    };
    derive_shared(domain, &mut limits)?;
    derive_amendment(domain, &mut limits)?;
    limits.member_parameters = limits.spec_parameters;
    limits.member_inputs = limits.spec_inputs;
    limits.member_named = limits.authority_grants;
    limits.member_dependencies = people.amendment_bytes.checked_mul(3)?;
    derive_verdicts(people.amendment_bytes, &mut limits)?;
    Some(limits)
}

fn derive_verdicts(amendment_bytes: u32, limits: &mut wire::Limits) -> Option<()> {
    limits.verdict_fields =
        piece_bound(amendment_bytes, size_of::<domain::FieldRule>(), wire::FieldRule::worst_case_bytes(&EMPTY)?, true)?;
    limits.contract_verdict_choices =
        piece_bound(amendment_bytes, size_of::<domain::Verdict>(), wire::Verdict::worst_case_bytes(&EMPTY)?, true)?;
    Some(())
}

/// Encode a people row; `names` are the configured models' names in configuration's order, which
/// a policy row keeps beside its roles' models (protocol/store.md, 5.1).
pub fn encode(row: &domain::Stored, limits: &wire::Limits, names: &[Box<[u8]>]) -> Result<Box<[u8]>, wire::Problem> {
    let value = match row {
        domain::Stored::Person { number, identity } => wire::Value::Person(wire::StoredPerson::new(
            limits,
            wire::StoredPersonParts { number: *number, identity: encode_identity(identity, limits)? },
        )?),
        domain::Stored::ReadResults { party, through } => wire::Value::ReadResults(wire::StoredReadResults::new(
            limits,
            wire::StoredReadResultsParts { party: *party, through: *through },
        )?),
        domain::Stored::ReadConversation { party, task, through } => {
            wire::Value::ReadConversation(wire::StoredReadConversation::new(
                limits,
                wire::StoredReadConversationParts { party: *party, task: *task, through: *through },
            )?)
        }
        domain::Stored::EndedRead { task, party, through } => wire::Value::EndedRead(wire::EndedRead::new(
            limits,
            wire::EndedReadParts { task: *task, party: *party, through: *through },
        )?),
        domain::Stored::SignIn { number, person, expires } => wire::Value::SignIn(wire::StoredSignIn::new(
            limits,
            wire::StoredSignInParts { number: *number, person: *person, expires: expires.as_nanos() },
        )?),
        domain::Stored::Roles { project, holdings } => wire::Value::Roles(wire::StoredRoles::new(
            limits,
            wire::StoredRolesParts { project: *project, holdings: encode_holdings(holdings, limits)? },
        )?),
        domain::Stored::Policy { project, name, value } => wire::Value::Policy(wire::StoredPolicy::new(
            limits,
            wire::StoredPolicyParts {
                project: *project,
                name: Box::from(name.as_bytes()),
                value: encode_policy_value(value, limits)?,
                models: encode_model_names(names, limits)?,
            },
        )?),
        domain::Stored::Acted { key, act, outcome, at } => wire::Value::Acted(wire::StoredActed::new(
            limits,
            wire::StoredActedParts {
                key: wire::ActKey::new(limits, wire::ActKeyParts { connector: key.connector, key: key.key })?,
                act: acts::encode(act, limits)?,
                outcome: outcomes::encode_outcome(outcome, limits)?,
                at: at.as_nanos(),
            },
        )?),
        domain::Stored::Answer { key, ask, models, outcome, at } => wire::Value::Answer(wire::StoredAnswer::new(
            limits,
            wire::StoredAnswerParts {
                key: encode_request_key(key, limits)?,
                ask: encode_ask(ask, limits)?,
                models: encode_model_names(models, limits)?,
                outcome: outcomes::encode_outcome(outcome, limits)?,
                at: at.as_nanos(),
            },
        )?),
    };
    let row = wire::Row::new(limits, wire::RowParts { value })?;
    let mut writer = Writer::new(usize::try_from(row.measure()).expect("row length fits usize"));
    row.encode(&mut writer).expect("measured room");
    Ok(writer.finish())
}
/// Decode a people row; a policy row's roles' models move to today's `names`, and a name no
/// longer configured drops out (protocol/store.md, 5.1).
#[expect(clippy::disallowed_methods, reason = "store protocol converts validated UTF-8 into the domain name")]
pub fn decode(bytes: &[u8], limits: &wire::Limits, names: &[Box<[u8]>]) -> Result<domain::Stored, wire::Problem> {
    let wire::RowParts { value } = wire::Row::decode(limits, &mut Reader::new(bytes))?.into_parts();
    let row = match value {
        wire::Value::Person(value) => {
            let wire::StoredPersonParts { number, identity } = value.into_parts();
            domain::Stored::Person { number, identity: decode_identity(identity) }
        }
        wire::Value::ReadResults(value) => {
            let wire::StoredReadResultsParts { party, through } = value.into_parts();
            domain::Stored::ReadResults { party, through }
        }
        wire::Value::ReadConversation(value) => {
            let wire::StoredReadConversationParts { party, task, through } = value.into_parts();
            domain::Stored::ReadConversation { party, task, through }
        }
        wire::Value::EndedRead(value) => {
            let wire::EndedReadParts { task, party, through } = value.into_parts();
            domain::Stored::EndedRead { task, party, through }
        }
        wire::Value::SignIn(value) => {
            let wire::StoredSignInParts { number, person, expires } = value.into_parts();
            domain::Stored::SignIn { number, person, expires: Wall::from_nanos(expires) }
        }
        wire::Value::Roles(value) => {
            let wire::StoredRolesParts { project, holdings } = value.into_parts();
            domain::Stored::Roles { project, holdings: decode_holdings(holdings) }
        }
        wire::Value::Policy(value) => {
            let wire::StoredPolicyParts { project, name, value, models } = value.into_parts();
            let mut value = decode_policy_value(value, limits)?;
            let moved = model_moves(&models, names);
            if let Some(models) = value.ceiling.models {
                value.ceiling.models = Some(models.remap(&moved));
            }
            for role in &mut value.roles {
                if let Some(models) = role.authority.models {
                    role.authority.models = Some(models.remap(&moved));
                }
            }
            let name = Box::from(::core::str::from_utf8(&name).expect("decoded UTF-8 name"));
            domain::Stored::Policy { project, name, value }
        }
        wire::Value::Acted(value) => {
            let wire::StoredActedParts { key, act, outcome, at } = value.into_parts();
            let wire::ActKeyParts { connector, key } = key.into_parts();
            domain::Stored::Acted {
                key: domain::ActKey { connector, key },
                act: acts::decode(act, limits)?,
                outcome: outcomes::decode_outcome(outcome),
                at: Wall::from_nanos(at),
            }
        }
        wire::Value::Answer(value) => {
            let wire::StoredAnswerParts { key, ask, models, outcome, at } = value.into_parts();
            domain::Stored::Answer {
                key: decode_request_key(key),
                ask: Box::new(decode_ask(ask, limits)?),
                models: decode_model_names(models)?,
                outcome: outcomes::decode_outcome(outcome),
                at: Wall::from_nanos(at),
            }
        }
    };
    Ok(row)
}

fn encode_model_names(names: &[Box<[u8]>], limits: &wire::Limits) -> Result<List<wire::ModelName>, wire::Problem> {
    let mut result = List::with_capacity(count(names.len())?);
    for name in names {
        let name = wire::ModelName::new(limits, wire::ModelNameParts { name: name.clone() })?;
        result.push(name).expect("same number of names");
    }
    Ok(result)
}
/// For each name the row kept, the bit of today's model of that name, or none.
fn model_moves(kept: &List<wire::ModelName>, names: &[Box<[u8]>]) -> [u64; 64] {
    let mut moved = [0; 64];
    for (bit, name) in kept.iter().enumerate() {
        for (today, configured) in names.iter().enumerate() {
            if configured.as_ref() == name.name() {
                if let (Some(slot), Ok(today)) = (moved.get_mut(bit), u32::try_from(today)) {
                    *slot = 1_u64.checked_shl(today).unwrap_or(0);
                }
                break;
            }
        }
    }
    moved
}
fn decode_model_names(names: List<wire::ModelName>) -> Result<Box<[Box<[u8]>]>, wire::Problem> {
    let mut result = List::with_capacity(names.capacity());
    for name in names.into_boxed() {
        result.push(name.into_parts().name).expect("same context count");
    }
    let result = result.into_boxed();
    if !domain::valid_model_names(&result) {
        return Err(wire::Problem { path: wire::Path::StoredAnswerModels, reason: skein_codec::Reason::Bound });
    }
    Ok(result)
}
fn problem() -> wire::Problem {
    wire::Problem { path: wire::Path::RowValue, reason: skein_codec::Reason::Bound }
}
fn count(length: usize) -> Result<u32, wire::Problem> {
    match u32::try_from(length) {
        Ok(length) => Ok(length),
        Err(_) => Err(problem()),
    }
}

pub(super) fn encode_role(value: domain::Role, limits: &wire::Limits) -> Result<wire::Role, wire::Problem> {
    Ok(match value {
        domain::Role::Owner => wire::Role::Owner,
        domain::Role::Maintainer => wire::Role::Maintainer,
        domain::Role::Member => wire::Role::Member,
        domain::Role::Observer => wire::Role::Observer,
        domain::Role::Policy { role } => {
            wire::Role::Policy(wire::RolePolicy::new(limits, wire::RolePolicyParts { role })?)
        }
    })
}
pub(super) fn decode_role(value: wire::Role) -> domain::Role {
    match value {
        wire::Role::Owner => domain::Role::Owner,
        wire::Role::Maintainer => domain::Role::Maintainer,
        wire::Role::Member => domain::Role::Member,
        wire::Role::Observer => domain::Role::Observer,
        wire::Role::Policy(value) => {
            let wire::RolePolicyParts { role } = value.into_parts();
            domain::Role::Policy { role }
        }
    }
}

pub(super) fn encode_holding(value: &domain::Holding, limits: &wire::Limits) -> Result<wire::Holding, wire::Problem> {
    let domain::Holding { person, role } = value;
    wire::Holding::new(limits, wire::HoldingParts { person: *person, role: encode_role(*role, limits)? })
}
pub(super) fn decode_holding(value: wire::Holding) -> domain::Holding {
    let wire::HoldingParts { person, role } = value.into_parts();
    domain::Holding { person, role: decode_role(role) }
}

pub(super) fn encode_identity_key(
    value: &domain::IdentityKey,
    limits: &wire::Limits,
) -> Result<wire::IdentityKey, wire::Problem> {
    let domain::IdentityKey { provider, subject } = value;
    wire::IdentityKey::new(limits, wire::IdentityKeyParts { provider: *provider, subject: subject.clone() })
}
pub(super) fn decode_identity_key(value: wire::IdentityKey) -> domain::IdentityKey {
    let wire::IdentityKeyParts { provider, subject } = value.into_parts();
    domain::IdentityKey { provider, subject }
}

pub(super) fn encode_identity(
    value: &domain::Identity,
    limits: &wire::Limits,
) -> Result<wire::Identity, wire::Problem> {
    let domain::Identity { key, login, name } = value;
    wire::Identity::new(
        limits,
        wire::IdentityParts { key: encode_identity_key(key, limits)?, login: login.clone(), name: name.clone() },
    )
}
pub(super) fn decode_identity(value: wire::Identity) -> domain::Identity {
    let wire::IdentityParts { key, login, name } = value.into_parts();
    domain::Identity { key: decode_identity_key(key), login, name }
}

pub fn encode_request_key(
    value: &domain::RequestKey,
    limits: &wire::Limits,
) -> Result<wire::RequestKey, wire::Problem> {
    let domain::RequestKey { person, key } = value;
    wire::RequestKey::new(limits, wire::RequestKeyParts { person: *person, key: *key })
}
#[must_use]
pub fn decode_request_key(value: wire::RequestKey) -> domain::RequestKey {
    let wire::RequestKeyParts { person, key } = value.into_parts();
    domain::RequestKey { person, key }
}

fn encode_policy_value_roles(
    values: &[domain::PolicyRole],
    limits: &wire::Limits,
) -> Result<List<wire::PolicyRole>, wire::Problem> {
    let mut result = List::with_capacity(count(values.len())?);
    for value in values {
        result.push(encode_policy_role(value, limits)?).expect("same number of members");
    }
    Ok(result)
}
fn decode_policy_value_roles(
    values: List<wire::PolicyRole>,
    limits: &wire::Limits,
) -> Result<Box<[domain::PolicyRole]>, wire::Problem> {
    let mut result = List::with_capacity(values.len());
    for value in values.into_boxed() {
        result.push(decode_policy_role(value, limits)?).expect("same number of members");
    }
    Ok(result.into_boxed())
}

fn decode_grants(values: List<wire::Grant>, limits: &wire::Limits) -> Result<Box<[domain::Grant]>, wire::Problem> {
    let mut result = List::with_capacity(values.len());
    for value in values.into_boxed() {
        result.push(decode_grant(value, limits)?).expect("same number of members");
    }
    Ok(result.into_boxed())
}
fn encode_grants(values: &[domain::Grant], limits: &wire::Limits) -> Result<List<wire::Grant>, wire::Problem> {
    let mut result = List::with_capacity(count(values.len())?);
    for value in values {
        result.push(encode_grant(value, limits)?).expect("same number of members");
    }
    Ok(result)
}
fn decode_requirements(
    values: List<wire::Requirement>,
    limits: &wire::Limits,
) -> Result<Box<[domain::Requirement]>, wire::Problem> {
    let mut result = List::with_capacity(values.len());
    for value in values.into_boxed() {
        result.push(decode_requirement(value, limits)?).expect("same number of members");
    }
    Ok(result.into_boxed())
}
fn encode_requirements(
    values: &[domain::Requirement],
    limits: &wire::Limits,
) -> Result<List<wire::Requirement>, wire::Problem> {
    let mut result = List::with_capacity(count(values.len())?);
    for value in values {
        result.push(encode_requirement(value, limits)?).expect("same number of members");
    }
    Ok(result)
}

fn decode_holdings(values: List<wire::Holding>) -> Box<[domain::Holding]> {
    let mut result = List::with_capacity(values.len());
    for value in values.into_boxed() {
        result.push(decode_holding(value)).expect("same number of members");
    }
    result.into_boxed()
}
fn encode_holdings(values: &[domain::Holding], limits: &wire::Limits) -> Result<List<wire::Holding>, wire::Problem> {
    let mut result = List::with_capacity(count(values.len())?);
    for value in values {
        result.push(encode_holding(value, limits)?).expect("same number of members");
    }
    Ok(result)
}

fn derive_shared(domain: &core::Limits, limits: &mut wire::Limits) -> Option<()> {
    let people = &domain.people;
    let policy_bytes = u32::try_from(domain.policy_bytes).ok()?;
    limits.pattern_segments = piece_bound(
        people.amendment_bytes.max(policy_bytes),
        size_of::<Box<[u8]>>(),
        wire::Segment::worst_case_bytes(&EMPTY)?,
        true,
    )?;
    limits.delegation_kinds = piece_bound(
        people.amendment_bytes.max(policy_bytes),
        size_of::<domain::Executor>(),
        wire::Executor::worst_case_bytes(&EMPTY)?,
        false,
    )?;
    limits.authority_grants = piece_bound(
        people.amendment_bytes.max(policy_bytes),
        size_of::<domain::Grant>(),
        wire::Grant::worst_case_bytes(&EMPTY)?,
        true,
    )?;
    limits.authority_note_resources = piece_bound(
        people.amendment_bytes.max(policy_bytes),
        size_of::<domain::ResourceScope>(),
        wire::ResourceScope::worst_case_bytes(&EMPTY)?,
        true,
    )?;
    limits.policy_value_permissions = piece_bound(
        policy_bytes,
        size_of::<domain::PermissionRole>(),
        wire::PermissionRole::worst_case_bytes(&EMPTY)?,
        false,
    )?;
    Some(())
}

fn derive_amendment(domain: &core::Limits, limits: &mut wire::Limits) -> Option<()> {
    let people = &domain.people;
    limits.policy_change_requirements_requirements = piece_bound(
        people.amendment_bytes,
        size_of::<domain::Requirement>(),
        wire::Requirement::worst_case_bytes(&EMPTY)?,
        true,
    )?;
    limits.policy_change_projections_grants =
        piece_bound(people.amendment_bytes, size_of::<domain::Grant>(), wire::Grant::worst_case_bytes(&EMPTY)?, true)?;
    limits.policy_change_permissions_mappings = piece_bound(
        people.amendment_bytes,
        size_of::<domain::PermissionRole>(),
        wire::PermissionRole::worst_case_bytes(&EMPTY)?,
        false,
    )?;
    limits.note_change_correct_references =
        piece_bound(people.amendment_bytes, size_of::<u64>(), wire::Number64::worst_case_bytes(&EMPTY)?, false)?;
    limits.resource_name_path =
        piece_bound(people.amendment_bytes, size_of::<Box<[u8]>>(), wire::Segment::worst_case_bytes(&EMPTY)?, true)?;
    limits.adoption_options =
        piece_bound(people.amendment_bytes, size_of::<Box<[u8]>>(), wire::Segment::worst_case_bytes(&EMPTY)?, true)?;
    limits.spec_parameters = piece_bound(
        people.amendment_bytes,
        size_of::<domain::Parameter>(),
        wire::ParameterBytes::worst_case_bytes(&EMPTY)?.checked_add(
            wire::Parameter::worst_case_bytes(&EMPTY)?
                .checked_sub(wire::ParameterResource::worst_case_bytes(&EMPTY)?)?,
        )?,
        true,
    )?
    .max(piece_bound(
        people.amendment_bytes,
        size_of::<domain::Parameter>(),
        wire::Parameter::worst_case_bytes(&EMPTY)?,
        false,
    )?);
    limits.spec_inputs =
        piece_bound(people.amendment_bytes, size_of::<u64>(), wire::Number64::worst_case_bytes(&EMPTY)?, false)?;
    limits.amendment_dependencies_some =
        piece_bound(people.amendment_bytes, size_of::<u64>(), wire::Number64::worst_case_bytes(&EMPTY)?, false)?;
    Some(())
}

mod account_hosts;

/// Reuse the people family's refusal vocabulary inside a decided core call.
pub(crate) fn refusal_bytes(value: &domain::Refusal) -> Result<Box<[u8]>, wire::Problem> {
    let value = outcomes::encode_refusal(value, &wire::CEILINGS)?;
    let mut writer = Writer::new(usize::try_from(value.measure()).expect("refusal size"));
    value.encode(&mut writer).expect("measured refusal");
    Ok(writer.finish())
}
pub(crate) fn read_refusal(bytes: &[u8]) -> Result<domain::Refusal, wire::Problem> {
    Ok(outcomes::decode_refusal(wire::Refusal::decode(&wire::CEILINGS, &mut Reader::new(bytes))?))
}

#[cfg(test)]
mod envelope_tests {
    use super::{core, derived_limits, wire};

    // Inputs read by the people codec from configuration's small and standard profiles.
    // The config crate also tests these with the complete shipped profile constants.
    fn profile_inputs(standard: bool) -> core::Limits {
        let mut limits = crate::tests::fixtures::CORE_LIMITS;
        limits.policy_bytes = 16_384;
        limits.people.amendment_bytes = 8_192;
        limits.people.identity_bytes = 1_024;
        limits.people.words = 2_048;
        limits.people.program_host_name_bytes = 32;
        limits.authority.segment_bytes = 256;
        if standard {
            limits.templates = 16;
            limits.people.operations = 16;
            limits.people.batch = 4;
            limits.people.holdings = 32;
            limits.people.program_hosts = 65;
            limits.authority.grants = 8;
            limits.authority.roles = 8;
            limits.authority.requirements = 8;
            limits.tasks.circle = 32;
            limits.tasks.dependencies = 16;
        } else {
            limits.templates = 8;
            limits.people.operations = 8;
            limits.people.batch = 2;
            limits.people.holdings = 8;
            limits.people.program_hosts = 9;
            limits.authority.grants = 4;
            limits.authority.roles = 4;
            limits.authority.requirements = 4;
            limits.tasks.circle = 8;
            limits.tasks.dependencies = 8;
        }
        limits
    }

    #[test]
    fn shipped_profile_people_inputs_derive_within_the_ceilings() {
        for standard in [false, true] {
            let limits = profile_inputs(standard);
            let derived = derived_limits(&limits).unwrap();
            assert_eq!(wire::check_limits(&derived), Ok(()));
            assert_eq!(super::limits(&limits), Some(derived));
        }
    }

    #[test]
    fn rejected_policy_derivation_keeps_the_required_byte_bounds() {
        let mut limits = profile_inputs(false);
        limits.policy_bytes = 65_536;
        let derived = derived_limits(&limits).unwrap();
        assert_eq!(derived.policy_value_permissions, 98_304);
        assert_eq!(derived.delegation_kinds, 73_728);
        assert!(wire::check_limits(&derived).is_err());
        assert_eq!(super::limits(&limits), None);
    }

    #[test]
    fn unrepresentable_arithmetic_has_no_derived_limits() {
        let mut limits = profile_inputs(false);
        limits.policy_bytes = u64::MAX;
        assert_eq!(derived_limits(&limits), None);
    }
}
