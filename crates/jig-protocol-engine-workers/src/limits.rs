//! Checked channel capacity (protocol/channel.md, section 11).
use crate::{Request, Worker, connection::Connection};
use core::mem::size_of;
use jig_protocol_worker_wire::{
    self as wire,
    sizes::{self, Sizes},
};
use skein_channel::{Role, Schema};
use skein_lib::{Duration, Map, Queue, Token};
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Limits {
    pub sizes: Sizes,
    pub names: u32,
    pub workers: u32,
    pub opening: u32,
    pub chunk: u32,
    pub handshake: Duration,
    pub hello: Duration,
    pub ping: Duration,
    pub silence: Duration,
    pub stall: Duration,
}
// One input and the resulting channel events, with one older channel refused.
pub const MAX_OUT_DOWN: u32 = 32;
// One expired connection per call; next_deadline retains other due connections.
pub const MAX_OUT_FIRE: u32 = 16;
pub(crate) fn channel(limits: &Limits) -> Option<skein_channel::Limits> {
    let codec = sizes::limits(&limits.sizes)?;
    let credential = u32::try_from(wire::Credential::worst_case_bytes(&codec)?).ok()?;
    let cap = sizes::engine_cap(&limits.sizes)?;
    Some(skein_channel::Limits {
        chunk: limits.chunk,
        credential,
        skip: limits.chunk,
        output_bytes: cap.bytes,
        output_frames: cap.frames,
        kinds: u32::try_from(wire::KINDS.len()).ok()?,
    })
}
pub(crate) fn schema(limits: &Limits) -> Option<Schema> {
    wire::schema(&sizes::limits(&limits.sizes)?).ok()
}
/// Whether these limits are representable: every edge bound they size
/// is finite and every capacity they derive fits (programming-model.md, 6.3).
#[must_use]
pub fn valid(limits: &Limits) -> bool {
    if limits.handshake == Duration::ZERO
        || limits.hello == Duration::ZERO
        || limits.stall == Duration::ZERO
        || limits.ping == Duration::ZERO
        || limits.silence < limits.ping.saturating_mul(3)
    {
        return false;
    }
    let Some(schema) = schema(limits) else {
        return false;
    };
    let Some(channel) = channel(limits) else {
        return false;
    };
    let Some(codec) = sizes::limits(&limits.sizes) else {
        return false;
    };
    limits.workers.checked_add(limits.opening).is_some()
        && schema.check(Role::Responder, &channel).is_ok()
        && channel_capacity(&channel).is_some()
        && wire::v9::worst_case_heap(&codec).is_some()
}

#[must_use]
pub fn worst_case(limits: &Limits) -> Option<u64> {
    if !valid(limits) {
        return None;
    }
    let connections = limits.workers.checked_add(limits.opening)?;
    let schema = schema(limits)?;
    let channel = channel(limits)?;
    let each = skein_channel::worst_case(&schema, Role::Responder, &channel)?;
    let codec = sizes::limits(&limits.sizes)?;
    // A decoded value and its converted output, with part slots, for each request and one transient.
    // Login rows store validated names/methods inline, so price their typed
    // representation even when small wire payloads would dominate no other record.
    let sources = skein_lib::List::<jig_domain_host::Login>::worst_case(limits.sizes.program_sources)?
        .checked_add(u64::from(limits.sizes.program_sources).checked_mul(u64::from(limits.sizes.detail))?)?;
    let records = wire::v9::worst_case_heap(&codec)?
        .max(sources)
        .checked_mul(2)?
        .checked_add(skein_lib::List::<wire::Part>::worst_case(3)?)?
        .checked_mul(u64::from(MAX_OUT_DOWN).checked_add(1)?)?;
    let names = u64::from(limits.names)
        .checked_mul(u64::try_from(size_of::<Worker>()).ok()?.checked_add(u64::from(limits.sizes.name))?)?;
    let table = Map::<Token, Connection>::worst_case(connections)?;
    let schema_heap =
        skein_lib::List::<skein_channel::Version>::worst_case(1)?
            .checked_add(skein_lib::List::<skein_channel::Kind>::worst_case(channel.kinds)?)?;
    table
        .checked_add(each.checked_mul(u64::from(connections))?)?
        .checked_add(names)?
        .checked_add(records)?
        .checked_add(schema_heap)?
        .checked_add(Queue::<Request>::worst_case(MAX_OUT_DOWN)?)
}

// The channel has no separate validity API at this pin. Keep its derived
// queue capacity and control-frame bounds, without its retained heap price.
fn channel_capacity(limits: &skein_channel::Limits) -> Option<u32> {
    limits.output_frames.checked_add(3)?;
    let terms = limits.kinds.checked_mul(6)?.checked_add(4)?;
    let open = limits.credential.checked_add(14)?;
    open.max(terms).max(262).checked_add(8)
}
