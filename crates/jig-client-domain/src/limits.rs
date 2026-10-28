//! Bounded client handles and engine watch documents (client/domain/client.md, 14).
use skein_lib::{Duration, RequestLimits};
/// A carried value's most bytes: the client wire's bound on `Keyed.value`
/// (protocol/client.md, 5).
pub const VALUE_BYTES: usize = 131_072;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Backoff {
    pub first: Duration,
    pub most: Duration,
}
/// Whole account-list receiving counts (client/domain/client.md, 14).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AccountCounts {
    pub accounts: u32,
    pub sources: u32,
    pub sign_ins: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub owners: &'static [crate::Spoken],
    pub requests: RequestLimits,
    pub ask_bytes: u64,
    pub handles: u32,
    pub watches: u32,
    pub reads: u32,
    pub turn_bytes: u32,
    pub document_bytes: u64,
    pub backoff: Backoff,
    pub rows: u32,
    pub accounts: AccountCounts,
    pub segments: u32,
    pub authority: u32,
    pub text: u32,
    /// A committed turn's shared text budget in the transcript: its texts, its
    /// calls' names, inputs and outcomes, and its results' summaries. At least
    /// `text` and at most `turn_bytes`.
    pub turn_text: u32,
    pub projects: u32,
    pub fields: u32,
    pub delegates: u32,
    pub records: u32,
    pub kept: u32,
    pub parts: u32,
    pub heartbeat: Duration,
}
/// Whether these limits are representable: every edge bound they size
/// is finite and every capacity they derive fits (programming-model.md, 6.3).
#[must_use]
pub fn valid(limits: &Limits) -> bool {
    if limits.turn_text < limits.text || limits.turn_text > limits.turn_bytes {
        return false;
    }
    if limits.watches == 0 || limits.backoff.first == Duration::ZERO || limits.backoff.first > limits.backoff.most {
        return false;
    }
    if limits.requests.out == 0
        || limits.requests.first == Duration::ZERO
        || limits.requests.first > limits.requests.most
    {
        return false;
    }
    if u32::try_from(limits.owners.len()).is_err() {
        return false;
    }
    true
}
