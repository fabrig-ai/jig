//! Placed holds retain their own identity and revision (domain/tasks.md, 5.9).
use crate::tasks_v9 as wire;
use jig_domain_tasks as domain;
use skein_lib::List;

pub(super) fn encode(row: &domain::PlacedHold, limits: &wire::Limits) -> Result<wire::PlacedHold, wire::Problem> {
    let owner = match row.owner {
        domain::Owner::Party { person } => {
            wire::HoldOwner::Party(wire::Person::new(limits, wire::PersonParts { person })?)
        }
        domain::Owner::Task { task } => {
            wire::HoldOwner::Task(wire::Number::new(limits, wire::NumberParts { value: task })?)
        }
    };
    let mode = match row.mode {
        domain::Mode::Stop => wire::HoldMode::Stop,
        domain::Mode::Drain => wire::HoldMode::Drain,
    };
    let scope = match row.scope {
        domain::Scope::Subtree { task } => {
            wire::HoldScope::Subtree(wire::SubtreeScope::new(limits, wire::SubtreeScopeParts { task })?)
        }
        domain::Scope::Project => wire::HoldScope::Project,
    };
    let mut exempt = List::with_capacity(limits.placed_hold_exempt);
    for task in &row.exempt {
        if exempt.push(*task).is_err() {
            return Err(wire::Problem { path: wire::Path::PlacedHoldExempt, reason: skein_codec::Reason::Bound });
        }
    }
    wire::PlacedHold::new(
        limits,
        wire::PlacedHoldParts {
            number: row.number,
            project: row.project,
            owner,
            mode,
            scope,
            reason: row.reason.clone(),
            placed_at: row.placed_at,
            revision: row.revision,
            exempt,
            lifted: row.lifted,
        },
    )
}

pub(super) fn decode(row: wire::PlacedHold) -> domain::PlacedHold {
    let wire::PlacedHoldParts { number, project, owner, mode, scope, reason, placed_at, revision, exempt, lifted } =
        row.into_parts();
    let owner = match owner {
        wire::HoldOwner::Party(value) => domain::Owner::Party { person: value.person() },
        wire::HoldOwner::Task(value) => domain::Owner::Task { task: value.value() },
    };
    let mode = match mode {
        wire::HoldMode::Stop => domain::Mode::Stop,
        wire::HoldMode::Drain => domain::Mode::Drain,
    };
    let scope = match scope {
        wire::HoldScope::Subtree(value) => domain::Scope::Subtree { task: value.task() },
        wire::HoldScope::Project => domain::Scope::Project,
    };
    domain::PlacedHold {
        number,
        project,
        owner,
        mode,
        scope,
        reason,
        placed_at,
        revision,
        exempt: exempt.into_boxed(),
        lifted,
    }
}
