use super::*;
use crate::Bounds;
use jig_client_domain::contract::*;
use skein_lib::{Duration, List};
fn pattern() -> OwnerPattern {
    let mut segments = List::with_capacity(3);
    segments.push(Box::from(b"a/b".as_slice())).unwrap();
    segments.push(Box::from(b"\xff".as_slice())).unwrap();
    OwnerPattern { segments, last: OwnerPatternEnd::Open(Box::from(b"tail".as_slice())) }
}
fn authority() -> OwnerAuthority {
    OwnerAuthority {
        models: None,
        tools: u64::MAX,
        grants: List::with_capacity(0),
        delegation: OwnerDelegation { kinds: List::with_capacity(0), tasks: u32::MAX, depth: u32::MAX },
        spend: u64::MAX,
        deadline: None,
        notes: u8::MAX,
        note_resources: List::with_capacity(0),
    }
}
fn requirement() -> PolicyRequirement {
    PolicyRequirement {
        connector: u16::MAX,
        kind: u16::MAX,
        pattern: pattern(),
        judge: PolicyJudge { connector: u16::MAX, requirement: u16::MAX, parameters: u32::MAX },
        guard: PolicyGuard::Observed { freshness: Duration::from_nanos(u64::MAX) },
        must_be_guarded: false,
    }
}
#[test]
fn policy_edits_price_whole_lists_reserved_capacity_and_every_literal_boundary() {
    let mut requirements = List::with_capacity(4);
    requirements.push(requirement()).unwrap();
    let mut grants = List::with_capacity(4);
    grants.push(OwnerGrant { connector: u16::MAX, kind: u16::MAX, pattern: pattern() }).unwrap();
    let values = [
        PolicyEdit::Ceiling { ceiling: authority() },
        PolicyEdit::Requirements { requirements },
        PolicyEdit::Projections { grants },
        PolicyEdit::Permissions { mappings: List::with_capacity(4) },
        PolicyEdit::Role(PolicyEditRole {
            number: u32::MAX,
            authority: authority(),
            period_spend: u64::MAX,
            requests: u16::MAX,
            decides: u8::MAX,
        }),
        PolicyEdit::ProjectSpend { period_spend: u64::MAX },
    ];
    for value in values {
        let cap = Bounds { rows: 4, text: 4, ..Bounds::default() };
        let price = bytes(&value, &cap).unwrap();
        assert!(fits(&value, &Bounds { document_bytes: price, ..cap }));
        assert!(!fits(&value, &Bounds { document_bytes: price - 1, ..cap }));
        assert_eq!(value, value.clone());
    }
    let value = PolicyEdit::Requirements { requirements: List::with_capacity(4) };
    assert_eq!(
        bytes(&value, &Bounds::default()),
        Some(u64::try_from(size_of::<PolicyEdit>()).unwrap() + List::<PolicyRequirement>::worst_case(4).unwrap())
    );
    assert!(!fits(&value, &Bounds { rows: 3, segments: 3, ..Bounds::default() }));
    let mut rows = List::with_capacity(1);
    rows.push(requirement()).unwrap();
    let value = PolicyEdit::Requirements { requirements: rows };
    assert!(!fits(&value, &Bounds { rows: 2, segments: 2, ..Bounds::default() }));
    assert!(!fits(&value, &Bounds { text: 3, ..Bounds::default() }));
}
#[test]
fn policy_role_prices_nested_authority_and_rejects_an_oversized_terminal_whole() {
    let mut value = authority();
    let mut grants = List::with_capacity(1);
    grants.push(OwnerGrant { connector: 1, kind: 2, pattern: pattern() }).unwrap();
    value.grants = grants;
    let value = PolicyEdit::Role(PolicyEditRole {
        number: u32::MAX,
        authority: value,
        period_spend: u64::MAX,
        requests: u16::MAX,
        decides: u8::MAX,
    });
    let cap = Bounds { rows: 3, segments: 3, text: 4, ..Bounds::default() };
    let price = bytes(&value, &cap).unwrap();
    assert!(fits(&value, &Bounds { document_bytes: price, ..cap }));
    assert!(!fits(&value, &Bounds { document_bytes: price - 1, ..cap }));
    assert!(!fits(&value, &Bounds { text: 3, ..cap }));
}
