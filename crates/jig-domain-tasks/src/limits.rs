//! Root-configured admission and edge limits (domain/tasks.md, section 10).
use crate::{Class, Retries};

/// Immutable root-configured limits for live task state, finite sources, owned semantic payloads
/// and retry policies; validate through `valid` before construction. (domain/tasks.md, section 10).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Limits {
    /// Placed holds in the deployment, including lifted holds kept for their tasks.
    pub placed_holds: u32,
    /// Placed holds retained in one project.
    pub project_placed_holds: u32,
    /// Placed holds retained for one party owner.
    pub party_placed_holds: u32,
    /// Tasks one standing placed hold may exempt; at most tasks.
    pub exemptions: u32,
    /// Maximum retained live task slots, name-index entries and retry alarms.
    pub tasks: u32,
    /// Maximum retained finite period, pool and recurring allotment identities.
    pub funders: u32,
    /// Maximum live tasks per project, including every member of an admitted batch.
    pub project_tasks: u32,
    /// Maximum live tracked tasks per project.
    pub project_goals: u32,
    /// Maximum lifetime tasks made in a requester tree, including its root; completed delegates do
    /// not return this capacity.
    pub tree_tasks: u32,
    /// Maximum structural depth below the requester-tree root, which has depth zero.
    pub depth: u32,
    /// Maximum live direct delegates of one requester task.
    pub delegates: u32,
    /// Maximum introduced peer references per live task.
    pub references: u32,
    /// Maximum standing interests per task.
    pub subscriptions: u32,
    /// Maximum directly created tasks in one atomic nonempty `Make` batch.
    pub batch: u32,
    /// Maximum immutable dependency identities per task; also bounds its remaining live
    /// `waiting_on` subset.
    pub dependencies: u32,
    /// Maximum resources required by one task.
    pub holdings: u32,
    /// Maximum connector resource kinds installed in this deployment.
    pub hold_kinds: u32,
    /// Maximum cached named connector admission reports.
    pub resource_reports: u32,
    /// Maximum distinct pools whose latest connector slot counts are retained.
    pub pools: u32,
    /// Maximum literal segments in one resource name.
    pub hold_segments: u32,
    /// Maximum aggregate literal bytes in one resource name.
    pub hold_bytes: u32,
    /// Maximum tasks waiting on any one resource.
    pub hold_waiters: u32,
    /// Longest span a cancel may give its task to finish.
    pub grace: skein_lib::Duration,
    /// Wall-clock bound before a task waiting for holds is held for review.
    pub hold_wait: skein_lib::Duration,
    /// Maximum tasks a circle of waits names in a refusal or a hold (domain/tasks.md, 6.7).
    pub circle: u32,
    /// Shape bound for historical input identities checked by the root before delegation.
    pub inputs: u32,
    /// Maximum combined specification words and byte-valued parameter bytes per task.
    pub spec_bytes: u32,
    /// Maximum parameters per specification.
    pub parameters: u32,
    /// Maximum result/reason bytes; contracts cannot allow larger results and cancellation may
    /// retain one bounded reason plus one bounded partial result.
    pub result_bytes: u32,
    /// Messages a run's inbox takes: the most its start carries, and the most relayed
    /// and unread (tasks.md, 8.2). At most `inbox_messages`.
    pub run_inbox: u32,
    /// Maximum whole unread word messages kept by one live task.
    pub inbox_messages: u32,
    /// Maximum total unread word bytes kept by one live task.
    pub inbox_bytes: u32,
    /// Maximum bytes in one admitted word message.
    pub message_bytes: u32,
    /// Time an active person task waits before becoming news for its requester.
    pub person_stall: skein_lib::Duration,
    /// Time an undecided proposal waits at a nonfinal holder before passing upward.
    pub proposal_stall: skein_lib::Duration,
    /// Time an undecided escalation waits at a nonfinal holder before passing upward.
    pub escalation_stall: skein_lib::Duration,
    /// Maximum connector resources on which a task has saved state.
    pub saved_resources: u32,
    /// Maximum distinct-code choices in a nonempty verdict contract.
    pub contract_choices: u32,
    /// Maximum retained configured agent-charter numbers.
    pub charters: u32,
    /// Charter versions held besides the newest view.
    pub versions: u32,
    /// Settings revisions held besides the newest.
    pub revisions: u32,
    /// Maximum resource grants per task authority carrier.
    pub authority_grants: u32,
    /// Maximum base segments per carried grant pattern.
    pub authority_segments: u32,
    /// Maximum total bytes across every grant's base and terminal segments in one authority
    /// carrier.
    pub authority_bytes: u32,
    /// Maximum carried authority executor-permission entries.
    pub executor_kinds: u32,
    /// Validated per-class failure retry and pause policies.
    pub retries: Retries,
    /// Optional content-free observation capacity; overflow is diagnostic and changes no decision.
    pub facts: u32,
    /// Recent owner-proven room refusals; zero omits history and counts omissions.
    pub room_refusals: u32,
}

/// Maximum ended identities named by bounded live dependencies, inputs or references.
#[must_use]
pub(crate) fn stub_capacity(limits: &Limits) -> Option<u32> {
    limits.tasks.checked_mul(
        limits
            .dependencies
            .checked_add(limits.inputs)?
            .checked_add(limits.references)?
            .checked_add(limits.batch.checked_mul(limits.dependencies.checked_add(limits.inputs)?)?)?,
    )
}

/// Whether these limits are representable: every edge bound they size
/// is finite and every capacity they derive fits (programming-model.md, 6.3).
#[must_use]
pub fn valid(limits: &Limits) -> bool {
    if limits.placed_holds == 0
        || limits.project_placed_holds == 0
        || limits.party_placed_holds == 0
        || limits.exemptions == 0
        || limits.exemptions > limits.tasks
        || limits.circle == 0
        || limits.tasks == 0
        || limits.batch == 0
        || limits.tree_tasks == 0
        || limits.run_inbox == 0
        || limits.run_inbox > limits.inbox_messages
        || limits.inbox_messages == 0
        || limits.inbox_bytes == 0
        || limits.message_bytes == 0
        || limits.message_bytes > limits.inbox_bytes
        || limits.grace == skein_lib::Duration::ZERO
        || limits.hold_wait == skein_lib::Duration::ZERO
        || limits.person_stall == skein_lib::Duration::ZERO
        || limits.proposal_stall == skein_lib::Duration::ZERO
        || limits.escalation_stall == skein_lib::Duration::ZERO
    {
        return false;
    }
    for class in [Class::Transient, Class::Permanent, Class::Run, Class::Agent, Class::Lost, Class::Invalid] {
        let retry = limits.retries.of(class);
        if retry.retries == u32::MAX || retry.base == skein_lib::Duration::ZERO || retry.max < retry.base {
            return false;
        }
    }
    crate::domain::output_bound(limits).is_some()
        && crate::activity::line_bytes_limit(limits).is_some()
        && crate::waits::valid(limits)
        && limits.tasks.checked_mul(4).is_some()
        && charter_capacity(limits).is_some()
}

fn charter_capacity(limits: &Limits) -> Option<u32> {
    limits.charters.checked_mul(2)?.checked_add(limits.versions)
}
