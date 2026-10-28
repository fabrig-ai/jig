#[path = "tests/asks.rs"]
mod asks;
#[path = "tests/fixtures.rs"]
mod fixtures;

#[path = "tests/answers.rs"]
mod answers;
#[path = "tests/carried.rs"]
mod carried;
#[path = "tests/moves.rs"]
mod moves;

#[path = "tests/documents.rs"]
mod documents;
#[path = "tests/llm.rs"]
mod llm;
#[path = "tests/services.rs"]
mod services;

#[path = "tests/objects.rs"]
mod objects;

#[path = "tests/pages.rs"]
mod pages;

#[path = "tests/reads.rs"]
mod reads;

#[path = "tests/accounts.rs"]
mod accounts;
#[path = "tests/watches.rs"]
mod watches;

#[path = "tests/closing.rs"]
mod closing;

#[path = "tests/compound.rs"]
mod compound;
#[path = "tests/create.rs"]
mod create;
#[path = "tests/effects.rs"]
mod effects;
#[path = "tests/queries.rs"]
mod queries;

#[path = "tests/limits.rs"]
mod limits;

#[path = "tests/programs.rs"]
mod programs;

#[path = "tests/settings.rs"]
mod settings;

#[path = "tests/settings_change.rs"]
mod settings_change;

#[path = "tests/finishes.rs"]
mod finishes;

#[test]
#[expect(clippy::disallowed_macros, reason = "ordinary test keeps the large wire-limit table on the heap")]
fn valid_refuses_each_unrepresentable_limit() {
    let limits = fixtures::env().limits;
    for (name, limits, accepted) in vec![
        ("fixture", limits, true),
        ("identity_bytes minimum", crate::Limits { identity_bytes: 7, ..limits }, false),
        ("streams minimum", crate::Limits { streams: 0, ..limits }, false),
        ("watches_per_stream minimum", crate::Limits { watches_per_stream: 0, ..limits }, false),
        ("queued_bytes minimum", crate::Limits { queued_bytes: 127, ..limits }, false),
        ("provider_codes minimum", crate::Limits { provider_codes: 0, ..limits }, false),
        ("provider_tokens minimum", crate::Limits { provider_tokens: 0, ..limits }, false),
        ("mints minimum", crate::Limits { mints: 0, ..limits }, false),
        ("message_bytes minimum", crate::Limits { message_bytes: 0, ..limits }, false),
        ("exchanges minimum", crate::Limits { exchanges: 0, ..limits }, false),
        ("exchanges_per_client minimum", crate::Limits { exchanges_per_client: 0, ..limits }, false),
        ("link_span minimum", crate::Limits { link_span: skein_lib::Duration::ZERO, ..limits }, false),
        ("exchange doubling", crate::Limits { exchanges: u32::MAX, ..limits }, false),
        ("issuer document", crate::Limits { identity_bytes: u32::MAX, ..limits }, false),
        (
            "signed_in_token_some",
            crate::Limits {
                wire: jig_client_protocol_wire::v19::Limits { signed_in_token_some: 39, ..limits.wire },
                ..limits
            },
            false,
        ),
        (
            "made_service_credential",
            crate::Limits {
                wire: jig_client_protocol_wire::v19::Limits { made_service_credential: 39, ..limits.wire },
                ..limits
            },
            false,
        ),
        (
            "link_sign_in_secret",
            crate::Limits {
                wire: jig_client_protocol_wire::v19::Limits { link_sign_in_secret: 31, ..limits.wire },
                ..limits
            },
            false,
        ),
        (
            "keyed_value_some",
            crate::Limits {
                wire: jig_client_protocol_wire::v19::Limits { keyed_value_some: u32::MAX, ..limits.wire },
                ..limits
            },
            false,
        ),
    ] {
        assert_eq!(crate::valid(&limits), accepted, "{name}");
    }
}
