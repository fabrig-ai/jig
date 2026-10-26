//! Cross-child joins consume the completed live restore (domain/engine.md, 6).
use crate::{Core, tasks};

impl Core {
    pub(super) fn unresolved_tasks(&self) -> u32 {
        let mut records = 0_u32;
        for task in self.tasks.view_tasks() {
            let row = self.tasks.task(task.number).expect("live task view");
            if !self.task_references_resolve(row) {
                records = records.saturating_add(1);
            }
        }
        records
    }

    pub(crate) fn task_references_resolve(&self, row: &tasks::TaskRecord) -> bool {
        if self.authority.policy(row.project).is_none()
            || !self.party_resolves(row.requester)
            || !self.executor_resolves(row.executor)
            || !self.funder_person_resolves(row.funder)
        {
            return false;
        }
        if let Some(person) = row.taken_by
            && self.people.identity(person).is_none()
        {
            return false;
        }
        if let Some(answer) = row.person_answer
            && self.people.identity(answer.by).is_none()
        {
            return false;
        }
        if let Some(grace) = &row.grace
            && !self.party_resolves(grace.by)
        {
            return false;
        }
        for added in &row.added {
            if !self.party_resolves(added.by) {
                return false;
            }
        }
        for word in &row.inbox {
            if !self.party_resolves(word.from) {
                return false;
            }
        }
        if !self.phase_parties_resolve(row) {
            return false;
        }
        if let Some(proposal) = &row.proposal
            && !self.proposal_parties_resolve(proposal)
        {
            return false;
        }
        if let Some(recurring) = &row.recurring
            && !self.batch_parties_resolve(&recurring.template.batch)
        {
            return false;
        }
        true
    }

    fn phase_parties_resolve(&self, row: &tasks::TaskRecord) -> bool {
        if let tasks::Phase::Held { why: tasks::Hold::StoppedBy { party }, .. } = row.phase
            && self.people.identity(party).is_none()
        {
            return false;
        }
        let closing = match &row.phase {
            tasks::Phase::Closing(closing) | tasks::Phase::Held { was: tasks::Was::Closing(closing), .. } => {
                Some(closing)
            }
            tasks::Phase::Waiting | tasks::Phase::Active(_) | tasks::Phase::Held { .. } | tasks::Phase::Ended(_) => {
                None
            }
        };
        if let Some(closing) = closing
            && let tasks::Ending::Cancelled { by, .. } = closing.ending
            && !self.party_resolves(by)
        {
            return false;
        }
        match row.escalation {
            tasks::Escalation::Waiting { holder: tasks::EscalationHolder::Person(person), .. }
            | tasks::Escalation::Rejected { by: person, .. } => {
                if self.people.identity(person).is_none() {
                    return false;
                }
            }
            tasks::Escalation::Unheld { .. }
            | tasks::Escalation::Routing { .. }
            | tasks::Escalation::Waiting {
                holder: tasks::EscalationHolder::Task(_) | tasks::EscalationHolder::Role { .. },
                ..
            } => {}
        }
        true
    }

    fn proposal_parties_resolve(&self, proposal: &tasks::Proposal) -> bool {
        match proposal.state {
            tasks::ProposalState::Pending { holder: tasks::ProposalHolder::Person(person), .. } => {
                if self.people.identity(person).is_none() {
                    return false;
                }
            }
            tasks::ProposalState::Accepted { by } | tasks::ProposalState::Rejected { by, .. } => {
                if !self.party_resolves(by) {
                    return false;
                }
            }
            tasks::ProposalState::Pending {
                holder: tasks::ProposalHolder::Task(_) | tasks::ProposalHolder::Policy { .. },
                ..
            }
            | tasks::ProposalState::Withdrawn => {}
        }
        match &proposal.action {
            tasks::ProposalAction::Batch(batch) => {
                if !self.batch_parties_resolve(batch) {
                    return false;
                }
            }
            tasks::ProposalAction::Amend { amendment, .. } => {
                if let Some(addressed) = amendment.addressed
                    && !self.executor_resolves(tasks::Executor::Person(addressed.address))
                {
                    return false;
                }
            }
            tasks::ProposalAction::Effect { .. }
            | tasks::ProposalAction::ProcedureEffect { .. }
            | tasks::ProposalAction::Widen { .. }
            | tasks::ProposalAction::Release { .. } => {}
        }
        true
    }

    fn funder_person_resolves(&self, funder: tasks::Funder) -> bool {
        match funder {
            tasks::Funder::Pool { person, .. } => self.people.identity(person).is_some(),
            tasks::Funder::Task(_) | tasks::Funder::Period { .. } | tasks::Funder::Recurring { .. } => true,
        }
    }

    fn batch_parties_resolve(&self, batch: &[tasks::New]) -> bool {
        for member in batch {
            if !self.executor_resolves(member.executor) || !self.funder_person_resolves(member.funder) {
                return false;
            }
        }
        true
    }

    fn executor_resolves(&self, executor: tasks::Executor) -> bool {
        match executor {
            tasks::Executor::Person(tasks::PersonAddress::Person(person)) => self.people.identity(person).is_some(),
            tasks::Executor::Person(tasks::PersonAddress::Role(_))
            | tasks::Executor::Agent { .. }
            | tasks::Executor::Procedure { .. } => true,
        }
    }

    fn party_resolves(&self, party: tasks::Party) -> bool {
        match party {
            tasks::Party::Person(person) => self.people.identity(person).is_some(),
            tasks::Party::Task(_) | tasks::Party::Deployment { .. } => true,
        }
    }
}
