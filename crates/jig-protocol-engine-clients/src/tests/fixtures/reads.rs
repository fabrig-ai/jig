use super::*;
use jig_client_test_records as records;
use jig_domain_core::reads;
use skein_lib::Reader;
pub fn read_exchange() -> Exchange {
    let mut limits = env();
    limits.limits.wire.owned_record = 4096;
    Exchange::with_env(limits)
}
pub fn encoded_read(value: &w::Read) -> Box<[u8]> {
    let mut out = Writer::new(usize::try_from(value.measure()).expect("read size"));
    value.encode(&mut out).expect("read bytes");
    out.finish()
}
pub fn owned_read(owner: w::Owner, version: u32, record: Box<[u8]>) -> Box<[u8]> {
    let record = match owner {
        w::Owner::Jig => record,
        w::Owner::Family(_) => family_record(record),
    };
    let value = w::Owned::new(&env().limits.wire, w::OwnedParts { owner, version, record }).expect("read envelope");
    let mut out = Writer::new(usize::try_from(value.measure()).expect("envelope size"));
    value.encode(&mut out).expect("envelope bytes");
    out.finish()
}
pub fn value_answer(value: &w::ReadValue) -> w::ReadAnswer {
    let mut out = Writer::new(usize::try_from(value.measure()).expect("value size"));
    value.encode(&mut out).expect("value bytes");
    let limits = w::Limits { owned_record: 4096, ..wire() };
    w::ReadAnswer::Value(
        w::Owned::new(&limits, w::OwnedParts { owner: w::Owner::Jig, version: 19, record: out.finish() })
            .expect("value envelope"),
    )
}
pub fn read_none(value: w::ReadNone) -> w::ReadAnswer {
    w::ReadAnswer::None(w::ReadAnswerNone::new(&wire(), w::ReadAnswerNoneParts { value }).expect("none"))
}
pub fn held_reads() -> [(&'static str, w::Read, reads::Read, reads::Answer, w::ReadAnswer); 4] {
    let limits = wire();
    [
        (
            "document",
            w::Read::Me,
            reads::Read::Frame,
            reads::Answer::Document(Box::new(core::documents::Document::Frame(frame_two_projects()))),
            value_answer(&w::ReadValue::Me(frame_two_projects_record())),
        ),
        (
            "page",
            w::Read::Activity(
                w::ReadActivity::new(&limits, w::ReadActivityParts { task: 7, before: None }).expect("activity"),
            ),
            reads::Read::Activity { task: 7, before: None },
            reads::Answer::Page(Box::new(activity_page())),
            value_answer(&w::ReadValue::Activity(records::carried_activity_page())),
        ),
        (
            "historical",
            w::Read::Task(w::ReadTask::new(&limits, w::ReadTaskParts { task: 7 }).expect("task")),
            reads::Read::Task { task: 7 },
            reads::Answer::Historical(Box::new(historical_task())),
            value_answer(&w::ReadValue::Historical(Box::new(records::historical_task_record()))),
        ),
        (
            "none",
            w::Read::Deployment,
            reads::Read::Deployment,
            reads::Answer::None(reads::Why::Gone),
            read_none(w::ReadNone::Gone),
        ),
    ]
}
impl Exchange {
    pub fn read(&mut self, value: &w::Read) -> (Token, ReplyTo, reads::Read) {
        let owner = self.send(crate::Message::Read(owned_read(w::Owner::Jig, 19, encoded_read(value))));
        self.authenticate();
        let Some(Request::Core(event)) = self.out.pop() else { panic!("core read") };
        let core::Event::Read { reply, sign_in, read } = *event else { panic!("read") };
        assert_eq!(sign_in, NUMBER);
        (owner, reply, read)
    }
    pub fn release_read(&mut self, reply: ReplyTo, answer: reads::Answer) {
        self.component.down(&self.env, Event::Read { reply, answer }, &mut self.out);
    }
    pub fn read_unanswered(&self, owner: Token) -> bool {
        self.component.connections.get(&owner).expect("read owner").response.is_none()
    }
    pub fn read_answer(&self, owner: Token) -> w::ReadAnswer {
        let (status, body) = self.response(owner);
        assert_eq!(status, 200);
        w::ReadAnswer::decode(&self.env.limits.wire, &mut Reader::new(body)).expect("read answer")
    }
    pub fn application_read(&mut self) -> (Token, ReplyTo) {
        let owner = w::Owner::Family(w::Family::new(&wire(), w::FamilyParts { number: 9 }).expect("family"));
        let socket = self.send(crate::Message::Read(owned_read(owner, 2, Box::from([255, 0, 1]))));
        self.authenticate();
        let Some(Request::Application(crate::ToApplication::Read { reply, sign_in, family, version, record, .. })) =
            self.out.pop()
        else {
            panic!("application read")
        };
        assert_eq!(sign_in, NUMBER);
        assert_eq!(family, 9);
        assert_eq!(version, 2);
        assert_eq!(record.as_ref(), &[255, 0, 1]);
        (socket, reply)
    }
    pub fn release_application_read(&mut self, reply: ReplyTo, family: u16, record: Option<Box<[u8]>>) {
        self.component.down(
            &self.env,
            Event::Application(crate::FromApplication::Read { reply, family, record }),
            &mut self.out,
        );
    }
}
pub fn cursors() -> Box<[(&'static str, reads::Cursor)]> {
    use jig_domain_core::{Key, documents as d};
    let position =
        reads::Position::Owner(Key::Tasks(jig_domain_tasks::Key::Activity { task: u64::MAX, position: u64::MAX }));
    let scopes = cursor_scopes();
    let mut rows = List::with_capacity(64);
    for (label, scope) in scopes {
        rows.push((label, reads::Cursor::Store { scope: scope.clone(), after: Some(position.clone()) }))
            .expect("cursor fixture room");
        rows.push((
            label,
            reads::Cursor::Live {
                scope,
                after: d::Cursor::Number { number: u64::MAX },
                store_after: Some(position.clone()),
            },
        ))
        .expect("cursor fixture room");
    }
    inbox_cursors(&mut rows);
    note_cursors(&mut rows);
    rows.push((
        "LiveNone",
        reads::Cursor::Live {
            scope: reads::Scope::Activity { task: 7 },
            after: d::Cursor::Number { number: 7 },
            store_after: None,
        },
    ))
    .expect("cursor fixture room");
    rows.into_boxed()
}

fn cursor_scopes() -> [(&'static str, reads::Scope); 18] {
    use jig_domain_notes as notes;
    use jig_domain_people as p;
    [
        ("Task", reads::Scope::Task { task: u64::MAX }),
        ("ProposalTask", reads::Scope::Proposal { proposer: p::Proposer::Task(u64::MAX), proposal: u64::MAX }),
        ("ProposalParty", reads::Scope::Proposal { proposer: p::Proposer::Party(u64::MAX), proposal: u64::MAX }),
        ("Escalation", reads::Scope::Escalation { task: u64::MAX }),
        ("Question", reads::Scope::Question { task: u64::MAX, message: u64::MAX }),
        ("PersonTask", reads::Scope::PersonTask { task: u64::MAX }),
        ("Result", reads::Scope::Result { task: u64::MAX }),
        ("Reply", reads::Scope::Reply { task: u64::MAX, activity: u64::MAX }),
        ("Turn", reads::Scope::Turn { task: u64::MAX, attempt: u64::MAX, turn: u32::MAX }),
        ("EndedDelegates", reads::Scope::EndedDelegates { task: u64::MAX }),
        ("History", reads::Scope::History { task: u64::MAX, subtree: true }),
        ("Activity", reads::Scope::Activity { task: u64::MAX }),
        ("Notes", reads::Scope::Notes { project: u32::MAX }),
        ("ScopedNotesAbsent", reads::Scope::ScopedNotes { scope: notes::Scope::Deployment, description: None }),
        (
            "ScopedNotesEmpty",
            reads::Scope::ScopedNotes {
                scope: notes::Scope::Project { project: u32::MAX },
                description: Some(Box::new([])),
            },
        ),
        (
            "ScopedNotesGoal",
            reads::Scope::ScopedNotes {
                scope: notes::Scope::Goal { project: u32::MAX, goal: u64::MAX },
                description: Some(Box::from(b"search".as_slice())),
            },
        ),
        ("Spend", reads::Scope::Spend { project: u32::MAX }),
        ("Holds", reads::Scope::Holds { project: u32::MAX }),
    ]
}
fn inbox_cursors(rows: &mut List<(&'static str, reads::Cursor)>) {
    use jig_domain_core::{CoreKey, Key, documents as d};
    use jig_domain_people as p;
    for (label, group) in [
        ("Needs", reads::Group::Needs),
        ("Updates", reads::Group::Updates),
        ("Proposals", reads::Group::Proposals),
        ("Escalations", reads::Group::Escalations),
        ("Questions", reads::Group::Questions),
        ("PersonTasks", reads::Group::PersonTasks),
        ("Results", reads::Group::Results),
        ("Replies", reads::Group::Replies),
    ] {
        rows.push((
            label,
            reads::Cursor::Store {
                scope: reads::Scope::Inbox {
                    group,
                    filter: reads::Filter { project: Some(u32::MAX), goal: Some(u64::MAX) },
                },
                after: None,
            },
        ))
        .expect("cursor fixture room");
    }
    let objects = [
        ("InboxQuestion", d::InboxObject::Question { message: u64::MAX }),
        ("InboxTaskProposal", d::InboxObject::Proposal { proposer: p::Proposer::Task(u64::MAX), number: u64::MAX }),
        ("InboxPartyProposal", d::InboxObject::Proposal { proposer: p::Proposer::Party(u64::MAX), number: u64::MAX }),
        ("InboxEscalation", d::InboxObject::Escalation { revision: u64::MAX }),
        ("InboxPersonTask", d::InboxObject::PersonTask),
        ("InboxReply", d::InboxObject::Reply { message: u64::MAX }),
        ("InboxResult", d::InboxObject::Result { position: u64::MAX }),
    ];
    for (label, object) in objects {
        let key = Key::Core(CoreKey::ProposalDecision { proposer: p::Proposer::Party(u64::MAX), proposal: u64::MAX });
        rows.push((
            label,
            reads::Cursor::Live {
                scope: reads::Scope::Inbox {
                    group: reads::Group::Needs,
                    filter: reads::Filter { project: None, goal: None },
                },
                after: d::Cursor::Inbox { task: u64::MAX, object, at: Some(Wall::from_nanos(u64::MAX)) },
                store_after: Some(reads::Position::Inbox { at: None, task: u64::MAX, object, key }),
            },
        ))
        .expect("cursor fixture room");
    }
}
fn note_cursors(rows: &mut List<(&'static str, reads::Cursor)>) {
    use jig_domain_core::Key;
    use jig_domain_notes as notes;
    for (label, last) in [
        ("ResourceExact", notes::Last::Exact(Box::from([255, b'/']))),
        ("ResourceOpen", notes::Last::Open(Box::from([255, b'/']))),
    ] {
        let scope = notes::Scope::Resources {
            project: u32::MAX,
            connector: u16::MAX,
            pattern: notes::Pattern { segments: Box::new([Box::from([255, 0].as_slice()), Box::new([])]), last },
        };
        rows.push((
            label,
            reads::Cursor::Store {
                scope: reads::Scope::ScopedNotes {
                    scope: scope.clone(),
                    description: Some(Box::from(b"search".as_slice())),
                },
                after: Some(reads::Position::Owner(Key::Notes(notes::Key::Line { scope, name: u64::MAX }))),
            },
        ))
        .expect("cursor fixture room");
    }
}
