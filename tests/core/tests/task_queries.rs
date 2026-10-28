use jig_core_world::{
    reads::{ask, latest},
    stories::reads::{request, world},
};
use jig_domain_core::{self as core, documents, reads, waits::WaitsOn};
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_test_domain as root;
use skein_lib::{ReplyTo, Token};

#[test]
fn a_placed_stop_reads_as_held_without_a_generic_escalation() {
    let mut world = world(1201);
    let task = world.assigned.as_ref().expect("chat").task;
    request(&mut world, 1, people::Ask::Stop { project: 1, task });
    ask(&mut world, 2, reads::Read::Task { task });
    let reads::Answer::Document(document) = latest(&world) else { panic!("a live task") };
    let documents::Document::Task(value) = document.as_ref() else { panic!("the task's document") };
    assert_eq!(value.waits_on, WaitsOn::Nothing);
    assert!(matches!(value.hold.as_ref().map(|hold| hold.reason), Some(tasks::Hold::Placed { .. })));
}

fn delegate_question(world: &mut jig_core_world::effects::World, parent: u64) {
    let mut authority = world.assigned.as_ref().expect("parent run").run.authority.clone();
    authority.budget.spend = 20;
    authority.delegation = tasks::Delegation { kinds: Box::new([]), tasks: 0, depth: 0 };
    world.send(root::Event::Core(core::Event::DelegateValidated {
        to: ReplyTo::new(Token::new(4001)),
        key: world.call_key(1),
        batch: Box::new([core::Delegate {
            deadline: jig_domain_tasks::Deadline::Named,
            label: 0,
            flags: jig_domain_tasks::Flags::PLAIN,
            tracked: None,
            budget: jig_domain_people::Budget::Named,
            executor: tasks::Executor::Agent { charter: 1 },
            spec: tasks::Spec {
                asks: false,
                words: b"ask the requester".as_slice().into(),
                parameters: Box::new([]),
                inputs: Box::new([]),
            },
            contract: tasks::Contract::Report { words: 64 },
            authority,
            llm: None,
            symbolic_grants: Box::new([]),
            dependencies: Box::new([]),
            wake: tasks::WakePolicy::DEFAULT,
        }]),
        stubs: Box::new([]),
    }));
    let core::CallPart::Delegated(numbers) = &world.answers.last().expect("delegation answered").2 else {
        panic!("admitted delegate")
    };
    let child = numbers[0].task;
    world.end(tasks::End::Parked, 0);
    assert_eq!(world.assigned.as_ref().expect("delegate assigned").task, child);
    world.send(root::Event::Core(core::Event::NamedAction {
        to: ReplyTo::new(Token::new(4002)),
        key: world.call_key(1),
        action: core::NamedAction::Message {
            target: parent,
            kind: tasks::MessageKind::Question,
            words: b"what next?".as_slice().into(),
        },
    }));
    assert!(matches!(world.answers.last(), Some((_, _, core::CallPart::Sent { .. }))));
}

#[test]
fn what_each_live_task_waits_on_matches_the_reference_from_the_store() {
    let mut world = world(1202);
    let parent = world.assigned.as_ref().expect("chat").task;
    delegate_question(&mut world, parent);
    request(&mut world, 1, people::Ask::Stop { project: 1, task: parent });
    let live: Vec<_> = world
        .store
        .rows
        .values()
        .filter_map(|row| match row {
            root::Record::Core(core::Record::Tasks(tasks::Stored::Live(row))) => Some(row.number),
            root::Record::Core(_) | root::Record::Connector { .. } => None,
        })
        .collect();
    for task in live {
        let expected = jig_core_world::reference::waits_on(&world.store.rows, task);
        ask(&mut world, 10 + task, reads::Read::Task { task });
        let reads::Answer::Document(document) = latest(&world) else { panic!("live task {task}") };
        let documents::Document::Task(value) = document.as_ref() else { panic!("task document") };
        assert_eq!(value.waits_on, expected, "task {task}");
    }
}

#[test]
fn me_reads_the_pool_left_in_its_creating_bounds() {
    let mut world = world(1203);
    ask(&mut world, 20, reads::Read::Frame);
    let reads::Answer::Document(document) = latest(&world) else { panic!("me document") };
    let documents::Document::Frame(value) = document.as_ref() else { panic!("me") };
    let project = value.projects.iter().find(|project| project.project == 1).expect("project");
    let Some(core::acts::Creating { within: core::acts::Bounds::Create { pool_left, .. }, .. }) = &project.creating
    else {
        panic!("creating")
    };
    assert_eq!(*pool_left, 200);
}

const QUERY_PARENT: u64 = 1;
const QUERY_FIRST_WATCHER: u64 = 100;
const QUERY_FENCE_WATCHER: u64 = 90;
use jig_core_world::fixtures::QUERY_REQUESTER;
const QUERY_TABLE: [documents::TaskQuery; 6] = [
    jig_core_world::fixtures::OWN_TASKS,
    documents::TaskQuery {
        project: Some(1),
        requester: None,
        root: Some(QUERY_PARENT),
        order: documents::QueryOrder::Changed,
        ..jig_core_world::fixtures::OWN_TASKS
    },
    documents::TaskQuery {
        project: Some(1),
        requester: None,
        parent: Some(QUERY_PARENT),
        order: documents::QueryOrder::Activity,
        ..jig_core_world::fixtures::OWN_TASKS
    },
    documents::TaskQuery {
        project: Some(1),
        label: Some(0),
        tracked: Some(false),
        order: documents::QueryOrder::Priority,
        ..jig_core_world::fixtures::OWN_TASKS
    },
    documents::TaskQuery {
        project: Some(1),
        requester: None,
        executor: Some(documents::ExecutorKind::Agent),
        phase: Some(documents::PhaseClass::Held),
        ..jig_core_world::fixtures::OWN_TASKS
    },
    documents::TaskQuery {
        project: Some(1),
        requester: None,
        order: documents::QueryOrder::Ended,
        ..jig_core_world::fixtures::OWN_TASKS
    },
];

#[test]
fn a_task_querys_lines_match_the_reference_over_the_stores_tasks() {
    let mut world = world(1204);
    let parent = world.assigned.as_ref().expect("configured fixture").task;
    delegate_question(&mut world, parent);
    request(&mut world, 1, people::Ask::Stop { project: 1, task: parent });
    world.watches = Some(jig_core_world::watches::Parties::new(
        1204,
        world.store.applied,
        false,
        world.sign_in.expect("configured fixture"),
    ));
    for (offset, query) in QUERY_TABLE.into_iter().enumerate() {
        let expected = jig_core_world::reference::lines(&world.store.rows, QUERY_REQUESTER, &query);
        let mut paged = Vec::new();
        let mut after = None;
        for page in 0..4 {
            let watcher = QUERY_FIRST_WATCHER + u64::try_from(offset * 4 + page).expect("configured fixture");
            world.send(root::Event::Core(core::Event::Watch {
                watcher: Token::new(watcher),
                sign_in: world.sign_in.expect("configured fixture"),
                subject: documents::Watched::Query { query },
                window: documents::Window { rows: 1, after },
            }));
            let documents::Document::Lines(value) = jig_core_world::watches::latest(&world, watcher) else {
                panic!("query")
            };
            paged.extend(value.lines.iter().map(|line| line.chip.number));
            after = value.next;
            world.send(root::Event::Core(core::Event::Unwatch { watcher: Token::new(watcher) }));
            world.send(root::Event::Core(core::Event::Delivered { watcher: Token::new(watcher) }));
            if after.is_none() {
                break;
            }
        }
        assert_eq!(paged, expected, "query {query:?}");
    }
}

#[test]
fn a_watched_task_querys_deliveries_each_follow_their_commit() {
    let mut world = jig_core_world::watches::world(1205, false);
    jig_core_world::watches::open(
        &mut world,
        QUERY_FENCE_WATCHER,
        documents::Watched::Query { query: jig_core_world::fixtures::OWN_TASKS },
    );
    world.send(root::Event::Core(core::Event::Delivered { watcher: Token::new(QUERY_FENCE_WATCHER) }));
    world.store.fault(jig_core_world::Fault::Hold { commits: 1 });
    let received = world.watches.as_ref().expect("configured fixture").deliveries.len();
    world.turn(1, 1, None, b"a committed reply");
    assert_eq!(world.watches.as_ref().expect("configured fixture").deliveries.len(), received);
    let commit = world.store.release_held().expect("held commit");
    world.send(root::Event::Committed { number: commit, fill: 0 });
    let (_, durable, document) = world
        .watches
        .as_ref()
        .expect("configured fixture")
        .deliveries
        .iter()
        .rev()
        .find(|(watcher, _, _)| *watcher == QUERY_FENCE_WATCHER)
        .expect("query update");
    assert!(*durable >= commit);
    let documents::Document::Lines(value) = document else { panic!("query") };
    assert_eq!(
        value.lines.get(0).expect("configured fixture").conversation.as_ref().expect("configured fixture").unread,
        documents::Count::Known(1)
    );
}

const QUERY_READ: u64 = 200;
const QUERY_NEXT_READ: u64 = 201;
const QUERY_REQUEST: u8 = 30;
const QUERY_MORE_REQUEST: u8 = 31;

#[test]
fn a_task_query_reads_a_partys_live_tasks_in_last_activity_order() {
    let mut world = world(1206);
    let older = world.assigned.as_ref().expect("first task").task;
    world.end(tasks::End::Parked, 0);
    world.advance(skein_lib::Duration::from_secs(1));
    let new_chat = jig_core_world::fixtures::chat(&world, 1, b"new task".as_slice().into());
    request(&mut world, QUERY_REQUEST, new_chat);
    let newer = world.assigned.as_ref().expect("second task").task;
    world.end(tasks::End::Parked, 0);
    world.advance(skein_lib::Duration::from_secs(1));
    request(
        &mut world,
        QUERY_MORE_REQUEST,
        people::Ask::Say { project: 1, task: older, words: b"new activity".as_slice().into() },
    );
    let query = documents::TaskQuery { order: documents::QueryOrder::Activity, ..jig_core_world::fixtures::OWN_TASKS };
    ask(&mut world, QUERY_READ, reads::Read::Tasks { query, live: None, ended: None });
    assert_eq!(jig_core_world::fixtures::query_numbers(&world), [older, newer]);
    let reads::Answer::Tasks(page) = latest(&world) else { panic!("query page") };
    assert_eq!(page.live_next, None);
}

#[test]
fn a_task_query_read_pages_its_live_group_and_then_ends() {
    let mut world = jig_core_world::stories::reads::world_with_rows(1207, 1);
    let mut expected = vec![world.assigned.as_ref().expect("first task").task];
    world.end(tasks::End::Parked, 0);
    let later_chat = jig_core_world::fixtures::chat(&world, 1, b"next task".as_slice().into());
    request(&mut world, QUERY_REQUEST, later_chat);
    expected.push(world.assigned.as_ref().expect("next task").task);
    let query = jig_core_world::fixtures::OWN_TASKS;
    ask(&mut world, QUERY_READ, reads::Read::Tasks { query, live: None, ended: None });
    assert_eq!(jig_core_world::fixtures::query_numbers(&world), expected[..1]);
    let reads::Answer::Tasks(page) = latest(&world) else { panic!("query page") };
    let Some(reads::Cursor::Live { scope, after, store_after }) = &page.live_next else { panic!("live cursor") };
    assert_eq!(scope, &reads::Scope::Tasks { query });
    assert_eq!(after, &documents::Cursor::Number { number: expected[0] });
    assert_eq!(store_after, &None);
    let from = page.live_next.clone();
    ask(&mut world, QUERY_NEXT_READ, reads::Read::Tasks { query, live: from, ended: None });
    assert_eq!(jig_core_world::fixtures::query_numbers(&world), expected[1..]);
    let reads::Answer::Tasks(page) = latest(&world) else { panic!("last query page") };
    assert_eq!(page.live_next, None);
}

#[test]
fn a_task_query_read_waits_for_the_commit_in_flight_and_survives_the_crash() {
    let mut world = world(1208);
    let task = world.assigned.as_ref().expect("task").task;
    let number = world.store.applied + 1;
    world.store.fault(jig_core_world::Fault::Hold { commits: 1 });
    world.cut = Some(jig_core_world::effects::Cut::Submitted(number));
    request(
        &mut world,
        QUERY_REQUEST,
        people::Ask::Say { project: 1, task, words: b"durable activity".as_slice().into() },
    );
    assert_eq!(world.reached_cut, Some(jig_core_world::effects::Cut::Submitted(number)));
    assert!(world.store.applied < number);
    ask(
        &mut world,
        QUERY_READ,
        reads::Read::Tasks { query: jig_core_world::fixtures::OWN_TASKS, live: None, ended: None },
    );
    assert!(world.reads.as_ref().expect("party").receipts.is_empty());
    let committed = world.store.release_held().expect("held commit");
    world.send(root::Event::Committed { number: committed, fill: 0 });
    assert_eq!(world.reads.as_ref().expect("party").receipts[0].1, number);
    assert_eq!(jig_core_world::fixtures::query_numbers(&world), [task]);
    let answer = latest(&world).clone();
    world.restart(1208, false);
    assert_eq!(world.reads.as_ref().expect("party").receipts.len(), 1);
    assert_eq!(latest(&world), &answer);
}

const CONVERSATION_TURN: u32 = 1;
const SEEN_CONVERSATION: u64 = 1;

#[test]
fn seen_over_a_conversations_turns_clears_its_lines_unread_count() {
    let mut world = world(1209);
    let task = world.assigned.as_ref().expect("conversation").task;
    world.turn(CONVERSATION_TURN, 0, None, b"a committed turn");
    let query = jig_core_world::fixtures::OWN_TASKS;
    ask(&mut world, QUERY_READ, reads::Read::Tasks { query, live: None, ended: None });
    let reads::Answer::Tasks(page) = latest(&world) else { panic!("task query") };
    let line = page.live.get(0).expect("conversation line");
    assert_eq!(line.conversation, Some(documents::ConversationRead { seen: 0, unread: documents::Count::Known(1) }));
    request(
        &mut world,
        QUERY_REQUEST,
        people::Ask::MarkSeen { seen: people::Seen::Conversation { task, through: SEEN_CONVERSATION } },
    );
    ask(&mut world, QUERY_NEXT_READ, reads::Read::Tasks { query, live: None, ended: None });
    let reads::Answer::Tasks(page) = latest(&world) else { panic!("task query") };
    let line = page.live.get(0).expect("conversation line");
    assert_eq!(
        line.conversation,
        Some(documents::ConversationRead { seen: SEEN_CONVERSATION, unread: documents::Count::Known(0) })
    );
}

#[test]
fn a_task_line_keeps_a_held_name_past_the_short_list_bound() {
    let (mut world, task, path) = jig_core_world::fixtures::long_resource_world(1210);
    ask(
        &mut world,
        QUERY_READ,
        reads::Read::Tasks {
            query: documents::TaskQuery { project: Some(1), requester: None, ..jig_core_world::fixtures::OWN_TASKS },
            live: None,
            ended: None,
        },
    );
    let reads::Answer::Tasks(page) = latest(&world) else { panic!("task query") };
    let line = page.live.iter().find(|line| line.chip.number == task).expect("resource task line served");
    let resource = line.chip.details.resources.get(0).expect("held resource");
    let documents::PartKey::Path(segments) = &resource.part.key else { panic!("resource path") };
    assert_eq!(segments.as_slice(), path.segments());
}

#[test]
fn a_query_without_index_keys_reads_the_requesters_live_tasks() {
    let mut world = world(1211);
    let task = world.assigned.as_ref().expect("live task").task;
    let query = documents::TaskQuery { requester: None, project: None, ..jig_core_world::fixtures::OWN_TASKS };
    ask(&mut world, QUERY_READ, reads::Read::Tasks { query, live: None, ended: None });
    let reads::Answer::Tasks(page) = latest(&world) else { panic!("task page: {:?}", latest(&world)) };
    assert_eq!(page.live.iter().map(|line| line.chip.number).collect::<Vec<_>>(), [task]);
    assert_eq!(page.ended, reads::EndedGroup::None);
}

#[test]
fn a_projectless_query_without_a_requester_keeps_the_asking_partys_tasks_alone() {
    let mut world = world(1212);
    let own = world.assigned.as_ref().expect("first party's task").task;
    let first = world.sign_in.expect("first party signed in");
    world.end(tasks::End::Parked, 0);
    let second = jig_core_world::fixtures::second_party(&mut world, b"query-second", people::Role::Owner);
    let other_partys_chat = jig_core_world::fixtures::chat(&world, 1, b"other party's task".as_slice().into());
    jig_core_world::fixtures::ask_as(&mut world, second, QUERY_MORE_REQUEST, other_partys_chat);
    let other = world.assigned.as_ref().expect("second party's task").task;
    assert_ne!(own, other);
    world.sign_in = Some(first);
    let query = documents::TaskQuery { requester: None, project: None, ..jig_core_world::fixtures::OWN_TASKS };
    ask(&mut world, QUERY_READ, reads::Read::Tasks { query, live: None, ended: None });
    let reads::Answer::Tasks(page) = latest(&world) else { panic!("task page: {:?}", latest(&world)) };
    assert_eq!(page.live.iter().map(|line| line.chip.number).collect::<Vec<_>>(), [own]);
}
