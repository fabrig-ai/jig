//! The store's patch phase, between the open's checks and cuts (store.md 10.2).
use super::check::{self, Checking, Counted};
use crate::directory::{OWNER, Opening};
use crate::generations::{Copying, Dropping, Map, Progress};
use crate::{Arrived, Below, Event, Io, Limits, OpenRefusal, Phase, Root, Store, directory, generations, open};
use alloc::boxed::Box;
use skein_io::file;
use skein_lib::{Env, List, Queue};
mod walking;
pub(crate) use walking::Walking;

/// What open's checks leave for the phase (store.md 10.2, step 1).
#[derive(Debug)]
pub(crate) struct Pending {
    pub plan: super::plan::Plan,
    pub machines: super::machines::Machines,
    /// The row's levels before the walk, retained for the manifest.
    pub found: super::plan::Levels,
}

/// Closing counts two KV close bits (1, 2), then two directory
/// close bits (4, 8); Recovering two KV open bits (1, 2).
#[derive(Debug)]
pub(crate) enum Patching {
    Closing { closed: u8 },
    Dropping(Dropping),
    Copying(Copying),
    Mapping { secrets: bool },
    Recovering { opened: u8 },
    Walking(Box<Walking>),
    Sealing { number: u64, before: super::plan::Levels, counts: List<Counted> },
    Checking(Box<Checking>),
}
fn root(store: &Store) -> Root {
    store.directory.root.expect("patching holds the deployment lock")
}
fn active(store: &Store) -> u64 {
    store.directory.active.expect("patching follows active's checks").number
}
fn refuse(store: &mut Store, above: &mut Queue<Event>) {
    store.directory.opening = Opening::Idle;
    open::refuse(store, store.directory.owner, OpenRefusal::Failed, above);
}
fn state(store: &mut Store, phase: Patching) {
    store.directory.opening = Opening::Patching(Box::new(phase));
}
fn take(store: &mut Store) -> Box<Patching> {
    match core::mem::replace(&mut store.directory.opening, Opening::Idle) {
        Opening::Patching(phase) => phase,
        Opening::Idle
        | Opening::Locking
        | Opening::Reading
        | Opening::SurveyOpen(_)
        | Opening::SurveyPage { .. }
        | Opening::SurveyClose(_)
        | Opening::Refusing(_)
        | Opening::LeftoverOpen
        | Opening::LeftoverRemoving { .. }
        | Opening::LeftoverClose
        | Opening::Making(_)
        | Opening::FirstSecrets
        | Opening::FirstManifest(_)
        | Opening::FirstActive
        | Opening::CheckingOpen
        | Opening::CheckingPage { .. }
        | Opening::CheckingClose
        | Opening::StoreOpen
        | Opening::SecretsOpen
        | Opening::Stores
        | Opening::SweepOpen { .. }
        | Opening::SweepPage(_)
        | Opening::SweepFile(_)
        | Opening::SweepRemoving { .. }
        | Opening::SweepSync { .. }
        | Opening::SweepClose(_)
        | Opening::TranscriptsMade
        | Opening::TranscriptsOpen
        | Opening::Pending(_) => unreachable!("phase routing checked"),
    }
}
pub(crate) fn begin(
    store: &mut Store,
    env: &Env<Limits>,
    pending: Box<Pending>,
    above: &mut Queue<Event>,
    below: &mut Below,
) {
    store.phase = Phase::Closed;
    if active(store).checked_add(1).is_none() {
        return refuse(store, above);
    }
    store.directory.patch = Some(pending);
    state(store, Patching::Closing { closed: 0 });
    close_stores(store, env, above, below);
}
fn kv_down(
    store: &mut Store,
    env: &Env<Limits>,
    secret: bool,
    request: skein_kv::Request,
    above: &mut Queue<Event>,
    below: &mut Below,
) {
    let kv_env = directory::env(store, env, secret);
    let mut events = Queue::with_capacity(skein_kv::MAX_OUT.above);
    if secret {
        skein_kv::down(&mut store.stores.secrets, &kv_env, request, &mut events, &mut below.secrets);
    } else {
        skein_kv::down(&mut store.stores.store, &kv_env, request, &mut events, &mut below.store);
    }
    directory::events_up(store, secret, &mut events, above, below);
    resume(store, env, above, below);
}
fn close_stores(store: &mut Store, env: &Env<Limits>, above: &mut Queue<Event>, below: &mut Below) {
    kv_down(store, env, false, skein_kv::Request::Close { owner: OWNER }, above, below);
    if store.phase != Phase::Stopped {
        kv_down(store, env, true, skein_kv::Request::Close { owner: OWNER }, above, below);
    }
}
fn close_directories(store: &mut Store, below: &mut Below) {
    let public = store.directory.store_root.take().expect("store directory open until KV closes");
    let private = store.directory.private_root.take().expect("secrets directory open until KV closes");
    below.store.push(file::Request::Close { owner: OWNER, file: public.token });
    below.secrets.push(file::Request::Close { owner: OWNER, file: private.token });
    // Release A's in-memory maps before opening the copy (store.md 10.2, Room).
    store.stores.store = skein_kv::Store::new(&store.limits.store);
    store.stores.secrets = skein_kv::Store::new(&store.limits.secrets);
}
fn dropping(store: &mut Store, above: &mut Queue<Event>, below: &mut Below) {
    let dropping = generations::drop_previous(root(store), active(store), below);
    match dropping.progress() {
        Progress::Pending => state(store, Patching::Dropping(dropping)),
        Progress::Done => copying(store, above, below),
        Progress::Failed => refuse(store, above),
    }
}
fn copying(store: &mut Store, above: &mut Queue<Event>, below: &mut Below) {
    let copying = generations::copy(root(store), active(store), store.limits.store.chunk, below);
    match copying.progress() {
        Progress::Pending => state(store, Patching::Copying(copying)),
        Progress::Done | Progress::Failed => refuse(store, above),
    }
}
fn open_copy(store: &mut Store, below: &mut Below) {
    let number = active(store).checked_add(1).expect("begin checked the next generation");
    below.store.push(file::Request::OpenDirectory {
        owner: OWNER,
        root: root(store).fd,
        name: generations::map(number, Map::Store),
        no_follow: true,
    });
    state(store, Patching::Mapping { secrets: false });
}
fn opened_copy(store: &mut Store, env: &Env<Limits>, above: &mut Queue<Event>, below: &mut Below) {
    let pending = store.directory.patch.take().expect("begin retained the plan until copy recovery");
    let Some(counts) = check::before(store) else { return refuse(store, above) };
    match walking::opened(store, pending, counts) {
        Some(walking) => {
            state(store, Patching::Walking(Box::new(walking)));
            walking::resume(store, env, above, below);
        }
        None => refuse(store, above),
    }
}
pub(crate) fn up(store: &mut Store, env: &Env<Limits>, io: Io, above: &mut Queue<Event>, below: &mut Below) {
    let (arrived, secret) = match io {
        Io::Store(arrived) => (arrived, false),
        Io::Secrets(arrived) => (arrived, true),
        Io::Transcripts(arrived) => return file_up(store, env, arrived, false, above, below),
    };
    // KV's close accepts a failed file close; patching refuses every file fault.
    if let file::Event::Failed { .. } = &arrived.event {
        return refuse(store, above);
    }
    if arrived.event.owner() == OWNER {
        return file_up(store, env, arrived, secret, above, below);
    }
    let kv_env = directory::env(store, env, secret);
    let mut events = Queue::with_capacity(skein_kv::MAX_OUT.above);
    if secret {
        skein_kv::up(&mut store.stores.secrets, &kv_env, arrived.event, &mut events, &mut below.secrets);
    } else {
        skein_kv::up(&mut store.stores.store, &kv_env, arrived.event, &mut events, &mut below.store);
    }
    for _ in 0..skein_kv::MAX_OUT.above {
        let Some(event) = events.pop() else {
            break;
        };
        kv(store, env, secret, event, above, below);
    }
}
pub(crate) fn kv(
    store: &mut Store,
    env: &Env<Limits>,
    secret: bool,
    event: skein_kv::Event,
    above: &mut Queue<Event>,
    below: &mut Below,
) {
    self::event(store, secret, event, above, below);
    resume(store, env, above, below);
}
/// Also serves `directory::events_up`'s synchronous events; no timed KV request here.
pub(crate) fn event(
    store: &mut Store,
    secret: bool,
    event: skein_kv::Event,
    above: &mut Queue<Event>,
    below: &mut Below,
) {
    if store.phase == Phase::Stopped {
        return;
    }
    match event {
        skein_kv::Event::Closed { .. } => {
            let phase = take(store);
            let mut closed = match *phase {
                Patching::Closing { closed } => closed,
                Patching::Dropping(_)
                | Patching::Copying(_)
                | Patching::Mapping { .. }
                | Patching::Recovering { .. }
                | Patching::Walking(_)
                | Patching::Sealing { .. }
                | Patching::Checking(_) => return refuse(store, above),
            };
            closed |= if secret { 2 } else { 1 };
            if closed == 3 {
                close_directories(store, below);
            }
            state(store, Patching::Closing { closed });
        }
        skein_kv::Event::Opened { .. } => {
            let phase = take(store);
            match *phase {
                Patching::Recovering { mut opened } => {
                    opened |= if secret { 2 } else { 1 };
                    state(store, Patching::Recovering { opened });
                }
                Patching::Closing { .. }
                | Patching::Mapping { .. }
                | Patching::Dropping(_)
                | Patching::Copying(_)
                | Patching::Walking(_)
                | Patching::Sealing { .. }
                | Patching::Checking(_) => refuse(store, above),
            }
        }
        skein_kv::Event::Committed { owner, number } => {
            if owner != skein_lib::Token::new(number) {
                return refuse(store, above);
            }
            let phase = take(store);
            match *phase {
                Patching::Walking(mut walking) => {
                    if !walking.committed(if secret { crate::KeptMap::Secrets } else { crate::KeptMap::Store }, number)
                    {
                        return refuse(store, above);
                    }
                    state(store, Patching::Walking(walking));
                }
                Patching::Sealing { number: expected, before, counts } if !secret && number == expected => {
                    // resume runs the check only after the seal is durable.
                    state(store, Patching::Sealing { number: 0, before, counts });
                }
                Patching::Closing { .. }
                | Patching::Dropping(_)
                | Patching::Copying(_)
                | Patching::Mapping { .. }
                | Patching::Recovering { .. }
                | Patching::Sealing { .. }
                | Patching::Checking(_) => refuse(store, above),
            }
        }
        skein_kv::Event::Refused { .. }
        | skein_kv::Event::Failed { .. }
        | skein_kv::Event::Got { .. }
        | skein_kv::Event::Loaded { .. }
        | skein_kv::Event::OpenFailed { .. }
        | skein_kv::Event::Recovered { .. }
        | skein_kv::Event::RecoveryFailed { .. } => refuse(store, above),
    }
}
/// Finish synchronous KV events with the current step's environment.
pub(crate) fn resume(store: &mut Store, env: &Env<Limits>, above: &mut Queue<Event>, below: &mut Below) {
    if store.phase == Phase::Stopped {
        return;
    }
    if let Some(phase) = directory::patching(store) {
        match *phase {
            Patching::Recovering { opened: 3 } => opened_copy(store, env, above, below),
            Patching::Walking(_) => walking::resume(store, env, above, below),
            Patching::Sealing { number: 0, .. } => checking(store, above, below),
            Patching::Recovering { .. }
            | Patching::Mapping { .. }
            | Patching::Closing { .. }
            | Patching::Dropping(_)
            | Patching::Copying(_)
            | Patching::Sealing { .. }
            | Patching::Checking(_) => {}
        }
    }
}
fn checking(store: &mut Store, above: &mut Queue<Event>, below: &mut Below) {
    let phase = take(store);
    let (before, counts) = match *phase {
        Patching::Sealing { number: 0, before, counts } => (before, counts),
        Patching::Closing { .. }
        | Patching::Dropping(_)
        | Patching::Copying(_)
        | Patching::Mapping { .. }
        | Patching::Recovering { .. }
        | Patching::Walking(_)
        | Patching::Sealing { .. }
        | Patching::Checking(_) => unreachable!("resume routed the durable seal"),
    };
    match check::start(store, before, counts, below) {
        Ok(checking) => state(store, Patching::Checking(Box::new(checking))),
        Err(why) => open::refuse(store, store.directory.owner, why, above),
    }
}
fn file_up(
    store: &mut Store,
    env: &Env<Limits>,
    arrived: Arrived,
    secret: bool,
    above: &mut Queue<Event>,
    below: &mut Below,
) {
    let phase = take(store);
    match *phase {
        Patching::Closing { closed } => closed_directory(store, closed, arrived, secret, above, below),
        Patching::Dropping(mut dropping) => match generations::dropped(&mut dropping, arrived, below) {
            Progress::Pending => state(store, Patching::Dropping(dropping)),
            Progress::Done => copying(store, above, below),
            Progress::Failed => refuse(store, above),
        },
        Patching::Copying(mut copying) => match generations::copied(&mut copying, arrived, below) {
            Progress::Pending => state(store, Patching::Copying(copying)),
            Progress::Done => open_copy(store, below),
            Progress::Failed => refuse(store, above),
        },
        Patching::Mapping { secrets } => opened_directory(store, env, secrets, arrived, above, below),
        Patching::Recovering { .. } | Patching::Walking(_) | Patching::Sealing { .. } => refuse(store, above),
        Patching::Checking(mut checking) => match check::up(&mut checking, arrived, below) {
            Ok(false) => state(store, Patching::Checking(checking)),
            Ok(true) => {
                let (before, counts) = checking.into_counts();
                super::switch::open(store, before, counts, above, below);
            }
            Err(why) => open::refuse(store, store.directory.owner, why, above),
        },
    }
}
fn closed_directory(
    store: &mut Store,
    mut closed: u8,
    arrived: Arrived,
    secret: bool,
    above: &mut Queue<Event>,
    below: &mut Below,
) {
    let file::Event::Closed { .. } = arrived.event else {
        return refuse(store, above);
    };
    closed |= if secret { 8 } else { 4 };
    if closed == 15 {
        dropping(store, above, below);
    } else {
        state(store, Patching::Closing { closed });
    }
}
fn opened_directory(
    store: &mut Store,
    env: &Env<Limits>,
    secrets: bool,
    arrived: Arrived,
    above: &mut Queue<Event>,
    below: &mut Below,
) {
    let Some(opened_root) = directory::opened(arrived) else {
        return refuse(store, above);
    };
    let Root { token: file, fd } = opened_root;
    if secrets {
        store.directory.private_root = Some(Root { token: file, fd });
        state(store, Patching::Recovering { opened: 0 });
        let public = store.directory.store_root.expect("copy's store directory opened first");
        kv_down(store, env, false, skein_kv::Request::Open { owner: OWNER, root: public.fd }, above, below);
        if store.phase != Phase::Stopped {
            kv_down(store, env, true, skein_kv::Request::Open { owner: OWNER, root: fd }, above, below);
        }
    } else {
        store.directory.store_root = Some(Root { token: file, fd });
        let number = active(store).checked_add(1).expect("next generation checked");
        below.secrets.push(file::Request::OpenPrivate {
            owner: OWNER,
            root: root(store).token,
            path: generations::map(number, Map::Secrets),
        });
        state(store, Patching::Mapping { secrets: true });
    }
}

/// A record's halt abandons the copy with its key and the step that refused it.
pub(crate) fn site(
    store: &mut Store,
    env: &Env<Limits>,
    owner: skein_lib::Token,
    patcher: Box<skein_codec::patch::Patcher>,
    result: Result<(), skein_codec::patch::Halt>,
    above: &mut Queue<Event>,
    below: &mut Below,
) {
    if owner != store.directory.owner {
        return refuse(store, above);
    }
    if directory::patching(store).is_none() {
        return refuse(store, above);
    }
    let phase = take(store);
    match *phase {
        Patching::Walking(mut walking) => {
            if let Err(why) = walking.answer(patcher, result) {
                return open::refuse(store, store.directory.owner, why, above);
            }
            state(store, Patching::Walking(walking));
            resume(store, env, above, below);
        }
        Patching::Closing { .. }
        | Patching::Dropping(_)
        | Patching::Copying(_)
        | Patching::Mapping { .. }
        | Patching::Recovering { .. }
        | Patching::Sealing { .. }
        | Patching::Checking(_) => refuse(store, above),
    }
}

/// Persistent state is counted by lib's common peak; copy, walk and check
/// buffers are disjoint. Both KV maps' bounds already include their commits.
pub(crate) fn worst_case(limits: &Limits, sets: &crate::PatchSets) -> Option<u64> {
    super::plan::bounds(sets).ok()?;
    // The counts' source and encoded row coexist with the manifest's buffer.
    // The first start reserves the ceiling even when its sets have no families.
    let codec = &crate::component_v3::CEILINGS;
    let manifest = crate::component_v3::Manifest::worst_case_heap(codec)?
        .checked_mul(2)?
        .checked_add(crate::component_v3::Manifest::worst_case_bytes(codec)?)?;
    let walking = walking::worst_case(limits)?;
    let copying = generations::operations_worst_case(limits.store.chunk)?;
    let checking = check::worst_case(limits, crate::component_v3::CEILINGS.manifest_counts)?;
    // Pending owns the machines during copying as well as walking.
    walking
        .max(copying)
        .max(checking)
        .max(super::plan::flights_worst_case(limits, sets)?)
        .checked_add(List::<Counted>::worst_case(crate::component_v3::CEILINGS.manifest_counts)?)?
        .checked_add(manifest)?
        .checked_add(super::machines::worst_case(sets, super::machines::Rooms::of(limits))?)?
        .checked_add(u64::try_from(size_of::<Patching>()).ok()?)?
        .checked_add(u64::try_from(size_of::<Pending>()).ok()?)?
        .checked_add(Queue::<Event>::worst_case(crate::MAX_OUT_DOWN)?)?
        .checked_add(Queue::<file::Request>::worst_case(skein_kv::MAX_OUT.below.checked_mul(2)?)?)?
        .checked_add(Queue::<skein_kv::Event>::worst_case(skein_kv::MAX_OUT.above)?)
}
