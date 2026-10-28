//! One process composing the real child, translation, table and driver,
//! and the secrets' store that keeps the driver's records (llm.md 5 and 8;
//! store.md 9) and the installation's identifier (store.md 4.4 item 9).
use crate::fixtures::{self, ACCOUNT_CHAT, ROOM};
use crate::world::{Installation, Story};
use jig_domain_accounts as child;
use jig_domain_core as core;
use jig_protocol_accounts::{
    finishes, installation, moves, party,
    records::{self, Records},
    restart,
    table::Table,
    translate::{self, Up},
};
use jig_protocol_store as store;
use skein_io::{self as io, kernel};
use skein_lib::{Env, List, Map, Queue, Secret, Time, Token, Wall, bytes, sha256};
use skein_oauth_accounts as driver;
use skein_world::Host;
use std::net::SocketAddr;

/// A store component over empty memory stores.
fn memory(limits: &store::Limits) -> store::Store {
    store::Store::over(
        limits,
        fixtures::APPLICATION,
        &[],
        store::Stores {
            store: skein_kv::Store::memory(&limits.store),
            secrets: skein_kv::Store::memory(&limits.secrets),
        },
    )
}

/// The store's answers, by what was asked; a replace is answered under its
/// source's number.
const OPEN: Token = Token::new(1);
const LOAD: Token = Token::new(2);
const GET: Token = Token::new(3);
const SOURCE: u64 = 1 << 32;
const MOVE_ADOPT: u64 = 1 << 33;
const SWEEP: Token = Token::new(1 << 34);
const MOVE_OUT: u64 = 1 << 35;
const MOVE_BACK: u64 = 1 << 36;
const MOVE_IN: u64 = 1 << 37;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Heard {
    Granted { account: u32, generation: u64 },
    Expiring { account: u32, generation: u64 },
    Failed { account: u32, failure: child::Failure },
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Trace {
    Requested(child::ToComponent),
    /// The store held this generation of the source's record at start.
    Loaded {
        account: u32,
        generation: u64,
    },
    /// The store answered that this generation of the record is durable.
    Kept {
        account: u32,
        generation: u64,
    },
    Heard(Heard),
    /// The driver asked for a sign-in's visit.
    Visit {
        account: u32,
    },
    /// The driver ended a sign-in, cancelled.
    SignInCancelled {
        account: u32,
    },
    Closed,
}
/// A value the table lent a run, by its digest.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Lent {
    pub account: u32,
    pub generation: u64,
    pub digest: [u8; 32],
}
#[derive(Debug)]
enum Erasing {
    Revoked,
    SignOut(bool),
    Removing,
}
pub struct Owner {
    io: io::Io,
    component: Option<driver::Component>,
    limits: driver::Limits,
    child: child::Domain,
    child_limits: child::Limits,
    child_out: Queue<child::Request>,
    table: Table,
    events: Queue<io::Event>,
    requests: Queue<io::Request>,
    /// The owner keeper files nothing; the driver's calls take the queue.
    files: Queue<driver::FileLower>,
    above: Queue<driver::Event>,
    store: store::Store,
    store_limits: store::Limits,
    store_above: Queue<store::Event>,
    below: store::Below,
    records: Records,
    moves: moves::Moves,
    finishes: finishes::Finishes,
    paste: [u8; 1024],
    paste_len: usize,
    paste_wrong: Option<()>,
    sign_in_again_requested: Option<()>,
    finish_carried: Option<u64>,
    move_out: Queue<moves::Next>,
    providers: Map<u32, driver::Account>,
    source_providers: Map<u32, u32>,
    inventory: restart::Inventory,
    move_mode: Option<()>,
    move_events: Queue<child::Event>,
    /// Test client's output and preallocated input; the referee only copies bytes.
    moved_file: Option<(u32, Secret)>,
    move_input: Box<[u8]>,
    move_input_len: Option<usize>,
    carried_move: Option<u64>,
    lent: Vec<Lent>,
    last_kept: Option<Secret>,
    completions: Queue<kernel::Complete>,
    submissions: Queue<kernel::Submit>,
    pub(crate) address: Option<SocketAddr>,
    heard: Vec<Heard>,
    facts: Vec<child::Fact>,
    trace: Vec<Trace>,
    close_requested: Option<()>,
    reject_requested: Option<()>,
    closing: bool,
    closed: bool,
    usable: bool,
    now: Time,
    story: Story,
    account: u32,
    /// The installation's identifier the store answered at open.
    installation: [u8; 16],
    /// The URL the driver asked to be visited.
    visit: Option<Box<[u8]>>,
    ended: Vec<child::Ended>,
    answers: Vec<child::Answer>,
    removed: Vec<u32>,
    erased: Option<()>,
    erasing: Map<u32, Erasing>,
    account_sign_out_asked: Option<()>,
}
impl std::fmt::Debug for Owner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Owner")
            .field("heard", &self.heard)
            .field("trace", &self.trace)
            .field("closed", &self.closed)
            .finish_non_exhaustive()
    }
}
impl Owner {
    /// The owner over an open memory store holding `held` as its source's
    /// record, as an earlier run left it, and `installation.kept` as the
    /// installation's identifier if an earlier start kept one.
    #[expect(
        clippy::too_many_lines,
        reason = "the explicit domain and protocol fixture restore the same source identities"
    )]
    pub(crate) fn new(story: Story, held: Option<&[u8]>, installation: Installation) -> Self {
        let mut limits = fixtures::limits();
        if story == Story::AccountSignInFull {
            limits.accounts = 1;
        }
        if story == Story::SignInAgain {
            // A cancelled listener counts until it settles (skein oauth.md 6.5): room beside the live sign-in.
            limits.listeners = 2;
            limits.io.sockets = 2 * limits.listeners + limits.exchanges;
        }
        let child_limits = fixtures::child_limits();
        let account = ACCOUNT_CHAT;
        let store_limits = fixtures::store_limits();
        let mut owner = Self {
            io: io::Io::new(&limits.io),
            component: None,
            child: child::Domain::new(
                &child_limits,
                child::Providers {
                    providers: Box::new([child::Provider {
                        number: 1,
                        name: b"Fake".as_slice().into(),
                        signed_in_plan: true,
                        manage_usage: Some(b"https://usage.example/".as_slice().into()),
                        models: Box::new([]),
                        offers: child::Offers { sign_in: true, key: false, programs: Box::new([]) },
                    }]),
                },
            ),
            child_out: Queue::with_capacity(ROOM),
            child_limits,
            table: Table::new(limits.accounts, limits.client.document.token_bytes),
            events: Queue::with_capacity(ROOM),
            requests: Queue::with_capacity(ROOM),
            files: Queue::with_capacity(ROOM),
            above: Queue::with_capacity(ROOM),
            completions: Queue::with_capacity(ROOM),
            submissions: Queue::with_capacity(ROOM),
            store: memory(&store_limits),
            store_limits,
            store_above: Queue::uncapped(),
            below: store::Below {
                store: Queue::uncapped(),
                secrets: Queue::uncapped(),
                transcripts: Queue::uncapped(),
            },
            records: Records::new(limits.accounts),
            moves: moves::Moves::new(bytes::copy_of(fixtures::MOVE_APPLICATION), limits.accounts),
            finishes: finishes::Finishes::new(limits.accounts),
            paste: [0; 1024],
            paste_len: 0,
            paste_wrong: None,
            sign_in_again_requested: None,
            finish_carried: None,
            move_out: Queue::with_capacity(ROOM),
            providers: Map::with_capacity(1),
            source_providers: Map::with_capacity(limits.accounts),
            inventory: restart::Inventory::new(limits.accounts, store_limits.secrets.page.rows),
            move_mode: None,
            move_events: Queue::with_capacity(ROOM),
            moved_file: None,
            move_input: bytes::zeroed(
                usize::try_from(jig_protocol_accounts::moved::bound(&limits.client.document)).expect("file bound"),
            ),
            move_input_len: None,
            carried_move: None,
            lent: Vec::with_capacity(8),
            last_kept: None,
            limits,
            address: None,
            heard: Vec::with_capacity(8),
            facts: Vec::with_capacity(32),
            trace: Vec::with_capacity(16),
            close_requested: None,
            reject_requested: None,
            closing: false,
            closed: false,
            usable: false,
            now: Time::ZERO,
            story,
            account,
            installation: [0; 16],
            visit: None,
            ended: Vec::new(),
            answers: Vec::new(),
            removed: Vec::new(),
            erased: None,
            erasing: Map::with_capacity(limits.accounts),
            account_sign_out_asked: None,
        };
        if !fixtures::account_sign_in(story) {
            let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: child_limits };
            for record in [
                child::Stored::Account {
                    number: account + 1_000,
                    provider: 1,
                    holder: child::Holder::Deployment,
                    place: 0,
                    uses: None,
                    subject: Box::new([]),
                    display: Box::new([]),
                },
                child::Stored::Source {
                    account: account + 1_000,
                    number: account,
                    place: 0,
                    kind: child::Kind::SignIn,
                    name: Box::new([]),
                    held: None,
                },
            ] {
                child::step(&mut owner.child, &env, child::Event::Restore { record }, &mut owner.child_out);
            }
        }
        if let Some(kept) = installation.kept {
            // An earlier start kept its draw with its first commit; this
            // start opens the same stores again.
            assert_eq!(owner.open(kept), kept, "an empty store answers this start's draw");
            let stores = std::mem::replace(&mut owner.store, memory(&owner.store_limits)).into_stores();
            owner.store = store::Store::over(&owner.store_limits, fixtures::APPLICATION, &[], stores);
        }
        owner.installation = owner.open(installation.drawn);
        if let Some(held) = held {
            let value = Secret::new(bytes::copy_of(held));
            owner.secrets(Time::ZERO, Wall::EPOCH, source_token(account), replace(account, value, None));
            let kept = owner.store_above.pop();
            assert!(matches!(kept, Some(store::Event::Kept { .. })), "the store keeps the record held before the run");
        }
        owner
    }
    pub(crate) fn new_move(held: bool, moving: bool) -> Self {
        let mut owner = Self::new(Story::SignIn, None, crate::world::FIRST_START);
        owner.move_mode = Some(());
        if held {
            for record in [
                child::Stored::Account {
                    number: 1000,
                    provider: 1,
                    holder: child::Holder::Party(7),
                    place: 0,
                    uses: None,
                    subject: bytes::copy_of(b"subject"),
                    display: bytes::copy_of(b"Moved"),
                },
                child::Stored::Source {
                    account: 1000,
                    number: 0,
                    place: 0,
                    kind: child::Kind::SignIn,
                    name: Box::new([]),
                    held: moving.then_some(child::Held::Moving),
                },
            ] {
                owner.child_event(Time::ZERO, Wall::EPOCH, child::Event::Restore { record });
            }
            let mut record = fixtures::kept();
            record.subject = Some(bytes::copy_of(b"subject"));
            record.issued_client_id = Some(bytes::copy_of(b"client"));
            record.granted_scopes = Some(bytes::copy_of(b"openid read"));
            record.earliest_refresh_at = Some(Wall::from_nanos(skein_lib::Duration::from_secs(200).as_nanos()));
            let value =
                Secret::new(skein_oauth::encode_record(&record, &owner.limits.client.document).expect("move fixture"));
            owner.secrets(Time::ZERO, Wall::EPOCH, source_token(0), replace(0, value, None));
            assert!(matches!(owner.store_above.pop(), Some(store::Event::Kept { .. })));
        }
        owner
    }
    pub(crate) fn change(&mut self, request: u64, change: child::Change) {
        self.move_events.push(child::Event::Change {
            request: Token::new(request),
            holder: child::Holder::Party(7),
            change,
        });
    }
    pub(crate) fn moved_file(&self, source: u32) -> Option<&Secret> {
        self.moved_file.as_ref().filter(|(number, _)| *number == source).map(|(_, value)| value)
    }
    pub(crate) fn carry_move(&mut self, value: &Secret) {
        self.move_input[..value.len()].copy_from_slice(value.expose());
        self.move_input_len = Some(value.len());
    }
    pub(crate) fn carried_move(&self) -> Option<u64> {
        self.carried_move
    }
    pub(crate) fn source_usable(&self) -> bool {
        self.child.source_grant(self.account, self.now).is_some()
    }
    pub(crate) fn carried_value(&mut self, carried: u64) -> Option<Secret> {
        let env = Env { now: self.now, wall: Wall::EPOCH, limits: self.limits };
        self.secret_value(&env, GET, store::SecretOwner::Application, carried)
    }
    /// Opens the store, as this start drew `drawn`, and answers the
    /// installation's identifier it keeps. At an empty store the owner, as
    /// the core's stand-in, commits the deployment's header first, which
    /// keeps the identifier.
    fn open(&mut self, drawn: [u8; 16]) -> [u8; 16] {
        // Memory stores open no file: the root is never read.
        let root = store::Root { token: OPEN, fd: kernel::Fd::new(3) };
        self.ask(Time::ZERO, Wall::EPOCH, store::Request::Open { owner: OPEN, root, installation: drawn });
        let Some(store::Event::Opened { owner: OPEN, last, installation, pending }) = self.store_above.pop() else {
            panic!("the memory store opens at once");
        };
        if pending {
            // This store-only driver has no engine root to ask for the switch.
            self.ask(Time::ZERO, Wall::EPOCH, store::Request::Switch { owner: OPEN });
            assert!(matches!(self.store_above.pop(), Some(store::Event::Switched { owner: OPEN })));
        }
        {
            let number = last + 1;
            let header = core::Record::Core(core::CoreRecord::Deployment(core::Deployment {
                id: fixtures::DEPLOYMENT,
                tasks: 0,
                people: 0,
                sign_ins: 0,
                messages: 0,
                runs: 0,
                calls: 0,
                connector_rows: 0,
                projects: 0,
                applications: 0,
                commits: number,
            }));
            self.ask(
                Time::ZERO,
                Wall::EPOCH,
                store::Request::Commit { number, writes: Box::new([store::Write::Save(header), store::Write::Wires]) },
            );
            assert!(
                matches!(self.store_above.pop(), Some(store::Event::Committed { .. })),
                "the memory store commits at once"
            );
        }
        installation
    }
    /// The installation's identifier the store answered at this start.
    #[must_use]
    pub fn installation(&self) -> [u8; 16] {
        self.installation
    }
    /// The URL the driver asked to be visited for a sign-in.
    #[must_use]
    pub fn visit(&self) -> Option<&[u8]> {
        self.visit.as_deref()
    }
    #[must_use]
    pub fn child_heard(&self) -> &[Heard] {
        &self.heard
    }
    #[must_use]
    pub fn facts(&self) -> &[child::Fact] {
        &self.facts
    }
    #[must_use]
    pub fn trace(&self) -> &[Trace] {
        &self.trace
    }
    #[must_use]
    pub fn table_value(&self, account: u32, generation: u64) -> Option<&[u8]> {
        Some(self.table.value(account, generation, self.now)?.token.expose())
    }
    #[must_use]
    pub fn became_usable(&self) -> bool {
        self.usable
    }
    #[must_use]
    pub fn closed(&self) -> bool {
        self.closed
    }
    pub(crate) fn close(&mut self) {
        self.close_requested = Some(());
    }
    pub(crate) fn reject(&mut self) {
        self.reject_requested = Some(());
    }
    /// Each value the table lent, in order.
    #[must_use]
    pub fn lent(&self) -> &[Lent] {
        &self.lent
    }
    /// The record the store last answered as kept, read back from it.
    #[must_use]
    pub fn last_kept(&self) -> Option<&[u8]> {
        self.last_kept.as_ref().map(Secret::expose)
    }
    #[must_use]
    pub fn account_sign_in_ended(&self) -> &[child::Ended] {
        &self.ended
    }
    #[must_use]
    pub fn account_answers(&self) -> &[child::Answer] {
        &self.answers
    }
    #[must_use]
    pub fn account_record_erased(&self) -> bool {
        self.erased.is_some()
    }
    #[must_use]
    pub fn account_removed(&self) -> bool {
        !self.removed.is_empty()
    }
    /// The actual durable source record, read after the story has settled.
    pub fn read_record(&mut self) -> Option<Box<[u8]>> {
        // This inspection runs outside the world's measured calls. Keep its
        // answer allocation local so it cannot change the owner's retained heap.
        let mut above = Queue::uncapped();
        store::down(
            &mut self.store,
            &Env { now: self.now, wall: Wall::EPOCH, limits: self.store_limits },
            store::Request::Secrets {
                owner: GET,
                request: store::Secrets::Get { owner: store::SecretOwner::Source, number: u64::from(self.account) },
            },
            &mut above,
            &mut self.below,
        );
        let Some(store::Event::Secret { owner: GET, value }) = above.pop() else {
            panic!("the memory store answers Get immediately");
        };
        value.map(|value| {
            let store::SecretValue::Record(value) = value else { panic!("source record kind") };
            bytes::copy_of(value.expose())
        })
    }
    pub(crate) fn unstarted(&self) -> bool {
        self.component.is_none()
    }
    // The story signs out after the successful account sign-in's HTTP exchanges close.
    // Its grant alone does not mean the driver has released that exchange's place.
    fn account_sign_out_ready(&self) -> bool {
        matches!(self.story, Story::AccountSignOut { .. })
            && self.usable
            && self.account_sign_out_asked.is_none()
            && self.io.is_empty()
            && self.requests.is_empty()
            && self.events.is_empty()
            && self.above.is_empty()
    }
    pub(crate) fn child_event(&mut self, now: Time, wall: Wall, event: child::Event) {
        child::step(&mut self.child, &Env { now, wall, limits: self.child_limits }, event, &mut self.child_out);
        self.usable |= self.child.source_grant(self.account, now).is_some();
        while let Some(ended) = self.child.pop_ended() {
            self.ended.push(ended);
        }
        while let Some(fact) = self.child.pop_fact() {
            self.facts.push(fact);
        }
    }
    /// The driver starts from the store's records (llm.md 5.1).
    fn start(&mut self, env: &Env<driver::Limits>) {
        let mut configured = List::with_capacity(1);
        if !fixtures::account_sign_in(self.story) && self.move_mode.is_none() {
            assert!(
                configured
                    .push(driver::Account::SignIn {
                        shared: false,
                        origins: List::with_capacity(0),
                        registration: fixtures::registration(installation::terms(&self.installation)),
                        endpoint: driver::Endpoint {
                            address: self.address.expect("issuer address"),
                            transport: driver::Transport::Plaintext,
                        },
                        keeper: driver::Keeper::Owner { kept: None },
                    })
                    .is_ok()
            );
        }
        self.load(env, &mut configured);
        let crypto = skein_lib::CryptoRng::new(&skein_world::crypto_seed(17), 0).expect("world crypto seed");
        self.component = Some(driver::Component::new(configured, &self.limits, crypto).expect("configured driver"));
        if self.story == Story::AccountSignInFull {
            let mut saved = fixtures::kept();
            saved.key = 999;
            saved.issued_client_id = Some(bytes::copy_of(b"issued-client"));
            saved.granted_scopes = Some(bytes::copy_of(b"openid read"));
            saved.subject = Some(bytes::copy_of(b"subject"));
            let value = skein_oauth::encode_record(&saved, &self.limits.client.document).expect("saved source");
            let template = party::party_account(
                &fixtures::party_registration(self.story),
                driver::Endpoint {
                    address: self.address.expect("issuer address"),
                    transport: driver::Transport::Plaintext,
                },
                List::with_capacity(0),
                &self.installation,
            );
            let restored = party::restore(
                template,
                999,
                Some(Secret::new(value)),
                &mut self.records,
                &self.limits.client.document,
            );
            self.component.as_mut().expect("started account driver").add(env, 999, restored, &mut self.above);
            assert!(self.above.is_empty());
        }
        let template = self.party_account();
        assert!(self.providers.insert(1, template).is_ok());
        if self.move_mode.is_some() {
            self.child_event(env.now, env.wall, child::Event::Want);
        } else if fixtures::account_sign_in(self.story) {
            self.child_event(env.now, env.wall, child::Event::Want);
            self.child_event(
                env.now,
                env.wall,
                child::Event::SignIn { request: Token::new(80), holder: child::Holder::Party(7), provider: 1, by: 7 },
            );
        } else {
            self.child_event(env.now, env.wall, child::Event::Want);
        }
    }
    /// Reads the store's records, page by page, into the configured sources.
    fn load(&mut self, env: &Env<driver::Limits>, configured: &mut List<driver::Account>) {
        let mut after = None;
        loop {
            let load = store::Secrets::Load { owner: store::SecretOwner::Source, after };
            self.secrets(env.now, env.wall, LOAD, load);
            let Some(store::Event::SecretsLoaded { owner: LOAD, rows, next }) = self.store_above.pop() else {
                panic!("a memory store answers a load at once");
            };
            for row in rows {
                let account = u32::try_from(row.number).expect("a source's number");
                let store::SecretValue::Record(value) = row.value else { panic!("restored source record kind") };
                let Some(record) = self.records.load(account, value.expose(), &self.limits.client.document) else {
                    continue;
                };
                self.trace.push(Trace::Loaded { account, generation: record.generation });
                if let Some(driver::Account::SignIn { keeper, .. }) = configured.get_mut(account) {
                    *keeper = driver::Keeper::Owner { kept: Some(record) };
                }
            }
            match next {
                Some(next) => after = Some(next),
                None => return,
            }
        }
    }
    fn driver(&mut self, env: &Env<driver::Limits>, request: driver::Request) {
        self.component.as_mut().expect("started").down(
            env,
            request,
            &mut self.above,
            &mut self.requests,
            &mut self.files,
        );
    }
    fn ask(&mut self, now: Time, wall: Wall, request: store::Request) {
        let env = Env { now, wall, limits: self.store_limits };
        store::down(&mut self.store, &env, request, &mut self.store_above, &mut self.below);
        assert!(
            self.below.store.is_empty() && self.below.secrets.is_empty() && self.below.transcripts.is_empty(),
            "a memory store files nothing"
        );
    }
    fn secrets(&mut self, now: Time, wall: Wall, owner: Token, request: store::Secrets) {
        self.ask(now, wall, store::Request::Secrets { owner, request });
    }
    /// Asks the store to keep a record in place of the one it holds, and
    /// passes its answer on.
    fn party_account(&self) -> driver::Account {
        let registration = fixtures::party_registration(self.story);
        let endpoint =
            driver::Endpoint { address: self.address.expect("issuer"), transport: driver::Transport::Plaintext };
        let mut origins = List::with_capacity(1);
        origins
            .push(driver::Origin {
                uri: registration.issuer.clone(),
                endpoint: driver::Endpoint {
                    address: self.address.expect("issuer"),
                    transport: driver::Transport::Plaintext,
                },
            })
            .expect("origin fits");
        party::party_account(&registration, endpoint, origins, &self.installation)
    }
    fn ready(&mut self, env: &Env<driver::Limits>, source: u32, refresh: bool) {
        if let Some(account) = self.moves.ready(source, refresh) {
            self.component.as_mut().expect("started").add(env, source, account, &mut self.above);
        }
    }
    fn restore_signed_in(&mut self, source: u32, provider: u32, value: Option<Secret>) {
        assert_eq!(provider, 1);
        let template = self.party_account();
        let account = party::restore(template, source, value, &mut self.records, &self.limits.client.document);
        if let Some(generation) = self.records.generation(source) {
            self.trace.push(Trace::Loaded { account: source, generation });
        }
        self.source_providers.insert(source, provider).expect("source fits driver");
        self.moves.stage(source, account);
    }
    fn secret_value(
        &mut self,
        env: &Env<driver::Limits>,
        exchange: Token,
        owner: store::SecretOwner,
        number: u64,
    ) -> Option<Secret> {
        // The move referee also reads after settlement, outside measured calls.
        let mut above = Queue::uncapped();
        store::down(
            &mut self.store,
            &Env { now: env.now, wall: env.wall, limits: self.store_limits },
            store::Request::Secrets { owner: exchange, request: store::Secrets::Get { owner, number } },
            &mut above,
            &mut self.below,
        );
        let Some(store::Event::Secret { owner, value }) = above.pop() else { panic!("immediate secret read") };
        assert_eq!(owner, exchange);
        value.map(|value| match value {
            store::SecretValue::Record(value)
            | store::SecretValue::Held(value)
            | store::SecretValue::Carried { kind: store::CarriedKind::Moved, value } => value,
            other @ (store::SecretValue::Digest { .. }
            | store::SecretValue::Key(_)
            | store::SecretValue::Carried {
                kind: store::CarriedKind::Key | store::CarriedKind::Value, ..
            }
            | store::SecretValue::Application(_)) => panic!("accounts payload kind: {other:?}"),
        })
    }
    fn move_steps(&mut self, env: &Env<driver::Limits>) {
        while let Some(next) = self.move_out.pop() {
            match next {
                moves::Next::Driver(request) => self.driver(env, request),
                moves::Next::Child(event) => self.child_event(env.now, env.wall, event),
                moves::Next::Store { source, request } => match request {
                    moves::Store::ReadOut => {
                        let value = self.secret_value(
                            env,
                            Token::new(MOVE_OUT | u64::from(source)),
                            store::SecretOwner::Source,
                            u64::from(source),
                        );
                        let provider = self.source_providers.get(&source).expect("withdrawn provider");
                        let next = self.moves.read(
                            source,
                            value,
                            self.providers.get(provider).expect("provider"),
                            &self.limits.client.document,
                        );
                        self.move_out.push(next);
                    }
                    moves::Store::ReadBack => {
                        let value = self.secret_value(
                            env,
                            Token::new(MOVE_BACK | u64::from(source)),
                            store::SecretOwner::Source,
                            u64::from(source),
                        );
                        self.restore_signed_in(
                            source,
                            *self.source_providers.get(&source).expect("return provider"),
                            value,
                        );
                    }
                    moves::Store::ReadIn { carried } => {
                        let value = self.secret_value(
                            env,
                            Token::new(MOVE_IN | u64::from(source)),
                            store::SecretOwner::Application,
                            carried,
                        );
                        let next = self.moves.opened(source, value, &self.providers, &self.limits.client.document);
                        self.move_out.push(next);
                    }
                    moves::Store::AdoptValue { carried, holds, value } => {
                        let owner = Token::new(MOVE_ADOPT | u64::from(source));
                        self.secrets(
                            env.now,
                            env.wall,
                            owner,
                            store::Secrets::AdoptValue {
                                carried,
                                holds,
                                owner: store::SecretOwner::Source,
                                number: u64::from(source),
                                value: jig_protocol_store::SecretValue::Record(value),
                                replaces: None,
                            },
                        );
                        let kept = match self.store_above.pop() {
                            Some(store::Event::Kept { .. }) => true,
                            Some(store::Event::Stale { .. }) => false,
                            other => panic!("adoption terminal: {other:?}"),
                        };
                        if let Some(adoption) = self.moves.adopted(source, kept) {
                            self.account = source;
                            self.inventory.name(source);
                            self.restore_signed_in(source, adoption.provider, Some(adoption.value));
                        }
                        self.child_event(env.now, env.wall, child::Event::Adopted { source, kept });
                    }
                },
            }
        }
    }
    fn sweep(&mut self, env: &Env<driver::Limits>) {
        if self.move_mode.is_none() {
            return;
        }
        if let Some(step) = self.inventory.step() {
            match step {
                restart::Sweep::Load { after } => {
                    self.secrets(
                        env.now,
                        env.wall,
                        SWEEP,
                        store::Secrets::Load { owner: store::SecretOwner::Source, after },
                    );
                    let Some(store::Event::SecretsLoaded { rows, next, .. }) = self.store_above.pop() else {
                        panic!("inventory page")
                    };
                    for row in rows {
                        self.inventory.row(row.number);
                    }
                    self.inventory.loaded(next);
                }
                restart::Sweep::Erase { number } => self.secrets(
                    env.now,
                    env.wall,
                    SWEEP,
                    store::Secrets::Erase { owner: store::SecretOwner::Source, number },
                ),
            }
        }
    }
    fn replace(&mut self, env: &Env<driver::Limits>, keep: records::Keep) {
        let records::Keep { account, value, replaces } = keep;
        self.secrets(env.now, env.wall, source_token(account), replace(account, value, replaces));
        self.answers(env);
    }
    /// The store's answers to replaces (store.md 9): the driver hears `Kept`,
    /// or a handed-in record is handed in, only once the record is durable.
    fn answers(&mut self, env: &Env<driver::Limits>) {
        // A batch may contain several erasures; their readbacks get their own answer queue.
        let mut answers = std::mem::replace(&mut self.store_above, Queue::uncapped());
        while let Some(event) = answers.pop() {
            let request = match event {
                store::Event::Kept { owner } | store::Event::Stale { owner }
                    if owner.raw() & MOVE_ADOPT == MOVE_ADOPT =>
                {
                    unreachable!("move adoption is answered in move_steps: {owner:?}");
                }
                store::Event::Erased { owner: SWEEP } => {
                    self.inventory.erased();
                    continue;
                }
                store::Event::Kept { owner } => {
                    let account = source(owner);
                    let Some(request) = self.records.kept(account) else { continue };
                    self.trace.push(Trace::Kept { account, generation: generation(&request) });
                    let get = store::Secrets::Get { owner: store::SecretOwner::Source, number: u64::from(account) };
                    self.secrets(env.now, env.wall, GET, get);
                    let Some(store::Event::Secret { owner: GET, value: Some(value) }) = self.store_above.pop() else {
                        panic!("a kept record reads back");
                    };
                    let store::SecretValue::Record(value) = value else { panic!("kept source record kind") };
                    self.last_kept = Some(value);
                    request
                }
                store::Event::Stale { owner } => {
                    let Some(request) = self.records.stale(source(owner)) else { continue };
                    request
                }
                store::Event::Erased { owner } => {
                    let source = source(owner);
                    let Some(pending) = self.erasing.remove(&source) else { continue };
                    if matches!(pending, Erasing::Removing) {
                        self.erasing.insert(source, pending).expect("pending removal's place");
                        continue;
                    }
                    self.records.erased(source);
                    self.secrets(
                        env.now,
                        env.wall,
                        GET,
                        store::Secrets::Get { owner: store::SecretOwner::Source, number: u64::from(source) },
                    );
                    let Some(store::Event::Secret { owner: GET, value }) = self.store_above.pop() else {
                        panic!("an erased record reads back");
                    };
                    assert!(value.is_none(), "the durable store no longer holds the record");
                    self.erased = Some(());
                    if let Erasing::SignOut(confirmed) = pending {
                        self.erasing.insert(source, Erasing::Removing).expect("pending erase's place");
                        self.child_event(env.now, env.wall, child::Event::SignedOut { source, confirmed });
                        self.driver(env, driver::Request::Remove { account: source });
                    }
                    continue;
                }
                store::Event::Switched { .. }
                | store::Event::Opened { .. }
                | store::Event::OpenRefused { .. }
                | store::Event::Site { .. }
                | store::Event::Patched { .. }
                | store::Event::Committed { .. }
                | store::Event::Failed { .. }
                | store::Event::FrameTooLarge { .. }
                | store::Event::Restored { .. }
                | store::Event::Loaded { .. }
                | store::Event::Read { .. }
                | store::Event::Got { .. }
                | store::Event::Turns { .. }
                | store::Event::Unread { .. }
                | store::Event::Carried { .. }
                | store::Event::Secret { .. }
                | store::Event::SecretsLoaded { .. }
                | store::Event::SecretsNotReady { .. }
                | store::Event::Corrupt { .. } => unreachable!("a replace is answered kept or stale"),
            };
            self.driver(env, request);
        }
    }
    fn erase(&mut self, env: &Env<driver::Limits>, source: u32, confirmed: Option<bool>) {
        if self.removed.contains(&source) {
            return;
        }
        if let Some(pending) = self.erasing.get_mut(&source) {
            if matches!(pending, Erasing::Revoked)
                && let Some(confirmed) = confirmed
            {
                *pending = Erasing::SignOut(confirmed);
            }
            return;
        }
        self.erasing
            .insert(source, confirmed.map_or(Erasing::Revoked, Erasing::SignOut))
            .expect("one erase per source");
        self.secrets(
            env.now,
            env.wall,
            source_token(source),
            store::Secrets::Erase { owner: store::SecretOwner::Source, number: u64::from(source) },
        );
    }
    pub(crate) fn hand_paste(&mut self, uri: &[u8], wrong: bool) {
        self.paste[..uri.len()].copy_from_slice(uri);
        self.paste_len = uri.len();
        self.paste_wrong = wrong.then_some(());
    }
    pub(crate) fn sign_in_again(&mut self) {
        self.sign_in_again_requested = Some(());
    }
    fn restart_sign_in(&mut self, now: Time, wall: Wall) {
        self.visit = None;
        self.child_event(
            now,
            wall,
            child::Event::SignIn { request: Token::new(83), holder: child::Holder::Party(7), by: 7, provider: 1 },
        );
    }
    fn paste(&mut self, env: &Env<driver::Limits>) {
        if self.paste_len == 0 {
            return;
        }
        let mut uri = bytes::copy_of(&self.paste[..self.paste_len]);
        self.paste_len = 0;
        if self.paste_wrong.take().is_some() {
            let at = uri.windows(6).position(|part| part == b"state=").expect("fixture state") + 6;
            uri[at] = if uri[at] == b'a' { b'b' } else { b'a' };
        }
        self.secrets(env.now, env.wall, GET, store::Secrets::Hold { value: Secret::new(uri) });
        let Some(store::Event::Carried { number, .. }) = self.store_above.pop() else { panic!("held address") };
        self.finish_carried = Some(number);
        self.child_event(
            env.now,
            env.wall,
            child::Event::Change {
                request: Token::new(81),
                holder: child::Holder::Party(7),
                change: child::Change::FinishSignIn { source: self.account, carried: number, by: 7 },
            },
        );
    }
    fn finish_steps(&mut self, env: &Env<driver::Limits>, mut next: finishes::Next) {
        loop {
            match next {
                finishes::Next::Get { exchange, carried } => {
                    let value = self.secret_value(env, GET, store::SecretOwner::Application, carried);
                    next = self.finishes.read(exchange, value).expect("pending finish read");
                }
                finishes::Next::Driver(request) => {
                    self.driver(env, request);
                    break;
                }
                finishes::Next::Child(event) => {
                    self.child_event(env.now, env.wall, event);
                    break;
                }
            }
        }
    }
    fn signing(&mut self, env: &Env<driver::Limits>, signing: translate::Down) {
        match signing {
            translate::Down::Restore { source, provider, kind: child::Kind::SignIn } if self.move_mode.is_some() => {
                let value = self.secret_value(env, GET, store::SecretOwner::Source, u64::from(source));
                self.restore_signed_in(source, provider, value);
                self.inventory.name(source);
            }
            translate::Down::Keep { .. } | translate::Down::Restore { .. } => (),
            translate::Down::Restored => self.inventory.restored(),
            translate::Down::Forget { source } => {
                match self.moves.forget(source) {
                    moves::Forget::Pending => return,
                    moves::Forget::Withdrawn => (),
                    moves::Forget::Source => {
                        self.ready(env, source, false);
                        self.driver(env, driver::Request::Remove { account: source });
                    }
                }
                self.inventory.forget(source);
                self.records.remove(source);
                self.secrets(
                    env.now,
                    env.wall,
                    source_token(source),
                    store::Secrets::Erase { owner: store::SecretOwner::Source, number: u64::from(source) },
                );
            }
            translate::Down::Withdraw { source, display } => {
                self.ready(env, source, false);
                let next = self.moves.withdraw(source, display);
                self.move_out.push(next);
            }
            translate::Down::Return { source } => {
                let next = self.moves.returning(source);
                self.move_out.push(next);
            }
            translate::Down::Refresh { source } => {
                self.ready(env, source, true);
            }
            translate::Down::Open { source, carried } => {
                let next = self.moves.open(source, carried);
                self.move_out.push(next);
            }
            translate::Down::Adopt { source, carried } => {
                let next = self.moves.adopt(source, carried, &self.limits.client.document);
                self.move_out.push(next);
            }
            translate::Down::SignIn { source, provider: 1 } => {
                self.account = source;
                let account = self.party_account();
                let before = self.above.len();
                self.component.as_mut().expect("started account driver").add(env, source, account, &mut self.above);
                if self.above.len() == before {
                    self.inventory.name(source);
                    self.source_providers.insert(source, 1).expect("source provider");
                    self.records.admit(source).expect("driver admitted account");
                    self.driver(env, driver::Request::SignIn { account: source, subject: None });
                }
            }
            translate::Down::SignIn { .. } => panic!("known account sign-in provider"),
            translate::Down::Redirected { source, carried } => {
                let next = self.finishes.redirected(source, carried);
                self.finish_steps(env, next);
            }
            translate::Down::CancelSignIn { source } => {
                self.driver(env, driver::Request::Cancel { account: source });
            }
            translate::Down::SignOut { source } => {
                self.ready(env, source, false);
                self.driver(env, driver::Request::SignOut { account: source });
            }
        }
    }
    fn child_requests(&mut self, env: &Env<driver::Limits>) {
        while let Some(request) = self.child_out.pop() {
            match request {
                child::Request::Component(request) => {
                    if let child::ToComponent::Want { account } = request {
                        self.ready(env, account, false);
                    }
                    if request == child::ToComponent::Close {
                        self.moves.close();
                    }
                    self.trace.push(Trace::Requested(request));
                    self.component.as_mut().expect("started").down(
                        env,
                        translate::down(request),
                        &mut self.above,
                        &mut self.requests,
                        &mut self.files,
                    );
                }
                child::Request::Signing(signing) => {
                    self.signing(env, translate::down_signing(signing));
                    self.move_steps(env);
                }
                child::Request::Answer { request, answer } => {
                    if request == Token::new(81)
                        && let Some(number) = self.finish_carried.take()
                    {
                        self.secrets(
                            env.now,
                            env.wall,
                            GET,
                            store::Secrets::Erase { owner: store::SecretOwner::Application, number },
                        );
                        assert!(matches!(self.store_above.pop(), Some(store::Event::Erased { .. })));
                    }
                    if let child::Answer::MovingOut { source, .. } = &answer {
                        let file = self.moves.take(*source).expect("moving out carries the file");
                        assert!(self.moves.take(*source).is_none(), "file goes out once");
                        self.moved_file = Some((*source, file));
                    }
                    self.answers.push(answer);
                }
                child::Request::Programs { .. }
                | child::Request::CheckProgram { .. }
                | child::Request::SignOutProgram { .. }
                | child::Request::Granted { grant: _ }
                | child::Request::Save { .. }
                | child::Request::Erase { .. } => (),
                child::Request::Closed => {
                    self.closed = true;
                    self.trace.push(Trace::Closed);
                }
            }
        }
    }
    fn observe(&mut self, env: &Env<driver::Limits>) {
        while let Some(event) = self.above.pop() {
            let failed_add = matches!(event, driver::Event::Refused { asked: driver::Asked::Add, .. });
            let event = match self.records.up(event, env.now, b"openid read") {
                Up::Granted { event, account, generation, value, expires } => {
                    self.lent.push(Lent { account, generation, digest: sha256::digest(value.token.expose()) });
                    self.table.granted(account, generation, value, expires).expect("configured value fits");
                    event
                }
                Up::Child(event) => event,
                Up::Revoked { source } => {
                    self.table.remove(source);
                    self.erase(env, source, None);
                    child::FromComponent::Failed { account: source, failure: child::Failure::SignInAgain }
                }
                Up::Finishing { source } => {
                    self.child_event(env.now, env.wall, child::Event::Finishing { source });
                    continue;
                }
                Up::FinishRefused { source, why } => {
                    self.child_event(env.now, env.wall, child::Event::FinishRefused { source, why });
                    continue;
                }
                Up::Visited { source, address } => {
                    self.trace.push(Trace::Visit { account: source });
                    self.visit = Some(address.clone());
                    self.child_event(env.now, env.wall, child::Event::Visited { source, address });
                    continue;
                }
                Up::SignedIn { source, subject, display } => {
                    self.child_event(
                        env.now,
                        env.wall,
                        child::Event::SignedIn { source, subject, display, declined: false },
                    );
                    continue;
                }
                Up::SignInFailed { source, failure } => {
                    self.child_event(env.now, env.wall, child::Event::SignInFailed { source, failure });
                    if !failed_add {
                        self.driver(env, driver::Request::SignOut { account: source });
                    }
                    continue;
                }
                Up::SignedOut { source, confirmed } => {
                    self.erase(env, source, Some(confirmed));
                    continue;
                }
                Up::Removed { source } => {
                    if let Some(next) = self.moves.removed(source) {
                        self.move_out.push(next);
                        self.move_steps(env);
                        continue;
                    }
                    self.inventory.forget(source);
                    self.erasing.remove(&source);
                    self.records.remove(source);
                    self.table.remove(source);
                    self.removed.push(source);
                    continue;
                }

                Up::Keep { account, record } => {
                    match self.records.keep(account, record.0, &self.limits.client.document) {
                        records::Next::Driver(request) => self.driver(env, request),
                        records::Next::Store(keep) => self.replace(env, keep),
                    }
                    continue;
                }
            };
            let heard = match event {
                child::FromComponent::Granted { form: _, account, generation, valid: _ } => {
                    Some(Heard::Granted { account, generation })
                }
                child::FromComponent::Expiring { account, generation } => Some(Heard::Expiring { account, generation }),
                child::FromComponent::Failed { account, failure } => Some(Heard::Failed { account, failure }),
                child::FromComponent::KeepFailed { .. } | child::FromComponent::Closed => None,
            };
            if let Some(heard) = heard {
                self.heard.push(heard);
                self.trace.push(Trace::Heard(heard));
            }
            self.child_event(env.now, env.wall, child::Event::Component(event));
        }
    }
}
fn source_token(account: u32) -> Token {
    Token::new(SOURCE | u64::from(account))
}
fn source(owner: Token) -> u32 {
    u32::try_from(owner.raw() - SOURCE).expect("a source's number")
}
fn replace(account: u32, value: Secret, replaces: Option<[u8; 32]>) -> store::Secrets {
    store::Secrets::Replace {
        owner: store::SecretOwner::Source,
        number: u64::from(account),
        value: jig_protocol_store::SecretValue::Record(value),
        replaces,
    }
}
/// The generation the store's answer makes durable.
fn generation(request: &driver::Request) -> u64 {
    if let driver::Request::Kept { generation, .. } = request {
        *generation
    } else if let driver::Request::HandIn { record, .. } = request {
        record.generation
    } else {
        unreachable!("the store answers a candidate or a handed-in record")
    }
}
impl Host for Owner {
    fn iterate(&mut self, now: Time, wall: Wall) {
        self.now = now;
        let env = Env { now, wall, limits: self.limits };
        let io_env = Env { now, wall, limits: self.limits.io };
        self.paste(&env);
        if self.component.is_none() && self.address.is_some() {
            self.start(&env);
        }
        if self.component.is_none() {
            return;
        }
        if self.sign_in_again_requested.take().is_some() {
            self.restart_sign_in(now, wall);
        }
        if self.account_sign_out_ready() {
            self.account_sign_out_asked = Some(());
            self.child_event(
                now,
                wall,
                child::Event::SignOut {
                    request: Token::new(81),
                    holder: child::Holder::Party(7),
                    source: self.account,
                },
            );
        }
        if self.close_requested.take().is_some() && !self.closing {
            self.closing = true;
            self.child_event(now, wall, child::Event::Close);
        }
        if self.reject_requested.take().is_some() {
            self.child_event(now, wall, child::Event::Unauthorized { account: self.account, generation: 1 });
        }
        self.answers(&env);
        if let Some(len) = self.move_input_len.take() {
            let value = Secret::new(bytes::copy_of(&self.move_input[..len]));
            self.secrets(now, wall, GET, store::Secrets::Carry { kind: jig_protocol_store::CarriedKind::Moved, value });
            let Some(store::Event::Carried { number, .. }) = self.store_above.pop() else { panic!("carried move") };
            self.carried_move = Some(number);
            self.change(93, child::Change::MoveIn { carried: number });
        }
        while let Some(event) = self.move_events.pop() {
            self.child_event(now, wall, event);
        }
        self.child_requests(&env);
        self.sweep(&env);
        for _ in 0..ROOM {
            if !self.io.is_ready() || self.events.room() < 3 || self.submissions.room() < 2 {
                break;
            }
            io::resume(&mut self.io, &io_env, &mut self.events, &mut self.submissions);
        }
        while let Some(complete) = self.completions.pop() {
            io::up(&mut self.io, &io_env, complete, &mut self.events, &mut self.submissions);
        }
        for _ in 0..ROOM {
            if !self.io.is_due(now) || self.events.room() < 3 || self.submissions.room() < 2 {
                break;
            }
            io::fire(&mut self.io, &io_env, &mut self.events, &mut self.submissions);
        }
        while let Some(event) = self.events.pop() {
            self.component.as_mut().expect("started").up(
                &env,
                event,
                &mut self.above,
                &mut self.requests,
                &mut self.files,
            );
        }
        let component = self.component.as_mut().expect("started");
        if component.has_work() || component.next_deadline().is_some_and(|due| due <= now) {
            component.fire(&env, &mut self.above, &mut self.requests, &mut self.files);
        }
        if self.child.is_due(now) && !self.closing {
            child::fire(&mut self.child, &Env { now, wall, limits: self.child_limits }, &mut self.child_out);
        }
        self.table.expire(now);
        self.observe(&env);
        self.child_requests(&env);
        self.answers(&env);
        for _ in 0..ROOM {
            if !self.io.takes() || self.submissions.room() < 2 {
                break;
            }
            let Some(request) = self.requests.pop() else { break };
            io::down(&mut self.io, &io_env, request, &mut self.submissions);
        }
        assert!(self.files.is_empty(), "the owner keeper files nothing");
        self.io.reclaim();
        self.component.as_mut().expect("started").reclaim();
    }
    fn completions(&mut self) -> &mut Queue<kernel::Complete> {
        &mut self.completions
    }
    fn submissions(&mut self) -> &mut Queue<kernel::Submit> {
        &mut self.submissions
    }
    fn work_pending(&self, now: Time) -> bool {
        self.paste_len > 0
            || self.sign_in_again_requested.is_some()
            || self.io.is_ready()
            || self.account_sign_out_ready()
            || !self.events.is_empty()
            || !self.requests.is_empty()
            || !self.above.is_empty()
            || !self.store_above.is_empty()
            || !self.move_events.is_empty()
            || self.move_input_len.is_some()
            || !self.move_out.is_empty()
            || (self.move_mode.is_some() && self.inventory.work_pending())
            || !self.child_out.is_empty()
            || !self.completions.is_empty()
            || (self.component.is_none() && self.address.is_some())
            || (self.close_requested.is_some() && !self.closing)
            || self.reject_requested.is_some()
            || self.component.as_ref().is_some_and(|component| {
                component.has_work() || component.next_deadline().is_some_and(|due| due <= now)
            })
    }
    fn next_deadline(&self) -> Option<Time> {
        [self.io.next_deadline(), self.next_policy_deadline()].into_iter().flatten().min()
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        [
            self.component.as_ref().and_then(driver::Component::next_deadline),
            (!self.closing).then(|| self.child.next_deadline()).flatten(),
            (!self.closing).then(|| self.table.next_expiry()).flatten(),
        ]
        .into_iter()
        .flatten()
        .min()
    }
    fn is_empty(&self) -> bool {
        self.closed
            && self.io.is_empty()
            && !self.records.keeping()
            && self.store_above.is_empty()
            && self.events.is_empty()
            && self.requests.is_empty()
            && self.files.is_empty()
            && self.above.is_empty()
            && self.child_out.is_empty()
            && self.completions.is_empty()
            && self.submissions.is_empty()
    }
    fn worst_case(&self) -> u64 {
        driver::worst_case(&self.limits).expect("driver bound")
            + io::worst_case(&self.limits.io).expect("io bound")
            + store::worst_case(&self.store_limits).expect("store bound")
            + records::worst_case(self.limits.accounts, &self.limits.client.document).expect("records bound")
            + moves::worst_case(self.limits.accounts, &self.limits.client.document).expect("moves bound")
            + restart::worst_case(self.limits.accounts, self.store_limits.secrets.page.rows).expect("inventory bound")
            + Queue::<moves::Next>::worst_case(ROOM).expect("move steps bound")
            + child::worst_case(&self.child_limits).expect("child bound")
            + jig_protocol_accounts::table::worst_case(self.limits.accounts, self.limits.client.document.token_bytes)
                .expect("table bound")
            + 512_000
    }
    fn operations(&self) -> u32 {
        io::operations(&self.limits.io).expect("io operation bound")
    }
}
