//! Project periods from the root and person allotments from policy
//! (protocol/configuration.md, 3; domain/authority.md, 7).
use crate::Core;
use jig_domain_people as people;
use jig_domain_tasks as tasks;

/// A changed spend keeps everything already spent or reserved (domain/authority.md, 7).
pub(crate) fn resized(spend: u64, numbers: tasks::Numbers) -> Option<u64> {
    let used = numbers.spent.checked_add(numbers.spent_below)?.checked_add(numbers.reserved)?;
    Some(spend.max(used))
}

impl Core {
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
