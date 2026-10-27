use super::fixtures::{document, path};
use crate::names::{NAMES, Unit};
use crate::{Profile, What, overrides, profile};
use alloc::collections::BTreeSet;
use skein_json::Document;
use skein_lib::{Decimal, List, Writer};

#[test]
fn every_row_is_a_name_apply_takes_at_its_small_value() {
    let small = profile(Profile::Small);
    for row in &NAMES {
        let value = overrides::value(&small, row.name.as_bytes()).expect(row.name);
        let mut copy = small;
        assert_eq!(overrides::apply(&mut copy, row.name.as_bytes(), value), Ok(()), "{}", row.name);
        assert_eq!(copy, small, "{}", row.name);
    }
}

#[test]
fn every_row_parses_at_its_own_small_value() {
    let small = profile(Profile::Small);
    let prefix = path(&[b"limits"]);
    let expected = overrides::parse(&document(br#"{"profile":"small","overrides":{}}"#), 0, &prefix).unwrap();
    let mut refused = List::with_capacity(u32::try_from(NAMES.len()).unwrap());
    for row in &NAMES {
        let value = overrides::value(&small, row.name.as_bytes()).expect(row.name);
        let document = override_document(row.name.as_bytes(), value);
        let parsed = overrides::parse(&document, 0, &prefix);
        if parsed != Ok(expected) {
            refused.push((row.name, parsed)).unwrap();
        }
    }
    assert!(refused.is_empty(), "rows refused or changed at their own values: {refused:?}");
}

#[test]
fn a_name_with_no_row_is_refused() {
    let small = profile(Profile::Small);
    for name in [b"tasks.absent".as_slice(), b"tasks.versions"] {
        let mut copy = small;
        assert_eq!(overrides::apply(&mut copy, name, 1), Err(What::Override));
        assert_eq!(overrides::value(&small, name), None);
    }
}

#[test]
fn rows_are_unique_and_well_formed() {
    const PREFIXES: [&str; 13] = [
        "tasks",
        "people",
        "fleet",
        "authority",
        "accounts",
        "notes",
        "documents",
        "views",
        "brief",
        "core",
        "hub",
        "link",
        "clients",
    ];
    let mut seen = BTreeSet::new();
    let mut groups = List::with_capacity(u32::try_from(PREFIXES.len()).unwrap());
    for row in &NAMES {
        assert!(seen.insert(row.name), "duplicate: {}", row.name);
        let (prefix, field) = row.name.split_once('.').expect("owner and field");
        assert!(!field.is_empty(), "{}", row.name);
        if groups.as_slice().last() != Some(&prefix) {
            groups.push(prefix).unwrap();
        }
        for byte in row.name.bytes() {
            assert!(byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_.".contains(&byte), "{}", row.name);
        }
        assert_eq!(row.name.ends_with("_ms"), row.unit == Unit::Milliseconds, "{}", row.name);
        if row.name.ends_with("_bytes") {
            assert_eq!(row.unit, Unit::Bytes, "{}", row.name);
        }
    }
    assert_eq!(groups.as_slice(), PREFIXES);
}

#[test]
fn each_line_is_at_most_160_bytes_on_one_line() {
    // A reference row's one-sentence byte bound.
    const LINE_BYTES: usize = 160;
    for row in &NAMES {
        assert!(row.line.len() <= LINE_BYTES, "{}", row.name);
        assert!(!row.line.contains(['\r', '\n']), "{}", row.name);
    }
}

#[test]
fn value_reads_a_duration_in_milliseconds() {
    let small = profile(Profile::Small);
    assert_eq!(overrides::value(&small, b"tasks.grace_ms"), Some(small.core.tasks.grace.as_nanos() / 1_000_000));
}

fn override_document(name: &[u8], value: u64) -> Document {
    let decimal = Decimal::of(value);
    let parts = [b"{\"profile\":\"small\",\"overrides\":{\"".as_slice(), name, b"\":", decimal.as_bytes(), b"}}"];
    let mut length = 0_usize;
    for part in parts {
        length = length.checked_add(part.len()).unwrap();
    }
    let mut writer = Writer::new(length);
    for part in parts {
        writer.put(part).unwrap();
    }
    document(&writer.finish())
}
