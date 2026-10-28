//! Client domain and API vocabulary (client/domain/client.md, 3).
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
extern crate alloc;
mod boundary;
mod carried;
pub mod contract;
mod domain;
pub mod limits;
mod link;
mod reads;
mod requests;
mod text;
mod watches;
/// The small-store version of keyed client request records.
pub const TABLE_VERSION: u16 = 2;
pub use boundary::*;
pub use domain::{Domain, fire, next_deadline, reclaim, step};
pub use limits::{AccountCounts, Backoff, Limits, VALUE_BYTES, valid};
#[cfg(test)]
mod tests;
