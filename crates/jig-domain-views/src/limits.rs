//! Fixed pacing capacities (`domain/engine.md`, 11 and 13).
//! There is no additional per-subject limit (`QUESTIONS.md`, item 17).
use crate::facts::Fact;
use crate::watch::Watcher;
use skein_lib::{Id, List, Map, Queue, Slab, Token};

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

/// Maximum pacing memory, including one bounded change event and scan in hand.
#[must_use]
pub fn worst_case(limits: &Limits) -> Option<u64> {
    if limits.runs == 0
        || limits.watchers == 0
        || limits.watchers_per_party == 0
        || limits.subjects_per_change == 0
        || limits.text_bytes == 0
    {
        return None;
    }
    Slab::<Watcher>::worst_case(slots(limits)?)?
        .checked_add(Map::<Token, crate::RetainedRun>::worst_case(limits.runs)?)?
        .checked_add(u64::from(limits.runs).checked_mul(u64::from(limits.text_bytes).checked_mul(2)?)?)?
        .checked_add(u64::from(limits.text_bytes).checked_mul(3)?)?
        .checked_add(Map::<Token, Id<Watcher>>::worst_case(limits.watchers)?)?
        .checked_add(Queue::<Fact>::worst_case(limits.facts)?)?
        .checked_add(List::<Token>::worst_case(limits.subjects_per_change)?)?
        .checked_add(List::<Id<Watcher>>::worst_case(limits.watchers)?)?
        .checked_add(List::<Token>::worst_case(limits.runs)?)
}
