mod backoff;
mod busy;
mod fixtures;
mod relays;
mod translate;

#[test]
fn valid_refuses_each_unrepresentable_limit() {
    for (name, limits, accepted) in [
        ("fixture", fixtures::LIMITS, true),
        ("handshake", crate::Limits { handshake: skein_lib::Duration::ZERO, ..fixtures::LIMITS }, false),
        ("ping", crate::Limits { ping: skein_lib::Duration::ZERO, ..fixtures::LIMITS }, false),
        ("stall", crate::Limits { stall: skein_lib::Duration::ZERO, ..fixtures::LIMITS }, false),
        ("silence", crate::Limits { silence: fixtures::LIMITS.ping, ..fixtures::LIMITS }, false),
        ("chunk", crate::Limits { chunk: 0, ..fixtures::LIMITS }, false),
        ("grace", crate::Limits { grace: skein_lib::Duration::ZERO, ..fixtures::LIMITS }, false),
        ("busy", crate::Limits { busy: skein_lib::Duration::ZERO, ..fixtures::LIMITS }, false),
        ("backoff", crate::Limits { backoff: skein_lib::Duration::ZERO, ..fixtures::LIMITS }, false),
        ("proved", crate::Limits { proved: skein_lib::Duration::ZERO, ..fixtures::LIMITS }, false),
        ("ceiling", crate::Limits { ceiling: skein_lib::Duration::ZERO, ..fixtures::LIMITS }, false),
        (
            "name edge",
            crate::Limits {
                sizes: jig_protocol_worker_wire::sizes::Sizes { name: u32::MAX, ..fixtures::SIZES },
                ..fixtures::LIMITS
            },
            false,
        ),
        (
            "program_sources edge",
            crate::Limits {
                sizes: jig_protocol_worker_wire::sizes::Sizes { program_sources: u32::MAX, ..fixtures::SIZES },
                ..fixtures::LIMITS
            },
            false,
        ),
        (
            "slots edge",
            crate::Limits {
                sizes: jig_protocol_worker_wire::sizes::Sizes { slots: u32::MAX, ..fixtures::SIZES },
                ..fixtures::LIMITS
            },
            false,
        ),
        (
            "turns edge",
            crate::Limits {
                sizes: jig_protocol_worker_wire::sizes::Sizes { turns: u32::MAX, ..fixtures::SIZES },
                ..fixtures::LIMITS
            },
            false,
        ),
        (
            "calls edge",
            crate::Limits {
                sizes: jig_protocol_worker_wire::sizes::Sizes { calls: u32::MAX, ..fixtures::SIZES },
                ..fixtures::LIMITS
            },
            false,
        ),
        (
            "accounts edge",
            crate::Limits {
                sizes: jig_protocol_worker_wire::sizes::Sizes { accounts: u32::MAX, ..fixtures::SIZES },
                ..fixtures::LIMITS
            },
            false,
        ),
        (
            "credentials edge",
            crate::Limits {
                sizes: jig_protocol_worker_wire::sizes::Sizes { credentials: u32::MAX, ..fixtures::SIZES },
                ..fixtures::LIMITS
            },
            false,
        ),
    ] {
        assert_eq!(crate::valid(&limits), accepted, "{name}");
    }
}
