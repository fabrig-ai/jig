//! Whole settings values and retained revision facts (client/domain/contract.md, 3–7; domain/engine.md, 17).
use super::{
    DocumentText, OwnerAuthority, OwnerGrant, OwnerKindImplication, PolicyEditRole, PolicyPermission,
    PolicyRequirement, ProposalKey, Proposer, Template, TemplateOverride, Text,
};
use alloc::boxed::Box;
use skein_lib::{Duration, List, Wall};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SettingNumber {
    pub number: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Setting {
    Charter(SettingNumber),
    Template(SettingNumber),
    Policy(SettingNumber),
    Rules,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SettingModel {
    pub dialect: u32,
    pub endpoint: u32,
    pub program: Option<Text>,
    pub name: Text,
    pub window: u32,
    pub output: u32,
    pub input_price: u64,
    pub cached_price: u64,
    pub cache_write_price: Option<u64>,
    pub output_price: u64,
    pub price_unit: u32,
    pub efforts: List<Text>,
    pub effort: Option<Text>,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SettingHosts {
    Workers,
    Engine,
    Both,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[expect(clippy::struct_excessive_bools, reason = "the whole charter carries independent tool grants and resume")]
pub struct Charter {
    pub number: u32,
    pub instructions: Text,
    pub waiting: Duration,
    pub resume: bool,
    pub turns: u32,
    pub time: Duration,
    pub reserve_turns: u32,
    pub reserve_time: Duration,
    pub model: SettingModel,
    pub alternatives: List<SettingModel>,
    pub inspect: bool,
    pub modify: bool,
    pub shell: bool,
    pub agents: bool,
    pub call_timeout: Duration,
    pub hosts: SettingHosts,
    pub spend: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SettingPolicy {
    pub escalation_role: Option<u32>,
    pub ceiling: OwnerAuthority,
    pub projections: List<OwnerGrant>,
    pub period_spend: u64,
    pub roles: List<PolicyEditRole>,
    pub requirements: List<PolicyRequirement>,
    pub permissions: List<PolicyPermission>,
    pub templates: List<TemplateOverride>,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Rules {
    pub ceiling: OwnerAuthority,
    pub period_spend: u64,
    pub minimum_run_spend: u64,
    pub maximum_run_spend: u64,
    pub implies: List<OwnerKindImplication>,
    pub requirements: List<PolicyRequirement>,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SettingValue {
    Charter(Box<Charter>),
    Template(Box<Template>),
    Policy(Box<SettingPolicy>),
    Rules(Box<Rules>),
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SettingSet {
    pub value: SettingValue,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SettingEdit {
    Set(SettingSet),
    Removed,
    Reset,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SettingItem {
    pub setting: Setting,
    pub edit: SettingEdit,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Running {
    Stays,
    Moves,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ChangeSettings {
    pub against: u32,
    pub items: List<SettingItem>,
    pub running: Running,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OutcomeSettingsChanged {
    pub revision: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RefusalStale {
    pub newest: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RefusalFull {
    pub revision: u32,
    pub tasks: List<u64>,
    pub more: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SettingFault {
    Shape,
    Count,
    Reference,
    Rules,
    Budget,
    Grace,
    Owner,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MalformedSettings {
    pub setting: Setting,
    pub fault: SettingFault,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ReadSetting {
    pub setting: Setting,
    pub revision: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ReadRevision {
    pub number: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ReadRevisions {
    pub before: Option<u32>,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SettingFollows {
    Default,
    Change,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SettingLine {
    pub setting: Setting,
    pub name: Option<DocumentText>,
    pub follows: SettingFollows,
    pub last: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RevisionParty {
    pub number: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RevisionDecider {
    FirstStart,
    Release,
    Party(RevisionParty),
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RevisionHead {
    pub number: u32,
    pub against: u32,
    pub at: Wall,
    pub by: RevisionDecider,
    pub proposal: Option<ProposalKey>,
    pub moved: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct HeldRevision {
    pub head: RevisionHead,
    pub holders: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SettingsProposal {
    pub proposer: Proposer,
    pub number: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SettingsDocument {
    pub newest: u32,
    pub settings: List<SettingLine>,
    pub held: List<HeldRevision>,
    pub pending: List<SettingsProposal>,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SettingDocument {
    pub setting: Setting,
    pub revision: u32,
    pub value: SettingValue,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Size {
    pub bytes: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RevisionEdit {
    Set(SettingSet),
    TooLarge(Size),
    Removed,
    Reset,
}
/// The choice a revision replaced, with whole values bounded as its edit is.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RevisionReplaced {
    Value(SettingSet),
    TooLarge(Size),
    Default,
    Removed,
    NotOffered,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RevisionItem {
    pub setting: Setting,
    pub edit: RevisionEdit,
    pub replaced: RevisionReplaced,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RevisionDocument {
    pub head: RevisionHead,
    pub items: List<RevisionItem>,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RevisionForgotten {
    pub oldest: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RevisionAnswer {
    Revision(RevisionDocument),
    Forgotten(RevisionForgotten),
    Unknown,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ReadRevisionAnswer {
    pub value: RevisionAnswer,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RevisionItemEdit {
    Set,
    Removed,
    Reset,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RevisionItemHead {
    pub setting: Setting,
    pub edit: RevisionItemEdit,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RevisionRow {
    pub head: RevisionHead,
    pub items: List<RevisionItemHead>,
    pub more: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RevisionsPage {
    pub rows: List<RevisionRow>,
    pub next: Option<u32>,
    pub forgotten: bool,
}

/// A deployment-scoped proposal; its accepter chooses whether running work moves.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ProposeSettings {
    pub against: u32,
    pub items: List<SettingItem>,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OriginRevision {
    pub revision: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SettingOrigin {
    Default(OriginRevision),
    Change(OriginRevision),
    NotOffered,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ReviewValue {
    Value(SettingSet),
    TooLarge(Size),
    None,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ReviewItem {
    pub setting: Setting,
    pub edit: RevisionItemEdit,
    pub now: ReviewValue,
    pub origin: SettingOrigin,
    pub would_be: ReviewValue,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SettingsReview {
    pub newest: u32,
    pub items: List<ReviewItem>,
    pub cut: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OwnerProposalActionSettings {
    pub against: u32,
    pub items: List<RevisionItemHead>,
    pub review: Option<SettingsReview>,
    pub made: Option<u32>,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OwnerRetainedActionSettings {
    pub against: u32,
    pub items: List<RevisionItemHead>,
    pub made: Option<u32>,
}

/// An owner's move of live agent tasks (engine.md, 17.4; contract.md, 3.2).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MoveToNewest {
    pub revision: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OutcomeMovedToNewest {
    pub newest: u32,
    pub moved: u32,
    pub stayed: u32,
}
