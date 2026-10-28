//! General operation targets and project template overrides on the wire.
use super::asks;
use alloc::boxed::Box;
use jig_client_domain::contract as c;
use jig_client_protocol_wire::v19 as w;
use skein_lib::{List, bytes};

pub(super) fn answer(value: &c::AnswerOn, limits: &w::Limits) -> Result<w::AnswerOn, w::Problem> {
    Ok(match value {
        c::AnswerOn::Question { task, message, words } => w::AnswerOn::Question(w::AnswerQuestion::new(
            limits,
            w::AnswerQuestionParts { task: *task, message: *message, words: bytes::copy_of(words.bytes()) },
        )?),
        c::AnswerOn::Person { task, revision, answer } => w::AnswerOn::Person(w::AnswerPerson::new(
            limits,
            w::AnswerPersonParts {
                task: *task,
                revision: asks::wire_revision(*revision, limits)?,
                answer: asks::wire_person_answer(answer, limits)?,
            },
        )?),
    })
}
pub(super) fn decide(value: &c::DecideOn, limits: &w::Limits) -> Result<w::DecideOn, w::Problem> {
    Ok(match value {
        c::DecideOn::Proposal { proposal, decision, running } => w::DecideOn::Proposal(w::DecideProposal::new(
            limits,
            w::DecideProposalParts {
                running: match running {
                    Some(value) => Some(super::settings::wire_running(value, limits)?),
                    None => None,
                },
                proposal: asks::wire_proposal_key(proposal, limits)?,
                decision: asks::wire_decision(decision, limits)?,
            },
        )?),
        c::DecideOn::Escalation { task, revision, decision } => w::DecideOn::Escalation(w::DecideEscalation::new(
            limits,
            w::DecideEscalationParts {
                task: *task,
                revision: asks::wire_revision(*revision, limits)?,
                decision: asks::wire_decision(decision, limits)?,
            },
        )?),
    })
}
pub(super) fn templates(values: &List<c::TemplateOverride>, limits: &w::Limits) -> Result<w::PolicyEdit, w::Problem> {
    let mut overrides = List::with_capacity(values.len());
    for v in values {
        overrides
            .push(w::TemplateOverride::new(
                limits,
                w::TemplateOverrideParts {
                    template: v.template,
                    executor: match v.executor {
                        Some(value) => Some(executor(value, limits)?),
                        None => None,
                    },
                    budget: match v.budget {
                        Some(value) => Some(budget(value, limits)?),
                        None => None,
                    },
                    deadline: match v.deadline {
                        Some(value) => Some(deadline(value, limits)?),
                        None => None,
                    },
                    priority: v.priority,
                },
            )?)
            .expect("same capacity");
    }
    Ok(w::PolicyEdit::Templates(w::PolicyEditTemplates::new(limits, w::PolicyEditTemplatesParts { overrides })?))
}
pub(super) fn executor(value: c::TemplateExecutor, limits: &w::Limits) -> Result<w::TemplateExecutor, w::Problem> {
    Ok(match value {
        c::TemplateExecutor::Charter(value) => {
            w::TemplateExecutor::Charter(w::TemplateCharter::new(limits, w::TemplateCharterParts { value })?)
        }
        c::TemplateExecutor::Chosen => w::TemplateExecutor::Chosen,
    })
}
pub(super) fn budget(value: c::TemplateBudget, limits: &w::Limits) -> Result<w::TemplateBudget, w::Problem> {
    Ok(match value {
        c::TemplateBudget::Amount(value) => {
            w::TemplateBudget::Amount(w::TemplateAmount::new(limits, w::TemplateAmountParts { value })?)
        }
        c::TemplateBudget::AsAllowed { cap } => {
            w::TemplateBudget::AsAllowed(w::TemplateAsAllowed::new(limits, w::TemplateAsAllowedParts { cap })?)
        }
        c::TemplateBudget::Given => w::TemplateBudget::Given,
    })
}
pub(super) fn deadline(value: c::TemplateDeadline, limits: &w::Limits) -> Result<w::TemplateDeadline, w::Problem> {
    Ok(match value {
        c::TemplateDeadline::None => w::TemplateDeadline::None,
        c::TemplateDeadline::Span(value) => {
            w::TemplateDeadline::Span(w::TemplateSpan::new(limits, w::TemplateSpanParts { value })?)
        }
        c::TemplateDeadline::Given => w::TemplateDeadline::Given,
    })
}
pub(super) fn restore_templates(values: &List<w::TemplateOverride>) -> c::PolicyEdit {
    c::PolicyEdit::Templates { overrides: restore_template_overrides(values) }
}
#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(super) fn restore_template_overrides(values: &List<w::TemplateOverride>) -> List<c::TemplateOverride> {
    let mut overrides = List::with_capacity(values.len());
    for v in values {
        overrides
            .push(c::TemplateOverride {
                template: v.template(),
                executor: match v.executor() {
                    Some(value) => Some(restore_executor(value)),
                    None => None,
                },
                budget: match v.budget() {
                    Some(value) => Some(restore_budget(value)),
                    None => None,
                },
                deadline: match v.deadline() {
                    Some(value) => Some(restore_deadline(value)),
                    None => None,
                },
                priority: *v.priority(),
            })
            .expect("same capacity");
    }
    overrides
}
pub(super) fn restore_executor(value: &w::TemplateExecutor) -> c::TemplateExecutor {
    match value {
        w::TemplateExecutor::Charter(v) => c::TemplateExecutor::Charter(v.value()),
        w::TemplateExecutor::Chosen => c::TemplateExecutor::Chosen,
    }
}
pub(super) fn restore_budget(value: &w::TemplateBudget) -> c::TemplateBudget {
    match value {
        w::TemplateBudget::Amount(v) => c::TemplateBudget::Amount(v.value()),
        w::TemplateBudget::AsAllowed(v) => c::TemplateBudget::AsAllowed { cap: *v.cap() },
        w::TemplateBudget::Given => c::TemplateBudget::Given,
    }
}
pub(super) fn restore_deadline(value: &w::TemplateDeadline) -> c::TemplateDeadline {
    match value {
        w::TemplateDeadline::None => c::TemplateDeadline::None,
        w::TemplateDeadline::Span(v) => c::TemplateDeadline::Span(v.value()),
        w::TemplateDeadline::Given => c::TemplateDeadline::Given,
    }
}

pub(super) fn tracking(value: c::Tracking, limits: &w::Limits) -> Result<w::Tracking, w::Problem> {
    Ok(match value {
        c::Tracking::Tracked { priority } => {
            w::Tracking::Tracked(w::TrackingTracked::new(limits, w::TrackingTrackedParts { priority })?)
        }
        c::Tracking::Untracked => w::Tracking::Untracked,
    })
}
pub(super) fn restore_tracking(value: &w::Tracking) -> c::Tracking {
    match value {
        w::Tracking::Tracked(v) => c::Tracking::Tracked { priority: v.priority() },
        w::Tracking::Untracked => c::Tracking::Untracked,
    }
}

pub(super) fn create(project: u32, tasks: &List<c::NewTask>, limits: &w::Limits) -> Result<w::Ask, w::Problem> {
    Ok(w::Ask::Create(w::Create::new(
        limits,
        w::CreateParts { project, tasks: batch(tasks, limits.create_tasks, limits)? },
    )?))
}
fn batch(tasks: &List<c::NewTask>, bound: u32, limits: &w::Limits) -> Result<Box<[u8]>, w::Problem> {
    let mut records = List::with_capacity(tasks.len());
    let mut size = 0_u32;
    for task in tasks {
        let record = new_task(task, limits)?;
        size = size.checked_add(record.measure()).ok_or(super::too_large())?.checked_add(4).ok_or(super::too_large())?;
        if size > bound {
            return Err(super::too_large());
        }
        records.push(record).expect("same capacity");
    }
    let mut out = skein_lib::Writer::new(usize::try_from(size).expect("wire size fits"));
    for record in records.into_boxed() {
        out.put(&record.measure().to_be_bytes()).expect("measured piece");
        record.encode(&mut out).expect("measured task");
    }
    Ok(out.finish())
}
fn new_task(v: &c::NewTask, limits: &w::Limits) -> Result<w::NewTask, w::Problem> {
    let mut dependencies = List::with_capacity(v.dependencies.len());
    for item in &v.dependencies {
        dependencies
            .push(match item {
                c::TaskRef::Member(value) => {
                    w::TaskRef::Member(w::TaskRefMember::new(limits, w::TaskRefMemberParts { value: *value })?)
                }
                c::TaskRef::Task(value) => {
                    w::TaskRef::Task(w::TaskRefTask::new(limits, w::TaskRefTaskParts { value: *value })?)
                }
            })
            .expect("same capacity");
    }
    let mut named = List::with_capacity(v.named.len());
    for grant in &v.named {
        named.push(asks::wire_owner_grant(grant, limits)?).expect("same capacity");
    }
    let executor = match v.executor {
        c::OwnerExecutor::Agent { charter } => {
            w::OwnerExecutor::Agent(w::OwnerExecutorAgent::new(limits, w::OwnerExecutorAgentParts { charter })?)
        }
        c::OwnerExecutor::Procedure { connector, code } => w::OwnerExecutor::Procedure(w::OwnerExecutorProcedure::new(
            limits,
            w::OwnerExecutorProcedureParts { connector, code },
        )?),
        c::OwnerExecutor::Person(address) => w::OwnerExecutor::Person(w::OwnerExecutorPerson::new(
            limits,
            w::OwnerExecutorPersonParts { value: asks::wire_person_address(&address, limits)? },
        )?),
    };
    w::NewTask::new(
        limits,
        w::NewTaskParts {
            executor,
            spec: asks::wire_specification(&v.spec, limits)?,
            contract: new_contract(&v.contract, limits)?,
            authority: asks::wire_owner_authority(&v.authority, limits)?,
            deadline: match v.deadline {
                c::NewDeadline::Named => w::NewDeadline::Named,
                c::NewDeadline::Span(value) => {
                    w::NewDeadline::Span(w::DeadlineSpan::new(limits, w::DeadlineSpanParts { value })?)
                }
            },
            budget: match v.budget {
                c::NewBudget::Named => w::NewBudget::Named,
                c::NewBudget::AsAllowed { cap } => {
                    w::NewBudget::AsAllowed(w::BudgetAsAllowed::new(limits, w::BudgetAsAllowedParts { cap })?)
                }
            },
            wake: match &v.wake {
                Some(wake) => Some(asks::wire_owner_wake_policy(wake, limits)?),
                None => None,
            },
            dependencies,
            named: asks::wire_owner_grant_piece(&named, limits)?,
            tracked: v.tracked,
            label: v.label,
            flags: w::TaskFlags::new(
                limits,
                w::TaskFlagsParts {
                    ended_by: match v.flags.ended_by {
                        c::EndedBy::Result => w::EndedBy::Result,
                        c::EndedBy::Requester => w::EndedBy::Requester,
                    },
                    conversation: v.flags.conversation,
                    listed: v.flags.listed,
                    retention: match v.flags.retention {
                        c::Retention::Kept => w::Retention::Kept,
                        c::Retention::ForgottenOnceRead => w::Retention::ForgottenOnceRead,
                    },
                },
            )?,
            llm: super::llm::wire_choice(v.llm.as_ref(), limits)?,
        },
    )
}
fn new_contract(value: &c::NewContract, limits: &w::Limits) -> Result<w::NewContract, w::Problem> {
    Ok(match value {
        c::NewContract::Report { words } => {
            w::NewContract::Report(w::ContractReport::new(limits, w::ContractReportParts { words: *words })?)
        }
        c::NewContract::Change { connector, kind, words } => w::NewContract::Change(w::ContractChange::new(
            limits,
            w::ContractChangeParts { connector: *connector, kind: *kind, words: *words },
        )?),
        c::NewContract::Verdict { choices } => {
            let mut records = List::with_capacity(choices.len());
            for v in choices {
                let mut fields = List::with_capacity(v.fields.len());
                for field in &v.fields {
                    fields
                        .push(w::FieldRule::new(
                            limits,
                            w::FieldRuleParts { name: bytes::copy_of(&field.name), max: field.max },
                        )?)
                        .expect("same capacity");
                }
                records
                    .push(w::VerdictChoice::new(
                        limits,
                        w::VerdictChoiceParts { code: v.code, words: v.words, followups: v.followups, fields },
                    )?)
                    .expect("same capacity");
            }
            w::NewContract::Verdict(w::ContractVerdict::new(limits, w::ContractVerdictParts { choices: records })?)
        }
    })
}
pub(super) fn restore_create(value: &w::Create, limits: &crate::Limits) -> Result<c::Ask, w::Problem> {
    Ok(c::Ask::Create { project: value.project(), tasks: restore_batch(value.tasks(), limits)? })
}
fn restore_batch(bytes: &[u8], limits: &crate::Limits) -> Result<List<c::NewTask>, w::Problem> {
    let mut reader = skein_lib::Reader::new(bytes);
    let mut tasks = List::with_capacity(limits.client.rows);
    while let Some(bytes) = create_piece(&mut reader)? {
        if tasks.len() == tasks.capacity() {
            return Err(super::too_large());
        }
        let v = w::NewTask::decode(&limits.wire, &mut skein_lib::Reader::new(&bytes))?;
        tasks.push(restore_task(&v, limits)?).expect("checked capacity");
    }
    Ok(tasks)
}
fn restore_task(v: &w::NewTask, limits: &crate::Limits) -> Result<c::NewTask, w::Problem> {
    let mut dependencies = List::with_capacity(v.dependencies().len());
    for item in v.dependencies() {
        dependencies
            .push(match item {
                w::TaskRef::Member(v) => c::TaskRef::Member(v.value()),
                w::TaskRef::Task(v) => c::TaskRef::Task(v.value()),
            })
            .expect("same capacity");
    }
    let contract = match v.contract() {
        w::NewContract::Report(v) => c::NewContract::Report { words: v.words() },
        w::NewContract::Change(v) => {
            c::NewContract::Change { connector: v.connector(), kind: v.kind(), words: v.words() }
        }
        w::NewContract::Verdict(v) => {
            let mut choices = List::with_capacity(v.choices().len());
            for choice in v.choices() {
                choices.push(super::documents::verdict_choice_fields(choice, limits)?).expect("same capacity");
            }
            c::NewContract::Verdict { choices }
        }
    };
    Ok(c::NewTask {
        deadline: match v.deadline() {
            w::NewDeadline::Named => c::NewDeadline::Named,
            w::NewDeadline::Span(value) => c::NewDeadline::Span(value.value()),
        },
        executor: super::facts::owner_executor(v.executor(), limits),
        spec: super::pages::specification_fields(v.spec(), limits)?,
        contract,
        authority: super::documents::owner_authority_fields(v.authority(), limits)?,
        budget: match v.budget() {
            w::NewBudget::Named => c::NewBudget::Named,
            w::NewBudget::AsAllowed(v) => c::NewBudget::AsAllowed { cap: *v.cap() },
        },
        wake: restore_wake(v.wake().as_ref(), limits),
        dependencies,
        named: super::documents::owner_grant_piece(v.named(), limits, limits.client.authority)?,
        tracked: *v.tracked(),
        label: v.label(),
        flags: c::TaskFlags {
            ended_by: match v.flags().ended_by() {
                w::EndedBy::Result => c::EndedBy::Result,
                w::EndedBy::Requester => c::EndedBy::Requester,
            },
            conversation: v.flags().conversation(),
            listed: v.flags().listed(),
            retention: match v.flags().retention() {
                w::Retention::Kept => c::Retention::Kept,
                w::Retention::ForgottenOnceRead => c::Retention::ForgottenOnceRead,
            },
        },
        llm: super::llm::choice(v.llm().as_ref(), limits)?,
    })
}

fn create_piece(reader: &mut skein_lib::Reader<'_>) -> Result<Option<Box<[u8]>>, w::Problem> {
    match skein_codec::piece::split(reader) {
        Ok(value) => Ok(value),
        Err(reason) => Err(super::piece_problem(reason)),
    }
}

pub(super) fn propose(
    project: u32,
    action: &c::ProposeAction,
    reason: &c::Text,
    limits: &w::Limits,
) -> Result<w::Ask, w::Problem> {
    let action = match action {
        c::ProposeAction::ReleasePlaced { task, hold, revision } => {
            w::ProposeAction::ReleasePlaced(w::ProposeReleasePlaced::new(
                limits,
                w::ProposeReleasePlacedParts { task: *task, hold: *hold, revision: *revision },
            )?)
        }
        c::ProposeAction::Settings(value) => {
            w::ProposeAction::Settings(super::settings_proposals::wire_propose_settings(value, limits)?)
        }
        c::ProposeAction::Batch { tasks } => w::ProposeAction::Batch(w::ProposeBatch::new(
            limits,
            w::ProposeBatchParts { tasks: batch(tasks, limits.propose_batch_tasks, limits)? },
        )?),
        c::ProposeAction::Amendment { task, revision, change } => {
            w::ProposeAction::Amendment(Box::new(w::ProposeAmendment::new(
                limits,
                w::ProposeAmendmentParts {
                    task: *task,
                    revision: asks::wire_revision(*revision, limits)?,
                    change: asks::wire_amendment(change, limits)?,
                },
            )?))
        }
    };
    Ok(w::Ask::Propose(w::Propose::new(
        limits,
        w::ProposeParts { project, action, reason: bytes::copy_of(reason.bytes()) },
    )?))
}
pub(super) fn restore_propose(value: &w::Propose, limits: &crate::Limits) -> Result<c::Ask, w::Problem> {
    let action = match value.action() {
        w::ProposeAction::ReleasePlaced(value) => {
            c::ProposeAction::ReleasePlaced { task: value.task(), hold: value.hold(), revision: value.revision() }
        }
        w::ProposeAction::Settings(value) => {
            c::ProposeAction::Settings(super::settings_proposals::restore_propose_settings(value, limits)?)
        }
        w::ProposeAction::Batch(value) => c::ProposeAction::Batch { tasks: restore_batch(value.tasks(), limits)? },
        w::ProposeAction::Amendment(value) => c::ProposeAction::Amendment {
            task: value.task(),
            revision: super::answers::revision(value.revision(), limits),
            change: Box::new(super::pages::amendment_fields(value.change(), limits)?),
        },
    };
    let reason = super::store::text(value.reason(), limits)?;
    Ok(c::Ask::Propose { project: value.project(), action, reason })
}

fn restore_wake(value: Option<&w::OwnerWakePolicy>, limits: &crate::Limits) -> Option<c::OwnerWakePolicy> {
    let wake = value?;
    Some(super::facts::owner_wake_policy(wake, limits))
}
