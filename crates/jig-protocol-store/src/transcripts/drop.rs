//! Remove a transcript before its commit; erase its orphan places later (store.md, 6 and 8.5).
use super::{
    File, Stage,
    cut::{self, Arrival},
};
use crate::{Arrived, Below, Event, Limits, Root, Store, keys, open, pages, transcripts};
use alloc::boxed::Box;
use skein_io::file;
use skein_lib::{Env, List, Queue, Token};
pub(crate) const OWNER: Token = Token::new(0x5455);
#[derive(Debug)]
struct Orphan {
    task: u64,
    after: Option<u64>,
    through: u64,
}
#[derive(Debug)]
enum Step {
    Idle,
    FileClose,
    Opening,
    Removing,
    Syncing,
    DirectoryClose,
}
#[derive(Debug)]
pub(crate) struct State {
    task: u64,
    directory: Option<Root>,
    step: Step,
    orphans: Queue<Orphan>,
}
impl State {
    pub fn new(_limits: &Limits) -> State {
        State { task: 0, directory: None, step: Step::Idle, orphans: Queue::uncapped() }
    }
}
// Bound a sweep's work by the disk budget and admitted transcript reservations.
fn capacity(limits: &Limits) -> Option<u32> {
    // Each task needs at least one 18-byte attempt key in the map or a reservation.
    u32::try_from(limits.store.budget.checked_div(18)?).ok()?.checked_add(super::reservation_count(limits)?)
}
fn attempt_key(store: &mut Store, task: u64, after: Option<u64>) -> Result<Option<keys::Own>, ()> {
    let first = match after {
        Some(last) => match last.checked_add(1) {
            Some(next) => next,
            None => return Ok(None),
        },
        None => 0,
    };
    let start = keys::encode_own(&keys::Own::Attempt { task, attempt: first }, store.limits.store.key).ok_or(())?;
    let end = match task.checked_add(1) {
        Some(next) => {
            keys::encode_own(&keys::Own::Attempt { task: next, attempt: 0 }, store.limits.store.key).ok_or(())?
        }
        None => Box::new([keys::COMPONENT, 3]),
    };
    let page = pages::load(store, skein_kv::Range { start, end: Some(end) }).ok_or(())?;
    let Some(row) = page.rows.into_iter().next() else {
        return Ok(None);
    };
    Ok(Some(keys::decode_own(&row.key).ok_or(())?))
}
pub(crate) fn queue(store: &mut Store, task: u64, through: u64) -> Option<()> {
    for orphan in &store.transcripts.drop.orphans {
        if orphan.task == task {
            return Some(());
        }
    }
    let mut pending = false;
    for ((found, _), _) in &store.transcripts.attempts {
        if *found == task {
            pending = true;
            break;
        }
    }
    if !pending && attempt_key(store, task, None).ok()?.is_none() {
        return Some(());
    }
    store.transcripts.drop.orphans.push(Orphan { task, after: None, through });
    Some(())
}
pub(crate) fn erases(store: &mut Store, ops: &mut List<skein_kv::Op>) -> Option<()> {
    for _ in 0..store.limits.orphan_erases {
        let Some(mut orphan) = store.transcripts.drop.orphans.pop() else { break };
        if orphan.through > store.stores.store.last() {
            store.transcripts.drop.orphans.push(orphan);
            break;
        }
        match attempt_key(store, orphan.task, orphan.after).ok()? {
            Some(key @ keys::Own::Attempt { task, attempt }) => {
                if task != orphan.task {
                    return None;
                }
                let key = keys::encode_own(&key, store.limits.store.key)?;
                ops.push(skein_kv::Op::Erase { key }).ok()?;
                orphan.after = Some(attempt);
                if attempt_key(store, orphan.task, orphan.after).ok()?.is_some() {
                    store.transcripts.drop.orphans.push(orphan);
                }
            }
            Some(keys::Own::Format | keys::Own::Layouts | keys::Own::Wires | keys::Own::Transcript { .. }) => {
                return None;
            }
            None => {}
        }
    }
    Some(())
}
pub(crate) fn restore(store: &mut Store) -> Result<(), ()> {
    let mut start: Box<[u8]> = Box::new([keys::COMPONENT, 2]);
    for _ in 0..=capacity(&store.limits).ok_or(())? {
        let page = pages::load(store, skein_kv::Range { start, end: Some(Box::new([keys::COMPONENT, 3])) }).ok_or(())?;
        let Some(row) = page.rows.into_iter().next() else {
            return Ok(());
        };
        let key = keys::decode_own(&row.key).ok_or(())?;
        let task = match key {
            keys::Own::Attempt { task, .. } => task,
            keys::Own::Format | keys::Own::Layouts | keys::Own::Wires | keys::Own::Transcript { .. } => return Err(()),
        };
        let place = keys::encode_own(&keys::Own::Transcript { task }, store.limits.store.key).ok_or(())?;
        let Ok(place) = open::get(store, OWNER, place) else {
            return Err(());
        };
        if place.is_none() {
            queue(store, task, 0).ok_or(())?;
        }
        start = pages::past(&row.key).ok_or(())?;
    }
    Err(())
}
pub(crate) fn begin(store: &mut Store, number: u64, tasks: &[u64], _above: &mut Queue<Event>, below: &mut Below) {
    assert!(tasks.len() == 1, "the writer removes one queued task at a time");
    assert!(
        store.transcripts.current.as_ref().expect("active commit").number == number,
        "drop belongs to active commit"
    );
    let task = *tasks.first().expect("one dropped task");
    store.transcripts.drop.task = task;
    store.transcripts.stage = Stage::Dropping;
    if let Some(file) = store.transcripts.files.get(&task) {
        store.transcripts.drop.step = Step::FileClose;
        below.transcripts.push(file::Request::Close { owner: OWNER, file: file.token });
    } else {
        opening(store, below);
    }
}
fn opening(store: &mut Store, below: &mut Below) {
    store.transcripts.drop.step = Step::Opening;
    below.transcripts.push(file::Request::OpenDirectory {
        owner: OWNER,
        root: store.directory.transcripts.expect("transcripts root").fd,
        name: super::name(store.transcripts.drop.task >> 12),
        no_follow: true,
    });
}
pub(crate) fn arrived(
    store: &mut Store,
    env: &Env<Limits>,
    arrived: Arrived,
    above: &mut Queue<Event>,
    below: &mut Below,
) {
    let fd = arrived.fd;
    let Some(event) = cut::arrival(arrived.event) else {
        transcripts::failed(store, None, above);
        return;
    };
    let step = core::mem::replace(&mut store.transcripts.drop.step, Step::Idle);
    let result = match step {
        Step::FileClose => match event {
            Arrival::Closed => {
                let _: Option<File> = store.transcripts.files.remove(&store.transcripts.drop.task);
                opening(store, below);
                Some(false)
            }
            Arrival::Opened(_) | Arrival::Missing | Arrival::Cut | Arrival::Synced | Arrival::Removed => None,
        },
        Step::Opening => match event {
            Arrival::Opened(token) => match fd {
                Some(fd) => {
                    store.transcripts.drop.directory = Some(Root { token, fd });
                    store.transcripts.drop.step = Step::Removing;
                    below.transcripts.push(file::Request::Remove {
                        owner: OWNER,
                        root: fd,
                        name: super::name(store.transcripts.drop.task),
                        directory: false,
                    });
                    Some(false)
                }
                None => None,
            },
            Arrival::Missing => Some(true),
            Arrival::Cut | Arrival::Synced | Arrival::Closed | Arrival::Removed => None,
        },
        Step::Removing => match event {
            Arrival::Removed | Arrival::Missing => {
                store.transcripts.drop.step = Step::Syncing;
                below.transcripts.push(file::Request::SyncDirectory {
                    owner: OWNER,
                    root: store.transcripts.drop.directory.expect("opened directory").fd,
                });
                Some(false)
            }
            Arrival::Opened(_) | Arrival::Cut | Arrival::Synced | Arrival::Closed => None,
        },
        Step::Syncing => match event {
            Arrival::Synced => {
                store.transcripts.drop.step = Step::DirectoryClose;
                below.transcripts.push(file::Request::Close {
                    owner: OWNER,
                    file: store.transcripts.drop.directory.expect("synced directory").token,
                });
                Some(false)
            }
            Arrival::Opened(_) | Arrival::Missing | Arrival::Cut | Arrival::Closed | Arrival::Removed => None,
        },
        Step::DirectoryClose => match event {
            Arrival::Closed => {
                store.transcripts.drop.directory = None;
                Some(true)
            }
            Arrival::Opened(_) | Arrival::Missing | Arrival::Cut | Arrival::Synced | Arrival::Removed => None,
        },
        Step::Idle => None,
    };
    match result {
        Some(true) => {
            store.transcripts.stage = Stage::Idle;
            super::write::fire(store, env, above, below);
        }
        Some(false) => {}
        None => transcripts::failed(store, None, above),
    }
}
