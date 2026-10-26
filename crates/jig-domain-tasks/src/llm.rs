//! An agent task's LLM choice: its main model and the models its runs' sub-agents may use, each
//! with an effort (domain/tasks.md, section 3). Tasks checks a choice as a batch or an amendment is
//! checked: an agent task's only, each model configured, each effort among those its model
//! declares, each model among its charter's and within its authority's models (domain/tasks.md,
//! sections 4 and 7; domain/authority.md, section 8.1). It resolves a charter's default for the
//! root, which records it as given when a creator gives none (domain/tasks.md, section 3).
//!
//! Models and efforts are indices in configuration's order: model `i` is the `i`th configured
//! model, bit `i` of `Models`; effort `e` is the `e`th effort that model declares
//! (`../protocol/configuration.md`, section 3).

use alloc::boxed::Box;

use skein_lib::List;

use crate::domain::Domain;
use crate::{Authority, Executor, Models, Refusal};

/// Most configured models, the bits of `Models` (`../protocol/configuration.md`, section 3). A
/// choice's sub-agents name at most this many.
pub const MODELS: u32 = 64;

/// One model and its effort (domain/tasks.md, section 3).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ModelEffort {
    /// The configured model, in configuration's order.
    pub model: u8,
    /// One of the efforts the model declares, in its order; `None` is the LLM provider's default,
    /// the only effort of a model that declares none.
    pub effort: Option<u8>,
}

/// What an agent task's runs' LLMs are, including its creator's optional account preference
/// (domain/tasks.md, section 3; domain/accounts.md, section 4.3).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct LlmChoice {
    /// Creator preference, distinct from the task's successfully claimed account.
    pub account: Option<u32>,
    /// The model its runs use, and their check holds its most costly completion (authority.md,
    /// section 8.3).
    pub main: ModelEffort,
    /// The models its runs' sub-agents may use, each named once, at most `MODELS`, among which
    /// smith chooses within a run.
    pub subagents: Box<[ModelEffort]>,
}

/// Why an LLM choice is refused (domain/tasks.md, sections 4 and 7).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum LlmFault {
    /// The model is not configured.
    Unconfigured,
    /// The effort is not one the model declares, or none is named for a model that declares some.
    Effort,
    /// The model is not one of the task's charter's.
    Charter,
    /// The task's authority does not allow the model (domain/authority.md, section 3).
    Authority,
    /// The task is a procedure's or a person's, whose executor runs no model.
    Procedure,
    /// The sub-agents name the model twice.
    Repeated,
}

/// One charter version's models, in charter order; a removal offers none
/// (domain/engine.md, 17.3).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct CharterModels {
    /// The charter's number.
    pub number: u32,
    /// The revision that set this charter value.
    pub revision: u32,
    /// Its configured model indices, at most `MODELS`; none marks a removal.
    pub models: Option<Box<[u8]>>,
}

/// Whether `choice` has the shape a task keeps: its sub-agents at most `MODELS`.
#[must_use]
pub(crate) fn valid_shape(choice: &LlmChoice) -> bool {
    choice.subagents.len() <= usize::try_from(MODELS).expect("u32 fits usize")
}

/// Whether `new`'s choice, if any, has the shape a task keeps; a recurrence checks the rest.
#[must_use]
pub(crate) fn valid_member(new: &crate::New) -> bool {
    match &new.llm {
        Some(choice) => valid_shape(choice),
        None => true,
    }
}

/// Check `choice` for a task run by `executor` within `authority` (domain/tasks.md, section 4;
/// domain/authority.md, section 8.1). A refusal names the first model that fails and why. A choice
/// beyond the authority is refused, not proposed: proposing a model's widening comes later.
pub(crate) fn check(
    domain: &Domain,
    executor: Executor,
    revision: u32,
    authority: &Authority,
    choice: &LlmChoice,
) -> Result<(), Refusal> {
    let charter = match executor {
        Executor::Agent { charter } => charter,
        Executor::Procedure { .. } | Executor::Person(_) => {
            return Err(Refusal::Llm { model: choice.main.model, why: LlmFault::Procedure });
        }
    };
    let models = domain.charter_models(charter, revision).unwrap_or(&[]);
    fits(domain, models, authority.models, choice.main)?;
    for (at, chosen) in choice.subagents.iter().enumerate() {
        for earlier in choice.subagents.iter().take(at) {
            if earlier.model == chosen.model {
                return Err(Refusal::Llm { model: chosen.model, why: LlmFault::Repeated });
            }
        }
        // Each passing entry names another configured model, so the loop ends by `MODELS`.
        fits(domain, models, authority.models, *chosen)?;
    }
    Ok(())
}

fn fits(domain: &Domain, charter: &[u8], allowed: Models, chosen: ModelEffort) -> Result<(), Refusal> {
    let Some(declared) = domain.efforts.get(usize::from(chosen.model)) else {
        return Err(Refusal::Llm { model: chosen.model, why: LlmFault::Unconfigured });
    };
    let effort = match chosen.effort {
        Some(effort) => effort < *declared,
        None => *declared == 0,
    };
    if !effort {
        return Err(Refusal::Llm { model: chosen.model, why: LlmFault::Effort });
    }
    let mut listed = false;
    for model in charter {
        if *model == chosen.model {
            listed = true;
        }
    }
    if !listed {
        return Err(Refusal::Llm { model: chosen.model, why: LlmFault::Charter });
    }
    if !allows(allowed, chosen.model) {
        return Err(Refusal::Llm { model: chosen.model, why: LlmFault::Authority });
    }
    Ok(())
}

fn allows(models: Models, model: u8) -> bool {
    match 1_u64.checked_shl(u32::from(model)) {
        Some(bit) => models.bits() & bit != 0,
        None => false,
    }
}

fn first_effort(domain: &Domain, model: u8) -> Option<u8> {
    match domain.efforts.get(usize::from(model)) {
        Some(0) | None => None,
        Some(_) => Some(0),
    }
}

impl Domain {
    /// The default LLM choice of an agent task of `charter` within `models`, which the root
    /// records as given (domain/tasks.md, section 3): main is the first of the charter's models,
    /// in its order, within `models`, and the sub-agents the charter's others within `models`;
    /// each with its model's first declared effort, or the provider's default for a model that
    /// declares none. `None` when the charter is not configured or allows none of `models`.
    #[must_use]
    pub fn default_llm(&self, charter: u32, models: Models) -> Option<LlmChoice> {
        let configured = self.charter_models(charter, self.newest())?;
        let mut main = None;
        let mut subagents =
            List::with_capacity(u32::try_from(configured.len()).expect("a charter's models fit `MODELS`"));
        for model in configured {
            // T10 narrows main to the first whose LLM provider has an account usable now and
            // serving the task's payer, else the first (domain/accounts.md, section 4.3).
            if !allows(models, *model) {
                continue;
            }
            // Configuration names a charter's models, not an effort for each (configuration.md,
            // 3), so the charter's effort domain/tasks.md section 3 names is each model's first.
            let chosen = ModelEffort { model: *model, effort: first_effort(self, *model) };
            match main {
                None => main = Some(chosen),
                Some(_) => subagents.push(chosen).expect("one per charter model"),
            }
        }
        Some(LlmChoice { account: None, main: main?, subagents: subagents.into_boxed() })
    }
}

/// Clearing the creator preference keeps the pin; an explicit different account supersedes it
/// only in the admitted amendment (accounts.md, 4.3; tasks.md, section 7).
pub(crate) fn replaces_account(kept: Option<u32>, choice: &LlmChoice) -> bool {
    match choice.account {
        Some(account) => kept.is_some() && kept != Some(account),
        None => false,
    }
}

/// Why an explicit account cannot fund the prospective task (domain/accounts.md, 4.3).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum AccountWhy {
    Unknown,
    Provider,
    Uses,
    Limited,
}
