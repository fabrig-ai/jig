//! Streaming settings restore; only names and revision heads survive a row (engine.md, 17.7–17.8).
use super::{Decider, Edit, Failure, Fault, Head, Item, Key, Record, Revision, Setting};
use crate::{Core, CoreKey, CoreRecord, Limits, Request, Requests, RestartRequest, Write};
use alloc::boxed::Box;
use jig_domain_authority as authority;
use jig_domain_fleet as fleet;
use jig_domain_tasks as tasks;
use skein_lib::{Env, List, Map, Queue};

#[derive(Debug)]
pub(crate) struct Restore {
    pending: Map<Setting, bool>,
    seen: Map<Setting, ()>,
    held: bool,
    extent: bool,
    ready: bool,
    release: Option<u32>,
}
impl Restore {
    pub(crate) fn new(limits: &Limits) -> Self {
        let count = super::capacity(limits).expect("settings bound");
        Self {
            pending: Map::with_capacity(count.checked_mul(2).expect("default union")),
            seen: Map::with_capacity(count),
            held: false,
            extent: false,
            ready: false,
            release: None,
        }
    }
}
impl Core {
    pub(crate) fn restore_setting(&mut self, record: Record, limits: &Limits) -> bool {
        if self.settings_restore.ready {
            return false;
        }
        match record {
            Record::Default { setting, default } => {
                if default.revision == 0 || self.settings_restore.seen.insert(setting, ()).ok() != Some(None) {
                    return false;
                }
                match self.current.defaults.get_mut(&setting) {
                    Some(given) if given.value == default.value => given.revision = default.revision,
                    Some(_) => {
                        if self.settings_restore.pending.insert(setting, true).is_err() {
                            return false;
                        }
                    }
                    None => {
                        if self.settings_restore.pending.insert(setting, false).is_err() {
                            return false;
                        }
                    }
                }
                true
            }
            Record::Changed { setting, changed } => {
                if changed.revision == 0 || self.current.changes.get(&setting).is_some() {
                    return false;
                }
                if let Err(failure) = super::check::admits_change(&self.current, setting, limits) {
                    self.restore_failure = Some(RestartRequest::Settings { failure, changed: true });
                    let _refused = self.restart_refuse();
                    return false;
                }
                if let Some(value) = &changed.value
                    && let Err(failure) = super::check::shape(setting, value, limits)
                {
                    self.restore_failure = Some(RestartRequest::Settings { failure, changed: true });
                    let _refused = self.restart_refuse();
                    return false;
                }
                self.current.changes.insert(setting, changed).is_ok()
            }
            Record::Revision { history, revision } => {
                if history
                    || self.settings_restore.held
                    || revision.head.number == 0
                    || revision.head.against >= revision.head.number
                {
                    return false;
                }
                if let Some((proposer, number)) = revision.head.proposal {
                    let valid = match proposer {
                        jig_domain_people::Proposer::Party(person) => person > 0,
                        jig_domain_people::Proposer::Task(_) => false,
                    };
                    if !valid || number == 0 {
                        return false;
                    }
                }
                self.current.head = revision.head;
                // The decoded items are a row transient, never retained by Current or Restore.
                self.settings_restore.held = true;
                true
            }
            Record::Extent { oldest } => {
                if self.settings_restore.extent {
                    return false;
                }
                self.current.oldest = oldest;
                self.settings_restore.extent = true;
                true
            }
            Record::Charter { .. } => false,
            Record::Proposal { proposer, proposal, value } => {
                let person = match proposer {
                    jig_domain_people::Proposer::Party(person) => person,
                    jig_domain_people::Proposer::Task(_) => return false,
                };
                if proposal == 0
                    || person == 0
                    || value.against == 0
                    || value.items.is_empty()
                    || value.items.len() > usize::try_from(tasks::SETTINGS_ITEMS).expect("item bound")
                    || super::change_shape(&value.items, limits).is_err()
                {
                    return false;
                }
                if self.settings_proposals.contains_key(&(proposer, proposal)) {
                    return false;
                }
                self.settings_proposals.insert((proposer, proposal), value).is_ok()
            }
        }
    }
    /// Finish the settings range before routing any later live row. Children
    /// run ahead of the release commit as they do in every ordinary decision.
    pub fn settings_restored(&mut self, env: &Env<Limits>) -> bool {
        if self.settings_restore.ready || self.restore_failure.is_some() {
            return self.restore_failure.is_none();
        }
        // The empty store is checked and recorded by the existing people-seed
        // continuation. It has no settings range to finish yet.
        if self.counters.deployment().commits == 0 && !self.settings_restore.held {
            return true;
        }
        if !self.settings_restore.held
            || !self.settings_restore.extent
            || self.current.oldest >= self.current.newest() && self.current.oldest != 0
        {
            let _request = self.refuse_settings(Failure { setting: Setting::Rules, fault: Fault::Shape });
            return false;
        }
        for (&setting, default) in &self.current.defaults {
            if self.settings_restore.seen.get(&setting).is_none() {
                assert!(self.settings_restore.pending.insert(setting, true).is_ok(), "bounded added default");
            }
            if default.revision > self.current.newest() {
                return false;
            }
        }
        for (_, changed) in &self.current.changes {
            if changed.revision > self.current.newest() {
                return false;
            }
        }
        if let Err(failure) = super::check(&self.current, &self.settings, &self.connectors, &env.limits) {
            let _request = self.refuse_settings(failure);
            return false;
        }
        let mut changed = false;
        for (&setting, _) in &self.settings_restore.pending {
            changed |= !self.current.changed(setting);
        }
        if changed {
            let previous = self.current.newest();
            let Some(number) = previous.checked_add(1) else {
                let _request = self.refuse_settings(Failure { setting: Setting::Rules, fault: Fault::Count });
                return false;
            };
            self.settings_restore.release = Some(previous);
            self.current.head =
                Head { number, against: previous, at: env.wall, by: Decider::Release, proposal: None, moved: false };
        }
        let newest = self.current.newest();
        for (&setting, _) in &self.settings_restore.pending {
            if let Some(default) = self.current.defaults.get_mut(&setting) {
                default.revision = newest;
            }
        }
        self.settings_restore.ready = true;
        self.tell_settings(env);
        true
    }
    /// The typed refusal is retained even when a later completion repeats it.
    pub fn refuse_settings(&mut self, failure: Failure) -> RestartRequest {
        let changed = self.current.changed(failure.setting);
        let request = RestartRequest::Settings { failure, changed };
        self.restore_failure = Some(request);
        let _refused = self.restart_refuse();
        request
    }

    pub(crate) fn tell_settings(&mut self, env: &Env<Limits>) {
        self.tell_replaced_settings(env, true, true);
    }
    pub(crate) fn tell_replaced_settings(&mut self, env: &Env<Limits>, rules: bool, charters: bool) {
        if rules {
            let mut facts = Queue::with_capacity(authority::POLICY_MAX_OUT);
            authority::step(
                &mut self.authority,
                authority::Event::Rules(Box::new(self.current.rules().clone())),
                &mut facts,
            );
            assert!(facts.is_empty(), "rules replacement has no policy lifecycle fact");
        }
        let versions = charters || self.tasks.newest() != self.current.newest();
        if versions {
            let charters = crate::charters::held_models(self, &env.limits);
            let mut tasks_out = Queue::with_capacity(crate::settings::task_room(&env.limits));
            tasks::step(
                &mut self.tasks,
                &Env { now: env.now, wall: env.wall, limits: super::task_limits(&env.limits) },
                tasks::Event::Charters {
                    newest: self.current.newest(),
                    charters,
                    efforts: crate::charters::efforts(&self.settings),
                },
                &mut tasks_out,
            );
            assert!(tasks_out.is_empty(), "charter table replacement emits nothing");
        }
        if !versions && !rules {
            return;
        }
        let needs = crate::fleet_needs::current(self, &env.limits);
        let mut fleet_out = Queue::with_capacity(fleet::max_out(&env.limits.fleet));
        fleet::step(
            &mut self.fleet,
            &Env { now: env.now, wall: env.wall, limits: super::fleet_limits(&env.limits) },
            fleet::Event::Needs(needs),
            &mut fleet_out,
        );
        assert!(fleet_out.is_empty(), "needs replacement retains running attempts");
    }
    /// Settings rows precede tasks rows, so identity and head equality are checked
    /// only after both ranges restore, before the release withdraws anything.
    pub(crate) fn settings_proposals_consistent(&self) -> bool {
        let against = self.settings_restore.release.unwrap_or(self.current.newest());
        let mut count = 0_u32;
        for (_, row) in self.tasks.person_proposals() {
            match &row.action {
                tasks::PersonAction::Settings { against: revision, items } => {
                    let Ok(number) = u32::try_from(row.number) else {
                        return false;
                    };
                    let Some(set) =
                        self.settings_proposals.get(&(jig_domain_people::Proposer::Party(row.proposer), number))
                    else {
                        return false;
                    };
                    if row.project != 0
                        || set.against != against
                        || *revision != against
                        || super::proposal_heads(&set.items).as_ref() != items.as_ref()
                    {
                        return false;
                    }
                    count = count.checked_add(1).expect("bounded pending sets");
                }
                tasks::PersonAction::Batch(_) | tasks::PersonAction::Amend { .. } => {}
            }
        }
        count == self.settings_proposals.len()
    }
    pub(crate) fn commit_restored_settings(&mut self, env: &Env<Limits>, room: u32) -> Requests {
        let mut out = Queue::with_capacity(room);
        if !self.settings_restored(env) {
            return Requests::Out(out);
        }
        if !self.settings_proposals_consistent() {
            let _refused = self.refuse_settings(Failure { setting: Setting::Rules, fault: Fault::Shape });
            return Requests::Out(out);
        }
        let release = self.settings_restore.release.is_some();
        let reserve = if release { super::withdrawal_writes(&env.limits).expect("withdrawal room") } else { 0 };
        let count = self.settings_restore.pending.len();
        let mut defaults = List::with_capacity(count);
        for (&setting, &present) in &self.settings_restore.pending {
            let write = if present {
                save(Record::Default {
                    setting,
                    default: self.current.defaults.get(&setting).expect("given default").clone(),
                })
            } else {
                Write::Erase(crate::Key::Core(CoreKey::Settings(Key::Default(setting))))
            };
            defaults.push(write).expect("default differences");
        }
        let mut items = List::with_capacity(count);
        for (&setting, &present) in &self.settings_restore.pending {
            if self.current.changed(setting) {
                continue;
            }
            let edit = if present {
                Edit::Set(self.current.defaults.get(&setting).expect("given default").value.clone())
            } else {
                Edit::Removed
            };
            items.push(Item { setting, edit }).expect("release item count");
        }
        let revision = match self.settings_restore.release {
            Some(_) => Some(Revision { head: self.current.head.clone(), items: items.into_boxed() }),
            None => None,
        };
        let (writes, oldest) = revision_writes(
            revision,
            defaults,
            self.current.oldest,
            self.current.newest(),
            env.limits.settings.history,
            room.saturating_sub(reserve),
        );
        self.current.oldest = oldest;
        for write in writes.into_boxed() {
            out.push(Request::Write(write));
        }
        if release {
            crate::routing::settings::withdraw_stale(self, env, &mut out, None);
        }
        self.settings_restore = Restore::new(&env.limits);
        self.settings_restore.ready = true;
        Requests::Out(out)
    }
}
fn save(record: Record) -> Write {
    Write::Save(crate::Record::Core(CoreRecord::Settings(record)))
}

/// A revision's writes and bounded history pruning, shared with changing settings.
/// `room` excludes the deployment header which the journal's root adds.
#[must_use]
pub fn revision_writes(
    revision: Option<Revision>,
    defaults: List<Write>,
    oldest: u32,
    newest: u32,
    history: u32,
    room: u32,
) -> (List<Write>, u32) {
    revision_writes_with_hold(revision, defaults, oldest, newest, history, room, false)
}

pub(crate) fn revision_writes_with_hold(
    revision: Option<Revision>,
    defaults: List<Write>,
    mut oldest: u32,
    newest: u32,
    history: u32,
    room: u32,
    keep_previous: bool,
) -> (List<Write>, u32) {
    let mut writes = List::with_capacity(room);
    for write in defaults.into_boxed() {
        writes.push(write).expect("settings commit preflight");
    }
    if let Some(revision) = revision {
        let previous = revision.head.against;
        if previous != 0 && !keep_previous {
            if previous >= newest.saturating_sub(history).max(1) {
                writes.push(Write::CopySettingsRevision(previous)).expect("revision archive room");
                if oldest == 0 {
                    oldest = previous;
                }
            }
            writes
                .push(Write::Erase(crate::Key::Core(CoreKey::Settings(Key::Held(previous)))))
                .expect("old held row erase room");
        }
        writes.push(save(Record::Revision { history: false, revision })).expect("new held row room");
    }
    let cutoff = newest.saturating_sub(history).max(1);
    let pruning = oldest != 0 && oldest < cutoff;
    let changed = !writes.is_empty();
    if changed || pruning {
        let begin = oldest;
        for _ in begin..cutoff {
            if oldest == 0 || writes.room() <= 1 {
                break;
            }
            writes
                .push(Write::Erase(crate::Key::Core(CoreKey::Settings(Key::History(oldest)))))
                .expect("prune room reserved");
            oldest = oldest.checked_add(1).expect("bounded old revision");
        }
        writes.push(save(Record::Extent { oldest })).expect("extent room reserved");
    }
    (writes, oldest)
}

pub(crate) fn first_start(core: &mut Core, env: &Env<Limits>, out: &mut Queue<Request>) -> bool {
    if let Err(failure) = super::check(&core.current, &core.settings, &core.connectors, &env.limits) {
        let _request = core.refuse_settings(failure);
        return false;
    }
    core.settings_restore.ready = true;
    core.settings_restore.held = true;
    core.settings_restore.extent = true;
    core.current.head.at = env.wall;
    for (&setting, default) in &core.current.defaults {
        out.push(Request::Write(save(Record::Default { setting, default: default.clone() })));
    }
    out.push(Request::Write(save(Record::Revision {
        history: false,
        revision: Revision { head: core.current.head.clone(), items: Box::new([]) },
    })));
    out.push(Request::Write(save(Record::Extent { oldest: 0 })));
    true
}
