//! The worker's channel owner (protocol/channel.md, 4 and 10.1).
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
extern crate alloc;
mod backoff;
mod boundary;
mod busy;
mod component;
mod connection;
mod grace;
mod limits;
mod relays;
mod translate;
pub use boundary::{Event, GrantValue, Request, Scheme};
pub use component::{Component, Credential, Unusable};
pub use limits::{Limits, MAX_OUT_DOWN, MAX_OUT_FIRE, channel_limits, valid};

/// The peer's terms do not suit this side (skein's channel.md, 5.1).
pub const REFUSED_LIMITS: u16 = 2;
/// A malformed record (skein's channel.md, 5.1).
pub const REFUSED_FRAMING: u16 = 3;

#[cfg(test)]
mod tests;
