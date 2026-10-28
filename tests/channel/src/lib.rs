//! Joint workers channel on the simulated kernel (protocol/channel.md, 12).
pub mod fixtures;
mod processes;
pub mod referee;
mod relay;
pub mod stories;
pub mod world;

mod account_setup;
mod live_input;
mod live_text;
pub mod program_sources;

pub mod program_runs;

pub mod worker_orders;
