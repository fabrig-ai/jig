//! Historical decision-race answers routed back to authenticated parties
//! (`domain/people.md`, section 5.1). These complete keyed decisions, not reads.

use crate::{Core, EscalationDecisionRecord, Event, Limits, Request, Requests};
use jig_domain_authority as authority;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_lib::{Env, Queue, Token};

#[expect(clippy::too_many_arguments, reason = "the route retains its event fields")]
pub(super) fn route_historical_escalation(
    core: &mut Core,
    env: &Env<Limits>,
    work: &mut Queue<Event>,
    request: Token,
    person: u64,
    project: u32,
    task: u64,
    revision: u64,
    row: Option<EscalationDecisionRecord>,
    busy: bool,
) -> Requests {
    let outcome = if busy {
        people::Outcome::Again(people::Again::Busy)
    } else if let Some(row) = row {
        let named = row.task == task
            && row.revision == revision
            && row.task <= core.counters.deployment().tasks
            && row.by != 0
            && row.by <= core.counters.deployment().people;
        if named {
            let bounded = match &row.decision {
                people::EscalationDecision::Release | people::EscalationDecision::Pass => true,
                people::EscalationDecision::Reject { reason } => {
                    reason.len()
                        <= usize::try_from(env.limits.escalation_reason_bytes).expect("reason bound fits usize")
                }
            };
            let private = row.project == project
                && row.requester != 0
                && row.requester <= core.counters.deployment().people
                && (row.requester == person
                    || match core.escalation_fallback(project) {
                        Some(tasks::EscalationHolder::Role { role: selected, .. }) => {
                            match core.people.role(person, project) {
                                Some(role) => role.number() == selected,
                                None => false,
                            }
                        }
                        Some(tasks::EscalationHolder::Task(_) | tasks::EscalationHolder::Person(_)) | None => false,
                    });
            if !private {
                people::Outcome::Refused(people::Refusal::Unknown)
            } else if bounded {
                let choice = match row.decision {
                    people::EscalationDecision::Release => people::EscalationChoice::Released,
                    people::EscalationDecision::Reject { .. } => people::EscalationChoice::Rejected,
                    people::EscalationDecision::Pass => people::EscalationChoice::Passed,
                };
                people::Outcome::EscalationDecided { task, revision, by: row.by, at: row.at, choice }
            } else {
                people::Outcome::Refused(people::Refusal::Unknown)
            }
        } else {
            people::Outcome::Refused(people::Refusal::Unknown)
        }
    } else if match core.tasks.task(task) {
        Some(row) => {
            row.project == project
                && core.people.role(person, project).is_some()
                && match row.phase {
                    tasks::Phase::Ended(_) => false,
                    tasks::Phase::Waiting
                    | tasks::Phase::Active(_)
                    | tasks::Phase::Closing(_)
                    | tasks::Phase::Held { .. } => true,
                }
        }
        None => false,
    } {
        people::Outcome::Refused(people::Refusal::NotWaiting)
    } else {
        people::Outcome::Refused(people::Refusal::Unknown)
    };
    work.push(Event::People(people::Event::Decided { request, outcome }));
    let mut out = Queue::with_capacity(1);
    out.push(Request::Decided);
    Requests::Out(out)
}

#[expect(clippy::too_many_arguments, reason = "the route retains its event fields")]
pub(super) fn route_historical_proposal(
    core: &mut Core,
    work: &mut Queue<Event>,
    request: Token,
    person: u64,
    project: u32,
    proposer: people::Proposer,
    proposal: u64,
    row: Option<crate::ProposalDecisionRecord>,
    busy: bool,
) -> Requests {
    let outcome = if busy {
        people::Outcome::Again(people::Again::Busy)
    } else if let Some(row) = row {
        let named = row.proposal == proposal
            && row.project == project
            && row.proposal <= core.counters.deployment().messages
            && row.by != 0
            && row.by <= core.counters.deployment().people
            && row.proposer == proposer;
        if named {
            let allowed = proposer == people::Proposer::Party(person)
                || person == row.by
                || (row.kind == tasks::ProposalKind::Settings && core.people.role(person, 0).is_some())
                || (row.kind != tasks::ProposalKind::Settings
                    && core.proposal_policy_standing(
                        person,
                        project,
                        match row.kind {
                            tasks::ProposalKind::Settings => {
                                unreachable!("settings visibility checked by deployment role")
                            }
                            tasks::ProposalKind::Batch => authority::ProposalKind::Batch,
                            tasks::ProposalKind::Effect => authority::ProposalKind::Effect,
                            tasks::ProposalKind::Amend => authority::ProposalKind::Amend,
                            tasks::ProposalKind::Widen => authority::ProposalKind::Widen,
                            tasks::ProposalKind::Release => authority::ProposalKind::Escalation,
                        },
                    ));
            let running_valid = match core.people.pending_ask(request) {
                Some(people::Ask::DecideProposal { decision, running, .. }) => {
                    let accepting = match decision {
                        people::ProposalDecision::Accept => true,
                        people::ProposalDecision::Reject { .. } | people::ProposalDecision::Pass => false,
                    };
                    running.is_some() == (row.kind == tasks::ProposalKind::Settings && accepting)
                }
                Some(_) | None => true,
            };
            if allowed && !running_valid {
                people::Outcome::Refused(people::Refusal::NotWellFormed { at: 0, why: people::Malformed::Running })
            } else if allowed {
                people::Outcome::ProposalDecided { proposer, proposal, by: row.by, at: row.at, choice: row.choice }
            } else {
                people::Outcome::Refused(people::Refusal::Unknown)
            }
        } else {
            people::Outcome::Refused(people::Refusal::Unknown)
        }
    } else {
        people::Outcome::Refused(people::Refusal::Unknown)
    };
    work.push(Event::People(people::Event::Decided { request, outcome }));
    let mut out = Queue::with_capacity(1);
    out.push(Request::Decided);
    Requests::Out(out)
}

/// Finish a withdrawal against a proposal that has left the live task state.
pub(super) fn decided_proposal(
    core: &mut Core,
    to: skein_lib::ReplyTo,
    key: crate::CallKey,
    proposal: u64,
    row: Option<tasks::Proposal>,
    busy: bool,
) -> Requests {
    let part = if busy {
        crate::CallPart::Unavailable
    } else if let Some(row) = row
        && row.number == proposal
        && row.proposer == key.task
    {
        let outcome = match row.state {
            tasks::ProposalState::Accepted { .. } => tasks::ProposalOutcome::Accepted,
            tasks::ProposalState::Rejected { .. } => tasks::ProposalOutcome::Rejected,
            tasks::ProposalState::Withdrawn => tasks::ProposalOutcome::Withdrawn,
            tasks::ProposalState::Pending { .. } => {
                return super::runs::proposal_refused(core, to, key, tasks::Refusal::Unknown);
            }
        };
        crate::CallPart::ProposalDecided { proposal, outcome }
    } else {
        return super::runs::proposal_refused(core, to, key, tasks::Refusal::Unknown);
    };
    super::runs::route_named_answer(core, to, key, part)
}
