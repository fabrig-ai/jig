use super::spine::{LIMITS, drain, frame, open_token};
use crate::tests::fixtures::refusal_answer;
use crate::tests::fixtures::{AnsweredByEngine, Fixture, ReadPage, Watched};
use crate::{
    Event, Limits, Request,
    action::Act,
    address::{Address, Place},
    fields::Purpose,
    intents::send_card,
    notices::kept::{self, Kind},
    objects::Source,
    store::{self, Record, STORE_VERSION},
};
use jig_client::contract::*;
use skein_lib::{Duration, Env, List, Queue, Time, Wall};
fn limits() -> Limits {
    Limits { ..LIMITS }
}
fn drive(
    domain: &mut Fixture,
    limits: &Limits,
    now: Time,
    event: impl crate::tests::fixtures::Input,
) -> Box<[Request]> {
    domain.ui.limits = *limits;
    domain.drive(now, event)
}
fn tick(domain: &mut Fixture, limits: &Limits, now: Time) -> Box<[Request]> {
    let mut out = Queue::uncapped();
    crate::fire(&mut domain.ui, &Env { now, wall: Wall::EPOCH, limits: *limits }, &mut out, &domain.client);
    drain(out)
}
fn start(domain: &mut Fixture, limits: &Limits, kept: List<Record>) -> (jig_client::Handle, jig_client::Handle) {
    let f = open_token(&drive(
        domain,
        limits,
        Time::ZERO,
        Event::Start { seed: 8, place: Place::Jig(Address::Inbox { project: None }), records: kept },
    ));
    let mut source = frame(7);
    source.party.login = DocumentText::complete(text(b"L"));
    let initial = drive(
        domain,
        limits,
        Time::ZERO,
        Watched { watch: f, event: WatchEvent::Delivered(Delivery::Me(Box::new(source))) },
    );
    let page = open_token(&initial);
    for request in &initial {
        if let Request::Client {
            asked: jig_client::Event::Read { handle: read, read: jig_client::Readable::Jig(Read::Inbox { .. }) },
        } = request
        {
            drive(
                domain,
                limits,
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
    (f, page)
}
fn text(bytes: &[u8]) -> Text {
    Text::new(Box::from(bytes), 32).unwrap()
}

#[test]
fn a_minute_counts_a_change_only_while_the_page_shows_wall_times() {
    let limits = limits();
    let mut domain = Fixture::new(&limits);
    start(&mut domain, &limits, List::with_capacity(0));
    assert_eq!(domain.next_deadline(), None);
    domain.timed_page(true);
    assert_eq!(domain.next_deadline(), Some(Time::from_nanos(Duration::from_secs(60).as_nanos())));
    let shown = domain.shown();
    assert!(tick(&mut domain, &limits, Time::from_nanos(Duration::from_secs(59).as_nanos())).is_empty());
    assert_eq!(domain.shown(), shown);
    assert!(tick(&mut domain, &limits, Time::from_nanos(Duration::from_secs(60).as_nanos())).is_empty());
    assert_eq!(domain.shown(), shown.checked_add(1).unwrap());
    drive(
        &mut domain,
        &limits,
        Time::from_nanos(Duration::from_secs(60).as_nanos()),
        Event::Went { place: Place::Elsewhere },
    );
    let shown = domain.shown();
    assert!(tick(&mut domain, &limits, Time::from_nanos(Duration::from_secs(120).as_nanos())).is_empty());
    assert_eq!(domain.shown(), shown);
}

#[test]
fn hidden_answers_have_notices_and_lingers_and_coalesced_saves_end_on_deadlines() {
    let limits = limits();
    let mut domain = Fixture::new(&limits);
    start(&mut domain, &limits, List::with_capacity(0));
    let key = ObjectKey::Task(8);
    domain.objects.name(key, Source::Window { number: 1 }).unwrap();
    let mut out = Queue::uncapped();
    let request = send_card(
        &mut domain.ui,
        key,
        ActKind::Stop,
        Box::new(Ask::Stop { project: 1, task: 8 }),
        &mut out,
        &domain.client,
    )
    .unwrap();
    drive(&mut domain, &limits, Time::ZERO, Event::Went { place: Place::Elsewhere });
    drive(&mut domain, &limits, Time::ZERO, AnsweredByEngine { request, answer: refusal_answer(Refusal::NotOffered) });
    let notice = *domain.notices().ids().first().unwrap().0;
    let Kind::Answer { object, .. } = &domain.notices().get(notice).unwrap().kind else {
        panic!("hidden card outcome");
    };
    assert_eq!(*object, key);
    assert_eq!(domain.notices().get(notice).unwrap().until, Time::from_nanos(Duration::from_secs(5).as_nanos()));
    let id = domain.objects.name(ObjectKey::Task(9), Source::Window { number: 1 }).unwrap();
    assert!(domain.leave(id, Ending::Cancelled { by: Actor::Party(7), reason: Some(text(b"done")), at: None }));
    out = Queue::uncapped();
    assert!(store::coalesce(
        &mut domain,
        Record::Draft { version: STORE_VERSION, party: 7, task: 8, words: text(b"first"), touched: 1 },
        &mut out
    ));
    drive(&mut domain, &limits, Time::from_nanos(500_000_000), crate::tests::fixtures::Tick);
    let newest = Record::Draft { version: STORE_VERSION, party: 7, task: 8, words: text(b"newest"), touched: 1 };
    assert!(store::coalesce(&mut domain, newest.clone(), &mut out));
    assert!(out.is_empty());
    assert!(tick(&mut domain, &limits, Time::from_nanos(1_499_999_999)).is_empty());
    assert_eq!(
        tick(&mut domain, &limits, Time::from_nanos(1_500_000_000)).as_ref(),
        &[Request::Save { records: crate::tests::fixtures::one(newest) }]
    );
    tick(&mut domain, &limits, Time::from_nanos(5_000_000_000));
    tick(&mut domain, &limits, Time::from_nanos(5_000_000_000));
    assert!(domain.notices().ids().is_empty());
    assert!(domain.objects().find(ObjectKey::Task(9)).is_none());
}

#[test]
fn full_containers_refuse_new_objects_intents_fields_and_notices() {
    let limits = full_limits();
    check_valid(limits);
    let mut domain = Fixture::new(&limits);
    let mut kept = List::with_capacity(1);
    kept.push(Record::Draft { version: STORE_VERSION, party: 7, task: 8, words: text(b"kept"), touched: 1 }).unwrap();
    let (f, page) = start(&mut domain, &limits, kept);
    domain.request_read(Source::Window { number: 1 }, Read::Deployment).unwrap();
    let mut entries = List::with_capacity(2);
    for task in 1..=2 {
        entries
            .push(InboxEntry {
                reply: None,
                lineage: List::with_capacity(0),
                goal: None,
                recipient: None,
                stall: None,
                object: ObjectKey::Result(task),
                project: 1,
                group: InboxGroup::Results,
                position: task,
                at: Wall::EPOCH,
            })
            .unwrap();
    }
    let inbox = Inbox {
        entries,
        needs: Needs { questions: 0, proposals: 0, escalations: 0, person_tasks: 0 },
        updates: Count { value: 0, limited: false },
        results_seen: 0,
        replies_seen: List::with_capacity(0),
        more: true,
    };
    let effects = drive(
        &mut domain,
        &limits,
        Time::ZERO,
        Watched { watch: page, event: WatchEvent::Delivered(Delivery::Inbox(Box::new(inbox))) },
    );
    assert_eq!(effects.len(), 0);
    let crate::page::Page::Work { excess: 1, refused: Some(Limit::Objects), .. } = domain.page() else {
        panic!("full kept the first and counted the rest");
    };
    for (number, outputs) in [(50, 1), (51, 0)] {
        assert_eq!(
            drive(
                &mut domain,
                &limits,
                Time::ZERO,
                crate::tests::fixtures::intent(7, number, Ask::Stop { project: 1, task: 8 })
            )
            .len(),
            outputs
        );
    }
    drive(
        &mut domain,
        &limits,
        Time::ZERO,
        Event::Act { action: Act::Edit { field: Purpose::NewChat, text: text(b"one") } },
    );
    assert_eq!(
        drive(
            &mut domain,
            &limits,
            Time::ZERO,
            Event::Act { action: Act::Edit { field: Purpose::Reason, text: text(b"two") } }
        )
        .len(),
        0
    );
    kept::show(&mut domain, Kind::Refused(Limit::Requests));
    kept::show(&mut domain, Kind::Refused(Limit::Fields));
    assert_eq!(domain.notices().ids().len(), 1);
    let mut oversized = frame(7);
    oversized.projects = List::with_capacity(2);
    assert_eq!(
        drive(
            &mut domain,
            &limits,
            Time::ZERO,
            Watched { watch: f, event: WatchEvent::Delivered(Delivery::Me(Box::new(oversized))) }
        )
        .len(),
        0
    );
    drive(&mut domain, &limits, Time::ZERO, Event::Went { place: Place::Jig(Address::Deployment) });
}

fn check_valid(limits: Limits) {
    for invalid in [
        Limits { watches: 0, ..limits },
        Limits { objects: u32::MAX, ..limits },
        Limits { notices: u32::MAX, ..limits },
        Limits { notice_life: Duration::ZERO, ..limits },
        Limits { save_delay: Duration::ZERO, ..limits },
    ] {
        assert!(!crate::valid(&invalid));
    }
}

fn full_limits() -> Limits {
    Limits {
        watches: 2,
        objects: 1,
        sources: 1,
        reads: 1,
        intents: 1,
        fields: 1,

        notices: 1,
        records: 1,
        projects: 1,
        roles: 1,
        parts: 1,
        text: 4,
        ..limits()
    }
}
