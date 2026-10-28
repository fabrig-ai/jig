//! Read exchanges remain pending until the core's held answer is released (client.md, 6).
use super::{Box, Component, Env, Limits, Queue, ReplyTo, Request, Token, Waiting, core, routes, w};
use skein_lib::{Reader, Writer};
impl Component {
    pub(super) fn serves(&self, family: Option<u16>, version: u32) -> bool {
        for range in &self.settings.served {
            if range.family == family && range.first <= version && version <= range.last {
                return true;
            }
        }
        false
    }
    pub(super) fn read(
        &mut self,
        env: &Env<Limits>,
        exchange: Token,
        sign_in: u64,
        body: &[u8],
        out: &mut Queue<Request>,
    ) {
        let Some(owner) = self.owner(exchange) else { return };
        let active = self.active(sign_in);
        if active >= env.limits.exchanges_per_client || self.admitted() >= env.limits.exchanges {
            self.respond(env, exchange, routes::read_none(core::reads::Why::Busy, &env.limits.wire));
            return;
        }
        let mut reader = Reader::new(body);
        let record = match w::Owned::decode(&env.limits.wire, &mut reader) {
            Ok(value) if reader.is_empty() => value.into_parts(),
            Ok(_) | Err(_) => {
                self.connections
                    .get_mut(&owner)
                    .expect("exchange owner")
                    .respond(crate::Reply::NotARecord, &env.limits);
                return;
            }
        };
        let family = match &record.owner {
            w::Owner::Jig => None,
            w::Owner::Family(value) => Some(value.number()),
        };
        if !self.serves(family, record.version) {
            let answer =
                w::ReadAnswer::NotServed(crate::served::not_served(&self.settings, record.owner, &env.limits.wire));
            self.respond(env, exchange, routes::encoded_read(&answer));
            return;
        }
        let request = match record.owner {
            w::Owner::Jig => {
                let Some(what) = crate::translate::reads::decode(&record.record, &env.limits.wire) else {
                    self.respond(env, exchange, routes::read_none(core::reads::Why::Unknown, &env.limits.wire));
                    return;
                };
                let request = Request::Core(Box::new(core::Event::Read {
                    reply: ReplyTo::new(exchange),
                    sign_in,
                    read: what.clone(),
                }));
                self.pending
                    .exchanges
                    .insert(exchange, Waiting::Read { sign_in, what: Box::new(what) })
                    .expect("same read slot");
                request
            }
            w::Owner::Family(value) => {
                let Ok(body) = w::FamilyRecord::decode(&env.limits.wire, &mut Reader::new(&record.record)) else {
                    self.connections
                        .get_mut(&owner)
                        .expect("exchange owner")
                        .respond(crate::Reply::NotARecord, &env.limits);
                    return;
                };
                let Some(window) = crate::served::window(body.window()) else {
                    self.connections
                        .get_mut(&owner)
                        .expect("exchange owner")
                        .respond(crate::Reply::NotARecord, &env.limits);
                    return;
                };
                let scope = crate::served::scope(body.scope());
                let family = value.number();
                self.pending
                    .exchanges
                    .insert(exchange, Waiting::ApplicationRead { sign_in, family, version: record.version })
                    .expect("same read slot");
                Request::Application(crate::ToApplication::Read {
                    reply: ReplyTo::new(exchange),
                    sign_in,
                    family,
                    version: record.version,
                    scope,
                    window,
                    record: body.into_parts().record,
                })
            }
        };
        out.push(request);
    }
    pub(super) fn read_answer(
        &mut self,
        env: &Env<Limits>,
        exchange: Token,
        answer: &core::reads::Answer,
        out: &mut Queue<Request>,
    ) {
        let Some(waiting) = self.pending.exchanges.remove(&exchange) else { return };
        match waiting {
            Waiting::Read { what, .. } => {
                self.read_response(
                    env,
                    exchange,
                    crate::translate::reads::answer(&what, answer, &self.settings.models, &env.limits.wire),
                    out,
                );
            }
            waiting @ Waiting::ApplicationRead { .. } => match answer {
                core::reads::Answer::None(why) => {
                    self.respond(env, exchange, routes::read_none(*why, &env.limits.wire));
                }
                core::reads::Answer::Setting(_)
                | core::reads::Answer::Revision(_)
                | core::reads::Answer::Revisions(_)
                | core::reads::Answer::Holds(_)
                | core::reads::Answer::Tasks(_)
                | core::reads::Answer::Document(_)
                | core::reads::Answer::Historical(_)
                | core::reads::Answer::Page(_) => {
                    self.pending.exchanges.insert(exchange, waiting).expect("same unrelated slot");
                }
            },
            Waiting::Authorize { .. }
            | Waiting::Core { .. }
            | Waiting::LinkGet { .. }
            | Waiting::SignOut { .. }
            | Waiting::Watch { .. }
            | Waiting::Keep { .. }
            | Waiting::Get { .. }
            | Waiting::Moving { .. }
            | Waiting::Made { .. }
            | Waiting::Ask { .. }
            | Waiting::Carry { .. }
            | Waiting::Checked { .. } => {
                self.pending.exchanges.insert(exchange, waiting).expect("same unrelated slot");
            }
        }
    }
    pub(super) fn application_read(
        &mut self,
        env: &Env<Limits>,
        exchange: Token,
        family: u16,
        record: Option<Box<[u8]>>,
        out: &mut Queue<Request>,
    ) {
        let Some(waiting) = self.pending.exchanges.remove(&exchange) else { return };
        match waiting {
            Waiting::ApplicationRead { family: number, version, .. } if number == family => {
                let answer = match record {
                    Some(record) => match w::Family::new(&env.limits.wire, w::FamilyParts { number }) {
                        Ok(family) => {
                            crate::translate::reads::owned(w::Owner::Family(family), version, record, &env.limits.wire)
                        }
                        Err(problem) => Err(problem),
                    },
                    None => Ok(crate::translate::reads::none(core::reads::Why::Unknown, &env.limits.wire)),
                };
                self.read_response(env, exchange, answer, out);
            }
            Waiting::Authorize { .. }
            | Waiting::Core { .. }
            | Waiting::LinkGet { .. }
            | Waiting::SignOut { .. }
            | Waiting::Watch { .. }
            | Waiting::Keep { .. }
            | Waiting::Get { .. }
            | Waiting::Moving { .. }
            | Waiting::Made { .. }
            | Waiting::Ask { .. }
            | Waiting::Carry { .. }
            | Waiting::Read { .. }
            | Waiting::ApplicationRead { .. }
            | Waiting::Checked { .. } => {
                self.pending.exchanges.insert(exchange, waiting).expect("same unrelated slot");
            }
        }
    }
    fn read_response(
        &mut self,
        env: &Env<Limits>,
        exchange: Token,
        answer: Result<w::ReadAnswer, w::Problem>,
        out: &mut Queue<Request>,
    ) {
        let Some(owner) = self.owner(exchange) else { return };
        match answer {
            Ok(value) => {
                let mut bytes = Writer::new(usize::try_from(value.measure()).expect("wire measure fits"));
                value.encode(&mut bytes).expect("measured read answer");
                self.connections
                    .get_mut(&owner)
                    .expect("exchange owner")
                    .respond(crate::Reply::Record(bytes.finish()), &env.limits);
            }
            Err(_) => self.connections.get_mut(&owner).expect("exchange owner").close(out),
        }
    }
}
