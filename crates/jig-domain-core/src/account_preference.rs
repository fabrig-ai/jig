//! Explicit account admission uses the prospective financial payer, never the requester
//! (accounts.md, 4.1–4.3; authority.md, 8.1; tasks.md, section 7).
use crate::Core;
use jig_domain_accounts as accounts;
use jig_domain_tasks as tasks;

impl Core {
    /// Availability is deliberately absent: a valid unavailable account is accepted and waits.
    pub(crate) fn account_preference(
        &self,
        choice: Option<&tasks::LlmChoice>,
        payer: tasks::Payer,
        kept: Option<u32>,
    ) -> Result<(), tasks::Refusal> {
        self.account_hint(choice, Some(payer), kept)
    }

    /// Proposals have no financial payer until acceptance.
    pub(crate) fn account_hint(
        &self,
        choice: Option<&tasks::LlmChoice>,
        payer: Option<tasks::Payer>,
        kept: Option<u32>,
    ) -> Result<(), tasks::Refusal> {
        let Some(choice) = choice else { return Ok(()) };
        let Some(number) = choice.account else { return Ok(()) };
        let model = self
            .settings
            .models
            .get(usize::from(choice.main.model))
            .ok_or(tasks::Refusal::Llm { model: choice.main.model, why: tasks::LlmFault::Unconfigured })?;
        let account = self.accounts.account(number).ok_or(refused(number, tasks::AccountWhy::Unknown))?;
        let provider = self.provider(model).ok_or(refused(number, tasks::AccountWhy::Provider))?;
        if account.provider() != provider {
            return Err(refused(number, tasks::AccountWhy::Provider));
        }
        if let Some(payer) = payer
            && !crate::coverage::serves(&self.people, account, crate::coverage::payer(payer))
        {
            return Err(refused(number, tasks::AccountWhy::Uses));
        }
        if let Some(old) = kept
            && old != number
            && let Some(kept) = self.accounts.account(old)
            && kept.provider() == provider
            && let Some(accounts::State::Limited { .. }) = self.accounts.account_state(old)
        {
            return Err(refused(number, tasks::AccountWhy::Limited));
        }
        Ok(())
    }

    /// The same account check before a standalone or accepted amendment reaches tasks.
    pub(crate) fn amendment_account(
        &self,
        task: u64,
        amendment: &tasks::Amendment,
        funder: Option<tasks::Funder>,
    ) -> Result<(), tasks::Refusal> {
        let Some(choice) = &amendment.llm else { return Ok(()) };
        if choice.account.is_none() {
            return Ok(());
        }
        let row = self.tasks.task(task).ok_or(tasks::Refusal::Unknown)?;
        executor(row.executor, Some(choice))?;
        let payer = self.tasks.prospective_payer(funder.unwrap_or(row.funder), &[]).ok_or(tasks::Refusal::Funding)?;
        self.account_preference(Some(choice), payer, row.kept_account)
    }
}

fn refused(account: u32, why: tasks::AccountWhy) -> tasks::Refusal {
    tasks::Refusal::Account { account, why }
}

impl Core {
    pub(crate) fn account_refusal(&self, why: tasks::Refusal) -> jig_domain_people::Refusal {
        let jig_domain_people::Outcome::Refused(why) = crate::routing::refusals::person_task_refusal(self, None, why)
        else {
            unreachable!("account admission has a permanent refusal");
        };
        why
    }

    /// All members of a funded proposal take the accepter's financial source.
    pub(crate) fn funded_accounts(&self, batch: &[tasks::New], funder: tasks::Funder) -> Result<(), tasks::Refusal> {
        for member in batch {
            executor(member.executor, member.llm.as_ref())?;
            if let Some(choice) = &member.llm
                && choice.account.is_some()
            {
                let payer = self.tasks.prospective_payer(funder, &[]).ok_or(tasks::Refusal::Funding)?;
                self.account_preference(Some(choice), payer, None)?;
            }
        }
        Ok(())
    }

    pub(crate) fn action_accounts(
        &self,
        action: &tasks::ProposalAction,
        funder: tasks::Funder,
    ) -> Result<(), tasks::Refusal> {
        match action {
            tasks::ProposalAction::Batch(batch) => self.funded_accounts(batch, funder),
            tasks::ProposalAction::Amend { task, amendment, .. } => {
                self.amendment_account(*task, amendment, Some(funder))
            }
            tasks::ProposalAction::Effect { .. }
            | tasks::ProposalAction::ProcedureEffect { .. }
            | tasks::ProposalAction::ReleasePlaced { .. }
            | tasks::ProposalAction::Release { .. }
            | tasks::ProposalAction::Widen { .. } => Ok(()),
        }
    }
}

impl Core {
    pub(crate) fn proposed_accounts(&self, batch: &[tasks::New]) -> Result<(), tasks::Refusal> {
        for member in batch {
            executor(member.executor, member.llm.as_ref())?;
            self.account_hint(member.llm.as_ref(), None, None)?;
        }
        Ok(())
    }

    pub(crate) fn proposed_action_accounts(&self, action: &tasks::ProposalAction) -> Result<(), tasks::Refusal> {
        match action {
            tasks::ProposalAction::Batch(batch) => self.proposed_accounts(batch),
            tasks::ProposalAction::Amend { task, amendment, .. } => {
                let row = self.tasks.task(*task).ok_or(tasks::Refusal::Unknown)?;
                executor(row.executor, amendment.llm.as_ref())?;
                self.account_hint(amendment.llm.as_ref(), None, row.kept_account)
            }
            tasks::ProposalAction::Effect { .. }
            | tasks::ProposalAction::ProcedureEffect { .. }
            | tasks::ProposalAction::ReleasePlaced { .. }
            | tasks::ProposalAction::Release { .. }
            | tasks::ProposalAction::Widen { .. } => Ok(()),
        }
    }

    /// Validate an accepted batch's explicit preferences without filling default models.
    #[expect(clippy::result_large_err, reason = "the refusal keeps tasks' exact problem for the creator")]
    pub(crate) fn batch_accounts(&self, batch: &[tasks::New]) -> Result<(), tasks::Problem> {
        let mut links = skein_lib::List::with_capacity(u32::try_from(batch.len()).expect("bounded batch"));
        for member in batch {
            links.push(tasks::PayerLink { task: member.number, funder: member.funder }).expect("one payer link");
        }
        for member in batch {
            if let Err(why) = executor(member.executor, member.llm.as_ref()) {
                return Err(tasks::Problem::new(Some(member.number), why));
            }
            if let Some(choice) = &member.llm
                && choice.account.is_some()
            {
                let payer = self
                    .tasks
                    .prospective_payer(member.funder, links.as_slice())
                    .ok_or(tasks::Problem::new(Some(member.number), tasks::Refusal::Funding))?;
                if let Err(why) = self.account_preference(Some(choice), payer, None) {
                    return Err(tasks::Problem::new(Some(member.number), why));
                }
            }
        }
        Ok(())
    }
}

impl Core {
    /// Every occurrence is charged to its project, regardless of template funder fields.
    pub(crate) fn recurring_accounts(&self, project: u32, batch: &[tasks::New]) -> Result<(), tasks::Refusal> {
        for member in batch {
            executor(member.executor, member.llm.as_ref())?;
            self.account_preference(member.llm.as_ref(), tasks::Payer::Project(project), None)?;
        }
        Ok(())
    }
}

pub(crate) fn executor(executor: tasks::Executor, choice: Option<&tasks::LlmChoice>) -> Result<(), tasks::Refusal> {
    if let Some(choice) = choice
        && choice.account.is_some()
    {
        match executor {
            tasks::Executor::Agent { .. } => {}
            tasks::Executor::Procedure { .. } | tasks::Executor::Person(_) => {
                return Err(tasks::Refusal::Llm { model: choice.main.model, why: tasks::LlmFault::Procedure });
            }
        }
    }
    Ok(())
}
