//! The deployment's generations on disk (protocol/store.md, 2, 10.1, 10.2 and 10.4).
use crate::{Arrived, Root, directory::OWNER};
use alloc::boxed::Box;
use skein_io::{digest::Digest, file, kernel::Kind};
use skein_lib::{Decimal, List, Queue, Writer};

pub(crate) const ACTIVE: &[u8] = b"active";
pub(crate) const GENERATIONS: &[u8] = b"generations";
pub(crate) const TRANSCRIPTS: &[u8] = b"transcripts";
/// A first start's generation (store.md 2).
pub(crate) const FIRST: u64 = 1;
/// Twenty digits and a newline (store.md 10.1).
pub(crate) const ACTIVE_BYTES: u32 = 21;
/// Both the world and service drivers accept 32 entries per page.
pub(crate) const PAGE: u32 = 32;
pub(crate) const DEPTH: u32 = 3;

/// The digest is kept only in memory for the switch's `Expect::Digest` (store.md 10.2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Active {
    pub number: u64,
    pub digest: Digest,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Map {
    Store,
    Secrets,
}

pub(crate) fn parse(bytes: &[u8]) -> Option<u64> {
    let (last, digits) = bytes.split_last()?;
    if *last != b'\n' {
        return None;
    }
    number(digits)
}
pub(crate) fn number(name: &[u8]) -> Option<u64> {
    if name.is_empty() || name.len() > 20 || name.first() == Some(&b'0') {
        return None;
    }
    let mut number = 0_u64;
    for byte in name {
        if !byte.is_ascii_digit() {
            return None;
        }
        number = number.checked_mul(10)?.checked_add(u64::from(byte.checked_sub(b'0')?))?;
    }
    Some(number)
}
/// Immutable account of the generation, at the wall clock of its write (store.md 2).
#[expect(clippy::too_many_arguments, reason = "the manifest's eight fields follow component-v3.schema")]
pub(crate) fn manifest(
    generation: u64,
    from: Option<u64>,
    before: crate::patching::plan::Levels,
    after: crate::patching::plan::Levels,
    counts: &[crate::codec::component::Count],
    store: u64,
    secrets: Option<u64>,
    written: skein_lib::Wall,
) -> Box<[u8]> {
    let mut kept = List::with_capacity(u32::try_from(counts.len()).expect("bounded manifest counts"));
    for count in counts {
        kept.push(count.clone()).expect("measured manifest counts");
    }
    crate::codec::component::Manifest {
        generation,
        from,
        before: crate::format::recorded(before),
        after: crate::format::recorded(after),
        counts: kept,
        store,
        secrets,
        written: written.as_nanos(),
    }
    .encode()
}
pub(crate) fn encode(number: u64) -> Box<[u8]> {
    join(Decimal::of(number).as_bytes(), b"\n")
}
fn join(prefix: &[u8], suffix: &[u8]) -> Box<[u8]> {
    let mut writer = Writer::new(prefix.len().checked_add(suffix.len()).expect("bounded generation path"));
    writer.put(prefix).expect("measured prefix");
    writer.put(suffix).expect("measured suffix");
    writer.finish()
}
pub(crate) fn generation(number: u64) -> Box<[u8]> {
    join(b"generations/", Decimal::of(number).as_bytes())
}
pub(crate) fn map(number: u64, map: Map) -> Box<[u8]> {
    let suffix = match map {
        Map::Store => b"/store".as_slice(),
        Map::Secrets => b"/secrets".as_slice(),
    };
    join(&generation(number), suffix)
}
pub(crate) fn is_temporary(name: &[u8]) -> bool {
    let Some(hex) = name.strip_prefix(b"active.skein-") else {
        return false;
    };
    if hex.len() != 16 {
        return false;
    }
    for byte in hex {
        if !byte.is_ascii_digit() && !(b'a'..=b'f').contains(byte) {
            return false;
        }
    }
    true
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Progress {
    Pending,
    Done,
    Failed,
}
#[derive(Debug)]
struct Level {
    parent: Root,
    name: Box<[u8]>,
    root: Option<Root>,
    entries: Box<[file::Entry]>,
    next: usize,
}
#[derive(Debug)]
enum Step {
    Opening,
    Listing,
    File,
    Closing { empty: bool },
    Removing,
    Failing,
    Finished,
}
/// A bounded stack; a page belongs to each level until its children are removed.
/// Every nonempty page is followed by a fresh open, so moving directory cursors
/// cannot hide entries (store.md 10.1). Failure drains every opened directory too.
#[derive(Debug)]
pub(crate) struct Removing {
    levels: List<Level>,
    step: Step,
}
pub(crate) fn remove(parent: Root, name: Box<[u8]>, queue: &mut Queue<file::Request>) -> Removing {
    let mut levels = List::with_capacity(DEPTH);
    queue.push(file::Request::OpenDirectory { owner: OWNER, root: parent.fd, name: name.clone(), no_follow: true });
    levels.push(Level { parent, name, root: None, entries: Box::new([]), next: 0 }).expect("first level fits");
    Removing { levels, step: Step::Opening }
}
fn top(removing: &mut Removing) -> &mut Level {
    let index = removing.levels.len().checked_sub(1).expect("a removal has a level");
    removing.levels.get_mut(index).expect("last level exists")
}
fn next(removing: &mut Removing, queue: &mut Queue<file::Request>) -> Progress {
    let level = top(removing);
    let root = level.root.expect("listed directory is open");
    let Some(entry) = level.entries.get(level.next) else {
        let empty = level.entries.is_empty();
        queue.push(file::Request::Close { owner: OWNER, file: root.token });
        removing.step = Step::Closing { empty };
        return Progress::Pending;
    };
    let name = entry.name.clone();
    let directory = entry.kind == Kind::Directory;
    level.next = level.next.checked_add(1).expect("page index bounded by PAGE");
    if directory {
        if removing.levels.room() == 0 {
            return fail(removing, queue);
        }
        queue.push(file::Request::OpenDirectory { owner: OWNER, root: root.fd, name: name.clone(), no_follow: true });
        removing
            .levels
            .push(Level { parent: root, name, root: None, entries: Box::new([]), next: 0 })
            .expect("room checked");
        removing.step = Step::Opening;
    } else {
        queue.push(file::Request::Remove { owner: OWNER, root: root.fd, name, directory: false });
        removing.step = Step::File;
    }
    Progress::Pending
}
fn fail(removing: &mut Removing, queue: &mut Queue<file::Request>) -> Progress {
    removing.step = Step::Failing;
    // At most DEPTH levels, with no work between the closes.
    for _ in 0..DEPTH {
        if removing.levels.is_empty() {
            break;
        }
        let level = top(removing);
        let root = level.root.take();
        let length = removing.levels.len().checked_sub(1).expect("nonempty stack");
        removing.levels.truncate(length);
        if let Some(root) = root {
            queue.push(file::Request::Close { owner: OWNER, file: root.token });
            return Progress::Pending;
        }
    }
    removing.step = Step::Finished;
    Progress::Failed
}
pub(crate) fn removed(removing: &mut Removing, arrived: Arrived, queue: &mut Queue<file::Request>) -> Progress {
    match core::mem::replace(&mut removing.step, Step::Finished) {
        Step::Opening => {
            let file::Event::Opened { file, .. } = arrived.event else {
                // A previous cut may already have removed the target completely.
                if let file::Event::Failed { error: skein_io::kernel::Error::NotFound, .. } = arrived.event {
                    let length = removing.levels.len().checked_sub(1).expect("opening level");
                    removing.levels.truncate(length);
                    if length == 0 {
                        return Progress::Done;
                    }
                    return next(removing, queue);
                }
                return fail(removing, queue);
            };
            let Some(fd) = arrived.fd else {
                return fail(removing, queue);
            };
            top(removing).root = Some(Root { token: file, fd });
            queue.push(file::Request::ListPage { owner: OWNER, directory: file, max: PAGE });
            removing.step = Step::Listing;
            Progress::Pending
        }
        Step::Listing => {
            let file::Event::Listed { entries, .. } = arrived.event else {
                return fail(removing, queue);
            };
            let level = top(removing);
            level.entries = entries;
            level.next = 0;
            next(removing, queue)
        }
        Step::File => {
            let file::Event::Removed { .. } = arrived.event else {
                return fail(removing, queue);
            };
            next(removing, queue)
        }
        Step::Closing { empty } => {
            top(removing).root = None;
            let file::Event::Closed { .. } = arrived.event else {
                return fail(removing, queue);
            };
            let level = top(removing);
            if empty {
                queue.push(file::Request::Remove {
                    owner: OWNER,
                    root: level.parent.fd,
                    name: level.name.clone(),
                    directory: true,
                });
                removing.step = Step::Removing;
            } else {
                level.entries = Box::new([]);
                queue.push(file::Request::OpenDirectory {
                    owner: OWNER,
                    root: level.parent.fd,
                    name: level.name.clone(),
                    no_follow: true,
                });
                removing.step = Step::Opening;
            }
            Progress::Pending
        }
        Step::Removing => {
            let file::Event::Removed { .. } = arrived.event else {
                return fail(removing, queue);
            };
            let length = removing.levels.len().checked_sub(1).expect("removed level");
            removing.levels.truncate(length);
            if length == 0 { Progress::Done } else { next(removing, queue) }
        }
        Step::Failing => fail(removing, queue),
        Step::Finished => Progress::Failed,
    }
}

/// The copy keeps its roots and files on the transcripts driver, away from kv's
/// handles. Even beside the transcript root it needs at most six open slots.
#[derive(Debug)]
enum CopyStep {
    Generation,
    StoreDirectory,
    Destination,
    Source,
    Page,
    ReadOpen,
    WriteOpen,
    Read,
    Write,
    FileSync,
    WriteClose,
    ReadClose,
    MapSync,
    SourceClose,
    DestinationClose,
    Generations,
    GenerationsSync,
    GenerationsClose,
    Failing,
    Finished,
}
/// Generation `from`'s maps copied into `from + 1` (store.md 10.2, step 2).
/// Directories are made 0o700, secrets opened private, files created exclusively
/// 0o600 and copied through one bounded chunk. Files, map directories and finally
/// generations are synced. The source is only read; a standing destination fails.
#[derive(Debug)]
pub(crate) struct Copying {
    root: Root,
    from: u64,
    to: u64,
    chunk: u32,
    map: Map,
    source: Option<Root>,
    destination: Option<Root>,
    reading: Option<skein_lib::Token>,
    writing: Option<skein_lib::Token>,
    entries: Box<[file::Entry]>,
    next: usize,
    last: bool,
    length: u64,
    offset: u64,
    transfer: u32,
    step: CopyStep,
    progress: Progress,
}
pub(crate) fn copy(root: Root, from: u64, chunk: u32, below: &mut crate::Below) -> Copying {
    let to = from.checked_add(1);
    let valid = from != 0 && to.is_some() && chunk != 0;
    let to = to.unwrap_or(from);
    if valid {
        below.transcripts.push(file::Request::MakeDirectory {
            owner: OWNER,
            root: root.token,
            path: generation(to),
            mode: 0o700,
        });
    }
    Copying {
        root,
        from,
        to,
        chunk,
        map: Map::Store,
        source: None,
        destination: None,
        reading: None,
        writing: None,
        entries: Box::new([]),
        next: 0,
        last: false,
        length: 0,
        offset: 0,
        transfer: 0,
        step: CopyStep::Generation,
        progress: if valid { Progress::Pending } else { Progress::Failed },
    }
}
fn copy_fail(copying: &mut Copying, queue: &mut Queue<file::Request>) -> Progress {
    copying.step = CopyStep::Failing;
    if let Some(token) = copying.writing.take() {
        queue.push(file::Request::Close { owner: OWNER, file: token });
        return Progress::Pending;
    }
    if let Some(token) = copying.reading.take() {
        queue.push(file::Request::Close { owner: OWNER, file: token });
        return Progress::Pending;
    }
    if let Some(root) = copying.source.take() {
        queue.push(file::Request::Close { owner: OWNER, file: root.token });
        return Progress::Pending;
    }
    if let Some(root) = copying.destination.take() {
        queue.push(file::Request::Close { owner: OWNER, file: root.token });
        return Progress::Pending;
    }
    Progress::Failed
}
fn copy_page(copying: &mut Copying, queue: &mut Queue<file::Request>) {
    copying.entries = Box::new([]);
    queue.push(file::Request::ListPage {
        owner: OWNER,
        directory: copying.source.expect("source map open").token,
        max: PAGE,
    });
    copying.step = CopyStep::Page;
}
fn copy_next(copying: &mut Copying, queue: &mut Queue<file::Request>) -> Progress {
    if let Some(entry) = copying.entries.get(copying.next) {
        if entry.kind != Kind::File {
            return copy_fail(copying, queue);
        }
        queue.push(file::Request::OpenReadNoFollow {
            owner: OWNER,
            root: copying.source.expect("source map open").fd,
            name: entry.name.clone(),
        });
        copying.step = CopyStep::ReadOpen;
    } else if copying.last {
        queue.push(file::Request::SyncDirectory {
            owner: OWNER,
            root: copying.destination.expect("destination map open").fd,
        });
        copying.step = CopyStep::MapSync;
    } else {
        copy_page(copying, queue);
    }
    Progress::Pending
}
fn copy_read(copying: &mut Copying, queue: &mut Queue<file::Request>) {
    if copying.offset == copying.length {
        queue.push(file::Request::Sync { owner: OWNER, file: copying.writing.expect("copy file open") });
        copying.step = CopyStep::FileSync;
    } else {
        let remaining = copying.length.checked_sub(copying.offset).expect("copy stays within opened length");
        copying.transfer = u32::try_from(remaining.min(u64::from(copying.chunk))).expect("bounded by chunk");
        queue.push(file::Request::ReadAt {
            owner: OWNER,
            file: copying.reading.expect("source file open"),
            offset: copying.offset,
            max: copying.transfer,
        });
        copying.step = CopyStep::Read;
    }
}
pub(crate) fn copied(copying: &mut Copying, arrived: Arrived, below: &mut crate::Below) -> Progress {
    if copying.progress != Progress::Pending {
        return Progress::Failed;
    }
    let progress = copy_arrived(copying, arrived, &mut below.transcripts);
    copying.progress = progress;
    progress
}
#[expect(clippy::too_many_lines, reason = "exhaustive copy transitions, one file request in flight (store.md 10.2)")]
fn copy_arrived(copying: &mut Copying, arrived: Arrived, queue: &mut Queue<file::Request>) -> Progress {
    match core::mem::replace(&mut copying.step, CopyStep::Finished) {
        CopyStep::Generation => {
            let file::Event::Made { .. } = arrived.event else {
                return copy_fail(copying, queue);
            };
            queue.push(file::Request::MakeDirectory {
                owner: OWNER,
                root: copying.root.token,
                path: map(copying.to, Map::Store),
                mode: 0o700,
            });
            copying.step = CopyStep::StoreDirectory;
        }
        CopyStep::StoreDirectory => {
            let file::Event::Made { .. } = arrived.event else {
                return copy_fail(copying, queue);
            };
            queue.push(file::Request::OpenDirectory {
                owner: OWNER,
                root: copying.root.fd,
                name: map(copying.to, Map::Store),
                no_follow: true,
            });
            copying.step = CopyStep::Destination;
        }
        CopyStep::Destination => {
            let file::Event::Opened { file, .. } = arrived.event else {
                return copy_fail(copying, queue);
            };
            // An Opened terminal always carries its descriptor from the service.
            let fd = arrived.fd.expect("opened directory has its descriptor");
            copying.destination = Some(Root { token: file, fd });
            queue.push(file::Request::OpenDirectory {
                owner: OWNER,
                root: copying.root.fd,
                name: map(copying.from, copying.map),
                no_follow: true,
            });
            copying.step = CopyStep::Source;
        }
        CopyStep::Source => {
            let file::Event::Opened { file, .. } = arrived.event else {
                return copy_fail(copying, queue);
            };
            let fd = arrived.fd.expect("opened directory has its descriptor");
            copying.source = Some(Root { token: file, fd });
            copy_page(copying, queue);
        }
        CopyStep::Page => {
            let file::Event::Listed { entries, .. } = arrived.event else {
                return copy_fail(copying, queue);
            };
            copying.last = entries.len() < usize::try_from(PAGE).expect("page fits usize");
            copying.entries = entries;
            copying.next = 0;
            return copy_next(copying, queue);
        }
        CopyStep::ReadOpen => {
            let file::Event::Opened { file, len, .. } = arrived.event else {
                return copy_fail(copying, queue);
            };
            copying.reading = Some(file);
            copying.length = len;
            copying.offset = 0;
            let entry = copying.entries.get(copying.next).expect("current entry retained");
            queue.push(file::Request::CreateNoFollow {
                owner: OWNER,
                root: copying.destination.expect("destination map open").fd,
                name: entry.name.clone(),
                mode: 0o600,
            });
            copying.step = CopyStep::WriteOpen;
        }
        CopyStep::WriteOpen => {
            let file::Event::Opened { file, .. } = arrived.event else {
                return copy_fail(copying, queue);
            };
            copying.writing = Some(file);
            copy_read(copying, queue);
        }
        CopyStep::Read => {
            let file::Event::Read { bytes, .. } = arrived.event else {
                return copy_fail(copying, queue);
            };
            if bytes.len() != usize::try_from(copying.transfer).expect("chunk fits usize") {
                return copy_fail(copying, queue);
            }
            queue.push(file::Request::WriteAt {
                owner: OWNER,
                file: copying.writing.expect("copy file open"),
                offset: copying.offset,
                bytes,
            });
            copying.step = CopyStep::Write;
        }
        CopyStep::Write => {
            let file::Event::Written { .. } = arrived.event else {
                return copy_fail(copying, queue);
            };
            copying.offset = copying.offset.checked_add(u64::from(copying.transfer)).expect("within opened length");
            copy_read(copying, queue);
        }
        CopyStep::FileSync => {
            let file::Event::Synced { .. } = arrived.event else {
                return copy_fail(copying, queue);
            };
            let file = copying.writing.take().expect("synced copy file open");
            queue.push(file::Request::Close { owner: OWNER, file });
            copying.step = CopyStep::WriteClose;
        }
        CopyStep::WriteClose => {
            let file::Event::Closed { .. } = arrived.event else {
                return copy_fail(copying, queue);
            };
            let file = copying.reading.take().expect("source file open");
            queue.push(file::Request::Close { owner: OWNER, file });
            copying.step = CopyStep::ReadClose;
        }
        CopyStep::ReadClose => {
            let file::Event::Closed { .. } = arrived.event else {
                return copy_fail(copying, queue);
            };
            copying.next = copying.next.checked_add(1).expect("page index bounded by PAGE");
            return copy_next(copying, queue);
        }
        CopyStep::MapSync => {
            let file::Event::Synced { .. } = arrived.event else {
                return copy_fail(copying, queue);
            };
            let root = copying.source.take().expect("source map open");
            queue.push(file::Request::Close { owner: OWNER, file: root.token });
            copying.step = CopyStep::SourceClose;
        }
        CopyStep::SourceClose => {
            let file::Event::Closed { .. } = arrived.event else {
                return copy_fail(copying, queue);
            };
            let root = copying.destination.take().expect("destination map open");
            queue.push(file::Request::Close { owner: OWNER, file: root.token });
            copying.step = CopyStep::DestinationClose;
        }
        CopyStep::DestinationClose => {
            let file::Event::Closed { .. } = arrived.event else {
                return copy_fail(copying, queue);
            };
            copying.entries = Box::new([]);
            match copying.map {
                Map::Store => {
                    copying.map = Map::Secrets;
                    queue.push(file::Request::OpenPrivate {
                        owner: OWNER,
                        root: copying.root.token,
                        path: map(copying.to, Map::Secrets),
                    });
                    copying.step = CopyStep::Destination;
                }
                Map::Secrets => {
                    queue.push(file::Request::OpenDirectory {
                        owner: OWNER,
                        root: copying.root.fd,
                        name: Box::from(GENERATIONS),
                        no_follow: true,
                    });
                    copying.step = CopyStep::Generations;
                }
            }
        }
        CopyStep::Generations => {
            let file::Event::Opened { file, .. } = arrived.event else {
                return copy_fail(copying, queue);
            };
            let fd = arrived.fd.expect("opened directory has its descriptor");
            copying.destination = Some(Root { token: file, fd });
            queue.push(file::Request::SyncDirectory { owner: OWNER, root: fd });
            copying.step = CopyStep::GenerationsSync;
        }
        CopyStep::GenerationsSync => {
            let file::Event::Synced { .. } = arrived.event else {
                return copy_fail(copying, queue);
            };
            let root = copying.destination.take().expect("generations open");
            queue.push(file::Request::Close { owner: OWNER, file: root.token });
            copying.step = CopyStep::GenerationsClose;
        }
        CopyStep::GenerationsClose => {
            let file::Event::Closed { .. } = arrived.event else {
                return copy_fail(copying, queue);
            };
            return Progress::Done;
        }
        CopyStep::Failing => return copy_fail(copying, queue),
        CopyStep::Finished => return Progress::Failed,
    }
    Progress::Pending
}

/// `active` replaced by io's durable replace, conditional on what open read
/// (store.md 10.2, step 7). After `Done`, `active` holds the new number and digest.
/// A `Conflict` or `Failed` (including committed) stops this start.
#[derive(Debug)]
pub(crate) struct Switching {
    number: Option<u64>,
    active: Option<Active>,
    progress: Progress,
}
pub(crate) fn switch(root: Root, active: Active, below: &mut crate::Below) -> Switching {
    let number = active.number.checked_add(1);
    if let Some(number) = number {
        below.transcripts.push(file::Request::Store {
            owner: OWNER,
            root: root.token,
            path: Box::from(ACTIVE),
            bytes: encode(number),
            expected: file::Expect::Digest(active.digest),
            no_follow: true,
        });
    }
    Switching { number, active: None, progress: if number.is_some() { Progress::Pending } else { Progress::Failed } }
}
pub(crate) fn switched(switching: &mut Switching, arrived: Arrived) -> Progress {
    if switching.progress != Progress::Pending {
        return Progress::Failed;
    }
    let file::Event::Stored { digest, .. } = arrived.event else {
        switching.progress = Progress::Failed;
        return Progress::Failed;
    };
    switching.active = Some(Active { number: switching.number.expect("switch requested a next number"), digest });
    switching.progress = Progress::Done;
    switching.progress
}

#[derive(Debug)]
enum DropStep {
    Opening,
    Renaming,
    BeforeRemoval,
    Removing(Removing),
    AfterRemoval,
    Closing(Progress),
    Finished,
}
/// The previous generation renamed to `dropped`, generations synced, removed as
/// a stray, and generations synced again (store.md 10.2, step 1). Active 1 and a
/// `NotFound` rename finish successfully. All handles use the transcripts driver.
#[derive(Debug)]
pub(crate) struct Dropping {
    previous: u64,
    root: Option<Root>,
    step: DropStep,
    progress: Progress,
}
pub(crate) fn drop_previous(root: Root, active: u64, below: &mut crate::Below) -> Dropping {
    if active > FIRST {
        below.transcripts.push(file::Request::OpenDirectory {
            owner: OWNER,
            root: root.fd,
            name: Box::from(GENERATIONS),
            no_follow: true,
        });
    }
    Dropping {
        previous: active.saturating_sub(1),
        root: None,
        step: DropStep::Opening,
        progress: if active == FIRST {
            Progress::Done
        } else if active == 0 {
            Progress::Failed
        } else {
            Progress::Pending
        },
    }
}
fn drop_close(dropping: &mut Dropping, progress: Progress, queue: &mut Queue<file::Request>) -> Progress {
    if let Some(root) = dropping.root.take() {
        queue.push(file::Request::Close { owner: OWNER, file: root.token });
        dropping.step = DropStep::Closing(progress);
        Progress::Pending
    } else {
        progress
    }
}
pub(crate) fn dropped(dropping: &mut Dropping, arrived: Arrived, below: &mut crate::Below) -> Progress {
    if dropping.progress != Progress::Pending {
        return Progress::Failed;
    }
    let progress = drop_arrived(dropping, arrived, &mut below.transcripts);
    dropping.progress = progress;
    progress
}
fn drop_arrived(dropping: &mut Dropping, arrived: Arrived, queue: &mut Queue<file::Request>) -> Progress {
    match core::mem::replace(&mut dropping.step, DropStep::Finished) {
        DropStep::Opening => {
            let file::Event::Opened { file, .. } = arrived.event else {
                return drop_close(dropping, Progress::Failed, queue);
            };
            let fd = arrived.fd.expect("opened directory has its descriptor");
            dropping.root = Some(Root { token: file, fd });
            queue.push(file::Request::Rename {
                owner: OWNER,
                from_root: fd,
                from: Box::from(Decimal::of(dropping.previous).as_bytes()),
                to_root: fd,
                to: Box::from(b"dropped".as_slice()),
            });
            dropping.step = DropStep::Renaming;
        }
        DropStep::Renaming => {
            let file::Event::Renamed { .. } = arrived.event else {
                let file::Event::Failed { error, .. } = arrived.event else {
                    return drop_close(dropping, Progress::Failed, queue);
                };
                let progress =
                    if error == skein_io::kernel::Error::NotFound { Progress::Done } else { Progress::Failed };
                return drop_close(dropping, progress, queue);
            };
            queue
                .push(file::Request::SyncDirectory { owner: OWNER, root: dropping.root.expect("generations open").fd });
            dropping.step = DropStep::BeforeRemoval;
        }
        DropStep::BeforeRemoval => {
            let file::Event::Synced { .. } = arrived.event else {
                return drop_close(dropping, Progress::Failed, queue);
            };
            let removing = remove(dropping.root.expect("generations open"), Box::from(b"dropped".as_slice()), queue);
            dropping.step = DropStep::Removing(removing);
        }
        DropStep::Removing(mut removing) => match removed(&mut removing, arrived, queue) {
            Progress::Pending => dropping.step = DropStep::Removing(removing),
            Progress::Failed => return drop_close(dropping, Progress::Failed, queue),
            Progress::Done => {
                queue.push(file::Request::SyncDirectory {
                    owner: OWNER,
                    root: dropping.root.expect("generations open").fd,
                });
                dropping.step = DropStep::AfterRemoval;
            }
        },
        DropStep::AfterRemoval => {
            let file::Event::Synced { .. } = arrived.event else {
                return drop_close(dropping, Progress::Failed, queue);
            };
            return drop_close(dropping, Progress::Done, queue);
        }
        DropStep::Closing(progress) => {
            let file::Event::Closed { .. } = arrived.event else {
                return Progress::Failed;
            };
            return progress;
        }
        DropStep::Finished => return Progress::Failed,
    }
    Progress::Pending
}

impl Copying {
    pub(crate) fn progress(&self) -> Progress {
        self.progress
    }
}

impl Dropping {
    pub(crate) fn progress(&self) -> Progress {
        self.progress
    }
}

impl Switching {
    pub(crate) fn progress(&self) -> Progress {
        self.progress
    }
    /// The caller keeps this only after `Done`.
    pub(crate) fn active(&self) -> Option<Active> {
        self.active
    }
}

/// A generation's manifest stored once by durable replace (store.md 2 and 10.2).
#[derive(Debug)]
pub(crate) struct Manifesting {
    progress: Progress,
}
pub(crate) fn store_manifest(root: Root, generation: u64, bytes: Box<[u8]>, below: &mut crate::Below) -> Manifesting {
    below.transcripts.push(file::Request::Store {
        owner: OWNER,
        root: root.token,
        path: join(&self::generation(generation), b"/manifest"),
        bytes,
        expected: file::Expect::Absent,
        no_follow: true,
    });
    Manifesting { progress: Progress::Pending }
}
pub(crate) fn manifested(manifesting: &mut Manifesting, arrived: Arrived) -> Progress {
    if manifesting.progress != Progress::Pending {
        return Progress::Failed;
    }
    manifesting.progress = match arrived.event {
        file::Event::Stored { .. } => Progress::Done,
        file::Event::Opened { .. }
        | file::Event::Refused { .. }
        | file::Event::Locked { .. }
        | file::Event::Busy { .. }
        | file::Event::Loaded { .. }
        | file::Event::Scanned { .. }
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
        | file::Event::Failed { .. } => Progress::Failed,
    };
    manifesting.progress
}
