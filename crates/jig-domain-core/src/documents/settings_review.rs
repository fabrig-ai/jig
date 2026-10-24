//! A pending proposal's values are reviewed against the newest settings (engine.md, 17.6).
use crate::{Core, Limits, settings as s};
use alloc::boxed::Box;
use jig_domain_tasks as t;
use skein_lib::List;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ReviewItem {
    pub setting: s::Setting,
    pub edit: crate::reads::RevisionEdit,
    pub now: Option<s::Value>,
    /// true is a change, false a default; absent is not offered.
    pub origin: Option<(bool, u32)>,
    pub would_be: Option<s::Value>,
}
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct SettingsReview {
    pub newest: u32,
    pub items: List<ReviewItem>,
    pub value_bytes: u64,
}
#[must_use]
pub fn settings_heads(items: &[t::SettingHead]) -> List<crate::reads::RevisionItemHead> {
    let mut result = List::with_capacity(t::SETTINGS_ITEMS);
    for item in items {
        result
            .push(crate::reads::RevisionItemHead {
                setting: match item.setting {
                    t::Setting::Charter(n) => s::Setting::Charter(n),
                    t::Setting::Template(n) => s::Setting::Template(n),
                    t::Setting::Policy(n) => s::Setting::Policy(n),
                    t::Setting::Rules => s::Setting::Rules,
                },
                edit: match item.edit {
                    t::SettingEdit::Set => crate::reads::RevisionEdit::Set,
                    t::SettingEdit::Removed => crate::reads::RevisionEdit::Removed,
                    t::SettingEdit::Reset => crate::reads::RevisionEdit::Reset,
                },
            })
            .expect("bounded item heads");
    }
    result
}
pub(super) fn review(core: &Core, limits: &Limits, proposer: u64, number: u64) -> Option<Box<SettingsReview>> {
    let number = u32::try_from(number).ok()?;
    let set = core.settings_proposals.get(&(jig_domain_people::Proposer::Party(proposer), number))?;
    let mut items = List::with_capacity(t::SETTINGS_ITEMS);
    for item in &set.items {
        let (now, origin) = core.current.lookup(item.setting);
        let origin = if now.is_some() { origin } else { None };
        let (edit, would_be) = match &item.edit {
            s::Edit::Set(value) => (crate::reads::RevisionEdit::Set, Some(value)),
            s::Edit::Removed => (crate::reads::RevisionEdit::Removed, None),
            s::Edit::Reset => (crate::reads::RevisionEdit::Reset, core.current.default_value(item.setting)),
        };
        items
            .push(ReviewItem { setting: item.setting, edit, now: now.cloned(), origin, would_be: would_be.cloned() })
            .expect("bounded review");
    }
    Some(Box::new(SettingsReview {
        newest: core.current.newest(),
        items,
        value_bytes: u64::from(limits.documents.text_bytes),
    }))
}
pub(super) fn worst_case(limits: &Limits) -> Option<u64> {
    s::set_worst_case(limits)?
        .checked_mul(2)?
        .checked_add(List::<ReviewItem>::worst_case(t::SETTINGS_ITEMS)?)?
        .checked_add(List::<crate::reads::RevisionItemHead>::worst_case(t::SETTINGS_ITEMS)?)?
        .checked_add(u64::try_from(size_of::<SettingsReview>()).ok()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn review_set_removal_and_reset_keep_now_and_the_would_be_default() {
        let (mut core, limits) = crate::tests::core();
        let setting = s::Setting::Charter(1);
        let default = core.current.value(setting).unwrap().clone();
        let mut changed = core.current.charter(1).unwrap().clone();
        changed.policy.instructions = b"changed".as_slice().into();
        let changed = s::Value::Charter(Box::new(changed));
        core.current.changes.insert(setting, s::Changed { value: Some(changed.clone()), revision: 2 }).unwrap();
        core.current.head.number = 2;
        for (edit, expected) in [
            (s::Edit::Set(default.clone()), Some(default.clone())),
            (s::Edit::Removed, None),
            (s::Edit::Reset, Some(default.clone())),
        ] {
            core.settings_proposals
                .insert(
                    (jig_domain_people::Proposer::Party(1), 7),
                    s::Proposal { against: 2, items: Box::new([s::Item { setting, edit }]) },
                )
                .unwrap();
            let review = review(&core, &limits, 1, 7).unwrap();
            let item = review.items.get(0).unwrap();
            assert_eq!(review.newest, 2);
            assert_eq!(item.now, Some(changed.clone()));
            assert_eq!(item.origin, Some((true, 2)));
            assert_eq!(item.would_be, expected);
        }
        core.current.changes.insert(setting, s::Changed { value: None, revision: 3 }).unwrap();
        core.current.head.number = 3;
        core.settings_proposals
            .insert(
                (jig_domain_people::Proposer::Party(1), 7),
                s::Proposal { against: 3, items: Box::new([s::Item { setting, edit: s::Edit::Reset }]) },
            )
            .unwrap();
        let review = review(&core, &limits, 1, 7).unwrap();
        let item = review.items.get(0).unwrap();
        assert_eq!(item.now, None);
        assert_eq!(item.origin, None);
        assert_eq!(item.would_be, Some(default));
    }
}
