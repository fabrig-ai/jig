//! The root keeps these orders whatever admits work (skein's draft
//! `dynamic-memory.md`, 4.6): store terminals in arrival order, and a kept
//! turn before its terminal acknowledgement, while admission is under load.

use jig_core_world::direct::Driver;
use jig_domain_core::{self as core, reads};
use jig_domain_tasks as tasks;
use jig_test_domain as root;
use skein_lib::{ReplyTo, Token};

const WORKER: Token = Token::new(7);
const READ_A: Token = Token::new(960);
const READ_B: Token = Token::new(961);
const TURN: u32 = 1;
const SPENT: u64 = 3;
const TURN_TEXT: &[u8] = b"kept turn";
const REPORT: &[u8] = b"finished work";
// Each story writes one turn, and the terminal story writes one final answer.
const ONCE: usize = 1;
const STORY_COMMITS: usize = 2;
// Same finite settlement budget as the direct world.
const STEPS: usize = 300;

/// O2: store answers taken while admission binds answer in arrival order.
#[test]
fn queued_read_answers_keep_their_arrival_order_under_load() {
    let mut driver = Driver::chat(9302);
    let (task, attempt) = run(&driver);
    driver.send(turn(task, attempt));
    driver.settle();
    let second = read_turn(&mut driver, READ_B);
    let first = read_turn(&mut driver, READ_A);
    driver.fill_to_watermark();
    let before_probe = driver.returned;
    driver.probe();
    assert_eq!(driver.returned, before_probe + 1, "new work is handed back under load");
    let returned = driver.returned;
    driver.send(first);
    driver.send(second);
    driver.collect();
    assert_eq!(driver.returned, returned, "both store terminals are taken under load");
    assert!(read_replies(&driver).is_empty(), "answers wait for preceding durability");
    driver.settle();
    let replies = read_replies(&driver);
    assert_eq!(replies.iter().map(|(reply, _)| *reply).collect::<Vec<_>>(), [READ_A, READ_B]);
    for (_, answer) in replies {
        let reads::Answer::Historical(past) = answer else { panic!("historical turn: {answer:?}") };
        let reads::Past::Turn(row) = &past.value else { panic!("requested turn") };
        assert_eq!((row.task, row.attempt, row.turn), (task, attempt, TURN));
        assert_eq!(row.transcript.as_ref(), TURN_TEXT);
    }
    assert!(driver.domain.quiescent() && !driver.stopped);
    println!("O2 handed-back inputs: {}", driver.returned);
}

/// O3: a kept turn precedes its terminal acknowledgement under load.
#[test]
#[ignore = "fails on main: a released callback whose decision is refused journal room is dropped (jig-test-domain lib.rs release/decide); passes once the watermark swap takes it"]
fn turn_kept_precedes_terminal_acknowledgement_under_load() {
    let mut driver = Driver::chat(9303);
    let (task, attempt) = run(&driver);
    driver.send(turn(task, attempt));
    driver.collect();
    driver.send(root::Event::Answer {
        read: None,
        refused: Box::new([]),
        channel: WORKER,
        task,
        attempt,
        cumulative: SPENT,
        end: tasks::End::Finished {
            result: tasks::TaskResult::Report { words: REPORT.into() },
            cancel_delegates: false,
        },
    });
    driver.collect();
    assert_eq!(driver.pending.len(), STORY_COMMITS, "turn and terminal commits are withheld");
    driver.fill_to_watermark();
    assert!(worker_notices(&driver).is_empty(), "acknowledgements wait for durability");
    // Unlike temper, this root routes released callbacks inside release().
    // Refill before that release, so their decisions face real pressure too.
    for _ in 0..STORY_COMMITS {
        driver.commit_one();
        driver.fill_to_watermark();
    }
    driver.settle();
    println!("O3 handed-back inputs: {}", driver.returned);
    assert_eq!(
        worker_notices(&driver),
        [(task, attempt, Some(TURN)), (task, attempt, None)],
        "worker sees its kept turn before the terminal acknowledgement, each once"
    );
    let writes: Vec<_> = driver.transactions.iter().flatten().collect();
    assert_eq!(
        writes
            .iter()
            .filter(|write| matches!(write,
                root::Write::Save(root::Record::Core(core::Record::Core(core::CoreRecord::Turn(row))))
                    if row.task == task && row.attempt == attempt && row.turn == TURN
            ))
            .count(),
        ONCE,
        "turn is written once"
    );
    assert_eq!(
        writes
            .iter()
            .filter(|write| matches!(write,
                root::Write::Save(root::Record::Core(core::Record::Core(core::CoreRecord::Terminal(row))))
                    if row.task == task && row.attempt == attempt
            ))
            .count(),
        ONCE,
        "terminal is written once"
    );
    assert_eq!(
        driver
            .released
            .iter()
            .filter(|request| matches!(request,
                root::Request::Deliver(root::Delivery::Core(core::Held::Result { task: named, words, .. }))
                    if *named == task && words.as_ref() == REPORT
            ))
            .count(),
        ONCE,
        "party receives the result once"
    );
    assert!(driver.domain.quiescent() && !driver.stopped);
}

fn run(driver: &Driver) -> (u64, u64) {
    let assignment = driver.assignment();
    (assignment.task, assignment.attempt)
}

fn turn(task: u64, attempt: u64) -> root::Event {
    root::Event::Turn {
        opens_window: true,
        refused: Box::new([]),
        channel: WORKER,
        task,
        attempt,
        turn: TURN,
        cumulative: SPENT,
        read: None,
        transcript: TURN_TEXT.into(),
    }
}

fn read_turn(driver: &mut Driver, reply: Token) -> root::Event {
    let (task, attempt) = run(driver);
    driver.send(root::Event::Core(core::Event::Read {
        reply: ReplyTo::new(reply),
        sign_in: driver.session(),
        read: reads::Read::Turn { task, attempt, turn: TURN },
    }));
    for _ in 0..STEPS {
        driver.collect();
        if let Some(index) =
            driver.events.iter().position(|event| matches!(event, root::Event::Core(core::Event::ReadLoaded { .. })))
        {
            return driver.events.remove(index).expect("actual store terminal");
        }
        if let Some(event) = driver.events.pop_front() {
            driver.send_retaining(event);
        }
        if !driver.pending.is_empty() {
            driver.commit_one();
        }
    }
    panic!("historical read issues a store load within the step budget");
}

fn read_replies(driver: &Driver) -> Vec<(Token, &reads::Answer)> {
    driver
        .released
        .iter()
        .filter_map(|request| {
            if let root::Request::Deliver(root::Delivery::Core(core::Held::Read { reply, answer })) = request {
                [READ_A, READ_B].into_iter().find(|token| *reply == ReplyTo::new(*token)).map(|token| (token, answer))
            } else {
                None
            }
        })
        .collect()
}

fn worker_notices(driver: &Driver) -> Vec<(u64, u64, Option<u32>)> {
    driver
        .released
        .iter()
        .filter_map(|request| {
            if let root::Request::Deliver(root::Delivery::Core(held)) = request {
                if let core::Held::AcknowledgeTurn { channel, run, attempt, turn } = held
                    && *channel == WORKER
                {
                    Some((run.raw(), attempt.raw(), Some(*turn)))
                } else if let core::Held::Acknowledge { channel, run, attempt } = held
                    && *channel == WORKER
                {
                    Some((run.raw(), attempt.raw(), None))
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect()
}
