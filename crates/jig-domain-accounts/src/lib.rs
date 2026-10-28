//! Accounts and their sources, component grants and usability (domain/accounts.md, section 3;
//! domain/engine.md, section 12). Each account is the deployment's or a party's, numbered by a
//! counter that never gives a number twice, with its sources in its holder's order; each source
//! keeps its own state. The component owns refreshing, records and generation numbering. This
//! child keeps names, validity, limits, refusals and failures, and releases all wants at close.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
extern crate alloc;
mod boundary;
mod changes;
mod checks;
mod domain;
mod keys;
mod limits;
mod ordering;
pub mod programs;
mod providers;
mod records;
mod restore;
mod selection;
mod signing;
pub use boundary::{
    ADDRESS, Answer, Change, Deadline, End, Ended, Event, Fact, Failure, Form, FromComponent, Grant, GrantTable,
    KeyFailure, Pending, Place, Program, ProgramFailure, ProgramHost, ProgramResult, Request, Scheme, Signing, State,
    ToComponent, Turned, Unread,
};
pub use checks::{Bound, LimitKey, NumericLimit, Refusal, SignIn, SignInRedirectWhy, room_for_sign_in};
pub use domain::{Account, Domain, fire, max_out, step};
pub use keys::{KEY_VALID, PROGRAM_VALID};
pub use limits::{Limits, worst_case};
pub use providers::{MANAGE_USAGE, Offers, PROVIDER_NAME, Provider, Providers};
pub use records::{DISPLAY, Held, Holder, Key, Kind, NAME, SUBJECT, Stored, USES, Use};
pub use selection::{Coverage, Excluded, HostSource, Selection, WaitWhy};

pub use jig_domain_workers::{SourceName, Worker};
