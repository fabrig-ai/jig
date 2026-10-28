//! The application-held client and its three presentation children (root.md, 3).
use crate::{Event, Limits, Request, first, route};
use skein_lib::{Env, Queue, Rng, Time};
#[derive(Debug)]
pub struct Domain {
    pub(crate) jig: jig_client_view::Ui,
    pub(crate) first: first::Side,
    pub(crate) second: crate::second::Side,
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
    assert!(crate::valid(limits), "root capacities are checked");
    let mut rng = Rng::new(seed);
    let client = if unlinked {
        jig_client::Client::unlinked(&limits.client, rng.next_u64())
    } else {
        jig_client::Client::new(&limits.client, rng.next_u64())
    }
    .expect("checked client capacities");
    let mut navigation = jig_client_view::page::navigation::Navigation::new(limits.jig.navigation_entries);
    navigation
        .entries
        .push(jig_client_view::page::navigation::Destination::Jig(jig_client_view::address::Address::Conversations {
            project: None,
        }))
        .expect("root navigation has room for Conversations");
    Domain {
        jig: jig_client_view::Ui::new(
            &limits.jig,
            navigation,
            jig_client_view::address::Address::Inbox { project: None },
            jig_client_view::page::Arrangement::none(),
        )
        .expect("root's UI configuration is valid"),
        first: first::Side::new(&limits.first, rng.next_u64()),
        second: crate::second::Side::new(limits.second_parts, limits.first.segments, limits.first.text, rng.next_u64()),
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
    pub const fn first(&self) -> &first::Side {
        &self.first
    }
    #[must_use]
    pub const fn second(&self) -> &crate::second::Side {
        &self.second
    }
}
pub fn step(root: &mut Domain, env: &Env<Limits>, event: Event, out: &mut Queue<Request>) {
    route::event(root, env, event, out);
    children_first(env, out);
}
pub fn fire(root: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    let mut child = Queue::with_capacity(jig_client_view::max_out(&env.limits.jig));
    root.jig.fire(env.now, env.wall, &root.client, &mut child);
    route::route_jig(root, env, &mut child, out);
    let mut child = Queue::with_capacity(first::max_out(&env.limits.first));
    first::fire(&mut root.first, &Env { now: env.now, wall: env.wall, limits: env.limits.first }, &mut child);
    route::route_first(root, env, &mut child, out);
    let mut client = Queue::with_capacity(jig_client::max_out(&env.limits.client));
    root.client.fire(env.now, env.wall, &mut client);
    route::route_client(root, env, &mut client, out);
    children_first(env, out);
}
#[must_use]
pub fn next_deadline(root: &Domain) -> Option<Time> {
    [jig_client_view::next_deadline(&root.jig), first::next_deadline(&root.first), root.client.next_deadline()]
        .into_iter()
        .flatten()
        .min()
}
/// Reclaim child state after all of the step's tells have been routed.
pub fn take(root: &mut Domain, _env: &Env<Limits>, _out: &mut Queue<Request>) {
    root.client.reclaim();
    jig_client_view::reclaim(&mut root.jig);
    first::reclaim(&mut root.first);
}
#[must_use]
pub fn shown(root: &Domain) -> u64 {
    root.jig
        .shown()
        .checked_add(first::shown(&root.first))
        .expect("first shown count fits")
        .checked_add(root.second.shown())
        .expect("second shown count fits")
}

/// Installs the application's arrangement before Start; jig validates it.
pub fn configure_arrangement(
    root: &mut Domain,
    arrangement: jig_client_view::page::Arrangement,
) -> Result<(), jig_client_view::page::Arrangement> {
    root.jig.configure_arrangement(arrangement)
}

fn children_first(env: &Env<Limits>, out: &mut Queue<Request>) {
    let mut client = Queue::with_capacity(crate::max_out(&env.limits));
    for _ in 0..out.len() {
        let request = out.pop().expect("counted root output");
        match request {
            Request::Client { .. } => client.push(request),
            request @ (Request::Save { .. }
            | Request::Erase { .. }
            | Request::First(_)
            | Request::Show { .. }
            | Request::SignIn { .. }
            | Request::SignOut
            | Request::Fact { .. }) => out.push(request),
        }
    }
    while let Some(request) = client.pop() {
        out.push(request);
    }
}
