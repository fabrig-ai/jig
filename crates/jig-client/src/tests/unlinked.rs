use super::fixtures::{self, Client, PARTY, STOP, TASK};
use crate::*;
use skein_lib::{Duration, List, Queue, Token, Wall};
const ME_WATCH: Token = Token::new(1);
const ENGINE_KEY: skein_lib::RequestKey = [1; 16];
const RETENTION: Duration = Duration::from_secs(60);
fn new() -> Client {
    let limits = fixtures::limits();
    let bound = max_out(&limits);
    Client {
        machine: crate::Client::unlinked(&limits, 7).unwrap(),
        out: Queue::with_capacity(bound),
        requests: List::with_capacity(bound),
    }
}
fn client() -> Client {
    let mut c = new();
    c.step(Event::StartUnlinked { kept: List::with_capacity(0) });
    c.step(Event::Up {
        up: Up::Served { served: true, versions: Some(contract::VersionRange { first: 19, last: 19 }) },
    });
    c.step(Event::SignIn { with: With::Transport });
    c
}
fn live() -> Client {
    let mut c = client();
    c.step(Event::Up {
        up: Up::Watched {
            watch: ME_WATCH,
            event: contract::WatchEvent::Delivered(contract::Delivery::Me(Box::new(contract::Me {
                providers: List::with_capacity(0),
                providers_cut: false,
                key_bytes: 256,
                redirect_bytes: 4096,
                words_bytes: 65_536,
                amendment_bytes: 65_536,
                party: contract::MeParty {
                    number: PARTY,
                    kind: contract::PartyKind::Person,
                    login: contract::DocumentText::complete(contract::Text::new(Box::new([]), 0).unwrap()),
                    name: contract::DocumentText::complete(contract::Text::new(Box::new([]), 0).unwrap()),
                },
                projects: List::with_capacity(0),
                needs: contract::Needs { questions: 0, proposals: 0, escalations: 0, person_tasks: 0 },
                needs_total: 0,
                updates: contract::Count { value: 0, limited: false },
                results_seen: 0,
                cut: false,
                retention: RETENTION,
                deployment_role: None,
            }))),
        },
    });
    c
}
#[test]
fn an_unlinked_client_gives_its_watch_as_the_domain_s_opening() {
    let c = client();
    let [
        Request::Told { told: Told::Link { link: Link::Starting } },
        Request::Down { down: Down::Open { watch, subject } },
    ] = c.requests()
    else {
        panic!("one opening: {:?}", c.requests());
    };
    assert_eq!((*watch, subject), (ME_WATCH, &Watchable::Jig(contract::Subject::Me)));
}
#[test]
fn an_unlinked_client_takes_an_answer_as_the_domain_s() {
    for what in
        [contract::Read::Project { project: fixtures::PROJECT }, contract::Read::Turn { task: 7, attempt: 44, turn: 3 }]
    {
        let mut c = live();
        c.step(Event::Read { handle: TASK, read: Readable::Jig(what) });
        let [Request::Down { down: Down::Read { read, .. } }] = c.requests() else {
            panic!("one read: {:?}", c.requests());
        };
        let read = *read;
        let page = Readout::Jig(contract::ReadResult::None(contract::ReadNone::Unknown));
        c.step(Event::Up { up: Up::ReadDone { read, result: page.clone() } });
        let [Request::Told { told: Told::Page { handle, page: told_page } }] = c.requests() else {
            panic!("one page: {:?}", c.requests());
        };
        assert_eq!((*handle, told_page), (TASK, &page));
    }
}
#[test]
fn an_unlinked_client_s_records_are_the_table_s() {
    let mut c = new();
    let record = skein_lib::RequestRecord {
        key: ENGINE_KEY,
        scope: PARTY,
        first_sent: Some(Wall::EPOCH),
        request: Held { key: STOP, request: fixtures::stop() },
    };
    let mut kept = List::with_capacity(1);
    kept.push(record).unwrap();
    c.step(Event::StartUnlinked { kept });
    let [Request::Told { told: Told::Kept { key, request } }, Request::Told { told: Told::Pending { key: pending } }] =
        c.requests()
    else {
        panic!("one restored request: {:?}", c.requests());
    };
    assert_eq!((*key, request, *pending), (STOP, &fixtures::stop(), STOP));
    assert_eq!(c.machine.pending(STOP), Some(Pending { progress: Progress::Parked, party: PARTY, owner: None }));
    assert_eq!(c.machine.link().credential, None);
}

#[test]
fn an_unlinked_sign_out_its_world_loses_keeps_the_party_signed_in() {
    let mut c = live();
    c.step(Event::SignOut);
    assert_eq!(c.requests().len(), 1);
    assert_eq!(c.requests(), &[Request::SignOut]);
    c.step(Event::SignOutLost);
    let [Request::Told { told: Told::Link { link } }] = c.requests() else { panic!("link tells the loss") };
    assert_eq!(*link, Link::Live);
    assert_eq!(c.machine.link().party, Some(PARTY));
    assert!(c.machine.link().sign_out_lost);
    c.step(Event::Up { up: Up::SignedIn });
    assert!(!c.machine.link().sign_out_lost);
}
