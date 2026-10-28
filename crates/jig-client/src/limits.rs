//! The composition's bounded queues (client/domain/client.md, 14).
use alloc::boxed::Box;
use skein_lib::{List, Map, Queue, RequestKey};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub domain: crate::DomainLimits,
    pub exchanges: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unusable {
    Limits,
}
#[must_use]
pub fn max_out(limits: &Limits) -> u32 {
    let link = jig_client_protocol_link::Limits::new(limits.domain, limits.exchanges).expect("valid limits");
    jig_client_protocol_link::limits::max_out(&link)
        .expect("valid link")
        .checked_mul(jig_client_domain::max_out(&limits.domain).checked_add(2).expect("valid domain bound"))
        .expect("valid client output bound")
}
/// Whether these limits are representable: every edge bound they size
/// is finite and every capacity they derive fits (programming-model.md, 6.3).
#[must_use]
pub fn valid(limits: &Limits) -> bool {
    if !jig_client_domain::valid(&limits.domain) {
        return false;
    }
    let Ok(link) = jig_client_protocol_link::Limits::new(limits.domain, limits.exchanges) else {
        return false;
    };
    output_bound(limits, &link).is_some()
}

#[must_use]
pub fn worst_case(limits: &Limits) -> Option<u64> {
    if !valid(limits) {
        return None;
    }
    let domain = jig_client_domain::worst_case(&limits.domain)?;
    let link = jig_client_protocol_link::Limits::new(limits.domain, limits.exchanges).ok()?;
    let bound = output_bound(limits, &link)?;
    let output_bytes = limits.domain.document_bytes.max(u64::from(link.wire.authorize_go_location));
    domain
        .checked_add(jig_client_protocol_link::worst_case(&link)?)?
        .checked_add(Queue::<jig_client_domain::Request>::worst_case(bound)?)?
        .checked_add(Queue::<jig_client_protocol_link::Request>::worst_case(bound)?)?
        .checked_add(Queue::<crate::client::DeferredRead>::worst_case(limits.domain.reads)?)?
        // Queued reads are validated owner envelopes; the existing read limit bounds their count.
        .checked_add(u64::from(limits.domain.reads).checked_mul(limits.domain.document_bytes.checked_mul(2)?)?)?
        .checked_add(40)?
        .checked_add(Map::<RequestKey, u64>::worst_case(limits.domain.requests.requests)?)?
        .checked_add(List::<crate::Record>::worst_case(bound)?)?
        .checked_add(List::<Box<[u8]>>::worst_case(bound)?)?
        .checked_add(u64::from(bound).checked_mul(output_bytes.checked_mul(2)?)?)
}

fn output_bound(limits: &Limits, link: &jig_client_protocol_link::Limits) -> Option<u32> {
    jig_client_protocol_link::limits::max_out(link)?
        .checked_mul(jig_client_domain::max_out(&limits.domain).checked_add(2)?)
}
