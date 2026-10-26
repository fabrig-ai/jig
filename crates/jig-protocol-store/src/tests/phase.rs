//! The patch phase on a simulated disk (store.md 10.2 and 10.4).
#![expect(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::disallowed_macros,
    clippy::wildcard_enum_match_arm,
    clippy::match_like_matches_macro,
    reason = "disk step tests use ordinary Rust (programming-model.md 10.2)"
)]
use super::fixtures::{
    self,
    disk::{Disk, Seen},
};
use super::patching::{BASE, SETS, decoded, label, levels, shelf, token};
use crate::directory::{OWNER, Opening};
use crate::patching::phase::{Patching, Pending};
use crate::{Arrived, Event, Get, Io, OpenRefusal, Phase, Request, Root, Row, Store, Write};
use jig_test_patches::patches as app;
use jig_test_patches_jig::patches as jig;
use skein_fake_machine::{How, Is, Item, Machine, Opened};
use skein_io::{
    file,
    file_layer::{self, FileIo},
    kernel::{Complete, Error, Kind, Submit},
};
use skein_lib::{Queue, Token};
use skein_sim::{Answer, Ask, Config, Handle, Pid, Sim};

const CHUNK: u32 = 4096;
const KEY: &[u8] = b"row";
const VALUE: &[u8] = b"committed";
fn empty_pending() -> Pending {
    let sets = crate::PatchSets::NONE;
    let found = crate::patching::plan::binary(&sets);
    let plan = crate::patching::plan::plan(&sets, &found, &found).expect("empty plan");
    let machines =
        crate::patching::machines::make(&sets, &plan, &found, super::patching::ROOMS).expect("empty machines");
    Pending { plan, machines, found }
}
fn assert_opened(events: &[Event], expected: u64) {
    let [Event::Opened { owner, last, .. }] = events else { panic!("one opened: {events:?}") };
    assert_eq!(*owner, Token::new(1));
    assert_eq!(*last, expected);
}

fn bridge_open(
    store: &mut Store,
    env: &skein_lib::Env<crate::Limits>,
    event: &Event,
    switching: &mut bool,
    above: &mut Queue<Event>,
    below: &mut crate::Below,
) -> bool {
    if let Event::Opened { pending: true, owner, .. } = event
        && !*switching
    {
        *switching = true;
        // This store-only driver has no engine root to ask for the switch.
        crate::down(store, env, Request::Switch { owner: *owner }, above, below);
    }
    if let Event::Switched { .. } = event { true } else { false }
}
fn baseline() -> Vec<Item> {
    baseline_with_proof(false)
}
fn baseline_with_proof(proof: bool) -> Vec<Item> {
    baseline_with_sets(proof, crate::PatchSets::NONE)
}
fn baseline_with_sets(proof: bool, sets: crate::PatchSets) -> Vec<Item> {
    let mut disk = Disk::new(&[]);
    let mut store = disk.store_with_chunk(CHUNK);
    store.patch_sets(sets);
    let events = disk.open(&mut store);
    assert!({ if let [Event::Opened { last: 0, .. }] = events.as_ref() { true } else { false } });
    let mut writes = vec![fixtures::header(1), Write::Put { key: Box::from(KEY), value: Box::from(VALUE) }];
    if proof {
        writes.push(fixtures::save(fixtures::run_proof(1)));
    }
    let events = disk.request(&mut store, fixtures::commit(1, writes.into_boxed_slice()));
    assert!({ if let [Event::Committed { number: 1, .. }] = events.as_ref() { true } else { false } });
    disk.items()
}
fn read_row(disk: &mut Disk, store: &mut Store) {
    let events = disk.request(store, Request::Get { owner: Token::new(2), key: Get::Application(Box::from(KEY)) });
    let [Event::Got { row: Some(Row::Application { key, value }), .. }] = events.as_ref() else {
        panic!("committed application row: {events:?}")
    };
    assert_eq!(key.as_ref(), KEY);
    assert_eq!(value.as_ref(), VALUE);
}
fn patch(disk: &mut Disk) -> Store {
    let mut store = disk.store_with_chunk(CHUNK);
    // An empty plan exercises the phase's file ordering without fixture records.
    store.directory.patch = Some(Box::new(empty_pending()));
    let events = disk.open(&mut store);
    assert_opened(&events, store.stores.store.last());
    let events = disk.request(&mut store, Request::Switch { owner: Token::new(1) });
    assert!({ if let [Event::Switched { owner }] = events.as_ref() { *owner == Token::new(1) } else { false } });
    store
}
#[test]
fn a_patch_opens_the_copy_with_the_committed_row_and_keeps_the_previous_whole() {
    let mut disk = Disk::new(&baseline());
    let mut store = patch(&mut disk);
    assert_eq!(disk.bytes(b"active").as_ref(), b"2\n");
    assert_manifest(&disk.bytes(b"generations/2/manifest"), 2, crate::format::NONE, crate::format::NONE, 2);
    let contents = disk.contents();
    // The maps are copied; each generation has its own manifest.
    for (path, kind, _, bytes) in contents
        .iter()
        .filter(|(p, ..)| p.starts_with(b"generations/1/store") || p.starts_with(b"generations/1/secrets"))
    {
        let target = [b"generations/2/", path.strip_prefix(b"generations/1/").expect("prefix")].concat();
        let (_, copied_kind, _, copied_bytes) =
            contents.iter().find(|(p, ..)| p.as_ref() == target).expect("whole copy");
        assert_eq!(copied_kind, kind);
        if path.starts_with(b"generations/1/secrets/") {
            assert_eq!(copied_bytes, bytes);
        }
    }
    read_row(&mut disk, &mut store);
    disk.crash(17);
    let mut store = disk.store_with_chunk(CHUNK);
    assert_opened(&disk.open(&mut store), 2);
    assert_eq!(store.directory.active.expect("active read").number, 2);
    assert!(
        !disk
            .requests()
            .iter()
            .any(|(driver, r)| *driver == 2 && { if let Seen::Remove { .. } = r { true } else { false } })
    );
    read_row(&mut disk, &mut store);
}
#[test]
fn the_next_patch_drops_one_keeps_two_and_opens_three() {
    let mut first = Disk::new(&baseline());
    patch(&mut first);
    let mut disk = Disk::new(&first.items());
    let mut store = patch(&mut disk);
    assert_eq!(disk.bytes(b"active").as_ref(), b"3\n");
    assert_manifest(&disk.bytes(b"generations/3/manifest"), 3, crate::format::NONE, crate::format::NONE, 3);
    let tree = disk.tree();
    assert!(!tree.iter().any(|(p, ..)| p.as_ref() == b"generations/1"));
    for generation in [2_u64, 3] {
        for map in ["store", "secrets"] {
            let path = format!("generations/{generation}/{map}");
            assert!(
                tree.iter()
                    .any(|(p, kind, mode)| p.as_ref() == path.as_bytes() && *kind == Kind::Directory && *mode == 0o700)
            );
        }
    }
    read_row(&mut disk, &mut store);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Other,
    Closing,
    Dropping,
    Copying,
    Opening,
    Walking,
    Windows,
    Sealing,
    Checking,
    Manifest,
    Switching,
}
fn stage(store: &Store) -> Stage {
    match &store.directory.opening {
        Opening::Patching(phase) => match **phase {
            Patching::Closing { .. } => Stage::Closing,
            Patching::Dropping(_) => Stage::Dropping,
            Patching::Copying(_) => Stage::Copying,
            Patching::Mapping { .. } | Patching::Recovering { .. } => Stage::Opening,
            Patching::Walking(ref walking) => {
                if walking.closing() {
                    Stage::Windows
                } else {
                    Stage::Walking
                }
            }
            Patching::Sealing { .. } => Stage::Sealing,
            Patching::Checking(_) => Stage::Checking,
        },
        Opening::Pending(pending) => match pending.stage {
            crate::patching::switch::Stage::Manifest { .. } => Stage::Manifest,
            crate::patching::switch::Stage::Switching { .. } => Stage::Switching,
            _ => Stage::Other,
        },
        _ => Stage::Other,
    }
}
#[derive(Debug, PartialEq, Eq)]
enum Trace {
    KvClose(usize),
    MapClose(usize),
    MapOpen(usize, Box<[u8]>),
    KvOpen(usize),
    Store(Box<[u8]>),
    Load(Box<[u8]>),
    Cut,
    Remove(Box<[u8]>),
    Other,
}
fn trace(driver: usize, request: &file::Request) -> Trace {
    match request {
        file::Request::Close { owner, .. } if *owner == OWNER => Trace::MapClose(driver),
        file::Request::Close { .. } => Trace::KvClose(driver),
        file::Request::OpenDirectory { name, .. } => Trace::MapOpen(driver, name.clone()),
        file::Request::OpenPrivate { path, .. } => Trace::MapOpen(driver, path.clone()),
        file::Request::List { .. } => Trace::KvOpen(driver),
        file::Request::Store { path, .. } => Trace::Store(path.clone()),
        file::Request::Load { path, .. } => Trace::Load(path.clone()),
        file::Request::Cut { .. } => Trace::Cut,
        file::Request::Remove { name, .. } => Trace::Remove(name.clone()),
        _ => Trace::Other,
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Change {
    Create,
    Write,
    Sync,
    Rename,
    Remove,
    Cut,
    Make,
    Other,
}
fn change(ask: &Ask) -> Change {
    match ask {
        Ask::Open {
            how: skein_io::kernel::OpenHow::Create { .. } | skein_io::kernel::OpenHow::CreateNoFollow { .. },
            ..
        } => Change::Create,
        Ask::Write { .. } | Ask::Append { .. } => Change::Write,
        Ask::Remove { .. } => Change::Remove,
        Ask::Truncate { .. } => Change::Cut,
        Ask::MakeDirectory { .. } => Change::Make,
        Ask::Sync { .. } => Change::Sync,
        Ask::Rename { .. } => Change::Rename,
        _ => Change::Other,
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum SiteAnswer {
    Written,
    NoSite,
    WrongOwner,
}
/// Same three real file drivers as `fixtures::disk`, with a seam inside Store's
/// durable replace. Cuts and faults act on the simulator's machine calls.
struct Harness {
    machine: Machine,
    handle: Opened,
    sim: Sim,
    pid: Pid,
    root: Root,
    io: [FileIo; 3],
    seeded: crate::Seeded,
    trace: Vec<(Stage, Trace)>,
    changes: Vec<(Stage, Change)>,
    cut: Option<(usize, bool)>,
    fault_write: bool,
    fault_at: Option<Stage>,
    halted: bool,
    source: Option<Vec<Item>>,
    sites: Vec<(crate::Owner, Box<[u8]>)>,
    site_answer: SiteAnswer,
    site_commit_limit: Option<u32>,
    site_secret_commit_limit: Option<u32>,
    page_header: Option<u64>,
    check_entries: Vec<Item>,
    wall: skein_lib::Wall,
}
impl Harness {
    fn new(items: &[Item]) -> Self {
        let mut machine = Machine::for_user(0);
        let handle = machine.lay(items);
        Self::over(machine, handle)
    }
    fn over(machine: Machine, handle: Opened) -> Self {
        let mut sim = Sim::new(1, Config::calm());
        let pid = sim.spawn_process();
        let fd = sim.root(pid, Handle::new(handle.raw()));
        let mut io = core::array::from_fn(|_| {
            let limits = fixtures::store_limits();
            let mut io = FileIo::new(8, limits.store.chunk, 32, limits.store.deadline).with_effective_user(0);
            assert_eq!(io.adopt_root(fd), Some(Token::new(1)));
            io
        });
        let [public, private, transcripts] = &mut io;
        let seeded = crate::seed([public, private, transcripts], 1);
        Self {
            machine,
            handle,
            sim,
            pid,
            root: Root { token: Token::new(1), fd },
            io,
            seeded,
            trace: Vec::new(),
            changes: Vec::new(),
            cut: None,
            fault_write: false,
            fault_at: None,
            halted: false,
            source: None,
            sites: Vec::new(),
            site_answer: SiteAnswer::Written,
            site_commit_limit: None,
            site_secret_commit_limit: None,
            page_header: None,
            check_entries: Vec::new(),
            wall: skein_lib::Wall::EPOCH,
        }
    }
    fn store(&self, patch: bool) -> Store {
        let mut limits = fixtures::store_limits();
        limits.store.chunk = CHUNK;
        limits.store.value =
            CHUNK.checked_sub(limits.store.key).expect("key fits").checked_sub(20).expect("header fits");
        limits.secrets.chunk = CHUNK;
        limits.secrets.value =
            CHUNK.checked_sub(limits.secrets.key).expect("key fits").checked_sub(20).expect("header fits");
        let mut store = Store::on_directory(&limits, fixtures::APPLICATION, &[], self.seeded);
        if patch {
            store.directory.patch = Some(Box::new(empty_pending()));
        }
        store
    }
    fn open(&mut self, store: &mut Store) -> Box<[Event]> {
        let mut env = fixtures::env();
        env.wall = self.wall;
        let mut above = Queue::with_capacity(128);
        let mut below = fixtures::below();
        crate::down(
            store,
            &env,
            Request::Open { owner: Token::new(1), root: self.root, installation: fixtures::INSTALLATION },
            &mut above,
            &mut below,
        );
        let mut switching = false;
        for _ in 0_u32..10_000 {
            let mut retained = Vec::new();
            while let Some(event) = above.pop() {
                if bridge_open(store, &env, &event, &mut switching, &mut above, &mut below) {
                    continue;
                }
                if let Event::Patched { of: crate::Owner::Application, patch: 2, .. } = &event {
                    // Tests can lower the page limit after recovery, at the
                    // first written site, and restore it before sealing.
                    if self.site_commit_limit.is_some() {
                        store.limits.store.commit = fixtures::store_limits().store.commit;
                    }
                }
                if let Event::Site { owner, of, key, site, at, mut patcher } = event {
                    self.sites.push((of, key.clone()));
                    if let Some(limit) = self.site_commit_limit {
                        store.limits.store.commit = limit;
                    }
                    if let Some(limit) = self.site_secret_commit_limit {
                        store.limits.secrets.commit = limit;
                    }
                    let result = if self.site_answer == SiteAnswer::NoSite {
                        Err(skein_codec::patch::Halt::NoSite(site))
                    } else {
                        let mut cx = jig_test_patches::patches::Context { key };
                        let (old, new) = patcher.parts();
                        jig_test_patches::patches::written(site, old, new, at, &mut cx)
                    };
                    crate::down(
                        store,
                        &env,
                        Request::Site {
                            owner: if self.site_answer == SiteAnswer::WrongOwner { Token::new(99) } else { owner },
                            patcher,
                            result,
                        },
                        &mut above,
                        &mut below,
                    );
                } else {
                    retained.push(event);
                }
            }
            for event in retained {
                above.push(event);
            }
            crate::fire(store, &env, &mut above, &mut below);

            let next = below
                .store
                .pop()
                .map(|r| (0, r))
                .or_else(|| below.secrets.pop().map(|r| (1, r)))
                .or_else(|| below.transcripts.pop().map(|r| (2, r)));
            let Some((driver, request)) = next else {
                if stage(store) == Stage::Walking || stage(store) == Stage::Windows {
                    continue;
                }
                return fixtures::events(&mut above);
            };
            let stage = stage(store);
            if stage == Stage::Checking && !self.check_entries.is_empty() {
                let items = core::mem::take(&mut self.check_entries);
                self.lay_private(&items);
            }
            if stage == Stage::Walking
                && let Some(last) = self.page_header
            {
                assert_page_rows(store, last);
            }
            if stage == Stage::Closing && self.source.is_none() {
                self.source = Some(self.source_items(store.directory.active.expect("active read").number));
            }
            self.trace.push((stage, trace(driver, &request)));
            let Some(arrived) = self.file(driver, request, stage) else {
                return fixtures::events(&mut above);
            };
            let io = match driver {
                0 => Io::Store(arrived),
                1 => Io::Secrets(arrived),
                2 => Io::Transcripts(arrived),
                _ => unreachable!(),
            };
            crate::up(store, &env, io, &mut above, &mut below);
        }
        panic!("phase did not settle");
    }
    fn file(&mut self, driver: usize, request: file::Request, stage: Stage) -> Option<Arrived> {
        let mut events = Queue::with_capacity(128);
        let mut subs: Queue<Submit> = Queue::with_capacity(128);
        let mut completions: Queue<Complete> = Queue::with_capacity(128);
        file_layer::down(&mut self.io[driver], fixtures::env().now, request, &mut events, &mut subs);
        for _ in 0_u32..100 {
            self.sim.submit(self.pid, &mut subs);
            let mut calls = Queue::with_capacity(128);
            let mut answers = Queue::with_capacity(128);
            self.sim.calls(&mut calls);
            while let Some(call) = calls.pop() {
                let changing = change(&call.ask);
                let targeted = (stage == Stage::Manifest
                    || stage == Stage::Opening
                    || stage == Stage::Walking
                    || stage == Stage::Windows
                    || stage == Stage::Sealing
                    || stage == Stage::Checking)
                    && changing != Change::Other;
                let ordinal = self.changes.len();
                if targeted {
                    self.changes.push((stage, changing));
                    if self.cut == Some((ordinal, false)) {
                        self.halted = true;
                        return None;
                    }
                }
                if (changing == Change::Write || stage == Stage::Checking)
                    && ((self.fault_write && stage == Stage::Manifest) || self.fault_at == Some(stage))
                {
                    self.fault_write = false;
                    self.fault_at = None;
                    answers.push(Answer { ticket: call.ticket, result: Err(Error::NoSpace) });
                } else {
                    skein_fake_machine::step(&mut self.machine, call, &mut answers);
                }
                self.sim.answer(&mut answers);
                if targeted && self.cut == Some((ordinal, true)) {
                    self.halted = true;
                    return None;
                }
            }
            self.sim.reap(self.pid, &mut completions);
            while let Some(complete) = completions.pop() {
                file_layer::up(&mut self.io[driver], complete, &mut events, &mut subs);
            }
            if self.io[driver].takes() {
                let event = events.pop().expect("file terminal");
                let fd = if let file::Event::Opened { file, .. } = &event {
                    self.io[driver].descriptor(*file)
                } else {
                    None
                };
                return Some(Arrived { event, fd });
            }
        }
        panic!("file did not settle");
    }
    fn bytes(&mut self, path: &[u8]) -> Box<[u8]> {
        let file = self.machine.open(self.handle, path, How::ReadNoFollow).expect("file stands");
        let bytes = self.machine.read(file, 0, u32::MAX).expect("file read");
        self.machine.close(file);
        bytes.into_boxed_slice()
    }
    fn items(&mut self) -> Vec<Item> {
        let mut items = Vec::new();
        self.walk(b"", &mut items);
        items
    }
    fn walk(&mut self, prefix: &[u8], items: &mut Vec<Item>) {
        let directory = self
            .machine
            .open(self.handle, if prefix.is_empty() { b"." } else { prefix }, How::Directory)
            .expect("directory");
        loop {
            let entries = self.machine.list(directory, 32, 8192).expect("list");
            if entries.is_empty() {
                break;
            }
            for (kind, name) in entries {
                let path = if prefix.is_empty() { name.to_vec() } else { [prefix, b"/", &name].concat() };
                if kind == Is::Directory {
                    items.push(Item::directory(&path).mode(0o700));
                    self.walk(&path, items);
                } else {
                    items.push(Item::file(&path, &self.bytes(&path)).mode(0o600));
                }
            }
        }
        self.machine.close(directory);
    }
    // Lay a file, or a segment and its second link, through the fake's Item
    // vocabulary. Move them beneath the live copy without changing its handles.
    fn lay_private(&mut self, laid: &[Item]) {
        use skein_fake_machine::Made;
        let mut items = Vec::new();
        for item in laid {
            if let Made::HardLink(target) = &item.made {
                let path = [b"generations/2/secrets/", target.as_slice()].concat();
                items.push(Item::file(target, &self.bytes(&path)).mode(0o600));
            }
        }
        items.extend_from_slice(laid);
        let staged = self.machine.lay(&items);
        let private = self
            .machine
            .open(self.handle, b"generations/2/secrets", How::DirectoryNoFollow)
            .expect("sealed copy's private directory");
        for item in &items {
            self.machine.rename(staged, &item.path, private, &item.path, false).expect("lay check entry");
        }
        self.machine.sync(private);
        self.machine.close(private);
        self.machine.close(staged);
    }
    fn source_items(&mut self, number: u64) -> Vec<Item> {
        let prefix = format!("generations/{number}/");
        self.items().into_iter().filter(|item| item.path.starts_with(prefix.as_bytes())).collect()
    }
    fn assert_source_unchanged(&mut self, number: u64) {
        assert_eq!(self.source_items(number), *self.source.as_ref().expect("captured before closing A"));
    }
    fn crash(self, seed: u64) -> Self {
        let mut machine = self.machine;
        machine.crash(seed);
        let handle = machine.reopen_root(0);
        Self::over(machine, handle)
    }
}
#[test]
fn the_phase_opens_pending_then_manifests_and_switches_without_reopening() {
    let mut first = Disk::new(&baseline());
    patch(&mut first);
    let mut disk = Harness::new(&first.items());
    let mut store = disk.store(true);
    assert_opened(&disk.open(&mut store), 3);
    let stages: Vec<_> = disk.trace.iter().map(|(stage, _)| *stage).filter(|s| *s != Stage::Other).fold(
        Vec::new(),
        |mut stages, stage| {
            if stages.last() != Some(&stage) {
                stages.push(stage);
            }
            stages
        },
    );
    assert_eq!(
        stages,
        [
            Stage::Closing,
            Stage::Dropping,
            Stage::Copying,
            Stage::Opening,
            Stage::Sealing,
            Stage::Checking,
            Stage::Manifest,
            Stage::Switching
        ]
    );
    {
        let stage = Stage::Closing;
        let closes: Vec<_> = disk.trace.iter().filter(|(s, _)| *s == stage).map(|(_, request)| request).collect();
        // Both KV log closes must finish before either map directory closes.
        assert_eq!(closes, [&Trace::KvClose(0), &Trace::KvClose(1), &Trace::MapClose(0), &Trace::MapClose(1)]);
    }
    let opens: Vec<_> = disk
        .trace
        .iter()
        .filter(|(s, _)| *s == Stage::Opening)
        .map(|(_, r)| r)
        .filter(|r| **r != Trace::Other)
        .collect();
    assert_eq!(opens.first(), Some(&&Trace::MapOpen(0, Box::from(b"generations/3/store".as_slice()))));
    assert_eq!(opens.get(1), Some(&&Trace::MapOpen(1, Box::from(b"generations/3/secrets".as_slice()))));
    assert!(opens.contains(&&Trace::KvOpen(0)) && opens.contains(&&Trace::KvOpen(1)));
    let switch = disk
        .trace
        .iter()
        .position(|(s, r)| *s == Stage::Switching && *r == Trace::Store(Box::from(b"active".as_slice())))
        .expect("switch");
    assert_eq!(switch + 1, disk.trace.len(), "the switch needs no reopen");
    assert_eq!(
        disk.trace
            .iter()
            .filter(|(_, r)| { if let Trace::Load(p) = r { p.as_ref() == b"active" } else { false } })
            .count(),
        1
    );
}
#[test]
fn cuts_at_the_manifests_write_syncs_and_rename_and_copy_recovery_keep_a_whole_generation() {
    let items = baseline();
    let mut completed = Harness::new(&items);
    let mut store = completed.store(true);
    assert_opened(&completed.open(&mut store), 2);
    let changes = completed.changes.clone();
    for change in [Change::Write, Change::Sync, Change::Rename] {
        assert!(changes.contains(&(Stage::Manifest, change)), "manifest {change:?}");
    }
    assert!(changes.contains(&(Stage::Opening, Change::Sync)), "copy recovery reaches the simulated disk");
    for (cut, target) in changes.iter().enumerate() {
        for after in [false, true] {
            let mut disk = Harness::new(&items);
            disk.cut = Some((cut, after));
            let mut store = disk.store(true);
            let events = disk.open(&mut store);
            assert_eq!(events.len(), usize::from(target.0 == Stage::Manifest), "cut {cut} {target:?}");
            assert!(
                events
                    .iter()
                    .all(|event| { if let Event::Opened { pending: true, .. } = event { true } else { false } })
            );
            assert!(disk.halted, "cut reached");
            disk.assert_source_unchanged(1);
            let mut disk = disk.crash(u64::try_from(cut).expect("seed"));
            let mut store = disk.store(false);
            assert_opened(&disk.open(&mut store), 1);
            assert_eq!(disk.bytes(b"active").as_ref(), b"1\n");
            let mut recovered = Disk::new(&disk.items());
            let mut store = recovered.store_with_chunk(CHUNK);
            assert_opened(&recovered.open(&mut store), 1);
            read_row(&mut recovered, &mut store);
            assert!(!recovered.tree().iter().any(|(p, ..)| p.starts_with(b"generations/2")), "cut {cut} {target:?}");
        }
    }
}
#[test]
fn a_manifest_write_fault_refuses_failed_and_the_next_start_removes_the_copy() {
    let mut disk = Harness::new(&baseline());
    disk.fault_write = true;
    let mut store = disk.store(true);
    let events = disk.open(&mut store);
    assert!({
        if let [Event::Opened { pending: true, .. }, Event::OpenRefused { why: OpenRefusal::Failed, .. }] =
            events.as_ref()
        {
            true
        } else {
            false
        }
    });
    assert_eq!(store.phase, Phase::Stopped);
    assert!(!disk.fault_write, "fault injected into the manifest's write");
    disk.assert_source_unchanged(1);
    assert_eq!(disk.bytes(b"active").as_ref(), b"1\n");
    let mut disk = disk.crash(19);
    let mut store = disk.store(false);
    assert_opened(&disk.open(&mut store), 1);
    let mut recovered = Disk::new(&disk.items());
    let mut store = recovered.store_with_chunk(CHUNK);
    assert_opened(&recovered.open(&mut store), 1);
    read_row(&mut recovered, &mut store);
    assert!(!recovered.tree().iter().any(|(p, ..)| p.starts_with(b"generations/2")));
}

#[test]
fn the_run_proofs_transcript_cut_precedes_the_pending_open_and_switch() {
    let mut items = baseline_with_proof(true);
    items.push(Item::directory(b"transcripts/0000000000000000"));
    items.push(Item::file(b"transcripts/0000000000000000/0000000000000001", b"uncommitted tail"));
    let mut disk = Harness::new(&items);
    let mut store = disk.store(true);
    assert_opened(&disk.open(&mut store), 2);
    let switch = disk.trace.iter().position(|(s, _)| *s == Stage::Switching).expect("switch");
    let cuts: Vec<_> = disk
        .trace
        .iter()
        .enumerate()
        .filter(|(_, (_, r))| *r == Trace::Cut)
        .map(|(index, (stage, _))| (index, *stage))
        .collect();
    assert_eq!(cuts.len(), 1);
    assert!(cuts.iter().all(|(index, stage)| *index < switch && *stage == Stage::Other));
    assert!(disk.bytes(b"transcripts/0000000000000000/0000000000000001").is_empty());
    disk.assert_source_unchanged(1);
}

#[test]
fn the_manifest_write_refuses_to_replace_a_standing_manifest() {
    let mut disk = Disk::new(&[
        Item::directory(b"generations"),
        Item::directory(b"generations/2"),
        Item::file(b"generations/2/manifest", b"standing"),
    ]);
    let mut below = fixtures::below();
    let mut writing = crate::generations::store_manifest(disk.root(), 2, Box::from(b"2\n".as_slice()), &mut below);
    let mut progress = crate::generations::Progress::Pending;
    disk.drive(&mut below, |io, _| {
        let Io::Transcripts(arrived) = io else {
            panic!("manifest uses driver 2");
        };
        progress = crate::generations::manifested(&mut writing, arrived);
    });
    assert_eq!(progress, crate::generations::Progress::Failed);
    assert_eq!(disk.bytes(b"generations/2/manifest").as_ref(), b"standing");
}

const FIXTURE_LAST: u64 = 3;
const SHELVES: u32 = 5;
fn fixture_key(prefix: u8, number: u32) -> Box<[u8]> {
    format!("{}/{number}", char::from(prefix)).into_bytes().into_boxed_slice()
}
fn fixture_baseline(bad: Option<(u32, u16, u32)>) -> Vec<Item> {
    fixture_baseline_with_secret(bad, false)
}
fn fixture_baseline_with_secret(bad: Option<(u32, u16, u32)>, secret: bool) -> Vec<Item> {
    let mut disk = Disk::new(&[]);
    let mut store = disk.store_with_chunk(CHUNK);
    store.patch_sets(BASE);
    store.application.version = 0;
    assert_opened(&disk.open(&mut store), 0);
    let mut rows = vec![
        Write::Put { key: fixture_key(b'l', 0), value: label(1) },
        Write::Put { key: fixture_key(b'l', 1), value: label(1) },
        // keys::application adds exactly tag 2, so this lands inside the
        // fixture-jig family's whole-map range [02 6e, 02 6f).
        Write::Put { key: Box::from(b"n/0".as_slice()), value: label(1) },
    ];
    let note_key = crate::keys::application(b"n/0", store.limits.store.key).expect("tag");
    assert!(note_key.as_ref() >= jig_test_patches_jig::keys::NOTES_FROM);
    assert!(note_key.as_ref() < jig_test_patches_jig::keys::NOTES_TO);
    for number in 0..SHELVES {
        let (version, books) = match bad {
            Some((index, version, books)) if index == number => (version, books),
            _ => (1, number.checked_add(1).expect("books")),
        };
        rows.push(Write::Put { key: fixture_key(b's', number), value: shelf(version, books, &label(1)) });
    }
    // Three rows and the header fit the fixture's four-write journal.
    let mut rows = rows.into_iter();
    for number in 1..=FIXTURE_LAST {
        let mut writes: Vec<_> = rows.by_ref().take(3).collect();
        writes.push(fixtures::header(number));
        if number == FIXTURE_LAST {
            // The last fixture page holds two rows, its header, and the start mark.
            writes.push(Write::Wires);
        }
        let events = disk.request(&mut store, fixtures::commit(number, writes.into_boxed_slice()));
        assert!(
            match events.as_ref() {
                [Event::Committed { number: found, .. }] => *found == number,
                _ => false,
            },
            "{events:?}"
        );
    }
    assert_eq!(store.stores.store.last(), FIXTURE_LAST);
    if secret {
        let events = disk
            .request(&mut store, Request::Secrets { owner: Token::new(2), request: fixtures::sign_in_secret(1, 7) });
        let [Event::Kept { .. }] = events.as_ref() else { panic!("fixture secret committed: {events:?}") };
    }
    disk.items()
}

struct SecretFixture {
    items: Vec<Item>,
    store_last: u64,
    secrets_last: u64,
    carried: Box<[u8]>,
    counter: Box<[u8]>,
    accounts: Box<[u8]>,
}
fn secret_row(store: &mut Store, key: &[u8]) -> Box<[u8]> {
    let mut above = Queue::with_capacity(skein_kv::MAX_OUT.above);
    let mut below = Queue::with_capacity(skein_kv::MAX_OUT.below);
    skein_kv::down(
        &mut store.stores.secrets,
        &crate::memory_env(store.limits.secrets),
        skein_kv::Request::Get { owner: Token::new(0), key: Box::from(key) },
        &mut above,
        &mut below,
    );
    assert!(below.is_empty());
    let Some(skein_kv::Event::Got { value: Some(value), .. }) = above.pop() else {
        panic!("secret row exists");
    };
    value
}
fn secret_key(store: &Store, number: u64) -> Box<[u8]> {
    crate::secrets::application(&number.to_be_bytes(), store.limits.secrets.key).expect("application prefix")
}
fn accounts_key(store: &Store) -> Box<[u8]> {
    let key =
        crate::keys::encode(&jig_domain_core::Key::Accounts(jig_domain_accounts::Key::Next), store.limits.store.key)
            .expect("accounts counter key");
    assert_eq!(key.as_ref(), &[1, 4, 0]);
    key
}
fn secret_fixture() -> SecretFixture {
    let mut disk = Disk::new(&fixture_baseline(None));
    let mut store = disk.store_with_chunk(CHUNK);
    store.patch_sets(BASE);
    store.application.version = 0;
    assert_opened(&disk.open(&mut store), FIXTURE_LAST);
    let number = FIXTURE_LAST.checked_add(1).expect("counter commit");
    let events = disk.request(
        &mut store,
        fixtures::commit(
            number,
            Box::new([
                fixtures::header(number),
                Write::Save(jig_domain_core::Record::Accounts(jig_domain_accounts::Stored::Next { number: 97 })),
                Write::Wires,
            ]),
        ),
    );
    let [Event::Committed { number, .. }] = events.as_ref() else { panic!("counter committed: {events:?}") };
    assert_eq!(*number, FIXTURE_LAST.checked_add(1).expect("counter commit"));
    for number in 0_u64..3 {
        let events = disk.request(
            &mut store,
            Request::Secrets {
                owner: Token::new(2),
                request: crate::Secrets::Keep {
                    owner: crate::SecretOwner::Application,
                    number,
                    value: crate::SecretValue::Application(skein_lib::Secret::new(token(
                        b"token",
                        number.checked_add(71).expect("issued"),
                    ))),
                },
            },
        );
        let [Event::Kept { .. }] = events.as_ref() else { panic!("token kept: {events:?}") };
    }
    let events = disk.request(
        &mut store,
        Request::Secrets {
            owner: Token::new(2),
            request: crate::Secrets::Carry {
                kind: crate::CarriedKind::Value,
                value: skein_lib::Secret::new(Box::from(b"carried bytes".as_slice())),
            },
        },
    );
    let [Event::Carried { number, .. }] = events.as_ref() else { panic!("value carried: {events:?}") };
    assert_eq!(*number, crate::CARRIED);
    let key = secret_key(&store, crate::CARRIED);
    let carried = secret_row(&mut store, &key);
    let counter = secret_row(&mut store, &[2, 1]);
    assert_eq!(
        crate::secrets_v1::NextCarried::decode(&crate::secrets_v1::CEILINGS, &mut skein_lib::Reader::new(&counter))
            .expect("versioned counter")
            .next(),
        crate::CARRIED.checked_add(1).expect("next carried number")
    );
    let key = accounts_key(&store);
    let accounts = fixtures::get_kv(&mut store, &key).expect("accounts counter");
    SecretFixture {
        items: disk.items(),
        store_last: store.stores.store.last(),
        secrets_last: store.stores.secrets.last(),
        carried,
        counter,
        accounts,
    }
}
fn assert_tokens(store: &mut Store) {
    use app::kept::fixture_token::v2::token as fields;
    for number in 0_u64..3 {
        let key = secret_key(store, number);
        let bytes = secret_row(store, &key);
        let (_, value, root) = decoded(app::kept::fixture_token::TEXTS[1], "Token", &bytes);
        assert_eq!(value.content(value.field(root, fields::VALUE).expect("value")).expect("bytes"), b"token");
        assert_eq!(
            value.uint(value.field(root, fields::ISSUED).expect("issued")).expect("u64"),
            number.checked_add(71).expect("issued")
        );
        assert_eq!(value.some(value.field(root, fields::EXPIRES).expect("expires")).expect("option"), None);
    }
}

#[test]
fn secret_tokens_patch_in_their_own_commits() {
    let before = secret_fixture();
    let mut disk = Harness::new(&before.items);
    let mut store = fixture_store(&disk, 2, fixtures::store_limits().store.commit, levels(2, 2));
    store.limits.secrets.ops = 2;
    let events = disk.open(&mut store);
    assert_tokens(&mut store);
    let key = secret_key(&store, crate::CARRIED);
    assert_eq!(secret_row(&mut store, &key), before.carried);
    assert_eq!(secret_row(&mut store, &[2, 1]), before.counter);
    let key = accounts_key(&store);
    assert_eq!(fixtures::get_kv(&mut store, &key).expect("accounts counter"), before.accounts);
    assert_eq!(store.stores.secrets.last(), before.secrets_last.checked_add(2).expect("token pages"));
    let last = before.store_last.checked_add(9).expect("eight store pages and seal");
    assert_opened(events.get(events.len().checked_sub(1).expect("opened")..).expect("last event"), last);
    assert_eq!(
        crate::codec::component::Manifest::decode(&disk.bytes(b"generations/2/manifest")),
        Ok(crate::codec::component::Manifest {
            generation: 2,
            from: Some(1),
            before: crate::format::recorded(levels(0, 0)),
            after: crate::format::recorded(levels(2, 2)),
            counts: fixture_counts(3),
            store: last,
            secrets: Some(store.stores.secrets.last()),
            written: 0,
        })
    );
    disk.assert_source_unchanged(1);
}

#[test]
fn a_secrets_page_commits_in_the_secrets_store_alone() {
    let before = secret_fixture();
    let mut disk = Harness::new(&before.items);
    let mut store = fixture_store(&disk, 2, fixtures::store_limits().store.commit, levels(2, 2));
    store.limits.secrets.ops = 2;
    let events = disk.open(&mut store);
    assert_eq!(
        patched(&events),
        [
            (crate::Owner::Application, 1, crate::Counts { records: 7, commits: 4 }),
            (crate::Owner::Jig, 1, crate::Counts { records: 1, commits: 1 }),
            (crate::Owner::Application, 2, crate::Counts { records: 8, commits: 5 }),
            (crate::Owner::Jig, 2, crate::Counts { records: 0, commits: 0 }),
        ]
    );
    assert_eq!(store.stores.secrets.last().checked_sub(before.secrets_last), Some(2));
    assert_eq!(store.stores.store.last().checked_sub(before.store_last), Some(9));
    assert_sealed(&mut store, before.store_last.checked_add(9).expect("eight store pages and seal"));
}

#[test]
fn secrets_pages_use_their_own_byte_and_operation_limits() {
    let before = secret_fixture();
    // Each v1 row is 29 bytes; each v2 put is 39 plus the commit's 26.
    // The journal alone must not split a secrets page. The secrets page
    // bytes and commit bytes must each split it independently.
    for (writes, ops, page_bytes, commit_bytes, commits) in
        [(1, 3, 4096, 4096, 1), (2, 16, 58, 4096, 2), (2, 16, 4096, 104, 2)]
    {
        let mut disk = Harness::new(&before.items);
        let mut store = fixture_store(&disk, writes, fixtures::store_limits().store.commit, levels(2, 2));
        store.limits.secrets.ops = ops;
        store.limits.secrets.page.bytes = page_bytes;
        store.limits.secrets.commit = commit_bytes;
        let events = disk.open(&mut store);
        let Some(Event::Opened { .. }) = events.last() else { panic!("patched open: {events:?}") };
        assert_eq!(store.stores.secrets.last().checked_sub(before.secrets_last), Some(commits));
        assert_tokens(&mut store);
    }
}
// A binary with only the first application patch landed.
const ONE: crate::PatchSets = crate::PatchSets {
    sets: &[
        crate::PatchSet { patches: &[app::PATCHES[0]], ..SETS.sets[0] },
        crate::PatchSet { patches: &[], ..SETS.sets[1] },
    ],
};
fn fixture_store(disk: &Harness, writes: u32, commit: u32, wanted: crate::patching::plan::Levels) -> Store {
    let mut store = disk.store(false);
    store.limits.journal.writes = writes;
    store.limits.store.commit = commit;
    store.patch_sets(if wanted == levels(1, 0) { ONE } else { SETS });
    store
}
fn patched(events: &[Event]) -> Vec<(crate::Owner, u32, crate::Counts)> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Patched { owner, of, patch, counts } => {
                assert_eq!(*owner, Token::new(1));
                Some((*of, *patch, *counts))
            }
            Event::Opened { .. } | Event::OpenRefused { .. } => None,
            _ => panic!("unexpected open answer: {event:?}"),
        })
        .collect()
}
fn assert_fixture_opened(events: &[Event], page_commits: u64) {
    assert_opened(
        events.get(events.len().checked_sub(1).expect("opened")..).expect("last event"),
        FIXTURE_LAST.checked_add(page_commits).expect("pages").checked_add(1).expect("seal"),
    );
}
fn fixture_rows(store: &mut Store, shelf_version: u16, note_version: u16) -> Vec<Box<[u8]>> {
    let mut rows = Vec::new();
    for number in 0..2 {
        let key = crate::keys::application(&fixture_key(b'l', number), store.limits.store.key).expect("key");
        let bytes = fixtures::get_kv(store, &key).expect("label");
        drop(decoded(app::kept::fixture_label::TEXTS[1], "Label", &bytes));
        assert_eq!(bytes, label(2));
        rows.push(bytes);
    }
    for number in 0..SHELVES {
        let relative = fixture_key(b's', number);
        let key = crate::keys::application(&relative, store.limits.store.key).expect("key");
        let bytes = fixtures::get_kv(store, &key).expect("shelf");
        let text = app::kept::fixture_shelf::TEXTS
            .get(usize::from(shelf_version.checked_sub(1).expect("version")))
            .expect("text");
        let (_, value, root) = decoded(text, "Shelf", &bytes);
        let held = value
            .content(value.field(root, app::kept::fixture_shelf::v1::shelf::LABEL).expect("label"))
            .expect("bytes");
        assert_eq!(held, label(2).as_ref());
        if shelf_version == 2 {
            let weight = value
                .uint(value.field(root, app::kept::fixture_shelf::v2::shelf::WEIGHT).expect("weight"))
                .expect("u32");
            let expected = u64::from(number.checked_add(1).expect("books"))
                .checked_mul(10)
                .expect("weight")
                .checked_add(u64::try_from(relative.len()).expect("key bytes"))
                .expect("weight");
            assert_eq!(weight, expected);
        }
        rows.push(bytes);
    }
    let key = crate::keys::application(b"n/0", store.limits.store.key).expect("key");
    let bytes = fixtures::get_kv(store, &key).expect("note");
    let text =
        jig::kept::fixture_note::TEXTS.get(usize::from(note_version.checked_sub(1).expect("version"))).expect("text");
    let (_, value, root) = decoded(text, "Note", &bytes);
    if note_version == 2 {
        assert!(
            !value
                .boolean(value.field(root, jig::kept::fixture_note::v2::note::PINNED).expect("pinned"))
                .expect("bool")
        );
    }
    rows.push(bytes);
    rows
}
fn assert_sealed(store: &mut Store, last: u64) {
    let header_key =
        crate::keys::encode(&jig_domain_core::Key::Core(jig_domain_core::CoreKey::Deployment), store.limits.store.key)
            .expect("header key");
    let header = fixtures::get_kv(store, &header_key).expect("sealed header");
    let expected = fixtures::header_put(fixtures::DEPLOYMENT, last);
    let skein_kv::Op::Put { value, .. } = expected else { panic!("header put") };
    assert_eq!(header, value);
    let key = crate::keys::encode_own(&crate::keys::Own::Format, store.limits.store.key).expect("format key");
    let mut expected = crate::format::current(store.application.version, fixtures::DEPLOYMENT);
    expected.levels = crate::format::recorded(crate::patching::plan::binary(&store.patch_sets));
    assert_eq!(fixtures::get_kv(store, &key).expect("format"), expected.encode().expect("format bytes"));
    let key = crate::keys::encode_own(&crate::keys::Own::Layouts, store.limits.store.key).expect("layouts key");
    assert_eq!(fixtures::get_kv(store, &key), Some(store.layouts.encode()));
}
#[test]
fn fixture_patches_walk_in_plan_order_answer_relative_keys_and_seal_three_rows() {
    let mut disk = Harness::new(&fixture_baseline(None));
    let mut store = fixture_store(&disk, 8, fixtures::store_limits().store.commit, levels(2, 2));
    disk.page_header = Some(FIXTURE_LAST);
    let events = disk.open(&mut store);
    assert_eq!(
        patched(&events),
        [
            (crate::Owner::Application, 1, crate::Counts { records: 7, commits: 2 }),
            (crate::Owner::Jig, 1, crate::Counts { records: 1, commits: 1 }),
            (crate::Owner::Application, 2, crate::Counts { records: 5, commits: 1 }),
            (crate::Owner::Jig, 2, crate::Counts { records: 0, commits: 0 }),
        ]
    );
    let expected_sites: Vec<_> = (0..SHELVES).map(|n| (crate::Owner::Application, fixture_key(b's', n))).collect();
    assert_eq!(disk.sites, expected_sites);
    assert_fixture_opened(&events, 4);
    fixture_rows(&mut store, 2, 2);
    assert_sealed(&mut store, FIXTURE_LAST.checked_add(5).expect("last"));
    assert_eq!(disk.bytes(b"active").as_ref(), b"2\n");
    assert_manifest(
        &disk.bytes(b"generations/2/manifest"),
        2,
        levels(0, 0),
        levels(2, 2),
        FIXTURE_LAST.checked_add(5).expect("seal"),
    );
    disk.assert_source_unchanged(1);
    // A plain restart reads the format and header from the durable seal too.
    let mut disk = disk.crash(29);
    let mut store = disk.store(false);
    store.patch_sets(SETS);
    assert_opened(&disk.open(&mut store), FIXTURE_LAST.checked_add(5).expect("last"));
    fixture_rows(&mut store, 2, 2);
    assert_sealed(&mut store, FIXTURE_LAST.checked_add(5).expect("last"));
}
#[test]
fn patch_one_alone_rewrites_held_labels_but_keeps_shelves_at_version_one() {
    let mut disk = Harness::new(&fixture_baseline(None));
    let mut store = fixture_store(&disk, 8, fixtures::store_limits().store.commit, levels(1, 0));
    let events = disk.open(&mut store);
    assert_eq!(patched(&events), [(crate::Owner::Application, 1, crate::Counts { records: 7, commits: 2 })]);
    assert_fixture_opened(&events, 2);
    assert!(disk.sites.is_empty());
    fixture_rows(&mut store, 1, 1);
}
#[test]
fn two_put_pages_take_three_shelf_commits_and_match_the_one_page_records() {
    let items = fixture_baseline(None);
    let mut whole = Harness::new(&items);
    let mut store = fixture_store(&whole, 8, fixtures::store_limits().store.commit, levels(2, 2));
    whole.open(&mut store);
    let expected = fixture_rows(&mut store, 2, 2);
    let mut paged = Harness::new(&items);
    let mut store = fixture_store(&paged, 2, fixtures::store_limits().store.commit, levels(2, 2));
    let events = paged.open(&mut store);
    assert_eq!(
        patched(&events),
        [
            (crate::Owner::Application, 1, crate::Counts { records: 7, commits: 4 }),
            (crate::Owner::Jig, 1, crate::Counts { records: 1, commits: 1 }),
            (crate::Owner::Application, 2, crate::Counts { records: 5, commits: 3 }),
            (crate::Owner::Jig, 2, crate::Counts { records: 0, commits: 0 }),
        ]
    );
    assert_fixture_opened(&events, 8);
    assert_eq!(fixture_rows(&mut store, 2, 2), expected);
}
fn assert_refused(events: &[Event], store: &Store) {
    assert!(
        match events.last() {
            Some(Event::OpenRefused { owner, why: OpenRefusal::Failed }) => *owner == Token::new(1),
            _ => false,
        },
        "{events:?}"
    );
    assert_eq!(store.phase, Phase::Stopped);
}
fn assert_abandoned(disk: Harness) {
    assert_abandoned_at(disk, FIXTURE_LAST);
}
fn assert_abandoned_at(disk: Harness, last: u64) {
    assert_abandoned_with(disk, last, BASE, 0);
}
fn assert_abandoned_with(disk: Harness, last: u64, sets: crate::PatchSets, version: u32) {
    let (_, events) = reopen_abandoned(disk, sets, version);
    assert_opened(&events, last);
}
fn reopen_abandoned(mut disk: Harness, sets: crate::PatchSets, version: u32) -> (Store, Box<[Event]>) {
    disk.assert_source_unchanged(1);
    assert_eq!(disk.bytes(b"active").as_ref(), b"1\n");
    let source = disk.source_items(1);
    let mut disk = disk.crash(31);
    let mut store = disk.store(false);
    store.patch_sets(sets);
    store.application.version = version;
    let events = disk.open(&mut store);
    assert!(!disk.items().iter().any(|item| item.path.starts_with(b"generations/2")));
    assert_eq!(disk.source_items(1), source);
    (store, events)
}
#[test]
fn a_shell_without_the_written_site_refuses_and_the_next_start_removes_the_copy() {
    let mut disk = Harness::new(&fixture_baseline(None));
    disk.site_answer = SiteAnswer::NoSite;
    let mut store = fixture_store(&disk, 8, fixtures::store_limits().store.commit, levels(2, 2));
    let events = disk.open(&mut store);
    assert_eq!(patch_refusal(&events, 2, 0), &skein_codec::patch::Halt::NoSite(skein_codec::patch::Site::new(2, 0)));
    assert_eq!(store.phase, Phase::Stopped);
    assert_eq!(disk.sites.len(), 1);
    assert_abandoned(disk);
}
#[test]
fn an_empty_shelf_refuses_before_any_put_of_its_page_commits() {
    let mut disk = Harness::new(&fixture_baseline(Some((1, 1, 0))));
    let mut store = fixture_store(&disk, 8, fixtures::store_limits().store.commit, levels(2, 2));
    let events = disk.open(&mut store);
    assert_eq!(patch_refusal(&events, 2, 1), &skein_codec::patch::Halt::Refused(app::refusal::p0002::EMPTY));
    assert_eq!(store.phase, Phase::Stopped);
    assert_eq!(disk.sites.len(), 2, "one successful record then Refused at the next site");
    let key = crate::keys::application(&fixture_key(b's', 0), store.limits.store.key).expect("key");
    assert_eq!(fixtures::get_kv(&mut store, &key).expect("uncommitted page"), shelf(1, 1, &label(2)));
    assert_eq!(patched(&events).len(), 2, "the failed patch never reports Patched");
    disk.assert_source_unchanged(1);
    assert_eq!(disk.bytes(b"active").as_ref(), b"1\n");
    let mut retry = disk.crash(31);
    let mut store = fixture_store(&retry, 8, fixtures::store_limits().store.commit, levels(2, 2));
    let events = retry.open(&mut store);
    assert_eq!(patch_refusal(&events, 2, 1), &skein_codec::patch::Halt::Refused(app::refusal::p0002::EMPTY));
    assert!(
        retry
            .trace
            .iter()
            .any(|(stage, trace)| *stage == Stage::Other && *trace == Trace::Remove(Box::from(b"2".as_slice()))),
        "the next start removes the abandoned copy"
    );
    assert_abandoned(retry);
}
#[test]
fn a_shelf_at_another_version_stops_the_start_without_changing_the_active_generation() {
    let mut disk = Harness::new(&fixture_baseline(Some((1, 2, 2))));
    let mut store = fixture_store(&disk, 8, fixtures::store_limits().store.commit, levels(2, 2));
    let events = disk.open(&mut store);
    let skein_codec::patch::Halt::Decode(_) = patch_refusal(&events, 1, 1) else { panic!("version halt: {events:?}") };
    assert_eq!(store.phase, Phase::Stopped);
    assert!(disk.sites.is_empty(), "identity decoding refuses the version before patch two");
    assert_abandoned(disk);
}
#[test]
fn a_site_answer_from_another_owner_refuses_the_open() {
    let mut disk = Harness::new(&fixture_baseline(None));
    disk.site_answer = SiteAnswer::WrongOwner;
    let mut store = fixture_store(&disk, 8, fixtures::store_limits().store.commit, levels(2, 2));
    assert_refused(&disk.open(&mut store), &store);
    assert_abandoned(disk);
}
#[test]
fn a_site_request_without_a_waiting_site_refuses_failed() {
    let mut disk = Disk::new(&baseline());
    let mut store = disk.store_with_chunk(CHUNK);
    assert_opened(&disk.open(&mut store), 1);
    let patcher = Box::new(skein_codec::patch::Patcher::new(skein_codec::patch::Capacity {
        versions: 0,
        patches: 0,
        text: 0,
        bytes: 0,
        nodes: 0,
    }));
    let events = disk.request(&mut store, Request::Site { owner: Token::new(1), patcher, result: Ok(()) });
    assert_refused(&events, &store);
}
#[test]
fn cuts_at_each_page_and_sealing_write_and_sync_reopen_the_unchanged_source() {
    let items = fixture_baseline(None);
    let mut completed = Harness::new(&items);
    let mut store = fixture_store(&completed, 2, fixtures::store_limits().store.commit, levels(2, 2));
    assert_fixture_opened(&completed.open(&mut store), 8);
    let changes = completed.changes.clone();
    for stage in [Stage::Walking, Stage::Sealing] {
        for change in [Change::Write, Change::Sync] {
            assert!(changes.contains(&(stage, change)), "{stage:?} {change:?}");
        }
    }
    for (cut, (stage, change)) in changes.iter().enumerate().filter(|(_, (stage, change))| {
        (match stage {
            Stage::Walking | Stage::Sealing => true,
            _ => false,
        }) && match change {
            Change::Write | Change::Sync => true,
            _ => false,
        }
    }) {
        for after in [false, true] {
            let mut disk = Harness::new(&items);
            disk.cut = Some((cut, after));
            let mut store = fixture_store(&disk, 2, fixtures::store_limits().store.commit, levels(2, 2));
            let events = disk.open(&mut store);
            assert!(disk.halted, "cut {cut} {stage:?} {change:?} after={after}");
            assert!(!events.iter().any(|e| match e {
                Event::Opened { .. } | Event::OpenRefused { .. } => true,
                _ => false,
            }));
            assert_abandoned(disk);
        }
    }
}

#[test]
fn patching_peak_grows_with_kept_texts_and_record_room() {
    let mut limits = fixtures::store_limits();
    limits.store.value = 128;
    let none = crate::patching::phase::worst_case(&limits, &crate::PatchSets::NONE).expect("empty peak");
    let baseline = crate::patching::phase::worst_case(&limits, &BASE).expect("baseline texts");
    let kept = crate::patching::phase::worst_case(&limits, &SETS).expect("kept peak");
    assert!(baseline > none);
    assert!(kept > baseline, "the families' kept texts, plans and machines contribute");
    limits.store.value = 256;
    assert!(crate::patching::phase::worst_case(&limits, &SETS).expect("more room") > kept);
}
#[test]
fn the_store_prices_the_larger_peak_and_worst_case_prices_none() {
    let mut limits = fixtures::store_limits();
    // A large decoded page makes serving dominate; a tiny page and large
    // record machines make patching dominate. Shared maps are counted once.
    for patching_larger in [false, true] {
        limits.store.page.bytes = if patching_larger { 1 } else { 16_778_240 };
        limits.store.value = if patching_larger { 16_777_216 } else { 128 };
        limits.joins = if patching_larger { 0 } else { 32 };
        let (common, serving) = crate::serving_peak(&limits).expect("serving peak");
        let patching = crate::patching::phase::worst_case(&limits, &SETS).expect("patching peak");
        assert_eq!(patching > serving, patching_larger, "both sides of the maximum");
        let total = crate::worst_case_with_patches(&limits, &SETS).expect("total");
        assert_eq!(total.checked_sub(common), Some(if patching_larger { patching } else { serving }));
        assert!(total < common.checked_add(serving).expect("serving").checked_add(patching).expect("both"));
        assert_eq!(crate::worst_case(&limits), crate::worst_case_with_patches(&limits, &crate::PatchSets::NONE));
    }
}

#[test]
fn a_plan_without_any_matching_records_reports_zero_counts_and_still_seals() {
    let mut disk = Harness::new(&baseline_with_sets(false, BASE));
    let mut store = fixture_store(&disk, 8, fixtures::store_limits().store.commit, levels(2, 2));
    let events = disk.open(&mut store);
    let zero = crate::Counts { records: 0, commits: 0 };
    assert_eq!(
        patched(&events),
        [
            (crate::Owner::Application, 1, zero),
            (crate::Owner::Jig, 1, zero),
            (crate::Owner::Application, 2, zero),
            (crate::Owner::Jig, 2, zero)
        ]
    );
    assert_opened(events.get(4..).expect("open after patches"), 2);
    assert_sealed(&mut store, 2);
}
#[test]
fn one_put_past_the_commit_byte_bound_stops_before_submitting_its_page() {
    let mut disk = Harness::new(&fixture_baseline(None));
    let mut store = fixture_store(&disk, 8, fixtures::store_limits().store.commit, levels(2, 2));
    disk.site_commit_limit = Some(65); // 26 + (tagged key 4 + shelf 27 + put overhead 9) - 1.
    let events = disk.open(&mut store);
    assert_check(&events, &store, crate::Check::Large { key: fixture_key(b's', 0) });
    assert_eq!(patched(&events).len(), 2, "patch two never commits its first page");
    assert_eq!(store.stores.store.last(), FIXTURE_LAST.checked_add(3).expect("prior patches"));
    assert_abandoned(disk);
}
#[test]
fn page_and_sealing_write_faults_refuse_and_leave_the_source_active() {
    let items = fixture_baseline(None);
    for stage in [Stage::Walking, Stage::Sealing] {
        let mut disk = Harness::new(&items);
        disk.fault_at = Some(stage);
        let mut store = fixture_store(&disk, 8, fixtures::store_limits().store.commit, levels(2, 2));
        let events = disk.open(&mut store);
        assert_refused(&events, &store);
        assert_eq!(disk.fault_at, None, "fault reached {stage:?}");
        assert_abandoned(disk);
    }
}

#[test]
fn a_full_byte_page_defers_the_next_finished_record_without_repeating_its_site() {
    let items = fixture_baseline(None);
    let mut whole = Harness::new(&items);
    let mut store = fixture_store(&whole, 8, fixtures::store_limits().store.commit, levels(2, 2));
    whole.open(&mut store);
    let expected = fixture_rows(&mut store, 2, 2);
    let mut disk = Harness::new(&items);
    // Exactly three shelf puts: 26 + 3 * (tagged key 4 + value 27 + 9).
    disk.site_commit_limit = Some(146);
    let mut store = fixture_store(&disk, 8, fixtures::store_limits().store.commit, levels(2, 2));
    let events = disk.open(&mut store);
    assert_eq!(
        patched(&events).get(2),
        Some(&(crate::Owner::Application, 2, crate::Counts { records: 5, commits: 2 }))
    );
    assert_fixture_opened(&events, 5);
    assert_eq!(disk.sites.len(), usize::try_from(SHELVES).expect("shelves"));
    assert_eq!(fixture_rows(&mut store, 2, 2), expected);
}

fn assert_manifest(
    bytes: &[u8],
    generation: u64,
    before: crate::patching::plan::Levels,
    after: crate::patching::plan::Levels,
    last: u64,
) {
    assert_eq!(
        crate::codec::component::Manifest::decode(bytes),
        Ok(crate::codec::component::Manifest {
            generation,
            from: generation.checked_sub(1),
            before: crate::format::recorded(before),
            after: crate::format::recorded(after),
            counts: if after == levels(2, 2) { fixture_counts(0) } else { skein_lib::List::with_capacity(0) },
            store: last,
            secrets: Some(0),
            written: 0,
        })
    );
}

fn patch_refusal(events: &[Event], expected_patch: u32, number: u32) -> &skein_codec::patch::Halt {
    let Some(Event::OpenRefused { owner: token, why: OpenRefusal::Patch { owner, patch, key, halt } }) = events.last()
    else {
        panic!("one patch refusal: {events:?}");
    };
    assert_eq!(*token, Token::new(1));
    assert_eq!(*owner, crate::Owner::Application);
    assert_eq!(*patch, expected_patch);
    assert_eq!(key, &fixture_key(b's', number));
    halt
}

#[test]
fn a_patch_manifest_records_the_copys_secret_commit_and_write_clock() {
    const WRITTEN: u64 = 73;
    let mut disk = Harness::new(&fixture_baseline_with_secret(None, true));
    disk.wall = skein_lib::Wall::from_nanos(WRITTEN);
    let mut store = fixture_store(&disk, 8, fixtures::store_limits().store.commit, levels(2, 2));
    assert_fixture_opened(&disk.open(&mut store), 4);
    assert_eq!(
        crate::codec::component::Manifest::decode(&disk.bytes(b"generations/2/manifest")),
        Ok(crate::codec::component::Manifest {
            generation: 2,
            from: Some(1),
            before: crate::format::recorded(levels(0, 0)),
            after: crate::format::recorded(levels(2, 2)),
            counts: fixture_counts(0),
            store: 8,
            secrets: Some(1),
            written: WRITTEN,
        })
    );
}

fn assert_page_rows(store: &mut Store, last: u64) {
    let key =
        crate::keys::encode(&jig_domain_core::Key::Core(jig_domain_core::CoreKey::Deployment), store.limits.store.key)
            .expect("header key");
    let bytes = fixtures::get_kv(store, &key).expect("page keeps header");
    let record = crate::codec::decode(
        &jig_domain_core::Key::Core(jig_domain_core::CoreKey::Deployment),
        &bytes,
        &store.codec,
        &store.names,
    )
    .expect("header decodes");
    assert_eq!(record, fixtures::header_record(last), "page commits never write the header");
    let key = crate::keys::encode_own(&crate::keys::Own::Format, store.limits.store.key).expect("format key");
    let mut expected = crate::format::current(0, fixtures::DEPLOYMENT);
    expected.levels = crate::format::recorded(levels(0, 0));
    assert_eq!(
        fixtures::get_kv(store, &key),
        Some(expected.encode().expect("format")),
        "page commits never write the format"
    );
    let key = crate::keys::encode_own(&crate::keys::Own::Layouts, store.limits.store.key).expect("layouts key");
    assert_eq!(
        fixtures::get_kv(store, &key),
        Some(crate::format::layouts().encode()),
        "page commits never write layouts"
    );
}

// component-v3.schema bounds: 64 counts, each family at most 64 bytes.
const MANIFEST_FAMILIES: usize = 64;
const MANIFEST_NAME: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789ab";
const MANIFEST_FAMILY: crate::KeptFamily = crate::KeptFamily {
    flights: &[],
    frames: None,
    name: MANIFEST_NAME,
    map: crate::KeptMap::Store,
    texts: &[],
    keys: &[],
};
const MANIFEST_SET: crate::PatchSet = crate::PatchSet {
    fresh: None,
    owner: crate::Owner::Application,
    name: "manifest-bounds",
    families: &[],
    patches: &[],
};

#[test]
fn manifest_bounds_accept_the_count_and_name_ceilings() {
    let sets = crate::PatchSets {
        sets: &[crate::PatchSet { families: &[MANIFEST_FAMILY; MANIFEST_FAMILIES], ..MANIFEST_SET }],
    };
    assert!(crate::patching::phase::worst_case(&fixtures::store_limits(), &sets).is_some());
}

#[test]
fn manifest_bounds_refuse_a_count_or_name_past_the_ceiling() {
    const HALF: usize = MANIFEST_FAMILIES / 2;
    const PAST_HALF: usize = HALF + 1;
    for sets in [
        crate::PatchSets {
            sets: &[
                crate::PatchSet { families: &[MANIFEST_FAMILY; HALF], ..MANIFEST_SET },
                crate::PatchSet { owner: crate::Owner::Jig, families: &[MANIFEST_FAMILY; PAST_HALF], ..MANIFEST_SET },
            ],
        },
        crate::PatchSets {
            sets: &[crate::PatchSet {
                families: &[crate::KeptFamily {
                    name: "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789abc",
                    ..MANIFEST_FAMILY
                }],
                ..MANIFEST_SET
            }],
        },
    ] {
        assert_eq!(crate::patching::phase::worst_case(&fixtures::store_limits(), &sets), None);
    }
}

fn fixture_counts(tokens: u64) -> skein_lib::List<crate::codec::component::Count> {
    fixtures::manifest_counts(&[
        ("fixture-label", 2, 2),
        ("fixture-shelf", 5, 5),
        ("fixture-token", tokens, tokens),
        ("fixture-note", 1, 1),
    ])
}

#[test]
fn the_manifest_counts_each_kept_family_before_and_after() {
    let before = secret_fixture();
    let mut disk = Harness::new(&before.items);
    let mut store = fixture_store(&disk, 2, fixtures::store_limits().store.commit, levels(2, 2));
    disk.open(&mut store);
    let manifest =
        crate::codec::component::Manifest::decode(&disk.bytes(b"generations/2/manifest")).expect("checked manifest");
    assert_eq!(manifest.counts, fixture_counts(3));
    let check_changes: Vec<_> = disk.changes.iter().filter(|(stage, _)| *stage == Stage::Checking).collect();
    assert!(check_changes.is_empty(), "the check writes nothing");
}

fn assert_check(events: &[Event], store: &Store, why: crate::Check) {
    let Some(Event::OpenRefused { owner, why: refusal }) = events.last() else { panic!("check refusal: {events:?}") };
    assert_eq!(*owner, Token::new(1));
    assert_eq!(*refusal, OpenRefusal::Check { why });
    assert_eq!(store.phase, Phase::Stopped);
}

#[test]
fn the_count_check_names_the_first_family_whose_rows_differ() {
    let mut store = fixtures::opened();
    store.patch_sets(SETS);
    fixtures::committed(
        &mut store,
        Box::new([fixtures::header(1), Write::Put { key: fixture_key(b'l', 0), value: label(1) }]),
    );
    let mut before = crate::patching::check::before(&mut store).expect("count kept rows");
    before.get_mut(0).expect("label count").before = 2;
    assert_eq!(
        crate::patching::check::counts(&mut store, before),
        Err(OpenRefusal::Check { why: crate::Check::Count { family: "fixture-label", before: 2, after: 1 } })
    );
}

fn fixture_with_record(record: jig_domain_core::Record) -> Vec<Item> {
    let mut disk = Disk::new(&fixture_baseline(None));
    let mut store = disk.store_with_chunk(CHUNK);
    store.patch_sets(BASE);
    store.application.version = 0;
    disk.open(&mut store);
    let last = FIXTURE_LAST + 1;
    let events =
        disk.request(&mut store, fixtures::commit(last, Box::new([fixtures::header(last), Write::Save(record)])));
    let [Event::Committed { .. }] = events.as_ref() else { panic!("base record committed: {events:?}") };
    disk.items()
}

fn ended_position() -> jig_domain_core::Record {
    jig_domain_core::Record::People(jig_domain_people::Stored::EndedRead {
        task: fixtures::ENDED,
        party: fixtures::ALICE,
        through: 1,
    })
}

#[test]
fn a_restart_row_past_the_deployments_lowered_limit_refuses_load() {
    let record = fixtures::live_task(fixtures::LIVE);
    let key = crate::keys::encode(&record.key(), fixtures::KEY_LIMIT).expect("live row key");
    let mut disk = Harness::new(&fixture_with_record(record));
    let mut store = fixture_store(&disk, 8, fixtures::store_limits().store.commit, levels(2, 2));
    // The fixture task has a dependency, valid under the base's two-entry limit.
    store.limits.core.tasks.dependencies = 0;
    store.codec = crate::codec::limits(&store.limits.core).expect("lower deployment limits");
    let events = disk.open(&mut store);
    assert_check(&events, &store, crate::Check::Load { key });
    assert_abandoned_at(disk, FIXTURE_LAST + 1);
}

#[test]
fn an_ended_read_without_its_task_refuses_the_join() {
    let record = ended_position();
    let key = crate::keys::encode(&record.key(), fixtures::KEY_LIMIT).expect("ended position key");
    let mut disk = Harness::new(&fixture_with_record(record));
    let mut store = fixture_store(&disk, 8, fixtures::store_limits().store.commit, levels(2, 2));
    let events = disk.open(&mut store);
    assert_check(&events, &store, crate::Check::Join { key });
    assert_abandoned_at(disk, FIXTURE_LAST + 1);
}

#[test]
fn a_secret_file_with_group_bits_or_another_link_refuses_private() {
    let before = secret_fixture();
    let prefix = b"generations/1/secrets/";
    let segment = before
        .items
        .iter()
        .find_map(|item| item.path.strip_prefix(prefix.as_slice()))
        .expect("source's secrets segment");
    // A refusal beyond a full listing page proves the scan does not stop at PAGE.
    let mut paged: Vec<_> = (0..crate::generations::PAGE)
        .map(|n| Item::file(format!("private-{n:02}").as_bytes(), b"page entry").mode(0o600))
        .collect();
    paged.push(Item::file(b"z-group", b"private check").mode(0o640));
    for (items, name) in [
        (vec![Item::file(b"!group", b"private check").mode(0o640)], b"!group".as_slice()),
        (vec![Item::hard_link(b"!linked", segment)], b"!linked".as_slice()),
        (paged, b"z-group".as_slice()),
    ] {
        let name = Box::from(name);
        let mut disk = Harness::new(&before.items);
        disk.check_entries = items;
        let mut store = fixture_store(&disk, 8, fixtures::store_limits().store.commit, levels(2, 2));
        let events = disk.open(&mut store);
        assert_check(&events, &store, crate::Check::Private { name });
        assert!(disk.check_entries.is_empty());
        assert_abandoned_at(disk, before.store_last);
    }
}

#[test]
fn a_secret_record_too_large_for_its_own_commit_refuses_large() {
    let before = secret_fixture();
    let mut disk = Harness::new(&before.items);
    let mut store = fixture_store(&disk, 8, fixtures::store_limits().store.commit, levels(2, 2));
    // The v1 token fits in 26 + 9 + 10 + 19 bytes; its v2 option adds one.
    disk.site_secret_commit_limit = Some(64);
    let events = disk.open(&mut store);
    assert_check(&events, &store, crate::Check::Large { key: Box::new(0_u64.to_be_bytes()) });
    assert_abandoned_at(disk, before.store_last);
}

#[test]
fn a_file_fault_during_the_check_refuses_failed_before_the_switch() {
    let mut disk = Harness::new(&fixture_baseline(None));
    disk.fault_at = Some(Stage::Checking);
    let mut store = fixture_store(&disk, 8, fixtures::store_limits().store.commit, levels(2, 2));
    let events = disk.open(&mut store);
    assert_refused(&events, &store);
    assert_eq!(disk.fault_at, None);
    assert_abandoned(disk);
}

#[test]
fn a_restart_row_at_the_deployments_lowered_limit_loads() {
    let record = fixtures::live_task(fixtures::LIVE);
    let mut disk = Harness::new(&fixture_with_record(record));
    let mut store = fixture_store(&disk, 8, fixtures::store_limits().store.commit, levels(2, 2));
    store.limits.core.tasks.dependencies = 1;
    store.codec = crate::codec::limits(&store.limits.core).expect("lower deployment limits");
    let events = disk.open(&mut store);
    let Some(Event::Opened { .. }) = events.last() else { panic!("row within its bound loads: {events:?}") };
    assert_eq!(disk.bytes(b"active").as_ref(), b"2\n");
}

// Two open windows, an uncrossed closed window and a windowless proof
// take two two-put commits. The fifth, already crossed and closed, is skipped.
const FRESH_TASKS: u64 = 5;
const FRESH_CLOSED_TASK: u64 = 3;
fn fresh_record(task: u64, crossed: bool) -> jig_domain_core::Record {
    use jig_domain_core::{CoreRecord, Record};
    let Record::Core(CoreRecord::RunProof(mut proof)) = jig_store_world::root::run_proof(task) else {
        panic!("run fixture")
    };
    if task == FRESH_TASKS.checked_sub(1).expect("windowless task") {
        proof.window = None;
    } else {
        proof.window.as_mut().expect("window fixture").closed =
            crossed || task == FRESH_CLOSED_TASK || task == FRESH_TASKS;
    }
    proof.crossed = crossed || task == FRESH_TASKS;
    Record::Core(CoreRecord::RunProof(proof))
}
fn fresh_baseline() -> Vec<Item> {
    let mut disk = Disk::new(&[]);
    let mut store = disk.store_with_chunk(CHUNK);
    store.patch_sets(FRESH_BEFORE);
    assert_opened(&disk.open(&mut store), 0);
    for task in 1..=FRESH_TASKS {
        let turn = jig_store_world::root::turn(task, 1, 1, b"committed transcript");
        let writes = Box::new([
            fixtures::header(task),
            fixtures::save(jig_store_world::root::live_task(task)),
            fixtures::save(fresh_record(task, false)),
            fixtures::save(jig_domain_core::Record::Core(jig_domain_core::CoreRecord::Turn(turn))),
        ]);
        let events = disk.request(&mut store, fixtures::commit(task, writes));
        let [Event::Committed { number, .. }] = events.as_ref() else { panic!("fresh fixture commit: {events:?}") };
        assert_eq!(*number, task);
    }
    disk.items()
}
fn fresh_store(disk: &Harness, after: bool) -> Store {
    let mut store = disk.store(false);
    store.limits.journal.writes = 2;
    store.patch_sets(if after { FRESH_AFTER } else { FRESH_BEFORE });
    store
}
// One credential exercises the closing walk's deployment-bounded decoder.
const FRESH_CREDENTIALS: &[u32] = &[2];
const FRESH_PROOF_TASK: u64 = 1;
fn bounded_fresh_proof() -> jig_domain_core::Record {
    let jig_domain_core::Record::Core(jig_domain_core::CoreRecord::RunProof(mut proof)) =
        fresh_record(FRESH_PROOF_TASK, false)
    else {
        panic!("fresh proof fixture")
    };
    proof.credentials = Box::from(FRESH_CREDENTIALS);
    jig_domain_core::Record::Core(jig_domain_core::CoreRecord::RunProof(proof))
}
fn fresh_proof_key() -> Box<[u8]> {
    crate::keys::encode(
        &jig_domain_core::Key::Core(jig_domain_core::CoreKey::RunProof(FRESH_PROOF_TASK)),
        fixtures::KEY_LIMIT,
    )
    .expect("fresh proof key")
}
fn fresh_source() -> (Disk, Store) {
    let mut disk = Disk::new(&fresh_baseline());
    let mut store = disk.store_with_chunk(CHUNK);
    store.patch_sets(FRESH_BEFORE);
    disk.open(&mut store);
    (disk, store)
}
fn fresh_with_proof() -> Vec<Item> {
    let (mut disk, mut store) = fresh_source();
    disk.request(
        &mut store,
        fixtures::commit(
            FRESH_TASKS + 1,
            Box::new([fixtures::header(FRESH_TASKS + 1), Write::Save(bounded_fresh_proof())]),
        ),
    );
    disk.items()
}
#[test]
fn a_closing_proof_past_its_lowered_decode_limit_refuses_load() {
    let mut disk = Harness::new(&fresh_with_proof());
    let mut store = fresh_store(&disk, true);
    store.limits.core.accounts.credentials = 0;
    store.codec = crate::codec::limits(&store.limits.core).expect("lower proof decode limit");
    let events = disk.open(&mut store);
    assert_check(&events, &store, crate::Check::Load { key: fresh_proof_key() });
    assert_abandoned_with(disk, FRESH_TASKS + 1, FRESH_BEFORE, fixtures::APPLICATION.version);
}
#[test]
fn a_closing_proof_at_its_decode_limit_keeps_its_credentials() {
    let mut disk = Harness::new(&fresh_with_proof());
    let mut store = fresh_store(&disk, true);
    store.limits.core.accounts.credentials = 1;
    store.codec = crate::codec::limits(&store.limits.core).expect("exact proof decode limit");
    let events = disk.open(&mut store);
    let Some(Event::Opened { .. }) = events.last() else { panic!("valid closing proof: {events:?}") };
    let jig_domain_core::Record::Core(jig_domain_core::CoreRecord::RunProof(mut expected)) = bounded_fresh_proof()
    else {
        panic!("proof fixture")
    };
    expected.window.as_mut().expect("window").closed = true;
    expected.crossed = true;
    let record = jig_domain_core::Record::Core(jig_domain_core::CoreRecord::RunProof(expected));
    assert_eq!(
        proof_bytes(&mut store, FRESH_PROOF_TASK),
        crate::codec::encode(&record, &store.codec, &[]).expect("proof")
    );
    assert_eq!(disk.bytes(b"active").as_ref(), b"2\n");
    disk.assert_source_unchanged(1);
}
// Private raw KV setup, driven by the existing disk: the public writer never
// produces these mismatched or malformed run-proof keys.
fn fresh_with_raw_proof(key: Box<[u8]>, record: jig_domain_core::Record) -> Vec<Item> {
    let (mut disk, mut store) = fresh_source();
    let ops = Box::new([
        fixtures::header_put(fixtures::DEPLOYMENT, FRESH_TASKS + 1),
        skein_kv::Op::Put { key, value: crate::codec::encode(&record, &store.codec, &[]).expect("raw proof") },
    ]);
    let env = crate::memory_env(store.limits.store);
    let mut above = Queue::with_capacity(skein_kv::MAX_OUT.above);
    let mut below = fixtures::below();
    skein_kv::down(
        &mut store.stores.store,
        &env,
        skein_kv::Request::Commit { owner: Token::new(1), ops },
        &mut above,
        &mut below.store,
    );
    disk.drive(&mut below, |io, below| {
        let Io::Store(arrived) = io else { panic!("raw proof uses the public map driver") };
        skein_kv::up(&mut store.stores.store, &env, arrived.event, &mut above, &mut below.store);
    });
    let Some(skein_kv::Event::Committed { number, .. }) = above.pop() else { panic!("raw proof committed") };
    assert_eq!(number, FRESH_TASKS + 1);
    assert!(above.is_empty());
    disk.items()
}
#[test]
fn a_closing_proof_at_another_tasks_key_refuses_load() {
    let key = fresh_proof_key();
    let mut disk = Harness::new(&fresh_with_raw_proof(key.clone(), fresh_record(2, false)));
    let mut store = fresh_store(&disk, true);
    let events = disk.open(&mut store);
    assert_check(&events, &store, crate::Check::Load { key });
    let (store, events) = reopen_abandoned(disk, FRESH_BEFORE, fixtures::APPLICATION.version);
    assert_refused(&events, &store);
}
#[test]
fn a_malformed_closing_proof_key_refuses_load() {
    // A valid run-proof address followed by an extra tag stays in its range
    // but cannot decode as a key (store.md 4.1).
    let mut key = fresh_proof_key().into_vec();
    key.push(0);
    let key = key.into_boxed_slice();
    let mut disk = Harness::new(&fresh_with_raw_proof(key.clone(), fresh_record(FRESH_PROOF_TASK, false)));
    let mut store = fresh_store(&disk, true);
    let events = disk.open(&mut store);
    assert_check(&events, &store, crate::Check::Load { key });
    let (store, events) = reopen_abandoned(disk, FRESH_BEFORE, fixtures::APPLICATION.version);
    assert_refused(&events, &store);
}
fn proof_bytes(store: &mut Store, task: u64) -> Box<[u8]> {
    let key = crate::keys::encode(
        &jig_domain_core::Key::Core(jig_domain_core::CoreKey::RunProof(task)),
        store.limits.store.key,
    )
    .expect("proof key");
    fixtures::get_kv(store, &key).expect("proof row")
}
fn assert_fresh_proofs(store: &mut Store, crossed: bool) {
    for task in 1..=FRESH_TASKS {
        let expected =
            crate::codec::encode(&fresh_record(task, crossed), &store.codec, &store.names).expect("proof encoding");
        assert_eq!(proof_bytes(store, task), expected, "task {task}");
    }
}
fn transcript_items(items: &[Item]) -> Vec<Item> {
    items.iter().filter(|item| item.path.starts_with(b"transcripts/")).cloned().collect()
}
#[test]
fn fresh_closing_pages_keep_transcripts_and_previous_proofs_byte_identical() {
    let items = fresh_baseline();
    let mut disk = Harness::new(&items);
    let mut store = fresh_store(&disk, true);
    let events = disk.open(&mut store);
    assert_eq!(patched(&events), [(crate::Owner::Smith, 1, crate::Counts { records: 0, commits: 0 })]);
    assert_fresh_proofs(&mut store, true);
    assert_eq!(store.stores.store.last(), FRESH_TASKS.checked_add(3).expect("two pages and seal"));
    assert_eq!(transcript_items(&disk.items()), transcript_items(&items));
    disk.assert_source_unchanged(1);
}
#[test]
fn each_closing_write_and_sync_cut_retries_from_open_windows() {
    let items = fresh_baseline();
    let mut completed = Harness::new(&items);
    let mut store = fresh_store(&completed, true);
    completed.open(&mut store);
    let cuts: Vec<_> = completed
        .changes
        .iter()
        .enumerate()
        .filter_map(|(index, (stage, change))| {
            (*stage == Stage::Windows && [Change::Write, Change::Sync].contains(change)).then_some(index)
        })
        .collect();
    assert_eq!(cuts.len(), 4, "two closing writes and syncs");
    for cut in cuts {
        let mut disk = Harness::new(&items);
        disk.cut = Some((cut, false));
        let mut store = fresh_store(&disk, true);
        let events = disk.open(&mut store);
        assert!(disk.halted, "cut {cut}: {events:?}");
        disk.assert_source_unchanged(1);
        let mut disk = disk.crash(u64::try_from(cut).expect("seed"));
        let mut store = fresh_store(&disk, false);
        assert_opened(&disk.open(&mut store), FRESH_TASKS);
        assert_fresh_proofs(&mut store, false);
        assert_eq!(disk.bytes(b"active").as_ref(), b"1\n");
        let mut disk = disk.crash(u64::try_from(cut).expect("seed"));
        let mut store = fresh_store(&disk, true);
        disk.open(&mut store);
        assert_fresh_proofs(&mut store, true);
        assert_eq!(disk.bytes(b"active").as_ref(), b"2\n");
        assert_eq!(transcript_items(&disk.items()), transcript_items(&items));
        disk.assert_source_unchanged(1);
    }
}
#[test]
fn a_closing_write_fault_refuses_failed_without_switching() {
    let mut disk = Harness::new(&fresh_baseline());
    disk.fault_at = Some(Stage::Windows);
    let mut store = fresh_store(&disk, true);
    let events = disk.open(&mut store);
    let Some(Event::OpenRefused { why, .. }) = events.last() else { panic!("closing refusal: {events:?}") };
    assert_eq!(*why, OpenRefusal::Failed);
    assert_eq!(disk.fault_at, None, "fault reached the closing write");
    assert_eq!(disk.bytes(b"active").as_ref(), b"1\n");
    disk.assert_source_unchanged(1);
}

const FRESH_AFTER: crate::PatchSets = crate::PatchSets { sets: &[super::patching::SMITH] };
const FRESH_BEFORE: crate::PatchSets = crate::PatchSets {
    sets: &[crate::PatchSet {
        patches: &[],
        families: &[crate::KeptFamily {
            texts: &[jig_test_patches_smith::patches::kept::fixture_turn::TEXTS[0]],
            ..super::patching::SMITH.families[0]
        }],
        ..super::patching::SMITH
    }],
};

#[test]
fn pending_state_and_wires_have_their_exact_resident_price() {
    // Store gains the 16-byte static wires slice and the 16-byte Mark
    // (8-byte discriminant, 8-byte commit number; store.md 5.3). Pending retains before/after
    // levels, the check's counts and the explicit switch's stage and owner.
    // Rebased store.md 5.3 adds secrets/skein_oauth/smith_transcript hashes (96 bytes);
    // the removed settled_call_name bound and alignment save 8: Store 4,008 -> 4,096.
    // engine.md 17.2/17.7/17.8 adds revision/version bounds to settings, tasks and fleet (24 bytes).
    // engine.md 17.6 adds core_v6::Limits.settings_refusal_value, one u32.
    // The codec limits live inline in Store, whose alignment rounds that addition up.
    let settings_refusal_limit = size_of::<u32>().next_multiple_of(align_of::<Store>());
    assert_eq!(size_of::<Store>(), 4_120 + settings_refusal_limit);
    assert_eq!(size_of::<crate::codec::component::Layouts>(), 320);
    // store.md 9: typed SecretValue makes each SecretRow 64 rather than 32 bytes.
    assert_eq!(size_of::<crate::SecretRow>(), 64);
    assert_eq!(size_of::<crate::directory::Directory>(), 280);
    assert_eq!(size_of::<Opening>(), 88);
    assert_eq!(size_of::<Patching>(), 168);
    assert_eq!(size_of::<Pending>(), 136);
    // Pending: before 32 + after 32 + counts List 32 + switch stage 80.
    assert_eq!(size_of::<crate::patching::switch::Pending>(), 176);
    assert_eq!(size_of::<crate::patching::switch::Stage>(), 80);
    // Retained and encoded counts: two * (64 Count carriers * 32 +
    // 64 family names * 64), plus the 5,464-byte encoded manifest.
    assert_eq!(crate::patching::switch::worst_case(), Some(17_928));
    // Baseline 581,544,799,687,077 + rebased Store 88 + two SecretRows * 32
    // + revision/version bounds 24 + task/run settings revision fields in generated
    // row, decoded/joined task and commit/frame carriers 268,452,352 (engine.md 17.2)
    // + wires 16 + pending 17,928 + Mark 16 + the aligned settings-refusal limit above.
    let settings_refusal_limit = u64::try_from(settings_refusal_limit).expect("resident limit fits u64");
    assert_eq!(crate::worst_case(&fixtures::store_limits()), Some(581_545_068_157_565 + settings_refusal_limit));
}
