//! A compound carries the same bounded task operations as standalone asks.
use crate::{Amendment, Ask, LimitKind, Limits, Refusal};
use alloc::boxed::Box;
use skein_lib::{Duration, List};

/// One task operation, whose project is named once by its compound.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Operation {
    Cancel { task: u64, reason: Box<[u8]>, grace: Option<Duration> },
    Move { task: u64, reason: Box<[u8]> },
    Amend { task: u64, revision: u64, amendment: Box<Amendment>, release: Option<u64> },
    Release { task: u64, revision: u64, again: Option<bool> },
    Message { task: u64, words: Box<[u8]> },
    Stop { task: u64 },
}
impl Operation {
    /// The standalone vocabulary, with its enclosing project's number.
    #[must_use]
    pub(crate) fn into_ask(self, project: u32) -> Ask {
        match self {
            Self::Cancel { task, reason, grace } => Ask::Cancel { project, task, reason, grace },
            Self::Move { task, reason } => Ask::Move { project, task, reason },
            Self::Amend { task, revision, amendment, release } => {
                Ask::Amend { project, task, revision, amendment: *amendment, release }
            }
            Self::Release { task, revision, again } => Ask::Release { project, task, revision, again },
            Self::Message { task, words } => Ask::Say { project, task, words },
            Self::Stop { task } => Ask::Stop { project, task },
        }
    }
}
pub(crate) fn refusal(limits: &Limits, operations: &List<Operation>) -> Option<Refusal> {
    if operations.is_empty() || operations.len() > limits.operations {
        return Some(if operations.is_empty() {
            Refusal::Limit { which: LimitKind::Operations, bound: None }
        } else {
            Refusal::configured(
                LimitKind::Operations,
                u64::from(limits.operations),
                crate::LimitKey::PeopleOperations,
                crate::LimitUnit::Items,
            )
        });
    }
    for operation in operations {
        // Bound the temporary standalone ask before copying any of its owned payload.
        let (bytes, maximum, which) = match operation {
            Operation::Cancel { reason, .. } | Operation::Move { reason, .. } => {
                (u64::try_from(reason.len()).ok(), limits.words, LimitKind::Words)
            }
            Operation::Message { words, .. } => (u64::try_from(words.len()).ok(), limits.words, LimitKind::Words),
            Operation::Amend { amendment, .. } => {
                (crate::amendment_bytes(amendment), limits.amendment_bytes, LimitKind::Amendment)
            }
            Operation::Release { .. } | Operation::Stop { .. } => (Some(0), 0, LimitKind::Words),
        };
        let Some(bytes) = bytes else {
            return Some(Refusal::configured(
                which,
                u64::from(maximum),
                if which == LimitKind::Amendment {
                    crate::LimitKey::PeopleAmendmentBytes
                } else {
                    crate::LimitKey::PeopleWords
                },
                crate::LimitUnit::Bytes,
            ));
        };
        if bytes > u64::from(maximum) {
            return Some(Refusal::configured(
                which,
                u64::from(maximum),
                if which == LimitKind::Amendment {
                    crate::LimitKey::PeopleAmendmentBytes
                } else {
                    crate::LimitKey::PeopleWords
                },
                crate::LimitUnit::Bytes,
            ));
        }
        let ask = operation.clone().into_ask(1);
        if !crate::domain::valid_ask(limits, &ask) {
            return Some(crate::domain::invalid_ask(limits, &ask));
        }
    }
    None
}
