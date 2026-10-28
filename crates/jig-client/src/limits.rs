//! The composition's receiving limits (client/domain/client.md, 14).
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
