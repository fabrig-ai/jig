//! Settings provenance from atomic rows and delivered assignments (domain/testing.md, 6).
use super::{Promise, Snapshot, Task, Violation, require};

/// A task's retained revision and its run's prepared revision.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct SettingsRevision {
    pub task: u32,
    pub run: Option<u32>,
}

pub(super) fn durable(last: &Snapshot, now: &Snapshot) -> Result<(), Violation> {
    for (&number, task) in &now.tasks {
        let before = last.tasks.get(&number);
        if before.is_none_or(|before| before.settings.task != task.settings.task) {
            require(
                task.settings.task == now.newest,
                Promise::Ownership,
                format!(
                    "task {number} was made or moved on settings revision {}, newest is {}",
                    task.settings.task, now.newest
                ),
            )?;
        }
    }
    Ok(())
}

pub(super) fn assigned(task: &Task, revision: u32) -> Result<(), Violation> {
    require(
        task.settings.run == Some(revision),
        Promise::Ownership,
        format!(
            "assignment for task {} carries settings revision {revision}, prepared under {:?}",
            task.number, task.settings.run
        ),
    )
}
