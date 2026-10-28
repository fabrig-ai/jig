use crate::tests::fixtures::{Fixture, Served, Watched};
use crate::{Event, Limits, Request, max_out};
use crate::{
    address::{Address, Place},
    page::Page,
    store::Record,
};
use jig_client::Link;
use jig_client::contract::{Count, Delivery, DocumentText, HeldRole, Inbox, Me, MeParty, Needs, PartyKind, WatchEvent};
use skein_lib::{Duration, Env, List, Queue, Time, Wall};

pub(crate) const LIMITS: Limits = Limits {
    watches: 4,
    notices: 2,
    notice_life: Duration::from_secs(5),
    linger: Duration::from_secs(5),
    save_delay: Duration::from_secs(1),
    intents: 2,
    fields: 4,

    objects: 2,
    sources: 8,
    reads: 4,
    parts: 8,
    delegates: 2,
    document_bytes: 65536,
    records: 2,
    kept: 4,
    drafts: 1,
    projects: 2,
    roles: 2,
    text: 32,
    rows: 2,
    segments: 4,
    accounts: jig_client::AccountCounts { accounts: 8, sources: 8, sign_ins: 8 },
    conversation_words: 2,
    navigation_entries: 8,
    updates_count: 99,
};
pub(crate) fn env(now: Time) -> Env<Limits> {
    Env { now, wall: Wall::EPOCH, limits: LIMITS }
}
pub(super) fn drain(mut out: Queue<Request>) -> Box<[Request]> {
    let mut rows = List::with_capacity(max_out(&LIMITS));
    while let Some(request) = out.pop() {
        rows.push(request).expect("output fits declared bound");
    }
    rows.into_boxed()
}
pub(super) fn drive(domain: &mut Fixture, now: Time, event: impl crate::tests::fixtures::Input) -> Box<[Request]> {
    domain.drive(now, event)
}
pub(super) fn open_token(requests: &[Request]) -> jig_client::Handle {
    for request in requests {
        if let Request::Client { asked: jig_client::Event::Watch { handle: watch, .. } } = request {
            return *watch;
        }
    }
    unreachable!("a watch was opened")
}
fn needs() -> Needs {
    Needs { questions: 0, proposals: 0, escalations: 0, person_tasks: 0 }
}
pub(crate) fn frame(party: u64) -> Me {
    Me {
        providers: List::with_capacity(0),
        providers_cut: false,
        key_bytes: 256,
        redirect_bytes: 4096,
        words_bytes: 65_536,
        amendment_bytes: 65_536,
        party: MeParty {
            number: party,
            kind: PartyKind::Person,
            login: DocumentText::complete(jig_client::contract::Text::new(Box::from(*b"fixture-login"), 32).unwrap()),
            name: DocumentText::complete(jig_client::contract::Text::new(Box::from(*b"Pat"), 32).unwrap()),
        },
        projects: List::with_capacity(0),
        needs: needs(),
        needs_total: 0,
        results_seen: 0,
        cut: false,
        updates: Count { value: 0, limited: false },

        retention: Duration::from_secs(1000),
        deployment_role: Some(HeldRole::Policy { role: 1 }),
    }
}
pub(crate) fn start(domain: &mut Fixture, kept: List<Record>) -> jig_client::Handle {
    open_token(&drive(
        domain,
        Time::ZERO,
        Event::Start { seed: 8, place: Place::Jig(Address::Inbox { project: None }), records: kept },
    ))
}
pub(super) fn delivered(domain: &mut Fixture, now: Time, watch: jig_client::Handle, frame: Me) -> Box<[Request]> {
    drive(domain, now, Watched { watch, event: WatchEvent::Delivered(Delivery::Me(Box::new(frame))) })
}
fn inbox() -> Inbox {
    Inbox {
        entries: List::with_capacity(0),
        needs: needs(),
        updates: Count { value: 0, limited: false },
        results_seen: 0,
        replies_seen: List::with_capacity(0),
        more: false,
    }
}

#[test]
fn start_opens_me_before_any_other_watch() {
    let mut domain = Fixture::new(&LIMITS);
    assert_eq!(domain.frame().party(), None);
    let opened = drive(
        &mut domain,
        Time::ZERO,
        Event::Start { seed: 8, place: Place::Jig(Address::Deployment), records: List::with_capacity(0) },
    );
    assert_eq!(opened.len(), 1);
    let Request::Client { asked: jig_client::Event::Watch { subject: jig_client::Watchable::Jig(subject), .. } } =
        &opened[0]
    else {
        unreachable!("start opens a watch");
    };
    assert_eq!(*subject, jig_client::contract::Subject::Me);
    assert_eq!(domain.page(), &Page::Starting);
    assert_eq!(domain.link(), Link::Starting);
}

#[test]
fn a_reopened_page_is_behind_until_its_first_delivery_and_never_changes_the_link() {
    let mut domain = Fixture::new(&LIMITS);
    let watch = start(&mut domain, List::with_capacity(0));
    let page = open_token(&delivered(&mut domain, Time::ZERO, watch, frame(7)));
    assert!(
        drive(
            &mut domain,
            Time::ZERO,
            crate::tests::fixtures::Ended { watch: page, end: jig_client::contract::WatchEnd::Dropped }
        )
        .is_empty()
    );
    assert_eq!(domain.link(), Link::Live);
    assert_eq!(
        domain.page(),
        &Page::Work { address: Address::Inbox { project: None }, behind: true, excess: 0, refused: None }
    );
    domain.reopen(page);
    let retry = Time::ZERO.saturating_add(Duration::from_secs(1));
    assert!(drive(&mut domain, retry, Watched { watch: page, event: WatchEvent::Opened }).is_empty());
    assert_eq!(
        domain.page(),
        &Page::Work { address: Address::Inbox { project: None }, behind: true, excess: 0, refused: None }
    );
    drive(
        &mut domain,
        retry,
        Watched { watch: page, event: WatchEvent::Delivered(Delivery::Inbox(Box::new(inbox()))) },
    );
    assert_eq!(
        domain.page(),
        &Page::Work { address: Address::Inbox { project: None }, behind: false, excess: 0, refused: None }
    );
}

#[test]
fn a_different_party_keeps_the_previous_partys_work_hidden_until_they_return() {
    let mut kept = List::with_capacity(1);
    kept.push(Record::Place { version: crate::store::STORE_VERSION, party: 7, address: Address::Deployment }).unwrap();
    let mut domain = Fixture::new(&LIMITS);
    let watch = start(&mut domain, kept);
    let refused = delivered(&mut domain, Time::ZERO, watch, frame(8));
    assert_eq!(refused.as_ref(), &[]);
    assert_eq!(domain.page(), &Page::SomeoneElse);
    delivered(&mut domain, Time::ZERO, watch, frame(7));
    assert_eq!(domain.frame().party(), Some(7));
    assert_eq!(
        domain.page(),
        &Page::Work { address: Address::Inbox { project: None }, behind: false, excess: 0, refused: None }
    );
}

#[test]
fn an_unsupported_version_parks_work_and_never_sends_a_new_engine_request() {
    let mut domain = Fixture::new(&LIMITS);
    let watch = start(&mut domain, List::with_capacity(0));
    let refused = drive(&mut domain, Time::ZERO, Served { served: false });
    assert_eq!(
        refused.as_ref(),
        &[Request::PartsLeft, Request::Client { asked: jig_client::Event::Unwatch { handle: watch } }]
    );
    assert_eq!(domain.page(), &Page::OutOfDate);
    assert_eq!(domain.next_deadline(), None);
    assert!(drive(&mut domain, Time::ZERO, Event::Went { place: Place::Jig(Address::Deployment) }).is_empty());
    assert!(delivered(&mut domain, Time::ZERO, watch, frame(7)).is_empty());
}

#[test]
fn foreign_requests_hide_work_until_me_confirms_the_party() {
    let mut domain = restored(
        &[(1, 7), (2, 7)],
        List::with_capacity(0),
        &Limits { intents: LIMITS.kept, conversation_words: LIMITS.kept, ..LIMITS },
    )
    .0;
    let watch = domain.frame_watch.unwrap();
    delivered(&mut domain, Time::ZERO, watch, frame(8));
    assert_eq!(domain.frame().party(), Some(8));
    assert_eq!(domain.page(), &Page::SomeoneElse);
    drive(&mut domain, Time::ZERO, jig_client::Up::NotKept { key: [1; 16] });
    assert_eq!(domain.page(), &Page::SomeoneElse);
}

#[test]
fn the_last_foreign_kept_request_retiring_restores_the_work_page() {
    let mut domain = restored(
        &[(1, 7), (2, 7), (3, 9)],
        List::with_capacity(0),
        &Limits { intents: LIMITS.kept, conversation_words: LIMITS.kept, ..LIMITS },
    )
    .0;
    let watch = domain.frame_watch.unwrap();
    // All three restored keys must retire before work returns.
    delivered(&mut domain, Time::ZERO, watch, frame(8));
    drive(&mut domain, Time::ZERO, jig_client::Up::NotKept { key: [1; 16] });
    drive(&mut domain, Time::ZERO, jig_client::Up::NotKept { key: [2; 16] });
    assert_eq!(domain.page(), &Page::SomeoneElse);
    drive(&mut domain, Time::ZERO, jig_client::Up::NotKept { key: [3; 16] });
    assert!(work_page(&domain));
    assert!(drive(&mut domain, Time::ZERO, jig_client::Up::NotKept { key: [3; 16] }).is_empty());
}

#[test]
fn own_kept_requests_do_not_hide_work_or_override_kept_draft_guard() {
    let mut domain = restored(
        &[(1, 7), (2, 7)],
        List::with_capacity(0),
        &Limits { intents: LIMITS.kept, conversation_words: LIMITS.kept, ..LIMITS },
    )
    .0;
    let watch = domain.frame_watch.unwrap();
    delivered(&mut domain, Time::ZERO, watch, frame(7));
    assert!(work_page(&domain));
    drive(&mut domain, Time::ZERO, jig_client::Up::NotKept { key: [1; 16] });
    assert!(work_page(&domain));

    let mut kept = List::with_capacity(1);
    kept.push(Record::Draft {
        version: crate::store::STORE_VERSION,
        party: 8,
        task: 9,
        words: jig_client::contract::Text::new(Box::from(*b"old"), LIMITS.text).unwrap(),
        touched: 1,
    })
    .unwrap();
    let mut guarded =
        restored(&[(1, 7)], kept, &Limits { intents: LIMITS.kept, conversation_words: LIMITS.kept, ..LIMITS }).0;
    let watch = guarded.frame_watch.unwrap();
    delivered(&mut guarded, Time::ZERO, watch, frame(7));
    drive(&mut guarded, Time::ZERO, jig_client::Up::NotKept { key: [1; 16] });
    assert_eq!(guarded.page(), &Page::SomeoneElse);
}

fn work_page(domain: &Fixture) -> bool {
    match domain.page() {
        Page::Work { .. } => true,
        Page::Starting | Page::Elsewhere | Page::SigningIn | Page::Missing | Page::OutOfDate | Page::SomeoneElse => {
            false
        }
    }
}

pub(super) fn party(number: u64) -> jig_client::contract::Party {
    jig_client::contract::Party {
        number,
        kind: PartyKind::Person,
        name: jig_client::contract::Text::new(Box::from(*b"Pat"), 32).unwrap(),
    }
}

pub(super) fn restored(rows: &[(u8, u64)], records: List<Record>, limits: &Limits) -> (Fixture, jig_client::Handle) {
    let mut kept = List::with_capacity(u32::try_from(rows.len()).unwrap());
    for (number, party) in rows {
        kept.push(crate::tests::fixtures::kept(*number, *party)).unwrap();
    }
    let fixture = Fixture::restored(limits, records, kept);
    let handle = fixture.frame_watch.unwrap();
    (fixture, handle)
}
