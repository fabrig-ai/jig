//! Root-configured capacities and operation-answer envelopes (domain/people.md, sections 2–5).

use skein_lib::Duration;

/// Immutable root-configured capacities and payload limits for people's retained state and one-step
/// outputs; validate via `valid` before construction. (domain/people.md, sections 2–5).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Limits {
    /// Configured workers plus the engine host in one retained program outcome.
    pub program_hosts: u32,
    /// Whole configured worker name bytes in retained operation answers.
    pub program_host_name_bytes: u32,
    /// Configured connector grants, derived from configuration's unique acts entries.
    pub act_grants: u32,
    /// Maximum members of one requested or proposed batch.
    pub batch: u32,
    /// Maximum task operations in one keyed compound request.
    pub operations: u32,
    /// Maximum retained parties and provider/subject index entries.
    pub people: u32,
    /// Maximum visible cached entries and unread count; results retain one extra overflow witness.
    pub inbox_entries: u32,
    /// Maximum live task/person-proposal census rows; per-conversation positions retain people × this bound, including ended conversations until forgotten.
    pub inbox_tasks: u32,
    /// Maximum recipient references in one task projection, independent of cached entries.
    pub inbox_recipients: u32,
    /// Positive maximum live secret-free sign-ins and expiry alarms.
    pub sign_ins: u32,
    /// Maximum retained project role sets.
    pub projects: u32,
    /// Maximum unique-person holdings per project role set.
    pub holdings: u32,
    /// Maximum retained configured first-owner entries; also bounds bootstrap output matches.
    pub initial_owners: u32,
    /// Maximum retained completed keys plus ordinary flights' answer reservations, engine-wide.
    /// A move out reserves two places, retaining Written's until completion or cancellation.
    /// New Seen flights use pending room independently and reserve no answer slot.
    pub requests: u32,
    /// Maximum unsaved entrance-pressure records, independent of diagnostic capacity.
    /// Permanent room refusals remain in the bounded keyed-answer table.
    pub room_refusals: u32,
    /// Maximum simultaneous keyed flights, including answered moves waiting for Written.
    pub pending: u32,
    /// Positive maximum reply destinations per flight, including its first caller.
    pub waiters: u32,
    /// Maximum combined subject, login and display-name bytes per party.
    pub identity_bytes: u32,
    /// Maximum opening-word or escalation rejection-reason bytes per keyed ask, including pending
    /// and completed copies.
    pub words: u32,
    /// Maximum owned bytes carried by one keyed task amendment.
    pub amendment_bytes: u32,
    /// Nonzero configured lifetime projected once from admission's wall/monotonic environment.
    pub sign_in_lifetime: Duration,
    /// Nonzero time for which completed person keys replay their durable answers.
    pub request_retention: Duration,
    /// Capacity of optional content-free observations; overflow increments a diagnostic lost
    /// counter.
    pub facts: u32,
}

/// Whether these limits are representable: every edge bound they size
/// is finite and every capacity they derive fits (programming-model.md, 6.3).
#[must_use]
pub fn valid(limits: &Limits) -> bool {
    if limits.act_grants > u32::from(u16::MAX).saturating_add(1)
        || limits.program_hosts == 0
        || limits.program_hosts > 128
        || limits.program_host_name_bytes == 0
        || limits.program_host_name_bytes > 4096
        || limits.waiters == 0
        || limits.sign_ins == 0
        || limits.sign_in_lifetime == Duration::ZERO
        || limits.request_retention == Duration::ZERO
    {
        return false;
    }
    limits.inbox_entries.checked_add(1).is_some()
        && limits.people.checked_add(1).is_some()
        && limits.people.checked_mul(limits.inbox_tasks).is_some()
        && inbox_capacity(limits).is_some()
        && limits.requests.checked_add(limits.room_refusals).is_some()
        && limits.initial_owners.checked_add(4).is_some()
        && limits.waiters.checked_add(2).is_some()
        && limits.sign_ins.checked_add(1).is_some()
        && limits.holdings.checked_add(1).is_some()
        && limits.people.checked_mul(limits.projects).is_some()
        && waiting_capacity(limits).is_some()
}

/// Retained operation answers own configured names, not observed login inventory.
#[must_use]
pub fn outcome_bytes(limits: &Limits) -> Option<u64> {
    // Circle and settings-full name at most NAMED_TASKS (32); Kept already prices 64.
    Some(host_outcome_bytes(limits)?.max(u64::from(crate::ADDRESS)).max(64 * 8))
}
fn host_outcome_bytes(limits: &Limits) -> Option<u64> {
    u64::from(limits.program_hosts).checked_mul(
        u64::try_from(size_of::<crate::AccountHostFailure>())
            .ok()?
            .checked_add(u64::from(limits.program_host_name_bytes))?,
    )
}

fn inbox_capacity(limits: &Limits) -> Option<u32> {
    limits.inbox_tasks.checked_mul(limits.inbox_recipients)?.checked_add(limits.inbox_entries.checked_add(1)?)
}
fn waiting_capacity(limits: &Limits) -> Option<u32> {
    limits.people.checked_add(limits.projects.checked_mul(limits.holdings)?)
}
