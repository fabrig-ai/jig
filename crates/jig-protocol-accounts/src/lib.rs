//! Accounts at the protocol boundary (protocol/llm.md, sections 5 and 6).
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
extern crate alloc;
#[rustfmt::skip]
#[path = "generated/v1.rs"]
pub mod v1;
pub mod finishes;
pub mod installation;
pub mod keys;
pub mod moved;
#[cfg(test)]
mod moved_tests;
pub mod moves;
pub mod party;
pub mod records;
pub use records::Unfit;
pub mod restart;
pub mod table;
#[cfg(test)]
mod tests;
pub mod translate;
