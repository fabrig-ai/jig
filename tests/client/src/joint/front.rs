//! The generic client world over the real session loop (testing.md, 5–7).
//! Scenario identities and the clock mapping are supplied independently of
//! owner numbers. Unsupported fault sources remain named until implemented.
use super::session::{Invalid, Session};
use crate::{
    harness::{ClientId, Front, FrontFault, Observed},
    test::Test,
};
use jig_client_test_domain::{Event, Request, first};
use jig_domain_people::Identity;
use skein_lib::{Duration, Time, Token, Wall};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingFault {
    Busy,
    NotReady,
    Restart,
    Versions,
    ObjectGone,
}

#[derive(Debug)]
pub enum FaultInvalid {
    Packet(Invalid),
    Pending(PendingFault),
}

/// Ordinary world adapter, with the real root beneath its protocol session.
pub struct Joint {
    pub session: Session,
    identities: BTreeMap<u64, Identity>,
    unsigned_on_bind: bool,
    clock: fn(Time) -> Wall,
    now: Time,
    wall: Wall,
    last_client: Option<ClientId>,
    last_watch: Option<(ClientId, Token)>,
    last_attempt: Option<(ClientId, Token)>,
    version_fault: Option<jig_client_domain::contract::VersionRange>,
    restart_configuration: Option<fn() -> jig_test_domain::Config>,
    busy_pressure: Option<fn() -> jig_fake_parties::Party>,
    not_ready_restore: Option<(fn() -> jig_test_domain::Config, Duration)>,
    restore_until: Option<Time>,
    observations: Vec<Observed>,
}

impl Joint {
    /// The fixture supplies actual configured identities and its wall clock.
    #[must_use]
    pub fn new(session: Session, identities: BTreeMap<u64, Identity>, clock: fn(Time) -> Wall) -> Self {
        let (now, wall) = session.driver.world.clocks();
        Self {
            session,
            identities,
            unsigned_on_bind: false,
            clock,
            now,
            wall,
            last_client: None,
            last_watch: None,
            last_attempt: None,
            version_fault: None,
            restart_configuration: None,
            busy_pressure: None,
            not_ready_restore: None,
            restore_until: None,
            observations: Vec::new(),
        }
    }

    /// Carry an unlinked client's contract operation to the real core.
    /// # Errors
    /// Returns work outside this transport's declared Jig owner intact.
    pub fn down(
        &mut self,
        client: ClientId,
        now: Time,
        wall: Wall,
        down: jig_client::Down,
    ) -> Result<(), jig_client::Down> {
        let request = super::transport::contract_request(down)?;
        Front::<Test>::wire(self, client, now, wall, request);
        Ok(())
    }

    /// Poll original core callbacks for an unlinked client with the supplied owners.
    /// This front serves Jig; undeclared family ranges are never invented.
    pub fn up(&mut self, now: Time, wall: Wall, owners: &[jig_client::Spoken]) -> Vec<(ClientId, jig_client::Up)> {
        let mut up = Vec::new();
        for (client, event) in Front::<Test>::poll(self, now, wall) {
            if let Event::Client {
                event: jig_client::Event::Up { up: jig_client::Up::Served { served, versions: None } },
            } = &event
            {
                up.push((
                    client,
                    jig_client::Up::Served { served: *served, versions: Some(self.session.driver.transport.served()) },
                ));
                for owner in owners {
                    up.push((client, jig_client::Up::FamilyServed { family: owner.family, versions: None }));
                }
            } else {
                let callback = super::transport::contract_event(&event).expect("original contract transport callback");
                Front::<Test>::received(self, client, &event);
                up.push((client, callback));
            }
        }
        up
    }

    /// Start without a protocol secret so the person initiates the real provider exchange.
    pub fn unsigned_on_bind(&mut self) {
        self.unsigned_on_bind = true;
    }

    /// Configure the actual incompatible served range for a version fault.
    pub fn version_fault(&mut self, versions: jig_client_domain::contract::VersionRange) {
        self.version_fault = Some(versions);
    }

    /// Supply the scenario's actual cold-root configuration independently of
    /// business state and retained protocol documents.
    pub fn restart_configuration(&mut self, configuration: fn() -> jig_test_domain::Config) {
        self.restart_configuration = Some(configuration);
    }

    /// The scenario supplies real scripted asks and matching bounded engine
    /// capacity. This installs pressure; only the engine can answer Busy.
    pub fn busy_pressure(&mut self, party: fn() -> jig_fake_parties::Party) {
        self.busy_pressure = Some(party);
    }

    /// Supply the actual cold-root configuration and original page release
    /// delay for a `NotReady` fault, independently of engine business state.
    pub fn not_ready_restore(&mut self, configuration: fn() -> jig_test_domain::Config, delay: Duration) {
        self.not_ready_restore = Some((configuration, delay));
    }

    /// Apply a fault only through an implemented real boundary. Missing owner
    /// sources cannot manufacture a refusal, disappearance or durable answer.
    /// # Errors
    /// Returns the exact unimplemented fault or an original routing error.
    pub fn try_fault(&mut self, now: Time, fault: &FrontFault) -> Result<(), FaultInvalid> {
        self.now = now;
        self.wall = (self.clock)(now);
        match fault {
            FrontFault::Slow { span } => self.session.delay(now.saturating_add(*span)),
            FrontFault::NoAnswer => {
                if let Some((client, attempt)) = self.last_attempt {
                    self.session.lose_answer(client, attempt);
                }
            }
            FrontFault::DropWatch => {
                if let Some((client, watch)) = self.last_watch {
                    self.session.drop_watch(client, watch, now, self.wall).map_err(FaultInvalid::Packet)?;
                }
            }
            FrontFault::SignedOut => {
                if let Some(client) = self.last_client {
                    self.session.sign_out(client, now, self.wall);
                }
            }
            FrontFault::Busy => {
                let Some(party) = self.busy_pressure else {
                    return Err(FaultInvalid::Pending(PendingFault::Busy));
                };
                self.session.driver.scripted_party(party(), now, self.wall);
            }
            FrontFault::NotReady => {
                let Some((configuration, delay)) = self.not_ready_restore else {
                    return Err(FaultInvalid::Pending(PendingFault::NotReady));
                };
                self.session.driver.pause_restore(configuration(), now, self.wall);
                self.restore_until = Some(now.saturating_add(delay));
            }
            FrontFault::Restart { offline } => {
                let Some(configuration) = self.restart_configuration else {
                    return Err(FaultInvalid::Pending(PendingFault::Restart));
                };
                self.session
                    .restart(configuration(), now, self.wall, now.saturating_add(*offline))
                    .map_err(FaultInvalid::Packet)?;
                self.observations.push(Observed::Reachable { live: false });
            }
            FrontFault::Versions => {
                let Some(versions) = self.version_fault else {
                    return Err(FaultInvalid::Pending(PendingFault::Versions));
                };
                self.session.versions(versions);
            }
            FrontFault::ObjectGone => return Err(FaultInvalid::Pending(PendingFault::ObjectGone)),
        }
        Ok(())
    }
}

impl Front<Test> for Joint {
    fn bind(&mut self, client: ClientId, party: u64) {
        let identity = self.identities.get(&party).expect("fixture supplies the actual authenticated identity").clone();
        self.last_client = Some(client);
        if self.unsigned_on_bind {
            self.session.bind_unsigned(client, party, identity, self.now, self.wall);
        } else {
            self.session.bind(client, party, identity, self.now, self.wall);
        }
    }
    fn wire(&mut self, client: ClientId, now: Time, wall: Wall, request: Request) {
        self.now = now;
        self.wall = wall;
        self.last_client = Some(client);
        match &request {
            Request::Client { request: jig_client::Request::Down { down: jig_client::Down::Send { attempt, .. } } } => {
                self.last_attempt = Some((client, *attempt));
            }
            Request::Client { request: jig_client::Request::Down { down: jig_client::Down::Open { watch, .. } } }
            | Request::First(first::Out::Open { watch, .. }) => {
                self.last_watch = Some((client, *watch));
            }
            Request::First(_)
            | Request::Save { .. }
            | Request::Erase { .. }
            | Request::Show { .. }
            | Request::SignOut
            | Request::SignIn { .. }
            | Request::Fact { .. }
            | Request::Client { .. } => {}
        }
        self.session.send(client, now, wall, request).expect("complete real outbound translation");
    }
    fn poll(&mut self, now: Time, wall: Wall) -> Vec<(ClientId, Event)> {
        self.now = now;
        self.wall = wall;
        if self.restore_until.is_some_and(|until| now >= until) {
            self.restore_until = None;
            self.session.driver.finish_restore(now, wall);
        }
        let offline = self.session.offline();
        let callbacks = self.session.poll(now, wall).expect("complete real inbound translation");
        if offline && !self.session.offline() {
            self.observations.push(Observed::Reachable { live: true });
        }
        callbacks
    }
    fn next_due(&self) -> Option<Time> {
        self.session.next_due().into_iter().chain(self.restore_until).min()
    }
    fn observed(&mut self) -> Vec<Observed> {
        let mut values = std::mem::take(&mut self.observations);
        values.extend(self.session.observed());
        values
    }
    fn fault(&mut self, now: Time, fault: FrontFault) {
        self.try_fault(now, &fault).expect("selected story requires an implemented actual fault source");
    }
    fn reload(&mut self, client: ClientId, now: Time) {
        self.now = now;
        self.wall = (self.clock)(now);
        self.session.reload(client, now, self.wall);
    }
    fn delivers(&mut self, client: ClientId, event: &Event) -> bool {
        !matches!(event, Event::SignedOut) || self.session.delivers_sign_out(client)
    }
    fn received(&mut self, client: ClientId, event: &Event) {
        self.session.received(client, self.now, self.wall, event);
    }
    fn sign_out(&mut self, client: ClientId, now: Time, wall: Wall) {
        self.now = now;
        self.wall = wall;
        self.last_client = Some(client);
        self.session.sign_out(client, now, wall);
    }
    fn sign_in(&mut self, client: ClientId, now: Time, wall: Wall) {
        self.now = now;
        self.wall = wall;
        self.last_client = Some(client);
        self.session.sign_in(client, now, wall);
    }
}

/// The shared real-core front fixture, with externally supplied owner limits.
#[must_use]
pub fn fixture(owner_limits: &jig_test_domain::Limits) -> Joint {
    use crate::joint::engine::Driver;
    use jig_client_domain::contract as client;
    use jig_core_world::peers::Peers;
    use jig_domain_people as people;

    let driver = Driver::new(
        jig_core_world::world::config(8455),
        *owner_limits,
        Peers::new(Vec::new(), Vec::new()),
        Time::ZERO,
        super::fixture::clock(Time::ZERO),
    );
    let limits = crate::test::limits();
    let codec = jig_protocol_transcript::Limits {
        smith: smith_transcript::v6::CEILINGS,
        bytes: 1_048_576,
        turn: client::TurnLimits { messages: 64, blocks: 16, text: 65_536 },
    };
    let identity = Identity {
        key: people::IdentityKey { provider: 0, subject: 7_u64.to_be_bytes().into() },
        login: b"login".as_slice().into(),
        name: b"Person".as_slice().into(),
    };
    Joint::new(Session::new(driver, limits, codec), BTreeMap::from([(1, identity)]), super::fixture::clock)
}

#[cfg(test)]
mod tests {
    use super::*;
    use jig_client_domain::contract as client;
    use jig_domain_people as people;

    fn clock(now: Time) -> Wall {
        Wall::from_nanos(1_000_000_000_000 + now.as_nanos())
    }

    fn front() -> Joint {
        front_with_limits(&jig_core_world::world::limits())
    }

    fn front_with_limits(owner_limits: &jig_test_domain::Limits) -> Joint {
        fixture(owner_limits)
    }

    fn answered_sign_out() -> (Joint, u64) {
        let mut front = front();
        let client = ClientId(0);
        Front::<Test>::bind(&mut front, client, 1);
        Front::<Test>::poll(&mut front, Time::ZERO, clock(Time::ZERO));
        let first = front.session.driver.transport.sign_in(client).expect("bound session");
        Front::<Test>::sign_out(&mut front, client, Time::ZERO, clock(Time::ZERO));
        for _ in 0..20 {
            let now = Front::<Test>::next_due(&front).unwrap_or(Time::ZERO);
            let callbacks = Front::<Test>::poll(&mut front, now, clock(now));
            if callbacks.iter().any(|(owner, event)| *owner == client && matches!(event, Event::SignedOut)) {
                return (front, first);
            }
        }
        panic!("the real sign-out must answer");
    }

    #[test]
    fn a_sign_out_answered_after_a_newer_sign_in_is_not_delivered() {
        let (mut front, first) = answered_sign_out();
        let client = ClientId(0);
        let now = front.now;
        Front::<Test>::sign_in(&mut front, client, now, clock(now));
        for _ in 0..20 {
            let now = Front::<Test>::next_due(&front).unwrap_or(now);
            Front::<Test>::poll(&mut front, now, clock(now));
            if front.session.driver.transport.sign_in(client).is_some_and(|sign_in| sign_in != first) {
                break;
            }
        }
        assert_eq!(
            (
                Front::<Test>::delivers(&mut front, client, &Event::SignedOut),
                front.session.driver.transport.sign_in(client)
            ),
            (false, Some(2)),
        );
    }

    #[test]
    fn a_sign_out_answered_with_no_newer_sign_in_is_delivered() {
        let (mut front, _) = answered_sign_out();
        let client = ClientId(0);
        assert_eq!(
            (
                Front::<Test>::delivers(&mut front, client, &Event::SignedOut),
                front.session.driver.transport.sign_in(client)
            ),
            (true, None),
        );
    }

    #[test]
    fn an_unsigned_protocol_client_signs_in_only_when_the_person_chooses_the_visible_provider() {
        use crate::{Settings, Until, World};
        use skein_fake_person::{Find, Step};
        use skein_ui::Role;
        let mut limits = jig_core_world::world::limits();
        limits.core.documents.rows = 2;
        limits.core.documents.lines = 2;
        limits.core.documents.chips = 2;
        limits.core.documents.text_bytes = 32;
        let mut front = front_with_limits(&limits);
        front.unsigned_on_bind();
        let original_rows = front.session.driver.world.store.rows.clone();
        let original_applied = front.session.driver.world.store.applied;
        let mut world = World::<Test, Joint>::new(Settings::default(), front);
        let client = world.client(1, crate::stories::conversation());
        world.run(Until::Time(Time::from_nanos(20_000_000)));
        assert_eq!(world.front().session.driver.transport.sign_in(client), None);
        assert_eq!(world.front().session.driver.world.store.rows, original_rows);
        assert_eq!(world.front().session.driver.world.store.applied, original_applied);
        assert!(world.tree(client).nodes().iter().any(|node| {
            node.role == Role::Button && node.name.as_deref() == Some(b"Sign in with Test".as_slice())
        }));
        world.person(
            client,
            vec![
                Step::See { find: Find::named(Role::Button, b"Sign in with Test"), within: Duration::from_secs(1) },
                Step::Press { find: Find::named(Role::Button, b"Sign in with Test") },
                Step::See { find: Find::named(Role::TextArea, b"First words"), within: Duration::from_secs(1) },
            ],
        );
        let outcome = world.run(Until::Done);
        assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed);
        assert!(world.front().session.driver.transport.sign_in(client).is_some());
        assert!(world.front().session.driver.world.store.applied > original_applied);
    }

    use crate::joint::take_signed_in;

    #[test]
    fn the_generic_real_front_binds_a_supplied_identity_and_transfers_actual_durability_before_owner_callbacks() {
        let mut front = front();
        Front::<Test>::bind(&mut front, ClientId(0), 1);
        let callbacks = take_signed_in(ClientId(0), Front::<Test>::poll(&mut front, Time::ZERO, clock(Time::ZERO)));
        assert_eq!(callbacks.len(), 1);
        assert!(matches!(
            &callbacks[0],
            (ClientId(0), Event::Client { event: jig_client::Event::Up { up: jig_client::Up::Served { .. } } })
        ));
        let observations = Front::<Test>::observed(&mut front);
        assert!(matches!(observations.first(), Some(Observed::Durable { at: Time::ZERO, .. })));
        assert!(Front::<Test>::observed(&mut front).is_empty());
        assert!(front.session.driver.transport.sign_in(ClientId(0)).is_some());
        Front::<Test>::fault(&mut front, Time::ZERO, FrontFault::SignedOut);
        let callbacks = Front::<Test>::poll(&mut front, Time::ZERO, clock(Time::ZERO));
        assert!(matches!(callbacks.as_slice(), [(ClientId(0), Event::SignedOut)]));
        assert_eq!(front.session.driver.transport.sign_in(ClientId(0)), None);
    }

    #[test]
    fn the_real_front_names_each_missing_fault_source_without_manufacturing_an_owner_outcome() {
        let mut front = front();
        let before = front.session.driver.world.store.rows.clone();
        for (fault, pending) in [
            (FrontFault::Busy, PendingFault::Busy),
            (FrontFault::NotReady, PendingFault::NotReady),
            (FrontFault::Restart { offline: skein_lib::Duration::from_secs(1) }, PendingFault::Restart),
            (FrontFault::Versions, PendingFault::Versions),
            (FrontFault::ObjectGone, PendingFault::ObjectGone),
        ] {
            assert!(
                matches!(front.try_fault(Time::ZERO, &fault), Err(FaultInvalid::Pending(actual)) if actual == pending)
            );
        }
        assert_eq!(front.session.driver.world.store.rows, before);
        assert!(Front::<Test>::observed(&mut front).is_empty());
    }

    fn pressure_party() -> jig_fake_parties::Party {
        use jig_fake_parties::{Act, Ask, Party, Role, Task};
        Party::new(
            1,
            Role::Owner,
            1_000_000,
            Box::new([
                Act::SignIn { provider: 0, subject: b"ordinary pressure".as_slice().into() },
                Act::Request { key: [61; 16], ask: Ask::Stop { task: Task::Number(u64::MAX) } },
            ]),
        )
    }

    #[test]
    fn a_real_busy_front_uses_scripted_capacity_and_recovers_the_same_unsaved_key_after_owner_expiry() {
        use skein_lib::{Duration, RequestEnvelope};
        let mut owner_limits = jig_core_world::world::limits();
        owner_limits.core.people.requests = 1;
        owner_limits.core.people.request_retention = Duration::from_secs(1);
        let mut front = front_with_limits(&owner_limits);
        Front::<Test>::bind(&mut front, ClientId(0), 1);
        let initial = take_signed_in(ClientId(0), Front::<Test>::poll(&mut front, Time::ZERO, clock(Time::ZERO)));
        assert!(matches!(
            initial.as_slice(),
            [(ClientId(0), Event::Client { event: jig_client::Event::Up { up: jig_client::Up::Served { .. } } })]
        ));
        let _ = Front::<Test>::observed(&mut front);
        front.busy_pressure(pressure_party);
        front.try_fault(Time::ZERO, &FrontFault::Busy).expect("actual scripted source");
        let party = &front.session.driver.world.peers.as_ref().expect("ordinary peers").parties[0];
        assert!(party.quiescent());
        assert_eq!(party.replies.len(), 2);
        assert_eq!(party.replies[1], (1_000_001, jig_fake_parties::Reply::Refused(jig_fake_parties::Refusal::Unknown)));
        assert!(Front::<Test>::poll(&mut front, Time::ZERO, clock(Time::ZERO)).is_empty());
        let pressure = Front::<Test>::observed(&mut front);
        assert!(pressure.iter().any(|value| matches!(value, Observed::Made { key, .. } if *key == [61; 16])));
        let applied = front.session.driver.world.store.applied;
        let rows = front.session.driver.world.store.rows.clone();
        let request = |attempt| Request::Client {
            request: jig_client::Request::Down {
                down: jig_client::Down::Send {
                    attempt,
                    key: [62; 16],
                    request: jig_client::Ask::Jig(Box::new(client::Ask::Stop { project: 1, task: u64::MAX })),
                    value: None,
                },
            },
        };
        Front::<Test>::wire(&mut front, ClientId(0), Time::ZERO, clock(Time::ZERO), request(Token::new(31)));
        let peer = front.session.driver.world.client_peer.as_ref().expect("registered protocol peer");
        assert!(peer.delivered.iter().any(|(_, value)| matches!(
            value,
            jig_test_domain::Delivery::Core(jig_domain_core::Held::PeopleReply {
                reply: people::Reply::Again(people::Again::Busy),
                ..
            })
        )));
        let callbacks = Front::<Test>::poll(&mut front, Time::ZERO, clock(Time::ZERO));
        assert!(
            matches!(callbacks.as_slice(), [(ClientId(0), Event::Client { event: jig_client::Event::Up { up: jig_client::Up::Answer {
            attempt, envelope: RequestEnvelope::Again, answer: None, why: None } } })] if *attempt == Token::new(31))
        );
        assert_eq!(front.session.driver.world.store.rows, rows);
        assert_eq!(front.session.driver.world.store.applied, applied);
        assert!(Front::<Test>::observed(&mut front).is_empty(), "Busy did not make or keep the client key");
        let later = Time::ZERO.saturating_add(Duration::from_secs(1));
        assert!(Front::<Test>::poll(&mut front, later, clock(later)).is_empty(), "owner expiry is not a final answer");
        Front::<Test>::wire(&mut front, ClientId(0), later, clock(later), request(Token::new(32)));
        let callbacks = Front::<Test>::poll(&mut front, later, clock(later));
        assert!(
            matches!(callbacks.as_slice(), [(ClientId(0), Event::Client { event: jig_client::Event::Up { up: jig_client::Up::Answer {
            attempt, envelope: RequestEnvelope::Final, answer: Some(_), why: None } } })] if *attempt == Token::new(32))
        );
        let observations = Front::<Test>::observed(&mut front);
        assert_eq!(
            observations
                .iter()
                .filter(|value| matches!(value,
            Observed::Made { party: 1, key, .. } if *key == [62; 16]))
                .count(),
            1
        );
        assert!(observations.iter().all(|value| !matches!(value, Observed::Kept { .. })));
    }
    #[test]
    fn a_fresh_provider_sign_in_keeps_actual_not_ready_without_a_session_and_requires_another_supplied_attempt() {
        let mut front = front();
        front.unsigned_on_bind();
        Front::<Test>::bind(&mut front, ClientId(0), 1);
        let _ = Front::<Test>::poll(&mut front, Time::ZERO, clock(Time::ZERO));
        let _ = Front::<Test>::observed(&mut front);
        let rows = front.session.driver.world.store.rows.clone();
        let applied = front.session.driver.world.store.applied;
        let commits = front.session.driver.world.commits.len();
        let delay = Duration::from_secs(1);
        front.not_ready_restore(|| jig_core_world::world::config(8455), delay);
        front.try_fault(Time::ZERO, &FrontFault::NotReady).expect("actual paused root before provider exchange");
        Front::<Test>::sign_in(&mut front, ClientId(0), Time::ZERO, clock(Time::ZERO));
        let captured = &front.session.driver.world.client_peer.as_ref().expect("actual protocol peer").delivered;
        assert!(captured.iter().any(|(_, value)| matches!(
            value,
            jig_test_domain::Delivery::Core(jig_domain_core::Held::PeopleReply {
                reply: people::Reply::Again(people::Again::NotReady),
                ..
            })
        )));
        assert!(Front::<Test>::poll(&mut front, Time::ZERO, clock(Time::ZERO)).is_empty());
        assert_eq!(front.session.sign_in_again(ClientId(0)), Some(people::Again::NotReady));
        assert_eq!(front.session.driver.transport.sign_in(ClientId(0)), None);
        assert_eq!(front.session.driver.world.store.rows, rows);
        assert_eq!(front.session.driver.world.store.applied, applied);
        assert_eq!(front.session.driver.world.commits.len(), commits);
        assert!(Front::<Test>::observed(&mut front).is_empty());
        let later = Time::ZERO.saturating_add(delay);
        assert!(Front::<Test>::poll(&mut front, later, clock(later)).is_empty());
        assert!(!front.session.driver.world.store.loading());
        assert_eq!(front.session.driver.transport.sign_in(ClientId(0)), None);
        assert_eq!(front.session.sign_in_again(ClientId(0)), Some(people::Again::NotReady));
        assert_eq!(front.session.driver.world.store.rows, jig_core_world::store_peer::with_start_mark(&rows));
        assert_eq!(front.session.driver.world.store.applied, applied + 1);
        assert_eq!(front.session.driver.world.commits.len(), commits + 1);
        Front::<Test>::sign_in(&mut front, ClientId(0), later, clock(later));
        assert!(take_signed_in(ClientId(0), Front::<Test>::poll(&mut front, later, clock(later))).is_empty());
        assert!(front.session.driver.transport.sign_in(ClientId(0)).is_some());
        assert_eq!(front.session.sign_in_again(ClientId(0)), None);
        assert!(front.session.driver.world.store.applied > applied);
        assert_eq!(front.session.driver.world.commits.len(), commits + 2);
    }

    #[test]
    fn a_real_not_ready_front_retains_whole_restore_pages_and_retries_the_same_unsaved_key_after_release() {
        use skein_lib::RequestEnvelope;
        let mut front = front();
        Front::<Test>::bind(&mut front, ClientId(0), 1);
        let _ = Front::<Test>::poll(&mut front, Time::ZERO, clock(Time::ZERO));
        let _ = Front::<Test>::observed(&mut front);
        let session = front.session.driver.transport.sign_in(ClientId(0)).expect("actual authenticated source");
        let rows = front.session.driver.world.store.rows.clone();
        let applied = front.session.driver.world.store.applied;
        let commits = front.session.driver.world.commits.len();
        let delay = Duration::from_secs(1);
        front.not_ready_restore(|| jig_core_world::world::config(8455), delay);
        front.try_fault(Time::ZERO, &FrontFault::NotReady).expect("actual paused owner restore");
        assert!(front.session.driver.world.store.loading());
        assert_eq!(Front::<Test>::next_due(&front), Some(Time::ZERO.saturating_add(delay)));
        let request = |attempt| Request::Client {
            request: jig_client::Request::Down {
                down: jig_client::Down::Send {
                    attempt,
                    key: [63; 16],
                    request: jig_client::Ask::Jig(Box::new(client::Ask::Stop { project: 1, task: u64::MAX })),
                    value: None,
                },
            },
        };
        Front::<Test>::wire(&mut front, ClientId(0), Time::ZERO, clock(Time::ZERO), request(Token::new(41)));
        assert!(front.session.driver.world.client_peer.as_ref().expect("actual protocol peer").delivered.iter().any(
            |(_, value)| matches!(
                value,
                jig_test_domain::Delivery::Core(jig_domain_core::Held::PeopleReply {
                    reply: people::Reply::Again(people::Again::NotReady),
                    ..
                })
            )
        ));
        let callbacks = Front::<Test>::poll(&mut front, Time::ZERO, clock(Time::ZERO));
        assert!(
            matches!(callbacks.as_slice(), [(ClientId(0), Event::Client { event: jig_client::Event::Up { up: jig_client::Up::Answer {
            attempt, envelope: RequestEnvelope::Again, answer: None, why: None } } })] if *attempt == Token::new(41))
        );
        assert_eq!(front.session.driver.world.store.rows, rows);
        assert_eq!(front.session.driver.world.store.applied, applied);
        assert_eq!(front.session.driver.world.commits.len(), commits);
        assert!(front.session.driver.world.store.loading());
        assert!(Front::<Test>::observed(&mut front).is_empty());
        assert_eq!(front.session.driver.transport.sign_in(ClientId(0)), Some(session));
        let later = Time::ZERO.saturating_add(delay);
        assert!(Front::<Test>::poll(&mut front, later, clock(later)).is_empty(), "page release is not a final reply");
        assert!(!front.session.driver.world.store.loading());
        assert!(!front.session.driver.world.hold_restore_pages);
        assert_eq!(front.session.driver.world.store.rows, jig_core_world::store_peer::with_start_mark(&rows));
        assert_eq!(front.session.driver.world.store.applied, applied + 1);
        assert_eq!(Front::<Test>::observed(&mut front), vec![Observed::Durable { commit: applied + 1, at: later }]);
        Front::<Test>::wire(&mut front, ClientId(0), later, clock(later), request(Token::new(42)));
        let callbacks = Front::<Test>::poll(&mut front, later, clock(later));
        assert!(
            matches!(callbacks.as_slice(), [(ClientId(0), Event::Client { event: jig_client::Event::Up { up: jig_client::Up::Answer {
            attempt, envelope: RequestEnvelope::Final, answer: Some(_), why: None } } })] if *attempt == Token::new(42))
        );
        let observed = Front::<Test>::observed(&mut front);
        assert_eq!(
            observed
                .iter()
                .filter(|value| matches!(value,
            Observed::Made { party: 1, key, .. } if *key == [63; 16]))
                .count(),
            1
        );
        assert!(observed.iter().all(|value| !matches!(value, Observed::Kept { .. })));
    }

    #[test]
    fn the_real_front_version_fault_uses_the_supplied_whole_protocol_range_independently_of_business_frames() {
        let mut front = front();
        Front::<Test>::bind(&mut front, ClientId(0), 1);
        let _ = Front::<Test>::poll(&mut front, Time::ZERO, clock(Time::ZERO));
        let _ = Front::<Test>::observed(&mut front);
        let rows = front.session.driver.world.store.rows.clone();
        let supplied = client::VersionRange { first: u32::MAX, last: u32::MAX };
        front.version_fault(supplied);
        Front::<Test>::fault(&mut front, Time::ZERO, FrontFault::Versions);
        let callbacks = Front::<Test>::poll(&mut front, Time::ZERO, clock(Time::ZERO));
        assert!(matches!(
            callbacks.as_slice(),
            [(
                ClientId(0),
                Event::Client {
                    event: jig_client::Event::Up { up: jig_client::Up::Served { served: false, versions: None } }
                }
            )]
        ));
        assert_eq!(front.session.driver.transport.hello(), jig_client::Up::Served { served: false, versions: None });
        assert_eq!(front.session.driver.world.store.rows, rows);
        assert!(Front::<Test>::observed(&mut front).is_empty());
    }

    #[test]
    fn the_real_front_restart_uses_supplied_configuration_and_reports_reachability_at_the_actual_recovery_deadline() {
        let mut front = front();
        front.restart_configuration(|| jig_core_world::world::config(8455));
        Front::<Test>::bind(&mut front, ClientId(0), 1);
        let _ = Front::<Test>::poll(&mut front, Time::ZERO, clock(Time::ZERO));
        let _ = Front::<Test>::observed(&mut front);
        let span = skein_lib::Duration::from_secs(1);
        Front::<Test>::fault(&mut front, Time::ZERO, FrontFault::Restart { offline: span });
        let observed = Front::<Test>::observed(&mut front);
        assert!(matches!(observed.first(), Some(Observed::Reachable { live: false })));
        assert_eq!(Front::<Test>::next_due(&front), Some(Time::ZERO.saturating_add(span)));
        assert!(Front::<Test>::poll(&mut front, Time::ZERO, clock(Time::ZERO)).is_empty());
        assert!(Front::<Test>::observed(&mut front).is_empty());
        let until = Time::ZERO.saturating_add(span);
        let callbacks = Front::<Test>::poll(&mut front, until, clock(until));
        assert!(matches!(
            callbacks.as_slice(),
            [(ClientId(0), Event::Client { event: jig_client::Event::Up { up: jig_client::Up::Served { .. } } })]
        ));
        assert!(matches!(Front::<Test>::observed(&mut front).first(), Some(Observed::Reachable { live: true })));
        assert!(Front::<Test>::observed(&mut front).is_empty());
    }
}
