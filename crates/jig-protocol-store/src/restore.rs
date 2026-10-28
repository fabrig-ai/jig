//! Restart pages, including the inbox census joins (protocol/store.md, 7.2).
use crate::pages::Decoded as Restored;
use crate::{Below, Limits, secrets};
use crate::{Cursor, Event, Phase, Restore, Row, Store, codec, intervals, keys, pages};
use alloc::boxed::Box;
use jig_domain_core::{Key, Record};
use jig_domain_tasks as tasks;
use skein_lib::{Env, List, Queue, Token};

pub(crate) struct Page {
    pub range: Restore,
    pub after: Option<Cursor>,
    pub rows: u32,
    pub bytes: u64,
}
pub(crate) fn restore(
    store: &mut Store,
    env: &Env<Limits>,
    owner: Token,
    page: Page,
    above: &mut Queue<Event>,
    below: &mut Below,
) {
    let Page { range, after, rows, bytes } = page;
    let people = people_interval(&range);
    let Some(walked) = restore_page(store, range, after.as_ref(), rows, bytes) else {
        store.phase = Phase::Stopped;
        above.push(Event::Corrupt { owner });
        return;
    };
    if let Some(interval) = people {
        let done = match &walked.next {
            Some(next) => next.interval > interval,
            None => true,
        };
        if done {
            secrets::start_sweep(store);
            secrets::sweep(store, env, above, below);
            if store.phase == Phase::Stopped {
                return;
            }
        }
    }
    above.push(Event::Restored { owner, rows: walked.rows, next: walked.next });
}

fn restore_page(store: &mut Store, range: Restore, after: Option<&Cursor>, rows: u32, bytes: u64) -> Option<Restored> {
    match store.phase {
        Phase::Ready { .. } => {}
        Phase::Closed | Phase::Stopped => return None,
    }
    let ranges = match range {
        Restore::Application(ranges) => pages::application(&ranges, store.limits.store.key)?,
        Restore::Jig(ranges) => intervals::intervals(&ranges)?,
    };
    let walked = pages::walk(store, &ranges, after, rows)?;
    let mut found = List::with_capacity(rows);
    let mut used = 0_u64;
    let mut cursor = after.cloned().unwrap_or(Cursor { interval: 0, after: Box::new([]) });
    for item in walked.rows {
        let (row, joined, size) = decode(store, item.row)?;
        let total = used.checked_add(size)?;
        if total > bytes || found.room() < if joined.is_some() { 2 } else { 1 } {
            return Some(Restored { rows: found.into_boxed(), next: Some(cursor) });
        }
        used = total;
        cursor = item.cursor;
        found.push(row).expect("bounded restore rows");
        if let Some(joined) = joined {
            found.push(joined).expect("pair room checked");
        }
    }
    Some(Restored { rows: found.into_boxed(), next: walked.next })
}

fn decode(store: &mut Store, row: skein_kv::Row) -> Option<(Row, Option<Row>, u64)> {
    let (row, size) = pages::decode(store, row)?;
    let task = match &row {
        Row::Jig(Record::Tasks(tasks::Stored::Result(row))) => row.task,
        Row::Jig(Record::People(jig_domain_people::Stored::EndedRead { task, .. })) => *task,
        Row::Jig(_) | Row::Application { .. } => return Some((row, None, size)),
    };
    let (record, owner_size) = match ended(store, task)? {
        Some(ended) => ended,
        None => panic!(
            "store: {:?} links to Ended({task}), which is absent",
            match &row {
                Row::Jig(record) => record.key(),
                Row::Application { .. } => unreachable!("typed census row"),
            }
        ),
    };
    if let Row::Jig(Record::People(jig_domain_people::Stored::EndedRead { .. })) = row {
        Some((row, Some(Row::Jig(record)), size.checked_add(owner_size)?))
    } else {
        Some((Row::Jig(record), None, owner_size))
    }
}

/// The restart's census join, shared with the check before the switch.
pub(crate) fn ended(store: &mut Store, task: u64) -> Option<Option<(Record, u64)>> {
    let key = Key::Tasks(tasks::Key::Ended(task));
    let mut above = Queue::with_capacity(skein_kv::MAX_OUT.above);
    let mut below = Queue::with_capacity(skein_kv::MAX_OUT.below);
    skein_kv::down(
        &mut store.stores.store,
        &crate::memory_env(store.limits.store),
        skein_kv::Request::Get { owner: Token::new(0), key: keys::encode(&key, store.limits.store.key)? },
        &mut above,
        &mut below,
    );
    assert!(below.is_empty(), "memory census joins issue no files");
    let value = match above.pop()? {
        skein_kv::Event::Got { value: Some(value), .. } => value,
        skein_kv::Event::Got { value: None, .. } => return Some(None),
        _ => return None,
    };
    let record = codec::decode(&key, &value, &store.codec, &store.names)?;
    if record.key() != key {
        return None;
    }
    Some(Some((record, u64::try_from(value.len()).ok()?)))
}

fn people_interval(range: &Restore) -> Option<u32> {
    match range {
        Restore::Jig(ranges) => {
            let mut interval = 0_u32;
            let mut people = None;
            for range in ranges {
                match range {
                    jig_domain_core::RestoreRange::People => {
                        let count = u32::try_from(intervals::restore(range).len()).ok()?;
                        people = Some(interval.checked_add(count.checked_sub(1)?)?);
                    }
                    jig_domain_core::RestoreRange::Deployment
                    | jig_domain_core::RestoreRange::Settings
                    | jig_domain_core::RestoreRange::Accounts
                    | jig_domain_core::RestoreRange::Tasks
                    | jig_domain_core::RestoreRange::Core
                    | jig_domain_core::RestoreRange::Census(_) => {}
                }
                interval = interval.checked_add(u32::try_from(intervals::restore(range).len()).ok()?)?;
            }
            people
        }
        Restore::Application(_) => None,
    }
}
