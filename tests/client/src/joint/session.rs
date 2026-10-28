//! The real protocol loop between the engine peer and original client routes
//! (client/domain/testing.md, 5). Authentication and activity reads use actual
//! owner doors; callbacks do not acknowledge their normal document flights.
use super::{
    engine::{Driver, Packet},
    packets::{self, Incoming},
    read_packets, requests,
    transport::Reply,
    truth::Truth,
};
use crate::harness::{ClientId, Observed};
use jig_client_domain::contract as client;
use jig_client_test_domain::{Event, Limits, Request};
use jig_domain_core as core;
use jig_domain_people as people;
use jig_test_domain as root;
use skein_lib::{Duration, Time, Wall};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
mod offline;
mod signing;

#[derive(Debug, PartialEq, Eq)]
pub enum Invalid {
    Request(requests::Invalid),
    Packet(packets::Invalid),
    Read(read_packets::Invalid),
    InternalPacket,
}

/// Opt-in external packet evidence, copied only after whole admission. Neither
/// list is reconstructed from a domain or used to decide an offer.
pub struct TaskOffers {
    pub source: skein_lib::List<core::acts::Offered>,
    pub native: skein_lib::List<client::Offered>,
    pub source_task: Box<core::documents::Task>,
    pub native_task: Box<client::Task>,
}

/// Protocol queues owned outside the client and engine roots. Business decisions
/// remain in the engine; parked requests retain their complete shell values.
pub struct Session {
    pub driver: Driver,
    limits: Limits,
    codec: jig_protocol_transcript::Limits,
    truth: Truth,
    parked: BTreeMap<ClientId, Vec<Request>>,
    unsigned: BTreeSet<ClientId>,
    reloaded: BTreeSet<ClientId>,
    sign_in_again: BTreeMap<ClientId, people::Again>,
    sign_in_failures: BTreeMap<ClientId, packets::SignInFailure>,
    callbacks: Vec<(ClientId, Event)>,
    sign_outs: BTreeMap<ClientId, VecDeque<u64>>,
    delayed: Vec<(ClientId, Event)>,
    delay_until: Option<Time>,
    lost_answers: BTreeSet<(ClientId, skein_lib::Token)>,
    lost_final_answers: u64,
    offline_until: Option<Time>,
    source_events: Vec<root::Event>,
    scheduled_sources: Vec<(Time, root::Event)>,
    stopped_seen: bool,
    lifecycle: Vec<Observed>,
    task_offers: Option<BTreeMap<(ClientId, u64), TaskOffers>>,
    card_offers: Option<super::offered_packets::Captured>,
    stream_heartbeat: Option<Duration>,
    stream_due: BTreeMap<ClientId, Time>,
}
impl Session {
    #[cfg(test)]
    pub(super) fn transcript_text_limit(&mut self, bytes: u32) {
        self.codec.turn.text = bytes;
        self.limits.client.domain.text = bytes;
        self.limits.client.domain.turn_text = bytes;
        self.limits.client.domain.turn_bytes = bytes;
    }

    #[must_use]
    pub fn new(driver: Driver, limits: Limits, codec: jig_protocol_transcript::Limits) -> Self {
        let truth = Truth::after(driver.world.store.rows.clone());
        Self {
            driver,
            limits,
            codec,
            truth,
            parked: BTreeMap::new(),
            unsigned: BTreeSet::new(),
            reloaded: BTreeSet::new(),
            sign_in_again: BTreeMap::new(),
            sign_in_failures: BTreeMap::new(),
            callbacks: Vec::new(),
            sign_outs: BTreeMap::new(),
            delayed: Vec::new(),
            delay_until: None,
            lost_answers: BTreeSet::new(),
            lost_final_answers: 0,
            offline_until: None,
            source_events: Vec::new(),
            scheduled_sources: Vec::new(),
            stopped_seen: false,
            lifecycle: Vec::new(),
            task_offers: None,
            card_offers: None,
            stream_heartbeat: None,
            stream_due: BTreeMap::new(),
        }
    }

    /// Actual final callbacks removed at the lost connection door, not scheduled faults.
    #[must_use]
    pub fn lost_final_answers(&self) -> u64 {
        self.lost_final_answers
    }

    /// Capture only card offers when a story asserts no whole Task offer packet.
    pub fn capture_card_offers(&mut self) {
        self.task_offers = None;
        self.card_offers = Some(BTreeMap::new());
    }

    /// Enable latest whole Task packet evidence for a public journey. Ordinary
    /// worlds keep this off and allocate no copies of the offered lists.
    pub fn capture_task_offers(&mut self) {
        self.task_offers = Some(BTreeMap::new());
        self.card_offers = Some(BTreeMap::new());
    }

    /// Supply the protocol stream's comment interval (protocol/client.md, 4).
    /// Its two-span tolerance must fit the client's configured silence timeout
    /// (client/domain/client.md, 8.1); neither configured duration is changed.
    pub fn stream_heartbeat(&mut self, span: Duration) {
        assert!(span != Duration::ZERO && span.as_nanos() <= self.limits.client.domain.heartbeat.as_nanos() / 2);
        self.stream_heartbeat = Some(span);
    }

    /// The source and receiving lists from the same admitted original packet.
    #[must_use]
    pub fn task_offers(&self, client: ClientId, task: u64) -> Option<&TaskOffers> {
        self.task_offers.as_ref()?.get(&(client, task))
    }

    /// Original engine and receiving offered lists for an admitted card read.
    #[must_use]
    pub fn card_offers(&self, client: ClientId, key: client::ObjectKey) -> Option<&super::offered_packets::CardOffers> {
        self.card_offers.as_ref()?.get(&(client, key))
    }

    /// Keys of the original admitted card packets for an external observer.
    #[must_use]
    pub fn card_offer_keys(&self, client: ClientId) -> Vec<client::ObjectKey> {
        self.card_offers.as_ref().map_or_else(Vec::new, |rows| {
            rows.keys().filter_map(|(owner, key)| if *owner == client { Some(*key) } else { None }).collect()
        })
    }

    /// Start with an externally supplied identity and genuine configured Hello.
    pub fn bind(&mut self, client: ClientId, party: u64, identity: people::Identity, now: Time, wall: Wall) {
        self.stopping().expect("whole original stopped routes");
        if self.driver.transport.is_bound(client) {
            self.detach(client, now, wall);
            self.driver.transport.replace_identity(client, party, identity);
        } else {
            self.driver.transport.bind(client, party, identity);
        }
        self.hello(client);
        self.sign_in(client, now, wall);
    }

    /// Bind a client whose protocol has no authenticated secret. Identity is
    /// retained for the provider exchange; it does not create a core session
    /// (protocol/client.md, sections 7–8).
    pub fn bind_unsigned(&mut self, client: ClientId, party: u64, identity: people::Identity, now: Time, wall: Wall) {
        self.stopping().expect("whole original stopped routes");
        if self.driver.transport.is_bound(client) {
            self.detach(client, now, wall);
            self.driver.transport.replace_identity(client, party, identity);
        } else {
            self.driver.transport.bind(client, party, identity);
        }
        self.unsigned.insert(client);
        self.hello(client);
    }

    /// A fresh root receives genuine stream negotiation and recreates its own
    /// reads/watches. Existing authentication remains the owner's actual session.
    pub fn reload(&mut self, client: ClientId, now: Time, wall: Wall) {
        self.detach(client, now, wall);
        if self.driver.transport.sign_in(client).is_some() {
            self.reloaded.insert(client);
            self.callbacks.push((client, crate::test::up(jig_client::Up::SignedIn)));
        }
        self.hello(client);
    }

    /// A version fault supplies the replacement range served by the protocol
    /// fixture, independently of core business configuration and Me.
    pub fn versions(&mut self, versions: client::VersionRange) {
        self.driver.transport.versions(versions);
        for client in self.driver.transport.clients() {
            self.hello(client);
        }
    }

    /// Supply external source events at declared world times. This ordinary
    /// transport owns delivery; the real core decides each outcome.
    pub fn sources_at(&mut self, mut sources: Vec<(Time, root::Event)>) {
        self.scheduled_sources.append(&mut sources);
        self.scheduled_sources.sort_by_key(|(at, _)| *at);
    }

    #[must_use]
    pub fn next_due(&self) -> Option<Time> {
        let due = if self.callbacks.is_empty() {
            if self.driver.world.stopped {
                if self.stopped_seen { None } else { Some(Time::ZERO) }
            } else if let Some(until) = self.offline_until {
                Some(until)
            } else {
                let heartbeat = if self.delay_until.is_none() { self.stream_due.values().copied().min() } else { None };
                [self.driver.next_due(), self.delay_until, heartbeat].into_iter().flatten().min()
            }
        } else {
            Some(Time::ZERO)
        };
        let source = if self.driver.world.stopped || self.offline_until.is_some() {
            None
        } else {
            self.scheduled_sources.first().map(|(at, _)| *at)
        };
        [due, source].into_iter().flatten().min()
    }

    fn hello(&mut self, client: ClientId) {
        self.stopping().expect("whole original stopped routes");
        let value = (client, crate::test::up(self.driver.transport.hello()));
        if self.offline() {
            self.delayed.push(value);
        } else {
            self.callbacks.push(value);
        }
    }

    fn source(&mut self, now: Time, wall: Wall, event: root::Event) {
        if self.offline() {
            self.source_events.push(event);
        } else {
            self.driver.send(now, wall, event);
        }
    }

    /// The fixture supplies the cold-root configuration and recovery deadline.
    /// Retained owner sessions and keyed answers remain in the actual store;
    /// every old protocol route and pending document flight is retired.
    /// # Errors
    /// Reports an original watch callback routing error.
    pub fn restart(&mut self, configuration: root::Config, now: Time, wall: Wall, until: Time) -> Result<(), Invalid> {
        self.keep_reload_bootstrap();
        self.delayed.clear();
        self.delay_until = None;
        self.lost_answers.clear();
        self.source_events.clear();
        let watches = self.driver.transport.watches();
        for watcher in &watches {
            let Incoming::Callback { client, event } =
                packets::dropped(&mut self.driver.transport, *watcher).map_err(Invalid::Packet)?
            else {
                return Err(Invalid::InternalPacket);
            };
            self.callbacks.push((client, event));
        }
        for client in self.driver.transport.clients() {
            self.driver.transport.detach(client);
        }
        self.driver.restart(configuration, now, wall);
        self.stopped_seen = false;
        for watcher in watches {
            self.driver.send(now, wall, root::Event::Core(core::Event::Unwatch { watcher }));
        }
        self.offline_until = Some(until);
        Ok(())
    }

    #[must_use]
    pub fn offline(&self) -> bool {
        self.offline_until.is_some() || self.driver.world.stopped
    }

    /// The existing world records the root's actual Stop request. Retire the
    /// failed connection once, without an answer, a Taken or a guessed reason.
    fn stopping(&mut self) -> Result<(), Invalid> {
        if !self.driver.world.stopped || self.stopped_seen {
            return Ok(());
        }
        self.callbacks.clear();
        self.delayed.clear();
        self.delay_until = None;
        self.lost_answers.clear();
        for watcher in self.driver.transport.watches() {
            let Incoming::Callback { client, event } =
                packets::dropped(&mut self.driver.transport, watcher).map_err(Invalid::Packet)?
            else {
                return Err(Invalid::InternalPacket);
            };
            self.callbacks.push((client, event));
        }
        for client in self.driver.transport.clients() {
            self.driver.transport.detach(client);
        }
        self.stopped_seen = true;
        self.lifecycle.push(Observed::Reachable { live: false });
        Ok(())
    }

    fn recover(&mut self, now: Time, wall: Wall) -> Result<(), Invalid> {
        self.offline_until = None;
        for client in self.driver.transport.clients() {
            if self.reloaded.contains(&client) && self.driver.transport.sign_in(client).is_some() {
                self.callbacks.push((client, crate::test::up(jig_client::Up::SignedIn)));
            }
            self.hello(client);
        }
        if self.delay_until.is_none_or(|until| now >= until) {
            self.callbacks.append(&mut self.delayed);
        }
        for event in std::mem::take(&mut self.source_events) {
            self.driver.send(now, wall, event);
        }
        for (client, requests) in std::mem::take(&mut self.parked) {
            for request in requests {
                self.send(client, now, wall, request)?;
            }
        }
        Ok(())
    }

    /// Delay actual owner callbacks at the protocol boundary while their normal
    /// document flights remain pending. The caller supplies the deadline.
    pub fn delay(&mut self, until: Time) {
        self.delay_until = Some(self.delay_until.map_or(until, |old| old.max(until)));
        self.delayed.append(&mut self.callbacks);
    }

    /// Lose one exact final wire attempt after its actual answer was made; the
    /// owner still retains its key and independent durable observations.
    pub fn lose_answer(&mut self, client: ClientId, attempt: skein_lib::Token) {
        let final_for_attempt = |(owner, event): &(ClientId, Event)| {
            *owner == client
                && matches!(event, Event::Client { event: jig_client::Event::Up { up: jig_client::Up::Answer { attempt: actual, envelope: skein_lib::RequestEnvelope::Final, .. } } } if *actual == attempt)
        };
        let before = self.callbacks.len().checked_add(self.delayed.len()).expect("finite callbacks");
        self.callbacks.retain(|event| !final_for_attempt(event));
        self.delayed.retain(|event| !final_for_attempt(event));
        if before == self.callbacks.len().checked_add(self.delayed.len()).expect("finite callbacks") {
            self.lost_answers.insert((client, attempt));
        }
    }

    /// Drop only the specified protocol watch, report that transport fact and
    /// close the actual owner watch without acknowledging its pending document.
    /// # Errors
    /// Returns a packet routing error without choosing an owner outcome.
    pub fn drop_watch(
        &mut self,
        client: ClientId,
        native: skein_lib::Token,
        now: Time,
        wall: Wall,
    ) -> Result<(), Invalid> {
        self.stopping()?;
        let Some(watcher) = self.driver.transport.watch_token(client, native) else { return Ok(()) };
        let callback = packets::dropped(&mut self.driver.transport, watcher).map_err(Invalid::Packet)?;
        self.incoming(callback, now, wall)?;
        self.driver.send(now, wall, root::Event::Core(core::Event::Unwatch { watcher }));
        Ok(())
    }

    /// # Errors
    /// Rejects an unsupported whole request. An absent authenticated session parks
    /// its original value without creating any owner route or changing its names.
    pub fn send(&mut self, client: ClientId, now: Time, wall: Wall, request: Request) -> Result<(), Invalid> {
        self.stopping()?;
        if self.offline() {
            self.disconnected(client, request);
            return Ok(());
        }
        match requests::send(&mut self.driver, client, now, wall, &request, &self.limits.client.domain) {
            Ok(()) => Ok(()),
            Err(requests::Invalid::Session) => {
                if self.unsigned.contains(&client) {
                    let event = match &request {
                        Request::Client {
                            request: jig_client::Request::Down { down: jig_client::Down::Open { watch, .. } },
                        } => Some(crate::test::up(jig_client::Up::WatchEnded {
                            watch: *watch,
                            end: client::WatchEnd::SignedOut,
                        })),
                        Request::Save { .. }
                        | Request::Erase { .. }
                        | Request::Client { .. }
                        | Request::First(_)
                        | Request::Show { .. }
                        | Request::SignIn { .. }
                        | Request::SignOut
                        | Request::Fact { .. } => None,
                    };
                    if let Some(event) = event {
                        self.callbacks.push((client, event));
                        return Ok(());
                    }
                }
                self.parked.entry(client).or_default().push(request);
                Ok(())
            }
            Err(why) => Err(Invalid::Request(why)),
        }
    }

    fn incoming(&mut self, value: Incoming, now: Time, wall: Wall) -> Result<(), Invalid> {
        match value {
            Incoming::Callback { client, event } => {
                if matches!(
                    event,
                    Event::Client {
                        event: jig_client::Event::Up {
                            up: jig_client::Up::Watched { .. } | jig_client::Up::WatchEnded { .. }
                        }
                    } | Event::First(jig_client_test_domain::first::Event::Watch { .. })
                ) {
                    self.stream_event(client, now);
                }
                if let Event::Client {
                    event:
                        jig_client::Event::Up {
                            up: jig_client::Up::Answer { attempt, envelope: skein_lib::RequestEnvelope::Final, .. },
                        },
                } = &event
                    && self.lost_answers.remove(&(client, *attempt))
                {
                    self.lost_final_answers += 1;
                    return Ok(());
                }
                if self.delay_until.is_some_and(|until| now < until) {
                    self.delayed.push((client, event));
                } else {
                    self.callbacks.push((client, event));
                }
            }
            Incoming::SignedOut { client, sign_in } => {
                self.sign_outs.entry(client).or_default().push_back(sign_in);
                self.incoming(Incoming::Callback { client, event: Event::SignedOut }, now, wall)?;
            }
            Incoming::SignInAgain { client, why } => {
                self.sign_in_again.insert(client, why);
                if self.driver.transport.sign_in(client).is_none() {
                    self.unsigned.insert(client);
                }
            }
            Incoming::SignInFailed { client, failure } => {
                self.sign_in_failures.insert(client, failure);
                if self.driver.transport.sign_in(client).is_none() {
                    self.unsigned.insert(client);
                }
            }
            Incoming::Authenticated { client, .. } => {
                self.callbacks.push((client, crate::test::up(jig_client::Up::SignedIn)));
                self.unsigned.remove(&client);
                self.sign_in_again.remove(&client);
                self.sign_in_failures.remove(&client);
                if let Some(requests) = self.parked.remove(&client) {
                    for request in requests {
                        self.send(client, now, wall, request)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn stream_event(&mut self, client: ClientId, now: Time) {
        let Some(span) = self.stream_heartbeat else { return };
        self.stream_due.insert(client, now.saturating_add(span));
        for watcher in self.driver.transport.watches() {
            let Some(watch) = self.driver.transport.watch(watcher) else { continue };
            if watch.client != client {
                continue;
            }
            let event = match watch.subject {
                super::transport::Watched::Jig(_) => {
                    crate::test::up(jig_client::Up::Watched { watch: watch.native, event: client::WatchEvent::Alive })
                }
                super::transport::Watched::Application { .. } => {
                    Event::First(jig_client_test_domain::first::Event::Watch {
                        watch: watch.native,
                        event: jig_client_test_domain::family::WatchEvent::Alive,
                    })
                }
            };
            if self.delay_until.is_some_and(|until| now < until) {
                self.delayed.push((client, event));
            } else {
                self.callbacks.push((client, event));
            }
        }
    }

    fn read(
        &mut self,
        to: skein_lib::Token,
        answer: &core::reads::Answer,
        now: Time,
        wall: Wall,
    ) -> Result<(), Invalid> {
        if self.driver.transport.reply(to).is_none() {
            return Ok(());
        }
        let value = read_packets::read_with_codec(
            &mut self.driver.transport,
            to,
            answer,
            &self.limits.client.domain,
            &self.codec,
        )
        .map_err(Invalid::Read)?;
        super::offered_packets::read(&mut self.card_offers, answer, &value);
        self.incoming(value, now, wall)?;
        Ok(())
    }

    fn application_document(
        &mut self,
        watcher: skein_lib::Token,
        number: u16,
        document: jig_test_connector::ClientDocument,
        now: Time,
        wall: Wall,
    ) -> Result<(), Invalid> {
        if self.driver.transport.watch(watcher).is_none() {
            return Ok(());
        }
        let value = packets::application_document(
            &mut self.driver.transport,
            watcher,
            number,
            document,
            &self.limits.first.connector,
        )
        .map_err(Invalid::Packet)?;
        if self.driver.transport.is_closing(watcher) {
            self.finish_closed_packet(watcher, now, wall);
            Ok(())
        } else {
            self.incoming(value, now, wall)
        }
    }

    fn application_read(
        &mut self,
        to: skein_lib::Token,
        number: u16,
        answer: Result<jig_test_connector::ClientDocument, jig_test_connector::DocumentMissing>,
        now: Time,
        wall: Wall,
    ) -> Result<(), Invalid> {
        if self.driver.transport.reply(to).is_none() {
            return Ok(());
        }
        let value =
            packets::application_read(&mut self.driver.transport, to, number, answer, &self.limits.first.connector)
                .map_err(Invalid::Packet)?;
        self.incoming(value, now, wall)
    }

    fn capture_task_packet(&mut self, document: &core::documents::Document, value: &Incoming) {
        if let (
            Some(captured),
            core::documents::Document::Task(source),
            Incoming::Callback {
                client,
                event:
                    Event::Client {
                        event:
                            jig_client::Event::Up {
                                up:
                                    jig_client::Up::Watched {
                                        event: client::WatchEvent::Delivered(client::Delivery::Task(native)),
                                        ..
                                    },
                            },
                    },
            },
        ) = (&mut self.task_offers, document, value)
        {
            captured.insert(
                (*client, source.chip.number),
                TaskOffers {
                    source: source.acts.clone(),
                    native: native.acts.clone(),
                    source_task: source.clone(),
                    native_task: native.clone(),
                },
            );
        }
    }

    #[expect(clippy::too_many_lines, reason = "one exhaustive delivery translation for the joint client session")]
    fn held(&mut self, delivery: root::Delivery, now: Time, wall: Wall) -> Result<(), Invalid> {
        match delivery {
            root::Delivery::Core(core::Held::Acted { .. }) | root::Delivery::ProjectMade { .. } => {}
            root::Delivery::Core(core::Held::PeopleReply { to, reply }) => {
                let to = to.into_token();
                if self.driver.transport.reply(to).is_none() {
                    return Ok(());
                }
                let translated =
                    super::wire::answers::reply(reply, &self.driver.transport.models, &self.limits.client.domain)
                        .map_err(|why| {
                            Invalid::Packet(packets::Invalid::Document(super::wire::documents::Invalid::Owner(why)))
                        })?;
                let translated = match translated {
                    super::wire::answers::Reply::MovingOut { account, number, source } => {
                        let record = self
                            .driver
                            .world
                            .take_moved(source)
                            .unwrap_or_else(|| skein_lib::Secret::new(Box::new([])));
                        super::wire::answers::Reply::Final(jig_client_domain::contract::Answer::Done(
                            jig_client_domain::contract::Outcome::Made {
                                what: jig_client_domain::contract::Made::MovingOut { account, number, record },
                            },
                        ))
                    }
                    value @ (super::wire::answers::Reply::SignedIn { .. }
                    | super::wire::answers::Reply::Final(_)
                    | super::wire::answers::Reply::Again(_)
                    | super::wire::answers::Reply::SignedOut
                    | super::wire::answers::Reply::Applied { .. }) => value,
                };
                let value =
                    packets::translated_reply(&mut self.driver.transport, to, translated, &self.limits.client.domain)
                        .map_err(Invalid::Packet)?;
                self.incoming(value, now, wall)?;
            }
            root::Delivery::Core(core::Held::Read { reply, answer }) => {
                self.read(reply.into_token(), &answer, now, wall)?;
            }
            root::Delivery::Core(core::Held::Delivery { watcher, document }) => {
                if self.driver.transport.watch(watcher).is_none() {
                    return Ok(());
                }
                let value =
                    packets::document(&mut self.driver.transport, watcher, &document, &self.limits.client.domain)
                        .map_err(Invalid::Packet)?;
                self.capture_task_packet(&document, &value);
                if self.driver.transport.is_closing(watcher) {
                    self.finish_closed_packet(watcher, now, wall);
                } else {
                    self.incoming(value, now, wall)?;
                }
            }
            root::Delivery::Core(core::Held::WatchEnded { watcher, why }) => {
                if self.driver.transport.watch(watcher).is_none() {
                    return Ok(());
                }
                let value = packets::ended(&mut self.driver.transport, watcher, why).map_err(Invalid::Packet)?;
                // A silent client waits for this actual terminal before reopening its original watch.
                self.incoming(value, now, wall)?;
            }
            root::Delivery::Application { to, number, outcome } => {
                let to = to.into_token();
                if self.driver.transport.reply(to).is_none() {
                    return Ok(());
                }
                let value = packets::application_reply(
                    &mut self.driver.transport,
                    to,
                    number,
                    outcome,
                    &self.limits.first.connector,
                )
                .map_err(Invalid::Packet)?;
                self.incoming(value, now, wall)?;
            }
            root::Delivery::ApplicationDocument { watcher, number, document } => {
                self.application_document(watcher, number, *document, now, wall)?;
            }
            root::Delivery::ApplicationRead { reply, number, answer } => {
                self.application_read(reply.into_token(), number, answer, now, wall)?;
            }
            // The core's decision; the clients' component's erase (store.md, 9).
            root::Delivery::Core(core::Held::CarriedUnused { number }) => self.driver.transport.unused(number),
            other @ (root::Delivery::Core(
                core::Held::ApplicationDelivery { .. }
                | core::Held::ApplicationWatchEnded { .. }
                | core::Held::ReadLoad { .. }
                | core::Held::Applied { .. }
                | core::Held::MakeEffect { .. }
                | core::Held::SettledCall { .. }
                | core::Held::Assign { .. }
                | core::Held::Grant { .. }
                | core::Held::Relayed { .. }
                | core::Held::Inbound { .. }
                | core::Held::Relay { .. }
                | core::Held::CallAnswer { .. }
                | core::Held::Result { .. }
                | core::Held::Acknowledge { .. }
                | core::Held::TaskTerminalAcknowledged { .. }
                | core::Held::TaskTurnKept { .. }
                | core::Held::AcknowledgeTurn { .. }
                | core::Held::Drained
                | core::Held::Cancel { .. }
                | core::Held::Sources { .. }
                | core::Held::CheckSource { .. }
                | core::Held::SignOutSource { .. }
                | core::Held::SourceSignedOut { .. }
                | core::Held::Refuse { .. }
                | core::Held::TurnBusy { .. }
                | core::Held::StopRun { .. }
                | core::Held::Load(..)
                | core::Held::NotesLoad { .. }
                | core::Held::NotesWritten { .. }
                | core::Held::NotesDeleted { .. },
            )
            | root::Delivery::Projection { .. }
            | root::Delivery::Message { .. }
            | root::Delivery::Restart(_)
            | root::Delivery::Link { .. }
            | root::Delivery::CallBusy { .. }
            | root::Delivery::CallAnswer { .. }
            | root::Delivery::Assigned { .. }
            | root::Delivery::Fleet(_)
            | root::Delivery::System { .. }
            | root::Delivery::LostRead { .. }
            | root::Delivery::Procedure { .. }) => {
                return unrouted_delivery(&other);
            }
        }
        Ok(())
    }

    fn finish_closed_packet(&mut self, watcher: skein_lib::Token, now: Time, wall: Wall) {
        for event in self.driver.transport.closed_flights(watcher) {
            self.driver.send(now, wall, root::Event::Core(event));
        }
    }

    fn immediate(&mut self, value: core::Now, now: Time, wall: Wall) -> Result<(), Invalid> {
        match value {
            core::Now::RunText { watcher, turn, from, to, text, cut, call, writing } => {
                if self.driver.transport.watch(watcher).is_none() {
                    return Ok(());
                }
                let source = jig_domain_views::Writing { turn, from, to, text, cut, call, writing };
                let value = packets::stream(&mut self.driver.transport, watcher, &source, &self.limits.client.domain)
                    .map_err(Invalid::Packet)?;
                if self.driver.transport.is_closing(watcher) {
                    self.finish_closed_packet(watcher, now, wall);
                } else {
                    self.incoming(value, now, wall)?;
                }
            }
            core::Now::WatchRefused { watcher, why } => {
                if self.driver.transport.watch(watcher).is_none() {
                    return Ok(());
                }
                let value = packets::refused(&mut self.driver.transport, watcher, why).map_err(Invalid::Packet)?;
                self.incoming(value, now, wall)?;
            }
            core::Now::SignInRefused { to } => {
                let to = to.into_token();
                if let Some(route) = self.driver.transport.reply(to) {
                    let Reply::SignIn { client } = *route else { return Err(Invalid::InternalPacket) };
                    self.driver.transport.take_reply(to).expect("original provider route");
                    self.incoming(
                        Incoming::SignInFailed { client, failure: packets::SignInFailure::Capacity },
                        now,
                        wall,
                    )?;
                }
            }
            other @ (core::Now::Relayed { .. }
            | core::Now::EffectAnswer { .. }
            | core::Now::SettingsAnswer { .. }
            | core::Now::SettledCallRefused { .. }
            | core::Now::Call { .. }
            | core::Now::RelayBusy { .. }
            | core::Now::CallBusy { .. }
            | core::Now::DropCall { .. }
            | core::Now::Undelivered { .. }
            | core::Now::Account(_)
            | core::Now::NotesIndexed { .. }
            | core::Now::NotesRecalled { .. }
            | core::Now::NotesRefused { .. }
            | core::Now::NoteBusy { .. }
            | core::Now::DropPayload { .. }
            | core::Now::DropAssignment { .. }
            | core::Now::TurnPayload { .. }
            | core::Now::AnswerPayload { .. }
            | core::Now::AcceptedTurn { .. }
            | core::Now::RefusedPayload { .. }
            | core::Now::Activate { .. }
            | core::Now::PrepareAgent { .. }
            | core::Now::StartPreparation { .. }
            | core::Now::SeenConversation { .. }
            | core::Now::LoadEndedRead { .. }
            | core::Now::HistoricalProposal { .. }
            | core::Now::DecidedProposal { .. }
            | core::Now::HistoricalPersonTask { .. }
            | core::Now::EndedTask { .. }
            | core::Now::PartyInputs { .. }
            | core::Now::CompleteBrief { .. }
            | core::Now::BriefCorePlanned { .. }
            | core::Now::WorkspaceRequest { .. }
            | core::Now::RunPreparationFailed { .. }
            | core::Now::RunPrepared { .. }
            | core::Now::HistoricalEscalation { .. }
            | core::Now::RestoreRefused) => {
                assert!(
                    !jig_core_world::client_peer::ClientPeer::immediate(&other),
                    "all immediate peer variants are explicitly routed"
                );
                return Err(Invalid::InternalPacket);
            }
        }
        Ok(())
    }

    /// # Errors
    /// Reports an unimplemented actual carrier or whole receiving refusal. The
    /// loop reads only externally released peer packets, not private root state.
    pub fn poll(&mut self, now: Time, wall: Wall) -> Result<Vec<(ClientId, Event)>, Invalid> {
        self.stopping()?;
        if self.driver.world.stopped {
            return Ok(std::mem::take(&mut self.callbacks));
        }
        if let Some(until) = self.offline_until {
            if now < until {
                return Ok(std::mem::take(&mut self.callbacks));
            }
            self.recover(now, wall)?;
        }
        let ready = self.scheduled_sources.partition_point(|(at, _)| *at <= now);
        let sources: Vec<_> = self.scheduled_sources.drain(..ready).collect();
        for (_, event) in sources {
            self.source(now, wall, event);
        }
        if self.delay_until.is_some_and(|until| now >= until) {
            self.delay_until = None;
            self.callbacks.append(&mut self.delayed);
        }
        for (to, answer) in self.driver.take_refusals() {
            if self.driver.transport.reply(to).is_none() {
                continue;
            }
            let value = packets::keyed_refusal(&mut self.driver.transport, to, answer, &self.limits.client.domain)
                .map_err(Invalid::Packet)?;
            self.incoming(value, now, wall)?;
        }
        loop {
            let packets = self.driver.poll(now, wall);
            self.stopping()?;
            if self.driver.world.stopped {
                break;
            }
            if packets.is_empty() {
                break;
            }
            for packet in packets {
                match packet {
                    Packet::Held { delivery, .. } => self.held(*delivery, now, wall)?,
                    Packet::Immediate(value) => self.immediate(*value, now, wall)?,
                }
            }
        }
        self.stream_due.retain(|client, _| {
            self.driver
                .transport
                .watches()
                .iter()
                .any(|watcher| self.driver.transport.watch(*watcher).is_some_and(|watch| watch.client == *client))
        });
        if !self.driver.world.stopped && self.delay_until.is_none() {
            let due: Vec<_> =
                self.stream_due.iter().filter_map(|(client, at)| (*at <= now).then_some(*client)).collect();
            for client in due {
                self.stream_event(client, now);
            }
        }
        Ok(std::mem::take(&mut self.callbacks))
    }

    /// Drain complete applied store images before the shell consumes callbacks.
    /// No callback or request supplies the durability index or retained facts.
    pub fn observed(&mut self) -> Vec<Observed> {
        let mut values: Vec<_> =
            self.driver.snapshots().into_iter().flat_map(|snapshot| self.truth.applied(snapshot)).collect();
        values.append(&mut self.lifecycle);
        values
    }

    /// A sign-out answer belongs only to the session whose secret it carried.
    pub(super) fn delivers_sign_out(&mut self, client: ClientId) -> bool {
        let sign_in = self
            .sign_outs
            .get_mut(&client)
            .and_then(VecDeque::pop_front)
            .expect("sign-out callback retains its exchange");
        self.driver.transport.sign_in(client).is_none_or(|current| current == sign_in)
    }

    /// Detach old routes without acknowledging their pending documents or
    /// reusing their tokens. The client supplies new reads/watches after reload.
    pub fn detach(&mut self, client: ClientId, now: Time, wall: Wall) {
        self.stream_due.remove(&client);
        self.sign_outs.remove(&client);
        self.parked.remove(&client);
        self.callbacks.retain(|(owner, _)| *owner != client);
        self.delayed.retain(|(owner, _)| *owner != client);
        self.lost_answers.retain(|(owner, _)| *owner != client);
        for watcher in self.driver.transport.detach(client) {
            self.driver.send(now, wall, root::Event::Core(core::Event::Unwatch { watcher }));
        }
    }
}

fn unrouted_delivery(other: &root::Delivery) -> Result<(), Invalid> {
    assert!(
        !jig_core_world::client_peer::ClientPeer::accepts(other),
        "all released peer variants are explicitly routed"
    );
    Err(Invalid::InternalPacket)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::joint::take_signed_in;
    use jig_core_world::peers::Peers;
    use skein_lib::Token;

    fn session() -> (Session, Time, Wall) {
        configured_session(jig_core_world::world::config(8455), Peers::new(Vec::new(), Vec::new()))
    }

    fn configured_session(configuration: root::Config, peers: Peers) -> (Session, Time, Wall) {
        session_with_limits(configuration, jig_core_world::world::limits(), peers)
    }

    fn session_with_limits(
        configuration: root::Config,
        mut owner_limits: root::Limits,
        peers: Peers,
    ) -> (Session, Time, Wall) {
        let now = Time::from_nanos(17);
        let wall = Wall::from_nanos(1_000_000_000_000);
        owner_limits.core.tasks.batch = owner_limits.core.tasks.batch.max(owner_limits.core.authority.batch);
        for charter in &configuration.core.defaults.charters {
            let bytes = core::policy_owned_bytes(&charter.policy).expect("fixture run policy size");
            owner_limits.core.run_bytes =
                owner_limits.core.run_bytes.max(u32::try_from(bytes).expect("fixture run bound"));
        }
        let room = core::room_max(&owner_limits.core).expect("fixture core output bound");
        let connectors = jig_test_connector::MAX_OUT.checked_mul(2).expect("two fixture connectors");
        owner_limits.journal.writes =
            owner_limits.journal.writes.max(room.writes.checked_add(connectors).expect("fixture writes"));
        owner_limits.journal.held =
            owner_limits.journal.held.max(room.held.checked_add(connectors).expect("fixture deliveries"));
        let driver = Driver::new(configuration, owner_limits, peers, now, wall);
        let mut limits = crate::test::limits();
        limits.client.domain.rows = 32;
        limits.client.domain.projects = 64;
        limits.client.domain.text = 4096;
        limits.client.domain.turn_text = 4096;
        limits.client.domain.turn_bytes = 4096;
        limits.client.domain.document_bytes = 1_048_576;
        limits.jig.rows = 32;
        limits.jig.projects = 64;
        limits.jig.roles = 64;
        limits.jig.text = 4096;
        limits.jig.document_bytes = 1_048_576;
        let codec = jig_protocol_transcript::Limits {
            smith: smith_transcript::v6::CEILINGS,
            bytes: 1_048_576,
            turn: client::TurnLimits { messages: 64, blocks: 16, text: 65_536 },
        };
        (Session::new(driver, limits, codec), now, wall)
    }
    fn identity() -> people::Identity {
        people::Identity {
            key: people::IdentityKey { provider: 0, subject: 7_u64.to_be_bytes().into() },
            login: Box::from(b"login".as_slice()),
            name: Box::from(b"Person".as_slice()),
        }
    }
    fn frame(watch: Token) -> Request {
        Request::Client {
            request: jig_client::Request::Down {
                down: jig_client::Down::Open { watch, subject: jig_client::Watchable::Jig(client::Subject::Me) },
            },
        }
    }

    #[test]
    fn the_joint_referee_catches_a_peer_dropping_an_actual_normal_delivery_acknowledgement() {
        let (mut session, now, wall) = session();
        let client = ClientId(0);
        let native = Token::new(19);
        session.bind(client, 1, identity(), now, wall);
        session.poll(now, wall).expect("actual sign-in");
        session.send(client, now, wall, frame(native)).expect("actual frame watch");
        let callbacks = session.poll(now, wall).expect("actual root delivery");
        assert!(callbacks.iter().any(|(owner, event)| *owner == client && matches!(event,
            Event::Client { event: jig_client::Event::Up { up: jig_client::Up::Watched { watch, event: client::WatchEvent::Delivered(_) } } } if *watch == native)));
        let watcher = session.driver.transport.watch_token(client, native).expect("original owner route");
        session.driver.referee.consumed(watcher);
        let acknowledgement = session.driver.transport.taken(client, native).expect("shell actually took the frame");
        assert!(matches!(&acknowledgement, core::Event::Delivered { watcher: actual } if *actual == watcher));
        assert_eq!(
            session.driver.referee.check(),
            Err(super::super::referee::MissingAcknowledgement(watcher)),
            "taking locally cannot stand in for the dropped root input"
        );
        assert_eq!(session.driver.referee.acknowledgements(), 0);
        session.driver.send(now, wall, root::Event::Core(acknowledgement));
        assert_eq!(session.driver.referee.check(), Ok(()));
        assert_eq!(session.driver.referee.acknowledgements(), 1);
    }

    #[test]
    fn a_closed_actual_watch_returns_its_terminal_before_the_same_native_watch_reopens() {
        let (mut session, now, wall) = session();
        let client = ClientId(0);
        session.bind(client, 1, identity(), now, wall);
        let _ = session.poll(now, wall).expect("actual authentication");
        let native = Token::new(19);
        session.send(client, now, wall, frame(native)).expect("actual owner watch");
        let original = session.driver.transport.watch_token(client, native).expect("original route");
        session
            .send(
                client,
                now,
                wall,
                Request::Client {
                    request: jig_client::Request::Down { down: jig_client::Down::Close { watch: native } },
                },
            )
            .expect("actual close");
        let mut callbacks = session.poll(now, wall).expect("discard late body and finish actual flight");
        callbacks.extend(session.poll(now, wall).expect("actual owning terminal"));
        assert!(
            matches!(callbacks.as_slice(), [(owner, Event::Client { event: jig_client::Event::Up { up: jig_client::Up::WatchEnded {
            watch, end: client::WatchEnd::Closed,
        } } })] if *owner == client && *watch == native)
        );
        assert!(session.driver.transport.watch(original).is_none());
        session.send(client, now, wall, frame(native)).expect("replacement actual watch");
        let replacement = session.driver.transport.watch_token(client, native).expect("replacement route");
        assert_ne!(replacement, original);
        let callbacks = session.poll(now, wall).expect("replacement body");
        assert!(
            matches!(callbacks.as_slice(), [(owner, Event::Client { event: jig_client::Event::Up { up: jig_client::Up::Watched {
            watch, event: client::WatchEvent::Delivered(client::Delivery::Me(_)),
        } } })] if *owner == client && *watch == native)
        );
    }

    #[test]
    fn stream_comments_keep_every_original_watch_alive_without_taking_a_normal_flight_or_recreating_closed_routes() {
        let (mut session, now, wall) = session();
        let client = ClientId(0);
        session.stream_heartbeat(Duration::from_millis(50));
        session.bind(client, 1, identity(), now, wall);
        let _ = session.poll(now, wall).expect("actual authentication");
        for watch in [Token::new(0), Token::new(u64::MAX)] {
            session.send(client, now, wall, frame(watch)).expect("actual owner watch");
        }
        let first = session.poll(now, wall).expect("actual Me packets");
        assert_eq!(
            first
                .iter()
                .filter(|(_, event)| matches!(
                    event,
                    Event::Client {
                        event: jig_client::Event::Up {
                            up: jig_client::Up::Watched {
                                event: client::WatchEvent::Delivered(client::Delivery::Me(_)),
                                ..
                            }
                        }
                    }
                ))
                .count(),
            2
        );
        let rows = session.driver.world.store.rows.clone();
        let applied = session.driver.world.store.applied;
        let commits = session.driver.world.commits.len();
        let due = now.saturating_add(Duration::from_millis(50));
        assert_eq!(session.next_due(), Some(due));
        assert!(session.poll(now.saturating_add(Duration::from_millis(49)), wall).expect("quiet stream").is_empty());
        let comments = session.poll(due, wall).expect("actual quiet-stream comment");
        assert_eq!(comments.len(), 2);
        assert!(comments.iter().all(|(owner, event)| *owner == client
            && matches!(
                event,
                Event::Client {
                    event: jig_client::Event::Up {
                        up: jig_client::Up::Watched { event: client::WatchEvent::Alive, .. }
                    }
                }
            )));
        let mut tokens: Vec<_> = comments
            .iter()
            .filter_map(|(_, event)| match event {
                Event::Client { event: jig_client::Event::Up { up: jig_client::Up::Watched { watch, .. } } } => {
                    Some(*watch)
                }
                Event::Start { .. }
                | Event::Went { .. }
                | Event::Act { .. }
                | Event::Client { event: jig_client::Event::Up { up: jig_client::Up::Answer { .. } } }
                | Event::Jig(_)
                | Event::First(_)
                | Event::SignedOut
                | Event::Client { .. } => None,
            })
            .collect();
        tokens.sort();
        assert_eq!(tokens, vec![Token::new(0), Token::new(u64::MAX)]);
        assert_eq!(session.driver.world.store.rows, rows);
        assert_eq!(session.driver.world.store.applied, applied);
        assert_eq!(session.driver.world.commits.len(), commits);
        assert!(session.driver.transport.taken(client, Token::new(0)).is_some(), "Alive did not ACK the normal flight");
        session
            .send(
                client,
                due,
                wall,
                Request::Client {
                    request: jig_client::Request::Down { down: jig_client::Down::Close { watch: Token::new(0) } },
                },
            )
            .expect("close original watch");
        let _ = session.poll(due, wall).expect("original close");
        let next = due.saturating_add(Duration::from_millis(50));
        let comments = session.poll(next, wall).expect("remaining original route");
        assert!(
            matches!(comments.as_slice(), [(owner, Event::Client { event: jig_client::Event::Up { up: jig_client::Up::Watched {
            watch, event: client::WatchEvent::Alive,
        } } })] if *owner == client && *watch == Token::new(u64::MAX))
        );
        session.detach(client, next, wall);
        assert!(
            session.poll(next.saturating_add(Duration::from_millis(50)), wall).expect("detached stream").is_empty()
        );
        assert!(session.stream_due.is_empty());
    }

    #[test]
    fn real_provider_invalid_identity_is_retained_after_release_and_only_an_explicit_exchange_replaces_it() {
        invalid_identity_is_retained_until_an_explicit_exchange();
    }

    fn invalid_identity_is_retained_until_an_explicit_exchange() {
        let (mut session, now, wall) = session();
        let mut invalid = identity();
        invalid.key.subject = Box::new([]);
        session.bind_unsigned(ClientId(0), 1, invalid, now, wall);
        let _ = session.poll(now, wall).expect("actual Hello");
        session.sign_in(ClientId(0), now, wall);
        let captured = &session.driver.world.client_peer.as_ref().expect("actual external peer").delivered;
        assert!(captured.iter().any(|(_, value)| matches!(
            value,
            root::Delivery::Core(core::Held::PeopleReply {
                reply: people::Reply::Refused(people::Refusal::Limit {
                    which: people::LimitKind::Identity,
                    bound: None
                }),
                ..
            })
        )));
        assert!(session.poll(now, wall).expect("provider refusal is a normal exchange terminal").is_empty());
        assert!(matches!(
            session.sign_in_failure(ClientId(0)),
            Some(packets::SignInFailure::Refused(client::Refusal::Limit {
                which: client::Limit::Admission { which: client::AdmissionLimit::Identity, bound: None }
            }))
        ));
        assert_eq!(session.driver.transport.sign_in(ClientId(0)), None);
        let applied = session.driver.world.store.applied;
        assert!(session.poll(now, wall).expect("no automatic retry").is_empty());
        assert_eq!(session.driver.world.store.applied, applied);
        assert!(session.sign_in_failure(ClientId(0)).is_some());
        // The explicit exchange resolves the owner seeded before sign-in,
        // despite the rejected identity's unused candidate number.
        session.driver.transport.replace_identity(ClientId(0), 1, identity());
        session.sign_in(ClientId(0), now, wall);
        assert!(session.driver.world.client_peer.as_ref().expect("actual external peer").delivered.iter().any(
            |(_, value)| matches!(
                value,
                root::Delivery::Core(core::Held::PeopleReply { reply: people::Reply::SignedIn { person: 1, .. }, .. })
            )
        ));
        assert!(
            take_signed_in(ClientId(0), session.poll(now, wall).expect("explicit admitted provider exchange"))
                .is_empty()
        );
        assert!(session.driver.transport.sign_in(ClientId(0)).is_some());
        assert!(session.sign_in_failure(ClientId(0)).is_none());
    }

    #[test]
    fn immediate_provider_capacity_is_one_original_terminal_and_does_not_consume_another_reply_owner() {
        let (mut session, now, wall) = session();
        session.bind_unsigned(ClientId(0), 1, identity(), now, wall);
        let _ = session.poll(now, wall).expect("Hello");
        let to = session.driver.transport.reply_to(Reply::SignIn { client: ClientId(0) }).into_token();
        session
            .immediate(core::Now::SignInRefused { to: skein_lib::ReplyTo::new(to) }, now, wall)
            .expect("actual capacity terminal");
        assert_eq!(session.sign_in_failure(ClientId(0)), Some(&packets::SignInFailure::Capacity));
        assert!(session.driver.transport.reply(to).is_none());
        assert_eq!(session.driver.transport.sign_in(ClientId(0)), None);
        assert!(session.poll(now, wall).expect("no callback or retry").is_empty());
        session
            .immediate(core::Now::SignInRefused { to: skein_lib::ReplyTo::new(to) }, now, wall)
            .expect("late terminal is already retired");
        let other = session.driver.transport.reply_to(Reply::SignOut { client: ClientId(0), sign_in: 1 }).into_token();
        assert_eq!(
            session.immediate(core::Now::SignInRefused { to: skein_lib::ReplyTo::new(other) }, now, wall),
            Err(Invalid::InternalPacket)
        );
        assert!(session.driver.transport.reply(other).is_some());
    }

    #[test]
    fn requests_wait_for_authentication_independently_of_me_durability() {
        let (mut session, now, wall) = session();
        session.driver.world.cut =
            Some(jig_core_world::effects::Cut::Submitted(session.driver.world.store.applied + 1));
        session.bind(ClientId(0), 1, identity(), now, wall);
        let native = Token::new(u64::MAX);
        session.send(ClientId(0), now, wall, frame(native)).expect("original request parks");
        assert_eq!(session.driver.transport.watch_token(ClientId(0), native), None);
        let callbacks = session.poll(now, wall).expect("Hello is protocol-owned");
        assert_eq!(callbacks.len(), 1, "actual callbacks: {callbacks:?}");
        assert!(matches!(
            &callbacks[0],
            (
                ClientId(0),
                Event::Client {
                    event: jig_client::Event::Up { up: jig_client::Up::Served { served: true, versions: None } }
                }
            )
        ));
        assert!(session.driver.withhold_completion().expect("actual authentication becomes durable").is_none());
        assert!(session.poll(now, wall).expect("completion withheld").is_empty());
        assert_eq!(session.observed(), vec![Observed::Durable { commit: session.driver.world.store.applied, at: now }]);
        assert!(session.observed().is_empty(), "one applied image is transferred once");
        session.driver.release_commits();
        let callbacks =
            take_signed_in(ClientId(0), session.poll(now, wall).expect("genuine session flushes original request"));
        assert_eq!(callbacks.len(), 1, "actual callbacks: {callbacks:?}");
        let (client, event) = &callbacks[0];
        assert_eq!(*client, ClientId(0));
        let Event::Client {
            event:
                jig_client::Event::Up {
                    up:
                        jig_client::Up::Watched { watch, event: client::WatchEvent::Delivered(client::Delivery::Me(value)) },
                },
        } = event
        else {
            panic!("actual whole Me")
        };
        assert_eq!(*watch, native);
        assert_eq!(value.party.number, 1);
        assert!(session.driver.transport.sign_in(ClientId(0)).is_some());
        assert!(session.poll(now, wall).expect("normal flight is still pending").is_empty());
        session.received(ClientId(0), now, wall, event);
        assert!(
            session.driver.transport.taken(ClientId(0), native).is_none(),
            "normal Taken was emitted once by the shell callback"
        );
    }

    #[test]
    fn real_protocol_loop_detaches_one_tabs_equal_tokens_and_discards_its_late_owner_callbacks() {
        let (mut session, now, wall) = session();
        session.bind(ClientId(0), 1, identity(), now, wall);
        session.bind(ClientId(1), 1, identity(), now, wall);
        let callbacks = session.poll(now, wall).expect("both genuine sign-ins");
        let callbacks = take_signed_in(ClientId(1), take_signed_in(ClientId(0), callbacks));
        assert_eq!(callbacks.len(), 2);
        let native = Token::new(9);
        for client in [ClientId(0), ClientId(1)] {
            session.send(client, now, wall, frame(native)).expect("original watch");
        }
        let first = session.driver.transport.watch_token(ClientId(0), native).expect("first route");
        let second = session.driver.transport.watch_token(ClientId(1), native).expect("second route");
        assert_ne!(first, second);
        session.detach(ClientId(0), now, wall);
        let callbacks = session.poll(now, wall).expect("old route cannot deliver late");
        assert_eq!(callbacks.len(), 1, "actual callbacks: {callbacks:?}");
        assert!(
            matches!(&callbacks[0], (ClientId(1), Event::Client { event: jig_client::Event::Up { up: jig_client::Up::Watched { watch, event: client::WatchEvent::Delivered(client::Delivery::Me(_)) } } }) if *watch == native),
            "actual callbacks: {callbacks:?}"
        );
        assert!(session.driver.transport.watch(first).is_none());
        assert!(session.driver.transport.watch(second).is_some());
    }

    #[test]
    fn real_protocol_loop_keeps_an_actual_unknown_watch_terminal_separate_from_normal_document_acknowledgement() {
        let (mut session, now, wall) = session();
        session.bind(ClientId(0), 1, identity(), now, wall);
        session.poll(now, wall).expect("genuine authentication");
        let native = Token::new(0);
        session
            .send(
                ClientId(0),
                now,
                wall,
                Request::Client {
                    request: jig_client::Request::Down {
                        down: jig_client::Down::Open {
                            watch: native,
                            subject: jig_client::Watchable::Jig(client::Subject::Run {
                                task: u64::MAX,
                                turn: None,
                                offset: 0,
                            }),
                        },
                    },
                },
            )
            .expect("owner decides visibility");
        let callbacks = session.poll(now, wall).expect("actual volatile terminal");
        assert_eq!(callbacks.len(), 1, "actual callbacks: {callbacks:?}");
        assert!(
            matches!(&callbacks[0], (ClientId(0), Event::Client { event: jig_client::Event::Up { up: jig_client::Up::WatchEnded { watch, end: client::WatchEnd::Unknown } } }) if *watch == native),
            "actual callbacks: {callbacks:?}"
        );
        assert!(session.driver.transport.watch_token(ClientId(0), native).is_none());
        assert!(session.driver.transport.taken(ClientId(0), native).is_none());
    }
    #[test]
    fn a_reloaded_epoch_discards_old_me_and_preserves_routes() {
        let (mut session, now, wall) = session();
        for client in [ClientId(0), ClientId(1)] {
            session.bind(client, 1, identity(), now, wall);
        }
        let callbacks = session.poll(now, wall).expect("both real sessions");
        let callbacks = take_signed_in(ClientId(1), take_signed_in(ClientId(0), callbacks));
        assert_eq!(callbacks.len(), 2);
        let native = Token::new(9);
        for client in [ClientId(0), ClientId(1)] {
            session.send(client, now, wall, frame(native)).expect("actual Me watch");
        }
        let old = session.driver.transport.watch_token(ClientId(0), native).expect("old tab route");
        let sibling = session.driver.transport.watch_token(ClientId(1), native).expect("sibling route");
        let packets = session.driver.poll(now, wall);
        assert_eq!(packets.len(), 2, "both actual owner Frames are held at the peer");
        session.reload(ClientId(0), now, wall);
        assert_eq!(session.next_due(), Some(Time::ZERO));
        assert!(session.driver.transport.watch(old).is_none());
        assert!(session.driver.transport.watch(sibling).is_some());
        for packet in packets {
            let Packet::Held { delivery, .. } = packet else { panic!("real held Me") };
            session.held(*delivery, now, wall).expect("old route discarded without acknowledging");
        }
        session.send(ClientId(0), now, wall, frame(native)).expect("root supplies its new watch");
        let reopened = session.driver.transport.watch_token(ClientId(0), native).expect("fresh route");
        assert_ne!(old, reopened);
        let callbacks =
            take_signed_in(ClientId(0), session.poll(now, wall).expect("actual fresh Me plus untouched sibling"));
        assert_eq!(callbacks.len(), 3);
        assert!(matches!(
            &callbacks[0],
            (ClientId(0), Event::Client { event: jig_client::Event::Up { up: jig_client::Up::Served { .. } } })
        ));
        let owners: Vec<_> = callbacks
            .iter()
            .filter_map(|(client, event)| {
                if matches!(
                    event,
                    Event::Client {
                        event: jig_client::Event::Up {
                            up: jig_client::Up::Watched {
                                event: client::WatchEvent::Delivered(client::Delivery::Me(_)),
                                ..
                            }
                        }
                    }
                ) {
                    Some(*client)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(owners, vec![ClientId(1), ClientId(0)]);
        assert_eq!(session.driver.transport.watch_token(ClientId(1), native), Some(sibling));
        assert!(session.driver.transport.taken(ClientId(0), native).is_some());
        assert!(session.driver.transport.taken(ClientId(0), native).is_none());
    }
    #[test]
    fn delayed_me_callbacks_preserve_the_flight_until_its_deadline() {
        let (mut session, now, wall) = session();
        session.bind(ClientId(0), 1, identity(), now, wall);
        session.poll(now, wall).expect("actual authentication");
        let native = Token::new(9);
        session.send(ClientId(0), now, wall, frame(native)).expect("original Me watch");
        let until = Time::from_nanos(1000);
        session.delay(until);
        assert!(session.poll(now, wall).expect("owner packets delayed at protocol boundary").is_empty());
        assert_eq!(session.delayed.len(), 1);
        assert!(session.driver.transport.watch_token(ClientId(0), native).is_some());
        assert!(session.poll(Time::from_nanos(999), wall).expect("before supplied deadline").is_empty());
        let callbacks = session.poll(until, wall).expect("same whole actual Me released");
        assert_eq!(callbacks.len(), 1);
        assert!(
            matches!(&callbacks[0], (ClientId(0), Event::Client { event: jig_client::Event::Up { up: jig_client::Up::Watched {
            watch, event: client::WatchEvent::Delivered(client::Delivery::Me(value)),
        } } }) if *watch == native && value.party.number == 1)
        );
        session.received(ClientId(0), until, wall, &callbacks[0].1);
        assert!(session.driver.transport.taken(ClientId(0), native).is_none());
        assert!(session.poll(until, wall).expect("no duplicate callback").is_empty());
    }

    #[test]
    fn dropping_a_watch_preserves_sibling_me_deliveries() {
        let (mut session, now, wall) = session();
        for client in [ClientId(0), ClientId(1)] {
            session.bind(client, 1, identity(), now, wall);
        }
        session.poll(now, wall).expect("actual independent sessions");
        let native = Token::new(9);
        for client in [ClientId(0), ClientId(1)] {
            session.send(client, now, wall, frame(native)).expect("real Me watches");
        }
        let dropped = session.driver.transport.watch_token(ClientId(0), native).expect("original route");
        let sibling =
            session.driver.transport.watch_token(ClientId(1), native).expect("independent equal native token");
        session.drop_watch(ClientId(0), native, now, wall).expect("actual external protocol drop");
        let callbacks = session.poll(now, wall).expect("late original Me is discarded");
        assert_eq!(callbacks.len(), 2);
        assert!(
            matches!(&callbacks[0], (ClientId(0), Event::Client { event: jig_client::Event::Up { up: jig_client::Up::WatchEnded { watch, end: client::WatchEnd::Dropped } } }) if *watch == native)
        );
        assert!(
            matches!(&callbacks[1], (ClientId(1), Event::Client { event: jig_client::Event::Up { up: jig_client::Up::Watched { watch, event: client::WatchEvent::Delivered(client::Delivery::Me(_)) } } }) if *watch == native)
        );
        assert!(session.driver.transport.watch(dropped).is_none());
        assert!(session.driver.transport.watch(sibling).is_some());
        assert!(session.driver.transport.taken(ClientId(0), native).is_none());
        session.received(ClientId(1), now, wall, &callbacks[1].1);
        assert!(session.driver.transport.taken(ClientId(1), native).is_none());
    }

    #[test]
    fn a_failed_actual_commit_retires_old_flights_without_an_answer_and_parks_exact_retries_until_cold_restart() {
        let (mut session, now, wall) = session();
        session.bind(ClientId(0), 1, identity(), now, wall);
        session.poll(now, wall).expect("genuine authentication");
        let _ = session.observed();
        let native = Token::new(u64::MAX);
        session.send(ClientId(0), now, wall, frame(native)).expect("original Me watch");
        let watcher = session.driver.transport.watch_token(ClientId(0), native).expect("original route");
        let durable = session.driver.world.store.applied;
        let rows = session.driver.world.store.rows.clone();
        session
            .driver
            .world
            .store
            .fault(jig_core_world::Fault::Fail { commit: durable.checked_add(1).expect("next actual transaction") });
        let key = [17; 16];
        let request = |attempt| Request::Client {
            request: jig_client::Request::Down {
                down: jig_client::Down::Send {
                    attempt,
                    key,
                    request: jig_client::Ask::Jig(Box::new(client::Ask::Stop { project: 1, task: u64::MAX })),
                    value: None,
                },
            },
        };
        session.send(ClientId(0), now, wall, request(Token::new(31))).expect("actual owner submit");
        assert!(session.driver.world.stopped, "existing Failed completion stops the real root");
        let callbacks = session.poll(now, wall).expect("actual connection departure");
        assert!(
            matches!(callbacks.as_slice(), [(ClientId(0), Event::Client { event: jig_client::Event::Up { up: jig_client::Up::WatchEnded {
            watch, end: client::WatchEnd::Dropped } } })] if *watch == native)
        );
        assert!(session.driver.transport.watch(watcher).is_none());
        assert!(session.driver.transport.taken(ClientId(0), native).is_none());
        assert_eq!(session.driver.world.store.applied, durable);
        assert_eq!(session.driver.world.store.rows, rows);
        assert_eq!(session.observed(), vec![Observed::Reachable { live: false }]);
        assert!(session.poll(now, wall).expect("stopped root has no fabricated replies").is_empty());
        assert!(session.observed().is_empty());
        assert_eq!(session.next_due(), None);
        let retry = Token::new(32);
        session.send(ClientId(0), now, wall, request(retry)).expect("exact later attempt is parked");
        assert_eq!(session.parked.get(&ClientId(0)).expect("parked retry"), &[request(retry)]);
        assert!(session.poll(now, wall).expect("no timed fake recovery").is_empty());
        let until = now.saturating_add(skein_lib::Duration::from_secs(1));
        session.restart(jig_core_world::world::config(8455), now, wall, until).expect("actual cold root");
        assert!(session.poll(now, wall).expect("supplied offline span").is_empty());
        let callbacks = session.poll(until, wall).expect("real restart and original retry");
        assert!(callbacks.iter().any(|(client, value)| *client == ClientId(0)
            && matches!(value, Event::Client { event: jig_client::Event::Up { up: jig_client::Up::Answer { attempt, envelope: skein_lib::RequestEnvelope::Final, answer: Some(_), .. } } } if *attempt == retry)));
        assert!(matches!(
            callbacks.first(),
            Some((ClientId(0), Event::Client { event: jig_client::Event::Up { up: jig_client::Up::Served { .. } } }))
        ));
        assert_eq!(
            session
                .observed()
                .iter()
                .filter(|value| matches!(value, Observed::Made { key: actual, .. } if *actual == key))
                .count(),
            1
        );
    }

    #[test]
    fn a_lost_actual_final_answer_replays_the_original_owner_key_without_a_second_durable_made_fact() {
        let (mut session, now, wall) = session();
        session.bind(ClientId(0), 1, identity(), now, wall);
        session.poll(now, wall).expect("genuine authentication");
        let _ = session.observed();
        let first = Token::new(31);
        let retry = Token::new(32);
        let key = [255; 16];
        let request = |attempt| Request::Client {
            request: jig_client::Request::Down {
                down: jig_client::Down::Send {
                    attempt,
                    key,
                    request: jig_client::Ask::Jig(Box::new(client::Ask::Stop { project: 1, task: u64::MAX })),
                    value: None,
                },
            },
        };
        session.lose_answer(ClientId(0), first);
        session.send(ClientId(0), now, wall, request(first)).expect("owner receives whole original ask");
        assert!(session.poll(now, wall).expect("only original final callback is lost").is_empty());
        let made = session.observed();
        assert_eq!(
            made.iter()
                .filter(|value| matches!(value, Observed::Made { party: 1, key: actual, .. } if *actual == key))
                .count(),
            1
        );
        assert!(matches!(made.first(), Some(Observed::Durable { .. })));
        session.send(ClientId(0), now, wall, request(retry)).expect("new attempt retains original key");
        let callbacks = session.poll(now, wall).expect("actual owner replays retained whole answer");
        assert_eq!(callbacks.len(), 1);
        assert!(
            matches!(&callbacks[0], (ClientId(0), Event::Client { event: jig_client::Event::Up { up: jig_client::Up::Answer { attempt, envelope: skein_lib::RequestEnvelope::Final, answer: Some(_), .. } } }) if *attempt == retry)
        );
        assert!(
            session
                .observed()
                .iter()
                .all(|value| !matches!(value, Observed::Made { key: actual, .. } if *actual == key))
        );
        assert!(session.poll(now, wall).expect("callback is consumed once").is_empty());
    }
    #[test]
    fn the_original_owner_sign_out_reply_waits_for_its_actual_commit_and_preserves_another_tabs_session() {
        let (mut session, now, wall) = session();
        for client in [ClientId(0), ClientId(1)] {
            session.bind(client, 1, identity(), now, wall);
        }
        session.poll(now, wall).expect("two genuine independent sessions");
        let first = session.driver.transport.sign_in(ClientId(0)).expect("first owner session");
        let sibling = session.driver.transport.sign_in(ClientId(1)).expect("sibling owner session");
        assert_ne!(first, sibling);
        session.driver.world.cut =
            Some(jig_core_world::effects::Cut::Submitted(session.driver.world.store.applied + 1));
        session.sign_out(ClientId(0), now, wall);
        assert!(session.driver.withhold_completion().expect("actual session erasure is durable").is_none());
        assert!(session.poll(now, wall).expect("withheld actual sign-out completion").is_empty());
        assert_eq!(session.driver.transport.sign_in(ClientId(0)), Some(first));
        session.driver.release_commits();
        let callbacks = session.poll(now, wall).expect("durable original sign-out reply");
        assert_eq!(callbacks.len(), 1);
        assert!(matches!(&callbacks[0], (ClientId(0), Event::SignedOut)));
        assert_eq!(session.driver.transport.sign_in(ClientId(0)), None);
        assert_eq!(session.driver.transport.sign_in(ClientId(1)), Some(sibling));
        assert!(session.driver.world.store.rows.values().all(|row| !matches!(row,
            root::Record::Core(core::Record::People(people::Stored::SignIn { number, .. })) if *number == first)));
        assert!(session.driver.world.store.rows.values().any(|row| matches!(row,
            root::Record::Core(core::Record::People(people::Stored::SignIn { number, .. })) if *number == sibling)));
    }

    #[test]
    fn cold_root_restart_parks_the_original_keyed_attempt_until_the_supplied_deadline_and_replays_one_durable_answer() {
        let (mut session, now, wall) = session();
        session.bind(ClientId(0), 1, identity(), now, wall);
        session.poll(now, wall).expect("genuine authentication");
        let sign_in = session.driver.transport.sign_in(ClientId(0));
        let _ = session.observed();
        let key = [211; 16];
        let request = |attempt| Request::Client {
            request: jig_client::Request::Down {
                down: jig_client::Down::Send {
                    attempt,
                    key,
                    request: jig_client::Ask::Jig(Box::new(client::Ask::Stop { project: 1, task: u64::MAX })),
                    value: None,
                },
            },
        };
        let first = Token::new(311);
        session.send(ClientId(0), now, wall, request(first)).expect("actual original attempt");
        session.poll(now, wall).expect("first actual answer");
        let made = session.observed();
        assert_eq!(
            made.iter().filter(|value| matches!(value, Observed::Made { key: actual, .. } if *actual == key)).count(),
            1
        );
        let until = now.saturating_add(skein_lib::Duration::from_secs(1));
        session.restart(jig_core_world::world::config(8455), now, wall, until).expect("actual cold restart");
        assert!(session.offline());
        assert_eq!(session.next_due(), Some(until));
        let retry = Token::new(312);
        let before = session.driver.world.store.rows.clone();
        session.send(ClientId(0), now, wall, request(retry)).expect("whole retry is parked");
        assert!(session.poll(now, wall).expect("offline source sends no answer").is_empty());
        assert_eq!(session.driver.world.store.rows, before);
        assert_eq!(session.driver.transport.sign_in(ClientId(0)), sign_in);
        let corrected_wall = Wall::from_nanos(wall.as_nanos() - 1);
        let callbacks = session.poll(until, corrected_wall).expect("actual recovery replays retained key");
        assert!(!session.offline());
        assert!(matches!(
            &callbacks[0],
            (ClientId(0), Event::Client { event: jig_client::Event::Up { up: jig_client::Up::Served { .. } } })
        ));
        assert_eq!(callbacks.iter().filter(|(_, value)| matches!(value, Event::Client { event: jig_client::Event::Up { up: jig_client::Up::Answer { attempt, envelope: skein_lib::RequestEnvelope::Final, answer: Some(_), .. } } } if *attempt == retry)).count(), 1);
        assert!(
            session
                .observed()
                .iter()
                .all(|value| !matches!(value, Observed::Made { key: actual, .. } if *actual == key))
        );
        assert_eq!(session.driver.world.clocks(), (until, corrected_wall));
        assert!(session.poll(until, corrected_wall).expect("callback transferred once").is_empty());
    }

    #[test]
    fn cold_restart_drops_old_normal_callbacks_and_equal_sibling_watch_tokens_without_taking_replacement_flights() {
        let (mut session, now, wall) = session();
        for client in [ClientId(0), ClientId(1)] {
            session.bind(client, 1, identity(), now, wall);
        }
        session.poll(now, wall).expect("actual sibling authentication");
        let native = Token::new(u64::MAX);
        let open = || Request::Client {
            request: jig_client::Request::Down {
                down: jig_client::Down::Open {
                    watch: native,
                    subject: jig_client::Watchable::Jig(client::Subject::Me),
                },
            },
        };
        for client in [ClientId(0), ClientId(1)] {
            session.send(client, now, wall, open()).expect("original frame watch");
        }
        let old = session.poll(now, wall).expect("normal old flights");
        assert_eq!(old.len(), 2);
        let owner = session.driver.transport.watch_token(ClientId(0), native).expect("original owner token");
        let until = now.saturating_add(skein_lib::Duration::from_secs(1));
        session.restart(jig_core_world::world::config(8455), now, wall, until).expect("cold root restart");
        let dropped = session.poll(now, wall).expect("only original departed watches");
        assert_eq!(dropped.len(), 2);
        for client in [ClientId(0), ClientId(1)] {
            assert!(dropped.iter().any(|(actual, value)| *actual == client && matches!(value, Event::Client { event: jig_client::Event::Up { up: jig_client::Up::WatchEnded { watch, end: client::WatchEnd::Dropped } } } if *watch == native)));
            assert!(session.driver.transport.watch_token(client, native).is_none());
        }
        session.poll(until, wall).expect("connection recovers");
        for client in [ClientId(0), ClientId(1)] {
            session.send(client, until, wall, open()).expect("replacement open after recovery");
        }
        let callbacks = session.poll(until, wall).expect("new normal flights");
        let replacement = session.driver.transport.watch_token(ClientId(0), native).expect("replacement owner token");
        assert_ne!(replacement, owner);
        let current = callbacks
            .iter()
            .find(|(client, value)| {
                *client == ClientId(0)
                    && matches!(
                        value,
                        Event::Client {
                            event: jig_client::Event::Up {
                                up: jig_client::Up::Watched { event: client::WatchEvent::Delivered(_), .. }
                            }
                        }
                    )
            })
            .expect("new frame");
        // Old owner packets are no longer transferred after disconnect. Only
        // receipt of a current callback releases the new normal flight.
        assert!(session.driver.transport.watch(owner).is_none());
        session.received(current.0, until, wall, &current.1);
        assert!(session.driver.transport.taken(ClientId(0), native).is_none());
        assert!(matches!(session.driver.transport.taken(ClientId(1), native), Some(core::Event::Delivered { .. })));
    }
    mod carried;
    mod llm;
    mod offline;
    mod project_names;
}

mod received;
