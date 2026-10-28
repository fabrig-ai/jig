//! Ops root/view shell adapter; observations come from public requests and trees.
#![allow(clippy::wildcard_imports, reason = "the adapter translates complete owner vocabularies")]
use jig_client_domain::contract::Outcome;
use jig_client_domain::contract::*;
use jig_client_view::Owner;
use jig_client_view::boundary::Offset;
use jig_client_world::harness::Seen;
use jig_client_world::{harness::*, observe};
use jig_ops_client_domain::{
    self as domain, Action, Address, Event, Request, infrastructure as infra, observability as obs,
};
use jig_ops_client_view as view;
use jig_ops_domain_connectors::{Connector, number};
use skein_ui::Act;
use skein_ui::{NodeId, Patch, Role, Tree};

use skein_lib::{Duration, Env, Queue, RequestEnvelope, Time};

use std::{
    collections::BTreeMap,
    sync::{Mutex, OnceLock},
};

/// Reference Ops client composed over the common shell world.
pub struct Ops;
/// Small explicit story capacities, also used to admit scripted whole documents.
#[must_use]
pub fn limits() -> domain::Limits {
    domain::Limits {
        jig: jig_client_view::Limits {
            watches: 8,
            notices: 2,
            notice_life: Duration::from_secs(5),
            // A finite fixture keeps the same before/after-linger behavior at a short scale.
            linger: Duration::from_millis(25),
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
        observability: obs::Limits {
            parts: 8,
            objects: 12,
            reads: 16,
            watches: 16,
            rows: 2,
            segments: 4,
            text: 32,
            retry: Duration::from_millis(10),
            document_bytes: 65536,
        },
        infrastructure: infra::Limits {
            parts: 8,
            objects: 12,
            reads: 16,
            watches: 16,
            requests: 4,
            rows: 2,
            segments: 4,
            text: 32,
            retry: Duration::from_millis(10),
            document_bytes: 65536,
        },
        client: client_limits(),
    }
}
type Books = (jig_client_view::words::Words, view::words::Words, view::words::Words);
type BookCache = Mutex<BTreeMap<(u32, u32, u32), Books>>;
fn phrasebooks(limits: &domain::Limits) -> Books {
    static BOOKS: OnceLock<BookCache> = OnceLock::new();
    let mut books = BOOKS.get_or_init(|| Mutex::new(BTreeMap::new())).lock().expect("fixture books lock");
    books
        .entry((limits.jig.text, limits.observability.text, limits.infrastructure.text))
        .or_insert_with(|| {
            (
                jig_client_view::words::Words::new(
                    jig_client_view::phrases::english::english(limits.jig.text).expect("English fits"),
                    0,
                    0,
                ),
                view::words::Words::english(number(Connector::Observability), limits.observability.text)
                    .expect("incident English fits"),
                view::words::Words::english(number(Connector::Infrastructure), limits.infrastructure.text)
                    .expect("environment English fits"),
            )
        })
        .clone()
}
// Story storage is declared independently of the shipped composition envelope.
// Builder checks catch a fixture growing beyond these bounds.
fn view_bounds(
    limits: &domain::Limits,
    jig: &jig_client_view::words::Words,
    obs: &view::words::Words,
    infra: &view::words::Words,
) -> view::Limits {
    let mut bounds = view::Limits::of(limits, jig, obs, infra).expect("composed view fits");
    assert!(bounds.tree.nodes >= 1024, "world profile fits the composition envelope");
    bounds.tree.nodes = 512;
    bounds.tree.bindings = 512;
    bounds.tree.slots = 128;
    bounds.tree.patches = 5 * bounds.tree.nodes;
    bounds.tree.text_bytes = bounds.tree.nodes.checked_mul(bounds.tree.text_per_node).expect("world text fits");
    bounds
}
// The ordinary world uses one immutable limits/English profile. Keep all
// composition checks in its initializer before admitting clients.
fn fixture_profile() -> view::Limits {
    static PROFILE: OnceLock<view::Limits> = OnceLock::new();
    *PROFILE.get_or_init(|| {
        let limits = limits();
        let (jig, obs, infra) = phrasebooks(&limits);
        view_bounds(&limits, &jig, &obs, &infra)
    })
}
impl App for Ops {
    type Root = domain::Domain;
    type View = view::View;
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
        let limits = limits();
        let (jig, obs, infra) = phrasebooks(&limits);
        let bounds = fixture_profile();
        let mut root = domain::unlinked(&limits, seed);
        domain::configure_arrangement(&mut root, view::arrangement()).expect("ops arrangement before start");
        (root, view::View::new(&bounds, &limits, jig, obs, infra, 1))
    }
    fn start(address: Address, kept: Vec<Self::Record>, offset: i32) -> Event {
        Event::Start { address, kept: super::stories::list(kept), offset: Offset { minutes: offset } }
    }
    fn went(address: Address) -> Event {
        Event::Went { place: domain::Place::Address(address) }
    }
    fn step(root: &mut Self::Root, env: &Env<Self::Limits>, event: Event, out: &mut Queue<Request>) {
        domain::step(root, env, event, out);
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
            Request::Show { address, replace } => Duty::Show { address, replace },
            Request::SignIn { provider } => Duty::SignIn { provider },
            Request::SignOut | Request::Client { request: jig_client::Request::SignOut } => Duty::SignOut,
            Request::Client { .. } => Duty::Wire(request),
            Request::Observability(obs::Out::Read { .. } | obs::Out::Open { .. } | obs::Out::Close { .. })
            | Request::Infrastructure(infra::Out::Read { .. } | infra::Out::Open { .. } | infra::Out::Close { .. }) => {
                panic!("family work routes through the client")
            }
            Request::Fact { .. }
            | Request::Observability(obs::Out::Fact { .. })
            | Request::Infrastructure(infra::Out::Fact { .. }) => Duty::Fact,
        }
    }
    fn record_key(record: &Self::Record) -> Self::Key {
        match record {
            domain::store::Record::Jig(record) => domain::store::Key::Jig(record.key()),
            domain::store::Record::Observability(obs::Record::Place { .. }) => {
                domain::store::Key::Observability(obs::Key::Place)
            }
            domain::store::Record::Infrastructure(infra::Record::Place { .. }) => {
                domain::store::Key::Infrastructure(infra::Key::Place)
            }
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
            | Request::Infrastructure(_)
            | Request::Observability(_) => None,
        }
    }
    fn view_limits(view: &Self::View) -> skein_ui::Limits {
        view.limits().tree
    }
    fn patch_limit(view: &Self::View) -> u32 {
        view.limits().tree.patches
    }
    fn window_changed(event: &Event) -> bool {
        match event {
            Event::Act { action: Action::Jig(act) } | Event::Jig(jig_client_view::Event::Act { action: act }) => {
                observe::jig_window_changed(act)
            }
            Event::Act {
                action: Action::Observability(obs::Act::Go { .. }) | Action::Infrastructure(infra::Act::Go { .. }),
            } => true,
            Event::Start { .. }
            | Event::Went { .. }
            | Event::Act { .. }
            | Event::Client { .. }
            | Event::Jig(_)
            | Event::Observability(_)
            | Event::Infrastructure(_)
            | Event::SignedOut => false,
        }
    }
    fn seen(tree: &Tree) -> Vec<Seen> {
        super::stories::seen(tree)
    }
    fn sent(request: &Request) -> Option<Sent> {
        if let Request::Client {
            request:
                request @ jig_client::Request::Exchange { message: jig_client::Message::Request(bytes), exchange, .. },
        } = request
        {
            use jig_client_protocol_wire::v19 as w;
            let keyed = w::Keyed::decode(&w::CEILINGS, &mut skein_lib::Reader::new(bytes)).expect("keyed Ops bytes");
            if *keyed.owner() == w::Owner::Jig {
                return jig_client_protocol_world::observe::client_sent(request);
            }
            let request = w::FamilyRequest::decode(&w::CEILINGS, &mut skein_lib::Reader::new(keyed.request()))
                .expect("family request");
            let ask = domain::wire::ask_value(request.record()).expect("whole environment ask");
            return Some(infrastructure_sent(*exchange, *keyed.key(), &ask));
        }
        let Request::Client {
            request: jig_client::Request::Down { down: jig_client::Down::Send { attempt, key, request, .. } },
        } = request
        else {
            return None;
        };
        let ask = match request {
            jig_client::Ask::Jig(ask) if matches!(ask.as_ref(), Ask::Seen { .. }) => return None,
            jig_client::Ask::Jig(value) => domain::Ask::Jig(value.clone()),
            jig_client::Ask::Family { record, .. } => {
                domain::Ask::Infrastructure(domain::wire::ask_value(&record.bytes).expect("whole environment ask"))
            }
        };
        Some(match &ask {
            domain::Ask::Jig(ask) => observe::jig_sent(*attempt, *key, ask),
            domain::Ask::Infrastructure(ask) => infrastructure_sent(*attempt, *key, ask),
        })
    }
    fn received(event: &Event) -> Vec<Receipt> {
        match event {
            Event::Jig(event) => observe::jig_received(event),
            Event::Client { event: jig_client::Event::Up { up } } => client_received(up),
            Event::Infrastructure(
                infra::Event::Read { result: infra::ReadResult::Value(doc), .. }
                | infra::Event::Watch { event: infra::WatchEvent::Delivered(doc), .. },
            ) => {
                if let infra::Document::Environment(env) = doc.as_ref()
                    && let Some(end) = env.end
                {
                    vec![Receipt::Card {
                        key: environment_key(env.project, env.number),
                        revisions: vec![(0, end.revision.value)],
                    }]
                } else {
                    vec![]
                }
            }
            Event::SignedOut => vec![Receipt::Parked],
            Event::Start { .. }
            | Event::Went { .. }
            | Event::Act { .. }
            | Event::Observability(_)
            | Event::Infrastructure(_)
            | Event::Client { .. } => vec![],
        }
    }
    fn card(tree: &Tree, node: NodeId) -> Option<CardKey> {
        let at = tree.nodes().iter().position(|n| n.id == node)?;
        let family = tree
            .nodes()
            .iter()
            .enumerate()
            .take(at + 1)
            .filter(|(i, n)| *i + usize::try_from(n.size).expect("subtree") > at)
            .filter_map(|(_, n)| {
                n.key.filter(|k| {
                    matches!(Owner::from_opaque(k.owner), Some(Owner::Application { number: connector }) if connector == number(Connector::Infrastructure))
                        && matches!(k.kind, 2 | 12)
                        && n.role == Role::Item
                })
            })
            .next_back();
        family.map(CardKey::of).or_else(|| observe::jig_card(tree, node))
    }
}
#[must_use]
pub(super) fn environment_key(project: u32, environment: u64) -> CardKey {
    CardKey::of(
        view::infrastructure::key(&infra::Subject {
            scope: infra::Scope::Project(project),
            kind: infra::Kind::Environment(environment),
        })
        .expect("legacy numeric environment key"),
    )
}

fn client_limits() -> jig_client::Limits {
    const OWNERS: [jig_client::Spoken; 2] = [
        jig_client::Spoken { family: number(Connector::Observability), version: jig_ops_client_protocol_wire::VERSION },
        jig_client::Spoken {
            family: number(Connector::Infrastructure),
            version: jig_ops_client_protocol_wire::VERSION,
        },
    ];
    let mut client = jig_client_world::test::limits().client;
    client.domain.owners = &OWNERS;
    client.domain.requests.bytes = 262_144;
    client
}

fn client_received(up: &jig_client::Up) -> Vec<Receipt> {
    use jig_client::Up;
    match up {
        Up::Answer { attempt, envelope, answer, .. } => {
            if *envelope == skein_lib::RequestEnvelope::SignedOut {
                return vec![Receipt::Parked];
            }
            let words = match answer {
                Some(jig_client::Answered::Jig(Answer::Done(Outcome::Owner(
                    jig_client::contract::OwnerOutcome::Said { task, message },
                )))) => Some((*task, *message)),
                _ => None,
            };
            vec![Receipt::Answer {
                attempt: *attempt,
                final_answer: *envelope == skein_lib::RequestEnvelope::Final,
                words,
            }]
        }
        Up::Watched { watch, event } => {
            observe::up_received(&jig_client::Up::Watched { watch: *watch, event: event.clone() })
        }
        Up::ReadDone { read, result: jig_client::Readout::Jig(result) } => {
            observe::up_received(&jig_client::Up::ReadDone {
                read: *read,
                result: jig_client::Readout::Jig(result.clone()),
            })
        }
        Up::FamilyDelivered { record, .. } if record.family == 2 => infrastructure_received(&record.bytes),
        Up::ReadDone { result: jig_client::Readout::Family { family: 2, record: Some(bytes) }, .. } => {
            infrastructure_read_received(bytes)
        }
        Up::Served { .. } | Up::WatchEnded { .. } => observe::up_received(up),
        Up::SignedOut => vec![Receipt::Parked],
        Up::SignedIn
        | Up::NotKept { .. }
        | Up::FamilyServed { .. }
        | Up::FamilyDelivered { .. }
        | Up::ReadDone { .. } => vec![],
    }
}
pub(crate) fn infrastructure_received(bytes: &[u8]) -> Vec<Receipt> {
    let doc = domain::wire::infrastructure::document_value(bytes).expect("whole infrastructure receipt");
    infrastructure_document_received(&doc)
}

pub(crate) fn infrastructure_read_received(bytes: &[u8]) -> Vec<Receipt> {
    let result = domain::wire::infrastructure::read_answer_value(bytes);
    let infra::ReadResult::Value(doc) = result else { return vec![] };
    infrastructure_document_received(&doc)
}

fn infrastructure_document_received(doc: &infra::Document) -> Vec<Receipt> {
    if let infra::Document::Environment(env) = doc
        && let Some(end) = env.end
    {
        vec![Receipt::Card { key: environment_key(env.project, env.number), revisions: vec![(0, end.revision.value)] }]
    } else {
        vec![]
    }
}

fn infrastructure_sent(attempt: skein_lib::Token, key: [u8; 16], ask: &infra::Ask) -> Sent {
    let (card, revision) = match ask {
        infra::Ask::NativeMoveEnd { object, revision, .. } => (
            CardKey::of(skein_ui::Key {
                owner: (jig_client_view::Owner::Application { number: 2 }).opaque(),
                kind: 12,
                a: 0,
                b: object.raw(),
            }),
            revision.value,
        ),
        infra::Ask::MoveEnd { project, environment, revision, .. } => {
            (environment_key(*project, *environment), revision.value)
        }
    };
    Sent { attempt, key, card: Some(card), revision: Some((0, revision)), words: None }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jig_client_protocol_wire::v19 as w;
    use skein_lib::{Overflow, Token, Writer};

    const ATTEMPT: Token = Token::new(7);
    const REQUEST_KEY: [u8; 16] = [3; 16];
    const PROJECT: u32 = 1;
    const TASK: u64 = 8;

    fn encoded<T>(
        value: &T,
        measure: fn(&T) -> u32,
        encode: fn(&T, &mut Writer) -> std::result::Result<(), Overflow>,
    ) -> Box<[u8]> {
        let mut writer = Writer::new(usize::try_from(measure(value)).expect("bounded wire fixture"));
        encode(value, &mut writer).expect("measured wire fixture");
        writer.finish()
    }

    fn requests(ask: jig_client::Ask) -> [Request; 2] {
        let (owner, version, bytes) = match &ask {
            jig_client::Ask::Jig(ask) => {
                let ask =
                    jig_client_protocol_link::translate::asks::ask(ask, &w::CEILINGS).expect("bounded jig fixture");
                (w::Owner::Jig, jig_client_protocol_wire::VERSION, encoded(&ask, w::Ask::measure, w::Ask::encode))
            }
            jig_client::Ask::Family { scope: Scope::Project(project), record } => {
                let owner =
                    w::Family::new(&w::CEILINGS, w::FamilyParts { number: record.family }).expect("family fixture");
                let scope = w::ScopeProject::new(&w::CEILINGS, w::ScopeProjectParts { value: *project })
                    .expect("project fixture");
                let envelope = w::FamilyRequest::new(
                    &w::CEILINGS,
                    w::FamilyRequestParts { scope: w::Scope::Project(scope), record: record.bytes.clone() },
                )
                .expect("bounded family fixture");
                (
                    w::Owner::Family(owner),
                    record.version,
                    encoded(&envelope, w::FamilyRequest::measure, w::FamilyRequest::encode),
                )
            }
            jig_client::Ask::Family { .. } => panic!("fixture uses a project scope"),
        };
        let keyed = w::Keyed::new(
            &w::CEILINGS,
            w::KeyedParts { key: REQUEST_KEY, owner, version, request: bytes, value: None },
        )
        .expect("bounded keyed fixture");
        [
            Request::Client {
                request: jig_client::Request::Down {
                    down: jig_client::Down::Send { attempt: ATTEMPT, key: REQUEST_KEY, request: ask, value: None },
                },
            },
            Request::Client {
                request: jig_client::Request::Exchange {
                    exchange: ATTEMPT,
                    credential: None,
                    message: jig_client::Message::Request(encoded(&keyed, w::Keyed::measure, w::Keyed::encode)),
                },
            },
        ]
    }

    #[test]
    fn both_ops_routes_exclude_top_level_seen() {
        for through in [SeenThrough::Results { through: 1 }, SeenThrough::Conversation { task: TASK, through: 1 }] {
            for request in requests(jig_client::Ask::Jig(Box::new(Ask::Seen { through }))) {
                assert!(Ops::sent(&request).is_none(), "{request:?}");
            }
        }
    }

    #[test]
    fn both_ops_routes_keep_ordinary_no_card_requests() {
        for ask in [
            Ask::Stop { project: PROJECT, task: TASK },
            Ask::Take { project: PROJECT, task: TASK },
            Ask::Compound {
                project: PROJECT,
                operations: jig_client_world::test::list([
                    Operation::Stop { project: PROJECT, task: TASK },
                    Operation::Message {
                        project: PROJECT,
                        task: TASK,
                        words: Text::new(b"continue".as_slice().into(), 32).expect("fixture text"),
                    },
                ]),
            },
        ] {
            for request in requests(jig_client::Ask::Jig(Box::new(ask))) {
                let sent = Ops::sent(&request).expect("ordinary request observed");
                assert_eq!(
                    (sent.attempt, sent.key, sent.card, sent.revision, sent.words),
                    (ATTEMPT, REQUEST_KEY, None, None, None),
                    "{request:?}"
                );
            }
        }
    }

    #[test]
    fn both_ops_routes_keep_infrastructure_requests() {
        let ask = infra::Ask::MoveEnd {
            project: PROJECT,
            environment: TASK,
            revision: Revision { value: 5 },
            until: skein_lib::Wall::from_nanos(100),
        };
        for request in requests(jig_client::Ask::Family {
            scope: Scope::Project(PROJECT),
            record: jig_client::Family {
                family: number(Connector::Infrastructure),
                version: jig_ops_client_protocol_wire::VERSION,
                bytes: domain::wire::ask(&ask).expect("bounded infrastructure fixture"),
            },
        }) {
            let sent = Ops::sent(&request).expect("infrastructure request observed");
            assert_eq!(
                (sent.attempt, sent.key, sent.card, sent.revision, sent.words),
                (ATTEMPT, REQUEST_KEY, Some(environment_key(PROJECT, TASK)), Some((0, 5)), None),
                "{request:?}"
            );
        }
    }

    #[test]
    fn application_read_receipt_unwraps_its_versioned_read_answer() {
        let document = infra::Document::Environment(super::super::stories::environment_document());
        let bytes = domain::wire::infrastructure::read_answer(&infra::ReadResult::Value(Box::new(document)))
            .expect("fixture read answer fits");
        let receipts = client_received(&jig_client::Up::ReadDone {
            read: skein_lib::Token::new(7),
            result: jig_client::Readout::Family { family: 2, record: Some(bytes) },
        });
        let [Receipt::Card { key, revisions }] = receipts.as_slice() else {
            panic!("one versioned environment receipt");
        };
        assert_eq!(*key, environment_key(1, 7));
        assert_eq!(*revisions, vec![(0, 5)]);
    }
    #[test]
    fn native_link_receipts_keep_version_refusals_and_ended_sessions() {
        let cases = [
            (jig_client::Up::Served { served: false, versions: None }, "[OutOfDate]"),
            (
                jig_client::Up::WatchEnded {
                    watch: skein_lib::Token::new(7),
                    end: jig_client::contract::WatchEnd::SignedOut,
                },
                "[WatchEnded { watch: Token(7) }, Parked]",
            ),
        ];
        for (up, expected) in cases {
            assert_eq!(format!("{:?}", client_received(&up)), expected, "{up:?}");
        }
    }
}
