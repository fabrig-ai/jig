//! A keyed request's carried value: kept before the application sees the
//! request, which names it by number and digest only (protocol/client.md, 5;
//! store.md 9).
use super::fixtures::{self as f, Exchange};
use crate::{Event, FromApplication, Request, SecretOwner, SecretsAnswer, SecretsRequest, ToApplication};
use jig_client_protocol_wire::v19 as w;
use jig_domain_core as core;
use jig_domain_people as p;
use skein_lib::{ReplyTo, Secret, Token, Writer};

const FIRST: u64 = 1 << 63;

fn carrying(owner: w::Owner, value: &[u8]) -> Box<[u8]> {
    let (request, version) = match owner {
        w::Owner::Jig => (f::encode_ask(&f::start(&f::wire())), 19),
        w::Owner::Family(_) => (f::family_request(Box::from(b"abc".as_slice())), 2),
    };
    keyed_value(owner, version, request, value)
}
fn keyed_value(owner: w::Owner, version: u32, request: Box<[u8]>, value: &[u8]) -> Box<[u8]> {
    let record = w::Keyed::new(
        &f::wire(),
        w::KeyedParts { key: [5; 16], owner, version, request, value: Some(Secret::new(Box::from(value))) },
    )
    .expect("carrying keyed");
    let mut out = Writer::new(usize::try_from(record.measure()).expect("short keyed record"));
    record.encode(&mut out).expect("bounded keyed");
    out.finish()
}
fn family() -> w::Owner {
    w::Owner::Family(w::Family::new(&f::wire(), w::FamilyParts { number: 9 }).expect("family"))
}
/// Sends a carrying family request; returns its connection and exchange
/// once the store has been asked to keep its value.
fn sent(exchange: &mut Exchange, value: &[u8]) -> (Token, Token) {
    let owner = exchange.send(crate::Message::Request(carrying(family(), value)));
    exchange.authenticate();
    let Some(Request::Secrets(SecretsRequest::Carry { exchange: name, value: kept, .. })) = exchange.out.pop() else {
        panic!("carry asked");
    };
    assert_eq!(kept.expose(), value);
    assert!(exchange.out.is_empty(), "the application waits for the kept value");
    (owner, name)
}
fn kept(exchange: &mut Exchange, name: Token, number: Option<u64>) {
    exchange.component.down(
        &exchange.env,
        Event::Secrets(SecretsAnswer::Carried { exchange: name, number }),
        &mut exchange.out,
    );
}
fn application(exchange: &mut Exchange) -> (ReplyTo, [u8; 32], core::Carried) {
    let Some(Request::Application(ToApplication::Request { reply_to, record, digest, carried, .. })) =
        exchange.out.pop()
    else {
        panic!("application request");
    };
    assert_eq!(record.as_ref(), b"abc");
    (reply_to, digest, carried.expect("carried value named"))
}
fn erased(exchange: &mut Exchange) -> SecretOwner {
    let Some(Request::Secrets(SecretsRequest::Erase { owner })) = exchange.out.pop() else {
        panic!("erase");
    };
    owner
}
fn down(exchange: &mut Exchange, event: Event) {
    exchange.component.down(&exchange.env, event, &mut exchange.out);
}

#[test]
fn a_carried_value_is_kept_before_the_application_sees_its_request() {
    let mut exchange = Exchange::new();
    let (owner, name) = sent(&mut exchange, b"key");
    kept(&mut exchange, name, Some(FIRST));
    let (to, digest, carried) = application(&mut exchange);
    assert_eq!(carried, core::Carried { number: FIRST, digest: skein_lib::sha256::digest(b"key") });
    assert_eq!(digest, crate::sign_in::carrying(b"abc", &Secret::new(Box::from(b"key".as_slice()))));
    down(
        &mut exchange,
        Event::Application(FromApplication::Answered { to, family: 9, record: Box::from(b"outcome".as_slice()) }),
    );
    assert_eq!(exchange.answer(owner), f::application_answer());
    assert!(exchange.out.is_empty(), "a used value stays kept");
    assert!(exchange.component.pending.carried.is_empty());
}
#[test]
fn another_value_under_the_same_record_is_another_request() {
    let first = crate::sign_in::carrying(b"abc", &Secret::new(Box::from(b"one".as_slice())));
    let second = crate::sign_in::carrying(b"abc", &Secret::new(Box::from(b"two".as_slice())));
    let moved = crate::sign_in::carrying(b"abcone", &Secret::new(Box::new([])));
    assert_ne!(first, second);
    assert_ne!(first, moved);
    assert_ne!(first.as_slice(), crate::sign_in::digest(b"abc").expose());
}
#[test]
fn a_value_the_core_leaves_unused_is_erased_once_its_number_is_named() {
    let mut exchange = Exchange::new();
    let (owner, name) = sent(&mut exchange, b"key");
    kept(&mut exchange, name, Some(FIRST));
    let (to, _, _) = application(&mut exchange);
    down(&mut exchange, Event::Application(FromApplication::Unused { number: FIRST + 1 }));
    assert!(exchange.out.is_empty(), "another decode's number is not this component's to erase");
    down(&mut exchange, Event::Application(FromApplication::Unused { number: FIRST }));
    assert_eq!(erased(&mut exchange), SecretOwner::Carried(FIRST));
    exchange.reply(to, p::Reply::Outcome(p::Outcome::Refused(p::Refusal::KeyConflict)));
    let w::Final::Refused(refused) = exchange.final_answer(owner) else { panic!("refused") };
    assert_eq!(refused.refusal(), &w::Refusal::KeyConflict);
    down(&mut exchange, Event::Application(FromApplication::Unused { number: FIRST }));
    assert!(exchange.out.is_empty(), "erased once");
    assert!(exchange.component.pending.carried.is_empty());
}
#[test]
fn an_undecodable_carrying_request_erases_its_value() {
    let mut exchange = Exchange::new();
    let (owner, name) = sent(&mut exchange, b"key");
    kept(&mut exchange, name, Some(FIRST));
    let (to, _, _) = application(&mut exchange);
    down(&mut exchange, Event::Application(FromApplication::Undecodable { to }));
    assert_eq!(erased(&mut exchange), SecretOwner::Carried(FIRST));
    assert_eq!(exchange.response(owner).0, 400);
    assert!(exchange.out.is_empty());
}
#[test]
fn a_value_the_store_did_not_keep_is_answered_busy_unseen() {
    let mut exchange = Exchange::new();
    let (owner, name) = sent(&mut exchange, b"key");
    kept(&mut exchange, name, None);
    assert_eq!(exchange.answer(owner), w::KeyedAnswer::Busy);
    assert!(exchange.out.is_empty(), "nothing kept, nothing seen");
    assert!(exchange.component.pending.carried.is_empty());
}
#[test]
fn a_value_kept_after_its_sign_in_ended_is_erased_unseen() {
    let mut exchange = Exchange::new();
    let (owner, name) = sent(&mut exchange, b"key");
    down(&mut exchange, Event::SignInEnded { sign_in: f::NUMBER });
    assert_eq!(exchange.answer(owner), w::KeyedAnswer::SignedOut);
    assert_eq!(erased(&mut exchange), SecretOwner::SignIn(f::NUMBER));
    assert!(!exchange.component.settled(), "a carry the store has not answered");
    kept(&mut exchange, name, Some(FIRST));
    assert_eq!(erased(&mut exchange), SecretOwner::Carried(FIRST));
    assert!(exchange.out.is_empty(), "the application never sees it");
    assert!(exchange.component.pending.carried.is_empty());
}
#[test]
fn a_value_on_an_ordinary_jig_ask_is_not_a_record() {
    let mut exchange = Exchange::new();
    let owner = exchange.send(crate::Message::Request(carrying(w::Owner::Jig, b"key")));
    exchange.authenticate();
    assert_eq!(exchange.response(owner).0, 400);
    assert!(exchange.out.is_empty(), "nothing kept");
}

#[test]
fn every_carrying_request_names_its_values_kind() {
    // One application request and the three account requests that persist a value.
    const CARRYING_REQUESTS: u32 = 1 + 3;
    let limits = f::wire();
    let value = b"carried request payload";
    let digest = skein_lib::sha256::digest(value);
    let changes = [
        (
            w::AccountChange::AddKey(
                w::AccountChangeAddKey::new(&limits, w::AccountChangeAddKeyParts { provider: 3, join: None, digest })
                    .expect("add key"),
            ),
            crate::CarriedKind::Key,
        ),
        (
            w::AccountChange::ReplaceKey(
                w::AccountChangeReplaceKey::new(&limits, w::AccountChangeReplaceKeyParts { source: 29, digest })
                    .expect("replace key"),
            ),
            crate::CarriedKind::Key,
        ),
        (
            w::AccountChange::MoveIn(
                w::AccountChangeMoveIn::new(&limits, w::AccountChangeMoveInParts { digest }).expect("move in"),
            ),
            crate::CarriedKind::Moved,
        ),
    ];
    let mut rows = skein_lib::List::with_capacity(CARRYING_REQUESTS);
    rows.push((carrying(family(), value), crate::CarriedKind::Value)).expect("application row");
    for (change, kind) in changes {
        let ask = w::Ask::AccountChange(
            w::AskAccountChange::new(&limits, w::AskAccountChangeParts { holder: w::AccountHolder::Own, change })
                .expect("account change"),
        );
        rows.push((keyed_value(w::Owner::Jig, 19, f::encode_ask(&ask), value), kind)).expect("account row");
    }
    for (request, expected) in rows.into_boxed() {
        let mut exchange = Exchange::new();
        exchange.send(crate::Message::Request(request));
        exchange.authenticate();
        let Some(Request::Secrets(SecretsRequest::Carry { kind, .. })) = exchange.out.pop() else {
            panic!("persistent carry")
        };
        assert_eq!(kind, expected);
    }
}
