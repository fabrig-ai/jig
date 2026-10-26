//! Generation startup, refusals, and independent file routes (store.md 2 and 10.1).
#![expect(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::disallowed_macros,
    reason = "disk step tests use ordinary Rust (skein's programming-model.md, 10.2)"
)]
use super::fixtures::{
    self,
    disk::{Disk, Seen},
};
use crate::{Arrived, Below, Event, Generations, Io, Older, OpenRefusal, Phase, Request, Store};
use alloc::boxed::Box;
use skein_fake_machine::Item;
use skein_io::{
    file,
    kernel::{Error, Fd, Kind},
};
use skein_lib::{Queue, Token};

fn scripted(store: &mut Store, below: &mut Below) -> Box<[Event]> {
    let (driver, request): (u8, file::Request) = below
        .store
        .pop()
        .map(|r| (0, r))
        .or_else(|| below.secrets.pop().map(|r| (1, r)))
        .or_else(|| below.transcripts.pop().map(|r| (2, r)))
        .expect("startup request");
    let owner = crate::directory::OWNER;
    let event = match request {
        file::Request::Lock { owner, .. } => file::Event::Locked { owner },
        file::Request::Load { owner, .. } => {
            file::Event::Failed { owner, error: Error::NotFound, committed: false, residue: None }
        }
        file::Request::MakeDirectory { owner, .. } => file::Event::Made { owner },
        file::Request::OpenDirectory { owner, .. }
        | file::Request::OpenPrivate { owner, .. }
        | file::Request::Create { owner, .. } => file::Event::Opened { owner, file: Token::new(100), len: 0 },
        file::Request::ListPage { owner, .. } => {
            let entries: Box<[file::Entry]> =
                if let crate::directory::Opening::CheckingPage { maps: 0, .. } = store.directory.opening {
                    Box::new([
                        file::Entry { name: Box::from(b"store".as_slice()), kind: Kind::Directory },
                        file::Entry { name: Box::from(b"secrets".as_slice()), kind: Kind::Directory },
                    ])
                } else {
                    Box::new([])
                };
            file::Event::Listed { owner, entries }
        }
        file::Request::List { owner, .. } => file::Event::Listed { owner, entries: Box::new([]) },
        file::Request::Close { owner, .. } => file::Event::Closed { owner },
        file::Request::Store { owner, bytes, .. } => {
            file::Event::Stored { owner, digest: skein_io::digest::digest(&bytes) }
        }
        file::Request::SyncDirectory { owner, .. } => file::Event::Synced { owner },
        file::Request::CreateNoFollow { .. }
        | file::Request::OpenRead { .. }
        | file::Request::OpenReadNoFollow { .. }
        | file::Request::OpenWrite { .. }
        | file::Request::OpenWriteNoFollow { .. }
        | file::Request::Scan { .. }
        | file::Request::Stat { .. }
        | file::Request::WriteAt { .. }
        | file::Request::ReadAt { .. }
        | file::Request::Cut { .. }
        | file::Request::Sync { .. }
        | file::Request::Rename { .. }
        | file::Request::Remove { .. } => panic!("unexpected startup request for {owner:?}"),
    };
    let arrived = Arrived { event, fd: Some(Fd::new(4)) };
    let io = match driver {
        0 => Io::Store(arrived),
        1 => Io::Secrets(arrived),
        _ => Io::Transcripts(arrived),
    };
    let (events, mut next) = fixtures::answer(store, io);
    while let Some(r) = next.store.pop() {
        below.store.push(r);
    }
    while let Some(r) = next.secrets.pop() {
        below.secrets.push(r);
    }
    while let Some(r) = next.transcripts.pop() {
        below.transcripts.push(r);
    }
    events
}
fn at_private() -> (Store, Below) {
    let (mut store, mut below) = fixtures::directory();
    for _ in 0_u32..100 {
        if let crate::directory::Opening::FirstSecrets = store.directory.opening {
            return (store, below);
        }
        assert!(scripted(&mut store, &mut below).is_empty());
    }
    panic!("private open reached");
}
fn disk() -> Store {
    let (mut store, mut below) = at_private();
    for _ in 0_u32..100 {
        let events = scripted(&mut store, &mut below);
        if !events.is_empty() {
            let [Event::Opened { last: 0, .. }] = &*events else { panic!("empty store opened: {events:?}") };
            return store;
        }
    }
    panic!("disk opened");
}
#[test]
fn the_secrets_directory_is_opened_as_private() {
    let (_, mut below) = at_private();
    let Some(file::Request::OpenPrivate { root, path, .. }) = below.secrets.pop() else {
        panic!("private secrets");
    };
    assert_eq!((root, path.as_ref()), (fixtures::ROOT.token, b"generations/1/secrets".as_slice()));
}
#[test]
fn skein_requests_go_below_on_their_stores_driver() {
    let (mut store, mut below) = at_private();
    for _ in 0_u32..100 {
        if let crate::directory::Opening::Stores = store.directory.opening {
            let Some(file::Request::List { owner, .. }) = below.secrets.pop() else { panic!("secrets recovery list") };
            assert_ne!(owner, crate::directory::OWNER);
            assert!(below.transcripts.is_empty());
            return;
        }
        scripted(&mut store, &mut below);
    }
    panic!("kv opens reached");
}
#[test]
fn a_directory_failure_refuses_open() {
    let (mut store, mut below) = fixtures::directory();
    let Some(file::Request::Lock { owner, .. }) = below.store.pop() else { panic!("lock first") };
    let arrived = Arrived {
        event: file::Event::Failed { owner, error: Error::Permission, committed: false, residue: None },
        fd: None,
    };
    let (events, _) = fixtures::answer(&mut store, Io::Store(arrived));
    assert_refused(&events, OpenRefusal::Failed);
    super::words::has_deployment_slots(&events, super::words::FAILED);
}
#[test]
fn a_failed_skein_commit_on_disk_answers_failed() {
    let mut store = disk();
    let mut below = submit(&mut store, 1);
    let Some(file::Request::WriteAt { owner, .. }) = below.store.pop() else {
        panic!("disk write");
    };
    let arrived = Arrived {
        event: file::Event::Failed { owner, error: Error::Permission, committed: false, residue: None },
        fd: None,
    };
    let (events, _) = fixtures::answer(&mut store, Io::Store(arrived));
    let [Event::Failed { number: 1 }] = &*events else {
        panic!("failed root commit");
    };
    assert_eq!(store.phase, Phase::Stopped);
}
#[test]
fn a_recovery_beneath_the_store_stops_it() {
    let mut store = fixtures::opened();
    let mut kv = Queue::with_capacity(skein_kv::MAX_OUT.above);
    kv.push(skein_kv::Event::Recovered { last: 0 });
    let mut above = Queue::with_capacity(crate::MAX_OUT_DOWN);
    crate::directory::events_up(&mut store, false, &mut kv, &mut above, &mut fixtures::below());
    assert_eq!(store.phase, Phase::Stopped);
}
fn submit(store: &mut Store, number: u64) -> Below {
    let mut above = Queue::with_capacity(crate::MAX_OUT_DOWN);
    let mut below = fixtures::below();
    crate::down(
        store,
        &fixtures::env(),
        fixtures::commit(number, Box::new([fixtures::header(number)])),
        &mut above,
        &mut below,
    );
    assert!(above.is_empty());
    below
}
#[test]
fn submitted_commits_advance_before_the_durable_number() {
    let mut store = disk();
    let mut below = submit(&mut store, 1);
    assert_eq!(store.stores.store.last(), 0);
    let Some(file::Request::WriteAt { owner, .. }) = below.store.pop() else {
        panic!("write the first commit");
    };
    let queued = submit(&mut store, 2);
    assert!(queued.store.is_empty());
    assert_eq!(store.phase, Phase::Ready { next: 3, format_due: false });
    let (_, mut below) =
        fixtures::answer(&mut store, Io::Store(Arrived { event: file::Event::Written { owner }, fd: None }));
    let Some(file::Request::Sync { .. }) = below.store.pop() else {
        panic!("sync root commit");
    };
    let (events, _) =
        fixtures::answer(&mut store, Io::Store(Arrived { event: file::Event::Synced { owner }, fd: None }));
    let [Event::Committed { number: 1, .. }] = &*events else {
        panic!("first commit durable");
    };
    assert_eq!(store.stores.store.last(), 1);
    assert_eq!(store.phase, Phase::Ready { next: 3, format_due: false });
}
#[test]
fn a_disk_secret_waits_for_its_own_sync() {
    let mut store = disk();
    // This IO transition starts in the post-mark serving state.
    store.mark = crate::Mark::Durable;
    let mut above = Queue::with_capacity(crate::MAX_OUT_DOWN);
    let mut below = fixtures::below();
    crate::down(
        &mut store,
        &fixtures::env(),
        Request::Secrets { owner: Token::new(9), request: fixtures::sign_in_secret(4, 41) },
        &mut above,
        &mut below,
    );
    assert!(above.is_empty());
    let Some(file::Request::WriteAt { owner, .. }) = below.secrets.pop() else {
        panic!("write the secret");
    };
    let (events, mut below) =
        fixtures::answer(&mut store, Io::Secrets(Arrived { event: file::Event::Written { owner }, fd: None }));
    assert!(events.is_empty());
    let Some(file::Request::Sync { .. }) = below.secrets.pop() else {
        panic!("sync secret");
    };
    let (events, _) =
        fixtures::answer(&mut store, Io::Secrets(Arrived { event: file::Event::Synced { owner }, fd: None }));
    let [Event::Kept { owner }] = &*events else {
        panic!("secret durable");
    };
    assert_eq!(*owner, Token::new(9));
    assert_eq!(store.stores.secrets.last(), 1);
    assert_eq!(store.stores.store.last(), 0);
}

fn assert_opened(events: &[Event]) {
    let [Event::Opened { .. }] = events else { panic!("opened: {events:?}") };
}
fn assert_refused(events: &[Event], expected: OpenRefusal) {
    let [Event::OpenRefused { why, .. }] = events else { panic!("refused: {events:?}") };
    assert_eq!(*why, expected);
}
fn standing(number: u64) -> Vec<Item> {
    vec![
        Item::file(b"active", format!("{number}\n").as_bytes()),
        Item::directory(b"generations"),
        Item::directory(format!("generations/{number}").as_bytes()),
        Item::directory(format!("generations/{number}/store").as_bytes()),
        Item::directory(format!("generations/{number}/secrets").as_bytes()).mode(0o700),
    ]
}
fn paths(disk: &mut Disk) -> Vec<Box<[u8]>> {
    disk.tree().into_iter().map(|(p, _, _)| p).collect()
}
fn has(paths: &[Box<[u8]>], path: &[u8]) -> bool {
    paths.iter().any(|p| p.as_ref() == path)
}
fn unchanged_refusal(items: &[Item], expected: OpenRefusal) {
    let mut disk = Disk::new(items);
    let before = disk.contents();
    let mut store = disk.store();
    assert_refused(&disk.open(&mut store), expected);
    assert_eq!(disk.contents(), before);
}
#[test]
fn a_first_start_makes_generation_one_then_active_then_transcripts() {
    let mut disk = Disk::new(&[]);
    let mut store = disk.store();
    assert_opened(&disk.open(&mut store));
    assert_eq!(disk.bytes(b"active").as_ref(), b"1\n");
    assert_eq!(crate::codec::component::Manifest::decode(&disk.bytes(b"generations/1/manifest")), Ok(first_manifest()));
    let tree = disk.tree();
    assert!(
        tree.iter().any(|(p, k, m)| p.as_ref() == b"generations/1/secrets" && *k == Kind::Directory && *m == 0o700)
    );
    assert!(has(&paths(&mut disk), b"generations/1/store/log-00000000000000000001"));
    let requests = disk.requests();
    let active = requests
        .iter()
        .position(|(_, r)| {
            if let Seen::Store { path, bytes, expected: file::Expect::Absent } = r {
                path.as_ref() == b"active" && bytes.as_ref() == b"1\n"
            } else {
                false
            }
        })
        .expect("active stored absent");
    let made = requests
        .iter()
        .position(|(_, r)| if let Seen::Make(p) = r { p.as_ref() == b"generations/1/store" } else { false })
        .expect("store made");
    let private = requests
        .iter()
        .position(|(_, r)| if let Seen::Private(p) = r { p.as_ref() == b"generations/1/secrets" } else { false })
        .expect("secrets made");
    let manifest = requests
        .iter()
        .position(|(driver, r)| {
            if let Seen::Store { path, bytes, expected: file::Expect::Absent } = r {
                *driver == 2
                    && path.as_ref() == b"generations/1/manifest"
                    && crate::codec::component::Manifest::decode(bytes) == Ok(first_manifest())
            } else {
                false
            }
        })
        .expect("manifest stored absent on the transcripts driver");
    let transcripts = requests
        .iter()
        .position(|(_, r)| if let Seen::Make(p) = r { p.as_ref() == b"transcripts" } else { false })
        .expect("transcripts made");
    assert!(made < private && private < manifest && manifest < active && active < transcripts);
    let Some((0, Seen::Lock)) = requests.first() else { panic!("store driver locks first") };
    assert_eq!(
        store.directory.active,
        Some(crate::generations::Active { number: 1, digest: skein_io::digest::digest(b"1\n") })
    );
}
#[test]
fn a_failed_manifest_refuses_before_active_is_stored() {
    let (mut store, mut below) = at_private();
    scripted(&mut store, &mut below);
    let Some(file::Request::Store { owner, path, .. }) = below.transcripts.pop() else {
        panic!("manifest on the transcripts driver");
    };
    assert_eq!(path.as_ref(), b"generations/1/manifest");
    assert!(below.store.is_empty());
    let arrived = Arrived {
        event: file::Event::Failed { owner, error: Error::Permission, committed: false, residue: None },
        fd: None,
    };
    let (events, below) = fixtures::answer(&mut store, Io::Transcripts(arrived));
    assert_refused(&events, OpenRefusal::Failed);
    assert!(below.store.is_empty());
}
#[test]
fn a_later_start_opens_the_generation_active_names() {
    let mut disk = Disk::new(&standing(7));
    let mut store = disk.store();
    assert_opened(&disk.open(&mut store));
    assert!(has(&paths(&mut disk), b"generations/7/store/log-00000000000000000001"));
    let makes: Vec<_> = disk
        .requests()
        .iter()
        .filter_map(|(_, r)| if let Seen::Make(p) = r { Some(p.as_ref()) } else { None })
        .collect();
    assert_eq!(makes, [b"transcripts".as_slice()]);
    assert_eq!(store.directory.active.expect("read active").number, 7);
}
#[test]
fn a_second_open_of_a_locked_deployment_is_refused_busy() {
    let mut disk = Disk::new(&[]);
    let mut first = disk.store();
    assert_opened(&disk.open(&mut first));
    let before = disk.contents();
    let root = disk.second_root();
    let mut second = disk.store();
    let events =
        disk.request(&mut second, Request::Open { owner: Token::new(2), root, installation: fixtures::INSTALLATION });
    assert_refused(&events, OpenRefusal::Generations { why: Generations::Busy });
    assert_eq!(disk.contents(), before);
}
#[test]
fn active_that_is_not_a_number_and_a_newline_is_unreadable() {
    for bytes in [
        b"".as_slice(),
        b"0\n",
        b"07\n",
        b"7",
        b"7\n\n",
        b" 7\n",
        b"x\n",
        b"18446744073709551616\n",
        b"1234567890123456789012",
    ] {
        unchanged_refusal(&[Item::file(b"active", bytes)], OpenRefusal::Generations { why: Generations::Unreadable });
    }
}
#[test]
fn a_missing_map_is_refused_naming_its_generation() {
    for maps in [
        vec![],
        vec![Item::directory(b"generations/3"), Item::directory(b"generations/3/store")],
        vec![Item::directory(b"generations/3"), Item::directory(b"generations/3/secrets").mode(0o700)],
        vec![
            Item::directory(b"generations/3"),
            Item::file(b"generations/3/store", b"wrong kind"),
            Item::directory(b"generations/3/secrets").mode(0o700),
        ],
    ] {
        let mut items = vec![Item::file(b"active", b"3\n"), Item::directory(b"generations")];
        items.extend(maps);
        unchanged_refusal(&items, OpenRefusal::Generations { why: Generations::Missing { generation: 3 } });
    }
}
#[test]
fn todays_layout_is_refused_before_anything_is_written() {
    for items in [
        vec![Item::directory(b"store"), Item::file(b"store/row", b"old row")],
        vec![Item::directory(b"secrets")],
        vec![Item::directory(b"transcripts")],
        vec![Item::directory(b"store"), Item::directory(b"secrets"), Item::directory(b"transcripts")],
    ] {
        unchanged_refusal(&items, OpenRefusal::OlderBuild { why: Older::Directory });
    }
}
#[test]
fn generations_no_active_names_are_refused_and_kept() {
    for rest in [
        vec![Item::directory(b"generations/2")],
        vec![
            Item::directory(b"generations/1"),
            Item::directory(b"generations/1/store"),
            Item::file(b"generations/1/store/row", b"row"),
        ],
        vec![Item::directory(b"generations/1"), Item::file(b"generations/1/other", b"row")],
    ] {
        let mut items = vec![Item::directory(b"generations")];
        items.extend(rest);
        items.push(Item::file(b"active.skein-0123456789abcdef", b"temporary kept on refusal"));
        unchanged_refusal(&items, OpenRefusal::Generations { why: Generations::Unnamed });
    }
}
#[test]
fn a_cut_first_starts_leftover_is_removed_and_the_start_made_again() {
    // Each durable prefix before active, including a kill that keeps every write.
    for maps in [
        vec![],
        vec![Item::directory(b"generations")],
        vec![Item::directory(b"generations"), Item::directory(b"generations/1")],
        vec![
            Item::directory(b"generations"),
            Item::directory(b"generations/1"),
            Item::directory(b"generations/1/store"),
        ],
        vec![
            Item::directory(b"generations"),
            Item::directory(b"generations/1"),
            Item::directory(b"generations/1/secrets").mode(0o700),
        ],
        standing(1).into_iter().skip(1).collect(),
    ] {
        let mut items = maps;
        items.push(Item::file(b"active.skein-0123456789abcdef", b"1\n"));
        let mut disk = Disk::new(&items);
        let mut store = disk.store();
        assert_opened(&disk.open(&mut store));
        assert!(!has(&paths(&mut disk), b"active.skein-0123456789abcdef"));
        assert_eq!(disk.bytes(b"active").as_ref(), b"1\n");
        assert!(has(&paths(&mut disk), b"generations/1/store"));
        assert!(has(&paths(&mut disk), b"generations/1/secrets"));
    }
}

#[test]
fn a_leftover_manifest_or_its_temporary_is_removed_before_starting_afresh() {
    for name in [b"manifest".as_slice(), b"manifest.skein-0123456789abcdef"] {
        let mut items: Vec<_> = standing(1).into_iter().skip(1).collect();
        let path = [b"generations/1/", name].concat();
        items.push(Item::file(&path, b"partial manifest"));
        let mut disk = Disk::new(&items);
        let mut store = disk.store();
        assert_opened(&disk.open(&mut store));
        assert_eq!(
            crate::codec::component::Manifest::decode(&disk.bytes(b"generations/1/manifest")),
            Ok(first_manifest())
        );
        assert_eq!(disk.bytes(b"active").as_ref(), b"1\n");
        assert!(!has(&paths(&mut disk), b"generations/1/manifest.skein-0123456789abcdef"));
        let requests = disk.requests();
        let removed = requests
            .iter()
            .position(
                |(_, r)| if let Seen::Remove { name, directory: true } = r { name.as_ref() == b"1" } else { false },
            )
            .expect("leftover generation removed whole");
        let made = requests
            .iter()
            .position(|(_, r)| if let Seen::Make(path) = r { path.as_ref() == b"generations/1" } else { false })
            .expect("generation made afresh");
        assert!(removed < made);
    }
}
#[test]
fn unrelated_or_multiple_manifest_entries_refuse_unnamed_unchanged() {
    for extra in [
        vec![Item::file(b"generations/1/other", b"keep")],
        vec![Item::directory(b"generations/1/manifest")],
        vec![Item::directory(b"generations/1/manifest.skein-0123456789abcdef")],
        vec![Item::file(b"generations/1/manifest.skein-0123456789abcde", b"keep")],
        vec![Item::file(b"generations/1/manifest.skein-0123456789abcdef0", b"keep")],
        vec![Item::file(b"generations/1/manifest.skein-0123456789abcdeF", b"keep")],
        vec![Item::file(b"generations/1/manifest.skein-0123456789abcdeg", b"keep")],
        vec![
            Item::file(b"generations/1/manifest", b"keep"),
            Item::file(b"generations/1/manifest.skein-0123456789abcdef", b"keep"),
        ],
        vec![
            Item::file(b"generations/1/manifest.skein-0123456789abcdef", b"keep"),
            Item::file(b"generations/1/manifest.skein-fedcba9876543210", b"keep"),
        ],
        vec![Item::file(b"generations/1/manifest", b"keep"), Item::file(b"generations/1/store/row", b"keep")],
        vec![Item::file(b"generations/1/manifest", b"keep"), Item::file(b"generations/1/secrets/row", b"keep")],
    ] {
        let mut items: Vec<_> = standing(1).into_iter().skip(1).collect();
        items.extend(extra);
        unchanged_refusal(&items, OpenRefusal::Generations { why: Generations::Unnamed });
    }
}

fn strays() -> Vec<Item> {
    let mut items = Vec::new();
    for name in ["3", "4", "6", "0", "dropped"] {
        items.push(Item::directory(format!("generations/{name}").as_bytes()));
        items.push(Item::directory(format!("generations/{name}/store").as_bytes()));
        items.push(Item::file(format!("generations/{name}/store/row").as_bytes(), b"kept bytes"));
        items.push(Item::directory(format!("generations/{name}/secrets").as_bytes()).mode(0o700));
    }
    // More than a 32-entry page in a map; removal must reopen and list again.
    for n in 0..=crate::generations::PAGE {
        items.push(Item::file(format!("generations/3/store/row-{n:02}").as_bytes(), b"stray"));
    }
    items.extend([
        Item::file(b"generations/junk", b"stray file"),
        Item::file(b"active.skein-0123456789abcdef", b"temporary"),
        Item::file(b"active.skein-fedcba9876543210", b"temporary"),
    ]);
    items
}
#[test]
fn strays_are_removed_once_the_checks_pass() {
    let mut items = standing(5);
    items.extend(strays());
    let mut disk = Disk::new(&items);
    let mut store = disk.store();
    assert_opened(&disk.open(&mut store));
    let paths = paths(&mut disk);
    for name in [
        b"generations/3".as_slice(),
        b"generations/6",
        b"generations/0",
        b"generations/dropped",
        b"generations/junk",
        b"active.skein-0123456789abcdef",
        b"active.skein-fedcba9876543210",
    ] {
        assert!(!has(&paths, name), "removed {name:?}");
    }
    assert!(has(&paths, b"generations/4/store/row"));
    assert_eq!(disk.bytes(b"generations/4/store/row").as_ref(), b"kept bytes");
    assert!(has(&paths, b"generations/5/store/log-00000000000000000001"));
    let requests = disk.requests();
    let temporary = requests
        .iter()
        .rposition(
            |(_, r)| {
                if let Seen::Remove { name, .. } = r { crate::generations::is_temporary(name) } else { false }
            },
        )
        .expect("top temporaries removed");
    let sync = requests.iter().rposition(|(driver, r)| *driver == 2 && (*r == Seen::Sync)).expect("generations synced");
    let close =
        requests.iter().rposition(|(driver, r)| *driver == 2 && (*r == Seen::Close)).expect("generations closed");
    let transcripts = requests
        .iter()
        .position(|(_, r)| if let Seen::Make(name) = r { name.as_ref() == b"transcripts" } else { false })
        .expect("transcripts made");
    assert!(temporary < sync && sync < close && close < transcripts);
}
#[test]
fn strays_stay_when_a_check_refuses() {
    let mut disk = Disk::new(&standing(5));
    let mut store = disk.store();
    assert_opened(&disk.open(&mut store));
    let events = disk.request(&mut store, fixtures::commit(1, Box::new([fixtures::header(1)])));
    let [Event::Committed { number: 1, .. }] = &*events else { panic!("committed: {events:?}") };
    disk.crash(4);
    // Restore strays on a new laid disk from the complete durable contents.
    let laid: Vec<_> = disk
        .contents()
        .into_iter()
        .map(|(p, k, m, b)| if k == Kind::Directory { Item::directory(&p).mode(m) } else { Item::file(&p, &b).mode(m) })
        .collect();
    let mut laid = laid;
    laid.extend(strays());
    let mut disk = Disk::new(&laid);
    let before = disk.contents();
    let mut store = disk.store();
    store.application.version = store.application.version.checked_add(1).expect("test version");
    let events = disk.open(&mut store);
    let [Event::OpenRefused { why: OpenRefusal::Format { .. }, .. }] = &*events else {
        panic!("format refused: {events:?}")
    };
    let after = disk.contents();
    for entry in before.iter().filter(|(p, _, _, _)| !p.starts_with(b"generations/5/")) {
        assert!(after.contains(entry), "stray untouched: {entry:?}");
    }
}
#[test]
fn transcripts_are_made_where_missing() {
    let mut disk = Disk::new(&standing(3));
    let mut store = disk.store();
    assert_opened(&disk.open(&mut store));
    assert!(has(&paths(&mut disk), b"transcripts"));
}
#[test]
fn the_first_start_cut_at_each_operation_starts_again_whole() {
    let mut completed = Disk::new(&[]);
    let mut store = completed.store();
    assert_opened(&completed.open(&mut store));
    let operations = completed.operations();
    for cut in 0..=operations {
        let mut disk = Disk::new(&[]);
        let mut store = disk.store();
        disk.cut_at(cut);
        disk.open(&mut store);
        disk.crash(u64::from(cut));
        let mut store = disk.store();
        assert_opened(&disk.open(&mut store));
        assert_eq!(disk.bytes(b"active").as_ref(), b"1\n", "cut {cut}");
        assert_eq!(
            crate::codec::component::Manifest::decode(&disk.bytes(b"generations/1/manifest")),
            Ok(first_manifest()),
            "cut {cut}"
        );
        assert_eq!(
            paths(&mut disk)
                .iter()
                .filter(|p| p.starts_with(b"generations/") && skein_lib::bytes::count(p, b"/", 2) == 1)
                .map(AsRef::as_ref)
                .collect::<Vec<_>>(),
            [b"generations/1".as_slice()]
        );
        assert!(!paths(&mut disk).iter().any(|p| crate::generations::is_temporary(p)));
        let events = disk.request(&mut store, fixtures::commit(1, Box::new([fixtures::header(1)])));
        let [Event::Committed { number: 1, .. }] = &*events else { panic!("committed, cut {cut}: {events:?}") };
        let events = disk.request(
            &mut store,
            Request::Get {
                owner: Token::new(2),
                key: crate::Get::Jig(jig_domain_core::Key::Core(jig_domain_core::CoreKey::Deployment)),
            },
        );
        let [Event::Got { row: Some(crate::Row::Jig(row)), .. }] = &*events else {
            panic!("row read back, cut {cut} of {operations}: {events:?}");
        };
        assert_eq!(*row, fixtures::header_record(1), "cut {cut} of {operations}");
    }
}

fn first_manifest() -> crate::codec::component::Manifest {
    crate::codec::component::Manifest {
        generation: 1,
        from: None,
        before: crate::format::recorded(crate::format::NONE),
        after: crate::format::recorded(crate::format::NONE),
        counts: skein_lib::List::with_capacity(0),
        store: 0,
        secrets: None,
        written: 0,
    }
}

#[test]
fn the_first_manifest_records_the_binarys_levels_and_the_write_clock() {
    const WRITTEN: u64 = 73;
    let (mut store, mut below) = at_private();
    store.patch_sets(super::patching::SETS);
    let Some(file::Request::OpenPrivate { owner, .. }) = below.secrets.pop() else { panic!("first secrets") };
    let mut env = fixtures::env();
    env.wall = skein_lib::Wall::from_nanos(WRITTEN);
    let mut above = Queue::with_capacity(crate::MAX_OUT_DOWN);
    crate::up(
        &mut store,
        &env,
        Io::Secrets(Arrived {
            event: file::Event::Opened { owner, file: Token::new(100), len: 0 },
            fd: Some(Fd::new(4)),
        }),
        &mut above,
        &mut below,
    );
    let Some(file::Request::Store { bytes, path, .. }) = below.transcripts.pop() else { panic!("first manifest") };
    assert_eq!(path.as_ref(), b"generations/1/manifest");
    assert_eq!(
        crate::codec::component::Manifest::decode(&bytes),
        Ok(crate::codec::component::Manifest {
            after: crate::codec::component::Levels { skein: None, smith: None, jig: Some(2), application: Some(2) },
            counts: fixtures::manifest_counts(fixtures::FIRST_MANIFEST_COUNTS),
            written: WRITTEN,
            ..first_manifest()
        })
    );
}

#[test]
fn a_first_start_counts_every_kept_family_zero() {
    for (sets, expected) in [
        (super::patching::SETS, fixtures::manifest_counts(fixtures::FIRST_MANIFEST_COUNTS)),
        (crate::PatchSets::NONE, fixtures::manifest_counts(&[])),
    ] {
        let (mut store, mut below) = at_private();
        store.patch_sets(sets);
        scripted(&mut store, &mut below);
        let Some(file::Request::Store { bytes, .. }) = below.transcripts.pop() else { panic!("first manifest") };
        let manifest = crate::codec::component::Manifest::decode(&bytes).expect("first manifest decodes");
        assert_eq!(manifest.counts, expected);
    }
}
