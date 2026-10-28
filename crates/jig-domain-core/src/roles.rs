//! Core-owned role and funding admission for authenticated parties
//! (domain/engine.md, sections 3 and 4.4).

use jig_domain_authority as authority;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_lib::Queue;

use crate::{Core, Limits, translate};

impl Core {
    /// Admit a person's funded move of a live task.
    pub fn move_admit(
        &self,
        person: u64,
        role: Option<people::Role>,
        project: u32,
        task: u64,
        tasks_left: u32,
        depth: u32,
    ) -> Result<(), people::Refusal> {
        self.move_admit_in(
            crate::compound::PartyTask { person, project, role, task },
            tasks_left,
            depth,
            &tasks::CompoundAdmission::new(&self.tasks, 0),
        )
    }

    pub(crate) fn move_admit_in(
        &self,
        party: crate::compound::PartyTask,
        tasks_left: u32,
        depth: u32,
        admission: &tasks::CompoundAdmission<'_>,
    ) -> Result<(), people::Refusal> {
        let crate::compound::PartyTask { person, project, role, task } = party;
        let Some(context) = admission.task(task) else { return Err(people::Refusal::Unknown) };
        if context.project != project {
            return Err(people::Refusal::Unknown);
        }
        if role.is_none() {
            return Err(people::Refusal::Role { held: None });
        }
        let holding = role.expect("checked member");
        let Some(role_policy) = self.authority.role(project, holding.number()) else {
            unreachable!("member belongs to configured policy")
        };
        let standing = Self::person_tree_in(person, task, depth, admission);
        if !role_policy.requests.allows(authority::RequestKind::Amend) && !standing {
            return Err(people::Refusal::Role { held: Some(holding) });
        }
        if !self.authority.escalations_held(project, holding.number()) {
            return Err(people::Refusal::NoEscalationHolder);
        }
        let Some(policy) = self.authority.policy(project) else { unreachable!("configured policy") };
        if self.period_budget(project) > policy.period_spend {
            return Err(people::Refusal::Authority {
                proposable: None,
                lacking: people::Lack::Budget { needs: self.period_budget(project), left: policy.period_spend },
            });
        }
        let Some(left) = context.takeover_budget() else {
            return Err(people::Refusal::Limit { which: people::LimitKind::Arithmetic });
        };
        let (source, fallback) = self.person_pool(project, person);
        let pool_numbers = match admission.funding(source) {
            Some(numbers) => numbers,
            None => fallback,
        };
        let mut giving = translate::authority_value(context.authority);
        giving.budget.spend = left;
        let mut findings = Queue::uncapped();
        let ask = authority::PersonAsk {
            project,
            role: holding.number(),
            pool: translate::authority_numbers(pool_numbers),
            tasks_left,
            request: authority::PersonRequest::Move(giving),
        };
        let checked = authority::check_request_with_standing(&self.authority, &ask, true, &mut findings);
        if checked.answer != authority::Answer::Allow {
            return Err(self.request_refusal(&ask, checked.answer, &mut findings));
        }
        if !context.takeover_state(person) {
            let people::Outcome::Refused(why) =
                crate::routing::refusals::person_task_refusal(self, Some(task), tasks::Refusal::State)
            else {
                unreachable!("a state refusal is permanent")
            };
            return Err(why);
        }
        Ok(())
    }

    /// Check standing, current policy and financial authority for a party's
    /// stop, cancel or release request before routing it to the task child.
    pub fn control_admit(
        &self,
        ask: &people::Ask,
        person: u64,
        project: u32,
        role: Option<people::Role>,
        limits: &tasks::Limits,
    ) -> Result<u64, people::Refusal> {
        self.control_admit_in(ask, person, project, role, limits, &tasks::CompoundAdmission::new(&self.tasks, 0))
    }

    pub(crate) fn control_admit_in(
        &self,
        ask: &people::Ask,
        person: u64,
        project: u32,
        role: Option<people::Role>,
        limits: &tasks::Limits,
        admission: &tasks::CompoundAdmission<'_>,
    ) -> Result<u64, people::Refusal> {
        let (task, kind, reason_len) = match ask {
            people::Ask::Stop { task, .. } => (*task, authority::RequestKind::Cancel, 0),
            people::Ask::Cancel { task, reason, .. } => (*task, authority::RequestKind::Cancel, reason.len()),
            people::Ask::Release { task, .. } => (*task, authority::RequestKind::Release, 0),
            people::Ask::Lift { .. }
            | people::Ask::Compound { .. }
            | people::Ask::MarkSeen { .. }
            | people::Ask::Application { .. }
            | people::Ask::TakePerson { .. }
            | people::Ask::HandBackPerson { .. }
            | people::Ask::AnswerPerson { .. }
            | people::Ask::Move { .. }
            | people::Ask::DecideProposal { .. }
            | people::Ask::Say { .. }
            | people::Ask::AnswerQuestion { .. }
            | people::Ask::Amend { .. }
            | people::Ask::EditNote { .. }
            | people::Ask::MakeService { .. }
            | people::Ask::SetRoles { .. }
            | people::Ask::Adopt { .. }
            | people::Ask::Rename { .. }
            | people::Ask::MakeProject { .. }
            | people::Ask::ChangeSettings { .. }
            | people::Ask::MoveToNewest { .. }
            | people::Ask::ChangePolicy { .. }
            | people::Ask::SetPool { .. }
            | people::Ask::DecideEscalation { .. }
            | people::Ask::Create { .. }
            | people::Ask::Propose { .. }
            | people::Ask::Withdraw { .. }
            | people::Ask::AccountSignIn { .. }
            | people::Ask::AccountSignOut { .. }
            | people::Ask::AccountChange { .. } => unreachable!("control route owns its ask"),
        };
        if let people::Ask::Cancel { grace: Some(span), .. } = ask
            && *span > limits.grace
        {
            return Err(people::Refusal::Limit { which: people::LimitKind::Grace });
        }
        let Some(context) = admission.task(task) else { return Err(people::Refusal::Unknown) };
        if let people::Ask::Release { again, .. } = ask {
            crate::revisions::release_run(&context, *again)?;
        }
        let Some(holding) = role else { return Err(people::Refusal::Role { held: None }) };
        let any_task = match holding {
            people::Role::Owner | people::Role::Maintainer => true,
            people::Role::Member | people::Role::Observer => false,
            people::Role::Policy { .. } => match self.authority.role(project, holding.number()) {
                Some(policy) => policy.requests.allows(kind),
                None => false,
            },
        };
        let standing = Self::person_tree_in(person, task, limits.depth, admission);
        if context.project != project || !any_task && !standing {
            return Err(people::Refusal::Unknown);
        }
        let pool = tasks::Funder::Pool { project, person, period: self.period(project) };
        let numbers = match admission.funding(pool) {
            Some(numbers) => numbers,
            None => tasks::Numbers { budget: 0, spent: 0, spent_below: 0, reserved: 0 },
        };
        let action = if kind == authority::RequestKind::Cancel {
            authority::PersonRequest::Cancel
        } else {
            authority::PersonRequest::Release
        };
        let mut findings = Queue::uncapped();
        let ask = authority::PersonAsk {
            project,
            role: holding.number(),
            pool: translate::authority_numbers(numbers),
            tasks_left: context.tasks_left().ok_or(people::Refusal::Limit { which: people::LimitKind::Arithmetic })?,
            request: action,
        };
        let checked = authority::check_request_with_standing(&self.authority, &ask, standing, &mut findings);
        if checked.answer != authority::Answer::Allow {
            return Err(self.request_refusal(&ask, checked.answer, &mut findings));
        }
        if kind == authority::RequestKind::Release
            && let Some(hold) = self.tasks.placed_on(task)
        {
            self.party_hold_admit(person, hold)?;
        }
        if reason_len > usize::try_from(limits.result_bytes).expect("u32 fits usize") {
            Err(people::Refusal::Limit { which: people::LimitKind::Words })
        } else {
            Ok(task)
        }
    }

    /// Whether a party may name this live task within its project.
    #[must_use]
    pub fn reaches(&self, person: u64, role: people::Role, project: u32, task: u64, depth: u32) -> bool {
        self.reaches_in(
            crate::compound::PartyTask { person, role: Some(role), project, task },
            depth,
            &tasks::CompoundAdmission::new(&self.tasks, 0),
        )
    }
    pub(crate) fn reaches_in(
        &self,
        party: crate::compound::PartyTask,
        depth: u32,
        admission: &tasks::CompoundAdmission<'_>,
    ) -> bool {
        let crate::compound::PartyTask { person, role, project, task } = party;
        let Some(role) = role else { return false };
        let Some(context) = admission.task(task) else { return false };
        context.project == project
            && (self.reaches_project(role, project) || Self::person_tree_in(person, task, depth, admission))
    }
    pub(crate) fn reaches_project(&self, role: people::Role, project: u32) -> bool {
        match role {
            people::Role::Owner | people::Role::Maintainer => true,
            people::Role::Member | people::Role::Observer => false,
            people::Role::Policy { .. } => match self.authority.role(project, role.number()) {
                Some(policy) => policy.requests.allows(authority::RequestKind::Amend),
                None => false,
            },
        }
    }

    /// Whether a person is the requesting ancestor of a live task.
    #[must_use]
    pub fn person_tree(&self, person: u64, task: u64, depth: u32) -> bool {
        Self::person_tree_in(person, task, depth, &tasks::CompoundAdmission::new(&self.tasks, 0))
    }
    pub(crate) fn person_tree_in(person: u64, task: u64, depth: u32, admission: &tasks::CompoundAdmission<'_>) -> bool {
        let mut next = task;
        for _ in 0..=depth {
            let Some(context) = admission.task(next) else { return false };
            match context.requester {
                tasks::Party::Person(requester) => return requester == person,
                tasks::Party::Task(parent) => next = parent,
                tasks::Party::Deployment { .. } => return false,
            }
        }
        false
    }

    /// Whether a person or their current role holds a task's escalation.
    #[must_use]
    pub fn person_escalation_recipient(&self, person: u64, role: people::Role, task: u64) -> bool {
        let Some(context) = self.tasks.escalation(task) else { return false };
        match context.escalation {
            tasks::Escalation::Waiting { holder: tasks::EscalationHolder::Person(holder), .. } => holder == person,
            tasks::Escalation::Waiting { holder: tasks::EscalationHolder::Role { project, role: holder }, .. } => {
                project == context.project && holder == role.number()
            }
            tasks::Escalation::Waiting { holder: tasks::EscalationHolder::Task(_), .. }
            | tasks::Escalation::Unheld { .. }
            | tasks::Escalation::Routing { .. }
            | tasks::Escalation::Rejected { .. } => false,
        }
    }

    /// Check a party role's current note scope, including connector paths.
    #[must_use]
    pub fn note_authorized(&self, project: u32, role: Option<people::Role>, scope: &people::NoteScope) -> bool {
        let Some(role) = role else { return false };
        let Some(policy) = self.authority.role(project, role.number()) else { return false };
        match scope {
            people::NoteScope::Deployment => policy.authority.notes.0 & 4 != 0,
            people::NoteScope::Project => policy.authority.notes.0 & 2 != 0,
            people::NoteScope::Goal { .. } => policy.authority.notes.0 & 1 != 0,
            people::NoteScope::Resources { connector, pattern } => {
                let needed = translate::pattern_to_authority(pattern.clone());
                for offered in &policy.authority.note_resources {
                    if offered.connector == *connector && authority::pattern_at_most(&needed, &offered.pattern) {
                        return true;
                    }
                }
                false
            }
        }
    }

    /// Preflight every waiting escalation against a candidate role roster
    /// before applying any membership change.
    pub fn roles_preflight(
        &self,
        limits: &Limits,
        contexts: &[tasks::EscalationContext],
        holdings: &[people::Holding],
    ) -> Result<u32, people::Refusal> {
        let mut changed = 0_u32;
        for context in contexts {
            let mut role = None;
            for holding in holdings {
                if holding.person == context.requester {
                    role = Some(holding.role);
                }
            }
            let Some(holder) = self.escalation_recipient(limits, context, role) else {
                return Err(people::Refusal::Authority { proposable: None, lacking: people::Lack::Shape });
            };
            match context.escalation {
                tasks::Escalation::Waiting { holder: old, .. } => {
                    if old != holder {
                        changed = changed.checked_add(1).expect("bounded task count");
                    }
                }
                tasks::Escalation::Unheld { .. }
                | tasks::Escalation::Routing { .. }
                | tasks::Escalation::Rejected { .. } => unreachable!("inspection contains waiting contexts only"),
            }
        }
        Ok(changed)
    }

    /// Admit an owner asking to change current role policy.
    pub fn role_allowed(&self, person: u64, project: u32, tasks_left: u32) -> Result<(), people::Refusal> {
        self.role_allowed_for(person, project, tasks_left, crate::acts::Act::ChangePolicy)
    }

    pub(crate) fn role_allowed_for(
        &self,
        person: u64,
        project: u32,
        tasks_left: u32,
        act: crate::acts::Act,
    ) -> Result<(), people::Refusal> {
        crate::acts::role(self, project, self.people.role(person, project), act)?;
        if self.authority.policy(project).is_none() {
            return Err(people::Refusal::Unknown);
        }
        let pool = tasks::Funder::Pool { project, person, period: self.period(project) };
        let numbers = match self.tasks.funding(pool) {
            Some(record) => record.numbers,
            None => tasks::Numbers { budget: 0, spent: 0, spent_below: 0, reserved: 0 },
        };
        let mut findings = Queue::uncapped();
        let ask = authority::PersonAsk {
            project,
            role: people::Role::Owner.number(),
            pool: authority::Numbers {
                budget: numbers.budget,
                spent: numbers.spent,
                spent_below: numbers.spent_below,
                reserved: numbers.reserved,
            },
            tasks_left,
            request: authority::PersonRequest::Policy,
        };
        let checked = authority::check_request(&self.authority, &ask, &mut findings);
        if checked.answer == authority::Answer::Allow {
            Ok(())
        } else {
            Err(self.request_refusal(&ask, checked.answer, &mut findings))
        }
    }
}

impl Core {
    pub(crate) fn reorder_allowed(&self, project: u32, role: Option<people::Role>) -> Result<(), people::Refusal> {
        crate::acts::role(self, project, role, crate::acts::Act::Reorder)?;
        let named = role.expect("common priority role judgment admitted membership");
        let allowed = match self.authority.role(project, named.number()) {
            Some(policy) => policy.requests.allows(authority::RequestKind::Amend),
            None => false,
        };
        if !allowed {
            return Err(people::Refusal::Role { held: role });
        }
        Ok(())
    }
}

/// Project roles' precedence for placed holds (domain/tasks.md, 5.9).
pub(crate) const fn rank(role: people::Role) -> u8 {
    match role {
        people::Role::Owner => 3,
        people::Role::Maintainer => 2,
        people::Role::Member | people::Role::Policy { .. } => 1,
        people::Role::Observer => 0,
    }
}
pub(crate) fn outranks(core: &Core, project: u32, party: u64, owner_person: u64) -> bool {
    let Some(role) = core.people.role(party, project) else { return false };
    match core.people.role(owner_person, project) {
        Some(owner) => rank(role) > rank(owner),
        None => rank(role) >= rank(people::Role::Member),
    }
}
pub(crate) const fn hold_owner(owner: tasks::Owner) -> people::HoldOwner {
    match owner {
        tasks::Owner::Party { person } => people::HoldOwner::Party { person },
        tasks::Owner::Task { task } => people::HoldOwner::Task { task },
    }
}
impl Core {
    pub(crate) fn party_hold_admit(&self, person: u64, hold: &tasks::PlacedHold) -> Result<(), people::Refusal> {
        let above = match hold.owner {
            tasks::Owner::Party { person: owner } => outranks(self, hold.project, person, owner),
            tasks::Owner::Task { .. } => false,
        };
        if self.tasks.may_lift(hold.number, tasks::Party::Person(person), above) {
            Ok(())
        } else {
            Err(people::Refusal::Authority {
                lacking: people::Lack::HoldOwner { hold: hold.number, owner: hold_owner(hold.owner) },
                proposable: None,
            })
        }
    }
}
