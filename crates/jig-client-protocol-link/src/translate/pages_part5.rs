//! Whole pages and single documents (protocol/client.md, 3 and 6).
use super::{
    Box, Limits, List, Reader, Wall, address_change_fields, budget_fields, c, cursor_fields, ending_fields,
    offered_piece, owner_amendment_state_fields, owner_authority_document_fields, owner_authority_fields,
    owner_history_holder_fields, owner_period_times_fields, owner_task_link_fields, passed_piece, piece_problem,
    proposed_task_piece, text, too_large, w,
};
#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn owner_funding_record_fields(value: &w::OwnerFundingRecord, limits: &Limits) -> c::OwnerFundingRecord {
    let opt0 = match value.period() {
        Some(item) => Some(owner_period_times_fields(item, limits)),
        None => None,
    };
    let opt1 = match value.parent() {
        Some(item) => Some(crate::translate::facts::funder(item, limits)),
        None => None,
    };
    c::OwnerFundingRecord {
        period: opt0,
        funder: crate::translate::facts::funder(value.funder(), limits),
        parent: opt1,
        numbers: crate::translate::facts::owner_numbers(value.numbers(), limits),
        made: value.made(),
        closed: value.closed(),
    }
}

pub(crate) fn owner_spend_page_fields(
    value: &w::OwnerSpendPage,
    limits: &Limits,
) -> Result<c::OwnerSpendPage, w::Problem> {
    if value.rows().len() > limits.client.rows {
        return Err(too_large());
    }
    let mut list0 = List::with_capacity(value.rows().len());
    for item in value.rows() {
        list0.push(owner_funding_record_fields(item, limits)).expect("reserved supplied list");
    }
    let opt1 = match value.before() {
        Some(item) => Some(cursor_fields(item, limits)?),
        None => None,
    };
    let opt2 = match value.after() {
        Some(item) => Some(cursor_fields(item, limits)?),
        None => None,
    };
    Ok(c::OwnerSpendPage { rows: list0, before: opt1, after: opt2, cut: value.cut() })
}

pub(crate) fn grant_piece(bytes: &[u8], limits: &Limits, bound: u32) -> Result<List<c::Grant>, w::Problem> {
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
        let row = w::Grant::decode(&limits.wire, &mut Reader::new(&item))?;
        let row = crate::translate::answers::grant(&row, limits);
        if rows.push(row).is_err() {
            return Err(too_large());
        }
    }
    Err(too_large())
}

pub(crate) fn note_scope_piece(bytes: &[u8], limits: &Limits, bound: u32) -> Result<List<c::NoteScope>, w::Problem> {
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
        let row = w::NoteScope::decode(&limits.wire, &mut Reader::new(&item))?;
        let row = crate::translate::answers::note_scope(&row, limits);
        if rows.push(row).is_err() {
            return Err(too_large());
        }
    }
    Err(too_large())
}

#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn authority_fields(value: &w::Authority, limits: &Limits) -> Result<c::Authority, w::Problem> {
    if value.task_kinds().len() > limits.client.authority {
        return Err(too_large());
    }
    let mut list0 = List::with_capacity(value.task_kinds().len());
    for item in value.task_kinds() {
        list0.push(crate::translate::answers::executor(item, limits)).expect("reserved supplied list");
    }
    let opt1 = match value.deadline() {
        Some(item) => Some(Wall::from_nanos(*item)),
        None => None,
    };
    if value.tools().len() > limits.client.rows {
        return Err(too_large());
    }
    let mut list2 = List::with_capacity(value.tools().len());
    for item in value.tools() {
        list2.push(*item).expect("reserved supplied list");
    }
    Ok(c::Authority {
        models: crate::translate::model_sets::decode(value.models().as_ref())?,
        budget: value.budget(),
        grants: grant_piece(value.grants(), limits, limits.client.authority)?,
        task_kinds: list0,
        depth: value.depth(),
        tasks: value.tasks(),
        deadline: opt1,
        tools: list2,
        note_scopes: note_scope_piece(value.note_scopes(), limits, limits.client.authority)?,
    })
}

pub(crate) fn parameter_fields(value: &w::Parameter, limits: &Limits) -> Result<c::Parameter, w::Problem> {
    Ok(match value {
        w::Parameter::Number(row) => c::Parameter::Number { name: row.name(), value: row.value() },
        w::Parameter::Bytes(row) => c::Parameter::Bytes { name: row.name(), value: text(row.value(), limits)? },
        w::Parameter::Resource(row) => {
            c::Parameter::Resource { name: row.name(), connector: row.connector(), resource: row.resource() }
        }
    })
}

pub(crate) fn specification_fields(value: &w::Specification, limits: &Limits) -> Result<c::Specification, w::Problem> {
    if value.parameters().len() > limits.client.rows {
        return Err(too_large());
    }
    let mut list0 = List::with_capacity(value.parameters().len());
    for item in value.parameters() {
        list0.push(parameter_fields(item, limits)?).expect("reserved supplied list");
    }
    if value.inputs().len() > limits.client.rows {
        return Err(too_large());
    }
    let mut list1 = List::with_capacity(value.inputs().len());
    for item in value.inputs() {
        list1.push(*item).expect("reserved supplied list");
    }
    Ok(c::Specification { asks: value.asks(), words: text(value.words(), limits)?, parameters: list0, inputs: list1 })
}

#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn amendment_fields(value: &w::Amendment, limits: &Limits) -> Result<c::Amendment, w::Problem> {
    let opt0 = match value.addressed() {
        Some(item) => Some(address_change_fields(item, limits)),
        None => None,
    };
    let opt1 = match value.spec() {
        Some(item) => Some(specification_fields(item, limits)?),
        None => None,
    };
    let opt2 = match value.wake() {
        Some(item) => Some(crate::translate::facts::owner_wake_policy(item, limits)),
        None => None,
    };
    let opt3 = match value.dependencies() {
        Some(item) => {
            if (item).len() > limits.client.rows {
                return Err(too_large());
            }
            let mut list0 = List::with_capacity((item).len());
            for item in item {
                list0.push(*item).expect("reserved supplied list");
            }
            Some(list0)
        }
        None => None,
    };
    let opt4 = match value.authority() {
        Some(item) => Some(owner_authority_fields(item, limits)?),
        None => None,
    };
    let opt5 = match value.instructions() {
        Some(item) => Some(text(item, limits)?),
        None => None,
    };
    Ok(c::Amendment {
        addressed: opt0,
        spec: opt1,
        wake: opt2,
        dependencies: opt3,
        authority: opt4,
        instructions: opt5,
        tracked: match value.tracked() {
            Some(value) => Some(crate::translate::operations::restore_tracking(value)),
            None => None,
        },
        llm: crate::translate::llm::choice(value.llm().as_ref(), limits)?,
    })
}

pub(crate) fn proposal_action_fields(
    value: &w::ProposalAction,
    limits: &Limits,
) -> Result<c::ProposalAction, w::Problem> {
    Ok(match value {
        w::ProposalAction::Batch(row) => c::ProposalAction::Batch {
            tasks: proposed_task_piece(row.tasks(), limits, limits.client.rows)?,
            own: row.own(),
        },
        w::ProposalAction::Effect(row) => {
            c::ProposalAction::Effect { part: crate::translate::facts::part(row.part(), limits) }
        }
        w::ProposalAction::Widen(row) => c::ProposalAction::Widen {
            before: authority_fields(row.before(), limits)?,
            after: authority_fields(row.after(), limits)?,
        },
        w::ProposalAction::Amend(row) => c::ProposalAction::Amend {
            task: row.task(),
            before: text(row.before(), limits)?,
            change: amendment_fields(row.change(), limits)?,
        },
    })
}

pub(crate) fn proposal_fields(value: &w::Proposal, limits: &Limits) -> Result<c::Proposal, w::Problem> {
    let opt0 = match value.ending() {
        Some(item) => Some(ending_fields(item, limits)?),
        None => None,
    };
    Ok(c::Proposal {
        key: crate::translate::answers::proposal_key(value.key(), limits),
        project: value.project(),
        proposer: crate::translate::answers::proposer(value.proposer(), limits),
        why: text(value.why(), limits)?,
        action: proposal_action_fields(value.action(), limits)?,
        needs: authority_fields(value.needs(), limits)?,
        funding: budget_fields(value.funding(), limits),
        waiting: crate::translate::facts::recipient(value.waiting(), limits),
        trail: passed_piece(value.trail(), limits, limits.client.rows)?,
        since: Wall::from_nanos(value.since()),
        acts: offered_piece(value.acts(), limits, c::MAX_OFFERED_ACTS)?,
        ending: opt0,
    })
}

#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn owner_proposal_action_fields(
    value: &w::OwnerProposalAction,
    limits: &Limits,
) -> Result<c::OwnerProposalAction, w::Problem> {
    Ok(match value {
        w::OwnerProposalAction::Settings(row) => c::OwnerProposalAction::Settings(Box::new(
            crate::translate::settings_proposals::restore_owner_proposal_action_settings(row, limits)?,
        )),
        w::OwnerProposalAction::Batch(row) => {
            c::OwnerProposalAction::Batch(proposed_task_piece(row.value(), limits, limits.client.rows)?)
        }
        w::OwnerProposalAction::Effect(row) => c::OwnerProposalAction::Effect {
            part: crate::translate::facts::owner_part(row.part(), limits),
            authority: owner_authority_document_fields(row.authority(), limits)?,
            attempt: row.attempt(),
            completion: row.completion(),
            position: row.position(),
            purpose: row.purpose(),
        },
        w::OwnerProposalAction::ProcedureEffect(row) => c::OwnerProposalAction::ProcedureEffect {
            part: crate::translate::facts::owner_part(row.part(), limits),
            authority: owner_authority_document_fields(row.authority(), limits)?,
            step: row.step(),
            purpose: row.purpose(),
        },
        w::OwnerProposalAction::Amend(row) => {
            let opt0 = match row.addressed() {
                Some(item) => Some(address_change_fields(item, limits)),
                None => None,
            };
            let opt1 = match row.release() {
                Some(item) => Some(*item),
                None => None,
            };
            c::OwnerProposalAction::Amend {
                task: owner_task_link_fields(row.task(), limits)?,
                addressed: opt0,
                before: Box::new(owner_amendment_state_fields(row.before(), limits)?),
                after: Box::new(owner_amendment_state_fields(row.after(), limits)?),
                reason: crate::translate::facts::document_text(row.reason(), limits)?,
                release: opt1,
            }
        }
        w::OwnerProposalAction::Widen(row) => c::OwnerProposalAction::Widen {
            task: owner_task_link_fields(row.task(), limits)?,
            before: owner_authority_document_fields(row.before(), limits)?,
            after: owner_authority_document_fields(row.after(), limits)?,
        },
        w::OwnerProposalAction::Release(row) => {
            let opt0 = match row.hold() {
                Some(item) => Some(Box::new(crate::translate::facts::owner_chip_hold(item, limits)?)),
                None => None,
            };
            c::OwnerProposalAction::Release { task: owner_task_link_fields(row.task(), limits)?, hold: opt0 }
        }
    })
}

pub(crate) fn owner_proposal_waiting_fields(
    value: &w::OwnerProposalWaiting,
    limits: &Limits,
) -> c::OwnerProposalWaiting {
    c::OwnerProposalWaiting {
        holder: owner_history_holder_fields(value.holder(), limits),
        since: Wall::from_nanos(value.since()),
    }
}
