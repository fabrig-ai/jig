//! The store alone encodes secrets envelopes; held bytes remain opaque (store.md 9).
use crate::{CARRIED, CarriedKind, HELD, SecretOwner, SecretValue, secrets_v1 as w};
use alloc::boxed::Box;
use skein_lib::{Reader, Secret, Writer, sha256};

// The version (u16) and the next number (u64).
pub(super) const NEXT_BYTES: u32 = 2 + 8;

pub(super) fn accepts(owner: SecretOwner, number: u64, value: &SecretValue) -> bool {
    match value {
        SecretValue::Digest { expiry, .. } => match owner {
            SecretOwner::SignIn => true,
            SecretOwner::Service => *expiry == u64::MAX,
            SecretOwner::Source | SecretOwner::Application => false,
        },
        SecretValue::Record(_) | SecretValue::Key(_) => owner == SecretOwner::Source,
        SecretValue::Application(_) => owner == SecretOwner::Application && number < CARRIED,
        SecretValue::Carried { .. } | SecretValue::Held(_) => false,
    }
}
/// Digests identify held values, including undecodable nested records, never envelopes.
pub(super) fn digest(value: &SecretValue) -> [u8; 32] {
    match value {
        SecretValue::Digest { expiry, digest } => {
            // The former credential value: its expiry followed by its digest.
            let mut hash = sha256::Sha256::new();
            hash.update(&expiry.to_be_bytes());
            hash.update(digest);
            hash.finish()
        }
        SecretValue::Record(value)
        | SecretValue::Key(value)
        | SecretValue::Application(value)
        | SecretValue::Held(value)
        | SecretValue::Carried { value, .. } => sha256::digest(value.expose()),
    }
}
pub(crate) fn encode(value: SecretValue, limit: u32) -> Option<Box<[u8]>> {
    let limits = &w::CEILINGS;
    match value {
        SecretValue::Digest { expiry, digest } => {
            let row = w::Digest::new(limits, w::DigestParts { expiry, digest }).ok()?;
            let mut bytes = writer(row.measure(), limit)?;
            row.encode(&mut bytes).ok()?;
            Some(bytes.finish())
        }
        SecretValue::Record(record) => {
            source(w::SourceValue::Record(w::SourceRecord::new(limits, w::SourceRecordParts { record }).ok()?), limit)
        }
        SecretValue::Key(key) => {
            source(w::SourceValue::Key(w::SourceKey::new(limits, w::SourceKeyParts { key }).ok()?), limit)
        }
        SecretValue::Carried { kind, value } => {
            let value = match kind {
                CarriedKind::Key => {
                    w::Carrying::Key(w::CarriedKey::new(limits, w::CarriedKeyParts { key: value }).ok()?)
                }
                CarriedKind::Value => {
                    w::Carrying::Value(w::CarriedValue::new(limits, w::CarriedValueParts { value }).ok()?)
                }
                CarriedKind::Moved => {
                    w::Carrying::Moved(w::CarriedMoved::new(limits, w::CarriedMovedParts { moved: value }).ok()?)
                }
            };
            let row = w::Carried::new(limits, w::CarriedParts { value }).ok()?;
            let mut bytes = writer(row.measure(), limit)?;
            row.encode(&mut bytes).ok()?;
            Some(bytes.finish())
        }
        SecretValue::Application(value) => {
            if value.len() > usize::try_from(limit).expect("map value limit fits usize") {
                return None;
            }
            Some(Box::from(value.expose()))
        }
        SecretValue::Held(_) => None,
    }
}
fn source(value: w::SourceValue, limit: u32) -> Option<Box<[u8]>> {
    let row = w::Source::new(&w::CEILINGS, w::SourceParts { value }).ok()?;
    let mut bytes = writer(row.measure(), limit)?;
    row.encode(&mut bytes).ok()?;
    Some(bytes.finish())
}
pub(super) fn decode(owner: SecretOwner, number: u64, bytes: Box<[u8]>) -> Option<SecretValue> {
    let limits = &w::CEILINGS;
    let mut reader = Reader::new(&bytes);
    match owner {
        SecretOwner::SignIn | SecretOwner::Service => {
            let w::DigestParts { expiry, digest } = w::Digest::decode(limits, &mut reader).ok()?.into_parts();
            let value = SecretValue::Digest { expiry, digest };
            if !accepts(owner, number, &value) {
                return None;
            }
            Some(value)
        }
        SecretOwner::Source => match w::Source::decode(limits, &mut reader).ok()?.into_parts().value {
            w::SourceValue::Record(row) => Some(SecretValue::Record(row.into_parts().record)),
            w::SourceValue::Key(row) => Some(SecretValue::Key(row.into_parts().key)),
        },
        SecretOwner::Application => {
            if number < CARRIED {
                return Some(SecretValue::Application(Secret::new(bytes)));
            }
            if number >= HELD {
                return None;
            }
            let (kind, value) = match w::Carried::decode(limits, &mut reader).ok()?.into_parts().value {
                w::Carrying::Key(row) => (CarriedKind::Key, row.into_parts().key),
                w::Carrying::Value(row) => (CarriedKind::Value, row.into_parts().value),
                w::Carrying::Moved(row) => (CarriedKind::Moved, row.into_parts().moved),
            };
            Some(SecretValue::Carried { kind, value })
        }
    }
}
pub(super) fn counter(next: u64) -> Box<[u8]> {
    let row = w::NextCarried::new(&w::CEILINGS, w::NextCarriedParts { next }).expect("fixed next number");
    let mut bytes = Writer::new(usize::try_from(row.measure()).expect("counter fits usize"));
    row.encode(&mut bytes).expect("measured counter");
    bytes.finish()
}
pub(super) fn next(bytes: &[u8]) -> Option<u64> {
    Some(w::NextCarried::decode(&w::CEILINGS, &mut Reader::new(bytes)).ok()?.next())
}

fn writer(bytes: u32, limit: u32) -> Option<Writer> {
    if bytes > limit {
        return None;
    }
    Some(Writer::new(usize::try_from(bytes).ok()?))
}
