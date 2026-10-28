//! Project periods from the root and person allotments from policy
//! (protocol/configuration.md, 3; domain/authority.md, 7).
use crate::{Core, Limits};
use jig_domain_authority as authority;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_lib::List;

/// A changed spend keeps everything already spent or reserved (domain/authority.md, 7).
pub(crate) fn resized(spend: u64, numbers: tasks::Numbers) -> Option<u64> {
    let used = numbers.spent.checked_add(numbers.spent_below)?.checked_add(numbers.reserved)?;
    Some(spend.max(used))
}

/// What one policy edit resizes in its project's open period
/// (domain/authority.md, 7): the period's new budget, if its spend changed,
/// and each open pool of a party whose role's spend changed.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Resize {
    pub(crate) period: Option<u64>,
    pub(crate) pools: List<PoolResize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PoolResize {
    pub(crate) person: u64,
    pub(crate) from: u64,
    pub(crate) to: u64,
}

impl Core {
    /// Plan the open period and pools before committing any part of a policy edit.
    pub(crate) fn resize(
        &self,
        project: u32,
        before: &authority::Policy,
        after: &authority::Policy,
        limits: &Limits,
    ) -> Result<Resize, people::Refusal> {
        let arithmetic = self.limit_refusal(people::LimitKind::Arithmetic);
        let mut plan = Resize { period: None, pools: List::with_capacity(limits.people.holdings) };
        let period = self.period(project);
        let Some(row) = self.tasks.funding(tasks::Funder::Period { project, period }) else {
            return Ok(plan);
        };
        if before.period_spend != after.period_spend {
            plan.period = Some(resized(after.period_spend, row.numbers).ok_or(arithmetic.clone())?);
        }
        let mut increase = 0_i128;
        for role in &after.roles {
            let mut changed = false;
            for old in &before.roles {
                if old.number == role.number {
                    changed = old.period_spend != role.period_spend;
                }
            }
            if !changed {
                continue;
            }
            for holding in self.people.project_members(project).unwrap_or(&[]) {
                if holding.role.number() != role.number {
                    continue;
                }
                let person = holding.person;
                let Some(pool) = self.tasks.funding(tasks::Funder::Pool { project, person, period }) else {
                    continue;
                };
                let from = pool.numbers.budget;
                let to = resized(role.period_spend, pool.numbers).ok_or(arithmetic.clone())?;
                let delta = i128::from(to).checked_sub(i128::from(from)).ok_or(arithmetic.clone())?;
                increase = increase.checked_add(delta).ok_or(arithmetic.clone())?;
                plan.pools.push(PoolResize { person, from, to }).expect("at most the project's parties");
            }
        }
        let reserved = i128::from(row.numbers.reserved).checked_add(increase).ok_or(arithmetic.clone())?;
        if reserved < 0 {
            return Err(arithmetic);
        }
        let spent =
            i128::from(row.numbers.spent).checked_add(i128::from(row.numbers.spent_below)).ok_or(arithmetic.clone())?;
        let used = spent.checked_add(reserved).ok_or(arithmetic.clone())?;
        if used > i128::from(plan.period.unwrap_or(row.numbers.budget)) {
            let Ok(needs) = u64::try_from(increase) else { return Err(arithmetic) };
            let left = row
                .numbers
                .budget
                .saturating_sub(row.numbers.spent)
                .saturating_sub(row.numbers.spent_below)
                .saturating_sub(row.numbers.reserved);
            return Err(people::Refusal::Authority { proposable: None, lacking: people::Lack::Budget { needs, left } });
        }
        Ok(plan)
    }

    /// The latest period the root has opened for this project, restored with its ledger.
    #[must_use]
    pub fn period(&self, project: u32) -> u64 {
        let mut current = 0;
        for (funder, _) in self.tasks.funding_records() {
            match funder {
                tasks::Funder::Period { project: owner, period } => {
                    if *owner == project {
                        current = current.max(*period);
                    }
                }
                tasks::Funder::Pool { .. } | tasks::Funder::Recurring { .. } | tasks::Funder::Task(_) => {}
            }
        }
        current
    }
    pub(crate) fn period_budget(&self, project: u32) -> u64 {
        match self.tasks.funding(tasks::Funder::Period { project, period: self.period(project) }) {
            Some(row) => row.numbers.budget,
            None => 0,
        }
    }
    pub(crate) fn pool_budget(&self, project: u32, person: u64) -> u64 {
        match self.people.role(person, project) {
            Some(role) => match self.authority.role(project, role.number()) {
                Some(policy) => policy.period_spend,
                None => 0,
            },
            None => 0,
        }
    }
    /// The most a person's next task can take from its role and remaining pool.
    pub(crate) fn creating_authority(
        &self,
        project: u32,
        person: u64,
        role: people::Role,
        tasks_left: u32,
    ) -> jig_domain_authority::Authority {
        let mut given = self.authority.role(project, role.number()).expect("configured role policy").authority.clone();
        given.models = self.authority.models(project, role.number()).expect("configured role policy");
        let (_, pool) = self.person_pool(project, person);
        given.delegation.tasks = given.delegation.tasks.min(tasks_left).saturating_sub(1);
        given.delegation.depth = given.delegation.depth.saturating_sub(1);
        let ask = jig_domain_authority::PersonAsk {
            project,
            role: role.number(),
            pool: crate::translate::authority_numbers(pool),
            tasks_left,
            request: jig_domain_authority::PersonRequest::Watch,
        };
        given.budget.spend = jig_domain_authority::as_allowed(&self.authority, &ask, 0, None);
        given
    }
}

#[cfg(test)]
mod tests {
    use super::resized;
    use jig_domain_tasks::Numbers;

    #[test]
    fn a_resized_budget_keeps_used_funds_and_checks_both_additions() {
        let mut numbers = Numbers { budget: u64::MAX, spent: u64::MAX - 2, spent_below: 1, reserved: 1 };
        assert_eq!(resized(0, numbers), Some(u64::MAX));
        numbers.reserved = 2;
        assert_eq!(resized(0, numbers), None);
        numbers.spent_below = 3;
        numbers.reserved = 0;
        assert_eq!(resized(0, numbers), None);
        numbers = Numbers { budget: 100, spent: 10, spent_below: 20, reserved: 30 };
        assert_eq!(resized(200, numbers), Some(200));
        assert_eq!(resized(60, numbers), Some(60));
        assert_eq!(resized(59, numbers), Some(60));
    }
}
