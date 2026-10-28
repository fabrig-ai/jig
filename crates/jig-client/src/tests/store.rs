use super::{
    fixtures::{self, Client, PROJECT, STOP, TASK},
    *,
};
use skein_lib::{Duration, Time};
const SECOND: Key = Key { asker: Asker::JIG, number: 2 };

#[test]
fn seen_sends_without_a_store_write_or_acknowledgement() {
    let mut c = Client::signed_in();
    c.step(Event::Send { name: None, key: SECOND, request: fixtures::seen() });
    assert!(fixtures::saved(c.requests()).is_empty());
    assert_eq!(fixtures::keyed_count(c.requests()), 1);
    assert_eq!(c.machine.pending(SECOND).unwrap().progress, Progress::InFlight);
}

#[test]
fn a_keyed_request_waits_for_its_record_to_be_stored() {
    let mut c = Client::signed_in();
    c.step(Event::Send { name: None, key: STOP, request: fixtures::stop() });
    assert_eq!(fixtures::keyed_count(c.requests()), 0);
    assert_eq!(fixtures::saved(c.requests()).len(), 1);
    c.step(Event::Stored);
    assert_eq!(fixtures::keyed_count(c.requests()), 1);
}
#[test]
fn a_read_leaves_while_a_record_is_being_stored() {
    let mut c = Client::signed_in();
    c.step(Event::Send { name: None, key: STOP, request: fixtures::stop() });
    c.step(Event::Read { handle: TASK, read: Readable::Jig(contract::Read::Project { project: PROJECT }) });
    let [Request::Exchange { message: Message::Read(_), .. }] = c.requests() else {
        panic!("read leaves during save: {:?}", c.requests());
    };
}
#[test]
fn a_watch_leaves_while_a_record_is_being_stored() {
    let mut c = Client::signed_in();
    c.step(Event::Send { name: None, key: STOP, request: fixtures::stop() });
    c.step(Event::Watch { handle: TASK, subject: task() });
    let [Request::Exchange { message: Message::Watch(_), .. }] = c.requests() else {
        panic!("watch leaves during save: {:?}", c.requests());
    };
}
#[test]
fn a_record_not_stored_answers_its_request_not_kept() {
    let mut c = Client::signed_in();
    c.step(Event::Send { name: None, key: STOP, request: fixtures::stop() });
    c.step(Event::NotStored);
    let mut answers = 0_u32;
    for request in c.requests() {
        if let Request::Told { told: Told::Answered { key, outcome } } = request {
            assert_eq!(*key, STOP);
            assert_eq!(*outcome, Outcome::NotKept);
            answers += 1;
        }
    }
    assert_eq!(answers, 1);
    assert_eq!(c.machine.pending(STOP), None);
    assert_eq!(fixtures::keyed_count(c.requests()), 0);
    c.step(Event::Stored);
    assert_eq!(fixtures::keyed_count(c.requests()), 0);
}
#[test]
fn a_parked_request_whose_record_is_not_stored_is_answered_not_kept() {
    let mut c = Client::signed_in();
    c.step(Event::StreamGone);
    c.step(Event::Send { name: None, key: STOP, request: fixtures::stop() });
    let records = fixtures::saved(c.requests());
    assert_eq!(records.len(), 1);
    c.step(Event::NotStored);
    let [Request::Told { told }, Request::Erase { records: erased }] = c.requests() else {
        panic!("one answer and erase: {:?}", c.requests());
    };
    assert_eq!(*told, Told::Answered { key: STOP, outcome: Outcome::NotKept });
    assert_eq!(erased.as_slice(), &[records.as_slice()[0].key.clone()]);
    assert_eq!(c.machine.pending(STOP), None);
    c.step(Event::Stored);
    c.fire(Time::from_nanos(100_000_000));
    c.stream(fixtures::hello(jig_client_protocol_wire::VERSION));
    let watch = c.opened_watch();
    c.stream(fixtures::frame(watch));
    assert_eq!(c.machine.link().state, Link::Live);
    assert_eq!(fixtures::keyed_count(c.requests()), 0);
    assert_eq!(fixtures::answers(c.requests()).as_slice(), &[]);
}
#[test]
fn a_failed_write_answers_only_the_requests_it_carried() {
    let mut c = Client::signed_in();
    c.step(Event::Send { name: None, key: STOP, request: fixtures::stop() });
    assert_eq!(fixtures::saved(c.requests()).len(), 1);
    c.step(Event::Send { name: None, key: SECOND, request: fixtures::stop() });
    assert_eq!(fixtures::saved(c.requests()).len(), 1);
    c.step(Event::NotStored);
    assert_eq!(fixtures::answers(c.requests()).as_slice(), &[(STOP, Outcome::NotKept)]);
    assert_eq!(c.machine.pending(STOP), None);
    assert_eq!(c.machine.pending(SECOND).unwrap().progress, Progress::Retrying { why: Retry::Lost });
    assert_eq!(fixtures::keyed_count(c.requests()), 0);
    c.step(Event::Stored);
    assert_eq!(fixtures::keyed_count(c.requests()), 0);
    c.fire(Time::ZERO.saturating_add(Duration::from_millis(10)));
    assert_eq!(fixtures::keyed_count(c.requests()), 1);
    assert!(c.machine.pending(SECOND).is_some());
}
#[test]
fn a_later_write_s_exchange_waits_when_an_earlier_write_is_stored() {
    let mut c = Client::signed_in();
    c.step(Event::Send { name: None, key: STOP, request: fixtures::stop() });
    c.step(Event::Send { name: None, key: SECOND, request: fixtures::stop() });
    c.step(Event::Stored);
    assert_eq!(fixtures::keyed_count(c.requests()), 0);
    c.step(Event::Stored);
    assert_eq!(fixtures::keyed_count(c.requests()), 2);
}
#[test]
fn a_record_carried_again_is_decided_by_its_later_write() {
    let mut c = Client::signed_in();
    c.step(Event::StreamGone);
    c.step(Event::Send { name: None, key: STOP, request: fixtures::stop() });
    assert_eq!(fixtures::saved(c.requests()).len(), 1);
    c.fire(Time::from_nanos(100_000_000));
    c.stream(fixtures::hello(jig_client_protocol_wire::VERSION));
    let watch = c.opened_watch();
    c.stream(fixtures::frame(watch));
    assert_eq!(fixtures::saved(c.requests()).len(), 1);
    c.step(Event::NotStored);
    assert_eq!(c.machine.pending(STOP).unwrap().progress, Progress::Retrying { why: Retry::Lost });
    assert_eq!(fixtures::answers(c.requests()).as_slice(), &[]);
    c.step(Event::Stored); // The reconnect's stream-name write precedes the later request write.
    c.step(Event::Stored);
    c.fire(Time::from_nanos(110_000_000));
    assert_eq!(fixtures::keyed_count(c.requests()), 1);
    assert!(c.machine.pending(STOP).is_some());
}
#[test]
fn a_step_s_records_leave_in_one_save() {
    let mut first = Client::signed_in();
    first.step(Event::StreamGone);
    first.step(Event::Send { name: None, key: STOP, request: fixtures::stop() });
    let mut kept = skein_lib::List::with_capacity(2);
    for record in fixtures::saved(first.requests()).into_boxed() {
        kept.push(record).unwrap();
    }
    first.step(Event::Send { name: None, key: SECOND, request: fixtures::stop() });
    for record in fixtures::saved(first.requests()).into_boxed() {
        kept.push(record).unwrap();
    }
    let mut c = Client::with_kept(kept);
    c.step(Event::SignIn { with: With::Held(skein_lib::Secret::new(Box::new([7; 40]))) });
    c.stream(fixtures::hello(jig_client_protocol_wire::VERSION));
    let watch = c.opened_watch();
    c.stream(fixtures::frame(watch));
    let mut saves = 0_u32;
    for request in c.requests() {
        if let Request::Save { records } = request {
            assert_eq!(records.len(), 2);
            saves += 1;
        }
    }
    assert_eq!(saves, 1);
    assert_eq!(fixtures::keyed_count(c.requests()), 0);
}
#[test]
fn a_request_lost_while_live_is_sent_again_and_the_link_stays_live() {
    let mut c = Client::signed_in();
    c.step(Event::Send { name: None, key: STOP, request: fixtures::stop() });
    c.step(Event::Stored);
    c.step(Event::Lost { exchange: c.exchange() });
    assert_eq!(c.machine.pending(STOP).unwrap().progress, Progress::Retrying { why: Retry::Lost });
    assert_eq!(c.machine.link().state, Link::Live);
    c.fire(Time::ZERO.saturating_add(Duration::from_millis(10)));
    assert_eq!(fixtures::keyed_count(c.requests()), 1);
    assert_eq!(c.machine.link().state, Link::Live);
}
#[test]
fn a_kept_record_of_another_version_is_dropped_and_told() {
    let mut first = Client::signed_in();
    first.step(Event::Send { name: None, key: STOP, request: fixtures::stop() });
    let records = fixtures::saved(first.requests());
    let mut record = records.as_slice()[0].clone();
    fixtures::other_store_version(&mut record);
    let mut kept = skein_lib::List::with_capacity(1);
    kept.push(record).unwrap();
    let c = Client::with_kept(kept);
    let [Request::Told { told: Told::Dropped { count } }] = c.requests() else {
        panic!("one dropped record: {:?}", c.requests());
    };
    assert_eq!(*count, 1);
    assert_eq!(c.machine.pending(STOP), None);
}
#[test]
fn a_final_answer_that_does_not_decode_is_told_retrying_as_undecodable() {
    let mut c = Client::signed_in();
    c.step(Event::Send { name: None, key: STOP, request: fixtures::stop() });
    c.step(Event::Stored);
    let record = jig_client_protocol_wire::v19::Owned::new(
        &jig_client_protocol_wire::v19::CEILINGS,
        jig_client_protocol_wire::v19::OwnedParts {
            owner: jig_client_protocol_wire::v19::Owner::Jig,
            version: jig_client_protocol_wire::VERSION,
            record: Box::new([255]),
        },
    )
    .unwrap();
    let message = fixtures::answer(&jig_client_protocol_wire::v19::KeyedAnswer::FinalAnswer(record));
    c.step(Event::Reply { exchange: c.exchange(), message });
    assert_eq!(c.machine.pending(STOP).unwrap().progress, Progress::Retrying { why: Retry::Undecodable });
    let [Request::Told { told }] = c.requests() else {
        panic!("one pending notice: {:?}", c.requests());
    };
    assert_eq!(*told, Told::Pending { key: STOP });
}

const STOP_NAME: skein_lib::RequestKey = [3; 16];
const SEEN_NAME: skein_lib::RequestKey = [4; 16];

fn keyed_keys(requests: &[Request]) -> skein_lib::List<skein_lib::RequestKey> {
    let mut keys = skein_lib::List::with_capacity(fixtures::limits().exchanges);
    for request in requests {
        if let Request::Exchange { message: Message::Request(bytes), .. } = request {
            let keyed = jig_client_protocol_wire::v19::Keyed::decode(
                &jig_client_protocol_wire::v19::CEILINGS,
                &mut skein_lib::Reader::new(bytes),
            )
            .unwrap();
            keys.push(*keyed.key()).unwrap();
        }
    }
    keys
}
fn mixed_pending() -> (Client, skein_lib::List<Record>) {
    let mut c = Client::signed_in();
    c.step(Event::Send { name: Some(STOP_NAME), key: STOP, request: fixtures::stop() });
    let records = fixtures::saved(c.requests());
    c.step(Event::Send { name: Some(SEEN_NAME), key: SECOND, request: fixtures::seen() });
    (c, records)
}
#[test]
fn seen_waits_behind_an_ordinary_write_until_stored() {
    let (mut c, records) = mixed_pending();
    assert_eq!(records.len(), 1);
    assert_eq!(records.as_slice()[0].key.as_ref(), STOP_NAME);
    assert!(fixtures::saved(c.requests()).is_empty());
    assert!(keyed_keys(c.requests()).is_empty());
    c.step(Event::Stored);
    assert_eq!(keyed_keys(c.requests()).as_slice(), &[STOP_NAME, SEEN_NAME]);
    assert!(fixtures::saved(c.requests()).is_empty());
    assert!(fixtures::answers(c.requests()).is_empty());
}
#[test]
fn a_failed_ordinary_write_leaves_seen_retrying_lost() {
    let (mut c, _) = mixed_pending();
    c.step(Event::NotStored);
    assert_eq!(fixtures::answers(c.requests()).as_slice(), &[(STOP, Outcome::NotKept)]);
    assert_eq!(c.machine.pending(STOP), None);
    assert_eq!(c.machine.pending(SECOND).unwrap().progress, Progress::Retrying { why: Retry::Lost });
    assert!(fixtures::saved(c.requests()).is_empty());
    assert!(keyed_keys(c.requests()).is_empty());
    c.step(Event::Stored); // Acknowledge Stop's terminal erase.
    c.fire(c.machine.next_deadline().unwrap());
    assert_eq!(keyed_keys(c.requests()).as_slice(), &[SEEN_NAME]);
    assert!(fixtures::saved(c.requests()).is_empty());
    assert!(fixtures::answers(c.requests()).is_empty());
    assert_eq!(c.machine.pending(SECOND).unwrap().progress, Progress::InFlight);
}
fn complete_seen(c: &mut Client, exchange: skein_lib::Token) {
    c.step(Event::Reply { exchange, message: fixtures::seen_answer() });
    assert_eq!(
        fixtures::answers(c.requests()).as_slice(),
        &[(
            SECOND,
            Outcome::Answered(Answered::Jig(contract::Answer::Done(contract::Outcome::Owner(
                contract::OwnerOutcome::MarkedSeen {
                    seen: contract::SeenThrough::Conversation { task: fixtures::SUBJECT_TASK, through: 1 },
                }
            )),))
        )]
    );
    let [Request::Told { .. }, Request::Erase { records }] = c.requests() else {
        panic!("one answer and erase: {:?}", c.requests());
    };
    assert_eq!(records.as_slice(), &[Box::<[u8]>::from(SEEN_NAME)]);
    assert!(fixtures::saved(c.requests()).is_empty());
    assert_eq!(c.machine.pending(SECOND), None);
    c.step(Event::Stored);
    assert!(c.requests().is_empty());
    c.step(Event::Reply { exchange, message: fixtures::seen_answer() });
    assert!(c.requests().is_empty());
}
fn later_stop(c: &mut Client) {
    c.step(Event::Send { name: Some(STOP_NAME), key: STOP, request: fixtures::stop() });
    let records = fixtures::saved(c.requests());
    assert_eq!(records.len(), 1);
    assert_eq!(records.as_slice()[0].key.as_ref(), STOP_NAME);
    assert!(keyed_keys(c.requests()).is_empty());
    c.step(Event::Stored);
    assert_eq!(keyed_keys(c.requests()).as_slice(), &[STOP_NAME]);
    assert_eq!(c.machine.pending(STOP).unwrap().progress, Progress::InFlight);
}
#[test]
fn acknowledging_an_absent_seen_erase_allows_later_ordinary_writes() {
    let mut c = Client::signed_in();
    c.step(Event::Send { name: Some(SEEN_NAME), key: SECOND, request: fixtures::seen() });
    let exchange = c.exchange();
    complete_seen(&mut c, exchange);
    later_stop(&mut c);
}
#[test]
fn reloading_seen_alone_restores_no_pending_work_or_notices() {
    let mut first = Client::signed_in();
    first.step(Event::Send { name: Some(SEEN_NAME), key: SECOND, request: fixtures::seen() });
    let mut c = Client::with_kept(fixtures::saved(first.requests()));
    assert!(c.requests().is_empty());
    assert_eq!(c.machine.pending(SECOND), None);
    c.sign_in();
    c.step(Event::Send { name: Some(SEEN_NAME), key: SECOND, request: fixtures::seen() });
    assert_eq!(keyed_keys(c.requests()).as_slice(), &[SEEN_NAME]);
    assert!(fixtures::saved(c.requests()).is_empty());
}
#[test]
fn reloading_mixed_work_restores_only_the_actual_ordinary_record() {
    let (_, records) = mixed_pending();
    let c = Client::with_kept(records);
    assert_eq!(c.requests(), &[Request::Told { told: Told::Kept { key: STOP, request: fixtures::stop() } },]);
    assert_eq!(c.machine.pending(STOP).unwrap().progress, Progress::Parked);
    assert_eq!(c.machine.pending(SECOND), None);
}
#[test]
fn a_kept_seen_record_decodes_restores_and_erases_without_resaving() {
    for first_sent in [None, Some(skein_lib::Wall::EPOCH)] {
        let mut records = skein_lib::List::with_capacity(1);
        records.push(fixtures::kept_seen(SECOND, SEEN_NAME, first_sent)).unwrap();
        let mut c = Client::with_kept(records);
        assert_eq!(c.requests(), &[Request::Told { told: Told::Kept { key: SECOND, request: fixtures::seen() } },]);
        c.step(Event::SignIn { with: With::Held(skein_lib::Secret::new(Box::new([7; 40]))) });
        c.stream(fixtures::hello(jig_client_protocol_wire::VERSION));
        let watch = c.opened_watch();
        c.step(Event::Stored); // Only the stream-name write.
        c.stream(fixtures::frame(watch));
        assert!(fixtures::saved(c.requests()).is_empty());
        assert_eq!(keyed_keys(c.requests()).as_slice(), &[SEEN_NAME]);
        let exchange = c.exchange();
        complete_seen(&mut c, exchange);
        later_stop(&mut c);
    }
}
