//! Party words are UTF-8 at request admission (client/domain/contract.md, 5.3).
use crate::{Ask, EscalationDecision, NoteChange, PersonResult, ProposalDecision};

pub(crate) fn valid(ask: &Ask) -> bool {
    match ask {
        Ask::Say { words, .. } | Ask::AnswerQuestion { words, .. } => text(words),
        Ask::Propose { action: crate::Action::Batch(batch), reason, .. } => text(reason) && batch_words(batch),
        Ask::Create { batch, .. } => batch_words(batch),
        Ask::MakeService { name, .. } => text(name),
        Ask::Propose { action: crate::Action::Settings { .. }, reason, .. }
        | Ask::Move { reason, .. }
        | Ask::Cancel { reason, .. }
        | Ask::Restore { reason, .. } => text(reason),
        Ask::DecideEscalation { decision, .. } => match decision {
            EscalationDecision::Reject { reason } => text(reason),
            EscalationDecision::Release | EscalationDecision::Pass => true,
        },
        Ask::DecideProposal { decision, .. } => match decision {
            ProposalDecision::Reject { reason } => text(reason),
            ProposalDecision::Accept | ProposalDecision::Pass => true,
        },
        Ask::AnswerPerson { result, .. } => match result {
            PersonResult::Report { words } | PersonResult::Verdict { words, .. } => text(words),
            PersonResult::Failure { reason } => text(reason),
            PersonResult::Approved { words, .. } => match words {
                Some(words) => text(words),
                None => true,
            },
        },
        Ask::EditNote { change, .. } => match change.as_ref() {
            NoteChange::Correct { description, body, .. } => text(description) && text(body),
            NoteChange::Delete { .. } => true,
        },
        Ask::Propose { action: crate::Action::Amend { amendment, .. }, reason, .. } => {
            text(reason) && amendment_words(amendment)
        }
        Ask::Amend { amendment, .. } => amendment_words(amendment),
        Ask::Compound { .. }
        | Ask::MarkSeen { .. }
        | Ask::Application { .. }
        | Ask::Adopt { .. }
        | Ask::TakePerson { .. }
        | Ask::HandBackPerson { .. }
        | Ask::Stop { .. }
        | Ask::Release { .. }
        | Ask::SetRoles { .. }
        | Ask::ChangePolicy { .. }
        | Ask::Rename { .. }
        | Ask::MakeProject { .. }
        | Ask::ChangeSettings { .. }
        | Ask::MoveToNewest { .. }
        | Ask::SetPool { .. }
        | Ask::Withdraw { .. }
        | Ask::AccountSignIn { .. }
        | Ask::AccountSignOut { .. }
        | Ask::AccountChange { .. } => true,
    }
}

#[expect(clippy::disallowed_methods, reason = "client/domain/contract.md, 5.3 requires UTF-8 text validation")]
fn text(bytes: &[u8]) -> bool {
    core::str::from_utf8(bytes).is_ok()
}

fn batch_words(batch: &[crate::Member]) -> bool {
    for member in batch {
        if !text(&member.words) {
            return false;
        }
        for parameter in &member.parameters {
            match parameter {
                crate::Parameter::Bytes { value, .. } => {
                    if !text(value) {
                        return false;
                    }
                }
                crate::Parameter::Number { .. } | crate::Parameter::Resource { .. } => {}
            }
        }
    }
    true
}

fn amendment_words(amendment: &crate::Amendment) -> bool {
    if !text(&amendment.reason) {
        return false;
    }
    if let Some(instructions) = &amendment.instructions
        && !text(instructions)
    {
        return false;
    }
    if let Some(spec) = &amendment.spec {
        if !text(&spec.words) {
            return false;
        }
        for parameter in &spec.parameters {
            match parameter {
                crate::Parameter::Bytes { value, .. } => {
                    if !text(value) {
                        return false;
                    }
                }
                crate::Parameter::Number { .. } | crate::Parameter::Resource { .. } => {}
            }
        }
    }
    true
}
