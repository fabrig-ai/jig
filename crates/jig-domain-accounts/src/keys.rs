//! Keeping and replacing LLM keys (accounts.md, 3.5 and section 5).
use crate::domain::{Account, Source, stored};
use crate::{Answer, Bound, Domain, Holder, KeyFailure, Kind, Limits, Refusal, Request, Signing};
use alloc::boxed::Box;
use skein_lib::{Duration, Env, Map, Queue, Token};

/// Program grants carry names, with no expiring table value (channel.md, section 8).
pub const PROGRAM_VALID: Duration = Duration::from_nanos(u64::MAX);

/// LLM keys do not expire; this grant horizon fits the worker wire's duration.
pub const KEY_VALID: Duration = Duration::from_secs(100 * 365 * 24 * 60 * 60);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct Keeping {
    pub(crate) request: Token,
    pub(crate) holder: Holder,
    pub(crate) provider: u32,
    pub(crate) join: Option<u32>,
    pub(crate) replacing: bool,
}

#[expect(clippy::too_many_arguments, reason = "one admitted account change and its carried number")]
pub(crate) fn add(
    domain: &mut Domain,
    env: &Env<Limits>,
    request: Token,
    holder: Holder,
    provider: u32,
    join: Option<u32>,
    carried: u64,
    out: &mut Queue<Request>,
) {
    if let Err(refusal) = addable(domain, &env.limits, holder, provider, join) {
        answer(request, refusal, out);
        return;
    }
    let source = domain.take();
    domain.counted(out);
    let keeping = Keeping { request, holder, provider, join, replacing: false };
    assert!(domain.keeping.insert(source, keeping) == Ok(None), "admitted keep with a new number");
    out.push(Request::Signing(Signing::Keep { source, carried, replaces: false }));
}

fn ready(domain: &Domain, limits: &Limits) -> Result<(), Refusal> {
    if !domain.wanted() {
        return Err(Refusal::Closing);
    }
    crate::room_for_sign_in(limits, domain.sign_ins())
}
fn addable(domain: &Domain, limits: &Limits, holder: Holder, provider: u32, join: Option<u32>) -> Result<(), Refusal> {
    if !domain.wanted() {
        return Err(Refusal::Closing);
    }
    match domain.providers.get(provider) {
        Some(offered) if offered.offers.key => {}
        Some(_) | None => return Err(Refusal::Provider { provider }),
    }
    match join {
        Some(account) => {
            let Some(entry) = domain.accounts.get(&account) else { return Err(Refusal::Unknown) };
            if entry.holder != holder || entry.provider != provider {
                return Err(Refusal::Unknown);
            }
            crate::changes::account(domain, holder, account)?;
            for (_, source) in &entry.sources {
                if domain.moves.contains_key(source) {
                    return Err(Refusal::Moving);
                }
            }
            domain.room_for_source(limits, account)?;
        }
        None => domain.room_for_account(limits, holder)?,
    }
    ready(domain, limits)
}

pub(crate) fn replace(
    domain: &mut Domain,
    env: &Env<Limits>,
    request: Token,
    holder: Holder,
    source: u32,
    carried: u64,
    out: &mut Queue<Request>,
) {
    let checked = replaceable(domain, &env.limits, holder, source);
    let account = match checked {
        Ok(account) => account,
        Err(refusal) => {
            answer(request, refusal, out);
            return;
        }
    };
    let provider = domain.accounts.get(&account).expect("checked account").provider;
    let keeping = Keeping { request, holder, provider, join: Some(account), replacing: true };
    assert!(domain.keeping.insert(source, keeping) == Ok(None), "one keep per source, admitted");
    out.push(Request::Signing(Signing::Keep { source, carried, replaces: true }));
}
fn replaceable(domain: &Domain, limits: &Limits, holder: Holder, source: u32) -> Result<u32, Refusal> {
    let account = crate::changes::source(domain, holder, source)?;
    let kind = domain.sources.get(&source).expect("checked source").kind;
    if kind != Kind::Key {
        return Err(Refusal::Kind { kind });
    }
    let provider = domain.accounts.get(&account).expect("checked account").provider;
    match domain.providers.get(provider) {
        Some(offered) if offered.offers.key => {}
        Some(_) | None => return Err(Refusal::Provider { provider }),
    }
    ready(domain, limits)?;
    if domain.keeping.contains_key(&source) {
        return Err(Refusal::Limit { bound: Bound::SignIns, number: None });
    }
    Ok(account)
}

pub(crate) fn failed(domain: &mut Domain, source: u32, why: KeyFailure, out: &mut Queue<Request>) {
    let Some(keeping) = domain.keeping.remove(&source) else { return };
    answer(keeping.request, Refusal::NotKept { why }, out);
}

/// A successful keep creates its rows only now. Replacements keep their source number.
pub(crate) fn granted(domain: &mut Domain, env: &Env<Limits>, source: u32, out: &mut Queue<Request>) -> Option<Token> {
    let keeping = domain.keeping.remove(&source)?;
    if keeping.replacing {
        let entry = domain.sources.get_mut(&source).expect("a replacing source cannot be removed");
        entry.hold = None;
        entry.hold_attention = false;
    } else {
        let account = match keeping.join {
            Some(account) => account,
            None => {
                let account = domain.take();
                domain.counted(out);
                let entry = Account {
                    provider: keeping.provider,
                    holder: keeping.holder,
                    place: domain.last_place(keeping.holder),
                    uses: None,
                    plan: false,
                    subject: Box::new([]),
                    display: Box::new([]),
                    sources: Map::with_capacity(env.limits.sources),
                };
                assert!(domain.accounts.insert(account, entry).is_ok(), "reserved account room");
                out.push(Request::Save { record: crate::ordering::account_record(domain, account) });
                account
            }
        };
        let entry = domain.accounts.get_mut(&account).expect("a joining account cannot be removed");
        let place = entry.sources.len();
        assert!(entry.sources.insert(place, source) == Ok(None), "reserved source room and dense order");
        let entry = Source::new(account, Kind::Key, Box::new([]), &env.limits);
        assert!(domain.sources.insert(source, entry) == Ok(None), "reserved source room");
    }
    out.push(Request::Save { record: stored(domain, env, source) });
    Some(keeping.request)
}

pub(crate) fn remove(
    domain: &mut Domain,
    env: &Env<Limits>,
    request: Token,
    holder: Holder,
    source: u32,
    out: &mut Queue<Request>,
) {
    let checked = crate::changes::source(domain, holder, source);
    let account = match checked {
        Ok(account) => account,
        Err(refusal) => {
            answer(request, refusal, out);
            return;
        }
    };
    let kind = domain.sources.get(&source).expect("checked source").kind;
    if kind != Kind::Key {
        answer(request, Refusal::Kind { kind }, out);
        return;
    }
    for (_, keeping) in &domain.keeping {
        if keeping.join == Some(account) {
            answer(request, Refusal::Limit { bound: Bound::SignIns, number: None }, out);
            return;
        }
    }
    crate::changes::detach(domain, env, account, source, out);
    out.push(Request::Signing(Signing::Forget { source }));
    out.push(Request::Answer { request, answer: Answer::Done });
}

pub(crate) fn close(domain: &mut Domain, out: &mut Queue<Request>) {
    for (_, keeping) in &domain.keeping {
        out.push(Request::Answer { request: keeping.request, answer: Answer::Refused { refusal: Refusal::Closing } });
    }
    domain.keeping = Map::with_capacity(domain.keeping.capacity());
}
fn answer(request: Token, refusal: Refusal, out: &mut Queue<Request>) {
    out.push(Request::Answer { request, answer: Answer::Refused { refusal } });
}
