//! Parties' reads through the memory component (protocol/store.md, 7.3).
use super::fixtures::{self, ALICE, CHILD, GOAL, GRANDCHILD};
use crate::{Arrived, Below, Event, Io, Request, Store};
use alloc::boxed::Box;
use jig_domain_core::{self as core, reads};
use jig_domain_tasks as tasks;
use skein_io::file;
use skein_lib::{Queue, Token};

fn child_store() -> Store {
    let mut store = fixtures::opened();
    fixtures::committed(
        &mut store,
        Box::new([
            fixtures::header(1),
            fixtures::save(fixtures::ended_of(GOAL, tasks::Party::Person(ALICE))),
            fixtures::save(fixtures::ended_of(CHILD, tasks::Party::Task(GOAL))),
            fixtures::save(fixtures::terminal(CHILD, 1)),
        ]),
    );
    store
}

fn load(store: &mut Store, range: reads::Range) -> reads::Loaded {
    let events = fixtures::run(store, Request::Read { owner: Token::new(7), range });
    let mut events = events.into_iter();
    let Some(Event::Read { owner, result }) = events.next() else { panic!("one read event") };
    assert_eq!(owner, Token::new(7));
    assert!(events.next().is_none());
    result
}

fn one_row(result: &reads::Loaded) -> &reads::Row {
    let reads::Loaded::Page { rows, next: None } = result else { panic!("one page: {result:?}") };
    let [row] = rows.as_ref() else { panic!("one row: {rows:?}") };
    row
}

fn assert_too_linked(result: &reads::Loaded, expected: u32) {
    let reads::Loaded::TooLinked { joins } = result else { panic!("join refusal: {result:?}") };
    assert_eq!(*joins, expected);
}

fn assert_empty(result: &reads::Loaded) {
    let reads::Loaded::Page { rows, next: None } = result else { panic!("one page: {result:?}") };
    assert!(rows.is_empty());
}

fn linked_numbers(row: &reads::Row) -> Box<[u64]> {
    let mut numbers = skein_lib::List::with_capacity(8);
    for task in &row.linked {
        numbers.push(task.number).expect("small fixture tree");
    }
    numbers.into_boxed()
}

#[test]
fn an_ended_task_read_joins_its_terminal_and_ancestry() {
    let mut store = child_store();
    let result = load(&mut store, fixtures::read(ALICE, reads::Scope::Task { task: CHILD }));
    let row = one_row(&result);
    assert_eq!(row.ancestors.as_ref(), [GOAL]);
    assert_eq!(row.related, Some(fixtures::terminal(CHILD, 1)));
}

#[test]
fn a_live_requester_joins_before_an_ended_one() {
    let mut store = child_store();
    let core::Record::Tasks(tasks::Stored::Ended(mut live)) = fixtures::ended_of(GOAL, tasks::Party::Person(ALICE))
    else {
        panic!("task fixture");
    };
    live.attempt = 2;
    fixtures::committed(
        &mut store,
        Box::new([fixtures::header(2), fixtures::save(core::Record::Tasks(tasks::Stored::Live(live)))]),
    );
    let result = load(&mut store, fixtures::read(ALICE, reads::Scope::Task { task: CHILD }));
    let [parent] = &*one_row(&result).linked else { panic!("one parent") };
    assert_eq!(parent.attempt, 2);
}

#[test]
fn ancestry_stops_at_the_domains_depth_limit() {
    let mut store = child_store();
    store.limits.core.tasks.depth = 1;
    fixtures::committed(
        &mut store,
        Box::new([fixtures::header(2), fixtures::save(fixtures::ended_of(GRANDCHILD, tasks::Party::Task(CHILD)))]),
    );
    let result = load(&mut store, fixtures::read(ALICE, reads::Scope::Task { task: GRANDCHILD }));
    assert_eq!(one_row(&result).ancestors.as_ref(), [CHILD]);
}

#[test]
fn links_join_dependencies_and_delegates_once_each() {
    let mut store = child_store();
    fixtures::committed(
        &mut store,
        Box::new([
            fixtures::header(2),
            fixtures::save(fixtures::with_links(
                fixtures::ended_of(CHILD, tasks::Party::Task(GOAL)),
                &[GOAL, GRANDCHILD],
            )),
            fixtures::save(fixtures::ended_of(GRANDCHILD, tasks::Party::Person(ALICE))),
        ]),
    );
    let result = load(&mut store, fixtures::read(ALICE, reads::Scope::Task { task: CHILD }));
    assert_eq!(linked_numbers(one_row(&result)).as_ref(), [GOAL, GRANDCHILD]);
}

#[test]
fn a_missing_task_reads_as_an_empty_page() {
    let mut store = child_store();
    let result = load(&mut store, fixtures::read(ALICE, reads::Scope::Task { task: GRANDCHILD }));
    assert_empty(&result);
}

#[test]
fn a_page_stops_before_the_row_past_its_bytes() {
    let mut store = child_store();
    let mut range = fixtures::read(ALICE, reads::Scope::Task { task: CHILD });
    range.bytes = 1;
    let result = load(&mut store, range);
    assert_empty(&result);
}

#[test]
fn a_row_past_the_whole_join_bound_is_refused_by_name() {
    // The ancestor fits, but the task's terminal needs a second join.
    const ANCESTOR_JOINS: u32 = 1;
    let mut store = child_store();
    store.limits.joins = ANCESTOR_JOINS;
    let result = load(&mut store, fixtures::read(ALICE, reads::Scope::Task { task: CHILD }));
    assert_too_linked(&result, ANCESTOR_JOINS);
}

fn plain_ended(task: u64, requester: tasks::Party) -> core::Record {
    let core::Record::Tasks(tasks::Stored::Ended(mut row)) = fixtures::ended_of(task, requester) else {
        panic!("ended fixture")
    };
    row.flags = tasks::Flags::PLAIN;
    core::Record::Tasks(tasks::Stored::Ended(row))
}

#[test]
fn a_later_row_past_the_bound_cuts_the_page_before_it() {
    // The first project-indexed owner costs one join; the child also needs its ancestor.
    const OWNER_JOINS: u32 = 1;
    let mut store = child_store();
    fixtures::committed(
        &mut store,
        Box::new([
            fixtures::header(2),
            fixtures::save(plain_ended(GOAL, tasks::Party::Person(ALICE))),
            fixtures::save(plain_ended(CHILD, tasks::Party::Task(GOAL))),
        ]),
    );
    let first = core::Record::Tasks(tasks::Stored::EndedByProject(tasks::EndedByProject {
        project: fixtures::PROJECT,
        label: 0,
        ended_at: skein_lib::Wall::from_nanos(2),
        task: GOAL,
    }));
    let second = core::Record::Tasks(tasks::Stored::EndedByProject(tasks::EndedByProject {
        project: fixtures::PROJECT,
        label: 0,
        ended_at: skein_lib::Wall::from_nanos(1),
        task: CHILD,
    }));
    fixtures::committed(
        &mut store,
        Box::new([fixtures::header(3), fixtures::save(first.clone()), fixtures::save(second)]),
    );
    store.limits.joins = OWNER_JOINS;
    let query = core::documents::TaskQuery {
        project: Some(fixtures::PROJECT),
        requester: None,
        root: None,
        parent: None,
        label: None,
        phase: None,
        executor: None,
        tracked: None,
        order: core::documents::QueryOrder::Number,
    };
    let mut range = fixtures::read(ALICE, reads::Scope::Tasks { query });
    let (rows, next) = page(load(&mut store, range.clone()));
    let [row] = rows.as_ref() else { panic!("one prefix row: {rows:?}") };
    assert_eq!(row.value, first);
    assert_eq!(next, Some(reads::Position::Owner(first.key())));
    range.after = next;
    let result = load(&mut store, range);
    assert_too_linked(&result, OWNER_JOINS);
}

#[test]
fn task_scopes_share_their_joined_owner() {
    let mut store = child_store();
    for scope in [
        reads::Scope::Task { task: CHILD },
        reads::Scope::PersonTask { task: CHILD },
        reads::Scope::Result { task: CHILD },
    ] {
        let result = load(&mut store, fixtures::read(ALICE, scope));
        assert_eq!(one_row(&result).value, fixtures::ended_of(CHILD, tasks::Party::Task(GOAL)));
    }
}

#[test]
fn a_point_cursor_excludes_the_owner_already_returned() {
    let mut store = child_store();
    let mut range = fixtures::read(ALICE, reads::Scope::Task { task: CHILD });
    range.after = Some(reads::Position::Owner(core::Key::Tasks(tasks::Key::Ended(CHILD))));
    let result = load(&mut store, range);
    assert_empty(&result);
}

#[test]
fn a_zero_row_bound_keeps_no_point_row() {
    let mut store = child_store();
    let mut range = fixtures::read(ALICE, reads::Scope::Task { task: CHILD });
    range.rows = 0;
    let result = load(&mut store, range);
    assert_empty(&result);
}

const PROPOSAL: u64 = 43;
const OTHER_PROPOSAL: u64 = 47;
const KEPT_REVISION: u64 = 3;
const HOLD_POSITION: u64 = 13;
const OTHER_HOLD_POSITION: u64 = 17;
const QUESTION: u64 = 5;
const OLD_QUESTION: u64 = 3;
const QUESTION_POSITION: u64 = 7;
const ANSWER_POSITION: u64 = 11;

fn history_store() -> Store {
    let mut store = fixtures::opened();
    let owner = fixtures::with_proposal_rows(
        fixtures::ended_of(CHILD, tasks::Party::Person(ALICE)),
        &[tasks::ProposalRow { proposal: PROPOSAL, revision: KEPT_REVISION }],
    );
    fixtures::committed(&mut store, Box::new([fixtures::header(1), fixtures::save(owner)]));
    fixtures::committed(
        &mut store,
        Box::new([
            fixtures::header(2),
            fixtures::save(fixtures::history_with_proposal(CHILD, 1, PROPOSAL)),
            fixtures::save(fixtures::history_with_proposal(CHILD, 2, PROPOSAL)),
            fixtures::save(fixtures::history_with_proposal(CHILD, 3, PROPOSAL)),
        ]),
    );
    fixtures::committed(
        &mut store,
        Box::new([
            fixtures::header(3),
            fixtures::save(fixtures::history_with_proposal(CHILD, 4, PROPOSAL)),
            fixtures::save(fixtures::history_with_proposal(CHILD, 5, PROPOSAL)),
        ]),
    );
    store
}

fn escalation_store() -> Store {
    let mut store = fixtures::opened();
    let owner =
        fixtures::with_hold(fixtures::ended_of(CHILD, tasks::Party::Person(ALICE)), HOLD_POSITION, KEPT_REVISION);
    fixtures::committed(
        &mut store,
        Box::new([
            fixtures::header(1),
            fixtures::save(owner),
            fixtures::save(fixtures::escalation_decision(CHILD, 1)),
            fixtures::save(fixtures::escalation_decision(CHILD, 3)),
        ]),
    );
    fixtures::committed(
        &mut store,
        Box::new([
            fixtures::header(2),
            fixtures::save(fixtures::escalation_decision(CHILD, 5)),
            fixtures::save(fixtures::hold_milestone(CHILD, HOLD_POSITION)),
            fixtures::save(fixtures::hold_milestone(CHILD, OTHER_HOLD_POSITION)),
        ]),
    );
    store
}

fn question_store() -> Store {
    let mut store = fixtures::opened();
    let owner = fixtures::with_question_lines(
        fixtures::ended_of(CHILD, tasks::Party::Person(ALICE)),
        &[tasks::QuestionLine {
            message: QUESTION,
            line: QUESTION_POSITION,
            answer: Some(tasks::AnswerLine { task: GRANDCHILD, position: ANSWER_POSITION }),
        }],
    );
    fixtures::committed(
        &mut store,
        Box::new([
            fixtures::header(1),
            fixtures::save(owner),
            fixtures::save(fixtures::question_line(CHILD, QUESTION_POSITION, QUESTION)),
            fixtures::save(fixtures::answer_line(GRANDCHILD, ANSWER_POSITION, QUESTION)),
        ]),
    );
    store
}

fn escalation_range() -> reads::Range {
    fixtures::read(ALICE, reads::Scope::Escalation { task: CHILD })
}

fn question_range(message: u64) -> reads::Range {
    fixtures::read(ALICE, reads::Scope::Question { task: CHILD, message })
}

fn proposal_range(proposal: u64) -> reads::Range {
    fixtures::read(ALICE, reads::Scope::Proposal { proposer: jig_domain_people::Proposer::Task(CHILD), proposal })
}

fn assert_gone(result: reads::Loaded) {
    let reads::Loaded::Gone = result else { panic!("gone: {result:?}") };
}

#[test]
fn a_party_proposal_joins_its_kept_decision() {
    let mut store = fixtures::opened();
    fixtures::committed(
        &mut store,
        Box::new([
            fixtures::header(1),
            fixtures::save(fixtures::person_proposal(PROPOSAL, true)),
            fixtures::save(fixtures::proposal_decision(ALICE, PROPOSAL)),
        ]),
    );
    let range = fixtures::read(
        ALICE,
        reads::Scope::Proposal { proposer: jig_domain_people::Proposer::Party(ALICE), proposal: PROPOSAL },
    );
    let result = load(&mut store, range);
    assert_eq!(one_row(&result).related, Some(fixtures::proposal_decision(ALICE, PROPOSAL)));
}

#[test]
fn a_task_proposal_reads_its_proposers_history_row_by_key() {
    let mut store = history_store();
    let result = load(&mut store, proposal_range(PROPOSAL));
    assert_eq!(one_row(&result).value, fixtures::history_with_proposal(CHILD, 3, PROPOSAL));
}

#[test]
fn an_escalation_reads_the_decision_its_task_names() {
    let mut store = escalation_store();
    let result = load(&mut store, escalation_range());
    assert_eq!(one_row(&result).value, fixtures::escalation_decision(CHILD, 3));
}

#[test]
fn an_escalation_joins_its_hold_milestone_by_position() {
    let mut store = escalation_store();
    let result = load(&mut store, escalation_range());
    assert_eq!(one_row(&result).related, Some(fixtures::hold_milestone(CHILD, 13)));
}

#[test]
fn a_question_joins_the_line_that_answers_it() {
    let mut store = question_store();
    let result = load(&mut store, question_range(QUESTION));
    assert_eq!(one_row(&result).related, Some(fixtures::answer_line(GRANDCHILD, 11, QUESTION)));
}

#[test]
fn a_question_older_than_the_kept_lines_reads_as_gone() {
    let mut store = question_store();
    fixtures::committed(
        &mut store,
        Box::new([fixtures::header(2), fixtures::save(fixtures::question_line(CHILD, 1, OLD_QUESTION))]),
    );
    let result = load(&mut store, question_range(OLD_QUESTION));
    assert_gone(result);
}

#[test]
fn a_reply_reads_its_notified_line_by_key() {
    let mut store = child_store();
    fixtures::committed(
        &mut store,
        Box::new([fixtures::header(2), fixtures::save(fixtures::notified_line(CHILD, QUESTION_POSITION, ALICE))]),
    );
    let range = fixtures::read(ALICE, reads::Scope::Reply { task: CHILD, activity: QUESTION_POSITION });
    let result = load(&mut store, range);
    assert_eq!(one_row(&result).value, fixtures::notified_line(CHILD, 7, ALICE));
}

#[test]
fn a_proposal_older_than_the_kept_rows_reads_as_gone() {
    let mut store = history_store();
    fixtures::committed(
        &mut store,
        Box::new([fixtures::header(4), fixtures::save(fixtures::history_with_proposal(CHILD, 6, OTHER_PROPOSAL))]),
    );
    let result = load(&mut store, proposal_range(OTHER_PROPOSAL));
    assert_gone(result);
}

#[test]
fn a_turn_waits_for_its_transcript_handler() {
    let (mut store, disk) = party_store();
    let below = waiting_party(&mut store, turn_range());
    let result = read_disk(&mut store, below, &disk);
    assert_eq!(one_row(&result).value, expected_turn());
}

#[test]
fn a_turn_reply_waits_for_its_transcript_evidence() {
    let (mut store, disk) = party_store();
    let below = waiting_party(&mut store, reply_range());
    let result = read_disk(&mut store, below, &disk);
    assert_eq!(one_row(&result).related, Some(expected_turn()));
}

fn expected_turn() -> core::Record {
    core::Record::Core(core::CoreRecord::Turn(fixtures::turn(CHILD, 1, 2, b"two")))
}

fn turn_range() -> reads::Range {
    fixtures::read(ALICE, reads::Scope::Turn { task: CHILD, attempt: 1, turn: 2 })
}

fn reply_range() -> reads::Range {
    fixtures::read(ALICE, reads::Scope::Reply { task: CHILD, activity: QUESTION_POSITION })
}

fn turn_reply() -> core::Record {
    core::Record::Tasks(tasks::Stored::Activity(tasks::ActivityLine {
        task: CHILD,
        position: QUESTION_POSITION,
        at: skein_lib::Wall::from_nanos(19),
        line: tasks::Line::Turn { reply: 1, attempt: 1, turn: 2, read: None },
    }))
}

fn party_store() -> (Store, Box<[u8]>) {
    let mut store = fixtures::opened();
    fixtures::committed(
        &mut store,
        Box::new([
            fixtures::header(1),
            fixtures::save(fixtures::ended_of(CHILD, tasks::Party::Person(ALICE))),
            fixtures::save(turn_reply()),
        ]),
    );
    let turns = [fixtures::turn(CHILD, 1, 1, b"one"), fixtures::turn(CHILD, 1, 2, b"two")];
    let mut above = Queue::uncapped();
    let mut below = fixtures::below();
    crate::down(
        &mut store,
        &fixtures::env(),
        fixtures::commit(
            2,
            Box::new([
                fixtures::header(2),
                fixtures::save(core::Record::Core(core::CoreRecord::Turn(turns[0].clone()))),
                fixtures::save(core::Record::Core(core::CoreRecord::Turn(turns[1].clone()))),
            ]),
        ),
        &mut above,
        &mut below,
    );
    assert!(above.is_empty());
    let disk = fixtures::written(&mut store, below, &turns);
    (store, disk)
}

fn party_submit(store: &mut Store, owner: Token, range: reads::Range) -> (Box<[Event]>, Below) {
    let mut above = Queue::uncapped();
    let mut below = fixtures::below();
    crate::down(store, &fixtures::env(), Request::Read { owner, range }, &mut above, &mut below);
    (fixtures::events(&mut above), below)
}

fn waiting_party(store: &mut Store, range: reads::Range) -> Below {
    let (events, below) = party_submit(store, Token::new(7), range);
    assert!(events.is_empty());
    below
}

fn corrupt_disk(disk: &mut [u8]) {
    *disk.get_mut(5).expect("frame byte") ^= 1;
}

fn read_result(events: Box<[Event]>) -> reads::Loaded {
    let mut events = events.into_iter();
    let Some(Event::Read { owner, result }) = events.next() else { panic!("one party read") };
    assert_eq!(owner, Token::new(7));
    assert!(events.next().is_none());
    result
}

fn read_disk(store: &mut Store, mut below: Below, disk: &[u8]) -> reads::Loaded {
    for _ in 0_u32..8 {
        let Some(file::Request::ReadAt { owner, offset, max, .. }) = below.transcripts.pop() else {
            panic!("one frame read request");
        };
        assert!(below.transcripts.is_empty());
        let first = usize::try_from(offset).expect("fixture offset");
        let end = first.checked_add(usize::try_from(max).expect("fixture read length")).expect("fixture interval");
        let bytes = Box::from(disk.get(first..end).expect("read within committed disk"));
        let (events, next) =
            fixtures::answer(store, Io::Transcripts(Arrived { event: file::Event::Read { owner, bytes }, fd: None }));
        below = next;
        if !events.is_empty() {
            assert!(below.transcripts.is_empty());
            return read_result(events);
        }
    }
    panic!("two fixture frames finish within four reads");
}

#[test]
fn a_turn_read_answers_once_its_frame_is_read() {
    let (mut store, disk) = party_store();
    let below = waiting_party(&mut store, turn_range());
    let result = read_disk(&mut store, below, &disk);
    let row = one_row(&result);
    assert_eq!(row.value, expected_turn());
    assert_eq!(row.owner, Some(Box::new(task_owner())));
    assert_eq!(row.ancestors.as_ref(), []);
}

fn task_owner() -> tasks::TaskRecord {
    let core::Record::Tasks(tasks::Stored::Ended(row)) = fixtures::ended_of(CHILD, tasks::Party::Person(ALICE)) else {
        panic!("ended task");
    };
    *row
}

#[test]
fn a_reply_that_was_a_turn_joins_that_turn() {
    let (mut store, disk) = party_store();
    let below = waiting_party(&mut store, reply_range());
    let result = read_disk(&mut store, below, &disk);
    let row = one_row(&result);
    assert_eq!(row.value, turn_reply());
    assert_eq!(row.related, Some(expected_turn()));
}

#[test]
fn a_turn_read_past_the_waiting_limit_is_busy() {
    let (mut store, _) = party_store();
    for owner in 1..=store.limits.reads {
        let (events, _) = party_submit(&mut store, Token::new(u64::from(owner)), turn_range());
        assert!(events.is_empty());
    }
    let (events, below) = party_submit(&mut store, Token::new(7), turn_range());
    let result = read_result(events);
    let reads::Loaded::Busy = result else { panic!("waiting table is full: {result:?}") };
    assert!(below.transcripts.is_empty());
}

#[test]
fn an_unread_frame_gives_a_reply_without_its_turn() {
    let (mut store, mut disk) = party_store();
    corrupt_disk(&mut disk);
    let below = waiting_party(&mut store, reply_range());
    let result = read_disk(&mut store, below, &disk);
    let row = one_row(&result);
    assert_eq!(row.value, turn_reply());
    assert_eq!(row.related, None);
}

fn page(result: reads::Loaded) -> (Box<[reads::Row]>, Option<reads::Position>) {
    let reads::Loaded::Page { rows, next } = result else { panic!("one page: {result:?}") };
    (rows, next)
}

fn values(rows: &[reads::Row]) -> Box<[core::Record]> {
    let mut found = skein_lib::List::with_capacity(u32::try_from(rows.len()).expect("bounded fixture rows"));
    for row in rows {
        found.push(row.value.clone()).expect("counted fixture rows");
    }
    found.into_boxed()
}

fn activity_position(position: u64) -> reads::Position {
    reads::Position::Owner(core::Key::Tasks(tasks::Key::Activity { task: CHILD, position }))
}

fn activity_range(after: Option<reads::Position>) -> reads::Range {
    let mut range = fixtures::read(ALICE, reads::Scope::Activity { task: CHILD });
    range.rows = 2;
    range.backwards = true;
    range.after = after;
    range
}

fn activity_store() -> Store {
    let mut store = fixtures::opened();
    let core::Record::Tasks(tasks::Stored::Ended(mut owner)) = fixtures::ended_of(CHILD, tasks::Party::Person(ALICE))
    else {
        panic!("ended fixture");
    };
    owner.activity = 5;
    fixtures::committed(
        &mut store,
        Box::new([
            fixtures::header(1),
            fixtures::save(core::Record::Tasks(tasks::Stored::Ended(owner))),
            fixtures::save(fixtures::activity_line(CHILD, 1)),
            fixtures::save(fixtures::activity_line(CHILD, 2)),
        ]),
    );
    fixtures::committed(
        &mut store,
        Box::new([
            fixtures::header(2),
            fixtures::save(fixtures::activity_line(CHILD, 3)),
            fixtures::save(fixtures::activity_line(CHILD, 4)),
            fixtures::save(fixtures::activity_line(CHILD, 5)),
        ]),
    );
    store
}

#[test]
fn activity_reads_newest_first_from_the_records_last_position() {
    let mut store = activity_store();
    let (rows, next) = page(load(&mut store, activity_range(None)));
    assert_eq!(values(&rows).as_ref(), [fixtures::activity_line(CHILD, 5), fixtures::activity_line(CHILD, 4)]);
    assert_eq!(next, Some(activity_position(4)));
}

#[test]
fn an_activity_cursor_continues_below_its_position() {
    let mut store = activity_store();
    let (rows, next) = page(load(&mut store, activity_range(Some(activity_position(4)))));
    assert_eq!(values(&rows).as_ref(), [fixtures::activity_line(CHILD, 3), fixtures::activity_line(CHILD, 2)]);
    assert_eq!(next, Some(activity_position(2)));
}

#[test]
fn a_tasks_history_joins_history_then_milestones() {
    let mut store = child_store();
    fixtures::committed(
        &mut store,
        Box::new([
            fixtures::header(2),
            fixtures::save(fixtures::history(CHILD, 3)),
            fixtures::save(fixtures::hold_milestone(CHILD, 1)),
        ]),
    );
    let range = fixtures::read(ALICE, reads::Scope::History { task: CHILD, subtree: false });
    let (rows, next) = page(load(&mut store, range));
    assert_eq!(values(&rows).as_ref(), [fixtures::history(CHILD, 3), fixtures::hold_milestone(CHILD, 1)]);
    for row in &rows {
        assert_eq!(row.owner.as_ref().expect("joined owner").number, CHILD);
    }
    assert_eq!(next, None);
}

#[test]
fn spend_reads_live_and_closed_periods_of_the_project() {
    let mut store = fixtures::opened();
    fixtures::committed(
        &mut store,
        Box::new([
            fixtures::header(1),
            fixtures::save(fixtures::ledger_period(fixtures::PROJECT, 2, false)),
            fixtures::save(fixtures::ledger_period(fixtures::PROJECT, 1, true)),
            fixtures::save(fixtures::ledger_period(7, 3, true)),
        ]),
    );
    let range = fixtures::read(ALICE, reads::Scope::Spend { project: fixtures::PROJECT });
    let (rows, next) = page(load(&mut store, range));
    assert_eq!(values(&rows).as_ref(), [fixtures::ledger_period(3, 2, false), fixtures::ledger_period(3, 1, true)]);
    assert_eq!(next, None);
}

fn project_scope() -> jig_domain_notes::Scope {
    jig_domain_notes::Scope::Project { project: fixtures::PROJECT }
}

fn goal_scope() -> jig_domain_notes::Scope {
    jig_domain_notes::Scope::Goal { project: fixtures::PROJECT, goal: GOAL }
}

fn commit_note(store: &mut Store, number: u64, scope: &jig_domain_notes::Scope, name: u64, description: &[u8]) {
    let (entry, line) = fixtures::notes_entry_in(scope, name, description);
    fixtures::committed(store, Box::new([fixtures::header(number), fixtures::save(entry), fixtures::save(line)]));
}

fn note_position(scope: &jig_domain_notes::Scope, name: u64) -> reads::Position {
    reads::Position::Owner(core::Key::Notes(jig_domain_notes::Key::Line { scope: scope.clone(), name }))
}

fn note_entry(scope: &jig_domain_notes::Scope, name: u64, description: &[u8]) -> core::Record {
    let (entry, _line) = fixtures::notes_entry_in(scope, name, description);
    entry
}

fn notes_store() -> Store {
    let mut store = fixtures::opened();
    commit_note(&mut store, 1, &project_scope(), 2, b"match project");
    commit_note(&mut store, 2, &goal_scope(), 1, b"other goal");
    commit_note(&mut store, 3, &goal_scope(), 3, b"match goal");
    store
}

fn project_notes() -> reads::Range {
    fixtures::read(ALICE, reads::Scope::Notes { project: fixtures::PROJECT })
}

fn filtered_notes() -> reads::Range {
    fixtures::read(
        ALICE,
        reads::Scope::ScopedNotes { scope: goal_scope(), description: Some(Box::from(b"match".as_slice())) },
    )
}

#[test]
fn project_notes_read_their_scopes_lines_with_entries() {
    let mut store = notes_store();
    let (rows, next) = page(load(&mut store, project_notes()));
    assert_eq!(
        values(&rows).as_ref(),
        [
            note_entry(&project_scope(), 2, b"match project"),
            note_entry(&goal_scope(), 1, b"other goal"),
            note_entry(&goal_scope(), 3, b"match goal"),
        ]
    );
    assert_eq!(next, None);
}

#[test]
fn scoped_notes_keep_only_matching_descriptions() {
    let mut store = notes_store();
    let (rows, next) = page(load(&mut store, filtered_notes()));
    assert_eq!(values(&rows).as_ref(), [note_entry(&goal_scope(), 3, b"match goal")]);
    assert_eq!(next, None);
}

#[test]
fn a_notes_cursor_resumes_after_its_line() {
    let mut store = notes_store();
    let mut range = project_notes();
    range.after = Some(note_position(&project_scope(), 2));
    let (rows, next) = page(load(&mut store, range));
    assert_eq!(
        values(&rows).as_ref(),
        [note_entry(&goal_scope(), 1, b"other goal"), note_entry(&goal_scope(), 3, b"match goal")]
    );
    assert_eq!(next, None);
}

#[test]
fn notes_across_two_scopes_come_by_scope_then_name() {
    let mut store = notes_store();
    let mut range = project_notes();
    range.rows = 2;
    let (rows, next) = page(load(&mut store, range));
    assert_eq!(
        values(&rows).as_ref(),
        [note_entry(&project_scope(), 2, b"match project"), note_entry(&goal_scope(), 1, b"other goal")]
    );
    assert_eq!(reads::row_key(rows.first().expect("two rows")), Some(note_position(&project_scope(), 2)));
    assert_eq!(next, Some(note_position(&goal_scope(), 1)));
}

#[test]
fn a_filtered_notes_page_goes_on_past_the_lines_it_skipped() {
    let mut store = notes_store();
    let mut range = filtered_notes();
    range.rows = 1;
    let (rows, next) = page(load(&mut store, range.clone()));
    assert!(rows.is_empty());
    assert_eq!(next, Some(note_position(&goal_scope(), 1)));
    range.after = next;
    let (rows, next) = page(load(&mut store, range));
    assert_eq!(values(&rows).as_ref(), [note_entry(&goal_scope(), 3, b"match goal")]);
    assert_eq!(next, None);
}

#[test]
fn an_activity_window_stops_at_position_one() {
    let mut store = activity_store();
    let (rows, next) = page(load(&mut store, activity_range(Some(activity_position(2)))));
    assert_eq!(values(&rows).as_ref(), [fixtures::activity_line(CHILD, 1)]);
    assert_eq!(next, None);
}

#[test]
fn an_encoded_activity_cut_keeps_the_newest_prefix() {
    let mut store = activity_store();
    store.limits.store.page.bytes = 80;
    let (rows, next) = page(load(&mut store, activity_range(None)));
    assert_eq!(values(&rows).as_ref(), [fixtures::activity_line(CHILD, 5)]);
    assert_eq!(next, Some(activity_position(5)));
}

#[test]
fn a_paged_join_cut_keeps_its_accepted_prefix() {
    // One owner and one row's ancestor fit; the next row cuts.
    const PAGE_JOINS: u32 = 2;
    let mut store = child_store();
    fixtures::committed(
        &mut store,
        Box::new([
            fixtures::header(2),
            fixtures::save(fixtures::history(CHILD, 1)),
            fixtures::save(fixtures::history(CHILD, 2)),
        ]),
    );
    store.limits.joins = PAGE_JOINS;
    let range = fixtures::read(ALICE, reads::Scope::History { task: CHILD, subtree: false });
    let (rows, next) = page(load(&mut store, range));
    assert_eq!(values(&rows).as_ref(), [fixtures::history(CHILD, 1)]);
    assert_eq!(next, Some(reads::Position::Owner(core::Key::Tasks(tasks::Key::History { task: CHILD, revision: 1 }))));
}

mod indexed;

mod turns;

mod activity;

#[test]
fn task_scopes_name_their_first_rows_join_bound() {
    const ANCESTOR_JOINS: u32 = 1; // The ancestor fits, but the terminal does not.
    let mut store = child_store();
    store.limits.joins = ANCESTOR_JOINS;
    for scope in [
        reads::Scope::Task { task: CHILD },
        reads::Scope::PersonTask { task: CHILD },
        reads::Scope::Result { task: CHILD },
    ] {
        let result = load(&mut store, fixtures::read(ALICE, scope));
        assert_too_linked(&result, ANCESTOR_JOINS);
    }
}

#[test]
fn a_history_first_row_past_its_joins_names_the_bound() {
    const OWNER_JOINS: u32 = 1; // The owner fits; its row's ancestor does not.
    let mut store = child_store();
    fixtures::committed(&mut store, Box::new([fixtures::header(2), fixtures::save(fixtures::history(CHILD, 1))]));
    store.limits.joins = OWNER_JOINS;
    for subtree in [false, true] {
        let result = load(&mut store, fixtures::read(ALICE, reads::Scope::History { task: CHILD, subtree }));
        assert_too_linked(&result, OWNER_JOINS);
    }
}

#[test]
fn a_note_past_its_joins_names_the_bound() {
    const NO_JOINS: u32 = 0; // Notes join their entry even without an owner.
    let mut store = notes_store();
    store.limits.joins = NO_JOINS;
    for scope in [
        reads::Scope::Notes { project: fixtures::PROJECT },
        reads::Scope::ScopedNotes { scope: goal_scope(), description: None },
    ] {
        let result = load(&mut store, fixtures::read(ALICE, scope));
        assert_too_linked(&result, NO_JOINS);
    }
}

#[test]
fn a_filtered_note_counts_as_examined_before_a_join_cut() {
    const NO_JOINS: u32 = 0;
    let mut store = notes_store();
    store.limits.joins = NO_JOINS;
    let mut range = filtered_notes();
    let (rows, next) = page(load(&mut store, range.clone()));
    assert!(rows.is_empty());
    assert_eq!(next, Some(note_position(&goal_scope(), 1)));
    range.after = next;
    let result = load(&mut store, range);
    assert_too_linked(&result, NO_JOINS);
}
