//! Keyed request admission and version envelopes (protocol/client.md, 5 and 9).
use super::{Box, Component, Env, Limits, Queue, ReplyTo, Request, Token, Waiting, core, people, routes, sign_in, w};
use crate::sign_in::CarriedRequest;
impl Component {
    pub(super) fn request(
        &mut self,
        env: &Env<Limits>,
        exchange: Token,
        sign_in: u64,
        body: &[u8],
        out: &mut Queue<Request>,
    ) {
        if self.stopping() {
            self.respond(env, exchange, routes::not_ready());
            return;
        }
        let Some(owner) = self.owner(exchange) else {
            return;
        };
        let active = self.active(sign_in);
        if active >= env.limits.exchanges_per_client || self.admitted() >= env.limits.exchanges {
            self.respond(env, exchange, routes::busy());
            return;
        }
        let keyed = match w::Keyed::decode(&env.limits.wire, &mut skein_lib::Reader::new(body)) {
            Ok(value) => value.into_parts(),
            Err(_) => {
                self.not_a_record(env, owner);
                return;
            }
        };
        let family = match &keyed.owner {
            w::Owner::Jig => None,
            w::Owner::Family(value) => Some(value.number()),
        };
        if !self.serves(family, keyed.version) {
            let answer =
                w::KeyedAnswer::NotServed(crate::served::not_served(&self.settings, keyed.owner, &env.limits.wire));
            self.respond(env, exchange, routes::encoded(&answer));
            return;
        }
        let request = match keyed.owner {
            w::Owner::Jig => {
                self.jig_request(env, exchange, sign_in, owner, keyed, out);
                return;
            }
            w::Owner::Family(family) => {
                let Ok(record) =
                    w::FamilyRequest::decode(&env.limits.wire, &mut skein_lib::Reader::new(&keyed.request))
                else {
                    self.not_a_record(env, owner);
                    return;
                };
                let scope = crate::served::scope(record.scope());
                let (digest, carried) = match &keyed.value {
                    None => (sign_in::digest(record.record()).expose().try_into().expect("32-byte digest"), None),
                    Some(value) => (
                        sign_in::carrying(record.record(), value),
                        Some(core::Carried { number: 0, digest: skein_lib::sha256::digest(value.expose()) }),
                    ),
                };
                let request = crate::ToApplication::Request {
                    reply_to: ReplyTo::new(exchange),
                    sign_in,
                    key: keyed.key,
                    family: family.number(),
                    version: keyed.version,
                    scope,
                    record: record.into_parts().record,
                    digest,
                    carried,
                };
                if let Some(value) = keyed.value {
                    self.carry(env, exchange, sign_in, CarriedRequest::Application(request), value, out);
                    return;
                }
                Request::Application(request)
            }
        };
        self.pending
            .exchanges
            .insert(exchange, Waiting::Ask { sign_in, family, version: keyed.version })
            .expect("get released slot");
        out.push(request);
    }
    /// Admit jig's key or moved file before asking the store to keep its value.
    #[expect(
        clippy::too_many_lines,
        reason = "one keyed entrance handles the ordinary ask, a carried value, and a whole settings change"
    )]
    fn jig_request(
        &mut self,
        env: &Env<Limits>,
        exchange: Token,
        sign_in: u64,
        owner: Token,
        keyed: w::KeyedParts,
        out: &mut Queue<Request>,
    ) {
        let ask = match w::Ask::decode(&env.limits.wire, &mut skein_lib::Reader::new(&keyed.request)) {
            Ok(value) => {
                let value = match value {
                    w::Ask::Propose(value)
                        if match value.action() {
                            w::ProposeAction::Settings(_) => true,
                            w::ProposeAction::Batch(_) | w::ProposeAction::Amendment(_) => false,
                        } =>
                    {
                        if keyed.value.is_some() {
                            self.not_a_record(env, owner);
                            return;
                        }
                        if let Ok(Some(answer)) = crate::translate::llm::refused(
                            &w::Ask::Propose(value.clone()),
                            &self.settings.models,
                            &env.limits.wire,
                        ) {
                            self.respond(env, exchange, routes::encoded(&answer));
                            return;
                        }
                        let Ok(change) =
                            crate::translate::settings::proposed(value, &self.settings.models, &env.limits.wire)
                        else {
                            self.not_a_record(env, owner);
                            return;
                        };
                        let digest = sign_in::digest(&keyed.request).expose().try_into().expect("32-byte digest");
                        self.pending
                            .exchanges
                            .insert(exchange, Waiting::Ask { sign_in, family: None, version: keyed.version })
                            .expect("released slot");
                        out.push(Request::Core(Box::new(core::Event::ProposeSettings {
                            reply_to: ReplyTo::new(exchange),
                            sign_in,
                            key: keyed.key,
                            digest,
                            change: Box::new(change),
                        })));
                        return;
                    }
                    w::Ask::ChangeSettings(value) => {
                        if keyed.value.is_some() {
                            self.not_a_record(env, owner);
                            return;
                        }
                        let Ok(change) =
                            crate::translate::settings::change(value, &self.settings.models, &env.limits.wire)
                        else {
                            self.not_a_record(env, owner);
                            return;
                        };
                        let digest = sign_in::digest(&keyed.request).expose().try_into().expect("32-byte digest");
                        self.pending
                            .exchanges
                            .insert(exchange, Waiting::Ask { sign_in, family: None, version: keyed.version })
                            .expect("get released slot");
                        out.push(Request::Core(Box::new(core::Event::ChangeSettings {
                            reply_to: ReplyTo::new(exchange),
                            sign_in,
                            key: keyed.key,
                            digest,
                            change: Box::new(change),
                        })));
                        return;
                    }
                    other @ (w::Ask::AccountChange(_)
                    | w::Ask::Amend(_)
                    | w::Ask::Answer(_)
                    | w::Ask::Cancel(_)
                    | w::Ask::ChangePolicy(_)
                    | w::Ask::Compound(_)
                    | w::Ask::Create(_)
                    | w::Ask::Decide(_)
                    | w::Ask::DeleteNote(_)
                    | w::Ask::EditNote(_)
                    | w::Ask::HandBack(_)
                    | w::Ask::MakeProject(_)
                    | w::Ask::MakeService(_)
                    | w::Ask::Message(_)
                    | w::Ask::MoveTask(_)
                    | w::Ask::MoveToNewest(_)
                    | w::Ask::Propose(_)
                    | w::Ask::Release(_)
                    | w::Ask::Rename(_)
                    | w::Ask::Seen(_)
                    | w::Ask::SetPool(_)
                    | w::Ask::SetRoles(_)
                    | w::Ask::SignIn(_)
                    | w::Ask::SignOut(_)
                    | w::Ask::Lift(_)
                    | w::Ask::Stop(_)
                    | w::Ask::Take(_)
                    | w::Ask::Withdraw(_)) => other,
                };
                if let Some(carried) = &keyed.value {
                    let bound = carried_bound(
                        &value,
                        self.settings.key_bytes,
                        self.settings.moved_bytes,
                        self.settings.redirect_bytes,
                    );
                    if u64::try_from(carried.len()).expect("value size fits u64") > bound
                        && account_key_digest(&value).is_some()
                    {
                        let reply = people::Reply::Refused(people::Refusal::fixed(
                            people::LimitKind::Key,
                            bound,
                            people::LimitUnit::Bytes,
                        ));
                        let answer = crate::translate::answers::answer(&reply, &self.settings.models, &env.limits.wire);
                        self.answer(env, exchange, answer, out);
                        return;
                    }
                }
                let valid = match account_key_digest(&value) {
                    Some(digest) => match &keyed.value {
                        Some(value) => digest == skein_lib::sha256::digest(value.expose()),
                        None => false,
                    },
                    None => keyed.value.is_none(),
                };
                if !valid {
                    self.not_a_record(env, owner);
                    return;
                }
                if let Ok(Some(answer)) = crate::translate::operations::refused(&value, &env.limits.wire) {
                    self.respond(env, exchange, routes::encoded(&answer));
                    return;
                }
                let models = &self.settings.models;
                if let Ok(Some(answer)) = crate::translate::llm::refused(&value, models, &env.limits.wire) {
                    self.respond(env, exchange, routes::encoded(&answer));
                    return;
                }
                crate::translate::asks::named(value, models, &env.limits.wire)
            }
            Err(problem) => Err(crate::translate::Untranslatable { path: problem.path }),
        };
        let Ok(ask) = ask else {
            self.not_a_record(env, owner);
            return;
        };
        if let Some(value) = keyed.value {
            let digest = skein_lib::sha256::digest(value.expose());
            let request = CarriedRequest::AccountKey { key: keyed.key, ask: Box::new(ask.ask), digest };
            self.carry(env, exchange, sign_in, request, value, out);
            return;
        }
        let request = Request::Core(Box::new(core::Event::People(people::Event::Ask {
            reply_to: ReplyTo::new(exchange),
            sign_in,
            key: keyed.key,
            ask: ask.ask,
            models: ask.models,
        })));
        self.pending
            .exchanges
            .insert(exchange, Waiting::Ask { sign_in, family: None, version: keyed.version })
            .expect("get released slot");
        out.push(request);
    }
    fn not_a_record(&mut self, env: &Env<Limits>, owner: Token) {
        self.connections.get_mut(&owner).expect("exchange owner").respond(crate::Reply::NotARecord, &env.limits);
    }
    /// Keeps the value a request carried before the core sees the
    /// request, which names it by number and digest only
    /// (protocol/client.md, 5; store.md 9).
    fn carry(
        &mut self,
        env: &Env<Limits>,
        exchange: Token,
        sign_in: u64,
        request: CarriedRequest,
        value: skein_lib::Secret,
        out: &mut Queue<Request>,
    ) {
        if self.pending.carried.len() == self.pending.carried.capacity() {
            self.respond(env, exchange, routes::busy());
            return;
        }
        let (family, version) = match &request {
            CarriedRequest::Application(crate::ToApplication::Request { family, version, .. }) => {
                (Some(*family), *version)
            }
            CarriedRequest::AccountKey { .. } => (None, jig_client_protocol_wire::VERSION),
            CarriedRequest::Application(crate::ToApplication::Read { .. } | crate::ToApplication::Watch { .. }) => {
                unreachable!("only a request carries a value")
            }
        };
        let kind = match &request {
            CarriedRequest::AccountKey { ask, .. } => carried_kind(ask),
            CarriedRequest::Application(_) => Some(crate::CarriedKind::Value),
        };
        let waiting = Waiting::Carry { sign_in, family, version, request: Box::new(request) };
        self.pending.carried.insert(exchange, None).expect("carried below its capacity");
        self.pending.exchanges.insert(exchange, waiting).expect("get released slot");
        let request = match kind {
            None => crate::SecretsRequest::Hold { exchange, value },
            Some(kind) => crate::SecretsRequest::Carry { exchange, value, kind },
        };
        out.push(Request::Secrets(request));
    }
}

/// The value's digest is the sole secret-derived fact admitted to the core.
fn account_key_digest(ask: &w::Ask) -> Option<[u8; 32]> {
    let change = match ask {
        w::Ask::AccountChange(row) => row.change(),
        w::Ask::ChangeSettings(_)
        | w::Ask::MoveToNewest(_)
        | w::Ask::Message(_)
        | w::Ask::Answer(_)
        | w::Ask::Take(_)
        | w::Ask::HandBack(_)
        | w::Ask::Decide(_)
        | w::Ask::Release(_)
        | w::Ask::Amend(_)
        | w::Ask::Lift(_)
        | w::Ask::Stop(_)
        | w::Ask::Cancel(_)
        | w::Ask::MoveTask(_)
        | w::Ask::Seen(_)
        | w::Ask::EditNote(_)
        | w::Ask::DeleteNote(_)
        | w::Ask::ChangePolicy(_)
        | w::Ask::SetRoles(_)
        | w::Ask::MakeService(_)
        | w::Ask::Rename(_)
        | w::Ask::MakeProject(_)
        | w::Ask::SetPool(_)
        | w::Ask::Compound(_)
        | w::Ask::Create(_)
        | w::Ask::Propose(_)
        | w::Ask::Withdraw(_)
        | w::Ask::SignIn(_)
        | w::Ask::SignOut(_) => return None,
    };
    match change {
        w::AccountChange::AddKey(row) => Some(*row.digest()),
        w::AccountChange::ReplaceKey(row) => Some(*row.digest()),
        w::AccountChange::FinishSignIn(row) => Some(*row.digest()),
        w::AccountChange::MoveIn(row) => Some(*row.digest()),
        w::AccountChange::DeclareProgram(_)
        | w::AccountChange::CheckProgram(_)
        | w::AccountChange::RemoveKey(_)
        | w::AccountChange::Resume(_)
        | w::AccountChange::Remove(_)
        | w::AccountChange::MoveAccount(_)
        | w::AccountChange::MoveSource(_)
        | w::AccountChange::SetUses(_)
        | w::AccountChange::MoveOut(_)
        | w::AccountChange::Written(_) => None,
    }
}

/// Only account key changes and a moved file carry jig values.
fn carried_bound(ask: &w::Ask, key_bytes: u32, moved_bytes: u64, redirect_bytes: u32) -> u64 {
    let change = match ask {
        w::Ask::AccountChange(row) => row.change(),
        w::Ask::ChangeSettings(_)
        | w::Ask::MoveToNewest(_)
        | w::Ask::Message(_)
        | w::Ask::Answer(_)
        | w::Ask::Take(_)
        | w::Ask::HandBack(_)
        | w::Ask::Decide(_)
        | w::Ask::Release(_)
        | w::Ask::Amend(_)
        | w::Ask::Lift(_)
        | w::Ask::Stop(_)
        | w::Ask::Cancel(_)
        | w::Ask::MoveTask(_)
        | w::Ask::Seen(_)
        | w::Ask::EditNote(_)
        | w::Ask::DeleteNote(_)
        | w::Ask::ChangePolicy(_)
        | w::Ask::SetRoles(_)
        | w::Ask::MakeService(_)
        | w::Ask::Rename(_)
        | w::Ask::MakeProject(_)
        | w::Ask::SetPool(_)
        | w::Ask::Compound(_)
        | w::Ask::Create(_)
        | w::Ask::Propose(_)
        | w::Ask::Withdraw(_)
        | w::Ask::SignIn(_)
        | w::Ask::SignOut(_) => return 0,
    };
    match change {
        w::AccountChange::FinishSignIn(_) => u64::from(redirect_bytes),
        w::AccountChange::MoveIn(_) => moved_bytes,
        w::AccountChange::AddKey(_) | w::AccountChange::ReplaceKey(_) => u64::from(key_bytes),
        w::AccountChange::DeclareProgram(_)
        | w::AccountChange::CheckProgram(_)
        | w::AccountChange::RemoveKey(_)
        | w::AccountChange::Resume(_)
        | w::AccountChange::Remove(_)
        | w::AccountChange::MoveAccount(_)
        | w::AccountChange::MoveSource(_)
        | w::AccountChange::SetUses(_)
        | w::AccountChange::MoveOut(_)
        | w::AccountChange::Written(_) => 0,
    }
}
/// The pasted address is the sole jig value held without a durable carry.
fn carried_kind(ask: &people::Ask) -> Option<crate::CarriedKind> {
    match ask {
        people::Ask::AccountChange { change, .. } => match change {
            people::AccountChange::FinishSignIn { .. } => None,
            people::AccountChange::AddKey { .. } | people::AccountChange::ReplaceKey { .. } => {
                Some(crate::CarriedKind::Key)
            }
            people::AccountChange::MoveIn { .. } => Some(crate::CarriedKind::Moved),
            people::AccountChange::DeclareProgram { .. }
            | people::AccountChange::CheckProgram { .. }
            | people::AccountChange::RemoveKey { .. }
            | people::AccountChange::Resume { .. }
            | people::AccountChange::Remove { .. }
            | people::AccountChange::MoveAccount { .. }
            | people::AccountChange::MoveSource { .. }
            | people::AccountChange::SetUses { .. }
            | people::AccountChange::MoveOut { .. }
            | people::AccountChange::Written { .. } => {
                unreachable!("only a key, moved file or pasted address carries a value")
            }
        },
        people::Ask::ChangeSettings { .. }
        | people::Ask::MoveToNewest { .. }
        | people::Ask::Compound { .. }
        | people::Ask::Create { .. }
        | people::Ask::MarkSeen { .. }
        | people::Ask::Application { .. }
        | people::Ask::EditNote { .. }
        | people::Ask::MakeService { .. }
        | people::Ask::Adopt { .. }
        | people::Ask::Propose { .. }
        | people::Ask::Withdraw { .. }
        | people::Ask::Stop { .. }
        | people::Ask::Cancel { .. }
        | people::Ask::Lift { .. }
        | people::Ask::Release { .. }
        | people::Ask::TakePerson { .. }
        | people::Ask::HandBackPerson { .. }
        | people::Ask::AnswerPerson { .. }
        | people::Ask::Move { .. }
        | people::Ask::DecideProposal { .. }
        | people::Ask::Say { .. }
        | people::Ask::AnswerQuestion { .. }
        | people::Ask::Amend { .. }
        | people::Ask::SetRoles { .. }
        | people::Ask::ChangePolicy { .. }
        | people::Ask::Rename { .. }
        | people::Ask::MakeProject { .. }
        | people::Ask::SetPool { .. }
        | people::Ask::DecideEscalation { .. }
        | people::Ask::AccountSignIn { .. }
        | people::Ask::AccountSignOut { .. } => {
            unreachable!("only a key, moved file or pasted address carries a value")
        }
    }
}
