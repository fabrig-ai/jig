//! The installation's identifier across crashes (store.md 4.4 item 9;
//! configuration.md 8): an empty store's first commit keeps the identifier
//! the shell drew, and every later start restores it instead of its own draw.
use jig_store_world::{World, root};

const FIRST: [u8; 16] = [1; 16];
const SECOND: [u8; 16] = [2; 16];
const THIRD: [u8; 16] = [3; 16];

/// A world over an empty store, opened with `FIRST` drawn.
fn empty(seed: u64) -> World {
    let world = World::new(seed);
    assert_eq!(world.installation, world.drawn, "an empty store answers this start's draw");
    assert_eq!(world.installation, FIRST);
    world
}

fn first_commit_operations() -> u32 {
    let mut world = empty(18);
    assert_eq!(world.commit(root::first()), Some(1));
    world.operation_count()
}

#[test]
fn the_identifier_survives_a_crash_at_the_first_commit_and_is_never_redrawn_once_kept() {
    let count = first_commit_operations();
    assert!(count > 0, "the first commit writes");
    for cut in 0..=count {
        let mut world = empty(u64::from(cut));
        world.cut_at(cut);
        let acknowledged = world.commit(root::first());
        world.crash(u64::from(cut));
        world.drawn = SECOND;
        let restored = world.reopen();
        let expected = if restored.last == 1 { FIRST } else { SECOND };
        assert_eq!(world.installation, expected, "kept with the first commit or not at all, cut {cut}");
        assert!(acknowledged.is_none() || restored.last == 1, "an acknowledged first commit is durable, cut {cut}");
        assert!(cut > 0 || restored.last == 0, "nothing kept before the first operation");
        assert!(cut < count || restored.last == 1, "kept once every operation is done, cut {cut}");
        if restored.last == 1 {
            world.accept_recovery(1);
        } else {
            assert_eq!(world.commit(root::first()), Some(1), "this start keeps its own draw, cut {cut}");
        }
        let kept = world.installation;
        world.crash(u64::from(cut).wrapping_add(100));
        world.drawn = THIRD;
        let restored = world.reopen();
        assert_eq!(restored.last, 1);
        assert_eq!(world.installation, kept, "a later start restores the kept identifier, cut {cut}");
    }
}

#[test]
fn every_first_commit_crash_keeps_the_three_creation_rows_or_none() {
    use jig_protocol_store::{codec, format};
    use skein_lib::{Env, Queue, Time, Token, Wall};
    let count = first_commit_operations();
    for cut in 0..=count {
        let mut world = empty(u64::from(cut));
        world.cut_at(cut);
        let _acknowledged = world.commit(root::first());
        world.crash(u64::from(cut));
        let restored = world.reopen();
        let mut stores = world.into_stores();
        let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: root::limits().store };
        let expected = [
            (
                Box::from([0, 0]),
                format::current(root::APPLICATION.version, root::DEPLOYMENT).encode().expect("format fits"),
            ),
            (Box::from([0, 3]), format::layouts().encode()),
            (Box::from([1, 4, 3]), codec::accounts::Installation { id: FIRST }.encode()),
        ];
        for (key, value) in expected {
            let mut above = Queue::uncapped();
            let mut below = Queue::uncapped();
            skein_kv::down(
                &mut stores.store,
                &env,
                skein_kv::Request::Get { owner: Token::new(0), key },
                &mut above,
                &mut below,
            );
            let Some(skein_kv::Event::Got { value: found, .. }) = above.pop() else { panic!("memory get") };
            assert_eq!(
                found,
                if restored.last == 0 { None } else { Some(value) },
                "creation rows are atomic, cut {cut}"
            );
            assert!(below.is_empty());
        }
    }
}
