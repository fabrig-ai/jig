//! Attempt-fenced drain controls through placement and reconnect.
use jig_domain_fleet::{self as fleet, Event, Hosted, Phase, Request};
use skein_lib::{Env, Queue, ReplyTo, Time, Token, Wall};

const RUN: Token = Token::new(7);
const ATTEMPT: Token = Token::new(11);
const CHANNEL: Token = Token::new(101);
const REJOINED: Token = Token::new(102);

struct World {
    domain: fleet::Domain,
    env: Env<fleet::Limits>,
    out: Queue<Request>,
}

impl World {
    fn new() -> Self {
        let limits = jig_fleet_world::LIMITS;
        Self {
            domain: jig_fleet_world::fixtures::domain(&limits, jig_fleet_world::fixtures::needs()),
            env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits },
            out: Queue::with_capacity(fleet::max_out(&limits)),
        }
    }

    fn step(&mut self, event: Event) -> Vec<Request> {
        fleet::step(&mut self.domain, &self.env, event, &mut self.out);
        self.drain()
    }

    fn drain(&mut self) -> Vec<Request> {
        let requests = std::iter::from_fn(|| self.out.pop()).collect();
        self.domain.reclaim();
        while self.domain.pop_fact().is_some() {}
        requests
    }

    fn start(&mut self) {
        assert!(self.step(Event::Start {
            reply_to: ReplyTo::new(ATTEMPT), run: RUN, attempt: ATTEMPT,
            workstream: RUN.raw(), kinds: fleet::Kinds::Workers,
            version: fleet::Version { charter: 1, revision: 1 }, placement: None,
            assignment: fleet::Assignment { turns: Token::new(0), answered: Token::new(0) },
            grants: Box::new([]),
        }).is_empty());
    }

    fn hello(&mut self, channel: Token, phase: Option<Phase>) -> Vec<Request> {
        self.step(Event::Hello {
            worker: 0, channel,
            hello: fleet::Hello {
                slots: 1, stop_bound: skein_lib::Duration::ZERO, workstreams: Box::new([]),
                bounds: jig_fleet_world::fixtures::bounds(),
                hosting: phase.map(|phase| Hosted { run: RUN, attempt: ATTEMPT, phase }).into_iter().collect(),
            },
        })
    }

    fn place(&mut self) -> Vec<Request> {
        assert!(self.domain.is_ready());
        fleet::resume(&mut self.domain, &self.env, &mut self.out);
        self.drain()
    }

    fn placed() -> Self {
        let mut world = Self::new();
        world.hello(CHANNEL, None);
        world.start();
        assert!(world.place().iter().any(|r| matches!(r, Request::Assign { .. })));
        world
    }

    fn park(&mut self) -> Vec<Request> {
        self.step(Event::Park { run: RUN, attempt: ATTEMPT })
    }
}

fn park(channel: Token) -> Request {
    Request::Park { channel, run: RUN, attempt: ATTEMPT }
}

#[test]
fn a_park_reaches_the_worker_hosting_the_attempt() {
    let mut world = World::placed();
    assert_eq!(world.park(), [park(CHANNEL)]);
    assert!(world.park().is_empty());
    assert!(world.step(Event::Park { run: RUN, attempt: Token::new(12) }).is_empty());
}

#[test]
fn a_park_is_sent_again_when_a_hello_lists_the_attempt_unanswered() {
    let mut world = World::placed();
    world.park();
    world.step(Event::Lost { channel: CHANNEL });
    assert_eq!(world.hello(REJOINED, Some(Phase::Active)), [park(REJOINED)]);
}

#[test]
fn a_park_of_an_adrift_attempt_waits_for_its_worker() {
    let mut world = World::placed();
    world.step(Event::Lost { channel: CHANNEL });
    assert!(world.park().is_empty());
    assert_eq!(world.hello(REJOINED, Some(Phase::Waiting)), [park(REJOINED)]);
}

#[test]
fn a_waiting_claim_keeps_its_park_until_placed() {
    let mut world = World::new();
    world.start();
    assert!(world.park().is_empty());
    world.hello(CHANNEL, None);
    let requests = world.place();
    assert!(matches!(&requests[..], [Request::Assign { .. }, Request::Placed { .. }, Request::Park { .. }]));
    assert_eq!(requests[2], park(CHANNEL));
}

#[test]
fn a_park_after_the_answer_changes_nothing() {
    let mut world = World::placed();
    world.park();
    let answered = world.step(Event::Answered {
        channel: CHANNEL, run: RUN, attempt: ATTEMPT,
        answer: fleet::Answer::Parked { turned: None }, payload: Token::new(99),
    });
    assert!(matches!(&answered[..], [Request::Answered { answer: fleet::Answer::Parked { .. }, .. }]));
    assert!(world.park().is_empty());
    world.step(Event::Lost { channel: CHANNEL });
    assert!(world.hello(REJOINED, Some(Phase::Answered)).is_empty());
}

#[test]
fn a_cancel_supersedes_a_park_and_is_replayed_instead() {
    let mut world = World::placed();
    world.park();
    assert!(matches!(&world.step(Event::Cancel { run: RUN, attempt: ATTEMPT })[..], [Request::Cancel { .. }]));
    assert!(world.park().is_empty());
    world.step(Event::Lost { channel: CHANNEL });
    assert!(matches!(&world.hello(REJOINED, Some(Phase::Active))[..], [Request::Cancel { channel: REJOINED, .. }]));
}
