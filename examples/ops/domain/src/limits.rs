//! Admission paces new work by the journal backlog watermark.
//! Connector and endpoint limits are application configuration (domain/root.md, 10).
use crate::boundary::Payload;
use crate::domain::{Assignment, HostCall, ReadCall, Work};
use crate::{Output, Write};
use alloc::boxed::Box;
use jig_domain_authority as authority;
use jig_domain_core as core;
use jig_domain_host as host;
use jig_ops_domain_infrastructure as infrastructure;
use jig_ops_domain_observability as observability;
use jig_protocol_charter as charter;
use skein_lib::{Journal, JournalLimits, Map, Queue, Token};

/// Each child receives its own limits; the root owns its admission pacing.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// The core and its children.
    pub core: core::Limits,
    /// Observability's working set and outbox.
    pub observability: observability::Limits,
    /// Infrastructure's working set and outbox.
    pub infrastructure: infrastructure::Limits,
    /// Engine-slot run hub.
    pub host: host::Limits,
    /// The charter codec's bounds.
    pub charter: smith_charter::v4::Limits,
    /// How many endpoint names the charter codec holds.
    pub endpoints: u32,
    /// The one commit barrier.
    pub journal: JournalLimits,
    /// New external work waits while the backlog is at or past this many
    /// entries (skein's draft `dynamic-memory.md`, 4.4); a pacing limit,
    /// not a memory one.
    pub watermark: u32,
    /// Maximum synchronous continuations for one admitted event.
    pub routes: u32,
}

/// Child states, journal, and bounded child-requested continuations.
/// Owned payload bytes are priced wherever the root retains or queues them;
/// values moved out of a child are no longer covered by that child's state.
#[must_use]
pub fn worst_case(limits: &Limits) -> Option<u64> {
    let calls = limits.host.slots.checked_mul(limits.host.run_calls)?;
    let effects = limits.core.call_records.checked_add(limits.core.tasks.tasks)?;
    let payloads = limits.core.fleet.turns.checked_add(limits.core.fleet.attempts)?;
    let children = core::worst_case(&limits.core)?
        .checked_add(observability::worst_case(&limits.observability)?)?
        .checked_add(infrastructure::worst_case(&limits.infrastructure)?)?
        .checked_add(host::worst_case(&limits.host)?)?;
    let tables = Map::<u64, Assignment>::worst_case(limits.core.tasks.tasks)?
        .checked_add(Map::<u64, Box<[charter::Section]>>::worst_case(limits.core.tasks.tasks)?)?
        .checked_add(Map::<Token, ReadCall>::worst_case(limits.observability.reads)?)?
        .checked_add(Map::<Token, Payload>::worst_case(payloads)?)?
        .checked_add(Map::<Token, infrastructure::Effect>::worst_case(effects)?)?
        .checked_add(Map::<Token, (Token, authority::Judge, [u8; 32])>::worst_case(limits.core.authority.facts)?)?
        .checked_add(Map::<u64, (u64, u16, u32)>::worst_case(limits.core.tasks.tasks)?)?
        .checked_add(Map::<Token, u64>::worst_case(limits.core.tasks.tasks)?)?
        .checked_add(Map::<Token, [u8; 16]>::worst_case(limits.infrastructure.made)?)?
        .checked_add(Map::<Box<[u8]>, HostCall>::worst_case(calls)?)?
        .checked_add(skein_lib::Deadlines::<(Token, Token, u32)>::worst_case(
            limits.host.slots.checked_mul(limits.host.turns)?,
        )?)?;
    // A complete assignment includes its charter, transcript, grants and
    // settled call tail. Held documents and provisional suffixes have their
    // own independent bounds; the queued carrier below takes their maximum.
    let assignment = limits
        .host
        .charter_bytes
        .checked_add(limits.host.transcript_bytes)?
        .checked_add(u64::from(limits.core.call_records).checked_mul(u64::from(limits.core.call_answer_bytes))?)?
        .checked_add(u64::from(limits.host.accounts).checked_mul(64)?)?
        .checked_add(skein_lib::List::<jig_domain_tasks::Word>::worst_case(limits.core.tasks.inbox_messages)?)?
        .checked_add(u64::from(limits.core.tasks.inbox_bytes))?;
    let value =
        assignment
            .checked_add(limits.core.policy_bytes)?
            .checked_add(u64::from(limits.core.tasks.batch).checked_mul(
                u64::from(limits.core.run_bytes).checked_add(u64::from(limits.core.tasks.message_bytes))?,
            )?)?
            .checked_add(limits.host.turn_bytes)?
            .checked_add(limits.host.outcome_bytes)?
            .checked_add(u64::from(limits.observability.answer_bytes))?
            .checked_add(65_536)?
            .max(core::reads::answer_worst_case(&limits.core)?)
            .max(infrastructure::document_worst_case(&limits.infrastructure)?)
            .max(observability::document_worst_case(&limits.observability)?)
            .max(u64::from(limits.core.views.text_bytes).checked_mul(2)?);
    let envelopes = u64::from(limits.observability.reads.checked_add(calls)?)
        .checked_mul(limits.core.fleet.call_name_bytes.checked_add(32)?)?;
    let owned = u64::from(limits.core.tasks.tasks)
        .checked_mul(assignment)?
        .checked_add(u64::from(limits.core.tasks.tasks).checked_mul(u64::from(limits.core.brief.brief_bytes))?)?
        .checked_add(envelopes)?
        .checked_add(limits.host.charter_bytes)?
        .checked_add(u64::from(payloads).checked_mul(value)?)?
        .checked_add(u64::from(effects).checked_mul(u64::from(limits.infrastructure.name_bytes).checked_mul(8)?)?)?
        .checked_add(u64::from(limits.endpoints).checked_mul(limits.host.charter_bytes.checked_add(64)?)?)?;
    let queued = limits
        .journal
        .writes
        .checked_add(limits.journal.held)?
        .checked_add(limits.journal.now)?
        .checked_add(limits.routes)?;
    children
        .checked_add(tables)?
        .checked_add(owned)?
        .checked_add(Journal::<Write, Output>::worst_case(&limits.journal)?)?
        .checked_add(Queue::<Work>::worst_case(limits.routes)?)?
        .checked_add(u64::from(queued).checked_mul(value)?)?
        // Tokenization and the bounded JSON nesting stack coexist with a
        // decoded tool input only during its synchronous translation.
        .checked_add(67_108_864)
}
