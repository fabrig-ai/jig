//! Client attempts and envelopes (protocol/client.md, 2, 5 and 13).
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
extern crate alloc;
mod authorize;
pub mod boundary;
mod component;
pub mod limits;
mod sign_in;
mod silence;
mod store;
pub mod translate;
mod watches;
pub use boundary::*;
/// The kept name of the preceding client's stream (protocol/client.md, 10).
pub const STREAM_KEY: &[u8] = b"stream";
pub use component::Link;
pub use limits::{Limits, Unusable, valid, worst_case};
pub use translate::Undecodable;
#[cfg(test)]
mod tests;

/// Whether the engine serves this client's wire version (protocol/client.md, 9).
#[must_use]
pub const fn serves(versions: jig_client_domain::contract::VersionRange) -> bool {
    versions.contains(jig_client_protocol_wire::VERSION)
}
