//! Core durable rows translated exhaustively (protocol/store.md, 5.1).
use super::{Limits, domain, task_problem, too_large, wire};
use alloc::boxed::Box;
use skein_lib::List;
pub(super) fn encode_projection_goal(
    source: &domain::ProjectionGoal,
    limits: &Limits,
) -> Result<wire::ProjectionGoal, wire::Problem> {
    let domain::ProjectionGoal { number, project, priority, words, phase } = source;
    wire::ProjectionGoal::new(
        &limits.core,
        wire::ProjectionGoalParts {
            number: *number,
            project: *project,
            priority: *priority,
            words: words.clone(),
            phase: encode_phase(phase, limits)?,
        },
    )
}
pub(super) fn decode_projection_goal(
    source: wire::ProjectionGoal,
    limits: &Limits,
) -> Result<domain::ProjectionGoal, wire::Problem> {
    let wire::ProjectionGoalParts { number, project, priority, words, phase } = source.into_parts();
    Ok(domain::ProjectionGoal { number, project, priority, words, phase: decode_phase(phase, limits)? })
}
pub(super) fn encode_projection_task(
    source: &domain::ProjectionTask,
    limits: &Limits,
) -> Result<wire::ProjectionTask, wire::Problem> {
    let domain::ProjectionTask { number, requester, words, phase } = source;
    wire::ProjectionTask::new(
        &limits.core,
        wire::ProjectionTaskParts {
            number: *number,
            requester: encode_party(requester, limits)?,
            words: words.clone(),
            phase: encode_phase(phase, limits)?,
        },
    )
}
pub(super) fn decode_projection_task(
    source: wire::ProjectionTask,
    limits: &Limits,
) -> Result<domain::ProjectionTask, wire::Problem> {
    let wire::ProjectionTaskParts { number, requester, words, phase } = source.into_parts();
    Ok(domain::ProjectionTask {
        number,
        requester: decode_party(requester),
        words,
        phase: decode_phase(phase, limits)?,
    })
}
pub(super) fn encode_projection_milestone(
    source: &domain::ProjectionMilestone,
    limits: &Limits,
) -> Result<wire::ProjectionMilestone, wire::Problem> {
    let domain::ProjectionMilestone { identity, phase, change, words } = source;
    wire::ProjectionMilestone::new(
        &limits.core,
        wire::ProjectionMilestoneParts {
            identity: encode_milestone_id(*identity, limits)?,
            phase: match phase {
                Some(value) => Some(encode_phase(value, limits)?),
                None => None,
            },
            change: match change {
                Some(value) => Some(wire::ChangeBytes::new(
                    &limits.core,
                    wire::ChangeBytesParts {
                        value: match crate::codec::tasks::carried::change(value, &limits.tasks) {
                            Ok(value) => value,
                            Err(error) => return Err(task_problem(error)),
                        },
                    },
                )?),
                None => None,
            },
            words: words.clone(),
        },
    )
}
pub(super) fn decode_projection_milestone(
    source: wire::ProjectionMilestone,
    limits: &Limits,
) -> Result<domain::ProjectionMilestone, wire::Problem> {
    let wire::ProjectionMilestoneParts { identity, phase, change, words } = source.into_parts();
    Ok(domain::ProjectionMilestone {
        identity: decode_milestone_id(identity),
        phase: match phase {
            Some(value) => Some(decode_phase(value, limits)?),
            None => None,
        },
        change: match change {
            Some(value) => {
                Some(match crate::codec::tasks::carried::read_change(&value.into_parts().value, &limits.tasks) {
                    Ok(value) => value,
                    Err(error) => return Err(task_problem(error)),
                })
            }
            None => None,
        },
        words,
    })
}
pub(super) fn encode_projection_record(
    source: &domain::ProjectionRecord,
    limits: &Limits,
) -> Result<wire::ProjectionRecord, wire::Problem> {
    Ok(match source {
        domain::ProjectionRecord::Failure(row) => wire::ProjectionRecord::Failure(wire::ProjectionFailure::new(
            &limits.core,
            wire::ProjectionFailureParts {
                goal: row.goal,
                connector: row.connector,
                after: row.after,
                state: encode_failure_state(row.state, limits)?,
            },
        )?),
        domain::ProjectionRecord::Goal { goal, pending, changed, fed_at, closing_at } => {
            wire::ProjectionRecord::Goal(wire::ProjectionGoalRow::new(
                &limits.core,
                wire::ProjectionGoalRowParts {
                    goal: encode_projection_goal(goal, limits)?,
                    pending: encode_pending(pending, limits)?,
                    changed: *changed,
                    fed_at: *fed_at,
                    closing_at: *closing_at,
                },
            )?)
        }
        domain::ProjectionRecord::Task { goal, task } => wire::ProjectionRecord::Task(wire::ProjectionTaskRow::new(
            &limits.core,
            wire::ProjectionTaskRowParts { goal: *goal, task: encode_projection_task(task, limits)? },
        )?),
        domain::ProjectionRecord::Milestone { goal, milestone } => {
            wire::ProjectionRecord::Milestone(wire::ProjectionMilestoneRow::new(
                &limits.core,
                wire::ProjectionMilestoneRowParts {
                    goal: *goal,
                    milestone: encode_projection_milestone(milestone, limits)?,
                },
            )?)
        }
    })
}
pub(super) fn decode_projection_record(
    source: wire::ProjectionRecord,
    limits: &Limits,
) -> Result<domain::ProjectionRecord, wire::Problem> {
    Ok(match source {
        wire::ProjectionRecord::Failure(row) => {
            let wire::ProjectionFailureParts { goal, connector, after, state } = row.into_parts();
            domain::ProjectionRecord::Failure(domain::ProjectionFailure {
                goal,
                connector,
                after,
                state: decode_failure_state(state),
            })
        }
        wire::ProjectionRecord::Goal(value) => {
            let wire::ProjectionGoalRowParts { goal, pending, changed, fed_at, closing_at } = value.into_parts();
            domain::ProjectionRecord::Goal {
                goal: decode_projection_goal(goal, limits)?,
                pending: decode_pending(pending),
                changed,
                fed_at,
                closing_at,
            }
        }
        wire::ProjectionRecord::Task(value) => {
            let wire::ProjectionTaskRowParts { goal, task } = value.into_parts();
            domain::ProjectionRecord::Task { goal, task: decode_projection_task(task, limits)? }
        }
        wire::ProjectionRecord::Milestone(value) => {
            let wire::ProjectionMilestoneRowParts { goal, milestone } = value.into_parts();
            domain::ProjectionRecord::Milestone { goal, milestone: decode_projection_milestone(milestone, limits)? }
        }
    })
}
fn encode_phase(source: &jig_domain_tasks::Phase, limits: &Limits) -> Result<wire::PhaseBytes, wire::Problem> {
    wire::PhaseBytes::new(
        &limits.core,
        wire::PhaseBytesParts {
            value: match crate::codec::tasks::carried::phase(source, &limits.tasks) {
                Ok(value) => value,
                Err(error) => return Err(task_problem(error)),
            },
        },
    )
}
fn decode_phase(source: wire::PhaseBytes, limits: &Limits) -> Result<jig_domain_tasks::Phase, wire::Problem> {
    match crate::codec::tasks::carried::read_phase(&source.into_parts().value, &limits.tasks) {
        Ok(value) => Ok(value),
        Err(error) => Err(task_problem(error)),
    }
}
fn encode_pending(values: &[u16], limits: &Limits) -> Result<List<wire::Connector>, wire::Problem> {
    let count = match u32::try_from(values.len()) {
        Ok(value) => value,
        Err(error) => return Err(too_large(error)),
    };
    if count > limits.core.projection_goal_row_pending {
        return Err(super::problem(skein_codec::Reason::Bound));
    }
    let mut result = List::with_capacity(count);
    for value in values {
        result
            .push(wire::Connector::new(&limits.core, wire::ConnectorParts { value: *value })?)
            .expect("same number of connectors");
    }
    Ok(result)
}
fn decode_pending(values: List<wire::Connector>) -> Box<[u16]> {
    let mut result = List::with_capacity(values.len());
    for value in values.into_boxed() {
        result.push(value.into_parts().value).expect("same number of connectors");
    }
    result.into_boxed()
}
fn encode_milestone_id(value: domain::MilestoneId, limits: &Limits) -> Result<wire::MilestoneId, wire::Problem> {
    Ok(match value {
        domain::MilestoneId::Lifecycle { task, position } => wire::MilestoneId::Lifecycle(
            wire::MilestoneLifecycle::new(&limits.core, wire::MilestoneLifecycleParts { task, position })?,
        ),
        domain::MilestoneId::Revision { task, revision } => wire::MilestoneId::Revision(wire::MilestoneRevision::new(
            &limits.core,
            wire::MilestoneRevisionParts { task, revision },
        )?),
    })
}
fn decode_milestone_id(value: wire::MilestoneId) -> domain::MilestoneId {
    match value {
        wire::MilestoneId::Lifecycle(value) => {
            let wire::MilestoneLifecycleParts { task, position } = value.into_parts();
            domain::MilestoneId::Lifecycle { task, position }
        }
        wire::MilestoneId::Revision(value) => {
            let wire::MilestoneRevisionParts { task, revision } = value.into_parts();
            domain::MilestoneId::Revision { task, revision }
        }
    }
}
fn encode_party(value: &jig_domain_tasks::Party, limits: &Limits) -> Result<wire::Party, wire::Problem> {
    Ok(match value {
        jig_domain_tasks::Party::Task(value) => {
            wire::Party::Task(wire::PartyTask::new(&limits.core, wire::PartyTaskParts { value: *value })?)
        }
        jig_domain_tasks::Party::Person(value) => {
            wire::Party::Person(wire::PartyPerson::new(&limits.core, wire::PartyPersonParts { value: *value })?)
        }
        jig_domain_tasks::Party::Deployment { project } => wire::Party::Deployment(wire::PartyDeployment::new(
            &limits.core,
            wire::PartyDeploymentParts { project: *project },
        )?),
    })
}
fn decode_party(value: wire::Party) -> jig_domain_tasks::Party {
    match value {
        wire::Party::Task(value) => jig_domain_tasks::Party::Task(value.into_parts().value),
        wire::Party::Person(value) => jig_domain_tasks::Party::Person(value.into_parts().value),
        wire::Party::Deployment(value) => jig_domain_tasks::Party::Deployment { project: value.into_parts().project },
    }
}

fn encode_failure_state(
    state: domain::ProjectionFailureState,
    limits: &Limits,
) -> Result<wire::ProjectionFailureState, wire::Problem> {
    Ok(match state {
        domain::ProjectionFailureState::Armed => wire::ProjectionFailureState::Armed,
        domain::ProjectionFailureState::Pending(source) => {
            wire::ProjectionFailureState::Pending(wire::ProjectionFailureSource::new(
                &limits.core,
                wire::ProjectionFailureSourceParts { source: source.source, at: source.at.as_nanos() },
            )?)
        }
        domain::ProjectionFailureState::Reported(source) => {
            wire::ProjectionFailureState::Reported(wire::ProjectionFailureSource::new(
                &limits.core,
                wire::ProjectionFailureSourceParts { source: source.source, at: source.at.as_nanos() },
            )?)
        }
    })
}
fn decode_failure_state(state: wire::ProjectionFailureState) -> domain::ProjectionFailureState {
    match state {
        wire::ProjectionFailureState::Armed => domain::ProjectionFailureState::Armed,
        wire::ProjectionFailureState::Pending(source) => {
            let row = source.into_parts();
            domain::ProjectionFailureState::Pending(domain::ProjectionFailureSource {
                source: row.source,
                at: skein_lib::Wall::from_nanos(row.at),
            })
        }
        wire::ProjectionFailureState::Reported(source) => {
            let row = source.into_parts();
            domain::ProjectionFailureState::Reported(domain::ProjectionFailureSource {
                source: row.source,
                at: skein_lib::Wall::from_nanos(row.at),
            })
        }
    }
}
