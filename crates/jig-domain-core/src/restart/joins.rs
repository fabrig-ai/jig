//! Cross-child joins consume the completed live restore (domain/engine.md, 6).
use crate::{Core, tasks};

impl Core {
    pub(crate) fn link_tasks(&self) {
        for task in self.tasks.view_tasks() {
            let row = self.tasks.task(task.number).expect("live task view");
            assert!(
                self.authority.policy(row.project).is_some(),
                "core: Live({}) links to project {}, which is absent",
                row.number,
                row.project
            );
            self.link_task_people(row);
        }
        let mut after = 0;
        while let Some(hold) = self.tasks.next_placed_hold(after) {
            after = hold.number;
            let owner = match hold.owner {
                tasks::Owner::Party { person } => tasks::Party::Person(person),
                tasks::Owner::Task { task } => tasks::Party::Task(task),
            };
            assert!(
                self.authority.policy(hold.project).is_some(),
                "core: PlacedHold({}) links to project {}, which is absent",
                hold.number,
                hold.project
            );
            assert!(
                self.party_resolves(owner),
                "core: PlacedHold({}) links to owner {owner:?}, which is absent",
                hold.number
            );
        }
    }

    fn link_person(&self, task: u64, field: &str, person: u64) {
        assert!(
            self.people.identity(person).is_some(),
            "core: Live({task}).{field} links to person {person}, which is absent"
        );
    }
    fn link_party(&self, task: u64, field: &str, party: tasks::Party) {
        match party {
            tasks::Party::Person(person) => self.link_person(task, field, person),
            tasks::Party::Task(_) | tasks::Party::Deployment { .. } => {}
        }
    }
    fn link_executor(&self, task: u64, field: &str, executor: tasks::Executor) {
        if let tasks::Executor::Person(tasks::PersonAddress::Person(person)) = executor {
            self.link_person(task, field, person);
        }
    }
    fn link_funder(&self, task: u64, field: &str, funder: tasks::Funder) {
        if let tasks::Funder::Pool { person, .. } = funder {
            self.link_person(task, field, person);
        }
    }
    fn link_batch_people(&self, task: u64, batch: &[tasks::New]) {
        for member in batch {
            self.link_executor(task, "batch.executor", member.executor);
            self.link_funder(task, "batch.funder", member.funder);
        }
    }
    fn link_task_people(&self, row: &tasks::TaskRecord) {
        let task = row.number;
        self.link_party(task, "requester", row.requester);
        self.link_executor(task, "executor", row.executor);
        self.link_funder(task, "funder", row.funder);
        if let Some(person) = row.taken_by {
            self.link_person(task, "taken_by", person);
        }
        if let Some(answer) = row.person_answer {
            self.link_person(task, "person_answer.by", answer.by);
        }
        if let Some(grace) = &row.grace {
            self.link_party(task, "grace.by", grace.by);
        }
        for added in &row.added {
            self.link_party(task, "added.by", added.by);
        }
        for word in &row.inbox {
            self.link_party(task, "inbox.from", word.from);
        }
        if let Some(number) = row.placed {
            let hold = self.tasks.placed_hold(number);
            assert!(hold.is_some(), "core: Live({task}).placed links to PlacedHold({number}), which is absent");
            let hold = hold.expect("placed hold linked above");
            assert!(
                self.authority.policy(hold.project).is_some(),
                "core: PlacedHold({number}).project links to project {}, which is absent",
                hold.project
            );
            if let tasks::Owner::Party { person } = hold.owner {
                self.link_person(task, "placed.owner", person);
            }
        }
        match &row.phase {
            tasks::Phase::Closing(closing) | tasks::Phase::Held { was: tasks::Was::Closing(closing), .. } => {
                if let tasks::Ending::Cancelled { by, .. } = closing.ending {
                    self.link_party(task, "ending.by", by);
                }
            }
            tasks::Phase::Waiting | tasks::Phase::Active(_) | tasks::Phase::Held { .. } | tasks::Phase::Ended(_) => {}
        }
        match row.escalation {
            tasks::Escalation::Waiting { holder: tasks::EscalationHolder::Person(person), .. }
            | tasks::Escalation::Rejected { by: person, .. } => self.link_person(task, "escalation.holder", person),
            tasks::Escalation::Unheld { .. }
            | tasks::Escalation::Routing { .. }
            | tasks::Escalation::Waiting { .. } => {}
        }
        if let Some(proposal) = &row.proposal {
            match proposal.state {
                tasks::ProposalState::Pending { holder: tasks::ProposalHolder::Person(person), .. } => {
                    self.link_person(task, "proposal.holder", person);
                }
                tasks::ProposalState::Accepted { by } | tasks::ProposalState::Rejected { by, .. } => {
                    self.link_party(task, "proposal.by", by);
                }
                tasks::ProposalState::Pending { .. } | tasks::ProposalState::Withdrawn => {}
            }
            match &proposal.action {
                tasks::ProposalAction::Batch(batch) => self.link_batch_people(task, batch),
                tasks::ProposalAction::Amend { amendment, .. } => {
                    if let Some(addressed) = amendment.addressed {
                        self.link_executor(task, "proposal.addressed", tasks::Executor::Person(addressed.address));
                    }
                }
                tasks::ProposalAction::Settings { .. }
                | tasks::ProposalAction::Effect { .. }
                | tasks::ProposalAction::ProcedureEffect { .. }
                | tasks::ProposalAction::Widen { .. }
                | tasks::ProposalAction::ReleasePlaced { .. }
                | tasks::ProposalAction::Release { .. } => {}
            }
        }
        if let Some(recurring) = &row.recurring {
            self.link_batch_people(task, &recurring.template.batch);
        }
    }

    #[cfg(test)]
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

    #[cfg(test)]
    fn phase_parties_resolve(&self, row: &tasks::TaskRecord) -> bool {
        if let Some(number) = row.placed {
            let Some(hold) = self.tasks.placed_hold(number) else { return false };
            if self.authority.policy(hold.project).is_none() {
                return false;
            }
            if let tasks::Owner::Party { person } = hold.owner
                && self.people.identity(person).is_none()
            {
                return false;
            }
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

    #[cfg(test)]
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
            tasks::ProposalAction::Settings { .. }
            | tasks::ProposalAction::Effect { .. }
            | tasks::ProposalAction::ProcedureEffect { .. }
            | tasks::ProposalAction::Widen { .. }
            | tasks::ProposalAction::ReleasePlaced { .. }
            | tasks::ProposalAction::Release { .. } => {}
        }
        true
    }

    #[cfg(test)]
    fn funder_person_resolves(&self, funder: tasks::Funder) -> bool {
        match funder {
            tasks::Funder::Pool { person, .. } => self.people.identity(person).is_some(),
            tasks::Funder::Task(_) | tasks::Funder::Period { .. } | tasks::Funder::Recurring { .. } => true,
        }
    }

    #[cfg(test)]
    fn batch_parties_resolve(&self, batch: &[tasks::New]) -> bool {
        for member in batch {
            if !self.executor_resolves(member.executor) || !self.funder_person_resolves(member.funder) {
                return false;
            }
        }
        true
    }

    #[cfg(test)]
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
