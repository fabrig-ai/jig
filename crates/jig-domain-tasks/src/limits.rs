//! Root-configured admission and memory bounds (domain/tasks.md, section 10). Counts retained live state and bounded scratch; parents count
//! their owned input/output copies separately. This module performs no admission
//! mutation, authority decision or allocation during the bound calculation.
use crate::domain::Task;
use crate::{AuthorityExecutor, Class, Fact, Grant, Parameter, Retries, Verdict};
use alloc::boxed::Box;
use skein_lib::{Deadlines, Id, List, Map, Queue, Slab};

/// Immutable root-configured limits for live task state, finite sources, owned semantic payloads
/// and retry policies; validate through `worst_case` before construction. (domain/tasks.md, section 10).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Limits {
    /// Placed holds in the deployment, including lifted holds kept for their tasks.
    pub placed_holds: u32,
    /// Placed holds retained in one project.
    pub project_placed_holds: u32,
    /// Placed holds retained for one party owner.
    pub party_placed_holds: u32,
    /// Tasks one standing placed hold may exempt; at most tasks.
    pub exemptions: u32,
    /// Maximum retained live task slots, name-index entries and retry alarms.
    pub tasks: u32,
    /// Maximum retained finite period, pool and recurring allotment identities.
    pub funders: u32,
    /// Maximum live tasks per project, including every member of an admitted batch.
    pub project_tasks: u32,
    /// Maximum live tracked tasks per project.
    pub project_goals: u32,
    /// Maximum lifetime tasks made in a requester tree, including its root; completed delegates do
    /// not return this capacity.
    pub tree_tasks: u32,
    /// Maximum structural depth below the requester-tree root, which has depth zero.
    pub depth: u32,
    /// Maximum live direct delegates of one requester task.
    pub delegates: u32,
    /// Maximum introduced peer references per live task.
    pub references: u32,
    /// Maximum standing interests per task.
    pub subscriptions: u32,
    /// Maximum directly created tasks in one atomic nonempty `Make` batch.
    pub batch: u32,
    /// Maximum immutable dependency identities per task; also bounds its remaining live
    /// `waiting_on` subset.
    pub dependencies: u32,
    /// Maximum resources required by one task.
    pub holdings: u32,
    /// Maximum connector resource kinds installed in this deployment.
    pub hold_kinds: u32,
    /// Maximum cached named connector admission reports.
    pub resource_reports: u32,
    /// Maximum distinct pools whose latest connector slot counts are retained.
    pub pools: u32,
    /// Maximum literal segments in one resource name.
    pub hold_segments: u32,
    /// Maximum aggregate literal bytes in one resource name.
    pub hold_bytes: u32,
    /// Maximum tasks waiting on any one resource.
    pub hold_waiters: u32,
    /// Longest span a cancel may give its task to finish.
    pub grace: skein_lib::Duration,
    /// Wall-clock bound before a task waiting for holds is held for review.
    pub hold_wait: skein_lib::Duration,
    /// Maximum tasks a circle of waits names in a refusal or a hold (domain/tasks.md, 6.7).
    pub circle: u32,
    /// Shape bound for historical input identities checked by the root before delegation.
    pub inputs: u32,
    /// Maximum combined specification words and byte-valued parameter bytes per task.
    pub spec_bytes: u32,
    /// Maximum parameters per specification.
    pub parameters: u32,
    /// Maximum result/reason bytes; contracts cannot allow larger results and cancellation may
    /// retain one bounded reason plus one bounded partial result.
    pub result_bytes: u32,
    /// Messages a run's inbox takes: the most its start carries, and the most relayed
    /// and unread (tasks.md, 8.2). At most `inbox_messages`.
    pub run_inbox: u32,
    /// Maximum whole unread word messages kept by one live task.
    pub inbox_messages: u32,
    /// Maximum total unread word bytes kept by one live task.
    pub inbox_bytes: u32,
    /// Maximum bytes in one admitted word message.
    pub message_bytes: u32,
    /// Time an active person task waits before becoming news for its requester.
    pub person_stall: skein_lib::Duration,
    /// Time an undecided proposal waits at a nonfinal holder before passing upward.
    pub proposal_stall: skein_lib::Duration,
    /// Time an undecided escalation waits at a nonfinal holder before passing upward.
    pub escalation_stall: skein_lib::Duration,
    /// Maximum connector resources on which a task has saved state.
    pub saved_resources: u32,
    /// Maximum distinct-code choices in a nonempty verdict contract.
    pub contract_choices: u32,
    /// Maximum retained configured agent-charter numbers.
    pub charters: u32,
    /// Charter versions held besides the newest view.
    pub versions: u32,
    /// Settings revisions held besides the newest.
    pub revisions: u32,
    /// Maximum resource grants per task authority carrier.
    pub authority_grants: u32,
    /// Maximum base segments per carried grant pattern.
    pub authority_segments: u32,
    /// Maximum total bytes across every grant's base and terminal segments in one authority
    /// carrier.
    pub authority_bytes: u32,
    /// Maximum carried authority executor-permission entries.
    pub executor_kinds: u32,
    /// Validated per-class failure retry and pause policies.
    pub retries: Retries,
    /// Optional content-free observation capacity; overflow is diagnostic and changes no decision.
    pub facts: u32,
    /// Recent owner-proven room refusals; zero omits history and counts omissions.
    pub room_refusals: u32,
}

/// Maximum ended identities named by bounded live dependencies, inputs or references.
#[must_use]
pub(crate) fn stub_capacity(limits: &Limits) -> Option<u32> {
    limits.tasks.checked_mul(
        limits
            .dependencies
            .checked_add(limits.inputs)?
            .checked_add(limits.references)?
            .checked_add(limits.batch.checked_mul(limits.dependencies.checked_add(limits.inputs)?)?)?,
    )
}

/// Pure checked heap bound for live containers and nested payloads, finite ledgers, charter
/// configuration, optional facts and bounded graph/traversal scratch under `limits`. Returns `None`
/// on overflow or invalid task/batch/tree/retry settings. Validates output-bound arithmetic; caller
/// separately counts incoming data, `RunContext`/saved-row copies and `Request` queues.
#[must_use]
#[expect(clippy::too_many_lines, reason = "one checked bound includes every retained task and hold container")]
pub fn worst_case(limits: &Limits) -> Option<u64> {
    crate::domain::output_bound(limits)?;
    crate::activity::line_bytes_limit(limits)?;
    if limits.placed_holds == 0
        || limits.project_placed_holds == 0
        || limits.party_placed_holds == 0
        || limits.exemptions == 0
        || limits.exemptions > limits.tasks
        || limits.circle == 0
        || limits.tasks == 0
        || limits.batch == 0
        || limits.tree_tasks == 0
        || limits.run_inbox == 0
        || limits.run_inbox > limits.inbox_messages
        || limits.inbox_messages == 0
        || limits.inbox_bytes == 0
        || limits.message_bytes == 0
        || limits.message_bytes > limits.inbox_bytes
        || limits.grace == skein_lib::Duration::ZERO
        || limits.hold_wait == skein_lib::Duration::ZERO
        || limits.person_stall == skein_lib::Duration::ZERO
        || limits.proposal_stall == skein_lib::Duration::ZERO
        || limits.escalation_stall == skein_lib::Duration::ZERO
    {
        return None;
    }
    for class in [Class::Transient, Class::Permanent, Class::Run, Class::Agent, Class::Lost, Class::Invalid] {
        let retry = limits.retries.of(class);
        if retry.retries == u32::MAX || retry.base == skein_lib::Duration::ZERO || retry.max < retry.base {
            return None;
        }
    }
    let segments = u64::from(limits.authority_grants)
        .checked_mul(2)?
        .checked_mul(u64::from(limits.authority_segments))?
        .checked_mul(u64::try_from(size_of::<Box<[u8]>>()).ok()?)?;
    // Every admitted field has a nonempty name, so its array length is at
    // most its semantic byte cap. Arrays are priced separately from bytes.
    let field_size = u64::try_from(size_of::<crate::ResultField>()).ok()?;
    let result_fields = u64::from(limits.result_bytes).checked_mul(field_size)?;
    let contract_fields = u64::from(limits.contract_choices).checked_mul(
        u64::from(limits.result_bytes)
            .checked_mul(u64::try_from(size_of::<crate::FieldRule>()).ok()?.checked_add(1)?)?,
    )?;
    let approval_contract = u64::from(limits.result_bytes)
        .checked_add(u64::from(limits.hold_bytes))?
        .checked_add(List::<Box<[u8]>>::worst_case(limits.hold_segments)?)?;
    // An LLM choice's sub-agents, at most one per configured model.
    let choice = u64::from(crate::MODELS).checked_mul(u64::try_from(size_of::<crate::ModelEffort>()).ok()?)?;
    let payload = u64::from(limits.result_bytes)
        .checked_mul(2)?
        .checked_add(choice)?
        .checked_add(u64::try_from(size_of::<crate::Word>()).ok()?)?
        .checked_add(u64::from(limits.inbox_messages).checked_mul(u64::try_from(size_of::<u64>()).ok()?)?)?
        .checked_add(u64::from(limits.spec_bytes))?
        .checked_add(u64::from(limits.message_bytes))?
        .checked_add(u64::from(limits.inbox_bytes))?
        .checked_add(u64::from(limits.inbox_bytes).checked_mul(field_size)?)?
        .checked_add(approval_contract)?
        .checked_add(contract_fields)?
        .checked_add(u64::from(limits.message_bytes).checked_mul(2)?)?
        .checked_add(u64::from(limits.message_bytes).checked_mul(field_size)?.checked_mul(3)?)?
        .checked_add(u64::try_from(size_of::<crate::Word>()).ok()?)?
        // The merged notice that a kept task goes on (domain/tasks.md, 5.7).
        .checked_add(u64::try_from(size_of::<crate::Word>()).ok()?)?
        .checked_add(u64::from(limits.inbox_messages).checked_mul(u64::try_from(size_of::<crate::Word>()).ok()?)?)?
        .checked_add(u64::from(limits.inbox_messages).checked_mul(u64::try_from(size_of::<crate::ReplyHint>()).ok()?)?)?
        .checked_add(
            u64::from(limits.inbox_messages).checked_mul(u64::try_from(size_of::<crate::QuestionLine>()).ok()?)?,
        )?
        .checked_add(
            u64::from(limits.inbox_messages).checked_mul(u64::try_from(size_of::<crate::ProposalRow>()).ok()?)?,
        )?
        // Each outgoing credit keeps typed parties and its original activity
        // position. Words remain in history and add no retained credit payload.
        .checked_add(
            u64::from(limits.inbox_messages).checked_mul(u64::try_from(size_of::<crate::QuestionCredit>()).ok()?)?,
        )?
        .checked_add(
            u64::from(limits.saved_resources).checked_mul(
                u64::try_from(size_of::<crate::SavedResource>())
                    .ok()?
                    .checked_add(
                        u64::from(limits.authority_segments)
                            .checked_mul(u64::try_from(size_of::<Box<[u8]>>()).ok()?)?,
                    )?
                    .checked_add(u64::from(limits.authority_bytes))?,
            )?,
        )?
        .checked_add(u64::from(limits.result_bytes).checked_add(result_fields)?.checked_mul(3)?)?
        .checked_add(
            // Live phase, observed phase, and bounded transition scratch.
            3_u64.checked_mul(
                u64::try_from(size_of::<crate::HoldSource>())
                    .ok()?
                    .checked_add(
                        u64::from(limits.hold_segments).checked_mul(u64::try_from(size_of::<Box<[u8]>>()).ok()?)?,
                    )?
                    .checked_add(u64::from(limits.hold_bytes))?
                    .checked_add(u64::from(limits.circle).checked_mul(8)?)?,
            )?,
        )?
        .checked_add(u64::from(limits.parameters).checked_mul(u64::try_from(size_of::<Parameter>()).ok()?)?)?
        .checked_add(u64::from(limits.inputs).checked_mul(8)?)?
        .checked_add(u64::from(limits.dependencies).checked_mul(16)?)?
        .checked_add(u64::from(limits.dependencies).checked_mul(u64::try_from(size_of::<crate::Added>()).ok()?)?)?
        .checked_add(
            u64::from(limits.holdings).checked_mul(
                u64::try_from(size_of::<crate::Holding>())
                    .ok()?
                    .checked_add(
                        u64::from(limits.hold_segments).checked_mul(u64::try_from(size_of::<Box<[u8]>>()).ok()?)?,
                    )?
                    .checked_add(u64::from(limits.hold_bytes))?,
            )?,
        )?
        .checked_add(u64::from(limits.delegates).checked_mul(8)?)?
        .checked_add(u64::from(limits.batch).checked_mul(8)?)?
        .checked_add(u64::from(limits.references).checked_mul(8)?)?
        .checked_add(
            u64::from(limits.subscriptions).checked_mul(u64::try_from(size_of::<crate::Subscription>()).ok()?)?,
        )?
        .checked_add(u64::from(limits.contract_choices).checked_mul(u64::try_from(size_of::<Verdict>()).ok()?)?)?
        .checked_add(u64::from(limits.authority_grants).checked_mul(u64::try_from(size_of::<Grant>()).ok()?)?)?
        .checked_add(
            u64::from(limits.authority_grants).checked_mul(u64::try_from(size_of::<crate::ResourceScope>()).ok()?)?,
        )?
        .checked_add(segments)?
        .checked_add(u64::from(limits.authority_bytes))?
        .checked_add(
            u64::from(limits.executor_kinds).checked_mul(u64::try_from(size_of::<AuthorityExecutor>()).ok()?)?,
        )?;
    let proposed_member = u64::try_from(size_of::<crate::New>())
        .ok()?
        .checked_add(choice)?
        .checked_add(u64::from(limits.spec_bytes))?
        .checked_add(u64::from(limits.authority_bytes))?
        .checked_add(u64::from(limits.dependencies).checked_mul(8)?)?
        .checked_add(
            u64::from(limits.holdings).checked_mul(
                u64::try_from(size_of::<crate::Holding>())
                    .ok()?
                    .checked_add(
                        u64::from(limits.hold_segments).checked_mul(u64::try_from(size_of::<Box<[u8]>>()).ok()?)?,
                    )?
                    .checked_add(u64::from(limits.hold_bytes))?,
            )?,
        )?
        .checked_add(approval_contract)?
        .checked_add(contract_fields)?
        .checked_add(u64::from(limits.contract_choices).checked_mul(u64::try_from(size_of::<Verdict>()).ok()?)?)?;
    let proposal_payload = u64::try_from(size_of::<crate::Proposal>())
        .ok()?
        .checked_add(u64::from(limits.message_bytes))?
        .checked_add(u64::from(limits.message_bytes).checked_mul(2)?)?
        .checked_add(u64::from(limits.batch).checked_mul(proposed_member)?)?;
    let proposal_payload = proposal_payload.max(
        u64::from(crate::SETTINGS_ITEMS)
            .checked_mul(u64::try_from(size_of::<crate::SettingHead>()).ok()?)?
            .checked_add(u64::try_from(size_of::<crate::Proposal>()).ok()?)?
            .checked_add(u64::from(limits.message_bytes).checked_mul(2)?)?,
    );
    // The writer map owns both its opaque key and a durable slot row with a
    // second copy of that name. Count their nested segment vectors and bytes.
    let writer_names = u64::from(limits.tasks).checked_mul(u64::from(limits.holdings))?.checked_mul(2)?.checked_mul(
        u64::from(limits.hold_segments)
            .checked_mul(u64::try_from(size_of::<Box<[u8]>>()).ok()?)?
            .checked_add(u64::from(limits.hold_bytes))?,
    )?;
    Slab::<Task>::worst_case(limits.tasks)?
        // One dirty-row set (plus drain scratch), and one bounded terminal-ID
        // batch per asker. IDs move into continuations, never duplicate text.
        .checked_add(List::<u64>::worst_case(limits.tasks)?.checked_mul(2)?)?
        .checked_add(Map::<u64, List<u64>>::worst_case(limits.tasks)?)?
        .checked_add(u64::from(limits.tasks).checked_mul(u64::from(limits.inbox_messages).checked_mul(8)?)?)?
        .checked_add(crate::placed::worst_case(limits)?)?
        .checked_add(crate::waits::worst_case(limits)?)?
        .checked_add(Map::<u64, crate::PersonProposal>::worst_case(limits.tasks)?)?
        .checked_add(u64::from(limits.tasks).checked_mul(proposal_payload)?)?
        .checked_add(crate::compound::worst_case(limits, payload.checked_add(proposal_payload)?)?)?
        .checked_add(Deadlines::<u64>::worst_case(limits.tasks)?)?
        .checked_add(List::<crate::moving::Plan>::worst_case(limits.delegates)?)?
        .checked_add(u64::from(limits.delegates).checked_mul(List::<u64>::worst_case(limits.tasks)?)?)?
        .checked_add(
            u64::from(limits.tasks)
                .checked_mul(u64::from(limits.result_bytes).checked_add(result_fields)?.checked_mul(2)?)?,
        )?
        .checked_add(Map::<crate::Funder, crate::FundingRecord>::worst_case(limits.funders)?)?
        .checked_add(Map::<u64, Id<Task>>::worst_case(limits.tasks)?)?
        .checked_add(Map::<u64, crate::Stub>::worst_case(stub_capacity(limits)?)?)?
        .checked_add(Deadlines::<u64>::worst_case(limits.tasks)?)?
        .checked_add(Deadlines::<u64>::worst_case(limits.tasks)?)?
        .checked_add(Map::<crate::holds::KindKey, crate::HoldKind>::worst_case(limits.hold_kinds)?)?
        .checked_add(Map::<crate::holds::ResourceKey, crate::ResourceAccess>::worst_case(limits.resource_reports)?)?
        .checked_add(Map::<crate::Name, crate::HoldKind>::worst_case(limits.resource_reports)?)?
        .checked_add(
            u64::from(limits.resource_reports).checked_mul(2)?.checked_mul(
                u64::from(limits.hold_segments)
                    .checked_mul(u64::try_from(size_of::<Box<[u8]>>()).ok()?)?
                    .checked_add(u64::from(limits.hold_bytes))?,
            )?,
        )?
        .checked_add(Map::<crate::Name, crate::PoolSlots>::worst_case(limits.pools)?)?
        .checked_add(
            u64::from(limits.pools).checked_mul(2)?.checked_mul(
                u64::from(limits.hold_segments)
                    .checked_mul(u64::try_from(size_of::<Box<[u8]>>()).ok()?)?
                    .checked_add(u64::from(limits.hold_bytes))?,
            )?,
        )?
        .checked_add(Map::<crate::Name, crate::WriterSlot>::worst_case(limits.tasks.checked_mul(limits.holdings)?)?)?
        .checked_add(writer_names)?
        .checked_add(Deadlines::<u64>::worst_case(limits.tasks)?)?
        .checked_add(Deadlines::<u64>::worst_case(limits.tasks)?)?
        .checked_add(Deadlines::<u64>::worst_case(limits.tasks.checked_mul(limits.subscriptions)?)?)?
        .checked_add(Deadlines::<u64>::worst_case(limits.tasks)?)?
        .checked_add(Queue::<Fact>::worst_case(limits.facts)?)?
        .checked_add(Queue::<crate::CapacityRefusal>::worst_case(limits.room_refusals)?)?
        .checked_add(List::<crate::CapacityRefusal>::worst_case(limits.room_refusals)?)?
        .checked_add(
            u64::from(limits.tasks).checked_mul(
                payload
                    .checked_add(proposal_payload)?
                    .checked_add(proposal_payload)?
                    .checked_add(u64::try_from(size_of::<crate::RecurringState>()).ok()?)?
                    .checked_add(u64::from(limits.batch).checked_mul(8)?)?
                    // Last-published row and atomic procedure admission snapshot.
                    .checked_mul(3)?
                    .checked_add(u64::try_from(size_of::<crate::TaskRecord>()).ok()?)?,
            )?,
        )?
        // Live agent count, including alignment room.
        .checked_add(u64::try_from(size_of::<u64>()).ok()?)?
        // Offered charters and newest removal markers, plus held versions and their models.
        .checked_add(u64::from(limits.charters.checked_mul(2)?.checked_add(limits.versions)?).checked_mul(
            u64::try_from(size_of::<crate::CharterModels>()).ok()?.checked_add(u64::from(crate::MODELS))?,
        )?)?
        .checked_add(Map::<u32, u32>::worst_case(limits.revisions.checked_add(1)?)?)?
        .checked_add(List::<u32>::worst_case(limits.revisions.checked_add(1)?)?)?
        .checked_add(List::<u64>::worst_case(limits.tasks)?)?
        .checked_add(u64::from(crate::MODELS))?
        // Atomic procedure admission saves every live row and finite balance.
        .checked_add(List::<crate::TaskRecord>::worst_case(limits.tasks)?)?
        .checked_add(List::<(crate::Funder, crate::Numbers)>::worst_case(limits.funders)?)?
        // Ordering question positions after their first answer.
        .checked_add(Map::<(bool, core::cmp::Reverse<u64>), crate::QuestionLine>::worst_case(limits.inbox_messages)?)?
        .checked_add(List::<crate::QuestionLine>::worst_case(limits.inbox_messages)?)?
        // Bounded graph/admission and traversal snapshots; no recursive walk.
        .checked_add(List::<u64>::worst_case(limits.tasks.max(limits.batch))?.checked_mul(3)?)?
        .checked_add(List::<u32>::worst_case(limits.batch)?)?
        .checked_add(List::<u64>::worst_case(limits.delegates)?)
}
