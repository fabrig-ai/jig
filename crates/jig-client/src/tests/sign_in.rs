use super::{
    fixtures::{self, CREDENTIAL, Client, NEW_CREDENTIAL, SERVICE_CREDENTIAL, STOP, TASK},
    *,
};
use jig_client_protocol_wire::v19 as w;
use skein_lib::{Duration, Time};
const FIRST_BACKOFF: Duration = Duration::from_millis(10);
const LEAST_BACKOFF: Duration = Duration::from_millis(5);
const HELD_WATCHES: u32 = 2;

#[test]
fn a_watch_asked_before_signing_in_opens_once_signed_in() {
    let mut c = Client::new();
    c.step(Event::Watch { handle: ME, subject: Watchable::Jig(contract::Subject::Me) });
    assert!(c.requests().is_empty());
    c.step(Event::Watch { handle: TASK, subject: task() });
    assert!(c.requests().is_empty());
    c.step(Event::SignIn { with: With::Held(fixtures::credential(&CREDENTIAL)) });
    let [Request::Told { told }, Request::OpenStream { credential }] = c.requests() else {
        panic!("starting with held credential: {:?}", c.requests());
    };
    assert_eq!(*told, Told::Link { link: Link::Starting });
    assert_eq!(credential, &Some(fixtures::credential(&CREDENTIAL)));
    c.stream(fixtures::hello(jig_client_protocol_wire::VERSION));
    assert_eq!(c.requests.len(), 3);
    let Request::Save { records } = c.requests.last().expect("hello saves its stream") else {
        panic!("hello saves its stream");
    };
    assert_eq!(records.len(), 1);
    assert_eq!(records.as_slice().first().unwrap().key.as_ref(), STREAM_KEY);
    c.requests.truncate(HELD_WATCHES);
    assert_eq!(c.requests().len(), 2);
    for request in c.requests() {
        let Request::Exchange { credential, message: Message::Watch(_), .. } = request else {
            panic!("one exchange for each held watch: {request:?}");
        };
        assert_eq!(credential, &Some(fixtures::credential(&CREDENTIAL)));
    }
}

#[test]
fn signed_out_ends_each_handle_once_and_parks_each_request() {
    for terminal in fixtures::SIGNING_TERMINALS {
        let mut c = Client::narrow_signed_in();
        c.step(Event::Watch { handle: TASK, subject: task() });
        let opening = c.exchange();
        let watch = c.opened_watch();
        c.step(Event::Send { name: None, key: STOP, request: fixtures::stop() });
        c.step(Event::Stored);
        let request = c.exchange();
        c.step(Event::Read { handle: ME, read: Readable::Jig(contract::Read::Project { project: PROJECT }) });
        let read = c.exchange();
        c.signed_out_by(terminal, &fixtures::SigningExchanges { request, read, opening, watch });
        assert_eq!(fixtures::ended(c.requests()).as_slice(), &[(TASK, Why::SignedOut)], "{terminal:?}");
        assert_eq!(fixtures::pages(c.requests()).as_slice(), &[(ME, signed_out_page())], "{terminal:?}");
        assert_eq!(c.machine.link().state, Link::SignedOut, "{terminal:?}");
        assert_eq!(c.machine.link().party, None, "{terminal:?}");
        assert_eq!(c.machine.link().credential, None, "{terminal:?}");
        assert_eq!(c.machine.pending(STOP).unwrap().progress, Progress::Parked, "{terminal:?}");
        assert_eq!(c.machine.document(TASK).unwrap().state, State::Ended, "{terminal:?}");
        c.step(Event::Reply { exchange: request, message: fixtures::answer(&w::KeyedAnswer::SignedOut) });
        assert!(c.requests().is_empty(), "{terminal:?}");
        c.step(Event::Reply { exchange: read, message: fixtures::unknown_read() });
        assert!(c.requests().is_empty(), "{terminal:?}");
    }
}
#[test]
fn a_lost_sign_out_keeps_the_credential_and_says_so() {
    let mut c = Client::signed_in();
    c.step(Event::Watch { handle: TASK, subject: task() });
    c.step(Event::SignOut);
    c.step(Event::Lost { exchange: c.exchange() });
    assert_eq!(c.machine.link().state, Link::Live);
    assert_eq!(c.machine.link().credential, Some(&fixtures::credential(&CREDENTIAL)));
    assert!(c.machine.link().sign_out_lost);
    assert_eq!(fixtures::ended(c.requests()).as_slice(), &[]);
    assert_eq!(c.machine.document(TASK).unwrap().state, State::Opening);
}
#[test]
fn an_earlier_sign_in_s_sign_out_answer_is_dropped() {
    let mut c = Client::signed_in();
    c.step(Event::SignOut);
    let exchange = c.exchange();
    c.step(Event::SignIn { with: With::Held(fixtures::credential(&NEW_CREDENTIAL)) });
    c.stream(fixtures::hello(jig_client_protocol_wire::VERSION));
    let watch = c.opened_watch();
    c.stream(fixtures::frame(watch));
    c.step(Event::Reply { exchange, message: fixtures::sign_out_answer(&w::SignOutAnswer::SignedOut) });
    assert_eq!(c.machine.link().state, Link::Live);
    assert_eq!(c.machine.link().credential, Some(&fixtures::credential(&NEW_CREDENTIAL)));
    assert!(!c.machine.link().sign_out_lost);
}
#[test]
fn a_service_signs_in_with_its_credential_as_the_message_s() {
    let mut c = Client::new();
    c.step(Event::SignIn { with: With::Service(fixtures::credential(&SERVICE_CREDENTIAL)) });
    let [Request::Exchange { exchange, credential, message }] = c.requests() else {
        panic!("one service sign-in: {:?}", c.requests());
    };
    assert_eq!(message, &Message::SignInService);
    assert_eq!(credential, &Some(fixtures::credential(&SERVICE_CREDENTIAL)));
    let exchange = *exchange;
    c.step(Event::Reply { exchange, message: fixtures::signed_in(&NEW_CREDENTIAL) });
    let [Request::Told { told }, Request::OpenStream { credential }] = c.requests() else {
        panic!("starting with new credential: {:?}", c.requests());
    };
    assert_eq!(*told, Told::Link { link: Link::Starting });
    assert_eq!(credential, &Some(fixtures::credential(&NEW_CREDENTIAL)));
}
#[test]
fn under_transport_exchanges_carry_no_credential() {
    let mut c = Client::new();
    c.step(Event::SignIn { with: With::Transport });
    let [Request::Told { told }, Request::OpenStream { credential }] = c.requests() else {
        panic!("starting with transport credential: {:?}", c.requests());
    };
    assert_eq!(*told, Told::Link { link: Link::Starting });
    assert_eq!(credential, &None);
    c.stream(fixtures::hello(jig_client_protocol_wire::VERSION));
    let watch = c.opened_watch();
    c.stream(fixtures::frame(watch));
    c.step(Event::Watch { handle: TASK, subject: task() });
    let [Request::Exchange { credential, .. }] = c.requests() else {
        panic!("one watch exchange: {:?}", c.requests());
    };
    assert_eq!(credential, &None);
    assert_eq!(c.machine.link().credential, None);
}
#[test]
fn a_held_credential_never_reaches_the_small_store() {
    let mut c = Client::signed_in();
    c.step(Event::Send { name: None, key: STOP, request: fixtures::stop() });
    let records = fixtures::saved(c.requests());
    assert_eq!(records.len(), 1);
    for record in &records {
        for bytes in record.key.windows(CREDENTIAL.len()) {
            assert_ne!(bytes, CREDENTIAL);
        }
        for bytes in record.bytes.windows(CREDENTIAL.len()) {
            assert_ne!(bytes, CREDENTIAL);
        }
    }
}
#[test]
fn signing_out_again_after_a_lost_sign_out_is_answered_signed_out() {
    let mut c = Client::signed_in();
    c.step(Event::SignOut);
    c.step(Event::Lost { exchange: c.exchange() });
    assert!(c.machine.link().sign_out_lost);
    c.step(Event::SignOut);
    assert!(!c.machine.link().sign_out_lost);
    c.step(Event::Reply { exchange: c.exchange(), message: fixtures::sign_out_answer(&w::SignOutAnswer::SignedOut) });
    assert_eq!(c.machine.link().state, Link::SignedOut);
    assert_eq!(c.machine.link().credential, None);
    assert!(!c.machine.link().sign_out_lost);
}
#[test]
fn a_sign_out_answered_not_ready_did_not_reach_the_engine() {
    let mut c = Client::signed_in();
    c.step(Event::SignOut);
    c.step(Event::Reply { exchange: c.exchange(), message: fixtures::sign_out_answer(&w::SignOutAnswer::NotReady) });
    assert_eq!(c.machine.link().state, Link::Live);
    assert_eq!(c.machine.link().credential, Some(&fixtures::credential(&CREDENTIAL)));
    assert!(c.machine.link().sign_out_lost);
}
#[test]
fn a_service_s_sign_in_answered_not_ready_is_sent_again_after_its_backoff() {
    let mut c = Client::new();
    c.step(Event::SignIn { with: With::Service(fixtures::credential(&SERVICE_CREDENTIAL)) });
    let first = c.exchange();
    c.step(Event::Reply { exchange: first, message: fixtures::sign_in_answer(&w::SignInAnswer::NotReady) });
    assert!(c.requests().is_empty());
    let until = c.machine.next_deadline().unwrap();
    assert!(until >= Time::ZERO.saturating_add(LEAST_BACKOFF));
    assert!(until <= Time::ZERO.saturating_add(FIRST_BACKOFF));
    c.fire(until);
    let [Request::Exchange { exchange, credential, message }] = c.requests() else {
        panic!("one service retry: {:?}", c.requests());
    };
    assert_ne!(*exchange, first);
    assert_eq!(message, &Message::SignInService);
    assert_eq!(credential, &Some(fixtures::credential(&SERVICE_CREDENTIAL)));
}

#[test]
fn a_sign_out_that_did_not_reach_the_engine_tells_the_link() {
    let mut c = Client::signed_in();
    c.step(Event::SignOut);
    c.step(Event::Lost { exchange: c.exchange() });
    let mut links = skein_lib::List::with_capacity(1);
    for request in c.requests() {
        match request {
            Request::Told { told: Told::Link { link } } => links.push(*link).unwrap(),
            Request::Exchange { .. }
            | Request::OpenStream { .. }
            | Request::Authorized(_)
            | Request::ServiceCredential { .. }
            | Request::CloseStream
            | Request::Save { .. }
            | Request::Erase { .. }
            | Request::Told { .. }
            | Request::Down { .. }
            | Request::SignOut => {}
        }
    }
    assert_eq!(links.as_slice(), &[Link::Live]);
    assert!(c.machine.link().sign_out_lost);
}

const MADE_PARTY: u64 = 7;
const CREDENTIALS_PER_REPLY: u32 = 1;
#[test]
fn the_link_s_service_credential_leaves_once_as_a_host_duty() {
    let mut c = Client::signed_in();
    c.step(Event::Send { name: None, key: STOP, request: fixtures::stop() });
    c.step(Event::Stored);
    let exchange = c.exchange();
    let reply = service_answer();
    c.step(Event::Reply { exchange, message: reply.clone() });
    let mut credentials = skein_lib::List::with_capacity(CREDENTIALS_PER_REPLY);
    for request in c.requests() {
        match request {
            Request::ServiceCredential { party, credential } => credentials.push((*party, credential.clone())).unwrap(),
            Request::Exchange { .. }
            | Request::OpenStream { .. }
            | Request::CloseStream
            | Request::Save { .. }
            | Request::Erase { .. }
            | Request::Told { .. }
            | Request::Down { .. }
            | Request::Authorized(_)
            | Request::SignOut => {}
        }
    }
    assert_eq!(credentials.as_slice(), &[(MADE_PARTY, fixtures::credential(&SERVICE_CREDENTIAL))]);
    c.step(Event::Reply { exchange, message: reply });
    assert!(c.requests().is_empty());
}
fn service_answer() -> Box<[u8]> {
    let limits = w::CEILINGS;
    let service = w::MadeService::new(
        &limits,
        w::MadeServiceParts { party: MADE_PARTY, credential: fixtures::credential(&SERVICE_CREDENTIAL) },
    )
    .unwrap();
    let made = w::OutcomeMade::new(&limits, w::OutcomeMadeParts { what: w::Made::Service(service) }).unwrap();
    let done = w::Done::new(&limits, w::DoneParts { outcome: w::Outcome::Made(made) }).unwrap();
    let value = w::Final::Done(done);
    let mut bytes = skein_lib::Writer::new(usize::try_from(value.measure()).unwrap());
    value.encode(&mut bytes).unwrap();
    let owned = w::Owned::new(
        &limits,
        w::OwnedParts { owner: w::Owner::Jig, version: jig_client_protocol_wire::VERSION, record: bytes.finish() },
    )
    .unwrap();
    fixtures::answer(&w::KeyedAnswer::FinalAnswer(owned))
}

fn signed_out_page() -> Readout {
    Readout::Jig(contract::ReadResult::None(contract::ReadNone::SignedOut))
}
fn unknown_page() -> Readout {
    Readout::Jig(contract::ReadResult::None(contract::ReadNone::Unknown))
}
fn assert_no_read_exchange(c: &Client) {
    for request in c.requests() {
        if let Request::Exchange { message: Message::Read(_), .. } = request {
            panic!("unexpected read replay: {request:?}");
        }
    }
}
fn assert_read_credential(c: &Client, expected: Option<&skein_lib::Secret>) {
    let [Request::Exchange { credential, message: Message::Read(_), .. }] = c.requests() else {
        panic!("one read exchange: {:?}", c.requests());
    };
    assert_eq!(credential.as_ref(), expected);
}
fn replacement_survives_old_terminals(c: &mut Client, old: &[skein_lib::Token]) {
    assert_no_read_exchange(c);
    c.stream(fixtures::hello(jig_client_protocol_wire::VERSION));
    let watch = c.opened_watch();
    assert_no_read_exchange(c);
    assert!(fixtures::pages(c.requests()).is_empty());
    c.stream(fixtures::frame(watch));
    assert_no_read_exchange(c);
    c.step(Event::Stored);
    assert_no_read_exchange(c);
    let replacement = c.read(TASK);
    assert!(!old.contains(&replacement));
    assert_read_credential(c, c.machine.link().credential);
    for exchange in old {
        c.step(Event::Reply { exchange: *exchange, message: fixtures::unknown_read() });
        assert!(c.requests().is_empty());
        c.step(Event::Lost { exchange: *exchange });
        assert!(c.requests().is_empty());
    }
    c.step(Event::Reply { exchange: replacement, message: fixtures::unknown_read() });
    assert_eq!(c.requests(), &[Request::Told { told: Told::Page { handle: TASK, page: unknown_page() } }]);
    c.step(Event::Lost { exchange: replacement });
    assert!(c.requests().is_empty());
}

#[test]
fn held_and_transport_installation_end_reads_even_before_first_sign_in() {
    for start in [Client::new, Client::signed_in] {
        for with in [With::Held(fixtures::credential(&NEW_CREDENTIAL)), With::Transport] {
            let mut c = start();
            let reads = [c.read(TASK), c.read(ME)];
            let expected_credential = match &with {
                With::Held(credential) => Some(credential.clone()),
                With::Transport => None,
                With::Link { .. } | With::Service(_) => unreachable!("immediate installation"),
            };
            c.step(Event::SignIn { with });
            assert_eq!(fixtures::pages(c.requests()).as_slice(), &[(TASK, signed_out_page()), (ME, signed_out_page())]);
            assert_eq!(c.machine.link().credential, expected_credential.as_ref());
            replacement_survives_old_terminals(&mut c, &reads);
        }
    }
}

fn exchanging_sign_ins() -> [With; 2] {
    [
        With::Link { secret: fixtures::credential(&CREDENTIAL), held: None },
        With::Service(fixtures::credential(&SERVICE_CREDENTIAL)),
    ]
}
fn reads_across_sign_in_exchange(with: With) -> (Client, skein_lib::Token, [skein_lib::Token; 2]) {
    let mut c = Client::signed_in();
    let before = c.read(TASK);
    c.step(Event::SignIn { with });
    let signing = c.exchange();
    let during = c.read(ME);
    assert_read_credential(&c, Some(&fixtures::credential(&CREDENTIAL)));
    (c, signing, [before, during])
}

#[test]
fn successful_exchange_cancels_reads_from_before_and_during_sign_in() {
    for with in exchanging_sign_ins() {
        let (mut c, signing, reads) = reads_across_sign_in_exchange(with);
        c.step(Event::Reply { exchange: signing, message: fixtures::signed_in(&NEW_CREDENTIAL) });
        assert_eq!(fixtures::pages(c.requests()).as_slice(), &[(TASK, signed_out_page()), (ME, signed_out_page())]);
        assert_eq!(c.machine.link().credential, Some(&fixtures::credential(&NEW_CREDENTIAL)));
        replacement_survives_old_terminals(&mut c, &reads);
    }
}

#[test]
fn read_responses_before_successful_installation_are_the_only_pages() {
    for with in exchanging_sign_ins() {
        let (mut c, signing, reads) = reads_across_sign_in_exchange(with);
        for (handle, exchange) in [TASK, ME].into_iter().zip(reads) {
            c.step(Event::Reply { exchange, message: fixtures::unknown_read() });
            assert_eq!(c.requests(), &[Request::Told { told: Told::Page { handle, page: unknown_page() } }]);
        }
        c.step(Event::Reply { exchange: signing, message: fixtures::signed_in(&NEW_CREDENTIAL) });
        assert!(fixtures::pages(c.requests()).is_empty());
        assert_no_read_exchange(&c);
        assert_eq!(c.machine.link().credential, Some(&fixtures::credential(&NEW_CREDENTIAL)));
    }
}

#[test]
fn sign_in_without_installation_leaves_old_reads_eligible() {
    // Malformed, refused, not ready, and lost all install no credential.
    for with in exchanging_sign_ins() {
        for result in [
            Some(Box::<[u8]>::from([])),
            Some(fixtures::sign_in_answer(&w::SignInAnswer::Refused)),
            Some(fixtures::sign_in_answer(&w::SignInAnswer::NotReady)),
            None,
        ] {
            let (mut c, signing, reads) = reads_across_sign_in_exchange(match &with {
                With::Link { secret, held } => With::Link { secret: secret.clone(), held: held.clone() },
                With::Service(credential) => With::Service(credential.clone()),
                With::Held(_) | With::Transport => unreachable!("exchange sign-in"),
            });
            let event = match result {
                Some(message) => Event::Reply { exchange: signing, message },
                None => Event::Lost { exchange: signing },
            };
            c.step(event);
            assert!(fixtures::pages(c.requests()).is_empty());
            assert_eq!(c.machine.link().credential, Some(&fixtures::credential(&CREDENTIAL)));
            for (handle, exchange) in [TASK, ME].into_iter().zip(reads) {
                c.step(Event::Reply { exchange, message: fixtures::unknown_read() });
                assert_eq!(c.requests(), &[Request::Told { told: Told::Page { handle, page: unknown_page() } }]);
            }
        }
    }
}

#[test]
fn a_signed_out_read_visibly_ends_itself_and_other_reads_once() {
    let mut c = Client::signed_in();
    let request = c.read(TASK);
    let read = c.read(ME);
    c.step(Event::Reply { exchange: read, message: fixtures::read_none(w::ReadNone::SignedOut) });
    assert_eq!(fixtures::pages(c.requests()).as_slice(), &[(TASK, signed_out_page()), (ME, signed_out_page())]);
    assert_eq!(c.machine.link().state, Link::SignedOut);
    for exchange in [request, read] {
        c.step(Event::Reply { exchange, message: fixtures::unknown_read() });
        assert!(c.requests().is_empty());
        c.step(Event::Lost { exchange });
        assert!(c.requests().is_empty());
    }
}

// Exceeds the domain fixture's output allowance without a reads term, with no watch handles.
const INDEPENDENT_READS: u32 = 64;
#[test]
fn independent_read_room_and_pages_fit_the_composed_queues() {
    let limits = Limits {
        domain: DomainLimits { handles: 0, watches: 1, reads: INDEPENDENT_READS, ..fixtures::domain_limits() },
        exchanges: INDEPENDENT_READS,
    };
    let mut c = Client::with_limits(limits, skein_lib::List::with_capacity(0), max_out(&limits));
    let mut expected = skein_lib::List::with_capacity(INDEPENDENT_READS);
    for number in 0..u64::from(INDEPENDENT_READS) {
        let handle = Handle { asker: Asker::JIG, number };
        c.read(handle);
        expected.push((handle, signed_out_page())).unwrap();
    }
    let extra = Handle { asker: Asker::JIG, number: u64::from(INDEPENDENT_READS) };
    c.step(Event::Read { handle: extra, read: Readable::Jig(contract::Read::Project { project: PROJECT }) });
    assert_eq!(fixtures::pages(c.requests()).as_slice(), &[(extra, Readout::Limit)]);
    c.step(Event::SignIn { with: With::Held(fixtures::credential(&NEW_CREDENTIAL)) });
    assert_eq!(fixtures::pages(c.requests()).as_slice(), expected.as_slice());
    assert!(u32::try_from(c.requests().len()).unwrap() <= max_out(&limits));
    assert_no_read_exchange(&c);
    c.read(TASK);
    assert_read_credential(&c, Some(&fixtures::credential(&NEW_CREDENTIAL)));
}

#[test]
fn overflowing_read_outputs_are_refused_before_construction() {
    let limits = Limits { domain: DomainLimits { reads: u32::MAX, ..fixtures::domain_limits() }, ..fixtures::limits() };
    assert!(!valid(&limits));
    assert_eq!(crate::Client::new(&limits, 7).expect_err("read output overflow"), Unusable::Limits);
    assert_eq!(crate::Client::unlinked(&limits, 7).expect_err("read output overflow"), Unusable::Limits);
}

#[test]
fn read_room_prices_the_composed_cancellation_buffers() {
    let full = fixtures::limits();
    let empty = Limits { domain: DomainLimits { reads: 0, ..full.domain }, ..full };
    let domain_growth = jig_client_domain::worst_case(&full.domain)
        .unwrap()
        .checked_sub(jig_client_domain::worst_case(&empty.domain).unwrap())
        .unwrap();
    let composed_growth = worst_case(&full).unwrap().checked_sub(worst_case(&empty).unwrap()).unwrap();
    assert!(max_out(&full) > max_out(&empty));
    assert!(composed_growth > domain_growth);
}

fn link_attempt() -> With {
    With::Link { secret: skein_lib::Secret::new(Box::new([3; 32])), held: None }
}
fn full_sign_in_answer() -> w::SignInAnswer {
    w::SignInAnswer::Full(
        w::SignInsFull::new(&w::CEILINGS, w::SignInsFullParts { sign_ins: 2, earliest: 400, link_until: 300 }).unwrap(),
    )
}
fn assert_link_told(c: &Client) {
    let mut told = false;
    for request in c.requests() {
        if let Request::Told { told: Told::Link { .. } } = request {
            told = true;
        }
    }
    assert!(told);
}

#[test]
fn each_link_retry_answer_is_visible_and_told_until_the_next_sign_in() {
    for (answer, why) in [
        (w::SignInAnswer::NotReady, NotSignedIn::NotReady),
        (w::SignInAnswer::Busy, NotSignedIn::Busy),
        (
            full_sign_in_answer(),
            NotSignedIn::Full {
                sign_ins: 2,
                earliest: skein_lib::Wall::from_nanos(400),
                link_until: skein_lib::Wall::from_nanos(300),
            },
        ),
    ] {
        let mut c = Client::new();
        c.step(Event::SignIn { with: link_attempt() });
        c.step(Event::Reply { exchange: c.exchange(), message: fixtures::sign_in_answer(&answer) });
        assert_eq!(c.machine.link().not_signed_in, Some(why));
        assert!(c.machine.link().credential.is_none());
        assert_link_told(&c);
        assert!(c.machine.next_deadline().is_none(), "link never retries itself");
        c.step(Event::SignIn { with: link_attempt() });
        assert_eq!(c.machine.link().not_signed_in, None);
        assert_link_told(&c);
        c.step(Event::Reply { exchange: c.exchange(), message: fixtures::signed_in(&NEW_CREDENTIAL) });
        assert_eq!(c.machine.link().not_signed_in, None);
        assert_eq!(c.machine.link().credential, Some(&fixtures::credential(&NEW_CREDENTIAL)));
        assert_link_told(&c);
    }
}

#[test]
fn a_services_busy_or_full_ends_signing_without_a_retry() {
    for answer in [w::SignInAnswer::Busy, full_sign_in_answer()] {
        let mut c = Client::new();
        c.step(Event::SignIn { with: With::Service(fixtures::credential(&SERVICE_CREDENTIAL)) });
        c.step(Event::Reply { exchange: c.exchange(), message: fixtures::sign_in_answer(&answer) });
        assert_eq!(c.machine.link().not_signed_in, None);
        assert!(c.machine.link().credential.is_none());
        assert!(c.machine.next_deadline().is_none());
        c.fire(Time::from_nanos(u64::MAX));
        assert!(c.requests().is_empty());
    }
}
