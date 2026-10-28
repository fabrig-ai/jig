//! One root decision per input, with store terminals taken before decisions (service.md, 4).
use crate::{End, PartyOutput, route, stop, system::System};
use alloc::boxed::Box;
use jig_domain_core as core;
use jig_test_domain as root;
use skein_lib::{Duration, Env, Queue, Time, Wall};

#[expect(missing_debug_implementations, reason = "protocol account children hold credentials")]
pub struct Engine {
    pub(crate) domain: root::Domain,
    pub(crate) accounts: Option<crate::accounts::Accounts>,
    pub(crate) workers: Option<crate::workers::Workers>,
    pub(crate) clients: Option<crate::clients::Clients>,
    pub(crate) admission: Option<skein_lib::Token>,
    pub(crate) values: crate::values::Values,
    pub(crate) tools: crate::tools::Tools,
    aborted: Option<()>,
    pub(crate) env: Env<root::Limits>,
    pub(crate) store: crate::store::Store,
    pub(crate) parties: Queue<core::Event>,
    pub(crate) outputs: Queue<PartyOutput>,
    pub(crate) events: Queue<root::Event>,
    pub(crate) requests: Queue<root::Request>,
    pub(crate) systems: [System; 2],
    pub(crate) end: Option<End>,
    pub(crate) begin: bool,
    pub(crate) max_out: u32,
    pub(crate) restart_reads: Option<(u16, u32)>,
    pub(crate) drain: Option<stop::Drain>,
    pub(super) blocked: bool,
    held: Queue<root::Event>,
    ready: bool,
    pub(crate) first_periods: skein_lib::List<(u32, u64)>,
    pub(crate) root_steps: u64,
}

pub(crate) struct Children {
    pub(crate) accounts: Option<crate::accounts::Accounts>,
    pub(crate) workers: Option<crate::workers::Workers>,
    pub(crate) clients: Option<crate::clients::Clients>,
}

impl Engine {
    pub(crate) fn new(
        config: root::Config,
        limits: &root::Limits,
        systems: [System; 2],
        children: Children,
        values: crate::values::Values,
        store: crate::store::Store,
    ) -> Self {
        let Children { accounts, workers, clients } = children;
        let max_out = root::max_out(limits);
        let capacity = max_out.checked_mul(2).expect("engine queues fit u32");
        let mut events = Queue::with_capacity(
            capacity
                .checked_mul(route_out(accounts.as_ref(), workers.as_ref(), clients.is_some()))
                .expect("translated event room"),
        );
        if accounts.is_some() || workers.is_some() {
            events.push(root::Event::Credential { number: 1, generation: 1, valid: Duration::from_nanos(u64::MAX) });
        }
        Self {
            domain: root::Domain::new(config, limits),
            accounts,
            workers,
            clients,
            admission: None,
            values,
            tools: crate::tools::Tools::new(&limits.core),
            aborted: None,
            env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits: *limits },
            store,
            parties: Queue::with_capacity(capacity),
            outputs: Queue::with_capacity(capacity),
            events,
            requests: Queue::with_capacity(max_out),
            systems,
            end: None,
            begin: true,
            max_out,
            restart_reads: None,
            drain: None,
            blocked: false,
            held: Queue::uncapped(),
            ready: true,
            first_periods: skein_lib::List::with_capacity(limits.core.authority.projects),
            root_steps: 0,
        }
    }
    pub const fn parties_in(&mut self) -> &mut Queue<core::Event> {
        &mut self.parties
    }
    pub const fn parties_out(&mut self) -> &mut Queue<PartyOutput> {
        &mut self.outputs
    }

    /// A test puts an application credential's later generation here before announcing it.
    pub fn put_application(&mut self, number: u32, generation: u64, value: skein_lib::Secret) {
        self.values.put_application(number, generation, value);
    }

    /// A test puts an account's grant here before announcing it, as the
    /// accounts component would: a key, until a key source comes (JP7).
    pub fn put_account(
        &mut self,
        account: u32,
        generation: u64,
        value: jig_protocol_accounts::table::Value,
        expires: Time,
    ) {
        self.values.accounts.granted(account, generation, value, expires).expect("a grant within the table's bounds");
    }

    pub(crate) fn room(&self) -> bool {
        (match &self.drain {
            Some(drain) => drain.busy.room() >= self.max_out,
            None => true,
        }) && self.store.room()
            && self.outputs.room() >= self.max_out
            && self.events.room()
                >= self
                    .max_out
                    .checked_mul(route_out(self.accounts.as_ref(), self.workers.as_ref(), self.clients.is_some()))
                    .expect("root translated outputs")
            && match &self.workers {
                Some(workers) => workers.batch_room(self.max_out),
                None => true,
            }
            && match &self.clients {
                Some(clients) => clients.room(self.max_out) && clients.commit_room(),
                None => true,
            }
            && match &self.accounts {
                Some(accounts) => accounts.batch_room(self.max_out),
                None => true,
            }
    }
    pub(crate) fn step(&mut self, event: root::Event) {
        self.root_steps = self.root_steps.wrapping_add(1);
        let watcher = match &mut self.clients {
            Some(clients) => clients.root_event(&event),
            None => None,
        };
        if let Err(event) = root::try_step(&mut self.domain, &self.env, event) {
            self.held.push(event);
            self.blocked = true;
        } else if watcher.is_some() {
            self.admission = watcher;
        }
    }

    #[must_use]
    pub fn next_deadline(&self) -> Option<Time> {
        if self.end.is_some() {
            let accounts = match &self.accounts {
                Some(accounts) => accounts.next_deadline(),
                None => None,
            };
            let workers = match &self.workers {
                Some(workers) => workers.next_deadline(),
                None => None,
            };
            let next =
                crate::accounts::earliest(crate::accounts::earliest(accounts, workers), self.store.next_deadline());
            return crate::accounts::earliest(
                next,
                match &self.clients {
                    Some(clients) => clients.next_deadline(),
                    None => None,
                },
            );
        }
        let mut next = crate::accounts::earliest(self.store.next_deadline(), self.values.accounts.next_expiry());
        for (timer, deadline) in self.domain.core_deadlines(self.env.now, self.env.wall) {
            if (self.domain.stopping() && timer.waits_while_stopping()) || (!self.held.is_empty() && timer.new_work()) {
                continue;
            }
            if let Some(deadline) = deadline {
                next = Some(match next {
                    Some(previous) => deadline.min(previous),
                    None => deadline,
                });
            }
        }
        if !self.domain.stopping() {
            for (_, deadline) in self.domain.connector_deadlines() {
                next = wall_deadline(next, deadline, &self.env);
            }
            next = wall_deadline(next, self.domain.effect_deadline(), &self.env);
        }
        if let Some(accounts) = &self.accounts {
            next = crate::accounts::earliest(next, accounts.next_deadline());
        }
        if let Some(workers) = &self.workers {
            next = crate::accounts::earliest(next, workers.next_deadline());
        }
        if let Some(clients) = &self.clients {
            next = crate::accounts::earliest(next, clients.next_deadline());
        }
        next
    }
    #[must_use]
    pub fn work_pending(&self, now: Time) -> bool {
        let components = self.store.work_pending(now)
            || (self.store.opened || self.end.is_some())
                && (match &self.accounts {
                    Some(accounts) => accounts.work_pending(now, self.store.component.mark_durable()),
                    None => false,
                } || match &self.workers {
                    Some(workers) => workers.work_pending(now),
                    None => false,
                } || match &self.clients {
                    Some(clients) => clients.work_pending(now),
                    None => false,
                });
        if self.end.is_some() || !self.store.opened || (self.begin && !self.listeners_open()) {
            return components;
        }
        let due = match self.next_deadline() {
            Some(deadline) => deadline <= now,
            None => false,
        };
        components
            || !self.store.above.is_empty()
            || !self.store.routed.is_empty()
            || (self.room()
                && (self.begin
                    || (!self.first_periods.is_empty() && self.domain.ready() && !self.blocked)
                    || !self.parties.is_empty()
                    || ((!self.events.is_empty() || !self.held.is_empty()) && !self.blocked)
                    || (!self.held.is_empty() && self.passing())
                    || self.ready
                    || (due && (!self.blocked || !self.held.is_empty()))))
    }
    fn passing(&self) -> bool {
        match self.events.iter().next() {
            Some(event) => root::class(event) != root::Class::New,
            None => false,
        }
    }

    pub(crate) fn io_room(&self) -> bool {
        self.events.room() >= jig_protocol_engine_workers::MAX_OUT_DOWN
            && match &self.workers {
                Some(workers) => workers.room(),
                None => true,
            }
            && match &self.clients {
                Some(clients) => clients.room(1),
                None => true,
            }
            && match &self.accounts {
                Some(accounts) => accounts.up_room(),
                None => true,
            }
    }
    pub(crate) fn listeners_open(&self) -> bool {
        (match &self.workers {
            Some(workers) => workers.opened(),
            None => true,
        }) && match &self.clients {
            Some(clients) => clients.opened(),
            None => true,
        }
    }
    pub(crate) fn abort_if_ended(&mut self) {
        match &self.end {
            Some(End::Aborted | End::Stopped(_)) => self.abort_components(),
            Some(End::Drained) | None => {}
        }
    }
    pub(crate) fn abort_components(&mut self) {
        if self.aborted.is_none() {
            self.aborted = Some(());
            self.store.close();
            if let Some(accounts) = &mut self.accounts {
                accounts.abort();
            }
            if !self.store.opened {
                self.clients = None;
            }
            if let Some(clients) = &mut self.clients {
                clients.down(jig_protocol_engine_clients::Event::Abort);
            }
            if let Some(workers) = &mut self.workers {
                if !self.store.opened {
                    for _ in 0..workers.io.len() {
                        let _unstarted = workers.io.pop();
                    }
                }
                workers.abort(self.env.now, &mut self.events);
            }
        }
    }
}

fn wall_deadline(next: Option<Time>, deadline: Option<Wall>, env: &Env<root::Limits>) -> Option<Time> {
    let Some(deadline) = deadline else {
        return next;
    };
    let span = Duration::from_nanos(deadline.as_nanos().saturating_sub(env.wall.as_nanos()));
    let time = env.now.saturating_add(span);
    Some(match next {
        Some(previous) => time.min(previous),
        None => time,
    })
}

pub fn iterate(engine: &mut Engine, now: Time, wall: Wall) {
    engine.env.now = now;
    engine.env.wall = wall;
    engine.store.clock(now, wall);
    engine.store.iterate();
    crate::store::answer(engine);
    if !engine.store.opened && engine.end.is_none() {
        return;
    }
    if let Some(accounts) = &mut engine.accounts {
        accounts.iterate(now, wall, &mut engine.values, &mut engine.events, &mut engine.store);
    }
    if let Some(workers) = &mut engine.workers {
        workers.iterate(now, &mut engine.events);
    }
    if let Some(clients) = &mut engine.clients {
        clients.starting = if engine.begin { Some(()) } else { None };
        clients.iterate(
            now,
            wall,
            &mut engine.events,
            &engine.env.limits.connector,
            engine.env.limits.core.documents.rows,
            &mut engine.accounts,
        );
    }
    clients_pass(engine);
    stop::advance(engine);
    if engine.end.is_some() {
        engine.abort_if_ended();
        return;
    }
    if !engine.room() {
        return;
    }
    if engine.begin
        && let Some(clients) = &mut engine.clients
    {
        for _ in 0..clients.early.len() {
            if !clients.room(1) {
                break;
            }
            let event = clients.early.pop().expect("client startup input");
            clients.not_ready(event);
        }
    }
    if engine.begin && !engine.listeners_open() {
        return;
    }
    if !engine.begin
        && let Some(clients) = &mut engine.clients
    {
        for _ in 0..clients.early.len() {
            if engine.events.room() == 0 {
                break;
            }
            let event = clients.early.pop().expect("client startup inputs");
            engine.events.push(event);
        }
    }
    if engine.begin {
        engine.begin = false;
        engine.step(root::Event::RestartBegin { pending: engine.store.pending });
    } else if !engine.blocked && !engine.held.is_empty() {
        // A refused retry remains ahead of every younger held remainder.
        let mut younger = ::core::mem::replace(&mut engine.held, Queue::uncapped());
        let event = younger.pop().expect("oldest held component input");
        engine.step(event);
        for _ in 0..younger.len() {
            engine.held.push(younger.pop().expect("younger held component input"));
        }
    } else if !engine.blocked && engine.domain.ready() && !engine.first_periods.is_empty() {
        let last = engine.first_periods.len().checked_sub(1).expect("configured first period");
        let (project, budget) = *engine.first_periods.get(last).expect("configured first period");
        engine.first_periods.truncate(last);
        engine.step(root::Event::Core(core::Event::Period { project, period: 1, budget, next: None }));
    } else if engine.held.is_empty() && !engine.blocked && !engine.events.is_empty() {
        let event = engine.events.pop().expect("ready component input");
        engine.step(event);
    } else if let Some(event) = engine.parties.pop() {
        engine.root_steps = engine.root_steps.wrapping_add(1);
        if let Err(event) = root::try_step(&mut engine.domain, &engine.env, root::Event::Core(event)) {
            engine.outputs.push(PartyOutput::Busy(Box::new(event)));
        }
    } else if !engine.blocked || !engine.held.is_empty() {
        fire(engine);
    }
    pass(engine);
    release(engine);
    clients_pass(engine);
}

/// Completions pass held work in arrival order, up to the next new event.
/// A mixed event adds its new remainder behind the held events and ends this walk.
fn pass(engine: &mut Engine) {
    if engine.held.is_empty() {
        return;
    }
    for _ in 0..engine.events.len() {
        if !engine.room() || !engine.passing() {
            break;
        }
        let event = engine.events.pop().expect("passing component input");
        let mixed = root::class(&event) == root::Class::Mixed;
        engine.step(event);
        if mixed {
            break;
        }
    }
}

fn clients_pass(engine: &mut Engine) {
    if let Some(clients) = &mut engine.clients {
        clients.advance(
            &mut engine.events,
            &engine.env.limits.connector,
            engine.env.limits.core.documents.rows,
            &mut engine.accounts,
        );
        if engine.end.is_none() {
            clients.secrets_step(&mut engine.store);
        }
        clients.advance(
            &mut engine.events,
            &engine.env.limits.connector,
            engine.env.limits.core.documents.rows,
            &mut engine.accounts,
        );
    }
}

fn release(engine: &mut Engine) {
    engine.ready = true;
    let mut empty = 0_u32;
    for _ in 0..engine.env.limits.routes {
        if !engine.room() || engine.end.is_some() {
            break;
        }
        root::release(&mut engine.domain, &engine.env, &mut engine.requests);
        if !engine.requests.is_empty() {
            engine.root_steps = engine.root_steps.wrapping_add(1);
        }
        if !engine.requests.is_empty() || engine.domain.quiescent() {
            engine.blocked = false;
        }
        if engine.requests.is_empty() {
            empty = empty.checked_add(1).expect("two empty passes");
        } else {
            empty = 0;
        }
        for _ in 0..engine.requests.len() {
            let request = engine.requests.pop().expect("bounded root output");
            route::request(engine, request);
            if engine.end.is_some() {
                break;
            }
        }
        if empty == 2 {
            engine.ready = false;
            break;
        }
    }
    if let Some(watcher) = engine.admission.take()
        && let Some(clients) = &mut engine.clients
    {
        clients.down(jig_protocol_engine_clients::Event::Admitted { watcher });
    }
    engine.domain.reclaim();
    stop::advance(engine);
}

fn fire(engine: &mut Engine) {
    for (timer, deadline) in engine.domain.core_deadlines(engine.env.now, engine.env.wall) {
        if (engine.domain.stopping() && timer.waits_while_stopping()) || (!engine.held.is_empty() && timer.new_work()) {
            continue;
        }
        if let Some(deadline) = deadline
            && deadline <= engine.env.now
        {
            engine.step(root::Event::Timer(timer));
            return;
        }
    }
    if engine.domain.stopping() {
        return;
    }
    for (number, deadline) in engine.domain.connector_deadlines() {
        if let Some(deadline) = deadline
            && deadline <= engine.env.wall
        {
            engine.step(root::Event::ConnectorTimer { number });
            return;
        }
    }
    if let Some(deadline) = engine.domain.effect_deadline()
        && deadline <= engine.env.wall
    {
        engine.step(root::Event::Core(core::Event::EffectDeadline));
    }
}

fn route_out(
    accounts: Option<&crate::accounts::Accounts>,
    workers: Option<&crate::workers::Workers>,
    clients: bool,
) -> u32 {
    let mut count = 1;
    if workers.is_some() {
        count = count.max(jig_protocol_engine_workers::MAX_OUT_DOWN);
    }
    if accounts.is_some() {
        count = count.max(skein_oauth_accounts::MAX_OUT_DOWN.above);
    }
    if clients {
        count = count.max(jig_protocol_engine_clients::MAX_OUT_DOWN);
    }
    count
}
