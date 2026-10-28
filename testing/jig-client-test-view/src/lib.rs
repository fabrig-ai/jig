//! The test application's view composition (client/domain/root.md, section 9).
//! Each child owns its words and bindings; this root only routes slots and acts.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
extern crate alloc;
pub mod bindings;
pub mod connector_values;
mod connector_values_refusal;
pub mod connector_values_words;
pub mod drawing;
pub mod family;
pub mod limits;
mod proposal;
pub mod root;
pub mod words;
pub use jig_client_test_domain::Action;
pub use limits::{Limits, valid};
pub use root::{View, build, decode};
#[cfg(test)]
mod tests;

/// Public document subjects name the family’s rendered cards.
pub use family::key as family_key;
