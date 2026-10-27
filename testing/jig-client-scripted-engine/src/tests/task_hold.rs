use super::*;
use crate::payload::{self, Payload};
use core::mem::size_of;

#[test]
fn task_hold_parent_prices_whole_owner_box_reserved_source_and_rejected_words_at_exact_bounds() {
    let bounds = Bounds { rows: 2, segments: 2, text: 32, document_bytes: u64::MAX, ..Bounds::default() };
    let mut row = task();
    row.hold = None;
    let absent = row.heap(&bounds).unwrap();
    let mut path = List::with_capacity(2);
    path.push(Box::<[u8]>::from([255_u8; 32])).unwrap();
    let held = OwnerChipHold {
        placed: None,
        circle: None,
        account: None,
        source: Some(OwnerPart { connector: u16::MAX, kind: OwnerPartKind::Resource, key: OwnerPartKey::Path(path) }),
        reason: HoldCategory::Uncertain { entry: u64::MAX },
        revision: u64::MAX,
        before: OwnerPriorPhase::Closing(ClosingStage::Effects),
        escalation: Some(OwnerHoldEscalation::Rejected {
            by: u64::MAX,
            reason: DocumentText { bytes: text(UTF8_TEXTS[0]), cut: true },
        }),
        tries: OwnerFailureTries { transient: 1, permanent: 2, run: 3, agent: 4, lost: 5, invalid: u32::MAX },
        hold_wait_since: None,
    };
    row.hold = Some(TaskHold::Owner(Box::new(held.clone())));
    let expected =
        u64::try_from(size_of::<OwnerChipHold>()).unwrap() + 2 * u64::try_from(size_of::<Box<[u8]>>()).unwrap() + 64;
    assert_eq!(row.heap(&bounds).unwrap() - absent, expected);
    let exact = u64::try_from(size_of::<Task>()).unwrap() + absent + expected;
    let before = row.clone();
    assert!(payload::fits(&row, &Bounds { document_bytes: exact, ..bounds }));
    assert!(!payload::fits(&row, &Bounds { document_bytes: exact - 1, ..bounds }));
    assert!(!payload::fits(&row, &Bounds { rows: 1, segments: 1, ..bounds }));
    assert!(!payload::fits(&row, &Bounds { text: 31, ..bounds }));
    assert_eq!(row, before);
    assert_eq!(row.hold, Some(TaskHold::Owner(Box::new(held))));
    let document = Hold {
        placed: None,
        revision: Revision { value: 7 },
        reason: HoldReason::Budget { spent: 1, limit: 1 },
        before: Phase::Running,
        since: Wall::EPOCH,
        result: Some(ResultValue::Report(DocumentText::complete(text(b"legacy result")))),
        spend: List::with_capacity(2),
    };
    let document_heap = document.heap(&bounds).unwrap();
    row.hold = Some(TaskHold::Document(Box::new(document.clone())));
    assert_eq!(row.heap(&bounds).unwrap() - absent, u64::try_from(size_of::<Hold>()).unwrap() + document_heap);
    assert_eq!(row.hold, Some(TaskHold::Document(Box::new(document))));
}

#[test]
fn scripted_owner_hold_moves_return_exact_category_for_release_and_atomic_amend_release() {
    for amendment in [false, true] {
        let mut setup = script();
        let mut row = task();
        let reason = HoldCategory::Failures { class: HoldFailure::Agent };
        row.hold = Some(TaskHold::Owner(Box::new(OwnerChipHold {
            placed: None,
            circle: None,
            account: None,
            source: None,
            reason,
            revision: 7,
            before: OwnerPriorPhase::Waiting,
            escalation: None,
            tries: OwnerFailureTries { transient: 1, permanent: 2, run: 3, agent: 4, lost: 5, invalid: 6 },
            hold_wait_since: None,
        })));
        row.acts = list(vec![Offered {
            effect: None,
            act: if amendment { ActKind::Amend } else { ActKind::Release },
            admission: Admission::Allowed,
        }]);
        let spec_revision = row.spec_revision;
        setup.truth.objects.insert(ObjectKey::Task(8), ReadValue::Task(Box::new(row)));
        let mut engine = Engine::new(setup, 1).unwrap();
        let ask = if amendment {
            crate::tests::releasing(
                Ask::Amend {
                    reason: text(b""),
                    project: 1,
                    task: 8,
                    revision: Some(spec_revision),

                    change: Box::new(Amendment {
                        llm: None,
                        addressed: None,
                        spec: None,
                        wake: None,
                        dependencies: None,
                        authority: None,
                        instructions: Some(text(b"")),
                        tracked: None,
                    }),
                },
                Revision { value: 6 },
            )
        } else {
            Ask::Release { project: 1, task: 8, revision: Revision { value: 6 }, again: None }
        };
        send(&mut engine, 0, 11, 1, 1, ask);
        assert_eq!(
            final_answer(&engine.poll(Time::from_nanos(10_000_000))[0]),
            &Answer::Refused {
                refusal: Refusal::Moved {
                    now: Revision { value: 7 },
                    what: Moved::Hold { reason: Some(CurrentHold::Reason(reason)) },
                },
                at: None
            }
        );
    }
}

const UTF8_TEXTS: [&[u8]; 1] = [&[b'!'; 32]];
