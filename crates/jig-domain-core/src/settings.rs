//! Deployment settings, defaults and revisions (domain/engine.md, section 17).
use crate::{Charter, ConfiguredPolicy, Template};
use alloc::boxed::Box;
use jig_domain_authority as authority;
use jig_domain_people as people;
use skein_lib::{List, Map, Wall};

mod change;
pub(crate) mod held;
pub use held::{Held, VersionKey};
pub(crate) mod check;
pub(crate) use change::{apply, change_shape, changes_current};
pub(crate) mod restart;
pub use check::{check, check_change};
pub use restart::revision_writes;

pub use people::{Running, Setting, SettingFault as Fault};
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Value {
    Charter(Box<Charter>),
    Template(Box<Template>),
    Policy(Box<ConfiguredPolicy>),
    Rules(Box<authority::Rules>),
}
#[derive(Debug)]
pub struct Defaults {
    pub charters: Box<[Charter]>,
    pub templates: Box<[Template]>,
    pub policies: Map<u32, ConfiguredPolicy>,
    pub rules: authority::Rules,
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Default {
    pub value: Value,
    pub revision: u32,
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Changed {
    pub value: Option<Value>,
    pub revision: u32,
}
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Edit {
    Set(Value),
    Removed,
    Reset,
}
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Item {
    pub setting: Setting,
    pub edit: Edit,
}
#[derive(Debug)]
pub struct Change {
    pub against: u32,
    pub items: Box<[Item]>,
    pub running: Running,
}
#[derive(Debug)]
pub struct Proposed {
    pub against: u32,
    pub items: Box<[Item]>,
    pub reason: Box<[u8]>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Decider {
    FirstStart,
    Release,
    Party(u64),
}
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Head {
    pub number: u32,
    pub against: u32,
    pub at: Wall,
    pub by: Decider,
    pub proposal: Option<(people::Proposer, u32)>,
    pub moved: bool,
}
/// A stored catalogue's identity; revision indices are relative to these names.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ModelName {
    pub name: Box<[u8]>,
    pub efforts: Box<[Box<[u8]>]>,
}
#[must_use]
pub fn model_names(models: &[crate::Model]) -> Box<[ModelName]> {
    let mut names = List::with_capacity(u32::try_from(models.len()).expect("configured model count"));
    for model in models {
        names.push(ModelName { name: model.name.clone(), efforts: model.efforts.clone() }).expect("model count");
    }
    names.into_boxed()
}
#[must_use]
pub fn model_names_worst_case() -> Option<u64> {
    List::<ModelName>::worst_case(jig_domain_tasks::MODELS)?.checked_add(
        u64::from(jig_domain_tasks::MODELS).checked_mul(
            u64::from(crate::MODEL_NAME_BYTES)
                .checked_add(List::<Box<[u8]>>::worst_case(crate::EFFORTS)?)?
                .checked_add(u64::from(crate::EFFORTS).checked_mul(u64::from(crate::EFFORT_BYTES))?)?,
        )?,
    )
}
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Revision {
    pub head: Head,
    pub items: Box<[Item]>,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Failure {
    pub setting: Setting,
    pub fault: Fault,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Limits {
    pub revisions: u32,
    pub versions: u32,
    pub history: u32,
    pub proposals: u32,
    pub alternatives: u32,
    pub permissions: u32,
}

/// Addresses sort in the same order as the store's tag 8 subfamilies, then tag 9.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Key {
    Default(Setting),
    Changed(Setting),
    Held(u32),
    Charter { charter: u32, revision: u32 },
    Proposal { proposer: people::Proposer, proposal: u32 },
    Extent,
    History(u32),
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CharterVersion {
    pub charter: Option<Box<Charter>>,
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Proposal {
    pub against: u32,
    pub items: Box<[Item]>,
}

/// Only defaults and changes own current values. Revision history stays in the store.
#[derive(Debug)]
pub struct Current {
    pub(crate) defaults: Map<Setting, Default>,
    pub(crate) changes: Map<Setting, Changed>,
    pub(crate) head: Head,
    pub(crate) oldest: u32,
}
impl Current {
    #[must_use]
    pub fn new(mut defaults: Defaults, limits: &crate::Limits) -> Self {
        let mut values = Map::with_capacity(capacity(limits).expect("settings count"));
        for charter in defaults.charters {
            let previous = values
                .insert(
                    Setting::Charter(charter.number),
                    Default { value: Value::Charter(Box::new(charter)), revision: 1 },
                )
                .expect("default charter bound");
            assert!(previous.is_none(), "unique default charter");
        }
        for template in defaults.templates {
            let previous = values
                .insert(
                    Setting::Template(template.number),
                    Default { value: Value::Template(Box::new(template)), revision: 1 },
                )
                .expect("default template bound");
            assert!(previous.is_none(), "unique default template");
        }
        for _ in 0..defaults.policies.len() {
            let number = *defaults.policies.first().expect("default policy count").0;
            let policy = defaults.policies.remove(&number).expect("default policy");
            values
                .insert(Setting::Policy(number), Default { value: Value::Policy(Box::new(policy)), revision: 1 })
                .expect("default policy bound");
        }
        values
            .insert(Setting::Rules, Default { value: Value::Rules(Box::new(defaults.rules)), revision: 1 })
            .expect("rules slot");
        Self {
            defaults: values,
            changes: Map::with_capacity(capacity(limits).expect("settings count")),
            head: Head {
                number: 1,
                against: 0,
                at: Wall::EPOCH,
                by: Decider::FirstStart,
                proposal: None,
                moved: false,
            },
            oldest: 0,
        }
    }
    #[must_use]
    pub fn value(&self, setting: Setting) -> Option<&Value> {
        match self.changes.get(&setting) {
            Some(change) => change.value.as_ref(),
            None => match self.defaults.get(&setting) {
                Some(default) => Some(&default.value),
                None => None,
            },
        }
    }
    /// The current value and the origin it follows, including explicit removal.
    #[must_use]
    pub fn lookup(&self, setting: Setting) -> (Option<&Value>, Option<(bool, u32)>) {
        match self.changes.get(&setting) {
            Some(change) => (change.value.as_ref(), Some((true, change.revision))),
            None => match self.defaults.get(&setting) {
                Some(default) => (Some(&default.value), Some((false, default.revision))),
                None => (None, None),
            },
        }
    }
    #[must_use]
    pub fn default_value(&self, setting: Setting) -> Option<&Value> {
        match self.defaults.get(&setting) {
            Some(default) => Some(&default.value),
            None => None,
        }
    }
    #[must_use]
    pub fn charter(&self, number: u32) -> Option<&Charter> {
        match self.value(Setting::Charter(number)) {
            Some(Value::Charter(value)) => Some(value),
            Some(Value::Template(_) | Value::Policy(_) | Value::Rules(_)) | None => None,
        }
    }
    #[must_use]
    pub fn template(&self, number: u32) -> Option<&Template> {
        match self.value(Setting::Template(number)) {
            Some(Value::Template(value)) => Some(value),
            Some(Value::Charter(_) | Value::Policy(_) | Value::Rules(_)) | None => None,
        }
    }
    #[must_use]
    pub fn policy(&self, number: u32) -> Option<&ConfiguredPolicy> {
        match self.value(Setting::Policy(number)) {
            Some(Value::Policy(value)) => Some(value),
            Some(Value::Charter(_) | Value::Template(_) | Value::Rules(_)) | None => None,
        }
    }
    #[must_use]
    pub fn rules(&self) -> &authority::Rules {
        match self.value(Setting::Rules) {
            Some(Value::Rules(value)) => value,
            Some(Value::Charter(_) | Value::Template(_) | Value::Policy(_)) | None => {
                unreachable!("checked settings always offer rules")
            }
        }
    }
    #[must_use]
    pub const fn newest(&self) -> u32 {
        self.head.number
    }
    #[must_use]
    pub const fn newest_head(&self) -> &Head {
        &self.head
    }
    #[must_use]
    pub const fn oldest(&self) -> u32 {
        self.oldest
    }
    #[must_use]
    pub fn changed(&self, setting: Setting) -> bool {
        self.changes.get(&setting).is_some()
    }
    /// Ordered setting names; this scratch holds no values.
    #[must_use]
    pub fn offered(&self) -> List<Setting> {
        offered(&self.defaults, &self.changes)
    }
}
#[must_use]
pub fn capacity(limits: &crate::Limits) -> Option<u32> {
    limits.tasks.charters.checked_add(limits.templates)?.checked_add(limits.authority.projects)?.checked_add(1)
}
fn offered(defaults: &Map<Setting, Default>, changes: &Map<Setting, Changed>) -> List<Setting> {
    let mut names =
        Map::with_capacity(defaults.capacity().checked_add(changes.capacity()).expect("settings scratch count"));
    for (&setting, _) in defaults {
        names.insert(setting, ()).expect("default names");
    }
    for (&setting, _) in changes {
        names.insert(setting, ()).expect("change names");
    }
    let mut result = List::with_capacity(names.len());
    for (&setting, ()) in &names {
        if match changes.get(&setting) {
            Some(change) => change.value.is_some(),
            None => defaults.get(&setting).is_some(),
        } {
            result.push(setting).expect("offered names");
        }
    }
    result
}

/// Settings' six record shapes, shared by the core and its store codec.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Record {
    Default { setting: Setting, default: Default },
    Changed { setting: Setting, changed: Changed },
    Revision { history: bool, revision: Revision },
    Extent { oldest: u32 },
    Charter { charter: u32, revision: u32, value: CharterVersion },
    Proposal { proposer: people::Proposer, proposal: u32, value: Proposal },
}
impl Record {
    #[must_use]
    pub fn key(&self) -> Key {
        match self {
            Self::Default { setting, .. } => Key::Default(*setting),
            Self::Changed { setting, .. } => Key::Changed(*setting),
            Self::Revision { history, revision } => {
                if *history {
                    Key::History(revision.head.number)
                } else {
                    Key::Held(revision.head.number)
                }
            }
            Self::Extent { .. } => Key::Extent,
            Self::Charter { charter, revision, .. } => Key::Charter { charter: *charter, revision: *revision },
            Self::Proposal { proposer, proposal, .. } => Key::Proposal { proposer: *proposer, proposal: *proposal },
        }
    }
}

/// One whole set of settings at the bounds the check enforces. The template
/// carrier deliberately shares the pre-existing conservative construction bound.
#[must_use]
pub fn set_worst_case(limits: &crate::Limits) -> Option<u64> {
    let charters = u64::from(limits.tasks.charters)
        .checked_mul(u64::from(limits.run_bytes).checked_add(u64::try_from(size_of::<Charter>()).ok()?)?)?;
    let templates = u64::from(limits.templates).checked_mul(
        u64::try_from(size_of::<Template>())
            .ok()?
            .checked_add(u64::from(limits.documents.text_bytes))?
            .checked_add(crate::memory::task_carrier(limits)?)?,
    )?;
    let policies = u64::from(limits.authority.projects)
        .checked_mul(limits.policy_bytes.checked_add(u64::try_from(size_of::<ConfiguredPolicy>()).ok()?)?)?;
    charters
        .checked_add(templates)?
        .checked_add(policies)?
        .checked_add(authority::rules_worst_case(&limits.authority)?)?
        .checked_add(u64::try_from(size_of::<authority::Rules>()).ok()?)
}
/// One record's whole items, priced independently of the held heads.
#[must_use]
pub fn revision_worst_case(limits: &crate::Limits) -> Option<u64> {
    set_worst_case(limits)?
        .checked_add(List::<Item>::worst_case(capacity(limits)?.checked_mul(2)?)?)?
        .checked_add(u64::try_from(size_of::<Head>()).ok()?)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
impl Current {
    pub(crate) fn test_charter_mut(&mut self, number: u32) -> &mut Charter {
        match &mut self.defaults.get_mut(&Setting::Charter(number)).expect("fixture default charter").value {
            Value::Charter(value) => value,
            Value::Template(_) | Value::Policy(_) | Value::Rules(_) => unreachable!("fixture charter value"),
        }
    }
    pub(crate) fn test_templates(&mut self, templates: Box<[Template]>) {
        let mut old = List::with_capacity(self.defaults.capacity());
        for (&setting, _) in &self.defaults {
            match setting {
                Setting::Template(_) => old.push(setting).expect("fixture defaults"),
                Setting::Charter(_) | Setting::Policy(_) | Setting::Rules => {}
            }
        }
        for setting in old.into_boxed() {
            self.defaults.remove(&setting);
        }
        for template in templates {
            self.defaults
                .insert(
                    Setting::Template(template.number),
                    Default { value: Value::Template(Box::new(template)), revision: 1 },
                )
                .expect("fixture templates");
        }
    }
}

/// The first start or a revision's largest write set, including its deployment
/// header and one steady-state history erase (engine.md, 17.7).
#[must_use]
pub fn writes_worst_case(limits: &crate::Limits) -> Option<u32> {
    let count = capacity(limits)?;
    let first = count
        .checked_add(limits.people.initial_owners)?
        .checked_add(limits.authority.projects.checked_mul(2)?)?
        .checked_add(4)?;
    first
        .max(count.checked_mul(4)?.checked_add(8)?.checked_add(limits.settings.versions.checked_mul(2)?)?)
        .checked_add(withdrawal_writes(limits)?)?
        .checked_add(move_writes(limits)?)
}

/// A party withdrawal takes five writes. A task withdrawal takes two activity
/// rows, its terminal history, up to three live rows (decision, inbox, wake),
/// and the retained set's erase: seven. Settings carry no task stubs.
#[must_use]
pub fn withdrawal_writes(limits: &crate::Limits) -> Option<u32> {
    limits.settings.proposals.checked_mul(7)
}
#[must_use]
pub fn proposal_heads(items: &[Item]) -> Box<[jig_domain_tasks::SettingHead]> {
    let mut heads = List::with_capacity(u32::try_from(items.len()).expect("bounded settings items"));
    for item in items {
        heads
            .push(jig_domain_tasks::SettingHead {
                setting: match item.setting {
                    Setting::Charter(n) => jig_domain_tasks::Setting::Charter(n),
                    Setting::Template(n) => jig_domain_tasks::Setting::Template(n),
                    Setting::Policy(n) => jig_domain_tasks::Setting::Policy(n),
                    Setting::Rules => jig_domain_tasks::Setting::Rules,
                },
                edit: match item.edit {
                    Edit::Set(_) => jig_domain_tasks::SettingEdit::Set,
                    Edit::Removed => jig_domain_tasks::SettingEdit::Removed,
                    Edit::Reset => jig_domain_tasks::SettingEdit::Reset,
                },
            })
            .expect("bounded heads");
    }
    heads.into_boxed()
}

/// The tasks child's revision tables are bounded by the core's settings limits.
pub(crate) fn task_limits(limits: &crate::Limits) -> jig_domain_tasks::Limits {
    let mut tasks = limits.tasks;
    tasks.revisions = limits.settings.revisions;
    tasks.versions = limits.settings.versions;
    tasks
}

/// One decision copies/erases each old head, drops each version, and saves one extent.
pub(crate) fn release_writes(limits: &crate::Limits) -> Option<u32> {
    limits.settings.revisions.checked_mul(2)?.checked_add(limits.settings.versions)?.checked_add(1)
}

/// Child output room uses the same revision limits as the child's allocation.
pub(crate) fn task_room(limits: &crate::Limits) -> u32 {
    jig_domain_tasks::max_out(&task_limits(limits))
}

/// Released names do not expand into ordinary sibling routes. Their total
/// cleanup writes are priced separately, once for the whole decision.
pub(crate) fn task_route_room(limits: &crate::Limits) -> u32 {
    task_room(limits)
        .checked_sub(limits.settings.revisions.checked_add(1).expect("released names"))
        .expect("child room includes released names")
}

/// Each moved live record and at most two projection rows, plus release cleanup.
pub(crate) fn move_writes(limits: &crate::Limits) -> Option<u32> {
    limits.tasks.tasks.checked_mul(3)?.checked_add(release_writes(limits)?)
}

/// The fleet's retained requirements use the core's settings version limit.
pub(crate) fn fleet_limits(limits: &crate::Limits) -> jig_domain_fleet::Limits {
    let mut fleet = limits.fleet;
    fleet.versions = limits.settings.versions;
    fleet
}
