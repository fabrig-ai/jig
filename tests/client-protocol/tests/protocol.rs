use jig_client_protocol_world::{fixtures, protocol};
use jig_client_world::{Until, stories};
#[test]
fn a_person_signs_in_by_the_printed_link_and_starts_a_chat() {
    let mut w = protocol::world(fixtures::SEED);
    let c = w.client(fixtures::FIRST_OWNER, stories::conversation());
    w.person(c, fixtures::sign_in_then(stories::start_chat_steps()));
    let outcome = w.run(Until::Done);
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed);
    assert_eq!(w.front().chats_made(), 1);
}

#[test]
fn a_person_s_part_request_is_answered_from_the_connector_s_outcome() {
    use skein_fake_person::{Find, Step};
    use skein_ui::Role;
    let mut w = fixtures::family_world(fixtures::SEED);
    let c = w.client(fixtures::FIRST_OWNER, stories::conversation());
    let parameters = Find::named(Role::TextField, b"Parameters");
    w.person(
        c,
        fixtures::sign_in_then(vec![
            Step::See { find: Find::text(b"Signed in as Person"), within: skein_lib::Duration::from_secs(1) },
            Step::Go {
                address: jig_client_test_domain::Address::First(jig_client_test_domain::first::Address {
                    project: 1,
                    item: 1,
                }),
            },
            Step::See { find: parameters.clone(), within: skein_lib::Duration::from_secs(1) },
            Step::Type { find: parameters, words: b"42".as_slice().into() },
            Step::Press { find: Find::named(Role::Button, b"Apply parameters") },
            Step::See { find: Find::text(b"Object changed"), within: skein_lib::Duration::from_secs(1) },
            Step::See { find: Find::text(b"Value: 42"), within: skein_lib::Duration::from_secs(1) },
        ]),
    );
    let outcome = w.run(Until::Done);
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed);
    assert_eq!(
        w.front().application_objects(),
        vec![(1, jig_test_connector::ApplicationObject { revision: 1, value: 42 })]
    );
}

#[test]
fn a_family_place_record_round_trips_through_its_owner_s_small_store_bytes() {
    use jig_client_test_domain::first::{Address, Record};
    for (label, value) in [
        ("ordinary", Record::Place { version: 1, party: 1, address: Address { project: 2, item: 3 } }),
        (
            "maximum names",
            Record::Place { version: 1, party: u64::MAX, address: Address { project: u32::MAX, item: u64::MAX } },
        ),
    ] {
        assert_eq!(
            jig_client_protocol_world::family::restored(&jig_client_protocol_world::family::saved(&value)),
            Some(value),
            "{label}"
        );
    }
}

#[test]
fn a_request_whose_answer_was_lost_is_made_once() {
    use jig_client_protocol_world::stories as bytes;
    for &seed in bytes::SEEDS {
        let (mut world, client) = bytes::ready(seed);
        pending_stop(&mut world, client);
        bytes::held(&mut world, client);
        assert_eq!(world.front().requests_made(), 2, "lost answer seed {seed}");
        assert!(world.front().cuts() > 0, "cut seed {seed}");
    }
}

#[test]
fn a_slow_reader_gets_fewer_deliveries_and_others_are_answered() {
    use jig_client_protocol_world::stories as bytes;
    use jig_client_world::FrontFault;
    use skein_lib::Duration;
    let (mut world, first) = bytes::two_clients(fixtures::SEED);
    let second = world.client(
        1,
        jig_client_test_domain::Address::Jig(jig_client_view::address::Address::Task {
            task: 1,
            section: jig_client_view::address::TaskSection::Activity,
        }),
    );
    world.person(
        second,
        fixtures::sign_in_then(vec![skein_fake_person::Step::See {
            find: skein_fake_person::Find::exactly_named(skein_ui::Role::Button, b"Write"),
            within: Duration::from_secs(1),
        }]),
    );
    bytes::finish(&mut world);
    let now = world.now();
    world.front().select_stream(first);
    world.fault(FrontFault::Slow { span: Duration::from_secs(1) });
    let before = world.front().deliveries(first);
    let answered = world.front().answers(second);
    let second_before = world.front().deliveries(second);
    bytes::send_words(&mut world, second, b"Please continue", false);
    bytes::finish(&mut world);
    assert_eq!(world.front().deliveries(first), before);
    assert!(world.now() < now.saturating_add(Duration::from_secs(1)));
    assert!(world.front().words_reached_worker(b"Please continue"));
    assert!(world.front().deliveries(second) > second_before);
    world.front().check_isolation(first, second, answered);
    assert_eq!(world.front().rules().isolated, 1);
}

#[test]
fn a_reload_restores_a_pending_request_from_the_small_store_s_bytes() {
    use jig_client_protocol_world::stories as bytes;
    for &seed in bytes::SEEDS {
        let (mut world, client) = bytes::ready(seed);
        pending_stop(&mut world, client);
        assert!(world.front().pending_bytes(client) > 0, "pending seed {seed}");
        world.reload(client);
        bytes::held(&mut world, client);
        assert_eq!(world.front().chats_made(), 1, "reload seed {seed}");
    }
}

#[test]
fn signed_out_with_requests_pending_then_signed_in_again() {
    use jig_client_protocol_world::stories as bytes;
    use skein_fake_person::{Find, Step};
    for &seed in bytes::SEEDS {
        let (mut world, client) = bytes::ready(seed);
        pending_stop(&mut world, client);
        world.fault(jig_client_world::FrontFault::SignedOut);
        world.person(
            client,
            vec![
                Step::See {
                    find: Find::named(skein_ui::Role::Button, b"Sign in with Test"),
                    within: skein_lib::Duration::from_secs(1),
                },
                Step::Press { find: Find::named(skein_ui::Role::Button, b"Sign in with Test") },
                Step::See { find: Find::text(b"Held by a placed hold"), within: skein_lib::Duration::from_secs(5) },
            ],
        );
        bytes::finish(&mut world);
        assert_eq!(world.front().chats_made(), 1, "signed out seed {seed}");
    }
}

#[test]
fn a_person_s_words_reach_a_running_run_through_bytes() {
    use jig_client_protocol_world::stories as bytes;
    for &seed in bytes::SEEDS {
        let (mut world, client) = bytes::ready(seed);
        bytes::send_words(&mut world, client, b"Please continue", false);
        bytes::finish(&mut world);
        assert!(world.front().words_reached_worker(b"Please continue"), "worker seed {seed}");
    }
}

#[test]
fn an_engine_outside_the_client_s_versions_shows_out_of_date() {
    use jig_client_protocol_world::stories as bytes;
    use skein_fake_person::{Find, Step};
    for &seed in bytes::SEEDS {
        let (mut world, client) = bytes::ready(seed);
        world.fault(jig_client_world::FrontFault::Versions);
        world.person(
            client,
            vec![Step::See { find: Find::text(b"Out of date"), within: skein_lib::Duration::from_secs(2) }],
        );
        bytes::finish(&mut world);
        assert_eq!(world.front().chats_made(), 1, "versions seed {seed}");
    }
}

#[test]
fn actual_busy_not_ready_restart_and_dropped_watch_retry_the_same_request() {
    use jig_client_protocol_world::stories as bytes;
    use jig_client_world::FrontFault;
    use skein_lib::Duration;
    for (label, fault) in [
        ("busy", FrontFault::Busy),
        ("not ready", FrontFault::NotReady),
        ("restart", FrontFault::Restart { offline: Duration::from_millis(100) }),
        ("dropped watch", FrontFault::DropWatch),
    ] {
        eprintln!("fault case {label}");
        if matches!(&fault, FrontFault::Busy | FrontFault::NotReady) {
            bytes::transient_request(fixtures::SEED, &fault);
            continue;
        }
        let (mut world, client) = bytes::ready(fixtures::SEED);
        pending_stop(&mut world, client);
        world.fault(fault);
        bytes::held(&mut world, client);
        assert_eq!(world.front().chats_made(), 1, "{label}");
    }
}

fn pending_stop(world: &mut jig_client_protocol_world::stories::Story, client: jig_client_world::ClientId) {
    world.fault(jig_client_world::FrontFault::NoAnswer);
    jig_client_protocol_world::stories::submit_stop(world, client);
    assert!(
        world.kept(client).iter().any(|record| matches!(record, jig_client_test_domain::store::Record::Client(..))),
        "the request remains in the small store"
    );
}

#[test]
fn words_to_a_stopped_task_release_it_in_one_compound_request() {
    use jig_client_protocol_world::stories as bytes;
    for &seed in bytes::SEEDS {
        let (mut world, client) = bytes::ready(seed);
        bytes::submit_stop(&mut world, client);
        bytes::held(&mut world, client);
        let before = world.front().requests_made();
        let runs = world.front().runs_started();
        bytes::send_words(&mut world, client, b"Please resume", true);
        bytes::finish(&mut world);
        assert_eq!(world.front().requests_made() - before, 1, "one compound seed {seed}");
        bytes::settle(&mut world, skein_lib::Duration::from_millis(100));
        assert_eq!(world.front().runs_started() - runs, 1, "resumed worker seed {seed}");
    }
}

const EFFECT_WAIT: skein_lib::Duration = skein_lib::Duration::from_secs(1);
#[test]
fn cancelling_a_task_says_what_ends_before_it_is_sent() {
    use jig_client_world::{Settings, World};
    use skein_fake_person::{Find, Step};
    use skein_ui::Role;
    let mut world = World::new(
        Settings { seed: fixtures::SEED, ..Settings::default() },
        jig_client_protocol_world::Protocol::two_run_slots(fixtures::SEED),
    );
    let client = world.client(fixtures::FIRST_OWNER, stories::conversation());
    world.person(client, jig_client_protocol_world::stories::start());
    jig_client_protocol_world::stories::finish(&mut world);
    world.front().make_live_delegate();
    let before = world.front().requests_made();
    world.person(
        client,
        vec![
            Step::See { find: Find::exactly_named(Role::Button, b"Cancel"), within: EFFECT_WAIT },
            Step::Press { find: Find::exactly_named(Role::Button, b"Cancel") },
            Step::See { find: Find::text(b"1 running tasks"), within: EFFECT_WAIT },
            Step::See { find: Find::text(b"2 live runs"), within: EFFECT_WAIT },
            Step::See {
                find: Find::text(b"Effects not yet begun will be withdrawn; begun effects will settle"),
                within: EFFECT_WAIT,
            },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, skein_world::domain::Verdict::Passed, "{outcome:?}");
    assert_eq!(world.front().requests_made(), before);
}

#[path = "client/conversations.rs"]
mod conversations;
#[path = "client/goals.rs"]
mod goals;
