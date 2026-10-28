//! Provisional current run documents (client/domain/contract.md, 7.3).
use super::Undecodable;
use super::documents::{classify, current_call_fields, live_call_fields, receiving, run_state_fields, run_text_fields};
use crate::Limits;
use jig_client_domain::contract as c;
use jig_client_protocol_wire::v19 as w;
pub fn run_state(record: w::RunState, limits: &Limits) -> Result<c::RunState, Undecodable> {
    if !crate::limits::valid(limits) || u64::from(record.measure()) > limits.client.document_bytes {
        return Err(Undecodable::TooLarge);
    }
    let checked = receiving(limits)?;
    let limits = &checked;
    let record = match w::RunState::new(&limits.wire, record.into_parts()) {
        Ok(value) => value,
        Err(problem) => return Err(classify(problem)),
    };
    match run_state_fields(&record, limits) {
        Ok(value) => Ok(value),
        Err(problem) => Err(classify(problem)),
    }
}
pub fn run_text(record: w::RunText, limits: &Limits) -> Result<c::RunText, Undecodable> {
    if !crate::limits::valid(limits) || u64::from(record.measure()) > limits.client.document_bytes {
        return Err(Undecodable::TooLarge);
    }
    let checked = receiving(limits)?;
    let limits = &checked;
    let record = match w::RunText::new(&limits.wire, record.into_parts()) {
        Ok(value) => value,
        Err(problem) => return Err(classify(problem)),
    };
    match run_text_fields(&record, limits) {
        Ok(value) => Ok(value),
        Err(problem) => Err(classify(problem)),
    }
}
pub fn current_call(record: w::CurrentCall, limits: &Limits) -> Result<c::CurrentCall, Undecodable> {
    if !crate::limits::valid(limits) || u64::from(record.measure()) > limits.client.document_bytes {
        return Err(Undecodable::TooLarge);
    }
    let checked = receiving(limits)?;
    let limits = &checked;
    let record = match w::CurrentCall::new(&limits.wire, record.into_parts()) {
        Ok(value) => value,
        Err(problem) => return Err(classify(problem)),
    };
    match current_call_fields(&record, limits) {
        Ok(value) => Ok(value),
        Err(problem) => Err(classify(problem)),
    }
}
pub fn live_call(record: w::LiveCall, limits: &Limits) -> Result<c::LiveCall, Undecodable> {
    if !crate::limits::valid(limits) || u64::from(record.measure()) > limits.client.document_bytes {
        return Err(Undecodable::TooLarge);
    }
    let checked = receiving(limits)?;
    let limits = &checked;
    let record = match w::LiveCall::new(&limits.wire, record) {
        Ok(value) => value,
        Err(problem) => return Err(classify(problem)),
    };
    match live_call_fields(&record, limits) {
        Ok(value) => Ok(value),
        Err(problem) => Err(classify(problem)),
    }
}
