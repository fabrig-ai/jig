//! Genuine Question and Result cards keep precisely their supplied acts.
use super::*;

#[test]
fn a_real_engine_question_offers_exactly_its_admitted_answer_act() {
    let mut world = configured_with_slots(8520, vec![Box::new([Script::Wait]), Box::new([Script::Wait])], 2, |_, _| {});
    let member = world.client_with(3, crate::stories::conversation(), receiving_limits(), client);
    world.person(member, start(b"ask a question"));
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed);
    assert!(outcome.judged.0 > 0);
    let assignment =
        world.front().session.driver.world.peers.as_ref().expect("actual worker").workers[0].seen[0].clone();
    // A real delegate may question its requester; self-messaging is not a
    // permitted task reference. Preserve the genuine parent/person inbox.
    let mut authority = match world
        .front()
        .session
        .driver
        .world
        .store
        .rows
        .get(&jig_test_domain::Key::Core(jig_domain_core::Key::Tasks(jig_domain_tasks::Key::Live(assignment.task))))
    {
        Some(jig_test_domain::Record::Core(jig_domain_core::Record::Tasks(jig_domain_tasks::Stored::Live(task)))) => {
            task.authority.clone()
        }
        other => panic!("actual parent: {other:?}"),
    };
    authority.budget.spend = 20;
    authority.delegation = jig_domain_tasks::Delegation { kinds: Box::new([]), tasks: 0, depth: 0 };
    let now = world.now();
    world.front().session.driver.send(
        now,
        clock(now),
        jig_test_domain::Event::Core(jig_domain_core::Event::DelegateValidated {
            to: skein_lib::ReplyTo::new(Token::new(9850)),
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
                executor: jig_domain_tasks::Executor::Agent { charter: 1 },
                spec: jig_domain_tasks::Spec {
                    asks: false,
                    words: b"ask its parent".as_slice().into(),
                    parameters: Box::new([]),
                    inputs: Box::new([]),
                },
                contract: jig_domain_tasks::Contract::Report { words: 128 },
                authority,
                llm: None,
                symbolic_grants: Box::new([]),
                dependencies: Box::new([]),
                wake: jig_domain_tasks::WakePolicy::DEFAULT,
            }]),
            stubs: Box::new([]),
        }),
    );
    world.person(member, vec![Step::Pause { span: Duration::from_millis(1) }]);
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed);
    assert!(outcome.judged.0 > 0);
    let child = world.front().session.driver.world.peers.as_ref().expect("actual delegate").workers[0].seen[1].clone();
    super::engine_stories::action(
        &mut world,
        &child,
        1,
        jig_domain_core::NamedAction::Message {
            target: assignment.task,
            kind: jig_domain_tasks::MessageKind::Question,
            words: b"which report should I use?".as_slice().into(),
        },
    );
    let within = Duration::from_secs(1);
    world.person(
        member,
        vec![
            Step::Go { address: crate::stories::inbox() },
            Step::See { find: Find::text(b"which report should I use?"), within },
            Step::See { find: Find::exactly_named(Role::Button, b"Answer"), within },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed, "{outcome:?}");
    assert!(outcome.judged.0 > 0);
    let questions: Vec<_> = world
        .front()
        .session
        .card_offer_keys(member)
        .into_iter()
        .filter(|key| matches!(key, client::ObjectKey::Question { .. }))
        .collect();
    assert_eq!(questions.len(), 1);
    super::super::offered_acts::card(&mut world, member, questions[0], b"Question");
    assert!(world.front().session.driver.world.store.rows.values().any(|row| matches!(row,
        jig_test_domain::Record::Core(jig_domain_core::Record::Tasks(jig_domain_tasks::Stored::Live(task)))
        if task.number == assignment.task && task.inbox.iter().any(|word|
            word.kind == jig_domain_tasks::MessageKind::Question && word.words.as_ref() == b"which report should I use?"))));
}

#[test]
fn a_real_completed_chat_result_offers_exactly_its_admitted_mark_read_act() {
    let mut world = world(
        8521,
        vec![Box::new([
            Script::Silence { for_: Duration::from_millis(100) },
            Script::Finish { report: b"the complete result".as_slice().into() },
        ])],
    );
    let member = world.client_with(3, crate::stories::conversation(), receiving_limits(), client);
    let mut steps = start(b"finish this chat");
    steps.push(Step::Pause { span: Duration::from_millis(200) });
    world.person(member, steps);
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed);
    assert!(outcome.judged.0 > 0);
    let within = Duration::from_secs(1);
    world.person(
        member,
        vec![
            Step::Go { address: crate::stories::inbox() },
            Step::Press { find: Find::exactly_named(Role::Button, b"Result") },
            Step::See { find: Find::text(b"the complete result"), within },
            Step::See { find: Find::exactly_named(Role::Button, b"Mark read"), within },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed, "{outcome:?}");
    assert!(outcome.judged.0 > 0);
    let results: Vec<_> = world
        .front()
        .session
        .card_offer_keys(member)
        .into_iter()
        .filter(|key| matches!(key, client::ObjectKey::Result(_)))
        .collect();
    assert_eq!(results.len(), 1);
    super::super::offered_acts::card(&mut world, member, results[0], b"Result");
    assert!(world.front().session.driver.world.store.rows.values().any(|row| matches!(row,
        jig_test_domain::Record::Core(jig_domain_core::Record::Tasks(jig_domain_tasks::Stored::Ended(task)))
        if matches!(&task.phase, jig_domain_tasks::Phase::Ended(jig_domain_tasks::Ending::Done(
            jig_domain_tasks::TaskResult::Report { words })) if words.as_ref() == b"the complete result"))));
}
