//! Bounded world storage keeps its own checked envelope (testing.md, section 6).
use crate::Limits;
use jig_client_view::Owner;
use skein_lib::{Duration, Token};
use skein_ui::{Builder, Role, SlotRef};

fn world_domain() -> jig_client_test_domain::Limits {
    // The focused world's profile, without changing its fixture or timers.
    let mut domain = super::limits();
    domain.jig.document_bytes = 65536;
    domain.jig.save_delay = Duration::from_millis(10);
    domain.client.domain.backoff.first = Duration::from_millis(10);
    domain.client.domain.backoff.most = Duration::from_millis(80);
    domain.client.domain.heartbeat = Duration::from_millis(100);
    domain.first.retry = Duration::from_millis(10);
    domain.client.domain.requests.first = Duration::from_millis(10);
    domain.client.domain.requests.most = Duration::from_millis(80);
    domain
}

#[test]
fn the_worlds_original_storage_counts_construct_a_finite_view() {
    let domain = world_domain();
    let jig = super::words();
    let first = super::family_words();
    let native = jig_client_view::limits::of_words(&domain.jig, jig).unwrap();
    let limits = Limits::for_world(&domain, jig, first, 1024, 256).unwrap();
    assert_eq!(limits.tree.nodes, 1024);
    assert_eq!(limits.tree.bindings, 1024);
    assert_eq!(limits.tree.slots, 256);
    assert_eq!(limits.tree.patches, 5120);
    assert!(limits.tree.text_per_node >= native.text_per_node);
    assert!(limits.tree.text_bytes < 1024 * limits.tree.text_per_node);
    assert!(crate::valid(&limits, &domain, jig, first));
    let view =
        crate::View::new(&limits, &domain, jig, first, jig_client_test_domain::first::Address { project: 3, item: 5 });
    assert_eq!(view.limits(), &limits);
    let mut builder = Builder::new(&limits.tree);
    builder.open(Role::Page);
    for _ in 0_u32..256_u32 {
        builder.slot(
            SlotRef {
                owner: (Owner::Application { number: jig_client_test_domain::FIRST }).opaque(),
                token: Token::new(1),
            },
            b"loading",
        );
    }
    for _ in 0_u32..511_u32 {
        builder.open(Role::Text);
        builder.close();
    }
    builder.close();
    assert_eq!(builder.finish().len(), 1024);
}

#[test]
fn bounded_world_storage_survives_complete_composition_text_overflow() {
    let mut domain = world_domain();
    domain.first.rows = 64;
    let jig = super::words();
    let first = super::family_words();
    let native = jig_client_view::limits::of_words(&domain.jig, jig).unwrap();
    let owner = crate::connector_values::requirements(&domain.first.connector, jig).unwrap();
    let fragment = u64::from(crate::family::PART_NODES.max(owner.nodes + 16).max(domain.first.rows + 2));
    let cards = u64::from(domain.first.rows + 1);
    let jig_cards = u64::from(jig_client_view::card_nodes(&domain.jig).unwrap()) * cards;
    let slots = (u64::from(native.nodes) + jig_cards) * fragment + jig_cards + u64::from(crate::family::NAV_NODES);
    let application = fragment + cards * 3 + 3;
    let base = jig_client_view::limits::compose(
        &domain.jig,
        jig,
        &jig_client_view::page::Arrangement::none(),
        u32::try_from(application).unwrap(),
        0,
        owner.depth + 4,
    )
    .unwrap();
    let full_nodes = u64::from(base.nodes) + slots;
    u32::try_from(full_nodes).unwrap();
    assert!(full_nodes * u64::from(base.text_per_node) > u64::from(u32::MAX));
    assert!(
        jig_client_view::limits::compose(
            &domain.jig,
            jig,
            &jig_client_view::page::Arrangement::none(),
            u32::try_from(application).unwrap(),
            u32::try_from(slots).unwrap(),
            owner.depth + 4,
        )
        .is_none()
    );
    assert!(Limits::of(&domain, jig, first).is_none());
    let limits = Limits::for_world(&domain, jig, first, 1024, 256).unwrap();
    assert_eq!(limits.tree.nodes, 1024);
    assert_eq!(limits.tree.slots, 256);
    assert!(limits.tree.text_bytes < 1024 * limits.tree.text_per_node);
    assert!(crate::valid(&limits, &domain, jig, first));
}

#[test]
fn bounded_world_storage_survives_the_jig_envelopes_text_overflow() {
    let mut domain = world_domain();
    domain.jig.rows = 4;
    domain.jig.objects = 8;
    domain.jig.watches = 16;
    domain.jig.document_bytes = 8192;
    let jig = super::words();
    let first = super::family_words();
    let envelope = jig_client_view::limits::envelope(
        &domain.jig,
        jig.jig_capacity().unwrap(),
        &jig_client_view::page::Arrangement::none(),
        0,
        0,
        0,
    )
    .unwrap();
    assert!(u64::from(envelope.nodes) * u64::from(envelope.text_per_node) > u64::from(u32::MAX));
    assert!(jig_client_view::limits::of_words(&domain.jig, jig).is_none());
    let limits = Limits::for_world(&domain, jig, first, 1024, 256).unwrap();
    assert_eq!(limits.tree.nodes, 1024);
    assert!(limits.tree.text_bytes < 1024 * limits.tree.text_per_node);
}

#[test]
fn bounded_storage_keeps_every_attribute_of_a_finite_complete_envelope() {
    let mut domain = world_domain();
    domain.jig.rows = 0;
    domain.jig.fields = 0;
    domain.jig.text = 1;
    domain.first.rows = 0;
    domain.first.text = 1;
    let jig = crate::connector_values_words::english(1).unwrap();
    let first = crate::words::Words::english(1).unwrap();
    let full = Limits::of(&domain, &jig, &first).unwrap();
    let mut expected = full;
    expected.tree.nodes = 32;
    expected.tree.bindings = 32;
    expected.tree.slots = 4;
    expected.tree.patches = 160;
    expected.tree.text_bytes = (full.tree.text_bytes / full.tree.nodes) * 32;
    assert_eq!(Limits::for_world(&domain, &jig, &first, 32, 4), Some(expected));
    assert!(Limits::for_world(&domain, &jig, &first, full.tree.nodes + 1, 0).is_none());
}

#[test]
fn invalid_world_storage_declarations_are_rejected() {
    let domain = world_domain();
    let jig = super::words();
    let first = super::family_words();
    assert!(Limits::for_world(&domain, jig, first, 0, 0).is_none());
    assert!(Limits::for_world(&domain, jig, first, 8, 9).is_none());
    assert!(Limits::for_world(&domain, jig, first, u32::MAX, 0).is_none());
}

#[test]
#[should_panic(expected = "tree fits its node capacity")]
fn a_world_fixture_beyond_its_node_bound_is_rejected() {
    let limits = Limits::for_world(&world_domain(), super::words(), super::family_words(), 1, 0).unwrap();
    let mut builder = Builder::new(&limits.tree);
    builder.open(Role::Page);
    builder.open(Role::Text);
}

#[test]
#[should_panic(expected = "slots fit their capacity")]
fn a_world_fixture_beyond_its_slot_bound_is_rejected() {
    let limits = Limits::for_world(&world_domain(), super::words(), super::family_words(), 8, 0).unwrap();
    let mut builder = Builder::new(&limits.tree);
    builder.open(Role::Page);
    builder.slot(
        SlotRef {
            owner: (Owner::Application { number: jig_client_test_domain::FIRST }).opaque(),
            token: Token::new(1),
        },
        b"loading",
    );
}

#[test]
fn valid_refuses_each_unrepresentable_limit() {
    let domain = world_domain();
    let jig = super::words();
    let first = super::family_words();
    let limits = Limits::for_world(&domain, jig, first, 1024, 256).unwrap();
    assert!(crate::valid(&limits, &domain, jig, first));
    let missing_jig = jig_client_view::words::Words::new(first.book.clone(), 0, 0);
    let missing_first = crate::words::Words { book: jig.book().clone(), language: 0 };
    for (condition, jig, first) in
        [("jig loading phrase", &missing_jig, first), ("family phrase capacities", jig, &missing_first)]
    {
        assert!(!crate::valid(&limits, &domain, jig, first), "{condition}");
    }
}

#[test]
fn validity_does_not_price_retained_heap() {
    let mut domain = world_domain();
    let jig = super::words();
    let first = super::family_words();
    let limits = Limits::for_world(&domain, jig, first, 1024, 256).unwrap();
    domain.jig.document_bytes = u64::MAX;
    assert!(crate::valid(&limits, &domain, jig, first));
}
