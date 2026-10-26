use super::*;
use skein_lib::{List, RequestRecord, Wall};
pub fn store_text() -> c::Text {
    c::Text::new(b"kept".as_slice().into(), 32).unwrap()
}
pub fn table_record(ask: c::Ask) -> RequestRecord<jig_client_domain::Held> {
    RequestRecord {
        key: KEY,
        scope: u64::MAX,
        first_sent: Some(Wall::from_nanos(u64::MAX)),
        request: jig_client_domain::Held { key: APPLICATION_KEY, request: Ask::Jig(Box::new(ask)) },
    }
}
fn specification() -> c::Specification {
    c::Specification { words: store_text(), parameters: List::with_capacity(0), inputs: List::with_capacity(0) }
}
fn amendment() -> c::Amendment {
    c::Amendment {
        llm: None,
        addressed: Some(c::AddressChange { revision: read_value_revision(), address: read_value_person_address() }),
        spec: Some(specification()),
        wake: Some(read_value_owner_wake_policy()),
        dependencies: Some(List::with_capacity(0)),
        authority: Some(read_value_owner_authority()),
        instructions: Some(store_text()),
        tracked: None,
    }
}
fn policy() -> c::PolicyEdit {
    c::PolicyEdit::Role(c::PolicyEditRole {
        number: 7,
        authority: read_value_owner_authority(),
        period_spend: u64::MAX,
        requests: u16::MAX,
        decides: u8::MAX,
    })
}
pub fn store_asks() -> List<c::Ask> {
    let revision = read_value_revision();
    list([
        c::Ask::Rename { project: 7, name: store_text() },
        c::Ask::MakeProject { name: store_text(), policy: 1 },
        message(),
        create_ask(),
        propose_amendment(revision),
        propose_batch(),
        c::Ask::Withdraw { project: 7, proposal: u64::MAX },
        compound_ask(revision),
        c::Ask::Message { project: 7, task: u64::MAX, words: store_text() },
        c::Ask::Answer {
            project: 7,
            on: c::AnswerOn::Question { task: u64::MAX, message: u64::MAX, words: store_text() },
        },
        c::Ask::Answer {
            project: 7,
            on: c::AnswerOn::Person {
                task: u64::MAX,
                revision,
                answer: c::PersonAnswer::Approved { state: b"state".as_slice().into(), words: Some(store_text()) },
            },
        },
        c::Ask::Take { project: 7, task: u64::MAX },
        c::Ask::HandBack { project: 7, task: u64::MAX },
        c::Ask::Decide {
            project: 7,
            on: c::DecideOn::Proposal {
                proposal: c::ProposalKey { proposer: c::Proposer::Party(7), number: u64::MAX },
                decision: c::Decision::Rejected { reason: store_text() },
                running: None,
            },
        },
        c::Ask::Decide {
            project: 7,
            on: c::DecideOn::Escalation {
                task: u64::MAX,
                revision,
                decision: c::Decision::LeftHeld { reason: store_text() },
            },
        },
        c::Ask::Release { project: 7, task: u64::MAX, revision, again: Some(true) },
        c::Ask::Amend {
            project: 7,
            task: u64::MAX,
            revision: Some(revision),
            change: Box::new(amendment()),
            reason: store_text(),
        },
        c::Ask::Stop { project: 7, task: u64::MAX },
        c::Ask::Cancel { project: 7, task: u64::MAX, reason: store_text(), grace: None },
        c::Ask::Move { project: 7, task: u64::MAX, reason: store_text() },
        c::Ask::Seen { through: c::SeenThrough::Conversation { task: u64::MAX, through: u64::MAX } },
        c::Ask::EditNote {
            project: 7,
            note: u64::MAX,
            scope: c::NoteScope::Task { project: 7, task: 9 },
            revision,
            description: store_text(),
            body: store_text(),
            references: list([u64::MAX]),
        },
        c::Ask::DeleteNote { project: 7, note: u64::MAX, scope: c::NoteScope::Deployment, revision },
        c::Ask::ChangePolicy { project: 7, edit: policy() },
        c::Ask::ChangePolicy { project: 7, edit: c::PolicyEdit::Ceiling { ceiling: read_value_owner_authority() } },
        templates(),
        c::Ask::Amend {
            project: 7,
            task: u64::MAX,
            revision: None,
            change: Box::new(c::Amendment {
                tracked: Some(c::Tracking::Tracked { priority: u32::MAX }),
                ..amendment()
            }),
            reason: store_text(),
        },
        c::Ask::Amend {
            project: 7,
            task: u64::MAX,
            revision: Some(revision),
            change: Box::new(c::Amendment { tracked: Some(c::Tracking::Untracked), ..amendment() }),
            reason: store_text(),
        },
        c::Ask::SetRoles {
            project: 7,
            holdings: list([c::Holding { party: u64::MAX, role: c::HeldRole::Policy { role: u32::MAX } }]),
        },
        c::Ask::MakeService { project: 7, name: store_text(), role: c::HeldRole::Member },
        c::Ask::SetPool { project: 7, party: u64::MAX, budget: u64::MAX },
        c::Ask::SignIn { holder: c::AccountHolder::Own, provider: u32::MAX },
        c::Ask::SignIn { holder: c::AccountHolder::Deployment, provider: u32::MAX },
        c::Ask::SignOut { holder: c::AccountHolder::Own, source: u32::MAX },
        c::Ask::SignOut { holder: c::AccountHolder::Deployment, source: u32::MAX },
    ])
}
pub fn list<T, const N: usize>(values: [T; N]) -> List<T> {
    let mut out = List::with_capacity(u32::try_from(N).unwrap());
    for value in values {
        out.push(value).ok().expect("fixture list fits");
    }
    out
}
pub const TABLE_VERSION: u16 = jig_client_domain::TABLE_VERSION;
pub fn saved_table(ask: c::Ask) -> Saved {
    Saved { version: TABLE_VERSION, record: table_record(ask) }
}
pub fn store_read(value: Saved) -> Record {
    let mut harness = Harness::new(&limits());
    harness.save(value);
    stored(&harness)
}
pub fn stored(harness: &Harness) -> Record {
    for value in &harness.out {
        if let Request::Save { record: Record { key, bytes: record } } = value {
            return Record { key: key.clone(), bytes: record.clone() };
        }
    }
    panic!("one store write")
}
pub fn other_store_version(mut value: Record) -> Record {
    let record = w::Kept::decode(&limits().wire, &mut Reader::new(&value.bytes)).unwrap();
    let record =
        w::Kept::new(&limits().wire, w::KeptParts { store_version: u16::MAX, record: record.record().into() }).unwrap();
    let mut writer = Writer::new(usize::try_from(record.measure()).unwrap());
    record.encode(&mut writer).unwrap();
    value.bytes = writer.finish();
    value
}
impl Harness {
    pub fn save(&mut self, value: Saved) {
        self.link.step(Time::ZERO, Event::Save { record: value }, &mut self.out);
    }
    pub fn start_store(&mut self, kept: List<Record>) {
        self.link.step(Time::ZERO, Event::Start { kept }, &mut self.out);
    }
    pub fn send_kept(&mut self, attempt: Token) {
        self.link.step(
            Time::ZERO,
            Event::Send { attempt, key: KEY, ask: Ask::Jig(Box::new(message())), value: None },
            &mut self.out,
        );
    }
    pub fn written(&mut self) {
        self.link.step(Time::ZERO, Event::Stored, &mut self.out);
    }
    pub fn write_failed(&mut self) {
        self.link.step(Time::ZERO, Event::NotStored, &mut self.out);
    }
    pub fn exchanges(&self) -> u32 {
        let mut count = 0_u32;
        for value in &self.out {
            match value {
                Request::Exchange { .. } => count = count.saturating_add(1),
                Request::Kept { .. }
                | Request::Save { .. }
                | Request::Erase { .. }
                | Request::Dropped
                | Request::Answer { .. }
                | Request::NotKept { .. }
                | Request::Read { .. }
                | Request::OpenStream { .. }
                | Request::CloseStream
                | Request::Watch { .. }
                | Request::FamilyDelivery { .. }
                | Request::Ended { .. }
                | Request::Hello { .. }
                | Request::Signing { .. }
                | Request::ServiceCredential { .. }
                | Request::Authorized(_)
                | Request::Fact { .. } => {}
            }
        }
        count
    }
    pub fn restored(&self, expected: &Saved) -> bool {
        for value in &self.out {
            match value {
                Request::Kept { record } if record == expected => return true,
                Request::Kept { .. }
                | Request::Exchange { .. }
                | Request::Save { .. }
                | Request::Erase { .. }
                | Request::Dropped
                | Request::Answer { .. }
                | Request::NotKept { .. }
                | Request::Read { .. }
                | Request::OpenStream { .. }
                | Request::CloseStream
                | Request::Watch { .. }
                | Request::FamilyDelivery { .. }
                | Request::Ended { .. }
                | Request::Hello { .. }
                | Request::Signing { .. }
                | Request::ServiceCredential { .. }
                | Request::Authorized(_)
                | Request::Fact { .. } => {}
            }
        }
        false
    }
}

pub fn one_store_read(value: Record) -> List<Record> {
    list([value])
}

pub const APPLICATION_KEY: jig_client_domain::Key =
    jig_client_domain::Key { asker: jig_client_domain::Asker::JIG, number: u64::MAX };

fn templates() -> c::Ask {
    c::Ask::ChangePolicy {
        project: 7,
        edit: c::PolicyEdit::Templates {
            overrides: list([
                c::TemplateOverride {
                    template: 1,
                    executor: Some(c::TemplateExecutor::Charter(u32::MAX)),
                    budget: Some(c::TemplateBudget::Amount(u64::MAX)),
                    deadline: Some(c::TemplateDeadline::Span(Duration::from_nanos(u64::MAX))),
                    priority: Some(u32::MAX),
                },
                c::TemplateOverride {
                    template: 2,
                    executor: Some(c::TemplateExecutor::Chosen),
                    budget: Some(c::TemplateBudget::AsAllowed { cap: Some(u64::MAX) }),
                    deadline: Some(c::TemplateDeadline::Given),
                    priority: None,
                },
            ]),
        },
    }
}

fn compound_ask(revision: c::Revision) -> c::Ask {
    c::Ask::Compound {
        project: 7,
        operations: list([
            c::Operation::Message { project: 7, task: u64::MAX, words: store_text() },
            c::Operation::Amend {
                project: 7,
                task: u64::MAX,
                revision: Some(revision),
                change: Box::new(amendment()),
                reason: store_text(),
            },
            c::Operation::Release { project: 7, task: u64::MAX, revision, again: Some(true) },
            c::Operation::Cancel { project: 7, task: u64::MAX, reason: store_text(), grace: None },
            c::Operation::Move { project: 7, task: u64::MAX, reason: store_text() },
            c::Operation::Stop { project: 7, task: u64::MAX },
        ]),
    }
}

fn propose_batch() -> c::Ask {
    let c::Ask::Create { project, tasks } = create_ask() else { panic!("create fixture") };
    c::Ask::Propose { project, action: c::ProposeAction::Batch { tasks }, reason: store_text() }
}

fn propose_amendment(revision: c::Revision) -> c::Ask {
    c::Ask::Propose {
        project: 7,
        action: c::ProposeAction::Amendment { task: u64::MAX, revision, change: Box::new(amendment()) },
        reason: store_text(),
    }
}

pub fn kept_stream() -> Record {
    let record =
        w::Kept::new(&limits().wire, w::KeptParts { store_version: TABLE_VERSION, record: Box::from(STREAM) }).unwrap();
    let mut writer = Writer::new(usize::try_from(record.measure()).unwrap());
    record.encode(&mut writer).unwrap();
    Record { key: Box::from(STREAM_KEY), bytes: writer.finish() }
}
