//! Raw retained New fields, without a clipped future-task projection.
use super::{historical::Invalid, owner};
use jig_client::DomainLimits as Limits;
use jig_client_domain::contract as client;
use jig_domain_tasks as tasks;
use skein_lib::List;

fn opaque(value: &[u8], limits: &Limits) -> Result<client::Bytes, owner::Invalid> {
    client::Bytes::new(owner::bytes(value, limits)?, limits.text).map_err(|_| owner::Invalid::TooLarge)
}
pub(super) fn text(value: &[u8], limits: &Limits) -> Result<client::Text, owner::Invalid> {
    client::Text::new(owner::bytes(value, limits)?, limits.text).map_err(|_| owner::Invalid::TooLarge)
}
fn list<T, U>(
    value: &[T],
    limits: &Limits,
    map: impl Fn(&T) -> Result<U, owner::Invalid>,
) -> Result<List<U>, owner::Invalid> {
    let capacity = u32::try_from(value.len()).map_err(|_| owner::Invalid::TooLarge)?;
    if capacity > limits.rows {
        return Err(owner::Invalid::TooLarge);
    }
    let mut result = List::with_capacity(capacity);
    for value in value {
        result.push(map(value)?).map_err(|_| owner::Invalid::TooLarge)?;
    }
    Ok(result)
}
fn pattern(value: &tasks::Pattern, limits: &Limits) -> Result<client::OwnerPattern, owner::Invalid> {
    Ok(client::OwnerPattern {
        segments: list(
            &value.segments,
            &Limits { rows: limits.segments, turn_text: limits.turn_text, ..*limits },
            |value| owner::bytes(value, limits),
        )?,
        last: match &value.last {
            tasks::Last::Exact(value) => client::OwnerPatternEnd::Exact(owner::bytes(value, limits)?),
            tasks::Last::Open(value) => client::OwnerPatternEnd::Open(owner::bytes(value, limits)?),
        },
    })
}
fn authority(
    value: &tasks::Authority,
    limits: &Limits,
    models: &[jig_domain_core::Model],
) -> Result<client::OwnerAuthority, owner::Invalid> {
    Ok(client::OwnerAuthority {
        models: Some(owner::models(value.models, models)?),
        tools: value.tools.0,
        grants: list(&value.grants, limits, |value| {
            Ok(client::OwnerGrant {
                connector: value.connector,
                kind: value.kind,
                pattern: pattern(&value.pattern, limits)?,
            })
        })?,
        delegation: client::OwnerDelegation {
            kinds: list(&value.delegation.kinds, limits, |value| {
                Ok(match value {
                    tasks::AuthorityExecutor::Charter(value) => client::OwnerAuthorityExecutor::Charter(*value),
                    tasks::AuthorityExecutor::Procedure(value) => client::OwnerAuthorityExecutor::Procedure(*value),
                    tasks::AuthorityExecutor::Role(value) => client::OwnerAuthorityExecutor::Role(*value),
                })
            })?,
            tasks: value.delegation.tasks,
            depth: value.delegation.depth,
        },
        spend: value.budget.spend,
        deadline: value.budget.deadline,
        notes: value.notes.0,
        note_resources: list(&value.note_resources, limits, |value| {
            Ok(client::OwnerResourceScope { connector: value.connector, pattern: pattern(&value.pattern, limits)? })
        })?,
    })
}
fn spec(value: &tasks::Spec, limits: &Limits) -> Result<client::Specification, owner::Invalid> {
    Ok(client::Specification {
        asks: false,
        words: text(&value.words, limits)?,
        parameters: list(&value.parameters, limits, |value| {
            Ok(match value {
                tasks::Parameter::Number { name, value } => client::Parameter::Number { name: *name, value: *value },
                tasks::Parameter::Bytes { name, value } => {
                    client::Parameter::Bytes { name: *name, value: text(value, limits)? }
                }
                tasks::Parameter::Resource { name, connector, resource } => {
                    client::Parameter::Resource { name: *name, connector: *connector, resource: *resource }
                }
            })
        })?,
        inputs: list(&value.inputs, limits, |value| Ok(*value))?,
    })
}
pub(super) fn contract(
    value: &tasks::Contract,
    limits: &Limits,
) -> Result<client::TaskContractDocument, owner::Invalid> {
    Ok(match value {
        tasks::Contract::Approval { state, part } => client::TaskContractDocument::Approval {
            state: owner::bytes(state, limits)?,
            part: owner::part(part, limits)?,
        },
        tasks::Contract::Report { words } => client::TaskContractDocument::Report { words: *words },
        tasks::Contract::Verdict { choices } => client::TaskContractDocument::Verdict {
            choices: list(choices, limits, |value| {
                Ok(client::VerdictChoice {
                    code: value.code,
                    words: value.words,
                    followups: value.followups,
                    fields: list(&value.fields, limits, |value| {
                        Ok(client::FieldRule { name: owner::bytes(&value.name, limits)?, max: value.max })
                    })?,
                })
            })?,
            // New retains the whole admitted closed list, never a source cut.
            cut: false,
        },
        tasks::Contract::Change { connector, kind, words } => {
            client::TaskContractDocument::Change { connector: *connector, kind: *kind, words: *words }
        }
    })
}
pub(super) fn goal(
    value: &tasks::New,
    limits: &Limits,
    models: &[jig_domain_core::Model],
) -> Result<client::OwnerPersonProposalGoal, Invalid> {
    if value.recurring.is_some() {
        return Err(Invalid::Recurrence);
    }
    Ok(client::OwnerPersonProposalGoal {
        number: value.number,
        project: value.project,
        executor: owner::executor(value.executor),
        spec: spec(&value.spec, limits).map_err(Invalid::Owner)?,
        contract: contract(&value.contract, limits).map_err(Invalid::Owner)?,
        authority: authority(&value.authority, limits, models).map_err(Invalid::Owner)?,
        numbers: owner::numbers(value.numbers),
        funder: owner::funder(value.funder),
        dependencies: list(&value.dependencies, limits, |value| Ok(*value)).map_err(Invalid::Owner)?,
        holdings: list(&value.holdings, limits, |value| {
            let (name, kind) = match value {
                tasks::Holding::Write { resource, kind } => (resource, *kind),
                tasks::Holding::Slot { pool, kind } => (pool, *kind),
            };
            let connector = name.connector;
            let path =
                list(&name.path, &Limits { rows: limits.segments, turn_text: limits.turn_text, ..*limits }, |value| {
                    opaque(value, limits)
                })?;
            Ok(match value {
                tasks::Holding::Write { .. } => client::OwnerGoalHolding::Write { connector, path, kind },
                tasks::Holding::Slot { .. } => client::OwnerGoalHolding::Slot { connector, path, kind },
            })
        })
        .map_err(Invalid::Owner)?,
        wake: owner::wake(value.wake),
        tracked: value.tracked,
    })
}
