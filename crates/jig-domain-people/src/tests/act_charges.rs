#![expect(
    clippy::disallowed_macros,
    clippy::disallowed_methods,
    reason = "step tests are ordinary Rust (testing-strategy.md, section 4)"
)]
use super::{Acted, valid, valid_outcome};
use crate::tests::LIMITS;
use crate::*;
use skein_lib::{Time, Wall};

#[test]
fn retained_act_owned_bytes_fit_a_smaller_charge() {
    // Each row makes a different act the largest; the 64-task Kept refusal is
    // the largest retained outcome at one configured host (people.md, 5.3).
    const REFUSAL_TASKS: usize = 64;
    let name_bytes = u32::try_from(NAME_BYTES).unwrap();
    let long_reason = name_bytes * 2;
    let short_adoption = u32::try_from(size_of::<Box<[u8]>>() * 2).unwrap();
    for (words, amendment_bytes, expected) in [
        (long_reason, short_adoption, long_reason),
        (LIMITS.words, LIMITS.amendment_bytes, LIMITS.amendment_bytes),
        (LIMITS.words, short_adoption, name_bytes),
    ] {
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
        let mut largest = 0;
        for act in [adopt, close, project] {
            valid(&limits, &act).unwrap();
            assert!(valid_outcome(&limits, &act, &outcome));
            let retained = Acted { act, outcome: outcome.clone(), at: Wall::EPOCH, due: Time::ZERO };
            let bytes = match &retained.act {
                Act::Adopt { adoption, .. } => {
                    adoption.resource.path.len() * size_of::<Box<[u8]>>()
                        + adoption.resource.path.iter().map(|part| part.len()).sum::<usize>()
                }
                Act::Close { reason, .. } => reason.len(),
                Act::MakeProject { name, .. } => name.len(),
                Act::AddKey { .. } | Act::RemoveAccount { .. } => 0,
            };
            let Outcome::Refused(Refusal::Kept { pending }) = &retained.outcome else { panic!("retained refusal") };
            let owned = bytes + pending.len() * size_of::<u64>();
            let charged = limits::act_bytes(&limits).unwrap() + limits::act_outcome_bytes(&limits).unwrap();
            assert!(u64::try_from(owned).unwrap() <= charged);
            largest = largest.max(bytes);
        }
        assert_eq!(largest, usize::try_from(expected).unwrap());
        assert_eq!(limits::act_bytes(&limits), Some(u64::from(expected)));
        assert_eq!(limits::act_outcome_bytes(&limits), Some(u64::try_from(REFUSAL_TASKS * size_of::<u64>()).unwrap()));
        let old_payload = limits::ask_bytes(&limits).unwrap() + outcome_bytes(&limits).unwrap();
        let new_payload = limits::act_bytes(&limits).unwrap() + limits::act_outcome_bytes(&limits).unwrap();
        assert!(new_payload < old_payload);
    }
}

#[test]
fn many_hosts_make_act_and_party_outcome_bounds_equal() {
    assert_eq!(limits::act_outcome_bytes(&LIMITS), outcome_bytes(&LIMITS));
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
