//! Whole pages and single documents (protocol/client.md, 3 and 6).
use super::{Limits, List, Wall, c, carried_turn_fields, text, too_large, w};
pub(crate) fn owner_message_proposal_kind_fields(
    value: &w::OwnerMessageProposalKind,
    _limits: &Limits,
) -> c::OwnerMessageProposalKind {
    match value {
        w::OwnerMessageProposalKind::Settings => c::OwnerMessageProposalKind::Settings,
        w::OwnerMessageProposalKind::Batch => c::OwnerMessageProposalKind::Batch,
        w::OwnerMessageProposalKind::Effect => c::OwnerMessageProposalKind::Effect,
        w::OwnerMessageProposalKind::Amend => c::OwnerMessageProposalKind::Amend,
        w::OwnerMessageProposalKind::Widen => c::OwnerMessageProposalKind::Widen,
        w::OwnerMessageProposalKind::Release => c::OwnerMessageProposalKind::Release,
    }
}

pub(crate) fn owner_notice_state_fields(value: &w::OwnerNoticeState, _limits: &Limits) -> c::OwnerNoticeState {
    match value {
        w::OwnerNoticeState::Held => c::OwnerNoticeState::Held,
        w::OwnerNoticeState::Done => c::OwnerNoticeState::Done,
        w::OwnerNoticeState::Failed => c::OwnerNoticeState::Failed,
        w::OwnerNoticeState::Cancelled => c::OwnerNoticeState::Cancelled,
    }
}

pub(crate) fn owner_news_class_fields(value: &w::OwnerNewsClass, _limits: &Limits) -> c::OwnerNewsClass {
    match value {
        w::OwnerNewsClass::Wakes => c::OwnerNewsClass::Wakes,
        w::OwnerNewsClass::Kept => c::OwnerNewsClass::Kept,
        w::OwnerNewsClass::Dropped => c::OwnerNewsClass::Dropped,
    }
}

pub(crate) fn owner_word_result_fields(
    value: &w::OwnerWordResult,
    limits: &Limits,
) -> Result<c::OwnerWordResult, w::Problem> {
    Ok(match value {
        w::OwnerWordResult::Approved(row) => {
            c::OwnerWordResult::Approved { state: text(row.state(), limits)?, words_present: row.words_present() }
        }
        w::OwnerWordResult::Report => c::OwnerWordResult::Report,
        w::OwnerWordResult::Verdict(row) => {
            if row.fields().len() > limits.client.rows {
                return Err(too_large());
            }
            let mut list0 = List::with_capacity(row.fields().len());
            for item in row.fields() {
                list0.push(crate::translate::facts::result_field(item, limits)).expect("reserved supplied list");
            }
            c::OwnerWordResult::Verdict { code: row.code(), fields: list0 }
        }
        w::OwnerWordResult::Change(row) => {
            c::OwnerWordResult::Change { connector: row.connector(), kind: row.kind(), resource: row.resource() }
        }
        w::OwnerWordResult::Failed => c::OwnerWordResult::Failed,
        w::OwnerWordResult::Cancelled => c::OwnerWordResult::Cancelled,
    })
}

pub(crate) fn owner_message_kind_fields(
    value: &w::OwnerMessageKind,
    limits: &Limits,
) -> Result<c::OwnerMessageKind, w::Problem> {
    Ok(match value {
        w::OwnerMessageKind::SettingsDecision(row) => c::OwnerMessageKind::SettingsDecision {
            proposal: row.proposal(),
            outcome: match row.outcome() {
                w::SettingsOutcome::Accepted(value) => c::SettingsOutcome::Accepted { revision: value.revision() },
                w::SettingsOutcome::Rejected => c::SettingsOutcome::Rejected,
                w::SettingsOutcome::Stale(value) => c::SettingsOutcome::Stale { newest: value.newest() },
            },
        },
        w::OwnerMessageKind::Escalation(row) => {
            c::OwnerMessageKind::Escalation { task: row.task(), revision: row.revision() }
        }
        w::OwnerMessageKind::Proposal(row) => c::OwnerMessageKind::Proposal {
            proposer: row.proposer(),
            proposal: row.proposal(),
            kind: owner_message_proposal_kind_fields(row.kind(), limits),
        },
        w::OwnerMessageKind::PersonStalled(row) => c::OwnerMessageKind::PersonStalled { task: row.task() },
        w::OwnerMessageKind::ProposalDecision(row) => {
            c::OwnerMessageKind::ProposalDecision { proposal: row.proposal(), accepted: row.accepted() }
        }
        w::OwnerMessageKind::Met(row) => {
            c::OwnerMessageKind::Met { dependency: row.dependency(), removed: row.removed() }
        }
        w::OwnerMessageKind::QuestionAnswered(row) => {
            c::OwnerMessageKind::QuestionAnswered { question: row.question() }
        }
        w::OwnerMessageKind::QuestionEnded(row) => c::OwnerMessageKind::QuestionEnded { question: row.question() },
        w::OwnerMessageKind::Words => c::OwnerMessageKind::Words,
        w::OwnerMessageKind::EffectNotMade(row) => c::OwnerMessageKind::EffectNotMade { entry: row.entry() },
        w::OwnerMessageKind::Amendment(row) => c::OwnerMessageKind::Amendment { revision: row.revision() },
        w::OwnerMessageKind::Question => c::OwnerMessageKind::Question,
        w::OwnerMessageKind::Answer(row) => c::OwnerMessageKind::Answer { question: row.question() },
        w::OwnerMessageKind::Notice(row) => c::OwnerMessageKind::Notice {
            subscription: row.subscription(),
            target: row.target(),
            state: owner_notice_state_fields(row.state(), limits),
        },
        w::OwnerMessageKind::Timer(row) => c::OwnerMessageKind::Timer { subscription: row.subscription() },
        w::OwnerMessageKind::News(row) => c::OwnerMessageKind::News {
            subscription: row.subscription(),
            class: owner_news_class_fields(row.class(), limits),
        },
        w::OwnerMessageKind::Result(row) => c::OwnerMessageKind::Result(owner_word_result_fields(row.value(), limits)?),
    })
}

pub(crate) fn owner_word_fields(value: &w::OwnerWord, limits: &Limits) -> Result<c::OwnerWord, w::Problem> {
    Ok(c::OwnerWord {
        number: value.number(),
        from: crate::translate::facts::owner_requester(value.from(), limits),
        kind: owner_message_kind_fields(value.kind(), limits)?,
        words: text(value.words(), limits)?,
        at: Wall::from_nanos(value.at()),
        hits: value.hits(),
        eligible: value.eligible(),
    })
}

pub(crate) fn owner_run_why_fields(value: &w::OwnerRunWhy, limits: &Limits) -> c::OwnerRunWhy {
    match value {
        w::OwnerRunWhy::Made => c::OwnerRunWhy::Made,
        w::OwnerRunWhy::Ready => c::OwnerRunWhy::Ready,
        w::OwnerRunWhy::Message(row) => c::OwnerRunWhy::Message { message: row.message() },
        w::OwnerRunWhy::Released => c::OwnerRunWhy::Released,
        w::OwnerRunWhy::Retry(row) => {
            c::OwnerRunWhy::Retry { tries: crate::translate::facts::owner_failure_tries(row.tries(), limits) }
        }
        w::OwnerRunWhy::Narrowed => c::OwnerRunWhy::Narrowed,
        w::OwnerRunWhy::Changed => c::OwnerRunWhy::Changed,
    }
}

pub(crate) fn owner_run_end_fields(value: &w::OwnerRunEnd, limits: &Limits) -> c::OwnerRunEnd {
    match value {
        w::OwnerRunEnd::Parked => c::OwnerRunEnd::Parked,
        w::OwnerRunEnd::Finished => c::OwnerRunEnd::Finished,
        w::OwnerRunEnd::Cancelled => c::OwnerRunEnd::Cancelled,
        w::OwnerRunEnd::Failed(row) => {
            c::OwnerRunEnd::Failed { class: crate::translate::answers::hold_failure(row.class(), limits) }
        }
        w::OwnerRunEnd::Refused => c::OwnerRunEnd::Refused,
    }
}

#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn owner_activity_fields(value: &w::OwnerActivity, limits: &Limits) -> Result<c::OwnerActivity, w::Problem> {
    Ok(match value {
        w::OwnerActivity::Words(row) => c::OwnerActivity::Words {
            message: row.message(),
            by: crate::translate::facts::owner_requester(row.by(), limits),
            kind: owner_message_kind_fields(row.kind(), limits)?,
            words: text(row.words(), limits)?,
        },
        w::OwnerActivity::Heard(row) => c::OwnerActivity::Heard(owner_word_fields(row.value(), limits)?),
        w::OwnerActivity::Notified(row) => c::OwnerActivity::Notified {
            recipient: crate::translate::facts::owner_requester(row.recipient(), limits),
            word: owner_word_fields(row.word(), limits)?,
        },
        w::OwnerActivity::RunBegan(row) => c::OwnerActivity::RunBegan {
            attempt: row.attempt(),
            why: owner_run_why_fields(row.why(), limits),
            resumed: row.resumed(),
        },
        w::OwnerActivity::RunEnded(row) => c::OwnerActivity::RunEnded {
            attempt: row.attempt(),
            end: owner_run_end_fields(row.end(), limits),
            spent: row.spent(),
        },
        w::OwnerActivity::Turn(row) => {
            let opt0 = match row.read() {
                Some(item) => Some(*item),
                None => None,
            };
            c::OwnerActivity::Turn { reply: row.reply(), attempt: row.attempt(), turn: row.turn(), read: opt0 }
        }
        w::OwnerActivity::Decided(row) => c::OwnerActivity::Decided { revision: row.revision() },
        w::OwnerActivity::Kept(row) => {
            c::OwnerActivity::Kept { attempt: row.attempt(), pending: row.pending().clone(), question: *row.question() }
        }
        w::OwnerActivity::Milestone(row) => c::OwnerActivity::Milestone { position: row.position() },
    })
}

pub(crate) fn owner_activity_line_fields(
    value: &w::OwnerActivityLine,
    limits: &Limits,
) -> Result<c::OwnerActivityLine, w::Problem> {
    Ok(c::OwnerActivityLine {
        task: value.task(),
        position: value.position(),
        at: Wall::from_nanos(value.at()),
        line: owner_activity_fields(value.line(), limits)?,
    })
}

#[expect(clippy::manual_map, reason = "step code matches optional fields without closures")]
pub(crate) fn archived_turn_fields(value: &w::ArchivedTurn, limits: &Limits) -> c::ArchivedTurn {
    let opt0 = match value.read() {
        Some(item) => Some(*item),
        None => None,
    };
    c::ArchivedTurn {
        task: value.task(),
        name: turn_name_fields(value.name(), limits),
        cumulative: value.cumulative(),
        read: opt0,
        at: Wall::from_nanos(value.at()),
        turn: match value.turn() {
            w::TurnBytes::Carried(row) => carried_turn_fields(row.bytes(), limits),
            w::TurnBytes::TooLarge(row) => {
                c::TurnRead::Undecodable { why: c::TurnProblem::TooLarge { bytes: row.bytes() } }
            }
        },
    }
}

pub(crate) fn turn_name_fields(value: &w::TurnName, _limits: &Limits) -> c::TurnName {
    c::TurnName { attempt: value.attempt(), number: value.number() }
}
