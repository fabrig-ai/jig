//! Named consistent limits, in each owner's types (configuration.md, 6).
mod small;
mod standard;
use jig_domain_core as core;
use jig_domain_host as host;
use skein_lib::Duration;
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Profile {
    Small,
    Standard,
}
/// The channel components' own numbers (channel.md, 11).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Link {
    pub chunk: u32,
    pub name: u32,
    pub secret: u32,
    pub names: u32,
    pub opening: u32,
    pub handshake: Duration,
    pub hello: Duration,
    pub ping: Duration,
    pub silence: Duration,
    pub stall: Duration,
    pub backoff: Duration,
    pub ceiling: Duration,
    pub proved: Duration,
    pub busy: Duration,
    pub facts: u32,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Limits {
    pub core: core::Limits,
    pub store: jig_protocol_store::Limits,
    pub hub: host::Limits,
    pub link: Link,
    pub token: u32,
    pub driver: skein_oauth_accounts::Limits,
    pub io: skein_io::Limits,
    pub journal: skein_lib::JournalLimits,
    pub routes: u32,
    pub clients: jig_protocol_engine_clients::Limits,
    pub clients_http: jig_protocol_engine_clients_http::Limits,
}
#[must_use]
pub fn profile(profile: Profile) -> Limits {
    match profile {
        Profile::Small => small::LIMITS,
        Profile::Standard => standard::LIMITS,
    }
}

/// A document retains every name the engine can admit.
pub(crate) const fn name_segments(tasks: &jig_domain_tasks::Limits, authority: &jig_domain_authority::Limits) -> u32 {
    let task =
        if tasks.hold_segments > tasks.authority_segments { tasks.hold_segments } else { tasks.authority_segments };
    if authority.segments > task { authority.segments } else { task }
}

/// Every admitted authority list fits independently of short document lists.
pub(crate) const fn authority_grants(
    tasks: &jig_domain_tasks::Limits,
    authority: &jig_domain_authority::Limits,
) -> u32 {
    if tasks.authority_grants > authority.grants { tasks.authority_grants } else { authority.grants }
}
pub(crate) const fn authority_executors(
    tasks: &jig_domain_tasks::Limits,
    authority: &jig_domain_authority::Limits,
) -> u32 {
    if tasks.executor_kinds > authority.executors { tasks.executor_kinds } else { authority.executors }
}

/// All retained sources, pending account sign-ins and the configured zero slot.
#[must_use]
pub const fn driver_places(accounts: &jig_domain_accounts::Limits) -> Option<u32> {
    let Some(sources) = accounts.accounts.checked_mul(accounts.sources) else { return None };
    let Some(pending) = sources.checked_add(accounts.sign_ins) else { return None };
    pending.checked_add(1)
}

/// Reprice the profile's stores without changing their admission bounds
/// (configuration.md, 6). A commit holds the budget, every operation's key
/// and nine framing bytes, and the 26-byte frame. Queue three (or `queued`)
/// commits, segment two, and snapshot after one.
#[must_use]
pub const fn store_budget(mut limits: skein_kv::Limits, budget: u64) -> Option<skein_kv::Limits> {
    let Some(key) = limits.key.checked_add(9) else { return None };
    let Some(operations) = limits.ops.checked_mul(key) else { return None };
    // Byte conversion keeps this checked narrowing usable in profile constants;
    // TryFrom is not const. The high four bytes must all be zero.
    if budget > 0xffff_ffff {
        return None;
    }
    let [a, b, c, d, ..] = budget.to_le_bytes();
    let Some(commit) = u32::from_le_bytes([a, b, c, d]).checked_add(operations) else { return None };
    let Some(commit) = commit.checked_add(26) else { return None };
    let [a, b, c, d] = commit.to_le_bytes();
    let bytes = u64::from_le_bytes([a, b, c, d, 0, 0, 0, 0]);
    let [a, b, c, d] = limits.queued.to_le_bytes();
    let queued = u64::from_le_bytes([a, b, c, d, 0, 0, 0, 0]);
    let Some(queued_bytes) = bytes.checked_mul(queued) else { return None };
    let Some(segment) = bytes.checked_mul(2) else { return None };
    limits.budget = budget;
    limits.commit = commit;
    limits.queued_bytes = queued_bytes;
    limits.segment = segment;
    limits.snapshot_after = bytes;
    Some(limits)
}
