use super::*;
use skein_lib::List;
pub fn read_wire_revision() -> w::Revision {
    jig_client_test_records::read_wire_revision()
}

pub fn read_wire_phase() -> w::Phase {
    w::Phase::Waiting
}

pub fn read_wire_steering() -> w::Steering {
    let lim = w::CEILINGS;
    w::Steering::new(
        &lim,
        w::SteeringParts {
            addressed: None,
            person_revision: None,
            taken_by: None,
            revision: read_wire_revision(),
            spec: read_wire_specification_document(),
            instructions: None,
            wake: read_wire_owner_wake_policy(),
            dependencies: List::with_capacity(0),
            authority: read_wire_owner_authority_document(),
            budget: read_wire_budget(),
        },
    )
    .unwrap()
}

pub fn read_wire_specification_document() -> w::SpecificationDocument {
    jig_client_test_records::read_wire_specification_document()
}

pub fn read_wire_owner_wake_policy() -> w::OwnerWakePolicy {
    let lim = w::CEILINGS;
    w::OwnerWakePolicy::new(
        &lim,
        w::OwnerWakePolicyParts {
            markers: u32::MAX,
            words: read_wire_wake_rule(),
            notices: read_wire_wake_rule(),
            news: read_wire_wake_rule(),
            results: read_wire_results_wake(),
            questions: false,
            answers: false,
            timers: false,
        },
    )
    .unwrap()
}

pub fn read_wire_wake_rule() -> w::WakeRule {
    w::WakeRule::Never
}

pub fn read_wire_results_wake() -> w::ResultsWake {
    w::ResultsWake::Never
}

pub fn read_wire_owner_authority_document() -> w::OwnerAuthorityDocument {
    let lim = w::CEILINGS;
    w::OwnerAuthorityDocument::new(
        &lim,
        w::OwnerAuthorityDocumentParts { cut: false, authority: read_wire_owner_authority() },
    )
    .unwrap()
}

pub fn read_wire_owner_authority() -> w::OwnerAuthority {
    let lim = w::CEILINGS;
    w::OwnerAuthority::new(
        &lim,
        w::OwnerAuthorityParts {
            models: Some(w::ModelSet::All),
            tools: 7,
            grants: Box::from([]),
            delegation: read_wire_owner_delegation(),
            spend: 7,
            deadline: None,
            notes: 7,
            note_resources: Box::from([]),
        },
    )
    .unwrap()
}

pub fn read_wire_owner_delegation() -> w::OwnerDelegation {
    let lim = w::CEILINGS;
    w::OwnerDelegation::new(&lim, w::OwnerDelegationParts { kinds: List::with_capacity(0), tasks: 7, depth: 7 })
        .unwrap()
}

pub fn read_wire_impact() -> w::Impact {
    let lim = w::CEILINGS;
    w::Impact::new(&lim, w::ImpactParts { tasks: read_wire_phase_counts(), live_runs: 7, proposals: 7 }).unwrap()
}

pub fn read_wire_phase_counts() -> w::PhaseCounts {
    let lim = w::CEILINGS;
    w::PhaseCounts::new(
        &lim,
        w::PhaseCountsParts { ended: 7, waiting: 7, running: 7, held: 7, closing: 7, done: 7, failed: 7, cancelled: 7 },
    )
    .unwrap()
}

pub fn read_wire_owner_escalation() -> w::OwnerEscalation {
    jig_client_test_records::read_wire_owner_escalation()
}

pub fn read_wire_question() -> w::Question {
    jig_client_test_records::read_wire_question()
}

pub fn read_wire_person_task() -> w::PersonTask {
    let lim = w::CEILINGS;
    w::PersonTask::new(
        &lim,
        w::PersonTaskParts {
            task: chip_record(),
            revision: read_wire_revision(),
            requester: read_wire_actor(),
            spec: read_wire_specification_document(),
            contract: read_wire_person_contract(),
            inputs: Box::from([]),
            addressed: read_wire_recipient(),
            taken: None,
            since: None,
            stall: None,
            deadline: None,
            acts: Box::from([]),
            ending: None,
        },
    )
    .unwrap()
}
