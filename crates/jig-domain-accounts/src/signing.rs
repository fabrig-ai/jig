//! A party's sign-in and sign-out (accounts.md, 3.3 and section 5; protocol/llm.md, 5.2).
//!
//! A sign-in takes its source's number as it starts, so that the component's record is named by
//! it from the first; the counter is saved then. The component's answer decides: a new subject
//! makes an account, the holder's own renews it with this source in its signed-in source's
//! place, and anything else is refused and the new record revoked. Plan usage declined keeps
//! the sign-in and refuses the source. A sign-out removes the source at once, and its account
//! with its last source, since the store keeps no account without one (`restore::finish`); the
//! party is answered when the component says whether the provider confirmed the revocation.
use crate::checks::room_for_sign_in;
use crate::domain::{Account, Hold, Source, changed, stored};
use crate::records::within;
use crate::{
    Answer, Bound, DISPLAY, Domain, End, Ended, Failure, Holder, Key, Kind, Limits, Refusal, Request, SignIn, Signing,
    ToComponent,
};
use alloc::boxed::Box;
use skein_lib::{Env, Map, Queue, Token};

/// A sign-in under way: whose request, for which holder, at which provider.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct UnderWay {
    request: Token,
    holder: Holder,
    provider: u32,
    visited: bool,
    by: u64,
    finish: Option<Token>,
    taken: bool,
}

impl UnderWay {
    pub(crate) fn pending(&self, source: u32) -> crate::Pending {
        crate::Pending { source, provider: self.provider, holder: self.holder, by: self.by }
    }
}

/// An account sign-in starts within its bound; its visit answers the party.
pub(crate) fn sign_in(
    domain: &mut Domain,
    env: &Env<Limits>,
    request: Token,
    holder: Holder,
    by: u64,
    provider: u32,
    out: &mut Queue<Request>,
) {
    // Remove the cancelled flight now: its delayed component terminal is stale, and its
    // place in the shared sign-ins bound is available to its replacement immediately.
    let mut previous = None;
    for (source, pending) in &domain.signing {
        if pending.by == by && pending.provider == provider && pending.holder == holder && !pending.taken {
            previous = Some(*source);
        }
    }
    if let Some(source) = previous {
        out.push(Request::Signing(Signing::CancelSignIn { source }));
        failed(domain, source, Failure::Cancelled, out);
    }
    if let Err(refusal) = startable(domain, &env.limits, holder, provider) {
        out.push(Request::Answer { request, answer: Answer::Refused { refusal } });
        return;
    }
    let source = domain.take();
    domain.counted(out);
    let entry = UnderWay { request, holder, provider, visited: false, by, finish: None, taken: false };
    assert!(domain.signing.insert(source, entry) == Ok(None), "a new number, within the sign-ins' bound");
    out.push(Request::Signing(Signing::SignIn { source, provider }));
}
/// Answer the account sign-in once its component has a listener ready.
pub(crate) fn visited(domain: &mut Domain, source: u32, address: Box<[u8]>, out: &mut Queue<Request>) {
    let Some(under_way) = domain.signing.get_mut(&source) else { return };
    if under_way.visited {
        return;
    }
    assert!(within(&address, crate::ADDRESS), "the component bounds the account sign-in address");
    under_way.visited = true;
    out.push(Request::Answer { request: under_way.request, answer: Answer::Started { source, address } });
}

/// Admit only the starter's waiting sign-in, with one finish outstanding.
pub(crate) fn finish(
    domain: &mut Domain,
    request: Token,
    source: u32,
    carried: u64,
    by: u64,
    out: &mut Queue<Request>,
) {
    if let Some(pending) = domain.signing.get_mut(&source)
        && pending.by == by
        && pending.visited
        && !pending.taken
        && pending.finish.is_none()
    {
        pending.finish = Some(request);
        out.push(Request::Signing(Signing::Redirected { source, carried }));
        return;
    }
    crate::changes::refuse(request, Refusal::SignInRedirect { why: crate::SignInRedirectWhy::NotWaiting }, out);
}

/// Taken ends the finish, while the sign-in awaits its own terminal.
pub(crate) fn finishing(domain: &mut Domain, source: u32, out: &mut Queue<Request>) {
    let Some(pending) = domain.signing.get_mut(&source) else { return };
    pending.taken = true;
    if let Some(request) = pending.finish.take() {
        out.push(Request::Answer { request, answer: Answer::Finishing { source } });
    }
}

pub(crate) fn finish_refused(
    domain: &mut Domain,
    source: u32,
    why: crate::SignInRedirectWhy,
    out: &mut Queue<Request>,
) {
    let Some(pending) = domain.signing.get_mut(&source) else { return };
    if let Some(request) = pending.finish.take() {
        crate::changes::refuse(request, Refusal::SignInRedirect { why }, out);
    }
}

fn finish_ended(pending: &UnderWay, out: &mut Queue<Request>) {
    if let Some(request) = pending.finish {
        crate::changes::refuse(request, Refusal::SignInRedirect { why: crate::SignInRedirectWhy::NotWaiting }, out);
    }
}

/// An account sign-in that ended before a visit still owes its party an answer.
fn not_started(under_way: &UnderWay, out: &mut Queue<Request>) {
    finish_ended(under_way, out);
    if !under_way.visited {
        out.push(Request::Answer {
            request: under_way.request,
            answer: Answer::Refused { refusal: Refusal::NotStarted },
        });
    }
}

/// Close answers account sign-ins still waiting for their visit, before forgetting them.
pub(crate) fn close(domain: &mut Domain, out: &mut Queue<Request>) {
    for (source, under_way) in &domain.signing {
        finish_ended(under_way, out);
        out.push(Request::Signing(Signing::CancelSignIn { source: *source }));
        if !under_way.visited {
            out.push(Request::Answer {
                request: under_way.request,
                answer: Answer::Refused { refusal: Refusal::Closing },
            });
        }
    }
    domain.signing = Map::with_capacity(domain.signing.capacity());
}

fn startable(domain: &Domain, limits: &Limits, holder: Holder, provider: u32) -> Result<(), Refusal> {
    if !domain.wanted() {
        return Err(Refusal::Closing);
    }
    match domain.providers.get(provider) {
        Some(offered) if offered.offers.sign_in && !(holder == Holder::Deployment && offered.signed_in_plan) => {}
        Some(_) | None => return Err(Refusal::Provider { provider }),
    }
    room_for_sign_in(limits, domain.sign_ins())?;
    if domain.numbers_left() < 1 {
        return Err(Refusal::Numbers);
    }
    Ok(())
}

/// The component's sign-in decides what the subject does (accounts.md, 3.3).
pub(crate) fn signed_in(
    domain: &mut Domain,
    env: &Env<Limits>,
    source: u32,
    subject: Box<[u8]>,
    display: Box<[u8]>,
    declined: bool,
    out: &mut Queue<Request>,
) {
    let Some(under_way) = domain.signing.remove(&source) else { return };
    not_started(&under_way, out);
    let decided = match domain.sign_in(&env.limits, under_way.holder, under_way.provider, &subject) {
        Ok(SignIn::New) => Ok(None),
        Ok(SignIn::Renew { account }) => match place(domain, account) {
            Some(place) => Ok(Some((account, place))),
            None => Err(Refusal::limited(Bound::Sources, crate::LimitKey::Sources, env.limits.sources)),
        },
        Err(refusal) => Err(refusal),
    };
    let hold = if declined { Some(Hold::Refused) } else { None };
    let end = match decided {
        Ok(None) => {
            let made = Made { holder: under_way.holder, provider: under_way.provider, subject, display };
            End::Made { account: make(domain, env, source, made, hold, out), declined }
        }
        Ok(Some((account, place))) => {
            renew(domain, env, account, place, source, hold, out);
            End::Renewed { account, declined }
        }
        Err(refusal) => {
            out.push(Request::Signing(Signing::SignOut { source }));
            End::Refused { refusal }
        }
    };
    domain.end(Ended { request: under_way.request, source, by: under_way.by, end });
}

/// The component's sign-in failed or was cancelled: it ends, with nothing made.
pub(crate) fn failed(domain: &mut Domain, source: u32, failure: Failure, out: &mut Queue<Request>) {
    let Some(under_way) = domain.signing.remove(&source) else { return };
    not_started(&under_way, out);
    domain.end(Ended { request: under_way.request, source, by: under_way.by, end: End::Failed { failure } });
}

/// Where a renewal's source goes: its account's signed-in source's place, or, with none, the
/// place after its last; none when the places are spent.
fn place(domain: &Domain, account: u32) -> Option<u32> {
    let entry = domain.accounts.get(&account)?;
    for (at, number) in &entry.sources {
        if domain.sources.get(number).expect("an account's sources are kept").kind == Kind::SignIn {
            return Some(*at);
        }
    }
    match entry.sources.last() {
        Some((last, _)) => last.checked_add(1),
        None => Some(0),
    }
}

struct Made {
    holder: Holder,
    provider: u32,
    subject: Box<[u8]>,
    display: Box<[u8]>,
}

/// A new account, with the provider's display name where it fits, and the source in its first
/// place; the account's number is taken now, and the counter saved.
fn make(
    domain: &mut Domain,
    env: &Env<Limits>,
    source: u32,
    made: Made,
    hold: Option<Hold>,
    out: &mut Queue<Request>,
) -> u32 {
    let account = domain.take();
    domain.counted(out);
    let display = if within(&made.display, DISPLAY) { made.display } else { Box::new([]) };
    out.push(Request::Save {
        record: crate::Stored::Account {
            number: account,
            provider: made.provider,
            holder: made.holder,
            place: domain.last_place(made.holder),
            uses: None,
            subject: made.subject.clone(),
            display: display.clone(),
        },
    });
    let entry = Account {
        provider: made.provider,
        holder: made.holder,
        place: domain.last_place(made.holder),
        uses: None,
        subject: made.subject,
        display,
        sources: Map::with_capacity(env.limits.sources),
        plan: domain.providers.get(made.provider).expect("a checked provider").signed_in_plan,
    };
    let fresh = match domain.accounts.insert(account, entry) {
        Ok(old) => old.is_none(),
        Err(_) => false,
    };
    assert!(fresh, "a new number, with room checked");
    join(domain, env, account, 0, source, hold, out);
    account
}

/// The holder's account of the subject renewed: this source takes its signed-in source's
/// place, which is erased and replaced at the component.
fn renew(
    domain: &mut Domain,
    env: &Env<Limits>,
    account: u32,
    place: u32,
    source: u32,
    hold: Option<Hold>,
    out: &mut Queue<Request>,
) {
    let entry = domain.accounts.get(&account).expect("a renewed account is kept");
    if let Some(old) = entry.sources.get(&place).copied() {
        drop(domain.sources.remove(&old).expect("an account's sources are kept"));
        domain.alarms.cancel(old);
        out.push(Request::Erase { key: Key::Source { account, number: old } });
        out.push(Request::Signing(Signing::Replaced { source: old, by: source }));
    }
    join(domain, env, account, place, source, hold, out);
}

/// The source joins its account at its place, is saved, and is wanted; refused when plan usage
/// was declined, until its holder acts (accounts.md, section 5).
fn join(
    domain: &mut Domain,
    env: &Env<Limits>,
    account: u32,
    place: u32,
    source: u32,
    hold: Option<Hold>,
    out: &mut Queue<Request>,
) {
    let mut entry = Source::new(account, Kind::SignIn, Box::new([]), &env.limits);
    entry.hold = hold;
    entry.hold_attention = hold == Some(Hold::Refused);
    assert!(domain.sources.insert(source, entry) == Ok(None), "a new number, within the sources' bound");
    let account_entry = domain.accounts.get_mut(&account).expect("a joined account is kept");
    account_entry.plan |= domain.providers.get(account_entry.provider).expect("a configured provider").signed_in_plan;
    let places = &mut account_entry.sources;
    assert!(places.insert(place, source).is_ok(), "a place checked within the sources' bound");
    out.push(Request::Save { record: stored(domain, env, source) });
    if hold == Some(Hold::Moving) {
        out.push(Request::Signing(Signing::Refresh { source }));
    }
    out.push(Request::Component(ToComponent::Want { account: source }));
    changed(domain, env, source, hold, out);
}

/// A signed-in source of the holder's is removed, and its account with its last source; the
/// component revokes its record, and the party is answered when it says so.
pub(crate) fn sign_out(
    domain: &mut Domain,
    env: &Env<Limits>,
    request: Token,
    holder: Holder,
    source: u32,
    out: &mut Queue<Request>,
) {
    if domain.issued.contains_key(&source) {
        if let Err(refusal) = crate::programs::sign_out(domain, env, request, holder, source, out) {
            out.push(Request::Answer { request, answer: Answer::Refused { refusal } });
        }
        return;
    }
    let account = match removable(domain, holder, source) {
        Ok(account) => account,
        Err(refusal) => {
            out.push(Request::Answer { request, answer: Answer::Refused { refusal } });
            return;
        }
    };

    crate::changes::detach(domain, env, account, source, out);
    assert!(domain.signing_out.insert(source, crate::changes::Leaving::Source(request)) == Ok(None), "room checked");
    out.push(Request::Signing(Signing::SignOut { source }));
}
fn removable(domain: &Domain, holder: Holder, source: u32) -> Result<u32, Refusal> {
    let account = crate::changes::source(domain, holder, source)?;
    if domain.moves.contains_key(&source) {
        return Err(Refusal::Moving);
    }
    for (_, keeping) in &domain.keeping {
        if keeping.join == Some(account) {
            return Err(Refusal::Limit { bound: Bound::SignIns, number: None });
        }
    }
    let entry = domain.sources.get(&source).expect("checked source");
    match entry.kind {
        Kind::SignIn | Kind::Program => {}
        Kind::Key | Kind::None => return Err(Refusal::Kind { kind: entry.kind }),
    }
    if domain.signing_out.contains_key(&source) {
        return Err(Refusal::Limit { bound: Bound::Sources, number: None });
    }
    if domain.signing_out.len() >= domain.signing_out.capacity() {
        return Err(Refusal::fixed(Bound::Sources, domain.signing_out.capacity()));
    }
    Ok(entry.account)
}

/// The component revoked and forgot a source's record: a party's sign-out is answered.
pub(crate) fn signed_out(domain: &mut Domain, source: u32, confirmed: bool, out: &mut Queue<Request>) {
    if let Some(leaving) = domain.signing_out.remove(&source) {
        match leaving {
            crate::changes::Leaving::Source(request) => {
                out.push(Request::Answer {
                    request,
                    answer: Answer::SignedOut { confirmed, failed_hosts: Box::new([]) },
                });
            }
            crate::changes::Leaving::Account(account) => {
                crate::changes::signed_out(domain, account, confirmed, Box::new([]), out);
            }
        }
    }
}

/// A move owns a number apart from its source's (accounts.md, section 6).
#[derive(PartialEq, Eq, Debug)]
pub(crate) struct Move {
    request: Token,
    holder: Holder,
    number: u32,
    out: bool,
}
#[derive(PartialEq, Eq, Debug)]
pub(crate) struct Arriving {
    request: Token,
    holder: Holder,
    carried: u64,
    opened: Option<Opened>,
}
#[derive(PartialEq, Eq, Debug)]
struct Opened {
    provider: u32,
    subject: Box<[u8]>,
    display: Box<[u8]>,
}

/// Start withdrawing a signed-in source, saving its moving hold before releasing it.
pub(crate) fn move_out(
    domain: &mut Domain,
    env: &Env<Limits>,
    request: Token,
    holder: Holder,
    account: u32,
    out: &mut Queue<Request>,
) {
    let source = match movable(domain, &env.limits, holder, account) {
        Ok(source) => source,
        Err(refusal) => return crate::changes::refuse(request, refusal, out),
    };
    let number = domain.take();
    domain.counted(out);
    let entry = Move { request, holder, number, out: false };
    assert!(domain.moves.insert(source, entry) == Ok(None), "one admitted move per source");
    crate::domain::moving(domain, env, source, out);
    let display = domain.accounts.get(&account).expect("a checked account").display.clone();
    out.push(Request::Component(ToComponent::Release { account: source }));
    out.push(Request::Signing(Signing::Withdraw { source, display }));
}
fn movable(domain: &Domain, limits: &Limits, holder: Holder, account: u32) -> Result<u32, Refusal> {
    crate::changes::account(domain, holder, account)?;
    let entry = domain.accounts.get(&account).expect("a checked account");
    let mut signed_in = None;
    for (_, source) in &entry.sources {
        if domain.sources.get(source).expect("an account's source").kind == Kind::SignIn {
            signed_in = Some(*source);
        }
    }
    let Some(source) = signed_in else { return Err(Refusal::NotSignedIn) };
    if domain.moves.contains_key(&source) || domain.signing_out.contains_key(&source) {
        return Err(Refusal::Moving);
    }
    match domain.source(source).expect("a checked source").state {
        crate::State::NeedsSignIn | crate::State::Unloaded { .. } => return Err(Refusal::NotSignedIn),
        crate::State::Moving => return Err(Refusal::Moving),
        crate::State::Asking
        | crate::State::Granted
        | crate::State::Later { .. }
        | crate::State::Limited { .. }
        | crate::State::Refused
        | crate::State::Misconfigured
        | crate::State::Cancelled
        | crate::State::Closing => {}
    }
    for (_, keeping) in &domain.keeping {
        if keeping.join == Some(account) {
            return Err(Refusal::Limit { bound: Bound::SignIns, number: None });
        }
    }
    room_for_sign_in(limits, domain.sign_ins())?;
    if domain.numbers_left() == 0 {
        return Err(Refusal::Numbers);
    }
    Ok(source)
}

/// Only a readable withdrawal answers moving out and starts the client's deadline.
pub(crate) fn withdrawn(domain: &mut Domain, env: &Env<Limits>, source: u32, readable: bool, out: &mut Queue<Request>) {
    let Some(moving) = domain.moves.get_mut(&source) else { return };
    if moving.out {
        return;
    }
    if !readable {
        crate::changes::refuse(moving.request, Refusal::NotSignedIn, out);
        cancel_move(domain, env, source, out);
        return;
    }
    moving.out = true;
    let account = domain.sources.get(&source).expect("a withdrawn source is kept").account;
    out.push(Request::Answer {
        request: moving.request,
        answer: Answer::MovingOut { account, number: moving.number, source },
    });
    domain.move_alarms.arm(source, env.now.saturating_add(env.limits.move_deadline)).expect("one alarm per move");
}

/// Written consumes only this holder's answered move, never a withdrawn or cancelled one.
pub(crate) fn written(
    domain: &mut Domain,
    env: &Env<Limits>,
    request: Token,
    holder: Holder,
    number: u32,
    out: &mut Queue<Request>,
) {
    let mut found = None;
    for (source, moving) in &domain.moves {
        if moving.number == number && moving.holder == holder && moving.out {
            found = Some(*source);
        }
    }
    let Some(source) = found else { return crate::changes::refuse(request, Refusal::Cancelled, out) };
    domain.moves.remove(&source).expect("a found move");
    domain.move_alarms.cancel(source);
    let account = domain.sources.get(&source).expect("a moved source").account;
    crate::changes::detach(domain, env, account, source, out);
    out.push(Request::Signing(Signing::Forget { source }));
    out.push(Request::Answer { request, answer: Answer::Done });
}

/// A cancelled move keeps its durable hold until the returned record grants a new generation.
pub(crate) fn cancel_move(domain: &mut Domain, env: &Env<Limits>, source: u32, out: &mut Queue<Request>) {
    let Some(_moving) = domain.moves.remove(&source) else { return };
    domain.move_alarms.cancel(source);
    crate::domain::moving(domain, env, source, out);
    out.push(Request::Signing(Signing::Return { source }));
    out.push(Request::Signing(Signing::Refresh { source }));
    out.push(Request::Component(ToComponent::Want { account: source }));
}

pub(crate) fn move_in(
    domain: &mut Domain,
    env: &Env<Limits>,
    request: Token,
    holder: Holder,
    carried: u64,
    out: &mut Queue<Request>,
) {
    if !domain.wanted() {
        return crate::changes::refuse(request, Refusal::Closing, out);
    }
    if let Err(refusal) = room_for_sign_in(&env.limits, domain.sign_ins()) {
        return crate::changes::refuse(request, refusal, out);
    }
    if domain.numbers_left() < 2 {
        return crate::changes::refuse(request, Refusal::Numbers, out);
    }
    let source = domain.take();
    domain.counted(out);
    let entry = Arriving { request, holder, carried, opened: None };
    assert!(domain.arriving.insert(source, entry) == Ok(None), "a new source within the shared bound");
    out.push(Request::Signing(Signing::Open { source, carried }));
}

/// The opened record is checked before adoption, and again once its adoption is durable.
pub(crate) fn opened(
    domain: &mut Domain,
    env: &Env<Limits>,
    source: u32,
    provider: u32,
    subject: Box<[u8]>,
    display: Box<[u8]>,
    out: &mut Queue<Request>,
) {
    let Some(arriving) = domain.arriving.get(&source) else { return };
    if arriving.opened.is_some() {
        return;
    }
    assert!(within(&subject, crate::SUBJECT) && within(&display, DISPLAY), "the protocol bounds the file's parts");
    if let Err(refusal) = arriving_at(domain, &env.limits, arriving.holder, provider, &subject) {
        let arriving = domain.arriving.remove(&source).expect("an opened arrival");
        out.push(Request::Signing(Signing::Forget { source }));
        return crate::changes::refuse(arriving.request, refusal, out);
    }
    let arriving = domain.arriving.get_mut(&source).expect("a checked arrival");
    arriving.opened = Some(Opened { provider, subject, display });
    out.push(Request::Signing(Signing::Adopt { source, carried: arriving.carried }));
}
fn arriving_at(
    domain: &Domain,
    limits: &Limits,
    holder: Holder,
    provider: u32,
    subject: &[u8],
) -> Result<Option<(u32, u32)>, Refusal> {
    match domain.providers.get(provider) {
        Some(offered) if offered.offers.sign_in && !(holder == Holder::Deployment && offered.signed_in_plan) => {}
        Some(_) | None => return Err(Refusal::Provider { provider }),
    }
    match domain.sign_in(limits, holder, provider, subject)? {
        SignIn::New => Ok(None),
        SignIn::Renew { account } => match place(domain, account) {
            Some(place) => Ok(Some((account, place))),
            None => Err(Refusal::limited(Bound::Sources, crate::LimitKey::Sources, limits.sources)),
        },
    }
}
pub(crate) fn not_ours(domain: &mut Domain, source: u32, out: &mut Queue<Request>) {
    if let Some(arriving) = domain.arriving.remove(&source) {
        crate::changes::refuse(arriving.request, Refusal::NotOurs, out);
    }
}
pub(crate) fn adopted(domain: &mut Domain, env: &Env<Limits>, source: u32, kept: bool, out: &mut Queue<Request>) {
    let Some(arriving) = domain.arriving.get(&source) else { return };
    if arriving.opened.is_none() {
        return;
    }
    let arriving = domain.arriving.remove(&source).expect("an adopted arrival");
    let opened = arriving.opened.expect("the record was opened before adoption");
    if !kept {
        return crate::changes::refuse(arriving.request, Refusal::NotKept { why: crate::KeyFailure::NotStored }, out);
    }
    let decided = arriving_at(domain, &env.limits, arriving.holder, opened.provider, &opened.subject);
    let (account, renewed) = match decided {
        Ok(None) => {
            let made = Made {
                holder: arriving.holder,
                provider: opened.provider,
                subject: opened.subject,
                display: opened.display,
            };
            (make(domain, env, source, made, Some(Hold::Moving), out), false)
        }
        Ok(Some((account, place))) => {
            renew(domain, env, account, place, source, Some(Hold::Moving), out);
            (account, true)
        }
        Err(refusal) => {
            out.push(Request::Signing(Signing::Forget { source }));
            return crate::changes::refuse(arriving.request, refusal, out);
        }
    };
    out.push(Request::Answer { request: arriving.request, answer: Answer::MovedIn { account, renewed } });
}

/// Pending requests get their one terminal; answered moves retain only the store's hold.
pub(crate) fn close_moves(domain: &mut Domain, out: &mut Queue<Request>) {
    for (_, moving) in &domain.moves {
        if !moving.out {
            crate::changes::refuse(moving.request, Refusal::Closing, out);
        }
    }
    for (_, arriving) in &domain.arriving {
        crate::changes::refuse(arriving.request, Refusal::Closing, out);
    }
    domain.moves = Map::with_capacity(domain.moves.capacity());
    domain.move_alarms = skein_lib::Deadlines::with_capacity(domain.moves.capacity());
    domain.arriving = Map::with_capacity(domain.arriving.capacity());
}
