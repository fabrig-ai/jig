use super::*;
pub fn hold_refusal(value: w::HoldCategory) -> w::Refusal {
    let reason = Some(w::CurrentHold::Reason(
        w::CurrentHoldReason::new(&wire(), w::CurrentHoldReasonParts { value }).expect("reason"),
    ));
    let what = w::Moved::Hold(w::MovedHold::new(&wire(), w::MovedHoldParts { reason }).expect("hold"));
    w::Refusal::Moved(w::RefusalMoved::new(&wire(), w::RefusalMovedParts { now: revision(31), what }).expect("moved"))
}
pub fn holds() -> [(p::HoldReason, w::HoldCategory); 21] {
    let limits = wire();
    [
        (p::HoldReason::Stalled, w::HoldCategory::Stalled),
        (p::HoldReason::Stopped, w::HoldCategory::Stopped),
        (p::HoldReason::Drift, w::HoldCategory::Drift),
        (p::HoldReason::Effects, w::HoldCategory::Effects),
        (p::HoldReason::EffectFailed, w::HoldCategory::EffectFailed),
        (p::HoldReason::Budget, w::HoldCategory::Budget),
        (p::HoldReason::Deadline, w::HoldCategory::Deadline),
        (p::HoldReason::HoldsWaited, w::HoldCategory::HoldsWaited),
        (p::HoldReason::ResourceUnavailable, w::HoldCategory::ResourceUnavailable),
        (p::HoldReason::ResourceAccess, w::HoldCategory::ResourceAccess),
        (p::HoldReason::Procedure, w::HoldCategory::Procedure),
        (
            p::HoldReason::Dependency { task: 9 },
            w::HoldCategory::Dependency(
                w::HoldCategoryDependency::new(&limits, w::HoldCategoryDependencyParts { task: 9 })
                    .expect("Dependency"),
            ),
        ),
        (
            p::HoldReason::Placed { hold: 9 },
            w::HoldCategory::Placed(
                w::HoldCategoryPlaced::new(&limits, w::HoldCategoryPlacedParts { hold: 9 }).expect("Placed"),
            ),
        ),
        (
            p::HoldReason::Uncertain { entry: 9 },
            w::HoldCategory::Uncertain(
                w::HoldCategoryUncertain::new(&limits, w::HoldCategoryUncertainParts { entry: 9 }).expect("Uncertain"),
            ),
        ),
        (
            p::HoldReason::PoolLost { pool: 9 },
            w::HoldCategory::PoolLost(
                w::HoldCategoryPoolLost::new(&limits, w::HoldCategoryPoolLostParts { pool: 9 }).expect("PoolLost"),
            ),
        ),
        (
            p::HoldReason::Failures { class: p::FailureClass::Transient },
            w::HoldCategory::Failures(
                w::HoldCategoryFailures::new(
                    &limits,
                    w::HoldCategoryFailuresParts { class: w::HoldFailure::Transient },
                )
                .expect("failure"),
            ),
        ),
        (
            p::HoldReason::Failures { class: p::FailureClass::Permanent },
            w::HoldCategory::Failures(
                w::HoldCategoryFailures::new(
                    &limits,
                    w::HoldCategoryFailuresParts { class: w::HoldFailure::Permanent },
                )
                .expect("failure"),
            ),
        ),
        (
            p::HoldReason::Failures { class: p::FailureClass::Run },
            w::HoldCategory::Failures(
                w::HoldCategoryFailures::new(&limits, w::HoldCategoryFailuresParts { class: w::HoldFailure::Run })
                    .expect("failure"),
            ),
        ),
        (
            p::HoldReason::Failures { class: p::FailureClass::Agent },
            w::HoldCategory::Failures(
                w::HoldCategoryFailures::new(&limits, w::HoldCategoryFailuresParts { class: w::HoldFailure::Agent })
                    .expect("failure"),
            ),
        ),
        (
            p::HoldReason::Failures { class: p::FailureClass::Lost },
            w::HoldCategory::Failures(
                w::HoldCategoryFailures::new(&limits, w::HoldCategoryFailuresParts { class: w::HoldFailure::Lost })
                    .expect("failure"),
            ),
        ),
        (
            p::HoldReason::Failures { class: p::FailureClass::Invalid },
            w::HoldCategory::Failures(
                w::HoldCategoryFailures::new(&limits, w::HoldCategoryFailuresParts { class: w::HoldFailure::Invalid })
                    .expect("failure"),
            ),
        ),
    ]
}
pub fn refusals() -> [(p::Refusal, w::Refusal); 9] {
    let limits = wire();
    [
        (p::Refusal::NotWaiting, w::Refusal::NotWaiting),
        (p::Refusal::NoFurther, w::Refusal::NoFurther),
        (p::Refusal::Unknown, w::Refusal::Unknown),
        (p::Refusal::NotOffered, w::Refusal::NotOffered),
        (p::Refusal::KeyConflict, w::Refusal::KeyConflict),
        (p::Refusal::NoEscalationHolder, w::Refusal::NoEscalationHolder),
        (
            p::Refusal::Taken { by: 7 },
            w::Refusal::Taken(w::RefusalTaken::new(&limits, w::RefusalTakenParts { by: 7 }).expect("taken")),
        ),
        (
            p::Refusal::Role { held: None },
            w::Refusal::Role(w::RefusalRole::new(&limits, w::RefusalRoleParts { held: None }).expect("role")),
        ),
        (
            p::Refusal::Role { held: Some(p::Role::Policy { role: 99 }) },
            w::Refusal::Role(
                w::RefusalRole::new(
                    &limits,
                    w::RefusalRoleParts {
                        held: Some(w::HeldRole::Policy(
                            w::HeldRolePolicy::new(&limits, w::HeldRolePolicyParts { role: 99 }).expect("policy"),
                        )),
                    },
                )
                .expect("role"),
            ),
        ),
    ]
}
pub fn ending_refusal(value: w::EndingFields) -> w::Refusal {
    let limits = wire();
    let row =
        w::Fact::Ending(w::EndingRow::new(&limits, w::EndingRowParts { parent: None, value }).expect("ending row"));
    let mut out = Writer::new(usize::try_from(row.measure().checked_add(4).expect("length")).expect("small row"));
    out.put(&row.measure().to_be_bytes()).expect("row length");
    row.encode(&mut out).expect("row");
    let how = w::Ending::new(&limits, w::EndingParts { rows: out.finish() }).expect("ending");
    w::Refusal::Ended(w::RefusalEnded::new(&limits, w::RefusalEndedParts { how }).expect("ended"))
}
pub fn endings() -> [(p::Ending, w::Refusal); 7] {
    let limits = wire();
    [
        (p::Ending::Done, ending_refusal(w::EndingFields::Done)),
        (
            p::Ending::Failed,
            ending_refusal(w::EndingFields::Failed(
                w::EndingFailedFields::new(&limits, w::EndingFailedFieldsParts { reason: None }).expect("failed"),
            )),
        ),
        (p::Ending::Deleted, ending_refusal(w::EndingFields::Deleted)),
        (
            p::Ending::Decided { by: 7, at: Wall::from_nanos(13) },
            ending_refusal(w::EndingFields::Decided(
                w::EndingDecidedFields::new(
                    &limits,
                    w::EndingDecidedFieldsParts { by: 7, party: None, how: None, at: 13 },
                )
                .expect("decided"),
            )),
        ),
        (
            p::Ending::Cancelled { by: p::Party::Person(7), at: Wall::from_nanos(13) },
            ending_refusal(w::EndingFields::Cancelled(
                w::EndingCancelledFields::new(
                    &limits,
                    w::EndingCancelledFieldsParts {
                        by: w::Actor::Party(
                            w::ActorParty::new(&limits, w::ActorPartyParts { value: 7 }).expect("actor"),
                        ),
                        reason: None,
                        at: Some(13),
                    },
                )
                .expect("cancelled"),
            )),
        ),
        (
            p::Ending::Cancelled { by: p::Party::Task(8), at: Wall::from_nanos(13) },
            ending_refusal(w::EndingFields::Cancelled(
                w::EndingCancelledFields::new(
                    &limits,
                    w::EndingCancelledFieldsParts {
                        by: w::Actor::Task(w::ActorTask::new(&limits, w::ActorTaskParts { value: 8 }).expect("actor")),
                        reason: None,
                        at: Some(13),
                    },
                )
                .expect("cancelled"),
            )),
        ),
        (
            p::Ending::Cancelled { by: p::Party::Deployment { project: 3 }, at: Wall::from_nanos(13) },
            ending_refusal(w::EndingFields::Cancelled(
                w::EndingCancelledFields::new(
                    &limits,
                    w::EndingCancelledFieldsParts {
                        by: w::Actor::ProjectDeployment(
                            w::ActorProjectDeployment::new(&limits, w::ActorProjectDeploymentParts { project: 3 })
                                .expect("actor"),
                        ),
                        reason: None,
                        at: Some(13),
                    },
                )
                .expect("cancelled"),
            )),
        ),
    ]
}
pub fn extra_refusals() -> [(p::Refusal, w::Refusal); 7] {
    let limits = wire();
    [
        (
            p::Refusal::Role { held: Some(p::Role::Owner) },
            w::Refusal::Role(
                w::RefusalRole::new(&limits, w::RefusalRoleParts { held: Some(w::HeldRole::Owner) }).expect("role"),
            ),
        ),
        (
            p::Refusal::Role { held: Some(p::Role::Maintainer) },
            w::Refusal::Role(
                w::RefusalRole::new(&limits, w::RefusalRoleParts { held: Some(w::HeldRole::Maintainer) })
                    .expect("role"),
            ),
        ),
        (
            p::Refusal::Role { held: Some(p::Role::Member) },
            w::Refusal::Role(
                w::RefusalRole::new(&limits, w::RefusalRoleParts { held: Some(w::HeldRole::Member) }).expect("role"),
            ),
        ),
        (
            p::Refusal::Role { held: Some(p::Role::Observer) },
            w::Refusal::Role(
                w::RefusalRole::new(&limits, w::RefusalRoleParts { held: Some(w::HeldRole::Observer) }).expect("role"),
            ),
        ),
        (
            p::Refusal::Moved { now: p::Moved::Spec { revision: 31 } },
            w::Refusal::Moved(
                w::RefusalMoved::new(&limits, w::RefusalMovedParts { now: revision(31), what: w::Moved::Spec })
                    .expect("moved"),
            ),
        ),
        (
            p::Refusal::Moved { now: p::Moved::Person { revision: 31 } },
            w::Refusal::Moved(
                w::RefusalMoved::new(&limits, w::RefusalMovedParts { now: revision(31), what: w::Moved::PersonTask })
                    .expect("moved"),
            ),
        ),
        (
            p::Refusal::Moved { now: p::Moved::Note { revision: 31 } },
            w::Refusal::Moved(
                w::RefusalMoved::new(&limits, w::RefusalMovedParts { now: revision(31), what: w::Moved::Note })
                    .expect("moved"),
            ),
        ),
    ]
}
