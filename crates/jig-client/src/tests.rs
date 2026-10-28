mod accounts;
mod facts;
mod fixtures;
mod reads;
mod shapes;
mod sign_in;
mod store;
mod thread;
mod unlinked;
mod versions;
mod words;
use crate::*;
use fixtures::{Client, ME, PARTY, PROJECT, STOP, TASK, task};
#[test]
fn a_watch_asked_of_the_client_leaves_as_one_opening_on_the_stream() {
    let mut c = Client::signed_in();
    c.step(Event::Watch { handle: TASK, subject: task() });
    let [Request::Exchange { credential: Some(_), message: Message::Watch(_), .. }] = c.requests() else {
        panic!("one watch exchange: {:?}", c.requests());
    };
}
#[test]
fn a_reply_reaches_the_domain_as_its_decoded_answer() {
    let mut c = Client::signed_in();
    c.step(Event::Read { handle: TASK, read: Readable::Jig(contract::Read::Project { project: PROJECT }) });
    let exchange = c.exchange();
    // ReadAnswer::None(Unknown) is encoded by the fixture.
    c.step(Event::Reply { exchange, message: fixtures::unknown_read() });
    let [Request::Told { told: Told::Page { handle, page } }] = c.requests() else {
        panic!("one page: {:?}", c.requests());
    };
    assert_eq!(*handle, TASK);
    assert_eq!(*page, Readout::Jig(contract::ReadResult::None(contract::ReadNone::Unknown)));
}
#[test]
fn an_application_watch_on_me_shares_the_client_s_own() {
    let mut c = Client::signed_in();
    c.step(Event::Watch { handle: ME, subject: Watchable::Jig(contract::Subject::Me) });
    let [Request::Told { told: Told::Changed { handle } }] = c.requests() else {
        panic!("one change: {:?}", c.requests());
    };
    assert_eq!(*handle, ME);
    assert_eq!(c.machine.document(ME).unwrap().state, State::Live);
    assert_eq!(c.machine.link().party, Some(PARTY));
}
#[test]
fn an_engine_not_serving_jig_s_version_leaves_the_link_out_of_date() {
    let mut c = Client::new();
    c.step(Event::SignIn { with: With::Held(skein_lib::Secret::new(Box::new([7; 40]))) });
    c.stream(fixtures::hello(0));
    assert_eq!(c.machine.link().state, Link::OutOfDate);
    c.step(Event::Watch { handle: TASK, subject: task() });
    assert!(c.requests().is_empty());
    assert_eq!(c.machine.next_deadline(), None);
}
#[test]
fn a_request_parked_offline_is_sent_once_me_returns() {
    let mut c = Client::signed_in();
    c.step(Event::StreamGone);
    assert_eq!(c.machine.link().state, Link::Offline);
    c.step(Event::Send { name: None, key: STOP, request: fixtures::stop() });
    assert_eq!(c.machine.pending(STOP).unwrap().progress, Progress::Parked);
    assert_eq!(fixtures::keyed_count(c.requests()), 0);
    c.step(Event::Stored);
    c.machine.fire(
        skein_lib::Time::from_nanos(100_000_000),
        skein_lib::Wall::EPOCH,
        &mut skein_lib::Queue::with_capacity(max_out(&fixtures::limits())),
    );
    c.stream(fixtures::hello(jig_client_protocol_wire::VERSION));
    let watch = c.opened_watch();
    c.step(Event::Stored);
    c.stream(fixtures::frame(watch));
    assert_eq!(c.machine.link().state, Link::Live);
    c.step(Event::Stored);
    assert_eq!(fixtures::keyed_count(c.requests()), 1);
}
#[test]
fn a_read_whose_exchange_is_lost_is_answered_unreachable() {
    let mut c = Client::signed_in();
    c.step(Event::Read { handle: TASK, read: Readable::Jig(contract::Read::Project { project: PROJECT }) });
    c.step(Event::Lost { exchange: c.exchange() });
    let [Request::Told { told: Told::Page { handle, page } }] = c.requests() else {
        panic!("one terminal: {:?}", c.requests());
    };
    assert_eq!(*handle, TASK);
    assert_eq!(*page, Readout::Jig(contract::ReadResult::None(contract::ReadNone::Unreachable)));
}

#[test]
fn a_stream_full_refusal_reaches_the_caller_through_retries_until_hello() {
    use skein_lib::{Secret, Time};
    for bound in [StreamBound::Component, StreamBound::Transport, StreamBound::Exchanges] {
        let mut c = Client::new();
        c.step(Event::SignIn { with: With::Held(Secret::new(Box::new([7; 40]))) });
        let full = StreamFull { bound, size: 2 };
        for retry in 1..=3 {
            c.step(Event::Streamed { message: Streamed::Full(full) });
            assert_eq!(c.machine.link().stream_full, Some(full));
            c.fire(Time::from_nanos(retry * 1_000_000_000));
            let mut opening = false;
            for request in c.requests() {
                if let Request::OpenStream { .. } = request {
                    opening = true;
                }
            }
            assert!(opening, "retry {retry}");
            assert_eq!(c.machine.link().stream_full, Some(full));
        }
        // A failure without a new full record also retains the last named refusal.
        c.step(Event::StreamGone);
        assert_eq!(c.machine.link().stream_full, Some(full));
        c.fire(Time::from_nanos(4_000_000_000));
        c.stream(fixtures::hello(jig_client_protocol_wire::VERSION));
        assert_eq!(c.machine.link().stream_full, None);
    }
}

#[test]
fn a_full_exchange_reaches_the_caller_as_a_named_refusal() {
    let mut c = Client::signed_in();
    c.step(Event::Full {
        exchange: skein_lib::Token::new(u64::MAX),
        full: StreamFull { bound: StreamBound::Exchanges, size: 99 },
    });
    assert_eq!(c.machine.link().stream_full, None, "stale full callbacks are ignored");
    c.step(Event::Read { handle: TASK, read: Readable::Jig(contract::Read::Project { project: PROJECT }) });
    let full = StreamFull { bound: StreamBound::Exchanges, size: 8 };
    c.step(Event::Full { exchange: c.exchange(), full });
    assert_eq!(c.machine.link().stream_full, Some(full));
}
