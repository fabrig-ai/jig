use alloc::boxed::Box;
use skein_lib::{List, Map};

use crate::{
    Effect, Entry, Environment, EnvironmentFact, Key, Named, Pool, ProcedureState, Resource, Service, ServiceFact,
};

/// Bounds for infrastructure's live records and system values.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Limits {
    /// Services in the working set.
    pub services: u32,
    /// Environments in the working set.
    pub environments: u32,
    /// Pools in the working set.
    pub pools: u32,
    /// Live tasks with named resources.
    pub tasks: u32,
    /// Live procedure states.
    pub procedures: u32,
    /// Staged effects and, separately, prepared application values.
    pub staged: u32,
    /// Durable proposed effects and, separately, retained application outcomes.
    pub proposals: u32,
    /// Unsettled outbox entries.
    pub effects: u32,
    /// Creations, including their outstanding effects and unanswered closes.
    pub made: u32,
    /// Maximum resources one task names.
    pub resources_per_task: u32,
    /// Maximum bytes in one name or version.
    pub name_bytes: u32,
    /// Maximum attempts before holding uncertainty.
    pub max_attempts: u32,
    /// Seconds before an uncertain attempt may be retried.
    pub retry_after_seconds: u64,
}

/// Whether these limits are representable: every edge bound they size
/// is finite and every capacity they derive fits (programming-model.md, 6.3).
#[must_use]
pub fn valid(limits: &Limits) -> bool {
    limits.name_bytes != 0
        && limits.max_attempts != 0
        && limits.retry_after_seconds != 0
        && closing_capacity(limits).is_some()
        && refresh_capacity(limits).is_some()
        && crate::drift_accounts::capacity(limits).is_some()
}

/// Conservative heap bound for tables and owned names.
#[must_use]
pub fn worst_case(limits: &Limits) -> Option<u64> {
    if !valid(limits) {
        return None;
    }
    let name = u64::from(limits.name_bytes);
    let refresh = refresh_capacity(limits)?;
    crate::application::worst_case(limits)?
        .checked_add(crate::drift_accounts::worst_case(limits)?)?
        .checked_add(Map::<skein_lib::Token, crate::ClientSubject>::worst_case(limits.proposals)?)?
        .checked_add(List::<skein_lib::Token>::worst_case(limits.proposals)?)?
        .checked_add(u64::from(limits.proposals).checked_mul(name.checked_mul(2)?)?)?
        .checked_add(Map::<Service, ServiceFact>::worst_case(limits.services)?)?
        .checked_add(Map::<u64, bool>::worst_case(closing_capacity(limits)?)?)?
        .checked_add(Map::<Resource, bool>::worst_case(refresh)?)?
        .checked_add(u64::from(refresh).checked_mul(u64::from(limits.name_bytes).checked_mul(2)?)?)?
        .checked_add(Map::<Environment, Option<EnvironmentFact>>::worst_case(limits.environments)?)?
        .checked_add(Map::<Pool, (u32, u32)>::worst_case(limits.pools)?)?
        .checked_add(Map::<u64, (u32, Box<[Named]>)>::worst_case(limits.tasks)?)?
        .checked_add(u64::from(limits.tasks).checked_mul(List::<Named>::worst_case(limits.resources_per_task)?)?)?
        .checked_add(Map::<u64, ProcedureState>::worst_case(limits.procedures)?)?
        .checked_add(List::<u64>::worst_case(limits.procedures)?)?
        .checked_add(Map::<skein_lib::Token, Effect>::worst_case(limits.staged)?)?
        .checked_add(Map::<u64, (u64, Effect)>::worst_case(limits.proposals)?)?
        .checked_add(Map::<[u8; 16], (u32, u64)>::worst_case(limits.made)?)?
        .checked_add(Map::<Key, Entry>::worst_case(limits.effects)?)?
        .checked_add(Map::<Environment, (Key, Option<u32>)>::worst_case(limits.made)?)?
        .checked_add(u64::from(limits.services).checked_mul(name.checked_mul(3)?)?)?
        .checked_add(u64::from(limits.environments).checked_mul(name.checked_mul(2)?)?)?
        .checked_add(u64::from(limits.pools).checked_mul(name)?)?
        .checked_add(
            u64::from(limits.tasks)
                .checked_mul(u64::from(limits.resources_per_task).checked_mul(name.checked_mul(2)?)?)?,
        )?
        .checked_add(u64::from(limits.procedures).checked_mul(name.checked_mul(3)?)?)?
        .checked_add(u64::from(limits.staged).checked_mul(name.checked_mul(3)?)?)?
        .checked_add(u64::from(limits.proposals).checked_mul(name.checked_mul(3)?)?)?
        .checked_add(u64::from(limits.effects).checked_mul(name.checked_mul(3)?)?)?
        .checked_add(u64::from(limits.made).checked_mul(name.checked_mul(2)?)?)
}

fn closing_capacity(limits: &Limits) -> Option<u32> {
    limits.tasks.checked_add(limits.procedures)?.checked_add(limits.effects)
}

fn refresh_capacity(limits: &Limits) -> Option<u32> {
    limits
        .tasks
        .checked_mul(limits.resources_per_task)?
        .checked_add(limits.procedures)?
        .checked_add(limits.environments)?
        .checked_add(limits.made)
}
