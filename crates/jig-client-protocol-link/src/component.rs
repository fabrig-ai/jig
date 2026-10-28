//! One bounded exchange per request-table attempt (protocol/client.md, 5).
//! A family's admission refusal uses jig's wire (contract.md, 3.3).
use crate::{Event, Fact, Limits, Message, Request, Streamed, Unusable, translate};
use alloc::boxed::Box;
use jig_client_domain::{Answered, Ask, Family, Readable, Readout, Retry, Watchable};
use jig_client_protocol_wire::{self as wire, v19 as w};
use skein_lib::{Map, Queue, Reader, RequestEnvelope, RequestKey, Secret, Time, Token, Writer};
#[derive(Debug)]
pub(crate) struct Attempt {
    pub(super) token: Token,
    pub(super) owner: w::Owner,
    pub(super) version: u32,
    pub(super) kind: ExchangeKind,
    pub(super) held: Option<Secret>,
}
impl Attempt {
    pub(super) fn carrying(&self) -> bool {
        match self.kind {
            ExchangeKind::Attempt { carrying } => carrying,
            ExchangeKind::Authorize
            | ExchangeKind::SignIn
            | ExchangeKind::SignOut { .. }
            | ExchangeKind::Read { .. }
            | ExchangeKind::WatchOpen { .. }
            | ExchangeKind::WatchClose { .. } => false,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub(super) enum ExchangeKind {
    Authorize,
    SignIn,
    SignOut { generation: u64 },
    // `carrying`: an exchange carrying a value either way, sent once and never held
    // behind the store (protocol/client.md, 5 and 10).
    Attempt { carrying: bool },
    Read { turn: Option<(u64, u64, u32)> },
    WatchOpen { name: u64 },
    WatchClose { name: u64 },
}
#[derive(Debug)]
pub struct Link {
    pub(super) limits: Limits,
    pub(super) attempts: Map<Token, Attempt>,
    pub(super) next: u64,
    pub(super) credential: Option<Secret>,
    pub(super) generation: u64,
    stream: Option<crate::silence::Silence>,
    pub(super) predecessor: Option<[u8; 16]>,
    retiring: Option<Token>,
    watches: crate::watches::Watches,
    pub(super) store: crate::store::Store,
}
impl Link {
    pub fn new(limits: &Limits) -> Result<Self, Unusable> {
        if !crate::valid(limits) {
            return Err(Unusable::Limits);
        }
        Ok(Self {
            limits: *limits,
            store: crate::store::Store::new(limits),
            attempts: Map::with_capacity(limits.exchanges),
            next: 1,
            credential: None,
            generation: 0,
            stream: None,
            predecessor: None,
            retiring: None,
            watches: crate::watches::Watches::new(limits),
        })
    }
    #[must_use]
    pub fn max_out(&self) -> u32 {
        crate::limits::max_out(&self.limits).expect("validated output bound")
    }
    #[must_use]
    pub fn next_deadline(&self) -> Option<Time> {
        let Some(stream) = &self.stream else { return None };
        Some(stream.silence())
    }
    /// Whether a read can own an exchange now; a completion or cancellation releases room.
    #[must_use]
    pub fn has_exchange_room(&self) -> bool {
        self.attempts.len() < self.attempts.capacity()
    }
    /// Number of logical exchanges, without their tokens or payloads.
    #[must_use]
    pub fn in_flight(&self) -> u32 {
        self.attempts.len()
    }
    pub fn reclaim(&mut self) {}
    #[must_use]
    pub fn worst_case(limits: &Limits) -> Option<u64> {
        crate::worst_case(limits)
    }
    pub fn step(&mut self, now: Time, event: Event, out: &mut Queue<Request>) {
        if let Event::Reply { exchange, .. } | Event::Lost { exchange } = &event
            && self.retiring == Some(*exchange)
        {
            self.retiring = None;
            if !self.watches.0.is_empty() {
                self.begin_stream(now, out);
            }
            return;
        }
        match event {
            Event::SignIn { with } => self.sign_in(now, with, out),
            Event::SignOut => self.sign_out(now, out),
            Event::Authorize { query } => self.authorize(query, out),
            Event::Start { kept } => self.start_store(kept, out),
            Event::Save { record } => self.save(record, out),
            Event::Erase { key } => self.erase(key, out),
            Event::Stored => self.written(out),
            Event::NotStored => self.write_failed(out),
            Event::Open { watch, subject } => self.open(now, watch, subject, out),
            Event::Continue { watch, subject } => {
                self.forget(watch, out);
                self.open(now, watch, Watchable::Jig(subject), out);
            }
            Event::Close { watch } => self.close(now, watch, out),
            Event::Streamed { message } => self.streamed(now, message, out),
            Event::StreamGone => self.drop_stream(now, out),
            Event::Send { attempt, key, ask, value } => self.send(now, attempt, key, ask, value, out),
            Event::Read { read, what } => self.read(now, read, what, out),
            Event::Reply { exchange, message } => self.answered(exchange, &message, out),
            Event::Lost { exchange } => {
                if let Some(attempt) = self.attempts.remove(&exchange) {
                    self.failure(attempt, out);
                }
            }
        }
    }
    pub fn fire(&mut self, now: Time, out: &mut Queue<Request>) {
        if let Some(stream) = &self.stream
            && now >= stream.silence()
        {
            self.drop_stream(now, out);
        }
    }
    fn send(
        &mut self,
        _now: Time,
        attempt: Token,
        key: RequestKey,
        ask: Ask,
        value: Option<Secret>,
        out: &mut Queue<Request>,
    ) {
        let mut carrying = value.is_some();
        if let Ask::Jig(ask) = &ask
            && let jig_client_domain::contract::Ask::AccountChange {
                change: jig_client_domain::contract::AccountChange::MoveOut { .. },
                ..
            } = ask.as_ref()
        {
            carrying = true;
        }
        // An exchange carrying a value either way never left if it cannot be sent.
        let refuse: fn(Token, &mut Queue<Request>) = if carrying { unsent } else { lost };
        if self.attempts.len() == self.attempts.capacity() {
            refuse(attempt, out);
            return;
        }
        let Some(next) = self.next.checked_add(1) else {
            refuse(attempt, out);
            return;
        };
        let Ok(record) = keyed(key, ask, value, &self.limits.wire) else {
            refuse(attempt, out);
            return;
        };
        let exchange = Token::new(self.next);
        self.next = next;
        let state = Attempt {
            held: None,
            token: attempt,
            owner: record.owner().clone(),
            version: record.version(),
            kind: ExchangeKind::Attempt { carrying },
        };
        self.attempts.insert(exchange, state).expect("checked exchange capacity");
        let mut writer = Writer::new(usize::try_from(record.measure()).expect("u32 fits usize"));
        record.encode(&mut writer).expect("measured envelope");
        self.exchange(exchange, Message::Request(writer.finish()), out);
    }
    fn read(&mut self, _now: Time, read: Token, what: Readable, out: &mut Queue<Request>) {
        let turn = if let Readable::Jig(jig_client_domain::contract::Read::Turn { task, attempt, turn }) = &what {
            Some((*task, *attempt, *turn))
        } else {
            None
        };
        let owner = match &what {
            Readable::Jig(_) => w::Owner::Jig,
            Readable::Family { record: value, .. } => w::Owner::Family(
                w::Family::new(&self.limits.wire, w::FamilyParts { number: value.family })
                    .expect("unbounded family number"),
            ),
        };
        if self.attempts.len() == self.attempts.capacity() {
            read_none(read, &owner, jig_client_domain::contract::ReadNone::Unreachable, out);
            return;
        }
        let Some(next) = self.next.checked_add(1) else {
            read_none(read, &owner, jig_client_domain::contract::ReadNone::Unreachable, out);
            return;
        };
        let Ok(record) = translate::reads::owned(what, &self.limits.wire) else {
            read_none(read, &owner, jig_client_domain::contract::ReadNone::Unreachable, out);
            return;
        };
        let exchange = Token::new(self.next);
        self.next = next;
        let state =
            Attempt { held: None, token: read, owner, version: record.version(), kind: ExchangeKind::Read { turn } };
        self.attempts.insert(exchange, state).expect("checked exchange capacity");
        let mut writer = Writer::new(usize::try_from(record.measure()).expect("u32 fits usize"));
        record.encode(&mut writer).expect("measured read envelope");
        self.exchange(exchange, Message::Read(writer.finish()), out);
    }
    fn answered(&mut self, exchange: Token, body: &[u8], out: &mut Queue<Request>) {
        let Some(attempt) = self.attempts.remove(&exchange) else { return };
        match attempt.kind {
            ExchangeKind::SignIn | ExchangeKind::SignOut { .. } => {
                self.signing_answer(attempt.kind, attempt.held, body, out);
                return;
            }
            ExchangeKind::Authorize => {
                self.authorized(body, out);
                return;
            }
            ExchangeKind::Attempt { .. } => {}
            ExchangeKind::WatchOpen { name } | ExchangeKind::WatchClose { name } => {
                self.watch_answer(attempt, name, body, out);
                return;
            }
            ExchangeKind::Read { .. } => {
                self.read_answer(attempt, body, out);
                return;
            }
        }
        let Ok(record) = w::KeyedAnswer::decode(&self.limits.wire, &mut Reader::new(body)) else {
            out.push(Request::Fact { fact: Fact::UndecodableAnswer });
            out.push(Request::Answer {
                attempt: attempt.token,
                envelope: RequestEnvelope::Lost,
                answer: None,
                why: Some(Retry::Undecodable),
            });
            return;
        };
        if let w::KeyedAnswer::NotServed(value) = &record {
            translate::envelope::not_served(value, out);
            out.push(Request::Answer {
                attempt: attempt.token,
                envelope: RequestEnvelope::Again,
                answer: None,
                why: None,
            });
            return;
        }
        let why = match &record {
            w::KeyedAnswer::Busy => Some(Retry::Busy),
            w::KeyedAnswer::AnswersFull(value) => {
                Some(Retry::Full { held: value.held(), room_at: skein_lib::Wall::from_nanos(value.room_at()) })
            }
            w::KeyedAnswer::NotReady => Some(Retry::NotReady),
            w::KeyedAnswer::SignedOut | w::KeyedAnswer::NotServed(_) => None,
            w::KeyedAnswer::FinalAnswer(_) => Some(Retry::Undecodable),
        };
        let envelope = match record {
            w::KeyedAnswer::NotServed(_) => unreachable!("version refusal returned"),
            w::KeyedAnswer::Busy | w::KeyedAnswer::NotReady | w::KeyedAnswer::AnswersFull(_) => RequestEnvelope::Again,
            w::KeyedAnswer::SignedOut => RequestEnvelope::SignedOut,
            w::KeyedAnswer::FinalAnswer(record) => {
                let credential = translate::answers::credential(&record, &self.limits);
                let answer = self.final_answer(&attempt, record);
                match answer {
                    Ok(answer) => {
                        if let Some((party, credential)) = credential {
                            out.push(Request::ServiceCredential { party, credential });
                        }
                        out.push(Request::Answer {
                            attempt: attempt.token,
                            envelope: RequestEnvelope::Final,
                            answer: Some(answer),
                            why: None,
                        });
                        return;
                    }
                    Err(_) => {
                        out.push(Request::Fact { fact: Fact::UndecodableAnswer });
                        RequestEnvelope::Lost
                    }
                }
            }
        };
        if envelope == RequestEnvelope::SignedOut {
            self.end_sign_in(out);
            out.push(Request::Signing { signing: crate::Signing::SignedOut });
        }
        out.push(Request::Answer { attempt: attempt.token, envelope, answer: None, why });
    }
    fn final_answer(&self, attempt: &Attempt, record: w::Owned) -> Result<Answered, translate::Undecodable> {
        if record.owner() == &attempt.owner && record.version() == attempt.version {
            return match record.owner() {
                w::Owner::Jig => match translate::answers::answer(w::KeyedAnswer::FinalAnswer(record), &self.limits) {
                    Ok(answer) => Ok(Answered::Jig(answer)),
                    Err(problem) => Err(problem),
                },
                w::Owner::Family(owner) => Ok(Answered::Family(Family {
                    family: owner.number(),
                    version: record.version(),
                    bytes: skein_lib::bytes::copy_of(record.record()),
                })),
            };
        }
        if record.owner() == &w::Owner::Jig && record.version() == wire::VERSION && attempt.owner != w::Owner::Jig {
            let answer = translate::answers::answer(w::KeyedAnswer::FinalAnswer(record), &self.limits)?;
            return match answer {
                answer @ jig_client_domain::contract::Answer::Refused { .. } => Ok(Answered::Jig(answer)),
                jig_client_domain::contract::Answer::Done(_) => Err(translate::Undecodable::Version),
            };
        }
        Err(translate::Undecodable::Version)
    }
    fn read_answer(&mut self, attempt: Attempt, body: &[u8], out: &mut Queue<Request>) {
        if let ExchangeKind::Read { turn: Some(identity) } = attempt.kind {
            match translate::turn::inspect(body, identity, &self.limits) {
                Ok(None) => {}
                Ok(Some(why)) => {
                    read_none(attempt.token, &attempt.owner, why, out);
                    return;
                }
                Err(_) => {
                    read_none(attempt.token, &attempt.owner, jig_client_domain::contract::ReadNone::Malformed, out);
                    return;
                }
            }
        }
        let result = match w::ReadAnswer::decode(&self.limits.wire, &mut Reader::new(body)) {
            Ok(w::ReadAnswer::NotServed(value)) => {
                translate::envelope::not_served(&value, out);
                out.push(Request::Read {
                    read: attempt.token,
                    result: Readout::NotServed {
                        family: translate::envelope::family(value.owner()),
                        served: translate::envelope::versions(&value),
                    },
                });
                return;
            }
            Ok(w::ReadAnswer::None(record)) => {
                if record.value() == &w::ReadNone::SignedOut {
                    self.end_sign_in(out);
                    out.push(Request::Signing { signing: crate::Signing::SignedOut });
                }
                read_none(attempt.token, &attempt.owner, translate::reads::none(record.value()), out);
                return;
            }
            Ok(w::ReadAnswer::Value(record)) => read_value(&attempt, record, &self.limits),
            Err(_) => Err(translate::Undecodable::Malformed),
        };
        match result {
            Ok(result) => out.push(Request::Read { read: attempt.token, result }),
            Err(_) => {
                out.push(Request::Fact { fact: Fact::UndecodableDocument });
                match attempt.kind {
                    ExchangeKind::Read { turn: Some(_) } => {
                        read_none(attempt.token, &attempt.owner, jig_client_domain::contract::ReadNone::Malformed, out);
                    }
                    ExchangeKind::Read { turn: None }
                    | ExchangeKind::Authorize
                    | ExchangeKind::Attempt { .. }
                    | ExchangeKind::SignIn
                    | ExchangeKind::SignOut { .. }
                    | ExchangeKind::WatchOpen { .. }
                    | ExchangeKind::WatchClose { .. } => failed(attempt, out),
                }
            }
        }
    }
}
fn read_value(attempt: &Attempt, record: w::Owned, limits: &Limits) -> Result<Readout, translate::Undecodable> {
    if record.owner() != &attempt.owner || record.version() != attempt.version {
        return Err(translate::Undecodable::Version);
    }
    match record.owner() {
        w::Owner::Jig => {
            let value = translate::reads::value(record.record(), limits)?;
            Ok(Readout::Jig(jig_client_domain::contract::ReadResult::Value(value)))
        }
        w::Owner::Family(owner) => {
            let family = owner.number();
            Ok(Readout::Family { family, record: Some(record.into_parts().record) })
        }
    }
}
fn read_none(read: Token, owner: &w::Owner, why: jig_client_domain::contract::ReadNone, out: &mut Queue<Request>) {
    let result = match owner {
        w::Owner::Jig => Readout::Jig(jig_client_domain::contract::ReadResult::None(why)),
        w::Owner::Family(owner) => Readout::Family { family: owner.number(), record: None },
    };
    out.push(Request::Read { read, result });
}
fn failed(attempt: Attempt, out: &mut Queue<Request>) {
    match attempt.kind {
        ExchangeKind::Authorize => out.push(Request::Authorized(crate::Authorized::Lost)),
        ExchangeKind::Attempt { .. } => lost(attempt.token, out),
        ExchangeKind::SignIn | ExchangeKind::SignOut { .. } => {
            out.push(Request::Signing { signing: crate::Signing::Lost });
        }
        ExchangeKind::WatchOpen { .. } | ExchangeKind::WatchClose { .. } => {
            unreachable!("watch failure belongs to its map")
        }
        ExchangeKind::Read { .. } => {
            read_none(attempt.token, &attempt.owner, jig_client_domain::contract::ReadNone::Unreachable, out);
        }
    }
}
fn lost(attempt: Token, out: &mut Queue<Request>) {
    out.push(Request::Answer { attempt, envelope: RequestEnvelope::Lost, answer: None, why: Some(Retry::Lost) });
}
fn unsent(attempt: Token, out: &mut Queue<Request>) {
    out.push(Request::Answer { attempt, envelope: RequestEnvelope::Again, answer: None, why: None });
}
fn keyed(key: RequestKey, ask: Ask, value: Option<Secret>, limits: &w::Limits) -> Result<w::Keyed, w::Problem> {
    let (owner, version, request) = match ask {
        Ask::Jig(ask) => {
            let record = translate::asks::ask(&ask, limits)?;
            let mut writer = Writer::new(usize::try_from(record.measure()).expect("u32 fits usize"));
            record.encode(&mut writer).expect("measured ask");
            (w::Owner::Jig, wire::VERSION, writer.finish())
        }
        Ask::Family { scope, record } => {
            let owner = w::Family::new(limits, w::FamilyParts { number: record.family })?;
            (w::Owner::Family(owner), record.version, translate::envelope::request(scope, &record, limits)?)
        }
    };
    w::Keyed::new(limits, w::KeyedParts { key, owner, version, request, value })
}

impl Link {
    pub(super) fn token(&mut self) -> Option<Token> {
        let next = self.next.checked_add(1)?;
        let token = Token::new(self.next);
        self.next = next;
        Some(token)
    }
    fn open(&mut self, now: Time, watch: Token, subject: Watchable, out: &mut Queue<Request>) {
        if self.watches.0.contains_key(&watch) {
            return;
        }
        if self.watches.0.len() == self.watches.0.capacity() {
            out.push(Request::Ended { watch, end: jig_client_domain::contract::WatchEnd::Busy });
            return;
        }
        let Ok(subject) = translate::watches::owned(subject, &self.limits.wire) else {
            out.push(Request::Ended { watch, end: jig_client_domain::contract::WatchEnd::Dropped });
            return;
        };
        let Some(name) = self.token() else {
            out.push(Request::Ended { watch, end: jig_client_domain::contract::WatchEnd::Dropped });
            return;
        };
        let opening = crate::watches::Opening {
            name: name.raw(),
            owner: subject.owner().clone(),
            version: subject.version(),
            subject: Some(subject),
            live: false,
            closing: false,
        };
        self.watches.0.insert(watch, opening).expect("checked watch capacity");
        match &self.stream {
            None if self.retiring.is_none() => {
                if let Some(stream) = self.predecessor.take() {
                    self.retiring = Some(name);
                    self.gone(name, stream, out);
                } else {
                    self.begin_stream(now, out);
                }
            }
            Some(stream) if stream.name.is_some() => self.send_open(now, watch, out),
            None | Some(_) => {}
        }
    }
    fn begin_stream(&mut self, now: Time, out: &mut Queue<Request>) {
        self.stream = Some(crate::silence::Silence::new(self.limits.client.heartbeat, now));
        out.push(Request::OpenStream { credential: self.credential.clone() });
    }
    fn send_open(&mut self, _now: Time, watch: Token, out: &mut Queue<Request>) {
        let Some(stream) = &self.stream else { return };
        let Some(stream) = stream.name else { return };
        if self.attempts.len() == self.attempts.capacity() {
            self.watches.end(watch, jig_client_domain::contract::WatchEnd::Busy, out);
            return;
        }
        let Some(exchange) = self.token() else {
            self.watches.end(watch, jig_client_domain::contract::WatchEnd::Dropped, out);
            return;
        };
        let opening = self.watches.0.get_mut(&watch).expect("pending opening");
        let Some(subject) = opening.subject.take() else { return };
        let record = w::Open::new(&self.limits.wire, w::OpenParts { stream, watch: opening.name, subject })
            .expect("checked owned subject");
        let attempt = Attempt {
            held: None,
            token: watch,
            owner: opening.owner.clone(),
            version: opening.version,
            kind: ExchangeKind::WatchOpen { name: opening.name },
        };
        self.attempts.insert(exchange, attempt).expect("checked opening exchange capacity");
        let mut writer = Writer::new(usize::try_from(record.measure()).expect("u32 fits usize"));
        record.encode(&mut writer).expect("measured opening");
        self.exchange(exchange, Message::Watch(writer.finish()), out);
    }
    fn close(&mut self, now: Time, watch: Token, out: &mut Queue<Request>) {
        let Some(opening) = self.watches.0.get(&watch) else { return };
        if opening.closing {
            return;
        }
        if opening.subject.is_some() {
            self.watches.end(watch, jig_client_domain::contract::WatchEnd::Closed, out);
            return;
        }
        let name = opening.name;
        let Some(stream) = &self.stream else { return };
        let stream = stream.name.expect("sent opening has hello");
        self.cancel_open(watch, name, out);
        if self.attempts.len() == self.attempts.capacity() {
            self.watches.end(watch, jig_client_domain::contract::WatchEnd::Dropped, out);
            return;
        }
        let Some(exchange) = self.token() else { return self.drop_stream(now, out) };
        let opening = self.watches.0.get_mut(&watch).expect("closing watch");
        opening.closing = true;
        let attempt = Attempt {
            held: None,
            token: watch,
            owner: opening.owner.clone(),
            version: opening.version,
            kind: ExchangeKind::WatchClose { name },
        };
        self.attempts.insert(exchange, attempt).expect("checked closing exchange capacity");
        self.unwatch(exchange, stream, name, out);
    }
    fn cancel_open(&mut self, watch: Token, name: u64, _out: &mut Queue<Request>) {
        let mut tokens = skein_lib::List::with_capacity(self.limits.exchanges);
        for (exchange, attempt) in &self.attempts {
            match attempt.kind {
                ExchangeKind::WatchOpen { name: old } | ExchangeKind::WatchClose { name: old } => {
                    if attempt.token == watch && old == name {
                        tokens.push(*exchange).expect("bounded exchanges");
                    }
                }
                ExchangeKind::Authorize
                | ExchangeKind::Attempt { .. }
                | ExchangeKind::Read { .. }
                | ExchangeKind::SignIn
                | ExchangeKind::SignOut { .. } => {}
            }
        }
        for exchange in &tokens {
            assert!(self.attempts.remove(exchange).is_some(), "collected live watch exchange");
        }
    }
    fn forget(&mut self, watch: Token, out: &mut Queue<Request>) {
        if let Some(opening) = self.watches.0.remove(&watch) {
            self.cancel_open(watch, opening.name, out);
            if opening.subject.is_none()
                && let Some(stream) = &self.stream
            {
                let stream = stream.name.expect("sent opening has hello");
                if let Some(exchange) = self.token() {
                    self.unwatch(exchange, stream, opening.name, out);
                }
            }
        }
    }
    fn unwatch(&mut self, exchange: Token, stream: [u8; 16], watch: u64, out: &mut Queue<Request>) {
        let record =
            w::WatchClose::new(&self.limits.wire, w::WatchCloseParts { stream, watch }).expect("fixed closing names");
        let mut writer = Writer::new(usize::try_from(record.measure()).expect("u32 fits usize"));
        record.encode(&mut writer).expect("measured closing");
        self.exchange(exchange, Message::Unwatch(writer.finish()), out);
    }
    fn watch_answer(&mut self, attempt: Attempt, name: u64, body: &[u8], out: &mut Queue<Request>) {
        if self.watches.named(name) != Some(attempt.token) {
            return;
        }
        if let ExchangeKind::WatchOpen { .. } = attempt.kind
            && self.watches.0.get(&attempt.token).expect("named opening").live
        {
            return;
        }
        match attempt.kind {
            ExchangeKind::WatchOpen { .. } => match w::OpenAnswer::decode(&self.limits.wire, &mut Reader::new(body)) {
                Ok(w::OpenAnswer::NotServed(value)) => {
                    translate::envelope::not_served(&value, out);
                    self.watches.end(attempt.token, jig_client_domain::contract::WatchEnd::NotServed, out);
                }
                Ok(w::OpenAnswer::Admitted) => self.watches.opened(attempt.token, out),
                Ok(w::OpenAnswer::Refused(why)) => {
                    let end = translate::watches::refused(why.why());
                    self.watches.end(attempt.token, end, out);
                    if end == jig_client_domain::contract::WatchEnd::SignedOut {
                        self.end_sign_in(out);
                        out.push(Request::Signing { signing: crate::Signing::SignedOut });
                    }
                }
                Err(_) => self.failure(attempt, out),
            },
            ExchangeKind::WatchClose { .. } => {
                match w::CloseAnswer::decode(&self.limits.wire, &mut Reader::new(body)) {
                    Ok(w::CloseAnswer::Closed) => {
                        self.watches.end(attempt.token, jig_client_domain::contract::WatchEnd::Closed, out);
                    }
                    Ok(w::CloseAnswer::SignedOut) => {
                        self.watches.end(attempt.token, jig_client_domain::contract::WatchEnd::SignedOut, out);
                        self.end_sign_in(out);
                        out.push(Request::Signing { signing: crate::Signing::SignedOut });
                    }
                    Err(_) => self.failure(attempt, out),
                }
            }
            ExchangeKind::Authorize
            | ExchangeKind::Attempt { .. }
            | ExchangeKind::Read { .. }
            | ExchangeKind::SignIn
            | ExchangeKind::SignOut { .. } => {
                unreachable!("watch exchange kind")
            }
        }
    }
    pub(super) fn failure(&mut self, attempt: Attempt, out: &mut Queue<Request>) {
        match attempt.kind {
            ExchangeKind::WatchOpen { name } => {
                if let Some(opening) = self.watches.0.get(&attempt.token)
                    && opening.name == name
                    && !opening.live
                {
                    self.watches.end(attempt.token, jig_client_domain::contract::WatchEnd::Dropped, out);
                }
            }
            ExchangeKind::WatchClose { name } => {
                if self.watches.named(name) == Some(attempt.token) {
                    self.watches.end(attempt.token, jig_client_domain::contract::WatchEnd::Dropped, out);
                }
            }
            ExchangeKind::SignOut { generation } if generation != self.generation => {}
            ExchangeKind::SignIn | ExchangeKind::SignOut { .. } => {
                out.push(Request::Signing { signing: crate::Signing::Lost });
            }
            ExchangeKind::Authorize | ExchangeKind::Attempt { .. } | ExchangeKind::Read { .. } => failed(attempt, out),
        }
    }
    fn streamed(&mut self, now: Time, message: Streamed, out: &mut Queue<Request>) {
        let Some(stream) = &mut self.stream else { return };
        stream.progress(now);
        let result = match message {
            Streamed::Heartbeat => {
                self.watches.alive(out);
                return;
            }
            Streamed::SignedOut => {
                self.end_sign_in(out);
                out.push(Request::Signing { signing: crate::Signing::SignedOut });
                return;
            }
            Streamed::Record(record) => match translate::pages::receiving(&self.limits) {
                Ok(receiving) => {
                    let mut reader = Reader::new(&record);
                    match w::StreamEvent::decode(&receiving.wire, &mut reader) {
                        Ok(record) if reader.remaining() == 0 => self.stream_event(now, record, out),
                        Ok(_) | Err(_) => Err(crate::Undecodable::Malformed),
                    }
                }
                Err(problem) => Err(problem),
            },
        };
        if result.is_err() {
            out.push(Request::Fact { fact: Fact::UndecodableEvent });
            self.drop_stream(now, out);
        }
    }
    fn stream_event(
        &mut self,
        now: Time,
        record: w::StreamEvent,
        out: &mut Queue<Request>,
    ) -> Result<(), crate::Undecodable> {
        let stream = self.stream.as_mut().expect("active stream");
        match record {
            w::StreamEvent::Hello(hello) => {
                stream.hello(&hello)?;
                for served in hello.served() {
                    if served.first() > served.last() {
                        return Err(crate::Undecodable::Malformed);
                    }
                }
                self.save_stream(*hello.stream(), out)?;
                out.push(Request::Hello { family: None, versions: hello_versions(&hello, None) });
                for owner in self.limits.client.owners {
                    let family = Some(owner.family);
                    out.push(Request::Hello { family, versions: hello_versions(&hello, family) });
                }
                let mut pending = skein_lib::List::with_capacity(self.limits.client.watches);
                for (watch, opening) in &self.watches.0 {
                    let family = match &opening.owner {
                        w::Owner::Jig => None,
                        w::Owner::Family(owner) => Some(owner.number()),
                    };
                    let served = match hello_versions(&hello, family) {
                        Some(range) => range.contains(opening.version),
                        None => false,
                    };
                    if opening.subject.is_some() && served {
                        pending.push(*watch).expect("bounded watches");
                    }
                }
                for watch in &pending {
                    self.send_open(now, *watch, out);
                }
            }
            w::StreamEvent::Delivery(value) => {
                if stream.name.is_none() {
                    return Err(crate::Undecodable::Malformed);
                }
                self.watches.delivery(value, &self.limits, out)?;
            }
            w::StreamEvent::Ended(value) => {
                if stream.name.is_none() {
                    return Err(crate::Undecodable::Malformed);
                }
                if let Some(watch) = self.watches.named(value.watch()) {
                    self.cancel_open(watch, value.watch(), out);
                    if let w::WatchEnd::NotServed(value) = value.why() {
                        translate::envelope::not_served(value, out);
                    }
                    let end = translate::watches::ended(value.why());
                    self.watches.end(watch, end, out);
                    if end == jig_client_domain::contract::WatchEnd::SignedOut {
                        self.end_sign_in(out);
                        out.push(Request::Signing { signing: crate::Signing::SignedOut });
                    }
                }
            }
            w::StreamEvent::Text(value) => {
                if stream.name.is_none() {
                    return Err(crate::Undecodable::Malformed);
                }
                if let Some(watch) = self.watches.named(value.watch()) {
                    let opening = self.watches.0.get(&watch).expect("named run watch");
                    if !opening.closing {
                        if opening.owner != w::Owner::Jig || opening.version != wire::VERSION {
                            return Err(crate::Undecodable::Version);
                        }
                        let value = translate::live::run_text(value.into_parts().value, &self.limits)?;
                        self.watches.opened(watch, out);
                        out.push(Request::Watch {
                            watch,
                            event: jig_client_domain::contract::WatchEvent::Delivered(
                                jig_client_domain::contract::Delivery::Run(Box::new(value)),
                            ),
                        });
                    }
                }
            }
        }
        self.watches.alive(out);
        Ok(())
    }
    pub(super) fn end_sign_in(&mut self, out: &mut Queue<Request>) {
        self.credential = None;
        while let Some((key, _)) = (&self.attempts).into_iter().next() {
            let key = *key;
            let attempt = self.attempts.remove(&key).expect("listed attempt");
            match attempt.kind {
                ExchangeKind::Authorize => out.push(Request::Authorized(crate::Authorized::Lost)),
                ExchangeKind::SignIn
                | ExchangeKind::SignOut { .. }
                | ExchangeKind::Attempt { .. }
                | ExchangeKind::Read { .. }
                | ExchangeKind::WatchOpen { .. }
                | ExchangeKind::WatchClose { .. } => {}
            }
            // The table's attempts end with the sign-in; a carried one has no other answer.
            if attempt.carrying() {
                lost(attempt.token, out);
            }
        }
        while let Some((watch, _)) = (&self.watches.0).into_iter().next() {
            let watch = *watch;
            self.watches.end(watch, jig_client_domain::contract::WatchEnd::SignedOut, out);
        }
        if self.stream.take().is_some() {
            out.push(Request::CloseStream);
        }
    }
    fn drop_stream(&mut self, _now: Time, out: &mut Queue<Request>) {
        let Some(stream) = self.stream.take() else { return };
        let mut watches = skein_lib::List::with_capacity(self.limits.client.watches);
        for (watch, _) in &self.watches.0 {
            watches.push(*watch).expect("bounded watches");
        }
        for watch in &watches {
            let name = self.watches.0.get(watch).expect("collected watch").name;
            self.cancel_open(*watch, name, out);
            self.watches.end(*watch, jig_client_domain::contract::WatchEnd::Dropped, out);
        }
        if let Some(stream) = stream.name
            && let Some(exchange) = self.token()
        {
            self.gone(exchange, stream, out);
        }
        out.push(Request::CloseStream);
        out.push(Request::Fact { fact: Fact::StreamDropped });
    }
    fn gone(&mut self, exchange: Token, stream: [u8; 16], out: &mut Queue<Request>) {
        let record = w::Gone::new(&self.limits.wire, w::GoneParts { stream }).expect("fixed stream name");
        let mut writer = Writer::new(usize::try_from(record.measure()).expect("u32 fits usize"));
        record.encode(&mut writer).expect("measured stream gone");
        self.exchange(exchange, Message::StreamGone(writer.finish()), out);
    }
}

fn hello_versions(hello: &w::Hello, family: Option<u16>) -> Option<jig_client_domain::contract::VersionRange> {
    for served in hello.served() {
        let owner = match served.owner() {
            w::Owner::Jig => None,
            w::Owner::Family(owner) => Some(owner.number()),
        };
        if owner == family {
            return Some(jig_client_domain::contract::VersionRange { first: served.first(), last: served.last() });
        }
    }
    None
}
