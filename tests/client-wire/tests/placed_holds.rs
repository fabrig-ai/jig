//! Explicit present-at-smallest placement goldens, alongside generated optional records.
use jig_client_protocol_wire::v19 as w;
use skein_lib::{Reader, Writer};
use std::fs;

#[test]
fn task_holds_with_a_placed_line_match_their_smallest_and_full_goldens() {
    let root = skein_scratch::paths::workspace().join("crates/jig-client-protocol-wire/golden");
    let write = std::env::var_os("SKEIN_CODEGEN_WRITE").is_some_and(|value| value == "1");
    for full in [false, true] {
        let number = if full { u64::MAX } else { 0 };
        let owner = if full {
            w::HoldOwner::Task(
                w::HoldOwnerTask::new(&w::CEILINGS, w::HoldOwnerTaskParts { task: number }).expect("task owner"),
            )
        } else {
            w::HoldOwner::Party(
                w::HoldOwnerParty::new(&w::CEILINGS, w::HoldOwnerPartyParts { person: number }).expect("party owner"),
            )
        };
        let scope = if full {
            w::HoldScope::Project
        } else {
            w::HoldScope::Subtree(
                w::HoldScopeSubtree::new(&w::CEILINGS, w::HoldScopeSubtreeParts { task: number }).expect("subtree"),
            )
        };
        let placed = w::PlacedLine::new(
            &w::CEILINGS,
            w::PlacedLineParts {
                number,
                owner,
                scope,
                mode: if full { w::HoldMode::Drain } else { w::HoldMode::Stop },
                reason: if full {
                    vec![255; w::CEILINGS.placed_line_reason as usize].into_boxed_slice()
                } else {
                    Box::new([])
                },
                placed_at: number,
                revision: number,
                lifted: full,
                may_lift: full,
            },
        )
        .expect("placement");
        let hold = w::OwnerChipHold::new(
            &w::CEILINGS,
            w::OwnerChipHoldParts {
                words: None,
                circle: None,
                account: None,
                source: None,
                reason: w::HoldCategory::Placed(
                    w::HoldCategoryPlaced::new(&w::CEILINGS, w::HoldCategoryPlacedParts { hold: number })
                        .expect("placed category"),
                ),
                revision: number,
                before: w::OwnerPriorPhase::Waiting,
                escalation: None,
                tries: w::OwnerFailureTries::new(
                    &w::CEILINGS,
                    w::OwnerFailureTriesParts { transient: 0, permanent: 0, run: 0, agent: 0, lost: 0, invalid: 0 },
                )
                .expect("tries"),
                hold_wait_since: None,
                placed: Some(placed),
            },
        )
        .expect("held");
        let value = w::TaskHold::Owner(
            w::TaskHoldOwner::new(&w::CEILINGS, w::TaskHoldOwnerParts { value: hold }).expect("task hold"),
        );
        let mut out = Writer::new(value.measure() as usize);
        value.encode(&mut out).expect("task hold bytes");
        let bytes = out.finish();
        let path = root.join(if full { "task_hold_placed_full.bin" } else { "task_hold_placed_smallest.bin" });
        if write {
            fs::write(&path, &bytes).expect("write golden");
        }
        let golden = fs::read(&path)
            .expect("run SKEIN_CODEGEN_WRITE=1 heavy cargo nextest run -p jig-client-wire-world --test placed_holds");
        assert_eq!(bytes.as_ref(), golden);
        assert_eq!(w::TaskHold::decode(&w::CEILINGS, &mut Reader::new(&golden)), Ok(value));
    }
}
