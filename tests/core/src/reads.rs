//! Read receipts observed at the root/store boundary (`contract.md`, section 6).
//! Parties know the last submitted commit and the real store's durable number.
//! They never inspect the core or the journal. Their receipts survive a crash.
use crate::{Store, effects::World};
use jig_domain_core::{self as core, reads};
use jig_test_domain as root;
use skein_lib::{ReplyTo, Token};
use std::collections::{BTreeMap, VecDeque};

/// Independent prerequisite for an admitted read's one answer.
#[derive(Debug)]
pub struct Fence(pub u64);

impl Fence {
    /// Reject a terminal observed before the preceding decision was durable.
    ///
    /// # Errors
    /// Returns an error if the store has not applied the prerequisite.
    pub fn answered(&self, durable: u64) -> Result<(), &'static str> {
        if durable < self.0 { Err("read preceded its durable prerequisite") } else { Ok(()) }
    }

    /// A crash preserves every commit whose state a party already read.
    ///
    /// # Errors
    /// Returns an error if restart lost the party's durable prerequisite.
    pub fn restarted(&self, durable: u64) -> Result<(), &'static str> {
        if durable < self.0 { Err("crash retracted an observed read") } else { Ok(()) }
    }
}

/// Scripted parties observe admissions and hold owner-range store requests.
#[derive(Debug)]
pub struct Parties {
    made: u64,
    awaiting: BTreeMap<u64, Fence>,
    admissions: VecDeque<u64>,
    queued: VecDeque<u64>,
    flights: BTreeMap<u64, u64>,
    pub loads: Vec<(Token, reads::Range)>,
    pub hold_loads: bool,
    pub receipts: Vec<(u64, u64, reads::Answer)>,
    pub application_receipts:
        Vec<(u64, u64, u16, Result<jig_test_connector::ClientDocument, jig_test_connector::DocumentMissing>)>,
}

impl Parties {
    #[must_use]
    pub fn new(made: u64) -> Self {
        Self {
            made,
            awaiting: BTreeMap::new(),
            admissions: VecDeque::new(),
            queued: VecDeque::new(),
            flights: BTreeMap::new(),
            loads: Vec::new(),
            hold_loads: false,
            receipts: Vec::new(),
            application_receipts: Vec::new(),
        }
    }

    pub fn inbound(&mut self, event: &root::Event) {
        match event {
            root::Event::Core(core::Event::Read { .. }) | root::Event::ReadApplication { .. } => {
                let reply = self.queued.pop_front().expect("the independent caller issued this read");
                assert!(self.awaiting.insert(reply, Fence(self.made)).is_none());
                self.admissions.push_back(reply);
            }
            root::Event::Core(core::Event::ReadLoaded { load, .. }) => {
                let reply = self.flights.remove(&load.raw()).expect("one terminal for the observed load");
                self.awaiting.get_mut(&reply).expect("admitted read").0 = self.made;
            }
            root::Event::Core(_)
            | root::Event::Link { .. }
            | root::Event::ResultRead { .. }
            | root::Event::Credential { .. }
            | root::Event::WatchApplication { .. }
            | root::Event::Application { .. }
            | root::Event::Turn { .. }
            | root::Event::Text { .. }
            | root::Event::Answer { .. }
            | root::Event::WriterRead { .. }
            | root::Event::Timer(_)
            | root::Event::Committed { .. }
            | root::Event::Failed { .. }
            | root::Event::Connector { .. }
            | root::Event::Stop
            | root::Event::Switched
            | root::Event::RestartBegin { .. }
            | root::Event::RestartDone(_)
            | root::Event::ConnectorTimer { .. }
            | root::Event::ProjectionEffect { .. }
            | root::Event::TopicCall { .. }
            | root::Event::EffectCall { .. }
            | root::Event::CallAnswered { .. }
            | root::Event::TranscriptLoaded { .. } => {}
        }
    }

    pub fn commit(&mut self, number: u64) {
        self.made = number;
    }

    pub fn replied(&mut self, reply: ReplyTo, answer: reads::Answer, store: &Store) {
        let reply = reply.into_token().raw();
        let fence = self.awaiting.remove(&reply).expect("one answer for the admitted read");
        fence.answered(store.applied).expect("read follows every prior commit");
        if self.admissions.front() == Some(&reply) {
            self.admissions.pop_front();
        }
        self.receipts.push((reply, fence.0, answer));
    }

    pub fn application_replied(
        &mut self,
        reply: ReplyTo,
        number: u16,
        answer: Result<jig_test_connector::ClientDocument, jig_test_connector::DocumentMissing>,
        store: &Store,
    ) {
        let reply = reply.into_token().raw();
        let fence = self.awaiting.remove(&reply).expect("one application read terminal");
        fence.answered(store.applied).expect("application read follows every prior commit");
        if self.admissions.front() == Some(&reply) {
            self.admissions.pop_front();
        }
        self.application_receipts.push((reply, fence.0, number, answer));
    }

    pub fn load(&mut self, load: Token, range: reads::Range, store: &Store) -> Vec<jig_test_store::StoreOut> {
        let reply = self.admissions.pop_front().expect("one owner load for an admitted read");
        self.awaiting
            .get(&reply)
            .expect("admitted read")
            .answered(store.applied)
            .expect("load follows the admission's durable fence");
        assert!(self.flights.insert(load.raw(), reply).is_none());

        self.loads.push((load, range));
        if self.hold_loads { Vec::new() } else { self.complete() }
    }

    pub fn complete(&mut self) -> Vec<jig_test_store::StoreOut> {
        std::mem::take(&mut self.loads)
            .into_iter()
            .map(|(load, range)| jig_test_store::StoreOut::ReadLoad { load, range: Box::new(range) })
            .collect()
    }

    #[must_use]
    pub fn paused(&self) -> bool {
        self.hold_loads && !self.loads.is_empty()
    }

    pub fn restart(&mut self, durable: u64) {
        for (_, prerequisite, _) in &self.receipts {
            Fence(*prerequisite).restarted(durable).expect("a crash preserves every observed read");
        }
        for (_, prerequisite, _, _) in &self.application_receipts {
            Fence(*prerequisite).restarted(durable).expect("crash preserves every observed connector read");
        }
        self.awaiting.clear();
        self.admissions.clear();
        self.queued.clear();
        self.flights.clear();
        self.loads.clear();
        self.made = durable;
    }
}

pub fn ask(world: &mut World, number: u64, read: reads::Read) {
    world.reads.as_mut().expect("read parties").queued.push_back(20_000 + number);
    world.send(root::Event::Core(core::Event::Read {
        reply: ReplyTo::new(Token::new(20_000 + number)),
        sign_in: world.sign_in.expect("signed in"),
        read,
    }));
}

pub fn application(
    world: &mut World,
    number: u64,
    scope: jig_domain_people::ApplicationScope,
    connector: u16,
    subject: jig_test_connector::ClientSubject,
) {
    application_window(
        world,
        number,
        scope,
        connector,
        subject,
        jig_domain_core::documents::Window { rows: 16, after: None },
    );
}

pub fn application_window(
    world: &mut World,
    number: u64,
    scope: jig_domain_people::ApplicationScope,
    connector: u16,
    subject: jig_test_connector::ClientSubject,
    window: jig_domain_core::documents::Window,
) {
    world.reads.as_mut().expect("read parties").queued.push_back(20_000 + number);
    world.send(root::Event::ReadApplication {
        reply: ReplyTo::new(Token::new(20_000 + number)),
        sign_in: world.sign_in.expect("signed in"),
        scope,
        number: connector,
        subject,
        window,
    });
}

#[must_use]
pub fn latest(world: &World) -> &reads::Answer {
    &world.reads.as_ref().expect("read parties").receipts.last().expect("one terminal answer").2
}

pub fn complete(world: &mut World) {
    let requests = world.reads.as_mut().expect("read parties").complete();
    for request in requests {
        world.store.ask(request);
    }
    world.drain();
}
