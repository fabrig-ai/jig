#![expect(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::wildcard_enum_match_arm,
    reason = "ordinary maximum fixtures script public sources and inspect node budgets"
)]
use super::{
    cards::{CLIENT, document, drive, start, words},
    pages::deliver,
    work_pages::{chip, list, part, project_doc},
};
use crate::tests::fixtures::{Fixture, ReadPage};
use crate::{JigBindings, Owner, words::Words};
use skein_ui::{Builder, Role, SlotRef, Tree};

use crate::{
    Event, Request,
    action::Act,
    address::{Address, Place, ProjectSection, TaskSection},
};
use jig_client::contract::*;
use skein_lib::List;
use skein_lib::{Token, Wall};
fn full() -> Text {
    let mut bytes = Vec::new();
    bytes.resize(usize::try_from(CLIENT.text).unwrap(), b'x');
    Text::new(bytes.into_boxed_slice(), CLIENT.text).unwrap()
}
fn full_owner_budget() -> OwnerBudget {
    OwnerBudget {
        numbers: OwnerNumbers { budget: u64::MAX, spent: u64::MAX, spent_below: u64::MAX, reserved: u64::MAX },
        left: u64::MAX,
        funder: Funder::Recurring { project: u32::MAX, task: u64::MAX, period: u64::MAX },
        deadline: Some(Wall::from_nanos(u64::MAX)),
    }
}
fn full_chip(number: u64) -> Chip {
    let mut value = chip(number);
    value.title = DocumentText::complete(full());
    value
}
fn parts() -> List<Part> {
    list([part(), Part::Panel { connector: 3, task: 8 }])
}
fn full_owner_authority() -> OwnerAuthorityDocument {
    let pattern = full_pattern();
    let grant = OwnerGrant { connector: 2, kind: u16::MAX, pattern: pattern.clone() };
    let scope = OwnerResourceScope { connector: 2, pattern };
    OwnerAuthorityDocument {
        authority: OwnerAuthority {
            models: None,
            tools: u64::MAX,
            grants: list([grant.clone(), grant]),
            delegation: OwnerDelegation {
                kinds: list([OwnerAuthorityExecutor::Charter(u32::MAX), OwnerAuthorityExecutor::Role(u32::MAX)]),
                tasks: u32::MAX,
                depth: u32::MAX,
            },
            spend: u64::MAX,
            deadline: Some(Wall::from_nanos(u64::MAX)),
            notes: u8::MAX,
            note_resources: list([scope.clone(), scope]),
        },
        cut: true,
    }
}
fn authority() -> Authority {
    Authority {
        models: None,
        budget: u64::MAX,
        grants: list([
            Grant { connector: 2, kind: 1, pattern: full_pattern() },
            Grant { connector: 3, kind: 2, pattern: full_pattern() },
        ]),
        task_kinds: list([Executor::Agent { charter: 2 }, Executor::Procedure { connector: 3, kind: 1 }]),
        depth: 2,
        tasks: 2,
        deadline: Some(Wall::from_nanos(u64::MAX)),
        tools: list([1, 2]),
        note_scopes: list([
            NoteScope::Resource { project: 4, connector: 2, pattern: full_pattern() },
            NoteScope::Resource { project: 4, connector: 3, pattern: full_pattern() },
        ]),
    }
}
pub(super) fn entered(address: Address) -> (Fixture, Vec<Request>) {
    let (mut d, watch) = start();
    let mut frame = document();
    let mut project = frame.projects.get(0).unwrap().clone();
    project.name = DocumentText::complete(full());
    project.role = HeldRole::Policy { role: 1 };
    let mut second = project.clone();
    second.project = 5;
    second.role = HeldRole::Policy { role: 2 };
    frame.projects = list([project, second]);
    frame.party.name = DocumentText::complete(full());
    frame.party.login = DocumentText::complete(full());
    deliver(&mut d, watch, Delivery::Me(Box::new(frame)));
    let out = drive(&mut d, Event::Went { place: Place::Jig(address) });
    (d, out)
}
pub(super) fn render(d: &Fixture, words: &Words) -> Tree {
    let limits = crate::limits::of_words(&CLIENT, words).unwrap();
    let mut b = Builder::new(&limits);
    let mut bindings = JigBindings::new(limits.bindings, CLIENT.text);
    b.open(Role::Page);
    assert!(crate::jig(
        &mut b,
        &d.view(),
        SlotRef { owner: (Owner::Application { number: 1 }).opaque(), token: Token::new(1) },
        words,
        &mut bindings
    ));
    b.close();
    let tree = b.finish();
    assert!(u32::try_from(tree.nodes().len()).unwrap() <= limits.nodes);
    assert_eq!(tree.nodes().iter().filter(|n| n.role == Role::Heading { level: 1 }).count(), 1);
    assert!(
        tree.nodes()
            .iter()
            .filter(|n| n.binding.is_some())
            .all(|n| n.name.as_ref().is_some_and(|name| !name.is_empty()))
    );
    tree
}
fn row(position: u64, event: Activity) -> ActivityLine {
    ActivityLine { position, at: Wall::EPOCH, event }
}
fn full_call(number: u64) -> Call {
    Call {
        number,
        tool: full(),
        input: full(),
        outcome: Some(full()),
        failed: false,
        objects: list([ObjectKey::Task(8), ObjectKey::Task(9)]),
        parts: parts(),
    }
}
fn full_turn() -> Turn {
    Turn {
        messages: list([
            Message { speaker: Speaker::Assistant, blocks: list([Block::Text(full()), Block::Call(full_call(1))]) },
            Message { speaker: Speaker::Assistant, blocks: list([Block::Text(full()), Block::Call(full_call(2))]) },
        ]),
        used: Usage { input: u64::MAX, output: u64::MAX, cached: u64::MAX, cache_written: u64::MAX },
        spent: u64::MAX,
        read_through: None,
        cut: true,
    }
}
fn task_pages(words: &Words) {
    for section in [
        TaskSection::Overview,
        TaskSection::Activity,
        TaskSection::Plan,
        TaskSection::History,
        TaskSection::Budget,
        TaskSection::Panels,
    ] {
        let (mut d, out) = entered(Address::Task { task: 8, section });
        let watch = out
            .iter()
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
        let mut doc = super::cards_objects::task_doc(&[ActKind::Cancel, ActKind::Amend]);
        doc.chip = full_chip(8);
        doc.chip.executor = Executor::Agent { charter: 2 };
        doc.spec = specification(full());
        doc.instructions = Some(DocumentText { bytes: full(), cut: true });
        doc.authority = full_owner_authority();
        doc.lineage_chips = list([full_chip(1), full_chip(2)]);
        doc.inputs = list([OwnerTaskLink::Live(full_chip(9)), OwnerTaskLink::Live(full_chip(10))]);
        doc.dependencies = doc.inputs.clone();
        doc.references = doc.inputs.clone();
        doc.resources = list(parts().as_slice().iter().cloned().map(TaskResource::Part));
        doc.context.as_mut().expect("supplied task context").parts = parts();
        doc.context.as_mut().expect("supplied task context").writer_slots = list([
            WriterSlot { resource: part(), task: 9, attempt: Some(1) },
            WriterSlot { resource: part(), task: 10, attempt: Some(2) },
        ]);
        doc.subscriptions.entries = list([
            Subscription { number: 1, kind: SubscriptionKind::Topic { connector: 2, topic: 1 } },
            Subscription { number: 2, kind: SubscriptionKind::Task { target: u64::MAX, held: true, result: true } },
        ]);
        doc.context.as_mut().expect("supplied task context").spend = list([
            SpendLine { task: 9, attempt: Some(1), spent: u64::MAX },
            SpendLine { task: 10, attempt: None, spent: u64::MAX },
        ]);
        doc.activity = 2;
        doc.context.as_mut().expect("supplied task context").runtime =
            Runtime::Running { attempt: 1, since: Wall::EPOCH, tool: Some(full()) };
        let mut sources = deliver(&mut d, watch, Delivery::Task(Box::new(doc)));
        sources.extend(out);
        for request in sources {
            match request {
                Request::Client {
                    asked:
                        jig_client::Event::Watch {
                            handle: watch,
                            subject: jig_client::Watchable::Jig(Subject::Tree { .. }),
                        },
                } => fill_tree(&mut d, watch),
                Request::Client {
                    asked:
                        jig_client::Event::Read { handle: read, read: jig_client::Readable::Jig(Read::Activity { .. }) },
                } => fill_activity(&mut d, read),
                Request::Client {
                    asked:
                        jig_client::Event::Read { handle: read, read: jig_client::Readable::Jig(Read::History { .. }) },
                } => fill_history(&mut d, read),
                _ => {}
            }
        }
        let tree = render(&d, words);
        if section == TaskSection::Plan {
            assert_eq!(tree.nodes().iter().filter(|node| node.role == Role::TreeItem).count(), 2);
        }
        if section == TaskSection::Activity {
            assert!(tree.nodes().iter().any(|node| node.role == Role::Log));
        }
    }
}

fn goal_offers(doc: &mut Project) {
    doc.acts = list([Offered {
        effect: None,
        act: ActKind::Reorder,
        admission: Admission::Bounded {
            within: Bounds::Create(CreateBounds {
                authority: Box::new(crate::tests::cards_objects::owner_authority()),
                pool_left: u64::MAX,
                charters: list([1, 2]),
                procedures: list([
                    CreateProcedure { connector: u16::MAX, code: u32::MAX },
                    CreateProcedure { connector: u16::MAX, code: u32::MAX.checked_sub(1).unwrap() },
                ]),
                latest: None,
                budget: 0,
                maximum_budget: u64::MAX,
                maximum_deadline: None,
            }),
            beyond: Beyond::Proposed { to: Recipient::Roles { project: 4, roles: list([1, 2]) } },
        },
    }]);
}
fn other_pages(words: &Words) {
    deployment(words);
    conversations(words);
    inbox(words);
    not_the_work(words);
}
fn deployment(words: &Words) {
    let (mut d, out) = entered(Address::Deployment);
    let watch = out
        .iter()
        .find_map(|r| match r {
            Request::Client { asked: jig_client::Event::Watch { handle: watch, subject: jig_client::Watchable::Jig(Subject::Deployment) } } => Some(*watch),
            _ => None,
        })
        .unwrap();
    let host = |number| Host {
        number,
        connected: false,
        grace: Some(Wall::from_nanos(u64::MAX)),
        used: u32::MAX,
        free: u32::MAX,
        runs: list([full_chip(8), full_chip(9)]),
        refused: Some(full()),
    };
    let account = |number| Account {
        number,
        usable: false,
        reason: Some(full()),
        runs: list([full_chip(8), full_chip(9)]),
        waiting: list([full_chip(8), full_chip(9)]),
    };
    deliver(
        &mut d,
        watch,
        Delivery::Deployment(Box::new(Deployment {
            hosts: list([host(1), host(2)]),
            accounts: list([account(1), account(2)]),
            capacity: list([
                Capacity { project: None, live: u32::MAX, limit: u32::MAX },
                Capacity { project: Some(4), live: u32::MAX, limit: u32::MAX },
            ]),
            proposals: u32::MAX,
            refusals: list([Refusal::Limit { which: Limit::Objects }, Refusal::Limit { which: Limit::Reads }]),
            parts: parts(),
        })),
    );
    render(&d, words);
}
fn conversations(words: &Words) {
    let (mut d, out) = entered(Address::Conversations { project: None });
    let watch =
        out.iter()
            .find_map(|r| match r {
                Request::Client {
                    asked:
                        jig_client::Event::Watch {
                            handle: watch,
                            subject: jig_client::Watchable::Jig(Subject::Tasks { .. }),
                        },
                } => Some(*watch),
                _ => None,
            })
            .unwrap();
    let line = |number| {
        let mut chip = full_chip(number);
        chip.details = Box::new(ChipPhase::Owner {
            phase: OwnerChipPhase::Active(RunStage::Idle),
            details: Box::new(super::owner_chip::details()),
        });
        let mut line = crate::tests::fixtures::task_line(chip, 4, OwnerRequester::Person(1), Wall::EPOCH);
        line.phases = OwnerPhaseCounts {
            waiting: u32::MAX,
            idle: u32::MAX,
            starting: u32::MAX,
            running: u32::MAX,
            backing_off: u32::MAX,
            closing: u32::MAX,
            held: u32::MAX,
            done: u32::MAX,
            failed: u32::MAX,
            cancelled: u32::MAX,
        };
        line.conversation = Some(ConversationRead { seen: 0, unread: Count { value: u32::MAX, limited: true } });
        line.budget = full_owner_budget();
        line
    };
    deliver(&mut d, watch, Delivery::Tasks(Box::new(Tasks { rows: list([line(8), line(9)]), more: true })));
    render(&d, words);
}
fn inbox(words: &Words) {
    let (mut d, out) = entered(Address::Inbox { project: None });
    let watch =
        out.iter()
            .find_map(|r| match r {
                Request::Client {
                    asked:
                        jig_client::Event::Watch {
                            handle: watch,
                            subject: jig_client::Watchable::Jig(Subject::Inbox { .. }),
                        },
                } => Some(*watch),
                _ => None,
            })
            .unwrap();
    let entry = |number| InboxEntry {
        object: ObjectKey::Task(number),
        group: InboxGroup::PersonTasks,
        project: 4,
        lineage: list([full_chip(1), full_chip(2)]),
        goal: Some(8),
        recipient: Some(Recipient::Role { project: 4, role: 1 }),
        stall: Some(Wall::from_nanos(u64::MAX)),
        reply: None,
        position: number,
        at: Wall::EPOCH,
    };
    deliver(
        &mut d,
        watch,
        Delivery::Inbox(Box::new(Inbox {
            entries: list([entry(8), entry(9)]),
            needs: Needs { questions: u32::MAX, proposals: u32::MAX, escalations: u32::MAX, person_tasks: u32::MAX },
            updates: Count { value: u32::MAX, limited: true },
            more: true,
            results_seen: 0,
            replies_seen: list([ReadPosition { task: 8, through: 0 }, ReadPosition { task: 9, through: 0 }]),
        })),
    );
    for request in &out {
        if let Request::Client {
            asked: jig_client::Event::Read { handle: read, read: jig_client::Readable::Jig(Read::Inbox { group, .. }) },
        } = request
        {
            let mut first = entry(if *group == InboxReadGroup::Needs { 10 } else { 12 });
            let mut second = entry(if *group == InboxReadGroup::Needs { 11 } else { 13 });
            if *group == InboxReadGroup::Updates {
                first.group = InboxGroup::Results;
                second.group = InboxGroup::Results;
            }
            drive(
                &mut d,
                ReadPage {
                    read: *read,
                    result: ReadResult::Value(ReadValue::Inbox(Box::new(InboxPage {
                        cut: false,
                        rows: list([first, second]),
                        before: Some(full_cursor()),
                        after: Some(full_cursor()),
                    }))),
                },
            );
        }
    }
    assert_eq!(d.inbox().rows.len(), 6);
    render(&d, words);
}
fn not_the_work(words: &Words) {
    let mut d = Fixture::new(&CLIENT);
    drive(&mut d, Event::Start { seed: 8, place: Place::Jig(Address::Inbox { project: None }), records: list([]) });
    drive(&mut d, jig_client::Up::SignedOut);
    render(&d, words);
    let (d, _) = start();
    render(&d, words);
}
#[test]
fn every_page_with_full_windows_nested_lists_and_maximum_text_fits_its_composed_capacity() {
    let words = words();
    task_pages(words);
    project_pages(words);
    other_pages(words);
}
#[test]
#[expect(
    clippy::disallowed_macros,
    clippy::print_stdout,
    reason = "reports configured capacity evidence for the composition handoff"
)]
fn capacity_composes_siblings_fragments_and_application_pages_with_checked_overflow() {
    let words = words();
    let limits = crate::limits::of_words(&CLIENT, words).unwrap();
    let largest = crate::page_nodes(&CLIENT, 0).unwrap();
    assert_eq!(
        limits.nodes,
        largest
            .checked_add(
                crate::banner_nodes(&CLIENT).unwrap().checked_add(crate::navigation_nodes(&CLIENT).unwrap()).unwrap()
            )
            .unwrap()
            .checked_add(crate::confirmation_nodes(&CLIENT).unwrap())
            .unwrap()
            .checked_add(crate::notices_nodes(&CLIENT).unwrap())
            .unwrap()
            .checked_add(3)
            .unwrap()
    );
    let extra = crate::limits::compose(
        &CLIENT,
        words,
        &crate::page::Arrangement::none(),
        largest.checked_add(7).unwrap(),
        9,
        2,
    )
    .unwrap();
    assert_eq!(extra.nodes, limits.nodes.checked_add(16).unwrap());
    assert_eq!(extra.depth, limits.depth.checked_add(2).unwrap());
    assert_eq!(crate::page_nodes(&CLIENT, 9), largest.checked_add(9));
    assert!(crate::limits::compose(&CLIENT, words, &crate::page::Arrangement::none(), u32::MAX, 0, 0).is_none());
    assert!(crate::page_nodes(&CLIENT, u32::MAX).is_none());
    let mut huge = CLIENT;
    huge.rows = u32::MAX;
    assert!(crate::page_nodes(&huge, 0).is_none());

    let (mut d, _) = start();
    drive(&mut d, Event::Went { place: Place::Elsewhere });
    let mut b = Builder::new(&extra);
    let mut bindings = JigBindings::new(limits.bindings, CLIENT.text);
    b.open(Role::Page);
    assert!(!crate::jig(
        &mut b,
        &d.view(),
        SlotRef { owner: (Owner::Application { number: 1 }).opaque(), token: Token::new(1) },
        words,
        &mut bindings
    ));
    b.open(Role::Main);
    b.open(Role::Heading { level: 1 });
    b.text(skein_ui::Run::Plain, b"Application page");
    b.close();
    b.close();
    crate::after_page(&mut b, &d.view(), words, &mut bindings);
    b.close();
    let slots = b.slots(0);
    assert_eq!(slots.len(), 1);
    let mut fragment = Builder::new(&extra);
    fragment.open(Role::Region);
    fragment.name(b"Application navigation");
    fragment.open(Role::Button);
    fragment.name(b"Home");
    fragment.close();
    fragment.close();
    b.splice(slots[0].id, fragment);
    let tree = b.finish();
    assert_eq!(tree.nodes().iter().filter(|node| node.role == Role::Heading { level: 1 }).count(), 1);
    println!(
        "fixture page nodes={largest}, composed nodes={}, depth={}, text/node={}, text bytes={}, bindings={}, slots={}, patches={}",
        limits.nodes,
        limits.depth,
        limits.text_per_node,
        limits.text_bytes,
        limits.bindings,
        limits.slots,
        limits.patches
    );
}

fn fill_tree(d: &mut Fixture, watch: jig_client::Handle) {
    let node = |number, parent| TreeNode {
        chip: full_chip(number),
        parent,
        dependencies: list([1, 2]),
        proposals: list([
            ProposalKey { proposer: Proposer::Task(8), number: 1 },
            ProposalKey { proposer: Proposer::Task(8), number: 2 },
        ]),
        spend: u64::MAX,
        folded: PhaseCounts { ended: 2, waiting: 2, running: 2, held: 2, closing: 2, done: 2, failed: 2, cancelled: 2 },
        held_below: 2,
    };
    deliver(
        d,
        watch,
        Delivery::Tree(Box::new(jig_client::contract::Tree {
            task: 8,
            nodes: list([node(9, None), node(10, Some(9))]),
            folded: PhaseCounts {
                ended: 2,
                waiting: 2,
                running: 2,
                held: 2,
                closing: 2,
                done: 2,
                failed: 2,
                cancelled: 2,
            },
            held_below: 2,
        })),
    );
}
fn fill_activity(d: &mut Fixture, read: jig_client::Handle) {
    drive(
        d,
        ReadPage {
            read,
            result: ReadResult::Value(ReadValue::Activity(Box::new(ActivityPage {
                cut: false,
                rows: list([
                    row(
                        2,
                        Activity::Turn {
                            name: TurnName { attempt: 1, number: 2 },
                            turn: TurnRead::Decoded(full_turn()),
                        },
                    ),
                    row(
                        1,
                        Activity::Turn {
                            name: TurnName { attempt: 1, number: 1 },
                            turn: TurnRead::Decoded(full_turn()),
                        },
                    ),
                ]),
                before: Some(full_cursor()),
                after: Some(full_cursor()),
            }))),
        },
    );
    for number in [1, 2] {
        for call in [1, 2] {
            drive(
                d,
                Event::Act {
                    action: Act::FoldCall {
                        name: jig_client::thread::CallName { attempt: 1, turn: number, call },
                        open: true,
                    },
                },
            );
        }
    }
}
fn fill_history(d: &mut Fixture, read: jig_client::Handle) {
    drive(
        d,
        ReadPage {
            read,
            result: ReadResult::Value(ReadValue::History(Box::new(HistoryPage {
                cut: false,
                rows: list([
                    HistoryRow::Line(Box::new(HistoryLine {
                        position: 2,
                        task: 8,
                        by: Actor::Party(1),
                        at: Wall::EPOCH,
                        change: HistoryChange::Amended { before: full(), after: full() },
                    })),
                    HistoryRow::Line(Box::new(HistoryLine {
                        position: 1,
                        task: 9,
                        by: Actor::Party(1),
                        at: Wall::EPOCH,
                        change: HistoryChange::Amended { before: full(), after: full() },
                    })),
                ]),
                before: Some(full_cursor()),
                after: Some(full_cursor()),
            }))),
        },
    );
}

fn fill_goals(d: &mut Fixture, watch: jig_client::Handle, _doc: &Project) {
    let goal = |number| TaskLine {
        chip: full_chip(number),
        tracked: Some(u32::MAX),
        phases: OwnerPhaseCounts {
            idle: 2,
            starting: 2,
            backing_off: 2,
            waiting: 2,
            running: 2,
            held: 2,
            closing: 2,
            done: 2,
            failed: 2,
            cancelled: 2,
        },
        budget: full_owner_budget(),
        held_below: 2,
        changed: Wall::EPOCH,
        last_activity: Wall::EPOCH,
        ended_at: None,
        project: 1,
        requester: OwnerRequester::Person(7),
        label: 2,
        flags: TaskFlags { ended_by: EndedBy::Result, conversation: false, listed: true, retention: Retention::Kept },
        waits_on: WaitsOn::Nothing,
        conversation: None,
    };
    deliver(d, watch, Delivery::Tasks(Box::new(Tasks { rows: list([goal(8), goal(9)]), more: true })));
}
fn fill_ended_goals(d: &mut Fixture, read: jig_client::Handle, _doc: &Project) {
    let goal = |number| TaskLine {
        chip: full_chip(number),
        tracked: Some(u32::MAX),
        phases: OwnerPhaseCounts {
            idle: 2,
            starting: 2,
            backing_off: 2,
            waiting: 2,
            running: 2,
            held: 2,
            closing: 2,
            done: 2,
            failed: 2,
            cancelled: 2,
        },
        budget: full_owner_budget(),
        held_below: 2,
        changed: Wall::EPOCH,
        last_activity: Wall::EPOCH,
        ended_at: None,
        project: 1,
        requester: OwnerRequester::Person(7),
        label: 2,
        flags: TaskFlags { ended_by: EndedBy::Result, conversation: false, listed: true, retention: Retention::Kept },
        waits_on: WaitsOn::Nothing,
        conversation: None,
    };
    drive(
        d,
        ReadPage {
            read,
            result: ReadResult::Value(ReadValue::Tasks(Box::new(TaskPage {
                live: List::with_capacity(0),
                live_after: None,
                ended: EndedGroup::Rows { rows: list([goal(10), goal(11)]), after: Some(full_cursor()), cut: false },
            }))),
        },
    );
}
fn fill_inbox(d: &mut Fixture, watch: jig_client::Handle) {
    let entry = |number| InboxEntry {
        object: ObjectKey::Task(number),
        group: InboxGroup::PersonTasks,
        project: 4,
        lineage: list([full_chip(1), full_chip(2)]),
        goal: Some(8),
        recipient: Some(Recipient::Role { project: 4, role: 1 }),
        stall: Some(Wall::from_nanos(u64::MAX)),
        reply: None,
        position: number,
        at: Wall::EPOCH,
    };
    deliver(
        d,
        watch,
        Delivery::Inbox(Box::new(Inbox {
            entries: list([entry(8), entry(9)]),
            needs: Needs { questions: u32::MAX, proposals: u32::MAX, escalations: u32::MAX, person_tasks: u32::MAX },
            updates: Count { value: u32::MAX, limited: true },
            more: true,
            results_seen: 0,
            replies_seen: list([ReadPosition { task: 8, through: 0 }, ReadPosition { task: 9, through: 0 }]),
        })),
    );
}
fn fill_tasks(d: &mut Fixture, watch: jig_client::Handle, _query: TaskQuery) {
    let row = |number| crate::tests::fixtures::task_line(full_chip(number), 4, OwnerRequester::Person(1), Wall::EPOCH);
    deliver(d, watch, Delivery::Tasks(Box::new(Tasks { rows: list([row(8), row(9)]), more: true })));
}
fn fill_notes(d: &mut Fixture, read: jig_client::Handle) {
    let note = |number| Note {
        number,
        revision: Revision { value: 7 },
        scope: NoteScope::Resource { project: 4, connector: 2, pattern: full_pattern() },
        description: DocumentText::complete(full()),
        body: DocumentText::complete(full()),
        writer: NoteAuthor::Party { party: 1 },
        tasks: list([8, 9]),
        changed: Some(Wall::EPOCH),
        acts: list([
            Offered { effect: None, act: ActKind::EditNote, admission: Admission::Allowed },
            Offered { effect: None, act: ActKind::DeleteNote, admission: Admission::Allowed },
        ]),
        cut: false,
    };
    drive(
        d,
        ReadPage {
            read,
            result: ReadResult::Value(ReadValue::Notes(Box::new(NotePage {
                cut: false,
                rows: list([note(1), note(2)]),
                before: Some(full_cursor()),
                after: Some(full_cursor()),
            }))),
        },
    );
}
fn fill_spend(d: &mut Fixture, read: jig_client::Handle, doc: &Project) {
    let period =
        SpendPeriod { period: u64::MAX, budget: doc.spend, pools: doc.pools.clone(), tasks: doc.spend_lines.clone() };
    drive(
        d,
        ReadPage {
            read,
            result: ReadResult::Value(ReadValue::Spend(Box::new(SpendPage {
                cut: false,
                rows: list([period.clone(), period]),
                before: Some(full_cursor()),
                after: Some(full_cursor()),
            }))),
        },
    );
}

fn specification(words: Text) -> SpecificationDocument {
    SpecificationDocument::complete(Specification {
        words,
        parameters: List::with_capacity(0),
        inputs: List::with_capacity(0),
    })
}

fn full_path() -> List<Box<[u8]>> {
    list([full().into_bytes(), full().into_bytes()])
}
fn full_pattern() -> OwnerPattern {
    OwnerPattern { segments: full_path(), last: OwnerPatternEnd::Exact(full().into_bytes()) }
}

fn full_cursor() -> Cursor {
    Cursor::new(full().into_bytes(), CLIENT.text).unwrap()
}

#[test]
fn each_page_bound_keeps_its_value_at_the_client_limits() {
    // Added-dependency words and provenance, circle chips and their fallbacks
    // enlarge the shared chip envelope and the standalone owner metadata.
    // Pin the resulting page totals so that later bound changes stay visible.
    const DEPLOYMENT_NODES: u32 = 10_937;
    assert_eq!(
        [
            crate::inbox_nodes(&CLIENT, 0),
            crate::conversations_nodes(&CLIENT, 0),
            crate::conversation_nodes(&CLIENT, 0),
            crate::task_nodes(&CLIENT, 0),
            crate::project_nodes(&CLIENT, 0),
            crate::deployment_nodes(&CLIENT, 0),
            crate::not_the_work_nodes(&CLIENT, 0),
            crate::page_nodes(&CLIENT, 0)
        ],
        [
            Some(414_443),
            Some(104_718),
            Some(1_239_864),
            Some(1_431_991),
            Some(621_789),
            Some(DEPLOYMENT_NODES),
            Some(25),
            Some(1_431_991)
        ]
    );
}

#[test]
fn an_arranged_task_page_counts_the_sum_of_its_sections() {
    let sections = [
        TaskSection::Overview,
        TaskSection::Activity,
        TaskSection::Plan,
        TaskSection::History,
        TaskSection::Budget,
        TaskSection::Panels,
    ];
    let counts = sections.map(|s| crate::task_section_nodes(&CLIENT, s).unwrap());
    assert_eq!(
        (crate::task_header_nodes(&CLIENT).unwrap(), counts),
        (52_042, [224_829, 1_379_949, 384_148, 17_224, 71, 94])
    );
    let arrangement = crate::page::Arrangement { task: list(sections), project: list([]) };
    assert_eq!(
        crate::arranged_nodes(&CLIENT, 0, &arrangement),
        Some(52_042 + 224_829 + 1_379_949 + 384_148 + 17_224 + 71 + 94)
    );
}
#[test]
fn no_arrangement_keeps_each_page_bound() {
    each_page_bound_keeps_its_value_at_the_client_limits();
    let arrangement = crate::page::Arrangement::none();
    assert_eq!(crate::arranged_nodes(&CLIENT, 0, &arrangement), Some(1_431_991));
    assert_eq!(crate::arranged_nodes(&CLIENT, 9, &arrangement), Some(1_432_000));
}

fn most_owed_cards() -> Fixture {
    let (mut d, watch) = start();
    deliver(&mut d, watch, Delivery::Me(Box::new(document())));
    let objects = list((1..=CLIENT.objects).map(|n| ObjectKey::Task(u64::from(n))));
    let out = drive(&mut d, Event::Named { objects, sections: List::with_capacity(0) });
    for request in out {
        if let Request::Client {
            asked: jig_client::Event::Read { handle: read, read: jig_client::Readable::Jig(Read::Task { task }) },
        } = request
        {
            let mut doc = super::cards_objects::task_doc(&[ActKind::Cancel]);
            doc.chip.task = task;
            drive(&mut d, ReadPage { read, result: ReadResult::Value(ReadValue::Task(Box::new(doc))) });
        }
    }
    for number in 1..=CLIENT.objects {
        let id = d.objects().find(ObjectKey::Task(u64::from(number))).unwrap();
        drive(&mut d, Event::Act { action: Act::Intend { object: id.token(), act: ActKind::Cancel } });
        drive(
            &mut d,
            Event::Act {
                action: Act::Edit { field: crate::fields::Purpose::Reason, text: super::cards::text(b"cancel") },
            },
        );
        if number < CLIENT.objects {
            drive(&mut d, Event::Act { action: Act::Confirm });
        }
    }
    drive(&mut d, Event::Named { objects: List::with_capacity(0), sections: List::with_capacity(0) });
    d
}
#[test]
fn the_most_owed_cards_fit_the_pages_bound() {
    let d = most_owed_cards();
    for (_, id) in d.objects().ids() {
        assert!(d.owes(*id));
    }
    assert_eq!(d.objects().len(), CLIENT.objects);
    let limits = crate::limits::of(&CLIENT, &crate::page::Arrangement::none()).unwrap();
    let mut b = Builder::new(&limits);
    let mut bindings = JigBindings::new(limits.bindings, CLIENT.text);
    b.open(Role::Page);
    crate::pages::after_page(&mut b, &d.view(), words(), &mut bindings);
    b.close();
    assert!(b.finish().len() <= limits.nodes);
}

mod project;
use project::project_pages;
