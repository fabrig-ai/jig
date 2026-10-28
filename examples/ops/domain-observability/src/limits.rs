/// Bounds for observability's working set and one step's outputs.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Limits {
    /// Cached service facts.
    pub services: u32,
    /// Live subscriptions.
    pub subscriptions: u32,
    /// Standing watches.
    pub watches: u32,
    /// Staged silences.
    pub staged: u32,
    /// Outstanding silences.
    pub effects: u32,
    /// Pending verdicts.
    pub judges: u32,
    /// Pending reads.
    pub reads: u32,
    /// Maximum subscribers reached by one alert.
    pub subscribers_per_alert: u32,
    /// Maximum load samples per service.
    pub samples_per_service: u32,
    /// Maximum services in one watch.
    pub services_per_watch: u32,
    /// Maximum alert numbers in one batch.
    pub alerts_per_batch: u32,
    /// Maximum bytes in a name or rule.
    pub name_bytes: u32,
    /// Maximum bytes in a read answer.
    pub answer_bytes: u32,
    /// Maximum attempts before an uncertain effect is held.
    pub max_attempts: u32,
    /// Seconds from sending an attempt to its absolute retry deadline.
    pub retry_after_seconds: u64,
}

/// Whether these limits are representable: every edge bound they size
/// is finite and every capacity they derive fits (programming-model.md, 6.3).
#[must_use]
pub fn valid(limits: &Limits) -> bool {
    limits.name_bytes != 0
        && limits.answer_bytes != 0
        && limits.max_attempts != 0
        && limits.retry_after_seconds != 0
        && limits.subscribers_per_alert <= limits.subscriptions
        && closing_capacity(limits).is_some()
        && refresh_capacity(limits).is_some()
        && crate::incidents::capacity(limits).is_some()
        && crate::watch_steps::capacity(limits).is_some()
        && crate::watch_steps::message_capacity(limits).is_some()
}

fn closing_capacity(limits: &Limits) -> Option<u32> {
    limits.subscriptions.checked_add(limits.watches)?.checked_add(limits.effects)
}

fn refresh_capacity(limits: &Limits) -> Option<u32> {
    limits
        .subscriptions
        .checked_add(limits.watches.checked_mul(limits.services_per_watch)?)?
        .checked_add(crate::incidents::capacity(limits)?)
}
