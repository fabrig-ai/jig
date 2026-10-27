//! Memory stays within the worst case (programming-model.md, 6.3), measured by
//! a counting allocator: every worker in contact holding as many workstream
//! keys as it may, every attempt tracked, every relayed call kept, and every
//! entry point on the way.

use jig_domain_fleet::{
    Answer, Domain, Event, Hello, Hosted, Limits, Phase, Request, fire, max_out, resume, step, worst_case,
};
use skein_lib::{Duration, Env, Queue, ReplyTo, Time, Token, Wall};
use skein_world::domain::heap::{self, Meter};

#[global_allocator]
static HEAP: heap::Counting = heap::Counting;

const LIMITS: Limits = Limits {
    configured_workers: 64,
    worker_name_bytes: 32,
    program_sources: 12,
    source_detail_bytes: 4096,
    needs_bytes: 0,
    versions: 0,
    workers: 3,
    engine_slots: 0,
    slots: 3,
    workstreams: 3,
    attempts: 12,
    grants: 4,
    calls: 4,
    call_name_bytes: 64,
    progress_bytes: 64,
    turns: 0,
    grace: Duration::from_secs(10),
    facts: 16,
    history: 4,
};

/// The fleet under `limits`, measured: each step's peak is checked against
/// the worst case, less what it handed out in requests, which their receivers
/// count.
struct Measured {
    domain: Domain,
    env: Env<Limits>,
    out: Queue<Request>,
    meter: Meter,
    bound: u64,
    names: u64,
}

impl Measured {
    fn new(limits: Limits) -> Measured {
        Self::with_needs(limits, jig_fleet_world::fixtures::needs)
    }

    fn with_needs(limits: Limits, needs: impl FnOnce() -> jig_domain_fleet::Needs) -> Measured {
        let bound = worst_case(&limits).expect("the test limits fit");
        let meter = Meter::new();
        let domain = jig_fleet_world::fixtures::domain(&limits, needs());
        let out = Queue::with_capacity(max_out(&limits));
        Measured { domain, env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits }, out, meter, bound, names: 0 }
    }

    fn name(&mut self) -> Token {
        self.names += 1;
        Token::new(self.names)
    }

    fn step(&mut self, event: Event) -> Vec<Request> {
        self.meter.start();
        step(&mut self.domain, &self.env, event, &mut self.out);
        self.drain()
    }

    /// Places what can be placed, one resume at a time.
    fn settle(&mut self) -> Vec<Request> {
        let mut requests = Vec::new();
        while self.domain.is_ready() {
            self.meter.start();
            resume(&mut self.domain, &self.env, &mut self.out);
            requests.extend(self.drain());
        }
        requests
    }

    /// Moves the clock to `secs` and fires every alarm due.
    fn at(&mut self, secs: u64) -> Vec<Request> {
        self.env.now = Time::ZERO.saturating_add(Duration::from_secs(secs));
        let mut requests = Vec::new();
        while self.domain.is_due(self.env.now) {
            self.meter.start();
            fire(&mut self.domain, &self.env, &mut self.out);
            requests.extend(self.drain());
        }
        requests
    }

    fn drain(&mut self) -> Vec<Request> {
        let measured = self.meter.end();
        let mut requests = Vec::with_capacity(self.out.len() as usize);
        while let Some(request) = self.out.pop() {
            if let Request::Sources { channel, sources } = request {
                assert!(sources.is_empty());
                step(&mut self.domain, &self.env, Event::Logins { channel, logins: Box::new([]) }, &mut self.out);
            } else {
                requests.push(request);
            }
        }
        self.meter.check(measured, self.bound, &self.env.limits);
        // The iteration ends: the reclaim point.
        self.domain.reclaim();
        while self.domain.pop_fact().is_some() {}
        requests
    }

    /// A hello on `channel`, holding as many keys as a worker may, of the
    /// most bytes, and hosting `hosting`.
    fn hello(&mut self, channel: Token, hosting: Vec<Hosted>) -> Vec<Request> {
        self.hello_with_endpoints(channel, hosting, jig_domain_fleet::PROGRAM_ENDPOINTS)
    }

    fn hello_with_endpoints(&mut self, channel: Token, hosting: Vec<Hosted>, endpoints: u32) -> Vec<Request> {
        let limits = self.env.limits;
        let workstreams = (0..limits.workstreams).map(|nth| channel.raw() * 100 + u64::from(nth)).collect();
        let mut bounds = jig_fleet_world::fixtures::bounds();
        bounds.program_endpoints = (0..endpoints).collect();
        self.step(Event::Hello {
            worker: u32::try_from(channel.raw()).expect("fixture channel") % LIMITS.configured_workers,
            channel,
            hello: Hello {
                bounds,
                stop_bound: Duration::from_secs(0),
                slots: limits.slots,
                workstreams,
                hosting: hosting.into(),
            },
        })
    }

    /// Starts an attempt of a run of its own, with a key of the most bytes.
    fn start(&mut self) -> (Token, Token) {
        let (run, attempt) = (self.name(), self.name());
        let workstream = run.raw();
        let asked = self.step(Event::Start {
            placement: None,
            version: jig_domain_fleet::Version { charter: 1, revision: 1 },
            grants: Box::new([]),
            reply_to: ReplyTo::new(attempt),
            run,
            attempt,
            workstream,
            kinds: jig_domain_fleet::Kinds::Workers,
            assignment: jig_domain_fleet::Assignment { turns: Token::new(0), answered: Token::new(0) },
        });
        assert!(asked.is_empty(), "a start waits for placement: {asked:?}");
        (run, attempt)
    }
}

#[test]
fn program_endpoints_retain_exactly_the_formulas_bytes() {
    let limits = Limits { engine_slots: 1, ..LIMITS };
    let channels = limits.workers + u32::from(limits.engine_slots > 0);
    let retained = |endpoints| {
        let mut fleet = Measured::new(limits);
        assert!(fleet.hello_with_endpoints(Token::new(0), Vec::new(), endpoints).is_empty());
        for nth in 0..limits.workers {
            assert!(fleet.hello_with_endpoints(channel(nth), Vec::new(), endpoints).is_empty());
        }
        assert_eq!(fleet.domain.workers(), limits.workers);
        assert_eq!(fleet.domain.attempts(), 0);
        fleet.meter.held()
    };
    let with_0 = retained(0);
    let with_64 = retained(jig_domain_fleet::PROGRAM_ENDPOINTS);
    let endpoints = u64::from(channels) * u64::from(jig_domain_fleet::PROGRAM_ENDPOINTS) * 4;
    assert_eq!(with_64 - with_0, endpoints);
}

#[test]
fn model_eligibility_on_every_host_stays_within_the_memory_bound() {
    // Fill the needs budget with charter headers: this maximizes eligibility
    // bytes per needs byte, independently of model name lengths.
    const CHARTERS: usize = 32;
    let needs_bytes = u64::try_from(CHARTERS * size_of::<jig_domain_fleet::CharterNeeds>()).expect("small fixture");
    let limits = Limits { needs_bytes, engine_slots: 1, ..LIMITS };
    let mut fleet = Measured::with_needs(limits, || jig_domain_fleet::Needs {
        charters: (0..CHARTERS)
            .map(|number| jig_domain_fleet::CharterNeeds {
                version: jig_domain_fleet::Version {
                    charter: u32::try_from(number).expect("small fixture"),
                    revision: 1,
                },
                kinds: jig_domain_fleet::Kinds::Both,
                models: Box::new([]),
                turns: 0,
                time: Duration::ZERO,
                waiting: Duration::ZERO,
                change_reserve: None,
            })
            .collect(),
        ..jig_fleet_world::fixtures::needs()
    });
    fleet.hello(Token::new(0), Vec::new());
    for nth in 0..limits.workers {
        fleet.hello(channel(nth), Vec::new());
    }
    for _ in 0..limits.attempts {
        fleet.start();
    }
    fleet.settle();
    assert_eq!(fleet.domain.workers(), limits.workers);
}

/// The channels of the workers, `C1` and on.
fn channel(nth: u32) -> Token {
    Token::new(1_000_000 + u64::from(nth))
}

/// The attempts placed, with their workers' channels.
fn placed(requests: &[Request]) -> Vec<(Token, Token, Token)> {
    let mut placed = Vec::new();
    for request in requests {
        if let Request::Assign { channel, run, attempt, .. } = request {
            placed.push((*channel, *run, *attempt));
        }
    }
    placed
}

/// Fills the fleet: every worker in contact with its keys, every attempt
/// tracked, as many placed as there are slots, and every relayed call kept.
fn fill(limits: Limits) -> (Measured, Vec<(Token, Token, Token)>, Vec<ReplyTo>) {
    let mut fleet = Measured::new(limits);
    for nth in 0..limits.workers {
        assert!(fleet.hello(channel(nth), Vec::new()).is_empty());
    }
    for _ in 0..limits.attempts {
        fleet.start();
    }
    let placed = placed(&fleet.settle());
    assert_eq!(u32::try_from(placed.len()).expect("a few"), limits.workers * limits.slots, "every slot taken");
    for &(channel, run, attempt) in &placed {
        fleet.step(Event::Observe {
            channel,
            run,
            attempt,
            report: jig_domain_fleet::Report::Activity(jig_domain_fleet::Activity::Tool {
                call: vec![1; usize::try_from(limits.call_name_bytes).expect("fits")].into_boxed_slice(),
                tool: vec![2; usize::try_from(limits.progress_bytes).expect("fits")].into_boxed_slice(),
            }),
        });
    }
    let mut calls = Vec::new();
    for (nth, &(channel, run, attempt)) in placed.iter().enumerate().take(limits.calls as usize) {
        let call = Token::new(nth as u64);
        let up = fleet.step(Event::Relay {
            channel,
            run,
            attempt,
            call: jig_domain_fleet::Call {
                name: vec![7; usize::try_from(limits.call_name_bytes).expect("fits")].into_boxed_slice(),
                tool: Box::new([]),
                writes: false,
                input: Box::from(call.raw().to_be_bytes()),
                deadline: Duration::ZERO,
            },
        });
        let Some(Request::Relay { reply_to, .. }) = up.into_iter().next() else {
            panic!("a call relayed up");
        };
        calls.push(reply_to);
    }
    assert_eq!(fleet.domain.workers(), limits.workers);
    assert_eq!(fleet.domain.attempts(), limits.attempts);
    assert_eq!(fleet.domain.calls(), limits.calls);
    (fleet, placed, calls)
}

#[test]
fn a_fleet_full_to_its_limits_stays_within_its_worst_case() {
    // This fixture fills runs; the source fixture below fills inventories.
    let (mut fleet, placed, calls) = fill(Limits { program_sources: 0, source_detail_bytes: 0, ..LIMITS });
    // At its fullest, the fleet holds a fair share of the bound: it is not
    // padded beyond use.
    let fullest = fleet.meter.held();
    // This run fixture declares no catalogue; its fixed catalogue allowance is
    // filled by deployments, independently of the run slots this story fills.
    let catalogue = u64::from(jig_domain_fleet::CATALOGUE_MODELS)
        * (u64::from(jig_domain_fleet::CATALOGUE_NAME_BYTES)
            + u64::try_from(size_of::<Box<[u8]>>()).expect("box header size"));
    let evidence = u64::from(LIMITS.workers)
        * u64::from(jig_domain_fleet::CATALOGUE_MODELS)
        * (u64::from(jig_domain_fleet::CATALOGUE_NAME_BYTES)
            + u64::try_from(size_of::<jig_domain_fleet::ModelBound>()).expect("model bound header size"));
    let run_bound = fleet.bound - catalogue - evidence;
    assert!(fullest.saturating_mul(3) > run_bound, "{fullest} held of a run bound of {run_bound}");
    // One more is refused, at the entrance.
    let (run, attempt) = (fleet.name(), fleet.name());
    let workstream = run.raw();
    let refused = fleet.step(Event::Start {
        placement: None,
        version: jig_domain_fleet::Version { charter: 1, revision: 1 },
        grants: Box::new([]),
        reply_to: ReplyTo::new(attempt),
        run,
        attempt,
        workstream,
        kinds: jig_domain_fleet::Kinds::Workers,
        assignment: jig_domain_fleet::Assignment { turns: Token::new(0), answered: Token::new(0) },
    });
    assert!(matches!(refused[..], [Request::Refused { .. }]), "{refused:?}");
    let up = fleet.step(Event::Relay {
        channel: placed[5].0,
        run: placed[5].1,
        attempt: placed[5].2,
        call: jig_domain_fleet::Call {
            name: Box::from(Token::new(9).raw().to_be_bytes()),
            tool: Box::new([]),
            writes: false,
            input: Box::from(Token::new(9).raw().to_be_bytes()),
            deadline: Duration::ZERO,
        },
    });
    assert!(matches!(up[..], [Request::RelayBusy { .. }]), "a call beyond the room is busy: {up:?}");
    // Drained: every call answered, every attempt answered or lost.
    for (nth, to) in calls.into_iter().enumerate() {
        let down = fleet.step(Event::Relayed { to, answer: Token::new(100 + nth as u64) });
        assert!(matches!(down[..], [Request::Relayed { .. }]), "{down:?}");
    }
    // Each attempt answered, and its answer made durable, frees a slot for
    // one waiting.
    let mut out = placed;
    while !out.is_empty() {
        for (channel, run, attempt) in out {
            let payload = Token::new(200 + attempt.raw());
            fleet.step(Event::Answer { channel, run, attempt, answer: Answer::Ended, payload });
            fleet.step(Event::Acknowledge { run, attempt });
        }
        out = self::placed(&fleet.settle());
    }
    assert_eq!((fleet.domain.attempts(), fleet.domain.calls()), (0, 0));
}

#[test]
fn every_entry_point_stays_within_the_worst_case() {
    let mut fleet = Measured::new(LIMITS);
    // Hellos: a worker with strays and answers it holds, before the parent
    // has loaded its claims.
    let (r1, a1, r2, a2) = (fleet.name(), fleet.name(), fleet.name(), fleet.name());
    let listed = fleet.hello(channel(0), jig_fleet_world::fixtures::hosting_pair(r1, a1, r2, a2));
    assert!(matches!(listed[..], [Request::Listed { .. }, Request::Listed { .. }]), "{listed:?}");
    let answer =
        Event::Answer { channel: channel(0), run: r2, attempt: a2, answer: Answer::Ended, payload: Token::new(1) };
    fleet.step(answer);
    fleet.step(Event::Loaded);
    fleet.step(Event::Adopt {
        holds: Box::new([]),
        grants: Box::new([]),
        reply_to: ReplyTo::new(a2),
        run: r2,
        attempt: a2,
        kept: 0,
        kind: jig_domain_fleet::HostKind::Worker,
        worked: false,
    });
    fleet.step(Event::Acknowledge { run: r2, attempt: a2 });
    fleet.step(Event::Adopt {
        holds: Box::new([]),
        grants: Box::new([]),
        reply_to: ReplyTo::new(a1),
        run: r1,
        attempt: a1,
        kept: 0,
        kind: jig_domain_fleet::HostKind::Worker,
        worked: false,
    });
    let (r3, a3) = fleet.start();
    let placed = placed(&fleet.settle());
    assert_eq!(placed.len(), 1);
    // Calls, answers, inbound events and facts.
    let up = fleet.step(Event::Relay {
        channel: placed[0].0,
        run: r3,
        attempt: a3,
        call: jig_domain_fleet::Call {
            name: Box::from(Token::new(1).raw().to_be_bytes()),
            tool: Box::new([]),
            writes: false,
            input: Box::from(Token::new(2).raw().to_be_bytes()),
            deadline: Duration::ZERO,
        },
    });
    let Some(Request::Relay { reply_to, .. }) = up.into_iter().next() else {
        panic!("a call relayed up");
    };
    fleet.step(Event::Relayed { to: reply_to, answer: Token::new(3) });
    fleet.step(Event::Inbound {
        run: r3,
        attempt: a3,
        message: jig_domain_fleet::Message { name: Token::new(4), sender: Token::new(4), words: Token::new(4) },
    });

    fleet.step(Event::Observe {
        channel: placed[0].0,
        run: r3,
        attempt: a3,
        report: jig_domain_fleet::Report::Activity(jig_domain_fleet::Activity::Active),
    });
    fleet.step(Event::Cancel { run: r1, attempt: a1 });
    // A busy refusal, placed again.
    let (channel_of, ..) = placed[0];
    let busy =
        Event::Answer { channel: channel_of, run: r3, attempt: a3, answer: Answer::Busy, payload: Token::new(6) };
    fleet.step(busy);
    fleet.settle();
    // A worker lost, back on another channel, and lost for good.
    fleet.step(Event::Lost { channel: channel(0) });
    let hosting = vec![Hosted { run: r3, attempt: a3, phase: Phase::Active }];
    fleet.hello(channel(1), hosting);
    for raw in [7, 8] {
        let answer = Event::Answer {
            channel: channel(1),
            run: r3,
            attempt: a3,
            answer: Answer::Failed { turned: None },
            payload: Token::new(raw),
        };
        fleet.step(answer);
    }
    fleet.step(Event::Acknowledge { run: r3, attempt: a3 });
    fleet.at(30);
    fleet.settle();
    assert_eq!((fleet.domain.attempts(), fleet.domain.calls()), (0, 0));
    fleet.hello(Token::new(0), Vec::new());
    for number in 10..=10 + LIMITS.history {
        fleet.hello(channel(number), Vec::new());
        fleet.step(Event::Lost { channel: channel(number) });
        fleet.hello(Token::new(0), Vec::new());
    }
    assert!(fleet.domain.host_history_lost() > 0);
    assert!(fleet.domain.hello_history_lost() > 0);
    fleet.meter.start();
    let hosts = fleet.domain.host_views();
    let attempts = fleet.domain.attempt_views();
    let refusals = fleet.domain.hello_refusals();
    assert!(!hosts.is_empty());
    assert!(attempts.is_empty());
    assert_eq!(refusals.len(), LIMITS.history);
    let measured = fleet.meter.end();
    fleet.meter.check(measured, fleet.bound, &fleet.env.limits);
}

#[test]
fn held_and_handed_turn_admissions_stay_within_the_worst_case() {
    let limits = Limits { turns: 8, ..LIMITS };
    let mut fleet = Measured::new(limits);
    let (run, attempt) = (fleet.name(), fleet.name());
    fleet.hello(channel(0), vec![Hosted { run, attempt, phase: Phase::Active }]);
    for turn in 1..=limits.turns {
        let out =
            fleet.step(Event::Turn { channel: channel(0), run, attempt, turn, body: Token::new(u64::from(turn)) });
        assert!(out.is_empty(), "a stray holds its turn for adoption");
    }
    let refused = fleet.step(Event::Turn { channel: channel(0), run, attempt, turn: 9, body: Token::new(9) });
    assert!(matches!(refused[..], [Request::TurnBusy { .. }, Request::Drop { .. }]));
    fleet.step(Event::Adopt {
        holds: Box::new([]),
        grants: Box::new([]),
        reply_to: ReplyTo::new(attempt),
        run,
        attempt,
        kept: 3,
        kind: jig_domain_fleet::HostKind::Worker,
        worked: false,
    });
    let released = fleet.settle();
    assert_eq!(released.iter().filter(|request| matches!(request, Request::Turned { .. })).count(), 5);
    for turn in 4..=limits.turns {
        fleet.step(Event::TurnKept { run, attempt, turn });
    }
    // Parent pressure frees the remaining admission for retry as well.
    fleet.step(Event::Turn { channel: channel(0), run, attempt, turn: 9, body: Token::new(90) });
    fleet.step(Event::TurnBusy { run, attempt, turn: 9 });
    fleet.step(Event::Turn { channel: channel(0), run, attempt, turn: 9, body: Token::new(91) });
    fleet.step(Event::TurnKept { run, attempt, turn: 9 });
    fleet.step(Event::Answer { channel: channel(0), run, attempt, answer: Answer::Ended, payload: Token::new(99) });
    fleet.step(Event::Acknowledge { run, attempt });
    fleet.settle();
    assert_eq!(fleet.domain.attempts(), 0);
}

#[test]
fn engine_slots_stay_within_the_fleets_worst_case() {
    let limits = Limits { workers: 0, engine_slots: 2, ..LIMITS };
    let mut fleet = Measured::new(limits);
    for _ in 0..2 {
        fleet.start();
        fleet.settle();
    }
}

#[test]
fn source_lifecycle_at_capacity_fits_the_memory_bound() {
    use jig_domain_fleet::{Login, LoginState, SourceName};
    let limits = LIMITS;
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits };
    let mut out = Queue::with_capacity(max_out(&limits));
    let meter = Meter::new();
    let mut domain = jig_fleet_world::fixtures::domain(&limits, jig_fleet_world::fixtures::needs());
    let mut measured_step = |event| {
        meter.start();
        step(&mut domain, &env, event, &mut out);
        let measured = meter.end();
        while out.pop().is_some() {}
        meter.check(measured, worst_case(&limits).expect("source inventory bound"), &limits);
    };
    let names: Box<[_]> = (0..limits.program_sources)
        .map(|index| SourceName::new(format!("{index:032}").as_bytes()).expect("full source name"))
        .collect();
    measured_step(Event::Sources { sources: names.clone() });
    for worker in 1..=limits.workers {
        let channel = Token::new(u64::from(worker));
        measured_step(Event::Hello {
            worker: u32::try_from(channel.raw()).expect("fixture channel") % LIMITS.configured_workers,
            channel,
            hello: Hello {
                slots: limits.slots,
                stop_bound: Duration::ZERO,
                workstreams: Box::new([]),
                hosting: Box::new([]),
                bounds: jig_fleet_world::fixtures::bounds(),
            },
        });
        measured_step(Event::Logins {
            channel,
            logins: names
                .iter()
                .map(|&name| Login {
                    name,
                    state: LoginState::Failed {
                        reason: jig_domain_fleet::SourceFailure::Exited,
                        detail: vec![b'x'; usize::try_from(limits.source_detail_bytes).expect("detail bound")]
                            .into_boxed_slice(),
                    },
                })
                .collect(),
        });
        for &name in &names {
            measured_step(Event::SignOut { to: ReplyTo::new(channel), channel, name });
            measured_step(Event::SignedOut {
                channel,
                name,
                result: jig_domain_fleet::SignOutResult::Failed {
                    reason: jig_domain_fleet::SourceFailure::Exited,
                    detail: vec![b'x'; usize::try_from(limits.source_detail_bytes).expect("detail bound")]
                        .into_boxed_slice(),
                },
            });
            measured_step(Event::SignOut { to: ReplyTo::new(channel), channel, name });
        }
    }
    // One sent replacement and the latest desired list coexist on every host.
    measured_step(Event::Sources { sources: Box::new([SourceName::new(b"replacement").expect("name")]) });
    measured_step(Event::Sources { sources: names });
    // Rejected full inventories coexist with loss's pending sign-out names.
    for worker in 1..=limits.workers {
        measured_step(Event::Logins {
            channel: Token::new(u64::from(worker)),
            logins: (0..limits.program_sources)
                .map(|_| Login {
                    name: SourceName::new(b"foreign").expect("foreign source"),
                    state: LoginState::Failed {
                        reason: jig_domain_fleet::SourceFailure::Exited,
                        detail: vec![b'x'; usize::try_from(limits.source_detail_bytes).expect("detail bound")]
                            .into_boxed_slice(),
                    },
                })
                .collect(),
        });
    }
}

#[test]
fn two_standing_refusals_with_no_history_retain_no_additional_heap() {
    let limits = Limits { needs_bytes: 4096, engine_slots: 1, history: 0, ..LIMITS };
    let mut world = Measured::with_needs(limits, || jig_domain_fleet::Needs {
        message_bytes: 10,
        ..jig_fleet_world::fixtures::model_needs()
    });
    let before = world.meter.held();
    for channel in [Token::new(0), Token::new(1), Token::new(2)] {
        world.step(Event::Hello {
            channel,
            worker: 0,
            hello: Hello {
                bounds: jig_domain_fleet::Bounds { message_bytes: 9, ..jig_fleet_world::fixtures::bounds() },
                stop_bound: Duration::ZERO,
                slots: 1,
                workstreams: Box::new([]),
                hosting: Box::new([]),
            },
        });
    }
    assert_eq!(world.meter.held(), before, "standing facts occupy the Domain's two inline options");
    assert!(world.domain.standing_refusal(jig_domain_fleet::Kinds::Engine).is_some());
    assert!(world.domain.standing_refusal(jig_domain_fleet::Kinds::Workers).is_some());
    assert!(world.domain.hello_refusals().is_empty());
}

#[test]
fn full_endpoint_metadata_including_engine_fits_its_accounted_channels() {
    let mut limits = LIMITS;
    limits.engine_slots = 1;
    let mut world = Measured::new(limits);
    world.hello(Token::new(0), Vec::new());
    for worker in 1..=limits.workers {
        world.hello(Token::new(u64::from(worker)), Vec::new());
    }
    assert_eq!(world.domain.program_endpoint(Token::new(0), 0), Some(true));
}

#[test]
fn retained_versions_on_every_host_stay_within_the_memory_bound() {
    use jig_domain_fleet::{CharterNeeds, Kinds, ModelNeed, Needs, Version};
    const VERSIONS: u32 = 2;
    let limits = Limits {
        needs_bytes: u64::try_from(size_of::<CharterNeeds>()).expect("charter header fits"),
        versions: VERSIONS,
        engine_slots: 1,
        ..LIMITS
    };
    let mut fleet = Measured::with_needs(limits, || {
        let mut charters = vec![CharterNeeds {
            version: Version { charter: 1, revision: VERSIONS + 1 },
            kinds: Kinds::Both,
            models: Box::new([]),
            turns: 0,
            time: Duration::ZERO,
            waiting: Duration::ZERO,
            change_reserve: None,
        }];
        for revision in 1..=VERSIONS {
            let mut charter = charters[0].clone();
            charter.version.revision = revision;
            charter.models = (0..=jig_domain_fleet::CATALOGUE_MODELS)
                .map(|_| ModelNeed {
                    name: vec![
                        b'm';
                        usize::try_from(jig_domain_fleet::CATALOGUE_NAME_BYTES).expect("model name bound")
                    ]
                    .into_boxed_slice(),
                    window: 1,
                    output: 1,
                })
                .collect();
            charters.push(charter);
        }
        Needs { charters: charters.into_boxed_slice(), ..jig_fleet_world::fixtures::needs() }
    });
    fleet.hello(Token::new(0), Vec::new());
    for nth in 0..limits.workers {
        fleet.hello(channel(nth), Vec::new());
    }
    assert_eq!(fleet.domain.workers(), limits.workers);
}

#[test]
fn park_memory_price() {
    eprintln!("park memory price: {}", worst_case(&LIMITS).expect("bounded"));
}
