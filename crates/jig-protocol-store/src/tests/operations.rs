//! Generation operations on the simulated disk (store.md 10.2 and 10.4).
#![expect(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::match_like_matches_macro,
    reason = "disk step tests use ordinary Rust (programming-model.md 10.2)"
)]
use super::fixtures::{
    self,
    disk::{Disk, Seen},
};
use crate::{
    Event, Get, Io, Request, Row, SecretOwner, Secrets, Store, Write,
    generations::{self, Active, Progress},
};
use skein_fake_machine::Item;
use skein_io::{file, kernel::Kind};
use skein_lib::{Secret, Token};

// Small enough to exercise multi-chunk files without making the crash sweep a
// byte-by-byte sweep of the component's format/header encodings.
const CHUNK: u32 = 4096;
const LAST: u64 = 3; // Three commits, the last header and application row.
const ROW_KEY: &[u8] = b"row";
const ROW_VALUE: &[u8] = b"committed";
const SECRET: &[u8] = b"a test secret";
const SECRET_NUMBER: u64 = 1;
fn opened(disk: &mut Disk) -> Store {
    let mut store = disk.store_with_chunk(CHUNK);
    let events = disk.open(&mut store);
    assert!({ if let [Event::Opened { .. }] = events.as_ref() { true } else { false } }, "{events:?}");
    store
}
fn real_generation() -> Disk {
    let mut disk = Disk::new(&[]);
    let mut store = opened(&mut disk);
    for number in 1..=LAST {
        let events = disk.request(
            &mut store,
            fixtures::commit(
                number,
                Box::new([
                    fixtures::header(number),
                    Write::Wires,
                    Write::Put { key: Box::from(ROW_KEY), value: Box::from(ROW_VALUE) },
                ]),
            ),
        );
        assert!({ if let [Event::Committed { .. }] = events.as_ref() { true } else { false } }, "{events:?}");
    }
    let events = disk.request(
        &mut store,
        Request::Secrets {
            owner: Token::new(1),
            request: Secrets::Keep {
                owner: SecretOwner::Application,
                number: SECRET_NUMBER,
                value: Secret::new(Box::from(SECRET)),
            },
        },
    );
    assert!({ if let [Event::Kept { .. }] = events.as_ref() { true } else { false } }, "{events:?}");
    // Leave the component and its open descriptors; captured trees carry only
    // the committed files and are laid into fresh machines for every sweep run.
    disk
}
fn arrived(io: Io) -> crate::Arrived {
    let Io::Transcripts(arrived) = io else { panic!("operation uses transcripts driver") };
    arrived
}
fn copy(disk: &mut Disk, from: u64, chunk: u32) -> Progress {
    let mut below = fixtures::below();
    let mut copying = generations::copy(disk.root(), from, chunk, &mut below);
    let mut progress = copying.progress();
    disk.drive(&mut below, |io, below| progress = generations::copied(&mut copying, arrived(io), below));
    progress
}
fn switch(disk: &mut Disk, active: Active) -> (Progress, Option<Active>) {
    let mut below = fixtures::below();
    let mut switching = generations::switch(disk.root(), active, &mut below);
    let mut progress = switching.progress();
    disk.drive(&mut below, |io, _| progress = generations::switched(&mut switching, arrived(io)));
    (progress, switching.active())
}
fn drop_previous(disk: &mut Disk, active: u64) -> Progress {
    let mut below = fixtures::below();
    let mut dropping = generations::drop_previous(disk.root(), active, &mut below);
    let mut progress = dropping.progress();
    disk.drive(&mut below, |io, below| progress = generations::dropped(&mut dropping, arrived(io), below));
    progress
}
fn active(number: u64) -> Active {
    Active { number, digest: skein_io::digest::digest(&generations::encode(number)) }
}
fn read_rows(disk: &mut Disk, store: &mut Store) {
    let events = disk.request(store, Request::Get { owner: Token::new(1), key: Get::Application(Box::from(ROW_KEY)) });
    assert!(
        {
            if let [Event::Got { row: Some(Row::Application { key, value }), .. }] = events.as_ref() {
                key.as_ref() == ROW_KEY && value.as_ref() == ROW_VALUE
            } else {
                false
            }
        },
        "{events:?}"
    );
    let next = store.stores.store.last().checked_add(1).expect("fixture mark number");
    disk.request(store, fixtures::commit(next, Box::new([fixtures::header(next), Write::Wires])));
    let events = disk.request(
        store,
        Request::Secrets {
            owner: Token::new(1),
            request: Secrets::Get { owner: SecretOwner::Application, number: SECRET_NUMBER },
        },
    );
    assert!(
        {
            if let [Event::Secret { value: Some(value), .. }] = events.as_ref() {
                value.expose() == SECRET
            } else {
                false
            }
        },
        "{events:?}"
    );
}
#[test]
fn a_copy_holds_the_same_files_and_bytes() {
    let mut original = real_generation();
    let mut items = original.items();
    // One beyond a listing page proves that the copy continues with its cursor.
    for number in 0..=generations::PAGE {
        items.push(Item::file(
            &[b"generations/1/store/page-".as_slice(), skein_lib::Decimal::of(u64::from(number)).as_bytes()].concat(),
            b"page",
        ));
    }
    let mut disk = Disk::new(&items);
    let before = disk.contents();
    assert_eq!(copy(&mut disk, 1, CHUNK), Progress::Done);
    let after = disk.contents();
    assert!(
        after
            .iter()
            .any(|(p, kind, mode, _)| p.as_ref() == b"generations/2" && *kind == Kind::Directory && *mode == 0o700)
    );
    assert_eq!(after.iter().filter(|(p, ..)| !p.starts_with(b"generations/2")).cloned().collect::<Vec<_>>(), before);
    // The copy takes the maps; the copy's own manifest is written later (store.md 10.2).
    assert!(before.iter().any(|(p, ..)| p.as_ref() == b"generations/1/manifest"));
    assert!(!after.iter().any(|(p, ..)| p.as_ref() == b"generations/2/manifest"));
    for (path, kind, _, bytes) in
        before.iter().filter(|(p, ..)| p.starts_with(b"generations/1/store") || p.starts_with(b"generations/1/secrets"))
    {
        let target = [b"generations/2/", path.strip_prefix(b"generations/1/").expect("source prefix")].concat();
        let (_, found_kind, mode, found_bytes) =
            after.iter().find(|(p, ..)| p.as_ref() == target).expect("copied entry");
        assert_eq!(found_kind, kind);
        assert_eq!(found_bytes, bytes);
        assert_eq!(*mode, if *kind == Kind::Directory { 0o700 } else { 0o600 });
    }
}
#[test]
fn a_copy_reads_and_writes_in_chunks_of_at_most_its_bound() {
    const BOUND: u32 = 4;
    const CONTENT: &[u8] = b"0123456789"; // Two and a half BOUND-sized chunks.
    let mut original = real_generation();
    let mut items = original.items();
    items.extend([Item::file(b"generations/1/store/chunks", CONTENT), Item::file(b"generations/1/store/empty", b"")]);
    let mut disk = Disk::new(&items);
    assert_eq!(copy(&mut disk, 1, BOUND), Progress::Done);
    assert_eq!(disk.bytes(b"generations/2/store/chunks").as_ref(), CONTENT);
    assert_eq!(disk.bytes(b"generations/2/store/empty").as_ref(), b"");
    let requests = disk.requests();
    let start = requests
        .iter()
        .position(|(_, r)| if let Seen::Create { name } = r { name.as_ref() == b"chunks" } else { false })
        .expect("chunks created");
    let reads: Vec<_> = requests
        .iter()
        .skip(start)
        .take_while(|(_, r)| !{ if let Seen::SyncFile = r { true } else { false } })
        .filter_map(|(_, r)| if let Seen::Read { max } = r { Some(*max) } else { None })
        .collect();
    let writes: Vec<_> = requests
        .iter()
        .skip(start)
        .take_while(|(_, r)| !{ if let Seen::SyncFile = r { true } else { false } })
        .filter_map(|(_, r)| if let Seen::Write { bytes } = r { Some(*bytes) } else { None })
        .collect();
    assert_eq!(reads, [BOUND, BOUND, BOUND / 2]);
    assert_eq!(writes, reads);
    assert!(requests.iter().all(|(_, r)| match r {
        Seen::Read { max } => *max <= BOUND,
        Seen::Write { bytes } => *bytes <= BOUND,
        Seen::Create { .. }
        | Seen::SyncFile
        | Seen::Rename { .. }
        | Seen::Lock
        | Seen::Load
        | Seen::Make(_)
        | Seen::Private(_)
        | Seen::Open
        | Seen::Store { .. }
        | Seen::Remove { .. }
        | Seen::Sync
        | Seen::Close
        | Seen::Other => true,
    }));
}
#[test]
fn a_copy_onto_a_standing_generation_fails() {
    let mut original = real_generation();
    let mut items = original.items();
    items.extend([Item::directory(b"generations/2"), Item::file(b"generations/2/kept", b"standing")]);
    let mut disk = Disk::new(&items);
    let before = disk.contents();
    assert_eq!(copy(&mut disk, 1, CHUNK), Progress::Failed);
    assert_eq!(disk.contents(), before);
}
#[test]
fn a_switch_names_the_next_generation_expecting_what_open_read() {
    let mut original = real_generation();
    let mut disk = Disk::new(&original.items());
    let store = opened(&mut disk);
    let active = store.directory.active.expect("component kept active");
    assert_eq!(copy(&mut disk, active.number, CHUNK), Progress::Done);
    let start = disk.requests().len();
    assert_eq!(switch(&mut disk, active), (Progress::Done, Some(self::active(2))));
    assert!({
        if let Some((2, Seen::Store { path, bytes, expected: file::Expect::Digest(digest) })) =
            disk.requests().get(start)
        {
            path.as_ref() == b"active" && bytes.as_ref() == b"2\n" && *digest == active.digest
        } else {
            false
        }
    });
    assert_eq!(disk.bytes(b"active").as_ref(), b"2\n");
    disk.crash(1);
    let mut store = opened(&mut disk);
    assert_eq!(store.directory.active.expect("switched active").number, 2);
    read_rows(&mut disk, &mut store);
    assert!(disk.tree().iter().any(|(p, ..)| p.as_ref() == b"generations/1/store"));
}
#[test]
fn a_switch_after_active_changed_conflicts_and_changes_nothing() {
    let mut original = real_generation();
    let mut disk = Disk::new(&original.items());
    let store = opened(&mut disk);
    let active = store.directory.active.expect("component kept active");
    let root = disk.root();
    let rewritten = disk.file(
        2,
        file::Request::Store {
            owner: Token::new(1),
            root: root.token,
            path: Box::from(b"active".as_slice()),
            bytes: Box::from(b"9\n".as_slice()),
            expected: file::Expect::Any,
            no_follow: true,
        },
    );
    assert!({ if let file::Event::Stored { .. } = rewritten.event { true } else { false } });
    let before = disk.contents();
    assert_eq!(switch(&mut disk, active), (Progress::Failed, None));
    assert_eq!(disk.contents(), before);
}
fn generations_two_and_three() -> Vec<Item> {
    let mut disk = real_generation();
    assert_eq!(copy(&mut disk, 1, CHUNK), Progress::Done);
    assert_eq!(switch(&mut disk, active(1)).0, Progress::Done);
    assert_eq!(drop_previous(&mut disk, 2), Progress::Done);
    assert_eq!(copy(&mut disk, 2, CHUNK), Progress::Done);
    assert_eq!(switch(&mut disk, active(2)).0, Progress::Done);
    disk.items()
}
#[test]
fn a_drop_renames_the_previous_aside_then_removes_it() {
    let mut disk = Disk::new(&generations_two_and_three());
    assert_eq!(drop_previous(&mut disk, 3), Progress::Done);
    assert!(
        disk.tree()
            .iter()
            .filter(|(p, ..)| p.starts_with(b"generations/"))
            .all(|(p, ..)| p.starts_with(b"generations/3"))
    );
    let requests = disk.requests();
    let rename = requests
        .iter()
        .position(|(_, r)| {
            if let Seen::Rename { from, to } = r { from.as_ref() == b"2" && to.as_ref() == b"dropped" } else { false }
        })
        .expect("renamed aside");
    let sync =
        requests.iter().position(|(_, r)| if let Seen::Sync = r { true } else { false }).expect("generations synced");
    let remove = requests
        .iter()
        .position(|(_, r)| if let Seen::Remove { .. } = r { true } else { false })
        .expect("previous removed");
    assert!(rename < sync && sync < remove);
}
#[test]
fn a_drop_with_no_previous_is_done() {
    let mut disk = real_generation();
    let start = disk.requests().len();
    assert_eq!(drop_previous(&mut disk, 1), Progress::Done);
    assert_eq!(disk.requests().len(), start);
}
fn sequence(disk: &mut Disk, active: Active) {
    if drop_previous(disk, active.number) != Progress::Done {
        return;
    }
    if copy(disk, active.number, CHUNK) != Progress::Done {
        return;
    }
    switch(disk, active);
}
fn changes_disk(seen: &Seen) -> bool {
    match seen {
        Seen::Make(_)
        | Seen::Private(_)
        | Seen::Create { .. }
        | Seen::Write { .. }
        | Seen::SyncFile
        | Seen::Sync
        | Seen::Rename { .. }
        | Seen::Remove { .. }
        | Seen::Store { .. } => true,
        Seen::Read { .. } | Seen::Lock | Seen::Load | Seen::Open | Seen::Close | Seen::Other => false,
    }
}
#[test]
fn a_drop_copy_and_switch_cut_anywhere_leave_one_whole_generation() {
    let mut original = real_generation();
    assert_eq!(copy(&mut original, 1, CHUNK), Progress::Done);
    assert_eq!(switch(&mut original, active(1)).0, Progress::Done);
    let contents = original.contents();
    let items = original.items();
    let mut completed = Disk::new(&items);
    let store = opened(&mut completed);
    let planned = store.directory.active.expect("component kept active");
    let start = completed.requests().len();
    sequence(&mut completed, planned);
    assert_eq!(completed.bytes(b"active").as_ref(), b"3\n");
    let trace = &completed.requests().get(start..).expect("sequence trace");
    let n = trace.len();
    let switch_at = trace
        .iter()
        .position(|(_, r)| if let Seen::Store { path, .. } = r { path.as_ref() == b"active" } else { false })
        .expect("switch Store");
    let mut cuts: Vec<_> =
        trace.iter().enumerate().filter_map(|(index, (_, r))| changes_disk(r).then_some(index)).collect();
    cuts.push(n);
    for cut in cuts {
        let mut disk = Disk::new(&items);
        let store = opened(&mut disk);
        let planned = store.directory.active.expect("component kept active");
        disk.cut_at(disk.operations() + u32::try_from(cut).expect("cut fits"));
        sequence(&mut disk, planned);
        disk.crash(u64::try_from(cut).expect("cut seed fits"));
        let mut store = opened(&mut disk);
        let number = store.directory.active.expect("reopened active").number;
        assert!(number == 2 || number == 3, "cut {cut}: active {number}");
        if cut <= switch_at {
            assert_eq!(number, 2, "cut before switch {cut}");
        }
        read_rows(&mut disk, &mut store);
        let tree = disk.tree();
        let allowed = [generations::generation(number), generations::generation(number - 1)];
        assert!(
            tree.iter().filter(|(p, ..)| p.starts_with(b"generations/")).all(|(p, ..)| {
                allowed
                    .iter()
                    .any(|prefix| p.as_ref() == prefix.as_ref() || p.starts_with(&[prefix.as_ref(), b"/"].concat()))
            }),
            "cut {cut}: {tree:?}"
        );
        // A previous that survived the rename must still be whole, never a
        // partially deleted numbered generation. Recovery may add log segments
        // to the active; compare the immutable previous's original files.
        let previous = number - 1;
        let prefix = [generations::generation(previous).as_ref(), b"/"].concat();
        if tree.iter().any(|(p, ..)| p.starts_with(prefix.as_ref())) {
            for (path, kind, _, bytes) in contents.iter().filter(|(p, ..)| p.starts_with(prefix.as_ref())) {
                if *kind == Kind::File {
                    assert_eq!(&disk.bytes(path), bytes, "cut {cut}");
                }
            }
        }
    }
}
