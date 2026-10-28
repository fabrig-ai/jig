//! Child limits remain in their owners' vocabulary (hosts.md, section 11).
use crate::{domain::Call, workspace::Content};
use alloc::boxed::Box;
use jig_domain_host as host;
use skein_lib::{Duration, Map, Token};
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

#[must_use]
pub fn worst_case(limits: &Limits) -> Option<u64> {
    smith::check(&limits.agent).ok()?;
    if !host::valid_program_endpoints(&limits.bounds.program_endpoints) {
        return None;
    }
    let hello_endpoints = u64::try_from(limits.bounds.program_endpoints.len()).ok()?.checked_mul(4)?;
    if limits.hub.slots > limits.agent.agents {
        return None;
    }
    let calls = limits.hub.slots.checked_mul(limits.agent.calls)?;
    let call =
        limits.hub.event_bytes.checked_add(limits.agent.call_bytes)?.checked_add(smith::Delivered::worst_case())?;
    host::worst_case(&limits.hub)?
        .checked_add(Map::<Token, Token>::worst_case(limits.hub.slots)?)?
        .checked_add(Map::<(Token, Token), Call>::worst_case(calls)?)?
        .checked_add(u64::from(calls).checked_mul(call)?)?
        .checked_add(Map::<Token, Box<[u8]>>::worst_case(limits.hub.slots)?)?
        .checked_add(u64::from(limits.hub.slots).checked_mul(u64::from(limits.agent.detail_bytes))?)?
        .checked_add(Map::<Token, Option<Content>>::worst_case(limits.hub.slots)?)?
        .checked_add(u64::from(limits.hub.slots).checked_mul(limits.agent.call_bytes)?)?
        .checked_add(hello_endpoints)
}
