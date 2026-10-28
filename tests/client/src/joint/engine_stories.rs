#![expect(clippy::wildcard_enum_match_arm, reason = "ordinary story fixtures reject every unexpected public result")]
//! Genuine engine acts feed the root; the person's corrections use the view
//! (domain/engine.md, 15; client/domain/testing.md, 5).
use super::*;

pub(super) fn action(
    world: &mut World<Test, Joint>,
    assignment: &jig_fake_workers::Assignment,
    completion: u32,
    action: jig_domain_core::NamedAction,
) {
    let now = world.now();
    world.front().session.driver.send(
        now,
        clock(now),
        jig_test_domain::Event::Core(jig_domain_core::Event::NamedAction {
            to: skein_lib::ReplyTo::new(Token::new(9_900 + u64::from(completion))),
            key: jig_domain_core::CallKey {
                task: assignment.task,
                attempt: assignment.attempt,
                completion,
                position: 0,
            },
            action,
        }),
    );
}

#[test]
fn a_note_written_corrected_by_a_person_and_recalled() {
    let mut world = configured(8517, vec![Box::new([Script::Wait])], |configuration, owner| {
        let mut rules = configuration.core.defaults.rules.clone();
        rules.ceiling.notes = jig_domain_authority::Scopes::PROJECT;
        let mut policy = configuration.core.seed_projects[0].policy.policy.clone();
        policy.ceiling.notes = jig_domain_authority::Scopes::PROJECT;
        policy.roles[2].authority.notes = jig_domain_authority::Scopes::PROJECT;
        let mut checked = jig_domain_authority::Domain::new(rules, owner.core.authority).expect("bounded note policy");
        let mut out = skein_lib::Queue::with_capacity(jig_domain_authority::POLICY_MAX_OUT);
        jig_domain_authority::step(&mut checked, jig_domain_authority::Event::Policy { project: 1, policy }, &mut out);
        assert_eq!(out.pop(), Some(jig_domain_authority::PolicyFact::Added { project: 1 }));
        configuration.core.seed_projects[0].policy.policy = checked.policy(1).expect("checked seed policy").clone();
        configuration.core.defaults.rules =
            (jig_domain_authority::Domain::new(checked.rules().clone(), *checked.limits()).expect("checked rules"))
                .rules()
                .clone();
    });
    let member = world.client_with(3, crate::stories::conversation(), receiving_limits(), client);
    world.person(member, start(b"remember and correct"));
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed);
    assert!(outcome.judged.0 > 0);
    let assignment =
        world.front().session.driver.world.peers.as_ref().expect("actual worker").workers[0].seen[0].clone();
    action(
        &mut world,
        &assignment,
        1,
        jig_domain_core::NamedAction::Note {
            entry: jig_domain_notes::New {
                name: 0,
                scope: jig_domain_notes::Scope::Project { project: 1 },
                description: b"original".as_slice().into(),
                body: b"first body".as_slice().into(),
                references: skein_lib::List::with_capacity(1),
                author: jig_domain_notes::Author::Task { task: assignment.task, attempt: assignment.attempt },
            },
            recalled: None,
        },
    );
    let name = match world.front().session.driver.world.answers.last().expect("committed note answer").2 {
        jig_domain_core::CallPart::NoteWritten { name, revision: 1 } => name,
        ref other => panic!("note write must be admitted: {other:?}"),
    };
    let within = Duration::from_secs(1);
    world.person(
        member,
        vec![
            Step::Go {
                address: jig_client_test_domain::Address::Jig(jig_client_view::address::Address::Project {
                    project: 1,
                    section: jig_client_view::address::ProjectSection::Notes,
                }),
            },
            Step::See { find: Find::text(b"original"), within },
            Step::See { find: Find::exactly_named(Role::Button, b"Edit note"), within },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed);
    assert!(outcome.judged.0 > 0);
    super::super::offered_acts::card(
        &mut world,
        member,
        client::ObjectKey::Note { project: 1, note: name },
        b"original",
    );
    world.person(
        member,
        vec![
            Step::Press { find: Find::exactly_named(Role::Button, b"Edit note") },
            Step::Type {
                find: Find::named(Role::TextArea, b"Note description"),
                words: b"corrected".as_slice().into(),
            },
            Step::Type { find: Find::named(Role::TextArea, b"Note body"), words: b"second body".as_slice().into() },
            Step::Press { find: Find::exactly_named(Role::Button, b"Confirm") },
            Step::See { find: Find::text(b"Note edited"), within },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed);
    assert!(outcome.judged.0 > 0);
    let actual = world.front().session.driver.world.peers.as_ref().expect("actual member").parties[2]
        .person
        .expect("member sign-in");
    assert!(world.front().session.driver.world.store.rows.values().any(|row| matches!(row,
        jig_test_domain::Record::Core(jig_domain_core::Record::Notes(jig_domain_notes::Record::Entry(entry)))
        if entry.name == name && entry.revision == 2 && entry.body.as_ref() == b"second body"
        && entry.author == jig_domain_notes::Author::Party { party: actual })));
    action(
        &mut world,
        &assignment,
        2,
        jig_domain_core::NamedAction::Recall { by: jig_domain_notes::Recall::Name { name }, page: 0 },
    );
    assert!(matches!(world.front().session.driver.world.answers.last(),
        Some((_, _, jig_domain_core::CallPart::NoteRecalled { entries, more: false }))
        if entries.len() == 1 && entries[0].revision == 2 && entries[0].body.as_ref() == b"second body"));
}

#[test]
#[expect(clippy::too_many_lines, reason = "one ordered engine/person escalation story retains its assertions together")]
fn a_delegate_held_past_its_tries_is_escalated_to_a_person_and_released() {
    use jig_domain_tasks as tasks;
    use jig_test_domain as root;
    let mut world = configured_with_slots(
        8518,
        vec![
            Box::new([
                Script::Silence { for_: Duration::from_secs(3) },
                Script::Finish { report: b"parent done".as_slice().into() },
            ]),
            Box::new([Script::Fail(jig_fake_workers::Failure::Run)]),
            Box::new([Script::Fail(jig_fake_workers::Failure::Run)]),
            Box::new([Script::Finish { report: b"delegate recovered".as_slice().into() }]),
        ],
        2,
        |_, _| {},
    );
    let member = world.client_with(1, crate::stories::conversation(), receiving_limits(), client);
    world.person(member, start(b"release the exhausted delegate"));
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed);
    assert!(outcome.judged.0 > 0);
    let assignment =
        world.front().session.driver.world.peers.as_ref().expect("actual worker").workers[0].seen[0].clone();
    let owner = &world.front().session.driver.world;
    let mut authority =
        match owner.store.rows.get(&root::Key::Core(jig_domain_core::Key::Tasks(tasks::Key::Live(assignment.task)))) {
            Some(root::Record::Core(jig_domain_core::Record::Tasks(tasks::Stored::Live(row)))) => row.authority.clone(),
            other => panic!("actual parent authority: {other:?}"),
        };
    authority.budget.spend = 20;
    authority.delegation = tasks::Delegation { kinds: Box::new([]), tasks: 0, depth: 0 };
    let now = world.now();
    world.front().session.driver.send(
        now,
        clock(now),
        root::Event::Core(jig_domain_core::Event::DelegateValidated {
            to: skein_lib::ReplyTo::new(Token::new(9801)),
            key: jig_domain_core::CallKey {
                task: assignment.task,
                attempt: assignment.attempt,
                completion: 1,
                position: 0,
            },
            batch: Box::new([jig_domain_core::Delegate {
                deadline: jig_domain_tasks::Deadline::Named,
                label: 0,
                flags: jig_domain_tasks::Flags::PLAIN,
                tracked: None,
                budget: jig_domain_people::Budget::Named,
                executor: tasks::Executor::Agent { charter: 1 },
                spec: tasks::Spec {
                    asks: false,
                    words: b"retry this delegate".as_slice().into(),
                    parameters: Box::new([]),
                    inputs: Box::new([]),
                },
                contract: tasks::Contract::Report { words: 128 },
                authority,
                llm: None,
                symbolic_grants: Box::new([]),
                dependencies: Box::new([]),
                wake: tasks::WakePolicy::DEFAULT,
            }]),
            stubs: Box::new([]),
        }),
    );
    let child = match &world.front().session.driver.world.answers.last().expect("delegate answer").2 {
        jig_domain_core::CallPart::Delegated(numbers) => numbers[0].task,
        other => panic!("delegation must be admitted: {other:?}"),
    };
    world.person(member, vec![Step::Pause { span: Duration::from_millis(2100) }]);
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed);
    assert!(outcome.judged.0 > 0);
    let revision = match world
        .front()
        .session
        .driver
        .world
        .store
        .rows
        .get(&root::Key::Core(jig_domain_core::Key::Tasks(tasks::Key::Live(child))))
    {
        Some(root::Record::Core(jig_domain_core::Record::Tasks(tasks::Stored::Live(row)))) => {
            assert!(matches!(row.phase, tasks::Phase::Held { why: tasks::Hold::Failures(tasks::Class::Run), .. }));
            match row.escalation {
                tasks::Escalation::Waiting { revision, holder: tasks::EscalationHolder::Task(parent), .. }
                    if parent == assignment.task =>
                {
                    revision
                }
                ref other => panic!("the parent receives the actual exhausted hold first: {other:?}"),
            }
        }
        other => panic!("held delegate: {other:?}"),
    };
    action(
        &mut world,
        &assignment,
        2,
        jig_domain_core::NamedAction::DecideEscalation {
            task: child,
            revision,
            choice: jig_domain_core::EscalationChoice::Pass,
        },
    );
    let requester = world.front().session.driver.world.peers.as_ref().expect("actual owner").parties[0]
        .person
        .expect("owner sign-in");
    assert!(
        matches!(world.front().session.driver.world.store.rows.get(&root::Key::Core(jig_domain_core::Key::Tasks(tasks::Key::Live(child)))),
        Some(root::Record::Core(jig_domain_core::Record::Tasks(tasks::Stored::Live(row))))
        if matches!(row.escalation, tasks::Escalation::Waiting { holder: tasks::EscalationHolder::Person(person), .. } if person == requester))
    );
    let within = Duration::from_secs(1);
    world.person(
        member,
        vec![
            Step::Go { address: crate::stories::inbox() },
            Step::See { find: Find::text(b"retry this delegate"), within },
            Step::See { find: Find::text(b"Failure tries exhausted"), within },
            Step::See { find: Find::text(b"Run tries: 2"), within },
            Step::See { find: Find::exactly_named(Role::Button, b"Release"), within },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed);
    assert!(outcome.judged.0 > 0);
    super::super::offered_acts::card(&mut world, member, client::ObjectKey::Escalation(child), b"Escalation");
    world.person(
        member,
        vec![
            Step::Press { find: Find::exactly_named(Role::Button, b"Release") },
            Step::Press { find: Find::exactly_named(Role::Button, b"Confirm") },
            Step::See { find: Find::text(b"Released"), within },
            Step::Pause { span: Duration::from_secs(1) },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed, "{outcome:?}");
    assert!(outcome.judged.0 > 0);
    let owner = &world.front().session.driver.world;
    for (task, words) in [(child, b"delegate recovered".as_slice()), (assignment.task, b"parent done".as_slice())] {
        assert!(matches!(owner.store.rows.get(&root::Key::Core(jig_domain_core::Key::Tasks(tasks::Key::Ended(task)))),
            Some(root::Record::Core(jig_domain_core::Record::Tasks(tasks::Stored::Ended(row))))
            if matches!(&row.phase, tasks::Phase::Ended(tasks::Ending::Done(tasks::TaskResult::Report { words: actual })) if actual.as_ref() == words)));
    }
    assert!(owner.store.rows.values().any(|row| matches!(row,
        root::Record::Core(jig_domain_core::Record::People(people::Stored::Answer { outcome, .. }))
        if matches!(outcome, people::Outcome::Released { task } if *task == child))));
    let attempts: std::collections::BTreeSet<_> = owner.peers.as_ref().expect("actual worker").workers[0]
        .seen
        .iter()
        .filter(|row| row.task == child)
        .map(|row| row.attempt)
        .collect();
    assert_eq!(attempts.len(), 3, "two failures then the person's released activation");
}

fn drift_limits() -> jig_client_test_domain::Limits {
    let mut limits = receiving_limits();
    limits.first.connector.paths = 2;
    limits.first.connector.segments = 2;
    limits.first.connector.segment_bytes = 16;
    limits
}

fn drift_client(seed: u64) -> (jig_client_test_domain::Domain, jig_client_test_view::View<'static>) {
    crate::test::new(&drift_limits(), seed)
}

#[test]
fn drift_holds_a_task_and_the_person_is_told_why() {
    use jig_test_connector as connector;
    use jig_test_domain as root;
    let mut world = configured_with_bounds(8519, vec![Box::new([Script::Wait])], 1, |_, _| {}, &drift_limits());
    let person = world.client_with(1, crate::stories::conversation(), drift_limits(), drift_client);
    world.person(person, start(b"watch the resource drift"));
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed);
    assert!(outcome.judged.0 > 0);
    let assignment =
        world.front().session.driver.world.peers.as_ref().expect("actual worker").workers[0].seen[0].clone();
    let resource = connector::Path::new(crate::test::list([Box::from([1_u8]), Box::from([1_u8])]), 16)
        .expect("two authentic connector path segments");
    for event in [
        connector::Event::Adopt { project: 1, resource: resource.clone(), role: connector::ResourceRole::Owned },
        connector::Event::Names { task: assignment.task, project: 1, resources: Box::new([resource.clone()]) },
    ] {
        let now = world.now();
        world.front().session.driver.send(now, clock(now), root::Event::Connector { number: 1, event });
    }
    for state in [10, 11] {
        let now = world.now();
        world.front().session.driver.world.systems[0].other_hand(&resource, state);
        world.front().session.driver.send(
            now,
            clock(now),
            root::Event::Connector {
                number: 1,
                event: connector::Event::System(connector::SystemEvent::Fact {
                    resource: resource.clone(),
                    fact: connector::Fact { state: Some(state), observed: clock(now), pending: false },
                    origin: connector::Origin::Other,
                }),
            },
        );
    }
    assert!(
        matches!(world.front().session.driver.world.store.rows.get(&root::Key::Core(jig_domain_core::Key::Tasks(jig_domain_tasks::Key::Live(assignment.task)))),
        Some(root::Record::Core(jig_domain_core::Record::Tasks(jig_domain_tasks::Stored::Live(row))))
        if matches!(row.phase, jig_domain_tasks::Phase::Held { why: jig_domain_tasks::Hold::Drift, .. }))
    );
    let within = Duration::from_secs(1);
    world.person(
        person,
        vec![
            Step::See { find: Find::text(b"Held drift"), within },
            Step::Press { find: Find::exactly_named(Role::Tab, b"Overview") },
            Step::See { find: Find::text(b"Hold account"), within },
            Step::See { find: Find::text(b"Observed resource state: 11"), within },
            Step::See { find: Find::text(b"Owned resource"), within },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed, "{outcome:?}");
    assert!(outcome.judged.0 > 0);
    let owner = &world.front().session.driver.world;
    assert!(owner.cancellations.contains(&(assignment.task, assignment.attempt)));
    assert_eq!(owner.peers.as_ref().expect("actual worker").workers[0].seen.len(), 1);
}
