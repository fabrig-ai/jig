//! The reference engine and worker service, composed on one loop (service.md, 7 and 10).
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
extern crate alloc;
#[cfg(test)]
#[global_allocator]
static HEAP: skein_heap::Counting = skein_heap::Counting;
mod accounts;
mod boundary;
mod clients;
pub mod family;
pub mod program_sources;
pub mod server;
mod tools;
mod values;
mod worker;
mod worker_route;
mod workers;
pub use accounts::AccountsRuntime;
pub use worker::{Worker, WorkerRuntime};
mod engine;
pub mod limits;
mod route;
mod startup;
mod stop;
mod store;
pub mod system;
use alloc::boxed::Box;
pub use boundary::{End, Files, PartyOutput, Seeds, Stop};
pub use engine::Engine;
pub use limits::Limits;
use skein_io::{
    self as io, Io,
    kernel::{Complete, Fd, Submit},
};
use skein_lib::{Env, List, Queue, Time, Token, Wall};
pub use startup::{named, startup};

/// No application server is available, or its loop has already started.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refused;

/// The worker's parsed agent values are supplied by its caller.
#[derive(Debug)]
pub struct Agent {
    pub endpoints: smith_protocol_channel::Endpoints,
    pub limits: smith_host_protocol::Limits,
    pub bounds: jig_domain_host::Bounds,
    pub host_limits: smith_host_domain::Limits,
}

#[expect(missing_debug_implementations, reason = "the service owns protocol account credentials")]
pub struct Service {
    started: bool,
    bundle_directory: Option<Box<[u8]>>,
    io: Io,
    io_env: Env<io::Limits>,
    completions: Queue<Complete>,
    submissions: Queue<Submit>,
    raw_submissions: Queue<Submit>,
    kernel: accounts::Kernel,
    told: Queue<io::Event>,
    engine: Option<Engine>,
    workers: List<Worker>,
    ended: Option<End>,
    signals: Option<Token>,
    signals_closing: bool,
    shutdowns: u8,
    bytes: u64,
    links: Queue<Box<[u8]>>,
}
/// Read-only startup evidence for the service's worlds; no private value is exposed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccountStartup {
    pub pending_mark: Option<u64>,
    pub durable: bool,
    pub driver_made: bool,
    pub restores_waiting: u32,
    pub wants_waiting: u32,
    pub granted: bool,
}

impl Service {
    #[must_use]
    pub fn account_startup(&self, source: u32, generation: u64, now: Time) -> Option<AccountStartup> {
        let engine = self.engine.as_ref()?;
        let accounts = engine.accounts.as_ref()?;
        let (driver_made, restores_waiting, wants_waiting) = accounts.startup(source);
        Some(AccountStartup {
            pending_mark: engine.store.component.pending_mark(),
            durable: engine.store.component.mark_durable(),
            driver_made,
            restores_waiting,
            wants_waiting,
            granted: engine.values.accounts.value(source, generation, now).is_some(),
        })
    }

    fn new(engine: Option<Engine>, workers: List<Worker>, limits: io::Limits, bytes: u64, queue: u32) -> Self {
        Self {
            started: false,
            bundle_directory: None,
            io: Io::new(&limits),
            io_env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits },
            completions: Queue::with_capacity(queue),
            submissions: Queue::with_capacity(queue),
            raw_submissions: Queue::with_capacity(queue),
            kernel: accounts::Kernel::new(
                io::operations(&limits)
                    .expect("io operations")
                    .checked_add(8)
                    .expect("file operations")
                    .checked_add(workers.len())
                    .expect("worker roots"),
            ),
            told: Queue::uncapped(),
            engine,
            workers,
            ended: None,
            signals: None,
            signals_closing: false,
            shutdowns: 0,
            bytes,
            links: Queue::with_capacity(queue),
        }
    }
    /// Inject private host-operation resources before the worker's loop starts.
    pub fn configure_program_sources(
        &mut self,
        worker: u32,
        resources: program_sources::Resources,
    ) -> Result<(), program_sources::Resources> {
        if self.started {
            return Err(resources);
        }
        match self.workers.get_mut(worker) {
            Some(worker) => worker.configure_program_sources(resources),
            None => Err(resources),
        }
    }
    /// The optional browser bundle directory, read by the binary before the loop.
    #[must_use]
    pub fn bundle_directory(&self) -> Option<&[u8]> {
        self.bundle_directory.as_deref()
    }
    /// Give the application server its browser bundle before driving the loop.
    pub fn serve(&mut self, bundle: server::Bundle) -> Result<(), Refused> {
        if self.started {
            return Err(Refused);
        }
        let engine = self.engine.as_mut().ok_or(Refused)?;
        let clients = engine.clients.as_mut().ok_or(Refused)?;
        clients.server.serve(bundle);
        Ok(())
    }
    /// Adopt the deployment directory opened by the caller before driving startup.
    pub fn adopt_store(&mut self, root: Fd, effective_user: u32) -> Result<(), Fd> {
        match &mut self.engine {
            Some(engine) => engine.store.adopt(root, effective_user),
            None => Err(root),
        }
    }
    pub fn adopt_signals(&mut self, descriptor: Fd) -> Result<(), Fd> {
        if self.signals.is_some() {
            return Err(descriptor);
        }
        match self.io.adopt_signals_for(descriptor, Token::new(1)) {
            Ok(source) => {
                self.signals = Some(source);
                Ok(())
            }
            Err(descriptor) => Err(descriptor),
        }
    }
    pub const fn completions(&mut self) -> &mut Queue<Complete> {
        &mut self.completions
    }
    pub const fn submissions(&mut self) -> &mut Queue<Submit> {
        &mut self.submissions
    }
    pub const fn links(&mut self) -> &mut Queue<Box<[u8]>> {
        &mut self.links
    }
    pub const fn engine(&mut self) -> Option<&mut Engine> {
        self.engine.as_mut()
    }
    /// Format domain roots alone for the protocol world's isolation check.
    pub fn fmt_roots(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut roots = formatter.debug_list();
        if let Some(engine) = &self.engine {
            roots.entry(&engine.domain);
        }
        for worker in &self.workers {
            roots.entry(&worker.root);
        }
        roots.finish()
    }
    /// Inputs handed to the roots, and passes that took output from them,
    /// so far: a root's state changes only with one, so a check of the
    /// roots' state need run only when it moved.
    #[must_use]
    pub fn root_steps(&self) -> u64 {
        let mut steps = match &self.engine {
            Some(engine) => engine.root_steps,
            None => 0,
        };
        for worker in &self.workers {
            steps = steps.wrapping_add(worker.root_steps);
        }
        steps
    }
    #[must_use]
    pub const fn end(&self) -> Option<&End> {
        match &self.engine {
            Some(engine) => engine.end.as_ref(),
            None => self.ended.as_ref(),
        }
    }
    #[must_use]
    pub const fn worst_case(&self) -> u64 {
        self.bytes
    }
    #[must_use]
    pub fn operations(&self) -> u32 {
        io::operations(&self.io_env.limits)
            .expect("checked io operations")
            .checked_add(8)
            .expect("file operations")
            .checked_add(self.workers.len())
            .expect("worker roots")
    }
    #[must_use]
    pub fn next_policy_deadline(&self) -> Option<Time> {
        self.policy_deadline()
    }
    #[must_use]
    pub fn next_deadline(&self) -> Option<Time> {
        match self.io.next_deadline() {
            Some(deadline) => Some(match self.policy_deadline() {
                Some(engine) => deadline.min(engine),
                None => deadline,
            }),
            None => self.policy_deadline(),
        }
    }
    #[must_use]
    pub fn work_pending(&self, now: Time) -> bool {
        self.io.is_ready()
            || self.io.is_due(now)
            || !self.completions.is_empty()
            || !self.told.is_empty()
            || !self.raw_submissions.is_empty()
            || match &self.engine {
                Some(engine) => engine.work_pending(now),
                None => false,
            }
            || self.workers_pending(now)
            || (self.end().is_some() && self.signals.is_some() && !self.signals_closing)
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.end().is_some()
            && self.workers_empty()
            && self.io.is_empty()
            && self.completions.is_empty()
            && self.submissions.is_empty()
            && self.told.is_empty()
            && self.raw_submissions.is_empty()
            && self.kernel.is_empty()
            && match &self.engine {
                Some(engine) if !engine.store.closed() => false,
                Some(engine) => match &engine.clients {
                    Some(clients) => clients.closed && clients.io.is_empty(),
                    None => true,
                },
                None => true,
            }
            && match &self.engine {
                Some(engine) => match &engine.accounts {
                    Some(accounts) => accounts.is_empty(),
                    None => true,
                },
                None => true,
            }
    }
    pub fn shutdown(&mut self) {
        if self.shutdowns == 0 {
            self.shutdowns = 1;
            if let Some(engine) = &mut self.engine {
                stop::shutdown(engine);
            }
            if self.engine.is_none() {
                for index in 0..self.workers.len() {
                    self.workers.get_mut(index).expect("worker index").stop();
                }
            }
        } else {
            self.ended = Some(End::Aborted);
            if let Some(engine) = &mut self.engine {
                engine.end = Some(End::Aborted);
            }
            for index in 0..self.workers.len() {
                let worker = self.workers.get_mut(index).expect("worker index");
                worker.abort();
            }
        }
    }
    fn root_complete(&mut self, complete: &Complete) -> bool {
        for index in 0..self.workers.len() {
            if self.workers.get_mut(index).expect("worker index").root_complete(complete) {
                return true;
            }
        }
        false
    }
    fn complete_record(&mut self, complete: Complete) {
        if self.root_complete(&complete) {
            return;
        }
        if store::owns(complete.op) {
            self.engine.as_mut().expect("store engine").store.complete(complete);
        } else {
            io::up(&mut self.io, &self.io_env, complete, &mut self.told, &mut self.raw_submissions);
        }
    }
    fn clock(&mut self, now: Time, wall: Wall) {
        self.io_env.now = now;
        self.io_env.wall = wall;
        if let Some(engine) = &mut self.engine
            && let Some(clients) = &mut engine.clients
            && let Err(problems) = clients.start(now, wall)
        {
            engine.end = Some(End::Stopped(Stop::Start(problems)));
            engine.clients = None;
        }
        if let Some(engine) = &mut self.engine
            && let Some(accounts) = &mut engine.accounts
        {
            accounts.clock(now, wall);
        }
    }
    fn route_components(&mut self) {
        if let Some(engine) = &mut self.engine {
            for _ in 0..engine.store.submitted.len() {
                if self.raw_submissions.room() == 0 {
                    break;
                }
                self.raw_submissions.push(engine.store.submitted.pop().expect("store submissions"));
            }
        }
        if let Some(engine) = &mut self.engine
            && engine.store.opened
            && let Some(clients) = &mut engine.clients
        {
            for _ in 0..clients.links.len() {
                if self.links.room() == 0 {
                    break;
                }
                let link = clients.links.pop().expect("client link count");
                self.links.push(link);
            }
            for _ in 0..clients.io.len() {
                if self.raw_submissions.room() < io::MAX_OUT_DOWN.submissions || !self.io.takes() {
                    break;
                }
                let request = clients.io.pop().expect("client io requests");
                io::down(&mut self.io, &self.io_env, request, &mut self.raw_submissions);
            }
        }
        if let Some(engine) = &mut self.engine
            && engine.store.opened
            && let Some(workers) = &mut engine.workers
        {
            for _ in 0..workers.io.len() {
                if self.raw_submissions.room() < io::MAX_OUT_DOWN.submissions {
                    break;
                }
                let request = workers.io.pop().expect("worker io requests");
                io::down(&mut self.io, &self.io_env, request, &mut self.raw_submissions);
            }
        }
        if let Some(engine) = &mut self.engine
            && engine.store.opened
            && let Some(accounts) = &mut engine.accounts
        {
            for _ in 0..accounts.io.len() {
                if self.raw_submissions.room() < io::MAX_OUT_DOWN.submissions {
                    break;
                }
                let request = accounts::tag(accounts.io.pop().expect("account io requests"));
                io::down(&mut self.io, &self.io_env, request, &mut self.raw_submissions);
            }
        }
        for index in 0..self.workers.len() {
            let worker = self.workers.get_mut(index).expect("worker index");
            for _ in 0..worker.io.len() {
                if self.raw_submissions.room() < io::MAX_OUT_DOWN.submissions || !self.io.takes() {
                    break;
                }
                io::down(
                    &mut self.io,
                    &self.io_env,
                    worker.io.pop().expect("worker io request"),
                    &mut self.raw_submissions,
                );
            }
        }
    }
    fn policy_deadline(&self) -> Option<Time> {
        let mut next = match &self.engine {
            Some(engine) => engine.next_deadline(),
            None => None,
        };
        for worker in &self.workers {
            if let Some(at) = worker.next_deadline() {
                next = Some(match next {
                    Some(next) => next.min(at),
                    None => at,
                });
            }
        }
        next
    }
    fn workers_pending(&self, now: Time) -> bool {
        for worker in &self.workers {
            let ready = match &self.engine {
                Some(engine) if !engine.store.opened => false,
                Some(engine) => match &engine.workers {
                    Some(workers) => workers.opened(),
                    None => true,
                },
                None => true,
            };
            if ready && worker.work_pending(now) {
                return true;
            }
        }
        false
    }
    fn workers_empty(&self) -> bool {
        for worker in &self.workers {
            if !worker.is_empty() {
                return false;
            }
        }
        true
    }
    fn io_room(&self) -> bool {
        if let Some(engine) = &self.engine
            && !engine.io_room()
        {
            return false;
        }
        true
    }
    fn close_signals(&mut self) {
        if self.end().is_some()
            && !self.signals_closing
            && self.raw_submissions.room() >= io::MAX_OUT_DOWN.submissions
            && let Some(entity) = self.signals
        {
            self.signals_closing = true;
            io::down(&mut self.io, &self.io_env, io::Request::Close { entity }, &mut self.raw_submissions);
        }
    }
}

pub fn iterate(service: &mut Service, now: Time, wall: Wall) {
    service.started = true;
    service.clock(now, wall);
    for _ in 0..service.io_env.limits.sockets.checked_add(service.io_env.limits.refusals).expect("io entries") {
        if !service.io.is_ready() || service.raw_submissions.room() < io::MAX_OUT_RESUME.submissions {
            break;
        }
        io::resume(&mut service.io, &service.io_env, &mut service.told, &mut service.raw_submissions);
    }
    if !service.io.is_ready() {
        for _ in 0..service.completions.len() {
            if service.raw_submissions.room() < io::MAX_OUT_UP.submissions {
                break;
            }
            let complete = service.completions.pop().expect("completion count");
            let complete = service.kernel.complete(complete, &service.raw_submissions);
            service.complete_record(complete);
        }
        if service.completions.is_empty() {
            for _ in 0..service.io_env.limits.sockets.checked_mul(2).expect("io timers") {
                if !service.io.is_due(now) || service.raw_submissions.room() < io::MAX_OUT_FIRE.submissions {
                    break;
                }
                io::fire(&mut service.io, &service.io_env, &mut service.told, &mut service.raw_submissions);
            }
        }
    }
    route_events(service, now);
    let started = match &service.engine {
        Some(engine) if !engine.store.opened => false,
        Some(engine) => match &engine.workers {
            Some(workers) => workers.opened(),
            None => true,
        },
        None => true,
    };
    if let Some(engine) = &mut service.engine {
        engine::iterate(engine, now, wall);
    }
    for index in 0..service.workers.len() {
        let worker = service.workers.get_mut(index).expect("worker index");
        if match &service.engine {
            Some(engine) => engine.end.is_some(),
            None => false,
        } {
            worker.abort();
        }
        if started {
            worker.iterate(now, wall);
        }
    }
    for index in 0..service.workers.len() {
        let worker = service.workers.get_mut(index).expect("worker index");
        worker.close_root();
        service.kernel.publish(&mut worker.submitted, &mut service.submissions);
    }
    if service.engine.is_none() && service.ended.is_none() && service.workers_empty() {
        service.ended = Some(End::Drained);
    }
    service.close_signals();
    if let Some(engine) = &mut service.engine {
        engine.abort_if_ended();
    }
    service.route_components();
    service.kernel.publish(&mut service.raw_submissions, &mut service.submissions);
    service.io.reclaim();
}

#[cfg(test)]
mod tests;

fn route_events(service: &mut Service, now: Time) {
    for _ in 0..service.told.len() {
        if !service.io_room() {
            break;
        }
        let event = service.told.pop().expect("io event count");
        if let io::Event::Shutdown { .. } = event {
            service.shutdown();
            continue;
        }
        if workers::owner(&event) == Some(Token::new(1)) {
            if let io::Event::Closed { .. } = event {
                service.signals = None;
            }
            continue;
        }
        let mut worker_index = None;
        for (index, worker) in service.workers.iter().enumerate() {
            if worker.accepts(&event) {
                worker_index = Some(u32::try_from(index).expect("worker index"));
                break;
            }
        }
        if let Some(index) = worker_index {
            service.workers.get_mut(index).expect("worker token owner").up(now, event);
        } else if let Some(engine) = &mut service.engine {
            if match workers::owner(&event) {
                Some(owner) => accounts::owns(owner),
                None => false,
            } {
                if let Some(accounts) = &mut engine.accounts {
                    accounts.up(accounts::untag(event));
                }
            } else if match workers::owner(&event) {
                Some(owner) => clients::owns(owner),
                None => false,
            } {
                if let Some(clients) = &mut engine.clients
                    && !clients.up(event)
                {
                    engine.end = Some(End::Stopped(Stop::Listener));
                }
            } else if let Some(workers) = &mut engine.workers {
                if !workers.up(now, event, &mut engine.events) {
                    engine.end = Some(End::Stopped(Stop::Listener));
                }
            } else {
                engine.end = Some(End::Stopped(Stop::Listener));
            }
        }
    }
}
