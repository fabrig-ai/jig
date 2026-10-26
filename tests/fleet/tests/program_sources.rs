use jig_domain_fleet::{self as fleet, Event, Request};
use skein_lib::{Duration, Env, Queue, ReplyTo, Time, Token, Wall};
const CHANNEL: Token = Token::new(101);
struct World {
    domain: fleet::Domain,
    env: Env<fleet::Limits>,
    out: Queue<Request>,
}
impl World {
    fn new(names: &[&[u8]]) -> Self {
        Self::under(names, fleet::Limits { program_sources: 2, ..jig_fleet_world::LIMITS })
    }
    fn under(names: &[&[u8]], limits: fleet::Limits) -> Self {
        let mut world = Self {
            domain: fleet::Domain::new(&limits, jig_fleet_world::fixtures::needs(), Box::new([])),
            env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits },
            out: Queue::with_capacity(fleet::max_out(&limits)),
        };
        let membership = fleet::Membership::new(
            Box::new([b"alpha".as_slice().into(), b"beta".as_slice().into()]),
            limits.configured_workers,
            limits.worker_name_bytes,
        )
        .expect("fixture membership");
        world.domain.configure_workers(membership);
        assert!(world.step(Event::Sources { sources: names.iter().map(|name| source(name)).collect() }).is_empty());
        world
    }
    fn step(&mut self, event: Event) -> Vec<Request> {
        fleet::step(&mut self.domain, &self.env, event, &mut self.out);
        let mut requests = Vec::new();
        while let Some(request) = self.out.pop() {
            requests.push(request);
        }
        self.domain.reclaim();
        requests
    }
    fn hello(&mut self, channel: Token) -> Vec<Request> {
        self.hello_at(channel, 0)
    }
    fn hello_at(&mut self, channel: Token, worker: u32) -> Vec<Request> {
        self.step(Event::Hello {
            worker,
            channel,
            hello: fleet::Hello {
                slots: 2,
                stop_bound: Duration::from_secs(1),
                workstreams: Box::new([]),
                hosting: Box::new([]),
                bounds: jig_fleet_world::fixtures::bounds(),
            },
        })
    }
    fn inventory(&mut self, names: &[&[u8]]) -> Vec<Request> {
        self.step(Event::Logins {
            channel: CHANNEL,
            logins: names
                .iter()
                .map(|name| fleet::Login { name: source(name), state: fleet::LoginState::NotSignedIn })
                .collect(),
        })
    }
    fn available(&self) -> bool {
        self.domain.free_kind(fleet::Kinds::Workers, 1, jig_domain_fleet::Version { charter: 0, revision: 1 }).is_some()
    }
}
fn source(name: &[u8]) -> fleet::SourceName {
    fleet::SourceName::new(name).expect("fixture source")
}
#[test]
fn even_empty_inventory_is_a_placement_barrier() {
    let mut world = World::new(&[]);
    assert!(matches!(&world.hello(CHANNEL)[..], [Request::Sources { sources, .. }] if sources.is_empty()));
    assert!(!world.available());
    assert!(world.domain.awaiting_logins(CHANNEL));
    assert!(world.inventory(&[]).is_empty());
    assert!(world.available());
    assert!(!world.domain.awaiting_logins(CHANNEL));
}
#[test]
fn old_whole_snapshot_crossing_a_replacement_updates_nothing() {
    let mut world = World::new(&[b"a", b"b"]);
    world.hello(CHANNEL);
    world.inventory(&[b"a", b"b"]);
    assert!(world.available());
    // Reordering the same set cannot start an indistinguishable barrier.
    assert!(world.step(Event::Sources { sources: Box::new([source(b"b"), source(b"a")]) }).is_empty());
    assert!(world.available());
    let sent = world.step(Event::Sources { sources: Box::new([source(b"a")]) });
    assert!(matches!(&sent[..], [Request::Sources { sources, .. }] if sources.as_ref() == [source(b"a")]));
    assert!(!world.available());
    assert!(
        world
            .step(Event::Logins {
                channel: CHANNEL,
                logins: Box::new([
                    fleet::Login { name: source(b"a"), state: fleet::LoginState::Usable },
                    fleet::Login { name: source(b"b"), state: fleet::LoginState::Usable },
                ])
            })
            .is_empty()
    );
    assert!(
        world
            .domain
            .program_logins(CHANNEL)
            .expect("old inventory")
            .iter()
            .all(|row| row.state == fleet::LoginState::NotSignedIn)
    );
    assert!(!world.available());
    world.inventory(&[b"a"]);
    assert!(world.available());
}
#[test]
fn partial_foreign_or_duplicate_snapshots_fault_the_link() {
    for names in [&[b"a".as_slice()][..], &[b"a".as_slice(), b"foreign"], &[b"a".as_slice(), b"a"]] {
        let mut world = World::new(&[b"a", b"b"]);
        world.hello(CHANNEL);
        assert!(matches!(&world.inventory(names)[..], [Request::Refuse { channel: CHANNEL }]));
        assert!(!world.available());
        assert_eq!(world.domain.program_logins(CHANNEL), None);
    }
}
#[test]
fn a_kept_login_no_longer_withholds_a_probe() {
    for state in [
        fleet::LoginState::Usable,
        fleet::LoginState::SignedInOtherwise(jig_domain_fleet::LoginMethod::new(b"api-key").expect("method")),
        fleet::LoginState::Failed { reason: jig_domain_fleet::SourceFailure::Exited, detail: Box::new([]) },
        fleet::LoginState::Failed { reason: jig_domain_fleet::SourceFailure::Deadline, detail: Box::new([]) },
    ] {
        let mut world = World::new(&[b"a"]);
        world.hello(CHANNEL);
        world.step(Event::Logins { channel: CHANNEL, logins: Box::new([fleet::Login { name: source(b"a"), state }]) });
        assert!(
            world
                .domain
                .probe(fleet::Kinds::Workers, 1, jig_domain_fleet::Version { charter: 0, revision: 1 }, &[], &[])
                .is_some()
        );
        assert!(
            world
                .domain
                .placement_available(fleet::PlacementConstraint { channel: CHANNEL, kind: fleet::HostKind::Worker })
        );
    }
}
#[test]
fn an_unconfigured_program_counts_as_absent() {
    let mut world = World::new(&[b"a"]);
    world.hello(CHANNEL);
    world.step(Event::Logins {
        channel: CHANNEL,
        logins: Box::new([fleet::Login {
            name: source(b"a"),
            state: fleet::LoginState::Failed { reason: fleet::SourceFailure::Unconfigured, detail: Box::new([]) },
        }]),
    });
    assert!(world.available());
}
#[test]
fn an_unawaited_sign_out_faults_the_link() {
    let mut world = World::new(&[b"a"]);
    world.hello(CHANNEL);
    world.inventory(&[b"a"]);
    assert_eq!(
        world.step(Event::SignedOut { channel: CHANNEL, name: source(b"a"), result: fleet::SignOutResult::SignedOut }),
        vec![Request::Refuse { channel: CHANNEL }]
    );
    assert!(!world.available());
}
#[test]
fn a_lost_sign_out_cannot_complete_a_new_channel_call() {
    let mut world = World::new(&[b"a"]);
    world.hello(CHANNEL);
    world.inventory(&[b"a"]);
    assert!(matches!(
        &world.step(Event::SignOut { to: ReplyTo::new(Token::new(1)), channel: CHANNEL, name: source(b"a") })[..],
        [Request::SignOut { .. }]
    ));
    assert!(matches!(&world.step(Event::Lost { channel: CHANNEL })[..], [Request::SignedOut { result: None, .. }]));
    assert!(world.step(Event::Lost { channel: CHANNEL }).is_empty());
    let replacement = Token::new(202);
    assert!(matches!(&world.hello(replacement)[..], [Request::Sources { .. }]));
    assert!(!world.available());
    world.step(Event::SignOut { to: ReplyTo::new(Token::new(2)), channel: replacement, name: source(b"a") });
    assert!(
        world
            .step(Event::SignedOut { channel: CHANNEL, name: source(b"a"), result: fleet::SignOutResult::SignedOut })
            .is_empty()
    );
    let ended = world.step(Event::SignedOut {
        channel: replacement,
        name: source(b"a"),
        result: fleet::SignOutResult::SignedOut,
    });
    let Some(Request::SignedOut { to, result: Some(fleet::SignOutResult::SignedOut), .. }) = ended.into_iter().next()
    else {
        panic!("sign-out completion")
    };
    assert_eq!(to.into_token(), Token::new(2));
}

fn worker(number: u32) -> fleet::Worker {
    configured().domain.configured_worker(number).expect("configured fixture worker")
}
fn configured() -> World {
    World::new(&[b"a"])
}

#[test]
fn already_offline_members_are_not_confirmed_signed_out() {
    let mut world = configured();
    assert_eq!(
        world.step(Event::SignOutProgram { source: 7, name: source(b"a") }),
        vec![
            Request::ProgramTargets { source: 7, workers: Box::new([worker(0), worker(1)]) },
            Request::ProgramSignedOut { source: 7, worker: worker(0), result: fleet::ProgramResult::NotInContact },
            Request::ProgramSignedOut { source: 7, worker: worker(1), result: fleet::ProgramResult::NotInContact },
        ]
    );
}
#[test]
fn loss_ends_the_configured_members_operation_once() {
    let mut world = configured();
    world.hello(CHANNEL);
    world.inventory(&[b"a"]);
    world.step(Event::SignOutProgram { source: 7, name: source(b"a") });
    assert_eq!(
        world.step(Event::Lost { channel: CHANNEL }),
        vec![Request::ProgramSignedOut { source: 7, worker: worker(0), result: fleet::ProgramResult::Lost },]
    );
    assert!(world.step(Event::Lost { channel: CHANNEL }).is_empty());
}
#[test]
fn reconnect_cleanup_is_separate_from_the_original_answer() {
    let mut world = configured();
    world.step(Event::CleanupPrograms { names: Box::new([source(b"a")]) });
    world.hello(CHANNEL);
    assert_eq!(
        world.step(Event::Logins {
            channel: CHANNEL,
            logins: Box::new([fleet::Login { name: source(b"a"), state: fleet::LoginState::Usable },])
        }),
        vec![Request::SignOut { channel: CHANNEL, name: source(b"a") }]
    );
    let answer =
        world.step(Event::SignedOut { channel: CHANNEL, name: source(b"a"), result: fleet::SignOutResult::SignedOut });
    assert!(answer.is_empty());
    assert!(!world.available());
    world.inventory(&[b"a"]);
    assert!(world.available());
}
#[test]
fn a_failed_cleanup_is_not_retried_by_periodic_inventory() {
    let mut world = configured();
    world.step(Event::CleanupPrograms { names: Box::new([source(b"a")]) });
    world.hello(CHANNEL);
    let rows = Box::new([fleet::Login { name: source(b"a"), state: fleet::LoginState::Usable }]);
    world.step(Event::Logins { channel: CHANNEL, logins: rows.clone() });
    world.step(Event::SignedOut {
        channel: CHANNEL,
        name: source(b"a"),
        result: fleet::SignOutResult::Failed {
            reason: fleet::SourceFailure::Exited,
            detail: b"failed".as_slice().into(),
        },
    });
    assert!(world.step(Event::Logins { channel: CHANNEL, logins: rows }).is_empty());
}
#[test]
fn a_check_waits_until_its_source_list_has_been_sent() {
    let mut world = configured();
    world.hello(CHANNEL);
    world.step(Event::Sources { sources: Box::new([source(b"a"), source(b"b")]) });
    assert!(world.step(Event::CheckProgram { name: source(b"b") }).is_empty());
    world.inventory(&[b"a"]);
    fleet::resume(&mut world.domain, &world.env, &mut world.out);
    let mut requests = Vec::new();
    while let Some(request) = world.out.pop() {
        requests.push(request);
    }
    assert!(requests.iter().any(|request| matches!(request,
        Request::Check { channel: CHANNEL, name } if *name == source(b"b"))));
}

#[test]
fn a_hello_refuses_an_unconfigured_position() {
    let mut world = configured();
    let rows = world.hello_at(CHANNEL, 2);
    assert_eq!(rows, vec![Request::Refuse { channel: CHANNEL }]);
    assert_eq!(world.domain.workers(), 0);
    assert_eq!(
        world.domain.hello_refusals().get(0).expect("position refusal").why,
        fleet::HelloRefusal::Position { worker: 2 }
    );
}
#[test]
fn a_hello_refuses_an_occupied_position() {
    let mut world = configured();
    world.hello(CHANNEL);
    let other = Token::new(102);
    assert_eq!(world.hello(other), vec![Request::Refuse { channel: other }]);
    assert_eq!(world.domain.workers(), 1);
    assert_eq!(
        world.domain.hello_refusals().get(0).expect("position refusal").why,
        fleet::HelloRefusal::Position { worker: 0 }
    );
}
#[test]
fn fan_out_reaches_distinct_configured_positions() {
    let mut world = configured();
    world.hello(CHANNEL);
    let other = Token::new(102);
    world.hello_at(other, 1);
    world.inventory(&[b"a"]);
    world.step(Event::Logins {
        channel: other,
        logins: Box::new([fleet::Login { name: source(b"a"), state: fleet::LoginState::NotSignedIn }]),
    });
    let rows = world.step(Event::SignOutProgram { source: 7, name: source(b"a") });
    assert_eq!(
        rows,
        vec![
            Request::ProgramTargets { source: 7, workers: Box::new([worker(0), worker(1)]) },
            Request::SignOut { channel: CHANNEL, name: source(b"a") },
            Request::SignOut { channel: other, name: source(b"a") }
        ]
    );
}

#[test]
fn a_holder_sign_out_joins_pending_cleanup() {
    let mut world = configured();
    world.step(Event::CleanupPrograms { names: Box::new([source(b"a")]) });
    world.hello(CHANNEL);
    world.step(Event::Logins {
        channel: CHANNEL,
        logins: Box::new([fleet::Login { name: source(b"a"), state: fleet::LoginState::Usable }]),
    });
    let started = world.step(Event::SignOutProgram { source: 7, name: source(b"a") });
    assert_eq!(
        started,
        vec![
            Request::ProgramTargets { source: 7, workers: Box::new([worker(0), worker(1)]) },
            Request::ProgramSignedOut { source: 7, worker: worker(1), result: fleet::ProgramResult::NotInContact }
        ]
    );
    assert_eq!(
        world.step(Event::SignedOut { channel: CHANNEL, name: source(b"a"), result: fleet::SignOutResult::SignedOut }),
        vec![Request::ProgramSignedOut { source: 7, worker: worker(0), result: fleet::ProgramResult::SignedOut }]
    );
}
#[test]
fn a_holder_sign_out_shares_a_direct_terminal() {
    let mut world = configured();
    world.hello(CHANNEL);
    world.inventory(&[b"a"]);
    let reply = Token::new(41);
    world.step(Event::SignOut { to: ReplyTo::new(reply), channel: CHANNEL, name: source(b"a") });
    let started = world.step(Event::SignOutProgram { source: 7, name: source(b"a") });
    assert_eq!(
        started,
        vec![
            Request::ProgramTargets { source: 7, workers: Box::new([worker(0), worker(1)]) },
            Request::ProgramSignedOut { source: 7, worker: worker(1), result: fleet::ProgramResult::NotInContact }
        ]
    );
    let ended =
        world.step(Event::SignedOut { channel: CHANNEL, name: source(b"a"), result: fleet::SignOutResult::SignedOut });
    let mut ended = ended.into_iter();
    let Some(Request::SignedOut { to, result, .. }) = ended.next() else { panic!("direct terminal") };
    assert_eq!(to.into_token(), reply);
    assert_eq!(result, Some(fleet::SignOutResult::SignedOut));
    assert_eq!(
        ended.collect::<Vec<_>>(),
        vec![Request::ProgramSignedOut { source: 7, worker: worker(0), result: fleet::ProgramResult::SignedOut }]
    );
}
#[test]
fn channel_loss_answers_both_attached_replies() {
    let mut world = configured();
    world.hello(CHANNEL);
    world.inventory(&[b"a"]);
    let reply = Token::new(41);
    world.step(Event::SignOut { to: ReplyTo::new(reply), channel: CHANNEL, name: source(b"a") });
    world.step(Event::SignOutProgram { source: 7, name: source(b"a") });
    let mut ended = world.step(Event::Lost { channel: CHANNEL }).into_iter();
    let Some(Request::SignedOut { to, result, .. }) = ended.next() else { panic!("direct terminal") };
    assert_eq!(to.into_token(), reply);
    assert_eq!(result, None);
    assert_eq!(
        ended.collect::<Vec<_>>(),
        vec![Request::ProgramSignedOut { source: 7, worker: worker(0), result: fleet::ProgramResult::Lost }]
    );
}

#[test]
fn joined_calls_fit_the_loss_output_bound() {
    const NAMES: [&[u8]; 2] = [b"a", b"b"];
    let limits = fleet::Limits {
        configured_workers: 2,
        workers: 1,
        slots: 1,
        attempts: 1,
        grants: 0,
        program_sources: u32::try_from(NAMES.len()).expect("source count"),
        ..jig_fleet_world::LIMITS
    };
    let mut world = World::under(&NAMES, limits);
    world.hello(CHANNEL);
    world.inventory(&NAMES);
    for (index, name) in NAMES.iter().enumerate() {
        world.step(Event::SignOut {
            to: ReplyTo::new(Token::new(u64::try_from(index).expect("reply token"))),
            channel: CHANNEL,
            name: source(name),
        });
    }
    for (index, name) in NAMES.iter().enumerate() {
        world.step(Event::SignOutProgram { source: u32::try_from(index).expect("source number"), name: source(name) });
    }
    let rows = world.step(Event::Lost { channel: CHANNEL });
    assert_eq!(rows.len(), NAMES.len() * 2);
    assert!(rows.iter().all(|row| matches!(
        row,
        Request::SignedOut { result: None, .. } | Request::ProgramSignedOut { result: fleet::ProgramResult::Lost, .. }
    )));
}

#[test]
fn a_second_holder_attachment_receives_a_failure() {
    let mut world = configured();
    world.hello(CHANNEL);
    world.inventory(&[b"a"]);
    world.step(Event::SignOutProgram { source: 3, name: source(b"a") });
    assert_eq!(
        world.step(Event::SignOutProgram { source: 7, name: source(b"a") }),
        vec![
            Request::ProgramTargets { source: 7, workers: Box::new([worker(0), worker(1)]) },
            Request::ProgramSignedOut { source: 7, worker: worker(0), result: fleet::ProgramResult::Failed },
            Request::ProgramSignedOut { source: 7, worker: worker(1), result: fleet::ProgramResult::NotInContact }
        ]
    );
    assert_eq!(
        world.step(Event::SignedOut { channel: CHANNEL, name: source(b"a"), result: fleet::SignOutResult::SignedOut }),
        vec![Request::ProgramSignedOut { source: 3, worker: worker(0), result: fleet::ProgramResult::SignedOut }]
    );
}

#[test]
fn a_full_pending_map_answers_the_holder_failed() {
    let mut world = World::new(&[b"a", b"b"]);
    world.hello(CHANNEL);
    world.inventory(&[b"a", b"b"]);
    for (index, name) in [b"a", b"b"].iter().enumerate() {
        world.step(Event::SignOut {
            to: ReplyTo::new(Token::new(u64::try_from(index).expect("reply number"))),
            channel: CHANNEL,
            name: source(*name),
        });
    }
    world.step(Event::Sources { sources: Box::new([source(b"c")]) });
    assert_eq!(
        world.step(Event::SignOutProgram { source: 7, name: source(b"c") }),
        vec![
            Request::ProgramTargets { source: 7, workers: Box::new([worker(0), worker(1)]) },
            Request::ProgramSignedOut { source: 7, worker: worker(0), result: fleet::ProgramResult::Failed },
            Request::ProgramSignedOut { source: 7, worker: worker(1), result: fleet::ProgramResult::NotInContact }
        ]
    );
}

#[test]
fn an_unconstrained_choice_avoids_kept_logins() {
    let mut world = World::new(&[b"a"]);
    world.hello(CHANNEL);
    world.step(Event::Logins {
        channel: CHANNEL,
        logins: Box::new([fleet::Login { name: source(b"a"), state: fleet::LoginState::Usable }]),
    });
    assert_eq!(
        world.domain.free_kind(fleet::Kinds::Workers, 1, jig_domain_fleet::Version { charter: 0, revision: 1 }),
        None
    );
}

#[test]
fn a_held_retired_name_still_withholds_its_host() {
    let mut world = World::new(&[b"a"]);
    world.hello(CHANNEL);
    world.step(Event::Logins {
        channel: CHANNEL,
        logins: Box::new([fleet::Login { name: source(b"a"), state: fleet::LoginState::Usable }]),
    });
    world.step(Event::Sources { sources: Box::new([]) });
    world.inventory(&[]);
    assert!(
        world
            .domain
            .probe(fleet::Kinds::Workers, 1, jig_domain_fleet::Version { charter: 0, revision: 1 }, &[], &[])
            .is_none()
    );
}
