//! Owner joins count decoded records (protocol/store.md, 7.3).
use jig_domain_core::{self as core, reads};
use jig_domain_tasks as tasks;
use jig_protocol_store::{Event, Request, Write};
use jig_store_world::{World, root};
use skein_lib::{Token, Wall};

const GOAL: u64 = 1;
const CHILD: u64 = 2;
const GRANDCHILD: u64 = 3;
const ALICE: u64 = 1;
// The Small profile's store join bound (jig-protocol-config/src/profile/small.rs).
fn small_joins() -> u32 {
    jig_protocol_config::profile(jig_protocol_config::Profile::Small).store.joins
}
// A point read has one row; this fixture's bounded task records fit 1 MiB.
const POINT_ROWS: u32 = 1;
const POINT_BYTES: u64 = 1_048_576;

fn chain(ended: &[u64]) {
    let mut world = World::with_joins(18, small_joins());
    let mut records = Vec::new();
    for (number, requester, depth) in [
        (GOAL, tasks::Party::Person(ALICE), 0),
        (CHILD, tasks::Party::Task(GOAL), 1),
        (GRANDCHILD, tasks::Party::Task(CHILD), 2),
    ] {
        let core::Record::Tasks(tasks::Stored::Live(mut task)) = root::live_task(number) else {
            panic!("live task fixture")
        };
        task.requester = requester;
        task.root = GOAL;
        task.depth = depth;
        let stored = if ended.contains(&number) {
            task.phase = tasks::Phase::Ended(tasks::Ending::Failed { reason: Box::new([]) });
            task.ended_at = Some(Wall::EPOCH);
            tasks::Stored::Ended(task)
        } else {
            tasks::Stored::Live(task)
        };
        records.push(core::Record::Tasks(stored));
    }
    let mut writes = vec![Write::Save(root::header(1))];
    writes.extend(records.iter().cloned().map(Write::Save));
    world.commit(writes.into_boxed_slice()).expect("chain commits");
    let events = world.request(Request::Read {
        owner: Token::new(1),
        range: reads::Range {
            party: ALICE,
            scope: reads::Scope::Task { task: GRANDCHILD },
            after: None,
            backwards: false,
            rows: POINT_ROWS,
            bytes: POINT_BYTES,
        },
    });
    let [Event::Read { result: reads::Loaded::Page { rows, next: None }, .. }] = events.as_slice() else {
        panic!("one uncut point-read terminal: {events:?}")
    };
    let [row] = rows.as_ref() else { panic!("one task row: {rows:?}") };
    let [goal, child, grandchild] = records.as_slice() else { panic!("three task fixtures") };
    assert_eq!(&row.value, grandchild);
    assert_eq!(row.ancestors.as_ref(), [CHILD, GOAL]);
    let [parent, root] = row.linked.as_ref() else { panic!("both ancestors: {:?}", row.linked) };
    for (task, expected) in [(parent, child), (root, goal), (row.owner.as_ref().expect("task owner"), grandchild)] {
        let core::Record::Tasks(tasks::Stored::Live(expected) | tasks::Stored::Ended(expected)) = expected else {
            panic!("task fixture")
        };
        assert_eq!(task, expected);
    }
}

#[test]
fn an_ended_ancestry_reads_within_smalls_joins() {
    for ended in [&[GOAL, CHILD][..], &[GOAL, CHILD, GRANDCHILD][..]] {
        chain(ended);
    }
}

#[test]
fn a_live_ancestry_still_reads_within_smalls_joins() {
    chain(&[]);
}

// Four levels fit Small, the fifth does not. The labels are the levels in the story.
const FOURTH: u64 = 4;
const FIFTH: u64 = 5;
const SIBLING: u64 = 6;
const SHALLOW: u64 = 7;
const PROJECT: u32 = 1;
const SEED: u64 = 18;
// A page must be able to examine both indexed rows before its joins cut it.
const QUERY_ROWS: u32 = 2;

fn project_query() -> core::documents::TaskQuery {
    core::documents::TaskQuery {
        project: Some(PROJECT),
        requester: None,
        root: None,
        parent: None,
        label: None,
        phase: None,
        executor: None,
        tracked: None,
        order: core::documents::QueryOrder::Number,
    }
}

fn task_record(number: u64, requester: tasks::Party, depth: u32, ended: bool) -> tasks::TaskRecord {
    let core::Record::Tasks(tasks::Stored::Live(mut task)) = root::live_task(number) else {
        panic!("live task fixture")
    };
    task.requester = requester;
    task.root = match requester {
        tasks::Party::Task(_) => GOAL,
        tasks::Party::Person(_) | tasks::Party::Deployment { .. } => number,
    };
    task.depth = depth;
    task.flags = tasks::Flags { listed: true, ..tasks::Flags::PLAIN };
    task.phase = tasks::Phase::Waiting;
    task.run_since = None;
    if ended {
        task.phase =
            tasks::Phase::Ended(tasks::Ending::Done(tasks::TaskResult::Report { words: b"done".as_slice().into() }));
        task.ended_at = Some(Wall::from_nanos(number));
    }
    *task
}

fn task_writes(task: &tasks::TaskRecord, writes: &mut Vec<Write>) {
    let ended = matches!(task.phase, tasks::Phase::Ended(_));
    let stored =
        if ended { tasks::Stored::Ended(Box::new(task.clone())) } else { tasks::Stored::Live(Box::new(task.clone())) };
    writes.push(Write::Save(core::Record::Tasks(stored)));
    writes.push(Write::Save(core::Record::Tasks(tasks::Stored::Milestone(tasks::Milestone {
        task: task.number,
        position: 1,
        phase: task.phase.clone(),
        at: Wall::EPOCH,
    }))));
    if ended {
        writes.push(Write::Save(core::Record::Tasks(tasks::Stored::EndedByProject(tasks::EndedByProject {
            project: task.project,
            label: task.label,
            ended_at: task.ended_at.expect("ended time"),
            task: task.number,
        }))));
        if let tasks::Party::Task(requester) = task.requester {
            writes.push(Write::Save(core::Record::Tasks(tasks::Stored::EndedDelegate(tasks::Edge {
                requester,
                task: task.number,
            }))));
        }
    }
}

fn small_chain(
    levels: u64,
    ancestors_ended: bool,
    leaf_ended: bool,
    dependency: bool,
) -> (World, Vec<tasks::TaskRecord>) {
    // Use the complete shipped Small store, including its depth, rather than widening the tiny world fixture.
    let limits = jig_protocol_config::profile(jig_protocol_config::Profile::Small).store;
    let mut world = World::unopened_observed(SEED, limits, None);
    assert_eq!(world.open(), Some(0));
    let mut records = Vec::new();
    for number in GOAL..=levels {
        let requester = if number == GOAL { tasks::Party::Person(ALICE) } else { tasks::Party::Task(number - 1) };
        let ended = if number == levels { leaf_ended } else { ancestors_ended };
        records.push(task_record(number, requester, u32::try_from(number - 1).expect("five levels"), ended));
    }
    if dependency {
        records.last_mut().expect("leaf").dependencies = Box::new([SIBLING]);
        records.push(task_record(SIBLING, tasks::Party::Task(GRANDCHILD), 3, true));
    }
    let mut writes = vec![Write::Save(root::header(1))];
    for task in &records {
        task_writes(task, &mut writes);
    }
    world.commit(writes.into_boxed_slice()).expect("Small chain commits");
    (world, records)
}

fn read(world: &mut World, scope: reads::Scope, after: Option<reads::Position>) -> reads::Loaded {
    let events = world.request(Request::Read {
        owner: Token::new(1),
        range: reads::Range { party: ALICE, scope, after, backwards: false, rows: QUERY_ROWS, bytes: POINT_BYTES },
    });
    let mut events = events.into_iter();
    let Some(Event::Read { result, .. }) = events.next() else { panic!("one read terminal") };
    assert!(events.next().is_none());
    result
}

fn too_linked(result: &reads::Loaded) {
    assert!(matches!(result, reads::Loaded::TooLinked { joins } if *joins == small_joins()), "{result:?}");
}

#[test]
fn a_row_four_requesters_deep_reads_whole_on_small() {
    for ancestors_ended in [false, true] {
        let (mut world, _) = small_chain(FOURTH, ancestors_ended, true, false);
        for scope in [
            reads::Scope::Task { task: FOURTH },
            reads::Scope::EndedDelegates { task: GRANDCHILD },
            reads::Scope::History { task: FOURTH, subtree: false },
        ] {
            let result = read(&mut world, scope, None);
            let reads::Loaded::Page { rows, next: None } = &result else { panic!("whole row: {result:?}") };
            let [row] = rows.as_ref() else { panic!("one row: {rows:?}") };
            assert_eq!(row.owner.as_ref().expect("owner").number, FOURTH);
            assert_eq!(row.ancestors.as_ref(), [GRANDCHILD, CHILD, GOAL]);
            assert_eq!(row.linked.iter().map(|task| task.number).collect::<Vec<_>>(), [GRANDCHILD, CHILD, GOAL]);
        }
    }
}

#[test]
fn a_row_five_requesters_deep_is_too_linked_on_small() {
    let (mut world, _) = small_chain(FIFTH, false, true, false);
    for scope in [
        reads::Scope::Task { task: FIFTH },
        reads::Scope::History { task: FIFTH, subtree: false },
        reads::Scope::EndedDelegates { task: FOURTH },
    ] {
        too_linked(&read(&mut world, scope, None));
    }
}

#[test]
fn a_project_query_serves_the_rows_before_a_too_linked_one() {
    let (mut world, _) = small_chain(FIFTH, false, true, false);
    let shallow = task_record(SHALLOW, tasks::Party::Person(ALICE), 0, true);
    let mut writes = vec![Write::Save(root::header(2))];
    task_writes(&shallow, &mut writes);
    world.commit(writes.into_boxed_slice()).expect("shallow task commits");
    let scope = reads::Scope::Tasks { query: project_query() };
    let result = read(&mut world, scope.clone(), None);
    let reads::Loaded::Page { rows, next } = &result else { panic!("prefix: {result:?}") };
    let [row] = rows.as_ref() else { panic!("one shallow row: {rows:?}") };
    assert_eq!(row.owner.as_ref().expect("owner").number, SHALLOW);
    assert_eq!(next, &Some(reads::Position::Owner(row.value.key())));
    too_linked(&read(&mut world, scope, next.clone()));
}

#[test]
fn a_row_four_deep_with_a_sibling_dependency_is_too_linked_on_small() {
    let (mut world, _) = small_chain(FOURTH, false, true, true);
    too_linked(&read(&mut world, reads::Scope::Task { task: FOURTH }, None));
}

#[test]
fn a_task_query_lists_a_live_task_past_the_bound() {
    use jig_domain_people as people;
    use skein_lib::{Env, Queue, Time};
    let profile = jig_protocol_config::profile(jig_protocol_config::Profile::Small);
    let (mut world, records) = small_chain(FIFTH, false, false, false);
    let mut core = core::Core::new(jig_core_world::world::config(SEED).core, &profile.core).expect("core");
    let mut out = Queue::with_capacity(people::max_out(&profile.core.people));
    people::step(
        &mut core.people,
        &Env { now: Time::ZERO, wall: Wall::EPOCH, limits: profile.core.people },
        people::Event::Restore {
            record: people::Stored::Roles {
                project: PROJECT,
                holdings: Box::new([people::Holding { person: ALICE, role: people::Role::Owner }]),
            },
        },
        &mut out,
    );
    let mut out = Queue::with_capacity(tasks::max_out(&profile.core.tasks));
    for task in &records {
        tasks::step(
            &mut core.tasks,
            &Env { now: Time::ZERO, wall: Wall::EPOCH, limits: profile.core.tasks },
            tasks::Event::Restore { record: tasks::Stored::Live(Box::new(task.clone())) },
            &mut out,
        );
    }
    let query = core::documents::TaskQuery { requester: Some(tasks::Party::Task(FOURTH)), ..project_query() };
    let lines = core::documents::task_query::lines(
        &core,
        &profile.core,
        ALICE,
        &query,
        &core::documents::Window { rows: POINT_ROWS, after: None },
    )
    .expect("live query");
    assert_eq!(lines.lines.iter().map(|line| line.chip.number).collect::<Vec<_>>(), [FIFTH]);
    let mut ended = records.last().expect("fifth task").clone();
    ended.phase =
        tasks::Phase::Ended(tasks::Ending::Done(tasks::TaskResult::Report { words: b"done".as_slice().into() }));
    ended.ended_at = Some(Wall::from_nanos(FIFTH));
    let mut writes = vec![Write::Save(root::header(2)), Write::Erase(core::Key::Tasks(tasks::Key::Live(FIFTH)))];
    task_writes(&ended, &mut writes);
    world.commit(writes.into_boxed_slice()).expect("fifth task ends");
    too_linked(&read(&mut world, reads::Scope::Tasks { query }, None));
}
