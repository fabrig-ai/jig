//! Checked retained and in-flight heap bounds for the core
//! (`domain/engine.md`, sections 5.6 and 10; `domain/root.md`, section 10).
//! Child bounds cover their own state. This module prices the core's route
//! tables, owned payload copies, store loads and one complete decision.

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
use skein_lib::{Id, List, Map, Queue, Set, Slab};

use crate::{CallKey, Event, HistoricalResult, Limits, Request};

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

/// The configured models' settings: each name, the efforts it declares and the effort it
/// carries (configuration.md, 3).
fn models_worst_case() -> Option<u64> {
    let efforts = bytes(crate::EFFORTS, sizeof(size_of::<Box<[u8]>>())?.checked_add(u64::from(crate::EFFORT_BYTES))?)?;
    bytes(
        tasks::MODELS,
        sizeof(size_of::<crate::Model>())?
            .checked_add(u64::from(crate::MODEL_NAME_BYTES))?
            .checked_add(u64::from(accounts::NAME))?
            .checked_add(efforts)?
            .checked_add(u64::from(crate::EFFORT_BYTES))?,
    )
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

/// Pure checked heap ceiling for the core's eight children, its retained
/// route state, loads and the largest synchronous decision. Invalid child
/// limits or unrepresentable arithmetic return `None`; a root adds its own
/// journal and connectors when pricing the whole application.
#[must_use]
#[expect(clippy::too_many_lines, reason = "one checked sum names each retained and in-flight owner")]
pub fn worst_case(limits: &Limits) -> Option<u64> {
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
    let pending = bytes(
        limits.tasks.tasks.checked_mul(limits.tasks.batch)?,
        task_carrier(limits)?.checked_mul(2)?.checked_add(u64::from(limits.tasks.message_bytes).checked_mul(2)?)?,
    )?;
    let route = crate::routing::room_max(limits)?;
    let children = [
        tasks::worst_case(&crate::settings::task_limits(limits))?,
        authority::worst_case(&limits.authority)?,
        people::worst_case(&limits.people)?,
        fleet::worst_case(&crate::settings::fleet_limits(limits))?,
        brief::gather_worst_case(&limits.brief)?,
        accounts::worst_case(&limits.accounts)?,
        notes::worst_case(&limits.notes)?,
        views::worst_case(&limits.views)?,
    ];
    let roles =
        Map::<crate::resources::Key, crate::connector::ResourceRole>::worst_case(limits.tasks.resource_reports)?
            .checked_add(bytes(
                limits.tasks.resource_reports,
                bytes(limits.tasks.hold_segments, sizeof(size_of::<Box<[u8]>>())?)?
                    .checked_add(u64::from(limits.tasks.hold_bytes))?,
            )?)?;
    let mut total = crate::inbox::worst_case(limits)?
        .checked_add(crate::inbox::census_worst_case(limits)?)?
        .checked_add(Map::<skein_lib::Token, crate::ended_marks::Loaded>::worst_case(limits.people.pending)?)?
        .checked_add(crate::effect_worst_case(limits)?)?
        .checked_add(crate::conversation_worst_case(limits)?)?
        // Last reported settings holder count, including alignment room.
        .checked_add(u64::try_from(size_of::<u64>()).ok()?)?
        .checked_add(crate::reads::worst_case(limits)?)?
        .checked_add(crate::documents::document_with_context_worst_case(limits)?)?
        .checked_add(crate::documents::document_scratch_worst_case(limits)?)?
        .checked_add(crate::acts::scratch_worst_case(limits)?)?
        // The early party-batch circle projection retains parents, a walk and its ordered names.
        .checked_add(List::<Option<u32>>::worst_case(limits.people.batch)?)?
        .checked_add(List::<u32>::worst_case(limits.people.batch)?.checked_mul(2)?)?
        .checked_add(List::<u64>::worst_case(limits.tasks.circle)?)?
        .checked_add(List::<people::ProjectRole>::worst_case(limits.people.projects)?)?
        .checked_add(List::<tasks::ViewTask>::worst_case(limits.tasks.tasks.checked_mul(2)?)?)?
        .checked_add(Map::<(u32, u64), u64>::worst_case(limits.tasks.tasks)?)?
        .checked_add(Map::<(u64, u64), u64>::worst_case(limits.tasks.tasks)?)?
        .checked_add(Map::<u32, Box<str>>::worst_case(limits.authority.projects)?)?
        .checked_add(bytes(limits.authority.projects, u64::try_from(people::NAME_BYTES).ok()?)?)?
        .checked_add(Map::<u32, Box<[people::TemplateOverride]>>::worst_case(limits.authority.projects)?)?
        .checked_add(bytes(
            limits.authority.projects.checked_add(1)?,
            List::<people::TemplateOverride>::worst_case(limits.templates)?,
        )?)?
        .checked_add(bytes(
            limits.views.watchers.checked_add(3)?,
            List::<people::TemplateOverride>::worst_case(limits.templates)?,
        )?)?
        .checked_add(models_worst_case()?)?
        .checked_add(pending)?
        .checked_add(roles)?;
    for child in children {
        total = total.checked_add(child)?;
    }
    // Every inline map value is at most an Event, and every key at most a
    // CallKey. Boxed values are charged separately below.
    let largest = size_of::<Event>();
    for value in [
        size_of::<crate::GoalRoute>(),
        size_of::<crate::routing::application_routes::Flight>(),
        size_of::<crate::Carried>(),
        size_of::<crate::PersonTaskRoute>(),
        size_of::<crate::PersonProposalRoute>(),
        size_of::<crate::RoutedCall>(),
        size_of::<crate::routing::NoteRoute>(),
        size_of::<crate::CallPart>(),
        size_of::<crate::routing::Creation>(),
        size_of::<crate::routing::party_inputs::Inputs>(),
        size_of::<crate::routing::DelegateCreation>(),
        size_of::<crate::routing::ProcedureCreation>(),
        size_of::<crate::run::Transcript>(),
        size_of::<crate::RunProof>(),
    ] {
        if value > largest {
            return None;
        }
    }
    for capacity in [
        limits.tasks.tasks,        // dependency results
        limits.people.pending,     // made
        limits.people.pending,     // application continuations
        limits.people.pending,     // LLM key flights
        limits.people.pending,     // goal routes
        limits.fleet.calls,        // delegation
        limits.people.pending,     // proposal routes
        limits.tasks.tasks,        // ending positions
        limits.people.pending,     // messages
        limits.people.pending,     // moves
        limits.people.pending,     // person tasks
        limits.people.pending,     // person escalations
        limits.people.pending,     // pending input loads
        limits.people.pending,     // creations
        limits.call_records,       // delegate creations
        limits.tasks.tasks,        // procedure creations
        limits.tasks.tasks,        // batches awaiting named resource reports
        limits.authority.projects, // permission roles
        limits.call_records,       // pending calls
        limits.call_records,       // answered calls
        limits.fleet.calls,        // routed calls
        limits.brief.briefs,       // briefs awaiting notes
        limits.tasks.tasks,        // contexts
        limits.tasks.tasks,        // workspaces
        limits.tasks.tasks,        // transcripts
        limits.tasks.tasks,        // proofs
        limits.tasks.tasks,        // restoring proofs
        limits.tasks.tasks,        // unreported restored claims
        limits.tasks.tasks,        // claims
    ] {
        total = total.checked_add(Map::<CallKey, Event>::worst_case(capacity)?)?;
    }
    total = total
        .checked_add(Map::<skein_lib::Token, crate::effects::ProcedureProposal>::worst_case(limits.tasks.tasks)?)?;
    total = total.checked_add(Map::<skein_lib::Token, crate::WatchBinding>::worst_case(limits.views.watchers)?)?;
    total = total.checked_add(Slab::<crate::WatchedSubject>::worst_case(limits.views.watchers)?)?;
    total = total.checked_add(Set::<Id<crate::WatchedSubject>>::worst_case(limits.views.watchers)?.checked_mul(2)?)?;
    total = total.checked_add(List::<(skein_lib::Token, jig_domain_views::End)>::worst_case(limits.views.watchers)?)?;
    total = total.checked_add(List::<u64>::worst_case(limits.views.watchers)?)?;
    // Views prices its token-only retained_runs snapshot. Finishing replaces
    // that scratch with reconciliation actions; the snapshots never coexist.
    total =
        total.checked_add(
            List::<crate::routing::watch_marks::Reconciliation>::worst_case(limits.views.runs)?
                .checked_sub(List::<skein_lib::Token>::worst_case(limits.views.runs)?)?,
        )?;
    total = total.checked_add(List::<u64>::worst_case(limits.tasks.depth.checked_add(1)?)?)?;
    total = total.checked_add(List::<u64>::worst_case(limits.documents.chips)?)?;
    total = total
        .checked_add(u64::from(limits.views.watchers).checked_mul(List::<u64>::worst_case(limits.documents.chips)?)?)?;
    total = total.checked_add(Map::<u64, Box<crate::Projection>>::worst_case(limits.tasks.tasks)?)?;
    total = total.checked_add(bytes(
        limits.tasks.tasks.checked_mul(limits.connectors.checked_add(2)?)?,
        crate::projection_bytes(limits)?,
    )?)?;
    total = total.checked_add(List::<u32>::worst_case(limits.authority.projects)?)?;
    total = total.checked_add(Map::<skein_lib::Token, crate::routing::NoteRoute>::worst_case(2)?)?;
    total = total.checked_add(Map::<u64, bool>::worst_case(limits.brief.briefs)?)?;
    total = total.checked_add(Map::<u64, crate::PendingRelay>::worst_case(limits.tasks.tasks)?)?;
    total = total.checked_add(bytes(limits.connectors, sizeof(size_of::<u16>())?)?)?;
    total =
        total.checked_add(Map::<(u64, u16), ()>::worst_case(limits.tasks.tasks.checked_mul(limits.connectors)?)?)?;
    total = total.checked_add(Queue::<Box<tasks::RunContext>>::worst_case(limits.tasks.tasks)?)?;
    total = total.checked_add(Queue::<fleet::Event>::worst_case(limits.tasks.tasks.checked_mul(2)?)?)?;
    total = total.checked_add(Map::<u64, Box<[u32]>>::worst_case(limits.tasks.tasks)?)?;
    total = total.checked_add(bytes(
        limits.tasks.tasks.checked_mul(3)?,
        bytes(limits.accounts.credentials, sizeof(size_of::<u32>())?)?,
    )?)?;
    // Live and replacement proofs, persisted copies, turn evidence and rebuilding scratch.
    total = total.checked_add(bytes(
        limits.tasks.tasks.checked_mul(4)?.checked_add(2)?,
        List::<u64>::worst_case(limits.tasks.run_inbox)?,
    )?)?;
    let grants = bytes(limits.fleet.grants, sizeof(size_of::<fleet::Grant>())?)?
        .checked_add(bytes(limits.fleet.grants, sizeof(size_of::<fleet::GrantName>())?)?)?;
    total = total.checked_add(bytes(limits.tasks.tasks.checked_mul(2)?, grants)?)?;

    total = total.checked_add(bytes(
        limits.tasks.tasks.checked_mul(4)?,
        bytes(
            tasks::MODELS,
            sizeof(size_of::<authority::AccountBinding>())?.checked_add(sizeof(size_of::<Option<u32>>())?)?,
        )?,
    )?)?;
    // Each prepared assignment owns the complete run header as well as its policy bytes.
    let run = sizeof(size_of::<crate::RunCharter>())?.checked_add(u64::from(limits.run_bytes))?;
    total = total.checked_add(bytes(limits.tasks.tasks, run)?)?;
    total = total
        .checked_add(List::<u32>::worst_case(limits.accounts.accounts)?)?
        .checked_add(List::<skein_lib::Token>::worst_case(limits.fleet.workers.checked_add(1)?)?)?
        .checked_add(bytes(
            tasks::MODELS.checked_mul(4)?,
            sizeof(size_of::<fleet::ModelNeed>())?.checked_add(u64::from(crate::MODEL_NAME_BYTES))?,
        )?)?;
    // One run check's transient people/account coverage proofs; selection itself allocates nothing.
    total = total.checked_add(List::<accounts::Coverage>::worst_case(limits.accounts.accounts)?)?.checked_add(
        List::<accounts::HostSource>::worst_case(limits.accounts.accounts.checked_mul(limits.accounts.sources)?)?,
    )?;
    let task = &limits.tasks;
    let new_task = task_carrier(limits)?;
    let context = context_carrier(limits)?;
    let field_size = sizeof(size_of::<tasks::ResultField>())?;
    let result_payload = u64::from(task.result_bytes).checked_mul(field_size.checked_add(1)?)?;
    let historical = sizeof(size_of::<HistoricalResult>())?.checked_add(result_payload)?;
    let histories = task.dependencies.checked_add(task.inputs)?;
    total = total.checked_add(bytes(task.tasks, bytes(histories, historical)?)?)?;
    total = total
        .checked_add(Map::<skein_lib::Token, crate::routing::result_reads::ResultRead>::worst_case(task.tasks)?)?;
    total = total.checked_add(bytes(
        task.tasks,
        bytes(histories, sizeof(size_of::<u64>())?)?
            .checked_add(List::<HistoricalResult>::worst_case(histories)?)?
            .checked_add(bytes(histories, result_payload)?)?,
    )?)?;
    // Each amendment retains only places, attempted IDs and its original replay bytes.
    let places = task.dependencies.checked_mul(task.depth.checked_add(1)?)?;
    let amendment_read = sizeof(size_of::<crate::routing::amendment_reads::AmendmentRead>())?
        .checked_add(List::<tasks::DependencyPlace>::worst_case(places)?)?
        .checked_add(List::<u64>::worst_case(places)?)?
        .checked_add(sizeof(size_of::<u64>())?)?
        .checked_add(
            new_task
                .checked_add(u64::from(task.spec_bytes))?
                .checked_add(u64::from(task.message_bytes))?
                .max(u64::from(limits.people.amendment_bytes)),
        )?;
    total = total.checked_add(bytes(task.tasks, amendment_read)?)?;
    total = total.checked_add(bytes(task.tasks.checked_mul(2)?, context)?)?;
    let creations = limits
        .people
        .pending
        .checked_mul(limits.people.batch.max(1))?
        .checked_mul(2)?
        .checked_add(limits.call_records.checked_mul(task.batch)?)?
        .checked_add(task.tasks.checked_mul(task.batch)?)?;
    total = total.checked_add(bytes(creations, new_task)?)?;
    // Each holdings flight also retains the step's one pending proposal.
    total = total.checked_add(bytes(
        task.tasks,
        sizeof(size_of::<tasks::Proposal>())?
            .checked_add(bytes(task.batch, new_task)?)?
            .checked_add(u64::from(task.message_bytes))?,
    )?)?;
    total = total.checked_add(bytes(
        limits.people.pending,
        new_task
            .checked_add(bytes(
                limits.people.batch.checked_mul(task.inputs)?,
                sizeof(size_of::<u64>())?.checked_add(sizeof(size_of::<tasks::Stub>())?)?,
            )?)?
            .checked_mul(2)?,
    )?)?;
    total = total.checked_add(bytes(limits.people.pending.checked_mul(2)?, u64::from(limits.people.words))?)?;
    total = total.checked_add(new_task)?;
    total = total.checked_add(bytes(
        limits.fleet.calls.checked_add(limits.load_slots)?,
        bytes(task.batch.checked_mul(task.inputs)?, sizeof(size_of::<tasks::Stub>())?)?,
    )?)?;
    total = total.checked_add(bytes(
        task.tasks,
        Queue::<Box<[u8]>>::worst_case(limits.resume_bytes)?
            .checked_add(u64::from(limits.resume_bytes))?
            .checked_add(Queue::<crate::transcript_text::TranscriptText>::worst_case(limits.brief_core_budgets.tail)?)?
            .checked_add(u64::from(limits.brief_core_budgets.tail))?,
    )?)?;
    total = total.checked_add(bytes(limits.authority.projects, limits.policy_bytes)?)?;
    // First-start seeds coexist with the configured policy table until people restore. Each held
    // policy is checked against `policy_bytes` as its row would be (configured at the start, asserted
    // at construction), so it costs its slot, its name and that bound, as the rows do.
    total = total.checked_add(List::<crate::SeedProject>::worst_case(limits.authority.projects)?)?;
    total = total.checked_add(bytes(
        limits.authority.projects,
        limits.policy_bytes.checked_add(u64::try_from(people::NAME_BYTES).ok()?)?,
    )?)?;
    total = total.checked_add(bytes(
        limits.people.initial_owners,
        sizeof(size_of::<people::InitialOwner>())?.checked_add(u64::from(limits.people.identity_bytes))?,
    )?)?;
    total = total.checked_add(Map::<people::IdentityKey, u64>::worst_case(limits.people.initial_owners)?)?;
    total = total.checked_add(bytes(limits.people.initial_owners, u64::from(limits.people.identity_bytes))?)?;
    total = total.checked_add(limits.policy_bytes.checked_mul(2)?)?;
    total = total.checked_add(crate::settings::worst_case(limits)?)?;
    total = total.checked_add(Map::<(people::Proposer, u32), crate::settings::Proposal>::worst_case(
        limits.settings.proposals,
    )?)?;
    total = total.checked_add(
        u64::from(limits.settings.proposals).checked_mul(
            crate::settings::set_worst_case(limits)?
                .checked_add(List::<crate::settings::Item>::worst_case(tasks::SETTINGS_ITEMS)?)?,
        )?,
    )?;
    // One held entrance, including its reason, and one offering awaiting tasks.
    total = total.checked_add(crate::settings::revision_worst_case(limits)?.checked_mul(2)?)?;
    total = total.checked_add(u64::from(limits.people.words))?;
    total = total.checked_add(List::<(people::Proposer, u32)>::worst_case(limits.settings.proposals)?)?;
    total = total.checked_add(u64::try_from(size_of::<crate::settings::Proposed>()).ok()?)?;
    // A decoded held revision is transient. At a release, the copied defaults
    // and the new record's items coexist until they pass into the journal.
    total = total.checked_add(crate::settings::revision_worst_case(limits)?.checked_mul(2)?)?;
    total = total.checked_add(bytes(
        limits.call_records,
        bytes(task.batch, sizeof(size_of::<u64>())?)?
            .checked_add(bytes(authority::max_out(&limits.authority)?, sizeof(size_of::<authority::Finding>())?)?)?,
    )?)?;
    let note_pattern =
        u64::from(limits.notes.pattern_bytes).checked_mul(sizeof(size_of::<Box<[u8]>>())?.checked_add(1)?)?;
    let note_entry = sizeof(size_of::<notes::Entry>())?
        .checked_add(note_pattern)?
        .checked_add(u64::from(limits.notes.description_bytes))?
        .checked_add(u64::from(limits.notes.body_bytes))?
        .checked_add(List::<u64>::worst_case(limits.notes.references)?)?;
    total = total.checked_add(bytes(limits.call_records, bytes(limits.notes.recalled, note_entry)?)?)?;
    total = total.checked_add(bytes(task.tasks, result_payload.checked_mul(3)?)?)?;
    total = total.checked_add(u64::from(task.message_bytes))?;
    // Documents move from this bounded construction map into held outputs.
    // The application journal separately prices outputs retained across decisions.
    total =
        total.checked_add(Map::<skein_lib::Token, crate::documents::Document>::worst_case(limits.views.watchers)?)?;
    total = total.checked_add(
        u64::from(limits.views.watchers).checked_mul(crate::documents::document_with_context_worst_case(limits)?)?,
    )?;

    // One decision may retain a routed event, a request, an owned row/load
    // copy and a connector handoff at each bounded route position.
    let payload = new_task
        .checked_mul(u64::from(task.batch.max(1)).checked_add(2)?)?
        .checked_add(bytes(task.batch, new_task)?)?
        .checked_add(sizeof(size_of::<tasks::Proposal>())?)?
        .checked_add(List::<people::Entry>::worst_case(crate::inbox::waiting_capacity(limits)?)?)?
        .checked_add(context)?
        .checked_add(List::<u64>::worst_case(task.run_inbox)?)?
        .checked_add(u64::from(task.message_bytes.max(task.result_bytes)).checked_mul(field_size.checked_add(1)?)?)?
        .checked_add(limits.policy_bytes)?
        .checked_add(u64::try_from(people::NAME_BYTES).ok()?)?
        .checked_add(run)?
        .checked_add(crate::charters::documents_worst_case(limits)?)?
        .checked_add(bytes(limits.tasks.charter_documents, sizeof(size_of::<skein_lib::Token>())?)?)?
        .checked_add(u64::from(task.message_bytes))?
        .checked_add(u64::from(limits.brief.brief_bytes))?
        .checked_add(u64::from(limits.resume_bytes))?
        .checked_add(u64::from(limits.brief_core_budgets.tail))?
        .checked_add(u64::from(limits.people.batch).checked_mul(
            u64::from(limits.people.amendment_bytes).checked_add(sizeof(size_of::<people::Member>())?)?,
        )?)?
        .checked_add(u64::from(limits.people.identity_bytes))?
        .checked_add(u64::from(limits.people.words))?
        .checked_add(u64::from(limits.notes.body_bytes))?
        .checked_add(u64::from(limits.documents.text_bytes))?;
    let source_payload = bytes(
        limits.fleet.program_sources,
        sizeof(size_of::<fleet::Login>())?
            .checked_add(sizeof(size_of::<fleet::SourceName>())?.checked_mul(3)?)?
            .checked_add(u64::from(limits.fleet.source_detail_bytes))?,
    )?;
    let program_payload = bytes(limits.accounts.program_hosts, sizeof(size_of::<accounts::ProgramHost>())?)?
        .max(bytes(limits.accounts.program_names, sizeof(size_of::<accounts::Program>())?)?);
    let payload = payload.max(source_payload).max(program_payload).max(people::outcome_bytes(&limits.people)?);
    total = total.checked_add(Queue::<fleet::Request>::worst_case(fleet::max_out(&limits.fleet).checked_mul(4)?)?)?;
    total = total.checked_add(Queue::<Event>::worst_case(route.writes)?)?;
    total = total.checked_add(Queue::<Request>::worst_case(route.writes.checked_add(1)?)?)?;
    total = total.checked_add(bytes(route.writes, payload)?)?;
    total = total.checked_add(bytes(limits.load_slots, payload)?)?;
    // One named client decision holds its routed Event and its emitted people::Request::Route.
    // People already prices the retained Pending clone in people::worst_case; core keeps none.
    total = total.checked_add(people::model_names_worst_case()?.checked_mul(2)?)?;
    Some(total)
}
