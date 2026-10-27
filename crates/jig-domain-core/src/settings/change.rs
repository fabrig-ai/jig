//! The edits a settings decision leaves (engine.md, 17.1–17.2).
use super::{Changed, Current, Edit, Failure, Fault, Item, Key, Record, Setting};
use crate::{CoreKey, CoreRecord, Limits, Write};
use skein_lib::List;

pub(crate) fn change_shape(items: &[Item], connectors: &[u16], limits: &Limits) -> Result<(), Failure> {
    if items.len()
        > usize::try_from(super::capacity(limits).expect("settings capacity").checked_mul(2).expect("item union"))
            .expect("item count")
    {
        return Err(Failure { setting: items.first().expect("past nonzero capacity").setting, fault: Fault::Count });
    }
    let mut counts = [0_u32; 3];
    for (at, item) in items.iter().enumerate() {
        match item.setting {
            Setting::Charter(0) | Setting::Template(0) | Setting::Policy(0) => {
                return Err(Failure { setting: item.setting, fault: Fault::Shape });
            }
            Setting::Charter(_) | Setting::Template(_) | Setting::Policy(_) | Setting::Rules => {}
        }
        for earlier in items.iter().take(at) {
            if item.setting == earlier.setting {
                return Err(Failure { setting: item.setting, fault: Fault::Shape });
            }
        }
        match &item.edit {
            Edit::Set(value) => {
                super::check::shape(item.setting, value, connectors, limits)?;
                super::check::count(item.setting, &mut counts, limits)?;
            }
            Edit::Removed if item.setting == Setting::Rules => {
                return Err(Failure { setting: item.setting, fault: Fault::Shape });
            }
            Edit::Removed | Edit::Reset => {}
        }
    }
    Ok(())
}
#[must_use]
#[expect(clippy::manual_map, reason = "step code borrows the default explicitly without a closure")]
pub(crate) fn changes_current(current: &Current, items: &[Item]) -> bool {
    for item in items {
        let after = match &item.edit {
            Edit::Set(value) => Some(value),
            Edit::Removed => None,
            Edit::Reset => match current.defaults.get(&item.setting) {
                Some(default) => Some(&default.value),
                None => None,
            },
        };
        if after != current.value(item.setting) {
            return true;
        }
    }
    false
}
/// Apply only a checked, nonempty set. Both persisted and held changes get the same stamp.
pub(crate) fn apply(current: &mut Current, items: &[Item], revision: u32) -> List<Write> {
    let mut writes =
        List::with_capacity(u32::try_from(items.len()).expect("bounded items").checked_mul(2).expect("reset writes"));
    // Erases precede puts in memory so replacing a whole bounded kind needs no spare slot.
    for item in items {
        let erase = match item.edit {
            Edit::Reset => true,
            Edit::Removed => current.defaults.get(&item.setting).is_none(),
            Edit::Set(_) => false,
        };
        if erase {
            drop(current.changes.remove(&item.setting));
        }
    }
    for item in items {
        let value = match &item.edit {
            Edit::Set(value) => Some(Some(value.clone())),
            Edit::Removed if current.defaults.get(&item.setting).is_some() => Some(None),
            Edit::Removed | Edit::Reset => None,
        };
        let write = match value {
            Some(value) => {
                let changed = Changed { value, revision };
                drop(current.changes.insert(item.setting, changed.clone()).expect("checked changed count"));
                Write::Save(crate::Record::Core(CoreRecord::Settings(Record::Changed {
                    setting: item.setting,
                    changed,
                })))
            }
            None => Write::Erase(crate::Key::Core(CoreKey::Settings(Key::Changed(item.setting)))),
        };
        writes.push(write).expect("one change write per item");
        if item.edit == Edit::Reset
            && let Some(default) = current.defaults.get_mut(&item.setting)
        {
            default.revision = revision;
            writes
                .push(Write::Save(crate::Record::Core(CoreRecord::Settings(Record::Default {
                    setting: item.setting,
                    default: default.clone(),
                }))))
                .expect("reset default write");
        }
    }
    writes
}
