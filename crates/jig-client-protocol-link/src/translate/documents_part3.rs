//! Field-for-field document values (client/domain/contract.md, 5–7).
use super::{
    Box, Limits, List, Reader, Wall, c, chip_fields, chip_piece, ending_fields, hold_fields, live_delegate_piece,
    offered_piece, owner_authority_document_fields, owner_task_link_piece, piece_problem,
    specification_document_fields, spend_line_fields, subscriptions_fields, task_contract_document_fields,
    task_hold_fields, task_resource_piece, task_result_fields, text, too_large, w,
};
pub(crate) fn budget_fields(value: &w::Budget, limits: &Limits) -> c::Budget {
    c::Budget {
        total: value.total(),
        own: value.own(),
        ended: value.ended(),
        reserved: value.reserved(),
        left: value.left(),
        funder: crate::translate::answers::actor(value.funder(), limits),
    }
}

#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn steering_fields(value: &w::Steering, limits: &Limits) -> Result<c::Steering, w::Problem> {
    let opt0 = match value.addressed() {
        Some(item) => Some(crate::translate::facts::recipient(item, limits)),
        None => None,
    };
    let opt1 = match value.person_revision() {
        Some(item) => Some(crate::translate::answers::revision(item, limits)),
        None => None,
    };
    let opt2 = match value.taken_by() {
        Some(item) => Some(*item),
        None => None,
    };
    let opt3 = match value.instructions() {
        Some(item) => Some(crate::translate::facts::document_text(item, limits)?),
        None => None,
    };
    if value.dependencies().len() > limits.client.rows {
        return Err(too_large());
    }
    let mut list4 = List::with_capacity(value.dependencies().len());
    for item in value.dependencies() {
        list4.push(*item).expect("reserved supplied list");
    }
    Ok(c::Steering {
        addressed: opt0,
        person_revision: opt1,
        taken_by: opt2,
        revision: crate::translate::answers::revision(value.revision(), limits),
        spec: specification_document_fields(value.spec(), limits)?,
        instructions: opt3,
        wake: crate::translate::facts::owner_wake_policy(value.wake(), limits),
        dependencies: list4,
        authority: owner_authority_document_fields(value.authority(), limits)?,
        budget: budget_fields(value.budget(), limits),
    })
}

pub(crate) fn phase_counts_fields(value: &w::PhaseCounts, _limits: &Limits) -> c::PhaseCounts {
    c::PhaseCounts {
        ended: value.ended(),
        waiting: value.waiting(),
        running: value.running(),
        held: value.held(),
        closing: value.closing(),
        done: value.done(),
        failed: value.failed(),
        cancelled: value.cancelled(),
    }
}

pub(crate) fn impact_fields(value: &w::Impact, limits: &Limits) -> c::Impact {
    c::Impact {
        tasks: phase_counts_fields(value.tasks(), limits),
        live_runs: value.live_runs(),
        proposals: value.proposals(),
    }
}

#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn passed_fields(value: &w::Passed, limits: &Limits) -> Result<c::Passed, w::Problem> {
    let opt0 = match value.lacking() {
        Some(item) => Some(crate::translate::answers::lacking(item, limits)),
        None => None,
    };
    Ok(c::Passed {
        by: crate::translate::answers::actor(value.by(), limits),
        at: Wall::from_nanos(value.at()),
        reason: text(value.reason(), limits)?,
        lacking: opt0,
    })
}

pub(crate) fn passed_piece(bytes: &[u8], limits: &Limits, bound: u32) -> Result<List<c::Passed>, w::Problem> {
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
        let row = w::Passed::decode(&limits.wire, &mut Reader::new(&item))?;
        let row = passed_fields(&row, limits)?;
        if rows.push(row).is_err() {
            return Err(too_large());
        }
    }
    Err(too_large())
}

pub(crate) fn escalation_fields(value: &w::Escalation, limits: &Limits) -> Result<c::Escalation, w::Problem> {
    let opt0 = match value.ending() {
        Some(item) => Some(ending_fields(item, limits)?),
        None => None,
    };
    Ok(c::Escalation {
        ending: opt0,
        task: chip_fields(value.task(), limits)?,
        hold: hold_fields(value.hold(), limits)?,
        steering: Box::new(steering_fields(value.steering(), limits)?),
        impact: impact_fields(value.impact(), limits),
        waiting: crate::translate::facts::recipient(value.waiting(), limits),
        trail: passed_piece(value.trail(), limits, limits.client.rows)?,
        acts: offered_piece(value.acts(), limits, c::MAX_OFFERED_ACTS)?,
    })
}

pub(crate) fn part_piece(bytes: &[u8], limits: &Limits, bound: u32) -> Result<List<c::Part>, w::Problem> {
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
        let row = w::Part::decode(&limits.wire, &mut Reader::new(&item))?;
        let row = crate::translate::facts::part(&row, limits);
        if rows.push(row).is_err() {
            return Err(too_large());
        }
    }
    Err(too_large())
}

pub(crate) fn closing_fields(value: &w::Closing, limits: &Limits) -> Result<c::Closing, w::Problem> {
    match crate::translate::facts::decode(value.rows(), limits)? {
        crate::translate::facts::Node::Closing(value) => Ok(*value),
        crate::translate::facts::Node::Chip(_)
        | crate::translate::facts::Node::ChipPhase(_)
        | crate::translate::facts::Node::Ending(_)
        | crate::translate::facts::Node::HeldRuntime(_)
        | crate::translate::facts::Node::HoldReason(_)
        | crate::translate::facts::Node::OwnerChipDetails(_)
        | crate::translate::facts::Node::OwnerChipEnding(_)
        | crate::translate::facts::Node::ResultValue(_) => Err(piece_problem(skein_codec::Reason::Malformed)),
    }
}

pub(crate) fn held_runtime_fields(value: &w::HeldRuntime, limits: &Limits) -> Result<c::HeldRuntime, w::Problem> {
    match crate::translate::facts::decode(value.rows(), limits)? {
        crate::translate::facts::Node::HeldRuntime(value) => Ok(*value),
        crate::translate::facts::Node::Chip(_)
        | crate::translate::facts::Node::ChipPhase(_)
        | crate::translate::facts::Node::Closing(_)
        | crate::translate::facts::Node::Ending(_)
        | crate::translate::facts::Node::HoldReason(_)
        | crate::translate::facts::Node::OwnerChipDetails(_)
        | crate::translate::facts::Node::OwnerChipEnding(_)
        | crate::translate::facts::Node::ResultValue(_) => Err(piece_problem(skein_codec::Reason::Malformed)),
    }
}

#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn runtime_fields(value: &w::Runtime, limits: &Limits) -> Result<c::Runtime, w::Problem> {
    Ok(match value {
        w::Runtime::Active(row) => c::Runtime::Active(crate::translate::facts::active_run(row.value(), limits)?),
        w::Runtime::Waiting(row) => {
            if row.dependencies().len() > limits.client.rows {
                return Err(too_large());
            }
            let mut list0 = List::with_capacity(row.dependencies().len());
            for item in row.dependencies() {
                list0.push(*item).expect("reserved supplied list");
            }
            let opt1 = match row.hold() {
                Some(item) => Some(crate::translate::facts::part(item, limits)),
                None => None,
            };
            c::Runtime::Waiting { dependencies: list0, hold: opt1 }
        }
        w::Runtime::Idle => c::Runtime::Idle,
        w::Runtime::Starting => c::Runtime::Starting,
        w::Runtime::Running(row) => {
            let opt0 = match row.tool() {
                Some(item) => Some(text(item, limits)?),
                None => None,
            };
            c::Runtime::Running { attempt: row.attempt(), since: Wall::from_nanos(row.since()), tool: opt0 }
        }
        w::Runtime::WaitingForWords(row) => {
            c::Runtime::WaitingForWords { attempt: row.attempt(), since: Wall::from_nanos(row.since()) }
        }
        w::Runtime::BackingOff(row) => c::Runtime::BackingOff {
            at: Wall::from_nanos(row.at()),
            attempt: row.attempt(),
            tries: row.tries(),
            class: crate::translate::facts::failure_class(row.class(), limits),
            reason: text(row.reason(), limits)?,
        },
        w::Runtime::Procedure(row) => {
            c::Runtime::Procedure { panel: crate::translate::facts::part(row.panel(), limits) }
        }
        w::Runtime::Person(row) => {
            let opt0 = match row.since() {
                Some(item) => Some(Wall::from_nanos(*item)),
                None => None,
            };
            c::Runtime::Person { addressed: crate::translate::facts::recipient(row.addressed(), limits), since: opt0 }
        }
        w::Runtime::Closing(row) => c::Runtime::Closing(closing_fields(row.value(), limits)?),
        w::Runtime::Held(row) => {
            let opt0 = match row.was() {
                Some(item) => Some(held_runtime_fields(item, limits)?),
                None => None,
            };
            c::Runtime::Held { was: opt0 }
        }
        w::Runtime::Ended => c::Runtime::Ended,
    })
}

#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn writer_slot_fields(value: &w::WriterSlot, limits: &Limits) -> c::WriterSlot {
    let opt0 = match value.attempt() {
        Some(item) => Some(*item),
        None => None,
    };
    c::WriterSlot {
        resource: crate::translate::facts::part(value.resource(), limits),
        task: value.task(),
        attempt: opt0,
    }
}

pub(crate) fn writer_slot_piece(bytes: &[u8], limits: &Limits, bound: u32) -> Result<List<c::WriterSlot>, w::Problem> {
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
        let row = w::WriterSlot::decode(&limits.wire, &mut Reader::new(&item))?;
        let row = writer_slot_fields(&row, limits);
        if rows.push(row).is_err() {
            return Err(too_large());
        }
    }
    Err(too_large())
}

pub(crate) fn task_context_fields(value: &w::TaskContext, limits: &Limits) -> Result<c::TaskContext, w::Problem> {
    let opt0 = match value.escalation() {
        Some(item) => Some(escalation_fields(item, limits)?),
        None => None,
    };
    let opt1 = match value.closing() {
        Some(item) => Some(closing_fields(item, limits)?),
        None => None,
    };
    if value.spend().len() > limits.client.rows {
        return Err(too_large());
    }
    let mut list2 = List::with_capacity(value.spend().len());
    for item in value.spend() {
        list2.push(spend_line_fields(item, limits)).expect("reserved supplied list");
    }
    Ok(c::TaskContext {
        escalation: opt0,
        parts: part_piece(value.parts(), limits, limits.client.rows)?,
        closing: opt1,
        spend: list2,
        runtime: runtime_fields(value.runtime(), limits)?,
        impact: impact_fields(value.impact(), limits),
        writer_slots: writer_slot_piece(value.writer_slots(), limits, limits.client.rows)?,
    })
}

#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn task_fields(value: &w::Task, limits: &Limits) -> Result<c::Task, w::Problem> {
    let opt0 = match value.goal() {
        Some(item) => Some(*item),
        None => None,
    };
    let opt1 = match value.priority() {
        Some(item) => Some(*item),
        None => None,
    };
    if value.lineage().len() > limits.client.rows {
        return Err(too_large());
    }
    let mut list2 = List::with_capacity(value.lineage().len());
    for item in value.lineage() {
        list2.push(*item).expect("reserved supplied list");
    }
    let opt3 = match value.person_revision() {
        Some(item) => Some(crate::translate::answers::revision(item, limits)),
        None => None,
    };
    let opt4 = match value.taken_by() {
        Some(item) => Some(*item),
        None => None,
    };
    let opt5 = match value.instructions() {
        Some(item) => Some(crate::translate::facts::document_text(item, limits)?),
        None => None,
    };
    let opt6 = match value.hold() {
        Some(item) => Some(task_hold_fields(item, limits)?),
        None => None,
    };
    if value.proposals().len() > limits.client.rows {
        return Err(too_large());
    }
    let mut list7 = List::with_capacity(value.proposals().len());
    for item in value.proposals() {
        list7.push(crate::translate::answers::proposal_key(item, limits)).expect("reserved supplied list");
    }
    let opt8 = match value.result() {
        Some(item) => Some(task_result_fields(item, limits)?),
        None => None,
    };
    let opt9 = match value.context() {
        Some(item) => Some(Box::new(task_context_fields(item, limits)?)),
        None => None,
    };
    Ok(c::Task {
        settings: match value.settings() {
            Some(settings) => Some(c::TaskSettings {
                task: settings.task(),
                run: match settings.run() {
                    Some(run) => Some(*run),
                    None => None,
                },
                newest: settings.newest(),
            }),
            None => None,
        },
        dependency_facts: crate::translate::facts::dependencies(value.dependency_facts(), limits),
        label: value.label(),
        flags: crate::translate::queries::flags(value.flags()),
        last_activity: Wall::from_nanos(value.last_activity()),
        waits_on: crate::translate::queries::waits(value.waits_on()),
        chip: chip_fields(value.chip(), limits)?,
        goal: opt0,
        priority: opt1,
        lineage_chips: chip_piece(value.lineage_chips(), limits, limits.client.rows)?,
        inputs: owner_task_link_piece(value.inputs(), limits, limits.client.rows)?,
        project: value.project(),
        requester: crate::translate::facts::owner_requester(value.requester(), limits),
        lineage: list2,
        spec: specification_document_fields(value.spec(), limits)?,
        spec_revision: crate::translate::answers::revision(value.spec_revision(), limits),
        person_revision: opt3,
        taken_by: opt4,
        instructions: opt5,
        wake: crate::translate::facts::owner_wake_policy(value.wake(), limits),
        subscriptions: subscriptions_fields(value.subscriptions(), limits)?,
        result_contract: task_contract_document_fields(value.result_contract(), limits)?,
        dependencies: owner_task_link_piece(value.dependencies(), limits, limits.client.rows)?,
        references: owner_task_link_piece(value.references(), limits, limits.client.rows)?,
        cut: value.cut(),
        resources: task_resource_piece(value.resources(), limits, limits.client.rows)?,
        budget: crate::translate::facts::owner_budget(value.budget(), limits),
        authority: owner_authority_document_fields(value.authority(), limits)?,
        hold: opt6,
        proposals: list7,
        activity: value.activity(),
        changed: Wall::from_nanos(value.changed()),
        acts: offered_piece(value.acts(), limits, c::MAX_OFFERED_ACTS)?,
        result: opt8,
        delegates: live_delegate_piece(value.delegates(), limits, limits.client.rows)?,
        context: opt9,
        llm: crate::translate::llm::choice(value.llm().as_ref(), limits)?,
        kept_account: *value.kept_account(),
        run_wait: crate::translate::task_wait::waiting(value.run_wait().as_ref(), limits),
    })
}

#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn tree_node_fields(value: &w::TreeNode, limits: &Limits) -> Result<c::TreeNode, w::Problem> {
    let opt0 = match value.parent() {
        Some(item) => Some(*item),
        None => None,
    };
    if value.dependencies().len() > limits.client.rows {
        return Err(too_large());
    }
    let mut list1 = List::with_capacity(value.dependencies().len());
    for item in value.dependencies() {
        list1.push(*item).expect("reserved supplied list");
    }
    if value.proposals().len() > limits.client.rows {
        return Err(too_large());
    }
    let mut list2 = List::with_capacity(value.proposals().len());
    for item in value.proposals() {
        list2.push(crate::translate::answers::proposal_key(item, limits)).expect("reserved supplied list");
    }
    Ok(c::TreeNode {
        chip: chip_fields(value.chip(), limits)?,
        parent: opt0,
        dependencies: list1,
        proposals: list2,
        spend: value.spend(),
        folded: phase_counts_fields(value.folded(), limits),
        held_below: value.held_below(),
    })
}

pub(crate) fn tree_node_piece(bytes: &[u8], limits: &Limits, bound: u32) -> Result<List<c::TreeNode>, w::Problem> {
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
        let row = w::TreeNode::decode(&limits.wire, &mut Reader::new(&item))?;
        let row = tree_node_fields(&row, limits)?;
        if rows.push(row).is_err() {
            return Err(too_large());
        }
    }
    Err(too_large())
}

pub(crate) fn tree_fields(value: &w::Tree, limits: &Limits) -> Result<c::Tree, w::Problem> {
    Ok(c::Tree {
        task: value.task(),
        nodes: tree_node_piece(value.nodes(), limits, limits.client.rows)?,
        folded: phase_counts_fields(value.folded(), limits),
        held_below: value.held_below(),
    })
}
