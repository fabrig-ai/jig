//! Claimed tasks' files are cut before the root starts (store.md, 8.4 and 10).
use crate::{Arrived, Below, Event, OpenRefusal, Store, codec, keys, open, pages};
use alloc::boxed::Box;
use jig_domain_core::{CoreRecord, Record};
use skein_io::{file, kernel::Error};
use skein_lib::{Queue, Token};
pub(crate) const OWNER: Token = Token::new(0x5454);
#[derive(Debug)]
enum Stage {
    Idle,
    Opening { length: u64 },
    Cutting { file: Token },
    Syncing { file: Token },
    Closing,
}
#[derive(Debug)]
pub(crate) struct State {
    after: Option<Box<[u8]>>,
    stage: Stage,
}
impl State {
    pub fn new() -> State {
        State { after: None, stage: Stage::Idle }
    }
}
pub(crate) fn begin(store: &mut Store, above: &mut Queue<Event>, below: &mut Below) {
    let result = if crate::maintenance::rehearsal(store) { Ok(true) } else { next(store, below) };
    finish(store, result, above);
}
fn next(store: &mut Store, below: &mut Below) -> Result<bool, ()> {
    let start = match &store.transcripts.cut.after {
        Some(key) => pages::past(key).ok_or(())?,
        None => Box::new([keys::JIG, 0, 2]),
    };
    let range = skein_kv::Range { start, end: Some(Box::new([keys::JIG, 0, 3])) };
    let page = pages::load(store, range).ok_or(())?;
    let Some(row) = page.rows.into_iter().next() else {
        return Ok(true);
    };
    let record = codec::decode(&keys::decode(&row.key).ok_or(())?, &row.value, &store.codec, &store.names).ok_or(())?;
    if keys::encode(&record.key(), store.limits.store.key).ok_or(())? != row.key {
        return Err(());
    }
    let task = match record {
        Record::Core(CoreRecord::RunProof(proof)) => proof.task,
        Record::Core(
            CoreRecord::Settings(_)
            | CoreRecord::Deployment(_)
            | CoreRecord::Projection(_)
            | CoreRecord::Call(_)
            | CoreRecord::Turn(_)
            | CoreRecord::Terminal(_)
            | CoreRecord::ProposalDecision(_)
            | CoreRecord::EscalationDecision(_),
        )
        | Record::Tasks(_)
        | Record::People(_)
        | Record::Notes(_)
        | Record::Accounts(_) => return Err(()),
    };
    store.transcripts.cut.after = Some(row.key);
    let key = keys::encode_own(&keys::Own::Transcript { task }, store.limits.store.key).ok_or(())?;
    let Ok(row) = open::get(store, OWNER, key) else {
        return Err(());
    };
    let length = match row {
        Some(bytes) => {
            let Ok(row) = codec::component::Transcript::decode(&bytes) else {
                return Err(());
            };
            row.length
        }
        None => 0,
    };
    store.transcripts.cut.stage = Stage::Opening { length };
    below.transcripts.push(file::Request::OpenWriteNoFollow {
        owner: OWNER,
        root: store.directory.transcripts.expect("opened transcripts root").fd,
        name: super::path(task),
    });
    Ok(false)
}
pub(crate) fn arrived(store: &mut Store, arrived: Arrived, above: &mut Queue<Event>, below: &mut Below) {
    let Some(event) = arrival(arrived.event) else {
        open::refuse(store, store.directory.owner, OpenRefusal::Failed, above);
        return;
    };
    let stage = core::mem::replace(&mut store.transcripts.cut.stage, Stage::Idle);
    let result = match stage {
        Stage::Opening { length } => match event {
            Arrival::Opened(file) => {
                store.transcripts.cut.stage = Stage::Cutting { file };
                below.transcripts.push(file::Request::Cut { owner: OWNER, file, length });
                Ok(false)
            }
            Arrival::Missing => next(store, below),
            Arrival::Cut | Arrival::Synced | Arrival::Closed | Arrival::Removed => Err(()),
        },
        Stage::Cutting { file } => match event {
            Arrival::Cut => {
                store.transcripts.cut.stage = Stage::Syncing { file };
                below.transcripts.push(file::Request::Sync { owner: OWNER, file });
                Ok(false)
            }
            Arrival::Opened(_) | Arrival::Missing | Arrival::Synced | Arrival::Closed | Arrival::Removed => Err(()),
        },
        Stage::Syncing { file } => match event {
            Arrival::Synced => {
                store.transcripts.cut.stage = Stage::Closing;
                below.transcripts.push(file::Request::Close { owner: OWNER, file });
                Ok(false)
            }
            Arrival::Opened(_) | Arrival::Missing | Arrival::Cut | Arrival::Closed | Arrival::Removed => Err(()),
        },
        Stage::Closing => match event {
            Arrival::Closed => next(store, below),
            Arrival::Opened(_) | Arrival::Missing | Arrival::Cut | Arrival::Synced | Arrival::Removed => Err(()),
        },
        Stage::Idle => Err(()),
    };
    finish(store, result, above);
}
fn finish(store: &mut Store, result: Result<bool, ()>, above: &mut Queue<Event>) {
    match result {
        Ok(true) => {
            if super::drop::restore(store).is_err() {
                open::refuse(store, store.directory.owner, OpenRefusal::Failed, above);
            } else {
                crate::patching::switch::opened(store);
                crate::maintenance::opening_ended(store);
                above.push(Event::Opened {
                    owner: store.directory.owner,
                    last: store.stores.store.last(),
                    installation: store.installation,
                    pending: crate::patching::switch::pending(store),
                });
            }
        }
        Ok(false) => {}
        Err(()) => open::refuse(store, store.directory.owner, OpenRefusal::Failed, above),
    }
}
pub(crate) enum Arrival {
    Opened(Token),
    Missing,
    Cut,
    Synced,
    Closed,
    Removed,
}
pub(crate) fn arrival(event: file::Event) -> Option<Arrival> {
    match event {
        file::Event::Opened { file, .. } => Some(Arrival::Opened(file)),
        file::Event::Cut { .. } => Some(Arrival::Cut),
        file::Event::Synced { .. } => Some(Arrival::Synced),
        file::Event::Closed { .. } => Some(Arrival::Closed),
        file::Event::Removed { .. } => Some(Arrival::Removed),
        file::Event::Failed { error, .. } => {
            if error == Error::NotFound {
                Some(Arrival::Missing)
            } else {
                None
            }
        }
        file::Event::Made { .. }
        | file::Event::Refused { .. }
        | file::Event::Locked { .. }
        | file::Event::Busy { .. }
        | file::Event::Loaded { .. }
        | file::Event::Scanned { .. }
        | file::Event::Stored { .. }
        | file::Event::Conflict { .. }
        | file::Event::TooLarge { .. }
        | file::Event::Stated { .. }
        | file::Event::Written { .. }
        | file::Event::Read { .. }
        | file::Event::Renamed { .. }
        | file::Event::Listed { .. }
        | file::Event::Cancelled { .. } => None,
    }
}
