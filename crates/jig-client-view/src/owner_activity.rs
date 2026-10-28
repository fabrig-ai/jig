//! Standalone immutable activity facts (client/domain/objects.md, 10;
//! contract.md, 5 and 7). Archive references stay references; no decoded turn,
//! sender name, fresh tail or run failure explanation is synthesized.
use crate::owner_activity_words::ActivityPhrase as P;
use crate::words::Words;
use skein_ui::phrases::Hole;
use skein_ui::{Builder, Role, Run};

use crate::Limits;
use jig_client::contract::{
    HoldFailure, OwnerActivity, OwnerActivityLine, OwnerFailureTries, OwnerMessageKind, OwnerMessageProposalKind,
    OwnerNewsClass, OwnerNoticeState, OwnerRequester, OwnerRunEnd, OwnerRunWhy, OwnerWord, OwnerWordResult, Text,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActivityFragment {
    pub nodes: u32,
    pub text_bytes: u32,
    pub text_per_node: u32,
    pub depth: u32,
    pub bindings: u32,
    pub slots: u32,
}
fn result_fits(result: &OwnerWordResult, words: &Text, limits: &Limits) -> Option<()> {
    match result {
        OwnerWordResult::Approved { state, words_present } => {
            if !state.fits(limits.text) || !words_present && !words.bytes().is_empty() {
                return None;
            }
        }
        OwnerWordResult::Verdict { fields, .. } => {
            if fields.capacity() > limits.rows {
                return None;
            }
            for field in fields {
                if u32::try_from(field.name.len()).ok()? > limits.text
                    || u32::try_from(field.value.len()).ok()? > limits.text
                {
                    return None;
                }
            }
        }
        OwnerWordResult::Report
        | OwnerWordResult::Change { .. }
        | OwnerWordResult::Failed
        | OwnerWordResult::Cancelled => {}
    }
    Some(())
}
fn message_fits(kind: &OwnerMessageKind, words: &Text, limits: &Limits) -> Option<()> {
    if !words.fits(limits.text) {
        return None;
    }
    match kind {
        OwnerMessageKind::Result(result) => result_fits(result, words, limits)?,
        OwnerMessageKind::Escalation { .. }
        | OwnerMessageKind::Proposal { .. }
        | OwnerMessageKind::PersonStalled { .. }
        | OwnerMessageKind::SettingsDecision { .. }
        | OwnerMessageKind::ProposalDecision { .. }
        | OwnerMessageKind::Words
        | OwnerMessageKind::EffectNotMade { .. }
        | OwnerMessageKind::Met { .. }
        | OwnerMessageKind::QuestionAnswered { .. }
        | OwnerMessageKind::QuestionEnded { .. }
        | OwnerMessageKind::Amendment { .. }
        | OwnerMessageKind::Question
        | OwnerMessageKind::Answer { .. }
        | OwnerMessageKind::Notice { .. }
        | OwnerMessageKind::Timer { .. }
        | OwnerMessageKind::News { .. } => {}
    }
    Some(())
}
/// Whole message envelope, refusing every nested excess and invalid Approved presence.
#[must_use]
pub fn word_required(value: &OwnerWord, limits: &Limits, words: &Words) -> Option<ActivityFragment> {
    message_fits(&value.kind, &value.words, limits)?;
    worst_case(limits, words)
}
/// Whole activity envelope. Text and field arrays are never shortened to fit.
#[must_use]
pub fn required(value: &OwnerActivityLine, limits: &Limits, words: &Words) -> Option<ActivityFragment> {
    match &value.line {
        OwnerActivity::Words { kind, words, .. } => message_fits(kind, words, limits)?,
        OwnerActivity::Heard(word) | OwnerActivity::Notified { word, .. } => {
            message_fits(&word.kind, &word.words, limits)?;
        }
        OwnerActivity::Kept { pending, .. } => {
            if pending.capacity() > limits.rows {
                return None;
            }
        }
        OwnerActivity::RunBegan { .. }
        | OwnerActivity::RunEnded { .. }
        | OwnerActivity::Turn { .. }
        | OwnerActivity::Decided { .. }
        | OwnerActivity::Milestone { .. } => {}
    }
    worst_case(limits, words)
}
/// Finite standalone envelope includes every field label, opaque byte and region name.
#[must_use]
pub fn worst_case(limits: &Limits, words: &Words) -> Option<ActivityFragment> {
    let nodes = 100_u32.checked_add(limits.rows.checked_mul(4)?)?;
    let saying = words.jig_capacity()?;
    let text_per_node = crate::literal_segment::max_bytes(limits.text)?.max(saying);
    Some(ActivityFragment {
        nodes,
        text_bytes: nodes.checked_mul(text_per_node)?,
        text_per_node,
        depth: 4,
        bindings: 0,
        slots: 0,
    })
}
fn say(b: &mut Builder, words: &Words, phrase: P, holes: &[Hole]) {
    let value =
        words.book().say(words.language(), phrase.id(), holes).expect("checked source activity sayings and holes");
    b.text(Run::Plain, &value.text);
}
fn caption(b: &mut Builder, words: &Words, phrase: P) {
    say(b, words, phrase, &[]);
}
fn number(b: &mut Builder, words: &Words, phrase: P, value: u64) {
    say(b, words, phrase, &[Hole::Number(value)]);
}
fn region(b: &mut Builder, words: &Words, phrase: P) {
    b.open(Role::Region);
    let value = words.book().say(words.language(), phrase.id(), &[]).expect("checked activity region name");
    b.name(&value.text);
}
fn identity(b: &mut Builder, value: OwnerRequester, words: &Words) {
    match value {
        OwnerRequester::Task(value) => number(b, words, P::TaskIdentity, value),
        OwnerRequester::Person(value) => number(b, words, P::PersonIdentity, value),
        OwnerRequester::Deployment { project } => number(b, words, P::DeploymentIdentity, u64::from(project)),
    }
}
fn proposal_kind(b: &mut Builder, value: OwnerMessageProposalKind, words: &Words) {
    let phrase = match value {
        OwnerMessageProposalKind::Settings => return,
        OwnerMessageProposalKind::Batch => P::Batch,
        OwnerMessageProposalKind::Effect => P::Effect,
        OwnerMessageProposalKind::Amend => P::Amend,
        OwnerMessageProposalKind::Widen => P::Widen,
        OwnerMessageProposalKind::Release => P::Release,
    };
    caption(b, words, phrase);
}
fn notice(b: &mut Builder, value: OwnerNoticeState, words: &Words) {
    let phrase = match value {
        OwnerNoticeState::Held => P::Held,
        OwnerNoticeState::Done => P::Done,
        OwnerNoticeState::Failed => P::NoticeFailed,
        OwnerNoticeState::Cancelled => P::NoticeCancelled,
    };
    caption(b, words, phrase);
}
fn news(b: &mut Builder, value: OwnerNewsClass, words: &Words) {
    let phrase = match value {
        OwnerNewsClass::Wakes => P::Wakes,
        OwnerNewsClass::Kept => P::Kept,
        OwnerNewsClass::Dropped => P::Dropped,
    };
    caption(b, words, phrase);
}
fn result(b: &mut Builder, value: &OwnerWordResult, words: &Words) {
    match value {
        OwnerWordResult::Approved { state, words_present } => {
            caption(b, words, P::Approved);
            caption(b, words, P::State);
            crate::literal_segment::render(b, state.bytes());
            caption(b, words, if *words_present { P::WordsPresent } else { P::WordsAbsent });
        }
        OwnerWordResult::Report => caption(b, words, P::Report),
        OwnerWordResult::Verdict { code, fields } => {
            caption(b, words, P::Verdict);
            number(b, words, P::Code, u64::from(*code));
            for field in fields {
                caption(b, words, P::FieldName);
                crate::literal_segment::render(b, &field.name);
                caption(b, words, P::FieldValue);
                crate::literal_segment::render(b, &field.value);
            }
        }
        OwnerWordResult::Change { connector, kind, resource } => {
            caption(b, words, P::Change);
            number(b, words, P::Connector, u64::from(*connector));
            number(b, words, P::ChangeKind, u64::from(*kind));
            number(b, words, P::Resource, *resource);
        }
        OwnerWordResult::Failed => caption(b, words, P::ResultFailed),
        OwnerWordResult::Cancelled => caption(b, words, P::ResultCancelled),
    }
}
fn kind(b: &mut Builder, value: &OwnerMessageKind, words: &Words) {
    match value {
        OwnerMessageKind::Escalation { task, revision } => {
            caption(b, words, P::Escalation);
            number(b, words, P::TaskIdentity, *task);
            number(b, words, P::Revision, *revision);
        }
        OwnerMessageKind::Proposal { proposer, proposal, kind } => {
            caption(b, words, P::Proposal);
            number(b, words, P::Proposer, *proposer);
            number(b, words, P::ProposalNumber, *proposal);
            proposal_kind(b, *kind, words);
        }
        OwnerMessageKind::PersonStalled { task } => {
            caption(b, words, P::PersonStalled);
            number(b, words, P::TaskIdentity, *task);
        }
        OwnerMessageKind::SettingsDecision { proposal, outcome } => {
            caption(b, words, P::SettingsDecision);
            number(b, words, P::ProposalNumber, *proposal);
            match outcome {
                jig_client::contract::SettingsOutcome::Accepted { revision } => {
                    caption(b, words, P::Accepted);
                    number(b, words, P::Revision, u64::from(*revision));
                }
                jig_client::contract::SettingsOutcome::Rejected => caption(b, words, P::Rejected),
                jig_client::contract::SettingsOutcome::Stale { newest } => {
                    caption(b, words, P::SettingsStale);
                    number(b, words, P::Newest, u64::from(*newest));
                }
            }
        }
        OwnerMessageKind::ProposalDecision { proposal, accepted } => {
            caption(b, words, P::ProposalDecision);
            number(b, words, P::ProposalNumber, *proposal);
            caption(b, words, if *accepted { P::Accepted } else { P::Rejected });
        }
        OwnerMessageKind::Words => caption(b, words, P::MessageWords),
        OwnerMessageKind::EffectNotMade { entry } => {
            caption(b, words, P::EffectNotMade);
            number(b, words, P::Entry, *entry);
        }
        OwnerMessageKind::Amendment { revision } => {
            caption(b, words, P::Amendment);
            number(b, words, P::Revision, *revision);
        }
        OwnerMessageKind::QuestionAnswered { question } => number(b, words, P::QuestionAnswered, *question),
        OwnerMessageKind::QuestionEnded { question } => number(b, words, P::QuestionEnded, *question),
        OwnerMessageKind::Met { dependency, removed } => words.text(
            b,
            if *removed {
                crate::phrases::jig::JigPhrase::DependencyRemoved
            } else {
                crate::phrases::jig::JigPhrase::DependencyMet
            },
            &[Hole::Number(*dependency)],
        ),
        OwnerMessageKind::Question => caption(b, words, P::Question),
        OwnerMessageKind::Answer { question } => {
            caption(b, words, P::Answer);
            number(b, words, P::QuestionNumber, *question);
        }
        OwnerMessageKind::Notice { subscription, target, state } => {
            caption(b, words, P::Notice);
            number(b, words, P::Subscription, *subscription);
            number(b, words, P::Target, *target);
            notice(b, *state, words);
        }
        OwnerMessageKind::Timer { subscription } => {
            caption(b, words, P::Timer);
            number(b, words, P::Subscription, *subscription);
        }
        OwnerMessageKind::News { subscription, class } => {
            caption(b, words, P::News);
            number(b, words, P::Subscription, *subscription);
            news(b, *class, words);
        }
        OwnerMessageKind::Result(value) => {
            caption(b, words, P::Result);
            result(b, value, words);
        }
    }
}
/// Render every whole admitted message field. Absence of cut is not a fabricated loss flag.
pub fn word(b: &mut Builder, value: &OwnerWord, words: &Words) {
    region(b, words, P::Word);
    number(b, words, P::Message, value.number);
    caption(b, words, P::From);
    identity(b, value.from, words);
    kind(b, &value.kind, words);
    caption(b, words, P::WholeWords);
    crate::literal_segment::render(b, value.words.bytes());
    say(b, words, P::At, &[Hole::Wall(words.adjusted_wall(value.at))]);
    number(b, words, P::Hits, u64::from(value.hits));
    caption(b, words, if value.eligible { P::Eligible } else { P::Ineligible });
    b.close();
}
fn tries(b: &mut Builder, value: OwnerFailureTries, words: &Words) {
    number(b, words, P::TransientTries, u64::from(value.transient));
    number(b, words, P::PermanentTries, u64::from(value.permanent));
    number(b, words, P::RunTries, u64::from(value.run));
    number(b, words, P::AgentTries, u64::from(value.agent));
    number(b, words, P::LostTries, u64::from(value.lost));
    number(b, words, P::InvalidTries, u64::from(value.invalid));
}
fn why(b: &mut Builder, value: OwnerRunWhy, words: &Words) {
    match value {
        OwnerRunWhy::Made => caption(b, words, P::Made),
        OwnerRunWhy::Ready => caption(b, words, P::Ready),
        OwnerRunWhy::Message { message } => {
            caption(b, words, P::MessageCause);
            number(b, words, P::Message, message);
        }
        OwnerRunWhy::Released => caption(b, words, P::Released),
        OwnerRunWhy::Retry { tries: value } => {
            caption(b, words, P::Retry);
            tries(b, value, words);
        }
        OwnerRunWhy::Narrowed => caption(b, words, P::Narrowed),
        OwnerRunWhy::Changed => caption(b, words, P::Changed),
    }
}
fn failure(b: &mut Builder, value: HoldFailure, words: &Words) {
    let phrase = match value {
        HoldFailure::Transient => P::Transient,
        HoldFailure::Permanent => P::Permanent,
        HoldFailure::Run => P::RunFailure,
        HoldFailure::Agent => P::AgentFailure,
        HoldFailure::Lost => P::Lost,
        HoldFailure::Invalid => P::Invalid,
    };
    caption(b, words, phrase);
}
fn end(b: &mut Builder, value: OwnerRunEnd, words: &Words) {
    match value {
        OwnerRunEnd::Parked => caption(b, words, P::Parked),
        OwnerRunEnd::Finished => caption(b, words, P::Finished),
        OwnerRunEnd::Cancelled => caption(b, words, P::CancelledEnd),
        OwnerRunEnd::Refused => caption(b, words, P::Refused),
        OwnerRunEnd::Failed { class } => {
            caption(b, words, P::FailedEnd);
            failure(b, class, words);
        }
    }
}
fn line(b: &mut Builder, value: &OwnerActivity, words: &Words) {
    match value {
        OwnerActivity::Words { message, by, kind: value, words: text } => {
            caption(b, words, P::Words);
            number(b, words, P::Message, *message);
            caption(b, words, P::From);
            identity(b, *by, words);
            kind(b, value, words);
            caption(b, words, P::WholeWords);
            crate::literal_segment::render(b, text.bytes());
        }
        OwnerActivity::Heard(value) => {
            caption(b, words, P::Heard);
            word(b, value, words);
        }
        OwnerActivity::Notified { recipient, word: value } => {
            caption(b, words, P::Notified);
            caption(b, words, P::Recipient);
            identity(b, *recipient, words);
            word(b, value, words);
        }
        OwnerActivity::RunBegan { attempt, why: value, resumed } => {
            caption(b, words, P::RunBegan);
            number(b, words, P::Attempt, *attempt);
            why(b, *value, words);
            caption(b, words, if *resumed { P::Resumed } else { P::NotResumed });
        }
        OwnerActivity::RunEnded { attempt, end: value, spent } => {
            caption(b, words, P::RunEnded);
            number(b, words, P::Attempt, *attempt);
            end(b, *value, words);
            number(b, words, P::Spent, *spent);
        }
        OwnerActivity::Turn { reply, attempt, turn, read } => {
            caption(b, words, P::Turn);
            number(b, words, P::Reply, *reply);
            number(b, words, P::Attempt, *attempt);
            number(b, words, P::TurnNumber, u64::from(*turn));
            match read {
                Some(value) => number(b, words, P::Read, *value),
                None => caption(b, words, P::ReadAbsent),
            }
        }
        OwnerActivity::Decided { revision } => {
            caption(b, words, P::Decided);
            number(b, words, P::Revision, *revision);
        }
        OwnerActivity::Kept { attempt, pending, question } => {
            if let Some(question) = question {
                number(b, words, P::KeptQuestion, *question);
            }
            words.text(b, crate::phrases::jig::JigPhrase::DependencyParked, &[]);
            number(b, words, P::Attempt, *attempt);
            for dependency in pending {
                words.text(b, crate::phrases::jig::JigPhrase::KeptUntil, &[Hole::Number(*dependency)]);
            }
        }
        OwnerActivity::Milestone { position } => {
            caption(b, words, P::Milestone);
            number(b, words, P::Position, *position);
        }
    }
}
/// Whole supplied task, commit position and clock plus its closed activity line.
pub fn render(b: &mut Builder, value: &OwnerActivityLine, words: &Words) {
    region(b, words, P::Activity);
    number(b, words, P::Task, value.task);
    number(b, words, P::Position, value.position);
    say(b, words, P::At, &[Hole::Wall(words.adjusted_wall(value.at))]);
    line(b, &value.line, words);
    b.close();
}
