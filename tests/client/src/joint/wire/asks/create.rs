//! The direct joint world's independent batch translation.
use super::{authority, client as c, people as p, request_arguments, specification};
use skein_lib::List;
pub(super) fn ask(
    project: u32,
    values: &List<c::NewTask>,
    models: &[jig_domain_core::Model],
    names: &mut List<Box<[u8]>>,
) -> Result<p::Ask, super::Invalid> {
    Ok(p::Ask::Create {
        project,
        batch: values.iter().map(|value| member(value, models, names)).collect::<Result<_, _>>()?,
    })
}
fn member(
    v: &c::NewTask,
    models: &[jig_domain_core::Model],
    names: &mut List<Box<[u8]>>,
) -> Result<p::Member, super::Invalid> {
    let spec = specification(&v.spec);
    Ok(p::Member {
        executor: match v.executor {
            c::OwnerExecutor::Agent { charter } => p::TaskExecutor::Agent { charter },
            c::OwnerExecutor::Procedure { connector, code } => p::TaskExecutor::Procedure { connector, code },
            c::OwnerExecutor::Person(address) => p::TaskExecutor::Person(match address {
                c::PersonAddress::Party(v) => p::PersonAddress::Person(v),
                c::PersonAddress::Role(v) => p::PersonAddress::Role(v),
            }),
        },
        words: spec.words,
        parameters: spec.parameters,
        inputs: spec.inputs,
        authority: authority(&v.authority, names)?,
        deadline: match v.deadline {
            c::NewDeadline::Named => p::Deadline::Named,
            c::NewDeadline::Span(value) => p::Deadline::Span(value),
        },
        budget: match v.budget {
            c::NewBudget::Named => p::Budget::Named,
            c::NewBudget::AsAllowed { cap } => p::Budget::AsAllowed { cap },
        },
        contract: match &v.contract {
            c::NewContract::Report { words } => p::Contract::Report { words: *words },
            c::NewContract::Change { connector, kind, words } => {
                p::Contract::Change { connector: *connector, kind: *kind, words: *words }
            }
            c::NewContract::Verdict { choices } => p::Contract::Verdict {
                choices: choices
                    .iter()
                    .map(|v| p::Verdict {
                        code: v.code,
                        words: v.words,
                        followups: v.followups,
                        fields: v.fields.iter().map(|f| p::FieldRule { name: f.name.clone(), max: f.max }).collect(),
                    })
                    .collect(),
            },
        },
        wake: match v.wake {
            Some(wake) => request_arguments::wake(wake),
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
        dependencies: v
            .dependencies
            .iter()
            .map(|v| match v {
                c::TaskRef::Member(v) => p::Dependency::Batch(*v),
                c::TaskRef::Task(v) => p::Dependency::Existing(*v),
            })
            .collect(),
        named: v.named.iter().map(super::grant).collect(),
        tracked: v.tracked,
        label: v.label,
        flags: p::Flags {
            ended_by: match v.flags.ended_by {
                c::EndedBy::Result => p::EndedBy::Result,
                c::EndedBy::Requester => p::EndedBy::Requester,
            },
            conversation: v.flags.conversation,
            listed: v.flags.listed,
            retention: match v.flags.retention {
                c::Retention::Kept => p::Retention::Kept,
                c::Retention::ForgottenOnceRead => p::Retention::ForgottenOnceRead,
            },
        },
        llm: super::super::llm::requested(v.llm.as_ref(), models)?,
    })
}

pub(super) fn propose(
    project: u32,
    action: &c::ProposeAction,
    reason: &c::Text,
    models: &[jig_domain_core::Model],
    names: &mut List<Box<[u8]>>,
) -> Result<p::Ask, super::Invalid> {
    let action = match action {
        c::ProposeAction::ReleasePlaced { task, hold, revision } => {
            p::Action::ReleasePlaced { task: *task, hold: *hold, revision: *revision }
        }
        c::ProposeAction::Settings(_) => return Err(super::Invalid::Settings),
        c::ProposeAction::Batch { tasks } => {
            p::Action::Batch(tasks.iter().map(|value| member(value, models, names)).collect::<Result<_, _>>()?)
        }
        c::ProposeAction::Amendment { task, revision, change } => p::Action::Amend {
            task: *task,
            revision: revision.value,
            amendment: Box::new(super::amend(change, reason, models, names)?),
        },
    };
    Ok(p::Ask::Propose { project, action, reason: reason.bytes().into() })
}
