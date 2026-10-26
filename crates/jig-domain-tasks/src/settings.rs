//! Settings revisions held by live agent tasks and their runs (domain/engine.md, 17.3).
use crate::domain::{Domain, task_mut};
use crate::{Active, Executor, Phase, SettingsRevision, TaskRecord};

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
}

fn add(domain: &mut Domain, revision: u32) {
    if let Some(count) = domain.holders.get_mut(&revision) {
        *count = count.checked_add(1).expect("bounded live agent tasks");
    } else {
        domain.holders.insert(revision, 1).expect("held revisions fit limits.revisions plus newest");
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
