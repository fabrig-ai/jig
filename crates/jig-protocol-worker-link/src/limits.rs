//! Bounds supplied by the layer (protocol/channel.md, 11).
use crate::{Component, Unusable};
use alloc::boxed::Box;
use core::mem::size_of;
use jig_domain_host as host;
use jig_protocol_worker_wire::{
    self as wire,
    sizes::{self, Sizes},
};
use skein_channel::{self as channel, Role};
use skein_lib::List;
use skein_lib::{Duration, Queue};
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Limits {
    pub sizes: Sizes,
    pub chunk: u32,
    pub handshake: Duration,
    pub ping: Duration,
    pub silence: Duration,
    pub stall: Duration,
    pub backoff: Duration,
    pub ceiling: Duration,
    pub proved: Duration,
    pub grace: Duration,
    pub busy: Duration,
}
pub const MAX_OUT_DOWN: u32 = 96;
pub const MAX_OUT_FIRE: u32 = 12;
pub fn channel_limits(limits: &Limits) -> Result<channel::Limits, Unusable> {
    let bodies = sizes::limits(&limits.sizes).ok_or(Unusable::Limits)?;
    let cap = sizes::worker_cap(&limits.sizes).ok_or(Unusable::Limits)?;
    let mut skip = 0;
    for rule in wire::KINDS {
        let largest = sizes::largest_body(&limits.sizes, rule.body).ok_or(Unusable::Limits)?;
        skip = skip.max(largest);
    }
    let credential = wire::Credential::worst_case_bytes(&bodies).ok_or(Unusable::Limits)?;
    let Ok(credential) = u32::try_from(credential) else { return Err(Unusable::Limits) };
    let Ok(kinds) = u32::try_from(wire::KINDS.len()) else { return Err(Unusable::Limits) };
    Ok(channel::Limits {
        chunk: limits.chunk,
        credential,
        skip,
        output_bytes: cap.cap.bytes,
        output_frames: cap.cap.frames,
        kinds,
    })
}
/// Whether these limits are representable: every edge bound they size
/// is finite and every capacity they derive fits (programming-model.md, 6.3).
#[must_use]
pub fn valid(limits: &Limits) -> bool {
    if limits.ping == Duration::ZERO
        || limits.silence < limits.ping.saturating_mul(3)
        || limits.stall == Duration::ZERO
        || limits.grace == Duration::ZERO
        || limits.busy == Duration::ZERO
        || limits.backoff == Duration::ZERO
        || limits.ceiling < limits.backoff
        || limits.handshake == Duration::ZERO
        || limits.proved == Duration::ZERO
    {
        return false;
    }
    let Some(bodies) = sizes::limits(&limits.sizes) else {
        return false;
    };
    let Ok(schema) = wire::schema(&bodies) else {
        return false;
    };
    let Ok(channel) = channel_limits(limits) else {
        return false;
    };
    schema.check(Role::Initiator, &channel).is_ok()
        && channel_capacity(&channel).is_some()
        && limits.sizes.slots.checked_mul(limits.sizes.calls).is_some()
        && limits.sizes.slots.checked_mul(limits.sizes.turns).is_some()
        && limits.sizes.accounts.checked_add(limits.sizes.credentials).is_some()
        && u64::from(channel.skip).checked_add(8).is_some()
        && inventory_bound(&bodies).is_some()
}

#[must_use]
pub fn worst_case(limits: &Limits) -> Option<u64> {
    if !valid(limits) {
        return None;
    }
    let bodies = sizes::limits(&limits.sizes)?;
    let schema = wire::schema(&bodies).ok()?;
    let channel = channel_limits(limits).ok()?;
    let bound = channel::worst_case(&schema, Role::Initiator, &channel)?;
    let state = u64::try_from(size_of::<Component>()).ok()?;
    let source_rows = List::<host::Login>::worst_case(limits.sizes.program_sources)?
        .checked_add(u64::from(limits.sizes.program_sources).checked_mul(u64::from(limits.sizes.detail))?)?;
    let pending_inventory = wire::Logins::worst_case_bytes(&bodies)?.checked_add(8)?;
    // One typed/codec row representation and one latest encoded inventory.
    bound
        .checked_add(source_rows)?
        .checked_add(pending_inventory)?
        .checked_add(state)?
        .checked_add(crate::relays::Relays::worst_case(limits)?)?
        .checked_add(crate::busy::Busy::worst_case(limits)?)?
        // Transient body and framed output copies during record translation.
        .checked_add(u64::from(channel.skip).checked_add(8)?.checked_mul(2)?)?
        .checked_add(List::<Box<[u8]>>::worst_case(limits.sizes.transcript.checked_div(4)?)?)?
        .checked_add(List::<host::StartMessage>::worst_case(limits.sizes.inbox)?)?
        .checked_add(List::<host::AnsweredCall>::worst_case(limits.sizes.answered)?)?
        .checked_add(List::<host::Grant>::worst_case(limits.sizes.accounts.checked_add(limits.sizes.credentials)?)?)?
        .checked_add(List::<wire::Part>::worst_case(1)?)?
        .checked_add(u64::from(limits.sizes.name))?
        .checked_add(u64::from(limits.sizes.secret))?
        .checked_add(Queue::<channel::Event>::worst_case(channel::MAX_UP.checked_mul(3)?)?)?
        .checked_add(Queue::<channel::Lower>::worst_case(channel::MAX_DOWN.checked_mul(3)?)?)
}

fn inventory_bound(bodies: &wire::v9::Limits) -> Option<u64> {
    wire::Logins::worst_case_bytes(bodies)?.checked_add(8)
}

// The channel has no separate validity API at this pin. Keep its derived
// queue capacity and control-frame bounds, without its retained heap price.
fn channel_capacity(limits: &skein_channel::Limits) -> Option<u32> {
    limits.output_frames.checked_add(3)?;
    let terms = limits.kinds.checked_mul(6)?.checked_add(4)?;
    let open = limits.credential.checked_add(14)?;
    open.max(terms).max(262).checked_add(8)
}
