use super::fixtures::{self as f, Exchange};
use crate::{Message, Request, SecretOwner, SecretsRequest};

type MessageCase = (&'static str, fn(Box<[u8]>) -> Message);
const MESSAGES: [MessageCase; 8] = [
    ("request", Message::Request),
    ("read", Message::Read),
    ("watch", Message::Watch),
    ("unwatch", Message::Unwatch),
    ("gone", Message::Gone),
    ("link", Message::SignInLink),
    ("service", Message::SignInService),
    ("sign out", Message::SignOut),
];

#[test]
fn a_message_past_the_component_bound_is_refused_before_credential_lookup() {
    for (name, message) in MESSAGES {
        let mut exchange = Exchange::new();
        let bytes = usize::try_from(f::MESSAGE_BYTES.checked_add(1).expect("past bound")).expect("fits");
        let owner = exchange.send(message(f::message_bytes(bytes)));
        assert_eq!(exchange.response(owner), (400, b"".as_slice()), "{name}");
        assert!(exchange.out.is_empty(), "{name}");
        assert!(exchange.component.pending.exchanges.is_empty(), "{name}");
    }
}

#[test]
fn a_message_at_the_component_bound_reaches_credential_lookup() {
    let mut exchange = Exchange::new();
    let bytes = usize::try_from(f::MESSAGE_BYTES).expect("fits");
    exchange.send(Message::Read(f::message_bytes(bytes)));
    let Some(Request::Secrets(SecretsRequest::Get { owner })) = exchange.out.pop() else {
        panic!("credential lookup");
    };
    assert_eq!(owner, SecretOwner::SignIn(f::NUMBER));
    assert!(exchange.out.is_empty());
}

#[test]
fn a_zero_message_bound_cannot_start_the_component() {
    let mut env = f::env();
    env.limits.message_bytes = 0;
    assert!(!crate::valid(&env.limits));
}
