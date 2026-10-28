use crate::{Address, Domain, Event, Limits, Request, first, step, take};
use jig_client::contract::{
    self, Answer, Delivery, DocumentText, HeldRole, Me, MeParty, Needs, PartyKind, WatchEnd, WatchEvent,
};
use jig_client_view::boundary::Offset;
use skein_lib::{Duration, Env, List, Queue, RequestLimits, Time, Token, Wall};
pub(super) const UNOWNED_KEY_NUMBER: u64 = 42;
pub(super) const RECORD_PROJECT: u32 = 3;
pub(super) const RECORD_ITEM: u64 = 5;
const JIG: jig_client_view::Limits = jig_client_view::Limits {
    watches: 4,
    notices: 2,
    notice_life: Duration::from_secs(5),
    linger: Duration::from_secs(5),
    save_delay: Duration::from_secs(1),
    intents: 2,
    fields: 4,

    objects: 2,
    sources: 8,
    reads: 4,
    parts: 10,
    delegates: 2,
    document_bytes: 65536,
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
};
pub(super) fn limits() -> Limits {
    Limits {
        jig: JIG,
        first: first::Limits {
            connector: crate::connector_values::ConnectorLimits {
                rows: 0,
                paths: 0,
                segments: 0,
                segment_bytes: 0,
                bytes: 4096,
            },
            parts: 10,
            text: 32,
            objects: 12,
            watches: 24,
            reads: 24,
            requests: 4,
            rows: 2,
            segments: 4,
            retry: Duration::from_nanos(10),
        },
        second_parts: 8,
        client: jig_client::Limits {
            domain: jig_client::DomainLimits {
                owners: &[
                    jig_client::Spoken { family: 1, version: jig_client_test_protocol_wire::VERSION },
                    jig_client::Spoken { family: 2, version: jig_client_test_protocol_wire::VERSION },
                ],
                requests: RequestLimits {
                    requests: 4,
                    bytes: 262_144,
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
pub(super) fn env() -> Env<Limits> {
    Env { now: Time::ZERO, wall: Wall::EPOCH, limits: limits() }
}
pub(super) fn drive(root: &mut Domain, event: Event) -> Vec<Request> {
    let up = match event {
        Event::First(first::Event::Read { read, result }) => jig_client::Up::ReadDone {
            read,
            result: jig_client::Readout::Family {
                family: crate::FIRST,
                record: match result {
                    first::ReadResult::Value(value) => Some(crate::family::wire::encode_document(&value)),
                    _ => None,
                },
            },
        },
        Event::First(first::Event::Watch { watch, event: first::WatchEvent::Delivered(value) }) => {
            jig_client::Up::FamilyDelivered {
                watch,
                record: jig_client::Family {
                    family: crate::FIRST,
                    version: jig_client_test_protocol_wire::VERSION,
                    bytes: crate::family::wire::encode_document(&value),
                },
            }
        }
        Event::First(first::Event::Ended { watch, end }) => jig_client::Up::WatchEnded { watch, end: family_end(end) },

        Event::SignedOut => jig_client::Up::SignedOut,
        event => return root_step(root, event),
    };
    root_step(root, Event::Client { event: jig_client::Event::Up { up } })
}
pub(super) fn root_step(root: &mut Domain, event: Event) -> Vec<Request> {
    let mut out = Queue::with_capacity(crate::max_out(&limits()));
    step(root, &env(), event, &mut out);
    out_iter(out)
}
pub(super) fn out_iter(mut out: Queue<Request>) -> Vec<Request> {
    let mut rows = Vec::new();
    while let Some(row) = out.pop() {
        rows.push(row);
    }
    rows
}
pub(super) fn taken(root: &mut Domain) -> Vec<Request> {
    let mut out = Queue::with_capacity(crate::max_out(&limits()));
    take(root, &env(), &mut out);
    out_iter(out)
}
pub(super) fn start(root: &mut Domain, kept: List<crate::store::Record>) -> Token {
    let mut rows = drive(root, Event::Client { event: jig_client::Event::Up { up: jig_client::Up::SignedIn } });
    rows.extend(drive(
        root,
        Event::Start {
            address: Address::Jig(jig_client_view::address::Address::Inbox { project: None }),
            kept,
            offset: Offset { minutes: 0 },
        },
    ));
    rows.iter()
        .find_map(|row| match row {
            Request::Client { request: jig_client::Request::Down { down: jig_client::Down::Open { watch, subject: jig_client::Watchable::Jig(contract::Subject::Me) } } } => Some(*watch),
            _ => None,
        })
        .unwrap()
}
pub(super) fn confirm(root: &mut Domain, watch: Token, party: u64) -> Vec<Request> {
    drive(
        root,
        Event::Client { event: jig_client::Event::Up { up: jig_client::Up::Served { served: true, versions: None } } },
    );
    for family in [crate::FIRST, crate::SECOND] {
        drive(
            root,
            Event::Client {
                event: jig_client::Event::Up {
                    up: jig_client::Up::FamilyServed {
                        family,
                        versions: Some(contract::VersionRange {
                            first: jig_client_test_protocol_wire::VERSION,
                            last: jig_client_test_protocol_wire::VERSION,
                        }),
                    },
                },
            },
        );
    }
    let frame = Me {
        providers: List::with_capacity(0),
        providers_cut: false,
        key_bytes: 256,
        redirect_bytes: 4096,
        words_bytes: 65_536,
        amendment_bytes: 65_536,
        party: MeParty {
            number: party,
            kind: PartyKind::Person,
            login: DocumentText::complete(contract::Text::new(Box::from(*b"fixture-login"), 32).unwrap()),
            name: DocumentText::complete(contract::Text::new(Box::from(*b"Pat"), 32).unwrap()),
        },
        projects: List::with_capacity(0),
        needs: Needs { questions: 0, proposals: 0, escalations: 0, person_tasks: 0 },
        needs_total: 0,
        results_seen: 0,
        cut: false,
        updates: contract::Count { value: 0, limited: false },

        retention: Duration::from_secs(1000),
        deployment_role: Some(HeldRole::Policy { role: 1 }),
    };
    drive(
        root,
        Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Watched { watch, event: WatchEvent::Delivered(Delivery::Me(Box::new(frame))) },
            },
        },
    )
}
pub(super) fn ask(root: &mut Domain, asked: Token) -> Vec<Request> {
    drive(
        root,
        Event::Client {
            event: jig_client::Event::Send {
                name: None,
                key: jig_client::Key { asker: jig_client::Asker::JIG, number: asked.raw() },
                request: jig_client::Ask::Jig(Box::new(contract::Ask::Stop { project: 1, task: 9 })),
            },
        },
    )
}
pub(super) fn resource_path(bytes: &[u8]) -> List<Box<[u8]>> {
    let mut rows = List::with_capacity(1);
    rows.push(Box::from(bytes)).unwrap();
    rows
}

pub(super) fn family_request(ask: first::Ask) -> jig_client::Family {
    let (project, value) = crate::family::wire::request(&ask);
    jig_client::Family {
        family: crate::FIRST,
        version: jig_client_test_protocol_wire::VERSION,
        bytes: jig_client_test_protocol_wire::codec::family::request_record(project, value),
    }
}
pub(super) fn family_answer(answer: first::Answer) -> jig_client::Answered {
    match answer {
        first::Answer::Refused(why) => {
            jig_client::Answered::Jig(Answer::Refused { refusal: why.into_refusal(), at: None })
        }
        answer => jig_client::Answered::Family(jig_client::Family {
            family: crate::FIRST,
            version: jig_client_test_protocol_wire::VERSION,
            bytes: crate::family::wire::encode_answer(&answer).unwrap(),
        }),
    }
}

pub(super) fn family_end(end: first::WatchEnd) -> WatchEnd {
    match end {
        first::WatchEnd::Closed => WatchEnd::Closed,
        first::WatchEnd::Dropped => WatchEnd::Dropped,
        first::WatchEnd::Unknown | first::WatchEnd::Unsupported => WatchEnd::Unknown,
        first::WatchEnd::NotAllowed => WatchEnd::NotAllowed,
        first::WatchEnd::Busy => WatchEnd::Busy,
        first::WatchEnd::SignedOut => WatchEnd::SignedOut,
        first::WatchEnd::Stopping => WatchEnd::Stopping,
        first::WatchEnd::SubjectEnded => WatchEnd::SubjectEnded,
    }
}
