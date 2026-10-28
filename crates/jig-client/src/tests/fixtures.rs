pub mod thread;
use crate::*;
use alloc::boxed::Box;
use jig_client_protocol_wire::v19 as w;
use skein_lib::{Duration, List, Queue, Secret, Time, Token, Wall, Writer};
pub const PARTY: u64 = 7;
pub const SUBJECT_TASK: u64 = 9;
pub const PROJECT: u32 = 1;
pub const TASK: Handle = Handle { asker: Asker::JIG, number: 2 };
pub const ME: Handle = Handle { asker: Asker::JIG, number: 3 };
pub const STOP: Key = Key { asker: Asker::JIG, number: 1 };
pub fn domain_limits() -> DomainLimits {
    DomainLimits {
        owners: &[],
        // tests/client/src/test.rs:70-77, the concrete world's table profile.
        requests: skein_lib::RequestLimits {
            requests: 4,
            bytes: 65536,
            out: 16,
            first: Duration::from_millis(10),
            most: Duration::from_millis(80),
            margin: Duration::from_secs(1),
        },
        ask_bytes: 16384,
        rows: 2,
        segments: 4,
        text: 32,
        turn_text: 32,
        projects: 2,
        fields: 4,
        delegates: 2,
        records: 2,
        kept: 4,
        parts: 8,
        authority: 8,
        accounts: AccountCounts { accounts: 8, sources: 8, sign_ins: 8 },
        heartbeat: Duration::from_millis(100),
        handles: 8,
        turn_bytes: 32,        // tests/client/src/test.rs:42
        reads: 8,              // tests/client/src/test.rs:32
        watches: 8,            // tests/client/src/test.rs:23
        document_bytes: 65536, // tests/client/src/test.rs:38
        backoff: Backoff {
            // tests/client/src/test.rs:51-54
            first: Duration::from_millis(10),
            most: Duration::from_millis(80),
        },
    }
}
pub fn limits() -> Limits {
    Limits { domain: domain_limits(), exchanges: 8 }
}
pub fn task() -> Watchable {
    Watchable::Jig(contract::Subject::Task { task: SUBJECT_TASK })
}
pub fn stop() -> Ask {
    Ask::Jig(Box::new(contract::Ask::Stop { project: PROJECT, task: SUBJECT_TASK }))
}
pub fn seen() -> Ask {
    Ask::Jig(Box::new(contract::Ask::Seen {
        through: contract::SeenThrough::Conversation { task: SUBJECT_TASK, through: 1 },
    }))
}
pub fn hello(version: u32) -> Box<[u8]> {
    let served =
        w::Served::new(&w::CEILINGS, w::ServedParts { owner: w::Owner::Jig, first: version, last: version }).unwrap();
    let mut rows = List::with_capacity(1);
    rows.push(served).unwrap();
    event(&w::StreamEvent::Hello(
        w::Hello::new(
            &w::CEILINGS,
            w::HelloParts { stream: [7; 16], served: rows, heartbeat: Duration::from_millis(100) },
        )
        .unwrap(),
    ))
}
pub fn event(value: &w::StreamEvent) -> Box<[u8]> {
    let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
    value.encode(&mut out).unwrap();
    out.finish()
}
pub fn frame(watch: u64) -> Box<[u8]> {
    let mut parts = jig_client_test_records::frame_record().into_parts();
    parts.retention = Duration::from_secs(60);
    let value = w::DeliveryValue::Me(w::Me::new(&w::CEILINGS, parts).unwrap());
    let mut bytes = Writer::new(usize::try_from(value.measure()).unwrap());
    value.encode(&mut bytes).unwrap();
    let document = w::Owned::new(
        &w::CEILINGS,
        w::OwnedParts { owner: w::Owner::Jig, version: jig_client_protocol_wire::VERSION, record: bytes.finish() },
    )
    .unwrap();
    event(&w::StreamEvent::Delivery(w::Delivery::new(&w::CEILINGS, w::DeliveryParts { watch, document }).unwrap()))
}
pub struct Client {
    pub machine: crate::Client,
    pub out: Queue<Request>,
    pub requests: List<Request>,
}
impl Client {
    pub fn new() -> Self {
        Self::with_kept(List::with_capacity(0))
    }
    pub fn with_kept(kept: List<Record>) -> Self {
        let limits = limits();
        Self::with_limits(limits, kept)
    }
    pub fn with_limits(limits: Limits, kept: List<Record>) -> Self {
        let mut c = Self {
            machine: crate::Client::new(&limits, 7).unwrap(),
            out: Queue::uncapped(),
            requests: List::with_capacity(0),
        };
        c.step(Event::Start { kept });
        c
    }
    pub fn narrow_signed_in() -> Self {
        let limits = limits();
        let limits = Limits {
            domain: DomainLimits {
                handles: 1,
                watches: 2,
                reads: 1,
                records: 1,
                kept: 1,
                parts: 1,
                requests: skein_lib::RequestLimits { requests: 1, out: 2, ..limits.domain.requests },
                turn_text: limits.domain.turn_text,
                ..limits.domain
            },
            exchanges: 5,
        };
        let mut c = Self::with_limits(limits, List::with_capacity(0));
        c.sign_in();
        c
    }
    pub fn signed_in() -> Self {
        let mut c = Self::new();
        c.sign_in();
        c
    }
    pub fn sign_in(&mut self) {
        self.step(Event::SignIn { with: With::Held(Secret::new(Box::new([7; 40]))) });
        self.stream(hello(jig_client_protocol_wire::VERSION));
        let watch = self.opened_watch();
        self.stream(frame(watch));
        self.step(Event::Stored);
        self.requests.clear();
    }
    pub fn step(&mut self, event: Event) {
        self.requests.clear();
        self.machine.step(Time::ZERO, Wall::EPOCH, event, &mut self.out);
        self.requests = List::with_capacity(self.out.len());
        while let Some(request) = self.out.pop() {
            self.requests.push(request).unwrap();
        }
    }
    pub fn fire(&mut self, now: Time) {
        self.requests.clear();
        self.machine.fire(now, Wall::EPOCH, &mut self.out);
        self.requests = List::with_capacity(self.out.len());
        while let Some(request) = self.out.pop() {
            self.requests.push(request).unwrap();
        }
    }
    pub fn stream(&mut self, bytes: Box<[u8]>) {
        self.step(Event::Streamed { message: Streamed::Record(bytes) });
    }
    pub fn requests(&self) -> &[Request] {
        self.requests.as_slice()
    }
    pub fn opened_watch(&self) -> u64 {
        for request in &self.requests {
            if let Request::Exchange { message: Message::Watch(bytes), .. } = request {
                return w::Open::decode(&w::CEILINGS, &mut skein_lib::Reader::new(bytes)).unwrap().watch();
            }
        }
        panic!("one watch opening")
    }
    pub fn exchange(&self) -> Token {
        for request in &self.requests {
            if let Request::Exchange { exchange, .. } = request {
                return *exchange;
            }
        }
        panic!("one exchange")
    }
    pub fn read(&mut self, handle: Handle) -> Token {
        self.step(Event::Read { handle, read: Readable::Jig(contract::Read::Project { project: PROJECT }) });
        self.exchange()
    }
}

pub fn keyed_count(requests: &[Request]) -> usize {
    let mut count = 0_usize;
    for request in requests {
        if let Request::Exchange { message: Message::Request(_), .. } = request {
            count = count.checked_add(1).expect("bounded requests");
        }
    }
    count
}
pub fn unknown_read() -> Box<[u8]> {
    read_none(w::ReadNone::Unknown)
}
pub fn read_none(why: w::ReadNone) -> Box<[u8]> {
    let value =
        w::ReadAnswer::None(w::ReadAnswerNone::new(&w::CEILINGS, w::ReadAnswerNoneParts { value: why }).unwrap());
    let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
    value.encode(&mut out).unwrap();
    out.finish()
}

pub fn saved(requests: &[Request]) -> List<Record> {
    let mut count = 0_u32;
    for request in requests {
        if let Request::Save { records } = request {
            count = count.checked_add(records.len()).expect("collected records fit");
        }
    }
    let mut records = List::with_capacity(count);
    for request in requests {
        if let Request::Save { records: rows } = request {
            for record in rows {
                records.push(record.clone()).unwrap();
            }
        }
    }
    records
}
pub fn answer(value: &w::KeyedAnswer) -> Box<[u8]> {
    let mut out = Writer::new(usize::try_from(value.measure()).unwrap());
    value.encode(&mut out).unwrap();
    out.finish()
}
pub fn other_store_version(record: &mut Record) {
    let value = w::Kept::decode(&w::CEILINGS, &mut skein_lib::Reader::new(&record.bytes)).unwrap();
    let mut parts = value.into_parts();
    parts.store_version = parts.store_version.checked_add(1).unwrap();
    let value = w::Kept::new(&w::CEILINGS, parts).unwrap();
    let mut writer = Writer::new(usize::try_from(value.measure()).unwrap());
    value.encode(&mut writer).unwrap();
    record.bytes = writer.finish();
}

pub fn answers(requests: &[Request]) -> List<(Key, Outcome)> {
    let mut answers = List::with_capacity(u32::try_from(requests.len()).unwrap());
    for request in requests {
        if let Request::Told { told: Told::Answered { key, outcome } } = request {
            answers.push((*key, outcome.clone())).unwrap();
        }
    }
    answers
}

pub const CREDENTIAL: [u8; 40] = [7; 40];
pub const SERVICE_CREDENTIAL: [u8; 40] = [8; 40];
pub const NEW_CREDENTIAL: [u8; 40] = [9; 40];
pub fn credential(bytes: &[u8; 40]) -> Secret {
    Secret::new(Box::new(*bytes))
}
pub fn signed_in(credential: &[u8; 40]) -> Box<[u8]> {
    let record = w::SignInAnswer::SignedIn(
        w::SignedIn::new(
            &w::CEILINGS,
            w::SignedInParts { token: Some(self::credential(credential)), party: PARTY, expires: 10 },
        )
        .unwrap(),
    );
    let mut out = Writer::new(usize::try_from(record.measure()).unwrap());
    record.encode(&mut out).unwrap();
    out.finish()
}
pub fn sign_in_answer(record: &w::SignInAnswer) -> Box<[u8]> {
    let mut out = Writer::new(usize::try_from(record.measure()).unwrap());
    record.encode(&mut out).unwrap();
    out.finish()
}
pub fn sign_out_answer(record: &w::SignOutAnswer) -> Box<[u8]> {
    let mut out = Writer::new(usize::try_from(record.measure()).unwrap());
    record.encode(&mut out).unwrap();
    out.finish()
}
pub fn ended(requests: &[Request]) -> List<(Handle, Why)> {
    let mut ended = List::with_capacity(u32::try_from(requests.len()).unwrap());
    for request in requests {
        if let Request::Told { told: Told::Ended { handle, why } } = request {
            ended.push((*handle, *why)).unwrap();
        }
    }
    ended
}
pub fn pages(requests: &[Request]) -> List<(Handle, Readout)> {
    let mut pages = List::with_capacity(u32::try_from(requests.len()).unwrap());
    for request in requests {
        if let Request::Told { told: Told::Page { handle, page } } = request {
            pages.push((*handle, page.clone())).unwrap();
        }
    }
    pages
}
#[derive(Clone, Copy, Debug)]
pub enum SigningTerminal {
    Request,
    Read,
    Watch,
    Stream,
    SignOut,
}
pub const SIGNING_TERMINALS: [SigningTerminal; 5] = [
    SigningTerminal::Request,
    SigningTerminal::Read,
    SigningTerminal::Watch,
    SigningTerminal::Stream,
    SigningTerminal::SignOut,
];
pub struct SigningExchanges {
    pub request: Token,
    pub read: Token,
    pub opening: Token,
    pub watch: u64,
}
impl Client {
    pub fn signed_out_by(&mut self, terminal: SigningTerminal, exchanges: &SigningExchanges) {
        let (exchange, message) = match terminal {
            SigningTerminal::Request => (exchanges.request, answer(&w::KeyedAnswer::SignedOut)),
            SigningTerminal::Read => (exchanges.read, read_none(w::ReadNone::SignedOut)),
            SigningTerminal::Watch => {
                let record = w::OpenAnswer::Refused(
                    w::OpenAnswerRefused::new(
                        &w::CEILINGS,
                        w::OpenAnswerRefusedParts { why: w::WatchRefusal::SignedOut },
                    )
                    .unwrap(),
                );
                let mut out = Writer::new(usize::try_from(record.measure()).unwrap());
                record.encode(&mut out).unwrap();
                (exchanges.opening, out.finish())
            }
            SigningTerminal::Stream => {
                self.stream(event(&w::StreamEvent::Ended(
                    w::Ended::new(&w::CEILINGS, w::EndedParts { watch: exchanges.watch, why: w::WatchEnd::SignedOut })
                        .unwrap(),
                )));
                return;
            }
            SigningTerminal::SignOut => {
                self.step(Event::SignOut);
                (self.exchange(), sign_out_answer(&w::SignOutAnswer::SignedOut))
            }
        };
        self.step(Event::Reply { exchange, message });
    }
}

pub fn facts_task() -> contract::Task {
    let limits = jig_client_protocol_link::Limits::new(domain_limits(), 8).unwrap();
    let mut parts = jig_client_test_records::task_record().into_parts();
    parts.chip = jig_client_test_records::owner_waiting_chip_record();
    let wire = w::Task::new(&w::CEILINGS, parts).unwrap();
    jig_client_protocol_link::translate::documents::task(wire, &limits).unwrap()
}
pub fn me() -> contract::Me {
    let limits = jig_client_protocol_link::Limits::new(domain_limits(), 8).unwrap();
    let mut me =
        jig_client_protocol_link::translate::documents::frame(jig_client_test_records::frame_record(), &limits)
            .unwrap();
    me.party.number = PARTY;
    me.projects.clear();
    let mut templates = List::with_capacity(2);
    for number in [1, 2] {
        templates
            .push(contract::Template {
                llm: None,
                number,
                name: contract::DocumentText {
                    bytes: contract::Text::new(b"template".as_slice().into(), 32).unwrap(),
                    cut: false,
                },
                executor: if number == 1 {
                    contract::TemplateExecutor::Charter(1)
                } else {
                    contract::TemplateExecutor::Chosen
                },
                contract: contract::TaskContractDocument::Report { words: 32 },
                shallower: 1,
                budget: if number == 1 {
                    contract::TemplateBudget::AsAllowed { cap: None }
                } else {
                    contract::TemplateBudget::Given
                },
                deadline: contract::TemplateDeadline::None,
                tracked: if number == 2 { Some(10) } else { None },
                flags: contract::TaskFlags {
                    ended_by: contract::EndedBy::Result,
                    conversation: number == 1,
                    listed: number == 2,
                    retention: contract::Retention::Kept,
                },
                label: number,
                grace: Some(SHAPE_GRACE),
            })
            .unwrap();
    }
    me.projects
        .push(contract::MeProject {
            models: List::with_capacity(0),
            project: PROJECT,
            name: contract::DocumentText::complete(contract::Text::new(Box::from(*b"Tally"), 32).unwrap()),
            role: contract::HeldRole::Owner,
            pool: None,
            templates,
            creating: Some(creating()),
        })
        .unwrap();
    me
}
pub fn facts_chip(phase: contract::OwnerChipPhase, work: Option<contract::OwnerChipWork>) -> contract::Chip {
    let mut chip = facts_task().chip;
    let contract::ChipPhase::Owner { details, .. } = chip.details.as_mut() else { panic!("owner fixture") };
    details.phase_since = PHASE_SINCE;
    details.ended_at = Some(ENDED_AT);
    details.retry = Some(RETRY_NOTICE);
    details.observation = None;
    if let Some(work) = work {
        details.observation = Some(contract::OwnerChipObservation {
            channel: Token::new(CHANNEL),
            work,
            since: OBSERVED_SINCE,
            reported_at: OBSERVED_SINCE,
            started: None,
        });
    }
    let details = details.clone();
    chip.details = Box::new(contract::ChipPhase::Owner { phase, details });
    chip
}
pub const PHASE_SINCE: Wall = Wall::from_nanos(11);
pub const OBSERVED_SINCE: Wall = Wall::from_nanos(13);
pub const ENDED_AT: Wall = Wall::from_nanos(17);
pub const RETRY_AT: Wall = Wall::from_nanos(19);
pub const ATTEMPT: u64 = 23;
pub const CHANNEL: u64 = 29;
pub const OTHER_PARTY: u64 = 31;
pub const OTHER_PROJECT: u32 = 2;
pub const OWNER_ROLE: u32 = 0;
pub const MEMBER_ROLE: u32 = 2;
pub const RETRY_NOTICE: contract::RetryNotice =
    contract::RetryNotice::OwnerFailed { class: contract::HoldFailure::Lost, count: 2, allowance: 3 };

/// Five seconds distinguishes a duration from the fixture wall in nanoseconds.
pub const SHAPE_GRACE: Duration = Duration::from_secs(5);
/// Four levels allow a three-level narrowing to retain two delegate levels.
pub const SHAPE_DEPTH: u32 = 4;
pub const SHAPE_NARROWING: u32 = 3;
pub fn creating() -> contract::Creating {
    let mut authority = facts_task().authority;
    authority.cut = false;
    authority.authority.delegation.depth = SHAPE_DEPTH;
    let mut charters = List::with_capacity(1);
    charters.push(1).unwrap();
    contract::Creating {
        within: contract::CreateBounds {
            authority: Box::new(authority),
            pool_left: 100,
            charters,
            procedures: List::with_capacity(0),
            latest: None,
            budget: 100,
            maximum_budget: 500,
            maximum_deadline: None,
        },
        beyond: contract::Beyond::Proposed { to: contract::Recipient::Role { project: PROJECT, role: OWNER_ROLE } },
    }
}

/// The first membership owns both offered shape templates.
pub fn shape_project(me: &mut contract::Me) -> &mut contract::MeProject {
    me.projects.get_mut(0).unwrap()
}
/// Read a shaped single-task create without repeating its destructuring.
pub fn shape_task(ask: contract::Ask) -> contract::NewTask {
    let contract::Ask::Create { tasks, .. } = ask else { panic!("create") };
    assert_eq!(tasks.len(), 1);
    tasks.get(0).unwrap().clone()
}
/// A span goal fits the ordinary budget; its wall bounds are deliberately finite.
pub fn span_goal_me() -> contract::Me {
    let mut me = me();
    let project = shape_project(&mut me);
    project.templates.get_mut(1).unwrap().deadline = contract::TemplateDeadline::Span(SHAPE_GRACE);
    let within = &mut project.creating.as_mut().unwrap().within;
    within.latest = Some(Wall::from_nanos(100));
    within.maximum_deadline = Some(Wall::from_nanos(100));
    me
}

pub fn seen_answer() -> Box<[u8]> {
    let seen = w::SeenThrough::Conversation(
        w::SeenConversation::new(&w::CEILINGS, w::SeenConversationParts { task: SUBJECT_TASK, through: 1 }).unwrap(),
    );
    let marked = w::OwnerOutcomeMarkedSeen::new(&w::CEILINGS, w::OwnerOutcomeMarkedSeenParts { seen }).unwrap();
    let outcome =
        w::OutcomeOwner::new(&w::CEILINGS, w::OutcomeOwnerParts { value: w::OwnerOutcome::MarkedSeen(marked) })
            .unwrap();
    let done = w::Done::new(&w::CEILINGS, w::DoneParts { outcome: w::Outcome::Owner(outcome) }).unwrap();
    let value = w::Final::Done(done);
    let mut writer = Writer::new(usize::try_from(value.measure()).unwrap());
    value.encode(&mut writer).unwrap();
    let owned = w::Owned::new(
        &w::CEILINGS,
        w::OwnedParts { owner: w::Owner::Jig, version: jig_client_protocol_wire::VERSION, record: writer.finish() },
    )
    .unwrap();
    answer(&w::KeyedAnswer::FinalAnswer(owned))
}
pub fn kept_seen(key: Key, name: skein_lib::RequestKey, first_sent: Option<Wall>) -> Record {
    let limits = jig_client_protocol_link::Limits::new(domain_limits(), limits().exchanges).unwrap();
    let table = jig_client_protocol_link::translate::store::table(
        &skein_lib::RequestRecord { key: name, scope: PARTY, first_sent, request: Held { key, request: seen() } },
        &limits,
    )
    .unwrap();
    let mut writer = Writer::new(usize::try_from(table.measure()).unwrap());
    table.encode(&mut writer).unwrap();
    let kept = w::Kept::new(
        &w::CEILINGS,
        w::KeptParts { store_version: jig_client_domain::TABLE_VERSION, record: writer.finish() },
    )
    .unwrap();
    let mut writer = Writer::new(usize::try_from(kept.measure()).unwrap());
    kept.encode(&mut writer).unwrap();
    Record { key: Box::from(name), bytes: writer.finish() }
}
