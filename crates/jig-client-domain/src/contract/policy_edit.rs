//! One keyed edit to project policy; domain/people.md, 5.2.
//! These are whole owner values, with no policy inferred from display text.
use super::{OwnerAuthority, OwnerGrant, OwnerPattern};
use skein_lib::{Duration, List};

/// One mutable part of project policy, sent in one request.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PolicyEdit {
    ProjectSpend { period_spend: u64 },
    Role(PolicyEditRole),
    Requirements { requirements: List<PolicyRequirement> },
    Projections { grants: List<OwnerGrant> },
    Permissions { mappings: List<PolicyPermission> },
    Templates { overrides: List<TemplateOverride> },
    RoleSpend { role: u32, period_spend: u64 },
}

/// A role's actual authority, period allotment and decision flags.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PolicyEditRole {
    pub number: u32,
    pub authority: OwnerAuthority,
    pub period_spend: u64,
    pub requests: u16,
    pub decides: u8,
}

/// One connector permission mapped to a project role.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PolicyPermission {
    pub connector: u16,
    pub permission: u16,
    pub role: u32,
}

/// The connector-owned judge and its parameter identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PolicyJudge {
    pub connector: u16,
    pub requirement: u16,
    pub parameters: u32,
}

/// Application-time guard or an observation's allowed age.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PolicyGuard {
    Guarded,
    Observed { freshness: Duration },
}

/// The complete effect requirement, including literal pattern boundaries.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PolicyRequirement {
    pub connector: u16,
    pub kind: u16,
    pub pattern: OwnerPattern,
    pub judge: PolicyJudge,
    pub guard: PolicyGuard,
    pub must_be_guarded: bool,
}

/// A template's executor, named by its charter or chosen by its creator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TemplateExecutor {
    Charter(u32),
    Chosen,
}
/// A template's budget filled in by its creator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TemplateBudget {
    Amount(u64),
    AsAllowed { cap: Option<u64> },
    Given,
}
/// A template's deadline filled in by its creator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TemplateDeadline {
    None,
    Span(Duration),
    Given,
}
/// Overrides of a named template; absent fields retain its defaults.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TemplateOverride {
    pub template: u32,
    pub executor: Option<TemplateExecutor>,
    pub budget: Option<TemplateBudget>,
    pub deadline: Option<TemplateDeadline>,
    pub priority: Option<u32>,
}
