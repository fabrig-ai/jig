use super::fixtures;
use crate::{End, stop};
use jig_test_domain as root;

fn components() -> crate::Service {
    {
        let mut service = fixtures::started(&fixtures::workers_document());
        for _ in 0..64 {
            fixtures::turn(&mut service);
            if !service.work_pending(skein_lib::Time::from_nanos(1)) && fixtures::engine(&mut service).domain.ready() {
                break;
            }
        }
        assert_eq!(service.end(), None);
        assert!(fixtures::engine(&mut service).domain.ready());
        service
    }
}

fn settle(service: &mut crate::Service) {
    for _ in 0_u32..16 {
        fixtures::turn(service);
    }
}

#[test]
fn a_shutdown_stops_the_listeners_before_the_core_hears_stopping() {
    let mut service = components();
    service.shutdown();
    let engine = fixtures::engine(&mut service);
    assert!(!engine.domain.stopping());
    assert_eq!(engine.drain.as_ref().expect("drain").phase, stop::Phase::Stopping);
    assert_eq!(engine.workers.as_ref().expect("workers").ending, None);
    assert_eq!(engine.events.len(), 1);
    let Some(root::Event::Stop) = engine.events.pop() else {
        panic!("the core hears stopping next");
    };
}

#[test]
fn components_close_only_after_the_core_says_drained() {
    let mut service = components();
    service.shutdown();
    let engine = fixtures::engine(&mut service);
    assert_eq!(engine.workers.as_ref().expect("workers").ending, None);
    assert_eq!(engine.drain.as_ref().expect("drain").phase, stop::Phase::Stopping);
    stop::drained(engine);
    assert_eq!(engine.workers.as_ref().expect("workers").ending, Some(stop::Closing::Drain));
    assert_eq!(engine.drain.as_ref().expect("drain").phase, stop::Phase::Closing { waiting: 2 });
}

#[test]
fn the_store_closes_last() {
    let mut service = components();
    service.shutdown();
    stop::drained(fixtures::engine(&mut service));
    assert_eq!(service.end(), None, "components have not reported closed");
    settle(&mut service);
    assert_eq!(service.end(), Some(&End::Drained));
    assert!(fixtures::engine(&mut service).accounts.as_ref().expect("accounts").is_empty());
    assert!(fixtures::engine(&mut service).workers.as_ref().expect("workers").closed());
}

#[test]
fn a_second_shutdown_aborts_each_component() {
    let mut service = components();
    service.shutdown();
    service.shutdown();
    fixtures::turn(&mut service);
    assert_eq!(service.end(), Some(&End::Aborted));
    settle(&mut service);
    assert!(fixtures::engine(&mut service).accounts.as_ref().expect("accounts").is_empty());
    assert!(fixtures::engine(&mut service).workers.as_ref().expect("workers").closed());
}

#[test]
fn an_in_process_workers_listener_sets_workers_in_process() {
    let config = fixtures::config();
    assert!(jig_protocol_config::core(&config, [7; 16], 7).workers_in_process);
}

#[test]
fn a_connector_input_returned_busy_is_kept_until_the_drain_ends() {
    let mut service = components();
    service.shutdown();
    crate::route::request(
        fixtures::engine(&mut service),
        root::Request::Busy { event: Box::new(root::Event::ConnectorTimer { number: 1 }) },
    );
    assert_eq!(fixtures::engine(&mut service).drain.as_ref().expect("drain").busy.len(), 1);
    stop::drained(fixtures::engine(&mut service));
    settle(&mut service);
    assert_eq!(service.end(), Some(&End::Drained));
    assert!(fixtures::engine(&mut service).drain.as_ref().expect("drain").busy.is_empty());
}

#[test]
fn a_call_the_root_answers_busy_while_stopping_reaches_its_worker() {
    let mut service = components();
    service.shutdown();
    crate::route::request(
        fixtures::engine(&mut service),
        root::Request::Deliver(root::Delivery::CallBusy {
            channel: skein_lib::Token::new(17),
            task: 2,
            attempt: 3,
            name: b"call".as_slice().into(),
        }),
    );
    assert_eq!(
        fixtures::engine(&mut service).workers.as_ref().expect("workers").component.dropped_down(),
        1,
        "the busy relay reaches the workers component even when its channel has gone"
    );
    assert_eq!(service.end(), None);
}

fn combined() -> crate::Service {
    let mut service = components();
    service.workers = fixtures::worker_service().workers;
    service
}
fn in_process_worker(service: &crate::Service) -> &crate::Worker {
    service.workers.get(0).expect("one worker layer")
}

#[test]
fn the_worker_goes_on_while_the_engine_drains() {
    let mut service = combined();
    service.shutdown();
    assert!(in_process_worker(&service).aborted.is_none());
    assert_eq!(service.end(), None);
}

#[test]
fn the_worker_is_aborted_only_once_the_engine_has_drained() {
    let mut service = combined();
    service.shutdown();
    assert!(in_process_worker(&service).aborted.is_none());
    stop::drained(fixtures::engine(&mut service));
    settle(&mut service);
    assert_eq!(service.end(), Some(&End::Drained));
    assert!(in_process_worker(&service).aborted.is_some());
}

#[test]
fn a_second_signal_aborts_both_layers() {
    let mut service = combined();
    service.shutdown();
    service.shutdown();
    assert_eq!(service.end(), Some(&End::Aborted));
    assert!(in_process_worker(&service).aborted.is_some());
    let count = in_process_worker(&service).io.len();
    service.shutdown();
    assert_eq!(in_process_worker(&service).io.len(), count, "abort is idempotent");
}

#[test]
fn the_service_ends_once_io_holds_nothing() {
    let mut service = components();
    service.shutdown();
    assert!(!service.is_empty(), "the components have not reported closed");
    settle(&mut service);
    assert_eq!(service.end(), Some(&End::Drained));
    assert!(service.is_empty());
}

#[test]
fn a_paused_core_timer_does_not_pin_the_service_while_stopping() {
    use jig_domain_core as core;
    use skein_lib::{Duration, Secret, Time, Wall};
    const EXPIRE_VALUES: Time = Time::from_nanos(u64::MAX);
    const LIVE_GAP: Duration = Duration::from_secs(1);
    const ACCOUNT: u32 = 1;
    const GENERATION: u64 = 1;
    let mut driven = fixtures::Driven::started(&fixtures::engine_document());
    let engine = fixtures::engine(&mut driven.service);
    engine.values.accounts.expire(EXPIRE_VALUES);
    let timers = engine.domain.core_deadlines(engine.env.now, engine.env.wall);
    let mut people = None;
    for (timer, deadline) in timers {
        if timer == core::Timer::People {
            people = deadline;
        }
    }
    let due = people.expect("sign-in timer");
    let live = due.saturating_add(LIVE_GAP);
    let value = jig_protocol_accounts::table::Value {
        token: Secret::new(fixtures::TOKEN.into()),
        scheme: jig_protocol_accounts::table::Scheme::Bearer,
    };
    engine.values.accounts.granted(ACCOUNT, GENERATION, value, live).unwrap();
    engine.env.now = due;
    engine.env.wall = Wall::from_nanos(due.as_nanos());
    assert_eq!(engine.next_deadline(), Some(due));
    engine.step(root::Event::Stop);
    assert!(engine.domain.stopping());
    assert_eq!(
        engine.domain.core_deadlines(engine.env.now, engine.env.wall),
        timers,
        "paused timers keep their values"
    );
    assert_eq!(engine.next_deadline(), Some(live), "only a live timer asks the service to fire");
}

#[test]
fn the_service_fires_wall_expiry_before_the_original_elapsed_alarm() {
    use jig_domain_core as core;
    use jig_domain_people as people;
    use skein_lib::{Time, Token};
    const WATCHER: Token = Token::new(901);
    const ELAPSED: Time = Time::from_nanos(2); // The fixture signs in at elapsed 1.
    let mut driven = fixtures::Driven::started(&fixtures::engine_document());
    let Some(people::Reply::SignedIn { expires, .. }) = driven.reply else {
        panic!("fixture sign-in");
    };
    driven.party(core::Event::Watch {
        watcher: WATCHER,
        sign_in: driven.sign_in.raw(),
        subject: core::documents::Watched::Frame,
        window: core::documents::Window { rows: 1, after: None },
    });
    driven.commit_all();
    driven.party(core::Event::Delivered { watcher: WATCHER });
    driven.commit_all();
    let engine = fixtures::engine(&mut driven.service);
    let original = engine.domain.core_deadlines(engine.env.now, engine.env.wall);
    let mut original_people = None;
    for (timer, deadline) in original {
        if timer == core::Timer::People {
            original_people = deadline;
        }
    }
    assert!(original_people.expect("armed sign-in") > ELAPSED);
    engine.env.now = ELAPSED;
    engine.env.wall = expires;
    assert_eq!(engine.next_deadline(), Some(ELAPSED), "the service resamples saved wall expiry");
    let mut endings = 0_u32;
    // The existing service/store route commits the expiry and releases its ending.
    for _ in 0_u32..16 {
        crate::iterate(&mut driven.service, ELAPSED, expires);
        let engine = fixtures::engine(&mut driven.service);
        while let Some(output) = engine.parties_out().pop() {
            let crate::PartyOutput::Held(root::Delivery::Core(core::Held::WatchEnded { watcher, why })) = output else {
                panic!("unexpected expiry output: {output:?}");
            };
            assert_eq!(watcher, WATCHER, "expiry ends the bound watch ({why:?})");
            endings += 1;
        }
    }
    assert_eq!(endings, 1, "one terminal through the ordinary service timer route");
    let engine = fixtures::engine(&mut driven.service);
    assert!(
        engine.domain.core_deadlines(engine.env.now, engine.env.wall).contains(&(core::Timer::People, None)),
        "the timer removed the sign-in, rather than only closing its watch during Resume"
    );
}

#[test]
fn a_stop_at_the_linked_sweep_drains_without_starting_the_accounts_driver() {
    let mut service = fixtures::started(&fixtures::accounts_document());
    let boundary = jig_domain_core::RestartStep::Sweep;
    let mut reached = false;
    for _ in 0_u32..64 {
        fixtures::turn(&mut service);
        if fixtures::engine(&mut service).domain.restart_step() == Some(boundary) {
            reached = true;
            break;
        }
    }
    assert!(reached, "the fixture reaches its linked boundary");
    assert!(!fixtures::engine(&mut service).store.component.mark_durable());
    service.shutdown();
    for _ in 0_u32..64 {
        fixtures::turn(&mut service);
        if service.end().is_some() {
            break;
        }
    }
    let engine = fixtures::engine(&mut service);
    assert_eq!(
        engine.end,
        Some(End::Drained),
        "linked stop: step {:?}, drain {:?}, accounts {:?}, marked {}",
        engine.domain.restart_step(),
        engine.drain,
        engine.accounts.as_ref().expect("accounts fixture").startup(0),
        engine.store.component.mark_durable()
    );
    assert!(engine.accounts.as_ref().expect("accounts fixture").is_empty());
    assert!(!engine.store.component.mark_durable(), "an interrupted start writes no mark");
}
