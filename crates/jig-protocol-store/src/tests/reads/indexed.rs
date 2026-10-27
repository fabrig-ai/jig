//! Index pages use their physical keys and point-joined owners.
use super::{assert_empty, assert_too_linked, fixtures, linked_numbers, load, page, values};
use crate::Store;
use alloc::boxed::Box;
use fixtures::{ALICE, CHILD, GOAL, GRANDCHILD, PROJECT};
use jig_domain_core::{self as core, documents, reads};
use jig_domain_tasks as tasks;

fn owner_at(task: u64, at: u64, project: u32) -> core::Record {
    let core::Record::Tasks(tasks::Stored::Ended(mut owner)) = fixtures::ended_of(task, tasks::Party::Person(ALICE))
    else {
        panic!("ended fixture")
    };
    owner.ended_at = Some(skein_lib::Wall::from_nanos(at));
    owner.project = project;
    core::Record::Tasks(tasks::Stored::Ended(owner))
}

fn keep(store: &mut Store, rows: &[core::Record]) {
    let number = store.stores.store.last().checked_add(1).expect("next fixture commit");
    let mut writes = skein_lib::List::with_capacity(4);
    writes.push(fixtures::header(number)).unwrap();
    for row in rows {
        writes.push(fixtures::save(row.clone())).unwrap();
    }
    fixtures::committed(store, writes.into_boxed());
}

fn reply_object(position: u64) -> tasks::UpdateObject {
    tasks::UpdateObject::Reply { message: tasks::Newest::of(7), activity: tasks::Newest::of(position) }
}

fn reply_line(task: u64, position: u64, at: u64) -> core::Record {
    let core::Record::Tasks(tasks::Stored::Activity(mut line)) = fixtures::notified_line(task, position, ALICE) else {
        panic!("activity fixture")
    };
    line.at = skein_lib::Wall::from_nanos(at);
    core::Record::Tasks(tasks::Stored::Activity(line))
}

fn inbox_store() -> Store {
    let mut store = fixtures::opened();
    let mut rows = skein_lib::List::with_capacity(10);
    for (task, at, project) in [(GOAL, 10, PROJECT), (CHILD, 30, 7), (GRANDCHILD, 50, PROJECT)] {
        rows.push(owner_at(task, at, project)).unwrap();
        rows.push(fixtures::update(ALICE, at, task, tasks::UpdateObject::Result)).unwrap();
    }
    for (position, at) in [(1, 20), (2, 40)] {
        rows.push(reply_line(GOAL, position, at)).unwrap();
        rows.push(fixtures::update(ALICE, at, GOAL, reply_object(position))).unwrap();
    }
    for chunk in rows.as_slice().chunks(3) {
        keep(&mut store, chunk);
    }
    store
}

fn inbox_range(group: reads::Group) -> reads::Range {
    fixtures::read(ALICE, reads::Scope::Inbox { group, filter: reads::Filter { project: None, goal: None } })
}

fn position(record: core::Record) -> reads::Position {
    reads::Position::Owner(record.key())
}

fn owner_numbers(rows: &[reads::Row]) -> Box<[u64]> {
    let mut found = skein_lib::List::with_capacity(u32::try_from(rows.len()).unwrap());
    for row in rows {
        found.push(row.owner.as_ref().expect("point-joined owner").number).unwrap();
    }
    found.into_boxed()
}

fn subtree_store() -> Store {
    let mut store = fixtures::opened();
    let rows = [
        fixtures::ended_of(GOAL, tasks::Party::Person(ALICE)),
        fixtures::history(GOAL, 1),
        fixtures::hold_milestone(GOAL, 1),
        fixtures::delegate(GOAL, CHILD),
        fixtures::ended_of(CHILD, tasks::Party::Task(GOAL)),
        fixtures::history(CHILD, 2),
        fixtures::hold_milestone(CHILD, 2),
        fixtures::history(GRANDCHILD, 3),
    ];
    for chunk in rows.chunks(3) {
        keep(&mut store, chunk);
    }
    store
}

fn subtree_range() -> reads::Range {
    fixtures::read(ALICE, reads::Scope::History { task: GOAL, subtree: true })
}

#[test]
fn ended_delegates_read_their_requesters_index() {
    let mut store = fixtures::opened();
    keep(
        &mut store,
        &[
            fixtures::ended_of(GOAL, tasks::Party::Person(ALICE)),
            fixtures::ended_of(CHILD, tasks::Party::Task(GOAL)),
            fixtures::ended_delegate(GOAL, CHILD),
        ],
    );
    keep(&mut store, &[fixtures::ended_delegate(GOAL, CHILD), fixtures::ended_delegate(GRANDCHILD, GOAL)]);
    let (rows, next) = page(load(&mut store, fixtures::read(ALICE, reads::Scope::EndedDelegates { task: GOAL })));
    assert_eq!(values(&rows).as_ref(), [fixtures::ended_delegate(GOAL, CHILD)]);
    assert_eq!(owner_numbers(&rows).as_ref(), [CHILD]);
    assert_eq!(linked_numbers(rows.first().unwrap()).as_ref(), [GOAL]);
    assert_eq!(next, None);
}

#[test]
fn subtree_history_walks_each_delegates_history_in_turn() {
    let mut store = subtree_store();
    let (rows, next) = page(load(&mut store, subtree_range()));
    assert_eq!(
        values(&rows).as_ref(),
        [
            fixtures::history(GOAL, 1),
            fixtures::hold_milestone(GOAL, 1),
            fixtures::history(CHILD, 2),
            fixtures::hold_milestone(CHILD, 2)
        ]
    );
    assert_eq!(owner_numbers(&rows).as_ref(), [GOAL, GOAL, CHILD, CHILD]);
    assert_eq!(next, None);
}

#[test]
fn a_subtree_cursor_resumes_at_its_delegates_index_row() {
    let mut store = subtree_store();
    let mut range = subtree_range();
    range.after = Some(position(fixtures::history(CHILD, 2)));
    let (rows, next) = page(load(&mut store, range));
    assert_eq!(values(&rows).as_ref(), [fixtures::hold_milestone(CHILD, 2)]);
    assert_eq!(owner_numbers(&rows).as_ref(), [CHILD]);
    assert_eq!(next, None);
}

#[test]
fn inbox_results_read_newest_first_within_the_group() {
    let mut store = inbox_store();
    let (rows, next) = page(load(&mut store, inbox_range(reads::Group::Results)));
    assert_eq!(
        values(&rows).as_ref(),
        [
            fixtures::update(ALICE, 50, GRANDCHILD, tasks::UpdateObject::Result),
            fixtures::update(ALICE, 30, CHILD, tasks::UpdateObject::Result),
            fixtures::update(ALICE, 10, GOAL, tasks::UpdateObject::Result)
        ]
    );
    assert_eq!(owner_numbers(&rows).as_ref(), [GRANDCHILD, CHILD, GOAL]);
    for row in &rows {
        assert!(ended(&row.owner.as_ref().unwrap().phase));
    }
    assert_eq!(next, None);
}

#[test]
fn an_inbox_filter_keeps_one_projects_rows() {
    let mut store = inbox_store();
    let mut range = inbox_range(reads::Group::Results);
    let reads::Scope::Inbox { filter, .. } = &mut range.scope else { panic!("inbox range") };
    filter.project = Some(PROJECT);
    let (rows, next) = page(load(&mut store, range));
    assert_eq!(owner_numbers(&rows).as_ref(), [GRANDCHILD, GOAL]);
    assert_eq!(next, None);
}

#[test]
fn ended_tasks_read_newest_first_from_the_index() {
    let mut store = ended_store();
    let (rows, next) = page(load(&mut store, ended_range()));
    assert_eq!(values(&rows).as_ref(), [ended_party(GRANDCHILD), ended_party(CHILD), ended_party(GOAL)]);
    assert_eq!(owner_numbers(&rows).as_ref(), [GRANDCHILD, CHILD, GOAL]);
    assert_eq!(next, None);
}

#[test]
fn inbox_replies_join_the_indexed_activity_line() {
    let mut store = inbox_store();
    let (rows, next) = page(load(&mut store, inbox_range(reads::Group::Replies)));
    assert_eq!(
        values(&rows).as_ref(),
        [fixtures::update(ALICE, 40, GOAL, reply_object(2)), fixtures::update(ALICE, 20, GOAL, reply_object(1))]
    );
    assert_eq!(rows.first().unwrap().related, Some(reply_line(GOAL, 2, 40)));
    assert_eq!(
        rows.first().unwrap().inbox,
        Some(documents::Cursor::Inbox {
            task: GOAL,
            object: documents::InboxObject::Reply { message: 7 },
            at: Some(skein_lib::Wall::from_nanos(40))
        })
    );
    assert_eq!(next, None);
}

#[test]
fn live_inbox_groups_have_no_durable_index_page() {
    let mut store = inbox_store();
    assert_empty(&load(&mut store, inbox_range(reads::Group::Needs)));
}

#[test]
fn an_inbox_page_that_skipped_rows_goes_on_from_the_last_examined() {
    let mut store = inbox_store();
    let mut range = inbox_range(reads::Group::Results);
    range.rows = 1;
    range.after = Some(position(fixtures::update(ALICE, 50, GRANDCHILD, tasks::UpdateObject::Result)));
    let (rows, next) = page(load(&mut store, range.clone()));
    assert!(rows.is_empty());
    assert_eq!(next, Some(position(fixtures::update(ALICE, 40, GOAL, reply_object(2)))));
    range.after = next;
    let (rows, next) = page(load(&mut store, range));
    assert_eq!(owner_numbers(&rows).as_ref(), [CHILD]);
    assert_eq!(next, Some(position(fixtures::update(ALICE, 30, CHILD, tasks::UpdateObject::Result))));
}

#[test]
fn a_page_that_returned_none_within_its_bound_is_cut_not_ended() {
    let mut store = ended_store();
    let number = store.stores.store.last().checked_add(1).expect("next fixture commit");
    fixtures::committed(
        &mut store,
        Box::new([fixtures::header(number), crate::Write::Erase(core::Key::Tasks(tasks::Key::Ended(GRANDCHILD)))]),
    );
    let mut range = ended_range();
    range.rows = 1;
    let (rows, next) = page(load(&mut store, range.clone()));
    assert!(rows.is_empty());
    assert_eq!(next, Some(position(ended_party(GRANDCHILD))));
    range.after = next;
    let (rows, _next) = page(load(&mut store, range));
    assert_eq!(owner_numbers(&rows).as_ref(), [CHILD]);
}

#[test]
fn a_subtree_history_page_moves_from_one_task_to_the_next() {
    let mut store = subtree_store();
    let mut range = subtree_range();
    range.rows = 2;
    let (rows, next) = page(load(&mut store, range.clone()));
    assert_eq!(values(&rows).as_ref(), [fixtures::history(GOAL, 1), fixtures::hold_milestone(GOAL, 1)]);
    assert_eq!(next, Some(position(fixtures::hold_milestone(GOAL, 1))));
    range.after = next;
    let (rows, next) = page(load(&mut store, range));
    assert_eq!(values(&rows).as_ref(), [fixtures::history(CHILD, 2), fixtures::hold_milestone(CHILD, 2)]);
    assert_eq!(next, None);
}

#[test]
fn an_index_join_cut_keeps_its_accepted_prefix() {
    let mut store = ended_store();
    store.limits.joins = 3;
    let (rows, next) = page(load(&mut store, ended_range()));
    assert_eq!(owner_numbers(&rows).as_ref(), [GRANDCHILD]);
    assert_eq!(next, Some(position(ended_party(GRANDCHILD))));
}

#[test]
fn an_index_byte_cut_keeps_its_accepted_prefix() {
    let mut store = ended_store();
    let (rows, _next) = page(load(&mut store, ended_range()));
    let mut range = ended_range();
    range.bytes = rows.first().unwrap().owned_bytes().unwrap();
    let (rows, next) = page(load(&mut store, range));
    assert_eq!(owner_numbers(&rows).as_ref(), [GRANDCHILD]);
    assert_eq!(next, Some(position(ended_party(GRANDCHILD))));
}

#[test]
fn inbox_goal_filters_select_on_the_joined_ancestry() {
    let mut store = inbox_store();
    keep(&mut store, &[fixtures::ended_of(CHILD, tasks::Party::Task(GOAL))]);
    let mut range = inbox_range(reads::Group::Results);
    let reads::Scope::Inbox { filter, .. } = &mut range.scope else { panic!("inbox range") };
    filter.goal = Some(GOAL);
    let (rows, next) = page(load(&mut store, range));
    assert_eq!(owner_numbers(&rows).as_ref(), [CHILD, GOAL]);
    assert_eq!(next, None);
}

fn ended(phase: &tasks::Phase) -> bool {
    match phase {
        tasks::Phase::Ended(_) => true,
        tasks::Phase::Waiting | tasks::Phase::Active(_) | tasks::Phase::Closing(_) | tasks::Phase::Held { .. } => false,
    }
}

#[test]
fn a_subtree_cursor_advances_past_rows_whose_owner_is_gone() {
    let mut store = subtree_store();
    let number = store.stores.store.last().checked_add(1).unwrap();
    fixtures::committed(
        &mut store,
        Box::new([fixtures::header(number), crate::Write::Erase(core::Key::Tasks(tasks::Key::Ended(CHILD)))]),
    );
    let mut range = subtree_range();
    range.rows = 1;
    range.after = Some(position(fixtures::hold_milestone(GOAL, 1)));
    let (rows, next) = page(load(&mut store, range.clone()));
    assert!(rows.is_empty());
    assert_eq!(next, Some(position(fixtures::history(CHILD, 2))));
    range.after = next;
    let (rows, next) = page(load(&mut store, range));
    assert!(rows.is_empty());
    assert_eq!(next, None);
}

fn task_query() -> documents::TaskQuery {
    documents::TaskQuery {
        project: None,
        requester: Some(tasks::Party::Person(ALICE)),
        root: None,
        parent: None,
        label: Some(2),
        phase: None,
        executor: None,
        tracked: None,
        order: documents::QueryOrder::Number,
    }
}

fn ended_index_store() -> Store {
    let mut store = fixtures::opened();
    for (task, at, label, listed) in [(GOAL, 10, 2, true), (CHILD, 30, 3, true), (GRANDCHILD, 50, 2, false)] {
        let core::Record::Tasks(tasks::Stored::Ended(mut owner)) = owner_at(task, at, PROJECT) else {
            panic!("ended owner")
        };
        owner.label = label;
        owner.flags.listed = listed;
        keep(
            &mut store,
            &[
                core::Record::Tasks(tasks::Stored::Ended(owner)),
                core::Record::Tasks(tasks::Stored::EndedByParty(tasks::EndedByParty {
                    party: ALICE,
                    label,
                    ended_at: skein_lib::Wall::from_nanos(at),
                    task,
                })),
            ],
        );
        if listed {
            keep(
                &mut store,
                &[core::Record::Tasks(tasks::Stored::EndedByProject(tasks::EndedByProject {
                    project: PROJECT,
                    label,
                    ended_at: skein_lib::Wall::from_nanos(at),
                    task,
                }))],
            );
        }
    }
    store
}

#[test]
fn ended_tasks_read_one_partys_label_newest_first() {
    let mut store = ended_index_store();
    let result = load(&mut store, fixtures::read(ALICE, reads::Scope::Tasks { query: task_query() }));
    let (rows, next) = page(result);
    assert_eq!(owner_numbers(&rows).as_ref(), [GRANDCHILD, GOAL]);
    assert_eq!(next, None);
}

#[test]
fn a_projects_ended_part_holds_listed_tasks_only() {
    let mut store = ended_index_store();
    let query = documents::TaskQuery { project: Some(PROJECT), requester: None, label: None, ..task_query() };
    let result = load(&mut store, fixtures::read(ALICE, reads::Scope::Tasks { query }));
    let (rows, next) = page(result);
    assert_eq!(owner_numbers(&rows).as_ref(), [GOAL, CHILD]);
    assert_eq!(next, None);
}

#[test]
fn an_ended_query_joins_its_requesters_position_for_another_reader() {
    let mut store = ended_index_store();
    let core::Record::Tasks(tasks::Stored::Ended(mut owner)) = owner_at(GOAL, 10, PROJECT) else {
        panic!("ended owner")
    };
    owner.label = 2;
    owner.flags.conversation = true;
    owner.last_reply = 3;
    keep(
        &mut store,
        &[
            core::Record::Tasks(tasks::Stored::Ended(owner)),
            core::Record::People(jig_domain_people::Stored::EndedRead { task: GOAL, party: ALICE, through: 2 }),
        ],
    );
    let result = load(&mut store, fixtures::read(ALICE + 1, reads::Scope::Tasks { query: task_query() }));
    let (rows, next) = page(result);
    assert_eq!(owner_numbers(&rows).as_ref(), [GRANDCHILD, GOAL]);
    assert_eq!(rows[1].ended_read, Some(2));
    assert_eq!(next, None);
}

fn ended_store() -> Store {
    let mut store = fixtures::opened();
    for task in [GOAL, CHILD, GRANDCHILD] {
        keep(&mut store, &[owner_at(task, task, PROJECT), ended_party(task)]);
    }
    store
}

fn ended_range() -> reads::Range {
    let mut range = fixtures::read(ALICE, reads::Scope::Tasks { query: task_query() });
    range.backwards = true;
    range
}

fn ended_party(task: u64) -> core::Record {
    core::Record::Tasks(tasks::Stored::EndedByParty(tasks::EndedByParty {
        party: ALICE,
        label: 2,
        ended_at: skein_lib::Wall::from_nanos(task),
        task,
    }))
}

#[test]
fn an_index_first_row_past_its_joins_names_the_bound() {
    const OWNER_JOINS: u32 = 1; // A conversation needs its ended read-position join too.
    let mut store = ended_store();
    store.limits.joins = OWNER_JOINS;
    let result = load(&mut store, ended_range());
    assert_too_linked(&result, OWNER_JOINS);
}

#[test]
fn an_inbox_first_row_past_its_joins_names_the_bound() {
    const OWNER_JOINS: u32 = 1;
    let mut store = inbox_store();
    store.limits.joins = OWNER_JOINS;
    let result = load(&mut store, inbox_range(reads::Group::Results));
    assert_too_linked(&result, OWNER_JOINS);
}
