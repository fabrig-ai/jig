use super::{List, chip_from_rows, chip_root_fact, frame_record, limits, task_record, w, wire_owner_chip_details_row};
use alloc::boxed::Box;
extern crate alloc;

#[must_use]
pub fn frame_two_projects_record() -> w::Me {
    let lim = limits();
    let mut parts = frame_record().into_parts();
    let mut identity = parts.party.into_parts();
    identity.login = w::DocumentText::new(&lim, w::DocumentTextParts { bytes: Box::from("é".as_bytes()), cut: true })
        .expect("literal UTF-8 login");
    parts.party = w::MeParty::new(&lim, identity).expect("literal frame party");
    let mut projects = List::with_capacity(2);
    for (project, role) in [(3, w::HeldRole::Owner), (5, w::HeldRole::Observer)] {
        projects
            .push(
                w::MeProject::new(
                    &lim,
                    w::MeProjectParts {
                        templates: Box::from([]),
                        creating: None,
                        project,
                        name: w::DocumentText::new(
                            &lim,
                            w::DocumentTextParts { bytes: Box::from(*b"Tally"), cut: false },
                        )
                        .expect("project name"),
                        role,
                        pool: None,
                        models: List::with_capacity(0),
                    },
                )
                .expect("literal fixture fits"),
            )
            .expect("literal fixture fits");
    }
    let mut size = 0_u32;
    for project in &projects {
        size = size.checked_add(project.measure()).expect("fixture sizes").checked_add(4).expect("piece header");
    }
    let mut out = skein_lib::Writer::new(usize::try_from(size).expect("fixture size"));
    for project in &projects {
        let mut encoded = skein_lib::Writer::new(usize::try_from(project.measure()).expect("fixture size"));
        project.encode(&mut encoded).expect("fixture encode");
        skein_codec::piece::write(&mut out, &encoded.finish()).expect("fixture piece");
    }
    parts.projects = out.finish();
    w::Me::new(&lim, parts).expect("literal fixture fits")
}
#[must_use]
pub fn owner_phase_records() -> [w::OwnerChipPhase; 6] {
    let lim = limits();
    [
        w::OwnerChipPhase::Waiting(
            w::OwnerChipPhaseWaiting::new(
                &lim,
                w::OwnerChipPhaseWaitingParts { dependencies: List::with_capacity(0), more: false, holds: false },
            )
            .expect("literal fixture fits"),
        ),
        w::OwnerChipPhase::Active(
            w::OwnerChipPhaseActive::new(&lim, w::OwnerChipPhaseActiveParts { value: w::RunStage::Idle })
                .expect("literal fixture fits"),
        ),
        w::OwnerChipPhase::Person(
            w::OwnerChipPhasePerson::new(
                &lim,
                w::OwnerChipPhasePersonParts {
                    address: w::PersonAddress::Role(
                        w::PersonAddressRole::new(&lim, w::PersonAddressRoleParts { value: 17 })
                            .expect("literal fixture fits"),
                    ),
                    taken_by: Some(19),
                    since: Some(23),
                },
            )
            .expect("literal fixture fits"),
        ),
        w::OwnerChipPhase::Closing(
            w::OwnerChipPhaseClosing::new(&lim, w::OwnerChipPhaseClosingParts { value: w::ClosingStage::Effects })
                .expect("literal fixture fits"),
        ),
        w::OwnerChipPhase::Held(
            w::OwnerChipPhaseHeld::new(
                &lim,
                w::OwnerChipPhaseHeldParts {
                    reason: w::HoldCategory::Uncertain(
                        w::HoldCategoryUncertain::new(&lim, w::HoldCategoryUncertainParts { entry: 29 })
                            .expect("literal fixture fits"),
                    ),
                    revision: 31,
                },
            )
            .expect("literal fixture fits"),
        ),
        w::OwnerChipPhase::Ended,
    ]
}
#[must_use]
pub fn owner_chip_record(phase: w::OwnerChipPhase, compact: w::Phase) -> w::Chip {
    let lim = limits();
    let mut root = match chip_root_fact() {
        w::Fact::Chip(value) => value.into_parts(),
        w::Fact::ChipPhase(_)
        | w::Fact::Closing(_)
        | w::Fact::Ending(_)
        | w::Fact::HeldRuntime(_)
        | w::Fact::HoldReason(_)
        | w::Fact::OwnerChipDetails(_)
        | w::Fact::OwnerChipEnding(_)
        | w::Fact::ResultValue(_) => unreachable!("literal chip root"),
    };
    let mut fields = root.value.into_parts();
    fields.phase = compact;
    root.value = w::ChipFields::new(&lim, fields).expect("literal fixture fits");
    chip_from_rows(&[
        w::Fact::Chip(w::ChipRow::new(&lim, root).expect("literal fixture fits")),
        w::Fact::ChipPhase(
            w::ChipPhaseRow::new(
                &lim,
                w::ChipPhaseRowParts {
                    parent: Some(0),
                    value: w::ChipPhaseFields::Owner(
                        w::ChipPhaseOwnerFields::new(&lim, w::ChipPhaseOwnerFieldsParts { phase, details: 2 })
                            .expect("literal fixture fits"),
                    ),
                },
            )
            .expect("literal fixture fits"),
        ),
        w::Fact::OwnerChipDetails(Box::new(wire_owner_chip_details_row())),
    ])
}
#[must_use]
pub fn owner_waiting_chip_record() -> w::Chip {
    owner_chip_record(owner_phase_records().into_iter().next().expect("literal fixture fits"), w::Phase::Waiting)
}
#[must_use]
pub fn task_with_hold_record() -> w::Task {
    let lim = limits();
    let mut parts = task_record().into_parts();
    parts.chip = owner_waiting_chip_record();
    parts.hold = Some(w::TaskHold::Owner(
        w::TaskHoldOwner::new(&lim, w::TaskHoldOwnerParts { value: hold_record() }).expect("literal fixture fits"),
    ));
    w::Task::new(&lim, parts).expect("literal fixture fits")
}
#[must_use]
pub fn hold_record() -> w::OwnerChipHold {
    let lim = limits();
    w::OwnerChipHold::new(
        &lim,
        w::OwnerChipHoldParts {
            words: None,
            placed: None,
            circle: None,
            account: None,
            source: None,
            reason: w::HoldCategory::Uncertain(
                w::HoldCategoryUncertain::new(&lim, w::HoldCategoryUncertainParts { entry: 29 })
                    .expect("literal fixture fits"),
            ),
            revision: 31,
            before: w::OwnerPriorPhase::Waiting,
            escalation: None,
            tries: w::OwnerFailureTries::new(
                &lim,
                w::OwnerFailureTriesParts { transient: 2, permanent: 3, run: 5, agent: 7, lost: 11, invalid: 13 },
            )
            .expect("literal fixture fits"),
            hold_wait_since: Some(17),
        },
    )
    .expect("literal fixture fits")
}

#[must_use]
pub fn owner_chip_records() -> [w::Chip; 6] {
    let mut phases = owner_phase_records().into_iter();
    [w::Phase::Waiting, w::Phase::Idle, w::Phase::Active, w::Phase::Closing, w::Phase::Held, w::Phase::Ended]
        .map(|compact| owner_chip_record(phases.next().expect("six literal phases"), compact))
}
