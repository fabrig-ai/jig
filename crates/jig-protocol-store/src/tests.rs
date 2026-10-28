//! Store key, interval and codec properties (protocol/store.md, section 14).
mod codec;
mod commits;
mod directory;
pub(crate) mod fixtures;
mod format;
mod frames;
mod generations;
mod gets;
mod intervals;
mod keys;
mod open;
mod pages;
mod patching;
mod reads;
mod restore;
mod secrets;
mod transcripts;

mod settings;

mod operations;
mod phase;
mod words;

mod frame_memory;
mod frame_patches;
mod settings_layout;

#[test]
fn valid_refuses_each_unrepresentable_limit() {
    let limits = fixtures::store_limits();
    for (name, limits, accepted) in [
        ("fixture", limits, true),
        ("store validity", crate::Limits { store: skein_kv::Limits { key: 0, ..limits.store }, ..limits }, false),
        ("secret validity", crate::Limits { secrets: skein_kv::Limits { key: 0, ..limits.secrets }, ..limits }, false),
        (
            "decoded codec",
            crate::Limits {
                core: jig_domain_core::Limits {
                    tasks: jig_domain_tasks::Limits { spec_bytes: u32::MAX, ..limits.core.tasks },
                    ..limits.core
                },
                ..limits
            },
            false,
        ),
        ("reservation addition", crate::Limits { queue: u32::MAX, ..limits }, false),
        (
            "reservation product",
            crate::Limits { journal: skein_lib::JournalLimits { writes: u32::MAX, ..limits.journal }, ..limits },
            false,
        ),
        ("operation capacity", crate::Limits { orphan_erases: u32::MAX, ..limits }, false),
        (
            "question ranges",
            crate::Limits {
                core: jig_domain_core::Limits {
                    tasks: jig_domain_tasks::Limits { tasks: u32::MAX, inbox_messages: 2, ..limits.core.tasks },
                    ..limits.core
                },
                ..limits
            },
            false,
        ),
        (
            "reader addition",
            crate::Limits { core: jig_domain_core::Limits { load_slots: u32::MAX, ..limits.core }, ..limits },
            false,
        ),
    ] {
        assert_eq!(crate::valid(&limits), accepted, "{name}");
    }
}
