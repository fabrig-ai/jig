//! Load bounded accounts and attach sources to their accounts (engine.md, 6).
use crate::domain::{Account, Domain, Hold, Source, settle};
use crate::{Deadline, Fact, Held, Limits, Stored};
use skein_lib::{Duration, Env, Map, Queue, Wall};

pub(crate) fn restore(domain: &mut Domain, env: &Env<Limits>, record: Stored) {
    if domain.restore_limit.is_some() {
        return;
    }
    let (taken, which) = match record {
        Stored::Issued { number, holder, name, signed_out } => {
            domain.past(number);
            (crate::programs::restore(domain, number, holder, name, signed_out), crate::RestoreLimit::ProgramNames)
        }
        Stored::Next { number } => {
            domain.next = domain.next.max(number);
            domain.kept = number;
            (true, crate::RestoreLimit::Accounts)
        }
        Stored::Account { number, provider, holder, place, uses, subject, display } => {
            domain.past(number);
            (
                account(
                    domain,
                    env,
                    number,
                    Account {
                        provider,
                        holder,
                        place,
                        uses,
                        plan: false,
                        subject,
                        display,
                        sources: Map::with_capacity(0),
                    },
                ),
                crate::RestoreLimit::Accounts,
            )
        }
        Stored::Source { account, number, place, kind, name, held } => {
            domain.past(number);
            let mut entry = Source::new(account, kind, name, &env.limits);
            hold(&mut entry, env, held);
            (source(domain, env, number, place, entry), crate::RestoreLimit::Sources)
        }
    };
    if !taken && domain.restore_limit.is_none() {
        domain.restore_limit = Some(which);
    }
}

/// Finish source-name links before any cleanup can run.
pub(crate) fn finish(domain: &mut Domain, _env: &Env<Limits>, out: &mut Queue<crate::Request>) {
    crate::programs::finish_restore(domain, out);
}

/// A free number, a bounded subject and display name, and a subject no other account of its
/// provider holds (accounts.md, 3.1 and 3.2).
fn account(domain: &mut Domain, env: &Env<Limits>, number: u32, mut entry: Account) -> bool {
    entry.sources = Map::with_capacity(env.limits.sources);
    match domain.accounts.insert(number, entry) {
        Ok(old) => old.is_none(),
        Err(_) => false,
    }
}

/// A free number, a kept account with room at a free place, one signed-in source an account, and
/// a program's name valid and unique in the deployment (accounts.md, 3.4).
fn source(domain: &mut Domain, env: &Env<Limits>, number: u32, place: u32, entry: Source) -> bool {
    assert!(
        domain.accounts.contains_key(&entry.account),
        "accounts: Source({number}) links to account {}, which is absent",
        entry.account
    );
    let account = domain.accounts.get(&entry.account).expect("linked source account");
    if account.sources.len() >= account.sources.capacity() {
        domain.restore_limit = Some(crate::RestoreLimit::SourcePlaces);
        return false;
    }
    assert!(
        !account.sources.contains_key(&place),
        "accounts: Source({number}) links to occupied place {place} of account {}",
        entry.account
    );
    let plan = match domain.providers.get(account.provider) {
        Some(provider) => provider.plan(entry.kind),
        None => false,
    };
    let account = entry.account;
    if domain.sources.insert(number, entry) != Ok(None) {
        return false;
    }
    let account = domain.accounts.get_mut(&account).expect("looked up above");
    account.plan |= plan;
    let places = &mut account.sources;
    assert!(places.insert(place, number) == Ok(None), "a free place with room, checked above");
    let _: Fact = settle(domain, env, number);
    true
}

/// A kept hold means the same after a restart: a reset passed while the child was down has
/// lifted it, and a later one is projected onto this run's clock (engine.md, 5.4).
fn hold(entry: &mut Source, env: &Env<Limits>, kept: Option<Held>) {
    let (hold, attention) = match kept {
        None => return,
        Some(Held::Refused) => (Hold::Refused, true),
        Some(Held::Moving) => (Hold::Moving, false),
        Some(Held::Limited { reset: None }) => (Hold::Limited { until: None }, true),
        Some(Held::Limited { reset: Some(reset) }) => {
            let Some(left) = left(env.wall, reset) else { return };
            (
                Hold::Limited { until: Some(Deadline { at: env.now.saturating_add(left), wall: reset }) },
                left >= env.limits.spent_attention,
            )
        }
    };
    entry.hold = Some(hold);
    entry.hold_attention = attention;
}

/// The time from now to a later wall time; none once it has passed.
fn left(now: Wall, reset: Wall) -> Option<Duration> {
    match reset.as_nanos().checked_sub(now.as_nanos()) {
        Some(0) | None => None,
        Some(nanos) => Some(Duration::from_nanos(nanos)),
    }
}
