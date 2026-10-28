//! The engine's worker channels (protocol/channel.md, sections 4 and 10.2).
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
extern crate alloc;
pub mod boundary;
mod component;
mod connection;
pub mod limits;
mod opening;
mod translate;
pub use boundary::{Event, GrantValue, Request, Scheme, Worker};
pub use component::{Component, Unusable};
pub use limits::{Limits, MAX_OUT_DOWN, MAX_OUT_FIRE, valid};
#[cfg(test)]
mod tests;
