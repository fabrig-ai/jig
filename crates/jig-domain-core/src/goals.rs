//! Core-owned funding admission for person-origin goals
//! (domain/engine.md, section 4.4; domain/authority.md, section 9).

use alloc::boxed::Box;
use jig_domain_authority as authority;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_lib::Queue;
use skein_lib::{ReplyTo, Token};

use crate::{Core, translate};

#[derive(Debug)]
pub struct CreateStart {
    /// Person pool charged by a direct start or later accepted proposal.
    pub source: tasks::Funder,
    /// Authority bounded to this goal's requested budget.
    pub authority: authority::Authority,
}

impl Core {
    /// Whether a configured recurring template is novel and affordable.
    #[must_use]
    pub fn recurring_admit(&self, project: u32, given: &tasks::Authority, template: &tasks::RecurringTemplate) -> bool {
        for member in &template.batch {
            match member.executor {
                tasks::Executor::Person(_) => return false,
                tasks::Executor::Agent { .. } | tasks::Executor::Procedure { .. } => {}
            }
        }
        for task in self.tasks.recurring_tasks(project) {
            if self.tasks.recurring_template(task).expect("identified recurring task").key == template.key {
                return false;
            }
        }
        let Some(policy) = self.authority.policy(project) else { return false };
        authority::at_most(&translate::authority_value(given), &policy.ceiling, &self.authority.rules().implies)
            && self.period_budget(project) <= policy.period_spend
    }

    /// Ask the task child to open a missing period or person pool before a
    /// goal starts; both requests join the same decision as the goal.
    #[must_use]
    pub fn open_person_pool(&self, project: u32, person: u64) -> (Option<tasks::Event>, Option<tasks::Event>) {
        let period = tasks::Funder::Period { project, period: self.period(project) };
        let open_period = if self.tasks.funding(period).is_none() {
            Some(tasks::Event::OpenPeriod {
                reply_to: ReplyTo::new(Token::new(u64::MAX)),
                project,
                period: self.period(project),
                budget: self.period_budget(project),
            })
        } else {
            None
        };
        let (pool, _) = self.person_pool(project, person);
        let carve_pool = if self.tasks.funding(pool).is_none() {
            Some(tasks::Event::CarvePool {
                reply_to: ReplyTo::new(Token::new(u64::MAX)),
                project,
                person,
                period: self.period(project),
                budget: self.pool_budget(project, person),
            })
        } else {
            None
        };
        (open_period, carve_pool)
    }

    /// Check the current holder's standing on one person-origin goal.
    pub fn goal_standing(
        &self,
        proposal: &tasks::PersonProposal,
        project: u32,
        role: Option<people::Role>,
    ) -> Result<(), people::Refusal> {
        if let tasks::PersonAction::ReleasePlaced { .. } = proposal.action {
            return Err(people::Refusal::NotWaiting);
        }
        if proposal.action.is_settings() {
            return if project == 0 && role == Some(people::Role::Owner) {
                Ok(())
            } else {
                Err(people::Refusal::Role { held: role })
            };
        }
        if proposal.project != project || role.is_none() {
            return Err(people::Refusal::Unknown);
        }
        let named = role.expect("checked role");
        let Some(policy) = self.authority.role(project, named.number()) else {
            return Err(people::Refusal::Unknown);
        };
        let kind = match &proposal.action {
            tasks::PersonAction::Settings { .. } => unreachable!("settings standing checked separately"),
            tasks::PersonAction::Batch(_) => authority::ProposalKind::Batch,
            tasks::PersonAction::Amend { .. } => authority::ProposalKind::Amend,
            tasks::PersonAction::ReleasePlaced { .. } => unreachable!("release recipient is the hold owner"),
        };
        if !policy.decides.allows(kind) {
            return Err(people::Refusal::Unknown);
        }
        Ok(())
    }

    /// Recheck authentic pool funding before a goal proposal is accepted.
    pub fn goal_accept_allowed(
        &self,
        proposal: &tasks::PersonProposal,
        project: u32,
        person: u64,
        role: people::Role,
        tasks_left: u32,
        decision: skein_lib::Wall,
    ) -> Result<tasks::Funder, people::Refusal> {
        let (source, numbers) = self.person_pool(project, person);
        match &proposal.action {
            tasks::PersonAction::Settings { .. } | tasks::PersonAction::ReleasePlaced { .. } => return Ok(source),
            tasks::PersonAction::Batch(batch) => {
                if let Err(why) = self.funded_accounts(batch, source) {
                    return Err(self.account_refusal(why));
                }
            }
            tasks::PersonAction::Amend { task, amendment, .. } => {
                if let Err(why) = self.amendment_account(*task, amendment, Some(source)) {
                    return Err(self.account_refusal(why));
                }
            }
        }
        let action = match &proposal.action {
            tasks::PersonAction::Settings { .. } | tasks::PersonAction::ReleasePlaced { .. } => return Ok(source),
            tasks::PersonAction::Batch(batch) => {
                let mut members =
                    skein_lib::List::with_capacity(u32::try_from(batch.len()).expect("bounded party batch"));
                for member in batch {
                    members
                        .push(authority::Delegate {
                            executor: self
                                .delegation_executor(project, member.executor)
                                .ok_or(people::Refusal::Unknown)?,
                            authority: crate::routing::deadlines::authority(
                                &member.authority,
                                member.deadline,
                                decision,
                            ),
                            symbolic: Box::new([]),
                        })
                        .expect("bounded party batch");
                }
                authority::Action::Batch(members.into_boxed())
            }
            tasks::PersonAction::Amend { task, amendment, .. } => {
                let row = self.tasks.task(*task).ok_or(people::Refusal::Unknown)?;
                authority::Action::Amend(translate::authority_value(
                    amendment.authority.as_ref().unwrap_or(&row.authority),
                ))
            }
        };
        let mut findings = Queue::with_capacity(authority::max_out(self.authority.limits()).expect("authority room"));
        let ask = authority::PersonAsk {
            project,
            role: role.number(),
            pool: translate::authority_numbers(numbers),
            tasks_left,
            request: authority::PersonRequest::Accept(action),
        };
        let checked = authority::check_request(&self.authority, &ask, &mut findings);
        if checked.answer == authority::Answer::Allow {
            Ok(source)
        } else {
            Err(self.request_refusal(&ask, checked.answer, &mut findings))
        }
    }

    /// Read the person's current pool, including their role's unopened allotment.
    #[must_use]
    pub fn person_pool(&self, project: u32, person: u64) -> (tasks::Funder, tasks::Numbers) {
        let source = tasks::Funder::Pool { project, person, period: self.period(project) };
        let numbers = match self.tasks.funding(source) {
            Some(row) => row.numbers,
            None => tasks::Numbers { budget: self.pool_budget(project, person), spent: 0, spent_below: 0, reserved: 0 },
        };
        (source, numbers)
    }
}
