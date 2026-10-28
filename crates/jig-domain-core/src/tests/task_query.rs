//! A task query's live group, its keys, order, window and marks.
use super::*;
use fixtures::{make_batch, query_fixture, restore};

const REQUESTER: u64 = 1;
const OTHER_REQUESTER: u64 = 2;
const FIRST: u64 = 1;
const SECOND: u64 = 2;
const THIRD: u64 = 3;
const SIGN_IN: u64 = 10;
const WATCHER: u64 = 30;
const REPLY: u64 = 60;
const CHILD: u64 = 4;
const QUESTION: u64 = 1;
const FIRST_GENERATED: u64 = 100;
const ALL: TaskQuery = TaskQuery {
    project: Some(1),
    requester: None,
    root: None,
    parent: None,
    label: None,
    phase: None,
    executor: None,
    tracked: None,
    order: QueryOrder::Number,
};
const QUERIES: [TaskQuery; 4] = [
    ALL,
    TaskQuery { requester: Some(tasks::Party::Person(REQUESTER)), ..ALL },
    TaskQuery { phase: Some(PhaseClass::Working), ..ALL },
    TaskQuery { order: QueryOrder::Activity, ..ALL },
];

#[test]
fn a_query_by_requester_party_selects_only_their_tasks() {
    let (core, limits) = query_fixture();
    let query = TaskQuery { requester: Some(tasks::Party::Person(REQUESTER)), ..ALL };
    let value = lines(&core, &limits, REQUESTER, &query, &Window { rows: 4, after: None }).unwrap();
    assert_eq!(value.lines.len(), 2);
    assert_eq!(value.lines.get(0).unwrap().chip.number, FIRST);
    assert_eq!(value.lines.get(1).unwrap().chip.number, SECOND);
    assert_eq!(value.next, None);
}

#[test]
fn a_query_naming_no_project_admits_only_the_requester_themself() {
    let (core, limits) = query_fixture();
    for (requester, expected) in [
        (None, Ok(())),
        (Some(tasks::Party::Person(OTHER_REQUESTER)), Err(Missing::NotAllowed)),
        (Some(tasks::Party::Deployment { project: 1 }), Err(Missing::Unknown)),
    ] {
        let query = TaskQuery { project: None, requester, ..ALL };
        let answer = match lines(&core, &limits, REQUESTER, &query, &window()) {
            Ok(_) => Ok(()),
            Err(why) => Err(why),
        };
        assert_eq!(answer, expected, "{requester:?}");
    }
    let mine = TaskQuery { project: None, requester: Some(tasks::Party::Person(REQUESTER)), ..ALL };
    assert_eq!(lines(&core, &limits, REQUESTER, &mine, &window()).unwrap().lines.get(0).unwrap().chip.number, FIRST);
}

#[test]
fn a_window_resumes_after_its_cursors_task() {
    let (core, limits) = query_fixture();
    let first = lines(&core, &limits, REQUESTER, &ALL, &window()).unwrap();
    assert_eq!(first.lines.get(0).unwrap().chip.number, FIRST);
    assert_eq!(first.next, Some(Cursor::Number { number: FIRST }));
    let next = lines(&core, &limits, REQUESTER, &ALL, &Window { rows: 4, after: first.next }).unwrap();
    assert_eq!(next.lines.len(), 2);
    assert_eq!(next.lines.get(0).unwrap().chip.number, SECOND);
    assert_eq!(next.lines.get(1).unwrap().chip.number, THIRD);
    assert_eq!(next.next, None);
}

#[test]
fn last_activity_order_puts_the_latest_first() {
    let (mut core, limits) = query_fixture();
    let mut first = core.tasks.task(FIRST).unwrap().clone();
    first.last_activity = Wall::from_nanos(50);
    let second = core.tasks.task(SECOND).unwrap().clone();
    let third = core.tasks.task(THIRD).unwrap().clone();
    restore(&mut core, &limits, Box::new([first, second, third]));
    let query = TaskQuery { order: QueryOrder::Activity, ..ALL };
    let value = lines(&core, &limits, REQUESTER, &query, &Window { rows: 4, after: None }).unwrap();
    assert_eq!(value.lines.get(0).unwrap().chip.number, FIRST);
    assert_eq!(value.lines.get(1).unwrap().chip.number, THIRD);
    assert_eq!(value.lines.get(2).unwrap().chip.number, SECOND);
}

#[test]
fn priority_order_puts_tracked_tasks_first_by_priority() {
    let (mut core, limits) = query_fixture();
    let mut first = core.tasks.task(FIRST).unwrap().clone();
    first.tracked = Some(8);
    let second = core.tasks.task(SECOND).unwrap().clone();
    let mut third = core.tasks.task(THIRD).unwrap().clone();
    third.tracked = Some(2);
    restore(&mut core, &limits, Box::new([first, second, third]));
    let query = TaskQuery { order: QueryOrder::Priority, ..ALL };
    let value = lines(&core, &limits, REQUESTER, &query, &Window { rows: 4, after: None }).unwrap();
    assert_eq!(value.lines.get(0).unwrap().chip.number, THIRD);
    assert_eq!(value.lines.get(1).unwrap().chip.number, FIRST);
    assert_eq!(value.lines.get(2).unwrap().chip.number, SECOND);
}

#[test]
fn a_task_in_another_project_does_not_mark_the_query() {
    let (mut core, limits) = query_fixture();
    watch_outputs(
        &mut core,
        &limits,
        Event::Watch {
            watcher: Token::new(WATCHER),
            sign_in: SIGN_IN,
            subject: Watched::Query { query: ALL },
            window: window(),
        },
    );
    watch_outputs(&mut core, &limits, Event::Delivered { watcher: Token::new(WATCHER) });
    let mut unrelated = core.tasks.task(THIRD).unwrap().clone();
    unrelated.project = 2;
    let first = core.tasks.task(FIRST).unwrap().clone();
    let second = core.tasks.task(SECOND).unwrap().clone();
    restore(&mut core, &limits, Box::new([first, second, unrelated]));
    core.touch_task_number(&limits, THIRD);
    assert!(core.changed_views.is_empty());
}

#[test]
fn a_delegates_question_marks_a_query_listing_its_parent() {
    let (mut core, limits) = query_fixture();
    let child = new_row(&core, CHILD, tasks::Funder::Task(FIRST), None);
    make_batch(&mut core, &limits, tasks::Party::Task(FIRST), Box::new([child]));
    finish_watch_outputs(&mut core, &limits);
    let query = TaskQuery { tracked: Some(true), ..ALL };
    watch_outputs(
        &mut core,
        &limits,
        Event::Watch {
            watcher: Token::new(WATCHER),
            sign_in: SIGN_IN,
            subject: Watched::Query { query },
            window: window(),
        },
    );
    watch_outputs(&mut core, &limits, Event::Delivered { watcher: Token::new(WATCHER) });
    send_task(
        &mut core,
        &limits,
        tasks::Event::Message {
            reply_to: ReplyTo::new(Token::new(REPLY)),
            project: 1,
            task: CHILD,
            word: tasks::Word {
                number: QUESTION,
                from: tasks::Party::Task(FIRST),
                kind: tasks::MessageKind::Question,
                words: Box::from(&b"what next?"[..]),
                at: Wall::EPOCH,
                hits: 1,
                eligible: true,
            },
        },
    );
    core.touch_task_number(&limits, CHILD);
    let binding = core.view_subjects.get(&Token::new(WATCHER)).unwrap();
    assert!(core.changed_views.contains(&binding.subject));
}

fn generated_event(core: &Core, draw: u64, iteration: u64) -> Event {
    let task = match core.tasks.view_tasks().as_ref().first() {
        Some(row) => row.number,
        None => FIRST,
    };
    let reply_to = ReplyTo::new(Token::new(REPLY));
    match draw {
        0 => Event::People(people::Event::Ask {
            models: Box::new([]),
            reply_to,
            sign_in: SIGN_IN,
            key: [u8::try_from(iteration).unwrap(); 16],
            ask: people::Ask::Create { project: 1, batch: Box::new([fixtures::member(b"new task", 100)]) },
        }),
        1 => Event::Tasks(tasks::Event::Amend {
            places: Box::new([]),
            funder: None,
            reply_to,
            by: tasks::Party::Person(REQUESTER),
            task,
            message: FIRST_GENERATED.checked_add(iteration).unwrap(),
            stop_run: false,
            amendment: tasks::Amendment {
                tracked: None,
                spec: Some(tasks::Spec {
                    asks: false,
                    words: Box::from(&b"changed task"[..]),
                    parameters: Box::new([]),
                    inputs: Box::new([]),
                }),
                addressed: None,
                instructions: None,
                release: None,
                wake: None,
                dependencies: None,
                authority: None,
                llm: None,
                reason: Box::new([]),
            },
        }),
        2 => Event::Tasks(tasks::Event::Control {
            reply_to,
            by: tasks::Party::Person(REQUESTER),
            task,
            control: tasks::Control::Cancel { reason: Box::new([]), grace: None, message: None },
        }),
        3 => Event::Tasks(tasks::Event::EffectsSettled { task }),
        _ => unreachable!("four generated operations"),
    }
}

#[test]
fn generated_commits_mark_each_query_whose_lines_change() {
    let (mut core, mut limits) = tree_core();
    let (auth, auth_limits) = watch_core();
    core.people = auth.people;
    limits.people = auth_limits.people;
    finish_watch_outputs(&mut core, &limits);
    for (offset, query) in QUERIES.into_iter().enumerate() {
        let watcher = Token::new(WATCHER + u64::try_from(offset).unwrap());
        watch_outputs(
            &mut core,
            &limits,
            Event::Watch { watcher, sign_in: SIGN_IN, subject: Watched::Query { query }, window: window() },
        );
        watch_outputs(&mut core, &limits, Event::Delivered { watcher });
    }
    let mut rng = skein_lib::Rng::new(5);
    for iteration in 0..32 {
        let before = QUERIES.map(|query| lines(&core, &limits, REQUESTER, &query, &window()).unwrap());
        let draw = rng.below(4);
        let event = generated_event(&core, draw, iteration);
        let ending = if let Event::Tasks(tasks::Event::EffectsSettled { task }) = &event { Some(*task) } else { None };
        watch_outputs(&mut core, &limits, event);
        if let Some(task) = ending {
            watch_outputs(&mut core, &limits, Event::Tasks(tasks::Event::Settled { task }));
        }
        for (offset, query) in QUERIES.iter().enumerate() {
            let after = lines(&core, &limits, REQUESTER, query, &window()).unwrap();
            if before[offset] != after {
                let watcher = Token::new(WATCHER + u64::try_from(offset).unwrap());
                let binding = core.view_subjects.get(&watcher).unwrap();
                assert!(core.changed_views.contains(&binding.subject), "commit {iteration}, query {query:?}");
            }
        }
        finish_watch_outputs(&mut core, &limits);
        for offset in 0..QUERIES.len() {
            watch_outputs(
                &mut core,
                &limits,
                Event::Delivered { watcher: Token::new(WATCHER + u64::try_from(offset).unwrap()) },
            );
        }
    }
}
