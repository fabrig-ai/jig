//! Public transcript decode against fixture bytes, with deterministic file replies.
//! Shared by the fuzzy model and the isolated heap test; step tests exercise real file drivers.
use jig_protocol_store::{
    self as store, Event, KeptFamily, KeptMap, Owner, PatchSet, PatchSets, Request, Root, TranscriptRead, codec,
    format, keys,
};
use jig_store_world::root;
use jig_test_patches_jig::{frames, patches as jig};
use jig_test_patches_smith::patches as smith;
use skein_io::{file, kernel::Fd};
use skein_lib::{Env, Queue, Time, Token, Wall};

const OWNER: Token = Token::new(9);
const ROOM: u32 = store::MAX_OUT_DOWN;
const COMMITTED: u64 = 1024;
pub(crate) const FRAME_KEY_BYTES: usize = 18;
pub(crate) const SETS: PatchSets = PatchSets {
    sets: &[
        PatchSet {
            owner: Owner::Jig,
            name: jig_test_patches_jig::NAME,
            patches: jig::PATCHES,
            fresh: None,
            families: &[KeptFamily {
                flights: &[],
                name: "fixture-frame",
                texts: jig::kept::fixture_frame::TEXTS,
                map: KeptMap::Store,
                keys: &[],
                frames: Some(jig_test_patches_jig::FRAME_ROOT),
            }],
        },
        PatchSet {
            owner: Owner::Smith,
            name: jig_test_patches_smith::NAME,
            patches: smith::PATCHES,
            fresh: Some(jig_test_patches_smith::FRESH),
            families: &[KeptFamily {
                flights: &[],
                name: "fixture-turn",
                texts: smith::kept::fixture_turn::TEXTS,
                map: KeptMap::Store,
                keys: &[],
                frames: Some(jig_test_patches_smith::ROOT),
            }],
        },
    ],
};
fn env(limits: &store::Limits) -> Env<store::Limits> {
    Env { now: Time::ZERO, wall: Wall::EPOCH, limits: *limits }
}
pub(crate) fn closed(sets: PatchSets, limits: &store::Limits) -> (store::Store, Env<store::Limits>) {
    let mut maps = store::Stores {
        store: skein_kv::Store::memory(&limits.store),
        secrets: skein_kv::Store::memory(&limits.secrets),
    };
    let codec = codec::limits(&limits.core).expect("codec");
    let mut fmt = format::current(root::APPLICATION.version, root::DEPLOYMENT);
    for set in sets.sets {
        let mut kept = false;
        for family in set.families {
            kept |= !family.texts.is_empty();
        }
        let level = if kept { Some(u32::try_from(set.patches.len()).expect("patch count")) } else { None };
        match set.owner {
            Owner::Jig => fmt.levels.jig = level,
            Owner::Smith => fmt.levels.smith = level,
            Owner::Skein | Owner::Application => panic!("frames fixture owners"),
        }
    }
    let own = |key| keys::encode_own(&key, limits.store.key).expect("component key");
    let ops = Box::new([
        skein_kv::Op::Put { key: own(keys::Own::Format), value: fmt.encode().expect("format") },
        skein_kv::Op::Put { key: own(keys::Own::Layouts), value: format::layouts().encode() },
        skein_kv::Op::Put {
            key: keys::encode(&root::header(1).key(), limits.store.key).expect("header key"),
            value: codec::encode(&root::header(1), &codec, &[]).expect("header"),
        },
        skein_kv::Op::Put {
            key: keys::installation(limits.store.key).expect("installation"),
            value: codec::accounts::Installation { id: root::DEPLOYMENT }.encode(),
        },
        skein_kv::Op::Put {
            key: own(keys::Own::Transcript { task: frames::TASK }),
            value: codec::component::Transcript { length: COMMITTED, window: 0 }.encode().expect("places"),
        },
        skein_kv::Op::Put {
            key: own(keys::Own::Attempt { task: frames::TASK, attempt: frames::ATTEMPT }),
            value: codec::component::Attempt { first: 0 }.encode().expect("attempt"),
        },
    ]);
    let mut above = Queue::with_capacity(ROOM);
    let mut below = Queue::with_capacity(ROOM);
    skein_kv::down(
        &mut maps.store,
        &Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits.store },
        skein_kv::Request::Commit { owner: OWNER, ops },
        &mut above,
        &mut below,
    );
    let mut component = store::Store::over(limits, root::APPLICATION, &[], maps);
    component.patch_sets(sets);
    (component, env(limits))
}
pub(crate) fn open(component: &mut store::Store, env: &Env<store::Limits>) {
    let mut above = Queue::with_capacity(ROOM);
    let mut below = store::Below {
        store: Queue::with_capacity(ROOM),
        secrets: Queue::with_capacity(ROOM),
        transcripts: Queue::with_capacity(ROOM),
    };
    store::down(
        component,
        env,
        Request::Open {
            owner: OWNER,
            root: Root { token: Token::new(1), fd: Fd::new(3) },
            installation: root::DEPLOYMENT,
        },
        &mut above,
        &mut below,
    );
    assert!(matches!(above.pop(), Some(Event::Opened { pending: false, .. })));
}
pub(crate) use jig_store_world::patches::fixture_turn as turn;
// Six file operations and two patch stops cover one small fixture frame.
const STEPS: u32 = 8;
pub(crate) fn decode(
    component: &mut store::Store,
    env: &Env<store::Limits>,
    raw: &[u8],
    no_site: bool,
) -> Option<Box<[u8]>> {
    let mut above = Queue::with_capacity(ROOM);
    let mut below = store::Below {
        store: Queue::with_capacity(ROOM),
        secrets: Queue::with_capacity(ROOM),
        transcripts: Queue::with_capacity(ROOM),
    };
    store::down(
        component,
        env,
        Request::Transcript {
            owner: OWNER,
            read: TranscriptRead::Turn { task: frames::TASK, attempt: frames::ATTEMPT, turn: frames::TURN },
            rows: 1,
            bytes: COMMITTED,
        },
        &mut above,
        &mut below,
    );
    for _ in 0..STEPS {
        if let Some(event) = above.pop() {
            match event {
                Event::Turns { turns, .. } => return Some(turns.first().expect("selected turn").transcript.clone()),
                Event::Unread { .. } => return None,
                Event::Site { owner, of, key, site, at, mut patcher } => {
                    assert_eq!(of, Owner::Smith);
                    assert_eq!(key.len(), FRAME_KEY_BYTES);
                    let result = if no_site {
                        Err(skein_codec::patch::Halt::NoSite(site))
                    } else {
                        let (old, new) = patcher.parts();
                        smith::written(site, old, new, at, &mut smith::Context { key })
                    };
                    store::down(component, env, Request::Site { owner, patcher, result }, &mut above, &mut below);
                    continue;
                }
                other @ (Event::Switched { .. }
                | Event::Opened { .. }
                | Event::OpenRefused { .. }
                | Event::Committed { .. }
                | Event::Failed { .. }
                | Event::FrameTooLarge { .. }
                | Event::Restored { .. }
                | Event::Loaded { .. }
                | Event::Read { .. }
                | Event::Got { .. }
                | Event::Kept { .. }
                | Event::Carried { .. }
                | Event::Stale { .. }
                | Event::Erased { .. }
                | Event::Secret { .. }
                | Event::SecretsLoaded { .. }
                | Event::Corrupt { .. }
                | Event::Patched { .. }) => panic!("decode event {other:?}"),
            }
        }
        let event = match below.transcripts.pop().expect("read or stop") {
            file::Request::OpenRead { owner, .. } => {
                file::Event::Opened { owner, file: Token::new(2), len: u64::try_from(raw.len()).expect("raw") }
            }
            file::Request::ReadAt { owner, offset, max, .. } => {
                let start = usize::try_from(offset).expect("offset");
                let end = start.saturating_add(usize::try_from(max).expect("max")).min(raw.len());
                file::Event::Read { owner, bytes: raw.get(start..end).unwrap_or_default().into() }
            }
            other @ (file::Request::Lock { .. }
            | file::Request::MakeDirectory { .. }
            | file::Request::OpenPrivate { .. }
            | file::Request::Create { .. }
            | file::Request::CreateNoFollow { .. }
            | file::Request::OpenReadNoFollow { .. }
            | file::Request::OpenWrite { .. }
            | file::Request::OpenWriteNoFollow { .. }
            | file::Request::OpenDirectory { .. }
            | file::Request::Load { .. }
            | file::Request::Scan { .. }
            | file::Request::Store { .. }
            | file::Request::Stat { .. }
            | file::Request::WriteAt { .. }
            | file::Request::Cut { .. }
            | file::Request::Sync { .. }
            | file::Request::Close { .. }
            | file::Request::SyncDirectory { .. }
            | file::Request::Rename { .. }
            | file::Request::Remove { .. }
            | file::Request::List { .. }
            | file::Request::ListPage { .. }) => panic!("read operation {other:?}"),
        };
        store::up(component, env, store::Io::Transcripts(store::Arrived { event, fd: None }), &mut above, &mut below);
    }
    panic!("bounded frame did not settle");
}
