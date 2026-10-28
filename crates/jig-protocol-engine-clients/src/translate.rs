//! Total translations of the core's client vocabulary (protocol/client.md, 3).
use alloc::boxed::Box;
use jig_client_protocol_wire::v19 as w;
use skein_lib::{List, Writer};
pub mod accounts;
pub mod activity;
pub(crate) mod answers;
pub(crate) mod asks;
mod catalogue;
pub mod chips;
pub(crate) mod cursor;
pub mod deployment;
pub mod history;
pub mod inbox;
pub mod live;
pub mod llm;
pub mod objects;
pub(crate) mod operations;
pub mod owner;
pub mod pages;
pub mod programs;
pub(crate) mod queries;
pub(crate) mod reads;
pub(crate) mod refusals;
pub mod settings;
pub mod task;
mod task_wait;
pub(crate) mod watches;
pub use objects::document;
pub use refusals::limit_bound;

fn problem(reason: skein_codec::Reason) -> w::Problem {
    w::Problem { path: w::Path::OwnedRecord, reason }
}
fn count(value: usize) -> Result<u32, w::Problem> {
    match u32::try_from(value) {
        Ok(value) => Ok(value),
        Err(_) => Err(problem(skein_codec::Reason::Bound)),
    }
}
fn pieces(rows: &List<Box<[u8]>>, bound: u32, path: w::Path) -> Result<Box<[u8]>, w::Problem> {
    let mut size = 0_u32;
    for row in rows {
        let len = count(row.len())?;
        size = size
            .checked_add(len)
            .ok_or(w::Problem { path, reason: skein_codec::Reason::Bound })?
            .checked_add(4)
            .ok_or(w::Problem { path, reason: skein_codec::Reason::Bound })?;
        if size > bound {
            return Err(w::Problem { path, reason: skein_codec::Reason::Bound });
        }
    }
    let mut out = Writer::new(usize::try_from(size).expect("u32 pieces fit usize"));
    for row in rows {
        match skein_codec::piece::write(&mut out, row) {
            Ok(()) => (),
            Err(_) => return Err(w::Problem { path, reason: skein_codec::Reason::Bound }),
        }
    }
    Ok(out.finish())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Untranslatable {
    pub path: w::Path,
}

mod me;

mod account_hosts;
