//! Durable task values translated exhaustively (protocol/store.md, 5.1).
use super::{proposals, value};
use crate::tasks_v9 as wire;
use jig_domain_tasks as domain;
use skein_lib::Wall;

/// Translate `NoticeState` to its durable tag.
pub fn notice_state(source: domain::NoticeState, limits: &wire::Limits) -> Result<wire::NoticeState, wire::Problem> {
    Ok(match source {
        domain::NoticeState::Held { revision } => {
            wire::NoticeState::Held(wire::NoticeHeld::new(limits, wire::NoticeHeldParts { revision })?)
        }
        domain::NoticeState::Done => wire::NoticeState::Done,
        domain::NoticeState::Failed => wire::NoticeState::Failed,
        domain::NoticeState::Cancelled => wire::NoticeState::Cancelled,
    })
}
/// Read `NoticeState` from its durable tag.
#[must_use]
pub fn read_notice_state(source: wire::NoticeState) -> domain::NoticeState {
    match source {
        wire::NoticeState::Held(held) => domain::NoticeState::Held { revision: held.revision() },
        wire::NoticeState::Done => domain::NoticeState::Done,
        wire::NoticeState::Failed => domain::NoticeState::Failed,
        wire::NoticeState::Cancelled => domain::NoticeState::Cancelled,
    }
}

/// Translate `NewsClass` to its durable tag.
#[must_use]
pub fn news_class(source: domain::NewsClass) -> wire::NewsClass {
    match source {
        domain::NewsClass::Wakes => wire::NewsClass::Wakes,
        domain::NewsClass::Kept => wire::NewsClass::Kept,
        domain::NewsClass::Dropped => wire::NewsClass::Dropped,
    }
}
/// Read `NewsClass` from its durable tag.
#[must_use]
pub fn read_news_class(source: wire::NewsClass) -> domain::NewsClass {
    match source {
        wire::NewsClass::Wakes => domain::NewsClass::Wakes,
        wire::NewsClass::Kept => domain::NewsClass::Kept,
        wire::NewsClass::Dropped => domain::NewsClass::Dropped,
    }
}

/// Translate `ResultKind` to its durable value.
pub fn result_kind(source: &domain::ResultKind, limits: &wire::Limits) -> Result<wire::ResultKind, wire::Problem> {
    Ok(match source {
        domain::ResultKind::Approved { state, words_present } => {
            wire::ResultKind::Approved(wire::ApprovalMessage::new(
                limits,
                wire::ApprovalMessageParts { state: state.clone(), words_present: *words_present },
            )?)
        }
        domain::ResultKind::Report => wire::ResultKind::Report,
        domain::ResultKind::Verdict { code, fields } => wire::ResultKind::Verdict(wire::VerdictMessage::new(
            limits,
            wire::VerdictMessageParts {
                code: *code,
                fields: super::pieces::fields(
                    fields,
                    limits,
                    limits.verdict_message_fields,
                    wire::Path::VerdictMessageFields,
                )?,
            },
        )?),
        domain::ResultKind::Change { connector, kind, resource } => wire::ResultKind::Change(wire::ChangeMessage::new(
            limits,
            wire::ChangeMessageParts { connector: *connector, kind: *kind, resource: *resource },
        )?),
        domain::ResultKind::Failed => wire::ResultKind::Failed,
        domain::ResultKind::Cancelled => wire::ResultKind::Cancelled,
    })
}
/// Read `ResultKind` from its durable value.
pub fn read_result_kind(source: wire::ResultKind, limits: &wire::Limits) -> Result<domain::ResultKind, wire::Problem> {
    Ok(match source {
        wire::ResultKind::Approved(source) => {
            let wire::ApprovalMessageParts { state, words_present } = source.into_parts();
            domain::ResultKind::Approved { state, words_present }
        }
        wire::ResultKind::Report => domain::ResultKind::Report,
        wire::ResultKind::Verdict(source) => {
            let wire::VerdictMessageParts { code, fields } = source.into_parts();
            domain::ResultKind::Verdict {
                code,
                fields: super::pieces::read_fields(fields, limits, wire::Path::VerdictMessageFields)?,
            }
        }
        wire::ResultKind::Change(source) => {
            let wire::ChangeMessageParts { connector, kind, resource } = source.into_parts();
            domain::ResultKind::Change { connector, kind, resource }
        }
        wire::ResultKind::Failed => domain::ResultKind::Failed,
        wire::ResultKind::Cancelled => domain::ResultKind::Cancelled,
    })
}

/// Translate `MessageKind` to its durable value.
pub fn message_kind(source: &domain::MessageKind, limits: &wire::Limits) -> Result<wire::MessageKind, wire::Problem> {
    Ok(match source {
        domain::MessageKind::Escalation { task, revision } => wire::MessageKind::Escalation(
            wire::EscalationMessage::new(limits, wire::EscalationMessageParts { task: *task, revision: *revision })?,
        ),
        domain::MessageKind::Proposal { proposer, proposal, kind } => {
            wire::MessageKind::Proposal(wire::ProposalMessage::new(
                limits,
                wire::ProposalMessageParts {
                    proposer: *proposer,
                    proposal: *proposal,
                    kind: proposals::kind(kind, limits)?,
                },
            )?)
        }
        domain::MessageKind::PersonStalled { task } => wire::MessageKind::PersonStalled(wire::StalledMessage::new(
            limits,
            wire::StalledMessageParts { task: *task },
        )?),
        domain::MessageKind::SettingsDecision { proposal, outcome } => {
            wire::MessageKind::SettingsDecision(wire::SettingsDecisionMessage::new(
                limits,
                wire::SettingsDecisionMessageParts {
                    proposal: *proposal,
                    outcome: settings_outcome(*outcome, limits)?,
                },
            )?)
        }
        domain::MessageKind::ProposalDecision { proposal, accepted } => {
            wire::MessageKind::ProposalDecision(wire::DecisionMessage::new(
                limits,
                wire::DecisionMessageParts { proposal: *proposal, accepted: *accepted },
            )?)
        }
        domain::MessageKind::QuestionAnswered { question } => wire::MessageKind::QuestionAnswered(
            wire::AnswerMessage::new(limits, wire::AnswerMessageParts { question: *question })?,
        ),
        domain::MessageKind::QuestionEnded { question } => wire::MessageKind::QuestionEnded(wire::AnswerMessage::new(
            limits,
            wire::AnswerMessageParts { question: *question },
        )?),
        domain::MessageKind::Words => wire::MessageKind::Words,
        domain::MessageKind::EffectNotMade { entry } => wire::MessageKind::EffectNotMade(wire::EffectMessage::new(
            limits,
            wire::EffectMessageParts { entry: *entry },
        )?),
        domain::MessageKind::Amendment { revision } => wire::MessageKind::Amendment(wire::AmendmentMessage::new(
            limits,
            wire::AmendmentMessageParts { revision: *revision },
        )?),
        domain::MessageKind::FinishRequested { revision } => wire::MessageKind::FinishRequested(
            wire::AmendmentMessage::new(limits, wire::AmendmentMessageParts { revision: *revision })?,
        ),
        domain::MessageKind::Met { dependency, removed } => wire::MessageKind::Met(wire::MetMessage::new(
            limits,
            wire::MetMessageParts { dependency: *dependency, removed: *removed },
        )?),
        domain::MessageKind::Question => wire::MessageKind::Question,
        domain::MessageKind::Answer { question } => wire::MessageKind::Answer(wire::AnswerMessage::new(
            limits,
            wire::AnswerMessageParts { question: *question },
        )?),
        domain::MessageKind::Notice { subscription, target, state } => {
            wire::MessageKind::Notice(wire::NoticeMessage::new(
                limits,
                wire::NoticeMessageParts {
                    subscription: *subscription,
                    target: *target,
                    state: notice_state(*state, limits)?,
                },
            )?)
        }
        domain::MessageKind::Timer { subscription } => wire::MessageKind::Timer(wire::TimerMessage::new(
            limits,
            wire::TimerMessageParts { subscription: *subscription },
        )?),
        domain::MessageKind::News { subscription, class } => wire::MessageKind::News(wire::NewsMessage::new(
            limits,
            wire::NewsMessageParts { subscription: *subscription, class: news_class(*class) },
        )?),
        domain::MessageKind::Result(kind) => wire::MessageKind::Result(wire::ResultMessage::new(
            limits,
            wire::ResultMessageParts { kind: result_kind(kind, limits)? },
        )?),
    })
}
/// Read `MessageKind` from its durable value.
pub fn read_message_kind(
    source: wire::MessageKind,
    limits: &wire::Limits,
) -> Result<domain::MessageKind, wire::Problem> {
    Ok(match source {
        wire::MessageKind::Escalation(source) => {
            let wire::EscalationMessageParts { task, revision } = source.into_parts();
            domain::MessageKind::Escalation { task, revision }
        }
        wire::MessageKind::Proposal(source) => {
            let wire::ProposalMessageParts { proposer, proposal, kind } = source.into_parts();
            domain::MessageKind::Proposal { proposer, proposal, kind: proposals::read_kind(kind, limits)? }
        }
        wire::MessageKind::PersonStalled(source) => {
            let wire::StalledMessageParts { task } = source.into_parts();
            domain::MessageKind::PersonStalled { task }
        }
        wire::MessageKind::SettingsDecision(source) => {
            let source = source.into_parts();
            domain::MessageKind::SettingsDecision {
                proposal: source.proposal,
                outcome: read_settings_outcome(source.outcome),
            }
        }
        wire::MessageKind::ProposalDecision(source) => {
            let wire::DecisionMessageParts { proposal, accepted } = source.into_parts();
            domain::MessageKind::ProposalDecision { proposal, accepted }
        }
        wire::MessageKind::QuestionAnswered(source) => {
            domain::MessageKind::QuestionAnswered { question: source.question() }
        }
        wire::MessageKind::QuestionEnded(source) => domain::MessageKind::QuestionEnded { question: source.question() },
        wire::MessageKind::Words => domain::MessageKind::Words,
        wire::MessageKind::EffectNotMade(source) => {
            let wire::EffectMessageParts { entry } = source.into_parts();
            domain::MessageKind::EffectNotMade { entry }
        }
        wire::MessageKind::Amendment(source) => {
            let wire::AmendmentMessageParts { revision } = source.into_parts();
            domain::MessageKind::Amendment { revision }
        }
        wire::MessageKind::FinishRequested(source) => {
            let wire::AmendmentMessageParts { revision } = source.into_parts();
            domain::MessageKind::FinishRequested { revision }
        }
        wire::MessageKind::Met(source) => {
            let wire::MetMessageParts { dependency, removed } = source.into_parts();
            domain::MessageKind::Met { dependency, removed }
        }
        wire::MessageKind::Question => domain::MessageKind::Question,
        wire::MessageKind::Answer(source) => {
            let wire::AnswerMessageParts { question } = source.into_parts();
            domain::MessageKind::Answer { question }
        }
        wire::MessageKind::Notice(source) => {
            let wire::NoticeMessageParts { subscription, target, state } = source.into_parts();
            domain::MessageKind::Notice { subscription, target, state: read_notice_state(state) }
        }
        wire::MessageKind::Timer(source) => {
            let wire::TimerMessageParts { subscription } = source.into_parts();
            domain::MessageKind::Timer { subscription }
        }
        wire::MessageKind::News(source) => {
            let wire::NewsMessageParts { subscription, class } = source.into_parts();
            domain::MessageKind::News { subscription, class: read_news_class(class) }
        }
        wire::MessageKind::Result(source) => {
            let wire::ResultMessageParts { kind } = source.into_parts();
            domain::MessageKind::Result(read_result_kind(kind, limits)?)
        }
    })
}

/// Translate `Word` to its durable record.
pub fn word(source: &domain::Word, limits: &wire::Limits) -> Result<wire::Word, wire::Problem> {
    let domain::Word { number, from, kind, words, at, hits, eligible } = source;
    wire::Word::new(
        limits,
        wire::WordParts {
            number: *number,
            from: value::party(from, limits)?,
            kind: message_kind(kind, limits)?,
            words: words.clone(),
            at: at.as_nanos(),
            hits: *hits,
            eligible: *eligible,
        },
    )
}
/// Read `Word` from its durable record.
pub fn read_word(source: wire::Word, limits: &wire::Limits) -> Result<domain::Word, wire::Problem> {
    let wire::WordParts { number, from, kind, words, at, hits, eligible } = source.into_parts();
    Ok(domain::Word {
        number,
        from: value::read_party(from),
        kind: read_message_kind(kind, limits)?,
        words,
        at: Wall::from_nanos(at),
        hits,
        eligible,
    })
}

/// Translate `EndKind` to its durable value.
pub fn end_kind(source: &domain::EndKind, limits: &wire::Limits) -> Result<wire::EndKind, wire::Problem> {
    Ok(match source {
        domain::EndKind::Parked => wire::EndKind::Parked,
        domain::EndKind::Finished => wire::EndKind::Finished,
        domain::EndKind::Cancelled => wire::EndKind::Cancelled,
        domain::EndKind::Failed { class } => {
            wire::EndKind::Failed(wire::FailedEnd::new(limits, wire::FailedEndParts { class: value::class(*class) })?)
        }
        domain::EndKind::Refused => wire::EndKind::Refused,
    })
}
/// Read `EndKind` from its durable value.
pub fn read_end_kind(source: wire::EndKind, _limits: &wire::Limits) -> Result<domain::EndKind, wire::Problem> {
    Ok(match source {
        wire::EndKind::Parked => domain::EndKind::Parked,
        wire::EndKind::Finished => domain::EndKind::Finished,
        wire::EndKind::Cancelled => domain::EndKind::Cancelled,
        wire::EndKind::Failed(source) => {
            let wire::FailedEndParts { class } = source.into_parts();
            domain::EndKind::Failed { class: value::read_class(class) }
        }
        wire::EndKind::Refused => domain::EndKind::Refused,
    })
}

/// Translate `Line` to its durable value.
pub fn line(source: &domain::Line, limits: &wire::Limits) -> Result<wire::Line, wire::Problem> {
    Ok(match source {
        domain::Line::Words { message, by, kind, words } => wire::Line::Words(wire::ActivityWords::new(
            limits,
            wire::ActivityWordsParts {
                message: *message,
                by: value::party(by, limits)?,
                kind: message_kind(kind, limits)?,
                words: words.clone(),
            },
        )?),
        domain::Line::Heard(message) => wire::Line::Heard(wire::HeardMessage::new(
            limits,
            wire::HeardMessageParts { word: word(message, limits)? },
        )?),
        domain::Line::Notified { recipient, word: message } => wire::Line::Notified(wire::NotifiedMessage::new(
            limits,
            wire::NotifiedMessageParts { recipient: value::party(recipient, limits)?, word: word(message, limits)? },
        )?),
        domain::Line::RunBegan { attempt, why, resumed } => wire::Line::RunBegan(wire::BeganLine::new(
            limits,
            wire::BeganLineParts { attempt: *attempt, why: value::why(*why, limits)?, resumed: *resumed },
        )?),
        domain::Line::RunEnded { attempt, end, spent } => wire::Line::RunEnded(wire::EndedLine::new(
            limits,
            wire::EndedLineParts { attempt: *attempt, end: end_kind(end, limits)?, spent: *spent },
        )?),
        domain::Line::Turn { reply, attempt, turn, read } => wire::Line::Turn(wire::TurnLine::new(
            limits,
            wire::TurnLineParts { reply: *reply, attempt: *attempt, turn: *turn, read: *read },
        )?),
        domain::Line::Decided { revision } => {
            wire::Line::Decided(wire::DecidedLine::new(limits, wire::DecidedLineParts { revision: *revision })?)
        }
        domain::Line::Kept { attempt, pending, question } => wire::Line::Kept(wire::KeptLine::new(
            limits,
            wire::KeptLineParts {
                attempt: *attempt,
                question: *question,
                pending: super::phase::references(
                    pending,
                    limits.kept_line_pending,
                    wire::Path::KeptLinePending,
                    limits,
                )?,
            },
        )?),
        domain::Line::Milestone { position } => {
            wire::Line::Milestone(wire::MilestoneLine::new(limits, wire::MilestoneLineParts { position: *position })?)
        }
    })
}
/// Read `Line` from its durable value.
pub fn read_line(source: wire::Line, limits: &wire::Limits) -> Result<domain::Line, wire::Problem> {
    Ok(match source {
        wire::Line::Words(source) => {
            let wire::ActivityWordsParts { message, by, kind, words } = source.into_parts();
            domain::Line::Words { message, by: value::read_party(by), kind: read_message_kind(kind, limits)?, words }
        }
        wire::Line::Heard(source) => {
            let wire::HeardMessageParts { word } = source.into_parts();
            domain::Line::Heard(read_word(word, limits)?)
        }
        wire::Line::Notified(source) => {
            let wire::NotifiedMessageParts { recipient, word } = source.into_parts();
            domain::Line::Notified { recipient: value::read_party(recipient), word: read_word(word, limits)? }
        }
        wire::Line::RunBegan(source) => {
            let wire::BeganLineParts { attempt, why, resumed } = source.into_parts();
            domain::Line::RunBegan { attempt, why: value::read_why(why), resumed }
        }
        wire::Line::RunEnded(source) => {
            let wire::EndedLineParts { attempt, end, spent } = source.into_parts();
            domain::Line::RunEnded { attempt, end: read_end_kind(end, limits)?, spent }
        }
        wire::Line::Turn(source) => {
            let wire::TurnLineParts { reply, attempt, turn, read } = source.into_parts();
            domain::Line::Turn { reply, attempt, turn, read }
        }
        wire::Line::Decided(source) => {
            let wire::DecidedLineParts { revision } = source.into_parts();
            domain::Line::Decided { revision }
        }
        wire::Line::Kept(source) => {
            let wire::KeptLineParts { attempt, pending, question } = source.into_parts();
            domain::Line::Kept { attempt, pending: super::phase::read_references(pending), question }
        }
        wire::Line::Milestone(source) => {
            let wire::MilestoneLineParts { position } = source.into_parts();
            domain::Line::Milestone { position }
        }
    })
}

/// Translate `ActivityLine` to its durable record.
pub fn activity_line(
    source: &domain::ActivityLine,
    limits: &wire::Limits,
) -> Result<wire::ActivityLine, wire::Problem> {
    let domain::ActivityLine { task, position, at, line: activity } = source;
    wire::ActivityLine::new(
        limits,
        wire::ActivityLineParts { task: *task, position: *position, at: at.as_nanos(), line: line(activity, limits)? },
    )
}
/// Read `ActivityLine` from its durable record.
pub fn read_activity_line(
    source: wire::ActivityLine,
    limits: &wire::Limits,
) -> Result<domain::ActivityLine, wire::Problem> {
    let wire::ActivityLineParts { task, position, at, line } = source.into_parts();
    Ok(domain::ActivityLine { task, position, at: Wall::from_nanos(at), line: read_line(line, limits)? })
}

fn settings_outcome(
    value: domain::SettingsOutcome,
    limits: &wire::Limits,
) -> Result<wire::SettingsOutcome, wire::Problem> {
    Ok(match value {
        domain::SettingsOutcome::Accepted { revision } => wire::SettingsOutcome::Accepted(
            wire::SettingsAcceptedRevision::new(limits, wire::SettingsAcceptedRevisionParts { revision })?,
        ),
        domain::SettingsOutcome::Rejected => wire::SettingsOutcome::Rejected,
        domain::SettingsOutcome::Stale { newest } => wire::SettingsOutcome::Stale(wire::SettingsStaleRevision::new(
            limits,
            wire::SettingsStaleRevisionParts { newest },
        )?),
    })
}
fn read_settings_outcome(value: wire::SettingsOutcome) -> domain::SettingsOutcome {
    match value {
        wire::SettingsOutcome::Accepted(value) => domain::SettingsOutcome::Accepted { revision: value.revision() },
        wire::SettingsOutcome::Rejected => domain::SettingsOutcome::Rejected,
        wire::SettingsOutcome::Stale(value) => domain::SettingsOutcome::Stale { newest: value.newest() },
    }
}
