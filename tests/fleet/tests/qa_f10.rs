use jig_fleet_world::{Settings, World};
#[test]
fn ownership_and_capacity_pressure_leave_no_unsettled_fleet_work() {
    for seed in [3, 11, 23] {
        let mut settings = Settings::calm(seed);
        settings.limits.calls = 1;
        let mut world = World::new(settings);
        world.run(400_000);
        let stats = world.stats();
        assert!(stats.capacity_refusals > 0, "seed {seed}: capacity actually reached");
        assert!(stats.calls > 0 && stats.replies > 0, "seed {seed}: {stats:?}");
    }
}
