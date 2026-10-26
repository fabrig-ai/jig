//! Store fill and admission at the high-water mark (protocol/store.md, 11.2).
use super::*;

fn filled_core(bytes: u64) -> (Core, Env<Limits>) {
    let (mut core, limits) = core();
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits };
    drop(step(&mut core, &env, Event::Fill { bytes }));
    (core, env)
}

fn signed_in(core: &mut Core, env: &Env<Limits>) {
    drop(step(
        core,
        env,
        Event::People(people::Event::SignedIn {
            held: None,
            reply_to: ReplyTo::new(Token::new(7200)),
            person: 1,
            sign_in: 10,
            identity: people::Identity {
                key: people::IdentityKey { provider: 0, subject: Box::new([1]) },
                login: b"owner".as_slice().into(),
                name: b"A Person".as_slice().into(),
            },
            kind: people::Kind::Person,
        }),
    ));
    drop(step(core, env, Event::Tasks(tasks::Event::Check)));
    drop(step(core, env, Event::Tasks(tasks::Event::Restored)));
    drop(step(core, env, Event::Period { project: 1, period: 1, budget: 1000, next: None }));
}

fn ask(core: &mut Core, env: &Env<Limits>, ask: people::Ask) -> Requests {
    step(
        core,
        env,
        Event::People(people::Event::Ask {
            models: Box::new([]),
            reply_to: ReplyTo::new(Token::new(7202)),
            sign_in: 10,
            key: [72; 16],
            ask,
        }),
    )
}

fn goal() -> people::Ask {
    let mut member = fixtures::member(b"new goal", 100);
    member.tracked = Some(1);
    people::Ask::Create { project: 1, batch: Box::new([member]) }
}

fn busy(requests: Requests) {
    let Requests::Out(mut out) = requests;
    let Some(Request::Held(held)) = out.pop() else { panic!("one busy answer") };
    let Held::PeopleReply { to, reply } = *held else { panic!("the party's answer") };
    assert_eq!(to.into_token(), Token::new(7202));
    assert_eq!(reply, people::Reply::Again(people::Again::Busy));
    let Some(Request::Decided) = out.pop() else { panic!("the decision closes") };
    assert!(out.is_empty(), "the refusal saves nothing");
}

#[test]
fn a_sign_in_at_the_fill_mark_is_refused() {
    let (mut core, env) = filled_core(limits().fill_mark);
    let identity = people::Identity {
        key: people::IdentityKey { provider: 0, subject: Box::new([2]) },
        login: b"second".as_slice().into(),
        name: b"B Person".as_slice().into(),
    };
    let Requests::Out(mut out) =
        step(&mut core, &env, Event::SignIn { held: None, reply_to: ReplyTo::new(Token::new(7201)), identity });
    let Some(Request::Now(now)) = out.pop() else { panic!("one refusal, first") };
    let Now::SignInRefused { to } = *now else { panic!("the sign-in's refusal") };
    assert_eq!(to.into_token(), Token::new(7201));
    let Some(Request::Decided) = out.pop() else { panic!("the decision closes") };
    assert!(out.is_empty(), "nothing is written");
}

#[test]
fn a_chat_at_the_fill_mark_is_answered_busy() {
    let (mut core, env) = filled_core(limits().fill_mark);
    signed_in(&mut core, &env);
    busy(ask(
        &mut core,
        &env,
        people::Ask::Create { project: 1, batch: Box::new([fixtures::member(b"new chat", 100)]) },
    ));
}

#[test]
fn a_goal_below_the_fill_mark_is_made() {
    let (mut core, env) = filled_core(limits().fill_mark - 1);
    signed_in(&mut core, &env);
    let Requests::Out(mut out) = ask(&mut core, &env, goal());
    let mut request = None;
    while let Some(next) = out.pop() {
        if let Request::Ask { ask: Ask::TaskHoldings { request: token, .. }, .. } = next {
            request = Some(token);
        }
    }
    let request = request.expect("goal asks for connector holdings");
    for connector in [1, 2] {
        drop(step(&mut core, &env, Event::Holdings { request, connector, holdings: Some(Box::new([])) }));
    }
    assert!(core.tasks.task(1).is_some(), "the goal was made below the mark");
}

#[test]
fn a_goal_at_the_fill_mark_is_answered_busy() {
    let (mut core, env) = filled_core(limits().fill_mark);
    signed_in(&mut core, &env);
    busy(ask(&mut core, &env, goal()));
}

#[test]
fn a_full_sign_in_pool_holds_its_answer_and_advances_the_deployment_numbers() {
    let (mut core, limits) = core();
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits };
    let identity = people::Identity {
        key: people::IdentityKey { provider: 0, subject: Box::new([1]) },
        login: b"owner".as_slice().into(),
        name: b"A Person".as_slice().into(),
    };
    let token = Token::new(7210);
    for _ in 0..limits.people.sign_ins {
        drop(step(
            &mut core,
            &env,
            Event::SignIn { held: None, reply_to: ReplyTo::new(token), identity: identity.clone() },
        ));
    }
    let before = core.counters.next_commit();
    assert!(!core.counters.dirty());
    let Requests::Out(mut out) =
        step(&mut core, &env, Event::SignIn { held: None, reply_to: ReplyTo::new(token), identity });
    let Some(Request::Held(held)) = out.pop() else { panic!("Full waits for durability") };
    let Held::PeopleReply { to, reply } = *held else { panic!("people reply") };
    assert_eq!(to.into_token(), token);
    assert_eq!(
        reply,
        people::Reply::Full {
            sign_ins: limits.people.sign_ins,
            earliest: Wall::from_nanos(limits.people.sign_in_lifetime.as_nanos()),
        }
    );
    let Some(Request::Decided) = out.pop() else { panic!("the decision closes") };
    assert!(out.is_empty(), "Full writes no people record");
    assert!(core.counters.dirty());
    let after = core.counters.next_commit();
    assert_eq!(after.people, before.people + 1);
    assert_eq!(after.sign_ins, before.sign_ins + 1);
    assert_eq!(after.commits, before.commits + 1);
}
