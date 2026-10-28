use jig_core_world::effects::{World, fixture};

#[test]
fn a_running_conversations_documents_read_back_after_cold_restart() {
    const SEED: u64 = 1701;
    let mut world = World::new(SEED, false);
    world.restart_oracle(fixture(SEED, false).0);
}

#[test]
fn every_link_precedes_the_expired_sign_in_sweep() {
    use jig_domain_core as core;
    use jig_domain_people as people;
    use jig_test_domain as root;
    use skein_lib::Wall;
    const SEED: u64 = 1702;
    const PARTY: u64 = 1;
    const MISSING_TASK: u64 = 999_999;
    const EXPIRED_SIGN_IN: u64 = 999_998;
    let mut world = World::new(SEED, false);
    world.seed(Box::new([
        root::Record::Core(core::Record::People(people::Stored::SignIn {
            number: EXPIRED_SIGN_IN,
            person: PARTY,
            expires: Wall::EPOCH,
        })),
        root::Record::Core(core::Record::People(people::Stored::ReadConversation {
            party: PARTY,
            task: MISSING_TASK,
            through: 0,
        })),
    ]));
    world
        .skip_restart_oracle("the fixture deliberately omits a read-position target to prove the link precedes expiry");
    let before = world.store.rows.clone();
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| world.restart(SEED, false)))
        .expect_err("the missing target must panic before expiry can erase its sign-in");
    let message = panic
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| panic.downcast_ref::<&str>().copied())
        .expect("link diagnostic");
    assert!(message.contains("people: ReadConversation(1, 999999) links to task 999999, which is absent"), "{message}");
    assert_eq!(world.store.rows, before, "no expiry, retention or erasure precedes the complete link pass");
}
