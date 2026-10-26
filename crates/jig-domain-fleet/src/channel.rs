//! The hosts with slots (domain/engine.md, section 8): workers are known by
//! their channel for delivery and by the configured worker position each channel
//! authenticated as for program logins and their answers. A returning worker
//! gets a fresh channel; stale channel results never update its replacement.
//! The engine host uses reserved token zero and never loses contact.
//!
//! A hello is the worker's entrance. It is turned away, its channel to be
//! closed, if the fleet has no room for another worker or its declared stop
//! bound is not strictly below the engine's grace. Its slots are
//! used up to the limit, and the workstreams it lists up to the limit, each
//! a task number. Each run it lists, up to the slots a worker
//! may have, is kept, cancelled again, acknowledged or found (see the
//! attempts' table); one beyond them, or beyond the room the fleet keeps for
//! listings, is cancelled and not tracked; and past twice the slots, a
//! listing is dropped and counted, to bound what a hello emits. A second
//! hello on a channel breaks the worker's contract, and is counted and
//! dropped.
//!
//! A lost channel's attempts are kept for the grace, adrift, and the worker
//! remains in bounded display history. A worker that refuses an assignment as busy is shutting
//! down, or fuller than the fleet knew: nothing more is placed on it until
//! it frees a slot.
//!
//! Placement requires every model of the run's charter, then takes a worker
//! in contact with a free slot that holds the
//! workstream's workspace, the first in the order of the protocol's names for
//! the channels; or, with none, the one with the most free slots. A worker
//! holds the workstreams its hello listed and those of the runs placed on it
//! since: placing one beyond the room evicts the one used longest ago.

use alloc::boxed::Box;
use core::mem;

use skein_lib::{Env, Id, Map, Queue, Set, Slab, Token};

use crate::attempt::{self, Attempt};
use crate::boundary::{Hello, HostKind, Hosted, Kinds, Phase, Request};
use crate::domain::Domain;
use crate::facts::Fact;
use crate::limits::Limits;

/// A worker in contact.
#[derive(Debug)]
pub(crate) struct Channel {
    pub(crate) evidence: crate::Bounds,
    pub(crate) program_endpoints: Box<[u32]>,
    pub(crate) sources: crate::sources::Sources,
    pub(crate) worker: Option<u32>,
    /// Model eligibility in configured charter order, independent of free slots.
    pub(crate) eligible: Box<[bool]>,
    pub(crate) declarations: Box<[bool]>,
    /// Which kind of host owns these slots.
    pub(crate) kind: HostKind,
    /// The protocol's name for its channel.
    pub(crate) token: Token,
    /// How many runs it hosts at once, as its hello said, within the limits.
    pub(crate) slots: u32,
    /// It refused an assignment as busy, and has freed no slot since:
    /// nothing is placed on it.
    pub(crate) draining: bool,
    /// The attempts it hosts or holds the answers of, each taking a slot.
    pub(crate) hosts: Set<Id<Attempt>>,
    /// The workstreams it holds workspaces for, by when each was last used,
    /// and the count that orders them.
    pub(crate) workstreams: Map<u64, u64>,
    pub(crate) uses: u64,
}

/// The protocol's name for the channel `channel`, which is in contact.
pub(crate) fn token(channels: &Slab<Channel>, channel: Id<Channel>) -> Token {
    channels.get(channel).expect("an attempt is on a worker in contact").token
}

/// Whether `channel` holds `workstream`, and when it last used it.
fn holds(channel: &Channel, workstream: u64) -> Option<u64> {
    for (&used, key) in &channel.workstreams {
        if *key == workstream {
            return Some(used);
        }
    }
    None
}

/// `channel` holds `workstream` from now on, its latest used: in
/// place of the one used longest ago if it holds as many as it may. Task number zero is not kept.
pub(crate) fn cache(channel: &mut Channel, limits: &Limits, workstream: u64) {
    if workstream == 0 || limits.workstreams == 0 {
        return;
    }
    let key = match holds(channel, workstream) {
        Some(used) => channel.workstreams.remove(&used).expect("held above"),
        None => {
            if channel.workstreams.len() >= limits.workstreams
                && let Some((&oldest, _)) = channel.workstreams.first()
            {
                channel.workstreams.remove(&oldest);
            }
            workstream
        }
    };
    let used = channel.uses;
    channel.uses = used.checked_add(1).expect("a u64 counts every use");
    let room = channel.workstreams.insert(used, key);
    assert!(room.is_ok(), "room was made above");
}

fn admit_worker(
    domain: &mut Domain,
    env: &Env<Limits>,
    channel: Token,
    worker: u32,
    hello: &Hello,
    out: &mut Queue<Request>,
) -> bool {
    let configured = domain.worker_names.len();
    let mut occupied = false;
    for (_, id) in &domain.tokens {
        if let Some(host) = domain.channels.get(*id) {
            occupied |= host.worker == Some(worker);
        }
    }
    if worker >= configured || occupied {
        refuse(domain, env, channel, hello, crate::HelloRefusal::Position { worker }, out);
        return false;
    }
    true
}

/// Hello, on a new channel: the worker is in contact, or turned away.
#[expect(clippy::too_many_lines, reason = "one hello admits a host and its immutable endpoint evidence atomically")]
pub(crate) fn hello(
    domain: &mut Domain,
    env: &Env<Limits>,
    channel: Token,
    worker: u32,
    hello: Hello,
    out: &mut Queue<Request>,
) {
    let limits = &env.limits;
    let engine = channel == Token::new(0);
    let reserved = match domain.engine_hello {
        crate::domain::EngineHello::Expected => false,
        crate::domain::EngineHello::Heard | crate::domain::EngineHello::Admitted => true,
    };
    if engine && reserved {
        refuse(domain, env, channel, &hello, crate::HelloRefusal::ReservedChannel, out);
        return;
    }
    if engine {
        domain.engine_hello = crate::domain::EngineHello::Heard;
    }
    if !engine && domain.tokens.contains_key(&channel) {
        domain.facts.push(Fact::Dropped);
        return;
    }
    if !crate::valid_program_endpoints(&hello.bounds.program_endpoints) {
        // Malformed metadata has no charter shortfall: reject the channel without
        // publishing an invented admission dimension.
        domain.facts.push(Fact::Dropped);
        out.push(Request::Refuse { channel });
        return;
    }
    let kind = if engine { HostKind::Engine } else { HostKind::Worker };
    if let Some(why) = crate::needs::shortfall(&domain.needs, &hello.bounds, kind) {
        refuse(domain, env, channel, &hello, crate::HelloRefusal::Agent { why }, out);
        return;
    }
    if engine {
        if let Some(&id) = domain.tokens.get(&channel) {
            let host = domain.channels.get_mut(id).expect("engine slots reserved");
            // Drop the construction bits before allocating their replacement.
            host.eligible = Box::new([]);
            host.eligible = crate::needs::eligibility(&domain.needs, &hello.bounds.models);
            host.declarations = crate::needs::declarations(&domain.needs, &hello.bounds.models);
            host.evidence = crate::needs::evidence(&domain.catalogue, &hello.bounds);
            host.program_endpoints = hello.bounds.program_endpoints;
            domain.engine_hello = crate::domain::EngineHello::Admitted;
            domain.standing_engine = None;
            domain.placing = true;
        }
        return;
    }
    if !admit_worker(domain, env, channel, worker, &hello, out) {
        return;
    }
    let too_long = hello.stop_bound >= limits.grace;
    if domain.channels.is_full() || too_long {
        let why = if too_long {
            crate::HelloRefusal::StopBound { grace: limits.grace }
        } else {
            crate::HelloRefusal::Workers { limit: limits.workers }
        };
        refuse(domain, env, channel, &hello, why, out);
        return;
    }
    let Hello { slots, workstreams, hosting, stop_bound: _, bounds } = hello;
    let mut worker = Channel {
        evidence: crate::needs::evidence(&domain.catalogue, &bounds),
        program_endpoints: bounds.program_endpoints,
        declarations: crate::needs::declarations(&domain.needs, &bounds.models),
        worker: Some(worker),
        sources: crate::sources::Sources::new(domain.program_sources.clone(), limits.program_sources, true),
        eligible: crate::needs::eligibility(&domain.needs, &bounds.models),
        kind: HostKind::Worker,
        token: channel,
        slots: slots.min(limits.slots),
        draining: false,
        hosts: Set::with_capacity(limits.slots),
        workstreams: Map::with_capacity(limits.workstreams),
        uses: 0,
    };
    for workstream in &workstreams {
        cache(&mut worker, limits, *workstream);
    }
    let Ok(id) = domain.channels.insert(worker) else {
        unreachable!("checked for room above");
    };
    let named = domain.tokens.insert(channel, id);
    assert!(named == Ok(None), "a channel is named once, with room for every worker");
    domain.standing_worker = None;
    let most = limits.slots.saturating_mul(2);
    let mut listed: u32 = 0;
    for Hosted { run, attempt, phase } in hosting {
        let answered = match phase {
            Phase::Answered => true,
            Phase::Preparing | Phase::Starting | Phase::Active | Phase::Waiting | Phase::Ending => false,
        };
        if listed >= most {
            domain.facts.push(Fact::Dropped);
            continue;
        }
        if listed >= limits.slots {
            // More than a worker may host: cancelled, and not tracked.
            domain.facts.push(Fact::Fenced);
            if !answered {
                out.push(Request::Cancel { channel, run, attempt });
            }
        } else {
            attempt::listed(domain, env, id, run, attempt, answered, out);
        }
        listed = listed.saturating_add(1);
    }
    out.push(Request::Sources { channel, sources: domain.program_sources.clone() });
    domain.facts.push(Fact::Hello { listed });
    domain.placing = true;
}

fn refuse(
    domain: &mut Domain,
    env: &Env<Limits>,
    channel: Token,
    hello: &Hello,
    why: crate::HelloRefusal,
    out: &mut Queue<Request>,
) {
    crate::history::refused(
        domain,
        crate::HelloRefusalRecord { channel, at: env.wall, slots: hello.slots, stop_bound: hello.stop_bound, why },
    );
    domain.facts.push(Fact::TurnedAway);
    out.push(Request::Refuse { channel });
}

/// Lost: the worker's attempts are kept for the grace, adrift. A channel
/// the fleet does not know never said hello, or was turned away.
pub(crate) fn lost(domain: &mut Domain, env: &Env<Limits>, channel: Token, out: &mut Queue<Request>) {
    if channel == Token::new(0) {
        return;
    }
    let Some(id) = domain.tokens.remove(&channel) else {
        return;
    };
    let entry = domain.channels.get_mut(id).expect("a named channel is in contact");
    crate::sources::lost(&mut entry.sources, channel, out);
    let worker = entry.worker;
    let slots = entry.slots;
    let draining = entry.draining;
    let hosts = mem::replace(&mut entry.hosts, Set::with_capacity(0));
    let until = env.now.saturating_add(env.limits.grace);
    for &attempt in &hosts {
        attempt::adrift(domain, attempt, channel, until);
    }
    domain.channels.retire(id);
    crate::history::lost(domain, crate::history::LostHost { worker, channel, slots, draining, until });
    domain.facts.push(Fact::Lost { adrift: hosts.len() });
}

/// The worker of `channel` refused an assignment as busy: nothing more is
/// placed on it until it frees a slot.
pub(crate) fn drain(domain: &mut Domain, channel: Id<Channel>) {
    domain.channels.get_mut(channel).expect("an answer's channel is in contact").draining = true;
}

/// A host declaring every model of `charter`: one with a free slot that holds
/// the workstream's workspace, or else the one with the most free slots.
pub(crate) fn choose(domain: &Domain, workstream: u64, kinds: Kinds, version: crate::Version) -> Option<Id<Channel>> {
    let mut best: Option<(Id<Channel>, u32)> = None;
    for (_, &id) in &domain.tokens {
        let channel = domain.channels.get(id).expect("a named channel is in contact");
        let free = channel.slots.saturating_sub(channel.hosts.len());
        if !channel.sources.clear()
            || channel.draining
            || free == 0
            || !kinds.allows(channel.kind)
            || !crate::needs::fits(&domain.needs, version, &channel.evidence, channel.kind)
            || !crate::needs::allows(&domain.needs, &channel.eligible, version)
        {
            continue;
        }
        if channel.kind == HostKind::Worker && holds(channel, workstream).is_some() {
            return Some(id);
        }
        best = match best {
            Some((other, most)) if most >= free => Some((other, most)),
            Some(_) | None => Some((id, free)),
        };
    }
    let (id, _) = best?;
    Some(id)
}

/// Candidate hosts in affinity/free-room order, checking only retained run models.
pub(crate) fn probe(
    domain: &Domain,
    workstream: u64,
    kinds: Kinds,
    version: crate::Version,
    models: &[crate::ModelNeed],
    excluded: &[Token],
) -> Option<crate::PlacementConstraint> {
    let mut best: Option<(&Channel, u32)> = None;
    for (_, &id) in &domain.tokens {
        let channel = domain.channels.get(id).expect("connected host");
        let free = channel.slots.saturating_sub(channel.hosts.len());
        if !channel.sources.ready()
            || free == 0
            || channel.draining
            || !kinds.allows(channel.kind)
            || !crate::needs::fits(&domain.needs, version, &channel.evidence, channel.kind)
            || excluded.contains(&channel.token)
            || !crate::needs::allows_models(&domain.needs, &channel.declarations, models)
        {
            continue;
        }
        if channel.kind == HostKind::Worker && holds(channel, workstream).is_some() {
            return Some(crate::PlacementConstraint { channel: channel.token, kind: channel.kind });
        }
        match best {
            Some((_, most)) if most >= free => {}
            Some(_) | None => best = Some((channel, free)),
        }
    }
    let (channel, _) = best?;
    Some(crate::PlacementConstraint { channel: channel.token, kind: channel.kind })
}

pub(crate) fn constrained(domain: &Domain, placement: crate::PlacementConstraint, kinds: Kinds) -> Option<Id<Channel>> {
    let id = *domain.tokens.get(&placement.channel)?;
    let channel = domain.channels.get(id)?;
    if !channel.sources.ready()
        || channel.kind != placement.kind
        || !kinds.allows(channel.kind)
        || channel.draining
        || channel.slots <= channel.hosts.len()
    {
        return None;
    }
    Some(id)
}
