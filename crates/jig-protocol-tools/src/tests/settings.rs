//! The protocol stand carries settings reads, leaving complete rendering to the application.
use crate::{Decoded, Problem, Why};
use jig_domain_core::{NamedAction, SettingsRead, settings::Setting};

#[test]
fn settings_reads_decode_through_the_reading_tool_declaration() {
    for (input, expected) in [
        (br#"{"kind":"inventory"}"#.as_slice(), SettingsRead::Inventory),
        (
            br#"{"kind":"setting","setting":"charter","number":1,"revision":2}"#,
            SettingsRead::Setting { setting: Setting::Charter(1), revision: 2 },
        ),
        (
            br#"{"kind":"setting","setting":"template","number":3,"revision":2}"#,
            SettingsRead::Setting { setting: Setting::Template(3), revision: 2 },
        ),
        (
            br#"{"kind":"setting","setting":"policy","number":4,"revision":2}"#,
            SettingsRead::Setting { setting: Setting::Policy(4), revision: 2 },
        ),
        (
            br#"{"kind":"setting","setting":"rules","revision":2}"#,
            SettingsRead::Setting { setting: Setting::Rules, revision: 2 },
        ),
        (br#"{"kind":"revision","number":5}"#, SettingsRead::Revision { number: 5 }),
        (br#"{"kind":"revisions"}"#, SettingsRead::Revisions { before: None }),
        (br#"{"kind":"revisions","before":4}"#, SettingsRead::Revisions { before: Some(4) }),
    ] {
        let Decoded::Action(NamedAction::Settings(read)) = crate::decode(b"settings", input, &super::fixtures::LIMITS)
        else {
            panic!("settings read: {input:?}")
        };
        assert_eq!(read, expected);
    }
    super::schema(b"settings");
    for declaration in crate::declarations() {
        if declaration.name == b"settings" {
            assert!(!declaration.writes, "settings is a read");
        }
    }
}

#[test]
fn malformed_settings_reads_name_the_invalid_field() {
    for (input, expected) in [
        (br#"{"kind":"other"}"#.as_slice(), Problem { field: b"kind", why: Why::Range }),
        (br#"{"kind":"setting","setting":"other","revision":1}"#, Problem { field: b"setting", why: Why::Range }),
        (br#"{"kind":"setting","setting":"charter","revision":1}"#, Problem { field: b"number", why: Why::Missing }),
        (br#"{"kind":"setting","setting":"rules"}"#, Problem { field: b"revision", why: Why::Missing }),
        (br#"{"kind":"revision","number":4294967296}"#, Problem { field: b"number", why: Why::Range }),
        (br#"{"kind":"revisions","before":4294967296}"#, Problem { field: b"before", why: Why::Range }),
    ] {
        let Decoded::Problem(problem) = crate::decode(b"settings", input, &super::fixtures::LIMITS) else {
            panic!("invalid settings input: {input:?}")
        };
        assert_eq!(problem, expected);
    }
}
