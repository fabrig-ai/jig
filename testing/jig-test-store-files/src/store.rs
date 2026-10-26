//! The component behind the domain worlds' durable store face.
use crate::Files;
use jig_protocol_store::{self as protocol, Below, Event, Io, Limits, Request, Stores};
use jig_test_domain as root;
use jig_test_store::{StoreOut, Up, route};
use skein_lib::{Duration, Env, Queue, Time, Token, Wall};
use std::collections::{BTreeMap, VecDeque};
use std::fmt;

/// The installation's identifier these worlds draw: an empty store keeps it,
/// and a reopened one answers the one it kept (configuration.md, 8).
const INSTALLATION: [u8; 16] = [0; 16];

/// Faults between the root and the component.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    Hold { commits: u32 },
    Fail { commit: u64 },
    SlowPages { by: u32 },
}
/// A commit held before it reaches the component.
#[derive(Clone, Debug)]
pub struct Pending {
    pub number: u64,
    pub writes: Box<[root::Write]>,
    pub delay: u32,
}
type TurnText = fn(&[u8]) -> Option<Box<[u8]>>;

struct Delayed {
    event: Event,
    delay: u32,
}
/// The real component, with a typed mirror of its durable writes.
pub struct Store {
    pub applied: u64,
    pub rows: BTreeMap<root::Key, root::Record>,
    pub pending: VecDeque<Pending>,
    pub held: VecDeque<u64>,
    component: Option<protocol::Store>,
    sets: protocol::PatchSets,
    initial_sets: protocol::PatchSets,
    patches: Vec<(usize, protocol::PatchSets)>,
    /// Patch progress events received during the latest open.
    patched: u32,
    turn_text: Option<TurnText>,
    files: Files,
    disk: Option<crate::disk::Disk>,
    routes: route::Routes,
    limits: root::Limits,
    component_limits: Limits,
    /// The configured models the component names a role's models by.
    models: Box<[jig_domain_core::Model]>,
    inflight: BTreeMap<u64, Box<[root::Write]>>,
    durable: Vec<(u64, Box<[root::Write]>)>,
    replies: Vec<Up>,
    delayed: VecDeque<Delayed>,
    hold_next: u32,
    holding: bool,
    fail_at: Option<u64>,
    page_delay: u32,
    pages_paused: bool,
    failed: bool,
}
impl fmt::Debug for Store {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Store")
            .field("applied", &self.applied)
            .field("rows", &self.rows)
            .field("pending", &self.pending)
            .field("held", &self.held)
            .finish_non_exhaustive()
    }
}
impl Store {
    /// The component under a configuration's `models`, in its order.
    #[must_use]
    pub fn new(limits: &root::Limits, models: &[jig_domain_core::Model]) -> Store {
        Self::with_patch_sets(limits, models, protocol::PatchSets::NONE)
    }
    /// Hand the binary's sets to the component before its first open.
    #[must_use]
    pub fn with_patch_sets(
        limits: &root::Limits,
        models: &[jig_domain_core::Model],
        sets: protocol::PatchSets,
    ) -> Store {
        let component_limits = component_limits(limits);
        let disk = if sets.sets.is_empty() { None } else { Some(crate::disk::Disk::new(&component_limits)) };
        let application = jig_test_store::application(&limits.connector);
        let mut component = if let Some(disk) = &disk {
            protocol::Store::on_directory(&component_limits, application, models, disk.seeded())
        } else {
            let stores = Stores {
                store: skein_kv::Store::memory(&component_limits.store),
                secrets: skein_kv::Store::memory(&component_limits.secrets),
            };
            protocol::Store::over(&component_limits, application, models, stores)
        };
        component.patch_sets(sets);
        let mut routes = route::Routes::new(limits.routes);
        routes.configure(limits);
        let mut store = Store {
            applied: 0,
            rows: BTreeMap::new(),
            pending: VecDeque::new(),
            held: VecDeque::new(),
            component: Some(component),
            sets,
            initial_sets: sets,
            patches: Vec::new(),
            patched: 0,
            turn_text: None,
            files: Files::new(),
            disk,
            routes,
            limits: *limits,
            component_limits,
            models: models.into(),
            inflight: BTreeMap::new(),
            durable: Vec::new(),
            replies: Vec::new(),
            delayed: VecDeque::new(),
            hold_next: 0,
            holding: false,
            fail_at: None,
            page_delay: 0,
            pages_paused: false,
            failed: false,
        };
        store.open();
        store
    }
    /// A fixture binary's host renderer, applied to the component's read bytes.
    /// The ordinary shell uses jig's smith renderer in the store route.
    pub fn transcript_text(&mut self, render: TurnText) {
        self.turn_text = Some(render);
    }
    /// Select the next binary's sets; they take effect on the next open.
    pub fn patch_sets(&mut self, sets: protocol::PatchSets) {
        assert!(self.disk.is_some() || sets.sets.is_empty(), "construct with sets to use a directory store");
        self.sets = sets;
    }
    /// Make a real transcript read unread without altering its typed mirror.
    pub fn unread_transcript(&mut self, task: u64) {
        match &mut self.disk {
            Some(disk) => disk.unread_transcript(task),
            None => self.files.unread_transcript(task),
        }
    }
    pub fn fault(&mut self, fault: Fault) {
        match fault {
            Fault::Hold { commits } => self.hold_next = commits,
            Fault::Fail { commit } => self.fail_at = Some(commit),
            Fault::SlowPages { by } => self.page_delay = by,
        }
    }
    pub fn submit(&mut self, number: u64, writes: Box<[root::Write]>, delay: u32) {
        assert!(!self.failed, "a failed world must restart before writing");
        let previous = self.pending.back().map_or(self.applied, |pending| pending.number);
        assert_eq!(number, previous.checked_add(1).expect("bounded commit number"));
        self.pending.push_back(Pending { number, writes, delay });
    }
    /// Release one commit, or hold its durable acknowledgement as the fault asks.
    ///
    /// # Errors
    /// Returns the selected or component-refused commit's number.
    pub fn tick(&mut self, hold: bool) -> Result<Option<u64>, u64> {
        self.tick_pages();
        if self.failed {
            return Ok(None);
        }
        let Some(front) = self.pending.front_mut() else {
            return Ok(None);
        };
        if front.delay != 0 {
            front.delay -= 1;
            return Ok(None);
        }
        let pending = self.pending.pop_front().expect("checked front");
        if self.fail_at == Some(pending.number) {
            self.failed = true;
            self.pending.clear();
            return Err(pending.number);
        }
        self.holding = hold || self.hold_next != 0 || !self.held.is_empty();
        self.hold_next = self.hold_next.saturating_sub(1);
        self.inflight.insert(pending.number, pending.writes.clone());
        self.request(StoreOut::Commit { number: pending.number, writes: pending.writes });
        if self.failed {
            return Err(pending.number);
        }
        assert_eq!(self.applied, pending.number, "the synchronous stand-in settles the commit");
        if self.holding { Ok(None) } else { Ok(Some(pending.number)) }
    }
    pub fn release_held(&mut self) -> Option<u64> {
        if self.failed { None } else { self.held.pop_front() }
    }
    /// Crash and reopen under a configuration whose models may differ from the last.
    pub fn reconfigure(&mut self, seed: u64, models: &[jig_domain_core::Model]) {
        self.models = models.into();
        self.crash(seed);
    }
    pub fn crash(&mut self, seed: u64) {
        let component = self.component.take().expect("component exists");
        let application = jig_test_store::application(&self.limits.connector);
        self.component = Some(if let Some(disk) = &mut self.disk {
            drop(component);
            disk.crash(&self.component_limits, seed);
            protocol::Store::on_directory(&self.component_limits, application, &self.models, disk.seeded())
        } else {
            let stores = component.into_stores();
            self.files.crash(seed);
            protocol::Store::over(&self.component_limits, application, &self.models, stores)
        });
        self.component.as_mut().expect("rebuilt component").patch_sets(self.sets);
        self.routes = route::Routes::new(self.limits.routes);
        self.routes.configure(&self.limits);
        self.pending.clear();
        self.held.clear();
        self.inflight.clear();
        self.replies.clear();
        self.delayed.clear();
        self.holding = false;
        self.failed = false;
        self.hold_next = 0;
        self.fail_at = None;
        self.open();
    }
    /// Replay durable commits into fresh stores and a fresh filesystem.
    #[must_use]
    pub fn fork(&self) -> Store {
        let mut fresh = Store::with_patch_sets(&self.limits, &self.models, self.initial_sets);
        for index in 0..=self.durable.len() {
            for (_, sets) in self.patches.iter().filter(|(at, _)| *at == index) {
                fresh.patch_sets(*sets);
                fresh.crash(0);
            }
            if let Some((number, writes)) = self.durable.get(index) {
                fresh.submit(*number, writes.clone(), 0);
                assert_eq!(fresh.tick(false), Ok(Some(*number)));
            }
        }
        fresh.patch_sets(self.sets);
        fresh.turn_text = self.turn_text;
        fresh.page_delay = self.page_delay;
        fresh
    }
    /// Save fixture rows in one numbered transaction with the deployment header.
    pub fn seed(&mut self, rows: Box<[root::Record]>) {
        assert!(self.pending.is_empty(), "seed only between root commits");
        let key = root::Key::Core(jig_domain_core::Key::Core(jig_domain_core::CoreKey::Deployment));
        let root::Record::Core(jig_domain_core::Record::Core(jig_domain_core::CoreRecord::Deployment(mut header))) =
            self.rows.get(&key).expect("fixture seed follows the first root commit").clone()
        else {
            panic!("the header has its deployment key");
        };
        let number = self.applied.checked_add(1).expect("bounded fixture commit");
        header.commits = number;
        let mut writes: Vec<_> = rows.into_vec().into_iter().map(root::Write::Save).collect();
        writes.push(root::Write::Save(root::Record::Core(jig_domain_core::Record::Core(
            jig_domain_core::CoreRecord::Deployment(header),
        ))));
        self.submit(number, writes.into_boxed_slice(), 0);
        assert_eq!(self.tick(false), Ok(Some(number)), "seed reaches the real component");
    }
    pub fn ask(&mut self, out: StoreOut) {
        match out {
            StoreOut::Commit { number, writes } => self.submit(number, writes, 0),
            out @ (StoreOut::Restart { .. }
            | StoreOut::ReadLoad { .. }
            | StoreOut::NotesLoad { .. }
            | StoreOut::Now(_)) => self.request(out),
        }
    }
    fn request(&mut self, out: StoreOut) {
        let request = route::down(&mut self.routes, out).expect("root has room for its store request");
        self.drive(VecDeque::from([request]), Queue::with_capacity(protocol::MAX_OUT_DOWN));
    }
    pub fn answers(&mut self) -> Vec<Up> {
        let mut replies = std::mem::take(&mut self.replies);
        if let Some(render) = self.turn_text {
            for reply in &mut replies {
                if let Up::Root(root::Event::TranscriptLoaded { rows, .. }) = reply {
                    for (row, text) in rows {
                        *text = render(&row.transcript);
                    }
                }
            }
        }
        replies
    }
    #[must_use]
    pub fn loading(&self) -> bool {
        !self.delayed.is_empty() || !self.routes.idle()
    }
    fn open(&mut self) {
        self.patched = 0;
        let root = match &self.disk {
            Some(disk) => disk.root(),
            None => self.files.root(),
        };
        self.drive(
            VecDeque::from([Request::Open { owner: Token::new(0), root, installation: INSTALLATION }]),
            Queue::with_capacity(protocol::MAX_OUT_DOWN),
        );
        assert!(!self.failed, "durable stores reopen after their files are cut");
        if self.patched > 0 {
            self.refresh_patched_rows();
            self.patches.push((self.durable.len(), self.sets));
        }
    }
    /// Read back the rows patching can change, including the seal's header.
    fn refresh_patched_rows(&mut self) {
        use jig_domain_core::{CoreKey, Key};
        let keys: Vec<_> = self
            .rows
            .keys()
            .filter_map(|key| {
                if let root::Key::Core(key @ Key::Core(CoreKey::RunProof(_) | CoreKey::Deployment)) = key {
                    Some(key.clone())
                } else {
                    None
                }
            })
            .collect();
        for key in keys {
            self.drive(
                VecDeque::from([Request::Get { owner: Token::new(0), key: protocol::Get::Jig(key) }]),
                Queue::with_capacity(protocol::MAX_OUT_DOWN),
            );
        }
        assert!(!self.failed, "patched mirror rows read back through the component");
    }
    fn env(&self) -> Env<Limits> {
        Env { now: Time::ZERO, wall: Wall::EPOCH, limits: self.component_limits }
    }
    fn drive(&mut self, mut requests: VecDeque<Request>, mut above: Queue<Event>) {
        let mut below = Below {
            store: Queue::with_capacity(skein_kv::MAX_OUT.below),
            secrets: Queue::with_capacity(skein_kv::MAX_OUT.below),
            transcripts: Queue::with_capacity(skein_kv::MAX_OUT.below),
        };
        loop {
            if self.disk.is_some() {
                let env = self.env();
                protocol::fire(self.component.as_mut().expect("component exists"), &env, &mut above, &mut below);
            }
            if let Some(event) = above.pop() {
                self.event(event, &mut requests);
                continue;
            }
            if let Some(request) = requests.pop_front() {
                let env = self.env();
                protocol::down(
                    self.component.as_mut().expect("component exists"),
                    &env,
                    request,
                    &mut above,
                    &mut below,
                );
                continue;
            }
            let io = if let Some(request) = below.transcripts.pop() {
                Io::Transcripts(self.file(2, request))
            } else if let Some(request) = below.store.pop() {
                Io::Store(self.file(0, request))
            } else if let Some(request) = below.secrets.pop() {
                Io::Secrets(self.file(1, request))
            } else {
                break;
            };
            let env = self.env();
            protocol::up(self.component.as_mut().expect("component exists"), &env, io, &mut above, &mut below);
        }
    }
    fn file(&mut self, driver: usize, request: skein_io::file::Request) -> protocol::Arrived {
        match &mut self.disk {
            Some(disk) => disk.answer(driver, request),
            None => self.files.answer(request),
        }
    }
    fn event(&mut self, event: Event, requests: &mut VecDeque<Request>) {
        match event {
            Event::Opened { last, pending, owner, .. } => {
                self.applied = last;
                if pending {
                    // B2: the root's switch step asks this.
                    requests.push_back(Request::Switch { owner });
                }
            }
            Event::Switched { .. } => {}
            Event::Committed { number, .. } => {
                let writes = self.inflight.remove(&number).expect("commit belongs to the root");
                for write in &writes {
                    match write {
                        root::Write::CopySettingsRevision(number) => {
                            use jig_domain_core as core;
                            let key = root::Key::Core(core::Key::Core(core::CoreKey::Settings(
                                core::settings::Key::Held(*number),
                            )));
                            let root::Record::Core(core::Record::Core(core::CoreRecord::Settings(
                                core::settings::Record::Revision { revision, .. },
                            ))) = self.rows.get(&key).expect("held revision").clone()
                            else {
                                panic!("revision row")
                            };
                            let row = root::Record::Core(core::Record::Core(core::CoreRecord::Settings(
                                core::settings::Record::Revision { history: true, revision },
                            )));
                            self.rows.insert(row.key(), row);
                        }
                        root::Write::Save(row) => {
                            self.rows.insert(row.key(), row.clone());
                        }
                        root::Write::Erase(key) => {
                            self.rows.remove(key);
                        }
                    }
                }
                self.applied = number;
                self.durable.push((number, writes));
                if self.holding {
                    self.held.push_back(number);
                }
            }
            // Worlds have no printed open progress lines.
            Event::Patched { .. } => self.patched = self.patched.checked_add(1).expect("bounded patch count"),
            Event::Got { owner, row } if owner == Token::new(0) => {
                let Some(protocol::Row::Jig(row)) = row else {
                    panic!("patched jig row remains at its key");
                };
                let row = root::Record::Core(row);
                self.rows.insert(row.key(), row);
            }
            Event::Failed { .. } | Event::FrameTooLarge { .. } => self.failed = true,
            Event::OpenRefused { why, .. } => panic!("world store open refused: {why:?}"),
            event @ (Event::Restored { .. } | Event::Loaded { .. } | Event::Read { .. })
                if self.page_delay != 0 || self.pages_paused =>
            {
                self.delayed.push_back(Delayed { event, delay: self.page_delay });
            }
            event @ Event::Corrupt { .. } => {
                self.failed = true;
                self.route(event, requests);
            }
            // These worlds supply sets without written sites; routing answers NoSite.
            event @ (Event::Site { .. }
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
            | Event::SecretsLoaded { .. }) => self.route(event, requests),
        }
    }
    fn route(&mut self, event: Event, requests: &mut VecDeque<Request>) {
        let mut outputs = Queue::with_capacity(8);
        route::up(&mut self.routes, event, &mut outputs);
        while let Some(output) = outputs.pop() {
            match output {
                Up::Ask(request) => requests.push_back(request),
                output @ (Up::Restore(_) | Up::Root(_)) => self.replies.push(output),
            }
        }
    }
    /// Suspend captured pages without losing their original store flights.
    pub fn pause_pages(&mut self, paused: bool) {
        self.pages_paused = paused;
    }
    fn tick_pages(&mut self) {
        if self.pages_paused {
            return;
        }
        let mut ready = Queue::with_capacity(u32::try_from(self.delayed.len()).expect("bounded flight table"));
        let count = self.delayed.len();
        for _ in 0..count {
            let mut delayed = self.delayed.pop_front().expect("counted pages");
            delayed.delay = delayed.delay.saturating_sub(1);
            if delayed.delay == 0 {
                ready.push(delayed.event);
            } else {
                self.delayed.push_back(delayed);
            }
        }
        // Released pages route directly; continuations alone receive the next delay.
        let mut requests = VecDeque::new();
        while let Some(event) = ready.pop() {
            self.route(event, &mut requests);
        }
        self.drive(requests, Queue::with_capacity(protocol::MAX_OUT_DOWN));
    }
}
/// Derive admission from the caller's actual budgets, never from fixture counts.
#[must_use]
pub fn component_limits(root: &root::Limits) -> Limits {
    let mut kv = skein_kv::Limits {
        key: u32::MAX,
        value: u32::MAX,
        ops: u32::MAX,
        commit: u32::MAX,
        queued: 8,
        queued_bytes: u64::MAX,
        budget: 4 * 1024 * 1024,
        segment: 1,
        snapshot_after: 1,
        chunk: 1,
        page: skein_kv::Page { rows: 2, bytes: u32::MAX },
        deadline: Duration::from_secs(1),
    };
    let sizes =
        protocol::sizes::check(&root.core, &jig_test_store::application(&root.connector), &root.journal, &kv, 1)
            .expect("root's actual journal and map rows fit the store");
    kv.key = sizes.key;
    kv.value = sizes.value;
    kv.ops = sizes.ops;
    kv.commit = sizes.commit;
    kv.queued_bytes = u64::from(sizes.commit) * u64::from(kv.queued);
    kv.segment = u64::from(sizes.commit) * 2;
    kv.snapshot_after = u64::from(sizes.commit);
    kv.chunk = sizes.key.checked_add(sizes.value).and_then(|sum| sum.checked_add(20)).expect("bounded largest row");
    kv.page.bytes = sizes.key.checked_add(sizes.value).expect("one largest row fits");
    protocol::sizes::whole(kv.chunk).expect("the largest row chunk also holds the whole manifest");
    Limits {
        core: root.core,
        store: kv,
        secrets: kv,
        journal: root.journal,
        joins: 128,
        queue: 8,
        files: 8,
        reads: root.core.load_slots.checked_add(root.core.tasks.tasks).expect("bounded readers"),
        orphan_erases: 1,
        held: 2,
    }
}
