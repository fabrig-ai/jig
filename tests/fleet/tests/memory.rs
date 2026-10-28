//! Saturated fleet lifecycle, refusals and exact ownership observations.

use jig_domain_fleet::{Answer, Domain, Event, Hello, Hosted, Limits, Phase, Request, fire, max_out, resume, step};
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

/// The fleet and its scenario inputs and requests.
struct Measured {
    domain: Domain,
    env: Env<Limits>,
    out: Queue<Request>,
    meter: Meter,
    names: u64,
}

impl Measured {
    fn new(limits: Limits) -> Measured {
        Self::with_needs(limits, jig_fleet_world::fixtures::needs)
    }

    fn with_needs(limits: Limits, needs: impl FnOnce() -> jig_domain_fleet::Needs) -> Measured {
        let meter = Meter::new();
        let domain = jig_fleet_world::fixtures::domain(&limits, needs());
        let out = Queue::with_capacity(max_out(&limits));
        Measured { domain, env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits }, out, meter, names: 0 }
    }

    fn name(&mut self) -> Token {
        self.names += 1;
        Token::new(self.names)
    }

    fn step(&mut self, event: Event) -> Vec<Request> {
        step(&mut self.domain, &self.env, event, &mut self.out);
        self.drain()
    }

    /// Places what can be placed, one resume at a time.
    fn settle(&mut self) -> Vec<Request> {
        let mut requests = Vec::new();
        while self.domain.is_ready() {
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
            fire(&mut self.domain, &self.env, &mut self.out);
            requests.extend(self.drain());
        }
        requests
    }

    fn drain(&mut self) -> Vec<Request> {
        let mut requests = Vec::with_capacity(self.out.len() as usize);
        while let Some(request) = self.out.pop() {
            if let Request::Sources { channel, sources } = request {
                assert!(sources.is_empty());
                step(&mut self.domain, &self.env, Event::Logins { channel, logins: Box::new([]) }, &mut self.out);
            } else {
                requests.push(request);
            }
        }

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
fn a_full_fleet_refuses_new_work_and_drains() {
    // This fixture fills runs; the source fixture below fills inventories.
    let (mut fleet, placed, calls) = fill(Limits { program_sources: 0, source_detail_bytes: 0, ..LIMITS });
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
fn every_entry_point_preserves_its_lifecycle() {
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
    let hosts = fleet.domain.host_views();
    let attempts = fleet.domain.attempt_views();
    let refusals = fleet.domain.hello_refusals();
    assert!(!hosts.is_empty());
    assert!(attempts.is_empty());
    assert_eq!(refusals.len(), LIMITS.history);
}

#[test]
fn held_turns_refuse_overflow_and_release_after_adoption() {
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
fn two_standing_refusals_with_no_history_retain_no_additional_heap() {
    let limits = Limits { needs_bytes: 4096, engine_slots: 1, history: 0, ..LIMITS };
    let mut world = Measured::with_needs(limits, || jig_domain_fleet::Needs {
        message_bytes: 10,
        ..jig_fleet_world::fixtures::model_needs()
    });
    let hello = |channel, bounds| Event::Hello {
        channel,
        worker: 0,
        hello: Hello { bounds, stop_bound: Duration::ZERO, slots: 1, workstreams: Box::new([]), hosting: Box::new([]) },
    };
    // Duplicate endpoints warm the facts and output queues without a standing refusal.
    world.step(hello(
        Token::new(3),
        jig_domain_fleet::Bounds { program_endpoints: Box::new([0, 0]), ..jig_fleet_world::fixtures::bounds() },
    ));
    let before = world.meter.held();
    for channel in [Token::new(0), Token::new(1), Token::new(2)] {
        world
            .step(hello(channel, jig_domain_fleet::Bounds { message_bytes: 9, ..jig_fleet_world::fixtures::bounds() }));
    }
    assert_eq!(world.meter.held(), before, "standing refusals retain no heap beyond the warmed queues");
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
