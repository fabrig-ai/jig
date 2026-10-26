//! Account move values are handed to one exchange and bounded before keeping.
use super::fixtures::{self as f, Exchange};
use crate::{Event, Message, Request, SecretOwner, SecretsAnswer, SecretsRequest};
use jig_client_protocol_wire::v19 as w;
use jig_domain_core as core;
use jig_domain_people as p;
use skein_lib::{ReplyTo, Secret, Token, Writer};

fn change(change: w::AccountChange) -> w::Ask {
    w::Ask::AccountChange(
        w::AskAccountChange::new(&f::wire(), w::AskAccountChangeParts { holder: w::AccountHolder::Own, change })
            .unwrap(),
    )
}
fn outgoing(exchange: &mut Exchange) -> (Token, ReplyTo) {
    let ask = change(w::AccountChange::MoveOut(
        w::AccountChangeMoveOut::new(&f::wire(), w::AccountChangeMoveOutParts { account: 19 }).unwrap(),
    ));
    let owner = exchange.send(Message::Request(f::keyed(w::Owner::Jig, 19, f::encode_ask(&ask))));
    exchange.authenticate();
    let Some(Request::Core(event)) = exchange.out.pop() else { panic!("core ask") };
    let core::Event::People(p::Event::Ask { reply_to, ask, .. }) = *event else { panic!("people ask") };
    assert_eq!(
        ask,
        p::Ask::AccountChange { holder: p::AccountHolder::Own, change: p::AccountChange::MoveOut { account: 19 } }
    );
    (owner, reply_to)
}
fn ready(exchange: &mut Exchange, to: ReplyTo) -> Token {
    let expected = to.into_token();
    exchange.reply(
        ReplyTo::new(expected),
        p::Reply::Outcome(p::Outcome::AccountMovingOut { account: 19, number: 23, source: 29 }),
    );
    let Some(Request::TakeMoved { exchange, source: 29 }) = exchange.out.pop() else { panic!("take from root") };
    assert_eq!(exchange, expected);
    exchange
}
fn taken(exchange: &mut Exchange, token: Token, record: Option<Secret>) {
    exchange.component.down(&exchange.env, Event::MovedTaken { exchange: token, record }, &mut exchange.out);
}
fn record(exchange: &Exchange, owner: Token) -> Secret {
    let w::Final::Done(done) = exchange.final_answer(owner) else { panic!("done") };
    let w::Outcome::Made(made) = done.outcome() else { panic!("made") };
    let w::Made::MovingOut(row) = made.what() else { panic!("moving out") };
    assert_eq!((row.account(), row.number()), (19, 23));
    row.record().clone()
}
#[test]
fn the_record_goes_to_the_first_exchange_and_a_replay_is_empty() {
    let mut exchange = Exchange::new();
    let (owner, to) = outgoing(&mut exchange);
    let token = ready(&mut exchange, to);
    assert!(exchange.component.connections.get(&owner).unwrap().response.is_none());
    taken(&mut exchange, token, Some(Secret::new(Box::from(b"private moved file".as_slice()))));
    assert_eq!(record(&exchange, owner).expose(), b"private moved file");
    exchange.component.connections.remove(&owner);
    let (owner, to) = outgoing(&mut exchange);
    let token = ready(&mut exchange, to);
    taken(&mut exchange, token, None);
    assert!(record(&exchange, owner).expose().is_empty());
    assert!(exchange.out.is_empty());
}
#[test]
fn a_join_has_no_second_record_and_does_not_answer_its_sibling() {
    let mut exchange = Exchange::new();
    let (first, to) = outgoing(&mut exchange);
    let (join, joined) = outgoing(&mut exchange);
    let first_token = ready(&mut exchange, to);
    let join_token = ready(&mut exchange, joined);
    taken(&mut exchange, first_token, Some(Secret::new(Box::from(b"file".as_slice()))));
    assert_eq!(record(&exchange, first).expose(), b"file");
    assert!(exchange.component.connections.get(&join).unwrap().response.is_none());
    taken(&mut exchange, join_token, None);
    assert!(record(&exchange, join).expose().is_empty());
    assert!(exchange.out.is_empty());
}
#[test]
fn a_sign_in_ended_while_taking_never_releases_the_file() {
    let mut exchange = Exchange::new();
    let (owner, to) = outgoing(&mut exchange);
    let token = ready(&mut exchange, to);
    exchange.component.down(&exchange.env, Event::SignInEnded { sign_in: f::NUMBER }, &mut exchange.out);
    assert_eq!(exchange.answer(owner), w::KeyedAnswer::SignedOut);
    let Some(Request::Secrets(SecretsRequest::Erase { .. })) = exchange.out.pop() else { panic!("erase sign-in") };
    taken(&mut exchange, token, Some(Secret::new(Box::from(b"file".as_slice()))));
    assert_eq!(exchange.answer(owner), w::KeyedAnswer::SignedOut);
    assert!(exchange.out.is_empty());
}
fn carried(exchange: &mut Exchange, kind: u8, value: &[u8], digest: [u8; 32]) -> Token {
    let limits = f::wire();
    let ask = change(match kind {
        0 => w::AccountChange::AddKey(
            w::AccountChangeAddKey::new(&limits, w::AccountChangeAddKeyParts { provider: 3, join: None, digest })
                .unwrap(),
        ),
        1 => w::AccountChange::ReplaceKey(
            w::AccountChangeReplaceKey::new(&limits, w::AccountChangeReplaceKeyParts { source: 29, digest }).unwrap(),
        ),
        _ => w::AccountChange::MoveIn(
            w::AccountChangeMoveIn::new(&limits, w::AccountChangeMoveInParts { digest }).unwrap(),
        ),
    });
    let keyed = w::Keyed::new(
        &limits,
        w::KeyedParts {
            key: [5; 16],
            owner: w::Owner::Jig,
            version: 19,
            request: f::encode_ask(&ask),
            value: Some(Secret::new(Box::from(value))),
        },
    )
    .unwrap();
    let mut bytes = Writer::new(usize::try_from(keyed.measure()).unwrap());
    keyed.encode(&mut bytes).unwrap();
    let owner = exchange.send(Message::Request(bytes.finish()));
    exchange.authenticate();
    owner
}
fn roomy() -> Exchange {
    let mut env = f::env();
    env.limits.message_bytes = 150_000;
    Exchange::with_env(env)
}
#[test]
fn key_and_move_values_keep_their_own_bounds_below_the_wire_bound() {
    for (kind, bound) in [(0, 7934), (1, 7934), (2, 70000)] {
        let mut exchange = roomy();
        let bytes = skein_lib::bytes::zeroed(bound);
        carried(&mut exchange, kind, &bytes, skein_lib::sha256::digest(&bytes));
        let Some(Request::Secrets(SecretsRequest::Carry { .. })) = exchange.out.pop() else {
            panic!("value kept at its bound")
        };
        assert!(exchange.out.is_empty());
        let mut exchange = roomy();
        let bytes = skein_lib::bytes::zeroed(bound + 1);
        let owner = carried(&mut exchange, kind, &bytes, skein_lib::sha256::digest(&bytes));
        let w::Final::Refused(row) = exchange.final_answer(owner) else { panic!("limit") };
        let w::Refusal::Limit(_) = row.refusal() else { panic!("limit refusal") };
        assert!(exchange.out.is_empty(), "refused before keeping anything");
    }
}
#[test]
fn a_move_in_keeps_its_value_and_erases_a_replays_unused_value() {
    let mut exchange = roomy();
    let digest = skein_lib::sha256::digest(b"file");
    for replay in 0..2 {
        let owner = carried(&mut exchange, 2, b"file", digest);
        let Some(Request::Secrets(SecretsRequest::Carry { exchange: token, value, .. })) = exchange.out.pop() else {
            panic!("keep file")
        };
        assert_eq!(value.expose(), b"file");
        let number = (1_u64 << 63_u32) + replay;
        exchange.component.down(
            &exchange.env,
            Event::Secrets(SecretsAnswer::Carried { exchange: token, number: Some(number) }),
            &mut exchange.out,
        );
        let Some(Request::Core(event)) = exchange.out.pop() else { panic!("core key") };
        let core::Event::Key { reply_to, ask, carried, .. } = *event else { panic!("carried key") };
        assert_eq!(
            *ask,
            p::Ask::AccountChange { holder: p::AccountHolder::Own, change: p::AccountChange::MoveIn { digest } }
        );
        assert_eq!(carried, core::Carried { number, digest });
        if replay == 1 {
            exchange.component.down(
                &exchange.env,
                Event::Application(crate::FromApplication::Unused { number }),
                &mut exchange.out,
            );
            let Some(Request::Secrets(SecretsRequest::Erase { owner: SecretOwner::Carried(erased) })) =
                exchange.out.pop()
            else {
                panic!("erase unused replay file")
            };
            assert_eq!(erased, number);
        }
        exchange.reply(reply_to, p::Reply::Outcome(p::Outcome::AccountMovedIn { account: 19, renewed: false }));
        let w::Final::Done(done) = exchange.final_answer(owner) else { panic!("moved in") };
        let w::Outcome::Owner(outcome) = done.outcome() else { panic!("owner outcome") };
        let w::OwnerOutcome::AccountMovedIn(row) = outcome.value() else { panic!("moved-in outcome") };
        assert_eq!(row.account(), 19);
        assert!(!row.renewed());
        assert!(exchange.out.is_empty());
        assert!(exchange.component.pending.carried.is_empty());
        exchange.component.connections.remove(&owner);
    }
}
#[test]
fn a_move_in_digest_must_match_before_the_value_is_kept() {
    let mut exchange = roomy();
    let owner = carried(&mut exchange, 2, b"file", [0; 32]);
    assert_eq!(exchange.response(owner).0, 400);
    assert!(exchange.out.is_empty());
}

#[test]
fn written_decodes_its_move_number() {
    let ask = change(w::AccountChange::Written(
        w::AccountChangeWritten::new(&f::wire(), w::AccountChangeWrittenParts { number: 23 }).unwrap(),
    ));
    assert_eq!(
        crate::translate::asks::ask(ask, &f::models(), &f::wire()),
        Ok(p::Ask::AccountChange { holder: p::AccountHolder::Own, change: p::AccountChange::Written { number: 23 } })
    );
}
#[test]
fn each_move_refusal_keeps_its_reason() {
    for (why, expected) in [
        (p::AccountMoveWhy::NotSignedIn, w::AccountMoveWhy::NotSignedIn),
        (p::AccountMoveWhy::NotOurs, w::AccountMoveWhy::NotOurs),
        (p::AccountMoveWhy::Held, w::AccountMoveWhy::Held),
        (p::AccountMoveWhy::Cancelled, w::AccountMoveWhy::Cancelled),
        (p::AccountMoveWhy::Moving, w::AccountMoveWhy::Moving),
    ] {
        let mut exchange = Exchange::new();
        let (owner, to) = outgoing(&mut exchange);
        exchange.reply(to, p::Reply::Outcome(p::Outcome::Refused(p::Refusal::AccountMove { why })));
        let w::Final::Refused(row) = exchange.final_answer(owner) else { panic!("refused move") };
        let w::Refusal::AccountMove(value) = row.refusal() else { panic!("move refusal") };
        assert_eq!(value.why(), &expected);
    }
}
