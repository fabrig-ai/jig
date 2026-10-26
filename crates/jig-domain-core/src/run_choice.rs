//! A current joint account/source/host check (accounts.md, 4.3; engine.md, 7.1).
use crate::{Core, RunPolicy};
use alloc::boxed::Box;
use jig_domain_accounts as accounts;
use jig_domain_authority as authority;
use jig_domain_fleet as fleet;
use skein_lib::List;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RunWait {
    Account { account: Option<u32>, source: Option<u32>, why: accounts::WaitWhy },
    InvalidAccount { account: u32, why: jig_domain_tasks::AccountWhy },
    Credential,
    Host,
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RunChoice {
    pub placement: fleet::PlacementConstraint,
    pub accounts: Box<[authority::AccountBinding]>,
    pub policy: RunPolicy,
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RunAllowance {
    pub choice: RunChoice,
    pub spend: u64,
}

impl Core {
    /// Probing retains no state or grants. At most accounts × (hosts + 1) probes build
    /// at most one fact per source, joining each host's kept logins to issued holders.
    #[must_use = "retain the checked choice or its waiting reason"]
    pub fn run_choice(&self, task: u64) -> Result<RunChoice, RunWait> {
        let row = self.tasks.task(task).ok_or(RunWait::Host)?;
        let payer = self.tasks.payer(task).ok_or(RunWait::Host)?;
        if row.kept_account.is_none()
            && let jig_domain_tasks::Funder::Recurring { .. } = row.funder
            && let Err(jig_domain_tasks::Refusal::Account { account, why }) =
                self.account_preference(row.llm.as_ref(), payer, None)
        {
            return Err(RunWait::InvalidAccount { account, why });
        }
        let coverage = self.account_coverage(payer);
        let policy = self.chosen_policy(task);
        let provider = self.provider(&policy.model).ok_or(account_wait(None))?;
        let preferred = row.kept_account.or(match &row.llm {
            Some(choice) => choice.account,
            None => None,
        });
        let mut excluded = None;
        if let Some(old) = row.kept_account
            && let Some(account) = self.accounts.account(old)
            && let Some(accounts::State::Limited { .. }) = self.accounts.account_state(old)
        {
            excluded = Some(accounts::Excluded { holder: account.holder(), provider: account.provider() });
        }
        let candidates = self.accounts.candidates(
            crate::coverage::payer(payer),
            provider,
            policy.model.program.as_deref(),
            preferred,
            excluded,
            coverage.as_slice(),
        );
        let mut waiting = None;
        let mut host_wait = false;
        let kinds = self.charter_of(task).hosts;
        let main = crate::fleet_needs::model(&policy.model);
        let models = core::slice::from_ref(&main);
        let version = self.charter_version(task);
        let mut facts = List::with_capacity(self.host_fact_capacity());
        for account in &candidates {
            let mut tried = List::with_capacity(self.fleet.host_capacity());
            for _ in 0..=self.fleet.host_capacity() {
                let Some(placement) = self.fleet.probe(kinds, task, version, models, tried.as_slice()) else {
                    host_wait = true;
                    break;
                };
                self.host_facts(placement, self.host_rule(placement), &mut facts);
                let selection = self.accounts.select(
                    crate::coverage::payer(payer),
                    provider,
                    policy.model.program.as_deref(),
                    Some(*account),
                    None,
                    coverage.as_slice(),
                    facts.as_slice(),
                    self.run_now,
                );
                let source = match selection {
                    accounts::Selection::Chosen { source, .. } => source,
                    accounts::Selection::Wait { why: accounts::WaitWhy::Host, .. } => {
                        host_wait = true;
                        tried.push(placement.channel).expect("one rejected channel per host");
                        continue;
                    }
                    accounts::Selection::Wait { account, source, why } => {
                        if waiting.is_none() {
                            waiting = Some(RunWait::Account { account: Some(account), source, why });
                        }
                        break;
                    }
                    accounts::Selection::NoAccount => break,
                };
                host_wait = true;
                let (bindings, retained, eligible) = self.compatible_models(
                    task,
                    placement,
                    &policy,
                    authority::AccountBinding { provider, account: *account, source },
                    facts.as_slice(),
                    coverage.as_slice(),
                );
                let mut needs = List::with_capacity(jig_domain_tasks::MODELS.checked_add(1).expect("main need"));
                needs.push(main.clone()).expect("main need");
                for model in &retained {
                    needs.push(crate::fleet_needs::model(model)).expect("retained models");
                }
                if eligible && self.fleet.supports(placement, needs.as_slice()) {
                    let mut selected = policy.clone();
                    selected.alternatives = retained;
                    return Ok(RunChoice { placement, accounts: bindings, policy: selected });
                }
                tried.push(placement.channel).expect("one rejected channel per host");
            }
        }
        if host_wait { Err(RunWait::Host) } else { Err(waiting.unwrap_or(account_wait(None))) }
    }

    /// Retain alternatives with compatible grants and check their host support.
    /// Selection and claim rebuild the same model/grant view for every task
    /// (hosts.md, 8.1).
    fn compatible_models(
        &self,
        task: u64,
        placement: fleet::PlacementConstraint,
        policy: &RunPolicy,
        main: authority::AccountBinding,
        facts: &[accounts::HostSource],
        coverage: &[accounts::Coverage],
    ) -> (Box<[authority::AccountBinding]>, Box<[crate::Model]>, bool) {
        let payer = self.tasks.payer(task).expect("live selected task");
        let provider = main.provider;
        let mut eligible =
            self.fleet.supports(placement, core::slice::from_ref(&crate::fleet_needs::model(&policy.model)));
        let mut bindings = List::with_capacity(jig_domain_tasks::MODELS);
        bindings.push(main).expect("main binding");
        let mut retained = List::with_capacity(jig_domain_tasks::MODELS);
        for model in &policy.alternatives {
            let Some(other) = self.provider(model) else { continue };
            if (other == provider && model.program != policy.model.program)
                || (other != provider && model.program.is_some())
            {
                continue;
            }
            let mut bound = false;
            for binding in &bindings {
                if binding.provider == other {
                    bound = true;
                }
            }
            let declared = self.fleet.supports(placement, core::slice::from_ref(&crate::fleet_needs::model(model)));
            if bound {
                eligible &= declared;
            } else if let accounts::Selection::Chosen { account, source } = self.accounts.select(
                crate::coverage::payer(payer),
                other,
                model.program.as_deref(),
                None,
                None,
                coverage,
                facts,
                self.run_now,
            ) {
                eligible &= declared;
                let binding = authority::AccountBinding { provider: other, account, source };
                bindings.push(binding).expect("one binding per retained provider");
                bound = true;
            }
            if bound {
                retained.push(model.clone()).expect("configured model bound");
            }
        }
        (bindings.into_boxed(), retained.into_boxed(), eligible)
    }

    pub(crate) fn provider(&self, model: &crate::Model) -> Option<u32> {
        Some(self.settings.llm_providers.serving(&model.name)?.number)
    }
}
fn account_wait(account: Option<u32>) -> RunWait {
    RunWait::Account { account, source: None, why: accounts::WaitWhy::Empty }
}

impl Core {
    /// Recheck recorded identities before a newly placed assignment leaves.
    #[must_use]
    pub(crate) fn claim_bindings_usable(&self, task: u64, attempt: u64) -> bool {
        let Some(proof) = self.proofs.get(&task) else { return false };
        if proof.attempt != attempt || proof.accounts.is_empty() {
            return false;
        }
        let Some(payer) = self.tasks.payer(task) else { return false };
        let coverage = self.account_coverage(payer);
        let channel = match proof.placement {
            Some(channel) => channel,
            None => match proof.host {
                fleet::HostKind::Engine => skein_lib::Token::new(0),
                fleet::HostKind::Worker => return false,
            },
        };
        let placement = fleet::PlacementConstraint { channel, kind: proof.host };
        if proof.host == fleet::HostKind::Worker && self.fleet.awaiting_logins(channel) {
            return false;
        }
        let mut facts = List::with_capacity(self.host_fact_capacity());
        self.host_facts(placement, self.host_rule(placement), &mut facts);
        let main = self.chosen_main(task);
        let main_provider = self.provider(main);
        for binding in &proof.accounts {
            let program = if main_provider == Some(binding.provider) { main.program.as_deref() } else { None };
            let selected = self.accounts.select(
                crate::coverage::payer(payer),
                binding.provider,
                program,
                Some(binding.account),
                None,
                coverage.as_slice(),
                facts.as_slice(),
                self.run_now,
            );
            if selected != (accounts::Selection::Chosen { account: binding.account, source: binding.source }) {
                return false;
            }
        }
        let policy = self.chosen_policy(task);
        let mut main_binding = None;
        for binding in &proof.accounts {
            if main_provider == Some(binding.provider) {
                main_binding = Some(*binding);
            }
        }
        let Some(main_binding) = main_binding else {
            return false;
        };
        let (bindings, models, eligible) =
            self.compatible_models(task, placement, &policy, main_binding, facts.as_slice(), coverage.as_slice());
        if !eligible || bindings.as_ref() != proof.accounts.as_ref() {
            return false;
        }
        let mut needs = List::with_capacity(jig_domain_tasks::MODELS.checked_add(1).expect("main need"));
        needs.push(crate::fleet_needs::model(main)).expect("main need");
        for model in &models {
            needs.push(crate::fleet_needs::model(model)).expect("retained model");
        }
        if !self.fleet.supports(placement, needs.as_slice()) {
            return false;
        }
        self.credentials_ready(task)
    }
}
