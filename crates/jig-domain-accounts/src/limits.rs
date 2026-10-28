use skein_lib::Duration;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Limits {
    pub accounts: u32,
    /// Accounts one holder keeps, the deployment or a party; at most `accounts` (accounts.md, 3.6).
    pub holder_accounts: u32,
    /// Sources one account keeps, in its holder's order.
    pub sources: u32,
    /// Lifetime program names, retired names included (accounts.md, 3.4).
    pub program_names: u32,
    /// Hosts a program sign-out can await; at most `PROGRAM_HOSTS`.
    pub program_hosts: u32,
    /// Sign-ins, LLM-key keeps, moves out and moves in under way at once.
    pub sign_ins: u32,
    pub credentials: u32,
    pub backoff_base: Duration,
    pub backoff_max: Duration,
    pub retry: Duration,
    /// How long a move out waits for Written after its answer (configuration.md, section 6).
    pub move_deadline: Duration,
    pub spent_attention: Duration,
    pub facts: u32,
}

/// Whether these limits are representable: every edge bound they size
/// is finite and every capacity they derive fits (programming-model.md, 6.3).
#[must_use]
pub fn valid(limits: &Limits) -> bool {
    if limits.accounts == 0
        || crate::domain::out_bound(limits).is_none()
        || limits.holder_accounts == 0
        || limits.holder_accounts > limits.accounts
        || limits.program_hosts == 0
        || limits.program_hosts > crate::programs::PROGRAM_HOSTS
        || limits.program_names == 0
        || limits.sources == 0
        || limits.sign_ins == 0
        || limits.retry == Duration::ZERO
        || limits.move_deadline == Duration::ZERO
        || limits.backoff_base == Duration::ZERO
        || limits.backoff_max < limits.backoff_base
    {
        return false;
    }
    true
}
