use crate::{Bounds, owner_chip as pricing};
use jig_client_domain::contract::*;
use skein_lib::{Duration, List, Token, Wall};
fn doc(bytes: &[u8], cut: bool) -> DocumentText {
    DocumentText { bytes: Text::new(bytes.into(), 4096).unwrap(), cut }
}
fn part() -> OwnerPart {
    let mut path: List<Box<[u8]>> = List::with_capacity(3);
    path.push(b"a/b".as_slice().into()).unwrap();
    path.push(Box::new([255, 0])).unwrap();
    OwnerPart { connector: u16::MAX, kind: OwnerPartKind::Resource, key: OwnerPartKey::Path(path) }
}
fn value() -> OwnerChipDetails {
    let mut resources = List::with_capacity(2);
    resources
        .push(OwnerResourceLine {
            part: part(),
            kind: u16::MAX,
            taken: false,
            holder: None,
            writer: Some(OwnerResourceWriter::Effect { entry: u64::MAX }),
            writer_lost: true,
        })
        .unwrap();
    OwnerChipDetails {
        dependencies: List::with_capacity(0),
        project: u32::MAX,
        executor: OwnerExecutor::Procedure { connector: u16::MAX, code: u32::MAX },
        phase_since: Wall::EPOCH,
        run_since: None,
        attempt: Some(u64::MAX),
        retry: None,
        wake: OwnerWakePolicy {
            markers: 0,
            words: WakeRule::Never,
            notices: WakeRule::Immediate,
            news: WakeRule::Never,
            results: ResultsWake::Each,
            questions: false,
            answers: true,
            timers: false,
        },
        person_wait: Some(OwnerPersonWait {
            since: Wall::from_nanos(1),
            stall: Duration::from_nanos(2),
            deadline: Wall::from_nanos(3),
            notice: Some(OwnerPersonStallNotice { message: u64::MAX, at: Wall::from_nanos(4), activity: u64::MAX }),
        }),
        hold: Some(OwnerChipHold {
            circle: None,
            account: None,
            source: Some(part()),
            reason: HoldCategory::Failures { class: HoldFailure::Invalid },
            revision: u64::MAX,
            before: OwnerPriorPhase::Active(RunStage::Claimed { attempt: u64::MAX }),
            escalation: Some(OwnerHoldEscalation::Rejected { by: u64::MAX, reason: doc(b"no", true) }),
            tries: OwnerFailureTries { transient: 1, permanent: 2, run: 3, agent: 4, lost: 5, invalid: u32::MAX },
            hold_wait_since: None,
        }),
        budget: OwnerBudget {
            numbers: OwnerNumbers { budget: u64::MAX, spent: u64::MAX, spent_below: u64::MAX, reserved: u64::MAX },
            left: u64::MAX,
            funder: Funder::Pool { project: u32::MAX, person: u64::MAX, period: u64::MAX },
            deadline: Some(Wall::from_nanos(5)),
        },
        activity: u64::MAX,
        resources,
        resources_cut: true,
        resources_observed: false,
        procedure: Some(part()),
        live_delegates: u32::MAX,
        closing_connectors: Some(List::with_capacity(3)),
        ending: Some(OwnerChipEnding::Cancelled {
            reason: doc(b"cancel", true),
            result: Some(ResultValue::Change { kind: u16::MAX, part: part(), words: doc(b"old", false) }),
            by: OwnerRequester::Deployment { project: u32::MAX },
            at: Wall::from_nanos(6),
        }),
        ended_at: None,
        observation: Some(OwnerChipObservation {
            channel: Token::new(u64::MAX),
            work: OwnerChipWork::Tool { call: Box::new([255, 0]), tool: b"tool".as_slice().into() },
            since: Wall::from_nanos(7),
            reported_at: Wall::from_nanos(8),
            started: None,
        }),
    }
}
#[test]
fn nested_chip_metadata_prices_every_allocation_at_exact_and_adjacent_whole_value_bounds() {
    let value = value();
    let bounds = Bounds { text: 6, rows: 3, segments: 3, document_bytes: u64::MAX, ..Bounds::default() };
    // Independent sum of the actual allocated capacities and byte literals.
    let each_part = List::<Box<[u8]>>::worst_case(3).unwrap() + 3 + 2;
    let exact = u64::try_from(core::mem::size_of::<OwnerChipDetails>()).unwrap()
        + List::<OwnerResourceLine>::worst_case(2).unwrap()
        + List::<u16>::worst_case(3).unwrap()
        + each_part * 4
        + 2
        + 6
        + 3
        + 2
        + 4;
    assert_eq!(pricing::bytes(&value, &bounds), Some(exact));
    for (capacity, accepted) in [(exact - 1, false), (exact, true), (exact + 1, true)] {
        assert_eq!(pricing::fits(&value, &Bounds { document_bytes: capacity, ..bounds }), accepted);
    }
    let boxed = Some(Box::new(value.clone()));
    let boxed_exact = exact + u64::try_from(core::mem::size_of::<Option<Box<OwnerChipDetails>>>()).unwrap();
    assert_eq!(pricing::boxed_bytes(&boxed, &bounds), Some(boxed_exact));
    assert!(pricing::boxed_fits(&boxed, &Bounds { document_bytes: boxed_exact, ..bounds }));
    assert!(!pricing::boxed_fits(&boxed, &Bounds { document_bytes: boxed_exact - 1, ..bounds }));
    let absent = None;
    let absent_exact = u64::try_from(core::mem::size_of::<Option<Box<OwnerChipDetails>>>()).unwrap();
    assert_eq!(pricing::boxed_bytes(&absent, &bounds), Some(absent_exact));
    assert!(pricing::boxed_fits(&absent, &Bounds { document_bytes: absent_exact, ..bounds }));
    assert!(!pricing::boxed_fits(&absent, &Bounds { document_bytes: absent_exact - 1, ..bounds }));
    assert_eq!(
        value.clone(),
        value,
        "all full-width scalars, options, cuts and opaque channel survive retained copying"
    );
}
#[test]
fn chip_admission_refuses_oversized_terminal_bytes_nested_raw_parts_and_unused_list_capacity() {
    let bounds = Bounds { text: 6, rows: 3, segments: 3, document_bytes: u64::MAX, ..Bounds::default() };
    let base = value();
    let mut invalid = base.clone();
    invalid.ending = Some(OwnerChipEnding::Failed(doc(UTF8_TEXTS[0], true)));
    assert!(!pricing::fits(&invalid, &bounds), "cut never permits silently shortening terminal bytes");
    let mut invalid = base.clone();
    invalid.ending = Some(OwnerChipEnding::Done(ResultValue::Approved { state: Box::new([255; 7]), words: None }));
    assert!(!pricing::fits(&invalid, &bounds), "opaque terminal state is charged whole");
    let mut invalid = base.clone();
    invalid.observation.as_mut().unwrap().work = OwnerChipWork::Tool { call: Box::new([255; 7]), tool: Box::new([]) };
    assert!(!pricing::fits(&invalid, &bounds));
    let mut invalid = base.clone();
    invalid.resources = List::with_capacity(4);
    assert!(!pricing::fits(&invalid, &bounds), "unused allocated resources remain bounded");
    let mut invalid = base.clone();
    invalid.closing_connectors = Some(List::with_capacity(4));
    assert!(!pricing::fits(&invalid, &bounds), "known empty is still an allocated list");
    let mut invalid = base.clone();
    invalid.procedure = Some(OwnerPart {
        connector: 1,
        kind: OwnerPartKind::Procedure,
        key: OwnerPartKey::Path(List::with_capacity(4)),
    });
    assert!(!pricing::fits(&invalid, &bounds), "unused nested segment capacity remains bounded");
    let mut invalid = base.clone();
    invalid.hold.as_mut().unwrap().escalation =
        Some(OwnerHoldEscalation::Rejected { by: u64::MAX, reason: doc(UTF8_TEXTS[0], false) });
    assert!(!pricing::fits(&invalid, &bounds));
    let mut invalid = base.clone();
    invalid.hold.as_mut().unwrap().source = Some(OwnerPart {
        connector: 1,
        kind: OwnerPartKind::Procedure,
        key: OwnerPartKey::Path({
            let mut path: List<Box<[u8]>> = List::with_capacity(1);
            path.push(Box::new([255_u8; 7])).unwrap();
            path
        }),
    });
    assert!(!pricing::fits(&invalid, &bounds));
    assert!(pricing::fits(&base, &bounds), "all refusal probes leave the original complete source untouched");
}

#[test]
fn owner_retry_scalar_notices_price_whole_retained_and_boxed_envelopes_at_adjacent_bounds() {
    let bounds = Bounds { text: 6, rows: 3, segments: 3, document_bytes: u64::MAX, ..Bounds::default() };
    let each_part = List::<Box<[u8]>>::worst_case(3).unwrap() + 3 + 2;
    let exact = u64::try_from(core::mem::size_of::<OwnerChipDetails>()).unwrap()
        + List::<OwnerResourceLine>::worst_case(2).unwrap()
        + List::<u16>::worst_case(3).unwrap()
        + each_part * 4
        + 2
        + 6
        + 3
        + 2
        + 4;
    let optional_scalar = u64::try_from(core::mem::size_of::<Option<RetryNotice>>()).unwrap();
    let mut metadata = value();
    for retry in [None, Some(RetryNotice::Refused { count: 0 }), Some(RetryNotice::Refused { count: u32::MAX })] {
        metadata.retry = retry;
        assert_eq!(pricing::bytes(&metadata, &bounds), Some(exact));
        assert!(crate::payload::fits(&retry, &Bounds { document_bytes: optional_scalar, ..bounds }));
        assert!(!crate::payload::fits(&retry, &Bounds { document_bytes: optional_scalar - 1, ..bounds }));
    }
    let mut notices = Vec::new();
    for class in [
        HoldFailure::Transient,
        HoldFailure::Permanent,
        HoldFailure::Run,
        HoldFailure::Agent,
        HoldFailure::Lost,
        HoldFailure::Invalid,
    ] {
        for (count, allowance) in [(0, 0), (0, u32::MAX), (u32::MAX, 0), (u32::MAX, u32::MAX)] {
            notices.push(RetryNotice::OwnerFailed { class, count, allowance });
        }
    }
    for class in [FailureClass::Temporary, FailureClass::Stall, FailureClass::HostLost, FailureClass::Permanent] {
        notices.push(RetryNotice::Failed { class, count: 7, allowance: 9 });
    }
    for retry in notices {
        let scalar = u64::try_from(core::mem::size_of::<RetryNotice>()).unwrap();
        assert!(crate::payload::fits(&retry, &Bounds { document_bytes: scalar, ..bounds }));
        assert!(!crate::payload::fits(&retry, &Bounds { document_bytes: scalar - 1, ..bounds }));
        metadata.retry = Some(retry);
        assert_eq!(metadata.clone().retry, Some(retry));
        assert_eq!(pricing::bytes(&metadata, &bounds), Some(exact));
        for (bound, expected) in [(exact - 1, false), (exact, true), (exact + 1, true)] {
            assert_eq!(pricing::fits(&metadata, &Bounds { document_bytes: bound, ..bounds }), expected);
        }
        let boxed = Some(Box::new(metadata.clone()));
        let boxed_exact = exact + u64::try_from(core::mem::size_of::<Option<Box<OwnerChipDetails>>>()).unwrap();
        assert_eq!(pricing::boxed_bytes(&boxed, &bounds), Some(boxed_exact));
        for (bound, expected) in [(boxed_exact - 1, false), (boxed_exact, true), (boxed_exact + 1, true)] {
            assert_eq!(pricing::boxed_fits(&boxed, &Bounds { document_bytes: bound, ..bounds }), expected);
        }
    }
}

const UTF8_TEXTS: [&[u8]; 1] = [&[b'~'; 7]];
