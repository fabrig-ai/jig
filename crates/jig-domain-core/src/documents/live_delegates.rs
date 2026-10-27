//! Complete close-disposal identities and their owner funding (`client/domain/objects.md`, 11).
use super::{Chip, Missing};
use crate::{Core, Limits};
use jig_domain_tasks as tasks;
use skein_lib::List;

/// A direct live delegate and the reservation charged if a party keeps it.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct LiveDelegate {
    pub chip: Chip,
    /// Budget minus spend by this task and ended descendants; live reservations stay included.
    pub takeover_budget: u64,
}

pub(super) fn from_row(
    core: &Core,
    limits: &Limits,
    party: u64,
    row: &tasks::TaskRecord,
) -> Result<List<LiveDelegate>, Missing> {
    if row.delegates.len() > usize::try_from(limits.documents.lines).expect("line count fits") {
        return Err(Missing::TooLarge);
    }
    let mut delegates = List::with_capacity(limits.documents.lines);
    for number in &row.delegates {
        let delegate = super::small::visible(core, party, *number)?;
        if delegate.requester != tasks::Party::Task(row.number) {
            return Err(Missing::Unknown);
        }
        delegates
            .push(LiveDelegate {
                chip: super::small::chip_with_core(core, limits, party, delegate)?,
                takeover_budget: core.tasks.takeover_budget(*number).ok_or(Missing::Unknown)?,
            })
            .expect("complete delegates fit checked bound");
    }
    Ok(delegates)
}
