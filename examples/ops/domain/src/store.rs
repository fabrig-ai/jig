//! Wrap each owner's rows and addresses without inspecting its values.
//! A new application replaces the two connector arms (domain/root.md, 6).
use jig_domain_core as core;
use jig_ops_domain_infrastructure as infrastructure;
use jig_ops_domain_observability as observability;

/// A contiguous family of rows belonging to one child.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Range {
    /// The core's live rows, in its restoration order.
    Core,
    /// Task owner rows used to rebuild the core's derived inbox after positions restore.
    Census(core::Census),
    /// Observability's live rows.
    Observability,
    /// Infrastructure's live rows.
    Infrastructure,
}

/// One child's stable row address.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Key {
    /// A core or core-child address.
    Core(core::Key),
    /// An observability address.
    Observability(observability::RecordKey),
    /// An infrastructure address.
    Infrastructure(infrastructure::RecordKey),
}

/// One child's durable row.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Record {
    /// A core or core-child row.
    Core(core::Record),
    /// An observability row.
    Observability(observability::Record),
    /// An infrastructure row.
    Infrastructure(infrastructure::Record),
}
impl Record {
    /// The row's stable address.
    #[must_use]
    pub fn key(&self) -> Key {
        match self {
            Self::Core(row) => Key::Core(row.key()),
            Self::Observability(row) => Key::Observability(row.key()),
            Self::Infrastructure(row) => Key::Infrastructure(row.key()),
        }
    }
}
/// One mutation in the event's atomic decision.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Write {
    /// Atomically copy the old settings revision to history.
    CopySettingsRevision(u32),
    /// Save the shell-supplied start’s wires.
    Wires,
    /// Save the owner's row at its key.
    Save(Record),
    /// Erase the owner's address.
    Erase(Key),
}
