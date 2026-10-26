//! A worker's account notices, and the account a failed run turned on,
//! reach the parent from the worker hosting the attempt, and from no other.

use jig_domain_fleet::{self as fleet, Answer, Domain, Event, Hello, Kinds, Limits, Request, Turned};
use skein_lib::{Duration, Env, Queue, ReplyTo, Time, Token, Wall};

const LIMITS: Limits = Limits { workers: 2, engine_slots: 0, ..jig_fleet_world::LIMITS };
const WORKER: Token = Token::new(5);
const OTHER: Token = Token::new(6);
const RUN: Token = Token::new(3);
const ATTEMPT: Token = Token::new(4);

fn step(domain: &mut Domain, event: Event) -> Vec<Request> {
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: LIMITS };
    let mut out = Queue::with_capacity(fleet::max_out(&LIMITS));
    fleet::step(domain, &env, event, &mut out);
    let mut requests = Vec::new();
    while let Some(request) = out.pop() {
        if let Request::Sources { channel, sources } = request {
            assert!(sources.is_empty());
            fleet::step(domain, &env, Event::Logins { channel, logins: Box::new([]) }, &mut out);
        } else {
            requests.push(request);
        }
    }
    domain.reclaim();
    requests
}

fn hello(channel: Token) -> Event {
    Event::Hello {
        worker: u32::try_from(channel.raw()).expect("fixture channel") % jig_fleet_world::LIMITS.configured_workers,
        channel,
        hello: Hello {
            bounds: jig_fleet_world::fixtures::bounds(),
            slots: 1,
            stop_bound: Duration::from_secs(1),
            workstreams: Box::new([]),
            hosting: Box::new([]),
        },
    }
}

/// An attempt hosted by `WORKER`, with `OTHER` also in contact.
fn hosted() -> Domain {
    let mut domain = jig_fleet_world::fixtures::domain(&LIMITS, jig_fleet_world::fixtures::needs());
    assert!(step(&mut domain, hello(WORKER)).is_empty());
    let start = Event::Start {
        placement: None,
        version: jig_domain_fleet::Version { charter: 1, revision: 1 },
        grants: Box::new([]),
        reply_to: ReplyTo::new(ATTEMPT),
        run: RUN,
        attempt: ATTEMPT,
        workstream: 1,
        kinds: Kinds::Workers,
        assignment: jig_domain_fleet::Assignment { turns: Token::new(0), answered: Token::new(0) },
    };
    assert!(step(&mut domain, start).is_empty());
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: LIMITS };
    let mut out = Queue::with_capacity(fleet::max_out(&LIMITS));
    fleet::resume(&mut domain, &env, &mut out);
    assert!(matches!(out.pop(), Some(Request::Assign { channel: WORKER, .. })));
    assert!(matches!(out.pop(), Some(Request::Placed { .. })));
    domain.reclaim();
    assert!(step(&mut domain, hello(OTHER)).is_empty());
    domain
}

fn refused(channel: Token) -> Event {
    Event::AccountRefused {
        channel,
        run: RUN,
        attempt: ATTEMPT,
        account: 7,
        code: Box::from(&b"forbidden"[..]),
        reason: Box::from(&b"suspended"[..]),
    }
}

#[test]
fn each_notice_from_the_hosting_worker_reaches_the_parent() {
    let mut domain = hosted();
    assert_eq!(
        step(
            &mut domain,
            Event::Unauthorized { channel: WORKER, run: RUN, attempt: ATTEMPT, account: 7, generation: 2 }
        ),
        [Request::Unauthorized { run: RUN, attempt: ATTEMPT, account: 7, generation: 2 }]
    );
    let reset = Some(Duration::from_secs(30));
    assert_eq!(
        step(&mut domain, Event::Limited { channel: WORKER, run: RUN, attempt: ATTEMPT, account: 7, reset }),
        [Request::Limited { run: RUN, attempt: ATTEMPT, account: 7, reset }]
    );
    assert_eq!(
        step(&mut domain, Event::Limited { channel: WORKER, run: RUN, attempt: ATTEMPT, account: 7, reset: None }),
        [Request::Limited { run: RUN, attempt: ATTEMPT, account: 7, reset: None }]
    );
    assert_eq!(
        step(&mut domain, refused(WORKER)),
        [Request::AccountRefused {
            run: RUN,
            attempt: ATTEMPT,
            account: 7,
            code: Box::from(&b"forbidden"[..]),
            reason: Box::from(&b"suspended"[..]),
        }]
    );
}

#[test]
fn a_notice_from_a_worker_not_hosting_the_attempt_reaches_nothing() {
    let mut domain = hosted();
    let notices = [
        Event::Unauthorized { channel: OTHER, run: RUN, attempt: ATTEMPT, account: 7, generation: 2 },
        Event::Limited { channel: OTHER, run: RUN, attempt: ATTEMPT, account: 7, reset: None },
        refused(OTHER),
    ];
    for notice in notices {
        assert_eq!(step(&mut domain, notice), []);
    }
}

#[test]
fn a_failed_answer_carries_the_account_it_turned_on_to_the_parent() {
    for turned in [
        Turned::Limited { account: 7, reset: Some(Duration::from_secs(30)) },
        Turned::Limited { account: 7, reset: None },
        Turned::Refused { account: 7 },
    ] {
        let mut domain = hosted();
        let answer = Answer::Failed { turned: Some(turned) };
        let payload = Token::new(9);
        assert_eq!(
            step(&mut domain, Event::Answer { channel: WORKER, run: RUN, attempt: ATTEMPT, answer, payload }),
            [Request::Answered { to: ReplyTo::new(ATTEMPT), run: RUN, attempt: ATTEMPT, answer, payload }]
        );
    }
}
