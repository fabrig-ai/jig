//! Notice outcomes without a visible card (client/domain/client.md, 13).
use crate::JigBindings;
use crate::cards::common;
use crate::phrases::jig::JigPhrase;
use crate::words::Words;
use skein_ui::phrases::Hole;
use skein_ui::{Builder, Role};

use crate::{Borrowed, action::Act, notices::kept::Kind, page::IntentPurpose};
use jig_client::contract::{Answer, ObjectKey, SeenThrough};
/// Build each visible notice as a named status, with one oldest-notice dismissal.
pub fn notices(b: &mut Builder, client: &Borrowed<'_>, words: &Words, bindings: &mut JigBindings) {
    let text_limit = bindings.text_limit();
    if client.notices().ids().is_empty() {
        return;
    }
    b.open(Role::Region);
    b.region(crate::regions::JigRegion::Notices.region());
    b.name(&words.say(JigPhrase::NoticesPart, &[]));
    for (id, ()) in client.notices().ids() {
        if let Some(notice) = client.notices().get(*id) {
            facts(b, &notice.kind, words, text_limit);
        }
    }
    if !client.notices().ids().is_empty()
        && client.confirmation().is_none()
        && client.project_page().form.confirmation.is_none()
    {
        crate::frame::control(b, words, bindings, Role::Button, JigPhrase::Dismiss, &[], Act::Dismiss);
    }
    b.close();
}
/// Whole notice facts, shared by the status renderer and vocabulary tests.
pub fn facts(b: &mut Builder, kind: &Kind, words: &Words, text_limit: u32) {
    b.open(Role::Status);
    b.name(&words.say(JigPhrase::Notice, &[]));
    match kind {
        Kind::Dropped { count } => words.text(b, JigPhrase::RequestsDropped, &[Hole::Number(u64::from(*count))]),
        Kind::Full { held, room_at } => words.answers_full(b, *held, *room_at),
        Kind::NotSent(why) => words.text(b, why.phrase(), &[]),
        Kind::KeptUnknown => words.text(b, JigPhrase::UnknownAction, &[]),
        Kind::SignOutLost => words.text(b, JigPhrase::SignOutLost, &[]),
        Kind::Link(link) => words.link(b, *link),
        Kind::Answer { object, answer: outcome } => {
            object_words(b, *object, words);
            answer(b, outcome, words, text_limit);
        }
        Kind::PageAnswer { answer: outcome } => answer(b, outcome, words, text_limit),
        Kind::Unknown { object, act } => {
            object_words(b, *object, words);
            words.text(b, common::act_phrase(*act), &[]);
            words.text(b, JigPhrase::UnknownAction, &[]);
        }
        Kind::PageUnknown { purpose } => {
            match purpose {
                IntentPurpose::ProjectSettings => words.text(b, JigPhrase::Settings, &[]),
                IntentPurpose::BeginConversation => words.text(b, JigPhrase::Conversations, &[]),
                IntentPurpose::CreateGoal => words.text(b, JigPhrase::ActSetGoal, &[]),
                IntentPurpose::Seen(seen) => match seen {
                    SeenThrough::Results { through } => {
                        words.text(b, JigPhrase::ResultsThrough, &[Hole::Number(*through)]);
                    }
                    SeenThrough::Conversation { task: chat, through } => {
                        words.text(b, JigPhrase::RepliesThrough, &[Hole::Number(*chat), Hole::Number(*through)]);
                    }
                },
            }
            words.text(b, JigPhrase::UnknownAction, &[]);
        }
        Kind::TooLinked { joins } => words.text(b, JigPhrase::TooLinked, &[Hole::Number(u64::from(*joins))]),
        Kind::Refused(which) => words.refusal(b, &jig_client::contract::Refusal::Limit { which: *which }, text_limit),
    }
    b.close();
}
fn answer(b: &mut Builder, answer: &Answer, words: &Words, text_limit: u32) {
    match answer {
        Answer::Done(outcome) => words.outcome(b, outcome),
        Answer::Refused { refusal: why, .. } => words.refusal(b, why, text_limit),
    }
}
fn object_words(b: &mut Builder, object: ObjectKey, words: &Words) {
    match object {
        ObjectKey::Task(task) | ObjectKey::Escalation(task) | ObjectKey::PersonTask(task) | ObjectKey::Result(task) => {
            words.text(b, JigPhrase::TaskPage, &[Hole::Number(task)]);
        }
        ObjectKey::Question { task, message } => {
            words.text(b, JigPhrase::RepliesThrough, &[Hole::Number(task), Hole::Number(message)]);
        }
        ObjectKey::Proposal(key) => {
            words.proposal(b, key);
        }
        ObjectKey::Note { project, note } => {
            words.text(b, JigPhrase::ContextProject, &[Hole::Number(u64::from(project))]);
            words.text(b, JigPhrase::MadeNote, &[Hole::Number(note)]);
        }
    }
}

pub mod kept;
