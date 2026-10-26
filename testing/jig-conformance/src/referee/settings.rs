//! Settings provenance from atomic rows and delivered assignments (domain/testing.md, 6).
use super::{Promise, Task, Violation};

/// A task's retained revision and its run's prepared revision.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct SettingsRevision {
    pub task: u32,
    pub run: Option<u32>,
}

// Share the durable task pass and its previous-row lookup. An unchanged task
// may retain an older revision even when this commit advances newest.
pub(super) fn task(before: Option<&Task>, task: &Task, newest: u32) -> Result<(), Violation> {
    if before.is_none_or(|before| before.settings.task != task.settings.task) && task.settings.task != newest {
        return Err(Violation {
            promise: Promise::Ownership,
            why: format!(
                "task {} was made or moved on settings revision {}, newest is {newest}",
                task.number, task.settings.task
            ),
        });
    }
    Ok(())
}

pub(super) fn assigned(task: &Task, revision: u32) -> Result<(), Violation> {
    if task.settings.run != Some(revision) {
        return Err(Violation {
            promise: Promise::Ownership,
            why: format!(
                "assignment for task {} carries settings revision {revision}, prepared under {:?}",
                task.number, task.settings.run
            ),
        });
    }
    Ok(())
}
