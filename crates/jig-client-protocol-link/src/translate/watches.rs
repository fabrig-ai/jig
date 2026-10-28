//! Watch subjects and terminal reasons (protocol/client.md, 7).
use super::{documents, live, objects, pages};
use crate::{Limits, Undecodable};
use alloc::boxed::Box;
use jig_client_domain::Watchable;
use jig_client_domain::contract as c;
use jig_client_protocol_wire::{VERSION, v19 as w};
use skein_lib::{List, Reader, Writer};

pub fn subject(value: &c::Subject, limits: &w::Limits) -> Result<w::Subject, w::Problem> {
    Ok(match value {
        c::Subject::Settings => w::Subject::Settings,
        c::Subject::Me => w::Subject::Me,
        c::Subject::Task { task } => {
            w::Subject::Task(w::SubjectTask::new(limits, w::SubjectTaskParts { task: *task })?)
        }
        c::Subject::Tree { task, window } => {
            w::Subject::Tree(w::SubjectTree::new(limits, w::SubjectTreeParts { task: *task, window: *window })?)
        }
        c::Subject::Chips { tasks } => {
            w::Subject::Chips(w::SubjectChips::new(limits, w::SubjectChipsParts { tasks: tasks.clone() })?)
        }
        c::Subject::Inbox { window } => {
            w::Subject::Inbox(w::SubjectInbox::new(limits, w::SubjectInboxParts { window: *window })?)
        }
        c::Subject::Tasks { query, window } => w::Subject::Tasks(w::SubjectTasks::new(
            limits,
            w::SubjectTasksParts { query: super::queries::query(*query, limits)?, window: *window },
        )?),
        c::Subject::Run { task, turn, offset } => {
            let turn = match turn {
                Some(turn) => Some(w::ProvisionalTurnName::new(
                    limits,
                    w::ProvisionalTurnNameParts { attempt: turn.attempt, number: turn.number, retry: turn.retry },
                )?),
                None => None,
            };
            w::Subject::Run(w::SubjectRun::new(limits, w::SubjectRunParts { task: *task, turn, offset: *offset })?)
        }
        c::Subject::Deployment => w::Subject::Deployment,
        c::Subject::Accounts => w::Subject::Accounts,
    })
}
pub fn owned(value: Watchable, limits: &w::Limits) -> Result<w::Owned, w::Problem> {
    let (owner, version, record) = match value {
        Watchable::Jig(value) => {
            let value = subject(&value, limits)?;
            let mut writer = Writer::new(usize::try_from(value.measure()).expect("u32 fits usize"));
            value.encode(&mut writer).expect("measured subject");
            (w::Owner::Jig, VERSION, writer.finish())
        }
        Watchable::Family { scope, window, record: value } => (
            w::Owner::Family(w::Family::new(limits, w::FamilyParts { number: value.family })?),
            value.version,
            super::envelope::record(scope, window, value, limits)?,
        ),
    };
    w::Owned::new(limits, w::OwnedParts { owner, version, record })
}
#[must_use]
pub fn refused(value: &w::WatchRefusal) -> c::WatchEnd {
    match value {
        w::WatchRefusal::Unknown => c::WatchEnd::Unknown,
        w::WatchRefusal::NotAllowed => c::WatchEnd::NotAllowed,
        w::WatchRefusal::Busy => c::WatchEnd::Busy,
        w::WatchRefusal::SignedOut => c::WatchEnd::SignedOut,
    }
}
#[must_use]
pub fn ended(value: &w::WatchEnd) -> c::WatchEnd {
    match value {
        w::WatchEnd::NotServed(_) => c::WatchEnd::NotServed,
        w::WatchEnd::Closed => c::WatchEnd::Closed,
        w::WatchEnd::Unknown => c::WatchEnd::Unknown,
        w::WatchEnd::NotAllowed => c::WatchEnd::NotAllowed,
        w::WatchEnd::Busy => c::WatchEnd::Busy,
        w::WatchEnd::SubjectEnded => c::WatchEnd::SubjectEnded,
        w::WatchEnd::SignedOut => c::WatchEnd::SignedOut,
        w::WatchEnd::Stopping => c::WatchEnd::Stopping,
        w::WatchEnd::TooLinked(value) => c::WatchEnd::TooLinked { joins: value.joins() },
    }
}

/// A delivery's discriminator and its whole record are admitted before decoding.
pub fn delivery(bytes: &[u8], limits: &Limits) -> Result<c::Delivery, Undecodable> {
    if crate::limits::worst_case(limits).is_none()
        || u64::try_from(bytes.len()).expect("usize fits u64") > limits.client.document_bytes
    {
        return Err(Undecodable::TooLarge);
    }
    let receiving = receiving(limits)?;
    let value = match w::DeliveryValue::decode(&receiving.wire, &mut Reader::new(bytes)) {
        Ok(value) => value,
        Err(problem) => return Err(documents::classify(problem)),
    };
    Ok(match value {
        w::DeliveryValue::OwnerProject(value) => {
            c::Delivery::OwnerProject(Box::new(documents::owner_project(value, limits)?))
        }
        w::DeliveryValue::OwnerAccounts(value) => {
            c::Delivery::OwnerAccounts(Box::new(match documents::owner_accounts(value, &receiving) {
                Ok(value) => value,
                Err(problem) => return Err(documents::classify(problem)),
            }))
        }
        w::DeliveryValue::Settings(value) => {
            c::Delivery::Settings(Box::new(match super::settings::restore_settings_document(&value, &receiving) {
                Ok(value) => value,
                Err(problem) => return Err(documents::classify(problem)),
            }))
        }
        w::DeliveryValue::Me(value) => c::Delivery::Me(Box::new(documents::frame(value, limits)?)),
        w::DeliveryValue::Task(value) => c::Delivery::Task(Box::new(documents::task(*value, limits)?)),
        w::DeliveryValue::OwnerProposal(value) => {
            c::Delivery::OwnerProposal(Box::new(objects::owner_proposal(value, limits)?))
        }
        w::DeliveryValue::Tree(value) => c::Delivery::Tree(Box::new(documents::tree(value, limits)?)),
        w::DeliveryValue::OwnerTree(value) => c::Delivery::OwnerTree(Box::new(owner_tree(value, limits)?)),
        w::DeliveryValue::Inbox(value) => c::Delivery::Inbox(Box::new(documents::inbox(value, limits)?)),
        w::DeliveryValue::OwnerInbox(value) => c::Delivery::OwnerInbox(Box::new(owner_inbox(value, limits)?)),
        w::DeliveryValue::Tasks(value) => c::Delivery::Tasks(Box::new(documents::tasks(value, limits)?)),
        w::DeliveryValue::Run(value) => c::Delivery::Run(Box::new(live::run_text(value, limits)?)),
        w::DeliveryValue::RunState(value) => c::Delivery::RunState(Box::new(live::run_state(value, limits)?)),
        w::DeliveryValue::Deployment(value) => c::Delivery::Deployment(Box::new(documents::deployment(value, limits)?)),
        w::DeliveryValue::OwnerDeployment(value) => {
            c::Delivery::OwnerDeployment(Box::new(documents::owner_deployment(value, limits)?))
        }
        w::DeliveryValue::Chips(value) => {
            c::Delivery::Chips(match documents::chip_piece(value.rows(), &receiving, limits.client.rows) {
                Ok(value) => value,
                Err(problem) => return Err(documents::classify(problem)),
            })
        }
    })
}
fn receiving(limits: &Limits) -> Result<Limits, Undecodable> {
    let mut checked = pages::receiving(limits)?;
    let bytes = u32::try_from(limits.client.document_bytes).expect("validated receiving bytes");
    checked.wire.chips_rows = bytes;
    checked.wire.owner_tree_rows = bytes;
    checked.wire.owner_tree_folded = limits.client.rows;
    checked.wire.owner_tree_row_proposals = limits.client.rows;
    checked.wire.owner_tree_row_dependencies = bytes;
    checked.wire.owner_inbox_needs = bytes;
    checked.wire.owner_inbox_updates = bytes;
    checked.wire.owner_inbox_selection_some = bytes;
    match w::check_limits(&checked.wire) {
        Ok(value) => value,
        Err(problem) => return Err(documents::classify(problem)),
    }
    Ok(checked)
}
#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
fn owner_inbox(record: w::OwnerInbox, limits: &Limits) -> Result<c::OwnerInbox, Undecodable> {
    let checked = receiving(limits)?;
    let needs = match pages::owner_inbox_entry_piece(record.needs(), &checked, limits.client.rows) {
        Ok(value) => value,
        Err(problem) => return Err(documents::classify(problem)),
    };
    let updates = match pages::owner_inbox_entry_piece(record.updates(), &checked, limits.client.rows) {
        Ok(value) => value,
        Err(problem) => return Err(documents::classify(problem)),
    };
    let selection = match record.selection() {
        Some(value) => Some(match pages::owner_inbox_selection_piece(value, &checked, limits.client.rows) {
            Ok(value) => value,
            Err(problem) => return Err(documents::classify(problem)),
        }),
        None => None,
    };
    let next = match record.next() {
        Some(value) => Some(pages::owner_inbox_cursor_fields(value, &checked)),
        None => None,
    };
    Ok(c::OwnerInbox {
        party: record.party(),
        needs,
        updates,
        next,
        cached_needs: record.cached_needs(),
        cached_updates: documents::count_fields(record.cached_updates(), &checked),
        results_seen: record.results_seen(),
        updates_time_complete: record.updates_time_complete(),
        selection,
    })
}
fn owner_tree(record: w::OwnerTree, limits: &Limits) -> Result<c::OwnerTree, Undecodable> {
    let checked = receiving(limits)?;
    let root = match documents::chip_fields(record.root(), &checked) {
        Ok(value) => value,
        Err(problem) => return Err(documents::classify(problem)),
    };
    let rows = match tree_rows(record.rows(), &checked) {
        Ok(value) => value,
        Err(problem) => return Err(documents::classify(problem)),
    };
    let mut folded = List::with_capacity(record.folded().len());
    for value in record.folded() {
        folded
            .push(c::OwnerTreeFold {
                parent: value.parent(),
                phases: documents::owner_phase_counts_fields(value.phases(), &checked),
                held_below: value.held_below(),
            })
            .expect("bounded folds");
    }
    let next = match record.next() {
        Some(value) => Some(match documents::cursor_fields(value, &checked) {
            Ok(value) => value,
            Err(problem) => return Err(documents::classify(problem)),
        }),
        None => None,
    };
    Ok(c::OwnerTree { root, rows, folded, held_below: record.held_below(), next })
}
fn tree_rows(bytes: &[u8], limits: &Limits) -> Result<List<c::OwnerTreeRow>, w::Problem> {
    let mut reader = Reader::new(bytes);
    let mut rows = List::with_capacity(limits.client.rows);
    let mut sum = 0_u64;
    for _ in 0..=limits.client.rows {
        let item = match skein_codec::piece::split(&mut reader) {
            Ok(value) => value,
            Err(problem) => return Err(super::piece_problem(problem)),
        };
        let Some(item) = item else {
            return Ok(rows);
        };
        sum = sum
            .checked_add(u64::try_from(item.len()).expect("usize fits u64"))
            .ok_or(super::too_large())?
            .checked_add(4)
            .ok_or(super::too_large())?;
        if sum > limits.client.document_bytes {
            return Err(super::too_large());
        }
        let row = w::OwnerTreeRow::decode(&limits.wire, &mut Reader::new(&item))?;
        let mut proposals = List::with_capacity(row.proposals().len());
        for key in row.proposals() {
            proposals.push(super::facts::proposal_key(key, limits)).expect("bounded proposals");
        }
        let row = c::OwnerTreeRow {
            chip: documents::chip_fields(row.chip(), limits)?,
            parent: row.parent(),
            depth: row.depth(),
            proposals,
            spent: row.spent(),
            dependencies: documents::owner_task_link_piece(row.dependencies(), limits, limits.client.rows)?,
            cut: row.cut(),
        };
        if rows.push(row).is_err() {
            return Err(super::too_large());
        }
    }
    Err(super::too_large())
}
