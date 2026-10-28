//! Private token and whole small-store files (protocol/client.md, 8 and 10).
//! Each batch is answered in order (client/domain/client.md, 16).
use skein_io::{
    file::{self, Expect},
    file_layer::{self, FileIo},
    kernel::{Complete, Error, Fd, Op, Submit},
};
use skein_lib::{Duration, Queue, Reader, Secret, Time, Token, Writer};
use std::collections::BTreeMap;
const TAG: u64 = 1 << 63;
const OWNER: Token = Token::new(1);
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Unopened,
    Opening,
    Token,
    Small,
    Ready,
    Writing,
    Credential,
    ClosingPrivate,
    ClosingRoot,
    Closed,
}
pub(crate) struct Files {
    io: FileIo,
    root: Option<Token>,
    private: Option<Token>,
    directory: Box<[u8]>,
    stage: Stage,
    records: BTreeMap<Box<[u8]>, Box<[u8]>>,
    max: u32,
    batches: u32,
    writing: u32,
    refused: bool,
    failure: Option<()>,
    closing: bool,
    token: Option<Secret>,
    credential: Option<Box<[u8]>>,
    loaded: Option<()>,
    pub(crate) events: Queue<file::Event>,
    pub(crate) submissions: Queue<Submit>,
    pub(crate) written: Option<(bool, u32)>,
    pub(crate) credential_written: Option<bool>,
}
impl Files {
    pub(crate) fn new(directory: Box<[u8]>, max: u32) -> Self {
        Self {
            io: FileIo::new(4, max, 1, Duration::from_secs(30)),
            root: None,
            private: None,
            directory,
            stage: Stage::Unopened,
            records: BTreeMap::new(),
            max,
            batches: 0,
            writing: 0,
            refused: false,
            failure: None,
            closing: false,
            token: None,
            credential: None,
            loaded: None,
            events: Queue::uncapped(),
            submissions: Queue::with_capacity(16),
            written: None,
            credential_written: None,
        }
    }
    pub(crate) fn adopt(&mut self, fd: Fd, user: u32, seed: u64) {
        assert!(self.stage == Stage::Unopened, "adopt once");
        self.io = FileIo::new(4, self.max, 1, Duration::from_secs(30)).with_effective_user(user);
        self.io.seed_randomness(seed);
        self.root = Some(self.io.adopt_root(fd).expect("root room"));
    }
    pub(crate) fn ready(&self) -> bool {
        self.stage == Stage::Ready && self.failure.is_none() && self.batches == 0 && self.credential.is_none()
    }
    pub(crate) fn token(&mut self) -> Option<Secret> {
        self.token.take()
    }
    pub(crate) fn loaded(&mut self) -> Option<Vec<jig_client::Record>> {
        self.loaded.take()?;
        Some(
            self.records
                .iter()
                .map(|(key, record)| jig_client::Record { key: key.clone(), bytes: record.clone() })
                .collect(),
        )
    }
    pub(crate) fn batch(&mut self) {
        self.batches = self.batches.checked_add(1).expect("bounded storage batches");
    }
    pub(crate) fn edit(&mut self, key: Box<[u8]>, record: Option<Box<[u8]>>) {
        match record {
            Some(record) => {
                let bytes: usize = self
                    .records
                    .iter()
                    .filter(|(old, _)| **old != key)
                    .map(|(key, value)| key.len() + value.len() + 8)
                    .sum();
                if bytes + key.len() + record.len() + 12 > usize::try_from(self.max).expect("file bound") {
                    self.refused = true;
                    return;
                }
                self.records.insert(key, record);
            }
            None => {
                self.records.remove(&key);
            }
        }
    }
    pub(crate) fn credential(&mut self, bytes: Box<[u8]>) {
        self.credential = Some(bytes);
    }
    pub(crate) fn close(&mut self) {
        self.closing = true;
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.stage == Stage::Closed && self.io.open_files() == 0 && self.io.takes() && self.submissions.is_empty()
    }
    pub(crate) fn pending(&self) -> bool {
        (self.stage == Stage::Unopened && self.root.is_some())
            || !self.events.is_empty()
            || (self.stage == Stage::Ready && (self.batches != 0 || self.credential.is_some() || self.closing))
    }
    pub(crate) fn deadline(&self) -> Option<Time> {
        self.io.next_deadline()
    }
    pub(crate) fn complete(&mut self, mut complete: Complete) {
        complete.op = Token::new(complete.op.raw() & !TAG);
        if let Op::Cancel { target } = &mut complete.kind {
            *target = Token::new(target.raw() & !TAG);
        }
        file_layer::up(&mut self.io, complete, &mut self.events, &mut self.submissions);
    }
    pub(crate) fn owns(op: Token) -> bool {
        op.raw() & TAG != 0
    }
    pub(crate) fn publish(&mut self, out: &mut Queue<Submit>) {
        while out.room() > 0 {
            let Some(mut submit) = self.submissions.pop() else { break };
            assert!(submit.op.raw() & TAG == 0, "finite file namespace");
            submit.op = Token::new(submit.op.raw() | TAG);
            if let Op::Cancel { target } = &mut submit.kind {
                *target = Token::new(target.raw() | TAG);
            }
            out.push(submit);
        }
    }
    fn request(&mut self, now: Time, stage: Stage, request: file::Request) {
        self.stage = stage;
        file_layer::down(&mut self.io, now, request, &mut self.events, &mut self.submissions);
    }
    pub(crate) fn progress(&mut self, now: Time) {
        if self.io.is_due(now) {
            file_layer::expire(&mut self.io, now, &mut self.submissions);
        }
        while let Some(event) = self.events.pop() {
            self.answer(event);
        }
        // Deliver the preceding write before a queued refusal can answer.
        if !self.io.takes() || self.written.is_some() {
            return;
        }
        let root = self.root.unwrap_or(OWNER);
        match self.stage {
            Stage::Unopened if self.root.is_some() => self.request(
                now,
                Stage::Opening,
                file::Request::OpenPrivate { owner: OWNER, root, path: self.directory.clone() },
            ),
            Stage::Token | Stage::Small => {
                let (path, max) = if self.stage == Stage::Token {
                    (b"token".as_slice(), 40)
                } else {
                    (b"store".as_slice(), self.max)
                };
                self.request(
                    now,
                    self.stage,
                    file::Request::Load {
                        owner: OWNER,
                        root: self.private.expect("private root"),
                        path: path.into(),
                        max,
                        no_follow: true,
                    },
                );
            }
            Stage::Ready if self.credential.is_some() => {
                let bytes = self.credential.take().expect("credential");
                self.request(now, Stage::Credential, self.store(b"token", bytes));
            }
            Stage::Ready if self.batches != 0 => {
                let batches = core::mem::take(&mut self.batches);
                if core::mem::take(&mut self.refused) {
                    self.written = Some((false, batches));
                    return;
                }
                if let Some(bytes) = self.encode() {
                    self.writing = batches;
                    self.request(now, Stage::Writing, self.store(b"store", bytes));
                } else {
                    self.failure = Some(());
                    self.written = Some((false, batches));
                }
            }
            Stage::Ready if self.closing => {
                if let Some(file) = self.private {
                    self.request(now, Stage::ClosingPrivate, file::Request::Close { owner: OWNER, file });
                } else {
                    self.request(now, Stage::ClosingRoot, file::Request::Close { owner: OWNER, file: root });
                }
            }
            Stage::Unopened
            | Stage::Opening
            | Stage::Ready
            | Stage::Writing
            | Stage::Credential
            | Stage::ClosingPrivate
            | Stage::ClosingRoot
            | Stage::Closed => {}
        }
    }
    fn store(&self, path: &[u8], bytes: Box<[u8]>) -> file::Request {
        file::Request::Store {
            owner: OWNER,
            root: self.private.expect("private root"),
            path: path.into(),
            bytes,
            expected: Expect::Any,
            no_follow: true,
        }
    }
    fn answer(&mut self, event: file::Event) {
        let absent = matches!(event, file::Event::Failed { error: Error::NotFound, .. });
        match self.stage {
            Stage::Opening => {
                if let file::Event::Opened { file, .. } = event {
                    self.private = Some(file);
                    self.stage = Stage::Token;
                } else {
                    self.failure = Some(());
                    self.stage = Stage::Ready;
                    self.closing = true;
                }
            }
            Stage::Token => {
                if let file::Event::Loaded { bytes, .. } = event {
                    if bytes.len() == 40 {
                        self.token = Some(Secret::new(bytes));
                    } else if !bytes.is_empty() {
                        self.failure = Some(());
                    }
                } else if !absent {
                    self.failure = Some(());
                }
                self.stage = Stage::Small;
            }
            Stage::Small => {
                if let file::Event::Loaded { bytes, .. } = event {
                    if !self.decode(&bytes) {
                        self.failure = Some(());
                    }
                } else if !absent {
                    self.failure = Some(());
                }
                self.stage = Stage::Ready;
                self.loaded = self.failure.is_none().then_some(());
            }
            Stage::Writing | Stage::Credential => {
                let ok = matches!(event, file::Event::Stored { .. });
                if self.stage == Stage::Writing {
                    self.written = Some((ok, core::mem::take(&mut self.writing)));
                } else {
                    self.credential_written = Some(ok);
                }
                self.stage = Stage::Ready;
            }
            Stage::ClosingPrivate => {
                assert!(matches!(event, file::Event::Closed { .. }), "root close");
                self.private = None;
                self.stage = Stage::Ready;
            }
            Stage::ClosingRoot => {
                assert!(matches!(event, file::Event::Closed { .. }), "root close");
                self.root = None;
                self.stage = Stage::Closed;
            }
            Stage::Unopened | Stage::Ready | Stage::Closed => panic!("unexpected file terminal"),
        }
    }
    fn encode(&self) -> Option<Box<[u8]>> {
        let length = self.records.iter().try_fold(4_usize, |size, (key, value)| {
            size.checked_add(8)?.checked_add(key.len())?.checked_add(value.len())
        })?;
        if length > usize::try_from(self.max).ok()? {
            return None;
        }
        let mut writer = Writer::new(length);
        writer.put(&u32::try_from(self.records.len()).ok()?.to_be_bytes()).ok()?;
        for (key, value) in &self.records {
            writer.put(&u32::try_from(key.len()).ok()?.to_be_bytes()).ok()?;
            writer.put(key).ok()?;
            writer.put(&u32::try_from(value.len()).ok()?.to_be_bytes()).ok()?;
            writer.put(value).ok()?;
        }
        Some(writer.finish())
    }
    fn decode(&mut self, bytes: &[u8]) -> bool {
        let mut reader = Reader::new(bytes);
        let Some(count) = reader.u32() else { return false };
        for _ in 0..count {
            let Some(len) = reader.u32() else { return false };
            let Some(key) = reader.bytes(len) else { return false };
            let Some(len) = reader.u32() else { return false };
            let Some(value) = reader.bytes(len) else { return false };
            if self.records.insert(key.into(), value.into()).is_some() {
                return false;
            }
        }
        reader.remaining() == 0
    }
}
