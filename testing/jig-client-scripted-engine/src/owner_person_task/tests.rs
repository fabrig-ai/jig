use super::{bytes, fits, worst_case};
use crate::{Bounds, payload};
use core::mem::size_of;
use jig_client_domain::contract::*;
use skein_lib::{List, Wall, bytes::copy_of};
fn doc(bytes: &[u8]) -> DocumentText {
    DocumentText { bytes: Text::new(copy_of(bytes), 8).expect("fixture text fits"), cut: true }
}
fn chip(n: u64) -> Chip {
    Chip {
        task: n,
        title: doc(b"chip"),
        executor: Executor::Party { party: u64::MAX },
        phase: Phase::Waiting,
        details: Box::new(ChipPhase::Waiting { dependencies: List::with_capacity(3), hold: None }),
    }
}
pub(crate) fn value() -> OwnerPersonTask {
    let mut fields = List::with_capacity(2);
    fields.push(FieldRule { name: copy_of(&[255, 0, 47]), max: u32::MAX }).expect("field fits");
    let mut choices = List::with_capacity(3);
    choices.push(VerdictChoice { code: u32::MAX, words: u32::MAX, followups: u32::MAX, fields }).expect("choice fits");
    let mut result_fields = List::with_capacity(3);
    result_fields.push(ResultField { name: copy_of(b"field"), value: copy_of(&[255, 0]) }).expect("result field fits");
    let result = Result {
        task: chip(2),
        ending: Ending::Done,
        value: Some(ResultValue::OwnerVerdict { verdict: u32::MAX, words: doc(b"why"), fields: result_fields }),
        at: None,
        details: None,
        acts: List::with_capacity(3),
    };
    let mut results = List::with_capacity(3);
    results.push(OwnerShownResult { task: OwnerTaskLink::Live(chip(2)), result: None }).expect("missing result fits");
    results
        .push(OwnerShownResult {
            task: OwnerTaskLink::Stored { number: 2, status: None },
            result: Some(Box::new(result)),
        })
        .expect("present result fits");
    OwnerPersonTask {
        task: chip(8),
        revision: Revision { value: u64::MAX },
        requester: OwnerRequester::Deployment { project: u32::MAX },
        spec: SpecificationDocument {
            specification: DocumentSpecification {
                asks: false,
                words: doc(b"spec"),
                parameters: List::with_capacity(2),
                inputs: List::with_capacity(3),
            },
            cut: true,
        },
        address: PersonAddress::Role(u32::MAX),
        taken_by: None,
        since: Some(Wall::EPOCH),
        stall: None,
        deadline: None,
        contract: TaskContractDocument::Verdict { choices, cut: true },
        results,
        cut: true,
        acts: List::with_capacity(3),
    }
}
fn bounds() -> Bounds {
    Bounds { rows: 3, segments: 3, text: 8, document_bytes: 65536, ..Bounds::default() }
}
#[test]
fn person_owner_admission_prices_reserved_slots_boxes_and_optional_whole_results_at_exact_boundary() {
    let value = value();
    let b = bounds();
    let untouched = value.clone();
    let chip_heap = 4
        + u64::try_from(size_of::<ChipPhase>()).expect("size fits")
        + List::<u64>::worst_case(3).expect("dependencies fit");
    let expected = u64::try_from(size_of::<OwnerPersonTask>()).expect("size fits")
        + chip_heap
        + 4
        + List::<ParameterDocument>::worst_case(2).expect("parameters fit")
        + List::<u64>::worst_case(3).expect("inputs fit")
        + List::<VerdictChoice>::worst_case(3).expect("choices fit")
        + List::<FieldRule>::worst_case(2).expect("fields fit")
        + 3
        + List::<OwnerShownResult>::worst_case(3).expect("shown results fit")
        + chip_heap
        + u64::try_from(size_of::<Result>()).expect("result size fits")
        + chip_heap
        + 3
        + List::<ResultField>::worst_case(3).expect("result fields fit")
        + 5
        + 2
        + List::<Offered>::worst_case(3).expect("result acts fit")
        + List::<Offered>::worst_case(3).expect("person acts fit");
    assert_eq!(bytes(&value, &b), Some(expected));
    for (size, accepted) in [(expected - 1, false), (expected, true), (expected + 1, true)] {
        assert_eq!(fits(&value, &Bounds { document_bytes: size, ..b }), accepted);
    }
    let read = ReadValue::OwnerPersonTask(Box::new(value.clone()));
    let read_bytes = u64::try_from(size_of::<ReadValue>()).expect("read size fits") + expected;
    assert!(payload::fits(&read, &Bounds { document_bytes: read_bytes, ..b }));
    assert!(!payload::fits(&read, &Bounds { document_bytes: read_bytes - 1, ..b }));
    assert_eq!(value, untouched);
    assert_eq!(worst_case(&b), Some(65536));
}
#[test]
fn person_owner_nested_reserved_overflow_refuses_whole_without_cut_or_missing_result_shortening() {
    let b = bounds();
    let original = value();
    let mut too_large = Vec::new();
    let mut v = original.clone();
    v.results = List::with_capacity(4);
    too_large.push(v);
    let mut v = original.clone();
    v.acts = List::with_capacity(MAX_OFFERED_ACTS.checked_add(1).expect("one offer slot above the ceiling"));
    too_large.push(v);
    let mut v = original.clone();
    v.spec.specification.parameters = List::with_capacity(4);
    too_large.push(v);
    let mut v = original.clone();
    v.spec.specification.inputs = List::with_capacity(4);
    too_large.push(v);
    let mut v = original.clone();
    v.contract = TaskContractDocument::Verdict { choices: List::with_capacity(4), cut: true };
    too_large.push(v);
    let mut v = original.clone();
    if let TaskContractDocument::Verdict { choices, .. } = &mut v.contract {
        choices.get_mut(0).expect("choice exists").fields = List::with_capacity(4);
    }
    too_large.push(v);
    let mut v = original.clone();
    v.results.get_mut(1).expect("result row exists").result.as_mut().expect("result exists").acts =
        List::with_capacity(MAX_OFFERED_ACTS.checked_add(1).expect("one result offer slot above the ceiling"));
    too_large.push(v);
    let mut v = original.clone();
    v.results.get_mut(1).expect("result row exists").result.as_mut().expect("result exists").value =
        Some(ResultValue::OwnerVerdict { verdict: 7, words: doc(b"why"), fields: List::with_capacity(4) });
    too_large.push(v);
    let mut v = original.clone();
    v.contract = TaskContractDocument::Approval {
        state: copy_of(&[255; 9]),
        part: OwnerPart { connector: 1, kind: OwnerPartKind::Outbox, key: OwnerPartKey::Number(0) },
    };
    too_large.push(v);
    for value in too_large {
        let unchanged = value.clone();
        assert!(!fits(&value, &b));
        assert_eq!(value, unchanged);
    }
    assert_eq!(original, value());
}
#[test]
fn person_owner_empty_reserved_results_and_opaque_approval_parts_remain_distinct_allocations() {
    let b = bounds();
    let mut v = value();
    v.results = List::with_capacity(3);
    v.contract = TaskContractDocument::Report { words: 0 };
    let reserved = bytes(&v, &b).expect("reserved document fits");
    v.results = List::with_capacity(0);
    assert_eq!(
        reserved - bytes(&v, &b).expect("empty document fits"),
        List::<OwnerShownResult>::worst_case(3).expect("shown slots fit")
    );
    let mut path = List::with_capacity(3);
    path.push(copy_of(b"a/b")).expect("part path fits");
    path.push(copy_of(&[255, 0])).expect("part path fits");
    v.contract = TaskContractDocument::Approval {
        state: copy_of(&[255, 0, 47]),
        part: OwnerPart { connector: u16::MAX, kind: OwnerPartKind::Outbox, key: OwnerPartKey::Path(path) },
    };
    let full = bytes(&v, &b).expect("opaque document fits");
    assert_eq!(
        full - bytes(&OwnerPersonTask { contract: TaskContractDocument::Report { words: 0 }, ..v.clone() }, &b)
            .expect("report fits"),
        3 + List::<Box<[u8]>>::worst_case(3).expect("path slots fit") + 3 + 2
    );
    assert!(fits(&v, &Bounds { document_bytes: full, ..b }));
    assert!(!fits(&v, &Bounds { document_bytes: full - 1, ..b }));
}

#[test]
fn person_owner_reserved_offer_slots_use_the_closed_ceiling_at_whole_byte_boundaries() {
    let b = bounds();
    let mut value = value();
    let baseline = bytes(&value, &b).expect("initial whole value fits");
    value.acts = List::with_capacity(MAX_OFFERED_ACTS);
    value
        .acts
        .push(Offered { effect: None, act: ActKind::Answer, admission: Admission::Allowed })
        .expect("answer fits");
    let result = value.results.get_mut(1).expect("second row").result.as_mut().expect("whole result");
    result.acts = List::with_capacity(MAX_OFFERED_ACTS);
    result
        .acts
        .push(Offered { effect: None, act: ActKind::Seen, admission: Admission::Refused(Refusal::NotWaiting) })
        .expect("refused offer fits");
    let slots = List::<Offered>::worst_case(MAX_OFFERED_ACTS).expect("closed offer slots fit")
        - List::<Offered>::worst_case(3).expect("initial offer slots fit");
    let expected = baseline + slots * 2;
    let original = value.clone();
    assert_eq!(bytes(&value, &b), Some(expected));
    assert!(fits(&value, &Bounds { document_bytes: expected, ..b }));
    assert!(!fits(&value, &Bounds { document_bytes: expected - 1, ..b }));
    let read = ReadValue::OwnerPersonTask(Box::new(value.clone()));
    let packet = u64::try_from(size_of::<ReadValue>()).expect("read inline size fits") + expected;
    assert!(payload::fits(&read, &Bounds { document_bytes: packet, ..b }));
    assert!(!payload::fits(&read, &Bounds { document_bytes: packet - 1, ..b }));
    for outer in [false, true] {
        let mut oversized = value.clone();
        let capacity = MAX_OFFERED_ACTS.checked_add(1).expect("one above ceiling");
        if outer {
            oversized.acts = List::with_capacity(capacity);
        } else {
            oversized.results.get_mut(1).expect("second row").result.as_mut().expect("whole result").acts =
                List::with_capacity(capacity);
        }
        let unchanged = oversized.clone();
        assert!(!fits(&oversized, &b));
        assert!(!payload::fits(&ReadValue::OwnerPersonTask(Box::new(oversized.clone())), &b));
        assert_eq!(oversized, unchanged);
    }
    value.acts = List::with_capacity(MAX_OFFERED_ACTS);
    value.results.get_mut(1).expect("second row").result.as_mut().expect("whole result").acts =
        List::with_capacity(MAX_OFFERED_ACTS);
    assert_eq!(bytes(&value, &b), Some(expected), "reserved empty slots retain their allocation price");
    assert_eq!(original.acts.capacity(), MAX_OFFERED_ACTS);
    assert!(MAX_OFFERED_ACTS > b.rows);
    value.results = List::with_capacity(b.rows + 1);
    assert!(!fits(&value, &b), "shown rows retain their independent ceiling");
}
