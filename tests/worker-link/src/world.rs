use crate::{fixtures, net::Net};
use jig_protocol_worker_link::{Component, Event, Limits, MAX_OUT_DOWN, Request};
use skein_fake_channel::ScriptedPeer;
use skein_lib::{Duration, Queue, Rng, Time, Token};
use std::collections::VecDeque;

#[derive(Debug)]
pub enum Trace {
    RelayStarted(skein_lib::Token, skein_lib::Token, Box<[u8]>),
    RelayEnded(skein_lib::Token, skein_lib::Token, Box<[u8]>),
    Busy(skein_lib::Time, skein_lib::Token, skein_lib::Token, u32),
    Kept(skein_lib::Token, skein_lib::Token, u32),
    Acknowledged(skein_lib::Token, skein_lib::Token),
    Resend(skein_lib::Time, skein_lib::Token, skein_lib::Token, u32),
    Ready,
    Lost,
    GracePassed,
    Closed,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum DialState {
    Settled,
    Pending,
}
pub struct World {
    component: Component,
    limits: Limits,
    now: Time,
    out: Queue<Request>,
    pending: VecDeque<Option<ScriptedPeer>>,
    net: Option<Net>,
    connection: Token,
    next: u64,
    dials: Vec<Time>,
    root: Vec<Request>,
    facts: Vec<skein_channel::Closed>,
    rng: Rng,
    silence_next: bool,
    other_version_next: bool,
    refused: Vec<(u16, Box<[u8]>)>,
    observed: Vec<Vec<skein_fake_channel::Observed>>,
    values: Vec<Request>,
    order: Vec<&'static str>,
    trace: Vec<Trace>,
    scripted: bool,
    stalled: u32,
    dial: DialState,
}
impl World {
    #[must_use]
    pub fn new(limits: &Limits, seed: u64) -> Self {
        Self {
            component: Component::new(limits, fixtures::credential(), seed).expect("world limits"),
            limits: *limits,
            now: Time::ZERO,
            out: Queue::with_capacity(MAX_OUT_DOWN),
            pending: VecDeque::new(),
            net: None,
            connection: Token::new(0),
            next: 1,
            dials: Vec::new(),
            root: Vec::new(),
            facts: Vec::new(),
            rng: Rng::new(seed),
            silence_next: false,
            other_version_next: false,
            refused: Vec::new(),
            observed: Vec::new(),
            values: Vec::new(),
            order: Vec::new(),
            trace: Vec::new(),
            scripted: false,
            stalled: 0,
            dial: DialState::Settled,
        }
    }
    pub fn engine(&mut self, peer: ScriptedPeer) {
        self.pending.push_back(Some(peer));
    }
    pub fn refuse_dial(&mut self) {
        self.pending.push_back(None);
    }
    pub fn silence_next_engine(&mut self) {
        self.silence_next = true;
    }
    pub fn change_next_engine_version(&mut self) {
        self.other_version_next = true;
    }
    pub fn down(&mut self, event: Event) {
        if matches!(event, Event::Dialled { .. } | Event::Unreachable) {
            self.dial = DialState::Settled;
        }
        if let Event::Up { up: jig_domain_host::LinkUp::Call { run, attempt, call, .. }, .. } = &event {
            self.trace.push(Trace::RelayStarted(*run, *attempt, call.clone()));
        }
        self.component.down(self.now, event, &mut self.out);
        self.run();
    }
    pub fn lose_channel(&mut self) {
        self.archive();
        self.down(Event::Gone { connection: self.connection });
    }
    pub fn advance(&mut self, span: Duration) {
        let end = self.now.saturating_add(span);
        while let Some(at) = self.component.next_deadline() {
            if at > end {
                break;
            }
            self.now = self.now.max(at);
            self.component.fire(self.now, &mut self.out);
            self.run();
        }
        self.now = end;
    }
    pub fn run(&mut self) {
        for _ in 0..10000 {
            if let Some(at) = self.component.next_deadline()
                && at <= self.now
            {
                self.component.fire(self.now, &mut self.out);
            }
            if let Some(request) = self.out.pop() {
                self.route(request);
                continue;
            }
            if let Some(net) = &mut self.net
                && let Some(event) = net.tick(&mut self.rng)
            {
                self.component.down(self.now, Event::Lower { connection: self.connection, event }, &mut self.out);
                continue;
            }
            return;
        }
        panic!("a world iteration settles within its bound");
    }
    fn route(&mut self, request: Request) {
        match request {
            Request::Dial => {
                self.dial = DialState::Pending;
                self.dials.push(self.now);
                if let Some(peer) = self.pending.pop_front() {
                    self.dial = DialState::Settled;
                    if let Some(peer) = peer {
                        let mut net = Net::new(peer, self.limits.chunk);
                        net.read_room = !std::mem::take(&mut self.silence_next);
                        net.other_version = std::mem::take(&mut self.other_version_next);
                        self.net = Some(net);
                        self.connection = Token::new(self.next);
                        self.next += 1;
                        self.component.down(self.now, Event::Dialled { connection: self.connection }, &mut self.out);
                    } else {
                        self.component.down(self.now, Event::Unreachable, &mut self.out);
                    }
                }
            }
            Request::Lower { connection, lower } => {
                assert_eq!(connection, self.connection);
                if let Some(net) = &mut self.net {
                    net.lower(lower, &mut self.rng);
                }
            }
            Request::Close { connection } | Request::Abort { connection } => {
                assert_eq!(connection, self.connection);
                self.archive();
                self.component.down(self.now, Event::Gone { connection }, &mut self.out);
            }
            Request::Fact { why } => self.facts.push(why),
            Request::RelayCancelled { run, attempt, call } => {
                self.trace.push(Trace::RelayEnded(run, attempt, call.clone()));
                self.root.push(Request::RelayCancelled { run, attempt, call });
            }
            Request::Resend { run, attempt, turn } => {
                self.trace.push(Trace::Resend(self.now, run, attempt, turn));
                self.root.push(Request::Resend { run, attempt, turn });
                if self.scripted {
                    self.component.down(
                        self.now,
                        Event::Up { up: fixtures::turn(), parts: Box::new([]) },
                        &mut self.out,
                    );
                }
            }
            Request::Ready => {
                self.trace.push(Trace::Ready);
                self.root.push(Request::Ready);
                if self.scripted {
                    self.component.down(self.now, Event::Hello { hello: fixtures::hello() }, &mut self.out);
                }
            }
            Request::Lost => {
                self.trace.push(Trace::Lost);
                self.root.push(Request::Lost);
            }
            Request::GracePassed => {
                self.trace.push(Trace::GracePassed);
                self.root.push(Request::GracePassed);
            }
            Request::Closed => {
                self.trace.push(Trace::Closed);
                self.root.push(Request::Closed);
            }
            Request::Down { down, parts } => {
                match &down {
                    jig_domain_host::LinkDown::Relayed { run, attempt, call, .. } => {
                        self.trace.push(Trace::RelayEnded(*run, *attempt, call.clone()));
                    }
                    jig_domain_host::LinkDown::TurnKept { run, attempt, turn } => {
                        self.trace.push(Trace::Kept(*run, *attempt, *turn));
                    }
                    jig_domain_host::LinkDown::Acknowledge { run, attempt } => {
                        self.trace.push(Trace::Acknowledged(*run, *attempt));
                    }
                    jig_domain_host::LinkDown::Assign { .. }
                    | jig_domain_host::LinkDown::Message { .. }
                    | jig_domain_host::LinkDown::Cancel { .. }
                    | jig_domain_host::LinkDown::Park { .. }
                    | jig_domain_host::LinkDown::Sources { .. }
                    | jig_domain_host::LinkDown::Check { .. }
                    | jig_domain_host::LinkDown::SignOut { .. }
                    | jig_domain_host::LinkDown::Grant { .. } => {}
                }
                self.order.push("down");
                self.root.push(Request::Down { down, parts });
            }
            request @ Request::Value { .. } => {
                self.order.push("value");
                self.values.push(request);
            }
        }
    }
    fn archive(&mut self) {
        if let Some(net) = self.net.take() {
            self.stalled += net.blocked;
            self.observed.push(net.peer.observed().to_vec());
            if let Some(refused) = net.refused {
                self.refused.push(refused);
            }
        }
    }
    pub fn play(&mut self, frame: skein_channel::Frame) {
        if frame.kind() == 0x0184
            && let Ok(record) = jig_protocol_worker_wire::TurnBusy::decode(
                &fixtures::codec(),
                &mut skein_lib::Reader::new(&frame.bytes()[8..]),
            )
        {
            self.trace.push(Trace::Busy(
                self.now,
                Token::new(record.task()),
                Token::new(record.attempt()),
                record.number(),
            ));
        }
        self.net.as_mut().expect("open channel").peer.play(frame);
        self.run();
    }
    #[must_use]
    pub fn records(&self) -> Vec<Vec<skein_fake_channel::Observed>> {
        let mut records = self.observed.clone();
        if let Some(net) = &self.net {
            records.push(net.peer.observed().to_vec());
        }
        records
    }
    #[must_use]
    pub fn blocked_writes(&self) -> u32 {
        self.stalled + self.net.as_ref().map_or(0, |net| net.blocked)
    }
    pub fn script_root(&mut self) {
        self.scripted = true;
    }
    #[must_use]
    pub fn trace(&self) -> &[Trace] {
        &self.trace
    }
    #[must_use]
    pub fn dial_pending(&self) -> bool {
        self.dial == DialState::Pending
    }
    pub fn connect_pending(&mut self) {
        assert!(self.dial_pending(), "only the requested dial is answered");
        let peer = self.pending.pop_front().expect("pending peer").expect("reachable");
        self.net = Some(Net::new(peer, self.limits.chunk));
        self.connection = Token::new(self.next);
        self.next += 1;
        self.down(Event::Dialled { connection: self.connection });
    }
    pub fn withhold_write_room(&mut self) {
        self.net.as_mut().expect("channel").write_room = false;
    }
    pub fn grant_write_room(&mut self) {
        self.net.as_mut().expect("channel").write_room = true;
        self.run();
    }
    pub fn silence_engine(&mut self) {
        self.net.as_mut().expect("channel").read_room = false;
    }
    #[must_use]
    pub fn dropped_facts(&self) -> u64 {
        self.component.dropped_facts()
    }
    #[must_use]
    pub fn has_channel(&self) -> bool {
        self.net.is_some()
    }
    #[must_use]
    pub fn next_deadline(&self) -> Option<Time> {
        self.component.next_deadline()
    }
    #[must_use]
    pub fn dropped_records(&self) -> u64 {
        self.component.dropped_records()
    }
    #[must_use]
    pub fn values(&self) -> &[Request] {
        &self.values
    }
    #[must_use]
    pub fn order(&self) -> &[&'static str] {
        &self.order
    }
    #[must_use]
    pub fn now(&self) -> Time {
        self.now
    }
    #[must_use]
    pub fn dials(&self) -> &[Time] {
        &self.dials
    }
    #[must_use]
    pub fn root(&self) -> &[Request] {
        &self.root
    }
    #[must_use]
    pub fn refusals(&self) -> &[(u16, Box<[u8]>)] {
        &self.refused
    }
    #[must_use]
    pub fn facts(&self) -> &[skein_channel::Closed] {
        &self.facts
    }
    #[must_use]
    pub fn credential(&self) -> &[u8] {
        self.net.as_ref().expect("channel open").peer.credential()
    }
}
