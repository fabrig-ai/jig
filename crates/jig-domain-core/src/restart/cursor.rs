//! Typed positions in the restart walk (protocol/store.md, 4.3 and 7.2).
//! The protocol's encoded intervals are checked against these typed places.
use super::RestoreRange;
use crate::Key;

/// A key's exclusive cursor in a request's concatenated restore ranges.
/// The interval precedes the typed key in comparisons; no bytes, IO or
/// allocation are needed for a restore key. Keys outside the walk have no place.
#[must_use]
pub fn restore_cursor(ranges: &[RestoreRange], key: &Key) -> Option<(u32, Key)> {
    let mut offset = 0_u32;
    for range in ranges {
        let (count, interval) = place(range, key)?;
        if let Some(interval) = interval {
            return Some((offset.checked_add(interval)?, key.clone()));
        }
        offset = offset.checked_add(count)?;
    }
    None
}

// One typed description of each range's interval count and membership.
fn place(range: &RestoreRange, key: &Key) -> Option<(u32, Option<u32>)> {
    let admitted = range.admits(key);
    let (count, interval) = match range {
        RestoreRange::Deployment | RestoreRange::Settings | RestoreRange::Core => (1, 0),
        RestoreRange::Accounts => {
            let interval = match key {
                Key::Accounts(jig_domain_accounts::Key::Issued { .. }) => 0,
                Key::Accounts(
                    jig_domain_accounts::Key::Next
                    | jig_domain_accounts::Key::Account { .. }
                    | jig_domain_accounts::Key::Source { .. },
                )
                | Key::Core(_)
                | Key::People(_)
                | Key::Tasks(_)
                | Key::Notes(_) => 1,
            };
            (2, interval)
        }
        RestoreRange::People => {
            let interval = if let Key::People(jig_domain_people::Key::Acted(_)) = key { 1 } else { 0 };
            (2, interval)
        }
        RestoreRange::Tasks => {
            let interval = if let Key::Tasks(jig_domain_tasks::Key::PlacedHold(_)) = key { 1 } else { 0 };
            (2, interval)
        }
        RestoreRange::Census(census) => {
            let count = u32::try_from(census.results.len())
                .ok()?
                .checked_add(1)?
                .checked_add(u32::try_from(census.questions.len()).ok()?)?;
            return Some((count, census.interval(key)));
        }
    };
    Some((count, if admitted { Some(interval) } else { None }))
}
