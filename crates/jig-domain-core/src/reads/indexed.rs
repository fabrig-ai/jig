//! Physical continuations for durable indexes (store.md 7.4).
use super::{Filter, Group, Position, Scope};
use crate::documents::Proposer;
use crate::{Key, documents};
use jig_domain_tasks as tasks;

/// Continue a live window in the index's physical order.
#[must_use]
pub fn index_key(scope: &Scope, party: u64, cursor: documents::Cursor, activity: Option<u64>) -> Option<Key> {
    let Scope::Inbox { group: Group::Updates | Group::Results | Group::Replies, .. } = scope else { return None };
    let documents::Cursor::Inbox { task, at: Some(at), object } = cursor else { return None };
    let object = match object {
        documents::InboxObject::Result { .. } => tasks::UpdateObject::Result,
        documents::InboxObject::Reply { message } => {
            tasks::UpdateObject::Reply { message: tasks::Newest::of(message), activity: tasks::Newest::of(activity?) }
        }
        documents::InboxObject::Proposal { proposer: Proposer::Party(proposer), number } if proposer == party => {
            tasks::UpdateObject::Proposal { proposal: tasks::Newest::of(number) }
        }
        documents::InboxObject::Question { .. }
        | documents::InboxObject::Proposal { .. }
        | documents::InboxObject::Escalation { .. }
        | documents::InboxObject::PersonTask => return None,
    };
    Some(Key::Tasks(tasks::Update { party, at, task, object }.key()))
}

pub(super) fn inbox_position(
    party: u64,
    group: Group,
    cursor: documents::Cursor,
    activity: Option<u64>,
) -> Option<Position> {
    if group.updates() {
        let key =
            index_key(&Scope::Inbox { group, filter: Filter { project: None, goal: None } }, party, cursor, activity)?;
        return Some(Position::Owner(key));
    }
    match cursor {
        documents::Cursor::Inbox { task, object, at } => {
            let key = match group {
                Group::Updates | Group::Results | Group::Replies => return None,
                Group::Needs | Group::Proposals | Group::Escalations | Group::Questions | Group::PersonTasks => {
                    match object {
                        documents::InboxObject::Proposal { proposer: Proposer::Party(_), number } => {
                            Key::Tasks(tasks::Key::PersonProposal(number))
                        }
                        documents::InboxObject::Question { .. }
                        | documents::InboxObject::Proposal { proposer: Proposer::Task(_), .. }
                        | documents::InboxObject::Escalation { .. }
                        | documents::InboxObject::PersonTask => Key::Tasks(tasks::Key::Live(task)),
                        documents::InboxObject::Result { .. } | documents::InboxObject::Reply { .. } => return None,
                    }
                }
            };
            Some(Position::Inbox { at, task, object, key })
        }
        documents::Cursor::Number { .. } => None,
    }
}

/// Verify the index's party and its joins before projecting an update.
pub(super) fn valid_update(party: u64, row: &super::Row) -> bool {
    if proposal_position(row).is_some() {
        return valid_proposal_update(party, row);
    }
    let crate::Record::Tasks(tasks::Stored::Update(update)) = &row.value else { return false };
    let Some(owner) = &row.owner else { return false };
    if update.party != party || update.task != owner.number {
        return false;
    }
    let (object, at) = match update.object {
        tasks::UpdateObject::Proposal { .. } => return false,
        tasks::UpdateObject::Result => {
            if let Some(at) = owner.ended_at
                && at != update.at
            {
                return false;
            }
            (documents::InboxObject::Result { position: owner.result_position }, owner.ended_at)
        }
        tasks::UpdateObject::Reply { message, activity } => {
            let Some(crate::Record::Tasks(tasks::Stored::Activity(line))) = &row.related else { return false };
            if line.task != update.task || line.position != activity.value() || line.at != update.at {
                return false;
            }
            let number = match &line.line {
                tasks::Line::Turn { reply, .. } => *reply,
                tasks::Line::Notified { word, .. } => word.number,
                tasks::Line::Words { .. }
                | tasks::Line::Heard(_)
                | tasks::Line::RunBegan { .. }
                | tasks::Line::RunEnded { .. }
                | tasks::Line::Decided { .. }
                | tasks::Line::Kept { .. }
                | tasks::Line::Milestone { .. } => return false,
            };
            if number != message.value() {
                return false;
            }
            (documents::InboxObject::Reply { message: number }, Some(update.at))
        }
    };
    row.inbox == Some(documents::Cursor::Inbox { task: update.task, object, at })
}

pub(super) fn is_update(key: &Key) -> bool {
    match key {
        Key::Tasks(tasks::Key::Update { .. }) => true,
        Key::Tasks(
            tasks::Key::Live(_)
            | tasks::Key::Ended(_)
            | tasks::Key::Stub(_)
            | tasks::Key::Ledger(_)
            | tasks::Key::ClosedLedger(_)
            | tasks::Key::DecidedProposal(_)
            | tasks::Key::Writer(_)
            | tasks::Key::Pool(_)
            | tasks::Key::PersonProposal(_)
            | tasks::Key::History { .. }
            | tasks::Key::Milestone { .. }
            | tasks::Key::Activity { .. }
            | tasks::Key::Result { .. }
            | tasks::Key::EndedByParty { .. }
            | tasks::Key::EndedByProject { .. }
            | tasks::Key::EndedDelegate { .. }
            | tasks::Key::Delegate { .. },
        )
        | Key::Core(_)
        | Key::People(_)
        | Key::Notes(_)
        | Key::Accounts(_) => false,
    }
}

/// An ended-delegate index row is joined to its ended owner by task number.
pub(super) fn ended_delegate_row(
    limits: &crate::Limits,
    requester: u64,
    row: &super::Row,
) -> Result<Option<super::PageRow>, documents::Missing> {
    let edge = match &row.value {
        crate::Record::Tasks(tasks::Stored::EndedDelegate(edge)) => edge,
        crate::Record::Tasks(
            tasks::Stored::EndedByParty(_)
            | tasks::Stored::EndedByProject(_)
            | tasks::Stored::Delegate(_)
            | tasks::Stored::Update(_)
            | tasks::Stored::Result(_)
            | tasks::Stored::Activity(_)
            | tasks::Stored::Milestone(_)
            | tasks::Stored::PersonProposal(_)
            | tasks::Stored::DecidedProposal(_)
            | tasks::Stored::ClosedLedger(_)
            | tasks::Stored::History(_)
            | tasks::Stored::Live(_)
            | tasks::Stored::Ended(_)
            | tasks::Stored::Stub(_)
            | tasks::Stored::Ledger(_)
            | tasks::Stored::Writer(_)
            | tasks::Stored::Pool(_),
        )
        | crate::Record::Core(_)
        | crate::Record::People(_)
        | crate::Record::Notes(_)
        | crate::Record::Accounts(_) => return Ok(None),
    };
    let Some(owner) = &row.owner else { return Ok(None) };
    if edge.requester != requester || edge.task != owner.number || owner.requester != tasks::Party::Task(requester) {
        return Ok(None);
    }
    match owner.phase {
        tasks::Phase::Ended(_) => {}
        tasks::Phase::Waiting | tasks::Phase::Active(_) | tasks::Phase::Closing(_) | tasks::Phase::Held { .. } => {
            return Ok(None);
        }
    }
    Ok(Some(super::PageRow::EndedDelegate(documents::ended_delegate_from_row(limits, owner)?)))
}

fn valid_proposal_update(party: u64, row: &super::Row) -> bool {
    let crate::Record::Tasks(tasks::Stored::DecidedProposal(proposal)) = &row.value else { return false };
    let Some(crate::Record::Core(crate::CoreRecord::ProposalDecision(decision))) = &row.related else { return false };
    if party != proposal.proposer
        || decision.proposer != Proposer::Party(party)
        || decision.proposal != proposal.number
        || decision.project != proposal.project
    {
        return false;
    }
    let valid = match proposal.state {
        tasks::PersonProposalState::Accepted { by: tasks::Party::Person(by) }
        | tasks::PersonProposalState::SettingsAccepted { by: tasks::Party::Person(by), .. } => {
            decision.by == by && decision.choice == jig_domain_people::ProposalChoice::Accepted
        }
        tasks::PersonProposalState::Rejected { by: tasks::Party::Person(by), at, .. } => {
            decision.by == by && decision.at == at && decision.choice == jig_domain_people::ProposalChoice::Rejected
        }
        tasks::PersonProposalState::Withdrawn { at } => {
            decision.by == party && decision.at == at && decision.choice == jig_domain_people::ProposalChoice::Withdrawn
        }
        tasks::PersonProposalState::SettingsAccepted {
            by: tasks::Party::Task(_) | tasks::Party::Deployment { .. },
            ..
        }
        | tasks::PersonProposalState::Pending { .. }
        | tasks::PersonProposalState::Accepted { by: tasks::Party::Task(_) | tasks::Party::Deployment { .. } }
        | tasks::PersonProposalState::Rejected {
            by: tasks::Party::Task(_) | tasks::Party::Deployment { .. }, ..
        } => false,
    };
    valid
        && row.inbox
            == Some(documents::Cursor::Inbox {
                task: proposal.first(),
                object: documents::InboxObject::Proposal { proposer: Proposer::Party(party), number: proposal.number },
                at: Some(decision.at),
            })
}

pub(super) fn proposal_position(row: &super::Row) -> Option<Position> {
    let crate::Record::Tasks(tasks::Stored::DecidedProposal(proposal)) = &row.value else { return None };
    let Some(documents::Cursor::Inbox { at: Some(at), .. }) = row.inbox else { return None };
    Some(Position::Owner(Key::Tasks(
        tasks::Update {
            party: proposal.proposer,
            at,
            task: proposal.first(),
            object: tasks::UpdateObject::Proposal { proposal: tasks::Newest::of(proposal.number) },
        }
        .key(),
    )))
}
