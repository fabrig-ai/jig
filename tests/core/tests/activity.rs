const PARTY: u64 = 1;
const FIRST_TURN: u32 = 1;
const SECOND_TURN: u32 = 2;
const THIRD_TURN: u32 = 3;
const FIRST_MARK: u8 = 141;
const SECOND_MARK: u8 = 142;
const VIEW: u64 = 143;
use jig_core_world::effects::World;
use jig_domain_core as core;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_test_domain as root;
use skein_lib::{ReplyTo, Token};

fn lines(world: &World, task: u64) -> Vec<tasks::ActivityLine> {
    world
        .store
        .rows
        .values()
        .filter_map(|row| match row {
            root::Record::Core(core::Record::Tasks(tasks::Stored::Activity(row))) if row.task == task => {
                Some(row.clone())
            }
            root::Record::Core(_) | root::Record::Connector { .. } => None,
        })
        .collect()
}

#[test]
#[expect(clippy::too_many_lines, reason = "one real-store story keeps its complete run and archive assertions")]
fn a_chat_s_activity_holds_the_person_s_words_its_turns_and_its_delegate_s_result() {
    let (configuration, limits) = jig_core_world::faults::plan_fixture(506);
    let mut world = World::configured_with(
        506,
        false,
        configuration,
        limits,
        jig_core_world::stories::reads::forgettable_conversation(),
    );
    let parent = world.assignments[0];
    world.turn(1, 2, None, b"the first committed turn");
    world.send(root::Event::Core(core::Event::People(people::Event::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(Token::new(5001)),
        sign_in: world.sign_in.expect("signed-in person"),
        key: [51; 16],
        ask: people::Ask::Say { project: 1, task: parent.0, words: b"the person's complete words".as_slice().into() },
    })));
    let message = world
        .people_answers
        .iter()
        .rev()
        .find_map(|reply| match reply {
            people::Reply::Outcome(people::Outcome::Said { message, .. }) => Some(*message),
            people::Reply::Signer(_)
            | people::Reply::Full { .. }
            | people::Reply::SignedIn { .. }
            | people::Reply::SignedOut
            | people::Reply::Outcome(_)
            | people::Reply::Again(_)
            | people::Reply::Refused(_) => None,
        })
        .expect("numbered person message");
    world.turn(2, 5, Some(message), b"the second committed turn");
    let mut authority = world.assigned.as_ref().expect("assigned parent").run.authority.clone();
    authority.tools = tasks::Tools(0);
    authority.grants = Box::new([]);
    authority.delegation = tasks::Delegation { kinds: Box::new([]), tasks: 0, depth: 0 };
    authority.budget.spend = 20;
    world.send(root::Event::Core(core::Event::DelegateValidated {
        to: ReplyTo::new(Token::new(5002)),
        key: world.call_key(1),
        batch: Box::new([core::Delegate {
            deadline: jig_domain_tasks::Deadline::Named,
            label: 0,
            flags: jig_domain_tasks::Flags::PLAIN,
            tracked: None,
            budget: jig_domain_people::Budget::Named,
            executor: tasks::Executor::Agent { charter: 1 },
            spec: tasks::Spec {
                words: b"a delegate's work".as_slice().into(),
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
    }));
    let core::CallPart::Delegated(children) = &world.answers.last().expect("delegate answer").2 else {
        panic!("delegate was refused: {:?}", world.answers);
    };
    let child = *world.assignments.iter().find(|(task, _)| *task == children[0].task).expect("delegate assigned");
    assert_ne!(parent.0, child.0);
    world.end(
        tasks::End::Finished {
            result: tasks::TaskResult::Report { words: b"the delegate's complete result".as_slice().into() },
            cancel_delegates: false,
        },
        1,
    );
    let heard = world
        .inbound
        .iter()
        .find(|word| word.from == tasks::Party::Task(child.0) && matches!(word.kind, tasks::MessageKind::Result(_)))
        .expect("delivered delegate result")
        .clone();
    jig_core_world::stories::reads::request(
        &mut world,
        61,
        people::Ask::MarkSeen { seen: people::Seen::Conversation { task: parent.0, through: 2 } },
    );
    world.send(root::Event::Answer {
        read: None,
        refused: Box::new([]),
        channel: Token::new(7),
        task: parent.0,
        attempt: parent.1,
        cumulative: 6,
        end: tasks::End::Finished {
            result: tasks::TaskResult::Report { words: b"done".as_slice().into() },
            cancel_delegates: false,
        },
    });
    let activity = lines(&world, parent.0);
    for (index, row) in activity.iter().enumerate() {
        assert_eq!(row.position, u64::try_from(index).expect("activity count fits u64") + 1);
    }
    let semantic: Vec<_> =
        activity.iter().filter(|row| !matches!(row.line, tasks::Line::Milestone { .. })).map(|row| &row.line).collect();
    assert_eq!(
        semantic,
        vec![
            &tasks::Line::RunBegan { attempt: parent.1, why: tasks::Why::Made, resumed: false },
            &tasks::Line::Turn { reply: 1, attempt: parent.1, turn: 1, read: None },
            &tasks::Line::Words {
                message,
                by: heard_person(&world),
                kind: tasks::MessageKind::Words,
                words: b"the person's complete words".as_slice().into()
            },
            &tasks::Line::Turn { reply: 2, attempt: parent.1, turn: 2, read: Some(message) },
            &tasks::Line::Heard(heard),
            &tasks::Line::RunEnded { attempt: parent.1, end: tasks::EndKind::Finished, spent: 6 },
        ]
    );
    for (turn, bytes) in [(1, b"the first committed turn".as_slice()), (2, b"the second committed turn".as_slice())] {
        let key = root::Key::Core(core::Key::Core(core::CoreKey::Turn { task: parent.0, attempt: parent.1, turn }));
        let root::Record::Core(core::Record::Core(core::CoreRecord::Turn(row))) = &world.store.rows[&key] else {
            panic!("turn archive")
        };
        assert_eq!(row.transcript.as_ref(), bytes);
        assert!(world.commits.iter().any(|commit| commit.iter().any(|write| matches!(write, root::Write::Save(root::Record::Core(core::Record::Core(core::CoreRecord::Turn(row)))) if row.task == parent.0 && row.attempt == parent.1 && row.turn == turn)) && commit.iter().any(|write| matches!(write, root::Write::Save(root::Record::Core(core::Record::Tasks(tasks::Stored::Activity(row)))) if row.task == parent.0 && row.line == (tasks::Line::Turn { reply: u64::from(turn), attempt: parent.1, turn, read: if turn == 1 { None } else { Some(message) } })) )));
    }
    assert!(world.commits.iter().any(|commit| {
        let terminal = commit.iter().any(|write| match write {
            root::Write::Save(root::Record::Core(core::Record::Core(core::CoreRecord::Terminal(row)))) => {
                row.task == parent.0 && row.attempt == parent.1 && row.cumulative == 6
            }
            root::Write::Save(_)
            | root::Write::Wires
            | root::Write::Erase(_)
            | root::Write::CopySettingsRevision(_) => false,
        });
        let line = commit.iter().any(|write| match write {
            root::Write::Save(root::Record::Core(core::Record::Tasks(tasks::Stored::Activity(row)))) => {
                row.task == parent.0
                    && row.line == tasks::Line::RunEnded { attempt: parent.1, end: tasks::EndKind::Finished, spent: 6 }
            }
            root::Write::Save(_)
            | root::Write::Wires
            | root::Write::Erase(_)
            | root::Write::CopySettingsRevision(_) => false,
        });
        terminal && line
    }));
    let key = root::Key::Core(core::Key::Tasks(tasks::Key::Ended(parent.0)));
    let root::Record::Core(core::Record::Tasks(tasks::Stored::Ended(ended))) = &world.store.rows[&key] else {
        panic!("ended task")
    };
    assert_eq!(ended.activity, activity.last().expect("final activity").position);
    world.restart_with(jig_core_world::faults::plan_fixture(506).0);
    assert_eq!(lines(&world, parent.0), activity);
    all_replies_read_then_ended_survive_restart_and_forgetting(&mut world, parent.0, &activity);
}

fn heard_person(world: &World) -> tasks::Party {
    let person = world
        .store
        .rows
        .values()
        .find_map(|row| match row {
            root::Record::Core(core::Record::People(people::Stored::SignIn { person, .. })) => Some(*person),
            root::Record::Core(_) | root::Record::Connector { .. } => None,
        })
        .expect("retained sign-in");
    tasks::Party::Person(person)
}

fn all_replies_read_then_ended_survive_restart_and_forgetting(
    world: &mut World,
    task: u64,
    activity: &[tasks::ActivityLine],
) {
    use jig_core_world::reads::{Parties, ask, latest};
    let read_key = root::Key::Core(core::Key::People(people::Key::EndedRead { party: 1, task }));
    assert!(matches!(
        world.store.rows.get(&read_key),
        Some(root::Record::Core(core::Record::People(people::Stored::EndedRead { through: 2, .. })))
    ));
    world.reads = Some(Parties::new(world.store.applied));
    ask(world, 71, core::reads::Read::Frame);
    let core::reads::Answer::Document(document) = latest(world) else { panic!("frame") };
    let core::documents::Document::Frame(frame) = document.as_ref() else { panic!("frame") };
    assert_eq!(frame.updates, core::documents::Count::Known(1), "only the unread result remains");
    let reply =
        activity.iter().find(|line| matches!(line.line, tasks::Line::Turn { reply: 2, .. })).expect("reply activity");
    ask(world, 72, core::reads::Read::Reply { task, activity: reply.position });
    assert!(
        matches!(latest(world), core::reads::Answer::Historical(value) if matches!(&value.value, core::reads::Past::Turn(turn) if turn.turn == 2 && turn.transcript.as_ref() == b"the second committed turn"))
    );
    let ended_key = root::Key::Core(core::Key::Tasks(tasks::Key::Ended(task)));
    let root::Record::Core(core::Record::Tasks(tasks::Stored::Ended(owner))) = world.store.rows[&ended_key].clone()
    else {
        panic!("ended owner")
    };
    jig_core_world::stories::reads::request(
        world,
        62,
        people::Ask::MarkSeen { seen: people::Seen::Results { through: owner.result_position } },
    );
    assert!(!world.store.rows.contains_key(&ended_key));
    assert!(!world.store.rows.contains_key(&read_key));
    assert!(world.commits.iter().any(|writes| writes.contains(&root::Write::Erase(ended_key.clone()))
        && writes.contains(&root::Write::Erase(read_key.clone()))));
    world.restart_with(jig_core_world::faults::plan_fixture(506).0);
    ask(world, 73, core::reads::Read::Reply { task, activity: reply.position });
    assert_eq!(latest(world), &core::reads::Answer::None(core::reads::Why::Gone));
}

#[test]
fn an_ended_conversation_keeps_its_unread_results_position() {
    let mut world = jig_core_world::stories::reads::world(507);
    let task = world.assigned.as_ref().expect("chat").task;
    world.turn(1, 1, None, b"reply");
    mark_conversation(&mut world, task);
    finish_chat(&mut world);
    let ended_key = root::Key::Core(core::Key::Tasks(tasks::Key::Ended(task)));
    let read_key = root::Key::Core(core::Key::People(people::Key::EndedRead { party: 1, task }));
    assert!(world.store.rows.contains_key(&ended_key));
    assert!(world.store.rows.contains_key(&read_key));
    world.restart(507, false);
    assert!(world.store.rows.contains_key(&ended_key));
    assert!(world.store.rows.contains_key(&read_key));
    jig_core_world::reads::ask(&mut world, 81, core::reads::Read::Frame);
    let core::reads::Answer::Document(frame) = jig_core_world::reads::latest(&world) else { panic!("frame") };
    let core::documents::Document::Frame(frame) = frame.as_ref() else { panic!("frame") };
    assert_eq!(frame.updates, core::documents::Count::Known(1));
}

#[test]
fn reading_an_ended_conversations_replies_then_forgets_it() {
    let mut world = jig_core_world::stories::reads::forgettable_world(508);
    let task = world.assigned.as_ref().expect("chat").task;
    world.turn(1, 1, None, b"reply");
    finish_chat(&mut world);
    let ended_key = root::Key::Core(core::Key::Tasks(tasks::Key::Ended(task)));
    let root::Record::Core(core::Record::Tasks(tasks::Stored::Ended(row))) = &world.store.rows[&ended_key] else {
        panic!("ended chat")
    };
    let through = row.result_position;
    jig_core_world::stories::reads::request(
        &mut world,
        62,
        people::Ask::MarkSeen { seen: people::Seen::Results { through } },
    );
    assert!(world.store.rows.contains_key(&ended_key));
    mark_conversation(&mut world, task);
    let read_key = root::Key::Core(core::Key::People(people::Key::EndedRead { party: 1, task }));
    assert!(!world.store.rows.contains_key(&ended_key));
    assert!(!world.store.rows.contains_key(&read_key));
    assert!(world.commits.iter().any(|writes| writes.contains(&root::Write::Erase(ended_key.clone()))
        && writes.contains(&root::Write::Erase(read_key.clone()))));
    world.restart(508, false);
    jig_core_world::reads::ask(&mut world, 81, core::reads::Read::Task { task });
    assert_eq!(jig_core_world::reads::latest(&world), &core::reads::Answer::None(core::reads::Why::Unknown));
}

fn mark_conversation(world: &mut World, task: u64) {
    jig_core_world::stories::reads::request(
        world,
        61,
        people::Ask::MarkSeen { seen: people::Seen::Conversation { task, through: 1 } },
    );
}

fn finish_chat(world: &mut World) {
    world.end(
        tasks::End::Finished {
            result: tasks::TaskResult::Report { words: b"done".as_slice().into() },
            cancel_delegates: false,
        },
        1,
    );
}

fn ended_read(task: u64, through: u64) -> root::Record {
    root::Record::Core(core::Record::People(people::Stored::EndedRead { task, party: PARTY, through }))
}
fn assert_ended_position(world: &mut World, task: u64, through: u64, unread: u32) {
    assert_eq!(
        world.store.rows.get(&root::Key::Core(core::Key::People(people::Key::EndedRead { task, party: PARTY }))),
        Some(&ended_read(task, through))
    );
    assert!(
        !world
            .store
            .rows
            .contains_key(&root::Key::Core(core::Key::People(people::Key::ReadConversation { task, party: PARTY })))
    );
    let query = core::documents::TaskQuery {
        phase: Some(core::documents::PhaseClass::Ended),
        order: core::documents::QueryOrder::Ended,
        ..jig_core_world::fixtures::OWN_TASKS
    };
    jig_core_world::reads::ask(world, VIEW, core::reads::Read::Tasks { query, live: None, ended: None });
    let core::reads::Answer::Tasks(page) = jig_core_world::reads::latest(world) else { panic!("task page") };
    let core::reads::EndedGroup::Rows { rows, .. } = &page.ended else { panic!("ended tasks") };
    let line = rows.iter().next().expect("task row");
    let position = line.conversation.expect("read position");
    assert_eq!(position.seen, through);
    assert_eq!(position.unread, core::documents::Count::Known(unread));
}
#[test]
fn a_restart_restores_an_ended_conversations_read_position() {
    let mut world = jig_core_world::stories::reads::world(834);
    let task = world.assigned.as_ref().expect("chat").task;
    world.turn(FIRST_TURN, 1, None, b"first");
    world.turn(SECOND_TURN, 2, None, b"second");
    jig_core_world::stories::reads::request(
        &mut world,
        FIRST_MARK,
        people::Ask::MarkSeen { seen: people::Seen::Conversation { task, through: 1 } },
    );
    jig_core_world::fixtures::finish_chat(&mut world, 2, b"done");
    assert_ended_position(&mut world, task, 1, 1);
    world.restart(834, false);
    assert_ended_position(&mut world, task, 1, 1);
}
#[test]
fn a_conversation_mark_waits_for_its_concurrent_ending() {
    let mut world = jig_core_world::stories::reads::world(835);
    let task = world.assigned.as_ref().expect("chat").task;
    world.turn(FIRST_TURN, 1, None, b"first");
    world.turn(SECOND_TURN, 2, None, b"second");
    jig_core_world::stories::reads::request(
        &mut world,
        FIRST_MARK,
        people::Ask::MarkSeen { seen: people::Seen::Conversation { task, through: 1 } },
    );
    world.store_delay = 3;
    let end = world.end_event(
        tasks::End::Finished {
            result: tasks::TaskResult::Report { words: b"done".as_slice().into() },
            cancel_delegates: false,
        },
        2,
    );
    let seen = people::Seen::Conversation { task, through: 2 };
    let mark = jig_core_world::stories::reads::request_event(&world, SECOND_MARK, people::Ask::MarkSeen { seen });
    world.send_all(vec![end, mark]);
    assert_eq!(world.people_answers.last(), Some(&people::Reply::Outcome(people::Outcome::MarkedSeen { seen })));
    assert_ended_position(&mut world, task, 2, 0);
}
#[test]
fn a_second_conversation_mark_is_busy_while_loading() {
    let mut world = jig_core_world::stories::reads::world(836);
    let task = world.assigned.as_ref().expect("chat").task;
    world.turn(FIRST_TURN, 1, None, b"first");
    world.turn(SECOND_TURN, 2, None, b"second");
    world.turn(THIRD_TURN, 3, None, b"third");
    jig_core_world::fixtures::finish_chat(&mut world, 3, b"done");
    world.store_delay = 3;
    let first = jig_core_world::stories::reads::request_event(
        &world,
        FIRST_MARK,
        people::Ask::MarkSeen { seen: people::Seen::Conversation { task, through: 2 } },
    );
    let second = jig_core_world::stories::reads::request_event(
        &world,
        SECOND_MARK,
        people::Ask::MarkSeen { seen: people::Seen::Conversation { task, through: 1 } },
    );
    world.send_all(vec![first, second]);
    assert!(
        world.people_answers.contains(&people::Reply::Again(people::Again::Busy)),
        "answers: {:?}; rejected inputs: {:?}",
        world.people_answers,
        world.busy_events
    );
    let seen = people::Seen::Conversation { task, through: 1 };
    jig_core_world::stories::reads::request(&mut world, SECOND_MARK, people::Ask::MarkSeen { seen });
    assert_eq!(world.people_answers.last(), Some(&people::Reply::Outcome(people::Outcome::MarkedSeen { seen })));
    assert_ended_position(&mut world, task, 2, 1);
}
