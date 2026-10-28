//! Concrete test-application adapter; application vocabulary is read only here.
#![allow(clippy::wildcard_imports, reason = "world adapters translate the complete owner vocabulary")]
use crate::harness::*;
use jig_client_domain::contract::*;
use jig_client_test_domain::{self as domain, Address, Event, Request, first};
use jig_client_test_view as view;
use jig_client_view::boundary::Offset;
use skein_lib::{Duration, Env, List, Queue, RequestEnvelope, RequestLimits, Time};
use skein_ui::Act;
use skein_ui::{NodeId, Patch, Tree};
use std::{
    collections::BTreeMap,
    sync::{Mutex, OnceLock},
};
/// The test application’s client root and view adapter.
pub struct Test;
// Keep the story's before/after-linger transition within one heartbeat.
pub(crate) const AUTHORITY_ENTRIES: u32 = 8;
const LINGER: Duration = Duration::from_millis(25);
/// Returns the small focused-world capacities.
#[must_use]
pub fn limits() -> jig_client_test_domain::Limits {
    jig_client_test_domain::Limits {
        jig: jig_client_view::Limits {
            watches: 8,
            notices: 2,
            notice_life: Duration::from_secs(5),
            linger: LINGER,
            save_delay: Duration::from_millis(10),
            intents: 2,
            fields: 4,

            objects: 3,
            sources: 8,
            reads: 8,
            parts: 8,
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
        },
        first: first::Limits {
            connector: jig_client_test_domain::connector_values::ConnectorLimits {
                rows: 4,
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
            retry: Duration::from_millis(10),
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
                    bytes: 65536,
                    out: 16,
                    first: Duration::from_millis(10),
                    most: Duration::from_millis(80),
                    margin: Duration::from_secs(1),
                },
                ask_bytes: 16384,
                handles: 32,
                watches: 32,
                reads: 32,
                turn_bytes: 32,
                document_bytes: 65536,
                backoff: jig_client::Backoff { first: Duration::from_millis(10), most: Duration::from_millis(80) },
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
                authority: AUTHORITY_ENTRIES,
                accounts: jig_client::AccountCounts { accounts: 8, sources: 8, sign_ins: 8 },
                heartbeat: Duration::from_millis(100),
            },
            exchanges: 8,
        },
    }
}

/// Creates a bounded fixture list with room for all supplied rows.
#[must_use]
pub fn list<T>(rows: impl IntoIterator<Item = T>) -> List<T> {
    let rows: Vec<_> = rows.into_iter().collect();
    let mut out = List::with_capacity(u32::try_from(rows.len()).expect("fixture rows fit"));
    for row in rows {
        assert!(out.push(row).is_ok(), "fixture capacity");
    }
    out
}

/// Check the public decision vocabulary supplied by an application's world.
pub(crate) fn decision_phrasebook(language: &skein_ui::phrases::Language) -> skein_ui::phrases::Phrasebook {
    use skein_ui::phrases::{PhraseLimits, Phrasebook, Segment};
    let mut segments = 0;
    let mut text_bytes = 0;
    for phrase in &language.phrases {
        for form in &phrase.forms {
            segments += u32::try_from(form.segments.len()).expect("bounded world phrase");
            for segment in &form.segments {
                if let Segment::Text(text) = segment {
                    text_bytes += u32::try_from(text.len()).expect("bounded world words");
                }
            }
        }
    }
    Phrasebook::load(
        jig_client::words::phrase_shapes(0),
        Box::new([language.clone()]),
        &PhraseLimits {
            languages: 1,
            phrases: jig_client::words::PHRASES,
            holes: 3,
            segments,
            text_bytes,
            hole_bytes: 0,
        },
    )
    .expect("world's public words are complete and checked")
}

// Checked immutable books and envelopes are shared by every equal fixture.
type Books = (jig_client_view::words::Words, view::words::Words);
pub(crate) fn phrasebooks(limits: &domain::Limits) -> &'static Books {
    static BOOKS: OnceLock<Mutex<BTreeMap<(u32, u32), &'static Books>>> = OnceLock::new();
    let mut books = BOOKS.get_or_init(|| Mutex::new(BTreeMap::new())).lock().expect("fixture books lock");
    books.entry((limits.jig.text, limits.first.text)).or_insert_with(|| {
        Box::leak(Box::new((
            view::connector_values_words::english(limits.jig.text).expect("composed application English fits"),
            view::words::Words::english(limits.first.text).expect("family English fits"),
        )))
    })
}
struct Fixture {
    books: &'static Books,
    bounds: view::Limits,
    worst_case: u64,
}
fn fixture(limits: &domain::Limits) -> &'static Fixture {
    fixture_with(limits, false)
}
type Fixtures = Vec<(domain::Limits, bool, &'static Fixture)>;
fn fixture_with(limits: &domain::Limits, arranged: bool) -> &'static Fixture {
    static FIXTURES: OnceLock<Mutex<Fixtures>> = OnceLock::new();
    let mut fixtures = FIXTURES.get_or_init(|| Mutex::new(Vec::new())).lock().expect("fixture envelopes lock");
    if let Some((_, _, fixture)) = fixtures.iter().find(|(key, mode, _)| key == limits && *mode == arranged) {
        return fixture;
    }
    let books = phrasebooks(limits);
    let bounds = view::Limits::for_world(limits, &books.0, &books.1, if arranged { 1024 } else { 512 }, 256)
        .expect("view bounds fit");
    let worst_case = domain::worst_case(limits).expect("root worst case")
        // The shell model is bounded by another renderer envelope.
        + 2 * view::worst_case(&bounds, limits, &books.0, &books.1).expect("view worst case");
    let fixture = Box::leak(Box::new(Fixture { books, bounds, worst_case }));
    fixtures.push((*limits, arranged, fixture));
    fixture
}
/// Constructs the test application's root and view with one declared bound.
#[must_use]
pub fn new(limits: &domain::Limits, seed: u64) -> (domain::Domain, view::View<'static>) {
    from_fixture(limits, seed, fixture(limits), false)
}
fn from_fixture(
    limits: &domain::Limits,
    seed: u64,
    fixture: &'static Fixture,
    linked: bool,
) -> (domain::Domain, view::View<'static>) {
    let (words, family) = fixture.books;
    let root = if linked { domain::new(limits, seed) } else { domain::unlinked(limits, seed) };
    (root, view::View::new(&fixture.bounds, limits, words, family, first::Address { project: 1, item: 1 }))
}
/// The same fixture with the application's byte transport attached.
#[must_use]
pub fn linked(seed: u64) -> (domain::Domain, view::View<'static>) {
    linked_with(&limits(), seed)
}
/// A native fixture with the caller's declared request and document bounds.
#[must_use]
pub fn linked_with(limits: &domain::Limits, seed: u64) -> (domain::Domain, view::View<'static>) {
    from_fixture(limits, seed, fixture(limits), true)
}
/// Constructs the one arranged client used by the all-sections story.
#[must_use]
pub fn arranged(seed: u64) -> (domain::Domain, view::View<'static>) {
    use jig_client_view::{address::TaskSection, page::Arrangement};
    let limits = limits();
    let (mut root, view) = from_fixture(&limits, seed, fixture_with(&limits, true), false);
    domain::configure_arrangement(
        &mut root,
        Arrangement {
            task: list([
                TaskSection::Overview,
                TaskSection::Activity,
                TaskSection::Plan,
                TaskSection::History,
                TaskSection::Budget,
                TaskSection::Panels,
            ]),
            project: list([]),
        },
    )
    .expect("arrangement is valid before start");
    (root, view)
}
impl App for Test {
    type Root = domain::Domain;
    type View = view::View<'static>;
    type Limits = domain::Limits;
    type Event = Event;
    type Request = Request;
    type Record = domain::store::Record;
    type Key = domain::store::Key;
    type Address = Address;
    fn limits() -> Self::Limits {
        limits()
    }
    fn new(seed: u64) -> (Self::Root, Self::View) {
        new(&limits(), seed)
    }
    fn start(address: Address, kept: Vec<Self::Record>, offset: i32) -> Event {
        Event::Start { address, kept: list(kept), offset: Offset { minutes: offset } }
    }
    fn went(address: Address) -> Event {
        Event::Went { place: domain::Place::Address(address) }
    }
    fn step(root: &mut Self::Root, env: &Env<Self::Limits>, event: Event, out: &mut Queue<Request>) {
        if matches!(&event, Event::Client { event: jig_client::Event::Up { up: jig_client::Up::Served { .. } } }) {
            for family in [domain::FIRST, domain::SECOND] {
                domain::step(
                    root,
                    env,
                    Event::Client {
                        event: jig_client::Event::Up {
                            up: jig_client::Up::FamilyServed {
                                family,
                                versions: Some(VersionRange {
                                    first: jig_client_test_protocol_wire::VERSION,
                                    last: jig_client_test_protocol_wire::VERSION,
                                }),
                            },
                        },
                    },
                    out,
                );
            }
        }
        domain::step(root, env, client_event(event), out);
    }
    fn fire(root: &mut Self::Root, env: &Env<Self::Limits>, out: &mut Queue<Request>) {
        domain::fire(root, env, out);
    }
    fn take(root: &mut Self::Root, env: &Env<Self::Limits>, out: &mut Queue<Request>) {
        domain::take(root, env, out);
    }
    fn next_deadline(root: &Self::Root) -> Option<Time> {
        domain::next_deadline(root)
    }
    fn build(view: &mut Self::View, root: &Self::Root, patches: &mut Queue<Patch>) {
        view::build(view, root, patches);
    }
    fn tree(view: &Self::View) -> &Tree {
        view.tree()
    }
    fn act(view: &mut Self::View, node: NodeId, act: Act) -> Option<Event> {
        view::decode(view, node, act).map(|action| Event::Act { action })
    }
    fn duty(request: Request) -> Duty<Self> {
        match request {
            Request::Save { record } => Duty::Store(StoreWrite::Save(record)),
            Request::Erase { key } => Duty::Store(StoreWrite::Erase(key)),
            Request::Client { request: jig_client::Request::SignOut } | Request::SignOut => Duty::SignOut,
            Request::Client { .. } => Duty::Wire(request),
            Request::Show { address, replace } => Duty::Show { address, replace },
            Request::SignIn { provider } => Duty::SignIn { provider },
            Request::First(_) | Request::Fact { .. } => Duty::Fact,
        }
    }
    fn record_key(record: &Self::Record) -> Self::Key {
        match record {
            domain::store::Record::Jig(record) => domain::store::Key::Jig(record.key()),
            domain::store::Record::First(first::Record::Place { .. }) => domain::store::Key::First(first::Key::Place),
            domain::store::Record::Client(record) => domain::store::Key::Client(record.key.clone()),
            domain::store::Record::Unlinked(record) => domain::store::Key::Unlinked(record.key),
        }
    }
    fn unsent(request: Request) -> Option<Event> {
        match request {
            Request::Client { request } => {
                let event = match request {
                    jig_client::Request::Down { down: jig_client::Down::Send { attempt, .. } } => {
                        jig_client::Event::Up {
                            up: jig_client::Up::Answer {
                                attempt,
                                envelope: RequestEnvelope::Lost,
                                answer: None,
                                why: None,
                            },
                        }
                    }
                    jig_client::Request::Down {
                        down:
                            jig_client::Down::Open { watch, .. }
                            | jig_client::Down::Close { watch }
                            | jig_client::Down::Continue { watch, .. },
                    } => jig_client::Event::Up { up: jig_client::Up::WatchEnded { watch, end: WatchEnd::Dropped } },
                    jig_client::Request::Down { down: jig_client::Down::Read { read, .. } } => jig_client::Event::Up {
                        up: jig_client::Up::ReadDone { read, result: jig_client::Readout::Limit },
                    },
                    jig_client::Request::Exchange { exchange, .. } => jig_client::Event::Lost { exchange },
                    jig_client::Request::OpenStream { .. } => jig_client::Event::StreamGone,
                    jig_client::Request::SignOut => jig_client::Event::SignOutLost,
                    jig_client::Request::Authorized(_)
                    | jig_client::Request::ServiceCredential { .. }
                    | jig_client::Request::CloseStream
                    | jig_client::Request::Save { .. }
                    | jig_client::Request::Erase { .. }
                    | jig_client::Request::Told { .. }
                    | jig_client::Request::Down {
                        down: jig_client::Down::Save { .. } | jig_client::Down::Erase { .. },
                    } => return None,
                };
                Some(Event::Client { event })
            }
            Request::Save { .. }
            | Request::Erase { .. }
            | Request::Show { .. }
            | Request::SignOut
            | Request::SignIn { .. }
            | Request::Fact { .. }
            | Request::First(_) => None,
        }
    }
    fn view_limits(view: &Self::View) -> skein_ui::Limits {
        view.limits().tree
    }
    fn patch_limit(view: &Self::View) -> u32 {
        view.limits().tree.patches
    }
    fn worst_case(limits: &Self::Limits) -> u64 {
        fixture(limits).worst_case
    }
    fn window_changed(event: &Self::Event) -> bool {
        crate::observe::window_changed(event)
    }
    fn seen(tree: &Tree) -> Vec<crate::harness::Seen> {
        crate::observe::seen(tree)
    }
    fn decision(request: &Request) -> Option<&Ask> {
        match request {
            Request::Client {
                request:
                    jig_client::Request::Down { down: jig_client::Down::Send { request: jig_client::Ask::Jig(ask), .. } },
            } => Some(ask),
            Request::Save { .. }
            | Request::Erase { .. }
            | Request::Client { .. }
            | Request::First(_)
            | Request::Show { .. }
            | Request::SignIn { .. }
            | Request::SignOut
            | Request::Fact { .. } => None,
        }
    }
    fn sent(request: &Request) -> Option<Sent> {
        crate::observe::sent(request)
    }
    fn received(event: &Event) -> Vec<Receipt> {
        crate::observe::received(event)
    }
    fn card(tree: &Tree, node: NodeId) -> Option<CardKey> {
        crate::observe::card(tree, node)
    }
}

/// Decode the transport's whole optional-value application request.
#[must_use]
pub fn family_request(record: &jig_client::Family) -> domain::family::Ask {
    use jig_client_test_protocol_wire::v2 as w;
    let mut reader = skein_lib::Reader::new(&record.bytes);
    let request = w::Request::decode(&w::CEILINGS, &mut reader).expect("whole application request").into_parts();
    assert_eq!(reader.remaining(), 0);
    let value = request.value.into_parts();
    domain::family::Ask::Touch {
        project: request.project,
        item: value.object,
        revision: Revision { value: value.revision },
        parameters: value.value,
    }
}
/// The typed test transport retains every supplied family subject field.
#[must_use]
pub fn family_subject(record: &jig_client::Family, read: bool) -> domain::family::Subject {
    use jig_client_test_protocol_wire::v2 as w;
    let mut reader = skein_lib::Reader::new(&record.bytes);
    let value = if read {
        w::Read::decode(&w::CEILINGS, &mut reader).expect("whole family read").into_parts().value
    } else {
        w::Subject::decode(&w::CEILINGS, &mut reader).expect("whole family subject").into_parts().value
    };
    assert_eq!(reader.remaining(), 0);
    domain::family::wire::subject_value(value)
}
/// Admission refusals stay on Jig's entrance wire; family answers use its codec.
#[must_use]
pub fn family_answer(answer: domain::family::Answer) -> jig_client::Answered {
    match answer {
        domain::family::Answer::Refused(why) => {
            jig_client::Answered::Jig(Answer::Refused { refusal: why.into_refusal(), at: None })
        }
        value @ (domain::family::Answer::Touched { .. }
        | domain::family::Answer::Moved { .. }
        | domain::family::Answer::Gone
        | domain::family::Answer::Connector(_)) => jig_client::Answered::Family(jig_client::Family {
            family: domain::FIRST,
            version: jig_client_test_protocol_wire::VERSION,
            bytes: domain::family::wire::encode_answer(&value).expect("whole application answer"),
        }),
    }
}
fn client_event(event: Event) -> Event {
    let up = match event {
        Event::Client { event: jig_client::Event::Up { up: jig_client::Up::Served { served, versions: None } } } => {
            jig_client::Up::Served { served, versions: None }
        }
        Event::Client { event: jig_client::Event::Up { up: jig_client::Up::Watched { watch, event } } } => {
            jig_client::Up::Watched { watch, event }
        }
        Event::Client { event: jig_client::Event::Up { up: jig_client::Up::WatchEnded { watch, end } } } => {
            jig_client::Up::WatchEnded { watch, end }
        }
        Event::Client {
            event:
                jig_client::Event::Up { up: jig_client::Up::ReadDone { read, result: jig_client::Readout::Jig(result) } },
        } => jig_client::Up::ReadDone { read, result: jig_client::Readout::Jig(result) },
        Event::SignedOut | Event::Client { event: jig_client::Event::Up { up: jig_client::Up::SignedOut } } => {
            jig_client::Up::SignedOut
        }
        Event::Client { event: jig_client::Event::SignOutLost } => {
            return Event::Client { event: jig_client::Event::SignOutLost };
        }
        Event::First(first::Event::Read { read, result }) => jig_client::Up::ReadDone {
            read,
            result: match result {
                first::ReadResult::Value(value) => jig_client::Readout::Family {
                    family: domain::FIRST,
                    record: Some(domain::family::wire::encode_document(&value)),
                },
                first::ReadResult::Unknown => jig_client::Readout::Family { family: domain::FIRST, record: None },
                first::ReadResult::Busy => jig_client::Readout::Limit,
                first::ReadResult::Unsupported => {
                    jig_client::Readout::NotServed { family: Some(domain::FIRST), served: None }
                }
                first::ReadResult::NotAllowed => jig_client::Readout::Jig(ReadResult::None(ReadNone::Refused)),
                first::ReadResult::SignedOut => jig_client::Readout::Jig(ReadResult::None(ReadNone::SignedOut)),
            },
        },
        Event::First(first::Event::Watch { watch, event: domain::family::WatchEvent::Delivered(value) }) => {
            jig_client::Up::FamilyDelivered {
                watch,
                record: jig_client::Family {
                    family: domain::FIRST,
                    version: jig_client_test_protocol_wire::VERSION,
                    bytes: domain::family::wire::encode_document(&value),
                },
            }
        }
        Event::First(first::Event::Watch { watch, event: domain::family::WatchEvent::Opened }) => {
            jig_client::Up::Watched { watch, event: WatchEvent::Opened }
        }
        Event::First(first::Event::Watch { watch, event: domain::family::WatchEvent::Alive }) => {
            jig_client::Up::Watched { watch, event: WatchEvent::Alive }
        }
        Event::First(first::Event::Ended { watch, end }) => jig_client::Up::WatchEnded {
            watch,
            end: match end {
                first::WatchEnd::Closed => WatchEnd::Closed,
                first::WatchEnd::Dropped => WatchEnd::Dropped,
                first::WatchEnd::Stopping => WatchEnd::Stopping,
                first::WatchEnd::Unknown | first::WatchEnd::Unsupported => WatchEnd::Unknown,
                first::WatchEnd::NotAllowed => WatchEnd::NotAllowed,
                first::WatchEnd::Busy => WatchEnd::Busy,
                first::WatchEnd::SubjectEnded => WatchEnd::SubjectEnded,
                first::WatchEnd::SignedOut => WatchEnd::SignedOut,
            },
        },
        event @ (Event::Start { .. }
        | Event::Went { .. }
        | Event::Act { .. }
        | Event::Client { .. }
        | Event::Jig(_)
        | Event::First(_)) => return event,
    };
    Event::Client { event: jig_client::Event::Up { up } }
}

mod native;
pub(crate) use native::up;
