//! Immutable task activity in commit order (domain/engine.md, sections 5.4, 5.5 and 7.2).
//! Only the last position is live. Messages retain their original admission,
//! before unread hints merge; history and lifecycle rows are named for demand loads.
use crate::{MessageKind, Party, Request, Stored, TaskRecord, Word};
use alloc::boxed::Box;
use skein_lib::{Queue, Wall};

/// One immutable line; its position never depends on wall-clock order.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ActivityLine {
    pub task: u64,
    pub position: u64,
    pub at: Wall,
    pub line: Line,
}

/// Ordered semantic activity; larger retained records are loaded by their exact identity.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Line {
    Words {
        message: u64,
        by: Party,
        kind: MessageKind,
        words: Box<[u8]>,
    },
    Heard(Word),
    /// A notice sent by this task, kept after its recipient's waiting entry clears.
    Notified {
        recipient: Party,
        word: Word,
    },
    RunBegan {
        attempt: u64,
        why: Why,
        resumed: bool,
    },
    RunEnded {
        attempt: u64,
        end: EndKind,
        spent: u64,
    },
    Turn {
        reply: u64,
        attempt: u64,
        turn: u32,
        read: Option<u64>,
    },
    Decided {
        revision: u64,
    },
    Milestone {
        position: u64,
    },
    /// The run finished done while its task was kept, and parked: the
    /// dependencies it waits for, at most `Limits.dependencies`
    /// (domain/tasks.md, 5.7; domain/engine.md, 7).
    Kept {
        attempt: u64,
        pending: Box<[u64]>,
    },
}

/// The durable cause carried through preparation to the committed claim.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Why {
    Made,
    Ready,
    Message {
        message: u64,
    },
    Released,
    Retry {
        tries: crate::Tries,
    },
    Narrowed,
    /// A procedure's connector or writer inputs changed.
    Changed,
}

/// The accepted terminal outcome; result bytes remain in their existing rows.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum EndKind {
    Parked,
    Finished,
    Cancelled,
    Failed { class: crate::Class },
    Refused,
}

pub(crate) fn append(task: &mut TaskRecord, at: Wall, line: Line, out: &mut Queue<Request>) {
    task.activity = task.activity.checked_add(1).expect("task activity position fits");
    out.push(Request::Save {
        record: Stored::Activity(ActivityLine { task: task.number, position: task.activity, at, line }),
    });
}

pub(crate) fn message(task: &mut TaskRecord, word: &Word, bound: u32, out: &mut Queue<Request>) {
    let line = match word.kind {
        MessageKind::Words | MessageKind::Question | MessageKind::Answer { .. } => {
            Line::Words { message: word.number, by: word.from, kind: word.kind.clone(), words: word.words.clone() }
        }
        MessageKind::EffectNotMade { .. }
        | MessageKind::Escalation { .. }
        | MessageKind::Proposal { .. }
        | MessageKind::SettingsDecision { .. }
        | MessageKind::ProposalDecision { .. }
        | MessageKind::FinishRequested { .. }
        | MessageKind::Amendment { .. }
        | MessageKind::Met { .. }
        | MessageKind::Notice { .. }
        | MessageKind::Timer { .. }
        | MessageKind::News { .. }
        | MessageKind::PersonStalled { .. }
        | MessageKind::Result(_) => Line::Heard(word.clone()),
    };
    append(task, word.at, line, out);
    crate::positions::message(task, word, bound);
}

/// Maximum complete bytes in an emitted line; referenced history/turn rows have their own bounds.
#[must_use]
pub fn line_bytes_limit(limits: &crate::Limits) -> Option<u64> {
    u64::try_from(size_of::<ActivityLine>()).ok()?.checked_add(
        u64::from(limits.message_bytes.max(limits.result_bytes))
            .checked_mul(u64::try_from(size_of::<crate::ResultField>()).ok()?.checked_add(1)?)?,
    )
}

/// Whole-decision line ceiling: admission, decisions and lifecycle cascades,
/// plus one notice for each bounded standing interest. Included in `max_out`.
#[must_use]
pub fn lines_per_decision(limits: &crate::Limits) -> Option<u32> {
    limits.tasks.checked_mul(12)?.checked_add(limits.tasks.checked_mul(limits.subscriptions)?)
}
