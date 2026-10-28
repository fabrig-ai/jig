use super::fixtures::{self, Client, ME, OTHER, OTHER_SUBJECT, SUBJECT_TASK, TASK, delivery, task};
use crate::*;
use skein_lib::{Duration, Time};
const REFUSALS: [(contract::WatchEnd, Why); 5] = [
    (contract::WatchEnd::Unknown, Why::Unknown),
    (contract::WatchEnd::NotAllowed, Why::NotAllowed),
    (contract::WatchEnd::SubjectEnded, Why::SubjectEnded),
    (contract::WatchEnd::SignedOut, Why::SignedOut),
    (contract::WatchEnd::TooLinked { joins: 4 }, Why::TooLinked { joins: 4 }),
];
#[test]
fn two_handles_on_one_subject_share_one_engine_watch() {
    let mut c = Client::started();
    c.step(Event::Watch { handle: TASK, subject: task(SUBJECT_TASK) });
    c.step(Event::Watch { handle: OTHER, subject: task(SUBJECT_TASK) });
    assert_eq!(c.downs().as_ref(), &[Down::Open { watch: fixtures::INITIAL_TASK_WATCH, subject: task(SUBJECT_TASK) }]);
    c.step(Event::Up { up: delivery(SUBJECT_TASK) });
    assert_eq!(c.told().as_ref(), &[Told::Changed { handle: TASK }, Told::Changed { handle: OTHER }]);
    assert_eq!(c.domain.document(OTHER).unwrap().state, State::Live);
    assert!(core::ptr::eq(
        c.domain.document(TASK).unwrap().latest.unwrap(),
        c.domain.document(OTHER).unwrap().latest.unwrap()
    ));
}
#[test]
fn the_last_unwatch_closes_the_engine_watch_and_tells_nothing() {
    let mut c = Client::started();
    c.step(Event::Watch { handle: TASK, subject: task(SUBJECT_TASK) });
    c.step(Event::Watch { handle: OTHER, subject: task(SUBJECT_TASK) });
    let name = c.name(task(SUBJECT_TASK));
    c.step(Event::Up { up: Up::Watched { watch: name, event: contract::WatchEvent::Opened } });
    c.downs();
    c.step(Event::Unwatch { handle: TASK });
    assert!(c.downs().is_empty());
    c.step(Event::Unwatch { handle: OTHER });
    assert_eq!(c.downs().as_ref(), &[Down::Close { watch: name }]);
    c.end(name, contract::WatchEnd::Closed);
    assert!(c.told().is_empty());
    assert_eq!(c.domain.document(TASK), None);
    assert_eq!(c.domain.watches.len(), 1);
}
#[test]
fn a_dropped_watch_opens_again_under_a_fresh_name_after_its_backoff() {
    let mut c = Client::started();
    c.step(Event::Watch { handle: TASK, subject: task(SUBJECT_TASK) });
    c.deliver(SUBJECT_TASK);
    let name = c.name(task(SUBJECT_TASK));
    c.downs();
    c.told();
    c.end(name, contract::WatchEnd::Dropped);
    assert_eq!(c.told().as_ref(), &[Told::Changed { handle: TASK }]);
    assert_eq!(c.domain.document(TASK).unwrap().state, State::Behind);
    let deadline = next_deadline(&c.domain).unwrap();
    assert!(deadline >= Time::ZERO.saturating_add(Duration::from_millis(5)));
    assert!(deadline <= Time::ZERO.saturating_add(Duration::from_millis(10)));
    c.at(Time::from_nanos(deadline.as_nanos() - 1));
    assert!(c.downs().is_empty());
    c.at(deadline);
    let next = c.name(task(SUBJECT_TASK));
    assert_ne!(next, name);
    assert_eq!(c.downs().as_ref(), &[Down::Open { watch: next, subject: task(SUBJECT_TASK) }]);
    c.deliver(SUBJECT_TASK);
    assert_eq!(c.domain.document(TASK).unwrap().state, State::Live);
}
#[test]
fn a_watch_moved_to_another_subject_keeps_its_document_behind() {
    let mut c = Client::started();
    c.step(Event::Watch { handle: TASK, subject: task(SUBJECT_TASK) });
    c.deliver(SUBJECT_TASK);
    c.told();
    let previous = c.domain.document(TASK).unwrap().latest.cloned();
    c.step(Event::Watch { handle: TASK, subject: task(OTHER_SUBJECT) });
    assert_eq!(c.domain.document(TASK), Some(Document { state: State::Behind, latest: previous.as_ref() }));
    assert_eq!(c.told().as_ref(), &[Told::Changed { handle: TASK }]);
    c.deliver(OTHER_SUBJECT);
    assert_eq!(c.domain.document(TASK).unwrap().state, State::Live);
    assert_ne!(c.domain.document(TASK).unwrap().latest, previous.as_ref());
}
#[test]
fn a_refused_watch_ends_each_of_its_handles_once() {
    for (end, why) in REFUSALS {
        let mut c = Client::started();
        c.step(Event::Watch { handle: TASK, subject: task(SUBJECT_TASK) });
        c.step(Event::Watch { handle: OTHER, subject: task(SUBJECT_TASK) });
        let name = c.name(task(SUBJECT_TASK));
        c.end(name, end);
        assert_eq!(
            c.told().as_ref(),
            &[Told::Ended { handle: TASK, why }, Told::Ended { handle: OTHER, why }],
            "{end:?}"
        );
        assert_eq!(c.domain.document(TASK).unwrap().state, State::Ended);
        c.end(name, end);
        assert!(c.told().is_empty(), "{end:?}");
    }
}
#[test]
fn a_delivery_for_a_closed_name_is_dropped() {
    let mut c = Client::started();
    c.step(Event::Watch { handle: TASK, subject: task(SUBJECT_TASK) });
    let name = c.name(task(SUBJECT_TASK));
    c.step(Event::Unwatch { handle: TASK });
    c.step(Event::Up { up: delivery(SUBJECT_TASK) });
    c.end(name, contract::WatchEnd::Closed);
    c.step(Event::Up { up: delivery(SUBJECT_TASK) });
    assert!(c.told().is_empty());
    assert_eq!(c.domain.document(TASK), None);
}
#[test]
fn a_watch_past_the_handles_limit_ends_at_once() {
    let mut limits = fixtures::limits();
    limits.handles = 1;
    let mut c = Client::new(limits);
    c.step(Event::Start { kept: skein_lib::List::with_capacity(0) });
    c.downs();
    c.step(Event::Watch { handle: TASK, subject: task(SUBJECT_TASK) });
    c.downs();
    c.step(Event::Watch { handle: OTHER, subject: task(SUBJECT_TASK) });
    assert_eq!(c.told().as_ref(), &[Told::Ended { handle: OTHER, why: Why::Limit }]);
    assert!(c.downs().is_empty());
}
#[test]
fn an_application_handle_on_me_shares_the_client_s_own() {
    let mut c = Client::started();
    let name = c.name(Watchable::Jig(contract::Subject::Me));
    c.step(Event::Watch { handle: ME, subject: Watchable::Jig(contract::Subject::Me) });
    assert!(c.downs().is_empty());
    c.step(Event::Unwatch { handle: ME });
    assert!(c.downs().is_empty());
    assert_eq!(c.name(Watchable::Jig(contract::Subject::Me)), name);
}

#[test]
fn a_watch_refused_busy_is_opened_again_after_a_backoff() {
    let mut c = Client::started();
    c.step(Event::Watch { handle: TASK, subject: task(SUBJECT_TASK) });
    c.deliver(SUBJECT_TASK);
    let previous = c.domain.document(TASK).unwrap().latest.cloned();
    let watch = c.name(task(SUBJECT_TASK));
    c.downs();
    c.told();
    c.end(watch, contract::WatchEnd::Busy);
    assert_eq!(c.told().as_ref(), &[Told::Changed { handle: TASK }]);
    assert_eq!(c.domain.document(TASK), Some(Document { state: State::Behind, latest: previous.as_ref() }));
    let deadline = next_deadline(&c.domain).unwrap();
    c.at(Time::from_nanos(deadline.as_nanos() - 1));
    assert!(c.downs().is_empty());
    c.at(deadline);
    let reopened = c.name(task(SUBJECT_TASK));
    assert_ne!(reopened, watch);
    assert_eq!(c.downs().as_ref(), &[Down::Open { watch: reopened, subject: task(SUBJECT_TASK) }]);
    c.deliver(SUBJECT_TASK);
    assert_eq!(c.domain.document(TASK).unwrap().state, State::Live);
}
const ME_REOPENS: [contract::WatchEnd; 7] = [
    contract::WatchEnd::Busy,
    contract::WatchEnd::Unknown,
    contract::WatchEnd::NotAllowed,
    contract::WatchEnd::Closed,
    contract::WatchEnd::SubjectEnded,
    contract::WatchEnd::Dropped,
    contract::WatchEnd::Stopping,
];
#[test]
fn me_refused_busy_is_opened_again_and_the_link_is_offline() {
    for end in ME_REOPENS {
        let mut c = Client::live();
        c.step(Event::Watch { handle: ME, subject: Watchable::Jig(contract::Subject::Me) });
        c.told();
        c.end(fixtures::ME_WATCH, end);
        assert_eq!(c.domain.link(), Link::Offline, "{end:?}");
        assert_eq!(c.domain.document(ME).unwrap().state, State::Behind, "{end:?}");
        assert_eq!(c.told().as_ref(), &[Told::Link { link: Link::Offline }, Told::Changed { handle: ME }], "{end:?}");
        c.at(next_deadline(&c.domain).unwrap());
        let watch = c.name(Watchable::Jig(contract::Subject::Me));
        assert_ne!(watch, fixtures::ME_WATCH, "{end:?}");
        assert_eq!(
            c.downs().as_ref(),
            &[Down::Open { watch, subject: Watchable::Jig(contract::Subject::Me) }],
            "{end:?}"
        );
        assert_eq!(c.domain.link(), Link::Offline, "{end:?}");
        let mut up = fixtures::me(fixtures::PARTY, Duration::from_secs(1));
        let Up::Watched { watch: name, .. } = &mut up else { unreachable!("me delivery") };
        *name = watch;
        c.step(Event::Up { up });
        assert_eq!(c.domain.link(), Link::Live, "{end:?}");
        assert_eq!(c.domain.document(ME).unwrap().state, State::Live, "{end:?}");
    }
}
#[test]
fn a_watch_closed_before_its_opening_is_answered_closes_once_opened() {
    let mut c = Client::started();
    c.step(Event::Watch { handle: TASK, subject: task(SUBJECT_TASK) });
    let watch = c.name(task(SUBJECT_TASK));
    c.downs();
    c.step(Event::Unwatch { handle: TASK });
    assert!(c.downs().is_empty());
    assert_eq!(c.domain.watches.len(), 2);
    c.step(Event::Up { up: Up::Watched { watch, event: contract::WatchEvent::Opened } });
    assert_eq!(c.downs().as_ref(), &[Down::Close { watch }]);
    c.step(Event::Up { up: Up::Watched { watch, event: contract::WatchEvent::Opened } });
    assert!(c.downs().is_empty());
    c.end(watch, contract::WatchEnd::Closed);
    assert_eq!(c.domain.watches.len(), 1);
    assert!(c.told().is_empty());
}
#[test]
fn a_watch_closed_then_refused_sends_no_closing() {
    let mut c = Client::started();
    c.step(Event::Watch { handle: TASK, subject: task(SUBJECT_TASK) });
    let watch = c.name(task(SUBJECT_TASK));
    c.downs();
    c.step(Event::Unwatch { handle: TASK });
    assert!(c.downs().is_empty());
    c.end(watch, contract::WatchEnd::Busy);
    assert!(c.downs().is_empty());
    assert!(c.told().is_empty());
    assert_eq!(c.domain.watches.len(), 1);
    assert_eq!(next_deadline(&c.domain), None);
}
#[test]
fn the_application_s_watches_leave_one_for_me() {
    let mut limits = fixtures::limits();
    limits.watches = 2;
    let mut c = Client::new(limits);
    c.step(Event::Up { up: Up::SignedOut });
    c.told();
    c.step(Event::Watch { handle: TASK, subject: task(SUBJECT_TASK) });
    c.step(Event::Watch { handle: OTHER, subject: task(OTHER_SUBJECT) });
    assert_eq!(c.told().as_ref(), &[Told::Ended { handle: OTHER, why: Why::Limit }]);
    assert!(c.downs().is_empty());
    c.step(Event::Up { up: Up::SignedIn });
    let me = c.name(Watchable::Jig(contract::Subject::Me));
    let watch = c.name(task(SUBJECT_TASK));
    assert_eq!(
        c.downs().as_ref(),
        &[
            Down::Open { watch: me, subject: Watchable::Jig(contract::Subject::Me) },
            Down::Open { watch, subject: task(SUBJECT_TASK) },
        ]
    );
    assert_eq!(c.domain.watches.len(), 2);
}

#[test]
fn a_delivery_answers_a_closing_watch_without_reviving_its_handle() {
    let mut c = Client::started();
    c.step(Event::Watch { handle: TASK, subject: task(SUBJECT_TASK) });
    let watch = c.name(task(SUBJECT_TASK));
    c.downs();
    c.step(Event::Unwatch { handle: TASK });
    assert!(c.downs().is_empty());
    c.step(Event::Up { up: delivery(SUBJECT_TASK) });
    assert_eq!(c.downs().as_ref(), &[Down::Close { watch }]);
    assert_eq!(c.domain.document(TASK), None);
    c.step(Event::Up { up: delivery(SUBJECT_TASK) });
    assert!(c.downs().is_empty());
    assert!(c.told().is_empty());
}
