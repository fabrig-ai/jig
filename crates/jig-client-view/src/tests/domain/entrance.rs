use jig_client::contract::*;
use skein_lib::Wall;

#[test]
fn application_entrance_refusals_keep_sparse_facts_and_reject_variable_allocations() {
    for refusal in [
        Refusal::Role { held: Some(HeldRole::Policy { role: u32::MAX }) },
        Refusal::Authority { lacking: Lacking::Budget { needed: u64::MAX, left: 0 }, proposable: Some(false) },
        Refusal::Moved {
            now: Revision { value: u64::MAX },
            what: Moved::Hold { reason: Some(CurrentHold::Reason(HoldCategory::Uncertain { entry: u64::MAX })) },
        },
        Refusal::Ended {
            how: Ending::Cancelled { by: Actor::ProjectDeployment { project: u32::MAX }, reason: None, at: None },
        },
        Refusal::Ended {
            how: Ending::Decided {
                by: u64::MAX,
                party: None,
                how: Some(Decision::Released { again: Some(false) }),
                at: Wall::EPOCH,
            },
        },
        Refusal::Limit { which: Limit::Admission { which: AdmissionLimit::Policy, bound: None } },
        Refusal::NotWaiting,
    ] {
        let kept = EntranceRefusal::new(refusal.clone()).unwrap();
        assert_eq!(kept.refusal(), &refusal);
        assert_eq!(kept.clone().into_refusal(), refusal);
    }
    let words = || Text::new(Box::from(*b"facts"), 5).unwrap();
    for refusal in [
        Refusal::Authority {
            lacking: Lacking::Grant(Grant {
                connector: 1,
                kind: 2,
                pattern: OwnerPattern {
                    segments: skein_lib::List::with_capacity(0),
                    last: OwnerPatternEnd::Exact(words().into_bytes()),
                },
            }),
            proposable: None,
        },
        Refusal::Moved {
            now: Revision { value: 1 },
            what: Moved::Hold {
                reason: Some(CurrentHold::Document(Box::new(HoldReason::Budget { spent: 1, limit: 2 }))),
            },
        },
        Refusal::Ended { how: Ending::Failed { reason: Some(words()) } },
        Refusal::Ended { how: Ending::Cancelled { by: Actor::Party(7), reason: Some(words()), at: Some(Wall::EPOCH) } },
        Refusal::Ended { how: Ending::Withdrawn { reason: words() } },
        Refusal::Ended {
            how: Ending::Decided {
                by: 7,
                party: Some(Party { number: 7, kind: PartyKind::Person, name: words() }),
                how: None,
                at: Wall::EPOCH,
            },
        },
        Refusal::Ended {
            how: Ending::Decided {
                by: 7,
                party: None,
                how: Some(Decision::Rejected { reason: words() }),
                at: Wall::EPOCH,
            },
        },
    ] {
        assert_eq!(EntranceRefusal::new(refusal.clone()), Err(refusal));
    }
}
