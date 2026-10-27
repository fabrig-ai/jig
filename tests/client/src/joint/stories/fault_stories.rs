//! Client faults cross the real protocol loop (testing.md, sections 5–7).
use super::*;

fn enable_tracing(world: &mut World<Test, Joint>) {
    world.front().session.driver.world.enable_tracing();
}

fn start() -> Vec<Step<jig_client_test_domain::Address>> {
    let within = Duration::from_secs(1);
    vec![
        Step::See { find: Find::exactly_named(Role::Button, b"Sign in with Test"), within },
        Step::Press { find: Find::exactly_named(Role::Button, b"Sign in with Test") },
        Step::See { find: Find::named(Role::TextArea, b"First words"), within },
        Step::Type { find: Find::named(Role::TextArea, b"First words"), words: Box::from(*b"Hello") },
        Step::Press { find: Find::exactly_named(Role::Button, b"Start a conversation") },
        Step::See { find: Find::text(b"Hello"), within },
        Step::Pause { span: Duration::from_millis(450) },
    ]
}

#[test]
fn a_draft_kept_across_a_reload_stays_unsent_on_the_actual_engine() {
    let mut world = super::world(super::turn());
    let client = world.client(1, crate::stories::conversation());
    let within = Duration::from_secs(1);
    let mut script = start();
    script.extend([
        Step::Press { find: Find::exactly_named(Role::Button, b"Write") },
        Step::Type { find: Find::named(Role::TextArea, b"Your answer"), words: Box::from(*b"Unsent draft") },
        Step::Pause { span: Duration::from_millis(30) },
        Step::Reload,
        Step::See { find: Find::exactly_named(Role::Button, b"Write"), within },
        Step::Press { find: Find::exactly_named(Role::Button, b"Write") },
    ]);
    world.person(client, script);
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed, "{outcome:?}");
    assert!(outcome.judged.0 > 0);
    assert!(world.kept(client).iter().any(|record| matches!(record,
        jig_client_test_domain::store::Record::Jig(jig_client_view::store::Record::Draft { words, .. })
        if words.bytes() == b"Unsent draft")));
    assert!(
        world
            .tree(client)
            .nodes()
            .iter()
            .any(|node| node.value.as_ref().is_some_and(|value| value.text.as_ref() == b"Unsent draft"))
    );
    let owner = &world.front().session.driver.world;
    assert_eq!(
        owner
            .store
            .rows
            .values()
            .filter(|record| matches!(
                record,
                jig_test_domain::Record::Core(jig_domain_core::Record::People(people::Stored::Answer { .. }))
            ))
            .count(),
        1
    );
    assert!(owner.peers.as_ref().expect("actual worker").workers[0].messages.is_empty());
}

pub(super) fn stop_ready() -> (World<Test, Joint>, crate::ClientId) {
    stop_ready_in(super::world(super::turn()))
}

fn stop_ready_in(mut world: World<Test, Joint>) -> (World<Test, Joint>, crate::ClientId) {
    let client = world.client(1, crate::stories::conversation());
    let within = Duration::from_secs(1);
    let mut script = start();
    script.extend([
        Step::Press { find: Find::exactly_named(Role::Button, b"Stop") },
        Step::See { find: Find::text(b"Stop the live run and hold its task"), within },
    ]);
    world.person(client, script);
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed);
    assert!(outcome.judged.0 > 0);
    (world, client)
}
pub(super) fn answers(world: &mut World<Test, Joint>) -> usize {
    world
        .front()
        .session
        .driver
        .world
        .store
        .rows
        .values()
        .filter(|record| {
            matches!(
                record,
                jig_test_domain::Record::Core(jig_domain_core::Record::People(people::Stored::Answer { .. }))
            )
        })
        .count()
}
fn shown(world: &World<Test, Joint>, client: crate::ClientId, words: &[u8]) -> bool {
    use skein_fake_person::{Face, Found, TreeFace};
    matches!(TreeFace::new(world.tree(client)).find(&Find::text(words)), Found::Node(_))
}

#[test]
fn a_held_actual_commit_keeps_the_stop_answer_pending_until_its_completion() {
    let (mut world, client) = stop_ready();
    let submitted = world.front().session.driver.world.store.applied.checked_add(1).expect("next transaction");
    world.front().session.driver.world.cut = Some(jig_core_world::effects::Cut::Submitted(submitted));
    world.person(
        client,
        vec![
            Step::Press { find: Find::exactly_named(Role::Button, b"Confirm") },
            Step::Pause { span: Duration::from_millis(30) },
        ],
    );
    let cut_boundary = world.now().saturating_add(Duration::from_millis(50));
    let outcome = world.run(Until::Time(cut_boundary));
    assert!(
        matches!(outcome.verdict, skein_world::domain::Verdict::Open { .. }),
        "the held request remains a real liveness obligation"
    );
    assert!(outcome.judged.0 > 0);
    assert_eq!(
        world.front().session.driver.world.reached_cut,
        Some(jig_core_world::effects::Cut::Submitted(submitted))
    );
    assert_eq!(answers(&mut world), 1, "only starting the chat is answered durably");
    assert!(!shown(&world, client, b"Held by a placed hold"));
    assert!(world.front().session.driver.withhold_completion().expect("actual store applied").is_none());
    assert_eq!(answers(&mut world), 2, "the stop is durable while its completion is withheld");
    assert!(!shown(&world, client, b"Held by a placed hold"), "durability alone cannot fabricate a callback");
    world.front().session.driver.release_commits();
    world
        .person(client, vec![Step::See { find: Find::text(b"Held by a placed hold"), within: Duration::from_secs(1) }]);
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed, "{outcome:?}");
    assert!(outcome.judged.0 > 0);
    assert_eq!(answers(&mut world), 2);
    assert!(world.front().session.driver.world.cancellations.iter().any(|(task, _)| *task == 1));
}

#[test]
fn a_failed_commit_stops_the_engine_and_the_person_sees_offline_before_keyed_recovery() {
    fn configuration() -> jig_test_domain::Config {
        jig_core_world::world::config(8510)
    }
    let (mut world, client) = stop_ready();
    world.front().restart_configuration(configuration);
    let (applied, rows) = {
        let owner = &world.front().session.driver.world;
        (owner.store.applied, owner.store.rows.clone())
    };
    world
        .front()
        .session
        .driver
        .world
        .store
        .fault(jig_core_world::Fault::Fail { commit: applied.checked_add(1).expect("next actual transaction") });
    world.person(
        client,
        vec![
            Step::Press { find: Find::exactly_named(Role::Button, b"Confirm") },
            Step::See { find: Find::text(b"Offline"), within: Duration::from_secs(1) },
            // A real person wake keeps the stopped-state observation before the pending ask deadline.
            Step::Pause { span: Duration::from_millis(250) },
        ],
    );
    let boundary = world.now().saturating_add(Duration::from_millis(250));
    let outcome = world.run(Until::Time(boundary));
    assert!(matches!(outcome.verdict, skein_world::domain::Verdict::Open { .. }));
    assert!(outcome.judged.0 > 0);
    assert!(shown(&world, client, b"Offline"));
    assert!(!shown(&world, client, b"Held by a placed hold"));
    let owner = &world.front().session.driver.world;
    assert!(owner.stopped);
    assert_eq!(owner.store.applied, applied);
    assert_eq!(owner.store.rows, rows);
    assert_eq!(answers(&mut world), 1);
    world.fault(crate::FrontFault::Restart { offline: Duration::from_millis(150) });
    world
        .person(client, vec![Step::See { find: Find::text(b"Held by a placed hold"), within: Duration::from_secs(1) }]);
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed, "{outcome:?}");
    assert!(outcome.judged.0 > 0);
    assert!(!world.front().session.driver.world.stopped);
    assert_eq!(answers(&mut world), 2, "the original keyed stop is made once after real recovery");
}

#[test]
fn a_reload_with_a_stop_pending_recovers_its_lost_committed_answer_by_the_original_key() {
    let (mut world, client) = stop_ready_in(super::bounded_world(
        super::turn(),
        |_| {},
        |limits| {
            // Old untaken flights remain owned until terminal; reload needs distinct new watches.
            limits.core.views.watchers = 12;
        },
    ));
    enable_tracing(&mut world);
    let submitted = world.front().session.driver.world.store.applied.checked_add(1).expect("next transaction");
    world.front().session.driver.world.cut = Some(jig_core_world::effects::Cut::Submitted(submitted));
    world.person(
        client,
        vec![
            Step::Press { find: Find::exactly_named(Role::Button, b"Confirm") },
            Step::Pause { span: Duration::from_millis(30) },
        ],
    );
    let boundary = world.now().saturating_add(Duration::from_millis(50));
    let outcome = world.run(Until::Time(boundary));
    assert!(matches!(outcome.verdict, skein_world::domain::Verdict::Open { .. }));
    assert_eq!(
        world.front().session.driver.world.reached_cut,
        Some(jig_core_world::effects::Cut::Submitted(submitted))
    );
    assert_eq!(answers(&mut world), 1);
    assert!(world.front().session.driver.withhold_completion().expect("actual durable commit").is_none());
    assert_eq!(answers(&mut world), 2);
    assert!(!shown(&world, client, b"Held by a placed hold"));
    world.fault(crate::FrontFault::NoAnswer);
    world.front().session.driver.release_commits();
    world.fault(crate::Fault::Reload { client });
    world
        .person(client, vec![Step::See { find: Find::text(b"Held by a placed hold"), within: Duration::from_secs(1) }]);
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed, "{outcome:?}");
    assert!(outcome.judged.0 > 0);
    assert_eq!(answers(&mut world), 2);
    let owner = &world.front().session.driver.world;
    assert_eq!(owner.cancellations.iter().filter(|(task, _)| *task == 1).count(), 1);
    let asks: Vec<_> = owner
        .trace
        .iter()
        .filter(|event| event.starts_with("input Core(People(Ask") && event.contains("ask: Stop"))
        .collect();
    assert!(asks.len() >= 2, "the pending stop is actually replayed after reload");
    let keys: std::collections::BTreeSet<_> = asks
        .iter()
        .map(|event| {
            let start = event.find("key: [").expect("actual request key");
            let end = event[start..].find("], ask:").expect("complete key") + start + 1;
            &event[start..end]
        })
        .collect();
    assert_eq!(keys.len(), 1);
}

#[test]
fn a_dropped_watch_and_slow_delivery_show_the_page_behind_before_the_held_chat_is_released() {
    let (mut world, client) = stop_ready_in(super::bounded_world(
        super::turn(),
        |_| {},
        |limits| {
            limits.core.views.watchers = 12;
        },
    ));
    let within = Duration::from_secs(1);
    world.person(
        client,
        vec![
            Step::Press { find: Find::exactly_named(Role::Button, b"Confirm") },
            Step::See { find: Find::text(b"Held by a placed hold"), within },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed, "{outcome:?}");
    assert_eq!(answers(&mut world), 2);
    world.fault(crate::FrontFault::DropWatch);
    world.fault(crate::FrontFault::Slow { span: Duration::from_millis(40) });
    world.person(
        client,
        vec![
            Step::See { find: Find::text(b"Behind: waiting for current facts"), within },
            Step::See { find: Find::exactly_named(Role::Button, b"Release"), within },
            Step::Press { find: Find::exactly_named(Role::Button, b"Release") },
            Step::Press { find: Find::exactly_named(Role::Button, b"Confirm") },
            Step::See { find: Find::text(b"Released"), within },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed, "{outcome:?}");
    assert!(outcome.judged.0 > 0);
    assert_eq!(outcome.faults, 2);
    assert_eq!(answers(&mut world), 3);
    let assignments = &world.front().session.driver.world.peers.as_ref().expect("actual worker").workers[0].seen;
    assert_eq!(assignments.len(), 2);
    assert_eq!(assignments[0].task, assignments[1].task);
    assert_ne!(assignments[0].attempt, assignments[1].attempt);
}

#[test]
fn a_crash_after_the_stop_commit_cold_restarts_offline_and_answers_the_same_key_once() {
    fn configuration() -> jig_test_domain::Config {
        jig_core_world::world::config(8510)
    }
    let (mut world, client) = stop_ready_in(super::bounded_world(
        super::turn(),
        |front| front.restart_configuration(configuration),
        |limits| limits.core.views.watchers = 12,
    ));
    enable_tracing(&mut world);
    let submitted = world.front().session.driver.world.store.applied.checked_add(1).expect("next transaction");
    world.front().session.driver.world.cut = Some(jig_core_world::effects::Cut::Submitted(submitted));
    world.person(
        client,
        vec![
            Step::Press { find: Find::exactly_named(Role::Button, b"Confirm") },
            Step::Pause { span: Duration::from_millis(30) },
        ],
    );
    let boundary = world.now().saturating_add(Duration::from_millis(50));
    let outcome = world.run(Until::Time(boundary));
    assert!(matches!(outcome.verdict, skein_world::domain::Verdict::Open { .. }));
    assert_eq!(
        world.front().session.driver.world.reached_cut,
        Some(jig_core_world::effects::Cut::Submitted(submitted))
    );
    assert_eq!(answers(&mut world), 1);
    assert!(world.front().session.driver.withhold_completion().expect("actual durable commit").is_none());
    assert_eq!(answers(&mut world), 2);
    assert!(!shown(&world, client, b"Held by a placed hold"));
    // Crash the actual root before that durable transaction's completion is delivered.
    world.fault(crate::FrontFault::Restart { offline: Duration::from_millis(150) });
    let within = Duration::from_secs(1);
    world.person(
        client,
        vec![
            Step::See { find: Find::text(b"Offline"), within },
            Step::See { find: Find::text(b"Held by a placed hold"), within },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed, "{outcome:?}");
    assert!(outcome.judged.0 > 0);
    assert_eq!(answers(&mut world), 2);
    let owner = &world.front().session.driver.world;
    assert!(!owner.stopped);
    let asks: Vec<_> = owner
        .trace
        .iter()
        .filter(|event| event.starts_with("input Core(People(Ask") && event.contains("ask: Stop"))
        .collect();
    assert!(asks.len() >= 2, "the committed stop is genuinely asked again after cold restart");
    let keys: std::collections::BTreeSet<_> = asks
        .iter()
        .map(|event| {
            let start = event.find("key: [").expect("actual request key");
            let end = event[start..].find("], ask:").expect("complete key") + start + 1;
            &event[start..end]
        })
        .collect();
    assert_eq!(keys.len(), 1);
}

#[test]
fn a_watch_ended_as_the_engine_stops_reaches_the_client_as_stopping() {
    let (mut world, client) = stop_ready();
    let now = world.now();
    let wall = skein_lib::Wall::from_nanos(now.as_nanos());
    world.front().session.driver.world.stop();
    let mut callbacks = world.front().session.poll(now, wall).expect("stopping watch terminals");
    callbacks.extend(world.front().session.poll(now, wall).expect("pending watch terminal"));
    assert!(callbacks.iter().any(|(owner, event)| matches!(event,
        jig_client_test_domain::Event::Client { event: jig_client::Event::Up { up: jig_client::Up::WatchEnded {
            end: jig_client_domain::contract::WatchEnd::Stopping, ..
        } } } if *owner == client)));
}
