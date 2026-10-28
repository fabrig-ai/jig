//! Keep: the child routing shape and exhaustive connector variants.
//! Replace: Ops connector sides, family addresses and application vocabulary.
//! The children keep documents, visibility and pending acts. The root retains
//! only those children and the client (client/domain/root.md, 3–10).
//! Each step routes one event; the client saves requests before keyed sends.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
extern crate alloc;
pub mod address;
pub mod boundary;
pub mod domain;
mod family;
pub mod infrastructure;
pub mod limits;
mod me;
pub mod observability;
mod parts;
mod route;
pub mod store;
pub mod wire;
pub use address::{Address, Place};
pub use boundary::{Action, Answered, Ask, Event, Fact, Request};
pub use domain::{
    Domain, configure_arrangement, configure_navigation, fire, new, next_deadline, shown, step, take, unlinked,
};
pub use limits::{Limits, worst_case};
#[cfg(test)]
mod tests;
