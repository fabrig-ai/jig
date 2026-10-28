use super::*;

#[test]
fn both_task_hold_forms_keep_every_placed_fact_and_their_independent_revisions() {
    let limits = fixtures::limits();
    for full in [false, true] {
        let number = if full { u64::MAX } else { 0 };
        let owner = if full {
            w::HoldOwner::Task(w::HoldOwnerTask::new(&w::CEILINGS, w::HoldOwnerTaskParts { task: number }).unwrap())
        } else {
            w::HoldOwner::Party(
                w::HoldOwnerParty::new(&w::CEILINGS, w::HoldOwnerPartyParts { person: number }).unwrap(),
            )
        };
        let scope = if full {
            w::HoldScope::Project
        } else {
            w::HoldScope::Subtree(
                w::HoldScopeSubtree::new(&w::CEILINGS, w::HoldScopeSubtreeParts { task: number }).unwrap(),
            )
        };
        let placed = w::PlacedLine::new(
            &w::CEILINGS,
            w::PlacedLineParts {
                number,
                owner,
                scope,
                mode: if full { w::HoldMode::Drain } else { w::HoldMode::Stop },
                reason: if full { Box::from([255; 32]) } else { Box::new([]) },
                placed_at: number,
                revision: number,
                lifted: full,
                may_lift: full,
            },
        )
        .unwrap();
        let mut document = fixtures::read_wire_hold().into_parts();
        document.placed = Some(placed.clone());
        let document = w::Hold::new(&w::CEILINGS, document).unwrap();
        let mut owner = jig_client_test_records::hold_record().into_parts();
        owner.placed = Some(placed.clone());
        let owner = w::OwnerChipHold::new(&w::CEILINGS, owner).unwrap();
        let document = c::TaskHold::Document(Box::new(translate::documents::hold_fields(&document, &limits).unwrap()));
        let owner =
            w::TaskHold::Owner(w::TaskHoldOwner::new(&w::CEILINGS, w::TaskHoldOwnerParts { value: owner }).unwrap());
        let owner = translate::documents::task_hold_fields(&owner, &limits).unwrap();
        for hold in [document, owner] {
            let value = hold.placed().expect("placed accessor");
            assert_eq!(value.number, number);
            assert_eq!(value.revision.value, number);
            assert_eq!(value.placed_at.as_nanos(), number);
            assert_eq!((value.lifted, value.may_lift), (full, full));
            assert_eq!(value.reason.as_ref(), placed.reason());
            assert_eq!(
                value.owner,
                if full { c::HoldOwner::Task { task: number } } else { c::HoldOwner::Party { person: number } }
            );
            assert_eq!(value.mode, if full { c::HoldMode::Drain } else { c::HoldMode::Stop });
            assert_eq!(value.scope, if full { c::HoldScope::Project } else { c::HoldScope::Subtree { task: number } });
            assert_ne!(hold.revision().value, number);
        }
    }
}

use crate::translate::{asks, store};
use skein_lib::Writer;

#[test]
fn holds_restore_both_modes_scopes_and_their_whole_reason() {
    let limits = fixtures::limits();
    for mode in [c::HoldMode::Stop, c::HoldMode::Drain] {
        for scope in [c::HoldScope::Subtree { task: u64::MAX }, c::HoldScope::Project] {
            let ask = c::Ask::Hold { project: u32::MAX, mode, scope, reason: fixtures::store_text() };
            let wire = asks::ask(&ask, &limits.wire).unwrap();
            let mut out = Writer::new(usize::try_from(wire.measure()).unwrap());
            wire.encode(&mut out).unwrap();
            let decoded = w::Ask::decode(&limits.wire, &mut Reader::new(&out.finish())).unwrap();
            assert_eq!(store::restore_ask(decoded, &limits), Ok(ask));
        }
    }
    assert_eq!(limits.wire.ask_hold_reason, limits.client.text);
}

#[test]
fn a_hold_is_refused_in_a_compound_at_the_client() {
    let limits = fixtures::limits();
    for scope in [c::HoldScope::Subtree { task: 1 }, c::HoldScope::Project] {
        let ask = c::Ask::Hold { project: 1, mode: c::HoldMode::Stop, scope, reason: fixtures::store_text() };
        let wire = asks::ask(&ask, &limits.wire).unwrap();
        let mut out = Writer::new(usize::try_from(wire.measure()).unwrap());
        wire.encode(&mut out).unwrap();
        let bytes = out.finish();
        let mut pieces = Writer::new(bytes.len().checked_add(4).unwrap());
        pieces.put(&u32::try_from(bytes.len()).unwrap().to_be_bytes()).unwrap();
        pieces.put(&bytes).unwrap();
        let compound =
            w::Compound::new(&limits.wire, w::CompoundParts { project: 1, operations: pieces.finish() }).unwrap();
        store::restore_ask(w::Ask::Compound(compound), &limits).expect_err("Hold is never a compound operation");
    }
}
