//! Durability before exchanges, and owner-versioned restoration (protocol/client.md, 10).
use crate::{Fact, Limits, Link, Message, Record, Request, Saved, translate};
use alloc::boxed::Box;
use jig_client_protocol_wire::v19 as w;
use skein_lib::{List, Queue, Reader, RequestKey, Token, Writer, bytes};
#[derive(Debug)]
struct Held {
    exchange: Token,
    message: Message,
    credential: Option<skein_lib::Secret>,
}
#[derive(Debug)]
pub(super) struct Store {
    waiting: bool,
    failed: bool,
    held: Queue<Held>,
}
impl Store {
    pub(super) fn new(_limits: &Limits) -> Self {
        Self { waiting: false, failed: false, held: Queue::uncapped() }
    }
}
/// Whether these limits are representable: every edge bound they size
/// is finite and every capacity they derive fits (programming-model.md, 6.3).
#[must_use]
pub(crate) fn valid(limits: &Limits) -> bool {
    w::Keyed::worst_case_bytes(&limits.wire).is_some()
}

pub(super) fn key(record: &Saved) -> Box<[u8]> {
    bytes::copy_of(&record.record.key)
}
fn encoded(value: &Saved, limits: &Limits) -> Result<Box<[u8]>, w::Problem> {
    let table = translate::store::table(&value.record, limits)?;
    let mut writer = Writer::new(usize::try_from(table.measure()).expect("record fits usize"));
    table.encode(&mut writer).expect("measured table record");
    let record = w::Kept::new(&limits.wire, w::KeptParts { store_version: value.version, record: writer.finish() })?;
    if u64::from(record.measure()) > limits.client.document_bytes {
        return Err(translate::too_large());
    }
    let mut writer = Writer::new(usize::try_from(record.measure()).expect("record fits usize"));
    record.encode(&mut writer).expect("measured kept record");
    Ok(writer.finish())
}
fn restore(value: w::Kept, storage_key: &[u8], limits: &Limits) -> Result<Saved, w::Problem> {
    let mut reader = Reader::new(value.record());
    let record = translate::store::restore_table(w::TableEntry::decode(&limits.wire, &mut reader)?, limits)?;
    if reader.remaining() != 0 || record.key.as_slice() != storage_key {
        return Err(translate::piece_problem(skein_codec::Reason::Malformed));
    }
    Ok(Saved { version: value.store_version(), record })
}
impl Link {
    /// Keeps keyed exchanges behind writes the composing client has already given.
    pub fn wait_for_store(&mut self) {
        self.store.waiting = true;
    }
    pub(super) fn exchange(&mut self, exchange: Token, message: Message, out: &mut Queue<Request>) {
        let tracked = match &message {
            Message::Request(_) | Message::Read(_) | Message::Watch(_) => true,
            Message::Unwatch(_)
            | Message::StreamGone(_)
            | Message::SignInLink(_)
            | Message::SignInService
            | Message::SignOut
            | Message::Authorize(_) => false,
        };
        if !self.attempts.contains_key(&exchange) && tracked {
            return;
        }
        // A request carrying a value is never kept, so never waits on a write.
        let keyed = match &message {
            Message::Request(_) => match self.attempts.get(&exchange) {
                Some(attempt) => !attempt.carrying(),
                None => true,
            },
            Message::Read(_)
            | Message::Watch(_)
            | Message::Unwatch(_)
            | Message::StreamGone(_)
            | Message::SignInLink(_)
            | Message::SignInService
            | Message::SignOut
            | Message::Authorize(_) => false,
        };
        if !keyed {
            out.push(Request::Exchange { exchange, message, credential: self.credential.clone() });
            return;
        }
        if self.store.failed {
            if let Some(attempt) = self.attempts.remove(&exchange) {
                out.push(Request::NotKept { attempt: attempt.token });
            }
        } else if self.store.waiting {
            self.store.held.push(Held { exchange, message, credential: self.credential.clone() });
        } else {
            out.push(Request::Exchange { exchange, message, credential: self.credential.clone() });
        }
    }
    fn gather(&mut self, out: &mut Queue<Request>) {
        self.store.waiting = true;
        let count = out.len();
        for _ in 0..count {
            match out.pop().expect("counted step outputs") {
                Request::Exchange { exchange, message, .. } => self.exchange(exchange, message, out),
                value @ (Request::Save { .. }
                | Request::Erase { .. }
                | Request::Kept { .. }
                | Request::Dropped
                | Request::Answer { .. }
                | Request::NotKept { .. }
                | Request::OpenStream { .. }
                | Request::CloseStream
                | Request::Watch { .. }
                | Request::FamilyDelivery { .. }
                | Request::Ended { .. }
                | Request::Hello { .. }
                | Request::Read { .. }
                | Request::Authorized(_)
                | Request::Signing { .. }
                | Request::ServiceCredential { .. }
                | Request::Fact { .. }) => out.push(value),
            }
        }
    }
    pub(super) fn save(&mut self, record: Saved, out: &mut Queue<Request>) {
        self.gather(out);
        let key = key(&record);
        match encoded(&record, &self.limits) {
            Ok(bytes) => out.push(Request::Save { record: Record { key, bytes } }),
            Err(_) => {
                self.write_failed(out);
                self.store.failed = true;
                out.push(Request::Fact { fact: Fact::UndecodableStore });
            }
        }
    }
    pub(super) fn erase(&mut self, key: RequestKey, out: &mut Queue<Request>) {
        self.gather(out);
        out.push(Request::Erase { key: bytes::copy_of(&key) });
    }
    pub(super) fn written(&mut self, out: &mut Queue<Request>) {
        self.store.waiting = false;
        self.store.failed = false;
        // Durability releases held exchanges in FIFO order, skipping stale attempts.
        while let Some(value) = self.store.held.pop() {
            if self.attempts.contains_key(&value.exchange) {
                out.push(Request::Exchange {
                    exchange: value.exchange,
                    message: value.message,
                    credential: value.credential,
                });
            }
        }
    }
    pub(super) fn write_failed(&mut self, out: &mut Queue<Request>) {
        self.store.waiting = false;
        self.store.failed = false;
        while let Some(value) = self.store.held.pop() {
            if let Some(attempt) = self.attempts.remove(&value.exchange) {
                out.push(Request::NotKept { attempt: attempt.token });
            }
        }
    }
    pub(super) fn save_stream(&mut self, stream: [u8; 16], out: &mut Queue<Request>) -> Result<(), crate::Undecodable> {
        let Ok(value) = w::Kept::new(
            &self.limits.wire,
            w::KeptParts { store_version: jig_client_domain::TABLE_VERSION, record: Box::from(stream) },
        ) else {
            return Err(crate::Undecodable::TooLarge);
        };
        let mut writer = Writer::new(usize::try_from(value.measure()).expect("fixed stream record"));
        value.encode(&mut writer).expect("measured stream record");
        out.push(Request::Save { record: Record { key: bytes::copy_of(crate::STREAM_KEY), bytes: writer.finish() } });
        Ok(())
    }
    pub(super) fn start_store(&mut self, kept: List<Record>, out: &mut Queue<Request>) {
        let mut bound = self.limits.client.requests.requests;
        for record in &kept {
            if record.key.as_ref() == crate::STREAM_KEY {
                bound = bound.saturating_add(1);
                break;
            }
        }
        if kept.len() > bound {
            out.push(Request::Fact { fact: Fact::UndecodableStore });
            return;
        }
        for entry in kept.into_boxed() {
            let mut reader = Reader::new(&entry.bytes);
            let result = if (entry.key.len() != 16 && entry.key.as_ref() != crate::STREAM_KEY)
                || u64::try_from(entry.bytes.len()).expect("bounded bytes fit u64") > self.limits.client.document_bytes
            {
                Err(translate::too_large())
            } else {
                w::Kept::decode(&self.limits.wire, &mut reader)
            };
            let Ok(value) = result else {
                out.push(Request::Dropped);
                out.push(Request::Fact { fact: Fact::UndecodableStore });
                continue;
            };
            if reader.remaining() != 0 || value.store_version() != jig_client_domain::TABLE_VERSION {
                out.push(Request::Dropped);
                continue;
            }
            if entry.key.as_ref() == crate::STREAM_KEY {
                match <[u8; 16]>::try_from(value.record()) {
                    Ok(stream) => self.predecessor = Some(stream),
                    Err(_) => {
                        out.push(Request::Dropped);
                        out.push(Request::Fact { fact: Fact::UndecodableStore });
                    }
                }
                continue;
            }
            match restore(value, &entry.key, &self.limits) {
                Ok(record) => out.push(Request::Kept { record }),
                Err(_) => {
                    out.push(Request::Dropped);
                    out.push(Request::Fact { fact: Fact::UndecodableStore });
                }
            }
        }
    }
}
