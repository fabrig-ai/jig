//! The test application's client tree: jig and two connector sides below one
//! typed request table. The root keeps no routing state and reads no answers.
//! Entry points follow client/domain/root.md, sections 3–10.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
extern crate alloc;
pub mod address;
pub mod boundary;
pub mod connector_bounds;
pub mod connector_values;
pub mod domain;
pub mod family;
pub mod first;
pub mod limits;
pub mod owner;
mod parts;
mod route;
pub mod second;
pub mod store;
pub use address::{Address, Place};
pub use boundary::{Action, Answered, Ask, Event, Fact, Request};
pub use domain::{Domain, configure_arrangement, fire, new, next_deadline, shown, step, take, unlinked};
pub use limits::{Limits, valid, worst_case};
pub use owner::{Connector, FIRST, SECOND};
/// The numbered dialect every fixture turn of the test application names:
/// smith's transcript v6 requires one, and the client shows an opaque
/// block's dialect by it.
pub const DIALECT: u32 = 1;
#[cfg(test)]
mod tests;
