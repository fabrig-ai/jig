//! The test root's output room and edge validity (service.md, 4).
use crate::Limits;
use jig_domain_core as core;
use jig_test_connector as connector;

/// Room for one release, including the door, deferred inputs and a commit.
#[must_use]
pub fn max_out(limits: &Limits) -> u32 {
    limits
        .journal
        .release
        .checked_add(limits.journal.now)
        .expect("root release room")
        .checked_add(limits.routes)
        .expect("deferred input room")
        .checked_add(2)
        .expect("commit and stop room")
}

/// Whether the root's child limits and output edges are representable.
#[must_use]
pub fn valid(limits: &Limits) -> bool {
    checked_valid(limits).is_some()
}
fn checked_valid(limits: &Limits) -> Option<()> {
    let room = core::route_bound(&limits.core)?;
    let connectors = connector::MAX_OUT.checked_mul(2)?;
    if limits.routes == 0
        || room.checked_add(connectors)? > limits.journal.writes
        || room.checked_add(connectors)? > limits.journal.held
        || !core::valid(&limits.core)
        || !connector::valid(&limits.connector)
    {
        return None;
    }
    core::reads::answer_worst_case(&limits.core)?;
    connector::document_worst_case(&limits.connector)?;
    Some(())
}
