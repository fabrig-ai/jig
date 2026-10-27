//! A compound crosses as pieces of the same records as standalone asks.
use super::{asks, store};
use crate::Limits;
use alloc::boxed::Box;
use jig_client_domain::contract as c;
use jig_client_protocol_wire::v19 as w;
use skein_lib::{List, Reader, Writer};
pub(super) fn encode(project: u32, values: &List<c::Operation>, limits: &w::Limits) -> Result<w::Ask, w::Problem> {
    let mut records = List::with_capacity(values.len());
    let mut size = 0_u32;
    for value in values {
        let record = operation(value, limits)?;
        size = size.checked_add(record.measure()).ok_or(problem())?.checked_add(4).ok_or(problem())?;
        if size > limits.compound_operations {
            return Err(problem());
        }
        records.push(record).expect("same capacity");
    }
    let mut out = Writer::new(usize::try_from(size).expect("wire size fits"));
    for record in records.into_boxed() {
        out.put(&record.measure().to_be_bytes()).expect("measured piece");
        record.encode(&mut out).expect("measured operation");
    }
    Ok(w::Ask::Compound(w::Compound::new(limits, w::CompoundParts { project, operations: out.finish() })?))
}
fn operation(value: &c::Operation, limits: &w::Limits) -> Result<w::Operation, w::Problem> {
    let ask = match value.clone() {
        c::Operation::Message { project, task, words } => c::Ask::Message { project, task, words },
        c::Operation::Amend { project, task, revision, change, reason } => {
            c::Ask::Amend { project, task, revision, change, reason }
        }
        c::Operation::Cancel { project, task, reason, grace } => c::Ask::Cancel { project, task, reason, grace },
        c::Operation::Release { project, task, revision, again } => c::Ask::Release { project, task, revision, again },
        c::Operation::Stop { project, task } => c::Ask::Stop { project, task },
        c::Operation::Move { project, task, reason } => c::Ask::Move { project, task, reason },
    };
    Ok(match asks::ask(&ask, limits)? {
        w::Ask::Message(v) => w::Operation::Message(v),
        w::Ask::Amend(v) => w::Operation::Amend(v),
        w::Ask::Cancel(v) => w::Operation::Cancel(v),
        w::Ask::Release(v) => w::Operation::Release(v),
        w::Ask::Stop(v) => w::Operation::Stop(v),
        w::Ask::MoveTask(v) => w::Operation::MoveTask(v),
        w::Ask::Lift(_)
        | w::Ask::ChangeSettings(_)
        | w::Ask::MoveToNewest(_)
        | w::Ask::Propose(_)
        | w::Ask::Withdraw(_)
        | w::Ask::Create(_)
        | w::Ask::Compound(_)
        | w::Ask::Answer(_)
        | w::Ask::Take(_)
        | w::Ask::HandBack(_)
        | w::Ask::Decide(_)
        | w::Ask::Seen(_)
        | w::Ask::EditNote(_)
        | w::Ask::DeleteNote(_)
        | w::Ask::ChangePolicy(_)
        | w::Ask::SetRoles(_)
        | w::Ask::MakeService(_)
        | w::Ask::SignIn(_)
        | w::Ask::AccountChange(_)
        | w::Ask::SignOut(_)
        | w::Ask::Rename(_)
        | w::Ask::MakeProject(_)
        | w::Ask::SetPool(_) => return Err(problem()),
    })
}
pub(super) fn restore(value: w::Compound, limits: &Limits) -> Result<c::Ask, w::Problem> {
    let mut reader = Reader::new(value.operations());
    let mut count = 0_u32;
    while piece(&mut reader)?.is_some() {
        count = count.checked_add(1).ok_or(problem())?;
    }
    let mut operations = List::with_capacity(count);
    let mut reader = Reader::new(value.operations());
    while let Some(bytes) = piece(&mut reader)? {
        let record = w::Operation::decode(&limits.wire, &mut Reader::new(&bytes))?;
        let ask = match record {
            w::Operation::Message(v) => w::Ask::Message(v),
            w::Operation::Amend(v) => w::Ask::Amend(v),
            w::Operation::Cancel(v) => w::Ask::Cancel(v),
            w::Operation::Release(v) => w::Ask::Release(v),
            w::Operation::Stop(v) => w::Ask::Stop(v),
            w::Operation::MoveTask(v) => w::Ask::MoveTask(v),
        };
        let operation = match store::restore_ask(ask, limits)? {
            c::Ask::Message { project, task, words } => c::Operation::Message { project, task, words },
            c::Ask::Amend { project, task, revision, change, reason } => {
                c::Operation::Amend { project, task, revision, change, reason }
            }
            c::Ask::Cancel { project, task, reason, grace } => c::Operation::Cancel { project, task, reason, grace },
            c::Ask::Release { project, task, revision, again } => {
                c::Operation::Release { project, task, revision, again }
            }
            c::Ask::Stop { project, task } => c::Operation::Stop { project, task },
            c::Ask::Move { project, task, reason } => c::Operation::Move { project, task, reason },
            c::Ask::Lift { .. }
            | c::Ask::ChangeSettings(_)
            | c::Ask::MoveToNewest(_)
            | c::Ask::Propose { .. }
            | c::Ask::Withdraw { .. }
            | c::Ask::Create { .. }
            | c::Ask::Compound { .. }
            | c::Ask::Answer { .. }
            | c::Ask::Take { .. }
            | c::Ask::HandBack { .. }
            | c::Ask::Decide { .. }
            | c::Ask::Seen { .. }
            | c::Ask::EditNote { .. }
            | c::Ask::DeleteNote { .. }
            | c::Ask::ChangePolicy { .. }
            | c::Ask::SetRoles { .. }
            | c::Ask::MakeService { .. }
            | c::Ask::SignIn { .. }
            | c::Ask::AccountChange { .. }
            | c::Ask::SignOut { .. }
            | c::Ask::Rename { .. }
            | c::Ask::MakeProject { .. }
            | c::Ask::SetPool { .. } => return Err(problem()),
        };
        operations.push(operation).expect("same capacity");
    }
    Ok(c::Ask::Compound { project: value.project(), operations })
}
fn problem() -> w::Problem {
    w::Problem { path: w::Path::CompoundOperations, reason: skein_codec::Reason::Bound }
}

fn piece(reader: &mut Reader<'_>) -> Result<Option<Box<[u8]>>, w::Problem> {
    match skein_codec::piece::split(reader) {
        Ok(value) => Ok(value),
        Err(_) => Err(problem()),
    }
}
