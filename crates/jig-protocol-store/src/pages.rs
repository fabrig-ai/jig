//! Ordered memory pages and exclusive cursors (protocol/store.md, 4.3 and 7.1).
use crate::{Cursor, KeptMap, Row, Store, codec, keys};
use alloc::boxed::Box;
use skein_kv::{KeyWriter, Range};
use skein_lib::{List, Queue, Token};

#[derive(Debug)]
pub(crate) struct WalkRow {
    pub row: skein_kv::Row,
    pub cursor: Cursor,
}

pub(crate) struct Walked {
    pub rows: Box<[WalkRow]>,
    pub next: Option<Cursor>,
}

pub(crate) struct Decoded {
    pub rows: Box<[Row]>,
    pub next: Option<Cursor>,
}

pub(crate) fn walk(store: &mut Store, intervals: &[Range], after: Option<&Cursor>, rows: u32) -> Option<Walked> {
    walk_map(store, KeptMap::Store, intervals, after, rows)
}

pub(crate) fn walk_map(
    store: &mut Store,
    map: KeptMap,
    intervals: &[Range],
    after: Option<&Cursor>,
    rows: u32,
) -> Option<Walked> {
    // Every encoded row costs at least one byte. The encoded page therefore
    // bounds the carrier list even when the asker supplies a larger row count.
    let limits = map_limits(store, map);
    let capacity = rows.min(limits.page.bytes);
    let mut found = List::with_capacity(capacity);
    let mut encoded = 0_u64;
    let mut last = after.cloned().unwrap_or(Cursor { interval: 0, after: Box::new([]) });
    let first = usize::try_from(last.interval).ok()?;
    for (index, interval) in intervals.iter().enumerate().skip(first) {
        let index = u32::try_from(index).ok()?;
        let mut range = interval.clone();
        if index == last.interval && !last.after.is_empty() {
            range.start = range.start.max(past(&last.after)?);
        }
        for _ in 0..=capacity {
            if let Some(end) = &range.end
                && range.start >= *end
            {
                break;
            }
            let page = load_map(store, map, range.clone())?;
            let mut page_rows = page.rows.into_iter();
            let Some(row) = page_rows.next() else { break };
            let size = u64::try_from(row.key.len()).ok()?.checked_add(u64::try_from(row.value.len()).ok()?)?;
            let total = encoded.checked_add(size)?;
            if found.len() == capacity || total > u64::from(limits.page.bytes) {
                return Some(Walked { rows: found.into_boxed(), next: Some(last) });
            }
            encoded = total;
            last = Cursor { interval: index, after: row.key.clone() };
            range.start = past(&row.key)?;
            found.push(WalkRow { row, cursor: last.clone() }).expect("encoded and row bounds checked");
            if page.next.is_none() {
                break;
            }
        }
        // The interval is exhausted here. Jig 17 sweeps secrets at this
        // point for the People's interval (prefix [JIG, 2]).
    }
    Some(Walked { rows: found.into_boxed(), next: None })
}

pub(crate) fn past(key: &[u8]) -> Option<Box<[u8]>> {
    let mut writer = KeyWriter::new(u32::MAX);
    for &byte in key {
        writer.tag(byte);
    }
    writer.tag(0);
    writer.finish()
}

pub(crate) struct Page {
    pub(crate) rows: Box<[skein_kv::Row]>,
    pub(crate) next: Option<Box<[u8]>>,
}

pub(crate) fn load(store: &mut Store, range: Range) -> Option<Page> {
    load_map(store, KeptMap::Store, range)
}

fn load_map(store: &mut Store, map: KeptMap, range: Range) -> Option<Page> {
    let mut limits = map_limits(store, map);
    // A valid maximum-length key's exclusive cursor has one extra byte.
    // This widens range endpoints only; stored keys retain their own bound.
    limits.key = limits.key.checked_add(1)?;
    let mut above = Queue::uncapped();
    let mut below = Queue::uncapped();
    let kv = match map {
        KeptMap::Store => &mut store.stores.store,
        KeptMap::Secrets => &mut store.stores.secrets,
    };
    skein_kv::down(
        kv,
        &crate::memory_env(limits),
        skein_kv::Request::Load {
            owner: Token::new(0),
            range,
            max: skein_kv::Page { rows: 1, bytes: limits.page.bytes },
        },
        &mut above,
        &mut below,
    );
    assert!(below.is_empty(), "memory pages issue no files");
    match above.pop()? {
        skein_kv::Event::Loaded { rows, next, .. } => Some(Page { rows, next }),
        skein_kv::Event::Opened { .. }
        | skein_kv::Event::Committed { .. }
        | skein_kv::Event::Refused { .. }
        | skein_kv::Event::Failed { .. }
        | skein_kv::Event::Got { .. }
        | skein_kv::Event::Closed { .. }
        | skein_kv::Event::OpenFailed { .. }
        | skein_kv::Event::Recovered { .. }
        | skein_kv::Event::RecoveryFailed { .. } => None,
    }
}

pub(crate) fn application(intervals: &[Range], limit: u32) -> Option<Box<[Range]>> {
    let mut tagged = List::with_capacity(u32::try_from(intervals.len()).ok()?);
    for interval in intervals {
        let start = keys::application(&interval.start, limit)?;
        let end: Option<Box<[u8]>> = match &interval.end {
            Some(end) => Some(keys::application(end, limit)?),
            None => Some(Box::new([keys::APPLICATION.checked_add(1)?])),
        };
        tagged.push(Range { start, end }).expect("one tagged range per input");
    }
    Some(tagged.into_boxed())
}

pub(crate) fn decode(store: &Store, row: skein_kv::Row) -> Option<(Row, u64)> {
    if let Some(key) = keys::of_application(&row.key) {
        let size = u64::try_from(key.len()).ok()?.checked_add(u64::try_from(row.value.len()).ok()?)?;
        return Some((Row::Application { key: Box::from(key), value: row.value }, size));
    }
    let key = keys::decode(&row.key)?;
    let record = codec::decode(&key, &row.value, &store.codec, &store.names)?;
    if record.key() != key {
        return None;
    }
    // The generated family's measure is its wire length. Once decoded,
    // these stored bytes give that measure without encoding the value again.
    Some((Row::Jig(record), u64::try_from(row.value.len()).ok()?))
}

pub(crate) fn map_limits(store: &Store, map: KeptMap) -> skein_kv::Limits {
    match map {
        KeptMap::Store => store.limits.store,
        KeptMap::Secrets => store.limits.secrets,
    }
}

/// Kept keys share the walk's map and application prefix (store.md 10.3).
pub(crate) fn kept_interval(store: &Store, owner: crate::Owner, map: KeptMap, keys: &crate::Keys) -> Option<Range> {
    if owner != crate::Owner::Application {
        return Some(Range { start: Box::from(keys.from), end: Some(Box::from(keys.to)) });
    }
    let limit = map_limits(store, map).key;
    let start = application_key(map, keys.from, limit)?;
    let end = Some(application_key(map, keys.to, limit)?);
    Some(Range { start, end })
}
fn application_key(map: KeptMap, key: &[u8], limit: u32) -> Option<Box<[u8]>> {
    match map {
        KeptMap::Store => keys::application(key, limit),
        KeptMap::Secrets => crate::secrets::application(key, limit),
    }
}
