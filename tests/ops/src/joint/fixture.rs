//! Finite receiving bounds and genuine non-person alert sources.
use super::front::Joint;
use crate::{
    client::Ops,
    conformance::{Config, Story},
};
use jig_client_world::{Settings, World};
use jig_domain_core as core;
use jig_domain_tasks as tasks;
use jig_ops_client_domain as client;
use jig_ops_client_view as view;
use jig_ops_domain as root;
use jig_ops_domain_infrastructure as infra;
use jig_ops_domain_observability as obs;
#[must_use]
pub fn limits() -> client::Limits {
    let mut limits = crate::client::limits();
    // Navigation overlaps a page's closing watches with its successor's reads.
    limits.client.exchanges = 16;
    limits.client.domain.handles = 128;
    limits.client.domain.watches = 64;
    limits.client.domain.reads = 64;
    limits.client.domain.text = 4096;
    limits.client.domain.turn_text = 4096;
    limits.client.domain.turn_bytes = 4096;
    limits.client.domain.document_bytes = 262_144;
    limits.client.domain.rows = 32;
    limits.client.domain.projects = 7;
    limits.client.domain.delegates = 8;
    limits.jig.text = 4096;
    limits.jig.document_bytes = 262_144;
    limits.jig.rows = 32;
    limits.jig.roles = 8;
    limits.jig.projects = 32;
    limits.jig.delegates = 8;
    limits.jig.objects = 16;
    limits.jig.sources = 24;
    limits.jig.parts = 24;
    limits.jig.watches = 32;
    limits.jig.reads = 32;
    limits.jig.conversation_words = 16;
    limits.observability.rows = 32;
    limits.observability.text = 4096;
    limits.infrastructure.rows = 32;
    limits.infrastructure.text = 4096;
    limits
}
struct Profile {
    bounds: view::Limits,
    jig: jig_client_view::words::Words,
    obs: view::words::Words,
    infra: view::words::Words,
    heap: u64,
}
fn profile() -> &'static Profile {
    static PROFILE: std::sync::OnceLock<Profile> = std::sync::OnceLock::new();
    PROFILE.get_or_init(|| {
        let limits = limits();
        let jig = jig_client_view::words::Words::new(
            jig_client_view::phrases::english::english(limits.jig.text).expect("jig English fits"),
            0,
            0,
        );
        let obs = view::words::Words::english(1, limits.observability.text).expect("Obs English fits");
        let infra = view::words::Words::english(2, limits.infrastructure.text).expect("Infra English fits");
        let text_per_node = jig
            .jig_capacity()
            .expect("jig saying bound")
            .max(obs.capacity().expect("Obs saying bound"))
            .max(infra.capacity().expect("Infra saying bound"))
            .checked_mul(2)
            .expect("node text bound");
        let bounds = view::Limits {
            tree: skein_ui::Limits {
                nodes: 1024,
                depth: 128,
                text_per_node,
                text_bytes: 1024 * text_per_node,
                drawing_bytes: 32,
                styles_per_node: 4,
                patches: 5120,
                bindings: 1024,
                slots: 256,
                markdown_depth: 32,
                scene_marks: 0,
                scene_bytes: 0,
            },
        };

        let heap = client::worst_case(&limits).expect("whole joint root bound")
            + 2 * view::worst_case(&bounds, &limits, &jig, &obs, &infra)
                .expect("full joint view, bindings and books bound");
        Profile { bounds, jig, obs, infra, heap }
    })
}
#[must_use]
pub fn client(seed: u64) -> (client::Domain, view::View) {
    let limits = limits();
    let profile = profile();
    let mut domain = client::new(&limits, seed);
    client::configure_arrangement(&mut domain, view::arrangement()).expect("ops arrangement before start");
    let mut view =
        view::View::new(&profile.bounds, &limits, profile.jig.clone(), profile.obs.clone(), profile.infra.clone(), 1);
    view.native_environments();
    (domain, view)
}
#[must_use]
pub fn world(seed: u64) -> World<Client, Joint> {
    world_for(seed, Story::Alert)
}
#[must_use]
pub fn environment_world(seed: u64) -> World<Client, Joint> {
    let mut world = world_for(seed, Story::Environment);
    let front = world.front();
    let fact = front
        .engine
        .systems
        .infrastructure(infra::SystemRequest::Pool { pool: infra::Pool(b"staging".as_slice().into()) });
    front.source(root::Event::Infrastructure(infra::Event::System(fact)));
    world
}
fn world_for(seed: u64, story: Story) -> World<Client, Joint> {
    let front = Joint::for_story(seed, limits(), story);
    let mut world = World::new(
        Settings { seed, heap_bytes_per_client: 64 * 1024 * 1024, iterations: 4096, ..Settings::default() },
        front,
    );
    // Draw transport faults explicitly from the real fault sources below;
    // the generic person still explores every kind of displayed binding.
    world.randomness(2, 0, 0);
    world
}
pub fn watch_setup(front: &mut Joint) {
    front.source(root::Event::Infrastructure(infra::Event::Names {
        project: 1,
        task: 1,
        resources: Box::new([infra::Resource::Service(infra::Service::new(
            b"production".as_slice().into(),
            b"checkout".as_slice().into(),
        ))]),
    }));
    front.source(root::Event::Core(core::Event::Tasks(tasks::Event::Kinds {
        connector: 2,
        kinds: Box::new([tasks::Kind {
            connector: 2,
            kind: 1,
            hold: tasks::HoldKind::Exclusive { taken: tasks::Taken::Waits },
        }]),
    })));
}
pub fn alert(front: &mut Joint) {
    let service = obs::Service::new(b"production".as_slice().into(), b"checkout".as_slice().into());
    for number in [7, 8, 9] {
        front.source(root::Event::Observability(obs::Event::System(obs::SystemEvent::Alert {
            own: false,
            alert: obs::Alert { number, service: service.clone(), severity: 1 },
        })));
    }
    front.engine.drain().expect("actual noise triage");
    // Triage advances the independent engine; release its facts and packets
    // only when the common client clock reaches the drain's completion.
    front.due = Some(skein_lib::Time::from_nanos(front.engine.clock().now));
    front
        .engine
        .systems
        .production
        .inject_incident(&jig_ops_fake_production::ServiceName::new("production", "checkout"));
    front.source(root::Event::Observability(obs::Event::System(obs::SystemEvent::Alert {
        own: false,
        alert: obs::Alert { number: 12, service, severity: 3 },
    })));
}
/// Refresh through the independent production API, never manufacture healthy facts.
pub fn refresh(front: &mut Joint) {
    let service = infra::Service::new(b"production".as_slice().into(), b"checkout".as_slice().into());
    let fact = front.engine.systems.infrastructure(infra::SystemRequest::Service { service });
    front.source(root::Event::Infrastructure(infra::Event::System(fact)));
    let service = obs::Service::new(b"production".as_slice().into(), b"checkout".as_slice().into());
    let fact = front.engine.systems.observability(obs::SystemRequest::Facts { service });
    front.source(root::Event::Observability(obs::Event::System(fact)));
}
#[must_use]
pub fn source_limits(seed: u64) -> root::Limits {
    *Config::new(seed, Story::Alert).client_backed().client_recovery().limits()
}

/// Same Ops root/shell observer with the joint world's declared receiving profile.
pub struct Client;
impl jig_client_world::harness::App for Client {
    type Root = client::Domain;
    type View = view::View;
    type Limits = client::Limits;
    type Event = client::Event;
    type Request = client::Request;
    type Record = client::store::Record;
    type Key = client::store::Key;
    type Address = client::Address;
    fn limits() -> Self::Limits {
        limits()
    }
    fn new(seed: u64) -> (Self::Root, Self::View) {
        client(seed)
    }
    fn start(address: Self::Address, kept: Vec<Self::Record>, offset: i32) -> Self::Event {
        Ops::start(address, kept, offset)
    }
    fn went(address: Self::Address) -> Self::Event {
        Ops::went(address)
    }
    fn step(
        root: &mut Self::Root,
        env: &skein_lib::Env<Self::Limits>,
        event: Self::Event,
        out: &mut skein_lib::Queue<Self::Request>,
    ) {
        Ops::step(root, env, event, out);
    }
    fn fire(root: &mut Self::Root, env: &skein_lib::Env<Self::Limits>, out: &mut skein_lib::Queue<Self::Request>) {
        Ops::fire(root, env, out);
    }
    fn take(root: &mut Self::Root, env: &skein_lib::Env<Self::Limits>, out: &mut skein_lib::Queue<Self::Request>) {
        Ops::take(root, env, out);
    }
    fn next_deadline(root: &Self::Root) -> Option<skein_lib::Time> {
        Ops::next_deadline(root)
    }
    fn build(view: &mut Self::View, root: &Self::Root, patches: &mut skein_lib::Queue<skein_ui::Patch>) {
        Ops::build(view, root, patches);
    }
    fn tree(view: &Self::View) -> &skein_ui::Tree {
        Ops::tree(view)
    }
    fn act(view: &mut Self::View, node: skein_ui::NodeId, act: skein_ui::Act) -> Option<Self::Event> {
        Ops::act(view, node, act)
    }
    fn duty(request: Self::Request) -> jig_client_world::harness::Duty<Self> {
        use jig_client_world::harness::Duty;
        match Ops::duty(request) {
            Duty::Store(value) => Duty::Store(value),
            Duty::Wire(value) => Duty::Wire(value),
            Duty::Show { address, replace } => Duty::Show { address, replace },
            Duty::SignIn { provider } => Duty::SignIn { provider },
            Duty::SignOut => Duty::SignOut,
            Duty::Fact => Duty::Fact,
        }
    }
    fn record_key(record: &Self::Record) -> Self::Key {
        Ops::record_key(record)
    }
    fn unsent(request: Self::Request) -> Option<Self::Event> {
        Ops::unsent(request)
    }
    fn view_limits(view: &Self::View) -> skein_ui::Limits {
        Ops::view_limits(view)
    }
    fn patch_limit(view: &Self::View) -> u32 {
        Ops::patch_limit(view)
    }
    fn worst_case(supplied: &Self::Limits) -> u64 {
        assert_eq!(*supplied, limits(), "constructor and receiving bounds match");
        profile().heap
    }
    fn seen(tree: &skein_ui::Tree) -> Vec<jig_client_world::harness::Seen> {
        Ops::seen(tree)
    }
    fn sent(request: &Self::Request) -> Option<jig_client_world::harness::Sent> {
        Ops::sent(request)
    }
    fn received(event: &Self::Event) -> Vec<jig_client_world::harness::Receipt> {
        Ops::received(event)
    }
    fn card(tree: &skein_ui::Tree, node: skein_ui::NodeId) -> Option<jig_client_world::harness::CardKey> {
        Ops::card(tree, node)
    }
    fn window_changed(event: &Self::Event) -> bool {
        Ops::window_changed(event)
    }
}

#[must_use]
pub fn see(words: &[u8]) -> skein_fake_person::Step<client::Address> {
    skein_fake_person::Step::See {
        find: skein_fake_person::Find::text(words),
        within: skein_lib::Duration::from_secs(3),
    }
}

#[must_use]
pub fn press(role: skein_ui::Role, words: &[u8]) -> skein_fake_person::Step<client::Address> {
    skein_fake_person::Step::Press { find: skein_fake_person::Find::named(role, words) }
}

#[must_use]
pub fn incident_lead(engine: &jig_conformance::Harness<super::engine::OpsApplication>) -> u64 {
    let mut rows = engine.store.rows.values();
    rows.find_map(|row| match row {
        jig_ops_domain::Record::Observability(jig_ops_domain_observability::Record::Incident(incident))
            if incident.project == 1 && incident.number == 12 =>
        {
            incident.lead
        }
        root::Record::Core(_) | root::Record::Observability(_) | root::Record::Infrastructure(_) => None,
    })
    .expect("incident 12 has its retained lead")
}

#[must_use]
pub fn remediation(engine: &jig_conformance::Harness<super::engine::OpsApplication>, lead: u64) -> u64 {
    let mut rows = engine.store.rows.values();
    rows.find_map(|row| match row {
        jig_ops_domain::Record::Core(jig_domain_core::Record::Tasks(
            jig_domain_tasks::Stored::Live(task) | jig_domain_tasks::Stored::Ended(task),
        )) if task.requester == jig_domain_tasks::Party::Task(lead) => Some(task.number),
        root::Record::Core(_) | root::Record::Observability(_) | root::Record::Infrastructure(_) => None,
    })
    .expect("the lead has its remediation delegate")
}
