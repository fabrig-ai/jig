//! Page policies exercised through the client boundary.
use jig_client::thread::window::End;
mod creating;
mod owner_effect_proposals;
mod task_proposals;
use super::spine::{LIMITS, delivered, drive, frame, open_token, start};
use crate::tests::fixtures::refusal_answer;
use crate::tests::fixtures::{AnsweredByEngine, Ended, Fixture, ReadPage, Served, Watched};
use crate::{
    Event, Request,
    address::{Address, Place, ProjectSection, TaskSection},
    page::{
        Arrangement, Page, Section,
        navigation::{Destination, Navigation},
    },
};
use jig_client::Link;
use jig_client::contract::*;
use skein_lib::{List, Time};
fn text(value: &[u8]) -> Text {
    Text::new(Box::from(value), LIMITS.text).unwrap()
}
pub(super) fn project(number: u32, chats: bool) -> MeProject {
    MeProject {
        models: List::with_capacity(0),
        templates: if chats { crate::tests::fixtures::chat_templates() } else { List::with_capacity(0) },
        creating: Some(crate::tests::fixtures::creating()),
        project: number,
        name: DocumentText::complete(text(b"shop")),
        role: HeldRole::Policy { role: 1 },
        pool: Some(PoolLedger {
            source: Funder::Pool { project: number, person: 7, period: 1 },
            parent: None,
            numbers: OwnerNumbers { budget: 20, spent: 0, spent_below: 0, reserved: 0 },
            left: 20,
            closed: false,
            began: None,
            next: None,
        }),
    }
}
fn budget() -> Budget {
    Budget { total: 20, own: 0, ended: 0, reserved: 0, left: 20, funder: Actor::Party(7) }
}
fn the_frame_counts_needs_exactly_and_caps_updates_on_every_page() {
    let mut domain = Fixture::new(&LIMITS);
    let f = start(&mut domain, List::with_capacity(0));
    let mut value = frame(7);
    value.needs = Needs { questions: u32::MAX, proposals: 2, escalations: 3, person_tasks: 4 };
    value.needs_total = u32::MAX;
    value.updates = Count { value: 100, limited: false };
    delivered(&mut domain, Time::ZERO, f, value.clone());
    let summary = domain.frame_summary();
    assert_eq!(summary.needs, u64::from(u32::MAX) + 9);
    assert_eq!(summary.updates, Count { value: 99, limited: true });
    assert_eq!(summary.party, Some(7));
    assert_eq!(summary.link, Link::Live);
    drive(&mut domain, Time::ZERO, Event::Went { place: Place::Elsewhere });
    assert_eq!(domain.frame_summary().needs, summary.needs);
    value.updates = Count { value: 99, limited: false };
    delivered(&mut domain, Time::ZERO, f, value.clone());
    assert_eq!(domain.frame_summary().updates, Count { value: 99, limited: false });
    value.updates.limited = true;
    delivered(&mut domain, Time::ZERO, f, value);
    assert!(domain.frame_summary().updates.limited);
    drive(
        &mut domain,
        Time::ZERO,
        Event::Went { place: Place::Jig(Address::Project { project: 1, section: ProjectSection::Overview }) },
    );
    assert_eq!(domain.frame_summary().project, Some(1));
    assert_eq!(Address::task(42), Address::Task { task: 42, section: TaskSection::Overview });
}
fn navigation_keeps_the_application_order_and_leaves_out_inapplicable_pages() {
    let mut domain = Fixture::new(&LIMITS);
    let mut navigation = Navigation::new(8);
    assert_eq!(navigation.home, Destination::Jig(Address::Inbox { project: None }));
    let own = Destination::Application { owner: 1, number: 3 };
    let chat = Destination::Jig(Address::Conversations { project: None });
    let deployment = Destination::Jig(Address::Deployment);
    navigation.home = own;
    for destination in [own, chat, deployment] {
        navigation.entries.push(destination).unwrap();
    }
    domain.configure_navigation(navigation).unwrap();
    let f = start(&mut domain, List::with_capacity(0));
    let mut value = frame(7);
    value.deployment_role = None;
    delivered(&mut domain, Time::ZERO, f, value.clone());
    assert_eq!(domain.navigation().entries.as_slice(), &[own, chat, deployment]);
    assert!(domain.navigation().offered(own, domain.frame().document));
    assert!(!domain.navigation().offered(chat, domain.frame().document));
    assert!(!domain.navigation().offered(deployment, domain.frame().document));
    value.projects = List::with_capacity(2);
    value.projects.push(project(1, false)).unwrap();
    value.projects.push(project(2, true)).unwrap();
    value.deployment_role = Some(HeldRole::Policy { role: 1 });
    delivered(&mut domain, Time::ZERO, f, value);
    assert!(domain.navigation().offered(chat, domain.frame().document));
    assert!(
        !domain
            .navigation()
            .offered(Destination::Jig(Address::Conversations { project: Some(1) }), domain.frame().document)
    );
    assert!(domain.navigation().offered(deployment, domain.frame().document));
    assert!(domain.configure_navigation(Navigation::new(0)).is_err());
    let mut fresh = Fixture::new(&LIMITS);
    assert!(fresh.configure_navigation(Navigation::new(9)).is_err());
    assert_eq!(own.place(), Place::Elsewhere);
}
fn arrangement(task: &[TaskSection], project: &[ProjectSection]) -> Arrangement {
    let mut value = Arrangement { task: List::with_capacity(7), project: List::with_capacity(11) };
    for section in task {
        value.task.push(*section).unwrap();
    }
    for section in project {
        value.project.push(*section).unwrap();
    }
    value
}
#[test]
fn an_arrangement_given_before_the_start_is_kept() {
    let mut domain = Fixture::new(&LIMITS);
    assert_eq!(domain.arrangement(), &Arrangement::none());
    assert_eq!(domain.arrangement().task.capacity(), 0);
    assert_eq!(domain.arrangement().project.capacity(), 0);
    let value = arrangement(&[TaskSection::Plan, TaskSection::History], &[ProjectSection::Goals]);
    assert_eq!(domain.configure_arrangement(value.clone()), Ok(()));
    start(&mut domain, List::with_capacity(0));
    assert_eq!(domain.arrangement(), &value);
}
#[test]
fn each_malformed_arrangement_is_refused() {
    let rows = [
        ("seven task sections", arrangement(&[TaskSection::Overview; 7], &[])),
        ("eleven project sections", arrangement(&[], &[ProjectSection::Overview; 11])),
        ("Plan twice", arrangement(&[TaskSection::Plan; 2], &[])),
        ("Goals twice", arrangement(&[], &[ProjectSection::Goals; 2])),
        ("Settings with Resources", arrangement(&[], &[ProjectSection::Settings, ProjectSection::Resources])),
        ("Settings with Policy", arrangement(&[], &[ProjectSection::Settings, ProjectSection::Policy])),
        ("Settings with Roles", arrangement(&[], &[ProjectSection::Settings, ProjectSection::Roles])),
        ("Settings with Spend", arrangement(&[], &[ProjectSection::Settings, ProjectSection::Spend])),
        ("Settings with Recurring", arrangement(&[], &[ProjectSection::Settings, ProjectSection::Recurring])),
    ];
    let mut domain = Fixture::new(&LIMITS);
    for (label, value) in rows {
        assert_eq!(domain.configure_arrangement(value.clone()), Err(value), "{label}");
        assert_eq!(domain.arrangement(), &Arrangement::none(), "{label}");
    }
    start(&mut domain, List::with_capacity(0));
    let value = arrangement(&[TaskSection::Plan], &[]);
    assert_eq!(domain.configure_arrangement(value.clone()), Err(value), "after the start");
}
fn non_work_pages_follow_their_causes_and_missing_tasks_do_not_reveal_visibility() {
    let mut domain = Fixture::new(&LIMITS);
    let f = start(&mut domain, List::with_capacity(0));
    assert_eq!(domain.page(), &Page::Starting);
    delivered(&mut domain, Time::ZERO, f, frame(7));
    drive(&mut domain, Time::ZERO, Event::Went { place: Place::Missing });
    assert_eq!(domain.page(), &Page::Missing);
    drive(&mut domain, Time::ZERO, jig_client::Up::SignedOut);
    assert_eq!(domain.page(), &Page::SigningIn);
    for end in [WatchEnd::Unknown, WatchEnd::NotAllowed] {
        let mut domain = Fixture::new(&LIMITS);
        let f = start(&mut domain, List::with_capacity(0));
        delivered(&mut domain, Time::ZERO, f, frame(7));
        let opened = drive(&mut domain, Time::ZERO, Event::Went { place: Place::Jig(Address::task(42)) });
        let output = drive(&mut domain, Time::ZERO, Ended { watch: open_token(&opened), end });
        if end == WatchEnd::Unknown {
            let mut read = None;
            for request in &output {
                if let Request::Client {
                    asked:
                        jig_client::Event::Read { handle: token, read: jig_client::Readable::Jig(Read::Task { task: 42 }) },
                } = request
                {
                    assert!(read.replace(*token).is_none());
                }
            }
            let read = read.expect("an absent live task is checked in retained work");
            let Page::Work { address, .. } = domain.page() else {
                panic!("task page awaits its retained read");
            };
            assert_eq!(*address, Address::task(42));
            drive(&mut domain, Time::ZERO, ReadPage { read, result: ReadResult::None(ReadNone::Unknown) });
        } else {
            for request in &output {
                if let Request::Client { asked: jig_client::Event::Read { .. } } = request {
                    panic!("a forbidden task must not probe retained work");
                }
            }
        }
        assert_eq!(domain.page(), &Page::Missing);
    }
    let mut old = Fixture::new(&LIMITS);
    let f = start(&mut old, List::with_capacity(0));
    drive(&mut old, Time::ZERO, Served { served: false });
    assert!(delivered(&mut old, Time::ZERO, f, frame(7)).is_empty());
    assert_eq!(old.page(), &Page::OutOfDate);
    let mut other = Fixture::new(&LIMITS);
    let mut kept = List::with_capacity(1);
    kept.push(crate::store::Record::Draft {
        version: crate::store::STORE_VERSION,
        party: 8,
        task: 42,
        words: text(b"secret"),
        touched: 1,
    })
    .unwrap();
    let f = start(&mut other, kept);
    delivered(&mut other, Time::ZERO, f, frame(7));
    assert_eq!(other.page(), &Page::SomeoneElse);
}
fn deployment_without_a_role_is_missing_and_never_watched() {
    let mut domain = Fixture::new(&LIMITS);
    let f = start(&mut domain, List::with_capacity(0));
    let mut value = frame(7);
    value.deployment_role = None;
    delivered(&mut domain, Time::ZERO, f, value);
    let out = drive(&mut domain, Time::ZERO, Event::Went { place: Place::Jig(Address::Deployment) });
    assert_eq!(domain.page(), &Page::Missing);
    for request in &out {
        if let Request::Client {
            asked: jig_client::Event::Watch { subject: jig_client::Watchable::Jig(subject), .. },
        } = request
        {
            assert_ne!(*subject, Subject::Deployment);
        }
    }
}

fn entry(task: u64, group: InboxGroup, at: u64) -> InboxEntry {
    InboxEntry {
        reply: None,
        object: ObjectKey::Task(task),
        project: 1,
        group,
        position: task,
        at: skein_lib::Wall::from_nanos(at),
        lineage: List::with_capacity(0),
        goal: Some(9),
        recipient: Some(Recipient::Role { project: 1, role: 2 }),
        stall: Some(skein_lib::Wall::from_nanos(100)),
    }
}
fn inbox(entries: List<InboxEntry>) -> Inbox {
    Inbox {
        entries,
        needs: Needs { questions: 0, proposals: 2, escalations: 0, person_tasks: 0 },
        updates: Count { value: 100, limited: true },
        results_seen: 0,
        replies_seen: List::with_capacity(0),
        more: false,
    }
}
pub(super) fn finish_initial_inbox_reads(domain: &mut Fixture, out: &[Request]) {
    for request in out {
        if let Request::Client {
            asked: jig_client::Event::Read { handle: read, read: jig_client::Readable::Jig(Read::Inbox { .. }) },
        } = request
        {
            drive(
                domain,
                Time::ZERO,
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
    domain.reclaim();
}
fn live_inbox() -> (Fixture, jig_client::Handle) {
    let mut domain = Fixture::new(&LIMITS);
    let f = start(&mut domain, List::with_capacity(0));
    let out = delivered(&mut domain, Time::ZERO, f, frame(7));
    let page = open_token(&out);
    finish_initial_inbox_reads(&mut domain, &out);
    (domain, page)
}
fn inbox_delivery(domain: &mut Fixture, page: jig_client::Handle, entries: List<InboxEntry>) -> Box<[Request]> {
    drive(
        domain,
        Time::ZERO,
        Watched { watch: page, event: WatchEvent::Delivered(Delivery::Inbox(Box::new(inbox(entries)))) },
    )
}
fn list_entry(task: u64, phase: Phase, changed: u64) -> TaskLine {
    TaskLine {
        chip: Chip {
            task,
            title: DocumentText::complete(text(b"held checkout")),
            executor: Executor::Agent { charter: 1 },
            phase,
            details: Box::new(super::objects::phase_details(phase)),
        },
        project: 1,
        requester: OwnerRequester::Person(1),
        tracked: None,
        label: 1,
        flags: TaskFlags {
            ended_by: EndedBy::Result,
            conversation: true,
            listed: false,
            retention: Retention::ForgottenOnceRead,
        },
        changed: skein_lib::Wall::from_nanos(changed),
        last_activity: skein_lib::Wall::EPOCH,
        ended_at: None,
        budget: OwnerBudget {
            numbers: OwnerNumbers { budget: 20, spent: 5, spent_below: 1, reserved: 2 },
            left: 12,
            funder: Funder::Pool { project: 1, person: 7, period: 3 },
            deadline: None,
        },
        waits_on: WaitsOn::Nothing,
        phases: OwnerPhaseCounts {
            waiting: 0,
            idle: 0,
            starting: 0,
            running: 0,
            backing_off: 0,
            closing: 0,
            held: 0,
            done: 0,
            failed: 0,
            cancelled: 0,
        },
        held_below: 0,
        conversation: Some(ConversationRead { seen: 2, unread: Count { value: 1, limited: false } }),
    }
}
fn inbox_orders_both_groups_filters_context_and_removes_role_entries_for_every_holder() {
    for group in [InboxGroup::Proposals, InboxGroup::Results] {
        let (mut domain, page) = live_inbox();
        let mut rows = List::with_capacity(2);
        rows.push(entry(8, group, 20)).unwrap();
        rows.push(entry(7, group, 10)).unwrap();
        inbox_delivery(&mut domain, page, rows);
        let first = if crate::page::inbox::needs(group) { 7 } else { 8 };
        assert_eq!(domain.inbox().rows.get(0).unwrap().card_key(), ObjectKey::Task(first));
        let projected = domain.inbox();
        let InboxRow::Entry(row) = projected.rows.get(0).unwrap() else {
            panic!("supplied Inbox row");
        };
        assert_eq!(row.recipient, Some(Recipient::Role { project: 1, role: 2 }));
        assert_eq!(domain.inbox().updates, Count { value: 100, limited: true });
        assert_eq!(crate::page::inbox::address(ObjectKey::Task(first)), Some(Address::task(first)));
        for filter in [
            crate::page::inbox::Filter { project: Some(2), kind: None, goal: None },
            crate::page::inbox::Filter { project: None, kind: Some(InboxGroup::Questions), goal: None },
            crate::page::inbox::Filter { project: None, kind: None, goal: Some(10) },
        ] {
            let out = drive(&mut domain, Time::ZERO, Event::Act { action: crate::action::Act::FilterInbox { filter } });
            assert!(domain.inbox().rows.is_empty());
            let mut groups = List::with_capacity(2);
            for request in &out {
                if let Request::Client {
                    asked:
                        jig_client::Event::Read {
                            read: jig_client::Readable::Jig(Read::Inbox { group, project, kind, goal, after }),
                            ..
                        },
                } = request
                {
                    assert_eq!(*project, filter.project);
                    assert_eq!(*kind, filter.kind);
                    assert_eq!(*goal, filter.goal);
                    assert!(after.is_none());
                    groups.push(*group).unwrap();
                }
            }
            assert_eq!(groups.len(), if filter.kind.is_some() { 1 } else { 2 });
            assert_eq!(*groups.get(0).unwrap(), InboxReadGroup::Needs);
            if filter.kind.is_none() {
                assert_eq!(*groups.get(1).unwrap(), InboxReadGroup::Updates);
            }
            finish_initial_inbox_reads(&mut domain, &out);
        }
    }
    for party in [7, 8] {
        let mut domain = Fixture::new(&LIMITS);
        let f = start(&mut domain, List::with_capacity(0));
        let page = open_token(&delivered(&mut domain, Time::ZERO, f, frame(party)));
        let mut rows = List::with_capacity(1);
        rows.push(entry(7, InboxGroup::Proposals, 10)).unwrap();
        inbox_delivery(&mut domain, page, rows);
        inbox_delivery(&mut domain, page, List::with_capacity(0));
        assert!(domain.inbox().rows.is_empty());
    }
}
fn opening_updates_marks_the_newest_result_displayed_as_seen() {
    let (mut domain, page) = live_inbox();
    drive(&mut domain, Time::ZERO, Event::Act { action: crate::action::Act::OpenUpdates });
    let mut rows = List::with_capacity(2);
    rows.push(entry(7, InboxGroup::Results, 10)).unwrap();
    rows.push(entry(8, InboxGroup::Results, 20)).unwrap();
    let out = inbox_delivery(&mut domain, page, rows.clone());
    let mut count = 0_u32;
    for request in &out {
        if let Request::Client { asked: jig_client::Event::Send { request: jig_client::Ask::Jig(ask), .. } } = request {
            assert_eq!(**ask, Ask::Seen { through: SeenThrough::Results { through: 8 } });
            count = count.checked_add(1).expect("bounded emitted requests");
        }
    }
    assert_eq!(count, 1);
    let out = inbox_delivery(&mut domain, page, rows);
    for request in &out {
        if let Request::Client { asked: jig_client::Event::Send { .. } } = request {
            panic!("passive delivery does not mark read again");
        }
    }
    assert!(drive(&mut domain, Time::ZERO, Event::Act { action: crate::action::Act::OpenUpdates }).is_empty());
}

fn starting_a_chat_joins_refuses_and_opens_only_after_its_durable_answer() {
    let mut about = Fixture::new(&LIMITS);
    let f = start(&mut about, List::with_capacity(0));
    let mut value = frame(7);
    value.projects = List::with_capacity(1);
    value.projects.push(project(1, true)).unwrap();
    let page = open_token(&delivered(&mut about, Time::ZERO, f, value));
    let mut rows = List::with_capacity(1);
    rows.push(entry(8, InboxGroup::Escalations, 10)).unwrap();
    inbox_delivery(&mut about, page, rows);
    assert!(about.objects.put(ObjectKey::Task(8), crate::objects::Body::Chip(list_entry(8, Phase::Held, 10).chip)));
    let mut out = skein_lib::Queue::with_capacity(crate::max_out(&LIMITS));
    crate::page::conversations::about(
        &mut about.ui,
        &super::spine::env(Time::ZERO),
        ObjectKey::Task(8),
        &mut out,
        &about.client,
    );
    let requests = super::spine::drain(out);
    assert_eq!(requests.len(), 1);
    let Request::Client { asked: jig_client::Event::Send { request: jig_client::Ask::Jig(ask), .. } } = &requests[0]
    else {
        panic!("chat about the held task");
    };
    assert_eq!(**ask, crate::tests::fixtures::chat_request(1, text(b"held checkout")));
    let mut domain = Fixture::new(&LIMITS);
    let f = start(&mut domain, List::with_capacity(0));
    let mut value = frame(7);
    value.projects = List::with_capacity(1);
    value.projects.push(project(1, true)).unwrap();
    delivered(&mut domain, Time::ZERO, f, value);
    drive(
        &mut domain,
        Time::ZERO,
        Event::Act { action: crate::action::Act::Edit { field: crate::fields::Purpose::NewChat, text: text(b"help") } },
    );
    for refused in [true, false] {
        let out =
            drive(&mut domain, Time::ZERO, Event::Act { action: crate::action::Act::BeginConversation { project: 1 } });
        let mut token = None;
        for request in &out {
            if let Request::Client {
                asked: jig_client::Event::Send { key: sent, request: jig_client::Ask::Jig(ask), .. },
            } = request
            {
                assert_eq!(**ask, crate::tests::fixtures::chat_request(1, text(b"help")));
                token = Some(*sent);
            }
        }
        assert!(
            drive(&mut domain, Time::ZERO, Event::Act { action: crate::action::Act::BeginConversation { project: 1 } })
                .is_empty()
        );
        assert_eq!(
            domain.page(),
            &Page::Work { address: Address::Inbox { project: None }, behind: false, excess: 0, refused: None }
        );
        let answer = if refused {
            refusal_answer(Refusal::Authority { lacking: Lacking::Budget { needed: 10, left: 2 }, proposable: None })
        } else {
            Answer::Done(Outcome::Made { what: Made::Tasks { tasks: crate::tests::fixtures::one(42) } })
        };
        drive(&mut domain, Time::ZERO, AnsweredByEngine { request: token.unwrap(), answer });
        if refused {
            assert!(domain.chats().refusal.is_some());
        } else {
            let Page::Work { address, .. } = domain.page() else {
                panic!("chat opened");
            };
            assert_eq!(*address, Address::Task { task: 42, section: TaskSection::Activity });
            assert!(domain.fields().get(crate::fields::Purpose::NewChat).unwrap().text.bytes().is_empty());
        }
        domain.reclaim();
    }
}

fn task_sections_and_activity_sources_follow_the_executor() {
    use crate::{address::TaskSection, page::task};
    for executor in [
        Executor::Agent { charter: 1 },
        Executor::Procedure { connector: 2, kind: 3 },
        Executor::Party { party: 7 },
        Executor::Role { project: 1, role: 1 },
    ] {
        let mut document = super::confirmation::task();
        document.chip.executor = executor;
        let sections = task::offers(&document);
        for section in [TaskSection::Overview, TaskSection::Activity, TaskSection::History, TaskSection::Budget] {
            assert!(task::offered(&sections, section));
        }
        assert!(!task::offered(&sections, TaskSection::Plan));
        let mut domain = Fixture::new(&LIMITS);
        let f = start(&mut domain, List::with_capacity(0));
        let initial = delivered(&mut domain, Time::ZERO, f, frame(7));
        finish_initial_inbox_reads(&mut domain, &initial);
        let opened = drive(
            &mut domain,
            Time::ZERO,
            Event::Went { place: Place::Jig(Address::Task { task: 8, section: TaskSection::Activity }) },
        );
        let watch = open_token(&opened);
        let requests = drive(
            &mut domain,
            Time::ZERO,
            Watched { watch, event: WatchEvent::Delivered(Delivery::Task(Box::new(document))) },
        );
        assert!(domain.task_page().header.is_some());
        match executor {
            Executor::Agent { .. } => {
                assert_eq!(requests.len(), 2);
                let Request::Client {
                    asked:
                        jig_client::Event::Watch {
                            subject: jig_client::Watchable::Jig(Subject::Run { task: 8, turn: None, offset: 0 }),
                            ..
                        },
                } = requests[0]
                else {
                    panic!("agent run watch");
                };
                let Request::Client {
                    asked:
                        jig_client::Event::Read {
                            read: jig_client::Readable::Jig(Read::Activity { task: 8, before: None }),
                            ..
                        },
                } = requests[1]
                else {
                    panic!("agent activity read");
                };
            }
            Executor::Procedure { connector, .. } => {
                assert_eq!(
                    requests.as_ref(),
                    &[Request::ShowPart { part: Part::Panel { connector, task: 8 }, scope: PartScope::Project(1) }]
                );
            }
            Executor::Party { .. } | Executor::Role { .. } => {
                let Request::Client {
                    asked: jig_client::Event::Read { read: jig_client::Readable::Jig(Read::PersonTask { task: 8 }), .. },
                } = requests[0]
                else {
                    panic!("person task card read");
                };
                assert_eq!(requests.len(), 1);
            }
        }
    }
}
fn tree_node(number: u64, phase: Phase, held: u32) -> TreeNode {
    let mut chip = super::objects::chip(number);
    chip.phase = phase;
    *chip.details = super::objects::phase_details(phase);
    TreeNode {
        chip,
        parent: Some(8),
        dependencies: List::with_capacity(0),
        proposals: List::with_capacity(0),
        spend: 0,
        folded: empty_phase_counts(),
        held_below: held,
    }
}

fn task_history_pages_and_scope_changes_discard_old_scope_reads() {
    use crate::{action::Act, address::TaskSection, page::task::HISTORY_WINDOW};
    let mut domain = Fixture::new(&LIMITS);
    let f = start(&mut domain, List::with_capacity(0));
    let initial = delivered(&mut domain, Time::ZERO, f, frame(7));
    finish_initial_inbox_reads(&mut domain, &initial);
    let opened = drive(
        &mut domain,
        Time::ZERO,
        Event::Went { place: Place::Jig(Address::Task { task: 8, section: TaskSection::History }) },
    );
    let Request::Client { asked: jig_client::Event::Read { handle: own, .. } } = opened[3] else {
        panic!("own history read");
    };
    let requests = drive(&mut domain, Time::ZERO, Event::Act { action: Act::HistoryScope { subtree: true } });
    let Request::Client {
        asked:
            jig_client::Event::Read { handle: read, read: jig_client::Readable::Jig(Read::History { subtree: true, .. }) },
    } = requests[0]
    else {
        panic!("subtree read");
    };
    let cursor = Cursor::new(Box::from(b"opaque history edge".as_slice()), 1024).expect("bounded opaque cursor");
    let mut rows = List::with_capacity(1);
    rows.push(HistoryRow::Line(Box::new(HistoryLine {
        position: 1,
        task: 9,
        by: Actor::Party(7),
        at: skein_lib::Wall::EPOCH,
        change: HistoryChange::Released,
    })))
    .unwrap();
    let page = HistoryPage { cut: false, rows, before: None, after: Some(cursor.clone()) };
    drive(
        &mut domain,
        Time::ZERO,
        ReadPage { read: own, result: ReadResult::Value(ReadValue::History(Box::new(page.clone()))) },
    );
    assert!(domain.history_rows().rows.is_empty());
    drive(&mut domain, Time::ZERO, ReadPage { read, result: ReadResult::Value(ReadValue::History(Box::new(page))) });
    assert_eq!(domain.history_rows().rows.len(), 1);
    let more = drive(
        &mut domain,
        Time::ZERO,
        Event::Act { action: Act::More { window: skein_lib::Token::new(HISTORY_WINDOW), end: End::After } },
    );
    let Request::Client { asked: jig_client::Event::Read { read: jig_client::Readable::Jig(what), .. } } = &more[0]
    else {
        panic!("next history page");
    };
    assert_eq!(*what, Read::History { task: 8, subtree: true, after: Some(cursor) });
}
fn task_parts_follow_the_section_and_leave_with_the_page_generation() {
    use crate::address::TaskSection;
    let mut domain = Fixture::new(&LIMITS);
    let f = start(&mut domain, List::with_capacity(0));
    let initial = delivered(&mut domain, Time::ZERO, f, frame(7));
    finish_initial_inbox_reads(&mut domain, &initial);
    let opened = drive(
        &mut domain,
        Time::ZERO,
        Event::Went { place: Place::Jig(Address::Task { task: 8, section: TaskSection::Overview }) },
    );
    let watch = open_token(&opened);
    let mut document = super::confirmation::task();
    document.resources = List::with_capacity(1);
    let resource = Part::Resource { connector: 2, path: resource_path(b"resource") };
    document.resources.push(TaskResource::Part(resource.clone())).unwrap();
    let context = document.context.as_mut().expect("supplied task context");
    context.parts = List::with_capacity(1);
    context.parts.push(Part::Panel { connector: 3, task: 8 }).unwrap();
    let requests = drive(
        &mut domain,
        Time::ZERO,
        Watched { watch, event: WatchEvent::Delivered(Delivery::Task(Box::new(document))) },
    );
    assert_eq!(requests.as_ref(), &[Request::ShowPart { part: resource, scope: PartScope::Project(1) }]);
    let left = drive(&mut domain, Time::ZERO, Event::Went { place: Place::Elsewhere });
    assert!(left.contains(&Request::PartsLeft));
    assert!(domain.task_page().header.is_none());
}

fn empty_phase_counts() -> PhaseCounts {
    PhaseCounts { ended: 0, waiting: 0, running: 0, held: 0, closing: 0, done: 0, failed: 0, cancelled: 0 }
}

fn project_document() -> Project {
    let mut acts = List::with_capacity(1);
    let mut charters = List::with_capacity(1);
    charters.push(1).unwrap();
    acts.push(Offered {
        effect: None,
        act: ActKind::Reorder,
        admission: Admission::Bounded {
            within: Bounds::Create(CreateBounds {
                authority: Box::new(crate::tests::cards_objects::owner_authority()),
                pool_left: 20,
                procedures: List::with_capacity(0),
                charters,
                latest: None,
                budget: 20,
                maximum_budget: 100,
                maximum_deadline: None,
            }),
            beyond: Beyond::Proposed { to: Recipient::Party(9) },
        },
    })
    .unwrap();
    Project {
        number: 1,
        period: 1,
        spend_lines: List::with_capacity(0),
        name: text(b"shop"),
        policy: Policy {
            ceiling: super::objects::authority(),
            period: 1,
            spend: 0,
            roles: List::with_capacity(0),
            requirements: List::with_capacity(0),
            deciders: List::with_capacity(0),
        },
        roles: List::with_capacity(0),
        spend: budget(),
        pools: List::with_capacity(0),
        resources: List::with_capacity(0),
        services: List::with_capacity(0),
        recurring: List::with_capacity(0),
        parts: List::with_capacity(0),
        acts,
    }
}
fn project_open(section: ProjectSection) -> (Fixture, Box<[Request]>) {
    let mut domain = Fixture::new(&LIMITS);
    let f = start(&mut domain, List::with_capacity(0));
    let mut value = frame(7);
    let role = project(1, true);
    value.projects = List::with_capacity(1);
    value.projects.push(role).unwrap();
    let out = delivered(&mut domain, Time::ZERO, f, value);
    finish_initial_inbox_reads(&mut domain, &out);
    let opened =
        drive(&mut domain, Time::ZERO, Event::Went { place: Place::Jig(Address::Project { project: 1, section }) });
    (domain, opened)
}
fn project_read(requests: &[Request]) -> jig_client::Handle {
    for request in requests {
        if let Request::Client {
            asked:
                jig_client::Event::Read { handle: read, read: jig_client::Readable::Jig(Read::Project { project: 1 }) },
        } = request
        {
            return *read;
        }
    }
    panic!("project read")
}
fn goal_line(number: u64, priority: u32, phase: Phase, changed: u64) -> TaskLine {
    let mut chip = super::objects::chip(number);
    chip.phase = phase;
    *chip.details = super::objects::phase_details(phase);
    TaskLine {
        chip,
        tracked: Some(priority),
        phases: OwnerPhaseCounts::ZERO,
        budget: OwnerBudget {
            numbers: OwnerNumbers { budget: 20, spent: 0, spent_below: 0, reserved: 0 },
            left: 20,
            funder: Funder::Pool { project: 1, person: 7, period: 3 },
            deadline: None,
        },
        held_below: 2,
        changed: skein_lib::Wall::from_nanos(changed),
        last_activity: skein_lib::Wall::from_nanos(changed),
        ended_at: None,
        project: 1,
        requester: OwnerRequester::Person(7),
        label: 2,
        flags: TaskFlags { ended_by: EndedBy::Result, conversation: false, listed: true, retention: Retention::Kept },
        waits_on: WaitsOn::Nothing,
        conversation: None,
    }
}
fn project_board_orders_live_waiting_and_ended_goals_and_names_task_cards() {
    use crate::{action::Act, page::project::GOALS_WINDOW};
    let (mut domain, opened) = project_open(ProjectSection::Goals);
    let project_read = project_read(&opened);
    drive(
        &mut domain,
        Time::ZERO,
        ReadPage { read: project_read, result: ReadResult::Value(ReadValue::Project(Box::new(project_document()))) },
    );
    let watch = open_token(&opened);
    let mut rows = List::with_capacity(2);
    rows.push(goal_line(8, 1, Phase::Running, 2)).unwrap();
    rows.push(goal_line(9, 20, Phase::Waiting, 1)).unwrap();
    let facts = drive(
        &mut domain,
        Time::ZERO,
        Watched { watch, event: WatchEvent::Delivered(Delivery::Tasks(Box::new(Tasks { rows, more: true }))) },
    );
    assert_eq!(domain.project_page().goals.as_slice()[0].chip.task, 8);
    assert_eq!(domain.project_page().goals.as_slice()[1].chip.task, 9);
    assert!(domain.project_page().goals_more);
    assert!(domain.objects().find(ObjectKey::Task(8)).is_some());
    let Request::Client {
        asked: jig_client::Event::Read { handle: read, read: jig_client::Readable::Jig(Read::Task { task: 8 }) },
    } = facts[0]
    else {
        panic!("goal task facts read");
    };
    let mut task = super::confirmation::task();
    task.goal = Some(task.chip.task);
    task.priority = Some(1);
    task.acts = List::with_capacity(1);
    task.acts.push(Offered { effect: None, act: ActKind::Reorder, admission: Admission::Allowed }).unwrap();
    drive(&mut domain, Time::ZERO, ReadPage { read, result: ReadResult::Value(ReadValue::Task(Box::new(task))) });
    let id = domain.objects().find(ObjectKey::Task(8)).unwrap();
    assert_eq!(crate::acts::offered(&domain.objects().get(id).unwrap(), ActKind::Reorder), Admission::Allowed);

    let mut rows = List::with_capacity(2);
    rows.push(goal_line(9, 2, Phase::Running, 1)).unwrap();
    rows.push(goal_line(8, 1, Phase::Running, 2)).unwrap();
    drive(
        &mut domain,
        Time::ZERO,
        Watched { watch, event: WatchEvent::Delivered(Delivery::Tasks(Box::new(Tasks { rows, more: false }))) },
    );
    assert_eq!(domain.project_page().goals.as_slice()[0].chip.task, 9);
    let Request::Client { asked: jig_client::Event::Read { handle: read, read: jig_client::Readable::Jig(what) } } =
        &opened[3]
    else {
        panic!("goals read from start")
    };
    assert_eq!(*what, Read::Tasks { query: crate::queries::ended_goals(1, 2), live: None, ended: None });
    let read = *read;
    let mut rows = List::with_capacity(2);
    rows.push(goal_line(9, 2, Phase::Done, 2)).unwrap();
    rows.push(goal_line(8, 1, Phase::Done, 1)).unwrap();
    let cursor = Cursor::new(Box::from(b"ended goals edge".as_slice()), 1024).expect("bounded opaque cursor");
    drive(
        &mut domain,
        Time::ZERO,
        ReadPage {
            read,
            result: ReadResult::Value(ReadValue::Tasks(Box::new(TaskPage {
                live: List::with_capacity(0),
                live_after: None,
                ended: EndedGroup::Rows { rows, after: Some(cursor.clone()), cut: false },
            }))),
        },
    );
    assert_eq!(domain.project_page().ended.as_slice()[0].chip.task, 9);
    assert_eq!(domain.project_page().listed_goals.len(), 2);
    let winner = domain.project_page().listed_goals.as_slice()[0].clone();
    assert_eq!(winner.chip.task, 9);
    assert_eq!(winner.tracked, Some(2), "live watch row wins over the read duplicate");
    assert_eq!(winner.chip.phase, Phase::Running);
    domain.reclaim();
    let more = drive(
        &mut domain,
        Time::ZERO,
        Event::Act { action: Act::More { window: skein_lib::Token::new(GOALS_WINDOW), end: End::After } },
    );
    let Request::Client { asked: jig_client::Event::Read { read: jig_client::Readable::Jig(what), .. } } = &more[0]
    else {
        panic!("ended goals continuation")
    };
    assert_eq!(*what, Read::Tasks { query: crate::queries::ended_goals(1, 2), live: None, ended: Some(cursor) });
    project_task_filters_and_spend_pages();
}
fn a_goal_past_the_pool_is_confirmed_as_proposed_and_sent_once() {
    use crate::{action::Act, fields::Purpose, page::project::goals::GoalForm};
    let (mut domain, opened) = project_open(ProjectSection::Goals);
    let read = project_read(&opened);
    drive(
        &mut domain,
        Time::ZERO,
        ReadPage { read, result: ReadResult::Value(ReadValue::Project(Box::new(project_document()))) },
    );
    drive(
        &mut domain,
        Time::ZERO,
        Event::Act { action: Act::Edit { field: Purpose::NewGoal, text: text(b"new goal") } },
    );
    let form =
        GoalForm { project: 1, executor: OwnerExecutor::Agent { charter: 1 }, budget: 21, deadline: None, priority: 2 };
    drive(&mut domain, Time::ZERO, Event::Act { action: Act::GoalForm { form } });
    let opened = drive(&mut domain, Time::ZERO, Event::Act { action: Act::Submit { field: Purpose::NewGoal } });
    assert!(opened.is_empty());
    let projected = domain.project_page();
    let confirmation = projected.form.confirmation.as_ref().unwrap();
    assert!(confirmation.proposed);
    assert_eq!(confirmation.pool_left, 20);
    assert!(domain.confirmation().is_none());
    let mut task = super::confirmation::task();
    task.acts = List::with_capacity(1);
    task.acts.push(Offered { effect: None, act: ActKind::Message, admission: Admission::Allowed }).unwrap();
    let id = domain.objects.name(ObjectKey::Task(8), crate::objects::Source::Window { number: 99 }).unwrap();
    assert!(domain.objects.put(ObjectKey::Task(8), crate::objects::Body::Task(Box::new(task))));
    crate::confirm::open(&mut domain.ui, &super::spine::env(Time::ZERO), id, ActKind::Message, &domain.client);
    assert!(domain.project_page().form.confirmation.is_none());
    assert!(domain.confirmation().is_some());
    drive(&mut domain, Time::ZERO, Event::Act { action: Act::Submit { field: Purpose::NewGoal } });
    assert!(domain.project_page().form.confirmation.is_some());
    assert!(domain.confirmation().is_none());
    let sent = drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm });
    let Request::Client { asked: jig_client::Event::Send { key: request, request: jig_client::Ask::Jig(ask), .. } } =
        &sent[0]
    else {
        panic!("one goal intent")
    };
    assert_eq!(
        **ask,
        crate::tests::fixtures::goal_request(
            GoalForm {
                project: 1,
                executor: OwnerExecutor::Agent { charter: 1 },
                budget: 21,
                priority: 2,
                deadline: None
            },
            text(b"new goal")
        )
    );
    assert_eq!(sent.len(), 1);
    assert!(drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm }).is_empty());
    let request = *request;
    drive(&mut domain, Time::ZERO, AnsweredByEngine { request, answer: refusal_answer(Refusal::NotOffered) });
    assert_eq!(domain.project_page().form.refusal, Some(Refusal::NotOffered));
    assert_eq!(domain.fields().get(Purpose::NewGoal).unwrap().text, text(b"new goal"));
    drive(&mut domain, Time::ZERO, Event::Act { action: Act::Submit { field: Purpose::NewGoal } });
    let sent = drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm });
    let Request::Client { asked: jig_client::Event::Send { key: request, .. } } = sent[0] else { panic!("retry goal") };
    drive(
        &mut domain,
        Time::ZERO,
        Event::Act { action: Act::Edit { field: Purpose::NewGoal, text: text(b"later words") } },
    );
    drive(
        &mut domain,
        Time::ZERO,
        AnsweredByEngine {
            request,
            answer: Answer::Done(Outcome::Made { what: Made::Tasks { tasks: crate::tests::fixtures::one(42) } }),
        },
    );
    assert_eq!(domain.address, Address::task(42));
    assert_eq!(domain.fields().get(Purpose::NewGoal).unwrap().text, text(b"later words"));
}
#[test]
fn goal_bounds_control_choices_without_me_authority_inference() {
    use crate::{action::Act, fields::Purpose, page::project::goals::GoalForm};
    use skein_lib::Wall;
    let (mut domain, opened) = project_open(ProjectSection::Goals);
    let read = project_read(&opened);
    let mut doc = project_document();
    let latest = Wall::from_nanos(100);
    doc.acts.get_mut(0).unwrap().admission = Admission::Bounded {
        within: Bounds::Create(CreateBounds {
            authority: Box::new(crate::tests::cards_objects::owner_authority()),
            pool_left: 20,
            procedures: List::with_capacity(0),
            charters: {
                let mut rows = List::with_capacity(1);
                rows.push(99).unwrap();
                rows
            },
            latest: Some(latest),
            budget: 3,
            maximum_budget: 100,
            maximum_deadline: None,
        }),
        beyond: Beyond::Refused(Refusal::Role { held: Some(HeldRole::Policy { role: 2 }) }),
    };
    crate::tests::fixtures::set_creating(&mut domain, doc.number, &doc.acts.get(0).unwrap().admission);
    drive(&mut domain, Time::ZERO, ReadPage { read, result: ReadResult::Value(ReadValue::Project(Box::new(doc))) });
    drive(
        &mut domain,
        Time::ZERO,
        Event::Act { action: Act::Edit { field: Purpose::NewGoal, text: text(b"bounded") } },
    );
    for (charter, budget, deadline, allowed) in [
        (99, 3, Some(latest), true),
        (99, 4, Some(latest), false),
        (99, 101, Some(latest), false),
        (1, 3, Some(latest), false),
        (99, 3, Some(Wall::from_nanos(101)), false),
        (99, 3, None, false),
    ] {
        let form = GoalForm { project: 1, executor: OwnerExecutor::Agent { charter }, budget, deadline, priority: 1 };
        drive(&mut domain, Time::ZERO, Event::Act { action: Act::GoalForm { form } });
        drive(&mut domain, Time::ZERO, Event::Act { action: Act::Submit { field: Purpose::NewGoal } });
        assert_eq!(domain.project_page().form.confirmation.is_some(), allowed);
        if allowed {
            let projected = domain.project_page();
            let open = projected.form.confirmation.as_ref().unwrap();
            assert_eq!(open.pool_left, 20);
            assert!(!open.proposed);
        } else {
            assert_eq!(
                domain.project_page().form.refusal,
                Some(if charter == 1 || budget > 100 {
                    Refusal::NotOffered
                } else {
                    Refusal::Role { held: Some(HeldRole::Policy { role: 2 }) }
                })
            );
            assert!(drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm }).is_empty());
        }
    }
    let form = GoalForm {
        project: 1,
        executor: OwnerExecutor::Agent { charter: 99 },
        budget: 3,
        deadline: Some(latest),
        priority: 1,
    };
    drive(&mut domain, Time::ZERO, Event::Act { action: Act::GoalForm { form } });
    drive(&mut domain, Time::ZERO, Event::Act { action: Act::Submit { field: Purpose::NewGoal } });
    assert!(domain.project_page().form.confirmation.is_some());
    let mut moved = domain.project_page().document.as_ref().unwrap().clone();
    match &mut moved.acts.get_mut(0).unwrap().admission {
        Admission::Bounded { within: Bounds::Create(CreateBounds { maximum_deadline, .. }), .. } => {
            *maximum_deadline = Some(Wall::from_nanos(99));
        }
        Admission::Allowed | Admission::Proposed { .. } | Admission::Bounded { .. } | Admission::Refused(_) => {
            panic!("bounded goal")
        }
    }
    let mut out = skein_lib::Queue::with_capacity(crate::max_out(&LIMITS));
    crate::tests::fixtures::set_creating(&mut domain, moved.number, &moved.acts.get(0).unwrap().admission);
    crate::page::project::document(&mut domain.ui, &super::spine::env(Time::ZERO), moved, &mut out, &domain.client);
    assert!(domain.project_page().form.confirmation.is_none());
    assert_eq!(domain.project_page().form.refusal, Some(Refusal::NotOffered));
    assert!(drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm }).is_empty());
    let mut refused = domain.project_page().document.as_ref().unwrap().clone();
    refused.acts.get_mut(0).unwrap().admission =
        Admission::Refused(Refusal::Role { held: Some(HeldRole::Policy { role: 2 }) });
    crate::tests::fixtures::set_creating(&mut domain, refused.number, &refused.acts.get(0).unwrap().admission);
    crate::page::project::document(&mut domain.ui, &super::spine::env(Time::ZERO), refused, &mut out, &domain.client);
    drive(&mut domain, Time::ZERO, Event::Act { action: Act::Submit { field: Purpose::NewGoal } });
    assert_eq!(domain.project_page().form.refusal, Some(Refusal::Role { held: Some(HeldRole::Policy { role: 2 }) }));
    bounded_goal_nobody_accepts(&mut domain, latest, &mut out);
}

#[expect(
    clippy::too_many_lines,
    reason = "whole existing story preserves every assertion and explicit page-loss fields"
)]
fn project_notes_keep_revisions_and_settings_deployment_keep_facts_and_parts() {
    use crate::{action::Act, objects::Body};
    let (mut domain, opened) = project_open(ProjectSection::Notes);
    let Request::Client { asked: jig_client::Event::Read { handle: read, .. } } = opened[2] else {
        panic!("notes read")
    };
    let mut acts = List::with_capacity(2);
    acts.push(Offered { effect: None, act: ActKind::EditNote, admission: Admission::Allowed }).unwrap();
    acts.push(Offered { effect: None, act: ActKind::DeleteNote, admission: Admission::Allowed }).unwrap();
    let note = Note {
        number: 3,
        revision: Revision { value: 1 },
        scope: NoteScope::Project(1),
        description: DocumentText::complete(text(b"description")),
        body: DocumentText::complete(text(b"body")),
        writer: NoteAuthor::Party { party: 7 },
        tasks: List::with_capacity(0),
        changed: Some(skein_lib::Wall::EPOCH),
        acts,
        cut: false,
    };
    let mut rows = List::with_capacity(1);
    rows.push(note.clone()).unwrap();
    drive(
        &mut domain,
        Time::ZERO,
        ReadPage {
            read,
            result: ReadResult::Value(ReadValue::Notes(Box::new(NotePage {
                cut: false,
                rows,
                before: None,
                after: None,
            }))),
        },
    );
    let id = domain.project_page().notes.as_slice()[0];
    assert_eq!(crate::acts::offered(&domain.objects().get(id).unwrap(), ActKind::EditNote), Admission::Allowed);
    drive(&mut domain, Time::ZERO, Event::Act { action: Act::Intend { object: id.token(), act: ActKind::DeleteNote } });
    assert_eq!(domain.confirmation().unwrap().revision, Some(Revision { value: 1 }));
    let sent = drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm });
    let Request::Client { asked: jig_client::Event::Send { key: request, .. } } = sent[0] else {
        panic!("delete note request")
    };
    let refresh = drive(
        &mut domain,
        Time::ZERO,
        AnsweredByEngine {
            request,
            answer: refusal_answer(Refusal::Moved { now: Revision { value: 2 }, what: Moved::Note }),
        },
    );
    let Request::Client { asked: jig_client::Event::Read { handle: read, .. } } = refresh[0] else {
        panic!("moved note read")
    };
    let mut newer = note;
    newer.revision = Revision { value: 2 };
    newer.body = DocumentText::complete(text(b"new body"));
    let mut rows = List::with_capacity(1);
    rows.push(newer).unwrap();
    drive(
        &mut domain,
        Time::ZERO,
        ReadPage {
            read,
            result: ReadResult::Value(ReadValue::Notes(Box::new(NotePage {
                cut: false,
                rows,
                before: None,
                after: None,
            }))),
        },
    );
    let Some(Body::Note { document, .. }) = &domain.objects().get(id).unwrap().body else { panic!("updated note") };
    assert_eq!(document.revision, Revision { value: 2 });
    assert_eq!(document.body, DocumentText::complete(text(b"new body")));
    assert_eq!(domain.project_page().notes.as_slice()[0], id);
    domain.reclaim();
    let moved = drive(
        &mut domain,
        Time::ZERO,
        Event::Went { place: Place::Jig(Address::Project { project: 1, section: ProjectSection::Settings }) },
    );
    let read = project_read(&moved);
    let mut doc = project_document();
    let resource = Part::Resource { connector: 1, path: resource_path(b"resource") };
    doc.resources = List::with_capacity(1);
    doc.resources.push(resource.clone()).unwrap();
    crate::tests::fixtures::set_creating(&mut domain, doc.number, &doc.acts.get(0).unwrap().admission);
    let shown =
        drive(&mut domain, Time::ZERO, ReadPage { read, result: ReadResult::Value(ReadValue::Project(Box::new(doc))) });
    assert_eq!(shown.as_ref(), &[Request::ShowPart { part: resource, scope: PartScope::Project(1) }]);
    assert_eq!(domain.project_page().document.as_ref().unwrap().period, 1);
    let opened = drive(&mut domain, Time::ZERO, Event::Went { place: Place::Jig(Address::Deployment) });
    let watch = open_token(&opened);
    let part = Part::Deployment { connector: 1 };
    let mut parts = List::with_capacity(1);
    parts.push(part.clone()).unwrap();
    let doc = Deployment {
        hosts: List::with_capacity(0),
        accounts: List::with_capacity(0),
        capacity: List::with_capacity(0),
        proposals: 3,
        refusals: List::with_capacity(0),
        parts,
    };
    let shown = drive(
        &mut domain,
        Time::ZERO,
        Watched { watch, event: WatchEvent::Delivered(Delivery::Deployment(Box::new(doc))) },
    );
    assert_eq!(shown.as_ref(), &[Request::ShowPart { part, scope: PartScope::Deployment }]);
    assert_eq!(domain.deployment_page().document.as_ref().unwrap().proposals, 3);
}

fn project_task_filters_and_spend_pages() {
    use crate::{action::Act, page::project::SPEND_WINDOW};
    let (mut domain, opened) = project_open(ProjectSection::Tasks);
    let watch = open_token(&opened);
    let mut rows = List::with_capacity(2);
    rows.push(crate::tests::fixtures::task_line(
        tree_node(9, Phase::Running, 0).chip,
        1,
        OwnerRequester::Person(8),
        skein_lib::Wall::from_nanos(2),
    ))
    .unwrap();
    rows.push(crate::tests::fixtures::task_line(
        tree_node(8, Phase::Running, 0).chip,
        1,
        OwnerRequester::Person(7),
        skein_lib::Wall::from_nanos(1),
    ))
    .unwrap();
    drive(
        &mut domain,
        Time::ZERO,
        Watched {
            watch,
            event: WatchEvent::Delivered(Delivery::Tasks(Box::new(Tasks { rows: rows.clone(), more: true }))),
        },
    );
    assert_eq!(domain.project_page().tasks.as_slice()[0].chip.task, 9);
    assert!(domain.project_page().tasks_more);
    let filter = TaskQuery {
        phase: Some(PhaseClass::Working),
        executor: Some(ExecutorKind::Person),
        root: Some(10),
        requester: Some(OwnerRequester::Person(7)),
        ..domain.project_page().task_filter
    };
    let selected = rows.get(1).unwrap().clone();
    rows.clear();
    rows.push(selected).unwrap();
    let opened = drive(&mut domain, Time::ZERO, Event::Act { action: Act::FilterTasks { filter } });
    let watch = open_token(&opened);
    drive(
        &mut domain,
        Time::ZERO,
        Watched { watch, event: WatchEvent::Delivered(Delivery::Tasks(Box::new(Tasks { rows, more: false }))) },
    );
    assert_eq!(domain.project_page().tasks.len(), 1);
    assert!(!domain.project_page().tasks_more);
    assert_eq!(domain.project_page().tasks.as_slice()[0].chip.task, 8);
    let id = domain.objects().find(ObjectKey::Task(8)).unwrap();
    assert_eq!(domain.objects().get(id).unwrap().project, Some(1));
    let (mut domain, opened) = project_open(ProjectSection::Spend);
    let Request::Client {
        asked: jig_client::Event::Read { handle: read, read: jig_client::Readable::Jig(Read::Spend { project: 1, .. }) },
    } = opened[2]
    else {
        panic!("past spend read")
    };
    let mut rows = List::with_capacity(1);
    rows.push(SpendPeriod {
        period: 1,
        budget: budget(),
        pools: List::with_capacity(0),
        tasks: List::with_capacity(0),
    })
    .unwrap();
    let cursor = Cursor::new(Box::from(b"past period edge".as_slice()), 1024).expect("bounded opaque cursor");
    drive(
        &mut domain,
        Time::ZERO,
        ReadPage {
            read,
            result: ReadResult::Value(ReadValue::Spend(Box::new(SpendPage {
                cut: false,
                rows,
                before: None,
                after: Some(cursor.clone()),
            }))),
        },
    );
    assert_eq!(domain.project_page().spend.as_slice()[0].period, 1);
    let more = drive(
        &mut domain,
        Time::ZERO,
        Event::Act { action: Act::More { window: skein_lib::Token::new(SPEND_WINDOW), end: End::After } },
    );
    let Request::Client { asked: jig_client::Event::Read { read: jig_client::Readable::Jig(what), .. } } = &more[0]
    else {
        panic!("past spend continuation")
    };
    assert_eq!(*what, Read::Spend { project: 1, after: Some(cursor) });
}

// One process per page family keeps startup costs within this session's share;
// each scenario above still allocates its own state and retains every assertion.
#[test]
fn frame_navigation_and_non_work_pages_follow_owner_facts() {
    the_frame_counts_needs_exactly_and_caps_updates_on_every_page();
    navigation_keeps_the_application_order_and_leaves_out_inapplicable_pages();
    non_work_pages_follow_their_causes_and_missing_tasks_do_not_reveal_visibility();
    deployment_without_a_role_is_missing_and_never_watched();
}
#[test]
fn inbox_and_conversation_pages_preserve_context_order_and_request_lifetimes() {
    inbox_orders_both_groups_filters_context_and_removes_role_entries_for_every_holder();
    opening_updates_marks_the_newest_result_displayed_as_seen();
    starting_a_chat_joins_refuses_and_opens_only_after_its_durable_answer();
}
#[test]
fn task_pages_preserve_executor_sections_tree_history_and_part_lifetimes() {
    task_sections_and_activity_sources_follow_the_executor();
    task_plan_keeps_folded_held_counts_and_filters_without_losing_rows();
    task_history_pages_and_scope_changes_discard_old_scope_reads();
    task_parts_follow_the_section_and_leave_with_the_page_generation();
}
#[test]
fn project_and_deployment_pages_preserve_facts_confirmations_and_revision_checks() {
    project_board_orders_live_waiting_and_ended_goals_and_names_task_cards();
    a_goal_past_the_pool_is_confirmed_as_proposed_and_sent_once();
    project_notes_keep_revisions_and_settings_deployment_keep_facts_and_parts();
}

pub(super) fn read_named(requests: &[Request], expected: &Read) -> jig_client::Handle {
    for request in requests {
        if let Request::Client {
            asked: jig_client::Event::Read { handle: read, read: jig_client::Readable::Jig(what) },
        } = request
            && what == expected
        {
            return *read;
        }
    }
    panic!("expected the named read");
}
fn reply_entry(task: u64, seen: u64, last: u64) -> InboxEntry {
    let mut row = entry(task, InboxGroup::Replies, last);
    let mut line = list_entry(task, Phase::Idle, last);
    line.conversation = Some(ConversationRead {
        seen,
        unread: Count { value: u32::try_from(last.saturating_sub(seen)).unwrap(), limited: false },
    });
    row.position = last;
    row.reply = Some(Box::new(line));
    row
}
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "whole existing story preserves every assertion and explicit page-loss fields"
)]
fn inbox_reply_receipts_keep_whole_chat_facts_shared_task_documents_and_seen_requests() {
    use crate::{action::Act, objects::Body, page::inbox::Filter};
    let (mut domain, page) = live_inbox();
    let mut rows = List::with_capacity(1);
    rows.push(reply_entry(8, 4, 7)).unwrap();
    let out = inbox_delivery(&mut domain, page, rows);
    let id = domain.objects().find(ObjectKey::Task(8)).unwrap();
    let object = domain.objects().get(id).unwrap();
    assert_eq!(object.project, Some(1));
    assert_eq!(object.reply.as_ref().unwrap().first_words.bytes(), b"held checkout");
    assert_eq!(object.reply.as_ref().unwrap().first_unread, 5);
    let Some(Body::Chip(chip)) = &object.body else {
        panic!("whole committed chat chip");
    };
    assert_eq!(chip.title.bytes(), b"held checkout");
    assert_eq!(object.sources.len(), 1);
    assert!(domain.conversation_rows().rows.is_empty());

    let mut task = super::confirmation::task();
    task.chip.task = 8;
    task.chip.title = DocumentText::complete(text(b"current task title"));
    task.spec = specification(text(b"whole task specification"));
    drive(
        &mut domain,
        Time::ZERO,
        ReadPage {
            read: read_named(&out, &Read::Task { task: 8 }),
            result: ReadResult::Value(ReadValue::Task(Box::new(task.clone()))),
        },
    );
    domain.reclaim();
    let filter = Filter { project: Some(1), kind: Some(InboxGroup::Replies), goal: None };
    let out = drive(&mut domain, Time::ZERO, Event::Act { action: Act::FilterInbox { filter } });
    let what = Read::Inbox {
        group: InboxReadGroup::Updates,
        project: filter.project,
        kind: filter.kind,
        goal: filter.goal,
        after: None,
    };
    let mut rows = List::with_capacity(1);
    rows.push(reply_entry(8, 6, 9)).unwrap();
    drive(
        &mut domain,
        Time::ZERO,
        ReadPage {
            read: read_named(&out, &what),
            result: ReadResult::Value(ReadValue::Inbox(Box::new(InboxPage {
                cut: false,
                rows,
                before: Some(Cursor::new(Box::from(b"back".as_slice()), 1024).expect("bounded opaque cursor")),
                after: None,
            }))),
        },
    );
    assert_eq!(
        domain.objects().get(id).unwrap().reply.as_ref().unwrap().first_unread,
        5,
        "live watch row wins over read duplicate"
    );
    inbox_delivery(&mut domain, page, List::with_capacity(0));
    domain.reclaim();
    let out = drive(&mut domain, Time::ZERO, Event::Act { action: Act::FilterInbox { filter } });
    let mut rows = List::with_capacity(1);
    rows.push(reply_entry(8, 6, 9)).unwrap();
    drive(
        &mut domain,
        Time::ZERO,
        ReadPage {
            read: read_named(&out, &what),
            result: ReadResult::Value(ReadValue::Inbox(Box::new(InboxPage {
                cut: false,
                rows,
                before: Some(Cursor::new(Box::from(b"back".as_slice()), 1024).expect("bounded opaque cursor")),
                after: None,
            }))),
        },
    );
    let object = domain.objects().get(id).unwrap();
    assert_eq!(object.reply.as_ref().unwrap().first_unread, 7);
    assert_eq!(object.sources.len(), 1, "watch and read share the displayed row owner");
    let Some(Body::Task(kept)) = &object.body else {
        panic!("task document retained");
    };
    assert_eq!(**kept, task, "reply receipts do not replace the task's facts");
    let out = drive(&mut domain, Time::ZERO, Event::Act { action: Act::OpenChat { task: 8 } });
    let mut marked = false;
    for request in &out {
        if let Request::Client { asked: jig_client::Event::Send { request: jig_client::Ask::Jig(ask), .. } } = request {
            assert_eq!(**ask, Ask::Seen { through: SeenThrough::Conversation { task: 8, through: 9 } });
            marked = true;
        }
    }
    assert!(marked, "an inbox-only reply marks its committed last position");
    assert!(domain.objects().get(id).is_none(), "leaving drops unretained page objects");

    let (mut domain, page) = live_inbox();
    let mut bad = reply_entry(8, 1, 2);
    bad.reply.as_mut().unwrap().chip.task = 9;
    let mut rows = List::with_capacity(1);
    rows.push(bad).unwrap();
    inbox_delivery(&mut domain, page, rows);
    assert!(domain.objects().get(domain.objects().find(ObjectKey::Task(8)).unwrap()).unwrap().reply.is_none());
    assert!(domain.objects().find(ObjectKey::Task(9)).is_none(), "mismatched facts cannot name another chat");
}

#[test]
fn bounded_goal_keeps_every_supplied_policy_recipient_in_order() {
    use crate::{action::Act, fields::Purpose, page::project::goals::GoalForm};
    let (mut domain, opened) = project_open(ProjectSection::Goals);
    let read = project_read(&opened);
    let mut roles = List::with_capacity(2);
    roles.push(7).unwrap();
    roles.push(2).unwrap();
    let recipient = Recipient::Roles { project: 1, roles };
    let mut doc = project_document();
    let Admission::Bounded { beyond, .. } = &mut doc.acts.get_mut(0).unwrap().admission else {
        panic!("bounded goal offer");
    };
    *beyond = Beyond::Proposed { to: recipient.clone() };
    crate::tests::fixtures::set_creating(&mut domain, doc.number, &doc.acts.get(0).unwrap().admission);
    drive(&mut domain, Time::ZERO, ReadPage { read, result: ReadResult::Value(ReadValue::Project(Box::new(doc))) });
    drive(
        &mut domain,
        Time::ZERO,
        Event::Act { action: Act::Edit { field: Purpose::NewGoal, text: text(b"collective") } },
    );
    let form =
        GoalForm { project: 1, executor: OwnerExecutor::Agent { charter: 1 }, budget: 21, deadline: None, priority: 1 };
    drive(&mut domain, Time::ZERO, Event::Act { action: Act::GoalForm { form } });
    drive(&mut domain, Time::ZERO, Event::Act { action: Act::Submit { field: Purpose::NewGoal } });
    let projected = domain.project_page();
    let open = projected.form.confirmation.as_ref().unwrap();
    assert_eq!(open.to, Some(recipient));
    assert!(open.proposed);
    let emitted = drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm });
    assert_eq!(emitted.len(), 1);
    let Request::Client { asked: jig_client::Event::Send { request: jig_client::Ask::Jig(ask), .. } } = &emitted[0]
    else {
        panic!("one goal intent")
    };
    assert_eq!(
        **ask,
        crate::tests::fixtures::goal_request(
            GoalForm {
                project: 1,
                executor: OwnerExecutor::Agent { charter: 1 },
                budget: 21,
                priority: 1,
                deadline: None
            },
            text(b"collective")
        )
    );
}

fn specification(words: Text) -> SpecificationDocument {
    SpecificationDocument::complete(Specification {
        asks: false,
        words,
        parameters: List::with_capacity(0),
        inputs: List::with_capacity(0),
    })
}

fn bounded_goal_nobody_accepts(domain: &mut Fixture, latest: skein_lib::Wall, out: &mut skein_lib::Queue<Request>) {
    use crate::{action::Act, fields::Purpose, page::project::goals::GoalForm};
    let nobody = Refusal::Authority { lacking: Lacking::Approver, proposable: Some(false) };
    let mut refused = domain.project_page().document.as_ref().unwrap().clone();
    refused.acts.get_mut(0).unwrap().admission = Admission::Bounded {
        within: Bounds::Create(CreateBounds {
            authority: Box::new(crate::tests::cards_objects::owner_authority()),
            pool_left: 20,
            procedures: List::with_capacity(0),
            charters: {
                let mut rows = List::with_capacity(1);
                rows.push(99).unwrap();
                rows
            },
            latest: Some(latest),
            budget: 3,
            maximum_budget: 100,
            maximum_deadline: None,
        }),
        beyond: Beyond::Refused(nobody.clone()),
    };
    crate::tests::fixtures::set_creating(domain, refused.number, &refused.acts.get(0).unwrap().admission);
    crate::page::project::document(&mut domain.ui, &super::spine::env(Time::ZERO), refused, out, &domain.client);
    let form = GoalForm {
        project: 1,
        executor: OwnerExecutor::Agent { charter: 99 },
        budget: 4,
        deadline: Some(latest),
        priority: 1,
    };
    drive(domain, Time::ZERO, Event::Act { action: Act::GoalForm { form } });
    let emitted = drive(domain, Time::ZERO, Event::Act { action: Act::Submit { field: Purpose::NewGoal } });
    assert!(emitted.is_empty());
    assert_eq!(domain.project_page().form.refusal, Some(nobody));
    assert!(domain.project_page().form.confirmation.is_none());
    assert!(drive(domain, Time::ZERO, Event::Act { action: Act::Confirm }).is_empty());
}

#[test]
fn goal_read_from_start_merges_live_watch_by_key() {
    let (mut domain, opened) = project_open(ProjectSection::Goals);
    let watch = open_token(&opened);
    let mut live = List::with_capacity(1);
    live.push(goal_line(9, 2, Phase::Running, 2)).unwrap();
    drive(
        &mut domain,
        Time::ZERO,
        Watched { watch, event: WatchEvent::Delivered(Delivery::Tasks(Box::new(Tasks { rows: live, more: false }))) },
    );
    let query = Read::Tasks { query: crate::queries::ended_goals(1, 2), live: None, ended: None };
    let mut rows = List::with_capacity(2);
    rows.push(goal_line(9, 99, Phase::Running, 1)).unwrap();
    rows.push(goal_line(8, 1, Phase::Done, 1)).unwrap();
    drive(
        &mut domain,
        Time::ZERO,
        ReadPage {
            read: read_named(&opened, &query),
            result: ReadResult::Value(ReadValue::Tasks(Box::new(TaskPage {
                live: List::with_capacity(0),
                live_after: None,
                ended: EndedGroup::Rows { rows, after: None, cut: false },
            }))),
        },
    );
    assert_eq!(domain.project_page().listed_goals.len(), 2);
    assert_eq!(domain.project_page().listed_goals.get(0).unwrap().chip.task, 9);
    assert_eq!(domain.project_page().listed_goals.get(0).unwrap().tracked, Some(2));
    assert_eq!(domain.project_page().listed_goals.get(1).unwrap().chip.phase, Phase::Done);
    let Some(crate::objects::Body::Chip(chip)) =
        &domain.objects().get(domain.objects().find(ObjectKey::Task(9)).unwrap()).unwrap().body
    else {
        panic!("live goal chip");
    };
    assert_eq!(chip.phase, Phase::Running);
}
fn assert_only_inbox_read(requests: &[Request], expected: &Read) {
    let mut count = 0_u32;
    for request in requests {
        if let Request::Client {
            asked: jig_client::Event::Read { read: jig_client::Readable::Jig(query @ Read::Inbox { .. }), .. },
        } = request
        {
            assert_eq!(query, expected);
            count = count.checked_add(1).expect("bounded request count");
        }
    }
    assert_eq!(count, 1, "only the selected group's exact query is emitted");
}
#[test]
fn inbox_group_pages_keep_orders_cursors_and_live_precedence() {
    check_inbox_group_pages_keep_orders_cursors_and_live_precedence();
}
fn send(domain: &mut Fixture, event: impl crate::tests::fixtures::Input) -> Box<[Request]> {
    domain.drive(Time::ZERO, event)
}
#[expect(
    clippy::too_many_lines,
    reason = "one scenario follows both independent cursor positions and watch precedence"
)]
fn check_inbox_group_pages_keep_orders_cursors_and_live_precedence() {
    use crate::{
        action::Act,
        page::conversations::{INBOX_UPDATES_WINDOW, INBOX_WINDOW},
        page::inbox::Filter,
    };
    let mut limits = LIMITS;
    limits.rows = 4;
    limits.objects = 8;
    limits.reads = 16;
    let row = |number, group, at| {
        let mut row = entry(number, group, at);
        row.object = match group {
            InboxGroup::Escalations => ObjectKey::Escalation(number),
            InboxGroup::Questions => ObjectKey::Question { task: number, message: 1 },
            InboxGroup::Results => ObjectKey::Result(number),
            InboxGroup::Proposals | InboxGroup::PersonTasks | InboxGroup::Replies => unreachable!("fixture kinds"),
        };
        row
    };
    let mut domain = Fixture::new(&limits);
    let f = open_token(&send(
        &mut domain,
        Event::Start { seed: 8, place: Place::Jig(Address::Inbox { project: None }), records: List::with_capacity(0) },
    ));
    send(&mut domain, Served { served: true });
    let entered =
        send(&mut domain, Watched { watch: f, event: WatchEvent::Delivered(Delivery::Me(Box::new(frame(7)))) });
    let watch = open_token(&entered);
    let mut live = List::with_capacity(1);
    live.push(row(7, InboxGroup::Escalations, 1)).unwrap();
    send(&mut domain, Watched { watch, event: WatchEvent::Delivered(Delivery::Inbox(Box::new(inbox(live)))) });
    let mut needs_query =
        Read::Inbox { group: InboxReadGroup::Needs, project: None, kind: None, goal: None, after: None };
    let mut updates_query =
        Read::Inbox { group: InboxReadGroup::Updates, project: None, kind: None, goal: None, after: None };
    let mut needs = List::with_capacity(3);
    needs.push(row(7, InboxGroup::Escalations, 99)).unwrap();
    needs.push(row(9, InboxGroup::Questions, 3)).unwrap();
    needs.push(row(8, InboxGroup::Questions, 2)).unwrap();
    let mut updates = List::with_capacity(2);
    updates.push(row(10, InboxGroup::Results, 4)).unwrap();
    updates.push(row(11, InboxGroup::Results, 5)).unwrap();
    let needs_after = Cursor::new(Box::from(b"needs-next".as_slice()), 1024).expect("bounded opaque cursor");
    let updates_after = Cursor::new(Box::from(b"updates-next".as_slice()), 1024).expect("bounded opaque cursor");
    for (query, rows, cursor) in [(&needs_query, needs, &needs_after), (&updates_query, updates, &updates_after)] {
        send(
            &mut domain,
            ReadPage {
                read: read_named(&entered, query),
                result: ReadResult::Value(ReadValue::Inbox(Box::new(InboxPage {
                    cut: false,
                    rows,
                    before: None,
                    after: Some(cursor.clone()),
                }))),
            },
        );
    }
    assert_eq!(domain.inbox().rows.len(), 5);
    for (index, key) in [
        (0, ObjectKey::Escalation(7)),
        (1, ObjectKey::Question { task: 8, message: 1 }),
        (2, ObjectKey::Question { task: 9, message: 1 }),
        (3, ObjectKey::Result(11)),
        (4, ObjectKey::Result(10)),
    ] {
        assert_eq!(domain.inbox().rows.get(index).unwrap().card_key(), key);
    }
    assert_eq!(domain.inbox().rows.get(0).unwrap().at(), Some(skein_lib::Wall::from_nanos(1)));
    domain.reclaim();
    let out = send(
        &mut domain,
        Event::Act { action: Act::More { window: skein_lib::Token::new(INBOX_WINDOW), end: End::After } },
    );
    if let Read::Inbox { after, .. } = &mut needs_query {
        *after = Some(needs_after);
    }
    assert_only_inbox_read(&out, &needs_query);
    assert_eq!(domain.inbox().updates_window.after, Some(updates_after.clone()));
    send(
        &mut domain,
        ReadPage {
            read: read_named(&out, &needs_query),
            result: ReadResult::Value(ReadValue::Inbox(Box::new(InboxPage {
                cut: false,
                rows: List::with_capacity(0),
                before: Some(Cursor::new(Box::from(b"needs-back".as_slice()), 1024).expect("bounded opaque cursor")),
                after: None,
            }))),
        },
    );
    assert!(!domain.inbox().needs_window.more);
    assert!(domain.inbox().updates_window.more);
    assert_eq!(domain.inbox().rows.len(), 3);
    domain.reclaim();
    let out = send(
        &mut domain,
        Event::Act { action: Act::More { window: skein_lib::Token::new(INBOX_UPDATES_WINDOW), end: End::After } },
    );
    if let Read::Inbox { after, .. } = &mut updates_query {
        *after = Some(updates_after);
    }
    assert_only_inbox_read(&out, &updates_query);
    let out = send(
        &mut domain,
        Event::Act {
            action: Act::FilterInbox {
                filter: Filter { project: Some(1), kind: Some(InboxGroup::Results), goal: Some(9) },
            },
        },
    );
    assert_only_inbox_read(
        &out,
        &Read::Inbox {
            group: InboxReadGroup::Updates,
            project: Some(1),
            kind: Some(InboxGroup::Results),
            goal: Some(9),
            after: None,
        },
    );
    send(
        &mut domain,
        Watched { watch, event: WatchEvent::Delivered(Delivery::Inbox(Box::new(inbox(List::with_capacity(0))))) },
    );
}

#[test]
fn procedure_goal_pairs_and_current_bounds_control_confirmation_and_exact_departure() {
    use crate::{action::Act, fields::Purpose, page::project::goals::GoalForm};
    let (mut domain, opened) = project_open(ProjectSection::Goals);
    let read = project_read(&opened);
    let exact = OwnerExecutor::Procedure { connector: u16::MAX, code: u32::MAX };
    let until = skein_lib::Wall::from_nanos(100);
    let doc = procedure_goal_document(until);
    crate::tests::fixtures::set_creating(&mut domain, doc.number, &doc.acts.get(0).unwrap().admission);
    drive(&mut domain, Time::ZERO, ReadPage { read, result: ReadResult::Value(ReadValue::Project(Box::new(doc))) });
    drive(
        &mut domain,
        Time::ZERO,
        Event::Act { action: Act::Edit { field: Purpose::NewGoal, text: text(b"numeric procedure") } },
    );
    for (executor, budget, deadline, predicted) in [
        (exact, 3, Some(until), Admission::Allowed),
        (exact, 4, Some(until), Admission::Proposed { to: Recipient::Party(9) }),
        (exact, 3, Some(skein_lib::Wall::from_nanos(101)), Admission::Proposed { to: Recipient::Party(9) }),
        (
            OwnerExecutor::Procedure { connector: 6, code: u32::MAX },
            3,
            Some(until),
            Admission::Refused(Refusal::NotOffered),
        ),
        (
            OwnerExecutor::Procedure { connector: u16::MAX, code: 7 },
            3,
            Some(until),
            Admission::Refused(Refusal::NotOffered),
        ),
        (OwnerExecutor::Agent { charter: 1 }, 3, Some(until), Admission::Refused(Refusal::NotOffered)),
        (exact, 11, Some(until), Admission::Refused(Refusal::NotOffered)),
        (exact, 3, None, Admission::Refused(Refusal::NotOffered)),
    ] {
        let form = GoalForm { project: 1, executor, budget, deadline, priority: 8 };
        drive(&mut domain, Time::ZERO, Event::Act { action: Act::GoalForm { form } });
        assert!(
            drive(&mut domain, Time::ZERO, Event::Act { action: Act::Submit { field: Purpose::NewGoal } }).is_empty()
        );
        match predicted {
            Admission::Allowed => assert!(!domain.project_page().form.confirmation.as_ref().unwrap().proposed),
            Admission::Proposed { to } => {
                assert_eq!(domain.project_page().form.confirmation.as_ref().unwrap().to, Some(to));
            }
            Admission::Refused(why) => {
                assert_eq!(domain.project_page().form.refusal, Some(why));
                assert!(domain.project_page().form.confirmation.is_none());
                assert!(drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm }).is_empty());
            }
            Admission::Bounded { .. } => panic!("concrete prediction"),
        }
    }
    let form = GoalForm { project: 1, executor: exact, budget: 4, deadline: Some(until), priority: 8 };
    drive(&mut domain, Time::ZERO, Event::Act { action: Act::GoalForm { form } });
    drive(&mut domain, Time::ZERO, Event::Act { action: Act::Submit { field: Purpose::NewGoal } });
    let mut moved = domain.project_page().document.as_ref().unwrap().clone();
    let Admission::Bounded { within: Bounds::Create(CreateBounds { procedures, .. }), .. } =
        &mut moved.acts.get_mut(0).unwrap().admission
    else {
        panic!("goal bounds");
    };
    procedures.clear();
    procedures.push(CreateProcedure { connector: 7, code: u32::MAX }).unwrap();
    let mut out = skein_lib::Queue::with_capacity(crate::max_out(&LIMITS));
    crate::tests::fixtures::set_creating(&mut domain, moved.number, &moved.acts.get(0).unwrap().admission);
    crate::page::project::document(&mut domain.ui, &super::spine::env(Time::ZERO), moved, &mut out, &domain.client);
    assert!(domain.project_page().form.confirmation.is_none());
    assert_eq!(domain.project_page().form.refusal, Some(Refusal::NotOffered));
    assert!(drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm }).is_empty());
    let mut restored = domain.project_page().document.as_ref().unwrap().clone();
    let Admission::Bounded { within: Bounds::Create(CreateBounds { procedures, .. }), .. } =
        &mut restored.acts.get_mut(0).unwrap().admission
    else {
        panic!("goal bounds");
    };
    procedures.push(CreateProcedure { connector: u16::MAX, code: u32::MAX }).unwrap();
    crate::tests::fixtures::set_creating(&mut domain, restored.number, &restored.acts.get(0).unwrap().admission);
    crate::page::project::document(&mut domain.ui, &super::spine::env(Time::ZERO), restored, &mut out, &domain.client);
    drive(&mut domain, Time::ZERO, Event::Act { action: Act::Submit { field: Purpose::NewGoal } });
    let sent = drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm });
    assert_eq!(sent.len(), 1);
    let Request::Client { asked: jig_client::Event::Send { request: jig_client::Ask::Jig(ask), .. } } = &sent[0] else {
        panic!("goal request");
    };
    assert_eq!(
        **ask,
        crate::tests::fixtures::goal_request(
            GoalForm { project: 1, executor: exact, budget: 4, priority: 8, deadline: Some(until) },
            text(b"numeric procedure")
        )
    );
    assert!(drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm }).is_empty());
}

fn procedure_goal_document(until: skein_lib::Wall) -> Project {
    let mut doc = project_document();
    doc.acts.get_mut(0).unwrap().admission = Admission::Bounded {
        within: Bounds::Create(CreateBounds {
            authority: Box::new(crate::tests::cards_objects::owner_authority()),
            pool_left: 50,
            charters: List::with_capacity(0),
            procedures: {
                let mut pairs = List::with_capacity(2);
                pairs.push(CreateProcedure { connector: u16::MAX, code: u32::MAX }).unwrap();
                pairs.push(CreateProcedure { connector: 7, code: u32::MAX }).unwrap();
                pairs
            },
            latest: Some(until),
            budget: 3,
            maximum_budget: 10,
            maximum_deadline: Some(skein_lib::Wall::from_nanos(200)),
        }),
        beyond: Beyond::Proposed { to: Recipient::Party(9) },
    };
    doc
}

#[test]
fn owner_started_chat_navigates_from_the_original_request_and_keeps_late_answers_whole() {
    use crate::{action::Act, fields::Purpose, notices::kept::Kind};
    for away in [false, true] {
        let mut domain = Fixture::new(&LIMITS);
        let token = start(&mut domain, List::with_capacity(0));
        let mut value = frame(7);
        value.projects = List::with_capacity(1);
        value.projects.push(project(1, true)).unwrap();
        delivered(&mut domain, Time::ZERO, token, value);
        drive(
            &mut domain,
            Time::ZERO,
            Event::Act { action: Act::Edit { field: Purpose::NewChat, text: text(b"start") } },
        );
        let sent = drive(&mut domain, Time::ZERO, Event::Act { action: Act::BeginConversation { project: 1 } });
        let mut request = None;
        for row in &sent {
            if let Request::Client {
                asked: jig_client::Event::Send { key: sent, request: jig_client::Ask::Jig(ask), .. },
            } = row
            {
                assert_eq!(**ask, crate::tests::fixtures::chat_request(1, text(b"start")));
                assert!(request.replace(*sent).is_none(), "one original start request");
            }
        }
        let request = request.unwrap();
        if away {
            drive(&mut domain, Time::ZERO, Event::Went { place: Place::Elsewhere });
        }
        let answer =
            Answer::Done(Outcome::Owner(OwnerOutcome::Created { first: u64::MAX, last: u64::MAX, given: None }));
        drive(&mut domain, Time::ZERO, AnsweredByEngine { request, answer: answer.clone() });
        assert!(domain.chats().starting.is_none());
        assert!(domain.fields().get(Purpose::NewChat).unwrap().text.bytes().is_empty());
        if away {
            assert_eq!(domain.notices.ids().len(), 1);
            for (notice, ()) in domain.notices.ids() {
                assert_eq!(
                    domain.notices.get(*notice).unwrap().kind,
                    Kind::Answer { object: ObjectKey::Task(u64::MAX), answer: answer.clone() }
                );
            }
            assert_eq!(domain.page(), &Page::Elsewhere);
        } else {
            let Page::Work { address, .. } = domain.page() else { panic!("actual chat task") };
            assert_eq!(*address, Address::Task { task: u64::MAX, section: TaskSection::Activity });
        }
    }
}

#[test]
fn owner_goal_answers_preserve_numeric_proposals_and_late_started_task_facts() {
    use crate::{action::Act, fields::Purpose, notices::kept::Kind, page::project::goals::GoalForm};
    for away in [false, true] {
        for proposed in [false, true] {
            let (mut domain, opened) = project_open(ProjectSection::Goals);
            let read = project_read(&opened);
            drive(
                &mut domain,
                Time::ZERO,
                ReadPage { read, result: ReadResult::Value(ReadValue::Project(Box::new(project_document()))) },
            );
            drive(
                &mut domain,
                Time::ZERO,
                Event::Act { action: Act::Edit { field: Purpose::NewGoal, text: text(b"goal") } },
            );
            let form = GoalForm {
                project: 1,
                executor: OwnerExecutor::Agent { charter: 1 },
                budget: 21,
                deadline: None,
                priority: 2,
            };
            drive(&mut domain, Time::ZERO, Event::Act { action: Act::GoalForm { form } });
            drive(&mut domain, Time::ZERO, Event::Act { action: Act::Submit { field: Purpose::NewGoal } });
            let sent = drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm });
            let Request::Client { asked: jig_client::Event::Send { key: request, .. } } = sent[0] else {
                panic!("original goal request")
            };
            if away {
                drive(&mut domain, Time::ZERO, Event::Went { place: Place::Elsewhere });
            }
            let outcome = Outcome::Owner(if proposed {
                OwnerOutcome::Proposed { proposal: u64::MAX, to: u32::MAX }
            } else {
                OwnerOutcome::Created { first: u64::MAX, last: u64::MAX, given: None }
            });
            let answer = Answer::Done(outcome.clone());
            drive(&mut domain, Time::ZERO, AnsweredByEngine { request, answer: answer.clone() });
            assert_eq!(domain.project_page().form.answer, Some(outcome));
            assert!(domain.project_page().form.pending.is_none());
            assert!(domain.fields().get(Purpose::NewGoal).unwrap().text.bytes().is_empty());
            if away || proposed {
                assert_eq!(domain.notices.ids().len(), 1);
                for (notice, ()) in domain.notices.ids() {
                    assert_eq!(domain.notices.get(*notice).unwrap().kind, Kind::PageAnswer { answer: answer.clone() });
                }
                if away {
                    assert_eq!(domain.page(), &Page::Elsewhere);
                }
            } else {
                let Page::Work { address, .. } = domain.page() else { panic!("actual goal task") };
                assert_eq!(*address, Address::task(u64::MAX));
            }
        }
    }
}

fn resource_path(bytes: &[u8]) -> List<Box<[u8]>> {
    let mut rows = List::with_capacity(1);
    rows.push(Box::from(bytes)).unwrap();
    rows
}

#[test]
fn literal_resource_names_keep_boundaries_and_open_grants_do_not_make_resource_slots() {
    use crate::address::TaskSection;
    let mut domain = Fixture::new(&LIMITS);
    let frame_watch = start(&mut domain, List::with_capacity(0));
    let initial = delivered(&mut domain, Time::ZERO, frame_watch, frame(7));
    finish_initial_inbox_reads(&mut domain, &initial);
    let opened = drive(
        &mut domain,
        Time::ZERO,
        Event::Went { place: Place::Jig(Address::Task { task: 8, section: TaskSection::Overview }) },
    );
    let watch = open_token(&opened);
    let mut first = List::with_capacity(2);
    first.push(Box::<[u8]>::from(*b"a/b")).unwrap();
    first.push(Box::<[u8]>::from(*b"c")).unwrap();
    let mut second = List::with_capacity(2);
    second.push(Box::<[u8]>::from(*b"a")).unwrap();
    second.push(Box::<[u8]>::from(*b"/bc")).unwrap();
    let one = Part::Resource { connector: 2, path: first };
    let two = Part::Resource { connector: 2, path: second };
    assert_ne!(one, two, "equal concatenated bytes are different segmented names");
    let mut doc = super::confirmation::task();
    doc.resources = List::with_capacity(2);
    doc.resources.push(TaskResource::Part(one.clone())).unwrap();
    doc.resources.push(TaskResource::Part(two.clone())).unwrap();
    doc.authority.authority.grants = List::with_capacity(1);
    doc.authority
        .authority
        .grants
        .push(OwnerGrant {
            connector: 2,
            kind: 9,
            pattern: OwnerPattern { segments: resource_path(b"a/b"), last: OwnerPatternEnd::Open(Box::from(*b"c")) },
        })
        .unwrap();
    let shown = drive(
        &mut domain,
        Time::ZERO,
        Watched { watch, event: WatchEvent::Delivered(Delivery::Task(Box::new(doc.clone()))) },
    );
    let mut parts = List::with_capacity(2);
    for request in &shown {
        if let Request::ShowPart { part, scope, .. } = request {
            assert_eq!(*scope, PartScope::Project(1));
            parts.push(part.clone()).unwrap();
        }
    }
    assert_eq!(parts.as_slice(), &[one, two]);
    let budget_opened = drive(
        &mut domain,
        Time::ZERO,
        Event::Went { place: Place::Jig(Address::Task { task: 8, section: TaskSection::Budget }) },
    );
    let budget_watch = open_token(&budget_opened);
    let budget = drive(
        &mut domain,
        Time::ZERO,
        Watched { watch: budget_watch, event: WatchEvent::Delivered(Delivery::Task(Box::new(doc))) },
    );
    let mut parts = List::with_capacity(1);
    for request in &budget {
        if let Request::ShowPart { part, scope, .. } = request {
            assert_eq!(*scope, PartScope::Project(1));
            parts.push(part.clone()).unwrap();
        }
    }
    assert_eq!(parts.as_slice(), &[Part::Kind { connector: 2, kind: 9 }], "an open pattern is not a resource name");
}

#[test]
fn goal_note_scope_keeps_its_supplied_project_instead_of_the_parent_page() {
    let mut domain = Fixture::new(&LIMITS);
    let frame_watch = start(&mut domain, List::with_capacity(0));
    let initial = delivered(&mut domain, Time::ZERO, frame_watch, frame(7));
    finish_initial_inbox_reads(&mut domain, &initial);
    drive(
        &mut domain,
        Time::ZERO,
        Event::Went { place: Place::Jig(Address::Project { project: 1, section: ProjectSection::Notes }) },
    );
    let scope = NoteScope::Task { project: u32::MAX, task: u64::MAX };
    let output = drive(
        &mut domain,
        Time::ZERO,
        Event::Act {
            action: crate::action::Act::FilterNotes {
                filter: crate::page::project::NoteFilter { scope: scope.clone(), description: Some(text(b"supplied")) },
            },
        },
    );
    let mut reads = List::with_capacity(1);
    for request in &output {
        if let Request::Client {
            asked:
                jig_client::Event::Read {
                    read: jig_client::Readable::Jig(Read::Notes { scope, description, after }), ..
                },
        } = request
        {
            reads.push((scope.clone(), description.clone(), after.clone())).unwrap();
        }
    }
    assert_eq!(reads.as_slice(), &[(scope.clone(), Some(text(b"supplied")), None)]);
    assert_eq!(domain.project_page().note_filter.as_ref().unwrap().scope, scope);
}

#[test]
fn note_reads_keep_full_writer_identity_and_optional_time_without_changing_the_named_revision() {
    for writer in [NoteAuthor::Party { party: u64::MAX }, NoteAuthor::Task { task: u64::MAX, attempt: u64::MAX }] {
        for changed in [None, Some(skein_lib::Wall::EPOCH)] {
            let (mut domain, opened) = project_open(ProjectSection::Notes);
            let Request::Client { asked: jig_client::Event::Read { handle: read, .. } } = opened[2] else {
                panic!("notes read")
            };
            let mut acts = List::with_capacity(1);
            acts.push(Offered { effect: None, act: ActKind::DeleteNote, admission: Admission::Allowed }).unwrap();
            let note = Note {
                number: 3,
                revision: Revision { value: u64::MAX },
                scope: NoteScope::Project(1),
                description: DocumentText::complete(text(b"description")),
                body: DocumentText { bytes: text(b"body"), cut: true },
                writer,
                tasks: List::with_capacity(0),
                changed,
                acts,
                cut: true,
            };
            let mut rows = List::with_capacity(1);
            rows.push(note.clone()).unwrap();
            drive(
                &mut domain,
                Time::ZERO,
                ReadPage {
                    read,
                    result: ReadResult::Value(ReadValue::Notes(Box::new(NotePage {
                        cut: false,
                        rows,
                        before: None,
                        after: None,
                    }))),
                },
            );
            let id = domain.project_page().notes.as_slice()[0];
            let Some(crate::objects::Body::Note { document, .. }) = &domain.objects().get(id).unwrap().body else {
                panic!("whole supplied note");
            };
            assert_eq!(**document, note);
            drive(
                &mut domain,
                Time::ZERO,
                Event::Act { action: crate::action::Act::Intend { object: id.token(), act: ActKind::DeleteNote } },
            );
            assert_eq!(domain.confirmation().unwrap().revision, Some(note.revision));
            let output = drive(&mut domain, Time::ZERO, Event::Act { action: crate::action::Act::Confirm });
            let mut requests = List::with_capacity(1);
            for request in &output {
                if let Request::Client { asked: jig_client::Event::Send { request: jig_client::Ask::Jig(ask), .. } } =
                    request
                {
                    requests.push(ask.as_ref().clone()).unwrap();
                }
            }
            assert_eq!(
                requests.as_slice(),
                &[Ask::DeleteNote { project: 1, note: 3, scope: note.scope, revision: note.revision }]
            );
        }
    }
}

#[test]
fn task_watch_replacements_preserve_requester_kinds_and_missing_link_facts_with_aggregate_loss() {
    let mut domain = Fixture::new(&LIMITS);
    let frame_watch = start(&mut domain, List::with_capacity(0));
    let initial = delivered(&mut domain, Time::ZERO, frame_watch, frame(7));
    finish_initial_inbox_reads(&mut domain, &initial);
    let opened = drive(&mut domain, Time::ZERO, Event::Went { place: Place::Jig(Address::task(8)) });
    let watch = open_token(&opened);
    let mut supplied = super::confirmation::task();
    let mut chip = supplied.chip.clone();
    chip.task = u64::MAX;
    for requester in [
        OwnerRequester::Task(u64::MAX),
        OwnerRequester::Person(u64::MAX),
        OwnerRequester::Deployment { project: u32::MAX },
    ] {
        for cut in [false, true] {
            supplied.requester = requester;
            supplied.cut = cut;
            supplied.inputs = List::with_capacity(2);
            supplied.inputs.push(OwnerTaskLink::Stored { number: u64::MAX, status: None }).unwrap();
            supplied.dependencies = List::with_capacity(2);
            supplied.dependencies.push(OwnerTaskLink::Retained { chip: chip.clone(), at: None }).unwrap();
            supplied.references = List::with_capacity(2);
            supplied.references.push(OwnerTaskLink::Live(chip.clone())).unwrap();
            drive(
                &mut domain,
                Time::ZERO,
                Watched { watch, event: WatchEvent::Delivered(Delivery::Task(Box::new(supplied.clone()))) },
            );
            let id = domain.task_page().header.unwrap();
            let Some(crate::objects::Body::Task(document)) = &domain.objects().get(id).unwrap().body else {
                panic!("whole watched task");
            };
            assert_eq!(**document, supplied);
            assert_eq!(document.inputs.capacity(), 2);
            assert_eq!(document.dependencies.get(0).unwrap().number(), u64::MAX);
            assert_eq!(document.references.get(0).unwrap().number(), u64::MAX);
            assert_eq!(document.inputs.get(0), Some(&OwnerTaskLink::Stored { number: u64::MAX, status: None }));
            assert_eq!(document.dependencies.get(0), Some(&OwnerTaskLink::Retained { chip: chip.clone(), at: None }));
        }
    }
}

#[test]
fn watched_task_policies_replace_every_channel_without_merging_standing_interests() {
    let mut domain = Fixture::new(&LIMITS);
    let frame_watch = start(&mut domain, List::with_capacity(0));
    let initial = delivered(&mut domain, Time::ZERO, frame_watch, frame(7));
    finish_initial_inbox_reads(&mut domain, &initial);
    let opened = drive(&mut domain, Time::ZERO, Event::Went { place: Place::Jig(Address::task(8)) });
    let watch = open_token(&opened);
    let mut supplied = super::confirmation::task();
    for rule in [
        WakeRule::Never,
        WakeRule::Immediate,
        WakeRule::Batch { count: u32::MAX, age: skein_lib::Duration::from_nanos(u64::MAX) },
    ] {
        for results in [ResultsWake::Never, ResultsWake::Each, ResultsWake::LastOrFailure] {
            supplied.wake = OwnerWakePolicy {
                words: rule,
                notices: rule,
                news: rule,
                results,
                questions: true,
                answers: false,
                timers: true,
            };
            supplied.subscriptions.entries = List::with_capacity(2);
            supplied
                .subscriptions
                .entries
                .push(Subscription {
                    number: u64::MAX,
                    kind: SubscriptionKind::Topic { connector: u16::MAX, topic: u64::MAX },
                })
                .unwrap();
            drive(
                &mut domain,
                Time::ZERO,
                Watched { watch, event: WatchEvent::Delivered(Delivery::Task(Box::new(supplied.clone()))) },
            );
            let id = domain.task_page().header.unwrap();
            let Some(crate::objects::Body::Task(document)) = &domain.objects().get(id).unwrap().body else {
                panic!("whole task policy");
            };
            assert_eq!(**document, supplied);
            assert_eq!(document.subscriptions.entries.capacity(), 2);
            supplied.subscriptions.entries = List::with_capacity(2);
            drive(
                &mut domain,
                Time::ZERO,
                Watched { watch, event: WatchEvent::Delivered(Delivery::Task(Box::new(supplied.clone()))) },
            );
            let Some(crate::objects::Body::Task(document)) = &domain.objects().get(id).unwrap().body else {
                panic!("new empty interests");
            };
            assert_eq!(**document, supplied);
            assert_eq!(document.subscriptions.entries.capacity(), 2);
            assert!(document.subscriptions.entries.is_empty());
            assert_eq!(document.wake.words, rule);
        }
    }
}

#[test]
fn task_watch_replacements_keep_served_goal_identity_and_whole_contract_loss_independent() {
    let mut domain = Fixture::new(&LIMITS);
    let frame_watch = start(&mut domain, List::with_capacity(0));
    let initial = delivered(&mut domain, Time::ZERO, frame_watch, frame(7));
    finish_initial_inbox_reads(&mut domain, &initial);
    let opened = drive(&mut domain, Time::ZERO, Event::Went { place: Place::Jig(Address::task(8)) });
    let watch = open_token(&opened);
    let mut choices = List::with_capacity(2);
    let mut fields = List::with_capacity(2);
    fields.push(FieldRule { name: Box::from(*b"\xff\0/"), max: u32::MAX }).unwrap();
    choices.push(VerdictChoice { code: u32::MAX, words: u32::MAX, followups: u32::MAX, fields }).unwrap();
    let part = OwnerPart { connector: u16::MAX, kind: OwnerPartKind::Result, key: OwnerPartKey::Number(u64::MAX) };
    for (goal, contract) in [
        (Some(8), TaskContractDocument::Report { words: u32::MAX }),
        (Some(u64::MAX), TaskContractDocument::Approval { state: Box::from(*b"\xff\0/"), part }),
        (None, TaskContractDocument::Verdict { choices, cut: true }),
        (Some(0), TaskContractDocument::Change { connector: u16::MAX, kind: u16::MAX, words: u32::MAX }),
    ] {
        let mut row = super::confirmation::task();
        row.goal = goal;
        row.result_contract = contract;
        row.spec.specification.words = DocumentText::complete(text(b""));
        row.cut = false;
        let expected = row.clone();
        drive(&mut domain, Time::ZERO, Watched { watch, event: WatchEvent::Delivered(Delivery::Task(Box::new(row))) });
        let id = domain.task_page().header.unwrap();
        let Some(crate::objects::Body::Task(retained)) = &domain.objects().get(id).unwrap().body else {
            panic!("whole task remains retained");
        };
        assert_eq!(**retained, expected);
        assert!(!retained.cut, "contract loss is not aggregate task-list loss");
        let sections = crate::page::task::offers(retained);
        assert!(crate::page::task::offered(&sections, TaskSection::Overview));
    }
}

#[test]
fn task_authority_watch_replacements_keep_bitmap_pattern_and_loss_facts_whole() {
    let mut domain = Fixture::new(&LIMITS);
    let frame_watch = start(&mut domain, List::with_capacity(0));
    let initial = delivered(&mut domain, Time::ZERO, frame_watch, frame(7));
    finish_initial_inbox_reads(&mut domain, &initial);
    let opened = drive(&mut domain, Time::ZERO, Event::Went { place: Place::Jig(Address::task(8)) });
    let watch = open_token(&opened);
    let mut row = super::confirmation::task();
    row.authority.authority.tools = u64::MAX;
    row.authority.authority.notes = u8::MAX;
    row.authority.authority.delegation.kinds = List::with_capacity(2);
    row.authority.authority.delegation.kinds.push(OwnerAuthorityExecutor::Role(u32::MAX)).unwrap();
    let mut segments = List::with_capacity(2);
    segments.push(Box::from(*b"a/b")).unwrap();
    segments.push(Box::from(*b"\xff\0")).unwrap();
    row.authority.authority.grants = List::with_capacity(2);
    row.authority
        .authority
        .grants
        .push(OwnerGrant {
            connector: u16::MAX,
            kind: u16::MAX,
            pattern: OwnerPattern { segments, last: OwnerPatternEnd::Open(Box::from(*b"c/")) },
        })
        .unwrap();
    for cut in [false, true, false] {
        row.authority.cut = cut;
        row.cut = !cut;
        drive(
            &mut domain,
            Time::ZERO,
            Watched { watch, event: WatchEvent::Delivered(Delivery::Task(Box::new(row.clone()))) },
        );
        let id = domain.task_page().header.unwrap();
        let Some(crate::objects::Body::Task(actual)) = &domain.objects().get(id).unwrap().body else {
            panic!("whole watched task");
        };
        assert_eq!(**actual, row);
        assert_eq!(actual.authority.cut, cut);
        assert_eq!(actual.cut, !cut);
        assert_eq!(actual.authority.authority.grants.capacity(), 2);
        assert_eq!(actual.authority.authority.grants.get(0).unwrap().pattern.segments.get(0).unwrap().as_ref(), b"a/b");
    }
}

#[test]
fn task_budget_watch_replacements_keep_all_owner_numbers_funders_and_optional_deadlines() {
    let mut domain = Fixture::new(&LIMITS);
    let frame_watch = start(&mut domain, List::with_capacity(0));
    let initial = delivered(&mut domain, Time::ZERO, frame_watch, frame(7));
    finish_initial_inbox_reads(&mut domain, &initial);
    let opened = drive(&mut domain, Time::ZERO, Event::Went { place: Place::Jig(Address::task(8)) });
    let watch = open_token(&opened);
    for funder in [
        Funder::Task(u64::MAX),
        Funder::Pool { project: u32::MAX, person: u64::MAX, period: u64::MAX - 1 },
        Funder::Recurring { project: u32::MAX, task: u64::MAX, period: u64::MAX - 2 },
        Funder::Period { project: u32::MAX, period: u64::MAX },
    ] {
        for deadline in [None, Some(skein_lib::Wall::EPOCH), Some(skein_lib::Wall::from_nanos(u64::MAX))] {
            let mut row = super::confirmation::task();
            row.budget = OwnerBudget {
                numbers: OwnerNumbers { budget: u64::MAX, spent: u64::MAX - 1, spent_below: 7, reserved: 11 },
                left: 17,
                funder,
                deadline,
            };
            row.authority.authority.deadline = Some(skein_lib::Wall::from_nanos(123));
            drive(
                &mut domain,
                Time::ZERO,
                Watched { watch, event: WatchEvent::Delivered(Delivery::Task(Box::new(row.clone()))) },
            );
            let id = domain.task_page().header.unwrap();
            let Some(crate::objects::Body::Task(actual)) = &domain.objects().get(id).unwrap().body else {
                panic!("whole watched task");
            };
            assert_eq!(**actual, row);
            assert_eq!(actual.budget.left, 17, "availability is supplied independently of the numbers");
            assert_eq!(actual.budget.deadline, deadline, "authority is a separate supplied fact");
            assert!(crate::page::task::offered(&crate::page::task::offers(actual), TaskSection::Budget));
        }
    }
}

#[test]
fn owner_goal_read_and_watch_keep_all_ten_counters_absent_clock_and_exact_budget_by_key() {
    let (mut domain, opened) = project_open(ProjectSection::Goals);
    let watch = open_token(&opened);
    let mut row = goal_line(9, u32::MAX, Phase::Running, 9);
    row.changed = skein_lib::Wall::EPOCH;
    row.held_below = u32::MAX;
    row.phases = OwnerPhaseCounts {
        waiting: u32::MAX,
        idle: u32::MAX - 1,
        starting: u32::MAX - 2,
        running: u32::MAX - 3,
        backing_off: u32::MAX - 4,
        closing: u32::MAX - 5,
        held: u32::MAX - 6,
        done: u32::MAX - 7,
        failed: u32::MAX - 8,
        cancelled: u32::MAX - 9,
    };
    row.budget.funder = Funder::Task(u64::MAX);
    row.budget.deadline = Some(skein_lib::Wall::from_nanos(u64::MAX));
    let mut rows = List::with_capacity(1);
    rows.push(row.clone()).unwrap();
    drive(
        &mut domain,
        Time::ZERO,
        Watched { watch, event: WatchEvent::Delivered(Delivery::Tasks(Box::new(Tasks { rows, more: false }))) },
    );
    let mut duplicate = row.clone();
    duplicate.changed = skein_lib::Wall::EPOCH;
    duplicate.phases = OwnerPhaseCounts::ZERO;
    let mut rows = List::with_capacity(2);
    rows.push(duplicate).unwrap();
    rows.push(goal_line(8, 1, Phase::Done, 1)).unwrap();
    drive(
        &mut domain,
        Time::ZERO,
        ReadPage {
            read: read_named(
                &opened,
                &Read::Tasks { query: crate::queries::ended_goals(1, 2), live: None, ended: None },
            ),
            result: ReadResult::Value(ReadValue::Tasks(Box::new(TaskPage {
                live: List::with_capacity(0),
                live_after: None,
                ended: EndedGroup::Rows { rows, after: None, cut: false },
            }))),
        },
    );
    assert_eq!(domain.project_page().listed_goals.get(0), Some(&row));
    assert_eq!(domain.project_page().listed_goals.len(), 2);
}

#[test]
fn the_inbox_shows_both_groups_of_its_filter() {
    let mut domain = Fixture::new(&LIMITS);
    let f = start(&mut domain, List::with_capacity(0));
    delivered(&mut domain, Time::ZERO, f, frame(7));
    for project in [None, Some(3)] {
        drive(&mut domain, Time::ZERO, Event::Went { place: Place::Jig(Address::Inbox { project }) });
        assert_eq!(
            domain.sections(),
            &[
                Section::Inbox { group: InboxReadGroup::Needs, project },
                Section::Inbox { group: InboxReadGroup::Updates, project },
            ],
            "{project:?}"
        );
    }
}
#[test]
fn a_task_address_shows_its_own_section_alone() {
    let mut domain = Fixture::new(&LIMITS);
    let f = start(&mut domain, List::with_capacity(0));
    delivered(&mut domain, Time::ZERO, f, frame(7));
    drive(
        &mut domain,
        Time::ZERO,
        Event::Went { place: Place::Jig(Address::Task { task: 9, section: TaskSection::Plan }) },
    );
    assert_eq!(domain.sections(), &[Section::Task { task: 9, section: TaskSection::Plan }]);
}
#[test]
fn a_page_not_about_the_work_shows_no_section() {
    let mut domain = Fixture::new(&LIMITS);
    assert_eq!(domain.sections(), &[]);
    let f = start(&mut domain, List::with_capacity(0));
    delivered(&mut domain, Time::ZERO, f, frame(7));
    for place in [Place::Elsewhere, Place::Missing] {
        drive(&mut domain, Time::ZERO, Event::Went { place });
        assert_eq!(domain.sections(), &[], "{place:?}");
    }
}

#[test]
fn an_application_page_shows_no_task_after_a_task_page() {
    let mut domain = Fixture::new(&LIMITS);
    let f = start(&mut domain, List::with_capacity(0));
    delivered(&mut domain, Time::ZERO, f, frame(7));
    drive(&mut domain, Time::ZERO, Event::Went { place: Place::Jig(Address::task(9)) });
    assert_eq!(domain.shown_task(), Some(9));
    drive(&mut domain, Time::ZERO, Event::Went { place: Place::Elsewhere });
    assert_eq!(domain.shown_task(), None);
}

#[test]
fn no_two_windows_of_a_page_share_a_token() {
    use crate::page::{conversations::*, project::*, task::*};
    let mut numbers = [
        crate::activity::routing::ACTIVITY_WINDOW,
        INBOX_WINDOW,
        INBOX_UPDATES_WINDOW,
        CONVERSATIONS_WINDOW,
        HISTORY_WINDOW,
        ENDED_WINDOW,
        GOALS_WINDOW,
        NOTES_WINDOW,
        SPEND_WINDOW,
    ];
    numbers.sort_unstable();
    for pair in numbers.windows(2) {
        assert_ne!(pair[0], pair[1], "{numbers:?}");
    }
}
#[test]
fn no_two_reads_of_a_page_share_a_number() {
    use crate::page::{conversations::*, project::*, task::*};
    let mut numbers = [
        crate::activity::routing::ACTIVITY_READ,
        INBOX_NEEDS_READ,
        INBOX_UPDATES_READ,
        CONVERSATIONS_READ,
        HISTORY_READ,
        ENDED_READ,
        PERSON_TASK_READ,
        GOALS_READ,
        NOTES_READ,
        SPEND_READ,
        PROJECT_READ,
        crate::ui::FINAL_TASK_READ,
    ];
    numbers.sort_unstable();
    for pair in numbers.windows(2) {
        assert_ne!(pair[0], pair[1], "{numbers:?}");
    }
}

#[test]
fn the_inbox_opens_its_watch_once_for_both_groups() {
    let mut domain = Fixture::new(&LIMITS);
    let f = start(&mut domain, List::with_capacity(0));
    delivered(&mut domain, Time::ZERO, f, frame(7));
    let requests = drive(&mut domain, Time::ZERO, Event::Went { place: Place::Jig(Address::Inbox { project: None }) });
    let mut inbox_watches = 0_u32;
    for request in &requests {
        if let Request::Client {
            asked: jig_client::Event::Watch { subject: jig_client::Watchable::Jig(Subject::Inbox { .. }), .. },
        } = request
        {
            inbox_watches += 1;
        }
    }
    assert_eq!(inbox_watches, 1);
}

#[test]
fn an_arranged_task_page_lists_every_section_in_order() {
    let mut domain = Fixture::new(&LIMITS);
    domain
        .configure_arrangement(arrangement(
            &[
                TaskSection::Panels,
                TaskSection::History,
                TaskSection::Budget,
                TaskSection::Plan,
                TaskSection::Overview,
                TaskSection::Activity,
            ],
            &[],
        ))
        .unwrap();
    let f = start(&mut domain, List::with_capacity(0));
    delivered(&mut domain, Time::ZERO, f, frame(7));
    let address = Address::Task { task: 8, section: TaskSection::Plan };
    drive(&mut domain, Time::ZERO, Event::Went { place: Place::Jig(address) });
    assert_eq!(domain.address, address);
    assert_eq!(
        domain.sections(),
        &[
            Section::Task { task: 8, section: TaskSection::Overview },
            Section::Task { task: 8, section: TaskSection::Activity },
            Section::Task { task: 8, section: TaskSection::Plan },
            Section::Task { task: 8, section: TaskSection::History },
            Section::Task { task: 8, section: TaskSection::Budget },
            Section::Task { task: 8, section: TaskSection::Panels },
        ]
    );
}
#[test]
fn settings_stand_for_its_five_on_an_arranged_project_page() {
    let rows: [(&str, ProjectSection, &[ProjectSection]); 2] = [
        (
            "address Settings",
            ProjectSection::Settings,
            &[
                ProjectSection::Recurring,
                ProjectSection::Spend,
                ProjectSection::Roles,
                ProjectSection::Policy,
                ProjectSection::Resources,
            ],
        ),
        ("arranged Settings", ProjectSection::Policy, &[ProjectSection::Settings]),
    ];
    for (label, own, sections) in rows {
        let mut domain = Fixture::new(&LIMITS);
        let mut value = arrangement(&[], sections);
        value.project.push(ProjectSection::Goals).unwrap();
        value.project.push(ProjectSection::Overview).unwrap();
        domain.configure_arrangement(value).unwrap();
        let f = start(&mut domain, List::with_capacity(0));
        delivered(&mut domain, Time::ZERO, f, frame(7));
        let address = Address::Project { project: 1, section: own };
        drive(&mut domain, Time::ZERO, Event::Went { place: Place::Jig(address) });
        assert_eq!(domain.address, address, "{label}");
        assert_eq!(
            domain.sections(),
            &[
                Section::Project { project: 1, section: ProjectSection::Overview },
                Section::Project { project: 1, section: ProjectSection::Goals },
                Section::Project { project: 1, section: ProjectSection::Settings },
            ],
            "{label}"
        );
    }
}
#[test]
fn an_arranged_task_section_its_document_does_not_offer_is_not_shown() {
    for section in [TaskSection::Plan, TaskSection::Budget, TaskSection::Panels] {
        for own in [TaskSection::Overview, section] {
            let mut domain = Fixture::new(&LIMITS);
            domain.configure_arrangement(arrangement(&[section], &[])).unwrap();
            let f = start(&mut domain, List::with_capacity(0));
            delivered(&mut domain, Time::ZERO, f, frame(7));
            let opened = drive(
                &mut domain,
                Time::ZERO,
                Event::Went { place: Place::Jig(Address::Task { task: 8, section: own }) },
            );
            let value = Section::Task { task: 8, section };
            assert!(domain.shows(value), "{own:?}, {section:?}: loading");
            let mut document = super::confirmation::task();
            document.chip.executor = Executor::Party { party: 7 };
            document.budget.numbers = OwnerNumbers { budget: 0, spent: 0, spent_below: 0, reserved: 0 };
            drive(
                &mut domain,
                Time::ZERO,
                Watched {
                    watch: open_token(&opened),
                    event: WatchEvent::Delivered(Delivery::Task(Box::new(document))),
                },
            );
            assert!(domain.sections().contains(&value), "{own:?}, {section:?}: fixed list");
            assert!(!domain.shows(value), "{own:?}, {section:?}: document received");
        }
    }
}

fn source_requests(requests: &[Request]) -> (List<Subject>, List<Read>) {
    let mut watches = List::with_capacity(LIMITS.watches);
    let mut reads = List::with_capacity(LIMITS.reads);
    for request in requests {
        if let Request::Client {
            asked: jig_client::Event::Watch { subject: jig_client::Watchable::Jig(subject), .. },
        } = request
        {
            watches.push(subject.clone()).unwrap();
        }
        if let Request::Client { asked: jig_client::Event::Read { read: jig_client::Readable::Jig(what), .. } } =
            request
        {
            reads.push(what.clone()).unwrap();
        }
    }
    (watches, reads)
}
fn arranged_task() -> (Fixture, Box<[Request]>) {
    let mut domain = Fixture::new(&LIMITS);
    domain
        .configure_arrangement(arrangement(
            &[
                TaskSection::Overview,
                TaskSection::Activity,
                TaskSection::Plan,
                TaskSection::History,
                TaskSection::Budget,
                TaskSection::Panels,
            ],
            &[],
        ))
        .unwrap();
    let f = start(&mut domain, List::with_capacity(0));
    let initial = delivered(&mut domain, Time::ZERO, f, frame(7));
    finish_initial_inbox_reads(&mut domain, &initial);
    let opened = drive(&mut domain, Time::ZERO, Event::Went { place: Place::Jig(Address::task(8)) });
    for request in &opened {
        if let Request::Client { asked: jig_client::Event::Unwatch { handle: watch } } = request {
            drive(&mut domain, Time::ZERO, Ended { watch: *watch, end: WatchEnd::Closed });
        }
    }
    domain.reclaim();
    (domain, opened)
}
#[test]
fn an_arranged_task_page_opens_its_watch_once() {
    let rows: [(Executor, Phase, &[Subject], &[Read]); 3] = [
        (
            Executor::Agent { charter: 1 },
            Phase::Running,
            &[Subject::Tree { task: 8, window: LIMITS.rows }, Subject::Run { task: 8, turn: None, offset: 0 }],
            &[Read::Activity { task: 8, before: None }],
        ),
        (
            Executor::Party { party: 7 },
            Phase::Held,
            &[Subject::Tree { task: 8, window: LIMITS.rows }],
            &[Read::PersonTask { task: 8 }],
        ),
        (
            Executor::Party { party: 7 },
            Phase::Done,
            &[],
            &[Read::Ended { task: 8, after: None }, Read::PersonTask { task: 8 }],
        ),
    ];
    for (executor, phase, watches, reads) in rows {
        let (mut domain, opened) = arranged_task();
        let (actual_watches, actual_reads) = source_requests(&opened);
        assert_eq!(actual_watches.as_slice(), &[Subject::Task { task: 8 }], "{executor:?}");
        assert_eq!(actual_reads.as_slice(), &[Read::History { task: 8, subtree: false, after: None }], "{executor:?}");
        let mut document = super::confirmation::task();
        document.chip.executor = executor;
        document.chip.phase = phase;
        document.context.as_mut().unwrap().impact.tasks.waiting = 1;
        let delivery =
            Watched { watch: open_token(&opened), event: WatchEvent::Delivered(Delivery::Task(Box::new(document))) };
        let requests = drive(&mut domain, Time::ZERO, delivery.clone());
        let (actual_watches, actual_reads) = source_requests(&requests);
        assert_eq!(actual_watches.as_slice(), watches, "{executor:?}");
        assert_eq!(actual_reads.as_slice(), reads, "{executor:?}");
        let again = drive(&mut domain, Time::ZERO, delivery);
        let (actual_watches, actual_reads) = source_requests(&again);
        assert_eq!(actual_watches.as_slice(), &[], "{executor:?}: repeated delivery");
        assert_eq!(actual_reads.as_slice(), &[], "{executor:?}: repeated delivery");
    }
}
#[test]
fn an_arranged_project_page_opens_the_goals_watch_once() {
    let mut domain = Fixture::new(&LIMITS);
    domain.configure_arrangement(arrangement(&[], &[ProjectSection::Goals, ProjectSection::Overview])).unwrap();
    let f = start(&mut domain, List::with_capacity(0));
    let mut me = frame(7);
    me.projects = super::super::work_pages::list([project(1, true)]);
    let initial = delivered(&mut domain, Time::ZERO, f, me);
    finish_initial_inbox_reads(&mut domain, &initial);
    let requests = drive(
        &mut domain,
        Time::ZERO,
        Event::Went { place: Place::Jig(Address::Project { project: 1, section: ProjectSection::Overview }) },
    );
    let (watches, reads) = source_requests(&requests);
    assert_eq!(
        watches.as_slice(),
        &[
            Subject::Tasks { query: crate::queries::goals(1), window: LIMITS.rows },
            Subject::Inbox { window: LIMITS.rows }
        ]
    );
    assert_eq!(
        reads.as_slice(),
        &[
            Read::Project { project: 1 },
            Read::Tasks { query: crate::queries::ended_goals(1, 2), live: None, ended: None }
        ]
    );
}
#[test]
fn an_arranged_task_document_opens_its_parts_once() {
    let (mut domain, opened) = arranged_task();
    let mut document = super::confirmation::task();
    let resource = Part::Resource { connector: 2, path: resource_path(b"resource") };
    let panel = Part::Panel { connector: 3, task: 8 };
    document.resources = List::with_capacity(1);
    document.resources.push(TaskResource::Part(resource.clone())).unwrap();
    let context = document.context.as_mut().unwrap();
    context.parts = List::with_capacity(1);
    context.parts.push(panel.clone()).unwrap();
    let requests = drive(
        &mut domain,
        Time::ZERO,
        Watched { watch: open_token(&opened), event: WatchEvent::Delivered(Delivery::Task(Box::new(document))) },
    );
    let parts = crate::tests::fixtures::parts(&requests, LIMITS.parts);
    assert_eq!(
        parts.as_ref(),
        &[
            Request::ShowPart { part: resource, scope: PartScope::Project(1) },
            Request::ShowPart { part: panel, scope: PartScope::Project(1) },
        ]
    );
}

#[test]
fn an_object_no_source_names_is_owed_while_deciding() {
    let mut domain = Fixture::new(&LIMITS);
    let source = crate::objects::Source::Window { number: 0 };
    let id = domain.objects.name(ObjectKey::Task(8), source).unwrap();
    domain.objects.get_mut(id).unwrap().card =
        crate::objects::Card::Deciding { request: None, progress: crate::intents::Progress::Confirming };
    domain.objects.replace(source, &[]);
    assert!(domain.owes(id));
}

fn application_page() -> Fixture {
    let mut domain = Fixture::new(&LIMITS);
    let f = start(&mut domain, List::with_capacity(0));
    let initial = delivered(&mut domain, Time::ZERO, f, frame(7));
    finish_initial_inbox_reads(&mut domain, &initial);
    let left = drive(&mut domain, Time::ZERO, Event::Went { place: Place::Elsewhere });
    for request in &left {
        if let Request::Client { asked: jig_client::Event::Unwatch { handle: watch } } = request {
            drive(&mut domain, Time::ZERO, Ended { watch: *watch, end: WatchEnd::Closed });
        }
    }
    domain.reclaim();
    domain
}
fn show_sections(domain: &mut Fixture, values: &[Section]) -> Box<[Request]> {
    let mut sections = List::with_capacity(u32::try_from(values.len()).unwrap());
    for section in values {
        sections.push(*section).unwrap();
    }
    let source = crate::objects::Source::Window { number: crate::application::WINDOW };
    let mut objects = List::with_capacity(LIMITS.objects);
    for (key, _) in domain.objects().ids() {
        if domain.objects().named(*key, source) {
            objects.push(*key).unwrap();
        }
    }
    drive(domain, Time::ZERO, Event::Named { objects, sections })
}
#[test]
fn an_application_page_naming_a_tasks_plan_opens_its_watch() {
    let mut domain = application_page();
    let opened = show_sections(&mut domain, &[Section::Task { task: 8, section: TaskSection::Plan }]);
    let (watches, reads) = source_requests(&opened);
    assert_eq!(watches.as_slice(), &[Subject::Task { task: 8 }]);
    assert_eq!(reads.as_slice(), &[]);
    let id = open_token(&opened);
    assert_eq!(domain.handles.watches.get(&id).unwrap().owner, crate::handles::Owner::Sections);
}
#[test]
fn naming_objects_anew_never_closes_a_sections_watch() {
    let mut domain = application_page();
    let opened = show_sections(&mut domain, &[Section::Task { task: 8, section: TaskSection::Plan }]);
    let watch = open_token(&opened);
    for keys in [&[ObjectKey::Task(8)][..], &[][..]] {
        let mut objects = List::with_capacity(u32::try_from(keys.len()).unwrap());
        for key in keys {
            objects.push(*key).unwrap();
        }
        let mut sections = List::with_capacity(1);
        sections.push(Section::Task { task: 8, section: TaskSection::Plan }).unwrap();
        let requests = drive(&mut domain, Time::ZERO, Event::Named { objects, sections });
        assert!(!requests.contains(&Request::Client { asked: jig_client::Event::Unwatch { handle: watch } }));
    }
    assert!(
        !show_sections(&mut domain, &[Section::Task { task: 8, section: TaskSection::Plan }])
            .contains(&Request::Client { asked: jig_client::Event::Unwatch { handle: watch } })
    );
}
#[test]
fn a_section_naming_a_second_task_is_not_shown() {
    let mut domain = application_page();
    let first = Section::Task { task: 8, section: TaskSection::Plan };
    let second = Section::Task { task: 9, section: TaskSection::Plan };
    show_sections(&mut domain, &[first, second]);
    assert_eq!(domain.sections(), &[first]);
    assert!(!domain.shows(second));
}
#[test]
fn an_empty_list_closes_every_sections_watch() {
    let mut domain = application_page();
    let opened = show_sections(&mut domain, &[Section::Task { task: 8, section: TaskSection::Plan }]);
    assert_eq!(
        show_sections(&mut domain, &[]).as_ref(),
        &[Request::Client { asked: jig_client::Event::Unwatch { handle: open_token(&opened) } }]
    );
}

#[test]
fn naming_sections_anew_never_closes_an_objects_watch() {
    let mut domain = application_page();
    let mut objects = List::with_capacity(1);
    objects.push(ObjectKey::Task(8)).unwrap();
    let opened = drive(&mut domain, Time::ZERO, Event::Named { objects, sections: List::with_capacity(0) });
    let watch = open_token(&opened);
    show_sections(&mut domain, &[Section::Task { task: 8, section: TaskSection::Plan }]);
    assert!(
        !show_sections(&mut domain, &[])
            .contains(&Request::Client { asked: jig_client::Event::Unwatch { handle: watch } })
    );
}
#[test]
fn replacing_sections_keeps_their_shared_subject_watch() {
    let mut domain = application_page();
    show_sections(&mut domain, &[Section::Task { task: 8, section: TaskSection::Overview }]);
    let requests = show_sections(&mut domain, &[Section::Task { task: 8, section: TaskSection::Plan }]);
    assert_eq!(requests.as_ref(), &[]);
}
#[test]
fn naming_the_same_sections_opens_nothing_again() {
    let mut domain = application_page();
    let sections = [Section::Task { task: 8, section: TaskSection::Plan }];
    show_sections(&mut domain, &sections);
    assert_eq!(show_sections(&mut domain, &sections).as_ref(), &[]);
}
#[test]
fn leaving_an_application_page_closes_its_sections() {
    let mut domain = application_page();
    let opened = show_sections(&mut domain, &[Section::Task { task: 8, section: TaskSection::Plan }]);
    let requests = drive(&mut domain, Time::ZERO, Event::Went { place: Place::Elsewhere });
    assert_eq!(
        requests.as_ref(),
        &[Request::Client { asked: jig_client::Event::Unwatch { handle: open_token(&opened) } }, Request::PartsLeft]
    );
}

#[test]
fn a_section_whose_task_is_not_allowed_says_it_is_missing() {
    let mut domain = application_page();
    let section = Section::Task { task: 8, section: TaskSection::Plan };
    let opened = show_sections(&mut domain, &[section]);
    drive(&mut domain, Time::ZERO, Ended { watch: open_token(&opened), end: WatchEnd::NotAllowed });
    assert_eq!(
        domain.section_status(section),
        Some(crate::page::SectionStatus { behind: false, excess: 0, refused: None, missing: true })
    );
    assert_eq!(domain.page(), &Page::Elsewhere);
}
#[test]
fn a_sections_dropped_watch_shows_that_section_behind() {
    let mut domain = application_page();
    let section = Section::Task { task: 8, section: TaskSection::Plan };
    let opened = show_sections(&mut domain, &[section, Section::Deployment]);
    drive(&mut domain, Time::ZERO, Ended { watch: open_token(&opened), end: WatchEnd::Dropped });
    let clear = crate::page::SectionStatus { behind: false, excess: 0, refused: None, missing: false };
    assert_eq!(domain.section_status(section), Some(crate::page::SectionStatus { behind: true, ..clear }));
    assert_eq!(domain.section_status(Section::Deployment), Some(clear));
}
#[test]
fn a_sections_missing_read_keeps_the_application_page() {
    let mut domain = application_page();
    let section = Section::Project { project: 1, section: ProjectSection::Goals };
    let opened = show_sections(&mut domain, &[section]);
    let mut project_read = None;
    for request in &opened {
        if let Request::Client {
            asked: jig_client::Event::Read { handle: read, read: jig_client::Readable::Jig(Read::Project { .. }) },
        } = request
        {
            project_read = Some(*read);
        }
    }
    let read = project_read.unwrap();
    drive(&mut domain, Time::ZERO, ReadPage { read, result: ReadResult::None(ReadNone::Unknown) });
    assert_eq!(
        domain.section_status(section),
        Some(crate::page::SectionStatus { behind: false, excess: 0, refused: None, missing: true })
    );
    assert_eq!(domain.page(), &Page::Elsewhere);
}

fn section_with_an_unrelated_section() -> (Fixture, Section) {
    let mut domain = application_page();
    let section = Section::Task { task: 8, section: TaskSection::Plan };
    show_sections(&mut domain, &[section, Section::Deployment]);
    crate::page::shown::context_subject(&mut domain, &Subject::Tree { task: 8, window: LIMITS.rows });
    (domain, section)
}
#[test]
fn a_sections_limit_refusal_stays_with_that_section() {
    let (mut domain, section) = section_with_an_unrelated_section();
    crate::page::routing::refuse(&mut domain, Limit::Reads);
    assert_eq!(domain.section_status(section).unwrap().refused, Some(Limit::Reads));
    assert_eq!(domain.section_status(Section::Deployment).unwrap().refused, None);
}
#[test]
fn a_sections_excess_rows_stay_with_that_section() {
    let (mut domain, section) = section_with_an_unrelated_section();
    crate::page::routing::excess(&mut domain, 3);
    assert_eq!(domain.section_status(section).unwrap().excess, 3);
    assert_eq!(domain.section_status(Section::Deployment).unwrap().excess, 0);
}

mod task_plan;
use task_plan::task_plan_keeps_folded_held_counts_and_filters_without_losing_rows;
