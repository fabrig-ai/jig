//! Exact numeric admission observations, independent of refusal wording; contract.md, 4.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LimitKey {
    Batch,
    Tasks,
    ProjectTasks,
    ProjectGoals,
    TreeTasks,
    Depth,
    Delegates,
    References,
    Subscriptions,
    Dependencies,
    SpecBytes,
    Parameters,
    ContractChoices,
    ResultBytes,
    Inputs,
    Holdings,
    HoldWaiters,
    InboxMessages,
    InboxBytes,
    MessageBytes,
    Grace,
    PlacedHolds,
    ProjectPlacedHolds,
    PartyPlacedHolds,
    Exemptions,
    AuthorityGrants,
    AuthorityBytes,
    AuthoritySegments,
    HoldSegments,
    HoldBytes,
    Funders,
    ExecutorKinds,
    ResourceReports,
    /// The immutable task contract's words ceiling, with no configuration key.
    ContractWords,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NumericLimit {
    pub key: LimitKey,
    pub value: u64,
}
