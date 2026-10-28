//! Child limits remain in their owners' vocabulary (hosts.md, section 11).
use alloc::boxed::Box;
use jig_domain_host as host;
use skein_lib::Duration;
use smith_host_domain as smith;

/// About twice the room one entrance's largest output once reserved
/// at the small profile (2 x 1,053 entries).
pub const WATERMARK: u32 = 2_048;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Limits {
    /// The engine's records wait while the worker's backlog is at or
    /// past this many entries (skein's draft `dynamic-memory.md`, 4.1
    /// and 4.4); a pacing limit, not a memory one.
    pub watermark: u32,
    pub hub: host::Limits,
    pub agent: smith::Limits,
    pub bounds: host::Bounds,
    pub stop_bound: Duration,
    pub workstreams: Box<[u64]>,
}

/// Smith, hub and endpoint limits checked before worker construction.
#[must_use]
pub fn valid(limits: &Limits) -> bool {
    smith::check(&limits.agent).is_ok()
        && host::valid_program_endpoints(&limits.bounds.program_endpoints)
        && host::valid(&limits.hub)
        && limits.hub.slots <= limits.agent.agents
        && limits.hub.slots.checked_mul(limits.agent.calls).is_some()
}
