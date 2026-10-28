//! Fixtures for durable document and owner-page reads.
use crate::{
    effects::World,
    reads::{Parties, ask, latest},
};
use jig_domain_core::{self as core, documents, reads};
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_test_domain as root;
use skein_lib::{Duration, ReplyTo, Token};

const DELEGATE: u64 = core::ToolFamilies::JIG.delegate.0;

#[must_use]
pub fn world(seed: u64) -> World {
    read_world(seed, None, 2, None, false)
}

/// A read fixture with a chosen bounded live page size.
#[must_use]
pub fn world_with_rows(seed: u64, rows: u32) -> World {
    read_world(seed, None, rows, None, false)
}

/// A conversation whose retention releases its durable owner after both marks.
#[must_use]
pub fn forgettable_world(seed: u64) -> World {
    let world = read_world(seed, None, 2, Some(forgettable_conversation()), false);
    assert!(world.assigned.is_some(), "initial conversation assigned: {:?}", world.people_answers);
    world
}

/// The initial authenticated conversation request, shared by read stories.
#[must_use]
pub fn forgettable_conversation() -> people::Ask {
    people::Ask::Create { project: 1, batch: Box::new([conversation_member(true)]) }
}

/// One explicitly flagged conversation under the world's agent charter.
#[must_use]
pub fn conversation_member(forgettable: bool) -> people::Member {
    people::Member {
        deadline: jig_domain_people::Deadline::Named,
        executor: people::TaskExecutor::Agent { charter: 1 },
        words: b"make an effect".as_slice().into(),
        parameters: Box::new([]),
        inputs: Box::new([]),
        named: Box::new([]),
        budget: people::Budget::Named,
        contract: people::Contract::Report { words: 128 },
        authority: people::Authority {
            tools: DELEGATE,
            models: None,
            grants: Box::new([]),
            delegation: people::Delegation {
                kinds: Box::new([people::Executor::Charter(1), people::Executor::Procedure(1)]),
                tasks: 1,
                depth: 1,
            },
            spend: 100,
            deadline: None,
            notes: 0,
            note_resources: Box::new([]),
        },
        wake: people::WakePolicy {
            words: people::WakeRule::Immediate,
            notices: people::WakeRule::Immediate,
            news: people::WakeRule::Immediate,
            results: people::ResultsWake::LastOrFailure,
            questions: true,
            answers: true,
            timers: true,
        },
        dependencies: Box::new([]),
        tracked: None,
        label: jig_client_test_records::CHAT,
        flags: people::Flags {
            ended_by: people::EndedBy::Result,
            conversation: true,
            listed: false,
            retention: if forgettable { people::Retention::ForgottenOnceRead } else { people::Retention::Kept },
        },
        llm: None,
    }
}

// Eight KiB admits a task owner and fixed evidence; a payload one byte larger cannot fit.
pub const TURN_READ_BYTES: u32 = 8192;
// The transcript admits the payload independently of the complete single-row read bound.
pub const LARGE_TURN_BYTES: u32 = TURN_READ_BYTES + 1;
/// The ordinary read world, admitting one transcript larger than its single-read budget.
#[must_use]
pub fn world_with_large_turn(seed: u64) -> World {
    read_world(seed, None, 2, None, true)
}

fn read_world(seed: u64, reply_bound: Option<u32>, rows: u32, initial: Option<people::Ask>, large_turn: bool) -> World {
    let (configuration, mut limits) = crate::effects::fixture(seed, false);
    if let Some(bound) = reply_bound {
        limits.core.tasks.inbox_messages = bound;
        limits.core.tasks.run_inbox = bound;
        limits.core.people.inbox_entries = bound;
    }
    if large_turn {
        limits.core.load_bytes = u64::from(TURN_READ_BYTES);
        limits.core.run_bytes = LARGE_TURN_BYTES;
    }
    limits.core.documents.rows = rows;
    if reply_bound.is_none() {
        limits.core.people.inbox_entries = 1;
    }
    limits.core.load_slots = 1;
    limits.core.tasks.resource_reports = 4;
    limits.core.people.requests = 8;
    limits.core.views.watchers = 1;
    limits.core.views.watchers_per_party = 1;
    limits.core.views.subjects_per_change = 1;
    let room = core::route_bound(&limits.core).expect("read story room");
    let connectors = jig_test_connector::MAX_OUT * 2;
    limits.journal.commits = 2;
    limits.watermark = root::default_watermark(&limits.journal);
    limits.journal.writes = room + connectors;
    limits.journal.held = (room + connectors) * 2;
    let mut world = match initial {
        Some(initial) => World::configured_with(seed, false, configuration, limits, initial),
        None => World::configured(seed, false, configuration, limits),
    };
    world.reads = Some(Parties::new(world.store.applied));
    world
}

pub fn request(world: &mut World, number: u8, ask: people::Ask) {
    let event = request_event(world, number, ask);
    world.send(event);
}

#[must_use]
pub fn request_event(world: &World, number: u8, ask: people::Ask) -> root::Event {
    root::Event::Core(core::Event::People(people::Event::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(Token::new(30_000 + u64::from(number))),
        sign_in: world.sign_in.expect("signed in"),
        key: [number; 16],
        ask,
    }))
}

#[must_use]
pub fn conversation(world: &mut World, task: u64, first: u64) -> Vec<tasks::ActivityLine> {
    let mut before = None;
    let mut lines = Vec::new();
    for page in 0..32 {
        ask(world, first + page, reads::Read::Activity { task, before: before.take() });
        let reads::Answer::Page(value) = latest(world) else { panic!("activity page") };
        assert!(value.rows.len() <= 2);
        for row in &value.rows {
            let reads::PageRow::Activity { line, turn: supplied } = row else { panic!("original activity") };
            if let tasks::Line::Turn { attempt, turn, .. } = line.line {
                let Some(reads::PageTurn::Carried(record)) = supplied else { panic!("page carries the turn") };
                assert_eq!((record.task, record.attempt, record.turn), (task, attempt, turn));
            }
            lines.push(line.clone());
        }
        before.clone_from(&value.next);
        if before.is_none() {
            return lines;
        }
    }
    panic!("bounded conversation ended");
}

/// A temporal cache prefix must remain a prefix even when a later turn has an earlier wall time.
pub fn a_backward_wall_correction_keeps_reply_paging_whole_after_restart() {
    let mut world = read_world(766, Some(2), 2, None, false);
    let task = world.assigned.as_ref().expect("chat").task;
    world.advance(Duration::from_secs(2));
    world.turn(1, 1, None, b"before correction one");
    world.turn(2, 2, None, b"before correction two");
    world.correct_wall_backwards(Duration::from_secs(1));
    for turn in 3..=5 {
        world.turn(turn, u64::from(turn), None, b"after correction");
    }
    let activity = conversation(&mut world, task, 210);
    let mut expected = activity
        .iter()
        .filter_map(|line| match line.line {
            tasks::Line::Turn { reply, .. } => Some((line.at, reply, line.position)),
            tasks::Line::Words { .. }
            | tasks::Line::Heard(_)
            | tasks::Line::Notified { .. }
            | tasks::Line::RunBegan { .. }
            | tasks::Line::RunEnded { .. }
            | tasks::Line::Decided { .. }
            | tasks::Line::Kept { .. }
            | tasks::Line::Milestone { .. } => None,
        })
        .collect::<Vec<_>>();
    expected.sort_unstable_by(|a, b| b.cmp(a));
    assert_eq!(expected.iter().map(|row| row.1).collect::<Vec<_>>(), [2, 1, 5, 4, 3]);
    world.restart(766, false);
    let mut found = Vec::new();
    let mut from = None;
    for page in 0..8 {
        ask(
            &mut world,
            250 + page,
            reads::Read::Inbox {
                group: reads::Group::Updates,
                filter: reads::Filter { project: Some(1), goal: None },
                from: from.take(),
            },
        );
        let reads::Answer::Page(value) = latest(&world) else { panic!("reply inbox page") };
        assert!(value.rows.len() <= 2);
        for row in &value.rows {
            let reads::PageRow::Inbox(entry) = row else { panic!("reply row") };
            let documents::InboxObject::Reply { message } = entry.object else { panic!("reply ordinal") };
            let conversation = entry.reply.as_ref().expect("whole committed conversation");
            assert_eq!(
                conversation.conversation.expect("read position").unread,
                documents::Count::AtLeast(2),
                "unread count respects the retained bound"
            );
            assert_eq!(conversation.conversation.expect("read position").seen, 0);
            found.push((entry.at.expect("reply wall time"), message, entry.activity.expect("immutable load position")));
        }
        from.clone_from(&value.next);
        if from.is_none() {
            break;
        }
    }
    assert_eq!(found, expected, "cold live and stored pages return every reply exactly once in wall order");
}

/// The fake store begins with retained owner entries, not cached note bodies.
pub fn scoped_description_search_pages_durable_notes_after_restart() {
    use jig_domain_notes as notes;
    use skein_lib::{List, Wall};
    let mut world = world(767);
    let scope = notes::Scope::Goal { project: 1, goal: 99 };
    for (name, scope, description) in [
        (1, scope.clone(), b"no match".as_slice()),
        (2, scope.clone(), b"no match".as_slice()),
        (3, scope.clone(), b"slow to warm up".as_slice()),
        (4, notes::Scope::Goal { project: 1, goal: 98 }, b"warm another goal".as_slice()),
        (5, scope.clone(), b"warm cache".as_slice()),
    ] {
        let line = root::Record::Core(core::Record::Notes(notes::Record::Line(notes::Line {
            scope: scope.clone(),
            name,
            description: description.into(),
            revision: 2,
        })));
        let row = root::Record::Core(core::Record::Notes(notes::Record::Entry(notes::Entry {
            name,
            scope,
            description: description.into(),
            body: b"kept complete body".as_slice().into(),
            references: List::with_capacity(0),
            author: notes::Author::Party { party: 1 },
            revision: 2,
            changed: Wall::from_nanos(name),
        })));
        world.seed(Box::new([row, line]));
    }
    for restart in [false, true] {
        if restart {
            world.restart(767, false);
        }
        let mut from = None;
        let mut found = Vec::new();
        let mut empty = 0;
        for number in 0..8 {
            ask(
                &mut world,
                300 + number + u64::from(restart) * 10,
                reads::Read::ScopedNotes {
                    scope: scope.clone(),
                    description: Some(b"warm".as_slice().into()),
                    from: from.take(),
                },
            );
            let reads::Answer::Page(page) = latest(&world) else { panic!("scoped note page") };
            assert!(page.rows.len() <= 2);
            empty += usize::from(page.rows.is_empty());
            for row in &page.rows {
                let reads::PageRow::Note(note) = row else { panic!("note row") };
                assert_eq!(note.body.bytes.as_ref(), b"kept complete body");
                assert_eq!(note.revision, 2);
                found.push(note.number);
            }
            from.clone_from(&page.next);
            if from.is_none() {
                break;
            }
        }
        assert_eq!(found, [3, 5], "scoped matches survive paging and cold restart");
        assert_eq!(empty, 1, "the first examined page has no selected rows but continues");
    }
}

/// The independent party pages mixed live and retained updates under each exact kind.
pub fn result_and_reply_filters_page_their_own_retained_updates() {
    let mut world = world(768);
    let mut tasks = Vec::new();
    for number in 0..3 {
        if number != 0 {
            let next_chat = crate::fixtures::chat(&world, 1, b"next chat".as_slice().into());
            request(&mut world, 40 + number, next_chat);
        }
        let task = world.assigned.as_ref().expect("chat").task;
        tasks.push(task);
        world.turn(1, 1, None, b"retained reply");
        world.end(
            tasks::End::Finished {
                result: tasks::TaskResult::Report { words: b"result".as_slice().into() },
                cancel_delegates: false,
            },
            1,
        );
    }
    world.restart(768, false);
    for group in [reads::Group::Results, reads::Group::Replies] {
        let mut from = None;
        let mut found = Vec::new();
        for number in 0..16 {
            ask(
                &mut world,
                400 + number + if group == reads::Group::Replies { 20 } else { 0 },
                reads::Read::Inbox { group, filter: reads::Filter { project: Some(1), goal: None }, from: from.take() },
            );
            let reads::Answer::Page(page) = latest(&world) else { panic!("filtered inbox page") };
            assert!(page.rows.len() <= 2);
            for row in &page.rows {
                let reads::PageRow::Inbox(entry) = row else { panic!("inbox row") };
                match entry.object {
                    documents::InboxObject::Result { .. } => assert_eq!(group, reads::Group::Results),
                    documents::InboxObject::Reply { .. } => assert_eq!(group, reads::Group::Replies),
                    documents::InboxObject::Proposal { .. }
                    | documents::InboxObject::Escalation { .. }
                    | documents::InboxObject::Question { .. }
                    | documents::InboxObject::PersonTask => panic!("updates only"),
                }
                let number = match &entry.task {
                    documents::TaskLink::Live(chip) | documents::TaskLink::Retained { chip, .. } => chip.number,
                    documents::TaskLink::Stored { number, .. } => *number,
                };
                found.push(number);
            }
            from.clone_from(&page.next);
            if from.is_none() {
                break;
            }
        }
        let mut expected = tasks.clone();
        expected.reverse();
        assert_eq!(found, expected, "each kind contains every retained update exactly once");
    }
}
