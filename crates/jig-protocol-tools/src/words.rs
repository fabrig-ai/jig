//! The words a run reads of a relayed message (domain/hosts.md, 9).
use alloc::boxed::Box;
use jig_domain_tasks as tasks;
use skein_lib::{Decimal, Writer};

/// Render one message with its label and any revision needed for a decision.
#[must_use]
pub fn words(word: &tasks::Word) -> Box<[u8]> {
    let (label, number) = match word.kind {
        tasks::MessageKind::Result(_) => (
            b"Result from task ".as_slice(),
            match word.from {
                tasks::Party::Task(task) => task,
                tasks::Party::Person(person) => person,
                tasks::Party::Deployment { .. } => 0,
            },
        ),
        tasks::MessageKind::SettingsDecision { proposal, outcome } => return settings(proposal, outcome, &word.words),
        tasks::MessageKind::ProposalDecision { proposal, accepted } => {
            (if accepted { b"Proposal accepted ".as_slice() } else { b"Proposal rejected ".as_slice() }, proposal)
        }
        tasks::MessageKind::QuestionAnswered { question } => {
            (b"Your person answered question in their inbox ".as_slice(), question)
        }
        tasks::MessageKind::QuestionEnded { question } => (b"Question ended unanswered ".as_slice(), question),
        tasks::MessageKind::PersonStalled { task } => (b"Person task past its waiting deadline ".as_slice(), task),
        tasks::MessageKind::EffectNotMade { entry } => (b"Effect not made ".as_slice(), entry),
        tasks::MessageKind::Escalation { task, revision } => {
            return held(b"Escalation: task ", task, revision, &word.words);
        }
        tasks::MessageKind::Notice { target, state: tasks::NoticeState::Held { revision }, .. } => {
            return held(b"Task ", target, revision, &word.words);
        }
        tasks::MessageKind::Words
        | tasks::MessageKind::Question
        | tasks::MessageKind::Answer { .. }
        | tasks::MessageKind::Notice { .. }
        | tasks::MessageKind::News { .. }
        | tasks::MessageKind::Timer { .. }
        | tasks::MessageKind::Met { .. }
        | tasks::MessageKind::FinishRequested { .. }
        | tasks::MessageKind::Amendment { .. }
        | tasks::MessageKind::Proposal { .. } => return word.words.clone(),
    };
    let number = Decimal::of(number);
    let bytes = label.len().checked_add(number.as_bytes().len()).expect("bounded message label");
    let bytes = bytes.checked_add(2).expect("bounded message separator");
    let bytes = bytes.checked_add(word.words.len()).expect("bounded message envelope");
    let mut writer = Writer::new(bytes);
    writer.put(label).expect("measured label");
    writer.put(number.as_bytes()).expect("measured source");
    writer.put(b":\n").expect("measured separator");
    writer.put(&word.words).expect("measured words");
    writer.finish()
}

fn held(label: &[u8], task: u64, revision: u64, words: &[u8]) -> Box<[u8]> {
    let task = Decimal::of(task);
    let revision = Decimal::of(revision);
    let parts = [label, task.as_bytes(), b" is held at hold revision ", revision.as_bytes(), b":\n", words];
    let mut total = 0_usize;
    for part in parts {
        total = total.checked_add(part.len()).expect("bounded held message");
    }
    let mut writer = Writer::new(total);
    for part in parts {
        writer.put(part).expect("measured held message");
    }
    writer.finish()
}

fn settings(proposal: u64, outcome: tasks::SettingsOutcome, words: &[u8]) -> Box<[u8]> {
    let proposal = Decimal::of(proposal);
    let (label, revision) = match outcome {
        tasks::SettingsOutcome::Accepted { revision } => (b" accepted at revision ".as_slice(), Some(revision)),
        tasks::SettingsOutcome::Rejected => (b" rejected".as_slice(), None),
        tasks::SettingsOutcome::Stale { newest } => (b" withdrawn as stale; newest ".as_slice(), Some(newest)),
    };
    let revision = match revision {
        Some(n) => Decimal::of(u64::from(n)),
        None => Decimal::of(0),
    };
    let revision = if outcome == tasks::SettingsOutcome::Rejected { b"".as_slice() } else { revision.as_bytes() };
    let parts = [b"Settings proposal ".as_slice(), proposal.as_bytes(), label, revision, b":\n", words];
    let mut length = 0_usize;
    for part in parts {
        length = length.checked_add(part.len()).expect("bounded settings message");
    }
    let mut writer = Writer::new(length);
    for part in parts {
        writer.put(part).expect("measured settings message");
    }
    writer.finish()
}
