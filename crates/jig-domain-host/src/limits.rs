/// The host child domain's limits (section 7), handed by its parent to every
/// step read-only.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Limits {
    /// The most bytes of one piece of live content (hosts.md, 6.5).
    pub piece_bytes: u32,
    /// Local program sources; their command lifecycle does not consume run slots.
    pub program_sources: u32,
    /// Positive delay from a completed check to the next periodic check.
    pub source_interval: skein_lib::Duration,
    /// Positive deadline span for one source check or sign-out command.
    pub source_timeout: skein_lib::Duration,
    /// Runs hosted at once: the run slots. An assignment beyond them is
    /// refused as busy.
    pub slots: u32,
    /// Distinct grants retained for a hosted attempt.
    pub accounts: u32,
    /// The most bytes of a charter.
    pub charter_bytes: u64,
    /// Opaque transcript, including the committed call tail.
    pub transcript_bytes: u64,
    /// Maximum opaque evidence bytes in one settled delivery call.
    pub delivery_evidence_bytes: u64,
    /// One completed turn body.
    pub turn_bytes: u64,
    /// The most bytes of a run's declared outcome. A run that says more has
    /// broken the rules.
    pub outcome_bytes: u64,
    /// The most bytes of detail an answer carries for operators: the tail of
    /// what it is given.
    pub detail_bytes: u32,
    /// Inbound events a run holds until it is live. Beyond them, an event is
    /// bounced.
    pub held: u32,
    /// Messages a run's inbox takes: the most a start carries, and the most
    /// refusals a turn or an answer carries.
    pub inbox: u32,
    /// The most bytes of an inbound event.
    pub event_bytes: u64,
    /// Host calls a run may have in flight at once, its delivery among them. A
    /// call beyond them is answered as busy.
    pub run_calls: u32,
    /// Facts kept until the parent drains them. Beyond them, facts are
    /// dropped and counted.
    pub facts: u32,
    /// Activity and live pieces kept for the core while the link cannot take it.
    pub told: u32,
    /// Normal retained turns per attempt. One additional agent window of
    /// this size is reserved for turns sent before stopping.
    pub turns: u32,
    /// Normal retained body bytes per attempt, with the same stopping reserve.
    pub turn_queue_bytes: u64,
}

/// The host calls in flight at once, across runs, under `limits`, or `None` if
/// they do not fit a `u32`.
pub(crate) fn calls(limits: &Limits) -> Option<u32> {
    limits.slots.checked_mul(limits.run_calls)
}

/// Whether these limits are representable: every edge bound they size
/// is finite and every capacity they derive fits (programming-model.md, 6.3).
#[must_use]
pub fn valid(limits: &Limits) -> bool {
    if limits.turns > 0 && (limits.turn_bytes == 0 || limits.turn_queue_bytes < limits.turn_bytes) {
        return false;
    }
    cancellations_bound(limits).is_some()
        && calls(limits).is_some()
        && turn_capacity(limits).is_some()
        && crate::sources::valid(limits)
}

fn turn_capacity(limits: &Limits) -> Option<u32> {
    limits.slots.checked_mul(limits.turns)?.checked_mul(2)
}

fn cancellations_bound(limits: &Limits) -> Option<u32> {
    limits
        .run_calls
        .checked_mul(2)?
        .max(limits.held.checked_add(limits.accounts)?)
        .checked_add(limits.program_sources)?
        .checked_add(4)?
        .checked_add(limits.turns.checked_mul(2)?)
}
