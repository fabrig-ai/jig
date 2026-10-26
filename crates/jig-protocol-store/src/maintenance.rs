//! Stopped-engine tools, using the start's drivers (protocol/store.md, 10.5).
use crate::generations::{self, Active, Copying, Progress, Removing};
use crate::{
    Arrived, BackRefusal, Below, Event, Io, KeptWire, Limits, OpenRefusal, Phase, Root, Store, directory, open,
};
use alloc::boxed::Box;
use skein_io::{file, kernel::Error};
use skein_lib::{Env, Queue, Token};

/// The store owns this child of the shell's scratch/cache root (store.md 10.5).
const SCRATCH: &[u8] = b"rehearsal";
const SCRATCH_PREFIX: &[u8] = b"rehearsal/";

#[derive(Debug)]
pub(crate) enum Operation {
    Rehearsal(Rehearsal),
    Back(Back),
}
#[derive(Debug)]
pub(crate) struct Rehearsal {
    source: Root,
    scratch: Root,
    owner: Token,
    installation: [u8; 16],
    generation: Option<u64>,
    wires: Option<Box<[KeptWire]>>,
    reported: bool,
    terminal: bool,
    scratch_locked: bool,
    inflight: u32,
    close: Option<Token>,
    step: RehearsalStep,
}
#[derive(Debug)]
enum RehearsalStep {
    Lock,
    Active,
    ScratchLock,
    MakingContainer,
    RemovingContainer { ending: bool },
    EndSync,
    OpeningCleanup { ending: bool },
    Cleaning { root: Root, removing: Removing, ending: bool },
    ClosingCleanup { ending: bool, failed: bool },
    Listing,
    OpeningFile { name: Box<[u8]>, ending: bool },
    RemovingFile { root: Root, ending: bool },
    ClosingFile { ending: bool },
    Making,
    Copying(Copying),
    Naming,
    Serving,
    ClosingMaps { closed: u8 },
    ClosingTranscripts,
    ClosingDirectories { left: u8 },
    ListingEnd,
    Finished,
}
#[derive(Debug)]
pub(crate) struct Back {
    root: Root,
    owner: Token,
    active: Option<Active>,
    previous: u64,
    manifest: u64,
    map: Option<Root>,
    step: BackStep,
}
#[derive(Debug)]
enum BackStep {
    Lock,
    Active,
    Manifest,
    Previous,
    PreviousClose,
    Map,
    Recovering,
    Closing,
    DirectoryClose,
    Switching,
    Finished,
}

fn rehearsal_state(store: &Store) -> Option<&Rehearsal> {
    match store.operation.as_deref() {
        Some(Operation::Rehearsal(r)) => Some(r),
        Some(Operation::Back(_)) | None => None,
    }
}
fn rehearsal_mut(store: &mut Store) -> Option<&mut Rehearsal> {
    match store.operation.as_deref_mut() {
        Some(Operation::Rehearsal(r)) => Some(r),
        Some(Operation::Back(_)) | None => None,
    }
}
pub(crate) fn rehearsal(store: &Store) -> bool {
    rehearsal_state(store).is_some()
}
pub(crate) fn transcripts(store: &Store) -> Root {
    match store.operation.as_deref() {
        Some(Operation::Rehearsal(rehearsal)) => rehearsal.source,
        Some(Operation::Back(_)) | None => store.directory.root.expect("open holds its deployment root"),
    }
}
fn load(root: Root, path: Box<[u8]>, max: u32, below: &mut Below) {
    below.transcripts.push(file::Request::Load {
        owner: directory::OWNER,
        root: root.token,
        path,
        max,
        no_follow: true,
    });
}
fn lock(root: Root, below: &mut Below) {
    below.transcripts.push(file::Request::Lock { owner: directory::OWNER, root: root.token });
}
pub(crate) fn rehearse(
    store: &mut Store,
    owner: Token,
    source: Root,
    scratch: Root,
    installation: [u8; 16],
    above: &mut Queue<Event>,
    below: &mut Below,
) {
    if store.phase != Phase::Closed
        || !store.directory.disk
        || store.operation.is_some()
        || store.directory.root.is_some()
        || source.fd == scratch.fd
    {
        return open::refuse(store, owner, OpenRefusal::Failed, above);
    }
    store.directory.owner = owner;
    store.operation = Some(Box::new(Operation::Rehearsal(Rehearsal {
        source,
        scratch,
        owner,
        installation,
        generation: None,
        wires: None,
        reported: false,
        terminal: false,
        scratch_locked: false,
        inflight: 0,
        close: None,
        step: RehearsalStep::Lock,
    })));
    lock(source, below);
}
pub(crate) fn back(store: &mut Store, owner: Token, root: Root, above: &mut Queue<Event>, below: &mut Below) {
    if store.phase != Phase::Closed
        || !store.directory.disk
        || store.operation.is_some()
        || store.directory.root.is_some()
    {
        above.push(Event::BackRefused { owner, why: BackRefusal::Busy });
        return;
    }
    store.operation = Some(Box::new(Operation::Back(Back {
        root,
        owner,
        active: None,
        previous: 0,
        manifest: 0,
        map: None,
        step: BackStep::Lock,
    })));
    lock(root, below);
}
pub(crate) fn close(store: &mut Store, owner: Token, above: &mut Queue<Event>) {
    match store.operation.as_deref_mut() {
        Some(Operation::Rehearsal(rehearsal)) if rehearsal.close.is_none() => rehearsal.close = Some(owner),
        Some(Operation::Rehearsal(_) | Operation::Back(_)) | None => {
            open::refuse(store, owner, OpenRefusal::Failed, above);
        }
    }
}
/// The source's row is read once, before any patch; a subsequent open of the
/// patched copy must not replace it. Ordinary opens read and discard it.
pub(crate) fn wires(store: &mut Store, wires: Option<Box<[KeptWire]>>) {
    if let Some(rehearsal) = rehearsal_mut(store)
        && !rehearsal.reported
    {
        rehearsal.wires = wires;
    }
}
pub(crate) fn opening_ended(store: &mut Store) {
    if let Some(r) = rehearsal_mut(store) {
        r.terminal = true;
    }
}
pub(crate) fn report(store: &mut Store, above: &mut Queue<Event>) {
    let fresh = match &store.directory.patch {
        Some(pending) => pending.plan.close(),
        None => false,
    };
    if let Some(rehearsal) = rehearsal_mut(store)
        && !rehearsal.reported
    {
        rehearsal.reported = true;
        above.push(Event::Rehearsing { owner: rehearsal.owner, wires: rehearsal.wires.take(), fresh });
    }
}
pub(crate) fn queued(below: &Below) -> u32 {
    below
        .store
        .len()
        .checked_add(below.secrets.len())
        .expect("bounded store queues")
        .checked_add(below.transcripts.len())
        .expect("bounded driver queues")
}
pub(crate) fn emitted(store: &mut Store, before: u32, below: &mut Below) {
    if let Some(r) = rehearsal_state(store) {
        scope_queue(&mut below.store, r.scratch);
        scope_queue(&mut below.secrets, r.scratch);
        scope_queue(&mut below.transcripts, r.scratch);
    }
    if let Some(rehearsal) = rehearsal_mut(store) {
        rehearsal.inflight = rehearsal
            .inflight
            .checked_add(queued(below).checked_sub(before).expect("only emits"))
            .expect("bounded flights");
    }
}
pub(crate) fn arrived(store: &mut Store) {
    if let Some(rehearsal) = rehearsal_mut(store) {
        rehearsal.inflight = rehearsal.inflight.checked_sub(1).expect("one terminal per file request");
    }
}
pub(crate) fn routes(store: &Store) -> bool {
    match store.operation.as_deref() {
        Some(Operation::Back(_)) => true,
        Some(Operation::Rehearsal(r)) => match r.step {
            RehearsalStep::Serving => false,
            RehearsalStep::Lock
            | RehearsalStep::Active
            | RehearsalStep::ScratchLock
            | RehearsalStep::MakingContainer
            | RehearsalStep::RemovingContainer { .. }
            | RehearsalStep::EndSync
            | RehearsalStep::OpeningCleanup { .. }
            | RehearsalStep::Cleaning { .. }
            | RehearsalStep::ClosingCleanup { .. }
            | RehearsalStep::Listing
            | RehearsalStep::OpeningFile { .. }
            | RehearsalStep::RemovingFile { .. }
            | RehearsalStep::ClosingFile { .. }
            | RehearsalStep::Making
            | RehearsalStep::Copying(_)
            | RehearsalStep::Naming
            | RehearsalStep::ClosingMaps { .. }
            | RehearsalStep::ClosingTranscripts
            | RehearsalStep::ClosingDirectories { .. }
            | RehearsalStep::ListingEnd
            | RehearsalStep::Finished => true,
        },
        None => false,
    }
}
fn closing(r: &Rehearsal) -> bool {
    match r.step {
        RehearsalStep::OpeningCleanup { ending }
        | RehearsalStep::Cleaning { ending, .. }
        | RehearsalStep::ClosingCleanup { ending, .. }
        | RehearsalStep::OpeningFile { ending, .. }
        | RehearsalStep::RemovingFile { ending, .. }
        | RehearsalStep::ClosingFile { ending }
        | RehearsalStep::RemovingContainer { ending } => ending,
        RehearsalStep::ClosingMaps { .. }
        | RehearsalStep::ClosingTranscripts
        | RehearsalStep::ClosingDirectories { .. }
        | RehearsalStep::EndSync
        | RehearsalStep::ListingEnd
        | RehearsalStep::Finished => true,
        RehearsalStep::Lock
        | RehearsalStep::Active
        | RehearsalStep::ScratchLock
        | RehearsalStep::MakingContainer
        | RehearsalStep::Listing
        | RehearsalStep::Making
        | RehearsalStep::Copying(_)
        | RehearsalStep::Naming
        | RehearsalStep::Serving => false,
    }
}
fn closing_maps(r: &Rehearsal) -> Option<u8> {
    match r.step {
        RehearsalStep::ClosingMaps { closed } => Some(closed),
        RehearsalStep::Lock
        | RehearsalStep::Active
        | RehearsalStep::ScratchLock
        | RehearsalStep::MakingContainer
        | RehearsalStep::RemovingContainer { .. }
        | RehearsalStep::EndSync
        | RehearsalStep::OpeningCleanup { .. }
        | RehearsalStep::Cleaning { .. }
        | RehearsalStep::ClosingCleanup { .. }
        | RehearsalStep::Listing
        | RehearsalStep::OpeningFile { .. }
        | RehearsalStep::RemovingFile { .. }
        | RehearsalStep::ClosingFile { .. }
        | RehearsalStep::Making
        | RehearsalStep::Copying(_)
        | RehearsalStep::Naming
        | RehearsalStep::Serving
        | RehearsalStep::ClosingTranscripts
        | RehearsalStep::ClosingDirectories { .. }
        | RehearsalStep::ListingEnd
        | RehearsalStep::Finished => None,
    }
}
/// A close first drains the operation already in flight. At a written site it
/// needs no reply: with no file flight, the patch machines can be released.
pub(crate) fn resume(store: &mut Store, env: &Env<Limits>, above: &mut Queue<Event>, below: &mut Below) {
    let begin = match rehearsal_state(store) {
        Some(r) => r.close.is_some() && r.inflight == 0 && !closing(r),
        None => false,
    };
    if !begin {
        let closing = match rehearsal_state(store) {
            Some(r) => closing_maps(r).is_some() && r.inflight == 0,
            None => false,
        };
        if closing {
            for secret in [false, true] {
                let kv_env = directory::env(store, env, secret);
                let mut events = Queue::with_capacity(skein_kv::MAX_OUT.above);
                if secret {
                    if skein_kv::is_ready(&store.stores.secrets, &kv_env.limits) {
                        skein_kv::fire(&mut store.stores.secrets, &kv_env, &mut events, &mut below.secrets);
                    }
                } else if skein_kv::is_ready(&store.stores.store, &kv_env.limits) {
                    skein_kv::fire(&mut store.stores.store, &kv_env, &mut events, &mut below.store);
                }
                close_events(store, secret, &mut events, above, below);
            }
        }
        return;
    }
    if let Some(r) = rehearsal_state(store)
        && !r.terminal
    {
        open::refuse(store, r.owner, OpenRefusal::Failed, above);
    }
    if let Some(r) = rehearsal_state(store)
        && !r.scratch_locked
    {
        end(store, false, above);
        return;
    }
    store.directory.opening = directory::Opening::Idle;
    store.directory.patch = None;
    if let Some(r) = rehearsal_mut(store) {
        r.step = RehearsalStep::ClosingMaps { closed: 0 };
    }
    // KV close drains its own handles even after a refused open or failed commit.
    for secret in [false, true] {
        let kv_env = directory::env(store, env, secret);
        let mut events = Queue::with_capacity(skein_kv::MAX_OUT.above);
        if secret {
            skein_kv::down(
                &mut store.stores.secrets,
                &kv_env,
                skein_kv::Request::Close { owner: directory::OWNER },
                &mut events,
                &mut below.secrets,
            );
        } else {
            skein_kv::down(
                &mut store.stores.store,
                &kv_env,
                skein_kv::Request::Close { owner: directory::OWNER },
                &mut events,
                &mut below.store,
            );
        }
        close_events(store, secret, &mut events, above, below);
    }
}
fn close_events(
    store: &mut Store,
    secret: bool,
    events: &mut Queue<skein_kv::Event>,
    above: &mut Queue<Event>,
    below: &mut Below,
) {
    for _ in 0..skein_kv::MAX_OUT.above {
        let Some(event) = events.pop() else { break };
        let Some(r) = rehearsal_mut(store) else { return };
        if let Some(mut closed) = closing_maps(r) {
            match event {
                skein_kv::Event::Closed { .. } | skein_kv::Event::Refused { .. } => {
                    closed |= if secret { 2 } else { 1 };
                    r.step = RehearsalStep::ClosingMaps { closed };
                    if closed == 3 {
                        close_transcripts(store, above, below);
                    }
                }
                skein_kv::Event::Opened { .. }
                | skein_kv::Event::Committed { .. }
                | skein_kv::Event::Failed { .. }
                | skein_kv::Event::Got { .. }
                | skein_kv::Event::Loaded { .. }
                | skein_kv::Event::OpenFailed { .. }
                | skein_kv::Event::Recovered { .. }
                | skein_kv::Event::RecoveryFailed { .. } => end(store, false, above),
            }
        }
    }
}
fn close_transcripts(store: &mut Store, above: &mut Queue<Event>, below: &mut Below) {
    if let Some((task, _)) = store.transcripts.files.first() {
        let task = *task;
        let file = store.transcripts.files.remove(&task).expect("first cached transcript exists");
        below.transcripts.push(file::Request::Close { owner: directory::OWNER, file: file.token });
        if let Some(r) = rehearsal_mut(store) {
            r.step = RehearsalStep::ClosingTranscripts;
        }
    } else {
        for _ in 0..store.limits.reads {
            let Some((owner, _)) = store.transcripts.reads.first() else { break };
            let owner = *owner;
            let removed = store.transcripts.reads.remove(&owner);
            assert!(removed.is_some(), "first transcript read exists");
            above.push(Event::Unread { owner });
        }
        close_directories(store, above, below);
    }
}
fn close_directories(store: &mut Store, _above: &mut Queue<Event>, below: &mut Below) {
    let mut left = 0_u8;
    for (root, queue) in [
        (store.directory.store_root.take(), &mut below.store),
        (store.directory.private_root.take(), &mut below.secrets),
        (store.directory.transcripts.take(), &mut below.transcripts),
    ] {
        if let Some(root) = root {
            queue.push(file::Request::Close { owner: directory::OWNER, file: root.token });
            left = left.checked_add(1).expect("three directories");
        }
    }
    store.stores.store = skein_kv::Store::new(&store.limits.store);
    store.stores.secrets = skein_kv::Store::new(&store.limits.secrets);
    let Some(r) = rehearsal_mut(store) else { return };
    if left == 0 {
        begin_cleanup(r, true, below);
    } else {
        r.step = RehearsalStep::ClosingDirectories { left };
    }
}
fn end(store: &mut Store, removed: bool, above: &mut Queue<Event>) {
    if let Some(r) = rehearsal_state(store)
        && !r.terminal
    {
        open::refuse(store, r.owner, OpenRefusal::Failed, above);
    }
    if let Some(r) = rehearsal_mut(store) {
        let owner = r.close.expect("close owns cleanup");
        r.step = RehearsalStep::Finished;
        above.push(Event::Closed { owner, removed });
    }
    store.phase = Phase::Stopped;
}
pub(crate) fn up(store: &mut Store, env: &Env<Limits>, io: Io, above: &mut Queue<Event>, below: &mut Below) {
    let (arrived, secret, transcript) = match io {
        Io::Store(a) => (a, false, false),
        Io::Secrets(a) => (a, true, false),
        Io::Transcripts(a) => (a, false, true),
    };
    let closing_maps = match rehearsal_state(store) {
        Some(r) => closing_maps(r).is_some(),
        None => false,
    };
    if closing_maps && !transcript {
        let kv_env = directory::env(store, env, secret);
        let mut events = Queue::with_capacity(skein_kv::MAX_OUT.above);
        if secret {
            skein_kv::up(&mut store.stores.secrets, &kv_env, arrived.event, &mut events, &mut below.secrets);
        } else {
            skein_kv::up(&mut store.stores.store, &kv_env, arrived.event, &mut events, &mut below.store);
        }
        return close_events(store, secret, &mut events, above, below);
    }
    let cached = match rehearsal_state(store) {
        Some(r) => match r.step {
            RehearsalStep::ClosingTranscripts => true,
            RehearsalStep::Lock
            | RehearsalStep::Active
            | RehearsalStep::ScratchLock
            | RehearsalStep::MakingContainer
            | RehearsalStep::RemovingContainer { .. }
            | RehearsalStep::EndSync
            | RehearsalStep::OpeningCleanup { .. }
            | RehearsalStep::Cleaning { .. }
            | RehearsalStep::ClosingCleanup { .. }
            | RehearsalStep::Listing
            | RehearsalStep::OpeningFile { .. }
            | RehearsalStep::RemovingFile { .. }
            | RehearsalStep::ClosingFile { .. }
            | RehearsalStep::Making
            | RehearsalStep::Copying(_)
            | RehearsalStep::Naming
            | RehearsalStep::Serving
            | RehearsalStep::ClosingMaps { .. }
            | RehearsalStep::ClosingDirectories { .. }
            | RehearsalStep::ListingEnd
            | RehearsalStep::Finished => false,
        },
        None => false,
    };
    if cached {
        if let file::Event::Closed { .. } = arrived.event {
            close_transcripts(store, above, below);
        } else {
            end(store, false, above);
        }
        return;
    }
    let mut operation = store.operation.take().expect("tool routing");
    match operation.as_mut() {
        Operation::Rehearsal(r) => {
            let step = core::mem::replace(&mut r.step, RehearsalStep::Finished);
            let result = rehearsal_up(r, step, arrived, store.limits.store.chunk, below);
            store.operation = Some(operation);
            match result {
                RehearsalResult::Pending => {}
                RehearsalResult::Open => {
                    let Some(r) = rehearsal_state(store) else { return };
                    directory::open(store, env, r.owner, r.scratch, r.installation, above, below);
                }
                RehearsalResult::Refused(why) => {
                    if let Some(r) = rehearsal_mut(store) {
                        r.step = RehearsalStep::Serving;
                    }
                    report(store, above);
                    open::refuse(store, store.directory.owner, why, above);
                }
                RehearsalResult::Closed(removed) => end(store, removed, above),
            }
        }
        Operation::Back(back) => {
            back_up(store, env, back, arrived, above, below);
            store.operation = Some(operation);
        }
    }
}
#[derive(Debug)]
enum RehearsalResult {
    Pending,
    Open,
    Refused(OpenRefusal),
    Closed(bool),
}
fn failed(r: &Rehearsal) -> RehearsalResult {
    if r.close.is_some() { RehearsalResult::Closed(false) } else { RehearsalResult::Refused(OpenRefusal::Failed) }
}
fn list_scratch(r: &Rehearsal, below: &mut Below) {
    below.transcripts.push(file::Request::Scan {
        owner: directory::OWNER,
        root: r.scratch.token,
        path: Box::from(SCRATCH),
        max: generations::PAGE,
        max_bytes: u64::from(generations::PAGE)
            .checked_mul(
                u64::try_from(size_of::<file::Entry>())
                    .expect("entry size fits u64")
                    .checked_add(255)
                    .expect("entry and name fit"),
            )
            .expect("bounded page"),
        no_follow: true,
    });
}
#[expect(clippy::too_many_lines, reason = "one bounded file transition per rehearsal step (store.md 10.5)")]
fn rehearsal_up(
    r: &mut Rehearsal,
    step: RehearsalStep,
    arrived: Arrived,
    chunk: u32,
    below: &mut Below,
) -> RehearsalResult {
    match step {
        RehearsalStep::Lock => match arrived.event {
            file::Event::Locked { .. } => {
                lock(r.scratch, below);
                r.step = RehearsalStep::ScratchLock;
            }
            file::Event::Busy { .. } => {
                return RehearsalResult::Refused(OpenRefusal::Generations { why: crate::Generations::Busy });
            }
            file::Event::Opened { .. }
            | file::Event::Refused { .. }
            | file::Event::Loaded { .. }
            | file::Event::Scanned { .. }
            | file::Event::Stored { .. }
            | file::Event::Conflict { .. }
            | file::Event::TooLarge { .. }
            | file::Event::Stated { .. }
            | file::Event::Written { .. }
            | file::Event::Read { .. }
            | file::Event::Cut { .. }
            | file::Event::Synced { .. }
            | file::Event::Closed { .. }
            | file::Event::Renamed { .. }
            | file::Event::Removed { .. }
            | file::Event::Listed { .. }
            | file::Event::Cancelled { .. }
            | file::Event::Made { .. }
            | file::Event::Failed { .. } => return failed(r),
        },
        RehearsalStep::Active => {
            let file::Event::Loaded { bytes, .. } = arrived.event else {
                return RehearsalResult::Refused(OpenRefusal::Generations { why: crate::Generations::Unreadable });
            };
            let Some(number) = generations::parse(&bytes) else {
                return RehearsalResult::Refused(OpenRefusal::Generations { why: crate::Generations::Unreadable });
            };
            r.generation = Some(number);
            begin_cleanup(r, false, below);
        }
        RehearsalStep::ScratchLock => {
            let file::Event::Locked { .. } = arrived.event else { return failed(r) };
            r.scratch_locked = true;
            load(r.source, Box::from(generations::ACTIVE), generations::ACTIVE_BYTES, below);
            r.step = RehearsalStep::Active;
        }
        RehearsalStep::OpeningCleanup { ending } => {
            if let file::Event::Failed { error: Error::NotFound, .. } = arrived.event {
                if ending || r.close.is_some() {
                    return RehearsalResult::Closed(true);
                }
                make_container(r, below);
            } else {
                let Some(root) = directory::opened(arrived) else { return failed(r) };
                let removing = generations::remove(root, Box::from(generations::GENERATIONS), &mut below.transcripts);
                r.step = RehearsalStep::Cleaning { root, removing, ending };
            }
        }
        RehearsalStep::Cleaning { root, mut removing, ending } => {
            match generations::removed(&mut removing, arrived, &mut below.transcripts) {
                Progress::Pending => r.step = RehearsalStep::Cleaning { root, removing, ending },
                Progress::Done => {
                    below.transcripts.push(file::Request::Close { owner: directory::OWNER, file: root.token });
                    r.step = RehearsalStep::ClosingCleanup { ending: ending || r.close.is_some(), failed: false };
                }
                Progress::Failed => {
                    below.transcripts.push(file::Request::Close { owner: directory::OWNER, file: root.token });
                    r.step = RehearsalStep::ClosingCleanup { ending, failed: true };
                }
            }
        }
        RehearsalStep::ClosingCleanup { ending, failed: fault } => {
            let file::Event::Closed { .. } = arrived.event else { return failed(r) };
            if fault {
                return failed(r);
            }
            list_scratch(r, below);
            r.step = if ending { RehearsalStep::ListingEnd } else { RehearsalStep::Listing };
        }
        RehearsalStep::OpeningFile { name, ending } => {
            let Some(root) = directory::opened(arrived) else { return failed(r) };
            below.transcripts.push(file::Request::Remove {
                owner: directory::OWNER,
                root: root.fd,
                name,
                directory: false,
            });
            r.step = RehearsalStep::RemovingFile { root, ending };
        }
        RehearsalStep::RemovingFile { root, ending } => {
            let file::Event::Removed { .. } = arrived.event else {
                below.transcripts.push(file::Request::Close { owner: directory::OWNER, file: root.token });
                r.step = RehearsalStep::ClosingCleanup { ending, failed: true };
                return RehearsalResult::Pending;
            };
            below.transcripts.push(file::Request::Close { owner: directory::OWNER, file: root.token });
            r.step = RehearsalStep::ClosingFile { ending };
        }
        RehearsalStep::ClosingFile { ending } => {
            let file::Event::Closed { .. } = arrived.event else { return failed(r) };
            list_scratch(r, below);
            r.step = if ending { RehearsalStep::ListingEnd } else { RehearsalStep::Listing };
        }
        RehearsalStep::Listing => return listed(r, arrived, false, below),
        RehearsalStep::ListingEnd => return listed(r, arrived, true, below),
        RehearsalStep::MakingContainer => {
            let file::Event::Made { .. } = arrived.event else { return failed(r) };
            below.transcripts.push(file::Request::MakeDirectory {
                owner: directory::OWNER,
                root: r.scratch.token,
                path: Box::from(generations::GENERATIONS),
                mode: 0o700,
            });
            r.step = RehearsalStep::Making;
        }
        RehearsalStep::RemovingContainer { ending } => {
            let file::Event::Removed { .. } = arrived.event else { return failed(r) };
            if ending {
                below.transcripts.push(file::Request::SyncDirectory { owner: directory::OWNER, root: r.scratch.fd });
                r.step = RehearsalStep::EndSync;
            } else {
                make_container(r, below);
            }
        }
        RehearsalStep::EndSync => {
            let file::Event::Synced { .. } = arrived.event else { return failed(r) };
            return RehearsalResult::Closed(true);
        }
        RehearsalStep::Making => {
            let file::Event::Made { .. } = arrived.event else { return failed(r) };
            let number = r.generation.expect("source active read");
            r.step = RehearsalStep::Copying(generations::copy_between(
                r.source,
                r.scratch,
                number,
                Some(number),
                chunk,
                below,
            ));
        }
        RehearsalStep::Copying(mut copying) => match generations::copied(&mut copying, arrived, below) {
            Progress::Pending => r.step = RehearsalStep::Copying(copying),
            Progress::Failed => return failed(r),
            Progress::Done => {
                below.transcripts.push(file::Request::Store {
                    owner: directory::OWNER,
                    root: r.scratch.token,
                    path: Box::from(generations::ACTIVE),
                    bytes: generations::encode(r.generation.expect("active read")),
                    expected: file::Expect::Absent,
                    no_follow: true,
                });
                r.step = RehearsalStep::Naming;
            }
        },
        RehearsalStep::Naming => {
            let file::Event::Stored { .. } = arrived.event else { return failed(r) };
            r.step = RehearsalStep::Serving;
            return RehearsalResult::Open;
        }
        RehearsalStep::ClosingDirectories { left } => {
            let file::Event::Closed { .. } = arrived.event else { return failed(r) };
            let left = left.checked_sub(1).expect("one close per directory");
            if left == 0 {
                begin_cleanup(r, true, below);
            } else {
                r.step = RehearsalStep::ClosingDirectories { left };
            }
        }
        RehearsalStep::Serving
        | RehearsalStep::ClosingMaps { .. }
        | RehearsalStep::ClosingTranscripts
        | RehearsalStep::Finished => return failed(r),
    }
    RehearsalResult::Pending
}
fn listed(r: &mut Rehearsal, arrived: Arrived, ending: bool, below: &mut Below) -> RehearsalResult {
    if let file::Event::Failed { error: Error::NotFound, .. } = arrived.event {
        if ending {
            return RehearsalResult::Closed(true);
        }
        make_container(r, below);
        return RehearsalResult::Pending;
    }
    let file::Event::Scanned { entries, .. } = arrived.event else { return failed(r) };
    if let Some(entry) = entries.first() {
        // Only our child is scanned. Other entries under the supplied cache
        // root belong to the shell and are never removed.
        if entry.kind == skein_io::kernel::Kind::Directory {
            return failed(r);
        }
        below.transcripts.push(file::Request::OpenDirectory {
            owner: directory::OWNER,
            root: r.scratch.fd,
            name: Box::from(SCRATCH),
            no_follow: true,
        });
        r.step = RehearsalStep::OpeningFile { name: entry.name.clone(), ending };
    } else {
        below.transcripts.push(file::Request::Remove {
            owner: directory::OWNER,
            root: r.scratch.fd,
            name: Box::from(SCRATCH),
            directory: true,
        });
        r.step = RehearsalStep::RemovingContainer { ending };
    }
    RehearsalResult::Pending
}
fn begin_cleanup(r: &mut Rehearsal, ending: bool, below: &mut Below) {
    below.transcripts.push(file::Request::OpenDirectory {
        owner: directory::OWNER,
        root: r.scratch.fd,
        name: Box::from(SCRATCH),
        no_follow: true,
    });
    r.step = RehearsalStep::OpeningCleanup { ending };
}
fn make_container(r: &mut Rehearsal, below: &mut Below) {
    below.transcripts.push(file::Request::MakeDirectory {
        owner: directory::OWNER,
        root: r.scratch.token,
        path: Box::from(SCRATCH),
        mode: 0o700,
    });
    r.step = RehearsalStep::MakingContainer;
}
fn child_path(path: &[u8]) -> Box<[u8]> {
    generations::join(SCRATCH_PREFIX, path)
}
fn scope_path(path: &mut Box<[u8]>) {
    if path.as_ref() != SCRATCH && !path.starts_with(SCRATCH_PREFIX) {
        *path = child_path(path);
    }
}
fn scope_queue(queue: &mut Queue<file::Request>, root: Root) {
    let count = queue.len();
    for _ in 0..count {
        let Some(mut request) = queue.pop() else { break };
        scope_request(&mut request, root);
        queue.push(request);
    }
}
/// Only paths based at the shell's adopted scratch root need a prefix. Open
/// map/file descriptors already name the child; source roots are untouched.
fn scope_request(request: &mut file::Request, scratch: Root) {
    match request {
        file::Request::MakeDirectory { root, path, .. }
        | file::Request::OpenPrivate { root, path, .. }
        | file::Request::Load { root, path, .. }
        | file::Request::Scan { root, path, .. }
        | file::Request::Store { root, path, .. } => {
            if *root == scratch.token {
                scope_path(path);
            }
        }
        file::Request::Create { root, name, .. }
        | file::Request::CreateNoFollow { root, name, .. }
        | file::Request::OpenRead { root, name, .. }
        | file::Request::OpenReadNoFollow { root, name, .. }
        | file::Request::OpenWrite { root, name, .. }
        | file::Request::OpenWriteNoFollow { root, name, .. }
        | file::Request::OpenDirectory { root, name, .. }
        | file::Request::Remove { root, name, .. } => {
            if *root == scratch.fd {
                scope_path(name);
            }
        }
        file::Request::Rename { from_root, from, to_root, to, .. } => {
            if *from_root == scratch.fd {
                scope_path(from);
            }
            if *to_root == scratch.fd {
                scope_path(to);
            }
        }
        file::Request::Lock { .. }
        | file::Request::Stat { .. }
        | file::Request::WriteAt { .. }
        | file::Request::ReadAt { .. }
        | file::Request::Cut { .. }
        | file::Request::Sync { .. }
        | file::Request::Close { .. }
        | file::Request::SyncDirectory { .. }
        | file::Request::List { .. }
        | file::Request::ListPage { .. } => {}
    }
}
fn back_refuse(store: &mut Store, back: &mut Back, why: BackRefusal, above: &mut Queue<Event>) {
    back.step = BackStep::Finished;
    store.phase = Phase::Stopped;
    above.push(Event::BackRefused { owner: back.owner, why });
}
#[expect(clippy::too_many_lines, reason = "one file or KV transition per going-back step (store.md 10.5)")]
fn back_up(
    store: &mut Store,
    env: &Env<Limits>,
    back: &mut Back,
    arrived: Arrived,
    above: &mut Queue<Event>,
    below: &mut Below,
) {
    match core::mem::replace(&mut back.step, BackStep::Finished) {
        BackStep::Lock => match arrived.event {
            file::Event::Locked { .. } => {
                load(back.root, Box::from(generations::ACTIVE), generations::ACTIVE_BYTES, below);
                back.step = BackStep::Active;
            }
            file::Event::Busy { .. } => back_refuse(store, back, BackRefusal::Busy, above),
            file::Event::Opened { .. }
            | file::Event::Refused { .. }
            | file::Event::Loaded { .. }
            | file::Event::Scanned { .. }
            | file::Event::Stored { .. }
            | file::Event::Conflict { .. }
            | file::Event::TooLarge { .. }
            | file::Event::Stated { .. }
            | file::Event::Written { .. }
            | file::Event::Read { .. }
            | file::Event::Cut { .. }
            | file::Event::Synced { .. }
            | file::Event::Closed { .. }
            | file::Event::Renamed { .. }
            | file::Event::Removed { .. }
            | file::Event::Listed { .. }
            | file::Event::Cancelled { .. }
            | file::Event::Made { .. }
            | file::Event::Failed { .. } => back_refuse(store, back, BackRefusal::Unreadable, above),
        },
        BackStep::Active => {
            let file::Event::Loaded { bytes, .. } = arrived.event else {
                return back_refuse(store, back, BackRefusal::Unreadable, above);
            };
            let Some(number) = generations::parse(&bytes) else {
                return back_refuse(store, back, BackRefusal::Unreadable, above);
            };
            back.active = Some(Active { number, digest: skein_io::digest::digest(&bytes) });
            load(
                back.root,
                generations::join(&generations::generation(number), b"/manifest"),
                crate::sizes::MANIFEST_BYTES,
                below,
            );
            back.step = BackStep::Manifest;
        }
        BackStep::Manifest => {
            if let file::Event::Failed { error: Error::NotFound, .. } = arrived.event {
                return back_refuse(store, back, BackRefusal::NoPrevious, above);
            }
            let file::Event::Loaded { bytes, .. } = arrived.event else {
                return back_refuse(store, back, BackRefusal::Unreadable, above);
            };
            let Ok(manifest) = crate::codec::component::Manifest::decode(&bytes) else {
                return back_refuse(store, back, BackRefusal::Unreadable, above);
            };
            let active = back.active.expect("active read");
            if manifest.generation != active.number {
                return back_refuse(store, back, BackRefusal::Unreadable, above);
            }
            let Some(previous) = manifest.from else { return back_refuse(store, back, BackRefusal::NoPrevious, above) };
            if previous.checked_add(1) != Some(active.number) {
                return back_refuse(store, back, BackRefusal::Unreadable, above);
            }
            back.previous = previous;
            back.manifest = manifest.store;
            below.transcripts.push(file::Request::OpenDirectory {
                owner: directory::OWNER,
                root: back.root.fd,
                name: generations::generation(previous),
                no_follow: true,
            });
            back.step = BackStep::Previous;
        }
        BackStep::Previous => {
            let Some(root) = directory::opened(arrived) else {
                return back_refuse(store, back, BackRefusal::NoPrevious, above);
            };
            below.transcripts.push(file::Request::Close { owner: directory::OWNER, file: root.token });
            back.step = BackStep::PreviousClose;
        }
        BackStep::PreviousClose => {
            let file::Event::Closed { .. } = arrived.event else {
                return back_refuse(store, back, BackRefusal::Unreadable, above);
            };
            below.store.push(file::Request::OpenDirectory {
                owner: directory::OWNER,
                root: back.root.fd,
                name: generations::map(back.active.expect("active read").number, generations::Map::Store),
                no_follow: true,
            });
            back.step = BackStep::Map;
        }
        BackStep::Map => {
            let Some(root) = directory::opened(arrived) else {
                return back_refuse(store, back, BackRefusal::Unreadable, above);
            };
            back.map = Some(root);
            back.step = BackStep::Recovering;
            back_kv(
                store,
                env,
                back,
                Some(skein_kv::Request::Open { owner: directory::OWNER, root: root.fd }),
                None,
                above,
                below,
            );
        }
        step @ (BackStep::Recovering | BackStep::Closing) => {
            back.step = step;
            back_kv(store, env, back, None, Some(arrived.event), above, below);
        }
        BackStep::DirectoryClose => {
            let file::Event::Closed { .. } = arrived.event else {
                return back_refuse(store, back, BackRefusal::Unreadable, above);
            };
            let active = back.active.expect("active read");
            below.transcripts.push(file::Request::Store {
                owner: directory::OWNER,
                root: back.root.token,
                path: Box::from(generations::ACTIVE),
                bytes: generations::encode(back.previous),
                expected: file::Expect::Digest(active.digest),
                no_follow: true,
            });
            back.step = BackStep::Switching;
        }
        BackStep::Switching => {
            let file::Event::Stored { .. } = arrived.event else {
                return back_refuse(store, back, BackRefusal::Unreadable, above);
            };
            back.step = BackStep::Finished;
            store.phase = Phase::Stopped;
            above.push(Event::Back { owner: back.owner, to: back.previous });
        }
        BackStep::Finished => {}
    }
}
fn back_kv(
    store: &mut Store,
    env: &Env<Limits>,
    back: &mut Back,
    request: Option<skein_kv::Request>,
    event: Option<file::Event>,
    above: &mut Queue<Event>,
    below: &mut Below,
) {
    let kv_env = directory::env(store, env, false);
    let mut events = Queue::with_capacity(skein_kv::MAX_OUT.above);
    if let Some(request) = request {
        skein_kv::down(&mut store.stores.store, &kv_env, request, &mut events, &mut below.store);
    }
    if let Some(event) = event {
        skein_kv::up(&mut store.stores.store, &kv_env, event, &mut events, &mut below.store);
    }
    for _ in 0..skein_kv::MAX_OUT.above {
        let Some(event) = events.pop() else { break };
        match event {
            skein_kv::Event::Opened { .. } => {
                let last = store.stores.store.last();
                if last > back.manifest {
                    return back_refuse(store, back, BackRefusal::Committed { manifest: back.manifest, last }, above);
                }
                if last != back.manifest {
                    return back_refuse(store, back, BackRefusal::Unreadable, above);
                }
                back.step = BackStep::Closing;
                skein_kv::down(
                    &mut store.stores.store,
                    &kv_env,
                    skein_kv::Request::Close { owner: directory::OWNER },
                    &mut events,
                    &mut below.store,
                );
            }
            skein_kv::Event::Closed { .. } => {
                below.store.push(file::Request::Close {
                    owner: directory::OWNER,
                    file: back.map.take().expect("recovered map").token,
                });
                back.step = BackStep::DirectoryClose;
            }
            skein_kv::Event::Committed { .. }
            | skein_kv::Event::Refused { .. }
            | skein_kv::Event::Failed { .. }
            | skein_kv::Event::Got { .. }
            | skein_kv::Event::Loaded { .. }
            | skein_kv::Event::OpenFailed { .. }
            | skein_kv::Event::Recovered { .. }
            | skein_kv::Event::RecoveryFailed { .. } => {
                return back_refuse(store, back, BackRefusal::Unreadable, above);
            }
        }
    }
}

/// Extra state named by store.md 10.5. The operation owns its inline copy or
/// removal state. Generation removal's stack is already in the shared price.
/// The initial copy's page and chunk, the wires' decoded/translated row, and
/// going back's manifest load are disjoint, so only their maximum is charged.
pub(crate) fn worst_case(limits: &Limits) -> Option<u64> {
    let inline = u64::try_from(size_of::<Operation>()).ok()?;
    let copy = generations::operations_worst_case(limits.store.chunk)?
        .checked_sub(u64::try_from(size_of::<Copying>()).ok()?)?;
    let wires = crate::component_v3::Wires::worst_case_heap(&crate::component_v3::CEILINGS)?
        .checked_add(skein_lib::List::<KeptWire>::worst_case(16)?)?
        .checked_add(512)?;
    let manifest = crate::component_v3::Manifest::worst_case_heap(&crate::component_v3::CEILINGS)?
        .checked_add(u64::from(crate::sizes::MANIFEST_BYTES))?;
    inline.checked_add(copy.max(wires).max(manifest))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stopped_engine_tools_have_their_exact_resident_price() {
        let limits = crate::tests::fixtures::store_limits();
        // The optional operation pointer adds 8 inline bytes (store.md 10.5).
        assert_eq!(size_of::<Store>(), 4_016);
        // Rehearsal: roots 32, owner 8, installation 16, generation 16, wires 16,
        // reported/terminal/scratch-lock flags 3, flight count 4, close owner 16, step 184,
        // and 1 alignment byte. Back's 120-byte state fits this union.
        assert_eq!(size_of::<Rehearsal>(), 296);
        assert_eq!(size_of::<RehearsalStep>(), 184);
        assert_eq!(size_of::<Back>(), 120);
        assert_eq!(size_of::<Operation>(), 296);
        // The separate source root adds 16 to Copying and its containing phase.
        assert_eq!(size_of::<Copying>(), 184);
        assert_eq!(size_of::<crate::patching::phase::Patching>(), 184);
        // Tool buffers: max(copy chunk 16,778,260 + 32 entries/names 8,928
        // + paths 1,024, decoded/translated wires 1,792, manifest 11,608).
        // The 296-byte operation owns the copy/removal phase inline.
        assert_eq!(worst_case(&limits), Some(16_788_508));
        // Baseline 581,544,799,705,037 + operation pointer 8 + tool price above.
        assert_eq!(crate::worst_case(&limits), Some(581_544_816_493_553));
    }
}
