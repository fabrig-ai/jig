//! Field-for-field document values (client/domain/contract.md, 5–7).
use super::{
    Box, Limits, List, Reader, Wall, c, chip_fields, hold_reason_fields, owner_authority_document_fields,
    owner_grant_piece, owner_task_link_piece, piece_problem, result_value_fields, task_contract_document_fields,
    too_large, w,
};
#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn spend_line_fields(value: &w::SpendLine, _limits: &Limits) -> c::SpendLine {
    let opt0 = match value.attempt() {
        Some(item) => Some(*item),
        None => None,
    };
    c::SpendLine { task: value.task(), attempt: opt0, spent: value.spent() }
}

pub(crate) fn hold_fields(value: &w::Hold, limits: &Limits) -> Result<c::Hold, w::Problem> {
    let opt0 = match value.result() {
        Some(item) => Some(result_value_fields(item, limits)?),
        None => None,
    };
    if value.spend().len() > limits.client.rows {
        return Err(too_large());
    }
    let mut list1 = List::with_capacity(value.spend().len());
    for item in value.spend() {
        list1.push(spend_line_fields(item, limits)).expect("reserved supplied list");
    }
    Ok(c::Hold {
        placed: crate::translate::answers::placed(value.placed().as_ref(), limits)?,
        revision: crate::translate::answers::revision(value.revision(), limits),
        reason: hold_reason_fields(value.reason(), limits)?,
        before: crate::translate::facts::phase(value.before(), limits),
        since: Wall::from_nanos(value.since()),
        result: opt0,
        spend: list1,
    })
}

pub(crate) fn task_hold_fields(value: &w::TaskHold, limits: &Limits) -> Result<c::TaskHold, w::Problem> {
    Ok(match value {
        w::TaskHold::Document(row) => c::TaskHold::Document(Box::new(hold_fields(row.value(), limits)?)),
        w::TaskHold::Owner(row) => {
            c::TaskHold::Owner(Box::new(crate::translate::facts::owner_chip_hold(row.value(), limits)?))
        }
    })
}

pub(crate) fn act_kind_fields(value: &w::ActKind, _limits: &Limits) -> c::ActKind {
    match value {
        w::ActKind::Reorder => c::ActKind::Reorder,
        w::ActKind::Message => c::ActKind::Message,
        w::ActKind::Answer => c::ActKind::Answer,
        w::ActKind::Take => c::ActKind::Take,
        w::ActKind::HandBack => c::ActKind::HandBack,
        w::ActKind::Accept => c::ActKind::Accept,
        w::ActKind::Withdraw => c::ActKind::Withdraw,
        w::ActKind::Reject => c::ActKind::Reject,
        w::ActKind::PassUp => c::ActKind::PassUp,
        w::ActKind::Release => c::ActKind::Release,
        w::ActKind::LeaveHeld => c::ActKind::LeaveHeld,
        w::ActKind::Amend => c::ActKind::Amend,
        w::ActKind::Stop => c::ActKind::Stop,
        w::ActKind::Cancel => c::ActKind::Cancel,
        w::ActKind::MoveTask => c::ActKind::Move,
        w::ActKind::Seen => c::ActKind::Seen,
        w::ActKind::EditNote => c::ActKind::EditNote,
        w::ActKind::DeleteNote => c::ActKind::DeleteNote,
        w::ActKind::Adopt => c::ActKind::Adopt,
        w::ActKind::Rename => c::ActKind::Rename,
        w::ActKind::ChangePolicy => c::ActKind::ChangePolicy,
        w::ActKind::SetRoles => c::ActKind::SetRoles,
        w::ActKind::MakeService => c::ActKind::MakeService,
        w::ActKind::SetPool => c::ActKind::SetPool,
    }
}

pub(crate) fn goal_procedure_fields(value: &w::CreateProcedure, _limits: &Limits) -> c::CreateProcedure {
    c::CreateProcedure { connector: value.connector(), code: value.code() }
}

pub(crate) fn owner_kind_implication_fields(
    value: &w::OwnerKindImplication,
    _limits: &Limits,
) -> c::OwnerKindImplication {
    c::OwnerKindImplication { connector: value.connector(), kind: value.kind(), implies: value.implies() }
}

#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn bounds_fields(value: &w::Bounds, limits: &Limits) -> Result<c::Bounds, w::Problem> {
    Ok(match value {
        w::Bounds::Create(row) => {
            if row.charters().len() > limits.client.rows {
                return Err(too_large());
            }
            let mut list0 = List::with_capacity(row.charters().len());
            for item in row.charters() {
                list0.push(*item).expect("reserved supplied list");
            }
            if row.procedures().len() > limits.client.rows {
                return Err(too_large());
            }
            let mut list1 = List::with_capacity(row.procedures().len());
            for item in row.procedures() {
                list1.push(goal_procedure_fields(item, limits)).expect("reserved supplied list");
            }
            let opt2 = match row.latest() {
                Some(item) => Some(Wall::from_nanos(*item)),
                None => None,
            };
            let opt3 = match row.maximum_deadline() {
                Some(item) => Some(Wall::from_nanos(*item)),
                None => None,
            };
            c::Bounds::Create(c::CreateBounds {
                authority: Box::new(owner_authority_document_fields(row.authority(), limits)?),
                pool_left: row.pool_left(),
                charters: list0,
                procedures: list1,
                latest: opt2,
                budget: row.budget(),
                maximum_budget: row.maximum_budget(),
                maximum_deadline: opt3,
            })
        }
        w::Bounds::Amend(row) => {
            if row.implications().len() > limits.client.authority {
                return Err(too_large());
            }
            let mut list0 = List::with_capacity(row.implications().len());
            for item in row.implications() {
                list0.push(owner_kind_implication_fields(item, limits)).expect("reserved supplied list");
            }
            c::Bounds::Amend {
                budget_left: row.budget_left(),
                grants: owner_grant_piece(row.grants(), limits, limits.client.authority)?,
                implications: list0,
                authority: Box::new(owner_authority_document_fields(row.authority(), limits)?),
                ceiling: Box::new(owner_authority_document_fields(row.ceiling(), limits)?),
            }
        }
    })
}

pub(crate) fn beyond_fields(value: &w::Beyond, limits: &Limits) -> Result<c::Beyond, w::Problem> {
    Ok(match value {
        w::Beyond::Proposed(row) => c::Beyond::Proposed { to: crate::translate::facts::recipient(row.to(), limits) },
        w::Beyond::Refused(row) => c::Beyond::Refused(crate::translate::answers::refusal(row.value(), limits)?),
    })
}

pub(crate) fn admission_fields(value: &w::Admission, limits: &Limits) -> Result<c::Admission, w::Problem> {
    Ok(match value {
        w::Admission::Allowed => c::Admission::Allowed,
        w::Admission::Proposed(row) => {
            c::Admission::Proposed { to: crate::translate::facts::recipient(row.to(), limits) }
        }
        w::Admission::Bounded(row) => c::Admission::Bounded {
            within: bounds_fields(row.within(), limits)?,
            beyond: beyond_fields(row.beyond(), limits)?,
        },
        w::Admission::Refused(row) => c::Admission::Refused(crate::translate::answers::refusal(row.value(), limits)?),
    })
}

#[expect(clippy::manual_map, reason = "step code matches the optional engine effect")]
pub(crate) fn offered_fields(value: &w::Offered, limits: &Limits) -> Result<c::Offered, w::Problem> {
    Ok(c::Offered {
        effect: match value.effect() {
            Some(value) => Some(effect_fields(value, limits)),
            None => None,
        },
        act: act_kind_fields(value.act(), limits),
        admission: admission_fields(value.admission(), limits)?,
    })
}

pub(crate) fn offered_piece(bytes: &[u8], limits: &Limits, bound: u32) -> Result<List<c::Offered>, w::Problem> {
    let mut reader = Reader::new(bytes);
    let mut rows = List::with_capacity(bound);
    let mut sum = 0_u64;
    for _ in 0..=bound {
        let item = match skein_codec::piece::split(&mut reader) {
            Ok(value) => value,
            Err(reason) => return Err(piece_problem(reason)),
        };
        let Some(item) = item else {
            return Ok(rows);
        };
        sum = sum
            .checked_add(u64::try_from(item.len()).expect("piece fits u64"))
            .ok_or(too_large())?
            .checked_add(4)
            .ok_or(too_large())?;
        if sum > limits.client.document_bytes {
            return Err(too_large());
        }
        let row = w::Offered::decode(&limits.wire, &mut Reader::new(&item))?;
        let row = offered_fields(&row, limits)?;
        if rows.push(row).is_err() {
            return Err(too_large());
        }
    }
    Err(too_large())
}

pub(crate) fn ending_fields(value: &w::Ending, limits: &Limits) -> Result<c::Ending, w::Problem> {
    match crate::translate::facts::decode(value.rows(), limits)? {
        crate::translate::facts::Node::Ending(value) => Ok(*value),
        crate::translate::facts::Node::Chip(_)
        | crate::translate::facts::Node::ChipPhase(_)
        | crate::translate::facts::Node::Closing(_)
        | crate::translate::facts::Node::HeldRuntime(_)
        | crate::translate::facts::Node::HoldReason(_)
        | crate::translate::facts::Node::OwnerChipDetails(_)
        | crate::translate::facts::Node::OwnerChipEnding(_)
        | crate::translate::facts::Node::ResultValue(_) => Err(piece_problem(skein_codec::Reason::Malformed)),
    }
}

#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn result_details_fields(value: &w::ResultDetails, limits: &Limits) -> Result<c::ResultDetails, w::Problem> {
    let opt0 = match value.chosen_by() {
        Some(item) => Some(*item),
        None => None,
    };
    let opt1 = match value.followups() {
        Some(item) => Some(owner_task_link_piece(item, limits, limits.client.rows)?),
        None => None,
    };
    let opt2 = match value.proposal() {
        Some(item) => Some(crate::translate::answers::proposal_key(item, limits)),
        None => None,
    };
    Ok(c::ResultDetails {
        contract: task_contract_document_fields(value.contract(), limits)?,
        requester: crate::translate::facts::owner_requester(value.requester(), limits),
        executor: crate::translate::facts::owner_executor(value.executor(), limits),
        chosen_by: opt0,
        delegates: owner_task_link_piece(value.delegates(), limits, limits.client.rows)?,
        followups: opt1,
        proposal: opt2,
        cut: value.cut(),
    })
}

#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn result_fields(value: &w::ResultDocument, limits: &Limits) -> Result<c::Result, w::Problem> {
    let opt0 = match value.value() {
        Some(item) => Some(result_value_fields(item, limits)?),
        None => None,
    };
    let opt1 = match value.at() {
        Some(item) => Some(Wall::from_nanos(*item)),
        None => None,
    };
    let opt2 = match value.details() {
        Some(item) => Some(Box::new(result_details_fields(item, limits)?)),
        None => None,
    };
    Ok(c::Result {
        task: chip_fields(value.task(), limits)?,
        ending: ending_fields(value.ending(), limits)?,
        value: opt0,
        at: opt1,
        details: opt2,
        acts: offered_piece(value.acts(), limits, c::MAX_OFFERED_ACTS)?,
    })
}

pub(crate) fn owner_chip_ending_fields(
    value: &w::OwnerChipEnding,
    limits: &Limits,
) -> Result<c::OwnerChipEnding, w::Problem> {
    match crate::translate::facts::decode(value.rows(), limits)? {
        crate::translate::facts::Node::OwnerChipEnding(value) => Ok(*value),
        crate::translate::facts::Node::Chip(_)
        | crate::translate::facts::Node::ChipPhase(_)
        | crate::translate::facts::Node::Closing(_)
        | crate::translate::facts::Node::Ending(_)
        | crate::translate::facts::Node::HeldRuntime(_)
        | crate::translate::facts::Node::HoldReason(_)
        | crate::translate::facts::Node::OwnerChipDetails(_)
        | crate::translate::facts::Node::ResultValue(_) => Err(piece_problem(skein_codec::Reason::Malformed)),
    }
}

pub(crate) fn task_result_fields(value: &w::TaskResult, limits: &Limits) -> Result<c::TaskResult, w::Problem> {
    Ok(match value {
        w::TaskResult::Document(row) => c::TaskResult::Document(Box::new(result_fields(row.value(), limits)?)),
        w::TaskResult::Ending(row) => c::TaskResult::Ending(Box::new(owner_chip_ending_fields(row.value(), limits)?)),
    })
}

pub(crate) fn live_delegate_fields(value: &w::LiveDelegate, limits: &Limits) -> Result<c::LiveDelegate, w::Problem> {
    Ok(c::LiveDelegate { chip: chip_fields(value.chip(), limits)?, takeover_budget: value.takeover_budget() })
}

pub(crate) fn live_delegate_piece(
    bytes: &[u8],
    limits: &Limits,
    bound: u32,
) -> Result<List<c::LiveDelegate>, w::Problem> {
    let mut reader = Reader::new(bytes);
    let mut rows = List::with_capacity(bound);
    let mut sum = 0_u64;
    for _ in 0..=bound {
        let item = match skein_codec::piece::split(&mut reader) {
            Ok(value) => value,
            Err(reason) => return Err(piece_problem(reason)),
        };
        let Some(item) = item else {
            return Ok(rows);
        };
        sum = sum
            .checked_add(u64::try_from(item.len()).expect("piece fits u64"))
            .ok_or(too_large())?
            .checked_add(4)
            .ok_or(too_large())?;
        if sum > limits.client.document_bytes {
            return Err(too_large());
        }
        let row = w::LiveDelegate::decode(&limits.wire, &mut Reader::new(&item))?;
        let row = live_delegate_fields(&row, limits)?;
        if rows.push(row).is_err() {
            return Err(too_large());
        }
    }
    Err(too_large())
}

pub(crate) fn effect_fields(value: &w::Effect, limits: &Limits) -> c::Effect {
    match value {
        w::Effect::Release(value) => c::Effect::Release(match value.what() {
            w::ReleaseEffect::Judged => c::ReleaseEffect::Judged,
            w::ReleaseEffect::RunAgain => c::ReleaseEffect::RunAgain,
            w::ReleaseEffect::Uncertain => c::ReleaseEffect::Uncertain,
        }),
        w::Effect::Cancel(value) => c::Effect::Cancel(super::impact_fields(value, limits)),
        w::Effect::MoveTask(value) => c::Effect::Move { reservation: value.reservation() },
    }
}
