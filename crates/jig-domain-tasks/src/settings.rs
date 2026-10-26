//! Settings revisions held by live agent tasks and their runs (domain/engine.md, 17.3).
use crate::domain::{Domain, task_mut};
use crate::{Active, Executor, Phase, Request, SettingsRevision, TaskRecord};
use alloc::boxed::Box;
use skein_lib::{List, Queue};

impl Domain {
    /// The newest revision named by the last charter replacement.
    #[must_use]
    pub const fn newest(&self) -> u32 {
        self.newest
    }

    /// The latest version made at or before `revision`, absent after a removal.
    #[must_use]
    pub fn charter_models(&self, charter: u32, revision: u32) -> Option<&[u8]> {
        let mut found = None;
        let mut latest = 0;
        for version in &self.charters {
            if version.number == charter && version.revision <= revision && version.revision >= latest {
                latest = version.revision;
                found = version.models.as_deref();
            }
        }
        found
    }

    /// Live agent tasks recording this revision as their task's or run's, counted once.
    #[must_use]
    pub fn holders(&self, revision: u32) -> u32 {
        self.holders.get(&revision).copied().unwrap_or(0)
    }

    /// Live holders in task-number order, bounded in the refusal with an exact remainder.
    #[must_use]
    pub fn holding(&self, revision: u32, limit: u32) -> (Box<[u64]>, u32) {
        let mut tasks = List::with_capacity(limit);
        let mut more = 0_u32;
        for (&number, _) in &self.names {
            let task = self.task(number).expect("indexed live task");
            let agent = match task.executor {
                Executor::Agent { .. } => true,
                Executor::Procedure { .. } | Executor::Person(_) => false,
            };
            if agent && (task.settings.task == revision || task.settings.run == Some(revision)) {
                if tasks.len() < limit {
                    tasks.push(number).expect("bounded named holders");
                } else {
                    more = more.checked_add(1).expect("bounded holders");
                }
            }
        }
        (tasks.into_boxed(), more)
    }
}

fn add(domain: &mut Domain, revision: u32) {
    add_to(&mut domain.holders, revision);
}

fn add_to(holders: &mut skein_lib::Map<u32, u32>, revision: u32) {
    if let Some(count) = holders.get_mut(&revision) {
        *count = count.checked_add(1).expect("bounded live agent tasks");
    } else {
        holders.insert(revision, 1).expect("held revisions fit limits.revisions plus newest");
    }
}

fn remove(domain: &mut Domain, revision: u32) {
    let count = domain.holders.get_mut(&revision).expect("revision held by live task");
    *count = count.checked_sub(1).expect("revision holder counted");
    if *count == 0 {
        domain.holders.remove(&revision);
    }
}

fn hold_settings(domain: &mut Domain, settings: SettingsRevision) {
    add(domain, settings.task);
    if let Some(run) = settings.run
        && run != settings.task
    {
        add(domain, run);
    }
}

fn release_settings(domain: &mut Domain, settings: SettingsRevision) {
    remove(domain, settings.task);
    if let Some(run) = settings.run
        && run != settings.task
    {
        remove(domain, run);
    }
}

pub(crate) fn hold(domain: &mut Domain, task: &TaskRecord) {
    match task.executor {
        Executor::Agent { .. } => hold_settings(domain, task.settings),
        Executor::Procedure { .. } | Executor::Person(_) => {}
    }
}

pub(crate) fn release(domain: &mut Domain, task: &TaskRecord) {
    match task.executor {
        Executor::Agent { .. } => release_settings(domain, task.settings),
        Executor::Procedure { .. } | Executor::Person(_) => {}
    }
}

pub(crate) fn publish(domain: &mut Domain, number: u64) {
    let task = task_mut(domain, number).expect("published task exists");
    if task.record.phase != Phase::Active(Active::Preparing) && crate::run::run_attempt(&task.record.phase).is_none() {
        task.record.settings.run = None;
    }
    match task.record.executor {
        Executor::Procedure { .. } | Executor::Person(_) => return,
        Executor::Agent { .. } => {}
    }
    let after = task.record.settings;
    if let Some(previous) = &task.published {
        let before = previous.settings;
        if before == after {
            return;
        }
        release_settings(domain, before);
    }
    hold_settings(domain, after);
}

/// Snapshot names, not transient decrements: a republished holder is no release.
pub(crate) fn before(domain: &Domain) -> List<u32> {
    let mut revisions = List::with_capacity(domain.holders.capacity());
    for (&revision, _) in &domain.holders {
        revisions.push(revision).expect("bounded held revisions");
    }
    revisions
}
pub(crate) fn released(domain: &Domain, before: List<u32>, out: &mut Queue<Request>) {
    for &revision in &before {
        if domain.holders(revision) == 0 {
            out.push(Request::Released { revision });
        }
    }
}

/// Recount after the whole move, avoiding a temporary extra holder slot.
pub(crate) fn recount(domain: &mut Domain) {
    while let Some((&revision, _)) = domain.holders.first() {
        domain.holders.remove(&revision);
    }
    for (_, &id) in &domain.names {
        let task = &domain.tasks.get(id).expect("live task").record;
        match task.executor {
            Executor::Agent { .. } => {}
            Executor::Procedure { .. } | Executor::Person(_) => continue,
        }
        add_to(&mut domain.holders, task.settings.task);
        if let Some(run) = task.settings.run
            && run != task.settings.task
        {
            add_to(&mut domain.holders, run);
        }
    }
}
