//! Keep: the child routing shape and exhaustive connector variants.
//! Replace: Ops connector sides, family addresses and application vocabulary.
use crate::infrastructure;
/// Application-configured child and client bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub jig: jig_client_view::Limits,
    pub infrastructure: infrastructure::Limits,
    pub observability: crate::observability::Limits,
    pub client: jig_client::Limits,
}

/// Whether the root's child limits are usable.
#[must_use]
pub fn valid(limits: &Limits) -> bool {
    limits.jig.kept >= limits.client.domain.requests.requests
        && jig_client_view::valid(&limits.jig)
        && infrastructure::valid(&limits.infrastructure)
        && crate::observability::valid(&limits.observability)
        && jig_client::valid(&limits.client)
}
