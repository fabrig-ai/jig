use skein_lib::Duration;

/// The fleet's limits (programming-model.md, section 7), handed by its parent
/// to every step read-only.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Limits {
    /// Configured workers, including those not in contact.
    pub configured_workers: u32,
    /// Bytes in each configured worker name.
    pub worker_name_bytes: u32,
    /// Program source names retained per connected worker, independently of run slots.
    pub program_sources: u32,
    /// Maximum diagnostic detail retained for each login or sign-out failure.
    pub source_detail_bytes: u32,
    /// Workers in contact at once. A worker beyond them is turned away at
    /// its hello.
    pub workers: u32,
    /// Owned charter/model requirements retained for admission and placement.
    pub needs_bytes: u64,
    /// Charter versions held besides the newest (engine.md, 17.8).
    pub versions: u32,
    /// Slots hosted inside the engine; zero in applications using only workers.
    pub engine_slots: u32,
    /// The most runs a worker hosts at once: slots a hello says beyond them
    /// are not used, and a hello listing more runs is turned away.
    pub slots: u32,
    /// The workstreams kept per worker, for placement to prefer.
    pub workstreams: u32,
    /// The parent's attempts tracked at once: waiting to be placed, out on
    /// workers, or their answers not yet acknowledged; and those it replaced
    /// while a worker may still host them. A start or an adoption beyond them
    /// is refused as busy. Beyond them, room is kept for what workers list
    /// that the parent has not claimed, a worker's slots for each worker; a
    /// listing beyond that room is cancelled.
    pub attempts: u32,
    /// Credential names held by each attempt, across both protocol tables.
    pub grants: u32,
    /// Relayed calls the parent serves at once. A call beyond them is
    /// dropped, and its run withdraws it past its own deadline.
    pub calls: u32,
    /// Maximum bytes of a run's opaque call name retained for each relay.
    pub call_name_bytes: u64,
    /// Whole tool-name bytes in one best-effort observation; zero omits names.
    pub progress_bytes: u64,
    /// Turns awaiting adoption or the parent's commitment, across all
    /// attempts. Bodies are the parent's tokens, never bytes held here.
    pub turns: u32,
    /// How long the runs of a worker whose channel was lost are kept for it
    /// to come back, and how long an attempt a worker lists, and the parent
    /// has not claimed, waits to be adopted once the parent has loaded its
    /// claims.
    pub grace: Duration,
    /// Facts kept until the parent drains them. Beyond them, facts are
    /// dropped and counted.
    pub facts: u32,
    /// Recent lost channels and refused hellos retained separately from diagnostics,
    /// at most this many of each. Oldest records are evicted and counted.
    pub history: u32,
}

/// Representability of the child limits (programming-model.md, 6.3).
#[must_use]
pub fn valid(limits: &Limits) -> bool {
    if limits.worker_name_bytes > crate::WORKER_NAME {
        return false;
    }
    valid_counts(limits).is_some()
}

fn valid_counts(limits: &Limits) -> Option<()> {
    limits.configured_workers.checked_add(1)?;
    limits.program_sources.checked_mul(2)?.checked_add(1)?;
    limits.workers.checked_add(u32::from(limits.engine_slots > 0))?.checked_add(limits.history)?;
    tracked(limits)?;
    needs_bytes(limits)?;
    Some(())
}

/// The attempts tracked at once under `limits`: the parent's, and the room
/// kept for workers' listings; or `None` if they do not fit a `u32`.
pub(crate) fn tracked(limits: &Limits) -> Option<u32> {
    limits.attempts.checked_add(limits.workers.checked_mul(limits.slots)?)
}

/// Newest's byte budget plus each retained version's header, model headers and
/// names. A main model may also occur among the catalogue-sized alternatives.
pub(crate) fn needs_bytes(limits: &Limits) -> Option<u64> {
    let model =
        u64::try_from(size_of::<crate::ModelNeed>()).ok()?.checked_add(u64::from(crate::CATALOGUE_NAME_BYTES))?;
    let version = u64::try_from(size_of::<crate::CharterNeeds>())
        .ok()?
        .checked_add(u64::from(crate::CATALOGUE_MODELS).checked_add(1)?.checked_mul(model)?)?;
    limits.needs_bytes.checked_add(u64::from(limits.versions).checked_mul(version)?)
}
