use super::super::fixture::see;
use super::*;
use jig_client_world::harness::Front;
use skein_fake_person::{Find, Step};
use skein_lib::Duration;
use skein_ui::Role;
type World = jig_client_world::World<super::super::fixture::Client, Joint>;

fn linked_conversation(seed: u64) -> (World, ClientId) {
    let mut world = super::super::fixture::world(seed);
    let client = world.client_with(
        1,
        d::Address::Jig(jig_client_view::address::Address::Conversations { project: None }),
        super::super::fixture::limits(),
        super::super::fixture::client,
    );
    super::super::fixture::watch_setup(world.front());
    world.person(
        client,
        vec![
            Step::See { find: Find::named(Role::TextArea, b"First words"), within: Duration::from_secs(3) },
            Step::Type { find: Find::named(Role::TextArea, b"First words"), words: b"@watch_setup".as_slice().into() },
            Step::Press { find: Find::named(Role::Button, b"Start a conversation") },
            see(b"delegate"),
        ],
    );
    let outcome = world.run(jig_client_world::Until::Done);
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed, "{outcome:?}");
    (world, client)
}
#[test]
fn a_linked_ops_root_starts_on_the_common_world_clock() {
    let (mut world, _) = linked_conversation(511);
    super::super::fixture::alert(world.front());
    let due = Time::from_nanos(world.front().engine.clock().now);
    let outcome = world.run(jig_client_world::Until::Time(due.saturating_add(Duration::from_secs(1))));
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed, "{outcome:?}");
    assert!(outcome.now >= due);
}

#[test]
fn a_linked_ops_root_signs_in_through_the_byte_face() {
    let mut world = super::super::fixture::world(511);
    let joint = world.front();
    joint.bind(ClientId(0), 1);
    let limits = super::super::fixture::limits();
    let mut root = d::new(&limits, 71);
    let mut requests = Queue::uncapped();
    let mut env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits };
    d::step(
        &mut root,
        &env,
        d::Event::Start {
            address: d::Address::Jig(jig_client_view::address::Address::Conversations { project: None }),
            kept: skein_lib::List::with_capacity(0),
            offset: jig_client_view::boundary::Offset { minutes: 0 },
        },
        &mut requests,
    );
    joint.starting(ClientId(0));
    for _ in 0..300 {
        while let Some(request) = requests.pop() {
            if let d::Request::Client { request } = request {
                joint.wire(ClientId(0), env.now, env.wall, d::Request::Client { request });
            }
        }
        for (_, event) in joint.poll(env.now, env.wall) {
            d::step(&mut root, &env, event, &mut requests);
        }
        if root.client().link().state == jig_client::Link::Live {
            return;
        }
        env.now = env.now.saturating_add(skein_lib::Duration::from_millis(10));
        d::fire(&mut root, &env, &mut requests);
    }
    panic!("not live: {:?}", root.client().link());
}
