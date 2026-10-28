use jig_client::{AuthorizeWhy, Authorized, Client, Event, Limits, Message, Request};
use skein_lib::{Queue, Secret, Time, Token, Wall, Writer, bytes};
fn client() -> (Client, Queue<Request>) {
    let world = jig_client_world::test::limits();
    let limits = Limits { domain: world.client.domain, exchanges: 8 };
    let client = Client::new(&limits, 7).expect("bounded fixture");
    let out = Queue::uncapped();
    (client, out)
}
fn step(client: &mut Client, event: Event, out: &mut Queue<Request>) {
    client.step(Time::ZERO, Wall::EPOCH, event, out);
}
fn authorize(client: &mut Client, out: &mut Queue<Request>) -> Token {
    step(client, Event::Authorize { query: bytes::copy_of(b"client_id=app") }, out);
    let Some(Request::Exchange { exchange, credential, message: Message::Authorize(body) }) = out.pop() else {
        panic!("authorization exchange")
    };
    assert_eq!(body.as_ref(), b"\0\0\0\x0dclient_id=app");
    assert_eq!(credential, Some(Secret::new(bytes::copy_of(&[7; 40]))));
    assert!(out.is_empty(), "authorization is never saved");
    exchange
}
fn signed_client() -> (Client, Queue<Request>) {
    let (mut client, mut out) = client();
    step(&mut client, Event::SignIn { with: jig_client::With::Held(Secret::new(bytes::copy_of(&[7; 40]))) }, &mut out);
    while out.pop().is_some() {}
    (client, out)
}
// The scripted engine's v19 answers, independent of the codec under test.
fn answer(client: &mut Client, exchange: Token, message: Box<[u8]>, out: &mut Queue<Request>) {
    step(client, Event::Reply { exchange, message }, out);
}
fn go(location: &[u8]) -> Box<[u8]> {
    let mut writer = Writer::new(location.len().checked_add(5).expect("short redirect"));
    writer.put(&[0]).expect("go tag");
    writer.put(&u32::try_from(location.len()).expect("short redirect").to_be_bytes()).expect("location length");
    writer.put(location).expect("location bytes");
    writer.finish()
}

#[test]
fn authorization_answers_reach_the_composed_clients_surface_without_storage() {
    for (wire, expected) in [
        (
            go(b"https://app.example.com/cb?code=one"),
            Authorized::Go { location: bytes::copy_of(b"https://app.example.com/cb?code=one") },
        ),
        (Box::from([1, 3]), Authorized::Refused(AuthorizeWhy::NoProvider)),
        (Box::from([2]), Authorized::SignedOut),
        (Box::from([3]), Authorized::NotReady),
        (Box::from([4]), Authorized::Busy),
    ] {
        let (mut client, mut out) = signed_client();
        let exchange = authorize(&mut client, &mut out);
        answer(&mut client, exchange, wire, &mut out);
        assert_eq!(out.pop(), Some(Request::Authorized(expected)));
        assert!(out.is_empty());
    }
}
#[test]
fn a_failed_or_nonrecord_authorization_is_lost_and_releases_its_slot() {
    for malformed in [None, Some(Box::from([255])), Some(Box::from([2, 0]))] {
        let (mut client, mut out) = signed_client();
        let exchange = authorize(&mut client, &mut out);
        match malformed {
            Some(message) => step(&mut client, Event::Reply { exchange, message }, &mut out),
            None => step(&mut client, Event::Lost { exchange }, &mut out),
        }
        assert_eq!(out.pop(), Some(Request::Authorized(Authorized::Lost)));
        assert!(out.is_empty());
        authorize(&mut client, &mut out);
    }
}
#[test]
fn a_second_authorization_while_one_is_out_is_busy_without_an_exchange() {
    let (mut client, mut out) = signed_client();
    let first = authorize(&mut client, &mut out);
    step(&mut client, Event::Authorize { query: bytes::copy_of(b"second") }, &mut out);
    assert_eq!(out.pop(), Some(Request::Authorized(Authorized::Busy)));
    assert!(out.is_empty());
    answer(&mut client, first, Box::from([4]), &mut out);
    assert_eq!(out.pop(), Some(Request::Authorized(Authorized::Busy)));
    authorize(&mut client, &mut out);
}

#[test]
fn replacing_the_credential_terminates_an_outstanding_authorization_once() {
    let (mut client, mut out) = signed_client();
    let exchange = authorize(&mut client, &mut out);
    step(&mut client, Event::SignIn { with: jig_client::With::Held(Secret::new(bytes::copy_of(&[8; 40]))) }, &mut out);
    let terminals = out.iter().filter(|request| **request == Request::Authorized(Authorized::Lost)).count();
    assert_eq!(terminals, 1);
    while out.pop().is_some() {}
    answer(&mut client, exchange, Box::from([2]), &mut out);
    assert!(out.is_empty());
    assert_eq!(client.link().credential, Some(&Secret::new(bytes::copy_of(&[8; 40]))));
}
