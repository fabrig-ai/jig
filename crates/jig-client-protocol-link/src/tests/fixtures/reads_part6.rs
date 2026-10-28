use super::*;
use skein_lib::{List, Wall};
pub fn read_value_phase() -> c::Phase {
    c::Phase::Waiting
}

pub fn read_value_steering() -> c::Steering {
    c::Steering {
        addressed: None,
        person_revision: None,
        taken_by: None,
        revision: read_value_revision(),
        spec: read_value_specification_document(),
        instructions: None,
        wake: read_value_owner_wake_policy(),
        dependencies: List::with_capacity(0),
        authority: read_value_owner_authority_document(),
        budget: read_value_budget(),
    }
}

pub fn read_value_specification_document() -> c::SpecificationDocument {
    c::SpecificationDocument { specification: read_value_document_specification(), cut: false }
}

pub fn read_value_document_specification() -> c::DocumentSpecification {
    c::DocumentSpecification {
        asks: false,
        words: read_value_document_text(),
        parameters: List::with_capacity(0),
        inputs: List::with_capacity(0),
    }
}

pub fn read_value_owner_wake_policy() -> c::OwnerWakePolicy {
    c::OwnerWakePolicy {
        words: read_value_wake_rule(),
        notices: read_value_wake_rule(),
        news: read_value_wake_rule(),
        results: read_value_results_wake(),
        questions: false,
        answers: false,
        timers: false,
    }
}

pub fn read_value_wake_rule() -> c::WakeRule {
    c::WakeRule::Never
}

pub fn read_value_results_wake() -> c::ResultsWake {
    c::ResultsWake::Never
}

pub fn read_value_owner_authority_document() -> c::OwnerAuthorityDocument {
    c::OwnerAuthorityDocument { authority: read_value_owner_authority(), cut: false }
}

pub fn read_value_owner_authority() -> c::OwnerAuthority {
    c::OwnerAuthority {
        models: Some(c::AuthorityModels::All),
        tools: 7,
        grants: List::with_capacity(0),
        delegation: read_value_owner_delegation(),
        spend: 7,
        deadline: None,
        notes: 7,
        note_resources: List::with_capacity(0),
    }
}

pub fn read_value_owner_delegation() -> c::OwnerDelegation {
    c::OwnerDelegation { kinds: List::with_capacity(0), tasks: 7, depth: 7 }
}

pub fn read_value_impact() -> c::Impact {
    c::Impact { tasks: read_value_phase_counts(), live_runs: 7, proposals: 7 }
}

pub fn read_value_phase_counts() -> c::PhaseCounts {
    c::PhaseCounts { ended: 7, waiting: 7, running: 7, held: 7, closing: 7, done: 7, failed: 7, cancelled: 7 }
}

pub fn read_value_owner_escalation() -> c::OwnerEscalation {
    c::OwnerEscalation { acts: List::with_capacity(0), task: Box::new(task()) }
}

pub fn read_value_question() -> c::Question {
    c::Question {
        task: chip(),
        from: None,
        message: 7,
        words: jig_client_domain::contract::DocumentText::complete(text(b"")),
        since: Wall::from_nanos(7),
        acts: List::with_capacity(0),
        ending: None,
    }
}

pub fn read_value_person_task() -> c::PersonTask {
    c::PersonTask {
        task: chip(),
        revision: read_value_revision(),
        requester: read_value_actor(),
        spec: read_value_specification_document(),
        contract: read_value_person_contract(),
        inputs: List::with_capacity(0),
        addressed: read_value_recipient(),
        taken: None,
        since: None,
        stall: None,
        deadline: None,
        acts: List::with_capacity(0),
        ending: None,
    }
}

pub fn read_value_person_contract() -> c::PersonContract {
    c::PersonContract::Choice { options: List::with_capacity(0) }
}
