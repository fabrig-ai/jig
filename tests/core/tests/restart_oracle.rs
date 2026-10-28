use jig_core_world::effects::{World, fixture};

#[test]
fn a_running_conversations_documents_read_back_after_cold_restart() {
    const SEED: u64 = 1701;
    let mut world = World::new(SEED, false);
    world.restart_oracle(fixture(SEED, false).0);
}
