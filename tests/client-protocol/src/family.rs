//! The client's family records, retaining the originally scoped route.
use jig_client_domain::Family;
use jig_client_test_domain::family::wire as application;
use jig_client_test_domain::{connector_values::ConnectorDocument, family as d, first};
use jig_client_test_protocol_wire::codec;
use jig_client_test_protocol_wire::v2 as w;
use skein_lib::{Reader, Writer};

#[must_use]
pub fn owned(bytes: Box<[u8]>) -> Family {
    Family { family: 1, version: jig_client_test_protocol_wire::VERSION, bytes }
}
/// Carry optional parameters without manufacturing a connector value.
#[must_use]
pub fn request(value: &first::Ask) -> Family {
    let (project, value) = application::request(value);
    owned(codec::family::request_record(project, value))
}
/// Carry the complete family subject.
#[must_use]
pub fn subject(value: d::Subject) -> Family {
    let (_, subject) = application::subject(value);
    owned(codec::family::subject(subject))
}
/// Carry the complete family subject.
#[must_use]
pub fn read(value: d::Subject) -> Family {
    let (_, subject) = application::subject(value);
    owned(codec::family::read(subject))
}
#[must_use]
pub fn answer(value: &Family) -> Option<first::Answer> {
    if value.family != 1 || value.version != jig_client_test_protocol_wire::VERSION {
        return None;
    }
    let value = codec::outcome_value(&value.bytes, &crate::fixtures::owner_limits().connector).ok()?;
    Some(first::Answer::Connector(application::answer(value)))
}
#[must_use]
pub fn document(subject: d::Subject, bytes: &[u8]) -> Option<d::Document> {
    let value = codec::document_value(bytes, &crate::fixtures::owner_limits().connector).ok()?;
    let value: ConnectorDocument =
        application::document(subject, value, &jig_client_world::test::limits().first.connector).ok()?;
    Some(d::Document::Connector(value))
}
#[must_use]
pub fn saved(value: &first::Record) -> Box<[u8]> {
    let first::Record::Place { version, party, address } = value;
    let record = w::Place::new(
        &w::CEILINGS,
        w::PlaceParts { store_version: *version, party: *party, project: address.project, item: address.item },
    )
    .expect("bounded place record");
    let mut writer = Writer::new(usize::try_from(record.measure()).expect("bounded encoding"));
    record.encode(&mut writer).expect("measured room");
    writer.finish()
}
#[must_use]
pub fn restored(bytes: &[u8]) -> Option<first::Record> {
    let mut reader = Reader::new(bytes);
    let record = w::Place::decode(&w::CEILINGS, &mut reader).ok()?;
    if reader.remaining() != 0 {
        return None;
    }
    let w::PlaceParts { store_version: version, party, project, item } = record.into_parts();
    Some(first::Record::Place { version, party, address: first::Address { project, item } })
}
#[must_use]
pub fn end(value: jig_client_domain::contract::WatchEnd) -> first::WatchEnd {
    use jig_client_domain::contract::WatchEnd as w;
    match value {
        w::Closed => first::WatchEnd::Closed,
        w::Dropped => first::WatchEnd::Dropped,
        w::NotServed => first::WatchEnd::Unsupported,
        w::Unknown | w::TooLinked { .. } => first::WatchEnd::Unknown,
        w::SignedOut => first::WatchEnd::SignedOut,
        w::NotAllowed => first::WatchEnd::NotAllowed,
        w::SubjectEnded => first::WatchEnd::SubjectEnded,
        w::Stopping => first::WatchEnd::Stopping,
        w::Busy => first::WatchEnd::Busy,
    }
}
#[must_use]
pub fn read_result(subject: d::Subject, bytes: &[u8]) -> first::ReadResult {
    let answer = codec::read_answer_value(bytes, &crate::fixtures::owner_limits().connector);
    match answer {
        Ok(Ok(value)) => application::document(subject, value, &jig_client_world::test::limits().first.connector)
            .map_or(first::ReadResult::Unsupported, |value| first::ReadResult::Value(d::Document::Connector(value))),
        Ok(Err(jig_test_connector::DocumentMissing::Unknown)) | Err(_) => first::ReadResult::Unknown,
        Ok(Err(jig_test_connector::DocumentMissing::NotAllowed)) => first::ReadResult::NotAllowed,
    }
}

#[must_use]
pub fn read_none(value: &jig_client_domain::contract::ReadResult) -> first::ReadResult {
    use jig_client_domain::contract::{ReadNone, ReadResult};
    match value {
        ReadResult::None(
            ReadNone::Unknown
            | ReadNone::Gone
            | ReadNone::TooLinked { .. }
            | ReadNone::TooLarge { .. }
            | ReadNone::Malformed,
        ) => first::ReadResult::Unknown,
        ReadResult::None(ReadNone::Refused) => first::ReadResult::NotAllowed,
        ReadResult::None(ReadNone::Busy | ReadNone::Unreachable) => first::ReadResult::Busy,
        ReadResult::None(ReadNone::SignedOut) => first::ReadResult::SignedOut,
        ReadResult::Value(_) => first::ReadResult::Unsupported,
    }
}
