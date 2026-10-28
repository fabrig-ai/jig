//! Single read documents (client/domain/contract.md, 5–6).
use super::pages::{
    owner_escalation_fields, owner_historical_fields, owner_person_task_fields, owner_proposal_fields,
    person_task_fields, project_fields, proposal_fields, question_fields, receiving,
};
use super::{Undecodable, documents::classify};
use crate::Limits;
use jig_client_domain::contract as c;
use jig_client_protocol_wire::v19 as w;
pub fn proposal(record: w::Proposal, limits: &Limits) -> Result<c::Proposal, Undecodable> {
    if !crate::limits::valid(limits) || u64::from(record.measure()) > limits.client.document_bytes {
        return Err(Undecodable::TooLarge);
    }
    let checked = receiving(limits)?;
    let limits = &checked;
    let record = match w::Proposal::new(&limits.wire, record.into_parts()) {
        Ok(value) => value,
        Err(problem) => return Err(classify(problem)),
    };
    match proposal_fields(&record, limits) {
        Ok(value) => Ok(value),
        Err(problem) => Err(classify(problem)),
    }
}

pub fn owner_proposal(record: w::OwnerProposal, limits: &Limits) -> Result<c::OwnerProposal, Undecodable> {
    if !crate::limits::valid(limits) || u64::from(record.measure()) > limits.client.document_bytes {
        return Err(Undecodable::TooLarge);
    }
    let checked = receiving(limits)?;
    let limits = &checked;
    let record = match w::OwnerProposal::new(&limits.wire, record.into_parts()) {
        Ok(value) => value,
        Err(problem) => return Err(classify(problem)),
    };
    match owner_proposal_fields(&record, limits) {
        Ok(value) => Ok(value),
        Err(problem) => Err(classify(problem)),
    }
}

pub fn escalation(record: w::Escalation, limits: &Limits) -> Result<c::Escalation, Undecodable> {
    if !crate::limits::valid(limits) || u64::from(record.measure()) > limits.client.document_bytes {
        return Err(Undecodable::TooLarge);
    }
    let checked = receiving(limits)?;
    let limits = &checked;
    let record = match w::Escalation::new(&limits.wire, record.into_parts()) {
        Ok(value) => value,
        Err(problem) => return Err(classify(problem)),
    };
    match crate::translate::documents::escalation_fields(&record, limits) {
        Ok(value) => Ok(value),
        Err(problem) => Err(classify(problem)),
    }
}

pub fn owner_escalation(record: w::OwnerEscalation, limits: &Limits) -> Result<c::OwnerEscalation, Undecodable> {
    if !crate::limits::valid(limits) || u64::from(record.measure()) > limits.client.document_bytes {
        return Err(Undecodable::TooLarge);
    }
    let checked = receiving(limits)?;
    let limits = &checked;
    let record = match w::OwnerEscalation::new(&limits.wire, record.into_parts()) {
        Ok(value) => value,
        Err(problem) => return Err(classify(problem)),
    };
    match owner_escalation_fields(&record, limits) {
        Ok(value) => Ok(value),
        Err(problem) => Err(classify(problem)),
    }
}

pub fn question(record: w::Question, limits: &Limits) -> Result<c::Question, Undecodable> {
    if !crate::limits::valid(limits) || u64::from(record.measure()) > limits.client.document_bytes {
        return Err(Undecodable::TooLarge);
    }
    let checked = receiving(limits)?;
    let limits = &checked;
    let record = match w::Question::new(&limits.wire, record.into_parts()) {
        Ok(value) => value,
        Err(problem) => return Err(classify(problem)),
    };
    match question_fields(&record, limits) {
        Ok(value) => Ok(value),
        Err(problem) => Err(classify(problem)),
    }
}

pub fn person_task(record: w::PersonTask, limits: &Limits) -> Result<c::PersonTask, Undecodable> {
    if !crate::limits::valid(limits) || u64::from(record.measure()) > limits.client.document_bytes {
        return Err(Undecodable::TooLarge);
    }
    let checked = receiving(limits)?;
    let limits = &checked;
    let record = match w::PersonTask::new(&limits.wire, record.into_parts()) {
        Ok(value) => value,
        Err(problem) => return Err(classify(problem)),
    };
    match person_task_fields(&record, limits) {
        Ok(value) => Ok(value),
        Err(problem) => Err(classify(problem)),
    }
}

pub fn owner_person_task(record: w::OwnerPersonTask, limits: &Limits) -> Result<c::OwnerPersonTask, Undecodable> {
    if !crate::limits::valid(limits) || u64::from(record.measure()) > limits.client.document_bytes {
        return Err(Undecodable::TooLarge);
    }
    let checked = receiving(limits)?;
    let limits = &checked;
    let record = match w::OwnerPersonTask::new(&limits.wire, record.into_parts()) {
        Ok(value) => value,
        Err(problem) => return Err(classify(problem)),
    };
    match owner_person_task_fields(&record, limits) {
        Ok(value) => Ok(value),
        Err(problem) => Err(classify(problem)),
    }
}

pub fn result(record: w::ResultDocument, limits: &Limits) -> Result<c::Result, Undecodable> {
    if !crate::limits::valid(limits) || u64::from(record.measure()) > limits.client.document_bytes {
        return Err(Undecodable::TooLarge);
    }
    let checked = receiving(limits)?;
    let limits = &checked;
    let record = match w::ResultDocument::new(&limits.wire, record.into_parts()) {
        Ok(value) => value,
        Err(problem) => return Err(classify(problem)),
    };
    match crate::translate::documents::result_fields(&record, limits) {
        Ok(value) => Ok(value),
        Err(problem) => Err(classify(problem)),
    }
}

pub fn project(record: w::Project, limits: &Limits) -> Result<c::Project, Undecodable> {
    if !crate::limits::valid(limits) || u64::from(record.measure()) > limits.client.document_bytes {
        return Err(Undecodable::TooLarge);
    }
    let checked = receiving(limits)?;
    let limits = &checked;
    let record = match w::Project::new(&limits.wire, record.into_parts()) {
        Ok(value) => value,
        Err(problem) => return Err(classify(problem)),
    };
    match project_fields(&record, limits) {
        Ok(value) => Ok(value),
        Err(problem) => Err(classify(problem)),
    }
}

pub fn owner_historical(record: w::OwnerHistorical, limits: &Limits) -> Result<c::OwnerHistorical, Undecodable> {
    if !crate::limits::valid(limits) || u64::from(record.measure()) > limits.client.document_bytes {
        return Err(Undecodable::TooLarge);
    }
    let checked = receiving(limits)?;
    let limits = &checked;
    let record = match w::OwnerHistorical::new(&limits.wire, record.into_parts()) {
        Ok(value) => value,
        Err(problem) => return Err(classify(problem)),
    };
    match owner_historical_fields(&record, limits) {
        Ok(value) => Ok(value),
        Err(problem) => Err(classify(problem)),
    }
}
