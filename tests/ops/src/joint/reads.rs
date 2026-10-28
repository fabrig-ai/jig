//! Ops owner reads from its independent fake store.
use jig_domain_core::{self as core, documents, reads};
use jig_domain_tasks as tasks;
use jig_test_domain as root;
use std::collections::BTreeMap;
mod delegates;
pub(super) type Store = jig_fake_store::Store<root::Key, root::Record>;
fn owner(store: &Store, number: u64) -> Option<&tasks::TaskRecord> {
    for kind in [tasks::Key::Live(number), tasks::Key::Ended(number)] {
        if let Some(root::Record::Core(core::Record::Tasks(tasks::Stored::Live(value) | tasks::Stored::Ended(value)))) =
            store.rows.get(&root::Key::Core(core::Key::Tasks(kind)))
        {
            return Some(value);
        }
    }
    None
}

fn row(store: &Store, value: &core::Record, task: &tasks::TaskRecord) -> reads::Row {
    let related_key = match value {
        core::Record::Tasks(tasks::Stored::Activity(tasks::ActivityLine {
            line: tasks::Line::Turn { attempt, turn, .. },
            ..
        })) => core::CoreKey::Turn { task: task.number, attempt: *attempt, turn: *turn },
        core::Record::Core(_)
        | core::Record::Tasks(_)
        | core::Record::People(_)
        | core::Record::Notes(_)
        | core::Record::Accounts(_) => core::CoreKey::Terminal { task: task.number, attempt: task.attempt },
    };
    let related = store.rows.get(&root::Key::Core(core::Key::Core(related_key))).and_then(|record| match record {
        root::Record::Core(value) => Some(value.clone()),
        root::Record::Connector { .. } => None,
    });
    joined_row(store, value, task, related)
}

fn joined_row(
    store: &Store,
    value: &core::Record,
    task: &tasks::TaskRecord,
    related: Option<core::Record>,
) -> reads::Row {
    let mut ancestors = Vec::new();
    let mut linked = BTreeMap::new();
    let mut requester = task.requester;
    for _ in 0..64 {
        let tasks::Party::Task(number) = requester else { break };
        let Some(parent) = owner(store, number) else { break };
        ancestors.push(number);
        linked.insert(number, Box::new(parent.clone()));
        requester = parent.requester;
    }
    for number in task
        .dependencies
        .iter()
        .chain(task.spec.inputs.iter())
        .chain(task.references.iter())
        .chain(task.delegates.iter())
        .chain(task.result_followups.iter())
    {
        if let Some(value) = owner(store, *number) {
            linked.insert(*number, Box::new(value.clone()));
        }
    }
    reads::Row {
        value: value.clone(),
        owner: Some(Box::new(task.clone())),
        ancestors: ancestors.into(),
        related,
        unjoined: None,
        ended_read: None,
        linked: linked.into_values().collect(),
        inbox: None,
    }
}

fn candidate(store: &Store, range: &reads::Range, value: &core::Record) -> Option<reads::Row> {
    match &range.scope {
        reads::Scope::Task { task } | reads::Scope::Result { task } | reads::Scope::PersonTask { task } => {
            let core::Record::Tasks(tasks::Stored::Ended(value)) = value else { return None };
            if value.number != *task {
                return None;
            }
            Some(row(store, &core::Record::Tasks(tasks::Stored::Ended(value.clone())), value))
        }
        reads::Scope::Activity { task } => {
            let core::Record::Tasks(tasks::Stored::Activity(line)) = value else { return None };
            if line.task != *task {
                return None;
            }
            Some(row(store, value, owner(store, *task)?))
        }
        reads::Scope::Reply { task, activity } => {
            let core::Record::Tasks(tasks::Stored::Activity(line)) = value else { return None };
            if line.task != *task || line.position != *activity {
                return None;
            }
            Some(row(store, value, owner(store, *task)?))
        }
        reads::Scope::Inbox {
            group: reads::Group::Updates | reads::Group::Results | reads::Group::Replies, ..
        } => {
            let core::Record::Tasks(tasks::Stored::Update(update)) = value else { return None };
            if update.party != range.party {
                return None;
            }
            let Some(task) = owner(store, update.task) else {
                return Some(reads::Row {
                    value: value.clone(),
                    owner: None,
                    ancestors: Box::new([]),
                    related: None,
                    unjoined: None,
                    ended_read: None,
                    linked: Box::new([]),
                    inbox: None,
                });
            };
            let mut value = row(store, value, task);
            let object = match update.object {
                tasks::UpdateObject::Proposal { .. } => return None,
                tasks::UpdateObject::Result => documents::InboxObject::Result { position: task.result_position },
                tasks::UpdateObject::Reply { message, activity } => {
                    let record = store.rows.get(&root::Key::Core(core::Key::Tasks(tasks::Key::Activity {
                        task: update.task,
                        position: activity.value(),
                    })))?;
                    let root::Record::Core(line) = record else { return None };
                    value.related = Some(line.clone());
                    documents::InboxObject::Reply { message: message.value() }
                }
            };
            value.inbox = Some(documents::Cursor::Inbox { task: task.number, object, at: Some(update.at) });
            Some(value)
        }

        reads::Scope::Notes { .. } | reads::Scope::ScopedNotes { .. } => {
            let core::Record::Notes(jig_domain_notes::Record::Entry(_)) = value else { return None };
            Some(reads::Row {
                value: value.clone(),
                owner: None,
                ancestors: Box::new([]),
                related: None,
                unjoined: None,
                ended_read: None,
                linked: Box::new([]),
                inbox: None,
            })
        }
        reads::Scope::EndedDelegates { task } => delegates::candidate(store, *task, value),
        reads::Scope::Turn { task, attempt, turn } => {
            let core::Record::Core(core::CoreRecord::Turn(line)) = value else { return None };
            if line.task != *task || line.attempt != *attempt || line.turn != *turn {
                return None;
            }
            Some(row(store, value, owner(store, *task)?))
        }
        reads::Scope::Proposal { .. }
        | reads::Scope::Escalation { .. }
        | reads::Scope::Question { .. }
        | reads::Scope::History { .. }
        | reads::Scope::Inbox {
            group:
                reads::Group::Needs
                | reads::Group::Proposals
                | reads::Group::Escalations
                | reads::Group::Questions
                | reads::Group::PersonTasks,
            ..
        }
        | reads::Scope::SettingsHistory { .. }
        | reads::Scope::Holds { .. }
        | reads::Scope::Spend { .. }
        | reads::Scope::Tasks { .. } => None,
    }
}

fn get(store: &Store, key: core::Key) -> Option<&core::Record> {
    match store.rows.get(&root::Key::Core(key))? {
        root::Record::Core(record) => Some(record),
        root::Record::Connector { .. } => None,
    }
}

fn point(store: &Store, scope: &reads::Scope) -> Option<reads::Row> {
    let (number, key, related) = match scope {
        reads::Scope::Question { task, message } => {
            let task_row = owner(store, *task)?;
            let line = task_row.question_lines.iter().find(|line| line.message == *message)?;
            (
                *task,
                core::Key::Tasks(tasks::Key::Activity { task: *task, position: line.line }),
                line.answer.map(|answer| {
                    core::Key::Tasks(tasks::Key::Activity { task: answer.task, position: answer.position })
                }),
            )
        }
        reads::Scope::Escalation { task } => {
            let task_row = owner(store, *task)?;
            (
                *task,
                core::Key::Core(core::CoreKey::EscalationDecision {
                    task: *task,
                    revision: task_row.escalation.revision(),
                }),
                Some(core::Key::Tasks(tasks::Key::Milestone { task: *task, position: task_row.hold })),
            )
        }
        reads::Scope::Proposal { proposer: documents::Proposer::Task(task), proposal } => {
            let task_row = owner(store, *task)?;
            let hint = task_row.proposal_rows.iter().find(|row| row.proposal == *proposal)?;
            (*task, core::Key::Tasks(tasks::Key::History { task: *task, revision: hint.revision }), None)
        }
        reads::Scope::Task { .. }
        | reads::Scope::Proposal { proposer: documents::Proposer::Party(_), .. }
        | reads::Scope::PersonTask { .. }
        | reads::Scope::Result { .. }
        | reads::Scope::Reply { .. }
        | reads::Scope::Turn { .. }
        | reads::Scope::EndedDelegates { .. }
        | reads::Scope::History { .. }
        | reads::Scope::Activity { .. }
        | reads::Scope::Inbox { .. }
        | reads::Scope::Tasks { .. }
        | reads::Scope::Notes { .. }
        | reads::Scope::ScopedNotes { .. }
        | reads::Scope::SettingsHistory { .. }
        | reads::Scope::Holds { .. }
        | reads::Scope::Spend { .. } => return None,
    };
    Some(joined_row(store, get(store, key)?, owner(store, number)?, related.and_then(|key| get(store, key)).cloned()))
}

fn single_page(store: &Store, range: &reads::Range) -> reads::Loaded {
    let Some(row) = point(store, &range.scope) else { return reads::Loaded::Gone };
    let key = reads::row_key(&row).expect("single row has a key");
    if let Some(after) = &range.after
        && if range.backwards { key >= *after } else { key <= *after }
    {
        return reads::Loaded::Page { rows: Box::new([]), next: None };
    }
    if range.rows == 0 || row.owned_bytes().expect("single row pricing") > range.bytes {
        return reads::Loaded::Busy;
    }
    reads::Loaded::Page { rows: Box::new([row]), next: None }
}

/// The fake transport selects durable owner rows and echoes opaque continuations.
#[must_use]
pub fn page(store: &Store, range: &reads::Range, tree_tasks: u32) -> reads::Loaded {
    if matches!(
        range.scope,
        reads::Scope::Question { .. }
            | reads::Scope::Escalation { .. }
            | reads::Scope::Proposal { proposer: documents::Proposer::Task(_), .. }
    ) {
        return single_page(store, range);
    }
    if let reads::Scope::History { task, subtree } = range.scope {
        let Some(rows) = delegates::history(store, task, subtree, tree_tasks) else { return reads::Loaded::Busy };
        return bounded_page(range, rows);
    }
    let mut ordered = BTreeMap::new();
    let bounds = index_bounds(range);
    let bounds = match bounds {
        Some((first, last)) => {
            let first = match &range.after {
                Some(reads::Position::Owner(key))
                    if root::Key::Core(key.clone()) >= first && root::Key::Core(key.clone()) <= last =>
                {
                    std::ops::Bound::Excluded(root::Key::Core(key.clone()))
                }
                Some(reads::Position::Owner(_) | reads::Position::Inbox { .. }) | None => {
                    std::ops::Bound::Included(first)
                }
            };
            (first, std::ops::Bound::Included(last))
        }
        None => (std::ops::Bound::Unbounded, std::ops::Bound::Unbounded),
    };
    let records = store.rows.range(bounds).map(|(_, value)| value);
    for record in records {
        let root::Record::Core(value) = record else { continue };
        if let Some(row) = candidate(store, range, value) {
            ordered.insert(reads::row_key(&row).expect("owner-addressed row"), row);
        }
    }
    bounded_page(range, ordered.into_values().collect())
}

fn bounded_page(range: &reads::Range, rows: Vec<reads::Row>) -> reads::Loaded {
    let ordered: BTreeMap<_, _> = rows.into_iter().map(|row| (reads::row_key(&row).expect("row key"), row)).collect();
    let mut candidates: Vec<_> = ordered
        .into_iter()
        .filter(|(position, _)| match &range.after {
            Some(after) => {
                if range.backwards {
                    position < after
                } else {
                    position > after
                }
            }
            None => true,
        })
        .collect();
    if range.backwards {
        candidates.reverse();
    }
    let mut rows = Vec::new();
    let mut bytes = 0;
    let mut last = None;
    let mut more = false;
    for (key, row) in candidates {
        let size = row.owned_bytes().expect("owner row pricing");
        if rows.len() == range.rows as usize || size > range.bytes - bytes {
            if rows.is_empty() {
                return reads::Loaded::Busy;
            }
            more = true;
            break;
        }
        bytes += size;
        last = Some(key);
        rows.push(row);
    }
    reads::Loaded::Page { rows: rows.into(), next: if more { last } else { None } }
}

fn index_bounds(range: &reads::Range) -> Option<(root::Key, root::Key)> {
    match range.scope {
        reads::Scope::Inbox {
            group: reads::Group::Updates | reads::Group::Results | reads::Group::Replies, ..
        } => {
            let key = |at, task, object| {
                root::Key::Core(core::Key::Tasks(tasks::Key::Update {
                    party: range.party,
                    at: tasks::Newest::from_raw(at),
                    task: tasks::Newest::from_raw(task),
                    object,
                }))
            };
            Some((
                key(0, 0, tasks::UpdateObject::Result),
                key(
                    u64::MAX,
                    u64::MAX,
                    tasks::UpdateObject::Reply {
                        message: tasks::Newest::from_raw(u64::MAX),
                        activity: tasks::Newest::from_raw(u64::MAX),
                    },
                ),
            ))
        }
        reads::Scope::EndedDelegates { task } => Some(delegates::bounds(task, true)),
        reads::Scope::Task { .. }
        | reads::Scope::Proposal { .. }
        | reads::Scope::Escalation { .. }
        | reads::Scope::Question { .. }
        | reads::Scope::PersonTask { .. }
        | reads::Scope::Result { .. }
        | reads::Scope::Reply { .. }
        | reads::Scope::Turn { .. }
        | reads::Scope::History { .. }
        | reads::Scope::Activity { .. }
        | reads::Scope::Inbox {
            group:
                reads::Group::Needs
                | reads::Group::Proposals
                | reads::Group::Escalations
                | reads::Group::Questions
                | reads::Group::PersonTasks,
            ..
        }
        | reads::Scope::Notes { .. }
        | reads::Scope::ScopedNotes { .. }
        | reads::Scope::SettingsHistory { .. }
        | reads::Scope::Holds { .. }
        | reads::Scope::Spend { .. }
        | reads::Scope::Tasks { .. } => None,
    }
}
