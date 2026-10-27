//! Names and indices share configuration's order, as the real client translators do.
use super::{asks, owner};
use jig_client::DomainLimits as Limits;
use jig_client_domain::contract as c;
use jig_domain_core::Model;
use jig_domain_people as p;
use jig_domain_tasks as t;
use skein_lib::List;

pub(super) fn name(value: &[u8], limits: &Limits) -> Result<c::Text, owner::Invalid> {
    c::Text::new(owner::bytes(value, limits)?, limits.text).map_err(|_| owner::Invalid::TooLarge)
}

#[expect(clippy::manual_map, reason = "optional provider copied without a closure")]
pub(super) fn models(
    allowed: jig_domain_authority::Models,
    configured: &[Model],
    providers: &jig_domain_accounts::Providers,
    limits: &Limits,
) -> Result<List<c::MeModel>, owner::Invalid> {
    let count = u32::try_from(configured.len()).map_err(|_| owner::Invalid::TooLarge)?;
    if count > limits.rows || count > t::MODELS {
        return Err(owner::Invalid::TooLarge);
    }
    let mut rows = List::with_capacity(count);
    for (at, model) in configured.iter().enumerate() {
        let shift = u32::try_from(at).map_err(|_| owner::Invalid::Llm)?;
        let bit = 1_u64.checked_shl(shift).ok_or(owner::Invalid::Llm)?;
        if allowed.bits() & bit == 0 {
            continue;
        }
        let count = u32::try_from(model.efforts.len()).map_err(|_| owner::Invalid::TooLarge)?;
        if count > limits.rows {
            return Err(owner::Invalid::TooLarge);
        }
        let mut efforts = List::with_capacity(count);
        for effort in &model.efforts {
            efforts.push(name(effort, limits)?).map_err(|_| owner::Invalid::TooLarge)?;
        }
        let provider = match providers.serving(&model.name) {
            Some(provider) => Some(provider.number),
            None => None,
        };
        rows.push(c::MeModel {
            name: name(&model.name, limits)?,
            provider,
            window: model.window,
            output: model.output,
            efforts,
            prices: c::ModelPrices {
                input: model.input_price,
                cached: model.cached_price,
                cache_write: model.cache_write_price,
                output: model.output_price,
                unit: model.price_unit,
            },
            description: None,
        })
        .map_err(|_| owner::Invalid::TooLarge)?;
    }
    Ok(rows)
}

pub(super) fn choice(
    value: Option<&t::LlmChoice>,
    configured: &[Model],
    limits: &Limits,
) -> Result<Option<c::LlmChoice>, owner::Invalid> {
    let Some(value) = value else { return Ok(None) };
    let count = u32::try_from(value.subagents.len()).map_err(|_| owner::Invalid::TooLarge)?;
    if count > limits.rows {
        return Err(owner::Invalid::TooLarge);
    }
    let mut subagents = List::with_capacity(count);
    for item in &value.subagents {
        subagents.push(model_effort(*item, configured, limits)?).map_err(|_| owner::Invalid::TooLarge)?;
    }
    Ok(Some(c::LlmChoice { account: value.account, main: model_effort(value.main, configured, limits)?, subagents }))
}

fn model_effort(
    value: t::ModelEffort,
    configured: &[Model],
    limits: &Limits,
) -> Result<c::ModelEffort, owner::Invalid> {
    let model = configured.get(usize::from(value.model)).ok_or(owner::Invalid::Llm)?;
    let effort = match value.effort {
        Some(at) => Some(name(model.efforts.get(usize::from(at)).ok_or(owner::Invalid::Llm)?, limits)?),
        None => None,
    };
    Ok(c::ModelEffort { model: name(&model.name, limits)?, effort })
}

pub(super) fn model_index(name: &[u8], configured: &[Model]) -> Result<u8, asks::Invalid> {
    for (at, model) in configured.iter().enumerate() {
        if name == model.name.as_ref() {
            return u8::try_from(at).map_err(|_| asks::Invalid::Llm);
        }
    }
    Err(asks::Invalid::Llm)
}

fn indices(value: &c::ModelEffort, configured: &[Model]) -> Result<p::ModelEffort, asks::Invalid> {
    let index = model_index(value.model.bytes(), configured)?;
    let model = configured.get(usize::from(index)).ok_or(asks::Invalid::Llm)?;
    let effort = match &value.effort {
        Some(name) => {
            let mut found = None;
            for (at, effort) in model.efforts.iter().enumerate() {
                if name.bytes() == effort.as_ref() {
                    found = Some(u8::try_from(at).map_err(|_| asks::Invalid::Llm)?);
                    break;
                }
            }
            Some(found.ok_or(asks::Invalid::Llm)?)
        }
        None => None,
    };
    Ok(p::ModelEffort { model: index, effort })
}

pub(super) fn requested(
    value: Option<&c::LlmChoice>,
    configured: &[Model],
) -> Result<Option<p::LlmChoice>, asks::Invalid> {
    let Some(value) = value else { return Ok(None) };
    let mut subagents = List::with_capacity(value.subagents.len());
    for item in &value.subagents {
        subagents.push(indices(item, configured)?).map_err(|_| asks::Invalid::Llm)?;
    }
    Ok(Some(p::LlmChoice {
        account: value.account,
        main: indices(&value.main, configured)?,
        subagents: subagents.into_boxed(),
    }))
}

pub(super) fn refusal(
    model: u8,
    why: p::LlmWhy,
    configured: &[Model],
    limits: &Limits,
) -> Result<c::Refusal, owner::Invalid> {
    let model = configured.get(usize::from(model)).ok_or(owner::Invalid::Llm)?;
    let why = match why {
        p::LlmWhy::Unconfigured => c::LlmWhy::Unconfigured,
        p::LlmWhy::Effort => c::LlmWhy::Effort,
        p::LlmWhy::Charter => c::LlmWhy::Charter,
        p::LlmWhy::Authority => c::LlmWhy::Authority,
        p::LlmWhy::Procedure => c::LlmWhy::Procedure,
        p::LlmWhy::Repeated => c::LlmWhy::Repeated,
    };
    Ok(c::Refusal::Llm { model: name(&model.name, limits)?, why })
}

/// Match the real entrance check before names become domain indices.
pub(super) fn refused(value: &c::Ask, configured: &[Model]) -> Result<Option<c::Answer>, asks::Invalid> {
    let (at, lacking) = match value {
        c::Ask::Create { tasks, .. } => (None, batch_lacking(tasks, configured)),
        c::Ask::Propose { action, .. } => match action {
            c::ProposeAction::Settings(_) => return Err(asks::Invalid::Settings),
            c::ProposeAction::Batch { tasks } => (None, batch_lacking(tasks, configured)),
            c::ProposeAction::Amendment { change, .. } => (None, lacking(change.llm.as_ref(), configured)),
        },
        c::Ask::Amend { change, .. } => (None, lacking(change.llm.as_ref(), configured)),
        c::Ask::Compound { operations, .. } => {
            for (at, operation) in operations.iter().enumerate() {
                let found = match operation {
                    c::Operation::Amend { change, .. } => lacking(change.llm.as_ref(), configured),
                    c::Operation::Message { .. }
                    | c::Operation::Cancel { .. }
                    | c::Operation::Release { .. }
                    | c::Operation::Stop { .. }
                    | c::Operation::Move { .. } => None,
                };
                if let Some(found) = found {
                    return Ok(Some(c::Answer::Refused {
                        refusal: c::Refusal::Llm { model: found.model, why: found.why },
                        at: Some(u32::try_from(at).map_err(|_| asks::Invalid::OperationIndex)?),
                    }));
                }
            }
            return Ok(None);
        }
        c::Ask::Message { .. }
        | c::Ask::Answer { .. }
        | c::Ask::Take { .. }
        | c::Ask::HandBack { .. }
        | c::Ask::Decide { .. }
        | c::Ask::Release { .. }
        | c::Ask::Stop { .. }
        | c::Ask::Cancel { .. }
        | c::Ask::Move { .. }
        | c::Ask::Seen { .. }
        | c::Ask::EditNote { .. }
        | c::Ask::DeleteNote { .. }
        | c::Ask::Rename { .. }
        | c::Ask::Hold { .. }
        | c::Ask::Lift { .. }
        | c::Ask::ChangeSettings(_)
        | c::Ask::MoveToNewest(_)
        | c::Ask::MakeProject { .. }
        | c::Ask::ChangePolicy { .. }
        | c::Ask::SetRoles { .. }
        | c::Ask::MakeService { .. }
        | c::Ask::SignIn { .. }
        | c::Ask::AccountChange { .. }
        | c::Ask::SignOut { .. }
        | c::Ask::SetPool { .. }
        | c::Ask::Withdraw { .. } => return Ok(None),
    };
    let Some(lacking) = lacking else { return Ok(None) };
    Ok(Some(c::Answer::Refused { refusal: c::Refusal::Llm { model: lacking.model, why: lacking.why }, at }))
}

struct Lacking {
    model: c::Text,
    why: c::LlmWhy,
}

fn batch_lacking(values: &List<c::NewTask>, configured: &[Model]) -> Option<Lacking> {
    for value in values {
        if let Some(found) = lacking(value.llm.as_ref(), configured) {
            return Some(found);
        }
    }
    None
}

fn lacking(value: Option<&c::LlmChoice>, configured: &[Model]) -> Option<Lacking> {
    let value = value?;
    if let Some(why) = unconfigured(&value.main, configured) {
        return Some(Lacking { model: value.main.model.clone(), why });
    }
    for subagent in &value.subagents {
        if let Some(why) = unconfigured(subagent, configured) {
            return Some(Lacking { model: subagent.model.clone(), why });
        }
    }
    None
}

fn unconfigured(value: &c::ModelEffort, configured: &[Model]) -> Option<c::LlmWhy> {
    for model in configured {
        if value.model.bytes() != model.name.as_ref() {
            continue;
        }
        let Some(effort) = &value.effort else { return None };
        for name in &model.efforts {
            if effort.bytes() == name.as_ref() {
                return None;
            }
        }
        return Some(c::LlmWhy::Effort);
    }
    Some(c::LlmWhy::Unconfigured)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choice_lists_obey_the_receiving_bound() {
        let source = t::LlmChoice {
            account: Some(3),
            main: t::ModelEffort { model: 0, effort: None },
            subagents: Box::new([t::ModelEffort { model: 0, effort: Some(1) }]),
        };
        let models = [jig_core_world::world::scripted_model()];
        let limits = super::super::deployment::tests::limits();
        assert!(choice(Some(&source), &models, &Limits { rows: 1, turn_text: limits.turn_text, ..limits }).is_ok());
        assert_eq!(
            choice(Some(&source), &models, &Limits { rows: 0, turn_text: limits.turn_text, ..limits }),
            Err(owner::Invalid::TooLarge)
        );
        assert_eq!(
            choice(Some(&source), &models, &Limits { text: 3, turn_text: 3, ..limits }),
            Err(owner::Invalid::TooLarge)
        );
    }

    #[test]
    fn model_lists_obey_the_receiving_bound() {
        let configured = [jig_core_world::world::scripted_model()];
        let providers = jig_domain_accounts::Providers::none();
        let limits = super::super::deployment::tests::limits();
        assert!(
            models(
                jig_domain_authority::Models::ALL,
                &configured,
                &providers,
                &Limits { rows: 2, turn_text: limits.turn_text, ..limits }
            )
            .is_ok()
        );
        assert_eq!(
            models(
                jig_domain_authority::Models::ALL,
                &configured,
                &providers,
                &Limits { rows: 1, turn_text: limits.turn_text, ..limits }
            ),
            Err(owner::Invalid::TooLarge)
        );
    }

    #[test]
    fn missing_choice_indices_are_refused() {
        let models = [jig_core_world::world::scripted_model()];
        let limits = super::super::deployment::tests::limits();
        for main in [t::ModelEffort { model: 1, effort: None }, t::ModelEffort { model: 0, effort: Some(2) }] {
            let source = t::LlmChoice { account: None, main, subagents: Box::new([]) };
            assert_eq!(choice(Some(&source), &models, &limits), Err(owner::Invalid::Llm));
        }
    }
}
