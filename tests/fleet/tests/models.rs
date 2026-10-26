//! Declared models narrow placement without refusing a host's hello (engine.md, 8).
use jig_domain_fleet::{self as fleet, AgentShortfall, Answer, Event, Hello, HelloRefusal, HostKind, Kinds, Request};
use jig_fleet_world::fixtures::{MODEL_CHARTER, MODEL_OUTPUT, MODEL_WINDOW, OTHER_CHARTER, model_bounds, model_needs};
use skein_lib::{Duration, Env, Queue, ReplyTo, Time, Token, Wall};

const FIRST: Token = Token::new(1);
const SECOND: Token = Token::new(2);
const SUITABLE: Token = Token::new(3);
const ENGINE: Token = Token::new(0);
const RUN: Token = Token::new(10);
const ATTEMPT: Token = Token::new(11);
const WORKSTREAM: u64 = 7;

struct Fleet {
    domain: fleet::Domain,
    env: Env<fleet::Limits>,
}

impl Fleet {
    fn new() -> Self {
        let limits = fleet::Limits { engine_slots: 1, ..jig_fleet_world::LIMITS };
        Self {
            domain: jig_fleet_world::fixtures::domain(&limits, model_needs()),
            env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits },
        }
    }

    fn send(&mut self, event: Event) -> Vec<Request> {
        let mut out = Queue::with_capacity(fleet::max_out(&self.env.limits));
        fleet::step(&mut self.domain, &self.env, event, &mut out);
        self.domain.reclaim();
        let mut requests = Vec::new();
        while let Some(request) = out.pop() {
            if let Request::Sources { channel, sources } = request {
                assert!(sources.is_empty());
                fleet::step(&mut self.domain, &self.env, Event::Logins { channel, logins: Box::new([]) }, &mut out);
            } else {
                requests.push(request);
            }
        }
        requests
    }

    fn hello(&mut self, channel: Token, names: &[&[u8]]) -> Vec<Request> {
        self.send(Event::Hello {
            worker: u32::try_from(channel.raw()).expect("fixture channel fits") % self.env.limits.configured_workers,
            channel,
            hello: hello(model_bounds(names)),
        })
    }

    fn start(&mut self, charter: u32) {
        self.send(Event::Start {
            placement: None,
            version: jig_domain_fleet::Version { charter, revision: 1 },
            grants: Box::new([]),
            reply_to: ReplyTo::new(ATTEMPT),
            run: RUN,
            attempt: ATTEMPT,
            workstream: WORKSTREAM,
            kinds: Kinds::Both,
            assignment: fleet::Assignment { turns: RUN, answered: ATTEMPT },
        });
    }

    fn settle(&mut self) -> Vec<Token> {
        let mut assigned = Vec::new();
        let mut out = Queue::with_capacity(fleet::max_out(&self.env.limits));
        while self.domain.is_ready() {
            fleet::resume(&mut self.domain, &self.env, &mut out);
            while let Some(request) = out.pop() {
                if let Request::Assign { channel, .. } = request {
                    assigned.push(channel);
                }
            }
            self.domain.reclaim();
        }
        assigned
    }
}

fn hello(bounds: fleet::Bounds) -> Hello {
    Hello { bounds, stop_bound: Duration::ZERO, slots: 1, workstreams: Box::new([]), hosting: Box::new([]) }
}

#[test]
fn a_hello_without_required_models_is_admitted() {
    for channel in [FIRST, ENGINE] {
        let mut fleet = Fleet::new();
        assert!(fleet.hello(channel, &[]).is_empty(), "{channel:?}");
        assert!(fleet.domain.hello_refusals().is_empty());
    }
}

#[test]
fn a_declared_models_smaller_window_or_output_refuses_hello() {
    for (window, output, why) in [
        (MODEL_WINDOW - 1, MODEL_OUTPUT, AgentShortfall::Window { charter: MODEL_CHARTER, model: 0 }),
        (MODEL_WINDOW, MODEL_OUTPUT - 1, AgentShortfall::Output { charter: MODEL_CHARTER, model: 0 }),
    ] {
        let mut fleet = Fleet::new();
        let mut bounds = model_bounds(&[b"main"]);
        bounds.models[0].window = window;
        bounds.models[0].output = output;
        assert_eq!(
            fleet.send(Event::Hello { worker: 0, channel: FIRST, hello: hello(bounds) }),
            vec![Request::Refuse { channel: FIRST }]
        );
        assert_eq!(fleet.domain.hello_refusals().get(0).expect("refused hello").why, HelloRefusal::Agent { why });
    }
}

#[test]
fn every_charter_model_is_required_before_slot_preferences_apply() {
    for missing in [b"main".as_slice(), b"alternative".as_slice()] {
        let mut fleet = Fleet::new();
        let mut unsuitable = hello(model_bounds(&[missing]));
        unsuitable.slots = fleet.env.limits.slots;
        unsuitable.workstreams = Box::new([WORKSTREAM]);
        fleet.send(Event::Hello { worker: 0, channel: FIRST, hello: unsuitable });
        fleet.hello(SUITABLE, &[b"main", b"alternative"]);
        fleet.start(MODEL_CHARTER);
        assert_eq!(
            fleet.domain.free_kind(
                Kinds::Both,
                WORKSTREAM,
                jig_domain_fleet::Version { charter: MODEL_CHARTER, revision: 1 }
            ),
            Some(HostKind::Worker)
        );
        assert_eq!(fleet.settle(), vec![SUITABLE], "host declares only {missing:?}");
    }
}

#[test]
fn a_waiting_run_is_placed_when_a_suitable_host_joins() {
    let mut fleet = Fleet::new();
    fleet.hello(FIRST, &[b"main"]);
    fleet.start(MODEL_CHARTER);
    assert!(fleet.settle().is_empty());
    assert_eq!(fleet.domain.waiting(), 1);
    assert!(!fleet.domain.is_ready(), "an unavailable model does not spin");
    fleet.hello(SUITABLE, &[b"main", b"alternative"]);
    assert_eq!(fleet.settle(), vec![SUITABLE]);
    assert_eq!(fleet.domain.waiting(), 0);
}

#[test]
fn eligibility_uses_the_runs_charter_number() {
    let mut fleet = Fleet::new();
    fleet.hello(FIRST, &[b"main", b"alternative"]);
    fleet.hello(SECOND, &[b"other"]);
    fleet.start(OTHER_CHARTER);
    assert_eq!(fleet.settle(), vec![SECOND]);
}

#[test]
fn a_busy_refusal_keeps_the_runs_model_requirements() {
    let mut fleet = Fleet::new();
    fleet.hello(FIRST, &[b"main", b"alternative"]);
    fleet.start(MODEL_CHARTER);
    fleet.settle();
    fleet.hello(SECOND, &[b"main"]);
    fleet.send(Event::Answer { channel: FIRST, run: RUN, attempt: ATTEMPT, answer: Answer::Busy, payload: RUN });
    assert!(fleet.settle().is_empty());
    assert_eq!(fleet.domain.waiting(), 1);
    fleet.hello(SUITABLE, &[b"main", b"alternative"]);
    assert_eq!(fleet.settle(), vec![SUITABLE]);
}

#[test]
fn the_engine_hello_supplies_its_model_eligibility() {
    let mut fleet = Fleet::new();
    fleet.start(MODEL_CHARTER);
    fleet.settle();
    fleet.hello(ENGINE, &[b"main", b"alternative"]);
    assert_eq!(
        fleet.domain.free_kind(
            Kinds::Both,
            WORKSTREAM,
            jig_domain_fleet::Version { charter: MODEL_CHARTER, revision: 1 }
        ),
        Some(HostKind::Engine)
    );
    assert_eq!(fleet.settle(), vec![ENGINE]);
}

fn main_need() -> fleet::ModelNeed {
    fleet::ModelNeed { name: b"main".as_slice().into(), window: MODEL_WINDOW, output: MODEL_OUTPUT }
}

#[test]
fn a_probe_requires_only_the_models_retained_for_this_run() {
    let mut fleet = Fleet::new();
    fleet.hello(FIRST, &[b"main"]);
    let main = [main_need()];
    assert_eq!(
        fleet.domain.probe(
            Kinds::Workers,
            WORKSTREAM,
            jig_domain_fleet::Version { charter: MODEL_CHARTER, revision: 1 },
            &main,
            &[]
        ),
        Some(fleet::PlacementConstraint { channel: FIRST, kind: HostKind::Worker })
    );
    let retained = [
        main_need(),
        fleet::ModelNeed { name: b"alternative".as_slice().into(), window: MODEL_WINDOW, output: MODEL_OUTPUT },
    ];
    assert_eq!(
        fleet.domain.probe(
            Kinds::Workers,
            WORKSTREAM,
            jig_domain_fleet::Version { charter: MODEL_CHARTER, revision: 1 },
            &retained,
            &[]
        ),
        None
    );
}

#[test]
fn a_pending_assignment_keeps_its_checked_host_when_room_is_lost() {
    let mut fleet = Fleet::new();
    fleet.hello(FIRST, &[b"main"]);
    fleet.hello(SECOND, &[b"main", b"alternative"]);
    let placement = fleet
        .domain
        .probe(
            Kinds::Workers,
            WORKSTREAM,
            jig_domain_fleet::Version { charter: MODEL_CHARTER, revision: 1 },
            &[main_need()],
            &[],
        )
        .expect("free main-model host");
    // A different attempt takes the last slot after the read-only probe.
    fleet.send(Event::Start {
        placement: Some(placement),
        version: jig_domain_fleet::Version { charter: MODEL_CHARTER, revision: 1 },
        grants: Box::new([]),
        reply_to: ReplyTo::new(Token::new(31)),
        run: Token::new(30),
        attempt: Token::new(31),
        workstream: WORKSTREAM,
        kinds: Kinds::Workers,
        assignment: fleet::Assignment { turns: RUN, answered: ATTEMPT },
    });
    fleet.settle();
    fleet.send(Event::Start {
        placement: Some(placement),
        version: jig_domain_fleet::Version { charter: MODEL_CHARTER, revision: 1 },
        grants: Box::new([]),
        reply_to: ReplyTo::new(ATTEMPT),
        run: RUN,
        attempt: ATTEMPT,
        workstream: WORKSTREAM,
        kinds: Kinds::Workers,
        assignment: fleet::Assignment { turns: RUN, answered: ATTEMPT },
    });
    assert!(fleet.settle().is_empty(), "the second same-kind host is not a substitute");
    assert_eq!(fleet.domain.waiting(), 0, "the stale check is refused for a fresh allowance");
}

#[path = "models/standing_refusal.rs"]
mod standing_refusal;
