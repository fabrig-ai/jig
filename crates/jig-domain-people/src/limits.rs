//! Root-configured capacities and checked heap accounting (domain/people.md, sections 2–5). `valid` validates limits before construction;
//! bounds cover retained state and scratch. Root accounts output queues and
//! payloads separately. These calculations know no IO, credentials or tasks.

use crate::domain::{Answered, Pending, SignIn};
use crate::{Fact, Holding, Identity, IdentityKey, InitialOwner, RequestKey};
use alloc::boxed::Box;
use skein_lib::{Deadlines, Duration, Id, List, Map, Queue, Set, Slab, Wall};

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

/// Containers, retained bytes, pending copies and bounded bootstrap config. Checked heap bound
/// under `limits` for retained containers/bytes, pending copies, bootstrap owners, restoration
/// sign-in snapshot, role-update scratch and facts. Returns `None` for arithmetic overflow, zero
/// waiters, zero sign-ins or zero sign-in lifetime; validates `max_out` additions. Caller separately counts
/// incoming/outgoing payloads and Request queue storage.
#[must_use]
pub fn worst_case(limits: &Limits) -> Option<u64> {
    if !valid(limits) {
        return None;
    }
    let conversation_positions = limits.people.checked_mul(limits.inbox_tasks)?;
    let refusal_views = limits.requests.checked_add(limits.room_refusals)?;
    let ask_bytes = ask_bytes(limits)?;
    // One local copy validates a restored answer while its retained names remain owned.
    let host_copy = host_outcome_bytes(limits)?;
    let people = Map::<u64, Identity>::worst_case(limits.people)?
        .checked_add(Map::<IdentityKey, u64>::worst_case(limits.people)?)?
        .checked_add(u64::from(limits.people).checked_mul(u64::from(limits.identity_bytes).checked_mul(2)?)?)?;
    let roles = Map::<u32, Box<[Holding]>>::worst_case(limits.projects)?.checked_add(
        u64::from(limits.projects)
            .checked_mul(u64::from(limits.holdings))?
            .checked_mul(u64::try_from(size_of::<Holding>()).ok()?)?,
    )?;
    let waiting = waiting_worst_case(limits)?;
    people
        .checked_add(host_copy)?
        .checked_add(
            u64::from(limits.amendment_bytes)
                .checked_add(u64::try_from(size_of::<crate::Amendment>()).ok()?)?
                .max(u64::try_from(crate::NAME_BYTES).ok()?)
                .max(u64::from(limits.words)),
        )?
        .checked_add(roles)?
        .checked_add(waiting)?
        .checked_add(Map::<u64, Box<[crate::domain::Need]>>::worst_case(limits.inbox_tasks)?)?
        .checked_add(
            u64::from(limits.inbox_tasks)
                .checked_mul(List::<crate::domain::Need>::worst_case(limits.inbox_recipients)?)?,
        )?
        .checked_add(List::<crate::domain::Need>::worst_case(limits.inbox_recipients)?)?
        .checked_add(Map::<u64, u64>::worst_case(limits.people)?)?
        .checked_add(Map::<u64, crate::domain::ChatTotal>::worst_case(limits.inbox_tasks)?)?
        .checked_add(Map::<(u64, u32), u128>::worst_case(limits.people.checked_mul(limits.projects)?)?)?
        .checked_add(Map::<u64, Box<[crate::domain::Recent]>>::worst_case(limits.people)?)?
        .checked_add(
            u64::from(limits.people).checked_mul(List::<crate::domain::Recent>::worst_case(limits.inbox_entries)?)?,
        )?
        .checked_add(List::<crate::domain::Recent>::worst_case(limits.inbox_entries)?)?
        .checked_add(List::<crate::domain::Recent>::worst_case(limits.inbox_entries)?)?
        .checked_add(List::<crate::Update>::worst_case(limits.inbox_entries)?)?
        .checked_add(Map::<(u64, u64), u64>::worst_case(conversation_positions)?)?
        .checked_add(List::<u64>::worst_case(limits.people)?)?
        .checked_add(Map::<u64, Box<[crate::Entry]>>::worst_case(limits.inbox_tasks)?)?
        .checked_add(
            u64::from(limits.inbox_tasks).checked_mul(List::<crate::Entry>::worst_case(limits.inbox_recipients)?)?,
        )?
        .checked_add(List::<crate::Entry>::worst_case(limits.inbox_recipients)?)?
        .checked_add(Map::<(u64, u32), Box<[crate::ResultRef]>>::worst_case(
            limits.people.checked_mul(limits.projects)?,
        )?)?
        .checked_add(
            u64::from(limits.people.checked_mul(limits.projects)?)
                .checked_mul(List::<crate::ResultRef>::worst_case(limits.inbox_entries.checked_add(1)?)?)?,
        )?
        .checked_add(List::<u32>::worst_case(limits.people.checked_mul(limits.projects)?)?)?
        .checked_add(Map::<u64, Box<[crate::ResultRef]>>::worst_case(limits.people)?)?
        .checked_add(
            u64::from(limits.people)
                .checked_mul(List::<crate::ResultRef>::worst_case(limits.inbox_entries.checked_add(1)?)?)?,
        )?
        .checked_add(List::<crate::ResultRef>::worst_case(limits.inbox_entries.checked_add(1)?)?)?
        .checked_add(sign_in_worst_case(limits)?)?
        .checked_add(Map::<RequestKey, Answered>::worst_case(limits.requests)?)?
        .checked_add(Deadlines::<RequestKey>::worst_case(limits.requests)?)?
        // Each retained answer owns its longest visit address or configured-host failure list.
        .checked_add(u64::from(limits.requests).checked_mul(ask_bytes.checked_add(outcome_bytes(limits)?)?)?)?
        .checked_add(act_worst_case(limits)?)?
        .checked_add(Slab::<Pending>::worst_case(limits.pending)?)?
        .checked_add(Map::<RequestKey, Id<Pending>>::worst_case(limits.pending)?)?
        .checked_add(u64::from(limits.pending).checked_mul(List::<skein_lib::ReplyTo>::worst_case(limits.waiters)?)?)?
        .checked_add(u64::from(limits.pending).checked_mul(ask_bytes.checked_mul(2)?)?)?
        .checked_add(u64::from(limits.initial_owners).checked_mul(
            u64::try_from(size_of::<InitialOwner>()).ok()?.checked_add(u64::from(limits.identity_bytes))?,
        )?)?
        .checked_add(List::<(u64, SignIn)>::worst_case(limits.sign_ins)?)?
        .checked_add(List::<Holding>::worst_case(limits.holdings)?)?
        .checked_add(Queue::<crate::CapacityRefusal>::worst_case(limits.room_refusals)?)?
        .checked_add(Map::<(Wall, u32), crate::CapacityRefusal>::worst_case(refusal_views)?)?
        .checked_add(List::<crate::CapacityRefusal>::worst_case(refusal_views)?)?
        .checked_add(Queue::<Fact>::worst_case(limits.facts)?)
}

pub(crate) fn ask_bytes(limits: &Limits) -> Option<u64> {
    let compound = List::<crate::Operation>::worst_case(limits.operations)?.checked_add(
        u64::from(limits.operations).checked_mul(
            u64::from(limits.amendment_bytes)
                .checked_add(u64::try_from(size_of::<crate::Amendment>()).ok()?)?
                .max(u64::try_from(crate::NAME_BYTES).ok()?)
                .max(u64::from(limits.words)),
        )?,
    )?;
    (u64::from(limits.batch)
        .checked_mul(u64::try_from(size_of::<crate::Member>()).ok()?.checked_add(u64::from(limits.amendment_bytes))?)?
        .checked_add(u64::from(limits.words))?)
    .max(compound)
    .max(u64::from(limits.words))
    .max(u64::from(limits.amendment_bytes))
    .max(u64::from(limits.holdings).checked_mul(u64::try_from(size_of::<Holding>()).ok()?)?)
    .checked_add(crate::model_names_worst_case()?)
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

fn sign_in_worst_case(limits: &Limits) -> Option<u64> {
    Map::<u64, SignIn>::worst_case(limits.sign_ins)?
        .checked_add(Deadlines::<u64>::worst_case(limits.sign_ins)?)?
        .checked_add(Set::<(Wall, u64)>::worst_case(limits.sign_ins)?)
}

pub(crate) fn act_bytes(limits: &Limits) -> Option<u64> {
    // valid_adoption counts both slice headers and contents within amendment_bytes.
    Some(u64::from(limits.words).max(u64::from(limits.amendment_bytes)).max(u64::try_from(crate::NAME_BYTES).ok()?))
}

pub(crate) fn act_outcome_bytes(limits: &Limits) -> Option<u64> {
    // RemoveAccount retains host failures; Close and every other act can retain
    // a refusal (Circle: 32 tasks; Kept: 64). The other outcomes own no bytes.
    Some(host_outcome_bytes(limits)?.max(64_u64.checked_mul(u64::try_from(size_of::<u64>()).ok()?)?))
}

pub(crate) fn act_worst_case(limits: &Limits) -> Option<u64> {
    Map::<crate::ActKey, crate::domain::acts::Acted>::worst_case(limits.requests)?
        .checked_add(Deadlines::<crate::ActKey>::worst_case(limits.requests)?)?
        .checked_add(Map::<crate::ActKey, Id<Pending>>::worst_case(limits.pending)?)?
        .checked_add(
            u64::from(limits.requests).checked_mul(act_bytes(limits)?.checked_add(act_outcome_bytes(limits)?)?)?,
        )?
        .checked_add(
            u64::from(limits.act_grants).checked_mul(
                u64::try_from(size_of::<crate::ActGrant>()).ok()?.checked_add(
                    u64::try_from(crate::ActKind::ALL.len())
                        .ok()?
                        .checked_mul(u64::try_from(size_of::<crate::ActKind>()).ok()?)?,
                )?,
            )?,
        )
}

fn waiting_worst_case(limits: &Limits) -> Option<u64> {
    // At most one role per holding; only held roles can have a cached inbox.
    let waiting_recipients = waiting_capacity(limits)?;
    Map::<crate::Whom, Box<[crate::Entry]>>::worst_case(waiting_recipients)?
        .checked_add(
            u64::from(waiting_recipients).checked_mul(List::<crate::Entry>::worst_case(limits.inbox_entries)?)?,
        )?
        .checked_add(List::<crate::Whom>::worst_case(waiting_recipients)?)?
        .checked_add(List::<crate::Entry>::worst_case(limits.inbox_entries)?)
}

fn inbox_capacity(limits: &Limits) -> Option<u32> {
    limits.inbox_tasks.checked_mul(limits.inbox_recipients)?.checked_add(limits.inbox_entries.checked_add(1)?)
}
fn waiting_capacity(limits: &Limits) -> Option<u32> {
    limits.people.checked_add(limits.projects.checked_mul(limits.holdings)?)
}
