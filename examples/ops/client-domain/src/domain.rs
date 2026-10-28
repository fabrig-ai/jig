//! Keep: the child routing shape and exhaustive connector variants.
//! Replace: Ops connector sides, family addresses and application vocabulary.
use crate::{Event, Limits, Request, infrastructure, route};
use skein_lib::{Env, Queue, Rng, Time};
/// The three children and their shared durable client.
#[derive(Debug)]
pub struct Domain {
    pub(crate) jig: jig_client_view::Ui,
    pub(crate) infrastructure: infrastructure::Side,
    pub(crate) observability: crate::observability::Side,
    pub(crate) client: jig_client::Client,
    pub(crate) salt: u64,
    pub(crate) unlinked: bool,
}
#[must_use]
pub fn new(limits: &Limits, seed: u64) -> Domain {
    build(limits, seed, false)
}
#[must_use]
pub fn unlinked(limits: &Limits, seed: u64) -> Domain {
    build(limits, seed, true)
}
fn build(limits: &Limits, seed: u64, unlinked: bool) -> Domain {
    assert!(crate::worst_case(limits).is_some(), "root capacities are checked");
    let mut rng = Rng::new(seed);
    let client = if unlinked {
        jig_client::Client::unlinked(&limits.client, rng.next_u64())
    } else {
        jig_client::Client::new(&limits.client, rng.next_u64())
    }
    .expect("checked client capacities");
    Domain {
        jig: jig_client_view::Ui::new(
            &limits.jig,
            jig_client_view::page::navigation::Navigation::new(limits.jig.navigation_entries),
            jig_client_view::address::Address::Inbox { project: None },
            jig_client_view::page::Arrangement::none(),
        )
        .expect("root's UI configuration is valid"),
        infrastructure: infrastructure::Side::new(&limits.infrastructure, rng.next_u64()),
        observability: crate::observability::Side::new(&limits.observability, rng.next_u64()),
        client,
        salt: rng.next_u64(),
        unlinked,
    }
}
impl Domain {
    #[must_use]
    pub const fn client(&self) -> &jig_client::Client {
        &self.client
    }

    #[must_use]
    pub const fn jig(&self) -> &jig_client_view::Ui {
        &self.jig
    }
    #[must_use]
    pub const fn infrastructure(&self) -> &infrastructure::Side {
        &self.infrastructure
    }
    #[must_use]
    pub const fn observability(&self) -> &crate::observability::Side {
        &self.observability
    }
}
pub fn step(root: &mut Domain, env: &Env<Limits>, event: Event, out: &mut Queue<Request>) {
    route::event(root, env, event, out);
}
pub fn fire(root: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    let mut child = Queue::uncapped();
    root.jig.fire(env.now, env.wall, &root.client, &mut child);
    route::route_jig(root, env, &mut child, out);
    let mut child = Queue::uncapped();
    infrastructure::fire(
        &mut root.infrastructure,
        &Env { now: env.now, wall: env.wall, limits: env.limits.infrastructure },
        &mut child,
    );
    route::route_infrastructure(root, env, &mut child, out);
    let mut child = Queue::uncapped();
    crate::observability::fire(
        &mut root.observability,
        &Env { now: env.now, wall: env.wall, limits: env.limits.observability },
        &mut child,
    );
    route::route_observability(root, env, &mut child, out);
    let mut client = Queue::uncapped();
    root.client.fire(env.now, env.wall, &mut client);
    route::route_client(root, env, &mut client, out);
}
#[must_use]
pub fn next_deadline(root: &Domain) -> Option<Time> {
    let mut next = jig_client_view::next_deadline(&root.jig);
    for deadline in [
        infrastructure::next_deadline(&root.infrastructure),
        crate::observability::next_deadline(&root.observability),
        root.client.next_deadline(),
    ]
    .into_iter()
    .flatten()
    {
        next = match next {
            Some(old) => Some(old.min(deadline)),
            None => Some(deadline),
        };
    }
    next
}
/// Reclaim the client and children after every tell has been routed.
pub fn take(root: &mut Domain, _env: &Env<Limits>, _out: &mut Queue<Request>) {
    root.client.reclaim();
    jig_client_view::reclaim(&mut root.jig);
    infrastructure::reclaim(&mut root.infrastructure);
    crate::observability::reclaim(&mut root.observability);
}
#[must_use]
pub fn shown(root: &Domain) -> u64 {
    root.jig
        .shown()
        .checked_add(infrastructure::shown(&root.infrastructure))
        .expect("infrastructure shown count fits")
        .checked_add(crate::observability::shown(&root.observability))
        .expect("observability shown count fits")
}

/// Installs the application's navigation before Start; the jig child validates its bound.
pub fn configure_navigation(
    root: &mut Domain,
    navigation: jig_client_view::page::navigation::Navigation,
) -> Result<(), jig_client_view::page::navigation::Navigation> {
    root.jig.configure_navigation(navigation)
}

/// Installs the application's arrangement before Start; jig validates it.
pub fn configure_arrangement(
    root: &mut Domain,
    arrangement: jig_client_view::page::Arrangement,
) -> Result<(), jig_client_view::page::Arrangement> {
    root.jig.configure_arrangement(arrangement)
}
