//! The application-held client (client/domain/client.md, 3 and 4).
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
extern crate alloc;
pub mod accounts;
mod client;
pub mod facts;
mod limits;
pub mod shapes;
mod store;
pub mod thread;
pub mod words;
pub use client::{Admission, Client, Event, LinkView, NotSignedIn, Request};
pub use jig_client_domain::Limits as DomainLimits;
pub use jig_client_domain::{
    AccountCounts, Answered, Ask, Asker, Backoff, Document, Down, Family, Handle, Held, Key, Latest, Link, Outcome,
    Pending, Progress, Readable, Readout, Retry, Spoken, State, Told, Up, Watchable, Why, contract,
};
pub use jig_client_protocol_link::{AuthorizeWhy, Authorized, Message, Record, STREAM_KEY, Streamed, With};
pub use limits::{Limits, Unusable, valid, worst_case};
#[cfg(test)]
mod tests;
