//! Core host tools at the engine's protocol boundary (domain/hosts.md,
//! section 8; protocol/channel.md, section 6.3). These functions keep no state.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
extern crate alloc;
use alloc::boxed::Box;
use jig_domain_core as core;
mod declarations;
mod decode;
mod json;
mod nested;
mod notes;
mod render;
mod words;
pub use words::words;
#[cfg(test)]
mod tests;
pub use declarations::{Declaration, declarations};
pub use decode::{batch, decode};
pub use render::{problem, render, settings_answer};
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Limits {
    pub input_bytes: u32,
    pub words: u32,
    pub batch: u32,
    pub depth: u32,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Why {
    Malformed,
    Missing,
    Type,
    Range,
    TooLarge,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Problem {
    pub field: &'static [u8],
    pub why: Why,
}
#[derive(Debug)]
#[expect(clippy::large_enum_variant, reason = "the decoded amendment stays inline as in the core's vocabulary")]
pub enum Decoded {
    Action(core::NamedAction),
    /// The core world's settings stand uses the existing settings-v1 whole-set codec.
    /// Applications supply their own settings tool and value translation.
    Settings {
        against: u32,
        set: Box<[u8]>,
        reason: Box<[u8]>,
    },
    Delegate(Box<[core::Delegate]>),
    Problem(Problem),
    NotMine,
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Rendered {
    pub error: bool,
    pub text: Box<[u8]>,
}
