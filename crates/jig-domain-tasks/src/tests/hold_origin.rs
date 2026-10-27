//! Public task entrances preserve immutable connector reference facts.

use super::revisions::{Harness, LIMITS, task, to};
use crate::{Active, Cause, Control, Domain, End, Event, Hold, HoldSource, Key, Name, Party, Phase, Request, Stored};
use alloc::boxed::Box;
use skein_lib::Wall;

fn resource(words: &[u8]) -> HoldSource {
    HoldSource::Resource(Name { connector: 7, path: Box::new([words.into()]) })
}
fn origin(phase: &Phase) -> Option<&HoldSource> {
    match phase {
        Phase::Held { source, .. } => source.as_deref(),
        Phase::Waiting | Phase::Active(_) | Phase::Closing(_) | Phase::Ended(_) => None,
    }
}

#[test]
fn hold_origin_survives_restart_terminal_and_release_in_exact_milestones() {
    let mut h = Harness::new();
    h.make([task(1, &[])].into());
    h.send(Event::Prepare { reply_to: to(), task: 1 });
    h.send(Event::Claim {
        account: None,
        reply_to: to(),
        task: 1,
        attempt: 1,
        resumed: false,
        budget: 10,
        writes: Box::new([]),
    });
    h.env.wall = Wall::from_nanos(10);
    h.send(Event::HoldFrom { task: 1, why: Hold::Drift, source: Box::new(resource(b"original")) });
    let first = h.domain.task(1).unwrap().clone();
    h.send(Event::Hold { task: 1, why: Hold::Drift });
    h.send(Event::HoldFrom { task: 1, why: Hold::Drift, source: Box::new(resource(b"original")) });
    assert_eq!(h.domain.task(1), Some(&first), "duplicates cannot erase the source or move clocks");
    h.env.wall = Wall::from_nanos(12);
    h.send(Event::HoldFrom { task: 1, why: Hold::Drift, source: Box::new(resource(b"changed")) });
    assert_eq!(h.domain.task(1).unwrap().revisions.hold, first.revisions.hold + 1);
    assert_eq!(h.domain.task(1).unwrap().phase_since, h.env.wall);
    h.route(1, crate::EscalationHolder::Person(9), 1);
    h.restart();
    h.send(Event::Activation {
        reply_to: to(),
        task: 1,
        attempt: 1,
        end: End::Parked,
        saved: None,
        cause: Cause::Unpriced,
    });
    let row = h.domain.task(1).unwrap();
    assert_eq!(origin(&row.phase), Some(&resource(b"changed")));
    let position = row.milestone;
    h.send(Event::Control {
        reply_to: to(),
        by: Party::Person(9),
        task: 1,
        control: Control::Release { again: None, message: None },
    });
    assert_eq!(h.domain.task(1).unwrap().phase, Phase::Active(Active::Due));
    let Some(Stored::Milestone(retained)) = h.stored.get(&Key::Milestone { task: 1, position }) else {
        panic!("held milestone retained")
    };
    assert_eq!(origin(&retained.phase), Some(&resource(b"changed")));
    let Some(Stored::Milestone(retained)) = h.stored.get(&Key::Milestone { task: 1, position: first.milestone }) else {
        panic!("original milestone retained")
    };
    assert_eq!(retained.phase, first.phase);
}

#[test]
fn hold_origin_shape_is_checked_on_admission_and_cold_restore() {
    let mut h = Harness::new();
    h.make([task(1, &[])].into());
    let before = h.domain.task(1).unwrap().clone();
    for (why, source) in [
        (Hold::Budget, Box::new(resource(b"wrong reason"))),
        (Hold::Drift, Box::new(resource(&[1; 65]))),
        (Hold::Uncertain { entry: 3 }, Box::new(HoldSource::Effect { connector: 7, entry: 4 })),
        (Hold::EffectFailed, Box::new(HoldSource::Effect { connector: 7, entry: 0 })),
    ] {
        h.send(Event::HoldFrom { task: 1, why, source: source.clone() });
        assert_eq!(h.domain.task(1), Some(&before));
        let mut broken = before.clone();
        broken.phase = Phase::Held { was: crate::Was::Active(Active::Due), why, source: Some(source) };
        broken.revisions.hold = 1;
        broken.escalation = crate::Escalation::Waiting {
            revision: 1,
            holder: crate::EscalationHolder::Person(9),
            entry: 1,
            since: Wall::EPOCH,
        };
        let mut cold = Harness::new();
        cold.domain = Domain::new(&LIMITS, 91, 1, crate::tests::charters(), Box::new([0]));
        let mut refused = false;
        for request in cold.send(Event::Restore { record: Stored::Live(Box::new(broken)) }) {
            let Request::RestoreRefused { .. } = request else { continue };
            refused = true;
        }
        assert!(refused);
    }
    h.send(Event::HoldFrom {
        task: 1,
        why: Hold::Uncertain { entry: 3 },
        source: Box::new(HoldSource::Effect { connector: 7, entry: 3 }),
    });
    h.route(1, crate::EscalationHolder::Person(9), 1);
    h.restart();
    assert_eq!(origin(&h.domain.task(1).unwrap().phase), Some(&HoldSource::Effect { connector: 7, entry: 3 }));
}

#[test]
fn connector_accounts_are_nonempty_bounded_and_name_the_uncertain_entry() {
    let limits = LIMITS;
    for (size, valid) in [(0, false), (1, true), (512, true), (513, false)] {
        let source = HoldSource::Account { connector: 7, entry: Some(3), words: Box::from(&[b'x'; 513][..size]) };
        assert_eq!(crate::hold_origin::valid(&limits, Hold::Uncertain { entry: 3 }, Some(&source)), valid);
        assert!(!crate::hold_origin::valid(&limits, Hold::Uncertain { entry: 4 }, Some(&source)));
    }
}
