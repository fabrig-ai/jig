//! Holder declarations and lifetime names (accounts.md, 3.4 and section 5).
use crate::domain::{Account, Source};
use crate::{Domain, Holder, Kind, Limits, Refusal, Request, Stored};
use alloc::boxed::Box;
use skein_lib::{Env, List, Map, Queue};

/// A reservation outlives its source: an old directory keeps its original holder.
#[derive(PartialEq, Eq, Debug)]
pub(crate) struct Issued {
    pub(crate) holder: Holder,
    pub(crate) name: crate::SourceName,
    pub(crate) signed_out: bool,
}

/// A declaration refuses before allocating either a number or a name.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum DeclarationRefusal {
    Account(Refusal),
    Name,
    Issued,
    Names,
}

/// Declare one source, saving the account, source, name reservation and counter together.
/// The caller validates a joined account's existing payers against its new plan terms.
///
/// # Errors
/// An invalid or issued name, the lifetime-name bound, or account/provider admission.
pub fn declare(
    domain: &mut Domain,
    env: &Env<Limits>,
    holder: Holder,
    provider: u32,
    join: Option<u32>,
    name: Box<[u8]>,
    out: &mut Queue<Request>,
) -> Result<u32, DeclarationRefusal> {
    if !domain.wanted() {
        return Err(DeclarationRefusal::Account(Refusal::Closing));
    }
    let Some(admitted) = crate::SourceName::new(&name) else { return Err(DeclarationRefusal::Name) };
    for (_, issued) in &domain.issued {
        if issued.name.as_bytes() == name.as_ref() {
            return Err(DeclarationRefusal::Issued);
        }
    }
    if domain.issued.len() >= env.limits.program_names.min(domain.issued.capacity()) {
        return Err(DeclarationRefusal::Names);
    }
    match domain.providers.get(provider) {
        Some(offered) if !offered.offers.programs.is_empty() => {}
        Some(_) | None => return Err(DeclarationRefusal::Account(Refusal::Provider { provider })),
    }
    let admission = match join {
        Some(account) => {
            if let Err(refusal) = crate::changes::account(domain, holder, account) {
                return Err(DeclarationRefusal::Account(refusal));
            }
            let Some(entry) = domain.accounts.get(&account) else {
                return Err(DeclarationRefusal::Account(Refusal::Unknown));
            };
            if entry.provider != provider {
                return Err(DeclarationRefusal::Account(Refusal::Unknown));
            }
            domain.room_for_source(&env.limits, account)
        }
        None => domain.room_for_account(&env.limits, holder),
    };
    if let Err(refusal) = admission {
        return Err(DeclarationRefusal::Account(refusal));
    }
    let account = match join {
        Some(number) => number,
        None => {
            let number = domain.take();
            let entry = Account {
                provider,
                holder,
                subject: Box::new([]),
                display: Box::new([]),
                sources: Map::with_capacity(env.limits.sources),
                place: domain.last_place(holder),
                uses: None,
                plan: true,
            };
            assert!(domain.accounts.insert(number, entry).is_ok(), "admitted account");
            out.push(Request::Save { record: crate::ordering::account_record(domain, number) });
            number
        }
    };
    let number = domain.take();
    let entry = domain.accounts.get_mut(&account).expect("admitted account remains kept");
    let place = entry.sources.len();
    assert!(entry.sources.insert(place, number) == Ok(None), "admitted source at the last place");
    entry.plan = true;
    let issued = Issued { holder, name: admitted, signed_out: false };
    assert!(domain.issued.insert(number, issued).is_ok(), "admitted lifetime name");
    out.push(Request::Save { record: Stored::Issued { number, holder, name: name.clone(), signed_out: false } });
    let source = Source::new(account, Kind::Program, name, &env.limits);
    assert!(domain.sources.insert(number, source) == Ok(None), "admitted source");
    out.push(Request::Save { record: crate::domain::stored(domain, env, number) });
    domain.counted(out);
    publish(domain, out);
    crate::domain::changed(domain, env, number, None, out);
    Ok(number)
}

pub(crate) fn restore(domain: &mut Domain, number: u32, holder: Holder, name: Box<[u8]>, signed_out: bool) -> bool {
    let Some(name) = crate::SourceName::new(&name) else { return false };
    if number == u32::MAX || domain.accounts.contains_key(&number) {
        return false;
    }
    for (other_number, issued) in &domain.issued {
        if *other_number == number || issued.name == name {
            return false;
        }
    }
    domain.issued.insert(number, Issued { holder, name, signed_out }).is_ok()
}

pub(crate) fn finish_restore(domain: &mut Domain, out: &mut Queue<Request>) {
    // A live source predating the name registry still owns its directory. Reserve it
    // before admitting changes, rather than making its old name available to a new holder.
    let mut missing = List::with_capacity(domain.sources.capacity());
    for (number, source) in &domain.sources {
        if source.kind == Kind::Program && !domain.issued.contains_key(number) {
            missing.push(*number).expect("one reservation per live source");
        }
    }
    for number in missing.into_boxed() {
        let Some(source) = domain.sources.get(&number) else { continue };
        let Some(account) = domain.accounts.get(&source.account) else { continue };
        let holder = account.holder;
        let name = source.name.clone();
        if restore(domain, number, holder, name.clone(), false) {
            out.push(Request::Save { record: Stored::Issued { number, holder, name, signed_out: false } });
        }
    }
    let mut rejected = List::with_capacity(domain.sources.capacity());
    for (number, source) in &domain.sources {
        let valid = match source.kind {
            Kind::Program => match domain.issued.get(number) {
                Some(issued) => match domain.accounts.get(&source.account) {
                    Some(account) => issued.holder == account.holder && issued.name.as_bytes() == source.name.as_ref(),
                    None => false,
                },
                None => false,
            },
            Kind::SignIn | Kind::Key | Kind::None => !domain.issued.contains_key(number),
        };
        if !valid {
            rejected.push(*number).expect("at most one rejected row per source");
        }
    }
    for number in &rejected {
        let Some(source) = domain.sources.remove(number) else { continue };
        if let Some(account) = domain.accounts.get_mut(&source.account) {
            let mut at = None;
            for (place, kept) in &account.sources {
                if kept == number {
                    at = Some(*place);
                }
            }
            if let Some(place) = at {
                account.sources.remove(&place);
            }
        }
        domain.alarms.cancel(*number);
        domain.unrestored = domain.unrestored.saturating_add(1);
    }
    let mut numbers = List::with_capacity(domain.accounts.capacity());
    for (number, _) in &domain.accounts {
        numbers.push(*number).expect("one number per account");
    }
    for number in &numbers {
        let Some(account) = domain.accounts.get_mut(number) else { continue };
        let mut plan = false;
        for (_, number) in &account.sources {
            if let Some(source) = domain.sources.get(number)
                && let Some(provider) = domain.providers.get(account.provider)
            {
                plan |= provider.plan(source.kind);
            }
        }
        account.plan = plan;
    }
}

/// The schema ceiling; configured membership sets the admitted runtime bound.
pub const PROGRAM_HOSTS: u32 = 128;
#[derive(Debug)]
pub(crate) struct Pending {
    pub(crate) leaving: crate::changes::Leaving,
    hosts: List<Host>,
    targeted: bool,
}
#[derive(Debug)]
pub(crate) struct Host {
    worker: crate::Worker,
    result: Option<crate::ProgramResult>,
}
#[must_use]
pub fn name(domain: &Domain, source: u32, holder: Holder) -> Option<&[u8]> {
    let issued = domain.issued.get(&source)?;
    if issued.holder != holder {
        return None;
    }
    Some(issued.name.as_bytes())
}
pub(crate) fn publish(domain: &Domain, out: &mut Queue<Request>) {
    if domain.issued.is_empty() {
        return;
    }
    let mut names = List::with_capacity(domain.issued.capacity());
    for (number, issued) in &domain.issued {
        names
            .push(crate::Program {
                name: issued.name,
                cleanup: issued.signed_out || !domain.sources.contains_key(number),
            })
            .expect("one desired state per reservation");
    }
    out.push(Request::Programs { names: names.into_boxed() });
}
/// An explicit holder check permits a fresh login on a live declaration.
/// A Refused source is withdrawn and not granted again while held Refused; Resume clears
/// only Limited holds. Withdrawal records no fact: facts are expendable.
/// # Errors
/// A foreign, retired or unknown source, or a closing child.
pub fn check(domain: &mut Domain, holder: Holder, source: u32, out: &mut Queue<Request>) -> Result<(), Refusal> {
    crate::changes::source(domain, holder, source)?;
    if domain.programs.contains_key(&source) {
        return Err(Refusal::Limit { bound: crate::Bound::Sources, number: None });
    }
    if domain.numbers_left() == 0 {
        return Err(Refusal::Numbers);
    }
    let issued = domain.issued.get_mut(&source).ok_or(Refusal::Unknown)?;
    issued.signed_out = false;
    let name = issued.name;
    out.push(Request::Save {
        record: Stored::Issued {
            number: source,
            holder,
            name: skein_lib::bytes::copy_of(name.as_bytes()),
            signed_out: false,
        },
    });
    // A fresh login gets a fresh epoch through the existing durable counter.
    domain.take();
    domain.counted(out);
    crate::domain::withdraw_program(domain, source);
    crate::domain::program_grant(domain, source, out);
    publish(domain, out);
    out.push(Request::CheckProgram { name });
    Ok(())
}
pub(crate) fn begin(
    domain: &mut Domain,
    limits: &Limits,
    source: u32,
    leaving: crate::changes::Leaving,
    out: &mut Queue<Request>,
) {
    let issued = domain.issued.get_mut(&source).expect("admitted issued program source");
    issued.signed_out = true;
    let name = issued.name;
    out.push(Request::Save {
        record: Stored::Issued {
            number: source,
            holder: issued.holder,
            name: skein_lib::bytes::copy_of(name.as_bytes()),
            signed_out: true,
        },
    });
    crate::domain::withdraw_program(domain, source);
    assert!(
        domain
            .programs
            .insert(source, Pending { leaving, hosts: List::with_capacity(limits.program_hosts), targeted: false })
            .is_ok(),
        "admitted program operation"
    );
    publish(domain, out);
    out.push(Request::SignOutProgram { source, name });
}
pub(crate) fn targets(domain: &mut Domain, source: u32, hosts: Box<[crate::Worker]>, out: &mut Queue<Request>) {
    let Some(pending) = domain.programs.get_mut(&source) else { return };
    if pending.targeted {
        return;
    }
    for worker in hosts {
        let mut duplicate = false;
        for row in &pending.hosts {
            duplicate |= row.worker == worker;
        }
        if !duplicate {
            pending.hosts.push(Host { worker, result: None }).expect("configured target count checked at startup");
        }
    }
    pending.targeted = true;
    complete(domain, source, out);
}
pub(crate) fn signed_out(
    domain: &mut Domain,
    source: u32,
    host: crate::Worker,
    result: crate::ProgramResult,
    out: &mut Queue<Request>,
) {
    let Some(pending) = domain.programs.get_mut(&source) else { return };
    let mut found = false;
    for index in 0..pending.hosts.len() {
        let Some(row) = pending.hosts.get_mut(index) else { continue };
        if row.worker == host {
            found = true;
            if row.result.is_none() {
                row.result = Some(result);
            }
        }
    }
    if !found && !pending.targeted {
        pending
            .hosts
            .push(Host { worker: host, result: Some(result) })
            .expect("one early terminal per configured target");
    }
    complete(domain, source, out);
}
fn complete(domain: &mut Domain, source: u32, out: &mut Queue<Request>) {
    let Some(pending) = domain.programs.get(&source) else { return };
    if !pending.targeted {
        return;
    }
    let mut failed = List::with_capacity(pending.hosts.capacity());
    for row in &pending.hosts {
        let failure = match row.result {
            None => return,
            Some(crate::ProgramResult::NotInContact) => Some(crate::ProgramFailure::NotInContact),
            Some(crate::ProgramResult::Lost) => Some(crate::ProgramFailure::Lost),
            Some(crate::ProgramResult::Failed) => Some(crate::ProgramFailure::Failed),
            Some(crate::ProgramResult::SignedOut) => None,
        };
        if let Some(result) = failure {
            failed.push(crate::ProgramHost { worker: row.worker, result }).expect("one per configured worker");
        }
    }
    let pending = domain.programs.remove(&source).expect("the completed program operation is still held");
    let failed_hosts = failed.into_boxed();
    let confirmed = failed_hosts.is_empty();
    match pending.leaving {
        crate::changes::Leaving::Source(request) => {
            out.push(Request::Answer { request, answer: crate::Answer::SignedOut { confirmed, failed_hosts } });
        }
        crate::changes::Leaving::Account(account) => {
            crate::changes::signed_out(domain, account, confirmed, failed_hosts, out);
        }
    }
}
/// # Errors
/// A foreign/unknown source, a closing child, or a concurrent operation.
pub fn sign_out(
    domain: &mut Domain,
    env: &Env<Limits>,
    request: skein_lib::Token,
    holder: Holder,
    source: u32,
    out: &mut Queue<Request>,
) -> Result<(), Refusal> {
    crate::changes::source(domain, holder, source)?;
    if name(domain, source, holder).is_none() {
        return Err(Refusal::Unknown);
    }
    if domain.programs.contains_key(&source) {
        return Err(Refusal::Limit { bound: crate::Bound::Sources, number: None });
    }
    if domain.programs.len() >= env.limits.program_names || domain.programs.len() == domain.programs.capacity() {
        return Err(Refusal::limited(
            crate::Bound::Sources,
            crate::LimitKey::ProgramNames,
            env.limits.program_names.min(domain.programs.capacity()),
        ));
    }
    begin(domain, &env.limits, source, crate::changes::Leaving::Source(request), out);
    Ok(())
}

/// Original holder evidence remains available after the live source is removed.
#[must_use]
#[expect(clippy::manual_map, reason = "the strict subset uses exhaustive matches without closures")]
pub fn owner(domain: &Domain, name: &[u8]) -> Option<Holder> {
    match issued(domain, name) {
        Some((_, holder)) => Some(holder),
        None => None,
    }
}

/// Issued source number and original holder, including signed-out and retired names.
#[must_use]
pub fn issued(domain: &Domain, name: &[u8]) -> Option<(u32, Holder)> {
    for (number, issued) in &domain.issued {
        if issued.name.as_bytes() == name {
            return Some((*number, issued.holder));
        }
    }
    None
}
