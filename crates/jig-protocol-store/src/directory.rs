//! Deployment generations and independent stores (protocol/store.md, 2 and 10.1).
use crate::generations::{self, Active, Manifesting, Map, PAGE, Progress, Removing};
use crate::patching::phase::{self, Patching, Pending};
use crate::{Arrived, Below, Event, Generations, Io, Limits, Older, OpenRefusal, Phase, Root, Store, open, secrets};
use alloc::boxed::Box;
use skein_io::{
    file,
    kernel::{Error, Kind},
};
use skein_lib::{Env, Queue, Token};

pub(crate) const OWNER: Token = Token::new(0x004a_4947);
#[derive(Clone, Copy, Debug)]
pub(crate) enum Survey {
    Top,
    Generations,
    First,
    Map(Map),
}
#[derive(Clone, Copy, Debug)]
pub(crate) enum Making {
    Generations,
    Generation,
    Store,
}
#[derive(Clone, Copy, Debug)]
pub(crate) enum SweepKind {
    Generations,
    Temporaries { first: bool },
}
#[derive(Debug)]
pub(crate) struct Sweep {
    kind: SweepKind,
    root: Root,
    entries: Box<[file::Entry]>,
    next: usize,
    dirty: bool,
    page_dirty: bool,
}
#[derive(Debug)]
pub(crate) enum Opening {
    Idle,
    Locking,
    Reading,
    SurveyOpen(Survey),
    SurveyPage { kind: Survey, root: Root },
    SurveyClose(Survey),
    Refusing(OpenRefusal),
    LeftoverOpen,
    LeftoverRemoving { root: Root, removing: Removing },
    LeftoverClose,
    Making(Making),
    FirstSecrets,
    FirstManifest(Manifesting),
    FirstActive,
    CheckingOpen,
    CheckingPage { root: Root, maps: u8 },
    CheckingClose,
    StoreOpen,
    SecretsOpen,
    Stores,
    SweepOpen { kind: SweepKind, dirty: bool },
    SweepPage(Sweep),
    SweepFile(Sweep),
    SweepRemoving { sweep: Sweep, removing: Removing },
    SweepSync { root: Root },
    SweepClose(Closing),
    TranscriptsMade,
    TranscriptsOpen,
    Patching(Box<Patching>),
    Pending(Box<crate::patching::switch::Pending>),
}
#[derive(Debug)]
pub(crate) struct Closing {
    kind: SweepKind,
    dirty: bool,
    restart: bool,
}
#[derive(Debug)]
pub(crate) struct Swept {
    root: Root,
    dirty: bool,
}
#[derive(Debug)]
pub(crate) struct Directory {
    pub disk: bool,
    pub patch: Option<Box<Pending>>,
    pub active: Option<Active>,
    pub transcripts: Option<Root>,
    pub root: Option<Root>,
    pub store_root: Option<Root>,
    pub owner: Token,
    pub opening: Opening,
    pub opened: u8,
    pub private_root: Option<Root>,
    pub generations_kind: Option<Kind>,
    pub leftover: bool,
    pub survey_maps: u8,
    pub survey_manifest: bool,
    pub swept: Option<Swept>,
}
impl Directory {
    pub fn new() -> Directory {
        Directory {
            disk: false,
            patch: None,
            active: None,
            transcripts: None,
            root: None,
            store_root: None,
            owner: OWNER,
            opening: Opening::Idle,
            opened: 0,
            private_root: None,
            generations_kind: None,
            leftover: false,
            survey_maps: 0,
            survey_manifest: false,
            swept: None,
        }
    }
}
pub(crate) fn env(store: &Store, env: &Env<Limits>, secret: bool) -> Env<skein_kv::Limits> {
    Env { now: env.now, wall: env.wall, limits: if secret { store.limits.secrets } else { store.limits.store } }
}
pub(crate) fn open(
    store: &mut Store,
    _env: &Env<Limits>,
    owner: Token,
    root: Root,
    installation: [u8; 16],
    above: &mut Queue<Event>,
    below: &mut Below,
) {
    if store.phase == Phase::Closed {
        store.installation = installation;
    }
    if !store.directory.disk {
        store.directory.transcripts = Some(root);
        open::open(store, owner, above, below);
        return;
    }
    store.directory.owner = owner;
    if store.phase != Phase::Closed || store.directory.root.is_some() {
        return refuse(store, above);
    }
    store.directory.root = Some(root);
    store.directory.opening = Opening::Locking;
    below.store.push(file::Request::Lock { owner: OWNER, root: root.token });
}
fn refuse(store: &mut Store, above: &mut Queue<Event>) {
    refusal(store, OpenRefusal::Failed, above);
}
fn refusal(store: &mut Store, why: OpenRefusal, above: &mut Queue<Event>) {
    open::refuse(store, store.directory.owner, why, above);
}
fn generation_refusal(store: &mut Store, why: Generations, above: &mut Queue<Event>) {
    refusal(store, OpenRefusal::Generations { why }, above);
}
fn made(event: file::Event) -> bool {
    match event {
        file::Event::Made { .. } => true,
        file::Event::Failed { error, .. } => error == Error::Exists,
        file::Event::Opened { .. }
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
        | file::Event::Cut { .. }
        | file::Event::Synced { .. }
        | file::Event::Closed { .. }
        | file::Event::Renamed { .. }
        | file::Event::Removed { .. }
        | file::Event::Listed { .. }
        | file::Event::Cancelled { .. } => false,
    }
}
pub(crate) fn opened(arrived: Arrived) -> Option<Root> {
    let file::Event::Opened { file, .. } = arrived.event else {
        return None;
    };
    Some(Root { token: file, fd: arrived.fd? })
}
fn deployment(store: &Store) -> Root {
    store.directory.root.expect("opening keeps deployment root")
}
fn active(store: &Store) -> u64 {
    store.directory.active.expect("active has been read or stored").number
}
fn open_directory(root: Root, name: Box<[u8]>, queue: &mut Queue<file::Request>) {
    queue.push(file::Request::OpenDirectory { owner: OWNER, root: root.fd, name, no_follow: true });
}
fn list(root: Root, queue: &mut Queue<file::Request>) {
    queue.push(file::Request::ListPage { owner: OWNER, directory: root.token, max: PAGE });
}
fn close(root: Root, queue: &mut Queue<file::Request>) {
    queue.push(file::Request::Close { owner: OWNER, file: root.token });
}
fn survey(store: &mut Store, kind: Survey, below: &mut Below) {
    let name = match kind {
        Survey::Top => Box::from(b".".as_slice()),
        Survey::Generations => Box::from(generations::GENERATIONS),
        Survey::First => generations::generation(generations::FIRST),
        Survey::Map(map) => generations::map(generations::FIRST, map),
    };
    store.directory.opening = Opening::SurveyOpen(kind);
    open_directory(deployment(store), name, &mut below.store);
}
fn make(store: &mut Store, kind: Making, below: &mut Below) {
    let path = match kind {
        Making::Generations => Box::from(generations::GENERATIONS),
        Making::Generation => generations::generation(generations::FIRST),
        Making::Store => generations::map(generations::FIRST, Map::Store),
    };
    store.directory.opening = Opening::Making(kind);
    below.store.push(file::Request::MakeDirectory { owner: OWNER, root: deployment(store).token, path, mode: 0o700 });
}
fn check(store: &mut Store, below: &mut Below) {
    store.directory.opening = Opening::CheckingOpen;
    open_directory(deployment(store), generations::generation(active(store)), &mut below.store);
}
fn missing(store: &mut Store, above: &mut Queue<Event>) {
    generation_refusal(store, Generations::Missing { generation: active(store) }, above);
}
fn leftover(store: &mut Store, below: &mut Below) {
    if store.directory.leftover {
        store.directory.opening = Opening::LeftoverOpen;
        open_directory(deployment(store), Box::from(generations::GENERATIONS), &mut below.store);
    } else {
        sweep_open(store, SweepKind::Temporaries { first: true }, false, below);
    }
}
fn surveyed(store: &mut Store, kind: Survey, above: &mut Queue<Event>, below: &mut Below) {
    match kind {
        Survey::Top => match store.directory.generations_kind {
            Some(Kind::Directory) => survey(store, Survey::Generations, below),
            Some(Kind::File | Kind::Symlink | Kind::Other) => generation_refusal(store, Generations::Unnamed, above),
            None => leftover(store, below),
        },
        Survey::Generations => {
            if store.directory.leftover {
                survey(store, Survey::First, below);
            } else {
                leftover(store, below);
            }
        }
        Survey::First => {
            if store.directory.survey_maps & 1 != 0 {
                survey(store, Survey::Map(Map::Store), below);
            } else if store.directory.survey_maps & 2 != 0 {
                survey(store, Survey::Map(Map::Secrets), below);
            } else {
                leftover(store, below);
            }
        }
        Survey::Map(Map::Store) => {
            if store.directory.survey_maps & 2 != 0 {
                survey(store, Survey::Map(Map::Secrets), below);
            } else {
                leftover(store, below);
            }
        }
        Survey::Map(Map::Secrets) => leftover(store, below),
    }
}
fn refusing(store: &mut Store, root: Root, why: OpenRefusal, below: &mut Below) {
    close(root, &mut below.store);
    store.directory.opening = Opening::Refusing(why);
}
fn survey_page(store: &mut Store, kind: Survey, root: Root, entries: Box<[file::Entry]>, below: &mut Below) {
    for entry in &entries {
        match kind {
            Survey::Top => {
                if entry.name.as_ref() == b"store"
                    || entry.name.as_ref() == b"secrets"
                    || entry.name.as_ref() == generations::TRANSCRIPTS
                {
                    return refusing(store, root, OpenRefusal::OlderBuild { why: Older::Directory }, below);
                }
                if entry.name.as_ref() == generations::GENERATIONS {
                    store.directory.generations_kind = Some(entry.kind);
                }
            }
            Survey::Generations => {
                if entry.name.as_ref() != b"1" || entry.kind != Kind::Directory {
                    return refusing(store, root, OpenRefusal::Generations { why: Generations::Unnamed }, below);
                }
                store.directory.leftover = true;
            }
            Survey::First => {
                if entry.kind == Kind::File && first_manifest(&entry.name) && !store.directory.survey_manifest {
                    store.directory.survey_manifest = true;
                    continue;
                }
                let bit = if entry.name.as_ref() == b"store" {
                    1
                } else if entry.name.as_ref() == b"secrets" {
                    2
                } else {
                    0
                };
                if bit == 0 || entry.kind != Kind::Directory {
                    return refusing(store, root, OpenRefusal::Generations { why: Generations::Unnamed }, below);
                }
                store.directory.survey_maps |= bit;
            }
            Survey::Map(Map::Store | Map::Secrets) => {
                return refusing(store, root, OpenRefusal::Generations { why: Generations::Unnamed }, below);
            }
        }
    }
    if entries.is_empty() {
        store.directory.opening = Opening::SurveyClose(kind);
        close(root, &mut below.store);
    } else {
        store.directory.opening = Opening::SurveyPage { kind, root };
        list(root, &mut below.store);
    }
}
/// The one file a cut first start can leave beside its empty maps (store.md 2).
fn first_manifest(name: &[u8]) -> bool {
    if name == b"manifest" {
        return true;
    }
    let Some(hex) = name.strip_prefix(b"manifest.skein-") else {
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
/// The checks in open.rs have passed. Only now may strays and transcripts change.
pub(crate) fn checked(store: &mut Store, above: &mut Queue<Event>, below: &mut Below) {
    if !store.directory.disk {
        crate::transcripts::cut::begin(store, above, below);
        return;
    }
    if pending(store).is_some() {
        open_directory(deployment(store), Box::from(generations::TRANSCRIPTS), &mut below.transcripts);
    } else {
        sweep_open(store, SweepKind::Generations, false, below);
    }
}
fn sweep_open(store: &mut Store, kind: SweepKind, dirty: bool, below: &mut Below) {
    let name = match kind {
        SweepKind::Generations => generations::GENERATIONS,
        SweepKind::Temporaries { .. } => b".",
    };
    store.directory.opening = Opening::SweepOpen { kind, dirty };
    open_directory(deployment(store), Box::from(name), &mut below.transcripts);
}
fn sweep_next(store: &mut Store, mut sweep: Sweep, below: &mut Below) {
    // A page holds at most PAGE entries; skip retained entries in this one step.
    for _ in 0..PAGE {
        let Some(entry) = sweep.entries.get(sweep.next) else {
            break;
        };
        sweep.next = sweep.next.checked_add(1).expect("bounded page index");
        let remove = match sweep.kind {
            SweepKind::Generations => {
                let number = generations::number(&entry.name);
                number != Some(active(store)) && !(active(store) > 1 && number == active(store).checked_sub(1))
            }
            SweepKind::Temporaries { .. } => generations::is_temporary(&entry.name),
        };
        if !remove {
            continue;
        }
        sweep.dirty = true;
        sweep.page_dirty = true;
        if entry.kind == Kind::Directory {
            let removing = generations::remove(sweep.root, entry.name.clone(), &mut below.transcripts);
            store.directory.opening = Opening::SweepRemoving { sweep, removing };
        } else {
            below.transcripts.push(file::Request::Remove {
                owner: OWNER,
                root: sweep.root.fd,
                name: entry.name.clone(),
                directory: false,
            });
            store.directory.opening = Opening::SweepFile(sweep);
        }
        return;
    }
    if sweep.page_dirty {
        close(sweep.root, &mut below.transcripts);
        store.directory.opening = Opening::SweepClose(Closing { kind: sweep.kind, dirty: sweep.dirty, restart: true });
    } else if !sweep.entries.is_empty() {
        sweep.entries = Box::new([]);
        list(sweep.root, &mut below.transcripts);
        store.directory.opening = Opening::SweepPage(sweep);
    } else {
        match sweep.kind {
            SweepKind::Generations => {
                store.directory.swept = Some(Swept { root: sweep.root, dirty: sweep.dirty });
                sweep_open(store, SweepKind::Temporaries { first: false }, false, below);
            }
            SweepKind::Temporaries { .. } => {
                close(sweep.root, &mut below.transcripts);
                store.directory.opening =
                    Opening::SweepClose(Closing { kind: sweep.kind, dirty: sweep.dirty, restart: false });
            }
        }
    }
}
fn directory(store: &mut Store, env: &Env<Limits>, arrived: Arrived, above: &mut Queue<Event>, below: &mut Below) {
    if pending(store).is_some() {
        return crate::patching::switch::up(store, arrived, above, below);
    }
    match core::mem::replace(&mut store.directory.opening, Opening::Idle) {
        Opening::Locking => locked(store, arrived, above, below),
        Opening::Reading => active_read(store, arrived, above, below),
        Opening::SurveyOpen(kind) => survey_opened(store, kind, arrived, above, below),
        Opening::SurveyPage { kind, root } => survey_listed(store, kind, root, arrived, above, below),
        Opening::SurveyClose(kind) => survey_closed(store, kind, arrived, above, below),
        Opening::Refusing(why) => refused(store, why, arrived, above),
        Opening::LeftoverOpen => leftover_opened(store, arrived, above, below),
        Opening::LeftoverRemoving { root, removing } => leftover_removed(store, root, removing, arrived, above, below),
        Opening::LeftoverClose => leftover_closed(store, arrived, above, below),
        Opening::Making(kind) => made_generation(store, kind, arrived, above, below),
        Opening::FirstSecrets => first_secrets_opened(store, env, arrived, above, below),
        Opening::FirstManifest(manifesting) => first_manifest_stored(store, manifesting, arrived, above, below),
        Opening::FirstActive => first_active_stored(store, arrived, above, below),
        Opening::CheckingOpen => checking_opened(store, arrived, above, below),
        Opening::CheckingPage { root, maps } => checking_listed(store, root, maps, arrived, above, below),
        Opening::CheckingClose => checking_closed(store, arrived, above, below),
        Opening::StoreOpen => store_opened(store, env, arrived, above, below),
        Opening::SecretsOpen => secrets_opened(store, env, arrived, above, below),
        Opening::SweepOpen { kind, dirty } => sweep_opened(store, kind, dirty, arrived, above, below),
        Opening::SweepPage(sweep) => sweep_listed(store, sweep, arrived, above, below),
        Opening::SweepFile(sweep) => sweep_file_removed(store, sweep, arrived, above, below),
        Opening::SweepRemoving { sweep, removing } => sweep_tree_removed(store, sweep, removing, arrived, above, below),
        Opening::SweepSync { root } => sweep_synced(store, root, arrived, above, below),
        Opening::SweepClose(closing) => sweep_closed(store, closing, arrived, above, below),
        Opening::TranscriptsMade => transcripts_made(store, env, arrived, above, below),
        Opening::TranscriptsOpen => transcripts_opened(store, arrived, above, below),
        Opening::Pending(_) => unreachable!("pending routed before taking opening"),
        Opening::Idle | Opening::Stores | Opening::Patching(_) => refuse(store, above),
    }
}
fn locked(store: &mut Store, arrived: Arrived, above: &mut Queue<Event>, below: &mut Below) {
    if let file::Event::Busy { .. } = arrived.event {
        return generation_refusal(store, Generations::Busy, above);
    }
    let file::Event::Locked { .. } = arrived.event else {
        return refuse(store, above);
    };
    read_active(store, below);
}
fn read_active(store: &mut Store, below: &mut Below) {
    let root = deployment(store);
    store.directory.opening = Opening::Reading;
    below.store.push(file::Request::Load {
        owner: OWNER,
        root: root.token,
        path: Box::from(generations::ACTIVE),
        max: generations::ACTIVE_BYTES,
        no_follow: true,
    });
}
fn active_read(store: &mut Store, arrived: Arrived, above: &mut Queue<Event>, below: &mut Below) {
    if let file::Event::Failed { error: Error::NotFound, .. } = arrived.event {
        // A first start writes the manifest's counts: its sets fit them first.
        if let Err(why) = crate::patching::plan::bounds(&store.patch_sets) {
            return refusal(store, OpenRefusal::Set { owner: why.owner, patch: why.patch }, above);
        }
        survey(store, Survey::Top, below);
        return;
    }
    if let file::Event::TooLarge { .. } = arrived.event {
        return generation_refusal(store, Generations::Unreadable, above);
    }
    let file::Event::Loaded { bytes, .. } = arrived.event else {
        return refuse(store, above);
    };
    let Some(number) = generations::parse(&bytes) else {
        return generation_refusal(store, Generations::Unreadable, above);
    };
    store.directory.active = Some(Active { number, digest: skein_io::digest::digest(&bytes) });
    check(store, below);
}
fn survey_opened(store: &mut Store, kind: Survey, arrived: Arrived, above: &mut Queue<Event>, below: &mut Below) {
    let Some(root) = opened(arrived) else {
        return refuse(store, above);
    };
    store.directory.opening = Opening::SurveyPage { kind, root };
    list(root, &mut below.store);
}
fn survey_listed(
    store: &mut Store,
    kind: Survey,
    root: Root,
    arrived: Arrived,
    above: &mut Queue<Event>,
    below: &mut Below,
) {
    let file::Event::Listed { entries, .. } = arrived.event else {
        return refuse(store, above);
    };
    survey_page(store, kind, root, entries, below);
}
fn survey_closed(store: &mut Store, kind: Survey, arrived: Arrived, above: &mut Queue<Event>, below: &mut Below) {
    let file::Event::Closed { .. } = arrived.event else {
        return refuse(store, above);
    };
    surveyed(store, kind, above, below);
}
fn refused(store: &mut Store, why: OpenRefusal, arrived: Arrived, above: &mut Queue<Event>) {
    let file::Event::Closed { .. } = arrived.event else {
        return refuse(store, above);
    };
    refusal(store, why, above);
}
fn leftover_opened(store: &mut Store, arrived: Arrived, above: &mut Queue<Event>, below: &mut Below) {
    let Some(root) = opened(arrived) else {
        return refuse(store, above);
    };
    let removing = generations::remove(root, Box::from(b"1".as_slice()), &mut below.store);
    store.directory.opening = Opening::LeftoverRemoving { root, removing };
}
fn leftover_removed(
    store: &mut Store,
    root: Root,
    mut removing: Removing,
    arrived: Arrived,
    above: &mut Queue<Event>,
    below: &mut Below,
) {
    match generations::removed(&mut removing, arrived, &mut below.store) {
        Progress::Pending => store.directory.opening = Opening::LeftoverRemoving { root, removing },
        Progress::Done => {
            close(root, &mut below.store);
            store.directory.opening = Opening::LeftoverClose;
        }
        Progress::Failed => refuse(store, above),
    }
}
fn leftover_closed(store: &mut Store, arrived: Arrived, above: &mut Queue<Event>, below: &mut Below) {
    let file::Event::Closed { .. } = arrived.event else {
        return refuse(store, above);
    };
    sweep_open(store, SweepKind::Temporaries { first: true }, false, below);
}
fn made_generation(store: &mut Store, kind: Making, arrived: Arrived, above: &mut Queue<Event>, below: &mut Below) {
    let root = deployment(store);
    if !made(arrived.event) {
        return refuse(store, above);
    }
    match kind {
        Making::Generations => make(store, Making::Generation, below),
        Making::Generation => make(store, Making::Store, below),
        Making::Store => {
            store.directory.opening = Opening::FirstSecrets;
            below.secrets.push(file::Request::OpenPrivate {
                owner: OWNER,
                root: root.token,
                path: generations::map(generations::FIRST, Map::Secrets),
            });
        }
    }
}
fn first_secrets_opened(
    store: &mut Store,
    env: &Env<Limits>,
    arrived: Arrived,
    above: &mut Queue<Event>,
    below: &mut Below,
) {
    let Some(private) = opened(arrived) else {
        return refuse(store, above);
    };
    store.directory.private_root = Some(private);
    let mut counts = skein_lib::List::with_capacity(crate::component_v3::CEILINGS.manifest_counts);
    for set in store.patch_sets.sets {
        for family in set.families {
            let count = crate::codec::component::Count::new(
                &crate::component_v3::CEILINGS,
                crate::codec::component::CountParts { family: Box::from(family.name.as_bytes()), before: 0, after: 0 },
            )
            .expect("plan::bounds checked each family's name before this first start wrote");
            counts.push(count).expect("plan::bounds checked the families' count before this first start wrote");
        }
    }
    let bytes = generations::manifest(
        generations::FIRST,
        None,
        crate::format::NONE,
        crate::patching::plan::binary(&store.patch_sets),
        counts.as_slice(),
        0,
        None,
        env.wall,
    );
    let manifesting = generations::store_manifest(deployment(store), generations::FIRST, bytes, below);
    store.directory.opening = Opening::FirstManifest(manifesting);
}
fn first_manifest_stored(
    store: &mut Store,
    mut manifesting: Manifesting,
    arrived: Arrived,
    above: &mut Queue<Event>,
    below: &mut Below,
) {
    match generations::manifested(&mut manifesting, arrived) {
        Progress::Pending => {
            store.directory.opening = Opening::FirstManifest(manifesting);
            return;
        }
        Progress::Failed => return refuse(store, above),
        Progress::Done => {}
    }
    let root = deployment(store);
    store.directory.opening = Opening::FirstActive;
    below.store.push(file::Request::Store {
        owner: OWNER,
        root: root.token,
        path: Box::from(generations::ACTIVE),
        bytes: generations::encode(generations::FIRST),
        expected: file::Expect::Absent,
        no_follow: true,
    });
}
fn first_active_stored(store: &mut Store, arrived: Arrived, above: &mut Queue<Event>, below: &mut Below) {
    let file::Event::Stored { digest, .. } = arrived.event else {
        return refuse(store, above);
    };
    store.directory.active = Some(Active { number: generations::FIRST, digest });
    check(store, below);
}
fn checking_opened(store: &mut Store, arrived: Arrived, above: &mut Queue<Event>, below: &mut Below) {
    if let file::Event::Failed { error: Error::NotFound | Error::NotADirectory, .. } = arrived.event {
        return missing(store, above);
    }
    let Some(root) = opened(arrived) else {
        return refuse(store, above);
    };
    list(root, &mut below.store);
    store.directory.opening = Opening::CheckingPage { root, maps: 0 };
}
fn checking_listed(
    store: &mut Store,
    root: Root,
    mut maps: u8,
    arrived: Arrived,
    above: &mut Queue<Event>,
    below: &mut Below,
) {
    let file::Event::Listed { entries, .. } = arrived.event else {
        return refuse(store, above);
    };
    for entry in &entries {
        if entry.kind == Kind::Directory {
            if entry.name.as_ref() == b"store" {
                maps |= 1;
            }
            if entry.name.as_ref() == b"secrets" {
                maps |= 2;
            }
        }
    }
    if entries.is_empty() {
        if maps != 3 {
            let why = OpenRefusal::Generations { why: Generations::Missing { generation: active(store) } };
            return refusing(store, root, why, below);
        }
        close(root, &mut below.store);
        store.directory.opening = Opening::CheckingClose;
    } else {
        list(root, &mut below.store);
        store.directory.opening = Opening::CheckingPage { root, maps };
    }
}
fn checking_closed(store: &mut Store, arrived: Arrived, above: &mut Queue<Event>, below: &mut Below) {
    let root = deployment(store);
    let file::Event::Closed { .. } = arrived.event else {
        return refuse(store, above);
    };
    open_directory(root, generations::map(active(store), Map::Store), &mut below.store);
    store.directory.opening = Opening::StoreOpen;
}
fn store_opened(store: &mut Store, env: &Env<Limits>, arrived: Arrived, above: &mut Queue<Event>, below: &mut Below) {
    let root = deployment(store);
    let Some(opened) = opened(arrived) else {
        return refuse(store, above);
    };
    store.directory.store_root = Some(opened);
    if let Some(private) = store.directory.private_root.take() {
        open_stores(store, env, private, above, below);
    } else {
        store.directory.opening = Opening::SecretsOpen;
        below.secrets.push(file::Request::OpenPrivate {
            owner: OWNER,
            root: root.token,
            path: generations::map(active(store), Map::Secrets),
        });
    }
}
fn secrets_opened(store: &mut Store, env: &Env<Limits>, arrived: Arrived, above: &mut Queue<Event>, below: &mut Below) {
    let Some(opened) = opened(arrived) else {
        return refuse(store, above);
    };
    open_stores(store, env, opened, above, below);
}
fn sweep_opened(
    store: &mut Store,
    kind: SweepKind,
    dirty: bool,
    arrived: Arrived,
    above: &mut Queue<Event>,
    below: &mut Below,
) {
    let Some(root) = opened(arrived) else {
        return refuse(store, above);
    };
    list(root, &mut below.transcripts);
    store.directory.opening =
        Opening::SweepPage(Sweep { kind, root, entries: Box::new([]), next: 0, dirty, page_dirty: false });
}
fn sweep_listed(store: &mut Store, mut sweep: Sweep, arrived: Arrived, above: &mut Queue<Event>, below: &mut Below) {
    let file::Event::Listed { entries, .. } = arrived.event else {
        return refuse(store, above);
    };
    sweep.entries = entries;
    sweep.next = 0;
    sweep.page_dirty = false;
    sweep_next(store, sweep, below);
}
fn sweep_file_removed(store: &mut Store, sweep: Sweep, arrived: Arrived, above: &mut Queue<Event>, below: &mut Below) {
    let file::Event::Removed { .. } = arrived.event else {
        return refuse(store, above);
    };
    sweep_next(store, sweep, below);
}
fn sweep_tree_removed(
    store: &mut Store,
    sweep: Sweep,
    mut removing: Removing,
    arrived: Arrived,
    above: &mut Queue<Event>,
    below: &mut Below,
) {
    match generations::removed(&mut removing, arrived, &mut below.transcripts) {
        Progress::Pending => store.directory.opening = Opening::SweepRemoving { sweep, removing },
        Progress::Done => sweep_next(store, sweep, below),
        Progress::Failed => refuse(store, above),
    }
}
fn sweep_synced(store: &mut Store, root: Root, arrived: Arrived, above: &mut Queue<Event>, below: &mut Below) {
    let file::Event::Synced { .. } = arrived.event else {
        return refuse(store, above);
    };
    close(root, &mut below.transcripts);
    store.directory.opening =
        Opening::SweepClose(Closing { kind: SweepKind::Generations, dirty: true, restart: false });
}
fn sweep_closed(store: &mut Store, closing: Closing, arrived: Arrived, above: &mut Queue<Event>, below: &mut Below) {
    let root = deployment(store);
    let Closing { kind, dirty, restart } = closing;
    let file::Event::Closed { .. } = arrived.event else {
        return refuse(store, above);
    };
    if restart {
        sweep_open(store, kind, dirty, below);
        return;
    }
    match kind {
        SweepKind::Generations => {
            store.directory.opening = Opening::TranscriptsMade;
            below.transcripts.push(file::Request::MakeDirectory {
                owner: OWNER,
                root: root.token,
                path: Box::from(generations::TRANSCRIPTS),
                mode: 0o700,
            });
        }
        SweepKind::Temporaries { first: true } => make(store, Making::Generations, below),
        SweepKind::Temporaries { first: false } => {
            let swept = store.directory.swept.take().expect("generations swept before temporaries");
            if swept.dirty {
                below.transcripts.push(file::Request::SyncDirectory { owner: OWNER, root: swept.root.fd });
                store.directory.opening = Opening::SweepSync { root: swept.root };
            } else {
                close(swept.root, &mut below.transcripts);
                store.directory.opening =
                    Opening::SweepClose(Closing { kind: SweepKind::Generations, dirty: false, restart: false });
            }
        }
    }
}
fn transcripts_made(
    store: &mut Store,
    env: &Env<Limits>,
    arrived: Arrived,
    above: &mut Queue<Event>,
    below: &mut Below,
) {
    let root = deployment(store);
    if !made(arrived.event) {
        return refuse(store, above);
    }
    if let Some(pending) = store.directory.patch.take() {
        phase::begin(store, env, pending, above, below);
        return;
    }
    store.directory.opening = Opening::TranscriptsOpen;
    open_directory(root, Box::from(generations::TRANSCRIPTS), &mut below.transcripts);
}
fn transcripts_opened(store: &mut Store, arrived: Arrived, above: &mut Queue<Event>, below: &mut Below) {
    let Some(opened) = opened(arrived) else {
        return refuse(store, above);
    };
    store.directory.transcripts = Some(opened);
    crate::transcripts::cut::begin(store, above, below);
}
fn open_stores(store: &mut Store, env: &Env<Limits>, opened: Root, above: &mut Queue<Event>, below: &mut Below) {
    store.directory.private_root = Some(opened);
    store.directory.opening = Opening::Stores;
    let kv_env = self::env(store, env, false);
    let mut events = Queue::uncapped();
    let root = store.directory.store_root.expect("the store directory is open");
    skein_kv::down(
        &mut store.stores.store,
        &kv_env,
        skein_kv::Request::Open { owner: OWNER, root: root.fd },
        &mut events,
        &mut below.store,
    );
    events_up(store, false, &mut events, above, below);
    let kv_env = self::env(store, env, true);
    let mut events = Queue::uncapped();
    skein_kv::down(
        &mut store.stores.secrets,
        &kv_env,
        skein_kv::Request::Open { owner: OWNER, root: opened.fd },
        &mut events,
        &mut below.secrets,
    );
    events_up(store, true, &mut events, above, below);
}
pub(crate) fn up(store: &mut Store, env: &Env<Limits>, io: Io, above: &mut Queue<Event>, below: &mut Below) {
    if store.phase == Phase::Stopped {
        return;
    }
    if patching(store).is_some() {
        phase::up(store, env, io, above, below);
        return;
    }
    let (arrived, secret) = match io {
        Io::Store(arrived) => (arrived, false),
        Io::Secrets(arrived) => (arrived, true),
        Io::Transcripts(arrived) => {
            match store.phase {
                Phase::Ready { .. } => {
                    if arrived.event.owner() == OWNER {
                        if pending(store).is_some() && store.directory.transcripts.is_none() {
                            transcripts_opened(store, arrived, above, below);
                        } else {
                            directory(store, env, arrived, above, below);
                        }
                    } else if arrived.event.owner() == crate::transcripts::cut::OWNER {
                        crate::transcripts::cut::arrived(store, arrived, above, below);
                    } else if arrived.event.owner() == crate::transcripts::drop::OWNER {
                        crate::transcripts::drop::arrived(store, env, arrived, above, below);
                    } else if arrived.event.owner() == crate::transcripts::read::OWNER {
                        crate::transcripts::read::arrived(store, arrived, above, below);
                    } else {
                        crate::transcripts::write::arrived(store, env, arrived, above, below);
                    }
                }
                Phase::Closed => directory(store, env, arrived, above, below),
                Phase::Stopped => {}
            }
            return;
        }
    };
    if arrived.event.owner() == OWNER {
        directory(store, env, arrived, above, below);
        return;
    }
    let kv_env = self::env(store, env, secret);
    let mut events = Queue::uncapped();
    if secret {
        skein_kv::up(&mut store.stores.secrets, &kv_env, arrived.event, &mut events, &mut below.secrets);
    } else {
        skein_kv::up(&mut store.stores.store, &kv_env, arrived.event, &mut events, &mut below.store);
    }
    events_up(store, secret, &mut events, above, below);
}
pub(crate) fn events_up(
    store: &mut Store,
    secret: bool,
    events: &mut Queue<skein_kv::Event>,
    above: &mut Queue<Event>,
    below: &mut Below,
) {
    for _ in 0..events.len() {
        let Some(event) = events.pop() else {
            break;
        };
        if patching(store).is_some() {
            phase::event(store, secret, event, above, below);
            continue;
        }
        match event {
            skein_kv::Event::Opened { .. } => {
                store.directory.opened |= if secret { 2 } else { 1 };
                if store.directory.opened == 3 {
                    open::open(store, store.directory.owner, above, below);
                }
            }
            skein_kv::Event::Committed { owner, number } => {
                if secret {
                    secrets::committed(store, owner, above);
                } else if owner == Token::new(number) {
                    crate::transcripts::committed(store, number);
                    store.mark.committed(number);
                    above.push(Event::Committed { number, fill: store.stores.store.used() });
                } else {
                    store.phase = Phase::Stopped;
                    above.push(Event::Failed { number });
                }
            }
            skein_kv::Event::Refused { owner, .. } => {
                store.phase = Phase::Stopped;
                if !secret {
                    crate::transcripts::failed(store, Some(owner.raw()), above);
                }
            }
            skein_kv::Event::Failed { number, .. } => {
                store.phase = Phase::Stopped;
                if !secret {
                    crate::transcripts::failed(store, Some(number), above);
                }
            }
            skein_kv::Event::OpenFailed { .. } => refuse(store, above),
            skein_kv::Event::Recovered { .. } | skein_kv::Event::RecoveryFailed { .. } => store.phase = Phase::Stopped,
            skein_kv::Event::Got { .. } | skein_kv::Event::Loaded { .. } | skein_kv::Event::Closed { .. } => {
                store.phase = Phase::Stopped;
            }
        }
    }
}
pub(crate) fn fire(store: &mut Store, env: &Env<Limits>, above: &mut Queue<Event>, below: &mut Below) {
    for secret in [false, true] {
        let kv_env = self::env(store, env, secret);
        let mut events = Queue::uncapped();
        if secret {
            if skein_kv::is_ready(&store.stores.secrets, &kv_env.limits) {
                skein_kv::fire(&mut store.stores.secrets, &kv_env, &mut events, &mut below.secrets);
            }
        } else if skein_kv::is_ready(&store.stores.store, &kv_env.limits) {
            skein_kv::fire(&mut store.stores.store, &kv_env, &mut events, &mut below.store);
        }
        events_up(store, secret, &mut events, above, below);
        phase::resume(store, env, above, below);
    }
}

/// The one fork shared by file and KV routing.
pub(crate) fn patching(store: &Store) -> Option<&Patching> {
    match &store.directory.opening {
        Opening::Patching(phase) => Some(phase),
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
        | Opening::Pending(_) => None,
    }
}

pub(crate) fn pending(store: &Store) -> Option<&crate::patching::switch::Pending> {
    match &store.directory.opening {
        Opening::Pending(pending) => Some(pending),
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
        | Opening::Patching(_) => None,
    }
}

pub(crate) fn pending_mut(store: &mut Store) -> Option<&mut crate::patching::switch::Pending> {
    match &mut store.directory.opening {
        Opening::Pending(pending) => Some(pending),
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
        | Opening::Patching(_) => None,
    }
}
