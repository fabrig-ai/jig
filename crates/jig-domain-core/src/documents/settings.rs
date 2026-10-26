//! Settings inventory without values (client/domain/contract.md, 5.1 and 7.1).
use super::{Missing, Text};
use crate::{
    Core, Limits,
    settings::{Head, Setting, Value},
};
use jig_domain_people as people;
use skein_lib::{List, Map};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Follows {
    Default,
    Change,
}
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct SettingLine {
    pub setting: Setting,
    pub name: Option<Text>,
    pub follows: Follows,
    pub last: u32,
}
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct HeldRevision {
    pub head: Head,
    pub holders: u32,
}
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct SettingsProposal {
    pub proposer: people::Proposer,
    pub number: u32,
}
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Settings {
    pub newest: u32,
    pub settings: List<SettingLine>,
    pub held: List<HeldRevision>,
    pub pending: List<SettingsProposal>,
}

pub(super) fn make(core: &Core, party: u64) -> Result<Settings, Missing> {
    if core.people.role(party, people::DEPLOYMENT).is_none() {
        return Err(Missing::NotAllowed);
    }
    let mut result = build(core);
    result.pending = List::with_capacity(core.settings_proposals.capacity());
    for (&(proposer, number), _) in &core.settings_proposals {
        result.pending.push(SettingsProposal { proposer, number }).expect("pending bound");
    }
    Ok(result)
}

pub(crate) fn build(core: &Core) -> Settings {
    let current = &core.current;
    let mut names =
        Map::with_capacity(current.defaults.len().checked_add(current.changes.len()).expect("settings bounds"));
    for (&setting, _) in &current.defaults {
        names.insert(setting, ()).expect("default names");
    }
    for (&setting, _) in &current.changes {
        names.insert(setting, ()).expect("change names");
    }
    let mut settings = List::with_capacity(names.len());
    for (&setting, ()) in &names {
        let (follows, last) = match current.changes.get(&setting) {
            Some(change) => (Follows::Change, change.revision),
            None => (Follows::Default, current.defaults.get(&setting).expect("named default").revision),
        };
        let name = match current.value(setting) {
            Some(Value::Template(value)) => Some(Text { bytes: value.name.clone(), cut: false }),
            Some(Value::Charter(_) | Value::Policy(_) | Value::Rules(_)) | None => None,
        };
        settings.push(SettingLine { setting, name, follows, last }).expect("one line per setting");
    }
    let mut held = List::with_capacity(core.held.heads.capacity().checked_add(1).expect("held inventory bound"));
    for (&revision, head) in &core.held.heads {
        held.push(HeldRevision { head: head.clone(), holders: core.tasks.holders(revision) }).expect("held inventory");
    }
    held.push(HeldRevision { head: current.newest_head().clone(), holders: core.tasks.holders(current.newest()) })
        .expect("newest held");
    Settings { newest: current.newest(), settings, held, pending: List::with_capacity(0) }
}

pub(super) fn worst_case(limits: &Limits) -> Option<u64> {
    let count = crate::settings::capacity(limits)?.checked_mul(2)?;
    List::<SettingLine>::worst_case(count)?
        .checked_add(u64::from(limits.templates).checked_mul(2)?.checked_mul(u64::from(limits.documents.text_bytes))?)?
        .checked_add(List::<HeldRevision>::worst_case(limits.settings.revisions.checked_add(1)?)?)?
        .checked_add(List::<SettingsProposal>::worst_case(limits.settings.proposals)?)?
        .checked_add(u64::try_from(size_of::<Settings>()).ok()?)
}
pub(super) fn scratch_worst_case(limits: &Limits) -> Option<u64> {
    Map::<Setting, ()>::worst_case(crate::settings::capacity(limits)?.checked_mul(2)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{Changed, Current};
    use alloc::boxed::Box;

    fn current() -> Current {
        let mut config = crate::tests::config(1);
        config.defaults.templates = Box::new([crate::Template {
            number: 1,
            name: Box::from(b"chat".as_slice()),
            executor: people::TemplateExecutor::Charter(1),
            contract: jig_domain_tasks::Contract::Report { words: 1 },
            shallower: 0,
            budget: people::TemplateBudget::Given,
            deadline: people::TemplateDeadline::None,
            tracked: None,
            flags: jig_domain_tasks::Flags::PLAIN,
            label: 0,
            grace: None,
            llm: None,
        }]);
        Current::new(config.defaults, &crate::tests::limits())
    }
    fn document(current: Current) -> Settings {
        let (mut core, _) = crate::tests::core();
        core.current = current;
        build(&core)
    }
    fn line(current: &Current) -> SettingLine {
        let mut copied = Current {
            defaults: Map::with_capacity(current.defaults.capacity()),
            changes: Map::with_capacity(current.changes.capacity()),
            head: current.head.clone(),
            oldest: current.oldest,
        };
        for (&setting, value) in &current.defaults {
            copied.defaults.insert(setting, value.clone()).unwrap();
        }
        for (&setting, value) in &current.changes {
            copied.changes.insert(setting, value.clone()).unwrap();
        }
        for line in document(copied).settings.into_boxed() {
            if line.setting == Setting::Template(1) {
                return line;
            }
        }
        panic!("the inventory names template 1")
    }
    #[test]
    fn defaults_changes_removals_and_resets_keep_the_value_revision() {
        let mut current = current();
        let original = current.defaults.get(&Setting::Template(1)).unwrap().value.clone();
        assert_eq!(
            line(&current),
            SettingLine {
                setting: Setting::Template(1),
                name: Some(Text { bytes: Box::from(b"chat".as_slice()), cut: false }),
                follows: Follows::Default,
                last: 1
            }
        );
        current.changes.insert(Setting::Template(1), Changed { value: Some(original.clone()), revision: 2 }).unwrap();
        assert_eq!((line(&current).follows, line(&current).last), (Follows::Change, 2));
        current.changes.insert(Setting::Template(1), Changed { value: None, revision: 3 }).unwrap();
        assert_eq!(
            line(&current),
            SettingLine { setting: Setting::Template(1), name: None, follows: Follows::Change, last: 3 }
        );
        let writes = crate::settings::apply(
            &mut current,
            &[crate::settings::Item { setting: Setting::Template(1), edit: crate::settings::Edit::Reset }],
            4,
        );
        assert_eq!(writes.len(), 2);
        assert_eq!((line(&current).follows, line(&current).last), (Follows::Default, 4));
    }
    #[test]
    fn inventory_keeps_disjoint_removed_names_in_setting_order() {
        let mut current = current();
        current.changes.insert(Setting::Template(2), Changed { value: None, revision: 2 }).unwrap();
        let document = document(current);
        let settings = document.settings.into_boxed();
        let wanted = [Setting::Charter(1), Setting::Template(1), Setting::Template(2), Setting::Rules];
        assert_eq!(settings.len(), wanted.len());
        for (line, setting) in settings.iter().zip(wanted) {
            assert_eq!(line.setting, setting);
        }
        assert_eq!(document.held.get(0).unwrap().holders, 0);
        assert!(document.pending.is_empty());
    }
}
