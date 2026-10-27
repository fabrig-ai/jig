//! Finished task and tree watches reuse the bounded durable task load (contract.md, 7.2).
use super::{Answer, Destination, Loaded, Past, Pending, Range, Row, Scope, rows};
use crate::documents::{Document, Missing, Subject};
use crate::{Core, Held, Limits, Request, Requests};
use alloc::boxed::Box;
use jig_domain_views as views;
use skein_lib::{Env, Id, Queue, Token};

pub(crate) fn start_watch(core: &mut Core, env: &Env<Limits>, watcher: Token) -> Requests {
    let task = crate::routing::view_routes::retained_task(core, watcher).expect("task construction");
    let binding = core.view_subjects.get(&watcher).expect("bound construction");
    let range = Range {
        party: binding.party,
        scope: Scope::Task { task },
        after: None,
        backwards: false,
        rows: env.limits.documents.rows,
        bytes: env.limits.load_bytes,
    };
    let pending = Pending {
        destination: Destination::Watch(watcher),
        sign_in: binding.sign_in,
        party: binding.party,
        range: range.clone(),
        tasks: None,
    };
    let Ok(load) = core.reads.insert(pending) else {
        return crate::routing::view_routes::retained_made(core, env, watcher, Err(views::End::Gone));
    };
    let mut out = Queue::with_capacity(2);
    out.push(Request::Held(Box::new(Held::ReadLoad { load: load.token(), range: Box::new(range) })));
    out.push(Request::Decided);
    Requests::Out(out)
}

pub(super) fn watch_loaded(
    core: &mut Core,
    env: &Env<Limits>,
    load: Id<Pending>,
    watcher: Token,
    result: Loaded,
) -> Requests {
    let pending = core.reads.get_mut(load).expect("pending watch load");
    pending.destination = Destination::Done;
    let sign_in = pending.sign_in;
    let party = pending.party;
    let range = pending.range.clone();
    core.reads.retire(load);
    let document = if core.people.person(sign_in, env.now, env.wall) == Some(party) {
        from_store(core, env, watcher, &range, result)
    } else {
        Err(views::End::SignedOut)
    };
    crate::routing::view_routes::retained_made(core, env, watcher, document)
}

fn from_store(
    core: &Core,
    env: &Env<Limits>,
    watcher: Token,
    range: &Range,
    result: Loaded,
) -> Result<Document, views::End> {
    let (raw, next) = match result {
        Loaded::Page { rows, next } => (rows, next),
        Loaded::TooLinked { .. } | Loaded::Busy | Loaded::Gone | Loaded::TurnProblem { .. } => {
            return Err(views::End::Gone);
        }
    };
    let Some(binding) = core.view_subjects.get(&watcher) else { return Err(views::End::Gone) };
    let subject = &core.view_names.get(binding.subject).expect("bound task subject").subject;
    // Validate the same row, byte, identity and visibility bounds as an ordinary task read.
    match subject {
        Subject::Tree { .. } => return tree(core, &env.limits, range, &raw, next.is_some()),
        Subject::Task { .. } => {}
        Subject::Frame { .. }
        | Subject::Chips { .. }
        | Subject::Inbox { .. }
        | Subject::Accounts { .. }
        | Subject::Query { .. }
        | Subject::Run { .. }
        | Subject::Settings
        | Subject::Deployment
        | Subject::Proposal { .. }
        | Subject::Escalation { .. }
        | Subject::Question { .. }
        | Subject::PersonTask { .. }
        | Subject::Result { .. }
        | Subject::Project { .. }
        | Subject::Note { .. }
        | Subject::Application { .. } => return Err(views::End::Gone),
    }
    match rows::from_store(core, &env.limits, range, raw, next, env.wall) {
        Answer::Historical(value) => {
            let document = match value.value {
                Past::Task { document, .. } => document,
                Past::PersonProposal { .. }
                | Past::Proposal(_)
                | Past::Escalation { .. }
                | Past::Question { .. }
                | Past::PersonTask { .. }
                | Past::Turn(_)
                | Past::Reply(_) => return Err(views::End::Gone),
            };
            Ok(Document::Task(document))
        }
        Answer::Setting(_)
        | Answer::Revision(_)
        | Answer::Revisions(_)
        | Answer::Tasks(_)
        | Answer::Document(_)
        | Answer::Page(_)
        | Answer::None(_) => Err(views::End::Gone),
    }
}

// A point tree needs only its retained root, never the task page's whole-value bounds.
fn tree(core: &Core, limits: &Limits, range: &Range, raw: &[Row], more: bool) -> Result<Document, views::End> {
    if raw.len() != 1 || range.rows == 0 || more {
        return Err(views::End::Gone);
    }
    let row = raw.first().expect("one retained root");
    let Some(bytes) = row.owned_bytes() else { return Err(views::End::Gone) };
    if bytes > range.bytes || !super::admits(range.party, &range.scope, &row.value.key()) {
        return Err(views::End::Gone);
    }
    let Some(owner) = super::vocabulary::row_owner(row) else { return Err(views::End::Gone) };
    match crate::documents::task_tree::ended_tree(core, limits, range.party, owner) {
        Ok(tree) => Ok(Document::Tree(tree)),
        Err(Missing::NotAllowed | Missing::Unknown | Missing::Ended | Missing::TooLarge) => Err(views::End::Gone),
    }
}
