use super::{
    objects,
    spine::{LIMITS, drive},
};
use crate::tests::fixtures::{Ended, Fixture, ReadPage, Watched};
use crate::{
    Event, Request,
    address::{Address, Place, TaskSection},
    objects::Body,
};
use jig_client::contract::*;
use skein_lib::{List, Time};

fn list<T>(values: impl IntoIterator<Item = T>) -> List<T> {
    let mut out = List::with_capacity(2);
    for value in values {
        out.push(value).ok().unwrap();
    }
    out
}
fn account() -> OwnerPart {
    OwnerPart { connector: u16::MAX, kind: OwnerPartKind::Hold, key: OwnerPartKey::Number(u64::MAX) }
}
fn origin() -> OwnerPart {
    OwnerPart {
        connector: 2,
        kind: OwnerPartKind::Resource,
        key: OwnerPartKey::Path(list([Box::from([255_u8, 0]), Box::from(*b"a/b")])),
    }
}
fn chip(account: Option<OwnerPart>) -> Chip {
    let mut details = super::chip_facts::details();
    details.project = 41;
    details.hold = Some(OwnerChipHold {
        words: None,
        placed: None,
        circle: None,
        account,
        source: Some(origin()),
        reason: HoldCategory::Drift,
        revision: 91,
        before: OwnerPriorPhase::Waiting,
        escalation: None,
        tries: OwnerFailureTries { transient: 0, permanent: 0, run: 0, agent: 0, lost: 0, invalid: 0 },
        hold_wait_since: None,
    });
    let mut chip = objects::chip(8);
    chip.details = Box::new(ChipPhase::Owner {
        phase: OwnerChipPhase::Held { reason: HoldCategory::Drift, revision: 91 },
        details: Box::new(details),
    });
    chip
}
fn task(account: Option<OwnerPart>) -> Task {
    let mut task = super::confirmation::task();
    task.project = 41;
    task.chip = chip(account);
    task
}
fn part() -> Part {
    Part::Hold { connector: u16::MAX, task: u64::MAX }
}
fn read(output: &[Request], expected: Read) -> jig_client::Handle {
    let mut found = None;
    for request in output {
        if let Request::Client {
            asked: jig_client::Event::Read { handle: read, read: jig_client::Readable::Jig(what) },
        } = request
            && *what == expected
        {
            assert!(found.replace(*read).is_none());
        }
    }
    found.unwrap()
}
fn app(domain: &mut Fixture, key: ObjectKey) -> Box<[Request]> {
    drive(domain, Time::ZERO, Event::Went { place: Place::Elsewhere });
    drive(domain, Time::ZERO, Event::Named { sections: List::with_capacity(0), objects: list([key]) })
}
fn task_watch(domain: &mut Fixture, section: TaskSection) -> jig_client::Handle {
    let output = drive(domain, Time::ZERO, Event::Went { place: Place::Jig(Address::Task { task: 8, section }) });
    let mut found = None;
    for request in &output {
        if let Request::Client {
            asked:
                jig_client::Event::Watch { handle: watch, subject: jig_client::Watchable::Jig(Subject::Task { task: 8 }) },
        } = request
        {
            assert!(found.replace(*watch).is_none());
        }
    }
    found.unwrap()
}

fn expected(_generation: u32) -> Request {
    Request::ShowPart { part: part(), scope: PartScope::Project(41) }
}
fn bounded(domain: &mut Fixture, count: u32, event: impl crate::tests::fixtures::Input) -> Box<[Request]> {
    domain.ui.limits.parts = count;
    domain.drive(Time::ZERO, event)
}

#[test]
fn native_hold_account_is_shown_once_by_whole_task_reads_in_every_section_and_leaves_with_its_generation() {
    for section in [
        TaskSection::Overview,
        TaskSection::Activity,
        TaskSection::Plan,
        TaskSection::History,
        TaskSection::Budget,
        TaskSection::Panels,
    ] {
        let (mut domain, _) = objects::live();
        let watch = task_watch(&mut domain, section);
        let request = drive(&mut domain, Time::ZERO, Ended { watch, end: WatchEnd::Unknown });
        let token = read(&request, Read::Task { task: 8 });
        let mut exact = task(Some(account()));
        let context = exact.context.as_mut().unwrap();
        context.impact.tasks.waiting = 1;
        context.parts = list([part()]);
        let shown = drive(
            &mut domain,
            Time::ZERO,
            ReadPage { read: token, result: ReadResult::Value(ReadValue::Task(Box::new(exact.clone()))) },
        );
        assert_eq!(crate::tests::fixtures::parts(&shown, LIMITS.parts).as_ref(), &[expected(1)]);
        let id = domain.objects().find(ObjectKey::Task(8)).unwrap();
        assert_eq!(domain.objects().get(id).unwrap().body, Some(Body::Task(Box::new(exact.clone()))));
        let left = drive(&mut domain, Time::ZERO, Event::Went { place: Place::Elsewhere });
        assert!(left.contains(&Request::PartsLeft));
        assert!(domain.task_page().header.is_none());
        assert!(
            drive(
                &mut domain,
                Time::ZERO,
                ReadPage { read: token, result: ReadResult::Value(ReadValue::Task(Box::new(exact))) }
            )
            .is_empty()
        );
        assert!(domain.objects().find(ObjectKey::Task(8)).is_none());
    }
}

#[test]
fn native_hold_account_application_chips_use_supplied_scope_and_whole_task_and_escalation_reads_register_once() {
    for cached in [false, true] {
        let (mut domain, _) = objects::live();
        let shown = app(&mut domain, ObjectKey::Task(8));
        let mut watch = None;
        for request in &shown {
            if let Request::Client {
                asked:
                    jig_client::Event::Watch { handle: found, subject: jig_client::Watchable::Jig(Subject::Chips { .. }) },
            } = request
            {
                assert!(watch.replace(*found).is_none());
            }
        }
        let watch = watch.unwrap();
        let mut old = super::confirmation::task();
        old.project = 7;
        if cached {
            let token = read(&shown, Read::Task { task: 8 });
            drive(
                &mut domain,
                Time::ZERO,
                ReadPage { read: token, result: ReadResult::Value(ReadValue::Task(Box::new(old.clone()))) },
            );
        }
        let exact = chip(Some(account()));
        let shown = drive(
            &mut domain,
            Time::ZERO,
            Watched { watch, event: WatchEvent::Delivered(Delivery::Chips(list([exact.clone()]))) },
        );
        assert_eq!(crate::tests::fixtures::parts(&shown, LIMITS.parts).as_ref(), &[expected(1)]);
        let id = domain.objects().find(ObjectKey::Task(8)).unwrap();
        let body = if cached {
            old.chip = exact.clone();
            Body::Task(Box::new(old))
        } else {
            Body::Chip(exact.clone())
        };
        assert_eq!(domain.objects().get(id).unwrap().body, Some(body));
        let left = drive(
            &mut domain,
            Time::ZERO,
            Event::Named { objects: List::with_capacity(0), sections: List::with_capacity(0) },
        );
        assert!(left.contains(&Request::Client { asked: jig_client::Event::Unwatch { handle: watch } }));
        let late = drive(
            &mut domain,
            Time::ZERO,
            Watched { watch, event: WatchEvent::Delivered(Delivery::Chips(list([exact]))) },
        );
        assert!(crate::tests::fixtures::parts(&late, LIMITS.parts).is_empty());
        assert!(domain.objects().find(ObjectKey::Task(8)).is_none());
    }
    for escalation in [false, true] {
        let (mut domain, _) = objects::live();
        let key = if escalation { ObjectKey::Escalation(8) } else { ObjectKey::Task(8) };
        let shown = app(&mut domain, key);
        let query = if escalation { Read::Escalation { task: 8 } } else { Read::Task { task: 8 } };
        let token = read(&shown, query);
        let exact = task(Some(account()));
        let (value, body) = if escalation {
            let value = OwnerEscalation { task: Box::new(exact), acts: List::with_capacity(0) };
            (ReadValue::OwnerEscalation(Box::new(value.clone())), Body::OwnerEscalation(Box::new(value)))
        } else {
            (ReadValue::Task(Box::new(exact.clone())), Body::Task(Box::new(exact)))
        };
        let shown = drive(&mut domain, Time::ZERO, ReadPage { read: token, result: ReadResult::Value(value) });
        assert_eq!(crate::tests::fixtures::parts(&shown, LIMITS.parts).as_ref(), &[expected(1)]);
        let id = domain.objects().find(key).unwrap();
        assert_eq!(domain.objects().get(id).unwrap().body, Some(body));
    }
}

#[test]
fn native_hold_account_absence_other_families_and_path_keys_never_invent_a_hold_from_the_independent_origin() {
    let mut accounts = List::with_capacity(8);
    accounts.push(None).unwrap();
    accounts.push(Some(OwnerPart { key: origin().key, ..account() })).unwrap();
    for kind in [
        OwnerPartKind::Resource,
        OwnerPartKind::Pool,
        OwnerPartKind::Result,
        OwnerPartKind::Procedure,
        OwnerPartKind::Effect,
        OwnerPartKind::Outbox,
    ] {
        accounts.push(Some(OwnerPart { kind, ..account() })).unwrap();
    }
    for account in accounts.into_boxed() {
        for application in [false, true] {
            let (mut domain, _) = objects::live();
            let exact = task(account.clone());
            let shown = if application {
                let shown = app(&mut domain, ObjectKey::Task(8));
                let token = read(&shown, Read::Task { task: 8 });
                drive(
                    &mut domain,
                    Time::ZERO,
                    ReadPage { read: token, result: ReadResult::Value(ReadValue::Task(Box::new(exact.clone()))) },
                )
            } else {
                let watch = task_watch(&mut domain, TaskSection::Overview);
                drive(
                    &mut domain,
                    Time::ZERO,
                    Watched { watch, event: WatchEvent::Delivered(Delivery::Task(Box::new(exact.clone()))) },
                )
            };
            assert!(crate::tests::fixtures::parts(&shown, LIMITS.parts).is_empty());
            let id = domain.objects().find(ObjectKey::Task(8)).unwrap();
            assert_eq!(domain.objects().get(id).unwrap().body, Some(Body::Task(Box::new(exact))));
        }
    }
}

#[test]
fn native_hold_accounts_obey_bounded_application_and_task_parts_without_dropping_whole_owner_facts() {
    for escalation in [false, true] {
        let (mut domain, _) = objects::live();
        let key = if escalation { ObjectKey::Escalation(8) } else { ObjectKey::Task(8) };
        let shown = app(&mut domain, key);
        let query = if escalation { Read::Escalation { task: 8 } } else { Read::Task { task: 8 } };
        let token = read(&shown, query);
        let mut exact = task(Some(account()));
        let independent = Part::Outbox { connector: 3, entry: 5, scope: PartScope::Deployment };
        exact.resources = list([TaskResource::Part(independent)]);
        let (value, body) = if escalation {
            let value = OwnerEscalation { task: Box::new(exact), acts: List::with_capacity(0) };
            (ReadValue::OwnerEscalation(Box::new(value.clone())), Body::OwnerEscalation(Box::new(value)))
        } else {
            (ReadValue::Task(Box::new(exact.clone())), Body::Task(Box::new(exact)))
        };
        let shown = bounded(&mut domain, 1, ReadPage { read: token, result: ReadResult::Value(value) });
        assert_eq!(crate::tests::fixtures::parts(&shown, LIMITS.parts).as_ref(), &[expected(1)]);
        let (notice, ()) = domain.notices().ids().iter().next().unwrap();
        assert_eq!(domain.notices().get(*notice).unwrap().kind, crate::notices::kept::Kind::Refused(Limit::Objects));
        let id = domain.objects().find(key).unwrap();
        assert_eq!(domain.objects().get(id).unwrap().body, Some(body));
    }
    let (mut domain, _) = objects::live();
    let watch = task_watch(&mut domain, TaskSection::Overview);
    let exact = task(Some(account()));
    let shown = bounded(
        &mut domain,
        0,
        Watched { watch, event: WatchEvent::Delivered(Delivery::Task(Box::new(exact.clone()))) },
    );
    assert!(crate::tests::fixtures::parts(&shown, LIMITS.parts).is_empty());
    if let crate::page::Page::Work { refused, .. } = domain.page() {
        assert_eq!(*refused, Some(Limit::Objects));
    } else {
        panic!("task page retained with its bounded refusal");
    }
    let id = domain.objects().find(ObjectKey::Task(8)).unwrap();
    assert_eq!(domain.objects().get(id).unwrap().body, Some(Body::Task(Box::new(exact))));
}
