//! The in-place payload hash against the generated envelope decoder (store.md 9).
use crate::{CARRIED, CarriedKind, HELD, SecretOwner, SecretValue, secrets::codec, secrets_v1 as w};
use alloc::boxed::Box;
use skein_lib::{Secret, Writer};

// Two credential owners, two source variants, three carried variants and raw application bytes.
const KINDS: usize = 8;
fn values() -> [(SecretOwner, u64, SecretValue); KINDS] {
    let payload = Secret::new(Box::from(b"opaque nested record or key".as_slice()));
    [
        (SecretOwner::SignIn, 0, SecretValue::Digest { expiry: 17, digest: [23; 32] }),
        (SecretOwner::Service, 0, SecretValue::Digest { expiry: u64::MAX, digest: [29; 32] }),
        // These bytes deliberately do not decode as a nested OAuth or moved record.
        (SecretOwner::Source, 0, SecretValue::Record(payload.clone())),
        (SecretOwner::Source, 0, SecretValue::Key(payload.clone())),
        (SecretOwner::Application, CARRIED, SecretValue::Carried { kind: CarriedKind::Key, value: payload.clone() }),
        (SecretOwner::Application, CARRIED, SecretValue::Carried { kind: CarriedKind::Value, value: payload.clone() }),
        (SecretOwner::Application, CARRIED, SecretValue::Carried { kind: CarriedKind::Moved, value: payload.clone() }),
        (SecretOwner::Application, CARRIED - 1, SecretValue::Application(payload)),
    ]
}

fn agrees(owner: SecretOwner, number: u64, bytes: &[u8]) {
    let found = codec::durable_digest(owner, number, bytes);
    match codec::decode(owner, number, Box::from(bytes)) {
        Some(value) => assert_eq!(found, Some(codec::digest(&value)), "{owner:?}/{number}: {bytes:?}"),
        None => assert_eq!(found, None, "{owner:?}/{number}: {bytes:?}"),
    }
}

#[test]
fn stored_digests_match_generated_decoding_for_every_owner() {
    for (owner, number, value) in values() {
        let expected = codec::digest(&value);
        let bytes = codec::encode(value, u32::MAX).expect("fixture encoding");
        assert_eq!(codec::durable_digest(owner, number, &bytes), Some(expected));
        for owner in [SecretOwner::SignIn, SecretOwner::Service, SecretOwner::Source, SecretOwner::Application] {
            for number in [0, CARRIED - 1, CARRIED, HELD - 1, HELD, u64::MAX] {
                agrees(owner, number, &bytes);
            }
        }
    }
}

#[test]
fn malformed_envelopes_agree_with_the_generated_decoder() {
    // Version (2), enum tag (1) and payload length (4), or the first seven credential bytes.
    const PREFIX: usize = 7;
    for (owner, number, value) in values() {
        let bytes = codec::encode(value, u32::MAX).expect("fixture encoding");
        for length in 0..bytes.len() {
            agrees(owner, number, bytes.get(..length).expect("truncation"));
        }
        for index in 0..PREFIX {
            for byte in 0..=u8::MAX {
                let mut changed = bytes.clone();
                *changed.get_mut(index).expect("fixture prefix") = byte;
                agrees(owner, number, &changed);
            }
        }
        let mut tail = Writer::new(bytes.len().checked_add(1).expect("trailing byte"));
        tail.put(&bytes).expect("row");
        tail.put(&[0]).expect("trailing byte");
        agrees(owner, number, &tail.finish());
    }
}

// Version (2), enum tag (1), secret length (4), followed by the declared bytes.
const PREFIX_BYTES: u32 = 7;
fn envelope(tag: u8, length: u32) -> Box<[u8]> {
    let mut bytes = Writer::new(usize::try_from(PREFIX_BYTES.checked_add(length).expect("row size")).expect("room"));
    bytes.put(&1_u16.to_be_bytes()).expect("version");
    bytes.put(&[tag]).expect("tag");
    bytes.put(&length.to_be_bytes()).expect("length");
    let payload = Secret::new(Box::from("x".repeat(usize::try_from(length).expect("payload size")).as_bytes()));
    bytes.put(payload.expose()).expect("payload");
    bytes.finish()
}

#[test]
fn each_payload_bound_agrees_with_the_generated_decoder() {
    for (owner, number, tag, bound) in [
        (SecretOwner::Source, 0, 0, w::CEILINGS.source_record_record),
        (SecretOwner::Source, 0, 1, w::CEILINGS.source_key_key),
        (SecretOwner::Application, CARRIED, 0, w::CEILINGS.carried_key_key),
        (SecretOwner::Application, CARRIED, 1, w::CEILINGS.carried_value_value),
        (SecretOwner::Application, CARRIED, 2, w::CEILINGS.carried_moved_moved),
    ] {
        for length in [0, bound, bound.checked_add(1).expect("past bound")] {
            agrees(owner, number, &envelope(tag, length));
        }
    }
}
