//! Proposal and escalation choices use their current waiting holder.
use crate::acts::Act;
use crate::{Core, Limits, documents};
use jig_domain_people as people;
use jig_domain_tasks as tasks;

#[expect(clippy::too_many_lines, reason = "both origin families share the exhaustive closed decision vocabulary")]
pub(crate) fn proposal(
    core: &Core,
    limits: &Limits,
    party: u64,
    act: Act,
    proposer: documents::Proposer,
    number: u64,
    decision: skein_lib::Wall,
) -> Result<(), people::Refusal> {
    match proposer {
        documents::Proposer::Party(person) => {
            let row = core.tasks.person_proposal(person, number).ok_or(people::Refusal::Unknown)?;
            if act == Act::Withdraw {
                return match row.state {
                    tasks::PersonProposalState::Pending { .. } if person == party => Ok(()),
                    tasks::PersonProposalState::Pending { .. }
                    | tasks::PersonProposalState::SettingsAccepted { .. }
                    | tasks::PersonProposalState::Accepted { .. }
                    | tasks::PersonProposalState::Rejected { .. }
                    | tasks::PersonProposalState::Withdrawn { .. } => Err(people::Refusal::NotOffered),
                };
            }
            let role = core.people.role(party, row.project);
            core.goal_standing(&row, row.project, role)?;
            match &row.state {
                tasks::PersonProposalState::Pending { .. } => {}
                tasks::PersonProposalState::SettingsAccepted { .. }
                | tasks::PersonProposalState::Accepted { .. }
                | tasks::PersonProposalState::Rejected { .. }
                | tasks::PersonProposalState::Withdrawn { .. } => {
                    return Err(people::Refusal::Unknown);
                }
            }
            match act {
                Act::Accept => {
                    if row.action.is_settings() {
                        return Ok(());
                    }
                    if let tasks::PersonAction::Batch(_) = &row.action
                        && core.projections.len() >= limits.tasks.tasks
                    {
                        return Err(people::Refusal::Limit { which: people::LimitKind::LiveTasks });
                    }
                    core.goal_accept_allowed(
                        &row,
                        row.project,
                        party,
                        role.expect("standing found role"),
                        limits.tasks.tree_tasks,
                        decision,
                    )?;
                    Ok(())
                }
                Act::Reject => Ok(()),
                Act::PassUp => Err(people::Refusal::NoFurther),
                Act::Withdraw
                | Act::Message
                | Act::Answer
                | Act::Take
                | Act::HandBack
                | Act::Release
                | Act::LeaveHeld
                | Act::Amend
                | Act::Stop
                | Act::Cancel
                | Act::Move
                | Act::Reorder
                | Act::Seen
                | Act::EditNote
                | Act::DeleteNote
                | Act::Adopt
                | Act::Rename
                | Act::ChangePolicy
                | Act::SetRoles
                | Act::MakeService
                | Act::SetPool => Err(people::Refusal::NotOffered),
            }
        }
        documents::Proposer::Task(task) => {
            let row = core.tasks.proposal(task, number).ok_or(people::Refusal::Unknown)?;
            let role = core.people.role(party, row.project);
            let current = match row.state {
                tasks::ProposalState::Pending { holder, .. } => holder,
                tasks::ProposalState::Accepted { .. }
                | tasks::ProposalState::Rejected { .. }
                | tasks::ProposalState::Withdrawn => return Err(people::Refusal::Unknown),
            };
            if let tasks::ProposalAction::Settings { .. } = row.action {
                if core.people.role(party, people::DEPLOYMENT) != Some(people::Role::Owner) {
                    return Err(people::Refusal::NotWaiting);
                }
                return match act {
                    Act::Accept | Act::Reject => Ok(()),
                    Act::PassUp => Err(people::Refusal::NoFurther),
                    Act::Withdraw
                    | Act::Message
                    | Act::Answer
                    | Act::Take
                    | Act::HandBack
                    | Act::Release
                    | Act::LeaveHeld
                    | Act::Amend
                    | Act::Stop
                    | Act::Cancel
                    | Act::Move
                    | Act::Reorder
                    | Act::Seen
                    | Act::EditNote
                    | Act::DeleteNote
                    | Act::Adopt
                    | Act::Rename
                    | Act::ChangePolicy
                    | Act::SetRoles
                    | Act::MakeService
                    | Act::SetPool => Err(people::Refusal::NotOffered),
                };
            }
            let (_, kind) = Core::proposal_kind(&row.action);
            let kind = kind.expect("ordinary proposal authority kind");
            let waiting = match current {
                tasks::ProposalHolder::Person(holder) => holder == party,
                tasks::ProposalHolder::Policy { project, .. } => {
                    project == row.project && core.proposal_policy_standing(party, row.project, kind)
                }
                tasks::ProposalHolder::Task(_) => false,
            };
            if !waiting || role.is_none() {
                return Err(people::Refusal::NotWaiting);
            }
            match act {
                Act::Accept => {
                    let action = core
                        .proposal_action_for_check(&row.action, decision)
                        .ok_or(people::Refusal::Authority { proposable: None, lacking: people::Lack::Shape })?;
                    core.proposal_person_admit(
                        party,
                        row.project,
                        role.expect("standing found role"),
                        action,
                        limits.tasks.tree_tasks,
                    )
                }
                Act::Reject => Ok(()),
                Act::PassUp => match current {
                    tasks::ProposalHolder::Person(_) => Ok(()),
                    tasks::ProposalHolder::Policy { .. } => Err(people::Refusal::NoFurther),
                    tasks::ProposalHolder::Task(_) => unreachable!("party standing checked"),
                },
                Act::Withdraw
                | Act::Message
                | Act::Answer
                | Act::Take
                | Act::HandBack
                | Act::Release
                | Act::LeaveHeld
                | Act::Amend
                | Act::Stop
                | Act::Cancel
                | Act::Move
                | Act::Reorder
                | Act::Seen
                | Act::EditNote
                | Act::DeleteNote
                | Act::Adopt
                | Act::Rename
                | Act::ChangePolicy
                | Act::SetRoles
                | Act::MakeService
                | Act::SetPool => Err(people::Refusal::NotOffered),
            }
        }
    }
}

pub(crate) fn escalation(
    core: &Core,
    limits: &Limits,
    party: u64,
    act: Act,
    number: u64,
) -> Result<(), people::Refusal> {
    let context = core.tasks.escalation(number).ok_or(people::Refusal::Unknown)?;
    let role = core.people.role(party, context.project);
    if !core.escalation_visible(&context, party, role) {
        return Err(people::Refusal::Unknown);
    }
    match context.escalation {
        tasks::Escalation::Waiting { .. } => {}
        tasks::Escalation::Unheld { .. } | tasks::Escalation::Routing { .. } | tasks::Escalation::Rejected { .. } => {
            return Err(people::Refusal::NotWaiting);
        }
    }
    if !Core::escalation_standing(&context, party, role) {
        return Err(people::Refusal::NotWaiting);
    }
    match act {
        Act::Release => {
            if !core.escalation_covers(limits, &context, party, role) {
                return Err(people::Refusal::Role { held: role });
            }
            core.escalation_release_allowed(&context, party, role, limits.tasks.tree_tasks)
        }
        Act::LeaveHeld => Ok(()),
        Act::PassUp => {
            if core.escalation_fallback(context.project).is_some() {
                Ok(())
            } else {
                Err(people::Refusal::NoFurther)
            }
        }
        Act::Message
        | Act::Answer
        | Act::Take
        | Act::HandBack
        | Act::Withdraw
        | Act::Accept
        | Act::Reject
        | Act::Amend
        | Act::Stop
        | Act::Cancel
        | Act::Move
        | Act::Reorder
        | Act::Seen
        | Act::EditNote
        | Act::DeleteNote
        | Act::Adopt
        | Act::Rename
        | Act::ChangePolicy
        | Act::SetRoles
        | Act::MakeService
        | Act::SetPool => Err(people::Refusal::NotOffered),
    }
}
