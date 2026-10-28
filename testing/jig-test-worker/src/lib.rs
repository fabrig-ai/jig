//! The reference worker root (domain/hosts.md, sections 4 and 6.8).
//! Payloads cross the agent boundary by hand-out to the protocol layer.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
extern crate alloc;

pub mod boundary;
mod domain;
mod hand_out;
mod limits;
pub mod translate;
mod workspace;

pub use boundary::{Encodable, Encoded, Event, Invalid, Request};
pub use domain::{Domain, fire, step};
pub use limits::{Limits, WATERMARK, worst_case};
pub use workspace::Content;

#[cfg(test)]
mod tests;
