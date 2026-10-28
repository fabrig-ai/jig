//! The joint world's real core under the engine clients component.
use jig_client_world::{
    harness::Observed,
    joint::{engine::Driver, truth::Truth},
};
use jig_domain_core as core;
use jig_protocol_engine_clients::{self as clients, Event, Request, SecretOwner, SecretsAnswer, SecretsRequest};
use jig_protocol_engine_clients_http as http;
use jig_test_domain as root;
use skein_lib::{Env, Queue, Time, Token, Wall};
use std::collections::{BTreeMap, BTreeSet};
pub(super) struct Engine {
    pub driver: Driver,
    pub account_sign_in_burst: Option<Vec<root::Event>>,
    pub component: clients::Component,
    pub server: jig_test_service::server::Server,
    server_out: Queue<jig_test_service::server::Routed>,
    mount: http::Mount,
    refused: skein_lib::Map<Token, ()>,
    mount_out: Queue<http::Request>,
    pub env: Env<clients::Limits>,
    pub out: Queue<Request>,
    pub link: Box<[u8]>,
    pub rules: crate::referee::Rules,
    pub limits: root::Limits,
    pub seed: u64,
    secrets: BTreeMap<SecretOwner, clients::Kept>,
    /// The values requests carried, by the number this peer drew, from
    /// 2^63 up as the store draws them (store.md 9).
    pub carried: BTreeMap<u64, skein_lib::Secret>,
    next_carried: u64,
    next_held: u64,
    pub account_keys: Vec<core::Carried>,
    pub not_records: u32,
    pub erasures: Vec<u64>,
    replies: BTreeSet<Token>,
    truth: Truth,
    observed: Vec<Observed>,
    ticked: Option<Time>,
    pub offered: BTreeMap<Token, u64>,
    pub taken: BTreeMap<Token, u64>,
}
impl Engine {
    pub fn bounded(seed: u64, limits: &root::Limits) -> Self {
        Self::configured(seed, limits, jig_core_world::world::config(seed))
    }
    pub fn configured(seed: u64, limits: &root::Limits, config: root::Config) -> Self {
        Self::with_peers(seed, limits, config, crate::fixtures::peers())
    }
    pub fn with_peers(
        seed: u64,
        limits: &root::Limits,
        config: root::Config,
        peers: jig_core_world::peers::Peers,
    ) -> Self {
        let limits = crate::fixtures::journal(*limits);
        let driver = Driver::new(config, limits, peers, Time::ZERO, crate::fixtures::wall(Time::ZERO));
        let truth = Truth::after(driver.world.store.rows.clone());
        let env =
            Env { now: Time::ZERO, wall: crate::fixtures::wall(Time::ZERO), limits: crate::fixtures::engine_limits() };
        let mut out = Queue::with_capacity(256);
        let component = clients::Component::new(crate::fixtures::settings(limits.core.documents.rows), &env, &mut out)
            .expect("bounded clients component");
        Self {
            driver,
            account_sign_in_burst: None,
            component,
            server: crate::fixtures::server(),
            server_out: Queue::with_capacity(256),
            mount: crate::fixtures::mount(),
            refused: skein_lib::Map::with_capacity(crate::fixtures::server_limits().connections),
            mount_out: Queue::with_capacity(256),
            env,
            out,
            link: Box::new([]),
            rules: crate::referee::Rules::default(),
            limits,
            seed,
            secrets: BTreeMap::new(),
            carried: BTreeMap::new(),
            next_carried: 1 << 63,
            next_held: (1 << 63) + (1 << 62),
            account_keys: Vec::new(),
            not_records: 0,
            erasures: Vec::new(),
            replies: BTreeSet::new(),
            truth,
            observed: Vec::new(),
            ticked: None,
            offered: BTreeMap::new(),
            taken: BTreeMap::new(),
        }
    }
    fn mount_env(&self) -> Env<http::Limits> {
        Env { now: self.env.now, wall: self.env.wall, limits: crate::fixtures::http_limits() }
    }
    fn mount_event(&mut self, event: http::Event) {
        self.mount.down(&self.mount_env(), event, &mut self.mount_out);
    }
    fn mount_step(&mut self) -> bool {
        let mut active = self.mount.up(&self.mount_env(), &mut self.mount_out);
        while let Some(request) = self.mount_out.pop() {
            active = true;
            match request {
                http::Request::Http { call, request } => self.server.answer(call, request, &mut self.server_out),
                http::Request::Component(message) => self.event(Event::Transport(message)),
            }
        }
        active
    }
    pub fn clock(&mut self, now: Time, wall: Wall) {
        self.server.clock(now, wall);
        self.env.now = now;
        self.env.wall = wall;
        self.driver.world.set_clock(now, wall);
    }
    pub fn event(&mut self, event: Event) {
        self.component.down(&self.env, event, &mut self.out);
    }
    pub fn request(&mut self, request: Request) -> Option<skein_io::Request> {
        match request {
            Request::TakeMoved { exchange, source } => {
                let record = self.driver.world.take_moved(source);
                self.event(Event::MovedTaken { exchange, record });
            }
            Request::Transport(message) => {
                if matches!(&message, clients::ToTransport::Reply { reply: clients::Reply::NotARecord, .. }) {
                    self.not_records += 1;
                }
                self.mount_event(http::Event::Component(message));
            }
            Request::Link { secret } => self.link = crate::fixtures::printed(&secret),
            Request::Secrets(request) => {
                let answer = match request {
                    SecretsRequest::Keep { owner, value } => {
                        self.rules.secret(value.digest.expose());
                        self.secrets.insert(owner, value);
                        SecretsAnswer::Kept { owner }
                    }
                    SecretsRequest::Erase { owner } => {
                        self.secrets.remove(&owner);
                        if let SecretOwner::Carried(number) = owner {
                            self.carried.remove(&number);
                            self.erasures.push(number);
                        }
                        SecretsAnswer::Erased { owner }
                    }
                    SecretsRequest::Hold { exchange, value } => {
                        self.rules.secret(value.expose());
                        let number = self.next_held;
                        self.next_held = number.checked_add(1).expect("held numbers left");
                        self.carried.insert(number, value);
                        SecretsAnswer::Carried { exchange, number: Some(number) }
                    }
                    SecretsRequest::Carry { exchange, value, .. } => {
                        // The referee's domain rule: no carried value in a root input.
                        self.rules.secret(value.expose());
                        let number = self.next_carried;
                        self.next_carried = number.checked_add(1).expect("carried numbers left");
                        self.carried.insert(number, value);
                        SecretsAnswer::Carried { exchange, number: Some(number) }
                    }
                    SecretsRequest::Get { owner } => {
                        SecretsAnswer::Got { owner, value: self.secrets.get(&owner).cloned() }
                    }
                };
                self.event(Event::Secrets(answer));
            }
            Request::Core(event) => {
                if let core::Event::Key { carried, .. } = &*event {
                    assert_eq!(
                        skein_lib::sha256::digest(
                            self.carried.get(&carried.number).expect("account key kept before core input").expose()
                        ),
                        carried.digest
                    );
                    self.account_keys.push(*carried);
                }
                self.apply(root::Event::Core(*event));
            }
            Request::Application(request) => {
                match jig_test_service::family::to_root(
                    request,
                    &crate::fixtures::owner_limits().connector,
                    crate::fixtures::owner_limits().core.documents.rows,
                ) {
                    Ok(event) => self.apply(event),
                    Err(event) => self.event(*event),
                }
            }
            Request::Minted { .. }
            | Request::MintRefused { .. }
            | Request::Redeemed { .. }
            | Request::Claimed { .. } => {
                panic!("the world asks for no application sign-ins")
            }
            Request::Closed => {}
        }
        None
    }
    pub fn io_event(&mut self, event: skein_io::Event) {
        self.server.up(event, &mut self.server_out);
    }
    pub fn io(&mut self) -> Option<skein_io::Request> {
        while let Some(routed) = self.server_out.pop() {
            match routed {
                jig_test_service::server::Routed::Io(request) => return Some(request),
                jig_test_service::server::Routed::Call { call, head } => {
                    if self.mount.room() > 0 {
                        self.mount_event(http::Event::Call { call, head });
                    } else {
                        self.refused.insert(call, ()).expect("bounded refused server calls");
                        self.server.answer(call, skein_http::server::Request::Discard, &mut self.server_out);
                        self.server.answer(
                            call,
                            skein_http::server::Request::Respond(skein_http::server::Response {
                                status: 503,
                                headers: Box::new([]),
                                body: skein_http::server::Body::None,
                                close: true,
                            }),
                            &mut self.server_out,
                        );
                    }
                }
                jig_test_service::server::Routed::Http { call, event } => {
                    if self.refused.get(&call).is_some() {
                        match event {
                            skein_http::server::Event::Done(_)
                            | skein_http::server::Event::Failed(_)
                            | skein_http::server::Event::Closed => {
                                self.refused.remove(&call);
                                self.server.done(call, &mut self.server_out);
                            }
                            skein_http::server::Event::Call(_)
                            | skein_http::server::Event::Ended
                            | skein_http::server::Event::Body(_)
                            | skein_http::server::Event::Reply(_)
                            | skein_http::server::Event::Refused(_) => {}
                        }
                    } else {
                        self.mount_event(http::Event::Http { call, event });
                    }
                }
            }
        }
        None
    }
    fn apply(&mut self, mut event: root::Event) {
        if let root::Event::Core(core::Event::Delivered { watcher }) = &event {
            *self.taken.entry(*watcher).or_default() += 1;
        }
        self.rules.domain(&event).expect("no authentication secret in a root input");
        let to = if let root::Event::Core(
            core::Event::SignIn { reply_to, .. }
            | core::Event::ProposeSettings { reply_to, .. }
            | core::Event::ChangeSettings { reply_to, .. }
            | core::Event::Key { reply_to, .. }
            | core::Event::People(
                jig_domain_people::Event::Ask { reply_to, .. } | jig_domain_people::Event::SignOut { reply_to, .. },
            ),
        )
        | root::Event::Application { reply_to, .. } = &mut event
        {
            Some(reply_to)
        } else if let root::Event::ReadApplication { reply, .. } | root::Event::Core(core::Event::Read { reply, .. }) =
            &mut event
        {
            Some(reply)
        } else {
            None
        };
        if let Some(to) = to {
            let original = std::mem::replace(to, skein_lib::ReplyTo::new(Token::new(0))).into_token();
            *to = skein_lib::ReplyTo::new(original);
            self.replies.insert(original);
        }
        let watcher = if let root::Event::WatchApplication { watcher, .. } = &event {
            Some(*watcher)
        } else if let root::Event::Core(event) = &event {
            watcher(event)
        } else {
            None
        };
        let peer = self.driver.world.client_peer.as_mut().expect("joint protocol peer");
        peer.clear_replies();
        for token in &self.replies {
            peer.register_reply(*token);
        }
        if matches!(
            &event,
            root::Event::Core(core::Event::People(jig_domain_people::Event::Ask {
                ask: jig_domain_people::Ask::AccountSignIn { .. },
                ..
            }))
        ) && let Some(burst) = &mut self.account_sign_in_burst
        {
            burst.push(event);
            return;
        }
        self.driver.world.send(event);
        let refused = self.deliveries(watcher);
        if let Some(watcher) = watcher
            && !refused
        {
            self.event(Event::Admitted { watcher });
        }
    }
    fn deliveries(&mut self, watcher: Option<Token>) -> bool {
        let peer = self.driver.world.client_peer.as_mut().expect("joint protocol peer");
        let held = std::mem::take(&mut peer.delivered);
        let immediate = std::mem::take(&mut peer.now);
        let mut refused = false;
        for (_, delivery) in held {
            match delivery {
                root::Delivery::Application { to, number, outcome } => {
                    self.event(Event::Application(clients::FromApplication::Answered {
                        to,
                        family: number,
                        record: jig_client_test_protocol_wire::codec::outcome(&outcome),
                    }));
                }
                root::Delivery::ApplicationDocument { watcher, number, document } => {
                    self.event(Event::Application(clients::FromApplication::Delivery {
                        watcher,
                        family: number,
                        record: jig_client_test_protocol_wire::codec::document(&document),
                    }));
                }
                root::Delivery::ApplicationRead { reply, number, answer } => {
                    let record = Some(jig_client_test_protocol_wire::codec::read_answer(&answer));
                    self.event(Event::Application(clients::FromApplication::Read { reply, family: number, record }));
                }
                root::Delivery::Core(held) => {
                    if let core::Held::Delivery { watcher, .. } = &held {
                        *self.offered.entry(*watcher).or_default() += 1;
                    }
                    if let Some(event) = held_event(held) {
                        self.event(event);
                    }
                }
                root::Delivery::Link { .. }
                | root::Delivery::ProjectMade { .. }
                | root::Delivery::CallBusy { .. }
                | root::Delivery::Projection { .. }
                | root::Delivery::Message { .. }
                | root::Delivery::Restart(_)
                | root::Delivery::CallAnswer { .. }
                | root::Delivery::Assigned { .. }
                | root::Delivery::Fleet(_)
                | root::Delivery::System { .. }
                | root::Delivery::LostRead { .. }
                | root::Delivery::Procedure { .. } => {}
            }
        }
        for now in immediate {
            if let core::Now::WatchRefused { watcher: owner, .. } = &now {
                refused |= Some(*owner) == watcher;
            }
            if let Some(event) = now_event(now) {
                self.event(event);
            }
        }
        refused
    }
    pub fn poll(&mut self) -> bool {
        if self.ticked != Some(self.env.now) {
            self.ticked = Some(self.env.now);
            self.driver.world.fire_due(self.env.now, self.env.wall);
        }
        let peer = self.driver.world.client_peer.as_ref().expect("joint protocol peer");
        let mut active = !peer.delivered.is_empty() || !peer.now.is_empty();
        self.deliveries(None);
        for snapshot in self.driver.snapshots() {
            self.observed.extend(self.truth.applied(snapshot));
        }
        self.server.fire(&mut self.server_out);
        active |= self.mount_step();
        active |= self.server.advance(&mut self.server_out) || !self.server_out.is_empty();
        self.component.fire(&self.env, &mut self.out);
        while self.component.up(&self.env, &mut self.out) {}
        active
    }
    pub fn observed(&mut self) -> Vec<Observed> {
        std::mem::take(&mut self.observed)
    }
    pub fn chats_made(&self) -> usize {
        let label = jig_core_world::fixtures::templates()
            .iter()
            .find(|template| template.number == jig_client::shapes::CHAT)
            .expect("chat template")
            .label;
        self.driver.world.store.rows.values().filter(|record| matches!(record, root::Record::Core(core::Record::Tasks(jig_domain_tasks::Stored::Live(task) | jig_domain_tasks::Stored::Ended(task))) if task.label == label)).count()
    }
}
#[must_use]
pub fn held_event(held: core::Held) -> Option<Event> {
    Some(match held {
        core::Held::PeopleReply { to, reply } => Event::Reply { to, reply },
        core::Held::Read { reply, answer } => Event::Read { reply, answer },
        core::Held::Delivery { watcher, document } => Event::Delivery { watcher, document },
        core::Held::WatchEnded { watcher, why } => Event::WatchEnded { watcher, why },
        core::Held::CarriedUnused { number } => Event::Application(clients::FromApplication::Unused { number }),
        core::Held::Acted { .. }
        | core::Held::Drained
        | core::Held::ApplicationDelivery { .. }
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
        | core::Held::Cancel { .. }
        | core::Held::Park { .. }
        | core::Held::Sources { .. }
        | core::Held::CheckSource { .. }
        | core::Held::SignOutSource { .. }
        | core::Held::SourceSignedOut { .. }
        | core::Held::Refuse { .. }
        | core::Held::TurnBusy { .. }
        | core::Held::StopRun { .. }
        | core::Held::ParkRun { .. }
        | core::Held::Load(..)
        | core::Held::NotesLoad { .. }
        | core::Held::NotesWritten { .. }
        | core::Held::NotesDeleted { .. } => return None,
    })
}
#[must_use]
pub fn now_event(now: core::Now) -> Option<Event> {
    Some(match now {
        core::Now::SignInRefused { to } => Event::SignInRefused { to },
        core::Now::WatchRefused { watcher, why } => Event::WatchRefused { watcher, why },
        core::Now::RunText { watcher, turn, from, to, text, cut, call, writing } => {
            Event::RunText { watcher, turn, from, to, text, cut, call, writing }
        }
        core::Now::RelayBusy { .. }
        | core::Now::CallBusy { .. }
        | core::Now::Relayed { .. }
        | core::Now::EffectAnswer { .. }
        | core::Now::SettingsAnswer { .. }
        | core::Now::SettledCallRefused { .. }
        | core::Now::Call { .. }
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
        | core::Now::RestoreRefused => return None,
    })
}

#[must_use]
pub fn watcher(event: &core::Event) -> Option<Token> {
    match event {
        core::Event::Watch { watcher, .. } | core::Event::WatchRun { watcher, .. } => Some(*watcher),
        core::Event::SettingsCommit { .. }
        | core::Event::Fill { .. }
        | core::Event::Stopping
        | core::Event::Marked { .. }
        | core::Event::ReadLoaded { .. }
        | core::Event::Read { .. }
        | core::Event::Act { .. }
        | core::Event::ProposeSettings { .. }
        | core::Event::ChangeSettings { .. }
        | core::Event::Key { .. }
        | core::Event::Application { .. }
        | core::Event::ApplicationPart { .. }
        | core::Event::ApplicationFinished { .. }
        | core::Event::ProjectionSettled { .. }
        | core::Event::SettledCall { .. }
        | core::Event::EffectStart { .. }
        | core::Event::EffectConnector(_)
        | core::Event::EffectDeadline
        | core::Event::SignIn { .. }
        | core::Event::Unwatch { .. }
        | core::Event::Delivered { .. }
        | core::Event::StreamDelivered { .. }
        | core::Event::RunCall { .. }
        | core::Event::RunInput { .. }
        | core::Event::RunText { .. }
        | core::Event::Tasks(_)
        | core::Event::StartRecurring { .. }
        | core::Event::StartBrief { .. }
        | core::Event::BriefNotes { .. }
        | core::Event::BriefAssembled { .. }
        | core::Event::WorkspacePrepared { .. }
        | core::Event::ClaimPrepared { .. }
        | core::Event::ClaimRefused { .. }
        | core::Event::ConnectorRepair { .. }
        | core::Event::Period { .. }
        | core::Event::PreparationFailed { .. }
        | core::Event::TranscriptUnread { .. }
        | core::Event::PersonProposal(_)
        | core::Event::PersonEscalation(_)
        | core::Event::TaskEscalation(_)
        | core::Event::People(_)
        | core::Event::Fleet(_)
        | core::Event::TurnPayload { .. }
        | core::Event::AnswerPayload { .. }
        | core::Event::AcceptedTurn { .. }
        | core::Event::RefusedPayload { .. }
        | core::Event::Activate { .. }
        | core::Event::PreparedAgent { .. }
        | core::Event::Forget { .. }
        | core::Event::EndedReadLoaded { .. }
        | core::Event::SeenConversationLoaded { .. }
        | core::Event::HistoricalProposal { .. }
        | core::Event::DecidedProposal { .. }
        | core::Event::HistoricalPersonTask { .. }
        | core::Event::EndedTaskLoaded { .. }
        | core::Event::PartyInputsLoaded { .. }
        | core::Event::HistoricalEscalation { .. }
        | core::Event::NamedAnswer { .. }
        | core::Event::SubscribeTopic { .. }
        | core::Event::NamedAction { .. }
        | core::Event::DelegateValidated { .. }
        | core::Event::DelegateInputFailed { .. }
        | core::Event::DelegateHoldings { .. }
        | core::Event::ProcedureHoldings { .. }
        | core::Event::ProcedureStep { .. }
        | core::Event::Brief(_)
        | core::Event::Holdings { .. }
        | core::Event::Account(_)
        | core::Event::View(_)
        | core::Event::Notes(_) => None,
    }
}
