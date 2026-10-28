//! Durable task values translated exhaustively (protocol/store.md, 5.1).
use super::phase;
use crate::tasks_v9 as wire;
use alloc::boxed::Box;
use jig_domain_tasks as domain;
use skein_lib::{List, Token, Wall};

/// Translate `Status` to its durable tag.
#[must_use]
pub fn status(source: domain::Status) -> wire::Status {
    match source {
        domain::Status::Done => wire::Status::Done,
        domain::Status::Failed => wire::Status::Failed,
        domain::Status::Cancelled => wire::Status::Cancelled,
    }
}
/// Read `Status` from its durable tag.
#[must_use]
pub fn read_status(source: wire::Status) -> domain::Status {
    match source {
        wire::Status::Done => domain::Status::Done,
        wire::Status::Failed => domain::Status::Failed,
        wire::Status::Cancelled => domain::Status::Cancelled,
    }
}

/// Translate `InvalidResult` to its durable tag.
#[must_use]
pub fn invalid_result(source: domain::InvalidResult) -> wire::InvalidResult {
    match source {
        domain::InvalidResult::Form => wire::InvalidResult::Form,
        domain::InvalidResult::Verdict => wire::InvalidResult::Verdict,
        domain::InvalidResult::Words => wire::InvalidResult::Words,
        domain::InvalidResult::Change => wire::InvalidResult::Change,
        domain::InvalidResult::Followups => wire::InvalidResult::Followups,
    }
}
/// Read `InvalidResult` from its durable tag.
#[must_use]
pub fn read_invalid_result(source: wire::InvalidResult) -> domain::InvalidResult {
    match source {
        wire::InvalidResult::Form => domain::InvalidResult::Form,
        wire::InvalidResult::Verdict => domain::InvalidResult::Verdict,
        wire::InvalidResult::Words => domain::InvalidResult::Words,
        wire::InvalidResult::Change => domain::InvalidResult::Change,
        wire::InvalidResult::Followups => domain::InvalidResult::Followups,
    }
}

/// Translate `Stub` to its durable record.
pub fn stub(source: &domain::Stub, limits: &wire::Limits) -> Result<wire::Stub, wire::Problem> {
    let domain::Stub { task, phase, result } = source;
    wire::Stub::new(limits, wire::StubParts { task: *task, phase: status(*phase), result: result.raw() })
}
/// Read `Stub` from its durable record.
pub fn read_stub(source: wire::Stub, _limits: &wire::Limits) -> Result<domain::Stub, wire::Problem> {
    let wire::StubParts { task, phase, result } = source.into_parts();
    Ok(domain::Stub { task, phase: read_status(phase), result: Token::new(result) })
}

/// Translate `ReplyHint` to its durable record.
pub fn reply_hint(source: &domain::ReplyHint, limits: &wire::Limits) -> Result<wire::ReplyHint, wire::Problem> {
    let domain::ReplyHint { number, activity, attempt, turn, at } = source;
    wire::ReplyHint::new(
        limits,
        wire::ReplyHintParts {
            number: *number,
            activity: *activity,
            attempt: *attempt,
            turn: *turn,
            at: at.as_nanos(),
        },
    )
}
/// Read `ReplyHint` from its durable record.
pub fn read_reply_hint(source: wire::ReplyHint, _limits: &wire::Limits) -> Result<domain::ReplyHint, wire::Problem> {
    let wire::ReplyHintParts { number, activity, attempt, turn, at } = source.into_parts();
    Ok(domain::ReplyHint { number, activity, attempt, turn, at: Wall::from_nanos(at) })
}

/// Translate `EscalationHolder` to its durable value.
pub fn escalation_holder(
    source: &domain::EscalationHolder,
    limits: &wire::Limits,
) -> Result<wire::EscalationHolder, wire::Problem> {
    Ok(match source {
        domain::EscalationHolder::Task(task) => {
            wire::EscalationHolder::Task(wire::EscalationTask::new(limits, wire::EscalationTaskParts { task: *task })?)
        }
        domain::EscalationHolder::Person(person) => wire::EscalationHolder::Person(wire::EscalationPerson::new(
            limits,
            wire::EscalationPersonParts { person: *person },
        )?),
        domain::EscalationHolder::Role { project, role } => wire::EscalationHolder::Role(wire::EscalationRole::new(
            limits,
            wire::EscalationRoleParts { project: *project, role: *role },
        )?),
    })
}
/// Read `EscalationHolder` from its durable value.
pub fn read_escalation_holder(
    source: wire::EscalationHolder,
    _limits: &wire::Limits,
) -> Result<domain::EscalationHolder, wire::Problem> {
    Ok(match source {
        wire::EscalationHolder::Task(source) => {
            let wire::EscalationTaskParts { task } = source.into_parts();
            domain::EscalationHolder::Task(task)
        }
        wire::EscalationHolder::Person(source) => {
            let wire::EscalationPersonParts { person } = source.into_parts();
            domain::EscalationHolder::Person(person)
        }
        wire::EscalationHolder::Role(source) => {
            let wire::EscalationRoleParts { project, role } = source.into_parts();
            domain::EscalationHolder::Role { project, role }
        }
    })
}

/// Translate `Escalation` to its durable value.
pub fn escalation(source: &domain::Escalation, limits: &wire::Limits) -> Result<wire::Escalation, wire::Problem> {
    Ok(match source {
        domain::Escalation::Unheld { revision } => wire::Escalation::Unheld(wire::UnheldEscalation::new(
            limits,
            wire::UnheldEscalationParts { revision: *revision },
        )?),
        domain::Escalation::Routing { revision } => wire::Escalation::Routing(wire::RoutingEscalation::new(
            limits,
            wire::RoutingEscalationParts { revision: *revision },
        )?),
        domain::Escalation::Waiting { revision, holder, entry, since } => {
            wire::Escalation::Waiting(wire::WaitingEscalation::new(
                limits,
                wire::WaitingEscalationParts {
                    revision: *revision,
                    holder: escalation_holder(holder, limits)?,
                    entry: *entry,
                    since: since.as_nanos(),
                },
            )?)
        }
        domain::Escalation::Rejected { revision, by, reason } => {
            wire::Escalation::Rejected(wire::RejectedEscalation::new(
                limits,
                wire::RejectedEscalationParts { revision: *revision, by: *by, reason: reason.clone() },
            )?)
        }
    })
}
/// Read `Escalation` from its durable value.
pub fn read_escalation(source: wire::Escalation, limits: &wire::Limits) -> Result<domain::Escalation, wire::Problem> {
    Ok(match source {
        wire::Escalation::Unheld(source) => {
            let wire::UnheldEscalationParts { revision } = source.into_parts();
            domain::Escalation::Unheld { revision }
        }
        wire::Escalation::Routing(source) => {
            let wire::RoutingEscalationParts { revision } = source.into_parts();
            domain::Escalation::Routing { revision }
        }
        wire::Escalation::Waiting(source) => {
            let wire::WaitingEscalationParts { revision, holder, entry, since } = source.into_parts();
            domain::Escalation::Waiting {
                revision,
                holder: read_escalation_holder(holder, limits)?,
                entry,
                since: Wall::from_nanos(since),
            }
        }
        wire::Escalation::Rejected(source) => {
            let wire::RejectedEscalationParts { revision, by, reason } = source.into_parts();
            domain::Escalation::Rejected { revision, by, reason }
        }
    })
}

/// Translate `PersonAnswer` to its durable record.
pub fn person_answer(
    source: &domain::PersonAnswer,
    limits: &wire::Limits,
) -> Result<wire::PersonAnswer, wire::Problem> {
    let domain::PersonAnswer { by, at } = source;
    wire::PersonAnswer::new(limits, wire::PersonAnswerParts { by: *by, at: at.as_nanos() })
}
/// Read `PersonAnswer` from its durable record.
pub fn read_person_answer(
    source: wire::PersonAnswer,
    _limits: &wire::Limits,
) -> Result<domain::PersonAnswer, wire::Problem> {
    let wire::PersonAnswerParts { by, at } = source.into_parts();
    Ok(domain::PersonAnswer { by, at: Wall::from_nanos(at) })
}

/// Translate `PersonStallNotice` to its durable record.
pub fn person_stall_notice(
    source: &domain::PersonStallNotice,
    limits: &wire::Limits,
) -> Result<wire::PersonStallNotice, wire::Problem> {
    let domain::PersonStallNotice { message, at, activity } = source;
    wire::PersonStallNotice::new(
        limits,
        wire::PersonStallNoticeParts { message: *message, at: at.as_nanos(), activity: *activity },
    )
}
/// Read `PersonStallNotice` from its durable record.
pub fn read_person_stall_notice(
    source: wire::PersonStallNotice,
    _limits: &wire::Limits,
) -> Result<domain::PersonStallNotice, wire::Problem> {
    let wire::PersonStallNoticeParts { message, at, activity } = source.into_parts();
    Ok(domain::PersonStallNotice { message, at: Wall::from_nanos(at), activity })
}

/// Translate `PersonWait` to its durable record.
pub fn person_wait(source: &domain::PersonWait, limits: &wire::Limits) -> Result<wire::PersonWait, wire::Problem> {
    let domain::PersonWait { since, stall, deadline, notice } = source;
    wire::PersonWait::new(
        limits,
        wire::PersonWaitParts {
            since: since.as_nanos(),
            stall: *stall,
            deadline: deadline.as_nanos(),
            notice: optional_notice(notice.as_ref(), limits)?,
        },
    )
}
/// Read `PersonWait` from its durable record.
pub fn read_person_wait(source: wire::PersonWait, limits: &wire::Limits) -> Result<domain::PersonWait, wire::Problem> {
    let wire::PersonWaitParts { since, stall, deadline, notice } = source.into_parts();
    Ok(domain::PersonWait {
        since: Wall::from_nanos(since),
        stall,
        deadline: Wall::from_nanos(deadline),
        notice: read_optional_notice(notice, limits)?,
    })
}

/// Translate `RecurringState` to its durable record.
pub fn recurring_state(
    source: &domain::RecurringState,
    limits: &wire::Limits,
    moved: Option<&[u64; 64]>,
) -> Result<wire::RecurringState, wire::Problem> {
    let domain::RecurringState { template, last_batch, last_period, pending_period } = source;
    wire::RecurringState::new(
        limits,
        wire::RecurringStateParts {
            template: phase::template(template, limits, moved)?,
            last_batch: phase::references(
                last_batch,
                limits.recurring_state_last_batch,
                wire::Path::RecurringStateLastBatch,
                limits,
            )?,
            last_period: *last_period,
            pending_period: *pending_period,
        },
    )
}
/// Read `RecurringState` from its durable record.
pub fn read_recurring_state(
    source: wire::RecurringState,
    limits: &wire::Limits,
) -> Result<domain::RecurringState, wire::Problem> {
    let wire::RecurringStateParts { template, last_batch, last_period, pending_period } = source.into_parts();
    Ok(domain::RecurringState {
        template: phase::read_template(template, limits)?,
        last_batch: phase::read_references(last_batch),
        last_period,
        pending_period,
    })
}

/// Translate `QuestionCredit` to its durable record.
pub fn question_credit(
    source: &domain::QuestionCredit,
    limits: &wire::Limits,
) -> Result<wire::QuestionCredit, wire::Problem> {
    let domain::QuestionCredit { number, answerer, asked, line } = source;
    wire::QuestionCredit::new(
        limits,
        wire::QuestionCreditParts {
            number: *number,
            answerer: super::value::party(answerer, limits)?,
            asked: super::value::party(asked, limits)?,
            line: *line,
        },
    )
}
/// Read `QuestionCredit` from its durable record.
pub fn read_question_credit(
    source: wire::QuestionCredit,
    _limits: &wire::Limits,
) -> Result<domain::QuestionCredit, wire::Problem> {
    let wire::QuestionCreditParts { number, answerer, asked, line } = source.into_parts();
    Ok(domain::QuestionCredit {
        number,
        answerer: super::value::read_party(answerer),
        asked: super::value::read_party(asked),
        line,
    })
}

/// Translate `SubscriptionKind` to its durable value.
pub fn subscription_kind(
    source: &domain::SubscriptionKind,
    limits: &wire::Limits,
) -> Result<wire::SubscriptionKind, wire::Problem> {
    Ok(match source {
        domain::SubscriptionKind::Task { target, held, result } => wire::SubscriptionKind::Task(
            wire::TaskInterest::new(limits, wire::TaskInterestParts { target: *target, held: *held, result: *result })?,
        ),
        domain::SubscriptionKind::Timer { at, period } => wire::SubscriptionKind::Timer(wire::TimerInterest::new(
            limits,
            wire::TimerInterestParts { at: at.as_nanos(), period: *period },
        )?),
        domain::SubscriptionKind::Topic { connector, topic } => wire::SubscriptionKind::Topic(
            wire::TopicInterest::new(limits, wire::TopicInterestParts { connector: *connector, topic: *topic })?,
        ),
    })
}
/// Read `SubscriptionKind` from its durable value.
pub fn read_subscription_kind(
    source: wire::SubscriptionKind,
    _limits: &wire::Limits,
) -> Result<domain::SubscriptionKind, wire::Problem> {
    Ok(match source {
        wire::SubscriptionKind::Task(source) => {
            let wire::TaskInterestParts { target, held, result } = source.into_parts();
            domain::SubscriptionKind::Task { target, held, result }
        }
        wire::SubscriptionKind::Timer(source) => {
            let wire::TimerInterestParts { at, period } = source.into_parts();
            domain::SubscriptionKind::Timer { at: Wall::from_nanos(at), period }
        }
        wire::SubscriptionKind::Topic(source) => {
            let wire::TopicInterestParts { connector, topic } = source.into_parts();
            domain::SubscriptionKind::Topic { connector, topic }
        }
    })
}

/// Translate `Subscription` to its durable record.
pub fn subscription(source: &domain::Subscription, limits: &wire::Limits) -> Result<wire::Subscription, wire::Problem> {
    let domain::Subscription { number, kind } = source;
    wire::Subscription::new(limits, wire::SubscriptionParts { number: *number, kind: subscription_kind(kind, limits)? })
}
/// Read `Subscription` from its durable record.
pub fn read_subscription(
    source: wire::Subscription,
    limits: &wire::Limits,
) -> Result<domain::Subscription, wire::Problem> {
    let wire::SubscriptionParts { number, kind } = source.into_parts();
    Ok(domain::Subscription { number, kind: read_subscription_kind(kind, limits)? })
}

/// Translate `SavedResource` to its durable record.
pub fn saved_resource(
    source: &domain::SavedResource,
    limits: &wire::Limits,
) -> Result<wire::SavedResource, wire::Problem> {
    let domain::SavedResource { connector, path } = source;
    wire::SavedResource::new(
        limits,
        wire::SavedResourceParts { connector: *connector, path: saved_path(path, limits)? },
    )
}
/// Read `SavedResource` from its durable record.
pub fn read_saved_resource(
    source: wire::SavedResource,
    _limits: &wire::Limits,
) -> Result<domain::SavedResource, wire::Problem> {
    let wire::SavedResourceParts { connector, path } = source.into_parts();
    Ok(domain::SavedResource { connector, path: read_saved_path(path) })
}

fn optional_notice(
    source: Option<&domain::PersonStallNotice>,
    limits: &wire::Limits,
) -> Result<Option<wire::PersonStallNotice>, wire::Problem> {
    match source {
        Some(source) => Ok(Some(person_stall_notice(source, limits)?)),
        None => Ok(None),
    }
}
fn read_optional_notice(
    source: Option<wire::PersonStallNotice>,
    limits: &wire::Limits,
) -> Result<Option<domain::PersonStallNotice>, wire::Problem> {
    match source {
        Some(source) => Ok(Some(read_person_stall_notice(source, limits)?)),
        None => Ok(None),
    }
}

fn saved_path(source: &[Box<[u8]>], limits: &wire::Limits) -> Result<List<wire::AuthoritySegment>, wire::Problem> {
    let mut path = List::with_capacity(limits.saved_resource_path);
    for segment in source {
        let segment = wire::AuthoritySegment::new(limits, wire::AuthoritySegmentParts { bytes: segment.clone() })?;
        if path.push(segment).is_err() {
            return Err(problem(wire::Path::SavedResourcePath));
        }
    }
    Ok(path)
}
fn read_saved_path(source: List<wire::AuthoritySegment>) -> Box<[Box<[u8]>]> {
    let mut path = List::with_capacity(source.len());
    for segment in source.into_boxed() {
        let segment = segment.into_parts().bytes;
        path.push(segment).expect("same number of segments");
    }
    path.into_boxed()
}
fn problem(path: wire::Path) -> wire::Problem {
    wire::Problem { path, reason: skein_codec::Reason::Bound }
}
