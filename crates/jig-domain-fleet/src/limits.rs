use alloc::boxed::Box;
use skein_lib::{Deadlines, Duration, Id, Map, Queue, Set, Slab, Token};

use crate::attempt::{Attempt, Run};
use crate::call::PendingCall;
use crate::channel::Channel;
use crate::facts::Fact;
use crate::turn::Pending;

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

/// The most memory the domain holds under `limits`, in bytes
/// (programming-model.md, 6.3), or `None` if it does not fit a `u64`.
///
/// It counts the containers, their bookkeeping included, not allocator overhead. What the fleet passes on (a hello's lists,
/// the parent's payloads, which it names by token) is moved into its state or
/// dropped in the step it arrives in: the workstreams it keeps are counted here, the
/// rest is its sender's to count.
#[must_use]
pub fn worst_case(limits: &Limits) -> Option<u64> {
    // Each fan-out includes its terminal or target-list message.
    limits.configured_workers.checked_add(1)?;
    limits.program_sources.checked_mul(2)?.checked_add(1)?;
    if limits.worker_name_bytes > crate::WORKER_NAME {
        return None;
    }
    let catalogue = u64::from(crate::CATALOGUE_MODELS).checked_mul(
        u64::from(crate::CATALOGUE_NAME_BYTES).checked_add(u64::try_from(size_of::<Box<[u8]>>()).ok()?)?,
    )?;
    let evidence = u64::from(crate::CATALOGUE_MODELS).checked_mul(
        u64::from(crate::CATALOGUE_NAME_BYTES).checked_add(u64::try_from(size_of::<crate::ModelBound>()).ok()?)?,
    )?;
    let configured = skein_lib::List::<Box<[u8]>>::worst_case(limits.configured_workers)?
        .checked_add(u64::from(limits.configured_workers).checked_mul(u64::from(limits.worker_name_bytes))?)?;
    let count = limits.workers.checked_add(u32::from(limits.engine_slots > 0))?;
    count.checked_add(limits.history)?;
    let endpoints = u64::from(count).checked_mul(u64::from(crate::PROGRAM_ENDPOINTS).checked_mul(4)?)?;
    let channels = Slab::<Channel>::worst_case(count)?
        .checked_add(endpoints)?
        .checked_add(Map::<Token, Id<Channel>>::worst_case(count)?)?;
    // Each worker: the attempts it hosts, and the workstream keys it holds.
    let hosts = Set::<Id<Attempt>>::worst_case(limits.slots)?;
    let keys = Map::<u64, u64>::worst_case(limits.workstreams)?;
    // Each configured charter owns a CharterNeeds, and each host keeps one bool
    // per charter. Model payloads can only lower this maximum charter count.
    let needs = needs_bytes(limits)?;
    let eligibility = needs.checked_add(needs.checked_div(u64::try_from(size_of::<crate::CharterNeeds>()).ok()?)?)?;
    let workers = u64::from(limits.workers).checked_mul(hosts.checked_add(keys)?.checked_add(eligibility)?)?;
    let engine = if limits.engine_slots > 0 {
        Set::<Id<Attempt>>::worst_case(limits.engine_slots)?
            .checked_add(Map::<u64, u64>::worst_case(0)?)?
            .checked_add(eligibility)?
    } else {
        0
    };
    let tracked = tracked(limits)?;
    let attempts = Slab::<Attempt>::worst_case(tracked)?
        .checked_add(Map::<(Token, Token), Id<Attempt>>::worst_case(tracked)?)?
        .checked_add(Map::<Token, Run>::worst_case(tracked)?)?
        .checked_add(Map::<u64, Id<Attempt>>::worst_case(tracked)?)?
        .checked_add(Deadlines::<Id<Attempt>>::worst_case(tracked)?)?
        .checked_add(
            u64::from(tracked).checked_mul(skein_lib::List::<crate::grants::Kept>::worst_case(limits.grants)?)?,
        )?
        .checked_add(u64::from(tracked).checked_mul(limits.call_name_bytes.checked_add(limits.progress_bytes)?)?)?;
    let calls = Slab::<PendingCall>::worst_case(limits.calls)?
        .checked_add(u64::from(limits.calls).checked_mul(limits.call_name_bytes)?)?;
    let turns = Map::<(Id<Attempt>, u32), Pending>::worst_case(limits.turns)?;
    let facts = Queue::<Fact>::worst_case(limits.facts)?;
    let views = skein_lib::List::<crate::HostView>::worst_case(count.checked_add(limits.history)?)?
        .checked_add(skein_lib::List::<crate::AttemptView>::worst_case(tracked)?)?
        .checked_add(skein_lib::List::<crate::HelloRefusalRecord>::worst_case(limits.history)?)?;
    let sources = crate::sources::worst_case(limits)?;
    sources
        .checked_add(configured)?
        .checked_add(catalogue)?
        .checked_add(u64::from(count).checked_mul(evidence)?)?
        .checked_add(channels)?
        .checked_add(workers)?
        .checked_add(engine)?
        .checked_add(attempts)?
        .checked_add(calls)?
        .checked_add(turns)?
        .checked_add(facts)?
        .checked_add(crate::history::worst_case(limits.history)?)?
        .checked_add(needs)?
        .checked_add(views)
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
