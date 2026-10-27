use super::{bytes, fits, worst_case};
use crate::Bounds;
use core::mem::size_of;
use jig_client_domain::contract::*;
use skein_lib::{List, Wall, bytes::copy_of};
fn doc(bytes: &[u8], cut: bool) -> DocumentText {
    DocumentText { bytes: Text::new(copy_of(bytes), 8).unwrap(), cut }
}
fn line(phase: OwnerMilestonePhase) -> OwnerMilestoneLine {
    OwnerMilestoneLine { task: u64::MAX, position: 0, phase, at: None }
}
fn part(capacity: u32) -> OwnerPart {
    let mut path = List::with_capacity(capacity);
    if capacity >= 3 {
        for bytes in [b"a/b".as_slice(), &[255, 0], b""] {
            path.push(copy_of(bytes)).unwrap();
        }
    }
    OwnerPart { connector: u16::MAX, kind: OwnerPartKind::Resource, key: OwnerPartKey::Path(path) }
}
fn bounds() -> Bounds {
    Bounds { text: 8, rows: 3, segments: 3, document_bytes: 65536, ..Bounds::default() }
}
#[test]
fn owner_milestone_recursive_heap_counts_reserved_fields_parts_and_bytes_at_exact_and_adjacent_whole_bounds() {
    let mut fields = List::with_capacity(3);
    fields.push(ResultField { name: copy_of(b"name"), value: copy_of(b"value") }).unwrap();
    let source = line(OwnerMilestonePhase::Held {
        before: OwnerPriorPhase::Closing(ClosingStage::Run { attempt: u64::MAX }),
        why: HoldCategory::Budget,
        source: Some(part(3)),
        ending: Some(OwnerChipEnding::Cancelled {
            reason: doc(b"stop", true),
            result: Some(ResultValue::OwnerVerdict { verdict: u32::MAX, words: doc(b"words", false), fields }),
            by: OwnerRequester::Deployment { project: u32::MAX },
            at: Wall::from_nanos(u64::MAX),
        }),
    });
    let expected = u64::try_from(size_of::<OwnerMilestoneLine>())
        .unwrap()
        .checked_add(List::<Box<[u8]>>::worst_case(3).unwrap())
        .unwrap()
        .checked_add(5)
        .unwrap()
        .checked_add(List::<ResultField>::worst_case(3).unwrap())
        .unwrap()
        .checked_add(9)
        .unwrap()
        .checked_add(9)
        .unwrap();
    let original = source.clone();
    assert_eq!(bytes(&source, &bounds()), Some(expected));
    for (amount, admitted) in
        [(expected.checked_sub(1).unwrap(), false), (expected, true), (expected.checked_add(1).unwrap(), true)]
    {
        let b = Bounds { document_bytes: amount, ..bounds() };
        assert_eq!(fits(&source, &b), admitted);
        assert_eq!(worst_case(&b), Some(amount));
    }
    assert_eq!(source, original);
}
#[test]
fn owner_milestone_reserved_empty_lists_optional_approval_and_every_nested_capacity_refuse_whole_source() {
    let base = bounds();
    let absent = line(OwnerMilestonePhase::Waiting);
    let root = u64::try_from(size_of::<OwnerMilestoneLine>()).unwrap();
    assert_eq!(bytes(&absent, &base), Some(root));
    assert!(worst_case(&Bounds { document_bytes: root.checked_sub(1).unwrap(), ..base }).is_none());
    let mut fields = List::with_capacity(3);
    let mut source = line(OwnerMilestonePhase::Ended(OwnerChipEnding::Done(ResultValue::OwnerVerdict {
        verdict: 0,
        words: doc(b"", false),
        fields: fields.clone(),
    })));
    assert_eq!(bytes(&source, &base), Some(root.checked_add(List::<ResultField>::worst_case(3).unwrap()).unwrap()));
    assert!(bytes(&source, &Bounds { rows: 2, segments: 2, ..base }).is_none());
    fields.push(ResultField { name: copy_of(&[255; 8]), value: copy_of(&[254; 8]) }).unwrap();
    source.phase = OwnerMilestonePhase::Ended(OwnerChipEnding::Done(ResultValue::OwnerVerdict {
        verdict: u32::MAX,
        words: doc(b"12345678", true),
        fields,
    }));
    let unchanged = source.clone();
    assert!(bytes(&source, &base).is_some());
    assert!(bytes(&source, &Bounds { text: 7, ..base }).is_none());
    assert_eq!(source, unchanged);
    for explanation in [None, Some(doc(b"", false)), Some(doc(b"12345678", true))] {
        let value = line(OwnerMilestonePhase::Ended(OwnerChipEnding::Done(ResultValue::Approved {
            state: copy_of(&[255; 8]),
            words: explanation,
        })));
        assert!(bytes(&value, &base).is_some());
        assert!(bytes(&value, &Bounds { text: 7, ..base }).is_none());
    }
    let reserved = line(OwnerMilestonePhase::Held {
        before: OwnerPriorPhase::Waiting,
        why: HoldCategory::Budget,
        source: Some(part(4)),
        ending: None,
    });
    assert!(bytes(&reserved, &base).is_none());
}
#[test]
fn owner_milestone_verdict_followup_allocations_remain_whole_and_no_terminal_clock_or_result_is_fabricated() {
    let base = bounds();
    let empty = line(OwnerMilestonePhase::Ended(OwnerChipEnding::Cancelled {
        reason: doc(b"", false),
        result: None,
        by: OwnerRequester::Task(0),
        at: Wall::from_nanos(0),
    }));
    assert_eq!(bytes(&empty, &base), Some(u64::try_from(size_of::<OwnerMilestoneLine>()).unwrap()));
    let tasks = List::<Chip>::with_capacity(3);
    let value = line(OwnerMilestonePhase::Ended(OwnerChipEnding::Done(ResultValue::Verdict {
        verdict: 0,
        words: doc(b"", true),
        fields: List::with_capacity(0),
        tasks,
        proposal: None,
    })));
    let expected = u64::try_from(size_of::<OwnerMilestoneLine>())
        .unwrap()
        .checked_add(List::<Chip>::worst_case(3).unwrap())
        .unwrap();
    assert_eq!(bytes(&value, &base), Some(expected));
    assert!(bytes(&value, &Bounds { rows: 2, segments: 2, ..base }).is_none());
    assert_eq!(value.at, None);
    let numeric = line(OwnerMilestonePhase::Held {
        before: OwnerPriorPhase::Active(RunStage::BackingOff { until: Wall::from_nanos(u64::MAX) }),
        why: HoldCategory::Placed { hold: u64::MAX },
        source: Some(OwnerPart {
            connector: u16::MAX,
            kind: OwnerPartKind::Effect,
            key: OwnerPartKey::Number(u64::MAX),
        }),
        ending: None,
    });
    assert_eq!(bytes(&numeric, &base), Some(u64::try_from(size_of::<OwnerMilestoneLine>()).unwrap()));
}
