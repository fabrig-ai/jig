//! Native client exchanges and streams (protocol/client.md, sections 2 and 8).
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
extern crate alloc;
pub mod boundary;
mod component;
mod exchange;
pub mod limits;
mod routes;
mod served;
mod sign_in;
mod stream;
pub use boundary::*;
pub use component::{Component, Unusable};
pub use limits::{Limits, MAX_OUT_DOWN, MAX_OUT_FIRE, MAX_OUT_UP, provider_limits, valid};

pub mod translate;
