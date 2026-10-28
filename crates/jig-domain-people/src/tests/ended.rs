use super::*;
const PARTY: u64 = 1;
const SIGN_IN: u64 = 10;
const TASK: u64 = 2;
const PROJECT: u32 = 1;
const MARK: u8 = 1;

fn world() -> Test {
    let mut test = Test::new(LIMITS);
    test.signin(PARTY, SIGN_IN, identity(0, 1));
    test.send(Event::Roles { project: PROJECT, holdings: Box::new([Holding { person: PARTY, role: Role::Member }]) });
    test.send(Event::ChatState {
        party: PARTY,
        project: PROJECT,
        task: TASK,
        last_reply: 2,
        live: true,
        seen: None,
        replies: Box::new([]),
    });
    test
}

#[test]
fn an_ended_conversation_moves_its_position_to_the_ended_family() {
    let mut test = world();
    let request =
        route(&test.request(SIGN_IN, MARK, Ask::MarkSeen { seen: Seen::Conversation { task: TASK, through: 1 } }));
    test.send(Event::ConversationSeen { request, project: PROJECT, last_reply: 2 });
    let out = test.send(Event::ChatState {
        party: PARTY,
        project: PROJECT,
        task: TASK,
        last_reply: 2,
        live: false,
        seen: None,
        replies: Box::new([]),
    });
    assert!(out.contains(&Request::Save { record: Stored::EndedRead { task: TASK, party: PARTY, through: 1 } }));
    assert!(out.contains(&Request::Erase { key: Key::ReadConversation { party: PARTY, task: TASK } }));
    assert_eq!(test.d.conversation_seen(PARTY, TASK), Some(0));
}

#[test]
fn an_ended_read_position_is_refused_at_restore() {
    let mut domain = Domain::new(&LIMITS, Box::new([]), 2, Box::new([]));
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: LIMITS };
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    step(
        &mut domain,
        &env,
        Event::Restore { record: Stored::EndedRead { task: TASK, party: PARTY, through: 1 } },
        &mut out,
    );
    assert!(!domain.ready());
    assert_eq!(
        out.pop(),
        Some(Request::RestoreRefused {
            key: Key::EndedRead { task: TASK, party: PARTY },
            refusal: Refusal::Limit { which: LimitKind::Restore, bound: None },
        })
    );
    assert!(out.is_empty());
}

#[test]
fn a_mark_on_an_ended_conversation_saves_its_ended_position() {
    let mut test = world();
    test.send(Event::ChatState {
        party: PARTY,
        project: PROJECT,
        task: TASK,
        last_reply: 2,
        live: false,
        seen: None,
        replies: Box::new([]),
    });
    let seen = Seen::Conversation { task: TASK, through: 1 };
    let request = route(&test.request(SIGN_IN, MARK, Ask::MarkSeen { seen }));
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    test.d.mark_ended(&test.env, EndedMark { request, project: PROJECT, last_reply: 2, seen: 0 }, &mut out);
    let mut requests = Vec::new();
    while let Some(request) = out.pop() {
        requests.push(request);
    }
    assert!(requests.contains(&Request::Save { record: Stored::EndedRead { task: TASK, party: PARTY, through: 1 } }));
    assert_eq!(reply(&requests), Reply::Outcome(Outcome::MarkedSeen { seen }));
    assert_eq!(test.d.conversation_seen(PARTY, TASK), Some(0));
}
