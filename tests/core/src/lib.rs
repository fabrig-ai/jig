//! The core's scripted domain world (`domain/engine.md`, section 15).

#![forbid(unsafe_code)]

mod charters;
pub mod direct;
pub mod world;

mod application_observations;
pub mod client_peer;
pub mod effects;
pub mod peers;
pub mod protocol;
pub mod store_peer;

/// The testing application's real store and world faults.
pub use jig_test_store_files::{Fault, Store};

pub mod observations;
pub mod referee;

pub mod faults;

pub mod watches;

pub mod reads;

pub mod stories;

pub mod fixtures;

pub mod agents_tools;
pub mod reference;
