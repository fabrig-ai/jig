//! The fleet's state and its entry points (programming-model.md, section 3).

use skein_lib::{Deadlines, Env, Id, List, Map, Queue, Slab, Time, Token};

use crate::attempt::{self, Attempt, Run};
use crate::boundary::{Event, HostKind, Request};
use crate::call::{self, PendingCall};
use crate::channel::{self, Channel};
use crate::facts::{Fact, Facts};
use crate::limits::{self, Limits};
use crate::turn::{self, Pending};
use alloc::boxed::Box;

/// The most requests a step, an alarm or a resume emits under `limits`: a
/// request for each run a hello lists, up to twice the slots (a cancel, an
/// acknowledgement, or the parent told an attempt is found or listed); or an
/// adoption's three (the replaced claim's cancel and its withdrawal, and the
/// adopted attempt's placing or answer). The parent reserves this much room
/// in `out` before calling it. A turn emits at most two (busy or
/// acknowledgement, and drop); adoption and cleanup release one held turn
/// per resume, within the same bound. A lost channel ends both the direct and
/// holder reply for each joined source operation, plus a possible fault refusal.
#[must_use]
pub const fn max_out(limits: &Limits) -> u32 {
    let listed = limits.slots.saturating_mul(limits.grants.saturating_add(2)).saturating_add(1);
    let tracked = limits.attempts.saturating_add(limits.workers.saturating_mul(limits.slots));
    let adopting = limits.grants.saturating_add(3);
    let other = if tracked > adopting { tracked } else { adopting };
    let runs = if listed > other { listed } else { other };
    let sources = limits.program_sources.saturating_mul(2).saturating_add(1);
    let workers = limits.configured_workers.saturating_add(1);
    let sources = if sources > workers { sources } else { workers };
    if runs > sources { runs } else { sources }
}

/// The engine's construction hello is reserved once heard, even if it fails admission.
#[derive(Debug)]
pub(crate) enum EngineHello {
    Expected,
    Heard,
    Admitted,
}

/// The fleet's state.
#[derive(Debug)]
pub struct Domain {
    pub(crate) program_sources: Box<[crate::SourceName]>,
    pub(crate) cleanup_programs: Box<[crate::SourceName]>,
    pub(crate) worker_names: crate::Membership,
    /// The workers in contact, and their channels' names.
    pub(crate) channels: Slab<Channel>,
    pub(crate) needs: crate::Needs,
    pub(crate) catalogue: Box<[Box<[u8]>]>,
    pub(crate) engine_hello: EngineHello,
    /// Latest actual refusals, independent of history and cleared only by same-kind admission.
    pub(crate) standing_engine: Option<crate::HelloRefusalRecord>,
    pub(crate) standing_worker: Option<crate::HelloRefusalRecord>,
    pub(crate) tokens: Map<Token, Id<Channel>>,
    /// The attempts tracked, by their run's and their own names.
    pub(crate) attempts: Slab<Attempt>,
    pub(crate) names: Map<(Token, Token), Id<Attempt>>,
    /// The runs of the attempts tracked.
    pub(crate) runs: Map<Token, Run>,
    /// The attempts waiting for a slot, in the order they started, and the
    /// count that orders them.
    pub(crate) waiting: Map<u64, Id<Attempt>>,
    pub(crate) serial: u64,
    /// An alarm per attempt, as its state says.
    pub(crate) alarms: Deadlines<Id<Attempt>>,
    /// Relayed calls the parent has yet to answer.
    pub(crate) calls: Slab<PendingCall>,
    pub(crate) turns: Map<(Id<Attempt>, u32), Pending>,
    pub(crate) turning: bool,
    /// Something changed that may let a waiting attempt be placed: the ready
    /// list.
    pub(crate) placing: bool,
    /// The parent's attempts tracked, within `Limits::attempts`.
    pub(crate) claims: u32,
    /// The parent has loaded its claims: strays' deadlines run.
    pub(crate) loaded: bool,
    pub(crate) facts: Facts,
    pub(crate) lost_hosts: Map<Token, crate::history::LostHost>,
    pub(crate) hello_refusals: Queue<crate::HelloRefusalRecord>,
    pub(crate) host_history_lost: u64,
    pub(crate) hello_history_lost: u64,
}

impl Domain {
    #[must_use]
    pub fn program_names(&self) -> &[crate::SourceName] {
        &self.program_sources
    }
    #[must_use]
    pub fn program_cleanup(&self, name: crate::SourceName) -> bool {
        self.cleanup_programs.contains(&name)
    }

    /// Actual evidence from an admitted hello; the engine follows the same rule.
    #[must_use]
    pub fn program_endpoint(&self, channel: Token, endpoint: u32) -> Option<bool> {
        if channel == Token::new(0)
            && match self.engine_hello {
                EngineHello::Admitted => false,
                EngineHello::Expected | EngineHello::Heard => true,
            }
        {
            return None;
        }
        let host = self.channels.get(*self.tokens.get(&channel)?)?;
        Some(host.program_endpoints.contains(&endpoint))
    }

    /// Supply the checked fixed membership once, before admitting channels.
    /// Its minter uses the fleet's configured-worker and name-byte limits.
    pub fn configure_workers(&mut self, names: crate::Membership) {
        assert!(self.workers() == 0 && self.worker_names.is_empty(), "membership supplied once before channels");
        self.worker_names = names;
    }
    /// Admit a position only from this fleet's fixed configured membership.
    #[must_use]
    pub fn configured_worker(&self, position: u32) -> Option<crate::Worker> {
        self.worker_names.worker(position)
    }
    /// The name of a position this fleet admitted; membership never changes.
    #[must_use]
    pub fn configured_name(&self, worker: crate::Worker) -> &[u8] {
        self.worker_names.name(worker)
    }
    #[must_use]
    pub fn worker_name(&self, worker: u32) -> Option<&[u8]> {
        self.worker_names.get(worker)
    }

    #[must_use]
    pub fn program_logins(&self, channel: Token) -> Option<&[crate::Login]> {
        Some(self.channels.get(*self.tokens.get(&channel)?)?.sources.logins())
    }
    #[must_use]
    pub fn awaiting_logins(&self, channel: Token) -> bool {
        match self.tokens.get(&channel) {
            Some(id) => self.channels.get(*id).expect("current channel").sources.awaiting(),
            None => true,
        }
    }
    /// A domain with room for `limits`.
    #[must_use]
    pub fn new(limits: &Limits, needs: crate::Needs, catalogue: Box<[Box<[u8]>]>) -> Domain {
        assert!(
            match crate::needs::owned_bytes(&needs) {
                Some(bytes) => bytes <= limits::needs_bytes(limits).expect("needs bound"),
                None => false,
            },
            "configured agent needs fit their byte bound"
        );
        assert!(
            catalogue.len() <= usize::try_from(crate::CATALOGUE_MODELS).expect("catalogue count"),
            "bounded model catalogue"
        );
        for name in &catalogue {
            assert!(
                name.len() <= usize::try_from(crate::CATALOGUE_NAME_BYTES).expect("model name bound"),
                "bounded model names"
            );
        }
        let tracked = limits::tracked(limits).expect("validated fleet limits");
        let channels_count =
            limits.workers.checked_add(u32::from(limits.engine_slots > 0)).expect("validated fleet limits");
        let mut channels = Slab::with_capacity(channels_count);
        let mut tokens = Map::with_capacity(channels_count);
        if limits.engine_slots > 0 {
            let engine = Channel {
                evidence: crate::needs::empty_evidence(),
                program_endpoints: Box::new([]),
                worker: None,
                declarations: Box::new([]),
                sources: crate::sources::Sources::new(Box::new([]), 0, false),
                eligible: crate::needs::eligibility(&needs, &[]),
                kind: HostKind::Engine,
                token: Token::new(0),
                slots: limits.engine_slots,
                draining: false,
                hosts: skein_lib::Set::with_capacity(limits.engine_slots),
                workstreams: Map::with_capacity(0),
                uses: 0,
            };
            let id = channels.insert(engine).expect("engine slot reserved");
            tokens.insert(Token::new(0), id).expect("engine token reserved");
        }
        Domain {
            program_sources: Box::new([]),
            worker_names: crate::Membership::empty(),
            cleanup_programs: Box::new([]),
            needs,
            catalogue,
            engine_hello: EngineHello::Expected,
            standing_engine: None,
            standing_worker: None,
            channels,
            tokens,
            attempts: Slab::with_capacity(tracked),
            names: Map::with_capacity(tracked),
            runs: Map::with_capacity(tracked),
            waiting: Map::with_capacity(tracked),
            serial: 0,
            alarms: Deadlines::with_capacity(tracked),
            calls: Slab::with_capacity(limits.calls),
            turns: Map::with_capacity(limits.turns),
            turning: false,
            placing: false,
            claims: 0,
            loaded: false,
            facts: Facts::with_capacity(limits.facts),
            lost_hosts: Map::with_capacity(limits.history),
            hello_refusals: Queue::with_capacity(limits.history),
            host_history_lost: 0,
            hello_history_lost: 0,
        }
    }

    /// Workers in contact.
    #[must_use]
    pub fn workers(&self) -> u32 {
        self.tokens.len().saturating_sub(u32::from(self.tokens.contains_key(&Token::new(0))))
    }

    /// Attempts tracked, closed ones included until they are reclaimed.
    #[must_use]
    pub fn attempts(&self) -> u32 {
        self.attempts.len()
    }

    /// Attempts waiting for a slot.
    #[must_use]
    pub fn waiting(&self) -> u32 {
        self.waiting.len()
    }

    /// Relayed calls the parent has yet to answer, answered ones included
    /// until they are reclaimed.
    #[must_use]
    pub fn calls(&self) -> u32 {
        self.calls.len()
    }

    /// Turns held for adoption or awaiting the parent's commitment.
    #[must_use]
    pub fn turns(&self) -> u32 {
        self.turns.len()
    }

    /// The kind placement would choose now for this charter; None without an eligible free slot.
    #[must_use]
    pub fn free_kind(&self, kinds: crate::Kinds, workstream: u64, version: crate::Version) -> Option<HostKind> {
        let id = channel::choose(self, workstream, kinds, version)?;
        Some(self.channels.get(id).expect("chosen channel is connected").kind)
    }

    /// Read-only bounded placement over this run's requirements, preserving affinity.
    #[must_use]
    pub fn probe(
        &self,
        kinds: crate::Kinds,
        workstream: u64,
        version: crate::Version,
        models: &[crate::ModelNeed],
        excluded: &[Token],
    ) -> Option<crate::PlacementConstraint> {
        channel::probe(self, workstream, kinds, version, models, excluded)
    }

    /// Latest observed refusal of an allowed kind, while no admitted, connected,
    /// non-draining host of any allowed kind exists (domain/engine.md, section 8).
    /// Slot use and model eligibility do not affect suppression. Engine wins equal
    /// wall times. These two inline facts live only until same-kind admission or restart.
    #[must_use]
    pub fn standing_refusal(&self, kinds: crate::Kinds) -> Option<(HostKind, crate::HelloRefusalRecord)> {
        for (_, id) in &self.tokens {
            let host = self.channels.get(*id).expect("connected host");
            let admitted = match host.kind {
                HostKind::Worker => true,
                HostKind::Engine => match self.engine_hello {
                    EngineHello::Admitted => true,
                    EngineHello::Expected | EngineHello::Heard => false,
                },
            };
            if admitted && kinds.allows(host.kind) && !host.draining {
                return None;
            }
        }
        let mut latest = None;
        if kinds.allows(HostKind::Engine)
            && let Some(record) = self.standing_engine
        {
            latest = Some((HostKind::Engine, record));
        }
        if kinds.allows(HostKind::Worker)
            && let Some(record) = self.standing_worker
        {
            let newer = match latest {
                Some((_, previous)) => record.at > previous.at,
                None => true,
            };
            if newer {
                latest = Some((HostKind::Worker, record));
            }
        }
        latest
    }

    #[must_use]
    pub fn host_capacity(&self) -> u32 {
        self.tokens.capacity()
    }
    #[must_use]
    pub fn supports(&self, placement: crate::PlacementConstraint, models: &[crate::ModelNeed]) -> bool {
        let Some(id) = self.tokens.get(&placement.channel) else { return false };
        let channel = self.channels.get(*id).expect("connected host");
        channel.kind == placement.kind && crate::needs::allows_models(&self.needs, &channel.declarations, models)
    }
    #[must_use]
    pub fn placement_available(&self, placement: crate::PlacementConstraint) -> bool {
        channel::constrained(self, placement, crate::Kinds::Both).is_some()
    }
    /// Last sign-out results on this current channel, independent of login checks.
    #[must_use]
    pub fn program_sign_outs(&self, channel: Token) -> Option<&Map<crate::SourceName, crate::SignOutResult>> {
        let id = *self.tokens.get(&channel)?;
        Some(&self.channels.get(id)?.sources.results)
    }
    /// Copy bounded connected and retained lost host facts without changing placement.
    #[must_use]
    pub fn host_views(&self) -> List<crate::HostView> {
        let mut result = List::with_capacity(
            self.tokens.capacity().checked_add(self.lost_hosts.capacity()).expect("accepted history limits"),
        );
        for (_, id) in &self.tokens {
            let value = self.channels.get(*id).expect("connected channel");
            result
                .push(crate::HostView {
                    channel: value.token,
                    worker: value.worker,
                    kind: value.kind,
                    slots: value.slots,
                    used: value.hosts.len(),
                    draining: value.draining,
                    connected: true,
                    grace_until: None,
                })
                .expect("one view per connected channel");
        }
        for (_, host) in &self.lost_hosts {
            if self.tokens.contains_key(&host.channel) {
                continue;
            }
            let mut used = 0_u32;
            for (_, id) in &self.names {
                let value = self.attempts.get(*id).expect("named attempt");
                if attempt::lost_host(value) == Some(host.channel) {
                    used = used.checked_add(1).expect("bounded lost host slots");
                }
            }
            result
                .push(crate::HostView {
                    channel: host.channel,
                    worker: host.worker,
                    kind: HostKind::Worker,
                    slots: host.slots,
                    used,
                    draining: host.draining,
                    connected: false,
                    grace_until: Some(host.until),
                })
                .expect("bounded connected and lost hosts");
        }
        result
    }
    /// Recent refused hellos, newest first, independent of diagnostic drains.
    #[must_use]
    pub fn hello_refusals(&self) -> List<crate::HelloRefusalRecord> {
        let mut result = List::with_capacity(self.hello_refusals.capacity());
        for record in self.hello_refusals.iter().rev() {
            result.push(*record).expect("bounded hello history");
        }
        result
    }
    /// Lost channel records evicted from the bounded history.
    #[must_use]
    pub const fn host_history_lost(&self) -> u64 {
        self.host_history_lost
    }
    /// Refused hellos evicted from the bounded history.
    #[must_use]
    pub const fn hello_history_lost(&self) -> u64 {
        self.hello_history_lost
    }
    /// Last best-effort observation of this current claim and channel, borrowed
    /// without cloning its bounded names. None after loss, replacement or answer.
    #[must_use]
    pub fn observation(&self, run: Token, attempt: Token) -> Option<&crate::Observation> {
        crate::progress::observed(self, run, attempt)
    }

    /// Copy bounded current attempt facts, without exposing outstanding replies.
    #[must_use]
    pub fn attempt_views(&self) -> List<crate::AttemptView> {
        let mut result = List::with_capacity(self.names.capacity());
        for (_, id) in &self.names {
            let value = self.attempts.get(*id).expect("named attempt");
            let (state, at, until) = match value.state {
                attempt::State::Waiting { .. } => (crate::AttemptState::Waiting, None, None),
                attempt::State::Adopted { until, .. } => (crate::AttemptState::Adopted, None, Some(until)),
                attempt::State::Claimed { at, .. } => (crate::AttemptState::Claimed, Some(at), None),
                attempt::State::Cancelled { at, .. } => (crate::AttemptState::Cancelled, Some(at), None),
                attempt::State::Handed { at } => (crate::AttemptState::Handed, Some(at), None),
                attempt::State::Acknowledged { until } => {
                    (crate::AttemptState::Acknowledged, Some(attempt::Where::Adrift { until }), Some(until))
                }
                attempt::State::Stray { at, until } => (crate::AttemptState::Stray, Some(at), until),
                attempt::State::Kept { at, until, .. } => (crate::AttemptState::Kept, Some(at), until),
                attempt::State::Crossed { at, .. } | attempt::State::Fenced { at } => {
                    (crate::AttemptState::Fenced, Some(at), None)
                }
                attempt::State::Closed => (crate::AttemptState::Closed, None, None),
            };
            let (placement, deadline) = match at {
                Some(attempt::Where::On(channel)) => {
                    let channel = self.channels.get(channel).expect("attempt host");
                    (crate::Placement::Connected { channel: channel.token, kind: channel.kind }, until)
                }
                Some(attempt::Where::Adrift { until }) => {
                    (crate::Placement::Adrift { channel: value.lost_host, until }, Some(until))
                }
                None => (crate::Placement::None, until),
            };
            result
                .push(crate::AttemptView {
                    run: value.run,
                    attempt: value.token,
                    kept: value.kept,
                    listed: value.listed,
                    state,
                    placement,
                    until: deadline,
                })
                .expect("one view per named attempt");
        }
        result
    }

    #[must_use]
    pub fn next_deadline(&self) -> Option<Time> {
        self.alarms.next()
    }

    /// Whether an alarm is due at `now`. While one is, the loop fires the
    /// root domain, which calls [`fire`].
    #[must_use]
    pub fn is_due(&self, now: Time) -> bool {
        match self.alarms.next() {
            Some(at) => at <= now,
            None => false,
        }
    }

    /// Whether placement or a held turn may make progress. While it may,
    /// the loop resumes the root domain, which calls [`resume`], at the
    /// start of the domain's stage, before its input events.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.placing || self.turning || crate::programs::ready(self)
    }

    /// The oldest fact not yet drained. The parent drains them at its own
    /// pace; what does not fit meanwhile is dropped and counted.
    pub fn pop_fact(&mut self) -> Option<Fact> {
        self.facts.pop()
    }

    /// How many facts were dropped for want of room, since the domain was
    /// made.
    #[must_use]
    pub fn facts_lost(&self) -> u64 {
        self.facts.lost()
    }

    /// The reclaim point: frees what closed in this iteration.
    pub fn reclaim(&mut self) {
        self.channels.reclaim();
        self.attempts.reclaim();
        self.calls.reclaim();
    }
}

/// Handles one event, emitting at most [`max_out`] requests.
pub fn step(domain: &mut Domain, env: &Env<Limits>, event: Event, out: &mut Queue<Request>) {
    match event {
        Event::Needs(needs) => crate::needs::replace(domain, &env.limits, needs),
        Event::CleanupPrograms { names } => {
            domain.cleanup_programs = names;
            domain.placing = true;
        }
        Event::CheckProgram { name } => crate::programs::check(domain, name, out),
        Event::SignOutProgram { source, name } => crate::programs::sign_out(domain, source, name, out),
        Event::Sources { sources } => crate::sources::replace(domain, env, sources, out),
        Event::Check { channel, name } => crate::sources::check(domain, channel, name, out),
        Event::SignOut { to, channel, name } => crate::sources::sign_out(domain, to, channel, name, out),
        Event::Logins { channel, logins } => crate::sources::logins(domain, env, channel, logins, out),
        Event::SignedOut { channel, name, result } => {
            crate::sources::signed_out(domain, env, channel, name, result, out);
        }
        Event::Start { reply_to, run, attempt, workstream, kinds, version, placement, assignment, grants } => {
            if attempt::start(
                domain, env, reply_to, run, attempt, workstream, kinds, version, placement, assignment, out,
            ) {
                crate::grants::keep(domain, env, run, attempt, &[], &grants, out);
            }
        }
        Event::Inbound { run, attempt, message } => call::inbound(domain, run, attempt, message, out),
        Event::Relay { channel, run, attempt, call } => {
            call::relay(domain, env, channel, run, attempt, call, out);
        }
        Event::Adopt { reply_to, run, attempt, kept, kind, worked, holds, grants } => match kind {
            HostKind::Engine => {
                if worked {
                    out.push(Request::Lost { to: reply_to, run, attempt });
                } else {
                    out.push(Request::NotStarted { to: reply_to, run, attempt });
                }
            }
            HostKind::Worker => {
                if attempt::adopt(domain, env, reply_to, run, attempt, kept, out) {
                    crate::grants::keep(domain, env, run, attempt, &holds, &grants, out);
                }
            }
        },
        Event::Cross { reply_to, run, attempt, worked } => {
            attempt::cross(domain, env, reply_to, run, attempt, worked, out);
        }
        Event::Cancel { run, attempt } => attempt::cancel(domain, run, attempt, out),
        Event::Relayed { to, answer } => call::relayed(domain, to, answer, out),
        Event::Acknowledge { run, attempt } => attempt::acknowledge(domain, run, attempt, out),
        Event::Grant { grant } => crate::grants::renew(domain, env, grant, out),
        Event::Unauthorized { channel, run, attempt, account, generation } => {
            call::credential_notice(
                domain,
                channel,
                run,
                attempt,
                Request::Unauthorized { run, attempt, account, generation },
                out,
            );
        }
        Event::Limited { channel, run, attempt, account, reset } => call::credential_notice(
            domain,
            channel,
            run,
            attempt,
            Request::Limited { run, attempt, account, reset },
            out,
        ),
        Event::AccountRefused { channel, run, attempt, account, code, reason } => call::credential_notice(
            domain,
            channel,
            run,
            attempt,
            Request::AccountRefused { run, attempt, account, code, reason },
            out,
        ),
        Event::Loaded => attempt::loaded(domain, env),
        Event::Hello { channel, worker, hello } => channel::hello(domain, env, channel, worker, hello, out),
        Event::Lost { channel } => channel::lost(domain, env, channel, out),
        Event::Answer { channel, run, attempt, answer, payload } => {
            attempt::answer(domain, channel, run, attempt, answer, payload, out);
        }
        Event::Turn { channel, run, attempt, turn, body } => {
            turn::received(domain, channel, run, attempt, turn, body, out);
        }
        Event::TurnKept { run, attempt, turn } => turn::kept(domain, run, attempt, turn, out),
        Event::TurnBusy { run, attempt, turn } => turn::busy(domain, run, attempt, turn, out),

        Event::Observe { channel, run, attempt, report } => {
            crate::progress::received(domain, env, channel, run, attempt, report, out);
        }
    }
}

/// Fires the earliest alarm due at `env.now`, if there is one, emitting at
/// most [`max_out`] requests. A stage fires its alarms after its input
/// events, so a hello that arrived in the same iteration wins over a grace
/// that passed while the loop waited.
pub fn fire(domain: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    if let Some(id) = domain.alarms.expire(env.now) {
        attempt::expire(domain, id, out);
    }
}

/// Releases one held turn or places a waiting attempt, emitting at most
/// [`max_out`] requests; with none, waits until something changes.
pub fn resume(domain: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    if crate::programs::ready(domain) {
        crate::programs::resume(domain, out);
    } else if domain.turning {
        turn::resume(domain, out);
    } else if domain.placing {
        attempt::resume(domain, env, out);
    }
}
