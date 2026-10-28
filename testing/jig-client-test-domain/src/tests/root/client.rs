use super::fixtures::{RECORD_ITEM, RECORD_PROJECT, UNOWNED_KEY_NUMBER};
use super::{confirm, drive, env, limits, out_iter, ready, start};
use crate::{Address, Event, Request, first, new, route};
use jig_client::Link;
use jig_client::{Asker, Key, Told, contract};
use skein_lib::{List, Queue};
const ME: u64 = 7;

#[test]
fn the_link_reaches_each_child() {
    let root = ready(7);
    assert_eq!(root.first().link(), Link::Live);
    assert_eq!(root.second().link(), Link::Live);
    assert_eq!(root.jig().link(), Link::Live);
}

#[test]
fn passes_the_client_s_transport_requests_out_in_order() {
    let mut root = new(&limits(), 7);
    let mut child = Queue::uncapped();
    child.push(jig_client::Request::OpenStream { credential: None });
    child.push(jig_client::Request::CloseStream);
    let mut out = Queue::uncapped();
    route::route_client(&mut root, &env(), &mut child, &mut out);
    assert_eq!(
        out_iter(out),
        vec![
            Request::Client { request: jig_client::Request::OpenStream { credential: None } },
            Request::Client { request: jig_client::Request::CloseStream },
        ]
    );
}

#[test]
fn a_kept_request_of_an_asker_no_owner_names_reaches_no_child() {
    let mut root = new(&limits(), 7);
    let before = crate::shown(&root);
    let mut out = Queue::uncapped();
    route::told(
        &mut root,
        &env(),
        Told::Kept {
            key: Key { asker: Asker(u32::MAX), number: UNOWNED_KEY_NUMBER },
            request: jig_client::Ask::Jig(Box::new(contract::Ask::Stop { project: 1, task: 9 })),
        },
        &mut out,
    );
    assert_eq!(out_iter(out), vec![]);
    assert_eq!(crate::shown(&root), before);
}

#[test]
fn routes_a_tell_to_the_child_its_asker_names() {
    let mut root = crate::unlinked(&limits(), 7);
    let watch = start(&mut root, List::with_capacity(0));
    confirm(&mut root, watch, 7);
    root.first = first::Side::new(&limits().first, 7);
    let mut out = Queue::uncapped();
    route::told(&mut root, &env(), Told::Changed { handle: first::me::handle() }, &mut out);
    assert_eq!(out_iter(out), vec![]);
    assert_eq!(root.first().party(), Some(7));
}

#[test]
fn the_reference_ui_s_records_leave_before_the_step_s_exchanges() {
    let mut root = ready(7);
    let rows = drive(
        &mut root,
        Event::Went {
            place: crate::Place::Address(Address::First(first::Address { project: RECORD_PROJECT, item: RECORD_ITEM })),
        },
    );
    let mut exchange = false;
    let mut stored = false;
    for row in rows {
        match row {
            Request::Client { .. } => exchange = true,
            Request::Save { .. } | Request::Erase { .. } => {
                assert!(!exchange, "child records precede transport work");
                stored = true;
            }
            Request::Show { .. }
            | Request::SignIn { .. }
            | Request::SignOut
            | Request::First(_)
            | Request::Fact { .. } => {}
        }
    }
    assert!(stored);
    assert!(exchange);
}

#[test]
fn a_family_side_reads_the_party_from_its_own_me_handle() {
    let mut root = crate::unlinked(&limits(), ME);
    let watch = start(&mut root, List::with_capacity(0));
    confirm(&mut root, watch, ME);
    let handle = first::me::handle();
    assert_eq!(handle.asker, Asker(u32::from(crate::FIRST)));
    assert_ne!(handle.asker, Asker::JIG);
    assert!(root.client().document(handle).is_some());
    root.first = first::Side::new(&limits().first, ME);
    root.jig = jig_client_view::Ui::new(
        &limits().jig,
        jig_client_view::page::navigation::Navigation::new(limits().jig.navigation_entries),
        jig_client_view::address::Address::Inbox { project: None },
        jig_client_view::page::Arrangement::none(),
    )
    .unwrap();
    let mut out = Queue::uncapped();
    route::told(&mut root, &env(), Told::Changed { handle }, &mut out);
    assert_eq!(root.first().party(), Some(ME));
    assert_eq!(root.jig().link(), Link::Starting);
    assert_eq!(out_iter(out), vec![]);
}
