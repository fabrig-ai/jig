//! Pages over durable indexes and delegate trees (protocol/store.md, 7.3–7.4).
use super::{Failure, Spent, empty, joins, paged};
use crate::{Cursor, Store, intervals, pages};
use alloc::boxed::Box;
use jig_domain_core::{self as core, documents, reads};
use jig_domain_tasks as tasks;
use skein_kv::Range;
use skein_lib::List;

fn owner_number(key: &core::Key) -> Result<u64, Failure> {
    match key {
        core::Key::Tasks(
            tasks::Key::EndedDelegate { task, .. }
            | tasks::Key::EndedByParty { task, .. }
            | tasks::Key::EndedByProject { task, .. },
        ) => Ok(*task),
        core::Key::Tasks(tasks::Key::Update { task, .. }) => Ok(task.value()),
        core::Key::Tasks(
            tasks::Key::Live(_)
            | tasks::Key::Ended(_)
            | tasks::Key::PlacedHold(_)
            | tasks::Key::Stub(_)
            | tasks::Key::Ledger(_)
            | tasks::Key::Writer(_)
            | tasks::Key::Pool(_)
            | tasks::Key::PersonProposal(_)
            | tasks::Key::ClosedLedger(_)
            | tasks::Key::DecidedProposal(_)
            | tasks::Key::History { .. }
            | tasks::Key::Milestone { .. }
            | tasks::Key::Activity { .. }
            | tasks::Key::Result { .. }
            | tasks::Key::Delegate { .. },
        )
        | core::Key::Core(_)
        | core::Key::People(_)
        | core::Key::Notes(_)
        | core::Key::Accounts(_) => Err(Failure::Corrupt),
    }
}

fn selection(scope: &reads::Scope) -> Option<(reads::Group, reads::Filter)> {
    match scope {
        reads::Scope::Inbox { group, filter } => Some((*group, *filter)),
        reads::Scope::Task { .. }
        | reads::Scope::Proposal { .. }
        | reads::Scope::Escalation { .. }
        | reads::Scope::Question { .. }
        | reads::Scope::PersonTask { .. }
        | reads::Scope::Result { .. }
        | reads::Scope::Reply { .. }
        | reads::Scope::Turn { .. }
        | reads::Scope::EndedDelegates { .. }
        | reads::Scope::History { .. }
        | reads::Scope::Activity { .. }
        | reads::Scope::Tasks { .. }
        | reads::Scope::Notes { .. }
        | reads::Scope::ScopedNotes { .. }
        | reads::Scope::SettingsHistory { .. }
        | reads::Scope::Holds { .. }
        | reads::Scope::Spend { .. } => None,
    }
}

fn history_root(scope: &reads::Scope) -> Option<u64> {
    match scope {
        reads::Scope::History { task, subtree: true } => Some(*task),
        reads::Scope::Task { .. }
        | reads::Scope::Proposal { .. }
        | reads::Scope::Escalation { .. }
        | reads::Scope::Question { .. }
        | reads::Scope::PersonTask { .. }
        | reads::Scope::Result { .. }
        | reads::Scope::Reply { .. }
        | reads::Scope::Turn { .. }
        | reads::Scope::EndedDelegates { .. }
        | reads::Scope::History { subtree: false, .. }
        | reads::Scope::Inbox { .. }
        | reads::Scope::Activity { .. }
        | reads::Scope::Tasks { .. }
        | reads::Scope::Notes { .. }
        | reads::Scope::ScopedNotes { .. }
        | reads::Scope::SettingsHistory { .. }
        | reads::Scope::Holds { .. }
        | reads::Scope::Spend { .. } => None,
    }
}

fn update(value: &core::Record) -> Option<tasks::Update> {
    match value {
        core::Record::Tasks(tasks::Stored::Update(update)) => Some(*update),
        core::Record::Tasks(
            tasks::Stored::Live(_)
            | tasks::Stored::Ended(_)
            | tasks::Stored::PlacedHold(_)
            | tasks::Stored::Stub(_)
            | tasks::Stored::Ledger(_)
            | tasks::Stored::Writer(_)
            | tasks::Stored::Pool(_)
            | tasks::Stored::PersonProposal(_)
            | tasks::Stored::ClosedLedger(_)
            | tasks::Stored::DecidedProposal(_)
            | tasks::Stored::History(_)
            | tasks::Stored::Milestone(_)
            | tasks::Stored::Activity(_)
            | tasks::Stored::Result(_)
            | tasks::Stored::EndedByParty(_)
            | tasks::Stored::EndedByProject(_)
            | tasks::Stored::EndedDelegate(_)
            | tasks::Stored::Delegate(_),
        )
        | core::Record::Core(_)
        | core::Record::People(_)
        | core::Record::Notes(_)
        | core::Record::Accounts(_) => None,
    }
}

fn delegate(value: &core::Record) -> Option<tasks::Edge> {
    match value {
        core::Record::Tasks(tasks::Stored::Delegate(edge)) => Some(*edge),
        core::Record::Tasks(
            tasks::Stored::Live(_)
            | tasks::Stored::Ended(_)
            | tasks::Stored::PlacedHold(_)
            | tasks::Stored::Stub(_)
            | tasks::Stored::Ledger(_)
            | tasks::Stored::Writer(_)
            | tasks::Stored::Pool(_)
            | tasks::Stored::PersonProposal(_)
            | tasks::Stored::ClosedLedger(_)
            | tasks::Stored::DecidedProposal(_)
            | tasks::Stored::History(_)
            | tasks::Stored::Milestone(_)
            | tasks::Stored::Activity(_)
            | tasks::Stored::Result(_)
            | tasks::Stored::EndedByParty(_)
            | tasks::Stored::EndedByProject(_)
            | tasks::Stored::EndedDelegate(_)
            | tasks::Stored::Update(_),
        )
        | core::Record::Core(_)
        | core::Record::People(_)
        | core::Record::Notes(_)
        | core::Record::Accounts(_) => None,
    }
}

fn group_keeps(group: reads::Group, object: tasks::UpdateObject) -> bool {
    match group {
        reads::Group::Updates => true,
        reads::Group::Results => object == tasks::UpdateObject::Result,
        reads::Group::Replies => match object {
            tasks::UpdateObject::Result | tasks::UpdateObject::Proposal { .. } => false,
            tasks::UpdateObject::Reply { .. } => true,
        },
        reads::Group::Needs
        | reads::Group::Proposals
        | reads::Group::Escalations
        | reads::Group::Questions
        | reads::Group::PersonTasks => false,
    }
}

fn proposal_update(
    store: &mut Store,
    range: &reads::Range,
    update: tasks::Update,
    number: u64,
    spent: &mut Spent,
) -> Result<Option<reads::Row>, Failure> {
    let Some((reads::Group::Updates, filter)) = selection(&range.scope) else {
        spent.rows = spent.rows.checked_sub(1).ok_or(Failure::Cut)?;
        return Ok(None);
    };
    let value =
        joins::get(store, &core::Key::Tasks(tasks::Key::DecidedProposal(number)), spent)?.ok_or(Failure::Corrupt)?;
    let core::Record::Tasks(tasks::Stored::DecidedProposal(proposal)) = &value else { return Err(Failure::Corrupt) };
    if proposal.proposer != range.party
        || proposal.proposer != update.party
        || proposal.number != number
        || proposal.first() != update.task
    {
        return Err(Failure::Corrupt);
    }
    if (filter.project.is_some() && filter.project != Some(proposal.project))
        || (filter.goal.is_some() && filter.goal != Some(proposal.first()))
    {
        spent.rows = spent.rows.checked_sub(1).ok_or(Failure::Cut)?;
        return Ok(None);
    }
    let proposer = documents::Proposer::Party(proposal.proposer);
    let decision = core::Key::Core(core::CoreKey::ProposalDecision { proposer, proposal: number });
    let mut row = joins::row(store, value, None, Some(decision), spent)?;
    row.inbox = Some(documents::Cursor::Inbox {
        task: update.task,
        object: documents::InboxObject::Proposal { proposer, number },
        at: Some(update.at),
    });
    Ok(Some(row))
}

fn joined(
    store: &mut Store,
    range: &reads::Range,
    value: core::Record,
    spent: &mut Spent,
) -> Result<Option<reads::Row>, Failure> {
    let number = owner_number(&value.key())?;
    let selection = selection(&range.scope);
    if let Some((group, _filter)) = selection {
        let update = update(&value).ok_or(Failure::Corrupt)?;
        match update.object {
            tasks::UpdateObject::Proposal { proposal } => {
                return proposal_update(store, range, update, proposal.value(), spent);
            }
            tasks::UpdateObject::Result | tasks::UpdateObject::Reply { .. } => {}
        }
        if !group_keeps(group, update.object) {
            spent.rows = spent.rows.checked_sub(1).ok_or(Failure::Cut)?;
            return Ok(None);
        }
    }
    let Some(owner) = joins::owner(store, number, spent)? else {
        spent.rows = spent.rows.checked_sub(1).ok_or(Failure::Cut)?;
        return Ok(None);
    };
    let mut related = None;
    let mut inbox = None;
    if let Some((_group, filter)) = selection {
        if filter.project.is_some() && filter.project != Some(owner.project) {
            spent.rows = spent.rows.checked_sub(1).ok_or(Failure::Cut)?;
            return Ok(None);
        }
        let update = update(&value).ok_or(Failure::Corrupt)?;
        let (object, at) = match update.object {
            tasks::UpdateObject::Proposal { .. } => unreachable!("party proposal joined separately"),
            tasks::UpdateObject::Result => {
                (documents::InboxObject::Result { position: owner.result_position }, owner.ended_at)
            }
            tasks::UpdateObject::Reply { message, activity } => {
                related = Some(core::Key::Tasks(tasks::Key::Activity { task: number, position: activity.value() }));
                (documents::InboxObject::Reply { message: message.value() }, Some(update.at))
            }
        };
        inbox = Some(documents::Cursor::Inbox { task: number, object, at });
    }
    let position_party = match (&range.scope, owner.requester) {
        (reads::Scope::Tasks { .. }, tasks::Party::Person(requester)) => requester,
        _ => range.party,
    };
    let ended_read = if owner.flags.conversation
        && let tasks::Phase::Ended(_) = owner.phase
    {
        match joins::get(
            store,
            &core::Key::People(jig_domain_people::Key::EndedRead { task: owner.number, party: position_party }),
            spent,
        )? {
            Some(core::Record::People(jig_domain_people::Stored::EndedRead { through, .. })) => Some(through),
            Some(_) => return Err(Failure::Corrupt),
            None => None,
        }
    } else {
        None
    };
    let mut row = joins::row(store, value, Some(&owner), related, spent)?;
    row.ended_read = ended_read;
    row.inbox = inbox;
    if let Some((_group, reads::Filter { goal: Some(goal), .. })) = selection
        && number != goal
        && !row.ancestors.contains(&goal)
    {
        return Ok(None);
    }
    Ok(Some(row))
}

fn family(
    store: &mut Store,
    range: &reads::Range,
    selected: &[Range],
    spent: &mut Spent,
) -> Result<reads::Loaded, Failure> {
    let after = paged::cursor(range, selected)?;
    let walked = pages::walk(store, selected, after.as_ref(), spent.rows).ok_or(Failure::Corrupt)?;
    let mut found = List::with_capacity(u32::try_from(walked.rows.len()).or(Err(Failure::Corrupt))?);
    let mut last = None;
    for item in walked.rows {
        let value = paged::decode(store, item.row)?;
        let position = reads::Position::Owner(value.key());
        match joined(store, range, value, spent) {
            Ok(row) => {
                if let Some(row) = row {
                    found.push(row).expect("bounded by examined index rows");
                }
                last = Some(position);
            }
            Err(Failure::Cut) => return Ok(reads::Loaded::Page { rows: found.into_boxed(), next: last }),
            Err(Failure::Corrupt) => return Err(Failure::Corrupt),
        }
    }
    Ok(reads::Loaded::Page { rows: found.into_boxed(), next: if walked.next.is_some() { last } else { None } })
}

pub(super) fn ended_delegates(
    store: &mut Store,
    range: &reads::Range,
    spent: &mut Spent,
) -> Result<reads::Loaded, Failure> {
    let selected = intervals::scope(range.party, &range.scope).ok_or(Failure::Corrupt)?;
    family(store, range, &selected, spent)
}

pub(super) fn ended_tasks(
    store: &mut Store,
    range: &reads::Range,
    spent: &mut Spent,
) -> Result<reads::Loaded, Failure> {
    let selected = intervals::scope(range.party, &range.scope).ok_or(Failure::Corrupt)?;
    family(store, range, &selected, spent)
}

pub(super) fn inbox(store: &mut Store, range: &reads::Range, spent: &mut Spent) -> Result<reads::Loaded, Failure> {
    let Some(selected) = intervals::scope(range.party, &range.scope) else { return Ok(empty()) };
    family(store, range, &selected, spent)
}

fn contains(members: &List<tasks::Edge>, task: u64) -> bool {
    for member in members {
        if member.task == task {
            return true;
        }
    }
    false
}

fn members(store: &mut Store, root: u64) -> Result<Box<[tasks::Edge]>, Failure> {
    let cap = store.limits.core.tasks.tree_tasks;
    let mut members = List::with_capacity(cap);
    members.push(tasks::Edge { requester: root, task: root }).or(Err(Failure::Cut))?;
    for index in 0..cap {
        let Some(member) = members.get(index) else { break };
        let requester = member.task;
        let selected = [intervals::delegates(requester)];
        let mut after = None;
        for _ in 0..=cap {
            let walked = pages::walk(store, &selected, after.as_ref(), cap).ok_or(Failure::Corrupt)?;
            if walked.rows.is_empty() && walked.next.is_some() {
                return Err(Failure::Cut);
            }
            for item in walked.rows {
                let record = paged::decode(store, item.row)?;
                let edge = delegate(&record).ok_or(Failure::Corrupt)?;
                if !contains(&members, edge.task) {
                    members.push(edge).or(Err(Failure::Cut))?;
                }
            }
            after = walked.next;
            if after.is_none() {
                break;
            }
        }
    }
    Ok(members.into_boxed())
}

fn resume(
    store: &mut Store,
    range: &reads::Range,
    root: u64,
    members: &[tasks::Edge],
    spent: &mut Spent,
) -> Result<usize, Failure> {
    let task = match &range.after {
        Some(reads::Position::Owner(core::Key::Tasks(
            tasks::Key::History { task, .. } | tasks::Key::Milestone { task, .. },
        ))) => *task,
        Some(reads::Position::Owner(_) | reads::Position::Inbox { .. }) => return Err(Failure::Corrupt),
        None => return Ok(0),
    };
    for (index, member) in members.iter().enumerate() {
        if member.task != task {
            continue;
        }
        if task != root {
            let key = core::Key::Tasks(tasks::Key::Delegate { requester: member.requester, task });
            let record = joins::get(store, &key, spent)?.ok_or(Failure::Corrupt)?;
            if delegate(&record) != Some(*member) {
                return Err(Failure::Corrupt);
            }
        }
        return Ok(index);
    }
    Err(Failure::Corrupt)
}

pub(super) fn subtree_history(
    store: &mut Store,
    range: &reads::Range,
    spent: &mut Spent,
) -> Result<reads::Loaded, Failure> {
    let root = history_root(&range.scope).ok_or(Failure::Corrupt)?;
    let members = members(store, root)?;
    let first = resume(store, range, root, &members, spent)?;
    let capacity = spent.rows.min(store.limits.store.page.bytes);
    let mut found = List::with_capacity(capacity);
    let mut last = None;
    let mut encoded = 0_u64;
    for (index, member) in members.iter().enumerate().skip(first) {
        let task = member.task;
        let owner = match joins::owner(store, task, spent) {
            Ok(owner) => owner,
            Err(Failure::Cut) => return Ok(reads::Loaded::Page { rows: found.into_boxed(), next: last }),
            Err(Failure::Corrupt) => return Err(Failure::Corrupt),
        };
        let selected =
            intervals::scope(range.party, &reads::Scope::History { task, subtree: false }).ok_or(Failure::Corrupt)?;
        let after: Option<Cursor> = if index == first { paged::cursor(range, &selected)? } else { None };
        let walked = pages::walk(store, &selected, after.as_ref(), spent.rows).ok_or(Failure::Corrupt)?;
        for item in walked.rows {
            let size = u64::try_from(item.row.key.len())
                .or(Err(Failure::Corrupt))?
                .checked_add(u64::try_from(item.row.value.len()).or(Err(Failure::Corrupt))?)
                .ok_or(Failure::Corrupt)?;
            encoded = encoded.checked_add(size).ok_or(Failure::Corrupt)?;
            if encoded > u64::from(store.limits.store.page.bytes) {
                return Ok(reads::Loaded::Page { rows: found.into_boxed(), next: last });
            }
            let value = paged::decode(store, item.row)?;
            let position = reads::Position::Owner(value.key());
            let row = match &owner {
                Some(owner) => joins::row(store, value, Some(owner), None, spent),
                None => {
                    spent.rows = spent.rows.checked_sub(1).ok_or(Failure::Cut)?;
                    last = Some(position);
                    continue;
                }
            };
            match row {
                Ok(row) => {
                    found.push(row).expect("bounded by remaining history rows");
                    last = Some(position);
                }
                Err(Failure::Cut) => return Ok(reads::Loaded::Page { rows: found.into_boxed(), next: last }),
                Err(Failure::Corrupt) => return Err(Failure::Corrupt),
            }
        }
        if walked.next.is_some() {
            return Ok(reads::Loaded::Page { rows: found.into_boxed(), next: last });
        }
    }
    Ok(reads::Loaded::Page { rows: found.into_boxed(), next: None })
}
