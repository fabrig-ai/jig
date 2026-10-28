//! The native client's HTTP transport (protocol/client.md, sections 8 and 12).
#![forbid(unsafe_code)]
mod connections;
pub mod diagnostics;
mod files;
mod shell;
pub use connections::{Conn, Connections, Limits, valid};
pub use shell::{Settings, Shell, Transport};
#[cfg(test)]
mod tests;
