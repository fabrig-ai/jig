use skein_lib::{Deadlines, Duration, List, Map, Queue};

use crate::domain::{Account, Granted, Source};
use crate::signing::UnderWay;
use crate::{DISPLAY, Ended, Fact, NAME, SUBJECT};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Limits {
    pub accounts: u32,
    /// Accounts one holder keeps, the deployment or a party; at most `accounts` (accounts.md, 3.6).
    pub holder_accounts: u32,
    /// Sources one account keeps, in its holder's order.
    pub sources: u32,
    /// Lifetime program names, retired names included (accounts.md, 3.4).
    pub program_names: u32,
    /// Hosts a program sign-out can await; at most `PROGRAM_HOSTS`.
    pub program_hosts: u32,
    /// Sign-ins, LLM-key keeps, moves out and moves in under way at once.
    pub sign_ins: u32,
    pub credentials: u32,
    pub backoff_base: Duration,
    pub backoff_max: Duration,
    pub retry: Duration,
    /// How long a move out waits for Written after its answer (configuration.md, section 6).
    pub move_deadline: Duration,
    pub spent_attention: Duration,
    pub facts: u32,
}

/// Whether these limits are representable: every edge bound they size
/// is finite and every capacity they derive fits (programming-model.md, 6.3).
#[must_use]
pub fn valid(limits: &Limits) -> bool {
    if limits.accounts == 0
        || crate::domain::out_bound(limits).is_none()
        || limits.holder_accounts == 0
        || limits.holder_accounts > limits.accounts
        || limits.program_hosts == 0
        || limits.program_hosts > crate::programs::PROGRAM_HOSTS
        || limits.program_names == 0
        || limits.sources == 0
        || limits.sign_ins == 0
        || limits.retry == Duration::ZERO
        || limits.move_deadline == Duration::ZERO
        || limits.backoff_base == Duration::ZERO
        || limits.backoff_max < limits.backoff_base
    {
        return false;
    }
    true
}

/// Accounts hold bounded names and times; every secret is below them. Each bound is at least
/// one, and a holder's accounts are within the deployment's. The configured providers are the
/// configuration's, outside this bound. Sign-ins under way are within `sign_ins`, sign-outs the
/// component has not answered within the sources' bound, removals within `accounts`,
/// LLM-key keeps sharing `sign_ins`, and sign-ins' ends within `facts`.
#[must_use]
pub fn worst_case(limits: &Limits) -> Option<u64> {
    if !valid(limits) {
        return None;
    }
    let accounts = u64::from(limits.accounts);
    let sources = limits.accounts.checked_mul(limits.sources)?;
    let names = u64::from(SUBJECT).checked_add(u64::from(DISPLAY))?.checked_mul(accounts)?;
    let places = Map::<u32, u32>::worst_case(limits.sources)?.checked_mul(accounts)?;
    let programs = u64::from(NAME).checked_mul(u64::from(sources))?;
    Map::<u32, Account>::worst_case(limits.accounts)?
        .checked_add(names)?
        .checked_add(
            u64::from(crate::USES).checked_mul(u64::try_from(size_of::<crate::Use>()).ok()?)?.checked_mul(accounts)?,
        )?
        .checked_add(places)?
        // Dense ordering keeps the previous source map and bounded number lists until saved.
        .checked_add(Map::<u32, u32>::worst_case(limits.sources)?)?
        .checked_add(List::<u32>::worst_case(limits.accounts)?.checked_mul(2)?)?
        .checked_add(List::<u32>::worst_case(limits.sources)?)?
        .checked_add(Map::<u32, Source>::worst_case(sources)?)?
        .checked_add(programs)?
        .checked_add(Map::<u32, crate::programs::Issued>::worst_case(limits.program_names)?)?
        .checked_add(Map::<u32, crate::programs::Pending>::worst_case(limits.program_names)?)?
        .checked_add(
            List::<crate::programs::Host>::worst_case(limits.program_hosts)?
                .checked_mul(u64::from(limits.program_names))?,
        )?
        .checked_add(List::<crate::ProgramHost>::worst_case(limits.program_hosts)?)?
        .checked_add(
            Map::<crate::Worker, crate::ProgramFailure>::worst_case(limits.program_hosts)?
                .checked_mul(u64::from(limits.accounts))?,
        )?
        .checked_add(Map::<u32, Granted>::worst_case(limits.credentials)?)?
        .checked_add(Deadlines::<u32>::worst_case(limits.credentials)?)?
        .checked_add(List::<u32>::worst_case(sources)?)?
        .checked_add(Deadlines::<u32>::worst_case(sources)?)?
        .checked_add(Queue::<Fact>::worst_case(limits.facts)?)?
        .checked_add(Map::<u32, UnderWay>::worst_case(limits.sign_ins)?)?
        .checked_add(Map::<u32, crate::signing::Move>::worst_case(limits.sign_ins)?)?
        .checked_add(Deadlines::<u32>::worst_case(limits.sign_ins)?)?
        .checked_add(Map::<u32, crate::signing::Arriving>::worst_case(limits.sign_ins)?)?
        .checked_add(u64::from(SUBJECT).checked_add(u64::from(DISPLAY))?.checked_mul(u64::from(limits.sign_ins))?)?
        // A withdraw request owns a copy of the account's display name.
        .checked_add(u64::from(DISPLAY).checked_mul(u64::from(limits.sign_ins))?)?
        // Each account sign-in can emit one answer owning its bounded visit address.
        .checked_add(u64::from(limits.sign_ins).checked_mul(u64::from(crate::ADDRESS))?)?
        .checked_add(Map::<u32, crate::changes::Leaving>::worst_case(sources)?)?
        .checked_add(Map::<u32, crate::keys::Keeping>::worst_case(limits.sign_ins)?)?
        .checked_add(Map::<u32, crate::changes::Removing>::worst_case(limits.accounts)?)?
        .checked_add(Queue::<Ended>::worst_case(limits.facts)?)
}
