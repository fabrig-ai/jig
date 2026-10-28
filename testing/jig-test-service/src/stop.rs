//! A failed commit ends the engine before any later output leaves (service.md, 5).
use crate::{End, Engine, Stop};
use alloc::boxed::Box;
use jig_protocol_config::{Path, Problems, What};
use jig_test_domain as root;
use skein_lib::Queue;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Closing {
    Drain,
    Abort,
}

#[derive(Debug)]
pub(super) struct Drain {
    pub phase: Phase,
    pub busy: Queue<Box<root::Event>>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Phase {
    Stopping,
    Closing { waiting: u32 },
    Drained,
}

pub(super) fn shutdown(engine: &mut Engine) {
    if engine.drain.is_none() {
        engine.drain = Some(Drain { phase: Phase::Stopping, busy: Queue::uncapped() });
        if let Some(workers) = &mut engine.workers {
            workers.down(engine.env.now, jig_protocol_engine_workers::Event::Stop, &mut engine.events);
        }
        if let Some(clients) = &mut engine.clients {
            clients.down(jig_protocol_engine_clients::Event::Stop);
        }
        engine.events.push(root::Event::Stop);
    }
}
pub(super) fn drained(engine: &mut Engine) {
    let Some(drain) = &mut engine.drain else {
        return;
    };
    if drain.phase != Phase::Stopping {
        return;
    }
    drain.phase = Phase::Closing {
        waiting: u32::from(engine.workers.is_some())
            .checked_add(u32::from(engine.accounts.is_some()))
            .expect("two engine components")
            .checked_add(u32::from(engine.clients.is_some()))
            .expect("three engine components"),
    };
    if let Some(workers) = &mut engine.workers {
        workers.close(engine.env.now, &mut engine.events);
    }
    if let Some(clients) = &mut engine.clients {
        clients.down(jig_protocol_engine_clients::Event::Close);
    }
    if engine.accounts.is_some() {
        engine.step(root::Event::Core(jig_domain_core::Event::Account(jig_domain_accounts::Event::Close)));
    }
}
pub(super) fn advance(engine: &mut Engine) {
    let Some(drain) = &mut engine.drain else {
        return;
    };
    match drain.phase {
        Phase::Stopping | Phase::Drained => return,
        Phase::Closing { .. } => {}
    }
    {
        let waiting = u32::from(match &engine.workers {
            Some(workers) => !workers.closed(),
            None => false,
        })
        .checked_add(u32::from(match &engine.accounts {
            Some(accounts) => !accounts.is_empty(),
            None => false,
        }))
        .expect("two engine components")
        .checked_add(u32::from(match &engine.clients {
            Some(clients) => !clients.closed || !clients.io.is_empty(),
            None => false,
        }))
        .expect("three engine components");
        drain.phase = Phase::Closing { waiting };
        if waiting == 0 && engine.store.idle() && engine.events.is_empty() && engine.requests.is_empty() {
            engine.store.close();
            if !engine.store.closed() {
                return;
            }
            drain.phase = Phase::Drained;
            for _ in 0..drain.busy.len() {
                let _event = drain.busy.pop().expect("deferred input count");
            }
            engine.end = Some(End::Drained);
        }
    }
}

impl crate::workers::Workers {
    pub(super) fn listener_closed(&mut self, owner: skein_lib::Token) -> bool {
        for index in 0..self.listeners.len() {
            let (name, listener) = self.listeners.get_mut(index).expect("listener index");
            if *name == owner {
                *listener = None;
                return true;
            }
        }
        false
    }
    pub(super) fn late_listener(&mut self, listener: skein_lib::Token) {
        match self.ending {
            Some(Closing::Abort) => self.io.push(skein_io::Request::Abort { entity: listener }),
            Some(Closing::Drain) => self.io.push(skein_io::Request::Close { entity: listener }),
            None => {}
        }
    }
    pub(super) fn close(&mut self, now: skein_lib::Time, events: &mut Queue<root::Event>) {
        self.ending = Some(Closing::Drain);
        for (_, listener) in &self.listeners {
            if let Some(listener) = listener {
                self.io.push(skein_io::Request::Close { entity: *listener });
            }
        }
        self.down(now, jig_protocol_engine_workers::Event::Close, events);
    }
    pub(super) fn closed(&self) -> bool {
        if !self.component_closed || !self.sockets.is_empty() || !self.io.is_empty() {
            return false;
        }
        for (_, listener) in &self.listeners {
            if listener.is_some() {
                return false;
            }
        }
        true
    }
}

pub(super) fn failed(engine: &mut Engine, number: u64) {
    engine.step(root::Event::Failed { number });
    engine.end = Some(End::Stopped(Stop::CommitFailed { number }));
}
pub(super) fn invalid_restore(engine: &mut Engine) {
    unavailable(engine, b"deployment");
}
pub(super) fn unavailable(engine: &mut Engine, member: &[u8]) {
    let mut problems = Problems::empty();
    problems.push(Path::root().member(member), What::Refers);
    engine.end = Some(End::Stopped(Stop::Start(problems)));
}
