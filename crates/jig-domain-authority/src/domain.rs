//! Policies in force, changed only by bounded events (domain/authority.md, 6).

use alloc::boxed::Box;
use skein_lib::{List, Map, Queue};

use crate::limits::{authority_within, requirements_within, within};
use crate::order::at_most_but_models;
use crate::{Limits, Models, Policy, Requirement, Role, Rules, max_out, worst_case};

/// Bounded live project-policy table and immutable deployment rules; contains no task ledger,
/// connector state or authentication. (domain/authority.md, sections 2 and 6).
#[derive(Debug)]
pub struct Domain {
    rules: Rules,
    policies: Map<u32, Policy>,
    limits: Limits,
}

/// Root-issued policy-table mutation, producing exactly one `PolicyFact` in caller-reserved output
/// room. (domain/authority.md, sections 2 and 6).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Event {
    /// Replace rules without revalidating existing projects (engine.md, 17.2).
    Rules(Box<Rules>),
    /// Restore an existing policy under its own ceiling, independently of new rules.
    Restore { project: u32, policy: Policy },
    /// Add or replace a policy after bounded validation; refusal leaves the prior entry intact.
    Policy {
        /** Project whose live policy is added or replaced. */
        project: u32,
        /** Owned candidate policy, checked against configured bounds and deployment ceilings before mutation. */
        policy: Policy,
    },
    /// Remove a policy; absence still yields a terminal dropped fact.
    Dropped {
        /** Project whose live policy should be removed, whether present or absent. */
        project: u32,
    },
}

/// Terminal policy admission reason; refused updates preserve the previous policy.
/// (domain/authority.md, sections 2 and 6).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PolicyRefusal {
    /// An owned candidate collection or byte field exceeds its configured bound.
    Oversized,
    /// Project authority or period spend exceeds deployment rules.
    AboveRules,
    /// Role authority, period spend, permission bits, uniqueness or escalation identity is invalid.
    InvalidRole,
    /// A new project cannot fit in the bounded table; existing entries are retained.
    Full,
}

/// One terminal in-memory policy lifecycle output per event; persistence and decision durability
/// belong to the root. (domain/authority.md, sections 2 and 6).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PolicyFact {
    /// A previously absent project was installed.
    Added {
        /** Project with a newly installed policy. */
        project: u32,
    },
    /// An existing project's policy was replaced.
    Changed {
        /** Project whose previous policy was replaced. */
        project: u32,
    },
    /// Drop completed, reporting whether the entry existed.
    Dropped {
        /** Project named by the drop event. */
        project: u32,
        /** Whether a live policy was actually removed. */
        existed: bool,
    },
    /// Update terminated without changing the policy table.
    Refused {
        /** Project named by the refused update. */
        project: u32,
        /** Admission reason; no replacement was made. */
        reason: PolicyRefusal,
    },
}

/// Exact output-room requirement for one policy event: one terminal lifecycle fact.
pub const POLICY_MAX_OUT: u32 = 1;

impl Domain {
    /// Admit owned `rules` and `limits`, creating an empty bounded policy table; return `None` for
    /// unrepresentable bounds, oversized rules or inconsistent run ceilings. Allocates only
    /// configured retained capacity.
    #[must_use]
    pub fn new(rules: Rules, limits: Limits) -> Option<Domain> {
        max_out(&limits)?;
        worst_case(&limits)?;
        if !valid_rules(&rules, &limits) {
            return None;
        }
        Some(Domain { rules, policies: Map::with_capacity(limits.projects), limits })
    }

    /// Add bounded deployment requirements while assembling startup configuration.
    /// The application's root calls this before the first event it admits.
    pub fn add_configured_requirements(&mut self, additions: &[Requirement]) -> bool {
        let Some(count) = u32::try_from(self.rules.requirements.len().saturating_add(additions.len())).ok() else {
            return false;
        };
        if count > self.limits.requirements || !requirements_within(additions, &self.limits) {
            return false;
        }
        let mut combined = List::with_capacity(count);
        for requirement in &self.rules.requirements {
            if combined.push(requirement.clone()).is_err() {
                return false;
            }
        }
        for requirement in additions {
            if combined.push(requirement.clone()).is_err() {
                return false;
            }
        }
        self.rules.requirements = combined.into_boxed();
        true
    }

    /// Borrow immutable deployment rules; no output or state change.
    #[must_use]
    pub const fn rules(&self) -> &Rules {
        &self.rules
    }

    /// Borrow immutable admission limits; no output or state change.
    #[must_use]
    pub const fn limits(&self) -> &Limits {
        &self.limits
    }

    /// Borrow the live policy for `project`, or `None` when absent; no output or state change.
    #[must_use]
    pub fn policy(&self, project: u32) -> Option<&Policy> {
        self.policies.get(&project)
    }

    /// Borrow role `number` within live `project`, or `None`; scans at most the admitted role limit
    /// with no allocation or output.
    #[must_use]
    #[expect(clippy::manual_find, reason = "the foundation's step subset uses bounded loops without closures")]
    pub fn role(&self, project: u32, number: u32) -> Option<&Role> {
        for role in &self.policy(project)?.roles {
            if role.number == number {
                return Some(role);
            }
        }
        None
    }

    /// The models a party in role `number` of live `project` may use: the configured models the
    /// rules' ceiling names, narrowed by the policy's ceiling and the role's, in configuration's
    /// order; `None` when the project or the role is absent. Scans as `role` does.
    #[must_use]
    pub fn models(&self, project: u32, number: u32) -> Option<Models> {
        let policy = self.policy(project)?;
        let role = self.role(project, number)?;
        Some(self.rules.ceiling.models.and(policy.ceiling.models).and(role.authority.models))
    }
}

/// Policy events emit one fact and retain the previous policy on refusal. Caller reserves
/// `POLICY_MAX_OUT` slots before applying the event. Apply one root-issued `event` to `domain` and
/// emit exactly one terminal policy fact. Caller must reserve `POLICY_MAX_OUT` free slots in `out`;
/// refusal preserves the previous policy, and the root owns persistence.
pub fn step(domain: &mut Domain, event: Event, out: &mut Queue<PolicyFact>) {
    assert!(out.room() >= POLICY_MAX_OUT, "caller reserves policy lifecycle output");
    let restoring = match &event {
        Event::Restore { .. } => true,
        Event::Rules(_) | Event::Policy { .. } | Event::Dropped { .. } => false,
    };
    match event {
        Event::Rules(rules) => {
            assert!(valid_rules(&rules, &domain.limits), "replacement rules validated at admission");
            domain.rules = *rules;
        }
        Event::Policy { project, policy } | Event::Restore { project, policy } => {
            if let Some(reason) = invalid(&domain.rules, &domain.limits, &policy, restoring) {
                out.push(PolicyFact::Refused { project, reason });
                return;
            }
            match domain.policies.insert(project, policy) {
                Ok(previous) => match previous {
                    Some(previous) => {
                        drop(previous);
                        out.push(PolicyFact::Changed { project });
                    }
                    None => out.push(PolicyFact::Added { project }),
                },
                Err(returned) => {
                    drop(returned);
                    out.push(PolicyFact::Refused { project, reason: PolicyRefusal::Full });
                }
            }
        }
        Event::Dropped { project } => {
            let previous = domain.policies.remove(&project);
            let existed = previous.is_some();
            drop(previous);
            out.push(PolicyFact::Dropped { project, existed });
        }
    }
}

/// Check a policy before a settings value is admitted; allocates nothing.
#[must_use]
pub fn policy_refusal(rules: &Rules, limits: &Limits, policy: &Policy) -> Option<PolicyRefusal> {
    invalid(rules, limits, policy, false)
}
/// The rules' bounded shape and consistent run ceilings.
#[must_use]
pub fn valid_rules(rules: &Rules, limits: &Limits) -> bool {
    authority_within(&rules.ceiling, limits)
        && requirements_within(&rules.requirements, limits)
        && rules.implies.len() <= limits.implications
        && rules.minimum_run_spend < rules.maximum_run_spend
        && rules.maximum_run_spend <= rules.ceiling.budget.spend
}
fn invalid(rules: &Rules, limits: &Limits, policy: &Policy, restoring: bool) -> Option<PolicyRefusal> {
    if !within(policy.projections.len(), limits.grants)
        || !authority_within(&policy.ceiling, limits)
        || !within(policy.roles.len(), limits.roles)
        || !requirements_within(&policy.requirements, limits)
    {
        return Some(PolicyRefusal::Oversized);
    }
    if !restoring
        && (!at_most_but_models(&policy.ceiling, &rules.ceiling, &rules.implies)
            || policy.period_spend > rules.period_spend)
    {
        return Some(PolicyRefusal::AboveRules);
    }
    for grant in &policy.projections {
        if !crate::limits::pattern_within(&grant.pattern, limits) {
            return Some(PolicyRefusal::Oversized);
        }
        let mut covered = false;
        for ceiling in &policy.ceiling.grants {
            covered |= crate::grant_at_most(grant, ceiling, &rules.implies);
        }
        if !restoring && !covered {
            return Some(PolicyRefusal::AboveRules);
        }
    }
    for (position, role) in policy.roles.iter().enumerate() {
        if !authority_within(&role.authority, limits) {
            return Some(PolicyRefusal::Oversized);
        }
        if restoring {
            continue;
        }
        if !at_most_but_models(&role.authority, &policy.ceiling, &rules.implies)
            || role.period_spend > policy.period_spend
            || role.requests.0 & !crate::Requests::ALL.0 != 0
            || role.decides.0 & !crate::Proposals::ALL.0 != 0
        {
            return Some(PolicyRefusal::InvalidRole);
        }
        for earlier in policy.roles.get(..position).expect("enumerated position is in the roles") {
            if earlier.number == role.number {
                return Some(PolicyRefusal::InvalidRole);
            }
        }
    }
    if !restoring && let Some(number) = policy.escalation_role {
        let mut valid = false;
        for role in &policy.roles {
            if role.number == number {
                valid = role.requests.allows(crate::RequestKind::Accept)
                    && role.decides.allows(crate::ProposalKind::Escalation);
            }
        }
        if !valid {
            return Some(PolicyRefusal::InvalidRole);
        }
    }
    None
}
