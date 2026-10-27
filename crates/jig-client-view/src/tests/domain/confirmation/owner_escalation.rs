use super::*;
use crate::tests::fixtures::ReadPage;

fn row(act: ActKind, reason: HoldCategory) -> OwnerEscalation {
    let mut task = task();
    task.context = None;
    task.person_revision = Some(Revision { value: 23 });
    task.taken_by = Some(u64::MAX);
    task.dependencies = List::with_capacity(2);
    task.dependencies.push(OwnerTaskLink::Stored { number: 99, status: Some(OwnerTaskStatus::Done) }).unwrap();
    task.inputs = List::with_capacity(2);
    task.inputs.push(OwnerTaskLink::Stored { number: 31, status: None }).unwrap();
    task.references = List::with_capacity(2);
    task.references.push(OwnerTaskLink::Live(chip(32))).unwrap();
    task.delegates = List::with_capacity(2);
    task.delegates.push(LiveDelegate { chip: chip(33), takeover_budget: u64::MAX }).unwrap();
    task.lineage_chips = List::with_capacity(2);
    task.lineage_chips.push(chip(34)).unwrap();
    task.resources = List::with_capacity(2);
    let mut path = List::with_capacity(2);
    path.push(Box::<[u8]>::from(*b"whole-resource")).unwrap();
    task.resources.push(TaskResource::Part(Part::Resource { connector: 7, path })).unwrap();
    task.result = Some(TaskResult::Ending(Box::new(OwnerChipEnding::Failed(DocumentText::complete(text(
        b"supplied retained task failure",
    ))))));
    let held = OwnerChipHold {
        words: None,
        placed: None,
        circle: None,
        account: None,
        source: None,
        reason,
        revision: u64::MAX,
        before: OwnerPriorPhase::Closing(ClosingStage::Effects),
        escalation: Some(OwnerHoldEscalation::Waiting {
            holder: OwnerEscalationHolder::Person(7),
            entry: 22,
            since: Wall::EPOCH,
        }),
        tries: OwnerFailureTries { transient: 1, permanent: 2, run: 3, agent: 4, lost: 5, invalid: 6 },
        hold_wait_since: None,
    };
    let mut details = super::super::chip_facts::details();
    details.project = task.project;
    details.hold = Some(held.clone());
    task.chip.details = Box::new(ChipPhase::Owner {
        phase: OwnerChipPhase::Held { reason, revision: held.revision },
        details: Box::new(details),
    });
    task.hold = Some(TaskHold::Owner(Box::new(held)));
    task.acts = offered(ActKind::Stop, Admission::Allowed);
    OwnerEscalation { acts: offered(act, Admission::Allowed), task: Box::new(task) }
}

#[test]
fn owner_escalation_original_reads_keep_whole_task_and_independent_offers_and_revisions() {
    let (mut domain, page) = live();
    let key = ObjectKey::Escalation(8);
    let output = super::super::objects::watched(&mut domain, page, &[key]);
    let read = super::super::objects::read_token(&output);
    let original = row(ActKind::Release, HoldCategory::Budget);
    drive(
        &mut domain,
        Time::ZERO,
        ReadPage { read, result: ReadResult::Value(ReadValue::OwnerEscalation(Box::new(original.clone()))) },
    );
    let id = domain.objects().find(key).unwrap();
    let object = domain.objects().get(id).unwrap();
    assert_eq!(object.project, Some(1));
    assert_eq!(object.body, Some(Body::OwnerEscalation(Box::new(original.clone()))));
    assert_eq!(acts::offered(&object, ActKind::Release), Admission::Allowed);
    assert_eq!(acts::offered(&object, ActKind::Stop), Admission::Refused(Refusal::NotOffered));
    assert_eq!(acts::revision(&object, ActKind::Release), Some(Revision { value: u64::MAX }));
    assert_eq!(acts::revision(&object, ActKind::Amend), Some(Revision { value: 9 }));
    assert_eq!(object.body.as_ref().unwrap().terminal(), None);
    assert_eq!(original.task.context, None);
    assert_eq!(original.task.person_revision, Some(Revision { value: 23 }));
    assert_eq!(original.task.dependencies.capacity(), 2);
    assert_eq!(original.task.delegates.get(0).unwrap().chip.task, 33);
    assert_eq!(original.task.references.get(0).unwrap().number(), 32);
    assert!(original.task.result.is_some());
    assert_eq!(object.body.as_ref().unwrap().ending(), None);
    assert_eq!(original.task.taken_by.unwrap(), u64::MAX);
}

#[test]
fn owner_escalation_wrong_task_or_project_never_consumes_the_original_read_and_old_tabs_stay_old() {
    let (mut domain, page) = live();
    let key = ObjectKey::Escalation(8);
    let output = super::super::objects::watched(&mut domain, page, &[key]);
    let mut read = super::super::objects::read_token(&output);
    let original = row(ActKind::Release, HoldCategory::Budget);
    for wrong_scope in [false, true] {
        let mut wrong = original.clone();
        if wrong_scope {
            wrong.task.project = 2;
        } else {
            wrong.task.chip.task = 99;
        }
        drive(
            &mut domain,
            Time::ZERO,
            ReadPage { read, result: ReadResult::Value(ReadValue::OwnerEscalation(Box::new(wrong))) },
        );
        assert!(domain.objects().get(domain.objects().find(key).unwrap()).unwrap().body.is_none());
        assert!(domain.handles.reads.get(&read).is_none());
        read = domain.read_again(read);
    }
    drive(
        &mut domain,
        Time::ZERO,
        ReadPage { read, result: ReadResult::Value(ReadValue::OwnerEscalation(Box::new(original.clone()))) },
    );
    let id = domain.objects().find(key).unwrap();
    assert_eq!(domain.objects().get(id).unwrap().body, Some(Body::OwnerEscalation(Box::new(original.clone()))));
    let mut wrong = original.clone();
    wrong.task.activity = u64::MAX;
    drive(
        &mut domain,
        Time::ZERO,
        ReadPage { read, result: ReadResult::Value(ReadValue::OwnerEscalation(Box::new(wrong))) }.late(),
    );
    assert_eq!(domain.objects().get(id).unwrap().body, Some(Body::OwnerEscalation(Box::new(original.clone()))));
    drive(&mut domain, Time::ZERO, Event::Went { place: crate::address::Place::Elsewhere });
    drive(
        &mut domain,
        Time::ZERO,
        ReadPage { read, result: ReadResult::Value(ReadValue::OwnerEscalation(Box::new(original))) }.late(),
    );
    assert!(domain.objects().find(key).is_none());
}

#[test]
fn owner_escalation_decisions_name_supplied_hold_and_outer_offers_without_guessing_destinations() {
    for act in [ActKind::Release, ActKind::LeaveHeld, ActKind::PassUp] {
        let (mut domain, _) = live();
        let original = row(act, HoldCategory::Uncertain { entry: 37 });
        let id = install(&mut domain, Body::OwnerEscalation(Box::new(original.clone())));
        intend(&mut domain, id, act);
        if act == ActKind::Release {
            assert_eq!(domain.confirmation().unwrap().missing, Some(Missing::Again));
            assert!(drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm }).is_empty());
            drive(&mut domain, Time::ZERO, Event::Act { action: Act::Again { again: false } });
        }
        if act == ActKind::LeaveHeld {
            edit(&mut domain, Purpose::Reason, b"keep this held");
        }
        let output = drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm });
        let expected = match act {
            ActKind::Release => {
                Ask::Release { project: 1, task: 8, revision: Revision { value: u64::MAX }, again: Some(false) }
            }
            ActKind::LeaveHeld => Ask::Decide {
                project: 1,
                on: DecideOn::Escalation {
                    task: 8,
                    revision: Revision { value: u64::MAX },
                    decision: Decision::LeftHeld { reason: text(b"keep this held") },
                },
            },
            ActKind::PassUp => Ask::Decide {
                project: 1,
                on: DecideOn::Escalation {
                    task: 8,
                    revision: Revision { value: u64::MAX },
                    decision: Decision::PassedUp,
                },
            },
            ActKind::Withdraw
            | ActKind::Message
            | ActKind::Answer
            | ActKind::Take
            | ActKind::HandBack
            | ActKind::Accept
            | ActKind::Reject
            | ActKind::Amend
            | ActKind::Stop
            | ActKind::Cancel
            | ActKind::Move
            | ActKind::Reorder
            | ActKind::Seen
            | ActKind::EditNote
            | ActKind::DeleteNote
            | ActKind::Adopt
            | ActKind::Rename
            | ActKind::ChangePolicy
            | ActKind::SetRoles
            | ActKind::MakeService
            | ActKind::SetPool => unreachable!(),
        };
        assert_eq!(sent(&output), &expected);
        assert_eq!(domain.objects().get(id).unwrap().body, Some(Body::OwnerEscalation(Box::new(original))));
    }
    let (mut domain, _) = live();
    let id = install(&mut domain, Body::OwnerEscalation(Box::new(row(ActKind::Release, HoldCategory::Budget))));
    intend(&mut domain, id, ActKind::PassUp);
    assert!(domain.confirmation().is_none());
    assert_eq!(domain.objects().get(id).unwrap().card, Card::Refused { refusal: Refusal::NotOffered });
}

#[test]
fn owner_escalation_atomic_amend_release_preserves_supplied_reason_and_independent_whole_baseline() {
    let (mut domain, _) = live();
    let mut original = row(ActKind::Amend, HoldCategory::Budget);
    original.acts = offered(
        ActKind::Amend,
        Admission::Bounded {
            within: Bounds::Amend {
                budget_left: 0,
                implications: List::with_capacity(0),
                grants: List::with_capacity(0),
                authority: Box::new(original.task.authority.clone()),
                ceiling: Box::new(original.task.authority.clone()),
            },
            beyond: Beyond::Refused(Refusal::NotOffered),
        },
    );
    let id = install(&mut domain, Body::OwnerEscalation(Box::new(original.clone())));
    intend_amend_release(&mut domain, id);
    drive(&mut domain, Time::ZERO, Event::Act { action: Act::Change { change: Box::new(change()) } });
    assert_eq!(domain.confirmation().unwrap().missing, Some(Missing::Words { field: Purpose::Reason }));
    edit(&mut domain, Purpose::Reason, b"");
    let output = drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm });
    assert_eq!(
        sent(&output),
        &crate::tests::fixtures::releasing(
            Ask::Amend {
                project: 1,
                task: 8,
                revision: Some(Revision { value: 9 }),
                change: Box::new(change()),
                reason: text(b""),
            },
            Some(Revision { value: u64::MAX })
        )
    );
    assert_eq!(domain.objects().get(id).unwrap().body, Some(Body::OwnerEscalation(Box::new(original))));
}

use jig_client::contract::DecideOn;
