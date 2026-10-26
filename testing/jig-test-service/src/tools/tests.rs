//! Application-owned evidence is joined to the settled outcome only at assignment.
use super::*;
use jig_domain_host as host;

const KEY: core::CallKey = core::CallKey { task: 1, attempt: 2, completion: 3, position: 4 };

fn delivery(key: core::CallKey) -> core::Settled {
    core::Settled {
        key,
        call: core::SettledCall {
            serial: 1,
            tool: b"deliver".as_slice().into(),
            answer: core::SettledAnswer::Delivery { outcome: core::DeliveryOutcome::Delivered },
        },
    }
}

#[test]
fn a_delivery_replay_uses_its_applications_evidence_by_key() {
    let other = core::CallKey { position: KEY.position + 1, ..KEY };
    let evidence = [(other, Box::from(b"another call".as_slice())), (KEY, Box::from(b"\x01receipt\x00".as_slice()))];
    let call = replay(delivery(KEY), &evidence).expect("application kept the evidence");
    assert_eq!(
        call.answer,
        host::SettledAnswer::Delivery {
            outcome: host::DeliveryOutcome::Delivered,
            evidence: b"\x01receipt\x00".as_slice().into(),
        }
    );
}

#[test]
fn a_delivery_without_application_evidence_cannot_be_assigned() {
    assert_eq!(replay(delivery(KEY), &[]), None);
}
