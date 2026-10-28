use jig_client_protocol_wire::v19 as w;
use jig_domain_core::{self as core, documents as d};
use jig_domain_people as p;
use jig_domain_tasks as t;
use jig_engine_clients_world::fixtures::{self, Peer};
use skein_lib::{Reader, Wall, Writer};

fn request() -> Box<[u8]> {
    let limits = fixtures::limits().wire;
    let ask = w::Ask::Lift(
        w::Lift::new(&limits, w::LiftParts { hold: u64::MAX, revision: u64::MAX, with_tasks: true }).expect("lift"),
    );
    let mut bytes = Writer::new(ask.measure() as usize);
    ask.encode(&mut bytes).expect("ask");
    let keyed = w::Keyed::new(
        &limits,
        w::KeyedParts { key: [9; 16], owner: w::Owner::Jig, version: 19, request: bytes.finish(), value: None },
    )
    .expect("keyed");
    let mut bytes = Writer::new(keyed.measure() as usize);
    keyed.encode(&mut bytes).expect("keyed bytes");
    bytes.finish()
}

fn answer(reply: p::Reply) -> w::Final {
    let mut peer = Peer::started(&fixtures::settings());
    let bearer = fixtures::signed_in(&mut peer);
    let socket = peer.post(b"/jig/v1/request", request(), Some(&bearer));
    let Some(core::Event::People(p::Event::Ask { reply_to, ask, .. })) = peer.take_core() else {
        panic!("native lift")
    };
    assert_eq!(ask, p::Ask::Lift { hold: u64::MAX, revision: u64::MAX, with_tasks: true });
    peer.answer_request(reply_to, reply);
    let limits = fixtures::limits().wire;
    let w::KeyedAnswer::FinalAnswer(record) =
        w::KeyedAnswer::decode(&limits, &mut Reader::new(peer.answered(socket).expect("durable reply")))
            .expect("envelope")
    else {
        panic!("final")
    };
    w::Final::decode(&limits, &mut Reader::new(record.record())).expect("native answer")
}

#[test]
fn lift_and_lifted_cross_the_native_boundary_without_losing_the_count() {
    let w::Final::Done(done) = answer(p::Reply::Outcome(p::Outcome::Lifted { hold: u64::MAX, released: u32::MAX }))
    else {
        panic!("done")
    };
    let w::Outcome::Owner(outcome) = done.outcome() else { panic!("owner") };
    let w::OwnerOutcome::Lifted(value) = outcome.value() else { panic!("lifted") };
    assert_eq!((value.hold(), value.released()), (u64::MAX, u32::MAX));
}

#[test]
fn both_hold_owners_and_the_independent_moved_revision_cross_the_native_boundary() {
    for owner in [p::HoldOwner::Party { person: u64::MAX }, p::HoldOwner::Task { task: u64::MAX }] {
        let w::Final::Refused(refused) = answer(p::Reply::Outcome(p::Outcome::Refused(p::Refusal::Authority {
            lacking: p::Lack::HoldOwner { hold: u64::MAX, owner },
            proposable: None,
        }))) else {
            panic!("refused")
        };
        let w::Refusal::Authority(value) = refused.refusal() else { panic!("authority") };
        assert_eq!(value.proposable(), &None);
        let w::Lacking::HoldOwner(value) = value.lacking() else { panic!("hold owner") };
        assert_eq!(value.hold(), u64::MAX);
        match (owner, value.owner()) {
            (p::HoldOwner::Party { person }, w::HoldOwner::Party(value)) => assert_eq!(value.person(), person),
            (p::HoldOwner::Task { task }, w::HoldOwner::Task(value)) => assert_eq!(value.task(), task),
            _ => panic!("owner kind preserved"),
        }
    }
    let w::Final::Refused(refused) = answer(p::Reply::Outcome(p::Outcome::Refused(p::Refusal::Moved {
        now: p::Moved::PlacedHold { revision: u64::MAX },
    }))) else {
        panic!("refused")
    };
    let w::Refusal::Moved(value) = refused.refusal() else { panic!("moved") };
    assert_eq!(value.now().value(), u64::MAX);
    let w::Moved::PlacedHold(value) = value.what() else { panic!("placed revision") };
    assert_eq!(value.revision(), u64::MAX);
}

#[test]
fn placed_hold_documents_preserve_the_smallest_and_full_owner_facts() {
    for full in [false, true] {
        let number = if full { u64::MAX } else { 0 };
        let placed = d::PlacedLine {
            number,
            owner: if full { p::HoldOwner::Task { task: number } } else { p::HoldOwner::Party { person: number } },
            mode: if full { t::Mode::Drain } else { t::Mode::Stop },
            scope: if full { t::Scope::Project } else { t::Scope::Subtree { task: number } },
            reason: if full { Box::from([255; 32]) } else { Box::new([]) },
            placed_at: Wall::from_nanos(number),
            revision: number,
            lifted: full,
            may_lift: full,
        };
        let line = d::HoldLine {
            placed: Some(placed.clone()),
            circle: None,
            account: None,
            source: None,
            reason: t::Hold::Placed { hold: number },
            revision: number,
            before: d::PriorPhase::Waiting,
            escalation: None,
            tries: t::Tries::NONE,
            hold_wait_since: None,
        };
        let encoded =
            jig_protocol_engine_clients::translate::chips::held(&line, &w::CEILINGS).expect("document translation");
        let mut bytes = Writer::new(encoded.measure() as usize);
        encoded.encode(&mut bytes).expect("hold bytes");
        let decoded = w::OwnerChipHold::decode(&w::CEILINGS, &mut Reader::new(&bytes.finish())).expect("decode");
        assert_eq!(decoded, encoded);
        let value = decoded.placed().as_ref().expect("placed facts");
        assert_eq!(
            (value.number(), value.revision(), value.placed_at(), value.lifted(), value.may_lift()),
            (number, number, number, full, full)
        );
        assert_eq!(value.reason(), placed.reason.as_ref());
        assert!(matches!(
            (value.mode(), value.scope()),
            (w::HoldMode::Stop, w::HoldScope::Subtree(_)) | (w::HoldMode::Drain, w::HoldScope::Project)
        ));
    }
}

#[test]
fn a_lift_capacity_diagnostic_keeps_the_hold_identity() {
    let value = p::CapacityRefusal {
        key: p::RequestKey { person: u64::MAX, key: [255; 16] },
        project: None,
        action: p::CapacityAction::Lift { hold: u64::MAX },
        at: Wall::EPOCH,
        cause: p::CapacityCause::Requests { limit: u32::MAX },
    };
    let record =
        jig_protocol_engine_clients::translate::deployment::person_refusal(&value, &w::CEILINGS).expect("diagnostic");
    let w::OwnerPersonCapacityAction::Lift(action) = record.action() else { panic!("lift action") };
    assert_eq!(action.hold(), u64::MAX);
    assert_eq!(record.project(), &None);
}

#[test]
fn hold_and_placed_cross_the_native_boundary() {
    for mode in [w::HoldMode::Stop, w::HoldMode::Drain] {
        for project_scope in [false, true] {
            let limits = fixtures::limits().wire;
            let reason = b"the complete reason, including \xe2\x98\x83";
            let scope = if project_scope {
                w::HoldScope::Project
            } else {
                w::HoldScope::Subtree(
                    w::HoldScopeSubtree::new(&limits, w::HoldScopeSubtreeParts { task: u64::MAX }).expect("scope"),
                )
            };
            let ask = w::Ask::Hold(
                w::AskHold::new(
                    &limits,
                    w::AskHoldParts { project: u32::MAX, mode: mode.clone(), scope, reason: reason.as_slice().into() },
                )
                .expect("Hold"),
            );
            let mut bytes = Writer::new(ask.measure() as usize);
            ask.encode(&mut bytes).expect("Hold encoding");
            let keyed = w::Keyed::new(
                &limits,
                w::KeyedParts {
                    key: [19; 16],
                    owner: w::Owner::Jig,
                    version: 19,
                    request: bytes.finish(),
                    value: None,
                },
            )
            .expect("keyed Hold");
            let mut bytes = Writer::new(keyed.measure() as usize);
            keyed.encode(&mut bytes).expect("keyed bytes");
            let mut peer = Peer::started(&fixtures::settings());
            let bearer = fixtures::signed_in(&mut peer);
            let socket = peer.post(b"/jig/v1/request", bytes.finish(), Some(&bearer));
            let Some(core::Event::People(p::Event::Ask { reply_to, ask, .. })) = peer.take_core() else {
                panic!("native Hold")
            };
            assert_eq!(
                ask,
                p::Ask::Hold {
                    project: u32::MAX,
                    mode: match mode {
                        w::HoldMode::Stop => p::HoldMode::Stop,
                        w::HoldMode::Drain => p::HoldMode::Drain,
                    },
                    scope: if project_scope { p::HoldScope::Project } else { p::HoldScope::Subtree { task: u64::MAX } },
                    reason: reason.as_slice().into(),
                }
            );
            peer.answer_request(reply_to, p::Reply::Outcome(p::Outcome::Placed { hold: u64::MAX, revision: u64::MAX }));
            let w::KeyedAnswer::FinalAnswer(record) =
                w::KeyedAnswer::decode(&limits, &mut Reader::new(peer.answered(socket).expect("durable Placed")))
                    .expect("envelope")
            else {
                panic!("final")
            };
            let w::Final::Done(done) = w::Final::decode(&limits, &mut Reader::new(record.record())).expect("answer")
            else {
                panic!("done")
            };
            let w::Outcome::Owner(owner) = done.outcome() else { panic!("owner outcome") };
            let w::OwnerOutcome::Placed(placed) = owner.value() else { panic!("Placed") };
            assert_eq!((placed.hold(), placed.revision().value()), (u64::MAX, u64::MAX));
        }
    }
}

#[test]
fn a_hold_capacity_diagnostic_carries_only_its_requested_project() {
    let value = p::CapacityRefusal {
        key: p::RequestKey { person: u64::MAX, key: [255; 16] },
        project: Some(u32::MAX),
        action: p::CapacityAction::Hold,
        at: Wall::EPOCH,
        cause: p::CapacityCause::Requests { limit: u32::MAX },
    };
    let record =
        jig_protocol_engine_clients::translate::deployment::person_refusal(&value, &w::CEILINGS).expect("diagnostic");
    assert_eq!(record.action(), &w::OwnerPersonCapacityAction::Hold);
    assert_eq!(record.project(), &Some(u32::MAX));
}
