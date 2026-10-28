//! Whole single-read documents, exhaustive over the core vocabulary (protocol/client.md, 3 and 6).
#![expect(clippy::manual_map, reason = "step code matches optional owner facts without closures")]
use super::{deployment, owner, task, w};
use alloc::boxed::Box;
use jig_domain_core::{documents as d, reads};
mod current;
pub(super) mod goal;
pub(super) mod historical;
mod projects;
pub(super) mod proposals;
pub(crate) mod question_answer;
pub use current::{escalation, person_task, question, result};
pub use projects::project;
pub use proposals::proposal;
pub fn historical(
    value: &reads::Historical,
    models: &[crate::Model],
    limits: &w::Limits,
) -> Result<w::OwnerHistorical, w::Problem> {
    let key = historical_key(&value.value, limits)?;
    let record = match &value.value {
        reads::Past::Task { document, result } => w::OwnerHistoricalValue::Task(w::OwnerHistoricalValueTask::new(
            limits,
            w::OwnerHistoricalValueTaskParts {
                document: task::document(document, models, limits)?,
                result: current::result(models, result, limits)?,
            },
        )?),
        reads::Past::Proposal(value) => w::OwnerHistoricalValue::Proposal(w::OwnerHistoricalValueProposal::new(
            limits,
            w::OwnerHistoricalValueProposalParts { value: historical::proposal_history(models, value, limits)? },
        )?),
        reads::Past::PersonProposal { proposal, .. }
            if match proposal.action {
                jig_domain_tasks::PersonAction::Settings { .. } => true,
                jig_domain_tasks::PersonAction::Batch(_)
                | jig_domain_tasks::PersonAction::Amend { .. }
                | jig_domain_tasks::PersonAction::ReleasePlaced { .. } => false,
            } =>
        {
            w::OwnerHistoricalValue::Proposal(w::OwnerHistoricalValueProposal::new(
                limits,
                w::OwnerHistoricalValueProposalParts { value: historical::settings_history(models, proposal, limits)? },
            )?)
        }
        reads::Past::PersonProposal { proposal, decision } => {
            w::OwnerHistoricalValue::PersonProposal(w::OwnerHistoricalValuePersonProposal::new(
                limits,
                w::OwnerHistoricalValuePersonProposalParts {
                    proposal: historical::person_proposal(models, proposal, limits)?,
                    decision: match *decision {
                        Some(item) => Some(historical::proposal_decision(item, limits)?),
                        None => None,
                    },
                },
            )?)
        }
        reads::Past::Escalation { task: document, decision, hold } => {
            w::OwnerHistoricalValue::Escalation(w::OwnerHistoricalValueEscalation::new(
                limits,
                w::OwnerHistoricalValueEscalationParts {
                    task: task::document(document, models, limits)?,
                    decision: historical::escalation_decision(decision, limits)?,
                    hold: match hold {
                        Some(item) => Some(historical::milestone(item, limits)?),
                        None => None,
                    },
                },
            )?)
        }
        reads::Past::Question { question, answer, ending } => {
            w::OwnerHistoricalValue::Question(w::OwnerHistoricalValueQuestion::new(
                limits,
                w::OwnerHistoricalValueQuestionParts {
                    question: current::question(models, question, limits)?,
                    answer: match answer {
                        Some(item) => Some(question_answer::answer(item, limits)?),
                        None => None,
                    },
                    ending: match ending {
                        Some(item) => Some(result(models, item, limits)?),
                        None => None,
                    },
                },
            )?)
        }
        reads::Past::PersonTask { document, result } => {
            w::OwnerHistoricalValue::PersonTask(w::OwnerHistoricalValuePersonTask::new(
                limits,
                w::OwnerHistoricalValuePersonTaskParts {
                    document: person_task(models, document, limits)?,
                    result: current::result(models, result, limits)?,
                },
            )?)
        }
        reads::Past::Reply(_) | reads::Past::Turn(_) => {
            return Err(w::Problem { path: w::Path::OwnerHistoricalValue, reason: skein_codec::Reason::Tag });
        }
    };
    w::OwnerHistorical::new(
        limits,
        w::OwnerHistoricalParts {
            key,
            value: record,
            at: match value.at {
                Some(item) => Some(item.as_nanos()),
                None => None,
            },
            by: match value.by {
                Some(item) => Some(owner::requester(item, limits)?),
                None => None,
            },
        },
    )
}
pub fn document(
    answer: &reads::Answer,
    models: &[crate::Model],
    limits: &w::Limits,
) -> Result<w::ReadValue, w::Problem> {
    Ok(match answer {
        reads::Answer::Document(value) => match value.as_ref() {
            d::Document::Frame(value) => w::ReadValue::Me(owner::frame(value, models, limits)?),
            d::Document::Task(value) => w::ReadValue::Task(task::document(value, models, limits)?),
            d::Document::Proposal(value) => w::ReadValue::OwnerProposal(proposal(models, value, limits)?),
            d::Document::Escalation(value) => w::ReadValue::OwnerEscalation(escalation(value, models, limits)?),
            d::Document::Question(value) => w::ReadValue::Question(question(models, value, limits)?),
            d::Document::PersonTask(value) => w::ReadValue::OwnerPersonTask(person_task(models, value, limits)?),
            d::Document::Result(value) => w::ReadValue::Result(result(models, value, limits)?),
            d::Document::Project(value) => w::ReadValue::OwnerProject(project(models, value, limits)?),
            d::Document::Settings(value) => w::ReadValue::Settings(super::settings::document(value, limits)?),
            d::Document::Deployment(value) => w::ReadValue::OwnerDeployment(deployment::document(value, limits)?),
            d::Document::Chips(_)
            | d::Document::Run(_)
            | d::Document::Tree(_)
            | d::Document::Lines(_)
            | d::Document::Accounts(_)
            | d::Document::Note(_)
            | d::Document::Inbox(_) => {
                return Err(w::Problem { path: w::Path::ReadValueTag, reason: skein_codec::Reason::Tag });
            }
        },
        reads::Answer::Historical(value) => match &value.value {
            reads::Past::Turn(record) => w::ReadValue::Turn(super::activity::archive(record, limits)?),
            reads::Past::Task { .. }
            | reads::Past::Proposal(_)
            | reads::Past::PersonProposal { .. }
            | reads::Past::Escalation { .. }
            | reads::Past::Question { .. }
            | reads::Past::PersonTask { .. }
            | reads::Past::Reply(_) => w::ReadValue::Historical(Box::new(historical(value, models, limits)?)),
        },
        reads::Answer::Holds(_)
        | reads::Answer::Setting(_)
        | reads::Answer::Revision(_)
        | reads::Answer::Revisions(_)
        | reads::Answer::Tasks(_)
        | reads::Answer::Page(_)
        | reads::Answer::None(_) => {
            return Err(w::Problem { path: w::Path::ReadValueTag, reason: skein_codec::Reason::Tag });
        }
    })
}

fn historical_key(value: &reads::Past, limits: &w::Limits) -> Result<w::ObjectKey, w::Problem> {
    Ok(match value {
        reads::Past::Task { document, .. } => {
            w::ObjectKey::Task(w::ObjectKeyTask::new(limits, w::ObjectKeyTaskParts { value: document.chip.number })?)
        }
        reads::Past::Proposal(value) => w::ObjectKey::Proposal(w::ObjectKeyProposal::new(
            limits,
            w::ObjectKeyProposalParts {
                value: task::proposal(
                    &d::ProposalRef { proposer: d::Proposer::Task(value.proposer), number: value.number },
                    limits,
                )?,
            },
        )?),
        reads::Past::PersonProposal { proposal, .. } => w::ObjectKey::Proposal(w::ObjectKeyProposal::new(
            limits,
            w::ObjectKeyProposalParts {
                value: task::proposal(
                    &d::ProposalRef { proposer: d::Proposer::Party(proposal.proposer), number: proposal.number },
                    limits,
                )?,
            },
        )?),
        reads::Past::Escalation { task: document, .. } => w::ObjectKey::Escalation(w::ObjectKeyEscalation::new(
            limits,
            w::ObjectKeyEscalationParts { value: document.chip.number },
        )?),
        reads::Past::Question { question, .. } => w::ObjectKey::Question(w::ObjectKeyQuestion::new(
            limits,
            w::ObjectKeyQuestionParts { task: question.task.number, message: question.message },
        )?),
        reads::Past::PersonTask { document, .. } => w::ObjectKey::PersonTask(w::ObjectKeyPersonTask::new(
            limits,
            w::ObjectKeyPersonTaskParts { value: document.task.number },
        )?),
        reads::Past::Reply(_) | reads::Past::Turn(_) => {
            return Err(w::Problem { path: w::Path::OwnerHistoricalValue, reason: skein_codec::Reason::Tag });
        }
    })
}
