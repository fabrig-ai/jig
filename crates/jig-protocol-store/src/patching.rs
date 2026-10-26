//! Patching at open (protocol/store.md 10.2 and 10.3): the owners' patch
//! sets, the plan across them, one machine per kept family, a record's
//! walk, and the phase that runs them over a copy.
pub(crate) mod check;
pub(crate) mod machines;
pub(crate) mod phase;
pub(crate) mod plan;
pub mod sets;
pub(crate) mod switch;
pub(crate) mod walk;

/// Records rewritten and page commits made by one patch (store.md 10.2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Counts {
    pub records: u64,
    pub commits: u64,
}
