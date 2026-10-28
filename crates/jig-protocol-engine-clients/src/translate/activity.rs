//! Page-carried immutable activity and opaque turns (protocol/client.md, 3).
use super::{chips, objects::question_answer, w};
use jig_domain_core::reads;
use jig_domain_tasks as t;
use skein_lib::bytes;

pub fn line(value: &t::ActivityLine, limits: &w::Limits) -> Result<w::OwnerActivityLine, w::Problem> {
    let line = match &value.line {
        t::Line::Words { .. } | t::Line::Heard(_) | t::Line::Notified { .. } => {
            return question_answer::answer(value, limits);
        }
        t::Line::RunBegan { attempt, why: value, resumed } => {
            w::OwnerActivity::RunBegan(w::OwnerActivityRunBegan::new(
                limits,
                w::OwnerActivityRunBeganParts { attempt: *attempt, why: why(*value, limits)?, resumed: *resumed },
            )?)
        }
        t::Line::RunEnded { attempt, end: value, spent } => w::OwnerActivity::RunEnded(w::OwnerActivityRunEnded::new(
            limits,
            w::OwnerActivityRunEndedParts { attempt: *attempt, end: end(*value, limits)?, spent: *spent },
        )?),
        t::Line::Turn { reply, attempt, turn, read } => w::OwnerActivity::Turn(w::OwnerActivityTurn::new(
            limits,
            w::OwnerActivityTurnParts { reply: *reply, attempt: *attempt, turn: *turn, read: *read },
        )?),
        t::Line::Decided { revision } => w::OwnerActivity::Decided(w::OwnerActivityDecided::new(
            limits,
            w::OwnerActivityDecidedParts { revision: *revision },
        )?),
        t::Line::Kept { attempt, pending, question } => {
            let mut rows =
                skein_lib::List::with_capacity(u32::try_from(pending.len()).expect("bounded pending dependencies"));
            for number in pending {
                rows.push(*number).expect("same pending dependencies");
            }
            w::OwnerActivity::Kept(w::OwnerActivityKept::new(
                limits,
                w::OwnerActivityKeptParts { attempt: *attempt, pending: rows, question: *question },
            )?)
        }
        t::Line::Milestone { position } => w::OwnerActivity::Milestone(w::OwnerActivityMilestone::new(
            limits,
            w::OwnerActivityMilestoneParts { position: *position },
        )?),
    };
    w::OwnerActivityLine::new(
        limits,
        w::OwnerActivityLineParts { task: value.task, position: value.position, at: value.at.as_nanos(), line },
    )
}
fn why(value: t::Why, limits: &w::Limits) -> Result<w::OwnerRunWhy, w::Problem> {
    Ok(match value {
        t::Why::Made => w::OwnerRunWhy::Made,
        t::Why::Ready => w::OwnerRunWhy::Ready,
        t::Why::Message { message } => {
            w::OwnerRunWhy::Message(w::OwnerRunWhyMessage::new(limits, w::OwnerRunWhyMessageParts { message })?)
        }
        t::Why::Released => w::OwnerRunWhy::Released,
        t::Why::Retry { tries } => w::OwnerRunWhy::Retry(w::OwnerRunWhyRetry::new(
            limits,
            w::OwnerRunWhyRetryParts { tries: chips::tries(tries, limits)? },
        )?),
        t::Why::Narrowed => w::OwnerRunWhy::Narrowed,
        t::Why::Changed => w::OwnerRunWhy::Changed,
    })
}
fn end(value: t::EndKind, limits: &w::Limits) -> Result<w::OwnerRunEnd, w::Problem> {
    Ok(match value {
        t::EndKind::Parked => w::OwnerRunEnd::Parked,
        t::EndKind::Finished => w::OwnerRunEnd::Finished,
        t::EndKind::Cancelled => w::OwnerRunEnd::Cancelled,
        t::EndKind::Failed { class } => w::OwnerRunEnd::Failed(w::OwnerRunEndFailed::new(
            limits,
            w::OwnerRunEndFailedParts { class: chips::class(class, limits)? },
        )?),
        t::EndKind::Refused => w::OwnerRunEnd::Refused,
    })
}

pub fn page_turn(
    task: u64,
    value: &t::ActivityLine,
    supplied: Option<&reads::PageTurn>,
    limits: &w::Limits,
) -> Result<w::ActivityLine, w::Problem> {
    let invalid = w::Problem { path: w::Path::ActivityOwnerDecoded, reason: skein_codec::Reason::Tag };
    if value.task != task {
        return Err(invalid);
    }
    let decoded = match &value.line {
        t::Line::Turn { attempt, turn, .. } => {
            let supplied = supplied.ok_or(invalid)?;
            match supplied {
                reads::PageTurn::Carried(record) => {
                    if (record.task, record.attempt, record.turn) != (task, *attempt, *turn) {
                        return Err(invalid);
                    }
                    Some(archive(record, limits)?)
                }
                reads::PageTurn::TooLarge { spent, read, at, bytes } => Some(w::ArchivedTurn::new(
                    limits,
                    w::ArchivedTurnParts {
                        task,
                        name: w::TurnName::new(limits, w::TurnNameParts { attempt: *attempt, number: *turn })?,
                        cumulative: *spent,
                        read: *read,
                        at: at.as_nanos(),
                        turn: w::TurnBytes::TooLarge(w::TurnTooLarge::new(
                            limits,
                            w::TurnTooLargeParts { bytes: *bytes },
                        )?),
                    },
                )?),
                reads::PageTurn::Unread => None,
            }
        }
        t::Line::Words { .. }
        | t::Line::Heard(_)
        | t::Line::Notified { .. }
        | t::Line::RunBegan { .. }
        | t::Line::RunEnded { .. }
        | t::Line::Decided { .. }
        | t::Line::Kept { .. }
        | t::Line::Milestone { .. } => {
            if supplied.is_some() {
                return Err(invalid);
            }
            None
        }
    };
    w::ActivityLine::new(
        limits,
        w::ActivityLineParts {
            position: value.position,
            at: value.at.as_nanos(),
            event: w::Activity::Owner(w::ActivityOwner::new(
                limits,
                w::ActivityOwnerParts { source: line(value, limits)?, decoded },
            )?),
        },
    )
}

/// The same immutable archive serves a page and a single-turn read.
pub fn archive(record: &jig_domain_core::TurnRecord, limits: &w::Limits) -> Result<w::ArchivedTurn, w::Problem> {
    let path = w::Path::TurnCarriedBytes;
    if record.transcript.len()
        > usize::try_from(limits.turn_carried_bytes.min(w::CEILINGS.turn_carried_bytes)).expect("u32 fits usize")
    {
        return Err(w::Problem { path, reason: skein_codec::Reason::Bound });
    }
    w::ArchivedTurn::new(
        limits,
        w::ArchivedTurnParts {
            task: record.task,
            name: w::TurnName::new(limits, w::TurnNameParts { attempt: record.attempt, number: record.turn })?,
            cumulative: record.spent,
            read: record.read,
            at: record.at.as_nanos(),
            turn: w::TurnBytes::Carried(w::TurnCarried::new(
                limits,
                w::TurnCarriedParts { bytes: bytes::copy_of(&record.transcript) },
            )?),
        },
    )
}
