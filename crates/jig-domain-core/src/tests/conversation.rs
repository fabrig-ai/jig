//! A settled record owns tool and answer bytes; its live address is transient (store.md, 5.1).
use super::*;

fn settled(answer: SettledAnswer) -> SettledCall {
    SettledCall { serial: 0, tool: b"tool".as_slice().into(), answer }
}

#[test]
fn a_settled_calls_owned_bytes_are_tool_and_answer_alone() {
    // Four bytes of tool; a delivery has no owned answer payload.
    for (answer, owned) in [
        (SettledAnswer::Host { error: false, body: b"answer".as_slice().into() }, 10),
        (SettledAnswer::Delivery { outcome: DeliveryOutcome::Delivered }, 4),
    ] {
        assert_eq!(settled(answer).owned_bytes(), Some(owned));
    }
}

#[test]
fn a_settlement_refusal_preserves_its_opaque_live_reply_address() {
    let (mut core, limits) = make_tree();
    let key = CallKey { task: 1, attempt: 2, completion: 3, position: 4 };
    let to = ReplyTo::new(Token::new(5));
    let Requests::Out(mut out) = crate::conversation::settle(
        &mut core,
        &limits,
        to,
        key,
        b"opaque address".as_slice().into(),
        settled(SettledAnswer::Host { error: false, body: b"answer".as_slice().into() }),
    );
    let Some(Request::Now(refused)) = out.pop() else { panic!("settlement refusal") };
    let Now::SettledCallRefused { to: reply, key: refused_key, name, tool } = *refused else {
        panic!("settlement refusal address")
    };
    assert_eq!(
        (reply.into_token(), refused_key, name.as_ref(), tool.as_ref()),
        (Token::new(5), key, b"opaque address".as_slice(), b"tool".as_slice())
    );
    let Some(Request::Decided) = out.pop() else { panic!("the decision closes") };
    assert!(out.is_empty());
}

#[test]
fn a_delivery_accepts_its_tools_bytes_at_the_limit() {
    let mut limits = limits();
    // The fixture's tool is four bytes; the delivery owns no payload.
    limits.call_answer_bytes = 4;
    assert!(settled(SettledAnswer::Delivery { outcome: DeliveryOutcome::Delivered }).valid(&limits));
}

#[test]
fn a_delivery_refuses_a_tool_one_byte_past_the_limit() {
    let mut limits = limits();
    limits.call_answer_bytes = 3;
    assert!(!settled(SettledAnswer::Delivery { outcome: DeliveryOutcome::Delivered }).valid(&limits));
}
