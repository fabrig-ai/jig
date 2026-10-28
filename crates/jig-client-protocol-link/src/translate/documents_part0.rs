//! Field-for-field document values (client/domain/contract.md, 5–7).
use super::{Limits, List, Reader, Wall, c, piece_problem, too_large, w};
pub(crate) fn frame_party_fields(value: &w::MeParty, limits: &Limits) -> Result<c::MeParty, w::Problem> {
    Ok(c::MeParty {
        number: value.number(),
        kind: crate::translate::answers::party_kind(value.kind(), limits),
        login: crate::translate::facts::document_text(value.login(), limits)?,
        name: crate::translate::facts::document_text(value.name(), limits)?,
    })
}

#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn pool_ledger_fields(value: &w::PoolLedger, limits: &Limits) -> c::PoolLedger {
    let opt0 = match value.parent() {
        Some(item) => Some(crate::translate::facts::funder(item, limits)),
        None => None,
    };
    let opt1 = match value.began() {
        Some(item) => Some(Wall::from_nanos(*item)),
        None => None,
    };
    let opt2 = match value.next() {
        Some(item) => Some(Wall::from_nanos(*item)),
        None => None,
    };
    c::PoolLedger {
        source: crate::translate::facts::funder(value.source(), limits),
        parent: opt0,
        numbers: crate::translate::facts::owner_numbers(value.numbers(), limits),
        left: value.left(),
        closed: value.closed(),
        began: opt1,
        next: opt2,
    }
}

#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn project_role_fields(value: &w::MeProject, limits: &Limits) -> Result<c::MeProject, w::Problem> {
    let opt1 = match value.pool() {
        Some(item) => Some(pool_ledger_fields(item, limits)),
        None => None,
    };
    Ok(c::MeProject {
        project: value.project(),
        name: crate::translate::facts::document_text(value.name(), limits)?,
        role: crate::translate::answers::held_role(value.role(), limits),
        pool: opt1,
        templates: super::me::templates(value.templates(), limits)?,
        creating: match value.creating() {
            Some(value) => Some(c::Creating {
                within: match super::bounds_fields(&w::Bounds::Create(value.within().clone()), limits)? {
                    c::Bounds::Create(value) => value,
                    c::Bounds::Amend { .. } => unreachable!("wire creating bounds"),
                },
                beyond: super::beyond_fields(value.beyond(), limits)?,
            }),
            None => None,
        },
        models: crate::translate::llm::models(value.models(), limits)?,
    })
}

pub(crate) fn needs_fields(value: &w::Needs, _limits: &Limits) -> c::Needs {
    c::Needs {
        questions: value.questions(),
        proposals: value.proposals(),
        escalations: value.escalations(),
        person_tasks: value.person_tasks(),
    }
}

pub(crate) fn count_fields(value: &w::Count, _limits: &Limits) -> c::Count {
    c::Count { value: value.value(), limited: value.limited() }
}

#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn frame_fields(value: &w::Me, limits: &Limits) -> Result<c::Me, w::Problem> {
    let list0 = super::me::projects(value.projects(), limits)?;
    let opt1 = match value.deployment_role() {
        Some(item) => Some(crate::translate::answers::held_role(item, limits)),
        None => None,
    };
    if value.key_bytes() > limits.wire.keyed_value_some || value.redirect_bytes() > limits.wire.keyed_value_some {
        return Err(too_large());
    }
    Ok(c::Me {
        providers: super::catalogue::offers(value.providers(), limits, &mut super::document_room(limits))?,
        providers_cut: value.providers_cut(),
        key_bytes: value.key_bytes(),
        redirect_bytes: value.redirect_bytes(),
        party: frame_party_fields(value.party(), limits)?,
        projects: list0,
        needs: needs_fields(value.needs(), limits),
        needs_total: value.needs_total(),
        updates: count_fields(value.updates(), limits),
        results_seen: value.results_seen(),
        cut: value.cut(),
        retention: value.retention(),
        deployment_role: opt1,
    })
}

pub(crate) fn chip_fields(value: &w::Chip, limits: &Limits) -> Result<c::Chip, w::Problem> {
    match crate::translate::facts::decode(value.rows(), limits)? {
        crate::translate::facts::Node::Chip(value) => Ok(*value),
        crate::translate::facts::Node::ChipPhase(_)
        | crate::translate::facts::Node::Closing(_)
        | crate::translate::facts::Node::Ending(_)
        | crate::translate::facts::Node::HeldRuntime(_)
        | crate::translate::facts::Node::HoldReason(_)
        | crate::translate::facts::Node::OwnerChipDetails(_)
        | crate::translate::facts::Node::OwnerChipEnding(_)
        | crate::translate::facts::Node::ResultValue(_) => Err(piece_problem(skein_codec::Reason::Malformed)),
    }
}

pub(crate) fn chip_piece(bytes: &[u8], limits: &Limits, bound: u32) -> Result<List<c::Chip>, w::Problem> {
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
        let row = w::Chip::decode(&limits.wire, &mut Reader::new(&item))?;
        let row = chip_fields(&row, limits)?;
        if rows.push(row).is_err() {
            return Err(too_large());
        }
    }
    Err(too_large())
}

pub(crate) fn owner_task_status_fields(value: &w::OwnerTaskStatus, _limits: &Limits) -> c::OwnerTaskStatus {
    match value {
        w::OwnerTaskStatus::Done => c::OwnerTaskStatus::Done,
        w::OwnerTaskStatus::Failed => c::OwnerTaskStatus::Failed,
        w::OwnerTaskStatus::Cancelled => c::OwnerTaskStatus::Cancelled,
    }
}

#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn owner_task_link_fields(
    value: &w::OwnerTaskLink,
    limits: &Limits,
) -> Result<c::OwnerTaskLink, w::Problem> {
    Ok(match value {
        w::OwnerTaskLink::Live(row) => c::OwnerTaskLink::Live(chip_fields(row.value(), limits)?),
        w::OwnerTaskLink::Retained(row) => {
            let opt0 = match row.at() {
                Some(item) => Some(Wall::from_nanos(*item)),
                None => None,
            };
            c::OwnerTaskLink::Retained { chip: chip_fields(row.chip(), limits)?, at: opt0 }
        }
        w::OwnerTaskLink::Stored(row) => {
            let opt0 = match row.status() {
                Some(item) => Some(owner_task_status_fields(item, limits)),
                None => None,
            };
            c::OwnerTaskLink::Stored { number: row.number(), status: opt0 }
        }
    })
}

pub(crate) fn owner_task_link_piece(
    bytes: &[u8],
    limits: &Limits,
    bound: u32,
) -> Result<List<c::OwnerTaskLink>, w::Problem> {
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
        let row = w::OwnerTaskLink::decode(&limits.wire, &mut Reader::new(&item))?;
        let row = owner_task_link_fields(&row, limits)?;
        if rows.push(row).is_err() {
            return Err(too_large());
        }
    }
    Err(too_large())
}

pub(crate) fn parameter_document_fields(
    value: &w::ParameterDocument,
    limits: &Limits,
) -> Result<c::ParameterDocument, w::Problem> {
    Ok(match value {
        w::ParameterDocument::Number(row) => c::ParameterDocument::Number { name: row.name(), value: row.value() },
        w::ParameterDocument::Bytes(row) => c::ParameterDocument::Bytes {
            name: row.name(),
            value: crate::translate::facts::document_text(row.value(), limits)?,
        },
        w::ParameterDocument::Resource(row) => {
            c::ParameterDocument::Resource { name: row.name(), connector: row.connector(), resource: row.resource() }
        }
    })
}

pub(crate) fn document_specification_fields(
    value: &w::DocumentSpecification,
    limits: &Limits,
) -> Result<c::DocumentSpecification, w::Problem> {
    if value.parameters().len() > limits.client.rows {
        return Err(too_large());
    }
    let mut list0 = List::with_capacity(value.parameters().len());
    for item in value.parameters() {
        list0.push(parameter_document_fields(item, limits)?).expect("reserved supplied list");
    }
    if value.inputs().len() > limits.client.rows {
        return Err(too_large());
    }
    let mut list1 = List::with_capacity(value.inputs().len());
    for item in value.inputs() {
        list1.push(*item).expect("reserved supplied list");
    }
    Ok(c::DocumentSpecification {
        asks: value.asks(),
        words: crate::translate::facts::document_text(value.words(), limits)?,
        parameters: list0,
        inputs: list1,
    })
}

pub(crate) fn specification_document_fields(
    value: &w::SpecificationDocument,
    limits: &Limits,
) -> Result<c::SpecificationDocument, w::Problem> {
    Ok(c::SpecificationDocument {
        specification: document_specification_fields(value.specification(), limits)?,
        cut: value.cut(),
    })
}

#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn subscription_kind_fields(value: &w::SubscriptionKind, _limits: &Limits) -> c::SubscriptionKind {
    match value {
        w::SubscriptionKind::Task(row) => {
            c::SubscriptionKind::Task { target: row.target(), held: row.held(), result: row.result() }
        }
        w::SubscriptionKind::Timer(row) => {
            let opt0 = match row.period() {
                Some(item) => Some(*item),
                None => None,
            };
            c::SubscriptionKind::Timer { at: Wall::from_nanos(row.at()), period: opt0 }
        }
        w::SubscriptionKind::Topic(row) => {
            c::SubscriptionKind::Topic { connector: row.connector(), topic: row.topic() }
        }
    }
}
