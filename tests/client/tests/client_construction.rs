use jig_client_world::{Settings, Until, stories, test};
use skein_fake_person::{Find, Step};
use skein_lib::Duration;
use skein_world::domain::Verdict;
use stories::client_construction as fixture;

#[test]
fn supplied_client_bounds_and_constructor_survive_tabs_reload_and_default_neighbours() {
    let settings = Settings { seed: 8530, ..Settings::default() };
    assert!(fixture::NAME.len() > usize::try_from(test::limits().jig.text).expect("default text bound fits"));
    let mut world = fixture::world(settings);
    let custom = world.client_with(7, stories::chat(), fixture::limits(), fixture::client);
    let default = world.client(8, stories::chat());
    let custom_tab = world.tab(custom);
    let default_tab = world.tab(default);
    let within = Duration::from_secs(1);
    for (client, name) in [
        (custom, fixture::NAME),
        (custom_tab, fixture::NAME),
        (default, b"Signed in as Pat".as_slice()),
        (default_tab, b"Signed in as Pat".as_slice()),
    ] {
        world.person(
            client,
            vec![Step::See { find: Find::text(name), within }, Step::See { find: Find::text(b"Chat nine"), within }],
        );
    }
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert!(outcome.judged.0 > 0);
    for client in [custom, custom_tab, default, default_tab] {
        world.reload(client);
    }
    let after_reload = world.tab(custom_tab);
    for (client, name) in [
        (custom, fixture::NAME),
        (custom_tab, fixture::NAME),
        (after_reload, fixture::NAME),
        (default, b"Signed in as Pat".as_slice()),
        (default_tab, b"Signed in as Pat".as_slice()),
    ] {
        world.person(
            client,
            vec![Step::See { find: Find::text(name), within }, Step::See { find: Find::text(b"Chat nine"), within }],
        );
    }
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert!(outcome.judged.0 > 0);
}

#[test]
fn the_default_constructor_keeps_its_original_receiving_refusal_for_a_wide_frame() {
    let mut world = fixture::world(Settings { seed: 8531, ..Settings::default() });
    let client = world.client(7, stories::inbox());
    let within = Duration::from_secs(1);
    world.person(
        client,
        vec![Step::Pause { span: Duration::from_millis(50) }, Step::See { find: Find::text(b"Connecting"), within }],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed);
    assert!(!world.tree(client).nodes().iter().any(|node| node.name.as_deref() == Some(fixture::NAME)));
    world.reload(client);
    let tab = world.tab(client);
    for client in [client, tab] {
        world.person(
            client,
            vec![
                Step::Pause { span: Duration::from_millis(50) },
                Step::See { find: Find::text(b"Connecting"), within },
            ],
        );
    }
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed);
}
