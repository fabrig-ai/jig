use jig_store_world::{World, referee::Referee, root};
use skein_fake_machine::Item;

fn whole(world: &mut World) {
    assert_eq!(world.active(), b"1\n");
    let paths = world.paths();
    let generations: Vec<_> =
        paths.iter().filter(|p| p.starts_with(b"generations/") && skein_lib::bytes::count(p, b"/", 2) == 1).collect();
    assert_eq!(generations, [b"generations/1".as_slice()]);
    assert!(!paths.iter().any(|p| p.starts_with(b"active.skein-")));
}
#[test]
fn a_first_start_cut_anywhere_is_whole_at_the_next_start() {
    let mut completed = World::unopened(18);
    assert_eq!(completed.open_switched(), Some(0));
    let operations = completed.operation_count();
    for cut in 0..=operations {
        let mut world = World::unopened(u64::from(cut));
        world.cut_at(cut);
        world.open_switched();
        world.crash(u64::from(cut));
        let restored = world.reopen();
        assert_eq!(restored.last, 0, "empty recovered generation, cut {cut}");
        whole(&mut world);
        assert_eq!(world.commit(root::first()), Some(1));
        assert_eq!(world.commit(root::plain_commit(2)), Some(2));
        world.crash(u64::from(cut));
        let restored = world.reopen();
        Referee::check(&world.model, &restored).unwrap_or_else(|e| panic!("first-start cut {cut}: {e:?}"));
    }
}
fn strays(world: &mut World) {
    world.lay(&[
        Item::directory(b"generations/2"),
        Item::directory(b"generations/2/store"),
        Item::file(b"generations/2/store/row", b"stray"),
        Item::directory(b"generations/2/secrets").mode(0o700),
        Item::file(b"generations/2/secrets/key", b"private stray"),
        Item::directory(b"generations/dropped"),
        Item::directory(b"generations/dropped/store"),
        Item::file(b"generations/dropped/store/row", b"stray"),
        Item::file(b"active.skein-0123456789abcdef", b"temporary"),
    ]);
}
fn committed(seed: u64) -> World {
    let mut world = World::new(seed);
    assert_eq!(world.commit(root::first()), Some(1));
    strays(&mut world);
    world.crash(seed);
    world
}
#[test]
fn strays_cut_anywhere_are_gone_at_the_next_start() {
    let mut completed = committed(19);
    completed.reopen();
    let operations = completed.operation_count();
    for cut in 0..=operations {
        let mut world = committed(u64::from(cut));
        world.cut_at(cut);
        world.open_switched();
        world.crash(u64::from(cut));
        let restored = world.reopen();
        whole(&mut world);
        Referee::check(&world.model, &restored).unwrap_or_else(|e| panic!("stray-removal cut {cut}: {e:?}"));
    }
}
