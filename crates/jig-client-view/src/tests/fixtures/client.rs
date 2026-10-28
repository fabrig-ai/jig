//! A reference UI with the real unlinked client and a scripted transport.
use crate::{Event, Limits, Request, Ui};
use jig_client::{Client, Down, Handle, Key, Readout, Up, Watchable, contract};
use skein_lib::{Duration, List, Queue, Time, Token, Wall};
pub(crate) const MOST: Duration = Duration::from_nanos(80);
const OBSERVATIONS: u32 = 4096;
const OUTPUTS: u32 = 512;
const EXCHANGES: u32 = 256;
const HEARTBEAT: Duration = Duration::from_secs(1000);
const SEED: u64 = 9;
#[derive(Debug)]
pub struct Fixture {
    pub ui: Ui,
    pub client: Client,
    pub down: List<Down>,
    pub now: Time,
    pub attempts: List<(Key, Token)>,
    pub watches: List<(contract::Subject, Token)>,
    pub transport_reads: List<(Handle, Token)>,
    pub read_origins: List<(Handle, crate::handles::Reading)>,
}
impl core::ops::Deref for Fixture {
    type Target = Ui;
    fn deref(&self) -> &Ui {
        &self.ui
    }
}
impl core::ops::DerefMut for Fixture {
    fn deref_mut(&mut self) -> &mut Ui {
        &mut self.ui
    }
}
impl Fixture {
    pub fn new(limits: &Limits) -> Self {
        Self::with_client_handles(limits, limits.watches.checked_add(limits.reads).unwrap().checked_add(1).unwrap())
    }
    pub fn with_client_handles(limits: &Limits, handles: u32) -> Self {
        Self::build(limits, handles, None)
    }
    pub fn restored(
        limits: &Limits,
        records: List<crate::store::Record>,
        kept: List<skein_lib::RequestRecord<jig_client::Held>>,
    ) -> Self {
        Self::build(
            limits,
            limits.watches.checked_add(limits.reads).unwrap().checked_add(1).unwrap(),
            Some((records, kept)),
        )
    }
    fn build(
        limits: &Limits,
        handles: u32,
        restored: Option<(List<crate::store::Record>, List<skein_lib::RequestRecord<jig_client::Held>>)>,
    ) -> Self {
        let domain = jig_client::DomainLimits {
            owners: &[],
            requests: skein_lib::RequestLimits {
                requests: limits.intents.max(1),
                bytes: 1 << 20,
                out: EXCHANGES,
                first: Duration::from_nanos(10),
                most: MOST,
                margin: Duration::from_secs(1),
            },
            ask_bytes: 65536,
            handles,
            watches: limits.watches.checked_add(1).unwrap(),
            reads: limits.reads,
            turn_bytes: limits.text,
            document_bytes: limits.document_bytes,
            backoff: jig_client::Backoff { first: Duration::from_nanos(10), most: MOST },
            rows: limits.rows,
            segments: limits.segments,
            text: limits.text,
            turn_text: limits.text,
            projects: limits.projects,
            fields: limits.fields,
            delegates: limits.delegates,
            records: limits.records,
            kept: limits.kept,
            parts: limits.parts,
            authority: 8,
            accounts: limits.accounts,
            heartbeat: HEARTBEAT,
        };
        let mut value = Self {
            ui: Ui::new(
                limits,
                crate::page::navigation::Navigation::new(limits.navigation_entries),
                crate::address::Address::Inbox { project: None },
                crate::page::Arrangement::none(),
            )
            .unwrap(),
            client: Client::unlinked(&jig_client::Limits { domain, exchanges: EXCHANGES }, SEED).unwrap(),
            now: Time::ZERO,
            down: List::with_capacity(OBSERVATIONS),
            attempts: List::with_capacity(OBSERVATIONS),
            watches: List::with_capacity(OBSERVATIONS),
            transport_reads: List::with_capacity(OBSERVATIONS),
            read_origins: List::with_capacity(OBSERVATIONS),
        };
        let mut out = Queue::with_capacity(OUTPUTS);
        let kept = if let Some((records, kept)) = restored {
            value.ui.step(
                Time::ZERO,
                Wall::EPOCH,
                &value.client,
                Event::Start {
                    records,
                    place: crate::address::Place::Jig(crate::address::Address::Inbox { project: None }),
                    seed: 8,
                },
                &mut out,
            );
            kept
        } else {
            List::with_capacity(0)
        };
        value.native(jig_client::Event::StartUnlinked { kept }, &mut out);
        value.native(jig_client::Event::Up { up: Up::SignedIn }, &mut out);
        value.native(jig_client::Event::Up { up: Up::Served { served: true, versions: None } }, &mut out);
        value.output(out);
        value
    }
    pub fn view(&self) -> crate::Borrowed<'_> {
        self.ui.borrow(&self.client)
    }
    pub fn frame(&self) -> crate::frame::me::FrameState<'_> {
        self.ui.frame(&self.client)
    }
    pub fn frame_summary(&self) -> crate::frame::me::Summary {
        self.ui.frame_summary(&self.client)
    }
    pub fn objects(&self) -> crate::ui::borrowed::ObjectsBorrowed<'_> {
        self.view().objects()
    }
    pub fn project_page(&self) -> crate::ui::borrowed::project::Project<'_> {
        self.view().project_page()
    }
    pub fn task_page(&self) -> crate::ui::borrowed::task::Task<'_> {
        self.view().task_page()
    }
    pub fn conversation_rows(&self) -> crate::windows::ConversationWindow {
        self.view().conversation_rows()
    }
    pub fn first_turn(&self) -> Option<contract::ProvisionalTurnName> {
        let projected = self.conversation();
        let turn = projected.live.get(0)?;
        Some(turn.name)
    }
    pub fn conversation(&self) -> crate::ui::borrowed::conversation::Activity<'_> {
        self.view().conversation()
    }
    pub fn inbox(&self) -> crate::page::inbox::Inbox {
        self.view().inbox()
    }
    pub fn deployment_page(&self) -> crate::ui::borrowed::Deployment<'_> {
        self.view().deployment_page()
    }
    pub fn shows(&self, section: crate::page::Section) -> bool {
        self.ui.shows(section, &self.client)
    }
    pub fn section_status(&self, section: crate::page::Section) -> Option<crate::page::SectionStatus> {
        self.view().section_status(section)
    }
    pub fn reopen(&mut self, watch: Handle) {
        self.drive(self.now, Ended { watch, end: contract::WatchEnd::Dropped });
        self.now = self.now.saturating_add(Duration::from_secs(1));
        let mut native = Queue::with_capacity(OUTPUTS);
        self.client.fire(self.now, Wall::EPOCH, &mut native);
        let mut ui = Queue::with_capacity(OUTPUTS);
        self.native_out(native, None, None, &mut ui);
        self.output(ui);
    }
    pub fn owes(&self, id: skein_lib::Id<crate::objects::Object>) -> bool {
        self.ui.owes(id, &self.client)
    }
    pub fn intent(&self, key: Key) -> Token {
        Token::new(key.number ^ self.ui.seed)
    }
    pub fn request_watch(&mut self, subject: contract::Subject) -> Option<Handle> {
        let mut out = Queue::with_capacity(OUTPUTS);
        let handle = self.ui.handles.watch(subject, crate::handles::Owner::Page, &mut out)?;
        self.output(out);
        Some(handle)
    }
    pub fn read_again(&mut self, old: Handle) -> Handle {
        let mut original = None;
        for (handle, reading) in self.read_origins.as_slice().iter().rev() {
            if *handle == old {
                original = Some(reading.clone());
                break;
            }
        }
        let original = original.expect("the fixture remembers its original read request");
        let handle = self.request_read(original.source, original.what.clone()).expect("a fresh read fits");
        if let contract::Read::Inbox { group, .. } = original.what {
            self.ui.inbox.window_mut(group).reading = Some(Token::new(handle.number));
        }
        handle
    }
    pub fn request_read(&mut self, source: crate::objects::Source, what: contract::Read) -> Option<Handle> {
        let mut out = Queue::with_capacity(OUTPUTS);
        let handle = self.ui.handles.read(source, what, &mut out)?;
        self.output(out);
        Some(handle)
    }
    pub fn drive(&mut self, now: Time, input: impl Input) -> Box<[Request]> {
        self.now = now;
        self.ui.now = now;
        self.ui.wall = Wall::EPOCH;
        self.down.clear();
        let mut out = Queue::with_capacity(OUTPUTS);
        input.enter(self, &mut out);
        self.output(out)
    }
    fn output(&mut self, mut out: Queue<Request>) -> Box<[Request]> {
        let mut observed = List::with_capacity(OBSERVATIONS);
        while let Some(request) = out.pop() {
            match request {
                Request::Client { asked } => {
                    if let jig_client::Event::Read { handle, .. } = &asked
                        && let Some(reading) = self.ui.handles.reads.get(handle)
                    {
                        self.read_origins.push((*handle, reading.clone())).unwrap();
                    }
                    observed.push(Request::Client { asked: copy_client_event(&asked) }).unwrap();
                    self.native(asked, &mut out);
                }
                request @ (Request::Show { .. }
                | Request::Save { .. }
                | Request::Erase { .. }
                | Request::ShowPart { .. }
                | Request::PartsLeft) => observed.push(request).unwrap(),
            }
        }
        self.client.reclaim();
        observed.into_boxed()
    }
    fn native(&mut self, event: jig_client::Event, ui_out: &mut Queue<Request>) {
        let sending = match &event {
            jig_client::Event::Send { key, .. } => Some(*key),
            jig_client::Event::Carry { .. }
            | jig_client::Event::Start { .. }
            | jig_client::Event::StartUnlinked { .. }
            | jig_client::Event::SignIn { .. }
            | jig_client::Event::Authorize { .. }
            | jig_client::Event::SignOut
            | jig_client::Event::SignOutLost
            | jig_client::Event::Watch { .. }
            | jig_client::Event::Unwatch { .. }
            | jig_client::Event::Read { .. }
            | jig_client::Event::Reply { .. }
            | jig_client::Event::Lost { .. }
            | jig_client::Event::Full { .. }
            | jig_client::Event::Streamed { .. }
            | jig_client::Event::StreamGone
            | jig_client::Event::Stored
            | jig_client::Event::NotStored
            | jig_client::Event::Up { .. } => None,
        };
        let reading = match &event {
            jig_client::Event::Read { handle, .. } => Some(*handle),
            jig_client::Event::Start { .. }
            | jig_client::Event::StartUnlinked { .. }
            | jig_client::Event::SignIn { .. }
            | jig_client::Event::Authorize { .. }
            | jig_client::Event::SignOut
            | jig_client::Event::SignOutLost
            | jig_client::Event::Watch { .. }
            | jig_client::Event::Unwatch { .. }
            | jig_client::Event::Send { .. }
            | jig_client::Event::Carry { .. }
            | jig_client::Event::Reply { .. }
            | jig_client::Event::Lost { .. }
            | jig_client::Event::Full { .. }
            | jig_client::Event::Streamed { .. }
            | jig_client::Event::StreamGone
            | jig_client::Event::Stored
            | jig_client::Event::NotStored
            | jig_client::Event::Up { .. } => None,
        };
        let mut out = Queue::with_capacity(OUTPUTS);
        self.client.step(self.now, Wall::EPOCH, event, &mut out);
        self.native_out(out, reading, sending, ui_out);
    }
    fn native_out(
        &mut self,
        mut out: Queue<jig_client::Request>,
        reading: Option<Handle>,
        sending: Option<Key>,
        ui_out: &mut Queue<Request>,
    ) {
        while let Some(request) = out.pop() {
            if let jig_client::Request::Down { down } = &request {
                self.down.push(down.clone()).unwrap();
            }
            match request {
                jig_client::Request::Told { told } => {
                    self.ui.step(self.now, Wall::EPOCH, &self.client, Event::Client { told }, ui_out);
                }
                jig_client::Request::Down { down: Down::Open { watch, subject: Watchable::Jig(subject) } } => {
                    self.watches.push((subject, watch)).unwrap();
                }
                jig_client::Request::Down {
                    down: Down::Continue { watch, subject: contract::Subject::Run { task, .. } },
                } => {
                    self.watches.push((contract::Subject::Run { task, turn: None, offset: 0 }, watch)).unwrap();
                }
                jig_client::Request::Down { down: Down::Send { attempt, .. } } => {
                    if let Some(key) = sending {
                        self.attempts.push((key, attempt)).unwrap();
                    }
                }
                jig_client::Request::Down { down: Down::Read { read, .. } } => {
                    if let Some(handle) = reading {
                        self.transport_reads.push((handle, read)).unwrap();
                    }
                }
                jig_client::Request::Down { down: Down::Close { watch } } => {
                    self.native(
                        jig_client::Event::Up { up: Up::WatchEnded { watch, end: contract::WatchEnd::Closed } },
                        ui_out,
                    );
                }
                jig_client::Request::Exchange { .. }
                | jig_client::Request::OpenStream { .. }
                | jig_client::Request::Authorized(_)
                | jig_client::Request::ServiceCredential { .. }
                | jig_client::Request::CloseStream
                | jig_client::Request::Save { .. }
                | jig_client::Request::Erase { .. }
                | jig_client::Request::SignOut
                | jig_client::Request::Down {
                    down:
                        Down::Save { .. }
                        | Down::Erase { .. }
                        | Down::Open { subject: Watchable::Family { .. }, .. }
                        | Down::Continue { .. },
                } => {}
            }
        }
    }
    pub(crate) fn watch_token(&self, handle: Handle) -> Option<Token> {
        let subject = &self.ui.handles.watches.get(&handle)?.subject;
        for (held, token) in self.watches.as_slice().iter().rev() {
            if held == subject {
                return Some(*token);
            }
        }
        None
    }
}
pub trait Input {
    fn enter(self, fixture: &mut Fixture, out: &mut Queue<Request>);
}
impl Input for Event {
    fn enter(self, fixture: &mut Fixture, out: &mut Queue<Request>) {
        fixture.ui.step(fixture.now, Wall::EPOCH, &fixture.client, self, out);
    }
}
impl Input for jig_client::Event {
    fn enter(self, fixture: &mut Fixture, out: &mut Queue<Request>) {
        fixture.native(self, out);
    }
}
impl Input for Up {
    fn enter(self, fixture: &mut Fixture, out: &mut Queue<Request>) {
        fixture.native(jig_client::Event::Up { up: self }, out);
    }
}
#[derive(Clone)]
pub struct Watched {
    pub watch: Handle,
    pub event: contract::WatchEvent,
}
impl Input for Watched {
    fn enter(self, fixture: &mut Fixture, out: &mut Queue<Request>) {
        if let Some(watch) = fixture.watch_token(self.watch) {
            fixture.native(jig_client::Event::Up { up: Up::Watched { watch, event: self.event } }, out);
        }
    }
}
#[derive(Clone)]
pub struct ReadPage {
    pub read: Handle,
    pub result: contract::ReadResult,
}
impl ReadPage {
    pub fn late(self) -> Event {
        Event::Client { told: jig_client::Told::Page { handle: self.read, page: Readout::Jig(self.result) } }
    }
}
impl Input for ReadPage {
    fn enter(self, fixture: &mut Fixture, out: &mut Queue<Request>) {
        let mut found = None;
        for (handle, read) in fixture.transport_reads.as_slice().iter().rev() {
            if *handle == self.read {
                found = Some(*read);
                break;
            }
        }
        if let Some(read) = found {
            fixture.native(jig_client::Event::Up { up: Up::ReadDone { read, result: Readout::Jig(self.result) } }, out);
        }
    }
}
pub struct Ended {
    pub watch: Handle,
    pub end: contract::WatchEnd,
}
impl Input for Ended {
    fn enter(self, fixture: &mut Fixture, out: &mut Queue<Request>) {
        if let Some(watch) = fixture.watch_token(self.watch) {
            fixture.native(jig_client::Event::Up { up: Up::WatchEnded { watch, end: self.end } }, out);
        }
    }
}
pub struct Served {
    pub served: bool,
}
impl Input for Served {
    fn enter(self, fixture: &mut Fixture, out: &mut Queue<Request>) {
        Up::Served { served: self.served, versions: None }.enter(fixture, out);
    }
}
pub struct Reported<K = Key> {
    pub request: K,
    pub progress: crate::intents::Progress,
}
impl<K: IntentKey> Input for Reported<K> {
    fn enter(self, fixture: &mut Fixture, out: &mut Queue<Request>) {
        let token = self.request.intent(&fixture.ui);
        crate::intents::progress(&mut fixture.ui, token, self.progress, out, &fixture.client);
    }
}
pub struct AnsweredByEngine<K = Key> {
    pub request: K,
    pub answer: contract::Answer,
}
impl<K: IntentKey> Input for AnsweredByEngine<K> {
    fn enter(self, fixture: &mut Fixture, out: &mut Queue<Request>) {
        let token = self.request.intent(&fixture.ui);
        let key = Key { asker: jig_client::Asker::JIG, number: token.raw() ^ fixture.ui.seed };
        let mut found = None;
        for (held, attempt) in fixture.attempts.as_slice().iter().rev() {
            if *held == key {
                found = Some(*attempt);
                break;
            }
        }
        if let Some(attempt) = found {
            fixture.native(
                jig_client::Event::Up {
                    up: Up::Answer {
                        attempt,
                        envelope: skein_lib::RequestEnvelope::Final,
                        answer: Some(jig_client::Answered::Jig(self.answer)),
                        why: None,
                    },
                },
                out,
            );
            return;
        }
        let env = skein_lib::Env { now: fixture.now, wall: Wall::EPOCH, limits: fixture.ui.limits };
        crate::intents::answered(&mut fixture.ui, &env, token, self.answer, out, &fixture.client);
    }
}
fn copy_client_event(event: &jig_client::Event) -> jig_client::Event {
    match event {
        jig_client::Event::Watch { handle, subject } => {
            jig_client::Event::Watch { handle: *handle, subject: subject.clone() }
        }
        jig_client::Event::Unwatch { handle } => jig_client::Event::Unwatch { handle: *handle },
        jig_client::Event::Read { handle, read } => jig_client::Event::Read { handle: *handle, read: read.clone() },
        jig_client::Event::Send { key, name, request } => {
            jig_client::Event::Send { key: *key, name: *name, request: request.clone() }
        }
        jig_client::Event::SignOut => jig_client::Event::SignOut,
        jig_client::Event::Carry { .. }
        | jig_client::Event::Start { .. }
        | jig_client::Event::StartUnlinked { .. }
        | jig_client::Event::SignIn { .. }
        | jig_client::Event::Authorize { .. }
        | jig_client::Event::SignOutLost
        | jig_client::Event::Reply { .. }
        | jig_client::Event::Lost { .. }
        | jig_client::Event::Full { .. }
        | jig_client::Event::Streamed { .. }
        | jig_client::Event::StreamGone
        | jig_client::Event::Stored
        | jig_client::Event::NotStored
        | jig_client::Event::Up { .. } => unreachable!("the UI emits only presentation requests"),
    }
}

pub trait IntentKey {
    fn intent(self, ui: &Ui) -> Token;
}
impl IntentKey for Token {
    fn intent(self, _ui: &Ui) -> Token {
        self
    }
}
impl IntentKey for Key {
    fn intent(self, ui: &Ui) -> Token {
        Token::new(self.number ^ ui.seed)
    }
}

/// Supply an owner's intent without routing a sibling through the Ui.
#[derive(Clone)]
pub struct SendIntent {
    pub scope: u64,
    pub origin: crate::intents::Origin,
    pub ask: Box<contract::Ask>,
}
impl Input for SendIntent {
    fn enter(self, fixture: &mut Fixture, out: &mut Queue<Request>) {
        fixture.ui.intents.ask(self.scope, self.origin, self.ask, out);
    }
}

pub fn intent(scope: u64, number: u64, ask: contract::Ask) -> SendIntent {
    SendIntent {
        scope,
        origin: crate::intents::Origin::Card {
            object: contract::ObjectKey::Task(number),
            act: contract::ActKind::Stop,
        },
        ask: Box::new(ask),
    }
}

pub struct Tick;
impl Input for Tick {
    fn enter(self, fixture: &mut Fixture, out: &mut Queue<Request>) {
        fixture.ui.fire(fixture.now, Wall::EPOCH, &fixture.client, out);
    }
}
pub fn kept(number: u8, party: u64) -> skein_lib::RequestRecord<jig_client::Held> {
    skein_lib::RequestRecord {
        key: [number; 16],
        scope: party,
        first_sent: Some(Wall::EPOCH),
        request: jig_client::Held {
            key: Key { asker: jig_client::Asker::JIG, number: u64::from(number) },
            request: jig_client::Ask::Jig(Box::new(contract::Ask::Stop { project: 1, task: 8 })),
        },
    }
}

pub struct CardAsk {
    pub object: contract::ObjectKey,
    pub act: contract::ActKind,
    pub ask: contract::Ask,
}
impl Input for CardAsk {
    fn enter(self, fixture: &mut Fixture, out: &mut Queue<Request>) {
        crate::intents::send_card(&mut fixture.ui, self.object, self.act, Box::new(self.ask), out, &fixture.client);
    }
}

pub struct FireClient {
    pub wall: Wall,
}
impl Input for FireClient {
    fn enter(self, fixture: &mut Fixture, out: &mut Queue<Request>) {
        let mut native = Queue::with_capacity(OUTPUTS);
        fixture.client.fire(fixture.now, self.wall, &mut native);
        fixture.native_out(native, None, None, out);
    }
}
