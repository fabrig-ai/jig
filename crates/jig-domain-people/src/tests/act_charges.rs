#![expect(clippy::disallowed_macros, reason = "step tests are ordinary Rust (testing-strategy.md, section 4)")]
use super::{valid, valid_outcome};
use crate::tests::LIMITS;
use crate::*;

#[test]
fn largest_acts_and_kept_refusals_are_valid() {
    // Each row makes a different act the largest; the 64-task Kept refusal is
    // the largest retained outcome at one configured host (people.md, 5.3).
    const REFUSAL_TASKS: usize = 64;
    let name_bytes = u32::try_from(NAME_BYTES).unwrap();
    let long_reason = name_bytes * 2;
    let short_adoption = u32::try_from(size_of::<Box<[u8]>>() * 2).unwrap();
    for (words, amendment_bytes) in
        [(long_reason, short_adoption), (LIMITS.words, LIMITS.amendment_bytes), (LIMITS.words, short_adoption)]
    {
        let limits = Limits { program_hosts: 1, words, amendment_bytes, ..LIMITS };
        let adopt = Act::Adopt {
            project: 1,
            adoption: Adoption {
                resource: ResourceName {
                    connector: 1,
                    path: Box::new(
                        [vec![0; usize::try_from(amendment_bytes).unwrap() - size_of::<Box<[u8]>>()].into()],
                    ),
                },
                role: ResourceRole::Owned,
                options: Box::new([]),
            },
        };
        let close =
            Act::Close { project: 1, task: 1, grace: None, reason: vec![0; usize::try_from(words).unwrap()].into() };
        let project = Act::MakeProject { maker: 1, name: "a".repeat(NAME_BYTES).into(), policy: 1 };
        let outcome = Outcome::Refused(Refusal::Kept { pending: vec![1; REFUSAL_TASKS].into() });
        for act in [adopt, close, project] {
            valid(&limits, &act).unwrap();
            assert!(valid_outcome(&limits, &act, &outcome));
        }
    }
}

#[test]
#[should_panic(expected = "configured act grants fit")]
fn a_grant_past_the_configured_bound_is_rejected() {
    drop(Domain::new(
        &Limits { act_grants: 0, ..LIMITS },
        Box::new([]),
        1,
        Box::new([ActGrant { connector: 1, may: ActKind::ALL.into() }]),
    ));
}

#[test]
fn a_grant_at_the_configured_bound_fits() {
    drop(Domain::new(
        &Limits { act_grants: 1, ..LIMITS },
        Box::new([]),
        1,
        Box::new([ActGrant { connector: 1, may: ActKind::ALL.into() }]),
    ));
}

#[test]
fn grant_count_is_bounded_by_connector_numbers() {
    const CONNECTOR_NUMBERS: u32 = 1_u32 << u16::BITS;
    for (act_grants, fits) in [(CONNECTOR_NUMBERS, true), (CONNECTOR_NUMBERS + 1, false)] {
        assert_eq!(crate::valid(&Limits { act_grants, ..LIMITS }), fits);
    }
}
