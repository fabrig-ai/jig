//! Copy this entry-point shape. Every child hand-off belongs to the admitted
//! decision; no child receives an event before its whole route fits (domain/root.md, 3–4).
use crate::boundary::Payload;
use crate::{Event, Limits, Output, Record, Released, Store, Write};
use alloc::boxed::Box;
use jig_domain_authority as authority;
use jig_domain_core as core;
use jig_domain_fleet as fleet;
use jig_domain_host as host;
use jig_ops_domain_connectors::Connector;
use jig_ops_domain_infrastructure as infrastructure;
use jig_ops_domain_observability as observability;
use skein_lib::{Decision, Env, Journal, Map, Queue, Token};

/// Prepared output parts retained under the core's assignment continuation.
#[derive(Debug)]
pub(crate) struct Assignment {
    pub(crate) placement: jig_domain_fleet::PlacementConstraint,
    pub(crate) attempt: u64,
    pub(crate) value: host::Assignment,
}

/// A connector read keeps only its caller continuation and opaque envelope.
#[derive(Debug)]
pub(crate) struct ReadCall {
    pub(crate) to: Token,
    pub(crate) name: Box<[u8]>,
    pub(crate) tool: Box<[u8]>,
}

/// This relay's fenced destination, in the engine's names.
#[derive(Clone, Copy, Debug)]
pub(crate) struct HostCall {
    pub(crate) run: Token,
    pub(crate) attempt: Token,
}

/// Configuration is handed to the child that owns each value.
#[derive(Debug)]
pub struct Config {
    /// Core rules and its children.
    pub core: core::Config,
    /// Agent capability supplied by its configuration, checked by the fleet.
    pub agent_bounds: host::Bounds,
    /// Observability owns the standing watches' delegate templates.
    pub triage_templates: Box<[observability::TriageTemplate]>,
    /// Observability owns the matching incident lead templates.
    pub lead_templates: Box<[observability::LeadTemplate]>,
    /// The infrastructure backend's available recovery promise.
    pub backend: infrastructure::Backend,
    /// Host tools supplied by the protocol configuration.
    pub tools: Box<[jig_protocol_charter::Tool]>,
    /// Charter codec endpoint names for the configured endpoint identities.
    pub charter_endpoints: Box<[jig_protocol_charter::EndpointName]>,
}

/// Children, one journal, and opaque child-requested continuation bindings.
/// No root-owned record or policy is stored (domain/root.md, 2).
#[derive(Debug)]
pub struct Domain {
    pub(crate) deployment: [u8; 16],
    pub(crate) core: core::Core,
    pub(crate) observability: observability::Domain,
    pub(crate) infrastructure: infrastructure::Domain,
    pub(crate) host: host::Domain,
    pub(crate) journal: Journal<Write, Output>,
    pub(crate) charter_endpoints: Box<[jig_protocol_charter::EndpointName]>,
    pub(crate) assignments: Map<u64, Assignment>,
    pub(crate) briefs: Map<u64, Box<[jig_protocol_charter::Section]>>,
    pub(crate) tools: Box<[jig_protocol_charter::Tool]>,
    pub(crate) read_calls: Map<Token, ReadCall>,
    pub(crate) payloads: Map<Token, Payload>,
    pub(crate) effects: Map<Token, infrastructure::Effect>,
    pub(crate) judges: Map<Token, (Token, authority::Judge, [u8; 32])>,
    pub(crate) procedures: Map<u64, (u64, u16, u32)>,
    pub(crate) effect_procedures: Map<Token, u64>,
    pub(crate) close_replies: Map<Token, [u8; 16]>,
    pub(crate) host_calls: Map<Box<[u8]>, HostCall>,
    pub(crate) turn_backoffs: skein_lib::Deadlines<(Token, Token, u32)>,
    pub(crate) turn_rng: skein_lib::Rng,
    pub(crate) next_token: u64,
    pub(crate) wrote: bool,
    pub(crate) stopped_reported: bool,
    pub(crate) restarting: bool,
    /// Header supplied by the core in this step; applied after its empty
    /// restore decision is accepted, so journal admission remains paired.
    pub(crate) restored_commits: Option<u64>,
}

impl Domain {
    /// Construct without issuing a request; the caller supplies the first input.
    ///
    /// # Errors
    /// The core's configured workers exceed the priced membership bounds.
    pub fn new(config: Config, limits: &Limits) -> Result<Self, core::Problem> {
        let room = crate::limits::room(limits).expect("finite root route");
        assert!(room.writes <= limits.journal.writes && room.held <= limits.journal.held, "admit the complete route");
        assert!(limits.routes > 0 && limits.journal.now > 0, "nonempty route and door");
        assert!(limits.host.slots == limits.core.fleet.engine_slots, "engine slot agreement");
        assert!(
            config.charter_endpoints.len() <= usize::try_from(limits.endpoints).expect("endpoint count"),
            "bounded codec endpoints"
        );
        for endpoint in &config.charter_endpoints {
            assert!(
                u64::try_from(endpoint.name.len()).expect("endpoint bytes") <= limits.host.charter_bytes,
                "endpoint name bound"
            );
        }
        let mut tool_bytes = 0_u64;
        for tool in &config.tools {
            tool_bytes = tool_bytes
                .checked_add(u64::try_from(::core::mem::size_of::<jig_protocol_charter::Tool>()).expect("tool size"))
                .expect("tool structs")
                .checked_add(
                    u64::try_from(
                        tool.name
                            .len()
                            .checked_add(tool.description.len())
                            .expect("tool description")
                            .checked_add(tool.schema.len())
                            .expect("tool schema"),
                    )
                    .expect("tool bytes"),
                )
                .expect("tool room");
        }
        assert!(tool_bytes <= limits.host.charter_bytes, "configured tools fit the charter byte bound");
        let host_calls = limits.host.slots.checked_mul(limits.host.run_calls).expect("hub relays");
        let deployment = config.core.deployment;
        let turn_rng = skein_lib::Rng::new(config.core.seed ^ 0x5455_524E_4255_5359);
        let mut core_config = config.core;
        core_config.hub_turns = limits.host.turns;
        let mut core = core::Core::new(core_config, &limits.core)?;
        check_engine_hello(&mut core.fleet, &limits.core.fleet, config.agent_bounds, limits.host.slots);
        // Connector hold rules are configuration, so a cold root installs
        // the same translated vocabulary before restoring live task rows.
        let mut configured = Queue::with_capacity(jig_domain_tasks::max_out(&limits.core.tasks));
        jig_domain_tasks::step(
            &mut core.tasks,
            &Env { now: skein_lib::Time::ZERO, wall: skein_lib::Wall::EPOCH, limits: limits.core.tasks },
            jig_domain_tasks::Event::Kinds {
                connector: jig_ops_domain_connectors::number(Connector::Infrastructure),
                kinds: crate::translate::hold_kinds(jig_ops_domain_connectors::number(Connector::Infrastructure)),
            },
            &mut configured,
        );
        assert!(configured.is_empty(), "configured hold rules decide nothing");
        assert!(
            limits.core.tasks.inbox_messages
                <= observability::message_capacity(&limits.observability).expect("watch news bound"),
            "offered watch inbox fits owner message bound"
        );
        Ok(Self {
            deployment,
            core,
            observability: observability::Domain::with_templates(&limits.observability, config.triage_templates)
                .with_leads(&limits.observability, config.lead_templates),
            infrastructure: infrastructure::Domain::new(config.backend, &limits.infrastructure),
            host: host::Domain::new(&limits.host),
            journal: Journal::new(&limits.journal),
            charter_endpoints: config.charter_endpoints,
            assignments: Map::with_capacity(limits.core.tasks.tasks),
            briefs: Map::with_capacity(limits.core.tasks.tasks),
            tools: config.tools,
            read_calls: Map::with_capacity(limits.observability.reads),
            payloads: Map::with_capacity(
                limits.core.fleet.turns.checked_add(limits.core.fleet.attempts).expect("payloads"),
            ),
            effects: Map::with_capacity(
                limits.core.call_records.checked_add(limits.core.tasks.tasks).expect("effect flights"),
            ),
            judges: Map::with_capacity(limits.core.authority.facts),
            procedures: Map::with_capacity(limits.core.tasks.tasks),
            effect_procedures: Map::with_capacity(limits.core.tasks.tasks),
            close_replies: Map::with_capacity(limits.infrastructure.made),
            host_calls: Map::with_capacity(host_calls),
            turn_backoffs: skein_lib::Deadlines::with_capacity(
                limits.host.slots.checked_mul(limits.host.turns).expect("hub retained turns"),
            ),
            turn_rng,
            next_token: 1,
            wrote: false,
            stopped_reported: false,
            restarting: false,
            restored_commits: None,
        })
    }

    /// Decisions open only when the core's cold-start script says so.
    #[must_use]
    pub fn ready(&self) -> bool {
        !self.restarting || self.core.restart_ready()
    }

    /// The iterate adapter may park once the journal and immediate work settle.
    #[must_use]
    pub fn quiescent(&self) -> bool {
        self.journal.idle()
            && (!self.ready() || (!self.host.is_ready() && !self.core.due_ready() && !self.core.finish_owed()))
    }

    /// The protocol timer adapter wakes the root at this hub retry deadline.
    #[must_use]
    pub fn next_turn_deadline(&self) -> Option<skein_lib::Time> {
        self.turn_backoffs.next()
    }

    /// Reclaim child-retired slots after an iteration.
    pub fn reclaim(&mut self) {
        self.core.reclaim();
        self.host.reclaim();
    }

    pub(crate) fn token(&mut self) -> Token {
        let token = Token::new(self.next_token);
        self.next_token = self.next_token.checked_add(1).expect("continuation numbering");
        token
    }
    pub(crate) fn payload(&mut self, payload: Payload) -> Token {
        let token = self.token();
        self.payloads.insert(token, payload).expect("admitted child payload room");
        token
    }
}

/// Synchronous hand-offs, bounded by one application's configured route room.
#[derive(Debug)]
pub(crate) enum Work {
    Event(Event),
    Core(core::Event),
    ResumeFleet,
    Finish,
    Infrastructure(infrastructure::Event),
    Observability(observability::Event),
    Host(host::Event),
    Restart(core::RestartRequest),
    SwitchChecked,
    AdoptClaims,
    AdoptDone,
}

/// Store terminals update the journal; every other input admits one decision.
pub fn step(domain: &mut Domain, env: &Env<Limits>, event: Event) {
    match event {
        Event::Store(Store::Committed { number, fill }) => {
            domain.journal.committed(number);
            let _filled = core::step(
                &mut domain.core,
                &Env { now: env.now, wall: env.wall, limits: env.limits.core },
                core::Event::Fill { bytes: fill },
            );
        }
        Event::Store(Store::Failed { number }) => domain.journal.failed(number),
        other @ (Event::WatchApplication { .. }
        | Event::ReadApplication { .. }
        | Event::Application { .. }
        | Event::ConnectorRead { .. }
        | Event::CallAnswered { .. }
        | Event::Core(_)
        | Event::Infrastructure(_)
        | Event::Observability(_)
        | Event::Host(_)
        | Event::Agent(_)
        | Event::AgentContent { .. }
        | Event::Store(
            Store::Switched | Store::Restore(_) | Store::Restored(_) | Store::Transcript { .. } | Store::Loaded(_),
        )
        | Event::Released(_)
        | Event::Restart { .. }
        | Event::Timer(_)
        | Event::ObservabilityTimer
        | Event::InfrastructureTimer
        | Event::TurnTimer
        | Event::ResultRead { .. }
        | Event::Effect { .. }) => decide(domain, env, Work::Event(other)),
    }
}

fn decide(domain: &mut Domain, env: &Env<Limits>, first: Work) {
    let room = match &first {
        // A read only keeps or consumes a volatile callback. It needs no
        // durable slot, including while a commit is in flight.
        Work::Event(
            Event::ConnectorRead { .. }
            | Event::Core(core::Event::Fleet(fleet::Event::Relay { call: fleet::Call { writes: false, .. }, .. }))
            | Event::Host(host::Event::Called { ask: host::Ask::Relay { writes: false, .. }, .. })
            | Event::Observability(
                observability::Event::Read { .. }
                | observability::Event::System(observability::SystemEvent::ReadDone { .. }),
            ),
        ) => skein_lib::JournalRoom { writes: 0, held: 0 },
        Work::Event(_)
        | Work::Core(_)
        | Work::Finish
        | Work::ResumeFleet
        | Work::Infrastructure(_)
        | Work::Observability(_)
        | Work::Host(_)
        | Work::Restart(_)
        | Work::SwitchChecked
        | Work::AdoptClaims
        | Work::AdoptDone => crate::limits::room(&env.limits).expect("validated route room"),
    };
    let Some(mut decision) = domain.journal.decision(&room) else {
        let _accepted = domain.journal.now(Output::Busy);
        return;
    };
    domain.wrote = false;
    let finish_only = match &first {
        Work::Finish => true,
        Work::Event(_)
        | Work::Core(_)
        | Work::ResumeFleet
        | Work::Infrastructure(_)
        | Work::Observability(_)
        | Work::Host(_)
        | Work::Restart(_)
        | Work::SwitchChecked
        | Work::AdoptClaims
        | Work::AdoptDone => false,
    };
    let mut work = Queue::with_capacity(env.limits.routes);
    work.push(first);
    for _ in 0..env.limits.routes {
        let next = match work.pop() {
            Some(next) => next,
            None => {
                // Coalesce projections only after ordinary hand-offs quiesce.
                let requests = core::finish_decision(
                    &mut domain.core,
                    &Env { now: env.now, wall: env.wall, limits: env.limits.core },
                );
                crate::route::core_out(domain, env, &mut decision, requests, &mut work);
                let Some(next) = work.pop() else { break };
                next
            }
        };
        crate::route::work(domain, env, &mut decision, next, &mut work);
    }
    assert!(work.is_empty(), "complete synchronous route fits its admission");
    if finish_only {
        assert!(!domain.wrote && !domain.core.counters.dirty(), "stopping continuation makes no commit");
    }
    // The cold load must read the durable header before numbering another
    // commit. Copy this guard with the core's restart script.
    if domain.wrote || (domain.core.counters.dirty() && domain.core.restart_step() != Some(core::RestartStep::LoadCore))
    {
        let header = domain.core.counters.next_commit();
        write(
            domain,
            &mut decision,
            Write::Save(Record::Core(core::Record::Core(core::CoreRecord::Deployment(header)))),
        );
    }
    domain.journal.accept(decision);
    if let Some(commits) = domain.restored_commits.take() {
        assert!(domain.journal.idle(), "the deployment header is the first cold-start row");
        domain.journal = Journal::from_durable(&env.limits.journal, commits);
    }
}

/// The adapter supplies the due owner's timer; admission precedes firing it.
/// Copy this dispatch and replace only the application's connector timers.
pub fn fire(domain: &mut Domain, env: &Env<Limits>, timer: crate::Timer) {
    let event = match timer {
        crate::Timer::Core(timer) => Event::Timer(timer),
        crate::Timer::Observability => Event::ObservabilityTimer,
        crate::Timer::Infrastructure => Event::InfrastructureTimer,
        crate::Timer::Turn => Event::TurnTimer,
    };
    step(domain, env, event);
}

/// Iterate drains only the journal: one commit and a bounded ordered release.
pub fn release(domain: &mut Domain, env: &Env<Limits>, out: &mut Queue<Output>) {
    if domain.journal.stopped() || domain.core.restart_failure().is_some() {
        if !domain.stopped_reported {
            domain.stopped_reported = true;
            out.push(Output::Stop);
        }
        return;
    }
    if let Some(commit) = domain.journal.commit() {
        out.push(Output::Commit { number: commit.number, writes: commit.writes });
    }
    let mut ready = Queue::with_capacity(env.limits.journal.release);
    let _released = domain.journal.release(&mut ready);
    for _ in 0..ready.len() {
        out.push(ready.pop().expect("journal release count"));
    }
    if domain.core.stopping() {
        if domain.core.finish_owed() {
            decide(domain, env, Work::Finish);
        }
        return;
    }
    if domain.journal.idle() && domain.ready() {
        if let Some(context) = domain.core.take_due() {
            decide(domain, env, Work::Core(core::Event::Activate { context, ready: true }));
        } else if domain.host.is_ready()
            || domain.core.fleet.is_ready()
            || domain.core.finish_owed()
            || timers_due(domain, env)
        {
            decide(domain, env, Work::ResumeFleet);
        }
    }
}

fn wall_due(env: &Env<Limits>, expiry: skein_lib::Wall) -> bool {
    let remaining = expiry.as_nanos().saturating_sub(env.wall.as_nanos());
    env.now.saturating_add(skein_lib::Duration::from_nanos(remaining)) <= env.now
}

fn timers_due(domain: &Domain, env: &Env<Limits>) -> bool {
    for due in [
        domain.core.people.next_deadline(),
        domain.core.tasks.next_deadline(),
        domain.core.fleet.next_deadline(),
        domain.core.brief.next_deadline(),
        domain.core.accounts.next_deadline(),
        domain.core.views.next_deadline(),
        observability::next_deadline(&domain.observability),
        infrastructure::next_deadline(&domain.infrastructure),
        domain.next_turn_deadline(),
    ] {
        if let Some(due) = due
            && due <= env.now
        {
            return true;
        }
    }
    if let Some(expiry) = domain.core.people.next_wall_expiry()
        && wall_due(env, expiry)
    {
        return true;
    }
    match domain.core.effect_deadline() {
        Some(due) => wall_due(env, due),
        None => false,
    }
}

pub(crate) fn write(domain: &mut Domain, decision: &mut Decision<Write, Output>, write: Write) {
    domain.wrote = true;
    decision.write(write).expect("reserved whole-route writes");
}
pub(crate) fn held(decision: &mut Decision<Write, Output>, value: Output) {
    decision.hold(value).expect("reserved whole-route outputs");
}
pub(crate) fn child(decision: &mut Decision<Write, Output>, value: Released) {
    held(decision, Output::ToChild(value));
}
pub(crate) fn now(domain: &mut Domain, value: Output) {
    domain.journal.now(value).expect("bounded door drained by each domain iteration");
}

fn check_engine_hello(domain: &mut fleet::Domain, limits: &fleet::Limits, agent_bounds: host::Bounds, slots: u32) {
    let mut out = Queue::with_capacity(fleet::max_out(limits));
    fleet::step(
        domain,
        &Env { now: skein_lib::Time::ZERO, wall: skein_lib::Wall::EPOCH, limits: *limits },
        fleet::Event::Hello {
            worker: 0,
            channel: Token::new(0),
            hello: fleet::Hello {
                slots,
                stop_bound: skein_lib::Duration::ZERO,
                workstreams: Box::new([]),
                hosting: Box::new([]),
                bounds: bounds(agent_bounds),
            },
        },
        &mut out,
    );
    assert!(out.is_empty(), "engine agent bounds must hold its configured charters");
}
fn bounds(value: host::Bounds) -> fleet::Bounds {
    let mut models = skein_lib::List::with_capacity(u32::try_from(value.models.len()).expect("configured model count"));
    for model in value.models {
        models
            .push(fleet::ModelBound { name: model.name, window: model.window, output: model.output })
            .expect("configured model count");
    }
    fleet::Bounds {
        program_endpoints: value.program_endpoints,
        models: models.into_boxed(),
        turns: value.turns,
        spend: value.spend,
        time: value.time,
        waiting: value.waiting,
        checks: value.checks,
        message_bytes: value.message_bytes,
        inbox: value.inbox,
        unacknowledged: value.unacknowledged,
    }
}
