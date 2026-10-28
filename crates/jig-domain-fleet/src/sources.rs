//! Whole per-channel inventories and once-ended sign-outs (channel.md, 5).
use crate::{Domain, Limits, Request};
use crate::{Login, LoginState, SignOutResult, SourceName};
use alloc::boxed::Box;
use skein_lib::{Env, List, Map, Queue, ReplyTo, Set, Token};

#[derive(Debug)]
pub(crate) struct Sources {
    pub(crate) sent: Box<[SourceName]>,
    previous: Option<Box<[SourceName]>>,
    pub(crate) desired: Option<Box<[SourceName]>>,
    waiting: bool,
    logins: Box<[Login]>,
    pub(crate) pending: Map<SourceName, Pending>,
    pub(crate) checks: Set<SourceName>,
    pub(crate) results: Map<SourceName, SignOutResult>,
    retained_removed: bool,
}
impl Sources {
    pub(crate) fn new(names: Box<[SourceName]>, capacity: u32, waiting: bool) -> Self {
        Self {
            sent: names,
            previous: None,
            desired: None,
            waiting,
            logins: Box::new([]),
            pending: Map::with_capacity(capacity),
            checks: Set::with_capacity(capacity),
            results: Map::with_capacity(capacity),
            retained_removed: false,
        }
    }
    pub(crate) fn ready(&self) -> bool {
        if self.waiting || self.desired.is_some() || self.retained_removed {
            return false;
        }
        true
    }
    pub(crate) fn clear(&self) -> bool {
        if !self.ready() {
            return false;
        }
        for login in &self.logins {
            if login.state.kept() {
                return false;
            }
        }
        true
    }
    pub(crate) fn logins(&self) -> &[Login] {
        &self.logins
    }
    pub(crate) fn awaiting(&self) -> bool {
        self.waiting || self.desired.is_some()
    }
}
pub(crate) fn replace(domain: &mut Domain, env: &Env<Limits>, names: Box<[SourceName]>, out: &mut Queue<Request>) {
    assert!(
        names.len() <= usize::try_from(env.limits.program_sources).expect("source bound") && unique(&names),
        "bounded unique source declarations"
    );
    if same_names(&domain.program_sources, &names) {
        return;
    }
    domain.program_sources = names;
    for (_, &id) in &domain.tokens {
        let channel = domain.channels.get_mut(id).expect("current channel");
        if channel.kind == crate::HostKind::Engine {
            continue;
        }
        channel.sources.desired = Some(domain.program_sources.clone());
        send(&mut channel.sources, channel.token, out);
    }
    domain.placing = true;
}
fn send(sources: &mut Sources, channel: Token, out: &mut Queue<Request>) {
    if sources.waiting {
        return;
    }
    let Some(names) = sources.desired.take() else { return };
    if same_names(&sources.sent, &names) {
        return;
    }
    sources.previous = Some(core::mem::replace(&mut sources.sent, names));
    sources.waiting = true;
    let mut removed = List::with_capacity(sources.results.capacity());
    for (name, _) in &sources.results {
        if !sources.sent.contains(name) {
            removed.push(*name).expect("one per retained result");
        }
    }
    for name in removed.into_boxed() {
        drop(sources.results.remove(&name));
    }
    out.push(Request::Sources { channel, sources: sources.sent.clone() });
}
fn same_names(left: &[SourceName], right: &[SourceName]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    for name in left {
        if !right.contains(name) {
            return false;
        }
    }
    true
}
fn covers(names: &[SourceName], rows: &[Login]) -> bool {
    if names.len() != rows.len() {
        return false;
    }
    for name in names {
        let mut count = 0_u32;
        for row in rows {
            if row.name == *name {
                count = count.saturating_add(1);
            }
        }
        if count != 1 {
            return false;
        }
    }
    true
}
pub(crate) fn logins(
    domain: &mut Domain,
    env: &Env<Limits>,
    channel: Token,
    rows: Box<[Login]>,
    out: &mut Queue<Request>,
) {
    let Some(&id) = domain.tokens.get(&channel) else { return };
    let host = domain.channels.get_mut(id).expect("current channel");
    let sources = &mut host.sources;
    if host.kind == crate::HostKind::Engine {
        fault(domain, env, channel, out);
        return;
    }
    let mut bounded = rows.len() <= usize::try_from(env.limits.program_sources).expect("source bound");
    for row in &rows {
        match &row.state {
            LoginState::Failed { detail, .. } => {
                if detail.len() > usize::try_from(env.limits.source_detail_bytes).expect("detail bound") {
                    bounded = false;
                }
            }
            LoginState::Usable | LoginState::NotSignedIn | LoginState::SignedInOtherwise(_) => {}
        }
    }
    if bounded && covers(&sources.sent, &rows) {
        for old in &sources.logins {
            if old.state.kept() && !sources.sent.contains(&old.name) {
                sources.retained_removed = true;
            }
        }
        sources.logins = rows;
        sources.waiting = false;
        sources.previous = None;
        send(sources, channel, out);
        domain.placing = true;
        crate::programs::cleanup(domain, channel, out);
    } else if bounded
        && sources.waiting
        && let Some(previous) = &sources.previous
    {
        if !covers(previous, &rows) {
            fault(domain, env, channel, out);
        }
        // A whole previous-list snapshot updates nothing, including restrictions.
    } else {
        fault(domain, env, channel, out);
    }
}
fn fault(domain: &mut Domain, env: &Env<Limits>, channel: Token, out: &mut Queue<Request>) {
    out.push(Request::Refuse { channel });
    crate::channel::lost(domain, env, channel, out);
}
pub(crate) fn check(domain: &Domain, channel: Token, name: SourceName, out: &mut Queue<Request>) {
    let Some(&id) = domain.tokens.get(&channel) else { return };
    let host = domain.channels.get(id).expect("current channel");
    if host.kind == crate::HostKind::Worker && host.sources.sent.contains(&name) {
        out.push(Request::Check { channel, name });
    }
}
pub(crate) fn sign_out(domain: &mut Domain, to: ReplyTo, channel: Token, name: SourceName, out: &mut Queue<Request>) {
    let Some(&id) = domain.tokens.get(&channel) else {
        out.push(Request::SignedOut { to, channel, name, result: None });
        return;
    };
    let host = domain.channels.get_mut(id).expect("current channel");
    if host.kind == crate::HostKind::Engine
        || host.sources.pending.contains_key(&name)
        || host.sources.pending.len() == host.sources.pending.capacity()
    {
        out.push(Request::SignedOut { to, channel, name, result: None });
        return;
    }
    host.sources.pending.insert(name, Pending { reply: Reply::Direct(to), sent: true }).expect("bounded sign-outs");
    out.push(Request::SignOut { channel, name });
}
pub(crate) fn signed_out(
    domain: &mut Domain,
    env: &Env<Limits>,
    channel: Token,
    name: SourceName,
    result: SignOutResult,
    out: &mut Queue<Request>,
) {
    let Some(&id) = domain.tokens.get(&channel) else { return };
    match &result {
        SignOutResult::Failed { detail, .. } => {
            if detail.len() > usize::try_from(env.limits.source_detail_bytes).expect("detail bound") {
                fault(domain, env, channel, out);
                return;
            }
        }
        SignOutResult::SignedOut => {}
    }
    let host = domain.channels.get_mut(id).expect("current channel");
    let Some(to) = host.sources.pending.remove(&name) else {
        fault(domain, env, channel, out);
        return;
    };
    if host.sources.sent.contains(&name) {
        host.sources.results.insert(name, result.clone()).expect("one result per declared source");
    }
    terminal(to.reply, channel, name, Some(result), out);
    // Even success is not a check of the login; restrictions wait for Logins.
}
pub(crate) fn lost(sources: &mut Sources, channel: Token, out: &mut Queue<Request>) {
    let mut names = List::with_capacity(sources.pending.len());
    for (name, _) in &sources.pending {
        names.push(*name).expect("pending count");
    }
    for name in names.into_boxed() {
        let to = sources.pending.remove(&name).expect("pending call");
        terminal(to.reply, channel, name, None, out);
    }
}

fn unique(names: &[SourceName]) -> bool {
    for (index, name) in names.iter().enumerate() {
        for other in names.iter().skip(index.saturating_add(1)) {
            if name == other {
                return false;
            }
        }
    }
    true
}

#[derive(Debug)]
pub(crate) enum Reply {
    Direct(ReplyTo),
    DirectProgram { to: ReplyTo, source: u32, worker: crate::Worker },
    Program { source: u32, worker: crate::Worker },
    Cleanup,
}
#[derive(Debug)]
pub(crate) struct Pending {
    pub(crate) reply: Reply,
    pub(crate) sent: bool,
}
pub(crate) fn terminal(
    reply: Reply,
    channel: Token,
    name: SourceName,
    result: Option<SignOutResult>,
    out: &mut Queue<Request>,
) {
    match reply {
        Reply::Direct(to) => out.push(Request::SignedOut { to, channel, name, result }),
        Reply::DirectProgram { to, source, worker } => {
            let terminal = program_result(result.as_ref());
            out.push(Request::SignedOut { to, channel, name, result });
            out.push(Request::ProgramSignedOut { source, worker, result: terminal });
        }
        Reply::Program { source, worker } => {
            let result = program_result(result.as_ref());
            out.push(Request::ProgramSignedOut { source, worker, result });
        }
        Reply::Cleanup => {}
    }
}

fn program_result(result: Option<&SignOutResult>) -> crate::ProgramResult {
    match result {
        Some(SignOutResult::SignedOut) => crate::ProgramResult::SignedOut,
        Some(SignOutResult::Failed { .. }) => crate::ProgramResult::Failed,
        None => crate::ProgramResult::Lost,
    }
}
