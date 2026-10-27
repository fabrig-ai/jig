use super::*;
use crate::payload::{self, Payload};
use core::mem::size_of;
use skein_lib::{List, Wall};
pub(super) fn details() -> OwnerChipDetails {
    OwnerChipDetails {
        dependencies: List::with_capacity(0),
        project: u32::MAX,
        executor: OwnerExecutor::Person(PersonAddress::Role(u32::MAX)),
        phase_since: Wall::from_nanos(10),
        run_since: None,
        attempt: None,
        retry: None,
        wake: OwnerWakePolicy {
            markers: 0,
            words: WakeRule::Never,
            notices: WakeRule::Never,
            news: WakeRule::Never,
            results: ResultsWake::Never,
            questions: false,
            answers: false,
            timers: false,
        },
        person_wait: None,
        hold: None,
        budget: OwnerBudget {
            numbers: OwnerNumbers { budget: 9, spent: 1, spent_below: 2, reserved: 3 },
            left: 4,
            funder: Funder::Period { project: u32::MAX, period: u64::MAX },
            deadline: None,
        },
        activity: u64::MAX,
        resources: List::with_capacity(0),
        resources_cut: false,
        resources_observed: false,
        procedure: None,
        live_delegates: u32::MAX,
        closing_connectors: None,
        ending: None,
        ended_at: None,
        observation: None,
    }
}

#[test]
fn whole_chip_admission_prices_mandatory_metadata_reserved_dependencies_and_title_bytes() {
    let mut dependencies = List::with_capacity(3);
    dependencies.push(u64::MAX).unwrap();
    let chip = Chip {
        task: u64::MAX,
        title: DocumentText { bytes: text(b"partial"), cut: true },
        executor: Executor::Role { project: u32::MAX, role: u32::MAX },
        phase: Phase::Waiting,
        details: Box::new(ChipPhase::Owner {
            phase: OwnerChipPhase::Waiting { dependencies, more: true, holds: true },
            details: Box::new(details()),
        }),
    };
    let mut bounds = Bounds { text: 7, rows: 3, document_bytes: u64::MAX, ..Bounds::default() };
    let whole = u64::try_from(size_of::<Chip>()).unwrap() + chip.heap(&bounds).unwrap();
    let expected = u64::try_from(
        size_of::<Chip>() + size_of::<ChipPhase>() + size_of::<OwnerChipDetails>() + 3 * size_of::<u64>(),
    )
    .unwrap()
        + 7;
    assert_eq!(whole, expected, "both mandatory boxes and all reserved dependency slots are charged");
    bounds.document_bytes = whole;
    assert!(payload::fits(&chip, &bounds));
    bounds.document_bytes = whole - 1;
    assert!(!payload::fits(&chip, &bounds));
    bounds.document_bytes = whole + 1;
    assert!(payload::fits(&chip, &bounds));
    bounds.rows = 2;
    assert!(!payload::fits(&chip, &bounds));
    bounds.rows = 3;
    bounds.text = 6;
    assert!(!payload::fits(&chip, &bounds));
    assert!(chip.title.cut);
    assert_eq!(chip.title.bytes(), b"partial");
}
