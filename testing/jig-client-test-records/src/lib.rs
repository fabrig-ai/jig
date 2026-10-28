//! Shared literals for client document codec tests (testing-strategy.md, 8).
pub use jig_client::shapes::{CHAT, GOAL};
use jig_client_protocol_wire::v19 as w;
use skein_lib::{Duration, List, Writer};
mod owner;
pub use owner::*;
#[must_use]
pub fn limits() -> w::Limits {
    w::Limits {
        document_text_bytes: 32,
        tasks_rows: 4096,
        owner_tree_rows: 4096,
        owner_tree_row_dependencies: 4096,
        owner_inbox_needs: 4096,
        owner_inbox_updates: 4096,
        owner_inbox_selection_some: 4096,
        owner_deployment_hosts: 4096,
        owner_deployment_attempts: 4096,
        owner_deployment_accounts: 4096,
        owner_accounts_accounts: 4096,
        owner_accounts_signing: 4096,
        owner_account_line_sources: 4096,
        owner_account_sources: 4096,
        owner_host_runs: 4096,
        owner_account_runs: 4096,
        owner_account_waiting: 4096,
        run_text_text: 32,
        current_call_input: 32,
        current_call_tool: 32,
        call_identity_id: 32,
        cursor_value: 64,
        owner_inbox_selection_goals: 2,
        me_projects: 512,
        document_specification_parameters: 2,
        document_specification_inputs: 2,
        owner_part_key_path_value: 2,
        owner_part_key_path_value_item: 32,
        ..w::CEILINGS
    }
}
#[must_use]
pub fn wire_text(value: &[u8]) -> w::DocumentText {
    w::DocumentText::new(&limits(), w::DocumentTextParts { bytes: Box::from(value), cut: false })
        .expect("literal fixture fits")
}
#[must_use]
pub fn chip_record() -> w::Chip {
    chip_phase_record(w::ChipPhaseFields::Starting)
}
#[must_use]
pub fn chip_phase_record(phase: w::ChipPhaseFields) -> w::Chip {
    let lim = limits();

    let root = chip_root_fact();
    let child = w::Fact::ChipPhase(
        w::ChipPhaseRow::new(&lim, w::ChipPhaseRowParts { parent: Some(0), value: phase })
            .expect("literal fixture fits"),
    );
    let first = fact_bytes(&root);
    let second = fact_bytes(&child);
    let mut rows = Writer::new(first.len().checked_add(second.len()).expect("literal fixture fits"));
    rows.put(&first).expect("literal fixture fits");
    rows.put(&second).expect("literal fixture fits");
    w::Chip::new(&lim, w::ChipParts { rows: rows.finish() }).expect("literal fixture fits")
}
#[must_use]
pub fn chip_pieces(count: u32) -> Box<[u8]> {
    let row = chip_record();
    let mut encoded = Writer::new(usize::try_from(row.measure()).expect("literal fixture fits"));
    row.encode(&mut encoded).expect("literal fixture fits");
    let encoded = encoded.finish();
    let mut pieces = Writer::new(
        encoded
            .len()
            .checked_add(4)
            .expect("literal fixture fits")
            .checked_mul(usize::try_from(count).expect("literal fixture fits"))
            .expect("literal fixture fits"),
    );
    for _ in 0..count {
        skein_codec::piece::write(&mut pieces, &encoded).expect("literal fixture fits");
    }
    pieces.finish()
}
#[must_use]
pub fn frame_record() -> w::Me {
    let lim = limits();
    w::Me::new(
        &lim,
        w::MeParts {
            providers: Box::new([]),
            providers_cut: false,
            key_bytes: 256,
            redirect_bytes: 4096,
            words_bytes: 65_536,
            amendment_bytes: 65_536,
            party: w::MeParty::new(
                &lim,
                w::MePartyParts {
                    number: 7,
                    kind: w::PartyKind::Person,
                    login: w::DocumentText::new(&lim, w::DocumentTextParts { bytes: Box::from([]), cut: false })
                        .expect("literal fixture fits"),
                    name: w::DocumentText::new(&lim, w::DocumentTextParts { bytes: Box::from([]), cut: false })
                        .expect("literal fixture fits"),
                },
            )
            .expect("literal fixture fits"),
            projects: Box::from([]),
            needs: w::Needs::new(&lim, w::NeedsParts { questions: 7, proposals: 7, escalations: 7, person_tasks: 7 })
                .expect("literal fixture fits"),
            needs_total: 7,
            updates: w::Count::new(&lim, w::CountParts { value: 7, limited: false }).expect("literal fixture fits"),
            results_seen: 7,
            cut: false,
            retention: Duration::from_nanos(19),
            deployment_role: None,
        },
    )
    .expect("literal fixture fits")
}
#[must_use]
#[expect(clippy::too_many_lines, reason = "one complete task wire fixture supplies all owner fields together")]
pub fn task_record() -> w::Task {
    let lim = limits();
    w::Task::new(
        &lim,
        w::TaskParts {
            dependency_facts: List::with_capacity(0),
            kept_account: None,
            run_wait: None,
            settings: Some(
                w::TaskSettings::new(&lim, w::TaskSettingsParts { task: 1, run: None, newest: 1 })
                    .expect("fixture settings"),
            ),
            llm: None,
            label: 0,
            flags: w::TaskFlags::new(
                &lim,
                w::TaskFlagsParts {
                    ended_by: w::EndedBy::Result,
                    conversation: false,
                    listed: false,
                    retention: w::Retention::Kept,
                },
            )
            .expect("fixture flags"),
            last_activity: 0,
            waits_on: w::WaitsOn::Nothing,
            chip: chip_record(),
            goal: None,
            priority: None,
            lineage_chips: Box::from([]),
            inputs: Box::from([]),
            project: 7,
            requester: w::OwnerRequester::Task(
                w::OwnerRequesterTask::new(&lim, w::OwnerRequesterTaskParts { value: 7 })
                    .expect("literal fixture fits"),
            ),
            lineage: List::with_capacity(0),
            spec: wire_specification_document(),
            spec_revision: w::Revision::new(&lim, w::RevisionParts { value: 7 }).expect("literal fixture fits"),
            person_revision: None,
            taken_by: None,
            instructions: None,
            wake: w::OwnerWakePolicy::new(
                &lim,
                w::OwnerWakePolicyParts {
                    words: w::WakeRule::Never,
                    notices: w::WakeRule::Never,
                    news: w::WakeRule::Never,
                    results: w::ResultsWake::Never,
                    questions: false,
                    answers: false,
                    timers: false,
                },
            )
            .expect("literal fixture fits"),
            subscriptions: w::Subscriptions::new(&lim, w::SubscriptionsParts { entries: List::with_capacity(0) })
                .expect("literal fixture fits"),
            result_contract: w::TaskContractDocument::Approval(
                w::TaskContractDocumentApproval::new(
                    &lim,
                    w::TaskContractDocumentApprovalParts {
                        state: Box::from([]),
                        part: w::OwnerPart::new(
                            &lim,
                            w::OwnerPartParts {
                                connector: 7,
                                kind: w::OwnerPartKind::Resource,
                                key: w::OwnerPartKey::Path(
                                    w::OwnerPartKeyPath::new(
                                        &lim,
                                        w::OwnerPartKeyPathParts { value: List::with_capacity(0) },
                                    )
                                    .expect("literal fixture fits"),
                                ),
                            },
                        )
                        .expect("literal fixture fits"),
                    },
                )
                .expect("literal fixture fits"),
            ),
            dependencies: Box::from([]),
            references: Box::from([]),
            cut: false,
            resources: Box::from([]),
            budget: w::OwnerBudget::new(
                &lim,
                w::OwnerBudgetParts {
                    numbers: w::OwnerNumbers::new(
                        &lim,
                        w::OwnerNumbersParts { budget: 7, spent: 7, spent_below: 7, reserved: 7 },
                    )
                    .expect("literal fixture fits"),
                    left: 7,
                    funder: w::Funder::Task(
                        w::FunderTask::new(&lim, w::FunderTaskParts { value: 7 }).expect("literal fixture fits"),
                    ),
                    deadline: None,
                },
            )
            .expect("literal fixture fits"),
            authority: wire_owner_authority_document(),
            hold: None,
            proposals: List::with_capacity(0),
            activity: 7,
            changed: 7,
            acts: Box::from([]),
            result: None,
            delegates: Box::from([]),
            context: None,
        },
    )
    .expect("literal fixture fits")
}
#[must_use]
pub fn wire_specification_document() -> w::SpecificationDocument {
    let lim = limits();
    w::SpecificationDocument::new(
        &lim,
        w::SpecificationDocumentParts {
            specification: w::DocumentSpecification::new(
                &lim,
                w::DocumentSpecificationParts {
                    words: w::DocumentText::new(&lim, w::DocumentTextParts { bytes: Box::from([]), cut: false })
                        .expect("literal fixture fits"),
                    parameters: List::with_capacity(0),
                    inputs: List::with_capacity(0),
                },
            )
            .expect("literal fixture fits"),
            cut: false,
        },
    )
    .expect("literal fixture fits")
}
#[must_use]
pub fn wire_owner_authority_document() -> w::OwnerAuthorityDocument {
    let lim = limits();
    w::OwnerAuthorityDocument::new(
        &lim,
        w::OwnerAuthorityDocumentParts {
            cut: false,
            authority: w::OwnerAuthority::new(
                &lim,
                w::OwnerAuthorityParts {
                    models: Some(w::ModelSet::All),
                    tools: 7,
                    grants: Box::from([]),
                    delegation: w::OwnerDelegation::new(
                        &lim,
                        w::OwnerDelegationParts { kinds: List::with_capacity(0), tasks: 7, depth: 7 },
                    )
                    .expect("literal fixture fits"),
                    spend: 7,
                    deadline: None,
                    notes: 7,
                    note_resources: Box::from([]),
                },
            )
            .expect("literal fixture fits"),
        },
    )
    .expect("literal fixture fits")
}
#[must_use]
pub fn chip_root_fact() -> w::Fact {
    let lim = limits();
    w::Fact::Chip(
        w::ChipRow::new(
            &lim,
            w::ChipRowParts {
                parent: None,
                value: w::ChipFields::new(
                    &lim,
                    w::ChipFieldsParts {
                        task: 7,
                        title: wire_text(b""),
                        executor: w::Executor::Agent(
                            w::ExecutorAgent::new(&lim, w::ExecutorAgentParts { charter: 7 })
                                .expect("literal fixture fits"),
                        ),
                        phase: w::Phase::Waiting,
                        details: 1,
                    },
                )
                .expect("literal fixture fits"),
            },
        )
        .expect("literal fixture fits"),
    )
}
#[must_use]
pub fn wire_owner_chip_details_row() -> w::OwnerChipDetailsRow {
    let lim = limits();
    w::OwnerChipDetailsRow::new(
        &lim,
        w::OwnerChipDetailsRowParts {
            parent: Some(1),
            value: w::OwnerChipDetailsFields::new(
                &lim,
                w::OwnerChipDetailsFieldsParts {
                    dependencies: List::with_capacity(0),
                    project: 7,
                    executor: w::OwnerExecutor::Agent(
                        w::OwnerExecutorAgent::new(&lim, w::OwnerExecutorAgentParts { charter: 7 })
                            .expect("literal fixture fits"),
                    ),
                    phase_since: 7,
                    run_since: None,
                    attempt: None,
                    retry: None,
                    wake: w::OwnerWakePolicy::new(
                        &lim,
                        w::OwnerWakePolicyParts {
                            words: w::WakeRule::Never,
                            notices: w::WakeRule::Never,
                            news: w::WakeRule::Never,
                            results: w::ResultsWake::Never,
                            questions: false,
                            answers: false,
                            timers: false,
                        },
                    )
                    .expect("literal fixture fits"),
                    person_wait: None,
                    hold: None,
                    budget: w::OwnerBudget::new(
                        &lim,
                        w::OwnerBudgetParts {
                            numbers: w::OwnerNumbers::new(
                                &lim,
                                w::OwnerNumbersParts { budget: 7, spent: 7, spent_below: 7, reserved: 7 },
                            )
                            .expect("literal fixture fits"),
                            left: 7,
                            funder: w::Funder::Task(
                                w::FunderTask::new(&lim, w::FunderTaskParts { value: 7 })
                                    .expect("literal fixture fits"),
                            ),
                            deadline: None,
                        },
                    )
                    .expect("literal fixture fits"),
                    activity: 7,
                    resources: Box::from([]),
                    resources_cut: false,
                    resources_observed: false,
                    procedure: None,
                    live_delegates: 7,
                    closing_connectors: None,
                    ending: None,
                    ended_at: None,
                    observation: None,
                },
            )
            .expect("literal fixture fits"),
        },
    )
    .expect("literal fixture fits")
}
#[must_use]
pub fn chip_from_rows(rows: &[w::Fact]) -> w::Chip {
    let mut length = 0_usize;
    for row in rows {
        length = length
            .checked_add(usize::try_from(row.measure()).expect("literal fixture fits"))
            .expect("literal fixture fits")
            .checked_add(4)
            .expect("literal fixture fits");
    }
    let mut bytes = Writer::new(length);
    for row in rows {
        bytes.put(&fact_bytes(row)).expect("literal fixture fits");
    }
    w::Chip::new(&limits(), w::ChipParts { rows: bytes.finish() }).expect("literal fixture fits")
}
#[must_use]
pub fn fact_bytes(value: &w::Fact) -> Box<[u8]> {
    let mut row = Writer::new(usize::try_from(value.measure()).expect("literal fixture fits"));
    value.encode(&mut row).expect("literal fixture fits");
    let mut piece = Writer::new(
        usize::try_from(value.measure().checked_add(4).expect("literal fixture fits")).expect("literal fixture fits"),
    );
    skein_codec::piece::write(&mut piece, &row.finish()).expect("literal fixture fits");
    piece.finish()
}

mod live;
pub use live::*;
mod accounts;
mod live_values;
pub use accounts::*;
pub use live_values::*;

mod objects;
pub use objects::*;

mod object_reads;
pub use object_reads::*;

mod pages;
pub use pages::*;

mod page_reads;
pub use page_reads::*;

mod activity_reads;
pub use activity_reads::*;

pub mod settings;
pub mod settings_proposals;
