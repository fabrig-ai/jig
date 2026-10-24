//! General operation targets and template overrides, translated into people records.
use super::{Untranslatable, asks};
use alloc::boxed::Box;
use jig_client_protocol_wire::v19 as w;
use jig_domain_people as p;
use skein_lib::List;

pub(super) fn answer(project: u32, on: w::AnswerOn) -> p::Ask {
    match on {
        w::AnswerOn::Question(value) => {
            let v = value.into_parts();
            p::Ask::AnswerQuestion { project, task: v.task, question: v.message, words: v.words }
        }
        w::AnswerOn::Person(value) => {
            let v = value.into_parts();
            p::Ask::AnswerPerson {
                project,
                task: v.task,
                revision: v.revision.value(),
                result: asks::person_answer(v.answer),
            }
        }
    }
}
pub(super) fn decide(project: u32, on: w::DecideOn) -> Result<p::Ask, Untranslatable> {
    Ok(match on {
        w::DecideOn::Proposal(value) => {
            let v = value.into_parts();
            p::Ask::DecideProposal {
                project,
                proposer: asks::proposer(v.proposal.proposer()),
                proposal: v.proposal.number(),
                decision: asks::proposal_decision(v.decision)?,
                running: match v.running {
                    Some(w::Running::Stays) => Some(p::Running::Stays),
                    Some(w::Running::Moves) => Some(p::Running::Moves),
                    None => None,
                },
            }
        }
        w::DecideOn::Escalation(value) => {
            let v = value.into_parts();
            p::Ask::DecideEscalation {
                project,
                task: v.task,
                revision: v.revision.value(),
                decision: asks::escalation_decision(v.decision)?,
            }
        }
    })
}
pub(super) fn templates(value: w::PolicyEditTemplates) -> p::PolicyChange {
    p::PolicyChange::Templates { overrides: overrides(value.into_parts().overrides) }
}
#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(super) fn overrides(values: List<w::TemplateOverride>) -> Box<[p::TemplateOverride]> {
    let mut overrides = List::with_capacity(values.capacity());
    for value in values.into_boxed() {
        let v = value.into_parts();
        overrides
            .push(p::TemplateOverride {
                template: v.template,
                executor: match v.executor {
                    Some(value) => Some(executor(value)),
                    None => None,
                },
                budget: match v.budget {
                    Some(value) => Some(budget(value)),
                    None => None,
                },
                deadline: match v.deadline {
                    Some(value) => Some(deadline(value)),
                    None => None,
                },
                priority: v.priority,
            })
            .expect("same capacity");
    }
    overrides.into_boxed()
}
pub(super) fn executor(value: w::TemplateExecutor) -> p::TemplateExecutor {
    match value {
        w::TemplateExecutor::Charter(v) => p::TemplateExecutor::Charter(v.value()),
        w::TemplateExecutor::Chosen => p::TemplateExecutor::Chosen,
    }
}
pub(super) fn budget(value: w::TemplateBudget) -> p::TemplateBudget {
    match value {
        w::TemplateBudget::Amount(v) => p::TemplateBudget::Amount(v.value()),
        w::TemplateBudget::AsAllowed(v) => p::TemplateBudget::AsAllowed { cap: *v.cap() },
        w::TemplateBudget::Given => p::TemplateBudget::Given,
    }
}
pub(super) fn deadline(value: w::TemplateDeadline) -> p::TemplateDeadline {
    match value {
        w::TemplateDeadline::None => p::TemplateDeadline::None,
        w::TemplateDeadline::Span(v) => p::TemplateDeadline::Span(v.value()),
        w::TemplateDeadline::Given => p::TemplateDeadline::Given,
    }
}

pub(super) fn tracking(value: w::Tracking) -> p::Tracking {
    match value {
        w::Tracking::Tracked(v) => p::Tracking::Tracked { priority: v.priority() },
        w::Tracking::Untracked => p::Tracking::Untracked,
    }
}

/// Decode the pieces before anything reaches the core.
pub(super) fn compound(
    value: w::Compound,
    models: &[crate::Model],
    limits: &w::Limits,
    names: &mut List<Box<[u8]>>,
) -> Result<p::Ask, Untranslatable> {
    let path = w::Path::CompoundOperations;
    let mut reader = skein_lib::Reader::new(value.operations());
    let mut decoded = List::with_capacity(asks::count(value.operations(), path)?);
    while reader.remaining() != 0 {
        let Ok(Some(bytes)) = skein_codec::piece::split(&mut reader) else {
            return Err(Untranslatable { path });
        };
        let Ok(operation) = w::Operation::decode(limits, &mut skein_lib::Reader::new(&bytes)) else {
            return Err(Untranslatable { path });
        };
        decoded.push(operation).expect("pieces bounded by bytes");
    }
    let mut operations = List::with_capacity(decoded.len());
    for operation in decoded.into_boxed() {
        operations
            .push(operation_to_people(operation, value.project(), models, limits, names)?)
            .expect("same capacity");
    }
    Ok(p::Ask::Compound { project: value.project(), operations })
}
fn operation_to_people(
    value: w::Operation,
    project: u32,
    models: &[crate::Model],
    limits: &w::Limits,
    names: &mut List<Box<[u8]>>,
) -> Result<p::Operation, Untranslatable> {
    if operation_project(&value) != project {
        return Err(Untranslatable { path: w::Path::CompoundOperations });
    }
    Ok(match value {
        w::Operation::Message(v) => {
            let v = v.into_parts();
            p::Operation::Message { task: v.task, words: v.words }
        }
        w::Operation::Amend(v) => {
            let v = v.into_parts();
            p::Operation::Amend {
                task: v.task,
                revision: match v.revision {
                    Some(v) => v.value(),
                    None => 0,
                },
                amendment: Box::new(asks::amendment(v.change, v.reason, models, limits, names)?),
                release: None,
            }
        }
        w::Operation::Cancel(v) => {
            let v = v.into_parts();
            p::Operation::Cancel { task: v.task, reason: v.reason, grace: v.grace }
        }
        w::Operation::Release(v) => {
            let v = v.into_parts();
            p::Operation::Release { task: v.task, revision: v.revision.value(), again: v.again }
        }
        w::Operation::Stop(v) => p::Operation::Stop { task: v.task() },
        w::Operation::MoveTask(v) => {
            let v = v.into_parts();
            p::Operation::Move { task: v.task, reason: v.reason }
        }
    })
}
fn operation_project(value: &w::Operation) -> u32 {
    match value {
        w::Operation::Message(v) => v.project(),
        w::Operation::Amend(v) => v.project(),
        w::Operation::Cancel(v) => v.project(),
        w::Operation::Release(v) => v.project(),
        w::Operation::Stop(v) => v.project(),
        w::Operation::MoveTask(v) => v.project(),
    }
}

/// Project mismatch is a typed refusal before the core sees any operation.
pub(crate) fn refused(record: &w::Ask, limits: &w::Limits) -> Result<Option<w::KeyedAnswer>, w::Problem> {
    let w::Ask::Compound(value) = record else {
        return Ok(None);
    };
    let mut reader = skein_lib::Reader::new(value.operations());
    let mut at = 0_u32;
    while let Some(bytes) = wire_piece(&mut reader)? {
        let operation = w::Operation::decode(limits, &mut skein_lib::Reader::new(&bytes))?;
        let project = operation_project(&operation);
        if project != value.project() {
            let refusal = w::Refusal::Malformed(w::RefusalMalformed::new(
                limits,
                w::RefusalMalformedParts { at, why: w::Malformed::ProjectMismatch },
            )?);
            let answer = w::Final::Refused(w::Refused::new(limits, w::RefusedParts { refusal, at: Some(at) })?);
            let mut out = skein_lib::Writer::new(usize::try_from(answer.measure()).expect("wire size fits"));
            answer.encode(&mut out).expect("measured answer");
            return match super::answers::owned(w::Owner::Jig, jig_client_protocol_wire::VERSION, out.finish(), limits) {
                Ok(value) => Ok(Some(value)),
                Err(problem) => Err(problem),
            };
        }
        at = at.checked_add(1).ok_or(super::problem(skein_codec::Reason::Bound))?;
    }
    Ok(None)
}

fn wire_piece(reader: &mut skein_lib::Reader<'_>) -> Result<Option<Box<[u8]>>, w::Problem> {
    match skein_codec::piece::split(reader) {
        Ok(value) => Ok(value),
        Err(_) => Err(super::problem(skein_codec::Reason::Malformed)),
    }
}

/// Batch members remain data until people admission resolves their authority.
pub(super) fn create(
    value: w::Create,
    models: &[crate::Model],
    limits: &w::Limits,
    names: &mut List<Box<[u8]>>,
) -> Result<p::Ask, Untranslatable> {
    Ok(p::Ask::Create {
        project: value.project(),
        batch: batch(value.tasks(), w::Path::CreateTasks, models, limits, names)?,
    })
}
fn batch(
    bytes: &[u8],
    path: w::Path,
    models: &[crate::Model],
    limits: &w::Limits,
    names: &mut List<Box<[u8]>>,
) -> Result<Box<[p::Member]>, Untranslatable> {
    let mut tasks = List::with_capacity(asks::count(bytes, path)?);
    let mut reader = skein_lib::Reader::new(bytes);
    for _ in 0..tasks.capacity() {
        let bytes = asks::item(&mut reader, path)?;
        let Ok(task) = w::NewTask::decode(limits, &mut skein_lib::Reader::new(&bytes)) else {
            return Err(Untranslatable { path });
        };
        tasks.push(member(task, models, limits, names)?).expect("counted pieces");
    }
    Ok(tasks.into_boxed())
}
fn member(
    value: w::NewTask,
    models: &[crate::Model],
    limits: &w::Limits,
    names: &mut List<Box<[u8]>>,
) -> Result<p::Member, Untranslatable> {
    let v = value.into_parts();
    let spec = asks::spec(v.spec);
    let mut dependencies = List::with_capacity(v.dependencies.len());
    for dependency in v.dependencies.into_boxed() {
        dependencies
            .push(match dependency {
                w::TaskRef::Member(v) => p::Dependency::Batch(v.value()),
                w::TaskRef::Task(v) => p::Dependency::Existing(v.value()),
            })
            .expect("same capacity");
    }
    let flags = v.flags.into_parts();
    Ok(p::Member {
        deadline: match v.deadline {
            w::NewDeadline::Named => p::Deadline::Named,
            w::NewDeadline::Span(value) => p::Deadline::Span(value.value()),
        },
        executor: match v.executor {
            w::OwnerExecutor::Agent(v) => p::TaskExecutor::Agent { charter: v.charter() },
            w::OwnerExecutor::Procedure(v) => p::TaskExecutor::Procedure { connector: v.connector(), code: v.code() },
            w::OwnerExecutor::Person(v) => p::TaskExecutor::Person(match v.value() {
                w::PersonAddress::Party(v) => p::PersonAddress::Person(v.value()),
                w::PersonAddress::Role(v) => p::PersonAddress::Role(v.value()),
            }),
        },
        words: spec.words,
        parameters: spec.parameters,
        inputs: spec.inputs,
        contract: new_contract(v.contract),
        authority: asks::authority(v.authority, limits, names)?,
        budget: match v.budget {
            w::NewBudget::Named => p::Budget::Named,
            w::NewBudget::AsAllowed(v) => p::Budget::AsAllowed { cap: *v.cap() },
        },
        wake: match v.wake {
            Some(wake) => asks::wake(wake),
            None => p::WakePolicy {
                words: p::WakeRule::Immediate,
                notices: p::WakeRule::Immediate,
                news: p::WakeRule::Immediate,
                results: p::ResultsWake::LastOrFailure,
                questions: true,
                answers: true,
                timers: true,
            },
        },
        dependencies: dependencies.into_boxed(),
        named: asks::grants(&v.named, limits.new_task_named, w::Path::NewTaskNamed, limits)?,
        tracked: v.tracked,
        label: v.label,
        flags: p::Flags {
            ended_by: match flags.ended_by {
                w::EndedBy::Result => p::EndedBy::Result,
                w::EndedBy::Requester => p::EndedBy::Requester,
            },
            conversation: flags.conversation,
            listed: flags.listed,
            retention: match flags.retention {
                w::Retention::Kept => p::Retention::Kept,
                w::Retention::ForgottenOnceRead => p::Retention::ForgottenOnceRead,
            },
        },
        llm: super::llm::people(v.llm, models, w::Path::NewTaskLlm)?,
    })
}
fn new_contract(value: w::NewContract) -> p::Contract {
    match value {
        w::NewContract::Report(v) => p::Contract::Report { words: v.words() },
        w::NewContract::Change(v) => p::Contract::Change { connector: v.connector(), kind: v.kind(), words: v.words() },
        w::NewContract::Verdict(v) => {
            let mut choices = List::with_capacity(v.choices().len());
            for choice in v.into_parts().choices.into_boxed() {
                let v = choice.into_parts();
                let mut fields = List::with_capacity(v.fields.len());
                for field in v.fields.into_boxed() {
                    let v = field.into_parts();
                    fields.push(p::FieldRule { name: v.name, max: v.max }).expect("same capacity");
                }
                choices
                    .push(p::Verdict {
                        code: v.code,
                        words: v.words,
                        followups: v.followups,
                        fields: fields.into_boxed(),
                    })
                    .expect("same capacity");
            }
            p::Contract::Verdict { choices: choices.into_boxed() }
        }
    }
}

pub(super) fn propose(
    value: w::Propose,
    models: &[crate::Model],
    limits: &w::Limits,
    names: &mut List<Box<[u8]>>,
) -> Result<p::Ask, Untranslatable> {
    let digest = match value.action() {
        w::ProposeAction::Settings(_) => {
            let record = w::Ask::Propose(value.clone());
            let mut bytes = skein_lib::Writer::new(usize::try_from(record.measure()).expect("ask size"));
            record.encode(&mut bytes).expect("measured ask");
            crate::sign_in::digest(&bytes.finish()).expose().try_into().expect("32-byte digest")
        }
        w::ProposeAction::Batch(_) | w::ProposeAction::Amendment(_) => [0; 32],
    };
    let v = value.into_parts();
    let action = match v.action {
        w::ProposeAction::Settings(value) => p::Action::Settings { against: value.against(), digest },
        w::ProposeAction::Batch(value) => {
            p::Action::Batch(batch(value.tasks(), w::Path::ProposeBatchTasks, models, limits, names)?)
        }
        w::ProposeAction::Amendment(value) => {
            let value = value.into_parts();
            p::Action::Amend {
                task: value.task,
                revision: value.revision.value(),
                amendment: Box::new(asks::amendment(value.change, v.reason.clone(), models, limits, names)?),
            }
        }
    };
    Ok(p::Ask::Propose { project: v.project, action, reason: v.reason })
}
