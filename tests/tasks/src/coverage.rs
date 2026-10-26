//! Semantic lifecycle reach, independent of diagnostic spelling.
use crate::referee::Seen;
use jig_domain_tasks::{Cause, End, Event, Phase, Status};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Reached {
    Done,
    Failed,
    Cancelled,
    Held,
    Refused,
    Restore,
    Turn,
    Priced,
}

pub const MARKERS: [Reached; 8] = [
    Reached::Done,
    Reached::Failed,
    Reached::Cancelled,
    Reached::Held,
    Reached::Refused,
    Reached::Restore,
    Reached::Turn,
    Reached::Priced,
];

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Coverage(pub BTreeSet<Reached>);
impl Coverage {
    pub fn observed(&mut self, seen: &Seen) {
        if let Seen::Ended { status, .. } = seen {
            self.0.insert(match status {
                Status::Done => Reached::Done,
                Status::Failed => Reached::Failed,
                Status::Cancelled => Reached::Cancelled,
            });
        }
        if let Seen::Stored { live, .. } | Seen::Resources { live, .. } = seen
            && live.iter().any(|row| matches!(row.phase, Phase::Held { .. }))
        {
            self.0.insert(Reached::Held);
        }
    }
    pub fn input(&mut self, event: &Event) {
        if matches!(event, Event::Restore { .. } | Event::Check | Event::Restored) {
            self.0.insert(Reached::Restore);
        }
        if matches!(event, Event::Turn { .. }) {
            self.0.insert(Reached::Turn);
        }
        if matches!(event, Event::Activation { end: End::Refused, .. }) {
            self.0.insert(Reached::Refused);
        }
        if matches!(event, Event::Activation { cause: Cause::Priced { .. }, .. }) {
            self.0.insert(Reached::Priced);
        }
    }
}
