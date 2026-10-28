//! The peers' plain delegated task fixture.
use super::{authority, core, tasks};

pub(super) fn delegate(words: Box<[u8]>, executor: tasks::Executor, budget: u64) -> core::Delegate {
    core::Delegate {
        deadline: jig_domain_tasks::Deadline::Named,
        label: 0,
        flags: jig_domain_tasks::Flags::PLAIN,
        tracked: None,
        budget: jig_domain_people::Budget::Named,
        executor,
        spec: tasks::Spec { asks: false, words, parameters: Box::new([]), inputs: Box::new([]) },
        contract: tasks::Contract::Report { words: 128 },
        authority: authority(budget, true),
        llm: None,
        symbolic_grants: Box::new([]),
        dependencies: Box::new([]),
        wake: tasks::WakePolicy::DEFAULT,
    }
}
