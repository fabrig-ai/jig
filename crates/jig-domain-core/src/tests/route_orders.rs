//! Carried ownership precedes the whole admission output (dynamic-memory.md, 4.6).
use super::*;

const REPLY: Token = Token::new(9705);
const VALUE: u64 = 1_u64 << 63;
const OWNER: Token = Token::new(9706);
const KEY: [u8; 16] = [75; 16];
const CONNECTOR: u16 = 1;
const ABSENT_PROJECT: u32 = 2;
// An unused application admission can erase an expired answer, save its
// replacement, answer, and drop the owner's input, then end the decision.
const OUTPUTS: u32 = 6;

/// A carried value the core leaves unused is handed back before every
/// other output of its route, the answer included, however many outputs
/// the route makes.
#[test]
fn an_unused_carried_value_leaves_before_the_rest_of_a_wide_route() {
    let (mut core, mut limits) = core();
    // Keep the sign-in alive beyond the retained answer's expiry.
    limits.people.sign_in_lifetime = Duration::from_secs(600);
    let mut env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits };
    let identity = core.people.identity(1).expect("fixture owner").clone();
    let Requests::Out(mut signed) =
        step(&mut core, &env, Event::SignIn { reply_to: ReplyTo::new(REPLY), identity, held: None });
    let mut sign_in = None;
    while let Some(request) = signed.pop() {
        if let Request::Held(held) = request
            && let Held::PeopleReply { reply: people::Reply::SignedIn { sign_in: number, .. }, .. } = *held
        {
            sign_in = Some(number);
        }
    }
    let sign_in = sign_in.expect("signed-in owner");
    let _expired = step(&mut core, &env, application(sign_in, None));
    let after_retention = Duration::from_secs(121);
    env.now = Time::ZERO.checked_add(after_retention).expect("story time");
    env.wall = Wall::from_nanos(after_retention.as_nanos());
    let Requests::Out(mut out) =
        step(&mut core, &env, application(sign_in, Some(Carried { number: VALUE, digest: [7; 32] })));
    let mut outputs = List::with_capacity(out.len());
    for _ in 0..out.len() {
        outputs.push(out.pop().expect("route output count")).expect("one place per output");
    }
    assert_eq!(outputs.len(), OUTPUTS);
    let [
        Request::Held(unused),
        Request::Write(Write::Erase(Key::People(people::Key::Answer(erased)))),
        Request::Write(Write::Save(Record::People(people::Stored::Answer { key, outcome, .. }))),
        Request::Held(answer),
        Request::Ask { connector: CONNECTOR, ask: Ask::DropApplication { owner: OWNER } },
        Request::Decided,
    ] = outputs.as_slice()
    else {
        panic!("unused, erase, save, answer, owner drop, decision: {outputs:?}")
    };
    let Held::CarriedUnused { number: VALUE } = unused.as_ref() else { panic!("unused carried value is first") };
    assert_eq!(*erased, people::RequestKey { person: 1, key: KEY });
    assert_eq!(key, erased);
    let Held::PeopleReply { to, reply: people::Reply::Outcome(reply) } = answer.as_ref() else {
        panic!("one keyed answer")
    };
    assert_eq!(*to, ReplyTo::new(REPLY));
    assert_eq!(reply, outcome);
    assert_eq!(*outcome, people::Outcome::Refused(people::Refusal::Role { held: None }));
}

fn application(sign_in: u64, carried: Option<Carried>) -> Event {
    Event::Application {
        reply_to: ReplyTo::new(REPLY),
        sign_in,
        key: KEY,
        scope: people::ApplicationScope::Project(ABSENT_PROJECT),
        connector: CONNECTOR,
        digest: [7; 32],
        owner: OWNER,
        carried,
    }
}
