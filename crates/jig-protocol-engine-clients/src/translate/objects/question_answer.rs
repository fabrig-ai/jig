//! Whole single-read owner documents (protocol/client.md, 3 and 6).
use super::super::{owner, w};
use jig_domain_tasks as t;
use skein_lib::{List, bytes};
pub fn message_kind(value: &t::MessageKind, limits: &w::Limits) -> Result<w::OwnerMessageKind, w::Problem> {
    Ok(match value {
        t::MessageKind::Met { dependency, removed } => w::OwnerMessageKind::Met(w::OwnerMessageKindMet::new(
            limits,
            w::OwnerMessageKindMetParts { dependency: *dependency, removed: *removed },
        )?),
        t::MessageKind::SettingsDecision { proposal, outcome } => {
            w::OwnerMessageKind::SettingsDecision(w::OwnerMessageKindSettingsDecision::new(
                limits,
                w::OwnerMessageKindSettingsDecisionParts {
                    proposal: *proposal,
                    outcome: settings_outcome(*outcome, limits)?,
                },
            )?)
        }
        t::MessageKind::Words => w::OwnerMessageKind::Words,
        t::MessageKind::Question => w::OwnerMessageKind::Question,
        t::MessageKind::Escalation { task, revision } => {
            w::OwnerMessageKind::Escalation(w::OwnerMessageKindEscalation::new(
                limits,
                w::OwnerMessageKindEscalationParts { task: *task, revision: *revision },
            )?)
        }
        t::MessageKind::Proposal { proposer, proposal, kind } => {
            w::OwnerMessageKind::Proposal(w::OwnerMessageKindProposal::new(
                limits,
                w::OwnerMessageKindProposalParts {
                    proposer: *proposer,
                    proposal: *proposal,
                    kind: super::proposals::kind(*kind),
                },
            )?)
        }
        t::MessageKind::PersonStalled { task } => w::OwnerMessageKind::PersonStalled(
            w::OwnerMessageKindPersonStalled::new(limits, w::OwnerMessageKindPersonStalledParts { task: *task })?,
        ),
        t::MessageKind::ProposalDecision { proposal, accepted } => {
            w::OwnerMessageKind::ProposalDecision(w::OwnerMessageKindProposalDecision::new(
                limits,
                w::OwnerMessageKindProposalDecisionParts { proposal: *proposal, accepted: *accepted },
            )?)
        }
        t::MessageKind::EffectNotMade { entry } => w::OwnerMessageKind::EffectNotMade(
            w::OwnerMessageKindEffectNotMade::new(limits, w::OwnerMessageKindEffectNotMadeParts { entry: *entry })?,
        ),
        t::MessageKind::Amendment { revision } | t::MessageKind::FinishRequested { revision } => {
            w::OwnerMessageKind::Amendment(w::OwnerMessageKindAmendment::new(
                limits,
                w::OwnerMessageKindAmendmentParts { revision: *revision },
            )?)
        }
        t::MessageKind::Answer { question } => w::OwnerMessageKind::Answer(w::OwnerMessageKindAnswer::new(
            limits,
            w::OwnerMessageKindAnswerParts { question: *question },
        )?),
        t::MessageKind::Timer { subscription } => w::OwnerMessageKind::Timer(w::OwnerMessageKindTimer::new(
            limits,
            w::OwnerMessageKindTimerParts { subscription: *subscription },
        )?),
        t::MessageKind::Notice { subscription, target, state } => {
            w::OwnerMessageKind::Notice(w::OwnerMessageKindNotice::new(
                limits,
                w::OwnerMessageKindNoticeParts {
                    subscription: *subscription,
                    target: *target,
                    state: match state {
                        t::NoticeState::Held { .. } => w::OwnerNoticeState::Held,
                        t::NoticeState::Done => w::OwnerNoticeState::Done,
                        t::NoticeState::Failed => w::OwnerNoticeState::Failed,
                        t::NoticeState::Cancelled => w::OwnerNoticeState::Cancelled,
                    },
                },
            )?)
        }
        t::MessageKind::News { subscription, class } => w::OwnerMessageKind::News(w::OwnerMessageKindNews::new(
            limits,
            w::OwnerMessageKindNewsParts {
                subscription: *subscription,
                class: match class {
                    t::NewsClass::Wakes => w::OwnerNewsClass::Wakes,
                    t::NewsClass::Kept => w::OwnerNewsClass::Kept,
                    t::NewsClass::Dropped => w::OwnerNewsClass::Dropped,
                },
            },
        )?),
        t::MessageKind::Result(value) => w::OwnerMessageKind::Result(w::OwnerMessageKindResult::new(
            limits,
            w::OwnerMessageKindResultParts { value: word_result(value, limits)? },
        )?),
    })
}
pub fn word_result(value: &t::ResultKind, limits: &w::Limits) -> Result<w::OwnerWordResult, w::Problem> {
    Ok(match value {
        t::ResultKind::Report => w::OwnerWordResult::Report,
        t::ResultKind::Failed => w::OwnerWordResult::Failed,
        t::ResultKind::Cancelled => w::OwnerWordResult::Cancelled,
        t::ResultKind::Approved { state, words_present } => {
            w::OwnerWordResult::Approved(w::OwnerWordResultApproved::new(
                limits,
                w::OwnerWordResultApprovedParts { state: bytes::copy_of(state), words_present: *words_present },
            )?)
        }
        t::ResultKind::Change { connector, kind, resource } => {
            w::OwnerWordResult::Change(w::OwnerWordResultChange::new(
                limits,
                w::OwnerWordResultChangeParts { connector: *connector, kind: *kind, resource: *resource },
            )?)
        }
        t::ResultKind::Verdict { code, fields } => {
            let mut copied = List::with_capacity(super::super::count(fields.len())?);
            for row in fields {
                copied
                    .push(w::ResultField::new(
                        limits,
                        w::ResultFieldParts { name: bytes::copy_of(&row.name), value: bytes::copy_of(&row.value) },
                    )?)
                    .expect("supplied fields fit");
            }
            w::OwnerWordResult::Verdict(w::OwnerWordResultVerdict::new(
                limits,
                w::OwnerWordResultVerdictParts { code: *code, fields: copied },
            )?)
        }
    })
}
pub fn word(value: &t::Word, limits: &w::Limits) -> Result<w::OwnerWord, w::Problem> {
    w::OwnerWord::new(
        limits,
        w::OwnerWordParts {
            number: value.number,
            from: owner::requester(value.from, limits)?,
            kind: message_kind(&value.kind, limits)?,
            words: bytes::copy_of(&value.words),
            at: value.at.as_nanos(),
            hits: value.hits,
            eligible: value.eligible,
        },
    )
}
pub fn answer(value: &t::ActivityLine, limits: &w::Limits) -> Result<w::OwnerActivityLine, w::Problem> {
    let line = match &value.line {
        t::Line::Words { message, by, kind, words } => w::OwnerActivity::Words(w::OwnerActivityWords::new(
            limits,
            w::OwnerActivityWordsParts {
                message: *message,
                by: owner::requester(*by, limits)?,
                kind: message_kind(kind, limits)?,
                words: bytes::copy_of(words),
            },
        )?),
        t::Line::Heard(value) => w::OwnerActivity::Heard(w::OwnerActivityHeard::new(
            limits,
            w::OwnerActivityHeardParts { value: word(value, limits)? },
        )?),
        t::Line::Notified { recipient, word: value } => w::OwnerActivity::Notified(w::OwnerActivityNotified::new(
            limits,
            w::OwnerActivityNotifiedParts {
                recipient: owner::requester(*recipient, limits)?,
                word: word(value, limits)?,
            },
        )?),
        t::Line::RunBegan { .. }
        | t::Line::RunEnded { .. }
        | t::Line::Turn { .. }
        | t::Line::Decided { .. }
        | t::Line::Kept { .. }
        | t::Line::Milestone { .. } => {
            return Err(w::Problem {
                path: w::Path::OwnerHistoricalValueQuestionAnswer,
                reason: skein_codec::Reason::Tag,
            });
        }
    };
    w::OwnerActivityLine::new(
        limits,
        w::OwnerActivityLineParts { task: value.task, position: value.position, at: value.at.as_nanos(), line },
    )
}

fn settings_outcome(value: t::SettingsOutcome, limits: &w::Limits) -> Result<w::SettingsOutcome, w::Problem> {
    Ok(match value {
        t::SettingsOutcome::Accepted { revision } => {
            w::SettingsOutcome::Accepted(w::SettingsAccepted::new(limits, w::SettingsAcceptedParts { revision })?)
        }
        t::SettingsOutcome::Rejected => w::SettingsOutcome::Rejected,
        t::SettingsOutcome::Stale { newest } => {
            w::SettingsOutcome::Stale(w::SettingsStale::new(limits, w::SettingsStaleParts { newest })?)
        }
    })
}
