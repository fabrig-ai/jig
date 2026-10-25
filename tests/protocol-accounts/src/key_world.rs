//! The LLM-key component over the real secrets store and grant table.
use crate::fixtures;
use jig_domain_accounts as accounts;
use jig_protocol_accounts::{
    keys::{Keys, Next},
    table::{Scheme, Table},
};
use jig_protocol_store as store;
use skein_lib::{Env, Queue, Secret, Time, Token, Wall};

pub struct World {
    pub keys: Keys,
    pub table: Table,
    pub heard: Vec<accounts::FromComponent>,
    store: store::Store,
    limits: store::Limits,
    above: Queue<store::Event>,
    below: store::Below,
}
impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}
impl World {
    #[must_use]
    pub fn new() -> Self {
        let limits = fixtures::store_limits();
        let mut world = Self {
            keys: Keys::new(2, 32),
            table: Table::new(2, 32),
            heard: Vec::new(),
            store: store::Store::over(
                &limits,
                fixtures::APPLICATION,
                &[],
                store::Stores {
                    store: skein_kv::Store::memory(&limits.store),
                    secrets: skein_kv::Store::memory(&limits.secrets),
                },
            ),
            limits,
            above: Queue::with_capacity(store::MAX_OUT_DOWN),
            below: store::Below {
                store: Queue::with_capacity(skein_kv::MAX_OUT.below),
                secrets: Queue::with_capacity(skein_kv::MAX_OUT.below),
                transcripts: Queue::with_capacity(skein_kv::MAX_OUT.below),
            },
        };
        let root = store::Root { token: Token::new(1), fd: skein_io::kernel::Fd::new(3) };
        world.down(store::Request::Open { owner: Token::new(1), root, installation: [5; 16] });
        assert!(matches!(world.above.pop(), Some(store::Event::Opened { .. })));
        world
    }
    fn down(&mut self, request: store::Request) {
        let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: self.limits };
        store::down(&mut self.store, &env, request, &mut self.above, &mut self.below);
        assert!(self.below.store.is_empty() && self.below.secrets.is_empty(), "memory store");
    }
    pub fn secret(&mut self, request: store::Secrets) -> store::Event {
        self.down(store::Request::Secrets { owner: Token::new(1), request });
        let event = self.above.pop().expect("one secrets answer");
        assert!(self.above.is_empty());
        event
    }
    pub fn carry(&mut self, value: &[u8]) -> u64 {
        let store::Event::Carried { number, .. } =
            self.secret(store::Secrets::Carry { value: Secret::new(value.into()) })
        else {
            panic!("carried LLM key")
        };
        number
    }
    pub fn get(&mut self, owner: store::SecretOwner, number: u64) -> Option<Secret> {
        let store::Event::Secret { value, .. } = self.secret(store::Secrets::Get { owner, number }) else {
            panic!("stored value")
        };
        value
    }
    pub fn keep(&mut self, source: u32, carried: u64, replaces: bool) {
        let mut next = Some(self.keys.keep(source, carried, replaces));
        for _ in 0..4 {
            next = match next {
                Some(Next::Store { exchange, request }) => {
                    let request = match request {
                        jig_protocol_accounts::keys::Store::Get { carried } => {
                            store::Secrets::Get { owner: store::SecretOwner::Application, number: carried }
                        }
                        jig_protocol_accounts::keys::Store::Adopt { carried, source, replaces } => {
                            store::Secrets::Adopt {
                                carried,
                                owner: store::SecretOwner::Source,
                                number: u64::from(source),
                                replaces,
                            }
                        }
                    };
                    self.down(store::Request::Secrets { owner: exchange, request });
                    match self.above.pop().expect("secrets answer") {
                        store::Event::Secret { value, .. } => self.keys.read(exchange, value),
                        store::Event::Kept { .. } => self.keys.kept(exchange, true, Time::ZERO),
                        store::Event::Stale { .. } => self.keys.kept(exchange, false, Time::ZERO),
                        other @ (store::Event::Opened { .. }
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
                        | store::Event::Erased { .. }
                        | store::Event::SecretsLoaded { .. }
                        | store::Event::Corrupt { .. }) => panic!("LLM key exchange: {other:?}"),
                    }
                }
                Some(Next::Granted { event, source, generation, value }) => {
                    assert_eq!(value.scheme, Scheme::Key);
                    self.table.granted(source, generation, value, Time::from_nanos(u64::MAX)).expect("LLM key fits");
                    self.heard.push(event);
                    None
                }
                Some(Next::Child(event)) => {
                    self.heard.push(event);
                    None
                }
                None => break,
            };
        }
        assert!(next.is_none() && !self.keys.pending(), "exchange settles");
    }
    pub fn forget(&mut self, source: u32) {
        self.keys.forget(source);
        self.table.remove(source);
        assert!(matches!(
            self.secret(store::Secrets::Erase { owner: store::SecretOwner::Source, number: u64::from(source) }),
            store::Event::Erased { .. }
        ));
    }
}
