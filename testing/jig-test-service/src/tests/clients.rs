use crate::{PartyOutput, engine, route, tests::fixtures as f};
use jig_domain_core as core;
use jig_domain_people as people;
use jig_test_domain as root;
use skein_io as io;
use skein_lib::{ReplyTo, Time, Token, Wall};

fn listening(engine: &mut crate::Engine) {
    let addr = address();
    let clients = engine.clients.as_mut().expect("clients configured");
    assert!(clients.up(io::Event::Listening { owner: Token::new(4_u64 << 60), listener: Token::new(99), addr }));
}
#[test]
fn the_listener_opens_before_the_restart_begins() {
    let mut service = started(true);
    let engine = f::engine(&mut service);
    waiting_workers(engine);
    engine::iterate(engine, Time::ZERO, Wall::EPOCH);
    assert!(engine.begin);
    assert!(engine.store.above.is_empty());
    take_listener(engine);
    listening(engine);
    before_restart_is_not_ready(engine);
    let workers = engine.workers.as_mut().expect("workers");
    assert!(workers.up(
        Time::ZERO,
        io::Event::Listening { owner: Token::new(1_u64 << 60), listener: Token::new(98), addr: address() },
        &mut engine.events
    ));
    engine::iterate(engine, Time::ZERO, Wall::EPOCH);
    assert!(!engine.begin);
    let Some(jig_protocol_store::Event::Restored { rows, next, .. }) = engine.store.above.pop() else {
        panic!("restart load completed by the real store")
    };
    assert_eq!(rows.len(), 0);
    assert_eq!(next, None);
}
#[test]
fn the_printed_link_reaches_the_caller_once() {
    let mut service = started(true);
    listening(f::engine(&mut service));
    f::turn(&mut service);
    let link = service.links().pop().expect("printed link");
    assert!(link.starts_with(b"http://127.0.0.1:7100/jig/v1/sign-in/link?secret="));
    assert!(service.links().is_empty());
    f::turn(&mut service);
    assert!(service.links().is_empty());
}
const HOST_UPTIME: Time = Time::from_nanos(600_000_000_000);
const UNUSABLE_QUEUE_BYTES: u32 = 0;
#[test]
fn a_printed_link_survives_the_first_host_clock() {
    let mut service = prepared(true);
    service.clock(HOST_UPTIME, Wall::EPOCH);
    let engine = ready_client(&mut service);
    let body = link_body(engine);
    engine.clients.as_mut().expect("clients").iterate(
        HOST_UPTIME,
        Wall::EPOCH,
        &mut engine.events,
        &engine.env.limits.connector,
        engine.env.limits.core.documents.rows,
        &mut engine.accounts,
    );
    let mut peer = Peer::new(engine, b"POST", b"/jig/v1/sign-in/link", &body, None);
    peer.pump(engine);
    let Some(root::Event::Core(core::Event::SignIn { .. })) = engine.events.pop() else {
        panic!("the fresh printed link must reach the root on a host already up for ten minutes")
    };
}
#[test]
fn a_refused_clients_constructor_ends_the_first_iterate() {
    let mut service = prepared(true);
    f::engine(&mut service).clients.as_mut().expect("clients").env.limits.queued_bytes = UNUSABLE_QUEUE_BYTES;
    crate::iterate(&mut service, HOST_UPTIME, Wall::EPOCH);
    let mut problems = jig_protocol_config::Problems::empty();
    problems
        .push(jig_protocol_config::Path::root().member(b"engine").member(b"clients"), jig_protocol_config::What::Bound);
    assert_eq!(service.end(), Some(&crate::End::Stopped(crate::Stop::Start(problems))));
}
#[test]
fn with_no_clients_member_party_outputs_go_to_the_port() {
    let mut service = started(false);
    let engine = f::engine(&mut service);
    route::request(
        engine,
        root::Request::Deliver(root::Delivery::Core(core::Held::PeopleReply {
            to: ReplyTo::new(Token::new(23)),
            reply: people::Reply::SignedOut,
        })),
    );
    let Some(PartyOutput::Held(root::Delivery::Core(core::Held::PeopleReply { to, reply }))) = engine.outputs.pop()
    else {
        panic!("party reply")
    };
    assert_eq!((to.into_token(), reply), (Token::new(23), people::Reply::SignedOut));
    assert!(engine.outputs.is_empty());
}

struct Peer {
    wire: skein_world::stream::Wire,
    owner: Option<Token>,
    closed: u32,
}
impl Peer {
    fn new(engine: &mut crate::Engine, method: &[u8], path: &[u8], body: &[u8], bearer: Option<&[u8]>) -> Self {
        let header = match bearer {
            Some(value) => f::join(&[b"Authorization: Bearer ", value, b"\r\n"]),
            None => Box::default(),
        };
        let length = f::decimal(u64::try_from(body.len()).expect("short body"));
        let bytes = f::join(&[
            method,
            b" ",
            path,
            b" HTTP/1.1\r\nHost: 127.0.0.1:7100\r\nConnection: close\r\nContent-Type: application/octet-stream\r\n",
            &header,
            b"Content-Length: ",
            &length,
            b"\r\n\r\n",
            body,
        ]);
        let mut wire = skein_world::stream::Wire::new(7);
        wire.write(&bytes);
        let clients = engine.clients.as_mut().expect("clients configured");
        assert!(clients.up(io::Event::Accepted {
            owner: Token::new(4_u64 << 60),
            socket: Token::new(100),
            peer: address()
        }));
        Self { wire, owner: None, closed: 0 }
    }
    fn pump(&mut self, engine: &mut crate::Engine) {
        let clients = engine.clients.as_mut().expect("clients configured");
        for _ in 0_u32..256 {
            advance_client(clients, &mut engine.events, &engine.env.limits);
            while let Some(request) = clients.io.pop() {
                match request {
                    io::Request::Listen { owner, addr } => {
                        assert!(clients.up(io::Event::Listening { owner, listener: Token::new(99), addr }));
                    }
                    io::Request::Bind { socket, owner } => {
                        assert_eq!(socket, Token::new(100));
                        self.owner = Some(owner);
                    }
                    io::Request::Stream { stream, down } => {
                        assert_eq!(stream, Token::new(100));
                        self.wire.take(down);
                    }
                    io::Request::Close { .. } | io::Request::Abort { .. } => {
                        self.closed = self.closed.checked_add(1).expect("socket close count");
                        assert!(clients.up(io::Event::Closed { owner: self.owner.expect("bound socket") }));
                    }
                    value @ (io::Request::Connect { .. }
                    | io::Request::Reject { .. }
                    | io::Request::Output { .. }
                    | io::Request::Spawn { .. }
                    | io::Request::Signal { .. }
                    | io::Request::Usage { .. }
                    | io::Request::Window { .. }) => panic!("unexpected client io {value:?}"),
                }
            }
            if let Some(owner) = self.owner
                && let Some(up) = self.wire.answer()
            {
                assert!(clients.up(io::Event::Stream { owner, up }));
            }
        }
    }
}
fn address() -> io::Addr {
    io::Addr::Ip((::core::net::Ipv4Addr::LOCALHOST, 7100).into())
}
fn ready_client(service: &mut crate::Service) -> &mut crate::Engine {
    let engine = f::engine(service);
    let clients = engine.clients.as_mut().expect("clients");
    clients.advance(
        &mut engine.events,
        &engine.env.limits.connector,
        engine.env.limits.core.documents.rows,
        &mut engine.accounts,
    );
    take_listener(engine);
    listening(engine);
    engine.clients.as_mut().expect("clients").starting = None;
    engine
}
fn link_body(engine: &mut crate::Engine) -> Box<[u8]> {
    use jig_client_protocol_wire::v19 as w;
    use skein_lib::{
        Writer,
        base64::{self, Alphabet},
    };
    let link = engine.clients.as_mut().expect("clients").links.pop().expect("printed link");
    let at = skein_lib::bytes::find(&link, b"secret=").expect("query").checked_add(7).expect("query start");
    let secret = base64::decode(Alphabet::Url, link.get(at..).expect("secret bytes"), 32).expect("secret");
    let value =
        w::LinkSignIn::new(&engine.clients.as_ref().expect("clients").env.limits.wire, w::LinkSignInParts { secret });
    let value = value.expect("bounded sign-in record");
    let mut writer = Writer::new(usize::try_from(value.measure()).expect("record bytes"));
    value.encode(&mut writer).expect("measured record");
    writer.finish()
}
#[test]
fn a_party_s_reply_reaches_the_clients_component() {
    use jig_protocol_engine_clients::{SecretOwner, SecretsRequest};
    let mut service = started(true);
    let engine = ready_client(&mut service);
    let body = link_body(engine);
    let mut peer = Peer::new(engine, b"POST", b"/jig/v1/sign-in/link", &body, None);
    peer.pump(engine);
    let Some(root::Event::Core(core::Event::SignIn { reply_to, .. })) = engine.events.pop() else {
        panic!("sign-in routed to root")
    };
    route::request(
        engine,
        root::Request::Deliver(root::Delivery::Core(core::Held::PeopleReply {
            to: reply_to,
            reply: people::Reply::SignedIn { sign_in: 41, person: 1, expires: Wall::from_nanos(100) },
        })),
    );
    peer.pump(engine);
    assert!(engine.outputs.is_empty());
    let Some(SecretsRequest::Keep { owner, .. }) = engine.clients.as_mut().expect("clients").secrets.pop() else {
        panic!("digest keep")
    };
    assert_eq!(owner, SecretOwner::SignIn(41));
}

fn signed_stream(engine: &mut crate::Engine) -> Peer {
    let (bearer, kept) = credential();
    let mut peer = Peer::new(engine, b"GET", b"/jig/v1/stream", &[], Some(&bearer));
    peer.pump(engine);
    authenticate(engine, &mut peer, kept);
    assert!(skein_lib::bytes::find(&peer.wire.received, b"text/event-stream").is_some());
    peer
}
#[test]
fn an_erased_sign_in_record_ends_its_stream_once_durable() {
    use jig_protocol_engine_clients::{SecretOwner, SecretsRequest};
    let mut service = started(true);
    let engine = ready_client(&mut service);
    let mut peer = signed_stream(engine);
    let mut writes = skein_lib::Queue::with_capacity(1);
    writes.push(root::Write::Erase(root::Key::Core(core::Key::People(people::Key::SignIn(41)))));
    engine.clients.as_mut().expect("clients").commit(57, &writes);
    peer.pump(engine);
    assert_eq!(peer.closed, 0);
    assert!(engine.clients.as_mut().expect("clients").secrets.is_empty());
    engine.clients.as_mut().expect("clients").committed(57);
    peer.pump(engine);
    assert_eq!(peer.closed, 1);
    let clients = engine.clients.as_mut().expect("clients");
    let Some(SecretsRequest::Erase { owner }) = clients.secrets.pop() else { panic!("digest erase") };
    assert_eq!(owner, SecretOwner::SignIn(41));
    clients.committed(57);
    assert!(clients.secrets.is_empty());
}

fn take_listener(engine: &mut crate::Engine) {
    let limits = engine.env.limits;
    engine.clients.as_mut().expect("clients").iterate(
        engine.env.now,
        engine.env.wall,
        &mut engine.events,
        &limits.connector,
        limits.core.documents.rows,
        &mut engine.accounts,
    );
    let Some(io::Request::Listen { .. }) = engine.clients.as_mut().expect("clients").io.pop() else {
        panic!("clients listener")
    };
}

fn started(clients: bool) -> crate::Service {
    let mut service = prepared(clients);
    service.clock(Time::ZERO, Wall::EPOCH);
    service
}
fn prepared(clients: bool) -> crate::Service {
    let (mut config, [first, second]) = crate::startup::fixture_connectors(&f::engine_document());
    let core = &mut config.limits.core;
    core.tasks.tasks = 2;
    core.tasks.exemptions = 2;
    core.people.operations = 2;
    core.tasks.project_tasks = 2;
    core.tasks.tree_tasks = 2;
    core.tasks.batch = 1;
    core.tasks.holdings = 2;
    core.tasks.run_inbox = 2;
    core.people.people = 2;
    core.people.sign_ins = 2;
    core.people.pending = 2;
    core.people.waiters = 2;
    core.people.inbox_tasks = 4;
    core.fleet.workers = 2;
    core.fleet.attempts = 2;
    core.fleet.calls = 2;
    core.views.watchers = 4;
    core.views.subjects_per_change = 4;
    core.documents.rows = 2;
    config.limits.journal = skein_lib::JournalLimits { commits: 3, writes: 2000, held: 2000, now: 32, release: 32 };
    config.limits.routes = 16;
    let mut limits = crate::limits::for_profile(jig_protocol_config::Profile::Small, &config.limits);
    limits.root.journal = f::journal(&limits.root);
    limits.root.watermark = root::default_watermark(&limits.root.journal);
    config.limits.journal = limits.root.journal;
    assert!(root::worst_case(&limits.root).is_some(), "test root pricing: {:?}", limits.root);
    let seeds = f::seeds();
    let root = root::Config { core: jig_protocol_config::core(&config, seeds.deployment, seeds.seed), first, second };
    let clients = if clients {
        Some(
            crate::clients::Clients::new(
                &config,
                address(),
                &seeds,
                512,
                &crate::limits::profile_limits(jig_protocol_config::Profile::Small).server,
            )
            .expect("clients fixture"),
        )
    } else {
        None
    };
    let values = crate::values::Values {
        application: skein_lib::Map::with_capacity(0),
        accounts: jig_protocol_accounts::table::Table::new(1, 256),
    };
    let systems = [crate::system::System::new(16, 16), crate::system::System::new(16, 16)];
    let store = crate::store::Store::new(
        &limits.root,
        &crate::store::limits(&limits.root, &limits.store).expect("fixture store admission"),
        &root.core.settings.models,
        f::seeds().installation,
        f::seeds().seed,
    );
    let children = engine::Children { accounts: None, workers: None, clients };
    let mut engine = crate::Engine::new(root, &limits.root, systems, children, values, store);
    f::memory_store(&mut engine);
    crate::Service::new(
        Some(engine),
        skein_lib::List::with_capacity(0),
        limits.io,
        crate::limits::worst_case(&limits).expect("test service pricing"),
        512,
    )
}

fn before_restart_is_not_ready(engine: &mut crate::Engine) {
    use jig_client_protocol_wire::v19 as w;
    engine.clients.as_mut().expect("clients").starting = Some(());
    let body = link_body(engine);
    let mut peer = Peer::new(engine, b"POST", b"/jig/v1/sign-in/link", &body, None);
    peer.pump(engine);
    assert_eq!(engine.clients.as_ref().expect("clients").early.len(), 1, "sign-in is staged before restart");
    engine::iterate(engine, Time::ZERO, Wall::EPOCH);
    peer.pump(engine);
    assert!(engine.begin, "the unopened worker listener still prevents restart");
    assert!(engine.store.above.is_empty(), "the refused sign-in made no commit");
    let value = w::SignInAnswer::decode(
        &engine.clients.as_ref().expect("clients").env.limits.wire,
        &mut skein_lib::Reader::new(response_body(&peer)),
    )
    .expect("sign-in answer");
    assert_eq!(value, w::SignInAnswer::NotReady);
}

fn waiting_workers(engine: &mut crate::Engine) {
    let config = f::config();
    assert!(engine.events.is_empty());
    engine.events = skein_lib::Queue::with_capacity(
        engine
            .max_out
            .checked_mul(2)
            .expect("engine routes")
            .checked_mul(jig_protocol_engine_workers::MAX_OUT_DOWN)
            .expect("worker event room"),
    );
    engine.workers = Some(
        crate::workers::Workers::new(
            &config,
            &f::files(&f::engine_document()),
            f::seeds().deployment,
            &crate::limits::for_profile(jig_protocol_config::Profile::Small, &config.limits).workers,
            engine
                .max_out
                .checked_mul(2)
                .expect("engine routes")
                .checked_mul(jig_protocol_engine_workers::MAX_OUT_DOWN)
                .expect("worker route room"),
        )
        .expect("workers fixture"),
    );
}

fn credential() -> (Box<[u8]>, jig_protocol_engine_clients::Kept) {
    use skein_lib::{
        Secret, Writer,
        base64::{self, Alphabet},
    };
    let mut token = Writer::new(40);
    token.put(&41_u64.to_be_bytes()).expect("number");
    token.put(&[7; 32]).expect("random bytes");
    let token = token.finish();
    let mut digest = Writer::new(32);
    for word in io::digest::digest(&token).0 {
        digest.put(&word.to_be_bytes()).expect("digest word");
    }
    let mut bearer = Writer::new(
        usize::try_from(base64::encoded_len(Alphabet::Url, 40).expect("encoded bearer size")).expect("small bearer"),
    );
    base64::encode(Alphabet::Url, &token, &mut bearer).expect("bearer bytes");
    (
        bearer.finish(),
        jig_protocol_engine_clients::Kept { digest: Secret::new(digest.finish()), expires: Wall::from_nanos(100) },
    )
}

fn authenticate(engine: &mut crate::Engine, peer: &mut Peer, kept: jig_protocol_engine_clients::Kept) {
    use jig_protocol_engine_clients::{Event, SecretOwner, SecretsAnswer, SecretsRequest};
    let clients = engine.clients.as_mut().expect("clients");
    let Some(SecretsRequest::Get { owner }) = clients.secrets.pop() else { panic!("digest get") };
    assert_eq!(owner, SecretOwner::SignIn(41));
    clients.down(Event::Secrets(SecretsAnswer::Got { owner: SecretOwner::SignIn(41), value: Some(kept) }));
    peer.pump(engine);
}
fn response_body(peer: &Peer) -> &[u8] {
    let at = skein_lib::bytes::find(&peer.wire.received, b"\r\n\r\n")
        .expect("HTTP head")
        .checked_add(4)
        .expect("body start");
    peer.wire.received.get(at..).expect("response body")
}
fn application_read(engine: &crate::Engine) -> Box<[u8]> {
    use jig_client_protocol_wire::v19 as w;
    let limits = &engine.clients.as_ref().expect("clients").env.limits.wire;
    let owner = w::Owner::Family(w::Family::new(limits, w::FamilyParts { number: 1 }).expect("test family"));
    let window = w::Window::new(limits, w::WindowParts { rows: 2, after: None }).expect("window");
    let value = w::FamilyRecord::new(
        limits,
        w::FamilyRecordParts {
            scope: w::Scope::Deployment,
            window,
            record: jig_client_test_protocol_wire::codec::read(&jig_test_connector::ClientSubject::Deployment),
        },
    )
    .expect("family record");
    let mut writer = skein_lib::Writer::new(usize::try_from(value.measure()).expect("record bytes"));
    value.encode(&mut writer).expect("measured family record");
    let record = w::Owned::new(
        limits,
        w::OwnedParts { owner, version: jig_client_test_protocol_wire::VERSION, record: writer.finish() },
    )
    .expect("test owned read");
    let mut writer = skein_lib::Writer::new(usize::try_from(record.measure()).expect("read bytes"));
    record.encode(&mut writer).expect("measured read");
    writer.finish()
}
#[test]
fn a_core_refusal_finishes_an_application_read() {
    use jig_client_protocol_wire::v19 as w;
    let mut service = started(true);
    let engine = ready_client(&mut service);
    let body = application_read(engine);
    let (bearer, kept) = credential();
    for (label, why, expected) in [
        ("busy", core::reads::Why::Busy, w::ReadNone::Busy),
        ("signed out", core::reads::Why::SignedOut, w::ReadNone::SignedOut),
    ] {
        let mut peer = Peer::new(engine, b"POST", b"/jig/v1/read", &body, Some(&bearer));
        peer.pump(engine);
        authenticate(engine, &mut peer, kept.clone());
        let Some(root::Event::ReadApplication { reply, .. }) = engine.events.pop() else {
            panic!("{label}: application read")
        };
        route::request(
            engine,
            root::Request::Deliver(root::Delivery::Core(core::Held::Read {
                reply,
                answer: core::reads::Answer::None(why),
            })),
        );
        peer.pump(engine);
        let limits = &engine.clients.as_ref().expect("clients").env.limits.wire;
        let answer =
            w::ReadAnswer::decode(limits, &mut skein_lib::Reader::new(response_body(&peer))).expect("read answer");
        let w::ReadAnswer::None(answer) = answer else { panic!("{label}: refusal") };
        assert_eq!(*answer.value(), expected, "{label}");
        assert_eq!(peer.closed, 1, "{label}");
    }
}

#[test]
fn durable_sign_in_endings_wait_for_output_room() {
    use jig_protocol_engine_clients::{SecretOwner, SecretsRequest};
    let mut service = started(true);
    let engine = ready_client(&mut service);
    let clients = engine.clients.as_mut().expect("clients");
    clients.secrets = skein_lib::Queue::with_capacity(4);
    let mut writes = skein_lib::Queue::with_capacity(2);
    for number in [41, 42] {
        writes.push(root::Write::Erase(root::Key::Core(core::Key::People(people::Key::SignIn(number)))));
    }
    clients.commit(57, &writes);
    clients.committed(57);
    for (label, number) in [("first ending", 41), ("resumed ending", 42)] {
        advance_client(clients, &mut engine.events, &engine.env.limits);
        let Some(SecretsRequest::Erase { owner }) = clients.secrets.pop() else { panic!("{label}: erase") };
        assert_eq!(owner, SecretOwner::SignIn(number), "{label}");
        assert!(clients.secrets.is_empty(), "{label}: one ending fits");
    }
    clients.committed(57);
    advance_client(clients, &mut engine.events, &engine.env.limits);
    assert!(clients.secrets.is_empty(), "each ending was sent once");
}

fn advance_client(
    clients: &mut crate::clients::Clients,
    events: &mut skein_lib::Queue<root::Event>,
    limits: &root::Limits,
) {
    clients.iterate(Time::ZERO, Wall::EPOCH, events, &limits.connector, limits.core.documents.rows, &mut None);
}

#[test]
fn a_kept_digest_reaches_the_store_s_secrets() {
    use jig_protocol_engine_clients::{SecretOwner, SecretsRequest};
    use jig_protocol_store::{Event, Secrets};
    let mut service = started(true);
    let engine = ready_client(&mut service);
    decisions_open(engine);
    let (bearer, value) = credential();
    let expected = jig_protocol_store::SecretValue::Digest {
        expiry: value.expires.as_nanos(),
        digest: value.digest.expose().try_into().expect("credential digest"),
    };
    engine
        .clients
        .as_mut()
        .expect("clients")
        .secrets
        .push(SecretsRequest::Keep { owner: SecretOwner::SignIn(41), value });
    engine::iterate(engine, Time::ZERO, Wall::EPOCH);
    let answer = engine
        .store
        .secrets(Token::new(77), Secrets::Get { owner: jig_protocol_store::SecretOwner::SignIn, number: 41 });
    let Some(Event::Secret { owner, value: Some(value) }) = answer else {
        panic!("same-pass stored digest get: {answer:?}")
    };
    assert_eq!(owner, Token::new(77));
    assert_eq!(value, expected);
    let mut peer = Peer::new(engine, b"GET", b"/jig/v1/stream", &[], Some(&bearer));
    peer.pump(engine);
    assert_eq!(engine.clients.as_ref().expect("clients").secrets.len(), 1, "credential get staged");
    engine::iterate(engine, Time::ZERO, Wall::EPOCH);
    assert!(engine.clients.as_ref().expect("clients").secrets.is_empty(), "get answered in this iterate");
    peer.pump(engine);
    assert!(skein_lib::bytes::find(&peer.wire.received, b"text/event-stream").is_some());
    assert!(engine.outputs.is_empty(), "the digest reached no parties port");
}

fn decisions_open(engine: &mut crate::Engine) {
    for _ in 0_u32..64 {
        engine::iterate(engine, Time::ZERO, Wall::EPOCH);
        if engine.domain.ready() {
            break;
        }
    }
    assert!(engine.domain.ready(), "real store restore and root restart completed");
    assert_eq!(engine.end, None);
}

#[test]
fn the_printed_url_carries_the_secret_the_component_gave() {
    let listener = ::core::net::SocketAddr::new(::core::net::IpAddr::from([127, 0, 0, 1]), 7100);
    let secret = skein_lib::Secret::new(Box::new([0; 32]));
    assert_eq!(
        crate::clients::printed(listener, &secret).as_ref(),
        b"http://127.0.0.1:7100/jig/v1/sign-in/link?secret=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
    );
}

#[test]
fn a_call_past_the_mounts_room_is_answered_busy() {
    use skein_lib::Queue;
    let mut clients = f::server::retained_clients();
    let capacity = crate::limits::profile_limits(jig_protocol_config::Profile::Small).clients_http.calls;
    let mut peers = skein_lib::List::with_capacity(capacity.checked_add(1).expect("first refused call"));
    // The existing Small server has eight connections for four mounted calls.
    // The first socket is the server fixture's existing socket.
    for number in 0..=capacity {
        if number == capacity {
            let (_, owner, _) = peers.as_slice().first().expect("retained call at the mount bound");
            clients.server.up(io::Event::Closed { owner: *owner }, &mut clients.server_out);
            clients.server_requests();
        }
        let socket = Token::new(f::server::SOCKET.raw() + u64::from(number));
        let (owner, call) = f::server::retained_call(&mut clients.server, &mut clients.server_out, socket);
        peers.push((socket, owner, call)).expect("fixture calls");
        clients.server_requests();
    }
    assert_eq!(clients.refused.len(), 1);
    let (response, closes) = f::server::retained_response(&mut clients, &peers);
    assert!(response.as_slice().starts_with(b"HTTP/1.1 503 "), "{response:?}");
    let head_end = skein_lib::bytes::find(response.as_slice(), b"\r\n\r\n").expect("response head");
    assert_eq!(response.as_slice().get(head_end + 4..), Some(b"".as_slice()));
    assert_eq!(closes, 1);
    assert!(clients.refused.is_empty());
    assert_eq!(clients.mount.room(), 0);
    // Retired connections leave their mounted calls until the mount handles the queued ends.
    for (_, owner, _) in peers.iter().take(usize::try_from(capacity).expect("capacity")) {
        clients.server.up(io::Event::Closed { owner: *owner }, &mut clients.server_out);
    }
    clients.server_requests();
    assert_eq!(clients.mount.room(), 0);
    let env = skein_lib::Env {
        now: Time::ZERO,
        wall: Wall::EPOCH,
        limits: crate::limits::profile_limits(jig_protocol_config::Profile::Small).clients_http,
    };
    let mut out = Queue::with_capacity(capacity.checked_mul(2).expect("retirement output"));
    while clients.mount.up(&env, &mut out) {}
    assert_eq!(clients.mount.room(), capacity);
}

#[test]
fn a_moved_record_bound_past_the_wire_value_bound_refuses_start() {
    let (mut config, _) = crate::startup::fixture_connectors(&f::engine_document());
    config.limits.driver.client.document.record_bytes = config.limits.clients.wire.keyed_value_some;
    let Err(problems) = crate::clients::Clients::new(
        &config,
        address(),
        &f::seeds(),
        512,
        &crate::limits::profile_limits(jig_protocol_config::Profile::Small).server,
    ) else {
        panic!("moved file does not fit the wire")
    };
    assert_eq!(problems.problems.get(0).expect("bound refusal").what, jig_protocol_config::What::Bound);
}
