//! Container capacities and byte bounds owned by the client.
//! Every retained container is sized from these at new; jig 21–23 add their
//! own entities. See client/domain/client.md, 14.
use crate::{store::Record, ui::Timer};
use alloc::boxed::Box;
use skein_lib::{Deadlines, Duration, List, Map};

/// Configured capacities, payload bounds and durations for the client spine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Limits {
    pub watches: u32,
    pub notices: u32,
    pub notice_life: Duration,
    pub linger: Duration,
    pub save_delay: Duration,
    pub intents: u32,
    pub fields: u32,

    pub objects: u32,
    pub sources: u32,
    pub reads: u32,
    pub parts: u32,
    /// Live delegates that one close confirmation may dispose of.
    pub delegates: u32,
    /// Whole payload budget enforced by the decoding owner: boxes (including
    /// the root box), list capacities, texts and nested documents.
    pub document_bytes: u64,
    pub records: u32,
    /// Requests restored under jig's asker, until their terminal.
    pub kept: u32,
    /// Per-task composers kept across navigation and restart.
    pub drafts: u32,
    pub projects: u32,
    pub roles: u32,
    pub text: u32,
    pub rows: u32,
    /// Whole account lists admitted by the client (client/domain/client.md, 14).
    pub accounts: jig_client::AccountCounts,
    pub segments: u32,
    /// Whole pending or sent word snapshots kept for the displayed conversation.
    pub conversation_words: u32,
    /// Maximum configured navigation destinations.
    pub navigation_entries: u32,
    /// The largest unread update count displayed without a lower-bound mark.
    pub updates_count: u32,
}

/// Whether these limits are representable: every edge bound they size
/// is finite and every capacity they derive fits (programming-model.md, 6.3).
#[must_use]
pub fn valid(limits: &Limits) -> bool {
    if limits.watches == 0
        || limits.notice_life == Duration::ZERO
        || limits.linger == Duration::ZERO
        || limits.save_delay == Duration::ZERO
        || limits.sources == 0
        || limits.conversation_words < limits.intents
    {
        return false;
    }
    limits.watches.checked_add(limits.reads).is_some()
        && timers(limits).is_some()
        && tree_names(limits).is_some()
        && crate::page::conversations::valid(limits)
        && crate::page::inbox::valid(limits)
        && crate::page::task::valid(limits)
        && crate::page::project::valid(limits)
        && crate::notices::kept::valid(limits)
        && crate::objects::valid(limits)
        && crate::windows::valid(limits)
}

/// The heap held by the implemented spine, including page reads and record payloads.
#[must_use]
pub fn worst_case(limits: &Limits) -> Option<u64> {
    if !valid(limits) {
        return None;
    }
    let timer_capacity = timers(limits)?;
    let mut total = Map::<jig_client::Handle, crate::handles::Watched>::worst_case(limits.watches)?;
    total = total.checked_add(List::<jig_client::contract::ObjectKey>::worst_case(tree_names(limits)?)?)?;
    total = total.checked_add(List::<crate::page::Section>::worst_case(crate::page::MOST_SECTIONS)?)?;
    total = total.checked_add(List::<crate::page::SectionStatus>::worst_case(crate::page::MOST_SECTIONS)?)?;
    total = total.checked_add(crate::application::worst_case(limits)?)?;
    total = total.checked_add(jig_client::thread::worst_case(&jig_client::thread::Limits {
        lines: limits.rows.max(limits.conversation_words),
        bytes: limits.document_bytes,
        text: limits.text,
    })?)?;
    total = total.checked_add(List::<crate::page::navigation::Destination>::worst_case(limits.navigation_entries)?)?;
    total = total.checked_add(crate::page::conversations::worst_case(limits)?)?;
    total = total.checked_add(crate::page::inbox::worst_case(limits)?)?;
    total = total.checked_add(crate::page::task::worst_case(limits)?)?;
    total = total.checked_add(crate::page::project::worst_case(limits)?)?;
    total = total.checked_add(crate::page::deployment::worst_case(limits)?)?;
    total = total.checked_add(crate::intents::worst_case(limits)?)?;
    total = total.checked_add(crate::notices::kept::worst_case(limits)?)?;
    total = total.checked_add(crate::fields::worst_case(limits)?)?;
    total = total.checked_add(crate::drafts::worst_case(limits)?)?;
    total = total.checked_add(List::<u64>::worst_case(limits.records)?)?;
    total = total.checked_add(List::<jig_client::contract::ObjectKey>::worst_case(limits.objects)?)?;
    total = total.checked_add(Map::<jig_client::Key, u64>::worst_case(limits.kept)?)?;
    total = total.checked_add(crate::confirm::worst_case(limits)?)?;
    total = total.checked_add(List::<crate::objects::Context>::worst_case(limits.rows.checked_mul(2)?)?)?;
    total = total.checked_add(crate::objects::worst_case(limits)?)?;
    total = total.checked_add(Map::<jig_client::contract::ObjectKey, Box<jig_client::contract::Ask>>::worst_case(
        limits.objects,
    )?)?;
    total = total.checked_add(Map::<jig_client::Handle, crate::handles::Reading>::worst_case(limits.reads)?)?;
    total = total.checked_add(u64::from(limits.reads).checked_mul(limits.document_bytes)?)?;
    total = total.checked_add(crate::windows::worst_case(limits)?)?;
    total = total.checked_add(Deadlines::<Timer>::worst_case(timer_capacity)?)?;
    total = total.checked_add(List::<Record>::worst_case(limits.records)?.checked_mul(2)?)?;
    total = total.checked_add(u64::from(limits.records).checked_mul(u64::from(limits.text))?)?;
    // Chips subjects own at most one configured task-number list each.
    total.checked_add(u64::from(limits.watches).checked_mul(List::<u64>::worst_case(limits.rows)?)?)
}

/// One deadline per lingering object, visible notice and saved record,
/// plus the minute tick. Checked before any container is allocated.
#[must_use]
pub fn timers(limits: &Limits) -> Option<u32> {
    limits.objects.checked_add(limits.notices)?.checked_add(limits.records)?.checked_add(1)
}

/// One task and every bounded proposal name per retained tree row.
pub(crate) fn tree_names(limits: &Limits) -> Option<u32> {
    limits.rows.checked_mul(limits.rows.checked_add(1)?)?.checked_add(1)
}
