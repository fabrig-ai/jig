//! Random people over the real core world, with independently drawn fault doors.
//! Engine cuts use the actual store; client faults use the shared shell harness
//! (client/domain/testing.md, sections 5–7).
use super::{Joint, fixture};
use crate::{Fault, FrontFault, Outcome, Test, Until, World};
use skein_fake_person::{Face, Find, Found, Step, TreeFace};
use skein_lib::{Duration, Rng};
use skein_ui::Role;
use skein_world::domain::Verdict;

/// The real engine transaction fault reached in this seed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineFault {
    Held,
    Crash,
    Failed,
}
/// The real client seam reached in this seed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientFault {
    DropWatch,
    Slow,
    NoAnswer,
    Reload,
    DoublePress,
}
/// Typed evidence, independent of formatted traces.
#[derive(Debug)]
pub struct Reach {
    pub engine: EngineFault,
    pub client: ClientFault,
    pub submitted: u64,
    pub durable_answers: usize,
    pub acknowledged: u64,
}

#[must_use]
pub fn assigned(seed: u64) -> (EngineFault, ClientFault) {
    let mut rng = Rng::new(seed);
    let engine = match rng.below(3) {
        0 => EngineFault::Held,
        1 => EngineFault::Crash,
        2 => EngineFault::Failed,
        _ => unreachable!(),
    };
    let client = match rng.below(5) {
        0 => ClientFault::DropWatch,
        1 => ClientFault::Slow,
        2 => ClientFault::NoAnswer,
        3 => ClientFault::Reload,
        4 => ClientFault::DoublePress,
        _ => unreachable!(),
    };
    (engine, client)
}
fn configuration() -> jig_test_domain::Config {
    jig_core_world::world::config(8510)
}
fn answers(world: &mut World<Test, Joint>) -> usize {
    world
        .front()
        .session
        .driver
        .world
        .store
        .rows
        .values()
        .filter(|row| {
            matches!(
                row,
                jig_test_domain::Record::Core(jig_domain_core::Record::People(
                    jig_domain_people::Stored::Answer { .. }
                ))
            )
        })
        .count()
}
fn receiving() -> jig_client_test_domain::Limits {
    let mut limits = crate::test::limits();
    limits.client.domain.text = 128;
    limits.client.domain.turn_text = 128;
    limits.client.domain.turn_bytes = 128;
    limits.jig.text = 128;
    limits.first.text = 128;
    limits
}
fn client(seed: u64) -> (jig_client_test_domain::Domain, jig_client_test_view::View<'static>) {
    crate::test::new(&receiving(), seed)
}
fn ready(seed: u64) -> (World<Test, Joint>, crate::ClientId) {
    let mut world = fixture::seeded_world_with_receiving(
        seed,
        fixture::turn(),
        |front| front.restart_configuration(configuration),
        |limits| limits.core.views.watchers = 12,
        &receiving(),
    );
    world.randomness(8, 0, 0);
    let client = world.client_with(1, crate::stories::conversation(), receiving(), client);
    let within = Duration::from_secs(1);
    world.person(
        client,
        vec![
            Step::See { find: Find::exactly_named(Role::Button, b"Sign in with Test"), within },
            Step::Press { find: Find::exactly_named(Role::Button, b"Sign in with Test") },
            Step::See { find: Find::named(Role::TextArea, b"First words"), within },
            Step::Type {
                find: Find::named(Role::TextArea, b"First words"),
                words: format!("seed {seed}").into_bytes().into_boxed_slice(),
            },
            Step::Press { find: Find::exactly_named(Role::Button, b"Start a conversation") },
            Step::Pause { span: Duration::from_millis(450) },
            Step::See { find: Find::exactly_named(Role::Button, b"Stop"), within },
            Step::Press { find: Find::exactly_named(Role::Button, b"Stop") },
            Step::See { find: Find::text(b"Stop the live run and hold its task"), within },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "ready seed {seed}: {outcome:?}");
    assert_eq!(answers(&mut world), 1);
    (world, client)
}
fn pending(
    world: &mut World<Test, Joint>,
    client: crate::ClientId,
    engine: EngineFault,
    fault: ClientFault,
    slack: Duration,
) -> u64 {
    let applied = world.front().session.driver.world.store.applied;
    let submitted = applied + 1;
    if engine == EngineFault::Failed {
        world.front().session.driver.world.store.fault(jig_core_world::Fault::Fail { commit: submitted });
    } else {
        world.front().session.driver.world.cut = Some(jig_core_world::effects::Cut::Submitted(submitted));
    }
    if fault == ClientFault::DoublePress {
        let Found::Node(node) = TreeFace::new(world.tree(client)).find(&Find::exactly_named(Role::Button, b"Confirm"))
        else {
            panic!("the actual confirmation is visible")
        };
        world.fault(Fault::DoublePress { client, node });
    } else {
        world.person(client, vec![Step::Press { find: Find::exactly_named(Role::Button, b"Confirm") }]);
    }
    world.person(client, vec![Step::Pause { span: slack }]);
    let boundary = world.now().saturating_add(slack);
    let outcome = world.run(Until::Time(boundary));
    assert!(matches!(outcome.verdict, Verdict::Open { .. }), "pending {engine:?}/{fault:?}: {outcome:?}");
    assert_eq!(answers(world), 1);
    if engine == EngineFault::Failed {
        assert!(world.front().session.driver.world.stopped);
        assert_eq!(world.front().session.driver.world.store.applied, applied);
    } else {
        assert_eq!(
            world.front().session.driver.world.reached_cut,
            Some(jig_core_world::effects::Cut::Submitted(submitted))
        );
        assert!(world.front().session.driver.withhold_completion().expect("actual applied store").is_none());
        assert_eq!(answers(world), 2);
    }
    submitted
}

fn lost_words(world: &mut World<Test, Joint>, client: crate::ClientId, seed: u64, slack: Duration) {
    let words = format!("kept {seed}").into_bytes();
    let within = Duration::from_secs(1);
    world.person(
        client,
        vec![
            Step::Press { find: Find::exactly_named(Role::Button, b"Write") },
            Step::Type { find: Find::named(Role::TextArea, b"Your answer"), words: words.clone().into_boxed_slice() },
        ],
    );
    let ready = world.run(Until::Done);
    assert_eq!(ready.verdict, Verdict::Passed, "ready words {seed}: {ready:?}");
    let prior_answers = answers(world);
    let submitted = world.front().session.driver.world.store.applied + 1;
    world.front().session.driver.world.cut = Some(jig_core_world::effects::Cut::Submitted(submitted));
    world.person(
        client,
        vec![Step::Press { find: Find::exactly_named(Role::Button, b"Confirm") }, Step::Pause { span: slack }],
    );
    let boundary = world.now().saturating_add(slack);
    let pending = world.run(Until::Time(boundary));
    assert!(matches!(pending.verdict, Verdict::Open { .. }), "pending words seed {seed}: {pending:?}");
    assert_eq!(
        world.front().session.driver.world.reached_cut,
        Some(jig_core_world::effects::Cut::Submitted(submitted))
    );
    assert_eq!(answers(world), prior_answers);
    assert!(world.front().session.driver.withhold_completion().expect("real words commit").is_none());
    assert_eq!(answers(world), prior_answers + 1);
    let lost = world.front().session.lost_final_answers();
    world.fault(FrontFault::NoAnswer);
    world.front().session.driver.release_commits();
    let boundary = world.now().saturating_add(Duration::from_millis(10));
    let unanswered = world.run(Until::Time(boundary));
    assert!(matches!(unanswered.verdict, Verdict::Open { .. }));
    assert_eq!(world.front().session.lost_final_answers(), lost + 1, "the real final answer was dropped");
    // Recovery is a person's actual reload; a discarded reply alone cannot invent a reconnection.
    world.fault(Fault::Reload { client });
    world.person(client, vec![Step::See { find: Find::text(&words), within }]);
    let replayed = world.run(Until::Done);
    assert_eq!(replayed.verdict, Verdict::Passed, "replayed words {seed}: {replayed:?}");
    assert_eq!(answers(world), prior_answers + 1, "the original key is not made again");
    let matching = world
        .front()
        .session
        .driver
        .world
        .store
        .rows
        .values()
        .filter(|row| {
            matches!(row,
        jig_test_domain::Record::Core(jig_domain_core::Record::Tasks(jig_domain_tasks::Stored::Activity(line)))
        if matches!(&line.line, jig_domain_tasks::Line::Words { words: bytes, .. } if bytes.as_ref() == words.as_slice()))
        })
        .count();
    assert_eq!(matching, 1, "the exact sent words remain once in actual durable activity");
}

/// One seed keeps its original engine, worker, schedule, person and fault draws.
#[must_use]
pub fn world(seed: u64) -> (Outcome, Reach) {
    let (engine, client_fault) = assigned(seed);
    let mut rng = Rng::new(seed ^ 0x45_04);
    let slack = Duration::from_millis(rng.between(40, 60));
    let (mut world, client) = ready(seed);
    let submitted = pending(&mut world, client, engine, client_fault, slack);
    if client_fault == ClientFault::Reload {
        world.fault(Fault::Reload { client });
    }
    match engine {
        EngineFault::Held => world.front().session.driver.release_commits(),
        EngineFault::Crash | EngineFault::Failed => {
            world.fault(FrontFault::Restart { offline: Duration::from_millis(rng.between(120, 170)) });
        }
    }
    world
        .person(client, vec![Step::See { find: Find::text(b"Held by a placed hold"), within: Duration::from_secs(2) }]);
    let settled = world.run(Until::Done);
    assert_eq!(settled.verdict, Verdict::Passed, "recovered seed {seed}: {settled:?}");
    if client_fault == ClientFault::NoAnswer {
        lost_words(&mut world, client, seed, slack);
    }
    if client_fault == ClientFault::DropWatch {
        world.fault(FrontFault::DropWatch);
    }
    if client_fault == ClientFault::Slow {
        world.fault(FrontFault::Slow { span: Duration::from_millis(rng.between(10, 35)) });
    }
    world.random_person(client);
    let outcome = world.run(Until::Done);
    world.front().session.driver.referee.check().expect("every actual taken normal delivery reaches the engine");
    let acknowledged = world.front().session.driver.referee.acknowledgements();
    let durable_answers = answers(&mut world);
    assert!(acknowledged > 0 && durable_answers >= 2);
    assert!(world.front().session.driver.world.cancellations.iter().any(|(task, _)| *task == 1));
    (outcome, Reach { engine, client: client_fault, submitted, durable_answers, acknowledged })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn random_people_and_both_actual_fault_doors_keep_the_store_and_protocol_referees() {
        // Seed 4505 found a lost-answer schedule without genuine connection recovery.
        let (outcome, reach) = world(4505);
        assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");
        assert!(outcome.judged.0 > 0 && reach.acknowledged > 0);
        assert_eq!((reach.engine, reach.client), assigned(4505));
    }
}
