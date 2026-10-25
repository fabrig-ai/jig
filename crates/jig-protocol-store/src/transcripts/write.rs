//! Ordered frames synced before their accepting commit (store.md, 6 and 8.2).
use super::{File, Stage};
use crate::{Arrived, Below, Event, Limits, Phase, Root, Store, Write, commits, transcripts};
use alloc::boxed::Box;
use skein_io::{file, kernel::Error};
use skein_lib::{Env, Queue, Token};
const OWNER: Token = Token::new(0x5452);
pub(crate) fn begin(
    store: &mut Store,
    env: &Env<Limits>,
    number: u64,
    writes: Box<[Write]>,
    above: &mut Queue<Event>,
    below: &mut Below,
) {
    let due = match store.phase {
        Phase::Ready { next, format_due } => {
            if number != next || number <= store.stores.store.last() {
                transcripts::failed(store, Some(number), above);
                return;
            }
            format_due
        }
        Phase::Closed => {
            transcripts::failed(store, Some(number), above);
            return;
        }
        Phase::Stopped => return,
    };
    if store.transcripts.current.is_some() && store.transcripts.queue.room() == 0 {
        transcripts::failed(store, Some(number), above);
        return;
    }
    let pending = match commits::operations(store, number, writes, due) {
        Ok(pending) => pending,
        Err(commits::Refusal::Failed) => {
            transcripts::failed(store, Some(number), above);
            return;
        }
        Err(commits::Refusal::FrameTooLarge { task, bytes, limit }) => {
            // Match ordinary failure ordering: this terminal first, then all
            // earlier pending commits. Never also fail this number generically.
            above.push(Event::FrameTooLarge { number, task, bytes, limit });
            transcripts::failed(store, None, above);
            return;
        }
    };
    store.phase = Phase::Ready { next: number.checked_add(1).unwrap_or(number), format_due: false };
    if store.transcripts.current.is_none() {
        store.transcripts.current = Some(pending);
    } else {
        store.transcripts.queue.push(pending);
    }
    fire(store, env, above, below);
}
pub(crate) fn fire(store: &mut Store, env: &Env<Limits>, above: &mut Queue<Event>, below: &mut Below) {
    if store.transcripts.reading.is_some() {
        return;
    }
    match store.transcripts.stage {
        Stage::Idle => {}
        Stage::Opening
        | Stage::Evicting { .. }
        | Stage::Making
        | Stage::RootSync
        | Stage::DirectoryOpen
        | Stage::Creating
        | Stage::Writing
        | Stage::Syncing
        | Stage::DirectorySync
        | Stage::DirectoryClose
        | Stage::Dropping => return,
    }
    for _ in 0..store.limits.queue.saturating_add(1) {
        if store.transcripts.current.is_none() {
            store.transcripts.current = store.transcripts.queue.pop();
        }
        let Some(pending) = store.transcripts.current.as_mut() else {
            return;
        };
        if let Some(frame) = pending.frames.pop() {
            store.transcripts.frame = Some(frame);
            start(store, above, below);
            return;
        }
        if let Some(task) = pending.drops.pop() {
            let number = pending.number;
            super::drop::begin(store, number, &[task], above, below);
            return;
        }
        let pending = store.transcripts.current.take().expect("current commit");
        commits::submit(store, env, pending.number, pending.ops, above, below);
        if store.phase == Phase::Stopped {
            return;
        }
    }
}
fn root(store: &Store) -> Root {
    store.directory.transcripts.expect("Open keeps the transcripts root")
}
fn task(store: &Store) -> u64 {
    store.transcripts.frame.as_ref().expect("active frame").task
}
fn start(store: &mut Store, above: &mut Queue<Event>, below: &mut Below) {
    let task = task(store);
    if let Some(file) = store.transcripts.files.get(&task) {
        if file.writable {
            write(store, above, below);
        } else {
            store.transcripts.stage = Stage::Evicting { task };
            below.transcripts.push(file::Request::Close { owner: OWNER, file: file.token });
        }
        return;
    }
    if store.transcripts.files.len() == store.limits.files {
        let mut oldest = None;
        for (task, file) in &store.transcripts.files {
            let replace = match oldest {
                Some((_, touched)) => file.touched < touched,
                None => true,
            };
            if replace {
                oldest = Some((*task, file.touched));
            }
        }
        let (task, _) = oldest.expect("a full nonempty file set");
        let file = store.transcripts.files.get(&task).expect("oldest file").token;
        store.transcripts.stage = Stage::Evicting { task };
        below.transcripts.push(file::Request::Close { owner: OWNER, file });
    } else {
        open(store, below);
    }
}
fn open(store: &mut Store, below: &mut Below) {
    store.transcripts.stage = Stage::Opening;
    below.transcripts.push(file::Request::OpenWriteNoFollow {
        owner: OWNER,
        root: root(store).fd,
        name: super::path(task(store)),
    });
}
fn write(store: &mut Store, above: &mut Queue<Event>, below: &mut Below) {
    let Some(touch) = store.transcripts.touch.checked_add(1) else {
        transcripts::failed(store, None, above);
        return;
    };
    store.transcripts.touch = touch;
    let frame = store.transcripts.frame.as_mut().expect("active frame");
    let file = store.transcripts.files.get_mut(&frame.task).expect("file opened");
    file.touched = touch;
    let bytes = core::mem::replace(&mut frame.bytes, Box::new([]));
    store.transcripts.stage = Stage::Writing;
    below.transcripts.push(file::Request::WriteAt { owner: OWNER, file: file.token, offset: frame.offset, bytes });
}
fn file_opened(store: &mut Store, file: Token, readable: bool, above: &mut Queue<Event>, below: &mut Below) {
    let task = task(store);
    if store
        .transcripts
        .files
        .insert(task, File { token: file, touched: store.transcripts.touch, writable: true, readable })
        .is_err()
    {
        transcripts::failed(store, None, above);
        return;
    }
    write(store, above, below);
}
fn directory_open(store: &mut Store, below: &mut Below) {
    store.transcripts.stage = Stage::DirectoryOpen;
    below.transcripts.push(file::Request::OpenDirectory {
        owner: OWNER,
        root: root(store).fd,
        name: super::name(task(store) >> 12),
        no_follow: true,
    });
}
fn finish(store: &mut Store, env: &Env<Limits>, above: &mut Queue<Event>, below: &mut Below) {
    store.transcripts.frame = None;
    store.transcripts.stage = Stage::Idle;
    fire(store, env, above, below);
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Opened,
    Missing,
    Exists,
    Made,
    Written,
    Synced,
    Closed,
}
struct Arrival {
    kind: Kind,
    file: Option<Token>,
    fd: Option<skein_io::kernel::Fd>,
}
fn arrival(arrived: Arrived) -> Option<Arrival> {
    let mut file = None;
    let kind = match arrived.event {
        file::Event::Opened { file: token, .. } => {
            file = Some(token);
            Kind::Opened
        }
        file::Event::Made { .. } => Kind::Made,
        file::Event::Written { .. } => Kind::Written,
        file::Event::Synced { .. } => Kind::Synced,
        file::Event::Closed { .. } => Kind::Closed,
        file::Event::Failed { error, .. } => {
            if error == Error::NotFound {
                Kind::Missing
            } else if error == Error::Exists {
                Kind::Exists
            } else {
                return None;
            }
        }
        file::Event::Refused { .. }
        | file::Event::Locked { .. }
        | file::Event::Busy { .. }
        | file::Event::Loaded { .. }
        | file::Event::Scanned { .. }
        | file::Event::Stored { .. }
        | file::Event::Conflict { .. }
        | file::Event::TooLarge { .. }
        | file::Event::Stated { .. }
        | file::Event::Read { .. }
        | file::Event::Cut { .. }
        | file::Event::Renamed { .. }
        | file::Event::Removed { .. }
        | file::Event::Listed { .. }
        | file::Event::Cancelled { .. } => return None,
    };
    Some(Arrival { kind, file, fd: arrived.fd })
}
pub(crate) fn arrived(
    store: &mut Store,
    env: &Env<Limits>,
    arrived: Arrived,
    above: &mut Queue<Event>,
    below: &mut Below,
) {
    let Some(arrived) = arrival(arrived) else {
        transcripts::failed(store, None, above);
        return;
    };
    let stage = core::mem::replace(&mut store.transcripts.stage, Stage::Idle);
    match stage {
        Stage::Opening => opening(store, arrived, above, below),
        Stage::Evicting { task } => {
            if arrived.kind != Kind::Closed {
                transcripts::failed(store, None, above);
                return;
            }
            let _: Option<File> = store.transcripts.files.remove(&task);
            open(store, below);
        }
        Stage::Making => {
            if arrived.kind == Kind::Made {
                store.transcripts.stage = Stage::RootSync;
                below.transcripts.push(file::Request::SyncDirectory { owner: OWNER, root: root(store).fd });
            } else if arrived.kind == Kind::Exists {
                directory_open(store, below);
            } else {
                transcripts::failed(store, None, above);
            }
        }
        Stage::RootSync => {
            if arrived.kind != Kind::Synced {
                transcripts::failed(store, None, above);
                return;
            }
            directory_open(store, below);
        }
        Stage::DirectoryOpen => created_directory(store, arrived, above, below),
        Stage::Creating => {
            if arrived.kind != Kind::Opened {
                transcripts::failed(store, None, above);
                return;
            }
            file_opened(store, arrived.file.expect("file opened token"), false, above, below);
        }
        Stage::Writing => {
            if arrived.kind != Kind::Written {
                transcripts::failed(store, None, above);
                return;
            }
            store.transcripts.stage = Stage::Syncing;
            let file = store.transcripts.files.get(&task(store)).expect("written file").token;
            below.transcripts.push(file::Request::Sync { owner: OWNER, file });
        }
        Stage::Syncing => {
            if arrived.kind != Kind::Synced {
                transcripts::failed(store, None, above);
                return;
            }
            match store.transcripts.directory {
                Some(directory) => {
                    store.transcripts.stage = Stage::DirectorySync;
                    below.transcripts.push(file::Request::SyncDirectory { owner: OWNER, root: directory.fd });
                }
                None => finish(store, env, above, below),
            }
        }
        Stage::DirectorySync => {
            if arrived.kind != Kind::Synced {
                transcripts::failed(store, None, above);
                return;
            }
            let directory = store.transcripts.directory.expect("created file directory");
            store.transcripts.stage = Stage::DirectoryClose;
            below.transcripts.push(file::Request::Close { owner: OWNER, file: directory.token });
        }
        Stage::DirectoryClose => {
            if arrived.kind != Kind::Closed {
                transcripts::failed(store, None, above);
                return;
            }
            store.transcripts.directory = None;
            finish(store, env, above, below);
        }
        Stage::Idle | Stage::Dropping => transcripts::failed(store, None, above),
    }
}

fn opening(store: &mut Store, arrived: Arrival, above: &mut Queue<Event>, below: &mut Below) {
    if arrived.kind == Kind::Opened {
        file_opened(store, arrived.file.expect("opened token"), true, above, below);
    } else if arrived.kind == Kind::Missing {
        store.transcripts.stage = Stage::Making;
        below.transcripts.push(file::Request::MakeDirectory {
            owner: OWNER,
            root: root(store).token,
            path: super::name(task(store) >> 12),
            mode: 0o700,
        });
    } else {
        transcripts::failed(store, None, above);
    }
}

fn created_directory(store: &mut Store, arrived: Arrival, above: &mut Queue<Event>, below: &mut Below) {
    if arrived.kind != Kind::Opened {
        transcripts::failed(store, None, above);
        return;
    }
    let Some(fd) = arrived.fd else {
        transcripts::failed(store, None, above);
        return;
    };
    let file = arrived.file.expect("directory opened token");
    store.transcripts.directory = Some(Root { token: file, fd });
    store.transcripts.stage = Stage::Creating;
    below.transcripts.push(file::Request::CreateNoFollow {
        owner: OWNER,
        root: fd,
        name: super::name(task(store)),
        mode: 0o600,
    });
}
