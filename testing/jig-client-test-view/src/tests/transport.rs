//! The view fixtures deliver typed owner values through the client's real byte carriers.
use super::{Domain, Event, Request, family, first};
use jig_client::{Down, Readable, Up, Watchable};
use skein_lib::{Duration, Env, Queue, RequestLimits, Time, Wall};

pub(super) fn drive(root: &mut Domain, event: Event, limits: &jig_client_test_domain::Limits) -> Vec<Request> {
    let up = match event {
        Event::First(first::Event::Read { read, result }) => Up::ReadDone {
            read,
            result: match result {
                first::ReadResult::Value(value) => {
                    jig_client::Readout::Family { family: 1, record: Some(family::wire::encode_document(&value)) }
                }
                first::ReadResult::Unknown => jig_client::Readout::Family { family: 1, record: None },
                first::ReadResult::Busy => jig_client::Readout::Limit,
                first::ReadResult::NotAllowed => jig_client::Readout::Jig(jig_client::contract::ReadResult::None(
                    jig_client::contract::ReadNone::Refused,
                )),
                first::ReadResult::SignedOut => jig_client::Readout::Jig(jig_client::contract::ReadResult::None(
                    jig_client::contract::ReadNone::SignedOut,
                )),
                first::ReadResult::Unsupported => jig_client::Readout::NotServed { family: Some(1), served: None },
            },
        },
        Event::First(first::Event::Watch { watch, event: first::WatchEvent::Delivered(value) }) => {
            Up::FamilyDelivered {
                watch,
                record: jig_client::Family { family: 1, version: 2, bytes: family::wire::encode_document(&value) },
            }
        }
        event => return step(root, event, limits),
    };
    step(root, Event::Client { event: jig_client::Event::Up { up } }, limits)
}
fn step(root: &mut Domain, event: Event, limits: &jig_client_test_domain::Limits) -> Vec<Request> {
    let mut out = Queue::uncapped();
    jig_client_test_domain::step(root, &Env { now: Time::ZERO, wall: Wall::EPOCH, limits: *limits }, event, &mut out);
    let mut rows = Vec::new();
    while let Some(request) = out.pop() {
        // Preserve these tests' owner-level observations while exercising encoded transport work.
        let request = match request {
            Request::Client {
                request: jig_client::Request::Down { down: Down::Read { read, what: Readable::Family { record, .. } } },
            } => Request::First(first::Out::Read { read, subject: subject(&record, true) }),
            Request::Client {
                request:
                    jig_client::Request::Down { down: Down::Open { watch, subject: Watchable::Family { record, .. } } },
            } => Request::First(first::Out::Open { watch, subject: subject(&record, false) }),
            Request::Client { request: jig_client::Request::Down { down: Down::Close { watch } } } => {
                Request::First(first::Out::Close { watch })
            }
            request => request,
        };
        rows.push(request);
    }
    rows
}
fn subject(record: &jig_client::Family, read: bool) -> family::Subject {
    use jig_client_test_protocol_wire::v2 as w;
    let mut reader = skein_lib::Reader::new(&record.bytes);
    let value = if read {
        w::Read::decode(&w::CEILINGS, &mut reader).unwrap().into_parts().value
    } else {
        w::Subject::decode(&w::CEILINGS, &mut reader).unwrap().into_parts().value
    };
    assert_eq!(reader.remaining(), 0);
    family::wire::subject_value(value)
}

pub(super) fn limits() -> jig_client_test_domain::Limits {
    jig_client_test_domain::Limits {
        jig: jig_client_view::Limits {
            watches: 8,
            notices: 2,
            notice_life: Duration::from_secs(5),
            linger: Duration::from_secs(5),
            save_delay: Duration::from_secs(1),
            intents: 2,
            fields: 4,

            objects: 3,
            sources: 8,
            reads: 8,
            parts: 8,
            delegates: 2,
            document_bytes: 1024,
            records: 2,
            kept: 4,
            drafts: 1,
            projects: 2,
            roles: 2,
            text: 32,
            rows: 2,
            segments: 4,
            accounts: jig_client::AccountCounts { accounts: 8, sources: 8, sign_ins: 8 },
            conversation_words: 2,
            navigation_entries: 8,
            updates_count: 99,
        },
        first: first::Limits {
            connector: jig_client_test_domain::connector_values::ConnectorLimits {
                rows: 0,
                paths: 0,
                segments: 0,
                segment_bytes: 0,
                bytes: 4096,
            },
            parts: 8,
            objects: 8,
            reads: 16,
            watches: 16,
            requests: 4,
            rows: 2,
            segments: 4,
            text: 32,
            retry: Duration::from_nanos(10),
        },
        second_parts: 8,
        client: jig_client::Limits {
            domain: jig_client::DomainLimits {
                owners: &[jig_client::Spoken { family: 1, version: 2 }, jig_client::Spoken { family: 2, version: 2 }],
                requests: RequestLimits {
                    requests: 4,
                    bytes: 65536,
                    out: 16,
                    first: Duration::from_nanos(10),
                    most: Duration::from_nanos(80),
                    margin: Duration::from_secs(1),
                },
                ask_bytes: 16384,
                handles: 32,
                watches: 32,
                reads: 32,
                turn_bytes: 32,
                document_bytes: 65536,
                backoff: jig_client::Backoff { first: Duration::from_nanos(10), most: Duration::from_nanos(80) },
                rows: 2,
                segments: 4,
                text: 32,
                turn_text: 32,
                projects: 2,
                fields: 4,
                delegates: 2,
                records: 2,
                kept: 4,
                parts: 10,
                authority: 8,
                accounts: jig_client::AccountCounts { accounts: 8, sources: 8, sign_ins: 8 },
                heartbeat: Duration::from_millis(100),
            },
            exchanges: 8,
        },
    }
}
