use crate::{Limits, worst_case};

#[test]
fn turn_text_at_either_receiving_bound_is_valid() {
    let base = super::fixtures::limits();
    // Leave room between the field bound and the existing turn-byte bound.
    let text = base.text.checked_sub(1).unwrap();
    for turn_text in [text, base.turn_bytes] {
        let limits = Limits { text, turn_text, ..base };
        assert!(crate::valid(&limits), "turn_text={turn_text}");
    }
}

#[test]
fn turn_text_outside_either_receiving_bound_is_refused() {
    let base = super::fixtures::limits();
    for turn_text in [base.text.checked_sub(1).unwrap(), base.turn_bytes.checked_add(1).unwrap()] {
        let limits = Limits { turn_text, ..base };
        assert!(!crate::valid(&limits), "turn_text={turn_text}");
    }
}

#[test]
fn pending_read_memory_includes_its_family_owner() {
    use crate::{Handle, reads::PendingRead};
    use skein_lib::{Map, Token};
    let empty = Limits { reads: 0, ..super::fixtures::limits() };
    let full = super::fixtures::limits();
    let growth = worst_case(&full).unwrap().checked_sub(worst_case(&empty).unwrap()).unwrap();
    let priced = Map::<Handle, PendingRead>::worst_case(full.reads)
        .unwrap()
        .checked_sub(Map::<Handle, PendingRead>::worst_case(0).unwrap())
        .unwrap();
    let token_only = Map::<Handle, Token>::worst_case(full.reads)
        .unwrap()
        .checked_sub(Map::<Handle, Token>::worst_case(0).unwrap())
        .unwrap();
    assert_eq!(growth, priced);
    assert!(growth > token_only);
}

#[test]
fn read_output_overflow_is_refused() {
    assert!(!crate::valid(&Limits { reads: u32::MAX, ..super::fixtures::limits() }));
}

#[test]
fn valid_refuses_each_unrepresentable_limit() {
    use crate::{Backoff, valid};
    use skein_lib::{Duration, RequestLimits};
    let base = super::fixtures::limits();
    assert!(valid(&base));
    let cases = [
        Limits { turn_text: base.text - 1, ..base },
        Limits { turn_text: base.turn_bytes + 1, ..base },
        Limits { watches: 0, ..base },
        Limits { backoff: Backoff { first: Duration::ZERO, ..base.backoff }, ..base },
        Limits { backoff: Backoff { first: Duration::from_secs(1), ..base.backoff }, ..base },
        Limits { requests: RequestLimits { out: 0, ..base.requests }, ..base },
        Limits { requests: RequestLimits { first: Duration::ZERO, ..base.requests }, ..base },
        Limits { requests: RequestLimits { first: Duration::from_secs(1), ..base.requests }, ..base },
        Limits { handles: u32::MAX, ..base },
        Limits { watches: u32::MAX, ..base },
        Limits { reads: u32::MAX, ..base },
        Limits { requests: RequestLimits { out: u32::MAX, ..base.requests }, ..base },
        Limits { requests: RequestLimits { requests: u32::MAX, ..base.requests }, ..base },
        // The final drop output must fit too.
        Limits {
            reads: u32::MAX - (base.handles * 2 + base.watches + base.requests.out + base.requests.requests * 2),
            ..base
        },
    ];
    for (case, limits) in cases.into_iter().enumerate() {
        assert!(!valid(&limits), "case {case}");
    }
}

#[test]
fn validity_does_not_price_retained_payloads() {
    let base = super::fixtures::limits();
    for limits in [
        Limits { document_bytes: u64::MAX, ..base },
        Limits { requests: skein_lib::RequestLimits { bytes: u64::MAX, ..base.requests }, ..base },
    ] {
        assert!(crate::valid(&limits));
    }
}
