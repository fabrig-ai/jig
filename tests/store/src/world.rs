//! Three file drivers over the simulated crashing machine (protocol/store.md, 14).
use crate::{model::Model, root};
use jig_domain_core as core;
use jig_protocol_store::{
    Arrived, Below, Counts, Event, Get, Io, Load, OpenRefusal, Owner, PatchSets, Request, Restore, Root, Row, Secrets,
    Store, TranscriptRead, Write,
};
use skein_fake_machine::{Machine, serve};
use skein_io::{
    file,
    file_layer::{self, FileIo},
    kernel::{Complete, Submit},
};
use skein_lib::{Env, Queue, Time, Token, Wall};
use skein_sim::{Ask, Config, Handle, Pid, Sim};
use std::collections::BTreeMap;
const OWNER: Token = Token::new(900);
const QUEUE: u32 = 128;

#[derive(Debug)]
pub struct Restored {
    pub last: u64,
    pub rows: BTreeMap<core::Key, core::Record>,
    pub application: BTreeMap<Box<[u8]>, Box<[u8]>>,
    pub transcripts: BTreeMap<u64, Vec<core::TurnRecord>>,
}

impl Restored {
    #[must_use]
    pub fn turns(&self, task: u64) -> &[core::TurnRecord] {
        self.transcripts.get(&task).map_or(&[], Vec::as_slice)
    }
}

pub struct World {
    observer: Option<fn(Step)>,
    sets: PatchSets,
    patched: Vec<(Owner, u32, Counts)>,
    refusal: Option<OpenRefusal>,
    kernel_cuts: bool,
    changes: Vec<(u32, Change)>,
    written: Vec<(u32, Box<[u8]>)>,
    halted: bool,
    machine: Machine,
    sim: Sim,
    pid: Pid,
    root: Root,
    io: [FileIo; 3],
    store: Store,
    env: Env<jig_protocol_store::Limits>,
    above: Queue<Event>,
    below: Below,
    subs: Queue<Submit>,
    completions: Queue<Complete>,
    cut: Option<u32>,
    operations: u32,
    starts: u64,
    transcript_sync: Option<u32>,
    next: u64,
    pub model: Model,
    /// The identifier the shell draws for the next start (configuration.md 8).
    pub drawn: [u8; 16],
    /// The installation's identifier the last open answered (store.md 4.4).
    pub installation: [u8; 16],
    pub pending: bool,
}

/// Optional ordinary-Rust observation seam, around store calls only.
#[derive(Clone, Copy, Debug)]
pub enum Step {
    Construct,
    Begin { input: u64, patching: bool },
    End { patching: bool },
}

/// Disk-changing kernel calls, including durable replace's internal steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Create,
    Write,
    Sync,
    Rename,
    Remove,
    Truncate,
    Make,
}

fn change(ask: &Ask) -> Option<Change> {
    use skein_io::kernel::OpenHow;
    match ask {
        Ask::Open { how: OpenHow::Create { .. } | OpenHow::CreateNoFollow { .. }, .. } => Some(Change::Create),
        Ask::Write { .. } | Ask::Append { .. } => Some(Change::Write),
        Ask::Sync { .. } => Some(Change::Sync),
        Ask::Rename { .. } => Some(Change::Rename),
        Ask::Remove { .. } => Some(Change::Remove),
        Ask::Truncate { .. } => Some(Change::Truncate),
        Ask::MakeDirectory { .. } => Some(Change::Make),
        Ask::Lock { .. }
        | Ask::Spawn { .. }
        | Ask::Open { .. }
        | Ask::Read { .. }
        | Ask::Stat { .. }
        | Ask::List { .. }
        | Ask::Close { .. } => None,
    }
}

// Incoming file buffers were allocated by io, outside a measured store
// call. Transfer their heap into the observation before the store takes it.
fn file_heap(event: &file::Event) -> u64 {
    let bytes = match event {
        file::Event::Loaded { bytes, .. } | file::Event::Read { bytes, .. } => bytes.len(),
        file::Event::Listed { entries, .. } | file::Event::Scanned { entries, .. } => {
            size_of_val(entries.as_ref()) + entries.iter().map(|entry| entry.name.len()).sum::<usize>()
        }
        file::Event::Locked { .. }
        | file::Event::Busy { .. }
        | file::Event::Made { .. }
        | file::Event::Refused { .. }
        | file::Event::Opened { .. }
        | file::Event::Stored { .. }
        | file::Event::Conflict { .. }
        | file::Event::TooLarge { .. }
        | file::Event::Stated { .. }
        | file::Event::Written { .. }
        | file::Event::Cut { .. }
        | file::Event::Synced { .. }
        | file::Event::Closed { .. }
        | file::Event::Renamed { .. }
        | file::Event::Removed { .. }
        | file::Event::Failed { .. }
        | file::Event::Cancelled { .. } => 0,
    };
    u64::try_from(bytes).expect("input heap fits")
}

fn drivers(root: skein_io::kernel::Fd, limits: &jig_protocol_store::Limits) -> [FileIo; 3] {
    std::array::from_fn(|_| {
        let whole = limits.store.chunk.max(jig_protocol_store::sizes::MANIFEST_BYTES);
        jig_protocol_store::sizes::whole(whole).expect("world drivers hold the largest manifest");
        let mut io =
            FileIo::with_whole_limit(8, limits.store.chunk, 32, whole, limits.store.deadline).with_effective_user(0);
        assert_eq!(io.adopt_root(root), Some(Token::new(1)));
        io
    })
}

fn below() -> Below {
    Below {
        store: Queue::with_capacity(QUEUE),
        secrets: Queue::with_capacity(QUEUE),
        transcripts: Queue::with_capacity(QUEUE),
    }
}

impl World {
    #[must_use]
    pub fn new(seed: u64) -> World {
        Self::with_joins(seed, root::limits().joins)
    }

    #[must_use]
    pub fn with_joins(seed: u64, joins: u32) -> World {
        let mut world = Self::unopened_with_joins(seed, joins);
        assert_eq!(world.open(), Some(0));
        world
    }

    #[must_use]
    pub fn unopened(seed: u64) -> World {
        Self::unopened_with_joins(seed, root::limits().joins)
    }

    fn unopened_with_joins(seed: u64, joins: u32) -> World {
        let mut limits = root::limits();
        limits.joins = joins;
        Self::unopened_observed(seed, limits, None)
    }

    #[must_use]
    pub fn unopened_observed(seed: u64, limits: jig_protocol_store::Limits, observer: Option<fn(Step)>) -> World {
        let mut machine = Machine::for_user(0);
        let handle = machine.lay(&[]);
        let mut sim = Sim::new(seed, Config::calm());
        let pid = sim.spawn_process();
        let fd = sim.root(pid, Handle::new(handle.raw()));
        let mut io = drivers(fd, &limits);
        let [store, secrets, transcripts] = &mut io;
        let seeded = jig_protocol_store::seed([store, secrets, transcripts], seed);
        if let Some(observe) = observer {
            observe(Step::Construct);
        }
        let component = Store::on_directory(&limits, root::APPLICATION, &[], seeded);
        if let Some(observe) = observer {
            observe(Step::End { patching: jig_protocol_store::sizes::is_patching(&component) });
        }
        World {
            observer,
            sets: PatchSets::NONE,
            patched: Vec::new(),
            refusal: None,
            kernel_cuts: false,
            changes: Vec::new(),
            written: Vec::new(),
            halted: false,
            machine,
            sim,
            pid,
            root: Root { token: Token::new(1), fd },
            io,
            store: component,
            env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits },
            above: Queue::with_capacity(QUEUE),
            below: below(),
            subs: Queue::with_capacity(QUEUE),
            completions: Queue::with_capacity(QUEUE),
            cut: None,
            operations: 0,
            starts: 0,
            transcript_sync: None,
            next: 1,
            model: Model::default(),
            drawn: [1; 16],
            installation: [0; 16],
            pending: false,
        }
    }

    /// Consume a settled world to inspect the component's durable rows.
    #[must_use]
    pub fn into_stores(self) -> jig_protocol_store::Stores {
        self.store.into_stores()
    }

    pub fn patch_sets(&mut self, sets: PatchSets) {
        self.sets = sets;
        self.store.patch_sets(sets);
    }

    #[must_use]
    pub fn patched(&self) -> &[(Owner, u32, Counts)] {
        &self.patched
    }

    #[must_use]
    pub const fn refusal(&self) -> Option<&OpenRefusal> {
        self.refusal.as_ref()
    }

    /// Count and cut kernel calls, exposing writes inside durable replace.
    pub fn trace_writes(&mut self) {
        self.kernel_cuts = true;
    }

    #[must_use]
    pub fn changes(&self) -> &[(u32, Change)] {
        &self.changes
    }

    fn observe(&self, step: Step) {
        if let Some(observe) = self.observer {
            observe(step);
        }
    }

    /// Exact bytes of traced kernel writes, for selecting commits by their records.
    #[must_use]
    pub fn written(&self) -> &[(u32, Box<[u8]>)] {
        &self.written
    }

    pub fn cut_at(&mut self, operation: u32) {
        self.cut = Some(operation);
    }

    #[must_use]
    pub const fn operation_count(&self) -> u32 {
        self.operations
    }

    #[must_use]
    pub const fn transcript_sync(&self) -> Option<u32> {
        self.transcript_sync
    }

    pub fn commit(&mut self, writes: Box<[Write]>) -> Option<u64> {
        let number = self.next;
        self.model.stage(number, &writes);
        self.operations = 0;
        self.transcript_sync = None;
        let events = self.request(Request::Commit { number, writes });
        let mut acknowledged = None;
        for event in events {
            if let Event::Committed { number: found, .. } = event {
                assert_eq!(found, number);
                self.model.acknowledge(number);
                self.next = number + 1;
                acknowledged = Some(number);
            } else {
                panic!("commit terminal: {event:?}");
            }
        }
        acknowledged
    }

    /// A deployment's changed limits take effect when its next process starts.
    pub fn restart_with_limits(&mut self, seed: u64, limits: jig_protocol_store::Limits) {
        self.env.limits = limits;
        self.crash(seed);
    }

    pub fn crash(&mut self, seed: u64) {
        self.machine.crash(seed);
        let handle = self.machine.reopen_root(0);
        self.sim = Sim::new(seed, Config::calm());
        self.pid = self.sim.spawn_process();
        let fd = self.sim.root(self.pid, Handle::new(handle.raw()));
        self.root = Root { token: Token::new(1), fd };
        self.starts += 1;
        self.io = drivers(fd, &self.env.limits);
        let [store, secrets, transcripts] = &mut self.io;
        let seeded = jig_protocol_store::seed(
            [store, secrets, transcripts],
            seed ^ 0xd1b5_4a32_d192_ed03_u64.wrapping_mul(self.starts),
        );
        self.store = Store::on_directory(&self.env.limits, root::APPLICATION, &[], seeded);
        self.store.patch_sets(self.sets);
        self.patched.clear();
        self.refusal = None;
        self.changes.clear();
        self.written.clear();
        self.halted = false;
        self.above = Queue::with_capacity(QUEUE);
        self.below = below();
        self.subs = Queue::with_capacity(QUEUE);
        self.completions = Queue::with_capacity(QUEUE);
        self.cut = None;
        self.operations = 0;
        self.transcript_sync = None;
    }

    pub fn reopen(&mut self) -> Restored {
        let last = self.open_switched().expect("reopen settles");
        self.next = last + 1;
        let mut restored = Restored {
            last,
            rows: BTreeMap::new(),
            application: self.application(false),
            transcripts: BTreeMap::new(),
        };
        let mut after = None;
        loop {
            let events = self.request(Request::Restore {
                owner: OWNER,
                range: Restore::Jig(core::RestoreRange::LIVE.into()),
                after: after.take(),
                rows: 2,
                bytes: u64::from(self.env.limits.store.page.bytes),
            });
            let [Event::Restored { rows, next, .. }] = events.as_slice() else {
                panic!("restore terminal: {events:?}")
            };
            for row in rows {
                let Row::Jig(record) = row else { panic!("jig restore") };
                assert!(restored.rows.insert(record.key(), record.clone()).is_none(), "no repeated page row");
            }
            after.clone_from(next);
            if after.is_none() {
                break;
            }
        }
        for record in restored.rows.values() {
            if let core::Record::Core(core::CoreRecord::RunProof(proof)) = record {
                let turns = match proof.window {
                    Some(core::CurrentWindow { attempt, turn, .. }) => {
                        self.transcript(TranscriptRead::Window { task: proof.task, attempt, turn, after: None })
                    }
                    None => Vec::new(),
                };
                restored.transcripts.insert(proof.task, turns);
            }
        }
        restored
    }

    pub fn accept_recovery(&mut self, last: u64) {
        self.model.recovered(last);
    }

    pub fn enqueue(&mut self, request: Request) {
        self.observe(Step::Begin { input: 0, patching: jig_protocol_store::sizes::is_patching(&self.store) });
        jig_protocol_store::down(&mut self.store, &self.env, request, &mut self.above, &mut self.below);
        self.observe(Step::End { patching: jig_protocol_store::sizes::is_patching(&self.store) });
    }

    #[must_use]
    pub fn manifest_queued(&self) -> bool {
        self.below
            .transcripts
            .iter()
            .any(|request| matches!(request, file::Request::Store { path, .. } if path.ends_with(b"/manifest")))
    }

    pub fn request(&mut self, request: Request) -> Vec<Event> {
        self.enqueue(request);
        self.settle()
    }

    pub fn settle(&mut self) -> Vec<Event> {
        self.drive();
        let mut events = Vec::new();
        while let Some(event) = self.above.pop() {
            events.push(event);
        }
        events
    }

    /// A request whose serving sites the shell answers at once, each by its
    /// owner's fixture with a fresh context (store.md 8.6); `request` hands
    /// them up instead.
    pub fn answered(&mut self, request: Request) -> Vec<Event> {
        let mut events = Vec::new();
        for event in self.request(request) {
            if !jig_protocol_store::sizes::is_patching(&self.store)
                && let Event::Site { owner, of, key, site, at, patcher } = event
            {
                let mut contexts = Contexts::default();
                let request = answer_site(owner, of, key, site, at, patcher, &mut contexts);
                events.extend(self.answered(request));
            } else {
                events.push(event);
            }
        }
        events
    }

    /// Open and switch directly for store-only stories without an engine root.
    pub fn open_switched(&mut self) -> Option<u64> {
        let last = self.open()?;
        if self.pending {
            // This store-only driver has no engine root to ask for the switch.
            self.switch()?;
        }
        Some(last)
    }

    pub fn switch(&mut self) -> Option<()> {
        let events = self.request(Request::Switch { owner: OWNER });
        match events.as_slice() {
            [Event::Switched { owner: OWNER }] => {
                self.pending = false;
                Some(())
            }
            [Event::OpenRefused { why, .. }] => {
                self.refusal = Some(why.clone());
                None
            }
            [] => None,
            other => panic!("switch terminal: {other:?}"),
        }
    }

    pub fn wires(&mut self, wires: &'static [jig_protocol_store::Wire]) -> bool {
        self.store.wires(wires)
    }

    pub fn open(&mut self) -> Option<u64> {
        self.patched.clear();
        self.refusal = None;
        let mut contexts = Contexts::default();
        let mut events = self.request(Request::Open { owner: OWNER, root: self.root, installation: self.drawn });
        for _ in 0..10_000 {
            let mut answer = None;
            for event in events {
                match event {
                    Event::Site { owner, of, key, site, at, patcher } => {
                        answer = Some(answer_site(owner, of, key, site, at, patcher, &mut contexts));
                    }
                    Event::Patched { of, patch, counts, .. } => self.patched.push((of, patch, counts)),
                    Event::Opened { last, installation, pending, .. } => {
                        self.pending = pending;
                        self.installation = installation;
                        if !self.patched.is_empty() {
                            self.model.patched(last);
                        }
                        self.next = last + 1;
                        return Some(last);
                    }
                    Event::OpenRefused { why, .. } => {
                        self.refusal = Some(why);
                        return None;
                    }
                    other @ (Event::Switched { .. }
                    | Event::Committed { .. }
                    | Event::Failed { .. }
                    | Event::FrameTooLarge { .. }
                    | Event::Restored { .. }
                    | Event::Loaded { .. }
                    | Event::Read { .. }
                    | Event::Got { .. }
                    | Event::Turns { .. }
                    | Event::Unread { .. }
                    | Event::Kept { .. }
                    | Event::Carried { .. }
                    | Event::Stale { .. }
                    | Event::Erased { .. }
                    | Event::Secret { .. }
                    | Event::SecretsLoaded { .. }
                    | Event::SecretsNotReady { .. }
                    | Event::Corrupt { .. }) => panic!("open terminal: {other:?}"),
                }
            }
            if let Some(request) = answer {
                events = self.request(request);
            } else {
                if self.halted || self.cut.is_some_and(|cut| self.operations >= cut) {
                    return None;
                }
                // Patching may have ready in-memory work between file asks.
                // Keep firing until its next site, file ask or terminal.
                self.drive();
                events = Vec::new();
                while let Some(event) = self.above.pop() {
                    events.push(event);
                }
            }
        }
        panic!("open did not settle");
    }

    /// All paths, with file bytes, for byte-for-byte refusal checks.
    #[must_use]
    pub fn contents(&mut self) -> BTreeMap<Vec<u8>, Option<Vec<u8>>> {
        use skein_fake_machine::{How, Is};
        let paths = self.paths();
        let root = self.machine.reopen_root(0);
        let mut result = BTreeMap::new();
        for path in paths {
            let file = self.machine.open(root, &path, How::ReadNoFollow).expect("listed path");
            let kind = self.machine.stat(file).is;
            let bytes = if kind == Is::Directory {
                None
            } else {
                Some(self.machine.read(file, 0, u32::MAX).expect("file bytes"))
            };
            self.machine.close(file);
            result.insert(path, bytes);
        }
        self.machine.close(root);
        result
    }

    /// Lay durable scenario entries beneath the existing deployment.
    pub fn lay(&mut self, items: &[skein_fake_machine::Item]) {
        use skein_fake_machine::{How, Made};
        let root = self.machine.reopen_root(0);
        for item in items {
            let split = item.path.iter().rposition(|b| *b == b'/');
            let (parent, name) =
                split.map_or((b".".as_slice(), item.path.as_slice()), |at| (&item.path[..at], &item.path[at + 1..]));
            let parent = self.machine.open(root, parent, How::DirectoryNoFollow).expect("laid parent stands");
            match &item.made {
                Made::Directory => {
                    self.machine.make_directory(parent, name, item.mode).expect("make scenario directory");
                }
                Made::File(bytes) => {
                    let file = self
                        .machine
                        .open(parent, name, How::CreateNoFollow { mode: item.mode })
                        .expect("make scenario file");
                    self.machine.write(file, 0, bytes).expect("write scenario file");
                    self.machine.sync(file);
                    self.machine.close(file);
                }
                Made::Link(_) | Made::HardLink(_) | Made::Fifo | Made::Device => {
                    panic!("generation scenarios lay files and directories")
                }
            }
            self.machine.sync(parent);
            self.machine.close(parent);
        }
        self.machine.close(root);
    }

    /// Rewrite any file beneath the root: an operator's rewrite of `active`
    /// (store.md 2, "Strays"), or a test laying kept bytes in place of the store's.
    pub fn rewrite(&mut self, path: &[u8], bytes: &[u8]) {
        use skein_fake_machine::How;
        let root = self.machine.reopen_root(0);
        let split = path.iter().rposition(|b| *b == b'/');
        let (parent, name) = split.map_or((b".".as_slice(), path), |at| (&path[..at], &path[at + 1..]));
        let parent = self.machine.open(root, parent, How::DirectoryNoFollow).expect("rewritten parent stands");
        self.machine.remove(parent, name, false).expect("rewritten file stands");
        self.machine.sync(parent);
        self.machine.close(parent);
        self.machine.close(root);
        self.lay(&[skein_fake_machine::Item::file(path, bytes)]);
    }

    #[must_use]
    pub fn paths(&mut self) -> Vec<Vec<u8>> {
        let root = self.machine.reopen_root(0);
        let mut paths = Vec::new();
        tree(&mut self.machine, root, b"", &mut paths);
        self.machine.close(root);
        paths.sort();
        paths
    }

    #[must_use]
    pub fn active(&mut self) -> Vec<u8> {
        use skein_fake_machine::How;
        let root = self.machine.reopen_root(0);
        let file = self.machine.open(root, b"active", How::ReadNoFollow).expect("active stands");
        let bytes = self.machine.read(file, 0, 21).expect("read active");
        self.machine.close(file);
        self.machine.close(root);
        bytes
    }

    fn drive(&mut self) {
        for _ in 0..10_000 {
            self.observe(Step::Begin { input: 0, patching: jig_protocol_store::sizes::is_patching(&self.store) });
            jig_protocol_store::fire(&mut self.store, &self.env, &mut self.above, &mut self.below);
            self.observe(Step::End { patching: jig_protocol_store::sizes::is_patching(&self.store) });
            if self.halted || self.cut.is_some_and(|cut| self.operations >= cut) {
                return;
            }
            let next = self
                .below
                .store
                .pop()
                .map(|request| (0, request))
                .or_else(|| self.below.secrets.pop().map(|request| (1, request)))
                .or_else(|| self.below.transcripts.pop().map(|request| (2, request)));
            let Some((driver, request)) = next else {
                return;
            };
            let synced = driver == 2 && matches!(&request, file::Request::Sync { .. });
            self.file(driver, request);
            if self.halted {
                return;
            }
            if !self.kernel_cuts {
                self.operations += 1;
            }
            if synced {
                self.transcript_sync = Some(self.operations);
            }
        }
        panic!("store did not quiesce");
    }

    fn file(&mut self, driver: usize, request: file::Request) {
        let io = self.io.get_mut(driver).expect("three drivers");
        assert!(io.takes());
        let mut events = Queue::with_capacity(QUEUE);
        file_layer::down(io, self.env.now, request, &mut events, &mut self.subs);
        for _ in 0..100 {
            if !self.subs.is_empty() {
                self.sim.submit(self.pid, &mut self.subs);
                if self.kernel_cuts {
                    let mut calls = Queue::with_capacity(QUEUE);
                    let mut answers = Queue::with_capacity(QUEUE);
                    self.sim.calls(&mut calls);
                    while let Some(call) = calls.pop() {
                        if let Ask::Write { bytes, .. } | Ask::Append { bytes, .. } = &call.ask {
                            self.written.push((self.operations, bytes.clone()));
                        }
                        if let Some(change) = change(&call.ask) {
                            self.changes.push((self.operations, change));
                        }
                        if self.cut == Some(self.operations) {
                            self.halted = true;
                            return;
                        }
                        self.operations += 1;
                        skein_fake_machine::step(&mut self.machine, call, &mut answers);
                    }
                    self.sim.answer(&mut answers);
                } else {
                    serve(&mut self.machine, &mut self.sim);
                }
            }
            self.sim.reap(self.pid, &mut self.completions);
            while let Some(complete) = self.completions.pop() {
                file_layer::up(io, complete, &mut events, &mut self.subs);
            }
            if io.takes() {
                break;
            }
        }
        assert!(io.takes(), "a calm file operation settles");
        assert!(!events.is_empty(), "one file terminal");
        while let Some(event) = events.pop() {
            let fd = if let file::Event::Opened { file, .. } = &event { io.descriptor(*file) } else { None };
            let input = if self.observer.is_some() { file_heap(&event) } else { 0 };
            let arrived = Arrived { event, fd };
            let event = match driver {
                0 => Io::Store(arrived),
                1 => Io::Secrets(arrived),
                2 => Io::Transcripts(arrived),
                _ => panic!("three drivers"),
            };
            if let Some(observe) = self.observer {
                observe(Step::Begin { input, patching: jig_protocol_store::sizes::is_patching(&self.store) });
            }
            jig_protocol_store::up(&mut self.store, &self.env, event, &mut self.above, &mut self.below);
            if let Some(observe) = self.observer {
                observe(Step::End { patching: jig_protocol_store::sizes::is_patching(&self.store) });
            }
        }
    }

    pub fn application(&mut self, load: bool) -> BTreeMap<Box<[u8]>, Box<[u8]>> {
        let mut result = BTreeMap::new();
        let mut after = None;
        loop {
            let ranges = Box::new([skein_kv::Range { start: Box::new([]), end: None }]);
            let request = if load {
                Request::Load {
                    owner: OWNER,
                    range: Load::Application(ranges),
                    after: after.take(),
                    rows: 2,
                    bytes: u64::from(self.env.limits.store.page.bytes),
                }
            } else {
                Request::Restore {
                    owner: OWNER,
                    range: Restore::Application(ranges),
                    after: after.take(),
                    rows: 2,
                    bytes: u64::from(self.env.limits.store.page.bytes),
                }
            };
            let events = self.request(request);
            let [Event::Restored { rows, next, .. } | Event::Loaded { rows, next, .. }] = events.as_slice() else {
                panic!("application page: {events:?}")
            };
            for row in rows {
                let Row::Application { key, value } = row else { panic!("application row") };
                assert!(result.insert(key.clone(), value.clone()).is_none(), "page boundary has no duplicate");
            }
            after.clone_from(next);
            if after.is_none() {
                break;
            }
        }
        result
    }

    pub fn transcript(&mut self, mut read: TranscriptRead) -> Vec<core::TurnRecord> {
        let mut result = Vec::new();
        loop {
            let events = self.answered(Request::Transcript {
                owner: OWNER,
                read,
                rows: 2,
                bytes: u64::from(self.env.limits.store.page.bytes),
            });
            if let [Event::Unread { .. }] = events.as_slice() {
                return result;
            }
            let [Event::Turns { turns, next, .. }] = events.as_slice() else {
                panic!("transcript terminal: {events:?}")
            };
            result.extend(turns.iter().cloned());
            let Some(after) = next else {
                return result;
            };
            read = match read {
                TranscriptRead::Window { task, attempt, turn, .. } => {
                    TranscriptRead::Window { task, attempt, turn, after: Some(*after) }
                }
                TranscriptRead::Page { task, backwards, .. } => {
                    TranscriptRead::Page { task, backwards, after: Some(*after) }
                }
                TranscriptRead::Turn { .. } => panic!("a single turn has no next page"),
            };
        }
    }

    pub fn get(&mut self, key: core::Key) -> core::Record {
        let events = self.request(Request::Get { owner: OWNER, key: Get::Jig(key) });
        let [Event::Got { row: Some(Row::Jig(row)), .. }] = events.as_slice() else {
            panic!("get terminal: {events:?}")
        };
        row.clone()
    }

    pub fn read_task(&mut self, task: u64) -> Vec<core::Record> {
        let range = core::reads::Range {
            party: 1,
            scope: core::reads::Scope::Task { task },
            after: None,
            backwards: false,
            rows: 2,
            bytes: u64::from(self.env.limits.store.page.bytes),
        };
        let events = self.request(Request::Read { owner: OWNER, range });
        let [Event::Read { result: core::reads::Loaded::Page { rows, next: None }, .. }] = events.as_slice() else {
            panic!("task read terminal: {events:?}")
        };
        rows.iter().map(|row| row.value.clone()).collect()
    }

    /// One secrets' request, its file operations counted from zero.
    /// Finish startup before post-mark fixture checks; gate stories use raw requests.
    pub fn mark_if_needed(&mut self) {
        if !self.store.mark_durable() {
            self.mark().expect("store-only start mark");
        }
    }

    /// The start mark for store-only stories, after their public restore.
    pub fn mark(&mut self) -> Option<u64> {
        self.commit(Box::new([Write::Save(root::header(self.next)), Write::Wires]))
    }

    pub fn secret(&mut self, request: Secrets) -> Vec<Event> {
        self.secrets(vec![request])
    }

    /// Secrets' requests asked together, before any of their files is
    /// written; their operations counted from zero.
    pub fn secrets(&mut self, requests: Vec<Secrets>) -> Vec<Event> {
        self.operations = 0;
        for request in requests {
            jig_protocol_store::down(
                &mut self.store,
                &self.env,
                Request::Secrets { owner: OWNER, request },
                &mut self.above,
                &mut self.below,
            );
        }
        self.drive();
        let mut events = Vec::new();
        while let Some(event) = self.above.pop() {
            events.push(event);
        }
        events
    }
}

fn tree(machine: &mut Machine, root: skein_fake_machine::Opened, path: &[u8], paths: &mut Vec<Vec<u8>>) {
    use skein_fake_machine::{How, Is};
    let directory =
        machine.open(root, if path.is_empty() { b"." } else { path }, How::DirectoryNoFollow).expect("tree directory");
    loop {
        let entries = machine.list(directory, 32, 8192).expect("tree page");
        if entries.is_empty() {
            break;
        }
        for (kind, name) in entries {
            let name = if path.is_empty() { name.to_vec() } else { [path, b"/", &name].concat() };
            paths.push(name.clone());
            if kind == Is::Directory {
                tree(machine, root, &name, paths);
            }
        }
    }
    machine.close(directory);
}

// Patching retains each owner's context for the open (store.md 10.3).
// Serving makes a new set of defaults at each site (store.md 8.6).
#[derive(Default)]
struct Contexts {
    application: jig_test_patches::patches::Context,
    smith: jig_test_patches_smith::patches::Context,
    jig: jig_test_patches_jig::patches::Context,
}
fn answer_site(
    owner: Token,
    of: Owner,
    key: Box<[u8]>,
    site: skein_codec::patch::Site,
    at: skein_codec::value::Node,
    mut patcher: Box<skein_codec::patch::Patcher>,
    contexts: &mut Contexts,
) -> Request {
    let (old, new) = patcher.parts();
    let result = match of {
        Owner::Application => {
            contexts.application.key = key;
            jig_test_patches::patches::written(site, old, new, at, &mut contexts.application)
        }
        Owner::Smith => {
            contexts.smith.key = key;
            jig_test_patches_smith::patches::written(site, old, new, at, &mut contexts.smith)
        }
        Owner::Jig => jig_test_patches_jig::patches::written(site, old, new, at, &mut contexts.jig),
        Owner::Skein => panic!("no skein fixture site"),
    };
    Request::Site { owner, patcher, result }
}
