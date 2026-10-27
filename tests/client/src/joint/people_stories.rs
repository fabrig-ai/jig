//! People act on the view; the engine and workers decide the outcomes
//! (domain/people.md, 11; domain/engine.md, 15).
use super::*;

#[test]
fn a_person_s_words_reach_a_running_run() {
    let mut world = super::world(super::turn());
    let client = world.client(1, crate::stories::conversation());
    let within = Duration::from_secs(1);
    world.person(
        client,
        vec![
            Step::See { find: Find::named(Role::Button, b"Sign in with Test"), within },
            Step::Press { find: Find::named(Role::Button, b"Sign in with Test") },
            Step::See { find: Find::named(Role::TextArea, b"First words"), within },
            Step::Type { find: Find::named(Role::TextArea, b"First words"), words: b"Hello".as_slice().into() },
            Step::Press { find: Find::named(Role::Button, b"Start a conversation") },
            Step::See { find: Find::text(b"Hello"), within },
            Step::Pause { span: Duration::from_millis(450) },
            Step::See { find: Find::named(Role::Button, b"Write"), within },
            Step::Press { find: Find::named(Role::Button, b"Write") },
            Step::Type {
                find: Find::named(Role::TextArea, b"Your answer"),
                words: b"Please continue".as_slice().into(),
            },
            Step::Press { find: Find::named(Role::Button, b"Confirm") },
            Step::See { find: Find::text(b"Please continue"), within },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed);
    assert!(outcome.judged.0 > 0);
    super::offered_acts::task(&mut world, client, 1);
    let peers = world.front().session.driver.world.peers.as_ref().expect("worker peer");
    let assigned = peers.workers[0].seen.first().expect("assigned chat");
    assert!(peers.workers[0].messages.iter().any(|(_, task, attempt, words)| *task == assigned.task
        && *attempt == assigned.attempt
        && words.as_ref() == b"Please continue"));
    assert_eq!(
        world
            .front()
            .session
            .driver
            .world
            .store
            .rows
            .values()
            .filter(|row| matches!(
                row,
                jig_test_domain::Record::Core(jig_domain_core::Record::People(people::Stored::Answer { .. }))
            ))
            .count(),
        2
    );
}

#[test]
fn a_person_stops_a_run_and_releases_it() {
    let mut world = super::world(super::turn());
    let client = world.client(1, crate::stories::conversation());
    let within = Duration::from_secs(1);
    world.person(
        client,
        vec![
            Step::See { find: Find::named(Role::Button, b"Sign in with Test"), within },
            Step::Press { find: Find::named(Role::Button, b"Sign in with Test") },
            Step::See { find: Find::named(Role::TextArea, b"First words"), within },
            Step::Type { find: Find::named(Role::TextArea, b"First words"), words: b"Hello".as_slice().into() },
            Step::Press { find: Find::named(Role::Button, b"Start a conversation") },
            Step::See { find: Find::text(b"Hello"), within },
            Step::Pause { span: Duration::from_millis(450) },
            Step::See { find: Find::named(Role::Button, b"Stop"), within },
            Step::Press { find: Find::named(Role::Button, b"Stop") },
            Step::See { find: Find::text(b"Stop the live run and hold its task"), within },
            Step::Press { find: Find::named(Role::Button, b"Confirm") },
            Step::See { find: Find::text(b"Held by a placed hold"), within },
            Step::See { find: Find::exactly_named(Role::Button, b"Release"), within },
            Step::Press { find: Find::exactly_named(Role::Button, b"Release") },
            Step::Press { find: Find::named(Role::Button, b"Confirm") },
            Step::See { find: Find::text(b"Released"), within },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed);
    assert!(outcome.judged.0 > 0);
    super::offered_acts::task(&mut world, client, 1);
    let peers = world.front().session.driver.world.peers.as_ref().expect("worker peer");
    assert_eq!(peers.workers[0].seen.len(), 2);
    let before = &peers.workers[0].seen[0];
    let after = &peers.workers[0].seen[1];
    assert_eq!(before.task, after.task);
    assert_ne!(before.attempt, after.attempt);
    assert_eq!(
        world
            .front()
            .session
            .driver
            .world
            .store
            .rows
            .values()
            .filter(|row| matches!(
                row,
                jig_test_domain::Record::Core(jig_domain_core::Record::People(people::Stored::Answer { .. }))
            ))
            .count(),
        3
    );
}

#[test]
fn a_request_sent_twice_is_made_once_across_an_engine_restart() {
    use skein_fake_person::{Face, Found, TreeFace};
    fn configuration() -> jig_test_domain::Config {
        jig_core_world::world::config(8510)
    }
    let mut world = super::configured_world(super::turn(), |front| front.restart_configuration(configuration));
    let client = world.client(1, crate::stories::conversation());
    let within = Duration::from_secs(1);
    world.front().session.driver.world.enable_tracing();
    world.person(
        client,
        vec![
            Step::See { find: Find::named(Role::Button, b"Sign in with Test"), within },
            Step::Press { find: Find::named(Role::Button, b"Sign in with Test") },
            Step::See { find: Find::named(Role::TextArea, b"First words"), within },
            Step::Type { find: Find::named(Role::TextArea, b"First words"), words: b"Hello".as_slice().into() },
            Step::Press { find: Find::named(Role::Button, b"Start a conversation") },
            Step::See { find: Find::text(b"Hello"), within },
            Step::Pause { span: Duration::from_millis(450) },
            Step::See { find: Find::exactly_named(Role::Button, b"Write"), within },
            Step::Press { find: Find::exactly_named(Role::Button, b"Write") },
            Step::Type {
                find: Find::named(Role::TextArea, b"Your answer"),
                words: b"Once after restart".as_slice().into(),
            },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed);
    let confirm = match TreeFace::new(world.tree(client)).find(&Find::exactly_named(Role::Button, b"Confirm")) {
        Found::Node(id) => id,
        Found::Absent | Found::Pending => panic!("person's ready confirmation"),
    };
    world.fault(crate::Fault::DoublePress { client, node: confirm });
    world.fault(crate::FrontFault::NoAnswer);
    world.fault(crate::FrontFault::Restart { offline: Duration::from_millis(50) });
    world.person(client, vec![Step::See { find: Find::text(b"Once after restart"), within }]);
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed);
    assert!(outcome.judged.0 > 0);
    let owner = &world.front().session.driver.world;
    assert_eq!(
        owner
            .store
            .rows
            .values()
            .filter(|row| matches!(
                row,
                jig_test_domain::Record::Core(jig_domain_core::Record::People(people::Stored::Answer { .. }))
            ))
            .count(),
        2
    );
    let peers = owner.peers.as_ref().expect("actual worker across restart");
    assert_eq!(
        peers.workers[0].messages.iter().filter(|(_, _, _, words)| words.as_ref() == b"Once after restart").count(),
        1
    );
    let words = format!("words: {:?}", b"Once after restart");
    let asks: Vec<_> = owner
        .trace
        .iter()
        .filter(|event| {
            event.starts_with("input Core(People(Ask") && event.contains("ask: Say") && event.contains(&words)
        })
        .collect();
    assert!(asks.len() >= 2, "the actual Say must reach the engine again after restart");
    let keys: std::collections::BTreeSet<_> = asks
        .iter()
        .map(|event| {
            let start = event.find("key: [").expect("actual request key");
            let end = event[start..].find("], ask:").expect("whole key followed by ask") + start + 1;
            &event[start..end]
        })
        .collect();
    assert_eq!(keys.len(), 1, "every actual retry preserves the original request key");
}

#[test]
fn a_hold_moved_under_the_open_confirmation_is_refused_in_place() {
    use skein_fake_person::{Face, Found, TreeFace};
    let mut world = super::configured_world(super::turn(), |front| {
        front.session.sources_at(vec![(
            Time::from_nanos(650_000_000),
            jig_test_domain::Event::Core(jig_domain_core::Event::Tasks(jig_domain_tasks::Event::Hold {
                task: 1,
                why: jig_domain_tasks::Hold::Budget,
            })),
        )]);
    });
    let client = world.client(1, crate::stories::conversation());
    let within = Duration::from_secs(1);
    world.person(
        client,
        vec![
            Step::See { find: Find::named(Role::Button, b"Sign in with Test"), within },
            Step::Press { find: Find::named(Role::Button, b"Sign in with Test") },
            Step::See { find: Find::named(Role::TextArea, b"First words"), within },
            Step::Type { find: Find::named(Role::TextArea, b"First words"), words: b"Hello".as_slice().into() },
            Step::Press { find: Find::named(Role::Button, b"Start a conversation") },
            Step::See { find: Find::text(b"Hello"), within },
            Step::Pause { span: Duration::from_millis(450) },
            Step::Press { find: Find::exactly_named(Role::Button, b"Stop") },
            Step::Press { find: Find::exactly_named(Role::Button, b"Confirm") },
            Step::See { find: Find::text(b"Held by a placed hold"), within },
            Step::See { find: Find::exactly_named(Role::Button, b"Release"), within },
            Step::Press { find: Find::exactly_named(Role::Button, b"Release") },
            Step::Pause { span: Duration::from_millis(250) },
            Step::See { find: Find::text(b"The revision this action names changed"), within },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed);
    assert!(outcome.judged.0 > 0);
    let mut face = TreeFace::new(world.tree(client));
    let confirm = match face.find(&Find::exactly_named(Role::Button, b"Confirm")) {
        Found::Node(id) => id,
        Found::Absent | Found::Pending => panic!("changed confirmation stays in place"),
    };
    assert!(!face.allows(confirm, &skein_ui::Act::Press), "the moved revision disables Confirm");
    world.person(
        client,
        vec![
            Step::Press { find: Find::exactly_named(Role::Button, b"Dismiss") },
            Step::See { find: Find::text(b"Held budget"), within },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed);
    assert!(outcome.judged.0 > 0);
    let owner = &world.front().session.driver.world;
    assert_eq!(owner.peers.as_ref().expect("actual worker").workers[0].seen[0].task, 1);
    assert_eq!(owner.peers.as_ref().expect("actual worker").workers[0].seen.len(), 1);
    assert!(owner.store.rows.values().any(|row| matches!(row,
        jig_test_domain::Record::Core(jig_domain_core::Record::Tasks(jig_domain_tasks::Stored::Live(task)))
        if task.number == 1 && task.revisions.hold == 2 && matches!(task.phase,
            jig_domain_tasks::Phase::Held { why: jig_domain_tasks::Hold::Budget, .. }))));
    assert_eq!(
        owner
            .store
            .rows
            .values()
            .filter(|row| matches!(
                row,
                jig_test_domain::Record::Core(jig_domain_core::Record::People(people::Stored::Answer { .. }))
            ))
            .count(),
        2
    );
}

#[test]
fn a_hold_moved_in_the_engine_is_refused_as_moved_and_shown_current() {
    let mut world = super::world(super::turn());
    let client = world.client(1, crate::stories::conversation());
    let within = Duration::from_secs(1);
    world.person(
        client,
        vec![
            Step::See { find: Find::named(Role::Button, b"Sign in with Test"), within },
            Step::Press { find: Find::named(Role::Button, b"Sign in with Test") },
            Step::See { find: Find::named(Role::TextArea, b"First words"), within },
            Step::Type { find: Find::named(Role::TextArea, b"First words"), words: b"Hello".as_slice().into() },
            Step::Press { find: Find::named(Role::Button, b"Start a conversation") },
            Step::See { find: Find::text(b"Hello"), within },
            Step::Pause { span: Duration::from_millis(450) },
            Step::Press { find: Find::exactly_named(Role::Button, b"Stop") },
            Step::Press { find: Find::exactly_named(Role::Button, b"Confirm") },
            Step::See { find: Find::text(b"Held by a placed hold"), within },
            Step::See { find: Find::exactly_named(Role::Button, b"Release"), within },
            Step::Press { find: Find::exactly_named(Role::Button, b"Release") },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed);
    assert!(outcome.judged.0 > 0);
    let moved_at = world.now().saturating_add(Duration::from_millis(20));
    world.front().session.sources_at(vec![(
        moved_at,
        jig_test_domain::Event::Core(jig_domain_core::Event::Tasks(jig_domain_tasks::Event::Hold {
            task: 1,
            why: jig_domain_tasks::Hold::Budget,
        })),
    )]);
    // Hold back deliveries, preserving the person's old revision while the
    // actual engine admits the scheduled new hold and judges the release.
    world.fault(crate::FrontFault::Slow { span: Duration::from_millis(80) });
    world.person(
        client,
        vec![
            Step::Pause { span: Duration::from_millis(40) },
            Step::Press { find: Find::exactly_named(Role::Button, b"Confirm") },
            Step::See { find: Find::text(b"The hold or its reason changed"), within },
            Step::See { find: Find::text(b"Held budget"), within },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed);
    assert!(outcome.judged.0 > 0);
    super::offered_acts::task(&mut world, client, 1);
    let owner = &world.front().session.driver.world;
    assert_eq!(owner.peers.as_ref().expect("actual worker").workers[0].seen.len(), 1);
    assert!(owner.store.rows.values().any(|row| matches!(row,
        jig_test_domain::Record::Core(jig_domain_core::Record::Tasks(jig_domain_tasks::Stored::Live(task)))
        if task.number == 1 && task.revisions.hold == 2 && matches!(task.phase,
            jig_domain_tasks::Phase::Held { why: jig_domain_tasks::Hold::Budget, .. }))));
    assert_eq!(
        owner
            .store
            .rows
            .values()
            .filter(|row| matches!(
                row,
                jig_test_domain::Record::Core(jig_domain_core::Record::People(people::Stored::Answer { .. }))
            ))
            .count(),
        3
    );
    assert!(owner.store.rows.values().any(|row| matches!(row,
    jig_test_domain::Record::Core(jig_domain_core::Record::People(people::Stored::Answer { outcome, .. }))
    if matches!(outcome, people::Outcome::Refused(people::Refusal::Moved {
        now: people::Moved::Hold { revision: 2, reason: people::HoldReason::Budget },
    })))));
}

fn signed_in_at_the_frame() -> (World<Test, Joint>, crate::harness::ClientId) {
    let mut world = super::world(super::turn());
    let client = world.client(1, crate::stories::inbox());
    world.person(
        client,
        vec![
            Step::See { find: sign_in_button(), within: Duration::from_secs(1) },
            Step::Press { find: sign_in_button() },
            Step::See { find: sign_out_button(), within: Duration::from_secs(1) },
        ],
    );
    assert_eq!(world.run(Until::Done).verdict, skein_world::domain::Verdict::Passed);
    (world, client)
}
fn sign_out_button() -> Find {
    Find::named(Role::Button, b"Sign out")
}
#[test]
fn a_person_signs_out_and_a_later_request_is_answered_signed_out() {
    use crate::joint::transport::{KeyedOwner, Reply};
    let (mut world, client) = signed_in_at_the_frame();
    let sign_in = session_sign_in(&world.front().session, client).expect("real sign-in");
    world.person(
        client,
        vec![
            Step::Press { find: sign_out_button() },
            Step::See { find: sign_in_button(), within: Duration::from_secs(1) },
        ],
    );
    assert_eq!(world.run(Until::Done).verdict, skein_world::domain::Verdict::Passed);
    let now = world.now();
    let attempt = Token::new(999);
    let key = [99; 16];
    let session = &mut world.front().session;
    assert_eq!(session_sign_in(session, client), None);
    let reply_to = session.driver.transport.reply_to(Reply::Keyed { client, attempt, key, owner: KeyedOwner::Jig });
    session.driver.send(
        now,
        super::clock(now),
        jig_test_domain::Event::Core(jig_domain_core::Event::People(people::Event::Ask {
            models: Box::new([]),
            reply_to,
            sign_in,
            key,
            ask: people::Ask::Stop { project: 1, task: u64::MAX },
        })),
    );
    let callbacks = session.poll(now, super::clock(now)).expect("actual engine terminal");
    assert!(callbacks.iter().any(|(owner, event)| *owner == client && matches!(event,
        jig_client_test_domain::Event::Client { event: jig_client::Event::Up { up: jig_client::Up::Answer { attempt: actual, envelope: skein_lib::RequestEnvelope::SignedOut, answer: None, why: None } } } if *actual == attempt)));
    assert!(session.driver.world.store.rows.values().all(|row| !matches!(row,
        jig_test_domain::Record::Core(jig_domain_core::Record::People(people::Stored::SignIn { number, .. })) if *number == sign_in)));
}

fn sign_in_button() -> Find {
    Find::named(Role::Button, b"Sign in with Test")
}

fn session_sign_in(session: &Session, client: crate::harness::ClientId) -> Option<u64> {
    session.driver.transport.sign_in(client)
}
