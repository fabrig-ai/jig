use jig_engine_workers_world::fixtures;
use jig_protocol_engine_workers::Event;
use skein_lib::Token;

#[test]
fn every_connection_holds_frames_at_its_cap() {
    use jig_engine_workers_world::World;
    let limits = fixtures::limits();
    let cap = jig_protocol_worker_wire::sizes::engine_cap(&limits.sizes).expect("cap");
    let mut world = World::new(&limits, &fixtures::workers(), 1);
    let peers = [(fixtures::WORKER_A, fixtures::SECRET_A), (fixtures::WORKER_B, fixtures::SECRET_B)];
    for (name, secret) in peers {
        let (connection, channel) = world.reporting(name, secret);
        world.withhold_write_room(connection);
        for _ in 0..cap.frames {
            world.down(Event::TurnBusy { channel, run: Token::new(7), attempt: Token::new(2), turn: 3 });
        }
        assert!(!world.gone(connection), "every frame fits the cap");
    }
    world.connect(fixtures::worker(fixtures::WORKER_C, fixtures::SECRET_C, fixtures::PAYLOAD));
}
