//! The real store and its service-owned file drivers (service.md, 3–5).
use alloc::boxed::Box;
use jig_protocol_store as protocol;
use jig_test_domain as root;
use jig_test_store::{StoreOut, Up, route};
use skein_io::{
    file,
    file_layer::{self, FileIo},
    kernel::{Complete, Fd, Op, Submit},
};
use skein_lib::{Duration, Env, Map, Queue, Time, Token, Wall};
const ADOPT: Token = Token::new(u64::MAX - 1);
const CLOSE: Token = Token::new(u64::MAX);
const MASK: u64 = (1_u64 << 60) - 1;

#[derive(Debug)]
struct Driver {
    io: FileIo,
    above: Queue<file::Event>,
    handles: Map<Token, ()>,
}
#[derive(Debug)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "generation pending and file-driver adoption are independent of open and close lifecycle"
)]
pub(super) struct Store {
    pub(super) component: protocol::Store,
    env: Env<protocol::Limits>,
    routes: route::Routes,
    pub(super) above: Queue<protocol::Event>,
    pub(super) routed: Queue<Up>,
    pub(super) secrets: Queue<protocol::Event>,
    /// The answers to the accounts' records' loads and writes (store.md 9).
    pub(super) sources: Queue<protocol::Event>,
    below: protocol::Below,
    drivers: [Driver; 3],
    pub(super) submitted: Queue<Submit>,
    pub(super) opened: bool,
    pub(super) pending: bool,
    /// The installation's identifier: the drawn one until the store opens,
    /// then the one it keeps (store.md 4.4).
    pub(super) installation: [u8; 16],
    adopted: bool,
    root: Option<Fd>,
    closing: bool,
    /// The configured models the component names a role's models by, for a test's memory store.
    #[cfg(test)]
    pub(super) models: Box<[jig_domain_core::Model]>,
}

pub(super) fn limits(root: &root::Limits, profile: &protocol::Limits) -> Option<protocol::Limits> {
    let mut kv = profile.store;
    let sizes = protocol::sizes::check(
        &root.core,
        &jig_test_store::application(&root.connector),
        &root.journal,
        &kv,
        profile.orphan_erases,
    )
    .ok()?;
    kv.key = sizes.key;
    kv.value = sizes.value;
    kv.ops = sizes.ops;
    kv.commit = sizes.commit;
    kv.queued_bytes = u64::from(sizes.commit).checked_mul(u64::from(kv.queued))?;
    kv.segment = u64::from(sizes.commit).checked_mul(2)?;
    kv.snapshot_after = u64::from(sizes.commit);
    kv.chunk = sizes.key.checked_add(sizes.value)?.checked_add(20)?;
    kv.page.bytes = sizes.key.checked_add(sizes.value)?;
    let limits = protocol::Limits {
        core: root.core,
        store: kv,
        secrets: kv,
        journal: root.journal,
        joins: root.core.load_slots,
        queue: kv.queued,
        files: profile.files,
        reads: root.core.load_slots.checked_add(root.core.tasks.tasks)?,
        orphan_erases: profile.orphan_erases,
        held: profile.held,
    };
    protocol::sizes::whole(max_whole(&limits)).ok()?;
    check_read_page(&limits)?;
    Some(limits)
}
impl Store {
    pub(super) fn new(
        application: &root::Limits,
        limits: &protocol::Limits,
        models: &[jig_domain_core::Model],
        installation: [u8; 16],
        seed: u64,
    ) -> Self {
        let room = root::max_out(application).checked_mul(2).expect("store route room");
        let mut routes = route::Routes::new(room);
        routes.configure(application);
        let mut drivers = [driver(limits, room), driver(limits, room), driver(limits, room)];
        let [store, secrets, transcripts] = &mut drivers;
        let seeded = protocol::seed([&mut store.io, &mut secrets.io, &mut transcripts.io], seed);
        Self {
            component: protocol::Store::on_directory(
                limits,
                jig_test_store::application(&application.connector),
                models,
                seeded,
            ),
            env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits: *limits },
            routes,
            above: Queue::uncapped(),
            routed: Queue::uncapped(),
            secrets: Queue::uncapped(),
            sources: Queue::uncapped(),
            below: protocol::Below {
                store: Queue::uncapped(),
                secrets: Queue::uncapped(),
                transcripts: Queue::uncapped(),
            },
            drivers,
            submitted: Queue::with_capacity(room),
            opened: false,
            pending: false,
            installation,
            adopted: false,
            root: None,
            closing: false,
            #[cfg(test)]
            models: models.into(),
        }
    }
    pub(super) fn adopt(&mut self, fd: Fd, user: u32) -> Result<(), Fd> {
        if self.adopted {
            return Err(fd);
        }
        self.adopted = true;
        for index in 0..3 {
            let driver = self.drivers.get_mut(index).expect("driver index");
            driver.io = core::mem::replace(&mut driver.io, FileIo::new(1, 1, 1, Duration::from_secs(1)))
                .with_effective_user(user);
            if index == 0 {
                let file = driver.io.adopt_root(fd).expect("deployment root slot");
                driver.handles.insert(file, ()).expect("owned deployment root");
            }
        }
        self.root = Some(fd);
        let clone =
            file::Request::OpenDirectory { owner: ADOPT, root: fd, name: b".".as_slice().into(), no_follow: true };
        self.below.secrets.push(clone);
        self.below.transcripts.push(file::Request::OpenDirectory {
            owner: ADOPT,
            root: fd,
            name: b".".as_slice().into(),
            no_follow: true,
        });
        Ok(())
    }
    pub(super) fn clock(&mut self, now: Time, wall: Wall) {
        self.env.now = now;
        self.env.wall = wall;
    }
    pub(super) fn ask(&mut self, request: protocol::Request) {
        match request {
            protocol::Request::Secrets { owner, request } => {
                if let Some(event) = self.secrets(owner, request) {
                    self.above.push(event);
                }
            }
            request @ (protocol::Request::Switch { .. }
            | protocol::Request::Open { .. }
            | protocol::Request::Site { .. }
            | protocol::Request::Commit { .. }
            | protocol::Request::Restore { .. }
            | protocol::Request::Load { .. }
            | protocol::Request::Read { .. }
            | protocol::Request::Get { .. }
            | protocol::Request::Transcript { .. }) => {
                protocol::down(&mut self.component, &self.env, request, &mut self.above, &mut self.below);
            }
        }
    }
    pub(super) fn down(&mut self, out: StoreOut) -> bool {
        if let Some(request) = route::down(&mut self.routes, out) {
            self.ask(request);
            true
        } else {
            false
        }
    }
    /// Get answers stay in this pass; secret writes' terminals are handed to their owner.
    pub(super) fn secrets(&mut self, owner: Token, request: protocol::Secrets) -> Option<protocol::Event> {
        let immediate = match &request {
            protocol::Secrets::Get { .. } | protocol::Secrets::Load { .. } => true,
            protocol::Secrets::Keep { .. }
            | protocol::Secrets::Adopt { .. }
            | protocol::Secrets::AdoptValue { .. }
            | protocol::Secrets::Replace { .. }
            | protocol::Secrets::Erase { .. }
            | protocol::Secrets::Hold { .. }
            | protocol::Secrets::Carry { .. } => false,
        };
        protocol::down(
            &mut self.component,
            &self.env,
            protocol::Request::Secrets { owner, request },
            &mut self.above,
            &mut self.below,
        );
        let mut answer = None;
        if immediate {
            for _ in 0..self.above.len() {
                let event = self.above.pop().expect("store terminals");
                if let protocol::Event::Secret { owner: found, .. }
                | protocol::Event::SecretsLoaded { owner: found, .. }
                | protocol::Event::SecretsNotReady { owner: found } = &event
                    && *found == owner
                {
                    answer = Some(event);
                } else {
                    self.above.push(event);
                }
            }
        }
        answer
    }
    pub(super) fn next_deadline(&self) -> Option<Time> {
        let mut next = None;
        for driver in &self.drivers {
            next = crate::accounts::earliest(next, driver.io.next_deadline());
        }
        next
    }
    pub(super) fn work_pending(&self, now: Time) -> bool {
        if !self.secrets.is_empty()
            || !self.sources.is_empty()
            || !self.above.is_empty()
            || !self.routed.is_empty()
            || !self.submitted.is_empty()
        {
            return true;
        }
        for index in 0..3 {
            let driver = self.drivers.get(index).expect("driver index");
            if driver.io.is_due(now)
                || !self.drivers.get(index).expect("driver index").above.is_empty()
                || driver.io.takes() && (!self.queue(index).is_empty() || self.can_close(index))
            {
                return true;
            }
        }
        false
    }
    fn can_close(&self, index: usize) -> bool {
        self.closing
            && !self.drivers.get(index).expect("driver index").handles.is_empty()
            && (index != 0
                || self.drivers[1].handles.is_empty()
                    && self.drivers[2].handles.is_empty()
                    && self.drivers[1].io.takes()
                    && self.drivers[2].io.takes()
                    && self.drivers[1].above.is_empty()
                    && self.drivers[2].above.is_empty())
    }
    pub(super) fn closed(&self) -> bool {
        self.closing && self.idle() && empty_handles(&self.drivers)
    }
    pub(super) fn idle(&self) -> bool {
        (self.closing || self.routes.idle())
            && self.secrets.is_empty()
            && self.sources.is_empty()
            && self.above.is_empty()
            && self.routed.is_empty()
            && self.submitted.is_empty()
            && self.below.store.is_empty()
            && self.below.secrets.is_empty()
            && self.below.transcripts.is_empty()
            && idle_drivers(&self.drivers)
    }
    pub(super) fn close(&mut self) {
        self.closing = true;
        self.root = None;
        for _ in 0..self.above.len() {
            let _terminal = self.above.pop();
        }
        for _ in 0..self.routed.len() {
            let _up = self.routed.pop();
        }
        for _ in 0..self.secrets.len() {
            let _terminal = self.secrets.pop();
        }
        for _ in 0..self.sources.len() {
            let _terminal = self.sources.pop();
        }
        for index in 0..3 {
            for _ in 0..self.queue(index).len() {
                let _request = self.pop(index);
            }
        }
    }
    pub(super) fn complete(&mut self, mut complete: Complete) {
        let index = usize::try_from((complete.op.raw() >> 60_u32).checked_sub(8).expect("driver namespace"))
            .expect("three drivers");
        complete.op = Token::new(complete.op.raw() & MASK);
        if let Op::Cancel { target } = &mut complete.kind {
            *target = Token::new(target.raw() & MASK);
        }
        let mut submitted = Queue::with_capacity(4);
        let driver = self.drivers.get_mut(index).expect("driver index");
        file_layer::up(&mut driver.io, complete, &mut driver.above, &mut submitted);
        self.publish(index, &mut submitted);
    }
    fn publish(&mut self, index: usize, queue: &mut Queue<Submit>) {
        let prefix = u64::try_from(index).expect("driver index").checked_add(8).expect("driver namespace") << 60_u32;
        for _ in 0..queue.len() {
            let mut submit = queue.pop().expect("file submissions");
            submit.op = Token::new(submit.op.raw() | prefix);
            if let Op::Cancel { target } = &mut submit.kind {
                *target = Token::new(target.raw() | prefix);
            }
            self.submitted.push(submit);
        }
    }
    fn queue(&self, index: usize) -> &Queue<file::Request> {
        if index == 0 {
            &self.below.store
        } else if index == 1 {
            &self.below.secrets
        } else {
            &self.below.transcripts
        }
    }
    fn pop(&mut self, index: usize) -> Option<file::Request> {
        if index == 0 {
            self.below.store.pop()
        } else if index == 1 {
            self.below.secrets.pop()
        } else {
            self.below.transcripts.pop()
        }
    }
    fn descriptor(&mut self, index: usize, event: &file::Event) -> Option<Fd> {
        if let file::Event::Opened { file, .. } = event {
            self.drivers
                .get_mut(index)
                .expect("driver index")
                .handles
                .insert(*file, ())
                .expect("bounded open file handles");
            return self.drivers.get(index).expect("driver index").io.descriptor(*file);
        }
        None
    }
    fn close_request(&mut self, index: usize) -> Option<file::Request> {
        if self.can_close(index) {
            let handles = &mut self.drivers.get_mut(index).expect("driver index").handles;
            if let Some((token, ())) = handles.iter().last() {
                let file = *token;
                handles.remove(&file);
                return Some(close(file));
            }
        }
        None
    }
    pub(super) fn iterate(&mut self) {
        if self.adopted && !self.closing {
            protocol::fire(&mut self.component, &self.env, &mut self.above, &mut self.below);
        }
        for index in 0..3 {
            let mut submitted = Queue::with_capacity(4);
            file_layer::expire(
                &mut self.drivers.get_mut(index).expect("driver index").io,
                self.env.now,
                &mut submitted,
            );
            for _ in 0..self.drivers.get(index).expect("driver index").above.len() {
                let event = self.drivers.get_mut(index).expect("driver index").above.pop().expect("file terminal");
                if event.owner() == CLOSE {
                    continue;
                }
                let fd = self.descriptor(index, &event);
                if event.owner() == ADOPT {
                    if fd.is_none() {
                        self.root = None;
                        self.above.push(protocol::Event::OpenRefused {
                            owner: Token::new(1),
                            why: protocol::OpenRefusal::Failed,
                        });
                    } else if !self.drivers[1].handles.is_empty()
                        && !self.drivers[2].handles.is_empty()
                        && let Some(fd) = self.root.take()
                    {
                        self.ask(protocol::Request::Open {
                            owner: Token::new(1),
                            root: protocol::Root { token: Token::new(1), fd },
                            installation: self.installation,
                        });
                    }
                    continue;
                }
                let arrived = protocol::Arrived { event, fd };
                let event = if index == 0 {
                    protocol::Io::Store(arrived)
                } else if index == 1 {
                    protocol::Io::Secrets(arrived)
                } else {
                    protocol::Io::Transcripts(arrived)
                };
                if !self.closing {
                    protocol::up(&mut self.component, &self.env, event, &mut self.above, &mut self.below);
                }
            }
            if self.drivers.get(index).expect("driver index").io.takes() {
                let request = self.pop(index);
                if let Some(file::Request::Close { file, .. }) = &request {
                    self.drivers.get_mut(index).expect("driver index").handles.remove(file);
                }
                let request = match request {
                    Some(request) => Some(request),
                    None => self.close_request(index),
                };
                if let Some(request) = request {
                    let driver = self.drivers.get_mut(index).expect("driver index");
                    file_layer::down(&mut driver.io, self.env.now, request, &mut driver.above, &mut submitted);
                }
            }
            self.publish(index, &mut submitted);
        }
    }
}
pub(super) fn owns(token: Token) -> bool {
    (8..11).contains(&(token.raw() >> 60))
}

fn handles(limits: &protocol::Limits) -> u32 {
    limits.files.checked_add(4).expect("store file handles fit")
}
fn max_read(limits: &protocol::Limits) -> u32 {
    limits
        .store
        .chunk
        .max(limits.core.resume_bytes)
        .max(u32::try_from(limits.core.load_bytes.min(u64::from(u32::MAX))).expect("file read length"))
}
fn max_whole(limits: &protocol::Limits) -> u32 {
    max_read(limits).max(protocol::sizes::MANIFEST_BYTES)
}
fn check_read_page(limits: &protocol::Limits) -> Option<()> {
    if max_read(limits) < limits.store.page.bytes {
        return None;
    }
    Some(())
}
fn driver(limits: &protocol::Limits, capacity: u32) -> Driver {
    let io = FileIo::with_whole_limit(handles(limits), max_read(limits), 32, max_whole(limits), limits.store.deadline);
    Driver { io, above: Queue::with_capacity(capacity), handles: Map::with_capacity(handles(limits)) }
}
fn close(file: Token) -> file::Request {
    file::Request::Close { owner: CLOSE, file }
}

pub(super) fn writes(mut queue: Queue<root::Write>) -> Box<[root::Write]> {
    let mut list = skein_lib::List::with_capacity(queue.len());
    for _ in 0..queue.len() {
        list.push(queue.pop().expect("journal write count")).expect("same writes");
    }
    list.into_boxed()
}
pub(super) fn request(engine: &mut crate::Engine, out: StoreOut) {
    if !engine.store.down(out) {
        crate::stop::invalid_restore(engine);
    }
}
pub(super) fn answer(engine: &mut crate::Engine) {
    if engine.end.is_some() {
        for _ in 0..engine.store.above.len() {
            let _late = engine.store.above.pop();
        }
        for _ in 0..engine.store.routed.len() {
            let _late = engine.store.routed.pop();
        }
        return;
    }
    for _ in 0..engine.store.above.len() {
        let event = engine.store.above.pop().expect("store terminals");
        match event {
            protocol::Event::Opened { installation, pending, .. } => {
                engine.store.installation = installation;
                engine.store.pending = pending;
                engine.store.opened = true;
            }
            event @ protocol::Event::Switched { .. } => {
                engine.store.pending = false;
                route::up(&mut engine.store.routes, event, &mut engine.store.routed);
            }
            // This shell prints no per-open progress lines.
            protocol::Event::Patched { .. } => {}
            protocol::Event::OpenRefused { .. } => crate::stop::unavailable(engine, b"deployment"),
            event @ (protocol::Event::SecretsNotReady { owner }
            | protocol::Event::Kept { owner }
            | protocol::Event::Stale { owner }
            | protocol::Event::Erased { owner }
            | protocol::Event::Carried { owner, .. }
            | protocol::Event::Secret { owner, .. }
            | protocol::Event::SecretsLoaded { owner, .. }) => {
                if crate::accounts::owns(owner) {
                    engine.store.sources.push(event);
                } else {
                    engine.store.secrets.push(event);
                }
            }
            // This shell never hands patch sets; routing answers Site with NoSite.
            other @ (protocol::Event::Site { .. }
            | protocol::Event::Committed { .. }
            | protocol::Event::Failed { .. }
            | protocol::Event::FrameTooLarge { .. }
            | protocol::Event::Restored { .. }
            | protocol::Event::Loaded { .. }
            | protocol::Event::Read { .. }
            | protocol::Event::Got { .. }
            | protocol::Event::Turns { .. }
            | protocol::Event::Unread { .. }
            | protocol::Event::Corrupt { .. }) => route::up(&mut engine.store.routes, other, &mut engine.store.routed),
        }
    }
    for _ in 0..engine.store.routed.len() {
        let up = engine.store.routed.pop().expect("store routing");
        match up {
            Up::Root(root::Event::Failed { number }) => crate::stop::failed(engine, number),
            Up::Root(
                event @ (root::Event::RestartDone(_)
                | root::Event::Core(jig_domain_core::Event::People(jig_domain_people::Event::Restored))),
            ) => engine.events.push(event),
            Up::Root(event) => {
                engine.blocked = false;
                engine.step(event);
            }
            Up::Restore(row) => {
                if row.key() == root::Key::Core(jig_domain_core::Key::Core(jig_domain_core::CoreKey::Deployment)) {
                    engine.first_periods.truncate(0);
                }
                engine.root_steps = engine.root_steps.wrapping_add(1);
                if !root::restore_record(&mut engine.domain, &engine.env, row) {
                    crate::stop::invalid_restore(engine);
                }
            }
            Up::Ask(request) => engine.store.ask(request),
        }
        if engine.end.is_some() {
            break;
        }
    }
}

fn empty_handles(drivers: &[Driver; 3]) -> bool {
    for driver in drivers {
        if !driver.handles.is_empty() {
            return false;
        }
    }
    true
}
fn idle_drivers(drivers: &[Driver; 3]) -> bool {
    for driver in drivers {
        if !driver.io.takes() || !driver.above.is_empty() {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod sizing_tests {
    use super::{check_read_page, max_read};
    use jig_protocol_config::Profile;

    #[test]
    fn drivers_read_a_whole_page_in_every_admitted_store() {
        for profile in [Profile::Small, Profile::Standard] {
            let configured = crate::limits::profile_limits(profile);
            let limits = super::limits(&configured.root, &configured.store).expect("admitted profile");
            assert!(max_read(&limits) >= limits.store.page.bytes);
        }
    }

    #[test]
    fn a_store_whose_reads_are_below_its_page_is_refused() {
        let mut limits = crate::limits::profile_limits(Profile::Small).store;
        let page = limits.store.page.bytes;
        let below = page.checked_sub(1).unwrap();
        // Synthetic fields check admission without constructing an invalid KV store.
        limits.store.chunk = below;
        limits.core.resume_bytes = below;
        limits.core.load_bytes = u64::from(below);
        assert_eq!(max_read(&limits), below);
        assert_eq!(check_read_page(&limits), None);
        for (chunk, resume, load) in [(page, below, below), (below, page, below), (below, below, page)] {
            limits.store.chunk = chunk;
            limits.core.resume_bytes = resume;
            limits.core.load_bytes = u64::from(load);
            assert_eq!(max_read(&limits), page);
            assert_eq!(check_read_page(&limits), Some(()));
        }
    }
}
