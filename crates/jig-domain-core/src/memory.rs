//! Limit validity and per-value policy and settings carriers for the core
//! (`domain/engine.md`, sections 5.6 and 10; programming-model.md, 6.3).

use alloc::boxed::Box;
use core::mem::size_of;
use jig_domain_accounts as accounts;
use jig_domain_authority as authority;
use jig_domain_brief as brief;
use jig_domain_fleet as fleet;
use jig_domain_notes as notes;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_domain_views as views;
use skein_lib::List;

use crate::Limits;

// A held authority policy is priced by its people snapshot's `policy_bytes`, which counts the
// people mirrors' sizes; each mirror is at least as large as the authority type it copies.
const _: () = assert!(
    size_of::<authority::Role>() <= size_of::<people::PolicyRole>(),
    "people's PolicyRole prices authority's Role"
);
const _: () = assert!(
    size_of::<authority::Authority>() <= size_of::<people::Authority>(),
    "people's Authority prices authority's Authority"
);
const _: () =
    assert!(size_of::<authority::Grant>() <= size_of::<people::Grant>(), "people's Grant prices authority's Grant");
const _: () = assert!(
    size_of::<authority::ResourceScope>() <= size_of::<people::ResourceScope>(),
    "people's ResourceScope prices authority's ResourceScope"
);
const _: () = assert!(
    size_of::<authority::Executor>() <= size_of::<people::Executor>(),
    "people's Executor prices authority's Executor"
);
const _: () = assert!(
    size_of::<authority::Requirement>() <= size_of::<people::Requirement>(),
    "people's Requirement prices authority's Requirement"
);

/// The bytes a run's policy from `policy`, a charter's, owns whatever its task's LLM choice: its
/// models, and its largest once more for a main model its sub-agents name again, each with its
/// longest declared effort chosen (domain/engine.md, 7.1).
#[must_use]
pub fn policy_owned_bytes(policy: &crate::RunPolicy) -> Option<u64> {
    let mut largest = model_owned_bytes(&policy.model)?;
    let mut bytes = sizeof(size_of::<crate::RunPolicy>())?
        .checked_add(u64::try_from(policy.instructions.len()).ok()?)?
        .checked_add(largest)?
        .checked_add(
            u64::try_from(policy.alternatives.len())
                .ok()?
                .checked_add(1)?
                .checked_mul(sizeof(size_of::<crate::Model>())?)?,
        )?;
    for model in &policy.alternatives {
        let owned = model_owned_bytes(model)?;
        bytes = bytes.checked_add(owned)?;
        largest = largest.max(owned);
    }
    bytes.checked_add(largest)
}

/// A model's name, its efforts' names and its chosen one's, at most its longest (tasks.md, 3).
fn model_owned_bytes(model: &crate::Model) -> Option<u64> {
    let mut longest = 0_u64;
    let mut bytes = u64::try_from(model.name.len())
        .ok()?
        .checked_add(u64::try_from(model.efforts.len()).ok()?.checked_mul(sizeof(size_of::<Box<[u8]>>())?)?)?;
    for effort in &model.efforts {
        let length = u64::try_from(effort.len()).ok()?;
        bytes = bytes.checked_add(length)?;
        longest = longest.max(length);
    }
    if let Some(effort) = &model.effort {
        longest = longest.max(u64::try_from(effort.len()).ok()?);
    }
    if let Some(program) = &model.program {
        bytes = bytes.checked_add(u64::try_from(program.len()).ok()?)?;
    }
    bytes.checked_add(longest)
}

fn bytes(count: u32, each: u64) -> Option<u64> {
    u64::from(count).checked_mul(each)
}

fn sizeof(value: usize) -> Option<u64> {
    u64::try_from(value).ok()
}

pub(crate) fn task_carrier(limits: &Limits) -> Option<u64> {
    let task = &limits.tasks;
    let names = bytes(
        task.holdings,
        sizeof(size_of::<tasks::Holding>())?
            .checked_add(bytes(task.hold_segments, sizeof(size_of::<Box<[u8]>>())?)?)?
            .checked_add(u64::from(task.hold_bytes))?,
    )?;
    let authority = bytes(
        task.authority_grants,
        sizeof(size_of::<tasks::Grant>())?
            .checked_add(bytes(task.authority_segments.checked_mul(2)?, sizeof(size_of::<Box<[u8]>>())?)?)?,
    )?
    .checked_add(u64::from(task.authority_bytes))?
    .checked_add(bytes(task.executor_kinds, sizeof(size_of::<tasks::AuthorityExecutor>())?)?)?
    .checked_add(bytes(task.authority_grants, sizeof(size_of::<tasks::ResourceScope>())?)?)?;
    sizeof(size_of::<tasks::New>())?
        .checked_add(u64::from(task.spec_bytes))?
        .checked_add(u64::from(task.message_bytes))?
        .checked_add(bytes(task.parameters, sizeof(size_of::<tasks::Parameter>())?)?)?
        .checked_add(bytes(task.inputs, sizeof(size_of::<u64>())?)?)?
        .checked_add(bytes(task.dependencies, sizeof(size_of::<u64>())?)?)?
        .checked_add(bytes(task.contract_choices, sizeof(size_of::<tasks::Verdict>())?)?)?
        .checked_add(bytes(
            task.contract_choices,
            u64::from(task.result_bytes).checked_mul(sizeof(size_of::<tasks::FieldRule>())?.checked_add(1)?)?,
        )?)?
        .checked_add(u64::from(task.result_bytes))?
        .checked_add(List::<Box<[u8]>>::worst_case(task.hold_segments)?)?
        .checked_add(u64::from(task.hold_bytes))?
        .checked_add(names)?
        .checked_add(authority)?
        .checked_add(bytes(tasks::MODELS, sizeof(size_of::<tasks::ModelEffort>())?)?)
}

fn context_carrier(limits: &Limits) -> Option<u64> {
    let task = &limits.tasks;
    task_carrier(limits)?
        .checked_add(sizeof(size_of::<tasks::RunContext>())?)?
        .checked_add(bytes(task.dependencies, sizeof(size_of::<tasks::Added>())?.checked_add(8)?)?)?
        .checked_add(bytes(task.inbox_messages, sizeof(size_of::<tasks::Word>())?)?)?
        .checked_add(u64::from(task.inbox_bytes))?
        .checked_add(u64::from(task.inbox_bytes).checked_mul(sizeof(size_of::<tasks::ResultField>())?)?)?
        .checked_add(bytes(task.delegates, sizeof(size_of::<tasks::DelegateState>())?)?)?
        .checked_add(bytes(task.saved_resources, sizeof(size_of::<tasks::SavedResource>())?)?)?
        .checked_add(u64::from(task.result_bytes))
}

pub use fleet::WorkerProblem as Problem;
pub(crate) fn problems(names: Box<[Box<[u8]>]>, limits: &Limits) -> Result<fleet::Membership, Problem> {
    fleet::Membership::new(names, limits.fleet.configured_workers, limits.fleet.worker_name_bytes)
}

/// Whether the core's limits and edge bounds are representable (programming-model.md, 6.3).
#[must_use]
pub fn valid(limits: &Limits) -> bool {
    checked_valid(limits).is_some()
}

fn checked_valid(limits: &Limits) -> Option<()> {
    if limits.tasks.circle > people::NAMED_TASKS {
        return None;
    }
    if limits.people.program_hosts < limits.fleet.configured_workers.checked_add(1)?
        || limits.people.program_host_name_bytes < limits.fleet.worker_name_bytes
        || limits.accounts.program_hosts < limits.fleet.configured_workers.checked_add(1)?
        || limits.people.operations > limits.tasks.tasks
        || limits.accounts.program_names > limits.fleet.program_sources
        || limits.settings.revisions == 0
        || limits.settings.versions == 0
        || limits.settings.history == 0
        || limits.settings.proposals == 0
        || limits.templates == 0
        || limits.resume_bytes == 0
        || limits.run_bytes == 0
        || limits.call_records == 0
        || limits.call_answer_bytes == 0
        || limits.tasks.inbox_messages < limits.people.inbox_entries
        || limits.people.inbox_tasks < limits.tasks.tasks.checked_mul(2)?
        || limits.people.inbox_recipients < crate::inbox::waiting_capacity(limits)?
        || limits.fleet.grants < limits.accounts.credentials.checked_add(limits.accounts.accounts.min(tasks::MODELS))?
        || limits.authority.accounts < limits.accounts.credentials.max(limits.accounts.accounts.min(tasks::MODELS))
        || limits.notes.scopes < 3_u32.checked_add(limits.authority.grants)?
    {
        return None;
    }
    if !tasks::valid(&crate::settings::task_limits(limits))
        || !people::valid(&limits.people)
        || !fleet::valid(&crate::settings::fleet_limits(limits))
        || !brief::valid(&limits.brief)
        || !accounts::valid(&limits.accounts)
        || !notes::valid(&limits.notes)
        || !views::valid(&limits.views)
        || limits.load_bytes == 0
        || limits.load_slots == 0
    {
        return None;
    }
    // Constructor counts formerly reached through the inbox and settings subtotals.
    crate::inbox::question_capacity(limits)?;
    limits.people.people.checked_mul(limits.people.inbox_entries)?.checked_add(1)?;
    limits.settings.versions.checked_add(crate::settings::capacity(limits)?.checked_mul(4)?)?;
    limits.tasks.charters.checked_mul(2)?.checked_add(limits.settings.versions)?;
    crate::routing::route_bound(limits)?;
    crate::documents::document_with_context_worst_case(limits)?;
    crate::settings::set_worst_case(limits)?;
    crate::settings::revision_worst_case(limits)?;
    crate::reads::query_worst_case(limits)?;
    crate::reads::answer_worst_case(limits)?;
    task_carrier(limits)?;
    context_carrier(limits)?;
    Some(())
}
