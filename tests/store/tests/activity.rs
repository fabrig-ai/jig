use jig_domain_core::{self as core, reads};
use jig_domain_tasks as tasks;
use jig_protocol_store::{Event, Request, Write};
use jig_store_world::{World, root};
use skein_lib::{Token, Wall};

const OWNER: u64 = 3;
// The activity fixture's existing per-read row and byte allowances.
const PAGE_ROWS: u32 = 32;
const PAGE_BYTES: u64 = 1_048_576;

fn ended(number: u64) -> Box<tasks::TaskRecord> {
    let core::Record::Tasks(tasks::Stored::Live(mut task)) = root::live_task(number) else {
        unreachable!("live task fixture")
    };
    task.requester = tasks::Party::Task(1);
    task.root = 1;
    task.depth = 1;
    task.phase = tasks::Phase::Ended(tasks::Ending::Failed { reason: Box::new([]) });
    task.ended_at = Some(Wall::EPOCH);
    task
}

fn fixture(joins: u32, turns: [u64; 3]) -> World {
    let mut world = World::with_joins(18, joins);
    let dependency = ended(2);
    let mut dependent = ended(OWNER);
    dependent.dependencies = Box::new([2]);
    dependent.activity = 8;
    dependent.turn = 3;
    dependent.ever_turned = true;
    let mut writes = vec![
        Write::Save(root::header(1)),
        Write::Save(root::live_task(1)),
        Write::Save(core::Record::Tasks(tasks::Stored::Ended(dependency))),
        Write::Save(core::Record::Tasks(tasks::Stored::Ended(dependent))),
    ];
    for position in 1..=8 {
        let line = match turns.iter().position(|&turn| turn == position) {
            Some(index) => tasks::Line::Turn {
                reply: 1,
                attempt: 1,
                turn: u32::try_from(index + 1).expect("three turns"),
                read: None,
            },
            None => tasks::Line::Decided { revision: position },
        };
        writes.push(Write::Save(core::Record::Tasks(tasks::Stored::Activity(tasks::ActivityLine {
            task: OWNER,
            position,
            at: Wall::EPOCH,
            line,
        }))));
    }
    assert_eq!(world.commit(writes.into_boxed_slice()), Some(1));
    for number in 1..=3 {
        assert_eq!(
            world.commit(root::turn_commit(u64::from(number) + 1, OWNER, number, b"turn")),
            Some(u64::from(number) + 1)
        );
    }
    world
}

fn activity(world: &mut World, after: Option<reads::Position>) -> (Box<[reads::Row]>, Option<reads::Position>) {
    bounded_activity(world, after, PAGE_ROWS, PAGE_BYTES)
}

fn loaded_activity(world: &mut World, after: Option<reads::Position>, rows: u32, bytes: u64) -> reads::Loaded {
    let events = world.request(Request::Read {
        owner: Token::new(1),
        range: reads::Range {
            party: 1,
            scope: reads::Scope::Activity { task: OWNER },
            after,
            backwards: true,
            rows,
            bytes,
        },
    });
    let mut events = events.into_iter();
    let Some(Event::Read { result, .. }) = events.next() else { panic!("activity terminal") };
    assert!(events.next().is_none(), "one read terminal");
    result
}

fn bounded_activity(
    world: &mut World,
    after: Option<reads::Position>,
    rows: u32,
    bytes: u64,
) -> (Box<[reads::Row]>, Option<reads::Position>) {
    let result = loaded_activity(world, after, rows, bytes);
    let reads::Loaded::Page { rows, next } = result else { panic!("activity page: {result:?}") };
    (rows, next)
}

fn positions(rows: &[reads::Row]) -> Vec<u64> {
    rows.iter()
        .map(|row| {
            let core::Record::Tasks(tasks::Stored::Activity(line)) = &row.value else { panic!("activity line") };
            assert_eq!(row.owner.as_ref().expect("page owner").number, OWNER);
            assert!(row.ancestors.is_empty());
            assert!(row.linked.is_empty());
            if let tasks::Line::Turn { attempt, turn, .. } = line.line {
                let Some(core::Record::Core(core::CoreRecord::Turn(record))) = &row.related else {
                    panic!("turn line carries its transcript")
                };
                assert_eq!((record.task, record.attempt, record.turn), (OWNER, attempt, turn));
                assert_eq!(record.transcript.as_ref(), b"turn");
            }
            line.position
        })
        .collect()
}

#[test]
fn ended_dependent_activity_is_not_false_eof() {
    // One owner and two turns fit; the third turn cuts the page.
    const PAGE_JOINS: u32 = 3;
    let mut world = fixture(PAGE_JOINS, [1, 4, 7]);
    let (rows, mut next) = activity(&mut world, None);
    let mut found = positions(&rows);
    assert_eq!(found.first(), Some(&8));
    assert!(next.is_some(), "third turn exceeds the page's two remaining joins");
    for _ in 0..8 {
        let Some(after) = next.take() else { break };
        let (rows, continuation) = activity(&mut world, Some(after));
        assert!(!rows.is_empty(), "sufficient joins make progress");
        found.extend(positions(&rows));
        next = continuation;
    }
    assert!(next.is_none(), "activity ends");
    assert_eq!(found, (1..=8).rev().collect::<Vec<_>>());
}

#[test]
fn an_activity_first_turn_past_its_joins_is_too_linked() {
    // The owner consumes the only join, before the newest turn.
    const OWNER_JOINS: u32 = 1;
    let mut world = fixture(OWNER_JOINS, [2, 5, 8]);
    let retry = reads::Position::Owner(core::Key::Tasks(tasks::Key::Activity { task: OWNER, position: 9 }));
    for after in [None, Some(retry)] {
        let result = loaded_activity(&mut world, after, PAGE_ROWS, PAGE_BYTES);
        let reads::Loaded::TooLinked { joins } = result else { panic!("join refusal: {result:?}") };
        assert_eq!(joins, OWNER_JOINS);
    }
}

#[test]
fn an_activity_first_turn_fits_its_whole_join_bound() {
    // One owner and one turn fit exactly, followed by the intervening words.
    const OWNER_AND_TURN_JOINS: u32 = 2;
    let mut world = fixture(OWNER_AND_TURN_JOINS, [2, 5, 8]);
    let (rows, next) = activity(&mut world, None);
    assert_eq!(positions(&rows), [8, 7, 6]);
    assert_eq!(next, Some(reads::Position::Owner(core::Key::Tasks(tasks::Key::Activity { task: OWNER, position: 6 }))));
}

#[test]
fn activity_row_and_byte_bounds_before_the_first_row_preserve_its_position() {
    let mut world = fixture(4, [1, 4, 7]);
    for (rows, bytes) in [(0, 1_048_576), (32, 0)] {
        for before in [None, Some(8)] {
            let after = before.map(|position| {
                reads::Position::Owner(core::Key::Tasks(tasks::Key::Activity { task: OWNER, position }))
            });
            let (found, next) = bounded_activity(&mut world, after, rows, bytes);
            assert!(found.is_empty());
            let next = next.expect("row or byte bound retains the starting position");
            assert_eq!(
                next,
                reads::Position::Owner(core::Key::Tasks(tasks::Key::Activity {
                    task: OWNER,
                    position: before.unwrap_or(9),
                }))
            );
            let (found, _) = activity(&mut world, Some(next));
            assert_eq!(positions(&found).first(), Some(&before.unwrap_or(9).saturating_sub(1)));
        }
    }
}
