//! Whole settings asks into core values (client/domain/contract.md, 3.2).
#![expect(clippy::manual_map, reason = "step code translates optional values with matches")]
use super::super::{Untranslatable, asks, operations, w};
use alloc::boxed::Box;
use jig_domain_authority as a;
use jig_domain_core::{self as c, settings as s};
use jig_domain_people as p;
use jig_domain_tasks as t;
use skein_lib::{List, Reader};
fn bad() -> Untranslatable {
    Untranslatable { path: w::Path::ChangeSettingsItems }
}
pub(crate) fn change(
    value: w::ChangeSettings,
    models: &[crate::Model],
    limits: &w::Limits,
) -> Result<s::Change, Untranslatable> {
    let v = value.into_parts();
    let count = asks::count(&v.items, w::Path::ChangeSettingsItems)?;
    let mut items = List::with_capacity(count);
    let mut reader = Reader::new(&v.items);
    for _ in 0..count {
        let bytes = asks::item(&mut reader, w::Path::ChangeSettingsItems)?;
        let mut piece = Reader::new(&bytes);
        let item = match w::SettingItem::decode(limits, &mut piece) {
            Ok(value) => value,
            Err(problem) => return Err(map_problem(problem)),
        };
        if !piece.is_empty() {
            return Err(bad());
        }
        let item = item.into_parts();
        items
            .push(s::Item {
                setting: setting(item.setting),
                edit: match item.edit {
                    w::SettingEdit::Removed => s::Edit::Removed,
                    w::SettingEdit::Reset => s::Edit::Reset,
                    w::SettingEdit::Set(value) => {
                        s::Edit::Set(setting_value(value.into_parts().value, models, limits)?)
                    }
                },
            })
            .expect("counted pieces");
    }
    Ok(s::Change {
        against: v.against,
        items: items.into_boxed(),
        running: match v.running {
            w::Running::Stays => s::Running::Stays,
            w::Running::Moves => s::Running::Moves,
        },
    })
}
fn map_problem(problem: w::Problem) -> Untranslatable {
    Untranslatable { path: problem.path }
}
fn setting(value: w::Setting) -> s::Setting {
    match value {
        w::Setting::Charter(value) => s::Setting::Charter(value.number()),
        w::Setting::Template(value) => s::Setting::Template(value.number()),
        w::Setting::Policy(value) => s::Setting::Policy(value.number()),
        w::Setting::Rules => s::Setting::Rules,
    }
}
fn setting_value(
    value: w::SettingValue,
    models: &[crate::Model],
    limits: &w::Limits,
) -> Result<s::Value, Untranslatable> {
    Ok(match value {
        w::SettingValue::Charter(value) => s::Value::Charter(Box::new(charter(*value))),
        w::SettingValue::Template(value) => s::Value::Template(Box::new(template(*value, models, limits)?)),
        w::SettingValue::Policy(value) => s::Value::Policy(Box::new(policy(*value, models, limits)?)),
        w::SettingValue::Rules(value) => s::Value::Rules(Box::new(rules(*value, models, limits)?)),
    })
}
fn model(value: w::SettingModel) -> c::Model {
    let v = value.into_parts();
    c::Model {
        dialect: v.dialect,
        endpoint: v.endpoint,
        program: v.program,
        name: v.name,
        window: v.window,
        output: v.output,
        input_price: v.input_price,
        cached_price: v.cached_price,
        cache_write_price: v.cache_write_price,
        output_price: v.output_price,
        price_unit: v.price_unit,
        efforts: v.efforts.into_boxed(),
        effort: v.effort,
    }
}
fn charter(value: w::Charter) -> c::Charter {
    let resume = value.resume();
    let inspect = value.inspect();
    let modify = value.modify();
    let shell = value.shell();
    let agents = value.agents();
    let v = value.into_parts();
    let mut alternatives = List::with_capacity(v.alternatives.len());
    for value in v.alternatives.into_boxed() {
        alternatives.push(model(value)).expect("same count");
    }
    c::Charter {
        number: v.number,
        hosts: match v.hosts {
            w::SettingHosts::Workers => jig_domain_fleet::Kinds::Workers,
            w::SettingHosts::Engine => jig_domain_fleet::Kinds::Engine,
            w::SettingHosts::Both => jig_domain_fleet::Kinds::Both,
        },
        spend: v.spend,
        policy: c::RunPolicy {
            instructions: v.instructions,
            waiting: v.waiting,
            resume,
            turns: v.turns,
            time: v.time,
            reserve_turns: v.reserve_turns,
            reserve_time: v.reserve_time,
            model: model(v.model),
            alternatives: alternatives.into_boxed(),
            inspect,
            modify,
            shell,
            agents,
            call_timeout: v.call_timeout,
        },
    }
}
fn authority(
    value: w::OwnerAuthority,
    models: &[crate::Model],
    limits: &w::Limits,
) -> Result<a::Authority, Untranslatable> {
    let mut names = List::with_capacity(p::MODELS);
    let mut value = asks::authority(value, limits, &mut names)?;
    if let Some(selected) = value.models
        && !selected.is_all()
    {
        let mut mask = 0_u64;
        for (at, name) in names.as_slice().iter().enumerate() {
            if selected.bits() & (1_u64.checked_shl(u32::try_from(at).expect("bounded model index")).unwrap_or(0)) == 0
            {
                continue;
            }
            let mut found = false;
            for (to, model) in models.iter().enumerate() {
                if model.name == *name {
                    mask |= 1_u64.checked_shl(u32::try_from(to).expect("bounded model index")).unwrap_or(0);
                    found = true;
                    break;
                }
            }
            if !found {
                return Err(bad());
            }
        }
        value.models = Some(p::Models::Named(mask));
    }
    c::authority_to_authority(value).ok_or(bad())
}
fn requirements(values: List<w::PolicyRequirement>) -> Box<[a::Requirement]> {
    let mut result = List::with_capacity(values.len());
    for value in values.into_boxed() {
        result.push(c::requirement_to_authority(asks::requirement(value))).expect("same count");
    }
    result.into_boxed()
}
fn policy(
    value: w::SettingPolicy,
    models: &[crate::Model],
    limits: &w::Limits,
) -> Result<c::ConfiguredPolicy, Untranslatable> {
    let v = value.into_parts();
    let mut roles = List::with_capacity(v.roles.len());
    for value in v.roles.into_boxed() {
        let v = value.into_parts();
        roles
            .push(a::Role {
                number: v.number,
                authority: authority(v.authority, models, limits)?,
                period_spend: v.period_spend,
                requests: a::Requests(v.requests),
                decides: a::Proposals(v.decides),
            })
            .expect("same count");
    }
    let mut grants = List::with_capacity(v.projections.len());
    for value in v.projections.into_boxed() {
        grants.push(asks::grant(value)).expect("same count");
    }
    let mut permissions = List::with_capacity(v.permissions.len());
    for value in v.permissions.into_boxed() {
        permissions
            .push(p::PermissionRole {
                connector: value.connector(),
                permission: value.permission(),
                role: value.role(),
            })
            .expect("same count");
    }
    let overrides = operations::overrides(v.templates);
    Ok(c::ConfiguredPolicy {
        policy: a::Policy {
            escalation_role: v.escalation_role,
            ceiling: authority(v.ceiling, models, limits)?,
            projections: c::grants_to_authority(grants.into_boxed()).ok_or(bad())?,
            period_spend: v.period_spend,
            roles: roles.into_boxed(),
            requirements: requirements(v.requirements),
        },
        permissions: permissions.into_boxed(),
        templates: overrides,
    })
}
fn rules(value: w::Rules, models: &[crate::Model], limits: &w::Limits) -> Result<a::Rules, Untranslatable> {
    let v = value.into_parts();
    let mut implies = List::with_capacity(v.implies.len());
    for value in v.implies.into_boxed() {
        implies
            .push(a::Implication { connector: value.connector(), kind: value.kind(), implies: value.implies() })
            .expect("same count");
    }
    let count = implies.len();
    Ok(a::Rules {
        ceiling: authority(v.ceiling, models, limits)?,
        period_spend: v.period_spend,
        minimum_run_spend: v.minimum_run_spend,
        maximum_run_spend: v.maximum_run_spend,
        implies: a::Implies::new(implies.into_boxed(), count).ok_or(bad())?,
        requirements: requirements(v.requirements),
    })
}
fn template(value: w::Template, models: &[crate::Model], limits: &w::Limits) -> Result<c::Template, Untranslatable> {
    let v = value.into_parts();
    if v.name.cut() {
        return Err(bad());
    }
    let flags = v.flags.into_parts();
    let llm = match super::super::llm::people(v.llm, models, w::Path::TemplateLlm)? {
        Some(value) => {
            let mut subagents = List::with_capacity(u32::try_from(value.subagents.len()).expect("bounded models"));
            for value in value.subagents {
                subagents.push(t::ModelEffort { model: value.model, effort: value.effort }).expect("same count");
            }
            Some(t::LlmChoice {
                account: value.account,
                main: t::ModelEffort { model: value.main.model, effort: value.main.effort },
                subagents: subagents.into_boxed(),
            })
        }
        None => None,
    };
    Ok(c::Template {
        number: v.number,
        name: v.name.into_parts().bytes,
        executor: operations::executor(v.executor),
        contract: contract(v.contract, limits)?,
        shallower: v.shallower,
        budget: operations::budget(v.budget),
        deadline: operations::deadline(v.deadline),
        tracked: v.tracked,
        flags: t::Flags {
            conversation: flags.conversation,
            listed: flags.listed,
            retention: match flags.retention {
                w::Retention::Kept => t::Retention::Kept,
                w::Retention::ForgottenOnceRead => t::Retention::ForgottenOnceRead,
            },
            ended_by: match flags.ended_by {
                w::EndedBy::Result => t::EndedBy::Result,
                w::EndedBy::Requester => t::EndedBy::Requester,
            },
        },
        label: v.label,
        grace: match v.grace {
            Some(value) => Some(value),
            None => None,
        },
        llm,
    })
}
fn contract(value: w::TaskContractDocument, limits: &w::Limits) -> Result<t::Contract, Untranslatable> {
    Ok(match value {
        w::TaskContractDocument::Report(value) => t::Contract::Report { words: value.words() },
        w::TaskContractDocument::Change(value) => {
            t::Contract::Change { connector: value.connector(), kind: value.kind(), words: value.words() }
        }
        w::TaskContractDocument::Approval(value) => {
            let v = value.into_parts();
            t::Contract::Approval { state: v.state, part: part(v.part) }
        }
        w::TaskContractDocument::Verdict(value) => {
            if value.cut() {
                return Err(bad());
            }
            let path = w::Path::TaskContractDocumentVerdictChoices;
            let count = asks::count(value.choices(), path)?;
            let mut choices = List::with_capacity(count);
            let mut reader = Reader::new(value.choices());
            for _ in 0..count {
                let bytes = asks::item(&mut reader, path)?;
                let mut piece = Reader::new(&bytes);
                let v = match w::VerdictChoice::decode(limits, &mut piece) {
                    Ok(value) => value.into_parts(),
                    Err(problem) => return Err(map_problem(problem)),
                };
                if !piece.is_empty() {
                    return Err(bad());
                }
                let mut fields = List::with_capacity(v.fields.len());
                for field in v.fields.into_boxed() {
                    let f = field.into_parts();
                    fields.push(t::FieldRule { name: f.name, max: f.max }).expect("same count");
                }
                choices
                    .push(t::Verdict {
                        code: v.code,
                        words: v.words,
                        followups: v.followups,
                        fields: fields.into_boxed(),
                    })
                    .expect("same count");
            }
            t::Contract::Verdict { choices: choices.into_boxed() }
        }
    })
}

fn part(value: w::OwnerPart) -> t::Part {
    let v = value.into_parts();
    t::Part {
        connector: v.connector,
        kind: match v.kind {
            w::OwnerPartKind::Resource => t::PartKind::Resource,
            w::OwnerPartKind::Pool => t::PartKind::Pool,
            w::OwnerPartKind::Result => t::PartKind::Result,
            w::OwnerPartKind::Procedure => t::PartKind::Procedure,
            w::OwnerPartKind::Effect => t::PartKind::Effect,
            w::OwnerPartKind::Outbox => t::PartKind::Outbox,
            w::OwnerPartKind::Hold => t::PartKind::Hold,
        },
        key: match v.key {
            w::OwnerPartKey::Number(value) => t::PartKey::Number(value.value()),
            w::OwnerPartKey::Path(value) => t::PartKey::Path(value.into_parts().value),
        },
    }
}

pub(crate) fn proposed(
    value: w::Propose,
    models: &[crate::Model],
    limits: &w::Limits,
) -> Result<s::Proposed, Untranslatable> {
    let value = value.into_parts();
    if value.project != 0 {
        return Err(Untranslatable { path: w::Path::ProposeProject });
    }
    let value_set = match value.action {
        w::ProposeAction::Settings(v) => v.into_parts(),
        w::ProposeAction::Batch(_) | w::ProposeAction::Amendment(_) | w::ProposeAction::ReleasePlaced(_) => {
            return Err(bad());
        }
    };
    let envelope = match w::ChangeSettings::new(
        limits,
        w::ChangeSettingsParts { against: value_set.against, items: value_set.items, running: w::Running::Stays },
    ) {
        Ok(value) => value,
        Err(problem) => return Err(map_problem(problem)),
    };
    let change = change(envelope, models, limits)?;
    Ok(s::Proposed { against: change.against, items: change.items, reason: value.reason })
}
