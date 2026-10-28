use skein_lib::Duration;

/// Bounds for the connector's live records and one step's output.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Limits {
    /// Maximum live tasks.
    pub tasks: u32,
    /// Maximum adopted project-resource pairs.
    pub adoptions: u32,
    /// Maximum task-topic subscriptions.
    pub subscriptions: u32,
    /// Maximum pools.
    pub pools: u32,
    /// Maximum configured resources.
    pub resources: u32,
    /// Maximum configured topics.
    pub topics: u32,
    /// Maximum configured effect kinds.
    pub kinds: u32,
    /// Configured requirement judges.
    pub requirements: u32,
    /// Configured procedure programs and live states.
    pub procedures: u32,
    /// Maximum actions in one procedure program.
    pub actions_per_procedure: u32,
    /// Working-set facts read from the system.
    pub facts: u32,
    /// Outstanding verdicts awaiting a fact change (at most four).
    pub judges: u32,
    /// Staged sections or workspace item sets; separately, retained application outcomes.
    pub values: u32,
    /// Maximum bytes in one read answer or staged section.
    pub value_bytes: u32,
    /// Maximum effects awaiting authority; separately, prepared application values and core parts.
    pub staged: u32,
    /// Maximum unsettled outbox entries.
    pub entries: u32,
    /// Maximum live objects this deployment made by key.
    pub made: u32,
    /// Maximum resources in one effect.
    pub resources_per_effect: u32,
    /// Maximum automatic attempts before an entry is held; a person may admit another attempt.
    pub max_attempts: u32,
    /// Lifetime of a system write before retry is allowed.
    pub write_lifetime: Duration,
    /// Margin for a wall clock that moved forward.
    pub clock_margin: Duration,
    /// Maximum resources named by one task.
    pub resources_per_task: u32,
    /// Maximum subscribers delivered by one news event.
    pub subscribers_per_topic: u32,
    /// Maximum lost allocations reported in one pool event.
    pub lost_per_pool: u32,
    /// Maximum segments in a path.
    pub path_segments: u32,
    /// Maximum bytes in one segment.
    pub segment_bytes: u32,
}

/// Whether these limits are representable: every edge bound they size
/// is finite and every capacity they derive fits (programming-model.md, 6.3).
#[must_use]
pub fn valid(limits: &Limits) -> bool {
    limits.path_segments != 0 && limits.segment_bytes != 0 && limits.max_attempts != 0 && limits.judges <= 4
}
