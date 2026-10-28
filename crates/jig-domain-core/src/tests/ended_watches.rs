//! Retained task facts are final deliveries, not evidence inferred from a watch end.
use super::*;

const WATCHER: Token = Token::new(30);
const RETAINED: u64 = 99;

fn ended_row(core: &Core, child: bool) -> tasks::TaskRecord {
    let mut row = core.tasks.task(1).unwrap().clone();
    row.number = RETAINED;
    row.phase = tasks::Phase::Ended(tasks::Ending::Cancelled {
        by: tasks::Party::Person(1),
        at: Wall::from_nanos(12),
        reason: Box::new(*b"released"),
        result: None,
    });
    row.ended_at = Some(Wall::from_nanos(13));
    if child {
        row.requester = tasks::Party::Task(1);
        row.depth = 1;
    }
    row
}

fn open_retained(core: &mut Core, limits: &Limits, subject: Watched) -> Token {
    let out = watch_outputs(core, limits, Event::Watch { watcher: WATCHER, sign_in: 10, subject, window: window() });
    let Request::Held(held) = &out[0] else { panic!("held load") };
    let Held::ReadLoad { load, range } = held.as_ref() else { panic!("retained load") };
    assert_eq!(range.scope, reads::Scope::Task { task: RETAINED });
    assert_eq!(finish_watch_outputs(core, limits).len(), 1, "construction waits for its terminal");
    *load
}

fn loaded(core: &mut Core, limits: &Limits, load: Token, row: tasks::TaskRecord) -> Box<[Request]> {
    watch_outputs(
        core,
        limits,
        Event::ReadLoaded {
            load,
            result: reads::Loaded::Page {
                rows: Box::new([stored_row(Record::Tasks(tasks::Stored::Ended(Box::new(row))), None)]),
                next: None,
            },
        },
    )
}

fn final_document(out: &[Request]) -> &Document {
    let Request::Held(held) = &out[0] else { panic!("held delivery") };
    let Held::Delivery { watcher, document } = held.as_ref() else { panic!("final document") };
    assert_eq!(*watcher, WATCHER);
    assert_eq!(out.len(), 2, "no end before the delivery terminal");
    document
}

#[test]
fn an_ended_root_or_child_has_its_retained_tree_then_one_ending() {
    for child in [false, true] {
        let (mut core, limits) = watch_core();
        let row = ended_row(&core, child);
        let load = open_retained(&mut core, &limits, Watched::Tree { task: RETAINED });
        let out = loaded(&mut core, &limits, load, row);
        let Document::Tree(tree) = final_document(&out) else { panic!("tree") };
        assert_eq!(tree.root.number, RETAINED);
        assert_eq!(tree.root.phase, PhaseLine::Ended);
        assert!(tree.rows.is_empty());
        assert!(tree.folded.is_empty());
        assert_eq!(tree.held_below, 0);
        assert_eq!(tree.next, None);
        assert!(core.tasks.task(RETAINED).is_none(), "retained facts never become live");
        let out = watch_outputs(&mut core, &limits, Event::Delivered { watcher: WATCHER });
        assert_one_watch_ending(&out, views::End::Gone);
        assert!(core.view_subjects.is_empty());
        assert_eq!(watch_outputs(&mut core, &limits, Event::Delivered { watcher: WATCHER }).len(), 1);
        let late = watch_outputs(&mut core, &limits, Event::ReadLoaded { load, result: reads::Loaded::Gone });
        assert_eq!(late.len(), 1, "no duplicate terminal");
        let Request::Decided = &late[0] else { panic!("only a decision terminal") };
    }
}

#[test]
fn a_wait_on_an_ended_task_gets_the_terminal_condition_in_its_first_document() {
    let (mut core, limits) = watch_core();
    let row = ended_row(&core, false);
    let load = open_retained(&mut core, &limits, Watched::Task { task: RETAINED });
    let out = loaded(&mut core, &limits, load, row);
    let Document::Task(task) = final_document(&out) else { panic!("task") };
    assert_eq!(task.chip.phase, PhaseLine::Ended);
    let Some(Ending::Cancelled { .. }) = &task.result else { panic!("retained cancellation") };
    assert_one_watch_ending(
        &watch_outputs(&mut core, &limits, Event::Delivered { watcher: WATCHER }),
        views::End::Gone,
    );
}

#[test]
fn leaving_during_retained_construction_drains_without_a_delivery() {
    let (mut core, limits) = watch_core();
    let row = ended_row(&core, false);
    let load = open_retained(&mut core, &limits, Watched::Tree { task: RETAINED });
    assert_eq!(watch_outputs(&mut core, &limits, Event::Unwatch { watcher: WATCHER }).len(), 1);
    assert_one_watch_ending(&loaded(&mut core, &limits, load, row), views::End::Unwatched);
}

#[test]
fn retained_construction_checks_current_sign_in_and_visibility() {
    for signed_out in [false, true] {
        let (mut core, limits) = watch_core();
        let mut row = ended_row(&core, false);
        let load = open_retained(&mut core, &limits, Watched::Tree { task: RETAINED });
        if signed_out {
            drop(watch_outputs(
                &mut core,
                &limits,
                Event::People(people::Event::SignOut { sign_in: 10, reply_to: ReplyTo::new(Token::new(91)) }),
            ));
        } else {
            row.project = 9;
        }
        let out = loaded(&mut core, &limits, load, row);
        assert_one_watch_ending(&out, if signed_out { views::End::SignedOut } else { views::End::Gone });
    }
}

#[test]
fn missing_retained_facts_end_without_claiming_success() {
    let (mut core, limits) = watch_core();
    let load = open_retained(&mut core, &limits, Watched::Task { task: RETAINED });
    let out = watch_outputs(&mut core, &limits, Event::ReadLoaded { load, result: reads::Loaded::Gone });
    assert_one_watch_ending(&out, views::End::Gone);
}

#[test]
fn retained_watch_loads_share_read_admission_and_reject_a_wrong_or_oversized_row() {
    for oversized in [false, true] {
        let (mut core, mut limits) = watch_core();
        limits.load_slots = 1;
        let mut row = ended_row(&core, false);
        let load = open_retained(&mut core, &limits, Watched::Tree { task: RETAINED });
        let out = watch_outputs(
            &mut core,
            &limits,
            Event::Watch {
                watcher: Token::new(31),
                sign_in: 10,
                subject: Watched::Task { task: RETAINED },
                window: window(),
            },
        );
        assert_watch_refusal(&out[0], WatchRefusal::Busy);
        if oversized {
            row.spec.words = skein_lib::bytes::zeroed(usize::try_from(limits.load_bytes).unwrap());
        } else {
            row.number += 1;
        }
        assert_one_watch_ending(&loaded(&mut core, &limits, load, row), views::End::Gone);
    }
}

#[test]
fn an_ended_tree_fits_when_the_tasks_whole_authority_does_not() {
    let (mut core, mut limits) = watch_core();
    let mut row = ended_row(&core, false);
    row.authority.grants = Box::new([tasks::Grant {
        connector: 1,
        kind: 1,
        pattern: tasks::Pattern {
            segments: Box::new([b"whole".as_slice().into()]),
            last: tasks::Last::Exact(b"end".as_slice().into()),
        },
    }]);
    limits.documents.text_bytes = 4;
    assert_eq!(task_from_row(&core, &limits, 1, &row, Wall::EPOCH), Err(Missing::TooLarge));
    let load = open_retained(&mut core, &limits, Watched::Tree { task: RETAINED });
    let out = loaded(&mut core, &limits, load, row);
    let Document::Tree(tree) = final_document(&out) else { panic!("bounded tree") };
    assert_eq!(tree.root.number, RETAINED);
    assert_eq!(tree.root.phase, PhaseLine::Ended);
}

#[test]
fn a_retained_watch_too_linked_ends_as_too_linked() {
    // Small permits four joins per read (profile/small.rs).
    const SMALL_JOINS: u32 = 4;
    for subject in [Watched::Task { task: RETAINED }, Watched::Tree { task: RETAINED }] {
        let (mut core, limits) = watch_core();
        let load = open_retained(&mut core, &limits, subject);
        let out = watch_outputs(
            &mut core,
            &limits,
            Event::ReadLoaded { load, result: reads::Loaded::TooLinked { joins: SMALL_JOINS } },
        );
        assert_one_watch_ending(&out, views::End::TooLinked { joins: SMALL_JOINS });
        assert!(core.view_subjects.is_empty());
    }
}
