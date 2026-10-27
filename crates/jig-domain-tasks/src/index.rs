//! Durable indexes for parties' reads (protocol/store.md, section 7.4).
use skein_lib::Wall;

/// A number ordered newest first by its stored complement.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Newest(u64);
impl Newest {
    #[must_use]
    pub const fn of(value: u64) -> Self {
        Self(!value)
    }
    #[must_use]
    pub const fn value(self) -> u64 {
        !self.0
    }
    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }
    #[must_use]
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }
}

/// A result sorts before a reply of the same task and time.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum UpdateObject {
    Result,
    Reply {
        message: Newest,
        activity: Newest,
    },
    /// A party proposal's retained decision, with no task owner required.
    Proposal {
        proposal: Newest,
    },
}

/// One person's result, reply or decided proposal, saved with the record it names.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Update {
    pub party: u64,
    pub at: Wall,
    pub task: u64,
    pub object: UpdateObject,
}
impl Update {
    #[must_use]
    pub const fn key(self) -> Key {
        Key::Update {
            party: self.party,
            at: Newest::of(self.at.as_nanos()),
            task: Newest::of(self.task),
            object: self.object,
        }
    }
}

/// Logical store `key` for task state or financial history, with no byte encoding performed
/// in this child. (domain/tasks.md, section 2).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Key {
    /// One durable placed hold.
    PlacedHold(u64),
    /// Logical mutable live-task row.
    Live(u64),
    /// Working-set pointer retained while a live task names the ending.
    Stub(u64),
    /// Logical finite period/pool row.
    Ledger(crate::Funder),
    /// One occupied writer slot for a held resource.
    Writer(u64),
    /// Last known capacity for a connector pool.
    Pool(u64),
    /// A person-origin proposal.
    PersonProposal(u64),
    /// Logical historical ended-task row.
    Ended(u64),
    /// A past funding period.
    ClosedLedger(crate::Funder),
    /// A decided person proposal.
    DecidedProposal(u64),
    /// Immutable plan change outside the live arena.
    History {
        task: u64,
        revision: u64,
    },
    /// One immutable lifecycle milestone in a task's history.
    Milestone {
        task: u64,
        position: u64,
    },
    /// One immutable line in the ordered task activity range.
    Activity {
        task: u64,
        position: u64,
    },
    /// One party's update in newest-first order.
    Update {
        party: u64,
        at: Newest,
        task: Newest,
        object: UpdateObject,
    },
    Result {
        party: u64,
        position: u64,
    },
    EndedDelegate {
        requester: u64,
        task: u64,
    },
    Delegate {
        requester: u64,
        task: u64,
    },
    /// Ended tasks by requester and label, newest ended first.
    EndedByParty {
        party: u64,
        label: u32,
        ended_at: Newest,
        task: u64,
    },
    /// Listed ended tasks by project and label, newest ended first.
    EndedByProject {
        project: u32,
        label: u32,
        ended_at: Newest,
        task: u64,
    },
}

impl Key {
    /// Whether a restart loads this family (protocol/store.md, 7.2).
    #[must_use]
    pub const fn live(&self) -> bool {
        match self {
            Self::PlacedHold(_)
            | Self::Live(_)
            | Self::Stub(_)
            | Self::Ledger(_)
            | Self::Writer(_)
            | Self::Pool(_)
            | Self::PersonProposal(_) => true,
            Self::Ended(_)
            | Self::ClosedLedger(_)
            | Self::DecidedProposal(_)
            | Self::History { .. }
            | Self::Milestone { .. }
            | Self::Activity { .. }
            | Self::Update { .. }
            | Self::Result { .. }
            | Self::EndedByParty { .. }
            | Self::EndedByProject { .. }
            | Self::EndedDelegate { .. }
            | Self::Delegate { .. } => false,
        }
    }
}

/// A person's result ordered by its root-issued commit position.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ResultRow {
    pub party: u64,
    pub position: u64,
    pub task: u64,
}

/// Lifetime ended descendants' outcomes, saturating (decisions.md 9, ruling 131).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Endings {
    pub done: u32,
    pub failed: u32,
    pub cancelled: u32,
}
impl Endings {
    pub const ZERO: Self = Self { done: 0, failed: 0, cancelled: 0 };
    pub(crate) fn add(&mut self, other: Self) {
        self.done = self.done.saturating_add(other.done);
        self.failed = self.failed.saturating_add(other.failed);
        self.cancelled = self.cancelled.saturating_add(other.cancelled);
    }
    pub(crate) fn ended(mut self, status: crate::Status) -> Self {
        let count = match status {
            crate::Status::Done => &mut self.done,
            crate::Status::Failed => &mut self.failed,
            crate::Status::Cancelled => &mut self.cancelled,
        };
        *count = count.saturating_add(1);
        self
    }
}

/// A requester-to-delegate edge retained until adoption (store.md 7.3).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Edge {
    pub requester: u64,
    pub task: u64,
}

/// An ended task indexed by its requesting party (protocol/store.md, 7.4).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct EndedByParty {
    pub party: u64,
    pub label: u32,
    pub ended_at: Wall,
    pub task: u64,
}
impl EndedByParty {
    /// Its key, newest ended first within the party and label.
    #[must_use]
    pub const fn key(self) -> Key {
        Key::EndedByParty {
            party: self.party,
            label: self.label,
            ended_at: Newest::of(self.ended_at.as_nanos()),
            task: self.task,
        }
    }
}

/// An ended task indexed by its project when listed (protocol/store.md, 7.4).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct EndedByProject {
    pub project: u32,
    pub label: u32,
    pub ended_at: Wall,
    pub task: u64,
}
impl EndedByProject {
    /// Its key, newest ended first within the project and label.
    #[must_use]
    pub const fn key(self) -> Key {
        Key::EndedByProject {
            project: self.project,
            label: self.label,
            ended_at: Newest::of(self.ended_at.as_nanos()),
            task: self.task,
        }
    }
}
