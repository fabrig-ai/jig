//! Fixed pacing capacities (`domain/engine.md`, 11 and 13).
//! There is no additional per-subject limit (`QUESTIONS.md`, item 17).
/// Capacities fixed when the child is constructed.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Limits {
    /// Live runs whose provisional text is retained, watched or not.
    /// Configure at least the engine's maximum number of live runs.
    pub runs: u32,
    /// Open watches.
    pub watchers: u32,
    /// Open watches belonging to one opaque party.
    pub watchers_per_party: u32,
    /// Subjects named by one change event.
    pub subjects_per_change: u32,
    /// Retained provisional bytes per live run.
    pub text_bytes: u32,
    /// Best effort observations waiting to be drained.
    pub facts: u32,
}
pub(crate) fn slots(limits: &Limits) -> Option<u32> {
    limits.watchers.checked_mul(2)
}

/// Representability of the child limits (programming-model.md, 6.3).
#[must_use]
pub fn valid(limits: &Limits) -> bool {
    limits.runs > 0
        && limits.watchers > 0
        && limits.watchers_per_party > 0
        && limits.subjects_per_change > 0
        && limits.text_bytes > 0
        && slots(limits).is_some()
}
