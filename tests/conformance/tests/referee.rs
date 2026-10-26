use jig_conformance::{Harness, referee::Promise, scenarios::Scenario};
use jig_conformance_world::{Config, Testing, actions, negative::Fault};
#[global_allocator]
static HEAP: skein_world::domain::heap::Counting = skein_world::domain::heap::Counting;

fn world(fault: Fault) -> Harness<Testing> {
    let scenario = (fault == Fault::StaleVerdict).then_some(Scenario::ChangedJudge);
    Harness::new(Config::new(31, false, 2, scenario).broken(fault), 31)
}

#[test]
fn application_referee_catches_early_answers_and_redecided_replays() {
    use jig_conformance::referee as r;
    let answer = r::ApplicationAnswer {
        key: r::ApplicationKey { person: 7, key: [4; 16] },
        connector: 2,
        digest: [9; 32],
        token: 5,
        until: 1000,
        outcome: Box::from([1, 8]),
    };
    let request =
        || r::Observed::ApplicationRequested { reply: 600, key: answer.key, connector: 2, digest: answer.digest };
    let reply = || r::Observed::ApplicationAnswered { reply: 600, connector: 2, outcome: answer.outcome.clone() };
    let mut early = world(Fault::None).referee;
    early.observe(1, request()).expect("offered application");
    early
        .observe(2, r::Observed::ApplicationSubmitted { commit: 1, answer: answer.clone() })
        .expect("store submission");
    assert_eq!(early.observe(3, reply()).expect_err("the root answered before durability").promise, Promise::Commit);

    for token in [5, 6] {
        let mut repeated = world(Fault::None).referee;
        repeated.observe(1, request()).expect("offered application");
        repeated
            .observe(2, r::Observed::ApplicationSubmitted { commit: 1, answer: answer.clone() })
            .expect("store submission");
        repeated
            .observe(3, r::Observed::Durable { number: 1, snapshot: r::Snapshot::default() })
            .expect("whole commit");
        repeated.observe(4, reply()).expect("durable terminal");
        repeated.observe(5, r::Observed::Restart).expect("outside ledger survives restart");
        repeated.observe(6, request()).expect("same key offered again");
        repeated
            .observe(
                7,
                r::Observed::ApplicationSubmitted {
                    commit: 2,
                    answer: r::ApplicationAnswer { token, ..answer.clone() },
                },
            )
            .expect("faulty root routed replay to decide");
        assert_eq!(
            repeated
                .observe(8, r::Observed::Durable { number: 2, snapshot: r::Snapshot::default() })
                .expect_err("a retained key cannot be decided twice, even under a fresh token")
                .promise,
            Promise::Once
        );
    }
}

#[test]
fn application_referee_accepts_failed_commit_retry_expiry_and_identical_replay() {
    use jig_conformance::referee as r;
    let answer = r::ApplicationAnswer {
        key: r::ApplicationKey { person: 7, key: [4; 16] },
        connector: 2,
        digest: [9; 32],
        token: 5,
        until: 10,
        outcome: Box::from([1, 8]),
    };
    let request =
        || r::Observed::ApplicationRequested { reply: 600, key: answer.key, connector: 2, digest: answer.digest };
    let reply = || r::Observed::ApplicationAnswered { reply: 600, connector: 2, outcome: answer.outcome.clone() };
    let mut referee = world(Fault::None).referee;
    referee.observe(1, request()).expect("offered application");
    referee
        .observe(2, r::Observed::ApplicationSubmitted { commit: 1, answer: answer.clone() })
        .expect("undurable submission");
    referee.observe(3, r::Observed::Restart).expect("failed submission lost");
    referee.observe(4, request()).expect("retry after failed commit");
    referee
        .observe(4, r::Observed::ApplicationSubmitted { commit: 1, answer: answer.clone() })
        .expect("fresh decision");
    referee.observe(5, r::Observed::Durable { number: 1, snapshot: r::Snapshot::default() }).expect("durable retry");
    referee.observe(6, reply()).expect("durable terminal");
    referee.observe(7, request()).expect("retained replay");
    referee.observe(8, reply()).expect("identical replay needs no new decision");
    referee.observe(10, request()).expect("expired key can be used again");
    referee
        .observe(
            10,
            r::Observed::ApplicationSubmitted {
                commit: 2,
                answer: r::ApplicationAnswer { token: 6, until: 20, ..answer },
            },
        )
        .expect("new retention");
    referee
        .observe(10, r::Observed::Durable { number: 2, snapshot: r::Snapshot::default() })
        .expect("new durable decision");
    referee
        .observe(11, r::Observed::ApplicationAnswered { reply: 600, connector: 2, outcome: Box::from([1, 8]) })
        .expect("new terminal");
}

#[test]
fn every_broken_root_is_caught_from_durable_rows_and_outside_deliveries() {
    for (fault, promise) in [
        (Fault::EarlyOutput, Promise::Commit),
        (Fault::SplitDecision, Promise::Spend),
        (Fault::AdmissionAfterMutation, Promise::Spend),
        (Fault::WrongRestart, Promise::Order),
        (Fault::UncheckedEffect, Promise::Spend),
        (Fault::WrongKind, Promise::Authority),
    ] {
        let mut world = world(fault);
        let failure = if matches!(fault, Fault::EarlyOutput | Fault::WrongRestart) {
            world.drain().expect_err("the root corruption must be visible")
        } else {
            world.drain().expect("fault waits for an effect");
            actions::effect(&mut world, 1);
            world.drain().expect_err("the root corruption must be visible")
        };
        assert_eq!(failure.promise, promise, "{fault:?}: {failure:?}");
    }
}

#[test]
fn every_broken_connector_is_caught_from_fake_systems_and_durable_attempts() {
    for fault in [Fault::RepeatedEffect, Fault::StaleVerdict, Fault::ForgottenDeadline, Fault::DifferentDecision] {
        let mut world = world(fault);
        world.drain().expect("faithful startup");
        if fault == Fault::StaleVerdict {
            world.advance(61_000_000_000);
            world.drain().expect("the recorded judge fact ages past its freshness");
        }
        if matches!(fault, Fault::ForgottenDeadline | Fault::DifferentDecision) {
            world.peers.hold_writes = true;
        }
        actions::effect(&mut world, 1);
        let failure = if matches!(fault, Fault::ForgottenDeadline | Fault::DifferentDecision) {
            world.drain().expect("initial decision is faithful and uncertain");
            let (number, pending) = world.peers.pending_writes[0].clone();
            world.crash().expect("cold store retains the initial decision");
            world.drain().expect("reconstructed connector awaits the retained system response");
            let event = world.systems[usize::from(number - 1)].answer(pending, jig_test_system::Fault::Late);
            world.send(jig_test_domain::Event::Connector { number, event: jig_test_connector::Event::System(event) });
            world.drain().expect_err("a second step must preserve decision and deadline")
        } else {
            world.drain().expect_err("the connector corruption must be visible")
        };
        let promise = if fault == Fault::StaleVerdict { Promise::Authority } else { Promise::Once };
        assert_eq!(failure.promise, promise, "{fault:?}: {failure:?}");
    }
}

#[test]
fn the_same_boundaries_accept_a_faithful_root_and_connector_across_restart() {
    let mut world = world(Fault::None);
    world.drain().expect("faithful startup");
    world.peers.hold_writes = true;
    actions::effect(&mut world, 1);
    world.drain().expect("faithful initial decision");
    world.crash().expect("faithful cold store");
    world.drain().expect("deadline and decision survive");
    world.peers.hold_writes = false;
    world.advance(2_000_000_000);
    world.drain().expect("faithful retry");
    assert_eq!(world.systems[0].observed().iter().filter(|effect| effect.applied).count(), 1);
}

#[test]
fn a_commit_after_drained_is_a_violation() {
    use jig_conformance::referee as r;
    let mut referee = jig_core_world::observations::Observer::new(&jig_core_world::world::config(31)).referee;
    referee
        .observe(1, r::Observed::Durable { number: 1, snapshot: r::Snapshot::default() })
        .expect("durable evidence accepted");
    referee.observe(2, r::Observed::Drained { at: 1 }).expect("durable evidence accepted");
    let failure = referee
        .observe(3, r::Observed::Durable { number: 2, snapshot: r::Snapshot::default() })
        .expect_err("drain invariant must reject this observation");
    assert_eq!(failure.promise, Promise::Commit);
    assert_eq!(failure.why, "CommitAfterDrained { number: 2 }");
}

#[test]
fn a_drain_before_its_commit_is_durable_is_a_violation() {
    use jig_conformance::referee as r;
    let mut referee = jig_core_world::observations::Observer::new(&jig_core_world::world::config(31)).referee;
    let failure =
        referee.observe(1, r::Observed::Drained { at: 1 }).expect_err("drain invariant must reject this observation");
    assert_eq!(failure.promise, Promise::Commit);
}

#[test]
fn a_cold_store_that_loses_a_drained_party_answer_is_a_violation() {
    use jig_conformance::referee as r;
    let mut referee = jig_core_world::observations::Observer::new(&jig_core_world::world::config(31)).referee;
    let mut snapshot = r::Snapshot::default();
    snapshot.receipts.insert(r::Receipt::Party(7, [2; 16]));
    referee.observe(1, r::Observed::Durable { number: 1, snapshot }).expect("durable evidence accepted");
    referee.observe(2, r::Observed::Drained { at: 1 }).expect("durable evidence accepted");
    referee.observe(3, r::Observed::Restart).expect("durable evidence accepted");
    let failure = referee
        .observe(4, r::Observed::ColdStore { receipts: std::collections::BTreeSet::new() })
        .expect_err("drain invariant must reject this observation");
    assert_eq!(failure.promise, Promise::Commit);
    assert_eq!(failure.why, "cold store lost a drained receipt");
}

fn position_referee() -> jig_conformance::referee::Referee {
    jig_core_world::observations::Observer::new(&jig_core_world::world::config(31)).referee
}

fn position_snapshot(
    referee: &jig_conformance::referee::Referee,
    phase: jig_conformance::referee::Phase,
) -> jig_conformance::referee::Snapshot {
    use jig_conformance::referee as r;
    let mut snapshot = r::Snapshot::default();
    snapshot.tasks.insert(
        5,
        r::Task {
            settings: r::SettingsRevision::default(),
            spec_revision: 0,
            hold_revision: 0,
            tracked: false,
            number: 5,
            project: 1,
            source: r::Source::Deployment,
            scope: referee.policy.projects[&1].clone(),
            executor: (0, 1),
            budget: 0,
            spent: 0,
            spent_below: 0,
            reserved: 0,
            run_reserved: 0,
            run_spent: 0,
            attempt: 1,
            period: None,
            phase,
            dependencies: Vec::new(),
            delegates: Vec::new(),
            pools: Vec::new(),
            inbox: Vec::new(),
            proposal: None,
        },
    );
    snapshot.read_positions.insert((1, 5), 2);
    snapshot.read_positions.insert((2, 5), 7);
    snapshot
}

const POSITION_BOUNDARIES: [bool; 2] = [false, true];

#[test]
fn a_read_position_moving_back_across_an_end_is_a_violation() {
    use jig_conformance::referee as r;
    for restart in POSITION_BOUNDARIES {
        let mut referee = position_referee();
        let before = position_snapshot(&referee, r::Phase::Active);
        let mut after = position_snapshot(&referee, r::Phase::Ended);
        after.read_positions.insert((1, 5), 1);
        after.read_positions.insert((2, 5), 8);
        referee.observe(1, r::Observed::Durable { number: 1, snapshot: before }).expect("live position");
        if restart {
            referee.observe(2, r::Observed::Restart).expect("cold start");
        }
        let failure = referee.observe(3, r::Observed::Durable { number: 2, snapshot: after }).expect_err("regression");
        assert_eq!(failure.promise, Promise::Lost, "restart {restart}");
        assert_eq!(failure.why, "a retained task lost or moved back its party's read position", "restart {restart}");
    }
}

#[test]
fn a_read_position_erased_without_its_task_is_a_violation() {
    use jig_conformance::referee as r;
    let mut referee = position_referee();
    let before = position_snapshot(&referee, r::Phase::Ended);
    let mut after = before.clone();
    after.read_positions.remove(&(1, 5));
    referee.observe(1, r::Observed::Durable { number: 1, snapshot: before }).expect("ended position");
    let failure = referee.observe(2, r::Observed::Durable { number: 2, snapshot: after }).expect_err("lost position");
    assert_eq!(failure.promise, Promise::Lost);
    assert_eq!(failure.why, "a retained task lost or moved back its party's read position");
}

#[test]
fn a_read_position_kept_across_a_restart_passes() {
    use jig_conformance::referee as r;
    let mut referee = position_referee();
    let before = position_snapshot(&referee, r::Phase::Active);
    let after = position_snapshot(&referee, r::Phase::Ended);
    referee.observe(1, r::Observed::Durable { number: 1, snapshot: before }).expect("live position");
    referee.observe(2, r::Observed::Restart).expect("cold start retains seen evidence");
    referee.observe(3, r::Observed::Durable { number: 2, snapshot: after }).expect("same positions after task end");
}

#[test]
fn a_task_and_its_read_positions_can_be_forgotten_together() {
    use jig_conformance::referee as r;
    let mut referee = position_referee();
    let before = position_snapshot(&referee, r::Phase::Ended);
    referee.observe(1, r::Observed::Durable { number: 1, snapshot: before }).expect("ended positions");
    referee.observe(2, r::Observed::Durable { number: 2, snapshot: r::Snapshot::default() }).expect("atomic forget");
}
