//! The composition's bounded queues (client/domain/client.md, 14).
use skein_lib::{Map, Queue, RequestKey};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub domain: crate::DomainLimits,
    pub exchanges: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unusable {
    Limits,
}
/// Whether these limits are representable: every edge bound they size
/// is finite and every capacity they derive fits (programming-model.md, 6.3).
#[must_use]
pub fn valid(limits: &Limits) -> bool {
    if !jig_client_domain::valid(&limits.domain) {
        return false;
    }
    jig_client_protocol_link::Limits::new(limits.domain, limits.exchanges).is_ok()
}

#[must_use]
pub fn worst_case(limits: &Limits) -> Option<u64> {
    if !valid(limits) {
        return None;
    }
    let domain = jig_client_domain::worst_case(&limits.domain)?;
    let link = jig_client_protocol_link::Limits::new(limits.domain, limits.exchanges).ok()?;
    domain
        .checked_add(jig_client_protocol_link::worst_case(&link)?)?
        .checked_add(Queue::<crate::client::DeferredRead>::worst_case(limits.domain.reads)?)?
        // Queued reads are validated owner envelopes; the existing read limit bounds their count.
        .checked_add(u64::from(limits.domain.reads).checked_mul(limits.domain.document_bytes.checked_mul(2)?)?)?
        .checked_add(40)?
        .checked_add(Map::<RequestKey, u64>::worst_case(limits.domain.requests.requests)?)
}
