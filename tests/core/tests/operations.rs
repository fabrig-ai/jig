use jig_core_world::fixtures::{live_row, person_unchanged};
use jig_core_world::stories::delegates::delegate_pair;
use jig_core_world::stories::reads::{request, world};
use jig_domain_core as core;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_test_domain as root;
use skein_lib::Duration;

const PROJECT: u32 = 1;
const OWNER: u64 = 1;
const CANCEL: u8 = 1;
const AMEND: u8 = 2;
const MOVE_SIGN_IN: u64 = 6100;
const MARK_BEFORE_MOVE: u8 = 21;
const MOVE: u8 = 22;
const FIRST_MOVE_TURN: u32 = 1;

#[test]
fn a_partys_cancel_with_a_grace_ends_the_chat_when_the_grace_ends() {
    let mut world = world(1102);
    let task = world.assigned.as_ref().expect("chat").task;
    request(
        &mut world,
        CANCEL,
        people::Ask::Cancel {
            project: PROJECT,
            task,
            reason: b"wrap up".as_slice().into(),
            grace: Some(Duration::from_secs(5)),
        },
    );
    assert_eq!(world.people_answers.last(), Some(&people::Reply::Outcome(people::Outcome::Cancelled { task })));
    assert_eq!(live_row(&world, task).grace.as_ref().expect("finish requested").reason.as_ref(), b"wrap up");
    let finish = world.inbound.last().expect("the run hears the finish word");
    assert_eq!(finish.words.as_ref(), b"wrap up");
    assert_eq!(finish.kind, tasks::MessageKind::FinishRequested { revision: 1 });
    world.advance(Duration::from_secs(5));
    world.end(tasks::End::Parked, 0);
    let Some(root::Record::Core(core::Record::Tasks(tasks::Stored::Ended(row)))) =
        world.store.rows.get(&root::Key::Core(core::Key::Tasks(tasks::Key::Ended(task))))
    else {
        panic!("the chat ended")
    };
    let tasks::Phase::Ended(tasks::Ending::Cancelled { by, reason, .. }) = &row.phase else { panic!("cancelled") };
    assert_eq!(*by, tasks::Party::Person(OWNER));
    assert_eq!(reason.as_ref(), b"wrap up");
}

#[test]
fn a_maintainer_tracks_a_task_by_amendment() {
    let mut world = jig_core_world::stories::offers::world(1103, Some(people::Role::Maintainer));
    let task = world.assigned.as_ref().expect("chat").task;
    let revision = live_row(&world, task).revisions.spec;
    request(
        &mut world,
        AMEND,
        people::Ask::Amend {
            project: PROJECT,
            task,
            revision,
            amendment: people::Amendment {
                tracked: Some(people::Tracking::Tracked { priority: 7 }),
                ..person_unchanged(b"tracked")
            },
            release: None,
        },
    );
    assert_eq!(
        world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::Amended { task, released: false }))
    );
    assert_eq!(live_row(&world, task).tracked, Some(7));
}

#[test]
fn a_grace_past_the_limit_is_refused_naming_it() {
    let mut world = world(1104);
    let task = world.assigned.as_ref().expect("chat").task;
    request(
        &mut world,
        CANCEL,
        people::Ask::Cancel {
            project: PROJECT,
            task,
            reason: b"wrap up".as_slice().into(),
            grace: Some(Duration::from_secs(601)),
        },
    );
    assert_eq!(
        world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::Refused(people::Refusal::Limit {
            which: people::LimitKind::Grace
        })))
    );
    assert_eq!(live_row(&world, task).grace, None);
}

fn compound(operations: impl IntoIterator<Item = people::Operation>) -> people::Ask {
    let mut bounded = skein_lib::List::with_capacity(3);
    for operation in operations {
        bounded.push(operation).expect("three test operations");
    }
    people::Ask::Compound { project: PROJECT, operations: bounded }
}

fn compound_world(seed: u64) -> jig_core_world::effects::World {
    let (config, mut limits) = jig_core_world::faults::plan_fixture(seed);
    limits.core.people.operations = 3;
    jig_core_world::effects::World::configured(seed, false, config, limits)
}

fn closing(chat: u64, cancelled: u64, moved: u64) -> people::Ask {
    compound([
        people::Operation::Cancel { task: cancelled, reason: b"done".as_slice().into(), grace: None },
        people::Operation::Move { task: moved, reason: b"continue".as_slice().into() },
        people::Operation::Cancel {
            task: chat,
            reason: b"finish".as_slice().into(),
            grace: Some(Duration::from_secs(5)),
        },
    ])
}

#[test]
fn a_compound_closes_a_conversation_moving_one_delegate_and_cancelling_another() {
    let mut world = compound_world(1107);
    let chat = world.assignments[0].0;
    let (cancelled, moved) = delegate_pair(&mut world);
    request(&mut world, 11, closing(chat, cancelled, moved));
    let first_message = 1;
    assert_eq!(live_row(&world, chat).grace.as_ref().expect("finish asked").message, first_message + 2);
    assert_eq!(
        world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::Compound { project: PROJECT, operations: 3, first_message }))
    );
    assert_eq!(live_row(&world, moved).requester, tasks::Party::Person(OWNER));
    let tasks::Phase::Closing(ending) = &live_row(&world, cancelled).phase else { panic!("delegate cancelled") };
    let tasks::Ending::Cancelled { by, reason, result, .. } = &ending.ending else { panic!("cancelled ending") };
    assert_eq!(*by, tasks::Party::Person(OWNER));
    assert_eq!(reason.as_ref(), b"done");
    assert_eq!(*result, None);
}

#[test]
fn a_compound_refused_at_its_second_operation_makes_nothing() {
    let mut world = world(1108);
    let chat = world.assigned.as_ref().expect("chat").task;
    let revision = live_row(&world, chat).revision;
    request(
        &mut world,
        12,
        compound([
            people::Operation::Message { task: chat, words: b"unmade".as_slice().into() },
            people::Operation::Cancel {
                task: chat,
                reason: b"end".as_slice().into(),
                grace: Some(Duration::from_secs(601)),
            },
        ]),
    );
    assert_eq!(
        world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::CompoundRefused {
            at: 1,
            why: people::Refusal::Limit { which: people::LimitKind::Grace }
        }))
    );
    assert_eq!(live_row(&world, chat).revision, revision);
    assert_eq!(world.inbound, []);
}

#[test]
fn a_compound_naming_another_projects_task_is_refused_at_its_place() {
    let (config, limits) = jig_core_world::fixtures::two_projects(1109);
    let mut world = jig_core_world::effects::World::configured(1109, false, config, limits);
    let first = world.assigned.as_ref().expect("first chat").task;
    let other_projects_chat = jig_core_world::fixtures::chat(&world, 2, b"other".as_slice().into());
    request(&mut world, 13, other_projects_chat);
    let people::Reply::Outcome(people::Outcome::Created { first: second, .. }) =
        *world.people_answers.last().expect("second chat")
    else {
        panic!("second project chat")
    };
    assert_eq!(live_row(&world, second).project, 2);
    request(
        &mut world,
        14,
        compound([people::Operation::Stop { task: first }, people::Operation::Stop { task: second }]),
    );
    assert_eq!(
        world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::CompoundRefused { at: 1, why: people::Refusal::Unknown }))
    );
    assert_eq!(live_row(&world, first).revisions.hold, 0);
}

#[test]
fn a_compound_reorders_two_goals_at_once() {
    let mut world = compound_world(1110);
    let chat = world.assignments[0].0;
    let (first, second) = delegate_pair(&mut world);
    let track = |task, priority| people::Operation::Amend {
        task,
        revision: 0,
        amendment: Box::new(people::Amendment {
            tracked: Some(people::Tracking::Tracked { priority }),
            ..person_unchanged(b"order")
        }),
        release: None,
    };
    request(&mut world, 15, compound([track(first, 1), track(second, 2)]));
    request(&mut world, 16, compound([track(first, 2), track(second, 1)]));
    assert_eq!((live_row(&world, first).tracked, live_row(&world, second).tracked), (Some(2), Some(1)));
    assert_eq!(live_row(&world, chat).tracked, None);
}

#[test]
fn a_compound_sent_again_after_a_crash_is_made_once_and_answered_once() {
    use jig_core_world::effects::Cut;
    let mut world = compound_world(1111);
    let chat = world.assignments[0].0;
    let (cancelled, moved) = delegate_pair(&mut world);
    let before = world.people_answers.len();
    let number = world.store.applied + 1;
    world.store.fault(jig_core_world::Fault::Hold { commits: 1 });
    world.cut = Some(Cut::Submitted(number));
    request(&mut world, 17, closing(chat, cancelled, moved));
    assert_eq!(world.reached_cut, Some(Cut::Submitted(number)));
    world.cut = Some(Cut::Durable(number));
    world.drain();
    assert_eq!(world.reached_cut, Some(Cut::Durable(number)));
    assert_eq!(world.store.applied, number);
    assert_eq!(world.people_answers.len(), before);
    world.restart_with(jig_core_world::faults::plan_fixture(1111).0);
    let revision = live_row(&world, chat).revision;
    let first_message = 1;
    assert_eq!(live_row(&world, chat).grace.as_ref().expect("grace retained").message, first_message + 2);
    request(&mut world, 17, closing(chat, cancelled, moved));
    assert_eq!(world.people_answers.len(), before + 1);
    assert_eq!(
        world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::Compound { project: PROJECT, operations: 3, first_message }))
    );
    assert_eq!(live_row(&world, chat).revision, revision);
    assert_eq!(live_row(&world, moved).requester, tasks::Party::Person(OWNER));
}

#[test]
fn a_tasks_refusal_precedes_a_later_core_refusal_in_a_compound() {
    let mut world = world(1112);
    let chat = world.assigned.as_ref().expect("chat").task;
    let revision = live_row(&world, chat).revision;
    request(
        &mut world,
        18,
        compound([
            people::Operation::Amend {
                task: chat,
                revision: 0,
                amendment: Box::new(people::Amendment {
                    dependencies: Some(Box::new([chat])),
                    ..person_unchanged(b"cycle")
                }),
                release: None,
            },
            people::Operation::Stop { task: u64::MAX },
        ]),
    );
    assert_eq!(
        world.people_answers.last(),
        Some(&people::Reply::Outcome(people::Outcome::CompoundRefused {
            at: 0,
            why: people::Refusal::Circle { tasks: Box::new([chat]), cut: false }
        }))
    );
    assert_eq!(live_row(&world, chat).revision, revision);
}

#[test]
fn an_earlier_compound_amendment_sets_the_next_spec_revision() {
    let mut world = world(1113);
    let task = world.assigned.as_ref().expect("chat").task;
    let edit = |revision, words: &[u8]| people::Operation::Amend {
        task,
        revision,
        release: None,
        amendment: Box::new(people::Amendment {
            spec: Some(people::Spec {
                asks: false,
                words: words.into(),
                parameters: Box::new([]),
                inputs: Box::new([]),
            }),
            ..person_unchanged(b"edit")
        }),
    };
    request(&mut world, 19, compound([edit(0, b"first"), edit(1, b"second")]));
    assert_eq!(live_row(&world, task).revisions.spec, 2);
    assert_eq!(live_row(&world, task).spec.words.as_ref(), b"second");
}

fn another_requester(world: &mut jig_core_world::effects::World) -> u64 {
    let first = world.sign_in.expect("first requester signed in");
    world.send(root::Event::Core(core::Event::SignIn {
        held: None,
        reply_to: skein_lib::ReplyTo::new(skein_lib::Token::new(MOVE_SIGN_IN)),
        identity: people::Identity {
            key: people::IdentityKey { provider: 0, subject: 8_u64.to_be_bytes().into() },
            login: b"second".as_slice().into(),
            name: b"second requester".as_slice().into(),
        },
    }));
    let people::Reply::SignedIn { person, .. } = *world.people_answers.last().expect("second sign-in") else {
        panic!("second signed in")
    };
    let second = world.sign_in.replace(first).expect("second session");
    world.send(root::Event::Core(core::Event::People(people::Event::Roles {
        project: PROJECT,
        holdings: Box::new([
            people::Holding { person: OWNER, role: people::Role::Owner },
            people::Holding { person, role: people::Role::Owner },
        ]),
    })));
    second
}

#[test]
fn a_moved_conversation_starts_its_new_requester_with_nothing_read() {
    let mut world = world(1116);
    let task = world.assigned.as_ref().expect("conversation").task;
    let ask = people::Ask::Move { project: PROJECT, task, reason: b"continue".as_slice().into() };
    assert_moved_read(&mut world, task, ask);
}

#[test]
fn a_compound_moved_conversation_starts_its_new_requester_with_nothing_read() {
    let mut world = world(1116);
    let task = world.assigned.as_ref().expect("conversation").task;
    let ask = compound([people::Operation::Move { task, reason: b"continue".as_slice().into() }]);
    assert_moved_read(&mut world, task, ask);
}

fn assert_moved_read(world: &mut jig_core_world::effects::World, task: u64, ask: people::Ask) {
    world.turn(FIRST_MOVE_TURN, 0, None, b"unread after move");
    let second = another_requester(world);
    request(
        world,
        MARK_BEFORE_MOVE,
        people::Ask::MarkSeen { seen: people::Seen::Conversation { task, through: u64::from(FIRST_MOVE_TURN) } },
    );
    assert!(
        world
            .store
            .rows
            .contains_key(&root::Key::Core(core::Key::People(people::Key::ReadConversation { party: OWNER, task })))
    );
    world.sign_in = Some(second);
    request(world, MOVE, ask);
    let tasks::Party::Person(to) = live_row(world, task).requester else { panic!("new requester") };
    assert_ne!(to, OWNER, "{:?}", world.people_answers.last());
    assert_eq!(
        world.store.rows.get(&root::Key::Core(core::Key::People(people::Key::ReadConversation { party: OWNER, task }))),
        None
    );
    assert_eq!(
        world.store.rows.get(&root::Key::Core(core::Key::People(people::Key::ReadConversation { party: to, task }))),
        Some(&root::Record::Core(core::Record::People(people::Stored::ReadConversation {
            party: to,
            task,
            through: 0
        })))
    );
    let decision = world
        .commits
        .iter()
        .find(|commit| {
            commit.iter().any(|write| {
                *write
                    == root::Write::Erase(root::Key::Core(core::Key::People(people::Key::ReadConversation {
                        party: OWNER,
                        task,
                    })))
            })
        })
        .expect("one move decision");
    assert!(decision.contains(&root::Write::Save(root::Record::Core(core::Record::People(
        people::Stored::ReadConversation { party: to, task, through: 0 }
    )))));
}
