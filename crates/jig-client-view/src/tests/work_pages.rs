#![expect(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::wildcard_enum_match_arm,
    reason = "ordinary fixtures script public events and query semantic nodes"
)]
use super::{
    cards::{contains, document, drive, start, text, words},
    pages::{build, deliver, named_binding},
};
use crate::tests::fixtures::{Fixture, ReadPage};
use crate::{JigBindings, words::Words};
use skein_ui::{Role, Tree};

use crate::{
    Event, Request,
    action::Act,
    address::{Address, Place, ProjectSection, TaskSection},
};
use jig_client::contract::*;
use skein_lib::{List, Wall};
pub(super) fn list<T>(values: impl IntoIterator<Item = T>) -> List<T> {
    let values: Vec<_> = values.into_iter().collect();
    let mut rows = List::with_capacity(u32::try_from(values.len()).unwrap());
    for value in values {
        rows.push(value).ok().unwrap();
    }
    rows
}
pub(super) fn part() -> Part {
    Part::Panel { connector: 2, task: 8 }
}
fn counts() -> PhaseCounts {
    PhaseCounts { ended: 0, waiting: 0, running: 0, held: 0, closing: 0, done: 0, failed: 0, cancelled: 0 }
}
pub(super) fn chip(task: u64) -> Chip {
    let mut chip = super::cards_objects::task_doc(&[]).chip;
    chip.task = task;
    chip
}
pub(super) fn entered(address: Address) -> (Fixture, Vec<Request>) {
    entered_arranged(address, crate::page::Arrangement::none())
}
pub(super) fn entered_arranged(address: Address, arrangement: crate::page::Arrangement) -> (Fixture, Vec<Request>) {
    let (mut d, f) = super::cards::start_arranged(arrangement);
    let mut frame = document();
    frame.projects.get_mut(0).unwrap().pool.as_mut().unwrap().left = 20;
    let initial = deliver(&mut d, f, Delivery::Me(Box::new(frame)));
    for request in &initial {
        if let Request::Client {
            asked: jig_client::Event::Read { handle: read, read: jig_client::Readable::Jig(Read::Inbox { .. }) },
        } = request
        {
            drive(
                &mut d,
                ReadPage {
                    read: *read,
                    result: ReadResult::Value(ReadValue::Inbox(Box::new(InboxPage {
                        cut: false,
                        rows: List::with_capacity(0),
                        before: None,
                        after: None,
                    }))),
                },
            );
        }
    }
    d.reclaim();
    let out = drive(&mut d, Event::Went { place: Place::Jig(address) });
    (d, out)
}
pub(super) fn task(section: TaskSection, executor: Executor) -> (Fixture, Vec<Request>) {
    task_arranged(section, executor, crate::page::Arrangement::none())
}
fn task_arranged(
    section: TaskSection,
    executor: Executor,
    arrangement: crate::page::Arrangement,
) -> (Fixture, Vec<Request>) {
    let (mut d, out) = entered_arranged(Address::Task { task: 8, section }, arrangement);
    let watch =
        out.iter()
            .find_map(|r| match r {
                Request::Client {
                    asked:
                        jig_client::Event::Watch {
                            handle: watch,
                            subject: jig_client::Watchable::Jig(Subject::Task { .. }),
                        },
                } => Some(*watch),
                _ => None,
            })
            .unwrap();
    let mut doc = super::cards_objects::task_doc(&[ActKind::Cancel]);
    doc.chip.executor = executor;
    doc.resources = list([TaskResource::Part(part())]);
    doc.context.as_mut().expect("supplied task context").parts = list([part()]);
    doc.inputs = list([OwnerTaskLink::Live(chip(9))]);
    doc.dependencies = list([OwnerTaskLink::Live(chip(9))]);
    let mut outputs = deliver(&mut d, watch, Delivery::Task(Box::new(doc)));
    outputs.extend(out);
    (d, outputs)
}
fn acts_are_named(tree: &Tree) {
    assert!(
        tree.nodes().iter().filter(|n| n.binding.is_some()).all(|n| n.name.as_ref().is_some_and(|n| !n.is_empty()))
    );
}
fn slot(tree: &Tree, bindings: &JigBindings) {
    let node = tree.nodes().iter().find(|n| n.role == Role::Slot).unwrap();
    let reference = node.slot.unwrap();
    assert_eq!(bindings.part(reference.token), Some(&part()));
}
fn task_sections(words: &Words) {
    for executor in [
        Executor::Agent { charter: 2 },
        Executor::Procedure { connector: 2, kind: 1 },
        Executor::Party { party: 1 },
        Executor::Role { project: 4, role: 1 },
    ] {
        for section in [TaskSection::Overview, TaskSection::Activity, TaskSection::Budget, TaskSection::Panels] {
            let (d, _) = task(section, executor);
            let (tree, bindings) = build(&d, 3, words);
            assert!(contains(&tree, b"Task 8"));
            assert!(contains(&tree, b"held") || contains(&tree, b"Held"));
            assert!(contains(&tree, b"Goal"));
            assert!(contains(&tree, b"Priority 1"));
            acts_are_named(&tree);
            if section == TaskSection::Overview {
                assert!(contains(&tree, b"whole task specification"));
                slot(&tree, &bindings);
            }
            if section == TaskSection::Panels {
                slot(&tree, &bindings);
            }
            if section == TaskSection::Activity && executor == (Executor::Procedure { connector: 2, kind: 1 }) {
                slot(&tree, &bindings);
            }
            if section == TaskSection::Budget {
                assert!(contains(&tree, b"Budget: 20"));
                assert!(contains(&tree, b"Left: 20"));
            }
        }
    }
}
fn plan_folds(words: &Words) {
    let (mut d, out) = task(TaskSection::Plan, Executor::Agent { charter: 2 });
    let tree_watch =
        out.iter()
            .find_map(|r| match r {
                Request::Client {
                    asked:
                        jig_client::Event::Watch {
                            handle: watch,
                            subject: jig_client::Watchable::Jig(Subject::Tree { .. }),
                        },
                } => Some(*watch),
                _ => None,
            })
            .unwrap();
    let node = |task, parent| TreeNode {
        chip: chip(task),
        parent,
        dependencies: list([]),
        proposals: list([]),
        spend: 1,
        folded: counts(),
        held_below: 1,
    };
    deliver(
        &mut d,
        tree_watch,
        Delivery::Tree(Box::new(jig_client::contract::Tree {
            task: 8,
            nodes: list([node(9, None), node(10, Some(9))]),
            folded: PhaseCounts { done: 3, ..counts() },
            held_below: 2,
        })),
    );
    let (tree, mut bindings) = build(&d, 3, words);
    assert_eq!(tree.nodes().iter().filter(|n| n.role == Role::TreeItem).count(), 2);
    assert!(contains(&tree, b"Held below: 2"));
    let fold = crate::decode(&mut bindings, named_binding(&tree, b"Fold subtree"), skein_ui::Act::Press).unwrap();
    drive(&mut d, Event::Act { action: fold });
    let (folded, mut bindings) = build(&d, 3, words);
    assert_eq!(folded.nodes().iter().filter(|n| n.role == Role::TreeItem).count(), 1);
    assert!(contains(&folded, b"Held below: 1"));
    let open = crate::decode(&mut bindings, named_binding(&folded, b"Open subtree"), skein_ui::Act::Press).unwrap();
    drive(&mut d, Event::Act { action: open });
    assert_eq!(build(&d, 3, words).0.nodes().iter().filter(|n| n.role == Role::TreeItem).count(), 2);
    for filter in
        [crate::page::task::PlanFilter::Held, crate::page::task::PlanFilter::Live, crate::page::task::PlanFilter::All]
    {
        drive(&mut d, Event::Act { action: Act::FilterPlan { filter } });
        acts_are_named(&build(&d, 3, words).0);
    }
    drive(&mut d, Event::Went { place: Place::Jig(Address::Task { task: 8, section: TaskSection::Plan }) });
    assert!(d.task_page().plan.nodes.is_empty());
}
pub(super) fn recurring_task(number: u64, words: &[u8]) -> ProposedTask {
    ProposedTask {
        deadline: NewDeadline::Named,
        number,
        spec: SpecificationDocument::complete(Specification {
            asks: false,
            words: text(words),
            parameters: list([]),
            inputs: list([]),
        }),
        executor: OwnerExecutor::Agent { charter: 2 },
        budget: 5,
        contract: TaskContractDocument::Report { words: 32 },
        authority: OwnerAuthorityDocument {
            authority: OwnerAuthority {
                models: None,
                tools: 0,
                grants: list([]),
                delegation: OwnerDelegation { kinds: list([]), tasks: 2, depth: 1 },
                spend: 5,
                deadline: None,
                notes: 0,
                note_resources: list([]),
            },
            cut: false,
        },
        funder: Funder::Recurring { project: 4, task: 7, period: 3 },
        dependencies: list([]),
        resources: list([]),
        wake: OwnerWakePolicy {
            words: WakeRule::Immediate,
            notices: WakeRule::Never,
            news: WakeRule::Never,
            results: ResultsWake::Each,
            questions: false,
            answers: false,
            timers: false,
        },
        priority: None,
        cut: false,
    }
}
pub(super) fn project(section: ProjectSection) -> (Fixture, Vec<Request>) {
    let (mut d, out) = entered(Address::Project { project: 4, section });
    let read = out
        .iter()
        .find_map(|r| match r {
            Request::Client {
                asked: jig_client::Event::Read { handle: read, read: jig_client::Readable::Jig(Read::Project { .. }) },
            } => Some(*read),
            _ => None,
        })
        .unwrap();
    let mut outputs =
        drive(&mut d, ReadPage { read, result: ReadResult::Value(ReadValue::Project(Box::new(project_doc()))) });
    outputs.extend(out);
    (d, outputs)
}
fn history(words: &Words) {
    let (mut d, out) = task(TaskSection::History, Executor::Agent { charter: 2 });
    let mut read = out
        .iter()
        .find_map(|r| match r {
            Request::Client {
                asked: jig_client::Event::Read { handle: read, read: jig_client::Readable::Jig(Read::History { .. }) },
            } => Some(*read),
            _ => None,
        })
        .unwrap();
    let doc = super::cards_objects::task_doc(&[]);
    let Some(TaskHold::Document(held)) = doc.hold else { panic!("supplied legacy historical hold") };
    let changes = [
        HistoryChange::Made { batch: 7 },
        HistoryChange::Amended { before: text(b"old instruction"), after: text(b"new instruction") },
        HistoryChange::Held(held),
        HistoryChange::Released,
        HistoryChange::Cancelled { reason: text(b"cancel history") },
        HistoryChange::Moved { from: Actor::Party(1), to: Actor::Party(2), reason: text(b"move history") },
    ];
    for (page, changes) in changes.chunks(2).enumerate() {
        drive(
            &mut d,
            ReadPage {
                read,
                result: ReadResult::Value(ReadValue::History(Box::new(HistoryPage {
                    cut: false,
                    rows: list(changes.iter().enumerate().map(|(index, change)| {
                        HistoryRow::Line(Box::new(HistoryLine {
                            position: u64::try_from(page.checked_mul(2).unwrap().checked_add(index).unwrap()).unwrap(),
                            task: 8,
                            by: Actor::Party(1),
                            at: Wall::EPOCH,
                            change: change.clone(),
                        }))
                    })),
                    before: Some(Cursor::new(Box::from(b"before".as_slice()), 1024).expect("bounded opaque cursor")),
                    after: Some(Cursor::new(Box::from(b"after".as_slice()), 1024).expect("bounded opaque cursor")),
                }))),
            },
        );
        let (tree, mut bindings) = build(&d, 3, words);
        let labels: &[&[u8]] = match page {
            0 => &[b"old instruction", b"new instruction"],
            1 => &[b"Released"],
            2 => &[b"cancel history", b"move history"],
            _ => unreachable!(),
        };
        for label in labels {
            assert!(contains(&tree, label), "{label:?}");
        }
        if page == 1 {
            assert!(
                tree.nodes().iter().any(|node| node.role == Role::Tab
                    && node.name.as_deref().is_some_and(|name| name.starts_with(b"Budget")))
            );
        }
        for label in [b"Read earlier".as_slice(), b"Read later"] {
            assert!(tree.nodes().iter().any(|node| node.role == Role::Button && node.name.as_deref() == Some(label)));
        }
        assert_eq!(tree.nodes().iter().filter_map(|n| n.key).filter(|key| key.kind == 21).count(), 2);
        assert_eq!(
            crate::decode(&mut bindings, named_binding(&tree, b"This task and its subtree"), skein_ui::Act::Press),
            Some(Act::HistoryScope { subtree: true })
        );
        let more = crate::decode(&mut bindings, named_binding(&tree, b"Read later"), skein_ui::Act::Press).unwrap();
        let out = drive(&mut d, Event::Act { action: more });
        read = out
            .iter()
            .find_map(|r| match r {
                Request::Client {
                    asked:
                        jig_client::Event::Read { handle: read, read: jig_client::Readable::Jig(Read::History { .. }) },
                } => Some(*read),
                _ => None,
            })
            .unwrap();
    }
}
pub(super) fn project_rows(d: &mut Fixture, out: &[Request]) {
    for r in out {
        match r {
            Request::Client {
                asked:
                    jig_client::Event::Watch {
                        handle: watch,
                        subject:
                            jig_client::Watchable::Jig(Subject::Tasks {
                                query: TaskQuery { tracked: Some(true), .. }, ..
                            }),
                    },
            } => {
                let doc = super::cards_objects::task_doc(&[]);
                deliver(
                    d,
                    *watch,
                    Delivery::Tasks(Box::new(Tasks {
                        rows: list([TaskLine {
                            chip: chip(8),
                            tracked: Some(2),
                            phases: OwnerPhaseCounts::ZERO,
                            budget: OwnerBudget { deadline: Some(Wall::EPOCH), ..doc.budget },
                            held_below: 1,
                            changed: Wall::EPOCH,
                            last_activity: Wall::EPOCH,
                            ended_at: None,
                            project: 1,
                            requester: OwnerRequester::Person(7),
                            label: 2,
                            flags: TaskFlags {
                                ended_by: EndedBy::Result,
                                conversation: false,
                                listed: true,
                                retention: Retention::Kept,
                            },
                            waits_on: WaitsOn::Nothing,
                            conversation: None,
                        }]),
                        more: true,
                    })),
                );
            }
            Request::Client {
                asked:
                    jig_client::Event::Watch { handle: watch, subject: jig_client::Watchable::Jig(Subject::Tasks { .. }) },
            } => {
                deliver(
                    d,
                    *watch,
                    Delivery::Tasks(Box::new(Tasks {
                        rows: list([crate::tests::fixtures::task_line(
                            chip(8),
                            4,
                            OwnerRequester::Person(1),
                            Wall::EPOCH,
                        )]),
                        more: true,
                    })),
                );
            }
            Request::Client {
                asked: jig_client::Event::Read { handle: read, read: jig_client::Readable::Jig(Read::Notes { .. }) },
            } => {
                drive(
                    d,
                    ReadPage {
                        read: *read,
                        result: ReadResult::Value(ReadValue::Notes(Box::new(NotePage {
                            cut: false,
                            rows: list([Note {
                                number: 1,
                                revision: Revision { value: 7 },
                                scope: NoteScope::Project(4),
                                description: DocumentText::complete(text(b"Useful correction")),
                                body: DocumentText::complete(text(b"whole note body")),
                                writer: NoteAuthor::Party { party: 1 },
                                tasks: list([8]),
                                changed: Some(Wall::EPOCH),
                                acts: list([Offered {
                                    effect: None,
                                    act: ActKind::EditNote,
                                    admission: Admission::Allowed,
                                }]),
                                cut: false,
                            }]),
                            before: Some(
                                Cursor::new(Box::from(b"before".as_slice()), 1024).expect("bounded opaque cursor"),
                            ),
                            after: Some(
                                Cursor::new(Box::from(b"after".as_slice()), 1024).expect("bounded opaque cursor"),
                            ),
                        }))),
                    },
                );
            }
            _ => {}
        }
    }
}
fn project_sections(words: &Words) {
    for section in [
        ProjectSection::Overview,
        ProjectSection::Goals,
        ProjectSection::Tasks,
        ProjectSection::Notes,
        ProjectSection::Settings,
        ProjectSection::Resources,
        ProjectSection::Policy,
        ProjectSection::Roles,
        ProjectSection::Spend,
        ProjectSection::Recurring,
    ] {
        let (mut d, out) = project(section);
        project_rows(&mut d, &out);
        let (tree, bindings) = build(&d, 3, words);
        assert!(contains(&tree, b"Workshop"));
        acts_are_named(&tree);
        match section {
            ProjectSection::Overview
            | ProjectSection::Settings
            | ProjectSection::Resources
            | ProjectSection::Policy => slot(&tree, &bindings),
            ProjectSection::Goals => {
                assert!(tree.nodes().iter().any(|n| n.name.as_deref() == Some(b"Goal specification")));
            }
            ProjectSection::Tasks => {
                assert!(
                    tree.nodes()
                        .iter()
                        .any(|node| node.role == Role::Button && node.name.as_deref() == Some(b"Tasks I asked for"))
                );
                assert!(
                    tree.nodes()
                        .iter()
                        .any(|node| node.role == Role::Button && node.name.as_deref() == Some(b"Executor active"))
                );
            }
            ProjectSection::Notes => {
                assert!(tree.nodes().iter().any(|n| n.name.as_deref() == Some(b"Search notes by description")));
                for label in [b"whole note body".as_slice(), b"Useful correction"] {
                    assert!(contains(&tree, label));
                }
                for label in [b"Read earlier".as_slice(), b"Read later"] {
                    assert!(
                        tree.nodes()
                            .iter()
                            .any(|node| node.role == Role::Button && node.name.as_deref() == Some(label))
                    );
                }
            }
            ProjectSection::Roles => assert!(contains(&tree, b"Notifier")),
            ProjectSection::Spend => assert!(contains(&tree, b"Period 3")),
            ProjectSection::Recurring => assert!(contains(&tree, b"Daily work")),
        }
    }
}
fn goal_form(words: &Words) {
    let (mut d, _) = project(ProjectSection::Goals);
    let form = crate::page::project::goals::GoalForm {
        project: 4,
        executor: OwnerExecutor::Agent { charter: 2 },
        budget: 30,
        deadline: None,
        priority: 1,
    };
    drive(&mut d, Event::Act { action: Act::GoalForm { form } });
    let (tree, mut bindings) = build(&d, 3, words);
    assert!(contains(&tree, b"this will be proposed"));
    let action = crate::decode(
        &mut bindings,
        named_binding(&tree, b"Goal budget"),
        skein_ui::Act::Input { value: Box::from(b"40".as_slice()) },
    )
    .unwrap();
    match action {
        Act::GoalForm { form: changed } => {
            assert_eq!(changed, crate::page::project::goals::GoalForm { budget: 40, ..form });
        }
        _ => panic!("typed goal field"),
    }
    let at = crate::confirmation::amendment::deadline(b"2026-10-09 17:00").unwrap();
    let action = crate::decode(
        &mut bindings,
        named_binding(&tree, b"Goal deadline (UTC, YYYY-MM-DD HH:MM)"),
        skein_ui::Act::Input { value: Box::from(b"2026-10-09 17:00".as_slice()) },
    )
    .unwrap();
    assert_eq!(action, Act::GoalForm { form: crate::page::project::goals::GoalForm { deadline: Some(at), ..form } });
    drive(&mut d, Event::Act { action });
    let (dated, _) = build(&d, 3, words);
    let field = dated
        .nodes()
        .iter()
        .find(|node| node.name.as_deref() == Some(b"Goal deadline (UTC, YYYY-MM-DD HH:MM)"))
        .unwrap();
    assert_eq!(field.value.as_ref().unwrap().text.as_ref(), b"2026-10-09 17:00");
    assert!(
        crate::decode(
            &mut bindings,
            named_binding(&tree, b"Goal priority"),
            skein_ui::Act::Input { value: Box::from(b"bad".as_slice()) }
        )
        .is_none()
    );
}
pub(super) fn deployment_domain() -> Fixture {
    let (mut d, out) = entered(Address::Deployment);
    let watch = out
        .iter()
        .find_map(|r| match r {
            Request::Client { asked: jig_client::Event::Watch { handle: watch, subject: jig_client::Watchable::Jig(Subject::Deployment) } } => Some(*watch),
            _ => None,
        })
        .unwrap();
    let value = Deployment {
        hosts: list([
            Host { number: 1, connected: true, grace: None, used: 1, free: 2, runs: list([chip(8)]), refused: None },
            Host {
                number: 2,
                connected: false,
                grace: Some(Wall::EPOCH),
                used: 0,
                free: 0,
                runs: list([]),
                refused: Some(text(b"No capacity")),
            },
        ]),
        accounts: list([Account {
            number: 1,
            usable: false,
            reason: Some(text(b"Cooling down")),
            runs: list([chip(8)]),
            waiting: list([chip(9)]),
        }]),
        capacity: list([
            Capacity { project: None, live: 3, limit: 4 },
            Capacity { project: Some(4), live: 2, limit: 3 },
        ]),
        proposals: 2,
        refusals: list([Refusal::Limit { which: Limit::LiveTasks { project: 4, limit: 3 } }]),
        parts: list([part()]),
    };
    deliver(&mut d, watch, Delivery::Deployment(Box::new(value)));
    d
}
fn deployment(words: &Words) {
    let d = deployment_domain();
    let (tree, bindings) = build(&d, 3, words);
    for label in [
        b"Connected".as_slice(),
        b"Lost",
        b"Grace holds until",
        b"Slots used: 1",
        b"Slots free: 2",
        b"Cooling down",
        b"Live tasks: 3",
        b"Task limit: 4",
        b"Pending proposals",
    ] {
        assert!(contains(&tree, label), "{label:?}");
    }
    slot(&tree, &bindings);
    acts_are_named(&tree);
}
#[test]
fn work_pages_show_executor_sections_folds_project_settings_and_deployment_capacity() {
    let words = words();
    task_sections(words);
    plan_folds(words);
    history(words);
    project_sections(words);
    recurring_times_use_supplied_boundaries_without_inferred_cadence(words);
    goal_form(words);
    deployment(words);
}

fn recurring_times_use_supplied_boundaries_without_inferred_cadence(words: &Words) {
    for next in [None, Some(Wall::from_nanos(3_600_000_000_000))] {
        for began in [None, Some(Wall::from_nanos(1_800_000_000_000))] {
            let (mut d, out) = entered(Address::Project { project: 4, section: ProjectSection::Recurring });
            let read =
                out.iter()
                    .find_map(|request| match request {
                        Request::Client {
                            asked:
                                jig_client::Event::Read {
                                    handle: read,
                                    read: jig_client::Readable::Jig(Read::Project { .. }),
                                },
                        } => Some(*read),
                        _ => None,
                    })
                    .unwrap();
            let mut doc = project_doc();
            let recurring = doc.recurring.get_mut(0).unwrap();
            recurring.next = next;
            recurring.current_period = 9;
            recurring.began = began;
            crate::tests::fixtures::set_creating(&mut d, doc.number, &doc.acts.get(0).unwrap().admission);
            drive(&mut d, ReadPage { read, result: ReadResult::Value(ReadValue::Project(Box::new(doc))) });
            let (tree, _) = build(&d, 3, words);
            assert!(contains(&tree, b"Daily work"));
            assert!(contains(&tree, b"Last batch period 3"));
            assert!(contains(&tree, b"Current period 9"));
            assert_eq!(contains(&tree, b"Period began at 01:30"), began.is_some());
            assert_eq!(contains(&tree, b"Runs at the next period"), next.is_none());
            assert_eq!(contains(&tree, b"Next runs at 02:00"), next.is_some());
            assert!(!contains(&tree, b"Deadline"));
        }
    }
}

#[test]
fn procedure_goal_choices_preserve_pairs_without_me_charters() {
    use crate::page::project::goals::GoalForm;
    let words = words();
    let (mut d, f) = start();
    let frame = document();
    deliver(&mut d, f, Delivery::Me(Box::new(frame)));
    let out = drive(
        &mut d,
        Event::Went { place: Place::Jig(Address::Project { project: 4, section: ProjectSection::Goals }) },
    );
    let read = out
        .iter()
        .find_map(|r| match r {
            Request::Client {
                asked: jig_client::Event::Read { handle: read, read: jig_client::Readable::Jig(Read::Project { .. }) },
            } => Some(*read),
            _ => None,
        })
        .unwrap();
    let mut doc = project_doc();
    doc.acts = list([Offered {
        effect: None,
        act: ActKind::Reorder,
        admission: Admission::Bounded {
            within: Bounds::Create(CreateBounds {
                authority: Box::new(crate::tests::cards_objects::owner_authority()),
                pool_left: 30,
                charters: list([99]),
                procedures: list([
                    CreateProcedure { connector: 7, code: u32::MAX },
                    CreateProcedure { connector: u16::MAX, code: u32::MAX },
                ]),
                latest: None,
                budget: 3,
                maximum_budget: 50,
                maximum_deadline: None,
            }),
            beyond: Beyond::Proposed { to: Recipient::Party(9) },
        },
    }]);
    crate::tests::fixtures::set_creating(&mut d, doc.number, &doc.acts.get(0).unwrap().admission);
    drive(&mut d, ReadPage { read, result: ReadResult::Value(ReadValue::Project(Box::new(doc))) });
    let (tree, mut bindings) = build(&d, 3, words);
    for (label, executor) in [
        (b"Charter 99".as_slice(), OwnerExecutor::Agent { charter: 99 }),
        (b"Connector 7, procedure 4294967295".as_slice(), OwnerExecutor::Procedure { connector: 7, code: u32::MAX }),
        (
            b"Connector 65535, procedure 4294967295".as_slice(),
            OwnerExecutor::Procedure { connector: u16::MAX, code: u32::MAX },
        ),
    ] {
        let action = crate::decode(&mut bindings, named_binding(&tree, label), skein_ui::Act::Press).unwrap();
        assert_eq!(
            action,
            Act::GoalForm { form: GoalForm { project: 4, executor, budget: 30, deadline: None, priority: 0 } }
        );
    }
    assert!(!tree.nodes().iter().any(|n| n.name.as_deref() == Some(b"Charter 2")));
    let choice = crate::decode(
        &mut bindings,
        named_binding(&tree, b"Connector 65535, procedure 4294967295"),
        skein_ui::Act::Press,
    )
    .unwrap();
    drive(&mut d, Event::Act { action: choice });
    let (tree, mut bindings) = build(&d, 3, words);
    let edit = crate::decode(
        &mut bindings,
        named_binding(&tree, b"Goal budget"),
        skein_ui::Act::Input { value: Box::from(b"4".as_slice()) },
    )
    .unwrap();
    let exact = OwnerExecutor::Procedure { connector: u16::MAX, code: u32::MAX };
    assert_eq!(
        edit,
        Act::GoalForm { form: GoalForm { project: 4, executor: exact, budget: 4, deadline: None, priority: 0 } }
    );
    drive(&mut d, Event::Act { action: edit });
    drive(
        &mut d,
        Event::Act { action: Act::Edit { field: crate::fields::Purpose::NewGoal, text: text(b"procedure words") } },
    );
    let (tree, _) = build(&d, 3, words);
    assert!(contains(&tree, b"proposed"));
    drive(&mut d, Event::Act { action: Act::Submit { field: crate::fields::Purpose::NewGoal } });
    let mut b = skein_ui::Builder::new(&super::cards::limits());
    let mut bindings = JigBindings::new(32, super::cards::CLIENT.text);
    b.open(Role::Page);
    crate::confirmation::confirmation(&mut b, &d.view(), words, &mut bindings);
    b.close();
    let tree = b.finish();
    assert!(contains(&tree, b"Connector 65535, procedure 4294967295"));
    assert!(contains(&tree, b"procedure words"));
    assert!(!contains(&tree, b"Charter 65535"));
}

#[test]
fn a_task_section_not_shown_builds_nothing() {
    let (d, _) = task(TaskSection::Overview, Executor::Agent { charter: 2 });
    let mut b = skein_ui::Builder::new(&super::cards::limits());
    let mut bindings = JigBindings::new(64, super::cards::CLIENT.text);
    b.open(Role::Page);
    assert!(!crate::sections::section(
        &mut b,
        &d.view(),
        words(),
        &mut bindings,
        crate::page::Section::Task { task: 9, section: TaskSection::Overview }
    ));
    b.close();
    assert_eq!(b.finish().len(), 1);
}
#[test]
fn the_tabs_mark_the_addresss_own_section_current() {
    let (d, _) = task(TaskSection::Plan, Executor::Agent { charter: 2 });
    let (tree, mut bindings) = build(&d, 3, words());
    let tabs: Vec<_> = tree.nodes().iter().filter(|n| n.role == Role::Tab).collect();
    assert_eq!(tabs.len(), 6);
    for (tab, section) in tabs.iter().zip([
        TaskSection::Overview,
        TaskSection::Activity,
        TaskSection::Plan,
        TaskSection::History,
        TaskSection::Budget,
        TaskSection::Panels,
    ]) {
        assert_eq!(
            tab.states.as_slice(),
            if section == TaskSection::Plan { &[skein_ui::State::Current][..] } else { &[] },
            "{section:?}"
        );
        assert_eq!(
            crate::decode(&mut bindings, tab.binding.unwrap().token, skein_ui::Act::Press),
            Some(Act::Go { address: Address::Task { task: 8, section } }),
            "{section:?}"
        );
    }
}
#[test]
fn each_task_section_carries_its_region_name() {
    use crate::regions::JigRegion;
    for (section, region, name) in [
        (TaskSection::Overview, JigRegion::TaskOverview, b"jig-task-overview".as_slice()),
        (TaskSection::Activity, JigRegion::TaskActivity, b"jig-task-activity".as_slice()),
        (TaskSection::Plan, JigRegion::TaskPlan, b"jig-task-plan".as_slice()),
        (TaskSection::History, JigRegion::TaskHistory, b"jig-task-history".as_slice()),
        (TaskSection::Budget, JigRegion::TaskBudget, b"jig-task-budget".as_slice()),
        (TaskSection::Panels, JigRegion::TaskPanels, b"jig-task-panels".as_slice()),
    ] {
        let (d, _) = task(section, Executor::Agent { charter: 2 });
        let (tree, _) = build(&d, 3, words());
        let node = tree.nodes().iter().find(|n| n.region == Some(region.region())).unwrap();
        assert_eq!(node.role, Role::Region, "{section:?}");
        assert_eq!(region.name(), name);
        let index = tree.nodes().iter().position(|n| core::ptr::eq(n, node)).unwrap();
        assert_eq!(tree.nodes()[index + 1].role, Role::Heading { level: 2 });
    }
}

mod sections;

fn arranged_task() -> Fixture {
    task_arranged(
        TaskSection::Plan,
        Executor::Agent { charter: 2 },
        crate::page::Arrangement {
            task: list([
                TaskSection::Overview,
                TaskSection::Activity,
                TaskSection::Plan,
                TaskSection::History,
                TaskSection::Budget,
                TaskSection::Panels,
            ]),
            project: list([]),
        },
    )
    .0
}
#[test]
fn an_arranged_task_page_shows_its_six_sections_in_order() {
    use crate::regions::JigRegion;
    let (tree, _) = build(&arranged_task(), 3, words());
    let regions: Vec<_> =
        tree.nodes().iter().filter_map(|n| n.region).filter(|r| *r != JigRegion::TaskHeader.region()).collect();
    let sections = [
        JigRegion::TaskOverview,
        JigRegion::TaskActivity,
        JigRegion::TaskPlan,
        JigRegion::TaskHistory,
        JigRegion::TaskBudget,
        JigRegion::TaskPanels,
    ];
    let shown: Vec<_> = regions.into_iter().filter(|r| sections.iter().any(|s| s.region() == *r)).collect();
    assert_eq!(shown, sections.map(JigRegion::region));
}
#[test]
fn an_arranged_task_page_showing_every_section_has_no_tabs() {
    let (tree, _) = build(&arranged_task(), 3, words());
    assert_eq!(tree.nodes().iter().filter(|n| n.role == Role::Tabs).count(), 0);
}
#[test]
fn the_addresss_own_section_is_marked_current_on_an_arranged_page() {
    use crate::regions::JigRegion;
    let (tree, _) = build(&arranged_task(), 3, words());
    for region in [
        JigRegion::TaskOverview,
        JigRegion::TaskActivity,
        JigRegion::TaskPlan,
        JigRegion::TaskHistory,
        JigRegion::TaskBudget,
        JigRegion::TaskPanels,
    ] {
        let node = tree.nodes().iter().find(|n| n.region == Some(region.region())).unwrap();
        assert_eq!(
            node.states.as_slice(),
            if region == JigRegion::TaskPlan { &[skein_ui::State::Current][..] } else { &[] },
            "{region:?}"
        );
    }
}

mod project;
pub(super) use project::project_doc;
