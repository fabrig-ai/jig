#![expect(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::wildcard_enum_match_arm,
    clippy::match_like_matches_macro,
    reason = "ordinary fixtures collect public outputs"
)]
use crate::{
    family::{self, Document, Item, ItemState, Kind, Scope, Subject},
    first::{self, FamilyTerminal as T},
};
use jig_client::contract::{Part, ProposalKey, Proposer, Revision};
use skein_lib::{Duration, Env, List, Queue, Time, Token, Wall};
fn limits() -> first::Limits {
    first::Limits {
        parts: 2,
        objects: 2,
        reads: 4,
        watches: 4,
        requests: 2,
        segments: 4,
        rows: 2,
        text: 32,
        connector: crate::connector_values::ConnectorLimits {
            rows: 2,
            paths: 2,
            segments: 2,
            segment_bytes: 32,
            bytes: 4096,
        },
        retry: Duration::from_nanos(10),
    }
}
fn drive(side: &mut first::Side, now: u64, event: first::Event) -> Vec<first::Request> {
    let mut out = Queue::uncapped();
    first::step(side, &Env { now: Time::from_nanos(now), wall: Wall::EPOCH, limits: limits() }, event, &mut out);
    let mut rows = Vec::new();
    while let Some(row) = out.pop() {
        rows.push(row);
    }
    rows
}
fn fire(side: &mut first::Side, now: u64) -> Vec<first::Request> {
    let mut out = Queue::uncapped();
    first::fire(side, &Env { now: Time::from_nanos(now), wall: Wall::EPOCH, limits: limits() }, &mut out);
    let mut rows = Vec::new();
    while let Some(row) = out.pop() {
        rows.push(row);
    }
    rows
}
fn start() -> (first::Side, Token, Token) {
    let (mut side, _) = super::root::live_first(limits());
    let rows = drive(&mut side, 0, first::Event::Went { address: first::Address { project: 3, item: 5 } });
    let read = rows
        .iter()
        .find_map(|row| match row {
            first::Request::Out(first::Out::Read { read, .. }) => Some(*read),
            _ => None,
        })
        .unwrap();
    let watch = opened(&rows);
    (side, read, watch)
}
fn opened(rows: &[first::Request]) -> Token {
    rows.iter()
        .find_map(|row| match row {
            first::Request::Out(first::Out::Open { watch, .. }) => Some(*watch),
            _ => None,
        })
        .unwrap()
}
fn item() -> Document {
    let mut proposals = List::with_capacity(2);
    for number in [u64::MAX - 1, u64::MAX] {
        proposals.push(ProposalKey { proposer: Proposer::Task(u64::MAX), number }).unwrap();
    }
    Document::Item(Item {
        project: 3,
        key: 5,
        revision: Revision { value: u64::MAX },
        state: ItemState::Ready,
        value: u64::MAX,
        task: u64::MAX,
        second: None,
        proposals,
        touchable: true,
    })
}
fn object(side: &first::Side) -> &first::Object {
    side.object(side.page().unwrap()).unwrap()
}
fn ends() -> [(family::WatchEnd, T); 9] {
    [
        (family::WatchEnd::Closed, T::Closed),
        (family::WatchEnd::Dropped, T::Dropped),
        (family::WatchEnd::Unknown, T::Unknown),
        (family::WatchEnd::Unsupported, T::Unsupported),
        (family::WatchEnd::SignedOut, T::SignedOut),
        (family::WatchEnd::NotAllowed, T::NotAllowed),
        (family::WatchEnd::SubjectEnded, T::SubjectEnded),
        (family::WatchEnd::Stopping, T::Stopping),
        (family::WatchEnd::Busy, T::Busy),
    ]
}
#[test]
fn family_missing_reads_keep_each_reason_and_consume_only_the_original_read_once() {
    for (result, terminal) in [
        (family::ReadResult::Unknown, T::Unknown),
        (family::ReadResult::Unsupported, T::Unsupported),
        (family::ReadResult::NotAllowed, T::NotAllowed),
    ] {
        let (mut side, read, watch) = start();
        assert!(drive(&mut side, 0, first::Event::Read { read, result }).is_empty());
        assert_eq!(object(&side).terminal, Some(terminal));
        let before = first::shown(&side);
        assert!(drive(&mut side, 0, first::Event::Read { read, result: family::ReadResult::Value(item()) }).is_empty());
        assert_eq!(first::shown(&side), before);
        drive(&mut side, 0, first::Event::Watch { watch, event: family::WatchEvent::Delivered(item()) });
        assert_eq!(object(&side).terminal, None);
        assert_eq!(object(&side).value, first::Value::Known(item()));
    }
}
#[test]
fn all_family_watch_terminals_keep_their_reason_and_leave_equal_tokens_in_other_tabs_alone() {
    for (end, terminal) in ends() {
        let (mut side, read, watch) = start();
        let (mut sibling, sibling_read, sibling_watch) = start();
        assert_eq!(watch, sibling_watch);
        drive(&mut side, 0, first::Event::Read { read, result: family::ReadResult::Value(item()) });
        drive(&mut sibling, 0, first::Event::Read { read: sibling_read, result: family::ReadResult::Value(item()) });
        let rows = drive(&mut side, 1, first::Event::Ended { watch, end });
        assert_eq!(object(&side).terminal, Some(terminal));
        assert_eq!(object(&sibling).terminal, None);
        assert_eq!(object(&side).value, first::Value::Known(item()));
        let recover = match end {
            family::WatchEnd::Dropped | family::WatchEnd::Busy | family::WatchEnd::Stopping => true,
            _ => false,
        };
        assert_eq!(first::next_deadline(&side), if recover { Some(Time::from_nanos(11)) } else { None });
        assert_eq!(
            rows.iter()
                .filter(|r| match r {
                    first::Request::Out(first::Out::Open { .. }) => true,
                    _ => false,
                })
                .count(),
            usize::from(end == family::WatchEnd::Closed)
        );
        assert_eq!(drive(&mut side, 2, first::Event::Ended { watch, end }).len(), 0);
        let before = first::shown(&sibling);
        drive(&mut sibling, 2, first::Event::Watch { watch: sibling_watch, event: family::WatchEvent::Alive });
        assert_eq!(first::shown(&sibling), before);
    }
}
#[test]
fn family_busy_stopping_and_drop_retry_only_at_the_supplied_deadline_and_fresh_values_clear_departure() {
    for end in [family::WatchEnd::Busy, family::WatchEnd::Stopping, family::WatchEnd::Dropped] {
        let (mut side, read, watch) = start();
        drive(&mut side, 0, first::Event::Read { read, result: family::ReadResult::Value(item()) });
        drive(&mut side, 3, first::Event::Ended { watch, end });
        drive(&mut side, 5, first::Event::Ended { watch, end });
        assert_eq!(first::next_deadline(&side), Some(Time::from_nanos(13)));
        let before = object(&side).terminal;
        drive(&mut side, 6, first::Event::Watch { watch, event: family::WatchEvent::Delivered(item()) });
        assert_eq!(object(&side).terminal, before, "late callbacks cannot cancel backoff");
        assert!(fire(&mut side, 12).is_empty());
        assert_eq!(opened(&fire(&mut side, 13)), watch);
        assert!(fire(&mut side, 13).is_empty());
        drive(&mut side, 14, first::Event::Watch { watch, event: family::WatchEvent::Delivered(item()) });
        assert_eq!(object(&side).terminal, None);
        assert_eq!(first::next_deadline(&side), None);
        assert_eq!(object(&side).card, first::Card::Open);
    }
}
#[test]
fn family_denied_and_ended_departures_disable_acts_without_replacing_supplied_documents_or_keyed_outcomes() {
    for end in [family::WatchEnd::NotAllowed, family::WatchEnd::SubjectEnded] {
        let (mut side, read, watch) = start();
        drive(&mut side, 0, first::Event::Read { read, result: family::ReadResult::Value(item()) });
        let id = side.page().unwrap();
        let rows = drive(
            &mut side,
            1,
            first::Event::Act { act: first::Act::Touch { object: id.token(), revision: Revision { value: u64::MAX } } },
        );
        let request = rows
            .iter()
            .find_map(|r| match r {
                first::Request::Send { request, .. } => Some(*request),
                _ => None,
            })
            .unwrap();
        let answer = family::Answer::Touched { item: 5, revision: Revision { value: 17 } };
        drive(&mut side, 2, first::Event::Answered { request, answer: answer.clone() });
        drive(&mut side, 3, first::Event::Ended { watch, end });
        assert_eq!(object(&side).card, first::Card::Done { answer });
        assert_eq!(object(&side).value, first::Value::Known(item()));
        assert!(
            drive(
                &mut side,
                4,
                first::Event::Act {
                    act: first::Act::Touch { object: id.token(), revision: Revision { value: u64::MAX } }
                }
            )
            .is_empty()
        );
        drive(
            &mut side,
            4,
            first::Event::Act {
                act: first::Act::Parameters { object: id.token(), revision: Revision { value: u64::MAX }, value: 12 },
            },
        );
        assert_eq!(object(&side).draft, None);
        assert!(fire(&mut side, u64::MAX).is_empty());
    }
}
#[test]
fn family_late_closed_watch_and_read_callbacks_cannot_change_a_reloaded_object() {
    let (mut side, read, watch) = start();
    drive(&mut side, 0, first::Event::Left);
    let rows = drive(&mut side, 1, first::Event::Went { address: first::Address { project: 3, item: 5 } });
    let new_watch = opened(&rows);
    assert_ne!(new_watch, watch);
    let before = first::shown(&side);
    drive(&mut side, 2, first::Event::Watch { watch, event: family::WatchEvent::Delivered(item()) });
    drive(&mut side, 2, first::Event::Read { read, result: family::ReadResult::NotAllowed });
    assert_eq!(first::shown(&side), before);
    drive(&mut side, 3, first::Event::Ended { watch, end: family::WatchEnd::NotAllowed });
    assert_eq!(object(&side).terminal, None);
    drive(&mut side, 4, first::Event::Watch { watch: new_watch, event: family::WatchEvent::Delivered(item()) });
    assert_eq!(object(&side).value, first::Value::Known(item()));
}
#[test]
fn family_terminal_envelopes_fit_full_rows_and_object_capacity_and_signed_out_parks_until_confirmation() {
    for (end, terminal) in ends() {
        let (mut side, read, watch) = start();
        drive(&mut side, 0, first::Event::Read { read, result: family::ReadResult::Value(item()) });
        let rows = drive(
            &mut side,
            0,
            first::Event::Show { part: Part::Panel { connector: crate::FIRST, task: 6 }, scope: Scope::Project(3) },
        );
        let second_watch = opened(&rows);
        drive(&mut side, 1, first::Event::Ended { watch, end });
        assert_eq!(object(&side).terminal, Some(terminal));
        assert!(first::worst_case(&limits()).unwrap() > 0);
        let other = side.find(Subject { scope: Scope::Project(3), kind: Kind::Panel(6) }).unwrap();
        assert_eq!(side.object(other).unwrap().terminal, None);
        drive(&mut side, 2, first::Event::Watch { watch: second_watch, event: family::WatchEvent::Alive });
        let rows = drive(&mut side, 2, first::Event::Went { address: first::Address { project: 3, item: 5 } });
        if end == family::WatchEnd::SignedOut {
            assert!(!rows.iter().any(|r| match r {
                first::Request::Out(first::Out::Open { .. }) => true,
                _ => false,
            }));
            let (_, client) = super::root::live_first(limits());
            let mut out = Queue::uncapped();
            first::me::told(
                &mut side,
                &Env { now: Time::from_nanos(3), wall: Wall::EPOCH, limits: limits() },
                &client,
                &jig_client::Told::Changed { handle: first::me::handle() },
                &mut out,
            );
            let mut resumed = Vec::new();
            while let Some(row) = out.pop() {
                resumed.push(row);
            }
            assert_ne!(opened(&resumed), watch);
            assert_eq!(object(&side).card, first::Card::Open);
        }
        assert_eq!(object(&side).value, first::Value::Known(item()));
    }
}
