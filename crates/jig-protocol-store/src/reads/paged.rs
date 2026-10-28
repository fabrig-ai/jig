//! Bounded family pages and dense backwards activity (protocol/store.md, 7.3).
use super::{Failure, Spent, empty, joins};
use crate::{Cursor, Row, Store, intervals, keys, pages};
use jig_domain_core::{self as core, reads};
use jig_domain_notes as notes;
use jig_domain_tasks as tasks;
use skein_kv::Range;
use skein_lib::List;

pub(super) fn backwards_interval(task: u64, last: u64, before: Option<u64>, rows: u32) -> Range {
    let high = match before {
        Some(before) => last.min(before.saturating_sub(1)),
        None => last,
    };
    let end = intervals::point(&core::Key::Tasks(tasks::Key::Activity { task, position: high }));
    if rows == 0 {
        return Range { start: end.start.clone(), end: Some(end.start) };
    }
    let low = high.saturating_sub(u64::from(rows.saturating_sub(1))).max(1);
    let start = keys::encode(&core::Key::Tasks(tasks::Key::Activity { task, position: low }), u32::MAX)
        .expect("fixed-width activity key fits");
    Range { start, end: end.end }
}

fn activity_before(range: &reads::Range, task: u64) -> Result<Option<u64>, Failure> {
    match &range.after {
        Some(reads::Position::Owner(core::Key::Tasks(tasks::Key::Activity { task: owner, position })))
            if *owner == task =>
        {
            Ok(Some(*position))
        }
        Some(reads::Position::Owner(_) | reads::Position::Inbox { .. }) => Err(Failure::Corrupt),
        None => Ok(None),
    }
}

pub(super) fn activity(
    store: &mut Store,
    range: &reads::Range,
    task: u64,
    spent: &mut Spent,
) -> Result<reads::Loaded, Failure> {
    let Some(owner) = joins::owner(store, task, spent)? else { return Ok(empty()) };
    let capacity = spent.rows.max(1).min(store.limits.store.page.bytes);
    let before = activity_before(range, task)?;
    let interval = backwards_interval(task, owner.activity, before, capacity);
    let high = match before {
        Some(before) => owner.activity.min(before.saturating_sub(1)),
        None => owner.activity,
    };
    let mut found = List::with_capacity(capacity);
    // An exclusive cursor just before this window retries its first row when
    // a bound prevents taking even that row. Overflow leaves no continuation,
    // as before; it is unreachable because activity counts committed rows.
    let start = match before {
        Some(before) => Some(before),
        None => owner.activity.checked_add(1),
    };
    let mut last = None;
    if let Some(position) = start {
        last = Some(reads::Position::Owner(core::Key::Tasks(tasks::Key::Activity { task, position })));
    }
    let mut encoded = 0_u64;
    let mut more = high.saturating_sub(u64::from(capacity)) > 0;
    // Walk only the requested dense window, with exact forward intervals in
    // descending position order. Reversing an ascending encoded prefix would
    // lose its newest rows when the encoded-byte bound cuts that prefix.
    for offset in 0..capacity {
        let position = high.saturating_sub(u64::from(offset));
        let key = core::Key::Tasks(tasks::Key::Activity { task, position });
        let point = intervals::point(&key);
        if point.start < interval.start {
            break;
        }
        let walked = pages::walk(store, &[point], None, 1).ok_or(Failure::Corrupt)?;
        let mut rows = walked.rows.into_iter();
        let Some(item) = rows.next() else {
            if walked.next.is_some() {
                more = true;
                break;
            }
            continue;
        };
        let size = u64::try_from(item.row.key.len())
            .or(Err(Failure::Corrupt))?
            .checked_add(u64::try_from(item.row.value.len()).or(Err(Failure::Corrupt))?)
            .ok_or(Failure::Corrupt)?;
        encoded = encoded.checked_add(size).ok_or(Failure::Corrupt)?;
        if encoded > u64::from(store.limits.store.page.bytes) {
            more = true;
            break;
        }
        let value = decode(store, item.row)?;
        if let core::Record::Tasks(tasks::Stored::Activity(line)) = &value
            && let tasks::Line::Turn { .. } = line.line
        {
            if spent.joins == 0 {
                more = true;
                break;
            }
            spent.joins = spent.joins.checked_sub(1).expect("turn join checked");
        }
        match joins::activity_row(value, &owner, spent) {
            Ok(row) => {
                last = Some(reads::Position::Owner(key));
                found.push(row).expect("bounded activity window");
            }
            Err(Failure::Cut) => {
                more = true;
                break;
            }
            Err(Failure::Corrupt) => return Err(Failure::Corrupt),
        }
    }
    Ok(reads::Loaded::Page { rows: found.into_boxed(), next: if more { last } else { None } })
}

pub(super) fn decode(store: &Store, row: skein_kv::Row) -> Result<core::Record, Failure> {
    let (row, _bytes) = pages::decode(store, row).ok_or(Failure::Corrupt)?;
    match row {
        Row::Jig(record) => Ok(record),
        Row::Application { .. } => Err(Failure::Corrupt),
    }
}

pub(super) fn cursor(range: &reads::Range, intervals: &[Range]) -> Result<Option<Cursor>, Failure> {
    let key = match &range.after {
        Some(reads::Position::Owner(key)) => keys::encode(key, u32::MAX).ok_or(Failure::Corrupt)?,
        Some(reads::Position::Inbox { .. }) => return Err(Failure::Corrupt),
        None => return Ok(None),
    };
    for (index, interval) in intervals.iter().enumerate() {
        if key >= interval.start
            && match &interval.end {
                Some(end) => key < *end,
                None => true,
            }
        {
            return Ok(Some(Cursor { interval: u32::try_from(index).or(Err(Failure::Corrupt))?, after: key }));
        }
    }
    Err(Failure::Corrupt)
}

fn family(
    store: &mut Store,
    range: &reads::Range,
    owner: Option<&tasks::TaskRecord>,
    spent: &mut Spent,
) -> Result<reads::Loaded, Failure> {
    let intervals = intervals::scope(range.party, &range.scope).ok_or(Failure::Corrupt)?;
    let after = cursor(range, &intervals)?;
    let walked = pages::walk(store, &intervals, after.as_ref(), spent.rows).ok_or(Failure::Corrupt)?;
    let mut found = List::with_capacity(u32::try_from(walked.rows.len()).or(Err(Failure::Corrupt))?);
    let mut last = None;
    for item in walked.rows {
        let value = decode(store, item.row)?;
        let position = reads::Position::Owner(value.key());
        match joins::row(store, value, owner, None, spent) {
            Ok(row) => {
                last = Some(position);
                found.push(row).expect("bounded by walked rows");
            }
            Err(Failure::Cut) => return Ok(reads::Loaded::Page { rows: found.into_boxed(), next: last }),
            Err(Failure::Corrupt) => return Err(Failure::Corrupt),
        }
    }
    Ok(reads::Loaded::Page { rows: found.into_boxed(), next: if walked.next.is_some() { last } else { None } })
}

pub(super) fn history(
    store: &mut Store,
    range: &reads::Range,
    task: u64,
    spent: &mut Spent,
) -> Result<reads::Loaded, Failure> {
    let Some(owner) = joins::owner(store, task, spent)? else { return Ok(empty()) };
    family(store, range, Some(&owner), spent)
}

pub(super) fn spend(store: &mut Store, range: &reads::Range, spent: &mut Spent) -> Result<reads::Loaded, Failure> {
    family(store, range, None, spent)
}

pub(super) fn notes(store: &mut Store, range: &reads::Range, spent: &mut Spent) -> Result<reads::Loaded, Failure> {
    let intervals = intervals::scope(range.party, &range.scope).ok_or(Failure::Corrupt)?;
    let after = cursor(range, &intervals)?;
    let walked = pages::walk(store, &intervals, after.as_ref(), spent.rows).ok_or(Failure::Corrupt)?;
    let mut found = List::with_capacity(u32::try_from(walked.rows.len()).or(Err(Failure::Corrupt))?);
    let mut last = None;
    for item in walked.rows {
        let line = match decode(store, item.row)? {
            core::Record::Notes(notes::Record::Line(line)) => line,
            core::Record::Notes(notes::Record::Entry(_))
            | core::Record::Core(_)
            | core::Record::Tasks(_)
            | core::Record::People(_)
            | core::Record::Accounts(_) => return Err(Failure::Corrupt),
        };
        let position =
            reads::Position::Owner(core::Key::Notes(notes::Key::Line { scope: line.scope.clone(), name: line.name }));
        let selected = match &range.scope {
            reads::Scope::ScopedNotes { description: Some(query), .. } => {
                skein_lib::bytes::find(&line.description, query).is_some()
            }
            reads::Scope::ScopedNotes { description: None, .. } | reads::Scope::Notes { .. } => true,
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
            | reads::Scope::Inbox { .. }
            | reads::Scope::Tasks { .. }
            | reads::Scope::SettingsHistory { .. }
            | reads::Scope::Holds { .. }
            | reads::Scope::Spend { .. } => return Err(Failure::Corrupt),
        };
        if !selected {
            spent.rows = spent.rows.checked_sub(1).ok_or(Failure::Cut)?;
            last = Some(position);
            continue;
        }
        let key = core::Key::Notes(notes::Key::Entry { name: line.name });
        let value = match joins::get(store, &key, spent) {
            Ok(Some(value)) => value,
            Ok(None) => {
                spent.rows = spent.rows.checked_sub(1).ok_or(Failure::Cut)?;
                last = Some(position);
                continue;
            }
            Err(Failure::Cut) => return Ok(reads::Loaded::Page { rows: found.into_boxed(), next: last }),
            Err(Failure::Corrupt) => return Err(Failure::Corrupt),
        };
        match joins::row(store, value, None, None, spent) {
            Ok(row) => {
                last = Some(position);
                found.push(row).expect("bounded by walked lines");
            }
            Err(Failure::Cut) => return Ok(reads::Loaded::Page { rows: found.into_boxed(), next: last }),
            Err(Failure::Corrupt) => return Err(Failure::Corrupt),
        }
    }
    Ok(reads::Loaded::Page { rows: found.into_boxed(), next: if walked.next.is_some() { last } else { None } })
}

pub(super) fn scoped_notes(
    store: &mut Store,
    range: &reads::Range,
    spent: &mut Spent,
) -> Result<reads::Loaded, Failure> {
    notes(store, range, spent)
}
