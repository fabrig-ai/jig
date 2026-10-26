//! Conversation words use the original admitted intent, never a later composer edit.
//! The draft owner clears that edit on its durable receipt; this module only keeps
//! the submitted words and their read position. See client/domain/client.md, 10.
use crate::{Ui, intents::Intent};
use jig_client::contract::{Answer, Ask, Limit, MeParty, Text};
use skein_lib::{Id, Token};

pub(crate) fn words(ask: &Ask, task: u64) -> Option<&Text> {
    let (words, _) = message(ask, task)?;
    Some(words)
}
fn message(ask: &Ask, task: u64) -> Option<(&Text, u32)> {
    match ask {
        Ask::Message { task: written, words, .. } => {
            if *written == task {
                Some((words, 0))
            } else {
                None
            }
        }
        Ask::Compound { operations, .. } => {
            for (place, operation) in operations.as_slice().iter().enumerate() {
                if let jig_client::contract::Operation::Message { task: written, words, .. } = operation
                    && *written == task
                {
                    return Some((words, u32::try_from(place).expect("bounded operation place fits")));
                }
            }
            None
        }
        Ask::Propose { .. }
        | Ask::Withdraw { .. }
        | Ask::Create { .. }
        | Ask::Answer { .. }
        | Ask::Take { .. }
        | Ask::HandBack { .. }
        | Ask::Decide { .. }
        | Ask::Release { .. }
        | Ask::Amend { .. }
        | Ask::Stop { .. }
        | Ask::Cancel { .. }
        | Ask::Move { .. }
        | Ask::Seen { .. }
        | Ask::EditNote { .. }
        | Ask::SignIn { .. }
        | Ask::AccountChange { .. }
        | Ask::SignOut { .. }
        | Ask::Rename { .. }
        | Ask::ChangeSettings(_)
        | Ask::MoveToNewest(_)
        | Ask::MakeProject { .. }
        | Ask::DeleteNote { .. }
        | Ask::ChangePolicy { .. }
        | Ask::SetRoles { .. }
        | Ask::MakeService { .. }
        | Ask::SetPool { .. } => None,
    }
}
/// Checks room before a new write is emitted; joined requests are already admitted.
pub(crate) fn admission(domain: &mut Ui, scope: u64, ask: &Ask) -> Option<Limit> {
    domain.conversation.words.bind(domain.confirmed);
    if domain.confirmed != Some(scope) || words(ask, domain.conversation.task).is_none() {
        return None;
    }
    if domain.conversation.words.admits() {
        None
    } else {
        Some(Limit::Words { limit: domain.conversation.words.rows.capacity() })
    }
}
/// Keeps the actual joined intent's snapshot, including across page navigation.
pub(crate) fn pending(domain: &mut Ui, id: Id<Intent>, lent: &jig_client::Client) {
    domain.conversation.words.bind(domain.confirmed);
    let intent = domain.intents.get(id).expect("admitted intent exists");
    if domain.confirmed != Some(intent.scope) {
        return;
    }
    let Some((text, place)) = message(&intent.ask, domain.conversation.task) else {
        return;
    };
    let by = author(domain, lent);
    let kept = domain.conversation.words.begin_frame(key(domain, id.token()), by, text.clone(), place);
    assert!(kept, "new writes check room; restored pending intents fit their paired capacity");
    domain.changed();
}
/// Restores pending writes first, before durable activity uses spare word room.
pub(crate) fn restore(domain: &mut Ui, lent: &jig_client::Client) {
    domain.conversation.words.bind(domain.confirmed);
    let by = author(domain, lent);
    for (id, ()) in domain.intents.ids() {
        let intent = domain.intents.get(*id).expect("registered intent exists");
        if domain.confirmed == Some(intent.scope)
            && !domain.conversation.words.pending(key(domain, id.token()))
            && let Some((text, place)) = message(&intent.ask, domain.conversation.task)
        {
            let kept = domain.conversation.words.begin_frame(key(domain, id.token()), by.clone(), text.clone(), place);
            assert!(kept, "pending intents restore before activity within their paired capacity");
        }
    }
}
/// A duplicate terminal never gets here; only a durable words result supplies a number.
pub(crate) fn answered(domain: &mut Ui, request: Token, answer: &Answer) {
    if !domain.conversation.words.pending(key(domain, request)) {
        return;
    }
    domain.conversation.answered(key(domain, request), answer);
    domain.changed();
}

fn author(domain: &Ui, lent: &jig_client::Client) -> Option<MeParty> {
    let frame = domain.frame(lent).document?;
    Some(frame.party.clone())
}

fn key(domain: &Ui, request: Token) -> jig_client::Key {
    jig_client::Key { asker: jig_client::Asker::JIG, number: request.raw() ^ domain.seed }
}
