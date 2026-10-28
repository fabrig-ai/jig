//! Admission facts come from the immutable startup limits; contract.md, 4.
use crate::Core;
use jig_domain_people::{LimitKey as K, LimitKind as L, LimitUnit as U, Refusal};
impl Core {
    pub(crate) fn limit_refusal(&self, which: L) -> Refusal {
        let limits = &self.refusal_limits;
        match which {
            L::Words => Refusal::configured(which, u64::from(limits.people.words), K::PeopleWords, U::Bytes),
            L::Amendment => {
                Refusal::configured(which, u64::from(limits.people.amendment_bytes), K::PeopleAmendmentBytes, U::Bytes)
            }
            L::Identity => {
                Refusal::configured(which, u64::from(limits.people.identity_bytes), K::PeopleIdentityBytes, U::Bytes)
            }
            L::Projects => Refusal::configured(which, u64::from(limits.people.projects), K::PeopleProjects, U::Items),
            L::Operations => {
                Refusal::configured(which, u64::from(limits.people.operations), K::PeopleOperations, U::Items)
            }
            L::Batch => Refusal::configured(which, u64::from(limits.tasks.batch), K::TasksBatch, U::Items),
            L::Goals => {
                Refusal::configured(which, u64::from(limits.tasks.project_goals), K::TasksProjectGoals, U::Items)
            }
            L::ProjectLiveTasks => {
                Refusal::configured(which, u64::from(limits.tasks.project_tasks), K::TasksProjectTasks, U::Items)
            }
            L::LiveTasks => Refusal::configured(which, u64::from(limits.tasks.tasks), K::TasksTasks, U::Items),
            L::Tree => Refusal::configured(which, u64::from(limits.tasks.tree_tasks), K::TasksTreeTasks, U::Items),
            L::Depth => Refusal::configured(which, u64::from(limits.tasks.depth), K::TasksDepth, U::Items),
            L::Delegates => Refusal::configured(which, u64::from(limits.tasks.delegates), K::TasksDelegates, U::Items),
            L::Accounts => {
                Refusal::configured(which, u64::from(limits.accounts.accounts), K::AccountsAccounts, U::Items)
            }
            L::HolderAccounts => Refusal::configured(
                which,
                u64::from(limits.accounts.holder_accounts),
                K::AccountsHolderAccounts,
                U::Items,
            ),
            L::Sources => Refusal::configured(which, u64::from(limits.accounts.sources), K::AccountsSources, U::Items),
            L::SignIns => Refusal::configured(which, u64::from(limits.accounts.sign_ins), K::AccountsSignIns, U::Items),
            L::SettingsProposals => {
                Refusal::configured(which, u64::from(limits.settings.proposals), K::CoreSettingsProposals, U::Items)
            }
            L::PlacedHolds => {
                Refusal::configured(which, u64::from(limits.tasks.placed_holds), K::TasksPlacedHolds, U::Items)
            }
            L::ProjectPlacedHolds => Refusal::configured(
                which,
                u64::from(limits.tasks.project_placed_holds),
                K::TasksProjectPlacedHolds,
                U::Items,
            ),
            L::PartyPlacedHolds => Refusal::configured(
                which,
                u64::from(limits.tasks.party_placed_holds),
                K::TasksPartyPlacedHolds,
                U::Items,
            ),
            L::Exemptions => {
                Refusal::configured(which, u64::from(limits.tasks.exemptions), K::TasksExemptions, U::Items)
            }
            L::Name => Refusal::fixed(
                which,
                u64::try_from(jig_domain_people::NAME_BYTES).expect("name ceiling fits"),
                U::Bytes,
            ),
            L::Key => Refusal::fixed(which, u64::from(self.settings.key_bytes), U::Bytes),
            L::Grace => {
                Refusal::configured(which, limits.tasks.grace.as_nanos() / 1_000_000, K::TasksGrace, U::Milliseconds)
            }
            L::Roles
            | L::Arithmetic
            | L::Duplicate
            | L::Empty
            | L::Subscription
            | L::Dependencies
            | L::Cycle
            | L::Spec
            | L::Contract
            | L::Authority
            | L::Inputs
            | L::Attempts
            | L::LiveDelegates
            | L::Restore
            | L::Read
            | L::Turn
            | L::HoldKind
            | L::HoldTaken
            | L::Holds
            | L::InboxFull
            | L::Notes
            | L::Revision
            | L::Question
            | L::Priority
            | L::Policy
            | L::Resource
            | L::Service => Refusal::Limit { which, bound: None },
        }
    }
}
