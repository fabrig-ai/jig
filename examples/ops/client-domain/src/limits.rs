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

#[must_use]
pub fn worst_case(limits: &Limits) -> Option<u64> {
    if limits.jig.kept < limits.client.domain.requests.requests {
        return None;
    }
    let mut total = jig_client_view::worst_case(&limits.jig)?;
    total = total.checked_add(infrastructure::worst_case(&limits.infrastructure)?)?;
    total = total.checked_add(crate::observability::worst_case(&limits.observability)?)?;
    total = total.checked_add(jig_client::worst_case(&limits.client)?)?;
    Some(total)
}
