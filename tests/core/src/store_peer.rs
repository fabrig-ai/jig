//! Whole external durable snapshots (`domain/testing.md`, 4, 7;
//! `client/domain/testing.md`, 5–6), independent of root state and replies.
use crate::Store;
use jig_test_domain::{Key, Record};
use skein_lib::Time;
use std::collections::BTreeMap;

/// The start's mark changes only the deployment counter in the typed row mirror
/// (`domain/engine.md`, 6); its wires belong to the component map.
#[must_use]
pub fn with_start_mark(rows: &BTreeMap<Key, Record>) -> BTreeMap<Key, Record> {
    let mut rows = rows.clone();
    let Some(Record::Core(jig_domain_core::Record::Core(jig_domain_core::CoreRecord::Deployment(header)))) =
        rows.get_mut(&Key::Core(jig_domain_core::Key::Core(jig_domain_core::CoreKey::Deployment)))
    else {
        panic!("whole original deployment header");
    };
    header.commits += 1;
    rows
}

/// One complete applied transaction, before its acknowledgement need arrive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DurableSnapshot {
    pub commit: u64,
    pub at: Time,
    pub rows: BTreeMap<Key, Record>,
}

/// Optional external store observer; default worlds do not allocate snapshots.
#[derive(Debug, Default)]
pub struct StorePeer {
    applied: u64,
    snapshots: Vec<DurableSnapshot>,
}

impl StorePeer {
    /// Observe a fresh store without inventing an initial transaction.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Install after an initialized world's already-observed bootstrap.
    #[must_use]
    pub fn after(applied: u64) -> Self {
        Self { applied, snapshots: Vec::new() }
    }

    /// Capture whole externally durable rows with the supplied monotonic time.
    pub fn capture(&mut self, now: Time, store: &Store) {
        if store.applied <= self.applied {
            return;
        }
        self.snapshots.push(DurableSnapshot { commit: store.applied, at: now, rows: store.rows.clone() });
        self.applied = store.applied;
    }

    /// Transfer captured transactions in their actual application order.
    #[must_use]
    pub fn take(&mut self) -> Vec<DurableSnapshot> {
        std::mem::take(&mut self.snapshots)
    }
}
