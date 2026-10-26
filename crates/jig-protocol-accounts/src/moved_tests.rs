use crate::{
    moved::{self, Moved, Unread},
    v1,
};
use jig_domain_accounts::{DISPLAY, SUBJECT};
use skein_lib::{Reader, Wall, Writer, bytes};
use skein_oauth::{Limits, SavedToken};

fn limits() -> Limits {
    Limits {
        document_bytes: 512,
        string_bytes: 128,
        token_bytes: 128,
        client_bytes: 64,
        issued_client_bytes: 64,
        granted_bytes: 64,
        subject_bytes: 64,
        id_token_bytes: 64,
        detail_bytes: 64,
        record_bytes: 512,
        depth: 4,
        tokens: 32,
    }
}

fn fixture(full: bool) -> Moved {
    let mut record = SavedToken {
        key: 7,
        generation: 9,
        access_token: bytes::copy_of(b"a"),
        refresh_token: None,
        metadata: None,
        expires_at: Wall::from_nanos(123),
        issued_client_id: None,
        granted_scopes: None,
        subject: None,
        id_token: None,
        earliest_refresh_at: None,
    };
    if full {
        record.refresh_token = Some(bytes::copy_of(b"refresh"));
        record.metadata = Some(bytes::copy_of(b"metadata"));
        record.issued_client_id = Some(bytes::copy_of(b"issued-client"));
        record.granted_scopes = Some(bytes::copy_of(b"scope one"));
        record.subject = Some(bytes::copy_of(b"subject"));
        record.id_token = Some(bytes::copy_of(b"id-token"));
        record.earliest_refresh_at = Some(Wall::from_nanos(100));
    }
    Moved {
        application: bytes::copy_of(b"a"),
        issuer: bytes::copy_of(b"i"),
        client_id: bytes::copy_of(b"c"),
        subject: bytes::copy_of(b"s"),
        display: bytes::copy_of(if full { b"Display" } else { b"" }),
        record,
    }
}

fn assert_same(expected: &Moved, actual: &Moved) {
    assert_eq!(expected.application, actual.application);
    assert_eq!(expected.issuer, actual.issuer);
    assert_eq!(expected.client_id, actual.client_id);
    assert_eq!(expected.subject, actual.subject);
    assert_eq!(expected.display, actual.display);
    assert!(expected.record == actual.record, "all saved-token fields retained");
}

// The version plus six length prefixes (protocol/llm.md, 5.6).
const HEADER: u32 = 2;
const PARTS: usize = 6;
const OUTER_CAPS: [(&str, u32); 5] = [
    ("application", moved::APPLICATION),
    ("issuer", moved::ISSUER),
    ("client id", moved::CLIENT_ID),
    ("subject", SUBJECT),
    ("display", DISPLAY),
];

#[test]
fn full_and_smallest_moved_records_round_trip() {
    for full in [true, false] {
        let original = fixture(full);
        let encoded = moved::encode(&original, &limits()).expect("valid record");
        assert_eq!(&encoded.expose()[..2], b"\0\x01");
        let decoded = moved::decode(encoded.expose(), &limits()).expect("valid file");
        assert_same(&original, &decoded);
        assert_eq!(moved::encode(&decoded, &limits()).expect("decoded record fits"), encoded);
    }
}

#[test]
fn schema_ceilings_match_the_parts_owners() {
    assert_eq!(v1::CEILINGS.moved_application, moved::APPLICATION);
    assert_eq!(v1::CEILINGS.moved_issuer, moved::ISSUER);
    assert_eq!(v1::CEILINGS.moved_client_id, moved::CLIENT_ID);
    assert_eq!(v1::CEILINGS.moved_subject, SUBJECT);
    assert_eq!(v1::CEILINGS.moved_display, DISPLAY);
    assert_eq!(
        u64::from(v1::CEILINGS.moved_record),
        skein_oauth::v3::SavedToken::worst_case_bytes(&skein_oauth::v3::CEILINGS).expect("saved token ceiling")
    );
}

// Walk the wire's lengths to edit a named part's prefix.
fn prefix(input: &[u8], part: usize) -> usize {
    let mut reader = Reader::new(input);
    reader.skip(HEADER).expect("version");
    for _ in 0..part {
        let len = reader.u32().expect("part length");
        reader.skip(len).expect("part bytes");
    }
    input.len().checked_sub(usize::try_from(reader.remaining()).expect("remaining fits")).expect("cursor")
}

fn length(input: &mut [u8], part: usize, len: u32) {
    let offset = prefix(input, part);
    for (index, byte) in len.to_be_bytes().into_iter().enumerate() {
        input[offset.checked_add(index).expect("prefix byte")] = byte;
    }
}

fn refused(input: &[u8], limits: &Limits, expected: Unread, name: &str) {
    assert_eq!(moved::decode(input, limits).err(), Some(expected), "{name}");
}

#[test]
fn old_jigm_files_are_refused_as_another_version() {
    // Literal output of the former encoder for fixture(false), including
    // its version-3 SavedToken. It is not made by the new encoder.
    let old = b"JIGM\0\x01\0\0\0\x01a\0\0\0\x01i\0\0\0\x01c\0\0\0\x01s\0\0\0\0\0\0\0\x22\0\x03\0\0\0\x07\0\0\0\0\0\0\0\x09\0\0\0\0\0\0\0\x7b\0\0\0\x01a\0\0\0\0\0\0\0";
    refused(old, &limits(), Unread::Version, "old JIGM form");
}

#[test]
fn another_version_is_refused_by_name() {
    let valid = moved::encode(&fixture(false), &limits()).expect("fixture");
    let mut changed = bytes::copy_of(valid.expose());
    for (index, byte) in 2_u16.to_be_bytes().into_iter().enumerate() {
        changed[index] = byte;
    }
    refused(&changed, &limits(), Unread::Version, "version 2");
}

#[test]
fn decode_refuses_each_part_one_past_its_bound() {
    let valid = moved::encode(&fixture(true), &limits()).expect("fixture");
    for (part, (name, cap)) in OUTER_CAPS.into_iter().chain([("record", limits().record_bytes)]).enumerate() {
        let mut changed = bytes::copy_of(valid.expose());
        length(&mut changed, part, cap.checked_add(1).expect("small cap"));
        refused(&changed, &limits(), Unread::TooLarge, name);
        // Only the prefix remains: oversize wins before trying to read bytes.
        let end = prefix(&changed, part).checked_add(4).expect("prefix end");
        refused(&changed[..end], &limits(), Unread::TooLarge, name);
    }
}

#[test]
fn every_truncation_of_a_valid_file_is_refused() {
    let valid = moved::encode(&fixture(true), &limits()).expect("fixture");
    for end in 0..valid.len() {
        refused(&valid.expose()[..end], &limits(), Unread::Malformed, "truncated file");
    }
}

#[test]
fn a_trailing_byte_is_refused() {
    let valid = moved::encode(&fixture(true), &limits()).expect("fixture");
    let mut trailing = Writer::new(valid.len().checked_add(1).expect("one extra byte"));
    trailing.put(valid.expose()).expect("room");
    trailing.put(b"x").expect("room");
    refused(&trailing.finish(), &limits(), Unread::Trailing, "trailing byte");
}

#[test]
fn whole_file_bound_is_checked_before_its_version() {
    let oversized =
        bytes::zeroed(usize::try_from(moved::bound(&limits()).checked_add(1).expect("bound plus one")).expect("fits"));
    refused(&oversized, &limits(), Unread::TooLarge, "whole-file bound");
}

fn filled(len: u32) -> Box<[u8]> {
    let mut field = bytes::zeroed(usize::try_from(len).expect("small bound"));
    field.fill(b'x');
    field
}

fn outer_field(original: &mut Moved, part: usize) -> &mut Box<[u8]> {
    match part {
        0 => &mut original.application,
        1 => &mut original.issuer,
        2 => &mut original.client_id,
        3 => &mut original.subject,
        4 => &mut original.display,
        _ => unreachable!("five outer fields"),
    }
}

#[test]
fn each_outer_part_accepts_its_exact_bound() {
    for (part, (_name, cap)) in OUTER_CAPS.into_iter().enumerate() {
        let mut original = fixture(false);
        *outer_field(&mut original, part) = filled(cap);
        let encoded = moved::encode(&original, &limits()).expect("field at bound");
        let decoded = moved::decode(encoded.expose(), &limits()).expect("field at bound decodes");
        assert_same(&original, &decoded);
    }
}

#[test]
fn encode_refuses_each_outer_part_one_past_its_bound() {
    for (part, (name, cap)) in OUTER_CAPS.into_iter().enumerate() {
        let mut original = fixture(false);
        *outer_field(&mut original, part) = filled(cap.checked_add(1).expect("small cap"));
        assert_eq!(moved::encode(&original, &limits()).err(), Some(Unread::TooLarge), "{name}");
    }
}

fn record_limits(original: &Moved) -> Limits {
    let mut limits = limits();
    limits.record_bytes =
        u32::try_from(skein_oauth::encode_record(&original.record, &limits).expect("record").len()).expect("size");
    limits
}

#[test]
fn a_record_at_its_runtime_bound_round_trips() {
    let original = fixture(true);
    let limits = record_limits(&original);
    let encoded = moved::encode(&original, &limits).expect("record at bound");
    assert_same(&original, &moved::decode(encoded.expose(), &limits).expect("record at bound decodes"));
}

#[test]
fn a_record_one_past_its_runtime_bound_is_refused() {
    let original = fixture(true);
    let mut limits = record_limits(&original);
    let encoded = moved::encode(&original, &limits).expect("fixture");
    limits.record_bytes = limits.record_bytes.checked_sub(1).expect("nonzero record");
    assert_eq!(moved::encode(&original, &limits).err(), Some(Unread::Record));
    refused(encoded.expose(), &limits, Unread::TooLarge, "record one past");
}

// Replace a whole part, retaining the following fields and their boundaries.
fn replace_part(input: &[u8], part: usize, replacement: &[u8]) -> Box<[u8]> {
    let mut reader = Reader::new(input);
    let version = reader.u16().expect("version");
    let replaced_length = Reader::new(&input[prefix(input, part)..]).u32().expect("replaced part length");
    let size = input
        .len()
        .checked_sub(usize::try_from(replaced_length).expect("length fits"))
        .expect("replaced part fits input")
        .checked_add(replacement.len())
        .expect("replacement capacity");
    let mut out = Writer::new(size);
    out.put(&version.to_be_bytes()).expect("room");
    for index in 0..PARTS {
        let len = reader.u32().expect("part length");
        let field = reader.bytes(len).expect("part bytes");
        let field = if index == part { replacement } else { field };
        out.put(&u32::try_from(field.len()).expect("length").to_be_bytes()).expect("room");
        out.put(field).expect("room");
    }
    out.finish()
}

#[test]
fn decode_refuses_every_empty_required_part() {
    let valid = moved::encode(&fixture(false), &limits()).expect("fixture");
    for part in [0, 1, 2, 3, 5] {
        let changed = replace_part(valid.expose(), part, b"");
        refused(&changed, &limits(), Unread::Malformed, "empty required part");
    }
}

#[test]
fn encode_refuses_every_empty_required_outer_part() {
    for part in 0..4 {
        let mut original = fixture(false);
        *outer_field(&mut original, part) = bytes::copy_of(b"");
        assert_eq!(moved::encode(&original, &limits()).err(), Some(Unread::Malformed));
    }
}

#[test]
fn an_empty_display_is_accepted() {
    let original = fixture(false);
    let encoded = moved::encode(&original, &limits()).expect("empty display allowed");
    let decoded = moved::decode(encoded.expose(), &limits()).expect("empty display allowed");
    assert!(decoded.display.is_empty());
}

#[test]
fn an_invalid_nested_record_is_refused_by_name() {
    let valid = moved::encode(&fixture(true), &limits()).expect("fixture");
    refused(&replace_part(valid.expose(), 5, b"invalid record"), &limits(), Unread::Record, "nested record");
}

#[test]
fn encode_refuses_a_record_skein_will_not_encode() {
    let mut original = fixture(true);
    original.record.access_token = bytes::copy_of(b"invalid bearer!");
    assert_eq!(moved::encode(&original, &limits()).err(), Some(Unread::Record));
}

#[test]
fn bound_is_the_encoded_size_with_every_part_at_its_bound() {
    let mut original = fixture(true);
    for (part, (_name, cap)) in OUTER_CAPS.into_iter().enumerate() {
        *outer_field(&mut original, part) = filled(cap);
    }
    let limits = record_limits(&original);
    let encoded = moved::encode(&original, &limits).expect("all parts at bounds");
    assert_eq!(moved::bound(&limits), u64::try_from(encoded.len()).expect("size"));
    assert_same(&original, &moved::decode(encoded.expose(), &limits).expect("largest file decodes"));
}

#[test]
fn a_runtime_allowance_above_the_schema_uses_its_ceiling() {
    let limits = Limits { record_bytes: u32::MAX, ..limits() };
    assert_eq!(moved::bound(&limits), v1::Moved::worst_case_bytes(&v1::CEILINGS).expect("schema size"));
}
