use super::*;
use jig_client_protocol_link::{Heard, Message};
use skein_http::{Header, Method};
use skein_lib::{Token, bytes};
mod fixtures;
mod stream;
use fixtures::{credential, mapping};
const EXCHANGE: u64 = 11;
const ROUTES: [&[u8]; 9] = [
    b"/jig/v1/request",
    b"/jig/v1/read",
    b"/jig/v1/watch",
    b"/jig/v1/unwatch",
    b"/jig/v1/stream-gone",
    b"/jig/v1/sign-in/link",
    b"/jig/v1/sign-in/deployment",
    b"/jig/v1/sign-out",
    b"/jig/v1/authorize",
];
const STATUSES: [u16; 4] = [201, 401, 500, 503];
#[test]
fn a_keyed_request_is_a_post_under_the_prefix_with_its_bearer() {
    let call = exchange(&mapping(), Some(&credential()), Message::Request(Box::from(*b"keyed")));
    assert_eq!(call.method, Method::Post);
    assert_eq!(call.path.as_ref(), b"/jig/v1/request");
    assert_eq!(call.body.as_ref(), b"keyed");
    let bearer = Header {
        name: Box::from(*b"Authorization"),
        value: Box::from(*b"Bearer CQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQ"),
    };
    assert!(call.headers.as_slice().contains(&bearer));
    assert!(call.headers.as_slice().contains(&Header {
        name: bytes::copy_of(b"Content-Type"),
        value: bytes::copy_of(b"application/octet-stream"),
    }));
}
#[test]
fn each_message_maps_to_its_route_under_the_prefix() {
    for (message, route) in fixtures::messages().into_iter().zip(ROUTES) {
        let call = exchange(&mapping(), None, message);
        assert_eq!(call.method, Method::Post, "{route:?}");
        assert_eq!(call.path.as_ref(), route, "{route:?}");
    }
}
#[test]
fn a_message_with_no_credential_carries_no_bearer() {
    let call = exchange(&mapping(), None, Message::SignOut);
    assert_eq!(call.headers.len(), 1);
    assert_eq!(call.headers.as_slice()[0].name.as_ref(), b"Content-Type");
}
#[test]
fn a_status_other_than_200_is_lost() {
    for status in STATUSES {
        assert_eq!(
            answered(Token::new(EXCHANGE), status, Box::new([])),
            Heard::Lost { exchange: Token::new(EXCHANGE) },
            "{status}"
        );
    }
}
#[test]
fn a_whole_successful_response_is_a_reply() {
    assert_eq!(
        answered(Token::new(EXCHANGE), 200, Box::from(*b"reply")),
        Heard::Reply { exchange: Token::new(EXCHANGE), message: Box::from(*b"reply") }
    );
}
#[test]
fn a_printed_link_gives_its_host_and_secret() {
    let result = printed(
        &mapping(),
        b"http://127.0.0.1:8080/jig/v1/sign-in/link?secret=AwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwM",
    )
    .unwrap();
    assert_eq!(result.host.as_ref(), b"127.0.0.1:8080");
    assert_eq!(result.secret.expose(), &fixtures::LINK_SECRET);
}
#[test]
fn a_printed_link_under_another_prefix_is_refused() {
    assert_eq!(
        printed(
            &mapping(),
            b"http://127.0.0.1:8080/other/v1/sign-in/link?secret=AwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwM"
        ),
        Err(Unlinked::NoSecret)
    );
}

#[test]
fn a_full_exchange_409_keeps_its_bound_and_size_and_malformed_conflicts_are_lost() {
    let exchange = Token::new(EXCHANGE);
    assert_eq!(
        answered(exchange, 409, Box::new([2, 0, 0, 0, 8])),
        Heard::Full {
            exchange,
            full: jig_client_domain::StreamFull { bound: jig_client_domain::StreamBound::Exchanges, size: 8 }
        }
    );
    for body in
        [Box::<[u8]>::from([]), Box::from([2, 0, 0, 8]), Box::from([3, 0, 0, 0, 8]), Box::from([2, 0, 0, 0, 8, 0])]
    {
        assert_eq!(answered(exchange, 409, body), Heard::Lost { exchange });
    }
}
