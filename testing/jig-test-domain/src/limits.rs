//! The test root's output reservation and conservative heap bound (service.md, 4).
use crate::{Assignment, Delivery, Limits, Write};
use alloc::boxed::Box;
use jig_domain_authority as authority;
use jig_domain_core as core;
use jig_domain_tasks as tasks;
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

/// Conservative pricing of every child and the retained root records.
#[must_use]
pub fn worst_case(limits: &Limits) -> Option<u64> {
    use skein_lib::{Journal, Map, Queue, Token};
    let room = core::room_max(&limits.core)?;
    let connectors = connector::MAX_OUT.checked_mul(2)?;
    if limits.routes == 0
        || room.writes.checked_add(connectors)? > limits.journal.writes
        || room.held.checked_add(connectors)? > limits.journal.held
    {
        return None;
    }

    let children =
        core::worst_case(&limits.core)?.checked_add(connector::worst_case(&limits.connector)?.checked_mul(2)?)?;
    let slots = u64::from(limits.journal.writes)
        .checked_mul(u64::from(limits.journal.commits).checked_add(1)?)?
        .checked_add(u64::from(limits.journal.held).checked_mul(2)?)?
        .checked_add(u64::from(limits.journal.now))?
        .checked_add(u64::from(limits.core.tasks.tasks).checked_mul(3)?)?
        .checked_add(u64::from(limits.core.call_records))?
        .checked_add(u64::from(limits.core.fleet.turns))?
        .checked_add(u64::from(limits.core.fleet.attempts))?;
    let slots = slots.checked_add(u64::from(limits.routes))?;
    let entries = limits.connector.entries.checked_mul(2)?;
    children
        .checked_mul(slots.checked_add(2)?)?
        .checked_add(Journal::<Write, Delivery>::worst_case(&limits.journal)?)?
        .checked_add(
            core::reads::answer_worst_case(&limits.core)?
                .max(connector::document_worst_case(&limits.connector)?)
                .checked_mul(u64::from(limits.journal.held).checked_add(u64::from(limits.journal.release))?)?,
        )?
        .checked_add(Queue::<core::Now>::worst_case(limits.journal.now)?)?
        .checked_add(Queue::<Box<crate::Event>>::worst_case(limits.routes)?)?
        .checked_add(Map::<Token, (skein_lib::ReplyTo, core::CallKey, u16)>::worst_case(limits.core.call_records)?)?
        .checked_add(Map::<u64, Assignment>::worst_case(limits.core.tasks.tasks)?)?
        .checked_add(Map::<u64, Box<[jig_domain_brief::GatherPlaced]>>::worst_case(limits.core.tasks.tasks)?)?
        .checked_add(
            skein_lib::List::<jig_domain_brief::GatherPlaced>::worst_case(limits.core.brief.sections)?
                .checked_add(u64::from(limits.core.brief.brief_bytes))?
                .checked_mul(u64::from(limits.core.tasks.tasks).checked_add(u64::from(limits.journal.held))?)?,
        )?
        .checked_add(Map::<u64, Box<[tasks::Name]>>::worst_case(limits.core.tasks.tasks)?)?
        .checked_add(Map::<u64, (u64, u16)>::worst_case(limits.core.tasks.tasks)?)?
        .checked_add(Map::<Token, crate::ProjectionPayload>::worst_case(
            limits.core.call_records.checked_add(limits.core.tasks.tasks)?,
        )?)?
        .checked_add(Map::<Token, u64>::worst_case(limits.core.tasks.tasks)?)?
        .checked_add(Map::<u64, u16>::worst_case(entries)?)?
        .checked_add(Map::<u64, crate::OutboxOwner>::worst_case(entries)?)?
        .checked_add(Map::<Token, (Token, authority::Judge, [u8; 32])>::worst_case(limits.core.authority.facts)?)
}
