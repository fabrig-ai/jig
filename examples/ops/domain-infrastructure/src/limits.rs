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
