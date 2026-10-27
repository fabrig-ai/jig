//! Same-step joins, each get and owned byte charged to the page (store.md, 7.3).
use super::{Failure, Spent, point};
use crate::Store;
use alloc::boxed::Box;
use jig_domain_core::{self as core, reads};
use jig_domain_tasks as tasks;
use skein_lib::{List, Set};

pub(super) fn as_task(value: &core::Record) -> Option<&tasks::TaskRecord> {
    match value {
        core::Record::Tasks(tasks::Stored::Live(task) | tasks::Stored::Ended(task)) => Some(task),
        core::Record::Tasks(
            tasks::Stored::PlacedHold(_)
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
            | tasks::Stored::Update(_)
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

pub(super) fn get(store: &mut Store, key: &core::Key, spent: &mut Spent) -> Result<Option<core::Record>, Failure> {
    spent.joins = spent.joins.checked_sub(1).ok_or(Failure::Joins)?;
    point::record(store, key)
}

pub(super) fn owner(
    store: &mut Store,
    task: u64,
    spent: &mut Spent,
) -> Result<Option<Box<tasks::TaskRecord>>, Failure> {
    spent.joins = spent.joins.checked_sub(1).ok_or(Failure::Joins)?;
    // A miss decodes nothing, so probing live then ended needs only one
    // join's decoded-record memory, even when neither key is found.
    for key in [tasks::Key::Live(task), tasks::Key::Ended(task)] {
        let Some(record) = point::record(store, &core::Key::Tasks(key))? else { continue };
        let Some(task) = as_task(&record) else { return Err(Failure::Corrupt) };
        return Ok(Some(Box::new(task.clone())));
    }
    Ok(None)
}

pub(super) fn activity_row(
    value: core::Record,
    owner: &tasks::TaskRecord,
    spent: &mut Spent,
) -> Result<reads::Row, Failure> {
    let row = reads::Row {
        value,
        owner: Some(Box::new(owner.clone())),
        ancestors: Box::new([]),
        related: None,
        unjoined: None,
        ended_read: None,
        linked: Box::new([]),
        inbox: None,
    };
    spent.bytes = spent.bytes.checked_sub(row.owned_bytes().ok_or(Failure::Corrupt)?).ok_or(Failure::Cut)?;
    spent.rows = spent.rows.checked_sub(1).ok_or(Failure::Cut)?;
    Ok(row)
}

pub(super) fn row(
    store: &mut Store,
    value: core::Record,
    task: Option<&tasks::TaskRecord>,
    related: Option<core::Key>,
    spent: &mut Spent,
) -> Result<reads::Row, Failure> {
    if spent.rows == 0 {
        return Err(Failure::Cut);
    }
    let mut linked = List::with_capacity(spent.joins);
    let mut ancestors = List::with_capacity(store.limits.core.tasks.depth.min(spent.joins));
    let mut seen = Set::with_capacity(spent.joins);
    if let Some(task) = task {
        let mut requester = task.requester;
        for _ in 0..store.limits.core.tasks.depth {
            let number = match requester {
                tasks::Party::Task(number) => number,
                tasks::Party::Person(_) | tasks::Party::Deployment { .. } => break,
            };
            if !seen.insert(number).or(Err(Failure::Joins))? {
                break;
            }
            let Some(parent) = owner(store, number, spent)? else { break };
            requester = parent.requester;
            ancestors.push(number).or(Err(Failure::Joins))?;
            linked.push(parent).or(Err(Failure::Joins))?;
        }
        for numbers in
            [&task.dependencies, &task.spec.inputs, &task.references, &task.delegates, &task.result_followups]
        {
            for &number in numbers {
                if seen.insert(number).or(Err(Failure::Joins))?
                    && let Some(record) = owner(store, number, spent)?
                {
                    linked.push(record).or(Err(Failure::Joins))?;
                }
            }
        }
    }
    let related = match related {
        Some(key) => get(store, &key, spent)?,
        None => None,
    };
    let mut owner = None;
    if let Some(task) = task {
        owner = Some(Box::new(task.clone()));
    }
    let row = reads::Row {
        value,
        owner,
        ancestors: ancestors.into_boxed(),
        related,
        unjoined: None,
        ended_read: None,
        linked: linked.into_boxed(),
        inbox: None,
    };
    spent.bytes = spent.bytes.checked_sub(row.owned_bytes().ok_or(Failure::Corrupt)?).ok_or(Failure::Cut)?;
    spent.rows = spent.rows.checked_sub(1).ok_or(Failure::Cut)?;
    Ok(row)
}

#[cfg(test)]
mod tests {
    use super::{Spent, owner};
    use crate::tests::fixtures::{self, ENDED, LIVE};

    #[test]
    fn every_owner_lookup_spends_exactly_one_join() {
        const MISSING: u64 = 99;
        // One lookup must use the one available join, including on a miss.
        const LOOKUP_JOINS: u32 = 1;
        let mut store = fixtures::opened();
        fixtures::committed(
            &mut store,
            Box::new([
                fixtures::header(1),
                fixtures::save(fixtures::live_task(LIVE)),
                fixtures::save(fixtures::ended_task(ENDED)),
            ]),
        );
        for (number, present) in [(LIVE, true), (ENDED, true), (MISSING, false)] {
            let mut spent = Spent { rows: 1, bytes: u64::MAX, joins: LOOKUP_JOINS };
            let Ok(found) = owner(&mut store, number, &mut spent) else { panic!("lookup fits one join: {number}") };
            assert_eq!(found.is_some(), present, "owner {number}");
            assert_eq!(spent.joins, 0, "owner {number}");
        }
    }
}
