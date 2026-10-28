//! Calls, exchanges, and durable sign-in sequencing (protocol/client.md, 8).
use crate::{
    Event, Kept, Limits, Request, SecretOwner, SecretsAnswer, SecretsRequest, Settings,
    exchange::Exchange,
    routes::{self, Route},
    sign_in::{self, Link, Pending, Waiting},
};
use alloc::boxed::Box;
use jig_client_protocol_wire::v19 as w;
use jig_domain_core as core;
use jig_domain_people as people;
use skein_lib::{CryptoRng, Env, Map, Queue, ReplyTo, Time, Token};
#[path = "closing.rs"]
mod closing;
#[path = "mint.rs"]
mod mint;
#[path = "provider.rs"]
pub(crate) mod provider;
#[path = "reads.rs"]
mod reads;
#[path = "services.rs"]
mod services;
#[path = "watches.rs"]
mod watches;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unusable {
    Limits,
    Seed,
}
#[derive(Debug)]
pub struct Component {
    settings: Settings,
    provider: provider::State,
    rng: CryptoRng,
    link: Option<Link>,
    /// The link a sign-in spends while the core answers it, given back on not ready, busy or full (client.md, 8).
    spending: Option<(Token, Link)>,
    connections: Map<Token, Exchange>,
    pending: Pending,
    streams: Map<Token, crate::stream::Stream>,
    next: u64,
    phase: closing::Phase,
}
impl Component {
    pub fn new(settings: Settings, env: &Env<Limits>, out: &mut Queue<Request>) -> Result<Self, Unusable> {
        if !crate::valid(&env.limits) {
            return Err(Unusable::Limits);
        }
        if settings.served.capacity() > env.limits.families
            || settings.providers.capacity() > env.limits.providers
            || settings.addresses.len() > usize::try_from(env.limits.providers).expect("u32 fits")
            || settings.heartbeat == skein_lib::Duration::ZERO
            || settings.rows == 0
            || settings.redirect_bytes > env.limits.wire.keyed_value_some
            || settings.key_bytes > env.limits.wire.keyed_value_some
            || settings.moved_bytes > u64::from(env.limits.wire.keyed_value_some)
            || !crate::translate::llm::fits(&settings.models, &env.limits.wire)
        {
            return Err(Unusable::Limits);
        }
        let Ok(hello) = crate::translate::watches::hello([0; 16], &settings, &env.limits.wire) else {
            return Err(Unusable::Limits);
        };
        if hello.measure() > env.limits.queued_bytes {
            return Err(Unusable::Limits);
        }
        if let Some(identity) = &settings.link {
            let bytes = identity.key.subject.len().checked_add(identity.login.len()).ok_or(Unusable::Limits)?;
            let bytes = bytes.checked_add(identity.name.len()).ok_or(Unusable::Limits)?;
            if bytes > usize::try_from(env.limits.identity_bytes).expect("u32 fits") {
                return Err(Unusable::Limits);
            }
        }
        if settings.provider.is_some() {
            let issuer = crate::provider_limits(&env.limits);
            let location = issuer
                .uri_bytes
                .checked_add(issuer.state_bytes.checked_mul(3).ok_or(Unusable::Limits)?)
                .ok_or(Unusable::Limits)?;
            let location = location
                .checked_add(issuer.token_bytes.checked_mul(3).ok_or(Unusable::Limits)?)
                .ok_or(Unusable::Limits)?;
            // Percent-encoded code/state, and room for separators or an issuer error.
            let location = location.checked_add(64).ok_or(Unusable::Limits)?;
            if env.limits.wire.authorize_go_location < location {
                return Err(Unusable::Limits);
            }
        }
        let rng = CryptoRng::new(&settings.seed, settings.stream).ok_or(Unusable::Seed)?;
        let mut value = Self {
            provider: provider::State::new(&env.limits),
            settings,
            rng,
            link: None,
            spending: None,
            connections: Map::with_capacity(env.limits.exchanges),
            pending: Pending {
                mints: Map::with_capacity(env.limits.mints),
                exchanges: Map::with_capacity(
                    env.limits.exchanges.checked_add(env.limits.exchanges).expect("checked pending capacity"),
                ),
                carried: Map::with_capacity(
                    env.limits.exchanges.checked_add(env.limits.exchanges).expect("checked pending capacity"),
                ),
            },
            streams: Map::with_capacity(env.limits.streams),
            next: 1,
            phase: closing::Phase::Active,
        };
        value.fresh_link(env, out);
        Ok(value)
    }
    fn name(&mut self) -> Token {
        let token = Token::new(self.next);
        self.next = self.next.checked_add(1).expect("component names do not exhaust");
        token
    }
    fn fresh_link(&mut self, env: &Env<Limits>, out: &mut Queue<Request>) {
        if self.settings.link.is_none() || self.link.is_some() || self.spending.is_some() {
            return;
        }
        let secret = sign_in::draw(&mut self.rng);
        self.link = Some(Link { secret: secret.clone(), until: env.now.saturating_add(env.limits.link_span) });
        out.push(Request::Link { secret });
    }
    pub fn down(&mut self, env: &Env<Limits>, event: Event, out: &mut Queue<Request>) {
        assert!(!self.closed(), "the owner sends nothing after Closed");
        match event {
            Event::Transport(message) => self.transport(env, message, out),
            Event::Reply { to, reply } => self.reply(env, to.into_token(), reply, out),
            Event::SignInRefused { to } => {
                let exchange = to.into_token();
                if let Some((reserved, _)) = &self.spending
                    && *reserved == exchange
                {
                    self.spending = None;
                }
                let why = if self.stopping() { crate::MintRefusal::NotReady } else { crate::MintRefusal::Bound };
                if !self.mint_refused(exchange, why, out) {
                    self.refused(env, exchange);
                }
            }
            Event::Secrets(answer) => self.secrets(env, answer, out),
            Event::Application(crate::FromApplication::Undecodable { to }) => {
                let exchange = to.into_token();
                self.pending.exchanges.remove(&exchange);
                self.unused(exchange, out);
                if let Some(owner) = self.owner(exchange) {
                    self.connections
                        .get_mut(&owner)
                        .expect("exchange owner")
                        .respond(crate::Reply::NotARecord, &env.limits);
                }
            }
            Event::SignInEnded { sign_in } => self.ended(env, sign_in, out),
            Event::Application(crate::FromApplication::Answered { to, family, record }) => {
                self.application(env, to.into_token(), family, record, out);
            }
            Event::MovedTaken { exchange, record } => self.moved_taken(env, exchange, record, out),
            Event::Stop => self.stop(out),
            Event::Close => self.close(out),
            Event::Abort => self.abort(out),
            Event::Read { reply, answer } => self.read_answer(env, reply.into_token(), &answer, out),
            Event::Application(crate::FromApplication::Read { reply, family, record }) => {
                self.application_read(env, reply.into_token(), family, record, out);
            }
            Event::Admitted { watcher } => self.watch_answer(env, watcher, None),
            Event::WatchRefused { watcher, why } => self.watch_answer(env, watcher, Some(why)),
            Event::Delivery { watcher, document } => self.delivery(env, watcher, &document),
            Event::WatchEnded { watcher, why } => self.watch_ended(env, watcher, why),
            Event::RunText { watcher, turn, from, to, text, cut, call, writing } => {
                self.run_text(env, watcher, jig_domain_views::Writing { turn, from, to, text, cut, call, writing });
            }
            Event::Application(crate::FromApplication::Delivery { watcher, family, record }) => {
                self.application_delivery(env, watcher, family, record);
            }
            Event::Redeem { exchange, authorization, content_type, body } => {
                let answer = self.redeem(env, authorization.as_ref(), content_type.as_deref(), body.expose());
                out.push(Request::Redeemed { exchange, answer });
            }
            Event::Claims { exchange, authorization } => {
                let answer = self.claims(env, authorization.as_ref());
                out.push(Request::Claimed { exchange, answer });
            }
            Event::Mint { exchange, identity } => self.mint(env, exchange, identity, out),
            Event::Application(crate::FromApplication::Unused { number }) => {
                let mut found = None;
                for (exchange, kept) in &self.pending.carried {
                    if *kept == Some(number) {
                        found = Some(*exchange);
                        break;
                    }
                }
                if let Some(exchange) = found {
                    self.unused(exchange, out);
                }
            }
        }
    }
    /// Advances one queued HTTP transition, bounding each invocation's output.
    pub fn up(&mut self, env: &Env<Limits>, out: &mut Queue<Request>) -> bool {
        if self.aborting() {
            if let Some((name, _)) = self.pending.mints.first() {
                let name = *name;
                self.mint_refused(name, crate::MintRefusal::NotReady, out);
                return true;
            }
            if self.stream_up(env, out) {
                return true;
            }
            if let Some((owner, _)) = self.connections.first() {
                let owner = *owner;
                let mut connection = self.connections.remove(&owner).expect("closing call");
                connection.close(out);
                return true;
            }
            if self.settled() {
                self.phase = closing::Phase::Closed;
                out.push(Request::Closed);
                return true;
            }
            return false;
        }
        let mut ready = None;
        for (exchange, waiting) in &self.pending.exchanges {
            match waiting {
                Waiting::Checked { .. } => {
                    ready = Some(*exchange);
                    break;
                }
                Waiting::Core { .. }
                | Waiting::LinkGet { .. }
                | Waiting::SignOut { .. }
                | Waiting::Watch { .. }
                | Waiting::Get { .. }
                | Waiting::Keep { .. }
                | Waiting::Moving { .. }
                | Waiting::Made { .. }
                | Waiting::Read { .. }
                | Waiting::Authorize { .. }
                | Waiting::ApplicationRead { .. }
                | Waiting::Ask { .. }
                | Waiting::Carry { .. } => {}
            }
        }
        if let Some(exchange) = ready {
            match self.pending.exchanges.remove(&exchange).expect("ready exchange") {
                Waiting::Checked { sign_in, body, route } => match route {
                    Route::Read => self.read(env, exchange, sign_in, &body, out),
                    Route::Watch | Route::Unwatch | Route::Gone | Route::Stream => {
                        self.watch_route(env, exchange, sign_in, route, &body, out);
                    }
                    Route::Request => self.request(env, exchange, sign_in, &body, out),
                    Route::SignOut => self.sign_out(env, exchange, sign_in, &body, out),
                    Route::Authorize => self.authorize_checked(env, exchange, sign_in, &body, out),
                    Route::Link | Route::Service => {}
                },
                Waiting::Core { .. }
                | Waiting::LinkGet { .. }
                | Waiting::SignOut { .. }
                | Waiting::Watch { .. }
                | Waiting::Get { .. }
                | Waiting::Keep { .. }
                | Waiting::Moving { .. }
                | Waiting::Made { .. }
                | Waiting::Read { .. }
                | Waiting::Authorize { .. }
                | Waiting::ApplicationRead { .. }
                | Waiting::Ask { .. }
                | Waiting::Carry { .. } => {}
            }
            return true;
        }
        let mut selected = None;
        for (owner, connection) in &self.connections {
            if connection.closing || connection.response.is_some() || connection.ended.is_some() {
                selected = Some(*owner);
                break;
            }
        }
        let Some(owner) = selected else {
            if self.stream_up(env, out) {
                return true;
            }
            return self.close_up(env, out);
        };
        let mut connection = self.connections.remove(&owner).expect("selected exchange");
        if let Some(reply) = connection.response.take() {
            out.push(Request::Transport(crate::ToTransport::Reply { exchange: owner, reply }));
        } else if let Some(why) = connection.ended {
            out.push(Request::Transport(crate::ToTransport::StreamEnded { stream: owner, why }));
        }
        if let Some(exchange) = connection.name {
            self.pending.exchanges.remove(&exchange);
        }
        if let Some(stream) = self.streams.get_mut(&owner) {
            stream.phase = crate::stream::Phase::Closing;
            stream.closed_connection = true;
        }
        true
    }
    fn transport(&mut self, env: &Env<Limits>, message: crate::FromTransport, out: &mut Queue<Request>) {
        match message {
            crate::FromTransport::Exchange { exchange, credential, message } => {
                let (route, body) = routes::message(message);
                self.receive(env, exchange, credential, route, body, out);
            }
            crate::FromTransport::StreamOpen { stream, credential } => {
                self.receive(env, stream, credential, Route::Stream, Box::new([]), out);
            }
            crate::FromTransport::StreamTaken { stream } => {
                if let Some(stream) = self.streams.get_mut(&stream)
                    && stream.waiting
                {
                    let taken = stream.sent(env, self.settings.heartbeat);
                    if let Some(event) = crate::stream::taken(taken) {
                        out.push(Request::Core(Box::new(event)));
                    }
                }
            }
            crate::FromTransport::StreamGone { stream } => {
                self.connections.remove(&stream);
                if let Some(value) = self.streams.get_mut(&stream) {
                    value.phase = crate::stream::Phase::Closing;
                    value.closed_connection = true;
                }
            }
        }
    }
    fn receive(
        &mut self,
        env: &Env<Limits>,
        owner: Token,
        credential: Option<skein_lib::Secret>,
        route: Route,
        body: Box<[u8]>,
        out: &mut Queue<Request>,
    ) {
        let exchange = self.name();
        let mut connection = Exchange::new(exchange, route == Route::Stream);
        self.body(env, &mut connection, route, body, credential, out);
        self.connections.insert(owner, connection).expect("transport bounds active exchanges");
    }
    fn body(
        &mut self,
        env: &Env<Limits>,
        connection: &mut Exchange,
        route: Route,
        body: Box<[u8]>,
        credential: Option<skein_lib::Secret>,
        out: &mut Queue<Request>,
    ) {
        let exchange = connection.name.expect("call owns exchange");
        if body.len() > usize::try_from(env.limits.message_bytes).expect("message bound fits") {
            connection.respond(crate::Reply::NotARecord, &env.limits);
            return;
        }
        if self.stopping()
            && let Some(answer) = routes::stopped(route)
        {
            connection.respond(crate::Reply::Record(answer), &env.limits);
            return;
        }
        if route == Route::Authorize && !self.authorize_query(env, connection, &body) {
            return;
        }
        match route {
            Route::Link => {
                let Ok(record) = w::LinkSignIn::decode(&env.limits.wire, &mut skein_lib::Reader::new(&body)) else {
                    connection.respond(crate::Reply::NotARecord, &env.limits);
                    return;
                };
                if let Some((_, link)) = &self.spending {
                    let answer = if sign_in::constant_time_eq(link.secret.expose(), record.secret()) {
                        routes::sign_in_busy()
                    } else {
                        routes::refused(&env.limits.wire)
                    };
                    connection.respond(crate::Reply::Record(answer), &env.limits);
                    return;
                }
                let live = match &self.link {
                    Some(link) => {
                        env.now < link.until && sign_in::constant_time_eq(link.secret.expose(), record.secret())
                    }
                    None => false,
                };
                if !live {
                    if expired(self.link.as_ref(), env.now) {
                        self.link = None;
                    }
                    self.fresh_link(env, out);
                    connection.respond(crate::Reply::Record(routes::refused(&env.limits.wire)), &env.limits);
                    return;
                }
                if self.admitted() >= env.limits.exchanges {
                    connection.respond(crate::Reply::Record(routes::sign_in_busy()), &env.limits);
                    return;
                }
                if let Some(link) = self.link.take() {
                    self.spending = Some((exchange, link));
                }
                if let Some((sign_in, digest)) = sign_in::credential(credential.as_ref()) {
                    self.pending
                        .exchanges
                        .insert(exchange, Waiting::LinkGet { sign_in, digest })
                        .expect("admitted exchange");
                    out.push(Request::Secrets(SecretsRequest::Get { owner: SecretOwner::SignIn(sign_in) }));
                } else {
                    self.link_sign_in(exchange, None, out);
                }
            }
            Route::Authorize
            | Route::Service
            | Route::SignOut
            | Route::Request
            | Route::Read
            | Route::Watch
            | Route::Unwatch
            | Route::Gone
            | Route::Stream => {
                let Some((sign_in, digest)) = sign_in::credential(credential.as_ref()) else {
                    let answer = routes::unauthenticated(route, &env.limits.wire);
                    if route == Route::Stream {
                        connection.ended = Some(crate::Ended::SignedOut);
                    } else {
                        connection.respond(crate::Reply::Record(answer), &env.limits);
                    }
                    return;
                };
                let owner =
                    if route == Route::Service { SecretOwner::Service(sign_in) } else { SecretOwner::SignIn(sign_in) };
                self.pending
                    .exchanges
                    .insert(exchange, Waiting::Get { owner, digest, body, route })
                    .expect("admitted exchange");
                out.push(Request::Secrets(SecretsRequest::Get { owner }));
            }
        }
    }
    pub(super) fn link_sign_in(&mut self, exchange: Token, held: Option<u64>, out: &mut Queue<Request>) {
        self.pending.exchanges.insert(exchange, Waiting::Core { held }).expect("admitted link slot");
        out.push(Request::Core(Box::new(core::Event::SignIn {
            reply_to: ReplyTo::new(exchange),
            identity: self.settings.link.as_ref().expect("link enabled").clone(),
            held,
        })));
    }
    fn reply(&mut self, env: &Env<Limits>, exchange: Token, reply: people::Reply, out: &mut Queue<Request>) {
        // The core names a value it leaves unused before its answer; one
        // still here is the request's to use (store.md 9).
        self.pending.carried.remove(&exchange);
        let Some(waiting) = self.pending.exchanges.remove(&exchange) else {
            return;
        };
        match waiting {
            Waiting::SignOut { sign_in, committed } => match reply {
                people::Reply::SignedOut => {
                    self.pending
                        .exchanges
                        .insert(exchange, Waiting::SignOut { sign_in, committed })
                        .expect("same sign-out slot");
                    if !committed {
                        self.ended(env, sign_in, out);
                    }
                }
                people::Reply::Again(people::Again::NotReady) => {
                    self.respond(env, exchange, routes::sign_out_not_ready());
                }
                people::Reply::Signer(_)
                | people::Reply::Full { .. }
                | people::Reply::SignedIn { .. }
                | people::Reply::Outcome(_)
                | people::Reply::Again(_)
                | people::Reply::Refused(_) => {
                    if let Some(owner) = self.owner(exchange) {
                        self.connections.get_mut(&owner).expect("exchange owner").close(out);
                    }
                }
            },
            Waiting::Authorize { authorization, .. } => self.authorize_reply(env, exchange, reply, authorization),
            Waiting::Core { held } => self.sign_in_reply(env, exchange, reply, held, out),
            Waiting::Ask { .. } => self.ask_reply(env, exchange, reply, out, waiting),
            Waiting::LinkGet { .. }
            | Waiting::Watch { .. }
            | Waiting::Read { .. }
            | Waiting::ApplicationRead { .. }
            | Waiting::Keep { .. }
            | Waiting::Get { .. }
            | Waiting::Moving { .. }
            | Waiting::Made { .. }
            | Waiting::Carry { .. }
            | Waiting::Checked { .. } => {
                self.pending.exchanges.insert(exchange, waiting).expect("same slot");
            }
        }
    }
    /// Erases the value `exchange`'s decode kept, which its request will
    /// not use (store.md 9).
    fn unused(&mut self, exchange: Token, out: &mut Queue<Request>) {
        if let Some(Some(number)) = self.pending.carried.remove(&exchange)
            && !self.aborting()
        {
            out.push(Request::Secrets(SecretsRequest::Erase { owner: SecretOwner::Carried(number) }));
        }
    }
    fn ask_reply(
        &mut self,
        env: &Env<Limits>,
        exchange: Token,
        reply: people::Reply,
        out: &mut Queue<Request>,
        waiting: Waiting,
    ) {
        let Waiting::Ask { sign_in, family, version } = waiting else { unreachable!("ask reply route") };
        let reply = if family.is_none()
            && version < jig_client_protocol_wire::VERSION
            && let people::Reply::Again(people::Again::Full { .. })
            | people::Reply::Outcome(people::Outcome::Again(people::Again::Full { .. })) = reply
        {
            people::Reply::Again(people::Again::Busy)
        } else {
            reply
        };
        match reply {
            people::Reply::Outcome(people::Outcome::AccountMovingOut { account, number, source }) => {
                self.pending
                    .exchanges
                    .insert(exchange, Waiting::Moving { sign_in, account, number })
                    .expect("same slot");
                out.push(Request::TakeMoved { exchange, source });
            }
            people::Reply::Outcome(people::Outcome::ServiceMade { person }) => {
                self.pending.exchanges.insert(exchange, Waiting::Made { sign_in, party: person }).expect("same slot");
                out.push(Request::Secrets(SecretsRequest::Get { owner: SecretOwner::Service(person) }));
            }
            people::Reply::Outcome(people::Outcome::Applied { .. }) => {
                self.pending.exchanges.insert(exchange, waiting).expect("same slot");
            }
            people::Reply::Signer(_)
            | people::Reply::Full { .. }
            | people::Reply::SignedIn { .. }
            | people::Reply::SignedOut
            | people::Reply::Outcome(_)
            | people::Reply::Again(_)
            | people::Reply::Refused(_) => {
                let answer = crate::translate::answers::answer(&reply, &self.settings.models, &env.limits.wire);
                self.answer(env, exchange, answer, out);
            }
        }
    }
    fn moved_taken(
        &mut self,
        env: &Env<Limits>,
        exchange: Token,
        record: Option<skein_lib::Secret>,
        out: &mut Queue<Request>,
    ) {
        let Some(waiting) = self.pending.exchanges.remove(&exchange) else { return };
        match waiting {
            Waiting::Moving { account, number, .. } => {
                let record = match record {
                    Some(record) => record,
                    None => skein_lib::Secret::new(Box::new([])),
                };
                let answer = crate::translate::answers::moving_out(account, number, record, &env.limits.wire);
                self.answer(env, exchange, answer, out);
            }
            Waiting::Core { .. }
            | Waiting::LinkGet { .. }
            | Waiting::SignOut { .. }
            | Waiting::Watch { .. }
            | Waiting::Keep { .. }
            | Waiting::Get { .. }
            | Waiting::Made { .. }
            | Waiting::Read { .. }
            | Waiting::Authorize { .. }
            | Waiting::ApplicationRead { .. }
            | Waiting::Ask { .. }
            | Waiting::Carry { .. }
            | Waiting::Checked { .. } => {
                self.pending.exchanges.insert(exchange, waiting).expect("same slot");
            }
        }
    }
    fn answer(
        &mut self,
        env: &Env<Limits>,
        exchange: Token,
        answer: Result<w::KeyedAnswer, w::Problem>,
        out: &mut Queue<Request>,
    ) {
        let Some(owner) = self.owner(exchange) else {
            return;
        };
        let connection = self.connections.get_mut(&owner).expect("exchange owner");
        match answer {
            Ok(answer) => {
                let mut body = skein_lib::Writer::new(usize::try_from(answer.measure()).expect("wire size fits"));
                answer.encode(&mut body).expect("measured answer");
                connection.respond(crate::Reply::Record(body.finish()), &env.limits);
            }
            Err(_) => connection.close(out),
        }
    }
    fn application(
        &mut self,
        env: &Env<Limits>,
        exchange: Token,
        family: u16,
        record: Box<[u8]>,
        out: &mut Queue<Request>,
    ) {
        // The application's answer: its request used its carried value.
        self.pending.carried.remove(&exchange);
        let Some(waiting) = self.pending.exchanges.remove(&exchange) else {
            return;
        };
        match waiting {
            Waiting::Ask { family: Some(number), version, .. } if number == family => {
                let answer = match w::Family::new(&env.limits.wire, w::FamilyParts { number }) {
                    Ok(value) => {
                        crate::translate::answers::owned(w::Owner::Family(value), version, record, &env.limits.wire)
                    }
                    Err(problem) => Err(problem),
                };
                self.answer(env, exchange, answer, out);
            }
            Waiting::Core { .. }
            | Waiting::LinkGet { .. }
            | Waiting::SignOut { .. }
            | Waiting::Watch { .. }
            | Waiting::Keep { .. }
            | Waiting::Get { .. }
            | Waiting::Moving { .. }
            | Waiting::Made { .. }
            | Waiting::Checked { .. }
            | Waiting::Read { .. }
            | Waiting::Authorize { .. }
            | Waiting::ApplicationRead { .. }
            | Waiting::Ask { .. }
            | Waiting::Carry { .. } => {
                self.pending.exchanges.insert(exchange, waiting).expect("same slot");
            }
        }
    }
    fn refused(&mut self, env: &Env<Limits>, exchange: Token) {
        if self.pending.exchanges.remove(&exchange).is_some() {
            self.respond(env, exchange, routes::refused(&env.limits.wire));
        }
    }
    fn owner(&self, exchange: Token) -> Option<Token> {
        for (owner, connection) in &self.connections {
            if connection.name == Some(exchange) && !connection.closing {
                return Some(*owner);
            }
        }
        None
    }
    fn respond(&mut self, env: &Env<Limits>, exchange: Token, body: Box<[u8]>) {
        if let Some(owner) = self.owner(exchange) {
            self.connections.get_mut(&owner).expect("exchange owner").respond(crate::Reply::Record(body), &env.limits);
        }
    }
    fn denied(&mut self, env: &Env<Limits>, exchange: Token, route: Route) {
        if let Some(owner) = self.owner(exchange) {
            let connection = self.connections.get_mut(&owner).expect("exchange owner");
            if route == Route::Stream {
                connection.ended = Some(crate::Ended::SignedOut);
            } else {
                connection.respond(crate::Reply::Record(routes::unauthenticated(route, &env.limits.wire)), &env.limits);
            }
        }
    }
    fn admitted(&self) -> u32 {
        let mut count = 0_u32;
        for (_, waiting) in &self.pending.exchanges {
            match waiting {
                Waiting::Core { .. }
                | Waiting::LinkGet { .. }
                | Waiting::SignOut { .. }
                | Waiting::Watch { .. }
                | Waiting::Keep { .. }
                | Waiting::Moving { .. }
                | Waiting::Made { .. }
                | Waiting::Read { .. }
                | Waiting::Authorize { .. }
                | Waiting::ApplicationRead { .. }
                | Waiting::Ask { .. }
                | Waiting::Carry { .. } => {
                    count = count.saturating_add(1);
                }
                Waiting::Get { .. } | Waiting::Checked { .. } => {}
            }
        }
        count
    }
    #[must_use]
    #[expect(clippy::manual_map, reason = "step code matches optional deadlines without closures")]
    pub fn next_deadline(&self) -> Option<Time> {
        let mut next = match &self.link {
            Some(link) => Some(link.until),
            None => None,
        };
        for (_, stream) in &self.streams {
            if !stream.closing() && !stream.waiting {
                next = Some(match next {
                    Some(next) => next.min(stream.deadline),
                    None => stream.deadline,
                });
            }
        }
        next
    }
    pub fn fire(&mut self, env: &Env<Limits>, _out: &mut Queue<Request>) {
        if expired(self.link.as_ref(), env.now) {
            self.link = None;
            return;
        }
        let mut due = None;
        for (owner, stream) in &self.streams {
            if !stream.closing() && !stream.waiting && stream.deadline <= env.now {
                due = Some(*owner);
                break;
            }
        }
        if let Some(owner) = due {
            let stream = self.streams.get_mut(&owner).expect("due stream");
            if stream.waiting {
                stream.phase = crate::stream::Phase::Closing;
            } else {
                stream.heartbeat(env);
            }
        }
    }

    pub fn reclaim(&mut self) {}
}
fn expired(link: Option<&Link>, now: Time) -> bool {
    match link {
        Some(link) => link.until <= now,
        None => false,
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

#[path = "request.rs"]
mod request;
