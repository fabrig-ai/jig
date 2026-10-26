//! Revisions and charter versions retained by live work (engine.md, 17.3 and 17.8).
use super::{CharterVersion, Current, Edit, Head, Item, Key, Record, Setting};
use crate::{Charter, Core, CoreKey, CoreRecord, Limits, Request, Write};
use alloc::boxed::Box;
use skein_lib::{Env, Map, Queue};

/// A charter version's name: its charter and the revision that made it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct VersionKey {
    pub charter: u32,
    pub revision: u32,
}

/// Heads besides the newest, and versions besides its overlay. An erased
/// removal's barrier stays while an older version could otherwise be selected.
#[derive(Debug)]
pub struct Held {
    pub(crate) heads: Map<u32, Head>,
    pub(crate) versions: Map<VersionKey, CharterVersion>,
}

/// A preflight contains only names; values remain owned by Current and Held.
#[derive(Debug)]
pub(crate) struct Plan {
    candidates: Map<VersionKey, ()>,
    keep: Map<VersionKey, ()>,
}
impl Plan {
    pub(crate) fn versions(&self) -> u32 {
        self.keep.len()
    }
}
impl Held {
    pub(crate) fn new(limits: &Limits) -> Self {
        Self {
            // A Moves decision stages the previous newest before releasing old heads.
            heads: Map::with_capacity(limits.settings.revisions.checked_add(1).expect("staged head")),
            versions: Map::with_capacity(limits.settings.versions),
        }
    }

    fn newest_key(&self, current: &Current, charter: u32) -> Option<VersionKey> {
        if let (_, Some((_, revision))) = current.lookup(Setting::Charter(charter)) {
            return Some(VersionKey { charter, revision });
        }
        let mut found = None;
        for (&key, value) in &self.versions {
            if key.charter == charter && value.charter.is_none() {
                found = Some(key);
            }
        }
        found
    }

    fn after_key(&self, current: &Current, items: &[Item], newest: u32, charter: u32) -> Option<VersionKey> {
        for item in items {
            if item.setting == Setting::Charter(charter) {
                return Some(VersionKey { charter, revision: newest });
            }
        }
        self.newest_key(current, charter)
    }

    /// Choose without copying a charter. The overlay can itself be a removal.
    pub(crate) fn selected(&self, current: &Current, charter: u32, revision: u32) -> Option<VersionKey> {
        let mut found = eligible(self.newest_key(current, charter), revision);
        for (&key, _) in &self.versions {
            if key.charter == charter && key.revision <= revision && Some(key) > found {
                found = Some(key);
            }
        }
        found
    }

    pub(crate) fn plan(
        &self,
        current: &Current,
        items: &[Item],
        newest: u32,
        previous: Option<u32>,
        limits: &Limits,
    ) -> Plan {
        let mut held = Map::with_capacity(limits.settings.revisions.checked_add(1).expect("held names"));
        for (&revision, _) in &self.heads {
            held.insert(revision, ()).expect("existing held names");
        }
        if let Some(revision) = previous {
            held.insert(revision, ()).expect("previous newest name");
        }
        self.plan_held(current, items, newest, &held, limits)
    }

    pub(crate) fn plan_held(
        &self,
        current: &Current,
        items: &[Item],
        newest: u32,
        held: &Map<u32, ()>,
        limits: &Limits,
    ) -> Plan {
        let capacity = scratch_capacity(limits).expect("held scratch bound");
        let mut candidates = Map::with_capacity(capacity);
        for (&key, _) in &self.versions {
            candidates.insert(key, ()).expect("existing version names");
        }
        for item in items {
            let Setting::Charter(charter) = item.setting else { continue };
            if let Some(key) = self.newest_key(current, charter) {
                candidates.insert(key, ()).expect("previous newest names");
            }
            if removal(&item.edit) && current.default_value(item.setting).is_none() {
                candidates.insert(VersionKey { charter, revision: newest }, ()).expect("erased removal names");
            }
        }
        let mut plan = Plan { candidates, keep: Map::with_capacity(capacity) };
        for (&key, ()) in &plan.candidates {
            let latest = self.after_key(current, items, newest, key.charter);
            if Some(key) == latest {
                continue;
            }
            let mut read = false;
            for (&revision, ()) in held {
                read |= select(&plan.candidates, latest, key.charter, revision) == Some(key);
            }
            if read {
                plan.keep.insert(key, ()).expect("selected version names");
            }
        }
        // Current names no erased removal. Keep its latest barrier precisely
        // when an older retained version would otherwise leak into the newest.
        for (&key, ()) in &plan.candidates {
            if Some(key) == self.after_key(current, items, newest, key.charter)
                && erased_after(current, items, key.charter)
            {
                let mut older = false;
                for (&kept, ()) in &plan.keep {
                    older |= kept.charter == key.charter && kept.revision < key.revision;
                }
                if older {
                    plan.keep.insert(key, ()).expect("barrier name");
                }
            }
        }
        plan
    }

    /// Commit a preflighted selection, moving existing values only once.
    #[expect(clippy::manual_map, reason = "step code keeps charter ownership explicit without a closure")]
    pub(crate) fn retain(&mut self, current: &Current, plan: &Plan, out: &mut Queue<Request>) -> bool {
        let mut changed = false;
        for (&key, ()) in &plan.candidates {
            if !plan.keep.contains_key(&key) && self.versions.remove(&key).is_some() {
                out.push(Request::Write(Write::Erase(crate::Key::Core(CoreKey::Settings(Key::Charter {
                    charter: key.charter,
                    revision: key.revision,
                })))));
                changed = true;
            }
        }
        for (&key, ()) in &plan.keep {
            if self.versions.contains_key(&key) {
                continue;
            }
            let charter = if self.newest_key(current, key.charter) == Some(key) {
                match current.charter(key.charter) {
                    Some(charter) => Some(Box::new(charter.clone())),
                    None => None,
                }
            } else {
                None // The new erased-removal barrier.
            };
            let value = CharterVersion { charter };
            out.push(Request::Write(Write::Save(crate::Record::Core(CoreRecord::Settings(Record::Charter {
                charter: key.charter,
                revision: key.revision,
                value: value.clone(),
            })))));
            self.versions.insert(key, value).expect("held versions preflighted");
            changed = true;
        }
        changed
    }
}
fn eligible(key: Option<VersionKey>, revision: u32) -> Option<VersionKey> {
    match key {
        Some(key) if key.revision <= revision => Some(key),
        Some(_) | None => None,
    }
}
fn select(
    candidates: &Map<VersionKey, ()>,
    newest: Option<VersionKey>,
    charter: u32,
    revision: u32,
) -> Option<VersionKey> {
    let mut found = eligible(newest, revision);
    for (&key, ()) in candidates {
        if key.charter == charter && key.revision <= revision && Some(key) > found {
            found = Some(key);
        }
    }
    found
}
fn removal(edit: &Edit) -> bool {
    match edit {
        Edit::Removed | Edit::Reset => true,
        Edit::Set(_) => false,
    }
}
fn erased_after(current: &Current, items: &[Item], charter: u32) -> bool {
    for item in items {
        if item.setting == Setting::Charter(charter) {
            return removal(&item.edit) && current.default_value(item.setting).is_none();
        }
    }
    current.lookup(Setting::Charter(charter)).1.is_none()
}

impl Core {
    /// Whether this revision is the newest or is retained by live work.
    #[must_use]
    pub fn held(&self, revision: u32) -> bool {
        revision == self.current.newest() || self.held.heads.contains_key(&revision)
    }

    /// The latest charter state made at or before a newest or held revision.
    #[must_use]
    pub fn charter_at(&self, number: u32, revision: u32) -> Option<&Charter> {
        if revision == self.current.newest() {
            return self.charter(number);
        }
        if !self.held(revision) {
            return None;
        }
        let key = self.held.selected(&self.current, number, revision)?;
        if let (_, Some((_, current))) = self.current.lookup(Setting::Charter(number))
            && key.revision == current
        {
            return self.charter(number);
        }
        self.held.versions.get(&key)?.charter.as_deref()
    }

    pub(crate) fn release_revision(&mut self, env: &Env<Limits>, revision: u32, out: &mut Queue<Request>) {
        if self.held.heads.remove(&revision).is_none() {
            return;
        }
        assert!(self.tasks.holders(revision) == 0, "released revision has no live holder");
        let cutoff = self.current.newest().saturating_sub(env.limits.settings.history).max(1);
        if revision >= cutoff {
            out.push(Request::Write(Write::CopySettingsRevision(revision)));
            self.current.oldest = if self.current.oldest == 0 { revision } else { self.current.oldest.min(revision) };
        }
        out.push(Request::Write(Write::Erase(crate::Key::Core(CoreKey::Settings(Key::Held(revision))))));
        let plan = self.held.plan(&self.current, &[], self.current.newest(), None, &env.limits);
        if self.held.retain(&self.current, &plan, out) {
            self.tell_replaced_settings(env, false, true);
        }
        self.touch_settings();
    }

    /// Coalesce extent changes after all sibling routes, before the decision's mark.
    pub(crate) fn finish_extent(&self, previous: u32, out: &mut Queue<Request>) {
        if previous == self.current.oldest {
            return;
        }
        let mut decided = false;
        for _ in 0..out.len() {
            match out.pop().expect("extent output count") {
                Request::Decided => decided = true,
                Request::Write(Write::Save(crate::Record::Core(CoreRecord::Settings(Record::Extent { .. })))) => {}
                request @ (Request::ProjectMade { .. }
                | Request::Write(_)
                | Request::Ask { .. }
                | Request::Held(_)
                | Request::Now(_)) => out.push(request),
            }
        }
        out.push(Request::Write(Write::Save(crate::Record::Core(CoreRecord::Settings(Record::Extent {
            oldest: self.current.oldest,
        })))));
        if decided {
            out.push(Request::Decided);
        }
    }
}

fn scratch_capacity(limits: &Limits) -> Option<u32> {
    limits.settings.versions.checked_add(super::capacity(limits)?.checked_mul(4)?)
}
/// Retained tables and one decision's two content-free version-name tables.
pub(crate) fn worst_case(limits: &Limits) -> Option<u64> {
    Map::<u32, Head>::worst_case(limits.settings.revisions.checked_add(1)?)?
        .checked_add(Map::<VersionKey, CharterVersion>::worst_case(limits.settings.versions)?)?
        .checked_add(
            u64::from(limits.settings.versions)
                .checked_mul(u64::from(limits.run_bytes).checked_add(u64::try_from(size_of::<Charter>()).ok()?)?)?,
        )?
        .checked_add(Map::<VersionKey, ()>::worst_case(scratch_capacity(limits)?)?.checked_mul(2)?)?
        .checked_add(Map::<u32, ()>::worst_case(limits.settings.revisions.checked_add(1)?)?)
}
