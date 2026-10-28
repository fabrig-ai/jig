//! Keep: frame composition, owned bindings and two slot-fill passes.
//! Replace: application families, connector fragments, addresses and loaded phrases.
//! The domain owns decisions; this view routes meanings and prints supplied facts.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
extern crate alloc;
pub mod bindings;
pub mod drawing;
pub mod infrastructure;
pub mod limits;
pub mod observability;
pub mod root;
pub mod time;
pub mod words;
pub use jig_ops_client_domain::Action;
pub use limits::{Limits, arrangement, valid};
pub use root::{View, build, decode};
#[cfg(test)]
mod tests;
