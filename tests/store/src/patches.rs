//! The two fixture binaries and their independent record model (store.md 14).
use crate::{World, root};
use jig_protocol_store::{Event, KeptFamily, KeptMap, Keys, Owner, PatchSet, PatchSets, SecretOwner, Secrets, Write};
use jig_test_patches::{self as fixture, patches as app};
use jig_test_patches_jig::{self as upstream, patches as jig};
use skein_lib::Secret;
use std::collections::BTreeMap;
const LABEL: KeptFamily = KeptFamily {
    frames: None,
    name: "fixture-label",
    map: KeptMap::Store,
    texts: app::kept::fixture_label::TEXTS,
    keys: &[Keys { from: fixture::keys::LABELS_FROM, to: fixture::keys::LABELS_TO, root: fixture::keys::LABELS_ROOT }],
};
const SHELF: KeptFamily = KeptFamily {
    frames: None,
    name: "fixture-shelf",
    map: KeptMap::Store,
    texts: app::kept::fixture_shelf::TEXTS,
    keys: &[Keys {
        from: fixture::keys::SHELVES_FROM,
        to: fixture::keys::SHELVES_TO,
        root: fixture::keys::SHELVES_ROOT,
    }],
};
const TOKEN: KeptFamily = KeptFamily {
    frames: None,
    name: "fixture-token",
    texts: app::kept::fixture_token::TEXTS,
    map: KeptMap::Secrets,
    keys: &[Keys { from: fixture::keys::TOKENS_FROM, to: fixture::keys::TOKENS_TO, root: fixture::keys::TOKENS_ROOT }],
};
const NOTE: KeptFamily = KeptFamily {
    frames: None,
    name: "fixture-note",
    map: KeptMap::Store,
    texts: jig::kept::fixture_note::TEXTS,
    keys: &[Keys { from: upstream::keys::NOTES_FROM, to: upstream::keys::NOTES_TO, root: upstream::keys::NOTES_ROOT }],
};
const APPLICATION: PatchSet = PatchSet {
    fresh: None,
    owner: Owner::Application,
    name: fixture::NAME,
    families: &[LABEL, SHELF, TOKEN],
    patches: app::PATCHES,
};
const JIG: PatchSet =
    PatchSet { fresh: None, owner: Owner::Jig, name: upstream::NAME, families: &[NOTE], patches: jig::PATCHES };
pub const SETS: PatchSets = PatchSets { sets: &[APPLICATION, JIG] };
pub const BASE: PatchSets = PatchSets {
    sets: &[
        PatchSet {
            families: &[
                KeptFamily { texts: &[app::kept::fixture_label::TEXTS[0]], ..LABEL },
                KeptFamily { texts: &[app::kept::fixture_shelf::TEXTS[0]], ..SHELF },
                KeptFamily { texts: &[app::kept::fixture_token::TEXTS[0]], ..TOKEN },
            ],
            patches: &[],
            ..APPLICATION
        },
        PatchSet {
            families: &[KeptFamily { texts: &[jig::kept::fixture_note::TEXTS[0]], ..NOTE }],
            patches: &[],
            ..JIG
        },
    ],
};

/// Fixture rows need 64 bytes; 1024 also leaves room for the component's
/// format, layouts and deployment header. A walk's page holds at most
/// `journal.writes` puts: at 32, each family's range is one page.
#[must_use]
pub fn limits() -> jig_protocol_store::Limits {
    let mut limits = root::limits();
    limits.journal.writes = 32;
    for map in [&mut limits.store, &mut limits.secrets] {
        map.key = 64;
        map.value = 1024;
        map.ops = 64;
        map.commit = 4096;
        map.queued_bytes = 8192;
        map.chunk = 4096;
        map.page.bytes = 2048;
    }
    limits.secrets.ops = 2;
    limits
}

/// Two labels, three shelves, two notes and three tokens: two-put pages
/// split the shelves and the tokens.
pub const STORY_LABELS: u32 = 2;
pub const STORY_SHELVES: u32 = 3;
pub const STORY_NOTES: u32 = 2;
pub const STORY_TOKENS: u64 = 3;

#[derive(Clone, Debug)]
pub struct Shelf {
    pub key: Vec<u8>,
    pub name: Vec<u8>,
    pub books: u32,
    pub caption: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct Records {
    pub labels: Vec<(Vec<u8>, Vec<u8>)>,
    pub shelves: Vec<Shelf>,
    pub notes: Vec<(Vec<u8>, Vec<u8>)>,
    pub tokens: Vec<(u64, Vec<u8>)>,
}

fn text(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend(u32::try_from(bytes.len()).expect("fixture text length").to_be_bytes());
    out.extend(bytes);
}
fn label(caption: &[u8], version: u16) -> Vec<u8> {
    let mut out = version.to_be_bytes().to_vec();
    text(&mut out, caption);
    if version == 2 {
        out.push(0);
    }
    out
}

fn token(bytes: &[u8], issued: u64, version: u16) -> Vec<u8> {
    let mut out = version.to_be_bytes().to_vec();
    text(&mut out, bytes);
    out.extend(issued.to_be_bytes());
    if version == 2 {
        out.push(0);
    }
    out
}

impl Records {
    #[must_use]
    pub fn story() -> Self {
        Self {
            labels: (0..STORY_LABELS).map(|n| (format!("l/{n}").into_bytes(), b"caption".to_vec())).collect(),
            shelves: (0..STORY_SHELVES)
                .map(|n| Shelf {
                    key: format!("s/{n}").into_bytes(),
                    name: b"shelf".to_vec(),
                    books: n + 1,
                    caption: b"held".to_vec(),
                })
                .collect(),
            tokens: (0..STORY_TOKENS).map(|n| (n, b"fixture credential".to_vec())).collect(),
            notes: (0..STORY_NOTES).map(|n| (format!("n/{n}").into_bytes(), b"note".to_vec())).collect(),
        }
    }

    /// Independent wire model: no patch function computes expected bytes.
    #[must_use]
    pub fn rows(&self, version: u16) -> BTreeMap<Box<[u8]>, Box<[u8]>> {
        let mut rows = BTreeMap::new();
        for (key, caption) in &self.labels {
            rows.insert(key.clone().into_boxed_slice(), label(caption, version).into_boxed_slice());
        }
        for Shelf { key, name, books, caption } in &self.shelves {
            let mut out = version.to_be_bytes().to_vec();
            text(&mut out, name);
            out.extend(books.to_be_bytes());
            text(&mut out, &label(caption, version));
            if version == 2 {
                out.extend((books * 10 + u32::try_from(key.len()).expect("relative key length")).to_be_bytes());
            }
            rows.insert(key.clone().into_boxed_slice(), out.into_boxed_slice());
        }
        for (key, content) in &self.notes {
            let mut out = version.to_be_bytes().to_vec();
            text(&mut out, content);
            if version == 2 {
                out.push(0);
            }
            rows.insert(key.clone().into_boxed_slice(), out.into_boxed_slice());
        }
        rows
    }

    #[must_use]
    pub fn writes(&self, version: u16) -> Box<[Write]> {
        let mut writes = vec![Write::Save(root::header(1))];
        writes.extend(self.rows(version).into_iter().map(|(key, value)| Write::Put { key, value }));
        writes.into_boxed_slice()
    }

    #[must_use]
    pub fn committed(&self, seed: u64, sets: PatchSets) -> World {
        self.committed_at(seed, sets, 1)
    }

    #[must_use]
    pub fn committed_at(&self, seed: u64, sets: PatchSets, version: u16) -> World {
        self.committed_with(seed, sets, version, limits(), &[])
    }

    /// Base rows and limits for the check's honest refusal scenarios.
    #[must_use]
    pub fn committed_with(
        &self,
        seed: u64,
        sets: PatchSets,
        version: u16,
        limits: jig_protocol_store::Limits,
        rows: &[jig_domain_core::Record],
    ) -> World {
        let mut world = World::unopened_observed(seed, limits, None);
        world.patch_sets(sets);
        assert_eq!(world.open_switched(), Some(0));
        let mut writes = self.writes(version).into_vec();
        writes.extend(rows.iter().cloned().map(Write::Save));
        assert_eq!(world.commit(writes.into_boxed_slice()), Some(1));
        for (number, bytes) in &self.tokens {
            assert!(*number < (1_u64 << 63));
            let events = world.secret(Secrets::Keep {
                owner: SecretOwner::Application,
                number: *number,
                value: Secret::new(token(bytes, *number, version).into_boxed_slice()),
            });
            assert!(matches!(events.as_slice(), [Event::Kept { .. }]), "token kept: {events:?}");
        }
        world.crash(seed);
        world
    }

    /// Decode by each family's kept text as well as compare every modeled byte.
    pub fn check(&self, world: &mut World, version: u16, context: &dyn std::fmt::Debug) {
        for (number, bytes) in &self.tokens {
            let events = world.secret(Secrets::Get { owner: SecretOwner::Application, number: *number });
            let [Event::Secret { value: Some(value), .. }] = events.as_slice() else {
                panic!("token {number}: {events:?}, {context:?}");
            };
            assert_eq!(value.expose(), token(bytes, *number, version), "token {number}, {context:?}");
            let schema =
                skein_codec::schema::parse(app::kept::fixture_token::TEXTS[usize::from(version - 1)].as_bytes())
                    .expect("token schema");
            let root = schema.find(b"Token").expect("token root");
            let mut decoded = skein_codec::value::Value::with_room(16);
            decoded.decode(&schema, root, Box::from(value.expose())).expect("token decodes at its level");
        }
        let rows = world.application(false);
        assert_eq!(rows, self.rows(version), "all records belong to one whole level: {context:?}");
        for (key, bytes) in rows {
            let (texts, root) = match key[0] {
                b'l' => (app::kept::fixture_label::TEXTS, "Label"),
                b's' => (app::kept::fixture_shelf::TEXTS, "Shelf"),
                b'n' => (jig::kept::fixture_note::TEXTS, "Note"),
                _ => panic!("fixture key"),
            };
            let schema = skein_codec::schema::parse(texts[usize::from(version - 1)].as_bytes()).expect("kept schema");
            let root = schema.find(root.as_bytes()).expect("record root");
            let mut value = skein_codec::value::Value::with_room(16);
            value.decode(&schema, root, bytes).expect("record decodes at its level");
        }
    }
}

/// All mutations from the copy's first mkdir through active's final sync.
/// The last cut also covers power loss after the switch's rename.
/// The ordinals name kernel calls, including durable replace's internals.
#[must_use]
pub fn cuts(world: &World) -> Vec<u32> {
    use crate::world::Change;
    let changes = world.changes();
    let first = changes.iter().position(|(_, change)| *change == Change::Make).expect("copy mkdir");
    let last = changes.iter().rposition(|(_, change)| *change == Change::Rename).expect("switch rename");
    assert!(first < last);
    assert_eq!(changes[last + 1].1, Change::Sync, "switch directory sync follows its rename");
    changes[first..=last + 1].iter().map(|(operation, _)| *operation).collect()
}

pub fn whole(world: &mut World) {
    assert_eq!(world.active(), b"2\n");
    let paths = world.paths();
    let generations: Vec<_> = paths
        .iter()
        .filter(|path| path.starts_with(b"generations/") && skein_lib::bytes::count(path, b"/", 2) == 1)
        .map(Vec::as_slice)
        .collect();
    assert_eq!(generations, [b"generations/1".as_slice(), b"generations/2".as_slice()]);
    assert!(paths.iter().any(|path| path == b"generations/2/manifest"));
    assert!(!paths.iter().any(|path| path.windows(7).any(|part| part == b".skein-")));
}

/// The previous generation is an opaque, byte-for-byte rollback image.
#[must_use]
pub fn source(world: &mut World) -> BTreeMap<Vec<u8>, Option<Vec<u8>>> {
    world.contents().into_iter().filter(|(path, _)| path.starts_with(b"generations/1/")).collect()
}

/// A start leaves every file it found as it was; the one write before its
/// checks is skein-kv's recovery, which may open an empty log segment in
/// each map it reads (store.md 10.1, step 4).
pub fn kept(
    before: &BTreeMap<Vec<u8>, Option<Vec<u8>>>,
    after: &BTreeMap<Vec<u8>, Option<Vec<u8>>>,
    what: &impl std::fmt::Debug,
) {
    for (path, bytes) in before {
        assert_eq!(after.get(path), Some(bytes), "{} changed, {what:?}", String::from_utf8_lossy(path));
    }
    for (path, bytes) in after {
        if !before.contains_key(path) {
            let segment = path
                .strip_prefix(b"generations/1/".as_slice())
                .and_then(|map| map.strip_prefix(b"store/log-").or_else(|| map.strip_prefix(b"secrets/log-")));
            assert!(
                segment.is_some_and(|digits| digits.len() == 20 && digits.iter().all(u8::is_ascii_digit))
                    && bytes.as_deref() == Some(&[][..]),
                "{} is new and not an empty recovery segment, {what:?}",
                String::from_utf8_lossy(path)
            );
        }
    }
}

/// The story reads the component's manifest through its public codec.
#[must_use]
pub fn counts(world: &mut World) -> Vec<(Vec<u8>, u64, u64)> {
    let contents = world.contents();
    let bytes = contents
        .get(b"generations/2/manifest".as_slice())
        .expect("patched manifest")
        .as_ref()
        .expect("manifest is a file");
    let manifest = jig_protocol_store::codec::component::Manifest::decode(bytes).expect("manifest codec");
    manifest.counts.iter().map(|count| (count.family().to_vec(), count.before(), count.after())).collect()
}

/// Frame-only sets: only smith moves across the fresh mark.
pub const FRESH_AFTER: PatchSets = PatchSets {
    sets: &[
        PatchSet {
            owner: Owner::Jig,
            name: jig_test_patches_jig::NAME,
            patches: jig::PATCHES,
            fresh: None,
            families: &[KeptFamily {
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
            patches: jig_test_patches_smith::patches::PATCHES,
            fresh: Some(jig_test_patches_smith::FRESH),
            families: &[KeptFamily {
                name: "fixture-turn",
                texts: jig_test_patches_smith::patches::kept::fixture_turn::TEXTS,
                map: KeptMap::Store,
                keys: &[],
                frames: Some(jig_test_patches_smith::ROOT),
            }],
        },
    ],
};
pub const FRESH_BEFORE: PatchSets = PatchSets {
    sets: &[
        FRESH_AFTER.sets[0],
        PatchSet {
            patches: &[],
            families: &[KeptFamily {
                texts: &[jig_test_patches_smith::patches::kept::fixture_turn::TEXTS[0]],
                ..FRESH_AFTER.sets[1].families[0]
            }],
            ..FRESH_AFTER.sets[1]
        },
    ],
};

/// Tight fixture limits: four puts per closing page and small file chunks.
#[must_use]
pub fn fresh_limits() -> jig_protocol_store::Limits {
    let mut limits = limits();
    limits.journal.writes = 4;
    limits
}

/// Six proofs force two crossing pages at the world's four-put journal.
/// Five have windows; every task has two committed fixture turns.
pub const FRESH_WINDOWS: &[bool] = &[true, true, true, true, true, false];
pub const FRESH_SAID: &[u8] = b"said";
pub const FRESH_REPLAY: &[u8] = b"replay";

#[must_use]
pub fn fresh_proof(task: u64, window: bool, crossed: bool) -> jig_domain_core::Record {
    let jig_domain_core::Record::Core(jig_domain_core::CoreRecord::RunProof(mut proof)) = root::run_proof(task) else {
        panic!("proof fixture")
    };
    proof.turn = Some(jig_domain_core::TurnProof { turn: 2, cumulative: 0, read: None });
    proof.window = window.then_some(jig_domain_core::CurrentWindow { attempt: 1, turn: 1, closed: crossed });
    proof.crossed = crossed;
    jig_domain_core::Record::Core(jig_domain_core::CoreRecord::RunProof(proof))
}

/// Independent fixture encoding, shared with the frame-read test seam.
#[must_use]
pub fn fixture_turn(version: u16, said: &[u8], replay: &[u8], tokens: u32) -> Box<[u8]> {
    let mut out = version.to_be_bytes().to_vec();
    text(&mut out, said);
    if version == 1 {
        text(&mut out, replay);
    } else {
        out.extend(tokens.to_be_bytes());
    }
    out.into_boxed_slice()
}

#[must_use]
pub fn fresh_committed(seed: u64, windows: &[bool], crossed: bool) -> World {
    let mut world = World::unopened_observed(seed, fresh_limits(), None);
    world.patch_sets(FRESH_BEFORE);
    world.open_switched().expect("fresh fixture open");
    let bytes = fixture_turn(1, FRESH_SAID, FRESH_REPLAY, 0);
    let mut number = 0;
    for (index, window) in windows.iter().enumerate() {
        let task = root::LIVE + u64::try_from(index).expect("task index");
        number += 1;
        world
            .commit(Box::new([Write::Save(root::header(number)), Write::Save(root::live_task(task))]))
            .expect("live task committed");
        for turn in 1..=2 {
            number += 1;
            world.commit(root::turn_commit(number, task, turn, &bytes)).expect("fixture turn committed");
        }
        // Keep the last turn proof with the designated window and crossing state.
        number += 1;
        world
            .commit(Box::new([Write::Save(root::header(number)), Write::Save(fresh_proof(task, *window, crossed))]))
            .expect("proof committed");
    }
    world.crash(seed);
    world
}
