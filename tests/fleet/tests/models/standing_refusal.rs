//! Standing admission facts survive history loss, but never blame an admitted host.
use jig_domain_fleet::{self as fleet, AgentShortfall, Event, Hello, HelloRefusal, HostKind, Kinds};
use skein_lib::{Duration, Env, Time, Token, Wall};

use super::Fleet;
impl Fleet {
    fn standing(history: u32) -> Self {
        let limits = fleet::Limits { engine_slots: 1, workers: 1, history, ..jig_fleet_world::LIMITS };
        let needs = fleet::Needs { message_bytes: 10, ..jig_fleet_world::fixtures::model_needs() };
        Self {
            domain: jig_fleet_world::fixtures::domain(&limits, needs),
            env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits },
        }
    }
    fn at_hello(&mut self, channel: u64, at: u64, worker: u32, bounds: fleet::Bounds, stop_bound: Duration) {
        self.env.wall = Wall::from_nanos(at);
        self.send(Event::Hello {
            channel: Token::new(channel),
            worker,
            hello: Hello { slots: 1, stop_bound, bounds, workstreams: Box::new([]), hosting: Box::new([]) },
        });
    }
    fn refused(&self, kinds: Kinds) -> Option<(HostKind, u64, HelloRefusal)> {
        self.domain.standing_refusal(kinds).map(|(kind, record)| (kind, record.at.as_nanos(), record.why))
    }
    fn short() -> fleet::Bounds {
        fleet::Bounds { message_bytes: 9, ..jig_fleet_world::fixtures::bounds() }
    }
    fn good() -> fleet::Bounds {
        jig_fleet_world::fixtures::bounds()
    }
    fn lost(&mut self, channel: u64) {
        self.send(Event::Lost { channel: Token::new(channel) });
    }
}

#[test]
fn standing_is_independent_of_history_capacity_eviction_and_fact_drains() {
    for capacity in [0, 1] {
        let mut f = Fleet::standing(capacity);
        assert_eq!(f.refused(Kinds::Both), None, "construction engine is not admitted");
        f.at_hello(1, 12, 0, Fleet::short(), Duration::ZERO);
        assert_eq!(
            f.refused(Kinds::Workers),
            Some((HostKind::Worker, 12, HelloRefusal::Agent { why: AgentShortfall::Message }))
        );
        f.at_hello(2, 18, 0, Fleet::good(), f.env.limits.grace);
        while f.domain.pop_fact().is_some() {}
        let expected = Some((HostKind::Worker, 18, HelloRefusal::StopBound { grace: f.env.limits.grace }));
        assert_eq!(f.refused(Kinds::Both), expected);
        assert_eq!(f.refused(Kinds::Engine), None);
        assert_eq!(f.domain.hello_refusals().len(), capacity);
        assert_eq!(f.domain.hello_history_lost(), 2 - u64::from(capacity));
        assert_eq!(f.refused(Kinds::Both), expected, "history reads do not drain standing facts");
        assert_eq!(Fleet::standing(capacity).refused(Kinds::Both), None, "restart clears volatile facts");
    }
}

#[test]
fn failed_engine_hello_is_not_live_and_reserved_repeats_do_not_replace_it() {
    let mut f = Fleet::standing(0);
    f.at_hello(1, 11, 0, Fleet::short(), Duration::ZERO);
    f.at_hello(0, 12, 0, Fleet::short(), Duration::ZERO);
    let engine = Some((HostKind::Engine, 12, HelloRefusal::Agent { why: AgentShortfall::Message }));
    assert_eq!(f.refused(Kinds::Both), engine);
    assert_eq!(f.refused(Kinds::Engine), engine);
    assert_eq!(f.refused(Kinds::Workers).expect("worker refusal stands").1, 11);
    f.at_hello(0, 30, 0, Fleet::good(), Duration::ZERO);
    assert_eq!(f.refused(Kinds::Both), engine, "ReservedChannel cannot overwrite the observed refusal");
    f.at_hello(2, 12, 0, Fleet::short(), Duration::ZERO);
    assert_eq!(f.refused(Kinds::Both), engine, "engine wins equal wall times deterministically");
    f.at_hello(3, 13, 0, Fleet::short(), Duration::ZERO);
    assert_eq!(f.refused(Kinds::Both).expect("allowed refusal stands").0, HostKind::Worker);
    assert_eq!(f.refused(Kinds::Engine), engine);
}

#[test]
fn admission_clears_only_its_kind_and_an_ineligible_host_suppresses_both() {
    let mut f = Fleet::standing(0);
    f.at_hello(0, 12, 0, Fleet::short(), Duration::ZERO);
    f.at_hello(1, 13, 0, Fleet::short(), Duration::ZERO);
    f.at_hello(2, 14, 0, Fleet::good(), Duration::ZERO);
    assert_eq!(f.refused(Kinds::Both), None, "admitted host suppresses even without requested models");
    assert!(
        f.domain
            .probe(
                Kinds::Workers,
                1,
                jig_domain_fleet::Version { charter: jig_fleet_world::fixtures::MODEL_CHARTER, revision: 1 },
                &jig_fleet_world::fixtures::model_needs().charters[0].models,
                &[]
            )
            .is_none()
    );
    assert_eq!(f.refused(Kinds::Workers), None);
    assert_eq!(
        f.refused(Kinds::Engine).expect("engine refusal stands").1,
        12,
        "wrong-kind admission never clears a fact"
    );
    f.at_hello(3, 15, 0, Fleet::good(), Duration::ZERO);
    f.lost(2);
    assert_eq!(f.refused(Kinds::Workers), Some((HostKind::Worker, 15, HelloRefusal::Position { worker: 0 })));
    f.at_hello(4, 16, 0, Fleet::good(), Duration::ZERO);
    f.lost(4);
    assert_eq!(f.refused(Kinds::Workers), None, "same-kind admission clears even after that host leaves");
}

#[test]
fn engine_admission_does_not_clear_a_worker_refusal() {
    let mut f = Fleet::standing(0);
    f.at_hello(1, 12, 0, Fleet::short(), Duration::ZERO);
    f.at_hello(0, 13, 0, Fleet::good(), Duration::ZERO);
    assert_eq!(f.refused(Kinds::Both), None);
    assert_eq!(f.refused(Kinds::Engine), None);
    assert_eq!(f.refused(Kinds::Workers).expect("worker refusal stands").1, 12);
}

#[test]
fn full_and_position_refusals_stand_but_a_dropped_duplicate_does_not_replace_them() {
    let mut f = Fleet::standing(1);
    f.at_hello(1, 10, 0, Fleet::good(), Duration::ZERO);
    f.at_hello(2, 11, 1, Fleet::good(), Duration::ZERO);
    let full = HelloRefusal::Workers { limit: 1 };
    assert_eq!(f.domain.hello_refusals().as_slice()[0].why, full);
    f.at_hello(1, 12, 0, Fleet::good(), Duration::ZERO);
    assert_eq!(
        f.domain.hello_refusals().as_slice()[0].why,
        full,
        "a dropped duplicate cannot manufacture another refusal"
    );
    f.lost(1);
    assert_eq!(f.refused(Kinds::Workers), Some((HostKind::Worker, 11, full)));
    f.at_hello(3, 13, f.env.limits.configured_workers, Fleet::good(), Duration::ZERO);
    assert_eq!(
        f.refused(Kinds::Workers).expect("worker refusal stands").2,
        HelloRefusal::Position { worker: f.env.limits.configured_workers }
    );
}

#[test]
fn a_busy_admitted_host_suppresses_until_it_drains() {
    let mut f = Fleet::standing(0);
    let bounds = jig_fleet_world::fixtures::model_bounds(&[b"main", b"alternative", b"other"]);
    f.at_hello(1, 10, 0, bounds, Duration::ZERO);
    f.start(jig_fleet_world::fixtures::MODEL_CHARTER);
    assert_eq!(f.settle(), [Token::new(1)]);
    f.at_hello(2, 11, 1, Fleet::good(), Duration::ZERO);
    assert_eq!(f.refused(Kinds::Workers), None, "occupied slots do not expose a standing refusal");
    f.send(Event::Answer {
        channel: Token::new(1),
        run: super::RUN,
        attempt: super::ATTEMPT,
        answer: fleet::Answer::Busy,
        payload: super::RUN,
    });
    assert_eq!(
        f.refused(Kinds::Workers).expect("worker refusal stands").1,
        11,
        "draining hosts no longer suppress refusal"
    );
}
