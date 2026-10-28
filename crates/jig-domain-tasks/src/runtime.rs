//! Durable lifecycle observations for chips (client/domain/objects.md, section 3).
//! Timestamps come from injected transition events; retries name the decision
//! that paused an activation, rather than a counter selected by a reader.
//! The row owns the attempt identity; a new claim clears both observations.
//! Its start report is root placement/adoption timing, not agent/tool timing.
use crate::Class;

/// The decision responsible for the most recent retry pause.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum RetryNotice {
    /// A classified attempt failure, with its counter and configured allowance.
    Failed { class: Class, count: u32, allowance: u32 },
    /// Preparation or placement refused before execution; no failure try spent.
    Refused { count: u32 },
}
