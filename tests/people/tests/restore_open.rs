use jig_domain_people::{
    self as people, Ask, Domain, Event, Key, LimitKind, Outcome, Refusal, Reply, Request, RequestKey, Stored,
    TaskExecutor, fire, max_out, step,
};
use jig_people_world::{LIMITS, fixtures::people_member, identity};
use skein_lib::{Duration, Env, Queue, ReplyTo, Time, Token, Wall};

const PERSON: u64 = 1;
const SIGN_IN: u64 = 10;
const PROJECT: u32 = 1;
const TASK: u64 = 20;
const PROPOSAL: u64 = 30;
const KEY: RequestKey = RequestKey { person: PERSON, key: [7; 16] };
const WORDS: &[u8] = b"open request";

fn restoring() -> (Domain, Env<people::Limits>, Queue<Request>) {
    (
        Domain::new(&LIMITS, Box::new([]), 1, Box::new([])),
        Env { limits: LIMITS, now: Time::ZERO, wall: Wall::EPOCH },
        Queue::with_capacity(max_out(&LIMITS)),
    )
}

fn answers(executor: TaskExecutor, conversation: bool) -> [(Ask, Outcome); 2] {
    let mut member = people_member(WORDS, 1);
    member.executor = executor;
    member.flags.ended_by = people::EndedBy::Requester;
    member.flags.conversation = conversation;
    [
        (
            Ask::Create { project: PROJECT, batch: Box::new([member.clone()]) },
            Outcome::Created { first: TASK, last: TASK, given: None },
        ),
        (
            Ask::Propose {
                project: PROJECT,
                action: people::Action::Batch(Box::new([member])),
                reason: b"proposed work".as_slice().into(),
            },
            Outcome::Proposed { proposal: PROPOSAL, to: 0 },
        ),
    ]
}

fn answer(ask: Ask, outcome: Outcome) -> Stored {
    Stored::Answer { key: KEY, ask: Box::new(ask), models: Box::new([]), outcome, at: Wall::EPOCH }
}

#[test]
fn restoring_an_answer_refuses_malformed_open_members() {
    let rows = [
        ("person", TaskExecutor::Person(people::PersonAddress::Person(PERSON)), true),
        ("procedure", TaskExecutor::Procedure { connector: 1, code: 1 }, true),
        ("no conversation", TaskExecutor::Agent { charter: 1 }, false),
    ];
    for (name, executor, conversation) in rows {
        for (ask, outcome) in answers(executor, conversation) {
            let (mut domain, env, mut out) = restoring();
            step(&mut domain, &env, Event::Restore { record: answer(ask, outcome) }, &mut out);
            step(&mut domain, &env, Event::Restored, &mut out);
            assert!(!domain.ready(), "{name}");
            assert_eq!(domain.next_deadline(), None, "no answer was retained: {name}");
            assert_eq!(out.len(), 1, "{name}");
            assert_eq!(
                out.pop(),
                Some(Request::RestoreRefused {
                    key: Key::Answer(KEY),
                    refusal: Refusal::Limit { which: LimitKind::Restore, bound: None },
                }),
                "{name}"
            );
        }
    }
}

#[test]
fn a_restored_open_answer_replays_its_batch_once() {
    for (ask, outcome) in answers(TaskExecutor::Agent { charter: 1 }, true) {
        let (mut domain, mut env, mut out) = restoring();
        let records = [
            answer(ask.clone(), outcome.clone()),
            Stored::Person { number: PERSON, identity: identity(PERSON) },
            Stored::SignIn {
                number: SIGN_IN,
                person: PERSON,
                expires: Wall::from_nanos(Duration::from_secs(3600).as_nanos()),
            },
        ];
        for record in records {
            step(&mut domain, &env, Event::Restore { record }, &mut out);
        }
        step(&mut domain, &env, Event::Restored, &mut out);
        assert!(domain.ready());
        assert!(out.is_empty());
        assert_eq!(domain.identity(PERSON), Some(&identity(PERSON)));
        let token = Token::new(1);
        step(
            &mut domain,
            &env,
            Event::Ask { reply_to: ReplyTo::new(token), sign_in: SIGN_IN, key: KEY.key, ask, models: Box::new([]) },
            &mut out,
        );
        assert_eq!(out.len(), 1, "replay routes and saves nothing");
        assert_eq!(out.pop(), Some(Request::Reply { to: ReplyTo::new(token), reply: Reply::Outcome(outcome) }));
        env.now = Time::from_nanos(LIMITS.request_retention.as_nanos());
        env.wall = Wall::from_nanos(LIMITS.request_retention.as_nanos());
        fire(&mut domain, &env, &mut out);
        assert_eq!(out.len(), 1, "one retained answer expires");
        assert_eq!(out.pop(), Some(Request::Erase { key: Key::Answer(KEY) }));
    }
}

#[test]
fn acted_rows_restore_without_a_person_or_grant_and_expire() {
    let (mut domain, mut env, mut out) = restoring();
    let key = people::ActKey { connector: 1, key: KEY.key };
    let act = people::Act::Close { project: PROJECT, task: TASK, reason: Box::new([]), grace: None };
    let outcome = Outcome::Compound { project: PROJECT, operations: 1, first_message: 1 };
    step(
        &mut domain,
        &env,
        Event::Restore { record: Stored::Acted { key, act: act.clone(), outcome: outcome.clone(), at: Wall::EPOCH } },
        &mut out,
    );
    step(&mut domain, &env, Event::Restored, &mut out);
    assert!(domain.ready());
    let to = Token::new(1);
    step(
        &mut domain,
        &env,
        Event::Act { reply_to: ReplyTo::new(to), connector: key.connector, key: key.key, act: Box::new(act.clone()) },
        &mut out,
    );
    assert_eq!(out.pop(), Some(Request::Acted { to: ReplyTo::new(to), outcome }));
    env.now = env.now.saturating_add(LIMITS.request_retention);
    fire(&mut domain, &env, &mut out);
    assert_eq!(out.pop(), Some(Request::Erase { key: Key::Acted(key) }));
    step(
        &mut domain,
        &env,
        Event::Act { reply_to: ReplyTo::new(to), connector: key.connector, key: key.key, act: Box::new(act) },
        &mut out,
    );
    assert_eq!(
        out.pop(),
        Some(Request::Acted { to: ReplyTo::new(to), outcome: Outcome::Refused(Refusal::NotOffered) })
    );
}
#[test]
fn not_ready_acts_are_not_saved() {
    let (mut domain, env, mut out) = restoring();
    let to = Token::new(1);
    step(
        &mut domain,
        &env,
        Event::Act {
            reply_to: ReplyTo::new(to),
            connector: 1,
            key: KEY.key,
            act: Box::new(people::Act::RemoveAccount { account: 1 }),
        },
        &mut out,
    );
    assert_eq!(
        out.pop(),
        Some(Request::Acted { to: ReplyTo::new(to), outcome: Outcome::Again(people::Again::NotReady) })
    );
    assert!(out.is_empty());
}
