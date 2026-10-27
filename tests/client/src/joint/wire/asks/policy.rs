//! Whole project-policy request fields on the typed peer.
use super::{authority, client, grant, pattern, people, request_arguments};

pub(super) fn policy_edit(
    value: &client::PolicyEdit,
    names: &mut skein_lib::List<Box<[u8]>>,
) -> Result<people::PolicyChange, super::Invalid> {
    Ok(match value {
        client::PolicyEdit::Templates { overrides } => {
            people::PolicyChange::Templates { overrides: overrides.iter().map(request_arguments::template).collect() }
        }
        client::PolicyEdit::RoleSpend { role, period_spend } => {
            people::PolicyChange::RoleSpend { role: *role, period_spend: *period_spend }
        }
        client::PolicyEdit::ProjectSpend { period_spend } => {
            people::PolicyChange::ProjectSpend { period_spend: *period_spend }
        }
        client::PolicyEdit::Role(role) => people::PolicyChange::Role(people::PolicyRole {
            number: role.number,
            authority: authority(&role.authority, names)?,
            period_spend: role.period_spend,
            requests: role.requests,
            decides: role.decides,
        }),
        client::PolicyEdit::Requirements { requirements } => people::PolicyChange::Requirements {
            requirements: requirements
                .iter()
                .map(|value| people::Requirement {
                    connector: value.connector,
                    kind: value.kind,
                    pattern: pattern(&value.pattern),
                    judge: people::Judge {
                        connector: value.judge.connector,
                        requirement: value.judge.requirement,
                        parameters: value.judge.parameters,
                    },
                    guard: match value.guard {
                        client::PolicyGuard::Guarded => people::Guard::Guarded,
                        client::PolicyGuard::Observed { freshness } => people::Guard::Observed { freshness },
                    },
                    must_be_guarded: value.must_be_guarded,
                })
                .collect(),
        },
        client::PolicyEdit::Projections { grants } => {
            people::PolicyChange::Projections { grants: grants.iter().map(grant).collect() }
        }
        client::PolicyEdit::Permissions { mappings } => people::PolicyChange::Permissions {
            mappings: mappings
                .iter()
                .map(|value| people::PermissionRole {
                    connector: value.connector,
                    permission: value.permission,
                    role: value.role,
                })
                .collect(),
        },
    })
}
