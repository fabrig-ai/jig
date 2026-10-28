//! Scoped ordering and examined continuations (protocol/store.md, 7.3).
use super::{Key, Position, Range, Row, Scope, notes, tasks};

fn subtree(scope: &Scope) -> bool {
    match scope {
        Scope::History { subtree: true, .. } => true,
        Scope::Task { .. }
        | Scope::Proposal { .. }
        | Scope::Escalation { .. }
        | Scope::Question { .. }
        | Scope::PersonTask { .. }
        | Scope::Result { .. }
        | Scope::Reply { .. }
        | Scope::Turn { .. }
        | Scope::EndedDelegates { .. }
        | Scope::History { subtree: false, .. }
        | Scope::Activity { .. }
        | Scope::Inbox { .. }
        | Scope::Tasks { .. }
        | Scope::Notes { .. }
        | Scope::ScopedNotes { .. }
        | Scope::SettingsHistory { .. }
        | Scope::Holds { .. }
        | Scope::Spend { .. } => false,
    }
}

fn history_after(position: Option<&Position>) -> Option<u64> {
    match position {
        Some(position) => history_task(position),
        None => None,
    }
}

fn history_task(position: &Position) -> Option<u64> {
    match position {
        Position::Owner(Key::Tasks(tasks::Key::History { task, .. } | tasks::Key::Milestone { task, .. })) => {
            Some(*task)
        }
        Position::Owner(_) | Position::Inbox { .. } => None,
    }
}

pub(super) fn later(range: &Range, previous: Option<&Position>, key: &Position) -> bool {
    if subtree(&range.scope) {
        return match previous {
            Some(previous) => match history_task(key) {
                Some(task) => history_task(previous) != Some(task) || key > previous,
                None => false,
            },
            None => history_task(key).is_some(),
        };
    }
    super::later(previous, key, range.backwards)
}

pub(super) fn examined(range: &Range, previous: Option<&Position>, next: &Position) -> bool {
    if subtree(&range.scope) {
        return previous == Some(next) || later(range, previous, next);
    }
    match next {
        Position::Owner(
            Key::Tasks(
                tasks::Key::Update { .. }
                | tasks::Key::EndedByParty { .. }
                | tasks::Key::EndedByProject { .. }
                | tasks::Key::EndedDelegate { .. },
            )
            | Key::Notes(notes::Key::Line { .. }),
        ) => match previous {
            Some(previous) => next >= previous,
            None => true,
        },
        Position::Owner(_) | Position::Inbox { .. } => false,
    }
}

pub(super) fn grouped(range: &Range, raw: &[Row]) -> bool {
    if !subtree(&range.scope) {
        return true;
    }
    let mut previous = history_after(range.after.as_ref());
    for (index, row) in raw.iter().enumerate() {
        let Some(position) = super::row_key(row) else { return false };
        let Some(task) = history_task(&position) else { return false };
        if previous != Some(task) {
            if history_after(range.after.as_ref()) == Some(task) {
                return false;
            }
            for prior in raw.get(..index).expect("prior rows are in the page") {
                let Some(position) = super::row_key(prior) else { return false };
                if history_task(&position) == Some(task) {
                    return false;
                }
            }
        }
        previous = Some(task);
    }
    true
}

#[cfg(test)]
mod tests;
