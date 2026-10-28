//! Keep: the child routing shape and exhaustive connector variants.
//! Replace: Ops connector sides, family addresses and application vocabulary.
use crate::observability;
use crate::{
    Action, Address, Domain, Event, Fact, Limits, Place, Request, infrastructure,
    store::{Key, Record},
};
use skein_lib::{Env, List, Queue};

fn infrastructure_env(env: &Env<Limits>) -> Env<infrastructure::Limits> {
    Env { now: env.now, wall: env.wall, limits: env.limits.infrastructure }
}
pub(crate) fn jig(root: &mut Domain, env: &Env<Limits>, event: jig_client_view::Event, out: &mut Queue<Request>) {
    let mut child = Queue::uncapped();
    root.jig.step(env.now, env.wall, &root.client, event, &mut child);
    route_jig(root, env, &mut child, out);
}
pub(crate) fn infrastructure(
    root: &mut Domain,
    env: &Env<Limits>,
    event: infrastructure::Event,
    out: &mut Queue<Request>,
) {
    let mut child = Queue::uncapped();
    infrastructure::step(&mut root.infrastructure, &infrastructure_env(env), event, &mut child);
    route_infrastructure(root, env, &mut child, out);
}
pub(crate) fn observability(
    root: &mut Domain,
    env: &Env<Limits>,
    event: observability::Event,
    out: &mut Queue<Request>,
) {
    let mut child = Queue::uncapped();
    observability::step(
        &mut root.observability,
        &Env { now: env.now, wall: env.wall, limits: env.limits.observability },
        event,
        &mut child,
    );
    route_observability(root, env, &mut child, out);
}
pub(crate) fn event(root: &mut Domain, env: &Env<Limits>, event: Event, out: &mut Queue<Request>) {
    match event {
        Event::Start { address, kept, offset: _ } => {
            let mut bytes = List::with_capacity(env.limits.client.domain.requests.requests);
            let mut records = List::with_capacity(env.limits.client.domain.requests.requests);
            let mut jig_kept = List::with_capacity(env.limits.jig.records);
            for record in kept.into_boxed() {
                match record {
                    Record::Jig(record) => {
                        if jig_kept.push(record).is_err() {
                            out.push(Request::Fact { fact: Fact::InvalidRecord });
                        }
                    }
                    Record::Infrastructure(record) => {
                        infrastructure(root, env, infrastructure::Event::Restore { record }, out);
                    }
                    Record::Observability(record) => {
                        observability(root, env, observability::Event::Restore { record }, out);
                    }
                    Record::Client(record) => {
                        bytes.push(record).expect("bounded client records");
                    }
                    Record::Unlinked(record) => {
                        records.push(record).expect("bounded unlinked records");
                    }
                }
            }
            let jig_address = match address.clone() {
                Address::Jig(address) => address,
                Address::Infrastructure(_) | Address::Observability(_) => {
                    jig_client_view::address::Address::Inbox { project: None }
                }
            };
            let mut child = Queue::uncapped();
            root.jig.step(
                env.now,
                env.wall,
                &root.client,
                jig_client_view::Event::Start {
                    place: jig_client_view::address::Place::Jig(jig_address),
                    records: jig_kept,
                    seed: root.salt,
                },
                &mut child,
            );
            let event = if root.unlinked {
                jig_client::Event::StartUnlinked { kept: records }
            } else {
                jig_client::Event::Start { kept: bytes }
            };
            client(root, env, event, out);
            route_jig(root, env, &mut child, out);
            told(root, env, jig_client::Told::Link { link: root.client.link().state }, out);
            match address {
                Address::Jig(_) => {
                    infrastructure(root, env, infrastructure::Event::Left, out);
                    observability(root, env, observability::Event::Left, out);
                }
                Address::Infrastructure(address) => {
                    went(root, env, Place::Address(Address::Infrastructure(address)), out);
                }
                Address::Observability(address) => {
                    went(root, env, Place::Address(Address::Observability(address)), out);
                }
            }
        }
        Event::Went { place } => went(root, env, place, out),
        Event::Act { action } => match action {
            Action::SignIn => out.push(Request::SignIn { provider: 1 }),
            Action::Jig(act) => jig(root, env, jig_client_view::Event::Act { action: act }, out),
            Action::Infrastructure(act) => infrastructure(root, env, infrastructure::Event::Act { act }, out),
            Action::Observability(act) => observability(root, env, observability::Event::Act { act }, out),
        },
        Event::Client { event } => client(root, env, event, out),
        Event::Jig(event) => jig(root, env, event, out),
        Event::Infrastructure(event) => infrastructure(root, env, event, out),
        Event::Observability(event) => observability(root, env, event, out),
        Event::SignedOut => client(root, env, jig_client::Event::Up { up: jig_client::Up::SignedOut }, out),
    }
}
fn went(root: &mut Domain, env: &Env<Limits>, place: Place, out: &mut Queue<Request>) {
    match place {
        Place::Address(Address::Jig(address)) => {
            infrastructure(root, env, infrastructure::Event::Left, out);
            observability(root, env, observability::Event::Left, out);
            jig(root, env, jig_client_view::Event::Went { place: jig_client_view::address::Place::Jig(address) }, out);
        }
        Place::Address(Address::Infrastructure(address)) => {
            observability(root, env, observability::Event::Left, out);
            jig(root, env, jig_client_view::Event::Left, out);
            infrastructure(root, env, infrastructure::Event::Went { address }, out);
        }
        Place::Address(Address::Observability(address)) => {
            infrastructure(root, env, infrastructure::Event::Left, out);
            jig(root, env, jig_client_view::Event::Left, out);
            observability(root, env, observability::Event::Went { address }, out);
        }
        Place::Missing => {
            infrastructure(root, env, infrastructure::Event::Left, out);
            observability(root, env, observability::Event::Left, out);
            jig(root, env, jig_client_view::Event::Went { place: jig_client_view::address::Place::Missing }, out);
        }
    }
}
pub(crate) fn route_jig(
    root: &mut Domain,
    env: &Env<Limits>,
    child: &mut Queue<jig_client_view::Request>,
    out: &mut Queue<Request>,
) {
    for _ in 0..child.len() {
        let request = child.pop().expect("counted jig output");
        match request {
            jig_client_view::Request::Client { asked } => client(root, env, asked, out),
            jig_client_view::Request::Show { place, how } => match place {
                jig_client_view::address::Place::Jig(address) => out.push(Request::Show {
                    address: Address::Jig(address),
                    replace: how == jig_client_view::How::Replace,
                }),
                jig_client_view::address::Place::Elsewhere | jig_client_view::address::Place::Missing => {}
            },
            jig_client_view::Request::Save { records } => {
                for record in records.into_boxed() {
                    out.push(Request::Save { record: Record::Jig(record) });
                }
            }
            jig_client_view::Request::Erase { records } => {
                for key in records.into_boxed() {
                    out.push(Request::Erase { key: Key::Jig(key) });
                }
            }
            jig_client_view::Request::ShowPart { part, scope } => {
                crate::parts::show(root, env, part, scope, out);
            }
            jig_client_view::Request::PartsLeft => {
                infrastructure(root, env, infrastructure::Event::PartsLeft, out);
                observability(root, env, observability::Event::PartsLeft, out);
            }
        }
    }
}
pub(crate) fn route_infrastructure(
    root: &mut Domain,
    env: &Env<Limits>,
    child: &mut Queue<infrastructure::Request>,
    out: &mut Queue<Request>,
) {
    for _ in 0..child.len() {
        match child.pop().expect("counted infrastructure output") {
            infrastructure::Request::Client { asked } => client(root, env, asked, out),
            infrastructure::Request::Show { address } => {
                out.push(Request::Show { address: Address::Infrastructure(address), replace: false });
            }
            infrastructure::Request::Save { record } => {
                out.push(Request::Save { record: Record::Infrastructure(record) });
            }
            infrastructure::Request::Send { request, ask, .. } => crate::family::send(root, env, request, &ask, out),
            infrastructure::Request::Out(value) => match value {
                infrastructure::Out::Fact { fact: infrastructure::Fact::SignedOut } => {
                    client(root, env, jig_client::Event::Up { up: jig_client::Up::SignedOut }, out);
                }
                infrastructure::Out::Read { .. }
                | infrastructure::Out::Open { .. }
                | infrastructure::Out::Close { .. }
                | infrastructure::Out::Fact {
                    fact:
                        infrastructure::Fact::Full
                        | infrastructure::Fact::InvalidDocument
                        | infrastructure::Fact::ReadNotAllowed
                        | infrastructure::Fact::WatchTerminal { .. },
                } => {
                    crate::family::infrastructure::asked(root, env, value, out);
                }
            },
            infrastructure::Request::Named { objects, sections } => {
                jig(root, env, jig_client_view::Event::Named { objects, sections }, out);
            }
        }
    }
}
pub(crate) fn route_observability(
    root: &mut Domain,
    env: &Env<Limits>,
    child: &mut Queue<observability::Request>,
    out: &mut Queue<Request>,
) {
    for _ in 0..child.len() {
        match child.pop().expect("counted observability output") {
            observability::Request::Client { asked } => client(root, env, asked, out),
            observability::Request::Show { address } => {
                out.push(Request::Show { address: Address::Observability(address), replace: false });
            }
            observability::Request::Save { record } => {
                out.push(Request::Save { record: Record::Observability(record) });
            }
            observability::Request::Out(value) => match value {
                observability::Out::Fact { fact: observability::Fact::SignedOut } => {
                    client(root, env, jig_client::Event::Up { up: jig_client::Up::SignedOut }, out);
                }
                observability::Out::Read { .. }
                | observability::Out::Open { .. }
                | observability::Out::Close { .. }
                | observability::Out::Fact {
                    fact:
                        observability::Fact::Full
                        | observability::Fact::InvalidDocument
                        | observability::Fact::ReadNotAllowed
                        | observability::Fact::WatchTerminal { .. },
                } => {
                    crate::family::observability::asked(root, env, value, out);
                }
            },
            observability::Request::Named { objects, sections } => {
                jig(root, env, jig_client_view::Event::Named { objects, sections }, out);
            }
        }
    }
}
pub(crate) fn client(root: &mut Domain, env: &Env<Limits>, event: jig_client::Event, out: &mut Queue<Request>) {
    let mut child = Queue::uncapped();
    root.client.step(env.now, env.wall, event, &mut child);
    route_client(root, env, &mut child, out);
}
pub(crate) fn route_client(
    root: &mut Domain,
    env: &Env<Limits>,
    child: &mut Queue<jig_client::Request>,
    out: &mut Queue<Request>,
) {
    while let Some(request) = child.pop() {
        match request {
            jig_client::Request::Told { told: value } => told(root, env, value, out),
            jig_client::Request::Save { records } => {
                for record in records.into_boxed() {
                    out.push(Request::Save { record: Record::Client(record) });
                }
            }
            jig_client::Request::Erase { records } => {
                for key in records.into_boxed() {
                    out.push(Request::Erase { key: Key::Client(key) });
                }
            }
            jig_client::Request::Down { down: jig_client::Down::Save { record } } => {
                out.push(Request::Save { record: Record::Unlinked(record) });
            }
            jig_client::Request::Down { down: jig_client::Down::Erase { key } } => {
                out.push(Request::Erase { key: Key::Unlinked(key) });
            }
            request @ (jig_client::Request::Exchange { .. }
            | jig_client::Request::OpenStream { .. }
            | jig_client::Request::CloseStream
            | jig_client::Request::Down { .. }
            | jig_client::Request::Authorized(_)
            | jig_client::Request::ServiceCredential { .. }
            | jig_client::Request::SignOut) => out.push(Request::Client { request }),
        }
    }
}
pub(crate) fn told(root: &mut Domain, env: &Env<Limits>, told: jig_client::Told, out: &mut Queue<Request>) {
    use jig_client::Told;
    let asker = match &told {
        Told::Link { .. } => {
            infrastructure_me(root, env, &told, out);
            observability_me(root, env, &told, out);
            jig_client::Asker::JIG
        }
        Told::Dropped { .. } => jig_client::Asker::JIG,
        Told::Kept { key, .. } | Told::Pending { key } | Told::Answered { key, .. } => key.asker,
        Told::Changed { handle } | Told::Text { handle } | Told::Ended { handle, .. } | Told::Page { handle, .. } => {
            handle.asker
        }
    };
    match jig_client_view::Owner::of_asker(asker) {
        Some(jig_client_view::Owner::Jig) => {
            let mut child = Queue::uncapped();
            root.jig.step(env.now, env.wall, &root.client, jig_client_view::Event::Client { told }, &mut child);
            route_jig(root, env, &mut child, out);
        }
        Some(jig_client_view::Owner::Application { number }) => match jig_ops_domain_connectors::connector(number) {
            Some(jig_ops_domain_connectors::Connector::Infrastructure) if !infrastructure_me(root, env, &told, out) => {
                crate::family::infrastructure::told(root, env, told, out);
            }
            Some(jig_ops_domain_connectors::Connector::Observability) if !observability_me(root, env, &told, out) => {
                crate::family::observability::told(root, env, told, out);
            }
            Some(
                jig_ops_domain_connectors::Connector::Infrastructure
                | jig_ops_domain_connectors::Connector::Observability,
            )
            | None => {}
        },
        None => {}
    }
}

fn infrastructure_me(root: &mut Domain, env: &Env<Limits>, told: &jig_client::Told, out: &mut Queue<Request>) -> bool {
    let mut child = Queue::uncapped();
    let consumed = infrastructure::me_told(
        &mut root.infrastructure,
        &Env { now: env.now, wall: env.wall, limits: env.limits.infrastructure },
        &root.client,
        told,
        &mut child,
    );
    route_infrastructure(root, env, &mut child, out);
    consumed
}

fn observability_me(root: &mut Domain, env: &Env<Limits>, told: &jig_client::Told, out: &mut Queue<Request>) -> bool {
    let mut child = Queue::uncapped();
    let consumed = observability::me_told(
        &mut root.observability,
        &Env { now: env.now, wall: env.wall, limits: env.limits.observability },
        &root.client,
        told,
        &mut child,
    );
    route_observability(root, env, &mut child, out);
    consumed
}
