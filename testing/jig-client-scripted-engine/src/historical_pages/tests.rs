use super::*;
use crate::Bounds;
use jig_client_domain::contract::*;
use skein_lib::{List, Wall};
fn doc(value: &[u8]) -> DocumentText {
    DocumentText { bytes: Text::new(Box::from(value), 8).expect("fixture fits"), cut: true }
}
fn chip() -> Chip {
    Chip {
        task: u64::MAX,
        title: doc(b""),
        executor: Executor::Party { party: u64::MAX },
        phase: Phase::Ended,
        details: Box::new(ChipPhase::Ended { how: Ending::Done, at: Wall::EPOCH }),
    }
}
fn page(ended: OwnerChipEnding, at: Option<Wall>) -> OwnerEndedPage {
    let mut rows = List::with_capacity(3);
    rows.push(OwnerEndedDelegate { chip: chip(), ended, at }).expect("row fits");
    OwnerEndedPage {
        rows,
        before: Some(Cursor::new(Box::from(UTF8_TEXTS[0]), 8).expect("opaque fits")),
        after: Some(Cursor::new(Box::from(UTF8_TEXTS[1]), 8).expect("opaque fits")),
        cut: true,
    }
}
#[test]
fn historical_ended_whole_boxes_reserved_rows_nested_reasons_and_read_bytes_refuse_one_under() {
    let bounds = Bounds { rows: 3, text: 8, ..Bounds::default() };
    for ending in [
        OwnerChipEnding::Done(ResultValue::Approved { state: Box::from([255, 0]), words: None }),
        OwnerChipEnding::Failed(doc(b"failure")),
        OwnerChipEnding::Cancelled {
            reason: doc(b"cancel"),
            result: Some(ResultValue::Words(doc(b"result"))),
            by: OwnerRequester::Task(u64::MAX),
            at: Wall::from_nanos(u64::MAX),
        },
    ] {
        for at in [None, Some(Wall::EPOCH), Some(Wall::from_nanos(u64::MAX))] {
            let source = page(ending.clone(), at);
            let original = source.clone();
            let n = ended_bytes(&source, &bounds).expect("full recursive bytes");
            assert!(ended_fits(&source, &Bounds { document_bytes: n, ..bounds }));
            assert!(!ended_fits(&source, &Bounds { document_bytes: n.checked_sub(1).expect("nonzero"), ..bounds }));
            let read = ReadValue::OwnerEnded(Box::new(source.clone()));
            let r = read_bytes(&read, &bounds).expect("boxed read bytes");
            assert_eq!(
                r,
                n.checked_add(u64::try_from(size_of::<ReadValue>()).expect("inline fits")).expect("whole fits")
            );
            assert!(read_fits(&read, &Bounds { document_bytes: r, ..bounds }));
            assert!(!read_fits(&read, &Bounds { document_bytes: r.checked_sub(1).expect("nonzero"), ..bounds }));
            assert_eq!(source, original);
            assert_eq!(source.rows.capacity(), 3);
            assert_eq!(source.rows.get(0).expect("row").at, at);
        }
    }
    let source = page(OwnerChipEnding::Failed(doc(b"12345678")), None);
    assert!(!ended_fits(&source, &Bounds { text: 7, ..bounds }));
    assert!(!ended_fits(&source, &Bounds { rows: 2, ..bounds }));
    let empty = OwnerEndedPage { rows: List::with_capacity(3), before: None, after: None, cut: false };
    assert_eq!(
        ended_bytes(&empty, &bounds),
        u64::try_from(size_of::<OwnerEndedPage>())
            .ok()
            .and_then(|n| n.checked_add(List::<OwnerEndedDelegate>::worst_case(3).expect("reserved rows")))
    );
    let too_many = OwnerEndedPage { rows: List::with_capacity(4), ..empty.clone() };
    assert!(!ended_fits(&too_many, &bounds));
    assert!(worst_case(&bounds).expect("admitted envelope") >= ended_bytes(&source, &bounds).expect("whole bytes"));
}
#[test]
fn historical_funding_all_four_source_kinds_keep_period_parent_counters_and_reserved_capacity() {
    let bounds = Bounds { rows: 3, text: 8, ..Bounds::default() };
    let kinds = [
        Funder::Task(u64::MAX),
        Funder::Pool { project: u32::MAX, person: u64::MAX, period: 0 },
        Funder::Recurring { project: 0, task: u64::MAX, period: u64::MAX },
        Funder::Period { project: u32::MAX, period: u64::MAX },
    ];
    let periods = [
        None,
        Some(OwnerPeriodTimes { began: Wall::EPOCH, next: None }),
        Some(OwnerPeriodTimes { began: Wall::from_nanos(u64::MAX), next: Some(Wall::EPOCH) }),
    ];
    for funder in kinds {
        for parent in [None, Some(kinds[0]), Some(kinds[1]), Some(kinds[2]), Some(kinds[3])] {
            for period in periods {
                for closed in [false, true] {
                    let row = OwnerFundingRecord {
                        period,
                        funder,
                        parent,
                        numbers: OwnerNumbers { budget: 1, spent: u64::MAX, spent_below: 7, reserved: 11 },
                        made: u32::MAX,
                        closed,
                    };
                    let mut rows = List::with_capacity(3);
                    rows.push(row.clone()).expect("row fits");
                    let source = OwnerSpendPage {
                        rows,
                        before: None,
                        after: Some(Cursor::new(Box::from(UTF8_TEXTS[2]), 8).expect("cursor fits")),
                        cut: true,
                    };
                    let original = source.clone();
                    let n = spend_bytes(&source, &bounds).expect("whole scalar records");
                    assert!(spend_fits(&source, &Bounds { document_bytes: n, ..bounds }));
                    assert!(!spend_fits(
                        &source,
                        &Bounds { document_bytes: n.checked_sub(1).expect("nonzero"), ..bounds }
                    ));
                    let wrapped = ReadValue::OwnerSpend(Box::new(source.clone()));
                    let r = read_bytes(&wrapped, &bounds).expect("read bytes");
                    assert!(read_fits(&wrapped, &Bounds { document_bytes: r, ..bounds }));
                    assert!(!read_fits(
                        &wrapped,
                        &Bounds { document_bytes: r.checked_sub(1).expect("nonzero"), ..bounds }
                    ));
                    assert_eq!(source, original);
                    assert_eq!(source.rows.get(0), Some(&row));
                    assert_eq!(source.rows.capacity(), 3);
                }
            }
        }
    }
    let empty = OwnerSpendPage { rows: List::with_capacity(3), before: None, after: None, cut: false };
    assert_eq!(
        spend_bytes(&empty, &bounds),
        u64::try_from(size_of::<OwnerSpendPage>())
            .ok()
            .and_then(|n| n.checked_add(List::<OwnerFundingRecord>::worst_case(3).expect("reserved rows")))
    );
    assert!(!spend_fits(&OwnerSpendPage { rows: List::with_capacity(4), ..empty }, &bounds));
}

#[test]
fn historical_ended_whole_chip_metadata_prices_reserved_resources_connectors_and_nested_independent_ending() {
    let bounds = Bounds { rows: 3, text: 8, ..Bounds::default() };
    let metadata = OwnerChipDetails {
        dependencies: List::with_capacity(0),
        project: u32::MAX,
        executor: OwnerExecutor::Person(PersonAddress::Role(u32::MAX)),
        phase_since: Wall::EPOCH,
        run_since: None,
        attempt: Some(u64::MAX),
        retry: None,
        wake: OwnerWakePolicy {
            markers: 0,
            words: WakeRule::Never,
            notices: WakeRule::Immediate,
            news: WakeRule::Never,
            results: ResultsWake::LastOrFailure,
            questions: true,
            answers: false,
            timers: true,
        },
        person_wait: None,
        hold: None,
        budget: OwnerBudget {
            numbers: OwnerNumbers { budget: 1, spent: 7, spent_below: 11, reserved: u64::MAX },
            left: 13,
            funder: Funder::Task(u64::MAX),
            deadline: None,
        },
        activity: u64::MAX,
        resources: List::with_capacity(3),
        resources_cut: true,
        resources_observed: false,
        procedure: None,
        live_delegates: u32::MAX,
        closing_connectors: Some(List::with_capacity(3)),
        ending: Some(OwnerChipEnding::Failed(doc(b"metadata"))),
        ended_at: Some(Wall::EPOCH),
        observation: None,
    };
    let mut source = page(
        OwnerChipEnding::Cancelled {
            reason: doc(b"source"),
            result: None,
            by: OwnerRequester::Deployment { project: u32::MAX },
            at: Wall::from_nanos(u64::MAX),
        },
        None,
    );
    let plain = ended_bytes(&source, &bounds).expect("plain whole page");
    *source.rows.get_mut(0).expect("row").chip.details =
        ChipPhase::Owner { phase: OwnerChipPhase::Ended, details: Box::new(metadata.clone()) };
    let whole = ended_bytes(&source, &bounds).expect("complete metadata page");
    assert_eq!(
        whole,
        plain.checked_add(crate::owner_chip::bytes(&metadata, &bounds).expect("whole metadata")).expect("whole sum")
    );
    let original = source.clone();
    assert!(ended_fits(&source, &Bounds { document_bytes: whole, ..bounds }));
    assert!(!ended_fits(&source, &Bounds { document_bytes: whole.checked_sub(1).expect("nonzero"), ..bounds }));
    assert_eq!(source, original);
    let mut over = source.clone();
    let ChipPhase::Owner { details, .. } = over.rows.get_mut(0).expect("row").chip.details.as_mut() else {
        panic!("source owner metadata")
    };
    details.resources = List::with_capacity(4);
    assert!(!ended_fits(&over, &bounds));
    assert_eq!(source, original);
}

const UTF8_TEXTS: [&[u8]; 3] = [b"~\0/", b"\0~", b"~\0"];
