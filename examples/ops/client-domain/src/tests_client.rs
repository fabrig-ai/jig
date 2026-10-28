//! Routing assertions on the root's client requests, without observations.
use super::{contract, env, infrastructure, load_environment, ready, until};
use crate::{Action, Event, Request};
use jig_client::{Asker, Handle, Key};
use skein_lib::Queue;
#[test]
fn passes_the_client_s_transport_requests_out_in_order() {
    let mut root = ready(81);
    let key = Key { asker: Asker::JIG, number: 99 };
    let ask = jig_client::Ask::Jig(Box::new(contract::Ask::Stop { project: 1, task: 9 }));
    let mut out = Queue::uncapped();
    crate::step(
        &mut root,
        &env(),
        Event::Client { event: jig_client::Event::Send { name: None, key, request: ask.clone() } },
        &mut out,
    );
    let Some(Request::Save { record: crate::store::Record::Unlinked(record) }) = out.pop() else {
        panic!("the client journals before sending")
    };
    assert_eq!(record.request, jig_client::Held { key, request: ask.clone() });
    let Some(Request::Client {
        request: jig_client::Request::Down { down: jig_client::Down::Send { key, request, .. } },
    }) = out.pop()
    else {
        panic!("the original transport send follows its journal")
    };
    assert_eq!(key, record.key);
    assert_eq!(request, ask);
    assert!(out.is_empty());
}
#[test]
fn a_tell_under_an_asker_no_connector_names_reaches_no_child() {
    let mut root = ready(82);
    let handle = Handle { asker: Asker(17), number: 9 };
    let mut out = Queue::uncapped();
    crate::step(
        &mut root,
        &env(),
        Event::Client {
            event: jig_client::Event::Read { handle, read: jig_client::Readable::Jig(contract::Read::Deployment) },
        },
        &mut out,
    );
    let Some(Request::Client { request: jig_client::Request::Down { down: jig_client::Down::Read { read, .. } } }) =
        out.pop()
    else {
        panic!("the client issued the read")
    };
    assert!(out.is_empty());
    let shown = crate::shown(&root);
    crate::step(
        &mut root,
        &env(),
        Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::ReadDone {
                    read,
                    result: jig_client::Readout::Jig(contract::ReadResult::None(contract::ReadNone::Unknown)),
                },
            },
        },
        &mut out,
    );
    assert_eq!(crate::shown(&root), shown);
    assert!(out.is_empty());
}
#[test]
fn routes_an_infrastructure_tell_to_its_side() {
    let mut root = ready(83);
    let object = load_environment(&mut root);
    until(&mut root, object.token());
    let mut out = Queue::uncapped();
    crate::step(
        &mut root,
        &env(),
        Event::Act {
            action: Action::Infrastructure(infrastructure::Act::MoveEnd {
                object: object.token(),
                revision: contract::Revision { value: 2 },
            }),
        },
        &mut out,
    );
    let attempt = (0..out.len())
        .find_map(|_| match out.pop().expect("counted output") {
            Request::Client { request: jig_client::Request::Down { down: jig_client::Down::Send { attempt, .. } } } => {
                Some(attempt)
            }
            _ => None,
        })
        .expect("the infrastructure request left");
    let answer = infrastructure::Answer::MovedEnd {
        environment: 5,
        revision: contract::Revision { value: 3 },
        until: skein_lib::Wall::from_nanos(100_000_000_000),
    };
    crate::step(
        &mut root,
        &env(),
        super::support::final_answer(attempt, crate::Answered::Infrastructure(answer.clone())),
        &mut out,
    );
    assert_eq!(root.infrastructure().object(object).expect("live object").card, infrastructure::Card::Done { answer });
}
