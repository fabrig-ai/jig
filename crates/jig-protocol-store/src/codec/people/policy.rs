//! Policy values retained by people (protocol/store.md, 5.1).
use super::{
    Duration, Wall, decode_authority_note_resources, decode_delegation_kinds, decode_grant_piece, decode_grants,
    decode_numbers, decode_permissions, decode_policy_value_roles, decode_requirement_piece, decode_requirements,
    decode_segments, decode_spec_parameters, domain, encode_authority_note_resources, encode_delegation_kinds,
    encode_grant_piece, encode_grants, encode_numbers, encode_permissions, encode_policy_value_roles,
    encode_requirement_piece, encode_requirements, encode_segments, encode_spec_parameters, permission_sum, shared_sum,
    wire,
};
use alloc::boxed::Box;
pub(super) fn encode_last(value: &domain::Last, limits: &wire::Limits) -> Result<wire::Last, wire::Problem> {
    Ok(match value {
        domain::Last::Exact(value) => {
            wire::Last::Exact(wire::LastExact::new(limits, wire::LastExactParts { value: value.clone() })?)
        }
        domain::Last::Open(value) => {
            wire::Last::Open(wire::LastOpen::new(limits, wire::LastOpenParts { value: value.clone() })?)
        }
    })
}
pub(super) fn decode_last(value: wire::Last) -> domain::Last {
    match value {
        wire::Last::Exact(value) => {
            let wire::LastExactParts { value } = value.into_parts();
            domain::Last::Exact(value)
        }
        wire::Last::Open(value) => {
            let wire::LastOpenParts { value } = value.into_parts();
            domain::Last::Open(value)
        }
    }
}

pub(crate) fn encode_pattern(value: &domain::Pattern, limits: &wire::Limits) -> Result<wire::Pattern, wire::Problem> {
    let domain::Pattern { segments, last } = value;
    wire::Pattern::new(
        limits,
        wire::PatternParts {
            segments: encode_segments(segments, limits, shared_sum(limits), limits.pattern_segments)?,
            last: encode_last(last, limits)?,
        },
    )
}
pub(crate) fn decode_pattern(value: wire::Pattern, limits: &wire::Limits) -> Result<domain::Pattern, wire::Problem> {
    let wire::PatternParts { segments, last } = value.into_parts();
    Ok(domain::Pattern { segments: decode_segments(segments, limits, shared_sum(limits))?, last: decode_last(last) })
}

pub(super) fn encode_grant(value: &domain::Grant, limits: &wire::Limits) -> Result<wire::Grant, wire::Problem> {
    let domain::Grant { connector, kind, pattern } = value;
    wire::Grant::new(
        limits,
        wire::GrantParts { connector: *connector, kind: *kind, pattern: encode_pattern(pattern, limits)? },
    )
}
pub(super) fn decode_grant(value: wire::Grant, limits: &wire::Limits) -> Result<domain::Grant, wire::Problem> {
    let wire::GrantParts { connector, kind, pattern } = value.into_parts();
    Ok(domain::Grant { connector, kind, pattern: decode_pattern(pattern, limits)? })
}

pub(super) fn encode_resource_scope(
    value: &domain::ResourceScope,
    limits: &wire::Limits,
) -> Result<wire::ResourceScope, wire::Problem> {
    let domain::ResourceScope { connector, pattern } = value;
    wire::ResourceScope::new(
        limits,
        wire::ResourceScopeParts { connector: *connector, pattern: encode_pattern(pattern, limits)? },
    )
}
pub(super) fn decode_resource_scope(
    value: wire::ResourceScope,
    limits: &wire::Limits,
) -> Result<domain::ResourceScope, wire::Problem> {
    let wire::ResourceScopeParts { connector, pattern } = value.into_parts();
    Ok(domain::ResourceScope { connector, pattern: decode_pattern(pattern, limits)? })
}

pub(super) fn encode_executor(value: domain::Executor, limits: &wire::Limits) -> Result<wire::Executor, wire::Problem> {
    Ok(match value {
        domain::Executor::Charter(value) => {
            wire::Executor::Charter(wire::ExecutorCharter::new(limits, wire::ExecutorCharterParts { value })?)
        }
        domain::Executor::Procedure(value) => {
            wire::Executor::Procedure(wire::ExecutorProcedure::new(limits, wire::ExecutorProcedureParts { value })?)
        }
        domain::Executor::Role(value) => {
            wire::Executor::Role(wire::ExecutorRole::new(limits, wire::ExecutorRoleParts { value })?)
        }
    })
}
pub(super) fn decode_executor(value: wire::Executor) -> domain::Executor {
    match value {
        wire::Executor::Charter(value) => {
            let wire::ExecutorCharterParts { value } = value.into_parts();
            domain::Executor::Charter(value)
        }
        wire::Executor::Procedure(value) => {
            let wire::ExecutorProcedureParts { value } = value.into_parts();
            domain::Executor::Procedure(value)
        }
        wire::Executor::Role(value) => {
            let wire::ExecutorRoleParts { value } = value.into_parts();
            domain::Executor::Role(value)
        }
    }
}

pub(super) fn encode_delegation(
    value: &domain::Delegation,
    limits: &wire::Limits,
) -> Result<wire::Delegation, wire::Problem> {
    let domain::Delegation { kinds, tasks, depth } = value;
    wire::Delegation::new(
        limits,
        wire::DelegationParts {
            kinds: encode_delegation_kinds(kinds, limits, shared_sum(limits), limits.delegation_kinds)?,
            tasks: *tasks,
            depth: *depth,
        },
    )
}
pub(super) fn decode_delegation(
    value: wire::Delegation,
    limits: &wire::Limits,
) -> Result<domain::Delegation, wire::Problem> {
    let wire::DelegationParts { kinds, tasks, depth } = value.into_parts();
    Ok(domain::Delegation { kinds: decode_delegation_kinds(kinds, limits, shared_sum(limits))?, tasks, depth })
}

#[expect(clippy::manual_map, reason = "step code translates optional fields through exhaustive matches")]
pub(super) fn encode_authority(
    value: &domain::Authority,
    limits: &wire::Limits,
) -> Result<wire::Authority, wire::Problem> {
    let domain::Authority { tools, models, grants, delegation, spend, deadline, notes, note_resources } = value;
    wire::Authority::new(
        limits,
        wire::AuthorityParts {
            tools: *tools,
            models: match models {
                Some(domain::Models::All) => Some(wire::ModelSet::All),
                Some(domain::Models::Named(bits)) => Some(wire::ModelSet::Named(wire::ModelSetNamed::new(
                    limits,
                    wire::ModelSetNamedParts { bits: *bits },
                )?)),
                None => None,
            },
            grants: encode_grant_piece(grants, limits, shared_sum(limits), limits.authority_grants)?,
            delegation: encode_delegation(delegation, limits)?,
            spend: *spend,
            deadline: match deadline {
                Some(value) => Some(value.as_nanos()),
                None => None,
            },
            notes: *notes,
            note_resources: encode_authority_note_resources(
                note_resources,
                limits,
                shared_sum(limits),
                limits.authority_note_resources,
            )?,
        },
    )
}
#[expect(clippy::manual_map, reason = "step code translates optional fields through exhaustive matches")]
pub(super) fn decode_authority(
    value: wire::Authority,
    limits: &wire::Limits,
) -> Result<domain::Authority, wire::Problem> {
    let wire::AuthorityParts { tools, models, grants, delegation, spend, deadline, notes, note_resources } =
        value.into_parts();
    Ok(domain::Authority {
        tools,
        models: match models {
            Some(wire::ModelSet::All) => Some(domain::Models::All),
            Some(wire::ModelSet::Named(value)) => Some(domain::Models::Named(value.bits())),
            None => None,
        },
        grants: decode_grant_piece(grants, limits, shared_sum(limits))?,
        delegation: decode_delegation(delegation, limits)?,
        spend,
        deadline: match deadline {
            Some(value) => Some(Wall::from_nanos(value)),
            None => None,
        },
        notes,
        note_resources: decode_authority_note_resources(note_resources, limits, shared_sum(limits))?,
    })
}

pub(super) fn encode_judge(value: domain::Judge, limits: &wire::Limits) -> Result<wire::Judge, wire::Problem> {
    let domain::Judge { connector, requirement, parameters } = value;
    wire::Judge::new(limits, wire::JudgeParts { connector, requirement, parameters })
}
pub(super) fn decode_judge(value: wire::Judge) -> domain::Judge {
    let wire::JudgeParts { connector, requirement, parameters } = value.into_parts();
    domain::Judge { connector, requirement, parameters }
}

pub(super) fn encode_guard(value: &domain::Guard, limits: &wire::Limits) -> Result<wire::Guard, wire::Problem> {
    Ok(match value {
        domain::Guard::Guarded => wire::Guard::Guarded,
        domain::Guard::Observed { freshness } => wire::Guard::Observed(wire::GuardObserved::new(
            limits,
            wire::GuardObservedParts { freshness: freshness.as_nanos() },
        )?),
    })
}
pub(super) fn decode_guard(value: wire::Guard) -> domain::Guard {
    match value {
        wire::Guard::Guarded => domain::Guard::Guarded,
        wire::Guard::Observed(value) => {
            let wire::GuardObservedParts { freshness } = value.into_parts();
            domain::Guard::Observed { freshness: Duration::from_nanos(freshness) }
        }
    }
}

pub(super) fn encode_requirement(
    value: &domain::Requirement,
    limits: &wire::Limits,
) -> Result<wire::Requirement, wire::Problem> {
    let domain::Requirement { connector, kind, pattern, judge, guard, must_be_guarded } = value;
    wire::Requirement::new(
        limits,
        wire::RequirementParts {
            connector: *connector,
            kind: *kind,
            pattern: encode_pattern(pattern, limits)?,
            judge: encode_judge(*judge, limits)?,
            guard: encode_guard(guard, limits)?,
            must_be_guarded: *must_be_guarded,
        },
    )
}
pub(super) fn decode_requirement(
    value: wire::Requirement,
    limits: &wire::Limits,
) -> Result<domain::Requirement, wire::Problem> {
    let wire::RequirementParts { connector, kind, pattern, judge, guard, must_be_guarded } = value.into_parts();
    Ok(domain::Requirement {
        connector,
        kind,
        pattern: decode_pattern(pattern, limits)?,
        judge: decode_judge(judge),
        guard: decode_guard(guard),
        must_be_guarded,
    })
}

pub(super) fn encode_permission_role(
    value: domain::PermissionRole,
    limits: &wire::Limits,
) -> Result<wire::PermissionRole, wire::Problem> {
    let domain::PermissionRole { connector, permission, role } = value;
    wire::PermissionRole::new(limits, wire::PermissionRoleParts { connector, permission, role })
}
pub(super) fn decode_permission_role(value: wire::PermissionRole) -> domain::PermissionRole {
    let wire::PermissionRoleParts { connector, permission, role } = value.into_parts();
    domain::PermissionRole { connector, permission, role }
}

pub(super) fn encode_policy_role(
    value: &domain::PolicyRole,
    limits: &wire::Limits,
) -> Result<wire::PolicyRole, wire::Problem> {
    let domain::PolicyRole { number, authority, period_spend, requests, decides } = value;
    wire::PolicyRole::new(
        limits,
        wire::PolicyRoleParts {
            number: *number,
            authority: encode_authority(authority, limits)?,
            period_spend: *period_spend,
            requests: *requests,
            decides: *decides,
        },
    )
}
pub(super) fn decode_policy_role(
    value: wire::PolicyRole,
    limits: &wire::Limits,
) -> Result<domain::PolicyRole, wire::Problem> {
    let wire::PolicyRoleParts { number, authority, period_spend, requests, decides } = value.into_parts();
    Ok(domain::PolicyRole { number, authority: decode_authority(authority, limits)?, period_spend, requests, decides })
}

pub(super) fn encode_policy_value(
    value: &domain::PolicyValue,
    limits: &wire::Limits,
) -> Result<wire::PolicyValue, wire::Problem> {
    let domain::PolicyValue {
        ceiling,
        escalation_role,
        period_spend,
        projections,
        roles,
        requirements,
        permissions,
        templates,
    } = value;
    wire::PolicyValue::new(
        limits,
        wire::PolicyValueParts {
            ceiling: encode_authority(ceiling, limits)?,
            escalation_role: *escalation_role,
            period_spend: *period_spend,
            templates: encode_templates(templates, limits, limits.policy_value_templates)?,
            projections: encode_grants(projections, limits)?,
            roles: encode_policy_value_roles(roles, limits)?,
            requirements: encode_requirements(requirements, limits)?,
            permissions: encode_permissions(
                permissions,
                limits,
                permission_sum(limits),
                limits.policy_value_permissions,
            )?,
        },
    )
}
pub(super) fn decode_policy_value(
    value: wire::PolicyValue,
    limits: &wire::Limits,
) -> Result<domain::PolicyValue, wire::Problem> {
    let wire::PolicyValueParts {
        ceiling,
        escalation_role,
        period_spend,
        projections,
        roles,
        requirements,
        permissions,
        templates,
    } = value.into_parts();
    Ok(domain::PolicyValue {
        ceiling: decode_authority(ceiling, limits)?,
        escalation_role,
        templates: decode_templates(templates),
        period_spend,
        projections: decode_grants(projections, limits)?,
        roles: decode_policy_value_roles(roles, limits)?,
        requirements: decode_requirements(requirements, limits)?,
        permissions: decode_permissions(permissions, limits, permission_sum(limits))?,
    })
}

pub(super) fn encode_policy_change(
    value: &domain::PolicyChange,
    limits: &wire::Limits,
) -> Result<wire::PolicyChange, wire::Problem> {
    Ok(match value {
        domain::PolicyChange::Templates { overrides } => {
            wire::PolicyChange::Templates(wire::PolicyChangeTemplates::new(
                limits,
                wire::PolicyChangeTemplatesParts {
                    overrides: encode_templates(overrides, limits, limits.policy_change_templates_overrides)?,
                },
            )?)
        }
        domain::PolicyChange::RoleSpend { role, period_spend } => {
            wire::PolicyChange::RoleSpend(wire::PolicyChangeRoleSpend::new(
                limits,
                wire::PolicyChangeRoleSpendParts { role: *role, period_spend: *period_spend },
            )?)
        }
        domain::PolicyChange::ProjectSpend { period_spend } => {
            wire::PolicyChange::ProjectSpend(wire::PolicyChangeProjectSpend::new(
                limits,
                wire::PolicyChangeProjectSpendParts { period_spend: *period_spend },
            )?)
        }
        domain::PolicyChange::Role(value) => wire::PolicyChange::Role(wire::PolicyChangeRole::new(
            limits,
            wire::PolicyChangeRoleParts { value: encode_policy_role(value, limits)? },
        )?),
        domain::PolicyChange::Requirements { requirements } => {
            wire::PolicyChange::Requirements(wire::PolicyChangeRequirements::new(
                limits,
                wire::PolicyChangeRequirementsParts {
                    requirements: encode_requirement_piece(
                        requirements,
                        limits,
                        limits.parameter_bytes_value,
                        limits.policy_change_requirements_requirements,
                    )?,
                },
            )?)
        }
        domain::PolicyChange::Projections { grants } => {
            wire::PolicyChange::Projections(wire::PolicyChangeProjections::new(
                limits,
                wire::PolicyChangeProjectionsParts {
                    grants: encode_grant_piece(
                        grants,
                        limits,
                        limits.parameter_bytes_value,
                        limits.policy_change_projections_grants,
                    )?,
                },
            )?)
        }
        domain::PolicyChange::Permissions { mappings } => {
            wire::PolicyChange::Permissions(wire::PolicyChangePermissions::new(
                limits,
                wire::PolicyChangePermissionsParts {
                    mappings: encode_permissions(
                        mappings,
                        limits,
                        limits.parameter_bytes_value,
                        limits.policy_change_permissions_mappings,
                    )?,
                },
            )?)
        }
    })
}
pub(super) fn decode_policy_change(
    value: wire::PolicyChange,
    limits: &wire::Limits,
) -> Result<domain::PolicyChange, wire::Problem> {
    Ok(match value {
        wire::PolicyChange::Templates(value) => {
            domain::PolicyChange::Templates { overrides: decode_templates(value.into_parts().overrides) }
        }
        wire::PolicyChange::RoleSpend(value) => {
            let wire::PolicyChangeRoleSpendParts { role, period_spend } = value.into_parts();
            domain::PolicyChange::RoleSpend { role, period_spend }
        }
        wire::PolicyChange::ProjectSpend(value) => {
            let wire::PolicyChangeProjectSpendParts { period_spend } = value.into_parts();
            domain::PolicyChange::ProjectSpend { period_spend }
        }
        wire::PolicyChange::Role(value) => {
            let wire::PolicyChangeRoleParts { value } = value.into_parts();
            domain::PolicyChange::Role(decode_policy_role(value, limits)?)
        }
        wire::PolicyChange::Requirements(value) => {
            let wire::PolicyChangeRequirementsParts { requirements } = value.into_parts();
            domain::PolicyChange::Requirements {
                requirements: decode_requirement_piece(requirements, limits, limits.parameter_bytes_value)?,
            }
        }
        wire::PolicyChange::Projections(value) => {
            let wire::PolicyChangeProjectionsParts { grants } = value.into_parts();
            domain::PolicyChange::Projections {
                grants: decode_grant_piece(grants, limits, limits.parameter_bytes_value)?,
            }
        }
        wire::PolicyChange::Permissions(value) => {
            let wire::PolicyChangePermissionsParts { mappings } = value.into_parts();
            domain::PolicyChange::Permissions {
                mappings: decode_permissions(mappings, limits, limits.parameter_bytes_value)?,
            }
        }
    })
}

pub(super) fn encode_person_address(
    value: &domain::PersonAddress,
    limits: &wire::Limits,
) -> Result<wire::PersonAddress, wire::Problem> {
    Ok(match value {
        domain::PersonAddress::Person(value) => wire::PersonAddress::Person(wire::PersonAddressPerson::new(
            limits,
            wire::PersonAddressPersonParts { value: *value },
        )?),
        domain::PersonAddress::Role(value) => wire::PersonAddress::Role(wire::PersonAddressRole::new(
            limits,
            wire::PersonAddressRoleParts { value: *value },
        )?),
    })
}
pub(super) fn decode_person_address(value: wire::PersonAddress) -> domain::PersonAddress {
    match value {
        wire::PersonAddress::Person(value) => {
            let wire::PersonAddressPersonParts { value } = value.into_parts();
            domain::PersonAddress::Person(value)
        }
        wire::PersonAddress::Role(value) => {
            let wire::PersonAddressRoleParts { value } = value.into_parts();
            domain::PersonAddress::Role(value)
        }
    }
}

pub(super) fn encode_address_change(
    value: &domain::AddressChange,
    limits: &wire::Limits,
) -> Result<wire::AddressChange, wire::Problem> {
    let domain::AddressChange { revision, address } = value;
    wire::AddressChange::new(
        limits,
        wire::AddressChangeParts { revision: *revision, address: encode_person_address(address, limits)? },
    )
}
pub(super) fn decode_address_change(value: wire::AddressChange) -> domain::AddressChange {
    let wire::AddressChangeParts { revision, address } = value.into_parts();
    domain::AddressChange { revision, address: decode_person_address(address) }
}

pub(super) fn encode_parameter(
    value: &domain::Parameter,
    limits: &wire::Limits,
) -> Result<wire::Parameter, wire::Problem> {
    Ok(match value {
        domain::Parameter::Number { name, value } => wire::Parameter::Number(wire::ParameterNumber::new(
            limits,
            wire::ParameterNumberParts { name: *name, value: *value },
        )?),
        domain::Parameter::Bytes { name, value } => wire::Parameter::Bytes(wire::ParameterBytes::new(
            limits,
            wire::ParameterBytesParts { name: *name, value: value.clone() },
        )?),
        domain::Parameter::Resource { name, connector, resource } => {
            wire::Parameter::Resource(wire::ParameterResource::new(
                limits,
                wire::ParameterResourceParts { name: *name, connector: *connector, resource: *resource },
            )?)
        }
    })
}
pub(super) fn decode_parameter(value: wire::Parameter) -> domain::Parameter {
    match value {
        wire::Parameter::Number(value) => {
            let wire::ParameterNumberParts { name, value } = value.into_parts();
            domain::Parameter::Number { name, value }
        }
        wire::Parameter::Bytes(value) => {
            let wire::ParameterBytesParts { name, value } = value.into_parts();
            domain::Parameter::Bytes { name, value }
        }
        wire::Parameter::Resource(value) => {
            let wire::ParameterResourceParts { name, connector, resource } = value.into_parts();
            domain::Parameter::Resource { name, connector, resource }
        }
    }
}

pub(super) fn encode_spec(value: &domain::Spec, limits: &wire::Limits) -> Result<wire::Spec, wire::Problem> {
    let domain::Spec { words, parameters, inputs } = value;
    wire::Spec::new(
        limits,
        wire::SpecParts {
            words: words.clone(),
            parameters: encode_spec_parameters(
                parameters,
                limits,
                limits.parameter_bytes_value,
                limits.spec_parameters,
            )?,
            inputs: encode_numbers(inputs, limits, limits.parameter_bytes_value, limits.spec_inputs)?,
        },
    )
}
pub(super) fn decode_spec(value: wire::Spec, limits: &wire::Limits) -> Result<domain::Spec, wire::Problem> {
    let wire::SpecParts { words, parameters, inputs } = value.into_parts();
    Ok(domain::Spec {
        words,
        parameters: decode_spec_parameters(parameters, limits, limits.parameter_bytes_value)?,
        inputs: decode_numbers(inputs, limits, limits.parameter_bytes_value)?,
    })
}

pub(super) fn encode_wake_rule(
    value: &domain::WakeRule,
    limits: &wire::Limits,
) -> Result<wire::WakeRule, wire::Problem> {
    Ok(match value {
        domain::WakeRule::Never => wire::WakeRule::Never,
        domain::WakeRule::Immediate => wire::WakeRule::Immediate,
        domain::WakeRule::Batch { count, age } => wire::WakeRule::Batch(wire::WakeRuleBatch::new(
            limits,
            wire::WakeRuleBatchParts { count: *count, age: age.as_nanos() },
        )?),
    })
}
pub(super) fn decode_wake_rule(value: wire::WakeRule) -> domain::WakeRule {
    match value {
        wire::WakeRule::Never => domain::WakeRule::Never,
        wire::WakeRule::Immediate => domain::WakeRule::Immediate,
        wire::WakeRule::Batch(value) => {
            let wire::WakeRuleBatchParts { count, age } = value.into_parts();
            domain::WakeRule::Batch { count, age: Duration::from_nanos(age) }
        }
    }
}

pub(super) fn encode_results_wake(value: domain::ResultsWake, _limits: &wire::Limits) -> wire::ResultsWake {
    match value {
        domain::ResultsWake::Never => wire::ResultsWake::Never,
        domain::ResultsWake::Each => wire::ResultsWake::Each,
        domain::ResultsWake::LastOrFailure => wire::ResultsWake::LastOrFailure,
    }
}
pub(super) fn decode_results_wake(value: wire::ResultsWake) -> domain::ResultsWake {
    match value {
        wire::ResultsWake::Never => domain::ResultsWake::Never,
        wire::ResultsWake::Each => domain::ResultsWake::Each,
        wire::ResultsWake::LastOrFailure => domain::ResultsWake::LastOrFailure,
    }
}

pub(super) fn encode_wake_policy(
    value: &domain::WakePolicy,
    limits: &wire::Limits,
) -> Result<wire::WakePolicy, wire::Problem> {
    let domain::WakePolicy { markers, words, notices, news, results, questions, answers, timers } = value;
    wire::WakePolicy::new(
        limits,
        wire::WakePolicyParts {
            markers: *markers,
            words: encode_wake_rule(words, limits)?,
            notices: encode_wake_rule(notices, limits)?,
            news: encode_wake_rule(news, limits)?,
            results: encode_results_wake(*results, limits),
            questions: *questions,
            answers: *answers,
            timers: *timers,
        },
    )
}
pub(super) fn decode_wake_policy(value: wire::WakePolicy) -> domain::WakePolicy {
    let wire::WakePolicyParts { markers, words, notices, news, results, questions, answers, timers } =
        value.into_parts();
    domain::WakePolicy {
        markers,
        words: decode_wake_rule(words),
        notices: decode_wake_rule(notices),
        news: decode_wake_rule(news),
        results: decode_results_wake(results),
        questions,
        answers,
        timers,
    }
}

pub(crate) fn encode_amendment(
    value: &domain::Amendment,
    limits: &wire::Limits,
) -> Result<wire::Amendment, wire::Problem> {
    check_amendment(value, limits)?;
    let domain::Amendment { tracked, addressed, instructions, spec, wake, dependencies, authority, llm, reason } =
        value;
    wire::Amendment::new(
        limits,
        wire::AmendmentParts {
            llm: super::llm::encode_llm(llm.as_ref(), limits)?,
            tracked: match tracked {
                Some(domain::Tracking::Tracked { priority }) => Some(wire::Tracking::Tracked(
                    wire::TrackingTracked::new(limits, wire::TrackingTrackedParts { priority: *priority })?,
                )),
                Some(domain::Tracking::Untracked) => Some(wire::Tracking::Untracked),
                None => None,
            },
            addressed: match addressed {
                Some(value) => Some(encode_address_change(value, limits)?),
                None => None,
            },
            instructions: instructions.clone(),
            spec: match spec {
                Some(value) => Some(encode_spec(value, limits)?),
                None => None,
            },
            wake: match wake {
                Some(value) => Some(encode_wake_policy(value, limits)?),
                None => None,
            },
            dependencies: match dependencies {
                Some(value) => Some(encode_numbers(
                    value,
                    limits,
                    limits.parameter_bytes_value,
                    limits.amendment_dependencies_some,
                )?),
                None => None,
            },
            authority: match authority {
                Some(value) => Some(encode_authority(value, limits)?),
                None => None,
            },
            reason: reason.clone(),
        },
    )
}
#[expect(clippy::manual_map, reason = "step code translates optional fields through exhaustive matches")]
pub(crate) fn decode_amendment(
    value: wire::Amendment,
    limits: &wire::Limits,
) -> Result<domain::Amendment, wire::Problem> {
    let wire::AmendmentParts { tracked, addressed, instructions, spec, wake, dependencies, authority, llm, reason } =
        value.into_parts();
    let value = domain::Amendment {
        llm: super::llm::decode_llm(llm),
        tracked: match tracked {
            Some(wire::Tracking::Tracked(value)) => {
                Some(domain::Tracking::Tracked { priority: value.into_parts().priority })
            }
            Some(wire::Tracking::Untracked) => Some(domain::Tracking::Untracked),
            None => None,
        },
        addressed: match addressed {
            Some(value) => Some(decode_address_change(value)),
            None => None,
        },
        instructions,
        spec: match spec {
            Some(value) => Some(decode_spec(value, limits)?),
            None => None,
        },
        wake: match wake {
            Some(value) => Some(decode_wake_policy(value)),
            None => None,
        },
        dependencies: match dependencies {
            Some(value) => Some(decode_numbers(value, limits, limits.parameter_bytes_value)?),
            None => None,
        },
        authority: match authority {
            Some(value) => Some(decode_authority(value, limits)?),
            None => None,
        },
        reason,
    };
    check_amendment(&value, limits)?;
    Ok(value)
}

fn check_amendment(value: &domain::Amendment, limits: &wire::Limits) -> Result<(), wire::Problem> {
    match domain::amendment_bytes(value) {
        Some(bytes) if bytes <= u64::from(limits.parameter_bytes_value) => Ok(()),
        Some(_) | None => Err(super::problem()),
    }
}

fn encode_templates(
    entries: &[domain::TemplateOverride],
    limits: &wire::Limits,
    bound: u32,
) -> Result<skein_lib::List<wire::TemplateOverride>, wire::Problem> {
    let mut values = skein_lib::List::with_capacity(bound);
    for entry in entries {
        let executor = match entry.executor {
            Some(domain::TemplateExecutor::Charter(value)) => Some(wire::TemplateExecutor::Charter(
                wire::TemplateExecutorCharter::new(limits, wire::TemplateExecutorCharterParts { value })?,
            )),
            Some(domain::TemplateExecutor::Chosen) => Some(wire::TemplateExecutor::Chosen),
            None => None,
        };
        let budget = match entry.budget {
            Some(domain::TemplateBudget::Amount(value)) => Some(wire::TemplateBudget::Amount(
                wire::TemplateBudgetAmount::new(limits, wire::TemplateBudgetAmountParts { value })?,
            )),
            Some(domain::TemplateBudget::AsAllowed { cap }) => Some(wire::TemplateBudget::AsAllowed(
                wire::TemplateBudgetAsAllowed::new(limits, wire::TemplateBudgetAsAllowedParts { cap })?,
            )),
            Some(domain::TemplateBudget::Given) => Some(wire::TemplateBudget::Given),
            None => None,
        };
        let deadline = match entry.deadline {
            Some(domain::TemplateDeadline::None) => Some(wire::TemplateDeadline::None),
            Some(domain::TemplateDeadline::Span(value)) => Some(wire::TemplateDeadline::Span(
                wire::TemplateDeadlineSpan::new(limits, wire::TemplateDeadlineSpanParts { value: value.as_nanos() })?,
            )),
            Some(domain::TemplateDeadline::Given) => Some(wire::TemplateDeadline::Given),
            None => None,
        };
        if values
            .push(wire::TemplateOverride::new(
                limits,
                wire::TemplateOverrideParts {
                    template: entry.template,
                    executor,
                    budget,
                    deadline,
                    priority: entry.priority,
                },
            )?)
            .is_err()
        {
            return Err(super::problem());
        }
    }
    Ok(values)
}
fn decode_templates(entries: skein_lib::List<wire::TemplateOverride>) -> Box<[domain::TemplateOverride]> {
    let mut values = skein_lib::List::with_capacity(entries.len());
    for entry in entries.into_boxed() {
        let wire::TemplateOverrideParts { template, executor, budget, deadline, priority } = entry.into_parts();
        let executor = match executor {
            Some(wire::TemplateExecutor::Charter(value)) => {
                Some(domain::TemplateExecutor::Charter(value.into_parts().value))
            }
            Some(wire::TemplateExecutor::Chosen) => Some(domain::TemplateExecutor::Chosen),
            None => None,
        };
        let budget = match budget {
            Some(wire::TemplateBudget::Amount(value)) => Some(domain::TemplateBudget::Amount(value.into_parts().value)),
            Some(wire::TemplateBudget::AsAllowed(value)) => {
                Some(domain::TemplateBudget::AsAllowed { cap: value.into_parts().cap })
            }
            Some(wire::TemplateBudget::Given) => Some(domain::TemplateBudget::Given),
            None => None,
        };
        let deadline = match deadline {
            Some(wire::TemplateDeadline::None) => Some(domain::TemplateDeadline::None),
            Some(wire::TemplateDeadline::Span(value)) => {
                Some(domain::TemplateDeadline::Span(Duration::from_nanos(value.into_parts().value)))
            }
            Some(wire::TemplateDeadline::Given) => Some(domain::TemplateDeadline::Given),
            None => None,
        };
        values
            .push(domain::TemplateOverride { template, executor, budget, deadline, priority })
            .expect("decoded overrides bounded");
    }
    values.into_boxed()
}
