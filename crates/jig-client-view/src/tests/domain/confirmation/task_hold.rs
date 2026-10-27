use super::*;

fn owner(reason: HoldCategory) -> OwnerChipHold {
    let mut segments = List::with_capacity(2);
    segments.push(Box::<[u8]>::from(*b"a/b")).unwrap();
    segments.push(Box::<[u8]>::from([255_u8, 0])).unwrap();
    OwnerChipHold {
        circle: None,
        account: None,
        source: Some(OwnerPart {
            connector: u16::MAX,
            kind: OwnerPartKind::Resource,
            key: OwnerPartKey::Path(segments),
        }),
        reason,
        revision: u64::MAX,
        before: OwnerPriorPhase::Closing(ClosingStage::Effects),
        escalation: Some(OwnerHoldEscalation::Rejected {
            by: u64::MAX,
            reason: DocumentText { bytes: text(b"exact rejected reason"), cut: true },
        }),
        tries: OwnerFailureTries { transient: 1, permanent: 2, run: 3, agent: 4, lost: 5, invalid: u32::MAX },
        hold_wait_since: None,
    }
}

#[test]
fn owner_task_holds_retain_whole_read_facts_and_require_repeat_only_for_uncertain_entry() {
    for reason in [
        HoldCategory::Failures { class: HoldFailure::Invalid },
        HoldCategory::Stalled,
        HoldCategory::Dependency { task: u64::MAX },
        HoldCategory::Stopped,
        HoldCategory::Placed { hold: u64::MAX },
        HoldCategory::Drift,
        HoldCategory::Effects,
        HoldCategory::EffectFailed,
        HoldCategory::Uncertain { entry: u64::MAX },
        HoldCategory::Budget,
        HoldCategory::Deadline,
        HoldCategory::HoldsWaited,
        HoldCategory::ResourceUnavailable,
        HoldCategory::ResourceAccess,
        HoldCategory::PoolLost { pool: u64::MAX },
        HoldCategory::Procedure,
    ] {
        let uncertain = reason == HoldCategory::Uncertain { entry: u64::MAX };
        let exact = owner(reason);
        let (mut domain, _) = live();
        let mut row = task();
        row.hold = Some(TaskHold::Owner(Box::new(exact.clone())));
        row.person_revision = Some(Revision { value: 23 });
        row.acts = offered(ActKind::Release, Admission::Allowed);
        let id = install(&mut domain, Body::Task(Box::new(row.clone())));
        reread(&mut domain, Body::Task(Box::new(row.clone())), Read::Task { task: 8 });
        assert_eq!(domain.objects().get(id).unwrap().body, Some(Body::Task(Box::new(row.clone()))));
        assert_eq!(acts::hold_revision(&domain.objects().get(id).unwrap()), Some(Revision { value: u64::MAX }));
        assert!(intend(&mut domain, id, ActKind::Release).is_empty());
        assert_eq!(domain.confirmation().unwrap().missing, if uncertain { Some(Missing::Again) } else { None });
        if uncertain {
            assert!(drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm }).is_empty());
            drive(&mut domain, Time::ZERO, Event::Act { action: Act::Again { again: false } });
        }
        let output = drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm });
        assert_eq!(
            sent(&output),
            &Ask::Release {
                project: 1,
                task: 8,
                revision: Revision { value: u64::MAX },
                again: if uncertain { Some(false) } else { None },
            }
        );
        let Some(Body::Task(retained)) = &domain.objects().get(id).unwrap().body else { panic!("whole task") };
        assert_eq!(retained.hold, Some(TaskHold::Owner(Box::new(exact))));
        assert_eq!(retained.person_revision, Some(Revision { value: 23 }));
        assert_eq!(retained.spec_revision, Revision { value: 9 });
    }
}

#[test]
fn owner_task_hold_live_revision_moves_independently_before_atomic_amend_release() {
    let (mut domain, _) = live();
    let mut row = task();
    let mut held = owner(HoldCategory::Budget);
    held.revision = u64::MAX - 1;
    row.hold = Some(TaskHold::Owner(Box::new(held.clone())));
    row.acts = offered(ActKind::Amend, Admission::Allowed);
    let id = install(&mut domain, Body::Task(Box::new(row.clone())));
    intend_amend_release(&mut domain, id);
    edit(&mut domain, Purpose::Reason, b"");
    drive(&mut domain, Time::ZERO, Event::Act { action: Act::Change { change: Box::new(change()) } });
    assert!(domain.confirmation().unwrap().missing.is_none());
    held.revision = u64::MAX;
    held.before = OwnerPriorPhase::Active(RunStage::Running { attempt: u64::MAX });
    held.hold_wait_since = Some(Wall::from_nanos(u64::MAX));
    row.hold = Some(TaskHold::Owner(Box::new(held.clone())));
    reread(&mut domain, Body::Task(Box::new(row.clone())), Read::Task { task: 8 });
    assert_eq!(domain.confirmation().unwrap().revision, Some(Revision { value: 9 }));
    assert_eq!(
        domain.confirmation().unwrap().missing,
        Some(Missing::ChangedHold { now: Some(Revision { value: u64::MAX }) })
    );
    assert!(drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm }).is_empty());
    intend_amend_release(&mut domain, id);
    edit(&mut domain, Purpose::Reason, b"");
    drive(&mut domain, Time::ZERO, Event::Act { action: Act::Change { change: Box::new(change()) } });
    let output = drive(&mut domain, Time::ZERO, Event::Act { action: Act::Confirm });
    assert_eq!(
        sent(&output),
        &crate::tests::fixtures::releasing(
            Ask::Amend {
                reason: text(b""),
                project: 1,
                task: 8,
                revision: Some(Revision { value: 9 }),
                change: Box::new(change()),
            },
            Some(Revision { value: u64::MAX })
        )
    );
    let Some(Body::Task(retained)) = &domain.objects().get(id).unwrap().body else { panic!("whole task") };
    assert_eq!(retained.hold, Some(TaskHold::Owner(Box::new(held))));
}
