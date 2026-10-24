//! Whole historical object facts (client/domain/contract.md, 6;
//! client/domain/client.md, 5). These passive values supply no current offers.
//! Raw person goals are admitted with recurrence absent at both entry and restore;
//! their boundary translator must refuse an unadmitted recurrence before copying.
use super::{
    Bytes, Funder, ObjectKey, OwnerActivity, OwnerActivityLine, OwnerAuthority, OwnerExecutor, OwnerMessageKind,
    OwnerMessageProposalKind, OwnerMilestoneLine, OwnerNumbers, OwnerPersonTask, OwnerProposalChoice,
    OwnerProposalHistory, OwnerProposalHistoryState, OwnerRequester, OwnerWakePolicy, ProposalKey, Proposer, Question,
    Result, Specification, Task, TaskContractDocument, Text,
};
use alloc::boxed::Box;
use skein_lib::{List, Wall};

/// A raw goal holding retains kind and literal segment boundaries.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum OwnerGoalHolding {
    Write { connector: u16, path: List<Bytes>, kind: u16 },
    Slot { connector: u16, path: List<Bytes>, kind: u16 },
}
/// Whole admitted New facts, not a clipped future-task document projection.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OwnerPersonProposalGoal {
    pub number: u64,
    pub project: u32,
    pub executor: OwnerExecutor,
    pub spec: Specification,
    pub contract: TaskContractDocument,
    pub authority: OwnerAuthority,
    pub numbers: OwnerNumbers,
    pub funder: Funder,
    pub dependencies: List<u64>,
    pub holdings: List<OwnerGoalHolding>,
    pub wake: OwnerWakePolicy,
    pub tracked: Option<u32>,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OwnerPersonProposal {
    pub number: u64,
    pub proposer: u64,
    pub project: u32,
    pub goal: OwnerPersonProposalGoal,
    pub state: OwnerPersonProposalState,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum OwnerPersonProposalState {
    Pending { since: Wall },
    Accepted { by: OwnerRequester },
    Rejected { by: OwnerRequester, reason: Text, message: u64, at: Wall },
}
/// The independently retained decision, including the inclusive created pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OwnerProposalDecision {
    pub project: u32,
    pub proposer: Proposer,
    pub proposal: u64,
    pub kind: OwnerMessageProposalKind,
    pub by: u64,
    pub at: Wall,
    pub choice: OwnerProposalChoice,
    pub created: Option<(u64, u64)>,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum OwnerEscalationDecision {
    Release,
    Reject { reason: Text },
    Pass,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OwnerEscalationDecisionRecord {
    pub project: u32,
    pub requester: u64,
    pub task: u64,
    pub revision: u64,
    pub by: u64,
    pub at: Wall,
    pub decision: OwnerEscalationDecision,
}
/// Each case keeps its complete sources independently, without invented context.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum OwnerHistoricalValue {
    Task { document: Box<Task>, result: Box<Result> },
    Proposal(OwnerProposalHistory),
    PersonProposal { proposal: Box<OwnerPersonProposal>, decision: Option<OwnerProposalDecision> },
    Escalation { task: Box<Task>, decision: OwnerEscalationDecisionRecord, hold: Option<OwnerMilestoneLine> },
    Question { question: Question, answer: Option<OwnerActivityLine>, ending: Option<Box<Result>> },
    PersonTask { document: Box<OwnerPersonTask>, result: Box<Result> },
}
/// Key is the original native query identity, validated against the nested facts.
/// Outer clock and sender are independent of all inner clocks and senders.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OwnerHistorical {
    pub key: ObjectKey,
    pub value: OwnerHistoricalValue,
    pub at: Option<Wall>,
    pub by: Option<OwnerRequester>,
}
impl OwnerHistorical {
    /// Check original query, envelope and nested identities before accepting a read.
    /// Answer order is authenticated by the source translator: Question does not
    /// supply the original activity position, so the receiver cannot reconstruct it.
    #[must_use]
    pub fn matches(&self, original: ObjectKey) -> bool {
        if original != self.key {
            return false;
        }
        match &self.value {
            OwnerHistoricalValue::Task { document, result } => {
                self.key == ObjectKey::Task(document.chip.task) && result.task.task == document.chip.task
            }
            OwnerHistoricalValue::Proposal(proposal) => {
                let proposer = match proposal.action {
                    super::OwnerRetainedAction::Settings(_) => {
                        return self.key
                            == ObjectKey::Proposal(ProposalKey {
                                proposer: Proposer::Party(proposal.proposer),
                                number: proposal.number,
                            })
                            || self.key
                                == ObjectKey::Proposal(ProposalKey {
                                    proposer: Proposer::Task(proposal.proposer),
                                    number: proposal.number,
                                });
                    }
                    super::OwnerRetainedAction::Batch(_)
                    | super::OwnerRetainedAction::Effect { .. }
                    | super::OwnerRetainedAction::ProcedureEffect { .. }
                    | super::OwnerRetainedAction::Amend { .. }
                    | super::OwnerRetainedAction::Widen { .. }
                    | super::OwnerRetainedAction::Release { .. } => Proposer::Task(proposal.proposer),
                };
                self.key == ObjectKey::Proposal(ProposalKey { proposer, number: proposal.number })
            }
            OwnerHistoricalValue::PersonProposal { proposal, decision } => {
                let key = ProposalKey { proposer: Proposer::Party(proposal.proposer), number: proposal.number };
                let record_matches = match decision {
                    Some(record) => {
                        record.project == proposal.project
                            && record.proposer == key.proposer
                            && record.proposal == key.number
                    }
                    None => match &proposal.state {
                        OwnerPersonProposalState::Accepted { .. } => false,
                        OwnerPersonProposalState::Pending { .. } | OwnerPersonProposalState::Rejected { .. } => true,
                    },
                };
                self.key == ObjectKey::Proposal(key) && proposal.goal.project == proposal.project && record_matches
            }
            OwnerHistoricalValue::Escalation { task, decision, hold } => {
                let hold_matches = match hold {
                    Some(hold) => hold.task == task.chip.task,
                    None => true,
                };
                self.key == ObjectKey::Escalation(task.chip.task)
                    && decision.task == task.chip.task
                    && decision.project == task.project
                    && hold_matches
            }
            OwnerHistoricalValue::Question { question, answer, ending } => {
                let answer_matches = match answer {
                    Some(answer) => answer.task == question.task.task && answers_question(answer, question.message),
                    None => true,
                };
                let ending_matches = match ending {
                    Some(ending) => ending.task.task == question.task.task,
                    None => true,
                };
                self.key == ObjectKey::Question { task: question.task.task, message: question.message }
                    && answer_matches
                    && ending_matches
            }
            OwnerHistoricalValue::PersonTask { document, result } => {
                self.key == ObjectKey::PersonTask(document.task.task) && result.task.task == document.task.task
            }
        }
    }
    /// Classification of supplied facts, not a retention duration or current act.
    /// Pending source-less objects still follow ordinary prune after read receipt.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        match &self.value {
            OwnerHistoricalValue::Task { .. }
            | OwnerHistoricalValue::PersonTask { .. }
            | OwnerHistoricalValue::Escalation { .. } => true,
            OwnerHistoricalValue::Proposal(proposal) => match &proposal.state {
                OwnerProposalHistoryState::Pending { .. } => false,
                OwnerProposalHistoryState::Accepted { .. }
                | OwnerProposalHistoryState::Rejected { .. }
                | OwnerProposalHistoryState::Withdrawn => true,
            },
            OwnerHistoricalValue::PersonProposal { proposal, .. } => match &proposal.state {
                OwnerPersonProposalState::Pending { .. } => false,
                OwnerPersonProposalState::Accepted { .. } | OwnerPersonProposalState::Rejected { .. } => true,
            },
            OwnerHistoricalValue::Question { answer, ending, .. } => answer.is_some() || ending.is_some(),
        }
    }
}
fn answers_question(answer: &OwnerActivityLine, message: u64) -> bool {
    match &answer.line {
        OwnerActivity::Words { kind, .. } => match kind {
            OwnerMessageKind::Answer { question } => *question == message,
            OwnerMessageKind::Escalation { .. }
            | OwnerMessageKind::Proposal { .. }
            | OwnerMessageKind::PersonStalled { .. }
            | OwnerMessageKind::ProposalDecision { .. }
            | OwnerMessageKind::Words
            | OwnerMessageKind::EffectNotMade { .. }
            | OwnerMessageKind::Met { .. }
            | OwnerMessageKind::Amendment { .. }
            | OwnerMessageKind::Question
            | OwnerMessageKind::Notice { .. }
            | OwnerMessageKind::Timer { .. }
            | OwnerMessageKind::News { .. }
            | OwnerMessageKind::Result(_) => false,
        },
        OwnerActivity::Heard(_)
        | OwnerActivity::Notified { .. }
        | OwnerActivity::RunBegan { .. }
        | OwnerActivity::RunEnded { .. }
        | OwnerActivity::Turn { .. }
        | OwnerActivity::Decided { .. }
        | OwnerActivity::Kept { .. }
        | OwnerActivity::Milestone { .. } => false,
    }
}
