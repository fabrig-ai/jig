//! Shell duties and the reusable application/front boundaries (testing.md, 4).
mod faults;
mod observation;
pub use faults::Fault;
pub mod referee;
pub use observation::*;
mod world;
use skein_lib::{Env, Queue, Time, Wall};
use skein_ui::Act;
use skein_ui::{NodeId, Patch, Tree};
use std::fmt::Debug;
pub use world::*;
/// An application root and view. The shell reads only requests and trees.
pub trait App: Sized {
    type Root;
    type View;
    type Limits: Clone;
    type Event;
    type Request;
    type Record: Clone + Debug;
    type Key: Clone + Eq + Debug;
    type Address: Clone + Eq + Debug;
    fn limits() -> Self::Limits;
    fn new(seed: u64) -> (Self::Root, Self::View);
    fn start(address: Self::Address, kept: Vec<Self::Record>, offset: i32) -> Self::Event;
    fn went(address: Self::Address) -> Self::Event;
    fn step(root: &mut Self::Root, env: &Env<Self::Limits>, event: Self::Event, out: &mut Queue<Self::Request>);
    fn fire(root: &mut Self::Root, env: &Env<Self::Limits>, out: &mut Queue<Self::Request>);
    fn take(root: &mut Self::Root, env: &Env<Self::Limits>, out: &mut Queue<Self::Request>);
    fn next_deadline(root: &Self::Root) -> Option<Time>;
    fn build(view: &mut Self::View, root: &Self::Root, patches: &mut Queue<Patch>);
    fn tree(view: &Self::View) -> &Tree;
    fn act(view: &mut Self::View, node: NodeId, act: Act) -> Option<Self::Event>;
    fn duty(request: Self::Request) -> Duty<Self>;
    fn record_key(record: &Self::Record) -> Self::Key;
    fn unsent(request: Self::Request) -> Option<Self::Event>;
    fn view_limits(view: &Self::View) -> skein_ui::Limits;
    fn patch_limit(view: &Self::View) -> u32;
    /// No bound of the application's own: the world's
    /// `heap_bytes_per_client` binds (programming-model.md, 6.3). An
    /// application that still states a bound overrides it.
    fn worst_case(_limits: &Self::Limits) -> u64 {
        u64::MAX
    }
    fn seen(tree: &Tree) -> Vec<Seen>;
    /// Translate a keyed wire request without reading root state.
    fn sent(request: &Self::Request) -> Option<Sent>;
    /// The jig request crossing the public wire boundary, when this application reports one.
    fn decision(_request: &Self::Request) -> Option<&jig_client::contract::Ask> {
        None
    }
    /// Translate delivered documents and answers before the callback.
    fn received(event: &Self::Event) -> Vec<Receipt>;
    /// The enclosing card a person acted on, from the public tree alone.
    fn card(tree: &Tree, node: NodeId) -> Option<CardKey>;
    /// Decode person-chosen display changes, such as filtering, folding or reading more.
    fn window_changed(_event: &Self::Event) -> bool {
        false
    }
}
/// One persistent shell record change.
#[derive(Debug)]
pub enum StoreWrite<R, K> {
    Save(R),
    Erase(K),
}
/// The shell obligation represented by a root request.
pub enum Duty<A: App> {
    Store(StoreWrite<A::Record, A::Key>),
    Wire(A::Request),
    Show { address: A::Address, replace: bool },
    SignIn { provider: u16 },
    SignOut,
    Fact,
}
/// Engine boundary; token routing and transport epochs belong to the front.
pub trait Front<A: App> {
    fn constructor(&self, constructor: ClientConstructor<A>) -> ClientConstructor<A> {
        constructor
    }

    fn bind(&mut self, client: ClientId, party: u64);
    fn starting(&mut self, _client: ClientId) -> Vec<A::Event> {
        Vec::new()
    }
    fn wire(&mut self, client: ClientId, now: Time, wall: Wall, request: A::Request);
    fn poll(&mut self, now: Time, wall: Wall) -> Vec<(ClientId, A::Event)>;
    fn next_due(&self) -> Option<Time>;
    fn observed(&mut self) -> Vec<Observed>;
    fn sent(&self, _request: &A::Request) -> Option<Sent> {
        None
    }
    fn receipts(&mut self, _client: ClientId, _event: &A::Event) -> Vec<Receipt> {
        Vec::new()
    }
    fn fault(&mut self, now: Time, fault: FrontFault);
    fn reload(&mut self, client: ClientId, now: Time);
    fn received(&mut self, client: ClientId, event: &A::Event);
    fn delivers(&mut self, _client: ClientId, _event: &A::Event) -> bool {
        true
    }
    fn stored(&mut self, _client: ClientId, _records: &[A::Record]) {}
    fn restored(&mut self, _client: ClientId, records: Vec<A::Record>) -> Vec<A::Record> {
        records
    }
    fn sign_in(&mut self, client: ClientId, now: Time, wall: Wall);
    fn sign_out(&mut self, client: ClientId, now: Time, wall: Wall);
}
/// A fault the engine boundary can inject.
#[derive(Clone, Debug)]
pub enum FrontFault {
    Busy,
    NotReady,
    Restart { offline: skein_lib::Duration },
    Slow { span: skein_lib::Duration },
    DropWatch,
    NoAnswer,
    SignedOut,
    Versions,
    ObjectGone,
}
